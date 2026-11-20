// ┌────────────────────────────────────────────────────────────┐
// │  M3: Codegen — Proof Tests (slice 1: leaf mappings)        │
// │                                                            │
// │  Two layers, mirroring tests/m2_proof.rs:                  │
// │                                                            │
// │  A. Pure-Rust mirror implementations of every spec in      │
// │     examples/self_host/codegen.morph, with golden tests    │
// │     covering the FULL mapping range of each table plus     │
// │     off-range inputs. The specs' constraints carry the     │
// │     complete mapping; these mirrors are the executable     │
// │     cross-check (the M2 lesson: never trust a verdict      │
// │     without cross-validation).                             │
// │                                                            │
// │     The real emitter functions are private to              │
// │     src/codegen/mod.rs, so — as in M2 — each mirror is a   │
// │     faithful hand copy of the table it mirrors, with the   │
// │     source location cited. Enum codes follow declaration   │
// │     order in the Rust source, as in the .morph file.       │
// │                                                            │
// │  B. Real pipeline tests: the compiled `morphic` binary     │
// │     runs `check` (parse + typecheck) on codegen.morph.     │
// │     A negative control proves the pipeline actually        │
// │     rejects invalid specs, so a pass is meaningful.        │
// └────────────────────────────────────────────────────────────┘

use std::io::Write;
use std::process::Command;

// ═════════════════════════════════════════════════════════════
// A. MIRRORS — slice 1 leaf mappings of the Rust emitter
// ═════════════════════════════════════════════════════════════

/// Mirror of the scalar leaf table of `ty_to_rust`
/// (src/codegen/mod.rs:1093). Composite and named types are
/// recursive/name-carrying in the original and unpinned in the
/// spec; mirrored here as None.
fn rust_type_name(tag: i64) -> Option<&'static str> {
    match tag {
        0 => Some("i64"),     // Int
        1 => Some("f64"),     // Float
        2 => Some("bool"),    // Bool
        3 => Some("String"),  // String
        4 => Some("char"),    // Char
        5 => Some("()"),      // Unit
        6 => Some("i8"),      // I8
        7 => Some("i16"),     // I16
        8 => Some("i32"),     // I32
        9 => Some("i64"),     // I64 — same name as Int, as in the original
        10 => Some("u8"),     // U8
        11 => Some("u16"),    // U16
        12 => Some("u32"),    // U32
        13 => Some("u64"),    // U64
        _ => None,
    }
}

/// Mirror of the operator table inside `ir_to_rust_expr`
/// (src/codegen/mod.rs:615). Append (17) and Merge (18) are
/// bound to ".push"/".extend" in the original table but those
/// values are dead — the arms are special-cased into method
/// calls before the symbol is used — so they have no infix
/// symbol: None here, exactly as the spec leaves them unpinned.
fn rust_binop_symbol(op: i64) -> Option<&'static str> {
    match op {
        0 => Some("+"),   // Add
        1 => Some("-"),   // Sub
        2 => Some("*"),   // Mul
        3 => Some("/"),   // Div
        4 => Some("%"),   // Mod
        5 => Some("&&"),  // And
        6 => Some("||"),  // Or
        7 => Some("^"),   // Xor
        8 => Some("=="),  // Eq
        9 => Some("!="),  // Neq
        10 => Some("<"),  // Lt
        11 => Some("<="), // Le
        12 => Some(">"),  // Gt
        13 => Some(">="), // Ge
        14 => Some("<<"), // Shl
        15 => Some(">>"), // Shr
        16 => Some("+"),  // Concat — shares Add's symbol
        _ => None,
    }
}

/// Mirror of the prefix arms of the UnaryOp match in
/// `ir_to_rust_expr` (src/codegen/mod.rs:636): only Neg and
/// Not render with a prefix symbol (parenthesized).
fn rust_unaryop_symbol(op: i64) -> Option<&'static str> {
    match op {
        0 => Some("-"), // Neg
        1 => Some("!"), // Not
        _ => None,
    }
}

/// Mirror of the method-call arms of the UnaryOp match in
/// `ir_to_rust_expr` (src/codegen/mod.rs:636).
fn rust_unaryop_method(op: i64) -> Option<&'static str> {
    match op {
        2 => Some("abs"),     // Abs
        3 => Some("len"),     // Len
        8 => Some("reverse"), // Reverse
        9 => Some("clone"),   // Clone
        _ => None,
    }
}

/// Mirror of the emission-family classification of the UnaryOp
/// match in `ir_to_rust_expr` (src/codegen/mod.rs:636):
/// 0 = prefix symbol, 1 = method call, 2 = list-part
/// index/slice form. Total over the ten operators.
fn rust_unaryop_form(op: i64) -> Option<i64> {
    match op {
        0 | 1 => Some(0),         // Neg, Not
        2 | 3 | 8 | 9 => Some(1), // Abs, Len, Reverse, Clone
        4..=7 => Some(2),         // Head, Tail, Init, Last
        _ => None,
    }
}

/// Mirror of the Collection match in `ir_to_rust_expr`
/// (src/codegen/mod.rs:676), EXPRESSION position forms:
/// 0 = vec![..], 1 = HashSet::from([..]), 2 = HashMap::from([..]),
/// 3 = array literal, 4 = tuple literal. (Statement position in
/// the original collapses List/Set/Map to the vec! form; that
/// position rule belongs to the statement-walker slice.)
fn rust_collection_form(kind: i64) -> Option<i64> {
    match kind {
        0 => Some(0), // List
        1 => Some(1), // Set
        2 => Some(2), // Map
        3 => Some(3), // Array
        4 => Some(4), // Tuple
        _ => None,
    }
}

/// Mirror of `is_rust_keyword` (src/codegen/mod.rs:53): the
/// emitter's exact 51-word set, copied verbatim.
fn is_rust_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern"
            | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match"
            | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self"
            | "static" | "struct" | "super" | "trait" | "true" | "type" | "unsafe"
            | "use" | "where" | "while" | "async" | "await" | "dyn" | "abstract"
            | "become" | "box" | "do" | "final" | "macro" | "override" | "priv"
            | "typeof" | "unsized" | "virtual" | "yield" | "try"
    )
}

/// Mirror of the operator table of `expr_to_rust`
/// (src/codegen/mod.rs:1188) — the spec-expression emitter used
/// for generated test assertions. A separate, weaker table than
/// `rust_binop_symbol`: the operators it does not map fall
/// through to "?" in the original, pinned here as rows.
fn spec_binop_symbol(op: i64) -> Option<&'static str> {
    match op {
        0 => Some("+"),  // Add
        1 => Some("-"),  // Sub
        2 => Some("*"),  // Mul
        3 => Some("/"),  // Div
        4 => Some("?"),  // Mod — unmapped in the original
        5 => Some("&&"), // And
        6 => Some("||"), // Or
        7 => Some("=="), // Eq
        8 => Some("!="), // Neq
        9 => Some("?"),  // Lt — unmapped in the original
        10 => Some("?"), // Le — unmapped
        11 => Some("?"), // Gt — unmapped
        12 => Some("?"), // Ge — unmapped
        13 => Some("?"), // Concat — unmapped
        _ => None,
    }
}

// ═════════════════════════════════════════════════════════════
// A. GOLDEN TESTS — full mapping range + off-range inputs
// ═════════════════════════════════════════════════════════════

#[cfg(test)]
mod golden_tests {
    use super::*;

    #[test]
    fn type_names_full_table() {
        // .morph tests: int-name, string-name, unit-name, u64-name
        assert_eq!(rust_type_name(0), Some("i64"));
        assert_eq!(rust_type_name(3), Some("String"));
        assert_eq!(rust_type_name(5), Some("()"));
        assert_eq!(rust_type_name(13), Some("u64"));
        // Remaining rows of the table
        assert_eq!(rust_type_name(1), Some("f64"));
        assert_eq!(rust_type_name(2), Some("bool"));
        assert_eq!(rust_type_name(4), Some("char"));
        assert_eq!(rust_type_name(6), Some("i8"));
        assert_eq!(rust_type_name(7), Some("i16"));
        assert_eq!(rust_type_name(8), Some("i32"));
        assert_eq!(rust_type_name(9), Some("i64"));
        assert_eq!(rust_type_name(10), Some("u8"));
        assert_eq!(rust_type_name(11), Some("u16"));
        assert_eq!(rust_type_name(12), Some("u32"));
        // Composite / named codes are unpinned by the spec
        for tag in 14..=25 {
            assert_eq!(rust_type_name(tag), None, "tag {} must be unpinned", tag);
        }
        // Off-range
        assert_eq!(rust_type_name(-1), None);
        assert_eq!(rust_type_name(26), None);
        assert_eq!(rust_type_name(100), None);
    }

    #[test]
    fn binop_symbols_full_table() {
        // .morph tests: add, mod, xor, shl, concat symbols
        assert_eq!(rust_binop_symbol(0), Some("+"));
        assert_eq!(rust_binop_symbol(4), Some("%"));
        assert_eq!(rust_binop_symbol(7), Some("^"));
        assert_eq!(rust_binop_symbol(14), Some("<<"));
        assert_eq!(rust_binop_symbol(16), Some("+"));
        // Remaining rows of the table
        assert_eq!(rust_binop_symbol(1), Some("-"));
        assert_eq!(rust_binop_symbol(2), Some("*"));
        assert_eq!(rust_binop_symbol(3), Some("/"));
        assert_eq!(rust_binop_symbol(5), Some("&&"));
        assert_eq!(rust_binop_symbol(6), Some("||"));
        assert_eq!(rust_binop_symbol(8), Some("=="));
        assert_eq!(rust_binop_symbol(9), Some("!="));
        assert_eq!(rust_binop_symbol(10), Some("<"));
        assert_eq!(rust_binop_symbol(11), Some("<="));
        assert_eq!(rust_binop_symbol(12), Some(">"));
        assert_eq!(rust_binop_symbol(13), Some(">="));
        assert_eq!(rust_binop_symbol(15), Some(">>"));
        // Append / Merge have no infix symbol
        assert_eq!(rust_binop_symbol(17), None);
        assert_eq!(rust_binop_symbol(18), None);
        // Off-range
        assert_eq!(rust_binop_symbol(-1), None);
        assert_eq!(rust_binop_symbol(19), None);
    }

    #[test]
    fn unaryop_symbols() {
        // .morph tests: neg-symbol, not-symbol
        assert_eq!(rust_unaryop_symbol(0), Some("-"));
        assert_eq!(rust_unaryop_symbol(1), Some("!"));
        // Every other operator is unpinned in this table
        for op in 2..=9 {
            assert_eq!(rust_unaryop_symbol(op), None, "op {} must be unpinned", op);
        }
        assert_eq!(rust_unaryop_symbol(-1), None);
        assert_eq!(rust_unaryop_symbol(10), None);
    }

    #[test]
    fn unaryop_methods() {
        // .morph tests: abs-method, len-method, clone-method
        assert_eq!(rust_unaryop_method(2), Some("abs"));
        assert_eq!(rust_unaryop_method(3), Some("len"));
        assert_eq!(rust_unaryop_method(9), Some("clone"));
        // Remaining row of the table
        assert_eq!(rust_unaryop_method(8), Some("reverse"));
        // Prefix and list-part operators are unpinned here
        for op in [0, 1, 4, 5, 6, 7] {
            assert_eq!(rust_unaryop_method(op), None, "op {} must be unpinned", op);
        }
        assert_eq!(rust_unaryop_method(-1), None);
        assert_eq!(rust_unaryop_method(10), None);
    }

    #[test]
    fn unaryop_forms_total() {
        // .morph tests: neg-form, abs-form, head-form, clone-form
        assert_eq!(rust_unaryop_form(0), Some(0));
        assert_eq!(rust_unaryop_form(2), Some(1));
        assert_eq!(rust_unaryop_form(4), Some(2));
        assert_eq!(rust_unaryop_form(9), Some(1));
        // Remaining rows — the classification is total
        assert_eq!(rust_unaryop_form(1), Some(0));
        assert_eq!(rust_unaryop_form(3), Some(1));
        assert_eq!(rust_unaryop_form(5), Some(2));
        assert_eq!(rust_unaryop_form(6), Some(2));
        assert_eq!(rust_unaryop_form(7), Some(2));
        assert_eq!(rust_unaryop_form(8), Some(1));
        // Off-range
        assert_eq!(rust_unaryop_form(-1), None);
        assert_eq!(rust_unaryop_form(10), None);
    }

    #[test]
    fn collection_forms_total() {
        // .morph tests: list-form, set-form, tuple-form
        assert_eq!(rust_collection_form(0), Some(0));
        assert_eq!(rust_collection_form(1), Some(1));
        assert_eq!(rust_collection_form(4), Some(4));
        // Remaining rows — the classification is total
        assert_eq!(rust_collection_form(2), Some(2));
        assert_eq!(rust_collection_form(3), Some(3));
        // Off-range
        assert_eq!(rust_collection_form(-1), None);
        assert_eq!(rust_collection_form(5), None);
    }

    #[test]
    fn keyword_set_complete() {
        // Every word of the emitter's 51-word set (source order).
        let keywords = [
            "as", "break", "const", "continue", "crate", "else", "enum", "extern",
            "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
            "mod", "move", "mut", "pub", "ref", "return", "self", "Self",
            "static", "struct", "super", "trait", "true", "type", "unsafe",
            "use", "where", "while", "async", "await", "dyn", "abstract",
            "become", "box", "do", "final", "macro", "override", "priv",
            "typeof", "unsized", "virtual", "yield", "try",
        ];
        assert_eq!(keywords.len(), 51);
        for kw in keywords {
            assert!(is_rust_keyword(kw), "{} must be a keyword", kw);
        }
        // .morph tests: fn-keyword, self-keyword
        assert!(is_rust_keyword("fn"));
        assert!(is_rust_keyword("Self"));
    }

    #[test]
    fn keyword_set_negative_space() {
        // .morph tests: plain-name, prefix-name, case-name
        assert!(!is_rust_keyword("sort"));
        assert!(!is_rust_keyword("selfish"));
        assert!(!is_rust_keyword("Fn"));
        // Exact, case-sensitive matching — near misses stay names
        assert!(!is_rust_keyword(""));
        assert!(!is_rust_keyword("SELF"));
        assert!(!is_rust_keyword("fn2"));
        assert!(!is_rust_keyword("as "));
        assert!(!is_rust_keyword(" async"));
        assert!(!is_rust_keyword("crate_"));
        assert!(!is_rust_keyword("main"));
        // Real Rust words the emitter's set does NOT carry
        assert!(!is_rust_keyword("gen")); // 2024-edition keyword, absent here
        assert!(!is_rust_keyword("union")); // contextual keyword, absent here
    }

    #[test]
    fn spec_binop_symbols_full_table() {
        // .morph tests: add-symbol, neq-symbol, mod-fallback, lt-fallback
        assert_eq!(spec_binop_symbol(0), Some("+"));
        assert_eq!(spec_binop_symbol(8), Some("!="));
        assert_eq!(spec_binop_symbol(4), Some("?"));
        assert_eq!(spec_binop_symbol(9), Some("?"));
        // Remaining rows of the table
        assert_eq!(spec_binop_symbol(1), Some("-"));
        assert_eq!(spec_binop_symbol(2), Some("*"));
        assert_eq!(spec_binop_symbol(3), Some("/"));
        assert_eq!(spec_binop_symbol(5), Some("&&"));
        assert_eq!(spec_binop_symbol(6), Some("||"));
        assert_eq!(spec_binop_symbol(7), Some("=="));
        assert_eq!(spec_binop_symbol(10), Some("?"));
        assert_eq!(spec_binop_symbol(11), Some("?"));
        assert_eq!(spec_binop_symbol(12), Some("?"));
        assert_eq!(spec_binop_symbol(13), Some("?"));
        // Off-range
        assert_eq!(spec_binop_symbol(-1), None);
        assert_eq!(spec_binop_symbol(14), None);
    }

    #[test]
    fn cross_table_consistency() {
        // The unary tables agree: symbol-pinned ops are exactly
        // the prefix-form ops, method-pinned ops are exactly the
        // method-form ops, and list-part ops have neither.
        for op in 0..=9 {
            let form = rust_unaryop_form(op).expect("form is total");
            assert_eq!(rust_unaryop_symbol(op).is_some(), form == 0, "op {}", op);
            assert_eq!(rust_unaryop_method(op).is_some(), form == 1, "op {}", op);
        }
        // Collection forms are a permutation of 0..=4.
        let mut forms: Vec<i64> = (0..=4).map(|k| rust_collection_form(k).unwrap()).collect();
        forms.sort_unstable();
        assert_eq!(forms, vec![0, 1, 2, 3, 4]);
        // The type table's one collision: Int and I64 share a name.
        assert_eq!(rust_type_name(0), rust_type_name(9));
        // Concat shares Add's symbol in the IR table.
        assert_eq!(rust_binop_symbol(16), rust_binop_symbol(0));
        // The two operator tables DIVERGE by name (codes differ
        // between the enums; compared here by operator name):
        // Mod is spec-code 4 / IR-code 4, Lt is 9 / 10,
        // Concat is 13 / 16.
        assert_eq!(spec_binop_symbol(4), Some("?")); // spec Mod
        assert_eq!(rust_binop_symbol(4), Some("%")); // IR Mod
        assert_eq!(spec_binop_symbol(9), Some("?")); // spec Lt
        assert_eq!(rust_binop_symbol(10), Some("<")); // IR Lt
        assert_eq!(spec_binop_symbol(13), Some("?")); // spec Concat
        assert_eq!(rust_binop_symbol(16), Some("+")); // IR Concat
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
        path.push(format!("morphic_m3_{}_{}.morph", label, std::process::id()));
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
    fn check_codegen_morph() {
        let out = Command::new(morphic_bin())
            .args(["check", "examples/self_host/codegen.morph"])
            .output()
            .expect("failed to run morphic binary");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "codegen.morph failed check:\n{}", stdout);
        assert!(stdout.contains("Specification is valid"), "unexpected output:\n{}", stdout);
    }

    #[test]
    fn check_rejects_undefined_function_call() {
        // Negative control: calls must resolve to a spec in the module.
        let src = r#"
T {
spec bad {
    input: op: Int
    output: String
    constraint: output == nosuchfn(op)
}
}
"#;
        let out = run_check_source(src, "undef_fn");
        assert!(!out.status.success(), "undefined function call was accepted");
    }

    #[test]
    fn spec_file_has_expected_shape() {
        // Supplementary shape check (the pipeline test above is the
        // real validation): the file declares the 8 slice-1 specs.
        let src = std::fs::read_to_string("examples/self_host/codegen.morph").unwrap();
        assert!(src.contains("Codegen"), "missing module Codegen");
        let spec_decls = src.lines().filter(|l| l.starts_with("spec ")).count();
        assert_eq!(spec_decls, 8, "codegen.morph should declare 8 specs");
        assert!(src.matches("test \"").count() >= 25, "codegen.morph should carry in-spec tests");
    }
}
