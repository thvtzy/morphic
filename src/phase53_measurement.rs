// ─────────────────────────────────────────────────────────────
//  PHASE 53 MEASUREMENT (test-only)
//
//  The consequence of the tuple-binding fix (bind_test_env in
//  synthesis/eval.rs: tuple test inputs bind elementwise),
//  measured on the two repo specs whose in-spec tests are
//  tuple-form, plus binary_search's sibling shape:
//
//  - examples/self_host/count_matching.morph — tuple of
//    first-order values ([...], 5): its tests BIND after the
//    fix, so candidate test_pass_ratio is measured by real
//    execution, not scored by evaluate_candidate's FALLBACK.
//  - examples/sort.morph — tuple carrying a lambda for the
//    fn-typed cmp parameter: at Phase 53 its tests still
//    could not bind (the evaluator had no function
//    values), so sort's crowns were judged by the compiled
//    vehicle below — the Phase 52 pattern (winner function
//    + a hand-written runner main, rustc, execute) — NOT
//    by the generated test module (blocker 2: it renders
//    the tuple input as __tuple__(...) and the cmp
//    argument as ??, which does not parse).
//
//  Asserted (invariants of the fix itself): every
//  count_matching winner's interpreter run reports
//  evaluable == 3; at Phase 53, every sort winner's
//  reported evaluable == 0 (the FALLBACK's then-legitimate
//  domain).
//  Recorded, not asserted (draw-dependent): crown pass
//  counts, pool tpr distributions, pool scans.
//
//  RE-BASED at Phase 131 (G4): Phase 130 gave the
//  evaluator function values, so sort's tests bind (the
//  invariant below is bindable == 6 now, not evaluable
//  == 0) and its winners are judged in-process; Phase
//  131's higher-order judging completed the template's
//  standing. The compiled vehicle is kept as the
//  cross-check it now is — the two judges agreeing on a
//  crown is the project's cross-validation standard, and
//  this instrument is where the agreement is sampled.
//  The count_matching invariant (evaluable == 3) stands
//  unmoved.
//
//  Run with:
//    cargo test -- --ignored --nocapture phase53_measurement
//  Report: target/phase53/measurement-report.md
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod measurement {
    use crate::codegen;
    use crate::spec::ast::FunctionSpec;
    use crate::spec::{parser, typeck};
    use crate::synthesis::engine::{self, CandidateImplementation};
    use crate::synthesis::{eval, selector};
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

    /// The real build pipeline for one spec, exactly as
    /// src/main.rs runs it (module-aware synthesis at opt 1 →
    /// verify_all → select_best), returning the verified pool
    /// alongside the crowned winner.
    fn run_pipeline(
        functions: &[FunctionSpec],
        spec_name: &str,
    ) -> (Vec<CandidateImplementation>, CandidateImplementation) {
        let func = functions
            .iter()
            .find(|f| f.name == spec_name)
            .unwrap_or_else(|| panic!("no spec named {}", spec_name));
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine::synthesize_with_module(func, 1, functions)
        }));
        let candidates = match outcome {
            Ok(Ok(c)) if !c.is_empty() => c,
            Ok(Ok(_)) => panic!("{}: synthesis returned no candidates", spec_name),
            Ok(Err(e)) => panic!("{}: synthesis error: {}", spec_name, e),
            Err(_) => panic!("{}: synthesis panicked", spec_name),
        };
        let verified = verify::verifier::Verifier::with_specs(functions)
            .verify_all(candidates, func)
            .unwrap_or_else(|e| panic!("{}: verify_all error: {}", spec_name, e));
        let winner = selector::select_best(verified.clone(), func)
            .unwrap_or_else(|e| panic!("{}: select_best error: {}", spec_name, e));
        (verified, winner)
    }

    /// Run a command with a timeout, output redirected to
    /// files under `dir` (the m5_s2 pattern). Returns
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
    /// everything before the generated #[cfg(test)] module
    /// (the m5_s2 helper, same rule).
    fn function_part(generated: &str) -> String {
        match generated.lines().position(|l| l.trim() == "#[cfg(test)]") {
            Some(idx) => generated.lines().take(idx).collect::<Vec<_>>().join("\n") + "\n",
            None => generated.to_string(),
        }
    }

    struct CompiledVerdict {
        function_compiles: bool,
        /// (passed, total) from the runner's TOTAL line;
        /// None if the runner did not compile or did not run
        /// to completion.
        tests: Option<(usize, usize)>,
    }

    /// Judge one candidate by the compiled vehicle: render it
    /// with the original codegen, keep the winner function,
    /// append the hand-written runner main, compile with
    /// rustc, execute, parse the per-test TOTAL.
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
        // The function alone, as a lib.
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
        // Function + runner main, as a binary.
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

    /// sort's six in-spec tests (examples/sort.morph) as a
    /// reporting runner: per-test PASS/FAIL (panics caught per
    /// test) and a TOTAL line. The generated test module
    /// cannot be the vehicle (blocker 2).
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

    /// count_matching's three in-spec tests
    /// (examples/self_host/count_matching.morph), same
    /// reporting-runner shape.
    const COUNT_RUNNER_MAIN: &str = r#"
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

    #[test]
    #[ignore = "Phase 53 measurement: 12 sort draws + 6 count_matching draws through the real pipeline, winners judged by compiled runners; run explicitly with --ignored --nocapture phase53_measurement"]
    fn phase53_measurement() {
        let root = env!("CARGO_MANIFEST_DIR");
        let dir = format!("{}/target/phase53", root);
        std::fs::create_dir_all(&dir).unwrap();
        let mut report = String::new();
        report.push_str("# Phase 53 measurement — the tuple-binding fix's consequence\n\n");
        report.push_str(
            "Vehicles: the IR interpreter (eval::run_spec_tests) where tests bind; \
             compiled runners (winner function + hand-written runner main, rustc, \
             execute) as the behavioral judge. Crown outcomes are recorded, not \
             asserted; the fix's invariants (count_matching evaluable == 3, sort \
             evaluable == 0) are asserted.\n\n",
        );

        // ── count_matching: the spec the fix unlocks ──────
        let cm_functions = load_functions("examples/self_host/count_matching.morph");
        let cm_spec = cm_functions
            .iter()
            .find(|f| f.name == "count_matching")
            .unwrap()
            .clone();
        report.push_str("## count_matching (tuple of first-order values — tests bind after the fix)\n\n");
        let mut cm_crowns_passing = 0usize;
        for draw in 0..6 {
            let (pool, winner) = run_pipeline(&cm_functions, "count_matching");
            let run = eval::run_spec_tests(&winner.body, &cm_spec);
            assert_eq!(
                run.evaluable, 3,
                "count_matching's tuple tests must bind after the fix (draw {})",
                draw
            );
            let mut tprs: Vec<f64> = pool.iter().map(|c| c.scores.test_pass_ratio).collect();
            tprs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let distinct_tpr = {
                let mut v: Vec<f64> = tprs.clone();
                v.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
                v.len()
            };
            let verdict = judge_compiled(
                &winner,
                COUNT_RUNNER_MAIN,
                &dir,
                &format!("cm_{}", draw),
            );
            let compiled_pass = verdict.tests.map(|(p, _)| p);
            if compiled_pass == Some(3) {
                cm_crowns_passing += 1;
            }
            let line = format!(
                "- draw {} — pool {} candidates, pool tpr min {:.3} / max {:.3} ({} distinct values); winner {:?} [body {}] tpr {:.3} verified={:?}; interpreter run: evaluable {} passed {}; compiled: function_compiles={} tests={:?}\n",
                draw,
                pool.len(),
                tprs.first().copied().unwrap_or(0.0),
                tprs.last().copied().unwrap_or(0.0),
                distinct_tpr,
                winner.provenance,
                short_hash(&body_key(&winner)),
                winner.scores.test_pass_ratio,
                winner.verification_status,
                run.evaluable,
                run.passed,
                verdict.function_compiles,
                verdict.tests,
            );
            print!("{}", line);
            report.push_str(&line);

            // Pool scan on the first two draws: does any
            // distinct pool body pass all 3 tests under the
            // COMPILED vehicle, and what tpr did the
            // interpreter give it? (Discriminates "correct
            // bodies exist but are interpreter-invisible"
            // from "no correct body is generated".)
            if draw < 2 {
                let mut seen: Vec<String> = Vec::new();
                let mut distinct = 0usize;
                let mut compiled = 0usize;
                let mut passing = 0usize;
                let mut passing_tprs: Vec<f64> = Vec::new();
                for (i, cand) in pool.iter().enumerate() {
                    let key = body_key(cand);
                    if seen.contains(&key) {
                        continue;
                    }
                    seen.push(key);
                    distinct += 1;
                    let v = judge_compiled(
                        cand,
                        COUNT_RUNNER_MAIN,
                        &dir,
                        &format!("cm_pool_{}_{}", draw, i),
                    );
                    if v.function_compiles {
                        compiled += 1;
                    }
                    if v.tests == Some((3, 3)) {
                        passing += 1;
                        passing_tprs.push(cand.scores.test_pass_ratio);
                    }
                }
                let line = format!(
                    "  pool scan draw {}: {} distinct bodies, {} function-compiles, {} pass 3/3 (their interpreter tprs: {:?})\n",
                    draw, distinct, compiled, passing, passing_tprs
                );
                print!("{}", line);
                report.push_str(&line);
            }
        }
        let line = format!(
            "\ncount_matching crowns passing all 3 in-spec tests (compiled vehicle): {}/6\n\n",
            cm_crowns_passing
        );
        print!("{}", line);
        report.push_str(&line);

        // ── sort: the spec whose tests bound at Phase 130 ──
        let sort_functions = load_functions("examples/sort.morph");
        let sort_spec = sort_functions
            .iter()
            .find(|f| f.name == "sort")
            .unwrap()
            .clone();
        report.push_str("## sort (tuple carrying a lambda — tests bind since Phase 130; the interpreter judges, the compiled vehicle cross-checks)\n\n");
        let mut sort_crowns_passing = 0usize;
        let mut sort_crowns_compiling = 0usize;
        for draw in 0..12 {
            let (pool, winner) = run_pipeline(&sort_functions, "sort");
            let run = eval::run_spec_tests(&winner.body, &sort_spec);
            // Re-based at Phase 131: the pre-130 invariant
            // (every winner's run reports evaluable == 0)
            // died with the function-value wall. The
            // invariant of the current world is the binding
            // itself — all six tests bind (the tuple-fix's
            // post-130 form) — and each winner's in-process
            // run is recorded in the draw line below, the
            // compiled vehicle kept as the cross-check it
            // now is rather than the only judge.
            assert_eq!(
                eval::bindable_test_count(&sort_spec),
                6,
                "sort's lambda-carrying tuple tests bind since Phase 130 (draw {})",
                draw
            );
            let verdict = judge_compiled(&winner, SORT_RUNNER_MAIN, &dir, &format!("sort_{}", draw));
            if verdict.function_compiles {
                sort_crowns_compiling += 1;
            }
            if verdict.tests == Some((6, 6)) {
                sort_crowns_passing += 1;
            }
            let line = format!(
                "- draw {} — pool {} candidates; winner {:?} [body {}] tpr {:.3} (interpreter run: evaluable {} passed {}); compiled: function_compiles={} tests={:?}\n",
                draw,
                pool.len(),
                winner.provenance,
                short_hash(&body_key(&winner)),
                winner.scores.test_pass_ratio,
                run.evaluable,
                run.passed,
                verdict.function_compiles,
                verdict.tests,
            );
            print!("{}", line);
            report.push_str(&line);

            // Pool scan on the first three draws: does ANY
            // distinct pool body pass all six tests under the
            // compiled vehicle? (The generation-gap question
            // behind the Phase 53 contingency.)
            if draw < 3 {
                let mut seen: Vec<String> = Vec::new();
                let mut distinct = 0usize;
                let mut compiled = 0usize;
                let mut passing = 0usize;
                for (i, cand) in pool.iter().enumerate() {
                    let key = body_key(cand);
                    if seen.contains(&key) {
                        continue;
                    }
                    seen.push(key);
                    distinct += 1;
                    let v = judge_compiled(
                        cand,
                        SORT_RUNNER_MAIN,
                        &dir,
                        &format!("sort_pool_{}_{}", draw, i),
                    );
                    if v.function_compiles {
                        compiled += 1;
                    }
                    if v.tests == Some((6, 6)) {
                        passing += 1;
                    }
                }
                let line = format!(
                    "  pool scan draw {}: {} distinct bodies, {} function-compiles, {} pass 6/6\n",
                    draw, distinct, compiled, passing
                );
                print!("{}", line);
                report.push_str(&line);
            }
        }
        let line = format!(
            "\nsort crowns: function compiles {}/12; passing all 6 in-spec tests (compiled vehicle): {}/12\n",
            sort_crowns_compiling, sort_crowns_passing
        );
        print!("{}", line);
        report.push_str(&line);

        std::fs::write(format!("{}/measurement-report.md", dir), &report).unwrap();
    }
}
