//! Phase 111: teeth for the `char_guard_table` seed
//! family — the phase's one seed-family addition, built
//! for the 1.0 library's `char_to_digit`.
//!
//! The family recovers a guard table stated over ONE
//! Char parameter (rows of guard leaves against CharLit
//! constants, pinning arbitrary hole-free expressions —
//! the constructor pins `Some(d)` / `None()` that the
//! Int guard table's literal-pin test exists to refuse)
//! as a match over the parameter. These tests pin, via
//! the engine's instrument surface: the family fires on
//! the shipped entry's shape; it declines conflicting
//! pins, disagreeing tails, an equation-shaped Char spec
//! (the equation family's territory), and a non-Char key.

use morphic::api;
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{self, SynthesisEngine};

fn kinds(source: &str, name: &str) -> Vec<String> {
    let module = parser::parse(source).expect("test source parses");
    let checked = typeck::check(module).expect("test source checks");
    let spec = checked
        .spec
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("spec {name} present"))
        .clone();
    engine::seed_engine_rng(42);
    let eng = SynthesisEngine::with_module(api::playground_config(), &checked.spec.functions);
    eng.constraint_seed_kinds(&spec)
}

#[test]
fn fires_on_the_shipped_char_to_digit() {
    let source = stdlib::LIBRARY_MODULES[1].1;
    let ks = kinds(source, "char_to_digit");
    assert!(
        ks.iter().any(|k| k == "char_guard_table"),
        "the shipped entry's seed kinds must include char_guard_table, got {ks:?}"
    );
}

#[test]
fn declines_conflicting_pins() {
    // Two rows pin '5' to different values: jointly
    // inconsistent — not a table, no seed.
    let source = r#"T { spec f {
        input: c: Char
        output: Option<Int>
        constraint: c != '5' || output == Some(5)
        constraint: c != '5' || output == Some(6)
        constraint: c >= '0' || output == None()
        constraint: c <= '9' || output == None()
        test { input: '5'; expect: Some(5) }
    } }"#;
    let ks = kinds(source, "f");
    assert!(
        !ks.iter().any(|k| k == "char_guard_table"),
        "conflicting pins must decline, got {ks:?}"
    );
}

#[test]
fn declines_disagreeing_tails() {
    // The left tail pins Some(0), the right tail pins
    // None(): expression pins have no typed default to
    // fall back to, so no single wildcard states this
    // table — no seed.
    let source = r#"T { spec f {
        input: c: Char
        output: Option<Int>
        constraint: c != '0' || output == Some(0)
        constraint: c >= '0' || output == Some(0)
        constraint: c <= '9' || output == None()
        test { input: '0'; expect: Some(0) }
    } }"#;
    let ks = kinds(source, "f");
    assert!(
        !ks.iter().any(|k| k == "char_guard_table"),
        "disagreeing tails must decline, got {ks:?}"
    );
}

#[test]
fn declines_equation_shaped_char_spec() {
    // A Char classifier stated as an equation is the
    // equation family's territory; the row builder finds
    // no rows and stays out.
    let source = stdlib::LIBRARY_MODULES[1].1;
    let ks = kinds(source, "char_is_digit");
    assert!(
        ks.iter().any(|k| k == "equation"),
        "the classifier must stay the equation family's, got {ks:?}"
    );
    assert!(
        !ks.iter().any(|k| k == "char_guard_table"),
        "no guard-table seed for an equation spec, got {ks:?}"
    );
}

#[test]
fn declines_non_char_key() {
    // The same row shape over an Int key is the standing
    // guard table's territory, not this family's.
    let source = r#"T { spec f {
        input: x: Int
        output: Int
        constraint: x != 5 || output == 5
        constraint: x >= 0 || output == 0
        constraint: x <= 9 || output == 0
        test { input: 5; expect: 5 }
    } }"#;
    let ks = kinds(source, "f");
    assert!(
        !ks.iter().any(|k| k == "char_guard_table"),
        "an Int key must decline, got {ks:?}"
    );
}
