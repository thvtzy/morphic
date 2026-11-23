//! Phase 130 (G4) — interpreter function values: the closure
//! core (G4_SCOPE §2.1–§2.3, §2.6; D1a/D2a/D5a).
//!
//! What this file pins, at the interpreter's own surface:
//!
//! * `IRNode::Lambda` / `IRNode::Closure` evaluate to the one
//!   function `Value` variant — Lambda by full-environment
//!   snapshot, Closure by name-resolved captures (§2.1/§2.2);
//! * lambda test rows BIND (the Phase 127 Unbindable standing
//!   ends) and `apply_twice` judges in-process 2/2 — the same
//!   body the compiled runner scores 2/2 (the Phase 127 probe
//!   pins the compiled side; the two judges agree on the
//!   shared points);
//! * application spends the standing evaluation budget
//!   (§2.3): sequential applications exhaust the shared
//!   step counter, and nested application is depth-bounded
//!   — a self-application bomb declines `NotEvaluable`
//!   instead of hanging or overflowing the host stack;
//! * a Let-bound function does not capture itself (§2.2);
//! * function values have NO equality (§2.6): `==`/`!=` over
//!   them declines by name, and test judging declines a
//!   comparison that would have to compare them — the
//!   derived-`==` fallback never decides one;
//! * the higher-order IR nodes (Map/Filter/Fold/Sort)
//!   stayed declined by name at this phase — they were
//!   Phase 131's arms. (Re-based at Phase 131: the nodes
//!   evaluate now; the boundary test below pins their new
//!   standing, and tests/phase131_ho_judging.rs carries
//!   their positive coverage.)

use morphic::spec::ast::{BinOp as SpecBinOp, Expr, TypeRef};
use morphic::spec::parser;
use morphic::synthesis::engine::{IRBinOp, IRLiteral, IRNode};
use morphic::synthesis::eval::{self, EvalError, TestOutcome, Value};

fn var(name: &str) -> IRNode {
    IRNode::Var(name.into())
}

fn call(name: &str, args: Vec<IRNode>) -> IRNode {
    IRNode::Call {
        function: name.into(),
        args,
    }
}

fn int(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}

fn lambda(params: &[&str], body: IRNode) -> IRNode {
    IRNode::Lambda {
        params: params
            .iter()
            .map(|p| (p.to_string(), TypeRef::Int))
            .collect(),
        body: Box::new(body),
    }
}

fn spec_fn(source: &str, name: &str) -> morphic::spec::ast::FunctionSpec {
    let module = parser::parse(source).expect("probe source parses");
    module
        .functions
        .into_iter()
        .find(|f| f.name == name)
        .expect("spec present")
}

fn not_evaluable(result: Result<Value, EvalError>) -> String {
    match result {
        Err(EvalError::NotEvaluable(reason)) => reason,
        other => panic!("expected a named NotEvaluable decline, got {other:?}"),
    }
}

const APPLY_TWICE: &str = r#"Probe { spec apply_twice {
    input: f: fn(Int) -> Int, x: Int
    output: Int
    constraint: output == f(f(x))
    test { input: (|n| n + 1, 40); expect: 42 }
    test { input: (|n| n * 2, 3); expect: 12 }
} }"#;

fn apply_twice_body() -> IRNode {
    call("f", vec![call("f", vec![var("x")])])
}

// ── The value itself ──────────────────────────────────────

#[test]
fn phase130_lambda_and_closure_nodes_evaluate_to_function_values() {
    let env = eval::Env::new();
    // A Lambda node is a value now (Phase 127 pinned the
    // "Lambda (function value)" decline; this is its flip).
    let v = eval::eval(&lambda(&["n"], var("n")), &env).expect("lambda evaluates");
    assert!(
        matches!(v, Value::Function { .. }),
        "a Lambda node evaluates to the function variant, got {v:?}"
    );

    // A Closure node resolves its captures by name against
    // the enclosing environment.
    let mut env = eval::Env::new();
    env.insert("x".into(), Value::Int(7));
    let closure = IRNode::Closure {
        params: vec![("n".into(), TypeRef::Int)],
        captures: vec!["x".into()],
        body: Box::new(IRNode::BinOp(
            IRBinOp::Add,
            Box::new(var("n")),
            Box::new(var("x")),
        )),
    };
    let v = eval::eval(&closure, &env).expect("closure evaluates");
    assert!(matches!(v, Value::Function { .. }), "got {v:?}");

    // A capture name the enclosing environment does not bind
    // declines by name — never a guessed capture.
    let unbound = IRNode::Closure {
        params: vec![("n".into(), TypeRef::Int)],
        captures: vec!["y".into()],
        body: Box::new(var("n")),
    };
    let reason = not_evaluable(eval::eval(&unbound, &env));
    assert!(
        reason.contains('y'),
        "the decline names the unbound capture: {reason}"
    );
}

#[test]
fn phase130_spec_lambda_maps_to_a_function_value() {
    // The test-binding route (value_of_expr): a spec-language
    // lambda is a mappable value since this phase — the
    // mapping the Phase 127 audit found returning None /
    // Unbindable ("it carries a value the interpreter cannot
    // represent, e.g. a function/lambda)".
    let expr = Expr::Lambda(
        vec![("n".into(), TypeRef::Int)],
        Box::new(Expr::BinOp(
            SpecBinOp::Add,
            Box::new(Expr::Var("n".into())),
            Box::new(Expr::IntLit(1)),
        )),
    );
    let v = eval::value_of_expr(&expr).expect("a spec lambda maps to a value");
    assert!(matches!(v, Value::Function { .. }), "got {v:?}");

    // And the mapped value APPLIES: f(5) = 6 through the
    // interpreter's call path (the apply_once shape — the
    // Phase 127 "call to unknown function 'f'" decline is
    // gone for a bound parameter).
    let mut env = eval::Env::new();
    env.insert("f".into(), v);
    env.insert("x".into(), Value::Int(5));
    assert_eq!(
        eval::eval(&call("f", vec![var("x")]), &env),
        Ok(Value::Int(6))
    );
}

// ── The frozen corpus standings (§3) ─────────────────────

#[test]
fn phase130_apply_twice_judges_in_process_2_of_2() {
    let spec = spec_fn(APPLY_TWICE, "apply_twice");
    // Binding: both lambda tests bind (Phase 127: 0/2).
    assert_eq!(eval::bindable_test_count(&spec), 2);
    // Judging: the faithful body passes both, in-process.
    // The compiled runner scores this same body 2/2 against
    // these tests (pinned by the Phase 127 probe) — the two
    // judges agree on the shared points.
    let outcomes = eval::test_outcomes(&apply_twice_body(), &spec);
    assert_eq!(outcomes, vec![TestOutcome::Passed, TestOutcome::Passed]);
    let run = eval::run_spec_tests(&apply_twice_body(), &spec);
    assert_eq!((run.evaluable, run.passed), (2, 2));
}

#[test]
fn phase130_apply_once_spec_judges_in_process() {
    // The Phase 127 probe's apply_once: its single lambda
    // test bound nothing and its body declined by name; both
    // halves are this phase's exit standing (§3: "apply_once
    // evaluates in-process").
    let spec = spec_fn(
        r#"Probe { spec apply_once {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: output == x
            test { input: (|n| n, 5); expect: 5 }
        } }"#,
        "apply_once",
    );
    assert_eq!(eval::bindable_test_count(&spec), 1);
    let outcomes = eval::test_outcomes(&call("f", vec![var("x")]), &spec);
    assert_eq!(outcomes, vec![TestOutcome::Passed]);
}

#[test]
fn phase130_require_row_application_decides_spec_silence() {
    // Phase 109's machinery, through an application: the
    // `require: f(x) > 0` row is decidable at a concrete
    // point once f binds and applies, so the violating point
    // is spec-silent and the satisfying point is judged —
    // before this phase the row was undecidable and both
    // points were judged.
    let spec = spec_fn(
        r#"Probe { spec apply_req_points {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            require: f(x) > 0
            constraint: output == x
            test { input: (|n| n, 5); expect: 5 }
            test { input: (|n| n - 5, 3); expect: 3 }
        } }"#,
        "apply_req_points",
    );
    let outcomes = eval::test_outcomes(&var("x"), &spec);
    assert_eq!(outcomes.len(), 2);
    assert_eq!(outcomes[0], TestOutcome::Passed);
    assert!(
        matches!(outcomes[1], TestOutcome::SpecSilent(_)),
        "the point violating f(x) > 0 is spec-silent, got {:?}",
        outcomes[1]
    );
}

// ── Capture semantics (§2.2) ──────────────────────────────

#[test]
fn phase130_capture_is_a_by_value_snapshot_at_creation() {
    // let x = 1 in
    // let g = |n| n + x in
    // let x = 99 in
    // g(10)  ==  11 — g captured x as it stood at creation.
    let prog = IRNode::Let {
        name: "x".into(),
        value: Box::new(int(1)),
        body: Box::new(IRNode::Let {
            name: "g".into(),
            value: Box::new(lambda(
                &["n"],
                IRNode::BinOp(IRBinOp::Add, Box::new(var("n")), Box::new(var("x"))),
            )),
            body: Box::new(IRNode::Let {
                name: "x".into(),
                value: Box::new(int(99)),
                body: Box::new(call("g", vec![int(10)])),
            }),
        }),
    };
    assert_eq!(eval::eval(&prog, &eval::Env::new()), Ok(Value::Int(11)));

    // The Closure node's by-name form: only the named
    // captures enter the closure's environment — a body
    // reaching for an in-scope but UNNAMED binding declines.
    let prog = IRNode::Let {
        name: "x".into(),
        value: Box::new(int(7)),
        body: Box::new(IRNode::Let {
            name: "g".into(),
            value: Box::new(IRNode::Closure {
                params: vec![("n".into(), TypeRef::Int)],
                captures: vec![],
                body: Box::new(IRNode::BinOp(
                    IRBinOp::Add,
                    Box::new(var("n")),
                    Box::new(var("x")),
                )),
            }),
            body: Box::new(call("g", vec![int(1)])),
        }),
    };
    let reason = not_evaluable(eval::eval(&prog, &eval::Env::new()));
    assert!(
        reason.contains("unbound variable `x`"),
        "an unnamed in-scope binding is not captured: {reason}"
    );

    // A parameter shadows a captured binding of the same
    // name at application.
    let prog = IRNode::Let {
        name: "x".into(),
        value: Box::new(int(1)),
        body: Box::new(IRNode::Let {
            name: "g".into(),
            value: Box::new(lambda(&["x"], var("x"))),
            body: Box::new(call("g", vec![int(5)])),
        }),
    };
    assert_eq!(eval::eval(&prog, &eval::Env::new()), Ok(Value::Int(5)));
}

#[test]
fn phase130_let_bound_function_does_not_capture_itself() {
    // let f = |n| f(n) in f(1): the closure's captured
    // environment is the one ENCLOSING the Let — f is not in
    // it (D2a: no self-capture, no fixpoint). The body's call
    // declines with the standing unknown-function reason.
    let prog = IRNode::Let {
        name: "f".into(),
        value: Box::new(lambda(&["n"], call("f", vec![var("n")]))),
        body: Box::new(call("f", vec![int(1)])),
    };
    let reason = not_evaluable(eval::eval(&prog, &eval::Env::new()));
    assert!(
        reason.contains("unknown function `f`"),
        "self-reference declines by the standing outcome: {reason}"
    );
}

// ── The budget (§2.3) ─────────────────────────────────────

#[test]
fn phase130_self_application_bomb_spends_the_budget_and_declines() {
    // The omega shape: let w = |f| f(f) in w(w). Application
    // is not an unbounded recursion channel beside the loop
    // budget. Nested application is host-stack recursion in
    // the interpreter, so the discipline has two halves: the
    // shared step counter (below) and a nesting-depth bound
    // — the bomb hits the depth bound and declines with the
    // standing NotEvaluable (this test completing is the
    // no-hang, no-overflow evidence).
    let omega = IRNode::Let {
        name: "w".into(),
        value: Box::new(lambda(&["f"], call("f", vec![var("f")]))),
        body: Box::new(call("w", vec![var("w")])),
    };
    let reason = not_evaluable(eval::eval(&omega, &eval::Env::new()));
    assert!(
        reason.contains("depth bound exceeded"),
        "the bomb declines by the application depth bound: {reason}"
    );

    // The growing shape: |f, n| f(f, n + 1) applied to
    // (itself, 0) — the argument grows without bound, but
    // the nesting is what is bounded, and it declines the
    // same way.
    let grower = IRNode::Let {
        name: "g".into(),
        value: Box::new(lambda(
            &["f", "n"],
            call(
                "f",
                vec![
                    var("f"),
                    IRNode::BinOp(IRBinOp::Add, Box::new(var("n")), Box::new(int(1))),
                ],
            ),
        )),
        body: Box::new(call("g", vec![var("g"), int(0)])),
    };
    let reason = not_evaluable(eval::eval(&grower, &eval::Env::new()));
    assert!(
        reason.contains("depth bound exceeded"),
        "the growing shape declines by the depth bound: {reason}"
    );

    // The counter half: SEQUENTIAL applications at depth 1
    // — no loop involved, so only application spends count.
    // The interpreter's budget is 10,000 steps; the 10,001st
    // application exhausts it and declines by that name.
    let f = eval::eval(&lambda(&["n"], var("n")), &eval::Env::new()).unwrap();
    let mut env = eval::Env::new();
    env.insert("f".into(), f);
    let block = IRNode::Block(
        (0..10_001).map(|_| call("f", vec![int(1)])).collect(),
    );
    let reason = not_evaluable(eval::eval(&block, &env));
    assert!(
        reason.contains("application bound exceeded"),
        "sequential applications exhaust the shared counter: {reason}"
    );
}

// ── No equality (§2.6) ────────────────────────────────────

#[test]
fn phase130_function_equality_declines_by_name() {
    // Two closures in an environment; `f == g` and `f != g`
    // in evaluated positions decline — decided here, never
    // inherited from a derived structural equality over
    // body AST + captured environment (D5a).
    let mut env = eval::Env::new();
    let f = eval::eval(&lambda(&["n"], var("n")), &env).unwrap();
    let g = eval::eval(
        &lambda(
            &["n"],
            IRNode::BinOp(IRBinOp::Add, Box::new(var("n")), Box::new(int(1))),
        ),
        &env,
    )
    .unwrap();
    env.insert("f".into(), f.clone());
    env.insert("g".into(), g);
    for op in [IRBinOp::Eq, IRBinOp::Neq] {
        let reason = not_evaluable(eval::eval(
            &IRNode::BinOp(op, Box::new(var("f")), Box::new(var("g"))),
            &env,
        ));
        assert!(
            reason.contains("no equality"),
            "{op:?} over function values declines naming the rule: {reason}"
        );
    }
    // Even the SAME closure compared to itself declines —
    // there is no identity shortcut either.
    let reason = not_evaluable(eval::eval(
        &IRNode::BinOp(IRBinOp::Eq, Box::new(var("f")), Box::new(var("f"))),
        &env,
    ));
    assert!(reason.contains("no equality"), "{reason}");
    // And a comparison that merely CONTAINS a function value
    // (a list of closures against itself) declines the same
    // way — the comparison would have to compare one.
    let mut env = eval::Env::new();
    env.insert("xs".into(), Value::List(vec![f]));
    let reason = not_evaluable(eval::eval(
        &IRNode::BinOp(IRBinOp::Eq, Box::new(var("xs")), Box::new(var("xs"))),
        &env,
    ));
    assert!(reason.contains("no equality"), "{reason}");
}

#[test]
fn phase130_same_f_judging_declines_the_comparison_not_the_spec() {
    // same_f (§3): checks (Phase 129), verdict Inconclusive
    // (the verifier — pinned by the Phase 127 probe, unmoved
    // here). On the executed side its tests now BIND (two
    // lambdas), and a body that performs the comparison
    // declines it; a body that simply answers passes — the
    // decline is the comparison's, not the spec's.
    let spec = spec_fn(
        r#"Probe { spec same_f {
            input: f: fn(Int) -> Int, g: fn(Int) -> Int
            output: Bool
            constraint: output == (f == g)
            test { input: (|n| n, |n| n); expect: true }
        } }"#,
        "same_f",
    );
    assert_eq!(eval::bindable_test_count(&spec), 1);
    let comparing = IRNode::BinOp(IRBinOp::Eq, Box::new(var("f")), Box::new(var("g")));
    let outcomes = eval::test_outcomes(&comparing, &spec);
    match &outcomes[0] {
        TestOutcome::BodyNotEvaluable(reason) => {
            assert!(
                reason.contains("no equality"),
                "the comparison declines by name: {reason}"
            );
        }
        other => panic!("expected the comparison to decline, got {other:?}"),
    }
    let answering = IRNode::Literal(IRLiteral::Bool(true));
    assert_eq!(
        eval::test_outcomes(&answering, &spec),
        vec![TestOutcome::Passed]
    );
}

#[test]
fn phase130_a_closure_result_cannot_be_compared_to_an_expected_lambda() {
    // The D5a trap at the judging site: a spec whose OUTPUT
    // is a function. The candidate returns a lambda; the
    // expected output is the syntactically identical lambda.
    // A derived structural equality would call them equal
    // and PASS the test — a fabricated judgement. The
    // comparison declines instead.
    let spec = spec_fn(
        r#"Probe { spec identity_fn {
            input: x: Int
            output: fn(Int) -> Int
            constraint: output == output
            test { input: 0; expect: (|n| n) }
        } }"#,
        "identity_fn",
    );
    assert_eq!(eval::bindable_test_count(&spec), 1);
    let outcomes = eval::test_outcomes(&lambda(&["n"], var("n")), &spec);
    match &outcomes[0] {
        TestOutcome::BodyNotEvaluable(reason) => {
            assert!(
                reason.contains("no equality"),
                "the expected-output comparison declines by name: {reason}"
            );
        }
        other => panic!(
            "expected the comparison to decline (never a syntactic-equality pass), got {other:?}"
        ),
    }
}

// ── Call declines keep their names ────────────────────────

#[test]
fn phase130_call_declines_name_themselves() {
    let mut env = eval::Env::new();
    env.insert("x".into(), Value::Int(5));
    // A bound non-function value cannot be applied.
    let reason = not_evaluable(eval::eval(&call("x", vec![int(1)]), &env));
    assert!(
        reason.contains("non-function"),
        "applying a non-function declines by name: {reason}"
    );
    // An unbound callee keeps the standing decline, verbatim
    // class (the Phase 127 probe pins it for `f`).
    let reason = not_evaluable(eval::eval(&call("missing", vec![]), &env));
    assert!(
        reason.contains("unknown function `missing`"),
        "the standing unknown-function decline is unchanged: {reason}"
    );
    // Arity is checked at application, by name.
    let f = eval::eval(&lambda(&["n"], var("n")), &eval::Env::new()).unwrap();
    let mut env = eval::Env::new();
    env.insert("f".into(), f);
    let reason = not_evaluable(eval::eval(&call("f", vec![int(1), int(2)]), &env));
    assert!(
        reason.contains("argument"),
        "an arity mismatch declines by name: {reason}"
    );
}

// ── The bundled exhibits' in-process standings ────────────

#[test]
fn phase130_bundled_examples_bind_and_their_templates_judge() {
    // sort and binary_search (§3): binding and application
    // landed at Phase 130 (the standings then were 3/6 and
    // 1/7 — swap-bound and loop-progress-bound). Their
    // required end standings (6/6 and 7/7 in-process)
    // landed at Phase 131 (Assign-to-Index, the Let
    // mutation delta, the bare-`None` idiom), and this pin
    // was re-based to them there; tests/
    // phase131_ho_judging.rs carries the per-item
    // evidence. What this test continues to pin: both
    // specs' lambda tests bind in full, and the templates
    // judge at the frozen standings.
    use morphic::synthesis::templates;

    let sort_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sort.morph"
    ))
    .unwrap();
    let sort_spec = spec_fn(&sort_src, "sort");
    assert_eq!(eval::bindable_test_count(&sort_spec), 6);
    let sort_temps = templates::generate_templates(&sort_spec);
    assert!(!sort_temps.is_empty());
    let run = eval::run_spec_tests(&sort_temps[0].1, &sort_spec);
    assert_eq!(
        (run.evaluable, run.passed),
        (6, 6),
        "sort's template at Phase 131: the frozen 6/6 in-process standing"
    );

    let bs_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/binary_search.morph"
    ))
    .unwrap();
    let bs_spec = spec_fn(&bs_src, "binary_search");
    assert_eq!(eval::bindable_test_count(&bs_spec), 7);
    let bs_temps = templates::generate_templates(&bs_spec);
    assert!(!bs_temps.is_empty());
    let run = eval::run_spec_tests(&bs_temps[0].1, &bs_spec);
    assert_eq!(
        (run.evaluable, run.passed),
        (7, 7),
        "binary_search's template at Phase 131: the frozen 7/7 in-process standing"
    );
}

// ── The Phase 131 boundary, re-based ─────────────────────

#[test]
fn phase131_higher_order_nodes_evaluate_and_decline_only_by_their_own_reasons() {
    // At Phase 130 this test pinned the four nodes'
    // blanket declines ("Map (higher-order)" etc.). Phase
    // 131 landed their arms: Map evaluates outright, and
    // the remaining three decline here only for their OWN
    // named reasons — the shared fixture's `inc` is a
    // one-argument Int function, so as a Filter predicate
    // its answer is not Bool, and as a Fold operation or
    // Sort comparator its arity is wrong. The blanket
    // higher-order decline is gone; tests/
    // phase131_ho_judging.rs carries the positive
    // coverage.
    let mut env = eval::Env::new();
    env.insert(
        "xs".into(),
        Value::List(vec![Value::Int(1), Value::Int(2)]),
    );
    let inc = lambda(
        &["n"],
        IRNode::BinOp(IRBinOp::Add, Box::new(var("n")), Box::new(int(1))),
    );
    let map = IRNode::Map {
        function: Box::new(inc.clone()),
        over: Box::new(var("xs")),
    };
    assert_eq!(
        eval::eval(&map, &env),
        Ok(Value::List(vec![Value::Int(2), Value::Int(3)])),
        "Map evaluates since Phase 131"
    );
    let cases: Vec<(IRNode, &str)> = vec![
        (
            IRNode::Filter {
                predicate: Box::new(inc.clone()),
                over: Box::new(var("xs")),
            },
            "not Bool",
        ),
        (
            IRNode::Fold {
                operation: Box::new(inc.clone()),
                initial: Box::new(int(0)),
                over: Box::new(var("xs")),
            },
            "expects 1 argument",
        ),
        (
            IRNode::Sort {
                comparator: Box::new(inc),
                collection: Box::new(var("xs")),
            },
            "expects 1 argument",
        ),
    ];
    for (node, expected) in cases {
        let reason = not_evaluable(eval::eval(&node, &env));
        assert!(
            reason.contains(expected),
            "the node declines by its own named reason: {reason}"
        );
    }
}
