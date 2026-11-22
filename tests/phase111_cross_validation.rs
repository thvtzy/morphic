//! Phase 111 cross-validation (gate clause 3's discipline,
//! STDLIB_SCOPE §3): every shipped Int/Char entry is
//! executed against a hand-written Rust mirror over a
//! battery far wider than its in-spec tests — pinned
//! points must agree 100%; the frozen precondition
//! boundaries (int_abs at the Int minimum, int_clamp on
//! inverted ranges) ride along as spec-silent points and
//! are reported in that class, never as failures.
//!
//! The battery runs through the production executed judge
//! (`eval::run_spec_tests` / `test_outcomes`) on a
//! generated spec carrying the entry's signature (and its
//! `require:` rows where the frozen contract has them);
//! the expected values in that spec are computed by the
//! mirrors below, so body-vs-mirror agreement is what is
//! asserted, point by point.
//!
//! This file also carries the `int_gcd` blocker pin: the
//! one §2.2 entry that does not ship. Its faithful Euclid
//! body (statement form, full preservation invariant)
//! executes 6/6 against its contract, yet the verifier
//! returns Inconclusive — the quantified
//! divisibility-preservation obligations do not discharge
//! — so no crown exists and no body ships. The pin holds
//! that measurement open: if the judge ever closes it,
//! this test fails and the entry's return is a scope
//! amendment, decided in the open.

use morphic::spec::ast::FunctionSpec;
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{IRBinOp, IRLiteral, IRNode};
use morphic::synthesis::eval::{self, TestOutcome};

// ── Hand-written mirrors ─────────────────────────────────

fn m_abs(x: i64) -> Option<i64> {
    x.checked_abs()
}
fn m_sign(x: i64) -> i64 {
    x.signum()
}
fn m_min(a: i64, b: i64) -> i64 {
    a.min(b)
}
fn m_max(a: i64, b: i64) -> i64 {
    a.max(b)
}
fn m_clamp(x: i64, lo: i64, hi: i64) -> i64 {
    x.clamp(lo, hi)
}
fn m_even(x: i64) -> bool {
    x % 2 == 0
}
fn m_odd(x: i64) -> bool {
    x % 2 != 0
}
fn m_digit(c: char) -> bool {
    c.is_ascii_digit()
}
fn m_alpha(c: char) -> bool {
    c.is_ascii_alphabetic()
}
fn m_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}
fn m_upper(c: char) -> bool {
    c.is_ascii_uppercase()
}
fn m_lower(c: char) -> bool {
    c.is_ascii_lowercase()
}
fn m_to_digit(c: char) -> Option<i64> {
    if c.is_ascii_digit() {
        Some(c as i64 - '0' as i64)
    } else {
        None
    }
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

fn fmt_char(c: char) -> String {
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

fn fmt_bool(b: bool) -> String {
    b.to_string()
}

fn fmt_opt(o: Option<i64>) -> String {
    match o {
        Some(n) => format!("Some({})", fmt_int(n)),
        None => "None()".to_string(),
    }
}

/// The battery alphabet: printable ASCII plus the named
/// whitespace escapes, NUL, vertical tab (NOT in the
/// frozen whitespace set — an adversarial pin) and DEL.
fn battery_chars() -> Vec<char> {
    let mut chars: Vec<char> = (32u8..=126).map(char::from).collect();
    chars.extend(['\t', '\n', '\r', '\u{0}', '\u{b}', '\u{7f}']);
    chars
}

// ── The battery runner ───────────────────────────────────

/// Builds a one-spec module carrying the entry's
/// signature (and `require:` rows, when given) with the
/// battery as its tests, then judges the shipped body
/// over it. `pinned` are (input, expected) source pairs
/// whose expectations the mirror computed; `silent` are
/// input sources at precondition boundaries, stated with
/// a placeholder expectation the judge must not reach.
/// Asserts: every pinned point passes, every silent
/// point is SpecSilent, nothing else is.
fn run_battery(
    entry: &str,
    signature: &str,
    require: Option<&str>,
    pinned: Vec<(String, String)>,
    silent: Vec<String>,
) {
    let mut src = String::from("Battery { spec bat {\n");
    src.push_str(signature);
    src.push('\n');
    if let Some(r) = require {
        src.push_str(&format!("    require: {r}\n"));
    }
    for (input, expect) in &pinned {
        src.push_str(&format!("    test {{ input: {input}; expect: {expect} }}\n"));
    }
    for input in &silent {
        src.push_str(&format!("    test {{ input: {input}; expect: 0 }}\n"));
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
    assert_eq!(outcomes.len(), pinned.len() + silent.len(), "{entry}: battery size");
    for (idx, outcome) in outcomes.iter().enumerate() {
        if idx < pinned.len() {
            assert!(
                matches!(outcome, TestOutcome::Passed),
                "{entry}: pinned point {} ({}) must pass, got {:?}",
                idx,
                pinned[idx].0,
                outcome
            );
        } else {
            assert!(
                matches!(outcome, TestOutcome::SpecSilent(_)),
                "{entry}: boundary point {} must be spec-silent, got {:?}",
                idx,
                outcome
            );
        }
    }
    let run = eval::run_spec_tests(&body, spec);
    assert_eq!(
        (run.passed, run.evaluable),
        (pinned.len(), pinned.len()),
        "{entry}: mirror agreement on every pinned point"
    );
    println!(
        "P111XV|{}|pinned={}|silent={}|agree=100%",
        entry,
        pinned.len(),
        silent.len()
    );
}

// ── Int batteries ────────────────────────────────────────

#[test]
fn xv_int_abs() {
    let pinned: Vec<(String, String)> = [
        0, 1, -1, 2, -2, 7, -7, 42, -42, 1_000_000, -1_000_000,
        i64::MAX, i64::MAX - 1, i64::MIN + 1, i64::MIN + 2, -i64::MAX,
    ]
    .iter()
    .map(|&x| (fmt_int(x), fmt_int(m_abs(x).expect("pinned points are in domain"))))
    .collect();
    run_battery(
        "int_abs",
        "    input: x: Int\n    output: Int",
        Some("x != (0 - 9223372036854775807 - 1)"),
        pinned,
        vec![fmt_int(i64::MIN)],
    );
}

#[test]
fn xv_int_sign() {
    let pinned: Vec<(String, String)> = [
        0, 1, -1, 5, -5, 42, -42, i64::MAX, i64::MIN, i64::MAX - 1, i64::MIN + 1,
    ]
    .iter()
    .map(|&x| (fmt_int(x), fmt_int(m_sign(x))))
    .collect();
    run_battery("int_sign", "    input: x: Int\n    output: Int", None, pinned, vec![]);
}

#[test]
fn xv_int_min_max() {
    let pairs: [(i64, i64); 16] = [
        (3, 7), (7, 3), (4, 4), (0, 0), (-2, -9), (-9, -2),
        (i64::MIN, 0), (0, i64::MIN), (i64::MAX, 0), (0, i64::MAX),
        (i64::MIN, i64::MAX), (i64::MAX, i64::MIN),
        (i64::MAX, i64::MAX - 1), (i64::MIN, i64::MIN + 1),
        (-1, 1), (1, -1),
    ];
    let pin = |f: fn(i64, i64) -> i64| -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|&(a, b)| (format!("({}, {})", fmt_int(a), fmt_int(b)), fmt_int(f(a, b))))
            .collect()
    };
    run_battery(
        "int_min",
        "    input: a: Int, b: Int\n    output: Int",
        None,
        pin(m_min),
        vec![],
    );
    run_battery(
        "int_max",
        "    input: a: Int, b: Int\n    output: Int",
        None,
        pin(m_max),
        vec![],
    );
}

#[test]
fn xv_int_clamp() {
    let triples: [(i64, i64, i64); 13] = [
        (5, 0, 10), (-3, 0, 10), (99, 0, 10), (0, 0, 10), (10, 0, 10),
        (5, 5, 5), (-7, -10, -5), (-12, -10, -5), (0, -5, 5),
        (i64::MAX, 0, i64::MAX), (i64::MIN, 0, i64::MAX),
        (7, 7, 7), (-1, 0, 0),
    ];
    let pinned: Vec<(String, String)> = triples
        .iter()
        .map(|&(x, lo, hi)| {
            (
                format!("({}, {}, {})", fmt_int(x), fmt_int(lo), fmt_int(hi)),
                fmt_int(m_clamp(x, lo, hi)),
            )
        })
        .collect();
    run_battery(
        "int_clamp",
        "    input: x: Int, lo: Int, hi: Int\n    output: Int",
        Some("lo <= hi"),
        pinned,
        vec!["(5, 10, 0)".to_string(), "(0, 1, (0 - 1))".to_string()],
    );
}

#[test]
fn xv_int_is_even_odd() {
    let inputs: [i64; 15] = [
        0, 1, 2, 3, 4, -1, -2, -3, -4, 42, -42,
        i64::MAX, i64::MIN, i64::MAX - 1, i64::MIN + 1,
    ];
    // The frozen cross-property (§2.2): odd is even's
    // exact complement, negatives included — asserted on
    // the mirrors; both bodies inherit it by agreeing
    // with the mirrors on every point.
    for &x in &inputs {
        assert_eq!(m_odd(x), !m_even(x), "mirror complement at {x}");
    }
    let pin = |f: fn(i64) -> bool| -> Vec<(String, String)> {
        inputs.iter().map(|&x| (fmt_int(x), fmt_bool(f(x)))).collect()
    };
    run_battery(
        "int_is_even",
        "    input: x: Int\n    output: Bool",
        None,
        pin(m_even),
        vec![],
    );
    run_battery(
        "int_is_odd",
        "    input: x: Int\n    output: Bool",
        None,
        pin(m_odd),
        vec![],
    );
}

// ── Char batteries (the full battery alphabet) ───────────

fn char_battery(entry: &str, mirror: fn(char) -> String) {
    let pinned: Vec<(String, String)> = battery_chars()
        .iter()
        .map(|&c| (fmt_char(c), mirror(c)))
        .collect();
    run_battery(entry, "    input: c: Char\n    output: Bool", None, pinned, vec![]);
}

#[test]
fn xv_char_classifiers() {
    char_battery("char_is_digit", |c| fmt_bool(m_digit(c)));
    char_battery("char_is_alpha", |c| fmt_bool(m_alpha(c)));
    char_battery("char_is_whitespace", |c| fmt_bool(m_ws(c)));
    char_battery("char_is_uppercase", |c| fmt_bool(m_upper(c)));
    char_battery("char_is_lowercase", |c| fmt_bool(m_lower(c)));
}

#[test]
fn xv_char_to_digit() {
    let pinned: Vec<(String, String)> = battery_chars()
        .iter()
        .map(|&c| (fmt_char(c), fmt_opt(m_to_digit(c))))
        .collect();
    run_battery(
        "char_to_digit",
        "    input: c: Char\n    output: Option<Int>",
        None,
        pinned,
        vec![],
    );
}

// ── The int_gcd blocker pin ──────────────────────────────

/// The frozen §2.2 contract for `int_gcd`, as authored in
/// Phase 111 (NOT shipped — it lives here and in the ROAD
/// note, not in stdlib/int.morph). The greatest-ness row
/// is a forall: the parser discards the `range(...)`
/// collection, so the row quantifies over every Int; its
/// body is stated so the unbounded reading is the true
/// one (negative divisors divide exactly when their
/// absolute value does, oversize divisors divide nothing
/// nonzero, and the single-zero cases are pinned by the
/// boundary rows, which is what the `a == 0 || b == 0`
/// leaves record).
const GCD_SPEC: &str = r#"Probe { spec int_gcd {
    input: a: Int, b: Int
    output: Int
    require: a != (0 - 9223372036854775807 - 1)
    require: b != (0 - 9223372036854775807 - 1)
    constraint: output >= 0
    constraint: a != 0 || b != 0 || output == 0
    constraint: b != 0 || a == 0 || output == int_abs(a)
    constraint: a != 0 || b == 0 || output == int_abs(b)
    constraint: output == 0 || (a % output == 0 && b % output == 0)
    constraint: forall d: Int in range(int_abs(a) + int_abs(b) + 1): d == 0 || d <= output || a % d != 0 || b % d != 0 || a == 0 || b == 0
    test { input: (12, 18); expect: 6 }
    test { input: (0, 0); expect: 0 }
    test { input: (7, 0); expect: 7 }
    test { input: (0, (0 - 5)); expect: 5 }
    test { input: ((0 - 12), 18); expect: 6 }
    test { input: (17, 5); expect: 1 }
} }"#;

fn var(name: &str) -> IRNode {
    IRNode::Var(name.into())
}

fn int(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}

fn bin(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
    IRNode::BinOp(op, Box::new(l), Box::new(r))
}

fn abs_of(v: &str) -> IRNode {
    IRNode::If {
        condition: Box::new(bin(IRBinOp::Lt, var(v), int(0))),
        then_branch: Box::new(bin(IRBinOp::Sub, int(0), var(v))),
        else_branch: Box::new(var(v)),
    }
}

/// Euclid's body in statement form, carrying the full
/// preservation invariant: non-negativity, the common-
/// divisor sets of (a, b) and (x, y) equal in both
/// directions (four forall rows), and the sum bound.
fn euclid_body() -> IRNode {
    let invariant = "x >= 0; y >= 0; \
        forall d: Int in range(int_abs(a) + int_abs(b) + 1): \
        d == 0 || a % d != 0 || b % d != 0 || x % d == 0; \
        forall d: Int in range(int_abs(a) + int_abs(b) + 1): \
        d == 0 || a % d != 0 || b % d != 0 || y % d == 0; \
        forall d: Int in range(int_abs(a) + int_abs(b) + 1): \
        d == 0 || x % d != 0 || y % d != 0 || a % d == 0; \
        forall d: Int in range(int_abs(a) + int_abs(b) + 1): \
        d == 0 || x % d != 0 || y % d != 0 || b % d == 0; \
        x + y <= int_abs(a) + int_abs(b)";
    IRNode::Block(vec![
        IRNode::Alloc {
            name: "x".into(),
            ty: morphic::spec::ast::TypeRef::Int,
            initial: Box::new(abs_of("a")),
        },
        IRNode::Alloc {
            name: "y".into(),
            ty: morphic::spec::ast::TypeRef::Int,
            initial: Box::new(abs_of("b")),
        },
        IRNode::While {
            condition: Box::new(bin(IRBinOp::Neq, var("y"), int(0))),
            body: Box::new(IRNode::Block(vec![
                IRNode::Alloc {
                    name: "t".into(),
                    ty: morphic::spec::ast::TypeRef::Int,
                    initial: Box::new(var("y")),
                },
                IRNode::Assign {
                    target: Box::new(var("y")),
                    value: Box::new(bin(IRBinOp::Mod, var("x"), var("y"))),
                },
                IRNode::Assign {
                    target: Box::new(var("x")),
                    value: Box::new(var("t")),
                },
            ])),
            invariant: Some(invariant.to_string()),
        },
        IRNode::Return(Box::new(var("x"))),
    ])
}

#[test]
fn int_gcd_is_a_named_blocker_not_a_shipped_entry() {
    // The entry is absent from the shipped library: no
    // spec in the prelude, no shipped body.
    assert!(
        stdlib::prelude_specs().iter().all(|s| s.name != "int_gcd"),
        "int_gcd must not enter the prelude while it is blocked"
    );
    assert!(stdlib::shipped_body("int_gcd").is_none());

    // The faithful body executes its whole contract
    // battery — the gap is not in the algorithm or the
    // interpreter.
    let module = parser::parse(GCD_SPEC).expect("gcd contract parses");
    let checked = typeck::check(module).expect("gcd contract checks");
    let functions: Vec<FunctionSpec> = checked.spec.functions.clone();
    let spec = &functions[0];
    let run = eval::run_spec_tests(&euclid_body(), spec);
    assert_eq!((run.passed, run.evaluable), (6, 6), "Euclid executes 6/6");

    // And the verifier does not crown it: with the full
    // preservation invariant the verdict is Inconclusive
    // (never refuted — a plainer invariant IS refuted,
    // so the preservation facts are load-bearing; the
    // quantified mod obligations simply do not
    // discharge). Measured in Phase 111; see the ROAD
    // note for the full isolation series.
    #[cfg(feature = "z3-support")]
    {
        use morphic::synthesis::engine::{
            CandidateImplementation, Provenance, ScoreBreakdown,
        };
        use morphic::verify::verdict::VerificationStatus;
        use morphic::verify::verifier::Verifier;
        let candidate = CandidateImplementation {
            id: 111,
            body: euclid_body(),
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
        let status = Verifier::with_specs(&functions).verify_one(&candidate, spec).status;
        assert_eq!(
            status,
            VerificationStatus::Inconclusive,
            "the gcd blocker measurement: Inconclusive, not Verified"
        );
    }
}
