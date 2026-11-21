// ─────────────────────────────────────────────────────────────
//  PHASE 58 MEASUREMENT (test-only)
//
//  Campaign slice G1 (compiled-runner final scoring,
//  synthesis::runner) measured on the two specs in the
//  FALLBACK's residue, plus two controls:
//
//  - examples/sort.morph and examples/binary_search.morph —
//    per draw: the crown BEFORE the stage (select_best on the
//    verified pool, FALLBACK tpr) vs the crown AFTER
//    (select_best on the runner-rescored pool), each judged
//    by the Phase 53 compiled vehicle (winner function as a
//    lib + a hand-written runner main), the stage's own
//    finalist verdicts, and its wall time.
//  - examples/self_host/count_matching.morph and
//    examples/self_host/is_sorted.morph — controls: their
//    tests fully bind in the interpreter, so the stage must
//    be a byte-identical no-op (asserted: outcome
//    NotTriggered, every score unchanged).
//
//  Asserted: the stage triggers (Scored) for sort and
//  binary_search on every draw; the controls never trigger.
//  Recorded, not asserted (draw-dependent): crown identities,
//  pass counts, wall times.
//
//  Run with:
//    cargo test -- --ignored --nocapture phase58_measurement
//  Report: target/phase58/measurement-report.md
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod measurement {
    use crate::codegen;
    use crate::spec::ast::FunctionSpec;
    use crate::spec::{parser, typeck};
    use crate::synthesis::engine::{self, CandidateImplementation};
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

    fn short_hash(text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn body_key(c: &CandidateImplementation) -> String {
        format!("{:?}", c.body)
    }

    /// synthesize + verify_all (the pipeline up to the G1
    /// stage), returning the verified pool.
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

    /// Run a command with a timeout, output redirected to
    /// files under `dir` (the Phase 53 pattern). Returns
    /// (success, stdout) or None on timeout/spawn failure.
    fn run_timed(
        dir: &str,
        tag: &str,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Option<(bool, String)> {
        let out_path = format!("{}/{}.stdout", dir, tag);
        let err_path = format!("{}/{}.stderr", dir, tag);
        let out_file = std::fs::File::create(&out_path).ok()?;
        let err_file = std::fs::File::create(&err_path).ok()?;
        let mut child = Command::new(program)
            .args(args)
            .stdout(Stdio::from(out_file))
            .stderr(Stdio::from(err_file))
            .spawn()
            .ok()?;
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout = std::fs::read_to_string(&out_path).unwrap_or_default();
                    return Some((status.success(), stdout));
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

    /// The winner-function part of a generated file:
    /// everything before the generated #[cfg(test)] module.
    fn function_part(generated: &str) -> String {
        match generated.lines().position(|l| l.trim() == "#[cfg(test)]") {
            Some(idx) => generated.lines().take(idx).collect::<Vec<_>>().join("\n") + "\n",
            None => generated.to_string(),
        }
    }

    struct CompiledVerdict {
        function_compiles: bool,
        tests: Option<(usize, usize)>,
    }

    /// Judge one candidate by the compiled vehicle (Phase 53's
    /// judge, unchanged): function as a lib; function + the
    /// hand-written runner main as a binary.
    fn judge_compiled(
        candidate: &CandidateImplementation,
        runner_main: &str,
        dir: &str,
        tag: &str,
    ) -> CompiledVerdict {
        let generated = match codegen::generate(candidate, "rust") {
            Ok(text) => text,
            Err(_) => {
                return CompiledVerdict { function_compiles: false, tests: None };
            }
        };
        let func_src = function_part(&generated);
        let fn_rs = format!("{}/{}.rs", dir, tag);
        std::fs::write(&fn_rs, &func_src).unwrap();
        let func = run_timed(
            dir,
            &format!("{}_fncc", tag),
            "rustc",
            &["--edition", "2021", "--crate-type", "lib", &fn_rs, "-o", &format!("{}/{}.rlib", dir, tag)],
            Duration::from_secs(60),
        );
        let function_compiles = matches!(func, Some((true, _)));
        let run_rs = format!("{}/{}_run.rs", dir, tag);
        std::fs::write(&run_rs, format!("{}{}", func_src, runner_main)).unwrap();
        let run_bin = format!("{}/{}_run", dir, tag);
        let compiled = run_timed(
            dir,
            &format!("{}_runcc", tag),
            "rustc",
            &["--edition", "2021", &run_rs, "-o", &run_bin],
            Duration::from_secs(60),
        );
        let tests = match compiled {
            Some((true, _)) => {
                match run_timed(dir, &format!("{}_run", tag), &run_bin, &[], Duration::from_secs(10)) {
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
        CompiledVerdict { function_compiles, tests }
    }

    const SORT_RUNNER_MAIN: &str = r#"
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

    const BS_RUNNER_MAIN: &str = r#"
fn check(name: &str, got: std::thread::Result<Option<i64>>, want: Option<i64>, results: &mut Vec<bool>) {
    let ok = matches!(&got, Ok(v) if *v == want);
    println!("{}: {}", name, if ok { "PASS" } else { "FAIL" });
    results.push(ok);
}
fn main() {
    let sub = |a: i64, b: i64| a - b;
    let mut r: Vec<bool> = Vec::new();
    check("middle", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9, 11, 13], 7, sub)), Some(3), &mut r);
    check("start", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9], 1, sub)), Some(0), &mut r);
    check("end", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9], 9, sub)), Some(4), &mut r);
    check("too-small", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9], 0, sub)), None, &mut r);
    check("too-large", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9], 10, sub)), None, &mut r);
    check("between", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9], 4, sub)), None, &mut r);
    check("empty", std::panic::catch_unwind(|| binary_search::<i64>(vec![], 5, sub)), None, &mut r);
    let passed = r.iter().filter(|b| **b).count();
    println!("TOTAL {}/7", passed);
}
"#;

    fn measure_spec(
        report: &mut String,
        rel_path: &str,
        spec_name: &str,
        runner_main: &str,
        draws: usize,
        dir: &str,
    ) {
        let functions = load_functions(rel_path);
        report.push_str(&format!("## {} ({} draws)\n\n", spec_name, draws));
        for draw in 0..draws {
            let (spec, pool) = verified_pool(&functions, spec_name);
            let crown_before = selector::select_best(pool.clone(), &spec).unwrap();
            let before_verdict = judge_compiled(
                &crown_before,
                runner_main,
                dir,
                &format!("{}_before_{}", spec_name, draw),
            );
            let wall = Instant::now();
            let (rescored, outcome) = rescore_finalists_report(pool.clone(), &spec);
            let wall_elapsed = wall.elapsed();
            let (finalist_lines, capped, stage_elapsed) = match &outcome {
                RunnerOutcome::Scored {
                    finalists,
                    capped_non_finalists,
                    elapsed,
                } => {
                    let lines = finalists
                        .iter()
                        .map(|f| {
                            format!(
                                "id {} → {}/{} (tpr {:.3})",
                                f.candidate_id, f.passed, f.total, f.new_test_pass_ratio
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    (lines, *capped_non_finalists, *elapsed)
                }
                other => panic!("{} draw {}: expected Scored, got {:?}", spec_name, draw, other),
            };
            let crown_after = selector::select_best(rescored.clone(), &spec).unwrap();
            let after_verdict = judge_compiled(
                &crown_after,
                runner_main,
                dir,
                &format!("{}_after_{}", spec_name, draw),
            );
            let after_tpr = rescored
                .iter()
                .find(|c| c.id == crown_after.id)
                .map(|c| c.scores.test_pass_ratio)
                .unwrap_or(-1.0);
            let line = format!(
                "- draw {} — pool {}; stage {:?} (wall {:?}); finalists: {}; capped {}\n  crown BEFORE: {:?} [body {}] tpr {:.3} → compiles {} tests {:?}\n  crown AFTER:  {:?} [body {}] tpr {:.3} → compiles {} tests {:?}{}\n",
                draw,
                pool.len(),
                stage_elapsed,
                wall_elapsed,
                finalist_lines,
                capped,
                crown_before.provenance,
                short_hash(&body_key(&crown_before)),
                crown_before.scores.test_pass_ratio,
                before_verdict.function_compiles,
                before_verdict.tests,
                crown_after.provenance,
                short_hash(&body_key(&crown_after)),
                after_tpr,
                after_verdict.function_compiles,
                after_verdict.tests,
                if crown_before.id == crown_after.id { " (same crown)" } else { " (CROWN CHANGED)" },
            );
            print!("{}", line);
            report.push_str(&line);
        }
        report.push('\n');
    }

    /// Pool scan (binary_search): for every distinct pool
    /// body — hole count, composite, whether the winner
    /// function compiles as a lib, and its runner pass count
    /// when it does. Establishes where the compiling bodies
    /// sit relative to the finalist set and the completeness
    /// gate.
    fn scan_binary_search_pool(report: &mut String, draws: usize, dir: &str) {
        let functions = load_functions("examples/binary_search.morph");
        report.push_str(&format!(
            "## binary_search pool scan ({} draws, distinct bodies)\n\n",
            draws
        ));
        for draw in 0..draws {
            let (spec, pool) = verified_pool(&functions, "binary_search");
            let _ = &spec;
            let mut seen: Vec<String> = Vec::new();
            let mut rows: Vec<String> = Vec::new();
            let (mut hole_free_compiling, mut holed_compiling, mut hole_free_total) =
                (0usize, 0usize, 0usize);
            for (i, cand) in pool.iter().enumerate() {
                let key = body_key(cand);
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key);
                let holes = engine::count_holes(&cand.body);
                if holes == 0 {
                    hole_free_total += 1;
                }
                let verdict = judge_compiled(
                    cand,
                    BS_RUNNER_MAIN,
                    dir,
                    &format!("bs_scan_{}_{}", draw, i),
                );
                if verdict.function_compiles {
                    if holes == 0 {
                        hole_free_compiling += 1;
                    } else {
                        holed_compiling += 1;
                    }
                }
                rows.push(format!(
                    "  - id {} {:?} holes {} composite {:.3} compiles {} tests {:?}\n",
                    cand.id,
                    cand.provenance,
                    holes,
                    engine::composite_score(&cand.scores),
                    verdict.function_compiles,
                    verdict.tests,
                ));
            }
            let header = format!(
                "- draw {} — {} distinct bodies ({} hole-free); compiling: {} hole-free, {} holed\n",
                draw,
                seen.len(),
                hole_free_total,
                hole_free_compiling,
                holed_compiling,
            );
            print!("{}", header);
            report.push_str(&header);
            for row in rows {
                report.push_str(&row);
            }
        }
        report.push('\n');
    }

    fn measure_control(report: &mut String, rel_path: &str, spec_name: &str, draws: usize) {
        let functions = load_functions(rel_path);
        report.push_str(&format!("## control: {} ({} draws)\n\n", spec_name, draws));
        for draw in 0..draws {
            let (spec, pool) = verified_pool(&functions, spec_name);
            let before: Vec<(u64, f64, f64)> = pool
                .iter()
                .map(|c| (c.id, c.scores.test_pass_ratio, c.score))
                .collect();
            let wall = Instant::now();
            let (out, outcome) = rescore_finalists_report(pool, &spec);
            let wall_elapsed = wall.elapsed();
            assert!(
                matches!(outcome, RunnerOutcome::NotTriggered),
                "{} draw {}: control must not trigger, got {:?}",
                spec_name, draw, outcome
            );
            let after: Vec<(u64, f64, f64)> = out
                .iter()
                .map(|c| (c.id, c.scores.test_pass_ratio, c.score))
                .collect();
            assert_eq!(before, after, "{} draw {}: control scores must be byte-identical", spec_name, draw);
            let line = format!(
                "- draw {} — NotTriggered, {} candidates' scores byte-identical, stage wall {:?}\n",
                draw,
                out.len(),
                wall_elapsed,
            );
            print!("{}", line);
            report.push_str(&line);
        }
        report.push('\n');
    }

    #[test]
    #[ignore = "Phase 58 measurement: 5 draws each of sort + binary_search through the full pipeline with G1 rescoring, crowns judged by compiled runners, plus 2 control draws each; run explicitly with --ignored --nocapture phase58_measurement"]
    fn phase58_measurement() {
        let root = env!("CARGO_MANIFEST_DIR");
        let dir = format!("{}/target/phase58", root);
        std::fs::create_dir_all(&dir).unwrap();
        let mut report = String::new();
        report.push_str("# Phase 58 measurement — G1 compiled-runner final scoring\n\n");
        report.push_str(
            "Pipeline per draw: synthesize_with_module (opt 1) → verify_all → \
             [crown BEFORE] → runner::rescore_finalists → [crown AFTER]. Crowns \
             judged by the Phase 53 compiled vehicle. Controls assert the stage \
             is a byte-identical no-op for fully-bindable specs.\n\n",
        );
        measure_spec(&mut report, "examples/sort.morph", "sort", SORT_RUNNER_MAIN, 5, &dir);
        measure_spec(
            &mut report,
            "examples/binary_search.morph",
            "binary_search",
            BS_RUNNER_MAIN,
            5,
            &dir,
        );
        scan_binary_search_pool(&mut report, 2, &dir);
        measure_control(&mut report, "examples/self_host/count_matching.morph", "count_matching", 2);
        measure_control(&mut report, "examples/self_host/is_sorted.morph", "is_sorted", 2);
        std::fs::write(format!("{}/measurement-report.md", dir), &report).unwrap();
        print!("{}", report);
    }
}
