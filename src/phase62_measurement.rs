// ─────────────────────────────────────────────────────────────
//  PHASE 62 MEASUREMENT (test-only)
//
//  The Clone bound (owner decision (i)) + the generation
//  campaign's CLOSING MEASUREMENT.
//
//  - Part A (in-process pipeline draws, the Phase 61 Part B
//    pattern): 5 draws of sort through the full pipeline
//    (synthesize → verify_all → G1 runner stage →
//    select_best), recording the scan template's presence,
//    its own runner verdict (singleton pool), its finalist
//    status in the stage, and the crown's identity /
//    provenance / recorded tpr / compiled verdict /
//    whole-file verdict. 5 control draws of count_matching
//    assert the Phase 60 outcome stands (forall-fold crown,
//    tpr 1.000, compiled 3/3).
//  - Part B (real binaries, BOTH configurations — the
//    m5_s2 nested-build pattern): each configuration builds
//    the four roundtrip files (sort, binary_search,
//    count_matching, is_sorted) at `-O 1`, 5 draws each;
//    per draw: end-to-end build, whole-file compile (incl.
//    the generated test module, as a lib), function-only
//    compile, test-module compile + execution with
//    passed/failed counts. Outcomes are RECORDED per draw,
//    never averaged away; only the binary's end-to-end
//    build success is asserted per draw (the m5_s2
//    precedent).
//
//  The checkbox-3 criteria (Phase 50 form, Phase 62 task):
//  tick the ROAD M5 roundtrip checkbox ONLY if every drawn
//  sort output in BOTH configurations compiles whole
//  (incl. test module) AND passes all 6 in-spec tests.
//  The verdict is computed and printed here; the ROAD note
//  records it.
//
//  Run with:
//    cargo test -- --ignored --nocapture phase62_measurement
//  Report: target/phase62/measurement-report.md
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod measurement {
    use crate::codegen;
    use crate::spec::ast::FunctionSpec;
    use crate::spec::{parser, typeck};
    use crate::synthesis::engine::{self, CandidateImplementation, Provenance};
    use crate::synthesis::runner::{rescore_finalists_report, RunnerOutcome};
    use crate::synthesis::selector;
    use crate::verify;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    fn load_functions(rel_path: &str) -> Vec<FunctionSpec> {
        let root = env!("CARGO_MANIFEST_DIR");
        let source = std::fs::read_to_string(format!("{}/{}", root, rel_path))
            .unwrap_or_else(|e| panic!("{}: read failed: {}", rel_path, e));
        let parsed = parser::parse(&source)
            .unwrap_or_else(|e| panic!("{}: parse failed: {}", rel_path, e));
        let checked = typeck::check(parsed)
            .unwrap_or_else(|e| panic!("{}: typeck failed: {}", rel_path, e));
        checked.spec.functions
    }

    fn body_key(c: &CandidateImplementation) -> String {
        format!("{:?}", c.body)
    }

    fn short_hash(text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn provenance_name(c: &CandidateImplementation) -> String {
        match &c.provenance {
            Provenance::Template { template_name } => format!("Template({})", template_name),
            Provenance::TestInduced { .. } => "TestInduced".into(),
            Provenance::ConstraintInduced { kind } => {
                format!("ConstraintInduced({})", kind)
            }
            Provenance::Mutation { mutation_kind, .. } => {
                format!("Mutation({})", mutation_kind)
            }
            Provenance::Crossover { .. } => "Crossover".into(),
            Provenance::LLMGenerated { .. } => "LLMGenerated".into(),
            Provenance::HumanGuided => "HumanGuided".into(),
        }
    }

    /// Run a program with stdout/stderr captured to files;
    /// returns (success, stdout) — the Phase 61 vehicle.
    fn run_out(
        dir: &str,
        tag: &str,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Option<(bool, String)> {
        run_captured(dir, tag, program, args, None, timeout)
            .map(|(ok, stdout, _)| (ok, stdout))
    }

    /// Run a program with stdout/stderr captured to files;
    /// returns (success, stderr) — the m5_s2 vehicle.
    fn run_err(
        dir: &str,
        tag: &str,
        program: &str,
        args: &[&str],
        cwd: &str,
        timeout: Duration,
    ) -> Option<(bool, String)> {
        run_captured(dir, tag, program, args, Some(cwd), timeout)
            .map(|(ok, _, stderr)| (ok, stderr))
    }

    fn run_captured(
        dir: &str,
        tag: &str,
        program: &str,
        args: &[&str],
        cwd: Option<&str>,
        timeout: Duration,
    ) -> Option<(bool, String, String)> {
        let out_path = format!("{}/{}.stdout", dir, tag);
        let err_path = format!("{}/{}.stderr", dir, tag);
        let out_file = std::fs::File::create(&out_path).ok()?;
        let err_file = std::fs::File::create(&err_path).ok()?;
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdout(Stdio::from(out_file))
            .stderr(Stdio::from(err_file));
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        let mut child = cmd.spawn().ok()?;
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout = std::fs::read_to_string(&out_path).unwrap_or_default();
                    let stderr = std::fs::read_to_string(&err_path).unwrap_or_default();
                    return Some((status.success(), stdout, stderr));
                }
                Ok(None) => {
                    if started.elapsed() > timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(_) => return None,
            }
        }
    }

    fn function_part(generated: &str) -> String {
        match generated.lines().position(|l| l.trim() == "#[cfg(test)]") {
            Some(idx) => generated.lines().take(idx).collect::<Vec<_>>().join("\n") + "\n",
            None => generated.to_string(),
        }
    }

    /// Compile a source text as a library; returns
    /// (success, sorted unique error codes from stderr).
    fn rustc_lib(dir: &str, tag: &str, source: &str) -> (bool, Vec<String>) {
        let rs = format!("{}/{}.rs", dir, tag);
        std::fs::write(&rs, source).unwrap();
        let out = run_out(
            dir,
            &format!("{}_cc", tag),
            "rustc",
            &[
                "--edition",
                "2021",
                "--crate-type",
                "lib",
                &rs,
                "-o",
                &format!("{}/{}.rlib", dir, tag),
            ],
            Duration::from_secs(60),
        );
        let err_text =
            std::fs::read_to_string(format!("{}/{}_cc.stderr", dir, tag)).unwrap_or_default();
        let mut codes: Vec<String> = Vec::new();
        for line in err_text.lines() {
            if let Some(start) = line.find("error[E") {
                let code: String = line[start + 6..].chars().take(5).collect();
                if !codes.contains(&code) {
                    codes.push(code);
                }
            } else if line.starts_with("error:") && !codes.contains(&"error".to_string()) {
                codes.push("error".to_string());
            }
        }
        codes.sort();
        (matches!(out, Some((true, _))), codes)
    }

    /// (function compiles, tests (passed, total)) — the
    /// Phase 53 compiled vehicle, as in Phases 59–61.
    fn judge_compiled(
        candidate: &CandidateImplementation,
        runner_main: &str,
        dir: &str,
        tag: &str,
    ) -> (bool, Option<(usize, usize)>) {
        let generated = match codegen::generate(candidate, "rust") {
            Ok(text) => text,
            Err(_) => return (false, None),
        };
        let func_src = function_part(&generated);
        let (function_compiles, _) = rustc_lib(dir, &format!("{}_fn", tag), &func_src);
        let run_rs = format!("{}/{}_run.rs", dir, tag);
        std::fs::write(&run_rs, format!("{}{}", func_src, runner_main)).unwrap();
        let run_bin = format!("{}/{}_run", dir, tag);
        let compiled = run_out(
            dir,
            &format!("{}_runcc", tag),
            "rustc",
            &["--edition", "2021", &run_rs, "-o", &run_bin],
            Duration::from_secs(60),
        );
        let tests = match compiled {
            Some((true, _)) => {
                match run_out(dir, &format!("{}_run", tag), &run_bin, &[], Duration::from_secs(10))
                {
                    Some((_, stdout)) => stdout.lines().find_map(|line| {
                        let rest = line.strip_prefix("TOTAL ")?;
                        let (p, t) = rest.split_once('/')?;
                        Some((p.trim().parse().ok()?, t.trim().parse().ok()?))
                    }),
                    None => None,
                }
            }
            _ => None,
        };
        (function_compiles, tests)
    }

    /// Compile the WHOLE generated file (function + test
    /// module) as a test binary and run it: (whole file
    /// compiles as --test, tests (passed, failed), codes).
    fn judge_whole_file(
        cand: &CandidateImplementation,
        dir: &str,
        tag: &str,
    ) -> (bool, Option<(usize, usize)>, Vec<String>) {
        let generated = match codegen::generate(cand, "rust") {
            Ok(text) => text,
            Err(_) => return (false, None, vec!["generate".to_string()]),
        };
        let rs = format!("{}/{}_whole.rs", dir, tag);
        std::fs::write(&rs, &generated).unwrap();
        let bin = format!("{}/{}_whole", dir, tag);
        let compiled = run_out(
            dir,
            &format!("{}_wholecc", tag),
            "rustc",
            &["--edition", "2021", "--test", &rs, "-o", &bin],
            Duration::from_secs(60),
        );
        let err_text =
            std::fs::read_to_string(format!("{}/{}_wholecc.stderr", dir, tag)).unwrap_or_default();
        let mut codes: Vec<String> = Vec::new();
        for line in err_text.lines() {
            if let Some(start) = line.find("error[E") {
                let code: String = line[start + 6..].chars().take(5).collect();
                if !codes.contains(&code) {
                    codes.push(code);
                }
            }
        }
        codes.sort();
        let compiles = matches!(compiled, Some((true, _)));
        let tests = if compiles {
            match run_out(dir, &format!("{}_whole", tag), &bin, &[], Duration::from_secs(10)) {
                Some((_, stdout)) => stdout.lines().find_map(|line| {
                    let rest = line.strip_prefix("test result:")?;
                    let mut passed = None;
                    let mut failed = None;
                    for part in rest.split(';') {
                        let part = part.trim();
                        if let Some(n) = part.strip_suffix(" passed") {
                            passed =
                                n.split_whitespace().last().and_then(|t| t.parse::<usize>().ok());
                        }
                        if let Some(n) = part.strip_suffix(" failed") {
                            failed =
                                n.split_whitespace().last().and_then(|t| t.parse::<usize>().ok());
                        }
                    }
                    Some((passed?, failed?))
                }),
                None => None,
            }
        } else {
            None
        };
        (compiles, tests, codes)
    }

    const SORT_MAIN: &str = r#"
fn check(name: &str, got: std::thread::Result<Vec<i64>>, want: Vec<i64>, results: &mut Vec<bool>) {
    let ok = matches!(&got, Ok(v) if *v == want);
    println!("{}: {}", name, if ok { "PASS" } else { "FAIL" });
    results.push(ok);
}
fn main() {
    let lt = |a: i64, b: i64| a < b;
    let mut r: Vec<bool> = Vec::new();
    check("empty", std::panic::catch_unwind(|| sort::<i64>(vec![], lt)), vec![], &mut r);
    check("single", std::panic::catch_unwind(|| sort::<i64>(vec![5], lt)), vec![5], &mut r);
    check("sorted", std::panic::catch_unwind(|| sort::<i64>(vec![1, 2, 3, 4, 5], lt)), vec![1, 2, 3, 4, 5], &mut r);
    check("reverse", std::panic::catch_unwind(|| sort::<i64>(vec![5, 4, 3, 2, 1], lt)), vec![1, 2, 3, 4, 5], &mut r);
    check("random", std::panic::catch_unwind(|| sort::<i64>(vec![3, 1, 4, 1, 5, 9, 2, 6], lt)), vec![1, 1, 2, 3, 4, 5, 6, 9], &mut r);
    check("dups", std::panic::catch_unwind(|| sort::<i64>(vec![2, 2, 1, 1, 3, 3], lt)), vec![1, 1, 2, 2, 3, 3], &mut r);
    let passed = r.iter().filter(|b| **b).count();
    println!("TOTAL {}/6", passed);
}
"#;

    const COUNT_MAIN: &str = r#"
fn check(name: &str, got: std::thread::Result<i64>, want: i64, results: &mut Vec<bool>) {
    let ok = matches!(&got, Ok(v) if *v == want);
    println!("{}: {}", name, if ok { "PASS" } else { "FAIL" });
    results.push(ok);
}
fn main() {
    let mut r: Vec<bool> = Vec::new();
    check("all-match", std::panic::catch_unwind(|| count_matching(vec![10, 20, 30], 5)), 3, &mut r);
    check("none-match", std::panic::catch_unwind(|| count_matching(vec![1, 2, 3], 5)), 0, &mut r);
    check("mixed", std::panic::catch_unwind(|| count_matching(vec![1, 10, 2, 20], 5)), 2, &mut r);
    let passed = r.iter().filter(|b| **b).count();
    println!("TOTAL {}/3", passed);
}
"#;

    fn verified_pool(
        functions: &[FunctionSpec],
        spec_name: &str,
    ) -> (FunctionSpec, Vec<CandidateImplementation>) {
        let func = functions
            .iter()
            .find(|f| f.name == spec_name)
            .unwrap_or_else(|| panic!("no spec named {}", spec_name))
            .clone();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine::synthesize_with_module(&func, 1, functions)
        }));
        let candidates = match outcome {
            Ok(Ok(c)) if !c.is_empty() => c,
            Ok(Ok(_)) => panic!("{}: synthesis returned no candidates", spec_name),
            Ok(Err(e)) => panic!("{}: synthesis error: {}", spec_name, e),
            Err(_) => panic!("{}: synthesis panicked", spec_name),
        };
        let verified = verify::verifier::Verifier::with_specs(functions)
            .verify_all(candidates, &func)
            .unwrap_or_else(|e| panic!("{}: verify_all error: {}", spec_name, e));
        (func, verified)
    }

    fn template_candidate<'a>(
        pool: &'a [CandidateImplementation],
        kind_name: &str,
    ) -> Option<&'a CandidateImplementation> {
        pool.iter().find(|c| {
            matches!(&c.provenance, Provenance::Template { template_name } if template_name == kind_name)
        })
    }

    /// The template body's own verdict under the G1 runner
    /// (singleton pool: the candidate is the would-be
    /// winner, hence a guaranteed finalist).
    fn runner_verdict_of(candidate: &CandidateImplementation, spec: &FunctionSpec) -> (usize, usize) {
        let (_, outcome) = rescore_finalists_report(vec![candidate.clone()], spec);
        match outcome {
            RunnerOutcome::Scored { finalists, .. } => {
                assert_eq!(finalists.len(), 1, "singleton pool must score exactly one finalist");
                (finalists[0].passed, finalists[0].total)
            }
            other => panic!("runner must trigger for {}: got {:?}", spec.name, other),
        }
    }

    // ── Part B: per-draw file measurement (the m5_s2
    // measure_draw, copied) ───────────────────────────────

    /// Parse-class error patterns in rustc stderr (the
    /// Phase 54 catalogue). Name-resolution (E0425), arity
    /// (E0061), type (E0308) and the E0689 junk class are
    /// later stages, not parse errors.
    fn has_parse_error(stderr: &str) -> bool {
        const PATTERNS: [&str; 7] = [
            "expected item",
            "expected expression",
            "expected one of",
            "unexpected token",
            "mismatched closing delimiter",
            "unknown start of token",
            "cannot parse",
        ];
        stderr
            .lines()
            .any(|l| l.starts_with("error") && PATTERNS.iter().any(|p| l.contains(p)))
    }

    fn error_codes(stderr: &str) -> Vec<String> {
        let mut codes: Vec<String> = Vec::new();
        for line in stderr.lines() {
            if let Some(idx) = line.find("[E") {
                let rest = &line[idx + 1..];
                if let Some(end) = rest.find(']') {
                    let code = &rest[..end];
                    if code.len() == 5 && !codes.contains(&code.to_string()) {
                        codes.push(code.to_string());
                    }
                }
            }
        }
        codes.sort();
        codes
    }

    fn first_error_line(stderr: &str) -> String {
        stderr
            .lines()
            .find(|l| l.starts_with("error"))
            .unwrap_or("")
            .chars()
            .take(140)
            .collect()
    }

    struct FileDraw {
        build_ok: bool,
        whole_compiles: bool,
        parse_ok: bool,
        whole_codes: Vec<String>,
        whole_first: String,
        function_compiles: bool,
        function_class: String,
        testmod_compiles: bool,
        testmod_codes: Vec<String>,
        tests_passed: Option<usize>,
        tests_failed: Option<usize>,
    }

    fn measure_draw(
        root: &str,
        dir: &str,
        bin: &str,
        file: &str,
        tag: &str,
    ) -> FileDraw {
        let out_rs = format!("{}/{}.rs", dir, tag);
        let src_path = format!("{}/{}", root, file);
        let build = run_err(
            dir,
            &format!("{}_build", tag),
            bin,
            &["build", &src_path, "--target", "rust", "-O", "1", "-o", &out_rs],
            root,
            Duration::from_secs(120),
        );
        let build_ok = matches!(build, Some((true, _))) && std::path::Path::new(&out_rs).exists();
        let fail = |class: &str| FileDraw {
            build_ok: false,
            whole_compiles: false,
            parse_ok: false,
            whole_codes: vec![],
            whole_first: String::new(),
            function_compiles: false,
            function_class: class.to_string(),
            testmod_compiles: false,
            testmod_codes: vec![],
            tests_passed: None,
            tests_failed: None,
        };
        if !build_ok {
            return fail("build failed");
        }
        let generated = std::fs::read_to_string(&out_rs).unwrap_or_default();
        let whole = run_err(
            dir,
            &format!("{}_whole", tag),
            "rustc",
            &[
                "--edition",
                "2021",
                "--crate-type",
                "lib",
                &out_rs,
                "-o",
                &format!("{}/{}.rlib", dir, tag),
            ],
            root,
            Duration::from_secs(120),
        );
        let (whole_compiles, whole_stderr) = match &whole {
            Some((ok, e)) => (*ok, e.clone()),
            None => (false, "timeout".to_string()),
        };
        let parse_ok = whole_compiles || !has_parse_error(&whole_stderr);
        let fn_rs = format!("{}/{}_fn.rs", dir, tag);
        std::fs::write(&fn_rs, function_part(&generated)).unwrap();
        let func = run_err(
            dir,
            &format!("{}_func", tag),
            "rustc",
            &[
                "--edition",
                "2021",
                "--crate-type",
                "lib",
                &fn_rs,
                "-o",
                &format!("{}/{}_fn.rlib", dir, tag),
            ],
            root,
            Duration::from_secs(120),
        );
        let (function_compiles, func_stderr) = match &func {
            Some((ok, e)) => (*ok, e.clone()),
            None => (false, "timeout".to_string()),
        };
        let function_class = if function_compiles {
            String::new()
        } else if func_stderr.contains("E0689") {
            "E0689 junk class".to_string()
        } else {
            format!("other {}", error_codes(&func_stderr).join("/"))
        };
        let test_bin = format!("{}/{}_testbin", dir, tag);
        let testc = run_err(
            dir,
            &format!("{}_testc", tag),
            "rustc",
            &["--edition", "2021", "--test", &out_rs, "-o", &test_bin],
            root,
            Duration::from_secs(120),
        );
        let (testmod_compiles, testc_stderr) = match &testc {
            Some((ok, e)) => (*ok, e.clone()),
            None => (false, "timeout".to_string()),
        };
        let (mut tests_passed, mut tests_failed) = (None, None);
        if testmod_compiles {
            let run = run_err(
                dir,
                &format!("{}_testrun", tag),
                &test_bin,
                &[],
                root,
                Duration::from_secs(30),
            );
            if run.is_some() {
                let stdout = std::fs::read_to_string(format!("{}/{}_testrun.stdout", dir, tag))
                    .unwrap_or_default();
                for line in stdout.lines() {
                    if let Some(rest) = line.strip_prefix("test result:") {
                        let nums: Vec<usize> = rest
                            .split(|c: char| !c.is_ascii_digit())
                            .filter(|t| !t.is_empty())
                            .filter_map(|t| t.parse().ok())
                            .collect();
                        if nums.len() >= 2 {
                            tests_passed = Some(nums[0]);
                            tests_failed = Some(nums[1]);
                        }
                    }
                }
            }
        }
        FileDraw {
            build_ok,
            whole_compiles,
            parse_ok,
            whole_codes: error_codes(&whole_stderr),
            whole_first: first_error_line(&whole_stderr),
            function_compiles,
            function_class,
            testmod_compiles,
            testmod_codes: error_codes(&testc_stderr),
            tests_passed,
            tests_failed,
        }
    }

    #[test]
    #[ignore = "Phase 62 closing measurement: 5 in-process sort draws + 5 count_matching controls + real binaries in both configurations x 4 files x 5 draws; run explicitly with --ignored --nocapture phase62_measurement"]
    fn phase62_measurement() {
        let root = env!("CARGO_MANIFEST_DIR").to_string();
        let out_dir = format!("{}/target/phase62", root);
        let draws_dir = format!("{}/draws", out_dir);
        std::fs::create_dir_all(&draws_dir).unwrap();
        let report_path = format!("{}/measurement-report.md", out_dir);
        let mut report = String::new();
        let mut writelnr = |report: &mut String, s: &str| {
            println!("{}", s);
            report.push_str(s);
            report.push('\n');
        };
        writelnr(
            &mut report,
            "# Phase 62 measurement — the Clone bound + the campaign's closing measurement",
        );
        writelnr(&mut report, "");

        // ── Part A: in-process pipeline draws ──
        writelnr(&mut report, "## Part A — sort draws (full pipeline, in-process)");
        let sort_functions = load_functions("examples/sort.morph");
        let mut template_verdicts: Vec<(usize, usize)> = Vec::new();
        let mut template_finalist = 0usize;
        let mut crown_is_template = 0usize;
        for draw in 0..5 {
            let (func, pool) = verified_pool(&sort_functions, "sort");
            let template = template_candidate(&pool, "sort_primitive").cloned();
            let (in_pool, verdict) = match &template {
                Some(t) => (true, Some(runner_verdict_of(t, &func))),
                None => (false, None),
            };
            if let Some(v) = verdict {
                template_verdicts.push(v);
            }
            let (rescored, outcome) = rescore_finalists_report(pool, &func);
            let (is_finalist, capped) = match (&outcome, &template) {
                (
                    RunnerOutcome::Scored {
                        finalists,
                        capped_non_finalists,
                        ..
                    },
                    Some(t),
                ) => (
                    finalists.iter().any(|f| f.candidate_id == t.id),
                    *capped_non_finalists,
                ),
                _ => (false, 0),
            };
            if is_finalist {
                template_finalist += 1;
            }
            let crown = selector::select_best(rescored, &func)
                .unwrap_or_else(|e| panic!("sort draw {}: select_best: {}", draw, e));
            let is_template_crown = matches!(
                &crown.provenance,
                Provenance::Template { template_name } if template_name == "sort_primitive"
            );
            if is_template_crown {
                crown_is_template += 1;
            }
            let (compiles, tests) =
                judge_compiled(&crown, SORT_MAIN, &draws_dir, &format!("a_sort_crown_{}", draw));
            let (whole_compiles, whole_tests, whole_codes) =
                judge_whole_file(&crown, &draws_dir, &format!("a_sort_draw_{}", draw));
            writelnr(
                &mut report,
                &format!(
                    "- draw {}: template in pool {}, verdict {:?}, finalist {}, capped {}; crown {} [{}] recorded tpr {:.3}; crown function compiles {}, compiled verdict {:?}; crown whole file compiles {}, generated tests {:?}, codes {:?}",
                    draw,
                    in_pool,
                    verdict,
                    is_finalist,
                    capped,
                    short_hash(&body_key(&crown)),
                    provenance_name(&crown),
                    crown.scores.test_pass_ratio,
                    compiles,
                    tests,
                    whole_compiles,
                    whole_tests,
                    whole_codes,
                ),
            );
            std::fs::write(&report_path, &report).unwrap();
        }
        writelnr(
            &mut report,
            &format!(
                "**sort Part A: template verdicts {:?}; template finalist {}/5; crown is the template {}/5**",
                template_verdicts, template_finalist, crown_is_template
            ),
        );
        writelnr(&mut report, "");

        // ── Part A control: count_matching ──
        writelnr(
            &mut report,
            "## Part A control — count_matching (Phase 60 outcome re-verified)",
        );
        let cm_functions = load_functions("examples/self_host/count_matching.morph");
        for draw in 0..5 {
            let (func, pool) = verified_pool(&cm_functions, "count_matching");
            let _ = func;
            let seed = pool
                .iter()
                .find(|c| matches!(
                    &c.provenance,
                    Provenance::ConstraintInduced { kind } if kind == "forall_fold"
                ))
                .unwrap_or_else(|| panic!("count_matching draw {}: forall-fold seed missing", draw))
                .clone();
            let (rescored, outcome) = rescore_finalists_report(pool, &cm_functions[0]);
            assert!(
                matches!(outcome, RunnerOutcome::Scored { .. }),
                "count_matching draw {}: the stage must score (Phase 60 arm), got {:?}",
                draw,
                outcome
            );
            let crown = selector::select_best(rescored, &cm_functions[0]).expect("select_best");
            assert_eq!(
                body_key(&crown),
                body_key(&seed),
                "count_matching draw {}: the crown must remain the forall-fold seed",
                draw
            );
            assert!(
                (crown.scores.test_pass_ratio - 1.0).abs() < 1e-9,
                "count_matching draw {}: crown tpr must be 1.000, got {}",
                draw,
                crown.scores.test_pass_ratio
            );
            let (_, tests) =
                judge_compiled(&crown, COUNT_MAIN, &draws_dir, &format!("a_cm_crown_{}", draw));
            assert_eq!(
                tests,
                Some((3, 3)),
                "count_matching draw {}: crown must pass 3/3 compiled",
                draw
            );
            writelnr(
                &mut report,
                &format!(
                    "- draw {}: crown {} [{}] recorded tpr {:.3}, compiled verdict {:?}",
                    draw,
                    short_hash(&body_key(&crown)),
                    provenance_name(&crown),
                    crown.scores.test_pass_ratio,
                    tests,
                ),
            );
            std::fs::write(&report_path, &report).unwrap();
        }
        writelnr(&mut report, "");

        // ── Part B: real binaries, both configurations ──
        writelnr(
            &mut report,
            "## Part B — real binaries, both configurations (4 files x 5 draws each)",
        );
        let off = run_err(
            &out_dir,
            "build_off",
            "cargo",
            &["build"],
            &root,
            Duration::from_secs(900),
        );
        assert!(
            matches!(off, Some((true, _))),
            "the flag-off cargo build failed: {:?}",
            off.map(|(_, e)| e)
        );
        let bin_off = format!("{}/morphic-off", out_dir);
        std::fs::copy(format!("{}/target/debug/morphic", root), &bin_off).unwrap();
        let on = run_err(
            &out_dir,
            "build_on",
            "cargo",
            &["build", "--features", "bootstrap-codegen"],
            &root,
            Duration::from_secs(900),
        );
        assert!(
            matches!(on, Some((true, _))),
            "the flag-on cargo build failed: {:?}",
            on.map(|(_, e)| e)
        );
        let bin_on = format!("{}/morphic-on", out_dir);
        std::fs::copy(format!("{}/target/debug/morphic", root), &bin_on).unwrap();
        let _ = run_err(
            &out_dir,
            "build_restore",
            "cargo",
            &["build"],
            &root,
            Duration::from_secs(900),
        );
        writelnr(
            &mut report,
            "Both binaries built from this tree: flag-off (original compiler) and flag-on (`--features bootstrap-codegen`). Each builds each file at `-O 1`, five draws.",
        );
        writelnr(&mut report, "");

        // (file, spec name, in-spec test count)
        let files: [(&str, &str, usize); 4] = [
            ("examples/sort.morph", "sort", 6),
            ("examples/binary_search.morph", "binary_search", 7),
            ("examples/self_host/count_matching.morph", "count_matching", 3),
            ("examples/self_host/is_sorted.morph", "is_sorted", 5),
        ];
        let mut sort_criteria_failures: Vec<String> = Vec::new();
        for (config, bin) in [("off", &bin_off), ("on", &bin_on)] {
            for (file, spec_name, test_count) in &files {
                let (mut builds, mut wholes, mut testmods, mut full_pass) = (0, 0, 0, 0);
                for draw in 1..=5 {
                    let tag = format!("{}_{}_{}", config, spec_name, draw);
                    let d = measure_draw(&root, &draws_dir, bin, file, &tag);
                    if d.build_ok {
                        builds += 1;
                    }
                    if d.whole_compiles {
                        wholes += 1;
                    }
                    if d.testmod_compiles {
                        testmods += 1;
                    }
                    let passes_all = d.tests_passed == Some(*test_count) && d.tests_failed == Some(0);
                    if d.whole_compiles && passes_all {
                        full_pass += 1;
                    }
                    if *spec_name == "sort" && !(d.whole_compiles && passes_all) {
                        sort_criteria_failures.push(format!(
                            "config {} draw {}: whole compiles {}, tests {:?}/{:?} (codes {:?})",
                            config, draw, d.whole_compiles, d.tests_passed, d.tests_failed,
                            d.whole_codes
                        ));
                    }
                    writelnr(
                        &mut report,
                        &format!(
                            "- {} {} draw {}: build {}, whole compiles {} (parse ok {}, codes {:?}{}), function compiles {} {}, test module compiles {} (codes {:?}), tests passed {:?} failed {:?}",
                            config,
                            spec_name,
                            draw,
                            d.build_ok,
                            d.whole_compiles,
                            d.parse_ok,
                            d.whole_codes,
                            if d.whole_first.is_empty() {
                                String::new()
                            } else {
                                format!("; first: {}", d.whole_first)
                            },
                            d.function_compiles,
                            d.function_class,
                            d.testmod_compiles,
                            d.testmod_codes,
                            d.tests_passed,
                            d.tests_failed,
                        ),
                    );
                    assert!(
                        d.build_ok,
                        "config {} {} draw {}: the binary did not build end-to-end",
                        config, spec_name, draw
                    );
                    std::fs::write(&report_path, &report).unwrap();
                }
                writelnr(
                    &mut report,
                    &format!(
                        "**{} {} totals: builds {}/5, whole compiles {}/5, test-module compiles {}/5, whole+all-tests-pass {}/5**",
                        config, spec_name, builds, wholes, testmods, full_pass
                    ),
                );
                writelnr(&mut report, "");
            }
        }

        // ── Checkbox-3 verdict (computed, recorded) ──
        let verdict = sort_criteria_failures.is_empty();
        writelnr(&mut report, "## Checkbox-3 verdict (Phase 50 form)");
        if verdict {
            writelnr(
                &mut report,
                "CRITERIA HOLD: every drawn sort output in BOTH configurations compiles whole (incl. test module) and passes all 6 in-spec tests across the 5 measured draws per configuration.",
            );
        } else {
            writelnr(
                &mut report,
                "CRITERIA DO NOT HOLD on every draw; the failing draws:",
            );
            for f in &sort_criteria_failures {
                writelnr(&mut report, &format!("- {}", f));
            }
        }
        std::fs::write(&report_path, &report).unwrap();
        println!(
            "PHASE62-CHECKBOX3|verdict={}|failures={}",
            if verdict { "CRITERIA-HOLD" } else { "CRITERIA-MISS" },
            sort_criteria_failures.len()
        );
        println!("PHASE62|DONE|report at target/phase62/measurement-report.md");
    }
}
