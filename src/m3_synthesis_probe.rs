// ─────────────────────────────────────────────────────────────
//  M3 SYNTHESIS PROBE (test-only) — slice 1: leaf mappings
//
//  Generalizes the M2 synthesis probe (src/m2_synthesis_probe.rs)
//  to the Codegen module: runs the real build pipeline (parse →
//  typecheck → synthesize at opt level 1 → verify_all (Z3) →
//  select_best → codegen) for every spec in
//  examples/self_host/codegen.morph, through the module-aware
//  path (synthesize_with_module + Verifier::with_specs), exactly
//  as the M2 probe does for validator.morph / printer.morph.
//
//  Per spec per run the probe records: winner provenance (with
//  the ConstraintInduced seed kind, when the winner is a seed),
//  winner test_pass_ratio (measured by the real IR interpreter),
//  winner verification status, whether a ConstraintInduced seed
//  exists in the verified pool and whether it is Verified, and
//  whether the winner's generated Rust compiles standalone
//  (rustc --crate-type lib on the emitted file — the same check
//  the M2 phases applied to target/m2-synthesis winners).
//
//  The probe never asserts on synthesis *quality* — the engine
//  is randomized — only on pipeline invariants: parse/typecheck
//  succeed and synthesize returns Ok with at least one
//  candidate. Winner code is written to
//  target/m3-synthesis/<spec>_run<N>.rs. Run with:
//
//    cargo test -- --ignored --nocapture m3_synthesis_probe
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod probe3 {
    use crate::codegen;
    use crate::spec;
    use crate::synthesis::engine::{self, IRNode, Provenance};
    use crate::synthesis::selector;
    use crate::verify;
    use std::process::Command;
    use std::time::Instant;

    fn count_holes(node: &IRNode) -> usize {
        use IRNode::*;
        match node {
            Hole { .. } => 1,
            Block(stmts) => stmts.iter().map(count_holes).sum(),
            Let { value, body, .. } => count_holes(value) + count_holes(body),
            If { condition, then_branch, else_branch } => {
                count_holes(condition) + count_holes(then_branch) + count_holes(else_branch)
            }
            While { condition, body, .. } => count_holes(condition) + count_holes(body),
            For { start, end, body, .. } => {
                count_holes(start) + count_holes(end) + count_holes(body)
            }
            Match { scrutinee, cases } => {
                count_holes(scrutinee) + cases.iter().map(|(_, b)| count_holes(b)).sum::<usize>()
            }
            Call { args, .. } => args.iter().map(count_holes).sum(),
            Return(e) => count_holes(e),
            BinOp(_, l, r) => count_holes(l) + count_holes(r),
            UnaryOp(_, e) => count_holes(e),
            Lambda { body, .. } | Closure { body, .. } => count_holes(body),
            Collection { elements, .. } => elements.iter().map(count_holes).sum(),
            Map { function, over } | Filter { predicate: function, over } => {
                count_holes(function) + count_holes(over)
            }
            Fold { operation, initial, over } => {
                count_holes(operation) + count_holes(initial) + count_holes(over)
            }
            Sort { comparator, collection } => count_holes(comparator) + count_holes(collection),
            Alloc { initial, .. } => count_holes(initial),
            Assign { target, value } => count_holes(target) + count_holes(value),
            Typed { node, .. } => count_holes(node),
            Index(c, i) => count_holes(c) + count_holes(i),
            Loop { body } => count_holes(body),
            Break { value } => value.as_ref().map(|v| count_holes(v)).unwrap_or(0),
            StructDef { body, .. } => count_holes(body),
            TryPropagate(e) => count_holes(e),
            Literal(_) | Var(_) => 0,
        }
    }

    fn provenance_name(p: &Provenance) -> String {
        match p {
            Provenance::LLMGenerated { .. } => "LLMGenerated".to_string(),
            Provenance::Mutation { .. } => "Mutation".to_string(),
            Provenance::Crossover { .. } => "Crossover".to_string(),
            Provenance::TestInduced { .. } => "TestInduced".to_string(),
            Provenance::ConstraintInduced { kind } => format!("ConstraintInduced({})", kind),
            Provenance::Template { .. } => "Template".to_string(),
            Provenance::HumanGuided => "HumanGuided".to_string(),
        }
    }

    fn load_codegen_module() -> Vec<spec::ast::FunctionSpec> {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/self_host/codegen.morph", root);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = spec::parser::parse(&source).expect("parse must succeed");
        let checked = spec::typeck::check(parsed).expect("typeck must succeed");
        checked.spec.functions
    }

    /// Compile the winner's emitted file standalone, the check
    /// the M2 phases applied externally to probe winners.
    /// Returns "ok" or the first rustc error line.
    fn compile_check(path: &str) -> String {
        let out = Command::new("rustc")
            .arg("--edition")
            .arg("2021")
            .arg("--crate-type")
            .arg("lib")
            .arg(path)
            .arg("-o")
            .arg(format!("{}.rlib", path))
            .output();
        match out {
            Ok(o) if o.status.success() => "ok".to_string(),
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let first = stderr
                    .lines()
                    .find(|l| l.starts_with("error"))
                    .unwrap_or("rustc failed");
                format!("ERR({})", first)
            }
            Err(e) => format!("ERR(spawn rustc: {})", e),
        }
    }

    #[test]
    #[ignore = "synthesis probe: randomized engine, prints a report; run explicitly with --ignored --nocapture"]
    fn m3_synthesis_probe() {
        let root = env!("CARGO_MANIFEST_DIR");
        let out_dir = format!("{}/target/m3-synthesis", root);
        std::fs::create_dir_all(&out_dir).unwrap();
        let functions = load_codegen_module();

        for run in 1..=3usize {
            for func in &functions {
                let name = func.name.clone();
                let started = Instant::now();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    engine::synthesize_with_module(func, 1, &functions)
                }));
                let synth_ms = started.elapsed().as_millis();

                let candidates = match outcome {
                    Ok(Ok(c)) => c,
                    Ok(Err(e)) => {
                        println!("PROBE3|run={}|{}|synth=ERR({})|ms={}", run, name, e, synth_ms);
                        continue;
                    }
                    Err(_) => {
                        println!("PROBE3|run={}|{}|synth=PANIC|ms={}", run, name, synth_ms);
                        continue;
                    }
                };
                assert!(
                    !candidates.is_empty(),
                    "synthesize returned no candidates for {}",
                    name
                );
                let n_candidates = candidates.len();

                // Phase 4 of the real pipeline: Z3 verification
                // with the whole module in scope.
                let verified = verify::verifier::Verifier::with_specs(&functions)
                    .verify_all(candidates, func)
                    .expect("verify_all must not error");
                let n_verified = verified.iter().filter(|c| c.verified).count();

                // Constraint-seed instrumentation: which seed
                // kinds reached the verified pool, and what
                // verdict each carries.
                let mut seed_kinds: Vec<String> = Vec::new();
                let mut seed_verified = 0usize;
                for c in &verified {
                    if let Provenance::ConstraintInduced { kind } = &c.provenance {
                        seed_kinds.push(kind.clone());
                        if c.verified {
                            seed_verified += 1;
                        }
                        println!(
                            "SEED3|run={}|{}|kind={}|verified={}|status={:?}|tpr={:.3}",
                            run, name, kind, c.verified, c.verification_status,
                            c.scores.test_pass_ratio
                        );
                    }
                }

                // Phase 5: selection, exactly as `morphic build` does.
                let winner = selector::select_best(verified, func).expect("select_best");
                let holes = count_holes(&winner.body);
                let winner_is_seed = matches!(winner.provenance, Provenance::ConstraintInduced { .. });

                // Phase 6: codegen (rust target), saved + compiled.
                let (codegen_status, compiles) = match codegen::generate(&winner, "rust") {
                    Ok(code) => {
                        let out = format!("{}/{}_run{}.rs", out_dir, name, run);
                        std::fs::write(&out, code).unwrap();
                        ("ok", compile_check(&out))
                    }
                    Err(_) => ("ERR", "n/a".to_string()),
                };

                println!(
                    "PROBE3|run={}|{}|synth=ok(n={})|ms={}|verified={}/{}|seeds=[{}]|seed_verified={}|winner_verified={}|winner_status={:?}|winner_prov={}|winner_is_seed={}|winner_holes={}|winner_score={:.3}|winner_tpr={:.3}|codegen={}|compiles={}",
                    run,
                    name,
                    n_candidates,
                    synth_ms,
                    n_verified,
                    n_candidates,
                    seed_kinds.join(","),
                    seed_verified,
                    winner.verified,
                    winner.verification_status,
                    provenance_name(&winner.provenance),
                    winner_is_seed,
                    holes,
                    winner.score,
                    winner.scores.test_pass_ratio,
                    codegen_status,
                    compiles
                );
            }
        }
    }
}
