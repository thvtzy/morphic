//! Phase 109 (§3.1 closure, decision D3a): the verifier
//! assumes `require:` preconditions.
//!
//! Before this phase, `verify_one` and the statement judge
//! encoded postconditions over ALL inputs; `require:` rows
//! reached only the LLM prompt. The audit's exhibit
//! (STDLIB_AUDIT.md §3.1): a faithful `int_clamp` body passes
//! 3/3 executed tests yet is **VerificationFailed** on
//! `{x:1, lo:1, hi:0}` — a point its own precondition
//! excludes. These tests pin the closed behaviour:
//!
//! * the faithful clamp body Verifies;
//! * a body wrong at a precondition-SATISFYING point still
//!   Refutes, with an on-domain countermodel;
//! * jointly unsatisfiable preconditions mint nothing —
//!   every candidate degrades to Inconclusive;
//! * an unencodable precondition declines the whole
//!   verification (never silently assumed, never dropped
//!   while judging proceeds);
//! * the statement judge (Phase 93/96 routes, including
//!   the invariant obligations and the Phase 97
//!   per-disjunct fallback) judges under the same
//!   assumptions;
//! * executed judging treats a precondition-violating
//!   test point as spec-silent: reported, never counted
//!   as a body failure.

use morphic::spec::ast::FunctionSpec;
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, IRUnaryOp, Provenance,
    ScoreBreakdown,
};
use morphic::synthesis::eval::{self, TestOutcome};
use morphic::verify::verdict::{SmtValue, VerificationStatus};
use morphic::verify::verifier::Verifier;

fn checked_spec(source: &str) -> FunctionSpec {
    let module = parser::parse(source).expect("test source parses");
    let checked = typeck::check(module).expect("test source typechecks");
    checked
        .spec
        .functions
        .into_iter()
        .next()
        .expect("test source has one spec")
}

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 109,
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

// ── The audit's §3.1 exhibit ─────────────────────────────

const CLAMP: &str = r#"Probe { spec probe_int_clamp {
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
} }"#;

/// The faithful clamp body: lo below, hi above, x between.
fn clamp_body() -> IRNode {
    IRNode::If {
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
    }
}

#[test]
fn phase109_clamp_faithful_body_verifies() {
    let clamp = checked_spec(CLAMP);
    let run = eval::run_spec_tests(&clamp_body(), &clamp);
    assert_eq!((run.passed, run.evaluable), (3, 3));
    let result = Verifier::new().verify_one(&candidate(&clamp, clamp_body()), &clamp);
    println!(
        "P109|clamp|faithful|verify={:?}|counterexample={:?}",
        result.status, result.counterexample
    );
    #[cfg(feature = "z3-support")]
    {
        assert_eq!(
            result.status,
            VerificationStatus::Verified,
            "the faithful clamp body must Verify once `require: lo <= hi` \
             is assumed (§3.1)"
        );
        assert!(result.counterexample.is_none());
        assert!(result.constraint_results.iter().all(|r| r.holds));
    }
}

#[test]
fn phase109_clamp_body_wrong_on_domain_still_refuted() {
    let clamp = checked_spec(CLAMP);
    // Missing the hi clamp: correct below and inside the
    // range, wrong at (99, 0, 10) — a point the
    // precondition ADMITS. Assumptions must not fabricate
    // a pass (the Phase 97 totalization lesson).
    let body = IRNode::If {
        condition: Box::new(IRNode::BinOp(
            IRBinOp::Lt,
            Box::new(var("x")),
            Box::new(var("lo")),
        )),
        then_branch: Box::new(var("lo")),
        else_branch: Box::new(var("x")),
    };
    let result = Verifier::new().verify_one(&candidate(&clamp, body), &clamp);
    println!(
        "P109|clamp|no-hi-clamp|verify={:?}|counterexample={:?}",
        result.status, result.counterexample
    );
    #[cfg(feature = "z3-support")]
    {
        assert_eq!(result.status, VerificationStatus::VerificationFailed);
        let cx = result
            .counterexample
            .expect("a refutation carries its countermodel");
        // The countermodel must be ON-domain: any lo/hi it
        // pins satisfy the precondition it was judged under.
        if let (Some(SmtValue::Int(lo)), Some(SmtValue::Int(hi))) =
            (cx.inputs.get("lo"), cx.inputs.get("hi"))
        {
            assert!(
                lo <= hi,
                "countermodel must satisfy require: lo <= hi, got lo={} hi={}",
                lo, hi
            );
        }
    }
}

#[test]
fn phase109_unsatisfiable_precondition_mints_no_verdict() {
    // `lo <= hi` and `lo > hi`: the domain is empty. Under
    // the assumptions every postcondition is vacuously
    // true — for ANY body. The conservative behaviour
    // (Phase 109 decision): the verification declines
    // whole, the candidate is Inconclusive, no row holds.
    let spec = checked_spec(
        r#"Probe { spec probe_unsat_pre {
            input: x: Int, lo: Int, hi: Int
            output: Int
            require: lo <= hi
            require: lo > hi
            constraint: output >= lo
            constraint: output <= hi
            test { input: (5, 0, 10); expect: 5 }
        } }"#,
    );
    for (label, body) in [
        ("faithful", clamp_body()),
        ("garbage", int(0)),
    ] {
        let result = Verifier::new().verify_one(&candidate(&spec, body), &spec);
        println!("P109|unsat-pre|{}|verify={:?}", label, result.status);
        #[cfg(feature = "z3-support")]
        {
            assert_eq!(
                result.status,
                VerificationStatus::Inconclusive,
                "{}: an unsatisfiable precondition must not mint a verdict",
                label
            );
            assert!(result.constraint_results.iter().all(|r| !r.holds));
        }
    }
}

#[test]
fn phase109_unencodable_precondition_declines_whole() {
    // A Float parameter binds Opaque in the encoder
    // (Phase 76), so `x == x` over it cannot be encoded.
    // The precondition is therefore NOT assumable: the
    // verification declines whole — the body below would
    // Verify if the precondition were silently dropped or
    // assumed, and must instead come back Inconclusive.
    let spec = checked_spec(
        r#"Probe { spec probe_float_pre {
            input: x: Float
            output: Int
            require: x == x
            constraint: output == 0
            test { input: 0; expect: 0 }
        } }"#,
    );
    let result = Verifier::new().verify_one(&candidate(&spec, int(0)), &spec);
    println!("P109|unencodable-pre|verify={:?}", result.status);
    #[cfg(feature = "z3-support")]
    assert_eq!(result.status, VerificationStatus::Inconclusive);
}

// ── The statement judge under assumptions ────────────────

#[test]
fn phase109_statement_judge_assumes_preconditions() {
    // A counting loop whose invariant INITIATION depends
    // on the precondition: at loop entry i = 0, and the
    // invariant row `i <= n` is exactly `0 <= n` — the
    // spec's `require:`. Without assumed preconditions the
    // invariant route cannot discharge initiation, the
    // loop records exhaustion, and the candidate is
    // Inconclusive; with them, it Verifies.
    let spec = checked_spec(
        r#"Probe { spec probe_count_to {
            input: n: Int
            output: Int
            require: n >= 0
            constraint: output == n
            test { input: 0; expect: 0 }
            test { input: 5; expect: 5 }
        } }"#,
    );
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "i".into(),
            ty: morphic::spec::ast::TypeRef::Int,
            initial: Box::new(int(0)),
        },
        IRNode::Alloc {
            name: "acc".into(),
            ty: morphic::spec::ast::TypeRef::Int,
            initial: Box::new(int(0)),
        },
        IRNode::While {
            condition: Box::new(IRNode::BinOp(
                IRBinOp::Lt,
                Box::new(var("i")),
                Box::new(var("n")),
            )),
            body: Box::new(IRNode::Block(vec![
                IRNode::Assign {
                    target: Box::new(var("acc")),
                    value: Box::new(IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(var("acc")),
                        Box::new(int(1)),
                    )),
                },
                IRNode::Assign {
                    target: Box::new(var("i")),
                    value: Box::new(IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(var("i")),
                        Box::new(int(1)),
                    )),
                },
            ])),
            invariant: Some("acc == i; i <= n".into()),
        },
        IRNode::Return(Box::new(var("acc"))),
    ]);
    let run = eval::run_spec_tests(&body, &spec);
    assert_eq!((run.passed, run.evaluable), (2, 2));
    let result = Verifier::new().verify_one(&candidate(&spec, body), &spec);
    println!("P109|stmt|count_to|verify={:?}", result.status);
    #[cfg(feature = "z3-support")]
    assert_eq!(
        result.status,
        VerificationStatus::Verified,
        "the invariant route must see `require: n >= 0` as a standing \
         assumption (§3.1 in the statement judge)"
    );
}

#[test]
fn phase109_per_disjunct_fallback_assumes_preconditions() {
    // The Phase 97 W8 shape: the row's whole encoding
    // declines (the output is a literal Err and the row
    // reads `output.unwrap()`), so the row is judged
    // per-disjunct. The one encodable disjunct is the
    // precondition itself, restated: it decides the row
    // ONLY under the assumed precondition. Without the
    // assumption the fallback's session satisfies
    // ¬(x > lo) and the row stays Unknown.
    let spec = checked_spec(
        r#"Probe { spec probe_guarded_unwrap {
            input: x: Int, lo: Int
            output: Result<Int, String>
            require: x > lo
            constraint: x > lo || output.unwrap() == x
            test { input: (5, 0); expect: Err("bad") }
        } }"#,
    );
    let body = IRNode::Call {
        function: "Err".into(),
        args: vec![IRNode::Literal(IRLiteral::String("bad".into()))],
    };
    let run = eval::run_spec_tests(&body, &spec);
    assert_eq!((run.passed, run.evaluable), (1, 1));
    let result = Verifier::new().verify_one(&candidate(&spec, body), &spec);
    println!("P109|w8|guarded_unwrap|verify={:?}", result.status);
    #[cfg(feature = "z3-support")]
    assert_eq!(
        result.status,
        VerificationStatus::Verified,
        "the per-disjunct fallback must assert the assumed precondition \
         alongside the encodable disjuncts (§3.1 in the W8 route)"
    );
}

// ── Executed judging: spec-silent points ─────────────────

#[test]
fn phase109_precondition_violating_test_points_are_spec_silent() {
    // The clamp spec plus one test at a point its own
    // precondition excludes: (x=5, lo=10, hi=0). The
    // faithful body answers 10 there (x < lo → lo); the
    // test expects 0. The spec makes no promise at that
    // point, so the point is spec-silent: reported as
    // such, excluded from the evaluable denominator,
    // never a body failure.
    let spec = checked_spec(
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
            test { input: (5, 10, 0); expect: 0 }
        } }"#,
    );
    let outcomes = eval::test_outcomes(&clamp_body(), &spec);
    assert_eq!(outcomes.len(), 4);
    assert_eq!(outcomes[0], TestOutcome::Passed);
    assert_eq!(outcomes[1], TestOutcome::Passed);
    assert_eq!(outcomes[2], TestOutcome::Passed);
    assert!(
        matches!(outcomes[3], TestOutcome::SpecSilent(_)),
        "the precondition-violating point must be spec-silent, got {:?}",
        outcomes[3]
    );
    let run = eval::run_spec_tests(&clamp_body(), &spec);
    assert_eq!(
        (run.passed, run.evaluable),
        (3, 3),
        "the silent point leaves the denominator entirely"
    );
    assert_eq!(
        eval::valued_test_count(&clamp_body(), &spec),
        3,
        "valued counts judgeable points only — the silent point's \
         answer is not evidence about the body"
    );
}

#[test]
fn phase109_spec_silent_division_point_is_never_evaluated() {
    // `require: b != 0` and a test at b = 0: the body
    // would divide by zero. The point is spec-silent and
    // the body is never evaluated there.
    let spec = checked_spec(
        r#"Probe { spec probe_div {
            input: a: Int, b: Int
            output: Int
            require: b != 0
            constraint: output * b == a
            test { input: (10, 2); expect: 5 }
            test { input: (10, 0); expect: 0 }
        } }"#,
    );
    let body = IRNode::BinOp(IRBinOp::Div, Box::new(var("a")), Box::new(var("b")));
    let outcomes = eval::test_outcomes(&body, &spec);
    assert_eq!(outcomes[0], TestOutcome::Passed);
    assert!(
        matches!(outcomes[1], TestOutcome::SpecSilent(_)),
        "the b = 0 point must be spec-silent, got {:?}",
        outcomes[1]
    );
    let run = eval::run_spec_tests(&body, &spec);
    assert_eq!((run.passed, run.evaluable), (1, 1));
}

// ── No-precondition specs are untouched ──────────────────

#[test]
fn phase109_no_precondition_verdicts_unchanged() {
    // The Phase 106 int_abs shape: no `require:` rows, so
    // the assumption machinery never engages and the
    // standing verdicts are byte-for-byte the old ones.
    let spec = checked_spec(
        r#"Probe { spec probe_int_abs {
            input: x: Int
            output: Int
            constraint: output >= 0
            constraint: (x >= 0) implies output == x
            constraint: (x < 0) implies output == 0 - x
            test { input: 4; expect: 4 }
            test { input: (0 - 4); expect: 4 }
        } }"#,
    );
    let body = IRNode::UnaryOp(IRUnaryOp::Abs, Box::new(var("x")));
    let result = Verifier::new().verify_one(&candidate(&spec, body), &spec);
    #[cfg(feature = "z3-support")]
    assert_eq!(result.status, VerificationStatus::Verified);
    let wrong = Verifier::new().verify_one(&candidate(&spec, var("x")), &spec);
    #[cfg(feature = "z3-support")]
    assert_eq!(wrong.status, VerificationStatus::VerificationFailed);
}
