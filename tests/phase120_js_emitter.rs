//! Phase 120 (1.1 Embeddable, EMBED_SCOPE D5a): the
//! JavaScript emitter completed for the embeddable
//! subset. The Phase 118 probes pinned generated JS
//! that does not run — no tail renderer (`int_abs`
//! answered `undefined`), `%`/`<=` rendered as a bare
//! `?` (SyntaxError), Match without an arm, Alloc/Assign
//! dropped (`list_sum` threw ReferenceError),
//! constructors/Index without representation, Int
//! literals as bare JS numbers (the 2^53 cliff D3a
//! rejected). These pins hold the completed emitter to
//! the freeze's exit condition:
//!
//! * every one of the 29 shipped bodies generates JS
//!   that parses and answers its frozen cross-validation
//!   battery (the Phase 111/112/115 points, in their
//!   §3 JS-representable forms: BigInt for Int, `null`
//!   for None, discriminated-union objects for Result)
//!   at 100% agreement, executed under node;
//! * each arm of the audit §4 inventory is pinned per
//!   node (tail renderer; Let/Alloc/Assign/While/Loop/
//!   Break/For/Match statements; If/Match/Index/
//!   constructors/list literals in expression position;
//!   Mod/Le/Ge and the BigInt Int semantics of D3a);
//! * entry calls render as plain calls to the entry's
//!   export name (the Phase 116 reconciliation: Phase
//!   121's module assembly satisfies them — proven here
//!   by hand-assembling a caller with its entry and
//!   running it), while a call to a non-entry,
//!   non-builtin name keeps its standing fallback;
//! * plausible-winner bodies in the same node
//!   vocabulary (Euclid's While body, a valued-Break
//!   Loop, a statement Match, the lexeme Append shape,
//!   a counting For) run correctly under node.
//!
//! Configuration note (the standing one): pins that
//! read the NATIVE prelude are gated
//! `#[cfg(not(feature = "wasm-playground"))]`; the
//! hand-body pins build their specs from local parsed
//! modules and run in every configuration.

use morphic::codegen;
use morphic::spec::ast::FunctionSpec;
use morphic::spec::parser;
#[cfg(not(feature = "wasm-playground"))]
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, IRPattern, IRUnaryOp, Provenance,
    ScoreBreakdown,
};

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 120,
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

#[cfg(not(feature = "wasm-playground"))]
fn entry(name: &str) -> (FunctionSpec, IRNode) {
    let spec = stdlib::prelude_specs()
        .into_iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("{name} in prelude"));
    let body = stdlib::shipped_body(name).unwrap_or_else(|| panic!("{name} ships a body"));
    (spec, body)
}

#[cfg(not(feature = "wasm-playground"))]
fn js_of_entry(name: &str) -> String {
    let (spec, body) = entry(name);
    codegen::generate(&candidate(&spec, body), "javascript")
        .unwrap_or_else(|e| panic!("{name} JS generation: {e}"))
}

/// A local spec from a one-module source (configuration-
/// independent: no prelude involved).
fn hand_spec(source: &str, name: &str) -> FunctionSpec {
    let module = parser::parse(source).expect("hand module parses");
    module
        .functions
        .into_iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("spec {name} in hand module"))
}

fn js_of_hand(source: &str, name: &str, body: IRNode) -> String {
    let spec = hand_spec(source, name);
    codegen::generate(&candidate(&spec, body), "javascript").expect("hand JS generation")
}

fn ret(b: IRNode) -> IRNode {
    IRNode::Return(Box::new(b))
}
fn var(n: &str) -> IRNode {
    IRNode::Var(n.into())
}
fn lit(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}
fn bin(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
    IRNode::BinOp(op, Box::new(l), Box::new(r))
}
fn call(f: &str, args: Vec<IRNode>) -> IRNode {
    IRNode::Call {
        function: f.into(),
        args,
    }
}

const SPEC_F: &str = "Probe {\nspec f {\n    input: x: Int\n    output: Int\n    constraint: output == x\n    test \"t\" { input: 1 expect: 1 }\n}\n}";

// ── The node harness ─────────────────────────────────────

/// Write `module_js` and `driver_js` into a fresh temp
/// dir, run the driver under node, and return stdout.
/// Any failure (write, spawn, non-zero exit) panics
/// with the captured streams — a generated body that
/// does not parse or run is a loud test failure, which
/// is the point.
fn run_node(label: &str, module_js: &str, driver_js: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase120-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("mod.mjs"), module_js).unwrap();
    std::fs::write(dir.join("driver.mjs"), driver_js).unwrap();
    let out = std::process::Command::new("node")
        .arg(dir.join("driver.mjs"))
        .output()
        .unwrap_or_else(|e| panic!("{label}: node must run (node v24 on PATH): {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "{label}: node driver failed\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}\n--- module ---\n{module_js}"
    );
    stdout
}

const EQ_FN: &str = r#"
function show(v) {
    return JSON.stringify(v, (k, x) => typeof x === "bigint" ? x.toString() + "n" : x);
}
function eq(a, b) {
    if (typeof a === "bigint" || typeof b === "bigint") return a === b;
    if (a === null || b === null) return a === b;
    if (Array.isArray(a) || Array.isArray(b)) {
        return Array.isArray(a) && Array.isArray(b) && a.length === b.length
            && a.every((v, i) => eq(v, b[i]));
    }
    if (typeof a === "object" && typeof b === "object") {
        const ka = Object.keys(a), kb = Object.keys(b);
        return ka.length === kb.length && ka.every(k => eq(a[k], b[k]));
    }
    return a === b;
}
"#;

/// The standard case-loop driver: `cases` is JS text of
/// the form `[{ args: [...], expect: ... }, ...]`.
/// Prints `RESULT <total> <fails>`; the caller asserts
/// on the exact line.
fn cases_driver(fn_name: &str, cases_js: &str) -> String {
    format!(
        "import {{ {fn_name} }} from \"./mod.mjs\";\n{EQ_FN}\nconst cases = {cases_js};\nlet fails = 0;\nfor (const [i, c] of cases.entries()) {{\n    let got;\n    try {{ got = {fn_name}(...c.args); }} catch (e) {{ console.log(`case ${{i}} threw: ${{e}}`); fails++; continue; }}\n    if (!eq(got, c.expect)) {{ console.log(`case ${{i}}: got ${{show(got)}} want ${{show(c.expect)}}`); fails++; }}\n}}\nconsole.log(`RESULT ${{cases.length}} ${{fails}}`);\nif (fails) process.exit(1);\n"
    )
}

// ── Battery values in their §3 JS forms ──────────────────

enum J {
    Int(i64),
    Bool(bool),
    Chr(char),
    Str(String),
    List(Vec<J>),
    Opt(Option<Box<J>>),
    Res(Result<Box<J>, Box<J>>),
}

fn js_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 || (c as u32) == 0x7f => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn js_val(j: &J) -> String {
    match j {
        J::Int(n) => format!("{n}n"),
        J::Bool(b) => b.to_string(),
        J::Chr(c) => js_str(&c.to_string()),
        J::Str(s) => js_str(s),
        J::List(items) => {
            let parts: Vec<String> = items.iter().map(js_val).collect();
            format!("[{}]", parts.join(", "))
        }
        J::Opt(None) => "null".into(),
        J::Opt(Some(v)) => js_val(v),
        J::Res(Ok(v)) => format!("{{ tag: \"Ok\", value: {} }}", js_val(v)),
        J::Res(Err(e)) => format!("{{ tag: \"Err\", error: {} }}", js_val(e)),
    }
}

fn case(args: Vec<J>, expect: J) -> String {
    let parts: Vec<String> = args.iter().map(js_val).collect();
    format!("{{ args: [{}], expect: {} }}", parts.join(", "), js_val(&expect))
}

fn run_battery(label: &str, name: &str, module_js: &str, cases: Vec<String>) {
    let total = cases.len();
    let driver = cases_driver(name, &format!("[{}]", cases.join(", ")));
    let out = run_node(label, module_js, &driver);
    assert!(
        out.contains(&format!("RESULT {total} 0")),
        "{label}: battery must agree 100% ({total} points):\n{out}"
    );
    println!("P120JS|{label}|points={total}|agree=100%");
}

// ── Mirrors (transcribed from the Phase 111/112/115
// cross-validation files; the same std semantics) ───

fn m_abs(x: i64) -> i64 {
    x.checked_abs().expect("pinned points are in domain")
}
fn m_sign(x: i64) -> i64 {
    x.signum()
}
fn m_clamp(x: i64, lo: i64, hi: i64) -> i64 {
    x.clamp(lo, hi)
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

fn battery_chars() -> Vec<char> {
    let mut chars: Vec<char> = (32u8..=126).map(char::from).collect();
    chars.extend(['\t', '\n', '\r', '\u{0}', '\u{b}', '\u{7f}']);
    chars
}

fn lists() -> Vec<Vec<i64>> {
    vec![
        vec![],
        vec![0],
        vec![1],
        vec![-1],
        vec![5],
        vec![1, 2, 3],
        vec![3, 2, 1],
        vec![2, 2, 2],
        vec![1, 2, 2, 3],
        vec![5, 4, 3, 2, 1],
        vec![3, 1, 4, 1, 5, 9, 2, 6],
        vec![10, -3, 7, -7],
        vec![i64::MAX],
        vec![i64::MIN],
        vec![i64::MAX, i64::MIN],
    ]
}

fn jlist(xs: &[i64]) -> J {
    J::List(xs.iter().map(|n| J::Int(*n)).collect())
}
fn jopt(o: Option<i64>) -> J {
    J::Opt(o.map(|n| Box::new(J::Int(n))))
}

// ── The 29-body node battery (the freeze's exit
// condition), grouped by family ──────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_int_entries_answer_their_batteries_under_node() {
    // int_abs: 16 pinned (the MIN point is spec-silent
    // per the frozen precondition — out of domain, R3).
    let pts = [
        0i64, 1, -1, 2, -2, 7, -7, 42, -42, 1_000_000, -1_000_000, i64::MAX, i64::MAX - 1,
        i64::MIN + 1, i64::MIN + 2, -i64::MAX,
    ];
    let cases: Vec<String> = pts
        .iter()
        .map(|x| case(vec![J::Int(*x)], J::Int(m_abs(*x))))
        .collect();
    run_battery("int-abs", "int_abs", &js_of_entry("int_abs"), cases);

    let pts = [0i64, 1, -1, 5, -5, 42, -42, i64::MAX, i64::MIN, i64::MAX - 1, i64::MIN + 1];
    let cases: Vec<String> = pts
        .iter()
        .map(|x| case(vec![J::Int(*x)], J::Int(m_sign(*x))))
        .collect();
    run_battery("int-sign", "int_sign", &js_of_entry("int_sign"), cases);

    let pairs = [
        (3i64, 7i64),
        (7, 3),
        (4, 4),
        (0, 0),
        (-2, -9),
        (-9, -2),
        (i64::MIN, 0),
        (0, i64::MIN),
        (i64::MAX, 0),
        (0, i64::MAX),
        (i64::MIN, i64::MAX),
        (i64::MAX, i64::MIN),
        (i64::MAX, i64::MAX - 1),
        (i64::MIN, i64::MIN + 1),
        (-1, 1),
        (1, -1),
    ];
    let cases: Vec<String> = pairs
        .iter()
        .map(|(a, b)| case(vec![J::Int(*a), J::Int(*b)], J::Int((*a).min(*b))))
        .collect();
    run_battery("int-min", "int_min", &js_of_entry("int_min"), cases);
    let cases: Vec<String> = pairs
        .iter()
        .map(|(a, b)| case(vec![J::Int(*a), J::Int(*b)], J::Int((*a).max(*b))))
        .collect();
    run_battery("int-max", "int_max", &js_of_entry("int_max"), cases);

    // int_clamp: 13 pinned (the two inverted-range
    // points are spec-silent — out of domain, R3).
    let triples = [
        (5i64, 0i64, 10i64),
        (-3, 0, 10),
        (99, 0, 10),
        (0, 0, 10),
        (10, 0, 10),
        (5, 5, 5),
        (-7, -10, -5),
        (-12, -10, -5),
        (0, -5, 5),
        (i64::MAX, 0, i64::MAX),
        (i64::MIN, 0, i64::MAX),
        (7, 7, 7),
        (-1, 0, 0),
    ];
    let cases: Vec<String> = triples
        .iter()
        .map(|(x, lo, hi)| {
            case(
                vec![J::Int(*x), J::Int(*lo), J::Int(*hi)],
                J::Int(m_clamp(*x, *lo, *hi)),
            )
        })
        .collect();
    run_battery("int-clamp", "int_clamp", &js_of_entry("int_clamp"), cases);

    let pts = [
        0i64, 1, 2, 3, 4, -1, -2, -3, -4, 42, -42, i64::MAX, i64::MIN, i64::MAX - 1,
        i64::MIN + 1,
    ];
    let cases: Vec<String> = pts
        .iter()
        .map(|x| case(vec![J::Int(*x)], J::Bool(x % 2 == 0)))
        .collect();
    run_battery("int-is-even", "int_is_even", &js_of_entry("int_is_even"), cases);
    let cases: Vec<String> = pts
        .iter()
        .map(|x| case(vec![J::Int(*x)], J::Bool(x % 2 != 0)))
        .collect();
    run_battery("int-is-odd", "int_is_odd", &js_of_entry("int_is_odd"), cases);
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_char_entries_answer_their_batteries_under_node() {
    let chars = battery_chars();
    assert_eq!(chars.len(), 101, "the frozen 101-point char sweep");
    let fns: Vec<(&str, fn(char) -> bool)> = vec![
        ("char_is_digit", m_digit),
        ("char_is_alpha", m_alpha),
        ("char_is_whitespace", m_ws),
        ("char_is_uppercase", m_upper),
        ("char_is_lowercase", m_lower),
    ];
    for (name, mirror) in fns {
        let cases: Vec<String> = chars
            .iter()
            .map(|c| case(vec![J::Chr(*c)], J::Bool(mirror(*c))))
            .collect();
        run_battery(name, name, &js_of_entry(name), cases);
    }
    let cases: Vec<String> = chars
        .iter()
        .map(|c| case(vec![J::Chr(*c)], jopt(m_to_digit(*c))))
        .collect();
    run_battery("char-to-digit", "char_to_digit", &js_of_entry("char_to_digit"), cases);
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_string_result_option_entries_answer_their_batteries_under_node() {
    let strs = [
        "", "a", "z", "ab", "hello", "hello world", " ", " ", "0", "false", "\n", "a\tb", "é",
        "🦀",
    ]
    .into_iter()
    .map(String::from)
    .chain([std::iter::repeat_n('a', 64).collect::<String>()])
    .collect::<Vec<_>>();
    assert_eq!(strs.len(), 15);
    let cases: Vec<String> = strs
        .iter()
        .map(|s| case(vec![J::Str(s.clone())], J::Bool(s.is_empty())))
        .collect();
    run_battery("string-is-empty", "string_is_empty", &js_of_entry("string_is_empty"), cases);

    // The battery, in value form (the mirror is
    // Result::unwrap_or over the same table).
    let table: Vec<(Result<i64, String>, i64)> = {
        let rs: Vec<Result<i64, String>> = vec![
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
        let ds = [0i64, 3, -7, i64::MAX];
        rs.iter()
            .flat_map(|r| ds.iter().map(move |d| (r.clone(), *d)))
            .collect()
    };
    let cases: Vec<String> = table
        .iter()
        .map(|(r, d)| {
            let rj = match r {
                Ok(v) => J::Res(Ok(Box::new(J::Int(*v)))),
                Err(e) => J::Res(Err(Box::new(J::Str(e.clone())))),
            };
            case(
                vec![rj, J::Int(*d)],
                J::Int(r.clone().unwrap_or(*d)),
            )
        })
        .collect();
    run_battery("result-unwrap-or", "result_unwrap_or", &js_of_entry("result_unwrap_or"), cases);

    let opts: Vec<Option<i64>> = vec![Some(0), Some(1), Some(-1), Some(42), Some(i64::MAX), Some(i64::MIN), None];
    let cases: Vec<String> = opts
        .iter()
        .flat_map(|o| [0i64, 5, -3, i64::MIN].iter().map(move |d| (o, *d)))
        .map(|(o, d)| case(vec![jopt(*o), J::Int(d)], J::Int(o.unwrap_or(d))))
        .collect();
    run_battery("option-unwrap-or", "option_unwrap_or", &js_of_entry("option_unwrap_or"), cases);

    let cases_in: Vec<(Option<i64>, &str)> = vec![
        (Some(0), ""),
        (Some(1), "x"),
        (Some(-1), "missing"),
        (Some(i64::MAX), "e"),
        (Some(i64::MIN), "unused"),
        (None, ""),
        (None, "missing"),
        (None, "a longer error message"),
        (None, "with \"quotes\" inside"),
    ];
    let cases: Vec<String> = cases_in
        .iter()
        .map(|(o, e)| {
            let expect = match o {
                Some(v) => J::Res(Ok(Box::new(J::Int(*v)))),
                None => J::Res(Err(Box::new(J::Str((*e).into())))),
            };
            case(vec![jopt(*o), J::Str((*e).into())], expect)
        })
        .collect();
    run_battery("option-ok-or", "option_ok_or", &js_of_entry("option_ok_or"), cases);
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_list_entries_answer_their_batteries_under_node() {
    let all = lists();
    let cases: Vec<String> = all
        .iter()
        .map(|xs| case(vec![jlist(xs)], J::Bool(xs.is_empty())))
        .collect();
    run_battery("list-is-empty", "list_is_empty", &js_of_entry("list_is_empty"), cases);

    let cases: Vec<String> = all
        .iter()
        .map(|xs| {
            let mut rev = xs.clone();
            rev.reverse();
            case(vec![jlist(xs)], jlist(&rev))
        })
        .collect();
    run_battery("list-reverse", "list_reverse", &js_of_entry("list_reverse"), cases);

    let cases: Vec<String> = all
        .iter()
        .flat_map(|xs| [0i64, 2, -1, 9, i64::MAX].iter().map(move |t| (xs, *t)))
        .map(|(xs, t)| case(vec![jlist(xs), J::Int(t)], J::Bool(xs.contains(&t))))
        .collect();
    run_battery("list-contains", "list_contains", &js_of_entry("list_contains"), cases);

    let cases: Vec<String> = all
        .iter()
        .map(|xs| case(vec![jlist(xs)], J::Int(xs.iter().sum())))
        .collect();
    run_battery("list-sum", "list_sum", &js_of_entry("list_sum"), cases);

    let prod_cases: Vec<Vec<i64>> = vec![
        vec![],
        vec![5],
        vec![2, 3, 4],
        vec![0, i64::MAX],
        vec![i64::MAX],
        vec![i64::MIN],
        vec![i64::MAX, 1],
        vec![i64::MIN, 1],
        vec![1, 1, 1],
        vec![-2, 3],
        vec![-1, -1],
        vec![7, 0, 9],
        vec![3, 1, 4, 1, 5],
    ];
    let cases: Vec<String> = prod_cases
        .iter()
        .map(|xs| case(vec![jlist(xs)], J::Int(xs.iter().product())))
        .collect();
    run_battery("list-product", "list_product", &js_of_entry("list_product"), cases);

    let cases: Vec<String> = all
        .iter()
        .map(|xs| case(vec![jlist(xs)], jopt(xs.iter().min().copied())))
        .collect();
    run_battery("list-min", "list_min", &js_of_entry("list_min"), cases);
    let cases: Vec<String> = all
        .iter()
        .map(|xs| case(vec![jlist(xs)], jopt(xs.iter().max().copied())))
        .collect();
    run_battery("list-max", "list_max", &js_of_entry("list_max"), cases);

    let take_lists: Vec<Vec<i64>> = vec![vec![], vec![5], vec![1, 2, 3], vec![4, 3, 2, 1]];
    let ns = [-3i64, 0, 1, 2, 3, 5, 99];
    let cases: Vec<String> = take_lists
        .iter()
        .flat_map(|xs| ns.iter().map(move |n| (xs, *n)))
        .map(|(xs, n)| {
            let k = n.clamp(0, xs.len() as i64) as usize;
            case(
                vec![jlist(xs), J::Int(n)],
                jlist(&xs[..k].to_vec()),
            )
        })
        .collect();
    run_battery("list-take", "list_take", &js_of_entry("list_take"), cases);
    let cases: Vec<String> = take_lists
        .iter()
        .flat_map(|xs| ns.iter().map(move |n| (xs, *n)))
        .map(|(xs, n)| {
            let k = n.clamp(0, xs.len() as i64) as usize;
            case(
                vec![jlist(xs), J::Int(n)],
                jlist(&xs[k..].to_vec()),
            )
        })
        .collect();
    run_battery("list-drop", "list_drop", &js_of_entry("list_drop"), cases);

    let cases: Vec<String> = all
        .iter()
        .map(|xs| {
            case(
                vec![jlist(xs)],
                J::Bool(xs.windows(2).all(|w| w[0] <= w[1])),
            )
        })
        .collect();
    run_battery("list-is-sorted", "list_is_sorted", &js_of_entry("list_is_sorted"), cases);

    let cases: Vec<String> = all
        .iter()
        .map(|xs| {
            let mut sorted = xs.clone();
            sorted.sort_unstable();
            case(vec![jlist(xs)], jlist(&sorted))
        })
        .collect();
    run_battery("list-sort", "list_sort", &js_of_entry("list_sort"), cases);
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_binary_search_answers_its_property_battery_under_node() {
    // The frozen battery is a PROPERTY (Phase 115): the
    // result is None iff the target is absent; Some(i)
    // only with xs[i] == target; unique-target points
    // pin the index exactly (64), duplicate-target
    // points are index-silent (3). 67 points total.
    let sorted_lists: Vec<Vec<i64>> = vec![
        vec![],
        vec![5],
        vec![1, 3, 5, 7, 9],
        vec![-5, -1, 2, 8],
        vec![2, 2, 2],
        vec![1, 2, 2, 3],
        vec![1, 1, 2, 3, 5, 8, 13],
        vec![i64::MIN, 0, i64::MAX],
    ];
    let mut case_rows: Vec<String> = Vec::new();
    let (mut total, mut exact, mut silent) = (0usize, 0usize, 0usize);
    for xs in &sorted_lists {
        let mut targets: Vec<i64> = xs.clone();
        targets.extend([-1, 0, 4, 100, i64::MIN, i64::MAX]);
        targets.sort_unstable();
        targets.dedup();
        for x in targets {
            let occ = xs.iter().filter(|v| **v == x).count();
            let unique = if occ == 1 {
                xs.iter().position(|v| *v == x).unwrap() as i64
            } else {
                -1
            };
            total += 1;
            if occ > 1 {
                silent += 1;
            } else {
                exact += 1;
            }
            case_rows.push(format!(
                "{{ xs: {}, x: {}n, occ: {}, unique: {}n }}",
                js_val(&jlist(xs)),
                x,
                occ,
                unique
            ));
        }
    }
    assert_eq!(
        (total, exact, silent),
        (67, 64, 3),
        "transcription of the frozen battery shape"
    );
    let driver = format!(
        "import {{ list_binary_search }} from \"./mod.mjs\";\nconst cases = [{}];\nlet fails = 0;\nfor (const [i, c] of cases.entries()) {{\n    const got = list_binary_search(c.xs, c.x);\n    if (c.occ === 0) {{\n        if (got !== null) {{ console.log(`case ${{i}}: want null, got ${{got}}`); fails++; }}\n    }} else if (typeof got !== \"bigint\" || got < 0n || got >= BigInt(c.xs.length) || c.xs[Number(got)] !== c.x) {{\n        console.log(`case ${{i}}: bad index ${{got}}`); fails++;\n    }} else if (c.occ === 1 && got !== c.unique) {{\n        console.log(`case ${{i}}: want unique index ${{c.unique}}, got ${{got}}`); fails++;\n    }}\n}}\nconsole.log(`RESULT ${{cases.length}} ${{fails}}`);\nif (fails) process.exit(1);\n",
        case_rows.join(", ")
    );
    let out = run_node("binary-search", &js_of_entry("list_binary_search"), &driver);
    assert!(
        out.contains("RESULT 67 0"),
        "binary_search property battery must hold at all 67 points:\n{out}"
    );
    println!("P120JS|list_binary_search|points=67|property holds (64 exact + 3 index-silent)");
}

// ── Per-arm pins (the audit §4 inventory, node by node) ──

#[test]
fn js_tail_renderer_returns_a_bare_expression_body() {
    // The Phase 118 headline gap: no Return node, no
    // `return` — the whole body IS the tail.
    let js = js_of_hand(SPEC_F, "f", bin(IRBinOp::Add, var("x"), lit(1)));
    assert!(js.contains("return (x + 1n);"), "tail renders a return:\n{js}");
    assert!(!js.contains("// unsupported"), "no unsupported arms:\n{js}");
}

#[test]
fn js_operators_mod_le_ge_render_real_operators() {
    // Phase 118: Mod/Le/Ge fell to a bare `?` — a
    // SyntaxError, unparseable. D3a: BigInt operands.
    let js = js_of_hand(SPEC_F, "f", ret(bin(IRBinOp::Mod, var("x"), lit(2))));
    assert!(js.contains("return (x % 2n);"), "{js}");
    let js = js_of_hand(SPEC_F, "f", ret(bin(IRBinOp::Le, var("x"), lit(2))));
    assert!(js.contains("return (x <= 2n);"), "{js}");
    let js = js_of_hand(SPEC_F, "f", ret(bin(IRBinOp::Ge, var("x"), lit(2))));
    assert!(js.contains("return (x >= 2n);"), "{js}");
    let js = js_of_hand(SPEC_F, "f", ret(bin(IRBinOp::Div, var("x"), lit(2))));
    // BigInt division truncates toward zero — the
    // audit's match for Morphic Int; not float division.
    assert!(js.contains("return (x / 2n);"), "{js}");
}

#[test]
fn js_int_literals_are_bigint_literals() {
    // D3a, the 2^53 cliff refused: the literal
    // 9223372036854775807 emitted bare reads back as
    // 9223372036854775808 in JS.
    let js = js_of_hand(
        SPEC_F,
        "f",
        ret(IRNode::Literal(IRLiteral::Int(i64::MAX))),
    );
    assert!(js.contains("return 9223372036854775807n;"), "{js}");
    let js = js_of_hand(
        SPEC_F,
        "f",
        ret(IRNode::Literal(IRLiteral::Int(i64::MIN))),
    );
    assert!(js.contains("return -9223372036854775808n;"), "{js}");
}

#[test]
fn js_constructors_and_index_render_the_frozen_forms() {
    // D4a: Some(x) is the payload itself, None is null,
    // Ok/Err are the discriminated-union objects.
    let js = js_of_hand(SPEC_F, "f", ret(call("Some", vec![var("x")])));
    assert!(js.contains("return (x);"), "{js}");
    let js = js_of_hand(SPEC_F, "f", ret(call("None", vec![])));
    assert!(js.contains("return null;"), "{js}");
    let js = js_of_hand(SPEC_F, "f", ret(call("Ok", vec![var("x")])));
    assert!(js.contains("return ({ tag: \"Ok\", value: x });"), "{js}");
    let js = js_of_hand(SPEC_F, "f", ret(call("Err", vec![var("x")])));
    assert!(js.contains("return ({ tag: \"Err\", error: x });"), "{js}");
    // A list literal is an array; Index is element
    // access (the Phase 118 `return undefined` forms).
    let js = js_of_hand(
        SPEC_F,
        "f",
        ret(IRNode::Collection {
            kind: morphic::synthesis::engine::CollectionKind::List,
            elements: vec![lit(1), lit(2)],
        }),
    );
    assert!(js.contains("return [1n, 2n];"), "{js}");
    const SPEC_XS: &str = "Probe {\nspec f {\n    input: xs: List<Int>\n    output: Int\n    constraint: output == xs\n    test \"t\" { input: 1 expect: 1 }\n}\n}";
    let js = js_of_hand(
        SPEC_XS,
        "f",
        ret(IRNode::Index(Box::new(var("xs")), Box::new(lit(0)))),
    );
    assert!(js.contains("return (xs)[0n];"), "{js}");
}

#[test]
fn js_if_in_expression_position_is_a_ternary() {
    // The composition shape int_clamp/int_sign crown
    // with: an If nested as a BinOp operand.
    let body = bin(
        IRBinOp::Add,
        IRNode::If {
            condition: Box::new(bin(IRBinOp::Lt, var("x"), lit(0))),
            then_branch: Box::new(lit(1)),
            else_branch: Box::new(lit(2)),
        },
        lit(10),
    );
    let js = js_of_hand(SPEC_F, "f", ret(body));
    assert!(
        js.contains("(((x < 0n)) ? (1n) : (2n))"),
        "ternary form:\n{js}"
    );
}

#[test]
fn js_while_arm_renders_a_real_loop() {
    let body = IRNode::Block(vec![
        IRNode::While {
            condition: Box::new(bin(IRBinOp::Neq, var("x"), lit(0))),
            body: Box::new(IRNode::Assign {
                target: Box::new(var("x")),
                value: Box::new(bin(IRBinOp::Sub, var("x"), lit(1))),
            }),
            invariant: Some("x >= 0".into()),
        },
        ret(var("x")),
    ]);
    let js = js_of_hand(SPEC_F, "f", body);
    assert!(js.contains("while ((x !== 0n)) {"), "{js}");
    assert!(js.contains("x = (x - 1n);"), "{js}");
    assert!(!js.contains("// unsupported"), "{js}");
}

#[test]
fn js_match_over_option_renders_null_discrimination() {
    let body = IRNode::Match {
        scrutinee: Box::new(var("x")),
        cases: vec![
            (
                IRPattern::Constructor("Some".into(), vec![IRPattern::Variable("v".into())]),
                var("v"),
            ),
            (IRPattern::Constructor("None".into(), vec![]), lit(0)),
        ],
    };
    const SPEC_OPT: &str = "Probe {\nspec f {\n    input: x: Option<Int>\n    output: Int\n    constraint: output == x\n    test \"t\" { input: 1 expect: 1 }\n}\n}";
    let js = js_of_hand(SPEC_OPT, "f", ret(body));
    assert!(js.contains("!== null"), "Some arm tests non-null:\n{js}");
    assert!(js.contains("const v ="), "payload binds:\n{js}");
    assert!(!js.contains("// unsupported"), "{js}");
}

// ── The entry-call reconciliation (work item 4) ──────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_entry_calls_render_as_plain_calls_and_run_assembled() {
    // A body calling a library entry generates working
    // JS: the call is a plain call to the entry's
    // export name, which Phase 121's module assembly
    // satisfies. Hand-assembled here: caller module +
    // entry module in one file, executed under node.
    let spec = hand_spec(
        "Caller {\nspec abs_plus {\n    input: x: Int\n    output: Int\n    constraint: output == x\n    test \"t\" { input: 1 expect: 1 }\n}\n}",
        "abs_plus",
    );
    let body = ret(bin(IRBinOp::Add, call("int_abs", vec![var("x")]), lit(1)));
    let caller_js = codegen::generate(&candidate(&spec, body), "javascript")
        .expect("an entry call generates on the JS route now");
    assert!(
        caller_js.contains("return (int_abs(x) + 1n);"),
        "plain call form:\n{caller_js}"
    );
    let module = format!("{}\n{}", js_of_entry("int_abs"), caller_js);
    let cases = vec![
        case(vec![J::Int(-5)], J::Int(6)),
        case(vec![J::Int(3)], J::Int(4)),
        case(vec![J::Int(0)], J::Int(1)),
    ];
    run_battery("entry-call", "abs_plus", &module, cases);

    // Statement position: a discarded entry call is a
    // call statement, not a comment.
    let body = IRNode::Block(vec![call("int_abs", vec![var("x")]), ret(var("x"))]);
    let js = codegen::generate(&candidate(&spec, body), "javascript").unwrap();
    assert!(js.contains("int_abs(x);"), "statement call:\n{js}");
    assert!(!js.contains("// unsupported"), "{js}");
}

#[test]
fn js_non_entry_calls_keep_their_standing_forms() {
    // The reconciliation's other direction: a call to a
    // name that is neither a builtin, a constructor,
    // nor a library entry keeps the standing fallback —
    // `undefined` in expression position, the standing
    // comment in statement position. (Phase 116 pins
    // the expression form too; this pins the statement
    // form, which the reconciliation must not widen.)
    let js = js_of_hand(SPEC_F, "f", ret(call("my_helper", vec![var("x")])));
    assert!(js.contains("return undefined;"), "{js}");
    let body = IRNode::Block(vec![call("my_helper", vec![var("x")]), ret(var("x"))]);
    let js = js_of_hand(SPEC_F, "f", body);
    assert!(js.contains("// unsupported"), "{js}");
}

// ── Plausible winners, executed under node ───────────────

#[test]
fn js_euclid_while_body_runs_under_node() {
    // The plausible-winner While vocabulary (the body
    // Phase 111's int_gcd pin executes natively):
    // Alloc + While + inner Alloc + Assign + Mod.
    const SPEC_GCD: &str = "Probe {\nspec gcd2 {\n    input: a: Int, b: Int\n    output: Int\n    constraint: output == a\n    test \"t\" { input: 1 expect: 1 }\n}\n}";
    let euclid = IRNode::Block(vec![
        IRNode::Alloc {
            name: "x".into(),
            ty: morphic::spec::ast::TypeRef::Int,
            initial: Box::new(var("a")),
        },
        IRNode::Alloc {
            name: "y".into(),
            ty: morphic::spec::ast::TypeRef::Int,
            initial: Box::new(var("b")),
        },
        IRNode::While {
            condition: Box::new(bin(IRBinOp::Neq, var("y"), lit(0))),
            body: Box::new(IRNode::Block(vec![
                IRNode::Alloc {
                    name: "t".into(),
                    ty: morphic::spec::ast::TypeRef::Int,
                    initial: Box::new(bin(IRBinOp::Mod, var("x"), var("y"))),
                },
                IRNode::Assign {
                    target: Box::new(var("x")),
                    value: Box::new(var("y")),
                },
                IRNode::Assign {
                    target: Box::new(var("y")),
                    value: Box::new(var("t")),
                },
            ])),
            invariant: Some("x >= 0".into()),
        },
        ret(var("x")),
    ]);
    let js = js_of_hand(SPEC_GCD, "gcd2", euclid);
    assert!(!js.contains("// unsupported"), "no dropped arms:\n{js}");
    let cases = vec![
        case(vec![J::Int(12), J::Int(18)], J::Int(6)),
        case(vec![J::Int(7), J::Int(0)], J::Int(7)),
        case(vec![J::Int(0), J::Int(5)], J::Int(5)),
        case(vec![J::Int(17), J::Int(5)], J::Int(1)),
        case(vec![J::Int(1071), J::Int(462)], J::Int(21)),
    ];
    run_battery("euclid", "gcd2", &js, cases);
}

#[test]
fn js_loop_with_valued_break_runs_under_node() {
    // Loop is the interpreter's value-carrying loop:
    // a valued Break is the loop's value (tail form).
    const SPEC_F2: &str = "Probe {\nspec f {\n    input: x: Int\n    output: Int\n    constraint: output == x\n    test \"t\" { input: 1 expect: 1 }\n}\n}";
    let body = IRNode::Loop {
        body: Box::new(IRNode::If {
            condition: Box::new(bin(IRBinOp::Gt, var("x"), lit(0))),
            then_branch: Box::new(IRNode::Break {
                value: Some(Box::new(bin(IRBinOp::Mul, var("x"), lit(2)))),
            }),
            else_branch: Box::new(IRNode::Break {
                value: Some(Box::new(lit(0))),
            }),
        }),
        invariant: None,
    };
    let js = js_of_hand(SPEC_F2, "f", body);
    assert!(!js.contains("// unsupported"), "{js}");
    let cases = vec![
        case(vec![J::Int(21)], J::Int(42)),
        case(vec![J::Int(-3)], J::Int(0)),
    ];
    run_battery("loop-break", "f", &js, cases);

    // Bare Break in statement position exits the loop.
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "n".into(),
            ty: morphic::spec::ast::TypeRef::Int,
            initial: Box::new(lit(0)),
        },
        IRNode::Loop {
            body: Box::new(IRNode::Block(vec![
                IRNode::Assign {
                    target: Box::new(var("n")),
                    value: Box::new(bin(IRBinOp::Add, var("n"), lit(1))),
                },
                IRNode::If {
                    condition: Box::new(bin(IRBinOp::Ge, var("n"), lit(3))),
                    then_branch: Box::new(IRNode::Break { value: None }),
                    else_branch: Box::new(IRNode::Block(vec![])),
                },
            ])),
            invariant: None,
        },
        ret(var("n")),
    ]);
    let js = js_of_hand(SPEC_F2, "f", body);
    let cases = vec![case(vec![J::Int(0)], J::Int(3))];
    run_battery("loop-bare-break", "f", &js, cases);
}

#[test]
fn js_statement_match_and_string_append_run_under_node() {
    // Statement-position Match over an Option
    // scrutinee, arms assigning to an Alloc'd cell.
    const SPEC_CLS: &str = "Probe {\nspec classify {\n    input: o: Option<Int>\n    output: Int\n    constraint: output == o\n    test \"t\" { input: 1 expect: 1 }\n}\n}";
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "r".into(),
            ty: morphic::spec::ast::TypeRef::Int,
            initial: Box::new(lit(-1)),
        },
        IRNode::Match {
            scrutinee: Box::new(var("o")),
            cases: vec![
                (
                    IRPattern::Constructor("Some".into(), vec![IRPattern::Variable("v".into())]),
                    IRNode::Assign {
                        target: Box::new(var("r")),
                        value: Box::new(var("v")),
                    },
                ),
                (
                    IRPattern::Constructor("None".into(), vec![]),
                    IRNode::Assign {
                        target: Box::new(var("r")),
                        value: Box::new(lit(0)),
                    },
                ),
            ],
        },
        ret(var("r")),
    ]);
    let js = js_of_hand(SPEC_CLS, "classify", body);
    assert!(!js.contains("// unsupported"), "{js}");
    let cases = vec![
        case(vec![J::Opt(Some(Box::new(J::Int(7))))], J::Int(7)),
        case(vec![J::Opt(None)], J::Int(0)),
    ];
    run_battery("stmt-match", "classify", &js, cases);

    // The lexeme shape (Phase 91's Append semantics):
    // a String accumulator grown by statement-position
    // Append of indexed chars — code-point indexing per
    // §3, so "héllo" round-trips exactly.
    const SPEC_ECHO: &str = "Probe {\nspec echo {\n    input: s: String\n    output: String\n    constraint: output == s\n    test \"t\" { input: 1 expect: 1 }\n}\n}";
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "out".into(),
            ty: morphic::spec::ast::TypeRef::String,
            initial: Box::new(IRNode::Literal(IRLiteral::String(String::new()))),
        },
        IRNode::For {
            var: "i".into(),
            start: Box::new(lit(0)),
            end: Box::new(IRNode::UnaryOp(
                IRUnaryOp::Len,
                Box::new(var("s")),
            )),
            body: Box::new(IRNode::Assign {
                target: Box::new(var("out")),
                value: Box::new(bin(
                    IRBinOp::Append,
                    var("out"),
                    IRNode::Index(Box::new(var("s")), Box::new(var("i"))),
                )),
            }),
        },
        ret(var("out")),
    ]);
    let js = js_of_hand(SPEC_ECHO, "echo", body);
    assert!(!js.contains("// unsupported"), "{js}");
    let cases = vec![
        case(vec![J::Str("abc".into())], J::Str("abc".into())),
        case(vec![J::Str("héllo".into())], J::Str("héllo".into())),
        case(vec![J::Str(String::new())], J::Str(String::new())),
    ];
    run_battery("lexeme-echo", "echo", &js, cases);

    // Statement-position Append on a String cell takes
    // the `+=` form (eval rebinds; JS `+=` is exactly
    // that for strings).
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "out".into(),
            ty: morphic::spec::ast::TypeRef::String,
            initial: Box::new(IRNode::Literal(IRLiteral::String(String::new()))),
        },
        bin(
            IRBinOp::Append,
            var("out"),
            IRNode::Literal(IRLiteral::Char('a')),
        ),
        bin(
            IRBinOp::Append,
            var("out"),
            IRNode::Literal(IRLiteral::Char('b')),
        ),
        ret(var("out")),
    ]);
    let js = js_of_hand(SPEC_ECHO, "echo", body);
    assert!(js.contains("out +="), "string append form:\n{js}");
    let cases = vec![case(vec![J::Str(String::new())], J::Str("ab".into()))];
    run_battery("lexeme-append", "echo", &js, cases);
}

#[test]
fn js_counting_for_and_let_chains_run_under_node() {
    // The count_matching shape: For + Index + Eq +
    // counter Assign (the Phase 115 vocabulary).
    const SPEC_CNT: &str = "Probe {\nspec count_eq {\n    input: xs: List<Int>, x: Int\n    output: Int\n    constraint: output == x\n    test \"t\" { input: 1 expect: 1 }\n}\n}";
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "c".into(),
            ty: morphic::spec::ast::TypeRef::Int,
            initial: Box::new(lit(0)),
        },
        IRNode::For {
            var: "i".into(),
            start: Box::new(lit(0)),
            end: Box::new(IRNode::UnaryOp(IRUnaryOp::Len, Box::new(var("xs")))),
            body: Box::new(IRNode::If {
                condition: Box::new(bin(
                    IRBinOp::Eq,
                    IRNode::Index(Box::new(var("xs")), Box::new(var("i"))),
                    var("x"),
                )),
                then_branch: Box::new(IRNode::Assign {
                    target: Box::new(var("c")),
                    value: Box::new(bin(IRBinOp::Add, var("c"), lit(1))),
                }),
                else_branch: Box::new(IRNode::Block(vec![])),
            }),
        },
        ret(var("c")),
    ]);
    let js = js_of_hand(SPEC_CNT, "count_eq", body);
    assert!(!js.contains("// unsupported"), "{js}");
    let cases = vec![
        case(vec![jlist(&[1, 2, 2, 3]), J::Int(2)], J::Int(2)),
        case(vec![jlist(&[]), J::Int(1)], J::Int(0)),
        case(vec![jlist(&[5, 5, 5]), J::Int(5)], J::Int(3)),
    ];
    run_battery("count-eq", "count_eq", &js, cases);

    // Let chains: const bindings feeding the tail.
    let body = IRNode::Let {
        name: "a".into(),
        value: Box::new(bin(IRBinOp::Add, var("x"), lit(1))),
        body: Box::new(IRNode::Let {
            name: "b".into(),
            value: Box::new(bin(IRBinOp::Mul, var("a"), lit(2))),
            body: Box::new(bin(IRBinOp::Add, var("b"), lit(5))),
        }),
    };
    let js = js_of_hand(SPEC_F, "f", body);
    assert!(js.contains("const a = (x + 1n);"), "{js}");
    let cases = vec![case(vec![J::Int(3)], J::Int(13))];
    run_battery("let-chain", "f", &js, cases);
}
