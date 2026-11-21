// ─────────────────────────────────────────────────────────────
//  PHASE 61 MEASUREMENT (test-only)
//
//  Campaign slice G6 + the sort scan template: close
//  sort's leg of the M5 roundtrip (checkbox 3).
//
//  - Part A (construction probe): the candidate scan-form
//    sort constructions, built as IR against sort's real
//    spec and rendered through the PRODUCTION codegen,
//    judged by rustc on the generated function text:
//      A  bubble sort, plain index reads (cmp on Index
//         places; swap via a Let temp read);
//      B  bubble sort, clone-mediated reads (every element
//         read through UnaryOp::Clone) — the construction
//         the template library now builds;
//      C  the insertion family: the extraction step every
//         insertion variant shares (a Let-bound index read
//         pushed into an Alloc'd output list);
//      D  the Phase 59 construction: the IR Sort node.
//    Plus the priced unlock: B's (and A's) generated text
//    with the signature patched, PROBE-ONLY, to
//    `sort<T: Clone>` — what the recorded owner-class
//    signature decision would buy, measured, not shipped.
//    PHASE 62 RE-RECORD (2026-11-21): the owner TOOK the
//    priced unlock (decision (i)) — generated signatures
//    now carry the Clone bound natively, so this probe's
//    world changed underneath it: construction B compiles
//    AS SHIPPED (its B' prediction, now the standing
//    assertion), the signature patch is a no-op, and the
//    Part B template verdict is (6, 6), not the recorded
//    (0, 6) wall. The Phase 61 measurements stand in the
//    ROAD note; the assertions below state the new reality.
//  - Part B (draws): 5 draws of sort through the full
//    pipeline (synthesize → verify_all → G1 runner stage →
//    select_best), recording the template seed's presence,
//    its composite rank and finalist status in the stage,
//    its own runner verdict (singleton pool), and the
//    crown's identity / recorded tpr / compiled verdict.
//    2 control draws of count_matching assert the Phase 60
//    outcome stands (forall-fold crown, tpr 1.000, 3/3).
//  - Part C (shipped-file leg): for each sort draw, the
//    crown's WHOLE generated file (function + test module)
//    compiled as a test binary and executed — the G6
//    annotation's claim on the shipped artifact.
//
//  Run with:
//    cargo test -- --ignored --nocapture phase61_measurement
//  Report: target/phase61/measurement-report.md
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod measurement {
    use crate::codegen;
    use crate::spec::ast::{FunctionSpec, TypeRef};
    use crate::spec::{parser, typeck};
    use crate::synthesis::engine::{
        self, CandidateImplementation, CollectionKind, IRBinOp, IRLiteral, IRNode, IRUnaryOp,
        Provenance,
    };
    use crate::synthesis::runner::{rescore_finalists_report, RunnerOutcome};
    use crate::synthesis::selector::{self, TypeVerdict};
    use crate::synthesis::templates::generate_templates;
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

    fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
        CandidateImplementation {
            id: 0,
            body,
            spec_name: spec.name.clone(),
            spec: spec.clone(),
            score: 0.0,
            scores: engine::ScoreBreakdown {
                test_pass_ratio: 0.0,
                constraint_score: 0.0,
                complexity_score: 0.0,
                quality_score: 0.0,
                synthesis_time_us: 0,
            },
            generation: 0,
            provenance: Provenance::Template {
                template_name: "probe".into(),
            },
            verified: false,
            verification_status: None,
        }
    }

    // ── The Phase 53 compiled vehicle (as in Phase 59) ────

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

    /// Compile a source file as a library; returns
    /// (success, sorted unique error codes from stderr).
    fn rustc_lib(dir: &str, tag: &str, source: &str) -> (bool, Vec<String>) {
        let rs = format!("{}/{}.rs", dir, tag);
        std::fs::write(&rs, source).unwrap();
        let out = run_timed(
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
        let err_text = std::fs::read_to_string(format!("{}/{}_cc.stderr", dir, tag))
            .unwrap_or_default();
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
    /// Phase 53 vehicle, as in the Phase 59 instrument.
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
        let compiled = run_timed(
            dir,
            &format!("{}_runcc", tag),
            "rustc",
            &["--edition", "2021", &run_rs, "-o", &run_bin],
            Duration::from_secs(60),
        );
        let tests = match compiled {
            Some((true, _)) => {
                match run_timed(dir, &format!("{}_run", tag), &run_bin, &[], Duration::from_secs(10))
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

    // ── Construction builders (Part A) ────────────────────

    fn var(n: &str) -> IRNode {
        IRNode::Var(n.to_string())
    }
    fn int(n: i64) -> IRNode {
        IRNode::Literal(IRLiteral::Int(n))
    }
    fn idx(list: &str, i: IRNode) -> IRNode {
        IRNode::Index(Box::new(var(list)), Box::new(i))
    }
    fn len(list: &str) -> IRNode {
        IRNode::UnaryOp(IRUnaryOp::Len, Box::new(var(list)))
    }

    /// The bubble-sort scan form, parameterized by how an
    /// element is READ from the working list: `plain` =
    /// the Index place itself (construction A), `clone` =
    /// UnaryOp::Clone over the Index place (construction B).
    fn bubble(spec: &FunctionSpec, clone_reads: bool) -> IRNode {
        let read = |list: &str, i: IRNode| -> IRNode {
            let place = idx(list, i);
            if clone_reads {
                IRNode::UnaryOp(IRUnaryOp::Clone, Box::new(place))
            } else {
                place
            }
        };
        let jm1 = || IRNode::BinOp(IRBinOp::Sub, Box::new(var("j")), Box::new(int(1)));
        IRNode::Block(vec![
            IRNode::Alloc {
                name: "a".into(),
                ty: spec.return_type.clone(),
                initial: Box::new(var("list")),
            },
            IRNode::For {
                var: "i".into(),
                start: Box::new(int(0)),
                end: Box::new(len("a")),
                body: Box::new(IRNode::For {
                    var: "j".into(),
                    start: Box::new(int(1)),
                    end: Box::new(len("a")),
                    body: Box::new(IRNode::If {
                        condition: Box::new(IRNode::Call {
                            function: "cmp".into(),
                            args: vec![read("a", var("j")), read("a", jm1())],
                        }),
                        then_branch: Box::new(IRNode::Let {
                            name: "tmp".into(),
                            value: Box::new(read("a", jm1())),
                            body: Box::new(IRNode::Block(vec![
                                IRNode::Assign {
                                    target: Box::new(idx("a", jm1())),
                                    value: Box::new(read("a", var("j"))),
                                },
                                IRNode::Assign {
                                    target: Box::new(idx("a", var("j"))),
                                    value: Box::new(var("tmp")),
                                },
                            ])),
                        }),
                        else_branch: Box::new(IRNode::Block(vec![])),
                    }),
                }),
            },
            IRNode::Return(Box::new(var("a"))),
        ])
    }

    /// Construction C — the insertion family: the
    /// extraction step every insertion variant shares
    /// (a Let-bound index read, pushed into an Alloc'd
    /// output list via the Append BinOp).
    fn insertion_extract(spec: &FunctionSpec) -> IRNode {
        let _ = spec;
        IRNode::Block(vec![
            IRNode::Alloc {
                name: "out".into(),
                ty: spec.return_type.clone(),
                initial: Box::new(IRNode::Collection {
                    kind: CollectionKind::List,
                    elements: vec![],
                }),
            },
            IRNode::For {
                var: "i".into(),
                start: Box::new(int(0)),
                end: Box::new(len("list")),
                body: Box::new(IRNode::Let {
                    name: "x".into(),
                    value: Box::new(idx("list", var("i"))),
                    body: Box::new(IRNode::BinOp(
                        IRBinOp::Append,
                        Box::new(var("out")),
                        Box::new(var("x")),
                    )),
                }),
            },
            IRNode::Return(Box::new(var("out"))),
        ])
    }

    /// Construction D — the Phase 59 form: the IR Sort node
    /// driven by the comparator parameter.
    fn sort_node() -> IRNode {
        IRNode::Sort {
            comparator: Box::new(var("cmp")),
            collection: Box::new(var("list")),
        }
    }

    // ── Pipeline (as in Phase 59/60) ──────────────────────

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

    /// Compile the WHOLE generated file (function + test
    /// module) as a test binary and run it: (whole file
    /// compiles, tests passed, tests failed, error codes).
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
        let compiled = run_timed(
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
            match run_timed(dir, &format!("{}_whole", tag), &bin, &[], Duration::from_secs(10)) {
                Some((_, stdout)) => stdout.lines().find_map(|line| {
                    // "test result: ok. 3 passed; 3 failed; ..."
                    let rest = line.strip_prefix("test result:")?;
                    let mut passed = None;
                    let mut failed = None;
                    for part in rest.split(';') {
                        let part = part.trim();
                        if let Some(n) = part.strip_suffix(" passed") {
                            passed = n.split_whitespace().last().and_then(|t| t.parse::<usize>().ok());
                        }
                        if let Some(n) = part.strip_suffix(" failed") {
                            failed = n.split_whitespace().last().and_then(|t| t.parse::<usize>().ok());
                        }
                    }
                    Some((passed?, failed?))
                }),
                None => None,
            }
        } else {
            None
        };
        (compiles, tests.map(|(p, f)| (p, p + f)), codes)
    }

    #[test]
    #[ignore = "Phase 61 measurement: construction probe through production codegen + 5 draws of sort through the full pipeline + whole-file shipped-leg measurement + 2 count_matching control draws; run explicitly with --ignored --nocapture phase61_measurement"]
    fn phase61_measurement() {
        let root = env!("CARGO_MANIFEST_DIR");
        let dir = format!("{}/target/phase61", root);
        std::fs::create_dir_all(&dir).unwrap();
        let mut report = String::from("# Phase 61 measurement — G6 + the sort scan template\n");

        let sort_functions = load_functions("examples/sort.morph");
        let sort_spec = sort_functions.iter().find(|f| f.name == "sort").unwrap().clone();

        // ── Part A: construction probe ──
        report.push_str("\n## Part A — construction probe (production codegen + rustc)\n");
        let constructions: Vec<(&str, IRNode)> = vec![
            ("A bubble/plain-reads", bubble(&sort_spec, false)),
            ("B bubble/clone-reads", bubble(&sort_spec, true)),
            ("C insertion/extraction", insertion_extract(&sort_spec)),
            ("D sort-node (Phase 59)", sort_node()),
        ];
        let mut b_fn_text = String::new();
        for (name, body) in &constructions {
            let cand = candidate(&sort_spec, body.clone());
            let verdict = selector::return_type_verdict(body, &sort_spec);
            let holes = engine::count_holes(body);
            let generated = codegen::generate(&cand, "rust").expect("generate construction");
            let fn_text = function_part(&generated);
            let (compiles, codes) = rustc_lib(&dir, &format!("probe_{}", name.chars().next().unwrap()), &fn_text);
            report.push_str(&format!(
                "- {}: selector verdict {:?}, holes {}, function compiles {}, error codes {:?}\n",
                name, verdict, holes, compiles, codes
            ));
            if name.starts_with('B') {
                b_fn_text = fn_text.clone();
                report.push_str(&format!(
                    "\n### construction B, rendered\n\n```rust\n{}```\n",
                    fn_text
                ));
            }
            // The recorded walls, asserted so the instrument
            // stays a standing record of them. Phase 62
            // re-record: B's wall (E0599 under the bound-free
            // signature) is GONE — the owner adopted the
            // Clone bound, so B compiles as shipped; A and C
            // still die E0507 (Clone does not imply Copy) and
            // D still dies E0631 (the bound does not touch
            // the Sort node's comparator shape).
            match name.chars().next().unwrap() {
                'A' => assert!(codes.contains(&"E0507".to_string()), "A must die E0507: {:?}", codes),
                'B' => assert!(compiles, "B must compile as shipped since Phase 62 (the Clone bound is native): {:?}", codes),
                'C' => assert!(codes.contains(&"E0507".to_string()), "C must die E0507: {:?}", codes),
                'D' => assert!(codes.contains(&"E0631".to_string()), "D must die E0631: {:?}", codes),
                _ => unreachable!(),
            }
            assert_eq!(holes, 0, "{} must be hole-free", name);
            assert_ne!(verdict, TypeVerdict::Mismatched, "{} must not be selector-Mismatched", name);
        }

        // The priced unlock — Phase 62 re-record: ADOPTED
        // (owner decision (i)). The generated signature
        // carries the bound natively, so the probe-only
        // patch is now the identity on B's text; what Phase
        // 61 measured as the hypothetical B' (compiles +
        // 6/6) is asserted here of the SHIPPED text.
        assert!(
            b_fn_text.contains("pub fn sort<T: Clone>("),
            "the shipped signature must carry the Clone bound natively: {}",
            b_fn_text.lines().find(|l| l.contains("pub fn sort")).unwrap_or("")
        );
        let patched_b = b_fn_text.clone();
        let (b_prime_compiles, b_prime_codes) = rustc_lib(&dir, "probe_B_prime", &patched_b);
        report.push_str(&format!(
            "- B' (B as shipped — the bound Phase 61 priced as hypothetical is native since Phase 62): compiles {}, error codes {:?}\n",
            b_prime_compiles, b_prime_codes
        ));
        assert!(b_prime_compiles, "B must compile under the (now native) bound: {:?}", b_prime_codes);
        let b_prime_cand = candidate(&sort_spec, bubble(&sort_spec, true));
        let run_rs = format!("{}/probe_B_prime_run.rs", dir);
        std::fs::write(&run_rs, format!("{}{}", patched_b, SORT_MAIN)).unwrap();
        let run_bin = format!("{}/probe_B_prime_run", dir);
        let cc = run_timed(&dir, "probe_B_prime_runcc", "rustc", &["--edition", "2021", &run_rs, "-o", &run_bin], Duration::from_secs(60));
        assert!(matches!(cc, Some((true, _))), "B' runner binary must compile");
        let ran = run_timed(&dir, "probe_B_prime_run", &run_bin, &[], Duration::from_secs(10));
        let b_prime_verdict = ran.and_then(|(_, stdout)| {
            stdout.lines().find_map(|line| {
                let rest = line.strip_prefix("TOTAL ")?;
                let (p, t) = rest.split_once('/')?;
                Some((p.trim().parse::<usize>().ok()?, t.trim().parse::<usize>().ok()?))
            })
        });
        report.push_str(&format!("- B' compiled-vehicle verdict: {:?}\n", b_prime_verdict));
        assert_eq!(b_prime_verdict, Some((6, 6)), "B under the bound must sort 6/6");
        let _ = b_prime_cand;

        // A under the same (now native) bound: Clone does
        // not imply Copy, so plain reads still move — the
        // bound prices construction B specifically. Phase 62
        // re-record: A's generated text already carries the
        // bound; no patch is applied or needed.
        let a_generated = codegen::generate(&candidate(&sort_spec, bubble(&sort_spec, false)), "rust").unwrap();
        let patched_a = function_part(&a_generated);
        assert!(patched_a.contains("pub fn sort<T: Clone>("), "A's text carries the native bound too");
        let (a_prime_compiles, a_prime_codes) = rustc_lib(&dir, "probe_A_prime", &patched_a);
        report.push_str(&format!(
            "- A' (A as shipped, under the same native bound): compiles {}, error codes {:?}\n",
            a_prime_compiles, a_prime_codes
        ));
        assert!(!a_prime_compiles && a_prime_codes.contains(&"E0507".to_string()));

        // The library's template body IS construction B.
        let library = generate_templates(&sort_spec);
        assert_eq!(library.len(), 1, "exactly one template fires for sort");
        report.push_str(&format!(
            "- library template body == construction B: {}\n",
            format!("{:?}", library[0].1) == format!("{:?}", bubble(&sort_spec, true))
        ));
        assert_eq!(
            format!("{:?}", library[0].1),
            format!("{:?}", bubble(&sort_spec, true)),
            "the SortPrimitive template must build construction B"
        );

        // ── Part B: draws ──
        report.push_str("\n## Part B — sort draws (full pipeline)\n");
        let mut template_in_pool = 0usize;
        let mut template_finalist = 0usize;
        for draw in 0..5 {
            let (func, pool) = verified_pool(&sort_functions, "sort");
            let template = template_candidate(&pool, "sort_primitive")
                .unwrap_or_else(|| panic!("sort draw {}: template seed missing", draw))
                .clone();
            template_in_pool += 1;
            let template_verdict = runner_verdict_of(&template, &func);
            assert_eq!(
                template_verdict,
                (6, 6),
                "sort draw {}: the scan template's runner verdict since Phase 62 (the Clone bound removed the E0599 wall; Phase 61 recorded (0, 6))",
                draw
            );
            // Finalist status in the draw's own stage run.
            let (rescored, outcome) = rescore_finalists_report(pool, &func);
            let (is_finalist, capped) = match &outcome {
                RunnerOutcome::Scored { finalists, capped_non_finalists, .. } => (
                    finalists.iter().any(|f| f.candidate_id == template.id),
                    *capped_non_finalists,
                ),
                other => panic!("sort draw {}: stage must score, got {:?}", draw, other),
            };
            if is_finalist {
                template_finalist += 1;
            }
            let crown = selector::select_best(rescored, &func)
                .unwrap_or_else(|e| panic!("sort draw {}: select_best: {}", draw, e));
            let (compiles, tests) = judge_compiled(&crown, SORT_MAIN, &dir, &format!("sort_crown_{}", draw));
            report.push_str(&format!(
                "- draw {}: template verdict {:?}, finalist {}, capped {}; crown {} [{}] recorded tpr {:.3}, compiles {}, compiled verdict {:?}\n",
                draw,
                template_verdict,
                is_finalist,
                capped,
                short_hash(&body_key(&crown)),
                provenance_name(&crown),
                crown.scores.test_pass_ratio,
                compiles,
                tests,
            ));

            // ── Part C for this draw: the shipped file ──
            let (whole_compiles, whole_tests, whole_codes) =
                judge_whole_file(&crown, &dir, &format!("sort_draw_{}", draw));
            report.push_str(&format!(
                "  whole file: compiles {}, generated tests {:?}, error codes {:?}\n",
                whole_compiles, whole_tests, whole_codes
            ));
        }
        report.push_str(&format!(
            "**sort: template in pool {}/5 draws; template finalist in {}/5 draws**\n",
            template_in_pool, template_finalist
        ));

        // ── Control: count_matching (Phase 60 outcome) ──
        let cm_functions = load_functions("examples/self_host/count_matching.morph");
        report.push_str("\n## Control — count_matching (Phase 60 re-verified)\n");
        for draw in 0..2 {
            let (func, pool) = verified_pool(&cm_functions, "count_matching");
            let seed = pool
                .iter()
                .find(|c| matches!(
                    &c.provenance,
                    Provenance::ConstraintInduced { kind } if kind == "forall_fold"
                ))
                .unwrap_or_else(|| panic!("count_matching draw {}: forall-fold seed missing", draw))
                .clone();
            let (rescored, outcome) = rescore_finalists_report(pool, &func);
            assert!(
                matches!(outcome, RunnerOutcome::Scored { .. }),
                "count_matching draw {}: the stage must score (Phase 60 arm), got {:?}",
                draw,
                outcome
            );
            let crown = selector::select_best(rescored, &func).expect("select_best");
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
            let (_, tests) = judge_compiled(&crown, COUNT_MAIN, &dir, &format!("cm_crown_{}", draw));
            assert_eq!(tests, Some((3, 3)), "count_matching draw {}: crown must pass 3/3", draw);
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
