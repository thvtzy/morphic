// ─────────────────────────────────────────────────────────────
//  M3 CROSS-VALIDATION HARNESS (test-only) — slices 1–3
//
//  Generalizes the M2 cross-validation harness
//  (src/m2_cross_validation.rs) to the Codegen module. For
//  every one of the 44 specs in
//  examples/self_host/codegen.morph (slice 1: 8 leaf mappings;
//  slice 2: 20 small template families; slice 3: 16 name/text
//  assembly specs) this harness runs the
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
//         single execution). Specs whose test syntax cannot
//         bind their inputs (rust_pattern_ctor_name,
//         rust_string_ownership, rust_param_text,
//         rust_generics_text, rust_signature, rust_test_case)
//         have no in-spec tests; their golden cases are the
//         proof file's pinned assertions for the same specs.
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
//         names × has_subs ∈ {false, true}; for slice 3:
//         rust_string_ownership over the full Int-code cross
//         product (-2..=30 × -2..=30 — its spec decides every
//         pair); the `++` equation templates over batteries
//         of representative strings / string tuples (the
//         equations are total, so every case is pinned);
//         rust_ident_suffix over its contract domain (the 51
//         keywords, which pin name++"_", plus already-
//         sanitized non-keyword names, which pin identity).
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
//  disagreements. Total specs are pinned on EVERY input of
//  their sweep: rust_keyword (every string decided),
//  rust_condition_wrap (its equation decides every Int; the
//  proof mirror states the 28 real IRNode kinds and the
//  equation extends it — false — to every other code, so
//  the whole -2..=30 band is pinned), rust_typed_form (same
//  shape: the proof mirror's domain is the 28 kinds, the
//  spec's third constraint extends 0 — delegate — to every
//  other code), rust_literal_bool and rust_test_name_form
//  (total over the finite Bool domain), rust_string_ownership
//  (total over Int × Int), and every `++` equation template
//  (total over its string inputs). For rust_pattern_ctor_name
//  the spec pins exactly the has_subs == false branch
//  (output == name, for every name); the has_subs == true
//  branch is template assembly the spec deliberately does
//  not state, so those cases are spec-silent and the
//  winner's actual outputs are sampled in the report.
//
//  rust_ident_suffix has a third case class. Its spec's
//  input contract is the SANITIZED string (the per-char
//  step's output: never empty, never containing a char the
//  step replaces, never starting with an ASCII digit).
//  Names needing char surgery (finding F5 — "empty-list",
//  "type ", "9lives", "a.b", "---", "ma tch", and the empty
//  string) are OUT OF CONTRACT for this spec: the suffix
//  step is not what decides them (sanitization is), so they
//  are executed and reported separately, and NEVER counted
//  as disagreements or as spec-silent cases.
//
//  The assembly mirrors of tests/m3_proof.rs
//  (composed behavior: nested types, joined patterns,
//  assembled constraints, the whole generate_rust output)
//  are golden-tested in Rust there; cross-validation here
//  judges exactly the pinned aspects the specs claim, per
//  spec, per row — for slice 3 that includes the template
//  equations themselves, whose winners ARE assemblies.
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
//  disagreement for that run — never silently dropped — with
//  ONE classified exception: a rustc failure at the frozen
//  emitter's RENDERING layer (compile_edge_kind, below).
//  The measured family: E0369 (Concat renders as Rust `+`
//  and the string-ownership rewrite never fires on Concat
//  operands — Phase 25 — so literal-first assemblies do not
//  compile), E0308 (the same bare-operand rendering for
//  `String + String`, and String-scrutinee matches rendered
//  without `.as_str()`), and parse-level failures (string
//  literals are emitted raw, contents unescaped, so a
//  literal containing `"` breaks the file's syntax). Such a
//  failure is recorded as COMPILE_EDGE — the winner was not
//  executed, its pinned agreement is not measurable this
//  run, and it is NOT a behavioral disagreement; the probe's
//  standalone winner-body compile check records the same
//  facts independently.
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
    /// and rust_test_name_form's complete domain); a string
    /// (rust_keyword and the single-String slice-3 templates);
    /// a (name, has_subs) pair (rust_pattern_ctor_name); an
    /// Int-code pair (rust_string_ownership); a String pair
    /// (rust_param_text); a (String, Bool) pair
    /// (rust_generics_text); or a String 4-tuple
    /// (rust_signature, rust_test_case).
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum In {
        Int(i64),
        Bool(bool),
        Text(&'static str),
        Pair(&'static str, bool),
        IntPair(i64, i64),
        StrPair(&'static str, &'static str),
        StrBool(&'static str, bool),
        Str4(&'static str, &'static str, &'static str, &'static str),
    }

    impl In {
        fn render(&self) -> String {
            match self {
                In::Int(i) => i.to_string(),
                In::Bool(b) => b.to_string(),
                In::Text(s) => format!("\"{}\"", s),
                In::Pair(n, h) => format!("(\"{}\", {})", n, h),
                In::IntPair(a, b) => format!("({}, {})", a, b),
                In::StrPair(a, b) => format!("(\"{}\", \"{}\")", a, b),
                In::StrBool(s, b) => format!("(\"{}\", {})", s, b),
                In::Str4(a, b, c, d) => {
                    format!("(\"{}\", \"{}\", \"{}\", \"{}\")", a, b, c, d)
                }
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
        /// A computed expectation (the slice-3 template
        /// mirrors build their outputs: quoted contents,
        /// suffixed names, assembled lines).
        Owned(String),
        /// Mirror abstains and the spec pins nothing: any winner
        /// output is spec-legal; recorded, never a disagreement.
        Unpinned,
        /// The input lies outside the spec's input contract
        /// (rust_ident_suffix on names needing char surgery —
        /// finding F5): the winner's output is recorded with
        /// the input, and the case counts in NO agreement
        /// bucket — not pinned, not spec-silent.
        OutOfContract,
    }

    impl Mirror {
        fn is_pinned(&self) -> bool {
            !matches!(self, Mirror::Unpinned | Mirror::OutOfContract)
        }

        fn is_out_of_contract(&self) -> bool {
            matches!(self, Mirror::OutOfContract)
        }

        fn agrees(&self, out: &Out) -> bool {
            match (self, out) {
                (Mirror::Bool(b), Out::Bool(o)) => b == o,
                (Mirror::Int(i), Out::Int(o)) => i == o,
                (Mirror::Text(s), Out::Text(o)) => s == o,
                (Mirror::Owned(s), Out::Text(o)) => s == o,
                _ => false,
            }
        }

        fn render(&self) -> String {
            match self {
                Mirror::Bool(b) => b.to_string(),
                Mirror::Int(i) => i.to_string(),
                Mirror::Text(s) => format!("\"{}\"", s),
                Mirror::Owned(s) => format!("\"{}\"", s),
                Mirror::Unpinned => "(spec silent)".to_string(),
                Mirror::OutOfContract => "(out of contract)".to_string(),
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

    // ── Slice-3 mirrors (same copying rule) ───────────────

    /// The keyword-suffix step over the SANITIZED string —
    /// the spec's input contract. A name in
    /// IDENT_OOC_NAMES needs char surgery (finding F5): the
    /// suffix step does not decide it, so the case is
    /// OutOfContract, counted in no agreement bucket.
    fn rust_ident_suffix(name: &str) -> Mirror {
        if IDENT_OOC_NAMES.contains(&name) {
            return Mirror::OutOfContract;
        }
        if is_rust_keyword(name) {
            Mirror::Owned(format!("{}_", name))
        } else {
            Mirror::Owned(name.to_string())
        }
    }

    fn rust_ident_empty(name: &str) -> Mirror {
        // The spec pins exactly the empty name; every
        // non-empty name is spec-silent here (the per-char
        // step is mirror-only, finding F5).
        if name.is_empty() {
            Mirror::Text("_")
        } else {
            Mirror::Unpinned
        }
    }

    fn rust_typed_form(kind: i64) -> Mirror {
        // The proof mirror's domain is the 28 IRNode kinds
        // (None outside); the spec's third constraint,
        // however, decides every other Int as delegate (0),
        // so under the pinned-by-constraint rule the whole
        // band is pinned — the rust_condition_wrap precedent.
        match kind {
            9 => Mirror::Int(1),
            22 => Mirror::Int(2),
            _ => Mirror::Int(0),
        }
    }

    fn rust_string_ownership(lit: i64, expected: i64) -> Mirror {
        // Total (the proof mirror is a plain bool function):
        // owned exactly at (String literal, String expectation).
        Mirror::Bool(lit == 3 && expected == 3)
    }

    /// The emitter's string escaping, hand-mirrored
    /// (Phase 29) — a copy of the proof file's mirror of
    /// the same rule (the harnesses mirror independently
    /// by design).
    fn escape_rust_str_mirror(s: &str) -> String {
        let mut out = String::new();
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c.is_control() => {
                    out.push_str(&format!("\\u{{{:x}}}", c as u32));
                }
                c => out.push(c),
            }
        }
        out
    }

    /// Whether escaping is the identity on these contents
    /// — the subdomain on which the literal specs'
    /// equations are exact (Phase 29).
    fn escape_is_identity(s: &str) -> bool {
        !s.chars().any(|c| c == '"' || c == '\\' || c.is_control())
    }

    fn rust_string_literal_text(contents: &str) -> Mirror {
        // Phase 29: the spec equation pins the wrap only
        // where escaping changes nothing. Off that
        // subdomain the escaping rule is mirror-pinned in
        // tests/m3_proof.rs and the spec is silent here —
        // the winner body implements the equation as
        // written, and the equation cannot state the
        // per-character escape (F5).
        if escape_is_identity(contents) {
            Mirror::Owned(format!(
                "\"{}\"",
                escape_rust_str_mirror(contents)
            ))
        } else {
            Mirror::Unpinned
        }
    }

    fn rust_string_literal_owned_text(contents: &str) -> Mirror {
        match rust_string_literal_text(contents) {
            Mirror::Owned(bare) => {
                Mirror::Owned(format!("{}.to_string()", bare))
            }
            other => other,
        }
    }

    fn rust_doc_line(line: &str) -> Mirror {
        Mirror::Owned(format!("//! {}\n", line))
    }

    fn rust_param_text(name: &str, ty: &str) -> Mirror {
        Mirror::Owned(format!("{}: {}", name, ty))
    }

    fn rust_generics_text(joined: &str, present: bool) -> Mirror {
        Mirror::Owned(if present {
            format!("<{}>", joined)
        } else {
            String::new()
        })
    }

    fn rust_signature(name: &str, generics: &str, params: &str, ret: &str) -> Mirror {
        Mirror::Owned(format!(
            "pub fn {}{}({}) -> {} {{\n",
            name, generics, params, ret
        ))
    }

    fn rust_precondition_assert(rendered: &str) -> Mirror {
        Mirror::Owned(format!(
            "    debug_assert!({}, \"Precondition violated\");\n",
            rendered
        ))
    }

    fn rust_precondition_block(asserts: &str) -> Mirror {
        Mirror::Owned(format!(
            "#[cfg(debug_assertions)]\n{{\n{}}}\n\n",
            asserts
        ))
    }

    fn rust_test_name_form(has_name: bool) -> Mirror {
        Mirror::Int(if has_name { 0 } else { 1 })
    }

    fn rust_test_case(
        test_name: &str,
        fn_name: &str,
        input_text: &str,
        expected_text: &str,
    ) -> Mirror {
        Mirror::Owned(format!(
            "    #[test]\n    fn {}() {{\n        let result = {}({});\n        assert_eq!(result, {});\n    }}\n",
            test_name, fn_name, input_text, expected_text
        ))
    }

    fn rust_test_module(tests: &str) -> Mirror {
        Mirror::Owned(format!(
            "\n#[cfg(test)]\nmod tests {{\n    use super::*;\n\n{}}}\n",
            tests
        ))
    }

    fn rust_file_fixed_text(piece: i64) -> Mirror {
        match piece {
            0 => Mirror::Text("}\n"),
            1 => Mirror::Text("\n"),
            _ => Mirror::Unpinned,
        }
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
            ("rust_ident_suffix", In::Text(s)) => rust_ident_suffix(s),
            ("rust_ident_empty", In::Text(s)) => rust_ident_empty(s),
            ("rust_typed_form", In::Int(a)) => rust_typed_form(*a),
            ("rust_string_ownership", In::IntPair(a, b)) => rust_string_ownership(*a, *b),
            ("rust_string_literal_text", In::Text(s)) => rust_string_literal_text(s),
            ("rust_string_literal_owned_text", In::Text(s)) => {
                rust_string_literal_owned_text(s)
            }
            ("rust_doc_line", In::Text(s)) => rust_doc_line(s),
            ("rust_param_text", In::StrPair(n, t)) => rust_param_text(n, t),
            ("rust_generics_text", In::StrBool(j, p)) => rust_generics_text(j, *p),
            ("rust_signature", In::Str4(n, g, p, r)) => rust_signature(n, g, p, r),
            ("rust_precondition_assert", In::Text(s)) => rust_precondition_assert(s),
            ("rust_precondition_block", In::Text(s)) => rust_precondition_block(s),
            ("rust_test_name_form", In::Bool(b)) => rust_test_name_form(*b),
            ("rust_test_case", In::Str4(t, f, i, e)) => rust_test_case(t, f, i, e),
            ("rust_test_module", In::Text(s)) => rust_test_module(s),
            ("rust_file_fixed_text", In::Int(a)) => rust_file_fixed_text(*a),
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

    /// Every spec in the module, in file order (slices 1–3).
    const ALL_SPECS: [&str; 44] = [
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
        "rust_ident_suffix",
        "rust_ident_empty",
        "rust_typed_form",
        "rust_string_ownership",
        "rust_string_literal_text",
        "rust_string_literal_owned_text",
        "rust_doc_line",
        "rust_param_text",
        "rust_generics_text",
        "rust_signature",
        "rust_precondition_assert",
        "rust_precondition_block",
        "rust_test_name_form",
        "rust_test_case",
        "rust_test_module",
        "rust_file_fixed_text",
    ];

    /// The Int-code specs swept over the full -2..=30 band
    /// (every spec whose single input is an Int code: 7 from
    /// slice 1, 18 from slice 2, 2 from slice 3).
    const INT_BAND_SPECS: [&str; 27] = [
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
        "rust_typed_form",
        "rust_file_fixed_text",
    ];

    /// Constructor names for the rust_pattern_ctor_name grid:
    /// the proof file's golden names ("Some", "") plus
    /// representative rendered constructor names from the
    /// assembly mirrors' golden tests.
    const CTOR_NAMES: [&str; 7] = ["Some", "None", "Ok", "Err", "Pair", "Leaf", ""];

    // ── Slice-3 sweep batteries ────────────────────────────

    /// Already-sanitized NON-keyword names for
    /// rust_ident_suffix: legitimate outputs of the per-char
    /// step (letters/digits/underscore, Unicode letters kept,
    /// a leading non-ASCII digit kept unprefixed) that are
    /// not keywords — the spec pins identity for each.
    const IDENT_VALID_NAMES: [&str; 12] = [
        "sort",
        "type_",
        "_",
        "_x",
        "a1_b",
        "empty_list",
        "caf\u{e9}", // "café": Unicode letters pass sanitization untouched
        "test_0",
        "Self_",
        "x",
        "while_",
        "\u{663}x", // leading Arabic-Indic digit: kept, no prefix (ASCII-only check)
    ];

    /// Names needing char surgery (finding F5) — OUT OF
    /// CONTRACT for rust_ident_suffix, whose input is the
    /// sanitized string (never empty, never containing a
    /// replaced char, never starting with an ASCII digit).
    /// Executed and reported separately; counted in no
    /// agreement bucket.
    const IDENT_OOC_NAMES: [&str; 7] = [
        "empty-list", // hyphen is replaced by sanitization
        "type ",      // trailing space is replaced
        "9lives",     // leading ASCII digit gets a '_' prefix
        "a.b",        // dot is replaced
        "---",        // every char is replaced
        "ma tch",     // interior space is replaced
        "",           // the sanitized string is never empty
    ];

    /// String contents battery for the two literal-template
    /// equations. The battery itself is unchanged (9 cases),
    /// but since Phase 29 the three cases whose escaping is
    /// non-identity (`a"b`, `a\b`, `line\nbreak`) are
    /// spec-silent for both literal specs: the spec
    /// equations pin the wrap only on the identity
    /// subdomain, and the escaping rule itself is
    /// mirror-pinned in tests/m3_proof.rs.
    const STRING_CONTENTS: [&str; 9] = [
        "",
        "Int",
        "x",
        "a\"b",
        "a\\b",
        "Vec<i64>",
        "with space",
        "caf\u{e9}",
        "line\nbreak",
    ];

    /// Doc lines for rust_doc_line (total equation).
    const DOC_LINES: [&str; 4] = ["Sorts a list", "", "  indented", "one"];

    /// (name, type) pairs for rust_param_text (total equation).
    const PARAM_PAIRS: [(&str, &str); 5] = [
        ("a", "i64"),
        ("list", "Vec<String>"),
        ("x", "String"),
        ("", ""),
        ("name", "()"),
    ];

    /// (joined, present) cases for rust_generics_text (total:
    /// both Bool branches are decided by the constraints).
    const GENERICS_CASES: [(&str, bool); 6] = [
        ("T", true),
        ("T, U", true),
        ("", true),
        ("T", false),
        ("", false),
        ("ignored", false),
    ];

    /// Signature 4-tuples for rust_signature (total equation).
    const SIGNATURE_CASES: [(&str, &str, &str, &str); 3] = [
        ("add", "", "a: i64, b: i64", "i64"),
        ("sort", "<T>", "list: Vec<T>", "Vec<T>"),
        ("f", "", "", "()"),
    ];

    /// Rendered-constraint texts for rust_precondition_assert.
    const ASSERT_TEXTS: [&str; 4] = ["x > 0", "true", "a", "len > 0"];

    /// Pre-joined assert blocks for rust_precondition_block.
    const BLOCK_INPUTS: [&str; 3] = [
        "    debug_assert!(true, \"Precondition violated\");\n",
        "",
        "    debug_assert!(a, \"Precondition violated\");\n    debug_assert!(b, \"Precondition violated\");\n",
    ];

    /// Test-block 4-tuples for rust_test_case (total equation).
    const TEST_CASES: [(&str, &str, &str, &str); 2] = [
        ("adds", "add", "3", "7"),
        ("names", "f", "\"Int\"", "\"Int\""),
    ];

    /// Pre-joined test blocks for rust_test_module.
    const MODULE_INPUTS: [&str; 3] = [
        "X",
        "",
        "    #[test]\n    fn adds() {\n        let result = add(3);\n        assert_eq!(result, 7);\n    }\n",
    ];

    /// Inputs for rust_ident_empty: the pinned empty name
    /// plus non-empty names (spec-silent here).
    const IDENT_EMPTY_INPUTS: [&str; 5] = ["", "x", "type", "sort", "_"];

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
            "rust_ident_suffix" => KEYWORD_POSITIVES
                .iter()
                .chain(IDENT_VALID_NAMES.iter())
                .chain(IDENT_OOC_NAMES.iter())
                .map(|s| In::Text(s))
                .collect(),
            "rust_ident_empty" => IDENT_EMPTY_INPUTS.iter().map(|s| In::Text(s)).collect(),
            "rust_string_ownership" => (-2..=30)
                .flat_map(|lit| (-2..=30).map(move |expected| In::IntPair(lit, expected)))
                .collect(),
            "rust_string_literal_text" | "rust_string_literal_owned_text" => {
                STRING_CONTENTS.iter().map(|s| In::Text(s)).collect()
            }
            "rust_doc_line" => DOC_LINES.iter().map(|s| In::Text(s)).collect(),
            "rust_param_text" => PARAM_PAIRS
                .iter()
                .map(|(n, t)| In::StrPair(n, t))
                .collect(),
            "rust_generics_text" => GENERICS_CASES
                .iter()
                .map(|(j, p)| In::StrBool(j, *p))
                .collect(),
            "rust_signature" => SIGNATURE_CASES
                .iter()
                .map(|(n, g, p, r)| In::Str4(n, g, p, r))
                .collect(),
            "rust_precondition_assert" => {
                ASSERT_TEXTS.iter().map(|s| In::Text(s)).collect()
            }
            "rust_precondition_block" => {
                BLOCK_INPUTS.iter().map(|s| In::Text(s)).collect()
            }
            "rust_test_name_form" => vec![In::Bool(false), In::Bool(true)],
            "rust_test_case" => TEST_CASES
                .iter()
                .map(|(t, f, i, e)| In::Str4(t, f, i, e))
                .collect(),
            "rust_test_module" => MODULE_INPUTS.iter().map(|s| In::Text(s)).collect(),
            s if INT_BAND_SPECS.contains(&s) => (-2..=30).map(In::Int).collect(),
            _ => panic!("no sweep defined for spec {}", spec),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Golden cases — the in-spec tests of codegen.morph (the
    //  case list only; expectations come from the mirrors,
    //  which m3_proof's golden tests pin to these same cases).
    //  Specs whose inputs the test syntax cannot bind
    //  (rust_pattern_ctor_name, rust_string_ownership,
    //  rust_param_text, rust_generics_text, rust_signature,
    //  rust_test_case) have no in-spec tests; their golden
    //  cases are the proof file's pinned assertions for the
    //  same specs (see their arms below).
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
            // ── slice 3 ──
            // tests: type/Self/try suffix; sort, type_, _ pass through
            "rust_ident_suffix" => ["type", "Self", "try", "sort", "type_", "_"]
                .iter()
                .map(|s| In::Text(s))
                .collect(),
            // test: empty-name
            "rust_ident_empty" => vec![In::Text("")],
            // tests: literal-check 9, typed-reenter 22, binop 11, var 10
            "rust_typed_form" => ints(&[9, 22, 11, 10]),
            // No in-spec tests (tuple inputs cannot be bound);
            // golden cases are the proof file's ownership
            // grid: lit 0..=5 × expected {0,1,2,3,4,5,13,25,26}
            // (54 cases, all inside the full cross product).
            "rust_string_ownership" => {
                let mut v = Vec::new();
                for lit in 0..=5 {
                    for e in [0, 1, 2, 3, 4, 5, 13, 25, 26] {
                        v.push(In::IntPair(lit, e));
                    }
                }
                v
            }
            // tests: plain, empty, symbol text
            "rust_string_literal_text" => ["Int", "", "Vec<i64>"]
                .iter()
                .map(|s| In::Text(s))
                .collect(),
            // tests: owned, owned-empty
            "rust_string_literal_owned_text" => ["Int", ""]
                .iter()
                .map(|s| In::Text(s))
                .collect(),
            // tests: doc line, empty doc line
            "rust_doc_line" => ["Sorts a list", ""]
                .iter()
                .map(|s| In::Text(s))
                .collect(),
            // No in-spec tests; proof assertions: (a, i64),
            // (list, Vec<String>)
            "rust_param_text" => vec![
                In::StrPair("a", "i64"),
                In::StrPair("list", "Vec<String>"),
            ],
            // No in-spec tests; proof assertions: absent,
            // single, joined pair
            "rust_generics_text" => vec![
                In::StrBool("", false),
                In::StrBool("T", true),
                In::StrBool("T, U", true),
            ],
            // No in-spec tests; the proof file's three
            // signature assertions
            "rust_signature" => SIGNATURE_CASES
                .iter()
                .map(|(n, g, p, r)| In::Str4(n, g, p, r))
                .collect(),
            // tests: assert line, assert-true
            "rust_precondition_assert" => ["x > 0", "true"]
                .iter()
                .map(|s| In::Text(s))
                .collect(),
            // test: block over one assert
            "rust_precondition_block" => vec![In::Text(BLOCK_INPUTS[0])],
            // tests: named (true), unnamed (false)
            "rust_test_name_form" => vec![In::Bool(true), In::Bool(false)],
            // No in-spec tests; the proof file's two
            // test-block assertions
            "rust_test_case" => TEST_CASES
                .iter()
                .map(|(t, f, i, e)| In::Str4(t, f, i, e))
                .collect(),
            // test: module over the marker "X"
            "rust_test_module" => vec![In::Text("X")],
            // tests: fn-close 0, doc-blank 1
            "rust_file_fixed_text" => ints(&[0, 1]),
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
    //  has_subs: bool) -> String`; slice 3 adds (i64, i64)
    //  for rust_string_ownership, (String, String) for
    //  rust_param_text, (String, bool) for rust_generics_text,
    //  and (String, String, String, String) for
    //  rust_signature / rust_test_case.
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
            In::IntPair(_, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::IntPair(a, b) => format!("({}, {})", a, b),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(i64, i64)] = &[{vals}];\n    for &(__a, __b) in __inputs {{\n        let __out = {name}(__a, __b);\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::StrPair(_, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::StrPair(a, b) => {
                            format!("({}, {})", rust_str_literal(a), rust_str_literal(b))
                        }
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(&str, &str)] = &[{vals}];\n    for &(__a, __b) in __inputs {{\n        let __out = {name}(__a.to_string(), __b.to_string());\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::StrBool(_, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::StrBool(s, b) => format!("({}, {})", rust_str_literal(s), b),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(&str, bool)] = &[{vals}];\n    for &(__a, __b) in __inputs {{\n        let __out = {name}(__a.to_string(), __b);\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::Str4(_, _, _, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::Str4(a, b, c, d) => format!(
                            "({}, {}, {}, {})",
                            rust_str_literal(a),
                            rust_str_literal(b),
                            rust_str_literal(c),
                            rust_str_literal(d)
                        ),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(&str, &str, &str, &str)] = &[{vals}];\n    for &(__a, __b, __c, __d) in __inputs {{\n        let __out = {name}(__a.to_string(), __b.to_string(), __c.to_string(), __d.to_string());\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
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
        /// Winner outputs on out-of-contract inputs
        /// (rust_ident_suffix only), in sweep order: reported,
        /// counted in no agreement bucket.
        out_of_contract: Vec<(In, Out)>,
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
            let m = mirror(spec, input);
            if m.is_out_of_contract() {
                // Counted in no bucket.
            } else if m.is_pinned() {
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
            if m.is_out_of_contract() {
                ag.out_of_contract.push((input.clone(), out.clone()));
                continue;
            }
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
    #[ignore = "cross-validation harness: 3 randomized synthesis runs x 44 specs, compiles + executes winners; run explicitly with --ignored --nocapture"]
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
        let _ = writeln!(md, "# M3 cross-validation (slices 1–3): synthesized winners vs tests/m3_proof.rs mirrors");
        let _ = writeln!(md);
        let _ = writeln!(md, "Pipeline per spec per run: parse → typecheck → synthesize_with_module (opt 1) → Z3 verify_all (module-aware) → select_best → codegen (rust), then the winner is compiled with rustc and EXECUTED over the complete finite domain (Int codes -2..=30 for the 27 Int-code specs; rust_keyword: 51 positives + 24 near-miss negatives; rust_literal_bool and rust_test_name_form: the full Bool domain; rust_pattern_ctor_name: 7 names × has_subs; slice 3: rust_string_ownership over the full -2..=30 × -2..=30 cross product, the `++` equation templates over string batteries, rust_ident_suffix over its sanitized-string contract domain plus out-of-contract names reported separately). See src/m3_cross_validation.rs for sweep definitions and mirror semantics: a case is pinned exactly where the spec's constraints decide it.");
        let _ = writeln!(md, "A winner that fails to compile/run counts as total disagreement for that run (exec FAIL) — except a rustc failure at the frozen emitter's rendering layer (E0369 Concat-operand ownership, Phase 25; E0308 string-boundary rendering; parse-level unescaped-literal failures), which is recorded as COMPILE_EDGE: not executed, not a behavioral disagreement. Spec-silent (unpinned) cases are reported separately and never counted as disagreements; out-of-contract cases (rust_ident_suffix) are reported separately and counted in no bucket.");
        let _ = writeln!(md);
        let _ = writeln!(md, "| Run | Spec | Winner tpr | Z3 verified | Winner prov | Golden agree | Sweep pinned agree | Unpinned cases | Exec |");
        let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|");

        let mut details = String::new();
        let _ = writeln!(details, "\n## Spec-silent behavior + out-of-contract behavior + disagreements (first 5 pinned per spec per run)\n");

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
                        for (input, out) in &ag.out_of_contract {
                            println!(
                                "XVAL3-OOC|run={}|spec={}|input={}|winner={}",
                                run, name, input.render(), out.render()
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
                        if !ag.disagreements.is_empty()
                            || ag.unpinned > 0
                            || !ag.out_of_contract.is_empty()
                        {
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
                            if !ag.out_of_contract.is_empty() {
                                let _ = writeln!(
                                    details,
                                    "- out of contract (input outside the spec's contract; reported, counted nowhere): {} cases",
                                    ag.out_of_contract.len()
                                );
                                for (input, out) in &ag.out_of_contract {
                                    let _ = writeln!(
                                        details,
                                        "  - input {}: winner {}",
                                        input.render(),
                                        out.render()
                                    );
                                }
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
                        match compile_edge_kind(&e) {
                            Some(kind) => {
                                record_compile_edge(&mut md, &mut details, run, func, kind, &e, golden_total, pinned_total, unpinned_total);
                            }
                            None => {
                                record_failure(&mut md, &mut details, run, func, &e, golden_total, pinned_total, unpinned_total);
                            }
                        }
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

    /// Classify a runner rustc failure as an emitter
    /// RENDERING edge (the winner's IR body is not what
    /// failed — its verbatim Rust rendering is), or None for
    /// a failure that keeps the standing total-disagreement
    /// treatment. The three rendering edges measured on
    /// slice-3 winners (Phase 27), all properties of the
    /// frozen emitter:
    ///  - E0369: Concat renders as Rust `+` and the
    ///    string-ownership rewrite never fires on Concat
    ///    operands (Phase 25) — a literal-first assembly is
    ///    `&str + String`.
    ///  - E0308: the same bare-operand rendering one step
    ///    later (`String + String`, the RHS never borrowed),
    ///    and String-scrutinee matches rendered without
    ///    `.as_str()` (&str patterns against a String).
    ///  - parse-level: string literals are emitted RAW
    ///    (contents unescaped), so a literal containing `"`
    ///    breaks the file's syntax before any type check
    ///    (`"""`, `", "Precondition violated");"`).
    /// Anything else (a winner that runs but panics, wrong
    /// output count, a non-rendering type error) is NOT an
    /// edge and stays a disagreement.
    fn compile_edge_kind(err: &str) -> Option<&'static str> {
        if err.contains("E0369") {
            Some("E0369 concat-ownership")
        } else if err.contains("E0308") {
            Some("E0308 string-boundary rendering")
        } else if err.contains("mismatched closing delimiter")
            || err.contains("is unknown")
            || err.contains("unexpected token")
            || err.contains("expected one of")
        {
            Some("literal-rendering (unescaped string literal)")
        } else {
            None
        }
    }

    /// The classified exception to record_failure: the
    /// winner's verbatim code failed rustc at the rendering
    /// layer (see compile_edge_kind). The winner was NOT
    /// executed: pinned agreement is unmeasurable for this
    /// run and this is not a behavioral disagreement. The
    /// row shows the pinned totals as not-measured (—/N)
    /// rather than 0/N.
    #[allow(clippy::too_many_arguments)]
    fn record_compile_edge(
        md: &mut String,
        details: &mut String,
        run: usize,
        func: &spec::ast::FunctionSpec,
        kind: &str,
        why: &str,
        golden_total: usize,
        pinned_total: usize,
        unpinned_total: usize,
    ) {
        println!(
            "XVAL3|run={}|spec={}|exec=COMPILE_EDGE({})|golden=-/{}|sweep_pinned=-/{}|unpinned={}",
            run, func.name, kind, golden_total, pinned_total, unpinned_total
        );
        let _ = writeln!(
            md,
            "| {} | {} | - | - | - | -/{} | -/{} | {} | COMPILE_EDGE ({}): {} |",
            run, func.name, golden_total, pinned_total, unpinned_total, kind, why
        );
        let _ = writeln!(
            details,
            "### run {} — {} — COMPILE EDGE ({}, winner not executed)\n- {}\n- The winner's IR body as emitted by the frozen emitter does not compile at the rendering layer (see compile_edge_kind in src/m3_cross_validation.rs). Pinned agreement is not measurable for this run; this is not a behavioral disagreement.\n",
            run, func.name, kind, why
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
    /// only their rows; the equation specs, the Bool specs,
    /// rust_string_ownership and the `++` templates are
    /// total; rust_pattern_ctor_name pins exactly its
    /// has_subs == false half; rust_ident_suffix pins its
    /// contract domain — 51 keywords + 12 sanitized names —
    /// with 7 further sweep cases out of contract, counted
    /// in no bucket). The harness's bookkeeping stands or
    /// falls with these numbers, so they are pinned here,
    /// per spec, all 44.
    const PINNED_TOTALS: [(&str, usize); 44] = [
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
        ("rust_ident_suffix", 63),
        ("rust_ident_empty", 1),
        ("rust_typed_form", 33),
        ("rust_string_ownership", 1089),
        ("rust_string_literal_text", 6),
        ("rust_string_literal_owned_text", 6),
        ("rust_doc_line", 4),
        ("rust_param_text", 5),
        ("rust_generics_text", 6),
        ("rust_signature", 3),
        ("rust_precondition_assert", 4),
        ("rust_precondition_block", 3),
        ("rust_test_name_form", 2),
        ("rust_test_case", 2),
        ("rust_test_module", 3),
        ("rust_file_fixed_text", 2),
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
        // rust_typed_form: the spec's constraints are total
        // over Int (9 → 1, 22 → 2, everything else → 0), so
        // the whole band is pinned, nothing silent.
        let tf = sweep("rust_typed_form");
        assert_eq!(tf.len(), 33);
        let (_, pinned, unpinned) = totals("rust_typed_form", &tf);
        assert_eq!((pinned, unpinned), (33, 0));
        // rust_file_fixed_text: only pieces 0 and 1 pinned.
        let ff = sweep("rust_file_fixed_text");
        assert_eq!(ff.len(), 33);
        let (_, pinned, unpinned) = totals("rust_file_fixed_text", &ff);
        assert_eq!((pinned, unpinned), (2, 31));
        // rust_string_ownership: total over the full cross
        // product -2..=30 × -2..=30.
        let so = sweep("rust_string_ownership");
        assert_eq!(so.len(), 1089);
        let (_, pinned, unpinned) = totals("rust_string_ownership", &so);
        assert_eq!((pinned, unpinned), (1089, 0));
        // rust_ident_suffix: 51 keywords + 12 sanitized names
        // pinned; 7 char-surgery names out of contract —
        // counted in NO bucket (not pinned, not silent).
        let is = sweep("rust_ident_suffix");
        assert_eq!(is.len(), 70);
        let (_, pinned, unpinned) = totals("rust_ident_suffix", &is);
        assert_eq!((pinned, unpinned), (63, 0));
        let ooc = is
            .iter()
            .filter(|i| mirror("rust_ident_suffix", i).is_out_of_contract())
            .count();
        assert_eq!(ooc, 7);
        // rust_ident_empty: the empty name pinned, the four
        // non-empty names spec-silent.
        let ie = sweep("rust_ident_empty");
        assert_eq!(ie.len(), 5);
        let (_, pinned, unpinned) = totals("rust_ident_empty", &ie);
        assert_eq!((pinned, unpinned), (1, 4));
        // rust_test_name_form: total over the Bool domain.
        let tn = sweep("rust_test_name_form");
        assert_eq!(tn.len(), 2);
        let (_, pinned, unpinned) = totals("rust_test_name_form", &tn);
        assert_eq!((pinned, unpinned), (2, 0));
        // Grand totals across the module: 2196 cases per run,
        // 1533 pinned, 656 spec-silent, 7 out of contract.
        // (Phase 29: the literal specs' three special-contents
        // cases each moved from pinned to spec-silent —
        // 1539 - 6 pinned, 650 + 6 silent.)
        let total_cases: usize = ALL_SPECS.iter().map(|n| sweep(n).len()).sum();
        let total_pinned: usize = PINNED_TOTALS.iter().map(|(_, t)| t).sum();
        assert_eq!(total_cases, 2196);
        assert_eq!(total_pinned, 1533);
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
        //
        // Phase 28 flip: the composition-guard builder
        // (Bool-expression guards — a Bool parameter or a call
        // to an indexed Bool spec — with expression pins)
        // recognizes rust_ident_suffix and rust_generics_text,
        // so the iterated set grows to 30. The two remaining
        // slice-3 shapes stay unrecognized BY DESIGN and are
        // asserted as such below: rust_string_ownership's
        // two-parameter region rows and rust_ident_empty's
        // String-equality leaf family.
        const RECOGNIZED: [&str; 30] = [
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
            "rust_ident_suffix", "rust_generics_text",
        ];
        let functions = load_codegen_module();
        assert_eq!(functions.len(), 44, "module spec count");
        let index = engine::SpecIndex::from_specs(&functions);
        for func in &functions {
            if !RECOGNIZED.contains(&func.name.as_str()) {
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
        for name in ["rust_string_ownership", "rust_ident_empty"] {
            assert!(
                index.callee_body(name).is_none(),
                "{}: recorded non-goal must stay unrecognized",
                name
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

        // rust_typed_form is total by its constraints: an
        // out-of-band code is pinned 0 (delegate), so a
        // winner returning 1 there IS a disagreement.
        let inputs = ints(&[9, 22, 28]);
        let outs = vec![Out::Int(1), Out::Int(2), Out::Int(1)];
        let ag = compare("rust_typed_form", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 3);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::Int(28));

        // rust_string_ownership is total over Int × Int:
        // owned exactly at (3, 3); a false positive at
        // (3, 26) is a disagreement.
        let inputs = vec![In::IntPair(3, 3), In::IntPair(3, 26), In::IntPair(0, 3)];
        let outs = vec![Out::Bool(true), Out::Bool(true), Out::Bool(false)];
        let ag = compare("rust_string_ownership", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 3);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::IntPair(3, 26));

        // rust_ident_suffix: a keyword pins name++"_", a
        // sanitized non-keyword pins identity, and a
        // char-surgery name is OUT OF CONTRACT — recorded
        // with the winner's output, counted in no bucket,
        // whatever the winner returns there.
        let inputs = vec![
            In::Text("type"),
            In::Text("sort"),
            In::Text("empty-list"),
        ];
        let outs = vec![
            Out::Text("type_".to_string()),
            Out::Text("sort".to_string()),
            Out::Text("empty-list".to_string()),
        ];
        let ag = compare("rust_ident_suffix", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert_eq!(ag.unpinned, 0);
        assert_eq!(ag.out_of_contract.len(), 1);
        assert_eq!(
            ag.out_of_contract[0],
            (In::Text("empty-list"), Out::Text("empty-list".to_string()))
        );
        assert!(ag.disagreements.is_empty());
        // A missing suffix on a keyword IS a disagreement.
        let outs = vec![
            Out::Text("type".to_string()),
            Out::Text("sort".to_string()),
            Out::Text("anything".to_string()),
        ];
        let ag = compare("rust_ident_suffix", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::Text("type"));

        // A `++` template (rust_string_literal_text) after
        // Phase 29: on identity-subdomain contents the
        // mirror is the computed quoting; on contents whose
        // escaping is non-identity the spec is SILENT (the
        // equation pins only the wrap there; the escaping
        // rule is mirror-pinned in the proof file), so the
        // escaped winner output is counted in no bucket —
        // and neither would the raw wrap be.
        let inputs = vec![In::Text("Int"), In::Text("a\"b")];
        let outs = vec![
            Out::Text("\"Int\"".to_string()),
            Out::Text("\"a\\\"b\"".to_string()),
        ];
        let ag = compare("rust_string_literal_text", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 1);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.unpinned, 1);
        assert!(ag.disagreements.is_empty());
        // A wrong quoting on a pinned case IS a disagreement.
        let outs = vec![
            Out::Text("\"Int".to_string()),
            Out::Text("\"a\\\"b\"".to_string()),
        ];
        let ag = compare("rust_string_literal_text", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_agree, 0);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::Text("Int"));
    }

    #[test]
    fn xval3_compile_edge_classification() {
        // The three measured rendering edges classify; a
        // behavioral failure does not.
        assert_eq!(
            compile_edge_kind("rustc failed: error[E0369]: cannot add `String` to `&str`"),
            Some("E0369 concat-ownership")
        );
        assert_eq!(
            compile_edge_kind("rustc failed: error[E0308]: mismatched types"),
            Some("E0308 string-boundary rendering")
        );
        assert_eq!(
            compile_edge_kind("rustc failed: error: mismatched closing delimiter: `}`"),
            Some("literal-rendering (unescaped string literal)")
        );
        assert_eq!(
            compile_edge_kind("rustc failed: error: prefix `violated` is unknown"),
            Some("literal-rendering (unescaped string literal)")
        );
        assert_eq!(compile_edge_kind("runner exited with code 101"), None);
        assert_eq!(
            compile_edge_kind("expected 9 output lines, got 4"),
            None
        );
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

        // Int-pair spec (slice 3): the runner passes two
        // i64 codes, matching `pub fn rust_string_ownership(
        // lit: i64, expected: i64) -> bool`.
        let so_func = dummy_func("rust_string_ownership", TypeRef::Int, TypeRef::Bool);
        let inputs = vec![In::IntPair(3, 3), In::IntPair(3, 26), In::IntPair(0, 3)];
        let so = "pub fn rust_string_ownership(lit: i64, expected: i64) -> bool {\n    lit == 3 && expected == 3\n}\n";
        let outs = execute_winner(0, &so_func, so, &inputs).expect("ownership winner must run");
        let ag = compare("rust_string_ownership", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 3);
        assert_eq!(ag.sweep_pinned_agree, 3);
        assert!(ag.disagreements.is_empty());
        let so_wrong = "pub fn rust_string_ownership(lit: i64, expected: i64) -> bool {\n    lit == 3\n}\n";
        let outs = execute_winner(0, &so_func, so_wrong, &inputs).expect("wrong ownership winner must run");
        let ag = compare("rust_string_ownership", &inputs, &outs);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::IntPair(3, 26));

        // String-pair spec (slice 3): the runner passes two
        // owned Strings, matching `pub fn rust_param_text(
        // name: String, ty: String) -> String`.
        let pt_func = dummy_func("rust_param_text", TypeRef::String, TypeRef::String);
        let inputs = vec![In::StrPair("a", "i64"), In::StrPair("list", "Vec<String>")];
        let pt = "pub fn rust_param_text(name: String, ty: String) -> String {\n    format!(\"{}: {}\", name, ty)\n}\n";
        let outs = execute_winner(0, &pt_func, pt, &inputs).expect("param winner must run");
        let ag = compare("rust_param_text", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert!(ag.disagreements.is_empty());

        // (String, bool) spec (slice 3): the runner passes
        // an owned String and the bool directly, matching
        // `pub fn rust_generics_text(joined: String,
        // present: bool) -> String`.
        let gt_func = dummy_func("rust_generics_text", TypeRef::String, TypeRef::String);
        let inputs = vec![In::StrBool("T", true), In::StrBool("T", false)];
        let gt = "pub fn rust_generics_text(joined: String, present: bool) -> String {\n    if present { format!(\"<{}>\", joined) } else { String::new() }\n}\n";
        let outs = execute_winner(0, &gt_func, gt, &inputs).expect("generics winner must run");
        let ag = compare("rust_generics_text", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert!(ag.disagreements.is_empty());

        // String 4-tuple spec (slice 3): the runner passes
        // four owned Strings, matching `pub fn
        // rust_signature(name: String, generics: String,
        // params: String, ret: String) -> String`.
        let sig_func = dummy_func("rust_signature", TypeRef::String, TypeRef::String);
        let inputs = vec![In::Str4("f", "", "", "()")];
        let sig = "pub fn rust_signature(name: String, generics: String, params: String, ret: String) -> String {\n    format!(\"pub fn {}{}({}) -> {} {{\\n\", name, generics, params, ret)\n}\n";
        let outs = execute_winner(0, &sig_func, sig, &inputs).expect("signature winner must run");
        let ag = compare("rust_signature", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 1);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert!(ag.disagreements.is_empty());
    }
}
