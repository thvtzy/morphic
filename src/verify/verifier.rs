// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC VERIFIER (v0.2)                                 │
// │  Z3-based formal verification of candidate implementations│
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::*;
use crate::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, IRPattern, IRUnaryOp,
    ScoreBreakdown, SpecIndex, composite_score, MAX_INLINE_DEPTH,
};
use super::smt::{Z3Session, SmtResult, SmtValue, z3_add, z3_sub, z3_mul, z3_and};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Instant;

// ── Public Types ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub candidate_id: u64,
    pub status: VerificationStatus,
    pub constraint_results: Vec<ConstraintResult>,
    pub counterexample: Option<Counterexample>,
    pub verification_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationStatus { Verified, VerificationFailed, Inconclusive, Malformed }

#[derive(Debug, Clone)]
pub struct ConstraintResult {
    pub constraint: String,
    pub holds: bool,
    pub solver_time_ms: u64,
    pub model: Option<HashMap<String, SmtValue>>,
}

#[derive(Debug, Clone)]
pub struct Counterexample {
    pub description: String,
    pub inputs: HashMap<String, SmtValue>,
}

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

    pub fn verify_one(
        &self,
        candidate: &CandidateImplementation,
        spec: &FunctionSpec,
    ) -> VerificationResult {
        let start = Instant::now();
        let mut constraint_results = Vec::new();
        let mut counterexample = None;

        if !is_well_formed(&candidate.body) {
            return VerificationResult {
                candidate_id: candidate.id, status: VerificationStatus::Malformed,
                constraint_results: vec![], counterexample: None,
                verification_time_ms: start.elapsed().as_millis() as u64,
            };
        }

        // The verdict must be a statement about THIS candidate:
        // the spec's parameters become free Z3 constants and the
        // candidate body is symbolically evaluated over them,
        // once, producing the expression `output` denotes for
        // this implementation. `None` = the body is outside the
        // encodable subset; every constraint is then Unknown and
        // the candidate can only be Inconclusive, never Verified.
        let param_env = param_env(spec);
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
            verification_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn verify_constraint(
        &self,
        constraint: &Constraint,
        label: &str,
        spec: &FunctionSpec,
        param_env: &SymEnv,
        output: &Option<Sym>,
    ) -> (ConstraintResult, Tri) {
        let start = Instant::now();
        let done = |holds: bool, model: Option<HashMap<String, SmtValue>>, tri: Tri| {
            (
                ConstraintResult {
                    constraint: label.into(),
                    holds,
                    solver_time_ms: start.elapsed().as_millis() as u64,
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
//   Call ("len", the "List" literal, and — when the verifier
//   carries the module — calls to other specs, inlined),
//   Collection, Index, Typed.
//
// Anything else — holes, loops, mutation, lambdas and other
// function values, higher-order combinators, calls that do
// not resolve, guard/constructor patterns that cannot be
// decided — makes the body unencodable (`None`), never a
// guessed value.
//
// Representable sorts are Int, Bool and String, plus literal
// lists of representable values (needed for len/Index over
// collection literals). Floats and Chars have no honest Z3
// sort here (IEEE floats are not Z3 reals), so they are
// `Opaque`: bindable and discardable, but any use that needs
// a concrete sort makes the body unencodable.

#[derive(Debug, Clone)]
enum Sym {
    Int(z3::ast::Int),
    Bool(z3::ast::Bool),
    Str(z3::ast::String),
    List(Vec<Sym>),
    Opaque,
}

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
struct EvalCtx<'a> {
    index: &'a SpecIndex,
    visiting: Vec<String>,
    depth: usize,
}

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
enum SymFlow {
    Normal(Sym),
    Returned(Sym),
}

impl SymFlow {
    fn into_value(self) -> Sym {
        match self {
            SymFlow::Normal(v) | SymFlow::Returned(v) => v,
        }
    }
}

/// The initial symbolic environment: one free constant per
/// spec parameter, of the parameter's sort. Parameters whose
/// type has no representable sort are Opaque.
fn param_env(spec: &FunctionSpec) -> SymEnv {
    let mut env = SymEnv::new();
    for p in &spec.params {
        let sym = match &p.ty {
            TypeRef::Int
            | TypeRef::I8
            | TypeRef::I16
            | TypeRef::I32
            | TypeRef::I64
            | TypeRef::U8
            | TypeRef::U16
            | TypeRef::U32
            | TypeRef::U64 => Sym::Int(z3::ast::Int::new_const(p.name.as_str())),
            TypeRef::Bool => Sym::Bool(z3::ast::Bool::new_const(p.name.as_str())),
            TypeRef::String => Sym::Str(z3::ast::String::new_const(p.name.as_str())),
            _ => Sym::Opaque,
        };
        env.insert(p.name.clone(), sym);
    }
    env
}

fn sym_eval(node: &IRNode, env: &mut SymEnv, ctx: &EvalCtx) -> Option<Sym> {
    Some(sym_eval_flow(node, env, ctx)?.into_value())
}

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

fn sym_literal(lit: &IRLiteral) -> Option<Sym> {
    match lit {
        IRLiteral::Int(n) => Some(Sym::Int(z3::ast::Int::from_i64(*n))),
        IRLiteral::Bool(b) => Some(Sym::Bool(z3::ast::Bool::from_bool(*b))),
        IRLiteral::String(s) => {
            Some(Sym::Str(z3::ast::String::from_str(s).ok()?))
        }
        // Float, Char, Unit: no honest Z3 sort in this encoder.
        IRLiteral::Float(_) | IRLiteral::Char(_) | IRLiteral::Unit => {
            Some(Sym::Opaque)
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
        // A list scrutinee has no literal patterns; an Opaque
        // scrutinee cannot be compared at all.
        (Sym::List(_), _) | (Sym::Opaque, _) => PatternCond::Undecidable,
        // Remaining pairs are provably disjoint sorts.
        _ => PatternCond::Never,
    }
}

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

fn sym_len(v: Sym) -> Option<Sym> {
    match v {
        Sym::List(items) => Some(Sym::Int(z3::ast::Int::from_i64(items.len() as i64))),
        Sym::Str(s) => Some(Sym::Int(s.length())),
        _ => None,
    }
}

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
            // A character of a literal string at a constant
            // in-range index: a Char value (Opaque sort).
            let n = i.as_i64()?;
            if n < 0 {
                return None;
            }
            let lit = s.as_string()?;
            if lit.chars().nth(n as usize).is_some() {
                Some(Sym::Opaque)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn sym_ite(cond: &z3::ast::Bool, a: Sym, b: Sym) -> Option<Sym> {
    match (a, b) {
        (Sym::Int(x), Sym::Int(y)) => Some(Sym::Int(cond.ite(&x, &y))),
        (Sym::Bool(x), Sym::Bool(y)) => Some(Sym::Bool(cond.ite(&x, &y))),
        (Sym::Str(x), Sym::Str(y)) => Some(Sym::Str(cond.ite(&x, &y))),
        (Sym::Opaque, Sym::Opaque) => Some(Sym::Opaque),
        _ => None,
    }
}

// ── Shared symbolic operators ──────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        _ => None,
    }
}

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

fn sym_neg(v: Sym) -> Option<Sym> {
    match v {
        Sym::Int(a) => Some(Sym::Int(z3_sub(&z3::ast::Int::from_i64(0), &a))),
        _ => None,
    }
}

fn sym_not(v: Sym) -> Option<Sym> {
    match v {
        Sym::Bool(a) => Some(Sym::Bool(a.not())),
        _ => None,
    }
}

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

fn encode_expr(expr: &Expr, env: &SymEnv, ctx: &EvalCtx) -> Option<Sym> {
    match expr {
        Expr::Var(name) => env.get(name).cloned(),
        Expr::IntLit(n) => Some(Sym::Int(z3::ast::Int::from_i64(*n))),
        Expr::FloatLit(_) => Some(Sym::Opaque),
        Expr::BoolLit(b) => Some(Sym::Bool(z3::ast::Bool::from_bool(*b))),
        Expr::StringLit(s) => Some(Sym::Str(z3::ast::String::from_str(s).ok()?)),
        // Char, like Float: no honest Z3 sort in this
        // encoder — Opaque (bindable, not usable in an
        // operation that needs a concrete sort).
        Expr::CharLit(_) => Some(Sym::Opaque),
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
        Expr::Field(..) | Expr::Lambda(..) | Expr::Comprehension { .. } => None,
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
pub fn body_is_encodable_with(
    spec: &FunctionSpec,
    body: &IRNode,
    index: &SpecIndex,
) -> bool {
    let mut env = param_env(spec);
    sym_eval(body, &mut env, &EvalCtx::root(index)).is_some()
}

pub fn verify_all(
    candidates: Vec<CandidateImplementation>,
    spec: &FunctionSpec,
) -> Result<Vec<CandidateImplementation>, VerificationError> {
    Verifier::new().verify_all(candidates, spec)
}

#[derive(Debug)]
pub enum VerificationError {
    SolverError(String),
    Timeout,
    InternalError(String),
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationError::SolverError(m) => write!(f, "Solver error: {}", m),
            VerificationError::Timeout => write!(f, "Verification timeout"),
            VerificationError::InternalError(m) => write!(f, "Internal: {}", m),
        }
    }
}
impl std::error::Error for VerificationError {}

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
}
