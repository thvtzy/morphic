//! Phase 124 (1.1 Embeddable, inserted by the owner's
//! route-(i) decision of 2026-11-23): the Char-pattern
//! lockstep cascade + NF-116-2's global fix.
//!
//! The pins:
//!
//! * `char_to_digit`'s shipped body, generated to Rust
//!   and COMPILED (the route no standing pin executed
//!   before Phase 123's facade rehearsal exposed the
//!   widening), answers its full frozen 101-point
//!   battery at 100% — the Phase 111 battery alphabet
//!   and mirror, through rustc this time;
//! * a synthetic Char match — literal arms (including
//!   the escape-sensitive chars), a wildcard arm and a
//!   binding arm in mixed order — renders literally
//!   and executes exactly;
//! * NF-116-2, globally: generated precondition asserts
//!   render genuinely for every constraint shape the
//!   renderer covers — Expr (with the assert-position
//!   clone rule reaching calls nested in the
//!   proposition), Predicate, And, Implies — and the
//!   one named fallback (the quantified kinds, Forall /
//!   Exists: no value-level rendering of a quantified
//!   proposition exists) keeps the placeholder BY NAME,
//!   pinned here as deliberate.
//! * The genuine assert's consequence for the file
//!   Phase 116 assembles: an entry a precondition names
//!   is emitted as a real function alongside (the
//!   precondition-aware extension of the used-entry
//!   scan), so a standalone generated file for
//!   `list_binary_search` — whose require names
//!   `list_is_sorted` — compiles and runs; before the
//!   extension the compiled route failed E0425 on
//!   exactly this (the Phase 115 compiled-route pin
//!   caught it, and is the regression this closes).
//!
//! Configuration note (the standing one): pins that
//! read the NATIVE prelude are gated
//! `#[cfg(not(feature = "wasm-playground"))]`.

use morphic::codegen;
use morphic::spec::ast::FunctionSpec;
use morphic::spec::parser;
use morphic::synthesis::engine::{
    CandidateImplementation, IRLiteral, IRNode, IRPattern, Provenance, ScoreBreakdown,
};

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 124,
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

fn hand_spec(source: &str) -> FunctionSpec {
    let module = parser::parse(source).expect("probe module parses");
    module.functions.into_iter().next().expect("one spec")
}

fn rust_of(spec: &FunctionSpec, body: IRNode) -> String {
    codegen::generate(&candidate(spec, body), "rust").expect("Rust generation")
}

fn compile_and_run(source: &str, tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase124-{tag}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join("probe.rs");
    std::fs::write(&src_path, source).unwrap();
    let bin_path = dir.join("probe_test");
    let compile = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2021",
            "--test",
            src_path.to_str().unwrap(),
            "-o",
            bin_path.to_str().unwrap(),
        ])
        .output()
        .expect("rustc runs");
    assert!(
        compile.status.success(),
        "the generated module must compile standalone:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = std::process::Command::new(&bin_path)
        .output()
        .expect("the compiled test binary runs");
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        run.status.success(),
        "the compiled battery must pass:\n{stdout}"
    );
    stdout
}

// ── char_to_digit through the compiled route ─────────

/// The Phase 111 battery alphabet: printable ASCII
/// plus the named whitespace escapes, NUL, vertical
/// tab and DEL — 101 chars.
#[cfg(not(feature = "wasm-playground"))]
fn battery_chars() -> Vec<char> {
    let mut chars: Vec<char> = (32u8..=126).map(char::from).collect();
    chars.extend(['\t', '\n', '\r', '\u{0}', '\u{b}', '\u{7f}']);
    chars
}

/// A Rust char literal for the harness source (the
/// Phase 111 source formatting, Rust-flavoured).
fn rust_char(c: char) -> String {
    match c {
        '\t' => "'\\t'".to_string(),
        '\n' => "'\\n'".to_string(),
        '\r' => "'\\r'".to_string(),
        '\\' => "'\\\\'".to_string(),
        '\'' => "'\\''".to_string(),
        c if (' '..='~').contains(&c) => format!("'{c}'"),
        c => format!("'\\u{{{:x}}}'", c as u32),
    }
}

#[cfg(not(feature = "wasm-playground"))]
fn m_to_digit(c: char) -> Option<i64> {
    if c.is_ascii_digit() {
        Some(c as i64 - '0' as i64)
    } else {
        None
    }
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn char_to_digit_compiled_route_answers_its_full_battery() {
    let spec = morphic::stdlib::prelude_specs()
        .into_iter()
        .find(|s| s.name == "char_to_digit")
        .expect("char_to_digit in prelude");
    let body = morphic::stdlib::shipped_body("char_to_digit").expect("char_to_digit ships a body");
    let mut rust = rust_of(&spec, body);
    // The widening is closed in the text itself: the
    // digit arms render as char literals, exactly one
    // wildcard arm remains (the None fall-through).
    for d in '0'..='9' {
        assert!(
            rust.contains(&format!("'{d}' =>")),
            "the '{d}' arm renders literally:\n{rust}"
        );
    }
    assert_eq!(
        rust.matches("_ =>").count(),
        1,
        "exactly the wildcard fall-through remains:\n{rust}"
    );
    // And through the compiled route: the full frozen
    // battery, expectations by the Phase 111 mirror.
    let cases: Vec<String> = battery_chars()
        .iter()
        .map(|&c| {
            let want = match m_to_digit(c) {
                Some(n) => format!("Some({n})"),
                None => "None".to_string(),
            };
            format!("({}, {})", rust_char(c), want)
        })
        .collect();
    assert_eq!(cases.len(), 101, "the frozen battery is 101 points");
    rust.push_str(&format!(
        r#"
#[cfg(test)]
mod phase124_battery {{
    use super::*;

    #[test]
    fn char_to_digit_full_battery() {{
        let cases: [(char, Option<i64>); 101] = [{}];
        for (c, want) in cases {{
            assert_eq!(char_to_digit(c), want, "at {{c:?}}");
        }}
    }}
}}
"#,
        cases.join(", ")
    ));
    let stdout = compile_and_run(&rust, "char-to-digit");
    assert!(
        stdout.contains("char_to_digit_full_battery") && stdout.contains("test result: ok"),
        "the compiled battery passes:\n{stdout}"
    );
}

// ── A synthetic Char match ───────────────────────────

#[test]
fn synthetic_char_match_renders_and_executes_exactly() {
    let spec = hand_spec(
        "Probe { spec probe {\n    input: c: Char\n    output: Int\n    constraint: output == 0\n    test \"t\" { input: 'a' expect: 1 }\n} }",
    );
    let lit_arm = |c: char, n: i64| {
        (
            IRPattern::Literal(IRLiteral::Char(c)),
            IRNode::Literal(IRLiteral::Int(n)),
        )
    };
    // Literal arms (two of them escape-sensitive), then
    // a wildcard arm, then a binding arm — mixed order:
    // the binding arm is unreachable by construction,
    // and the pin is that it still RENDERS as a binding
    // (the emitter does not widen what it cannot see
    // executed).
    let body = IRNode::Match {
        scrutinee: Box::new(IRNode::Var("c".into())),
        cases: vec![
            lit_arm('a', 1),
            lit_arm('\n', 2),
            lit_arm('\'', 3),
            lit_arm('\\', 4),
            (IRPattern::Wildcard, IRNode::Literal(IRLiteral::Int(9))),
            (
                IRPattern::Variable("other".into()),
                IRNode::Literal(IRLiteral::Int(5)),
            ),
        ],
    };
    let mut rust = rust_of(&spec, body);
    for arm in ["'a' =>", "'\\n' =>", "'\\'' =>", "'\\\\' =>", "_ =>", "other =>"] {
        assert!(rust.contains(arm), "the arm {arm} renders:\n{rust}");
    }
    rust.push_str(
        r#"
#[cfg(test)]
mod phase124_synthetic {
    use super::*;

    #[test]
    fn synthetic_match_executes() {
        assert_eq!(probe('a'), 1);
        assert_eq!(probe('\n'), 2);
        assert_eq!(probe('\''), 3);
        assert_eq!(probe('\\'), 4);
        assert_eq!(probe('z'), 9, "the wildcard arm answers off-literal inputs");
        assert_eq!(probe('0'), 9);
    }
}
"#,
    );
    let stdout = compile_and_run(&rust, "synthetic-match");
    assert!(
        stdout.contains("synthetic_match_executes") && stdout.contains("test result: ok"),
        "the synthetic match executes exactly:\n{stdout}"
    );
}

// ── NF-116-2, globally ───────────────────────────────

fn spec_with_require(input: &str, output: &str, require: &str) -> FunctionSpec {
    hand_spec(&format!(
        "Probe {{ spec probe {{\n    input: {input}\n    output: {output}\n    require: {require}\n    constraint: output == output\n}} }}"
    ))
}

#[test]
fn predicate_precondition_renders_genuinely_with_the_clone_rule() {
    let spec = spec_with_require("xs: List<Int>", "List<Int>", "list_is_sorted(xs)");
    let rust = rust_of(&spec, IRNode::Var("xs".into()));
    assert!(
        rust.contains("debug_assert!(list_is_sorted(xs.clone()), \"Precondition violated\");"),
        "the predicate renders as the call, its List argument cloned at assert position:\n{rust}"
    );
    assert!(
        !rust.contains("debug_assert!(true"),
        "no placeholder stands:\n{rust}"
    );
}

#[test]
fn expr_preconditions_render_genuinely() {
    // A comparison the expression parser carries: an
    // Expr(BinOp) row, rendered by the assert walker.
    let spec = spec_with_require("x: Int", "Int", "x != 0");
    let rust = rust_of(&spec, IRNode::Var("x".into()));
    assert!(
        rust.contains("debug_assert!((x != 0), \"Precondition violated\");"),
        "the Expr row renders genuinely:\n{rust}"
    );
    // A call nested inside the proposition: the clone
    // rule reaches it (the proposition must not consume
    // the function's argument).
    let spec = spec_with_require("xs: List<Int>", "Bool", "len(xs) > 0");
    let rust = rust_of(&spec, IRNode::Literal(IRLiteral::Bool(true)));
    assert!(
        rust.contains("debug_assert!((len(xs.clone()) > 0), \"Precondition violated\");"),
        "the nested call's List argument clones at assert position:\n{rust}"
    );
}

#[test]
fn implies_precondition_renders_genuinely() {
    let spec = spec_with_require("x: Int, y: Int", "Int", "x != 0 implies y != 0");
    let rust = rust_of(&spec, IRNode::Var("y".into()));
    assert!(
        rust.contains("debug_assert!((!((x != 0)) || ((y != 0))), \"Precondition violated\");"),
        "the implication renders as its proposition:\n{rust}"
    );
}

#[test]
fn quantified_precondition_keeps_the_named_fallback() {
    // The ONE shape that keeps the placeholder, by
    // name: a quantified proposition has no value-level
    // rendering a debug_assert could evaluate. The
    // fallback is recorded in rust_constraint_form's
    // family 0 and in the Phase 124 ROAD note — this
    // pin holds it deliberate, not silent.
    let spec = spec_with_require(
        "xs: List<Int>",
        "List<Int>",
        "forall i: Int in xs: i > 0",
    );
    let rust = rust_of(&spec, IRNode::Var("xs".into()));
    assert!(
        rust.contains("debug_assert!(true, \"Precondition violated\");"),
        "the quantified row keeps the named fallback:\n{rust}"
    );
}

/// The standing exhibit end-to-end: `list_binary_search`
/// generated standalone. Its require names
/// `list_is_sorted`; the genuine assert calls it, so
/// the Phase 116 route emits it as a real function in
/// the same file (the precondition-aware used-entry
/// scan) and the file compiles and answers.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn list_binary_search_standalone_file_compiles_and_answers() {
    let spec = morphic::stdlib::prelude_specs()
        .into_iter()
        .find(|s| s.name == "list_binary_search")
        .expect("list_binary_search in prelude");
    let body =
        morphic::stdlib::shipped_body("list_binary_search").expect("list_binary_search ships");
    let mut rust = rust_of(&spec, body);
    assert!(
        rust.contains("debug_assert!(list_is_sorted(xs.clone()), \"Precondition violated\");"),
        "the genuine assert stands:\n{rust}"
    );
    assert!(
        rust.contains("pub fn list_is_sorted("),
        "the precondition's entry is emitted alongside:\n{rust}"
    );
    rust.push_str(
        r#"
#[cfg(test)]
mod phase124_search {
    use super::*;

    #[test]
    fn search_points() {
        assert_eq!(list_binary_search(vec![1, 3, 5, 7, 9], 5), Some(2));
        assert_eq!(list_binary_search(vec![1, 3, 5, 7, 9], 4), None);
        assert_eq!(list_binary_search(vec![], 1), None);
        assert_eq!(list_binary_search(vec![2, 2, 2], 2), Some(0));
    }
}
"#,
    );
    let stdout = compile_and_run(&rust, "binary-search");
    assert!(
        stdout.contains("search_points") && stdout.contains("test result: ok"),
        "the standalone file compiles and answers:\n{stdout}"
    );
}
