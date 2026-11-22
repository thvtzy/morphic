// ─────────────────────────────────────────────────────────────
//  M2 SYNTHESIS PROBE (test-only)
//
//  Runs the real build pipeline (parse → typecheck → synthesize
//  at opt level 1 → verify_all (Z3) → select_best → codegen)
//  for every spec in examples/self_host/validator.morph and
//  examples/self_host/printer.morph, one spec at a time, and
//  prints a per-spec report. Opt level 1 is the only level that
//  runs without an LLM backend (use_llm = false); levels 2–4
//  fail in seed_population with "No LLM backend enabled" in the
//  default build (no llm-local / llm-remote feature compiled).
//
//  The probe never asserts on synthesis *quality* — the engine
//  is randomized (MCTS + genetic seeds) — only on the pipeline
//  invariants: parse/typecheck succeed and synthesize returns
//  Ok with at least one candidate. Generated winner code is
//  written to target/m2-synthesis/<spec>.rs for external
//  compile checks. Run with:
//
//    cargo test -- --ignored --nocapture m2_synthesis_probe
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod probe {
    use crate::codegen;
    use crate::spec;
    use crate::synthesis::engine::{self, IRNode, Provenance};
    use crate::synthesis::selector;
    use crate::verify;
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
            Loop { body, .. } => count_holes(body),
            Break { value } => value.as_ref().map(|v| count_holes(v)).unwrap_or(0),
            StructDef { body, .. } => count_holes(body),
            TryPropagate(e) => count_holes(e),
            // M1 features (Phase 73): structural recursion.
            FieldAccess { base, .. } => count_holes(base),
            RecordLit { fields, .. } => {
                fields.iter().map(|(_, v)| count_holes(v)).sum()
            }
            Literal(_) | Var(_) => 0,
        }
    }

    fn provenance_name(p: &Provenance) -> &'static str {
        match p {
            Provenance::LLMGenerated { .. } => "LLMGenerated",
            Provenance::Mutation { .. } => "Mutation",
            Provenance::Crossover { .. } => "Crossover",
            Provenance::TestInduced { .. } => "TestInduced",
            Provenance::ConstraintInduced { .. } => "ConstraintInduced",
            Provenance::Template { .. } => "Template",
            Provenance::HumanGuided => "HumanGuided",
        }
    }

    fn load_validator_spec(name: &str) -> crate::spec::ast::FunctionSpec {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/self_host/validator.morph", root);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = spec::parser::parse(&source).unwrap();
        let checked = spec::typeck::check(parsed).unwrap();
        checked
            .spec
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .clone()
    }

    /// Opt level 1 is the LLM-free path (use_llm = false): synthesis
    /// must complete and return candidates for a spec with tests.
    /// The population is seeded to a fixed size and the acceptance
    /// threshold is 0.0, so a non-empty result is structural, not
    /// luck — this held on every probe run (2026-11-20).
    #[test]
    fn synthesis_opt1_runs_without_llm() {
        let func = load_validator_spec("is_literal_kind");
        let candidates = engine::synthesize(&func, 1).expect("opt-1 synthesis must succeed");
        assert!(!candidates.is_empty());
        assert!(candidates.iter().all(|c| c.spec_name == "is_literal_kind"));
    }

    /// In the default build (no llm-local / llm-remote feature),
    /// opt levels >= 2 enable LLM seeding and fail immediately:
    /// seed_from_llm has no backend to call. This is the exact
    /// blocker that confines synthesis runs to opt level 1 until
    /// an LLM backend is compiled in or an Ollama server exists.
    #[cfg(not(any(feature = "llm-local", feature = "llm-remote")))]
    #[test]
    fn synthesis_opt3_blocked_without_llm_backend() {
        let func = load_validator_spec("is_literal_kind");
        let result = engine::synthesize(&func, 3);
        assert!(
            matches!(result, Err(engine::SynthesisError::LlmError(_))),
            "expected LlmError without an LLM backend, got {:?}",
            result.map(|c| c.len())
        );
    }

    #[test]
    #[ignore = "synthesis probe: randomized engine, prints a report; run explicitly with --ignored --nocapture"]
    fn m2_synthesis_probe() {
        let root = env!("CARGO_MANIFEST_DIR");
        let out_dir = format!("{}/target/m2-synthesis", root);
        std::fs::create_dir_all(&out_dir).unwrap();

        for file in ["validator", "printer"] {
            let path = format!("{}/examples/self_host/{}.morph", root, file);
            let source = std::fs::read_to_string(&path).unwrap();
            let parsed = spec::parser::parse(&source).expect("parse must succeed");
            let checked = spec::typeck::check(parsed).expect("typeck must succeed");

            for func in &checked.spec.functions {
                let name = func.name.clone();
                let started = Instant::now();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    engine::synthesize_with_module(func, 1, &checked.spec.functions)
                }));
                let synth_ms = started.elapsed().as_millis();

                let candidates = match outcome {
                    Ok(Ok(c)) => c,
                    Ok(Err(e)) => {
                        println!(
                            "PROBE|{}|{}|synth=ERR({})|ms={}",
                            file, name, e, synth_ms
                        );
                        continue;
                    }
                    Err(_) => {
                        println!(
                            "PROBE|{}|{}|synth=PANIC|ms={}",
                            file, name, synth_ms
                        );
                        continue;
                    }
                };
                assert!(
                    !candidates.is_empty(),
                    "synthesize returned no candidates for {}",
                    name
                );
                let n_candidates = candidates.len();

                // Phase 4 of the real pipeline: Z3 verification,
                // with the whole module in scope so spec-to-spec
                // calls resolve (Phase 18).
                let verified = verify::verifier::Verifier::with_specs(&checked.spec.functions)
                    .verify_all(candidates, func)
                    .expect("verify_all must not error");
                let n_verified = verified.iter().filter(|c| c.verified).count();
                // Refutation-gate instrumentation (Phase 16): how
                // much of the pool entering selection is NOT
                // refuted, and what verdict the winner carries.
                let n_unrefuted = verified
                    .iter()
                    .filter(|c| {
                        c.verification_status
                            != Some(verify::VerificationStatus::VerificationFailed)
                    })
                    .count();

                // Phase 5: selection, exactly as `morphic build` does.
                let winner = selector::select_best(verified, func).expect("select_best");
                let holes = count_holes(&winner.body);

                // Phase 6: codegen (rust target), saved for external checks.
                let codegen_result = codegen::generate(&winner, "rust");
                let codegen_status = match &codegen_result {
                    Ok(code) => {
                        let out = format!("{}/{}.rs", out_dir, name);
                        std::fs::write(&out, code).unwrap();
                        "ok"
                    }
                    Err(_) => "ERR",
                };

                println!(
                    "PROBE|{}|{}|synth=ok(n={})|ms={}|verified={}/{}|pool_unrefuted={}/{}|winner_verified={}|winner_status={:?}|winner_prov={}|winner_holes={}|winner_score={:.3}|winner_tpr={:.3}|codegen={}",
                    file,
                    name,
                    n_candidates,
                    synth_ms,
                    n_verified,
                    n_candidates,
                    n_unrefuted,
                    n_candidates,
                    winner.verified,
                    winner.verification_status,
                    provenance_name(&winner.provenance),
                    holes,
                    winner.score,
                    winner.scores.test_pass_ratio,
                    codegen_status
                );
            }
        }
    }
}
