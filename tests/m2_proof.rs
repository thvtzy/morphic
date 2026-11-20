// ┌────────────────────────────────────────────────────────────┐
// │  M2: Pretty-Printer + Validator — Proof Tests              │
// │                                                            │
// │  Two layers, mirroring tests/tokenizer_proof.rs (M0 B):    │
// │                                                            │
// │  A. Pure-Rust mirror implementations of every spec in      │
// │     examples/self_host/validator.morph and printer.morph,  │
// │     with golden tests for every test case written in the   │
// │     .morph files — plus the multi-input specs              │
// │     (tags_compatible, needs_parens) whose test inputs      │
// │     cannot be expressed in the current spec test syntax.   │
// │                                                            │
// │  B. Real pipeline tests: the compiled `morphic` binary     │
// │     runs `check` (parse + typecheck) on both .morph        │
// │     files. Negative controls prove the pipeline actually   │
// │     rejects invalid specs, so a pass is meaningful.        │
// └────────────────────────────────────────────────────────────┘

use std::io::Write;
use std::process::Command;

// ═════════════════════════════════════════════════════════════
// A. VALIDATOR — pure functional mirror (no self, no &mut)
// ═════════════════════════════════════════════════════════════

fn is_known_type_tag(tag: i64) -> bool {
    (0..=8).contains(&tag)
}

fn is_known_node_kind(kind: i64) -> bool {
    (0..=9).contains(&kind)
}

/// Fixed arity per kind; variadic kinds (Call=4, ListLiteral=8)
/// are unpinned in the spec, mirrored here as None.
fn node_arity(kind: i64) -> Option<i64> {
    match kind {
        0 | 1 => Some(0),
        3 | 6 => Some(1),
        2 | 5 => Some(2),
        _ => None,
    }
}

fn is_literal_kind(kind: i64) -> bool {
    kind == 0
}

fn tags_compatible(expected: i64, actual: i64) -> bool {
    expected == actual || actual == 0 || expected == 0
}

fn validate_kind_tag(kind: i64) -> bool {
    is_known_node_kind(kind)
}

fn max_depth_ok(depth: i64) -> bool {
    (1..=64).contains(&depth)
}

// ═════════════════════════════════════════════════════════════
// A. PRINTER — pure functional mirror (no self, no &mut)
// ═════════════════════════════════════════════════════════════

fn type_tag_name(tag: i64) -> Option<&'static str> {
    match tag {
        1 => Some("Int"),
        2 => Some("Float"),
        3 => Some("Bool"),
        4 => Some("String"),
        6 => Some("List"),
        7 => Some("Option"),
        _ => None,
    }
}

fn node_kind_name(kind: i64) -> Option<&'static str> {
    match kind {
        0 => Some("Literal"),
        1 => Some("Variable"),
        2 => Some("BinaryOp"),
        3 => Some("UnaryOp"),
        4 => Some("Call"),
        5 => Some("Index"),
        6 => Some("FieldAccess"),
        7 => Some("Lambda"),
        _ => None,
    }
}

fn binop_symbol(op: i64) -> Option<&'static str> {
    match op {
        0 => Some("+"),
        1 => Some("-"),
        2 => Some("*"),
        3 => Some("/"),
        5 => Some("=="),
        7 => Some("<"),
        _ => None,
    }
}

fn needs_parens(op: i64, parent_op: i64) -> bool {
    (op == 0 || op == 1) && (parent_op == 2 || parent_op == 3)
}

fn is_infix_kind(kind: i64) -> bool {
    kind == 2
}

fn line_width_ok(width: i64) -> bool {
    (0..=100).contains(&width)
}

fn print_node_kind(kind: i64) -> Option<&'static str> {
    node_kind_name(kind)
}

// ═════════════════════════════════════════════════════════════
// A. GOLDEN TESTS — validator (mirror the .morph test cases)
// ═════════════════════════════════════════════════════════════

#[cfg(test)]
mod validator_tests {
    use super::*;

    #[test]
    fn known_type_tag() {
        // .morph tests: unknown-tag, int-tag, result-tag, out-of-range, far-out
        assert!(is_known_type_tag(0));
        assert!(is_known_type_tag(1));
        assert!(is_known_type_tag(8));
        assert!(!is_known_type_tag(9));
        assert!(!is_known_type_tag(42));
    }

    #[test]
    fn known_node_kind() {
        // .morph tests: literal-kind, comprehension-kind, unknown-kind
        assert!(is_known_node_kind(0));
        assert!(is_known_node_kind(9));
        assert!(!is_known_node_kind(10));
    }

    #[test]
    fn arity_table() {
        // .morph tests: literal, variable, unary, binary, index arity
        assert_eq!(node_arity(0), Some(0));
        assert_eq!(node_arity(1), Some(0));
        assert_eq!(node_arity(3), Some(1));
        assert_eq!(node_arity(2), Some(2));
        assert_eq!(node_arity(5), Some(2));
        // FieldAccess row of the table
        assert_eq!(node_arity(6), Some(1));
        // Variadic kinds are unpinned by the spec
        assert_eq!(node_arity(4), None);
        assert_eq!(node_arity(8), None);
    }

    #[test]
    fn literal_kind() {
        // .morph tests: literal, binary-not-literal, call-not-literal
        assert!(is_literal_kind(0));
        assert!(!is_literal_kind(2));
        assert!(!is_literal_kind(4));
    }

    #[test]
    fn compatible_tags() {
        // Multi-input spec: golden tests live here (see file header).
        assert!(tags_compatible(1, 1)); // Int := Int
        assert!(tags_compatible(4, 4)); // String := String
        assert!(!tags_compatible(1, 4)); // Int := String rejected
        assert!(!tags_compatible(3, 6)); // Bool := List rejected
        assert!(tags_compatible(1, 0)); // Unknown actual accepts
        assert!(tags_compatible(0, 6)); // Unknown expected accepts
    }

    #[test]
    fn validate_kind_tag_composition() {
        // .morph tests: call-kind-ok, bad-kind-rejected
        assert!(validate_kind_tag(4));
        assert!(!validate_kind_tag(77));
    }

    #[test]
    fn depth_guard() {
        // .morph tests: shallow, at-cap, over-cap, zero-depth
        assert!(max_depth_ok(1));
        assert!(max_depth_ok(64));
        assert!(!max_depth_ok(65));
        assert!(!max_depth_ok(0));
    }
}

// ═════════════════════════════════════════════════════════════
// A. GOLDEN TESTS — printer (mirror the .morph test cases)
// ═════════════════════════════════════════════════════════════

#[cfg(test)]
mod printer_tests {
    use super::*;

    #[test]
    fn type_tag_names() {
        // .morph tests: int-name, string-name, list-name
        assert_eq!(type_tag_name(1), Some("Int"));
        assert_eq!(type_tag_name(4), Some("String"));
        assert_eq!(type_tag_name(6), Some("List"));
        // Remaining rows of the table
        assert_eq!(type_tag_name(2), Some("Float"));
        assert_eq!(type_tag_name(3), Some("Bool"));
        assert_eq!(type_tag_name(7), Some("Option"));
    }

    #[test]
    fn node_kind_names() {
        // .morph tests: literal-name, binary-name, call-name, lambda-name
        assert_eq!(node_kind_name(0), Some("Literal"));
        assert_eq!(node_kind_name(2), Some("BinaryOp"));
        assert_eq!(node_kind_name(4), Some("Call"));
        assert_eq!(node_kind_name(7), Some("Lambda"));
        // Remaining rows of the table
        assert_eq!(node_kind_name(1), Some("Variable"));
        assert_eq!(node_kind_name(3), Some("UnaryOp"));
        assert_eq!(node_kind_name(5), Some("Index"));
        assert_eq!(node_kind_name(6), Some("FieldAccess"));
    }

    #[test]
    fn binop_symbols() {
        // .morph tests: add-symbol, mul-symbol, eq-symbol, lt-symbol
        assert_eq!(binop_symbol(0), Some("+"));
        assert_eq!(binop_symbol(2), Some("*"));
        assert_eq!(binop_symbol(5), Some("=="));
        assert_eq!(binop_symbol(7), Some("<"));
        // Remaining rows of the table
        assert_eq!(binop_symbol(1), Some("-"));
        assert_eq!(binop_symbol(3), Some("/"));
    }

    #[test]
    fn parens_precedence() {
        // Multi-input spec: golden tests live here (see file header).
        assert!(needs_parens(0, 2)); // (a + b) * c
        assert!(needs_parens(1, 3)); // (a - b) / c
        assert!(!needs_parens(2, 0)); // a * b + c
        assert!(!needs_parens(0, 1)); // a + b - c
        assert!(!needs_parens(2, 3)); // a * b / c
    }

    #[test]
    fn infix_kind() {
        // .morph tests: binary-is-infix, call-not-infix, literal-not-infix
        assert!(is_infix_kind(2));
        assert!(!is_infix_kind(4));
        assert!(!is_infix_kind(0));
    }

    #[test]
    fn line_width() {
        // .morph tests: normal-line, at-cap, over-cap
        assert!(line_width_ok(80));
        assert!(line_width_ok(100));
        assert!(!line_width_ok(101));
    }

    #[test]
    fn print_node_kind_composition() {
        // .morph tests: print-binary, print-lambda
        assert_eq!(print_node_kind(2), Some("BinaryOp"));
        assert_eq!(print_node_kind(7), Some("Lambda"));
    }

    #[test]
    fn cross_file_consistency() {
        // validator.morph and printer.morph share the NodeKind coding:
        // every kind the printer can name must be a kind the validator
        // knows, and every arity the validator pins must be known too.
        for kind in 0..=9 {
            if node_kind_name(kind).is_some() {
                assert!(is_known_node_kind(kind), "kind {} named but unknown", kind);
            }
            if node_arity(kind).is_some() {
                assert!(is_known_node_kind(kind), "kind {} has arity but unknown", kind);
            }
        }
        // Every type tag with a printed name must be a known type tag.
        for tag in 0..=8 {
            if type_tag_name(tag).is_some() {
                assert!(is_known_type_tag(tag), "tag {} named but unknown", tag);
            }
        }
    }
}

// ═════════════════════════════════════════════════════════════
// B. REAL PIPELINE — `morphic check` (parse + typecheck)
// ═════════════════════════════════════════════════════════════

#[cfg(test)]
mod pipeline_tests {
    use super::*;

    fn morphic_bin() -> &'static str {
        env!("CARGO_BIN_EXE_morphic")
    }

    fn run_check_source(source: &str, label: &str) -> std::process::Output {
        let mut path = std::env::temp_dir();
        path.push(format!("morphic_m2_{}_{}.morph", label, std::process::id()));
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(source.as_bytes()).unwrap();
        }
        let out = Command::new(morphic_bin())
            .arg("check")
            .arg(&path)
            .output()
            .expect("failed to run morphic binary");
        let _ = std::fs::remove_file(&path);
        out
    }

    #[test]
    fn check_validator_morph() {
        let out = Command::new(morphic_bin())
            .args(["check", "examples/self_host/validator.morph"])
            .output()
            .expect("failed to run morphic binary");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "validator.morph failed check:\n{}", stdout);
        assert!(stdout.contains("Specification is valid"), "unexpected output:\n{}", stdout);
    }

    #[test]
    fn check_printer_morph() {
        let out = Command::new(morphic_bin())
            .args(["check", "examples/self_host/printer.morph"])
            .output()
            .expect("failed to run morphic binary");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "printer.morph failed check:\n{}", stdout);
        assert!(stdout.contains("Specification is valid"), "unexpected output:\n{}", stdout);
    }

    #[test]
    fn check_rejects_undefined_variable() {
        // Negative control: an undeclared variable in a constraint
        // must fail the typechecker (proves a pass above is real).
        let src = r#"
T {
spec bad {
    input: tag: Int
    output: Bool
    constraint: output == (nope>=0&&nope<=8)
}
}
"#;
        let out = run_check_source(src, "undef_var");
        assert!(!out.status.success(), "undefined variable was accepted");
    }

    #[test]
    fn check_rejects_undefined_function_call() {
        // Negative control: calls must resolve to a spec in the module.
        let src = r#"
T {
spec bad {
    input: kind: Int
    output: Bool
    constraint: output == nosuchfn(kind)
}
}
"#;
        let out = run_check_source(src, "undef_fn");
        assert!(!out.status.success(), "undefined function call was accepted");
    }

    #[test]
    fn check_rejects_syntax_error() {
        // Negative control: an unknown keyword inside a spec body must
        // fail the parser. (Truncated files are also rejected since
        // the parser-hardening pass; this control keeps using a token
        // the parser actively rejects.)
        let src = r#"
T {
spec broken {
    input: tag: Int
    output: Bool
    frobnicate: 3
}
}
"#;
        let out = run_check_source(src, "syntax");
        assert!(!out.status.success(), "broken syntax was accepted");
    }

    #[test]
    fn spec_files_have_expected_shape() {
        // Supplementary shape check (the pipeline tests above are the
        // real validation): both files declare 7 specs each.
        for (path, module) in [
            ("examples/self_host/validator.morph", "Validator"),
            ("examples/self_host/printer.morph", "Printer"),
        ] {
            let src = std::fs::read_to_string(path).unwrap();
            assert!(src.contains(module), "{} missing module {}", path, module);
            let spec_decls = src.lines().filter(|l| l.starts_with("spec ")).count();
            assert_eq!(spec_decls, 7, "{} should declare 7 specs", path);
            assert!(src.matches("test \"").count() >= 10, "{} should carry in-spec tests", path);
        }
    }

    #[test]
    fn build_first_spec_resolves_module_calls() {
        // The CLI build path must be module-aware (Phase 19): the
        // fixture's FIRST spec is a composition spec whose
        // constraint calls a spec defined LATER in the same file.
        // Standalone synthesis gives it no constraint seed and no
        // way to resolve the call, so the winner cannot be the
        // resolved callee body. The module-aware pipeline — the
        // one the probe and cross-validation run
        // (synthesize_with_module + Verifier::with_specs) —
        // inlines the callee, so the build must succeed, the
        // generated Rust must BE the callee's behavior (the
        // range check itself, no call left in the output), and
        // the generated tests must pass when executed.
        let src = r#"
CompositionFixture {
spec composed_first {
    doc: "Composition proof: the first spec calls a later spec in the same module."
    input: kind: Int
    output: Bool

    constraint: output == later_check(kind)

    optimize: time < O(n)

    test "call-kind-ok" { input: 4 expect: true }
    test "bad-kind-rejected" { input: 77 expect: false }
}

spec later_check {
    doc: "Callee, defined after its caller."
    input: kind: Int
    output: Bool

    constraint: output == (kind>=0&&kind<=9)

    optimize: time < O(n)

    test "literal-kind" { input: 0 expect: true }
    test "unknown-kind" { input: 10 expect: false }
}
}
"#;
        let dir = std::env::temp_dir().join(format!(
            "morphic_m2_build_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let fixture = dir.join("composition_fixture.morph");
        let out_rs = dir.join("composition_fixture.rs");
        {
            let mut f = std::fs::File::create(&fixture).unwrap();
            f.write_all(src.as_bytes()).unwrap();
        }

        let out = Command::new(morphic_bin())
            .arg("build")
            .arg(&fixture)
            .args(["-O", "1", "--target", "rust", "-o"])
            .arg(&out_rs)
            .output()
            .expect("failed to run morphic binary");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "morphic build of a composition-first module failed:\nstdout:\n{}\nstderr:\n{}",
            stdout, stderr
        );
        assert!(
            stdout.contains("Generated:"),
            "build did not report a generated file:\n{}",
            stdout
        );

        let code = std::fs::read_to_string(&out_rs).unwrap();
        // The resolved behavior: the callee's own postcondition,
        // inlined — not a call to the callee.
        assert!(
            code.contains("kind >= 0") && code.contains("kind <= 9"),
            "generated code does not contain the resolved range check:\n{}",
            code
        );
        assert!(
            !code.contains("later_check"),
            "generated code still calls the callee instead of inlining it:\n{}",
            code
        );

        // The generated code must compile, and the spec's tests
        // (emitted as a #[cfg(test)] module) must pass executed.
        let test_bin = dir.join("composition_fixture_test");
        let compile = Command::new("rustc")
            .args(["--edition", "2021", "--test"])
            .arg(&out_rs)
            .arg("-o")
            .arg(&test_bin)
            .output()
            .expect("failed to run rustc");
        assert!(
            compile.status.success(),
            "generated code failed to compile:\n{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = Command::new(&test_bin)
            .output()
            .expect("failed to run generated tests");
        assert!(
            run.status.success(),
            "generated tests failed:\n{}",
            String::from_utf8_lossy(&run.stdout)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
