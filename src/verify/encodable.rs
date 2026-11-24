// ─────────────────────────────────────────────────────────────
//  ENCODABILITY, WITHOUT A SOLVER (Phase 66, v0.6 W1)
//
//  `verifier::body_is_encodable_with` answers "does symbolic
//  evaluation of this body over the spec's parameters
//  produce a value?" by actually running the Z3-backed
//  symbolic evaluator — which makes a pure yes/no question
//  depend on the z3 crate being linked (audit finding S3).
//  This module answers the same question with an ABSTRACT
//  evaluation: instead of Z3 values it tracks, per
//  subexpression, exactly the facts the real evaluator's
//  Some/None outcome depends on:
//
//    - the SORT of the value (Int / Bool / Str / List /
//      ListLen / ArrayLen / Char / CharStr / Record /
//      Option / Result / Opaque) — every operator in the
//      real evaluator is sort-strict, and sort mismatches
//      are `None`;
//    - for Int, whether the Z3 term is a NUMERAL (the only
//      place a value is ever inspected is `sym_index`'s
//      `as_i64()`). A term is a numeral only when it was
//      built by `Int::from_i64` — integer literals, and
//      `len` of a literal list. Z3's arithmetic
//      constructors do NOT fold: `1 + 1` is an application
//      term that `Z3_get_numeral_int64` cannot read
//      (measured — see the cross-check's corner corpus), so
//      every arithmetic result is Unknown here, as are
//      `len` of a string and `abs` (an ite term) and every
//      `ite` itself. Known values otherwise flow unchanged
//      through Let/Var/Collection/Match/inlining.
//    - for Str, whether the term is a known literal
//      (`as_string()` in `sym_index`): literals are, unless
//      they contain a NUL byte — `z3::ast::String::from_str`
//      fails on exactly those (CString), which the real
//      evaluator maps to unencodable. Concatenations and
//      parameters are not known literals.
//    - for Char (Phase 76), the same numeral question: a
//      Char literal and a constant string index are
//      numerals; a Char parameter and every `ite` are not.
//      The bridge's cross-representation comparison
//      (CharStr vs Char) is encodable exactly when the
//      Char side is a numeral.
//    - for Option/Result (Phase 76), the constructor tag's
//      CLASS (literal true / literal false / free): the
//      real equality case analysis turns on which payload
//      slots a literal tag leaves unobserved, and two free
//      tags merge to a free tag exactly as two literal
//      tags merge to themselves.
//
//  The walk mirrors `sym_eval_flow` construct by construct,
//  including its control flow (Return unwinding, dead match
//  arms whose bodies are never evaluated, the inlining
//  depth cap and cycle guard). It is used ONLY where no Z3
//  backend exists; the Z3 build keeps the original
//  predicate. The cross-check test at the bottom of this
//  file (z3-support builds) asserts the two agree on every
//  body in a harvested corpus, so the mirror cannot drift
//  silently from the evaluator it abstracts.
// ─────────────────────────────────────────────────────────────

use std::collections::HashMap;

use crate::spec::ast::{
    BinOp as SpecBinOp, Constraint, Expr as SpecExpr, FunctionSpec, OrderOp, TypeRef,
    UnaryOp as SpecUnaryOp,
};
use crate::synthesis::engine::{
    IRBinOp, IRLiteral, IRNode, IRPattern, IRUnaryOp, SpecIndex, MAX_INLINE_DEPTH,
};

// ── Abstract values ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Abs {
    Int(IntVal),
    Bool,
    Str(StrVal),
    List(Vec<Abs>),
    /// A list known only by its length (Phase 76) — `len`
    /// is an (Unknown) Int; everything else is `None`.
    ListLen,
    /// A `List<Int>` value as a pair (Phase 139, W1) —
    /// mirrors `Sym::ArrayLen`: the standing length term
    /// plus an `Array<Int, Int>` term. The pair carries
    /// no element facts; at W2 it carries its length's
    /// abstract form (`AbsLen`), which is what the
    /// mirror's guard discharge (§2.3) computes over:
    /// `len` is an (Unknown) Int, an Int Append stores
    /// (the shape survives, the form grows), merges of
    /// two pairs — or of a pair with a concrete Int list,
    /// through the §2.5 bridge — stay pairs; indexing
    /// exists only under the guard discharge, equality
    /// (§2.4) and Concat-as-length (§2.6) have their arms,
    /// exactly as the real evaluator's stand at W2.
    ArrayLen { len: AbsLen },
    /// A Char scalar (Phase 76): Known iff the real term
    /// is a Z3 numeral (literal / constant index).
    Char(IntVal),
    /// An indexed char in the bridge's string form.
    CharStr,
    Record {
        type_name: String,
        fields: Vec<(String, Abs)>,
    },
    Option {
        some: AbsTag,
        payload: Option<Box<Abs>>,
    },
    Result {
        ok: AbsTag,
        ok_payload: Option<Box<Abs>>,
        err_payload: Option<Box<Abs>>,
    },
    /// A function-typed parameter in the UF arm's slice
    /// (Phase 133) — mirrors `Sym::Func`: the value's only
    /// symbolic life is application, whose abstract result
    /// is the range's plain unknown value (see `abs_apply`).
    Func {
        domain: Vec<AbsUfSort>,
        range: AbsUfSort,
    },
    Opaque,
}

/// The abstract form of a paired binding's length term
/// (Phase 140) — the facts the mirror's guard domain can
/// state about it. `Known` is a numeral: a crossed
/// literal's count, grown by appends. `Param(name, off)`
/// is the named parameter's length term plus a constant
/// offset (appends shift the offset); it is set only for
/// a parameter bound directly in `param_env`, the one
/// place a length has a spec-level name the require facts
/// can speak about. `Unknown` is everything else —
/// nested pairs (record fields, Option payloads), merges
/// of unlike forms: reads over an Unknown length decline
/// on the mirror side. That is a sound subset of the real
/// side's solver discharge (which works on the terms
/// themselves), never a superset: the mirror may decline
/// a read the solver would discharge — over a nested
/// pair's length, or under non-linear require facts the
/// difference domain cannot state — and the agreement
/// battery pins the two equal on every shape the corpus
/// exercises.
#[derive(Debug, Clone, PartialEq)]
enum AbsLen {
    Known(i64),
    Param(String, i64),
    Unknown,
}

impl AbsLen {
    /// Append's growth: the length term plus one.
    fn grown(&self) -> AbsLen {
        match self {
            AbsLen::Known(n) => AbsLen::Known(n + 1),
            AbsLen::Param(name, off) => AbsLen::Param(name.clone(), off + 1),
            AbsLen::Unknown => AbsLen::Unknown,
        }
    }

    /// An ite merge's form: the merged length denotes the
    /// shared form when both sides agree (the real term
    /// is `ite(c, t, t)`, which is `t` in every model);
    /// unlike forms merge to a term the domain cannot
    /// state.
    fn merged(&self, other: &AbsLen) -> AbsLen {
        if self == other {
            self.clone()
        } else {
            AbsLen::Unknown
        }
    }
}

/// A sort in the UF arm's slice (Phase 133) — mirrors
/// verifier.rs's `UfSort` decision for decision: Int
/// family / Bool / String are in; Char, List, Option,
/// Result, Record and function types are out, and a
/// signature mentioning them binds Opaque on both sides.
#[derive(Debug, Clone, Copy, PartialEq)]
enum AbsUfSort {
    Int,
    Bool,
    Str,
}

impl AbsUfSort {
    /// The abstract value of an application result: the
    /// range sort's plain symbolic value — an Int term
    /// that is no numeral, a Bool, a non-literal String.
    fn result(self) -> Abs {
        match self {
            AbsUfSort::Int => Abs::Int(IntVal::Unknown),
            AbsUfSort::Bool => Abs::Bool,
            AbsUfSort::Str => Abs::Str(StrVal::Unknown),
        }
    }

    /// Whether an abstract argument value carries this
    /// sort — the mirror of `sym_apply`'s argument
    /// discipline (an Int term at an Int point, and so
    /// on; every other class declines on both sides).
    fn accepts(self, v: &Abs) -> bool {
        matches!(
            (self, v),
            (AbsUfSort::Int, Abs::Int(_))
                | (AbsUfSort::Bool, Abs::Bool)
                | (AbsUfSort::Str, Abs::Str(_))
        )
    }
}

/// The UF sort of a single type, if it lies in the slice
/// (mirrors `uf_sort_of`).
fn abs_uf_sort(ty: &TypeRef) -> Option<AbsUfSort> {
    match ty {
        TypeRef::Int
        | TypeRef::I8
        | TypeRef::I16
        | TypeRef::I32
        | TypeRef::I64
        | TypeRef::U8
        | TypeRef::U16
        | TypeRef::U32
        | TypeRef::U64 => Some(AbsUfSort::Int),
        TypeRef::Bool => Some(AbsUfSort::Bool),
        TypeRef::String => Some(AbsUfSort::Str),
        _ => None,
    }
}

/// The UF signature of a function type, if every domain
/// sort and the range sort lie in the slice (mirrors
/// `uf_signature`).
fn abs_uf_signature(ty: &TypeRef) -> Option<(Vec<AbsUfSort>, AbsUfSort)> {
    let TypeRef::Function(params, ret) = ty else {
        return None;
    };
    let mut domain = Vec::with_capacity(params.len());
    for p in params {
        domain.push(abs_uf_sort(p)?);
    }
    Some((domain, abs_uf_sort(ret)?))
}

/// Apply a function-typed parameter's UF, abstractly
/// (Phase 133) — mirrors `sym_apply`: arity and argument
/// sorts must match the declared signature exactly; the
/// result is the range's abstract value.
fn abs_apply(func: &Abs, args: Vec<Abs>) -> Option<Abs> {
    let Abs::Func { domain, range } = func else {
        return None;
    };
    if args.len() != domain.len() {
        return None;
    }
    for (arg, want) in args.iter().zip(domain.iter()) {
        if !want.accepts(arg) {
            return None;
        }
    }
    Some(range.result())
}

/// The class of a constructor tag (mirrors the real
/// evaluator's `Tag`): literal true, literal false, or a
/// free symbolic term. Only the class matters to
/// encodability.
#[derive(Debug, Clone, Copy, PartialEq)]
enum AbsTag {
    True,
    False,
    Free,
}

impl AbsTag {
    /// The class of an `ite` merge: a shared literal
    /// survives; anything else is free.
    fn merge(self, other: AbsTag) -> AbsTag {
        if self == other {
            self
        } else {
            AbsTag::Free
        }
    }
}

/// Whether an Int term is a Z3 numeral (see module docs).
#[derive(Debug, Clone, Copy, PartialEq)]
enum IntVal {
    Known(i64),
    Unknown,
}

/// Whether a Str term is a known literal (see module docs).
#[derive(Debug, Clone, PartialEq)]
enum StrVal {
    Known(String),
    Unknown,
}

type AbsEnv = HashMap<String, Abs>;

/// A base a guard-domain term speaks about (Phase 140):
/// a spec Int parameter's value, or a spec `List<Int>`
/// parameter's length. Facts and goals are stated over
/// bases, never over binder names — the domain's analogue
/// of the real side's guards being Z3 terms, not names.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AbsBase {
    Var(String),
    LenOf(String),
}

/// A simple term in the guard domain: a base plus a
/// constant offset (`base + off`); a `None` base is the
/// constant `off` itself.
#[derive(Debug, Clone, PartialEq)]
struct AbsTerm {
    base: Option<AbsBase>,
    off: i64,
}

/// A comparison operator in an extracted guard fact.
#[derive(Debug, Clone, Copy, PartialEq)]
enum AbsCmp {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

/// One atomic comparison between simple terms, extracted
/// from a spec's `require:` row (Phase 140). The domain
/// is difference logic over the bases: each fact is a
/// lower bound on a difference of two base values, and
/// entailment is the longest-path closure — sound over
/// the integers, and a subset of what the real side's
/// solver derives from the same rows' terms (an extracted
/// fact is a conjunct of its row, stated exactly).
#[derive(Debug, Clone)]
struct AbsFact {
    lhs: AbsTerm,
    cmp: AbsCmp,
    rhs: AbsTerm,
}

/// The inlining context, mirroring verifier::EvalCtx field
/// for field (same cap, same cycle guard).
struct AbsCtx<'a> {
    index: &'a SpecIndex,
    visiting: Vec<String>,
    depth: usize,
    /// The guard facts in scope for the index arm's
    /// discharge (Phase 140), extracted from the spec's
    /// require rows — the mirror of EvalCtx's guard terms.
    facts: Vec<AbsFact>,
    /// The spec's parameter names whose `Var` form the
    /// guard domain may still speak about. A name leaves
    /// the set when a Let or Match binding shadows it (the
    /// facts speak about the parameter's value, not the
    /// binding's), and the set clears inside an inlined
    /// callee, whose parameters are bound to argument
    /// VALUES: a callee's same-named parameter is a
    /// different value, and the caller facts say nothing
    /// about it by name. (Length bases survive inlining —
    /// they are carried by the argument VALUES' forms, not
    /// looked up by name.)
    params: std::collections::HashSet<String>,
}

impl<'a> AbsCtx<'a> {
    fn root(index: &'a SpecIndex) -> Self {
        Self {
            index,
            visiting: Vec::new(),
            depth: 0,
            facts: Vec::new(),
            params: std::collections::HashSet::new(),
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
            facts: self.facts.clone(),
            params: self.params.clone(),
        }
    }

    /// The root context for judging `spec`'s bodies
    /// (Phase 140): the require facts extracted, the
    /// parameter names live.
    fn for_spec(index: &'a SpecIndex, spec: &FunctionSpec) -> Self {
        let mut ctx = Self::root(index);
        ctx.facts = guard_facts(spec);
        ctx.params = spec.params.iter().map(|p| p.name.clone()).collect();
        ctx
    }

    /// The same context with one parameter name shadowed
    /// by a local binding (Phase 140).
    fn without_param(&self, name: &str) -> Self {
        let mut params = self.params.clone();
        params.remove(name);
        Self {
            index: self.index,
            visiting: self.visiting.clone(),
            depth: self.depth,
            facts: self.facts.clone(),
            params,
        }
    }

    /// The same context inside an inlined callee (Phase
    /// 140): no parameter name is live (see the field
    /// doc), the facts stand.
    fn for_callee(&self) -> Self {
        Self {
            index: self.index,
            visiting: self.visiting.clone(),
            depth: self.depth,
            facts: self.facts.clone(),
            params: std::collections::HashSet::new(),
        }
    }
}

enum AbsFlow {
    Normal(Abs),
    Returned(Abs),
}

impl AbsFlow {
    fn into_value(self) -> Abs {
        match self {
            AbsFlow::Normal(v) | AbsFlow::Returned(v) => v,
        }
    }
}

fn param_env(spec: &FunctionSpec, index: &SpecIndex) -> AbsEnv {
    let mut env = AbsEnv::new();
    for p in &spec.params {
        let mut val = fresh_abs(&p.ty, index, 0);
        // A directly-bound `List<Int>` parameter's length
        // is the parameter's own length term (Phase 140) —
        // the base the guard facts speak about. Nested
        // pairs (record fields, Option payloads) keep the
        // Unknown form: their lengths have no spec-level
        // name a require could state a fact over.
        if matches!(&p.ty, TypeRef::List(el) if matches!(**el, TypeRef::Int)) {
            if let Abs::ArrayLen { len } = &mut val {
                *len = AbsLen::Param(p.name.clone(), 0);
            }
        }
        env.insert(p.name.clone(), val);
    }
    env
}

/// The abstract counterpart of the real evaluator's
/// `fresh_sym` (same recursion bound): a fresh value of a
/// type, tracking only the facts encodability turns on.
/// Record types resolve through the index's declarations
/// (a declarations-less index binds them Opaque, exactly
/// as the real binding does).
fn fresh_abs(ty: &TypeRef, index: &SpecIndex, depth: usize) -> Abs {
    const MAX_FRESH_DEPTH: usize = 8;
    if depth > MAX_FRESH_DEPTH {
        return Abs::Opaque;
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
        | TypeRef::U64 => Abs::Int(IntVal::Unknown),
        TypeRef::Bool => Abs::Bool,
        TypeRef::String => Abs::Str(StrVal::Unknown),
        TypeRef::Char => Abs::Char(IntVal::Unknown),
        TypeRef::List(inner) => match **inner {
            // Phase 139 (W1): `List<Int>` binds as the
            // pair, mirroring `fresh_sym` — every other
            // element sort keeps ListLen (§9 R9). The
            // length form starts Unknown; `param_env`
            // names it for a directly-bound parameter
            // (Phase 140).
            TypeRef::Int => Abs::ArrayLen {
                len: AbsLen::Unknown,
            },
            _ => Abs::ListLen,
        },
        TypeRef::Option(inner) => Abs::Option {
            some: AbsTag::Free,
            payload: Some(Box::new(fresh_abs(inner, index, depth + 1))),
        },
        TypeRef::Result(ok_ty, err_ty) => Abs::Result {
            ok: AbsTag::Free,
            ok_payload: Some(Box::new(fresh_abs(ok_ty, index, depth + 1))),
            err_payload: Some(Box::new(fresh_abs(err_ty, index, depth + 1))),
        },
        TypeRef::Named(name) => match index.record_fields(name) {
            Some((resolved, fields)) => Abs::Record {
                type_name: resolved,
                fields: fields
                    .iter()
                    .map(|(fname, fty)| {
                        (fname.clone(), fresh_abs(fty, index, depth + 1))
                    })
                    .collect(),
            },
            None => Abs::Opaque,
        },
        // A function-typed parameter (Phase 133): the UF
        // binding when the signature lies in the arm's
        // slice, Opaque outside it — as the real binding.
        TypeRef::Function(..) => match abs_uf_signature(ty) {
            Some((domain, range)) => Abs::Func { domain, range },
            None => Abs::Opaque,
        },
        _ => Abs::Opaque,
    }
}

// ── The walk (mirrors sym_eval / sym_eval_flow) ────────────

fn abs_eval(node: &IRNode, env: &mut AbsEnv, ctx: &AbsCtx) -> Option<Abs> {
    Some(abs_eval_flow(node, env, ctx)?.into_value())
}

fn abs_eval_flow(node: &IRNode, env: &mut AbsEnv, ctx: &AbsCtx) -> Option<AbsFlow> {
    match node {
        IRNode::Literal(lit) => Some(AbsFlow::Normal(abs_literal(lit)?)),

        IRNode::Var(name) => env.get(name).cloned().map(AbsFlow::Normal),

        IRNode::BinOp(op, l, r) => {
            let lv = abs_eval(l, env, ctx)?;
            let rv = abs_eval(r, env, ctx)?;
            Some(AbsFlow::Normal(abs_binop_ir(*op, lv, rv)?))
        }

        IRNode::UnaryOp(op, inner) => {
            let v = abs_eval(inner, env, ctx)?;
            Some(AbsFlow::Normal(abs_unary_ir(*op, v)?))
        }

        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => match abs_eval(condition, env, ctx)? {
            Abs::Bool => {
                let t = abs_eval(then_branch, env, ctx)?;
                let e = abs_eval(else_branch, env, ctx)?;
                Some(AbsFlow::Normal(abs_ite(t, e)?))
            }
            _ => None,
        },

        IRNode::Let { name, value, body } => {
            let v = match abs_eval_flow(value, env, ctx)? {
                AbsFlow::Returned(v) => return Some(AbsFlow::Returned(v)),
                AbsFlow::Normal(v) => v,
            };
            let mut inner = env.clone();
            inner.insert(name.clone(), v);
            // The binding shadows a same-named parameter
            // for the guard domain (Phase 140): the facts
            // speak about the parameter's value, not this
            // binding's.
            let inner_ctx = ctx.without_param(name);
            abs_eval_flow(body, &mut inner, &inner_ctx)
        }

        IRNode::Block(stmts) => {
            let mut last = Abs::Opaque; // an empty block is Unit
            for stmt in stmts {
                match abs_eval_flow(stmt, env, ctx)? {
                    AbsFlow::Returned(v) => return Some(AbsFlow::Returned(v)),
                    AbsFlow::Normal(v) => last = v,
                }
            }
            Some(AbsFlow::Normal(last))
        }

        IRNode::Return(inner) => {
            let v = abs_eval(inner, env, ctx)?;
            Some(AbsFlow::Returned(v))
        }

        IRNode::Match { scrutinee, cases } => {
            abs_match(scrutinee, cases, env, ctx).map(AbsFlow::Normal)
        }

        IRNode::Call { function, args } => abs_call(function, args, env, ctx).map(AbsFlow::Normal),

        IRNode::Collection { elements, .. } => {
            let mut values = Vec::with_capacity(elements.len());
            for el in elements {
                match abs_eval_flow(el, env, ctx)? {
                    AbsFlow::Returned(v) => return Some(AbsFlow::Returned(v)),
                    AbsFlow::Normal(v) => values.push(v),
                }
            }
            Some(AbsFlow::Normal(Abs::List(values)))
        }

        IRNode::Index(coll, idx) => {
            let coll_v = abs_eval(coll, env, ctx)?;
            let idx_v = abs_eval(idx, env, ctx)?;
            // The paired binding (Phase 140, §2.3): the
            // read exists only under the guard discipline —
            // the mirror's discharge over the spec's
            // require facts, in the difference domain. The
            // read's value is an Int term the domain does
            // not track: Int(Unknown), exactly as the real
            // select is a non-numeral term.
            if let (Abs::ArrayLen { len }, Abs::Int(_)) = (&coll_v, &idx_v) {
                let len_t = abs_len_form(len)?;
                let idx_t = abs_index_term(idx, env, ctx)?;
                return if abs_guard_discharged(&ctx.facts, &idx_t, &len_t) {
                    Some(AbsFlow::Normal(Abs::Int(IntVal::Unknown)))
                } else {
                    None
                };
            }
            Some(AbsFlow::Normal(abs_index(coll_v, idx_v)?))
        }

        IRNode::FieldAccess { base, field } => {
            // Projection by name (Phase 76), mirroring the
            // real evaluator: a record base and a known
            // field, else unencodable.
            match abs_eval(base, env, ctx)? {
                Abs::Record { fields, .. } => fields
                    .into_iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, v)| AbsFlow::Normal(v)),
                _ => None,
            }
        }

        IRNode::RecordLit { type_name, fields } => {
            // Construction (Phase 76): fields in written
            // order, Return unwinding as in Collection; the
            // stored name resolves through the index exactly
            // as the real evaluator resolves it.
            let mut values = Vec::with_capacity(fields.len());
            for (name, value) in fields {
                match abs_eval_flow(value, env, ctx)? {
                    AbsFlow::Returned(v) => return Some(AbsFlow::Returned(v)),
                    AbsFlow::Normal(v) => values.push((name.clone(), v)),
                }
            }
            let type_name = match ctx.index.record_fields(type_name) {
                Some((resolved, _)) => resolved,
                None => type_name.clone(),
            };
            Some(AbsFlow::Normal(Abs::Record {
                type_name,
                fields: values,
            }))
        }

        IRNode::Typed { node, .. } => abs_eval_flow(node, env, ctx),

        // Outside the encodable subset, exactly as in the
        // real evaluator.
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

fn abs_literal(lit: &IRLiteral) -> Option<Abs> {
    match lit {
        IRLiteral::Int(n) => Some(Abs::Int(IntVal::Known(*n))),
        IRLiteral::Bool(_) => Some(Abs::Bool),
        IRLiteral::String(s) => {
            // z3::ast::String::from_str fails only on an
            // interior NUL (CString); the real evaluator maps
            // that failure to unencodable.
            if s.contains('\0') {
                None
            } else {
                Some(Abs::Str(StrVal::Known(s.clone())))
            }
        }
        // A Char literal is a numeral scalar (Phase 76).
        IRLiteral::Char(c) => Some(Abs::Char(IntVal::Known(*c as u32 as i64))),
        // Float, Unit: Opaque in the real evaluator.
        IRLiteral::Float(_) | IRLiteral::Unit => Some(Abs::Opaque),
    }
}

// ── Match (mirrors sym_match / sym_pattern_cond) ───────────

enum PatternCond {
    Eq,
    Never,
    Undecidable,
}

fn abs_pattern_cond(scrut: &Abs, lit: &IRLiteral) -> PatternCond {
    match (scrut, lit) {
        (Abs::Int(_), IRLiteral::Int(_)) => PatternCond::Eq,
        (Abs::Int(_), IRLiteral::Float(f)) => {
            // Interpreter promotion: Int(i) == Float(f) iff
            // i as f64 == f; only an integral f in i64 range
            // can ever be equal.
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                PatternCond::Eq
            } else {
                PatternCond::Never
            }
        }
        (Abs::Bool, IRLiteral::Bool(_)) => PatternCond::Eq,
        (Abs::Str(_), IRLiteral::String(s)) => {
            if s.contains('\0') {
                PatternCond::Undecidable
            } else {
                PatternCond::Eq
            }
        }
        // Char scrutinees against Char literal patterns
        // encode (Phase 76), in both Char representations.
        (Abs::Char(_), IRLiteral::Char(_)) => PatternCond::Eq,
        (Abs::CharStr, IRLiteral::Char(_)) => PatternCond::Eq,
        (Abs::List(_), _) | (Abs::Opaque, _) => PatternCond::Undecidable,
        // A function scrutinee (Phase 133): declines, as
        // the real side — Undecidable, not Never.
        (Abs::Func { .. }, _) => PatternCond::Undecidable,
        // Records, tagged values and list-lengths never
        // equal a literal pattern; a Char never equals a
        // literal of another sort.
        _ => PatternCond::Never,
    }
}

fn abs_match(
    scrutinee: &IRNode,
    cases: &[(IRPattern, IRNode)],
    env: &mut AbsEnv,
    ctx: &AbsCtx,
) -> Option<Abs> {
    let scrut = abs_eval(scrutinee, env, ctx)?;
    // (is_conditional, value) per arm — the real evaluator
    // also builds the Z3 condition, but encodability turns
    // only on its existence.
    let mut arms: Vec<(bool, Abs)> = Vec::new();
    // Tag-side coverage from constructor arms (Phase 91),
    // mirroring sym_match: a free-tag scrutinee is
    // exhausted once live arms discriminate both sides.
    let mut covers_pos = false;
    let mut covers_neg = false;
    for (pattern, body) in cases {
        match pattern {
            IRPattern::Wildcard => {
                arms.push((false, abs_eval(body, env, ctx)?));
                break; // later arms are unreachable
            }
            IRPattern::Variable(name) => {
                let mut inner = env.clone();
                inner.insert(name.clone(), scrut.clone());
                // The pattern binding shadows a same-named
                // parameter for the guard domain (Phase 140).
                let inner_ctx = ctx.without_param(name);
                arms.push((false, abs_eval(body, &mut inner, &inner_ctx)?));
                break;
            }
            IRPattern::Literal(lit) => match abs_pattern_cond(&scrut, lit) {
                PatternCond::Eq => {
                    arms.push((true, abs_eval(body, env, ctx)?));
                }
                // Provably never equal: the arm is dead and
                // its body is never evaluated.
                PatternCond::Never => continue,
                PatternCond::Undecidable => return None,
            },
            IRPattern::Constructor(name, pargs) => {
                match abs_constructor_arm(&scrut, name) {
                    // The scrutinee never carries this
                    // constructor: dead arm, as in the real
                    // evaluator.
                    AbsCtorArm::Dead => continue,
                    AbsCtorArm::Unsupported => return None,
                    AbsCtorArm::Live {
                        conditional,
                        payload,
                        side,
                    } => {
                        let mut inner = env.clone();
                        // A payload binding shadows a
                        // same-named parameter for the guard
                        // domain (Phase 140).
                        let mut inner_ctx_holder;
                        let mut arm_ctx = ctx;
                        match (pargs.len(), &payload) {
                            (0, _) => {}
                            (1, Some(p)) => match &pargs[0] {
                                IRPattern::Variable(n) => {
                                    inner.insert(n.clone(), (**p).clone());
                                    inner_ctx_holder = ctx.without_param(n);
                                    arm_ctx = &inner_ctx_holder;
                                }
                                IRPattern::Wildcard => {}
                                _ => return None,
                            },
                            _ => return None,
                        }
                        let value = abs_eval(body, &mut inner, arm_ctx)?;
                        match side {
                            AbsTagSide::Pos => covers_pos = true,
                            AbsTagSide::Neg => covers_neg = true,
                        }
                        arms.push((conditional, value));
                        if !conditional {
                            break; // later arms are unreachable
                        }
                    }
                }
            }
            IRPattern::Guard(..) => return None,
        }
    }
    let (last_conditional, last_val) = arms.pop()?;
    if last_conditional && !(covers_pos && covers_neg) {
        return None; // the match can fall through
    }
    let mut acc = last_val;
    for (_, val) in arms.into_iter().rev() {
        acc = abs_ite(val, acc)?;
    }
    Some(acc)
}

/// Which side of a tagged scrutinee a constructor
/// discriminates (Phase 91), mirroring the real
/// evaluator's TagSide.
#[derive(Clone, Copy)]
enum AbsTagSide {
    Pos,
    Neg,
}

/// A constructor pattern read against a scrutinee value
/// (Phase 91), mirroring the real evaluator's CtorArm:
/// only the tag's CLASS is tracked, since encodability
/// turns on whether a condition exists, not its term.
enum AbsCtorArm {
    Dead,
    Unsupported,
    Live {
        conditional: bool,
        payload: Option<Box<Abs>>,
        side: AbsTagSide,
    },
}

fn abs_constructor_arm(scrut: &Abs, name: &str) -> AbsCtorArm {
    let (tag, payload, side) = match (scrut, name) {
        (Abs::Option { some, payload }, "Some") => {
            (*some, payload.clone(), AbsTagSide::Pos)
        }
        (Abs::Option { some, .. }, "None") => (*some, None, AbsTagSide::Neg),
        (Abs::Result { ok, ok_payload, .. }, "Ok") => {
            (*ok, ok_payload.clone(), AbsTagSide::Pos)
        }
        (Abs::Result { ok, err_payload, .. }, "Err") => {
            (*ok, err_payload.clone(), AbsTagSide::Neg)
        }
        _ => return AbsCtorArm::Unsupported,
    };
    match (tag, side) {
        (AbsTag::True, AbsTagSide::Pos) | (AbsTag::False, AbsTagSide::Neg) => {
            AbsCtorArm::Live {
                conditional: false,
                payload,
                side,
            }
        }
        (AbsTag::True, AbsTagSide::Neg) | (AbsTag::False, AbsTagSide::Pos) => {
            AbsCtorArm::Dead
        }
        (AbsTag::Free, _) => AbsCtorArm::Live {
            conditional: true,
            payload,
            side,
        },
    }
}

// ── Calls (mirrors sym_call / sym_invoke) ──────────────────

fn abs_invoke(ctx: &AbsCtx, name: &str, args: Vec<Abs>) -> Option<Abs> {
    if !ctx.can_inline(name) {
        return None;
    }
    let callee = ctx.index.get(name)?;
    if callee.params.len() != args.len() {
        return None;
    }
    let body = ctx.index.callee_body(name)?;
    let mut env: AbsEnv = callee
        .params
        .iter()
        .map(|p| p.name.clone())
        .zip(args)
        .collect();
    // Inside the callee no parameter NAME is live for the
    // guard domain (Phase 140): the callee's parameters are
    // bound to argument values, and the facts speak about
    // the caller's parameters by name. Length bases survive
    // — they ride on the argument values' forms.
    abs_eval(&body, &mut env, &ctx.child(name).for_callee())
}

fn abs_call(
    function: &str,
    args: &[IRNode],
    env: &mut AbsEnv,
    ctx: &AbsCtx,
) -> Option<Abs> {
    match function {
        "List" => {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(abs_eval(arg, env, ctx)?);
            }
            Some(Abs::List(values))
        }
        "len" if args.len() == 1 => abs_len(abs_eval(&args[0], env, ctx)?),
        // F2 (Phase 100, D4a): the conversion — the
        // mirror of `sym_int_to_str` (see `abs_int_to_str`).
        "int_to_str" if args.len() == 1 => {
            abs_int_to_str(abs_eval(&args[0], env, ctx)?)
        }
        // The Result/Option constructors (Phase 76), with
        // the real evaluator's literal tags.
        "Ok" if args.len() == 1 => Some(Abs::Result {
            ok: AbsTag::True,
            ok_payload: Some(Box::new(abs_eval(&args[0], env, ctx)?)),
            err_payload: None,
        }),
        "Err" if args.len() == 1 => Some(Abs::Result {
            ok: AbsTag::False,
            ok_payload: None,
            err_payload: Some(Box::new(abs_eval(&args[0], env, ctx)?)),
        }),
        "Some" if args.len() == 1 => Some(Abs::Option {
            some: AbsTag::True,
            payload: Some(Box::new(abs_eval(&args[0], env, ctx)?)),
        }),
        "None" if args.is_empty() => Some(Abs::Option {
            some: AbsTag::False,
            payload: None,
        }),
        _ => {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(abs_eval(arg, env, ctx)?);
            }
            // Phase 133 (G4 D6a): a call naming a
            // function-typed parameter encodes as UF
            // application, under the mirror of the real
            // arm's condition (`sym_call`): no spec of
            // this name in the index, and the environment
            // binds the name to a UF. When it engages, its
            // outcome is final.
            if ctx.index.get(function).is_none() {
                if let Some(func) = env.get(function) {
                    if matches!(func, Abs::Func { .. }) {
                        return abs_apply(func, values);
                    }
                }
            }
            abs_invoke(ctx, function, values)
        }
    }
}

// ── Operators (mirror sym_len / sym_index / sym_ite /
//    sym_binop / sym_unary_ir) ─────────────────────────────

fn abs_len(v: Abs) -> Option<Abs> {
    match v {
        // A literal list's length is a Z3 numeral.
        Abs::List(items) => Some(Abs::Int(IntVal::Known(items.len() as i64))),
        // A string's length is a seq.len term, not a numeral.
        Abs::Str(_) => Some(Abs::Int(IntVal::Unknown)),
        // A symbolic list's length is its binding constant,
        // not a numeral.
        Abs::ListLen => Some(Abs::Int(IntVal::Unknown)),
        // A paired binding's length is the same standing
        // term (Phase 139): a binding constant, not a
        // numeral — mirrors `sym_len`'s ArrayLen arm.
        Abs::ArrayLen { .. } => Some(Abs::Int(IntVal::Unknown)),
        _ => None,
    }
}

/// The mirror of `sym_int_to_str` (Phase 100, F2 D4a),
/// case for case: a Known Int folds to the Known Str of
/// its exact Rust `Display` rendering (the real side's
/// `as_i64()` fold); an Unknown Int yields an Unknown
/// Str (the real side's UF application — a Str term that
/// is no literal); every other sort declines on both
/// sides.
fn abs_int_to_str(v: Abs) -> Option<Abs> {
    match v {
        Abs::Int(IntVal::Known(n)) => {
            Some(Abs::Str(StrVal::Known(format!("{}", n))))
        }
        Abs::Int(IntVal::Unknown) => Some(Abs::Str(StrVal::Unknown)),
        _ => None,
    }
}

/// Extract the guard facts from a spec's require rows
/// (Phase 140): the top-level conjuncts that are atomic
/// comparisons between simple terms. Anything else — an
/// implication (conditional, not a fact), a disjunction,
/// a negation, a quantifier, a comparison over a term the
/// domain cannot state — contributes nothing. The real
/// side's solver works from the rows' full terms, so its
/// discharge is a superset of this domain's; the mirror
/// declining where the domain cannot state the fact is
/// the sound direction (see `AbsLen`).
fn guard_facts(spec: &FunctionSpec) -> Vec<AbsFact> {
    let mut out = Vec::new();
    for row in &spec.preconditions {
        collect_facts(row, spec, &mut out);
    }
    out
}

fn collect_facts(c: &Constraint, spec: &FunctionSpec, out: &mut Vec<AbsFact>) {
    match c {
        Constraint::And(parts) => {
            for p in parts {
                collect_facts(p, spec, out);
            }
        }
        Constraint::Expr(e) => collect_expr_facts(e, spec, out),
        Constraint::Eq(a, b) => push_fact(a, AbsCmp::Eq, b, spec, out),
        Constraint::Order(op, a, b) => {
            let cmp = match op {
                OrderOp::LessThan => AbsCmp::Lt,
                OrderOp::LessThanOrEqual => AbsCmp::Le,
                OrderOp::GreaterThan => AbsCmp::Gt,
                OrderOp::GreaterThanOrEqual => AbsCmp::Ge,
            };
            push_fact(a, cmp, b, spec, out);
        }
        _ => {}
    }
}

fn collect_expr_facts(e: &SpecExpr, spec: &FunctionSpec, out: &mut Vec<AbsFact>) {
    match e {
        SpecExpr::BinOp(SpecBinOp::And, a, b) => {
            collect_expr_facts(a, spec, out);
            collect_expr_facts(b, spec, out);
        }
        SpecExpr::BinOp(op, a, b) => {
            let cmp = match op {
                SpecBinOp::Eq => AbsCmp::Eq,
                SpecBinOp::Lt => AbsCmp::Lt,
                SpecBinOp::Le => AbsCmp::Le,
                SpecBinOp::Gt => AbsCmp::Gt,
                SpecBinOp::Ge => AbsCmp::Ge,
                // Neq yields no difference bound (a ≠ fact
                // alone entails no ≥ fact over a free Int —
                // the real side cannot use it alone either).
                _ => return,
            };
            push_fact(a, cmp, b, spec, out);
        }
        _ => {}
    }
}

fn push_fact(a: &SpecExpr, cmp: AbsCmp, b: &SpecExpr, spec: &FunctionSpec, out: &mut Vec<AbsFact>) {
    if let (Some(lhs), Some(rhs)) = (spec_term(a, spec), spec_term(b, spec)) {
        out.push(AbsFact { lhs, cmp, rhs });
    }
}

/// A simple term over a spec expression: a parameter
/// base (an Int parameter, or a `List<Int>` parameter's
/// length) plus a constant offset, closed under ± a
/// constant. Anything wider — a product, a base ± a base,
/// a field's length (no spec-level name) — is not a term
/// the domain can state.
fn spec_term(e: &SpecExpr, spec: &FunctionSpec) -> Option<AbsTerm> {
    match e {
        SpecExpr::IntLit(n) => Some(AbsTerm {
            base: None,
            off: *n,
        }),
        SpecExpr::Var(name) => {
            if spec
                .params
                .iter()
                .any(|p| &p.name == name && matches!(p.ty, TypeRef::Int))
            {
                Some(AbsTerm {
                    base: Some(AbsBase::Var(name.clone())),
                    off: 0,
                })
            } else {
                None
            }
        }
        SpecExpr::Length(inner) => spec_len_term(inner, spec),
        SpecExpr::Call(name, args) if name == "len" && args.len() == 1 => {
            spec_len_term(&args[0], spec)
        }
        SpecExpr::BinOp(op, a, b) => {
            let x = spec_term(a, spec)?;
            let y = spec_term(b, spec)?;
            combine_spec_terms(*op, x, y)
        }
        SpecExpr::UnaryOp(SpecUnaryOp::Neg, inner) => match spec_term(inner, spec)? {
            AbsTerm { base: None, off } => Some(AbsTerm {
                base: None,
                off: -off,
            }),
            _ => None,
        },
        _ => None,
    }
}

fn spec_len_term(inner: &SpecExpr, spec: &FunctionSpec) -> Option<AbsTerm> {
    match inner {
        SpecExpr::Var(name) => {
            let is_int_list = spec.params.iter().any(|p| {
                &p.name == name
                    && matches!(&p.ty, TypeRef::List(el) if matches!(**el, TypeRef::Int))
            });
            if is_int_list {
                Some(AbsTerm {
                    base: Some(AbsBase::LenOf(name.clone())),
                    off: 0,
                })
            } else {
                None
            }
        }
        // A list literal's length is its count (the parser
        // lowers `[a, b]` to a `List` call).
        SpecExpr::Call(name, args) if name == "List" => Some(AbsTerm {
            base: None,
            off: args.len() as i64,
        }),
        _ => None,
    }
}

fn combine_spec_terms(op: SpecBinOp, x: AbsTerm, y: AbsTerm) -> Option<AbsTerm> {
    match op {
        SpecBinOp::Add => {
            if x.base.is_none() {
                Some(AbsTerm {
                    base: y.base,
                    off: x.off + y.off,
                })
            } else if y.base.is_none() {
                Some(AbsTerm {
                    base: x.base,
                    off: x.off + y.off,
                })
            } else {
                None
            }
        }
        SpecBinOp::Sub => {
            // Only a constant subtrahend keeps the term
            // simple; a negated base is not a simple term.
            if y.base.is_none() {
                Some(AbsTerm {
                    base: x.base,
                    off: x.off - y.off,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The mirror's guard discharge (Phase 140): the read at
/// `idx` into a pair of length `len` exists only when the
/// extracted facts entail `0 <= idx < len` in difference
/// logic — the longest-path closure of the facts' bounds.
/// Sound over the integers (each extracted fact is a
/// conjunct of its require row, stated exactly), and a
/// subset of the real side's solver entailment over the
/// same rows.
fn abs_guard_discharged(facts: &[AbsFact], idx: &AbsTerm, len: &AbsTerm) -> bool {
    // Nodes: the constant base (None) plus every base the
    // facts or the goal mention.
    let mut nodes: Vec<Option<AbsBase>> = vec![None];
    for t in facts
        .iter()
        .flat_map(|f| [&f.lhs.base, &f.rhs.base])
        .chain([&idx.base, &len.base])
    {
        if !nodes.contains(t) {
            nodes.push(t.clone());
        }
    }
    let n = nodes.len();
    let pos = |b: &Option<AbsBase>| nodes.iter().position(|x| x == b).unwrap();
    // dist[i][j] = the best known lower bound of
    // val(node_i) − val(node_j); NEG = no bound known.
    const NEG: i64 = i64::MIN / 4;
    let mut dist = vec![vec![NEG; n]; n];
    for (i, row) in dist.iter_mut().enumerate() {
        row[i] = 0;
    }
    for f in facts {
        let (a, b) = (pos(&f.lhs.base), pos(&f.rhs.base));
        let (oa, ob) = (f.lhs.off, f.rhs.off);
        // value(lhs) = val(a) + oa, value(rhs) = val(b) + ob.
        let mut bound = |i: usize, j: usize, w: i64, dist: &mut Vec<Vec<i64>>| {
            if w > dist[i][j] {
                dist[i][j] = w;
            }
        };
        match f.cmp {
            AbsCmp::Ge => bound(a, b, ob - oa, &mut dist),
            AbsCmp::Gt => bound(a, b, ob - oa + 1, &mut dist),
            AbsCmp::Le => bound(b, a, oa - ob, &mut dist),
            AbsCmp::Lt => bound(b, a, oa - ob + 1, &mut dist),
            AbsCmp::Eq => {
                bound(a, b, ob - oa, &mut dist);
                bound(b, a, oa - ob, &mut dist);
            }
        }
    }
    // Longest-path closure (the node count is the spec's
    // parameter count plus a constant — tiny).
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                if dist[i][k] != NEG && dist[k][j] != NEG {
                    let via = dist[i][k].saturating_add(dist[k][j]);
                    if via > dist[i][j] {
                        dist[i][j] = via;
                    }
                }
            }
        }
    }
    // Inconsistent facts (a positive cycle) entail
    // everything — the solver side discharges vacuously
    // under an unsatisfiable guard set too, and the
    // verdict layer declines such specs whole on both
    // builds before encodability is consulted.
    if (0..n).any(|i| dist[i][i] > 0) {
        return true;
    }
    let zero = pos(&None);
    let ii = pos(&idx.base);
    let ll = pos(&len.base);
    // 0 <= idx  ⟺  val(ii) − val(0) >= −idx.off
    if dist[ii][zero] < -idx.off {
        return false;
    }
    // idx < len  ⟺  val(ll) − val(ii) >= idx.off − len.off + 1
    if dist[ll][ii] < idx.off - len.off + 1 {
        return false;
    }
    true
}

/// The guard-domain form of a pair's abstract length.
fn abs_len_form(len: &AbsLen) -> Option<AbsTerm> {
    match len {
        AbsLen::Known(m) => Some(AbsTerm {
            base: None,
            off: *m,
        }),
        AbsLen::Param(name, off) => Some(AbsTerm {
            base: Some(AbsBase::LenOf(name.clone())),
            off: *off,
        }),
        AbsLen::Unknown => None,
    }
}

/// The guard-domain form of `len(e)` on the IR side: the
/// length form of the value `e` denotes, carried by the
/// value itself (a parameter pair's form survives Lets,
/// merges that agree, and inlining, exactly as the real
/// length term does).
fn abs_len_term(node: &IRNode, env: &mut AbsEnv, ctx: &AbsCtx) -> Option<AbsTerm> {
    match node {
        IRNode::Var(name) => match env.get(name) {
            Some(Abs::ArrayLen { len }) => abs_len_form(len),
            Some(Abs::List(items)) => Some(AbsTerm {
                base: None,
                off: items.len() as i64,
            }),
            _ => None,
        },
        IRNode::Collection { elements, .. } => Some(AbsTerm {
            base: None,
            off: elements.len() as i64,
        }),
        IRNode::Call { function, args } if function == "List" => Some(AbsTerm {
            base: None,
            off: args.len() as i64,
        }),
        _ => match abs_eval(node, env, ctx) {
            Some(Abs::List(items)) => Some(AbsTerm {
                base: None,
                off: items.len() as i64,
            }),
            _ => None,
        },
    }
}

/// The guard-domain form of an index expression on the IR
/// side: a parameter base (an unshadowed Int parameter,
/// or a length form) plus a constant offset, closed under
/// ± a constant; failing that, the constant form when the
/// evaluated value is a known numeral (a computed literal:
/// `1 + 1`, a constant read from a literal list, …).
/// Anything wider is not a term the domain can state, and
/// the read declines on the mirror side.
fn abs_index_term(node: &IRNode, env: &mut AbsEnv, ctx: &AbsCtx) -> Option<AbsTerm> {
    match node {
        IRNode::Var(name) => match env.get(name) {
            Some(Abs::Int(IntVal::Known(n))) => Some(AbsTerm {
                base: None,
                off: *n,
            }),
            Some(Abs::Int(IntVal::Unknown)) if ctx.params.contains(name) => Some(AbsTerm {
                base: Some(AbsBase::Var(name.clone())),
                off: 0,
            }),
            _ => None,
        },
        IRNode::Call { function, args } if function == "len" && args.len() == 1 => {
            abs_len_term(&args[0], env, ctx)
        }
        IRNode::UnaryOp(IRUnaryOp::Len, inner) => abs_len_term(inner, env, ctx),
        IRNode::BinOp(op, l, r) if matches!(op, IRBinOp::Add | IRBinOp::Sub) => {
            let x = abs_index_term(l, env, ctx)?;
            let y = abs_index_term(r, env, ctx)?;
            match op {
                IRBinOp::Add => {
                    if x.base.is_none() {
                        Some(AbsTerm {
                            base: y.base,
                            off: x.off + y.off,
                        })
                    } else if y.base.is_none() {
                        Some(AbsTerm {
                            base: x.base,
                            off: x.off + y.off,
                        })
                    } else {
                        None
                    }
                }
                _ => {
                    if y.base.is_none() {
                        Some(AbsTerm {
                            base: x.base,
                            off: x.off - y.off,
                        })
                    } else {
                        None
                    }
                }
            }
        }
        IRNode::UnaryOp(IRUnaryOp::Neg, inner) => match abs_index_term(inner, env, ctx)? {
            AbsTerm { base: None, off } => Some(AbsTerm {
                base: None,
                off: -off,
            }),
            _ => None,
        },
        _ => match abs_eval(node, env, ctx) {
            Some(Abs::Int(IntVal::Known(n))) => Some(AbsTerm {
                base: None,
                off: n,
            }),
            _ => None,
        },
    }
}

fn abs_index(coll: Abs, idx: Abs) -> Option<Abs> {
    match (coll, idx) {
        (Abs::List(items), Abs::Int(IntVal::Known(n))) => {
            if n < 0 {
                return None;
            }
            items.into_iter().nth(n as usize)
        }
        // The String-index→Char bridge (Phase 76), mirroring
        // the real evaluator case for case: a CONSTANT index
        // into a KNOWN literal string is exact — the char's
        // scalar, or unencodable out of bounds; every other
        // string/index pair is the bridge's CharStr form.
        (Abs::Str(StrVal::Known(s)), Abs::Int(IntVal::Known(n))) => {
            if n < 0 {
                return None;
            }
            s.chars()
                .nth(n as usize)
                .map(|c| Abs::Char(IntVal::Known(c as u32 as i64)))
        }
        (Abs::Str(_), Abs::Int(_)) => Some(Abs::CharStr),
        _ => None,
    }
}

/// Merge a payload slot across an `ite`, mirroring the
/// real evaluator's `sym_ite_payload`: a one-sided payload
/// is the merged payload (it is observed only under its
/// side's tag).
fn abs_ite_payload(a: Option<Box<Abs>>, b: Option<Box<Abs>>) -> Option<Option<Box<Abs>>> {
    match (a, b) {
        (None, None) => Some(None),
        (Some(x), None) => Some(Some(x)),
        (None, Some(y)) => Some(Some(y)),
        (Some(x), Some(y)) => abs_ite(*x, *y).map(|v| Some(Box::new(v))),
    }
}

fn abs_ite(a: Abs, b: Abs) -> Option<Abs> {
    match (a, b) {
        (Abs::Int(_), Abs::Int(_)) => Some(Abs::Int(IntVal::Unknown)),
        (Abs::Bool, Abs::Bool) => Some(Abs::Bool),
        (Abs::Str(_), Abs::Str(_)) => Some(Abs::Str(StrVal::Unknown)),
        // An ite of Chars is an application term, never a
        // numeral — even between two literals.
        (Abs::Char(_), Abs::Char(_)) => Some(Abs::Char(IntVal::Unknown)),
        (Abs::CharStr, Abs::CharStr) => Some(Abs::CharStr),
        (Abs::ListLen, Abs::ListLen) => Some(Abs::ListLen),
        // Two paired bindings merge as a pair (Phase 139,
        // mirroring sym_ite's §2.11 arm); the merged
        // length keeps the form both sides share
        // (Phase 140 — see `AbsLen::merged`).
        (Abs::ArrayLen { len: la }, Abs::ArrayLen { len: lb }) => Some(Abs::ArrayLen {
            len: la.merged(&lb),
        }),
        // A pair meets a concrete list through the §2.5
        // bridge, mirroring `sym_pair_of_concrete`: the
        // concrete side crosses exactly when every
        // element is an Int (an empty list crosses
        // vacuously); any other element sort declines,
        // as the real crossing declines. The crossed
        // length form is the literal's count.
        (Abs::ArrayLen { len }, Abs::List(items)) | (Abs::List(items), Abs::ArrayLen { len }) => {
            if items.iter().all(|it| matches!(it, Abs::Int(_))) {
                Some(Abs::ArrayLen {
                    len: len.merged(&AbsLen::Known(items.len() as i64)),
                })
            } else {
                None
            }
        }
        // Two concrete lists merge elementwise (Phase 91),
        // mirroring sym_ite: equal lengths only.
        (Abs::List(x), Abs::List(y)) => {
            if x.len() != y.len() {
                return None;
            }
            let mut out = Vec::with_capacity(x.len());
            for (u, v) in x.into_iter().zip(y) {
                out.push(abs_ite(u, v)?);
            }
            Some(Abs::List(out))
        }
        (Abs::Opaque, Abs::Opaque) => Some(Abs::Opaque),
        (
            Abs::Record {
                type_name: ta,
                fields: fa,
            },
            Abs::Record {
                type_name: tb,
                fields: fb,
            },
        ) => {
            if ta != tb || fa.len() != fb.len() {
                return None;
            }
            let mut merged = Vec::with_capacity(fa.len());
            for (name, va) in fa {
                let vb = fb.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone())?;
                merged.push((name, abs_ite(va, vb)?));
            }
            Some(Abs::Record {
                type_name: ta,
                fields: merged,
            })
        }
        (
            Abs::Option {
                some: ta,
                payload: pa,
            },
            Abs::Option {
                some: tb,
                payload: pb,
            },
        ) => Some(Abs::Option {
            some: ta.merge(tb),
            payload: abs_ite_payload(pa, pb)?,
        }),
        (
            Abs::Result {
                ok: ta,
                ok_payload: oa,
                err_payload: ea,
            },
            Abs::Result {
                ok: tb,
                ok_payload: ob,
                err_payload: eb,
            },
        ) => Some(Abs::Result {
            ok: ta.merge(tb),
            ok_payload: abs_ite_payload(oa, ob)?,
            err_payload: abs_ite_payload(ea, eb)?,
        }),
        _ => None,
    }
}

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

/// Numeral tracking: a Z3 term is a readable numeral
/// (`as_i64` succeeds) only if it IS one — `Int::from_i64`
/// (integer literals, and `len` of a literal list, which
/// the real evaluator also builds with `from_i64`). Z3's
/// arithmetic constructors build application terms, NOT
/// folded numerals: `1 * 1` is the term `(* 1 1)`, and
/// `Z3_get_numeral_int64` does not evaluate it. (Measured
/// by the cross-check below: `Index([10,20], 1*1)` is
/// unencodable in the real evaluator.) So every arithmetic
/// result — Add/Sub/Mul/Div/Mod/Neg/Abs — is Unknown here,
/// and Known values arise only from literals, `len` of a
/// list, and values flowing unchanged through
/// Let/Var/Collection/Match/inlining.
fn no_fold() -> IntVal {
    IntVal::Unknown
}

fn abs_binop(kind: BinKind, l: Abs, r: Abs) -> Option<Abs> {
    use BinKind::*;
    match (kind, l, r) {
        (Add, Abs::Int(_), Abs::Int(_)) => Some(Abs::Int(no_fold())),
        (Sub, Abs::Int(_), Abs::Int(_)) => Some(Abs::Int(no_fold())),
        (Mul, Abs::Int(_), Abs::Int(_)) => Some(Abs::Int(no_fold())),
        (Div, Abs::Int(_), Abs::Int(_)) => Some(Abs::Int(no_fold())),
        (Mod, Abs::Int(_), Abs::Int(_)) => Some(Abs::Int(no_fold())),
        (And, Abs::Bool, Abs::Bool) => Some(Abs::Bool),
        (Or, Abs::Bool, Abs::Bool) => Some(Abs::Bool),
        (Xor, Abs::Bool, Abs::Bool) => Some(Abs::Bool),
        (Eq, Abs::Int(_), Abs::Int(_)) => Some(Abs::Bool),
        (Eq, Abs::Bool, Abs::Bool) => Some(Abs::Bool),
        (Eq, Abs::Str(_), Abs::Str(_)) => Some(Abs::Bool),
        (Neq, Abs::Int(_), Abs::Int(_)) => Some(Abs::Bool),
        (Neq, Abs::Bool, Abs::Bool) => Some(Abs::Bool),
        (Neq, Abs::Str(_), Abs::Str(_)) => Some(Abs::Bool),
        (Lt, Abs::Int(_), Abs::Int(_)) => Some(Abs::Bool),
        (Le, Abs::Int(_), Abs::Int(_)) => Some(Abs::Bool),
        (Gt, Abs::Int(_), Abs::Int(_)) => Some(Abs::Bool),
        (Ge, Abs::Int(_), Abs::Int(_)) => Some(Abs::Bool),
        (Lt, Abs::Str(_), Abs::Str(_)) => Some(Abs::Bool),
        (Le, Abs::Str(_), Abs::Str(_)) => Some(Abs::Bool),
        (Gt, Abs::Str(_), Abs::Str(_)) => Some(Abs::Bool),
        (Ge, Abs::Str(_), Abs::Str(_)) => Some(Abs::Bool),
        (Concat, Abs::Str(_), Abs::Str(_)) => Some(Abs::Str(StrVal::Unknown)),
        (Concat, Abs::List(mut a), Abs::List(b)) => {
            a.extend(b);
            Some(Abs::List(a))
        }
        (Append, Abs::List(mut a), item) => {
            a.push(item);
            Some(Abs::List(a))
        }
        // Append over a length-only list (Phase 91): the
        // length grows by exactly one, whatever the item.
        (Append, Abs::ListLen, _) => Some(Abs::ListLen),
        // Append over a paired binding (Phase 139):
        // §2.6's store form, mirroring the real arm's
        // item discipline — the item must be an Int; the
        // pair survives with its length form grown by one
        // (Phase 140). Any other item declines, as the
        // real store declines.
        (Append, Abs::ArrayLen { len }, Abs::Int(_)) => Some(Abs::ArrayLen {
            len: len.grown(),
        }),
        // Equality over paired bindings (Phase 140,
        // §2.4): the shape exists exactly when the real
        // pointwise form does — pair vs pair, and pair vs
        // concrete list through the §2.5 bridge (every
        // element an Int; an empty list crosses
        // vacuously). Neq is the negation, same shapes.
        // A pair against a length-only binding has no arm
        // on either side.
        (Eq, Abs::ArrayLen { .. }, Abs::ArrayLen { .. }) => Some(Abs::Bool),
        (Neq, Abs::ArrayLen { .. }, Abs::ArrayLen { .. }) => Some(Abs::Bool),
        (Eq, Abs::ArrayLen { .. }, Abs::List(items))
        | (Eq, Abs::List(items), Abs::ArrayLen { .. }) => {
            if items.iter().all(|it| matches!(it, Abs::Int(_))) {
                Some(Abs::Bool)
            } else {
                None
            }
        }
        (Neq, Abs::ArrayLen { .. }, Abs::List(items))
        | (Neq, Abs::List(items), Abs::ArrayLen { .. }) => {
            if items.iter().all(|it| matches!(it, Abs::Int(_))) {
                Some(Abs::Bool)
            } else {
                None
            }
        }
        // Concat of two paired bindings (Phase 140,
        // §2.6): declines elementwise, binds as the
        // length-only form with the exact sum length —
        // mirroring the real arms, the bridge crossing
        // included.
        (Concat, Abs::ArrayLen { .. }, Abs::ArrayLen { .. }) => Some(Abs::ListLen),
        (Concat, Abs::ArrayLen { .. }, Abs::List(items))
        | (Concat, Abs::List(items), Abs::ArrayLen { .. }) => {
            if items.iter().all(|it| matches!(it, Abs::Int(_))) {
                Some(Abs::ListLen)
            } else {
                None
            }
        }
        // Append of an indexed char onto a string (Phase
        // 91): a concatenation term, never a known literal.
        (Append, Abs::Str(_), Abs::CharStr) => Some(Abs::Str(StrVal::Unknown)),
        // Char comparisons (Phase 76): all six, on scalars.
        (Eq, Abs::Char(_), Abs::Char(_)) => Some(Abs::Bool),
        (Neq, Abs::Char(_), Abs::Char(_)) => Some(Abs::Bool),
        (Lt, Abs::Char(_), Abs::Char(_)) => Some(Abs::Bool),
        (Le, Abs::Char(_), Abs::Char(_)) => Some(Abs::Bool),
        (Gt, Abs::Char(_), Abs::Char(_)) => Some(Abs::Bool),
        (Ge, Abs::Char(_), Abs::Char(_)) => Some(Abs::Bool),
        // Indexed chars among themselves: all six.
        (Eq, Abs::CharStr, Abs::CharStr) => Some(Abs::Bool),
        (Neq, Abs::CharStr, Abs::CharStr) => Some(Abs::Bool),
        (Lt, Abs::CharStr, Abs::CharStr) => Some(Abs::Bool),
        (Le, Abs::CharStr, Abs::CharStr) => Some(Abs::Bool),
        (Gt, Abs::CharStr, Abs::CharStr) => Some(Abs::Bool),
        (Ge, Abs::CharStr, Abs::CharStr) => Some(Abs::Bool),
        // Indexed char against a CONSTANT Char (a numeral):
        // all six. Against a symbolic Char the real
        // evaluator has no conversion — unencodable, and
        // no arm below matches it.
        (Eq, Abs::CharStr, Abs::Char(IntVal::Known(_))) => Some(Abs::Bool),
        (Neq, Abs::CharStr, Abs::Char(IntVal::Known(_))) => Some(Abs::Bool),
        (Lt, Abs::CharStr, Abs::Char(IntVal::Known(_))) => Some(Abs::Bool),
        (Le, Abs::CharStr, Abs::Char(IntVal::Known(_))) => Some(Abs::Bool),
        (Gt, Abs::CharStr, Abs::Char(IntVal::Known(_))) => Some(Abs::Bool),
        (Ge, Abs::CharStr, Abs::Char(IntVal::Known(_))) => Some(Abs::Bool),
        (Eq, Abs::Char(IntVal::Known(_)), Abs::CharStr) => Some(Abs::Bool),
        (Neq, Abs::Char(IntVal::Known(_)), Abs::CharStr) => Some(Abs::Bool),
        (Lt, Abs::Char(IntVal::Known(_)), Abs::CharStr) => Some(Abs::Bool),
        (Le, Abs::Char(IntVal::Known(_)), Abs::CharStr) => Some(Abs::Bool),
        (Gt, Abs::Char(IntVal::Known(_)), Abs::CharStr) => Some(Abs::Bool),
        (Ge, Abs::Char(IntVal::Known(_)), Abs::CharStr) => Some(Abs::Bool),
        // Record / Option / Result equality: the helpers
        // mirror the real case analysis (tag classes and
        // payload presence decide what must encode).
        (
            Eq,
            Abs::Record {
                type_name: an,
                fields: af,
            },
            Abs::Record {
                type_name: bn,
                fields: bf,
            },
        ) => abs_record_eq(&an, &af, &bn, &bf),
        (
            Neq,
            Abs::Record {
                type_name: an,
                fields: af,
            },
            Abs::Record {
                type_name: bn,
                fields: bf,
            },
        ) => abs_record_eq(&an, &af, &bn, &bf),
        (
            Eq,
            Abs::Option {
                some: at,
                payload: ap,
            },
            Abs::Option {
                some: bt,
                payload: bp,
            },
        ) => abs_option_eq(at, &ap, bt, &bp),
        (
            Neq,
            Abs::Option {
                some: at,
                payload: ap,
            },
            Abs::Option {
                some: bt,
                payload: bp,
            },
        ) => abs_option_eq(at, &ap, bt, &bp),
        (
            Eq,
            Abs::Result {
                ok: at,
                ok_payload: ao,
                err_payload: ae,
            },
            Abs::Result {
                ok: bt,
                ok_payload: bo,
                err_payload: be,
            },
        ) => abs_result_eq(at, &ao, &ae, bt, &bo, &be),
        (
            Neq,
            Abs::Result {
                ok: at,
                ok_payload: ao,
                err_payload: ae,
            },
            Abs::Result {
                ok: bt,
                ok_payload: bo,
                err_payload: be,
            },
        ) => abs_result_eq(at, &ao, &ae, bt, &bo, &be),
        _ => None,
    }
}

/// Whether an equality between two abstract values
/// encodes — the `Eq` row of `abs_binop` as a predicate,
/// for the composite equality helpers.
fn abs_eq(a: &Abs, b: &Abs) -> bool {
    abs_binop(BinKind::Eq, a.clone(), b.clone()).is_some()
}

/// Mirror of `sym_record_eq`'s Some/None structure:
/// different declared types are provably unequal (Bool);
/// one type with mismatched field sets is unencodable;
/// otherwise every field's equality must encode.
fn abs_record_eq(
    a_name: &str,
    a_fields: &[(String, Abs)],
    b_name: &str,
    b_fields: &[(String, Abs)],
) -> Option<Abs> {
    if a_name != b_name {
        return Some(Abs::Bool);
    }
    if a_fields.len() != b_fields.len() {
        return None;
    }
    for (name, va) in a_fields {
        let vb = b_fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)?;
        if !abs_eq(va, vb) {
            return None;
        }
    }
    Some(Abs::Bool)
}

/// Mirror of `sym_option_eq`: a literal-None side makes
/// the equality tag-only; otherwise both payloads must be
/// present and their equality must encode.
fn abs_option_eq(
    a_some: AbsTag,
    a_pay: &Option<Box<Abs>>,
    b_some: AbsTag,
    b_pay: &Option<Box<Abs>>,
) -> Option<Abs> {
    if a_some == AbsTag::False || b_some == AbsTag::False {
        return Some(Abs::Bool);
    }
    let pa = a_pay.as_ref()?;
    let pb = b_pay.as_ref()?;
    if abs_eq(pa, pb) {
        Some(Abs::Bool)
    } else {
        None
    }
}

/// Mirror of `sym_result_eq`'s tag case analysis: which
/// payload slots are observed is decided by the tag
/// classes, and exactly those slots' equalities must
/// encode (with both payloads present).
fn abs_result_eq(
    a_ok: AbsTag,
    a_okp: &Option<Box<Abs>>,
    a_errp: &Option<Box<Abs>>,
    b_ok: AbsTag,
    b_okp: &Option<Box<Abs>>,
    b_errp: &Option<Box<Abs>>,
) -> Option<Abs> {
    use AbsTag::*;
    let slot = |x: &Option<Box<Abs>>, y: &Option<Box<Abs>>| -> bool {
        match (x, y) {
            (Some(a), Some(b)) => abs_eq(a, b),
            _ => false,
        }
    };
    let encodable = match (a_ok, b_ok) {
        (False, False) => slot(a_errp, b_errp),
        (True, True) => slot(a_okp, b_okp),
        (True, False) | (False, True) => true,
        (True, Free) | (Free, True) => slot(a_okp, b_okp),
        (False, Free) | (Free, False) => slot(a_errp, b_errp),
        (Free, Free) => slot(a_okp, b_okp) && slot(a_errp, b_errp),
    };
    if encodable {
        Some(Abs::Bool)
    } else {
        None
    }
}

fn abs_binop_ir(op: IRBinOp, l: Abs, r: Abs) -> Option<Abs> {
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
        // Z3's Int has no shifts; Merge is meaningless here.
        IRBinOp::Shl | IRBinOp::Shr | IRBinOp::Merge => return None,
    };
    abs_binop(kind, l, r)
}

fn abs_unary_ir(op: IRUnaryOp, v: Abs) -> Option<Abs> {
    match op {
        // Neg is z3_sub(0, v): an application term, never a
        // numeral (see no_fold).
        IRUnaryOp::Neg => match v {
            Abs::Int(_) => Some(Abs::Int(no_fold())),
            _ => None,
        },
        IRUnaryOp::Not => match v {
            Abs::Bool => Some(Abs::Bool),
            _ => None,
        },
        // Abs builds an ite — never a numeral.
        IRUnaryOp::Abs => match v {
            Abs::Int(_) => Some(Abs::Int(IntVal::Unknown)),
            _ => None,
        },
        IRUnaryOp::Len => abs_len(v),
        // Clone is the identity (Phase 91): the value —
        // and every fact tracked about it — is unchanged.
        IRUnaryOp::Clone => Some(v),
        _ => None,
    }
}

// ── Public entry ───────────────────────────────────────────

/// The pure counterpart of `verifier::body_is_encodable_with`
/// (see the module docs): same question, same answer, no
/// solver. In a `z3-support` build the verifier keeps using
/// its symbolic evaluator; this predicate serves the builds
/// where no solver exists.
pub fn body_is_encodable_pure(spec: &FunctionSpec, body: &IRNode, index: &SpecIndex) -> bool {
    let mut env = param_env(spec, index);
    abs_eval(body, &mut env, &AbsCtx::for_spec(index, spec)).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthesis::engine::CollectionKind;

    fn spec(params: Vec<(&str, TypeRef)>) -> FunctionSpec {
        FunctionSpec {
            name: "probe".into(),
            doc: None,
            generics: vec![],
            params: params
                .into_iter()
                .map(|(name, ty)| crate::spec::ast::Param {
                    name: name.into(),
                    ty,
                    annotations: HashMap::new(),
                })
                .collect(),
            return_type: TypeRef::Bool,
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: HashMap::new(),
        }
    }

    fn int(n: i64) -> IRNode {
        IRNode::Literal(IRLiteral::Int(n))
    }
    fn var(name: &str) -> IRNode {
        IRNode::Var(name.into())
    }
    fn binop(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
        IRNode::BinOp(op, Box::new(l), Box::new(r))
    }
    fn list(items: Vec<IRNode>) -> IRNode {
        IRNode::Collection {
            kind: CollectionKind::List,
            elements: items,
        }
    }
    fn index(coll: IRNode, idx: IRNode) -> IRNode {
        IRNode::Index(Box::new(coll), Box::new(idx))
    }

    fn encodable(spec: &FunctionSpec, body: &IRNode) -> bool {
        body_is_encodable_pure(spec, body, &SpecIndex::empty())
    }

    #[test]
    fn literal_and_param_arithmetic_are_encodable() {
        let s = spec(vec![("x", TypeRef::Int)]);
        assert!(encodable(&s, &binop(IRBinOp::Add, var("x"), int(1))));
        assert!(encodable(&s, &int(3)));
    }

    #[test]
    fn int_to_str_shapes_are_encodable() {
        // Phase 100 (F2): the mirror's `int_to_str` arm —
        // a Known Int folds to a Known Str, an Unknown Int
        // to an Unknown Str (the UF's abstract shadow);
        // wrong sort and wrong arity decline, mirroring
        // `sym_int_to_str` case for case.
        let s = spec(vec![("x", TypeRef::Int), ("t", TypeRef::String)]);
        let call = |args: Vec<IRNode>| IRNode::Call {
            function: "int_to_str".into(),
            args,
        };
        assert!(encodable(&s, &call(vec![int(42)])));
        assert!(encodable(&s, &call(vec![int(-42)])));
        assert!(encodable(&s, &call(vec![var("x")])));
        assert!(!encodable(&s, &call(vec![var("t")])));
        assert!(!encodable(&s, &call(vec![int(1), int(2)])));
        assert!(!encodable(&s, &call(vec![])));
    }

    #[test]
    fn holes_and_loops_are_not_encodable() {
        let s = spec(vec![("x", TypeRef::Int)]);
        assert!(!encodable(&s, &IRNode::Hole { id: 0, expected_type: None }));
        assert!(!encodable(
            &s,
            &IRNode::While {
                condition: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
                body: Box::new(int(1)),
                invariant: None,
            }
        ));
    }

    #[test]
    fn index_needs_a_constant_index_into_a_literal_list() {
        let s = spec(vec![("x", TypeRef::Int)]);
        // Literal index: encodable.
        assert!(encodable(&s, &index(list(vec![int(10), int(20)]), int(1))));
        // Computed-constant index: NOT encodable — Z3's
        // arithmetic constructors build application terms,
        // not numerals, so `as_i64` cannot read `0 + 1`
        // (measured by the cross-check below).
        assert!(!encodable(
            &s,
            &index(list(vec![int(10), int(20)]), binop(IRBinOp::Add, int(0), int(1)))
        ));
        // len of a literal list is a numeral: encodable
        // (index 2 = len of the 2-element list, in range).
        assert!(encodable(
            &s,
            &index(
                list(vec![int(10), int(20), int(30)]),
                IRNode::UnaryOp(
                    IRUnaryOp::Len,
                    Box::new(list(vec![int(10), int(20)]))
                )
            )
        ));
        // A parameter index is symbolic: NOT encodable.
        assert!(!encodable(&s, &index(list(vec![int(10), int(20)]), var("x"))));
        // Out-of-range constant: NOT encodable.
        assert!(!encodable(&s, &index(list(vec![int(10)]), int(5))));
    }

    #[test]
    fn sort_discipline_is_strict() {
        let s = spec(vec![("x", TypeRef::Int), ("b", TypeRef::Bool)]);
        // Int + Bool: unencodable.
        assert!(!encodable(&s, &binop(IRBinOp::Add, var("x"), var("b"))));
        // If on an Int condition: unencodable.
        assert!(!encodable(
            &s,
            &IRNode::If {
                condition: Box::new(var("x")),
                then_branch: Box::new(int(1)),
                else_branch: Box::new(int(2)),
            }
        ));
        // If with mismatched branch sorts (Int vs Bool): unencodable.
        assert!(!encodable(
            &s,
            &IRNode::If {
                condition: Box::new(var("b")),
                then_branch: Box::new(int(1)),
                else_branch: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
            }
        ));
    }

    /// THE CROSS-CHECK (z3-support builds only): the pure
    /// mirror must agree with the real symbolic predicate on
    /// every body in a harvested corpus — the example
    /// modules' synthesized pools (hundreds of bodies of
    /// every shape the engine produces) plus hand-built
    /// corner bodies aimed at the value-sensitive spots
    /// (constant folding, len-indexing, string indexing,
    /// match fall-through). Phase 76 extends the corpus
    /// with the M1 shapes: the tokenizer module under a
    /// declarations-carrying index (record binding active),
    /// its classifier pools (Char trees), hand-built
    /// record/constructor/bridge bodies, and the bridge's
    /// named boundary (an indexed char compared against a
    /// symbolic Char parameter — unencodable on both sides).
    /// A disagreement here is a drift bug in exactly one of
    /// the two implementations.
    #[cfg(feature = "z3-support")]
    #[test]
    fn pure_mirror_agrees_with_symbolic_predicate() {
        use crate::verify::verifier::body_is_encodable_with;

        let root = env!("CARGO_MANIFEST_DIR");
        let mut checked = 0usize;
        let mut bodies: Vec<(FunctionSpec, IRNode, SpecIndex)> = Vec::new();

        // Harvested pools: every candidate body the engine
        // produces for the validator + printer specs.
        for file in ["validator", "printer"] {
            let source =
                std::fs::read_to_string(format!("{}/examples/self_host/{}.morph", root, file))
                    .unwrap();
            let parsed = crate::spec::parser::parse(&source).unwrap();
            let checked_module = crate::spec::typeck::check(parsed).unwrap();
            let functions = checked_module.spec.functions.clone();
            let index = SpecIndex::from_specs(&functions);
            for spec in &functions {
                let candidates =
                    crate::synthesis::engine::synthesize_with_module(spec, 1, &functions)
                        .expect("synthesis runs");
                for c in candidates {
                    bodies.push((spec.clone(), c.body, index.clone()));
                }
            }
        }

        // Hand-built corners against a two-param spec.
        let s = spec(vec![("x", TypeRef::Int), ("t", TypeRef::String)]);
        let idx = SpecIndex::empty();
        let corners: Vec<IRNode> = vec![
            index(list(vec![int(10), int(20)]), binop(IRBinOp::Mul, int(1), int(1))),
            index(list(vec![int(10), int(20)]), binop(IRBinOp::Div, int(3), int(2))),
            index(list(vec![int(10), int(20)]), binop(IRBinOp::Div, int(1), int(0))),
            index(
                list(vec![int(10), int(20)]),
                IRNode::UnaryOp(IRUnaryOp::Neg, Box::new(int(-1))),
            ),
            index(
                IRNode::Literal(IRLiteral::String("abc".into())),
                int(2),
            ),
            index(
                IRNode::Literal(IRLiteral::String("abc".into())),
                int(3),
            ),
            index(var("t"), int(0)),
            binop(IRBinOp::Shl, var("x"), int(1)),
            IRNode::Match {
                scrutinee: Box::new(var("x")),
                cases: vec![(
                    IRPattern::Literal(IRLiteral::Int(1)),
                    IRNode::Literal(IRLiteral::Bool(true)),
                )],
            },
            IRNode::Match {
                scrutinee: Box::new(var("x")),
                cases: vec![
                    (
                        IRPattern::Literal(IRLiteral::Int(1)),
                        IRNode::Literal(IRLiteral::Bool(true)),
                    ),
                    (IRPattern::Wildcard, IRNode::Literal(IRLiteral::Bool(false))),
                ],
            },
            // Phase 100 (F2): the conversion shapes — the
            // fold, the UF shadow, the computed-argument
            // boundary (an Add term is no numeral on either
            // side), and the two declines.
            IRNode::Call {
                function: "int_to_str".into(),
                args: vec![int(42)],
            },
            IRNode::Call {
                function: "int_to_str".into(),
                args: vec![var("x")],
            },
            IRNode::Call {
                function: "int_to_str".into(),
                args: vec![binop(IRBinOp::Add, int(40), int(2))],
            },
            IRNode::Call {
                function: "int_to_str".into(),
                args: vec![var("t")],
            },
            IRNode::Call {
                function: "int_to_str".into(),
                args: vec![int(1), int(2)],
            },
        ];
        for body in corners {
            bodies.push((s.clone(), body, idx.clone()));
        }

        // Phase 76: the tokenizer module under an index
        // that carries its type declarations, so record
        // parameters bind as records on both sides.
        let field = |base: IRNode, name: &str| IRNode::FieldAccess {
            base: Box::new(base),
            field: name.into(),
        };
        let call = |name: &str, args: Vec<IRNode>| IRNode::Call {
            function: name.into(),
            args,
        };
        let chr = |c: char| IRNode::Literal(IRLiteral::Char(c));
        let boolean = |b: bool| IRNode::Literal(IRLiteral::Bool(b));
        {
            let source = std::fs::read_to_string(format!(
                "{}/examples/functional_tokenizer.morph",
                root
            ))
            .unwrap();
            let parsed = crate::spec::parser::parse(&source).unwrap();
            let checked_module = crate::spec::typeck::check(parsed).unwrap();
            let module = checked_module.spec.clone();
            let tok_index = SpecIndex::from_module(&module);
            let functions = module.functions.clone();

            // The classifier pools: Char-comparison trees.
            for spec in &functions {
                if matches!(
                    spec.name.as_str(),
                    "is_whitespace" | "is_ident_start" | "is_ident_continue"
                ) {
                    let candidates =
                        crate::synthesis::engine::synthesize_with_module(
                            spec, 1, &functions,
                        )
                        .expect("synthesis runs");
                    for c in candidates {
                        bodies.push((spec.clone(), c.body, tok_index.clone()));
                    }
                }
            }

            let spec_of = |name: &str| {
                functions
                    .iter()
                    .find(|f| f.name == name)
                    .unwrap()
                    .clone()
            };
            let peek = spec_of("peek");
            let next_char = spec_of("next_char");
            let tokenize_all = spec_of("tokenize_all");

            // peek's mirror body: If + len + string index +
            // Some/None construction.
            let peek_body = IRNode::If {
                condition: Box::new(binop(
                    IRBinOp::Lt,
                    field(var("state"), "pos"),
                    call("len", vec![field(var("state"), "source")]),
                )),
                then_branch: Box::new(call(
                    "Some",
                    vec![index(
                        field(var("state"), "source"),
                        field(var("state"), "pos"),
                    )],
                )),
                else_branch: Box::new(call("None", vec![])),
            };
            bodies.push((peek.clone(), peek_body, tok_index.clone()));

            // next_char's mirror body: a RecordLit over
            // field projections, with the bridge inside the
            // newline test.
            let is_newline = binop(
                IRBinOp::And,
                binop(
                    IRBinOp::Lt,
                    field(var("state"), "pos"),
                    call("len", vec![field(var("state"), "source")]),
                ),
                binop(
                    IRBinOp::Eq,
                    index(
                        field(var("state"), "source"),
                        field(var("state"), "pos"),
                    ),
                    chr('\n'),
                ),
            );
            let next_char_body = IRNode::RecordLit {
                type_name: "TokenizerState".into(),
                fields: vec![
                    ("source".into(), field(var("state"), "source")),
                    (
                        "pos".into(),
                        binop(IRBinOp::Add, field(var("state"), "pos"), int(1)),
                    ),
                    (
                        "line".into(),
                        IRNode::If {
                            condition: Box::new(is_newline.clone()),
                            then_branch: Box::new(binop(
                                IRBinOp::Add,
                                field(var("state"), "line"),
                                int(1),
                            )),
                            else_branch: Box::new(field(var("state"), "line")),
                        },
                    ),
                    (
                        "col".into(),
                        IRNode::If {
                            condition: Box::new(is_newline),
                            then_branch: Box::new(int(1)),
                            else_branch: Box::new(binop(
                                IRBinOp::Add,
                                field(var("state"), "col"),
                                int(1),
                            )),
                        },
                    ),
                    ("tokens".into(), field(var("state"), "tokens")),
                ],
            };
            bodies.push((next_char.clone(), next_char_body, tok_index.clone()));

            // A Result ite over a record field and an error
            // string (tokenize_all's shape family).
            let result_ite = IRNode::If {
                condition: Box::new(binop(
                    IRBinOp::Ge,
                    field(var("state"), "pos"),
                    call("len", vec![field(var("state"), "source")]),
                )),
                then_branch: Box::new(call(
                    "Ok",
                    vec![field(var("state"), "tokens")],
                )),
                else_branch: Box::new(call(
                    "Err",
                    vec![IRNode::Literal(IRLiteral::String("eof".into()))],
                )),
            };
            bodies.push((tokenize_all.clone(), result_ite, tok_index.clone()));

            // Refusals that must agree: an unknown field, a
            // field of a non-record, a record literal with a
            // hole inside.
            bodies.push((
                peek.clone(),
                field(var("state"), "bogus"),
                tok_index.clone(),
            ));
            bodies.push((
                peek.clone(),
                field(field(var("state"), "pos"), "x"),
                tok_index.clone(),
            ));
            bodies.push((
                next_char.clone(),
                IRNode::RecordLit {
                    type_name: "TokenizerState".into(),
                    fields: vec![(
                        "pos".into(),
                        IRNode::Hole {
                            id: 0,
                            expected_type: None,
                        },
                    )],
                },
                tok_index.clone(),
            ));

            // The bridge's corners against a String+Char
            // spec: an indexed char alone (encodable), an
            // indexed char compared to a Char LITERAL
            // (encodable), an indexed char compared to the
            // Char PARAMETER (the named boundary —
            // unencodable), and Char-pattern matches.
            let sc = spec(vec![("s", TypeRef::String), ("c", TypeRef::Char)]);
            let at0 = index(var("s"), int(0));
            bodies.push((sc.clone(), at0.clone(), idx.clone()));
            bodies.push((
                sc.clone(),
                binop(IRBinOp::Eq, at0.clone(), chr('a')),
                idx.clone(),
            ));
            bodies.push((
                sc.clone(),
                binop(IRBinOp::Eq, at0.clone(), var("c")),
                idx.clone(),
            ));
            bodies.push((
                sc.clone(),
                IRNode::Match {
                    scrutinee: Box::new(var("c")),
                    cases: vec![
                        (
                            IRPattern::Literal(IRLiteral::Char(' ')),
                            boolean(true),
                        ),
                        (IRPattern::Wildcard, boolean(false)),
                    ],
                },
                idx.clone(),
            ));
            bodies.push((
                sc.clone(),
                IRNode::Match {
                    scrutinee: Box::new(var("c")),
                    cases: vec![(
                        IRPattern::Literal(IRLiteral::Char(' ')),
                        boolean(true),
                    )],
                },
                idx.clone(),
            ));

            // Phase 91 corners: the small arms must agree
            // on both sides — constructor matches
            // (exhaustive, fall-through, and a nested
            // sub-pattern), Clone, the (List, List) ite
            // (equal and unequal lengths), and the Append
            // forms (ListLen growth, (Str, CharStr), and
            // the unarmed (Str, Char) decline).
            let s91 = spec(vec![
                ("o", TypeRef::Option(Box::new(TypeRef::Int))),
                ("xs", TypeRef::List(Box::new(TypeRef::Int))),
                ("b", TypeRef::Bool),
                ("t", TypeRef::String),
                ("x", TypeRef::Int),
            ]);
            let ctor =
                |name: &str, pargs: Vec<IRPattern>| IRPattern::Constructor(name.into(), pargs);
            let match_o = |cases: Vec<(IRPattern, IRNode)>| IRNode::Match {
                scrutinee: Box::new(var("o")),
                cases,
            };
            bodies.push((
                s91.clone(),
                match_o(vec![
                    (
                        ctor("Some", vec![IRPattern::Variable("y".into())]),
                        var("y"),
                    ),
                    (ctor("None", vec![]), int(0)),
                ]),
                idx.clone(),
            ));
            bodies.push((
                s91.clone(),
                match_o(vec![(
                    ctor("Some", vec![IRPattern::Variable("y".into())]),
                    var("y"),
                )]),
                idx.clone(),
            ));
            bodies.push((
                s91.clone(),
                match_o(vec![
                    (
                        ctor("Some", vec![IRPattern::Literal(IRLiteral::Int(5))]),
                        int(1),
                    ),
                    (ctor("None", vec![]), int(0)),
                ]),
                idx.clone(),
            ));
            bodies.push((
                s91.clone(),
                IRNode::UnaryOp(IRUnaryOp::Clone, Box::new(var("x"))),
                idx.clone(),
            ));
            let if_lists = |a: Vec<IRNode>, b: Vec<IRNode>| {
                IRNode::Index(
                    Box::new(IRNode::If {
                        condition: Box::new(var("b")),
                        then_branch: Box::new(list(a)),
                        else_branch: Box::new(list(b)),
                    }),
                    Box::new(int(0)),
                )
            };
            bodies.push((
                s91.clone(),
                if_lists(vec![int(1), int(2)], vec![int(3), int(4)]),
                idx.clone(),
            ));
            bodies.push((
                s91.clone(),
                if_lists(vec![int(1), int(2)], vec![int(3), int(4), int(5)]),
                idx.clone(),
            ));
            bodies.push((
                s91.clone(),
                call("len", vec![binop(IRBinOp::Append, var("xs"), int(7))]),
                idx.clone(),
            ));
            bodies.push((
                s91.clone(),
                binop(IRBinOp::Append, var("t"), index(var("t"), int(0))),
                idx.clone(),
            ));
            bodies.push((
                s91.clone(),
                binop(IRBinOp::Append, var("t"), chr('x')),
                idx.clone(),
            ));
        }

        for (spec, body, index) in &bodies {
            let symbolic = body_is_encodable_with(spec, body, index);
            let pure = body_is_encodable_pure(spec, body, index);
            assert_eq!(
                symbolic, pure,
                "encodability disagreement for spec {} body {:?}",
                spec.name, body
            );
            checked += 1;
        }
        assert!(
            checked > 100,
            "the corpus must be substantial, got {} bodies",
            checked
        );
    }
}
