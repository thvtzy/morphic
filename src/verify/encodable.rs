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
//      ListLen / Char / CharStr / Record / Option / Result /
//      Opaque) — every operator in the real evaluator is
//      sort-strict, and sort mismatches are `None`;
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

use crate::spec::ast::{FunctionSpec, TypeRef};
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
    Opaque,
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

/// The inlining context, mirroring verifier::EvalCtx field
/// for field (same cap, same cycle guard).
struct AbsCtx<'a> {
    index: &'a SpecIndex,
    visiting: Vec<String>,
    depth: usize,
}

impl<'a> AbsCtx<'a> {
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
        env.insert(p.name.clone(), fresh_abs(&p.ty, index, 0));
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
        TypeRef::List(_) => Abs::ListLen,
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
            abs_eval_flow(body, &mut inner, ctx)
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
            let coll = abs_eval(coll, env, ctx)?;
            let idx = abs_eval(idx, env, ctx)?;
            Some(AbsFlow::Normal(abs_index(coll, idx)?))
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
                arms.push((false, abs_eval(body, &mut inner, ctx)?));
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
                        match (pargs.len(), &payload) {
                            (0, _) => {}
                            (1, Some(p)) => match &pargs[0] {
                                IRPattern::Variable(n) => {
                                    inner.insert(n.clone(), (**p).clone());
                                }
                                IRPattern::Wildcard => {}
                                _ => return None,
                            },
                            _ => return None,
                        }
                        let value = abs_eval(body, &mut inner, ctx)?;
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
    abs_eval(&body, &mut env, &ctx.child(name))
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
        _ => None,
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
    abs_eval(body, &mut env, &AbsCtx::root(index)).is_some()
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
