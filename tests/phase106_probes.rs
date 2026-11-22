//! Phase 106 (1.0 stdlib audit) probes.
//!
//! These are audit instruments, not production coverage. They pin the
//! distinction the audit depends on:
//!
//! * a name can be present in LANGUAGE_REFERENCE Appendix A and in the
//!   typechecker's coarse return-type table;
//! * while having no semantics in the interpreter, the Z3 encoder, or
//!   the Rust call renderer beyond an ordinary free call.
//!
//! A second probe records that a source-level `import` is parsed into
//! the AST but not resolved by the production check pipeline.

use morphic::spec::ast::FunctionSpec;
use morphic::spec::{parser, typeck};
use morphic::spec::ast::{Expr, Test, TypeRef};
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, IRPattern, IRUnaryOp, Provenance,
    ScoreBreakdown,
};
use morphic::synthesis::eval::{self, Env, Value};
use morphic::verify::verdict::VerificationStatus;
use morphic::verify::verifier::Verifier;

fn checked_spec(source: &str) -> FunctionSpec {
    let module = parser::parse(source).expect("probe source parses");
    let checked = typeck::check(module).expect("probe source typechecks");
    checked
        .spec
        .functions
        .into_iter()
        .next()
        .expect("probe source has one spec")
}

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 106,
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

fn call(name: &str, args: Vec<IRNode>) -> IRNode {
    IRNode::Call {
        function: name.into(),
        args,
    }
}

fn var(name: &str) -> IRNode {
    IRNode::Var(name.into())
}

fn list(values: &[i64]) -> Value {
    Value::List(values.iter().map(|n| Value::Int(*n)).collect())
}

struct DirectCase {
    name: &'static str,
    source: &'static str,
    body: IRNode,
    env: Env,
}

fn direct_cases() -> Vec<DirectCase> {
    vec![
        DirectCase {
            name: "len",
            source: r#"Probe { spec probe_len {
                input: xs: List<Int>
                output: Int
                constraint: output == len(xs)
                test { input: [1, 2, 3]; expect: 3 }
            } }"#,
            body: call("len", vec![var("xs")]),
            env: Env::from([("xs".into(), list(&[1, 2, 3]))]),
        },
        DirectCase {
            name: "int_to_str",
            source: r#"Probe { spec probe_int_to_str {
                input: n: Int
                output: String
                constraint: output == int_to_str(n)
                test { input: (0 - 42); expect: "-42" }
            } }"#,
            body: call("int_to_str", vec![var("n")]),
            env: Env::from([("n".into(), Value::Int(-42))]),
        },
        DirectCase {
            name: "is_sorted",
            source: r#"Probe { spec probe_is_sorted {
                input: xs: List<Int>, cmp: fn(Int, Int) -> Bool
                output: Bool
                constraint: output == is_sorted(xs, cmp)
                test { input: ([1, 2], |a, b| a <= b); expect: true }
            } }"#,
            body: call("is_sorted", vec![var("xs"), var("cmp")]),
            env: Env::from([("xs".into(), list(&[1, 2]))]),
        },
        DirectCase {
            name: "is_permutation",
            source: r#"Probe { spec probe_is_permutation {
                input: xs: List<Int>, ys: List<Int>
                output: Bool
                constraint: output == is_permutation(xs, ys)
                test { input: ([1, 2], [2, 1]); expect: true }
            } }"#,
            body: call("is_permutation", vec![var("xs"), var("ys")]),
            env: Env::from([
                ("xs".into(), list(&[1, 2])),
                ("ys".into(), list(&[2, 1])),
            ]),
        },
        DirectCase {
            name: "contains",
            source: r#"Probe { spec probe_contains {
                input: xs: List<Int>, x: Int
                output: Bool
                constraint: output == contains(xs, x)
                test { input: ([1, 2], 2); expect: true }
            } }"#,
            body: call("contains", vec![var("xs"), var("x")]),
            env: Env::from([("xs".into(), list(&[1, 2])), ("x".into(), Value::Int(2))]),
        },
        DirectCase {
            name: "reverse",
            source: r#"Probe { spec probe_reverse {
                input: xs: List<Int>
                output: List<Int>
                constraint: output == reverse(xs)
                test { input: [1, 2]; expect: [2, 1] }
            } }"#,
            body: call("reverse", vec![var("xs")]),
            env: Env::from([("xs".into(), list(&[1, 2]))]),
        },
        DirectCase {
            name: "sort",
            source: r#"Probe { spec probe_sort {
                input: xs: List<Int>
                output: List<Int>
                constraint: output == sort(xs)
                test { input: [2, 1]; expect: [1, 2] }
            } }"#,
            body: call("sort", vec![var("xs")]),
            env: Env::from([("xs".into(), list(&[2, 1]))]),
        },
        DirectCase {
            name: "filter",
            source: r#"Probe { spec probe_filter {
                input: xs: List<Int>, keep: fn(Int) -> Bool
                output: List<Int>
                constraint: output == filter(xs, keep)
                test { input: ([1, 2, 3], |x| x > 1); expect: [2, 3] }
            } }"#,
            body: call("filter", vec![var("xs"), var("keep")]),
            env: Env::from([("xs".into(), list(&[1, 2, 3]))]),
        },
        DirectCase {
            name: "map",
            source: r#"Probe { spec probe_map {
                input: xs: List<Int>, f: fn(Int) -> Int
                output: List<Int>
                constraint: output == map(xs, f)
                test { input: ([1, 2], |x| x + 1); expect: [2, 3] }
            } }"#,
            body: call("map", vec![var("xs"), var("f")]),
            env: Env::from([("xs".into(), list(&[1, 2]))]),
        },
        DirectCase {
            name: "fold",
            source: r#"Probe { spec probe_fold {
                input: xs: List<Int>, init: Int, f: fn(Int, Int) -> Int
                output: Int
                constraint: output == fold(xs, init, f)
                test { input: ([1, 2], 0, |a, x| a + x); expect: 3 }
            } }"#,
            body: call("fold", vec![var("xs"), var("init"), var("f")]),
            env: Env::from([
                ("xs".into(), list(&[1, 2])),
                ("init".into(), Value::Int(0)),
            ]),
        },
        DirectCase {
            name: "sum",
            source: r#"Probe { spec probe_sum {
                input: xs: List<Int>
                output: Int
                constraint: output == sum(xs)
                test { input: [1, 2, 3]; expect: 6 }
            } }"#,
            body: call("sum", vec![var("xs")]),
            env: Env::from([("xs".into(), list(&[1, 2, 3]))]),
        },
        DirectCase {
            name: "product",
            source: r#"Probe { spec probe_product {
                input: xs: List<Int>
                output: Int
                constraint: output == product(xs)
                test { input: [2, 3, 4]; expect: 24 }
            } }"#,
            body: call("product", vec![var("xs")]),
            env: Env::from([("xs".into(), list(&[2, 3, 4]))]),
        },
        DirectCase {
            name: "min",
            source: r#"Probe { spec probe_min {
                input: xs: List<Int>
                output: Int
                constraint: output == min(xs)
                test { input: [2, 1, 3]; expect: 1 }
            } }"#,
            body: call("min", vec![var("xs")]),
            env: Env::from([("xs".into(), list(&[2, 1, 3]))]),
        },
        DirectCase {
            name: "max",
            source: r#"Probe { spec probe_max {
                input: xs: List<Int>
                output: Int
                constraint: output == max(xs)
                test { input: [2, 1, 3]; expect: 3 }
            } }"#,
            body: call("max", vec![var("xs")]),
            env: Env::from([("xs".into(), list(&[2, 1, 3]))]),
        },
        DirectCase {
            name: "all",
            source: r#"Probe { spec probe_all {
                input: xs: List<Int>, p: fn(Int) -> Bool
                output: Bool
                constraint: output == all(xs, p)
                test { input: ([2, 4], |x| x % 2 == 0); expect: true }
            } }"#,
            body: call("all", vec![var("xs"), var("p")]),
            env: Env::from([("xs".into(), list(&[2, 4]))]),
        },
        DirectCase {
            name: "any",
            source: r#"Probe { spec probe_any {
                input: xs: List<Int>, p: fn(Int) -> Bool
                output: Bool
                constraint: output == any(xs, p)
                test { input: ([1, 4], |x| x % 2 == 0); expect: true }
            } }"#,
            body: call("any", vec![var("xs"), var("p")]),
            env: Env::from([("xs".into(), list(&[1, 4]))]),
        },
        DirectCase {
            name: "range",
            source: r#"Probe { spec probe_range {
                input: n: Int
                output: List<Int>
                constraint: output == range(n)
                test { input: 3; expect: [0, 1, 2] }
            } }"#,
            body: call("range", vec![var("n")]),
            env: Env::from([("n".into(), Value::Int(3))]),
        },
        DirectCase {
            name: "zip",
            source: r#"Probe { spec probe_zip {
                input: xs: List<Int>, ys: List<Int>
                output: List<Int>
                constraint: output == zip(xs, ys)
                test { input: ([1, 2], [3, 4]); expect: [1, 2] }
            } }"#,
            body: call("zip", vec![var("xs"), var("ys")]),
            env: Env::from([
                ("xs".into(), list(&[1, 2])),
                ("ys".into(), list(&[3, 4])),
            ]),
        },
        DirectCase {
            name: "take",
            source: r#"Probe { spec probe_take {
                input: xs: List<Int>, n: Int
                output: List<Int>
                constraint: output == take(xs, n)
                test { input: ([1, 2, 3], 2); expect: [1, 2] }
            } }"#,
            body: call("take", vec![var("xs"), var("n")]),
            env: Env::from([
                ("xs".into(), list(&[1, 2, 3])),
                ("n".into(), Value::Int(2)),
            ]),
        },
        DirectCase {
            name: "drop",
            source: r#"Probe { spec probe_drop {
                input: xs: List<Int>, n: Int
                output: List<Int>
                constraint: output == drop(xs, n)
                test { input: ([1, 2, 3], 1); expect: [2, 3] }
            } }"#,
            body: call("drop", vec![var("xs"), var("n")]),
            env: Env::from([
                ("xs".into(), list(&[1, 2, 3])),
                ("n".into(), Value::Int(1)),
            ]),
        },
    ]
}

#[test]
fn phase106_appendix_a_direct_call_surface() {
    let verifier = Verifier::new();
    let mut implemented = Vec::new();
    let mut typechecked_only = Vec::new();

    for case in direct_cases() {
        let spec = checked_spec(case.source);
        let cand = candidate(&spec, case.body.clone());
        let verdict = verifier.verify_one(&cand, &spec).status;
        let evaluated = eval::eval(&case.body, &case.env);
        let run = eval::run_spec_tests(&case.body, &spec);
        println!(
            "P106|appendix|{}|check=ok|eval={}|tests={}/{}|verify={:?}",
            case.name,
            if evaluated.is_ok() { "ok" } else { "not-evaluable" },
            run.passed,
            run.evaluable,
            verdict,
        );
        // "Implemented" is judged on the Z3-free fact (the call
        // evaluates); the Verified half of the standard is asserted
        // only where the verifier exists (the no-Z3 profile's
        // verify_one is Inconclusive for every body by construction).
        if evaluated.is_ok() {
            implemented.push(case.name);
            #[cfg(feature = "z3-support")]
            assert_eq!(
                verdict,
                VerificationStatus::Verified,
                "{}: an implemented Appendix A call must verify",
                case.name
            );
        } else {
            typechecked_only.push(case.name);
            assert_eq!(
                verdict,
                VerificationStatus::Inconclusive,
                "{}: an unimplemented Appendix A call must not verify",
                case.name
            );
        }
    }

    assert_eq!(implemented, vec!["len", "int_to_str"]);
    assert_eq!(typechecked_only.len(), 18);
}

#[test]
fn phase106_hand_body_boundaries() {
    let verifier = Verifier::new();

    // Int abs: the IR already has an Abs unary node; the concrete
    // interpreter evaluates it and the Z3 encoder encodes it.
    let int_abs = checked_spec(
        r#"Probe { spec probe_int_abs {
            input: x: Int
            output: Int
            constraint: output >= 0
            constraint: output == x || output == (0 - x)
            test { input: (0 - 7); expect: 7 }
            test { input: 4; expect: 4 }
        } }"#,
    );
    let int_abs_body = IRNode::UnaryOp(IRUnaryOp::Abs, Box::new(var("x")));
    let run = eval::run_spec_tests(&int_abs_body, &int_abs);
    let verdict = verifier
        .verify_one(&candidate(&int_abs, int_abs_body.clone()), &int_abs)
        .status;
    println!(
        "P106|hand|int_abs|tests={}/{}|verify={:?}",
        run.passed, run.evaluable, verdict
    );
    assert_eq!((run.passed, run.evaluable), (2, 2));
    #[cfg(feature = "z3-support")]
    assert_eq!(verdict, VerificationStatus::Verified);

    // Int clamp: a faithful hand body passes every executed test
    // but is REFUTED by the verifier — the spec's only protection
    // is `require: lo <= hi`, and `verify_one` judges postconditions
    // over ALL inputs: preconditions are parsed (and shown to the
    // LLM prompt) but never asserted as assumptions in either Z3
    // judge. On hi < lo inputs the postconditions are jointly
    // unsatisfiable, so the faithful body "fails". (The Phase 106
    // pipeline probe crowns only 1/3 Inconclusive for this spec —
    // the search gap and this precondition gap are separate facts.)
    //
    // PHASE 109 UPDATE (§3.1 closed, decision D3a): the verifier
    // now assumes `require:` preconditions in both judges, so
    // this exhibit's verdict flipped — the faithful body VERIFIES
    // and there is no counterexample. The pre-fix measurement
    // (VerificationFailed, counterexample {x:1, lo:1, hi:0})
    // stands in STDLIB_AUDIT.md §3.1 and the Phase 106 ROAD note;
    // the dedicated teeth live in tests/phase109_preconditions.rs.
    let clamp = checked_spec(
        r#"Probe { spec probe_int_clamp {
            input: x: Int, lo: Int, hi: Int
            output: Int
            require: lo <= hi
            constraint: output >= lo
            constraint: output <= hi
            constraint: x < lo implies output == lo
            constraint: x > hi implies output == hi
            constraint: (x >= lo && x <= hi) implies output == x
            test { input: (5, 0, 10); expect: 5 }
            test { input: ((0 - 3), 0, 10); expect: 0 }
            test { input: (99, 0, 10); expect: 10 }
        } }"#,
    );
    let clamp_body = IRNode::If {
        condition: Box::new(IRNode::BinOp(
            IRBinOp::Lt,
            Box::new(var("x")),
            Box::new(var("lo")),
        )),
        then_branch: Box::new(var("lo")),
        else_branch: Box::new(IRNode::If {
            condition: Box::new(IRNode::BinOp(
                IRBinOp::Gt,
                Box::new(var("x")),
                Box::new(var("hi")),
            )),
            then_branch: Box::new(var("hi")),
            else_branch: Box::new(var("x")),
        }),
    };
    let run = eval::run_spec_tests(&clamp_body, &clamp);
    let result = verifier.verify_one(&candidate(&clamp, clamp_body.clone()), &clamp);
    println!(
        "P106|hand|int_clamp|tests={}/{}|verify={:?}|counterexample={:?}",
        run.passed, run.evaluable, result.status, result.counterexample,
    );
    assert_eq!((run.passed, run.evaluable), (3, 3));
    #[cfg(feature = "z3-support")]
    assert_eq!(result.status, VerificationStatus::Verified);

    // Float literals are not merely unverifiable: the source parser
    // has no FloatLit arm, so a decimal literal in a .morph test is a
    // parse failure today. Pin that separately from the body verdict.
    let float_literal_source = r#"Probe { spec probe_float_literal {
        input: x: Float
        output: Float
        constraint: output == x
        test { input: 2.5; expect: 2.5 }
    } }"#;
    let float_parse = parser::parse(float_literal_source);
    println!(
        "P106|hand|float_literal_source|parse={}",
        if float_parse.is_ok() { "ok" } else { "parse-error" }
    );
    assert!(float_parse.is_err(), "decimal literals must fail to parse today");

    // Float abs: the same body is executable, but a Float parameter
    // binds as Opaque in the encoder, so no Float-dependent output
    // can be Verified today. The tests are attached as AST FloatLit
    // values because the source form above cannot carry them.
    let mut float_abs = checked_spec(
        r#"Probe { spec probe_float_abs {
            input: x: Float
            output: Float
            constraint: output == x || output == (0 - x)
        } }"#,
    );
    for (input, expected) in [(-2.5, 2.5), (1.5, 1.5)] {
        float_abs.tests.push(Test {
            name: None,
            input: Expr::FloatLit(input),
            expected_output: Expr::FloatLit(expected),
            timeout_ms: None,
            property: false,
        });
    }
    let float_abs_body = IRNode::UnaryOp(IRUnaryOp::Abs, Box::new(var("x")));
    let run = eval::run_spec_tests(&float_abs_body, &float_abs);
    let verdict = verifier
        .verify_one(&candidate(&float_abs, float_abs_body.clone()), &float_abs)
        .status;
    println!(
        "P106|hand|float_abs|tests={}/{}|verify={:?}",
        run.passed, run.evaluable, verdict
    );
    assert_eq!((run.passed, run.evaluable), (2, 2));
    assert_eq!(verdict, VerificationStatus::Inconclusive);

    // Result unwrap_or: a constructor-Match hand body executes and
    // verifies against the guarded-method spec. (The pipeline probe
    // crowns 1/2 Inconclusive — again a search gap, not a language
    // gap.)
    let unwrap_or = checked_spec(
        r#"Probe { spec probe_result_unwrap_or {
            input: r: Result<Int, String>, fallback: Int
            output: Int
            constraint: r.is_err() || output == r.unwrap()
            constraint: r.is_ok() || output == fallback
            test { input: (Ok(7), 3); expect: 7 }
            test { input: (Err("bad"), 3); expect: 3 }
        } }"#,
    );
    let unwrap_or_body = IRNode::Match {
        scrutinee: Box::new(var("r")),
        cases: vec![
            (
                IRPattern::Constructor("Ok".into(), vec![IRPattern::Variable("v".into())]),
                var("v"),
            ),
            (
                IRPattern::Constructor("Err".into(), vec![IRPattern::Wildcard]),
                var("fallback"),
            ),
        ],
    };
    let run = eval::run_spec_tests(&unwrap_or_body, &unwrap_or);
    let verdict = verifier
        .verify_one(&candidate(&unwrap_or, unwrap_or_body.clone()), &unwrap_or)
        .status;
    println!(
        "P106|hand|result_unwrap_or|tests={}/{}|verify={:?}",
        run.passed, run.evaluable, verdict
    );
    assert_eq!((run.passed, run.evaluable), (2, 2));
    #[cfg(feature = "z3-support")]
    assert_eq!(verdict, VerificationStatus::Verified);

    // List reverse: the IR has a Reverse unary node and codegen has a
    // rendering, but both judges decline it.
    let reverse = checked_spec(
        r#"Probe { spec probe_list_reverse {
            input: xs: List<Int>
            output: List<Int>
            constraint: len(output) == len(xs)
            constraint: forall i: Int in range(len(xs)): output[i] == xs[(len(xs) - 1) - i]
            test { input: [1, 2, 3]; expect: [3, 2, 1] }
            test { input: []; expect: [] }
        } }"#,
    );
    let reverse_body = IRNode::UnaryOp(IRUnaryOp::Reverse, Box::new(var("xs")));
    let outcomes = eval::test_outcomes(&reverse_body, &reverse);
    let verdict = verifier
        .verify_one(&candidate(&reverse, reverse_body.clone()), &reverse)
        .status;
    println!(
        "P106|hand|list_reverse|outcomes={:?}|verify={:?}",
        outcomes, verdict
    );
    assert!(matches!(
        outcomes[0],
        eval::TestOutcome::BodyNotEvaluable(_)
    ));
    assert_eq!(verdict, VerificationStatus::Inconclusive);

    // Higher-order map: the IR has a Map node and the Rust emitter
    // renders it, but the interpreter and Z3 encoder both decline
    // the node, independently of G4's test-binding wall.
    let map_inc = checked_spec(
        r#"Probe { spec probe_map_inc {
            input: xs: List<Int>
            output: List<Int>
            constraint: len(output) == len(xs)
            constraint: forall i: Int in range(len(xs)): output[i] == xs[i] + 1
            test { input: [1, 2, 3]; expect: [2, 3, 4] }
            test { input: []; expect: [] }
        } }"#,
    );
    let map_body = IRNode::Map {
        function: Box::new(IRNode::Lambda {
            params: vec![("x".into(), TypeRef::Int)],
            body: Box::new(IRNode::BinOp(
                IRBinOp::Add,
                Box::new(var("x")),
                Box::new(IRNode::Literal(IRLiteral::Int(1))),
            )),
        }),
        over: Box::new(var("xs")),
    };
    let outcomes = eval::test_outcomes(&map_body, &map_inc);
    let verdict = verifier
        .verify_one(&candidate(&map_inc, map_body.clone()), &map_inc)
        .status;
    let rust = morphic::api::render(&candidate(&map_inc, map_body.clone()), "rust")
        .expect("Map node renders to Rust");
    println!(
        "P106|hand|map_inc|outcomes={:?}|verify={:?}|rust_contains_map={}",
        outcomes,
        verdict,
        rust.contains(".iter().map(")
    );
    assert!(matches!(
        outcomes[0],
        eval::TestOutcome::BodyNotEvaluable(_)
    ));
    assert_eq!(verdict, VerificationStatus::Inconclusive);
    assert!(rust.contains(".iter().map("));
}

#[test]
fn phase106_option_has_no_eliminator_in_spec_expressions() {
    // Result has the D1a method table; Option has no methods at
    // all, and spec expressions have no match form — so a spec can
    // discriminate None vs Some only by equality and can never
    // project a symbolic Some payload. Both spellings are type
    // errors naming the closed table.
    for spelling in ["x.unwrap()", "x.is_some()"] {
        let source = format!(
            r#"Probe {{ spec probe_option {{
                input: x: Option<Int>
                output: Int
                constraint: output == {spelling}
                test {{ input: Some(1); expect: 1 }}
            }} }}"#
        );
        let parsed = parser::parse(&source).expect("option probe parses");
        let err = match typeck::check(parsed) {
            Ok(_) => panic!("Option method must be rejected"),
            Err(e) => e,
        };
        println!("P106|option|{spelling}|typecheck=error|{}", err.message);
        assert!(
            err.message.contains("is_ok") || err.message.contains("Unknown method"),
            "error must name the closed method table: {}",
            err.message
        );
    }
}

#[test]
fn phase106_import_is_parsed_but_not_resolved_by_check() {
    // The imported file deliberately does not exist. If imports were
    // resolved, check would fail before it could report this module.
    let source = r#"Probe {
        import "std/collections.morph";
        spec local_identity {
            input: x: Int
            output: Int
            constraint: output == x
            test { input: 7; expect: 7 }
        }
    }"#;
    let parsed = parser::parse(source).expect("import syntax parses");
    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].path, "std/collections.morph");

    let report = morphic::api::check(source);
    println!(
        "P106|import|parsed=1|check_ok={}|specs={}",
        report.ok,
        report.specs.len()
    );
    assert!(report.ok, "check ignores the unresolved import: {:?}", report.error);
    assert_eq!(report.specs.len(), 1);
}
