// ─────────────────────────────────────────────────────────────
//  M3 CROSS-VALIDATION HARNESS (test-only) — slices 1–2
//
//  Generalizes the M2 cross-validation harness
//  (src/m2_cross_validation.rs) to the Codegen module. For
//  every one of the 28 specs in
//  examples/self_host/codegen.morph (slice 1: 8 leaf mappings;
//  slice 2: 20 small template families) this harness runs the
//  real pipeline at opt level 1 — parse → typecheck →
//  synthesize_with_module → Z3 verify_all (module-aware) →
//  select_best → codegen, exactly as `morphic build` and the
//  M3 synthesis probe do — then EXECUTES the winning
//  implementation and compares its behavior against the
//  hand-written mirrors of tests/m3_proof.rs over:
//
//    (i)  every golden case: the in-spec tests of
//         codegen.morph (the case list is in GOLDEN below;
//         every golden input is asserted to lie inside the
//         sweep, so golden results are read off the same
//         single execution). rust_pattern_ctor_name has no
//         in-spec tests (the test syntax cannot bind tuple
//         inputs); its golden cases are the proof file's
//         pinned assertions ("Some" and "" with no
//         sub-patterns).
//
//    (ii) the COMPLETE finite domains: for the Int-code specs,
//         every code -2..=30 — the full meaningful domain of
//         each table (largest: IRNode kinds 0..=27) plus a
//         margin on both sides; for rust_keyword (String
//         input), the full 51-word positive set plus a
//         negative set of near misses (the 2024 keyword "gen",
//         the contextual keyword "union", trailing/leading
//         spaces, uppercase variants, the empty string,
//         common non-keywords); for rust_literal_bool, the
//         complete Bool domain (false, true); for
//         rust_pattern_ctor_name, a grid of representative
//         names × has_subs ∈ {false, true}.
//
//  Pinned vs spec-silent is defined by the SPEC's constraints
//  (the Phase 23 rule, generalizing slice 1's partial-table
//  bookkeeping): a case is pinned exactly where a constraint
//  decides the output for that input. Mirror semantics
//  (copied from tests/m3_proof.rs, which is the source of
//  truth; if the two ever diverge, update this copy): a
//  mirror returns None exactly where the spec is silent —
//  the unpinned rows of every partial guard table and every
//  off-range code. Those cases are reported as "unpinned"
//  (winner output recorded) and NEVER counted as
//  disagreements. Two specs are total by a single equation
//  and therefore pinned on EVERY input: rust_keyword (every
//  string decided) and rust_condition_wrap (its equation
//  decides every Int; the proof mirror states the 28 real
//  IRNode kinds and the equation extends it — false — to
//  every other code, so the whole -2..=30 band is pinned).
//  rust_literal_bool is total over its finite Bool domain.
//  For rust_pattern_ctor_name the spec pins exactly the
//  has_subs == false branch (output == name, for every
//  name); the has_subs == true branch is template assembly
//  the spec deliberately does not state, so those cases are
//  spec-silent and the winner's actual outputs are sampled
//  in the report. The assembly mirrors of tests/m3_proof.rs
//  (composed behavior: nested types, joined patterns,
//  assembled constraints) are golden-tested in Rust there;
//  no synthesized winner in this slice IS an assembly — the
//  winners are the classification/affix/fixed-text functions
//  themselves — so cross-validation here judges exactly the
//  pinned aspects the specs claim, per spec, per row.
//
//  Execution mechanics mirror M2: the winner's generated Rust
//  (verbatim) is placed in a small runner binary whose main()
//  calls the winner on every sweep input and prints each
//  output with {:?}; the runner is compiled with
//  `rustc --edition 2021` and run once per spec, under the
//  same 10-second hang-proof deadline (a non-terminating
//  winner counts as total disagreement, never a hung
//  harness). A winner that fails to compile, run, or produce
//  exactly one parseable output per input counts as TOTAL
//  disagreement for that run — never silently dropped.
//
//  The harness asserts nothing about agreement — winners are
//  draw-dependent and disagreement is a measurement, not a
//  test failure. It runs 3 synthesis runs and writes the full
//  per-spec agreement matrix to target/m3-synthesis/
//  cross-validation.md. Run with:
//
//    cargo test -- --ignored --nocapture m3_cross_validation
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod xval3 {
    use crate::codegen;
    use crate::spec::{self, ast::TypeRef};
    use crate::synthesis::engine;
    use crate::synthesis::selector;
    use crate::verify;
    use std::collections::HashSet;
    use std::fmt::Write as _;
    use std::process::Command;
    use std::time::{Duration, Instant};

    fn provenance_name(p: &engine::Provenance) -> String {
        match p {
            engine::Provenance::LLMGenerated { .. } => "LLMGenerated".to_string(),
            engine::Provenance::Mutation { .. } => "Mutation".to_string(),
            engine::Provenance::Crossover { .. } => "Crossover".to_string(),
            engine::Provenance::TestInduced { .. } => "TestInduced".to_string(),
            engine::Provenance::ConstraintInduced { kind } => {
                format!("ConstraintInduced({})", kind)
            }
            engine::Provenance::Template { .. } => "Template".to_string(),
            engine::Provenance::HumanGuided => "HumanGuided".to_string(),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Value model
    // ─────────────────────────────────────────────────────────

    /// One sweep input: an Int code; a Bool (rust_literal_bool's
    /// complete domain); a string (rust_keyword); or a
    /// (name, has_subs) pair (rust_pattern_ctor_name, the
    /// module's one multi-input spec).
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum In {
        Int(i64),
        Bool(bool),
        Text(&'static str),
        Pair(&'static str, bool),
    }

    impl In {
        fn render(&self) -> String {
            match self {
                In::Int(i) => i.to_string(),
                In::Bool(b) => b.to_string(),
                In::Text(s) => format!("\"{}\"", s),
                In::Pair(n, h) => format!("(\"{}\", {})", n, h),
            }
        }
    }

    /// A winner's executed output, parsed from the runner.
    #[derive(Debug, Clone, PartialEq)]
    enum Out {
        Bool(bool),
        Int(i64),
        Text(String),
    }

    impl Out {
        fn render(&self) -> String {
            match self {
                Out::Bool(b) => b.to_string(),
                Out::Int(i) => i.to_string(),
                Out::Text(s) => format!("\"{}\"", s),
            }
        }
    }

    /// A mirror expectation for one input.
    #[derive(Debug, Clone, PartialEq)]
    enum Mirror {
        Bool(bool),
        Int(i64),
        Text(&'static str),
        /// Mirror abstains and the spec pins nothing: any winner
        /// output is spec-legal; recorded, never a disagreement.
        Unpinned,
    }

    impl Mirror {
        fn is_pinned(&self) -> bool {
            !matches!(self, Mirror::Unpinned)
        }

        fn agrees(&self, out: &Out) -> bool {
            match (self, out) {
                (Mirror::Bool(b), Out::Bool(o)) => b == o,
                (Mirror::Int(i), Out::Int(o)) => i == o,
                (Mirror::Text(s), Out::Text(o)) => s == o,
                _ => false,
            }
        }

        fn render(&self) -> String {
            match self {
                Mirror::Bool(b) => b.to_string(),
                Mirror::Int(i) => i.to_string(),
                Mirror::Text(s) => format!("\"{}\"", s),
                Mirror::Unpinned => "(spec silent)".to_string(),
            }
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Mirrors — copied from tests/m3_proof.rs (source of
    //  truth; that file's golden tests pin these functions to
    //  the full mapping tables). Option::None rows of the
    //  originals become Mirror::Unpinned, as documented in
    //  the module header.
    // ─────────────────────────────────────────────────────────

    fn rust_type_name(tag: i64) -> Mirror {
        match tag {
            0 => Mirror::Text("i64"),
            1 => Mirror::Text("f64"),
            2 => Mirror::Text("bool"),
            3 => Mirror::Text("String"),
            4 => Mirror::Text("char"),
            5 => Mirror::Text("()"),
            6 => Mirror::Text("i8"),
            7 => Mirror::Text("i16"),
            8 => Mirror::Text("i32"),
            9 => Mirror::Text("i64"),
            10 => Mirror::Text("u8"),
            11 => Mirror::Text("u16"),
            12 => Mirror::Text("u32"),
            13 => Mirror::Text("u64"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_binop_symbol(op: i64) -> Mirror {
        match op {
            0 => Mirror::Text("+"),
            1 => Mirror::Text("-"),
            2 => Mirror::Text("*"),
            3 => Mirror::Text("/"),
            4 => Mirror::Text("%"),
            5 => Mirror::Text("&&"),
            6 => Mirror::Text("||"),
            7 => Mirror::Text("^"),
            8 => Mirror::Text("=="),
            9 => Mirror::Text("!="),
            10 => Mirror::Text("<"),
            11 => Mirror::Text("<="),
            12 => Mirror::Text(">"),
            13 => Mirror::Text(">="),
            14 => Mirror::Text("<<"),
            15 => Mirror::Text(">>"),
            16 => Mirror::Text("+"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_unaryop_symbol(op: i64) -> Mirror {
        match op {
            0 => Mirror::Text("-"),
            1 => Mirror::Text("!"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_unaryop_method(op: i64) -> Mirror {
        match op {
            2 => Mirror::Text("abs"),
            3 => Mirror::Text("len"),
            8 => Mirror::Text("reverse"),
            9 => Mirror::Text("clone"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_unaryop_form(op: i64) -> Mirror {
        match op {
            0 | 1 => Mirror::Int(0),
            2 | 3 | 8 | 9 => Mirror::Int(1),
            4..=7 => Mirror::Int(2),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_collection_form(kind: i64) -> Mirror {
        match kind {
            0 => Mirror::Int(0),
            1 => Mirror::Int(1),
            2 => Mirror::Int(2),
            3 => Mirror::Int(3),
            4 => Mirror::Int(4),
            _ => Mirror::Unpinned,
        }
    }

    /// The emitter's exact 51-word set (tests/m3_proof.rs,
    /// copied verbatim). Total: every string is decided.
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

    fn spec_binop_symbol(op: i64) -> Mirror {
        match op {
            0 => Mirror::Text("+"),
            1 => Mirror::Text("-"),
            2 => Mirror::Text("*"),
            3 => Mirror::Text("/"),
            4 => Mirror::Text("?"),
            5 => Mirror::Text("&&"),
            6 => Mirror::Text("||"),
            7 => Mirror::Text("=="),
            8 => Mirror::Text("!="),
            9 => Mirror::Text("?"),
            10 => Mirror::Text("?"),
            11 => Mirror::Text("?"),
            12 => Mirror::Text("?"),
            13 => Mirror::Text("?"),
            _ => Mirror::Unpinned,
        }
    }

    // ── Slice-2 mirrors (same copying rule) ───────────────

    fn rust_type_form(tag: i64) -> Mirror {
        match tag {
            0..=13 => Mirror::Int(0),
            14 | 15 => Mirror::Int(1),
            16 | 17 | 19 | 24 | 25 => Mirror::Int(2),
            18 | 20 => Mirror::Int(3),
            21 => Mirror::Int(4),
            22 => Mirror::Int(5),
            23 => Mirror::Int(6),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_type_prefix(tag: i64) -> Mirror {
        match tag {
            16 => Mirror::Text("Vec<"),
            17 => Mirror::Text("HashSet<"),
            18 => Mirror::Text("HashMap<"),
            19 => Mirror::Text("Option<"),
            20 => Mirror::Text("Result<"),
            21 => Mirror::Text("("),
            22 => Mirror::Text("["),
            23 => Mirror::Text("fn("),
            24 => Mirror::Text("&"),
            25 => Mirror::Text("impl Iterator<Item = "),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_type_suffix(tag: i64) -> Mirror {
        match tag {
            16 | 17 | 18 | 19 | 20 | 25 => Mirror::Text(">"),
            21 => Mirror::Text(")"),
            22 => Mirror::Text("]"),
            23 | 24 => Mirror::Text(""),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_type_separator(tag: i64) -> Mirror {
        match tag {
            18 | 20 | 21 | 23 => Mirror::Text(", "),
            22 => Mirror::Text("; "),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_type_midfix(tag: i64) -> Mirror {
        match tag {
            23 => Mirror::Text(") -> "),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_literal_form(kind: i64) -> Mirror {
        match kind {
            0 => Mirror::Int(0),
            1 => Mirror::Int(1),
            2 => Mirror::Int(2),
            3 => Mirror::Int(3),
            4 => Mirror::Int(4),
            5 => Mirror::Int(5),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_literal_bool(value: bool) -> Mirror {
        // Total over the Bool domain (mirror returns &'static
        // str, never None): both inputs pinned.
        Mirror::Text(if value { "true" } else { "false" })
    }

    fn rust_literal_fixed_text(kind: i64) -> Mirror {
        match kind {
            5 => Mirror::Text("()"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_pattern_form(kind: i64) -> Mirror {
        match kind {
            0 => Mirror::Int(0),
            1 => Mirror::Int(1),
            2 => Mirror::Int(2),
            3 => Mirror::Int(3),
            4 => Mirror::Int(4),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_pattern_fixed_text(kind: i64) -> Mirror {
        match kind {
            0 => Mirror::Text("_"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_pattern_literal_form(kind: i64) -> Mirror {
        match kind {
            0 | 2 => Mirror::Int(0),
            1 | 4 | 5 => Mirror::Int(1),
            3 => Mirror::Int(2),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_pattern_literal_text(kind: i64) -> Mirror {
        match kind {
            1 | 4 | 5 => Mirror::Text("_"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_expr_form(kind: i64) -> Mirror {
        match kind {
            0 => Mirror::Int(0),
            1 => Mirror::Int(1),
            3 => Mirror::Int(2),
            4 => Mirror::Int(3),
            7 => Mirror::Int(4),
            8 => Mirror::Int(5),
            2 | 5 | 6 | 9 | 10 | 11 | 12 => Mirror::Int(6),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_expr_fixed_text(kind: i64) -> Mirror {
        match kind {
            2 | 5 | 6 | 9 | 10 | 11 | 12 => Mirror::Text("??"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_constraint_form(kind: i64) -> Mirror {
        match kind {
            0 | 1 | 2 | 3 | 4 | 6 | 10 => Mirror::Int(0),
            5 => Mirror::Int(4),
            7 => Mirror::Int(1),
            8 => Mirror::Int(2),
            9 => Mirror::Int(3),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_constraint_fixed_text(kind: i64) -> Mirror {
        match kind {
            0 | 1 | 2 | 3 | 4 | 6 | 10 => Mirror::Text("true"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_order_form(op: i64) -> Mirror {
        match op {
            0 => Mirror::Int(0),
            1 => Mirror::Int(1),
            2 | 3 => Mirror::Int(2),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_order_separator(op: i64) -> Mirror {
        match op {
            0 => Mirror::Text(" < "),
            1 => Mirror::Text(" <= "),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_condition_wrap(kind: i64) -> Mirror {
        // The proof mirror states Some(..) for the 28 real
        // IRNode kinds and None outside them; the spec's
        // single equation, however, decides EVERY Int
        // (output == kind ∈ {block-like set}), so under the
        // pinned-by-constraint rule the whole band is pinned:
        // true exactly on the block-like kinds, false on
        // every other code, in or out of range.
        Mirror::Bool(matches!(
            kind,
            1 | 2 | 3 | 4 | 5 | 6 | 13 | 14 | 24 | 26
        ))
    }

    fn mirror(spec: &str, input: &In) -> Mirror {
        match (spec, input) {
            ("rust_type_name", In::Int(a)) => rust_type_name(*a),
            ("rust_binop_symbol", In::Int(a)) => rust_binop_symbol(*a),
            ("rust_unaryop_symbol", In::Int(a)) => rust_unaryop_symbol(*a),
            ("rust_unaryop_method", In::Int(a)) => rust_unaryop_method(*a),
            ("rust_unaryop_form", In::Int(a)) => rust_unaryop_form(*a),
            ("rust_collection_form", In::Int(a)) => rust_collection_form(*a),
            ("rust_keyword", In::Text(s)) => Mirror::Bool(is_rust_keyword(s)),
            ("spec_binop_symbol", In::Int(a)) => spec_binop_symbol(*a),
            ("rust_type_form", In::Int(a)) => rust_type_form(*a),
            ("rust_type_prefix", In::Int(a)) => rust_type_prefix(*a),
            ("rust_type_suffix", In::Int(a)) => rust_type_suffix(*a),
            ("rust_type_separator", In::Int(a)) => rust_type_separator(*a),
            ("rust_type_midfix", In::Int(a)) => rust_type_midfix(*a),
            ("rust_literal_form", In::Int(a)) => rust_literal_form(*a),
            ("rust_literal_bool", In::Bool(b)) => rust_literal_bool(*b),
            ("rust_literal_fixed_text", In::Int(a)) => rust_literal_fixed_text(*a),
            ("rust_pattern_form", In::Int(a)) => rust_pattern_form(*a),
            ("rust_pattern_fixed_text", In::Int(a)) => rust_pattern_fixed_text(*a),
            ("rust_pattern_literal_form", In::Int(a)) => rust_pattern_literal_form(*a),
            ("rust_pattern_literal_text", In::Int(a)) => rust_pattern_literal_text(*a),
            ("rust_pattern_ctor_name", In::Pair(n, h)) => {
                if *h {
                    // With-sub-patterns branch: template
                    // assembly, unpinned in the spec.
                    Mirror::Unpinned
                } else {
                    Mirror::Text(n)
                }
            }
            ("rust_expr_form", In::Int(a)) => rust_expr_form(*a),
            ("rust_expr_fixed_text", In::Int(a)) => rust_expr_fixed_text(*a),
            ("rust_constraint_form", In::Int(a)) => rust_constraint_form(*a),
            ("rust_constraint_fixed_text", In::Int(a)) => rust_constraint_fixed_text(*a),
            ("rust_order_form", In::Int(a)) => rust_order_form(*a),
            ("rust_order_separator", In::Int(a)) => rust_order_separator(*a),
            ("rust_condition_wrap", In::Int(a)) => rust_condition_wrap(*a),
            _ => panic!("no mirror for spec {} with input {:?}", spec, input),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Sweeps — the complete finite domains. Int-code specs:
    //  -2..=30 covers every table's full code range (largest:
    //  TypeRef 0..=25) plus a margin on both sides; no
    //  cherry-picking — every code in the band is swept.
    //  rust_keyword: the 51 positives + near-miss negatives.
    // ─────────────────────────────────────────────────────────

    const KEYWORD_POSITIVES: [&str; 51] = [
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false",
        "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
        "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn",
        "abstract", "become", "box", "do", "final", "macro", "override", "priv",
        "typeof", "unsized", "virtual", "yield", "try",
    ];

    const KEYWORD_NEGATIVES: [&str; 24] = [
        "gen",     // 2024-edition keyword, absent from the emitter's set
        "union",   // contextual keyword, absent
        "async ",  // trailing space
        " async",  // leading space
        "as ",     // trailing space on a short keyword
        "SELF",    // uppercase variants
        "FN",
        "Fn",      // (also an in-spec test)
        "AS",
        "",        // empty string
        "sort",    // common non-keywords (also in-spec tests)
        "selfish",
        "main",
        "fn2",
        "crate_",
        "morphic",
        "int",
        "string",
        "keyword",
        "letx",
        "move2",
        "pub_",
        "try!",
        "returns",
    ];

    /// Every spec in the module, in file order (slices 1+2).
    const ALL_SPECS: [&str; 28] = [
        "rust_type_name",
        "rust_binop_symbol",
        "rust_unaryop_symbol",
        "rust_unaryop_method",
        "rust_unaryop_form",
        "rust_collection_form",
        "rust_keyword",
        "spec_binop_symbol",
        "rust_type_form",
        "rust_type_prefix",
        "rust_type_suffix",
        "rust_type_separator",
        "rust_type_midfix",
        "rust_literal_form",
        "rust_literal_bool",
        "rust_literal_fixed_text",
        "rust_pattern_form",
        "rust_pattern_fixed_text",
        "rust_pattern_literal_form",
        "rust_pattern_literal_text",
        "rust_pattern_ctor_name",
        "rust_expr_form",
        "rust_expr_fixed_text",
        "rust_constraint_form",
        "rust_constraint_fixed_text",
        "rust_order_form",
        "rust_order_separator",
        "rust_condition_wrap",
    ];

    /// The Int-code specs swept over the full -2..=30 band
    /// (every spec whose single input is an Int code: 7 from
    /// slice 1, 18 from slice 2).
    const INT_BAND_SPECS: [&str; 25] = [
        "rust_type_name",
        "rust_binop_symbol",
        "rust_unaryop_symbol",
        "rust_unaryop_method",
        "rust_unaryop_form",
        "rust_collection_form",
        "spec_binop_symbol",
        "rust_type_form",
        "rust_type_prefix",
        "rust_type_suffix",
        "rust_type_separator",
        "rust_type_midfix",
        "rust_literal_form",
        "rust_literal_fixed_text",
        "rust_pattern_form",
        "rust_pattern_fixed_text",
        "rust_pattern_literal_form",
        "rust_pattern_literal_text",
        "rust_expr_form",
        "rust_expr_fixed_text",
        "rust_constraint_form",
        "rust_constraint_fixed_text",
        "rust_order_form",
        "rust_order_separator",
        "rust_condition_wrap",
    ];

    /// Constructor names for the rust_pattern_ctor_name grid:
    /// the proof file's golden names ("Some", "") plus
    /// representative rendered constructor names from the
    /// assembly mirrors' golden tests.
    const CTOR_NAMES: [&str; 7] = ["Some", "None", "Ok", "Err", "Pair", "Leaf", ""];

    fn sweep(spec: &str) -> Vec<In> {
        match spec {
            "rust_keyword" => KEYWORD_POSITIVES
                .iter()
                .chain(KEYWORD_NEGATIVES.iter())
                .map(|s| In::Text(s))
                .collect(),
            "rust_literal_bool" => vec![In::Bool(false), In::Bool(true)],
            "rust_pattern_ctor_name" => CTOR_NAMES
                .iter()
                .flat_map(|n| [In::Pair(n, false), In::Pair(n, true)])
                .collect(),
            s if INT_BAND_SPECS.contains(&s) => (-2..=30).map(In::Int).collect(),
            _ => panic!("no sweep defined for spec {}", spec),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Golden cases — the in-spec tests of codegen.morph (the
    //  case list only; expectations come from the mirrors,
    //  which m3_proof's golden tests pin to these same cases).
    //  rust_pattern_ctor_name is the one exception: it has no
    //  in-spec tests, and its golden cases are the proof
    //  file's pinned assertions (see its arm below).
    // ─────────────────────────────────────────────────────────

    fn ints(vals: &[i64]) -> Vec<In> {
        vals.iter().map(|v| In::Int(*v)).collect()
    }

    fn golden(spec: &str) -> Vec<In> {
        match spec {
            // tests: int-name 0, string-name 3, unit-name 5, u64-name 13
            "rust_type_name" => ints(&[0, 3, 5, 13]),
            // tests: add 0, mod 4, xor 7, shl 14, concat 16
            "rust_binop_symbol" => ints(&[0, 4, 7, 14, 16]),
            // tests: neg 0, not 1
            "rust_unaryop_symbol" => ints(&[0, 1]),
            // tests: abs 2, len 3, clone 9
            "rust_unaryop_method" => ints(&[2, 3, 9]),
            // tests: neg-form 0, abs-form 2, head-form 4, clone-form 9
            "rust_unaryop_form" => ints(&[0, 2, 4, 9]),
            // tests: list 0, set 1, tuple 4
            "rust_collection_form" => ints(&[0, 1, 4]),
            // tests: fn, Self, sort, selfish, Fn
            "rust_keyword" => ["fn", "Self", "sort", "selfish", "Fn"]
                .iter()
                .map(|s| In::Text(s))
                .collect(),
            // tests: add 0, neq 8, mod-fallback 4, lt-fallback 9
            "spec_binop_symbol" => ints(&[0, 8, 4, 9]),
            // ── slice 2 ──
            // tests: scalar 0, name 14, list 16, map 18, tuple 21, array 22, function 23
            "rust_type_form" => ints(&[0, 14, 16, 18, 21, 22, 23]),
            // tests: vec 16, set 17, ref 24, stream 25
            "rust_type_prefix" => ints(&[16, 17, 24, 25]),
            // tests: vec 16, tuple 21, function 23, ref 24
            "rust_type_suffix" => ints(&[16, 21, 23, 24]),
            // tests: map 18, array 22, function 23
            "rust_type_separator" => ints(&[18, 22, 23]),
            // test: function-midfix 23
            "rust_type_midfix" => ints(&[23]),
            // tests: int 0, float 1, string 3, unit 5
            "rust_literal_form" => ints(&[0, 1, 3, 5]),
            // tests: true-text, false-text (the complete Bool domain)
            "rust_literal_bool" => vec![In::Bool(true), In::Bool(false)],
            // test: unit-text 5
            "rust_literal_fixed_text" => ints(&[5]),
            // tests: wildcard 0, literal 2, guard 4
            "rust_pattern_form" => ints(&[0, 2, 4]),
            // test: wildcard-text 0
            "rust_pattern_fixed_text" => ints(&[0]),
            // tests: int 0, float 1, string 3, unit 5
            "rust_pattern_literal_form" => ints(&[0, 1, 3, 5]),
            // tests: float 1, char 4, unit 5
            "rust_pattern_literal_text" => ints(&[1, 4, 5]),
            // No in-spec tests exist (tuple inputs cannot be
            // bound); golden cases are the proof file's pinned
            // assertions: ("Some", false) and ("", false). The
            // proof file's third assertion, ("Some", true) ->
            // None, is a spec-silent case by design and rides
            // the sweep as such, not as a golden case.
            "rust_pattern_ctor_name" => {
                vec![In::Pair("Some", false), In::Pair("", false)]
            }
            // tests: var 0, floatlit-fallback 2, call 7, binop 8, comprehension 12
            "rust_expr_form" => ints(&[0, 2, 7, 8, 12]),
            // tests: floatlit 2, unaryop 9
            "rust_expr_fixed_text" => ints(&[2, 9]),
            // tests: true 0, and 5, or-swallowed 6, not 7, order 9
            "rust_constraint_form" => ints(&[0, 5, 6, 7, 9]),
            // tests: true 0, or 6, predicate 10
            "rust_constraint_fixed_text" => ints(&[0, 6, 10]),
            // tests: lt 0, le 1, gt-fallback 2, ge-fallback 3
            "rust_order_form" => ints(&[0, 1, 2, 3]),
            // tests: lt 0, le 1
            "rust_order_separator" => ints(&[0, 1]),
            // tests: match-wraps 6, loop-wraps 24, literal-bare 9, hole-bare 0
            "rust_condition_wrap" => ints(&[6, 24, 9, 0]),
            _ => panic!("no golden cases defined for spec {}", spec),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Winner execution: build a runner binary around the
    //  winner's generated code (verbatim) and run it once over
    //  the whole input list. String-input specs are called
    //  with an owned String, matching the emitted signature
    //  (`pub fn rust_keyword(name: String) -> bool`); the
    //  pair-input spec is called with (String, bool), matching
    //  `pub fn rust_pattern_ctor_name(name: String,
    //  has_subs: bool) -> String`.
    // ─────────────────────────────────────────────────────────

    fn rust_str_literal(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                _ => out.push(c),
            }
        }
        out.push('"');
        out
    }

    fn build_runner(func: &spec::ast::FunctionSpec, winner_code: &str, inputs: &[In]) -> String {
        match &inputs[0] {
            In::Int(_) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::Int(v) => v.to_string(),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[i64] = &[{vals}];\n    for &__a in __inputs {{\n        let __out = {name}(__a);\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::Bool(_) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::Bool(b) => b.to_string(),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[bool] = &[{vals}];\n    for &__a in __inputs {{\n        let __out = {name}(__a);\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::Text(_) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::Text(s) => rust_str_literal(s),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[&str] = &[{vals}];\n    for &__a in __inputs {{\n        let __out = {name}(__a.to_string());\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::Pair(_, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::Pair(n, h) => format!("({}, {})", rust_str_literal(n), h),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(&str, bool)] = &[{vals}];\n    for &(__n, __h) in __inputs {{\n        let __out = {name}(__n.to_string(), __h);\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
        }
    }

    /// Inverse of Rust's `{:?}` formatting for the three output
    /// shapes the slice-1 specs produce (copied from the M2
    /// harness, including its complete simple-escape unescaper).
    fn parse_output(line: &str, return_type: &TypeRef) -> Result<Out, String> {
        match return_type {
            TypeRef::Bool => line
                .parse::<bool>()
                .map(Out::Bool)
                .map_err(|_| format!("unparseable Bool output {:?}", line)),
            TypeRef::Int => line
                .parse::<i64>()
                .map(Out::Int)
                .map_err(|_| format!("unparseable Int output {:?}", line)),
            TypeRef::String => {
                let inner = line
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .ok_or_else(|| format!("unquoted String output {:?}", line))?;
                let mut out = String::new();
                let mut chars = inner.chars();
                while let Some(c) = chars.next() {
                    if c != '\\' {
                        out.push(c);
                        continue;
                    }
                    match chars.next() {
                        Some('n') => out.push('\n'),
                        Some('t') => out.push('\t'),
                        Some('r') => out.push('\r'),
                        Some('0') => out.push('\0'),
                        Some('\\') => out.push('\\'),
                        Some('"') => out.push('"'),
                        Some(other) => {
                            out.push('\\');
                            out.push(other);
                        }
                        None => return Err("trailing backslash in String output".to_string()),
                    }
                }
                Ok(Out::Text(out))
            }
            other => Err(format!("unsupported return type {} for execution", other)),
        }
    }

    fn execute_winner(
        run_idx: usize,
        func: &spec::ast::FunctionSpec,
        winner_code: &str,
        inputs: &[In],
    ) -> Result<Vec<Out>, String> {
        execute_winner_with_timeout(
            run_idx,
            func,
            winner_code,
            inputs,
            Duration::from_secs(RUNNER_TIMEOUT_SECS),
        )
    }

    /// Per-runner wall-clock limit, inherited from the M2
    /// harness (Phase 16 lesson): winners can be
    /// non-terminating, and a hung winner must read as total
    /// disagreement, never hang the harness.
    const RUNNER_TIMEOUT_SECS: u64 = 10;

    fn execute_winner_with_timeout(
        run_idx: usize,
        func: &spec::ast::FunctionSpec,
        winner_code: &str,
        inputs: &[In],
        timeout: Duration,
    ) -> Result<Vec<Out>, String> {
        let dir = std::env::temp_dir().join(format!(
            "morphic_xval3_{}_{}_{}",
            std::process::id(),
            run_idx,
            func.name
        ));
        std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {}", e))?;
        let src_path = dir.join("main.rs");
        std::fs::write(&src_path, build_runner(func, winner_code, inputs))
            .map_err(|e| format!("write runner: {}", e))?;
        let bin_path = dir.join("runner");
        let compile = Command::new("rustc")
            .arg("--edition")
            .arg("2021")
            .arg(&src_path)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .map_err(|e| format!("spawn rustc: {}", e))?;
        if !compile.status.success() {
            let stderr = String::from_utf8_lossy(&compile.stderr);
            let errors: Vec<&str> = stderr
                .lines()
                .filter(|l| l.starts_with("error"))
                .take(3)
                .collect();
            let excerpt: Vec<&str> = if errors.is_empty() {
                stderr.lines().take(4).collect()
            } else {
                errors
            };
            return Err(format!("rustc failed: {}", excerpt.join(" | ")));
        }
        let stdout_path = dir.join("stdout.txt");
        let stdout_file =
            std::fs::File::create(&stdout_path).map_err(|e| format!("stdout file: {}", e))?;
        let mut child = Command::new(&bin_path)
            .stdout(std::process::Stdio::from(stdout_file))
            .spawn()
            .map_err(|e| format!("spawn runner: {}", e))?;
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        let _ = std::fs::remove_dir_all(&dir);
                        return Err(format!(
                            "runner timed out after {}s (non-terminating winner)",
                            timeout.as_secs()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(format!("wait runner: {}", e)),
            }
        };
        if !status.success() {
            return Err(format!("runner exited {}", status));
        }
        let stdout = std::fs::read_to_string(&stdout_path)
            .map_err(|_| "runner output is not UTF-8".to_string())?;
        let lines: Vec<&str> = stdout.lines().collect();
        if lines.len() != inputs.len() {
            return Err(format!(
                "expected {} output lines, got {}",
                inputs.len(),
                lines.len()
            ));
        }
        let outs: Result<Vec<Out>, String> = lines
            .iter()
            .map(|l| parse_output(l, &func.return_type))
            .collect();
        let outs = outs?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok(outs)
    }

    // ─────────────────────────────────────────────────────────
    //  Comparison
    // ─────────────────────────────────────────────────────────

    #[derive(Debug, Clone, PartialEq)]
    struct Disagreement {
        input: In,
        winner: Out,
        mirror: Mirror,
    }

    #[derive(Debug, Default)]
    struct Agreement {
        golden_agree: usize,
        golden_total: usize,
        sweep_pinned_agree: usize,
        sweep_pinned_total: usize,
        unpinned: usize,
        /// Distinct winner outputs on unpinned inputs, in the
        /// order first seen (usually one: the wildcard value).
        unpinned_winner_values: Vec<Out>,
        /// All pinned disagreements, in sweep order.
        disagreements: Vec<Disagreement>,
    }

    fn totals(spec: &str, inputs: &[In]) -> (usize, usize, usize) {
        let golden_cases = golden(spec);
        let golden_set: HashSet<&In> = golden_cases.iter().collect();
        let mut golden_total = 0;
        let mut pinned_total = 0;
        let mut unpinned = 0;
        for input in inputs {
            if golden_set.contains(input) {
                golden_total += 1;
            }
            if mirror(spec, input).is_pinned() {
                pinned_total += 1;
            } else {
                unpinned += 1;
            }
        }
        (golden_total, pinned_total, unpinned)
    }

    fn compare(spec: &str, inputs: &[In], outs: &[Out]) -> Agreement {
        assert_eq!(inputs.len(), outs.len(), "one output per input");
        let golden_cases = golden(spec);
        let golden_set: HashSet<&In> = golden_cases.iter().collect();
        let mut ag = Agreement::default();
        for (input, out) in inputs.iter().zip(outs.iter()) {
            let m = mirror(spec, input);
            let is_golden = golden_set.contains(input);
            if is_golden {
                ag.golden_total += 1;
            }
            if !m.is_pinned() {
                ag.unpinned += 1;
                if !ag.unpinned_winner_values.contains(out) {
                    ag.unpinned_winner_values.push(out.clone());
                }
                continue;
            }
            ag.sweep_pinned_total += 1;
            if m.agrees(out) {
                ag.sweep_pinned_agree += 1;
                if is_golden {
                    ag.golden_agree += 1;
                }
            } else {
                ag.disagreements.push(Disagreement {
                    input: input.clone(),
                    winner: out.clone(),
                    mirror: m,
                });
            }
        }
        ag
    }

    // ─────────────────────────────────────────────────────────
    //  The harness (ignored; run explicitly)
    // ─────────────────────────────────────────────────────────

    #[test]
    #[ignore = "cross-validation harness: 3 randomized synthesis runs x 28 specs, compiles + executes winners; run explicitly with --ignored --nocapture"]
    fn m3_cross_validation() {
        let root = env!("CARGO_MANIFEST_DIR");
        let out_dir = format!("{}/target/m3-synthesis", root);
        std::fs::create_dir_all(&out_dir).unwrap();

        let path = format!("{}/examples/self_host/codegen.morph", root);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = spec::parser::parse(&source).expect("parse must succeed");
        let checked = spec::typeck::check(parsed).expect("typeck must succeed");
        let functions = checked.spec.functions.clone();

        let mut md = String::new();
        let _ = writeln!(md, "# M3 cross-validation (slices 1–2): synthesized winners vs tests/m3_proof.rs mirrors");
        let _ = writeln!(md);
        let _ = writeln!(md, "Pipeline per spec per run: parse → typecheck → synthesize_with_module (opt 1) → Z3 verify_all (module-aware) → select_best → codegen (rust), then the winner is compiled with rustc and EXECUTED over the complete finite domain (Int codes -2..=30 for the 25 Int-code specs; rust_keyword: 51 positives + 24 near-miss negatives; rust_literal_bool: the full Bool domain; rust_pattern_ctor_name: 7 names × has_subs). See src/m3_cross_validation.rs for sweep definitions and mirror semantics: a case is pinned exactly where the spec's constraints decide it.");
        let _ = writeln!(md, "A winner that fails to compile/run counts as total disagreement for that run (exec FAIL). Spec-silent (unpinned) cases are reported separately and never counted as disagreements.");
        let _ = writeln!(md);
        let _ = writeln!(md, "| Run | Spec | Winner tpr | Z3 verified | Winner prov | Golden agree | Sweep pinned agree | Unpinned cases | Exec |");
        let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|");

        let mut details = String::new();
        let _ = writeln!(details, "\n## Spec-silent behavior + disagreements (first 5 pinned per spec per run)\n");

        for run in 1..=3usize {
            for func in &functions {
                let name = func.name.clone();
                let inputs = sweep(&name);
                let (golden_total, pinned_total, unpinned_total) = totals(&name, &inputs);

                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    engine::synthesize_with_module(func, 1, &functions)
                }));
                let candidates = match outcome {
                    Ok(Ok(c)) if !c.is_empty() => c,
                    Ok(Ok(_)) => {
                        record_failure(&mut md, &mut details, run, func, "synthesis returned no candidates", golden_total, pinned_total, unpinned_total);
                        continue;
                    }
                    Ok(Err(e)) => {
                        record_failure(&mut md, &mut details, run, func, &format!("synthesis error: {}", e), golden_total, pinned_total, unpinned_total);
                        continue;
                    }
                    Err(_) => {
                        record_failure(&mut md, &mut details, run, func, "synthesis panicked", golden_total, pinned_total, unpinned_total);
                        continue;
                    }
                };
                let verified = match verify::verifier::Verifier::with_specs(&functions)
                    .verify_all(candidates, func)
                {
                    Ok(v) => v,
                    Err(e) => {
                        record_failure(&mut md, &mut details, run, func, &format!("verify_all error: {}", e), golden_total, pinned_total, unpinned_total);
                        continue;
                    }
                };
                let winner = match selector::select_best(verified, func) {
                    Ok(w) => w,
                    Err(e) => {
                        record_failure(&mut md, &mut details, run, func, &format!("select_best error: {}", e), golden_total, pinned_total, unpinned_total);
                        continue;
                    }
                };
                let winner_code = match codegen::generate(&winner, "rust") {
                    Ok(c) => c,
                    Err(e) => {
                        record_failure(&mut md, &mut details, run, func, &format!("codegen error: {}", e), golden_total, pinned_total, unpinned_total);
                        continue;
                    }
                };
                let winner_path = format!("{}/{}_xval_run{}.rs", out_dir, name, run);
                let _ = std::fs::write(&winner_path, &winner_code);

                match execute_winner(run, func, &winner_code, &inputs) {
                    Ok(outs) => {
                        let ag = compare(&name, &inputs, &outs);
                        println!(
                            "XVAL3|run={}|spec={}|tpr={:.3}|verified={}|status={:?}|prov={}|golden={}/{}|sweep_pinned={}/{}|unpinned={}|exec=ok",
                            run, name, winner.scores.test_pass_ratio, winner.verified,
                            winner.verification_status,
                            provenance_name(&winner.provenance),
                            ag.golden_agree, ag.golden_total,
                            ag.sweep_pinned_agree, ag.sweep_pinned_total,
                            ag.unpinned
                        );
                        for d in ag.disagreements.iter().take(5) {
                            println!(
                                "XVAL3-DIS|run={}|spec={}|input={}|winner={}|mirror={}",
                                run, name, d.input.render(), d.winner.render(), d.mirror.render()
                            );
                        }
                        let _ = writeln!(
                            md,
                            "| {} | {} | {:.3} | {} | {} | {}/{} | {}/{} | {} | ok |",
                            run, name, winner.scores.test_pass_ratio, winner.verified,
                            provenance_name(&winner.provenance),
                            ag.golden_agree, ag.golden_total,
                            ag.sweep_pinned_agree, ag.sweep_pinned_total,
                            ag.unpinned
                        );
                        if !ag.disagreements.is_empty() || ag.unpinned > 0 {
                            let _ = writeln!(details, "### run {} — {}", run, name);
                            if ag.unpinned > 0 {
                                let vals: Vec<String> =
                                    ag.unpinned_winner_values.iter().map(|o| o.render()).collect();
                                let _ = writeln!(
                                    details,
                                    "- unpinned (spec silent): {} cases; winner returns {} there",
                                    ag.unpinned,
                                    vals.join(", ")
                                );
                            }
                            for d in ag.disagreements.iter().take(5) {
                                let _ = writeln!(
                                    details,
                                    "- input {}: winner {}, mirror {}",
                                    d.input.render(),
                                    d.winner.render(),
                                    d.mirror.render()
                                );
                            }
                            if ag.disagreements.len() > 5 {
                                let _ = writeln!(
                                    details,
                                    "- … and {} more pinned disagreements",
                                    ag.disagreements.len() - 5
                                );
                            }
                            let _ = writeln!(details);
                        }
                    }
                    Err(e) => {
                        record_failure(&mut md, &mut details, run, func, &e, golden_total, pinned_total, unpinned_total);
                    }
                }
            }
        }

        md.push_str(&details);
        let report_path = format!("{}/cross-validation.md", out_dir);
        std::fs::write(&report_path, &md).unwrap();
        println!("XVAL3-REPORT|written to {}", report_path);
        print!("{}", md);
    }

    #[allow(clippy::too_many_arguments)]
    fn record_failure(
        md: &mut String,
        details: &mut String,
        run: usize,
        func: &spec::ast::FunctionSpec,
        why: &str,
        golden_total: usize,
        pinned_total: usize,
        unpinned_total: usize,
    ) {
        println!(
            "XVAL3|run={}|spec={}|exec=FAIL({})|golden=0/{}|sweep_pinned=0/{}|unpinned={}",
            run, func.name, why, golden_total, pinned_total, unpinned_total
        );
        let _ = writeln!(
            md,
            "| {} | {} | - | - | - | 0/{} | 0/{} | {} | FAIL: {} |",
            run, func.name, golden_total, pinned_total, unpinned_total, why
        );
        let _ = writeln!(
            details,
            "### run {} — {} — EXECUTION FAILURE\n- {}\n",
            run, func.name, why
        );
    }

    // ─────────────────────────────────────────────────────────
    //  Mechanics tests (not ignored): the harness must provably
    //  detect disagreement, not just agreement; the seed
    //  builders must provably recognize the slice-1 shapes.
    // ─────────────────────────────────────────────────────────

    fn load_codegen_module() -> Vec<spec::ast::FunctionSpec> {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/self_host/codegen.morph", root);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = spec::parser::parse(&source).expect("parse must succeed");
        let checked = spec::typeck::check(parsed).expect("typeck must succeed");
        checked.spec.functions
    }

    fn dummy_func(name: &str, param_type: TypeRef, return_type: TypeRef) -> spec::ast::FunctionSpec {
        spec::ast::FunctionSpec {
            name: name.to_string(),
            doc: None,
            generics: Vec::new(),
            params: vec![spec::ast::Param {
                name: "p0".to_string(),
                ty: param_type,
                annotations: Default::default(),
            }],
            return_type,
            preconditions: Vec::new(),
            postconditions: Vec::new(),
            invariants: Vec::new(),
            complexity: None,
            resource: None,
            tests: Vec::new(),
            annotations: Default::default(),
        }
    }

    /// Pinned-case totals per spec over its sweep, computed
    /// from the spec constraints (partial guard tables pin
    /// only their rows; the two equation specs and the Bool
    /// spec are total; rust_pattern_ctor_name pins exactly
    /// its has_subs == false half). The harness's bookkeeping
    /// stands or falls with these numbers, so they are pinned
    /// here, per spec, all 28.
    const PINNED_TOTALS: [(&str, usize); 28] = [
        ("rust_type_name", 14),
        ("rust_binop_symbol", 17),
        ("rust_unaryop_symbol", 2),
        ("rust_unaryop_method", 4),
        ("rust_unaryop_form", 10),
        ("rust_collection_form", 5),
        ("rust_keyword", 75),
        ("spec_binop_symbol", 14),
        ("rust_type_form", 26),
        ("rust_type_prefix", 10),
        ("rust_type_suffix", 10),
        ("rust_type_separator", 5),
        ("rust_type_midfix", 1),
        ("rust_literal_form", 6),
        ("rust_literal_bool", 2),
        ("rust_literal_fixed_text", 1),
        ("rust_pattern_form", 5),
        ("rust_pattern_fixed_text", 1),
        ("rust_pattern_literal_form", 6),
        ("rust_pattern_literal_text", 3),
        ("rust_pattern_ctor_name", 7),
        ("rust_expr_form", 13),
        ("rust_expr_fixed_text", 7),
        ("rust_constraint_form", 11),
        ("rust_constraint_fixed_text", 7),
        ("rust_order_form", 4),
        ("rust_order_separator", 2),
        ("rust_condition_wrap", 33),
    ];

    #[test]
    fn xval3_golden_cases_lie_inside_the_sweeps() {
        for name in ALL_SPECS {
            let sw = sweep(name);
            for g in golden(name) {
                assert!(sw.contains(&g), "{}: golden {:?} not in sweep", name, g);
            }
            for input in &sw {
                let _ = mirror(name, input); // must not panic
            }
            let (_, pinned, _) = totals(name, &sw);
            let expected = PINNED_TOTALS
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, t)| *t)
                .unwrap_or_else(|| panic!("no pinned total recorded for {}", name));
            assert_eq!(pinned, expected, "{}: pinned total over its sweep", name);
        }
        // The keyword sweep is exactly 51 pinned positives +
        // 24 pinned negatives (the spec is total).
        let kw = sweep("rust_keyword");
        assert_eq!(kw.len(), 75);
        let (_, pinned, unpinned) = totals("rust_keyword", &kw);
        assert_eq!((pinned, unpinned), (75, 0));
        // Int sweeps: full band -2..=30 for every code spec.
        for name in INT_BAND_SPECS {
            assert_eq!(sweep(name).len(), 33, "{}: sweep band", name);
        }
        // rust_condition_wrap's equation is total over Int:
        // the whole band is pinned, nothing silent.
        let cw = sweep("rust_condition_wrap");
        let (_, pinned, unpinned) = totals("rust_condition_wrap", &cw);
        assert_eq!((pinned, unpinned), (33, 0));
        // rust_literal_bool: the complete Bool domain, total.
        let lb = sweep("rust_literal_bool");
        assert_eq!(lb.len(), 2);
        let (_, pinned, unpinned) = totals("rust_literal_bool", &lb);
        assert_eq!((pinned, unpinned), (2, 0));
        // rust_pattern_ctor_name: 7 names × 2; the
        // has_subs == false half is pinned, the assembly half
        // is spec-silent.
        let cn = sweep("rust_pattern_ctor_name");
        assert_eq!(cn.len(), 14);
        let (_, pinned, unpinned) = totals("rust_pattern_ctor_name", &cn);
        assert_eq!((pinned, unpinned), (7, 7));
        // Grand totals across the module: 916 cases per run,
        // 301 pinned, 615 spec-silent.
        let total_cases: usize = ALL_SPECS.iter().map(|n| sweep(n).len()).sum();
        let total_pinned: usize = PINNED_TOTALS.iter().map(|(_, t)| t).sum();
        assert_eq!(total_cases, 916);
        assert_eq!(total_pinned, 301);
    }

    #[test]
    fn xval3_seed_shapes_recognized_and_encodable() {
        // Diagnostic pin, generalized to the whole module
        // (Phase 23) and flipped by Phase 24: for ALL 28 specs,
        // the shared resolver's body builders recognize a body
        // (an equation RHS for rust_keyword /
        // rust_condition_wrap, a guard-table match for the
        // Int/Bool tables, the guarded-identity match for
        // rust_pattern_ctor_name) and the verifier can encode
        // that body in module context — the two mechanism steps
        // a ConstraintInduced seed needs before pool admission.
        //
        // Phase 23 asserted two exceptions UNRECOGNIZED, on
        // exact predicates:
        //  - rust_literal_bool: guard rows over a Bool param;
        //    guard_table_body admitted Int params only,
        //    guard_leaf/guard_row read Int constants and
        //    scanned an integer range for the pinned value.
        //  - rust_pattern_ctor_name: two params, a
        //    Bool-equality guard leaf, and an output pin that
        //    is a VARIABLE (output == name), where guard_row
        //    required the pin to convert to an IRNode::Literal.
        // Phase 24 generalized the builders — finite-domain
        // Bool guard tables (domain {false, true} enumerated
        // instead of an integer-interval scan) and
        // guarded-identity seeds (one guard param pinned by
        // literal-guard rows, the output pin exactly the
        // remaining parameter) — so the exceptions are gone
        // and this test now asserts recognition for the whole
        // module.
        //
        // Phase 26 scoping (the Phase 22 precedent): slice 3
        // added 16 specs to the module (44 total). Several
        // slice-3 shapes are EXPECTED to have no recognized
        // seed body — the call-guarded composition
        // (rust_ident_suffix), the mixed literal/concat rows
        // (rust_generics_text), the two-parameter leaf check
        // (rust_string_ownership) — and whether the `++`
        // equation templates seed is the next phase's
        // measurement question, not something to assert ahead
        // of the evidence. The recognition assertions below
        // therefore iterate the 28 slice-1+2 specs by name,
        // with their assertions unchanged; the module count
        // assertion moves to 44.
        const SLICE_1_2: [&str; 28] = [
            "rust_type_name", "rust_binop_symbol", "rust_unaryop_symbol",
            "rust_unaryop_method", "rust_unaryop_form", "rust_collection_form",
            "rust_keyword", "spec_binop_symbol", "rust_type_form",
            "rust_type_prefix", "rust_type_suffix", "rust_type_separator",
            "rust_type_midfix", "rust_literal_form", "rust_literal_bool",
            "rust_literal_fixed_text", "rust_pattern_form",
            "rust_pattern_fixed_text", "rust_pattern_literal_form",
            "rust_pattern_literal_text", "rust_pattern_ctor_name",
            "rust_expr_form", "rust_expr_fixed_text", "rust_constraint_form",
            "rust_constraint_fixed_text", "rust_order_form",
            "rust_order_separator", "rust_condition_wrap",
        ];
        let functions = load_codegen_module();
        assert_eq!(functions.len(), 44, "module spec count");
        let index = engine::SpecIndex::from_specs(&functions);
        for func in &functions {
            if !SLICE_1_2.contains(&func.name.as_str()) {
                continue;
            }
            let body = index.callee_body(&func.name).unwrap_or_else(|| {
                panic!("{}: no seed body recognized", func.name)
            });
            assert!(
                verify::verifier::body_is_encodable_with(func, &body, &index),
                "{}: recognized seed body is not encodable",
                func.name
            );
        }
    }

    #[test]
    fn xval3_compare_counts_agreement_and_disagreement() {
        // Int classification: one wrong value must be detected.
        let inputs = ints(&[0, 1, 4]);
        let outs = vec![Out::Int(0), Out::Int(9), Out::Int(4)];
        let ag = compare("rust_collection_form", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 3);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::Int(1));

        // String table with unpinned codes: pinned rows are
        // compared, unpinned ones recorded, not counted.
        let inputs = ints(&[0, 20, 100]);
        let outs = vec![
            Out::Text("i64".to_string()),
            Out::Text("Vec<T>".to_string()),
            Out::Text("".to_string()),
        ];
        let ag = compare("rust_type_name", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 1);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.unpinned, 2);
        assert_eq!(ag.unpinned_winner_values.len(), 2);
        assert!(ag.disagreements.is_empty());

        // rust_keyword is total: a false negative on a keyword
        // and a false positive on a near miss both count.
        let inputs = vec![In::Text("fn"), In::Text("gen"), In::Text("sort")];
        let outs = vec![Out::Bool(false), Out::Bool(true), Out::Bool(false)];
        let ag = compare("rust_keyword", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 3);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.disagreements.len(), 2);

        // rust_literal_bool is total over its Bool domain: a
        // wrong text on either value is a disagreement.
        let inputs = vec![In::Bool(true), In::Bool(false)];
        let outs = vec![Out::Text("true".to_string()), Out::Text("oops".to_string())];
        let ag = compare("rust_literal_bool", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::Bool(false));

        // rust_pattern_ctor_name: the no-sub-patterns branch
        // is pinned (output == name); the with-sub-patterns
        // branch is spec-silent — any winner output there is
        // recorded, never a disagreement.
        let inputs = vec![In::Pair("Some", false), In::Pair("Some", true)];
        let outs = vec![
            Out::Text("Some".to_string()),
            Out::Text("Some(x)".to_string()),
        ];
        let ag = compare("rust_pattern_ctor_name", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 1);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.unpinned, 1);
        assert_eq!(
            ag.unpinned_winner_values,
            vec![Out::Text("Some(x)".to_string())]
        );
        assert!(ag.disagreements.is_empty());

        // rust_condition_wrap's equation is total: an
        // out-of-range code is pinned false, so a winner
        // returning true there IS a disagreement.
        let inputs = ints(&[6, 28]);
        let outs = vec![Out::Bool(true), Out::Bool(true)];
        let ag = compare("rust_condition_wrap", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::Int(28));
    }

    #[test]
    fn xval3_runner_executes_and_detects() {
        // Int spec, end to end through the runner path.
        let int_func = dummy_func("rust_collection_form", TypeRef::Int, TypeRef::Int);
        let inputs = ints(&[0, 1, 2, 3, 4]);
        let correct = "pub fn rust_collection_form(kind: i64) -> i64 {\n    kind\n}\n";
        let outs = execute_winner(0, &int_func, correct, &inputs).expect("correct winner must run");
        let ag = compare("rust_collection_form", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_agree, ag.sweep_pinned_total);
        assert!(ag.disagreements.is_empty());

        let wrong = "pub fn rust_collection_form(kind: i64) -> i64 {\n    0\n}\n";
        let outs = execute_winner(0, &int_func, wrong, &inputs).expect("wrong winner must run");
        let ag = compare("rust_collection_form", &inputs, &outs);
        assert_eq!(ag.disagreements.len(), 4);

        // A non-terminating winner must fail, not hang.
        let hanging = "pub fn rust_collection_form(kind: i64) -> i64 {\n    loop {}\n}\n";
        let err = execute_winner_with_timeout(0, &int_func, hanging, &inputs, Duration::from_secs(1))
            .expect_err("non-terminating winner must fail, not hang");
        assert!(err.contains("timed out"), "got: {}", err);

        // String-input spec: the runner passes owned Strings,
        // matching the emitted signature.
        let kw_func = dummy_func("rust_keyword", TypeRef::String, TypeRef::Bool);
        let inputs = vec![In::Text("fn"), In::Text("gen"), In::Text("async ")];
        let kw = "pub fn rust_keyword(name: String) -> bool {\n    name == \"fn\"\n}\n";
        let outs = execute_winner(0, &kw_func, kw, &inputs).expect("keyword winner must run");
        let ag = compare("rust_keyword", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 3);
        assert_eq!(ag.sweep_pinned_agree, 3);
        assert!(ag.disagreements.is_empty());

        // Bool-input spec: the runner passes the bool
        // directly, matching `pub fn rust_literal_bool(value:
        // bool) -> String`.
        let lb_func = dummy_func("rust_literal_bool", TypeRef::Bool, TypeRef::String);
        let inputs = vec![In::Bool(false), In::Bool(true)];
        let lb = "pub fn rust_literal_bool(value: bool) -> String {\n    value.to_string()\n}\n";
        let outs = execute_winner(0, &lb_func, lb, &inputs).expect("bool winner must run");
        let ag = compare("rust_literal_bool", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert!(ag.disagreements.is_empty());

        // Pair-input spec: the runner passes (String, bool),
        // matching the emitted two-param signature; the
        // has_subs branch's output is spec-silent and only
        // sampled.
        let cn_func = dummy_func("rust_pattern_ctor_name", TypeRef::String, TypeRef::String);
        let inputs = vec![
            In::Pair("Some", false),
            In::Pair("", false),
            In::Pair("Some", true),
        ];
        let cn = "pub fn rust_pattern_ctor_name(name: String, has_subs: bool) -> String {\n    if has_subs { format!(\"{}()\", name) } else { name }\n}\n";
        let outs = execute_winner(0, &cn_func, cn, &inputs).expect("ctor winner must run");
        let ag = compare("rust_pattern_ctor_name", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert_eq!(ag.unpinned, 1);
        assert_eq!(
            ag.unpinned_winner_values,
            vec![Out::Text("Some()".to_string())]
        );
        assert!(ag.disagreements.is_empty());
    }
}
