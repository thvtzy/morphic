// ─────────────────────────────────────────────────────────────
//  PHASE 60 MEASUREMENT (test-only)
//
//  Campaign slice G3 (the forall-fold constraint-seed
//  family + the runner's trigger extension) measured on
//  count_matching, the family's one target, plus the
//  is_sorted control:
//
//  - Part A (firing inventory): every function of every
//    .morph file in examples/ is run through the engine's
//    constraint-seed census; the forall_fold firing set
//    must be EXACTLY {count_matching}. This is the
//    territory-disjointness proof over the whole 142-spec
//    corpus: no M2/M3/tokenizer spec and none of the
//    other standalone specs may gain the seed.
//  - Part B (draws): 5 draws of count_matching through
//    the real pipeline (synthesize → verify_all → runner
//    stage → select_best), recording per draw: the seed's
//    presence in the verified pool, its Z3 status and
//    pre-stage (interpreter) tpr, the runner outcome, the
//    seed's finalist verdict, and the crown's identity,
//    recorded tpr, and compiled verdict.
//  - Part C (control): 2 draws of is_sorted — its
//    adjacent-scan TEMPLATE sits behind the same
//    interpreter wall, but template provenance is
//    deliberately NOT a trigger (the recorded scope
//    limit), so the stage must stay NotTriggered and its
//    crowns must keep their Phase 59 character.
//
//  Asserted (deterministic): the Part A firing set; the
//  seed's presence in every verified pool; the runner
//  scoring on every draw with the seed judged 3/3; the
//  crown being the seed on every draw; the control's
//  NotTriggered outcomes.
//  Recorded, not asserted (draw-dependent): the capped
//  non-finalist counts and stage wall times.
//
//  Run with:
//    cargo test -- --ignored --nocapture phase60_measurement
//  Report: target/phase60/measurement-report.md
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod measurement {
    use crate::codegen;
    use crate::spec::ast::FunctionSpec;
    use crate::spec::{parser, typeck};
    use crate::synthesis::engine::{self, CandidateImplementation, Provenance, SynthesisConfig, SynthesisEngine};
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
            Provenance::ConstraintInduced { kind } => format!("ConstraintInduced({})", kind),
            Provenance::Mutation { mutation_kind, .. } => {
                format!("Mutation({})", mutation_kind)
            }
            Provenance::Crossover { .. } => "Crossover".into(),
            Provenance::LLMGenerated { .. } => "LLMGenerated".into(),
            Provenance::HumanGuided => "HumanGuided".into(),
        }
    }

    fn is_forall_fold(c: &CandidateImplementation) -> bool {
        matches!(&c.provenance, Provenance::ConstraintInduced { kind } if kind == "forall_fold")
    }

    // ── The Phase 53 compiled vehicle ─────────────────────

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

    fn function_part(generated: &str) -> String {
        match generated.lines().position(|l| l.trim() == "#[cfg(test)]") {
            Some(idx) => generated.lines().take(idx).collect::<Vec<_>>().join("\n") + "\n",
            None => generated.to_string(),
        }
    }

    /// (function compiles, tests (passed, total)) — tests is
    /// None when the runner binary did not compile or run.
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
        (function_compiles, tests)
    }

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

    const IS_SORTED_MAIN: &str = r#"
fn check(name: &str, got: std::thread::Result<bool>, want: bool, results: &mut Vec<bool>) {
    let ok = matches!(&got, Ok(v) if *v == want);
    println!("{}: {}", name, if ok { "PASS" } else { "FAIL" });
    results.push(ok);
}
fn main() {
    let mut r: Vec<bool> = Vec::new();
    check("empty", std::panic::catch_unwind(|| is_sorted(vec![])), true, &mut r);
    check("single", std::panic::catch_unwind(|| is_sorted(vec![5])), true, &mut r);
    check("sorted", std::panic::catch_unwind(|| is_sorted(vec![1, 2, 3])), true, &mut r);
    check("unsorted", std::panic::catch_unwind(|| is_sorted(vec![3, 1, 2])), false, &mut r);
    check("equal", std::panic::catch_unwind(|| is_sorted(vec![5, 5, 5])), true, &mut r);
    let passed = r.iter().filter(|b| **b).count();
    println!("TOTAL {}/5", passed);
}
"#;

    // ── Pipeline ──────────────────────────────────────────

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

    #[test]
    #[ignore = "Phase 60 measurement: forall-fold firing inventory over the whole corpus + 5 draws of count_matching through the full pipeline (seed presence, runner verdicts, crowns) + 2 is_sorted control draws; run explicitly with --ignored --nocapture phase60_measurement"]
    fn phase60_measurement() {
        let root = env!("CARGO_MANIFEST_DIR");
        let dir = format!("{}/target/phase60", root);
        std::fs::create_dir_all(&dir).unwrap();
        let mut report = String::from("# Phase 60 measurement — G3 forall-fold seed family + runner trigger extension\n");

        // ── Part A: firing inventory over the whole corpus ──
        let corpus_files = [
            "examples/sort.morph",
            "examples/binary_search.morph",
            "examples/functional_tokenizer.morph",
            "examples/self_host/codegen.morph",
            "examples/self_host/count_matching.morph",
            "examples/self_host/is_sorted.morph",
            "examples/self_host/printer.morph",
            "examples/self_host/validator.morph",
        ];
        let mut total_specs = 0usize;
        let mut firing: Vec<String> = Vec::new();
        let mut census_lines: Vec<String> = Vec::new();
        for rel in corpus_files {
            let functions = load_functions(rel);
            let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &functions);
            for func in &functions {
                total_specs += 1;
                let kinds = engine.constraint_seed_kinds(func);
                if kinds.iter().any(|k| k == "forall_fold") {
                    firing.push(func.name.clone());
                }
                census_lines.push(format!("{} :: {:?}", func.name, kinds));
            }
        }
        firing.sort();
        report.push_str(&format!(
            "\n## Part A — firing inventory\n\n{} specs across {} files; forall_fold firing set: {:?}\n",
            total_specs,
            corpus_files.len(),
            firing
        ));
        report.push_str("\nConstraint-seed census (spec :: kinds):\n");
        for line in &census_lines {
            report.push_str(&format!("- {}\n", line));
        }
        assert_eq!(
            firing,
            vec!["count_matching".to_string()],
            "forall_fold may fire ONLY for count_matching"
        );
        assert_eq!(total_specs, 142, "the corpus is 142 specs");

        // The seed body itself, for the record.
        let cm_functions = load_functions("examples/self_host/count_matching.morph");
        let cm_engine = SynthesisEngine::with_module(SynthesisConfig::default(), &cm_functions);
        let cm_spec = &cm_functions[0];
        let seed_kinds = cm_engine.constraint_seed_kinds(cm_spec);
        report.push_str(&format!(
            "\ncount_matching constraint seeds: {:?}\n",
            seed_kinds
        ));

        // ── Part B: count_matching draws ──
        report.push_str("\n## Part B — count_matching (5 draws, full pipeline)\n");
        let mut seed_in_pool = 0usize;
        let mut crown_is_seed = 0usize;
        for draw in 0..5 {
            let (func, pool) = verified_pool(&cm_functions, "count_matching");
            let seed = pool
                .iter()
                .find(|c| is_forall_fold(c))
                .unwrap_or_else(|| panic!("draw {}: forall_fold seed missing from verified pool", draw))
                .clone();
            seed_in_pool += 1;
            assert_eq!(
                seed.scores.test_pass_ratio, 0.0,
                "draw {}: the seed's pre-stage (interpreter) tpr must be 0.0 — the coverage fact the extension bridges",
                draw
            );
            let seed_key = body_key(&seed);
            let (rescored, outcome) = rescore_finalists_report(pool, &func);
            let (finalists, capped, elapsed) = match &outcome {
                RunnerOutcome::Scored { finalists, capped_non_finalists, elapsed } => {
                    (finalists.clone(), *capped_non_finalists, *elapsed)
                }
                other => panic!("draw {}: the runner stage must score, got {:?}", draw, other),
            };
            let seed_outcome = finalists
                .iter()
                .find(|f| f.candidate_id == seed.id)
                .unwrap_or_else(|| panic!("draw {}: the seed must be a finalist (guaranteed slot)", draw));
            assert_eq!(
                (seed_outcome.passed, seed_outcome.total),
                (3, 3),
                "draw {}: the compiled vehicle must judge the seed 3/3",
                draw
            );
            assert!(
                (seed_outcome.new_test_pass_ratio - 1.0).abs() < 1e-9,
                "draw {}: the seed's rescored tpr must be 1.000, got {}",
                draw,
                seed_outcome.new_test_pass_ratio
            );
            let crown = selector::select_best(rescored, &func)
                .unwrap_or_else(|e| panic!("draw {}: select_best: {}", draw, e));
            let is_seed = body_key(&crown) == seed_key;
            if is_seed {
                crown_is_seed += 1;
            }
            assert!(
                is_seed,
                "draw {}: the crown must be the forall_fold seed, got {} [{}]",
                draw,
                short_hash(&body_key(&crown)),
                provenance_name(&crown)
            );
            let (compiles, tests) = judge_compiled(
                &crown,
                COUNT_MAIN,
                &dir,
                &format!("cm_crown_{}", draw),
            );
            assert_eq!(
                tests,
                Some((3, 3)),
                "draw {}: the crown must pass 3/3 under the compiled vehicle",
                draw
            );
            report.push_str(&format!(
                "- draw {}: seed in pool (Z3 {:?}, pre-stage tpr {:.3}); runner Scored ({} finalists, {} capped, {:?}); seed finalist verdict {}/{} → tpr {:.3}; crown is the seed (recorded tpr {:.3}, compiles {}, compiled verdict {:?})\n",
                draw,
                seed.verification_status,
                seed.scores.test_pass_ratio,
                finalists.len(),
                capped,
                elapsed,
                seed_outcome.passed,
                seed_outcome.total,
                seed_outcome.new_test_pass_ratio,
                crown.scores.test_pass_ratio,
                compiles,
                tests,
            ));
        }
        report.push_str(&format!(
            "**count_matching: seed in pool {}/5 draws; crown is the seed {}/5**\n",
            seed_in_pool, crown_is_seed
        ));

        // ── Part C: is_sorted control ──
        let is_functions = load_functions("examples/self_host/is_sorted.morph");
        report.push_str("\n## Part C — is_sorted control (2 draws; template provenance is not a trigger)\n");
        for draw in 0..2 {
            let (func, pool) = verified_pool(&is_functions, "is_sorted");
            assert!(
                pool.iter().any(|c| matches!(c.provenance, Provenance::Template { .. })),
                "is_sorted draw {}: the adjacent-scan template must be in the pool (setup)",
                draw
            );
            let (rescored, outcome) = rescore_finalists_report(pool, &func);
            assert!(
                matches!(outcome, RunnerOutcome::NotTriggered),
                "is_sorted draw {}: the stage must not trigger on template provenance, got {:?}",
                draw,
                outcome
            );
            let crown = selector::select_best(rescored, &func).expect("select_best");
            let (_, tests) = judge_compiled(
                &crown,
                IS_SORTED_MAIN,
                &dir,
                &format!("is_crown_{}", draw),
            );
            report.push_str(&format!(
                "- draw {}: NotTriggered; crown {} [{}] recorded tpr {:.3}, compiled verdict {:?}\n",
                draw,
                short_hash(&body_key(&crown)),
                provenance_name(&crown),
                crown.scores.test_pass_ratio,
                tests,
            ));
        }

        std::fs::write(format!("{}/measurement-report.md", dir), &report).unwrap();
        println!("{}", report);
    }
}
