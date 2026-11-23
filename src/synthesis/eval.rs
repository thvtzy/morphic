// ┌──────────────────────────────────────────────────────────┐
// │  IR INTERPRETER — candidate test execution               │
// │                                                          │
// │  A small, total evaluator for the IR. `evaluate_         │
// │  candidate` uses it to run a candidate body against a    │
// │  spec's tests and measure a real pass ratio, instead     │
// │  of assuming every test passes.                          │
// │                                                          │
// │  The evaluable subset is the expression subset plus,     │
// │  since Phase 95 (encoder extension, W9), the statement/  │
// │  loop subset the denoted and template bodies are made    │
// │  of: Block sequencing (always present), Alloc, Assign,   │
// │  While, Loop, Break, TryPropagate, constructor-pattern   │
// │  Match, and Clone — and, since Phase 115 (the List       │
// │  family's judging machinery), the bounded For scan the   │
// │  compiled runner's bodies are written in — and, since    │
// │  Phase 130 (G4), function values: Lambda/Closure         │
// │  evaluate to closures and a call through a bound         │
// │  function value applies it, under the same budget —      │
// │  and, since Phase 131 (G4), the higher-order             │
// │  combinators (Map/Filter/Fold/Sort, applying their       │
// │  function arguments through those closures), Assign to   │
// │  an indexed place, a Let body evaluated in the current   │
// │  environment (its Assigns to pre-existing bindings take  │
// │  effect, as compiled; its bindings stay scoped), and     │
// │  the bare-constructor `Var("None")` idiom — and, since   │
// │  Phase 132 (G4), named-reference function supply: where  │
// │  a judging route carries a module index, a bare name in  │
// │  a function position in a test row resolves through it   │
// │  to a function value built from the named spec's         │
// │  denoted body (§2.5's second supply form).               │
// │  Loops carry an explicit iteration bound (see            │
// │  MAX_LOOP_ITERATIONS): exceeding it is a judged          │
// │  `NotEvaluable`, never a hang and never a fabricated     │
// │  value; closure application spends from the same         │
// │  budget, so it is not an unbounded recursion channel     │
// │  beside it. Anything still outside the subset — Holes,   │
// │  calls to functions that are not known pure builtins or  │
// │  bound function values — returns                         │
// │  `EvalError::NotEvaluable` with a reason. The            │
// │  evaluator never panics and never guesses a value.       │
// └──────────────────────────────────────────────────────────┘

use super::engine::{IRBinOp, IRLiteral, IRNode, IRPattern, IRUnaryOp, SpecIndex};
use crate::spec::ast::{Constraint, Expr, FunctionSpec, OrderOp, Param, TypeRef};
use std::collections::HashMap;

/// A runtime value in the evaluable subset.
#[derive(Debug, Clone)]
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
    /// A function value (Phase 130, G4 — G4_SCOPE §2.1, the
    /// ONE function variant): the parameter names, the body,
    /// and the captured environment — a by-value snapshot of
    /// bindings taken where the lambda was evaluated (§2.2;
    /// for the IR `Closure` node, exactly its named captures
    /// resolved against the enclosing environment). Applying
    /// it evaluates the body in the captured environment
    /// extended with the argument bindings (§2.3).
    ///
    /// There is NO equality on function values (§2.6, D5a):
    /// the manual `PartialEq` below never equates one, and
    /// `values_equal` — the judged comparison — declines
    /// them explicitly before any structural comparison can
    /// see one.
    Function {
        params: Vec<String>,
        body: Box<IRNode>,
        env: Env,
    },
}

/// Structural equality for the first-order variants — the
/// semantics the derived instance gave them, replicated by
/// hand because the derive cannot exist once `Value` holds
/// an `IRNode` body (the IR carries no `PartialEq`), and
/// must not exist for the function variant anyway: a
/// function value is unequal to everything, including
/// itself (G4_SCOPE §2.6 — there is no equality on function
/// values; judged comparisons decline them in
/// `values_equal`, and this instance is only the floor
/// under non-judged uses like pattern matching, where a
/// function value simply never matches a literal).
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            (Value::List(a), Value::List(b)) => a == b,
            (
                Value::Record {
                    type_name: t1,
                    fields: f1,
                },
                Value::Record {
                    type_name: t2,
                    fields: f2,
                },
            ) => t1 == t2 && f1 == f2,
            (Value::Ok(a), Value::Ok(b)) => a == b,
            (Value::Err(a), Value::Err(b)) => a == b,
            (Value::Some(a), Value::Some(b)) => a == b,
            (Value::None, Value::None) => true,
            _ => false,
        }
    }
}

/// Whether a value is, or anywhere contains, a function
/// value — the reach of the §2.6 no-equality rule: a judged
/// comparison that would have to compare a function value
/// anywhere inside its operands declines rather than
/// compares the rest.
fn contains_function(v: &Value) -> bool {
    match v {
        Value::Function { .. } => true,
        Value::List(items) => items.iter().any(contains_function),
        Value::Record { fields, .. } => {
            fields.iter().any(|(_, f)| contains_function(f))
        }
        Value::Ok(inner) | Value::Err(inner) | Value::Some(inner) => {
            contains_function(inner)
        }
        _ => false,
    }
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
/// candidate body, `Break` unwinds to the innermost enclosing
/// loop (Phase 95), everything else is a plain value.
enum Flow {
    Normal(Value),
    Returned(Value),
    /// A `Break` in flight: `Some(v)` for the valued form,
    /// `None` for the bare form. Only a loop may consume it;
    /// anywhere else it is the named failure "Break outside
    /// a loop" (see `into_value` and `eval`).
    Broke(Option<Value>),
}

impl Flow {
    /// The value of a flow at a position that needs a plain
    /// value (an operand, a scrutinee, a call argument…).
    /// `Returned` unwraps to its value — the standing
    /// laxness of the expression subset (a `Return` nested
    /// inside an expression is read as the value it carries),
    /// unchanged by Phase 95. A `Break` in flight at such a
    /// position has no enclosing loop to land in: a named
    /// failure, never a guessed value.
    fn into_value(self) -> Result<Value, EvalError> {
        match self {
            Flow::Normal(v) | Flow::Returned(v) => Ok(v),
            Flow::Broke(_) => Err(not_evaluable("Break outside a loop")),
        }
    }
}

/// The loop-iteration budget of one top-level `eval` call
/// (Phase 95): the total number of loop iterations —
/// nested loops share the one counter — an evaluation may
/// perform before the evaluation is judged `NotEvaluable`
/// ("loop iteration bound exceeded"). The bound exists so
/// in-process judging is total: a non-terminating candidate
/// (a `while true` body, a loop whose progress step is
/// missing) is a judged non-answer, never a hang.
///
/// Size rationale: the largest honest consumers are the
/// tokenizer compositions — the cross-validation batteries'
/// worst point totals a few hundred iterations (an outer
/// tokenize loop over a ≤ 25-char source, each step running
/// a skip loop and an identifier loop bounded by the source
/// length). 10,000 is more than an order of magnitude above
/// that and three orders above the in-spec tests, while
/// keeping one runaway evaluation in the millisecond range
/// (an iteration here is a handful of environment writes).
///
/// Since Phase 130 (G4, G4_SCOPE §2.3) the same counter is
/// also the application budget: applying a closure spends
/// one unit, so closure application is not an unbounded
/// recursion channel beside the loop bound — a
/// self-application bomb declines `NotEvaluable` against
/// this discipline (in practice against the application
/// DEPTH bound below, which it reaches first), exactly as
/// a runaway loop declines against the iteration bound.
const MAX_LOOP_ITERATIONS: usize = 10_000;

/// The application nesting bound of one evaluation
/// (Phase 130, G4_SCOPE §2.3's stack-facing half): at most
/// this many closure applications may be LIVE at once.
/// Sequential applications — a loop or a Map applying a
/// closure per element — nest one deep and are bounded by
/// the step counter alone; only NESTED application (a
/// closure body applying a closure that applies…) counts
/// here, because nested application is host-stack
/// recursion in the interpreter itself.
///
/// Size rationale, measured on this tree (debug build):
/// the application chain (eval_flow → eval_call →
/// apply_closure → eval_flow…) costs tens of KB of host
/// stack per live level, and a fat body (application under
/// If/BinOp layers) exhausts the smallest stack in the
/// building — the test harness's 2 MB threads — at ~30
/// nested applications. A bound of 10 keeps the worst case
/// near a third of that stack with the entry chain
/// included, and stands 5× above the deepest nesting any
/// honest body shape performs (apply_twice nests 2; the
/// templates' comparator applications nest 1). A body that
/// needs deeper application nesting declines here by name
/// — recursion in this language is a named-spec feature,
/// not an interpreter-application one (§2.2).
const MAX_APPLICATION_DEPTH: usize = 10;

/// The budget state of one top-level `eval` call: the
/// shared step counter (loop iterations + closure
/// applications) and the live application depth. One
/// struct, threaded exactly where the bare counter was
/// threaded before Phase 130.
struct Budget {
    steps: usize,
    depth: usize,
}

/// Evaluate an IR node in `env`. The environment is cloned
/// internally wherever a construct binds variables, so the
/// caller's env is never mutated.
pub fn eval(node: &IRNode, env: &Env) -> Result<Value, EvalError> {
    let mut working = env.clone();
    let mut budget = Budget {
        steps: MAX_LOOP_ITERATIONS,
        depth: 0,
    };
    match eval_flow(node, &mut working, &mut budget)? {
        Flow::Normal(v) | Flow::Returned(v) => Ok(v),
        Flow::Broke(_) => Err(not_evaluable("Break outside a loop")),
    }
}

fn eval_flow(
    node: &IRNode,
    env: &mut Env,
    budget: &mut Budget,
) -> Result<Flow, EvalError> {
    match node {
        IRNode::Literal(lit) => Ok(Flow::Normal(literal_value(lit))),

        IRNode::Var(name) => match env.get(name) {
            Some(v) => Ok(Flow::Normal(v.clone())),
            None => {
                // The bare-constructor idiom (Phase 131):
                // a body may carry the nullary constructor
                // as a bare Var — the binary_search
                // template's final `Return(Var("None"))` is
                // the exhibit — and the compiled route
                // renders the bare name as the variant
                // itself. An UNBOUND `None` therefore reads
                // as the constructor value here too. A
                // binding of the same name wins (the lookup
                // above ran first): the idiom is the
                // fallback for an unbound name, never an
                // override. Only the nullary constructor has
                // this form in bodies — `Some`/`Ok`/`Err`
                // are Calls (see eval_call's arms) and a
                // bare Var naming one still declines below.
                if name == "None" {
                    Ok(Flow::Normal(Value::None))
                } else {
                    Err(not_evaluable(format!(
                        "unbound variable `{}`",
                        name
                    )))
                }
            }
        },

        IRNode::BinOp(op, l, r) => {
            // Statement-position Append on a bound variable
            // rebinds it (Phase 95): the IR's Append renders
            // in the compiled runner as a `.push(...)` on the
            // binding, so the node is effectful wherever it
            // stands — the template bodies accumulate their
            // lexeme and token list exactly this way, as
            // statements whose value is then read back from
            // the variable. The verifier's statement layer
            // makes the same distinction (Phase 93: the
            // statement Append rebinds; the expression Append
            // is the pure form). Any other Append shape —
            // a non-variable target, an unbound variable —
            // takes the pure `eval_binop` form below (an
            // unbound variable then fails with the standard
            // unbound-variable reason when read).
            if *op == IRBinOp::Append {
                if let IRNode::Var(name) = &**l {
                    if let Some(current) = env.get(name).cloned() {
                        let item = eval_flow(r, env, budget)?.into_value()?;
                        let next = eval_binop(*op, current, item)?;
                        env.insert(name.clone(), next.clone());
                        return Ok(Flow::Normal(next));
                    }
                }
            }
            let lv = eval_flow(l, env, budget)?.into_value()?;
            let rv = eval_flow(r, env, budget)?.into_value()?;
            Ok(Flow::Normal(eval_binop(*op, lv, rv)?))
        }

        IRNode::UnaryOp(op, inner) => {
            let v = eval_flow(inner, env, budget)?.into_value()?;
            Ok(Flow::Normal(eval_unaryop(*op, v)?))
        }

        IRNode::If { condition, then_branch, else_branch } => {
            match eval_flow(condition, env, budget)?.into_value()? {
                Value::Bool(true) => eval_flow(then_branch, env, budget),
                Value::Bool(false) => eval_flow(else_branch, env, budget),
                other => Err(not_evaluable(format!(
                    "if condition is not Bool (got {:?})",
                    other
                ))),
            }
        }

        IRNode::Let { name, value, body } => {
            let v = match eval_flow(value, env, budget)? {
                Flow::Returned(v) => return Ok(Flow::Returned(v)),
                Flow::Broke(b) => return Ok(Flow::Broke(b)),
                Flow::Normal(v) => v,
            };
            // The body evaluates in the CURRENT environment
            // (Phase 131) — the compiled semantics: the
            // emitter renders `let name = value;` followed
            // by the body as the continuation of the same
            // scope, so an Assign (or a statement Append) to
            // a binding that PRE-DATES the Let takes effect
            // exactly as it does compiled. Before this phase
            // the body ran in a clone and those mutations
            // were discarded on exit — a loop whose progress
            // step sits inside a Let body (the binary_search
            // template's While) never progressed in-process
            // while its compiled form terminated: a
            // statement-subset divergence, closed here.
            //
            // What stays scoped is BINDING, not mutation —
            // the clone's binding discipline, kept entire:
            // when the body completes (on any exit — value,
            // Return, Break, or failure), the Let's own
            // binding is restored to its pre-Let state and
            // every name the body created (an Alloc's
            // binding, a nested construct's leftover) is
            // removed. A read after the Let therefore sees
            // exactly the bindings it saw before, with
            // exactly the mutations the compiled form would
            // have made to them. The one deliberate
            // divergence from the compiled text: the Let's
            // own name, if the body rebinds it, is restored
            // rather than carried — a Let used as a block
            // statement is not the tail, and the clone's
            // scoping of the name itself was never the
            // divergence being closed.
            let saved = env.insert(name.clone(), v);
            let pre_existing: std::collections::HashSet<String> =
                env.keys().cloned().collect();
            let result = eval_flow(body, env, budget);
            env.retain(|key, _| pre_existing.contains(key));
            match saved {
                Some(previous) => {
                    env.insert(name.clone(), previous);
                }
                None => {
                    env.remove(name);
                }
            }
            result
        }

        IRNode::Block(stmts) => {
            let mut last = Value::Unit;
            for stmt in stmts {
                match eval_flow(stmt, env, budget)? {
                    Flow::Returned(v) => return Ok(Flow::Returned(v)),
                    Flow::Broke(b) => return Ok(Flow::Broke(b)),
                    Flow::Normal(v) => last = v,
                }
            }
            Ok(Flow::Normal(last))
        }

        IRNode::Return(inner) => {
            let v = eval_flow(inner, env, budget)?.into_value()?;
            Ok(Flow::Returned(v))
        }

        IRNode::Match { scrutinee, cases } => {
            let scrut = eval_flow(scrutinee, env, budget)?.into_value()?;
            for (pattern, arm) in cases {
                match bind_pattern(pattern, &scrut)? {
                    Some(bindings) => {
                        let mut inner = env.clone();
                        inner.extend(bindings);
                        return eval_flow(arm, &mut inner, budget);
                    }
                    None => continue,
                }
            }
            Err(not_evaluable("no match arm matched the scrutinee"))
        }

        IRNode::Call { function, args } => {
            eval_call(function, args, env, budget).map(Flow::Normal)
        }

        // Function values (Phase 130, G4 §2.1/§2.2): a
        // Lambda evaluates to a closure capturing a by-value
        // snapshot of the environment in force here — the
        // whole environment, cloned. A lambda evaluated as a
        // Let's value therefore does NOT capture the Let's
        // own binding (the Let evaluates the value before
        // inserting it): no self-capture, no fixpoint.
        IRNode::Lambda { params, body } => Ok(Flow::Normal(Value::Function {
            params: params.iter().map(|(name, _)| name.clone()).collect(),
            body: body.clone(),
            env: env.clone(),
        })),

        // The Closure node (§2.2): captures resolve BY NAME
        // against the enclosing environment — exactly the
        // named bindings enter the snapshot, nothing else
        // (an in-scope binding the node does not name is not
        // captured, and the body declines it as unbound when
        // applied). An unbound capture name declines here,
        // by name, at creation.
        IRNode::Closure {
            params,
            captures,
            body,
        } => {
            let mut captured = Env::new();
            for name in captures {
                match env.get(name) {
                    Some(v) => {
                        captured.insert(name.clone(), v.clone());
                    }
                    None => {
                        return Err(not_evaluable(format!(
                            "Closure capture `{}` is not bound in the enclosing environment",
                            name
                        )));
                    }
                }
            }
            Ok(Flow::Normal(Value::Function {
                params: params.iter().map(|(name, _)| name.clone()).collect(),
                body: body.clone(),
                env: captured,
            }))
        }

        IRNode::Collection { elements, .. } => {
            let mut values = Vec::with_capacity(elements.len());
            for el in elements {
                match eval_flow(el, env, budget)? {
                    Flow::Returned(v) => return Ok(Flow::Returned(v)),
                    Flow::Broke(b) => return Ok(Flow::Broke(b)),
                    Flow::Normal(v) => values.push(v),
                }
            }
            Ok(Flow::Normal(Value::List(values)))
        }

        IRNode::Index(coll, idx) => {
            let coll = eval_flow(coll, env, budget)?.into_value()?;
            let idx = eval_flow(idx, env, budget)?.into_value()?;
            Ok(Flow::Normal(eval_index(coll, idx)?))
        }

        IRNode::Typed { node, .. } => eval_flow(node, env, budget),

        // ── The statement/loop subset (Phase 95, W9) ──────
        // The semantics are the compiled runner's, the same
        // ones the verifier's statement layer mirrors
        // (Phase 93): Alloc binds (its value is the bound
        // value), Assign rebinds an already-bound variable,
        // loops repeat under the iteration budget, Break
        // leaves the innermost loop, Return leaves the body
        // (the `Returned` flow above), and TryPropagate
        // threads a Result — the Ok payload continues, an
        // Err unwinds the enclosing body with that Err as
        // its result (the `?` semantics). One boundary
        // rule, forced by composition: a BLOCK in a value
        // position (an Alloc initializer, an Assign value,
        // a TryPropagate operand) is a body — its Return
        // is its result, absorbed as the block's value by
        // `eval_body_value`. Every other node in those
        // positions propagates its flows: in particular a
        // TryPropagate's own Err-unwind (a Returned flow
        // out of a non-Block initial) passes through the
        // enclosing Alloc untouched, exactly as the
        // compiled `let step = f()?;` unwinds. The
        // compiled runner never inlines, so it never
        // faces this; the in-process composition (the
        // cross-validation harness's judging route)
        // inlines callee bodies — which are Blocks —
        // into exactly these positions. (An earlier draft
        // of this phase propagated Returned out of Block
        // initials too; the composed tokenize_one body
        // then exited at its skip prefix and judged
        // nothing — measured, and corrected.)
        IRNode::Alloc { name, initial, .. } => {
            match eval_body_value(initial, env, budget)? {
                Flow::Normal(v) => {
                    env.insert(name.clone(), v.clone());
                    Ok(Flow::Normal(v))
                }
                flow => Ok(flow),
            }
        }

        IRNode::Assign { target, value } => {
            // The assigned value first (the standing
            // order): a Block there is a body, absorbed by
            // eval_body_value; a control flow out of it
            // propagates untouched, as the Var arm has
            // always done.
            let v = match eval_body_value(value, env, budget)? {
                Flow::Normal(v) => v,
                flow => return Ok(flow),
            };
            match &**target {
                IRNode::Var(name) => {
                    if !env.contains_key(name) {
                        return Err(not_evaluable(format!(
                            "Assign to unbound variable `{}` (no Alloc binds it)",
                            name
                        )));
                    }
                    env.insert(name.clone(), v.clone());
                    Ok(Flow::Normal(v))
                }
                // An indexed place (Phase 131): `xs[i] = v`
                // mutates the bound list in place — the
                // compiled form is a Vec index assignment,
                // and the sort template's swap is the
                // exhibit. Places nest (`grid[i][j] = v`).
                // The failure shapes are the read side's
                // (eval_index): an unbound base, a non-list
                // base or element, a negative index, an
                // out-of-bounds index — each named, never a
                // panic and never a silent grow.
                IRNode::Index(..) => {
                    assign_to_index(target, v.clone(), env, budget)?;
                    Ok(Flow::Normal(v))
                }
                _ => Err(not_evaluable(
                    "Assign target is not a variable or an indexed place \
                     (only a variable or an indexed element can be rebound)",
                )),
            }
        }

        IRNode::While {
            condition, body, ..
        } => {
            loop {
                let cond = match eval_flow(condition, env, budget)? {
                    Flow::Normal(Value::Bool(b)) => b,
                    Flow::Normal(other) => {
                        return Err(not_evaluable(format!(
                            "while condition is not Bool (got {:?})",
                            other
                        )));
                    }
                    flow => return Ok(flow),
                };
                if !cond {
                    break;
                }
                spend_iteration(budget)?;
                match eval_flow(body, env, budget)? {
                    Flow::Normal(_) => continue,
                    Flow::Returned(v) => return Ok(Flow::Returned(v)),
                    Flow::Broke(None) => break,
                    // The valued Break form is genuinely
                    // ambiguous here: the compiled language
                    // gives `while` no break value (a Rust
                    // `while` cannot break with one), so the
                    // form is declined and named rather than
                    // assigned a guessed meaning. (Inside a
                    // `Loop` the same form is the loop's
                    // value — see the Loop arm.)
                    Flow::Broke(Some(_)) => {
                        return Err(not_evaluable(
                            "valued Break inside a While loop is outside the evaluable subset",
                        ));
                    }
                }
            }
            Ok(Flow::Normal(Value::Unit))
        }

        IRNode::Loop { body, .. } => {
            loop {
                spend_iteration(budget)?;
                match eval_flow(body, env, budget)? {
                    Flow::Normal(_) => continue,
                    Flow::Returned(v) => return Ok(Flow::Returned(v)),
                    Flow::Broke(v) => {
                        return Ok(Flow::Normal(v.unwrap_or(Value::Unit)));
                    }
                }
            }
        }

        // For (Phase 115, the List family's judging
        // machinery — STDLIB_SCOPE §2.6 priced this arm
        // into the List phase): the compiled runner's
        // native loop shape, `for var in start..end`. The
        // semantics are the compiled ones: the bounds are
        // evaluated ONCE, before the first iteration; the
        // variable is bound per iteration and SCOPED to
        // the loop (an outer binding of the same name is
        // restored on every exit path, and no binding
        // leaks out); the shared iteration budget is
        // spent per iteration, so a vast range is a judged
        // NotEvaluable, never a hang. A bare Break leaves
        // the loop; a valued one is declined and named,
        // exactly as in a While — the compiled language
        // gives `for` no break value either.
        IRNode::For {
            var,
            start,
            end,
            body,
        } => {
            let start = match eval_flow(start, env, budget)?.into_value()? {
                Value::Int(n) => n,
                other => {
                    return Err(not_evaluable(format!(
                        "For loop start is not Int (got {:?})",
                        other
                    )));
                }
            };
            let end = match eval_flow(end, env, budget)?.into_value()? {
                Value::Int(n) => n,
                other => {
                    return Err(not_evaluable(format!(
                        "For loop end is not Int (got {:?})",
                        other
                    )));
                }
            };
            let saved = env.remove(var);
            let mut flow = Flow::Normal(Value::Unit);
            let mut current = start;
            let mut exit = Ok(());
            while current < end {
                if let Err(e) = spend_iteration(budget) {
                    exit = Err(e);
                    break;
                }
                env.insert(var.clone(), Value::Int(current));
                match eval_flow(body, env, budget) {
                    Ok(Flow::Normal(_)) => {}
                    Ok(Flow::Broke(None)) => break,
                    Ok(Flow::Broke(Some(_))) => {
                        exit = Err(not_evaluable(
                            "valued Break inside a For loop is outside the evaluable subset",
                        ));
                        break;
                    }
                    Ok(returned @ Flow::Returned(_)) => {
                        flow = returned;
                        break;
                    }
                    Err(e) => {
                        exit = Err(e);
                        break;
                    }
                }
                current += 1;
            }
            match saved {
                Some(previous) => {
                    env.insert(var.clone(), previous);
                }
                None => {
                    env.remove(var);
                }
            }
            exit?;
            Ok(flow)
        }

        IRNode::Break { value } => {
            let value = match value {
                None => None,
                Some(inner) => {
                    Some(eval_flow(inner, env, budget)?.into_value()?)
                }
            };
            Ok(Flow::Broke(value))
        }

        IRNode::TryPropagate(inner) => {
            match eval_body_value(inner, env, budget)? {
                Flow::Normal(Value::Ok(payload)) => {
                    Ok(Flow::Normal((*payload).clone()))
                }
                Flow::Normal(err @ Value::Err(_)) => Ok(Flow::Returned(err)),
                Flow::Normal(other) => Err(not_evaluable(format!(
                    "TryPropagate (?) on a non-Result value ({:?})",
                    other
                ))),
                flow => Ok(flow),
            }
        }

        // The higher-order combinators (Phase 131, G4):
        // Map/Filter/Fold/Sort evaluate by applying their
        // function argument — a Lambda/Closure node, or any
        // node evaluating to a bound function value —
        // through the closure machinery of Phase 130
        // (apply_value below), so every application spends
        // the shared budget and nests under the depth bound
        // exactly as a Call through a bound name does. The
        // semantics are the compiled forms': Map applies
        // per element in order; Filter keeps the elements
        // its predicate answers `true` for (a non-Bool
        // answer declines — truthiness is never guessed);
        // Fold is the left fold from its initial value;
        // Sort orders by the comparator whose `true` means
        // "the first argument precedes" (the spec-level
        // meaning: cmp(output[i], output[j]) holds for
        // i < j), stably — elements the comparator cannot
        // separate in either direction keep their input
        // order, as the compiled `sort_by` is stable. A
        // non-function function-position, a non-List
        // collection, and a non-Bool comparator answer each
        // decline by name.
        IRNode::Map { function, over } => {
            let items = eval_list_operand("Map", over, env, budget)?;
            let f = eval_function_operand("Map", function, env, budget)?;
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(apply_value(&f, vec![item], budget)?);
            }
            Ok(Flow::Normal(Value::List(out)))
        }

        IRNode::Filter { predicate, over } => {
            let items = eval_list_operand("Filter", over, env, budget)?;
            let f = eval_function_operand("Filter", predicate, env, budget)?;
            let mut out = Vec::new();
            for item in items {
                match apply_value(&f, vec![item.clone()], budget)? {
                    Value::Bool(true) => out.push(item),
                    Value::Bool(false) => {}
                    other => {
                        return Err(not_evaluable(format!(
                            "Filter predicate's answer is not Bool (got {:?}): \
                             a predicate must answer Bool",
                            other
                        )));
                    }
                }
            }
            Ok(Flow::Normal(Value::List(out)))
        }

        IRNode::Fold {
            operation,
            initial,
            over,
        } => {
            let items = eval_list_operand("Fold", over, env, budget)?;
            let f = eval_function_operand("Fold", operation, env, budget)?;
            let mut acc = eval_flow(initial, env, budget)?.into_value()?;
            for item in items {
                acc = apply_value(&f, vec![acc, item], budget)?;
            }
            Ok(Flow::Normal(acc))
        }

        IRNode::Sort {
            comparator,
            collection,
        } => {
            let items = eval_list_operand("Sort", collection, env, budget)?;
            let f = eval_function_operand("Sort", comparator, env, budget)?;
            let sorted = sort_values(items, &f, budget)?;
            Ok(Flow::Normal(Value::List(sorted)))
        }

        // Everything below stays outside the subset:
        // unfilled holes and struct definitions. Neither
        // can be given a value without guessing, so they
        // are reported as not evaluable. (For graduated
        // into the subset in Phase 115 — its arm sits with
        // the other loop forms above; Lambda/Closure in
        // Phase 130 — their arms sit with the Call arm
        // above; Map/Filter/Fold/Sort in Phase 131 — their
        // arms sit directly above this comment.)
        IRNode::Hole { .. } => Err(not_evaluable("unfilled Hole")),
        IRNode::StructDef { .. } => Err(not_evaluable("StructDef (declaration)")),
        // M1 features (Phase 75): record construction and
        // field projection over Value::Record (D2a/D3a).
        // Fields evaluate in written order, like Collection
        // elements; a Return inside a field value unwinds
        // the whole evaluation, also like Collection.
        IRNode::RecordLit { type_name, fields } => {
            let mut values = Vec::with_capacity(fields.len());
            for (name, field_node) in fields {
                match eval_flow(field_node, env, budget)? {
                    Flow::Returned(v) => return Ok(Flow::Returned(v)),
                    Flow::Broke(b) => return Ok(Flow::Broke(b)),
                    Flow::Normal(v) => values.push((name.clone(), v)),
                }
            }
            Ok(Flow::Normal(Value::Record {
                type_name: type_name.clone(),
                fields: values,
            }))
        }
        IRNode::FieldAccess { base, field } => {
            let base_value = eval_flow(base, env, budget)?.into_value()?;
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

/// Spend one loop iteration of an evaluation's budget
/// (Phase 95): the termination discipline of the statement
/// subset. Exhaustion is a judged `NotEvaluable` — the
/// evaluation produced no answer — never a hang and never
/// a fabricated exit value.
fn spend_iteration(budget: &mut Budget) -> Result<(), EvalError> {
    if budget.steps == 0 {
        return Err(not_evaluable(format!(
            "loop iteration bound exceeded ({} iterations): the body does not terminate within the interpreter's bound",
            MAX_LOOP_ITERATIONS
        )));
    }
    budget.steps -= 1;
    Ok(())
}

/// Spend one closure application of an evaluation's budget
/// (Phase 130, G4_SCOPE §2.3): application draws on the
/// SAME counter as loop iterations (`spend_iteration`), so
/// the two runaway shapes share one bound and one failure
/// mode. The message names the shared discipline.
fn spend_application(budget: &mut Budget) -> Result<(), EvalError> {
    if budget.steps == 0 {
        return Err(not_evaluable(format!(
            "function application bound exceeded: applications share the interpreter's evaluation budget with loop iterations ({} total), and it is exhausted",
            MAX_LOOP_ITERATIONS
        )));
    }
    budget.steps -= 1;
    Ok(())
}

/// Evaluate a node sitting in a composition-boundary
/// value position (an Alloc initializer, an Assign value,
/// a TryPropagate operand — the positions the in-process
/// composition inlines callee bodies into). A Block there
/// is a body: its `Returned` flow is its result, absorbed
/// as a plain value. Any other node propagates its flows
/// untouched, so a control flow raised by the position's
/// own operators (a TryPropagate unwind, a Break headed
/// for its loop) is never mistaken for a value.
fn eval_body_value(
    node: &IRNode,
    env: &mut Env,
    budget: &mut Budget,
) -> Result<Flow, EvalError> {
    let flow = eval_flow(node, env, budget)?;
    if matches!(node, IRNode::Block(_)) {
        match flow {
            Flow::Returned(v) => Ok(Flow::Normal(v)),
            other => Ok(other),
        }
    } else {
        Ok(flow)
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
///   unparsed string condition; a constructor pattern whose
///   arity contradicts the value family it names), so the
///   whole match is not evaluable rather than a guess.
///
/// Constructor patterns match in-process since Phase 95
/// (the Phase 75 residual is closed): `Some`/`None` against
/// the Option values, `Ok`/`Err` against the Result values,
/// with the payload bound by the single sub-pattern
/// (recursively — a nested constructor pattern matches a
/// nested constructor value), and a constructor naming a
/// record type against a `Value::Record` of that type, its
/// sub-patterns binding the fields positionally in written
/// order (the record value's own convention). A constructor
/// pattern against a value of a different family simply
/// does not match — the next arm is tried, exactly as the
/// compiled runner falls through. Parity is with the
/// compiled runner, which has matched these patterns since
/// the bodies were first compiled.
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
        IRPattern::Constructor(name, subs) => {
            // The payload-carrying constructors: the pattern
            // matches its own family's value and binds the
            // payload through the one sub-pattern.
            let payload = match (name.as_str(), value) {
                ("Some", Value::Some(v)) => Some(&**v),
                ("Ok", Value::Ok(v)) => Some(&**v),
                ("Err", Value::Err(v)) => Some(&**v),
                _ => None,
            };
            if let Some(payload) = payload {
                let [sub] = subs.as_slice() else {
                    return Err(not_evaluable(format!(
                        "constructor pattern `{}` carries {} sub-patterns for a one-payload value",
                        name,
                        subs.len()
                    )));
                };
                return bind_pattern(sub, payload);
            }
            // The nullary constructor.
            if name == "None" {
                return match value {
                    Value::None => {
                        if subs.is_empty() {
                            Ok(Some(Env::new()))
                        } else {
                            Err(not_evaluable(
                                "constructor pattern `None` carries sub-patterns for a payload-free value",
                            ))
                        }
                    }
                    _ => Ok(None),
                };
            }
            // A record constructor: the pattern names the
            // record's type and binds its fields positionally.
            if let Value::Record { type_name, fields } = value {
                if type_name == name {
                    if subs.len() != fields.len() {
                        return Err(not_evaluable(format!(
                            "record pattern `{}` carries {} sub-patterns for {} fields",
                            name,
                            subs.len(),
                            fields.len()
                        )));
                    }
                    let mut env = Env::new();
                    for (sub, (_, field_value)) in subs.iter().zip(fields.iter()) {
                        match bind_pattern(sub, field_value)? {
                            Some(bindings) => env.extend(bindings),
                            None => return Ok(None),
                        }
                    }
                    return Ok(Some(env));
                }
            }
            // Every remaining combination is a constructor of
            // one family against a value of another: no match.
            Ok(None)
        }
        IRPattern::Guard(..) => Err(not_evaluable(
            "match guard patterns are outside the evaluable subset",
        )),
    }
}

fn eval_call(
    function: &str,
    args: &[IRNode],
    env: &mut Env,
    budget: &mut Budget,
) -> Result<Value, EvalError> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(eval_flow(arg, env, budget)?.into_value()?);
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
        // F2 (Phase 100, D1a/D2a/D3a): the Int→decimal-text
        // conversion. Rust's own `Display` IS the
        // implementation, so negatives and the i64
        // extremes are exact by construction (the Phase 99
        // probe 5 table is the test obligation, carried by
        // `eval_int_to_str_battery`). Wrong arity/type is
        // a named failure, as `len`'s arm.
        "int_to_str" => match values.as_slice() {
            [Value::Int(n)] => Ok(Value::Str(format!("{}", n))),
            _ => Err(not_evaluable(
                "int_to_str expects exactly one Int argument",
            )),
        },
        // Not a builtin. Since Phase 130 (G4, §2.3) a call
        // whose callee name is bound in the environment to
        // a FUNCTION VALUE applies it — this is how a body
        // applies its function-typed parameter (`f(x)`) and
        // any Let-bound closure. The builtin arms above keep
        // their standing precedence (the checker resolves
        // builtins and module signatures before variables
        // the same way). A name bound to a non-function value
        // declines by name; a name bound nowhere keeps the
        // standing decline. Template-internal pseudo-calls
        // ("first", "rest", "push", "combine", …) and
        // spec-to-spec calls land in one of those two
        // declines: there is no definition to execute here.
        other => match env.get(other) {
            Some(Value::Function {
                params,
                body,
                env: captured,
            }) => apply_closure(params, body, captured, values, budget),
            Some(_) => Err(not_evaluable(format!(
                "call to `{}`, which is bound to a non-function value (only a function value can be applied)",
                other
            ))),
            None => Err(not_evaluable(format!(
                "call to unknown function `{}`",
                other
            ))),
        },
    }
}

/// Apply a closure value to argument values (Phase 130, G4
/// §2.3): the parameters bind to the arguments in the
/// closure's CAPTURED environment — never the call site's —
/// extended with those bindings (a parameter shadows a
/// captured binding of the same name), and the body
/// evaluates there, drawing on the caller's remaining
/// budget: the body's own loops and nested applications
/// spend from the same counter the caller is spending.
/// Application itself spends one unit up front
/// (`spend_application`), and live applications nest at
/// most `MAX_APPLICATION_DEPTH` deep — the discipline's
/// two halves: total work bounded by the shared counter,
/// recursion depth bounded so a self-application bomb
/// declines by name instead of exhausting the host stack.
/// Arity is the checker's coarse discipline made concrete:
/// a mismatch declines by name.
fn apply_closure(
    params: &[String],
    body: &IRNode,
    captured: &Env,
    args: Vec<Value>,
    budget: &mut Budget,
) -> Result<Value, EvalError> {
    if args.len() != params.len() {
        return Err(not_evaluable(format!(
            "function value expects {} argument(s), {} given",
            params.len(),
            args.len()
        )));
    }
    spend_application(budget)?;
    if budget.depth >= MAX_APPLICATION_DEPTH {
        return Err(not_evaluable(format!(
            "function application depth bound exceeded ({} nested applications): application nesting is bounded with the shared evaluation budget, so a self-application recursion declines instead of exhausting the host stack",
            MAX_APPLICATION_DEPTH
        )));
    }
    budget.depth += 1;
    let mut call_env = captured.clone();
    for (param, arg) in params.iter().zip(args) {
        call_env.insert(param.clone(), arg);
    }
    // A Return inside the body is the application's result
    // (into_value's standing reading); a Break with no
    // enclosing loop inside the body declines there.
    let result = eval_flow(body, &mut call_env, budget);
    budget.depth -= 1;
    result?.into_value()
}

/// Apply a function VALUE to argument values (Phase 131):
/// the value-level entry to `apply_closure`, shared by the
/// higher-order arms. A non-function value declines by
/// name at the call sites (they know their node's name);
/// here the shape is already a closure.
fn apply_value(
    f: &Value,
    args: Vec<Value>,
    budget: &mut Budget,
) -> Result<Value, EvalError> {
    match f {
        Value::Function {
            params,
            body,
            env: captured,
        } => apply_closure(params, body, captured, args, budget),
        other => Err(not_evaluable(format!(
            "applied a non-function value ({:?}): only a function value can be applied",
            other
        ))),
    }
}

/// The collection operand of a higher-order node (Phase
/// 131): evaluates to the element list, or declines naming
/// the node and the actual shape.
fn eval_list_operand(
    node_name: &str,
    over: &IRNode,
    env: &mut Env,
    budget: &mut Budget,
) -> Result<Vec<Value>, EvalError> {
    match eval_flow(over, env, budget)?.into_value()? {
        Value::List(items) => Ok(items),
        other => Err(not_evaluable(format!(
            "{} over a value that is not a List ({:?}): the combinator is defined over lists",
            node_name, other
        ))),
    }
}

/// The function operand of a higher-order node (Phase
/// 131): evaluates to a function value — a Lambda/Closure
/// node, or a Var (or any expression) bound to one — or
/// declines naming the node.
fn eval_function_operand(
    node_name: &str,
    function: &IRNode,
    env: &mut Env,
    budget: &mut Budget,
) -> Result<Value, EvalError> {
    let f = eval_flow(function, env, budget)?.into_value()?;
    match f {
        Value::Function { .. } => Ok(f),
        other => Err(not_evaluable(format!(
            "{} with a function argument that is not a function value ({:?})",
            node_name, other
        ))),
    }
}

/// Stable ordering by a Bool comparator (Phase 131): the
/// comparator's `true` means its first argument strictly
/// precedes its second; a pair the comparator cannot
/// separate in either direction is equivalent and keeps
/// its input order (the merge below always takes the left
/// run's head on equivalence — the compiled `sort_by` is
/// stable the same way). A non-Bool comparator answer
/// declines by name. Merge sort: O(n log n) comparator
/// applications, each spending the shared budget through
/// apply_value.
fn sort_values(
    items: Vec<Value>,
    comparator: &Value,
    budget: &mut Budget,
) -> Result<Vec<Value>, EvalError> {
    if items.len() <= 1 {
        return Ok(items);
    }
    let mid = items.len() / 2;
    let mut halves = items.into_iter();
    let left: Vec<Value> = halves.by_ref().take(mid).collect();
    let right: Vec<Value> = halves.collect();
    let left = sort_values(left, comparator, budget)?;
    let right = sort_values(right, comparator, budget)?;
    let mut out = Vec::with_capacity(left.len() + right.len());
    let mut left = left.into_iter().peekable();
    let mut right = right.into_iter().peekable();
    // Does `a` strictly precede `b` under the comparator?
    let precedes = |a: &Value, b: &Value, budget: &mut Budget| -> Result<bool, EvalError> {
        match apply_value(comparator, vec![a.clone(), b.clone()], budget)? {
            Value::Bool(v) => Ok(v),
            other => Err(not_evaluable(format!(
                "Sort comparator's answer is not Bool (got {:?}): \
                 a comparator must answer Bool",
                other
            ))),
        }
    };
    while left.peek().is_some() && right.peek().is_some() {
        let take_right = {
            let l = left.peek().expect("peeked");
            let r = right.peek().expect("peeked");
            // Take the right head only when it strictly
            // precedes the left head; equivalence (neither
            // direction precedes) keeps the left head
            // first — stability.
            precedes(r, l, budget)?
        };
        if take_right {
            out.push(right.next().expect("peeked"));
        } else {
            out.push(left.next().expect("peeked"));
        }
    }
    out.extend(left);
    out.extend(right);
    Ok(out)
}

/// Assign into an indexed place (Phase 131): the target
/// is an `Index` chain rooted at a variable; the value is
/// written into the bound list at the indexed slot,
/// navigating nested lists for nested places. Index
/// expressions evaluate root-first (the order the compiled
/// place evaluates). The failure shapes are eval_index's:
/// unbound root, non-list base or element, negative index,
/// out-of-bounds index — each named.
fn assign_to_index(
    target: &IRNode,
    value: Value,
    env: &mut Env,
    budget: &mut Budget,
) -> Result<(), EvalError> {
    // Walk the chain down to its root variable, collecting
    // the index nodes outermost-first.
    let mut index_nodes: Vec<&IRNode> = Vec::new();
    let mut cursor = target;
    let root = loop {
        match cursor {
            IRNode::Index(base, idx) => {
                index_nodes.push(idx);
                cursor = base;
            }
            IRNode::Var(name) => break name,
            _ => {
                return Err(not_evaluable(
                    "Assign target is not a variable or an indexed place \
                     (only a variable or an indexed element can be rebound)",
                ));
            }
        }
    };
    if !env.contains_key(root) {
        return Err(not_evaluable(format!(
            "Assign target's base `{}` is not bound (no Alloc binds it)",
            root
        )));
    }
    // Indices evaluate innermost-first (root-first): the
    // reverse of the collection order above.
    let mut indices: Vec<i64> = Vec::with_capacity(index_nodes.len());
    for node in index_nodes.iter().rev() {
        match eval_flow(node, env, budget)?.into_value()? {
            Value::Int(i) => indices.push(i),
            other => {
                return Err(not_evaluable(format!(
                    "Assign target's index is not Int (got {:?})",
                    other
                )));
            }
        }
    }
    let slot = env.get_mut(root).expect("root checked bound above");
    assign_at_path(slot, &indices, value)
}

/// Write `value` into the list slot at `path` inside
/// `slot` (Phase 131): the recursive half of
/// assign_to_index, over an already-located value.
fn assign_at_path(slot: &mut Value, path: &[i64], value: Value) -> Result<(), EvalError> {
    let (first, rest) = match path.split_first() {
        Some(pair) => pair,
        // The path is never empty (the target was an Index
        // node); an empty path would be the Var arm's job.
        None => {
            *slot = value;
            return Ok(());
        }
    };
    let items = match slot {
        Value::List(items) => items,
        other => {
            return Err(not_evaluable(format!(
                "Assign target indexes into a non-list value ({:?})",
                other
            )));
        }
    };
    if *first < 0 {
        return Err(not_evaluable("negative list index"));
    }
    let element = items
        .get_mut(*first as usize)
        .ok_or_else(|| not_evaluable("list index out of bounds"))?;
    assign_at_path(element, rest, value)
}

/// Evaluate one D1a builtin method call on an already
/// evaluated receiver (Phase 75): the spec language's method
/// semantics on interpreter values. The table is exactly the
/// typechecker's closed table (Phase 74) — `Result`:
/// `is_ok` / `is_err` / `unwrap`; `Option`: `is_some` /
/// `is_none` / `unwrap` (Phase 112, the STDLIB_SCOPE §2.5
/// extension); `List`: `len` — all zero-argument. Anything
/// else (unknown method, wrong receiver family, arguments
/// to a builtin, a receiver with no method table) is a
/// named `NotEvaluable`, never a guessed value; `unwrap`
/// on an `Err` or `None` value is the named failure the
/// interpreter's failure semantics require.
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
        Value::Some(payload) => match name {
            "is_some" => Ok(Value::Bool(true)),
            "is_none" => Ok(Value::Bool(false)),
            "unwrap" => Ok((**payload).clone()),
            _ => Err(not_evaluable(format!(
                "method `{}` does not exist on an Option value",
                name
            ))),
        },
        Value::None => match name {
            "is_some" => Ok(Value::Bool(false)),
            "is_none" => Ok(Value::Bool(true)),
            // The Err case's failure semantics, verbatim:
            // unwrapping a None has no value.
            "unwrap" => Err(not_evaluable(
                "method `unwrap` called on a None value",
            )),
            _ => Err(not_evaluable(format!(
                "method `{}` does not exist on an Option value",
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
            // String indexing is TOTAL in the language:
            // the compiled rendering is
            // `chars().nth(i).unwrap_or('\0')` (Phase 85's
            // index rendering, pinned in codegen), and the
            // Z3 CharStr bridge totalizes the same way. An
            // out-of-range or negative read is the NUL
            // sentinel, not an error — the tokenizer's
            // ident loop depends on it at end of input
            // (the sentinel is not an ident-continue char,
            // so the loop breaks exactly where peek's None
            // breaks it). List indexing above stays an
            // error: the compiled form is a Vec index,
            // whose out-of-range behavior is a panic — a
            // non-answer either way.
            let c = if i < 0 {
                '\0'
            } else {
                s.chars().nth(i as usize).unwrap_or('\0')
            };
            Ok(Value::Char(c))
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
/// Int/Int compares exactly (Phase 111 finding): routing it
/// through f64 made distinct ints within f64 spacing of each
/// other — everything from 2^53 up, e.g. the Int minimum and
/// its neighbours — compare equal, which silently corrupted
/// both test outcomes and `require:` evaluation at extremes.
///
/// Returns `None` for DECLINED (Phase 130, G4_SCOPE §2.6,
/// D5a): there is no equality on function values, so a
/// comparison whose operands are — or anywhere contain — a
/// function value is not decided here at all. The derived
/// structural `==` must never see a closure (it would hand
/// it a syntactic equality over body AST + captured
/// environment that means nothing); each caller turns the
/// decline into its own standing outcome (a `NotEvaluable`
/// in evaluated positions, an undecided row in `require:`
/// evaluation, a declined comparison in test judging).
fn values_equal(l: &Value, r: &Value) -> Option<bool> {
    if contains_function(l) || contains_function(r) {
        return None;
    }
    if let (Value::Int(a), Value::Int(b)) = (l, r) {
        return Some(a == b);
    }
    if let (Some(a), Some(b)) = (as_f64(l), as_f64(r)) {
        if matches!(l, Value::Int(_) | Value::Float(_))
            && matches!(r, Value::Int(_) | Value::Float(_))
        {
            return Some(a == b);
        }
    }
    Some(l == r)
}

fn eval_binop(op: IRBinOp, l: Value, r: Value) -> Result<Value, EvalError> {
    use IRBinOp::*;
    match op {
        // Equality declines on function values (§2.6): the
        // comparison is outside the evaluable subset, named
        // — never a structural guess.
        Eq => values_equal(&l, &r)
            .map(Value::Bool)
            .ok_or_else(|| {
                not_evaluable(
                    "equality on function values: function values have no equality (G4 §2.6)",
                )
            }),
        Neq => values_equal(&l, &r)
            .map(|eq| Value::Bool(!eq))
            .ok_or_else(|| {
                not_evaluable(
                    "equality on function values: function values have no equality (G4 §2.6)",
                )
            }),
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
            // The lexeme shape (Phase 95, the design doc's
            // Phase 95 gate): a Char appended to a String —
            // the compiled rendering is `push(char)`. The
            // (Str, Str) form is not an Append shape (string
            // concatenation is Concat's arm), exactly as in
            // the encoder's W5 arm (Phase 91).
            (Value::Str(mut s), Value::Char(c)) => {
                s.push(c);
                Ok(Value::Str(s))
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
        // Clone is the identity over immutable values
        // (Phase 95): a copy of a value IS the value, the
        // same reading the encoder's W3 arm took in
        // Phase 91. The list operations beside it stay
        // outside the subset — they have no measured
        // consumer in the interpreter.
        Clone => Ok(v),
        Head | Tail | Init | Last | Reverse => Err(not_evaluable(format!(
            "unary operator {:?} is outside the evaluable subset",
            op
        ))),
    }
}

// ── Spec test execution ───────────────────────────────────

// ── Named-reference function supply (Phase 132, G4 §2.5) ──
//
// The second supply form: a bare name standing in a
// function position in a test row resolves through the
// module index to a function value built from the named
// spec's body — sibling specs and shipped entries alike
// (the index is where the prelude merge lives, so an
// entry resolves exactly as a sibling does, and a
// module's own definition wins a collision, Phase 110).
// The construction mirrors the two standing consumer
// routes for denoted bodies:
//
// - an acyclic body is call-resolved by substitution
//   (`SpecIndex::inline_calls`, the resolver route the
//   compositions are judged through), and the closure
//   captures nothing;
// - a SELF-RECURSIVE body (recursion is a named-spec
//   feature, §2.2 — the named reference is its vehicle)
//   keeps its raw denoted form, and the closure's
//   captured environment binds the spec's own name to
//   the closure itself, built as a ladder of
//   MAX_APPLICATION_DEPTH + 1 generations: each
//   generation's environment holds the previous one
//   under the name. The ladder's foot is unreachable —
//   an application chain that deep has already declined
//   by the application's own depth bound — so a
//   recursion past the bound declines by the depth rule,
//   by name, never by a missing binding and never by
//   exhausting the host stack. Sibling calls in such a
//   body bind as named values in the same environment.
//   A call cycle back through a sibling (mutual
//   recursion) is not a G4 form and declines the supply.
//
// Every failure is a named decline (the Err text), never
// a silent non-binding: an unknown name, a name denoting
// no executable body, a body that cannot be
// call-resolved. The position discipline — a name binds
// only in a function position, and only when its
// signature fits — is the binding layer's (`bind_part`).

/// The function value a named reference denotes, built
/// from the named spec's body in the index (see the
/// section head). `Err` is the decline reason, naming
/// the name and the class.
fn named_function_value(index: &SpecIndex, name: &str) -> Result<Value, String> {
    named_function_value_in(index, name, &[])
}

fn named_function_value_in(
    index: &SpecIndex,
    name: &str,
    visiting: &[String],
) -> Result<Value, String> {
    if visiting.iter().any(|v| v == name) {
        return Err(format!(
            "the named function reference `{name}` cannot be built: its body calls \
             back into `{name}` — mutual recursion between named references is not \
             a G4 form (G4_SCOPE §2.2)"
        ));
    }
    let spec = index.get(name).ok_or_else(|| {
        format!(
            "unknown function reference `{name}`: the module index holds no spec of that name"
        )
    })?;
    let raw = index.callee_body(name).ok_or_else(|| {
        format!(
            "the named function reference `{name}` denotes no executable body: \
             no shipped body is registered for it and its constraints build no denotation"
        )
    })?;
    let params: Vec<String> = spec.params.iter().map(|p| p.name.clone()).collect();
    let mut called: Vec<String> = Vec::new();
    collect_called_indexed(&raw, index, &mut called);
    if !called.iter().any(|c| c == name) {
        // Acyclic: the standing consumer construction —
        // resolve the body's spec calls by substitution.
        let body = index.inline_calls(&raw).ok_or_else(|| {
            format!(
                "the named function reference `{name}` denotes a body that cannot be \
                 call-resolved against the module index (a call cycle, an arity \
                 mismatch, or a shape the resolver cannot expand)"
            )
        })?;
        return Ok(Value::Function {
            params,
            body: Box::new(body),
            env: Env::new(),
        });
    }
    // Self-recursive: the ladder construction (see the
    // section head). Sibling calls bind as named values
    // in every generation's environment.
    let mut chain: Vec<String> = visiting.to_vec();
    chain.push(name.to_string());
    let mut base_env = Env::new();
    for callee in called.iter().filter(|c| *c != name) {
        let value = named_function_value_in(index, callee, &chain)
            .map_err(|e| format!("the named function reference `{name}` cannot be built: {e}"))?;
        base_env.insert(callee.clone(), value);
    }
    let mut current = Value::Function {
        params: params.clone(),
        body: Box::new(raw.clone()),
        env: base_env.clone(),
    };
    for _ in 0..MAX_APPLICATION_DEPTH {
        let mut env = base_env.clone();
        env.insert(name.to_string(), current);
        current = Value::Function {
            params: params.clone(),
            body: Box::new(raw.clone()),
            env,
        };
    }
    Ok(current)
}

/// The indexed specs an IR body calls, in first-mention
/// order — the named reference's call set. Descends
/// everywhere a call can sit, lambda and closure bodies
/// included (a lambda created inside the body captures
/// the environment its body is evaluated in, so its
/// calls resolve against the same bindings).
fn collect_called_indexed(node: &IRNode, index: &SpecIndex, out: &mut Vec<String>) {
    match node {
        IRNode::Call { function, args } => {
            if index.get(function).is_some() && !out.iter().any(|n| n == function) {
                out.push(function.clone());
            }
            for a in args {
                collect_called_indexed(a, index, out);
            }
        }
        IRNode::Block(items) => {
            for item in items {
                collect_called_indexed(item, index, out);
            }
        }
        IRNode::Let { value, body, .. } => {
            collect_called_indexed(value, index, out);
            collect_called_indexed(body, index, out);
        }
        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_called_indexed(condition, index, out);
            collect_called_indexed(then_branch, index, out);
            collect_called_indexed(else_branch, index, out);
        }
        IRNode::While {
            condition, body, ..
        } => {
            collect_called_indexed(condition, index, out);
            collect_called_indexed(body, index, out);
        }
        IRNode::For {
            start, end, body, ..
        } => {
            collect_called_indexed(start, index, out);
            collect_called_indexed(end, index, out);
            collect_called_indexed(body, index, out);
        }
        IRNode::Match { scrutinee, cases } => {
            collect_called_indexed(scrutinee, index, out);
            for (_, body) in cases {
                collect_called_indexed(body, index, out);
            }
        }
        IRNode::Return(inner) | IRNode::TryPropagate(inner) => {
            collect_called_indexed(inner, index, out);
        }
        IRNode::UnaryOp(_, inner) => collect_called_indexed(inner, index, out),
        IRNode::BinOp(_, l, r) | IRNode::Index(l, r) => {
            collect_called_indexed(l, index, out);
            collect_called_indexed(r, index, out);
        }
        IRNode::Lambda { body, .. } | IRNode::Closure { body, .. } => {
            collect_called_indexed(body, index, out);
        }
        IRNode::Collection { elements, .. } => {
            for e in elements {
                collect_called_indexed(e, index, out);
            }
        }
        IRNode::Map { function, over } | IRNode::Filter { predicate: function, over } => {
            collect_called_indexed(function, index, out);
            collect_called_indexed(over, index, out);
        }
        IRNode::Fold {
            operation,
            initial,
            over,
        } => {
            collect_called_indexed(operation, index, out);
            collect_called_indexed(initial, index, out);
            collect_called_indexed(over, index, out);
        }
        IRNode::Sort {
            comparator,
            collection,
        } => {
            collect_called_indexed(comparator, index, out);
            collect_called_indexed(collection, index, out);
        }
        IRNode::Alloc { initial, .. } => collect_called_indexed(initial, index, out),
        IRNode::Assign { target, value } => {
            collect_called_indexed(target, index, out);
            collect_called_indexed(value, index, out);
        }
        IRNode::Typed { node, .. } => collect_called_indexed(node, index, out),
        IRNode::Loop { body, .. } => collect_called_indexed(body, index, out),
        IRNode::Break { value } => {
            if let Some(v) = value {
                collect_called_indexed(v, index, out);
            }
        }
        IRNode::StructDef { body, .. } => collect_called_indexed(body, index, out),
        IRNode::FieldAccess { base, .. } => collect_called_indexed(base, index, out),
        IRNode::RecordLit { fields, .. } => {
            for (_, v) in fields {
                collect_called_indexed(v, index, out);
            }
        }
        IRNode::Hole { .. } | IRNode::Literal(_) | IRNode::Var(_) => {}
    }
}

/// Whether the named spec's signature fits the function
/// type of the position it is supplied for, at the
/// checker's coarse discipline: arity is exact; a type
/// component must be equal, except that a generic
/// component on either side fits anything (instantiation
/// is the caller's business, as it is for the checker's
/// own fn-typed parameters).
fn signature_fits(named: &FunctionSpec, expected: &TypeRef) -> bool {
    let TypeRef::Function(param_tys, return_ty) = expected else {
        return false;
    };
    named.params.len() == param_tys.len()
        && named
            .params
            .iter()
            .zip(param_tys.iter())
            .all(|(p, t)| type_fits(&p.ty, t))
        && type_fits(&named.return_type, return_ty)
}

fn type_fits(actual: &TypeRef, expected: &TypeRef) -> bool {
    actual == expected
        || matches!(actual, TypeRef::Generic(_))
        || matches!(expected, TypeRef::Generic(_))
}

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
///
/// A second carve-out (Phase 130, G4 §2.5): `Expr::Lambda`
/// maps HERE to a function value, because `expr_to_ir`
/// likewise lowers it to a Hole. The body is lowered once,
/// at mapping time, through `expr_to_ir` (a body part the
/// lowering drops — a method call, say — becomes a Hole in
/// the closure's body and declines by name when the closure
/// is applied). The captured environment is empty: a test
/// literal is a closed supply form, and a body reaching
/// past its parameters for a free name declines as an
/// unbound variable at application, never a guessed
/// capture.
///
/// A third carve-out (Phase 132, G4 §2.5): with a module
/// index in hand (`value_of_expr_in`), a bare name
/// (`Expr::Var`) naming an indexed spec maps HERE to the
/// function value the name denotes — the named-reference
/// supply form (see the named-supply section above). This
/// mapping is the primitive, position-blind; the
/// function-position discipline (a name binds only where
/// a function is being supplied, and only when its
/// signature fits the position) lives in the binding
/// layer (`bind_part`). Without an index this function is
/// the standing one, and a bare name takes the generic
/// route below, which cannot map it — exactly as before.
pub fn value_of_expr(expr: &Expr) -> Option<Value> {
    value_of_expr_indexed(expr, None)
}

/// `value_of_expr` with a module index in hand: a bare
/// name naming an indexed spec additionally maps to the
/// function value it denotes (Phase 132, §2.5). Every
/// other form maps exactly as `value_of_expr` maps it.
pub fn value_of_expr_in(expr: &Expr, index: &SpecIndex) -> Option<Value> {
    value_of_expr_indexed(expr, Some(index))
}

fn value_of_expr_indexed(expr: &Expr, index: Option<&SpecIndex>) -> Option<Value> {
    match expr {
        Expr::IntLit(n) => Some(Value::Int(*n)),
        Expr::FloatLit(f) => Some(Value::Float(*f)),
        Expr::BoolLit(b) => Some(Value::Bool(*b)),
        Expr::StringLit(s) => Some(Value::Str(s.clone())),
        Expr::CharLit(c) => Some(Value::Char(*c)),
        Expr::Lambda(params, body) => Some(Value::Function {
            params: params.iter().map(|(name, _)| name.clone()).collect(),
            body: Box::new(super::engine::expr_to_ir(body)),
            env: Env::new(),
        }),
        Expr::Var(name) => {
            // The named-reference carve-out (Phase 132):
            // with an index, a name denoting a spec maps
            // to its function value. Any other name — and
            // every name without an index — takes the
            // generic route, which is the standing
            // behavior (a bare `None` reads as the None
            // value through it, Phase 131; an unbound name
            // does not map).
            if let Some(index) = index {
                if let Ok(value) = named_function_value(index, name) {
                    return Some(value);
                }
            }
            let ir = super::engine::expr_to_ir(expr);
            eval(&ir, &Env::new()).ok()
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            let receiver = value_of_expr_indexed(receiver, index)?;
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(value_of_expr_indexed(arg, index)?);
            }
            eval_method_call(&receiver, name, &values).ok()
        }
        _ => {
            let ir = super::engine::expr_to_ir(expr);
            eval(&ir, &Env::new()).ok()
        }
    }
}

/// Why a test input did not bind: either a SPECIFIC
/// named decline (the payload is the outcome's reason —
/// a named function reference that cannot resolve, does
/// not fit, or stands in a non-function position), or
/// the GENERIC failure, whose reason is the standing
/// class selection in `test_outcomes_indexed` (unmappable
/// value vs shape/arity), unchanged from before
/// Phase 132.
enum BindDecline {
    Specific(String),
    Generic,
}

/// Build the environment for one test: the test input value is
/// bound to the spec's parameter(s). A single parameter takes
/// the value directly; multiple parameters bind elementwise
/// from either of the two multi-value input forms:
///
/// - a TUPLE input `(v1, v2, ...)` — the parser lowers it to
///   `Call("__tuple__", [v1, v2, ...])` (parser.rs), and each
///   element is mapped on its own (Phase 53: before this, the
///   tuple was mapped as ONE value, which always failed —
///   "__tuple__" is not an evaluable function — so tuple tests
///   never bound);
/// - a whole input that maps to a List of exactly the
///   parameter count.
///
/// A tuple whose arity differs from the parameter count, or
/// with an element that cannot be mapped (a nested tuple —
/// there is no tuple Value), does not bind: the test is
/// excluded, exactly as before. A lambda element DOES map
/// since Phase 130 — to a closure (see `value_of_expr`).
/// An element that is a bare sibling name in a function
/// position is the named-reference supply form (Phase 132,
/// §2.5): it maps where the binding carries a module index
/// (through `bind_part`), to the function value the name
/// denotes; on a route carrying no index it does not bind,
/// and its decline names the missing index.
fn bind_test_env_indexed(
    params: &[Param],
    input: &Expr,
    index: Option<&SpecIndex>,
) -> Result<Env, BindDecline> {
    // Tuple form: bind elementwise. Checked before the
    // whole-value mapping, which cannot represent a tuple.
    if params.len() >= 2 {
        if let Expr::Call(name, parts) = input {
            if name == "__tuple__" && parts.len() == params.len() {
                let mut env = Env::new();
                for (param, part) in params.iter().zip(parts.iter()) {
                    env.insert(param.name.clone(), bind_part(part, param, index)?);
                }
                return Ok(env);
            }
        }
    }
    // A single parameter supplied by a bare name goes
    // through the same per-part discipline as a tuple
    // element (the named-reference position rules).
    if params.len() == 1 {
        if let Expr::Var(_) = input {
            let value = bind_part(input, &params[0], index)?;
            let mut env = Env::new();
            env.insert(params[0].name.clone(), value);
            return Ok(env);
        }
    }
    let value = value_of_expr_indexed(input, index).ok_or(BindDecline::Generic)?;
    let mut env = Env::new();
    match params.len() {
        0 => Ok(env),
        1 => {
            env.insert(params[0].name.clone(), value);
            Ok(env)
        }
        _ => match value {
            Value::List(items) if items.len() == params.len() => {
                for (param, item) in params.iter().zip(items) {
                    env.insert(param.name.clone(), item);
                }
                Ok(env)
            }
            _ => Err(BindDecline::Generic),
        },
    }
}

/// Bind one supply position — a tuple element, or a lone
/// input — against its parameter. A bare name is the
/// named-reference supply form (Phase 132, §2.5) and is
/// position-disciplined: it binds only in a FUNCTION
/// position (the parameter's type is a function type),
/// only where the route carries a module index, and only
/// when the named spec's signature fits the position
/// (`signature_fits`). Each failure is a SPECIFIC decline
/// naming the name and the class — a named reference
/// never silently binds and never silently vanishes into
/// the generic class. A bare name in a non-function
/// position that resolves to nothing is not a reference
/// at all and keeps the generic failure, as before.
/// Every other form maps through `value_of_expr_indexed`.
fn bind_part(
    part: &Expr,
    param: &Param,
    index: Option<&SpecIndex>,
) -> Result<Value, BindDecline> {
    if let Expr::Var(name) = part {
        let fn_position = matches!(param.ty, TypeRef::Function(..));
        match index {
            Some(index) => {
                if fn_position {
                    let value =
                        named_function_value(index, name).map_err(BindDecline::Specific)?;
                    let named = index
                        .get(name)
                        .expect("a resolved reference names an indexed spec");
                    if !signature_fits(named, &param.ty) {
                        let actual = TypeRef::Function(
                            named.params.iter().map(|p| p.ty.clone()).collect(),
                            Box::new(named.return_type.clone()),
                        );
                        return Err(BindDecline::Specific(format!(
                            "the named function reference `{name}` does not match the \
                             function type of the parameter it is supplied for: \
                             `{name}` is {actual}, the position requires {}",
                            param.ty
                        )));
                    }
                    return Ok(value);
                }
                if index.get(name).is_some() {
                    return Err(BindDecline::Specific(format!(
                        "the named reference `{name}` stands in a non-function position: \
                         a bare name in a test input is a function supply form \
                         (G4 §2.5), and the parameter it would bind is {}, \
                         not a function type",
                        param.ty
                    )));
                }
            }
            None => {
                if fn_position {
                    return Err(BindDecline::Specific(format!(
                        "the named function reference `{name}` cannot be resolved here: \
                         named supply resolves through a module index, and this \
                         judging route carries none"
                    )));
                }
            }
        }
    }
    value_of_expr_indexed(part, index).ok_or(BindDecline::Generic)
}

/// Outcome of running a candidate body against a spec's tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestRun {
    /// Tests whose input AND expected output could be mapped
    /// to evaluator values, minus any point the spec's own
    /// `require:` excludes (Phase 109: spec-silent points
    /// are reported per test but never enter this
    /// denominator). Tests that cannot be mapped are
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
/// tuple-carrying residue: nested tuples — there is no tuple
/// Value. Named function references bind since Phase 132
/// wherever the count is taken with a module index —
/// `bindable_test_count_in`; this context-free count still
/// reads them as residue, having no index to resolve them
/// against) — the trigger signal for the compiled-runner final
/// scoring in synthesis::runner (Phase 58), which exists
/// for exactly that residue.
pub fn bindable_test_count(spec: &FunctionSpec) -> usize {
    test_bindability(spec).iter().filter(|b| **b).count()
}

/// `bindable_test_count` with a module index in hand:
/// tests supplying a function by named reference
/// (Phase 132, §2.5) bind on this route.
pub fn bindable_test_count_in(spec: &FunctionSpec, index: &SpecIndex) -> usize {
    test_bindability_in(spec, index)
        .iter()
        .filter(|b| **b)
        .count()
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
    test_bindability_indexed(spec, None)
}

/// `test_bindability` with a module index in hand
/// (Phase 132: named-reference tests bind).
pub fn test_bindability_in(spec: &FunctionSpec, index: &SpecIndex) -> Vec<bool> {
    test_bindability_indexed(spec, Some(index))
}

fn test_bindability_indexed(spec: &FunctionSpec, index: Option<&SpecIndex>) -> Vec<bool> {
    spec.tests
        .iter()
        .map(|t| {
            bind_test_env_indexed(&spec.params, &t.input, index).is_ok()
                && value_of_expr_indexed(&t.expected_output, index).is_some()
        })
        .collect()
}

// ── Preconditions at concrete points (Phase 109) ──────
//
// §3.1's executed half: a test point that violates the
// spec's own `require:` is spec-silent (see
// `TestOutcome::SpecSilent`). Deciding that needs the
// preconditions evaluated at the point — three-valued:
// `Some(true)` / `Some(false)` where the row's shape is
// concretely evaluable, `None` where it is not (a
// quantified row, a named predicate, a method call the
// IR lowering drops to a hole, a call to a sibling spec —
// the interpreter has no definitions for those here).
// Only a PROVEN violation silences a point; an undecided
// row leaves the point judged exactly as before, so this
// evaluator can only ever shrink the judged set toward
// the points the spec actually governs, never hide a
// failure the spec does speak about.

/// Evaluate a spec expression at a concrete point by the
/// same route `value_of_expr` uses for composites: lower
/// through the engine's `expr_to_ir` and run the
/// interpreter in the point's environment.
fn value_of_expr_at(expr: &Expr, env: &Env) -> Option<Value> {
    let ir = super::engine::expr_to_ir(expr);
    eval(&ir, env).ok()
}

/// Three-valued evaluation of one constraint at a point.
fn constraint_holds_at(c: &Constraint, env: &Env) -> Option<bool> {
    match c {
        Constraint::True => Some(true),
        Constraint::Expr(e) => match value_of_expr_at(e, env) {
            Some(Value::Bool(b)) => Some(b),
            _ => None,
        },
        Constraint::Eq(a, b) => {
            let (x, y) = (value_of_expr_at(a, env)?, value_of_expr_at(b, env)?);
            // A comparison involving a function value
            // declines (values_equal's None): the row is
            // undecided at this point — it neither silences
            // the point nor judges it (Phase 130, §2.6).
            values_equal(&x, &y)
        }
        Constraint::Order(op, a, b) => {
            let (x, y) = (value_of_expr_at(a, env)?, value_of_expr_at(b, env)?);
            let ord = match (&x, &y) {
                (Value::Int(m), Value::Int(n)) => m.cmp(n),
                (Value::Char(m), Value::Char(n)) => m.cmp(n),
                (Value::Float(m), Value::Float(n)) => m.partial_cmp(n)?,
                _ => return None,
            };
            Some(match op {
                OrderOp::LessThan => ord == std::cmp::Ordering::Less,
                OrderOp::LessThanOrEqual => ord != std::cmp::Ordering::Greater,
                OrderOp::GreaterThan => ord == std::cmp::Ordering::Greater,
                OrderOp::GreaterThanOrEqual => ord != std::cmp::Ordering::Less,
            })
        }
        Constraint::And(parts) => {
            let mut unknown = false;
            for p in parts {
                match constraint_holds_at(p, env) {
                    Some(false) => return Some(false),
                    Some(true) => {}
                    None => unknown = true,
                }
            }
            if unknown { None } else { Some(true) }
        }
        Constraint::Or(parts) => {
            let mut unknown = false;
            for p in parts {
                match constraint_holds_at(p, env) {
                    Some(true) => return Some(true),
                    Some(false) => {}
                    None => unknown = true,
                }
            }
            if unknown { None } else { Some(false) }
        }
        Constraint::Not(inner) => constraint_holds_at(inner, env).map(|b| !b),
        Constraint::Implies(a, b) => {
            match (constraint_holds_at(a, env), constraint_holds_at(b, env)) {
                (Some(false), _) => Some(true),
                (Some(true), r) => r,
                (None, Some(true)) => Some(true),
                (None, _) => None,
            }
        }
        // Quantified rows and named predicates have no
        // concrete evaluation here.
        Constraint::Forall { .. } | Constraint::Exists { .. } | Constraint::Predicate(..) => {
            None
        }
    }
}

/// Whether the point bound in `env` PROVABLY violates
/// the spec's `require:` preconditions: some row decides
/// false at the point. Rows that cannot be decided do
/// not silence the point.
fn precondition_violated_at(spec: &FunctionSpec, env: &Env) -> bool {
    spec.preconditions
        .iter()
        .any(|pre| constraint_holds_at(pre, env) == Some(false))
}

/// Per-test outcome of executing one candidate body against
/// a spec's tests — the fine-grained form of `TestRun`
/// (Phase 68, v0.6 W3). `run_spec_tests` is a fold over
/// these: a test counts as evaluable iff its outcome is
/// neither `Unbindable` nor `SpecSilent`, and as passed iff
/// its outcome is `Passed`.
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
    /// Also this outcome when the body DID produce a value
    /// but the expected-output comparison declines it — a
    /// function value on either side of the comparison
    /// (Phase 130, §2.6): the interpreter could judge the
    /// body; the comparison is what is outside its subset.
    BodyNotEvaluable(String),
    /// The test itself cannot be mapped to interpreter
    /// values, so no candidate could ever be judged against
    /// it. The payload is the reason CLASS: the expected
    /// output is unmappable; the input is unmappable (it
    /// carries a value with no interpreter representation,
    /// e.g. a tuple — there is no tuple value; that is the
    /// remaining unrepresentable class since Phase 132);
    /// a named function reference declines (an unknown
    /// name, a name denoting no executable body, a
    /// signature that does not fit its position, a name in
    /// a non-function position, or no module index on the
    /// judging route — each named in the payload); or the
    /// input's shape does not match the parameter list.
    Unbindable(String),
    /// The test binds, but its input VIOLATES the spec's
    /// own `require:` precondition (Phase 109, §3.1): the
    /// spec makes no promise at that point, so the point
    /// is spec-silent — reported here with the reason, and
    /// excluded from the evaluable denominator exactly
    /// like `Unbindable`. No candidate may be passed or
    /// failed on it, and the body is never evaluated
    /// there. Only a point PROVEN to violate (a
    /// precondition row the concrete evaluator below can
    /// decide, deciding false) is silent; a precondition
    /// it cannot decide leaves the point judged as before.
    SpecSilent(String),
}

/// Execute `body` against every test in `spec.tests`,
/// reporting the per-test outcome (in `spec.tests` order).
pub fn test_outcomes(body: &IRNode, spec: &FunctionSpec) -> Vec<TestOutcome> {
    test_outcomes_indexed(body, spec, None)
}

/// `test_outcomes` with a module index in hand: tests
/// supplying a function by named reference (Phase 132,
/// §2.5) bind and judge on this route.
pub fn test_outcomes_in(
    body: &IRNode,
    spec: &FunctionSpec,
    index: &SpecIndex,
) -> Vec<TestOutcome> {
    test_outcomes_indexed(body, spec, Some(index))
}

fn test_outcomes_indexed(
    body: &IRNode,
    spec: &FunctionSpec,
    index: Option<&SpecIndex>,
) -> Vec<TestOutcome> {
    spec.tests
        .iter()
        .map(|test| {
            let Some(expected) = value_of_expr_indexed(&test.expected_output, index) else {
                return TestOutcome::Unbindable(
                    "expected output cannot be mapped to an interpreter value".into(),
                );
            };
            let env = match bind_test_env_indexed(&spec.params, &test.input, index) {
                Ok(env) => env,
                // A named reference's decline is specific:
                // its reason IS the outcome's payload (the
                // name and the class), never the generic
                // class below.
                Err(BindDecline::Specific(reason)) => {
                    return TestOutcome::Unbindable(reason);
                }
                Err(BindDecline::Generic) => {
                    return TestOutcome::Unbindable(
                        if value_of_expr_indexed(&test.input, index).is_none() {
                            "input cannot be mapped to an interpreter value \
                             (it carries a value the interpreter cannot represent, \
                             e.g. a tuple)"
                                .into()
                        } else {
                            "input does not match the spec's parameter list (shape/arity)"
                                .into()
                        },
                    );
                }
            };
            // Phase 109 (§3.1): a point the spec's own
            // `require:` excludes is spec-silent — the
            // body is never evaluated there and the point
            // is never a body failure.
            if precondition_violated_at(spec, &env) {
                return TestOutcome::SpecSilent(
                    "input violates the spec's `require:` precondition — \
                     the spec makes no promise at this point"
                        .into(),
                );
            }
            match eval(body, &env) {
                Ok(actual) => match values_equal(&actual, &expected) {
                    Some(true) => TestOutcome::Passed,
                    Some(false) => TestOutcome::Failed,
                    // The comparison itself declines
                    // (Phase 130, §2.6): the body produced a
                    // value (or the expectation is one) that
                    // is — or contains — a function value,
                    // and function values have no equality.
                    // The test counts as evaluable (the
                    // interpreter could judge the body; it
                    // is the COMPARISON that is outside its
                    // subset) and can never be passed on a
                    // structural guess.
                    None => TestOutcome::BodyNotEvaluable(
                        "expected-output comparison declines function values: \
                         function values have no equality (G4 §2.6)"
                            .into(),
                    ),
                },
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
            .filter(|o| {
                !matches!(o, TestOutcome::Unbindable(_) | TestOutcome::SpecSilent(_))
            })
            .count(),
        passed: outcomes
            .iter()
            .filter(|o| matches!(o, TestOutcome::Passed))
            .count(),
    }
}

/// `run_spec_tests` with a module index in hand
/// (Phase 132: named-reference tests bind and judge).
pub fn run_spec_tests_in(body: &IRNode, spec: &FunctionSpec, index: &SpecIndex) -> TestRun {
    let outcomes = test_outcomes_in(body, spec, index);
    TestRun {
        evaluable: outcomes
            .iter()
            .filter(|o| {
                !matches!(o, TestOutcome::Unbindable(_) | TestOutcome::SpecSilent(_))
            })
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
    valued_test_count_indexed(body, spec, None)
}

/// `valued_test_count` with a module index in hand
/// (Phase 132: named-reference tests bind).
pub fn valued_test_count_in(body: &IRNode, spec: &FunctionSpec, index: &SpecIndex) -> usize {
    valued_test_count_indexed(body, spec, Some(index))
}

fn valued_test_count_indexed(
    body: &IRNode,
    spec: &FunctionSpec,
    index: Option<&SpecIndex>,
) -> usize {
    let mut valued = 0;
    for test in &spec.tests {
        let (Ok(env), Some(_)) = (
            bind_test_env_indexed(&spec.params, &test.input, index),
            value_of_expr_indexed(&test.expected_output, index),
        ) else {
            continue;
        };
        // Phase 109 (§3.1): a spec-silent point (below)
        // is not evidence about the body either way —
        // its answer is as meaningless here as it is in
        // `test_outcomes`.
        if precondition_violated_at(spec, &env) {
            continue;
        }
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

    // ── Phase 100 (F2): the `int_to_str` builtin ─────────

    /// The conversion battery (Phase 99 probe 5's table,
    /// plus the tokenizer's line numbers): the interpreter
    /// must render each value EXACTLY as Rust's `Display`
    /// — the same values the encoder's literal fold and
    /// the compiled emitter are judged against in their
    /// own Phase 100 tests.
    const INT_TO_STR_BATTERY: &[(i64, &str)] = &[
        (0, "0"),
        (1, "1"),
        (2, "2"),
        (3, "3"),
        (9, "9"),
        (10, "10"),
        (42, "42"),
        (-1, "-1"),
        (-42, "-42"),
        (1_000_000, "1000000"),
        (i64::MAX, "9223372036854775807"),
        (i64::MIN, "-9223372036854775808"),
    ];

    #[test]
    fn eval_int_to_str_battery() {
        let env = Env::new();
        for (n, want) in INT_TO_STR_BATTERY {
            let call = IRNode::Call {
                function: "int_to_str".into(),
                args: vec![int(*n)],
            };
            assert_eq!(
                eval(&call, &env),
                Ok(Value::Str(want.to_string())),
                "int_to_str({}) must render as Rust Display",
                n
            );
        }
        // A variable argument converts its bound value.
        let env = env_with(&[("line", Value::Int(7))]);
        let call = IRNode::Call {
            function: "int_to_str".into(),
            args: vec![var("line")],
        };
        assert_eq!(eval(&call, &env), Ok(Value::Str("7".into())));
    }

    #[test]
    fn eval_int_to_str_wrong_shape_is_not_evaluable() {
        // Wrong type and wrong arity are named failures,
        // never guessed values (the `len` arm's discipline).
        let env = Env::new();
        let bad_type = IRNode::Call {
            function: "int_to_str".into(),
            args: vec![string("x")],
        };
        assert!(
            matches!(eval(&bad_type, &env), Err(EvalError::NotEvaluable(_))),
            "int_to_str(\"x\") must be NotEvaluable"
        );
        let bad_arity = IRNode::Call {
            function: "int_to_str".into(),
            args: vec![int(1), int(2)],
        };
        assert!(
            matches!(eval(&bad_arity, &env), Err(EvalError::NotEvaluable(_))),
            "int_to_str(1, 2) must be NotEvaluable"
        );
        let zero_arity = IRNode::Call {
            function: "int_to_str".into(),
            args: vec![],
        };
        assert!(
            matches!(eval(&zero_arity, &env), Err(EvalError::NotEvaluable(_))),
            "int_to_str() must be NotEvaluable"
        );
    }

    #[test]
    fn eval_int_to_str_in_statement_position() {
        // The Phase 95 statement subset evaluates calls
        // through `eval_call`: a statement body that binds
        // the conversion and returns it judges identically.
        let body = IRNode::Block(vec![
            IRNode::Alloc {
                name: "msg".into(),
                ty: crate::spec::ast::TypeRef::String,
                initial: Box::new(IRNode::Call {
                    function: "int_to_str".into(),
                    args: vec![int(-42)],
                }),
            },
            IRNode::Return(Box::new(var("msg"))),
        ]);
        assert_eq!(
            eval(&body, &Env::new()),
            Ok(Value::Str("-42".into())),
            "statement-position int_to_str must evaluate"
        );
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
        // Re-pinned at Phase 95: Alloc, Assign and While
        // left this list — they are the statement subset
        // now (their positive tests are the phase95_* tests
        // below). The While case that remains here is the
        // non-terminating one: it is NotEvaluable by the
        // iteration bound, not by shape. The Assign case
        // remains because its target is unbound in this
        // env (no Alloc binds `x`).
        // Re-pinned at Phase 130 (G4): Lambda left this
        // list — it evaluates to a function value now (the
        // phase130_* tests are its positive coverage).
        // Re-pinned at Phase 131 (G4): the Map case left
        // this list with the rest of the higher-order
        // nodes — they evaluate now (the phase131_* tests
        // are their positive coverage, declines included).
        let cases: Vec<IRNode> = vec![
            IRNode::Hole { id: 0, expected_type: None },
            IRNode::While {
                condition: Box::new(boolean(true)),
                body: Box::new(int(1)),
                invariant: None,
            },
            IRNode::Assign {
                target: Box::new(var("x")),
                value: Box::new(int(1)),
            },
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
        // A body the interpreter cannot execute: bound
        // tests, but every outcome is BodyNotEvaluable,
        // and the tests still count as evaluable in the
        // fold. (The example is a Head body — the list
        // primitives stay outside the subset. Until
        // Phase 115 this example was a For body; For
        // graduated into the subset with the List
        // family's judging machinery, so the example
        // moved to a form that still declines.)
        let loopy = IRNode::UnaryOp(IRUnaryOp::Head, Box::new(var("kind")));
        let outcomes = test_outcomes(&loopy, &spec);
        assert!(
            outcomes.iter().all(|o| matches!(o, TestOutcome::BodyNotEvaluable(_))),
            "unevaluable body must be BodyNotEvaluable on every test: {outcomes:?}"
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
        // A lambda input for a fn-typed parameter: until
        // Phase 130 (G4) this was the Unbindable exemplar —
        // the evaluator had no function values (the sort
        // residue). A lambda maps to a closure now, so the
        // test binds and judges like any other.
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
        assert_eq!(outcomes, vec![TestOutcome::Passed]);
        assert_eq!(
            run_spec_tests(&boolean(true), &spec),
            TestRun { evaluable: 1, passed: 1 }
        );

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
        // The unbindable test does not enter the evaluable count.
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
        // Unevaluable (unbound variable / hole): valued nowhere.
        assert_eq!(valued_test_count(&var("result"), &spec), 0);
        assert_eq!(
            valued_test_count(&IRNode::Hole { id: 0, expected_type: None }, &spec),
            0
        );
        // Re-pinned at Phase 95: the Alloc/Return body
        // below used to be this test's unevaluable example;
        // Alloc is in the evaluable subset now, so the body
        // answers on every test (valued 3 — it returns the
        // alloc'd 0, wrong on the merits, which is exactly
        // the distinction this counter exists to draw). The
        // still-unevaluable example is a Map body: the
        // higher-order nodes stay outside the subset until
        // Phase 131 (G4). (Until Phase 130 the example was
        // a Lambda body — a Lambda evaluates to a function
        // value now, so it values everywhere. Until
        // Phase 115 the example was a For body; For
        // graduated into the subset with the List family's
        // judging machinery.)
        let alloc_body = IRNode::Block(vec![
            IRNode::Alloc {
                name: "acc".into(),
                ty: crate::spec::ast::TypeRef::Int,
                initial: Box::new(int(0)),
            },
            IRNode::Return(Box::new(var("acc"))),
        ]);
        assert_eq!(valued_test_count(&alloc_body, &spec), 3);
        let map_body = IRNode::Map {
            function: Box::new(IRNode::Lambda {
                params: vec![("x".into(), crate::spec::ast::TypeRef::Int)],
                body: Box::new(var("kind")),
            }),
            over: Box::new(var("kind")),
        };
        assert_eq!(valued_test_count(&map_body, &spec), 0);
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
        let env = bind_test_env_indexed(&spec.params, &spec.tests[0].input, None)
            .ok()
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
    fn run_spec_tests_tuple_with_lambda_element_binds() {
        // sort.morph's shape: (list, cmp) where cmp's test
        // value is a lambda. Until Phase 130 (G4) the element
        // could not be mapped — the evaluator had no function
        // values — and the test stayed excluded (this test's
        // previous form pinned exactly that). A lambda maps
        // to a closure now: the test binds, and the identity
        // body fails it on the merits (the list comes back
        // unsorted), which is the FALLBACK leaving this
        // shape's domain.
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
        let env = bind_test_env_indexed(&spec.params, &spec.tests[0].input, None)
            .ok()
            .expect("tuple input with a lambda element must bind");
        assert!(
            matches!(env.get("cmp"), Some(Value::Function { .. })),
            "the lambda element binds as a function value, got {:?}",
            env.get("cmp")
        );
        assert_eq!(bindable_test_count(&spec), 1);
        let run = run_spec_tests(&var("list"), &spec);
        assert_eq!(run, TestRun { evaluable: 1, passed: 0 });
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
    fn try_propagate_evaluates_result_threading() {
        // Phase 95 closes the Phase 75 residual this test
        // used to pin (`try_propagate_remains_outside_the_
        // evaluable_subset`): `?` propagation is in the
        // subset now. An Ok payload continues as the value.
        let node = IRNode::TryPropagate(Box::new(call_ir("Ok", vec![int(1)])));
        assert_eq!(eval(&node, &Env::new()), Ok(Value::Int(1)));
        // An Err unwinds the enclosing body: the statement
        // after it (a Hole, which would fail evaluation if
        // reached) never runs, and the body's result is
        // the Err itself.
        let body = IRNode::Block(vec![
            IRNode::TryPropagate(Box::new(call_ir(
                "Err",
                vec![string("boom")],
            ))),
            IRNode::Hole {
                id: 0,
                expected_type: None,
            },
        ]);
        assert_eq!(
            eval(&body, &Env::new()),
            Ok(Value::Err(Box::new(Value::Str("boom".into()))))
        );
        // A non-Result operand is a named failure, not a
        // guessed payload.
        let node = IRNode::TryPropagate(Box::new(int(1)));
        match eval(&node, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("TryPropagate") && reason.contains("non-Result"),
                    "the failure must name the cause, got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
    }

    // ── Phase 95: the statement/loop subset ───────────
    // (Encoder extension, W9: Alloc / Assign / While /
    // Loop / Break / TryPropagate evaluation, constructor
    // Match, Clone, statement Append — the shapes the
    // denoted and template bodies are made of.)

    fn alloc(name: &str, initial: IRNode) -> IRNode {
        IRNode::Alloc {
            name: name.into(),
            ty: crate::spec::ast::TypeRef::Int,
            initial: Box::new(initial),
        }
    }

    fn assign(name: &str, value: IRNode) -> IRNode {
        IRNode::Assign {
            target: Box::new(var(name)),
            value: Box::new(value),
        }
    }

    #[test]
    fn phase95_alloc_assign_and_block_statements() {
        // Alloc binds; Assign rebinds the binding; the
        // block's value is its last statement's.
        let body = IRNode::Block(vec![
            alloc("x", int(1)),
            assign("x", binop(IRBinOp::Add, var("x"), int(2))),
            var("x"),
        ]);
        assert_eq!(eval(&body, &Env::new()), Ok(Value::Int(3)));
        // An Alloc's own value is the bound value (the
        // statement layer's running-value discipline).
        assert_eq!(eval(&alloc("x", int(5)), &Env::new()), Ok(Value::Int(5)));
        // Assign to a variable nothing binds: a named
        // failure, not a silent creation.
        match eval(&assign("ghost", int(1)), &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("ghost") && reason.contains("unbound"),
                    "the failure must name the variable, got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
        // An indexed Assign target (Phase 131): the place
        // is written, and the statement's value is the
        // assigned value. (The mutation itself is observed
        // through a following read in the phase131 tests;
        // the point here is the shape left the decline
        // list.) A target that is neither a variable nor an
        // indexed place keeps the named decline.
        let indexed = IRNode::Assign {
            target: Box::new(IRNode::Index(Box::new(var("xs")), Box::new(int(0)))),
            value: Box::new(int(1)),
        };
        assert_eq!(
            eval(
                &indexed,
                &env_with(&[("xs", Value::List(vec![Value::Int(0)]))])
            ),
            Ok(Value::Int(1))
        );
        let bad = IRNode::Assign {
            target: Box::new(binop(IRBinOp::Add, var("xs"), int(1))),
            value: Box::new(int(1)),
        };
        match eval(&bad, &env_with(&[("xs", Value::List(vec![Value::Int(0)]))])) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("not a variable or an indexed place"),
                    "the failure must name the cause, got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
    }

    #[test]
    fn phase95_while_and_loop_with_break() {
        // While: mutation persists across iterations.
        let counter = IRNode::Block(vec![
            alloc("n", int(0)),
            IRNode::While {
                condition: Box::new(binop(IRBinOp::Lt, var("n"), int(5))),
                body: Box::new(IRNode::Block(vec![assign(
                    "n",
                    binop(IRBinOp::Add, var("n"), int(1)),
                )])),
                invariant: None,
            },
            var("n"),
        ]);
        assert_eq!(eval(&counter, &Env::new()), Ok(Value::Int(5)));
        // While exited by a bare Break in a branch.
        let breaker = IRNode::Block(vec![
            alloc("n", int(0)),
            IRNode::While {
                condition: Box::new(boolean(true)),
                body: Box::new(IRNode::Block(vec![
                    assign("n", binop(IRBinOp::Add, var("n"), int(1))),
                    IRNode::If {
                        condition: Box::new(binop(IRBinOp::Ge, var("n"), int(3))),
                        then_branch: Box::new(IRNode::Break { value: None }),
                        else_branch: Box::new(IRNode::Literal(IRLiteral::Unit)),
                    },
                ])),
                invariant: None,
            },
            var("n"),
        ]);
        assert_eq!(eval(&breaker, &Env::new()), Ok(Value::Int(3)));
        // Loop with a valued Break: the value is the
        // loop's value (the compiled `loop { break v }`).
        let valued = IRNode::Block(vec![
            alloc("n", int(0)),
            IRNode::Loop {
                invariant: None,
                body: Box::new(IRNode::Block(vec![
                    assign("n", binop(IRBinOp::Add, var("n"), int(1))),
                    IRNode::If {
                        condition: Box::new(binop(IRBinOp::Eq, var("n"), int(4))),
                        then_branch: Box::new(IRNode::Break {
                            value: Some(Box::new(binop(
                                IRBinOp::Mul,
                                var("n"),
                                int(10),
                            ))),
                        }),
                        else_branch: Box::new(IRNode::Literal(IRLiteral::Unit)),
                    },
                ])),
            },
        ]);
        assert_eq!(eval(&valued, &Env::new()), Ok(Value::Int(40)));
        // A Break with no enclosing loop: named failure.
        match eval(&IRNode::Break { value: None }, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("Break outside a loop"),
                    "got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
        // The valued Break form inside a While is the one
        // declined form: named, not assigned a meaning.
        let declined = IRNode::While {
            condition: Box::new(boolean(true)),
            body: Box::new(IRNode::Break {
                value: Some(Box::new(int(1))),
            }),
            invariant: None,
        };
        match eval(&declined, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("valued Break inside a While"),
                    "the declined form must be named, got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
    }

    #[test]
    fn phase95_iteration_bound_judges_nontermination() {
        // A While that never exits: judged NotEvaluable by
        // the bound — promptly (the test completing at all
        // is the wall-time evidence) and with the bound
        // named in the reason.
        let forever = IRNode::While {
            condition: Box::new(boolean(true)),
            body: Box::new(IRNode::Literal(IRLiteral::Unit)),
            invariant: None,
        };
        match eval(&forever, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("iteration bound"),
                    "the reason must name the bound, got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
        // A bare Loop with no Break: the same judgment.
        let forever = IRNode::Loop {
            body: Box::new(IRNode::Literal(IRLiteral::Unit)),
            invariant: None,
        };
        assert!(matches!(
            eval(&forever, &Env::new()),
            Err(EvalError::NotEvaluable(_))
        ));
        // A loop that terminates within the bound is
        // judged on its merits: counting to the bound
        // minus one succeeds.
        let counter = IRNode::Block(vec![
            alloc("n", int(0)),
            IRNode::While {
                condition: Box::new(binop(
                    IRBinOp::Lt,
                    var("n"),
                    int((MAX_LOOP_ITERATIONS - 1) as i64),
                )),
                body: Box::new(IRNode::Block(vec![assign(
                    "n",
                    binop(IRBinOp::Add, var("n"), int(1)),
                )])),
                invariant: None,
            },
            var("n"),
        ]);
        assert_eq!(
            eval(&counter, &Env::new()),
            Ok(Value::Int((MAX_LOOP_ITERATIONS - 1) as i64))
        );
    }

    #[test]
    fn phase95_constructor_match_binds_payloads() {
        let match_on = |scrutinee: IRNode, cases: Vec<(IRPattern, IRNode)>| IRNode::Match {
            scrutinee: Box::new(scrutinee),
            cases,
        };
        let ctor = |name: &str, subs: Vec<IRPattern>| {
            IRPattern::Constructor(name.into(), subs)
        };
        // Some(ch) binds the payload — peek's dispatch
        // shape, the canonical loop condition.
        let body = match_on(
            call_ir("Some", vec![chr('h')]),
            vec![
                (ctor("None", vec![]), boolean(false)),
                (
                    ctor("Some", vec![IRPattern::Variable("ch".into())]),
                    binop(IRBinOp::Eq, var("ch"), chr('h')),
                ),
            ],
        );
        assert_eq!(eval(&body, &Env::new()), Ok(Value::Bool(true)));
        // None takes the nullary arm.
        let body = match_on(
            call_ir("None", vec![]),
            vec![
                (ctor("None", vec![]), boolean(true)),
                (
                    ctor("Some", vec![IRPattern::Variable("ch".into())]),
                    boolean(false),
                ),
            ],
        );
        assert_eq!(eval(&body, &Env::new()), Ok(Value::Bool(true)));
        // Ok/Err bind their payloads.
        let body = match_on(
            call_ir("Err", vec![string("boom")]),
            vec![
                (
                    ctor("Ok", vec![IRPattern::Variable("v".into())]),
                    int(0),
                ),
                (
                    ctor("Err", vec![IRPattern::Variable("e".into())]),
                    call_ir("len", vec![var("e")]),
                ),
            ],
        );
        assert_eq!(eval(&body, &Env::new()), Ok(Value::Int(4)));
        // A constructor of another family does not match:
        // the match falls through to the wildcard arm.
        let body = match_on(
            call_ir("Some", vec![int(1)]),
            vec![
                (
                    ctor("Ok", vec![IRPattern::Variable("v".into())]),
                    int(0),
                ),
                (IRPattern::Wildcard, int(9)),
            ],
        );
        assert_eq!(eval(&body, &Env::new()), Ok(Value::Int(9)));
        // Nested constructor patterns match nested values.
        let body = match_on(
            call_ir("Some", vec![call_ir("Ok", vec![int(3)])]),
            vec![(
                ctor(
                    "Some",
                    vec![ctor("Ok", vec![IRPattern::Variable("x".into())])],
                ),
                binop(IRBinOp::Add, var("x"), int(1)),
            )],
        );
        assert_eq!(eval(&body, &Env::new()), Ok(Value::Int(4)));
        // A record constructor binds fields positionally
        // (written order — the record value's convention).
        let env = env_with(&[("t", token_val(7, "x", 1, 1))]);
        let body = match_on(
            var("t"),
            vec![(
                ctor(
                    "Token",
                    vec![
                        IRPattern::Variable("k".into()),
                        IRPattern::Wildcard,
                        IRPattern::Wildcard,
                        IRPattern::Wildcard,
                    ],
                ),
                var("k"),
            )],
        );
        assert_eq!(eval(&body, &env), Ok(Value::Int(7)));
        // Teeth: the same scrutinee under SWAPPED arms
        // gives the opposite answer — the arms genuinely
        // discriminate, they do not both fire or both miss.
        let swapped = match_on(
            call_ir("Some", vec![chr('h')]),
            vec![
                (ctor("None", vec![]), boolean(true)),
                (
                    ctor("Some", vec![IRPattern::Variable("ch".into())]),
                    boolean(false),
                ),
            ],
        );
        assert_eq!(eval(&swapped, &Env::new()), Ok(Value::Bool(false)));
        // An arity-contradicting constructor pattern is a
        // named decline, not a silent miss.
        let bad = match_on(
            call_ir("Some", vec![int(1)]),
            vec![(ctor("Some", vec![]), int(0))],
        );
        match eval(&bad, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("sub-patterns"),
                    "the decline must be named, got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
    }

    #[test]
    fn phase95_clone_is_the_identity() {
        // Over a scalar, a list, and a record: a copy of an
        // immutable value is the value (the encoder's W3
        // reading, Phase 91).
        assert_eq!(
            eval(
                &IRNode::UnaryOp(IRUnaryOp::Clone, Box::new(int(4))),
                &Env::new()
            ),
            Ok(Value::Int(4))
        );
        let env = env_with(&[("t", token_val(0, "spec", 1, 1))]);
        assert_eq!(
            eval(
                &IRNode::UnaryOp(IRUnaryOp::Clone, Box::new(var("t"))),
                &env
            ),
            Ok(token_val(0, "spec", 1, 1))
        );
        // The list operations beside Clone stay outside.
        let list = IRNode::Collection {
            kind: CollectionKind::List,
            elements: vec![int(1)],
        };
        assert!(eval(
            &IRNode::UnaryOp(IRUnaryOp::Head, Box::new(list)),
            &Env::new()
        )
        .is_err());
    }

    // ── Phase 115: the For arm ────────────────────────
    // (The List family's judging machinery, STDLIB_SCOPE
    // §2.6: the compiled runner's native loop shape is
    // `for i in start..end` — its loop variable is the
    // one index the generated Rust can use — so the
    // in-process judge learns the same form: bounds
    // evaluated once, the variable bound per iteration
    // and scoped to the loop, the shared iteration
    // budget spent per iteration.)

    fn for_(var_name: &str, start: IRNode, end: IRNode, body: IRNode) -> IRNode {
        IRNode::For {
            var: var_name.into(),
            start: Box::new(start),
            end: Box::new(end),
            body: Box::new(body),
        }
    }

    #[test]
    fn phase115_for_scan_accumulates() {
        // The count-shaped scan (the runner fixture's
        // shape): count the elements above a limit, the
        // loop variable as the Index — the form the
        // compiled runner has always executed.
        let body = IRNode::Block(vec![
            alloc("count", int(0)),
            for_(
                "i",
                int(0),
                IRNode::UnaryOp(
                    IRUnaryOp::Len,
                    Box::new(var("values")),
                ),
                assign(
                    "count",
                    binop(
                        IRBinOp::Add,
                        var("count"),
                        IRNode::If {
                            condition: Box::new(binop(
                                IRBinOp::Gt,
                                IRNode::Index(
                                    Box::new(var("values")),
                                    Box::new(var("i")),
                                ),
                                var("limit"),
                            )),
                            then_branch: Box::new(int(1)),
                            else_branch: Box::new(int(0)),
                        },
                    ),
                ),
            ),
            var("count"),
        ]);
        let env = env_with(&[
            (
                "values",
                Value::List(vec![Value::Int(10), Value::Int(1), Value::Int(20)]),
            ),
            ("limit", Value::Int(5)),
        ]);
        assert_eq!(eval(&body, &env), Ok(Value::Int(2)));
        // A sum scan over the same shape.
        let sum = IRNode::Block(vec![
            alloc("acc", int(0)),
            for_(
                "i",
                int(0),
                IRNode::UnaryOp(IRUnaryOp::Len, Box::new(var("values"))),
                assign(
                    "acc",
                    binop(
                        IRBinOp::Add,
                        var("acc"),
                        IRNode::Index(Box::new(var("values")), Box::new(var("i"))),
                    ),
                ),
            ),
            var("acc"),
        ]);
        assert_eq!(eval(&sum, &env), Ok(Value::Int(31)));
    }

    #[test]
    fn phase115_for_bounds_and_scoping() {
        // Empty ranges run zero iterations, in either
        // direction; the accumulator is untouched.
        for (start, end) in [(3, 3), (5, 2), (0, 0)] {
            let body = IRNode::Block(vec![
                alloc("acc", int(7)),
                for_("i", int(start), int(end), assign("acc", int(0))),
                var("acc"),
            ]);
            assert_eq!(
                eval(&body, &Env::new()),
                Ok(Value::Int(7)),
                "range {start}..{end} must not iterate"
            );
        }
        // The loop variable is scoped to the loop (the
        // compiled semantics): an outer binding of the
        // same name survives the loop.
        let scoped = IRNode::Block(vec![
            alloc("i", int(99)),
            for_("i", int(0), int(3), IRNode::Literal(IRLiteral::Unit)),
            var("i"),
        ]);
        assert_eq!(eval(&scoped, &Env::new()), Ok(Value::Int(99)));
        // With no outer binding, the variable does not
        // leak out of the loop.
        let leaked = IRNode::Block(vec![
            for_("i", int(0), int(3), IRNode::Literal(IRLiteral::Unit)),
            var("i"),
        ]);
        match eval(&leaked, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(reason.contains("unbound"), "got: {reason}");
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
        // Bounds are evaluated once: a bound that reads
        // the accumulator does not move as it changes.
        let once = IRNode::Block(vec![
            alloc("acc", int(0)),
            alloc("n", int(3)),
            for_(
                "i",
                int(0),
                var("n"),
                IRNode::Block(vec![
                    assign("acc", binop(IRBinOp::Add, var("acc"), int(1))),
                    assign("n", binop(IRBinOp::Add, var("n"), int(10))),
                ]),
            ),
            var("acc"),
        ]);
        assert_eq!(eval(&once, &Env::new()), Ok(Value::Int(3)));
    }

    #[test]
    fn phase115_for_flows_and_declines() {
        // A Return inside the body leaves the whole
        // evaluation with its value.
        let early = IRNode::Block(vec![
            alloc("acc", int(0)),
            for_(
                "i",
                int(0),
                int(10),
                IRNode::If {
                    condition: Box::new(binop(IRBinOp::Eq, var("i"), int(3))),
                    then_branch: Box::new(IRNode::Return(Box::new(var("i")))),
                    else_branch: Box::new(IRNode::Literal(IRLiteral::Unit)),
                },
            ),
            var("acc"),
        ]);
        assert_eq!(eval(&early, &Env::new()), Ok(Value::Int(3)));
        // A bare Break exits the loop; the value after
        // the loop is the accumulator's.
        let breaker = IRNode::Block(vec![
            alloc("acc", int(0)),
            for_(
                "i",
                int(0),
                int(10),
                IRNode::Block(vec![
                    assign("acc", binop(IRBinOp::Add, var("acc"), int(1))),
                    IRNode::If {
                        condition: Box::new(binop(IRBinOp::Ge, var("acc"), int(3))),
                        then_branch: Box::new(IRNode::Break { value: None }),
                        else_branch: Box::new(IRNode::Literal(IRLiteral::Unit)),
                    },
                ]),
            ),
            var("acc"),
        ]);
        assert_eq!(eval(&breaker, &Env::new()), Ok(Value::Int(3)));
        // A valued Break inside a For: the compiled
        // language gives `for` no break value — declined
        // and named, exactly as in a While.
        let valued = for_(
            "i",
            int(0),
            int(3),
            IRNode::Break {
                value: Some(Box::new(int(1))),
            },
        );
        match eval(&valued, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(
                    reason.contains("valued Break") && reason.contains("For"),
                    "got: {reason}"
                );
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
        // A non-Int bound: declined and named.
        let bad_bound = for_("i", boolean(true), int(3), IRNode::Literal(IRLiteral::Unit));
        match eval(&bad_bound, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(reason.contains("For"), "got: {reason}");
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
        // The shared iteration budget binds For loops
        // too: a range past the bound is a judged
        // NotEvaluable, never a hang.
        let huge = for_(
            "i",
            int(0),
            int(20_000),
            IRNode::Literal(IRLiteral::Unit),
        );
        match eval(&huge, &Env::new()) {
            Err(EvalError::NotEvaluable(reason)) => {
                assert!(reason.contains("bound exceeded"), "got: {reason}");
            }
            other => panic!("expected NotEvaluable, got {other:?}"),
        }
    }

    #[test]
    fn phase95_append_str_char_and_statement_rebind() {
        // The lexeme shape as an expression: Append of a
        // Char onto a String.
        assert_eq!(
            eval(
                &binop(IRBinOp::Append, string("ab"), chr('c')),
                &Env::new()
            ),
            Ok(Value::Str("abc".into()))
        );
        // Statement position on a variable rebinds it —
        // the push the compiled runner performs: a token
        // list accumulated across statements reads back
        // with the appended element.
        let body = IRNode::Block(vec![
            alloc(
                "xs",
                IRNode::Collection {
                    kind: CollectionKind::List,
                    elements: vec![int(1)],
                },
            ),
            binop(IRBinOp::Append, var("xs"), int(2)),
            var("xs"),
        ]);
        // (the Alloc's declared type is Int in the helper;
        // the interpreter does not re-check it — the IR is
        // the typechecker's output, not its input.)
        assert_eq!(
            eval(&body, &Env::new()),
            Ok(Value::List(vec![Value::Int(1), Value::Int(2)]))
        );
        // The string form of the same rebind.
        let body = IRNode::Block(vec![
            alloc("s", string("")),
            binop(IRBinOp::Append, var("s"), chr('a')),
            binop(IRBinOp::Append, var("s"), chr('b')),
            var("s"),
        ]);
        assert_eq!(eval(&body, &Env::new()), Ok(Value::Str("ab".into())));
        // A non-variable Append target keeps the pure
        // form: the value is computed, nothing is rebound.
        assert_eq!(
            eval(
                &binop(
                    IRBinOp::Append,
                    IRNode::Collection {
                        kind: CollectionKind::List,
                        elements: vec![int(1)],
                    },
                    int(2)
                ),
                &Env::new()
            ),
            Ok(Value::List(vec![Value::Int(1), Value::Int(2)]))
        );
    }

    #[test]
    fn phase95_composition_boundary_block_values_and_try_unwind() {
        // A Block in a value position is a body: its
        // Return is its result. This is the rule the
        // in-process composition (callee bodies inlined
        // into Alloc initials / TryPropagate operands)
        // judges by.
        let body = IRNode::Block(vec![
            alloc(
                "x",
                IRNode::Block(vec![IRNode::Return(Box::new(int(5)))]),
            ),
            var("x"),
        ]);
        assert_eq!(eval(&body, &Env::new()), Ok(Value::Int(5)));
        // A TryPropagate over an inlined body returning
        // Ok binds the payload through the Alloc.
        let body = IRNode::Block(vec![
            alloc(
                "step",
                IRNode::TryPropagate(Box::new(IRNode::Block(vec![
                    IRNode::Return(Box::new(call_ir("Ok", vec![int(7)]))),
                ]))),
            ),
            var("step"),
        ]);
        assert_eq!(eval(&body, &Env::new()), Ok(Value::Int(7)));
        // And its Err unwind passes THROUGH the Alloc:
        // the statement after it (a Hole, which would fail
        // evaluation if reached) never runs, and the
        // enclosing body's result is the Err — the
        // compiled `let step = f()?;` behavior.
        let body = IRNode::Block(vec![
            alloc(
                "step",
                IRNode::TryPropagate(Box::new(IRNode::Block(vec![
                    IRNode::Return(Box::new(call_ir(
                        "Err",
                        vec![string("boom")],
                    ))),
                ]))),
            ),
            IRNode::Hole {
                id: 0,
                expected_type: None,
            },
        ]);
        assert_eq!(
            eval(&body, &Env::new()),
            Ok(Value::Err(Box::new(Value::Str("boom".into()))))
        );
    }

    /// The mirror skip body in statement form over the
    /// parameter `state`, with peek/next_char inlined over
    /// the local `cur` — the Phase 92 denotation's shape,
    /// hand-composed from this module's own `peek_body` /
    /// `next_char_body` transcriptions (which are written
    /// over `state`; the helpers below are their `cur`
    /// renamings).
    fn phase95_skip_body() -> IRNode {
        fn fa(base: IRNode, field: &str) -> IRNode {
            IRNode::FieldAccess {
                base: Box::new(base),
                field: field.into(),
            }
        }
        let peek_cur = IRNode::If {
            condition: Box::new(binop(
                IRBinOp::Lt,
                fa(var("cur"), "pos"),
                call_ir("len", vec![fa(var("cur"), "source")]),
            )),
            then_branch: Box::new(call_ir(
                "Some",
                vec![IRNode::Index(
                    Box::new(fa(var("cur"), "source")),
                    Box::new(fa(var("cur"), "pos")),
                )],
            )),
            else_branch: Box::new(call_ir("None", vec![])),
        };
        let is_newline = binop(
            IRBinOp::And,
            binop(
                IRBinOp::Lt,
                fa(var("cur"), "pos"),
                call_ir("len", vec![fa(var("cur"), "source")]),
            ),
            binop(
                IRBinOp::Eq,
                IRNode::Index(
                    Box::new(fa(var("cur"), "source")),
                    Box::new(fa(var("cur"), "pos")),
                ),
                chr('\n'),
            ),
        );
        let next_cur = IRNode::RecordLit {
            type_name: "TokenizerState".into(),
            fields: vec![
                ("source".into(), fa(var("cur"), "source")),
                (
                    "pos".into(),
                    binop(IRBinOp::Add, fa(var("cur"), "pos"), int(1)),
                ),
                (
                    "line".into(),
                    IRNode::If {
                        condition: Box::new(is_newline.clone()),
                        then_branch: Box::new(binop(
                            IRBinOp::Add,
                            fa(var("cur"), "line"),
                            int(1),
                        )),
                        else_branch: Box::new(fa(var("cur"), "line")),
                    },
                ),
                (
                    "col".into(),
                    IRNode::If {
                        condition: Box::new(is_newline),
                        then_branch: Box::new(int(1)),
                        else_branch: Box::new(binop(
                            IRBinOp::Add,
                            fa(var("cur"), "col"),
                            int(1),
                        )),
                    },
                ),
                ("tokens".into(), fa(var("cur"), "tokens")),
            ],
        };
        let ws = |ch: char| binop(IRBinOp::Eq, var("ch"), chr(ch));
        let cond = IRNode::Match {
            scrutinee: Box::new(peek_cur),
            cases: vec![
                (
                    IRPattern::Constructor("None".into(), vec![]),
                    boolean(false),
                ),
                (
                    IRPattern::Constructor(
                        "Some".into(),
                        vec![IRPattern::Variable("ch".into())],
                    ),
                    [ws(' '), ws('\t'), ws('\n'), ws('\r')]
                        .into_iter()
                        .reduce(|a, b| binop(IRBinOp::Or, a, b))
                        .unwrap(),
                ),
            ],
        };
        IRNode::Block(vec![
            alloc("cur", var("state")),
            IRNode::While {
                condition: Box::new(cond),
                body: Box::new(IRNode::Block(vec![IRNode::Assign {
                    target: Box::new(var("cur")),
                    value: Box::new(next_cur),
                }])),
                invariant: None,
            },
            IRNode::Return(Box::new(var("cur"))),
        ])
    }

    #[test]
    fn phase95_skip_body_passes_golden_tests_and_teeth_hold() {
        let specs = load_specs_from(RESTORED_TOKENIZER_MODULE);
        let skip = spec_named(&specs, "skip_whitespace");
        // The statement-shaped mirror body is judged
        // in-process now: both golden tests pass — the
        // Phase 75 record for this spec (binds, but the
        // loop body is unjudgeable) is superseded.
        assert_eq!(
            test_outcomes(&phase95_skip_body(), skip),
            vec![TestOutcome::Passed, TestOutcome::Passed]
        );
        assert_eq!(
            run_spec_tests(&phase95_skip_body(), skip),
            TestRun {
                evaluable: 2,
                passed: 2
            }
        );
        // Beyond the golden tests: a newline run advances
        // line/col through the loop (the mirror's own
        // arithmetic, executed in-process).
        let env = env_with(&[("state", state_val("\n\nab", 0, 1, 1, vec![]))]);
        assert_eq!(
            eval(&phase95_skip_body(), &env),
            Ok(state_val("\n\nab", 2, 3, 1, vec![]))
        );
        // Teeth: the identity body still fails the golden
        // tests in-process — it passes "no-whitespace" and
        // fails "skip-spaces", exactly as the compiled
        // judging has always recorded (the α scandal is a
        // spec-strength fact, not an interpreter gap).
        assert_eq!(
            run_spec_tests(&var("state"), skip),
            TestRun {
                evaluable: 2,
                passed: 1
            }
        );
        // Teeth: a partial skip (advance at most once,
        // unconditionally) fails BOTH golden tests — it
        // moves on "hi", where nothing may move, and it
        // stops after one space on "  hi". A wrong body is
        // judged wrong in-process, in both directions.
        let partial = IRNode::Block(vec![
            alloc("cur", var("state")),
            IRNode::If {
                condition: Box::new(binop(
                    IRBinOp::Lt,
                    field(var("cur"), "pos"),
                    call_ir("len", vec![field(var("cur"), "source")]),
                )),
                then_branch: Box::new(IRNode::Block(vec![IRNode::Assign {
                    target: Box::new(var("cur")),
                    value: Box::new(IRNode::RecordLit {
                        type_name: "TokenizerState".into(),
                        fields: vec![
                            ("source".into(), field(var("cur"), "source")),
                            (
                                "pos".into(),
                                binop(
                                    IRBinOp::Add,
                                    field(var("cur"), "pos"),
                                    int(1),
                                ),
                            ),
                            ("line".into(), field(var("cur"), "line")),
                            (
                                "col".into(),
                                binop(
                                    IRBinOp::Add,
                                    field(var("cur"), "col"),
                                    int(1),
                                ),
                            ),
                            ("tokens".into(), field(var("cur"), "tokens")),
                        ],
                    }),
                }])),
                else_branch: Box::new(IRNode::Literal(IRLiteral::Unit)),
            },
            IRNode::Return(Box::new(var("cur"))),
        ]);
        assert_eq!(
            run_spec_tests(&partial, skip),
            TestRun {
                evaluable: 2,
                passed: 0
            }
        );
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

        // skip_whitespace: the body used here is a
        // non-terminating While (condition `true`), so the
        // judgment is BodyNotEvaluable by the Phase 95
        // iteration bound — the tests bind and count as
        // evaluable, and the outcome says exactly why no
        // verdict was produced. (Before Phase 95 the
        // reason was the loop's shape; the terminating
        // mirror body is judged in-process by
        // `phase95_skip_body_passes_golden_tests_and_teeth_
        // hold` above.)
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
