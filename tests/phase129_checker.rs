//! Phase 129 (G4 checker admission, D3a) — corpus instrument.
//!
//! Prints the check outcome of every `.morph` file in the
//! standing on-disk corpus (examples, self-host specs, stdlib,
//! playground examples + corpus, held_out), one `P129|` line
//! per file, so the before/after diff of the Call-arm fallback
//! is a measured table rather than an impression. The table
//! measured before the fallback landed is pinned by
//! `phase129_corpus_check_outcomes` itself.

use morphic::spec::{parser, typeck};
use std::path::{Path, PathBuf};

fn corpus_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in [
        "examples",
        "stdlib",
        "playground/examples",
        "playground/corpus",
        "held_out",
    ] {
        collect_morph(&root.join(dir), &mut files);
    }
    files.sort();
    files
}

fn collect_morph(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_morph(&path, out);
        } else if path.extension().is_some_and(|e| e == "morph") {
            out.push(path);
        }
    }
}

fn outcome_of(path: &Path) -> String {
    let source = std::fs::read_to_string(path).expect("corpus file readable");
    let module = match parser::parse(&source) {
        Ok(m) => m,
        Err(e) => return format!("PARSE_ERR {e}"),
    };
    let n = module.functions.len();
    match typeck::check(module) {
        Ok(_) => format!("CHECK_OK specs={n}"),
        Err(e) => format!("CHECK_ERR {}", e.message),
    }
}

#[test]
fn phase129_corpus_check_outcomes() {
    // The standing corpus, pinned as measured BEFORE the
    // Call-arm fallback landed (Phase 129's before/after
    // instrument): the fallback's whole discipline is that
    // this table does not move — the three admitted shapes
    // live in spec sources inside the test suites, not in
    // any file below. The two non-Ok rows are intentional
    // negative fixtures of the playground.
    // Phase 135 amendment: the stdlib/list.morph row's
    // spec count moved 12 → 18 — Amendment 2's six entries
    // joining the module is the phase's own act, and the
    // outcome stays CHECK_OK; no other row moved.
    const PINNED: &[(&str, &str)] = &[
        ("examples/binary_search.morph", "CHECK_OK specs=1"),
        ("examples/functional_tokenizer.morph", "CHECK_OK specs=8"),
        ("examples/self_host/codegen.morph", "CHECK_OK specs=130"),
        ("examples/self_host/count_matching.morph", "CHECK_OK specs=1"),
        ("examples/self_host/is_sorted.morph", "CHECK_OK specs=1"),
        ("examples/self_host/printer.morph", "CHECK_OK specs=7"),
        ("examples/self_host/validator.morph", "CHECK_OK specs=7"),
        ("examples/sort.morph", "CHECK_OK specs=1"),
        ("held_out/rust_visibility.morph", "CHECK_OK specs=3"),
        ("playground/corpus/prelude-int-abs.morph", "CHECK_OK specs=1"),
        ("playground/corpus/prelude-phase112.morph", "CHECK_OK specs=2"),
        ("playground/corpus/prelude-phase115.morph", "CHECK_OK specs=3"),
        ("playground/corpus/stress-biglen.morph", "CHECK_OK specs=1"),
        (
            "playground/examples/invalid-syntax.morph",
            "PARSE_ERR Parse error at line 16, column 1: Expected expression\n  aint: output == x /\n\n                    ^",
        ),
        (
            "playground/examples/invalid-type.morph",
            "CHECK_ERR In spec 'bad_concat': postcondition: `++` (concat) requires two String or two List operands, got Int and Int",
        ),
        ("playground/examples/sort.morph", "CHECK_OK specs=1"),
        ("playground/examples/tokenizer.morph", "CHECK_OK specs=8"),
        ("playground/examples/validator.morph", "CHECK_OK specs=7"),
        ("stdlib/char.morph", "CHECK_OK specs=6"),
        ("stdlib/int.morph", "CHECK_OK specs=7"),
        ("stdlib/list.morph", "CHECK_OK specs=18"),
        ("stdlib/option.morph", "CHECK_OK specs=2"),
        ("stdlib/result.morph", "CHECK_OK specs=1"),
        ("stdlib/string.morph", "CHECK_OK specs=1"),
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = corpus_files();
    assert_eq!(
        files.len(),
        PINNED.len(),
        "the corpus file set itself is pinned; a new .morph file must be recorded deliberately"
    );
    for (path, (rel, expected)) in files.iter().zip(PINNED) {
        assert_eq!(
            path.strip_prefix(root).unwrap().display().to_string(),
            *rel,
            "corpus file order/set moved"
        );
        let outcome = outcome_of(path);
        println!("P129|corpus|{rel}|{outcome}");
        assert_eq!(&outcome, expected, "check outcome moved for {rel}");
    }
}

// ── The admission (G4_SCOPE §2.4 / §3, Phase 129) ─────────
//
// The three Err shapes of the Phase 127 taxonomy flip to Ok;
// the named non-movers stand; the fallback admits ONLY
// Function-typed callees.

use morphic::spec::ast::FunctionSpec;
use morphic::synthesis::engine::{
    CandidateImplementation, IRNode, Provenance, ScoreBreakdown,
};
use morphic::verify::verdict::VerificationStatus;
use morphic::verify::verifier::Verifier;

fn try_checked(source: &str) -> Result<Vec<FunctionSpec>, String> {
    let module = parser::parse(source).map_err(|e| format!("parse: {e}"))?;
    let checked = typeck::check(module).map_err(|e| format!("check: {}", e.message))?;
    Ok(checked.spec.functions)
}

/// The frozen type specimen, verbatim from G4_SCOPE §3.
const APPLY_TWICE: &str = r#"Probe { spec apply_twice {
    input: f: fn(Int) -> Int, x: Int
    output: Int
    constraint: output == f(f(x))
    test { input: (|n| n + 1, 40); expect: 42 }
    test { input: (|n| n * 2, 3); expect: 12 }
} }"#;

#[test]
fn phase129_apply_twice_checks() {
    let checked = try_checked(APPLY_TWICE);
    assert!(
        checked.is_ok(),
        "the frozen specimen checks end to end: {checked:?}"
    );
}

#[test]
fn phase129_apply_fa_checks() {
    // Application under a forall row (the audit's second Err
    // shape).
    let checked = try_checked(
        r#"Probe { spec apply_fa {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: forall i: Int in range(1): output == f(f(x))
            test { input: (|n| n + 1, 40); expect: 42 }
        } }"#,
    );
    assert!(checked.is_ok(), "apply_fa checks end to end: {checked:?}");
}

#[test]
fn phase129_apply_req_checks() {
    // Application in a require: row (the audit's third Err
    // shape).
    let checked = try_checked(
        r#"Probe { spec apply_req {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            require: f(x) > 0
            constraint: output == x
            test { input: (|n| n, 5); expect: 5 }
        } }"#,
    );
    assert!(checked.is_ok(), "apply_req checks end to end: {checked:?}");
}

#[test]
fn phase129_named_non_movers_stand() {
    // pass_f: a fn parameter passed as an argument — Ok
    // before, Ok now, via the fiction table as before.
    let checked = try_checked(
        r#"Probe { spec pass_f {
            input: f: fn(Int, Int) -> Bool, xs: List<Int>
            output: Bool
            constraint: output == is_sorted(xs, f)
            test { input: (|a, b| a < b, [1, 2]); expect: true }
        } }"#,
    );
    assert!(checked.is_ok(), "pass_f stands Ok: {checked:?}");

    // apply_atom: the bare-atom row still lowers to
    // Constraint::Predicate and checks — the §2.4 side door,
    // neither widened nor closed by this phase.
    let atom_src = r#"Probe { spec apply_atom {
        input: cmp: fn(Int, Int) -> Bool, a: Int, b: Int
        output: Bool
        constraint: cmp(a, b)
        constraint: output == true
        test { input: (|x, y| x < y, 1, 2); expect: true }
    } }"#;
    let module = parser::parse(atom_src).expect("atom spec parses");
    assert!(
        module.functions[0]
            .postconditions
            .iter()
            .any(|c| matches!(c, morphic::spec::ast::Constraint::Predicate(name, _) if name == "cmp")),
        "the bare-atom row still lowers to Constraint::Predicate"
    );
    let checked = try_checked(atom_src);
    assert!(checked.is_ok(), "apply_atom stands Ok: {checked:?}");

    // same_f: fn equality in a constraint stays CHECKABLE —
    // its Inconclusive verdict is the verifier's standing
    // (§2.6/§2.7), not the checker's business.
    let checked = try_checked(
        r#"Probe { spec same_f {
            input: f: fn(Int) -> Int, g: fn(Int) -> Int
            output: Bool
            constraint: output == (f == g)
            test { input: (|n| n, |n| n); expect: true }
        } }"#,
    );
    assert!(checked.is_ok(), "same_f stays checkable: {checked:?}");

    // ignore_f: a fn parameter declared and never applied
    // checks — and its Verified standing survives untouched
    // (the §2.7 no-poisoning pin, at this layer too).
    let ignore_src = r#"Probe { spec ignore_f {
        input: f: fn(Int) -> Int, x: Int
        output: Int
        constraint: output == x
        test { input: (|n| n + 1, 5); expect: 5 }
    } }"#;
    let specs = try_checked(ignore_src).expect("ignore_f checks");
    let spec = &specs[0];
    let cand = CandidateImplementation {
        id: 129,
        body: IRNode::Var("x".into()),
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
    };
    let verdict = Verifier::new().verify_one(&cand, spec).status;
    #[cfg(feature = "z3-support")]
    assert_eq!(verdict, VerificationStatus::Verified, "ignore_f still Verifies");
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(verdict, VerificationStatus::Inconclusive);
}

#[test]
fn phase129_fallback_admits_only_function_typed_callees() {
    // A non-function VARIABLE used as a callee is still the
    // standing error — the fallback admits Function types,
    // not variables generally.
    let checked = try_checked(
        r#"Probe { spec call_an_int {
            input: x: Int
            output: Int
            constraint: output == x(1)
            test { input: 1; expect: 1 }
        } }"#,
    );
    let err = checked.expect_err("an Int variable is not callable");
    assert!(
        err.contains("Called undefined function: 'x'"),
        "standing diagnostic expected, got: {err}"
    );

    // A genuinely unknown name is untouched too (the standing
    // pin lives in phase110/typeck; kept here so the
    // discipline reads in one place).
    let checked = try_checked(
        r#"Probe { spec call_nothing {
            input: x: Int
            output: Int
            constraint: output == nosuchfn(x)
            test { input: 1; expect: 1 }
        } }"#,
    );
    let err = checked.expect_err("an unknown callee still fails");
    assert!(
        err.contains("Called undefined function: 'nosuchfn'"),
        "standing diagnostic expected, got: {err}"
    );
}

#[test]
fn phase129_application_yields_the_declared_return_type() {
    // The admitted application's type is the Function type's
    // own return — not a placeholder. `++` is the checker's
    // one precise operator, so it discriminates: String-
    // returning f composes, Int-returning f does not.
    let checked = try_checked(
        r#"Probe { spec apply_str {
            input: f: fn(Int) -> String, x: Int
            output: String
            constraint: output == f(x) ++ "!"
            test { input: (|n| "n", 1); expect: "n!" }
        } }"#,
    );
    assert!(
        checked.is_ok(),
        "a String-returning parameter applies at String: {checked:?}"
    );

    let checked = try_checked(
        r#"Probe { spec apply_int_concat {
            input: f: fn(Int) -> Int, x: Int
            output: String
            constraint: output == f(x) ++ "!"
            test { input: (|n| n, 1); expect: "1!" }
        } }"#,
    );
    assert!(
        checked.is_err(),
        "an Int-returning parameter does not apply at String: {checked:?}"
    );
}

