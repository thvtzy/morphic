// ─────────────────────────────────────────────────────────────
//  M3 CROSS-VALIDATION HARNESS (test-only) — slice 1
//
//  Generalizes the M2 cross-validation harness
//  (src/m2_cross_validation.rs) to the Codegen module. For
//  every one of the 8 slice-1 specs in
//  examples/self_host/codegen.morph this harness runs the real
//  pipeline at opt level 1 — parse → typecheck →
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
//         single execution), and
//
//    (ii) the COMPLETE finite domains: for the Int-code specs,
//         every code -2..=30 — the full meaningful domain of
//         each table (largest: TypeRef codes 0..=25) plus a
//         margin on both sides; for rust_keyword (String
//         input), the full 51-word positive set plus a
//         negative set of near misses (the 2024 keyword "gen",
//         the contextual keyword "union", trailing/leading
//         spaces, uppercase variants, the empty string,
//         common non-keywords).
//
//  Mirror semantics (copied from tests/m3_proof.rs, which is
//  the source of truth; if the two ever diverge, update this
//  copy): a mirror returns None exactly where the spec is
//  silent — rust_type_name codes 14..=25 (composites, slice 2)
//  and every off-range code, rust_binop_symbol Append/Merge,
//  the unpinned rows of the two partial unary tables, and
//  off-range codes of the two total Int classifications.
//  Those cases are reported as "unpinned" (winner output
//  recorded) and NEVER counted as disagreements. rust_keyword
//  is the one total spec: its equation decides every string,
//  so every input is pinned.
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

    /// One sweep input: an Int code, or a string for
    /// rust_keyword (the module's one String-input spec).
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum In {
        Int(i64),
        Text(&'static str),
    }

    impl In {
        fn render(&self) -> String {
            match self {
                In::Int(i) => i.to_string(),
                In::Text(s) => format!("\"{}\"", s),
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

    fn sweep(spec: &str) -> Vec<In> {
        match spec {
            "rust_keyword" => KEYWORD_POSITIVES
                .iter()
                .chain(KEYWORD_NEGATIVES.iter())
                .map(|s| In::Text(s))
                .collect(),
            "rust_type_name" | "rust_binop_symbol" | "rust_unaryop_symbol"
            | "rust_unaryop_method" | "rust_unaryop_form" | "rust_collection_form"
            | "spec_binop_symbol" => (-2..=30).map(In::Int).collect(),
            _ => panic!("no sweep defined for spec {}", spec),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Golden cases — the in-spec tests of codegen.morph (the
    //  case list only; expectations come from the mirrors,
    //  which m3_proof's golden tests pin to these same cases).
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
            _ => panic!("no golden cases defined for spec {}", spec),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Winner execution: build a runner binary around the
    //  winner's generated code (verbatim) and run it once over
    //  the whole input list. String-input specs are called
    //  with an owned String, matching the emitted signature
    //  (`pub fn rust_keyword(name: String) -> bool`).
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
                        In::Text(_) => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[i64] = &[{vals}];\n    for &__a in __inputs {{\n        let __out = {name}(__a);\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
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
                        In::Int(_) => unreachable!(),
                    })
                    .collect();
                format!(
                    "{code}\n\nfn main() {{\n    let __inputs: &[&str] = &[{vals}];\n    for &__a in __inputs {{\n        let __out = {name}(__a.to_string());\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
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
    #[ignore = "cross-validation harness: 3 randomized synthesis runs x 8 specs, compiles + executes winners; run explicitly with --ignored --nocapture"]
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
        let _ = writeln!(md, "# M3 cross-validation (slice 1): synthesized winners vs tests/m3_proof.rs mirrors");
        let _ = writeln!(md);
        let _ = writeln!(md, "Pipeline per spec per run: parse → typecheck → synthesize_with_module (opt 1) → Z3 verify_all (module-aware) → select_best → codegen (rust), then the winner is compiled with rustc and EXECUTED over the complete finite domain (Int codes -2..=30; rust_keyword: 51 positives + 24 near-miss negatives). See src/m3_cross_validation.rs for sweep definitions and mirror semantics.");
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
                            "XVAL3|run={}|spec={}|tpr={:.3}|verified={}|prov={}|golden={}/{}|sweep_pinned={}/{}|unpinned={}|exec=ok",
                            run, name, winner.scores.test_pass_ratio, winner.verified,
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

    #[test]
    fn xval3_golden_cases_lie_inside_the_sweeps() {
        for name in [
            "rust_type_name",
            "rust_binop_symbol",
            "rust_unaryop_symbol",
            "rust_unaryop_method",
            "rust_unaryop_form",
            "rust_collection_form",
            "rust_keyword",
            "spec_binop_symbol",
        ] {
            let sw = sweep(name);
            for g in golden(name) {
                assert!(sw.contains(&g), "{}: golden {:?} not in sweep", name, g);
            }
            for input in &sw {
                let _ = mirror(name, input); // must not panic
            }
        }
        // The keyword sweep is exactly 51 pinned positives +
        // 24 pinned negatives (the spec is total).
        let kw = sweep("rust_keyword");
        assert_eq!(kw.len(), 75);
        let (_, pinned, unpinned) = totals("rust_keyword", &kw);
        assert_eq!((pinned, unpinned), (75, 0));
        // Int sweeps: full band -2..=30 for every code spec.
        for name in [
            "rust_type_name",
            "rust_binop_symbol",
            "rust_unaryop_symbol",
            "rust_unaryop_method",
            "rust_unaryop_form",
            "rust_collection_form",
            "spec_binop_symbol",
        ] {
            assert_eq!(sweep(name).len(), 33, "{}: sweep band", name);
        }
    }

    #[test]
    fn xval3_seed_shapes_recognized_and_encodable() {
        // Diagnostic pin: for every slice-1 spec, the shared
        // resolver's body builders recognize a body (an equation
        // RHS for rust_keyword, a guard-table match for the
        // other seven) and the verifier can encode that body in
        // module context — the two mechanism steps a
        // ConstraintInduced seed needs before pool admission.
        let functions = load_codegen_module();
        let index = engine::SpecIndex::from_specs(&functions);
        for func in &functions {
            let body = index
                .callee_body(&func.name)
                .unwrap_or_else(|| panic!("{}: no seed body recognized", func.name));
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
    }
}
