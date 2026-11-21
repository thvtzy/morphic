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
    Opaque,
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

fn param_env(spec: &FunctionSpec) -> AbsEnv {
    let mut env = AbsEnv::new();
    for p in &spec.params {
        let abs = match &p.ty {
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
            _ => Abs::Opaque,
        };
        env.insert(p.name.clone(), abs);
    }
    env
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
        // M1 features (Phase 73): in lockstep with the real
        // evaluator — no record value exists in either walk
        // until Phase 76 (Sym::Record).
        | IRNode::FieldAccess { .. }
        | IRNode::RecordLit { .. }
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
        // Float, Char, Unit: Opaque in the real evaluator.
        IRLiteral::Float(_) | IRLiteral::Char(_) | IRLiteral::Unit => Some(Abs::Opaque),
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
        (Abs::List(_), _) | (Abs::Opaque, _) => PatternCond::Undecidable,
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
            IRPattern::Constructor(..) => continue,
            IRPattern::Guard(..) => return None,
        }
    }
    let (last_conditional, last_val) = arms.pop()?;
    if last_conditional {
        return None; // the match can fall through
    }
    let mut acc = last_val;
    for (_, val) in arms.into_iter().rev() {
        acc = abs_ite(val, acc)?;
    }
    Some(acc)
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
        (Abs::Str(StrVal::Known(s)), Abs::Int(IntVal::Known(n))) => {
            if n < 0 {
                return None;
            }
            if s.chars().nth(n as usize).is_some() {
                Some(Abs::Opaque) // a Char value
            } else {
                None
            }
        }
        _ => None,
    }
}

fn abs_ite(a: Abs, b: Abs) -> Option<Abs> {
    match (a, b) {
        (Abs::Int(_), Abs::Int(_)) => Some(Abs::Int(IntVal::Unknown)),
        (Abs::Bool, Abs::Bool) => Some(Abs::Bool),
        (Abs::Str(_), Abs::Str(_)) => Some(Abs::Str(StrVal::Unknown)),
        (Abs::Opaque, Abs::Opaque) => Some(Abs::Opaque),
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
        _ => None,
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
    let mut env = param_env(spec);
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
    /// match fall-through). A disagreement here is a drift
    /// bug in exactly one of the two implementations.
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
