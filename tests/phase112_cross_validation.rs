//! Phase 112 cross-validation (gate clause 3's
//! discipline, STDLIB_SCOPE §3): every shipped
//! String/Result/Option entry is executed against a
//! hand-written Rust mirror over a battery far wider
//! than its in-spec tests — pinned points must agree
//! 100%. None of these entries carries a precondition,
//! so there are no spec-silent points in these
//! batteries; every point is pinned.
//!
//! The battery runs through the production executed
//! judge (`eval::run_spec_tests` / `test_outcomes`) on
//! a generated spec carrying the entry's signature; the
//! expected values are computed by the mirrors below.

//! Configuration note: these pins judge the NATIVE
//! prelude (embedded library data) and are gated
//! `#[cfg(not(feature = "wasm-playground"))]` — under
//! that feature the prelude is installed from the stdlib
//! asset at runtime (browser-side behaviour is pinned by
//! the Phase 112 asset pins + the playground corpus); a
//! workspace `--all-targets` build unifies the feature
//! into this package via the morphic-wasm member. In the
//! standing native configurations every pin runs.

use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::eval::{self, TestOutcome};

// ── Hand-written mirrors ─────────────────────────────────

fn m_is_empty(s: &str) -> bool {
    s.is_empty()
}
fn m_result_unwrap_or(r: Result<i64, String>, d: i64) -> i64 {
    r.unwrap_or(d)
}
fn m_option_unwrap_or(o: Option<i64>, d: i64) -> i64 {
    o.unwrap_or(d)
}
fn m_option_ok_or(o: Option<i64>, e: String) -> Result<i64, String> {
    o.ok_or(e)
}

// ── Source formatting for battery points ─────────────────

fn fmt_int(n: i64) -> String {
    if n == i64::MIN {
        "(0 - 9223372036854775807 - 1)".to_string()
    } else if n < 0 {
        format!("(0 - {})", n.unsigned_abs())
    } else {
        n.to_string()
    }
}

fn fmt_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn fmt_result(r: &Result<i64, String>) -> String {
    match r {
        Ok(n) => format!("Ok({})", fmt_int(*n)),
        Err(e) => format!("Err({})", fmt_str(e)),
    }
}

fn fmt_option(o: &Option<i64>) -> String {
    match o {
        Some(n) => format!("Some({})", fmt_int(*n)),
        None => "None()".to_string(),
    }
}

fn fmt_bool(b: bool) -> String {
    b.to_string()
}

// ── The battery runner ───────────────────────────────────

/// Builds a one-spec module carrying the entry's
/// signature with the battery as its tests, then judges
/// the shipped body over it. Every point is pinned:
/// each outcome must be Passed.
fn run_battery(entry: &str, signature: &str, pinned: Vec<(String, String)>) {
    let mut src = String::from("Battery { spec bat {\n");
    src.push_str(signature);
    src.push('\n');
    for (input, expect) in &pinned {
        src.push_str(&format!("    test {{ input: {input}; expect: {expect} }}\n"));
    }
    src.push_str("} }");
    let module = parser::parse(&src)
        .unwrap_or_else(|e| panic!("{entry}: battery source must parse: {e}"));
    let checked = typeck::check(module)
        .unwrap_or_else(|e| panic!("{entry}: battery source must check: {e}"));
    let spec = &checked.spec.functions[0];
    let body = stdlib::shipped_body(entry)
        .unwrap_or_else(|| panic!("{entry} ships a body"));
    let outcomes = eval::test_outcomes(&body, spec);
    assert_eq!(outcomes.len(), pinned.len(), "{entry}: battery size");
    for (idx, outcome) in outcomes.iter().enumerate() {
        assert!(
            matches!(outcome, TestOutcome::Passed),
            "{entry}: pinned point {} ({}) must pass, got {:?}",
            idx,
            pinned[idx].0,
            outcome
        );
    }
    let run = eval::run_spec_tests(&body, spec);
    assert_eq!(
        (run.passed, run.evaluable),
        (pinned.len(), pinned.len()),
        "{entry}: mirror agreement on every pinned point"
    );
    println!("P112XV|{}|pinned={}|agree=100%", entry, pinned.len());
}

// ── The batteries ────────────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn xv_string_is_empty() {
    let strings: Vec<String> = vec![
        String::new(),
        "a".into(),
        "z".into(),
        "ab".into(),
        "hello".into(),
        "hello world".into(),
        " ".into(),
        "  ".into(),
        "0".into(),
        "false".into(),
        "\n".into(),
        "a\tb".into(),
        "é".into(),
        "🦀".into(),
        "a".repeat(64),
    ];
    let pinned: Vec<(String, String)> = strings
        .iter()
        .map(|s| (fmt_str(s), fmt_bool(m_is_empty(s))))
        .collect();
    run_battery(
        "string_is_empty",
        "    input: s: String\n    output: Bool",
        pinned,
    );
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn xv_result_unwrap_or() {
    let results: Vec<Result<i64, String>> = vec![
        Ok(0),
        Ok(1),
        Ok(-1),
        Ok(42),
        Ok(i64::MAX),
        Ok(i64::MIN),
        Err(String::new()),
        Err("bad".into()),
        Err("a longer error message with spaces".into()),
    ];
    let defaults = [0, 3, -7, i64::MAX];
    let mut pinned = Vec::new();
    for r in &results {
        for d in defaults {
            pinned.push((
                format!("({}, {})", fmt_result(r), fmt_int(d)),
                fmt_int(m_result_unwrap_or(r.clone(), d)),
            ));
        }
    }
    run_battery(
        "result_unwrap_or",
        "    input: r: Result<Int, String>, d: Int\n    output: Int",
        pinned,
    );
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn xv_option_unwrap_or() {
    let options: Vec<Option<i64>> = vec![
        Some(0),
        Some(1),
        Some(-1),
        Some(42),
        Some(i64::MAX),
        Some(i64::MIN),
        None,
    ];
    let defaults = [0, 5, -3, i64::MIN];
    let mut pinned = Vec::new();
    for o in &options {
        for d in defaults {
            pinned.push((
                format!("({}, {})", fmt_option(o), fmt_int(d)),
                fmt_int(m_option_unwrap_or(*o, d)),
            ));
        }
    }
    run_battery(
        "option_unwrap_or",
        "    input: o: Option<Int>, d: Int\n    output: Int",
        pinned,
    );
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn xv_option_ok_or() {
    let cases: Vec<(Option<i64>, String)> = vec![
        (Some(0), String::new()),
        (Some(1), "x".into()),
        (Some(-1), "missing".into()),
        (Some(i64::MAX), "e".into()),
        (Some(i64::MIN), "unused".into()),
        (None, String::new()),
        (None, "missing".into()),
        (None, "a longer error message".into()),
        (None, "with \"quotes\" inside".into()),
    ];
    let pinned: Vec<(String, String)> = cases
        .iter()
        .map(|(o, e)| {
            (
                format!("({}, {})", fmt_option(o), fmt_str(e)),
                fmt_result(&m_option_ok_or(*o, e.clone())),
            )
        })
        .collect();
    run_battery(
        "option_ok_or",
        "    input: o: Option<Int>, e: String\n    output: Result<Int, String>",
        pinned,
    );
}
