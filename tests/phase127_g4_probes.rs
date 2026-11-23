//! Phase 127 (G4 scope audit) probes.
//!
//! These are audit instruments, not production coverage. They pin
//! the standing of function values layer by layer, as measured on
//! the audit tree (master @ 4881c1d, v1.1.0):
//!
//! * the spec language *declares* function types (parser +
//!   typechecker + LANGUAGE_REFERENCE §4/§11) — what happens when
//!   a spec actually *uses* one is measured per layer below;
//! * the interpreter's `Value` has no function variant, so a
//!   lambda test input cannot bind, a Lambda/Closure/Map/Filter/
//!   Fold/Sort body declines by name, and a call through a
//!   function-typed parameter is "call to unknown function";
//! * the compiled runner — the judging route of last resort —
//!   renders the same bodies and test lambdas to Rust and CAN
//!   score them: the split between "G4 proper" (no function
//!   values anywhere) and "judging context" (which judge holds
//!   the body) is measured, not asserted.
//!
//! Every probe prints a `P127|...` line with the measured
//! standing; the assertions pin exactly what the line reports.

use morphic::spec::ast::{Expr, FunctionSpec, TypeRef};
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, Provenance, ScoreBreakdown,
};
use morphic::synthesis::eval::{self, Value};
use morphic::synthesis::{runner, templates};
use morphic::verify::verdict::VerificationStatus;
use morphic::verify::verifier::{self, Verifier};

fn try_checked(source: &str) -> Result<Vec<FunctionSpec>, String> {
    let module = parser::parse(source).map_err(|e| format!("parse: {e}"))?;
    let checked = typeck::check(module).map_err(|e| format!("check: {}", e.message))?;
    Ok(checked.spec.functions)
}

fn checked_spec(source: &str) -> FunctionSpec {
    try_checked(source)
        .expect("probe source typechecks")
        .into_iter()
        .next()
        .expect("probe source has one spec")
}

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 127,
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

/// The minimal higher-order spec: a function-typed parameter,
/// applied twice in the constraint, supplied by lambda tests.
const APPLY_TWICE: &str = r#"Probe { spec apply_twice {
    input: f: fn(Int) -> Int, x: Int
    output: Int
    constraint: output == f(f(x))
    test { input: (|n| n + 1, 40); expect: 42 }
    test { input: (|n| n * 2, 3); expect: 12 }
} }"#;

/// The faithful body for APPLY_TWICE: apply the parameter twice.
fn apply_twice_body() -> IRNode {
    call("f", vec![call("f", vec![var("x")])])
}

#[test]
fn phase127_apply_spec_end_to_end() {
    // Parse: the documented form parses — fn-typed parameter,
    // lambda test inputs and all.
    let module = parser::parse(APPLY_TWICE).expect("fn-typed parameter parses");
    let spec = module.functions.into_iter().next().unwrap();
    assert!(matches!(spec.params[0].ty, TypeRef::Function(..)));
    println!("P127|apply|parse=Ok");

    // Check: THE WALL IS HERE, one layer earlier than the
    // stdlib audit recorded. A plain postcondition that APPLIES
    // its function parameter is a type error — check_expr's
    // Call arm resolves callees against module sigs and the
    // stdlib fiction table, never against the parameter env,
    // so the spec language cannot even STATE `output == f(x)`.
    let checked = try_checked(APPLY_TWICE);
    match &checked {
        Ok(_) => println!("P127|apply|check=Ok"),
        Err(e) => println!("P127|apply|check=Err({e})"),
    }
    let err = checked.expect_err("applying a fn parameter in a postcondition is a type error today");
    assert!(err.contains("Called undefined function: 'f'"), "{err}");
    println!("P127|apply|check=Err(Called undefined function: 'f')");

    // The remaining layers are probed on the PARSED spec (the
    // binding and judging layers do not consult the checker):
    //
    // Test binding: the lambda inputs cannot become values.
    let bindable = eval::bindable_test_count(&spec);
    println!(
        "P127|apply|bindable={}/{}",
        bindable,
        spec.tests.len()
    );
    assert_eq!(bindable, 0, "lambda test inputs bind nothing today");

    // Executed judging never reaches the body: the tests are
    // Unbindable before any candidate is evaluated.
    let outcomes = eval::test_outcomes(&apply_twice_body(), &spec);
    println!("P127|apply|outcomes={outcomes:?}");
    assert!(matches!(
        outcomes[0],
        eval::TestOutcome::Unbindable(_)
    ));

    // Z3: the apply body is outside the encodable subset, and
    // the verdict over the (unchecked) spec is Inconclusive.
    let encodable = verifier::body_is_encodable(&spec, &apply_twice_body());
    let verdict = Verifier::new()
        .verify_one(&candidate(&spec, apply_twice_body()), &spec)
        .status;
    println!("P127|apply|encodable={encodable}|verify={verdict:?}");
    assert!(!encodable);
    assert_eq!(verdict, VerificationStatus::Inconclusive);
}

#[test]
fn phase127_where_application_is_admitted() {
    // The check wall is shape-specific. Measured per shape:
    // (a) plain postcondition application — type error (above).
    // (b) application inside a FORALL row — sort's row 2 shape.
    let forall_src = r#"Probe { spec apply_fa {
        input: f: fn(Int) -> Int, x: Int
        output: Int
        constraint: forall i: Int in range(1): output == f(f(x))
        test { input: (|n| n + 1, 40); expect: 42 }
    } }"#;
    let checked = try_checked(forall_src);
    match &checked {
        Ok(_) => println!("P127|admit|forall_row=Ok"),
        Err(e) => println!("P127|admit|forall_row=Err({e})"),
    }
    // (c) application inside a REQUIRE row.
    let require_src = r#"Probe { spec apply_req {
        input: f: fn(Int) -> Int, x: Int
        output: Int
        require: f(x) > 0
        constraint: output == x
        test { input: (|n| n, 5); expect: 5 }
    } }"#;
    let checked = try_checked(require_src);
    match &checked {
        Ok(_) => println!("P127|admit|require_row=Ok"),
        Err(e) => println!("P127|admit|require_row=Err({e})"),
    }
    // (d) the parameter passed as an ARGUMENT (Var position) —
    // sort's `is_sorted(list, cmp)` shape, via the fiction
    // table's own name:
    let arg_src = r#"Probe { spec pass_f {
        input: f: fn(Int, Int) -> Bool, xs: List<Int>
        output: Bool
        constraint: output == is_sorted(xs, f)
        test { input: (|a, b| a < b, [1, 2]); expect: true }
    } }"#;
    let checked = try_checked(arg_src);
    match &checked {
        Ok(_) => println!("P127|admit|as_argument=Ok"),
        Err(e) => println!("P127|admit|as_argument=Err({e})"),
    }
    assert!(checked.is_ok(), "passing a fn parameter as an argument checks");

    // (e) application as a BARE CONSTRAINT ATOM — the shape
    // sort's row 2 actually survives by. The parser lowers a
    // whole-row call to Constraint::Predicate, and
    // check_constraint's Predicate arm never resolves the
    // name ("assumed to be provided by the synthesis
    // backend"). The row checks — vacuously: §3.10 makes the
    // same form unencodable, so it constrains nothing.
    let atom_src = r#"Probe { spec apply_atom {
        input: cmp: fn(Int, Int) -> Bool, a: Int, b: Int
        output: Bool
        constraint: cmp(a, b)
        constraint: output == true
        test { input: (|x, y| x < y, 1, 2); expect: true }
    } }"#;
    let module = parser::parse(atom_src).expect("atom spec parses");
    let has_predicate = module.functions[0]
        .postconditions
        .iter()
        .any(|c| matches!(c, morphic::spec::ast::Constraint::Predicate(name, _) if name == "cmp"));
    println!("P127|admit|bare_atom_is_predicate={has_predicate}");
    assert!(has_predicate, "a bare call row lowers to Constraint::Predicate");
    let checked = try_checked(atom_src);
    match &checked {
        Ok(_) => println!("P127|admit|bare_atom=Ok"),
        Err(e) => println!("P127|admit|bare_atom=Err({e})"),
    }
    assert!(checked.is_ok(), "the Predicate path never resolves the callee name");

    // (f) sort's exact row-2 shape: forall + implies + the
    // comparator applied as the implies' bare-atom side.
    let sort_row_src = r#"Probe { spec sort_row {
        input: cmp: fn(Int, Int) -> Bool, xs: List<Int>
        output: Bool
        constraint: forall i: Int, j: Int in range(len(xs)): i < j implies cmp(xs[i], xs[j])
        constraint: output == true
        test { input: (|x, y| x < y, [1, 2]); expect: true }
    } }"#;
    let checked = try_checked(sort_row_src);
    match &checked {
        Ok(_) => println!("P127|admit|sort_row_shape=Ok"),
        Err(e) => println!("P127|admit|sort_row_shape=Err({e})"),
    }
    assert!(checked.is_ok(), "sort's row-2 shape checks — through the Predicate side door");
}

#[test]
fn phase127_apply_body_declines_by_name_in_eval() {
    // A bindable spec (no lambda tests) whose body applies its
    // fn-typed parameter: separates the APPLICATION decline
    // from the test-BINDING decline of the probe above.
    let spec = checked_spec(
        r#"Probe { spec apply_once {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: output == x
            test { input: (|n| n, 5); expect: 5 }
        } }"#,
    );
    // Even this spec's only test carries a lambda, so use a
    // body-level measurement instead: evaluate the apply body
    // in an env where every non-function input is bound.
    let mut env = eval::Env::new();
    env.insert("x".into(), Value::Int(5));
    let result = eval::eval(&call("f", vec![var("x")]), &env);
    println!("P127|apply_body|eval={result:?}");
    match result {
        Err(eval::EvalError::NotEvaluable(reason)) => {
            assert!(
                reason.contains("unknown function `f`"),
                "the decline must name the cause: {reason}"
            );
        }
        other => panic!("expected the named NotEvaluable decline, got {other:?}"),
    }

    // And the Lambda/Closure nodes themselves, as bodies:
    for (label, body) in [
        (
            "lambda",
            IRNode::Lambda {
                params: vec![("n".into(), TypeRef::Int)],
                body: Box::new(var("n")),
            },
        ),
        (
            "closure",
            IRNode::Closure {
                params: vec![("n".into(), TypeRef::Int)],
                captures: vec!["x".into()],
                body: Box::new(var("n")),
            },
        ),
    ] {
        let result = eval::eval(&body, &env);
        println!("P127|apply_body|{label}={result:?}");
        assert!(
            matches!(result, Err(eval::EvalError::NotEvaluable(_))),
            "{label} must decline as a function value"
        );
    }
    let _ = spec;
}

#[test]
fn phase127_fn_param_does_not_poison_unrelated_verification() {
    // The fn-typed parameter binds Opaque in the encoder; a
    // body and constraint that never touch it still verify.
    let spec = checked_spec(
        r#"Probe { spec ignore_f {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: output == x
            test { input: (|n| n + 1, 5); expect: 5 }
        } }"#,
    );
    let verdict = Verifier::new()
        .verify_one(&candidate(&spec, var("x")), &spec)
        .status;
    println!("P127|ignore_f|verify={verdict:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(verdict, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(verdict, VerificationStatus::Inconclusive);
}

#[test]
fn phase127_fn_equality_has_no_home() {
    // Two fn-typed parameters, compared in a constraint: what
    // does each layer say?
    let src = r#"Probe { spec same_f {
        input: f: fn(Int) -> Int, g: fn(Int) -> Int
        output: Bool
        constraint: output == (f == g)
        test { input: (|n| n, |n| n); expect: true }
    } }"#;
    let checked = try_checked(src);
    match &checked {
        Ok(_) => println!("P127|fn_eq|check=Ok"),
        Err(e) => println!("P127|fn_eq|check=Err({e})"),
    }
    if let Ok(specs) = checked {
        let spec = &specs[0];
        let verdict = Verifier::new()
            .verify_one(
                &candidate(spec, IRNode::Literal(IRLiteral::Bool(true))),
                spec,
            )
            .status;
        println!("P127|fn_eq|verify={verdict:?}");
        assert_eq!(verdict, VerificationStatus::Inconclusive);
    }
}

#[test]
fn phase127_named_function_reference_binds_nothing() {
    // The typechecker resolves a bare sibling name to a
    // Function type (typeck's Var arm reads function_sigs) —
    // but there is no VALUE for the name to bind to in a test.
    let src = r#"Probe {
        spec double_it {
            input: x: Int
            output: Int
            constraint: output == 2 * x
            test { input: 21; expect: 42 }
        }
        spec apply_named {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: output == f(x)
            test { input: (double_it, 21); expect: 42 }
        }
    }"#;
    let checked = try_checked(src);
    match &checked {
        Ok(_) => println!("P127|named_ref|check=Ok"),
        Err(e) => println!("P127|named_ref|check=Err({e})"),
    }
    // The value mapping of a bare function name, directly:
    let value = eval::value_of_expr(&Expr::Var("double_it".into()));
    println!("P127|named_ref|value_of_var={value:?}");
    assert!(value.is_none(), "a named function has no Value today");
    if let Ok(specs) = checked {
        let apply_named = &specs[1];
        let bindable = eval::bindable_test_count(apply_named);
        println!("P127|named_ref|bindable={}/{}", bindable, apply_named.tests.len());
        assert_eq!(bindable, 0);
    }
}

#[test]
fn phase127_bundled_examples_standings() {
    for name in ["sort", "binary_search"] {
        let path = format!("{}/examples/{name}.morph", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&path).expect("bundled example readable");
        let module = parser::parse(&source).expect("bundled example parses");
        let n_specs = module.functions.len();
        let checked = typeck::check(module);
        match &checked {
            Ok(_) => println!("P127|example|{name}|specs={n_specs}|check=Ok"),
            Err(e) => println!("P127|example|{name}|specs={n_specs}|check=Err({})", e.message),
        }
        if let Ok(checked) = checked {
            let spec = &checked.spec.functions[0];
            let bindable = eval::bindable_test_count(spec);
            println!(
                "P127|example|{name}|bindable={}/{}",
                bindable,
                spec.tests.len()
            );
            assert_eq!(bindable, 0, "{name}'s lambda tests bind nothing today");
            #[cfg(feature = "compiled-runner")]
            assert_eq!(
                runner::trigger(&[candidate(spec, var("list"))], spec),
                Some(runner::Trigger::UnbindableTests),
                "{name} sits in the runner's UnbindableTests territory"
            );
        }
    }

    // The sort template body against the sort spec: the
    // template calls the comparator parameter directly.
    let path = format!("{}/examples/sort.morph", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&path).unwrap();
    let spec = checked_spec(&source);
    let temps = templates::generate_templates(&spec);
    println!(
        "P127|example|sort|templates={:?}",
        temps.iter().map(|(k, _)| k.name()).collect::<Vec<_>>()
    );
    assert!(!temps.is_empty());
    let (_, body) = &temps[0];
    let run = eval::run_spec_tests(body, &spec);
    let verdict = Verifier::new()
        .verify_one(&candidate(&spec, body.clone()), &spec)
        .status;
    println!(
        "P127|example|sort|template_run=evaluable:{} passed:{}|verify={verdict:?}",
        run.evaluable, run.passed
    );
    assert_eq!(run.evaluable, 0);

    // binary_search's template through the compiled runner
    // directly: does the compiled judge rescue it, as it did
    // sort's? (The pipeline probe below crowns this body at
    // tpr 0 — this measurement says which judge-side fact
    // that 0 records.)
    let path = format!("{}/examples/binary_search.morph", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&path).unwrap();
    let bs_spec = checked_spec(&source);
    let bs_temps = templates::generate_templates(&bs_spec);
    println!(
        "P127|example|binary_search|templates={:?}",
        bs_temps.iter().map(|(k, _)| k.name()).collect::<Vec<_>>()
    );
    if let Some((_, bs_body)) = bs_temps.first() {
        let bs_cand = candidate(&bs_spec, bs_body.clone());
        let (rescored, outcome) = runner::rescore_finalists_report(vec![bs_cand], &bs_spec);
        println!(
            "P127|example|binary_search|runner={outcome:?}|tpr={}",
            rescored[0].scores.test_pass_ratio
        );
        // In the compiled-runner configuration: judged — and
        // failed at compilation (the G5 wall: the template's
        // i64 list indices do not compile in the harness's
        // generated Rust), so the runner's 0/7 is a compilation
        // verdict, not a binding verdict. Without the runner
        // the ratio stands at its in-process 0. Either way the
        // pin is 0.0.
        assert_eq!(rescored[0].scores.test_pass_ratio, 0.0);
    }
}

#[test]
fn phase127_compiled_runner_judges_what_the_interpreter_cannot() {
    // The decisive judging-context measurement, in two pairs.
    //
    // Pair 1: a body that APPLIES its fn-typed parameter,
    // against lambda-carrying tests. (The spec's constraint
    // is beside the point for this instrument — the runner
    // executes tests only, per runner.rs policy; what is
    // measured is purely whether the compiled judge can score
    // an application body against lambda tests at all.)
    let spec = checked_spec(
        r#"Probe { spec apply_render {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: output == x
            test { input: (|n| n + 1, 40); expect: 42 }
            test { input: (|n| n * 2, 3); expect: 12 }
        } }"#,
    );
    // The tests above expect f(f(x))'s answers, so the apply
    // body is the faithful one for the TESTS (the constraint
    // cannot state it — the check wall, probe 1).
    let cand = candidate(&spec, apply_twice_body());
    let in_process = eval::run_spec_tests(&cand.body, &spec);
    println!(
        "P127|judges|apply_in_process=evaluable:{} passed:{}",
        in_process.evaluable, in_process.passed
    );
    assert_eq!(in_process.evaluable, 0);
    let (rescored, outcome) = runner::rescore_finalists_report(vec![cand], &spec);
    let tpr = rescored[0].scores.test_pass_ratio;
    println!("P127|judges|apply_runner={outcome:?}|compiled_tpr={tpr}");
    #[cfg(feature = "compiled-runner")]
    assert_eq!(tpr, 1.0, "the compiled judge scores the apply body 2/2");
    #[cfg(not(feature = "compiled-runner"))]
    assert_eq!(tpr, 0.0, "no compiled runner in this configuration");

    // Pair 2: the production case — sort + its SortPrimitive
    // template, which calls the comparator parameter directly.
    let path = format!("{}/examples/sort.morph", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&path).unwrap();
    let sort_spec = checked_spec(&source);
    let temps = templates::generate_templates(&sort_spec);
    let template_cand = candidate(&sort_spec, temps[0].1.clone());
    let in_process = eval::run_spec_tests(&template_cand.body, &sort_spec);
    println!(
        "P127|judges|sort_in_process=evaluable:{} passed:{}",
        in_process.evaluable, in_process.passed
    );
    let (rescored, outcome) = runner::rescore_finalists_report(vec![template_cand], &sort_spec);
    let tpr = rescored[0].scores.test_pass_ratio;
    println!("P127|judges|sort_runner={outcome:?}|compiled_tpr={tpr}");
    #[cfg(feature = "compiled-runner")]
    assert_eq!(tpr, 1.0, "the compiled judge scores sort's template 6/6 today");
    #[cfg(not(feature = "compiled-runner"))]
    assert_eq!(tpr, 0.0, "no compiled runner in this configuration");
}

#[test]
fn phase127_pipeline_crowns_for_the_bundled_ho_specs() {
    // The production pipeline (seeded profile, seed 42) on the
    // two bundled specs whose tests carry lambdas: what does
    // the crown look like TODAY, with the runner in the loop?
    let mut tprs = Vec::new();
    for name in ["sort", "binary_search"] {
        let path = format!("{}/examples/{name}.morph", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&path).unwrap();
        let started = std::time::Instant::now();
        let report = morphic::api::synthesize_seeded(&source, 42)
            .unwrap_or_else(|e| panic!("{name} pipeline runs: {} {}", e.stage, e.message));
        let w = &report.winner;
        println!(
            "P127|pipeline|{name}|candidates={}|winner_tpr={}|winner_tests={}/{}|verify={:?}|elapsed={:?}",
            report.candidates,
            w.test_pass_ratio,
            w.tests_passed,
            w.tests_evaluable,
            w.verification_status,
            started.elapsed()
        );
        println!("P127|pipeline|{name}|winner_body={}", w.body_debug);
        tprs.push(w.test_pass_ratio);
    }
    // The standings the prints record, pinned: with the
    // compiled runner, sort's crown is its template at tpr 1
    // (rescued by the runner — a real 6/6) and binary_search's
    // crown is its template at tpr 0 (the runner judged it
    // and its compiled form failed — G5). WITHOUT the runner
    // both crowns read 1.0 — vacuously: the engine's scoring
    // fallback assigns 1.0 when a spec has tests but zero are
    // evaluable ("tests assumed passed"), and nothing in that
    // configuration replaces the fallback with a measurement.
    // The same number, two different epistemic standings —
    // which is precisely the judging hole G4 closes.
    #[cfg(feature = "compiled-runner")]
    assert_eq!(tprs, vec![1.0, 0.0]);
    #[cfg(not(feature = "compiled-runner"))]
    assert_eq!(tprs, vec![1.0, 1.0]);
}

#[test]
fn phase127_emitter_standings_per_target() {
    // A CHECKED spec carrying the fn-typed parameter (its
    // constraint never applies f — the check wall), with a
    // candidate body that applies the parameter twice.
    // Candidate bodies are IR and are not typechecked; the
    // emitters render spec + body, which is exactly the
    // production shape (a synthesized winner under a spec).
    let spec = checked_spec(
        r#"Probe { spec apply_render {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: output == x
            test { input: (|n| n, 5); expect: 5 }
        } }"#,
    );
    let cand = candidate(&spec, apply_twice_body());
    for target in ["rust", "javascript", "c", "python", "wasm"] {
        match morphic::api::render(&cand, target) {
            Ok(text) => {
                let sig = text
                    .lines()
                    .find(|l| l.contains("apply_render"))
                    .unwrap_or("")
                    .trim()
                    .to_string();
                println!("P127|emit|{target}|Ok|{sig}");
            }
            Err(e) => println!("P127|emit|{target}|Err|{e}"),
        }
    }
    // Full texts for the targets whose one-line signature
    // hides the body standing:
    for target in ["javascript", "c", "python"] {
        let text = morphic::api::render(&cand, target).expect("renders");
        println!("P127|emit_full|{target}=\n{text}");
    }
    // The measured body standings, pinned: only Rust renders
    // the application. JS accepts the parameter (untyped) and
    // returns undefined for the body; C types the parameter
    // `void*` and comments the body out; Python passes.
    let js = morphic::api::render(&cand, "javascript").unwrap();
    assert!(js.contains("return undefined"), "{js}");
    let c = morphic::api::render(&cand, "c").unwrap();
    assert!(c.contains("void* f") && c.contains("/* unsupported */"), "{c}");
    let py = morphic::api::render(&cand, "python").unwrap();
    assert!(py.contains("pass  # unsupported"), "{py}");
    let js_map = morphic::api::render(
        &candidate(
            &checked_spec(
                r#"Probe { spec probe_map_inc2 {
                    input: xs: List<Int>
                    output: List<Int>
                    constraint: len(output) == len(xs)
                    test { input: [1, 2, 3]; expect: [2, 3, 4] }
                } }"#,
            ),
            IRNode::Map {
                function: Box::new(IRNode::Lambda {
                    params: vec![("x".into(), TypeRef::Int)],
                    body: Box::new(IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(var("x")),
                        Box::new(int(1)),
                    )),
                }),
                over: Box::new(var("xs")),
            },
        ),
        "javascript",
    )
    .unwrap();
    assert!(
        js_map.contains("return undefined"),
        "the Map node renders undefined in JS today: {js_map}"
    );
    // The Rust route, in full: the generated signature carries
    // the fn-pointer type, and the body applies it — the one
    // emitter that already believes in function values.
    let rust = morphic::api::render(&cand, "rust").expect("rust renders");
    println!("P127|emit|rust_full=\n{rust}");
    assert!(rust.contains("fn(i64) -> i64"), "{rust}");
    assert!(rust.contains("f(f(x))"), "{rust}");

    // A Map{Lambda} body across targets (the Phase 106 exhibit,
    // extended past Rust):
    let map_spec = checked_spec(
        r#"Probe { spec probe_map_inc {
            input: xs: List<Int>
            output: List<Int>
            constraint: len(output) == len(xs)
            test { input: [1, 2, 3]; expect: [2, 3, 4] }
        } }"#,
    );
    let map_body = IRNode::Map {
        function: Box::new(IRNode::Lambda {
            params: vec![("x".into(), TypeRef::Int)],
            body: Box::new(IRNode::BinOp(
                IRBinOp::Add,
                Box::new(var("x")),
                Box::new(int(1)),
            )),
        }),
        over: Box::new(var("xs")),
    };
    let map_cand = candidate(&map_spec, map_body);
    for target in ["rust", "javascript", "c", "python", "wasm"] {
        match morphic::api::render(&map_cand, target) {
            Ok(text) => {
                let first = text.lines().take(3).collect::<Vec<_>>().join(" / ");
                println!("P127|emit_map|{target}|Ok|{first}");
            }
            Err(e) => println!("P127|emit_map|{target}|Err|{e}"),
        }
    }
}

#[test]
fn phase127_range_is_not_a_g4_item() {
    // `range` was removed at the 1.0 freeze with NO §2 analysis
    // (STDLIB_SCOPE §2.7) — it is a first-order entry. A hand
    // body in the Phase 95/115 vocabulary (Alloc + For +
    // Assign/Concat) is judged in-process TODAY; only its
    // element-level contract rows wait on §3.9, like every
    // Tier-2 List entry.
    let spec = checked_spec(
        r#"Probe { spec range_upto {
            input: n: Int
            output: List<Int>
            constraint: len(output) == n
            test { input: 3; expect: [0, 1, 2] }
            test { input: 0; expect: [] }
        } }"#,
    );
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "acc".into(),
            ty: TypeRef::List(Box::new(TypeRef::Int)),
            initial: Box::new(IRNode::Collection {
                kind: morphic::synthesis::engine::CollectionKind::List,
                elements: vec![],
            }),
        },
        IRNode::For {
            var: "i".into(),
            start: Box::new(int(0)),
            end: Box::new(var("n")),
            body: Box::new(IRNode::Assign {
                target: Box::new(var("acc")),
                value: Box::new(IRNode::BinOp(
                    IRBinOp::Concat,
                    Box::new(var("acc")),
                    Box::new(IRNode::Collection {
                        kind: morphic::synthesis::engine::CollectionKind::List,
                        elements: vec![var("i")],
                    }),
                )),
            }),
        },
        IRNode::Return(Box::new(var("acc"))),
    ]);
    let run = eval::run_spec_tests(&body, &spec);
    let verdict = Verifier::new()
        .verify_one(&candidate(&spec, body.clone()), &spec)
        .status;
    println!(
        "P127|range|run={}/{}|verify={verdict:?}",
        run.passed, run.evaluable
    );
    assert_eq!((run.passed, run.evaluable), (2, 2));
    assert_eq!(verdict, VerificationStatus::Inconclusive);
}

#[test]
fn phase127_zip_wall_is_tuples_not_functions() {
    // zip's removal reason (STDLIB_SCOPE §2.7): tuple elements
    // are unmodelled at every layer. The wall is on the
    // EXPECTED side: the inputs bind, the output cannot be a
    // Value — there is no tuple Value. No lambda is involved.
    let spec = checked_spec(
        r#"Probe { spec zip_probe {
            input: xs: List<Int>, ys: List<Int>
            output: List<Int>
            constraint: len(output) == len(xs)
            test { input: ([1, 2], [3, 4]); expect: [(1, 3), (2, 4)] }
        } }"#,
    );
    let test = &spec.tests[0];
    let expected = eval::value_of_expr(&test.expected_output);
    println!("P127|zip|value_of_expected={expected:?}");
    assert!(expected.is_none(), "a list of tuples has no Value");
    let bindable = eval::bindable_test_count(&spec);
    println!("P127|zip|bindable={}/{}", bindable, spec.tests.len());
    assert_eq!(bindable, 0);
    // The tuple itself, as a value:
    let tuple = eval::value_of_expr(&Expr::Call(
        "__tuple__".into(),
        vec![Expr::IntLit(1), Expr::IntLit(3)],
    ));
    println!("P127|zip|value_of_tuple={tuple:?}");
    assert!(tuple.is_none(), "there is no tuple Value");
}
