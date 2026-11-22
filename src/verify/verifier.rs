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
    /// The module the verified spec lives in, for resolving
    /// spec-to-spec calls by inlining (see `sym_invoke`).
    /// Empty unless built with `with_specs`/`with_index` — a
    /// standalone verifier judges exactly what it judged
    /// before: calls it cannot resolve make a body or a
    /// constraint unencodable, hence Inconclusive.
    index: SpecIndex,
}

impl Default for Verifier {
    fn default() -> Self {
        Self {
            timeout_per_constraint_ms: 5_000,
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

        let mut any_refuted = false;
        let mut any_unknown = false;
        for (i, c) in spec.postconditions.iter().enumerate() {
            let (result, tri) = self.verify_constraint(
                c,
                &format!("post[{}]", i),
                spec,
                &param_env,
                &output,
            );
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
//   Literal, Var, BinOp, UnaryOp (Neg/Not/Abs/Len), If, Match
//   (literal/variable/wildcard patterns), Let, Block, Return,
//   Call ("len", the "List" literal, the Ok/Err/Some/None
//   constructors, and — when the verifier carries the module —
//   calls to other specs, inlined), Collection, Index, Typed,
//   FieldAccess, RecordLit (Phase 76).
//
// Anything else — holes, loops, mutation, lambdas and other
// function values, higher-order combinators, calls that do
// not resolve, guard/constructor patterns that cannot be
// decided — makes the body unencodable (`None`), never a
// guessed value.
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
/// backwards. Mirrors the interpreter: arms are tried in order,
/// constructor patterns never match a value in this domain,
/// guard patterns are undecidable. The last reachable arm
/// must be unconditional (wildcard/variable) — otherwise some
/// scrutinee values match nothing and the body has no defined
/// output, which is unencodable rather than a guess.
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
            IRPattern::Constructor(..) => continue,
            IRPattern::Guard(..) => return None,
        }
    }
    let (last_cond, last_val) = arms.pop()?;
    if last_cond.is_some() {
        return None; // the match can fall through
    }
    let mut acc = last_val;
    for (cond, val) in arms.into_iter().rev() {
        let cond = cond.expect("non-final arms are conditional");
        acc = sym_ite(&cond, val, acc)?;
    }
    Some(acc)
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
        // Head/Tail/Init/Last/Reverse/Clone are outside the
        // interpreter's evaluable subset too.
        _ => None,
    }
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
        // Quantified formulas and named predicates (is_sorted
        // and friends) have no encoding here.
        Constraint::Forall { .. }
        | Constraint::Exists { .. }
        | Constraint::Predicate(..) => None,
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
        // named. The resolver denotes no body for
        // skip_whitespace (peek denotes, since Phase 81 —
        // asserted here so the contrast is pinned, not
        // anecdotal).
        assert!(
            index.callee_body("skip_whitespace").is_none(),
            "no denotation exists for skip_whitespace: nothing to inline"
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
}

