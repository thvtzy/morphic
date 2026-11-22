//! Phase 112 (1.0 library, String/Result/Option): the
//! Option method-table extension — the frozen enabling
//! extension of STDLIB_SCOPE §2.5.
//!
//! The closed MethodCall builtin table (Phase 74, D1a)
//! covered `Result::{is_ok, is_err, unwrap}` and
//! `List::len` only, so no spec could project a symbolic
//! `Some` payload and the Option entries' contracts were
//! unstatable. The extension adds `Option::{is_some,
//! is_none, unwrap}` through the layers the table touches
//! — typechecker, interpreter, Z3 encoder — with the
//! Phase 97 guarded-unwrap discipline holding for Option
//! exactly as for Result. These tests are the teeth:
//! contracts become statable and judge correctly, and
//! wrong-receiver / wrong-arity uses are type errors.
//!
//! The native Rust emitter's spec-Expr arm is
//! deliberately NOT extended here: the self-host Codegen
//! spec `rust_expr_method_form` pins the table by name,
//! so the emitter arm moves only with a Codegen-spec
//! lockstep amendment (the Phase 78 precedent) — a
//! bounded bootstrap cascade, reported as the Phase 112
//! finding rather than absorbed silently.

use morphic::spec::ast::{Expr, FunctionSpec};
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{
    CandidateImplementation, IRLiteral, IRNode, IRPattern, Provenance, ScoreBreakdown,
};
use morphic::synthesis::eval::{self, Value};
use morphic::verify::verdict::VerificationStatus;
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

fn check_err(source: &str) -> String {
    let module = parser::parse(source).expect("test source parses");
    match typeck::check(module) {
        Ok(_) => panic!("source must fail typecheck:\n{source}"),
        Err(e) => e.message,
    }
}

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 112,
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

/// The faithful constructor-Match body for an
/// Option-scrutinee spec: `Some(v) -> some_arm`,
/// `None -> none_arm`.
fn option_match(scrutinee: &str, some_arm: IRNode, none_arm: IRNode) -> IRNode {
    IRNode::Match {
        scrutinee: Box::new(var(scrutinee)),
        cases: vec![
            (
                IRPattern::Constructor("Some".into(), vec![IRPattern::Variable("v".into())]),
                some_arm,
            ),
            (IRPattern::Constructor("None".into(), vec![]), none_arm),
        ],
    }
}

const UNWRAP_OR: &str = r#"Probe { spec probe_unwrap_or {
    input: o: Option<Int>, d: Int
    output: Int
    constraint: o.is_none() || output == o.unwrap()
    constraint: o.is_some() || output == d
    test { input: (Some(7), 3); expect: 7 }
    test { input: (None(), 3); expect: 3 }
    test { input: (Some(0), 5); expect: 0 }
    test { input: (None(), 0); expect: 0 }
} }"#;

const OK_OR: &str = r#"Probe { spec probe_ok_or {
    input: o: Option<Int>, e: String
    output: Result<Int, String>
    constraint: o.is_none() || output == Ok(o.unwrap())
    constraint: o.is_some() || output == Err(e)
    test { input: (Some(4), "x"); expect: Ok(4) }
    test { input: (None(), "x"); expect: Err("x") }
    test { input: (None(), ""); expect: Err("") }
} }"#;

// ── Typechecker: contracts become statable ─────────────

#[test]
fn option_method_contracts_typecheck() {
    // The guarded form over the extended table (the
    // frozen contract shape of `option_unwrap_or`).
    let spec = checked_spec(UNWRAP_OR);
    assert_eq!(spec.params[0].ty.to_string(), "Option<Int>");
    // The conversion form (the frozen `option_ok_or`).
    let spec = checked_spec(OK_OR);
    assert_eq!(spec.return_type.to_string(), "Result<Int, String>");
    // A direct equation over is_some.
    checked_spec(
        r#"Probe { spec probe_is_some {
            input: o: Option<Int>
            output: Bool
            constraint: output == o.is_some()
            test { input: Some(1); expect: true }
            test { input: None(); expect: false }
        } }"#,
    );
    // Chained postfix forms still compose: unwrap, then
    // a Result method on the projected value's spec —
    // here unwrap of an Option<Result<..>> then is_ok.
    checked_spec(
        r#"Probe { spec probe_chain {
            input: o: Option<Result<Int, String>>
            output: Bool
            constraint: o.is_none() || output == o.unwrap().is_ok()
            constraint: o.is_some() || output == false
            test { input: Some(Ok(1)); expect: true }
            test { input: None(); expect: false }
        } }"#,
    );
    // The standing tables are untouched: Result methods
    // and List::len still check (the tokenizer dialect).
    checked_spec(
        r#"Probe { spec probe_result {
            input: r: Result<Int, String>, xs: List<Int>
            output: Int
            constraint: r.is_err() || output == r.unwrap()
            constraint: r.is_ok() || output == xs.len()
            test { input: (Ok(7), [1]); expect: 7 }
        } }"#,
    );
}

#[test]
fn option_method_type_errors_name_the_method() {
    // Wrong receiver family: Option methods on an Int.
    let msg = check_err(
        r#"Probe { spec bad {
            input: x: Int
            output: Bool
            constraint: output == x.is_some()
        } }"#,
    );
    assert!(msg.contains("is_some"), "error must name the method: {msg}");
    // Cross-family: a Result has no is_some.
    let msg = check_err(
        r#"Probe { spec bad {
            input: r: Result<Int, String>
            output: Bool
            constraint: output == r.is_some()
        } }"#,
    );
    assert!(msg.contains("is_some"), "error must name the method: {msg}");
    // Cross-family the other way: an Option has no is_ok.
    let msg = check_err(
        r#"Probe { spec bad {
            input: o: Option<Int>
            output: Bool
            constraint: output == o.is_ok()
        } }"#,
    );
    assert!(msg.contains("is_ok"), "error must name the method: {msg}");
    // An Option has no len (that is the List method).
    let msg = check_err(
        r#"Probe { spec bad {
            input: o: Option<Int>
            output: Int
            constraint: output == o.len()
        } }"#,
    );
    assert!(msg.contains("len"), "error must name the method: {msg}");
    // Wrong arity: every builtin is zero-argument.
    let msg = check_err(
        r#"Probe { spec bad {
            input: o: Option<Int>
            output: Bool
            constraint: output == o.is_some(1)
        } }"#,
    );
    assert!(msg.contains("is_some"), "error must name the method: {msg}");
    let msg = check_err(
        r#"Probe { spec bad {
            input: o: Option<Int>, d: Int
            output: Int
            constraint: o.is_none() || output == o.unwrap(2)
        } }"#,
    );
    assert!(msg.contains("unwrap"), "error must name the method: {msg}");
    // Unknown methods still error, on Option receivers too.
    let msg = check_err(
        r#"Probe { spec bad {
            input: o: Option<Int>
            output: Int
            constraint: output == o.foo()
        } }"#,
    );
    assert!(msg.contains("foo"), "error must name the method: {msg}");
}

// ── Interpreter: the table on closed values ────────────

fn method_on(receiver: Expr, name: &str) -> Option<Value> {
    eval::value_of_expr(&Expr::MethodCall {
        receiver: Box::new(receiver),
        name: name.into(),
        args: vec![],
    })
}

fn ctor(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call(name.into(), args)
}

#[test]
fn option_methods_execute_on_closed_values() {
    let some5 = ctor("Some", vec![Expr::IntLit(5)]);
    let none = ctor("None", vec![]);
    assert_eq!(method_on(some5.clone(), "is_some"), Some(Value::Bool(true)));
    assert_eq!(method_on(some5.clone(), "is_none"), Some(Value::Bool(false)));
    assert_eq!(method_on(some5.clone(), "unwrap"), Some(Value::Int(5)));
    assert_eq!(method_on(none.clone(), "is_some"), Some(Value::Bool(false)));
    assert_eq!(method_on(none.clone(), "is_none"), Some(Value::Bool(true)));
    // unwrap on a None value is an evaluation failure —
    // value_of_expr's only failure channel is None —
    // never a guessed payload.
    assert_eq!(method_on(none.clone(), "unwrap"), None);
    // Cross-family names do not exist on Option values.
    assert_eq!(method_on(some5.clone(), "is_ok"), None);
    assert_eq!(method_on(some5, "len"), None);
    assert_eq!(method_on(none, "foo"), None);
}

// ── Judging: guarded rows prove and refute ─────────────

#[test]
fn option_unwrap_or_hand_body_executes_and_verifies() {
    let spec = checked_spec(UNWRAP_OR);
    let body = option_match("o", var("v"), var("d"));
    let run = eval::run_spec_tests(&body, &spec);
    assert_eq!((run.passed, run.evaluable), (4, 4), "faithful body is golden-exact");
    // A body that ignores the payload (always the
    // default) fails the Some points — the guarded
    // unwrap row genuinely constrains.
    let lazy = var("d");
    let run = eval::run_spec_tests(&lazy, &spec);
    assert_eq!(run.passed, 2, "the default-only body passes only the None points");
    #[cfg(feature = "z3-support")]
    {
        let verifier = Verifier::with_specs(std::slice::from_ref(&spec));
        let verdict = verifier.verify_one(&candidate(&spec, body), &spec).status;
        assert_eq!(verdict, VerificationStatus::Verified, "guarded rows prove");
        let verdict = verifier.verify_one(&candidate(&spec, lazy), &spec).status;
        assert_eq!(
            verdict,
            VerificationStatus::VerificationFailed,
            "guarded rows refute a body that drops the payload"
        );
    }
}

#[test]
fn option_ok_or_hand_body_executes_and_verifies() {
    let spec = checked_spec(OK_OR);
    let body = option_match("o", call("Ok", vec![var("v")]), call("Err", vec![var("e")]));
    let run = eval::run_spec_tests(&body, &spec);
    assert_eq!((run.passed, run.evaluable), (3, 3));
    #[cfg(feature = "z3-support")]
    {
        let verdict = Verifier::with_specs(std::slice::from_ref(&spec))
            .verify_one(&candidate(&spec, body), &spec)
            .status;
        assert_eq!(verdict, VerificationStatus::Verified);
    }
}

#[test]
fn is_some_equation_proves_and_refutes() {
    let spec = checked_spec(
        r#"Probe { spec probe_is_some {
            input: o: Option<Int>
            output: Bool
            constraint: output == o.is_some()
            test { input: Some(1); expect: true }
            test { input: None(); expect: false }
        } }"#,
    );
    let truth = IRNode::Literal(IRLiteral::Bool(true));
    let falsity = IRNode::Literal(IRLiteral::Bool(false));
    let body = option_match("o", truth.clone(), falsity.clone());
    let run = eval::run_spec_tests(&body, &spec);
    assert_eq!((run.passed, run.evaluable), (2, 2));
    #[cfg(feature = "z3-support")]
    {
        let verifier = Verifier::with_specs(std::slice::from_ref(&spec));
        assert_eq!(
            verifier.verify_one(&candidate(&spec, body), &spec).status,
            VerificationStatus::Verified
        );
        let swapped = option_match("o", falsity, truth);
        assert_eq!(
            verifier.verify_one(&candidate(&spec, swapped), &spec).status,
            VerificationStatus::VerificationFailed,
            "is_some is not is_none: the swapped table refutes"
        );
    }
}
