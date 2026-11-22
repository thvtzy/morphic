//! Phase 115 cross-validation (gate clause 3's
//! discipline, STDLIB_SCOPE §3): every shipped List
//! entry is executed against a hand-written Rust
//! mirror over a battery far wider than its in-spec
//! tests — pinned points must agree 100%.
//!
//! The battery runs through the production executed
//! judge (`eval::test_outcomes`) on a generated spec
//! carrying the entry's signature; the expected values
//! are computed by the mirrors below. None of the
//! entries except `list_binary_search` carries a
//! precondition, and its battery runs sorted inputs
//! only, so no point here is spec-silent by
//! precondition. The one spec-silent class is
//! `list_binary_search`'s duplicate-target points: the
//! frozen contract lets ANY valid index satisfy a
//! duplicate target, so those points are judged on the
//! contract's property (Some(i) with xs[i] == target)
//! rather than a pinned index, and are counted
//! separately below.
//!
//! Arithmetic discipline: the language's Int
//! arithmetic is checked (overflow is an evaluation
//! failure, not a value), so the sum/product batteries
//! pin representable extremes — a lone MAX or MIN,
//! cancelling pairs — and no overflowing point. Such a
//! point has no value in the language to agree on.

use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::eval::{self, Env, TestOutcome, Value};

// ── Hand-written mirrors ─────────────────────────────────

fn m_is_empty(xs: &[i64]) -> bool {
    xs.is_empty()
}
fn m_reverse(xs: &[i64]) -> Vec<i64> {
    xs.iter().rev().copied().collect()
}
fn m_contains(xs: &[i64], x: i64) -> bool {
    xs.contains(&x)
}
fn m_sum(xs: &[i64]) -> i64 {
    xs.iter().sum()
}
fn m_product(xs: &[i64]) -> i64 {
    xs.iter().product()
}
fn m_min(xs: &[i64]) -> Option<i64> {
    xs.iter().min().copied()
}
fn m_max(xs: &[i64]) -> Option<i64> {
    xs.iter().max().copied()
}
fn clamp_n(xs: &[i64], n: i64) -> usize {
    n.clamp(0, xs.len() as i64) as usize
}
fn m_take(xs: &[i64], n: i64) -> Vec<i64> {
    xs[..clamp_n(xs, n)].to_vec()
}
fn m_drop(xs: &[i64], n: i64) -> Vec<i64> {
    xs[clamp_n(xs, n)..].to_vec()
}
fn m_is_sorted(xs: &[i64]) -> bool {
    xs.windows(2).all(|w| w[0] <= w[1])
}
fn m_sort(xs: &[i64]) -> Vec<i64> {
    let mut out = xs.to_vec();
    out.sort();
    out
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

fn fmt_list(xs: &[i64]) -> String {
    let parts: Vec<String> = xs.iter().map(|n| fmt_int(*n)).collect();
    format!("[{}]", parts.join(", "))
}

fn fmt_bool(b: bool) -> String {
    b.to_string()
}

fn fmt_option(o: &Option<i64>) -> String {
    match o {
        Some(n) => format!("Some({})", fmt_int(*n)),
        None => "None()".to_string(),
    }
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
    println!("P115XV|{}|pinned={}|agree=100%", entry, pinned.len());
}

/// The shared list battery: degenerate, singleton,
/// typical, adversarial (duplicates, plateaus,
/// reversals, negatives, representable extremes).
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

// ── The batteries ────────────────────────────────────────

#[test]
fn xv_list_is_empty() {
    let pinned: Vec<(String, String)> = lists()
        .iter()
        .map(|xs| (fmt_list(xs), fmt_bool(m_is_empty(xs))))
        .collect();
    run_battery(
        "list_is_empty",
        "    input: xs: List<Int>\n    output: Bool",
        pinned,
    );
}

#[test]
fn xv_list_reverse() {
    let pinned: Vec<(String, String)> = lists()
        .iter()
        .map(|xs| (fmt_list(xs), fmt_list(&m_reverse(xs))))
        .collect();
    run_battery(
        "list_reverse",
        "    input: xs: List<Int>\n    output: List<Int>",
        pinned,
    );
}

#[test]
fn xv_list_contains() {
    let targets = [0, 2, -1, 9, i64::MAX];
    let mut pinned = Vec::new();
    for xs in lists() {
        for x in targets {
            pinned.push((
                format!("({}, {})", fmt_list(&xs), fmt_int(x)),
                fmt_bool(m_contains(&xs, x)),
            ));
        }
    }
    run_battery(
        "list_contains",
        "    input: xs: List<Int>, x: Int\n    output: Bool",
        pinned,
    );
}

#[test]
fn xv_list_sum() {
    // No overflowing point (checked arithmetic): the
    // extremes appear alone or cancelling.
    let pinned: Vec<(String, String)> = lists()
        .iter()
        .map(|xs| (fmt_list(xs), fmt_int(m_sum(xs))))
        .collect();
    run_battery(
        "list_sum",
        "    input: xs: List<Int>\n    output: Int",
        pinned,
    );
}

#[test]
fn xv_list_product() {
    // The product battery is its own: multi-element
    // extreme products would overflow, so extremes ride
    // with 0 or 1, or stand alone.
    let cases: Vec<Vec<i64>> = vec![
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
    let pinned: Vec<(String, String)> = cases
        .iter()
        .map(|xs| (fmt_list(xs), fmt_int(m_product(xs))))
        .collect();
    run_battery(
        "list_product",
        "    input: xs: List<Int>\n    output: Int",
        pinned,
    );
}

#[test]
fn xv_list_min() {
    let pinned: Vec<(String, String)> = lists()
        .iter()
        .map(|xs| (fmt_list(xs), fmt_option(&m_min(xs))))
        .collect();
    run_battery(
        "list_min",
        "    input: xs: List<Int>\n    output: Option<Int>",
        pinned,
    );
}

#[test]
fn xv_list_max() {
    let pinned: Vec<(String, String)> = lists()
        .iter()
        .map(|xs| (fmt_list(xs), fmt_option(&m_max(xs))))
        .collect();
    run_battery(
        "list_max",
        "    input: xs: List<Int>\n    output: Option<Int>",
        pinned,
    );
}

#[test]
fn xv_list_take() {
    let cases: Vec<Vec<i64>> = vec![vec![], vec![5], vec![1, 2, 3], vec![4, 3, 2, 1]];
    let ns = [-3, 0, 1, 2, 3, 5, 99];
    let mut pinned = Vec::new();
    for xs in &cases {
        for n in ns {
            pinned.push((
                format!("({}, {})", fmt_list(xs), fmt_int(n)),
                fmt_list(&m_take(xs, n)),
            ));
        }
    }
    run_battery(
        "list_take",
        "    input: xs: List<Int>, n: Int\n    output: List<Int>",
        pinned,
    );
}

#[test]
fn xv_list_drop() {
    let cases: Vec<Vec<i64>> = vec![vec![], vec![5], vec![1, 2, 3], vec![4, 3, 2, 1]];
    let ns = [-3, 0, 1, 2, 3, 5, 99];
    let mut pinned = Vec::new();
    for xs in &cases {
        for n in ns {
            pinned.push((
                format!("({}, {})", fmt_list(xs), fmt_int(n)),
                fmt_list(&m_drop(xs, n)),
            ));
        }
    }
    run_battery(
        "list_drop",
        "    input: xs: List<Int>, n: Int\n    output: List<Int>",
        pinned,
    );
}

#[test]
fn xv_list_is_sorted() {
    let pinned: Vec<(String, String)> = lists()
        .iter()
        .map(|xs| (fmt_list(xs), fmt_bool(m_is_sorted(xs))))
        .collect();
    run_battery(
        "list_is_sorted",
        "    input: xs: List<Int>\n    output: Bool",
        pinned,
    );
}

#[test]
fn xv_list_sort() {
    let pinned: Vec<(String, String)> = lists()
        .iter()
        .map(|xs| (fmt_list(xs), fmt_list(&m_sort(xs))))
        .collect();
    run_battery(
        "list_sort",
        "    input: xs: List<Int>\n    output: List<Int>",
        pinned,
    );
}

/// list_binary_search, judged on the contract's
/// property directly (the mirror is the semantics, not
/// a specific index): for sorted inputs, the result is
/// None iff the target is absent, and Some(i) only
/// with xs[i] == target. Unique-target points pin the
/// index exactly (the body's first-match scan and any
/// correct search agree there); duplicate-target
/// points are the frozen spec-silent class — the
/// specific index is not pinned, the property is —
/// and are counted separately.
#[test]
fn xv_list_binary_search() {
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
    let body = stdlib::shipped_body("list_binary_search").unwrap();
    let mut pinned_exact = 0usize;
    let mut index_silent = 0usize;
    for xs in &sorted_lists {
        let mut targets: Vec<i64> = xs.clone();
        targets.extend([-1, 0, 4, 100, i64::MIN, i64::MAX]);
        targets.sort();
        targets.dedup();
        for x in targets {
            let mut env: Env = Env::new();
            env.insert(
                "xs".to_string(),
                Value::List(xs.iter().map(|n| Value::Int(*n)).collect()),
            );
            env.insert("x".to_string(), Value::Int(x));
            let result = eval::eval(&body, &env)
                .unwrap_or_else(|e| panic!("search {xs:?} for {x}: body must evaluate: {e:?}"));
            let occurrences = xs.iter().filter(|v| **v == x).count();
            match (&result, occurrences) {
                (Value::None, 0) => pinned_exact += 1,
                (Value::Some(idx), occ) => {
                    let Value::Int(idx) = **idx else {
                        panic!("search {xs:?} for {x}: payload must be an index, got {idx:?}")
                    };
                    assert!(
                        idx >= 0 && (idx as usize) < xs.len() && xs[idx as usize] == x,
                        "search {xs:?} for {x}: Some({idx}) must index an occurrence"
                    );
                    if occ == 1 {
                        assert_eq!(
                            idx as usize,
                            xs.iter().position(|v| *v == x).unwrap(),
                            "search {xs:?} for {x}: a unique target pins its index"
                        );
                        pinned_exact += 1;
                    } else {
                        // Duplicate target: any valid index
                        // satisfies the frozen contract —
                        // spec-silent, counted, not pinned.
                        index_silent += 1;
                    }
                }
                (other, occ) => panic!(
                    "search {xs:?} for {x}: got {other:?} with {occ} occurrences in the list"
                ),
            }
        }
    }
    assert!(pinned_exact > 0 && index_silent > 0);
    println!(
        "P115XV|list_binary_search|pinned-property={}|index-silent={}|agree=100%",
        pinned_exact + index_silent,
        index_silent
    );
}
