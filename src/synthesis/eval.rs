// ┌──────────────────────────────────────────────────────────┐
// │  IR INTERPRETER — candidate test execution               │
// │                                                          │
// │  A small, total evaluator for the *expression subset*    │
// │  of the IR. `evaluate_candidate` uses it to run a        │
// │  candidate body against a spec's tests and measure a     │
// │  real pass ratio, instead of assuming every test passes. │
// │                                                          │
// │  Anything outside the subset — Holes, loops, mutation,   │
// │  lambdas, higher-order combinators, calls to functions   │
// │  that are not known pure builtins — returns              │
// │  `EvalError::NotEvaluable` with a reason. The evaluator  │
// │  never panics and never guesses a value.                 │
// └──────────────────────────────────────────────────────────┘

use super::engine::{IRBinOp, IRLiteral, IRNode, IRPattern, IRUnaryOp};
use crate::spec::ast::{Expr, FunctionSpec, Param};
use std::collections::HashMap;

/// A runtime value in the evaluable subset.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Char(char),
    Unit,
    List(Vec<Value>),
    /// A record value (M1 feature 2/3, decisions D2a/D3a;
    /// Phase 75): the declared type name plus the field
    /// values in the constructing literal's written order.
    /// Equality is the derived structural equality over that
    /// representation — field order is part of it, matching
    /// how literals are written in the modules (declaration
    /// order).
    Record {
        type_name: String,
        fields: Vec<(String, Value)>,
    },
    /// `Ok(v)` — a Result success value (Phase 75). Result
    /// and Option values exist so that bodies and test
    /// expectations built from the `Ok`/`Err`/`Some`/`None`
    /// constructor calls can be judged in-process; before
    /// this phase those calls were unevaluable and any test
    /// carrying one was unbindable.
    Ok(Box<Value>),
    /// `Err(e)` — a Result failure value (Phase 75).
    Err(Box<Value>),
    /// `Some(v)` — an Option present value (Phase 75).
    Some(Box<Value>),
    /// `None()` — an Option empty value (Phase 75).
    None,
}

/// Variable environment for one evaluation.
pub type Env = HashMap<String, Value>;

/// The single failure mode of the evaluator: this node (or a node
/// it contains) is outside the evaluable subset, or evaluation
/// itself is undefined (unbound variable, out-of-bounds index,
/// integer overflow, …). The payload says exactly why.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    NotEvaluable(String),
}

fn not_evaluable(reason: impl Into<String>) -> EvalError {
    EvalError::NotEvaluable(reason.into())
}

/// Internal control flow: `Return` unwinds to the top of the
/// candidate body, everything else is a plain value.
enum Flow {
    Normal(Value),
    Returned(Value),
}

impl Flow {
    fn unwrap(self) -> Value {
        match self {
            Flow::Normal(v) | Flow::Returned(v) => v,
        }
    }
}

/// Evaluate an IR node in `env`. The environment is cloned
/// internally wherever a construct binds variables, so the
/// caller's env is never mutated.
pub fn eval(node: &IRNode, env: &Env) -> Result<Value, EvalError> {
    let mut working = env.clone();
    Ok(eval_flow(node, &mut working)?.unwrap())
}

fn eval_flow(node: &IRNode, env: &mut Env) -> Result<Flow, EvalError> {
    match node {
        IRNode::Literal(lit) => Ok(Flow::Normal(literal_value(lit))),

        IRNode::Var(name) => env
            .get(name)
            .cloned()
            .map(Flow::Normal)
            .ok_or_else(|| not_evaluable(format!("unbound variable `{}`", name))),

        IRNode::BinOp(op, l, r) => {
            let lv = eval_flow(l, env)?.unwrap();
            let rv = eval_flow(r, env)?.unwrap();
            Ok(Flow::Normal(eval_binop(*op, lv, rv)?))
        }

        IRNode::UnaryOp(op, inner) => {
            let v = eval_flow(inner, env)?.unwrap();
            Ok(Flow::Normal(eval_unaryop(*op, v)?))
        }

        IRNode::If { condition, then_branch, else_branch } => {
            match eval_flow(condition, env)?.unwrap() {
                Value::Bool(true) => eval_flow(then_branch, env),
                Value::Bool(false) => eval_flow(else_branch, env),
                other => Err(not_evaluable(format!(
                    "if condition is not Bool (got {:?})",
                    other
                ))),
            }
        }

        IRNode::Let { name, value, body } => {
            let v = match eval_flow(value, env)? {
                Flow::Returned(v) => return Ok(Flow::Returned(v)),
                Flow::Normal(v) => v,
            };
            let mut inner = env.clone();
            inner.insert(name.clone(), v);
            eval_flow(body, &mut inner)
        }

        IRNode::Block(stmts) => {
            let mut last = Value::Unit;
            for stmt in stmts {
                match eval_flow(stmt, env)? {
                    Flow::Returned(v) => return Ok(Flow::Returned(v)),
                    Flow::Normal(v) => last = v,
                }
            }
            Ok(Flow::Normal(last))
        }

        IRNode::Return(inner) => {
            let v = eval_flow(inner, env)?.unwrap();
            Ok(Flow::Returned(v))
        }

        IRNode::Match { scrutinee, cases } => {
            let scrut = eval_flow(scrutinee, env)?.unwrap();
            for (pattern, arm) in cases {
                match bind_pattern(pattern, &scrut)? {
                    Some(bindings) => {
                        let mut inner = env.clone();
                        inner.extend(bindings);
                        return eval_flow(arm, &mut inner);
                    }
                    None => continue,
                }
            }
            Err(not_evaluable("no match arm matched the scrutinee"))
        }

        IRNode::Call { function, args } => eval_call(function, args, env).map(Flow::Normal),

        IRNode::Collection { elements, .. } => {
            let mut values = Vec::with_capacity(elements.len());
            for el in elements {
                match eval_flow(el, env)? {
                    Flow::Returned(v) => return Ok(Flow::Returned(v)),
                    Flow::Normal(v) => values.push(v),
                }
            }
            Ok(Flow::Normal(Value::List(values)))
        }

        IRNode::Index(coll, idx) => {
            let coll = eval_flow(coll, env)?.unwrap();
            let idx = eval_flow(idx, env)?.unwrap();
            Ok(Flow::Normal(eval_index(coll, idx)?))
        }

        IRNode::Typed { node, .. } => eval_flow(node, env),

        // Everything below is outside the expression subset:
        // unfilled holes, loops and other effectful control flow,
        // allocation/mutation, function values, higher-order
        // combinators, and struct definitions. None of them can
        // be given a value without guessing, so they are reported
        // as not evaluable.
        IRNode::Hole { .. } => Err(not_evaluable("unfilled Hole")),
        IRNode::While { .. } => Err(not_evaluable("While loop")),
        IRNode::For { .. } => Err(not_evaluable("For loop")),
        IRNode::Loop { .. } => Err(not_evaluable("Loop")),
        IRNode::Break { .. } => Err(not_evaluable("Break outside a loop")),
        IRNode::Alloc { .. } => Err(not_evaluable("Alloc (mutable state)")),
        IRNode::Assign { .. } => Err(not_evaluable("Assign (mutation)")),
        IRNode::Lambda { .. } => Err(not_evaluable("Lambda (function value)")),
        IRNode::Closure { .. } => Err(not_evaluable("Closure (function value)")),
        IRNode::Map { .. } => Err(not_evaluable("Map (higher-order)")),
        IRNode::Filter { .. } => Err(not_evaluable("Filter (higher-order)")),
        IRNode::Fold { .. } => Err(not_evaluable("Fold (higher-order)")),
        IRNode::Sort { .. } => Err(not_evaluable("Sort (higher-order)")),
        IRNode::StructDef { .. } => Err(not_evaluable("StructDef (declaration)")),
        // TryPropagate: Result VALUES exist since Phase 75,
        // but `?` propagation semantics in the interpreter do
        // not — unwinding an Err out of the enclosing body is
        // a control-flow behavior this evaluator does not
        // model, so the node stays outside the subset (the
        // Phase 73-era reason, "no Result values", no longer
        // states the cause and is not repeated).
        IRNode::TryPropagate(_) => Err(not_evaluable(
            "TryPropagate (? propagation is outside the evaluable subset)",
        )),
        // M1 features (Phase 75): record construction and
        // field projection over Value::Record (D2a/D3a).
        // Fields evaluate in written order, like Collection
        // elements; a Return inside a field value unwinds
        // the whole evaluation, also like Collection.
        IRNode::RecordLit { type_name, fields } => {
            let mut values = Vec::with_capacity(fields.len());
            for (name, field_node) in fields {
                match eval_flow(field_node, env)? {
                    Flow::Returned(v) => return Ok(Flow::Returned(v)),
                    Flow::Normal(v) => values.push((name.clone(), v)),
                }
            }
            Ok(Flow::Normal(Value::Record {
                type_name: type_name.clone(),
                fields: values,
            }))
        }
        IRNode::FieldAccess { base, field } => {
            let base_value = eval_flow(base, env)?.unwrap();
            match base_value {
                Value::Record { type_name, fields } => fields
                    .into_iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, v)| Flow::Normal(v))
                    .ok_or_else(|| {
                        not_evaluable(format!(
                            "record `{}` has no field `{}`",
                            type_name, field
                        ))
                    }),
                other => Err(not_evaluable(format!(
                    "field `{}` accessed on a non-record value ({:?})",
                    field, other
                ))),
            }
        }
    }
}

fn literal_value(lit: &IRLiteral) -> Value {
    match lit {
        IRLiteral::Int(n) => Value::Int(*n),
        IRLiteral::Float(f) => Value::Float(*f),
        IRLiteral::Bool(b) => Value::Bool(*b),
        IRLiteral::String(s) => Value::Str(s.clone()),
        IRLiteral::Char(c) => Value::Char(*c),
        IRLiteral::Unit => Value::Unit,
    }
}

/// Try to match `pattern` against `value`. Returns:
/// - `Ok(Some(bindings))` — matched, with variables to bind
/// - `Ok(None)` — definitely does not match; try the next arm
/// - `Err(..)` — cannot decide (guard patterns carry an
///   unparsed string condition), so the whole match is
///   not evaluable rather than a guess.
///
/// Constructor patterns return `Ok(None)`: since Phase 75
/// the value domain DOES include constructor values
/// (`Value::Ok` / `Err` / `Some` / `None`), but constructor
/// pattern matching in the interpreter does not — a body
/// whose match discriminates on a constructor cannot be
/// judged in-process (its constructor arms never match
/// here, so it falls through to a later arm or to the
/// no-match failure). That is a recorded residual of this
/// evaluator's subset, not a claim about the values.
fn bind_pattern(pattern: &IRPattern, value: &Value) -> Result<Option<Env>, EvalError> {
    match pattern {
        IRPattern::Wildcard => Ok(Some(Env::new())),
        IRPattern::Variable(name) => {
            let mut env = Env::new();
            env.insert(name.clone(), value.clone());
            Ok(Some(env))
        }
        IRPattern::Literal(lit) => {
            if literal_value(lit) == *value {
                Ok(Some(Env::new()))
            } else {
                Ok(None)
            }
        }
        IRPattern::Constructor(..) => Ok(None),
        IRPattern::Guard(..) => Err(not_evaluable(
            "match guard patterns are outside the evaluable subset",
        )),
    }
}

fn eval_call(function: &str, args: &[IRNode], env: &mut Env) -> Result<Value, EvalError> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(eval_flow(arg, env)?.unwrap());
    }
    match function {
        // `List[...]` is the spec language's list literal; the
        // parser lowers it to a Call named "List" (parser.rs).
        "List" => Ok(Value::List(values)),
        // The Result/Option constructors (Phase 75): the
        // typechecker types `Ok(v)` / `Err(e)` / `Some(v)` /
        // `None()` as constructor calls (Phase 74, parallel
        // to the standing Some/None treatment), and bodies
        // and test expectations are full of them — `peek`'s
        // expected outputs are `Some('h')` / `None()`,
        // `tokenize_one`'s are `Ok(state)`. Each takes
        // exactly its declared arity; anything else is a
        // named failure, not a guessed value.
        "Ok" | "Err" | "Some" => {
            let [value] = values.as_slice() else {
                return Err(not_evaluable(format!(
                    "constructor `{}` expects exactly one argument",
                    function
                )));
            };
            let value = value.clone();
            Ok(match function {
                "Ok" => Value::Ok(Box::new(value)),
                "Err" => Value::Err(Box::new(value)),
                _ => Value::Some(Box::new(value)),
            })
        }
        "None" => {
            if values.is_empty() {
                Ok(Value::None)
            } else {
                Err(not_evaluable(
                    "constructor `None` expects no arguments",
                ))
            }
        }
        // The one pure builtin the specs and templates rely on.
        "len" => match values.as_slice() {
            [Value::List(items)] => Ok(Value::Int(items.len() as i64)),
            [Value::Str(s)] => Ok(Value::Int(s.chars().count() as i64)),
            _ => Err(not_evaluable("len expects one List or String argument")),
        },
        // Template-internal pseudo-calls ("first", "rest",
        // "push", "combine", …) and spec-to-spec calls are not
        // builtins: there is no definition to execute here.
        other => Err(not_evaluable(format!(
            "call to unknown function `{}`",
            other
        ))),
    }
}

/// Evaluate one D1a builtin method call on an already
/// evaluated receiver (Phase 75): the spec language's method
/// semantics on interpreter values. The table is exactly the
/// typechecker's closed table (Phase 74) — `Result`:
/// `is_ok` / `is_err` / `unwrap`; `List`: `len` — all
/// zero-argument. Anything else (unknown method, wrong
/// receiver family, arguments to a builtin, a receiver with
/// no method table) is a named `NotEvaluable`, never a
/// guessed value; `unwrap` on an `Err` value is the named
/// failure the interpreter's failure semantics require.
pub(crate) fn eval_method_call(
    receiver: &Value,
    name: &str,
    args: &[Value],
) -> Result<Value, EvalError> {
    // Every ratified builtin is zero-argument (D1a); an
    // argument is a named failure here just as it is a type
    // error in the checker (Phase 74).
    if !args.is_empty() {
        return Err(not_evaluable(format!(
            "method `{}` is a zero-argument builtin ({} argument(s) given)",
            name,
            args.len()
        )));
    }
    match receiver {
        Value::Ok(payload) => match name {
            "is_ok" => Ok(Value::Bool(true)),
            "is_err" => Ok(Value::Bool(false)),
            "unwrap" => Ok((**payload).clone()),
            _ => Err(not_evaluable(format!(
                "method `{}` does not exist on a Result value",
                name
            ))),
        },
        Value::Err(_) => match name {
            "is_ok" => Ok(Value::Bool(false)),
            "is_err" => Ok(Value::Bool(true)),
            // The interpreter's failure semantics: unwrapping
            // an Err has no value, so it is an evaluation
            // failure with a reason — never a panic and never
            // a guessed payload.
            "unwrap" => Err(not_evaluable(
                "method `unwrap` called on an Err value",
            )),
            _ => Err(not_evaluable(format!(
                "method `{}` does not exist on a Result value",
                name
            ))),
        },
        Value::List(items) => match name {
            "len" => Ok(Value::Int(items.len() as i64)),
            _ => Err(not_evaluable(format!(
                "method `{}` does not exist on a List value",
                name
            ))),
        },
        _ => Err(not_evaluable(format!(
            "method `{}` called on a value with no method table ({:?})",
            name, receiver
        ))),
    }
}

fn eval_index(coll: Value, idx: Value) -> Result<Value, EvalError> {
    match (coll, idx) {
        (Value::List(items), Value::Int(i)) => {
            if i < 0 {
                return Err(not_evaluable("negative list index"));
            }
            items
                .get(i as usize)
                .cloned()
                .ok_or_else(|| not_evaluable("list index out of bounds"))
        }
        (Value::Str(s), Value::Int(i)) => {
            if i < 0 {
                return Err(not_evaluable("negative string index"));
            }
            s.chars()
                .nth(i as usize)
                .map(Value::Char)
                .ok_or_else(|| not_evaluable("string index out of bounds"))
        }
        _ => Err(not_evaluable("indexing a non-collection value")),
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(n) => Some(*n as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

/// Value equality with numeric promotion: Int(1) == Float(1.0).
fn values_equal(l: &Value, r: &Value) -> bool {
    if let (Some(a), Some(b)) = (as_f64(l), as_f64(r)) {
        if matches!(l, Value::Int(_) | Value::Float(_))
            && matches!(r, Value::Int(_) | Value::Float(_))
        {
            return a == b;
        }
    }
    l == r
}

fn eval_binop(op: IRBinOp, l: Value, r: Value) -> Result<Value, EvalError> {
    use IRBinOp::*;
    match op {
        Eq => Ok(Value::Bool(values_equal(&l, &r))),
        Neq => Ok(Value::Bool(!values_equal(&l, &r))),
        Lt | Le | Gt | Ge => {
            let ord = match (&l, &r) {
                (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
                (Value::Str(a), Value::Str(b)) => a.partial_cmp(b),
                (Value::Char(a), Value::Char(b)) => a.partial_cmp(b),
                _ => match (as_f64(&l), as_f64(&r)) {
                    (Some(a), Some(b)) => a.partial_cmp(&b),
                    _ => None,
                },
            };
            let ord = ord.ok_or_else(|| {
                not_evaluable(format!("values are not orderable: {:?} vs {:?}", l, r))
            })?;
            let result = match op {
                Lt => ord == std::cmp::Ordering::Less,
                Le => ord != std::cmp::Ordering::Greater,
                Gt => ord == std::cmp::Ordering::Greater,
                _ => ord != std::cmp::Ordering::Less,
            };
            Ok(Value::Bool(result))
        }
        And => match (l, r) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
            _ => Err(not_evaluable("`&&` on non-Bool operands")),
        },
        Or => match (l, r) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
            _ => Err(not_evaluable("`||` on non-Bool operands")),
        },
        Xor => match (l, r) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a ^ b)),
            _ => Err(not_evaluable("xor on non-Bool operands")),
        },
        Add | Sub | Mul | Div | Mod => eval_arith(op, l, r),
        Concat => match (l, r) {
            (Value::Str(mut a), Value::Str(b)) => {
                a.push_str(&b);
                Ok(Value::Str(a))
            }
            (Value::List(mut a), Value::List(b)) => {
                a.extend(b);
                Ok(Value::List(a))
            }
            _ => Err(not_evaluable("concat on non-String/non-List operands")),
        },
        Append => match (l, r) {
            (Value::List(mut items), item) => {
                items.push(item);
                Ok(Value::List(items))
            }
            _ => Err(not_evaluable("append on a non-List value")),
        },
        Shl | Shr => match (l, r) {
            (Value::Int(a), Value::Int(b)) => {
                let shifted = if op == Shl {
                    a.checked_shl(b as u32)
                } else {
                    a.checked_shr(b as u32)
                };
                shifted.map(Value::Int).ok_or_else(|| not_evaluable("shift overflow"))
            }
            _ => Err(not_evaluable("shift on non-Int operands")),
        },
        Merge => Err(not_evaluable("Merge operator")),
    }
}

fn eval_arith(op: IRBinOp, l: Value, r: Value) -> Result<Value, EvalError> {
    use IRBinOp::*;
    // Pure-integer arithmetic stays integer (checked: overflow
    // and division by zero are evaluation failures, not wraps).
    if let (Value::Int(a), Value::Int(b)) = (&l, &r) {
        let (a, b) = (*a, *b);
        let result = match op {
            Add => a.checked_add(b),
            Sub => a.checked_sub(b),
            Mul => a.checked_mul(b),
            Div => a.checked_div(b),
            Mod => a.checked_rem(b),
            _ => unreachable!(),
        };
        return result
            .map(Value::Int)
            .ok_or_else(|| not_evaluable("integer arithmetic overflow or division by zero"));
    }
    match (as_f64(&l), as_f64(&r)) {
        (Some(a), Some(b)) => {
            let result = match op {
                Add => a + b,
                Sub => a - b,
                Mul => a * b,
                Div => a / b,
                Mod => a % b,
                _ => unreachable!(),
            };
            Ok(Value::Float(result))
        }
        _ => Err(not_evaluable(format!(
            "arithmetic on non-numeric operands: {:?} and {:?}",
            l, r
        ))),
    }
}

fn eval_unaryop(op: IRUnaryOp, v: Value) -> Result<Value, EvalError> {
    use IRUnaryOp::*;
    match op {
        Neg => match v {
            Value::Int(n) => n
                .checked_neg()
                .map(Value::Int)
                .ok_or_else(|| not_evaluable("integer negation overflow")),
            Value::Float(f) => Ok(Value::Float(-f)),
            _ => Err(not_evaluable("negation of a non-numeric value")),
        },
        Not => match v {
            Value::Bool(b) => Ok(Value::Bool(!b)),
            _ => Err(not_evaluable("`!` on a non-Bool value")),
        },
        Abs => match v {
            Value::Int(n) => n
                .checked_abs()
                .map(Value::Int)
                .ok_or_else(|| not_evaluable("integer abs overflow")),
            Value::Float(f) => Ok(Value::Float(f.abs())),
            _ => Err(not_evaluable("abs of a non-numeric value")),
        },
        Len => match v {
            Value::List(items) => Ok(Value::Int(items.len() as i64)),
            Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
            _ => Err(not_evaluable("len of a non-collection value")),
        },
        Head | Tail | Init | Last | Reverse | Clone => Err(not_evaluable(format!(
            "unary operator {:?} is outside the evaluable subset",
            op
        ))),
    }
}

// ── Spec test execution ───────────────────────────────────

/// Map a spec test's input/output `Expr` to an evaluator
/// `Value`. Literals map directly; composite literal
/// expressions (e.g. `1 + 2`, `List[1, 2]`) are lowered through
/// the engine's own `expr_to_ir` and evaluated in an empty
/// environment — the same conversion `seed_from_tests` uses
/// for expected outputs. Anything referencing a variable or an
/// unknown call cannot be mapped and returns `None`.
///
/// One carve-out (Phase 75): `Expr::MethodCall` is evaluated
/// HERE, directly on mapped values, because `expr_to_ir`
/// lowers it to a Hole (the IR kind table has no method-call
/// kind — pinned by
/// `expr_to_ir_method_call_lowers_to_hole_no_ir_kind`).
/// The receiver and arguments map recursively through this
/// same function, so a method call on a closed receiver
/// (`Ok(5).is_ok()`, `Ok([1, 2]).unwrap().len()`) maps,
/// while one on a variable does not. The semantics are
/// `eval_method_call`'s D1a table; its failures (unwrap on
/// an Err, unknown method, …) map to `None`, this function's
/// only failure channel. Note the boundary precisely: a
/// method call UNDER another composite (a field projection
/// or index over a method call, say) still routes through
/// the `expr_to_ir` fallback and hits the pinned Hole —
/// no test in the restored tokenizer module has that shape,
/// and constraints themselves are never mapped here at all
/// (Z3 judges them natively, Phase 76; the compiled runner
/// renders them, Phase 77).
pub fn value_of_expr(expr: &Expr) -> Option<Value> {
    match expr {
        Expr::IntLit(n) => Some(Value::Int(*n)),
        Expr::FloatLit(f) => Some(Value::Float(*f)),
        Expr::BoolLit(b) => Some(Value::Bool(*b)),
        Expr::StringLit(s) => Some(Value::Str(s.clone())),
        Expr::CharLit(c) => Some(Value::Char(*c)),
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            let receiver = value_of_expr(receiver)?;
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(value_of_expr(arg)?);
            }
            eval_method_call(&receiver, name, &values).ok()
        }
        _ => {
            let ir = super::engine::expr_to_ir(expr);
            eval(&ir, &Env::new()).ok()
        }
    }
}

/// Build the environment for one test: the test input value is
/// bound to the spec's parameter(s). A single parameter takes
/// the value directly; multiple parameters bind elementwise
/// from either of the two multi-value input forms:
///
/// - a TUPLE input `(v1, v2, ...)` — the parser lowers it to
///   `Call("__tuple__", [v1, v2, ...])` (parser.rs), and each
///   element is mapped through `value_of_expr` on its own
///   (Phase 53: before this, the tuple was mapped as ONE
///   value, which always failed — "__tuple__" is not an
///   evaluable function — so tuple tests never bound);
/// - a whole input that maps to a List of exactly the
///   parameter count.
///
/// A tuple whose arity differs from the parameter count, or
/// with an element that cannot be mapped (a lambda for a
/// fn-typed parameter — the evaluator has no function values;
/// a nested tuple — there is no tuple Value), does not bind:
/// the test is excluded, exactly as before.
fn bind_test_env(params: &[Param], input: &Expr) -> Option<Env> {
    // Tuple form: bind elementwise. Checked before the
    // whole-value mapping, which cannot represent a tuple.
    if params.len() >= 2 {
        if let Expr::Call(name, parts) = input {
            if name == "__tuple__" && parts.len() == params.len() {
                let mut env = Env::new();
                for (param, part) in params.iter().zip(parts.iter()) {
                    env.insert(param.name.clone(), value_of_expr(part)?);
                }
                return Some(env);
            }
        }
    }
    let value = value_of_expr(input)?;
    let mut env = Env::new();
    match params.len() {
        0 => Some(env),
        1 => {
            env.insert(params[0].name.clone(), value);
            Some(env)
        }
        _ => match value {
            Value::List(items) if items.len() == params.len() => {
                for (param, item) in params.iter().zip(items) {
                    env.insert(param.name.clone(), item);
                }
                Some(env)
            }
            _ => None,
        },
    }
}

/// Outcome of running a candidate body against a spec's tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestRun {
    /// Tests whose input AND expected output could be mapped
    /// to evaluator values. Tests that cannot be mapped are
    /// excluded entirely — they are a limitation of the test
    /// encoding, not of the candidate.
    pub evaluable: usize,
    /// Evaluable tests the candidate actually passed. A
    /// candidate whose body is not evaluable (holes, unbound
    /// variables, unknown calls, …) fails the test: it scores
    /// 0 on it, it does not make the test non-evaluable.
    pub passed: usize,
}

/// How many of the spec's tests can be bound at all — input
/// AND expected output mappable to evaluator values. Binding
/// never looks at a candidate body, so this is a property of
/// the spec's test encodings alone: it equals `evaluable` for
/// every candidate. A spec whose count falls short of its
/// test count has tests the interpreter cannot judge (the
/// lambda-carrying tuple residue) — the trigger signal for
/// the compiled-runner final scoring in synthesis::runner
/// (Phase 58), which exists for exactly that residue.
pub fn bindable_test_count(spec: &FunctionSpec) -> usize {
    test_bindability(spec).iter().filter(|b| **b).count()
}

/// Per-test bindability, in `spec.tests` order: test `i`
/// binds iff its input binds against the spec's parameters
/// AND its expected output maps to an evaluator value. This
/// is an INVENTORY of what the interpreter could judge a
/// candidate against — it executes nothing (running a test
/// needs a candidate body; see `run_spec_tests`). The
/// playground's test panel reports exactly this (Phase 66,
/// v0.6 W1/W2 surface).
pub fn test_bindability(spec: &FunctionSpec) -> Vec<bool> {
    spec.tests
        .iter()
        .map(|t| {
            bind_test_env(&spec.params, &t.input).is_some()
                && value_of_expr(&t.expected_output).is_some()
        })
        .collect()
}

/// Per-test outcome of executing one candidate body against
/// a spec's tests — the fine-grained form of `TestRun`
/// (Phase 68, v0.6 W3). `run_spec_tests` is a fold over
/// these: a test counts as evaluable iff its outcome is not
/// `Unbindable`, and as passed iff its outcome is `Passed`.
/// The playground's synthesis panel reports outcomes per
/// test so that a small passed count never hides WHY the
/// rest did not pass — the denominator-honesty rule.
#[derive(Debug, Clone, PartialEq)]
pub enum TestOutcome {
    /// Bound, evaluated, and the value matched.
    Passed,
    /// Bound and evaluated, but the value differed.
    Failed,
    /// Bound, but the body is outside the interpreter's
    /// evaluable subset on this test's environment (a loop
    /// body, a hole, an unbound variable, …). The payload is
    /// the evaluator's own reason. The test still counts as
    /// evaluable — the interpreter COULD judge it; this body
    /// gave it nothing to judge (see `valued_test_count`).
    BodyNotEvaluable(String),
    /// The test itself cannot be mapped to interpreter
    /// values, so no candidate could ever be judged against
    /// it. The payload is the reason CLASS: the expected
    /// output is unmappable, the input is unmappable (it
    /// carries a value with no interpreter representation,
    /// e.g. a function/lambda for a fn-typed parameter), or
    /// the input's shape does not match the parameter list.
    Unbindable(String),
}

/// Execute `body` against every test in `spec.tests`,
/// reporting the per-test outcome (in `spec.tests` order).
pub fn test_outcomes(body: &IRNode, spec: &FunctionSpec) -> Vec<TestOutcome> {
    spec.tests
        .iter()
        .map(|test| {
            let Some(expected) = value_of_expr(&test.expected_output) else {
                return TestOutcome::Unbindable(
                    "expected output cannot be mapped to an interpreter value".into(),
                );
            };
            let Some(env) = bind_test_env(&spec.params, &test.input) else {
                return TestOutcome::Unbindable(if value_of_expr(&test.input).is_none() {
                    "input cannot be mapped to an interpreter value \
                     (it carries a value the interpreter cannot represent, \
                     e.g. a function/lambda)"
                        .into()
                } else {
                    "input does not match the spec's parameter list (shape/arity)".into()
                });
            };
            match eval(body, &env) {
                Ok(actual) if values_equal(&actual, &expected) => TestOutcome::Passed,
                Ok(_) => TestOutcome::Failed,
                Err(EvalError::NotEvaluable(reason)) => {
                    TestOutcome::BodyNotEvaluable(reason)
                }
            }
        })
        .collect()
}

/// Execute `body` against every mappable test in `spec.tests`.
pub fn run_spec_tests(body: &IRNode, spec: &FunctionSpec) -> TestRun {
    let outcomes = test_outcomes(body, spec);
    TestRun {
        evaluable: outcomes
            .iter()
            .filter(|o| !matches!(o, TestOutcome::Unbindable(_)))
            .count(),
        passed: outcomes
            .iter()
            .filter(|o| matches!(o, TestOutcome::Passed))
            .count(),
    }
}

/// How many of the spec's bindable tests produce a VALUE
/// under `body` — `eval` returns Ok — regardless of whether
/// the value matches the expected output. `run_spec_tests`
/// deliberately does not draw this distinction (a body that
/// cannot be evaluated fails the test, like a wrong body);
/// this count exists for the runner's Phase 60 trigger
/// extension (synthesis::runner), which must tell "the body
/// evaluated and answered wrong" (valued > 0 — the
/// interpreter's 0.0 is behavioral truth) from "the body
/// cannot be evaluated at all" (valued == 0 while tests
/// bind — the 0.0 is a coverage fact about the
/// interpreter's subset, e.g. a For/Alloc/Assign scan body,
/// and the compiled vehicle is the honest judge).
pub fn valued_test_count(body: &IRNode, spec: &FunctionSpec) -> usize {
    let mut valued = 0;
    for test in &spec.tests {
        let (Some(env), Some(_)) = (
            bind_test_env(&spec.params, &test.input),
            value_of_expr(&test.expected_output),
        ) else {
            continue;
        };
        if eval(body, &env).is_ok() {
            valued += 1;
        }
    }
    valued
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::ast::{BinOp as SpecBinOp, Test};
    use crate::synthesis::engine::CollectionKind;

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
    fn env_with(pairs: &[(&str, Value)]) -> Env {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    // ── Literals / Var ────────────────────────────────────

    #[test]
    fn eval_literals() {
        let env = Env::new();
        assert_eq!(eval(&int(42), &env), Ok(Value::Int(42)));
        assert_eq!(eval(&boolean(true), &env), Ok(Value::Bool(true)));
        assert_eq!(eval(&string("hi"), &env), Ok(Value::Str("hi".into())));
        assert_eq!(
            eval(&IRNode::Literal(IRLiteral::Unit), &env),
            Ok(Value::Unit)
        );
        assert_eq!(
            eval(&IRNode::Literal(IRLiteral::Char('x')), &env),
            Ok(Value::Char('x'))
        );
    }

    #[test]
    fn eval_var_bound_and_unbound() {
        let env = env_with(&[("x", Value::Int(7))]);
        assert_eq!(eval(&var("x"), &env), Ok(Value::Int(7)));
        // The classic synthesized-candidate failure: free variables
        // like Var("result") must fail evaluation, not guess.
        assert!(matches!(
            eval(&var("result"), &env),
            Err(EvalError::NotEvaluable(_))
        ));
    }

    // ── Operators ─────────────────────────────────────────

    #[test]
    fn eval_int_arithmetic() {
        let env = Env::new();
        let expr = binop(IRBinOp::Add, int(2), binop(IRBinOp::Mul, int(3), int(4)));
        assert_eq!(eval(&expr, &env), Ok(Value::Int(14)));
        assert_eq!(
            eval(&binop(IRBinOp::Mod, int(17), int(5)), &env),
            Ok(Value::Int(2))
        );
        // Division by zero is an evaluation failure, never a panic.
        assert!(eval(&binop(IRBinOp::Div, int(1), int(0)), &env).is_err());
    }

    #[test]
    fn eval_comparisons_and_logic() {
        let env = env_with(&[("tag", Value::Int(5))]);
        // (tag>=0 && tag<=8) — the validator.morph dialect shape.
        let expr = binop(
            IRBinOp::And,
            binop(IRBinOp::Ge, var("tag"), int(0)),
            binop(IRBinOp::Le, var("tag"), int(8)),
        );
        assert_eq!(eval(&expr, &env), Ok(Value::Bool(true)));
        let env = env_with(&[("tag", Value::Int(9))]);
        assert_eq!(eval(&expr, &env), Ok(Value::Bool(false)));
        // Numeric promotion: Int(1) == Float(1.0).
        let eq = binop(
            IRBinOp::Eq,
            int(1),
            IRNode::Literal(IRLiteral::Float(1.0)),
        );
        assert_eq!(eval(&eq, &Env::new()), Ok(Value::Bool(true)));
        // String ordering.
        let lt = binop(IRBinOp::Lt, string("a"), string("b"));
        assert_eq!(eval(&lt, &Env::new()), Ok(Value::Bool(true)));
    }

    #[test]
    fn eval_unary_ops() {
        let env = Env::new();
        assert_eq!(
            eval(&IRNode::UnaryOp(IRUnaryOp::Neg, Box::new(int(3))), &env),
            Ok(Value::Int(-3))
        );
        assert_eq!(
            eval(&IRNode::UnaryOp(IRUnaryOp::Not, Box::new(boolean(true))), &env),
            Ok(Value::Bool(false))
        );
        // Len over a collection literal.
        let list = IRNode::Collection {
            kind: CollectionKind::List,
            elements: vec![int(1), int(2), int(3)],
        };
        assert_eq!(
            eval(&IRNode::UnaryOp(IRUnaryOp::Len, Box::new(list)), &env),
            Ok(Value::Int(3))
        );
        // Head is outside the subset.
        let list = IRNode::Collection {
            kind: CollectionKind::List,
            elements: vec![int(1)],
        };
        assert!(eval(&IRNode::UnaryOp(IRUnaryOp::Head, Box::new(list)), &env).is_err());
    }

    #[test]
    fn eval_string_concat_and_len_call() {
        let env = Env::new();
        let concat = IRNode::BinOp(
            IRBinOp::Concat,
            Box::new(string("foo")),
            Box::new(string("bar")),
        );
        assert_eq!(eval(&concat, &env), Ok(Value::Str("foobar".into())));
        let len_call = IRNode::Call {
            function: "len".into(),
            args: vec![string("abcd")],
        };
        assert_eq!(eval(&len_call, &env), Ok(Value::Int(4)));
    }

    // ── Control flow ──────────────────────────────────────

    #[test]
    fn eval_if_let_block_return() {
        let env = env_with(&[("kind", Value::Int(0))]);
        let if_node = IRNode::If {
            condition: Box::new(binop(IRBinOp::Eq, var("kind"), int(0))),
            then_branch: Box::new(boolean(true)),
            else_branch: Box::new(boolean(false)),
        };
        assert_eq!(eval(&if_node, &env), Ok(Value::Bool(true)));

        // let x = 3 in x * 2
        let let_node = IRNode::Let {
            name: "x".into(),
            value: Box::new(int(3)),
            body: Box::new(binop(IRBinOp::Mul, var("x"), int(2))),
        };
        assert_eq!(eval(&let_node, &Env::new()), Ok(Value::Int(6)));

        // Block: a Return unwinds past later statements.
        let block = IRNode::Block(vec![
            IRNode::Return(Box::new(int(1))),
            IRNode::Hole { id: 9, expected_type: None },
        ]);
        assert_eq!(eval(&block, &Env::new()), Ok(Value::Int(1)));
        // Block value is its last statement.
        let block = IRNode::Block(vec![int(1), int(2)]);
        assert_eq!(eval(&block, &Env::new()), Ok(Value::Int(2)));
    }

    #[test]
    fn eval_match_literal_wildcard_and_binding() {
        // The TestInduced shape: match on the input variable with
        // literal arms and a wildcard arm.
        let body = IRNode::Match {
            scrutinee: Box::new(var("kind")),
            cases: vec![
                (IRPattern::Literal(IRLiteral::Int(0)), boolean(true)),
                (IRPattern::Literal(IRLiteral::Int(2)), boolean(false)),
                (IRPattern::Wildcard, boolean(false)),
            ],
        };
        let env = env_with(&[("kind", Value::Int(0))]);
        assert_eq!(eval(&body, &env), Ok(Value::Bool(true)));
        let env = env_with(&[("kind", Value::Int(2))]);
        assert_eq!(eval(&body, &env), Ok(Value::Bool(false)));
        let env = env_with(&[("kind", Value::Int(7))]);
        assert_eq!(eval(&body, &env), Ok(Value::Bool(false)));

        // Variable pattern binds the scrutinee.
        let body = IRNode::Match {
            scrutinee: Box::new(int(41)),
            cases: vec![(
                IRPattern::Variable("n".into()),
                binop(IRBinOp::Add, var("n"), int(1)),
            )],
        };
        assert_eq!(eval(&body, &Env::new()), Ok(Value::Int(42)));

        // No matching arm → not evaluable, not a guessed value.
        let body = IRNode::Match {
            scrutinee: Box::new(int(3)),
            cases: vec![(IRPattern::Literal(IRLiteral::Int(0)), boolean(true))],
        };
        assert!(eval(&body, &Env::new()).is_err());
    }

    #[test]
    fn eval_collection_and_index() {
        let env = Env::new();
        let list = IRNode::Collection {
            kind: CollectionKind::List,
            elements: vec![int(10), int(20), int(30)],
        };
        let idx = IRNode::Index(Box::new(list.clone()), Box::new(int(1)));
        assert_eq!(eval(&idx, &env), Ok(Value::Int(20)));
        let oob = IRNode::Index(Box::new(list), Box::new(int(9)));
        assert!(eval(&oob, &env).is_err());
    }

    // ── NotEvaluable surface ──────────────────────────────

    #[test]
    fn eval_not_evaluable_nodes() {
        let env = Env::new();
        let cases: Vec<IRNode> = vec![
            IRNode::Hole { id: 0, expected_type: None },
            IRNode::While {
                condition: Box::new(boolean(true)),
                body: Box::new(int(1)),
                invariant: None,
            },
            IRNode::Alloc {
                name: "x".into(),
                ty: crate::spec::ast::TypeRef::Int,
                initial: Box::new(int(0)),
            },
            IRNode::Assign {
                target: Box::new(var("x")),
                value: Box::new(int(1)),
            },
            IRNode::Lambda { params: vec![], body: Box::new(int(1)) },
            IRNode::Call { function: "is_sorted".into(), args: vec![] },
            IRNode::Call { function: "first".into(), args: vec![int(1)] },
        ];
        for node in &cases {
            assert!(
                matches!(eval(node, &env), Err(EvalError::NotEvaluable(_))),
                "expected NotEvaluable for {:?}",
                node
            );
        }
    }

    // ── Test value mapping ────────────────────────────────

    #[test]
    fn value_of_expr_maps_literals_and_composites() {
        assert_eq!(value_of_expr(&Expr::IntLit(3)), Some(Value::Int(3)));
        assert_eq!(
            value_of_expr(&Expr::StringLit("Int".into())),
            Some(Value::Str("Int".into()))
        );
        // Composite literal: 1 + 2 lowers and evaluates.
        let sum = Expr::BinOp(
            SpecBinOp::Add,
            Box::new(Expr::IntLit(1)),
            Box::new(Expr::IntLit(2)),
        );
        assert_eq!(value_of_expr(&sum), Some(Value::Int(3)));
        // List literal: the parser's Call("List", …) form.
        let list = Expr::Call(
            "List".into(),
            vec![Expr::IntLit(1), Expr::IntLit(2)],
        );
        assert_eq!(
            value_of_expr(&list),
            Some(Value::List(vec![Value::Int(1), Value::Int(2)]))
        );
        // A free variable cannot be mapped.
        assert_eq!(value_of_expr(&Expr::Var("output".into())), None);
        // Char literal (Phase 45, finding F4): maps to the
        // Value::Char the interpreter has always carried.
        assert_eq!(
            value_of_expr(&Expr::CharLit('x')),
            Some(Value::Char('x'))
        );
        assert_eq!(
            value_of_expr(&Expr::CharLit('\n')),
            Some(Value::Char('\n'))
        );
    }

    // ── run_spec_tests ────────────────────────────────────

    fn is_literal_kind_spec() -> FunctionSpec {
        // Mirrors examples/self_host/validator.morph `is_literal_kind`.
        let mut spec = dummy_spec_with(
            "is_literal_kind",
            vec![Param {
                name: "kind".into(),
                ty: crate::spec::ast::TypeRef::Int,
                annotations: Default::default(),
            }],
            crate::spec::ast::TypeRef::Bool,
        );
        spec.tests = vec![
            Test {
                name: Some("literal".into()),
                input: Expr::IntLit(0),
                expected_output: Expr::BoolLit(true),
                timeout_ms: None,
                property: false,
            },
            Test {
                name: Some("binary-not-literal".into()),
                input: Expr::IntLit(2),
                expected_output: Expr::BoolLit(false),
                timeout_ms: None,
                property: false,
            },
            Test {
                name: Some("call-not-literal".into()),
                input: Expr::IntLit(4),
                expected_output: Expr::BoolLit(false),
                timeout_ms: None,
                property: false,
            },
        ];
        spec
    }

    fn dummy_spec_with(
        name: &str,
        params: Vec<Param>,
        return_type: crate::spec::ast::TypeRef,
    ) -> FunctionSpec {
        FunctionSpec {
            name: name.into(),
            doc: None,
            generics: vec![],
            params,
            return_type,
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: Default::default(),
        }
    }

    #[test]
    fn run_spec_tests_correct_candidate_passes_all() {
        let spec = is_literal_kind_spec();
        // kind == 0 — the semantics of is_literal_kind.
        let correct = binop(IRBinOp::Eq, var("kind"), int(0));
        let run = run_spec_tests(&correct, &spec);
        assert_eq!(run, TestRun { evaluable: 3, passed: 3 });
    }

    #[test]
    fn run_spec_tests_wrong_candidate_fails_by_exact_amount() {
        let spec = is_literal_kind_spec();
        // Always false: passes the two false-expecting tests only.
        let wrong = boolean(false);
        let run = run_spec_tests(&wrong, &spec);
        assert_eq!(run, TestRun { evaluable: 3, passed: 2 });
        // Always true: passes only the literal test.
        let wrong = boolean(true);
        let run = run_spec_tests(&wrong, &spec);
        assert_eq!(run, TestRun { evaluable: 3, passed: 1 });
    }

    #[test]
    fn run_spec_tests_unevaluable_body_fails_tests_not_skips_them() {
        let spec = is_literal_kind_spec();
        // Unbound variable body: every test counts, none pass.
        let body = var("result");
        let run = run_spec_tests(&body, &spec);
        assert_eq!(run, TestRun { evaluable: 3, passed: 0 });
        // Hole body: same.
        let body = IRNode::Hole { id: 0, expected_type: None };
        let run = run_spec_tests(&body, &spec);
        assert_eq!(run, TestRun { evaluable: 3, passed: 0 });
    }

    // ── test_outcomes (Phase 68, W3) ──────────────────────

    #[test]
    fn test_outcomes_classify_pass_fail_and_body_coverage() {
        let spec = is_literal_kind_spec();
        // Correct body: every test Passed.
        let correct = binop(IRBinOp::Eq, var("kind"), int(0));
        assert_eq!(
            test_outcomes(&correct, &spec),
            vec![TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed]
        );
        // Wrong body: evaluated, value differed → Failed.
        let wrong = boolean(true);
        assert_eq!(
            test_outcomes(&wrong, &spec),
            vec![TestOutcome::Passed, TestOutcome::Failed, TestOutcome::Failed]
        );
        // Loop body: bound tests, but the interpreter cannot
        // execute the body — BodyNotEvaluable, and the tests
        // still count as evaluable in the fold.
        let loopy = IRNode::For {
            var: "i".into(),
            start: Box::new(int(0)),
            end: Box::new(int(1)),
            body: Box::new(boolean(true)),
        };
        let outcomes = test_outcomes(&loopy, &spec);
        assert!(
            outcomes.iter().all(|o| matches!(o, TestOutcome::BodyNotEvaluable(_))),
            "loop body must be BodyNotEvaluable on every test: {outcomes:?}"
        );
        assert_eq!(
            run_spec_tests(&loopy, &spec),
            TestRun { evaluable: 3, passed: 0 },
            "the fold over outcomes must reproduce run_spec_tests exactly"
        );
    }

    #[test]
    fn test_outcomes_classify_unbindable_reason_classes() {
        use crate::spec::ast::TypeRef;
        // A lambda input for a fn-typed parameter: the
        // evaluator has no function values (the sort residue).
        let mut spec = dummy_spec_with(
            "apply_cmp",
            vec![Param {
                name: "cmp".into(),
                ty: TypeRef::Function(vec![TypeRef::Int, TypeRef::Int], Box::new(TypeRef::Bool)),
                annotations: Default::default(),
            }],
            TypeRef::Bool,
        );
        spec.tests = vec![Test {
            name: Some("lambda-input".into()),
            input: Expr::Lambda(
                vec![("a".into(), TypeRef::Int), ("b".into(), TypeRef::Int)],
                Box::new(Expr::BoolLit(true)),
            ),
            expected_output: Expr::BoolLit(true),
            timeout_ms: None,
            property: false,
        }];
        let outcomes = test_outcomes(&boolean(true), &spec);
        match &outcomes[0] {
            TestOutcome::Unbindable(reason) => {
                assert!(reason.contains("function"), "reason class must name the cause: {reason}")
            }
            other => panic!("expected Unbindable, got {other:?}"),
        }

        // A scalar input for a two-parameter spec: the value
        // maps fine; the SHAPE does not match the parameters.
        let mut spec = dummy_spec_with(
            "pair_sum",
            vec![
                Param { name: "a".into(), ty: TypeRef::Int, annotations: Default::default() },
                Param { name: "b".into(), ty: TypeRef::Int, annotations: Default::default() },
            ],
            TypeRef::Int,
        );
        spec.tests = vec![Test {
            name: Some("scalar-for-pair".into()),
            input: Expr::IntLit(5),
            expected_output: Expr::IntLit(5),
            timeout_ms: None,
            property: false,
        }];
        let outcomes = test_outcomes(&int(0), &spec);
        match &outcomes[0] {
            TestOutcome::Unbindable(reason) => {
                assert!(reason.contains("parameter list"), "reason class: {reason}")
            }
            other => panic!("expected Unbindable, got {other:?}"),
        }
        // Neither unbindable test enters the evaluable count.
        assert_eq!(
            run_spec_tests(&int(0), &spec),
            TestRun { evaluable: 0, passed: 0 }
        );
    }

    /// Phase 60: `valued_test_count` separates "the body
    /// evaluated and answered wrong" from "the body cannot
    /// be evaluated at all" — the distinction
    /// `run_spec_tests` deliberately does not draw (both
    /// score 0 on the test). The runner's Phase 60 trigger
    /// extension reads it: a candidate valued on ZERO
    /// bindable tests is interpreter-unevaluable, so its 0.0
    /// is a coverage fact, not a behavioral one.
    #[test]
    fn valued_test_count_distinguishes_wrong_from_unevaluable() {
        let spec = is_literal_kind_spec();
        // Evaluable and correct: valued everywhere.
        let correct = binop(IRBinOp::Eq, var("kind"), int(0));
        assert_eq!(valued_test_count(&correct, &spec), 3);
        // Evaluable and wrong: still valued — it produced an
        // answer on every test.
        assert_eq!(valued_test_count(&boolean(false), &spec), 3);
        // Unevaluable (unbound variable / hole / a loop body
        // over the parameter): valued nowhere.
        assert_eq!(valued_test_count(&var("result"), &spec), 0);
        assert_eq!(
            valued_test_count(&IRNode::Hole { id: 0, expected_type: None }, &spec),
            0
        );
        let loop_body = IRNode::Block(vec![
            IRNode::Alloc {
                name: "acc".into(),
                ty: crate::spec::ast::TypeRef::Int,
                initial: Box::new(int(0)),
            },
            IRNode::Return(Box::new(var("acc"))),
        ]);
        assert_eq!(valued_test_count(&loop_body, &spec), 0);
    }

    #[test]
    fn run_spec_tests_unmappable_test_is_excluded() {
        let mut spec = is_literal_kind_spec();
        // An input that references a variable cannot be mapped:
        // the test is excluded from the denominator entirely.
        spec.tests.push(Test {
            name: Some("unmappable".into()),
            input: Expr::Var("kind".into()),
            expected_output: Expr::BoolLit(true),
            timeout_ms: None,
            property: false,
        });
        let correct = binop(IRBinOp::Eq, var("kind"), int(0));
        let run = run_spec_tests(&correct, &spec);
        assert_eq!(run, TestRun { evaluable: 3, passed: 3 });
    }

    // ── Tuple-form test inputs (Phase 53) ─────────────────
    //
    // The parser lowers a tuple input `(v1, v2, ...)` to
    // Call("__tuple__", [v1, v2, ...]) (parser.rs) and a list
    // literal to Call("List", [...]). Before the Phase 53 fix,
    // bind_test_env mapped the input as ONE value; a
    // "__tuple__" call is not an evaluable function, so every
    // tuple test was excluded (evaluable == 0) and multi-param
    // specs fell to evaluate_candidate's FALLBACK — tpr 1.0
    // for EVERY candidate (the Phase 52 finding: sort's
    // crowned winners were trivial bodies).

    fn list_lit(items: &[i64]) -> Expr {
        Expr::Call(
            "List".into(),
            items.iter().map(|n| Expr::IntLit(*n)).collect(),
        )
    }

    fn tuple(parts: Vec<Expr>) -> Expr {
        Expr::Call("__tuple__".into(), parts)
    }

    fn two_int_params() -> Vec<Param> {
        ["a", "b"]
            .iter()
            .map(|name| Param {
                name: name.to_string(),
                ty: crate::spec::ast::TypeRef::Int,
                annotations: Default::default(),
            })
            .collect()
    }

    fn test(input: Expr, expected_output: Expr) -> Test {
        Test {
            name: None,
            input,
            expected_output,
            timeout_ms: None,
            property: false,
        }
    }

    #[test]
    fn run_spec_tests_tuple_input_binds_elementwise() {
        // (a: Int, b: Int) -> Int, tuple tests — including one
        // composite element (1 + 2), which maps through the
        // same value_of_expr path as a whole input.
        let mut spec = dummy_spec_with(
            "add_pair",
            two_int_params(),
            crate::spec::ast::TypeRef::Int,
        );
        spec.tests = vec![
            test(
                tuple(vec![Expr::IntLit(1), Expr::IntLit(2)]),
                Expr::IntLit(3),
            ),
            test(
                tuple(vec![Expr::IntLit(10), Expr::IntLit(5)]),
                Expr::IntLit(15),
            ),
            test(
                tuple(vec![Expr::IntLit(0), Expr::IntLit(0)]),
                Expr::IntLit(0),
            ),
            test(
                tuple(vec![
                    Expr::BinOp(
                        SpecBinOp::Add,
                        Box::new(Expr::IntLit(1)),
                        Box::new(Expr::IntLit(2)),
                    ),
                    Expr::IntLit(4),
                ]),
                Expr::IntLit(7),
            ),
        ];
        // Correct: a + b passes all four.
        let correct = binop(IRBinOp::Add, var("a"), var("b"));
        let run = run_spec_tests(&correct, &spec);
        assert_eq!(run, TestRun { evaluable: 4, passed: 4 });
        // Trivial: returns the first parameter — passes only
        // the (0, 0) test. Before the fix both bodies saw
        // evaluable == 0 here (and tpr 1.0 via the FALLBACK).
        let trivial = var("a");
        let run = run_spec_tests(&trivial, &spec);
        assert_eq!(run, TestRun { evaluable: 4, passed: 1 });
    }

    #[test]
    fn run_spec_tests_tuple_with_list_element_binds() {
        // Mirrors examples/self_host/count_matching.morph:
        // (list: List<Int>, threshold: Int) with tuple tests
        // whose first element is a list literal.
        let mut spec = dummy_spec_with(
            "count_matching_shape",
            vec![
                Param {
                    name: "list".into(),
                    ty: crate::spec::ast::TypeRef::List(Box::new(
                        crate::spec::ast::TypeRef::Int,
                    )),
                    annotations: Default::default(),
                },
                Param {
                    name: "threshold".into(),
                    ty: crate::spec::ast::TypeRef::Int,
                    annotations: Default::default(),
                },
            ],
            crate::spec::ast::TypeRef::Int,
        );
        spec.tests = vec![
            test(tuple(vec![list_lit(&[10, 20, 30]), Expr::IntLit(5)]), Expr::IntLit(3)),
            test(tuple(vec![list_lit(&[1, 2, 3]), Expr::IntLit(5)]), Expr::IntLit(0)),
            test(tuple(vec![list_lit(&[1, 10, 2, 20]), Expr::IntLit(5)]), Expr::IntLit(2)),
        ];
        // The bindings are exactly the element values.
        let env = bind_test_env(&spec.params, &spec.tests[0].input)
            .expect("tuple input with a list element must bind");
        assert_eq!(
            env.get("list"),
            Some(&Value::List(vec![Value::Int(10), Value::Int(20), Value::Int(30)]))
        );
        assert_eq!(env.get("threshold"), Some(&Value::Int(5)));
        // len(list) passes exactly the first test; the constant
        // 0 passes exactly the second. The point: all three
        // tests now COUNT (evaluable 3, was 0) and the counts
        // are the bodies' real pass counts.
        let len_body = IRNode::UnaryOp(IRUnaryOp::Len, Box::new(var("list")));
        let run = run_spec_tests(&len_body, &spec);
        assert_eq!(run, TestRun { evaluable: 3, passed: 1 });
        let run = run_spec_tests(&int(0), &spec);
        assert_eq!(run, TestRun { evaluable: 3, passed: 1 });
    }

    #[test]
    fn run_spec_tests_tuple_with_lambda_element_stays_excluded() {
        // sort.morph's shape: (list, cmp) where cmp's test
        // value is a lambda. The evaluator has no function
        // values, so the element cannot be mapped and the test
        // stays excluded — the FALLBACK's remaining legitimate
        // domain, unchanged by the tuple fix.
        let mut spec = dummy_spec_with(
            "sort_shape",
            vec![
                Param {
                    name: "list".into(),
                    ty: crate::spec::ast::TypeRef::List(Box::new(
                        crate::spec::ast::TypeRef::Int,
                    )),
                    annotations: Default::default(),
                },
                Param {
                    name: "cmp".into(),
                    ty: crate::spec::ast::TypeRef::Function(
                        vec![
                            crate::spec::ast::TypeRef::Int,
                            crate::spec::ast::TypeRef::Int,
                        ],
                        Box::new(crate::spec::ast::TypeRef::Bool),
                    ),
                    annotations: Default::default(),
                },
            ],
            crate::spec::ast::TypeRef::List(Box::new(crate::spec::ast::TypeRef::Int)),
        );
        let lambda = Expr::Lambda(
            vec![
                ("a".into(), crate::spec::ast::TypeRef::Named("_".into())),
                ("b".into(), crate::spec::ast::TypeRef::Named("_".into())),
            ],
            Box::new(Expr::BinOp(
                SpecBinOp::Lt,
                Box::new(Expr::Var("a".into())),
                Box::new(Expr::Var("b".into())),
            )),
        );
        spec.tests = vec![test(
            tuple(vec![list_lit(&[3, 1, 2]), lambda]),
            list_lit(&[1, 2, 3]),
        )];
        let run = run_spec_tests(&var("list"), &spec);
        assert_eq!(run, TestRun { evaluable: 0, passed: 0 });
    }

    #[test]
    fn run_spec_tests_tuple_arity_mismatch_stays_excluded() {
        // A tuple whose arity differs from the parameter count
        // does not bind (it never bound before the fix either).
        let mut spec = dummy_spec_with(
            "add_pair",
            two_int_params(),
            crate::spec::ast::TypeRef::Int,
        );
        spec.tests = vec![test(
            tuple(vec![Expr::IntLit(1), Expr::IntLit(2), Expr::IntLit(3)]),
            Expr::IntLit(6),
        )];
        let correct = binop(IRBinOp::Add, var("a"), var("b"));
        let run = run_spec_tests(&correct, &spec);
        assert_eq!(run, TestRun { evaluable: 0, passed: 0 });
    }

    #[test]
    fn run_spec_tests_nested_tuple_stays_excluded() {
        // ((1, 2), 3) for a 2-param spec: the inner tuple is an
        // element, and elements map through value_of_expr,
        // which has no tuple value — the test stays excluded.
        // (No repo spec uses this shape; recorded, not handled.)
        let mut spec = dummy_spec_with(
            "add_pair",
            two_int_params(),
            crate::spec::ast::TypeRef::Int,
        );
        spec.tests = vec![test(
            tuple(vec![
                tuple(vec![Expr::IntLit(1), Expr::IntLit(2)]),
                Expr::IntLit(3),
            ]),
            Expr::IntLit(6),
        )];
        let correct = binop(IRBinOp::Add, var("a"), var("b"));
        let run = run_spec_tests(&correct, &spec);
        assert_eq!(run, TestRun { evaluable: 0, passed: 0 });
    }

    #[test]
    fn run_spec_tests_list_form_multi_param_binding_unchanged() {
        // The pre-existing multi-param arm, pinned: the input
        // as a whole evaluates to a List of exactly the
        // parameter count and binds elementwise from the List.
        let mut spec = dummy_spec_with(
            "add_pair",
            two_int_params(),
            crate::spec::ast::TypeRef::Int,
        );
        spec.tests = vec![test(list_lit(&[1, 2]), Expr::IntLit(3))];
        let correct = binop(IRBinOp::Add, var("a"), var("b"));
        let run = run_spec_tests(&correct, &spec);
        assert_eq!(run, TestRun { evaluable: 1, passed: 1 });
    }

    // ── Records, Result/Option values, method calls ─────
    // (Phase 75, M1 features: Value::Record + the
    // FieldAccess / RecordLit evaluation arms, the
    // Ok/Err/Some/None constructor calls, and spec-level
    // MethodCall evaluation for the D1a builtin table.)

    fn chr(c: char) -> IRNode {
        IRNode::Literal(IRLiteral::Char(c))
    }

    fn field(base: IRNode, name: &str) -> IRNode {
        IRNode::FieldAccess {
            base: Box::new(base),
            field: name.into(),
        }
    }

    fn call_ir(name: &str, args: Vec<IRNode>) -> IRNode {
        IRNode::Call {
            function: name.into(),
            args,
        }
    }

    fn record_val(type_name: &str, fields: Vec<(&str, Value)>) -> Value {
        Value::Record {
            type_name: type_name.into(),
            fields: fields
                .into_iter()
                .map(|(n, v)| (n.to_string(), v))
                .collect(),
        }
    }

    fn token_val(kind: i64, lexeme: &str, line: i64, col: i64) -> Value {
        record_val(
            "Token",
            vec![
                ("kind", Value::Int(kind)),
                ("lexeme", Value::Str(lexeme.into())),
                ("line", Value::Int(line)),
                ("col", Value::Int(col)),
            ],
        )
    }

    fn state_val(source: &str, pos: i64, line: i64, col: i64, tokens: Vec<Value>) -> Value {
        record_val(
            "TokenizerState",
            vec![
                ("source", Value::Str(source.into())),
                ("pos", Value::Int(pos)),
                ("line", Value::Int(line)),
                ("col", Value::Int(col)),
                ("tokens", Value::List(tokens)),
            ],
        )
    }

    #[test]
    fn eval_record_lit_constructs_record_value_in_written_order() {
        // Token { kind: 0, lexeme: "spec", line: 1, col: 1 } —
        // the tokenizer's nested-literal shape.
        let lit = IRNode::RecordLit {
            type_name: "Token".into(),
            fields: vec![
                ("kind".into(), int(0)),
                ("lexeme".into(), string("spec")),
                ("line".into(), int(1)),
                ("col".into(), int(1)),
            ],
        };
        assert_eq!(
            eval(&lit, &Env::new()),
            Ok(token_val(0, "spec", 1, 1)),
            "a record literal must evaluate to a record value carrying its type name"
        );

        // Field values are full expressions, evaluated: a
        // state whose pos field is 0 + 1 and whose tokens
        // field is a list holding a record literal.
        let state = IRNode::RecordLit {
            type_name: "TokenizerState".into(),
            fields: vec![
                ("source".into(), string("spec")),
                ("pos".into(), binop(IRBinOp::Add, int(0), int(1))),
                ("line".into(), int(1)),
                ("col".into(), int(2)),
                (
                    "tokens".into(),
                    IRNode::Collection {
                        kind: CollectionKind::List,
                        elements: vec![IRNode::RecordLit {
                            type_name: "Token".into(),
                            fields: vec![
                                ("kind".into(), int(0)),
                                ("lexeme".into(), string("spec")),
                                ("line".into(), int(1)),
                                ("col".into(), int(1)),
                            ],
                        }],
                    },
                ),
            ],
        };
        assert_eq!(
            eval(&state, &Env::new()),
            Ok(state_val("spec", 1, 1, 2, vec![token_val(0, "spec", 1, 1)]))
        );
    }

    #[test]
    fn eval_record_lit_field_failures_surface_in_written_order() {
        // Two failing fields: the FIRST field's failure is
        // the reported one (fields evaluate in written
        // order, like Collection elements).
        let lit = IRNode::RecordLit {
            type_name: "Token".into(),
            fields: vec![
                (
                    "kind".into(),
                    IRNode::Hole {
                        id: 0,
                        expected_type: None,
                    },
                ),
                ("lexeme".into(), var("unbound")),
            ],
        };
        match eval(&lit, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("Hole"),
                    "the first field's failure must surface, got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }

        // A Return inside a field value unwinds the whole
        // evaluation, exactly as it does inside a Collection.
        let lit = IRNode::RecordLit {
            type_name: "Token".into(),
            fields: vec![("kind".into(), IRNode::Return(Box::new(int(3))))],
        };
        assert_eq!(eval(&lit, &Env::new()), Ok(Value::Int(3)));
    }

    #[test]
    fn eval_field_access_projects_named_fields() {
        let env = env_with(&[(
            "state",
            state_val("spec", 4, 1, 5, vec![token_val(0, "spec", 1, 1)]),
        )]);
        // Scalar projection.
        assert_eq!(
            eval(&field(var("state"), "pos"), &env),
            Ok(Value::Int(4))
        );
        // Projection of a list-of-records field, then index,
        // then projection again: state.tokens[0].lexeme.
        let lexeme = field(
            IRNode::Index(
                Box::new(field(var("state"), "tokens")),
                Box::new(int(0)),
            ),
            "lexeme",
        );
        assert_eq!(eval(&lexeme, &env), Ok(Value::Str("spec".into())));
        // Projection directly off a record literal.
        let lit = IRNode::RecordLit {
            type_name: "Token".into(),
            fields: vec![
                ("kind".into(), int(7)),
                ("lexeme".into(), string("x")),
                ("line".into(), int(1)),
                ("col".into(), int(1)),
            ],
        };
        assert_eq!(
            eval(&field(lit, "kind"), &Env::new()),
            Ok(Value::Int(7))
        );
    }

    #[test]
    fn eval_field_access_failures_are_named_not_guessed() {
        let env = env_with(&[("state", state_val("hi", 0, 1, 1, vec![]))]);
        // Unknown field: the reason names the field.
        match eval(&field(var("state"), "bogus"), &env) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("bogus"),
                    "unknown-field failure must name the field, got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
        // Non-record base: named, not a panic or a guess.
        match eval(&field(int(3), "pos"), &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("pos"),
                    "non-record failure must name the field, got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
        // A base that itself cannot evaluate propagates its
        // own reason.
        match eval(
            &field(
                IRNode::Hole {
                    id: 0,
                    expected_type: None,
                },
                "pos",
            ),
            &Env::new(),
        ) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(reason.contains("Hole"), "got: {reason}");
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
    }

    #[test]
    fn eval_constructor_calls_build_result_and_option_values() {
        let env = Env::new();
        assert_eq!(
            eval(&call_ir("Ok", vec![int(5)]), &env),
            Ok(Value::Ok(Box::new(Value::Int(5))))
        );
        assert_eq!(
            eval(&call_ir("Err", vec![string("boom")]), &env),
            Ok(Value::Err(Box::new(Value::Str("boom".into()))))
        );
        assert_eq!(
            eval(&call_ir("Some", vec![chr('h')]), &env),
            Ok(Value::Some(Box::new(Value::Char('h'))))
        );
        assert_eq!(eval(&call_ir("None", vec![]), &env), Ok(Value::None));
        // Constructors compose: Ok(Some(1)).
        assert_eq!(
            eval(&call_ir("Ok", vec![call_ir("Some", vec![int(1)])]), &env),
            Ok(Value::Ok(Box::new(Value::Some(Box::new(Value::Int(1))))))
        );
        // Wrong arity is a named failure, never a guess.
        for bad in [
            call_ir("Ok", vec![]),
            call_ir("Ok", vec![int(1), int(2)]),
            call_ir("Err", vec![]),
            call_ir("Some", vec![]),
            call_ir("None", vec![int(1)]),
        ] {
            assert!(
                matches!(eval(&bad, &env), Err(EvalError::NotEvaluable(_))),
                "expected NotEvaluable for {bad:?}"
            );
        }
    }

    #[test]
    fn eval_method_call_builtins_and_named_failures() {
        let ok5 = Value::Ok(Box::new(Value::Int(5)));
        let err = Value::Err(Box::new(Value::Str("boom".into())));
        // The D1a Result table.
        assert_eq!(eval_method_call(&ok5, "is_ok", &[]), Ok(Value::Bool(true)));
        assert_eq!(eval_method_call(&ok5, "is_err", &[]), Ok(Value::Bool(false)));
        assert_eq!(eval_method_call(&err, "is_ok", &[]), Ok(Value::Bool(false)));
        assert_eq!(eval_method_call(&err, "is_err", &[]), Ok(Value::Bool(true)));
        assert_eq!(eval_method_call(&ok5, "unwrap", &[]), Ok(Value::Int(5)));
        // unwrap on an Err value: a named eval failure.
        match eval_method_call(&err, "unwrap", &[]) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("unwrap") && reason.contains("Err"),
                    "unwrap-on-Err failure must name the operation, got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
        // The D1a List table.
        let list = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(eval_method_call(&list, "len", &[]), Ok(Value::Int(3)));
        // Wrong family, unknown method, arguments to a
        // zero-arg builtin, method on a value with no table:
        // each a named failure naming the method.
        for (receiver, name, args) in [
            (ok5.clone(), "len", vec![]),
            (list.clone(), "is_ok", vec![]),
            (ok5.clone(), "bogus", vec![]),
            (Value::Int(1), "is_ok", vec![]),
            (ok5.clone(), "is_ok", vec![Value::Int(1)]),
        ] {
            match eval_method_call(&receiver, name, &args) {
                Err(EvalError::NotEvaluable(reason)) => {
                    assert!(
                        reason.contains(name),
                        "failure must name the method `{name}`, got: {reason}"
                    );
                }
                other => panic!("expected NotEvaluable for {name} on {receiver:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn value_of_expr_evaluates_method_calls_on_closed_receivers() {
        let ok_int = |n: i64| Expr::Call("Ok".into(), vec![Expr::IntLit(n)]);
        let method = |receiver: Expr, name: &str| Expr::MethodCall {
            receiver: Box::new(receiver),
            name: name.into(),
            args: vec![],
        };
        assert_eq!(
            value_of_expr(&method(ok_int(5), "is_ok")),
            Some(Value::Bool(true))
        );
        assert_eq!(
            value_of_expr(&method(
                Expr::Call("Err".into(), vec![Expr::StringLit("x".into())]),
                "is_err"
            )),
            Some(Value::Bool(true))
        );
        assert_eq!(
            value_of_expr(&method(ok_int(5), "unwrap")),
            Some(Value::Int(5))
        );
        // Chained: Ok([1, 2]).unwrap().len() — the
        // tokenize_all constraint shape, on a closed receiver.
        let chained = method(
            method(
                Expr::Call(
                    "Ok".into(),
                    vec![Expr::Call(
                        "List".into(),
                        vec![Expr::IntLit(1), Expr::IntLit(2)],
                    )],
                ),
                "unwrap",
            ),
            "len",
        );
        assert_eq!(value_of_expr(&chained), Some(Value::Int(2)));
        // unwrap on Err has no value: unmappable, like every
        // other failure in value_of_expr's Option channel.
        assert_eq!(
            value_of_expr(&method(
                Expr::Call("Err".into(), vec![Expr::StringLit("x".into())]),
                "unwrap"
            )),
            None
        );
        // A method on a free variable cannot be mapped (the
        // receiver has no closed value) — constraints over
        // variables are judged by Z3 / the compiled runner,
        // not by this mapping.
        assert_eq!(
            value_of_expr(&method(Expr::Var("output".into()), "is_err")),
            None
        );
        // Unknown methods and wrong-family methods do not map.
        assert_eq!(value_of_expr(&method(ok_int(5), "bogus")), None);
        assert_eq!(value_of_expr(&method(ok_int(5), "len")), None);
    }

    #[test]
    fn value_of_expr_maps_record_and_constructor_test_forms() {
        let token_lit = Expr::RecordLit {
            type_name: "Token".into(),
            fields: vec![
                ("kind".into(), Expr::IntLit(0)),
                ("lexeme".into(), Expr::StringLit("spec".into())),
                ("line".into(), Expr::IntLit(1)),
                ("col".into(), Expr::IntLit(1)),
            ],
        };
        // A bare record literal maps to a record value.
        assert_eq!(
            value_of_expr(&token_lit),
            Some(token_val(0, "spec", 1, 1))
        );
        // Field projection over a closed record literal.
        let state_lit = Expr::RecordLit {
            type_name: "TokenizerState".into(),
            fields: vec![
                ("source".into(), Expr::StringLit("hi".into())),
                ("pos".into(), Expr::IntLit(0)),
                ("line".into(), Expr::IntLit(1)),
                ("col".into(), Expr::IntLit(1)),
                (
                    "tokens".into(),
                    Expr::Call("List".into(), vec![token_lit.clone()]),
                ),
            ],
        };
        assert_eq!(
            value_of_expr(&state_lit),
            Some(state_val("hi", 0, 1, 1, vec![token_val(0, "spec", 1, 1)]))
        );
        assert_eq!(
            value_of_expr(&Expr::Field(
                Box::new(state_lit.clone()),
                "pos".into()
            )),
            Some(Value::Int(0))
        );
        // Ok(record) — tokenize_one's expected-output shape.
        assert_eq!(
            value_of_expr(&Expr::Call("Ok".into(), vec![state_lit])),
            Some(Value::Ok(Box::new(state_val(
                "hi",
                0,
                1,
                1,
                vec![token_val(0, "spec", 1, 1)]
            ))))
        );
        // Ok([Token, Token]) — tokenize_all's shape: records
        // nested inside a list literal inside a constructor.
        let two_tokens = Expr::Call(
            "Ok".into(),
            vec![Expr::Call(
                "List".into(),
                vec![token_lit.clone(), token_lit],
            )],
        );
        assert_eq!(
            value_of_expr(&two_tokens),
            Some(Value::Ok(Box::new(Value::List(vec![
                token_val(0, "spec", 1, 1),
                token_val(0, "spec", 1, 1)
            ]))))
        );
        // Some('h') / None() — peek's expected outputs.
        assert_eq!(
            value_of_expr(&Expr::Call("Some".into(), vec![Expr::CharLit('h')])),
            Some(Value::Some(Box::new(Value::Char('h'))))
        );
        assert_eq!(
            value_of_expr(&Expr::Call("None".into(), vec![])),
            Some(Value::None)
        );
    }

    #[test]
    fn try_propagate_remains_outside_the_evaluable_subset() {
        // Residual, pinned deliberately: Result VALUES exist
        // since this phase, but `?` propagation semantics in
        // the interpreter do not — a body using TryPropagate
        // is still not evaluable in-process, and the reason
        // must not claim the old cause (no Result values).
        let node = IRNode::TryPropagate(Box::new(call_ir("Ok", vec![int(1)])));
        match eval(&node, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    !reason.contains("no Result values"),
                    "the stale reason must not survive: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
    }

    // ── Headline evidence (Phase 75): the restored ──────
    // tokenizer module's record-carrying specs bind their
    // tests in-process and are judged by the interpreter.
    //
    // The module text is the Phase 73/74 restored scratch,
    // embedded verbatim: the current module's types, the
    // dropped tokenize_one/tokenize_all constraints restored
    // (acda3f9), and the 9 mirror-only tests restored in the
    // D3a named form. examples/functional_tokenizer.morph
    // itself is NOT touched by this phase (Phase 79 owns the
    // file); the classifiers' specs are read from it below,
    // since their tests never left the file.

    const RESTORED_TOKENIZER_MODULE: &str = r#"ScratchTokenizer {

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

    fn load_specs_from(source: &str) -> Vec<FunctionSpec> {
        let parsed = crate::spec::parser::parse(source).expect("module must parse");
        let checked = crate::spec::typeck::check(parsed).expect("module must typecheck");
        checked.spec.functions
    }

    fn spec_named<'a>(specs: &'a [FunctionSpec], name: &str) -> &'a FunctionSpec {
        specs
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("spec {name} must exist"))
    }

    fn peek_body() -> IRNode {
        // if state.pos < len(state.source)
        //   then Some(state.source[state.pos]) else None()
        // — the mirror's peek (tests/tokenizer_proof.rs):
        // chars().nth(pos).
        IRNode::If {
            condition: Box::new(binop(
                IRBinOp::Lt,
                field(var("state"), "pos"),
                call_ir("len", vec![field(var("state"), "source")]),
            )),
            then_branch: Box::new(call_ir(
                "Some",
                vec![IRNode::Index(
                    Box::new(field(var("state"), "source")),
                    Box::new(field(var("state"), "pos")),
                )],
            )),
            else_branch: Box::new(call_ir("None", vec![])),
        }
    }

    fn next_char_body() -> IRNode {
        // The mirror's next_char: pos + 1; on '\n' line + 1
        // and col = 1, else col + 1 (line carried).
        let is_newline = binop(
            IRBinOp::And,
            binop(
                IRBinOp::Lt,
                field(var("state"), "pos"),
                call_ir("len", vec![field(var("state"), "source")]),
            ),
            binop(
                IRBinOp::Eq,
                IRNode::Index(
                    Box::new(field(var("state"), "source")),
                    Box::new(field(var("state"), "pos")),
                ),
                chr('\n'),
            ),
        );
        IRNode::RecordLit {
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
        }
    }

    #[test]
    fn tokenizer_record_specs_bind_and_are_judged() {
        let specs = load_specs_from(RESTORED_TOKENIZER_MODULE);

        // Every record-carrying spec's tests now BIND
        // in-process (Phase 48: "nothing to bind `state`
        // to" — all five were unbindable).
        for (name, count) in [
            ("peek", 2),
            ("next_char", 1),
            ("skip_whitespace", 2),
            ("tokenize_one", 2),
            ("tokenize_all", 2),
        ] {
            let spec = spec_named(&specs, name);
            assert_eq!(
                test_bindability(spec),
                vec![true; count],
                "{name}: every restored test must bind in-process"
            );
            assert_eq!(bindable_test_count(spec), count);
        }

        // peek: the mirror body passes both tests.
        let peek = spec_named(&specs, "peek");
        assert_eq!(
            test_outcomes(&peek_body(), peek),
            vec![TestOutcome::Passed, TestOutcome::Passed]
        );
        assert_eq!(
            run_spec_tests(&peek_body(), peek),
            TestRun {
                evaluable: 2,
                passed: 2
            }
        );

        // next_char: the mirror body passes its test.
        let next_char = spec_named(&specs, "next_char");
        assert_eq!(
            test_outcomes(&next_char_body(), next_char),
            vec![TestOutcome::Passed]
        );
        assert_eq!(
            run_spec_tests(&next_char_body(), next_char),
            TestRun {
                evaluable: 1,
                passed: 1
            }
        );

        // skip_whitespace: the mirror body is a loop, which
        // stays outside the evaluable subset — the tests
        // bind and count as evaluable, and the outcome says
        // exactly why no verdict was produced.
        let skip = spec_named(&specs, "skip_whitespace");
        let loop_body = IRNode::While {
            condition: Box::new(boolean(true)),
            body: Box::new(var("state")),
            invariant: None,
        };
        let outcomes = test_outcomes(&loop_body, skip);
        assert!(
            outcomes
                .iter()
                .all(|o| matches!(o, TestOutcome::BodyNotEvaluable(_))),
            "skip_whitespace outcomes must be BodyNotEvaluable, got {outcomes:?}"
        );
        assert_eq!(
            run_spec_tests(&loop_body, skip),
            TestRun {
                evaluable: 2,
                passed: 0
            }
        );

        // tokenize_one: the partial body Ok(state) — correct
        // at end-of-input, wrong on "keyword-spec" (it does
        // not append the token). Judged, honestly split.
        let one = spec_named(&specs, "tokenize_one");
        let identity_step = call_ir("Ok", vec![var("state")]);
        assert_eq!(
            test_outcomes(&identity_step, one),
            vec![TestOutcome::Failed, TestOutcome::Passed]
        );
        assert_eq!(
            run_spec_tests(&identity_step, one),
            TestRun {
                evaluable: 2,
                passed: 1
            }
        );

        // tokenize_all: the partial body Ok(state.tokens) —
        // correct on "empty", wrong on "simple-spec".
        let all = spec_named(&specs, "tokenize_all");
        let tokens_of_state = call_ir("Ok", vec![field(var("state"), "tokens")]);
        assert_eq!(
            test_outcomes(&tokens_of_state, all),
            vec![TestOutcome::Failed, TestOutcome::Passed]
        );
        assert_eq!(
            run_spec_tests(&tokens_of_state, all),
            TestRun {
                evaluable: 2,
                passed: 1
            }
        );
    }

    #[test]
    fn tokenizer_classifier_bodies_pass_and_match_the_mirror() {
        // The classifiers never carried records — their
        // in-spec tests bind as they always did. The bodies
        // below transcribe tests/tokenizer_proof.rs lines
        // 60–70 exactly; passing the specs' tests with them
        // cross-checks the interpreter against the mirror.
        let root = env!("CARGO_MANIFEST_DIR");
        let source = std::fs::read_to_string(format!(
            "{root}/examples/functional_tokenizer.morph"
        ))
        .unwrap();
        let specs = load_specs_from(&source);
        let c = || var("c");
        let char_range = |lo: char, hi: char| {
            binop(
                IRBinOp::And,
                binop(IRBinOp::Ge, c(), chr(lo)),
                binop(IRBinOp::Le, c(), chr(hi)),
            )
        };
        let ident_start = binop(
            IRBinOp::Or,
            binop(IRBinOp::Or, char_range('a', 'z'), char_range('A', 'Z')),
            binop(IRBinOp::Eq, c(), chr('_')),
        );
        let bodies: Vec<(&str, IRNode)> = vec![
            (
                "is_whitespace",
                [(" ", ' '), ("\t", '\t'), ("\n", '\n'), ("\r", '\r')]
                    .into_iter()
                    .map(|(_, ch)| binop(IRBinOp::Eq, c(), chr(ch)))
                    .reduce(|a, b| binop(IRBinOp::Or, a, b))
                    .unwrap(),
            ),
            ("is_ident_start", ident_start.clone()),
            (
                "is_ident_continue",
                binop(IRBinOp::Or, ident_start, char_range('0', '9')),
            ),
        ];
        for (name, body) in &bodies {
            let spec = spec_named(&specs, name);
            assert_eq!(
                run_spec_tests(body, spec),
                TestRun {
                    evaluable: 3,
                    passed: 3
                },
                "{name}: the mirror body must pass all in-spec tests"
            );
        }

        // Mirror cross-check beyond the specs' own tests:
        // peek / next_char on inputs the tests do not carry,
        // against the mirror's documented behavior.
        let env = env_with(&[("state", state_val("hi", 1, 1, 2, vec![]))]);
        assert_eq!(
            eval(&peek_body(), &env),
            Ok(Value::Some(Box::new(Value::Char('i')))),
            "mirror peek at pos 1 of \"hi\" is Some('i')"
        );
        let env = env_with(&[("state", state_val("hi", 2, 1, 3, vec![]))]);
        assert_eq!(
            eval(&peek_body(), &env),
            Ok(Value::None),
            "mirror peek at end of input is None"
        );
        let env = env_with(&[("state", state_val("a\nb", 1, 1, 2, vec![]))]);
        assert_eq!(
            eval(&next_char_body(), &env),
            Ok(state_val("a\nb", 2, 2, 1, vec![])),
            "mirror next_char over '\\n' advances line and resets col"
        );
    }
}
