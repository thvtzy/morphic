//! Phase 110 (packaging spine, decision D2a): the prelude
//! merge.
//!
//! The bundled standard library (stdlib/, embedded by
//! src/stdlib.rs) is merged into every user module at the
//! two points all front doors share — the typechecker's
//! signature registration and the SpecIndex constructors —
//! so a user spec calls a library entry by plain name, with
//! no import, identically in the CLI, the API and the
//! playground. These tests pin:
//!
//! * the bundled library parses + checks standalone, and
//!   its one entry this phase (`int_abs`, the spine's
//!   end-to-end proof) carries the frozen contract;
//! * the entry's shipped body — crowned at library-build
//!   time, shipped as data — judges Z3-Verified ∧
//!   golden-exact against its own spec, and is the body the
//!   production pipeline itself crowns for the entry;
//! * a user module calling the entry with no import line
//!   checks, resolves through the index (registry → inline),
//!   and crowns a Verified, fully-passing caller through
//!   the production pipeline and through the API;
//! * the collision rule: a user module's own definition of
//!   an entry's name wins — spec, denotation and semantics;
//! * invisibility when unused: a standing module's check
//!   report, crowned body and diagnostics are unchanged;
//! * `morphic init` generates a real project (no fictional
//!   `morphic-std` dependency, no fake import, and the
//!   generated spec checks).

use morphic::api;
use morphic::spec::ast::FunctionSpec;
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{
    self, CandidateImplementation, IRBinOp, IRLiteral, IRNode, Provenance, ScoreBreakdown,
    SpecIndex,
};
use morphic::synthesis::{eval, runner, selector};
use morphic::verify::verdict::VerificationStatus;
use morphic::verify::verifier::Verifier;

/// The proof module: one user spec calling `int_abs` with
/// no import. Single source of truth — the playground
/// corpus pins the same bytes (playground/corpus/cases.json).
const PROOF: &str = include_str!("../playground/corpus/prelude-int-abs.morph");

/// A module that defines its OWN `int_abs` (the identity —
/// deliberately NOT the library's semantics) and calls it.
const COLLISION: &str = r#"Collision {
spec int_abs {
    input: x: Int
    output: Int
    constraint: output == x
    test { input: (0 - 3); expect: (0 - 3) }
}
spec use_it {
    input: x: Int
    output: Int
    constraint: output == int_abs(x)
    test { input: (0 - 3); expect: (0 - 3) }
    test { input: 5; expect: 5 }
}
}"#;

const VALIDATOR: &str = include_str!("../examples/self_host/validator.morph");

fn checked_functions(source: &str) -> Vec<FunctionSpec> {
    let module = parser::parse(source).expect("test source parses");
    let checked = typeck::check(module).expect("test source typechecks");
    checked.spec.functions
}

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 110,
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

fn int(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}

fn bin(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
    IRNode::BinOp(op, Box::new(l), Box::new(r))
}

/// Body identity, the codebase's own way: the IR Debug
/// rendering (the fingerprint the pipeline itself uses —
/// IRNode carries no PartialEq).
fn dbg(node: &IRNode) -> String {
    format!("{:?}", node)
}

fn dbg_opt(node: &Option<IRNode>) -> Option<String> {
    node.as_ref().map(dbg)
}

/// The pipeline stages exactly as the CLI runs them
/// (parse → typeck → engine with the module in scope →
/// verify → runner stage → select), with the seeded
/// playground profile for determinism — the W3 harness
/// precedent. Returns the crowned winner.
fn crown(functions: &[FunctionSpec], spec: &FunctionSpec, seed: u64) -> CandidateImplementation {
    engine::seed_engine_rng(seed);
    let mut eng = engine::SynthesisEngine::with_module(api::playground_config(), functions);
    let cands = eng.run(spec).expect("engine run");
    let verified = Verifier::with_specs(functions)
        .verify_all(cands, spec)
        .expect("verify_all");
    let pool = runner::rescore_finalists(verified, spec);
    selector::select_best(pool, spec).expect("select_best")
}

// ── The bundled library itself ───────────────────────────

#[test]
fn bundled_library_parses_and_checks_standalone() {
    for (name, source) in stdlib::LIBRARY_MODULES {
        let module = parser::parse(source)
            .unwrap_or_else(|e| panic!("stdlib module '{name}' must parse: {e}"));
        typeck::check(module)
            .unwrap_or_else(|e| panic!("stdlib module '{name}' must check: {e}"));
    }
    // This phase ships exactly one entry: the spine's proof.
    let specs = stdlib::prelude_specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["int_abs"]);
    let abs = &specs[0];
    assert_eq!(abs.params.len(), 1);
    assert_eq!(abs.params[0].ty.to_string(), "Int");
    assert_eq!(abs.return_type.to_string(), "Int");
    // The frozen domain note (STDLIB_SCOPE §2.2): the Int
    // minimum is excluded by precondition.
    assert_eq!(abs.preconditions.len(), 1);
    assert_eq!(abs.tests.len(), 5);
}

#[test]
fn shipped_body_judges_verified_and_golden_exact() {
    let abs = stdlib::prelude_specs().into_iter().next().unwrap();
    let body = stdlib::shipped_body("int_abs").expect("int_abs ships a body");
    // The body crowned at library-build time (Phase 110,
    // seeded pipeline, seed 42), transcribed as data.
    let expected = IRNode::If {
        condition: Box::new(bin(IRBinOp::Lt, var("x"), int(0))),
        then_branch: Box::new(bin(IRBinOp::Sub, int(0), var("x"))),
        else_branch: Box::new(var("x")),
    };
    assert_eq!(dbg(&body), dbg(&expected));

    // Golden-exact on the entry's own tests: 4 evaluable
    // points pass; the Int-minimum point is spec-silent
    // under the precondition (Phase 109), never a failure.
    let run = eval::run_spec_tests(&body, &abs);
    assert_eq!((run.passed, run.evaluable), (4, 4));
    let outcomes = eval::test_outcomes(&body, &abs);
    assert_eq!(outcomes.len(), 5);
    assert!(
        matches!(outcomes[4], eval::TestOutcome::SpecSilent(_)),
        "the Int-minimum point must be spec-silent, got {:?}",
        outcomes[4]
    );

    // Verified against its own contract.
    #[cfg(feature = "z3-support")]
    {
        let verdict = Verifier::new().verify_one(&candidate(&abs, body.clone()), &abs).status;
        assert_eq!(verdict, VerificationStatus::Verified);
    }
}

/// The library-build discipline, pinned: the production
/// pipeline (seeded profile, seed 42) crowns EXACTLY the
/// shipped body for the entry — if the engine's crown ever
/// moves, the library is re-crowned and the data updated,
/// in the open.
#[cfg(feature = "z3-support")]
#[test]
fn pipeline_crowns_the_shipped_body_for_the_entry() {
    let source = stdlib::LIBRARY_MODULES[0].1;
    let report = api::synthesize_seeded_spec(source, "int_abs", 42)
        .expect("pipeline run over the library module");
    let shipped = stdlib::shipped_body("int_abs").unwrap();
    assert_eq!(report.winner.body_debug, format!("{:?}", shipped));
    assert_eq!(report.winner.verification_status, "Verified");
    assert_eq!(
        (report.winner.tests_passed, report.winner.tests_evaluable, report.winner.tests_total),
        (4, 4, 5)
    );
}

// ── The merge: a caller with no import ───────────────────

#[test]
fn user_module_calls_entry_with_no_import() {
    // Checks — before this phase this module failed with
    // "Called undefined function: 'int_abs'".
    let functions = checked_functions(PROOF);
    // The prelude does not leak into the checked module:
    // the user's module is still exactly the user's spec.
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].name, "abs_of_negated");

    // Resolves through the index.
    let index = SpecIndex::from_specs(&functions);
    let entry = index.get("int_abs").expect("prelude entry merged");
    assert_eq!(entry.params.len(), 1);
    assert_eq!(entry.return_type.to_string(), "Int");
    let shipped = stdlib::shipped_body("int_abs").unwrap();
    assert_eq!(dbg_opt(&index.callee_body("int_abs")), Some(dbg(&shipped)));

    // Inlining substitutes the caller's argument into the
    // shipped body — the mechanism every judging path rides.
    let neg_x = bin(IRBinOp::Sub, int(0), var("x"));
    let call = IRNode::Call { function: "int_abs".into(), args: vec![neg_x.clone()] };
    let inlined = index.inline_calls(&call).expect("call inlines");
    let expected = IRNode::If {
        condition: Box::new(bin(IRBinOp::Lt, neg_x.clone(), int(0))),
        then_branch: Box::new(bin(IRBinOp::Sub, int(0), neg_x.clone())),
        else_branch: Box::new(neg_x),
    };
    assert_eq!(dbg(&inlined), dbg(&expected));
}

#[test]
fn production_pipeline_crowns_a_verified_caller() {
    let functions = checked_functions(PROOF);
    let spec = &functions[0];
    let winner = crown(&functions, spec, 42);
    #[cfg(feature = "z3-support")]
    assert_eq!(
        winner.verification_status,
        Some(VerificationStatus::Verified),
        "the caller must crown Verified through the production pipeline"
    );
    // Fully golden-exact under the interpreter — which also
    // proves the crowned body carries no unresolved call
    // (the interpreter declines calls it cannot resolve).
    let run = eval::run_spec_tests(&winner.body, spec);
    assert_eq!((run.passed, run.evaluable), (3, 3));
}

#[test]
fn api_agrees_on_the_prelude_caller() {
    let report = api::synthesize_seeded_spec(PROOF, "abs_of_negated", 42)
        .expect("API run over the proof module");
    // Independent re-run of the same stages → body identity.
    let functions = checked_functions(PROOF);
    let winner = crown(&functions, &functions[0], 42);
    assert_eq!(report.winner.body_debug, format!("{:?}", winner.body));
    let run = eval::run_spec_tests(&winner.body, &functions[0]);
    assert_eq!(
        (report.winner.tests_passed, report.winner.tests_evaluable, report.winner.tests_total),
        (run.passed, run.evaluable, 3)
    );
    assert_eq!((run.passed, run.evaluable), (3, 3));
    // The verdict each configuration can honestly give.
    #[cfg(feature = "z3-support")]
    assert_eq!(report.winner.verification_status, "Verified");
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(report.winner.verification_status, "Inconclusive");
    println!(
        "P110PIN passed={} evaluable={} total={} status={} outcomes={} body={}",
        run.passed,
        run.evaluable,
        3,
        report.winner.verification_status,
        report
            .winner
            .test_outcomes
            .iter()
            .map(|o| o.outcome.as_str())
            .collect::<Vec<_>>()
            .join(","),
        report.winner.body_debug
    );
}

// ── The collision rule ───────────────────────────────────

#[test]
fn collision_user_definition_wins() {
    let functions = checked_functions(COLLISION);
    let index = SpecIndex::from_specs(&functions);
    // The index holds the USER's int_abs (its only test is
    // the identity test), not the library's.
    let entry = index.get("int_abs").expect("user spec present");
    assert_eq!(entry.tests.len(), 1);
    // Its denotation derives from the user's own rows — the
    // shipped body is NOT registered for a colliding name.
    assert_eq!(dbg_opt(&index.callee_body("int_abs")), Some(dbg(&var("x"))));
    // And the semantics are the user's: use_it(-3) is -3,
    // where the library entry would give 3.
    let use_it = &functions[1];
    let winner = crown(&functions, use_it, 42);
    let run = eval::run_spec_tests(&winner.body, use_it);
    assert_eq!((run.passed, run.evaluable), (2, 2));
    #[cfg(feature = "z3-support")]
    assert_eq!(winner.verification_status, Some(VerificationStatus::Verified));
}

// ── Invisibility when unused ─────────────────────────────

#[test]
fn prelude_is_invisible_when_unused() {
    // The check report lists exactly the module's own specs.
    let report = api::check(VALIDATOR);
    assert!(report.ok, "validator must still check: {:?}", report.error);
    let parsed = parser::parse(VALIDATOR).unwrap();
    let own: Vec<&str> = parsed.functions.iter().map(|f| f.name.as_str()).collect();
    let listed: Vec<&str> = report.specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(listed, own);

    // The module's own denotations are untouched by the
    // merge (the prelude is in the index, but it changes
    // nothing the module itself denotes).
    let module = parser::parse(VALIDATOR).unwrap();
    let index = SpecIndex::from_module(&module);
    assert!(index.get("int_abs").is_some(), "prelude merged");
    let expected = bin(
        IRBinOp::And,
        bin(IRBinOp::Ge, var("tag"), int(0)),
        bin(IRBinOp::Le, var("tag"), int(8)),
    );
    assert_eq!(dbg_opt(&index.callee_body("is_known_type_tag")), Some(dbg(&expected)));

    // A seeded crown is byte-identical to the pre-merge pin
    // (measured in Phase 110 before the merge landed).
    let report = api::synthesize_seeded(VALIDATOR, 42).expect("validator run");
    assert_eq!(
        report.winner.body_debug,
        "BinOp(And, BinOp(Ge, Var(\"tag\"), Literal(Int(0))), BinOp(Le, Var(\"tag\"), Literal(Int(8))))"
    );
    assert_eq!(
        (report.winner.tests_passed, report.winner.tests_evaluable, report.winner.tests_total),
        (5, 5, 5)
    );

    // Diagnostics are unchanged: a genuinely undefined
    // function still fails with the standing message.
    let bad = r#"Bad { spec b {
        input: x: Int
        output: Int
        constraint: output == nope_fn(x)
        test { input: 1; expect: 1 }
    } }"#;
    let module = parser::parse(bad).unwrap();
    let err = match typeck::check(module) {
        Ok(_) => panic!("undefined call must fail"),
        Err(e) => e,
    };
    assert!(
        err.message.contains("Called undefined function: 'nope_fn'"),
        "standing diagnostic expected, got: {}",
        err.message
    );
}

// ── morphic init ─────────────────────────────────────────

#[test]
fn init_generates_a_real_project() {
    let dir = std::env::temp_dir().join(format!("morphic-phase110-init-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bin = env!("CARGO_BIN_EXE_morphic");

    let out = std::process::Command::new(bin)
        .arg("init")
        .arg("demo")
        .current_dir(&dir)
        .output()
        .expect("morphic init runs");
    assert!(out.status.success(), "init failed: {}", String::from_utf8_lossy(&out.stderr));

    let toml = std::fs::read_to_string(dir.join("demo/Morphic.toml")).unwrap();
    assert!(!toml.contains("morphic-std"), "the fictional crate must be gone:\n{toml}");
    assert!(!toml.contains("[dependencies]"), "{toml}");

    let spec_src = std::fs::read_to_string(dir.join("demo/main.morph")).unwrap();
    assert!(!spec_src.contains("use morphic"), "{spec_src}");
    assert!(
        !spec_src
            .lines()
            .any(|l| l.trim_start().starts_with("import ") || l.trim_start().starts_with("use ")),
        "no import/use statement may remain:\n{spec_src}"
    );
    // The generated spec is a real module: it parses and
    // checks (its starter spec calls the prelude entry).
    let module = parser::parse(&spec_src).expect("generated spec parses");
    typeck::check(module).expect("generated spec checks");
    // And through the CLI itself.
    let out = std::process::Command::new(bin)
        .arg("check")
        .arg(dir.join("demo/main.morph"))
        .output()
        .expect("morphic check runs");
    assert!(
        out.status.success(),
        "generated project must check via the CLI: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
