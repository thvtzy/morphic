// ─────────────────────────────────────────────────────────────
//  PHASE 59 MEASUREMENT (test-only)
//
//  Campaign slice G2 (algorithm-template seeding,
//  synthesis::templates wired into the engine's
//  seed_from_templates) measured on the specs the Phase 57
//  audit left behind, plus the count_matching control:
//
//  - Part A (keying inventory): every function of every
//    .morph file in examples/ is run through
//    templates::generate_templates; the firing set must be
//    EXACTLY {sort → sort_primitive, binary_search →
//    binary_search, is_sorted → adjacent_scan}. This is the
//    blast-radius proof for the structural keying: no
//    M2/M3/tokenizer corpus spec may gain a template seed.
//  - Part B (draws): 5 draws each of sort, binary_search,
//    is_sorted through the real pipeline (synthesize →
//    verify_all → G1 runner stage → select_best), recording
//    per draw: the template seed's presence in the verified
//    pool, the template body's OWN compiled verdict (the G1
//    runner on a singleton pool for the two unbindable
//    specs; the Phase 53 compiled vehicle for is_sorted),
//    and the crown's identity, recorded tpr, and compiled
//    verdict. 2 control draws of count_matching (no
//    template may fire for it — its shape is the
//    forall-fold constraint family's, not a template's).
//    Through Phase 59 the control also asserted the
//    runner never triggers for count_matching; Phase 60
//    (G3) re-records that line: the forall-fold seed is
//    interpreter-unevaluable, so the runner's trigger
//    extension now scores the pool and crowns the seed —
//    the control below asserts the new reality (and the
//    Phase 60 instrument carries the full measurement).
//
//  Phase 131 (G4) re-basing, executed: Phase 130's
//  function values retired the G1 stage's premise for
//  sort and binary_search (their tests bind; the stage
//  does not fire), so their template verdicts are now
//  measured through this file's compiled vehicle — the
//  route is_sorted always took — with the in-process
//  verdict recorded beside: sort's template (6, 6) on
//  both judges; binary_search's template still does not
//  compile (G5), and its standing judge is the
//  interpreter at (7, 7). The count_matching control is
//  re-based to the reality Phase 115 created: the
//  production forall-fold seed is a For scan, evaluable
//  since the For arm, so the stage does not fire for its
//  pool and the crown is the seed at an
//  interpreter-measured tpr — the control pins the
//  seed's in-process (3, 3), the stage's non-invocation,
//  and the crown's identity. Part A and is_sorted's
//  vehicle verdict stand unmoved.
//
//  Asserted (deterministic): the Part A firing set; the
//  template seed's presence in every verified pool; the
//  template bodies' own verdicts (sort (6, 6) in-process
//  and compiled — the E0631/E0599 walls of the early
//  record were removed at Phase 62; binary_search (7, 7)
//  in-process with its compiled form still walled by G5;
//  is_sorted 5/5 under the compiled vehicle — the body is
//  correct, its crown is blocked by interpreter judging,
//  recorded per draw); the count_matching control
//  invariants.
//  Recorded, not asserted (draw-dependent): crown
//  identities and pass counts.
//
//  Run with:
//    cargo test -- --ignored --nocapture phase59_measurement
//  Report: target/phase59/measurement-report.md
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod measurement {
    use crate::codegen;
    use crate::spec::ast::FunctionSpec;
    use crate::spec::{parser, typeck};
    use crate::synthesis::engine::{self, CandidateImplementation, Provenance};
    use crate::synthesis::eval;
    use crate::synthesis::runner::{rescore_finalists_report, RunnerOutcome};
    use crate::synthesis::selector;
    use crate::synthesis::templates::{generate_templates, TemplateKind};
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

    const BINARY_SEARCH_MAIN: &str = r#"
fn check(name: &str, got: std::thread::Result<Option<i64>>, want: Option<i64>, results: &mut Vec<bool>) {
    let ok = matches!(&got, Ok(v) if *v == want);
    println!("{}: {}", name, if ok { "PASS" } else { "FAIL" });
    results.push(ok);
}
fn main() {
    let cmp = |a: i64, b: i64| a - b;
    let mut r: Vec<bool> = Vec::new();
    check("middle", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9, 11, 13], 7, cmp)), Some(3), &mut r);
    check("start", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9], 1, cmp)), Some(0), &mut r);
    check("end", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9], 9, cmp)), Some(4), &mut r);
    check("small", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9], 0, cmp)), None, &mut r);
    check("large", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9], 10, cmp)), None, &mut r);
    check("between", std::panic::catch_unwind(|| binary_search::<i64>(vec![1, 3, 5, 7, 9], 4, cmp)), None, &mut r);
    check("empty", std::panic::catch_unwind(|| binary_search::<i64>(vec![], 5, cmp)), None, &mut r);
    let passed = r.iter().filter(|b| **b).count();
    println!("TOTAL {}/7", passed);
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

    fn template_candidate<'a>(
        pool: &'a [CandidateImplementation],
        kind_name: &str,
    ) -> Option<&'a CandidateImplementation> {
        pool.iter().find(|c| {
            matches!(&c.provenance, Provenance::Template { template_name } if template_name == kind_name)
        })
    }

    /// The template body's own verdict under the G1 runner
    /// was measured through this helper until Phase 131:
    /// it drove the stage on a singleton pool and expected
    /// Scored. Phase 130 retired that premise for sort and
    /// binary_search (their tests bind; the stage does not
    /// fire), and Phase 131's re-basing measures their
    /// compiled verdicts through judge_compiled instead —
    /// so the helper is removed rather than left as a
    /// panic with a stale premise. The stage's own
    /// end-to-end coverage lives in runner.rs's
    /// instruments.

    #[test]
    #[ignore = "Phase 59 measurement: keying inventory over the whole corpus + 5 draws each of sort/binary_search/is_sorted through the full pipeline (template presence, template compiled verdicts, crowns) + 2 count_matching control draws; run explicitly with --ignored --nocapture phase59_measurement"]
    fn phase59_measurement() {
        let root = env!("CARGO_MANIFEST_DIR");
        let dir = format!("{}/target/phase59", root);
        std::fs::create_dir_all(&dir).unwrap();
        let mut report = String::from("# Phase 59 measurement — G2 algorithm-template seeding\n");

        // ── Part A: keying inventory over the whole corpus ──
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
        let mut firing: Vec<(String, TemplateKind)> = Vec::new();
        for rel in corpus_files {
            for func in load_functions(rel) {
                total_specs += 1;
                for (kind, _) in generate_templates(&func) {
                    firing.push((func.name.clone(), kind));
                }
            }
        }
        firing.sort_by(|a, b| a.0.cmp(&b.0));
        report.push_str(&format!(
            "\n## Part A — keying inventory\n\n{} specs across {} files; firing set: {:?}\n",
            total_specs,
            corpus_files.len(),
            firing
        ));
        assert_eq!(
            firing,
            vec![
                ("binary_search".to_string(), TemplateKind::BinarySearch),
                ("is_sorted".to_string(), TemplateKind::AdjacentScan),
                ("sort".to_string(), TemplateKind::SortPrimitive),
            ],
            "templates may fire ONLY for the three structurally-keyed standalone specs"
        );

        // The rendered template functions (for the record).
        for (rel, name, kind_name) in [
            ("examples/sort.morph", "sort", "sort_primitive"),
            ("examples/binary_search.morph", "binary_search", "binary_search"),
            ("examples/self_host/is_sorted.morph", "is_sorted", "adjacent_scan"),
        ] {
            let functions = load_functions(rel);
            let func = functions.iter().find(|f| f.name == name).unwrap();
            let (_, body) = generate_templates(func).into_iter().next().unwrap();
            let probe = CandidateImplementation {
                id: 0,
                body,
                spec_name: func.name.clone(),
                spec: func.clone(),
                score: 0.0,
                scores: engine::ScoreBreakdown {
                    test_pass_ratio: 0.0,
                    constraint_score: 0.0,
                    complexity_score: 0.0,
                    quality_score: 0.0,
                    synthesis_time_us: 0,
                },
                generation: 0,
                provenance: Provenance::Template { template_name: kind_name.into() },
                verified: false,
                verification_status: None,
            };
            let generated = codegen::generate(&probe, "rust").expect("generate template body");
            report.push_str(&format!(
                "\n### template body for {} ({})\n\n```rust\n{}```\n",
                name,
                kind_name,
                function_part(&generated)
            ));
        }

        // ── Part B: draws ──
        let draw_specs: [(&str, &str, &str, &str, usize); 3] = [
            ("examples/sort.morph", "sort", "sort_primitive", SORT_MAIN, 6),
            ("examples/binary_search.morph", "binary_search", "binary_search", BINARY_SEARCH_MAIN, 7),
            ("examples/self_host/is_sorted.morph", "is_sorted", "adjacent_scan", IS_SORTED_MAIN, 5),
        ];
        for (rel, name, kind_name, main_src, total) in draw_specs {
            let functions = load_functions(rel);
            report.push_str(&format!("\n## Part B — {} (template: {})\n", name, kind_name));
            let mut template_in_pool = 0usize;
            let mut crown_is_template = 0usize;
            let mut crown_pass_sum = 0usize;
            for draw in 0..5 {
                let (func, pool) = verified_pool(&functions, name);
                let template = template_candidate(&pool, kind_name)
                    .unwrap_or_else(|| panic!("{} draw {}: template seed missing from verified pool", name, draw))
                    .clone();
                template_in_pool += 1;

                // The template body's own verdicts.
                // Re-based at Phase 131 (G4): the G1 stage
                // no longer fires for sort/binary_search —
                // their tests bind and judge in-process
                // since Phase 130 — so their compiled
                // verdicts are measured through this file's
                // compiled vehicle, the route is_sorted
                // always took, with the in-process verdict
                // recorded beside them. is_sorted keeps the
                // vehicle alone: its template's judging wall
                // is §3.4's, not G4's, and stands.
                let in_process = eval::run_spec_tests(&template.body, &func);
                let (compiles, tests) = judge_compiled(
                    &template,
                    main_src,
                    &dir,
                    &format!("{}_tmpl_{}", name, draw),
                );
                let template_verdict = tests.unwrap_or((0, total));
                if name == "is_sorted" {
                    assert_eq!(
                        template_verdict,
                        (5, 5),
                        "the adjacent-scan body must pass 5/5 under the compiled vehicle"
                    );
                } else if name == "sort" {
                    // Phase 62 re-record (2026-11-21) stands
                    // behind the compiled expectation: the
                    // owner's Clone-bound decision removed
                    // the E0599 wall and the runner measured
                    // the template (6, 6). At Phase 131 the
                    // measurement route changed, not the
                    // verdict: the interpreter judges the
                    // template 6/6 itself (the frozen G4
                    // standing), and the compiled vehicle
                    // agrees 6/6.
                    assert_eq!(
                        (in_process.evaluable, in_process.passed),
                        (6, 6),
                        "sort: the template judges 6/6 in-process since Phase 131"
                    );
                    assert_eq!(
                        template_verdict,
                        (6, total),
                        "sort: the compiled vehicle must agree at {}/{}",
                        total,
                        total
                    );
                } else {
                    // binary_search: the compiled side's
                    // wall stands — G5: the generated
                    // function indexes a Vec by i64 and does
                    // not compile, so the vehicle returns
                    // no verdict (template_verdict reads
                    // (0, 7), the recorded no-verdict shape).
                    // The standing judge is the interpreter,
                    // at the frozen 7/7 since Phase 131.
                    assert!(
                        !compiles,
                        "binary_search: the template's generated function must still fail compilation (G5)"
                    );
                    assert_eq!(
                        (in_process.evaluable, in_process.passed),
                        (7, 7),
                        "binary_search: the template judges 7/7 in-process since Phase 131"
                    );
                }

                // Crown after the full pipeline.
                let (rescored, _outcome) = rescore_finalists_report(pool, &func);
                let crown = selector::select_best(rescored, &func)
                    .unwrap_or_else(|e| panic!("{} draw {}: select_best: {}", name, draw, e));
                let is_template = body_key(&crown) == body_key(&template);
                if is_template {
                    crown_is_template += 1;
                }
                let (compiles, tests) = judge_compiled(
                    &crown,
                    main_src,
                    &dir,
                    &format!("{}_crown_{}", name, draw),
                );
                let passed = tests.map(|(p, _)| p).unwrap_or(0);
                crown_pass_sum += passed;
                report.push_str(&format!(
                    "- draw {}: template verdict {:?} (in-process {}/{}); crown {} [{}] recorded tpr {:.3}, compiles {}, compiled verdict {:?}\n",
                    draw,
                    template_verdict,
                    in_process.passed,
                    in_process.evaluable,
                    short_hash(&body_key(&crown)),
                    provenance_name(&crown),
                    crown.scores.test_pass_ratio,
                    compiles,
                    tests,
                ));
            }
            report.push_str(&format!(
                "**{}: template in pool {}/5 draws; crown is the template {}/5; crown compiled passes total {}/{}**\n",
                name,
                template_in_pool,
                crown_is_template,
                crown_pass_sum,
                5 * total,
            ));
        }

        // ── Control: count_matching ──
        // G2's invariant stands (no template seed — the
        // shape is the forall-fold family's). The runner
        // line was Phase 60's re-record (the stage scores
        // via the unevaluable constraint seed and crowns
        // it). Phase 131 re-bases that line again: the
        // production forall-fold seed is a For scan, and
        // the interpreter's For arm (Phase 115) made it
        // evaluable — it judges (3, 3) in-process — so the
        // stage's ConstructedSeeds trigger no longer fires
        // for this pool, and the crown is the seed at an
        // interpreter-measured tpr 1.000. (The runner
        // fixture's own scan body keeps a genuinely
        // unevaluable ingredient — Head/Tail since Phase
        // 131 — so the trigger itself stays exercised in
        // runner.rs's instruments.)
        let cm_functions = load_functions("examples/self_host/count_matching.morph");
        report.push_str("\n## Control — count_matching (no template in G2; forall-fold crown, interpreter-judged since Phase 115)\n");
        for draw in 0..2 {
            let (func, pool) = verified_pool(&cm_functions, "count_matching");
            assert!(
                !pool.iter().any(|c| matches!(c.provenance, Provenance::Template { .. })),
                "count_matching draw {}: no template seed may exist in G2",
                draw
            );
            assert!(
                pool.iter().any(|c| matches!(
                    &c.provenance,
                    Provenance::ConstraintInduced { kind } if kind == "forall_fold"
                )),
                "count_matching draw {}: the forall-fold seed must be in the pool (Phase 60)",
                draw
            );
            let seed = pool
                .iter()
                .find(|c| matches!(
                    &c.provenance,
                    Provenance::ConstraintInduced { kind } if kind == "forall_fold"
                ))
                .unwrap()
                .clone();
            let seed_key = body_key(&seed);
            // The seed judges in-process (Phase 115's For
            // arm) — the fact that retired the stage's
            // trigger for this pool.
            let seed_run = eval::run_spec_tests(&seed.body, &func);
            assert_eq!(
                (seed_run.evaluable, seed_run.passed),
                (3, 3),
                "count_matching draw {}: the forall-fold seed judges 3/3 in-process",
                draw
            );
            let (rescored, outcome) = rescore_finalists_report(pool, &func);
            assert!(
                matches!(outcome, RunnerOutcome::NotTriggered),
                "count_matching draw {}: the runner stage must not fire — the seed judges in-process since Phase 115 (Phase 60's Scored expectation recorded the pre-For-arm world), got {:?}",
                draw,
                outcome
            );
            let crown = selector::select_best(rescored, &func).expect("select_best");
            assert!(
                body_key(&crown) == seed_key,
                "count_matching draw {}: the crown must be the forall-fold seed's body (Phase 60's crown, interpreter-judged since Phase 115), got {} [{}]",
                draw,
                short_hash(&body_key(&crown)),
                provenance_name(&crown)
            );
            let (_, tests) = judge_compiled(&crown, COUNT_MAIN, &dir, &format!("cm_crown_{}", draw));
            report.push_str(&format!(
                "- draw {}: crown {} [{}] recorded tpr {:.3}, compiled verdict {:?}\n",
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
