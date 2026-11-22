// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC VERIFIER (v0.2)                                 │
// │  Z3-based formal verification of candidate implementations│
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::*;
use crate::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, IRPattern, IRUnaryOp,
    ScoreBreakdown, SpecIndex, composite_score, MAX_INLINE_DEPTH,
};
#[cfg(feature = "z3-support")]
use super::smt::{Z3Session, SmtResult, SmtValue, z3_add, z3_sub, z3_mul, z3_and};
use std::collections::HashMap;
#[cfg(feature = "z3-support")]
use std::str::FromStr;

// ── Public Types ───────────────────────────────────────────
//
// The verdict vocabulary lives in super::verdict (pure data,
// Phase 66) and is re-exported here so every existing
// `verify::verifier::{VerificationStatus, ...}` path keeps
// resolving in both build configurations.
pub use super::verdict::{
    ConstraintResult, Counterexample, VerificationError, VerificationResult,
    VerificationStatus,
};

// ── Verifier ───────────────────────────────────────────────

pub struct Verifier {
    timeout_per_constraint_ms: u64,
    /// Unrolling fuel for statement-mode judging
    /// (Phase 93): the per-loop iteration bound of the
    /// statement encoder's BMC discipline when
    /// `verify_one` engages it. See `STMT_VERIFY_FUEL`
    /// for the derivation of the default.
    stmt_fuel: usize,
    /// The module the verified spec lives in, for resolving
    /// spec-to-spec calls by inlining (see `sym_invoke`).
    /// Empty unless built with `with_specs`/`with_index` — a
    /// standalone verifier judges exactly what it judged
    /// before: calls it cannot resolve make a body or a
    /// constraint unencodable, hence Inconclusive.
    index: SpecIndex,
}

/// The production unrolling fuel for statement-mode
/// judging in `verify_one` (Phase 93). The BMC discipline
/// (ENCODER_EXTENSION_DESIGN.md §2.2) is sound at ANY
/// fuel — a higher fuel only completes more paths — so
/// the default is a cost decision, and it is the smallest
/// fuel with the full measured symbolic product:
///
/// - every symbolic outcome Phase 90 measured is already
///   decided at fuel 1: skip Unknown (exhaustion is
///   reachable at every fuel — no fuel Verifies an
///   unbounded loop symbolically), the STEP body's row 2
///   Refuted (the `" 5"` countermodel completes within
///   one iteration), the LOOP body's row Unknown;
/// - point precision is NOT this fuel's job: battery
///   points are judged through `sym_eval_stmt` with the
///   per-point fuel the trip counts dictate (skip = trip
///   count, STEP = max(skip, ident+1), LOOP = max(outer,
///   inner)), supplied by the caller;
/// - the cost wall is real: nested-denotation unrolling
///   grows ~×8 per iteration in merging mode, and the
///   LOOP composition at fuel 2 exceeded a 280 s wall in
///   the Phase 90 measurement. A `verify_one` default
///   must never be able to do that to a synthesis pool.
const STMT_VERIFY_FUEL: usize = 1;

impl Default for Verifier {
    fn default() -> Self {
        Self {
            timeout_per_constraint_ms: 5_000,
            stmt_fuel: STMT_VERIFY_FUEL,
            index: SpecIndex::empty(),
        }
    }
}

impl Verifier {
    pub fn new() -> Self { Self::default() }

    /// A verifier that knows the whole module the spec belongs
    /// to, so calls to sibling specs — in candidate bodies and
    /// in postconditions alike — resolve by inlining the
    /// callee's body.
    pub fn with_specs(specs: &[FunctionSpec]) -> Self {
        Self::with_index(SpecIndex::from_specs(specs))
    }

    pub fn with_index(index: SpecIndex) -> Self {
        Self {
            index,
            ..Self::default()
        }
    }

    /// A verifier that knows the whole module the spec
    /// belongs to — specs AND type declarations — so
    /// record-typed parameters bind symbolically
    /// (Phase 76, `Sym::Record`) in addition to spec-to-spec
    /// inlining.
    pub fn with_module(module: &Spec) -> Self {
        Self::with_index(SpecIndex::from_module(module))
    }

    pub fn verify_all(
        &self,
        candidates: Vec<CandidateImplementation>,
        spec: &FunctionSpec,
    ) -> Result<Vec<CandidateImplementation>, VerificationError> {
        let mut verified = Vec::with_capacity(candidates.len());
        for mut candidate in candidates {
            let result = self.verify_one(&candidate, spec);
            candidate.verified = result.status == VerificationStatus::Verified;
            // Record the full verdict on the candidate itself:
            // selection's refutation gate reads exactly this —
            // the verdict computed here, nowhere recomputed.
            candidate.verification_status = Some(result.status.clone());
            let passed = result.constraint_results.iter().filter(|r| r.holds).count();
            let total = result.constraint_results.len();
            if total > 0 {
                candidate.scores.constraint_score = passed as f64 / total as f64;
            }
            candidate.score = composite_score(&candidate.scores);
            verified.push(candidate);
        }
        Ok(verified)
    }

    #[cfg(feature = "z3-support")]
    pub fn verify_one(
        &self,
        candidate: &CandidateImplementation,
        spec: &FunctionSpec,
    ) -> VerificationResult {
        let start = crate::timeutil::now();
        let mut constraint_results = Vec::new();
        let mut counterexample = None;

        if !is_well_formed(&candidate.body) {
            return VerificationResult {
                candidate_id: candidate.id, status: VerificationStatus::Malformed,
                constraint_results: vec![], counterexample: None,
                verification_time_ms: start.elapsed_ms(),
            };
        }

        // The verdict must be a statement about THIS candidate:
        // the spec's parameters become free Z3 constants and the
        // candidate body is symbolically evaluated over them,
        // once, producing the expression `output` denotes for
        // this implementation. `None` = the body is outside the
        // encodable subset; every constraint is then Unknown and
        // the candidate can only be Inconclusive, never Verified.
        let param_env = param_env(spec, &self.index);
        let output = sym_eval(
            &candidate.body,
            &mut param_env.clone(),
            &EvalCtx::root(&self.index),
        );

        // Statement-mode routing (Phase 93): the
        // statement encoder engages ONLY where the
        // expression encoder declined AND the decline is
        // for statement/loop nodes — in the body itself
        // or in a denotation it calls. Every other
        // decline (holes, lambdas, guard patterns,
        // unresolvable calls) keeps the standing outcome:
        // no output, every constraint Unknown. When the
        // statement evaluation itself declines, the same
        // fallback applies — the outcome below is then
        // None and the constraint loop is byte-for-byte
        // the pre-Phase-93 one.
        let stmt_outcome = if output.is_none()
            && body_mentions_statements(&candidate.body, &self.index)
        {
            let ctx = StmtCtx::root(
                &self.index,
                self.stmt_fuel,
                self.timeout_per_constraint_ms,
            );
            sym_eval_stmt(&candidate.body, param_env.clone(), &ctx)
        } else {
            None
        };

        let mut any_refuted = false;
        let mut any_unknown = false;
        for (i, c) in spec.postconditions.iter().enumerate() {
            let (result, tri) = match (&output, &stmt_outcome) {
                (None, Some(outcome)) => self.verify_constraint_stmt(
                    c,
                    &format!("post[{}]", i),
                    spec,
                    &param_env,
                    outcome,
                ),
                _ => self.verify_constraint(
                    c,
                    &format!("post[{}]", i),
                    spec,
                    &param_env,
                    &output,
                ),
            };
            match tri {
                Tri::Refuted => {
                    any_refuted = true;
                    if counterexample.is_none() {
                        counterexample = Some(Counterexample {
                            description: format!("Postcondition {} violated", i + 1),
                            inputs: result.model.clone().unwrap_or_default(),
                        });
                    }
                }
                Tri::Unknown => any_unknown = true,
                Tri::Proven => {}
            }
            constraint_results.push(result);
        }

        VerificationResult {
            candidate_id: candidate.id,
            status: if any_refuted {
                VerificationStatus::VerificationFailed
            } else if any_unknown {
                VerificationStatus::Inconclusive
            } else {
                VerificationStatus::Verified
            },
            constraint_results, counterexample,
            verification_time_ms: start.elapsed_ms(),
        }
    }

    /// No-Z3 backend (Phase 66): formal verification is
    /// unavailable in this build (v0.6 D2 — the verdict is
    /// labelled, never simulated). A holed body is Malformed:
    /// that judgment is pure and stays. Every other candidate
    /// is Inconclusive, with one non-holding result per
    /// postcondition — exactly what the Z3 backend reports
    /// for a body it cannot encode, so `verify_all`'s
    /// bookkeeping (constraint_score = 0/total, the
    /// unencodable body's number) is the same here, and no
    /// constraint is ever silently treated as true.
    #[cfg(not(feature = "z3-support"))]
    pub fn verify_one(
        &self,
        candidate: &CandidateImplementation,
        spec: &FunctionSpec,
    ) -> VerificationResult {
        let start = crate::timeutil::now();

        if !is_well_formed(&candidate.body) {
            return VerificationResult {
                candidate_id: candidate.id, status: VerificationStatus::Malformed,
                constraint_results: vec![], counterexample: None,
                verification_time_ms: start.elapsed_ms(),
            };
        }

        let constraint_results = spec
            .postconditions
            .iter()
            .enumerate()
            .map(|(i, _)| ConstraintResult {
                constraint: format!("post[{}]", i),
                holds: false,
                solver_time_ms: 0,
                model: None,
            })
            .collect();

        VerificationResult {
            candidate_id: candidate.id,
            status: VerificationStatus::Inconclusive,
            constraint_results,
            counterexample: None,
            verification_time_ms: start.elapsed_ms(),
        }
    }

    #[cfg(feature = "z3-support")]
    fn verify_constraint(
        &self,
        constraint: &Constraint,
        label: &str,
        spec: &FunctionSpec,
        param_env: &SymEnv,
        output: &Option<Sym>,
    ) -> (ConstraintResult, Tri) {
        let start = crate::timeutil::now();
        let done = |holds: bool, model: Option<HashMap<String, SmtValue>>, tri: Tri| {
            (
                ConstraintResult {
                    constraint: label.into(),
                    holds,
                    solver_time_ms: start.elapsed_ms(),
                    model,
                },
                tri,
            )
        };

        // No symbolic output (unencodable body) or an
        // unencodable constraint: the honest outcome is
        // Unknown. Nothing is silently treated as true.
        let Some(output) = output else {
            return done(false, None, Tri::Unknown);
        };
        let mut env = param_env.clone();
        env.insert("output".to_string(), output.clone());
        let Some(formula) = encode_constraint(constraint, &env, &EvalCtx::root(&self.index)) else {
            return done(false, None, Tri::Unknown);
        };

        let z3 = Z3Session::new(self.timeout_per_constraint_ms);
        // D5a side conditions: every Char parameter (and
        // every Char nested in a record/Option/Result
        // parameter) ranges over the Unicode scalar values
        // — without these, a "Char" constant could be a
        // surrogate or beyond U+10FFFF and scalar-order
        // proofs would be proving a different type.
        for p in &spec.params {
            if let Some(sym) = param_env.get(&p.name) {
                let mut conditions = Vec::new();
                collect_char_conditions(sym, &mut conditions);
                for cond in &conditions {
                    z3.assert(cond);
                }
            }
        }
        let negated = formula.not();
        z3.assert(&negated);

        // Int parameters, for real counterexample values.
        let int_params: Vec<(&str, &z3::ast::Int)> = spec
            .params
            .iter()
            .filter_map(|p| match param_env.get(&p.name) {
                Some(Sym::Int(ast)) => Some((p.name.as_str(), ast)),
                _ => None,
            })
            .collect();

        match z3.check_and_eval_ints(&int_params) {
            SmtResult::Unsat => done(true, None, Tri::Proven),
            SmtResult::Sat(model) => done(
                false,
                if model.is_empty() { None } else { Some(model) },
                Tri::Refuted,
            ),
            _ => done(false, None, Tri::Unknown),
        }
    }

    /// The statement-mode judge (Phase 93): one
    /// postcondition against a statement evaluation's
    /// outcome, under the BMC discipline of
    /// ENCODER_EXTENSION_DESIGN.md §2.2, verbatim:
    ///
    /// - **Refuted** ⟺ some COMPLETED path has
    ///   `side ∧ pc ∧ ¬exhausted ∧ ¬formula` satisfiable
    ///   (a countermodel on a path the unrolling ran to
    ///   completion — a real countermodel);
    /// - **Proven** ⟺ no completed path refutes AND
    ///   `side ∧ exhausted` is unsatisfiable (no path
    ///   was cut off by the fuel bound);
    /// - **Unknown** otherwise: exhaustion reachable, a
    ///   solver Unknown anywhere, or the row unencodable
    ///   on some path (the W8 literal-tag boundaries
    ///   judge Unknown here exactly as in
    ///   `verify_constraint`).
    ///
    /// The verdict vocabulary is the same `Tri` /
    /// `VerificationStatus` the expression judge
    /// produces, and `verify_one` folds it into the
    /// candidate status by the same rule — the selector
    /// (`composite_score`, the refutation gate) reads no
    /// new status and gains no Verified-preference from
    /// this path: a statement body reaches Verified here
    /// only when its loops provably exit within the fuel
    /// on every constructible path, which for the
    /// unbounded tokenizer loops over symbolic inputs
    /// never happens (that route is Phase 96's
    /// invariants, not unrolling).
    #[cfg(feature = "z3-support")]
    fn verify_constraint_stmt(
        &self,
        constraint: &Constraint,
        label: &str,
        spec: &FunctionSpec,
        param_env: &SymEnv,
        outcome: &StmtOutcome,
    ) -> (ConstraintResult, Tri) {
        let start = crate::timeutil::now();
        let done = |holds: bool, model: Option<HashMap<String, SmtValue>>, tri: Tri| {
            (
                ConstraintResult {
                    constraint: label.into(),
                    holds,
                    solver_time_ms: start.elapsed_ms(),
                    model,
                },
                tri,
            )
        };

        // The D5a Char side conditions, asserted exactly
        // as in `verify_constraint`.
        let mut side: Vec<z3::ast::Bool> = Vec::new();
        for p in &spec.params {
            if let Some(sym) = param_env.get(&p.name) {
                collect_char_conditions(sym, &mut side);
            }
        }
        // Int parameters, for real counterexample values.
        let int_params: Vec<(&str, &z3::ast::Int)> = spec
            .params
            .iter()
            .filter_map(|p| match param_env.get(&p.name) {
                Some(Sym::Int(ast)) => Some((p.name.as_str(), ast)),
                _ => None,
            })
            .collect();

        let not_exhausted = outcome.exhausted.not();
        let mut any_unknown = false;
        for (pc, output) in &outcome.paths {
            let mut env = param_env.clone();
            env.insert("output".to_string(), output.clone());
            let Some(formula) =
                encode_constraint(constraint, &env, &EvalCtx::root(&self.index))
            else {
                return done(false, None, Tri::Unknown);
            };
            let z3 = Z3Session::new(self.timeout_per_constraint_ms);
            for cond in &side {
                z3.assert(cond);
            }
            z3.assert(pc);
            z3.assert(&not_exhausted);
            z3.assert(&formula.not());
            match z3.check_and_eval_ints(&int_params) {
                SmtResult::Sat(model) => {
                    return done(
                        false,
                        if model.is_empty() { None } else { Some(model) },
                        Tri::Refuted,
                    );
                }
                SmtResult::Unsat => {}
                _ => any_unknown = true,
            }
        }
        // The exhaustion gate: Proven requires that the
        // fuel bound cut off NO path — otherwise the
        // completed paths' silence proves nothing about
        // the paths that wanted another iteration.
        let z3 = Z3Session::new(self.timeout_per_constraint_ms);
        for cond in &side {
            z3.assert(cond);
        }
        z3.assert(&outcome.exhausted);
        match z3.check() {
            SmtResult::Unsat if !any_unknown => done(true, None, Tri::Proven),
            _ => done(false, None, Tri::Unknown),
        }
    }
}

/// Three-valued outcome of one constraint check against one
/// candidate. `ConstraintResult::holds` records only `Proven`;
/// the distinction between Refuted and Unknown drives the
/// candidate's overall status (Failed vs Inconclusive).
#[cfg(feature = "z3-support")]
enum Tri {
    Proven,
    Refuted,
    Unknown,
}

// ── Symbolic candidate encoding ────────────────────────────
//
// The candidate body is folded over the spec's parameters as
// free Z3 constants, mirroring the evaluable subset of the IR
// interpreter (synthesis/eval.rs) construct by construct:
//
//   Literal, Var, BinOp, UnaryOp (Neg/Not/Abs/Len/Clone), If, Match
//   (literal/variable/wildcard patterns, and constructor
//   patterns over Option/Result — Phase 91), Let, Block, Return,
//   Call ("len", the "List" literal, the Ok/Err/Some/None
//   constructors, and — when the verifier carries the module —
//   calls to other specs, inlined), Collection, Index, Typed,
//   FieldAccess, RecordLit (Phase 76).
//
// Anything else — holes, loops, mutation, lambdas and other
// function values, higher-order combinators, calls that do
// not resolve, guard patterns, and constructor patterns
// outside the tag+payload model — makes the body unencodable
// (`None`), never a guessed value.
//
// Representable sorts are Int, Bool and String, plus literal
// lists of representable values (needed for len/Index over
// collection literals), records (per-field values), the
// Result/Option tag+payload forms, and Char (Phase 76, D5a:
// a Char is its Unicode scalar value on the Int sort; a Char
// PARAM binds with the scalar range side-condition asserted
// at check time — see `char_range_condition`). Floats have
// no honest Z3 sort here (IEEE floats are not Z3 reals), so
// they are `Opaque`: bindable and discardable, but any use
// that needs a concrete sort makes the body unencodable.

/// A sum-type constructor tag. Literal tags arise from
/// constructor calls (`Ok(...)` is `Tag::True`); a symbolic
/// tag arises from a parameter binding or an `ite` merge.
/// The distinction is load-bearing for equality: comparing
/// against a literal `None`/`Err` needs no payload term at
/// all (see `sym_option_eq` / `sym_result_eq`).
#[derive(Debug, Clone, PartialEq)]
#[cfg(feature = "z3-support")]
enum Tag {
    True,
    False,
    Free(z3::ast::Bool),
}

#[cfg(feature = "z3-support")]
impl Tag {
    fn term(&self) -> z3::ast::Bool {
        match self {
            Tag::True => z3::ast::Bool::from_bool(true),
            Tag::False => z3::ast::Bool::from_bool(false),
            Tag::Free(b) => b.clone(),
        }
    }

    /// The tag of `if cond { a } else { b }` over two tagged
    /// values: a shared literal survives; anything else is
    /// the free ite term.
    fn merge(cond: &z3::ast::Bool, a: &Tag, b: &Tag) -> Tag {
        if a == b {
            a.clone()
        } else {
            Tag::Free(cond.ite(&a.term(), &b.term()))
        }
    }
}

#[derive(Debug, Clone)]
#[cfg(feature = "z3-support")]
enum Sym {
    Int(z3::ast::Int),
    Bool(z3::ast::Bool),
    Str(z3::ast::String),
    List(Vec<Sym>),
    /// A list whose elements are unmodelled but whose
    /// LENGTH is this Int term — the binding of a List-typed
    /// parameter or record field (Phase 76). `len` reads the
    /// term; element access and list equality are
    /// unencodable (a length is not the list).
    ListLen(z3::ast::Int),
    /// A Char as its Unicode scalar value (Phase 76, D5a).
    /// Deliberately NOT `Sym::Int`: sort-strictness — a
    /// Char never compares or computes as an Int. A Char
    /// term built from a literal is a Z3 numeral; a Char
    /// parameter is a free constant carrying the range
    /// side-condition.
    Char(z3::ast::Int),
    /// The character at a string index, as a length-≤1
    /// string term (`s.at(i)`) — the String-index→Char
    /// bridge (Phase 76). The safe z3 API exposes `str.at`
    /// but no `str.to_code`, and the raw context handle the
    /// FFI would need is not public, so the indexed char
    /// cannot be joined to the scalar sort in general. What
    /// this form supports, soundly: equality/order against
    /// another indexed char (identical or not — Z3 string
    /// order on length-≤1 strings IS scalar order, with the
    /// out-of-bounds empty string sorting below every real
    /// char) and against a CONSTANT Char (converted to its
    /// one-char string). Against a SYMBOLIC Char parameter
    /// it is unencodable — that is the bridge's named
    /// boundary. Out-of-bounds is totalized as the empty
    /// string: unequal to every real char, below every real
    /// char — the same documented totalization class as
    /// Z3's Div/Mod (the interpreter fails there instead).
    /// A CONSTANT index into a LITERAL string stays exact
    /// (the real scalar, or unencodable out of bounds).
    CharStr(z3::ast::String),
    /// A record value (Phase 76, D2a): the resolved
    /// declaration name plus per-field values. A record
    /// parameter binds as a record of per-field fresh values
    /// (`fresh_sym` over the declared field types).
    Record {
        type_name: String,
        fields: Vec<(String, Sym)>,
    },
    /// An Option value (Phase 76): constructor tag plus the
    /// Some payload. Invariant: the payload is absent ONLY
    /// when the tag is the literal `Tag::False` (a `None`
    /// construction, possibly merged with itself) — a tag
    /// that can be true always carries a payload.
    Option {
        some: Tag,
        payload: Option<Box<Sym>>,
    },
    /// A Result value (Phase 76): constructor tag plus the
    /// per-constructor payloads. Invariants mirror Option:
    /// the Ok payload is absent only under a literal Err
    /// tag, the Err payload only under a literal Ok tag.
    Result {
        ok: Tag,
        ok_payload: Option<Box<Sym>>,
        err_payload: Option<Box<Sym>>,
    },
    Opaque,
}

#[cfg(feature = "z3-support")]
type SymEnv = HashMap<String, Sym>;

/// Resolution context for spec-to-spec calls, threaded
/// through every encoder in this module: the module index to
/// resolve against, the stack of spec names currently being
/// inlined on this path (the cycle guard), and the inlining
/// depth (the expansion cap). A call whose target is already
/// on the stack, or that would exceed
/// `MAX_INLINE_DEPTH`, is unencodable — the honest outcome is
/// Inconclusive, never a guessed value or an unbounded
/// expansion.
#[cfg(feature = "z3-support")]
struct EvalCtx<'a> {
    index: &'a SpecIndex,
    visiting: Vec<String>,
    depth: usize,
}

#[cfg(feature = "z3-support")]
impl<'a> EvalCtx<'a> {
    fn root(index: &'a SpecIndex) -> Self {
        Self {
            index,
            visiting: Vec::new(),
            depth: 0,
        }
    }

    fn can_inline(&self, name: &str) -> bool {
        self.depth < MAX_INLINE_DEPTH && !self.visiting.iter().any(|v| v == name)
    }

    fn child(&self, name: &str) -> Self {
        let mut visiting = self.visiting.clone();
        visiting.push(name.to_string());
        Self {
            index: self.index,
            visiting,
            depth: self.depth + 1,
        }
    }
}

/// Invoke a call to another spec, symbolically: the callee's
/// body — `SpecIndex::callee_body`, the same body the callee's
/// constraint seed is built from — is evaluated in a fresh
/// environment binding the callee's parameters to the
/// already-encoded argument values. Substituting VALUES, not
/// syntax, makes capture impossible by construction. Shared
/// by the body side (`sym_call`, IR calls) and the constraint
/// side (`encode_expr`, spec-expression calls), so a call
/// denotes the same thing in a candidate and in the
/// postcondition it is judged against.
#[cfg(feature = "z3-support")]
fn sym_invoke(ctx: &EvalCtx, name: &str, args: Vec<Sym>) -> Option<Sym> {
    if !ctx.can_inline(name) {
        return None;
    }
    let callee = ctx.index.get(name)?;
    if callee.params.len() != args.len() {
        return None;
    }
    let body = ctx.index.callee_body(name)?;
    let mut env: SymEnv = callee
        .params
        .iter()
        .map(|p| p.name.clone())
        .zip(args)
        .collect();
    sym_eval(&body, &mut env, &ctx.child(name))
}

/// Internal control flow, mirroring the interpreter: `Return`
/// unwinds to the top of the candidate body.
#[cfg(feature = "z3-support")]
enum SymFlow {
    Normal(Sym),
    Returned(Sym),
}

#[cfg(feature = "z3-support")]
impl SymFlow {
    fn into_value(self) -> Sym {
        match self {
            SymFlow::Normal(v) | SymFlow::Returned(v) => v,
        }
    }
}

/// The initial symbolic environment: one free symbolic
/// value per spec parameter, built from the parameter's
/// TYPE by `fresh_sym`. Parameters whose type has no
/// representable sort are Opaque.
#[cfg(feature = "z3-support")]
fn param_env(spec: &FunctionSpec, index: &SpecIndex) -> SymEnv {
    let mut env = SymEnv::new();
    for p in &spec.params {
        let sym = fresh_sym(&p.ty, &p.name, index, 0);
        env.insert(p.name.clone(), sym);
    }
    env
}

/// Recursion bound for `fresh_sym`: a self-referential
/// record (`Node { next: Option<Node> }`) would otherwise
/// expand forever. Past the bound the value binds Opaque —
/// honest, and far deeper than any module in the tree.
#[cfg(feature = "z3-support")]
const MAX_FRESH_DEPTH: usize = 8;

/// A fresh symbolic value of a type: the parameter-binding
/// half of the Phase 76 representations. `path` names the
/// Z3 constants after the value's position (`state`,
/// `state.pos`, `output.is_ok`, …) so counterexamples read
/// in the spec's own vocabulary. Record fields recurse over
/// the DECLARED field types (the index must carry the
/// module's declarations — `SpecIndex::from_module`; with a
/// declarations-less index a record type binds Opaque,
/// exactly the pre-Phase-76 behavior). A List binds its
/// length only (`Sym::ListLen`): elements are unmodelled.
#[cfg(feature = "z3-support")]
fn fresh_sym(ty: &TypeRef, path: &str, index: &SpecIndex, depth: usize) -> Sym {
    if depth > MAX_FRESH_DEPTH {
        return Sym::Opaque;
    }
    match ty {
        TypeRef::Int
        | TypeRef::I8
        | TypeRef::I16
        | TypeRef::I32
        | TypeRef::I64
        | TypeRef::U8
        | TypeRef::U16
        | TypeRef::U32
        | TypeRef::U64 => Sym::Int(z3::ast::Int::new_const(path)),
        TypeRef::Bool => Sym::Bool(z3::ast::Bool::new_const(path)),
        TypeRef::String => Sym::Str(z3::ast::String::new_const(path)),
        TypeRef::Char => Sym::Char(z3::ast::Int::new_const(path)),
        TypeRef::List(_) => {
            Sym::ListLen(z3::ast::Int::new_const(format!("{path}.len").as_str()))
        }
        TypeRef::Option(inner) => Sym::Option {
            some: Tag::Free(z3::ast::Bool::new_const(
                format!("{path}.is_some").as_str(),
            )),
            payload: Some(Box::new(fresh_sym(
                inner,
                &format!("{path}.some"),
                index,
                depth + 1,
            ))),
        },
        TypeRef::Result(ok_ty, err_ty) => Sym::Result {
            ok: Tag::Free(z3::ast::Bool::new_const(
                format!("{path}.is_ok").as_str(),
            )),
            ok_payload: Some(Box::new(fresh_sym(
                ok_ty,
                &format!("{path}.ok"),
                index,
                depth + 1,
            ))),
            err_payload: Some(Box::new(fresh_sym(
                err_ty,
                &format!("{path}.err"),
                index,
                depth + 1,
            ))),
        },
        TypeRef::Named(name) => match index.record_fields(name) {
            Some((resolved, fields)) => Sym::Record {
                type_name: resolved,
                fields: fields
                    .iter()
                    .map(|(fname, fty)| {
                        (
                            fname.clone(),
                            fresh_sym(
                                fty,
                                &format!("{path}.{fname}"),
                                index,
                                depth + 1,
                            ),
                        )
                    })
                    .collect(),
            },
            None => Sym::Opaque,
        },
        // Float, Unit, tuples, sets/maps, function types,
        // generics, references, streams: no representable
        // sort in this encoder.
        _ => Sym::Opaque,
    }
}

/// The D5a range side-condition for one Char term: a
/// Unicode scalar value — 0 ≤ c ≤ 0x10FFFF, excluding the
/// surrogate gap U+D800..=U+DFFF.
#[cfg(feature = "z3-support")]
fn char_range_condition(c: &z3::ast::Int) -> z3::ast::Bool {
    let zero = z3::ast::Int::from_i64(0);
    let max = z3::ast::Int::from_i64(0x10FFFF);
    let gap_lo = z3::ast::Int::from_i64(0xD800);
    let gap_hi = z3::ast::Int::from_i64(0xDFFF);
    let in_range = z3::ast::Bool::and(&[&c.ge(&zero), &c.le(&max)]);
    let in_gap = z3::ast::Bool::and(&[&c.ge(&gap_lo), &c.le(&gap_hi)]);
    z3::ast::Bool::and(&[&in_range, &in_gap.not()])
}

/// Collect the range side-conditions owed by every Char
/// term inside a bound parameter value (Char fields of
/// record parameters and Char payloads of Option/Result
/// parameters included — they are Char parameters in every
/// sense that matters to scalar reasoning).
#[cfg(feature = "z3-support")]
fn collect_char_conditions(sym: &Sym, out: &mut Vec<z3::ast::Bool>) {
    match sym {
        Sym::Char(c) => out.push(char_range_condition(c)),
        Sym::List(items) => {
            for item in items {
                collect_char_conditions(item, out);
            }
        }
        Sym::Record { fields, .. } => {
            for (_, v) in fields {
                collect_char_conditions(v, out);
            }
        }
        Sym::Option { payload, .. } => {
            if let Some(p) = payload {
                collect_char_conditions(p, out);
            }
        }
        Sym::Result {
            ok_payload,
            err_payload,
            ..
        } => {
            if let Some(p) = ok_payload {
                collect_char_conditions(p, out);
            }
            if let Some(p) = err_payload {
                collect_char_conditions(p, out);
            }
        }
        _ => {}
    }
}

#[cfg(feature = "z3-support")]
fn sym_eval(node: &IRNode, env: &mut SymEnv, ctx: &EvalCtx) -> Option<Sym> {
    Some(sym_eval_flow(node, env, ctx)?.into_value())
}

#[cfg(feature = "z3-support")]
fn sym_eval_flow(node: &IRNode, env: &mut SymEnv, ctx: &EvalCtx) -> Option<SymFlow> {
    match node {
        IRNode::Literal(lit) => Some(SymFlow::Normal(sym_literal(lit)?)),

        IRNode::Var(name) => env.get(name).cloned().map(SymFlow::Normal),

        IRNode::BinOp(op, l, r) => {
            let lv = sym_eval(l, env, ctx)?;
            let rv = sym_eval(r, env, ctx)?;
            Some(SymFlow::Normal(sym_binop_ir(*op, lv, rv)?))
        }

        IRNode::UnaryOp(op, inner) => {
            let v = sym_eval(inner, env, ctx)?;
            Some(SymFlow::Normal(sym_unary_ir(*op, v)?))
        }

        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => match sym_eval(condition, env, ctx)? {
            Sym::Bool(cond) => {
                let t = sym_eval(then_branch, env, ctx)?;
                let e = sym_eval(else_branch, env, ctx)?;
                Some(SymFlow::Normal(sym_ite(&cond, t, e)?))
            }
            _ => None,
        },

        IRNode::Let { name, value, body } => {
            let v = match sym_eval_flow(value, env, ctx)? {
                SymFlow::Returned(v) => return Some(SymFlow::Returned(v)),
                SymFlow::Normal(v) => v,
            };
            let mut inner = env.clone();
            inner.insert(name.clone(), v);
            sym_eval_flow(body, &mut inner, ctx)
        }

        IRNode::Block(stmts) => {
            let mut last = Sym::Opaque; // an empty block is Unit
            for stmt in stmts {
                match sym_eval_flow(stmt, env, ctx)? {
                    SymFlow::Returned(v) => return Some(SymFlow::Returned(v)),
                    SymFlow::Normal(v) => last = v,
                }
            }
            Some(SymFlow::Normal(last))
        }

        IRNode::Return(inner) => {
            let v = sym_eval(inner, env, ctx)?;
            Some(SymFlow::Returned(v))
        }

        IRNode::Match { scrutinee, cases } => {
            sym_match(scrutinee, cases, env, ctx).map(SymFlow::Normal)
        }

        IRNode::Call { function, args } => {
            sym_call(function, args, env, ctx).map(SymFlow::Normal)
        }

        IRNode::Collection { elements, .. } => {
            // The interpreter maps every collection literal to
            // a List value regardless of kind; mirror that.
            let mut values = Vec::with_capacity(elements.len());
            for el in elements {
                match sym_eval_flow(el, env, ctx)? {
                    SymFlow::Returned(v) => return Some(SymFlow::Returned(v)),
                    SymFlow::Normal(v) => values.push(v),
                }
            }
            Some(SymFlow::Normal(Sym::List(values)))
        }

        IRNode::Index(coll, idx) => {
            let coll = sym_eval(coll, env, ctx)?;
            let idx = sym_eval(idx, env, ctx)?;
            Some(SymFlow::Normal(sym_index(coll, idx)?))
        }

        IRNode::FieldAccess { base, field } => {
            // Projection by name (Phase 76, D2a): the base
            // must be a record value and the field must be
            // one of its fields — an unknown field or a
            // non-record base is unencodable, never a guess.
            match sym_eval(base, env, ctx)? {
                Sym::Record { fields, .. } => fields
                    .into_iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, v)| SymFlow::Normal(v)),
                _ => None,
            }
        }

        IRNode::RecordLit { type_name, fields } => {
            // Construction (Phase 76, D3a's IR half): fields
            // evaluate in written order; a `Return` inside a
            // field value unwinds the whole evaluation,
            // exactly as inside a Collection. The stored type
            // name is the RESOLVED declaration name when the
            // index carries the module's declarations, so a
            // constructed record compares equal to a
            // parameter-bound one even when the literal was
            // written through an alias.
            let mut values = Vec::with_capacity(fields.len());
            for (name, value) in fields {
                match sym_eval_flow(value, env, ctx)? {
                    SymFlow::Returned(v) => return Some(SymFlow::Returned(v)),
                    SymFlow::Normal(v) => values.push((name.clone(), v)),
                }
            }
            let type_name = match ctx.index.record_fields(type_name) {
                Some((resolved, _)) => resolved,
                None => type_name.clone(),
            };
            Some(SymFlow::Normal(Sym::Record {
                type_name,
                fields: values,
            }))
        }

        IRNode::Typed { node, .. } => sym_eval_flow(node, env, ctx),

        // Everything below is outside the encodable subset,
        // exactly as in the interpreter: unfilled holes, loops,
        // allocation/mutation, function values, higher-order
        // combinators, struct definitions, Result propagation.
        IRNode::Hole { .. }
        | IRNode::While { .. }
        | IRNode::For { .. }
        | IRNode::Loop { .. }
        | IRNode::Break { .. }
        | IRNode::Alloc { .. }
        | IRNode::Assign { .. }
        | IRNode::Lambda { .. }
        | IRNode::Closure { .. }
        | IRNode::Map { .. }
        | IRNode::Filter { .. }
        | IRNode::Fold { .. }
        | IRNode::Sort { .. }
        | IRNode::StructDef { .. }
        | IRNode::TryPropagate(_) => None,
    }
}

#[cfg(feature = "z3-support")]
fn sym_literal(lit: &IRLiteral) -> Option<Sym> {
    match lit {
        IRLiteral::Int(n) => Some(Sym::Int(z3::ast::Int::from_i64(*n))),
        IRLiteral::Bool(b) => Some(Sym::Bool(z3::ast::Bool::from_bool(*b))),
        IRLiteral::String(s) => {
            Some(Sym::Str(z3::ast::String::from_str(s).ok()?))
        }
        // Float, Unit: no honest Z3 sort in this encoder.
        // Char (Phase 76, D5a): its Unicode scalar value.
        IRLiteral::Float(_) | IRLiteral::Unit => Some(Sym::Opaque),
        IRLiteral::Char(c) => {
            Some(Sym::Char(z3::ast::Int::from_i64(*c as u32 as i64)))
        }
    }
}

/// Symbolic match, folded into nested `ite`s from the last arm
/// backwards. Arms are tried in order, guard patterns are
/// undecidable. The last reachable arm must be unconditional
/// (wildcard/variable) — otherwise some scrutinee values match
/// nothing and the body has no defined output, which is
/// unencodable rather than a guess.
///
/// Constructor patterns (Phase 91, W2) encode against the
/// Phase 76 tag+payload model: `Some`/`None` over an Option
/// scrutinee, `Ok`/`Err` over a Result scrutinee — the arm's
/// condition is the tag side it discriminates, and a
/// Variable/Wildcard sub-pattern binds the payload. A
/// constructor match with no unconditional arm is still
/// encodable when live arms jointly cover both tag sides:
/// the folded value's fallback is then unreachable by
/// construction. Anything further declines — a constructor
/// of the other tagged form, a nested sub-pattern, a
/// payload pattern against a slot that carries none.
#[cfg(feature = "z3-support")]
fn sym_match(
    scrutinee: &IRNode,
    cases: &[(IRPattern, IRNode)],
    env: &mut SymEnv,
    ctx: &EvalCtx,
) -> Option<Sym> {
    let scrut = sym_eval(scrutinee, env, ctx)?;
    // (condition, value) per arm; condition None = unconditional.
    let mut arms: Vec<(Option<z3::ast::Bool>, Sym)> = Vec::new();
    // Tag-side coverage from constructor arms (Phase 91):
    // a free-tag scrutinee is exhausted once live arms
    // discriminate both of its sides.
    let mut covers_pos = false;
    let mut covers_neg = false;
    for (pattern, body) in cases {
        match pattern {
            IRPattern::Wildcard => {
                arms.push((None, sym_eval(body, env, ctx)?));
                break; // later arms are unreachable
            }
            IRPattern::Variable(name) => {
                let mut inner = env.clone();
                inner.insert(name.clone(), scrut.clone());
                arms.push((None, sym_eval(body, &mut inner, ctx)?));
                break;
            }
            IRPattern::Literal(lit) => {
                match sym_pattern_cond(&scrut, lit) {
                    PatternCond::Eq(cond) => {
                        arms.push((Some(cond), sym_eval(body, env, ctx)?));
                    }
                    // Provably never equal (interpreter equality
                    // is sort-strict here): the arm is dead.
                    PatternCond::Never => continue,
                    PatternCond::Undecidable => return None,
                }
            }
            IRPattern::Constructor(name, pargs) => {
                match sym_constructor_arm(&scrut, name) {
                    // The scrutinee never carries this
                    // constructor: the arm is dead, exactly
                    // like a Never literal.
                    CtorArm::Dead => continue,
                    CtorArm::Unsupported => return None,
                    CtorArm::Live {
                        cond,
                        payload,
                        side,
                    } => {
                        let mut inner = env.clone();
                        match (pargs.len(), &payload) {
                            (0, _) => {}
                            (1, Some(p)) => match &pargs[0] {
                                IRPattern::Variable(n) => {
                                    inner.insert(n.clone(), p.clone());
                                }
                                IRPattern::Wildcard => {}
                                // A nested sub-pattern is
                                // outside the tag+payload
                                // model: decline.
                                _ => return None,
                            },
                            // A payload pattern against a
                            // slot that carries none (or an
                            // arity the constructors do not
                            // have): decline.
                            _ => return None,
                        }
                        let value = sym_eval(body, &mut inner, ctx)?;
                        match side {
                            TagSide::Pos => covers_pos = true,
                            TagSide::Neg => covers_neg = true,
                        }
                        let unconditional = cond.is_none();
                        arms.push((cond, value));
                        if unconditional {
                            break; // later arms are unreachable
                        }
                    }
                }
            }
            IRPattern::Guard(..) => return None,
        }
    }
    let (last_cond, last_val) = arms.pop()?;
    if last_cond.is_some() && !(covers_pos && covers_neg) {
        return None; // the match can fall through
    }
    let mut acc = last_val;
    for (cond, val) in arms.into_iter().rev() {
        let cond = cond.expect("non-final arms are conditional");
        acc = sym_ite(&cond, val, acc)?;
    }
    Some(acc)
}

/// Which side of a tagged scrutinee a constructor
/// discriminates (Phase 91): the payload side (`Some`/`Ok`)
/// or the empty side (`None`/`Err`).
#[cfg(feature = "z3-support")]
#[derive(Clone, Copy)]
enum TagSide {
    Pos,
    Neg,
}

/// A constructor pattern read against a scrutinee value
/// (Phase 91, W2).
#[cfg(feature = "z3-support")]
enum CtorArm {
    /// The scrutinee's tag is literally the other
    /// constructor: this arm can never match.
    Dead,
    /// The arm discriminates this side of the tag: `cond`
    /// is the tag condition (None when the tag is
    /// literally this constructor, making the arm
    /// unconditional), `payload` is the value the
    /// constructor carries (None for the nullary
    /// constructor), and `side` is the tag side the arm
    /// covers toward exhaustiveness.
    Live {
        cond: Option<z3::ast::Bool>,
        payload: Option<Sym>,
        side: TagSide,
    },
    /// A constructor/scrutinee pairing the Sym model does
    /// not represent (the other tagged form's constructor,
    /// an unknown constructor, a non-tagged scrutinee):
    /// unencodable.
    Unsupported,
}

#[cfg(feature = "z3-support")]
fn sym_constructor_arm(scrut: &Sym, name: &str) -> CtorArm {
    // (tag, payload slot, side) for the constructor over
    // this scrutinee form. The payload slot follows the
    // Sym invariants: it is present whenever the tag can
    // be this constructor, absent only under the opposing
    // literal tag — exactly the Dead case below.
    let (tag, payload, side) = match (scrut, name) {
        (Sym::Option { some, payload }, "Some") => {
            (some, payload.as_deref().cloned(), TagSide::Pos)
        }
        (Sym::Option { some, .. }, "None") => (some, None, TagSide::Neg),
        (Sym::Result { ok, ok_payload, .. }, "Ok") => {
            (ok, ok_payload.as_deref().cloned(), TagSide::Pos)
        }
        (Sym::Result { ok, err_payload, .. }, "Err") => {
            (ok, err_payload.as_deref().cloned(), TagSide::Neg)
        }
        _ => return CtorArm::Unsupported,
    };
    match (tag, side) {
        // The tag is literally this constructor: the arm
        // matches unconditionally.
        (Tag::True, TagSide::Pos) | (Tag::False, TagSide::Neg) => CtorArm::Live {
            cond: None,
            payload,
            side,
        },
        // The tag is literally the other constructor:
        // dead arm.
        (Tag::True, TagSide::Neg) | (Tag::False, TagSide::Pos) => CtorArm::Dead,
        // A free tag: the arm matches under the tag term
        // (negated on the empty side).
        (Tag::Free(t), TagSide::Pos) => CtorArm::Live {
            cond: Some(t.clone()),
            payload,
            side,
        },
        (Tag::Free(t), TagSide::Neg) => CtorArm::Live {
            cond: Some(t.not()),
            payload,
            side,
        },
    }
}

#[cfg(feature = "z3-support")]
enum PatternCond {
    Eq(z3::ast::Bool),
    Never,
    Undecidable,
}

/// The condition "scrutinee equals this literal pattern", in
/// the interpreter's equality semantics: same-sort equality,
/// plus Int/Float numeric promotion. Provably disjoint sorts
/// never match; anything involving an unrepresentable value
/// is undecidable.
#[cfg(feature = "z3-support")]
fn sym_pattern_cond(scrut: &Sym, lit: &IRLiteral) -> PatternCond {
    match (scrut, lit) {
        (Sym::Int(a), IRLiteral::Int(n)) => {
            PatternCond::Eq(a.eq(&z3::ast::Int::from_i64(*n)))
        }
        (Sym::Int(a), IRLiteral::Float(f)) => {
            // Interpreter promotion: Int(i) == Float(f) iff
            // i as f64 == f. Only an integral f in i64 range
            // can ever be equal.
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                PatternCond::Eq(a.eq(&z3::ast::Int::from_i64(*f as i64)))
            } else {
                PatternCond::Never
            }
        }
        (Sym::Bool(a), IRLiteral::Bool(b)) => {
            PatternCond::Eq(a.eq(&z3::ast::Bool::from_bool(*b)))
        }
        (Sym::Str(a), IRLiteral::String(s)) => {
            match z3::ast::String::from_str(s) {
                Ok(lit) => PatternCond::Eq(a.eq(&lit)),
                Err(_) => PatternCond::Undecidable,
            }
        }
        // A Char scrutinee against a Char literal pattern:
        // scalar equality (Phase 76). An indexed char
        // (CharStr) compares as its one-char string.
        (Sym::Char(a), IRLiteral::Char(c)) => {
            PatternCond::Eq(a.eq(&z3::ast::Int::from_i64(*c as u32 as i64)))
        }
        (Sym::CharStr(a), IRLiteral::Char(c)) => {
            match z3::ast::String::from_str(&c.to_string()) {
                Ok(lit) => PatternCond::Eq(a.eq(&lit)),
                Err(_) => PatternCond::Undecidable,
            }
        }
        // A list scrutinee has no literal patterns; an Opaque
        // scrutinee cannot be compared at all.
        (Sym::List(_), _) | (Sym::Opaque, _) => PatternCond::Undecidable,
        // Remaining pairs are provably disjoint sorts —
        // records, tagged values and list-lengths never equal
        // a literal pattern, and a Char never equals an Int,
        // Float, Bool or String literal.
        _ => PatternCond::Never,
    }
}

#[cfg(feature = "z3-support")]
fn sym_call(function: &str, args: &[IRNode], env: &mut SymEnv, ctx: &EvalCtx) -> Option<Sym> {
    match function {
        // `List[...]` is the spec language's list literal; the
        // parser lowers it to a Call named "List" (parser.rs).
        "List" => {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(sym_eval(arg, env, ctx)?);
            }
            Some(Sym::List(values))
        }
        // The one pure builtin the specs and templates rely on.
        "len" if args.len() == 1 => sym_len(sym_eval(&args[0], env, ctx)?),
        // The Result/Option constructors (Phase 76): they
        // build the tag+payload forms. Exact arity, as in
        // the interpreter's eval_call — a wrong-arity
        // constructor call is unencodable, not inlined.
        "Ok" if args.len() == 1 => Some(Sym::Result {
            ok: Tag::True,
            ok_payload: Some(Box::new(sym_eval(&args[0], env, ctx)?)),
            err_payload: None,
        }),
        "Err" if args.len() == 1 => Some(Sym::Result {
            ok: Tag::False,
            ok_payload: None,
            err_payload: Some(Box::new(sym_eval(&args[0], env, ctx)?)),
        }),
        "Some" if args.len() == 1 => Some(Sym::Option {
            some: Tag::True,
            payload: Some(Box::new(sym_eval(&args[0], env, ctx)?)),
        }),
        "None" if args.is_empty() => Some(Sym::Option {
            some: Tag::False,
            payload: None,
        }),
        // A call to another spec resolves by inlining the
        // callee's body (see sym_invoke); template-internal
        // pseudo-calls and unknown names resolve to nothing
        // and stay unencodable.
        _ => {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(sym_eval(arg, env, ctx)?);
            }
            sym_invoke(ctx, function, values)
        }
    }
}

#[cfg(feature = "z3-support")]
fn sym_len(v: Sym) -> Option<Sym> {
    match v {
        Sym::List(items) => Some(Sym::Int(z3::ast::Int::from_i64(items.len() as i64))),
        Sym::Str(s) => Some(Sym::Int(s.length())),
        Sym::ListLen(n) => Some(Sym::Int(n)),
        _ => None,
    }
}

/// Whether a Str term IS a string literal, and its value.
/// `as_string()` alone does not answer this: measured on
/// z3 0.20, it returns `Some("")` for a free constant and
/// for a concatenation — only a genuine literal returns
/// its own value. Z3 hash-conses literals, so a literal
/// compares equal (AST identity) to a fresh build of its
/// value, and nothing else does; that equality is the
/// exact test. This is the fact the pure mirror tracks as
/// `StrVal::Known`.
#[cfg(feature = "z3-support")]
fn str_literal(s: &z3::ast::String) -> Option<String> {
    let value = s.as_string()?;
    let rebuilt = z3::ast::String::from_str(&value).ok()?;
    if *s == rebuilt {
        Some(value)
    } else {
        None
    }
}

#[cfg(feature = "z3-support")]
fn sym_index(coll: Sym, idx: Sym) -> Option<Sym> {
    match (coll, idx) {
        (Sym::List(items), Sym::Int(i)) => {
            // Only a constant index picks an element; a
            // symbolic index would need array theory. An
            // out-of-bounds constant mirrors the interpreter's
            // evaluation failure: unencodable, not a guess.
            let n = i.as_i64()?;
            if n < 0 {
                return None;
            }
            items.into_iter().nth(n as usize)
        }
        (Sym::Str(s), Sym::Int(i)) => {
            // The String-index→Char bridge (Phase 76).
            // CONSTANT index into a LITERAL string: exact —
            // the character's scalar value, and an
            // out-of-bounds index is unencodable, mirroring
            // the interpreter's evaluation failure (no
            // totalization where the value is known).
            if let (Some(lit), Some(n)) = (str_literal(&s), i.as_i64()) {
                if n < 0 {
                    return None;
                }
                return lit
                    .chars()
                    .nth(n as usize)
                    .map(|c| Sym::Char(z3::ast::Int::from_i64(c as u32 as i64)));
            }
            // The general case: `str.at` — the character as
            // a length-≤1 string (Sym::CharStr; see its doc
            // for the sound fragment and the named boundary).
            Some(Sym::CharStr(s.at(i)))
        }
        _ => None,
    }
}

/// Merge an Option/Result payload slot across an `ite`:
/// a slot present on either side is observed only under
/// the tag that side carries, so a one-sided payload is
/// the merged payload; two present payloads merge by
/// `sym_ite` (and can fail, failing the whole merge).
#[cfg(feature = "z3-support")]
fn sym_ite_payload(
    cond: &z3::ast::Bool,
    a: Option<Box<Sym>>,
    b: Option<Box<Sym>>,
) -> Option<Option<Box<Sym>>> {
    match (a, b) {
        (None, None) => Some(None),
        (Some(x), None) => Some(Some(x)),
        (None, Some(y)) => Some(Some(y)),
        (Some(x), Some(y)) => sym_ite(cond, *x, *y).map(|v| Some(Box::new(v))),
    }
}

#[cfg(feature = "z3-support")]
fn sym_ite(cond: &z3::ast::Bool, a: Sym, b: Sym) -> Option<Sym> {
    match (a, b) {
        (Sym::Int(x), Sym::Int(y)) => Some(Sym::Int(cond.ite(&x, &y))),
        (Sym::Bool(x), Sym::Bool(y)) => Some(Sym::Bool(cond.ite(&x, &y))),
        (Sym::Str(x), Sym::Str(y)) => Some(Sym::Str(cond.ite(&x, &y))),
        (Sym::Char(x), Sym::Char(y)) => Some(Sym::Char(cond.ite(&x, &y))),
        (Sym::CharStr(x), Sym::CharStr(y)) => Some(Sym::CharStr(cond.ite(&x, &y))),
        (Sym::ListLen(x), Sym::ListLen(y)) => Some(Sym::ListLen(cond.ite(&x, &y))),
        (Sym::List(x), Sym::List(y)) => {
            // Two concrete lists merge elementwise (Phase
            // 91, W4; the Phase 90 probe's p90_ite arm,
            // promoted). The merged list has a definite
            // length only when both sides agree on it —
            // unequal lengths decline rather than invent
            // one.
            if x.len() != y.len() {
                return None;
            }
            let mut out = Vec::with_capacity(x.len());
            for (u, v) in x.into_iter().zip(y) {
                out.push(sym_ite(cond, u, v)?);
            }
            Some(Sym::List(out))
        }
        (Sym::Opaque, Sym::Opaque) => Some(Sym::Opaque),
        (
            Sym::Record {
                type_name: ta,
                fields: fa,
            },
            Sym::Record {
                type_name: tb,
                fields: fb,
            },
        ) => {
            // Same declaration, same field set: merge
            // fieldwise (in the left record's field order).
            if ta != tb || fa.len() != fb.len() {
                return None;
            }
            let mut merged = Vec::with_capacity(fa.len());
            for (name, va) in fa {
                let vb = fb
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, v)| v.clone())?;
                merged.push((name, sym_ite(cond, va, vb)?));
            }
            Some(Sym::Record {
                type_name: ta,
                fields: merged,
            })
        }
        (
            Sym::Option {
                some: ta,
                payload: pa,
            },
            Sym::Option {
                some: tb,
                payload: pb,
            },
        ) => Some(Sym::Option {
            some: Tag::merge(cond, &ta, &tb),
            payload: sym_ite_payload(cond, pa, pb)?,
        }),
        (
            Sym::Result {
                ok: ta,
                ok_payload: oa,
                err_payload: ea,
            },
            Sym::Result {
                ok: tb,
                ok_payload: ob,
                err_payload: eb,
            },
        ) => Some(Sym::Result {
            ok: Tag::merge(cond, &ta, &tb),
            ok_payload: sym_ite_payload(cond, oa, ob)?,
            err_payload: sym_ite_payload(cond, ea, eb)?,
        }),
        _ => None,
    }
}

// ── Shared symbolic operators ──────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "z3-support")]
enum BinKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Eq,
    Neq,
    Lt,
    Le,
    Gt,
    Ge,
    Concat,
    Append,
}

/// One operator table for both IR bodies and spec expressions:
/// the two syntaxes denote the same operations. Sort-strict,
/// like the interpreter: operands of the wrong sort (or an
/// Opaque operand) make the whole expression unencodable.
/// Z3 totalizes Div/Mod where the interpreter fails (division
/// by zero, overflow): proofs about bodies using them are
/// proofs about Z3's totalized semantics — see ROAD notes.
#[cfg(feature = "z3-support")]
fn sym_binop(kind: BinKind, l: Sym, r: Sym) -> Option<Sym> {
    use BinKind::*;
    match (kind, l, r) {
        (Add, Sym::Int(a), Sym::Int(b)) => Some(Sym::Int(z3_add(&a, &b))),
        (Sub, Sym::Int(a), Sym::Int(b)) => Some(Sym::Int(z3_sub(&a, &b))),
        (Mul, Sym::Int(a), Sym::Int(b)) => Some(Sym::Int(z3_mul(&a, &b))),
        (Div, Sym::Int(a), Sym::Int(b)) => Some(Sym::Int(a.div(&b))),
        // The interpreter's Mod is Rust `%` (remainder, sign of
        // the dividend) = Z3's rem, not Z3's mod.
        (Mod, Sym::Int(a), Sym::Int(b)) => Some(Sym::Int(a.rem(&b))),
        (And, Sym::Bool(a), Sym::Bool(b)) => {
            Some(Sym::Bool(z3::ast::Bool::and(&[&a, &b])))
        }
        (Or, Sym::Bool(a), Sym::Bool(b)) => {
            Some(Sym::Bool(z3::ast::Bool::or(&[&a, &b])))
        }
        (Xor, Sym::Bool(a), Sym::Bool(b)) => Some(Sym::Bool(a.xor(&b))),
        (Eq, Sym::Int(a), Sym::Int(b)) => Some(Sym::Bool(a.eq(&b))),
        (Eq, Sym::Bool(a), Sym::Bool(b)) => Some(Sym::Bool(a.eq(&b))),
        (Eq, Sym::Str(a), Sym::Str(b)) => Some(Sym::Bool(a.eq(&b))),
        (Neq, Sym::Int(a), Sym::Int(b)) => Some(Sym::Bool(a.eq(&b).not())),
        (Neq, Sym::Bool(a), Sym::Bool(b)) => Some(Sym::Bool(a.eq(&b).not())),
        (Neq, Sym::Str(a), Sym::Str(b)) => Some(Sym::Bool(a.eq(&b).not())),
        (Lt, Sym::Int(a), Sym::Int(b)) => Some(Sym::Bool(a.lt(&b))),
        (Le, Sym::Int(a), Sym::Int(b)) => Some(Sym::Bool(a.le(&b))),
        (Gt, Sym::Int(a), Sym::Int(b)) => Some(Sym::Bool(a.gt(&b))),
        (Ge, Sym::Int(a), Sym::Int(b)) => Some(Sym::Bool(a.ge(&b))),
        (Lt, Sym::Str(a), Sym::Str(b)) => Some(Sym::Bool(a.str_lt(&b))),
        (Le, Sym::Str(a), Sym::Str(b)) => Some(Sym::Bool(a.str_le(&b))),
        (Gt, Sym::Str(a), Sym::Str(b)) => Some(Sym::Bool(a.str_gt(&b))),
        (Ge, Sym::Str(a), Sym::Str(b)) => Some(Sym::Bool(a.str_ge(&b))),
        (Concat, Sym::Str(a), Sym::Str(b)) => {
            Some(Sym::Str(z3::ast::String::concat(&[&a, &b])))
        }
        (Concat, Sym::List(mut a), Sym::List(b)) => {
            a.extend(b);
            Some(Sym::List(a))
        }
        (Append, Sym::List(mut a), item) => {
            a.push(item);
            Some(Sym::List(a))
        }
        // Append over a length-only list (Phase 91, W5):
        // the elements are unmodelled, but the length is
        // exact — appending one element grows it by one,
        // whatever the element is.
        (Append, Sym::ListLen(n), _) => {
            Some(Sym::ListLen(z3_add(&n, &z3::ast::Int::from_i64(1))))
        }
        // Append of an indexed char onto a string (Phase
        // 91, W5): the lexeme shape. The CharStr form is
        // already the character as a length-≤1 string, so
        // concatenation is the append — including the
        // bridge's out-of-bounds totalization (the empty
        // string appends nothing, as an out-of-bounds
        // index denotes no character).
        (Append, Sym::Str(a), Sym::CharStr(b)) => {
            Some(Sym::Str(z3::ast::String::concat(&[&a, &b])))
        }
        // Char comparisons (Phase 76, D5a): scalar order on
        // the Int sort. Arithmetic on Chars does not exist
        // in the interpreter and stays unencodable.
        (Eq, Sym::Char(a), Sym::Char(b)) => Some(Sym::Bool(a.eq(&b))),
        (Neq, Sym::Char(a), Sym::Char(b)) => Some(Sym::Bool(a.eq(&b).not())),
        (Lt, Sym::Char(a), Sym::Char(b)) => Some(Sym::Bool(a.lt(&b))),
        (Le, Sym::Char(a), Sym::Char(b)) => Some(Sym::Bool(a.le(&b))),
        (Gt, Sym::Char(a), Sym::Char(b)) => Some(Sym::Bool(a.gt(&b))),
        (Ge, Sym::Char(a), Sym::Char(b)) => Some(Sym::Bool(a.ge(&b))),
        // Indexed chars (the bridge's CharStr form): string
        // equality/order over length-≤1 strings IS scalar
        // equality/order (see Sym::CharStr). Against a
        // CONSTANT Char the scalar converts to its one-char
        // string; against a symbolic Char there is no
        // conversion in the safe API — unencodable, the
        // bridge's named boundary.
        (Eq, Sym::CharStr(a), Sym::CharStr(b)) => Some(Sym::Bool(a.eq(&b))),
        (Neq, Sym::CharStr(a), Sym::CharStr(b)) => {
            Some(Sym::Bool(a.eq(&b).not()))
        }
        (Lt, Sym::CharStr(a), Sym::CharStr(b)) => Some(Sym::Bool(a.str_lt(&b))),
        (Le, Sym::CharStr(a), Sym::CharStr(b)) => Some(Sym::Bool(a.str_le(&b))),
        (Gt, Sym::CharStr(a), Sym::CharStr(b)) => Some(Sym::Bool(a.str_gt(&b))),
        (Ge, Sym::CharStr(a), Sym::CharStr(b)) => Some(Sym::Bool(a.str_ge(&b))),
        (Eq, Sym::CharStr(a), Sym::Char(b)) => {
            Some(Sym::Bool(a.eq(&char_const_string(&b)?)))
        }
        (Neq, Sym::CharStr(a), Sym::Char(b)) => {
            Some(Sym::Bool(a.eq(&char_const_string(&b)?).not()))
        }
        (Lt, Sym::CharStr(a), Sym::Char(b)) => {
            Some(Sym::Bool(a.str_lt(&char_const_string(&b)?)))
        }
        (Le, Sym::CharStr(a), Sym::Char(b)) => {
            Some(Sym::Bool(a.str_le(&char_const_string(&b)?)))
        }
        (Gt, Sym::CharStr(a), Sym::Char(b)) => {
            Some(Sym::Bool(a.str_gt(&char_const_string(&b)?)))
        }
        (Ge, Sym::CharStr(a), Sym::Char(b)) => {
            Some(Sym::Bool(a.str_ge(&char_const_string(&b)?)))
        }
        (Eq, Sym::Char(a), Sym::CharStr(b)) => {
            Some(Sym::Bool(char_const_string(&a)?.eq(&b)))
        }
        (Neq, Sym::Char(a), Sym::CharStr(b)) => {
            Some(Sym::Bool(char_const_string(&a)?.eq(&b).not()))
        }
        (Lt, Sym::Char(a), Sym::CharStr(b)) => {
            Some(Sym::Bool(char_const_string(&a)?.str_lt(&b)))
        }
        (Le, Sym::Char(a), Sym::CharStr(b)) => {
            Some(Sym::Bool(char_const_string(&a)?.str_le(&b)))
        }
        (Gt, Sym::Char(a), Sym::CharStr(b)) => {
            Some(Sym::Bool(char_const_string(&a)?.str_gt(&b)))
        }
        (Ge, Sym::Char(a), Sym::CharStr(b)) => {
            Some(Sym::Bool(char_const_string(&a)?.str_ge(&b)))
        }
        // Record equality (Phase 76): fieldwise, by name —
        // the interpreter's structural equality over
        // Value::Record, with the type name part of it.
        (
            Eq,
            Sym::Record {
                type_name: an,
                fields: af,
            },
            Sym::Record {
                type_name: bn,
                fields: bf,
            },
        ) => sym_record_eq(&an, &af, &bn, &bf).map(Sym::Bool),
        (
            Neq,
            Sym::Record {
                type_name: an,
                fields: af,
            },
            Sym::Record {
                type_name: bn,
                fields: bf,
            },
        ) => sym_record_eq(&an, &af, &bn, &bf).map(|b| Sym::Bool(b.not())),
        // Option / Result equality (Phase 76): tag logic
        // plus the observed payload — see the helpers.
        (
            Eq,
            Sym::Option {
                some: at,
                payload: ap,
            },
            Sym::Option {
                some: bt,
                payload: bp,
            },
        ) => sym_option_eq(&at, &ap, &bt, &bp).map(Sym::Bool),
        (
            Neq,
            Sym::Option {
                some: at,
                payload: ap,
            },
            Sym::Option {
                some: bt,
                payload: bp,
            },
        ) => sym_option_eq(&at, &ap, &bt, &bp).map(|b| Sym::Bool(b.not())),
        (
            Eq,
            Sym::Result {
                ok: at,
                ok_payload: ao,
                err_payload: ae,
            },
            Sym::Result {
                ok: bt,
                ok_payload: bo,
                err_payload: be,
            },
        ) => sym_result_eq(&at, &ao, &ae, &bt, &bo, &be).map(Sym::Bool),
        (
            Neq,
            Sym::Result {
                ok: at,
                ok_payload: ao,
                err_payload: ae,
            },
            Sym::Result {
                ok: bt,
                ok_payload: bo,
                err_payload: be,
            },
        ) => sym_result_eq(&at, &ao, &ae, &bt, &bo, &be)
            .map(|b| Sym::Bool(b.not())),
        _ => None,
    }
}

/// The one-char string of a CONSTANT Char term, for the
/// bridge's cross-representation comparisons. A symbolic
/// Char term has no readable numeral (`as_i64` fails) and
/// no string form in the safe API — None, which the caller
/// turns into "unencodable" (the named boundary).
#[cfg(feature = "z3-support")]
fn char_const_string(c: &z3::ast::Int) -> Option<z3::ast::String> {
    let n = c.as_i64()?;
    let ch = char::from_u32(u32::try_from(n).ok()?)?;
    z3::ast::String::from_str(&ch.to_string()).ok()
}

/// Equality of two symbolic values as a Bool term — the
/// `Eq` row of `sym_binop`, unwrapped.
#[cfg(feature = "z3-support")]
fn sym_eq(l: Sym, r: Sym) -> Option<z3::ast::Bool> {
    match sym_binop(BinKind::Eq, l, r)? {
        Sym::Bool(b) => Some(b),
        _ => None,
    }
}

/// Record equality: records of different declared types
/// are provably unequal (the interpreter's structural
/// equality includes the type name — `Some(false)`, not
/// unencodable); records of one type are equal iff every
/// field is equal, compared BY NAME (written field order
/// is not part of a value's identity here — the declared
/// field set is). A field whose equality cannot encode
/// makes the whole equality unencodable.
#[cfg(feature = "z3-support")]
fn sym_record_eq(
    a_name: &str,
    a_fields: &[(String, Sym)],
    b_name: &str,
    b_fields: &[(String, Sym)],
) -> Option<z3::ast::Bool> {
    if a_name != b_name {
        return Some(z3::ast::Bool::from_bool(false));
    }
    if a_fields.len() != b_fields.len() {
        return None; // one declared type, two field sets: malformed
    }
    let mut acc = z3::ast::Bool::from_bool(true);
    for (name, va) in a_fields {
        let vb = b_fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)?;
        acc = z3::ast::Bool::and(&[&acc, &sym_eq(va.clone(), vb.clone())?]);
    }
    Some(acc)
}

/// Option equality. When either side is a literal `None`
/// (tag `Tag::False`), equality is exactly tag equality —
/// no payload term is needed, which is what lets
/// `output == None()` judge a body whose payload sort has
/// no equality. Otherwise both sides can be `Some`, both
/// payloads are present (the Sym::Option invariant), and
/// equality is tag equality plus payload equality under
/// the tag.
#[cfg(feature = "z3-support")]
fn sym_option_eq(
    a_some: &Tag,
    a_pay: &Option<Box<Sym>>,
    b_some: &Tag,
    b_pay: &Option<Box<Sym>>,
) -> Option<z3::ast::Bool> {
    if *a_some == Tag::False || *b_some == Tag::False {
        return Some(a_some.term().eq(&b_some.term()));
    }
    let pa = a_pay.as_ref()?;
    let pb = b_pay.as_ref()?;
    let pe = sym_eq((**pa).clone(), (**pb).clone())?;
    let tags = a_some.term().eq(&b_some.term());
    let under_tag = z3::ast::Bool::or(&[&a_some.term().not(), &pe]);
    Some(z3::ast::Bool::and(&[&tags, &under_tag]))
}

/// Result equality, by tag case analysis: literal tags
/// decide which payload slot (if any) is observed; two
/// free tags need both slots' equalities. A payload slot
/// that is absent where the case analysis observes it
/// makes the equality unencodable (it cannot arise under
/// the Sym::Result invariants from well-formed values).
#[cfg(feature = "z3-support")]
fn sym_result_eq(
    a_ok: &Tag,
    a_okp: &Option<Box<Sym>>,
    a_errp: &Option<Box<Sym>>,
    b_ok: &Tag,
    b_okp: &Option<Box<Sym>>,
    b_errp: &Option<Box<Sym>>,
) -> Option<z3::ast::Bool> {
    use z3::ast::Bool;
    let slot = |x: &Option<Box<Sym>>, y: &Option<Box<Sym>>| -> Option<z3::ast::Bool> {
        sym_eq((**x.as_ref()?).clone(), (**y.as_ref()?).clone())
    };
    match (a_ok, b_ok) {
        (Tag::False, Tag::False) => slot(a_errp, b_errp),
        (Tag::True, Tag::True) => slot(a_okp, b_okp),
        (Tag::True, Tag::False) | (Tag::False, Tag::True) => {
            Some(Bool::from_bool(false))
        }
        (Tag::True, Tag::Free(_)) => {
            let e = slot(a_okp, b_okp)?;
            Some(Bool::and(&[&b_ok.term(), &e]))
        }
        (Tag::Free(_), Tag::True) => {
            let e = slot(a_okp, b_okp)?;
            Some(Bool::and(&[&a_ok.term(), &e]))
        }
        (Tag::False, Tag::Free(_)) => {
            let e = slot(a_errp, b_errp)?;
            Some(Bool::and(&[&b_ok.term().not(), &e]))
        }
        (Tag::Free(_), Tag::False) => {
            let e = slot(a_errp, b_errp)?;
            Some(Bool::and(&[&a_ok.term().not(), &e]))
        }
        (Tag::Free(_), Tag::Free(_)) => {
            let oe = slot(a_okp, b_okp)?;
            let ee = slot(a_errp, b_errp)?;
            let ta = a_ok.term();
            let tb = b_ok.term();
            let observed = Bool::or(&[
                &Bool::and(&[&ta, &oe]),
                &Bool::and(&[&ta.not(), &ee]),
            ]);
            Some(Bool::and(&[&ta.eq(&tb), &observed]))
        }
    }
}

#[cfg(feature = "z3-support")]
fn sym_binop_ir(op: IRBinOp, l: Sym, r: Sym) -> Option<Sym> {
    let kind = match op {
        IRBinOp::Add => BinKind::Add,
        IRBinOp::Sub => BinKind::Sub,
        IRBinOp::Mul => BinKind::Mul,
        IRBinOp::Div => BinKind::Div,
        IRBinOp::Mod => BinKind::Mod,
        IRBinOp::And => BinKind::And,
        IRBinOp::Or => BinKind::Or,
        IRBinOp::Xor => BinKind::Xor,
        IRBinOp::Eq => BinKind::Eq,
        IRBinOp::Neq => BinKind::Neq,
        IRBinOp::Lt => BinKind::Lt,
        IRBinOp::Le => BinKind::Le,
        IRBinOp::Gt => BinKind::Gt,
        IRBinOp::Ge => BinKind::Ge,
        IRBinOp::Concat => BinKind::Concat,
        IRBinOp::Append => BinKind::Append,
        // Z3's Int has no shifts, and Merge has no meaning in
        // the interpreter either: unencodable.
        IRBinOp::Shl | IRBinOp::Shr | IRBinOp::Merge => return None,
    };
    sym_binop(kind, l, r)
}

#[cfg(feature = "z3-support")]
fn sym_neg(v: Sym) -> Option<Sym> {
    match v {
        Sym::Int(a) => Some(Sym::Int(z3_sub(&z3::ast::Int::from_i64(0), &a))),
        _ => None,
    }
}

#[cfg(feature = "z3-support")]
fn sym_not(v: Sym) -> Option<Sym> {
    match v {
        Sym::Bool(a) => Some(Sym::Bool(a.not())),
        _ => None,
    }
}

#[cfg(feature = "z3-support")]
fn sym_abs(v: Sym) -> Option<Sym> {
    match v {
        Sym::Int(a) => {
            let zero = z3::ast::Int::from_i64(0);
            let neg = z3_sub(&zero, &a);
            Some(Sym::Int(a.ge(&zero).ite(&a, &neg)))
        }
        _ => None,
    }
}

#[cfg(feature = "z3-support")]
fn sym_unary_ir(op: IRUnaryOp, v: Sym) -> Option<Sym> {
    match op {
        IRUnaryOp::Neg => sym_neg(v),
        IRUnaryOp::Not => sym_not(v),
        IRUnaryOp::Abs => sym_abs(v),
        IRUnaryOp::Len => sym_len(v),
        // Clone (Phase 91, W3): a value copy. Sym terms
        // are immutable values, so the copy IS the value —
        // the identity is the exact symbolic semantics
        // (the Phase 90 probe's clone elision, promoted).
        IRUnaryOp::Clone => Some(v),
        // Head/Tail/Init/Last/Reverse are outside the
        // interpreter's evaluable subset too.
        _ => None,
    }
}

// ── Statement-mode encoding (Phase 93) ───────────────────
//
// The production form of the Phase 90 probe evaluator
// (ENCODER_EXTENSION_DESIGN.md §2.6/§2.7 Layer 1): a
// path-state symbolic evaluator for the statement subset
// of the IR — Block / Alloc / Assign / While / Loop /
// Break / TryPropagate — layered on the expression
// encoder's primitives (`sym_eval`, `sym_call`,
// `sym_ite`, `encode_constraint`). Loops unroll to a
// fuel bound under the §2.2 BMC discipline: evaluation
// returns (paths, exhausted), and `verify_one`'s
// statement judge (`verify_constraint_stmt`) turns that
// into verdicts — Refuted on completed paths, Proven
// only when exhaustion is unreachable, Unknown
// otherwise. Symbolic Verified of an unbounded loop is
// NOT this layer's product (exhaustion is reachable at
// every fuel); that route is loop invariants (Phase 96).
//
// Two execution modes, as measured in Phase 90. GUIDED:
// while the solver decides a branch/loop condition
// under the current path (forced-true or forced-false),
// execution follows the single decided branch — binds
// replace outright, no ite merge, terms stay
// one-iteration sized (point evaluation is flat ~5 ms).
// MERGING: the first undecidable branch flips the state
// to classic pc-narrowing symbolic execution (binds
// merge by ite) for the rest of the run. Naive merging
// from the start grows ~×8 per iteration; guidance is
// what keeps points cheap, and merging is what keeps
// symbolic verdicts honest.
//
// Value positions delegate to the expression encoder
// wherever it reaches; the statement layer adds exactly
// two things there: calls to specs whose denotations
// are statement-bodied (evaluated by this layer and
// assembled back into one value), and Append in the
// forms the statement bodies use (see `stmt_append`).
// Term identity is Z3 node identity throughout — never
// Debug strings (Phase 90's measured 96 s trap).
//
// Declines, kept honest (all return None → the caller
// reports Unknown, never a guess): a `Break` carrying a
// value (the bodies at stake break bare; a valued break
// is a value-position loop result this layer does not
// model), a `For` loop, an Assign to a non-Var target,
// a TryPropagate anywhere but an Alloc's initial value,
// and anything the expression encoder declines inside a
// delegated subtree.

/// Statement-evaluation context: the expression
/// encoder's resolution context (index, cycle guard,
/// inlining depth) plus the unrolling fuel and the
/// solver timeout every decision check runs under (the
/// verifier's per-constraint timeout — a cap hit decides
/// nothing and surfaces as Unknown, never as a verdict).
#[cfg(feature = "z3-support")]
struct StmtCtx<'a> {
    eval: EvalCtx<'a>,
    fuel: usize,
    timeout_ms: u64,
}

#[cfg(feature = "z3-support")]
impl<'a> StmtCtx<'a> {
    fn root(index: &'a SpecIndex, fuel: usize, timeout_ms: u64) -> Self {
        Self {
            eval: EvalCtx::root(index),
            fuel,
            timeout_ms,
        }
    }

    fn child(&self, name: &str) -> Self {
        Self {
            eval: self.eval.child(name),
            fuel: self.fuel,
            timeout_ms: self.timeout_ms,
        }
    }
}

/// One execution path's state under statement-mode
/// evaluation.
#[cfg(feature = "z3-support")]
#[derive(Clone)]
struct StmtState {
    env: SymEnv,
    /// Path condition under which execution reaches the
    /// current point.
    pc: z3::ast::Bool,
    /// (pc, value) return contributions, in order.
    rets: Vec<(z3::ast::Bool, Sym)>,
    /// Path conditions under which a Break fired and has
    /// not yet been consumed by its loop.
    breaks: Vec<z3::ast::Bool>,
    /// Path conditions under which a loop still wanted
    /// another iteration when its fuel ran out. Where
    /// the exhaustion disjunction is satisfiable the
    /// body has NOT been fully evaluated.
    exhausted: Vec<z3::ast::Bool>,
    /// The running block value (production Block
    /// semantics: the last statement's value).
    last: Option<Sym>,
    /// Variables bound since this state was created
    /// (drives the If environment merge).
    writes: std::collections::HashSet<String>,
    /// Standing assumptions for solver-guided decisions.
    assumptions: Vec<z3::ast::Bool>,
    /// Guided mode: every branch so far was solver-
    /// DECIDED, so the state is a single path and binds
    /// replace outright (no ite merge). The first
    /// undecidable branch flips the state to merging
    /// mode for the rest of the run.
    guided: bool,
}

/// A statement body's evaluated outcome: the per-path
/// output values and the exhaustion disjunction — the
/// pair the §2.2 discipline judges over.
#[cfg(feature = "z3-support")]
struct StmtOutcome {
    /// (pc, output value) per return path, plus the
    /// fall-through value under the remaining pc when
    /// the body can end without returning.
    paths: Vec<(z3::ast::Bool, Sym)>,
    /// Disjunction of every loop-exhaustion path
    /// condition recorded during the evaluation.
    exhausted: z3::ast::Bool,
}

#[cfg(feature = "z3-support")]
fn stmt_is_true(b: &z3::ast::Bool) -> bool {
    matches!(b.as_bool(), Some(true))
}

#[cfg(feature = "z3-support")]
fn stmt_is_false(b: &z3::ast::Bool) -> bool {
    matches!(b.as_bool(), Some(false))
}

#[cfg(feature = "z3-support")]
fn stmt_or_all(terms: &[z3::ast::Bool]) -> z3::ast::Bool {
    let mut acc = z3::ast::Bool::from_bool(false);
    for t in terms {
        acc = z3::ast::Bool::or(&[&acc, t]);
    }
    acc
}

/// Structural identity of two Sym values, by Z3 NODE
/// identity per term (Z3 hash-conses; AST `==` is
/// `Z3_is_eq_ast`, O(1)). Never Debug strings: printing
/// a shared DAG as a tree is exponential in its depth
/// (Phase 90 measured 96 s on the STEP body). Used for
/// the merge fast path below.
#[cfg(feature = "z3-support")]
fn stmt_sym_identical(a: &Sym, b: &Sym) -> bool {
    fn term_eq<T: PartialEq>(x: &T, y: &T) -> bool {
        x == y
    }
    match (a, b) {
        (Sym::Int(x), Sym::Int(y)) => term_eq(x, y),
        (Sym::Bool(x), Sym::Bool(y)) => term_eq(x, y),
        (Sym::Str(x), Sym::Str(y)) => term_eq(x, y),
        (Sym::Char(x), Sym::Char(y)) => term_eq(x, y),
        (Sym::CharStr(x), Sym::CharStr(y)) => term_eq(x, y),
        (Sym::ListLen(x), Sym::ListLen(y)) => term_eq(x, y),
        (Sym::List(x), Sym::List(y)) => {
            x.len() == y.len()
                && x.iter().zip(y.iter()).all(|(u, v)| stmt_sym_identical(u, v))
        }
        (
            Sym::Record {
                type_name: ta,
                fields: fa,
            },
            Sym::Record {
                type_name: tb,
                fields: fb,
            },
        ) => {
            ta == tb
                && fa.len() == fb.len()
                && fa.iter().zip(fb.iter()).all(|((na, va), (nb, vb))| {
                    na == nb && stmt_sym_identical(va, vb)
                })
        }
        (
            Sym::Option {
                some: sa,
                payload: pa,
            },
            Sym::Option {
                some: sb,
                payload: pb,
            },
        ) => {
            std::mem::discriminant(sa) == std::mem::discriminant(sb)
                && match (pa, pb) {
                    (None, None) => true,
                    (Some(x), Some(y)) => stmt_sym_identical(x, y),
                    _ => false,
                }
        }
        (
            Sym::Result {
                ok: oa,
                ok_payload: pa,
                err_payload: ea,
            },
            Sym::Result {
                ok: ob,
                ok_payload: pb,
                err_payload: eb,
            },
        ) => {
            std::mem::discriminant(oa) == std::mem::discriminant(ob)
                && match (pa, pb) {
                    (None, None) => true,
                    (Some(x), Some(y)) => stmt_sym_identical(x, y),
                    _ => false,
                }
                && match (ea, eb) {
                    (None, None) => true,
                    (Some(x), Some(y)) => stmt_sym_identical(x, y),
                    _ => false,
                }
        }
        _ => false,
    }
}

/// The statement layer's merge: node-identical sides
/// merge for free at EVERY level (the tokenizer state
/// at every step carries its token list unchanged, and
/// merging it must not rebuild the term); structural
/// arms recurse through this function; leaf arms defer
/// to production `sym_ite`.
#[cfg(feature = "z3-support")]
fn stmt_ite(cond: &z3::ast::Bool, a: Sym, b: Sym) -> Option<Sym> {
    if stmt_sym_identical(&a, &b) {
        return Some(b);
    }
    match (a, b) {
        (Sym::List(x), Sym::List(y)) => {
            if x.len() != y.len() {
                return None;
            }
            let mut out = Vec::with_capacity(x.len());
            for (u, v) in x.into_iter().zip(y) {
                out.push(stmt_ite(cond, u, v)?);
            }
            Some(Sym::List(out))
        }
        (
            Sym::Record {
                type_name: ta,
                fields: fa,
            },
            Sym::Record {
                type_name: tb,
                fields: fb,
            },
        ) => {
            if ta != tb || fa.len() != fb.len() {
                return None;
            }
            let mut merged = Vec::with_capacity(fa.len());
            for (name, va) in fa {
                let vb = fb
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, v)| v.clone())?;
                merged.push((name, stmt_ite(cond, va, vb)?));
            }
            Some(Sym::Record {
                type_name: ta,
                fields: merged,
            })
        }
        (
            Sym::Option {
                some: ta,
                payload: pa,
            },
            Sym::Option {
                some: tb,
                payload: pb,
            },
        ) => Some(Sym::Option {
            some: Tag::merge(cond, &ta, &tb),
            payload: stmt_ite_payload(cond, pa, pb)?,
        }),
        (
            Sym::Result {
                ok: ta,
                ok_payload: oa,
                err_payload: ea,
            },
            Sym::Result {
                ok: tb,
                ok_payload: ob,
                err_payload: eb,
            },
        ) => Some(Sym::Result {
            ok: Tag::merge(cond, &ta, &tb),
            ok_payload: stmt_ite_payload(cond, oa, ob)?,
            err_payload: stmt_ite_payload(cond, ea, eb)?,
        }),
        (a, b) => sym_ite(cond, a, b),
    }
}

#[cfg(feature = "z3-support")]
fn stmt_ite_payload(
    cond: &z3::ast::Bool,
    a: Option<Box<Sym>>,
    b: Option<Box<Sym>>,
) -> Option<Option<Box<Sym>>> {
    // Mirrors production `sym_ite_payload`: a slot
    // present on one side only is observed solely under
    // that side's tag, so it carries over.
    match (a, b) {
        (None, None) => Some(None),
        (Some(x), None) => Some(Some(x)),
        (None, Some(y)) => Some(Some(y)),
        (Some(x), Some(y)) => stmt_ite(cond, *x, *y).map(|v| Some(Box::new(v))),
    }
}

/// Append one element, in the forms the statement
/// bodies use: production `sym_binop_ir`'s Append arms
/// (concrete list, length-only list, Str+CharStr), plus
/// the one form Phase 91 deliberately left unarmed in
/// the expression encoder — a string grown by a NUMERAL
/// Char. At a concrete point a constant index into a
/// literal string yields the scalar Char form, and the
/// lexeme loop's append is exactly that shape; the
/// character is a known numeral, so the concatenation
/// is exact, not a conversion guess. This arm lives
/// only in the statement layer: the routing discipline
/// keeps it from ever judging a body the expression
/// encoder declined for non-statement reasons.
#[cfg(feature = "z3-support")]
fn stmt_append(cur: Sym, item: Sym) -> Option<Sym> {
    if let (Sym::Str(s), Sym::Char(c)) = (&cur, &item) {
        let n = c.as_i64()?;
        let ch = char::from_u32(n as u32)?;
        let one = z3::ast::String::from_str(&ch.to_string()).ok()?;
        return Some(Sym::Str(z3::ast::String::concat(&[s, &one])));
    }
    sym_binop_ir(IRBinOp::Append, cur, item)
}

/// Satisfiability of a conjunction of Bool terms under
/// the verifier's per-constraint timeout: Some(true) =
/// SAT, Some(false) = UNSAT, None = the solver could
/// not decide (Unknown or the cap hit — a cap hit
/// decides nothing).
#[cfg(feature = "z3-support")]
fn stmt_sat(terms: &[z3::ast::Bool], timeout_ms: u64) -> Option<bool> {
    let z = Z3Session::new(timeout_ms);
    for t in terms {
        z.assert(t);
    }
    match z.check() {
        SmtResult::Sat(_) => Some(true),
        SmtResult::Unsat => Some(false),
        _ => None,
    }
}

/// Solver decision under the state's standing context
/// (assumptions + Char side conditions + pc):
/// Some(true) = satisfiable, Some(false) = refuted,
/// None = the solver could not decide.
#[cfg(feature = "z3-support")]
fn stmt_decide(st: &StmtState, f: &z3::ast::Bool, timeout_ms: u64) -> Option<bool> {
    let mut assertions: Vec<z3::ast::Bool> = st.assumptions.clone();
    for v in st.env.values() {
        collect_char_conditions(v, &mut assertions);
    }
    assertions.push(st.pc.clone());
    assertions.push(f.clone());
    stmt_sat(&assertions, timeout_ms)
}

/// Bind `name` to `v` under the current path condition:
/// an unconditional bind replaces; a conditioned bind
/// merges with the previous value by ite (a merge the
/// Sym forms cannot express fails the evaluation).
#[cfg(feature = "z3-support")]
fn stmt_bind(st: &mut StmtState, name: &str, v: Sym) -> Option<()> {
    st.writes.insert(name.to_string());
    if st.guided || stmt_is_true(&st.pc) {
        // A guided state is a single decided path: the
        // bind is unconditional on it.
        st.env.insert(name.to_string(), v);
        return Some(());
    }
    match st.env.get(name) {
        None => {
            st.env.insert(name.to_string(), v);
            Some(())
        }
        Some(old) => {
            let old = old.clone();
            let merged = stmt_ite(&st.pc, v, old)?;
            st.env.insert(name.to_string(), merged);
            Some(())
        }
    }
}

/// Bind the Ok payload of a Result value (the
/// TryPropagate semantics): on the Err path the
/// enclosing function returns the Err at once; on the
/// Ok path execution continues with the payload bound.
#[cfg(feature = "z3-support")]
fn stmt_bind_try(st: &mut StmtState, name: &str, v: Sym, ctx: &StmtCtx) -> Option<()> {
    match v {
        Sym::Result {
            ok,
            ok_payload,
            err_payload,
        } => match ok {
            Tag::True => {
                let p = ok_payload?;
                stmt_bind(st, name, *p)
            }
            Tag::False => {
                let e = err_payload?;
                let ret = Sym::Result {
                    ok: Tag::False,
                    ok_payload: None,
                    err_payload: Some(e),
                };
                st.rets.push((st.pc.clone(), ret));
                st.pc = z3::ast::Bool::from_bool(false);
                Some(())
            }
            Tag::Free(t) => {
                // Guided: decide the tag under the
                // standing context first — at a point it
                // is always decidable, and the decided
                // path stays merge-free.
                if st.guided {
                    match (
                        stmt_decide(st, &t, ctx.timeout_ms),
                        stmt_decide(st, &t.not(), ctx.timeout_ms),
                    ) {
                        (Some(true), Some(false)) => {
                            let p = ok_payload?;
                            st.pc = z3::ast::Bool::and(&[&st.pc, &t]);
                            return stmt_bind(st, name, *p);
                        }
                        (Some(false), Some(true)) => {
                            let e = err_payload?;
                            let ret = Sym::Result {
                                ok: Tag::False,
                                ok_payload: None,
                                err_payload: Some(e),
                            };
                            st.rets.push((st.pc.clone(), ret));
                            st.pc = z3::ast::Bool::from_bool(false);
                            return Some(());
                        }
                        _ => {
                            st.guided = false;
                        }
                    }
                }
                let p = ok_payload?;
                let e = err_payload?;
                let err_ret = Sym::Result {
                    ok: Tag::False,
                    ok_payload: None,
                    err_payload: Some(e),
                };
                let err_pc = z3::ast::Bool::and(&[&st.pc, &t.not()]);
                st.rets.push((err_pc, err_ret));
                st.pc = z3::ast::Bool::and(&[&st.pc, &t]);
                stmt_bind(st, name, *p)
            }
        },
        _ => None,
    }
}

/// Assemble a denoted sub-evaluation's paths into one
/// value: by ite where the Sym forms allow, else — the
/// point case, where every path condition is a closed
/// term — by solver concretization (exactly one live
/// path). The assembled value is observed only on
/// completed paths: the caller's judging conjoins
/// ¬exhausted, which excludes exactly the region where
/// the fold's fallback value is arbitrary.
#[cfg(feature = "z3-support")]
fn stmt_assemble(paths: &[(z3::ast::Bool, Sym)], timeout_ms: u64) -> Option<Sym> {
    if paths.len() == 1 {
        return Some(paths[0].1.clone());
    }
    let mut acc: Option<Sym> = None;
    let mut ite_ok = true;
    for (pc, v) in paths.iter().rev() {
        match acc.take() {
            None => acc = Some(v.clone()),
            Some(rest) => match stmt_ite(pc, v.clone(), rest) {
                Some(m) => acc = Some(m),
                None => {
                    ite_ok = false;
                    break;
                }
            },
        }
    }
    if ite_ok {
        return acc;
    }
    let mut live: Option<Sym> = None;
    for (pc, v) in paths {
        match stmt_sat(&[pc.clone()], timeout_ms) {
            Some(true) => {
                if live.is_some() {
                    return None;
                }
                live = Some(v.clone());
            }
            Some(false) => {}
            None => return None,
        }
    }
    live
}

/// Invoke a call whose callee's denotation is
/// statement-bodied (the expression encoder's
/// `sym_invoke` declines on it): the callee body is
/// evaluated by THIS layer in a fresh environment
/// binding the callee's parameters to the argument
/// values — substituting values, not syntax, exactly
/// as `sym_invoke` does — under the same inlining
/// discipline (depth cap + cycle guard, via
/// `EvalCtx::can_inline`). The callee's exhaustion is
/// gated by the caller's current path condition and
/// recorded on the caller; a callee that exhausts on
/// every path kills the caller's path BY EXHAUSTION
/// (recorded, never silently) and hands back a
/// placeholder no live path observes.
#[cfg(feature = "z3-support")]
fn stmt_invoke_value(
    st: &mut StmtState,
    ctx: &StmtCtx,
    name: &str,
    args: &[IRNode],
) -> Option<Sym> {
    let spec = ctx.eval.index.get(name)?;
    let body = ctx.eval.index.callee_body(name)?;
    if !ctx.eval.can_inline(name) || spec.params.len() != args.len() {
        return None;
    }
    let mut argv = Vec::with_capacity(args.len());
    for a in args {
        argv.push(stmt_eval_value(a, st, ctx)?);
    }
    let mut env = SymEnv::new();
    for (p, v) in spec.params.iter().zip(argv) {
        env.insert(p.name.clone(), v);
    }
    let outcome = sym_eval_stmt(&body, env, &ctx.child(name))?;
    if !stmt_is_false(&outcome.exhausted) {
        let gated = z3::ast::Bool::and(&[&st.pc, &outcome.exhausted]);
        st.exhausted.push(gated);
    }
    if outcome.paths.is_empty() {
        st.pc = z3::ast::Bool::from_bool(false);
        return Some(Sym::Opaque);
    }
    stmt_assemble(&outcome.paths, ctx.timeout_ms)
}

/// Expression-position evaluation for the statement
/// layer: the production symbolic evaluator wherever it
/// reaches (which since Phase 91 includes constructor
/// Match and Clone), plus the two statement-layer
/// additions — calls the expression encoder leaves
/// undenoted because their denotations are
/// statement-bodied, and Append in `stmt_append`'s
/// forms. Anything else the expression encoder declines
/// inside a delegated subtree declines here too.
#[cfg(feature = "z3-support")]
fn stmt_eval_value(node: &IRNode, st: &mut StmtState, ctx: &StmtCtx) -> Option<Sym> {
    match node {
        IRNode::Call { function, args } => {
            let mut env = st.env.clone();
            if let Some(v) = sym_call(function, args, &mut env, &ctx.eval) {
                return Some(v);
            }
            stmt_invoke_value(st, ctx, function, args)
        }
        IRNode::BinOp(IRBinOp::Append, l, r) => {
            let lv = stmt_eval_value(l, st, ctx)?;
            let rv = stmt_eval_value(r, st, ctx)?;
            stmt_append(lv, rv)
        }
        _ => {
            let mut env = st.env.clone();
            sym_eval(node, &mut env, &ctx.eval)
        }
    }
}

/// Statement-mode evaluation. Mirrors the compiled
/// semantics the cross-validation runner executes:
/// Alloc binds, Assign rebinds, Break leaves the
/// innermost loop, Return leaves the function, and
/// loops repeat — bounded by ctx.fuel, with the paths
/// that wanted more recorded as exhaustion.
#[cfg(feature = "z3-support")]
fn stmt_eval(node: &IRNode, st: &mut StmtState, ctx: &StmtCtx) -> Option<()> {
    if stmt_is_false(&st.pc) {
        return Some(());
    }
    match node {
        IRNode::Block(stmts) => {
            for s in stmts {
                stmt_eval(s, st, ctx)?;
            }
            Some(())
        }
        IRNode::Alloc { name, initial, .. } => {
            if let IRNode::TryPropagate(inner) = &**initial {
                let v = stmt_eval_value(inner, st, ctx)?;
                if stmt_is_false(&st.pc) {
                    // The call died by exhaustion
                    // mid-statement; the path is dead
                    // and its value a placeholder.
                    return Some(());
                }
                return stmt_bind_try(st, name, v, ctx);
            }
            let v = stmt_eval_value(initial, st, ctx)?;
            if stmt_is_false(&st.pc) {
                return Some(());
            }
            st.last = Some(v.clone());
            stmt_bind(st, name, v)
        }
        IRNode::Assign { target, value } => {
            let IRNode::Var(name) = &**target else {
                return None;
            };
            let v = stmt_eval_value(value, st, ctx)?;
            if stmt_is_false(&st.pc) {
                return Some(());
            }
            st.last = Some(v.clone());
            stmt_bind(st, name, v)
        }
        IRNode::Return(inner) => {
            let v = stmt_eval_value(inner, st, ctx)?;
            st.rets.push((st.pc.clone(), v));
            st.pc = z3::ast::Bool::from_bool(false);
            Some(())
        }
        IRNode::Break { value } => {
            // A valued Break is a value-position loop
            // result, which this layer does not model:
            // decline rather than drop the value.
            if value.is_some() {
                return None;
            }
            st.breaks.push(st.pc.clone());
            st.pc = z3::ast::Bool::from_bool(false);
            Some(())
        }
        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond = match stmt_eval_value(condition, st, ctx)? {
                Sym::Bool(b) => b,
                _ => return None,
            };
            if st.guided {
                // Guided path: decide the condition
                // under the standing context and take
                // the single decided branch in place —
                // no clone, no merge.
                let can_then = stmt_decide(st, &cond, ctx.timeout_ms);
                let can_else = stmt_decide(st, &cond.not(), ctx.timeout_ms);
                match (can_then, can_else) {
                    (Some(true), Some(false)) => {
                        st.pc = z3::ast::Bool::and(&[&st.pc, &cond]);
                        return stmt_eval(then_branch, st, ctx);
                    }
                    (Some(false), Some(true)) => {
                        st.pc = z3::ast::Bool::and(&[&st.pc, &cond.not()]);
                        return stmt_eval(else_branch, st, ctx);
                    }
                    _ => {
                        // Undecidable here: merging
                        // mode from this point on.
                        st.guided = false;
                    }
                }
            }
            let base_pc = st.pc.clone();
            let fresh = |pc: z3::ast::Bool| StmtState {
                env: st.env.clone(),
                pc,
                rets: Vec::new(),
                breaks: Vec::new(),
                exhausted: Vec::new(),
                last: None,
                writes: std::collections::HashSet::new(),
                assumptions: st.assumptions.clone(),
                guided: false,
            };
            let mut then_st = fresh(z3::ast::Bool::and(&[&base_pc, &cond]));
            stmt_eval(then_branch, &mut then_st, ctx)?;
            let mut else_st = fresh(z3::ast::Bool::and(&[&base_pc, &cond.not()]));
            stmt_eval(else_branch, &mut else_st, ctx)?;
            // Continuation: the paths that neither
            // returned nor broke inside either branch.
            let mut cont = base_pc.clone();
            for (pc, _) in then_st.rets.iter().chain(else_st.rets.iter()) {
                cont = z3::ast::Bool::and(&[&cont, &pc.not()]);
            }
            for pc in then_st.breaks.iter().chain(else_st.breaks.iter()) {
                cont = z3::ast::Bool::and(&[&cont, &pc.not()]);
            }
            st.rets.extend(then_st.rets.iter().cloned());
            st.rets.extend(else_st.rets.iter().cloned());
            st.breaks.extend(then_st.breaks.iter().cloned());
            st.breaks.extend(else_st.breaks.iter().cloned());
            st.exhausted.extend(then_st.exhausted.iter().cloned());
            st.exhausted.extend(else_st.exhausted.iter().cloned());
            if stmt_is_false(&cont) {
                // No live continuation: the merged
                // environment is dead. Keep the pre-If
                // one — it is never observed on a live
                // path.
                st.pc = cont;
                return Some(());
            }
            // Merge the branch environments. A variable
            // neither branch wrote carries the pre-If
            // value on both sides. A written variable
            // merges by ite when both sides hold it; a
            // variable only one side holds is
            // branch-local (read, for the bodies at
            // stake, only inside its own branch).
            let mut merged = SymEnv::new();
            let keys: std::collections::HashSet<&String> = then_st
                .env
                .keys()
                .chain(else_st.env.keys())
                .collect();
            for k in keys {
                let written =
                    then_st.writes.contains(k) || else_st.writes.contains(k);
                match (then_st.env.get(k), else_st.env.get(k)) {
                    (Some(a), Some(b)) if written => {
                        merged.insert(k.clone(), stmt_ite(&cond, a.clone(), b.clone())?);
                    }
                    (Some(a), _) => {
                        merged.insert(k.clone(), a.clone());
                    }
                    (None, Some(b)) => {
                        merged.insert(k.clone(), b.clone());
                    }
                    (None, None) => unreachable!(),
                }
            }
            st.env = merged;
            st.writes.extend(then_st.writes.iter().cloned());
            st.writes.extend(else_st.writes.iter().cloned());
            st.last = match (then_st.last, else_st.last) {
                (Some(a), Some(b)) => stmt_ite(&cond, a, b),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => st.last.clone(),
            };
            st.pc = cont;
            Some(())
        }
        IRNode::While {
            condition, body, ..
        } => stmt_loop(Some(condition), body, st, ctx),
        IRNode::Loop { body } => stmt_loop(None, body, st, ctx),
        IRNode::BinOp(IRBinOp::Append, l, r) => {
            // A statement-position Append rebinds its
            // target (the compiled rendering is a push
            // on the binding).
            let IRNode::Var(name) = &**l else {
                return None;
            };
            let cur = st.env.get(name)?.clone();
            let item = stmt_eval_value(r, st, ctx)?;
            if stmt_is_false(&st.pc) {
                return Some(());
            }
            let v = stmt_append(cur, item)?;
            st.last = Some(v.clone());
            stmt_bind(st, name, v)
        }
        _ => {
            // Any other statement is an expression
            // statement: its value is the block's
            // running value, discarded unless last.
            let v = stmt_eval_value(node, st, ctx)?;
            st.last = Some(match st.last.take() {
                Some(old) if !stmt_is_true(&st.pc) => stmt_ite(&st.pc, v, old)?,
                _ => v,
            });
            Some(())
        }
    }
}

/// One loop (While or Loop), unrolled to ctx.fuel
/// iterations. Two modes. GUIDED: while the solver
/// decides the condition under the current path
/// (forced-true or forced-false), the loop follows the
/// single decided branch — the path condition
/// accumulates the decided facts and NO ite merge
/// happens, so terms stay one-iteration sized. MERGING:
/// once a condition is genuinely undecided, the loop
/// narrows the pc and merges binds by ite, as a
/// standard symbolic unrolling. In both modes the
/// paths that would iterate past the fuel are recorded
/// as exhaustion and excluded from the continuation —
/// never silently treated as exits (the BMC
/// discipline).
#[cfg(feature = "z3-support")]
fn stmt_loop(
    condition: Option<&IRNode>,
    body: &IRNode,
    st: &mut StmtState,
    ctx: &StmtCtx,
) -> Option<()> {
    let breaks0 = st.breaks.len();
    let mut iter = 0usize;
    let mut merging = false;
    loop {
        let c = match condition {
            Some(cn) => match stmt_eval_value(cn, st, ctx)? {
                Sym::Bool(b) => b,
                _ => return None,
            },
            None => z3::ast::Bool::from_bool(true),
        };
        let live = st.pc.clone();
        if !merging {
            // Guided decisions: does any live path
            // iterate (pc ∧ c satisfiable)? Does any
            // live path exit (pc ∧ ¬c satisfiable)?
            let can_iter = stmt_decide(st, &c, ctx.timeout_ms);
            let can_exit = stmt_decide(st, &c.not(), ctx.timeout_ms);
            match (can_iter, can_exit) {
                (Some(false), _) => {
                    // Forced exit: no exhaustion, the
                    // loop is done on every live path.
                    let mut exit_pc = live;
                    for b in st.breaks.iter().skip(breaks0) {
                        exit_pc = z3::ast::Bool::or(&[&exit_pc, b]);
                    }
                    st.breaks.truncate(breaks0);
                    st.pc = exit_pc;
                    return Some(());
                }
                (_, Some(false)) => {
                    // Forced iteration.
                    if iter >= ctx.fuel {
                        st.exhausted.push(live);
                        let mut exit_pc = z3::ast::Bool::from_bool(false);
                        for b in st.breaks.iter().skip(breaks0) {
                            exit_pc = z3::ast::Bool::or(&[&exit_pc, b]);
                        }
                        st.breaks.truncate(breaks0);
                        st.pc = exit_pc;
                        return Some(());
                    }
                    st.pc = z3::ast::Bool::and(&[&live, &c]);
                    stmt_eval(body, st, ctx)?;
                    iter += 1;
                    if stmt_is_false(&st.pc) {
                        let mut exit_pc = z3::ast::Bool::from_bool(false);
                        for b in st.breaks.iter().skip(breaks0) {
                            exit_pc = z3::ast::Bool::or(&[&exit_pc, b]);
                        }
                        st.breaks.truncate(breaks0);
                        st.pc = exit_pc;
                        return Some(());
                    }
                    continue;
                }
                _ => {
                    // Genuinely symbolic (or the solver
                    // could not decide): merge from
                    // here on.
                    merging = true;
                    st.guided = false;
                }
            }
        }
        if iter >= ctx.fuel {
            // Out of fuel: the paths where the loop
            // still wants another iteration are the
            // exhaustion; the paths where it does not
            // (plus this loop's break paths) leave.
            let still = z3::ast::Bool::and(&[&live, &c]);
            if !stmt_is_false(&still) {
                st.exhausted.push(still);
            }
            let mut exit_pc = z3::ast::Bool::and(&[&live, &c.not()]);
            for b in st.breaks.iter().skip(breaks0) {
                exit_pc = z3::ast::Bool::or(&[&exit_pc, b]);
            }
            st.breaks.truncate(breaks0);
            st.pc = exit_pc;
            return Some(());
        }
        let iter_pc = z3::ast::Bool::and(&[&live, &c]);
        if stmt_is_false(&iter_pc) {
            // The loop has exited on every live path.
            let mut exit_pc = live;
            for b in st.breaks.iter().skip(breaks0) {
                exit_pc = z3::ast::Bool::or(&[&exit_pc, b]);
            }
            st.breaks.truncate(breaks0);
            st.pc = exit_pc;
            return Some(());
        }
        st.pc = iter_pc;
        stmt_eval(body, st, ctx)?;
        iter += 1;
        if stmt_is_false(&st.pc) {
            // Every path through the body returned or
            // broke: only the break paths fall through
            // the loop.
            let mut exit_pc = z3::ast::Bool::from_bool(false);
            for b in st.breaks.iter().skip(breaks0) {
                exit_pc = z3::ast::Bool::or(&[&exit_pc, b]);
            }
            st.breaks.truncate(breaks0);
            st.pc = exit_pc;
            return Some(());
        }
    }
}

/// Run a statement body to its outcome: the per-path
/// outputs and the exhaustion disjunction. An outcome
/// with NO paths and NO exhaustion is not an outcome
/// (the body produced no value on any path) and
/// declines; an empty path set WITH an exhaustion
/// record is a real outcome (every path wanted more
/// fuel) — judges read it as unresolved, never Proven.
#[cfg(feature = "z3-support")]
fn sym_eval_stmt(body: &IRNode, env: SymEnv, ctx: &StmtCtx) -> Option<StmtOutcome> {
    let mut st = StmtState {
        env,
        pc: z3::ast::Bool::from_bool(true),
        rets: Vec::new(),
        breaks: Vec::new(),
        exhausted: Vec::new(),
        last: None,
        writes: std::collections::HashSet::new(),
        assumptions: Vec::new(),
        guided: true,
    };
    stmt_eval(body, &mut st, ctx)?;
    let mut paths = st.rets.clone();
    if let Some(v) = st.last.clone() {
        // The fall-through path is genuine only if the
        // final pc is satisfiable — a body whose every
        // branch returned leaves a pc that is
        // unsatisfiable but not literally `false` (it is
        // the negation of the return pcs), and its
        // `last` (a branch's running value) is not an
        // output. Ask the solver; keep the path only
        // when fall-through is possible or undecidable.
        // A second guard: when returns exist, a
        // fall-through whose value is of a different Sym
        // family than the returns is a statement
        // artifact (a branch's running value on an
        // exhaustion-adjacent path), not an output of
        // this body.
        let verdict = if stmt_is_false(&st.pc) {
            Some(false)
        } else {
            stmt_decide(&st, &z3::ast::Bool::from_bool(true), ctx.timeout_ms)
        };
        let family_match = match st.rets.first() {
            None => true,
            Some((_, rv)) => {
                std::mem::discriminant(&v) == std::mem::discriminant(rv)
            }
        };
        if verdict != Some(false) && family_match {
            paths.push((st.pc.clone(), v));
        }
    }
    if paths.is_empty() && st.exhausted.is_empty() {
        return None;
    }
    Some(StmtOutcome {
        paths,
        exhausted: stmt_or_all(&st.exhausted),
    })
}

/// At a battery point the evaluation's path conditions
/// are closed terms: exactly one path is live. Returns
/// that path's output — or None when the fuel ran out
/// on the live path (exhaustion satisfiable), when two
/// paths are live, or when the solver cannot decide a
/// path.
#[cfg(feature = "z3-support")]
fn stmt_concretize(outcome: &StmtOutcome, timeout_ms: u64) -> Option<Sym> {
    if stmt_sat(&[outcome.exhausted.clone()], timeout_ms) != Some(false) {
        return None;
    }
    let mut live: Option<Sym> = None;
    for (pc, v) in &outcome.paths {
        match stmt_sat(&[pc.clone()], timeout_ms) {
            Some(true) => {
                if live.is_some() {
                    return None;
                }
                live = Some(v.clone());
            }
            Some(false) => {}
            None => return None,
        }
    }
    live
}

/// The statement encoder's engagement predicate
/// (Phase 93): does this body contain a statement/loop
/// node — While / Loop / Break / Alloc / Assign /
/// TryPropagate — in its own tree or in the denotation
/// of a spec it calls (transitively, under the
/// inlining discipline)? `verify_one` engages the
/// statement layer only where this holds AND the
/// expression encoder declined, so a body the
/// expression encoder declined for any other reason
/// (a hole, a lambda, a guard pattern, an Append form
/// it leaves unarmed) keeps its standing Inconclusive
/// without ever touching this layer.
#[cfg(feature = "z3-support")]
fn body_mentions_statements(node: &IRNode, index: &SpecIndex) -> bool {
    fn walk(
        node: &IRNode,
        index: &SpecIndex,
        visiting: &mut Vec<String>,
        depth: usize,
    ) -> bool {
        match node {
            IRNode::While { .. }
            | IRNode::Loop { .. }
            | IRNode::Break { .. }
            | IRNode::Alloc { .. }
            | IRNode::Assign { .. }
            | IRNode::TryPropagate(_) => true,
            IRNode::Call { function, args } => {
                if args.iter().any(|a| walk(a, index, visiting, depth)) {
                    return true;
                }
                if depth >= MAX_INLINE_DEPTH || visiting.iter().any(|v| v == function) {
                    return false;
                }
                match index.callee_body(function) {
                    Some(body) => {
                        visiting.push(function.clone());
                        let found = walk(&body, index, visiting, depth + 1);
                        visiting.pop();
                        found
                    }
                    None => false,
                }
            }
            IRNode::Block(stmts) => {
                stmts.iter().any(|s| walk(s, index, visiting, depth))
            }
            IRNode::Let { value, body, .. } => {
                walk(value, index, visiting, depth)
                    || walk(body, index, visiting, depth)
            }
            IRNode::If {
                condition,
                then_branch,
                else_branch,
            } => {
                walk(condition, index, visiting, depth)
                    || walk(then_branch, index, visiting, depth)
                    || walk(else_branch, index, visiting, depth)
            }
            IRNode::Match { scrutinee, cases } => {
                walk(scrutinee, index, visiting, depth)
                    || cases
                        .iter()
                        .any(|(_, b)| walk(b, index, visiting, depth))
            }
            IRNode::BinOp(_, l, r) | IRNode::Index(l, r) => {
                walk(l, index, visiting, depth) || walk(r, index, visiting, depth)
            }
            IRNode::UnaryOp(_, inner)
            | IRNode::Return(inner)
            | IRNode::Typed { node: inner, .. } => walk(inner, index, visiting, depth),
            IRNode::Collection { elements, .. } => {
                elements.iter().any(|e| walk(e, index, visiting, depth))
            }
            IRNode::FieldAccess { base, .. } => walk(base, index, visiting, depth),
            IRNode::RecordLit { fields, .. } => fields
                .iter()
                .any(|(_, v)| walk(v, index, visiting, depth)),
            _ => false,
        }
    }
    walk(node, index, &mut Vec::new(), 0)
}

// ── Constraint → Z3 Translation ────────────────────────────
//
// The spec side of the check: postconditions are encoded in
// the SAME environment as the candidate — parameters are the
// free constants and `output` is the candidate's symbolic
// body. Every encoder returns Option: a formula that cannot
// be encoded is Unknown to the caller, never silently `true`
// (the pre-fix encoders mapped boolean-valued subexpressions
// to int 0 and any Or-rooted formula to `true`, which made
// the verdict a function of constraint syntax alone).

#[cfg(feature = "z3-support")]
fn encode_constraint(c: &Constraint, env: &SymEnv, ctx: &EvalCtx) -> Option<z3::ast::Bool> {
    match c {
        Constraint::True => Some(z3::ast::Bool::from_bool(true)),
        Constraint::Not(inner) => Some(encode_constraint(inner, env, ctx)?.not()),
        Constraint::And(parts) => {
            let mut acc = z3::ast::Bool::from_bool(true);
            for p in parts {
                acc = z3_and(&acc, &encode_constraint(p, env, ctx)?);
            }
            Some(acc)
        }
        Constraint::Or(parts) => {
            let mut acc = z3::ast::Bool::from_bool(false);
            for p in parts {
                acc = z3::ast::Bool::or(&[&acc, &encode_constraint(p, env, ctx)?]);
            }
            Some(acc)
        }
        Constraint::Implies(a, b) => {
            Some(encode_constraint(a, env, ctx)?.implies(&encode_constraint(b, env, ctx)?))
        }
        Constraint::Eq(a, b) => match sym_binop(
            BinKind::Eq,
            encode_expr(a, env, ctx)?,
            encode_expr(b, env, ctx)?,
        )? {
            Sym::Bool(b) => Some(b),
            _ => None,
        },
        Constraint::Order(op, a, b) => {
            let kind = match op {
                OrderOp::LessThan => BinKind::Lt,
                OrderOp::LessThanOrEqual => BinKind::Le,
                OrderOp::GreaterThan => BinKind::Gt,
                OrderOp::GreaterThanOrEqual => BinKind::Ge,
            };
            match sym_binop(kind, encode_expr(a, env, ctx)?, encode_expr(b, env, ctx)?)? {
                Sym::Bool(b) => Some(b),
                _ => None,
            }
        }
        Constraint::Expr(expr) => match encode_expr(expr, env, ctx)? {
            Sym::Bool(b) => Some(b),
            _ => None,
        },
        // Forall (Phase 94, encoder extension Part 2):
        // an Int-binder quantifier whose body is
        // quantifier-free and encodes with the binders
        // in scope closes with z3's `forall_const` —
        // the form Phase 90 measured decisive at every
        // skip battery point (mirror Proven 12/12,
        // overshoot Refuted, the invariant consecution
        // discharged in 5 ms). NO trigger patterns:
        // the bare form discharged everything measured,
        // bare and patterned alike (5 vs 6 ms on the
        // consecution), and a pattern is a second
        // mechanism that can only restrict
        // instantiation. The accepted envelope is the
        // measured one, and everything outside it
        // declines whole (None), never partially: a
        // non-Int binder (the CharStr bridge speaks
        // Int indices), an empty or duplicate binder
        // list, a body containing a nested quantifier
        // (unmeasured), or a body the encoder cannot
        // express with the binders bound.
        Constraint::Forall { vars, body } => {
            if vars.is_empty()
                || vars.iter().any(|(_, ty)| *ty != TypeRef::Int)
                || vars
                    .iter()
                    .enumerate()
                    .any(|(i, (n, _))| vars[..i].iter().any(|(m, _)| m == n))
                || constraint_mentions_quantifier(body)
            {
                return None;
            }
            let mut env = env.clone();
            let mut bounds: Vec<z3::ast::Int> = Vec::with_capacity(vars.len());
            for (name, _) in vars {
                let bound = z3::ast::Int::new_const(name.as_str());
                env.insert(name.clone(), Sym::Int(bound.clone()));
                bounds.push(bound);
            }
            let f = encode_constraint(body, &env, ctx)?;
            let bound_refs: Vec<&dyn z3::ast::Ast> =
                bounds.iter().map(|b| b as &dyn z3::ast::Ast).collect();
            Some(z3::ast::forall_const(&bound_refs, &[], &f))
        }
        // Existentials and named predicates (is_sorted
        // and friends) have no encoding here.
        Constraint::Exists { .. } | Constraint::Predicate(..) => None,
    }
}

/// Whether a constraint syntactically contains a
/// quantifier anywhere below its root — the Forall
/// arm's nesting guard (a Forall's own body must be
/// quantifier-free; quantifiers elsewhere in a
/// constraint tree are reached structurally and judged
/// on their own envelope).
#[cfg(feature = "z3-support")]
fn constraint_mentions_quantifier(c: &Constraint) -> bool {
    match c {
        Constraint::Forall { .. } | Constraint::Exists { .. } => true,
        Constraint::Not(inner) => constraint_mentions_quantifier(inner),
        Constraint::And(parts) | Constraint::Or(parts) => {
            parts.iter().any(constraint_mentions_quantifier)
        }
        Constraint::Implies(a, b) => {
            constraint_mentions_quantifier(a) || constraint_mentions_quantifier(b)
        }
        _ => false,
    }
}

#[cfg(feature = "z3-support")]
fn encode_expr(expr: &Expr, env: &SymEnv, ctx: &EvalCtx) -> Option<Sym> {
    match expr {
        Expr::Var(name) => env.get(name).cloned(),
        Expr::IntLit(n) => Some(Sym::Int(z3::ast::Int::from_i64(*n))),
        Expr::FloatLit(_) => Some(Sym::Opaque),
        Expr::BoolLit(b) => Some(Sym::Bool(z3::ast::Bool::from_bool(*b))),
        Expr::StringLit(s) => Some(Sym::Str(z3::ast::String::from_str(s).ok()?)),
        // Char (Phase 76, D5a): its Unicode scalar value on
        // the Int sort — a literal is a genuine scalar, so
        // no side condition is owed for it.
        Expr::CharLit(c) => Some(Sym::Char(z3::ast::Int::from_i64(*c as u32 as i64))),
        Expr::BinOp(op, l, r) => {
            let kind = match op {
                BinOp::Add => BinKind::Add,
                BinOp::Sub => BinKind::Sub,
                BinOp::Mul => BinKind::Mul,
                BinOp::Div => BinKind::Div,
                BinOp::Mod => BinKind::Mod,
                BinOp::And => BinKind::And,
                BinOp::Or => BinKind::Or,
                BinOp::Eq => BinKind::Eq,
                BinOp::Neq => BinKind::Neq,
                BinOp::Lt => BinKind::Lt,
                BinOp::Le => BinKind::Le,
                BinOp::Gt => BinKind::Gt,
                BinOp::Ge => BinKind::Ge,
                BinOp::Concat => BinKind::Concat,
            };
            sym_binop(kind, encode_expr(l, env, ctx)?, encode_expr(r, env, ctx)?)
        }
        Expr::UnaryOp(op, inner) => match op {
            UnaryOp::Neg => sym_neg(encode_expr(inner, env, ctx)?),
            UnaryOp::Not => sym_not(encode_expr(inner, env, ctx)?),
        },
        Expr::Length(inner) => sym_len(encode_expr(inner, env, ctx)?),
        Expr::Call(name, args) => match name.as_str() {
            "len" if args.len() == 1 => sym_len(encode_expr(&args[0], env, ctx)?),
            "List" => {
                let mut values = Vec::with_capacity(args.len());
                for a in args {
                    values.push(encode_expr(a, env, ctx)?);
                }
                Some(Sym::List(values))
            }
            // The Result/Option constructors (Phase 76),
            // building the tag+payload forms — the same
            // shapes sym_call builds on the body side, so a
            // constraint's `Ok(...)` and a body's meet.
            "Ok" if args.len() == 1 => Some(Sym::Result {
                ok: Tag::True,
                ok_payload: Some(Box::new(encode_expr(&args[0], env, ctx)?)),
                err_payload: None,
            }),
            "Err" if args.len() == 1 => Some(Sym::Result {
                ok: Tag::False,
                ok_payload: None,
                err_payload: Some(Box::new(encode_expr(&args[0], env, ctx)?)),
            }),
            "Some" if args.len() == 1 => Some(Sym::Option {
                some: Tag::True,
                payload: Some(Box::new(encode_expr(&args[0], env, ctx)?)),
            }),
            "None" if args.is_empty() => Some(Sym::Option {
                some: Tag::False,
                payload: None,
            }),
            // A call to another spec resolves by inlining the
            // callee's body (see sym_invoke) — this arm is what
            // makes composition postconditions judgable at all;
            // unknown functions have no definition to encode.
            _ => {
                let mut values = Vec::with_capacity(args.len());
                for a in args {
                    values.push(encode_expr(a, env, ctx)?);
                }
                sym_invoke(ctx, name, values)
            }
        },
        Expr::Index(coll, idx) => {
            sym_index(encode_expr(coll, env, ctx)?, encode_expr(idx, env, ctx)?)
        }
        // Field projection (Phase 76, D2a): the base must
        // encode to a record and the field must be among
        // its fields — an unknown field or a non-record
        // base is unencodable, never a guess.
        Expr::Field(base, field) => match encode_expr(base, env, ctx)? {
            Sym::Record { fields, .. } => fields
                .into_iter()
                .find(|(name, _)| name == field)
                .map(|(_, v)| v),
            _ => None,
        },
        // Record construction (Phase 76, D3a): the fields
        // encode in written order; the stored type name is
        // the resolved declaration name when the index
        // carries declarations (see sym_eval's RecordLit).
        Expr::RecordLit { type_name, fields } => {
            let mut values = Vec::with_capacity(fields.len());
            for (name, value) in fields {
                values.push((name.clone(), encode_expr(value, env, ctx)?));
            }
            let type_name = match ctx.index.record_fields(type_name) {
                Some((resolved, _)) => resolved,
                None => type_name.clone(),
            };
            Some(Sym::Record {
                type_name,
                fields: values,
            })
        }
        // Method calls (Phase 76, D1a): the closed builtin
        // table, encoded against the tag+payload forms.
        // Every builtin is zero-argument; an argument, an
        // unknown name, or a receiver of the wrong family
        // is unencodable.
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            if !args.is_empty() {
                return None;
            }
            let recv = encode_expr(receiver, env, ctx)?;
            match (name.as_str(), recv) {
                ("is_ok", Sym::Result { ok, .. }) => Some(Sym::Bool(ok.term())),
                ("is_err", Sym::Result { ok, .. }) => {
                    Some(Sym::Bool(ok.term().not()))
                }
                // unwrap projects the Ok payload. Its
                // constraint uses are the guarded shapes
                // (`is_err() || …unwrap()…`): on the Err
                // branch the disjunction is already
                // satisfied. When the receiver is a literal
                // Err construction the payload slot is
                // absent and there is nothing sound to
                // project — unencodable, named here.
                ("unwrap", Sym::Result { ok_payload, .. }) => {
                    ok_payload.map(|p| *p)
                }
                ("len", v @ (Sym::List(_) | Sym::ListLen(_))) => sym_len(v),
                _ => None,
            }
        }
        // Lambda and comprehension have no encoding here.
        Expr::Lambda(..) | Expr::Comprehension { .. } => None,
    }
}

/// A candidate body is a well-formed implementation only if
/// synthesis is finished with it: a body still containing
/// holes is not an implementation, and there is nothing for
/// the verifier to judge.
fn is_well_formed(node: &IRNode) -> bool {
    crate::synthesis::engine::count_holes(node) == 0
}

// ── Public API ─────────────────────────────────────────────

/// Whether `body` lies in the verifier's encodable subset for
/// `spec`: symbolic evaluation over the spec's parameters as
/// free constants produces a value. This is exactly the
/// precondition `verify_one` needs before any postcondition
/// can come back Proven — a body failing it can only ever be
/// Inconclusive, never Verified. Synthesis consults it when
/// admitting constraint-derived seeds, so a seed is only built
/// from a constraint when the verifier can actually judge the
/// resulting body (an unencodable seed would be one more
/// Inconclusive pool member, indistinguishable from junk).
pub fn body_is_encodable(spec: &FunctionSpec, body: &IRNode) -> bool {
    body_is_encodable_with(spec, body, &SpecIndex::empty())
}

/// `body_is_encodable` in a module context: spec-to-spec
/// calls in the body resolve against `index` while judging
/// encodability, so a body whose calls resolve to encodable
/// callee bodies is itself encodable.
#[cfg(feature = "z3-support")]
pub fn body_is_encodable_with(
    spec: &FunctionSpec,
    body: &IRNode,
    index: &SpecIndex,
) -> bool {
    let mut env = param_env(spec, index);
    sym_eval(body, &mut env, &EvalCtx::root(index)).is_some()
}

/// No-Z3 build (Phase 66): the same question answered by
/// the pure abstract walk in super::encodable — it tracks
/// per subexpression exactly the facts this predicate's
/// answer depends on (sorts, numeral/literal known-ness),
/// and a cross-check test in that module pins the two
/// implementations to agreement on a harvested corpus.
#[cfg(not(feature = "z3-support"))]
pub fn body_is_encodable_with(
    spec: &FunctionSpec,
    body: &IRNode,
    index: &SpecIndex,
) -> bool {
    super::encodable::body_is_encodable_pure(spec, body, index)
}

pub fn verify_all(
    candidates: Vec<CandidateImplementation>,
    spec: &FunctionSpec,
) -> Result<Vec<CandidateImplementation>, VerificationError> {
    Verifier::new().verify_all(candidates, spec)
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::ast::{BinOp as SpecBinOp, Expr as SpecExpr};
    use crate::synthesis::engine::{
        IRBinOp, IRLiteral, IRNode, IRPattern, Provenance, ScoreBreakdown,
    };

    fn load_spec(file: &str, name: &str) -> FunctionSpec {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/self_host/{}.morph", root, file);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = crate::spec::parser::parse(&source).unwrap();
        let checked = crate::spec::typeck::check(parsed).unwrap();
        checked
            .spec
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .clone()
    }

    fn candidate(spec: &FunctionSpec, id: u64, body: IRNode) -> CandidateImplementation {
        CandidateImplementation {
            id,
            body,
            spec_name: spec.name.clone(),
            spec: spec.clone(),
            score: 0.0,
            scores: ScoreBreakdown {
                test_pass_ratio: 0.0,
                constraint_score: 0.0,
                complexity_score: 0.0,
                quality_score: 0.0,
                synthesis_time_us: 0,
            },
            generation: 0,
            provenance: Provenance::HumanGuided,
            verified: false,
            verification_status: None,
        }
    }

    fn int(n: i64) -> IRNode {
        IRNode::Literal(IRLiteral::Int(n))
    }
    fn boolean(b: bool) -> IRNode {
        IRNode::Literal(IRLiteral::Bool(b))
    }
    fn string(s: &str) -> IRNode {
        IRNode::Literal(IRLiteral::String(s.into()))
    }
    fn var(name: &str) -> IRNode {
        IRNode::Var(name.into())
    }
    fn binop(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
        IRNode::BinOp(op, Box::new(l), Box::new(r))
    }
    fn match_on(scrutinee: IRNode, arms: Vec<(IRPattern, IRNode)>) -> IRNode {
        IRNode::Match {
            scrutinee: Box::new(scrutinee),
            cases: arms,
        }
    }
    fn lit_arm(n: i64, body: IRNode) -> (IRPattern, IRNode) {
        (IRPattern::Literal(IRLiteral::Int(n)), body)
    }
    fn wild(body: IRNode) -> (IRPattern, IRNode) {
        (IRPattern::Wildcard, body)
    }

    /// The TestInduced seed completion for is_literal_kind with
    /// the semantically correct wildcard: kind==0 → true, the
    /// two tested non-literal kinds → false, everything else
    /// → false. This body IS `kind == 0` for every input and
    /// measures tpr 1.0.
    fn is_literal_kind_completion() -> IRNode {
        match_on(
            var("kind"),
            vec![
                lit_arm(0, boolean(true)),
                lit_arm(2, boolean(false)),
                lit_arm(4, boolean(false)),
                wild(boolean(false)),
            ],
        )
    }

    fn status_of(spec: &FunctionSpec, body: IRNode) -> VerificationStatus {
        Verifier::new()
            .verify_one(&candidate(spec, 1, body), spec)
            .status
    }

    fn load_module(file: &str) -> Vec<FunctionSpec> {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/self_host/{}.morph", root, file);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = crate::spec::parser::parse(&source).unwrap();
        let checked = crate::spec::typeck::check(parsed).unwrap();
        checked.spec.functions
    }

    fn module_spec(module: &[FunctionSpec], name: &str) -> FunctionSpec {
        module.iter().find(|f| f.name == name).unwrap().clone()
    }

    fn status_of_in_module(
        module: &[FunctionSpec],
        spec: &FunctionSpec,
        body: IRNode,
    ) -> VerificationStatus {
        Verifier::with_specs(module)
            .verify_one(&candidate(spec, 1, body), spec)
            .status
    }

    // ── Spec-to-spec calls ────────────────────────────────

    /// validate_kind_tag's postcondition IS a call (`output ==
    /// is_known_node_kind(kind)`). Without a module the
    /// constraint cannot encode and every candidate is
    /// Inconclusive — the pre-fix ceiling. With the module, the
    /// constraint-side call inlines the callee's range body and
    /// a constant candidate is REFUTED (true fails at kind 77,
    /// false fails at kind 4): the first refutation this spec
    /// can produce.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn composition_wrong_candidate_is_refuted_with_module() {
        let module = load_module("validator");
        let spec = module_spec(&module, "validate_kind_tag");
        assert_eq!(
            status_of(&spec, boolean(true)),
            VerificationStatus::Inconclusive
        );
        assert_eq!(
            status_of_in_module(&module, &spec, boolean(true)),
            VerificationStatus::VerificationFailed
        );
        assert_eq!(
            status_of_in_module(&module, &spec, boolean(false)),
            VerificationStatus::VerificationFailed
        );
    }

    /// Both sides inline: a candidate whose body is the raw
    /// call itself verifies against the calling postcondition —
    /// body side and constraint side resolve the same callee
    /// body and meet.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn composition_call_body_is_verified_with_module() {
        let module = load_module("validator");
        let spec = module_spec(&module, "validate_kind_tag");
        let body = IRNode::Call {
            function: "is_known_node_kind".into(),
            args: vec![var("kind")],
        };
        assert_eq!(
            status_of(&spec, body.clone()),
            VerificationStatus::Inconclusive
        );
        assert_eq!(
            status_of_in_module(&module, &spec, body),
            VerificationStatus::Verified
        );
    }

    /// Body-side inlining evaluates the ACTUAL callee: a body
    /// calling is_literal_kind (kind == 0) does not satisfy
    /// validate_kind_tag (0 <= kind <= 9) — the two disagree at
    /// kind 5, so the candidate is refuted, not merely
    /// unjudged.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn composition_call_to_wrong_callee_is_refuted() {
        let module = load_module("validator");
        let spec = module_spec(&module, "validate_kind_tag");
        let body = IRNode::Call {
            function: "is_literal_kind".into(),
            args: vec![var("kind")],
        };
        assert_eq!(
            status_of_in_module(&module, &spec, body),
            VerificationStatus::VerificationFailed
        );
    }

    /// print_node_kind's callee is a guard-table spec (no
    /// equation): the resolver's guard-table body makes the
    /// composition judgable. The per-test match — right on both
    /// tested kinds, wrong on the untested Literal row — is
    /// refuted; the full lookup verifies.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn composition_guard_table_callee_is_judgable() {
        let module = load_module("printer");
        let spec = module_spec(&module, "print_node_kind");
        let per_test = match_on(
            var("kind"),
            vec![
                lit_arm(2, string("BinaryOp")),
                lit_arm(7, string("Lambda")),
                wild(string("")),
            ],
        );
        assert_eq!(
            status_of_in_module(&module, &spec, per_test),
            VerificationStatus::VerificationFailed
        );
        let lookup = match_on(
            var("kind"),
            vec![
                lit_arm(0, string("Literal")),
                lit_arm(1, string("Variable")),
                lit_arm(2, string("BinaryOp")),
                lit_arm(3, string("UnaryOp")),
                lit_arm(4, string("Call")),
                lit_arm(5, string("Index")),
                lit_arm(6, string("FieldAccess")),
                lit_arm(7, string("Lambda")),
                wild(string("")),
            ],
        );
        assert_eq!(
            status_of_in_module(&module, &spec, lookup),
            VerificationStatus::Verified
        );
    }

    fn eq_spec(name: &str, rhs: SpecExpr) -> FunctionSpec {
        FunctionSpec {
            name: name.into(),
            doc: None,
            generics: vec![],
            params: vec![Param {
                name: "x".into(),
                ty: TypeRef::Int,
                annotations: HashMap::new(),
            }],
            return_type: TypeRef::Bool,
            preconditions: vec![],
            postconditions: vec![Constraint::Expr(SpecExpr::BinOp(
                SpecBinOp::Eq,
                Box::new(SpecExpr::Var("output".into())),
                Box::new(rhs),
            ))],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: HashMap::new(),
        }
    }

    fn call_expr(name: &str) -> SpecExpr {
        SpecExpr::Call(name.into(), vec![SpecExpr::Var("x".into())])
    }

    /// A cyclic pair (a calls b, b calls a) yields Inconclusive
    /// from BOTH sides — a candidate whose body is the cyclic
    /// call (body side cannot resolve it) and a plain candidate
    /// judged against the cyclic postcondition (constraint side
    /// cannot resolve it) — never a hang or a panic. The test
    /// completing at all is the termination evidence.
    #[test]
    fn cyclic_spec_calls_are_inconclusive_not_hanging() {
        let a = eq_spec("cyc_a", call_expr("cyc_b"));
        let b = eq_spec("cyc_b", call_expr("cyc_a"));
        let module = vec![a.clone(), b];
        let call_body = IRNode::Call {
            function: "cyc_b".into(),
            args: vec![var("x")],
        };
        assert_eq!(
            status_of_in_module(&module, &a, call_body),
            VerificationStatus::Inconclusive
        );
        assert_eq!(
            status_of_in_module(&module, &a, boolean(true)),
            VerificationStatus::Inconclusive
        );
    }

    /// Inlining is transitive under a depth cap: a three-spec
    /// chain (top → mid → tail, the tail a plain range)
    /// verifies end to end; a twelve-spec chain exceeds the cap
    /// and stays Inconclusive rather than expanding without
    /// bound.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn spec_call_chains_inline_transitively_under_a_depth_cap() {
        let range = SpecExpr::BinOp(
            SpecBinOp::And,
            Box::new(SpecExpr::BinOp(
                SpecBinOp::Ge,
                Box::new(SpecExpr::Var("x".into())),
                Box::new(SpecExpr::IntLit(0)),
            )),
            Box::new(SpecExpr::BinOp(
                SpecBinOp::Le,
                Box::new(SpecExpr::Var("x".into())),
                Box::new(SpecExpr::IntLit(9)),
            )),
        );
        let tail = eq_spec("chain_tail", range);
        let mid = eq_spec("chain_mid", call_expr("chain_tail"));
        let top = eq_spec("chain_top", call_expr("chain_mid"));
        let module = vec![top.clone(), mid, tail];
        let body = IRNode::Call {
            function: "chain_mid".into(),
            args: vec![var("x")],
        };
        assert_eq!(
            status_of_in_module(&module, &top, body),
            VerificationStatus::Verified
        );

        // Twelve specs, each calling the next, the last a range:
        // resolving the head needs eleven nested inlinings.
        let mut module: Vec<FunctionSpec> = (0..11)
            .map(|i| eq_spec(&format!("deep_{}", i), call_expr(&format!("deep_{}", i + 1))))
            .collect();
        module.push(eq_spec(
            "deep_11",
            SpecExpr::BinOp(
                SpecBinOp::Ge,
                Box::new(SpecExpr::Var("x".into())),
                Box::new(SpecExpr::IntLit(0)),
            ),
        ));
        let head = module[0].clone();
        let body = IRNode::Call {
            function: "deep_1".into(),
            args: vec![var("x")],
        };
        assert_eq!(
            status_of_in_module(&module, &head, body),
            VerificationStatus::Inconclusive
        );
    }

    // ── The decisive pair ─────────────────────────────────

    /// Guard-style spec (node_arity: per-row `kind<a||kind>a||
    /// output==v` constraints). Before candidate encoding, EVERY
    /// candidate "verified" — the Or-rooted formula encoded to
    /// `true` regardless of the body. A body that returns 0 for
    /// every kind violates the kind==3 row (arity 1), so it must
    /// be refuted, not verified.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn wrong_candidate_for_guard_style_spec_is_refuted() {
        let spec = load_spec("validator", "node_arity");
        assert_eq!(
            status_of(&spec, int(0)),
            VerificationStatus::VerificationFailed
        );
    }

    /// The matching correct candidate for the same guard-style
    /// spec must verify: verdicts are candidate-dependent.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn correct_candidate_for_guard_style_spec_is_verified() {
        let spec = load_spec("validator", "node_arity");
        let body = match_on(
            var("kind"),
            vec![
                lit_arm(0, int(0)),
                lit_arm(1, int(0)),
                lit_arm(3, int(1)),
                lit_arm(6, int(1)),
                lit_arm(2, int(2)),
                lit_arm(5, int(2)),
                wild(int(0)),
            ],
        );
        assert_eq!(status_of(&spec, body), VerificationStatus::Verified);
    }

    /// Equation-style spec (is_literal_kind: `output ==
    /// (kind==0)`). Before candidate encoding this NEVER
    /// verified — `output` was a free int variable compared
    /// against 0. The correct seed completion must verify now.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn correct_completion_for_equation_spec_is_verified() {
        let spec = load_spec("validator", "is_literal_kind");
        assert_eq!(
            status_of(&spec, is_literal_kind_completion()),
            VerificationStatus::Verified
        );
        // The direct expression body is the same semantics.
        let expr_body = binop(IRBinOp::Eq, var("kind"), int(0));
        assert_eq!(
            status_of(&spec, expr_body),
            VerificationStatus::Verified
        );
    }

    /// Same spec, semantically wrong candidate (always true):
    /// refuted, with a counterexample.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn wrong_candidate_for_equation_spec_is_refuted() {
        let spec = load_spec("validator", "is_literal_kind");
        let result = Verifier::new().verify_one(
            &candidate(&spec, 1, boolean(true)),
            &spec,
        );
        assert_eq!(result.status, VerificationStatus::VerificationFailed);
        assert!(result.counterexample.is_some());
    }

    /// verify_all must RECORD the verdict it computed on each
    /// candidate (`verification_status`), not collapse it to
    /// the `verified` bool: selection's refutation gate reads
    /// exactly this field. What is recorded must agree with
    /// verify_one on the same body — one source of truth for
    /// the verdict, computed once, in the verifier.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn verify_all_records_status_on_candidates() {
        let spec = load_spec("validator", "node_arity");
        let correct = match_on(
            var("kind"),
            vec![
                lit_arm(0, int(0)),
                lit_arm(1, int(0)),
                lit_arm(3, int(1)),
                lit_arm(6, int(1)),
                lit_arm(2, int(2)),
                lit_arm(5, int(2)),
                wild(int(0)),
            ],
        );
        let bodies = vec![correct, int(0)];
        // The decisive pair: one Verified, one refuted.
        let expected: Vec<VerificationStatus> = bodies
            .iter()
            .map(|b| status_of(&spec, b.clone()))
            .collect();
        assert_eq!(
            expected,
            vec![
                VerificationStatus::Verified,
                VerificationStatus::VerificationFailed
            ]
        );
        let candidates: Vec<CandidateImplementation> = bodies
            .into_iter()
            .enumerate()
            .map(|(i, b)| candidate(&spec, i as u64 + 1, b))
            .collect();
        let out = verify_all(candidates, &spec).expect("verify_all");
        assert_eq!(out.len(), expected.len());
        for (c, want) in out.iter().zip(expected.iter()) {
            assert_eq!(c.verification_status.as_ref(), Some(want));
            assert_eq!(c.verified, *want == VerificationStatus::Verified);
        }
    }

    // ── Honest verdicts outside the subset ────────────────

    /// A body outside the encodable subset (a While loop) must
    /// be Inconclusive — never Verified for a body the verifier
    /// did not actually judge.
    #[test]
    fn unencodable_body_is_inconclusive() {
        let spec = load_spec("validator", "is_literal_kind");
        let body = IRNode::While {
            condition: Box::new(boolean(true)),
            body: Box::new(int(1)),
            invariant: None,
        };
        assert_eq!(
            status_of(&spec, body),
            VerificationStatus::Inconclusive
        );
        // An unknown call is equally unencodable.
        let body = IRNode::Call {
            function: "is_sorted".into(),
            args: vec![var("kind")],
        };
        assert_eq!(
            status_of(&spec, body),
            VerificationStatus::Inconclusive
        );
    }

    /// A body still containing synthesis holes is not a
    /// well-formed implementation at all.
    #[test]
    fn holed_body_is_malformed() {
        let spec = load_spec("validator", "is_literal_kind");
        let body = IRNode::Hole {
            id: 0,
            expected_type: None,
        };
        assert_eq!(status_of(&spec, body), VerificationStatus::Malformed);
    }

    /// validate_kind_tag's constraint calls another spec
    /// (`output == is_known_node_kind(kind)`). That constraint
    /// cannot be encoded, so the honest verdict is Inconclusive
    /// — before this fix it was VerificationFailed (the call
    /// encoded to int 0 and `output` was a free variable).
    #[test]
    fn unencodable_constraint_is_inconclusive() {
        let spec = load_spec("validator", "validate_kind_tag");
        let body = binop(
            IRBinOp::And,
            binop(IRBinOp::Ge, var("kind"), int(0)),
            binop(IRBinOp::Le, var("kind"), int(9)),
        );
        assert_eq!(
            status_of(&spec, body),
            VerificationStatus::Inconclusive
        );
    }

    // ── String outputs ────────────────────────────────────

    /// node_kind_name is a guard-style String lookup. The
    /// correct match completion must verify; a constant wrong
    /// string must be refuted. (Before candidate encoding, both
    /// "verified" — the verdict was pure constraint syntax.)
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn string_lookup_spec_verdicts_follow_the_candidate() {
        let spec = load_spec("printer", "node_kind_name");
        let names = [
            "Literal",
            "Variable",
            "BinaryOp",
            "UnaryOp",
            "Call",
            "Index",
            "FieldAccess",
            "Lambda",
        ];
        let mut arms: Vec<(IRPattern, IRNode)> = names
            .iter()
            .enumerate()
            .map(|(i, n)| lit_arm(i as i64, string(n)))
            .collect();
        arms.push(wild(string("")));
        assert_eq!(
            status_of(&spec, match_on(var("kind"), arms)),
            VerificationStatus::Verified
        );
        assert_eq!(
            status_of(&spec, string("Nope")),
            VerificationStatus::VerificationFailed
        );
    }

    // ── Proof is stronger than tests, honestly ────────────

    /// is_known_type_tag: the exact range expression verifies.
    /// The per-test match completion measures tpr 1.0 (its arms
    /// cover exactly the tested inputs) but is NOT the range
    /// predicate — Z3 must refute it (e.g. tag = 5 is in range
    /// but falls into the false wildcard). This is the honest
    /// tests-vs-proof gap, pinned.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn range_spec_distinguishes_proof_from_test_coverage() {
        let spec = load_spec("validator", "is_known_type_tag");
        let exact = binop(
            IRBinOp::And,
            binop(IRBinOp::Ge, var("tag"), int(0)),
            binop(IRBinOp::Le, var("tag"), int(8)),
        );
        assert_eq!(status_of(&spec, exact), VerificationStatus::Verified);

        let test_shaped = match_on(
            var("tag"),
            vec![
                lit_arm(0, boolean(true)),
                lit_arm(1, boolean(true)),
                lit_arm(8, boolean(true)),
                lit_arm(9, boolean(false)),
                lit_arm(42, boolean(false)),
                wild(boolean(false)),
            ],
        );
        let result = Verifier::new().verify_one(&candidate(&spec, 1, test_shaped), &spec);
        assert_eq!(result.status, VerificationStatus::VerificationFailed);
        assert!(result.counterexample.is_some());
    }

    /// Multi-param spec (needs_parens): the constraint's exact
    /// boolean expression as a body must verify.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn multi_param_equation_spec_verifies_exact_body() {
        let spec = load_spec("printer", "needs_parens");
        let body = binop(
            IRBinOp::And,
            binop(
                IRBinOp::Or,
                binop(IRBinOp::Eq, var("op"), int(0)),
                binop(IRBinOp::Eq, var("op"), int(1)),
            ),
            binop(
                IRBinOp::Or,
                binop(IRBinOp::Eq, var("parent_op"), int(2)),
                binop(IRBinOp::Eq, var("parent_op"), int(3)),
            ),
        );
        assert_eq!(status_of(&spec, body), VerificationStatus::Verified);
        // And a constant body must not.
        assert_eq!(
            status_of(&spec, boolean(true)),
            VerificationStatus::VerificationFailed
        );
    }

    // ── verify_all integration ────────────────────────────

    /// verify_all must mark only the semantically correct
    /// candidate verified, and the constraint score must be the
    /// fraction of postconditions proven FOR THAT CANDIDATE.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn verify_all_marks_only_the_correct_candidate() {
        let spec = load_spec("validator", "is_literal_kind");
        let wrong = candidate(&spec, 1, boolean(true));
        let correct = candidate(&spec, 2, is_literal_kind_completion());
        let out = Verifier::new()
            .verify_all(vec![wrong, correct], &spec)
            .expect("verify_all");
        assert!(!out[0].verified);
        assert!(out[1].verified);
        assert_eq!(out[0].scores.constraint_score, 0.0);
        assert_eq!(out[1].scores.constraint_score, 1.0);
    }

    /// The spec-side expression encoder handles the constraint
    /// shapes the parser produces (guards against regressions
    /// in encode coverage for Or/Ge/Neq forms).
    #[test]
    fn spec_constraint_shapes_encode() {
        // output >= 0 as a standalone postcondition (node_arity's
        // last constraint) is an Expr(Ge) — exercised end-to-end
        // by the node_arity tests above. Here: a != constraint.
        let spec = FunctionSpec {
            name: "neq_spec".into(),
            doc: None,
            generics: vec![],
            params: vec![crate::spec::ast::Param {
                name: "x".into(),
                ty: crate::spec::ast::TypeRef::Int,
                annotations: Default::default(),
            }],
            return_type: crate::spec::ast::TypeRef::Bool,
            preconditions: vec![],
            postconditions: vec![Constraint::Expr(SpecExpr::BinOp(
                SpecBinOp::Neq,
                Box::new(SpecExpr::Var("output".into())),
                Box::new(SpecExpr::Var("x".into())),
            ))],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: Default::default(),
        };
        // Body `true`: output != x is not implied (x could make
        // the bool compare unequal sorts) — output is Bool, x is
        // Int, so this constraint is unencodable: Inconclusive.
        assert_eq!(
            status_of(&spec, boolean(true)),
            VerificationStatus::Inconclusive
        );
    }

    // ── Phase 76 evidence: the restored tokenizer module ──
    //
    // The same restored module text the interpreter tests
    // execute (eval.rs RESTORED_TOKENIZER_MODULE, Phase
    // 75), judged here by the verifier with hand bodies
    // transcribed from the hand-written mirror — the
    // bodies Phase 75 executed. Per-spec verdicts are the
    // phase's evidence table (the M1 gate re-run is Phase
    // 80's): the classifiers and peek verify outright;
    // skip_whitespace's identity body verifies;
    // tokenize_all's weak postcondition verifies;
    // tokenize_one's postconditions encode and REFUTE the
    // partial bodies (no verifying body exists short of a
    // real tokenizing loop); next_char's body encodes but
    // its postconditions call peek, for which the resolver
    // denotes no body — the named reason it stays
    // Inconclusive.

    const RESTORED_TOKENIZER_MODULE: &str = r#"
ScratchTokenizer {

type TokenKind = Int;

type Token = {
    kind: TokenKind,
    lexeme: String,
    line: Int,
    col: Int
}

type TokenizerState = {
    source: String,
    pos: Int,
    line: Int,
    col: Int,
    tokens: List<Token>
}

type StepResult = Result<TokenizerState, String>;

spec peek {
    input: state: TokenizerState
    output: Option<Char>

    constraint: state.pos<len(state.source)||output==None()
    constraint: state.pos>=len(state.source)||output==Some(state.source[state.pos])

    test "non-empty" { input: TokenizerState { source: "hi", pos: 0, line: 1, col: 1, tokens: [] } expect: Some('h') }
    test "at-end" { input: TokenizerState { source: "", pos: 0, line: 1, col: 1, tokens: [] } expect: None() }
}

spec next_char {
    input: state: TokenizerState
    output: TokenizerState

    constraint: output.pos==state.pos+1
    constraint: peek(state)!=Some('\n')||output.line==state.line+1
    constraint: peek(state)!=Some('\n')||output.col==1
    constraint: peek(state)==Some('\n')||output.col==state.col+1

    test "normal" {
        input: TokenizerState { source: "ab", pos: 0, line: 1, col: 1, tokens: [] };
        expect: TokenizerState { source: "ab", pos: 1, line: 1, col: 2, tokens: [] }
    }
}

spec skip_whitespace {
    input: state: TokenizerState
    output: TokenizerState

    constraint: output.pos>=state.pos
    constraint: len(output.source)==len(state.source)

    test "no-whitespace" {
        input: TokenizerState { source: "hi", pos: 0, line: 1, col: 1, tokens: [] };
        expect: TokenizerState { source: "hi", pos: 0, line: 1, col: 1, tokens: [] }
    }

    test "skip-spaces" {
        input: TokenizerState { source: "  hi", pos: 0, line: 1, col: 1, tokens: [] };
        expect: TokenizerState { source: "  hi", pos: 2, line: 1, col: 3, tokens: [] }
    }
}

spec tokenize_one {
    input: state: TokenizerState
    output: StepResult

    constraint: output.is_err() || len(output.unwrap().tokens) == len(state.tokens) + 1
    constraint: output.is_ok() || state.pos >= len(state.source)

    test "keyword-spec" {
        input: TokenizerState { source: "spec", pos: 0, line: 1, col: 1, tokens: [] };
        expect: Ok(TokenizerState { source: "spec", pos: 4, line: 1, col: 5, tokens: [
            Token { kind: 0, lexeme: "spec", line: 1, col: 1 }
        ] })
    }

    test "end-of-input" {
        input: TokenizerState { source: "", pos: 0, line: 1, col: 1, tokens: [] };
        expect: Ok(TokenizerState { source: "", pos: 0, line: 1, col: 1, tokens: [] })
    }
}

spec tokenize_all {
    input: state: TokenizerState
    output: Result<List<Token>, String>

    constraint: output.is_err() || output.unwrap().len() >= len(state.tokens)
    optimize: time < O(n)

    test "simple-spec" {
        input: TokenizerState { source: "spec sort", pos: 0, line: 1, col: 1, tokens: [] };
        expect: Ok([
            Token { kind: 0, lexeme: "spec", line: 1, col: 1 },
            Token { kind: 0, lexeme: "sort", line: 1, col: 6 }
        ])
    }

    test "empty" {
        input: TokenizerState { source: "", pos: 0, line: 1, col: 1, tokens: [] };
        expect: Ok([])
    }
}

}"#;

    #[cfg(feature = "z3-support")]
    fn load_restored_module() -> Spec {
        let parsed = crate::spec::parser::parse(RESTORED_TOKENIZER_MODULE)
            .expect("restored module parses");
        crate::spec::typeck::check(parsed)
            .expect("restored module typechecks")
            .spec
    }

    #[cfg(feature = "z3-support")]
    fn restored_spec<'m>(module: &'m Spec, name: &str) -> &'m FunctionSpec {
        module
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("restored module has spec {}", name))
    }

    /// The hand-written mirror's peek, as an IR body (the
    /// same transcription eval.rs executes in Phase 75).
    #[cfg(feature = "z3-support")]
    fn peek_mirror_body() -> IRNode {
        IRNode::If {
            condition: Box::new(IRNode::BinOp(
                IRBinOp::Lt,
                Box::new(IRNode::FieldAccess {
                    base: Box::new(IRNode::Var("state".into())),
                    field: "pos".into(),
                }),
                Box::new(IRNode::Call {
                    function: "len".into(),
                    args: vec![IRNode::FieldAccess {
                        base: Box::new(IRNode::Var("state".into())),
                        field: "source".into(),
                    }],
                }),
            )),
            then_branch: Box::new(IRNode::Call {
                function: "Some".into(),
                args: vec![IRNode::Index(
                    Box::new(IRNode::FieldAccess {
                        base: Box::new(IRNode::Var("state".into())),
                        field: "source".into(),
                    }),
                    Box::new(IRNode::FieldAccess {
                        base: Box::new(IRNode::Var("state".into())),
                        field: "pos".into(),
                    }),
                )],
            }),
            else_branch: Box::new(IRNode::Call {
                function: "None".into(),
                args: vec![],
            }),
        }
    }

    /// The mirror's next_char, as an IR body (a RecordLit
    /// over field projections — Phase 75's transcription).
    #[cfg(feature = "z3-support")]
    fn next_char_mirror_body() -> IRNode {
        let field = |name: &str| IRNode::FieldAccess {
            base: Box::new(IRNode::Var("state".into())),
            field: name.into(),
        };
        IRNode::RecordLit {
            type_name: "TokenizerState".into(),
            fields: vec![
                ("source".into(), field("source")),
                (
                    "pos".into(),
                    IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(field("pos")),
                        Box::new(IRNode::Literal(IRLiteral::Int(1))),
                    ),
                ),
                ("line".into(), field("line")),
                ("col".into(), field("col")),
                ("tokens".into(), field("tokens")),
            ],
        }
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase76_classifiers_verify_and_wrong_bodies_refute() {
        // The classifiers live in the standing examples
        // module (the restored scratch declares only the
        // five state specs) — same constraints, same
        // resolver bodies.
        let examples = {
            let src = std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/functional_tokenizer.morph"
            ))
            .expect("examples tokenizer reads");
            let parsed = crate::spec::parser::parse(&src).expect("examples parses");
            crate::spec::typeck::check(parsed)
                .expect("examples typechecks")
                .spec
        };
        let ex_index = SpecIndex::from_module(&examples);
        let ex_verifier = Verifier::with_module(&examples);
        for name in ["is_whitespace", "is_ident_start", "is_ident_continue"] {
            let spec = restored_spec(&examples, name);
            let exact = ex_index
                .callee_body(name)
                .unwrap_or_else(|| panic!("{}: resolver body", name));
            let good = candidate(&spec, 76, exact);
            assert_eq!(
                ex_verifier.verify_one(&good, &spec).status,
                VerificationStatus::Verified,
                "{}: the classifier body must verify after D5a",
                name
            );
            let wrong = candidate(&spec, 76, IRNode::Literal(IRLiteral::Bool(true)));
            assert_eq!(
                ex_verifier.verify_one(&wrong, &spec).status,
                VerificationStatus::VerificationFailed,
                "{}: the constant-true body must be refuted after D5a",
                name
            );
        }
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase76_peek_mirror_body_is_verified() {
        let module = load_restored_module();
        let spec = restored_spec(&module, "peek");
        let verifier = Verifier::with_module(&module);
        let good = candidate(&spec, 76, peek_mirror_body());
        assert_eq!(
            verifier.verify_one(&good, &spec).status,
            VerificationStatus::Verified,
            "peek's mirror body must verify: record fields, the \
             string-index bridge and Option equality all encode"
        );
    }

    /// The mirror's next_char, FULL body (Phase 81): the
    /// Phase 76 transcription above carries line/col
    /// unchanged, which the spec's rows refute at
    /// non-newline inputs — it was an encodability probe,
    /// never a verifying body. The full mirror branches
    /// line/col on `peek(state) == Some('\n')`, stated as an
    /// IR Call to peek (the form a candidate body carries;
    /// the verifier inlines it through the resolver).
    #[cfg(feature = "z3-support")]
    fn next_char_full_mirror_body() -> IRNode {
        let field = |name: &str| IRNode::FieldAccess {
            base: Box::new(IRNode::Var("state".into())),
            field: name.into(),
        };
        let peek_is_newline = || {
            IRNode::BinOp(
                IRBinOp::Eq,
                Box::new(IRNode::Call {
                    function: "peek".into(),
                    args: vec![IRNode::Var("state".into())],
                }),
                Box::new(IRNode::Call {
                    function: "Some".into(),
                    args: vec![IRNode::Literal(IRLiteral::Char('\n'))],
                }),
            )
        };
        IRNode::RecordLit {
            type_name: "TokenizerState".into(),
            fields: vec![
                ("source".into(), field("source")),
                (
                    "pos".into(),
                    IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(field("pos")),
                        Box::new(IRNode::Literal(IRLiteral::Int(1))),
                    ),
                ),
                (
                    "line".into(),
                    IRNode::If {
                        condition: Box::new(peek_is_newline()),
                        then_branch: Box::new(IRNode::BinOp(
                            IRBinOp::Add,
                            Box::new(field("line")),
                            Box::new(IRNode::Literal(IRLiteral::Int(1))),
                        )),
                        else_branch: Box::new(field("line")),
                    },
                ),
                (
                    "col".into(),
                    IRNode::If {
                        condition: Box::new(peek_is_newline()),
                        then_branch: Box::new(IRNode::Literal(IRLiteral::Int(1))),
                        else_branch: Box::new(IRNode::BinOp(
                            IRBinOp::Add,
                            Box::new(field("col")),
                            Box::new(IRNode::Literal(IRLiteral::Int(1))),
                        )),
                    },
                ),
                ("tokens".into(), field("tokens")),
            ],
        }
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase81_next_char_verifies_once_peek_denotes() {
        // RE-STATED from Phase 76's
        // phase76_next_char_body_encodes_but_peek_call_blocks_the_verdict,
        // whose pinned premise — the resolver denotes no
        // body for peek — is exactly what Phase 81 removes
        // (the complementary-guard builder, remainder (i)).
        // The encodability half of the old test is unchanged;
        // the blocker assertions are replaced by the closure
        // they predicted.
        let module = load_restored_module();
        let index = SpecIndex::from_module(&module);
        let spec = restored_spec(&module, "next_char");
        assert!(
            body_is_encodable_with(&spec, &next_char_mirror_body(), &index),
            "next_char's mirror body must be encodable after Phase 76"
        );
        // peek now denotes, through the complementary-guard
        // builder appended at the END of the resolver chain.
        let peek_body = index
            .callee_body("peek")
            .expect("peek must denote after Phase 81");
        // Self-check (the Phase 80 pricing's added evidence
        // for this builder): the denoted body verifies
        // against peek's OWN postconditions.
        let peek_spec = restored_spec(&module, "peek");
        let verifier = Verifier::with_module(&module);
        let cand = candidate(&peek_spec, 81, peek_body);
        assert_eq!(
            verifier.verify_one(&cand, &peek_spec).status,
            VerificationStatus::Verified,
            "peek's denoted body must verify against peek's own postconditions"
        );
        // And next_char's full mirror body — whose line/col
        // logic calls peek(state) — now verifies with peek
        // inlined through the denotation.
        let cand = candidate(&spec, 81, next_char_full_mirror_body());
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::Verified,
            "next_char's full mirror body must verify once peek denotes"
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase76_skip_whitespace_and_tokenize_all_verify() {
        let module = load_restored_module();
        let verifier = Verifier::with_module(&module);
        // skip_whitespace's two stated postconditions are
        // pos-monotonicity and source-length preservation;
        // the identity body satisfies both, provably.
        let spec = restored_spec(&module, "skip_whitespace");
        let identity = candidate(&spec, 76, IRNode::Var("state".into()));
        assert_eq!(
            verifier.verify_one(&identity, &spec).status,
            VerificationStatus::Verified,
            "skip_whitespace identity body must verify"
        );
        // tokenize_all's single postcondition is the weak
        // length bound; Ok(state.tokens) satisfies it.
        let spec = restored_spec(&module, "tokenize_all");
        let body = IRNode::Call {
            function: "Ok".into(),
            args: vec![IRNode::FieldAccess {
                base: Box::new(IRNode::Var("state".into())),
                field: "tokens".into(),
            }],
        };
        let cand = candidate(&spec, 76, body);
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::Verified,
            "tokenize_all's Ok(state.tokens) body must verify"
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase76_tokenize_one_constraints_encode_and_refute_partial_bodies() {
        let module = load_restored_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let spec = restored_spec(&module, "tokenize_one");
        // Ok(state): the is_err/is_ok/unwrap/len machinery
        // encodes, and the first postcondition REFUTES this
        // body (the token list does not grow) — the
        // constraints bite; no partial body verifies, and
        // none exists short of a real tokenizing loop.
        let ok_state = IRNode::Call {
            function: "Ok".into(),
            args: vec![IRNode::Var("state".into())],
        };
        assert!(
            body_is_encodable_with(&spec, &ok_state, &index),
            "the Ok(state) body must be encodable"
        );
        let cand = candidate(&spec, 76, ok_state);
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::VerificationFailed,
            "tokenize_one must refute Ok(state): tokens do not grow"
        );
        // Err(_): the second postcondition refutes it (an
        // error is only licensed at end of input).
        let err = IRNode::Call {
            function: "Err".into(),
            args: vec![IRNode::Literal(IRLiteral::String("boom".into()))],
        };
        let cand = candidate(&spec, 76, err);
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::VerificationFailed,
            "tokenize_one must refute an unconditional Err body"
        );
    }

    /// Phase 84 — RE-STATED from Phase 82's
    /// `phase82_tokenize_one_eof_admitted_amendment_a`,
    /// whose pinned position (amendment A: row 1's
    /// disjunct reads the INPUT's pos; row 2 unamended)
    /// is superseded by the owner-ratified round-2
    /// amendment (ratified 2026-11-22; Phase 83's option
    /// 1, applied in this phase): A′ moves row 1's
    /// end-of-input disjunct to the OUTPUT state —
    /// `|| output.unwrap().pos >= len(output.unwrap().source)`
    /// — and R2-α (callee form) gives row 2 an Err
    /// license for a non-token-start character at the
    /// input's current position. The point machinery
    /// below is Phase 80's, unchanged; what moves is
    /// exactly what the round-2 rows move:
    ///
    ///   * (A) row 1 still PROVES under the golden EOF
    ///     output, now through the output-side disjunct:
    ///     the golden output's own pos 0 >= len("") = 0,
    ///     so the row reads `false || 0 == 0 + 1 || true`
    ///     with the truth on the output side. Row 2's
    ///     verdict at this literal point becomes
    ///     UNKNOWN — the literal-OOB artifact Phase 83
    ///     named: row 2 now mentions
    ///     `state.source[state.pos]`, and a constant
    ///     out-of-bounds index into the literal empty
    ///     string is what sym_index refuses. The
    ///     admission itself is proven on the symbolic
    ///     path in phase84_symbolic_state_encodability.
    ///   * (B) the literal Err at EOF moves the same
    ///     way (Proven under A → Unknown at the literal
    ///     point), discharged symbolically in the same
    ///     companion test.
    ///   * (C) the free-tag analysis of row 1 at the
    ///     point is SAT in every case, as under A — the
    ///     reasoning moves sides: the free payload IS
    ///     the EOF state, so the output-side disjunct
    ///     (0 >= 0) is true of it. The (L=2 ∧ is_ok)
    ///     case remains the rows' named EOF residue:
    ///     at end-of-input inputs the rows constrain
    ///     nothing further, and the golden tests are
    ///     the only judge at EOF.
    ///   * (D) teeth at the "keyword-spec" point,
    ///     unchanged in verdict: a mid-input no-growth
    ///     Ok is REFUTED by row 1 (the output's pos
    ///     0 >= 4 is false on the output side now), and
    ///     the mirror's one-token output is Proven.
    ///
    /// The token lists are bound as ListLen(n): the rows
    /// observe them only through len, and ListLen is the
    /// encoder's own model of a list observed through
    /// len.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase84_tokenize_one_eof_admitted_round2() {
        use crate::verify::smt::{SmtResult, Z3Session};

        // The REAL completed module (examples/), not the
        // Phase 76 scratch: the gate judges this file, and
        // the round-2 amendment lives in it. The two row
        // lines are pinned textually first — the needles
        // are the full lines, so the header's prose quotes
        // cannot match; each occurs exactly once.
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/functional_tokenizer.morph"
        ))
        .expect("examples tokenizer reads");
        let row1_line = "    constraint: output.is_err() || len(output.unwrap().tokens) == len(state.tokens) + 1 || output.unwrap().pos >= len(output.unwrap().source)";
        let row2_line = "    constraint: output.is_ok() || state.pos >= len(state.source) || (is_ident_start(state.source[state.pos]) == false && is_whitespace(state.source[state.pos]) == false && state.source[state.pos] != ':' && state.source[state.pos] != '{' && state.source[state.pos] != '}' && state.source[state.pos] != '(' && state.source[state.pos] != ')' && state.source[state.pos] != ',' && state.source[state.pos] != '<' && state.source[state.pos] != '>' && state.source[state.pos] != '=')";
        assert_eq!(
            src.matches(row1_line).count(),
            1,
            "round-2 row 1 (A′) is the file's row 1"
        );
        assert_eq!(
            src.matches(row2_line).count(),
            1,
            "round-2 row 2 (R2-α, callee form) is the file's row 2"
        );
        let module = {
            let parsed = crate::spec::parser::parse(&src).expect("examples parses");
            crate::spec::typeck::check(parsed)
                .expect("examples typechecks")
                .spec
        };
        let spec = restored_spec(&module, "tokenize_one");
        assert_eq!(spec.postconditions.len(), 2, "tokenize_one has two rows");
        assert!(
            format!("{:?}", spec.postconditions[0]).contains("is_err"),
            "row 1 is the is_err row"
        );
        assert!(
            format!("{:?}", spec.postconditions[1]).contains("is_ok"),
            "row 2 is the is_ok row"
        );
        let verifier = Verifier::with_module(&module);
        let index = SpecIndex::from_module(&module);

        // ── The golden fact, read off the spec's own
        // test: the EOF input and the Ok/0-token output
        // it demands.
        let test = spec
            .tests
            .iter()
            .find(|t| t.name.as_deref() == Some("end-of-input"))
            .expect("the end-of-input test is in-spec");
        fn rec_fields(e: &SpecExpr) -> &Vec<(String, SpecExpr)> {
            match e {
                SpecExpr::RecordLit { type_name, fields } if type_name == "TokenizerState" => {
                    fields
                }
                other => panic!("expected a TokenizerState literal, got {:?}", other),
            }
        }
        fn field<'f>(fields: &'f [(String, SpecExpr)], name: &str) -> &'f SpecExpr {
            fields
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, e)| e)
                .unwrap_or_else(|| panic!("field {} present", name))
        }
        let is_empty_list = |e: &SpecExpr| {
            matches!(e, SpecExpr::Call(n, a) if n == "List" && a.is_empty())
        };
        let in_fields = rec_fields(&test.input);
        assert!(
            matches!(field(in_fields, "source"), SpecExpr::StringLit(s) if s.is_empty()),
            "EOF input: empty source"
        );
        assert!(
            matches!(field(in_fields, "pos"), SpecExpr::IntLit(0)),
            "EOF input: pos 0"
        );
        assert!(
            is_empty_list(field(in_fields, "tokens")),
            "EOF input: no tokens yet"
        );
        let (ctor, ctor_args) = match &test.expected_output {
            SpecExpr::Call(n, a) => (n.as_str(), a),
            other => panic!("expected output is a constructor call, got {:?}", other),
        };
        assert_eq!(ctor, "Ok", "the golden EOF output is Ok");
        assert_eq!(ctor_args.len(), 1);
        assert!(
            is_empty_list(field(rec_fields(&ctor_args[0]), "tokens")),
            "the golden EOF output carries NO token"
        );

        // ── Point instantiation. state := a point state;
        // outputs observed through exactly what the rows
        // read (tag; payload token-list length).
        let int = |n: i64| Sym::Int(z3::ast::Int::from_i64(n));
        let state_of = |source: &str, tok_len: i64| Sym::Record {
            type_name: "TokenizerState".into(),
            fields: vec![
                (
                    "source".into(),
                    Sym::Str(z3::ast::String::from_str(source).unwrap()),
                ),
                ("pos".into(), int(0)),
                ("line".into(), int(1)),
                ("col".into(), int(1)),
                (
                    "tokens".into(),
                    Sym::ListLen(z3::ast::Int::from_i64(tok_len)),
                ),
            ],
        };
        let ok_of = |st: Sym| Sym::Result {
            ok: Tag::True,
            ok_payload: Some(Box::new(st)),
            err_payload: None,
        };
        let mut env = SymEnv::new();
        env.insert("state".into(), state_of("", 0));

        // (A) The golden output: row 1 PROVEN — under A′
        // the truth is the OUTPUT-side disjunct (the
        // golden output's own pos 0 >= len("") = 0).
        // Row 2 at this literal point is UNKNOWN: the α
        // license mentions state.source[state.pos], and
        // sym_index refuses the constant out-of-bounds
        // index into the literal empty string (Phase
        // 83's named artifact); the admission is proven
        // symbolically in
        // phase84_symbolic_state_encodability.
        let golden = ok_of(state_of("", 0));
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[0],
            "row1@golden-eof",
            spec,
            &env,
            &Some(golden.clone()),
        );
        assert!(
            matches!(tri, Tri::Proven),
            "round-2 row 1 must ADMIT the golden EOF output: \
             false || 0 == 0 + 1 || (output pos 0 >= len 0)"
        );
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[1],
            "row2@golden-eof",
            spec,
            &env,
            &Some(golden),
        );
        assert!(
            matches!(tri, Tri::Unknown),
            "row 2 at the literal EOF point is the OOB-index artifact \
             (its admission is proven on the symbolic path)"
        );

        // (B) A literal Err output at EOF: under
        // amendment A row 2 was Proven here (0 >= 0);
        // under R2-α the literal point is the same
        // OOB-index artifact as in (A) — Unknown — and
        // the license (the row's own EOF disjunct) is
        // proven on the symbolic path in the companion
        // test.
        let err = Sym::Result {
            ok: Tag::False,
            ok_payload: None,
            err_payload: Some(Box::new(
                Sym::Str(z3::ast::String::from_str("boom").unwrap()),
            )),
        };
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[1],
            "row2@err-eof",
            spec,
            &env,
            &Some(err),
        );
        assert!(
            matches!(tri, Tri::Unknown),
            "row 2 at the literal EOF point is the OOB-index artifact \
             for an Err output too (symbolic-path admission in the \
             companion test)"
        );

        // (C) Round-2 row 1's satisfying set at EOF, by
        // tag case analysis over a free tag: the free
        // payload below IS the EOF state, so A′'s
        // output-side disjunct (its pos 0 >= len 0) is
        // true of it and every case is SAT — including
        // the two whose UNSAT was Phase 80's
        // contradiction.
        let row1_free = |tok_len: i64, is_ok: &z3::ast::Bool| -> z3::ast::Bool {
            let mut e = env.clone();
            e.insert(
                "output".into(),
                Sym::Result {
                    ok: Tag::Free(is_ok.clone()),
                    ok_payload: Some(Box::new(state_of("", tok_len))),
                    err_payload: Some(Box::new(
                        Sym::Str(z3::ast::String::from_str("boom").unwrap()),
                    )),
                },
            );
            encode_constraint(&spec.postconditions[0], &e, &EvalCtx::root(&index))
                .expect("amended row 1 encodes under a free tag")
        };
        let check = |formula: &z3::ast::Bool, extra: &z3::ast::Bool| -> SmtResult {
            let z3 = Z3Session::new(10_000);
            z3.assert(formula);
            z3.assert(extra);
            z3.check()
        };
        let is_ok = z3::ast::Bool::new_const("out_is_ok");
        assert!(
            matches!(
                check(&row1_free(0, &is_ok), &is_ok),
                SmtResult::Sat(_)
            ),
            "row 1 ∧ is_ok ∧ 0 tokens: SAT — the golden output is admitted \
             (Phase 80: UNSAT, the contradiction)"
        );
        assert!(
            matches!(
                check(&row1_free(0, &is_ok), &is_ok.not()),
                SmtResult::Sat(_)
            ),
            "row 1 ∧ is_err: SAT — an Err output satisfies row 1 at EOF"
        );
        assert!(
            matches!(
                check(&row1_free(1, &is_ok), &is_ok),
                SmtResult::Sat(_)
            ),
            "row 1 ∧ is_ok ∧ 1 token: SAT — growth still satisfies row 1"
        );
        assert!(
            matches!(
                check(&row1_free(2, &is_ok), &is_ok),
                SmtResult::Sat(_)
            ),
            "row 1 ∧ is_ok ∧ 2 tokens: SAT — at EOF inputs the amended row \
             constrains nothing further (Phase 80: UNSAT, growth was exactly +1)"
        );

        // (D) Teeth, at the "keyword-spec" point (source
        // "spec", pos 0, no tokens): the no-growth
        // output's own pos is 0 and its source length 4,
        // so A′'s output-side disjunct is false there
        // (0 >= 4) and the row bites exactly as before.
        let mut env = SymEnv::new();
        env.insert("state".into(), state_of("spec", 0));
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[0],
            "row1@spec-nogrowth",
            spec,
            &env,
            &Some(ok_of(state_of("spec", 0))),
        );
        assert!(
            matches!(tri, Tri::Refuted),
            "round-2 row 1 must still refute a mid-input no-growth Ok: \
             false || 0 == 0 + 1 || (output pos 0 >= 4) is false"
        );
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[0],
            "row1@spec-growth",
            spec,
            &env,
            &Some(ok_of(state_of("spec", 1))),
        );
        assert!(
            matches!(tri, Tri::Proven),
            "round-2 row 1 proves for the mirror's one-token output mid-input"
        );
    }

    /// The hand-written mirror's tokenize_one, as an IR
    /// body (structural transcription of
    /// mirror_tokenize_one in
    /// src/tokenizer_cross_validation.rs): skip, then the
    /// peek dispatch — EOF, the ident arm with its
    /// consumption loop, the nine single-char arms, the
    /// error arm. The lexeme accumulation inside the loop
    /// is the one approximate joint (a Char pushed onto a
    /// String, written as a Concat of the indexed char);
    /// it sits inside the While, which decides the body's
    /// encodability on its own.
    #[cfg(feature = "z3-support")]
    fn tokenize_one_mirror_body() -> IRNode {
        let var = |n: &str| IRNode::Var(n.into());
        let field = |base: IRNode, name: &str| IRNode::FieldAccess {
            base: Box::new(base),
            field: name.into(),
        };
        let call = |f: &str, args: Vec<IRNode>| IRNode::Call {
            function: f.into(),
            args,
        };
        let peek_is_none = |s: IRNode| {
            IRNode::BinOp(
                IRBinOp::Eq,
                Box::new(call("peek", vec![s])),
                Box::new(call("None", vec![])),
            )
        };
        let peeked = |s: IRNode| {
            IRNode::Index(
                Box::new(field(s.clone(), "source")),
                Box::new(field(s, "pos")),
            )
        };
        // push_token(start, end, kind, lexeme): the end
        // state with one Token appended, the token stamped
        // with the START state's line/col (D4a).
        let push_token = |start: IRNode, end: IRNode, kind: i64, lexeme: IRNode| {
            let tok = IRNode::RecordLit {
                type_name: "Token".into(),
                fields: vec![
                    ("kind".into(), IRNode::Literal(IRLiteral::Int(kind))),
                    ("lexeme".into(), lexeme),
                    ("line".into(), field(start.clone(), "line")),
                    ("col".into(), field(start, "col")),
                ],
            };
            IRNode::RecordLit {
                type_name: "TokenizerState".into(),
                fields: vec![
                    ("source".into(), field(end.clone(), "source")),
                    ("pos".into(), field(end.clone(), "pos")),
                    ("line".into(), field(end.clone(), "line")),
                    ("col".into(), field(end.clone(), "col")),
                    (
                        "tokens".into(),
                        IRNode::BinOp(
                            IRBinOp::Concat,
                            Box::new(field(end, "tokens")),
                            Box::new(IRNode::Collection {
                                kind: crate::synthesis::engine::CollectionKind::List,
                                elements: vec![tok],
                            }),
                        ),
                    ),
                ],
            }
        };
        let single = |kind: i64, ch: char| {
            call(
                "Ok",
                vec![push_token(
                    var("s1"),
                    call("next_char", vec![var("s1")]),
                    kind,
                    IRNode::Literal(IRLiteral::String(ch.to_string())),
                )],
            )
        };
        // The nine single-char arms, innermost last arm
        // first; the fall-through is the error arm.
        let mut dispatch = call(
            "Err",
            vec![IRNode::Literal(IRLiteral::String(
                "Unexpected character".into(),
            ))],
        );
        for (ch, kind) in [
            ('=', 12),
            ('>', 11),
            ('<', 10),
            (',', 9),
            (')', 4),
            ('(', 3),
            ('}', 6),
            ('{', 5),
            (':', 7),
        ] {
            dispatch = IRNode::If {
                condition: Box::new(IRNode::BinOp(
                    IRBinOp::Eq,
                    Box::new(peeked(var("s1"))),
                    Box::new(IRNode::Literal(IRLiteral::Char(ch))),
                )),
                then_branch: Box::new(single(kind, ch)),
                else_branch: Box::new(dispatch),
            };
        }
        // The ident arm: consume while is_ident_continue,
        // accumulating the lexeme, then push one Ident
        // token. The lexeme push (a Char onto a String) is
        // written as a Concat of the indexed char — the one
        // approximate joint, inside the While.
        let ident_arm = IRNode::Let {
            name: "s".into(),
            value: Box::new(var("s1")),
            body: Box::new(IRNode::Let {
                name: "lex".into(),
                value: Box::new(IRNode::Literal(IRLiteral::String(String::new()))),
                body: Box::new(IRNode::Block(vec![
                    IRNode::While {
                        condition: Box::new(IRNode::BinOp(
                            IRBinOp::And,
                            Box::new(IRNode::BinOp(
                                IRBinOp::Neq,
                                Box::new(call("peek", vec![var("s")])),
                                Box::new(call("None", vec![])),
                            )),
                            Box::new(call(
                                "is_ident_continue",
                                vec![peeked(var("s"))],
                            )),
                        )),
                        body: Box::new(IRNode::Block(vec![
                            IRNode::Assign {
                                target: Box::new(var("lex")),
                                value: Box::new(IRNode::BinOp(
                                    IRBinOp::Concat,
                                    Box::new(var("lex")),
                                    Box::new(peeked(var("s"))),
                                )),
                            },
                            IRNode::Assign {
                                target: Box::new(var("s")),
                                value: Box::new(call(
                                    "next_char",
                                    vec![var("s")],
                                )),
                            },
                        ])),
                        invariant: None,
                    },
                    call(
                        "Ok",
                        vec![push_token(var("s1"), var("s"), 0, var("lex"))],
                    ),
                ])),
            }),
        };
        IRNode::Let {
            name: "s1".into(),
            value: Box::new(call("skip_whitespace", vec![var("state")])),
            body: Box::new(IRNode::If {
                condition: Box::new(peek_is_none(var("s1"))),
                then_branch: Box::new(call("Ok", vec![var("s1")])),
                else_branch: Box::new(IRNode::If {
                    condition: Box::new(call(
                        "is_ident_start",
                        vec![peeked(var("s1"))],
                    )),
                    then_branch: Box::new(ident_arm),
                    else_branch: Box::new(dispatch),
                }),
            }),
        }
    }
    /// Phase 84 — RE-STATED from Phase 82's
    /// `phase82_tokenize_one_mirror_body_vs_amended_rows`:
    /// the mirror-transcribed tokenize_one body against
    /// the ROUND-2 rows (A′ + R2-α, owner-ratified
    /// 2026-11-22, applied in this phase) — the contract
    /// the generation campaign (Phase 85) is built
    /// against, measured through the production encoder:
    /// the whole body judged by verify_one, and the
    /// mirror's own output judged at the point for each
    /// of the cross-validation battery's input classes.
    /// Phase 82's table pinned two REFUTED classes under
    /// amendment A; Phase 83 measured the round-2
    /// formulations on scratch copies; this re-statement
    /// pins Phase 83's joint table against the REAL
    /// module — both classes are closed, and the one
    /// surviving deviation is stated at its own point
    /// below.
    ///
    /// Whole body: verify_one returns Inconclusive, and
    /// the body is outside the encodable subset twice
    /// over, both blockers named (unchanged): its ident
    /// arm is a While loop (While is an unencodable arm
    /// of sym_eval — the standing loop boundary), and
    /// its first step calls skip_whitespace, for which
    /// the resolver denotes NO body (its two stated rows
    /// are field inequalities no builder denotes), so
    /// there is nothing to inline even before the loop.
    /// A Phase 85 body that verifies must be an
    /// encodable formulation of this behavior, not this
    /// transcription.
    ///
    /// Per class (input at pos 0, no tokens; the
    /// mirror's output judged against both rows at the
    /// point):
    ///   ""      EOF           Ok, no token   Proven / Unknown — row 1 admits; row 2's literal point is the OOB artifact (admission proven symbolically, companion test)
    ///   "spec"  ident         Ok, +1 token   Proven / Proven — admitted by the growth disjunct
    ///   "  spec" leading ws   Ok, +1 token   Proven / Proven — admitted by growth
    ///   "\n\nx" newline run   Ok, +1 token   Proven / Proven — admitted by growth
    ///   ":"     single char   Ok, +1 token   Proven / Proven — admitted by growth
    ///   " "     ws-only rest  Ok, no token   Proven / Proven — ADMITTED (class W closed by A′; Phase 82: row 1 refuted it)
    ///   "5"     unexpected    Err            free tag / Proven — ADMITTED (class E closed by R2-α; Phase 82: row 2 refuted it)
    ///   "é"     unexpected    Err            free tag / Proven — α licenses a non-ASCII bad char too
    ///
    /// The one surviving deviation, named: at " 5"
    /// (whitespace, then a bad char) the mirror's Err
    /// stays REFUTED by row 2 — α reads the PRE-skip
    /// character, a space — and the joint rows admit
    /// only the fabricated Ok/+1 there. Phase 83 priced
    /// this subclass as unreachable from the spec side
    /// (the peek form over-licenses; the skip form is
    /// unencodable; a skip denotation is underivable);
    /// it stands as the rows' one named deviation from
    /// the mirror — the generation campaign's one named
    /// executed-agreement gap.
    ///
    /// Teeth under the joint rows, pinned below: a
    /// no-growth Ok at a token start is refuted by row
    /// 1; Err at identifier, punctuation, and whitespace
    /// positions is refuted by row 2. A′'s residue is
    /// pinned too: a no-growth Ok whose pos jumps to
    /// end-of-input is admitted by row 1 (the row cannot
    /// tell a skip-to-EOF from a teleport; the golden
    /// tests judge that shape). At EOF inputs the rows
    /// constrain nothing further (the companion test's
    /// L=2 case) — executed agreement is the only judge
    /// there.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase84_tokenize_one_mirror_body_vs_round2_rows() {
        // The REAL module (examples/): the round-2
        // amendment lives there, and the gate judges
        // this file.
        let module = {
            let src = std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/functional_tokenizer.morph"
            ))
            .expect("examples tokenizer reads");
            let parsed = crate::spec::parser::parse(&src).expect("examples parses");
            crate::spec::typeck::check(parsed)
                .expect("examples typechecks")
                .spec
        };
        let spec = restored_spec(&module, "tokenize_one");
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);

        // ── Whole body: Inconclusive, with both blockers
        // named. Phase 92 re-pin: the resolver now DOES
        // denote skip_whitespace (the keyed While body —
        // peek denotes since Phase 81), asserted here so
        // the contrast is pinned, not anecdotal. The
        // whole-body verdict below is unmoved: a
        // denotation is a meaning, and the encoder still
        // has no While arm to judge this body with
        // (Phase 93) — the mirror transcription stays
        // unencodable and Inconclusive.
        assert!(
            index.callee_body("skip_whitespace").is_some(),
            "Phase 92: skip_whitespace denotes (the keyed While body)"
        );
        assert!(
            index.callee_body("peek").is_some(),
            "peek denotes since Phase 81"
        );
        let body = tokenize_one_mirror_body();
        assert!(
            !body_is_encodable_with(&spec, &body, &index),
            "the mirror transcription is outside the encodable subset \
             (While arm; the skip_whitespace call)"
        );
        let cand = candidate(&spec, 82, body);
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::Inconclusive,
            "the mirror body cannot verify whole: loops do not encode"
        );

        // ── Per-class point verdicts: the mirror's own
        // output at each battery input, judged against
        // both round-2 rows at the point.
        let int = |n: i64| Sym::Int(z3::ast::Int::from_i64(n));
        let state_of = |source: &str, pos: i64, tok_len: i64| Sym::Record {
            type_name: "TokenizerState".into(),
            fields: vec![
                (
                    "source".into(),
                    Sym::Str(z3::ast::String::from_str(source).unwrap()),
                ),
                ("pos".into(), int(pos)),
                ("line".into(), int(1)),
                ("col".into(), int(1)),
                (
                    "tokens".into(),
                    Sym::ListLen(z3::ast::Int::from_i64(tok_len)),
                ),
            ],
        };
        let ok_of = |st: Sym| Sym::Result {
            ok: Tag::True,
            ok_payload: Some(Box::new(st)),
            err_payload: None,
        };
        // (class name, input source, mirror output,
        //  expected row 1, expected row 2). The mirror
        //  outputs carry its true end pos; the rows read
        //  the tag, the payload's token length, the
        //  OUTPUT's pos / source length (row 1's A′
        //  disjunct), and the INPUT's pos / source
        //  length / indexed char (row 2's α license).
        let classes: Vec<(&str, &str, Sym, Tri, Tri)> = vec![
            (
                "eof",
                "",
                ok_of(state_of("", 0, 0)),
                Tri::Proven,
                Tri::Unknown,
            ),
            (
                "ident",
                "spec",
                ok_of(state_of("spec", 4, 1)),
                Tri::Proven,
                Tri::Proven,
            ),
            (
                "leading-ws-ident",
                "  spec",
                ok_of(state_of("  spec", 6, 1)),
                Tri::Proven,
                Tri::Proven,
            ),
            (
                "newline-run",
                "\n\nx",
                ok_of(state_of("\n\nx", 3, 1)),
                Tri::Proven,
                Tri::Proven,
            ),
            (
                "single-colon",
                ":",
                ok_of(state_of(":", 1, 1)),
                Tri::Proven,
                Tri::Proven,
            ),
            (
                "corner-whitespace-only",
                " ",
                ok_of(state_of(" ", 1, 0)),
                Tri::Proven,
                Tri::Proven,
            ),
        ];
        for (name, source, output, want1, want2) in classes {
            let mut env = SymEnv::new();
            env.insert("state".into(), state_of(source, 0, 0));
            let (_, tri) = verifier.verify_constraint(
                &spec.postconditions[0],
                "row1@class",
                spec,
                &env,
                &Some(output.clone()),
            );
            assert!(
                matches!((&tri, &want1), (Tri::Proven, Tri::Proven)
                    | (Tri::Refuted, Tri::Refuted)
                    | (Tri::Unknown, Tri::Unknown)),
                "class {}: round-2 row 1 verdict for the mirror output",
                name
            );
            let (_, tri) = verifier.verify_constraint(
                &spec.postconditions[1],
                "row2@class",
                spec,
                &env,
                &Some(output),
            );
            assert!(
                matches!((&tri, &want2), (Tri::Proven, Tri::Proven)
                    | (Tri::Refuted, Tri::Refuted)
                    | (Tri::Unknown, Tri::Unknown)),
                "class {}: row 2 verdict for the mirror output",
                name
            );
        }

        // The unexpected-character class, closed: the
        // mirror's Err at input "5". Row 2 now PROVES it
        // — the α license fires on the indexed char '5'
        // (not ident-start, not whitespace, not a
        // punctuation) where the unamended row 2 refuted
        // it in Phase 82. Row 1 is satisfied through its
        // is_err disjunct — discharged via the free tag,
        // as in Phase 80 (row 1 under a LITERAL Err tag
        // is the encoder's named unencodable case). The
        // non-ASCII point ("é") is admitted the same way.
        let mut env = SymEnv::new();
        env.insert("state".into(), state_of("5", 0, 0));
        let err = Sym::Result {
            ok: Tag::False,
            ok_payload: None,
            err_payload: Some(Box::new(Sym::Str(
                z3::ast::String::from_str("Unexpected character '5' at line 1")
                    .unwrap(),
            ))),
        };
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[1],
            "row2@digit-err",
            spec,
            &env,
            &Some(err),
        );
        assert!(
            matches!(tri, Tri::Proven),
            "round-2 row 2 ADMITS the mirror's mid-input Err at \"5\" \
             (the α license; Phase 82: refuted)"
        );
        let is_ok = z3::ast::Bool::new_const("out_is_ok");
        let mut env = SymEnv::new();
        env.insert("state".into(), state_of("5", 0, 0));
        env.insert(
            "output".into(),
            Sym::Result {
                ok: Tag::Free(is_ok.clone()),
                ok_payload: Some(Box::new(state_of("5", 1, 1))),
                err_payload: Some(Box::new(
                    Sym::Str(z3::ast::String::from_str("boom").unwrap()),
                )),
            },
        );
        let formula = encode_constraint(
            &spec.postconditions[0],
            &env,
            &EvalCtx::root(&index),
        )
        .expect("row 1 encodes under a free tag");
        {
            use crate::verify::smt::{SmtResult, Z3Session};
            let z3 = Z3Session::new(10_000);
            z3.assert(&formula);
            z3.assert(&is_ok.not());
            assert!(
                matches!(z3.check(), SmtResult::Sat(_)),
                "row 1 is satisfiable by the class-E Err (free tag)"
            );
        }
        let mut env = SymEnv::new();
        env.insert("state".into(), state_of("é", 0, 0));
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[1],
            "row2@eacute-err",
            spec,
            &env,
            &Some(Sym::Result {
                ok: Tag::False,
                ok_payload: None,
                err_payload: Some(Box::new(Sym::Str(
                    z3::ast::String::from_str("Unexpected character").unwrap(),
                ))),
            }),
        );
        assert!(
            matches!(tri, Tri::Proven),
            "round-2 row 2 admits the mirror's Err at a non-ASCII bad char"
        );

        // The surviving deviation, at its own point: at
        // " 5" the mirror's Err stays REFUTED by row 2
        // (α reads the pre-skip char — a space), and the
        // only shape the joint rows admit is the
        // fabricated Ok/+1. This subclass is the rows'
        // one named deviation from the mirror.
        let mut env = SymEnv::new();
        env.insert("state".into(), state_of(" 5", 0, 0));
        let err5 = Sym::Result {
            ok: Tag::False,
            ok_payload: None,
            err_payload: Some(Box::new(Sym::Str(
                z3::ast::String::from_str("Unexpected character '5' at line 1")
                    .unwrap(),
            ))),
        };
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[1],
            "row2@ws-digit-err",
            spec,
            &env,
            &Some(err5),
        );
        assert!(
            matches!(tri, Tri::Refuted),
            "\" 5\" residue: the mirror's Err stays refuted — α reads \
             the pre-skip char"
        );
        for (i, label) in ["row1", "row2"].iter().enumerate() {
            let (_, tri) = verifier.verify_constraint(
                &spec.postconditions[i],
                label,
                spec,
                &env,
                &Some(ok_of(state_of(" 5", 2, 1))),
            );
            assert!(
                matches!(tri, Tri::Proven),
                "\" 5\" residue: only the fabricated Ok/+1 is admitted ({})",
                label
            );
        }

        // Teeth under the joint rows: Err at an
        // identifier start, at a punctuation start, and
        // at the whitespace corner is refuted by row 2 —
        // the α license does not over-admit. (Row 1's
        // teeth — the no-growth Ok at a token start —
        // are pinned in the companion test's section D.)
        for (source, label) in [
            ("spec", "identifier start"),
            (":", "punctuation start"),
            (" ", "whitespace corner"),
        ] {
            let mut env = SymEnv::new();
            env.insert("state".into(), state_of(source, 0, 0));
            let (_, tri) = verifier.verify_constraint(
                &spec.postconditions[1],
                "row2@teeth",
                spec,
                &env,
                &Some(Sym::Result {
                    ok: Tag::False,
                    ok_payload: None,
                    err_payload: Some(Box::new(Sym::Str(
                        z3::ast::String::from_str("x").unwrap(),
                    ))),
                }),
            );
            assert!(
                matches!(tri, Tri::Refuted),
                "teeth: Err at a {} stays refuted by row 2",
                label
            );
        }

        // A′'s residue in the joint context: a no-growth
        // Ok whose pos jumps to end-of-input is admitted
        // by row 1 — the row cannot distinguish it from
        // the class-W skip; the golden tests judge it.
        let mut env = SymEnv::new();
        env.insert("state".into(), state_of("spec", 0, 0));
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[0],
            "row1@teleport",
            spec,
            &env,
            &Some(ok_of(state_of("spec", 4, 0))),
        );
        assert!(
            matches!(tri, Tri::Proven),
            "A′ residue: a no-growth Ok whose pos reaches EOF is \
             admitted by row 1"
        );
    }

    // ── Phase 84: round-2 rows, standing pins ────────
    //
    // Phase 83 measured the round-2 candidates on
    // SCRATCH copies of the examples module (a
    // substitution builder plus five Z3-gated tests).
    // The owner ratified option 1 (A′ + R2-α, callee
    // form) on 2026-11-22 and this phase applied it to
    // examples/functional_tokenizer.morph — so the
    // scratch apparatus is retired here: the
    // substitution builder and the candidate-row
    // constants are removed, and the five measurement
    // tests are disposed of as follows.
    //
    //   * phase83_a_prime_output_pos_disjunct and
    //     phase83_joint_a_prime_alpha_battery: their
    //     surviving assertions are the standing battery
    //     itself — folded into
    //     phase84_tokenize_one_mirror_body_vs_round2_rows
    //     (above), judged against the REAL module. The
    //     A/A′ contrast assertion (the real file refutes
    //     the class-W output) is dropped: its premise
    //     is what this phase removes.
    //   * phase83_row2_alpha_err_license: the callee
    //     half's case table is the real row 2 now and
    //     is pinned in the same battery (class E, the
    //     teeth, the non-ASCII point); the direct half
    //     measured an unratified formulation — its
    //     verdicts stand in the Phase 83 record, not in
    //     the suite.
    //   * phase83_row2_beta_formulations_fail: retired.
    //     Its subject (formulations that are not rows
    //     of any spec) does not belong against the real
    //     module; its one load-bearing pin — the
    //     resolver denotes no body for skip_whitespace —
    //     stands in the mirror-body test above, and the
    //     β verdicts are recorded in the Phase 83 note.
    //   * phase83_symbolic_state_encodability:
    //     RE-STATED below against the real module — the
    //     symbolic-path facts are not point facts and
    //     do not fold into the battery.

    /// The real examples module (round-2 rows as
    /// applied in this phase).
    #[cfg(feature = "z3-support")]
    fn phase84_real_module() -> Spec {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/functional_tokenizer.morph"
        ))
        .expect("examples tokenizer reads");
        let parsed = crate::spec::parser::parse(&src).expect("examples parses");
        crate::spec::typeck::check(parsed)
            .expect("examples typechecks")
            .spec
    }

    /// Point Syms in the Phase 82 shapes: a state is
    /// observed through exactly what the rows read (the
    /// source as a literal string, pos, the token list
    /// through its length).
    #[cfg(feature = "z3-support")]
    fn phase84_state(source: &str, pos: i64, tok_len: i64) -> Sym {
        let int = |n: i64| Sym::Int(z3::ast::Int::from_i64(n));
        Sym::Record {
            type_name: "TokenizerState".into(),
            fields: vec![
                (
                    "source".into(),
                    Sym::Str(z3::ast::String::from_str(source).unwrap()),
                ),
                ("pos".into(), int(pos)),
                ("line".into(), int(1)),
                ("col".into(), int(1)),
                (
                    "tokens".into(),
                    Sym::ListLen(z3::ast::Int::from_i64(tok_len)),
                ),
            ],
        }
    }

    #[cfg(feature = "z3-support")]
    fn phase84_ok(st: Sym) -> Sym {
        Sym::Result {
            ok: Tag::True,
            ok_payload: Some(Box::new(st)),
            err_payload: None,
        }
    }

    #[cfg(feature = "z3-support")]
    fn phase84_err(msg: &str) -> Sym {
        Sym::Result {
            ok: Tag::False,
            ok_payload: None,
            err_payload: Some(Box::new(
                Sym::Str(z3::ast::String::from_str(msg).unwrap()),
            )),
        }
    }

    /// Phase 84 — RE-STATED from Phase 83's
    /// `phase83_symbolic_state_encodability`, which
    /// measured the candidate rows on scratch modules.
    /// The rows are the module's own now (A′ row 1;
    /// R2-α callee row 2 — the direct formulation was
    /// not ratified and its measurement loop is dropped;
    /// its verdicts stand in the Phase 83 record), the
    /// scratch substitution is gone, and this test reads
    /// the real examples module throughout.
    ///
    /// The battery's case tables judge literal points,
    /// where sym_index is exact; production judging
    /// (verify_one) binds the state SYMBOLICALLY, so the
    /// rows must also encode — and judge — with a free
    /// source and a free pos, where
    /// `state.source[state.pos]` takes the CharStr bridge
    /// form (`str.at` of a symbolic index).
    ///
    /// Measured, against the real rows:
    ///   * α row 2 ENCODES under a fully free state (the
    ///     callee form inlines both classifiers over the
    ///     CharStr argument), and under that path the
    ///     EOF Err admission the literal table cannot
    ///     judge (the OOB artifact) is PROVEN: with
    ///     len(source) == 0 ∧ pos == 0 asserted, the
    ///     row's negation has no model — the license is
    ///     the row's own EOF disjunct. The digit
    ///     admission is likewise proven symbolically
    ///     (source == "5" ∧ pos == 0: no countermodel —
    ///     the license fires through the bridge, not
    ///     only at literal points).
    ///   * The EOF admission for the golden OK output —
    ///     Unknown at the literal points in the standing
    ///     tests (the same artifact) — is proven on this
    ///     path: a free state with a free Ok payload,
    ///     under the EOF facts, admits no countermodel.
    ///   * A′ row 1 encodes both against a point Ok
    ///     output and against a fully free Ok payload
    ///     (its disjunct is Int >= len(Str) over free
    ///     terms).
    ///   * Control: α row 2 under a free state and an
    ///     Err output is NOT valid (a countermodel
    ///     exists — a token-start char): the row
    ///     genuinely discriminates, it does not merely
    ///     encode.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase84_symbolic_state_encodability() {
        use crate::verify::smt::{SmtResult, Z3Session};

        // A state whose fields are free constants, in
        // param_env's shapes; returns the state plus the
        // source/pos terms for fact assertions.
        let free_state = || {
            let source = z3::ast::String::new_const("state.source");
            let pos = z3::ast::Int::new_const("state.pos");
            let st = Sym::Record {
                type_name: "TokenizerState".into(),
                fields: vec![
                    ("source".into(), Sym::Str(source.clone())),
                    ("pos".into(), Sym::Int(pos.clone())),
                    (
                        "line".into(),
                        Sym::Int(z3::ast::Int::new_const("state.line")),
                    ),
                    (
                        "col".into(),
                        Sym::Int(z3::ast::Int::new_const("state.col")),
                    ),
                    (
                        "tokens".into(),
                        Sym::ListLen(z3::ast::Int::new_const("state.tokens.len")),
                    ),
                ],
            };
            (st, source, pos)
        };
        let free_payload = || Sym::Record {
            type_name: "TokenizerState".into(),
            fields: vec![
                (
                    "source".into(),
                    Sym::Str(z3::ast::String::new_const("output.source")),
                ),
                (
                    "pos".into(),
                    Sym::Int(z3::ast::Int::new_const("output.pos")),
                ),
                (
                    "line".into(),
                    Sym::Int(z3::ast::Int::new_const("output.line")),
                ),
                (
                    "col".into(),
                    Sym::Int(z3::ast::Int::new_const("output.col")),
                ),
                (
                    "tokens".into(),
                    Sym::ListLen(z3::ast::Int::new_const("output.tokens.len")),
                ),
            ],
        };

        let module = phase84_real_module();
        let spec = restored_spec(&module, "tokenize_one");
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);

        // α row 2 under a symbolic state, Err output:
        // encodability, then the two admissions.
        let (state, source, pos) = free_state();
        let mut env = SymEnv::new();
        env.insert("state".into(), state);
        env.insert("output".into(), phase84_err("Unexpected character"));
        let formula = encode_constraint(
            &spec.postconditions[1],
            &env,
            &EvalCtx::root(&index),
        );
        assert!(
            formula.is_some(),
            "α row 2 ENCODES under a symbolic state — the symbolic \
             index takes the CharStr bridge form"
        );
        let formula = formula.unwrap();

        // Admission at end-of-input, symbolic path:
        // under len(source) == 0 ∧ pos == 0 the negated
        // row has no model.
        let z3 = Z3Session::new(10_000);
        z3.assert(&formula.not());
        z3.assert(&source.length().eq(&z3::ast::Int::from_i64(0)));
        z3.assert(&pos.eq(&z3::ast::Int::from_i64(0)));
        assert!(
            matches!(z3.check(), SmtResult::Unsat),
            "α row 2 admits Err at end-of-input under the symbolic path"
        );
        // Admission at the digit point, symbolic path:
        // under source == "5" ∧ pos == 0 the negated row
        // has no model — the license fires through the
        // bridge, not only at literal points.
        let z3 = Z3Session::new(10_000);
        z3.assert(&formula.not());
        z3.assert(
            &source.eq(&z3::ast::String::from_str("5").unwrap()),
        );
        z3.assert(&pos.eq(&z3::ast::Int::from_i64(0)));
        assert!(
            matches!(z3.check(), SmtResult::Unsat),
            "α row 2 admits Err at \"5\" under the symbolic path"
        );
        // Control: with no facts, the row under an Err
        // output is not valid — a countermodel exists
        // (the free char may be a token start).
        let mut env = SymEnv::new();
        let (state, _, _) = free_state();
        env.insert("state".into(), state);
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[1],
            "alpha@free-state-err",
            spec,
            &env,
            &Some(phase84_err("Unexpected character")),
        );
        assert!(
            matches!(tri, Tri::Refuted),
            "α row 2 discriminates under a free state (not vacuous)"
        );

        // The EOF admission for an Ok output (the golden
        // output's symbolic counterpart): a free state
        // with a free Ok payload; under the EOF facts
        // the negated row has no model. This discharges
        // the literal points' Unknown artifact in the
        // standing tests.
        let (state, source, pos) = free_state();
        let mut env = SymEnv::new();
        env.insert("state".into(), state);
        env.insert("output".into(), phase84_ok(free_payload()));
        let formula = encode_constraint(
            &spec.postconditions[1],
            &env,
            &EvalCtx::root(&index),
        )
        .expect("α row 2 encodes with a symbolic state and a free Ok output");
        let z3 = Z3Session::new(10_000);
        z3.assert(&formula.not());
        z3.assert(&source.length().eq(&z3::ast::Int::from_i64(0)));
        z3.assert(&pos.eq(&z3::ast::Int::from_i64(0)));
        assert!(
            matches!(z3.check(), SmtResult::Unsat),
            "α row 2 admits the golden Ok output at end-of-input \
             under the symbolic path"
        );

        // A′ row 1 under symbolic shapes: a free state
        // with a point Ok output encodes, and so does a
        // fully free Ok payload.
        let (state, _, _) = free_state();
        let mut env = SymEnv::new();
        env.insert("state".into(), state);
        env.insert(
            "output".into(),
            phase84_ok(phase84_state("spec", 4, 1)),
        );
        assert!(
            encode_constraint(&spec.postconditions[0], &env, &EvalCtx::root(&index))
                .is_some(),
            "A′ row 1 encodes with a symbolic state and a point Ok output"
        );
        env.insert("output".into(), phase84_ok(free_payload()));
        assert!(
            encode_constraint(&spec.postconditions[0], &env, &EvalCtx::root(&index))
                .is_some(),
            "A′ row 1 encodes with a fully free Ok payload: \
             pos >= len(source) over free terms"
        );
    }

    /// Phase 87 — skip_whitespace strengthening
    /// MEASUREMENT (workstream 2 of the road-B push;
    /// scratch only: the examples file is NOT touched
    /// and nothing measured here is ratified — the
    /// Phase 88 gate re-run prices these rows from
    /// this record). skip's spec states two invariants
    /// (pos monotone, source length), and its crowned
    /// winner satisfies them without performing the
    /// skip (Phase 86 freeze: Verified, tpr 0.500,
    /// mirror agreement 4/12, stated rows 12/12).
    /// Candidate strengthenings, formulated in the
    /// module dialect and measured through the
    /// production encoder (Phase 83/84 method):
    ///   (a) boundary — the output position is at EOF
    ///       or the char there is not whitespace;
    ///   (b) no-overshoot — an advanced output
    ///       position's previous char is whitespace;
    ///   (c) all-whitespace-between — a Forall over
    ///       the skipped range.
    ///
    /// MEASURED (this test pins each claim):
    ///   (a) ENCODES — symbolically (free state/output,
    ///       the indexed char through the Phase 76
    ///       CharStr bridge) and at literal points.
    ///       The hand mirror's outputs satisfy it at
    ///       10 of 12 battery points (Proven); the two
    ///       EOF outputs ("" and " ") judge Unknown at
    ///       literal points — the standing OOB
    ///       artifact (the row's index term is out of
    ///       range exactly where its first disjunct
    ///       holds; the symbolic path carries the
    ///       proof burden, as with R2-α in Phase 84).
    ///       The crowned winner is REFUTED by (a)
    ///       (VerificationFailed): the strengthening
    ///       bites. But no constraint-seed family
    ///       fires for the strengthened spec (kinds
    ///       [] — the same as the current spec), and
    ///       NO pool body is Verified under
    ///       current+(a) (0 in the measured pool; the
    ///       zero is structural — a Verified body is
    ///       loop-free, and a loop-free body cannot
    ///       cross an unbounded whitespace run): the
    ///       only body that performs the skip is the
    ///       hand mirror, whose While body is
    ///       Inconclusive by construction, and no
    ///       denotation exists for skip_whitespace
    ///       (callee_body None, re-pinned here).
    ///   (b) ENCODES (symbolically and at all 12
    ///       points; the mirror satisfies it 12/12).
    ///       The crowned winner SURVIVES it
    ///       (Verified) — a body that barely advances
    ///       satisfies a no-overshoot row vacuously —
    ///       and pool bodies are Verified under
    ///       current+(b) (11 in the measured pool).
    ///       Alone, (b) changes nothing.
    ///   (a)+(b) together still do NOT pin the skip:
    ///       both rows PROVE at the overshoot points
    ///       ("a b" from 0 landing at pos 2; "ab cd"
    ///       from 0 landing at pos 3) — the rows pin
    ///       the landing to A word boundary (never on
    ///       whitespace, always just after one), not
    ///       to THE first boundary. The exact pin is
    ///       (c)'s content.
    ///   (c) is STATABLE — the formulation parses to
    ///       Constraint::Forall with the intended
    ///       body — and UNENCODABLE: the production
    ///       encoder returns None for Forall
    ///       (verifier.rs: quantified formulas have no
    ///       encoding here), confirming the Phase 80
    ///       option-D boundary. Closing the skip
    ///       clause exactly is therefore encoder
    ///       research (the L–XL extension the owner
    ///       did not commission), not a spec edit.
    ///   No candidate reads line/col: even a
    ///       hypothetical Verified body under (a)+(b)
    ///       could mis-track them; golden/executed
    ///       agreement remains their only judge.
    ///
    /// The priced statement for Phase 88: ratifying
    /// (a) (with or without (b)) trades skip's
    /// standing — a Verified winner that satisfies
    /// the stated rows without skipping (golden
    /// 4/12, partial BY SPEC STRENGTH) — for NO
    /// Verified body at all, with the gate's skip
    /// clause still unprovable exactly ((c)
    /// unencodable, line/col unpinned). It buys a
    /// refuted wrong winner and a spec that states
    /// the boundary; it cannot buy a Verified
    /// correct skip.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase87_skip_strengthening_measurement() {
        use crate::spec::ast::Constraint;
        use crate::synthesis::engine::{
            synthesize_with_module_spec, SynthesisConfig, SynthesisEngine,
        };
        use crate::synthesis::selector::select_best;

        let module = phase84_real_module();
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);

        // Candidate rows parsed in the module dialect
        // (the parser is syntactic; the encoder
        // resolves the classifier call against the
        // real module index, as it did for R2-α).
        let parse_row = |text: &str| -> Constraint {
            let src = format!(
                "T {{\n spec probe {{\n input: state: TokenizerState\n output: TokenizerState\n constraint: {}\n }}\n}}",
                text
            );
            let parsed = crate::spec::parser::parse(&src).expect("candidate row parses");
            parsed.functions[0].postconditions[0].clone()
        };
        let row_a = parse_row(
            "output.pos >= len(output.source) || is_whitespace(output.source[output.pos]) == false",
        );
        let row_b = parse_row(
            "output.pos == state.pos || is_whitespace(output.source[output.pos - 1])",
        );
        let strengthened = |row: &Constraint| {
            let mut s = skip.clone();
            s.postconditions.push(row.clone());
            s
        };

        // Point Syms in the Phase 84 shapes.
        let state_sym = |source: &str, pos: i64, line: i64, col: i64| Sym::Record {
            type_name: "TokenizerState".into(),
            fields: vec![
                (
                    "source".into(),
                    Sym::Str(z3::ast::String::from_str(source).unwrap()),
                ),
                ("pos".into(), Sym::Int(z3::ast::Int::from_i64(pos))),
                ("line".into(), Sym::Int(z3::ast::Int::from_i64(line))),
                ("col".into(), Sym::Int(z3::ast::Int::from_i64(col))),
                (
                    "tokens".into(),
                    Sym::ListLen(z3::ast::Int::from_i64(0)),
                ),
            ],
        };
        // The mirror's skip, simulated on chars (the
        // Phase 47 mirror semantics: whitespace is
        // exactly ' ' '\t' '\n' '\r' — U+A0 is not —
        // and next_char tracks line/col through
        // newlines).
        let mirror_skip = |source: &str, pos: i64, line: i64, col: i64| {
            let chars: Vec<char> = source.chars().collect();
            let (mut p, mut l, mut c) = (pos, line, col);
            while (p as usize) < chars.len()
                && matches!(chars[p as usize], ' ' | '\t' | '\n' | '\r')
            {
                if chars[p as usize] == '\n' {
                    l += 1;
                    c = 1;
                } else {
                    c += 1;
                }
                p += 1;
            }
            (p, l, c)
        };
        // The Phase 47 skip battery (xval skip_battery):
        // (source, pos, line, col).
        let battery: Vec<(&str, i64, i64, i64)> = vec![
            ("", 0, 1, 1),
            (" ", 0, 1, 1),
            ("  \t\n x", 0, 1, 1),
            ("no-ws", 0, 1, 1),
            ("\n\nab", 0, 1, 1),
            (" \u{A0}x", 0, 1, 1),
            ("a b", 0, 1, 1),
            ("\r\n\t", 0, 1, 1),
            ("a b", 1, 1, 2),
            ("  \t\n x", 2, 1, 3),
            ("\n\nab", 1, 1, 2),
            (" \u{A0}x", 1, 1, 2),
        ];
        // Mirror satisfaction at the battery points:
        // (a) Proven everywhere except the two EOF
        // outputs (the literal-point OOB artifact);
        // (b) Proven at all 12.
        for (label, row) in [("a", &row_a), ("b", &row_b)] {
            let spec = strengthened(row);
            for (i, (src, pos, line, col)) in battery.iter().enumerate() {
                let (p, l, c) = mirror_skip(src, *pos, *line, *col);
                let mut env = SymEnv::new();
                env.insert("state".into(), state_sym(src, *pos, *line, *col));
                let (_, tri) = verifier.verify_constraint(
                    row,
                    "phase87@point",
                    &spec,
                    &env,
                    &Some(state_sym(src, p, l, c)),
                );
                match (label, i) {
                    ("a", 0) | ("a", 1) => assert!(
                        matches!(tri, Tri::Unknown),
                        "row (a) at EOF point {}: the OOB artifact judges Unknown, got {}",
                        i,
                        match tri {
                            Tri::Proven => "Proven",
                            Tri::Refuted => "Refuted",
                            Tri::Unknown => "Unknown",
                        }
                    ),
                    _ => assert!(
                        matches!(tri, Tri::Proven),
                        "row ({}) at battery point {} ({:?} from {}): the mirror's \
                         output must satisfy it, got {}",
                        label,
                        i,
                        src,
                        pos,
                        match tri {
                            Tri::Proven => "Proven",
                            Tri::Refuted => "Refuted",
                            Tri::Unknown => "Unknown",
                        }
                    ),
                }
            }
        }

        // The overshoot admissions: outputs that are
        // NOT the mirror's (they jump a word) satisfy
        // BOTH rows — (a)+(b) pin a word boundary, not
        // the first one.
        for (src, out_pos) in [("a b", 2), ("ab cd", 3)] {
            for (label, row) in [("a", &row_a), ("b", &row_b)] {
                let spec = strengthened(row);
                let mut env = SymEnv::new();
                env.insert("state".into(), state_sym(src, 0, 1, 1));
                let (_, tri) = verifier.verify_constraint(
                    row,
                    "phase87@overshoot",
                    &spec,
                    &env,
                    &Some(state_sym(src, out_pos, 1, 3)),
                );
                assert!(
                    matches!(tri, Tri::Proven),
                    "row ({}) must ADMIT the overshoot {:?} -> {} (the residual \
                     freedom only the Forall would close)",
                    label,
                    src,
                    out_pos
                );
            }
        }

        // Symbolic-path encodability: free state and
        // free output records (the CharStr bridge form
        // for the indexed char).
        let free_record = |prefix: &str| Sym::Record {
            type_name: "TokenizerState".into(),
            fields: vec![
                (
                    "source".into(),
                    Sym::Str(z3::ast::String::new_const(format!("{}.source", prefix))),
                ),
                (
                    "pos".into(),
                    Sym::Int(z3::ast::Int::new_const(format!("{}.pos", prefix))),
                ),
                (
                    "line".into(),
                    Sym::Int(z3::ast::Int::new_const(format!("{}.line", prefix))),
                ),
                (
                    "col".into(),
                    Sym::Int(z3::ast::Int::new_const(format!("{}.col", prefix))),
                ),
                (
                    "tokens".into(),
                    Sym::ListLen(z3::ast::Int::new_const(format!(
                        "{}.tokens.len",
                        prefix
                    ))),
                ),
            ],
        };
        for (label, row) in [("a", &row_a), ("b", &row_b)] {
            let mut env = SymEnv::new();
            env.insert("state".into(), free_record("state"));
            env.insert("output".into(), free_record("output"));
            let f = encode_constraint(row, &env, &EvalCtx::root(&index));
            assert!(
                f.is_some(),
                "row ({}) ENCODES under a fully free state and output \
                 (the CharStr bridge form)",
                label
            );
        }

        // The crowned winner under the CURRENT rows,
        // judged against each strengthened spec: (a)
        // refutes it, (b) does not, (a)+(b) refutes it
        // through (a).
        let pool = synthesize_with_module_spec(&skip, 1, &module)
            .expect("skip synthesis under current rows");
        let verified = verifier
            .verify_all(pool, &skip)
            .expect("verify pool under current rows");
        let winner = select_best(verified, &skip).expect("a crowned winner exists");
        assert!(
            winner.verified,
            "the standing winner is Verified under the current rows"
        );
        assert_eq!(
            verifier.verify_one(&winner, &strengthened(&row_a)).status,
            VerificationStatus::VerificationFailed,
            "row (a) REFUTES the crowned winner: the strengthening bites"
        );
        assert_eq!(
            verifier.verify_one(&winner, &strengthened(&row_b)).status,
            VerificationStatus::Verified,
            "row (b) alone leaves the crowned winner standing (vacuous bite)"
        );
        let mut both = skip.clone();
        both.postconditions.push(row_a.clone());
        both.postconditions.push(row_b.clone());
        assert_eq!(
            verifier.verify_one(&winner, &both).status,
            VerificationStatus::VerificationFailed,
            "(a)+(b) refute the winner through (a)"
        );

        // Seed existence + pool verdicts under each
        // strengthened spec: no constraint-seed family
        // fires (the current spec's kinds are the
        // control), no body is Verified under
        // current+(a), and bodies are Verified under
        // current+(b) without performing the skip.
        assert!(
            index.callee_body("skip_whitespace").is_some(),
            "Phase 92 re-pin: a keyed denotation now exists for \
             skip_whitespace — a resolver meaning, not a seed: no seed \
             family builds from it, and the seed claims below are unmoved"
        );
        let engine = SynthesisEngine::with_module_spec(SynthesisConfig::default(), &module);
        assert!(
            engine.constraint_seed_kinds(&skip).is_empty(),
            "control: no constraint-seed kinds fire for the current spec either"
        );
        for (label, row, want_verified) in [("a", &row_a, Some(0usize)), ("b", &row_b, None)] {
            let spec = strengthened(row);
            assert!(
                engine.constraint_seed_kinds(&spec).is_empty(),
                "no constraint-seed kinds fire under strengthened row ({})",
                label
            );
            let pool = synthesize_with_module_spec(&spec, 1, &module)
                .expect("skip synthesis under strengthened rows");
            let verified = verifier
                .verify_all(pool, &spec)
                .expect("verify pool under strengthened rows");
            let n_verified = verified.iter().filter(|c| c.verified).count();
            match want_verified {
                // (a): exactly zero — structurally, not
                // by luck of the draw: a Verified body
                // must be loop-free (loops do not
                // encode), and a loop-free body advances
                // a bounded number of steps, so a
                // whitespace run longer than that bound
                // is a countermodel (it lands mid-run,
                // on whitespace).
                Some(want) => assert_eq!(
                    n_verified, want,
                    "Verified bodies under current+({})",
                    label
                ),
                // (b): at least the standing winner's
                // class verifies (it satisfies (b)
                // already); the exact count is search
                // dynamics, the existence is the claim.
                None => assert!(
                    n_verified >= 1,
                    "bodies are Verified under current+(b) without \
                     performing the skip ({} found)",
                    n_verified
                ),
            }
        }

        // (c) The all-whitespace-between property as a
        // Forall: statable in the dialect (the parser
        // accepts the formulation). At Phase 87 this
        // pin asserted it was UNENCODABLE through the
        // production encoder — Phase 94's forall_const
        // arm removes that boundary, and the pin flips
        // in place: the row now encodes. (Whether it
        // also DECIDES is Phase 94's battery, below;
        // the row's adoption into the spec stays R1's,
        // in Phase 96.)
        let forall_row = {
            let src = "T {\n spec probe {\n input: state: TokenizerState\n output: TokenizerState\n constraint: forall i: Int in range(len(output.source)): i < state.pos || i >= output.pos || is_whitespace(output.source[i])\n }\n}";
            let parsed = crate::spec::parser::parse(src).expect("forall row parses");
            parsed.functions[0].postconditions[0].clone()
        };
        assert!(
            matches!(forall_row, Constraint::Forall { .. }),
            "the all-whitespace-between formulation is statable: it parses \
             to a Forall, got {:?}",
            forall_row
        );
        let mut env = SymEnv::new();
        env.insert("state".into(), state_sym(" x", 0, 1, 1));
        env.insert("output".into(), state_sym(" x", 2, 1, 3));
        assert!(
            encode_constraint(&forall_row, &env, &EvalCtx::root(&index)).is_some(),
            "Phase 94: the Forall ENCODES through the production encoder \
             (the forall_const arm) — the boundary this pin recorded \
             through Phase 93 is removed"
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase76_char_range_side_condition_is_decisive() {
        // A Char parameter ranges over Unicode scalar
        // values ONLY because verify_constraint asserts the
        // range side-condition; without it both specs below
        // would be refutable (by a surrogate / out-of-range
        // "char"). The Int control proves the condition is
        // Char-specific.
        let src = r#"
CharRangeProbe {

spec within_unicode {
    input: c: Char
    output: Bool
    constraint: output == (c <= '\u{10FFFF}')
}

spec not_a_surrogate {
    input: c: Char
    output: Bool
    constraint: output == (c <= '\u{D7FF}' || c >= '\u{E000}')
}

spec int_control {
    input: n: Int
    output: Bool
    constraint: output == (n <= 1114111)
}

}
"#;
        let parsed = crate::spec::parser::parse(src).expect("probe parses");
        let module = crate::spec::typeck::check(parsed)
            .expect("probe typechecks")
            .spec;
        let verifier = Verifier::with_module(&module);
        for name in ["within_unicode", "not_a_surrogate"] {
            let spec = restored_spec(&module, name);
            let cand = candidate(&spec, 76, IRNode::Literal(IRLiteral::Bool(true)));
            assert_eq!(
                verifier.verify_one(&cand, &spec).status,
                VerificationStatus::Verified,
                "{}: true must verify ONLY via the Char range side-condition",
                name
            );
        }
        let spec = restored_spec(&module, "int_control");
        let cand = candidate(&spec, 76, IRNode::Literal(IRLiteral::Bool(true)));
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::VerificationFailed,
            "the Int control must be refuted: no side-condition leaks to Int"
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase76_bridge_boundary_indexed_char_vs_char_param() {
        // The bridge's named boundary: an indexed char
        // compares against Char LITERALS (and other indexed
        // chars), never against a symbolic Char parameter —
        // the z3 0.20 public API has no string-to-code
        // bridge between the sequence sort and the scalar
        // sort.
        let src = r#"
BridgeProbe {

spec idx_vs_param {
    input: s: String, c: Char
    output: Bool
    constraint: output == (s[0] == c)
}

spec idx_vs_lit {
    input: s: String
    output: Bool
    constraint: output == (s[0] == 'a')
}

spec any_output {
    input: s: String
    output: Bool
    constraint: output == true
}

}
"#;
        let parsed = crate::spec::parser::parse(src).expect("probe parses");
        let module = crate::spec::typeck::check(parsed)
            .expect("probe typechecks")
            .spec;
        let verifier = Verifier::with_module(&module);
        // Boundary: the constraint itself is unencodable.
        let spec = restored_spec(&module, "idx_vs_param");
        let cand = candidate(&spec, 76, IRNode::Literal(IRLiteral::Bool(true)));
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::Inconclusive,
            "indexed char vs Char parameter is the named bridge boundary"
        );
        // Inside the fragment: the same comparison against
        // a literal verifies with the mirror body.
        let spec = restored_spec(&module, "idx_vs_lit");
        let body = IRNode::BinOp(
            IRBinOp::Eq,
            Box::new(IRNode::Index(
                Box::new(IRNode::Var("s".into())),
                Box::new(IRNode::Literal(IRLiteral::Int(0))),
            )),
            Box::new(IRNode::Literal(IRLiteral::Char('a'))),
        );
        let cand = candidate(&spec, 76, body);
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::Verified,
            "indexed char vs Char literal must verify"
        );
        // A constant index into a literal string is EXACT:
        // in-bounds yields the real scalar (verified
        // against the true constraint); out-of-bounds is
        // unencodable, never totalized.
        let spec = restored_spec(&module, "any_output");
        let exact = IRNode::BinOp(
            IRBinOp::Eq,
            Box::new(IRNode::Index(
                Box::new(IRNode::Literal(IRLiteral::String("ab".into()))),
                Box::new(IRNode::Literal(IRLiteral::Int(1))),
            )),
            Box::new(IRNode::Literal(IRLiteral::Char('b'))),
        );
        let cand = candidate(&spec, 76, exact);
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::Verified,
            "constant index into a literal string is the exact scalar"
        );
        let oob = IRNode::BinOp(
            IRBinOp::Eq,
            Box::new(IRNode::Index(
                Box::new(IRNode::Literal(IRLiteral::String("ab".into()))),
                Box::new(IRNode::Literal(IRLiteral::Int(5))),
            )),
            Box::new(IRNode::Literal(IRLiteral::Char('b'))),
        );
        let cand = candidate(&spec, 76, oob);
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::Inconclusive,
            "constant out-of-bounds index is unencodable, not totalized"
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase76_record_params_fields_and_literals_verify() {
        let src = r#"
RecordProbe {

type P = { x: Int, y: Int }

spec move_x {
    input: p: P
    output: P
    constraint: output.x == p.x + 1
    constraint: output.y == p.y
}

spec move_x_lit {
    input: p: P
    output: P
    constraint: output == P { x: p.x + 1, y: p.y }
}

}
"#;
        let parsed = crate::spec::parser::parse(src).expect("probe parses");
        let module = crate::spec::typeck::check(parsed)
            .expect("probe typechecks")
            .spec;
        let verifier = Verifier::with_module(&module);
        let field = |name: &str| IRNode::FieldAccess {
            base: Box::new(IRNode::Var("p".into())),
            field: name.into(),
        };
        let good_body = IRNode::RecordLit {
            type_name: "P".into(),
            fields: vec![
                (
                    "x".into(),
                    IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(field("x")),
                        Box::new(IRNode::Literal(IRLiteral::Int(1))),
                    ),
                ),
                ("y".into(), field("y")),
            ],
        };
        for name in ["move_x", "move_x_lit"] {
            let spec = restored_spec(&module, name);
            let cand = candidate(&spec, 76, good_body.clone());
            assert_eq!(
                verifier.verify_one(&cand, &spec).status,
                VerificationStatus::Verified,
                "{}: the RecordLit body must verify against the record parameter",
                name
            );
        }
        // The identity body violates both statements.
        let spec = restored_spec(&module, "move_x");
        let cand = candidate(&spec, 76, IRNode::Var("p".into()));
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::VerificationFailed,
            "the identity body must be refuted"
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase76_guarded_unwrap_verifies_and_literal_err_unwrap_is_unencodable() {
        // The guarded shape the tokenizer uses: is_ok
        // guards unwrap; the payload encodes and the
        // postcondition decides.
        let src = r#"
UnwrapProbe {

spec guarded {
    input: r: Result<Int, String>
    output: Bool
    constraint: output == (r.is_err() || r.unwrap() == 3)
}

}
"#;
        let parsed = crate::spec::parser::parse(src).expect("probe parses");
        let module = crate::spec::typeck::check(parsed)
            .expect("probe typechecks")
            .spec;
        let verifier = Verifier::with_module(&module);
        let spec = restored_spec(&module, "guarded");
        // Body: the constraint's own RHS as a candidate is
        // judged through verify — instead judge two
        // constructed Result values via a body that returns
        // the guard expression itself is not expressible in
        // IR (no MethodCall kind); use constant bodies:
        // `true` is refuted (an Ok(4) input falsifies it),
        // proving unwrap encoded rather than vanished.
        let cand = candidate(&spec, 76, IRNode::Literal(IRLiteral::Bool(true)));
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::VerificationFailed,
            "constant true must be refuted: unwrap of Ok(4) is 4, not 3"
        );
        // unwrap of a literal Err construction has no Ok
        // payload to denote: the constraint is unencodable
        // (the named case), so the verdict is Inconclusive
        // for every body.
        let hand = FunctionSpec {
            name: "unwrap_of_err".into(),
            doc: None,
            generics: vec![],
            params: vec![],
            return_type: TypeRef::Int,
            preconditions: vec![],
            postconditions: vec![Constraint::Expr(SpecExpr::BinOp(
                SpecBinOp::Eq,
                Box::new(SpecExpr::Var("output".into())),
                Box::new(SpecExpr::MethodCall {
                    receiver: Box::new(SpecExpr::Call(
                        "Err".into(),
                        vec![SpecExpr::StringLit("x".into())],
                    )),
                    name: "unwrap".into(),
                    args: vec![],
                }),
            ))],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: Default::default(),
        };
        let cand = candidate(&hand, 76, IRNode::Literal(IRLiteral::Int(0)));
        assert_eq!(
            verifier.verify_one(&cand, &hand).status,
            VerificationStatus::Inconclusive,
            "unwrap of a literal Err is unencodable by construction"
        );
    }

    // ────────────────────────────────────────────────────
    // Phase 91 — ENCODER EXTENSION small arms.
    //
    // The four S-priced arms of ENCODER_EXTENSION_DESIGN.md
    // §4, promoted from the Phase 90 probes that measured
    // their boundaries in situ (p90_match's constructor
    // arms, the probe's Clone elision, p90_ite's (List,
    // List) arm, p90_append's ListLen and (Str, CharStr)
    // forms):
    //   W2  constructor-pattern arms in sym_match, against
    //       the Phase 76 tag+payload Sym model;
    //   W3  UnaryOp::Clone as the identity in sym_unary_ir;
    //   W4  a (List, List) elementwise arm in sym_ite;
    //   W5  Append over ListLen (n → n+1) and (Str,
    //       CharStr) concat in sym_binop.
    // Each test pins the flip (a body that was Inconclusive
    // now judges), the teeth (a wrong body Refutes — never
    // Proven), and the declines (shapes the Sym model
    // cannot represent exactly stay Inconclusive).
    // ────────────────────────────────────────────────────

    #[cfg(feature = "z3-support")]
    const PHASE91_MODULE: &str = r#"
Phase91Arms {

spec opt_flag {
    input: o: Option<Int>
    output: Int

    constraint: o==None()||output==1
    constraint: o!=None()||output==0
}

spec opt_id {
    input: o: Option<Int>
    output: Option<Int>

    constraint: output==o
}

spec res_or_zero {
    input: r: Result<Int, String>
    output: Int

    constraint: r.is_err()||output==r.unwrap()
    constraint: r.is_ok()||output==0
}

spec clone_id {
    input: x: Int
    output: Int

    constraint: output==x
}

spec pick_first {
    input: b: Bool
    output: Int

    constraint: b==false||output==1
    constraint: b==true||output==3
}

spec push_len {
    input: xs: List<Int>
    output: Int

    constraint: output==len(xs)+1
}

spec grow_first {
    input: s: String
    output: String

    constraint: len(s)==0||len(output)==len(s)+1
}

}
"#;

    #[cfg(feature = "z3-support")]
    fn load_phase91_module() -> Spec {
        let parsed = crate::spec::parser::parse(PHASE91_MODULE)
            .expect("phase 91 module parses");
        crate::spec::typeck::check(parsed)
            .expect("phase 91 module typechecks")
            .spec
    }

    #[cfg(feature = "z3-support")]
    fn ctor(name: &str, pargs: Vec<IRPattern>, body: IRNode) -> (IRPattern, IRNode) {
        (IRPattern::Constructor(name.into(), pargs), body)
    }

    #[cfg(feature = "z3-support")]
    fn call(name: &str, args: Vec<IRNode>) -> IRNode {
        IRNode::Call {
            function: name.into(),
            args,
        }
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase91_constructor_match_option_arms_verify_and_refute() {
        let module = load_phase91_module();
        let flag = restored_spec(&module, "opt_flag");
        // The canonical shape the Phase 90 probe priced
        // (p90_match): discriminate on the tag. Production
        // sym_match skipped Constructor arms wholesale, so
        // this body was Inconclusive before the arm.
        let good = match_on(
            var("o"),
            vec![
                ctor("Some", vec![IRPattern::Wildcard], int(1)),
                ctor("None", vec![], int(0)),
            ],
        );
        assert_eq!(status_of(flag, good), VerificationStatus::Verified);
        // Teeth: swapped arms are a different function —
        // Refuted, not Proven and not merely unjudged.
        let swapped = match_on(
            var("o"),
            vec![
                ctor("Some", vec![IRPattern::Wildcard], int(0)),
                ctor("None", vec![], int(1)),
            ],
        );
        assert_eq!(
            status_of(flag, swapped),
            VerificationStatus::VerificationFailed
        );

        // The payload bind is load-bearing: the identity
        // over Option through a constructor match verifies
        // only if the bound name denotes the Some payload
        // itself.
        let id = restored_spec(&module, "opt_id");
        let good_id = match_on(
            var("o"),
            vec![
                ctor(
                    "Some",
                    vec![IRPattern::Variable("x".into())],
                    call("Some", vec![var("x")]),
                ),
                ctor("None", vec![], call("None", vec![])),
            ],
        );
        assert_eq!(status_of(id, good_id), VerificationStatus::Verified);
        // Teeth: a fabricated payload refutes.
        let fabricated = match_on(
            var("o"),
            vec![
                ctor(
                    "Some",
                    vec![IRPattern::Variable("x".into())],
                    call("Some", vec![int(0)]),
                ),
                ctor("None", vec![], call("None", vec![])),
            ],
        );
        assert_eq!(
            status_of(id, fabricated),
            VerificationStatus::VerificationFailed
        );
        // Teeth: arms landing on the wrong tag side
        // refute (each side builds the other constructor).
        let wrong_side = match_on(
            var("o"),
            vec![
                ctor(
                    "Some",
                    vec![IRPattern::Variable("x".into())],
                    call("None", vec![]),
                ),
                ctor("None", vec![], call("Some", vec![int(0)])),
            ],
        );
        assert_eq!(
            status_of(id, wrong_side),
            VerificationStatus::VerificationFailed
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase91_constructor_match_result_arms_verify_and_refute() {
        let module = load_phase91_module();
        let spec = restored_spec(&module, "res_or_zero");
        let good = match_on(
            var("r"),
            vec![
                ctor("Ok", vec![IRPattern::Variable("x".into())], var("x")),
                ctor("Err", vec![IRPattern::Wildcard], int(0)),
            ],
        );
        assert_eq!(status_of(spec, good), VerificationStatus::Verified);
        // Teeth: dropping the Ok payload refutes — row 1
        // reads it back through unwrap.
        let dropped = match_on(
            var("r"),
            vec![
                ctor("Ok", vec![IRPattern::Wildcard], int(0)),
                ctor("Err", vec![IRPattern::Wildcard], int(0)),
            ],
        );
        assert_eq!(
            status_of(spec, dropped),
            VerificationStatus::VerificationFailed
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase91_constructor_match_declines_stay_inconclusive() {
        let module = load_phase91_module();
        let flag = restored_spec(&module, "opt_flag");
        // A match that can fall through (no None arm, no
        // unconditional arm) has no defined output on the
        // missing side: unencodable, as before.
        let fallthrough = match_on(
            var("o"),
            vec![ctor("Some", vec![IRPattern::Wildcard], int(1))],
        );
        assert_eq!(
            status_of(flag, fallthrough),
            VerificationStatus::Inconclusive
        );
        // A nested sub-pattern (a literal inside Some) is
        // outside the tag+payload model: decline, never an
        // approximation.
        let nested = match_on(
            var("o"),
            vec![
                ctor(
                    "Some",
                    vec![IRPattern::Literal(IRLiteral::Int(5))],
                    int(1),
                ),
                ctor("None", vec![], int(0)),
            ],
        );
        assert_eq!(
            status_of(flag, nested),
            VerificationStatus::Inconclusive
        );
        // A constructor of the other tagged form over an
        // Option scrutinee has no Sym representation:
        // decline.
        let wrong_form = match_on(
            var("o"),
            vec![
                ctor("Ok", vec![IRPattern::Variable("x".into())], int(1)),
                ctor("None", vec![], int(0)),
            ],
        );
        assert_eq!(
            status_of(flag, wrong_form),
            VerificationStatus::Inconclusive
        );
    }

    /// The headline shape: a Match over `peek(state)`
    /// itself — the canonical loop-condition form of the
    /// tokenizer bodies (design doc W2). The scrutinee is
    /// an inlined callee value (a free-tag Option built by
    /// ite), and the match rebuilds it arm by arm; peek's
    /// rows pin the output exactly, so the identity over
    /// peek must Verify outright.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase91_match_over_peek_call_verifies() {
        let module = load_restored_module();
        let spec = restored_spec(&module, "peek");
        let verifier = Verifier::with_module(&module);
        let body = match_on(
            call("peek", vec![var("state")]),
            vec![
                ctor(
                    "Some",
                    vec![IRPattern::Variable("ch".into())],
                    call("Some", vec![var("ch")]),
                ),
                ctor("None", vec![], call("None", vec![])),
            ],
        );
        let cand = candidate(spec, 91, body);
        assert_eq!(
            verifier.verify_one(&cand, spec).status,
            VerificationStatus::Verified,
            "match over peek(state) must verify once constructor arms encode"
        );
        // Teeth: the same match with the arms' values
        // exchanged is a different function — Refuted.
        let swapped = match_on(
            call("peek", vec![var("state")]),
            vec![
                ctor(
                    "Some",
                    vec![IRPattern::Variable("ch".into())],
                    call("None", vec![]),
                ),
                ctor(
                    "None",
                    vec![],
                    call("Some", vec![IRNode::Literal(IRLiteral::Char('x'))]),
                ),
            ],
        );
        let cand = candidate(spec, 91, swapped);
        assert_eq!(
            verifier.verify_one(&cand, spec).status,
            VerificationStatus::VerificationFailed
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase91_clone_is_the_identity() {
        let module = load_phase91_module();
        let spec = restored_spec(&module, "clone_id");
        // The probe's Clone elision, promoted: a value
        // copy is the value (Sym terms are immutable), so
        // Clone(x) judges exactly as x does. Before the
        // arm this body was Inconclusive.
        let cloned = IRNode::UnaryOp(IRUnaryOp::Clone, Box::new(var("x")));
        assert_eq!(status_of(spec, cloned), VerificationStatus::Verified);
        // Teeth: a clone of a different value is that
        // value, not x — Refuted.
        let cloned_other = IRNode::UnaryOp(
            IRUnaryOp::Clone,
            Box::new(binop(IRBinOp::Add, var("x"), int(1))),
        );
        assert_eq!(
            status_of(spec, cloned_other),
            VerificationStatus::VerificationFailed
        );
        // Clone composes over a compound value: cloning
        // peek's result whole still verifies against
        // peek's rows (the hand bodies' `peek(Clone …)`
        // shape family).
        let module = load_restored_module();
        let peek = restored_spec(&module, "peek");
        let verifier = Verifier::with_module(&module);
        let body = IRNode::UnaryOp(
            IRUnaryOp::Clone,
            Box::new(call("peek", vec![var("state")])),
        );
        let cand = candidate(peek, 91, body);
        assert_eq!(
            verifier.verify_one(&cand, peek).status,
            VerificationStatus::Verified
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase91_ite_merges_concrete_lists_elementwise() {
        let module = load_phase91_module();
        let spec = restored_spec(&module, "pick_first");
        let list = |items: Vec<i64>| call("List", items.into_iter().map(int).collect());
        let pick = |then_items: Vec<i64>, else_items: Vec<i64>| {
            IRNode::Index(
                Box::new(IRNode::If {
                    condition: Box::new(var("b")),
                    then_branch: Box::new(list(then_items)),
                    else_branch: Box::new(list(else_items)),
                }),
                Box::new(int(0)),
            )
        };
        // Merging two concrete lists elementwise is what
        // a state record's carried list field needs; with
        // no (List, List) arm the If was unencodable.
        assert_eq!(
            status_of(spec, pick(vec![1, 2], vec![3, 4])),
            VerificationStatus::Verified
        );
        // Teeth: branches exchanged — Refuted.
        assert_eq!(
            status_of(spec, pick(vec![3, 4], vec![1, 2])),
            VerificationStatus::VerificationFailed
        );
        // Decline: lists of different lengths have no
        // merged length — the arm must not invent one.
        assert_eq!(
            status_of(spec, pick(vec![1, 2], vec![3, 4, 5])),
            VerificationStatus::Inconclusive
        );
    }

    #[cfg(feature = "z3-support")]
    #[test]
    fn phase91_append_arms_length_and_charstr() {
        let module = load_phase91_module();
        // Length-only: a List parameter binds as ListLen;
        // Append grows the length by exactly one, whatever
        // the (unmodelled) elements are.
        let spec = restored_spec(&module, "push_len");
        let grown = call("len", vec![binop(IRBinOp::Append, var("xs"), int(7))]);
        assert_eq!(status_of(spec, grown), VerificationStatus::Verified);
        // Teeth: two appends grow by two — Refuted
        // against the +1 row; zero appends likewise.
        let twice = call(
            "len",
            vec![binop(
                IRBinOp::Append,
                binop(IRBinOp::Append, var("xs"), int(7)),
                int(8),
            )],
        );
        assert_eq!(
            status_of(spec, twice),
            VerificationStatus::VerificationFailed
        );
        assert_eq!(
            status_of(spec, call("len", vec![var("xs")])),
            VerificationStatus::VerificationFailed
        );

        // (Str, CharStr): the lexeme shape — a string
        // grown by an indexed char grows by that char
        // (the bridge's totalization: at an empty string
        // the indexed char is empty and nothing grows —
        // the row's len(s)==0 guard states exactly that).
        let spec = restored_spec(&module, "grow_first");
        let body = binop(
            IRBinOp::Append,
            var("s"),
            IRNode::Index(Box::new(var("s")), Box::new(int(0))),
        );
        assert_eq!(status_of(spec, body), VerificationStatus::Verified);
        // Teeth: the string alone does not grow — Refuted.
        assert_eq!(
            status_of(spec, var("s")),
            VerificationStatus::VerificationFailed
        );
        // Decline: appending a Char SCALAR (not the
        // bridge's CharStr form) is not one of this
        // phase's arms — it stays unencodable rather
        // than guessing a conversion.
        let scalar = binop(
            IRBinOp::Append,
            var("s"),
            IRNode::Literal(IRLiteral::Char('x')),
        );
        assert_eq!(
            status_of(spec, scalar),
            VerificationStatus::Inconclusive
        );
    }

    // ────────────────────────────────────────────────────
    // Phase 92 — resolver denotations, judged.
    //
    // The chain entries land in src/synthesis/engine.rs
    // (the phase92_* tests there pin the bodies and the
    // decline teeth); these tests pin what the
    // denotations DO in the verifier: next_char's
    // denotation self-verifies with peek composed inside
    // it, bodies that call next_char judge for the first
    // time, the statement-mode denotations (skip, step)
    // manufacture no verdicts, and the Phase 90 BMC
    // discipline runs unchanged on PRODUCTION
    // denotations instead of the probe's hand-supplied
    // map.
    // ────────────────────────────────────────────────────

    /// Phase 92: next_char denotes (record completion,
    /// dual-registered), and the denotation SELF-VERIFIES
    /// against next_char's own rows — the peek precedent
    /// (Phase 81) one level deeper: the denoted body's
    /// guard calls peek(state), peek denotes through the
    /// complementary-guard builder, and the composition
    /// resolves end to end. Before this phase the same
    /// judgments were Inconclusive by construction:
    /// callee_body("next_char") was None, so no body that
    /// called next_char was Z3-evaluable at all (the
    /// design doc's finding 2).
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase92_next_char_denotation_self_verifies() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let spec = restored_spec(&module, "next_char").clone();
        let denotation = index
            .callee_body("next_char")
            .expect("next_char denotes after Phase 92");
        // The pure encodability mirror agrees: the
        // denotation is expression-mode (a Let over a
        // RecordLit of Ifs), fully inside the encodable
        // subset.
        assert!(
            body_is_encodable_with(&spec, &denotation, &index),
            "next_char's denotation must be encodable (expression-mode body)"
        );
        // Self-verification, timed: the composition the
        // design doc warned about (guided decisions
        // inside nested symbolic denotations reaching
        // the 5 s per-constraint cap) is measured here on
        // the production path — verify_one over the
        // denotation, whose guard invokes peek's
        // denotation per row.
        let started = std::time::Instant::now();
        let cand = candidate(&spec, 9201, denotation);
        let result = verifier.verify_one(&cand, &spec);
        let elapsed_ms = started.elapsed().as_millis();
        assert_eq!(
            result.status,
            VerificationStatus::Verified,
            "next_char's denotation must self-verify against its own rows"
        );
        println!(
            "P92|next_char-self-verify|total_ms={}",
            elapsed_ms
        );
        // A body that merely CALLS next_char now judges:
        // the bare call, as a candidate for next_char
        // itself, verifies — the call's meaning is the
        // denotation, evaluated in the caller's context.
        let call_body = IRNode::Call {
            function: "next_char".to_string(),
            args: vec![IRNode::Var("state".to_string())],
        };
        let cand = candidate(&spec, 9202, call_body);
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::Verified,
            "a body calling next_char must judge through its denotation"
        );
        // Teeth: the denotation is semantically
        // load-bearing, not a rubber stamp — advancing
        // TWICE through it refutes against the rows
        // (output.pos == state.pos + 1 fails at +2).
        let twice = IRNode::Call {
            function: "next_char".to_string(),
            args: vec![IRNode::Call {
                function: "next_char".to_string(),
                args: vec![IRNode::Var("state".to_string())],
            }],
        };
        let cand = candidate(&spec, 9203, twice);
        assert_eq!(
            verifier.verify_one(&cand, &spec).status,
            VerificationStatus::VerificationFailed,
            "next_char(next_char(state)) must REFUTE against next_char's rows"
        );
    }

    /// Phase 92 teeth: a denotation is a meaning, not a
    /// verdict. The two statement-mode denotations
    /// (skip, step) are available to every consumer from
    /// today, but sym_eval still has no While/Loop arm
    /// (Phase 93), so anything whose judgment must pass
    /// through them stays Inconclusive — never Verified.
    ///
    /// Phase 93 re-pin (reason recorded, per the Phase
    /// 92 precedent): the Phase 93 statement encoder now
    /// judges through both denotations, and the two
    /// halves land where the design says they must. The
    /// skip half's status is UNCHANGED (Inconclusive) but
    /// reached by judging: the While body evaluates, and
    /// its rows judge Unknown because exhaustion is
    /// reachable at the production fuel — not because
    /// nothing could evaluate it. The step half's status
    /// MOVES, Inconclusive → VerificationFailed: row 2
    /// is Refuted on a completed path (the `" 5"`
    /// countermodel, ENCODER_EXTENSION_DESIGN.md §2.4)
    /// while row 1 stays Unknown (exhaustion). That is
    /// Layer 1's designed product — decisive verdicts
    /// where they exist — not a denotation smuggling a
    /// pass: the step body is refuted by its own spec's
    /// standing rows, and the residue is R2's
    /// ratification (Phase 96), not an encoder gap.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase92_statement_denotations_manufacture_no_verdicts() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        // A candidate for skip_whitespace whose whole
        // body is the call to skip: the resolver supplies
        // the While body and (Phase 93) the statement
        // encoder evaluates it — the verdict is
        // Inconclusive by exhaustion, the honest outcome
        // for an unbounded loop judged symbolically.
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let call_skip = IRNode::Call {
            function: "skip_whitespace".to_string(),
            args: vec![IRNode::Var("state".to_string())],
        };
        assert!(
            !body_is_encodable_with(&skip, &call_skip, &index),
            "a call through the While denotation stays outside the encodable subset"
        );
        let cand = candidate(&skip, 9211, call_skip);
        assert_eq!(
            verifier.verify_one(&cand, &skip).status,
            VerificationStatus::Inconclusive,
            "judging through skip's denotation lands Inconclusive: \
             exhaustion is reachable at the production fuel"
        );
        // The step denotation itself, as a candidate for
        // tokenize_one: judged by the Phase 93 statement
        // encoder and REFUTED by standing row 2 (the
        // `" 5"` countermodel) — see the re-pin note
        // above.
        let one = restored_spec(&module, "tokenize_one").clone();
        let step = index
            .callee_body("tokenize_one")
            .expect("tokenize_one denotes after Phase 92");
        assert!(
            !body_is_encodable_with(&one, &step, &index),
            "the STEP denotation stays outside the encodable subset (Loop/While)"
        );
        let cand = candidate(&one, 9212, step);
        assert_eq!(
            verifier.verify_one(&cand, &one).status,
            VerificationStatus::VerificationFailed,
            "the STEP denotation is REFUTED by standing row 2 through \
             the Phase 93 statement encoder (the \" 5\" countermodel)"
        );
    }

    /// Phase 92: the Phase 90 BMC discipline, re-run
    /// with the denotation map supplied ENTIRELY by
    /// production `SpecIndex::callee_body` (Phase 90
    /// hand-supplied the pool seed for next_char and a
    /// test-local transcription for skip). The verdicts
    /// must be Phase 90's, point for point: skip 12/12
    /// at fuel = trip count with row (a) Proven at every
    /// point; the STEP body 10/10 with rows Proven at Ok
    /// points; and the " 5" diagnostic unchanged — row 2
    /// still Refutes the faithful body, now through
    /// production resolver meanings end to end (the R2
    /// ratification's subject, not an encoder gap).
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase92_bmc_points_run_on_production_denotations() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let mut denotations = std::collections::HashMap::new();
        for name in ["next_char", "skip_whitespace", "tokenize_one"] {
            let body = index
                .callee_body(name)
                .unwrap_or_else(|| panic!("{} denotes after Phase 92", name));
            denotations.insert(name.to_string(), body);
        }
        // ── skip: the production denotation, judged
        // directly (its callees resolve through the
        // production chain: peek by sym_call, next_char
        // by sym_call first and the map as fallback).
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let skip_body = denotations["skip_whitespace"].clone();
        let row_a = p90_parse_row(
            "output.pos >= len(output.source) || is_whitespace(output.source[output.pos]) == false",
        );
        let mut spec_a = skip.clone();
        spec_a.postconditions.push(row_a.clone());
        let battery: Vec<P90St> = vec![
            p90_st("", 0, 1, 1),
            p90_st(" ", 0, 1, 1),
            p90_st("  \t\n x", 0, 1, 1),
            p90_st("no-ws", 0, 1, 1),
            p90_st("\n\nab", 0, 1, 1),
            p90_st(" \u{A0}x", 0, 1, 1),
            p90_st("a b", 0, 1, 1),
            p90_st("\r\n\t", 0, 1, 1),
            p90_st("a b", 1, 1, 2),
            p90_st("  \t\n x", 2, 1, 3),
            p90_st("\n\nab", 1, 1, 2),
            p90_st(" \u{A0}x", 1, 1, 2),
        ];
        for (i, input) in battery.iter().enumerate() {
            let (mirror, trips) = p90_mirror_skip(input);
            let expected = p90_state_sym(&mirror);
            let ctx = P90Ctx {
                index: &index,
                denotations: &denotations,
                fuel: trips,
                depth: 0,
                list_append: true,
            };
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            let outcome = p90_run(&skip_body, env.clone(), &ctx)
                .expect("the production skip denotation evaluates in statement mode");
            let out = p90_concretize(&outcome).unwrap_or_else(|| {
                panic!(
                    "point {} ({:?} from {}): the production skip denotation \
                     must resolve at the trip count {}",
                    i, input.source, input.pos, trips
                )
            });
            assert!(
                p90_values_equal(&out, &expected),
                "point {} ({:?} from {}): the production denotation's output \
                 must equal the mirror's",
                i, input.source, input.pos
            );
            for (j, row) in skip.postconditions.iter().enumerate() {
                let (res, tri) = verifier.verify_constraint(
                    row, "p92@skip-current", &skip, &env, &Some(out.clone()),
                );
                assert!(
                    matches!(tri, Tri::Proven),
                    "point {} current row {} must be Proven, got solver {} ms",
                    i, j, res.solver_time_ms
                );
            }
            let (res, tri) = verifier.verify_constraint(
                &row_a, "p92@skip-a", &spec_a, &env, &Some(out.clone()),
            );
            assert!(
                matches!(tri, Tri::Proven),
                "point {} row (a) must be Proven for the production \
                 denotation, got solver {} ms",
                i, res.solver_time_ms
            );
            println!(
                "P92|skip-point|{}|src={:?}|from={}|trips={}|row_a_ms={}",
                i, input.source, input.pos, trips, res.solver_time_ms
            );
        }
        // ── STEP: the production denotation at the
        // 10-point battery (skip and next_char resolve
        // through production meanings).
        let spec = restored_spec(&module, "tokenize_one").clone();
        let step_body = denotations["tokenize_one"].clone();
        let step_battery: Vec<P90St> = vec![
            p90_st("spec", 0, 1, 1),
            p90_st("", 0, 1, 1),
            p90_st("  spec", 0, 1, 1),
            p90_st(":", 0, 1, 1),
            p90_st("{", 0, 1, 1),
            p90_st("=", 0, 1, 1),
            p90_st("5", 0, 1, 1),
            p90_st(" ", 0, 1, 1),
            p90_st("a b", 0, 1, 1),
            p90_st("\n\nx", 0, 1, 1),
        ];
        let run_step = |input: &P90St, fuel: usize| {
            let ctx = P90Ctx {
                index: &index,
                denotations: &denotations,
                fuel,
                depth: 0,
                list_append: true,
            };
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            p90_run(&step_body, env, &ctx)
        };
        for (i, input) in step_battery.iter().enumerate() {
            let (mirror, sk, id) = p90_mirror_one(input);
            let pred = if id >= 1 { sk.max(id + 1) } else { sk };
            let outcome = run_step(input, pred)
                .expect("the production STEP denotation evaluates in statement mode");
            let out = p90_concretize(&outcome).unwrap_or_else(|| {
                panic!(
                    "point {} ({:?}): the production STEP denotation must \
                     resolve at the predicted fuel {}",
                    i, input.source, pred
                )
            });
            match (&mirror, &out) {
                (Ok(_), Sym::Result { ok: Tag::True, .. }) => {
                    assert!(
                        p90_values_equal(&out, &p90_result_sym(&mirror)),
                        "point {} ({:?}): the STEP output must equal the mirror's",
                        i, input.source
                    );
                }
                (Err(_), Sym::Result { ok: Tag::False, .. }) => {
                    // Tag agreement; the message text is
                    // the standing F2 deviation.
                }
                _ => panic!(
                    "point {} ({:?}): tag disagreement with the mirror",
                    i, input.source
                ),
            }
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            let is_err = matches!(&mirror, Err(_));
            for (j, row) in spec.postconditions.iter().enumerate() {
                let (res, tri) = verifier.verify_constraint(
                    row, "p92@step", &spec, &env, &Some(out.clone()),
                );
                if is_err && j == 0 {
                    assert!(
                        matches!(tri, Tri::Unknown),
                        "point {} ({:?}) row 0 at an Err output must judge \
                         Unknown (literal-Err unwrap boundary), got solver {} ms",
                        i, input.source, res.solver_time_ms
                    );
                    continue;
                }
                assert!(
                    matches!(tri, Tri::Proven),
                    "point {} ({:?}) row {} must be Proven for the production \
                     STEP denotation, got solver {} ms",
                    i, input.source, j, res.solver_time_ms
                );
            }
            println!(
                "P92|step-point|{}|src={:?}|skip_trips={}|ident_trips={}|min_fuel={}",
                i, input.source, sk, id, pred
            );
        }
        // ── LOOP: tokenize_all's candidate body (the
        // pool template body — tokenize_all denotes
        // nothing itself) calling the PRODUCTION
        // tokenize_one denotation through
        // TryPropagate: the composition tokenize_all's
        // rows will be judged against in Phase 93. The
        // verdicts are Phase 90's, source for source.
        let all_spec = restored_spec(&module, "tokenize_all").clone();
        let loop_body = p90_template_body(&module, "tokenize_all");
        for src in ["", "spec sort", "spec{input:()}", "spec 5", "  \n ", "a", "x{y}:(,)", "a\nb"] {
            let input = p90_st(src, 0, 1, 1);
            let (mirror, outer, inner) = p90_mirror_all(&input);
            let pred = outer.max(inner);
            let ctx = P90Ctx {
                index: &index,
                denotations: &denotations,
                fuel: pred,
                depth: 0,
                list_append: true,
            };
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(&input));
            let outcome = p90_run(&loop_body, env.clone(), &ctx)
                .expect("the LOOP body evaluates over the production step denotation");
            let out = p90_concretize(&outcome).unwrap_or_else(|| {
                panic!(
                    "{:?}: the LOOP body must resolve at the predicted fuel {} \
                     over production denotations",
                    src, pred
                )
            });
            match (&mirror, &out) {
                (Ok(toks), Sym::Result { ok: Tag::True, ok_payload, .. }) => {
                    let expected = Sym::List(toks.iter().map(p90_token_sym).collect());
                    let payload = ok_payload.as_ref().expect("Ok payload");
                    assert!(
                        p90_values_equal(payload, &expected),
                        "{:?}: the LOOP output token list must equal the mirror's",
                        src
                    );
                }
                (Err(_), Sym::Result { ok: Tag::False, .. }) => {
                    // Tag agrees; the message text is the
                    // F2 deviation, not judged here.
                }
                _ => panic!("{:?}: tag disagreement with the mirror", src),
            }
            let (res, tri) = verifier.verify_constraint(
                &all_spec.postconditions[0], "p92@loop", &all_spec, &env, &Some(out),
            );
            if matches!(&mirror, Err(_)) {
                assert!(
                    matches!(tri, Tri::Unknown),
                    "{:?}: tokenize_all's row at an Err output must judge \
                     Unknown (literal-Err unwrap boundary), got solver {} ms",
                    src, res.solver_time_ms
                );
            } else {
                assert!(
                    matches!(tri, Tri::Proven),
                    "{:?}: tokenize_all's row must be Proven for the LOOP body \
                     over production denotations, got solver {} ms",
                    src, res.solver_time_ms
                );
            }
            println!(
                "P92|loop-point|src={:?}|outer_iters={}|inner_max={}|min_fuel={}|row_ms={}",
                src, outer, inner, pred, res.solver_time_ms
            );
        }
        // The diagnostic point, through production
        // denotations end to end: the faithful body errs
        // at " 5" and standing row 2 Refutes it — the
        // residue is R2's β-skip ratification (Phase 96),
        // not an encoder gap.
        let diag = p90_st(" 5", 0, 1, 1);
        let outcome = run_step(&diag, 2).expect("STEP evaluates at the diagnostic point");
        let out = p90_concretize(&outcome).expect("the diagnostic point resolves");
        assert!(
            matches!(&out, Sym::Result { ok: Tag::False, .. }),
            "the faithful body errs at \" 5\""
        );
        let mut env = SymEnv::new();
        env.insert("state".into(), p90_state_sym(&diag));
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[1], "p92@diag-row2", &spec, &env, &Some(out.clone()),
        );
        assert!(
            matches!(tri, Tri::Refuted),
            "row 2 REFUTES the faithful body at \" 5\" through production \
             denotations: the residue is the R2-β-skip ratification"
        );
        println!("P92|step-diag|\" 5\": row 2 Refuted through production denotations");
    }

    // ────────────────────────────────────────────────────
    // Phase 93 — the statement-mode encoder in
    // production: the Phase 90 probe batteries re-run
    // through `sym_eval_stmt` + `verify_constraint_stmt`
    // (the production evaluator and judge), on the
    // production denotations of Phase 92. The probe
    // suite below stands as the measurement record;
    // these tests are the regression form of the same
    // evidence.
    // ────────────────────────────────────────────────────

    /// Judge one row over one statement outcome through
    /// the production statement judge, returning the
    /// verdict and the judge's solver time.
    #[cfg(feature = "z3-support")]
    fn p93_judge(
        verifier: &Verifier,
        row: &Constraint,
        spec: &FunctionSpec,
        env: &SymEnv,
        outcome: &StmtOutcome,
    ) -> (Tri, u64) {
        let (res, tri) = verifier.verify_constraint_stmt(row, "p93", spec, env, outcome);
        (tri, res.solver_time_ms)
    }

    /// Phase 93: the skip battery at points, through the
    /// production statement encoder — the production
    /// denotation body (Phase 92) resolves at exactly
    /// the trip count, its outputs equal the mirror's,
    /// and the production statement judge Proves the
    /// current rows and strengthening row (a) at all 12
    /// points (the BMC result Phase 92 reproduced
    /// through the probe, now through production code).
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase93_stmt_skip_points_battery() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let body = index
            .callee_body("skip_whitespace")
            .expect("skip denotes after Phase 92");
        let row_a = p90_parse_row(
            "output.pos >= len(output.source) || is_whitespace(output.source[output.pos]) == false",
        );
        let mut spec_a = skip.clone();
        spec_a.postconditions.push(row_a.clone());
        let battery: Vec<P90St> = vec![
            p90_st("", 0, 1, 1),
            p90_st(" ", 0, 1, 1),
            p90_st("  \t\n x", 0, 1, 1),
            p90_st("no-ws", 0, 1, 1),
            p90_st("\n\nab", 0, 1, 1),
            p90_st(" \u{A0}x", 0, 1, 1),
            p90_st("a b", 0, 1, 1),
            p90_st("\r\n\t", 0, 1, 1),
            p90_st("a b", 1, 1, 2),
            p90_st("  \t\n x", 2, 1, 3),
            p90_st("\n\nab", 1, 1, 2),
            p90_st(" \u{A0}x", 1, 1, 2),
        ];
        for (i, input) in battery.iter().enumerate() {
            let (mirror, trips) = p90_mirror_skip(input);
            let expected = p90_state_sym(&mirror);
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            // Below the trip count the live path is cut
            // off: no concretization (the fuel
            // discipline, in production).
            if trips >= 1 {
                let ctx = StmtCtx::root(&index, trips - 1, 5_000);
                let short = sym_eval_stmt(&body, env.clone(), &ctx)
                    .expect("the production skip denotation evaluates below the trip count");
                assert!(
                    stmt_concretize(&short, 5_000).is_none(),
                    "point {} ({:?} from {}): fuel {} (one below the trip count {}) \
                     must NOT resolve — the live path is exhausted there",
                    i, input.source, input.pos, trips - 1, trips
                );
            }
            let ctx = StmtCtx::root(&index, trips, 5_000);
            let outcome = sym_eval_stmt(&body, env.clone(), &ctx)
                .expect("the production skip denotation evaluates in statement mode");
            let out = stmt_concretize(&outcome, 5_000).unwrap_or_else(|| {
                panic!(
                    "point {} ({:?} from {}): the production skip denotation \
                     must resolve at the trip count {}",
                    i, input.source, input.pos, trips
                )
            });
            assert!(
                p90_values_equal(&out, &expected),
                "point {} ({:?} from {}): the production denotation's output \
                 must equal the mirror's",
                i, input.source, input.pos
            );
            let mut judge_ms = 0u64;
            for (j, row) in skip.postconditions.iter().enumerate() {
                let (tri, ms) = p93_judge(&verifier, row, &skip, &env, &outcome);
                judge_ms += ms;
                assert!(
                    matches!(tri, Tri::Proven),
                    "point {} current row {} must be Proven through the \
                     production statement judge",
                    i, j
                );
            }
            let (tri, ms) = p93_judge(&verifier, &row_a, &spec_a, &env, &outcome);
            judge_ms += ms;
            assert!(
                matches!(tri, Tri::Proven),
                "point {} row (a) must be Proven through the production \
                 statement judge",
                i
            );
            println!(
                "P93|skip-point|{}|src={:?}|from={}|trips={}|judge_ms={}",
                i, input.source, input.pos, trips, judge_ms
            );
        }
    }

    /// Phase 93: the STEP battery at points, through the
    /// production statement encoder — the production
    /// STEP denotation resolves at the predicted fuels,
    /// its Ok outputs equal the mirror's, and the
    /// production statement judge Proves both rows at Ok
    /// points; at the Err point row 1 (index 0) judges
    /// Unknown (the W8 literal-Err unwrap boundary) and
    /// row 2 (index 1) Proven.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase93_stmt_step_points_battery() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let spec = restored_spec(&module, "tokenize_one").clone();
        let body = index
            .callee_body("tokenize_one")
            .expect("tokenize_one denotes after Phase 92");
        let battery: Vec<P90St> = vec![
            p90_st("spec", 0, 1, 1),
            p90_st("", 0, 1, 1),
            p90_st("  spec", 0, 1, 1),
            p90_st(":", 0, 1, 1),
            p90_st("{", 0, 1, 1),
            p90_st("=", 0, 1, 1),
            p90_st("5", 0, 1, 1),
            p90_st(" ", 0, 1, 1),
            p90_st("a b", 0, 1, 1),
            p90_st("\n\nx", 0, 1, 1),
        ];
        for (i, input) in battery.iter().enumerate() {
            let (mirror, sk, id) = p90_mirror_one(input);
            let pred = if id >= 1 { sk.max(id + 1) } else { sk };
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            let ctx = StmtCtx::root(&index, pred, 5_000);
            let outcome = sym_eval_stmt(&body, env.clone(), &ctx)
                .expect("the production STEP denotation evaluates in statement mode");
            let out = stmt_concretize(&outcome, 5_000).unwrap_or_else(|| {
                panic!(
                    "point {} ({:?}): the production STEP denotation must \
                     resolve at the predicted fuel {}",
                    i, input.source, pred
                )
            });
            match (&mirror, &out) {
                (Ok(_), Sym::Result { ok: Tag::True, .. }) => {
                    assert!(
                        p90_values_equal(&out, &p90_result_sym(&mirror)),
                        "point {} ({:?}): the STEP output must equal the mirror's",
                        i, input.source
                    );
                }
                (Err(_), Sym::Result { ok: Tag::False, .. }) => {
                    // Tag agreement; the message text is
                    // the standing F2 deviation.
                }
                _ => panic!(
                    "point {} ({:?}): tag disagreement with the mirror",
                    i, input.source
                ),
            }
            let is_err = matches!(&mirror, Err(_));
            let mut judge_ms = 0u64;
            for (j, row) in spec.postconditions.iter().enumerate() {
                let (tri, ms) = p93_judge(&verifier, row, &spec, &env, &outcome);
                judge_ms += ms;
                if is_err && j == 0 {
                    assert!(
                        matches!(tri, Tri::Unknown),
                        "point {} ({:?}) row 0 at an Err output must judge \
                         Unknown through the statement judge (literal-Err \
                         unwrap boundary)",
                        i, input.source
                    );
                    continue;
                }
                assert!(
                    matches!(tri, Tri::Proven),
                    "point {} ({:?}) row {} must be Proven through the \
                     production statement judge",
                    i, input.source, j
                );
            }
            println!(
                "P93|step-point|{}|src={:?}|skip_trips={}|ident_trips={}|fuel={}|judge_ms={}",
                i, input.source, sk, id, pred, judge_ms
            );
        }
    }

    /// Phase 93: the LOOP battery at points, through the
    /// production statement encoder — the template LOOP
    /// body over the production STEP denotation resolves
    /// at the predicted fuels, its token lists equal the
    /// mirror's, and tokenize_all's row judges Proven at
    /// Ok sources and Unknown at the Err source (W8)
    /// through the production statement judge.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase93_stmt_loop_points_battery() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let spec = restored_spec(&module, "tokenize_all").clone();
        let body = p90_template_body(&module, "tokenize_all");
        for src in ["", "spec sort", "spec{input:()}", "spec 5", "  \n ", "a", "x{y}:(,)", "a\nb"] {
            let input = p90_st(src, 0, 1, 1);
            let (mirror, outer, inner) = p90_mirror_all(&input);
            let pred = outer.max(inner);
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(&input));
            let ctx = StmtCtx::root(&index, pred, 5_000);
            let outcome = sym_eval_stmt(&body, env.clone(), &ctx)
                .expect("the LOOP body evaluates over the production step denotation");
            let out = stmt_concretize(&outcome, 5_000).unwrap_or_else(|| {
                panic!(
                    "{:?}: the LOOP body must resolve at the predicted fuel {} \
                     over production denotations",
                    src, pred
                )
            });
            match (&mirror, &out) {
                (Ok(toks), Sym::Result { ok: Tag::True, ok_payload, .. }) => {
                    let expected = Sym::List(toks.iter().map(p90_token_sym).collect());
                    let payload = ok_payload.as_ref().expect("Ok payload");
                    assert!(
                        p90_values_equal(payload, &expected),
                        "{:?}: the LOOP output token list must equal the mirror's",
                        src
                    );
                }
                (Err(_), Sym::Result { ok: Tag::False, .. }) => {
                    // Tag agrees; the message text is the
                    // F2 deviation, not judged here.
                }
                _ => panic!("{:?}: tag disagreement with the mirror", src),
            }
            let (tri, ms) = p93_judge(
                &verifier,
                &spec.postconditions[0],
                &spec,
                &env,
                &outcome,
            );
            if matches!(&mirror, Err(_)) {
                assert!(
                    matches!(tri, Tri::Unknown),
                    "{:?}: tokenize_all's row at an Err output must judge \
                     Unknown through the statement judge (literal-Err \
                     unwrap boundary)",
                    src
                );
            } else {
                assert!(
                    matches!(tri, Tri::Proven),
                    "{:?}: tokenize_all's row must be Proven through the \
                     production statement judge",
                    src
                );
            }
            println!(
                "P93|loop-point|src={:?}|outer={}|inner={}|fuel={}|judge_ms={}",
                src, outer, inner, pred, ms
            );
        }
    }

    /// Phase 93: symbolic skip judging through the
    /// production statement encoder — the production
    /// denotation over a fully symbolic state judges
    /// Unknown at fuels 1/2/4 under the current rows and
    /// under strengthening row (a): exhaustion is
    /// reachable at every fuel, so unrolling refutes and
    /// decides points but never Verifies an unbounded
    /// loop (the §2.2 corollary, in production). The
    /// mechanism is pinned, not just the verdict: the
    /// outcome's exhaustion is satisfiable at each fuel.
    /// The Forall row (c) also judges Unknown — through
    /// Phase 93 by the other mechanism (its encoding
    /// was Phase 94's arm; production `encode_constraint`
    /// returned None for it). Phase 94 removed that
    /// mechanism: the block below now pins the flip —
    /// the row ENCODES over the symbolic outcome, and
    /// its Unknown is the exhaustion mechanism, the
    /// same one as every other row here (the per-path
    /// decomposition is Phase 94's own test). The two
    /// Unknowns are still never conflated: this test
    /// asserts the encoding exists.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase93_stmt_skip_symbolic_unknown() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let body = index
            .callee_body("skip_whitespace")
            .expect("skip denotes after Phase 92");
        let env = param_env(&skip, &index);
        let row_a = p90_parse_row(
            "output.pos >= len(output.source) || is_whitespace(output.source[output.pos]) == false",
        );
        let mut spec_a = skip.clone();
        spec_a.postconditions.push(row_a.clone());
        for fuel in [1usize, 2, 4] {
            let ctx = StmtCtx::root(&index, fuel, 5_000);
            let outcome = sym_eval_stmt(&body, env.clone(), &ctx)
                .expect("the production skip denotation evaluates symbolically");
            assert_eq!(
                stmt_sat(&[outcome.exhausted.clone()], 5_000),
                Some(true),
                "fuel {}: exhaustion must be REACHABLE — that reachability, \
                 not an encoding gap, is why no symbolic verdict here is Proven",
                fuel
            );
            let mut judge_ms = 0u64;
            for (j, row) in skip.postconditions.iter().enumerate() {
                let (tri, ms) = p93_judge(&verifier, row, &skip, &env, &outcome);
                judge_ms += ms;
                assert!(
                    matches!(tri, Tri::Unknown),
                    "fuel {}: bounded unrolling must NOT Verify the skip \
                     symbolically (exhaustion is always reachable), row {}",
                    fuel, j
                );
            }
            let (tri, ms) = p93_judge(&verifier, &row_a, &spec_a, &env, &outcome);
            judge_ms += ms;
            assert!(
                matches!(tri, Tri::Unknown),
                "fuel {}: row (a) must judge Unknown symbolically — the \
                 faithful body completes only on bounded runs",
                fuel
            );
            println!(
                "P93|skip-sym|fuel={}|paths={}|judge_ms={}",
                fuel,
                outcome.paths.len(),
                judge_ms
            );
        }
        // The (c) row at the same fuel. Through
        // Phase 93 this judged Unknown by the
        // unencodable mechanism; Phase 94's
        // forall_const arm encodes it, so the pin
        // flips: the encoding EXISTS over the
        // symbolic outcome's path env, and the
        // verdict is Unknown by exhaustion — the
        // mechanism every other row in this test
        // judges by.
        let (_, row_c) = p90_module_with_forall_row();
        let ctx = StmtCtx::root(&index, 4, 5_000);
        let outcome = sym_eval_stmt(&body, env.clone(), &ctx)
            .expect("the production skip denotation evaluates symbolically");
        let mut path_env = env.clone();
        path_env.insert(
            "output".into(),
            outcome
                .paths
                .first()
                .expect("the symbolic outcome has a completed path")
                .1
                .clone(),
        );
        assert!(
            encode_constraint(&row_c, &path_env, &EvalCtx::root(&index)).is_some(),
            "Phase 94: the Forall row (c) ENCODES over the symbolic \
             outcome (the unencodable mechanism is removed)"
        );
        let (tri, ms_c) = p93_judge(&verifier, &row_c, &skip, &env, &outcome);
        assert!(
            matches!(tri, Tri::Unknown),
            "the Forall row (c) judges Unknown symbolically — by \
             EXHAUSTION since Phase 94 (it encodes; no completed path \
             refutes it; the exhaustion gate blocks Proven)"
        );
        println!(
            "P93|skip-sym|fuel=4|row (c): Unknown (exhaustion since Phase 94; judge_ms={})",
            ms_c
        );
    }

    /// Phase 93: symbolic STEP judging and the `" 5"`
    /// diagnostic, end to end through production paths.
    /// Symbolically (verify_one's own route, production
    /// fuel): row 1 (index 0) Unknown by exhaustion, row
    /// 2 (index 1) REFUTED — the candidate status is
    /// VerificationFailed with a counterexample. At the
    /// concrete diagnostic point the faithful body errs
    /// and the production statement judge Refutes row 2
    /// over the production outcome (row 1 Unknown, W8).
    /// The LOOP body and the skip denotation, as
    /// candidates, stay Inconclusive symbolically —
    /// their rows judge Unknown, nothing refutes them.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase93_stmt_step_symbolic_and_diag() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let spec = restored_spec(&module, "tokenize_one").clone();
        let step = index
            .callee_body("tokenize_one")
            .expect("tokenize_one denotes after Phase 92");
        // Symbolic judging through the production
        // evaluator + judge at the production fuel.
        let env = param_env(&spec, &index);
        let ctx = StmtCtx::root(&index, STMT_VERIFY_FUEL, 5_000);
        let outcome = sym_eval_stmt(&step, env.clone(), &ctx)
            .expect("the STEP denotation evaluates symbolically in production");
        let (tri0, ms0) = p93_judge(&verifier, &spec.postconditions[0], &spec, &env, &outcome);
        let (tri1, ms1) = p93_judge(&verifier, &spec.postconditions[1], &spec, &env, &outcome);
        assert!(
            matches!(tri0, Tri::Unknown),
            "row 1 (index 0) must judge Unknown symbolically (exhaustion-bounded)"
        );
        assert!(
            matches!(tri1, Tri::Refuted),
            "row 2 (index 1) must be REFUTED symbolically — the \" 5\" \
             countermodel completes on a bounded path"
        );
        println!(
            "P93|step-sym|fuel={}|paths={}|row0_ms={}|row1_ms={}",
            STMT_VERIFY_FUEL,
            outcome.paths.len(),
            ms0,
            ms1
        );
        // verify_one's own verdict on the denotation.
        let cand = candidate(&spec, 9301, step.clone());
        let result = verifier.verify_one(&cand, &spec);
        assert_eq!(
            result.status,
            VerificationStatus::VerificationFailed,
            "verify_one must REFUTE the STEP denotation under standing row 2"
        );
        assert!(
            result.counterexample.is_some(),
            "a refutation carries its counterexample"
        );
        assert!(
            result.constraint_results.iter().all(|r| !r.holds),
            "neither row holds for the STEP denotation symbolically"
        );
        println!(
            "P93|step-verify-one|status=VerificationFailed|time_ms={}",
            result.verification_time_ms
        );
        // The diagnostic point, through the production
        // judge over the production outcome.
        let diag = p90_st(" 5", 0, 1, 1);
        let mut env = SymEnv::new();
        env.insert("state".into(), p90_state_sym(&diag));
        let ctx = StmtCtx::root(&index, 2, 5_000);
        let outcome = sym_eval_stmt(&step, env.clone(), &ctx)
            .expect("STEP evaluates at the diagnostic point");
        let out = stmt_concretize(&outcome, 5_000).expect("the diagnostic point resolves");
        assert!(
            matches!(&out, Sym::Result { ok: Tag::False, .. }),
            "the faithful body errs at \" 5\""
        );
        let (tri, _) = p93_judge(&verifier, &spec.postconditions[1], &spec, &env, &outcome);
        assert!(
            matches!(tri, Tri::Refuted),
            "row 2 REFUTES the faithful body at \" 5\" through the \
             production statement judge: the residue is the R2-β-skip \
             ratification (Phase 96)"
        );
        let (tri, _) = p93_judge(&verifier, &spec.postconditions[0], &spec, &env, &outcome);
        assert!(
            matches!(tri, Tri::Unknown),
            "row 1 at the Err point judges Unknown (W8 literal-Err boundary)"
        );
        println!("P93|step-diag|\" 5\": row 2 Refuted, row 1 Unknown (production judge)");
        // The LOOP body as a candidate: Inconclusive.
        let all_spec = restored_spec(&module, "tokenize_all").clone();
        let loop_body = p90_template_body(&module, "tokenize_all");
        let cand = candidate(&all_spec, 9302, loop_body);
        let result = verifier.verify_one(&cand, &all_spec);
        assert_eq!(
            result.status,
            VerificationStatus::Inconclusive,
            "the LOOP body stays Inconclusive symbolically (row Unknown, \
             nothing refuted)"
        );
        println!(
            "P93|loop-verify-one|status=Inconclusive|time_ms={}",
            result.verification_time_ms
        );
        // The skip denotation as a candidate:
        // Inconclusive (its rows judge Unknown).
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let skip_body = index
            .callee_body("skip_whitespace")
            .expect("skip denotes after Phase 92");
        let cand = candidate(&skip, 9303, skip_body);
        let result = verifier.verify_one(&cand, &skip);
        assert_eq!(
            result.status,
            VerificationStatus::Inconclusive,
            "the skip denotation stays Inconclusive symbolically \
             (exhaustion-bounded rows)"
        );
        println!(
            "P93|skip-verify-one|status=Inconclusive|time_ms={}",
            result.verification_time_ms
        );
    }

    /// Phase 93 teeth: wrong bodies Refute through the
    /// production statement judge where the discipline
    /// says they must — and the routing gate, not
    /// capability, decides what gets judged at all.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase93_stmt_teeth() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let one = restored_spec(&module, "tokenize_one").clone();
        let step = index
            .callee_body("tokenize_one")
            .expect("tokenize_one denotes after Phase 92");
        // ── The routing gate: engagement is syntactic
        // (statement/loop nodes in the body or its
        // denotations), never capability-based.
        assert!(
            body_mentions_statements(&step, &index),
            "the STEP body itself carries Loop/Alloc nodes"
        );
        let call_skip = IRNode::Call {
            function: "skip_whitespace".to_string(),
            args: vec![IRNode::Var("state".to_string())],
        };
        assert!(
            body_mentions_statements(&call_skip, &index),
            "a bare call engages through its statement-bodied denotation"
        );
        let append_char = IRNode::BinOp(
            IRBinOp::Append,
            Box::new(IRNode::Literal(IRLiteral::String("x".to_string()))),
            Box::new(IRNode::Literal(IRLiteral::Char('y'))),
        );
        assert!(
            !body_mentions_statements(&append_char, &index),
            "an Append the expression encoder leaves unarmed mentions \
             no statements — the statement layer must not engage for it"
        );
        let cand = candidate(&one, 9311, append_char);
        assert_eq!(
            verifier.verify_one(&cand, &one).status,
            VerificationStatus::Inconclusive,
            "a body declined for a NON-statement reason keeps its standing \
             Inconclusive even though stmt_append could evaluate it: \
             routing, not capability, gates the statement layer"
        );
        // ── Identity-as-skip at a whitespace point: the
        // current rows Proven (the standing scandal,
        // reproduced through the production judge), row
        // (a) Refuted.
        let row_a = p90_parse_row(
            "output.pos >= len(output.source) || is_whitespace(output.source[output.pos]) == false",
        );
        let mut spec_a = skip.clone();
        spec_a.postconditions.push(row_a.clone());
        let identity = IRNode::Return(Box::new(IRNode::Var("state".to_string())));
        let ws_start = p90_st("  \t\n x", 0, 1, 1);
        let mut env = SymEnv::new();
        env.insert("state".into(), p90_state_sym(&ws_start));
        let ctx = StmtCtx::root(&index, 0, 5_000);
        let outcome = sym_eval_stmt(&identity, env.clone(), &ctx)
            .expect("the identity body evaluates in statement mode");
        for row in skip.postconditions.iter() {
            let (tri, _) = p93_judge(&verifier, row, &skip, &env, &outcome);
            assert!(
                matches!(tri, Tri::Proven),
                "the identity body satisfies the current rows (the standing scandal)"
            );
        }
        let (tri, _) = p93_judge(&verifier, &row_a, &spec_a, &env, &outcome);
        assert!(
            matches!(tri, Tri::Refuted),
            "row (a) REFUTES the identity body at a whitespace start \
             through the production statement judge"
        );
        // ── Partial skip (≤ 1 char): (a) Refutes it on a
        // run of 5, Proves it on a run of 1.
        let partial = p90_partial_skip_body();
        let outcome = sym_eval_stmt(&partial, env.clone(), &ctx)
            .expect("the partial skip evaluates in statement mode");
        let (tri, _) = p93_judge(&verifier, &row_a, &spec_a, &env, &outcome);
        assert!(
            matches!(tri, Tri::Refuted),
            "row (a) REFUTES the partial skip on a run of 5"
        );
        let one_ws = p90_st(" ", 0, 1, 1);
        let mut env1 = SymEnv::new();
        env1.insert("state".into(), p90_state_sym(&one_ws));
        let outcome = sym_eval_stmt(&partial, env1.clone(), &ctx)
            .expect("the partial skip evaluates in statement mode");
        let (tri, _) = p93_judge(&verifier, &row_a, &spec_a, &env1, &outcome);
        assert!(
            matches!(tri, Tri::Proven),
            "the partial skip is exactly right on a run of 1: (a) admits it there"
        );
        println!("P93|teeth-skip|identity: current Proven, (a) Refuted; partial: (a) Refuted on run 5, Proven on run 1");
        // ── Double-append: a step-shaped body that
        // appends the token TWICE at ":" — tokenize_one's
        // row 1 (exactly one more token) Refutes it
        // through the production judge; row 2 admits the
        // Ok.
        let v = |n: &str| IRNode::Var(n.to_string());
        let fa = |base: IRNode, field: &str| IRNode::FieldAccess {
            base: Box::new(base),
            field: field.to_string(),
        };
        let call = |f: &str, args: Vec<IRNode>| IRNode::Call {
            function: f.to_string(),
            args,
        };
        let state_ty = TypeRef::Named("TokenizerState".to_string());
        let token_lit = IRNode::RecordLit {
            type_name: "Token".to_string(),
            fields: vec![
                ("kind".to_string(), IRNode::Literal(IRLiteral::Int(3))),
                (
                    "lexeme".to_string(),
                    IRNode::Literal(IRLiteral::String("(".to_string())),
                ),
                ("line".to_string(), fa(v("s"), "line")),
                ("col".to_string(), fa(v("s"), "col")),
            ],
        };
        let double_append = IRNode::Block(vec![
            IRNode::Alloc {
                name: "s".to_string(),
                ty: state_ty.clone(),
                initial: Box::new(call("skip_whitespace", vec![v("state")])),
            },
            IRNode::Alloc {
                name: "toks".to_string(),
                ty: TypeRef::List(Box::new(TypeRef::Named("Token".to_string()))),
                initial: Box::new(fa(v("s"), "tokens")),
            },
            IRNode::BinOp(
                IRBinOp::Append,
                Box::new(v("toks")),
                Box::new(token_lit.clone()),
            ),
            IRNode::BinOp(IRBinOp::Append, Box::new(v("toks")), Box::new(token_lit)),
            IRNode::Return(Box::new(call(
                "Ok",
                vec![IRNode::RecordLit {
                    type_name: "TokenizerState".to_string(),
                    fields: vec![
                        ("source".to_string(), fa(v("s"), "source")),
                        ("pos".to_string(), fa(v("s"), "pos")),
                        ("line".to_string(), fa(v("s"), "line")),
                        ("col".to_string(), fa(v("s"), "col")),
                        ("tokens".to_string(), v("toks")),
                    ],
                }],
            ))),
        ]);
        let colon = p90_st(":", 0, 1, 1);
        let mut env = SymEnv::new();
        env.insert("state".into(), p90_state_sym(&colon));
        let ctx = StmtCtx::root(&index, 1, 5_000);
        let outcome = sym_eval_stmt(&double_append, env.clone(), &ctx)
            .expect("the double-append body evaluates in statement mode");
        let (tri, _) = p93_judge(&verifier, &one.postconditions[0], &one, &env, &outcome);
        assert!(
            matches!(tri, Tri::Refuted),
            "row 1 REFUTES the double-append body (two tokens where \
             exactly one more is licensed)"
        );
        let (tri, _) = p93_judge(&verifier, &one.postconditions[1], &one, &env, &outcome);
        assert!(
            matches!(tri, Tri::Proven),
            "row 2 admits the double-append body's Ok (it discriminates \
             the count, not the tag)"
        );
        println!("P93|teeth-step|double-append at \":\": row 1 Refuted, row 2 Proven");
        // ── Swapped arms: the STEP denotation with its
        // EOF dispatch branches exchanged — at "spec" it
        // returns Ok with NO token consumed, and row 1
        // Refutes it.
        let IRNode::Block(top) = &step else {
            panic!("the STEP denotation is a Block");
        };
        assert_eq!(top.len(), 2, "the STEP body is [skip alloc, EOF dispatch]");
        let IRNode::If {
            condition,
            then_branch,
            else_branch,
        } = &top[1]
        else {
            panic!("the STEP body's second statement is the EOF dispatch If");
        };
        let swapped = IRNode::Block(vec![
            top[0].clone(),
            IRNode::If {
                condition: condition.clone(),
                then_branch: else_branch.clone(),
                else_branch: then_branch.clone(),
            },
        ]);
        let spec_point = p90_st("spec", 0, 1, 1);
        let mut env = SymEnv::new();
        env.insert("state".into(), p90_state_sym(&spec_point));
        let ctx = StmtCtx::root(&index, 5, 5_000);
        let outcome = sym_eval_stmt(&swapped, env.clone(), &ctx)
            .expect("the swapped body evaluates in statement mode");
        let (tri, _) = p93_judge(&verifier, &one.postconditions[0], &one, &env, &outcome);
        assert!(
            matches!(tri, Tri::Refuted),
            "row 1 REFUTES the branch-swapped STEP body at \"spec\" \
             (Ok with no token where a token start stands)"
        );
        println!("P93|teeth-step|swapped EOF branches at \"spec\": row 1 Refuted");
    }

    // ────────────────────────────────────────────────────
    // Phase 94 — the Forall encoding in production:
    // the `forall_const` arm in `encode_constraint`
    // (design §3.3). The Phase 90 probe below stands
    // as the measurement record; these tests re-run
    // its evidence through the production encoder and
    // the production judges. The bounded-expansion
    // fallback the design priced as a companion is
    // NOT built: the quantifier arm decides every
    // point, tooth and symbolic case in this battery,
    // so the fallback is recorded measured-unnecessary
    // (ROAD Phase 94 note). No spec is amended — the
    // (c) row's adoption is R1's, in Phase 96; the row
    // here comes from the Phase 90 scratch surgery.
    // ────────────────────────────────────────────────────

    /// Judge one row at a formula level through the
    /// PRODUCTION encoder: encode (must succeed),
    /// then satisfiability of the negation.
    #[cfg(feature = "z3-support")]
    fn p94_judge_formula(
        row: &Constraint,
        env: &SymEnv,
        ctx: &EvalCtx,
    ) -> (&'static str, u64) {
        let f = encode_constraint(row, env, ctx)
            .expect("the row encodes through the production encoder");
        match p90_sat_all(&[f.not()]) {
            (Some(false), ms) => ("Proven", ms),
            (Some(true), ms) => ("Refuted", ms),
            (None, ms) => ("Unknown", ms),
        }
    }

    /// Phase 94: the (c) row at the 12 skip points,
    /// through production `encode_constraint` — the
    /// mirror's outputs Proven 12/12 (Phase 90's probe
    /// verdicts reproduced exactly, cross-checked
    /// against the probe encoder), the overshoot
    /// outputs Refuted, identity/partial admitted;
    /// and the JOINT (a)+(c) teeth at the whitespace
    /// point: row (a) Refutes the identity and partial
    /// outputs that (c) admits, and both rows Prove
    /// the mirror's output. The exact pin is (a)+(c)
    /// jointly — neither row oversold.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase94_forall_points_battery() {
        let (module_c, row_c) = p90_module_with_forall_row();
        let index = SpecIndex::from_module(&module_c);
        let ectx = EvalCtx::root(&index);
        let judge_c = |state: &P90St, output: &P90St| -> (&'static str, u64) {
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(state));
            env.insert("output".into(), p90_state_sym(output));
            p94_judge_formula(&row_c, &env, &ectx)
        };
        let battery: Vec<P90St> = vec![
            p90_st("", 0, 1, 1),
            p90_st(" ", 0, 1, 1),
            p90_st("  \t\n x", 0, 1, 1),
            p90_st("no-ws", 0, 1, 1),
            p90_st("\n\nab", 0, 1, 1),
            p90_st(" \u{A0}x", 0, 1, 1),
            p90_st("a b", 0, 1, 1),
            p90_st("\r\n\t", 0, 1, 1),
            p90_st("a b", 1, 1, 2),
            p90_st("  \t\n x", 2, 1, 3),
            p90_st("\n\nab", 1, 1, 2),
            p90_st(" \u{A0}x", 1, 1, 2),
        ];
        for (i, input) in battery.iter().enumerate() {
            let (mirror, _) = p90_mirror_skip(input);
            let (verdict, ms) = judge_c(input, &mirror);
            assert_eq!(
                verdict, "Proven",
                "point {}: (c) must PROVE the mirror's output through \
                 the production encoder",
                i
            );
            // Cross-check: the probe encoder's verdict
            // on the same point agrees with production.
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            env.insert("output".into(), p90_state_sym(&mirror));
            let pf = p90_encode_forall(&row_c, &env, &ectx)
                .expect("the probe encoder encodes the Forall at a point");
            let probe_verdict = match p90_sat_all(&[pf.not()]) {
                (Some(false), _) => "Proven",
                (Some(true), _) => "Refuted",
                (None, _) => "Unknown",
            };
            assert_eq!(
                probe_verdict, verdict,
                "point {}: probe and production encoders must agree",
                i
            );
            println!(
                "P94|forall-point|{}|src={:?}|verdict={}|judge_ms={}",
                i, input.source, verdict, ms
            );
        }
        // Teeth: the overshoot outputs are Refuted.
        for (src, out_pos) in [("a b", 2i64), ("ab cd", 3i64)] {
            let input = p90_st(src, 0, 1, 1);
            let output = p90_st(src, out_pos, 1, out_pos + 1);
            let (verdict, ms) = judge_c(&input, &output);
            assert_eq!(
                verdict, "Refuted",
                "(c) must REFUTE the overshoot {:?} -> {}",
                src, out_pos
            );
            println!(
                "P94|forall-tooth|overshoot {:?} -> {}|Refuted|judge_ms={}",
                src, out_pos, ms
            );
        }
        // The division of labor at the whitespace
        // point: (c) admits identity and partial
        // (their crossings are all whitespace); row
        // (a) Refutes both; the mirror's output is
        // Proven under both rows jointly.
        let ws = p90_st("  \t\n x", 0, 1, 1);
        let (mirror, _) = p90_mirror_skip(&ws);
        let mut partial = ws.clone();
        partial.pos = 1;
        partial.col = 2;
        let row_a = p90_parse_row(
            "output.pos >= len(output.source) || is_whitespace(output.source[output.pos]) == false",
        );
        let judge_a = |output: &P90St| -> (&'static str, u64) {
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(&ws));
            env.insert("output".into(), p90_state_sym(output));
            p94_judge_formula(&row_a, &env, &ectx)
        };
        for (label, out) in [("identity", &ws), ("partial", &partial)] {
            let (vc, msc) = judge_c(&ws, out);
            assert_eq!(
                vc, "Proven",
                "(c) admits the {} output at the point (its crossing \
                 is all whitespace — the landing is row (a)'s business)",
                label
            );
            let (va, msa) = judge_a(out);
            assert_eq!(
                va, "Refuted",
                "row (a) must REFUTE the {} output at the point \
                 (it lands on whitespace, short of the boundary)",
                label
            );
            println!(
                "P94|forall-joint|{}|(c)={} ({} ms)|(a)={} ({} ms)",
                label, vc, msc, va, msa
            );
        }
        let (vc, msc) = judge_c(&ws, &mirror);
        let (va, msa) = judge_a(&mirror);
        assert_eq!(vc, "Proven", "(c) proves the mirror's output");
        assert_eq!(va, "Proven", "(a) proves the mirror's output");
        println!(
            "P94|forall-joint|mirror|(c)={} ({} ms)|(a)={} ({} ms)",
            vc, msc, va, msa
        );
    }

    /// Phase 94: the (c) row through the PRODUCTION
    /// STATEMENT JUDGE at the 12 points — the Phase 92
    /// denotation's outcomes, judged by
    /// `verify_constraint_stmt` with the production
    /// Forall arm inside: Proven at all 12 points (at
    /// a point the outcome is complete — exhaustion
    /// is unsatisfiable — so the BMC discipline
    /// promotes the per-path silence to Proven).
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase94_forall_stmt_judge_points() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let body = index
            .callee_body("skip_whitespace")
            .expect("skip denotes after Phase 92");
        let (_, row_c) = p90_module_with_forall_row();
        let battery: Vec<P90St> = vec![
            p90_st("", 0, 1, 1),
            p90_st(" ", 0, 1, 1),
            p90_st("  \t\n x", 0, 1, 1),
            p90_st("no-ws", 0, 1, 1),
            p90_st("\n\nab", 0, 1, 1),
            p90_st(" \u{A0}x", 0, 1, 1),
            p90_st("a b", 0, 1, 1),
            p90_st("\r\n\t", 0, 1, 1),
            p90_st("a b", 1, 1, 2),
            p90_st("  \t\n x", 2, 1, 3),
            p90_st("\n\nab", 1, 1, 2),
            p90_st(" \u{A0}x", 1, 1, 2),
        ];
        for (i, input) in battery.iter().enumerate() {
            let (_, trips) = p90_mirror_skip(input);
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            let ctx = StmtCtx::root(&index, trips, 5_000);
            let outcome = sym_eval_stmt(&body, env.clone(), &ctx)
                .expect("the production skip denotation evaluates at a point");
            assert_eq!(
                stmt_sat(&[outcome.exhausted.clone()], 5_000),
                Some(false),
                "point {}: at the trip-count fuel the outcome is \
                 complete (exhaustion unsatisfiable)",
                i
            );
            let (tri, ms) = p93_judge(&verifier, &row_c, &skip, &env, &outcome);
            assert!(
                matches!(tri, Tri::Proven),
                "point {}: the production statement judge must PROVE \
                 (c) over the denoted skip outcome",
                i
            );
            println!(
                "P94|stmt-point|{}|src={:?}|trips={}|verdict=Proven|judge_ms={}",
                i, input.source, trips, ms
            );
        }
    }

    /// A state record with its `pos` field advanced
    /// by `delta` (the other fields carried) — the
    /// free-symbolic wrong outputs of the next test.
    #[cfg(feature = "z3-support")]
    fn p94_with_pos(rec: &Sym, delta: i64) -> Sym {
        let Sym::Record { type_name, fields } = rec else {
            panic!("a state record");
        };
        let fields = fields
            .iter()
            .map(|(n, v)| {
                if n == "pos" {
                    let Sym::Int(i) = v else {
                        panic!("pos is an Int");
                    };
                    (
                        n.clone(),
                        Sym::Int(z3::ast::Int::add(&[
                            i,
                            &z3::ast::Int::from_i64(delta),
                        ])),
                    )
                } else {
                    (n.clone(), v.clone())
                }
            })
            .collect();
        Sym::Record {
            type_name: type_name.clone(),
            fields,
        }
    }

    /// Phase 94: the symbolic mechanism, decomposed.
    /// Over the production denotation's SYMBOLIC
    /// outcome (fuel 4): (c) now ENCODES on every
    /// completed path and NO completed path refutes
    /// it (pc ∧ ¬exhausted ∧ ¬(c) is UNSAT per path),
    /// while exhaustion stays satisfiable — so the
    /// aggregate verdict is Unknown by the exhaustion
    /// gate alone, the unencodable mechanism removed.
    /// (Phase 96's invariant route remains the
    /// designated path to a symbolic Verified.)
    /// And the wrong free outputs, judged per row
    /// through `verify_constraint` over a fully
    /// symbolic state: identity is Proven by (c)
    /// (vacuous range) and Refuted by (a); the
    /// overshoot (pos+2) is Refuted by (c) and
    /// by (a); the bare
    /// pos+1 output is Refuted by (c) too — (c)
    /// admits a partial BODY only through its path
    /// condition (the statement judge above), never
    /// a bare advance whose crossing is unproven.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase94_forall_symbolic_mechanism() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let body = index
            .callee_body("skip_whitespace")
            .expect("skip denotes after Phase 92");
        let (_, row_c) = p90_module_with_forall_row();
        let row_a = p90_parse_row(
            "output.pos >= len(output.source) || is_whitespace(output.source[output.pos]) == false",
        );
        let env = param_env(&skip, &index);
        let ctx = StmtCtx::root(&index, 4, 5_000);
        let outcome = sym_eval_stmt(&body, env.clone(), &ctx)
            .expect("the production skip denotation evaluates symbolically");
        let not_exhausted = outcome.exhausted.not();
        for (pc, output) in &outcome.paths {
            let mut penv = env.clone();
            penv.insert("output".into(), output.clone());
            let f = encode_constraint(&row_c, &penv, &EvalCtx::root(&index))
                .expect("Phase 94: (c) ENCODES over a symbolic completed path");
            assert_eq!(
                stmt_sat(&[pc.clone(), not_exhausted.clone(), f.not()], 5_000),
                Some(false),
                "no completed path of the faithful body refutes (c)"
            );
        }
        assert_eq!(
            stmt_sat(&[outcome.exhausted.clone()], 5_000),
            Some(true),
            "exhaustion stays satisfiable — it alone blocks Proven"
        );
        let (tri, ms) = p93_judge(&verifier, &row_c, &skip, &env, &outcome);
        assert!(
            matches!(tri, Tri::Unknown),
            "the aggregate symbolic verdict for (c) is Unknown — \
             by exhaustion, not by unencodability"
        );
        println!(
            "P94|symbolic|faithful body|(c)=Unknown (exhaustion gate)|judge_ms={}",
            ms
        );
        // Wrong free outputs over the symbolic state,
        // per row through the production judge.
        let state_sym = env
            .get("state")
            .expect("the param env binds state")
            .clone();
        let judge = |row: &Constraint, output: &Sym| -> (Tri, u64) {
            let (res, tri) =
                verifier.verify_constraint(row, "p94", &skip, &env, &Some(output.clone()));
            (tri, res.solver_time_ms)
        };
        let (tc, mc) = judge(&row_c, &state_sym);
        assert!(
            matches!(tc, Tri::Proven),
            "identity output: (c) is Proven (the crossed range is empty)"
        );
        let (ta, ma) = judge(&row_a, &state_sym);
        assert!(
            matches!(ta, Tri::Refuted),
            "identity output: row (a) REFUTES it (a model lands it \
             on whitespace short of EOF)"
        );
        println!(
            "P94|symbolic|identity|(c)=Proven ({} ms)|(a)=Refuted ({} ms)",
            mc, ma
        );
        let over = p94_with_pos(&state_sym, 2);
        let (tc, mc) = judge(&row_c, &over);
        assert!(
            matches!(tc, Tri::Refuted),
            "overshoot output (pos+2): (c) REFUTES it (a model \
             crosses a non-whitespace char)"
        );
        let (ta, ma) = judge(&row_a, &over);
        assert!(
            matches!(ta, Tri::Refuted),
            "overshoot output (pos+2): row (a) REFUTES it too \
             (a model lands it on whitespace short of EOF)"
        );
        println!(
            "P94|symbolic|overshoot|(c)=Refuted ({} ms)|(a)=Refuted ({} ms)",
            mc, ma
        );
        let partial = p94_with_pos(&state_sym, 1);
        let (tc, mc) = judge(&row_c, &partial);
        assert!(
            matches!(tc, Tri::Refuted),
            "bare pos+1 output: (c) REFUTES it at the free-output \
             level (the crossed char is unproven) — a partial body \
             is admitted only through its path condition"
        );
        println!(
            "P94|symbolic|partial (bare)|(c)=Refuted ({} ms)",
            mc
        );
    }

    /// Phase 94: the arm's envelope — what encodes
    /// and what declines, pinned at the encoder.
    /// Encodes: Int binders (single and multi-var)
    /// over quantifier-free encodable bodies.
    /// Declines whole: non-Int binders, an empty or
    /// duplicate binder list, a nested quantifier in
    /// the body, an unencodable body — and Exists,
    /// whose boundary does not move in this phase.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase94_forall_envelope() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let ectx = EvalCtx::root(&index);
        let env = SymEnv::new();
        let parse_row = |src: &str| -> Constraint {
            let parsed = crate::spec::parser::parse(src).expect("probe row parses");
            parsed.functions[0].postconditions[0].clone()
        };
        // Non-Int binders decline.
        for (label, src) in [
            (
                "Char binder",
                "T {\n spec probe {\n input: x: Int\n output: Int\n constraint: forall c: Char in range(3): c == c\n }\n}",
            ),
            (
                "String binder",
                "T {\n spec probe {\n input: x: Int\n output: Int\n constraint: forall s: String in range(3): s == s\n }\n}",
            ),
        ] {
            let row = parse_row(src);
            assert!(
                matches!(row, Constraint::Forall { .. }),
                "{}: the probe row is a Forall",
                label
            );
            assert!(
                encode_constraint(&row, &env, &ectx).is_none(),
                "{}: a non-Int binder DECLINES (the bridge speaks Int indices)",
                label
            );
            println!("P94|envelope|{}|declines", label);
        }
        // AST-level declines.
        let int_var = |n: &str| (n.to_string(), TypeRef::Int);
        let declines: Vec<(&str, Constraint)> = vec![
            (
                "empty binder list",
                Constraint::Forall {
                    vars: vec![],
                    body: Box::new(Constraint::True),
                },
            ),
            (
                "duplicate binders",
                Constraint::Forall {
                    vars: vec![int_var("i"), int_var("i")],
                    body: Box::new(Constraint::True),
                },
            ),
            (
                "nested Forall",
                Constraint::Forall {
                    vars: vec![int_var("i")],
                    body: Box::new(Constraint::Forall {
                        vars: vec![int_var("j")],
                        body: Box::new(Constraint::True),
                    }),
                },
            ),
            (
                "Forall nested under And in the body",
                Constraint::Forall {
                    vars: vec![int_var("i")],
                    body: Box::new(Constraint::And(vec![
                        Constraint::True,
                        Constraint::Forall {
                            vars: vec![int_var("j")],
                            body: Box::new(Constraint::True),
                        },
                    ])),
                },
            ),
            (
                "unencodable body (a named predicate)",
                Constraint::Forall {
                    vars: vec![int_var("i")],
                    body: Box::new(Constraint::Predicate(
                        "is_sorted".into(),
                        vec![Expr::Var("i".into())],
                    )),
                },
            ),
            (
                "Exists (boundary unmoved)",
                Constraint::Exists {
                    vars: vec![int_var("i")],
                    body: Box::new(Constraint::True),
                },
            ),
        ];
        for (label, row) in &declines {
            assert!(
                encode_constraint(row, &env, &ectx).is_none(),
                "{}: DECLINES whole",
                label
            );
            println!("P94|envelope|{}|declines", label);
        }
        // Multi-var Int binders encode and decide:
        // the tautology is Proven, the strict order
        // claim is Refuted (i = j is a countermodel).
        let taut = parse_row(
            "T {\n spec probe {\n input: x: Int\n output: Int\n constraint: forall i: Int, j: Int in range(10): i < j || i >= j\n }\n}",
        );
        let (verdict, ms) = p94_judge_formula(&taut, &env, &ectx);
        assert_eq!(
            verdict, "Proven",
            "a two-Int-binder Forall over a tautology is Proven"
        );
        println!(
            "P94|envelope|multi-var tautology|Proven|judge_ms={}",
            ms
        );
        let strict = parse_row(
            "T {\n spec probe {\n input: x: Int\n output: Int\n constraint: forall i: Int, j: Int in range(10): i < j\n }\n}",
        );
        let (verdict, ms) = p94_judge_formula(&strict, &env, &ectx);
        assert_eq!(
            verdict, "Refuted",
            "a two-Int-binder Forall claiming i < j universally is Refuted"
        );
        println!(
            "P94|envelope|multi-var strict|Refuted|judge_ms={}",
            ms
        );
    }

    // ────────────────────────────────────────────────────
    // Phase 90 — ENCODER EXTENSION feasibility probes.
    //
    // Scratch instruments for ENCODER_EXTENSION_DESIGN.md;
    // NOTHING below touches production code. The
    // statement-mode evaluator (p90_eval), the denotation
    // overrides, the exhaustion-aware judge and the
    // Forall encoder are test-local prototypes of the
    // designs the document prices. They reuse the
    // production symbolic machinery (sym_eval, sym_call,
    // sym_ite, encode_constraint, verify_constraint)
    // wherever the production subset reaches, and extend
    // it only where the extension itself would have to:
    // Alloc / Assign / Loop / While / Break /
    // TryPropagate, Append over the length-only list form
    // and over strings, and spec calls the resolver
    // cannot denote (supplied from a probe-local
    // denotation map — the resolver extension's shape).
    // ────────────────────────────────────────────────────

    #[cfg(feature = "z3-support")]
    #[derive(Clone)]
    struct P90State {
        env: SymEnv,
        /// Path condition under which execution reaches
        /// the current point.
        pc: z3::ast::Bool,
        /// (pc, value) return contributions, in order.
        rets: Vec<(z3::ast::Bool, Sym)>,
        /// Path conditions under which a Break fired and
        /// has not yet been consumed by its loop.
        breaks: Vec<z3::ast::Bool>,
        /// Path conditions under which a loop still
        /// wanted another iteration when its fuel ran
        /// out. Where the exhaustion disjunction is
        /// satisfiable the body has NOT been fully
        /// evaluated — the BMC transfer discipline the
        /// design document states.
        exhausted: Vec<z3::ast::Bool>,
        /// The running block value (production Block
        /// semantics: the last statement's value).
        last: Option<Sym>,
        /// Variables bound since this state was created
        /// (drives the If environment merge).
        writes: std::collections::HashSet<String>,
        /// Standing assumptions for solver-guided loop
        /// decisions (empty in these probes — point states
        /// are literal; the field exists so a guided
        /// decision never silently drops context).
        assumptions: Vec<z3::ast::Bool>,
        /// Guided mode: every branch so far was solver-
        /// DECIDED, so the state is a single path and
        /// binds replace outright (no ite merge). The
        /// first undecidable branch (If condition, loop
        /// condition, TryPropagate tag) flips the state
        /// to merging mode for the rest of the run.
        guided: bool,
    }

    #[cfg(feature = "z3-support")]
    struct P90Ctx<'a> {
        index: &'a SpecIndex,
        denotations: &'a std::collections::HashMap<String, IRNode>,
        fuel: usize,
        depth: usize,
        /// When false, Append over the length-only list
        /// form fails — the ablation that measures the
        /// list-append wall behind the statement wall.
        list_append: bool,
    }

    #[cfg(feature = "z3-support")]
    #[derive(Debug)]
    struct P90Outcome {
        /// (pc, output value) per return path, plus the
        /// fall-through value under the remaining pc
        /// when the body can end without returning.
        paths: Vec<(z3::ast::Bool, Sym)>,
        /// Disjunction of every loop-exhaustion path
        /// condition recorded during the evaluation.
        exhausted: z3::ast::Bool,
    }

    #[cfg(feature = "z3-support")]
    fn p90_true() -> z3::ast::Bool {
        z3::ast::Bool::from_bool(true)
    }

    #[cfg(feature = "z3-support")]
    fn p90_false() -> z3::ast::Bool {
        z3::ast::Bool::from_bool(false)
    }

    #[cfg(feature = "z3-support")]
    fn p90_and(a: &z3::ast::Bool, b: &z3::ast::Bool) -> z3::ast::Bool {
        z3::ast::Bool::and(&[a, b])
    }

    #[cfg(feature = "z3-support")]
    fn p90_or(a: &z3::ast::Bool, b: &z3::ast::Bool) -> z3::ast::Bool {
        z3::ast::Bool::or(&[a, b])
    }

    #[cfg(feature = "z3-support")]
    fn p90_or_all(terms: &[z3::ast::Bool]) -> z3::ast::Bool {
        let mut acc = p90_false();
        for t in terms {
            acc = p90_or(&acc, t);
        }
        acc
    }

    #[cfg(feature = "z3-support")]
    fn p90_is_true(b: &z3::ast::Bool) -> bool {
        matches!(b.as_bool(), Some(true))
    }

    #[cfg(feature = "z3-support")]
    fn p90_is_false(b: &z3::ast::Bool) -> bool {
        matches!(b.as_bool(), Some(false))
    }

    /// Bind `name` to `v` under the current path
    /// condition: an unconditional bind replaces; a
    /// conditioned bind merges with the previous value
    /// by ite (the production sym_ite; a merge the Sym
    /// forms cannot express fails the evaluation).
    #[cfg(feature = "z3-support")]
    /// Structural identity of two Sym values: same term
    /// DAGs (Z3 hash-conses, and the Debug rendering of a
    /// term is its structural s-expression), same shapes.
    /// Used for the merge fast path below.
    #[cfg(feature = "z3-support")]
    fn p90_sym_identical(a: &Sym, b: &Sym) -> bool {
        // Z3 hash-conses terms and its AST `==` is node
        // identity (Z3_is_eq_ast) — O(1) per term. (An
        // earlier draft compared Debug strings; printing
        // a shared DAG as a tree is exponential in its
        // depth and cost ~96 s on the STEP body — the
        // measured reason this uses node identity.)
        fn term_eq<T: PartialEq>(x: &T, y: &T) -> bool {
            x == y
        }
        match (a, b) {
            (Sym::Int(x), Sym::Int(y)) => term_eq(x, y),
            (Sym::Bool(x), Sym::Bool(y)) => term_eq(x, y),
            (Sym::Str(x), Sym::Str(y)) => term_eq(x, y),
            (Sym::Char(x), Sym::Char(y)) => term_eq(x, y),
            (Sym::CharStr(x), Sym::CharStr(y)) => term_eq(x, y),
            (Sym::ListLen(x), Sym::ListLen(y)) => term_eq(x, y),
            (Sym::List(x), Sym::List(y)) => {
                x.len() == y.len()
                    && x.iter().zip(y.iter()).all(|(u, v)| p90_sym_identical(u, v))
            }
            (
                Sym::Record { type_name: ta, fields: fa },
                Sym::Record { type_name: tb, fields: fb },
            ) => {
                ta == tb
                    && fa.len() == fb.len()
                    && fa.iter().zip(fb.iter()).all(|((na, va), (nb, vb))| {
                        na == nb && p90_sym_identical(va, vb)
                    })
            }
            (
                Sym::Option { some: sa, payload: pa },
                Sym::Option { some: sb, payload: pb },
            ) => {
                std::mem::discriminant(sa) == std::mem::discriminant(sb)
                    && match (pa, pb) {
                        (None, None) => true,
                        (Some(x), Some(y)) => p90_sym_identical(x, y),
                        _ => false,
                    }
            }
            (
                Sym::Result { ok: oa, ok_payload: pa, err_payload: ea },
                Sym::Result { ok: ob, ok_payload: pb, err_payload: eb },
            ) => {
                std::mem::discriminant(oa) == std::mem::discriminant(ob)
                    && match (pa, pb) {
                        (None, None) => true,
                        (Some(x), Some(y)) => p90_sym_identical(x, y),
                        _ => false,
                    }
                    && match (ea, eb) {
                        (None, None) => true,
                        (Some(x), Some(y)) => p90_sym_identical(x, y),
                        _ => false,
                    }
            }
            _ => false,
        }
    }

    /// The probe's merge: identical sides merge for
    /// free at EVERY level (production `sym_ite` has no
    /// (List, List) arm, so merging a record whose list
    /// field is carried unchanged — the tokenizer state
    /// at every step — fails inside its Record arm,
    /// which recurses into itself). Structural arms
    /// mirror production's, recursing through this
    /// function instead; leaf arms defer to production.
    #[cfg(feature = "z3-support")]
    fn p90_ite(cond: &z3::ast::Bool, a: Sym, b: Sym) -> Option<Sym> {
        if p90_sym_identical(&a, &b) {
            return Some(b);
        }
        match (a, b) {
            (Sym::List(x), Sym::List(y)) => {
                if x.len() != y.len() {
                    return None;
                }
                let mut out = Vec::with_capacity(x.len());
                for (u, v) in x.into_iter().zip(y) {
                    out.push(p90_ite(cond, u, v)?);
                }
                Some(Sym::List(out))
            }
            (
                Sym::Record { type_name: ta, fields: fa },
                Sym::Record { type_name: tb, fields: fb },
            ) => {
                if ta != tb || fa.len() != fb.len() {
                    return None;
                }
                let mut merged = Vec::with_capacity(fa.len());
                for (name, va) in fa {
                    let vb = fb
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, v)| v.clone())?;
                    merged.push((name, p90_ite(cond, va, vb)?));
                }
                Some(Sym::Record { type_name: ta, fields: merged })
            }
            (
                Sym::Option { some: ta, payload: pa },
                Sym::Option { some: tb, payload: pb },
            ) => Some(Sym::Option {
                some: Tag::merge(cond, &ta, &tb),
                payload: p90_ite_payload(cond, pa, pb)?,
            }),
            (
                Sym::Result { ok: ta, ok_payload: oa, err_payload: ea },
                Sym::Result { ok: tb, ok_payload: ob, err_payload: eb },
            ) => Some(Sym::Result {
                ok: Tag::merge(cond, &ta, &tb),
                ok_payload: p90_ite_payload(cond, oa, ob)?,
                err_payload: p90_ite_payload(cond, ea, eb)?,
            }),
            (a, b) => sym_ite(cond, a, b),
        }
    }

    #[cfg(feature = "z3-support")]
    fn p90_ite_payload(
        cond: &z3::ast::Bool,
        a: Option<Box<Sym>>,
        b: Option<Box<Sym>>,
    ) -> Option<Option<Box<Sym>>> {
        // Mirrors production `sym_ite_payload`: a slot
        // present on one side only is observed solely
        // under that side's tag, so it carries over.
        match (a, b) {
            (None, None) => Some(None),
            (Some(x), None) => Some(Some(x)),
            (None, Some(y)) => Some(Some(y)),
            (Some(x), Some(y)) => p90_ite(cond, *x, *y).map(|v| Some(Box::new(v))),
        }
    }

    #[cfg(feature = "z3-support")]
    fn p90_bind(st: &mut P90State, name: &str, v: Sym) -> Option<()> {
        st.writes.insert(name.to_string());
        if st.guided || p90_is_true(&st.pc) {
            // A guided state is a single decided path:
            // the bind is unconditional on it.
            st.env.insert(name.to_string(), v);
            return Some(());
        }
        match st.env.get(name) {
            None => {
                st.env.insert(name.to_string(), v);
                Some(())
            }
            Some(old) => {
                let old = old.clone();
                let merged = p90_ite(&st.pc, v, old)?;
                st.env.insert(name.to_string(), merged);
                Some(())
            }
        }
    }

    /// Append one element, in the forms the tokenize
    /// bodies need: a concrete list (the production
    /// semantics), the length-only list form (the count
    /// is the whole content the rows at stake read —
    /// gated on ctx.list_append), and a string grown by
    /// an indexed char (the CharStr bridge form) or by a
    /// numeral Char.
    #[cfg(feature = "z3-support")]
    fn p90_append(cur: Sym, item: Sym, ctx: &P90Ctx) -> Option<Sym> {
        match (cur, item) {
            (Sym::List(mut a), it) => {
                a.push(it);
                Some(Sym::List(a))
            }
            (Sym::ListLen(n), _) if ctx.list_append => Some(Sym::ListLen(z3::ast::Int::add(
                &[&n, &z3::ast::Int::from_i64(1)],
            ))),
            (Sym::Str(s), Sym::CharStr(cs)) => {
                Some(Sym::Str(z3::ast::String::concat(&[&s, &cs])))
            }
            (Sym::Str(s), Sym::Char(c)) => {
                let n = c.as_i64()?;
                let ch = char::from_u32(n as u32)?;
                let one = z3::ast::String::from_str(&ch.to_string()).ok()?;
                Some(Sym::Str(z3::ast::String::concat(&[&s, &one])))
            }
            _ => None,
        }
    }

    /// Assemble a denotation sub-evaluation's paths into
    /// one value: by ite where the Sym forms allow, else
    /// — the point case, where every path condition is a
    /// closed term — by solver concretization (exactly
    /// one live path).
    #[cfg(feature = "z3-support")]
    fn p90_assemble(paths: &[(z3::ast::Bool, Sym)]) -> Option<Sym> {
        if paths.len() == 1 {
            return Some(paths[0].1.clone());
        }
        let mut acc: Option<Sym> = None;
        let mut ite_ok = true;
        for (pc, v) in paths.iter().rev() {
            match acc.take() {
                None => acc = Some(v.clone()),
                Some(rest) => match p90_ite(pc, v.clone(), rest) {
                    Some(m) => acc = Some(m),
                    None => {
                        ite_ok = false;
                        break;
                    }
                },
            }
        }
        if ite_ok {
            return acc;
        }
        let mut live: Option<Sym> = None;
        for (pc, v) in paths {
            match p90_sat_all(&[pc.clone()]).0 {
                Some(true) => {
                    if live.is_some() {
                        return None;
                    }
                    live = Some(v.clone());
                }
                Some(false) => {}
                None => return None,
            }
        }
        live
    }

    /// Value-level pattern matching WITH constructor
    /// arms — the probe's replacement for production
    /// `sym_match`, which skips `IRPattern::Constructor`
    /// arms wholesale (`=> continue`), so any Match over
    /// an Option/Result constructor evaluates to None
    /// there. Discipline mirrors production: arms in
    /// order, first match wins; the match must be
    /// PROVABLY exhaustive (an unconditional arm, or the
    /// solver refutes the leftover condition) or the
    /// whole node is unencodable — never a silent
    /// fall-through. Payload binds support Variable and
    /// Wildcard sub-patterns only.
    #[cfg(feature = "z3-support")]
    fn p90_match(
        scrut: Sym,
        cases: &[(IRPattern, IRNode)],
        st: &mut P90State,
        ctx: &P90Ctx,
    ) -> Option<Sym> {
        // (condition, binds, body) per live arm.
        let mut arms: Vec<(z3::ast::Bool, Vec<(String, Sym)>, IRNode)> = Vec::new();
        for (pattern, body) in cases {
            match pattern {
                IRPattern::Wildcard => {
                    arms.push((p90_true(), Vec::new(), body.clone()));
                }
                IRPattern::Variable(name) => {
                    arms.push((
                        p90_true(),
                        vec![(name.clone(), scrut.clone())],
                        body.clone(),
                    ));
                }
                IRPattern::Literal(lit) => match sym_pattern_cond(&scrut, lit) {
                    PatternCond::Eq(cond) => arms.push((cond, Vec::new(), body.clone())),
                    PatternCond::Never => continue,
                    PatternCond::Undecidable => return None,
                },
                IRPattern::Constructor(name, pargs) => {
                    // (tag condition, payload) for this
                    // constructor over this scrutinee.
                    let (cond, payload): (z3::ast::Bool, Option<Sym>) = match (&scrut, name.as_str()) {
                        (Sym::Option { some, payload }, "Some") => match some {
                            Tag::True => (p90_true(), payload.as_deref().cloned()),
                            Tag::False => continue,
                            Tag::Free(t) => (t.clone(), payload.as_deref().cloned()),
                        },
                        (Sym::Option { some, .. }, "None") => match some {
                            Tag::True => continue,
                            Tag::False => (p90_true(), None),
                            Tag::Free(t) => (t.not(), None),
                        },
                        (
                            Sym::Result { ok, ok_payload, .. },
                            "Ok",
                        ) => match ok {
                            Tag::True => (p90_true(), ok_payload.as_deref().cloned()),
                            Tag::False => continue,
                            Tag::Free(t) => (t.clone(), ok_payload.as_deref().cloned()),
                        },
                        (
                            Sym::Result { ok, err_payload, .. },
                            "Err",
                        ) => match ok {
                            Tag::True => continue,
                            Tag::False => (p90_true(), err_payload.as_deref().cloned()),
                            Tag::Free(t) => (t.not(), err_payload.as_deref().cloned()),
                        },
                        _ => return None,
                    };
                    let mut binds = Vec::new();
                    match (pargs.len(), payload) {
                        (0, _) => {}
                        (1, Some(p)) => match &pargs[0] {
                            IRPattern::Variable(n) => binds.push((n.clone(), p)),
                            IRPattern::Wildcard => {}
                            _ => return None,
                        },
                        _ => return None,
                    }
                    arms.push((cond, binds, body.clone()));
                }
                IRPattern::Guard(..) => return None,
            }
        }
        if arms.is_empty() {
            return None;
        }
        // Exhaustiveness: leftover = none of the arm
        // conditions hold. An arm with a literally-true
        // condition settles it; otherwise ask the solver
        // (UNSAT without side conditions implies UNSAT
        // with them — the sound direction).
        let mut covered = p90_false();
        let mut unconditional = false;
        for (cond, _, _) in &arms {
            if p90_is_true(cond) {
                unconditional = true;
            }
            covered = p90_or(&covered, cond);
        }
        if !unconditional {
            let leftover = covered.not();
            match p90_sat_all(&[leftover]) {
                (Some(false), _) => {}
                _ => return None,
            }
        }
        // Evaluate the arm bodies under their binds
        // (env extended, then restored) and merge in
        // order with ite.
        let mut values: Vec<(z3::ast::Bool, Sym)> = Vec::new();
        for (cond, binds, body) in &arms {
            let saved: Vec<(String, Option<Sym>)> = binds
                .iter()
                .map(|(n, v)| {
                    let old = st.env.get(n).cloned();
                    st.env.insert(n.clone(), v.clone());
                    (n.clone(), old)
                })
                .collect();
            let v = p90_eval_value(body, st, ctx);
            for (n, old) in saved {
                match old {
                    Some(o) => {
                        st.env.insert(n, o);
                    }
                    None => {
                        st.env.remove(&n);
                    }
                }
            }
            values.push((cond.clone(), v?));
        }
        let (_, mut acc) = values.pop()?;
        for (cond, v) in values.into_iter().rev() {
            acc = p90_ite(&cond, v, acc)?;
        }
        Some(acc)
    }

    /// Expression-position evaluation: the production
    /// symbolic evaluator wherever it reaches, the
    /// probe's extensions where it does not (denoted
    /// calls the resolver leaves undenoted, Append in
    /// the probe's forms).
    #[cfg(feature = "z3-support")]
    fn p90_eval_value(node: &IRNode, st: &mut P90State, ctx: &P90Ctx) -> Option<Sym> {
        match node {
            IRNode::UnaryOp(IRUnaryOp::Clone, inner) => {
                // Probe-level extension, named for the build
                // plan: production `sym_unary_ir` has NO Clone
                // arm (verifier.rs, "outside the interpreter's
                // evaluable subset too" — the interpreter in
                // fact evaluates Clone fine; the hand bodies
                // execute 39/39). Clone is a value copy and
                // Sym terms are immutable, so identity is the
                // exact symbolic semantics.
                p90_eval_value(inner, st, ctx)
            }
            IRNode::Match { scrutinee, cases } => {
                // Probe-level constructor matching (see
                // p90_match): production `sym_match` skips
                // Constructor arms entirely, so a peek-style
                // Match evaluates to None there.
                let s = p90_eval_value(scrutinee, st, ctx)?;
                p90_match(s, cases, st, ctx)
            }
            IRNode::Call { function, args } => {
                let mut env = st.env.clone();
                if let Some(v) = sym_call(function, args, &mut env, &EvalCtx::root(ctx.index)) {
                    return Some(v);
                }
                // Probe denotation fallback — the
                // resolver extension's shape: a body
                // keyed by spec name, evaluated with the
                // callee's parameters bound to the
                // argument values, under the production
                // inlining depth cap.
                let body = ctx.denotations.get(function)?;
                let spec = ctx.index.get(function)?;
                if ctx.depth >= MAX_INLINE_DEPTH || spec.params.len() != args.len() {
                    return None;
                }
                let mut argv = Vec::with_capacity(args.len());
                for a in args {
                    argv.push(p90_eval_value(a, st, ctx)?);
                }
                let mut env = SymEnv::new();
                for (p, v) in spec.params.iter().zip(argv) {
                    env.insert(p.name.clone(), v);
                }
                let sub = P90Ctx {
                    index: ctx.index,
                    denotations: ctx.denotations,
                    fuel: ctx.fuel,
                    depth: ctx.depth + 1,
                    list_append: ctx.list_append,
                };
                let outcome = p90_run(body, env, &sub)?;
                if !p90_is_false(&outcome.exhausted) {
                    let gated = p90_and(&st.pc, &outcome.exhausted);
                    st.exhausted.push(gated);
                }
                if outcome.paths.is_empty() {
                    // The denoted call exhausted on every
                    // path: the caller's path is dead by
                    // exhaustion (recorded above), not
                    // unencodable. Kill it and hand back a
                    // placeholder no live path observes.
                    st.pc = p90_false();
                    return Some(Sym::Opaque);
                }
                p90_assemble(&outcome.paths)
            }
            IRNode::BinOp(IRBinOp::Append, l, r) => {
                let lv = p90_eval_value(l, st, ctx)?;
                let rv = p90_eval_value(r, st, ctx)?;
                p90_append(lv, rv, ctx)
            }
            _ => {
                let mut env = st.env.clone();
                sym_eval(node, &mut env, &EvalCtx::root(ctx.index))
            }
        }
    }

    /// Bind the Ok payload of a Result value (the
    /// TryPropagate semantics): on the Err path the
    /// enclosing function returns the Err at once; on
    /// the Ok path execution continues with the payload
    /// bound.
    #[cfg(feature = "z3-support")]
    fn p90_bind_try(st: &mut P90State, name: &str, v: Sym) -> Option<()> {
        match v {
            Sym::Result {
                ok,
                ok_payload,
                err_payload,
            } => match ok {
                Tag::True => {
                    let p = ok_payload?;
                    p90_bind(st, name, *p)
                }
                Tag::False => {
                    let e = err_payload?;
                    let ret = Sym::Result {
                        ok: Tag::False,
                        ok_payload: None,
                        err_payload: Some(e),
                    };
                    st.rets.push((st.pc.clone(), ret));
                    st.pc = p90_false();
                    Some(())
                }
                Tag::Free(t) => {
                    // Guided: decide the tag under the
                    // standing context first — at a point
                    // it is always decidable, and the
                    // decided path stays merge-free.
                    if st.guided {
                        match (p90_decide(st, &t), p90_decide(st, &t.not())) {
                            (Some(true), Some(false)) => {
                                let p = ok_payload?;
                                st.pc = p90_and(&st.pc, &t);
                                return p90_bind(st, name, *p);
                            }
                            (Some(false), Some(true)) => {
                                let e = err_payload?;
                                let ret = Sym::Result {
                                    ok: Tag::False,
                                    ok_payload: None,
                                    err_payload: Some(e),
                                };
                                st.rets.push((st.pc.clone(), ret));
                                st.pc = p90_false();
                                return Some(());
                            }
                            _ => {
                                st.guided = false;
                            }
                        }
                    }
                    let p = ok_payload?;
                    let e = err_payload?;
                    let err_ret = Sym::Result {
                        ok: Tag::False,
                        ok_payload: None,
                        err_payload: Some(e),
                    };
                    let err_pc = p90_and(&st.pc, &t.not());
                    st.rets.push((err_pc, err_ret));
                    st.pc = p90_and(&st.pc, &t);
                    p90_bind(st, name, *p)
                }
            },
            _ => None,
        }
    }
    /// Statement-mode evaluation. Mirrors the compiled
    /// semantics the cross-validation runner executes:
    /// Alloc binds, Assign rebinds, Break leaves the
    /// innermost loop, Return leaves the function, and
    /// loops repeat — bounded by ctx.fuel, with the
    /// paths that wanted more recorded as exhaustion.
    #[cfg(feature = "z3-support")]
    fn p90_eval(node: &IRNode, st: &mut P90State, ctx: &P90Ctx) -> Option<()> {
        if p90_is_false(&st.pc) {
            return Some(());
        }
        match node {
            IRNode::Block(stmts) => {
                for s in stmts {
                    p90_eval(s, st, ctx)?;
                }
                Some(())
            }
            IRNode::Alloc { name, initial, .. } => {
                if let IRNode::TryPropagate(inner) = &**initial {
                    let v = p90_eval_value(inner, st, ctx)?;
                    if p90_is_false(&st.pc) {
                        // The call died by exhaustion
                        // mid-statement; the path is dead
                        // and its value a placeholder.
                        return Some(());
                    }
                    return p90_bind_try(st, name, v);
                }
                let v = p90_eval_value(initial, st, ctx)?;
                if p90_is_false(&st.pc) {
                    return Some(());
                }
                st.last = Some(v.clone());
                p90_bind(st, name, v)
            }
            IRNode::Assign { target, value } => {
                let IRNode::Var(name) = &**target else {
                    return None;
                };
                let v = p90_eval_value(value, st, ctx)?;
                if p90_is_false(&st.pc) {
                    return Some(());
                }
                st.last = Some(v.clone());
                p90_bind(st, name, v)
            }
            IRNode::Return(inner) => {
                let v = p90_eval_value(inner, st, ctx)?;
                st.rets.push((st.pc.clone(), v));
                st.pc = p90_false();
                Some(())
            }
            IRNode::Break { .. } => {
                st.breaks.push(st.pc.clone());
                st.pc = p90_false();
                Some(())
            }
            IRNode::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = match p90_eval_value(condition, st, ctx)? {
                    Sym::Bool(b) => b,
                    _ => return None,
                };
                if st.guided {
                    // Guided path: decide the condition
                    // under the standing context and take
                    // the single decided branch in place —
                    // no clone, no merge.
                    let can_then = p90_decide(st, &cond);
                    let can_else = p90_decide(st, &cond.not());
                    match (can_then, can_else) {
                        (Some(true), Some(false)) => {
                            st.pc = p90_and(&st.pc, &cond);
                            return p90_eval(then_branch, st, ctx);
                        }
                        (Some(false), Some(true)) => {
                            st.pc = p90_and(&st.pc, &cond.not());
                            return p90_eval(else_branch, st, ctx);
                        }
                        _ => {
                            // Undecidable here: merging
                            // mode from this point on.
                            st.guided = false;
                        }
                    }
                }
                let base_pc = st.pc.clone();
                let fresh = |pc: z3::ast::Bool| P90State {
                    env: st.env.clone(),
                    pc,
                    rets: Vec::new(),
                    breaks: Vec::new(),
                    exhausted: Vec::new(),
                    last: None,
                    writes: std::collections::HashSet::new(),
                    assumptions: st.assumptions.clone(),
                    guided: false,
                };
                let mut then_st = fresh(p90_and(&base_pc, &cond));
                p90_eval(then_branch, &mut then_st, ctx)?;
                let mut else_st = fresh(p90_and(&base_pc, &cond.not()));
                p90_eval(else_branch, &mut else_st, ctx)?;
                // Continuation: the paths that neither
                // returned nor broke inside either branch.
                let mut cont = base_pc.clone();
                for (pc, _) in then_st.rets.iter().chain(else_st.rets.iter()) {
                    cont = p90_and(&cont, &pc.not());
                }
                for pc in then_st.breaks.iter().chain(else_st.breaks.iter()) {
                    cont = p90_and(&cont, &pc.not());
                }
                st.rets.extend(then_st.rets.iter().cloned());
                st.rets.extend(else_st.rets.iter().cloned());
                st.breaks.extend(then_st.breaks.iter().cloned());
                st.breaks.extend(else_st.breaks.iter().cloned());
                st.exhausted.extend(then_st.exhausted.iter().cloned());
                st.exhausted.extend(else_st.exhausted.iter().cloned());
                if p90_is_false(&cont) {
                    // No live continuation: the merged
                    // environment is dead. Keep the pre-If
                    // one — it is never observed on a live
                    // path.
                    st.pc = cont;
                    return Some(());
                }
                // Merge the branch environments. A
                // variable neither branch wrote carries
                // the pre-If value on both sides. A
                // written variable merges by ite when
                // both sides hold it; a variable only one
                // side holds is branch-local (read, for
                // the bodies at stake, only inside its
                // own branch).
                let mut merged = SymEnv::new();
                let keys: std::collections::HashSet<&String> = then_st
                    .env
                    .keys()
                    .chain(else_st.env.keys())
                    .collect();
                for k in keys {
                    let written =
                        then_st.writes.contains(k) || else_st.writes.contains(k);
                    match (then_st.env.get(k), else_st.env.get(k)) {
                        (Some(a), Some(b)) if written => {
                            merged.insert(k.clone(), p90_ite(&cond, a.clone(), b.clone())?);
                        }
                        (Some(a), _) => {
                            merged.insert(k.clone(), a.clone());
                        }
                        (None, Some(b)) => {
                            merged.insert(k.clone(), b.clone());
                        }
                        (None, None) => unreachable!(),
                    }
                }
                st.env = merged;
                st.writes.extend(then_st.writes.iter().cloned());
                st.writes.extend(else_st.writes.iter().cloned());
                st.last = match (then_st.last, else_st.last) {
                    (Some(a), Some(b)) => p90_ite(&cond, a, b),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => st.last.clone(),
                };
                st.pc = cont;
                Some(())
            }
            IRNode::While {
                condition, body, ..
            } => p90_loop(Some(condition), body, st, ctx),
            IRNode::Loop { body } => p90_loop(None, body, st, ctx),
            IRNode::BinOp(IRBinOp::Append, l, r) => {
                // A statement-position Append rebinds
                // its target (the compiled rendering is
                // a push on the binding).
                let IRNode::Var(name) = &**l else {
                    return None;
                };
                let cur = st.env.get(name)?.clone();
                let item = p90_eval_value(r, st, ctx)?;
                if p90_is_false(&st.pc) {
                    return Some(());
                }
                let v = p90_append(cur, item, ctx)?;
                st.last = Some(v.clone());
                p90_bind(st, name, v)
            }
            _ => {
                // Any other statement is an expression
                // statement: its value is the block's
                // running value, discarded unless last.
                let v = p90_eval_value(node, st, ctx)?;
                st.last = Some(match st.last.take() {
                    Some(old) if !p90_is_true(&st.pc) => p90_ite(&st.pc, v, old)?,
                    _ => v,
                });
                Some(())
            }
        }
    }

    /// Solver decision under the state's standing
    /// context (assumptions + Char side conditions +
    /// pc): Some(true) = satisfiable, Some(false) =
    /// refuted, None = the solver could not decide.
    #[cfg(feature = "z3-support")]
    fn p90_decide(st: &P90State, f: &z3::ast::Bool) -> Option<bool> {
        let mut assertions: Vec<z3::ast::Bool> = st.assumptions.clone();
        for v in st.env.values() {
            collect_char_conditions(v, &mut assertions);
        }
        assertions.push(st.pc.clone());
        assertions.push(f.clone());
        p90_sat_all(&assertions).0
    }

    /// One loop (While or Loop), unrolled to ctx.fuel
    /// iterations. Two modes. GUIDED: while the solver
    /// decides the condition under the current path
    /// (forced-true or forced-false), the loop follows
    /// the single decided branch — the path condition
    /// accumulates the decided facts and NO ite merge
    /// happens, so terms stay one-iteration sized (the
    /// measured alternative, pc-narrowing from the
    /// start, grows ~×8 per iteration on the skip body:
    /// 1/3/15/118/896 ms at fuels 0–4 on a one-trip
    /// point). MERGING: once a condition is genuinely
    /// undecided, the loop narrows the pc and merges
    /// binds by ite, as a standard symbolic unrolling.
    /// In both modes the paths that would iterate past
    /// the fuel are recorded as exhaustion and excluded
    /// from the continuation — never silently treated
    /// as exits (the BMC discipline).
    #[cfg(feature = "z3-support")]
    fn p90_loop(
        condition: Option<&IRNode>,
        body: &IRNode,
        st: &mut P90State,
        ctx: &P90Ctx,
    ) -> Option<()> {
        let breaks0 = st.breaks.len();
        let mut iter = 0usize;
        let mut merging = false;
        loop {
            let c = match condition {
                Some(cn) => match p90_eval_value(cn, st, ctx)? {
                    Sym::Bool(b) => b,
                    _ => return None,
                },
                None => p90_true(),
            };
            let live = st.pc.clone();
            if !merging {
                // Guided decisions: does any live path
                // iterate (pc ∧ c satisfiable)? Does any
                // live path exit (pc ∧ ¬c satisfiable)?
                let can_iter = p90_decide(st, &c);
                let can_exit = p90_decide(st, &c.not());
                match (can_iter, can_exit) {
                    (Some(false), _) => {
                        // Forced exit: no exhaustion, the
                        // loop is done on every live path.
                        let mut exit_pc = live;
                        for b in st.breaks.iter().skip(breaks0) {
                            exit_pc = p90_or(&exit_pc, b);
                        }
                        st.breaks.truncate(breaks0);
                        st.pc = exit_pc;
                        return Some(());
                    }
                    (_, Some(false)) => {
                        // Forced iteration.
                        if iter >= ctx.fuel {
                            st.exhausted.push(live);
                            let mut exit_pc = p90_false();
                            for b in st.breaks.iter().skip(breaks0) {
                                exit_pc = p90_or(&exit_pc, b);
                            }
                            st.breaks.truncate(breaks0);
                            st.pc = exit_pc;
                            return Some(());
                        }
                        st.pc = p90_and(&live, &c);
                        p90_eval(body, st, ctx)?;
                        iter += 1;
                        if p90_is_false(&st.pc) {
                            let mut exit_pc = p90_false();
                            for b in st.breaks.iter().skip(breaks0) {
                                exit_pc = p90_or(&exit_pc, b);
                            }
                            st.breaks.truncate(breaks0);
                            st.pc = exit_pc;
                            return Some(());
                        }
                        continue;
                    }
                    _ => {
                        // Genuinely symbolic (or the
                        // solver could not decide): merge
                        // from here on.
                        merging = true;
                        st.guided = false;
                    }
                }
            }
            if iter >= ctx.fuel {
                // Out of fuel: the paths where the loop
                // still wants another iteration are the
                // exhaustion; the paths where it does not
                // (plus this loop's break paths) leave.
                let still = p90_and(&live, &c);
                if !p90_is_false(&still) {
                    st.exhausted.push(still);
                }
                let mut exit_pc = p90_and(&live, &c.not());
                for b in st.breaks.iter().skip(breaks0) {
                    exit_pc = p90_or(&exit_pc, b);
                }
                st.breaks.truncate(breaks0);
                st.pc = exit_pc;
                return Some(());
            }
            let iter_pc = p90_and(&live, &c);
            if p90_is_false(&iter_pc) {
                // The loop has exited on every live path.
                let mut exit_pc = live;
                for b in st.breaks.iter().skip(breaks0) {
                    exit_pc = p90_or(&exit_pc, b);
                }
                st.breaks.truncate(breaks0);
                st.pc = exit_pc;
                return Some(());
            }
            st.pc = iter_pc;
            p90_eval(body, st, ctx)?;
            iter += 1;
            if p90_is_false(&st.pc) {
                // Every path through the body returned
                // or broke: only the break paths fall
                // through the loop.
                let mut exit_pc = p90_false();
                for b in st.breaks.iter().skip(breaks0) {
                    exit_pc = p90_or(&exit_pc, b);
                }
                st.breaks.truncate(breaks0);
                st.pc = exit_pc;
                return Some(());
            }
        }
    }

    /// Run a body to its outcome: the per-path outputs
    /// and the exhaustion disjunction.
    #[cfg(feature = "z3-support")]
    /// Clone-elision rewrite: `UnaryOp(Clone, x)` → `x`,
    /// recursively. Production `sym_unary_ir` has no Clone
    /// arm, and Clone nodes sit INSIDE delegated subtrees
    /// (the skip body's While condition is a Match over
    /// `peek(Clone cur)`), so the probe cannot intercept
    /// them node-by-node — it rewrites the body trees up
    /// front. Sound because Clone is a value copy and Sym
    /// terms are immutable; the production fix is the
    /// one-arm change this rewrite stands in for.
    #[cfg(feature = "z3-support")]
    fn p90_decloned(node: &IRNode) -> IRNode {
        let d = |n: &IRNode| Box::new(p90_decloned(n));
        match node {
            IRNode::UnaryOp(IRUnaryOp::Clone, inner) => p90_decloned(inner),
            IRNode::Block(stmts) => {
                IRNode::Block(stmts.iter().map(|s| p90_decloned(s)).collect())
            }
            IRNode::Let { name, value, body } => IRNode::Let {
                name: name.clone(),
                value: d(value),
                body: d(body),
            },
            IRNode::If { condition, then_branch, else_branch } => IRNode::If {
                condition: d(condition),
                then_branch: d(then_branch),
                else_branch: d(else_branch),
            },
            IRNode::While { condition, body, invariant } => IRNode::While {
                condition: d(condition),
                body: d(body),
                invariant: invariant.clone(),
            },
            IRNode::For { var, start, end, body } => IRNode::For {
                var: var.clone(),
                start: d(start),
                end: d(end),
                body: d(body),
            },
            IRNode::Match { scrutinee, cases } => IRNode::Match {
                scrutinee: d(scrutinee),
                cases: cases
                    .iter()
                    .map(|(p, b)| (p.clone(), p90_decloned(b)))
                    .collect(),
            },
            IRNode::Call { function, args } => IRNode::Call {
                function: function.clone(),
                args: args.iter().map(|a| p90_decloned(a)).collect(),
            },
            IRNode::Return(inner) => IRNode::Return(d(inner)),
            IRNode::BinOp(op, l, r) => IRNode::BinOp(*op, d(l), d(r)),
            IRNode::UnaryOp(op, inner) => IRNode::UnaryOp(*op, d(inner)),
            IRNode::Lambda { params, body } => IRNode::Lambda {
                params: params.clone(),
                body: d(body),
            },
            IRNode::Closure { params, captures, body } => IRNode::Closure {
                params: params.clone(),
                captures: captures.clone(),
                body: d(body),
            },
            IRNode::Collection { kind, elements } => IRNode::Collection {
                kind: *kind,
                elements: elements.iter().map(|e| p90_decloned(e)).collect(),
            },
            IRNode::Map { function, over } => IRNode::Map {
                function: d(function),
                over: d(over),
            },
            IRNode::Filter { predicate, over } => IRNode::Filter {
                predicate: d(predicate),
                over: d(over),
            },
            IRNode::Fold { operation, initial, over } => IRNode::Fold {
                operation: d(operation),
                initial: d(initial),
                over: d(over),
            },
            IRNode::Sort { comparator, collection } => IRNode::Sort {
                comparator: d(comparator),
                collection: d(collection),
            },
            IRNode::Alloc { name, ty, initial } => IRNode::Alloc {
                name: name.clone(),
                ty: ty.clone(),
                initial: d(initial),
            },
            IRNode::Assign { target, value } => IRNode::Assign {
                target: target.clone(),
                value: d(value),
            },
            IRNode::Typed { node: inner, ty } => IRNode::Typed {
                node: d(inner),
                ty: ty.clone(),
            },
            IRNode::Index(c, i) => IRNode::Index(d(c), d(i)),
            IRNode::Loop { body } => IRNode::Loop { body: d(body) },
            IRNode::Break { value } => IRNode::Break {
                value: value.as_ref().map(|v| d(v)),
            },
            IRNode::StructDef { name, fields, body } => IRNode::StructDef {
                name: name.clone(),
                fields: fields.clone(),
                body: d(body),
            },
            IRNode::TryPropagate(inner) => IRNode::TryPropagate(d(inner)),
            IRNode::FieldAccess { base, field } => IRNode::FieldAccess {
                base: d(base),
                field: field.clone(),
            },
            IRNode::RecordLit { type_name, fields } => IRNode::RecordLit {
                type_name: type_name.clone(),
                fields: fields
                    .iter()
                    .map(|(n, v)| (n.clone(), p90_decloned(v)))
                    .collect(),
            },
            other => other.clone(),
        }
    }

    #[cfg(feature = "z3-support")]
    fn p90_run(body: &IRNode, env: SymEnv, ctx: &P90Ctx) -> Option<P90Outcome> {
        let mut st = P90State {
            env,
            pc: p90_true(),
            rets: Vec::new(),
            breaks: Vec::new(),
            exhausted: Vec::new(),
            last: None,
            writes: std::collections::HashSet::new(),
            assumptions: Vec::new(),
            guided: true,
        };
        p90_eval(&p90_decloned(body), &mut st, ctx)?;
        let mut paths = st.rets.clone();
        if let Some(v) = st.last.clone() {
            // The fall-through path is genuine only if
            // the final pc is satisfiable — a body whose
            // every branch returned leaves a pc that is
            // unsatisfiable but not literally `false`
            // (it is the negation of the return pcs), and
            // its `last` (a branch's running value) is
            // not an output. Ask the solver; keep the
            // path only when fall-through is possible or
            // undecidable. A second guard: when returns
            // exist, a fall-through whose value is of a
            // different Sym family than the returns is a
            // statement artifact (a branch's running
            // value on an exhaustion-adjacent path), not
            // an output of this body.
            let verdict = if p90_is_false(&st.pc) {
                Some(false)
            } else {
                p90_decide(&st, &p90_true())
            };
            let family_match = match st.rets.first() {
                None => true,
                Some((_, rv)) => {
                    std::mem::discriminant(&v) == std::mem::discriminant(rv)
                }
            };
            if verdict != Some(false) && family_match {
                paths.push((st.pc.clone(), v));
            }
        }
        if paths.is_empty() && st.exhausted.is_empty() {
            return None;
        }
        // An empty path set WITH an exhaustion record is
        // a real outcome (every path wanted more fuel);
        // the judges read it as unresolved, never Proven.
        Some(P90Outcome {
            paths,
            exhausted: p90_or_all(&st.exhausted),
        })
    }

    /// Satisfiability of a conjunction of Bool terms,
    /// under the production per-constraint timeout
    /// (5 s): (Some(true) = SAT, Some(false) = UNSAT,
    /// None = solver Unknown), plus wall time.
    #[cfg(feature = "z3-support")]
    fn p90_sat_all(terms: &[z3::ast::Bool]) -> (Option<bool>, u64) {
        let z = Z3Session::new(5_000);
        for t in terms {
            z.assert(t);
        }
        let t0 = std::time::Instant::now();
        let r = z.check();
        let ms = t0.elapsed().as_millis() as u64;
        (
            match r {
                SmtResult::Sat(_) => Some(true),
                SmtResult::Unsat => Some(false),
                SmtResult::Unknown | SmtResult::Timeout => None,
            },
            ms,
        )
    }

    #[cfg(feature = "z3-support")]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum P90Tri {
        Proven,
        Refuted,
        Unknown,
    }

    #[cfg(feature = "z3-support")]
    fn p90_tri_name(t: P90Tri) -> &'static str {
        match t {
            P90Tri::Proven => "Proven",
            P90Tri::Refuted => "Refuted",
            P90Tri::Unknown => "Unknown",
        }
    }

    /// The Char range side conditions for a spec's
    /// parameters, as terms (verify_constraint asserts
    /// the same conditions inside its own sessions).
    #[cfg(feature = "z3-support")]
    fn p90_side_conditions(spec: &FunctionSpec, env: &SymEnv) -> Vec<z3::ast::Bool> {
        let mut side = Vec::new();
        for p in &spec.params {
            if let Some(sym) = env.get(&p.name) {
                collect_char_conditions(sym, &mut side);
            }
        }
        side
    }

    /// The BMC-discipline verdict for one formula over
    /// one probe evaluation: Refuted when a countermodel
    /// exists on a path where no loop exhausted its
    /// fuel; Proven when no such countermodel exists on
    /// any path AND exhaustion is unreachable; otherwise
    /// Unknown (the probe's Inconclusive). The formula
    /// is built per output path from the environment
    /// extended with that path's output.
    #[cfg(feature = "z3-support")]
    fn p90_judge_with(
        spec: &FunctionSpec,
        env: &SymEnv,
        outcome: &P90Outcome,
        mut formula_of: impl FnMut(&SymEnv) -> Option<z3::ast::Bool>,
    ) -> (P90Tri, u64) {
        let side = p90_side_conditions(spec, env);
        let mut total_ms = 0u64;
        let mut any_unknown = false;
        for (pc, output) in &outcome.paths {
            let mut full_env = env.clone();
            full_env.insert("output".to_string(), output.clone());
            let Some(formula) = formula_of(&full_env) else {
                return (P90Tri::Unknown, total_ms);
            };
            let neg = p90_and(&formula.not(), &outcome.exhausted.not());
            let neg = p90_and(&neg, pc);
            let mut terms = side.clone();
            terms.push(neg);
            let (sat, ms) = p90_sat_all(&terms);
            total_ms += ms;
            match sat {
                Some(true) => return (P90Tri::Refuted, total_ms),
                Some(false) => {}
                None => any_unknown = true,
            }
        }
        let mut terms = side;
        terms.push(outcome.exhausted.clone());
        let (sat, ms) = p90_sat_all(&terms);
        total_ms += ms;
        match (sat, any_unknown) {
            (Some(false), false) => (P90Tri::Proven, total_ms),
            _ => (P90Tri::Unknown, total_ms),
        }
    }

    /// p90_judge_with for a spec Constraint row, encoded
    /// through the production constraint encoder.
    #[cfg(feature = "z3-support")]
    fn p90_judge_row(
        row: &Constraint,
        spec: &FunctionSpec,
        env: &SymEnv,
        outcome: &P90Outcome,
        index: &SpecIndex,
    ) -> (P90Tri, u64) {
        let ctx = EvalCtx::root(index);
        p90_judge_with(spec, env, outcome, |full_env| {
            encode_constraint(row, full_env, &ctx)
        })
    }

    /// At a battery point the evaluation's path
    /// conditions are closed terms: exactly one path is
    /// live. Returns that path's output — or None when
    /// the fuel ran out on the live path (exhaustion
    /// satisfiable), when two paths are live, or when
    /// the solver cannot decide a path.
    #[cfg(feature = "z3-support")]
    fn p90_concretize(outcome: &P90Outcome) -> Option<Sym> {
        let (exh_sat, _) = p90_sat_all(&[outcome.exhausted.clone()]);
        if exh_sat != Some(false) {
            return None;
        }
        let mut live: Option<Sym> = None;
        for (pc, v) in &outcome.paths {
            match p90_sat_all(&[pc.clone()]).0 {
                Some(true) => {
                    if live.is_some() {
                        return None;
                    }
                    live = Some(v.clone());
                }
                Some(false) => {}
                None => return None,
            }
        }
        live
    }
    // ── Phase 90 point/mirror helpers ──────────────────

    #[cfg(feature = "z3-support")]
    #[derive(Debug, Clone, PartialEq)]
    struct P90Tok {
        kind: i64,
        lexeme: String,
        line: i64,
        col: i64,
    }

    #[cfg(feature = "z3-support")]
    #[derive(Debug, Clone)]
    struct P90St {
        source: String,
        pos: i64,
        line: i64,
        col: i64,
        tokens: Vec<P90Tok>,
    }

    #[cfg(feature = "z3-support")]
    fn p90_st(source: &str, pos: i64, line: i64, col: i64) -> P90St {
        P90St {
            source: source.to_string(),
            pos,
            line,
            col,
            tokens: Vec::new(),
        }
    }

    #[cfg(feature = "z3-support")]
    fn p90_state_sym(st: &P90St) -> Sym {
        let int = |n: i64| Sym::Int(z3::ast::Int::from_i64(n));
        let tokens = st
            .tokens
            .iter()
            .map(|t| Sym::Record {
                type_name: "Token".into(),
                fields: vec![
                    ("kind".into(), int(t.kind)),
                    (
                        "lexeme".into(),
                        Sym::Str(z3::ast::String::from_str(&t.lexeme).unwrap()),
                    ),
                    ("line".into(), int(t.line)),
                    ("col".into(), int(t.col)),
                ],
            })
            .collect();
        Sym::Record {
            type_name: "TokenizerState".into(),
            fields: vec![
                (
                    "source".into(),
                    Sym::Str(z3::ast::String::from_str(&st.source).unwrap()),
                ),
                // pos as (n + 0), NOT a numeral: a numeral
                // index into a literal string takes
                // sym_index's exact path, whose out-of-
                // bounds case is unencodable — and peek's
                // eager If evaluates that branch even when
                // dead, so a literal EOF point would not
                // evaluate at all (the Phase 87 literal-OOB
                // artifact). The unfolded term routes every
                // index through the totalized CharStr path
                // while remaining solver-equal to n.
                (
                    "pos".into(),
                    Sym::Int(z3::ast::Int::add(&[
                        &z3::ast::Int::from_i64(st.pos),
                        &z3::ast::Int::from_i64(0),
                    ])),
                ),
                ("line".into(), int(st.line)),
                ("col".into(), int(st.col)),
                ("tokens".into(), Sym::List(tokens)),
            ],
        }
    }

    #[cfg(feature = "z3-support")]
    fn p90_result_sym(r: &Result<P90St, String>) -> Sym {
        match r {
            Ok(st) => Sym::Result {
                ok: Tag::True,
                ok_payload: Some(Box::new(p90_state_sym(st))),
                err_payload: None,
            },
            Err(e) => Sym::Result {
                ok: Tag::False,
                ok_payload: None,
                err_payload: Some(Box::new(Sym::Str(
                    z3::ast::String::from_str(e).unwrap(),
                ))),
            },
        }
    }

    #[cfg(feature = "z3-support")]
    fn p90_field<'s>(rec: &'s Sym, name: &str) -> Option<&'s Sym> {
        match rec {
            Sym::Record { fields, .. } => {
                fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
            }
            _ => None,
        }
    }

    /// An equality formula between two Sym values, in
    /// the fragment the point outputs live in (records
    /// of Int/Str/List, token records, Result of state).
    /// None outside that fragment.
    #[cfg(feature = "z3-support")]
    fn p90_eq(a: &Sym, b: &Sym) -> Option<z3::ast::Bool> {
        match (a, b) {
            (Sym::Int(x), Sym::Int(y)) => Some(x.eq(y)),
            (Sym::Bool(x), Sym::Bool(y)) => Some(x.eq(y)),
            (Sym::Str(x), Sym::Str(y)) => Some(x.eq(y)),
            (Sym::Char(x), Sym::Char(y)) => Some(x.eq(y)),
            (Sym::List(xs), Sym::List(ys)) => {
                if xs.len() != ys.len() {
                    return Some(p90_false());
                }
                let mut acc = p90_true();
                for (x, y) in xs.iter().zip(ys.iter()) {
                    acc = p90_and(&acc, &p90_eq(x, y)?);
                }
                Some(acc)
            }
            (Sym::ListLen(x), Sym::ListLen(y)) => Some(x.eq(y)),
            (Sym::List(xs), Sym::ListLen(n)) => {
                Some(z3::ast::Int::from_i64(xs.len() as i64).eq(n))
            }
            (Sym::ListLen(n), Sym::List(ys)) => {
                Some(n.eq(&z3::ast::Int::from_i64(ys.len() as i64)))
            }
            (
                Sym::Record {
                    fields: fa,
                    type_name: ta,
                },
                Sym::Record {
                    fields: fb,
                    type_name: tb,
                },
            ) => {
                if ta != tb || fa.len() != fb.len() {
                    return Some(p90_false());
                }
                let mut acc = p90_true();
                for (name, va) in fa {
                    let vb = fb.iter().find(|(n, _)| n == name).map(|(_, v)| v)?;
                    acc = p90_and(&acc, &p90_eq(va, vb)?);
                }
                Some(acc)
            }
            (
                Sym::Result {
                    ok: ta,
                    ok_payload: oa,
                    err_payload: ea,
                },
                Sym::Result {
                    ok: tb,
                    ok_payload: ob,
                    err_payload: eb,
                },
            ) => {
                let mut acc = ta.term().eq(&tb.term());
                if let (Some(x), Some(y)) = (oa, ob) {
                    let eq = p90_eq(x, y)?;
                    acc = p90_and(&acc, &p90_or(&tb.term().not(), &eq));
                }
                if let (Some(x), Some(y)) = (ea, eb) {
                    let eq = p90_eq(x, y)?;
                    acc = p90_and(&acc, &p90_or(&tb.term(), &eq));
                }
                Some(acc)
            }
            _ => None,
        }
    }

    /// Solver-decidable equality of two point values:
    /// true when the equality formula's negation is
    /// UNSAT.
    #[cfg(feature = "z3-support")]
    fn p90_values_equal(a: &Sym, b: &Sym) -> bool {
        match p90_eq(a, b) {
            Some(f) => matches!(p90_sat_all(&[f.not()]).0, Some(false)),
            None => false,
        }
    }

    // The mirrors, simulated on chars (the cross-
    // validation semantics): whitespace is exactly
    // ' ' '\t' '\n' '\r'; next_char tracks line/col
    // through newlines; tokens stamp the START state's
    // line/col (D4a). Each returns its loop trip counts
    // beside the outcome — the predicted minimal fuel.
    #[cfg(feature = "z3-support")]
    fn p90_mirror_peek(st: &P90St) -> Option<char> {
        st.source.chars().nth(st.pos as usize)
    }

    #[cfg(feature = "z3-support")]
    fn p90_mirror_next(st: &P90St) -> P90St {
        let mut out = st.clone();
        match p90_mirror_peek(st) {
            Some('\n') => {
                out.line += 1;
                out.col = 1;
            }
            _ => {
                out.col += 1;
            }
        }
        out.pos += 1;
        out
    }

    #[cfg(feature = "z3-support")]
    fn p90_mirror_ws(c: char) -> bool {
        matches!(c, ' ' | '\t' | '\n' | '\r')
    }

    #[cfg(feature = "z3-support")]
    fn p90_mirror_skip(st: &P90St) -> (P90St, usize) {
        let mut cur = st.clone();
        let mut trips = 0;
        while p90_mirror_peek(&cur).map_or(false, p90_mirror_ws) {
            cur = p90_mirror_next(&cur);
            trips += 1;
        }
        (cur, trips)
    }

    #[cfg(feature = "z3-support")]
    fn p90_mirror_one(st: &P90St) -> (Result<P90St, String>, usize, usize) {
        let (s, skip_trips) = p90_mirror_skip(st);
        let ident_start = |c: char| c.is_ascii_lowercase() || c.is_ascii_uppercase() || c == '_';
        let ident_cont =
            |c: char| ident_start(c) || c.is_ascii_digit();
        match p90_mirror_peek(&s) {
            None => (Ok(s), skip_trips, 0),
            Some(c) if ident_start(c) => {
                let mut lexeme = String::new();
                let mut cur = s.clone();
                let mut trips = 0;
                while p90_mirror_peek(&cur).map_or(false, ident_cont) {
                    lexeme.push(p90_mirror_peek(&cur).unwrap());
                    cur = p90_mirror_next(&cur);
                    trips += 1;
                }
                let mut out = cur;
                out.tokens.push(P90Tok {
                    kind: 0,
                    lexeme,
                    line: s.line,
                    col: s.col,
                });
                (Ok(out), skip_trips, trips)
            }
            Some(c) => {
                let kind = match c {
                    '(' => Some(3),
                    ')' => Some(4),
                    '{' => Some(5),
                    '}' => Some(6),
                    ':' => Some(7),
                    ',' => Some(9),
                    '<' => Some(10),
                    '>' => Some(11),
                    '=' => Some(12),
                    _ => None,
                };
                match kind {
                    Some(kind) => {
                        let mut out = p90_mirror_next(&s);
                        out.tokens.push(P90Tok {
                            kind,
                            lexeme: c.to_string(),
                            line: s.line,
                            col: s.col,
                        });
                        (Ok(out), skip_trips, 0)
                    }
                    None => (
                        Err(format!("Unexpected character '{}' at line {}", c, s.line)),
                        skip_trips,
                        0,
                    ),
                }
            }
        }
    }

    #[cfg(feature = "z3-support")]
    fn p90_mirror_all(st: &P90St) -> (Result<Vec<P90Tok>, String>, usize, usize) {
        let mut cur = st.clone();
        let mut outer = 0usize;
        let mut inner_max = 0usize;
        loop {
            outer += 1;
            let (r, sk, id) = p90_mirror_one(&cur);
            // A step's own fuel need: its skip While needs
            // sk; its ident Loop runs id + 1 iterations
            // (the last one observes the non-ident char
            // and breaks) when the ident path is taken.
            inner_max = inner_max.max(sk).max(if id >= 1 { id + 1 } else { 0 });
            match r {
                Ok(next) => {
                    let progressed = next.pos != cur.pos;
                    cur = next;
                    if !progressed {
                        break;
                    }
                }
                Err(e) => return (Err(e), outer, inner_max),
            }
        }
        (Ok(cur.tokens), outer, inner_max)
    }

    /// A production seed body pulled from a real synthesis
    /// pool by provenance kind — the route
    /// tokenizer_benchmark.rs's `pool_seed` uses. Phase 90
    /// needs next_char's `record_completion` seed as a
    /// DENOTATION: `SpecIndex::callee_body` does not include
    /// the record-completion builder (it is seed-only), so
    /// today NO body that calls next_char is Z3-evaluable at
    /// all — a resolver gap one chain-entry wide, measured
    /// here by supplying the seed through the probe's own
    /// denotation map.
    #[cfg(feature = "z3-support")]
    fn p90_seed_body(module: &Spec, spec: &FunctionSpec, kind: &str) -> IRNode {
        let candidates =
            crate::synthesis::engine::synthesize_with_module_spec(spec, 1, module)
                .expect("synthesis runs for the seed pool");
        candidates
            .iter()
            .find(|c| {
                matches!(&c.provenance, crate::synthesis::engine::Provenance::ConstraintInduced { kind: k } if k == kind)
            })
            .unwrap_or_else(|| panic!("{} must have a {} seed", spec.name, kind))
            .body
            .clone()
    }

    /// The Phase 85 hand-supplied mirror body for
    /// skip_whitespace, rebuilt verbatim (Alloc cur;
    /// While over a peek Match; Assign cur = next_char;
    /// Return cur) — the body the resolver cannot
    /// denote and the Phase 90 probes judge.
    #[cfg(feature = "z3-support")]
    fn p90_hand_skip_body() -> IRNode {
        let v = |n: &str| IRNode::Var(n.to_string());
        let call = |fu: &str, args: Vec<IRNode>| IRNode::Call {
            function: fu.to_string(),
            args,
        };
        IRNode::Block(vec![
            IRNode::Alloc {
                name: "cur".to_string(),
                ty: TypeRef::Named("TokenizerState".to_string()),
                initial: Box::new(v("state")),
            },
            IRNode::While {
                condition: Box::new(IRNode::Match {
                    scrutinee: Box::new(call(
                        "peek",
                        vec![IRNode::UnaryOp(IRUnaryOp::Clone, Box::new(v("cur")))],
                    )),
                    cases: vec![
                        (
                            IRPattern::Constructor("None".to_string(), vec![]),
                            IRNode::Literal(IRLiteral::Bool(false)),
                        ),
                        (
                            IRPattern::Constructor(
                                "Some".to_string(),
                                vec![IRPattern::Variable("ch".to_string())],
                            ),
                            call("is_whitespace", vec![v("ch")]),
                        ),
                    ],
                }),
                body: Box::new(IRNode::Block(vec![IRNode::Assign {
                    target: Box::new(v("cur")),
                    value: Box::new(call("next_char", vec![v("cur")])),
                }])),
                invariant: None,
            },
            IRNode::Return(Box::new(v("cur"))),
        ])
    }

    /// A partial skip (at most one char) — the teeth
    /// body for the strengthened rows: correct on runs
    /// of length ≤ 1, refutable on longer runs.
    #[cfg(feature = "z3-support")]
    fn p90_partial_skip_body() -> IRNode {
        let v = |n: &str| IRNode::Var(n.to_string());
        let call = |fu: &str, args: Vec<IRNode>| IRNode::Call {
            function: fu.to_string(),
            args,
        };
        IRNode::If {
            condition: Box::new(IRNode::Match {
                scrutinee: Box::new(call("peek", vec![v("state")])),
                cases: vec![
                    (
                        IRPattern::Constructor("None".to_string(), vec![]),
                        IRNode::Literal(IRLiteral::Bool(false)),
                    ),
                    (
                        IRPattern::Constructor(
                            "Some".to_string(),
                            vec![IRPattern::Variable("ch".to_string())],
                        ),
                        call("is_whitespace", vec![v("ch")]),
                    ),
                ],
            }),
            then_branch: Box::new(IRNode::Return(Box::new(call(
                "next_char",
                vec![v("state")],
            )))),
            else_branch: Box::new(IRNode::Return(Box::new(v("state")))),
        }
    }

    /// The production pool's template seed body for a
    /// tokenize spec (the Phase 85/87 family).
    #[cfg(feature = "z3-support")]
    fn p90_template_body(module: &Spec, name: &str) -> IRNode {
        use crate::synthesis::engine::synthesize_with_module_spec;
        let spec = restored_spec(module, name);
        let cands = synthesize_with_module_spec(spec, 1, module)
            .expect("template pool synthesis");
        cands
            .iter()
            .find(|c| {
                matches!(&c.provenance, Provenance::ConstraintInduced { kind } if kind == "tokenize_template")
            })
            .unwrap_or_else(|| panic!("{}: template seed must enter the pool", name))
            .body
            .clone()
    }

    /// A candidate row parsed in the module dialect
    /// (the Phase 87 pattern).
    #[cfg(feature = "z3-support")]
    fn p90_parse_row(text: &str) -> Constraint {
        let src = format!(
            "T {{\n spec probe {{\n input: state: TokenizerState\n output: TokenizerState\n constraint: {}\n }}\n}}",
            text
        );
        let parsed = crate::spec::parser::parse(&src).expect("candidate row parses");
        parsed.functions[0].postconditions[0].clone()
    }

    // ── Phase 90 Forall machinery ──────────────────────

    /// The (c) row — the all-whitespace-between Forall —
    /// inserted into a scratch copy of the real module
    /// (the examples file is never touched), parsed and
    /// typechecked whole: the row is statable AND
    /// well-typed in the module dialect. Returns the
    /// amended module and the row itself.
    #[cfg(feature = "z3-support")]
    fn p90_module_with_forall_row() -> (Spec, Constraint) {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/functional_tokenizer.morph"
        ))
        .expect("examples tokenizer reads");
        let anchor = "    constraint: len(output.source)==len(state.source)\n";
        assert_eq!(
            src.matches(anchor).count(),
            1,
            "the surgery anchor (skip's second row) occurs exactly once"
        );
        let row_line = "    constraint: forall i: Int in range(len(output.source)): i < state.pos || i >= output.pos || is_whitespace(output.source[i])\n";
        let amended = src.replacen(anchor, &format!("{}{}", anchor, row_line), 1);
        let parsed = crate::spec::parser::parse(&amended).expect("(c)-amended module parses");
        let module = crate::spec::typeck::check(parsed)
            .expect("(c)-amended module typechecks")
            .spec;
        let skip = restored_spec(&module, "skip_whitespace");
        let row = skip
            .postconditions
            .iter()
            .find(|c| matches!(c, Constraint::Forall { .. }))
            .expect("the inserted row is a Forall")
            .clone();
        (module, row)
    }

    /// The probe's Forall encoder: the one production
    /// boundary crossed — a single Int bound variable,
    /// the body encoded through the production
    /// constraint encoder, closed with z3's
    /// forall_const. (The parser discards the `in
    /// range(...)` collection expression — parser.rs's
    /// parse_forall binds only the variables — so the
    /// bound lives in the body's own guards, exactly as
    /// the ratified formulation writes it.)
    #[cfg(feature = "z3-support")]
    fn p90_encode_forall(c: &Constraint, env: &SymEnv, ctx: &EvalCtx) -> Option<z3::ast::Bool> {
        match c {
            Constraint::Forall { vars, body } => {
                if vars.len() != 1 || vars[0].1 != TypeRef::Int {
                    return None;
                }
                let bound = z3::ast::Int::new_const(vars[0].0.clone());
                let mut env = env.clone();
                env.insert(vars[0].0.clone(), Sym::Int(bound.clone()));
                let f = encode_constraint(body, &env, ctx)?;
                let bound_dyn: &dyn z3::ast::Ast = &bound;
                Some(z3::ast::forall_const(&[bound_dyn], &[], &f))
            }
            _ => None,
        }
    }

    /// is_whitespace over the indexed char of a symbolic
    /// string (the CharStr bridge form), through the
    /// production inliner.
    #[cfg(feature = "z3-support")]
    fn p90_ws_at(
        source: &z3::ast::String,
        idx: &z3::ast::Int,
        index: &SpecIndex,
    ) -> Option<z3::ast::Bool> {
        let ch = sym_index(Sym::Str(source.clone()), Sym::Int(idx.clone()))?;
        match sym_invoke(&EvalCtx::root(index), "is_whitespace", vec![ch])? {
            Sym::Bool(b) => Some(b),
            _ => None,
        }
    }

    /// The bounded expansion of (c): the conjunction
    /// over offsets 0..k of (offset out of range OR the
    /// char there is whitespace). Exact whenever the
    /// skipped range is at most k long — the same fuel
    /// bound as the body's unrolling.
    #[cfg(feature = "z3-support")]
    fn p90_forall_bounded(
        state: &Sym,
        output: &Sym,
        k: usize,
        index: &SpecIndex,
    ) -> Option<z3::ast::Bool> {
        let src = match p90_field(output, "source")? {
            Sym::Str(s) => s.clone(),
            _ => return None,
        };
        let opos = match p90_field(output, "pos")? {
            Sym::Int(i) => i.clone(),
            _ => return None,
        };
        let spos = match p90_field(state, "pos")? {
            Sym::Int(i) => i.clone(),
            _ => return None,
        };
        let mut acc = p90_true();
        for j in 0..k {
            let idx = z3::ast::Int::add(&[&spos, &z3::ast::Int::from_i64(j as i64)]);
            let ws = p90_ws_at(&src, &idx, index)?;
            let within = idx.lt(&opos);
            acc = p90_and(&acc, &p90_or(&within.not(), &ws));
        }
        Some(acc)
    }
    #[cfg(feature = "z3-support")]
    fn p90_token_sym(t: &P90Tok) -> Sym {
        let int = |n: i64| Sym::Int(z3::ast::Int::from_i64(n));
        Sym::Record {
            type_name: "Token".into(),
            fields: vec![
                ("kind".into(), int(t.kind)),
                (
                    "lexeme".into(),
                    Sym::Str(z3::ast::String::from_str(&t.lexeme).unwrap()),
                ),
                ("line".into(), int(t.line)),
                ("col".into(), int(t.col)),
            ],
        }
    }

    /// Phase 90, Part 1 (points) — the mirror-faithful
    /// skip body (a While over peek/next_char) against
    /// skip_whitespace's current rows and the Phase 87
    /// strengthening (a), at the 12-point battery: the
    /// minimal resolving fuel equals the mirror's trip
    /// count at every point; the rows judge Proven at
    /// that fuel; (a) judges Proven except at the two
    /// EOF outputs (the standing literal-OOB artifact);
    /// and the teeth bodies (identity, partial skip)
    /// stand and fall exactly where Phase 87's table
    /// says they must.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase90_while_skip_points() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let body = p90_hand_skip_body();
        let mut denotations = std::collections::HashMap::new();
        denotations.insert(
            "next_char".to_string(),
            p90_seed_body(&module, restored_spec(&module, "next_char"), "record_completion"),
        );
        let battery: Vec<P90St> = vec![
            p90_st("", 0, 1, 1),
            p90_st(" ", 0, 1, 1),
            p90_st("  \t\n x", 0, 1, 1),
            p90_st("no-ws", 0, 1, 1),
            p90_st("\n\nab", 0, 1, 1),
            p90_st(" \u{A0}x", 0, 1, 1),
            p90_st("a b", 0, 1, 1),
            p90_st("\r\n\t", 0, 1, 1),
            p90_st("a b", 1, 1, 2),
            p90_st("  \t\n x", 2, 1, 3),
            p90_st("\n\nab", 1, 1, 2),
            p90_st(" \u{A0}x", 1, 1, 2),
        ];
        let row_a = p90_parse_row(
            "output.pos >= len(output.source) || is_whitespace(output.source[output.pos]) == false",
        );
        let mut spec_a = skip.clone();
        spec_a.postconditions.push(row_a.clone());
        let run_at = |b: &IRNode, input: &P90St, fuel: usize| {
            let ctx = P90Ctx {
                index: &index,
                denotations: &denotations,
                fuel,
                depth: 0,
                list_append: true,
            };
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            p90_run(b, env, &ctx)
        };
        for (i, input) in battery.iter().enumerate() {
            let (mirror, trips) = p90_mirror_skip(input);
            let expected = p90_state_sym(&mirror);
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            let mut resolved = false;
            for fuel in 0..=(trips + 2) {
                let outcome = run_at(&body, input, fuel)
                    .expect("the hand skip body evaluates in statement mode");
                match p90_concretize(&outcome) {
                    None => assert!(
                        fuel < trips,
                        "point {} ({:?} from {}): fuel {} must resolve — \
                         only fuels below the trip count ({}) may exhaust",
                        i, input.source, input.pos, fuel, trips
                    ),
                    Some(out) => {
                        assert!(
                            fuel >= trips,
                            "point {}: resolution begins at the trip count ({}) \
                             and persists; fuel {} resolved early",
                            i, trips, fuel
                        );
                        if fuel > trips {
                            continue;
                        }
                        assert!(
                            p90_values_equal(&out, &expected),
                            "point {} ({:?} from {}): the evaluated output must \
                             equal the mirror's",
                            i, input.source, input.pos
                        );
                        // Rows are judged at the minimal
                        // resolving fuel (= the trip count).
                        // The current rows: Proven. Row
                        // (a): Proven except the two EOF
                        // outputs (the literal-OOB
                        // artifact, Phase 87).
                        for (j, row) in skip.postconditions.iter().enumerate() {
                            let (res, tri) = verifier.verify_constraint(
                                row, "p90@skip-current", &skip, &env, &Some(out.clone()),
                            );
                            assert!(
                                matches!(tri, Tri::Proven),
                                "point {} current row {} must be Proven, got solver {} ms",
                                i, j, res.solver_time_ms
                            );
                        }
                        let (res, tri) = verifier.verify_constraint(
                            &row_a, "p90@skip-a", &spec_a, &env, &Some(out.clone()),
                        );
                        assert!(
                            matches!(tri, Tri::Proven),
                            "point {} row (a) must be Proven for the mirror \
                             body, got solver {} ms",
                            i, res.solver_time_ms
                        );
                        println!(
                            "P90|skip-point|{}|src={:?}|from={}|trips={}|min_fuel={}|row_a_ms={}",
                            i, input.source, input.pos, trips, fuel, res.solver_time_ms
                        );
                        resolved = true;
                    }
                }
            }
            assert!(
                resolved,
                "point {} ({:?} from {}): the body must resolve at the trip count",
                i, input.source, input.pos
            );
        }
        // Teeth: the identity body (the standing
        // Verified winner's shape) satisfies the current
        // rows and is REFUTED by (a) at a whitespace
        // start; the partial skip (≤ 1 char) is refuted
        // by (a) on a longer run and stands where it is
        // correct.
        let identity = IRNode::Return(Box::new(IRNode::Var("state".to_string())));
        let ws_start = p90_st("  \t\n x", 0, 1, 1);
        let mut env = SymEnv::new();
        env.insert("state".into(), p90_state_sym(&ws_start));
        let out = p90_concretize(
            &run_at(&identity, &ws_start, 0).expect("identity evaluates"),
        )
        .expect("identity resolves at fuel 0");
        for row in skip.postconditions.iter() {
            let (_, tri) =
                verifier.verify_constraint(row, "p90@identity", &skip, &env, &Some(out.clone()));
            assert!(
                matches!(tri, Tri::Proven),
                "the identity body satisfies the current rows (the standing scandal)"
            );
        }
        let (_, tri) =
            verifier.verify_constraint(&row_a, "p90@identity-a", &spec_a, &env, &Some(out));
        assert!(
            matches!(tri, Tri::Refuted),
            "row (a) REFUTES the identity body at a whitespace start"
        );
        let partial = p90_partial_skip_body();
        let out = p90_concretize(
            &run_at(&partial, &ws_start, 0).expect("partial skip evaluates"),
        )
        .expect("partial skip resolves at fuel 0");
        let (_, tri) =
            verifier.verify_constraint(&row_a, "p90@partial-a", &spec_a, &env, &Some(out));
        assert!(
            matches!(tri, Tri::Refuted),
            "row (a) REFUTES the partial skip on a run of 5"
        );
        let one = p90_st(" ", 0, 1, 1);
        let mut env1 = SymEnv::new();
        env1.insert("state".into(), p90_state_sym(&one));
        let out = p90_concretize(&run_at(&partial, &one, 0).expect("partial evaluates"))
            .expect("partial resolves");
        let (_, tri) =
            verifier.verify_constraint(&row_a, "p90@partial-a-ok", &spec_a, &env1, &Some(out));
        assert!(
            matches!(tri, Tri::Proven),
            "the partial skip is exactly right on a run of 1: (a) admits it there"
        );
        println!("P90|skip-teeth|identity: current Proven, (a) Refuted; partial: (a) Refuted on run 5, Proven on run 1");
    }

    /// Phase 90, Part 1 (points) — the production
    /// template STEP body for tokenize_one, with the
    /// hand skip body supplied through the probe
    /// denotation map (the resolver extension's shape),
    /// at the 10-point battery plus the diagnostic
    /// whitespace-then-bad-char point: the minimal
    /// resolving fuel is the max of the skip and ident
    /// trip counts; the outputs equal the mirror's; the
    /// amended rows judge Proven at every battery point;
    /// and at " 5" row 2 REFUTES the faithful body — the
    /// measured proof that the split's remaining half
    /// is a row problem (the R2-β-skip ratification),
    /// not an encoder problem. The β-skip license,
    /// built over the denoted skip, admits that same
    /// Err and keeps its teeth.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase90_while_step_points() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let spec = restored_spec(&module, "tokenize_one").clone();
        let body = p90_template_body(&module, "tokenize_one");
        let mut denotations = std::collections::HashMap::new();
        denotations.insert("skip_whitespace".to_string(), p90_hand_skip_body());
        denotations.insert(
            "next_char".to_string(),
            p90_seed_body(&module, restored_spec(&module, "next_char"), "record_completion"),
        );
        let run_at = |input: &P90St, fuel: usize| {
            let ctx = P90Ctx {
                index: &index,
                denotations: &denotations,
                fuel,
                depth: 0,
                list_append: true,
            };
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            p90_run(&body, env, &ctx)
        };
        let battery: Vec<P90St> = vec![
            p90_st("spec", 0, 1, 1),
            p90_st("", 0, 1, 1),
            p90_st("  spec", 0, 1, 1),
            p90_st(":", 0, 1, 1),
            p90_st("{", 0, 1, 1),
            p90_st("=", 0, 1, 1),
            p90_st("5", 0, 1, 1),
            p90_st(" ", 0, 1, 1),
            p90_st("a b", 0, 1, 1),
            p90_st("\n\nx", 0, 1, 1),
        ];
        for (i, input) in battery.iter().enumerate() {
            let (mirror, sk, id) = p90_mirror_one(input);
            // Minimal resolving fuel: the skip While
            // needs sk; the ident Loop, when taken, runs
            // id + 1 iterations (the last breaks).
            let pred = if id >= 1 { sk.max(id + 1) } else { sk };
            if pred >= 1 {
                let outcome = run_at(input, pred - 1)
                    .expect("the template STEP body evaluates in statement mode");
                assert!(
                    p90_concretize(&outcome).is_none(),
                    "point {} ({:?}): fuel {} (one below the predicted {}) must exhaust",
                    i, input.source, pred - 1, pred
                );
            }
            let outcome = run_at(input, pred)
                .expect("the template STEP body evaluates in statement mode");
            let out = p90_concretize(&outcome).unwrap_or_else(|| {
                panic!(
                    "point {} ({:?}): fuel {} (the predicted max({}, {})) must resolve",
                    i, input.source, pred, sk, id
                )
            });
            match (&mirror, &out) {
                (Ok(_), Sym::Result { ok: Tag::True, .. }) => {
                    assert!(
                        p90_values_equal(&out, &p90_result_sym(&mirror)),
                        "point {} ({:?}): the STEP output must equal the mirror's",
                        i, input.source
                    );
                }
                (Err(_), Sym::Result { ok: Tag::False, .. }) => {
                    // The tag agrees; the message text is
                    // the named F2 deviation (the mirror
                    // interpolates, the template's is
                    // constant) — not judged here.
                }
                _ => panic!(
                    "point {} ({:?}): tag disagreement with the mirror",
                    i, input.source
                ),
            }
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            let mut ms = 0u64;
            let is_err = matches!(&mirror, Err(_));
            for (j, row) in spec.postconditions.iter().enumerate() {
                let (res, tri) = verifier.verify_constraint(
                    row, "p90@step", &spec, &env, &Some(out.clone()),
                );
                ms += res.solver_time_ms;
                if is_err && j == 0 {
                    // Row 1 at a literal Err output is
                    // UNKNOWN, not Proven: its dead
                    // `output.unwrap()` disjuncts are
                    // unencodable under a literal Err tag
                    // (the Phase 76 unwrap boundary) — an
                    // encoding limit at concrete points,
                    // not a body fault. Symbolically (tag
                    // Free) the row encodes fine.
                    assert!(
                        matches!(tri, Tri::Unknown),
                        "point {} ({:?}) row 0 at an Err output must judge \
                         Unknown (literal-Err unwrap boundary), got solver {} ms",
                        i, input.source, res.solver_time_ms
                    );
                    continue;
                }
                assert!(
                    matches!(tri, Tri::Proven),
                    "point {} ({:?}) row {} must be Proven for the faithful \
                     template body, got solver {} ms",
                    i, input.source, j, res.solver_time_ms
                );
            }
            println!(
                "P90|step-point|{}|src={:?}|skip_trips={}|ident_trips={}|min_fuel={}|rows_ms={}",
                i, input.source, sk, id, pred, ms
            );
        }
        // The diagnostic point: " 5" — whitespace, then a
        // bad char. The faithful body errs; row 1 admits
        // (is_err), row 2 REFUTES (its license reads the
        // pre-skip char, a space).
        let diag = p90_st(" 5", 0, 1, 1);
        let outcome = run_at(&diag, 2).expect("STEP evaluates at the diagnostic point");
        let out = p90_concretize(&outcome).expect("the diagnostic point resolves");
        assert!(
            matches!(&out, Sym::Result { ok: Tag::False, .. }),
            "the faithful body errs at \" 5\""
        );
        let mut env = SymEnv::new();
        env.insert("state".into(), p90_state_sym(&diag));
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[0], "p90@diag-row1", &spec, &env, &Some(out.clone()),
        );
        assert!(
            matches!(tri, Tri::Unknown),
            "row 1 at a literal Err output judges Unknown (the Phase 76 \
             unwrap boundary); its is_err disjunct is what admits the Err \
             symbolically"
        );
        let (_, tri) = verifier.verify_constraint(
            &spec.postconditions[1], "p90@diag-row2", &spec, &env, &Some(out.clone()),
        );
        assert!(
            matches!(tri, Tri::Refuted),
            "row 2 REFUTES the faithful body at \" 5\": no encoder can verify \
             this body against the standing rows — the residue is the \
             R2-β-skip ratification, not an encoder gap"
        );
        // The β-skip license over the DENOTED skip: the
        // skipped state's char decides. Built from the
        // production machinery over the probe's denoted
        // skip output.
        let beta_license = |input: &P90St, output: &Sym| -> z3::ast::Bool {
            let ctx = P90Ctx {
                index: &index,
                denotations: &denotations,
                fuel: 4,
                depth: 0,
                list_append: true,
            };
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            let skipped = p90_concretize(
                &p90_run(&p90_hand_skip_body(), env, &ctx).expect("denoted skip evaluates"),
            )
            .expect("denoted skip resolves");
            let src = match p90_field(&skipped, "source") {
                Some(Sym::Str(s)) => s.clone(),
                _ => panic!("skipped source"),
            };
            let pos = match p90_field(&skipped, "pos") {
                Some(Sym::Int(i)) => i.clone(),
                _ => panic!("skipped pos"),
            };
            let ch = sym_index(Sym::Str(src.clone()), Sym::Int(pos.clone()))
                .expect("the skipped char takes the bridge form");
            let call_bool = |name: &str| -> z3::ast::Bool {
                match sym_invoke(&EvalCtx::root(&index), name, vec![ch.clone()]) {
                    Some(Sym::Bool(b)) => b,
                    _ => panic!("classifier {} inlines over the skipped char", name),
                }
            };
            let mut nt = p90_and(
                &call_bool("is_ident_start").not(),
                &call_bool("is_whitespace").not(),
            );
            for p in [':', '{', '}', '(', ')', ',', '<', '>', '='] {
                let lit = Sym::Char(z3::ast::Int::from_i64(p as u32 as i64));
                let neq = match sym_binop(BinKind::Neq, ch.clone(), lit) {
                    Some(Sym::Bool(b)) => b,
                    _ => panic!("the skipped char disequality encodes"),
                };
                nt = p90_and(&nt, &neq);
            }
            let eof = pos.ge(&src.length());
            let is_ok = match output {
                Sym::Result { ok, .. } => ok.term(),
                _ => panic!("step output is a Result"),
            };
            p90_or(&is_ok, &p90_or(&eof, &nt))
        };
        let f = beta_license(&diag, &out);
        assert_eq!(
            p90_sat_all(&[f.not()]).0,
            Some(false),
            "the β-skip license ADMITS the faithful Err at \" 5\""
        );
        // Teeth: a fabricated Err at an identifier start
        // stays refuted under the β license.
        let spec_pt = p90_st("spec", 0, 1, 1);
        let fake_err = p90_result_sym(&Err("fabricated".to_string()));
        let f2 = beta_license(&spec_pt, &fake_err);
        assert_eq!(
            p90_sat_all(&[f2.not()]).0,
            Some(true),
            "the β-skip license REFUTES a fabricated Err at an identifier start"
        );
        println!("P90|step-diag|\" 5\": row 1 Unknown (unwrap boundary), row 2 Refuted; β-skip license admits the Err and keeps its teeth");
    }
    /// Phase 90, Part 1 (symbolic) — the hand skip body
    /// against skip_whitespace's rows over a FREE state:
    /// bounded unrolling is decisive at points but
    /// cannot Verify symbolically at any fuel — a
    /// whitespace run longer than the fuel is always a
    /// live exhaustion path, so the BMC discipline
    /// judges Inconclusive (the probe's Unknown) at
    /// every depth, for the current rows and for the
    /// strengthened combinations alike. This is the
    /// measurement that prices the invariant strategy:
    /// skip's symbolic Verified can only come from a
    /// loop invariant, never from unrolling depth.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase90_while_skip_symbolic() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let skip = restored_spec(&module, "skip_whitespace").clone();
        let body = p90_hand_skip_body();
        let mut denotations = std::collections::HashMap::new();
        denotations.insert(
            "next_char".to_string(),
            p90_seed_body(&module, restored_spec(&module, "next_char"), "record_completion"),
        );
        let env = param_env(&skip, &index);
        let row_a = p90_parse_row(
            "output.pos >= len(output.source) || is_whitespace(output.source[output.pos]) == false",
        );
        for fuel in [1usize, 2, 4] {
            let ctx = P90Ctx {
                index: &index,
                denotations: &denotations,
                fuel,
                depth: 0,
                list_append: true,
            };
            let outcome = p90_run(&body, env.clone(), &ctx)
                .expect("the hand skip body evaluates symbolically");
            for (j, row) in skip.postconditions.iter().enumerate() {
                let (tri, ms) = p90_judge_row(row, &skip, &env, &outcome, &index);
                println!(
                    "P90|skip-sym|fuel={}|row={}|verdict={}|judge_ms={}",
                    fuel,
                    j,
                    p90_tri_name(tri),
                    ms
                );
                assert_eq!(
                    tri,
                    P90Tri::Unknown,
                    "fuel {}: bounded unrolling must NOT Verify the skip \
                     symbolically (exhaustion is always reachable), row {}",
                    fuel, j
                );
            }
        }
        // The strengthened combinations at fuel 4: (a)
        // alone, (a) + the true Forall (c) through the
        // probe encoder, (a) + the bounded expansion of
        // (c) at the same fuel. All Unknown — the
        // exhaustion discipline binds every form.
        let (_, row_c) = p90_module_with_forall_row();
        let fuel = 4usize;
        let ctx = P90Ctx {
            index: &index,
            denotations: &denotations,
            fuel,
            depth: 0,
            list_append: true,
        };
        let outcome = p90_run(&body, env.clone(), &ctx).expect("skip evaluates");
        let ectx = EvalCtx::root(&index);
        let (tri, ms) = p90_judge_with(&skip, &env, &outcome, |full| {
            encode_constraint(&row_a, full, &ectx)
        });
        println!("P90|skip-sym|fuel=4|rows=current+(a)|verdict={}|judge_ms={}", p90_tri_name(tri), ms);
        assert_eq!(tri, P90Tri::Unknown);
        let (tri, ms) = p90_judge_with(&skip, &env, &outcome, |full| {
            let a = encode_constraint(&row_a, full, &ectx)?;
            let c = p90_encode_forall(&row_c, full, &ectx)?;
            Some(p90_and(&a, &c))
        });
        println!("P90|skip-sym|fuel=4|rows=current+(a)+(c)|verdict={}|judge_ms={}", p90_tri_name(tri), ms);
        assert_eq!(tri, P90Tri::Unknown);
        let (tri, ms) = p90_judge_with(&skip, &env, &outcome, |full| {
            let a = encode_constraint(&row_a, full, &ectx)?;
            let state = full.get("state")?.clone();
            let output = full.get("output")?.clone();
            let c = p90_forall_bounded(&state, &output, fuel, &index)?;
            Some(p90_and(&a, &c))
        });
        println!("P90|skip-sym|fuel=4|rows=current+(a)+bounded(c)|verdict={}|judge_ms={}", p90_tri_name(tri), ms);
        assert_eq!(tri, P90Tri::Unknown);
    }

    /// Phase 90, Part 1 (symbolic) — the template STEP
    /// body over a free state. The ablations come first
    /// (structural, solver-free): without the denotation
    /// map the body does not evaluate at all (the
    /// resolver wall, in situ), and without the
    /// list-append form it dies at the first Append over
    /// the length-only list (the wall behind the wall).
    /// With both, the body evaluates at fuels 2 and 4
    /// and neither row Verifies: row 1 is bounded by
    /// exhaustion (an identifier longer than the fuel),
    /// and row 2 is bounded by the " 5" countermodel
    /// the point probe refutes outright — the judge
    /// reports whichever of Refuted/Unknown the solver
    /// reaches, never Proven.
    /// Phase 90, Part 1 (symbolic) — the template STEP
    /// body over a fully symbolic state, with the
    /// resolver extensions supplied as probe
    /// denotations. Ablations first: no skip denotation
    /// → the body does not evaluate at all; no
    /// list-append form → it dies at the first Append.
    /// With both, it evaluates (13 paths) and judges:
    /// row 1 Unknown (exhaustion-bounded), row 2
    /// REFUTED — the " 5" countermodel exists
    /// symbolically, so no unrolling depth can Verify
    /// this body against the standing rows.
    ///
    /// #[ignore]: a measurement instrument, not a
    /// regression test — a full run takes ~36 s
    /// (solver-guided decisions of 0.9–5 s inside the
    /// evaluation). Run with
    /// `cargo test phase90_while_step_symbolic -- --ignored`.
    /// Measured output is recorded in
    /// ENCODER_EXTENSION_DESIGN.md.
    #[cfg(feature = "z3-support")]
    #[test]
    #[ignore]
    fn phase90_while_step_symbolic() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let spec = restored_spec(&module, "tokenize_one").clone();
        let body = p90_template_body(&module, "tokenize_one");
        let env = param_env(&spec, &index);
        // Ablation 1: no denotations — the skip call is
        // unresolvable and the body does not evaluate.
        let empty = std::collections::HashMap::new();
        let ctx = P90Ctx {
            index: &index,
            denotations: &empty,
            fuel: 4,
            depth: 0,
            list_append: true,
        };
        assert!(
            p90_run(&body, env.clone(), &ctx).is_none(),
            "without a skip denotation the STEP body does not evaluate \
             (the resolver wall, measured in situ)"
        );
        let mut denotations = std::collections::HashMap::new();
        denotations.insert("skip_whitespace".to_string(), p90_hand_skip_body());
        denotations.insert(
            "next_char".to_string(),
            p90_seed_body(&module, restored_spec(&module, "next_char"), "record_completion"),
        );
        // Ablation 2: no list-append form — the first
        // Append over the length-only tokens list kills
        // the evaluation.
        let ctx = P90Ctx {
            index: &index,
            denotations: &denotations,
            fuel: 4,
            depth: 0,
            list_append: false,
        };
        assert!(
            p90_run(&body, env.clone(), &ctx).is_none(),
            "without Append over the length-only list form the STEP body \
             does not evaluate (the list wall behind the statement wall)"
        );
        // The full prototype at fuels 1 and 2. (Fuel 4
        // was measured once during development: the
        // symbolic evaluation + judging ran past six
        // minutes without finishing — the merging-mode
        // cost wall, recorded in the design doc. The
        // verdicts asserted here are fuel-independent:
        // exhaustion is reachable at every depth.)
        for fuel in [1usize, 2] {
            let ctx = P90Ctx {
                index: &index,
                denotations: &denotations,
                fuel,
                depth: 0,
                list_append: true,
            };
            let outcome = p90_run(&body, env.clone(), &ctx)
                .expect("the STEP body evaluates with denotations + list-append");
            for (j, row) in spec.postconditions.iter().enumerate() {
                let (tri, ms) = p90_judge_row(row, &spec, &env, &outcome, &index);
                println!(
                    "P90|step-sym|fuel={}|row={}|verdict={}|judge_ms={}|paths={}",
                    fuel,
                    j,
                    p90_tri_name(tri),
                    ms,
                    outcome.paths.len()
                );
                assert_ne!(
                    tri,
                    P90Tri::Proven,
                    "fuel {} row {} must not Verify: row 1 is exhaustion-bounded \
                     and row 2 carries the \" 5\" countermodel",
                    fuel, j
                );
            }
        }
    }

    /// Phase 90, Part 1 (points) — the template LOOP
    /// body for tokenize_all, with both denotations
    /// supplied (the hand skip, and the STEP body itself
    /// as tokenize_one's denotation — the resolver
    /// extension's second half): the minimal resolving
    /// fuel is the max of the outer iteration count and
    /// the inner trip counts; the outputs equal the
    /// mirror's token lists; the spec's one row judges
    /// Proven at every point.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase90_while_loop_points() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let verifier = Verifier::with_module(&module);
        let spec = restored_spec(&module, "tokenize_all").clone();
        let body = p90_template_body(&module, "tokenize_all");
        let mut denotations = std::collections::HashMap::new();
        denotations.insert("skip_whitespace".to_string(), p90_hand_skip_body());
        denotations.insert(
            "next_char".to_string(),
            p90_seed_body(&module, restored_spec(&module, "next_char"), "record_completion"),
        );
        denotations.insert(
            "tokenize_one".to_string(),
            p90_template_body(&module, "tokenize_one"),
        );
        let run_at = |input: &P90St, fuel: usize| {
            let ctx = P90Ctx {
                index: &index,
                denotations: &denotations,
                fuel,
                depth: 0,
                list_append: true,
            };
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(input));
            p90_run(&body, env, &ctx)
        };
        for src in ["", "spec sort", "spec{input:()}", "spec 5", "  \n ", "a", "x{y}:(,)", "a\nb"] {
            let input = p90_st(src, 0, 1, 1);
            let (mirror, outer, inner) = p90_mirror_all(&input);
            let pred = outer.max(inner);
            if pred >= 1 {
                let outcome = run_at(&input, pred - 1)
                    .expect("the template LOOP body evaluates in statement mode");
                assert!(
                    p90_concretize(&outcome).is_none(),
                    "{:?}: fuel {} (one below the predicted {}) must exhaust",
                    src, pred - 1, pred
                );
            }
            let outcome = run_at(&input, pred)
                .expect("the template LOOP body evaluates in statement mode");
            let out = p90_concretize(&outcome).unwrap_or_else(|| {
                panic!("{:?}: fuel {} (the predicted) must resolve", src, pred)
            });
            match (&mirror, &out) {
                (Ok(toks), Sym::Result { ok: Tag::True, ok_payload, .. }) => {
                    let expected = Sym::List(toks.iter().map(p90_token_sym).collect());
                    let payload = ok_payload.as_ref().expect("Ok payload");
                    assert!(
                        p90_values_equal(payload, &expected),
                        "{:?}: the LOOP output token list must equal the mirror's",
                        src
                    );
                }
                (Err(_), Sym::Result { ok: Tag::False, .. }) => {
                    // Tag agrees; the message text is the
                    // F2 deviation, not judged here.
                }
                _ => panic!("{:?}: tag disagreement with the mirror", src),
            }
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(&input));
            let (res, tri) = verifier.verify_constraint(
                &spec.postconditions[0], "p90@loop", &spec, &env, &Some(out),
            );
            if matches!(&mirror, Err(_)) {
                // At a literal Err output the row's
                // `output.unwrap()` is unencodable (the
                // Phase 76 unwrap boundary): Unknown, an
                // encoding limit at concrete points, not
                // a body fault.
                assert!(
                    matches!(tri, Tri::Unknown),
                    "{:?}: tokenize_all's row at an Err output must judge \
                     Unknown (literal-Err unwrap boundary), got solver {} ms",
                    src, res.solver_time_ms
                );
            } else {
                assert!(
                    matches!(tri, Tri::Proven),
                    "{:?}: tokenize_all's row must be Proven for the faithful \
                     LOOP body, got solver {} ms",
                    src, res.solver_time_ms
                );
            }
            println!(
                "P90|loop-point|src={:?}|outer_iters={}|inner_max={}|min_fuel={}|row_ms={}",
                src, outer, inner, pred, res.solver_time_ms
            );
        }
    }

    /// Phase 90, Part 1 (symbolic) — the LOOP body over
    /// a free state: without the step denotation the
    /// body does not evaluate at all (the TryPropagate's
    /// call is unresolvable — measured); with it, the
    /// body evaluates at fuels 1 and 2 and the row does
    /// not Verify (exhaustion: more tokens than the
    /// fuel). The nested blow-up is reported as judge
    /// time and formula size per fuel.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase90_while_loop_symbolic() {
        let module = phase84_real_module();
        let index = SpecIndex::from_module(&module);
        let spec = restored_spec(&module, "tokenize_all").clone();
        let body = p90_template_body(&module, "tokenize_all");
        let env = param_env(&spec, &index);
        // Ablation: skip denoted, the step not — the
        // TryPropagate call cannot resolve.
        let mut partial = std::collections::HashMap::new();
        partial.insert("skip_whitespace".to_string(), p90_hand_skip_body());
        partial.insert(
            "next_char".to_string(),
            p90_seed_body(&module, restored_spec(&module, "next_char"), "record_completion"),
        );
        let ctx = P90Ctx {
            index: &index,
            denotations: &partial,
            fuel: 2,
            depth: 0,
            list_append: true,
        };
        assert!(
            p90_run(&body, env.clone(), &ctx).is_none(),
            "without a tokenize_one denotation the LOOP body does not \
             evaluate (the step-denotation wall, measured in situ)"
        );
        let mut denotations = partial.clone();
        denotations.insert(
            "tokenize_one".to_string(),
            p90_template_body(&module, "tokenize_one"),
        );
        // Fuel 1 only: it already yields the verdict
        // (Unknown — exhaustion is a live path), and a
        // fuel-2 run was measured past the practical
        // wall during development (guided decisions of
        // 0.6–3.2 s each inside the nested denotations,
        // then merging; >280 s without finishing). The
        // fuel-1 row formula's tree print measured
        // 921,091 chars — recorded in the design doc as
        // the formula-size datum; not recomputed here.
        for fuel in [1usize] {
            let ctx = P90Ctx {
                index: &index,
                denotations: &denotations,
                fuel,
                depth: 0,
                list_append: true,
            };
            let outcome = p90_run(&body, env.clone(), &ctx)
                .expect("the LOOP body evaluates with both denotations");
            let (tri, ms) =
                p90_judge_row(&spec.postconditions[0], &spec, &env, &outcome, &index);
            println!(
                "P90|loop-sym|fuel={}|verdict={}|judge_ms={}|paths={}",
                fuel,
                p90_tri_name(tri),
                ms,
                outcome.paths.len()
            );
            assert_ne!(
                tri,
                P90Tri::Proven,
                "fuel {}: bounded unrolling must not Verify tokenize_all \
                 symbolically (more tokens than the fuel is a live path)",
                fuel
            );
        }
    }

    /// Phase 90, Part 2 — the Forall encoding. The (c)
    /// row is statable and well-typed in the real module
    /// (scratch surgery; the file is untouched) and —
    /// through Phase 93 — still unencodable through
    /// the production encoder (the pin below flipped
    /// when Phase 94 removed that boundary). Through
    /// the probe's forall_const
    /// encoder it judges the mirror's outputs at the 12
    /// skip points, refutes the overshoot outputs (its
    /// teeth), and — by design — admits the identity and
    /// partial outputs (the division of labor with row
    /// (a): (c) pins what was crossed, (a) pins where
    /// the landing is). The bounded expansion decides
    /// the same points. The symbolic consecution step —
    /// the invariant proof obligation for the skip loop
    /// — is measured with and without a quantifier
    /// pattern.
    #[cfg(feature = "z3-support")]
    #[test]
    fn phase90_forall_encoding_measurement() {
        let (module_c, row_c) = p90_module_with_forall_row();
        let index = SpecIndex::from_module(&module_c);
        let ectx = EvalCtx::root(&index);
        // The production boundary, re-pinned on the
        // typechecked row. Through Phase 93 this
        // asserted None (the boundary this probe
        // measured against); Phase 94's forall_const
        // arm removes it, so the pin flips to Some in
        // place. The judging below still runs through
        // the probe encoder — this test stands as the
        // measurement record; Phase 94's production
        // battery re-runs the same points through
        // production `encode_constraint`.
        let mut env0 = SymEnv::new();
        env0.insert("state".into(), p90_state_sym(&p90_st(" x", 0, 1, 1)));
        env0.insert("output".into(), p90_state_sym(&p90_st(" x", 2, 1, 3)));
        assert!(
            encode_constraint(&row_c, &env0, &ectx).is_some(),
            "Phase 94: the production encoder ENCODES the Forall \
             (the boundary recorded here through Phase 93 is removed)"
        );
        let judge_forall = |state: &P90St, output: &P90St| -> (P90Tri, u64) {
            let mut env = SymEnv::new();
            env.insert("state".into(), p90_state_sym(state));
            env.insert("output".into(), p90_state_sym(output));
            let f = p90_encode_forall(&row_c, &env, &ectx)
                .expect("the probe encoder encodes the Forall at a point");
            match p90_sat_all(&[f.not()]) {
                (Some(false), ms) => (P90Tri::Proven, ms),
                (Some(true), ms) => (P90Tri::Refuted, ms),
                (None, ms) => (P90Tri::Unknown, ms),
            }
        };
        let judge_bounded = |state: &P90St, output: &P90St| -> (P90Tri, u64) {
            let f = p90_forall_bounded(
                &p90_state_sym(state),
                &p90_state_sym(output),
                8,
                &index,
            )
            .expect("the bounded expansion encodes at a point");
            match p90_sat_all(&[f.not()]) {
                (Some(false), ms) => (P90Tri::Proven, ms),
                (Some(true), ms) => (P90Tri::Refuted, ms),
                (None, ms) => (P90Tri::Unknown, ms),
            }
        };
        let battery: Vec<P90St> = vec![
            p90_st("", 0, 1, 1),
            p90_st(" ", 0, 1, 1),
            p90_st("  \t\n x", 0, 1, 1),
            p90_st("no-ws", 0, 1, 1),
            p90_st("\n\nab", 0, 1, 1),
            p90_st(" \u{A0}x", 0, 1, 1),
            p90_st("a b", 0, 1, 1),
            p90_st("\r\n\t", 0, 1, 1),
            p90_st("a b", 1, 1, 2),
            p90_st("  \t\n x", 2, 1, 3),
            p90_st("\n\nab", 1, 1, 2),
            p90_st(" \u{A0}x", 1, 1, 2),
        ];
        let mut proven = 0usize;
        for (i, input) in battery.iter().enumerate() {
            let (mirror, _) = p90_mirror_skip(input);
            let (tf, msf) = judge_forall(input, &mirror);
            let (tb, msb) = judge_bounded(input, &mirror);
            assert_ne!(
                tf,
                P90Tri::Refuted,
                "point {}: the Forall must not refute the mirror's output",
                i
            );
            assert_eq!(
                tb,
                P90Tri::Proven,
                "point {}: the bounded expansion must prove the mirror's output",
                i
            );
            if tf == P90Tri::Proven {
                proven += 1;
            }
            println!(
                "P90|forall-point|{}|src={:?}|forall={} ({} ms)|bounded={} ({} ms)",
                i,
                input.source,
                p90_tri_name(tf),
                msf,
                p90_tri_name(tb),
                msb
            );
        }
        println!(
            "P90|forall-points|mirror Proven {}/12 through forall_const; bounded expansion 12/12",
            proven
        );
        // Teeth: the overshoot outputs — a word jumped —
        // are REFUTED by (c) (the crossing includes a
        // non-whitespace char). This is the freedom
        // (a)+(b) left open in Phase 87.
        for (src, out_pos) in [("a b", 2i64), ("ab cd", 3i64)] {
            let input = p90_st(src, 0, 1, 1);
            let output = p90_st(src, out_pos, 1, out_pos + 1);
            let (tf, _) = judge_forall(&input, &output);
            assert_eq!(
                tf,
                P90Tri::Refuted,
                "the Forall REFUTES the overshoot {:?} -> {}",
                src, out_pos
            );
            let (tb, _) = judge_bounded(&input, &output);
            assert_eq!(tb, P90Tri::Refuted, "the bounded expansion refutes it too");
        }
        // The division of labor: identity and partial
        // outputs are ADMITTED by (c) (everything they
        // crossed IS whitespace — they are row (a)'s
        // business, measured in the skip point probe).
        let ws = p90_st("  \t\n x", 0, 1, 1);
        let identity_out = ws.clone();
        let (tf, _) = judge_forall(&ws, &identity_out);
        assert_eq!(
            tf,
            P90Tri::Proven,
            "(c) admits the identity output (the empty range is vacuous)"
        );
        let mut partial_out = ws.clone();
        partial_out.pos = 1;
        partial_out.col = 2;
        let (tf, _) = judge_forall(&ws, &partial_out);
        assert_eq!(
            tf,
            P90Tri::Proven,
            "(c) admits the partial skip (its crossing is all whitespace)"
        );
        // The invariant consecution step for the skip
        // loop, symbolically: H(p0, p) = every position
        // in [p0, p) is whitespace in src. Given H(p0, p),
        // p0 <= p < len(src), and whitespace at p, prove
        // H(p0, p+1). Measured bare and with a pattern
        // on the str.at term.
        let consecution = |with_pattern: bool| -> Option<(P90Tri, u64)> {
            let src = z3::ast::String::new_const("p90c.src");
            let p0 = z3::ast::Int::new_const("p90c.p0");
            let p = z3::ast::Int::new_const("p90c.p");
            let h = |a: &z3::ast::Int, b: &z3::ast::Int| -> Option<z3::ast::Bool> {
                let i = z3::ast::Int::new_const("p90c.i");
                let ws = p90_ws_at(&src, &i, &index)?;
                let in_range = p90_and(&a.le(&i), &i.lt(b));
                let body = p90_or(&in_range.not(), &ws);
                let bound_dyn: &dyn z3::ast::Ast = &i;
                if with_pattern {
                    let at = src.at(&i);
                    let pat = z3::Pattern::new(&[&at as &dyn z3::ast::Ast]);
                    Some(z3::ast::forall_const(&[bound_dyn], &[&pat], &body))
                } else {
                    Some(z3::ast::forall_const(&[bound_dyn], &[], &body))
                }
            };
            let h_pre = h(&p0, &p)?;
            let one = z3::ast::Int::from_i64(1);
            let p_next = z3::ast::Int::add(&[&p, &one]);
            let h_post = h(&p0, &p_next)?;
            let ws_p = p90_ws_at(&src, &p, &index)?;
            let facts = vec![
                h_pre,
                p0.le(&p),
                p.lt(&src.length()),
                ws_p,
                h_post.not(),
            ];
            match p90_sat_all(&facts) {
                (Some(false), ms) => Some((P90Tri::Proven, ms)),
                (Some(true), ms) => Some((P90Tri::Refuted, ms)),
                (None, ms) => Some((P90Tri::Unknown, ms)),
            }
        };
        for with_pattern in [false, true] {
            let (tri, ms) = consecution(with_pattern)
                .expect("the consecution formulas build");
            println!(
                "P90|forall-consecution|pattern={}|verdict={}|ms={}",
                with_pattern,
                p90_tri_name(tri),
                ms
            );
            assert_ne!(
                tri,
                P90Tri::Refuted,
                "the consecution implication is true: the solver must not \
                 refute it (pattern={})",
                with_pattern
            );
        }
        println!("P90|forall-teeth|overshoot Refuted by (c) and by the bounded expansion; identity/partial admitted by (c) (row (a)'s business)");
    }
}

