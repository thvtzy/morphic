// ─────────────────────────────────────────────────────────────
//  M3 CROSS-VALIDATION HARNESS (test-only) — slices 1–4
//
//  Generalizes the M2 cross-validation harness
//  (src/m2_cross_validation.rs) to the Codegen module. For
//  every one of the 79 specs in
//  examples/self_host/codegen.morph (slice 1: 8 leaf mappings;
//  slice 2: 20 small template families; slice 3: 16 name/text
//  assembly specs; slice 4: 35 expression-walker specs) this
//  harness runs the
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
//         For slice 4: the single-Int classifications over
//         the full -2..=30 band; the (kind, lit) code specs
//         (rust_concat_left_owned, rust_concat_right_form,
//         rust_match_scrutinee_bare) over the complete
//         33 x 11 grid (kinds -2..=30 x lit kinds -2..=8 —
//         every reachable pair plus margin on both axes);
//         rust_match_scrutinee_form over the complete
//         Bool x Bool domain; and the template specs over
//         batteries of pre-rendered child texts (the
//         equations are total, so every battery case is
//         pinned), including inputs that are themselves
//         one template's output — the two-level nestings
//         the proof file's assembly mirror golden-tests,
//         stated per spec.
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
//  Slice 4 adds three bookkeeping shapes. (i) Composed
//  specs: rust_binop_infix_text states its equation over a
//  CALL to rust_binop_symbol, so it is pinned exactly
//  where the symbol table is (ops 0..=16) and spec-silent
//  where the table is (17, 18, off-band) — the composed
//  spec decides nothing its callee does not;
//  rust_condition_text composes rust_condition_wrap,
//  whose equation is total, so it is pinned on the whole
//  band. (ii) Catch-all classifications:
//  rust_concat_right_form's last row pins family 2 for
//  EVERY kind outside its named sets, and
//  rust_match_scrutinee_bare's rows pin false for every
//  kind outside {9, 22} — under the pinned-by-constraint
//  rule (the rust_condition_wrap / rust_typed_form
//  precedent) those off-band codes are pinned here even
//  though the proof mirrors abstain outside the reachable
//  kinds 0..=27; on every reachable input the two mirror
//  sets agree exactly, so the divergence is in stated
//  domain only, never in a decided value (see the two
//  mirrors' comments). (iii) rust_match_scrutinee_bare is
//  partial BY DESIGN (gap G2): kind 22 (Typed) is unpinned
//  for every lit — the flat codes cannot carry the inner
//  node the original recurses into — so its sweep reports
//  the pinned half (352 of 363) and the silent half
//  separately.
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
//  ONE classified exception: a rustc failure at the emitter's
//  RENDERING layer (compile_edge_kind, below).
//  The measured family (Phases 25/27/28; FIXED in Phase 29
//  under the user's unfreeze of exactly those three
//  defects): E0369 (Concat rendered as Rust `+`
//  and the string-ownership rewrite never fired on Concat
//  operands, so literal-first assemblies did not
//  compile), E0308 (the same bare-operand rendering for
//  `String + String`, and String-scrutinee matches rendered
//  without `.as_str()`), and parse-level failures (string
//  literals were emitted raw, contents unescaped, so a
//  literal containing `"` broke the file's syntax). The
//  classification is retained as a guard: such a
//  failure is recorded as COMPILE_EDGE — the winner was not
//  executed, its pinned agreement is not measurable this
//  run, and it is NOT a behavioral disagreement; the probe's
//  standalone winner-body compile check records the same
//  facts independently. Phase 29's re-measurement recorded
//  zero COMPILE_EDGE rows.
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
    /// Int-code pair (rust_string_ownership; slice 4's
    /// (kind, lit) code specs); a String pair (rust_param_text;
    /// slice 4's two-text templates); a (String, Bool) pair
    /// (rust_generics_text); or a String 4-tuple
    /// (rust_signature, rust_test_case). Slice 4 adds: an
    /// (Int, String) pair (the Int-keyed text templates and
    /// rust_condition_text), an (Int, String, String) triple
    /// (rust_binop_infix_text), a (Bool, String) pair
    /// (rust_break_text), a (Bool, String, String) triple
    /// (rust_concat_left_text), a Bool pair
    /// (rust_match_scrutinee_form), and a String triple (the
    /// three-text templates).
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
        IntStr(i64, &'static str),
        IntStrStr(i64, &'static str, &'static str),
        BoolStr(bool, &'static str),
        BoolStrStr(bool, &'static str, &'static str),
        BoolPair(bool, bool),
        Str3(&'static str, &'static str, &'static str),
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
                In::IntStr(a, s) => format!("({}, \"{}\")", a, s),
                In::IntStrStr(a, s, t) => format!("({}, \"{}\", \"{}\")", a, s, t),
                In::BoolStr(b, s) => format!("({}, \"{}\")", b, s),
                In::BoolStrStr(b, s, t) => format!("({}, \"{}\", \"{}\")", b, s, t),
                In::BoolPair(a, b) => format!("({}, {})", a, b),
                In::Str3(a, b, c) => format!("(\"{}\", \"{}\", \"{}\")", a, b, c),
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

    // ── Slice-4 mirrors (same copying rule) ───────────────
    //
    //  Divergence audit against tests/m3_proof.rs (the task
    //  of this extension): every mirror below is a literal
    //  transcription of its proof counterpart — the proof
    //  Option::None rows become Mirror::Unpinned, Some/plain
    //  values become the value — with exactly TWO stated-
    //  domain extensions, both following this harness's
    //  pinned-by-constraint precedent (rust_condition_wrap,
    //  rust_typed_form): rust_concat_right_form and
    //  rust_match_scrutinee_bare, whose specs textually
    //  decide codes outside the reachable kinds 0..=27
    //  where the proof mirrors abstain. On every input both
    //  mirrors state, they state the same value; the proof
    //  file's golden tests pin the proof copies to the
    //  emitter, and no reachable input is decided
    //  differently by the two sets. Two compositions are
    //  worth naming: rust_condition_text composes the
    //  TOTAL wrap mirror here (Bool for every code), where
    //  the proof composes the Option mirror (None -> bare);
    //  off-band the spec's wrap is false and both give the
    //  bare rendering, so they agree there too.
    //  rust_binop_infix_text's proof mirror unwraps the
    //  symbol (never called off-table); here the composed
    //  mirror abstains off-table instead — again the same
    //  values wherever both state one.

    fn rust_expr_binop_form(op: i64) -> Mirror {
        match op {
            0..=15 => Mirror::Int(0),
            16 => Mirror::Int(1),
            17 => Mirror::Int(2),
            18 => Mirror::Int(3),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_binop_infix_text(op: i64, l: &str, r: &str) -> Mirror {
        // Composed over the slice-1 symbol mirror exactly
        // as the walker consults the table: pinned exactly
        // where the table is (ops 0..=16), spec-silent
        // where it is not.
        match rust_binop_symbol(op) {
            Mirror::Text(sym) => Mirror::Owned(format!("({} {} {})", l, sym, r)),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_binop_append_text(l: &str, r: &str) -> Mirror {
        Mirror::Owned(format!("{}.push({})", l, r))
    }

    fn rust_binop_merge_text(l: &str, r: &str) -> Mirror {
        Mirror::Owned(format!("{}.extend({})", l, r))
    }

    fn rust_binop_concat_text(owned: &str, borrowed: &str) -> Mirror {
        Mirror::Owned(format!("({} + {})", owned, borrowed))
    }

    fn rust_concat_left_owned(kind: i64, lit: i64) -> Mirror {
        // Total (the proof mirror is a plain bool function,
        // and the spec's rows decide every pair the same
        // way): owned exactly at (Literal kind 9, String
        // lit kind 3).
        Mirror::Bool(kind == 9 && lit == 3)
    }

    fn rust_concat_left_text(owned: bool, rendered: &str, owned_text: &str) -> Mirror {
        Mirror::Owned(if owned {
            owned_text.to_string()
        } else {
            rendered.to_string()
        })
    }

    fn rust_concat_right_form(kind: i64, lit: i64) -> Mirror {
        // The spec is TOTAL over Int x Int: the (9, 3) row
        // pins 0, the ten block-like rows pin 1, and the
        // catch-all row pins 2 for every remaining pair —
        // including kinds outside the reachable 0..=27,
        // where the proof mirror returns None (it states
        // the walker's domain). Stated-domain extension
        // only; the two agree on every reachable pair.
        if kind == 9 && lit == 3 {
            Mirror::Int(0)
        } else if matches!(kind, 1 | 2 | 3 | 4 | 5 | 6 | 13 | 14 | 24 | 26) {
            Mirror::Int(1)
        } else {
            Mirror::Int(2)
        }
    }

    fn rust_concat_right_text(form: i64, rendered: &str) -> Mirror {
        match form {
            0 => Mirror::Owned(rendered.to_string()),
            1 => Mirror::Owned(format!("&({})", rendered)),
            2 => Mirror::Owned(format!("&{}", rendered)),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_unary_text(op: i64, inner: &str) -> Mirror {
        match op {
            0 => Mirror::Owned(format!("(- {})", inner)),
            1 => Mirror::Owned(format!("(! {})", inner)),
            2 => Mirror::Owned(format!("{}.abs()", inner)),
            3 => Mirror::Owned(format!("{}.len()", inner)),
            4 => Mirror::Owned(format!("({})[0]", inner)),
            5 => Mirror::Owned(format!("({})[1..].to_vec()", inner)),
            6 => Mirror::Owned(format!(
                "({})[..({}).len() - 1].to_vec()",
                inner, inner
            )),
            7 => Mirror::Owned(format!("({})[({}).len() - 1]", inner, inner)),
            8 => Mirror::Owned(format!("{}.reverse()", inner)),
            9 => Mirror::Owned(format!("{}.clone()", inner)),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_collection_prefix(kind: i64) -> Mirror {
        match kind {
            0 => Mirror::Text("vec!["),
            1 => Mirror::Text("std::collections::HashSet::from(["),
            2 => Mirror::Text("std::collections::HashMap::from(["),
            3 => Mirror::Text("["),
            4 => Mirror::Text("("),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_collection_suffix(kind: i64) -> Mirror {
        match kind {
            0 => Mirror::Text("]"),
            1 => Mirror::Text("])"),
            2 => Mirror::Text("])"),
            3 => Mirror::Text("]"),
            4 => Mirror::Text(")"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_map_text(over: &str, func: &str) -> Mirror {
        Mirror::Owned(format!("({}).iter().map({}).collect()", over, func))
    }

    fn rust_filter_text(over: &str, pred: &str) -> Mirror {
        Mirror::Owned(format!("({}).iter().filter({}).collect()", over, pred))
    }

    fn rust_fold_text(over: &str, initial: &str, operation: &str) -> Mirror {
        Mirror::Owned(format!("({}).iter().fold({}, {})", over, initial, operation))
    }

    fn rust_sort_text(coll: &str, cmp: &str) -> Mirror {
        Mirror::Owned(format!(
            "{{ let mut sorted = {}; sorted.sort_by({}); sorted }}",
            coll, cmp
        ))
    }

    fn rust_call_text(func: &str, args: &str) -> Mirror {
        Mirror::Owned(format!("{}({})", func, args))
    }

    fn rust_condition_text(kind: i64, rendered: &str) -> Mirror {
        // Composed over the slice-2 classification mirror,
        // which is total over Int, so this spec is pinned
        // on the whole band: wrapped exactly on the
        // block-like kinds.
        match rust_condition_wrap(kind) {
            Mirror::Bool(true) => Mirror::Owned(format!("({})", rendered)),
            _ => Mirror::Owned(rendered.to_string()),
        }
    }

    fn rust_if_text(cond: &str, then_text: &str, else_text: &str) -> Mirror {
        Mirror::Owned(format!(
            "if {} {{ {} }} else {{ {} }}",
            cond, then_text, else_text
        ))
    }

    fn rust_lambda_text(params: &str, body: &str) -> Mirror {
        Mirror::Owned(format!("|{}| {{ {} }}", params, body))
    }

    fn rust_match_arm_text(pat: &str, body: &str) -> Mirror {
        Mirror::Owned(format!("{} => {}", pat, body))
    }

    fn rust_match_text(scrutinee: &str, arms: &str) -> Mirror {
        Mirror::Owned(format!("match {} {{ {} }}", scrutinee, arms))
    }

    fn rust_match_scrutinee_bare(kind: i64, lit: i64) -> Mirror {
        // Partial BY DESIGN (gap G2): kind 22 (Typed) is
        // unpinned for every lit — its bareness is its
        // inner node's, which the flat codes cannot carry.
        // Every other pair IS decided by the spec's rows:
        // (9, 3) true, and false everywhere else, including
        // kinds outside the reachable 0..=27 (the
        // kind==9||kind==22 row pins false there), where
        // the proof mirror abstains — the same stated-
        // domain extension as rust_concat_right_form.
        if kind == 22 {
            Mirror::Unpinned
        } else {
            Mirror::Bool(kind == 9 && lit == 3)
        }
    }

    fn rust_match_scrutinee_form(has_string_pat: bool, bare: bool) -> Mirror {
        // Total over the Bool x Bool domain.
        Mirror::Int(if has_string_pat && !bare { 1 } else { 0 })
    }

    fn rust_match_scrutinee_text(form: i64, rendered: &str) -> Mirror {
        match form {
            0 => Mirror::Owned(rendered.to_string()),
            1 => Mirror::Owned(format!("{}.as_str()", rendered)),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_block_text(inner: &str) -> Mirror {
        Mirror::Owned(format!("{{ {} }}", inner))
    }

    fn rust_loop_text(body: &str) -> Mirror {
        Mirror::Owned(format!("loop {{\n{}}}", body))
    }

    fn rust_struct_def_text(name: &str, fields: &str, body: &str) -> Mirror {
        Mirror::Owned(format!("{{ struct {} {{ {} }} {} }}", name, fields, body))
    }

    fn rust_index_text(coll: &str, idx: &str) -> Mirror {
        Mirror::Owned(format!("({})[{}]", coll, idx))
    }

    fn rust_try_text(inner: &str) -> Mirror {
        Mirror::Owned(format!("({})?", inner))
    }

    fn rust_assign_text(target: &str, value: &str) -> Mirror {
        Mirror::Owned(format!("({} = {})", target, value))
    }

    fn rust_return_text(inner: &str) -> Mirror {
        Mirror::Owned(format!("(return {})", inner))
    }

    fn rust_break_text(has_value: bool, value: &str) -> Mirror {
        Mirror::Owned(if has_value {
            format!("(break {})", value)
        } else {
            "(break)".to_string()
        })
    }

    fn rust_hole_text(kind: i64) -> Mirror {
        match kind {
            0 => Mirror::Text("panic!(\"Hole not filled\")"),
            _ => Mirror::Unpinned,
        }
    }

    fn rust_unsupported_text(shape: &str) -> Mirror {
        Mirror::Owned(format!(
            "panic!(\"unsupported IR in expression position: {}\")",
            shape
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
            // ── slice 4 ──
            ("rust_expr_binop_form", In::Int(a)) => rust_expr_binop_form(*a),
            ("rust_binop_infix_text", In::IntStrStr(o, l, r)) => {
                rust_binop_infix_text(*o, l, r)
            }
            ("rust_binop_append_text", In::StrPair(l, r)) => {
                rust_binop_append_text(l, r)
            }
            ("rust_binop_merge_text", In::StrPair(l, r)) => rust_binop_merge_text(l, r),
            ("rust_binop_concat_text", In::StrPair(o, b)) => {
                rust_binop_concat_text(o, b)
            }
            ("rust_concat_left_owned", In::IntPair(k, l)) => {
                rust_concat_left_owned(*k, *l)
            }
            ("rust_concat_left_text", In::BoolStrStr(o, r, t)) => {
                rust_concat_left_text(*o, r, t)
            }
            ("rust_concat_right_form", In::IntPair(k, l)) => {
                rust_concat_right_form(*k, *l)
            }
            ("rust_concat_right_text", In::IntStr(f, r)) => {
                rust_concat_right_text(*f, r)
            }
            ("rust_unary_text", In::IntStr(o, i)) => rust_unary_text(*o, i),
            ("rust_collection_prefix", In::Int(a)) => rust_collection_prefix(*a),
            ("rust_collection_suffix", In::Int(a)) => rust_collection_suffix(*a),
            ("rust_map_text", In::StrPair(o, f)) => rust_map_text(o, f),
            ("rust_filter_text", In::StrPair(o, p)) => rust_filter_text(o, p),
            ("rust_fold_text", In::Str3(o, i, op)) => rust_fold_text(o, i, op),
            ("rust_sort_text", In::StrPair(c, m)) => rust_sort_text(c, m),
            ("rust_call_text", In::StrPair(f, a)) => rust_call_text(f, a),
            ("rust_condition_text", In::IntStr(k, r)) => rust_condition_text(*k, r),
            ("rust_if_text", In::Str3(c, t, e)) => rust_if_text(c, t, e),
            ("rust_lambda_text", In::StrPair(p, b)) => rust_lambda_text(p, b),
            ("rust_match_arm_text", In::StrPair(p, b)) => rust_match_arm_text(p, b),
            ("rust_match_text", In::StrPair(s, a)) => rust_match_text(s, a),
            ("rust_match_scrutinee_bare", In::IntPair(k, l)) => {
                rust_match_scrutinee_bare(*k, *l)
            }
            ("rust_match_scrutinee_form", In::BoolPair(h, b)) => {
                rust_match_scrutinee_form(*h, *b)
            }
            ("rust_match_scrutinee_text", In::IntStr(f, r)) => {
                rust_match_scrutinee_text(*f, r)
            }
            ("rust_block_text", In::Text(s)) => rust_block_text(s),
            ("rust_loop_text", In::Text(s)) => rust_loop_text(s),
            ("rust_struct_def_text", In::Str3(n, f, b)) => rust_struct_def_text(n, f, b),
            ("rust_index_text", In::StrPair(c, i)) => rust_index_text(c, i),
            ("rust_try_text", In::Text(s)) => rust_try_text(s),
            ("rust_assign_text", In::StrPair(t, v)) => rust_assign_text(t, v),
            ("rust_return_text", In::Text(s)) => rust_return_text(s),
            ("rust_break_text", In::BoolStr(h, v)) => rust_break_text(*h, v),
            ("rust_hole_text", In::Int(a)) => rust_hole_text(*a),
            ("rust_unsupported_text", In::Text(s)) => rust_unsupported_text(s),
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

    /// Every spec of slices 1–4, in file order. (Phase 31:
    /// slice 4's 35 walker specs join the sweeps — 79 in
    /// total — so this list and PINNED_TOTALS cover the
    /// whole module and the grand totals below are module
    /// totals.)
    const ALL_SPECS: [&str; 79] = [
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
        // ── slice 4 ──
        "rust_expr_binop_form",
        "rust_binop_infix_text",
        "rust_binop_append_text",
        "rust_binop_merge_text",
        "rust_binop_concat_text",
        "rust_concat_left_owned",
        "rust_concat_left_text",
        "rust_concat_right_form",
        "rust_concat_right_text",
        "rust_unary_text",
        "rust_collection_prefix",
        "rust_collection_suffix",
        "rust_map_text",
        "rust_filter_text",
        "rust_fold_text",
        "rust_sort_text",
        "rust_call_text",
        "rust_condition_text",
        "rust_if_text",
        "rust_lambda_text",
        "rust_match_arm_text",
        "rust_match_text",
        "rust_match_scrutinee_bare",
        "rust_match_scrutinee_form",
        "rust_match_scrutinee_text",
        "rust_block_text",
        "rust_loop_text",
        "rust_struct_def_text",
        "rust_index_text",
        "rust_try_text",
        "rust_assign_text",
        "rust_return_text",
        "rust_break_text",
        "rust_hole_text",
        "rust_unsupported_text",
    ];

    /// The Int-code specs swept over the full -2..=30 band
    /// (every spec whose single input is an Int code: 7 from
    /// slice 1, 18 from slice 2, 2 from slice 3, 4 from
    /// slice 4).
    const INT_BAND_SPECS: [&str; 31] = [
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
        // ── slice 4 ──
        "rust_expr_binop_form",
        "rust_collection_prefix",
        "rust_collection_suffix",
        "rust_hole_text",
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

    // ── Slice-4 sweep batteries ────────────────────────────

    /// The (kind, lit) grid for the two-parameter code
    /// specs (rust_concat_left_owned, rust_concat_right_form,
    /// rust_match_scrutinee_bare): every IRNode kind in the
    /// band × every IRLiteral kind plus margin on both axes
    /// (33 × 11 = 363) — the ownership cross-product
    /// precedent, at the grid the proof file's golden grids
    /// cover (kinds 0..=27 × lits 0..=5, strictly inside).
    fn kind_lit_grid() -> Vec<In> {
        (-2..=30)
            .flat_map(|kind| (-2..=8).map(move |lit| In::IntPair(kind, lit)))
            .collect()
    }

    /// (op, l, r) cases for rust_binop_infix_text: the
    /// symbol table decides ops 0..=16 (pinned); Append
    /// (17), Merge (18) and an off-band code are spec-silent
    /// for the composed spec. The op-2 case's left text is
    /// itself a Call template's output over an infix
    /// rendering — the proof file's two-level assembly
    /// (BinOp in Call in BinOp) stated at this spec's level.
    const INFIX_CASES: [(i64, &str, &str); 8] = [
        (0, "a", "b"),
        (8, "x", "y"),
        (14, "a", "b"),
        (16, "l", "r"),
        (2, "add((a + 1), b)", "2"),
        (17, "a", "b"),
        (18, "a", "b"),
        (-1, "a", "b"),
    ];

    /// Operand pairs for rust_binop_append_text (total).
    const APPEND_PAIRS: [(&str, &str); 4] = [
        ("items", "x"),
        ("a", "b"),
        ("xs", "next()"),
        ("", "x"),
    ];

    /// Operand pairs for rust_binop_merge_text (total).
    const MERGE_PAIRS: [(&str, &str); 3] = [("a", "b"), ("items", "more"), ("", "x")];

    /// Treated-operand pairs for rust_binop_concat_text
    /// (total): the inputs are the Phase 29 treatments'
    /// outputs — an owned left text, a borrowed right text.
    /// The third case's owned text is itself a Concat
    /// rendering: the proof assembly's Concat chain, outer
    /// level.
    const CONCAT_PAIRS: [(&str, &str); 3] = [
        ("\"a\".to_string()", "&b"),
        ("s", "&x"),
        ("(\"a\".to_string() + &x)", "\"b\""),
    ];

    /// (owned, rendered, owned_text) cases for
    /// rust_concat_left_text (total over the Bool: both
    /// branches decided for every text pair).
    const LEFT_TEXT_CASES: [(bool, &str, &str); 4] = [
        (true, "\"a\"", "\"a\".to_string()"),
        (false, "x", "x.to_string()"),
        (true, "s", "owned!"),
        (false, "\"b\"", "unused"),
    ];

    /// (form, rendered) cases for rust_concat_right_text:
    /// the three pinned forms (form 2 twice, over a plain
    /// and a parenthesized rendering) plus two off-table
    /// forms (spec-silent).
    const RIGHT_TEXT_CASES: [(i64, &str); 6] = [
        (0, "\"a\""),
        (1, "if c { 1 }"),
        (2, "x"),
        (2, "(a + b)"),
        (3, "x"),
        (-1, "x"),
    ];

    /// (over, func) pairs for rust_map_text (total).
    const MAP_PAIRS: [(&str, &str); 2] = [("xs", "|x| { x }"), ("ys", "f")];

    /// (over, pred) pairs for rust_filter_text (total).
    const FILTER_PAIRS: [(&str, &str); 2] = [("xs", "|x| { ok }"), ("ys", "p")];

    /// (over, initial, operation) triples for
    /// rust_fold_text (total; text order is initial,
    /// operation — the IR's field order reversed).
    const FOLD_TRIPLES: [(&str, &str, &str); 2] =
        [("xs", "0", "add"), ("ys", "init", "op")];

    /// (coll, cmp) pairs for rust_sort_text (total).
    const SORT_PAIRS: [(&str, &str); 2] = [("xs", "cmp"), ("ys", "|a, b| { a }")];

    /// (func, args) pairs for rust_call_text (total). The
    /// third case's args text contains an infix rendering:
    /// the proof assembly's Call level.
    const CALL_PAIRS: [(&str, &str); 3] = [
        ("add", "1, 2"),
        ("f", ""),
        ("add", "(a + 1), b"),
    ];

    /// (cond, then, else) triples for rust_if_text (total).
    /// The second case's condition arrives condition-
    /// rendered over a Match: the proof assembly's
    /// Match-condition case, outer level.
    const IF_TRIPLES: [(&str, &str, &str); 3] = [
        ("(a && b)", "1", "2"),
        ("(match y { _ => 1 })", "2", "3"),
        ("flag", "t", "e"),
    ];

    /// (params, body) pairs for rust_lambda_text (total).
    const LAMBDA_PAIRS: [(&str, &str); 3] = [
        ("x, y", "(x + y)"),
        ("x", "(x + 1)"),
        ("", "0"),
    ];

    /// (pat, body) pairs for rust_match_arm_text (total).
    const MATCH_ARM_PAIRS: [(&str, &str); 3] =
        [("_", "0"), ("\"go\"", "1"), ("Some(x)", "2")];

    /// (scrutinee, arms) pairs for rust_match_text (total).
    /// The second case is the proof assembly's string-
    /// pattern match (scrutinee already .as_str()-treated),
    /// the third a Match inside a Match arm — both stated
    /// at this spec's level.
    const MATCH_PAIRS: [(&str, &str); 3] = [
        ("y", "_ => 0"),
        ("s.as_str()", "\"go\" => 1, _ => 0"),
        ("a", "_ => match b { _ => 0 }"),
    ];

    /// (form, rendered) cases for rust_match_scrutinee_text:
    /// forms 0 and 1 pinned, form 2 spec-silent.
    const SCRUTINEE_TEXT_CASES: [(i64, &str); 4] =
        [(0, "y"), (1, "y"), (0, "s"), (2, "y")];

    /// Block inner texts for rust_block_text (total).
    const BLOCK_TEXT_INPUTS: [&str; 3] = ["let x = 1;", "", "x = 1; y = 2;"];

    /// Loop body texts for rust_loop_text (total).
    const LOOP_TEXT_INPUTS: [&str; 2] = ["tick();", ""];

    /// (name, fields, body) triples for
    /// rust_struct_def_text (total).
    const STRUCT_TRIPLES: [(&str, &str, &str); 2] = [
        ("Point", "x: i64, y: i64", "origin"),
        ("Empty", "", "unit"),
    ];

    /// (coll, idx) pairs for rust_index_text (total).
    const INDEX_PAIRS: [(&str, &str); 2] = [("xs", "i"), ("a", "0")];

    /// Inner texts for rust_try_text (total).
    const TRY_TEXT_INPUTS: [&str; 2] = ["load()", "x"];

    /// (target, value) pairs for rust_assign_text (total).
    const ASSIGN_PAIRS: [(&str, &str); 2] = [("x", "5"), ("y", "(a + b)")];

    /// Inner texts for rust_return_text (total).
    const RETURN_TEXT_INPUTS: [&str; 2] = ["5", "x"];

    /// (has_value, value) cases for rust_break_text (total
    /// over the Bool; the value text is interpolated only
    /// on the true branch but the output is decided for
    /// every pair).
    const BREAK_CASES: [(bool, &str); 4] = [
        (true, "x"),
        (false, ""),
        (true, "5"),
        (false, "ignored"),
    ];

    /// Shape names for rust_unsupported_text (total).
    const SHAPE_INPUTS: [&str; 2] = ["Alloc", "Weird"];

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
            // ── slice 4 ──
            "rust_binop_infix_text" => INFIX_CASES
                .iter()
                .map(|(o, l, r)| In::IntStrStr(*o, l, r))
                .collect(),
            "rust_binop_append_text" => APPEND_PAIRS
                .iter()
                .map(|(l, r)| In::StrPair(l, r))
                .collect(),
            "rust_binop_merge_text" => MERGE_PAIRS
                .iter()
                .map(|(l, r)| In::StrPair(l, r))
                .collect(),
            "rust_binop_concat_text" => CONCAT_PAIRS
                .iter()
                .map(|(o, b)| In::StrPair(o, b))
                .collect(),
            "rust_concat_left_owned" | "rust_concat_right_form"
            | "rust_match_scrutinee_bare" => kind_lit_grid(),
            "rust_concat_left_text" => LEFT_TEXT_CASES
                .iter()
                .map(|(o, r, t)| In::BoolStrStr(*o, r, t))
                .collect(),
            "rust_concat_right_text" => RIGHT_TEXT_CASES
                .iter()
                .map(|(f, r)| In::IntStr(*f, r))
                .collect(),
            "rust_unary_text" => {
                (-2..=12).map(|op| In::IntStr(op, "xs")).collect()
            }
            "rust_map_text" => {
                MAP_PAIRS.iter().map(|(o, f)| In::StrPair(o, f)).collect()
            }
            "rust_filter_text" => FILTER_PAIRS
                .iter()
                .map(|(o, p)| In::StrPair(o, p))
                .collect(),
            "rust_fold_text" => FOLD_TRIPLES
                .iter()
                .map(|(o, i, op)| In::Str3(o, i, op))
                .collect(),
            "rust_sort_text" => {
                SORT_PAIRS.iter().map(|(c, m)| In::StrPair(c, m)).collect()
            }
            "rust_call_text" => {
                CALL_PAIRS.iter().map(|(f, a)| In::StrPair(f, a)).collect()
            }
            "rust_condition_text" => {
                // The whole kind band over one rendering
                // (the classification is total, so every case
                // is pinned), plus the proof file's three
                // rendered-text cases; kind 10's proof case
                // rides the band (rendered "flag").
                let mut v: Vec<In> =
                    (-2..=30).map(|k| In::IntStr(k, "flag")).collect();
                v.push(In::IntStr(6, "match y { _ => 1 }"));
                v.push(In::IntStr(3, "if a { b }"));
                v.push(In::IntStr(11, "(a && b)"));
                v
            }
            "rust_if_text" => IF_TRIPLES
                .iter()
                .map(|(c, t, e)| In::Str3(c, t, e))
                .collect(),
            "rust_lambda_text" => LAMBDA_PAIRS
                .iter()
                .map(|(p, b)| In::StrPair(p, b))
                .collect(),
            "rust_match_arm_text" => MATCH_ARM_PAIRS
                .iter()
                .map(|(p, b)| In::StrPair(p, b))
                .collect(),
            "rust_match_text" => MATCH_PAIRS
                .iter()
                .map(|(s, a)| In::StrPair(s, a))
                .collect(),
            "rust_match_scrutinee_form" => vec![
                In::BoolPair(false, false),
                In::BoolPair(false, true),
                In::BoolPair(true, false),
                In::BoolPair(true, true),
            ],
            "rust_match_scrutinee_text" => SCRUTINEE_TEXT_CASES
                .iter()
                .map(|(f, r)| In::IntStr(*f, r))
                .collect(),
            "rust_block_text" => {
                BLOCK_TEXT_INPUTS.iter().map(|s| In::Text(s)).collect()
            }
            "rust_loop_text" => {
                LOOP_TEXT_INPUTS.iter().map(|s| In::Text(s)).collect()
            }
            "rust_struct_def_text" => STRUCT_TRIPLES
                .iter()
                .map(|(n, f, b)| In::Str3(n, f, b))
                .collect(),
            "rust_index_text" => INDEX_PAIRS
                .iter()
                .map(|(c, i)| In::StrPair(c, i))
                .collect(),
            "rust_try_text" => {
                TRY_TEXT_INPUTS.iter().map(|s| In::Text(s)).collect()
            }
            "rust_assign_text" => ASSIGN_PAIRS
                .iter()
                .map(|(t, v)| In::StrPair(t, v))
                .collect(),
            "rust_return_text" => RETURN_TEXT_INPUTS
                .iter()
                .map(|s| In::Text(s))
                .collect(),
            "rust_break_text" => BREAK_CASES
                .iter()
                .map(|(h, v)| In::BoolStr(*h, v))
                .collect(),
            "rust_unsupported_text" => {
                SHAPE_INPUTS.iter().map(|s| In::Text(s)).collect()
            },
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
            // ── slice 4 ──
            // tests: add-form 0, shr-form 15, concat 16, append 17, merge 18
            "rust_expr_binop_form" => ints(&[0, 15, 16, 17, 18]),
            // No in-spec tests (multi-input specs); the
            // proof file's infix assertions, plus its
            // two-level assembly's outer level (BinOp over
            // the Call rendering) as a composed case.
            "rust_binop_infix_text" => vec![
                In::IntStrStr(0, "a", "b"),
                In::IntStrStr(8, "x", "y"),
                In::IntStrStr(14, "a", "b"),
                In::IntStrStr(2, "add((a + 1), b)", "2"),
            ],
            // proof assertions
            "rust_binop_append_text" => vec![In::StrPair("items", "x")],
            "rust_binop_merge_text" => vec![In::StrPair("a", "b")],
            "rust_binop_concat_text" => {
                vec![In::StrPair("\"a\".to_string()", "&b")]
            }
            // The proof file asserts the full 0..=27 x 0..=5
            // grid; the golden points are its decisive cells:
            // the one true point, a same-kind false, a
            // same-lit false, a far corner.
            "rust_concat_left_owned" => vec![
                In::IntPair(9, 3),
                In::IntPair(9, 0),
                In::IntPair(0, 3),
                In::IntPair(27, 5),
            ],
            // The proof assembly's Concat chain exercises
            // both branches (the owned literal; a plain
            // rendering passing through).
            "rust_concat_left_text" => vec![
                In::BoolStrStr(true, "\"a\"", "\"a\".to_string()"),
                In::BoolStrStr(false, "x", "x.to_string()"),
            ],
            // Proof grid points, plus the catch-all
            // extension point (28, 0): the proof mirror
            // abstains there (None — outside the walker's
            // domain), the spec's catch-all row pins 2; see
            // the mirror's comment.
            "rust_concat_right_form" => vec![
                In::IntPair(9, 3),
                In::IntPair(1, 0),
                In::IntPair(0, 0),
                In::IntPair(28, 0),
            ],
            // proof text assertions
            "rust_concat_right_text" => vec![
                In::IntStr(0, "\"a\""),
                In::IntStr(1, "if c { 1 }"),
                In::IntStr(2, "x"),
            ],
            // proof assertions: all ten ops over "xs"
            "rust_unary_text" => {
                (0..=9).map(|op| In::IntStr(op, "xs")).collect()
            }
            // tests: list-prefix 0, set-prefix 1, tuple-prefix 4
            "rust_collection_prefix" => ints(&[0, 1, 4]),
            // tests: list-suffix 0, set-suffix 1, tuple-suffix 4
            "rust_collection_suffix" => ints(&[0, 1, 4]),
            // proof assertions
            "rust_map_text" => vec![In::StrPair("xs", "|x| { x }")],
            "rust_filter_text" => vec![In::StrPair("xs", "|x| { ok }")],
            "rust_fold_text" => vec![In::Str3("xs", "0", "add")],
            "rust_sort_text" => vec![In::StrPair("xs", "cmp")],
            "rust_call_text" => {
                vec![In::StrPair("add", "1, 2"), In::StrPair("f", "")]
            }
            // proof assertions (kind 10's case, rendered
            // "flag", rides the band sweep)
            "rust_condition_text" => vec![
                In::IntStr(6, "match y { _ => 1 }"),
                In::IntStr(3, "if a { b }"),
                In::IntStr(11, "(a && b)"),
                In::IntStr(10, "flag"),
            ],
            "rust_if_text" => vec![In::Str3("(a && b)", "1", "2")],
            "rust_lambda_text" => vec![In::StrPair("x, y", "(x + y)")],
            "rust_match_arm_text" => vec![In::StrPair("_", "0")],
            "rust_match_text" => vec![In::StrPair("y", "_ => 0")],
            // proof assertions; kind 22's cells are
            // spec-silent by design (G2) and ride the sweep,
            // never the golden set
            "rust_match_scrutinee_bare" => vec![
                In::IntPair(9, 3),
                In::IntPair(9, 0),
                In::IntPair(10, 3),
            ],
            // proof assertions: the complete Bool x Bool domain
            "rust_match_scrutinee_form" => vec![
                In::BoolPair(true, false),
                In::BoolPair(true, true),
                In::BoolPair(false, false),
                In::BoolPair(false, true),
            ],
            "rust_match_scrutinee_text" => {
                vec![In::IntStr(0, "y"), In::IntStr(1, "y")]
            }
            "rust_block_text" => vec![In::Text("let x = 1;")],
            "rust_loop_text" => vec![In::Text("tick();")],
            "rust_struct_def_text" => {
                vec![In::Str3("Point", "x: i64, y: i64", "origin")]
            }
            "rust_index_text" => vec![In::StrPair("xs", "i")],
            "rust_try_text" => vec![In::Text("load()")],
            "rust_assign_text" => vec![In::StrPair("x", "5")],
            "rust_return_text" => vec![In::Text("5")],
            // proof assertions
            "rust_break_text" => {
                vec![In::BoolStr(true, "x"), In::BoolStr(false, "")]
            }
            // test: hole-text
            "rust_hole_text" => ints(&[0]),
            // test: alloc-text
            "rust_unsupported_text" => vec![In::Text("Alloc")],
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
    //  rust_signature / rust_test_case. Slice 4 adds (i64,
    //  String) for the Int-keyed text templates, (i64,
    //  String, String) for rust_binop_infix_text, (bool,
    //  String) for rust_break_text, (bool, String, String)
    //  for rust_concat_left_text, (bool, bool) for
    //  rust_match_scrutinee_form, and (String, String,
    //  String) for the three-text templates — each matching
    //  the emitted multi-param signature the same way.
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
            In::IntStr(_, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::IntStr(a, s) => {
                            format!("({}, {})", a, rust_str_literal(s))
                        }
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(i64, &str)] = &[{vals}];\n    for &(__a, __b) in __inputs {{\n        let __out = {name}(__a, __b.to_string());\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::IntStrStr(_, _, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::IntStrStr(a, s, t) => format!(
                            "({}, {}, {})",
                            a,
                            rust_str_literal(s),
                            rust_str_literal(t)
                        ),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(i64, &str, &str)] = &[{vals}];\n    for &(__a, __b, __c) in __inputs {{\n        let __out = {name}(__a, __b.to_string(), __c.to_string());\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::BoolStr(_, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::BoolStr(b, s) => {
                            format!("({}, {})", b, rust_str_literal(s))
                        }
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(bool, &str)] = &[{vals}];\n    for &(__a, __b) in __inputs {{\n        let __out = {name}(__a, __b.to_string());\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::BoolStrStr(_, _, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::BoolStrStr(b, s, t) => format!(
                            "({}, {}, {})",
                            b,
                            rust_str_literal(s),
                            rust_str_literal(t)
                        ),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(bool, &str, &str)] = &[{vals}];\n    for &(__a, __b, __c) in __inputs {{\n        let __out = {name}(__a, __b.to_string(), __c.to_string());\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::BoolPair(_, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::BoolPair(a, b) => format!("({}, {})", a, b),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(bool, bool)] = &[{vals}];\n    for &(__a, __b) in __inputs {{\n        let __out = {name}(__a, __b);\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
                    code = winner_code,
                    vals = vals.join(", "),
                    name = func.name,
                )
            }
            In::Str3(_, _, _) => {
                let vals: Vec<String> = inputs
                    .iter()
                    .map(|i| match i {
                        In::Str3(a, b, c) => format!(
                            "({}, {}, {})",
                            rust_str_literal(a),
                            rust_str_literal(b),
                            rust_str_literal(c)
                        ),
                        _ => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[(&str, &str, &str)] = &[{vals}];\n    for &(__a, __b, __c) in __inputs {{\n        let __out = {name}(__a.to_string(), __b.to_string(), __c.to_string());\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
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
    #[ignore = "cross-validation harness: 3 randomized synthesis runs x 79 specs, compiles + executes winners; run explicitly with --ignored --nocapture"]
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
        let _ = writeln!(md, "# M3 cross-validation (slices 1–4): synthesized winners vs tests/m3_proof.rs mirrors");
        let _ = writeln!(md);
        let _ = writeln!(md, "Pipeline per spec per run: parse → typecheck → synthesize_with_module (opt 1) → Z3 verify_all (module-aware) → select_best → codegen (rust), then the winner is compiled with rustc and EXECUTED over the complete finite domain (Int codes -2..=30 for the 31 Int-code specs; rust_keyword: 51 positives + 24 near-miss negatives; rust_literal_bool and rust_test_name_form: the full Bool domain; rust_pattern_ctor_name: 7 names × has_subs; slice 3: rust_string_ownership over the full -2..=30 × -2..=30 cross product, the `++` equation templates over string batteries, rust_ident_suffix over its sanitized-string contract domain plus out-of-contract names reported separately; slice 4: the (kind, lit) code specs over the full 33 × 11 grid, rust_match_scrutinee_form over the full Bool × Bool domain, the walker templates over batteries of pre-rendered child texts incl. composed two-level inputs). See src/m3_cross_validation.rs for sweep definitions and mirror semantics: a case is pinned exactly where the spec's constraints decide it.");
        let _ = writeln!(md, "A winner that fails to compile/run counts as total disagreement for that run (exec FAIL) — except a rustc failure at the emitter's rendering layer (the family measured in Phases 25/27/28 and fixed in Phase 29: E0369 Concat-operand ownership; E0308 string-boundary rendering; parse-level unescaped-literal failures), which is recorded as COMPILE_EDGE: not executed, not a behavioral disagreement. Spec-silent (unpinned) cases are reported separately and never counted as disagreements; out-of-contract cases (rust_ident_suffix) are reported separately and counted in no bucket.");
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
    /// in no bucket; slice 4: the (kind, lit) specs are
    /// total over their 363-case grid except
    /// rust_match_scrutinee_bare, whose kind-22 column —
    /// 11 cases — is unpinned by design (G2);
    /// rust_binop_infix_text pins exactly the symbol
    /// table's range, 5 of its 8 battery cases;
    /// rust_condition_text is total over its 36 cases
    /// because the classification it composes is total).
    /// The harness's bookkeeping stands or falls with these
    /// numbers, so they are pinned here, per spec, all 79.
    const PINNED_TOTALS: [(&str, usize); 79] = [
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
        // ── slice 4 ──
        ("rust_expr_binop_form", 19),
        ("rust_binop_infix_text", 5),
        ("rust_binop_append_text", 4),
        ("rust_binop_merge_text", 3),
        ("rust_binop_concat_text", 3),
        ("rust_concat_left_owned", 363),
        ("rust_concat_left_text", 4),
        ("rust_concat_right_form", 363),
        ("rust_concat_right_text", 4),
        ("rust_unary_text", 10),
        ("rust_collection_prefix", 5),
        ("rust_collection_suffix", 5),
        ("rust_map_text", 2),
        ("rust_filter_text", 2),
        ("rust_fold_text", 2),
        ("rust_sort_text", 2),
        ("rust_call_text", 3),
        ("rust_condition_text", 36),
        ("rust_if_text", 3),
        ("rust_lambda_text", 3),
        ("rust_match_arm_text", 3),
        ("rust_match_text", 3),
        ("rust_match_scrutinee_bare", 352),
        ("rust_match_scrutinee_form", 4),
        ("rust_match_scrutinee_text", 3),
        ("rust_block_text", 3),
        ("rust_loop_text", 2),
        ("rust_struct_def_text", 2),
        ("rust_index_text", 2),
        ("rust_try_text", 2),
        ("rust_assign_text", 2),
        ("rust_return_text", 2),
        ("rust_break_text", 4),
        ("rust_hole_text", 1),
        ("rust_unsupported_text", 2),
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
        // ── slice 4 ──
        // The (kind, lit) grid specs: 33 kinds x 11 lits.
        for name in [
            "rust_concat_left_owned",
            "rust_concat_right_form",
            "rust_match_scrutinee_bare",
        ] {
            assert_eq!(sweep(name).len(), 363, "{}: grid sweep", name);
        }
        // rust_concat_left_owned and rust_concat_right_form
        // are total over the grid (the catch-all row decides
        // every off-table pair for the latter).
        for name in ["rust_concat_left_owned", "rust_concat_right_form"] {
            let (_, pinned, unpinned) = totals(name, &sweep(name));
            assert_eq!((pinned, unpinned), (363, 0), "{}", name);
        }
        // rust_match_scrutinee_bare: the kind-22 column (11
        // cases) is unpinned by design (G2); the rest of the
        // grid is pinned — the pinned half stands alone.
        let sb = sweep("rust_match_scrutinee_bare");
        let (_, pinned, unpinned) = totals("rust_match_scrutinee_bare", &sb);
        assert_eq!((pinned, unpinned), (352, 11));
        // rust_match_scrutinee_form: the complete Bool x
        // Bool domain, total.
        let sf = sweep("rust_match_scrutinee_form");
        assert_eq!(sf.len(), 4);
        let (_, pinned, unpinned) = totals("rust_match_scrutinee_form", &sf);
        assert_eq!((pinned, unpinned), (4, 0));
        // rust_condition_text composes a total
        // classification: all 36 cases pinned.
        let ct = sweep("rust_condition_text");
        assert_eq!(ct.len(), 36);
        let (_, pinned, unpinned) = totals("rust_condition_text", &ct);
        assert_eq!((pinned, unpinned), (36, 0));
        // rust_expr_binop_form pins the nineteen IRBinOp
        // codes of the band; the rest is spec-silent.
        let bf = sweep("rust_expr_binop_form");
        assert_eq!(bf.len(), 33);
        let (_, pinned, unpinned) = totals("rust_expr_binop_form", &bf);
        assert_eq!((pinned, unpinned), (19, 14));
        // rust_hole_text pins kind 0 only.
        let ht = sweep("rust_hole_text");
        assert_eq!(ht.len(), 33);
        let (_, pinned, unpinned) = totals("rust_hole_text", &ht);
        assert_eq!((pinned, unpinned), (1, 32));
        // rust_binop_infix_text pins exactly where its
        // callee's table does: 5 of 8 battery cases.
        let it = sweep("rust_binop_infix_text");
        assert_eq!(it.len(), 8);
        let (_, pinned, unpinned) = totals("rust_binop_infix_text", &it);
        assert_eq!((pinned, unpinned), (5, 3));
        // Grand totals across the module: 3548 cases per
        // run, 2761 pinned, 780 spec-silent, 7 out of
        // contract. (Slices 1–3: 2196 cases, 1533 pinned,
        // 656 silent — Phase 29 moved the literal specs'
        // three special-contents cases each from pinned to
        // spec-silent, 1539 - 6 / 650 + 6. Slice 4 adds
        // 1352 cases, 1228 pinned, 124 silent.)
        let total_cases: usize = ALL_SPECS.iter().map(|n| sweep(n).len()).sum();
        let total_pinned: usize = PINNED_TOTALS.iter().map(|(_, t)| t).sum();
        assert_eq!(total_cases, 3548);
        assert_eq!(total_pinned, 2761);
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
        //
        // Phase 30 scoping (the Phase 26 precedent): slice 4
        // added 35 specs to the module (79 total) — the
        // expression walker's arm templates. Which of their
        // shapes the builders recognize (the `++` equations
        // over pre-rendered children, the Int-guard rows with
        // expression pins, the call-guarded rows) is the next
        // phase's measurement question, not something to
        // assert ahead of the evidence. The recognition
        // assertions below therefore keep iterating the same
        // 30 names; only the module count assertion moves,
        // to 79. The sweep harness above (ALL_SPECS /
        // PINNED_TOTALS) likewise still covers slices 1–3;
        // extending the sweeps to slice 4 belongs to the
        // measurement phase.
        //
        // Phase 31 flip: the measurement is in (probe +
        // cross-validation, 3 runs each; see the ROAD's
        // Phase 31 record). The recognized set grows by the
        // 27 slice-4 specs whose seed the probe records in
        // every run: the 21 equation templates (incl.
        // rust_binop_infix_text, whose equation composes a
        // call inside the `++` chain), the 4 single-Int
        // guard tables with literal pins
        // (rust_expr_binop_form, rust_collection_prefix,
        // rust_collection_suffix, rust_hole_text), and the 2
        // composition-guard shapes (rust_condition_text's
        // call guard, rust_break_text's Bool guard with a
        // compound pin). The eight remaining slice-4 shapes
        // were asserted UNRECOGNIZED below, each on its
        // measured predicate: the Int-keyed expression pins
        // (rust_unary_text, rust_concat_right_text,
        // rust_match_scrutinee_text — guard_table_body
        // admits one parameter and literal pins only via
        // guard_row; composition_guard_body's leaves are
        // Bool-domain only via guard_leaf_on_expr), the
        // two-parameter Int guard tables with literal pins
        // (rust_concat_left_owned, rust_concat_right_form,
        // rust_match_scrutinee_bare — guard_table_body's
        // arity is one; scrutinee_bare is additionally
        // partial by design, G2), the Bool x Bool region
        // rows (rust_match_scrutinee_form — its rows guard
        // on TWO different Bool expressions, where
        // composition_guard_row requires one shared guard),
        // and rust_concat_left_text (a plain Bool-parameter
        // guard with bare-variable pins at arity three:
        // composition_guard_body's territory rule refuses
        // it, and neither earlier family admits its arity).
        //
        // Phase 32 flip: the fifth builder family — the
        // expression-pin guard table (rows recognized as the
        // guard family's are, the key one Int parameter, the
        // pin admitted the way composition_pin admits pins,
        // with at least one compound pin as the territory
        // rule) — recognizes the Int-keyed expression-pin
        // trio, so the iterated set grows to 60. The five
        // remaining slice-4 shapes stay unrecognized on their
        // Phase 31 predicates, asserted below: the
        // two-parameter Int guard tables with literal pins,
        // the Bool x Bool region rows, and
        // rust_concat_left_text's territory hole.
        const RECOGNIZED: [&str; 60] = [
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
            // ── slice 4 (Phase 31) ──
            "rust_expr_binop_form", "rust_binop_infix_text",
            "rust_binop_append_text", "rust_binop_merge_text",
            "rust_binop_concat_text", "rust_collection_prefix",
            "rust_collection_suffix", "rust_map_text", "rust_filter_text",
            "rust_fold_text", "rust_sort_text", "rust_call_text",
            "rust_condition_text", "rust_if_text", "rust_lambda_text",
            "rust_match_arm_text", "rust_match_text", "rust_block_text",
            "rust_loop_text", "rust_struct_def_text", "rust_index_text",
            "rust_try_text", "rust_assign_text", "rust_return_text",
            "rust_break_text", "rust_hole_text", "rust_unsupported_text",
            // ── slice 4 (Phase 32): expression-pin guard table ──
            "rust_unary_text", "rust_concat_right_text",
            "rust_match_scrutinee_text",
        ];
        let functions = load_codegen_module();
        assert_eq!(functions.len(), 79, "module spec count");
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
        for name in [
            "rust_string_ownership",
            "rust_ident_empty",
            // ── slice 4 (Phase 31): measured unrecognized ──
            "rust_concat_right_form",
            "rust_match_scrutinee_form",
            "rust_match_scrutinee_bare",
            "rust_concat_left_owned",
            "rust_concat_left_text",
        ] {
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

        // ── slice 4 ──
        // rust_unary_text: the op-2 row pins the method
        // template; a prefix rendering there disagrees, and
        // an off-table op is spec-silent, never a
        // disagreement.
        let inputs = vec![In::IntStr(2, "xs"), In::IntStr(10, "xs")];
        let outs = vec![
            Out::Text("(- xs)".to_string()),
            Out::Text("whatever".to_string()),
        ];
        let ag = compare("rust_unary_text", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 1);
        assert_eq!(ag.sweep_pinned_agree, 0);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::IntStr(2, "xs"));
        assert_eq!(ag.unpinned, 1);

        // rust_concat_right_form's catch-all row pins family
        // 2 at an off-band kind: a winner returning the
        // parenthesized-borrow family there IS a
        // disagreement (the stated-domain extension is
        // load-bearing, not decorative).
        let inputs = vec![In::IntPair(28, 0), In::IntPair(9, 3)];
        let outs = vec![Out::Int(1), Out::Int(0)];
        let ag = compare("rust_concat_right_form", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::IntPair(28, 0));

        // rust_match_scrutinee_bare: the (9, 3) point is
        // pinned true; the whole kind-22 column is
        // spec-silent by design (G2) — any winner output
        // there is recorded, never a disagreement.
        let inputs = vec![
            In::IntPair(9, 3),
            In::IntPair(22, 3),
            In::IntPair(22, 0),
        ];
        let outs = vec![Out::Bool(true), Out::Bool(true), Out::Bool(false)];
        let ag = compare("rust_match_scrutinee_bare", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 1);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.unpinned, 2);
        assert!(ag.disagreements.is_empty());
        // And a wrong value on the pinned point disagrees.
        let outs = vec![Out::Bool(false), Out::Bool(true), Out::Bool(false)];
        let ag = compare("rust_match_scrutinee_bare", &inputs, &outs);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, In::IntPair(9, 3));
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

        // (Int, String) spec (slice 4): the runner passes an
        // i64 code and an owned String, matching
        // `pub fn rust_unary_text(op: i64, inner: String)
        // -> String`.
        let un_func = dummy_func("rust_unary_text", TypeRef::Int, TypeRef::String);
        let inputs = vec![In::IntStr(2, "xs"), In::IntStr(0, "xs")];
        let un = "pub fn rust_unary_text(op: i64, inner: String) -> String {\n    match op { 2 => format!(\"{}.abs()\", inner), _ => format!(\"(- {})\", inner) }\n}\n";
        let outs = execute_winner(0, &un_func, un, &inputs).expect("unary winner must run");
        let ag = compare("rust_unary_text", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert!(ag.disagreements.is_empty());

        // (Bool, bool) spec (slice 4): the runner passes two
        // bools, matching `pub fn rust_match_scrutinee_form(
        // has_string_pat: bool, bare: bool) -> i64`.
        let sf_func = dummy_func("rust_match_scrutinee_form", TypeRef::Bool, TypeRef::Int);
        let inputs = vec![In::BoolPair(true, false), In::BoolPair(true, true)];
        let sf = "pub fn rust_match_scrutinee_form(has_string_pat: bool, bare: bool) -> i64 {\n    if has_string_pat && !bare { 1 } else { 0 }\n}\n";
        let outs = execute_winner(0, &sf_func, sf, &inputs).expect("scrutinee-form winner must run");
        let ag = compare("rust_match_scrutinee_form", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert!(ag.disagreements.is_empty());

        // (Bool, String, String) spec (slice 4): the runner
        // passes a bool and two owned Strings, matching
        // `pub fn rust_concat_left_text(owned: bool,
        // rendered: String, owned_text: String) -> String`.
        let lt_func = dummy_func("rust_concat_left_text", TypeRef::Bool, TypeRef::String);
        let inputs = vec![
            In::BoolStrStr(true, "\"a\"", "\"a\".to_string()"),
            In::BoolStrStr(false, "x", "x.to_string()"),
        ];
        let lt = "pub fn rust_concat_left_text(owned: bool, rendered: String, owned_text: String) -> String {\n    if owned { owned_text } else { rendered }\n}\n";
        let outs = execute_winner(0, &lt_func, lt, &inputs).expect("left-text winner must run");
        let ag = compare("rust_concat_left_text", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert!(ag.disagreements.is_empty());
    }
}
