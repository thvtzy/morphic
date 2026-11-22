//! Phase 112: the `match_from_rows` seed family — the
//! one seed-family addition of the String/Result/Option
//! phase, built for the frozen Result/Option entries
//! (STDLIB_SCOPE §2.5 names it as Phase 112 build scope).
//!
//! The gap (Phase 106 audit §2.5, re-measured at this
//! phase's start): a spec stated as guarded method rows
//! over a Result/Option parameter — the frozen contract
//! form — was claimed by NO seed family (the row
//! families' guard leaves are comparisons; the equation
//! families state equations), so the pipeline crowned
//! test-induced match junk, Inconclusive, while a
//! hand-written constructor Match against the same rows
//! Verified ∧ golden-exact. The family reads the Match
//! directly off the rows: the negated method test names
//! the excluded case, the row's pin is that arm's body,
//! and the payload arm's `unwrap()` rewrites to the
//! bound payload variable.
//!
//! Teeth: the family fires on the three shipped library
//! specs and on a renamed, composed-pin variant (it
//! reads structure, not names); it declines a duplicate
//! arm, a single row, two candidate scrutinees, an
//! empty-arm pin that projects a payload, a foreign
//! method call in a pin, a compound guard, and an
//! equation-only spec.

use morphic::api;
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{seed_engine_rng, SynthesisEngine};

fn kinds(source: &str, spec_name: &str) -> Vec<String> {
    let module = parser::parse(source).expect("test source parses");
    let checked = typeck::check(module).expect("test source typechecks");
    seed_engine_rng(42);
    let eng = SynthesisEngine::with_module(api::playground_config(), &checked.spec.functions);
    let spec = checked
        .spec
        .functions
        .iter()
        .find(|f| f.name == spec_name)
        .unwrap();
    eng.constraint_seed_kinds(spec)
}

fn fires(source: &str, spec_name: &str) -> bool {
    kinds(source, spec_name).contains(&"match_from_rows".to_string())
}

#[test]
fn fires_on_the_shipped_library_specs() {
    let result = include_str!("../stdlib/result.morph");
    let option = include_str!("../stdlib/option.morph");
    assert!(fires(result, "result_unwrap_or"));
    assert!(fires(option, "option_unwrap_or"));
    assert!(fires(option, "option_ok_or"));
}

#[test]
fn fires_on_a_renamed_composed_pin_variant() {
    // Different parameter names, and the payload pin is a
    // composed expression over the projection — the
    // family reads the rows' structure, not the library's
    // names or the pin's exact shape.
    let src = r#"Probe { spec shifted {
        input: res: Result<Int, String>, base: Int
        output: Int
        constraint: res.is_err() || output == res.unwrap() + base
        constraint: res.is_ok() || output == base
        test { input: (Ok(1), 10); expect: 11 }
        test { input: (Err("x"), 10); expect: 10 }
    } }"#;
    assert!(fires(src, "shifted"));
    // And the crowned body is the recovered Match, judged
    // Verified through the production pipeline. (The
    // crown verdict is a Z3-configuration fact; the
    // no-Z3 suite pins the family's firing above and the
    // executed agreement through the cross-validation
    // harness.)
    #[cfg(feature = "z3-support")]
    {
        let report = api::synthesize_seeded_spec(src, "shifted", 42).expect("pipeline run");
        assert_eq!(report.winner.verification_status, "Verified");
        assert_eq!(
            report.winner.body_debug,
            "Match { scrutinee: Var(\"res\"), cases: [(Constructor(\"Ok\", [Variable(\"v\")]), \
             BinOp(Add, Var(\"v\"), Var(\"base\"))), (Constructor(\"Err\", [Wildcard]), Var(\"base\"))] }"
                .replace("         ", "")
        );
    }
}

#[test]
fn declines_a_duplicate_arm() {
    // Two rows state the payload arm; the empty arm is
    // unstated. Not a table.
    let src = r#"Probe { spec dup {
        input: r: Result<Int, String>, d: Int
        output: Int
        constraint: r.is_err() || output == r.unwrap()
        constraint: r.is_err() || output == 0
        test { input: (Ok(1), 3); expect: 1 }
    } }"#;
    assert!(!fires(src, "dup"));
}

#[test]
fn declines_a_single_row() {
    let src = r#"Probe { spec one {
        input: o: Option<Int>, d: Int
        output: Int
        constraint: o.is_none() || output == o.unwrap()
        test { input: (Some(1), 3); expect: 1 }
    } }"#;
    assert!(!fires(src, "one"));
}

#[test]
fn declines_two_candidate_scrutinees() {
    // Two Option parameters: which one the rows denote
    // is not this family's call.
    let src = r#"Probe { spec two {
        input: a: Option<Int>, b: Option<Int>
        output: Int
        constraint: a.is_none() || output == a.unwrap()
        constraint: a.is_some() || output == 0
        test { input: (Some(1), None()); expect: 1 }
    } }"#;
    assert!(!fires(src, "two"));
}

#[test]
fn declines_an_empty_arm_pin_that_projects() {
    // The None arm has no payload; a pin projecting one
    // there is not a row of this table.
    let src = r#"Probe { spec proj {
        input: o: Option<Int>, d: Int
        output: Int
        constraint: o.is_none() || output == o.unwrap()
        constraint: o.is_some() || output == o.unwrap()
        test { input: (Some(1), 3); expect: 1 }
    } }"#;
    assert!(!fires(src, "proj"));
}

#[test]
fn declines_a_foreign_method_call_in_a_pin() {
    // The payload pin calls a method on the PROJECTED
    // value, not the scrutinee's own unwrap — outside
    // the family's rewrite.
    let src = r#"Probe { spec foreign {
        input: o: Option<Result<Int, String>>
        output: Bool
        constraint: o.is_none() || output == o.unwrap().is_ok()
        constraint: o.is_some() || output == false
        test { input: Some(Ok(1)); expect: true }
    } }"#;
    assert!(!fires(src, "foreign"));
}

#[test]
fn declines_a_compound_guard() {
    // One row, two guard leaves: not the single-test
    // guard form.
    let src = r#"Probe { spec compound {
        input: r: Result<Int, String>, d: Int
        output: Int
        constraint: r.is_err() || r.is_ok() || output == d
        constraint: r.is_ok() || output == d
        test { input: (Ok(1), 3); expect: 3 }
    } }"#;
    assert!(!fires(src, "compound"));
}

#[test]
fn declines_an_equation_only_spec() {
    // No rows at all: the equation family's territory,
    // and this family must not invent a Match for it.
    let src = r#"Probe { spec eq {
        input: o: Option<Int>
        output: Bool
        constraint: output == o.is_some()
        test { input: Some(1); expect: true }
    } }"#;
    assert!(!fires(src, "eq"));
}
