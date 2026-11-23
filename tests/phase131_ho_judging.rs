//! Phase 131 (G4) — higher-order judging in-process
//! (G4_SCOPE §3): the Map/Filter/Fold/Sort IR arms evaluate,
//! Assign reaches indexed places, a Let body's Assigns to
//! pre-existing bindings take effect (the compiled
//! semantics), and the bare-constructor `Var("None")` idiom
//! resolves — the phase that owns the frozen standings
//! **sort 6/6 and binary_search 7/7 in-process**.
//!
//! What this file pins, at the interpreter's own surface:
//!
//! * Map/Filter/Fold/Sort evaluate by applying their
//!   function argument through the Phase 130 closure
//!   machinery (budget + depth discipline inherited); a
//!   non-function function-position, a non-List collection,
//!   a non-Bool Filter predicate or Sort comparator, and
//!   an arity mismatch each decline BY NAME — never a
//!   guessed value;
//! * Assign to an indexed place (`xs[i] = v`, nested
//!   included) mutates the bound list — sort's swap is the
//!   exhibit; an unbound base, a non-list base, and an
//!   out-of-range or negative index decline by name;
//! * the Let change's exact semantic delta: a body
//!   evaluates in the current environment, so Assigns (and
//!   statement Appends) to bindings that PRE-DATE the Let
//!   persist — a loop whose progress step sits inside a
//!   Let body progresses — while BINDING stays scoped
//!   exactly as the old clone scoped it: the Let's own
//!   binding is restored on exit and names created inside
//!   the body (Allocs) do not leak;
//! * `Var("None")` evaluates to the None value (the
//!   compiled route renders the bare name as the variant);
//!   an environment binding of the same name wins, and the
//!   unary constructors keep their Call form — they have
//!   no bare-Var resolution;
//! * the two bundled exhibits at their frozen standings,
//!   through `run_spec_tests` on the template bodies.

use morphic::spec::parser;
use morphic::spec::ast::{FunctionSpec, TypeRef};
use morphic::synthesis::engine::{IRBinOp, IRLiteral, IRNode};
#[cfg(feature = "compiled-runner")]
use morphic::synthesis::engine::{CandidateImplementation, Provenance, ScoreBreakdown};
use morphic::synthesis::eval::{self, EvalError, TestOutcome, Value};
use morphic::synthesis::templates;

fn var(name: &str) -> IRNode {
    IRNode::Var(name.into())
}

fn int(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}

fn list(items: &[i64]) -> IRNode {
    IRNode::Collection {
        kind: morphic::synthesis::engine::CollectionKind::List,
        elements: items.iter().map(|n| int(*n)).collect(),
    }
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

fn binop(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
    IRNode::BinOp(op, Box::new(l), Box::new(r))
}

fn alloc(name: &str, initial: IRNode) -> IRNode {
    IRNode::Alloc {
        name: name.into(),
        ty: TypeRef::Int,
        initial: Box::new(initial),
    }
}

fn assign_var(name: &str, value: IRNode) -> IRNode {
    IRNode::Assign {
        target: Box::new(var(name)),
        value: Box::new(value),
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

fn ints(v: &Value) -> Vec<i64> {
    match v {
        Value::List(items) => items
            .iter()
            .map(|i| match i {
                Value::Int(n) => *n,
                other => panic!("expected Int element, got {other:?}"),
            })
            .collect(),
        other => panic!("expected a List value, got {other:?}"),
    }
}

// ── The higher-order node arms ────────────────────────────

#[test]
fn phase131_map_filter_fold_sort_nodes_evaluate() {
    let mut env = eval::Env::new();
    env.insert(
        "xs".into(),
        Value::List(vec![
            Value::Int(3),
            Value::Int(1),
            Value::Int(4),
            Value::Int(2),
        ]),
    );

    // Map: the function applies per element, in order.
    let map = IRNode::Map {
        function: Box::new(lambda(
            &["n"],
            binop(IRBinOp::Mul, var("n"), int(10)),
        )),
        over: Box::new(var("xs")),
    };
    assert_eq!(ints(&eval::eval(&map, &env).unwrap()), vec![30, 10, 40, 20]);

    // Map's function position may also be a bound function
    // value named by a Var (the parameter-application
    // shape: a spec's fn-typed parameter threaded into a
    // combinator).
    let mut env2 = env.clone();
    let closure = eval::eval(
        &lambda(&["n"], binop(IRBinOp::Add, var("n"), int(1))),
        &env2,
    )
    .unwrap();
    env2.insert("f".into(), closure);
    let map_var = IRNode::Map {
        function: Box::new(var("f")),
        over: Box::new(var("xs")),
    };
    assert_eq!(ints(&eval::eval(&map_var, &env2).unwrap()), vec![4, 2, 5, 3]);

    // Filter: keeps exactly the elements the predicate
    // answers true for, in order.
    let filter = IRNode::Filter {
        predicate: Box::new(lambda(
            &["n"],
            binop(IRBinOp::Ge, var("n"), int(3)),
        )),
        over: Box::new(var("xs")),
    };
    assert_eq!(ints(&eval::eval(&filter, &env).unwrap()), vec![3, 4]);

    // Fold: a left fold in element order — ((0+3)+1)+4+2.
    let fold = IRNode::Fold {
        operation: Box::new(lambda(
            &["acc", "n"],
            binop(IRBinOp::Add, var("acc"), var("n")),
        )),
        initial: Box::new(int(0)),
        over: Box::new(var("xs")),
    };
    assert_eq!(eval::eval(&fold, &env), Ok(Value::Int(10)));
    // Fold order is observable with a non-commutative op:
    // ((10-3)-1)-4-2 = 0 only as a left fold from 10.
    let fold_sub = IRNode::Fold {
        operation: Box::new(lambda(
            &["acc", "n"],
            binop(IRBinOp::Sub, var("acc"), var("n")),
        )),
        initial: Box::new(int(10)),
        over: Box::new(list(&[3, 1, 4, 2])),
    };
    assert_eq!(eval::eval(&fold_sub, &env), Ok(Value::Int(0)));

    // Sort: the comparator's `true` means "first argument
    // precedes" — ascending with `a < b`, descending with
    // `a > b`; an empty and a singleton collection sort to
    // themselves.
    let sort = IRNode::Sort {
        comparator: Box::new(lambda(
            &["a", "b"],
            binop(IRBinOp::Lt, var("a"), var("b")),
        )),
        collection: Box::new(var("xs")),
    };
    assert_eq!(ints(&eval::eval(&sort, &env).unwrap()), vec![1, 2, 3, 4]);
    let sort_desc = IRNode::Sort {
        comparator: Box::new(lambda(
            &["a", "b"],
            binop(IRBinOp::Gt, var("a"), var("b")),
        )),
        collection: Box::new(var("xs")),
    };
    assert_eq!(
        ints(&eval::eval(&sort_desc, &env).unwrap()),
        vec![4, 3, 2, 1]
    );
    let sort_empty = IRNode::Sort {
        comparator: Box::new(lambda(
            &["a", "b"],
            binop(IRBinOp::Lt, var("a"), var("b")),
        )),
        collection: Box::new(list(&[])),
    };
    assert_eq!(ints(&eval::eval(&sort_empty, &env).unwrap()), Vec::<i64>::new());
}

#[test]
fn phase131_higher_order_nodes_decline_by_name() {
    let mut env = eval::Env::new();
    env.insert(
        "xs".into(),
        Value::List(vec![Value::Int(1), Value::Int(2)]),
    );
    env.insert("n".into(), Value::Int(5));

    // The function position holds a non-function value.
    let reason = not_evaluable(eval::eval(
        &IRNode::Map {
            function: Box::new(var("n")),
            over: Box::new(var("xs")),
        },
        &env,
    ));
    assert!(reason.contains("Map") && reason.contains("not a function"), "{reason}");

    // The collection is not a List.
    let reason = not_evaluable(eval::eval(
        &IRNode::Map {
            function: Box::new(lambda(&["x"], var("x"))),
            over: Box::new(var("n")),
        },
        &env,
    ));
    assert!(reason.contains("Map") && reason.contains("not a List"), "{reason}");

    // A Filter predicate answering a non-Bool declines —
    // truthiness is never guessed.
    let reason = not_evaluable(eval::eval(
        &IRNode::Filter {
            predicate: Box::new(lambda(&["x"], var("x"))),
            over: Box::new(var("xs")),
        },
        &env,
    ));
    assert!(
        reason.contains("Filter") && reason.contains("not Bool"),
        "{reason}"
    );

    // A Sort comparator answering a non-Bool declines.
    let reason = not_evaluable(eval::eval(
        &IRNode::Sort {
            comparator: Box::new(lambda(&["a", "b"], var("a"))),
            collection: Box::new(var("xs")),
        },
        &env,
    ));
    assert!(
        reason.contains("Sort") && reason.contains("not Bool"),
        "{reason}"
    );

    // An arity mismatch in the operation declines through
    // the application machinery's own named failure.
    let reason = not_evaluable(eval::eval(
        &IRNode::Fold {
            operation: Box::new(lambda(&["acc"], var("acc"))),
            initial: Box::new(int(0)),
            over: Box::new(var("xs")),
        },
        &env,
    ));
    assert!(reason.contains("expects 1 argument"), "{reason}");
}

// ── Assign to an indexed place ────────────────────────────

#[test]
fn phase131_assign_to_index_mutates_the_bound_list() {
    // The sort swap's shape, in miniature: an indexed
    // assignment inside a block mutates the Alloc'd list,
    // and the mutated list is what later reads see.
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "xs".into(),
            ty: TypeRef::List(Box::new(TypeRef::Int)),
            initial: Box::new(list(&[1, 2, 3])),
        },
        IRNode::Assign {
            target: Box::new(IRNode::Index(Box::new(var("xs")), Box::new(int(0)))),
            value: Box::new(int(9)),
        },
        var("xs"),
    ]);
    assert_eq!(
        ints(&eval::eval(&body, &eval::Env::new()).unwrap()),
        vec![9, 2, 3]
    );

    // The full swap, through a Let-bound temp — the
    // template's exact construction: the temp Let's body
    // performs both indexed Assigns, and they take effect
    // on the list bound outside the Let.
    let swap = IRNode::Block(vec![
        IRNode::Alloc {
            name: "a".into(),
            ty: TypeRef::List(Box::new(TypeRef::Int)),
            initial: Box::new(list(&[2, 1])),
        },
        IRNode::Let {
            name: "tmp".into(),
            value: Box::new(IRNode::Index(Box::new(var("a")), Box::new(int(0)))),
            body: Box::new(IRNode::Block(vec![
                IRNode::Assign {
                    target: Box::new(IRNode::Index(
                        Box::new(var("a")),
                        Box::new(int(0)),
                    )),
                    value: Box::new(IRNode::Index(
                        Box::new(var("a")),
                        Box::new(int(1)),
                    )),
                },
                IRNode::Assign {
                    target: Box::new(IRNode::Index(
                        Box::new(var("a")),
                        Box::new(int(1)),
                    )),
                    value: Box::new(var("tmp")),
                },
            ])),
        },
        var("a"),
    ]);
    assert_eq!(
        ints(&eval::eval(&swap, &eval::Env::new()).unwrap()),
        vec![1, 2]
    );

    // Nested places: xs[1][0] = 7 reaches the inner list.
    let nested = IRNode::Block(vec![
        IRNode::Alloc {
            name: "grid".into(),
            ty: TypeRef::List(Box::new(TypeRef::List(Box::new(TypeRef::Int)))),
            initial: Box::new(IRNode::Collection {
                kind: morphic::synthesis::engine::CollectionKind::List,
                elements: vec![list(&[1]), list(&[2])],
            }),
        },
        IRNode::Assign {
            target: Box::new(IRNode::Index(
                Box::new(IRNode::Index(Box::new(var("grid")), Box::new(int(1)))),
                Box::new(int(0)),
            )),
            value: Box::new(int(7)),
        },
        IRNode::Index(
            Box::new(IRNode::Index(Box::new(var("grid")), Box::new(int(1)))),
            Box::new(int(0)),
        ),
    ]);
    assert_eq!(eval::eval(&nested, &eval::Env::new()), Ok(Value::Int(7)));
}

#[test]
fn phase131_assign_to_index_declines_by_name() {
    // Unbound base variable.
    let reason = not_evaluable(eval::eval(
        &IRNode::Assign {
            target: Box::new(IRNode::Index(Box::new(var("ghost")), Box::new(int(0)))),
            value: Box::new(int(1)),
        },
        &eval::Env::new(),
    ));
    assert!(reason.contains("ghost") && reason.contains("not bound"), "{reason}");

    // Base bound to a non-list.
    let mut env = eval::Env::new();
    env.insert("n".into(), Value::Int(3));
    let reason = not_evaluable(eval::eval(
        &IRNode::Assign {
            target: Box::new(IRNode::Index(Box::new(var("n")), Box::new(int(0)))),
            value: Box::new(int(1)),
        },
        &env,
    ));
    assert!(reason.contains("non-list"), "{reason}");

    // Out-of-bounds and negative indices: the read side's
    // named failures, not a panic and not a silent grow.
    let mut env = eval::Env::new();
    env.insert("xs".into(), Value::List(vec![Value::Int(1)]));
    let reason = not_evaluable(eval::eval(
        &IRNode::Assign {
            target: Box::new(IRNode::Index(Box::new(var("xs")), Box::new(int(5)))),
            value: Box::new(int(1)),
        },
        &env,
    ));
    assert!(reason.contains("out of bounds"), "{reason}");
    let reason = not_evaluable(eval::eval(
        &IRNode::Assign {
            target: Box::new(IRNode::Index(Box::new(var("xs")), Box::new(int(-1)))),
            value: Box::new(int(1)),
        },
        &env,
    ));
    assert!(reason.contains("negative"), "{reason}");

    // A target that is neither a variable nor an indexed
    // place keeps a named decline.
    let reason = not_evaluable(eval::eval(
        &IRNode::Assign {
            target: Box::new(binop(IRBinOp::Add, var("xs"), int(1))),
            value: Box::new(int(1)),
        },
        &env,
    ));
    assert!(reason.contains("Assign target"), "{reason}");
}

// ── The Let delta: mutation persists, binding stays scoped ─

#[test]
fn phase131_let_body_assigns_to_outer_bindings_persist() {
    // The minimal loop-progress probe (Phase 130 isolated
    // this exact shape): the While's body is a Let whose
    // body Assigns the loop variable. Before this phase
    // the Let evaluated its body in a cloned environment,
    // the Assign was discarded, the condition never moved,
    // and the evaluation declined on the budget. The
    // compiled route progresses; now the interpreter does.
    let body = IRNode::Block(vec![
        alloc("n", int(0)),
        IRNode::While {
            condition: Box::new(binop(IRBinOp::Lt, var("n"), int(3))),
            invariant: None,
            body: Box::new(IRNode::Let {
                name: "step".into(),
                value: Box::new(binop(IRBinOp::Add, var("n"), int(1))),
                body: Box::new(assign_var("n", var("step"))),
            }),
        },
        var("n"),
    ]);
    assert_eq!(eval::eval(&body, &eval::Env::new()), Ok(Value::Int(3)));

    // A statement Append inside a Let body mutates the
    // outer list the same way (the BinOp Append statement
    // form is the other env-mutating shape).
    let appends = IRNode::Block(vec![
        IRNode::Alloc {
            name: "xs".into(),
            ty: TypeRef::List(Box::new(TypeRef::Int)),
            initial: Box::new(list(&[])),
        },
        IRNode::Let {
            name: "t".into(),
            value: Box::new(int(7)),
            body: Box::new(binop(IRBinOp::Append, var("xs"), var("t"))),
        },
        var("xs"),
    ]);
    assert_eq!(
        ints(&eval::eval(&appends, &eval::Env::new()).unwrap()),
        vec![7]
    );
}

#[test]
fn phase131_let_binding_stays_scoped_while_mutation_persists() {
    // The Let's own binding shadows inside the body and is
    // restored on exit — the clone's binding discipline,
    // kept. (The compiled form scopes a Let's binding to
    // its body the same way whenever the Let is not the
    // enclosing block's tail.)
    let mut env = eval::Env::new();
    env.insert("x".into(), Value::Int(1));
    let scoped = IRNode::Block(vec![
        IRNode::Let {
            name: "x".into(),
            value: Box::new(int(99)),
            body: Box::new(var("x")),
        },
        var("x"),
    ]);
    assert_eq!(eval::eval(&scoped, &env), Ok(Value::Int(1)));
    let inside = IRNode::Let {
        name: "x".into(),
        value: Box::new(int(99)),
        body: Box::new(var("x")),
    };
    assert_eq!(eval::eval(&inside, &env), Ok(Value::Int(99)));

    // Assigning the Let's OWN name inside its body is the
    // body's business (the body reads the new value); it
    // does not leak past the Let either.
    let rebind = IRNode::Block(vec![
        IRNode::Let {
            name: "x".into(),
            value: Box::new(int(10)),
            body: Box::new(IRNode::Block(vec![
                assign_var("x", int(20)),
                var("x"),
            ])),
        },
        var("x"),
    ]);
    assert_eq!(eval::eval(&rebind, &env), Ok(Value::Int(1)));

    // A name created inside the body (an Alloc) does not
    // leak: reading it after the Let declines as unbound,
    // exactly as it did under the clone.
    let leak = IRNode::Block(vec![
        IRNode::Let {
            name: "t".into(),
            value: Box::new(int(1)),
            body: Box::new(alloc("u", int(5))),
        },
        var("u"),
    ]);
    let reason = not_evaluable(eval::eval(&leak, &eval::Env::new()));
    assert!(reason.contains("unbound variable `u`"), "{reason}");

    // Cleanup runs on the control-flow exits too: a Let
    // whose body Returns, sitting in an Alloc initializer
    // (the composition position whose Returned flow is
    // absorbed as the value), leaves no binding behind.
    let absorbed = IRNode::Block(vec![
        IRNode::Alloc {
            name: "y".into(),
            ty: TypeRef::Int,
            initial: Box::new(IRNode::Block(vec![IRNode::Let {
                name: "t".into(),
                value: Box::new(int(7)),
                body: Box::new(IRNode::Return(Box::new(var("t")))),
            }])),
        },
        var("t"),
    ]);
    let reason = not_evaluable(eval::eval(&absorbed, &eval::Env::new()));
    assert!(reason.contains("unbound variable `t`"), "{reason}");
}

// ── The bare-constructor idiom ────────────────────────────

#[test]
fn phase131_bare_none_var_is_the_constructor_value() {
    // The binary_search template's final return, in
    // miniature: a bare Var("None") is the Option variant,
    // as the compiled route renders it.
    assert_eq!(
        eval::eval(&var("None"), &eval::Env::new()),
        Ok(Value::None)
    );
    let ret = IRNode::Return(Box::new(var("None")));
    assert_eq!(eval::eval(&ret, &eval::Env::new()), Ok(Value::None));

    // An environment binding of the same name wins — the
    // idiom is the fallback for an UNBOUND name, never an
    // override of a binding.
    let mut env = eval::Env::new();
    env.insert("None".into(), Value::Int(4));
    assert_eq!(eval::eval(&var("None"), &env), Ok(Value::Int(4)));

    // The unary constructors have no bare form: they are
    // Calls (eval_call's arms), and a bare Var naming one
    // still declines as unbound.
    let reason = not_evaluable(eval::eval(&var("Some"), &eval::Env::new()));
    assert!(reason.contains("unbound variable `Some`"), "{reason}");
}

// ── The frozen exhibits ───────────────────────────────────

#[test]
fn phase131_sort_template_judges_in_process_6_of_6() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sort.morph"
    ))
    .unwrap();
    let spec = spec_fn(&src, "sort");
    let temps = templates::generate_templates(&spec);
    assert!(!temps.is_empty());
    let outcomes = eval::test_outcomes(&temps[0].1, &spec);
    assert!(
        outcomes.iter().all(|o| matches!(o, TestOutcome::Passed)),
        "every sort test passes in-process: {outcomes:?}"
    );
    let run = eval::run_spec_tests(&temps[0].1, &spec);
    assert_eq!((run.evaluable, run.passed), (6, 6));
}

#[test]
fn phase131_binary_search_template_judges_in_process_7_of_7() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/binary_search.morph"
    ))
    .unwrap();
    let spec = spec_fn(&src, "binary_search");
    let temps = templates::generate_templates(&spec);
    assert!(!temps.is_empty());
    let started = std::time::Instant::now();
    let outcomes = eval::test_outcomes(&temps[0].1, &spec);
    assert!(
        outcomes.iter().all(|o| matches!(o, TestOutcome::Passed)),
        "every binary_search test passes in-process: {outcomes:?}"
    );
    let run = eval::run_spec_tests(&temps[0].1, &spec);
    assert_eq!((run.evaluable, run.passed), (7, 7));
    // The loops progress now: judging the template is a
    // millisecond affair, where the pre-131 standings had
    // five of seven tests burn the full shared budget each
    // (the pipeline cost this phase collapses is measured
    // in the Phase 127 crown probe).
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "the template's judging must not burn the budget: {:?}",
        started.elapsed()
    );
}

// ── §3's compiled standing, re-measured ──────────────────

/// The apply spec of the Phase 127 pair-1 exhibit, verbatim
/// in shape: a function-typed parameter applied twice, its
/// constraint deliberately not stating the application (the
/// Phase 127 form), two lambda tests.
const APPLY_RENDER: &str = r#"Probe { spec apply_render {
    input: f: fn(Int) -> Int, x: Int
    output: Int
    constraint: output == x
    test { input: (|n| n + 1, 40); expect: 42 }
    test { input: (|n| n * 2, 3); expect: 12 }
} }"#;

#[cfg(feature = "compiled-runner")]
fn apply_candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 131,
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

#[test]
fn phase131_apply_render_compiled_standing_remeasured() {
    // G4_SCOPE §3 froze apply_render's compiled standing —
    // Scored 2/2 at the audit — and assigned its
    // re-measurement to this phase. The G1 stage itself can
    // no longer be driven against this spec: its trigger was
    // the tests' unbindability, and fabricating that premise
    // back is not a measurement. What §3 protects is the
    // compiled ROUTE's verdict on the body, so it is measured
    // the way the campaign measures compiled verdicts when
    // the stage's premise does not apply (the Phase 53
    // vehicle, as Phase 59 uses it for is_sorted): codegen
    // the body, compile the generated function with rustc,
    // and run the spec's own two tests against it from a
    // hand-written main. In-process the body stands 2/2 in
    // every configuration (asserted in both arms below).
    let spec = spec_fn(APPLY_RENDER, "apply_render");
    let body = IRNode::Call {
        function: "f".into(),
        args: vec![IRNode::Call {
            function: "f".into(),
            args: vec![var("x")],
        }],
    };
    let in_process = eval::run_spec_tests(&body, &spec);
    assert_eq!(
        (in_process.evaluable, in_process.passed),
        (2, 2),
        "apply body in-process: the Phase 130 standing"
    );

    #[cfg(feature = "compiled-runner")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        fn run_timed(cmd: &mut Command, timeout: Duration) -> (bool, String) {
            let mut child = match cmd
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => return (false, String::new()),
            };
            let start = Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        let out = child.wait_with_output().expect("output readable");
                        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                        text.push_str(&String::from_utf8_lossy(&out.stderr));
                        return (out.status.success(), text);
                    }
                    Ok(None) => {
                        if start.elapsed() > timeout {
                            let _ = child.kill();
                            let _ = child.wait();
                            return (false, String::new());
                        }
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => return (false, String::new()),
                }
            }
        }

        let cand = apply_candidate(&spec, body);
        let generated = morphic::codegen::generate(&cand, "rust")
            .expect("the apply body generates Rust");
        let function_part = generated
            .split_once("#[cfg(test)]")
            .map(|(head, _)| head)
            .unwrap_or(&generated)
            .to_string();
        assert!(
            function_part
                .contains("pub fn apply_render(f: fn(i64) -> i64, x: i64) -> i64"),
            "the generated signature is the Phase 127 pin:\n{function_part}"
        );
        let main_rs = r#"
fn check(name: &str, got: std::thread::Result<i64>, want: i64, results: &mut Vec<bool>) {
    let ok = matches!(&got, Ok(v) if *v == want);
    println!("{}: {}", name, if ok { "PASS" } else { "FAIL" });
    results.push(ok);
}
fn main() {
    let mut r: Vec<bool> = Vec::new();
    check("inc-twice", std::panic::catch_unwind(|| apply_render(|n: i64| n + 1, 40)), 42, &mut r);
    check("dbl-twice", std::panic::catch_unwind(|| apply_render(|n: i64| n * 2, 3)), 12, &mut r);
    let passed = r.iter().filter(|b| **b).count();
    println!("TOTAL {}/2", passed);
}
"#;
        let dir = std::env::temp_dir().join(format!(
            "morphic-phase131-apply-render-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir creatable");
        let src = dir.join("judge.rs");
        let bin = dir.join("judge");
        {
            let mut f = std::fs::File::create(&src).expect("source writable");
            f.write_all(function_part.as_bytes()).expect("write fn");
            f.write_all(main_rs.as_bytes()).expect("write main");
        }
        let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
        let (compiled, cout) = run_timed(
            Command::new(&rustc)
                .arg("--edition")
                .arg("2021")
                .arg(&src)
                .arg("-o")
                .arg(&bin),
            Duration::from_secs(120),
        );
        assert!(compiled, "the generated apply function compiles:\n{cout}");
        let (ran, rout) = run_timed(&mut Command::new(&bin), Duration::from_secs(30));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(ran, "the compiled apply function runs:\n{rout}");
        assert!(
            rout.contains("TOTAL 2/2"),
            "§3's compiled standing, re-measured at Phase 131: Scored 2/2, \
             unregressed:\n{rout}"
        );
    }
}
