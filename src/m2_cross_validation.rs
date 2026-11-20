// ─────────────────────────────────────────────────────────────
//  M2 CROSS-VALIDATION HARNESS (test-only)
//
//  ROAD M2 gate item (d): cross-validate synthesized output
//  against the hand-written Rust mirrors in tests/m2_proof.rs.
//  For every one of the 14 M2 specs this harness runs the real
//  pipeline at opt level 1 — parse → typecheck → synthesize →
//  Z3 verify_all → select_best → codegen, exactly as
//  `morphic build` and the synthesis probe do — then EXECUTES
//  the winning implementation and compares its behavior against
//  the mirror over:
//
//    (i)  every golden case available for the spec: the union of
//         the in-spec tests (validator.morph / printer.morph)
//         and the extra golden assertions in tests/m2_proof.rs
//         (the FieldAccess arity row, the untested name-table
//         rows, and all cases of the two multi-input specs,
//         whose inputs the spec test syntax cannot bind). The
//         mirror functions in m2_proof.rs are themselves pinned
//         to those cases by that file's golden tests, so the
//         mirror is the reference and the golden inputs are the
//         case list (see GOLDEN below; every golden input is
//         asserted to lie inside the sweep, so golden results
//         are read off the same single execution).
//
//    (ii) a fixed input sweep per spec (see SWEEPS below):
//         the full meaningful domain of each Int code space,
//         the values just inside and just outside every defined
//         range, negative values, and a far-out probe. The two
//         two-parameter specs get a grid over both parameters.
//
//  Execution mechanics: the winner's generated Rust (verbatim,
//  as codegen produced it) is placed in a small runner binary
//  whose main() calls the winner on every sweep input and
//  prints each output with {:?}; the runner is compiled with
//  `rustc --edition 2021` (a binary, not the probe's lib check)
//  and run once per spec. Mirror expectations are computed
//  in-process. A winner that fails to compile, run, or produce
//  exactly one parseable output per input counts as TOTAL
//  disagreement for that run and is recorded as an execution
//  failure — never silently dropped.
//
//  Mirror semantics (copied from tests/m2_proof.rs, which is
//  the source of truth; if the two ever diverge, update this
//  copy): most mirrors are total (Bool equations, the arity
//  table's pinned rows). Three mirror families abstain where
//  the spec abstains, and the abstention is modeled honestly:
//    - String lookups (type_tag_name, node_kind_name,
//      binop_symbol, print_node_kind) return Option::None for
//      codes with no name row; the spec's guard constraints pin
//      nothing there, so ANY winner output is spec-legal. These
//      cases are reported as "unpinned" (winner output recorded)
//      and never counted as disagreements.
//    - node_arity returns None for unpinned kinds, but the spec
//      still constrains those outputs with `output >= 0`; those
//      cases count as pinned under that floor rule.
//  In this codebase the mirrors were co-written with the specs,
//  so the mirror abstains exactly where the spec is silent;
//  there are no inputs where the mirror opines but the spec
//  leaves the output free (other than the node_arity floor,
//  where the spec's weak opinion is the one enforced).
//
//  The harness asserts nothing about agreement — winners are
//  draw-dependent and disagreement is a measurement, not a
//  test failure. It runs 3 synthesis runs and writes the full
//  per-spec agreement matrix to target/m2-synthesis/
//  cross-validation.md. Run with:
//
//    cargo test -- --ignored --nocapture m2_cross_validation
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod xval {
    use crate::codegen;
    use crate::spec::{self, ast::TypeRef};
    use crate::synthesis::engine;
    use crate::synthesis::selector;
    use crate::verify;
    use std::collections::HashSet;
    use std::fmt::Write as _;
    use std::process::Command;
    use std::time::{Duration, Instant};

    // ─────────────────────────────────────────────────────────
    //  Value model
    // ─────────────────────────────────────────────────────────

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
        /// Mirror abstains on the value (node_arity unpinned
        /// kinds), but the spec still requires output >= floor.
        IntFloor(i64),
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
                (Mirror::IntFloor(f), Out::Int(o)) => o >= f,
                _ => false,
            }
        }

        fn render(&self) -> String {
            match self {
                Mirror::Bool(b) => b.to_string(),
                Mirror::Int(i) => i.to_string(),
                Mirror::Text(s) => format!("\"{}\"", s),
                Mirror::IntFloor(f) => format!("(any Int >= {})", f),
                Mirror::Unpinned => "(spec silent)".to_string(),
            }
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Mirrors — copied from tests/m2_proof.rs (source of
    //  truth; that file's golden tests pin these functions to
    //  the spec test cases). Option::None rows of the originals
    //  become Mirror::Unpinned / Mirror::IntFloor as documented
    //  in the module header.
    // ─────────────────────────────────────────────────────────

    fn is_known_type_tag(tag: i64) -> Mirror {
        Mirror::Bool((0..=8).contains(&tag))
    }

    fn is_known_node_kind(kind: i64) -> Mirror {
        Mirror::Bool((0..=9).contains(&kind))
    }

    /// Fixed arity per kind; variadic kinds (Call=4,
    /// ListLiteral=8) are unpinned in the spec — the original
    /// returns None there; the spec's `output >= 0` still binds.
    fn node_arity(kind: i64) -> Mirror {
        match kind {
            0 | 1 => Mirror::Int(0),
            3 | 6 => Mirror::Int(1),
            2 | 5 => Mirror::Int(2),
            _ => Mirror::IntFloor(0),
        }
    }

    fn is_literal_kind(kind: i64) -> Mirror {
        Mirror::Bool(kind == 0)
    }

    fn tags_compatible(expected: i64, actual: i64) -> Mirror {
        Mirror::Bool(expected == actual || actual == 0 || expected == 0)
    }

    fn validate_kind_tag(kind: i64) -> Mirror {
        is_known_node_kind(kind)
    }

    fn max_depth_ok(depth: i64) -> Mirror {
        Mirror::Bool((1..=64).contains(&depth))
    }

    fn type_tag_name(tag: i64) -> Mirror {
        match tag {
            1 => Mirror::Text("Int"),
            2 => Mirror::Text("Float"),
            3 => Mirror::Text("Bool"),
            4 => Mirror::Text("String"),
            6 => Mirror::Text("List"),
            7 => Mirror::Text("Option"),
            _ => Mirror::Unpinned,
        }
    }

    fn node_kind_name(kind: i64) -> Mirror {
        match kind {
            0 => Mirror::Text("Literal"),
            1 => Mirror::Text("Variable"),
            2 => Mirror::Text("BinaryOp"),
            3 => Mirror::Text("UnaryOp"),
            4 => Mirror::Text("Call"),
            5 => Mirror::Text("Index"),
            6 => Mirror::Text("FieldAccess"),
            7 => Mirror::Text("Lambda"),
            _ => Mirror::Unpinned,
        }
    }

    fn binop_symbol(op: i64) -> Mirror {
        match op {
            0 => Mirror::Text("+"),
            1 => Mirror::Text("-"),
            2 => Mirror::Text("*"),
            3 => Mirror::Text("/"),
            5 => Mirror::Text("=="),
            7 => Mirror::Text("<"),
            _ => Mirror::Unpinned,
        }
    }

    fn needs_parens(op: i64, parent_op: i64) -> Mirror {
        Mirror::Bool((op == 0 || op == 1) && (parent_op == 2 || parent_op == 3))
    }

    fn is_infix_kind(kind: i64) -> Mirror {
        Mirror::Bool(kind == 2)
    }

    fn line_width_ok(width: i64) -> Mirror {
        Mirror::Bool((0..=100).contains(&width))
    }

    fn print_node_kind(kind: i64) -> Mirror {
        node_kind_name(kind)
    }

    fn mirror(spec: &str, input: &[i64]) -> Mirror {
        match (spec, input) {
            ("is_known_type_tag", [a]) => is_known_type_tag(*a),
            ("is_known_node_kind", [a]) => is_known_node_kind(*a),
            ("node_arity", [a]) => node_arity(*a),
            ("is_literal_kind", [a]) => is_literal_kind(*a),
            ("tags_compatible", [a, b]) => tags_compatible(*a, *b),
            ("validate_kind_tag", [a]) => validate_kind_tag(*a),
            ("max_depth_ok", [a]) => max_depth_ok(*a),
            ("type_tag_name", [a]) => type_tag_name(*a),
            ("node_kind_name", [a]) => node_kind_name(*a),
            ("binop_symbol", [a]) => binop_symbol(*a),
            ("needs_parens", [a, b]) => needs_parens(*a, *b),
            ("is_infix_kind", [a]) => is_infix_kind(*a),
            ("line_width_ok", [a]) => line_width_ok(*a),
            ("print_node_kind", [a]) => print_node_kind(*a),
            _ => panic!("no mirror for spec {} with input {:?}", spec, input),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Sweeps — fixed per spec, derived from the spec + mirror
    //  semantics (documented per spec; no cherry-picking: every
    //  defined code, both sides of every range boundary,
    //  negatives, and a far-out probe are always included).
    //
    //   is_known_type_tag  tag domain 0..=8   → -2..=12 + 42,100
    //   is_known_node_kind kind domain 0..=9  → -2..=13 + 42,100
    //   node_arity         kind domain 0..=9  → -2..=13 + 42
    //   is_literal_kind    kind domain 0..=9  → -2..=13 + 42
    //   tags_compatible    tag domain 0..=8 for both params,
    //                      plus -1 and 9 out-of-domain probes
    //                      → grid {-1,0..=9}² (121 pairs)
    //   validate_kind_tag  kind domain 0..=9  → -2..=13 + 42,77,100
    //                      (77 is the spec's own far-out test)
    //   max_depth_ok       range 1..=64 → full -2..=70 + 100,1000
    //   type_tag_name      tag domain 0..=8   → -2..=12 + 42
    //   node_kind_name     kind domain 0..=9  → -2..=13 + 42
    //   binop_symbol       op domain 0..=12   → -2..=15 + 42
    //   needs_parens       op domain 0..=12 for both params;
    //                      grid over the semantically distinct
    //                      values {-1,0,1,2,3,4,5,7,12,13}²:
    //                      additive {0,1}, multiplicative {2,3},
    //                      other defined {4,5,7,12}, outside
    //                      {-1,13} (100 pairs)
    //   is_infix_kind      kind domain 0..=9  → -2..=13 + 42
    //   line_width_ok      range 0..=100 → full -2..=105 + 200,1000
    //   print_node_kind    kind domain 0..=9  → -2..=13 + 42
    // ─────────────────────────────────────────────────────────

    fn range(lo: i64, hi: i64, extra: &[i64]) -> Vec<Vec<i64>> {
        (lo..=hi)
            .chain(extra.iter().copied())
            .map(|v| vec![v])
            .collect()
    }

    fn grid(vals: &[i64]) -> Vec<Vec<i64>> {
        let mut out = Vec::new();
        for &a in vals {
            for &b in vals {
                out.push(vec![a, b]);
            }
        }
        out
    }

    fn sweep(spec: &str) -> Vec<Vec<i64>> {
        match spec {
            "is_known_type_tag" => range(-2, 12, &[42, 100]),
            "is_known_node_kind" => range(-2, 13, &[42, 100]),
            "node_arity" => range(-2, 13, &[42]),
            "is_literal_kind" => range(-2, 13, &[42]),
            "tags_compatible" => grid(&[-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            "validate_kind_tag" => range(-2, 13, &[42, 77, 100]),
            "max_depth_ok" => range(-2, 70, &[100, 1000]),
            "type_tag_name" => range(-2, 12, &[42]),
            "node_kind_name" => range(-2, 13, &[42]),
            "binop_symbol" => range(-2, 15, &[42]),
            "needs_parens" => grid(&[-1, 0, 1, 2, 3, 4, 5, 7, 12, 13]),
            "is_infix_kind" => range(-2, 13, &[42]),
            "line_width_ok" => range(-2, 105, &[200, 1000]),
            "print_node_kind" => range(-2, 13, &[42]),
            _ => panic!("no sweep defined for spec {}", spec),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Golden cases — the union of the in-spec tests and the
    //  extra golden assertions in tests/m2_proof.rs (marked †).
    //  Expectations come from the mirror (pinned to these very
    //  cases by m2_proof's golden tests); this table is the
    //  case list only.
    // ─────────────────────────────────────────────────────────

    fn singles(vals: &[i64]) -> Vec<Vec<i64>> {
        vals.iter().map(|v| vec![*v]).collect()
    }

    fn golden(spec: &str) -> Vec<Vec<i64>> {
        match spec {
            // spec tests: 0,1,8,9,42 (proof asserts the same set)
            "is_known_type_tag" => singles(&[0, 1, 8, 9, 42]),
            // spec tests: 0,9,10
            "is_known_node_kind" => singles(&[0, 9, 10]),
            // spec tests: 0,1,3,2,5; †6 (FieldAccess row),
            // †4, †8 (variadic rows, floor-pinned only)
            "node_arity" => singles(&[0, 1, 2, 3, 4, 5, 6, 8]),
            // spec tests: 0,2,4
            "is_literal_kind" => singles(&[0, 2, 4]),
            // proof-only (multi-input): Int:=Int, String:=String,
            // Int:=String, Bool:=List, Unknown actual, Unknown expected
            "tags_compatible" => vec![
                vec![1, 1],
                vec![4, 4],
                vec![1, 4],
                vec![3, 6],
                vec![1, 0],
                vec![0, 6],
            ],
            // spec tests: 4,77
            "validate_kind_tag" => singles(&[4, 77]),
            // spec tests: 1,64,65,0
            "max_depth_ok" => singles(&[0, 1, 64, 65]),
            // spec tests: 1,4,6; †2, †3, †7 (remaining table rows)
            "type_tag_name" => singles(&[1, 2, 3, 4, 6, 7]),
            // spec tests: 0,2,4,7; †1, †3, †5, †6 (remaining rows)
            "node_kind_name" => singles(&[0, 1, 2, 3, 4, 5, 6, 7]),
            // spec tests: 0,2,5,7; †1, †3 (remaining table rows)
            "binop_symbol" => singles(&[0, 1, 2, 3, 5, 7]),
            // proof-only (multi-input precedence cases)
            "needs_parens" => vec![
                vec![0, 2],
                vec![1, 3],
                vec![2, 0],
                vec![0, 1],
                vec![2, 3],
            ],
            // spec tests: 2,4,0
            "is_infix_kind" => singles(&[0, 2, 4]),
            // spec tests: 80,100,101
            "line_width_ok" => singles(&[80, 100, 101]),
            // spec tests: 2,7
            "print_node_kind" => singles(&[2, 7]),
            _ => panic!("no golden cases defined for spec {}", spec),
        }
    }

    // ─────────────────────────────────────────────────────────
    //  Winner execution: build a runner binary around the
    //  winner's generated code (verbatim) and run it once over
    //  the whole input list.
    // ─────────────────────────────────────────────────────────

    fn build_runner(func: &spec::ast::FunctionSpec, winner_code: &str, inputs: &[Vec<i64>]) -> String {
        let tuples: Vec<String> = inputs
            .iter()
            .map(|i| {
                if i.len() == 1 {
                    format!("({}, 0)", i[0])
                } else {
                    format!("({}, {})", i[0], i[1])
                }
            })
            .collect();
        let call = if func.params.len() == 1 {
            format!("{}(__a)", func.name)
        } else {
            format!("{}(__a, __b)", func.name)
        };
        format!(
            "{code}\n\nfn main() {{\n    let __inputs: &[(i64, i64)] = &[{tuples}];\n    for &(__a, __b) in __inputs {{\n        let __out = {call};\n        println!(\"{{:?}}\", __out);\n    }}\n}}\n",
            code = winner_code,
            tuples = tuples.join(", "),
            call = call,
        )
    }

    /// Inverse of Rust's `{:?}` formatting for the three output
    /// shapes the M2 specs produce. String outputs are the only
    /// ones needing unquoting; generated names/symbols contain
    /// no escapes in practice, but the unescaper is complete for
    /// the simple escape set.
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

    /// Compile and run the winner over `inputs`. Any failure —
    /// rustc error, non-zero exit, wrong output count, unparseable
    /// output — is an Err describing it; the caller records it as
    /// total disagreement for the run.
    fn execute_winner(
        run_idx: usize,
        func: &spec::ast::FunctionSpec,
        winner_code: &str,
        inputs: &[Vec<i64>],
    ) -> Result<Vec<Out>, String> {
        execute_winner_with_timeout(
            run_idx,
            func,
            winner_code,
            inputs,
            Duration::from_secs(RUNNER_TIMEOUT_SECS),
        )
    }

    /// Per-runner wall-clock limit. A sweep is at most a few
    /// hundred calls of a pure function — milliseconds for any
    /// terminating winner. The limit exists because winners can
    /// be non-terminating: once selection stopped crowning the
    /// (terminating) per-test matches, loop-bodied Inconclusive
    /// candidates began winning, and a body like
    /// `match depth { 1 => { while true { … } … }, … }` spins
    /// forever on a legal sweep input. Without a limit the
    /// harness itself hangs (observed: a Phase 16 measurement
    /// run killed after ~54 minutes at 100% CPU); with one, a
    /// non-terminating winner reads as what the harness has
    /// always counted it as — fails to run, total disagreement.
    const RUNNER_TIMEOUT_SECS: u64 = 10;

    fn execute_winner_with_timeout(
        run_idx: usize,
        func: &spec::ast::FunctionSpec,
        winner_code: &str,
        inputs: &[Vec<i64>],
        timeout: Duration,
    ) -> Result<Vec<Out>, String> {
        let dir = std::env::temp_dir().join(format!(
            "morphic_xval_{}_{}_{}",
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
            // Report the actual error lines: rustc prints warnings
            // before errors, so a head-of-stderr excerpt can show a
            // warning and hide the error that failed the compile.
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
        // Run with stdout redirected to a file (so polling
        // try_wait cannot deadlock on a full pipe) and a hard
        // deadline: kill a runner that does not finish — see
        // RUNNER_TIMEOUT_SECS for why winners can hang.
        let stdout_path = dir.join("stdout.txt");
        let stdout_file = std::fs::File::create(&stdout_path)
            .map_err(|e| format!("stdout file: {}", e))?;
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
        input: Vec<i64>,
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

    /// Case totals for a spec's sweep, computable without
    /// executing anything (used for the execution-failure path,
    /// where agreement is 0 by definition).
    fn totals(spec: &str, inputs: &[Vec<i64>]) -> (usize, usize, usize) {
        let golden_cases = golden(spec);
        let golden_set: HashSet<&Vec<i64>> = golden_cases.iter().collect();
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

    fn compare(spec: &str, inputs: &[Vec<i64>], outs: &[Out]) -> Agreement {
        assert_eq!(inputs.len(), outs.len(), "one output per input");
        let golden_cases = golden(spec);
        let golden_set: HashSet<&Vec<i64>> = golden_cases.iter().collect();
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

    fn fmt_input(input: &[i64]) -> String {
        let parts: Vec<String> = input.iter().map(|v| v.to_string()).collect();
        format!("({})", parts.join(", "))
    }

    // ─────────────────────────────────────────────────────────
    //  The harness (ignored; run explicitly)
    // ─────────────────────────────────────────────────────────

    #[test]
    #[ignore = "cross-validation harness: 3 randomized synthesis runs x 14 specs, compiles + executes winners; run explicitly with --ignored --nocapture"]
    fn m2_cross_validation() {
        let root = env!("CARGO_MANIFEST_DIR");
        let out_dir = format!("{}/target/m2-synthesis", root);
        std::fs::create_dir_all(&out_dir).unwrap();

        // Parse + typecheck both spec files once; specs are
        // inputs, identical across runs.
        let mut modules: Vec<(String, Vec<spec::ast::FunctionSpec>)> = Vec::new();
        for file in ["validator", "printer"] {
            let path = format!("{}/examples/self_host/{}.morph", root, file);
            let source = std::fs::read_to_string(&path).unwrap();
            let parsed = spec::parser::parse(&source).expect("parse must succeed");
            let checked = spec::typeck::check(parsed).expect("typeck must succeed");
            modules.push((file.to_string(), checked.spec.functions.clone()));
        }

        let mut md = String::new();
        let _ = writeln!(md, "# M2 cross-validation: synthesized winners vs tests/m2_proof.rs mirrors");
        let _ = writeln!(md);
        let _ = writeln!(md, "Pipeline per spec per run: parse → typecheck → synthesize (opt 1) → Z3 verify_all → select_best → codegen (rust), then the winner is compiled with rustc and EXECUTED over the spec's golden cases + input sweep (see src/m2_cross_validation.rs for sweep definitions and mirror semantics).");
        let _ = writeln!(md, "A winner that fails to compile/run counts as total disagreement for that run (exec FAIL).");
        let _ = writeln!(md);
        let _ = writeln!(md, "| Run | Spec | Winner tpr | Z3 verified | Golden agree | Sweep pinned agree | Unpinned cases | Exec |");
        let _ = writeln!(md, "|---|---|---|---|---|---|---|---|");

        let mut details = String::new();
        let _ = writeln!(details, "\n## Disagreements (pinned cases only, first 5 per spec per run)\n");

        for run in 1..=3usize {
            for (_file, functions) in &modules {
                for func in functions {
                    let name = func.name.clone();
                    let inputs = sweep(&name);
                    let (golden_total, pinned_total, unpinned_total) = totals(&name, &inputs);

                    // Real pipeline, same shape as the probe and
                    // `morphic build` (synthesize is wrapped: the
                    // randomized engine may panic on some draws).
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        engine::synthesize(func, 1)
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
                    let verified = match verify::verifier::verify_all(candidates, func) {
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
                    // Save the winner next to the probe's outputs
                    // (run-suffixed so runs don't overwrite).
                    let winner_path = format!("{}/{}_xval_run{}.rs", out_dir, name, run);
                    let _ = std::fs::write(&winner_path, &winner_code);

                    match execute_winner(run, func, &winner_code, &inputs) {
                        Ok(outs) => {
                            let ag = compare(&name, &inputs, &outs);
                            println!(
                                "XVAL|run={}|spec={}|tpr={:.3}|verified={}|golden={}/{}|sweep_pinned={}/{}|unpinned={}|exec=ok",
                                run, name, winner.scores.test_pass_ratio, winner.verified,
                                ag.golden_agree, ag.golden_total,
                                ag.sweep_pinned_agree, ag.sweep_pinned_total,
                                ag.unpinned
                            );
                            for d in ag.disagreements.iter().take(5) {
                                println!(
                                    "XVAL-DIS|run={}|spec={}|input={}|winner={}|mirror={}",
                                    run, name, fmt_input(&d.input), d.winner.render(), d.mirror.render()
                                );
                            }
                            let _ = writeln!(
                                md,
                                "| {} | {} | {:.3} | {} | {}/{} | {}/{} | {} | ok |",
                                run, name, winner.scores.test_pass_ratio, winner.verified,
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
                                        fmt_input(&d.input),
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
        }

        md.push_str(&details);
        let report_path = format!("{}/cross-validation.md", out_dir);
        std::fs::write(&report_path, &md).unwrap();
        println!("XVAL-REPORT|written to {}", report_path);
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
            "XVAL|run={}|spec={}|exec=FAIL({})|golden=0/{}|sweep_pinned=0/{}|unpinned={}",
            run, func.name, why, golden_total, pinned_total, unpinned_total
        );
        let _ = writeln!(
            md,
            "| {} | {} | - | - | 0/{} | 0/{} | {} | FAIL: {} |",
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
    //  detect disagreement, not just agreement.
    // ─────────────────────────────────────────────────────────

    fn dummy_func(name: &str, n_params: usize, return_type: TypeRef) -> spec::ast::FunctionSpec {
        spec::ast::FunctionSpec {
            name: name.to_string(),
            doc: None,
            generics: Vec::new(),
            params: (0..n_params)
                .map(|i| spec::ast::Param {
                    name: format!("p{}", i),
                    ty: TypeRef::Int,
                    annotations: Default::default(),
                })
                .collect(),
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
    fn xval_golden_cases_lie_inside_the_sweeps() {
        // The harness reads golden results off the sweep
        // execution, so every golden input must be swept; sweep
        // arity must match the spec's parameter count; and the
        // mirror dispatch must cover every swept input.
        for name in [
            "is_known_type_tag",
            "is_known_node_kind",
            "node_arity",
            "is_literal_kind",
            "tags_compatible",
            "validate_kind_tag",
            "max_depth_ok",
            "type_tag_name",
            "node_kind_name",
            "binop_symbol",
            "needs_parens",
            "is_infix_kind",
            "line_width_ok",
            "print_node_kind",
        ] {
            let sw = sweep(name);
            let arity = sw[0].len();
            assert!(sw.iter().all(|i| i.len() == arity), "{}: ragged sweep", name);
            for g in golden(name) {
                assert!(sw.contains(&g), "{}: golden {:?} not in sweep", name, g);
            }
            for input in &sw {
                let _ = mirror(name, input); // must not panic
            }
        }
    }

    #[test]
    fn xval_compare_counts_agreement_and_disagreement() {
        // Bool spec: hand-built outputs — one wrong value must be
        // detected, counted, and reported with its input.
        let inputs = vec![vec![0], vec![2], vec![4]];
        let outs = vec![Out::Bool(true), Out::Bool(true), Out::Bool(false)];
        let ag = compare("is_literal_kind", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 3);
        assert_eq!(ag.sweep_pinned_agree, 2);
        assert_eq!(ag.golden_total, 3); // 0, 2, 4 are all golden
        assert_eq!(ag.golden_agree, 2);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, vec![2]);
        assert_eq!(ag.disagreements[0].winner, Out::Bool(true));
        assert_eq!(ag.disagreements[0].mirror, Mirror::Bool(false));

        // String spec with an unpinned input: the pinned row is
        // compared, the unpinned one is recorded, not counted.
        let inputs = vec![vec![1], vec![0]];
        let outs = vec![Out::Text("Int".to_string()), Out::Text("whatever".to_string())];
        let ag = compare("type_tag_name", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 1);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.golden_total, 1); // only input 1 is golden here
        assert_eq!(ag.golden_agree, 1);
        assert_eq!(ag.unpinned, 1);
        assert_eq!(ag.unpinned_winner_values, vec![Out::Text("whatever".to_string())]);
        assert!(ag.disagreements.is_empty());

        // node_arity floor rule: unpinned kinds agree iff >= 0.
        let inputs = vec![vec![6], vec![4]];
        let outs = vec![Out::Int(0), Out::Int(-1)];
        let ag = compare("node_arity", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 0);
        assert_eq!(ag.disagreements.len(), 2);
    }

    #[test]
    fn xval_runner_executes_and_detects() {
        // End-to-end mechanics: hand-written winners compiled and
        // run through the same runner path the harness uses. The
        // correct one must fully agree; the wrong ones must be
        // caught on exactly the inputs where they differ.
        let bool_func = dummy_func("is_literal_kind", 1, TypeRef::Bool);
        let inputs = vec![vec![0], vec![2], vec![4], vec![7]];

        let correct = "pub fn is_literal_kind(kind: i64) -> bool {\n    kind == 0\n}\n";
        let outs = execute_winner(0, &bool_func, correct, &inputs).expect("correct winner must run");
        let ag = compare("is_literal_kind", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_agree, ag.sweep_pinned_total);
        assert!(ag.disagreements.is_empty());

        // A non-terminating winner must read as "fails to run"
        // (total disagreement), never hang the harness: loop
        // bodies are exactly what the verifier calls
        // Inconclusive, and they win pools once refuted
        // candidates are gated out. Short limit so the suite
        // stays fast; the production limit is
        // RUNNER_TIMEOUT_SECS.
        let hanging = "pub fn is_literal_kind(kind: i64) -> bool {\n    loop {}\n}\n";
        let err = execute_winner_with_timeout(
            0,
            &bool_func,
            hanging,
            &inputs,
            Duration::from_secs(1),
        )
        .expect_err("non-terminating winner must fail, not hang");
        assert!(
            err.contains("timed out"),
            "timeout must be the recorded failure, got: {}",
            err
        );

        let wrong = "pub fn is_literal_kind(kind: i64) -> bool {\n    kind == 2\n}\n";
        let outs = execute_winner(0, &bool_func, wrong, &inputs).expect("wrong winner must run");
        let ag = compare("is_literal_kind", &inputs, &outs);
        let bad: Vec<Vec<i64>> = ag.disagreements.iter().map(|d| d.input.clone()).collect();
        assert_eq!(bad, vec![vec![0], vec![2]]);

        // String outputs: quoting/unquoting round-trip, plus an
        // unpinned input that must not count as a disagreement.
        let str_func = dummy_func("type_tag_name", 1, TypeRef::String);
        let inputs = vec![vec![1], vec![2], vec![0]];
        let partial = "pub fn type_tag_name(tag: i64) -> String {\n    match tag { 1 => \"Int\".to_string(), _ => \"\".to_string() }\n}\n";
        let outs = execute_winner(0, &str_func, partial, &inputs).expect("string winner must run");
        let ag = compare("type_tag_name", &inputs, &outs);
        assert_eq!(ag.sweep_pinned_total, 2);
        assert_eq!(ag.sweep_pinned_agree, 1);
        assert_eq!(ag.unpinned, 1);
        assert_eq!(ag.disagreements.len(), 1);
        assert_eq!(ag.disagreements[0].input, vec![2]);
    }
}
