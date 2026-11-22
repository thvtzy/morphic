//! Phase 106 pipeline probes: seeded production-profile runs over
//! representative stdlib-candidate specs. These tests assert only
//! that the pipeline completes; the measured winner status and
//! executed-test fraction are printed for the audit record. A
//! candidate that crowns Inconclusive or passes part of its tests is
//! a successful *probe* — that outcome is the datum.

use morphic::api::{synthesize_seeded_spec, PLAYGROUND_DEFAULT_SEED};

fn run(label: &str, source: &str, spec_name: &str) {
    let report = synthesize_seeded_spec(source, spec_name, PLAYGROUND_DEFAULT_SEED)
        .unwrap_or_else(|e| panic!("{label}: pipeline failed at {}: {}", e.stage, e.message));
    let w = &report.winner;
    let body: String = w.body_debug.chars().take(220).collect();
    println!(
        "P106|pipe|{}|candidates={}|status={}|pipeline_tpr={:.3}|interp={}/{}/{}|body={}",
        label,
        report.candidates,
        w.verification_status,
        w.test_pass_ratio,
        w.tests_passed,
        w.tests_evaluable,
        w.tests_total,
        body.replace('\n', " "),
    );
}

const INT_CLAMP: &str = r#"Probe { spec int_clamp {
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

const CHAR_IS_DIGIT: &str = r#"Probe { spec char_is_digit {
    input: c: Char
    output: Bool
    constraint: output == (c >= '0' && c <= '9')
    test { input: '7'; expect: true }
    test { input: 'a'; expect: false }
    test { input: ' '; expect: false }
} }"#;

const STRING_IS_EMPTY: &str = r#"Probe { spec string_is_empty {
    input: s: String
    output: Bool
    constraint: output == (len(s) == 0)
    test { input: ""; expect: true }
    test { input: "x"; expect: false }
} }"#;

const STRING_REVERSE: &str = r#"Probe { spec string_reverse {
    input: s: String
    output: String
    constraint: len(output) == len(s)
    constraint: forall i: Int in range(len(s)): output[i] == s[(len(s) - 1) - i]
    test { input: "abc"; expect: "cba" }
    test { input: ""; expect: "" }
    test { input: "ab"; expect: "ba" }
} }"#;

const LIST_MAP_INC: &str = r#"Probe { spec list_map_inc {
    input: xs: List<Int>
    output: List<Int>
    constraint: len(output) == len(xs)
    constraint: forall i: Int in range(len(xs)): output[i] == xs[i] + 1
    test { input: [1, 2, 3]; expect: [2, 3, 4] }
    test { input: []; expect: [] }
} }"#;

const LIST_REVERSE: &str = r#"Probe { spec list_reverse {
    input: xs: List<Int>
    output: List<Int>
    constraint: len(output) == len(xs)
    constraint: forall i: Int in range(len(xs)): output[i] == xs[(len(xs) - 1) - i]
    test { input: [1, 2, 3]; expect: [3, 2, 1] }
    test { input: []; expect: [] }
} }"#;

const SUM_VIA_FOLD: &str = r#"Probe { spec sum_list {
    input: xs: List<Int>
    output: Int
    constraint: output == fold(xs, 0, |acc, x| acc + x)
    test { input: [1, 2, 3]; expect: 6 }
    test { input: []; expect: 0 }
    test { input: [5]; expect: 5 }
} }"#;

const LIST_CONTAINS: &str = r#"Probe { spec list_contains_int {
    input: xs: List<Int>, x: Int
    output: Bool
    constraint: forall i: Int in range(len(xs)): xs[i] == x implies output == true
    constraint: (forall i: Int in range(len(xs)): xs[i] != x) implies output == false
    test { input: ([1, 2, 3], 2); expect: true }
    test { input: ([1, 2, 3], 9); expect: false }
    test { input: ([], 1); expect: false }
} }"#;

const RESULT_UNWRAP_OR: &str = r#"Probe { spec result_unwrap_or {
    input: r: Result<Int, String>, fallback: Int
    output: Int
    constraint: r.is_err() || output == r.unwrap()
    constraint: r.is_ok() || output == fallback
    test { input: (Ok(7), 3); expect: 7 }
    test { input: (Err("bad"), 3); expect: 3 }
} }"#;

const SORT_INTS: &str = r#"Probe { spec sort_ints {
    input: xs: List<Int>
    output: List<Int>
    constraint: is_permutation(xs, output)
    constraint: len(output) == len(xs)
    constraint: forall i: Int, j: Int in range(len(output)): i < j implies output[i] <= output[j]
    test { input: [3, 1, 2]; expect: [1, 2, 3] }
    test { input: []; expect: [] }
    test { input: [2, 2, 1]; expect: [1, 2, 2] }
} }"#;

#[test]
fn phase106_pipe_int_clamp() {
    run("int_clamp", INT_CLAMP, "int_clamp");
}

#[test]
fn phase106_pipe_char_is_digit() {
    run("char_is_digit", CHAR_IS_DIGIT, "char_is_digit");
}

#[test]
fn phase106_pipe_string_is_empty() {
    run("string_is_empty", STRING_IS_EMPTY, "string_is_empty");
}

#[test]
fn phase106_pipe_string_reverse() {
    run("string_reverse", STRING_REVERSE, "string_reverse");
}

#[test]
fn phase106_pipe_list_map_inc() {
    run("list_map_inc", LIST_MAP_INC, "list_map_inc");
}

#[test]
fn phase106_pipe_list_reverse() {
    run("list_reverse", LIST_REVERSE, "list_reverse");
}

#[test]
fn phase106_pipe_sum_via_fold() {
    run("sum_via_fold", SUM_VIA_FOLD, "sum_list");
}

#[test]
fn phase106_pipe_list_contains() {
    run("list_contains", LIST_CONTAINS, "list_contains_int");
}

#[test]
fn phase106_pipe_result_unwrap_or() {
    run("result_unwrap_or", RESULT_UNWRAP_OR, "result_unwrap_or");
}

#[test]
fn phase106_pipe_sort_ints() {
    run("sort_ints", SORT_INTS, "sort_ints");
}

#[test]
fn phase106_pipe_existing_is_sorted() {
    let source = std::fs::read_to_string("examples/self_host/is_sorted.morph").unwrap();
    run("existing_is_sorted", &source, "is_sorted");
}

#[test]
fn phase106_pipe_existing_count_matching() {
    let source = std::fs::read_to_string("examples/self_host/count_matching.morph").unwrap();
    run("existing_count_matching", &source, "count_matching");
}

#[test]
fn phase106_pipe_existing_sort_generic() {
    let source = std::fs::read_to_string("examples/sort.morph").unwrap();
    run("existing_sort_generic", &source, "sort");
}

#[test]
fn phase106_pipe_existing_binary_search() {
    let source = std::fs::read_to_string("examples/binary_search.morph").unwrap();
    run("existing_binary_search", &source, "binary_search");
}
