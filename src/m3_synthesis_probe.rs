// ─────────────────────────────────────────────────────────────
//  M3 SYNTHESIS PROBE (test-only) — slices 1–3: leaf mappings,
//  small template families, name/text assembly
//
//  Generalizes the M2 synthesis probe (src/m2_synthesis_probe.rs)
//  to the Codegen module: runs the real build pipeline (parse →
//  typecheck → synthesize at opt level 1 → verify_all (Z3) →
//  select_best → codegen) for every spec in
//  examples/self_host/codegen.morph (all 44: the 8 slice-1 leaf
//  mappings, the 20 slice-2 template families, and the 16
//  slice-3 name/text-assembly specs — String inputs, multi-
//  String tuples up to 4 params, an Int-code pair, Bool+String
//  and Bool-only inputs), through the module-aware
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
//  the M2 phases applied to target/m2-synthesis winners). For
//  slice-3 `++` template winners the compile verdict is
//  recorded factually: the frozen emitter renders Concat as
//  Rust `+` without the string-ownership rewrite on Concat
//  operands (the Phase 25 edge), so literal-first assemblies
//  fail E0369 while variable-first ones compile — the probe
//  reports the rustc verdict, never a judgment.
//
//  For the four slice-3 specs Phase 26 predicted seedless
//  (rust_ident_suffix, rust_ident_empty, rust_string_ownership,
//  rust_generics_text) the probe additionally dumps one CAND3
//  line per verified-pool candidate: provenance, verdict,
//  status, measured tpr, the spec names the body CALLS (the
//  composition question for rust_ident_suffix: does any pool
//  candidate call rust_keyword?), its root shape, and whether
//  it contains a Concat or a Match — the characterization
//  evidence for seedless winners.
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

    /// The slice-3 specs Phase 26 predicted seedless; their
    /// pools are dumped candidate by candidate (CAND3 lines).
    const SEEDLESS_WATCH: [&str; 4] = [
        "rust_ident_suffix",
        "rust_ident_empty",
        "rust_string_ownership",
        "rust_generics_text",
    ];

    /// Body facts for the CAND3 pool dump: the spec names this
    /// body calls (sorted, deduplicated), its root shape, and
    /// whether it contains a Concat or a Match anywhere.
    fn body_facts(node: &IRNode) -> (Vec<String>, &'static str, bool, bool) {
        use crate::synthesis::engine::IRBinOp;
        use IRNode::*;
        fn walk(
            node: &IRNode,
            calls: &mut Vec<String>,
            concat: &mut bool,
            has_match: &mut bool,
        ) {
            match node {
                Call { function, args } => {
                    if !calls.contains(function) {
                        calls.push(function.clone());
                    }
                    for a in args {
                        walk(a, calls, concat, has_match);
                    }
                }
                BinOp(op, l, r) => {
                    if matches!(op, IRBinOp::Concat) {
                        *concat = true;
                    }
                    walk(l, calls, concat, has_match);
                    walk(r, calls, concat, has_match);
                }
                Match { scrutinee, cases } => {
                    *has_match = true;
                    walk(scrutinee, calls, concat, has_match);
                    for (_, b) in cases {
                        walk(b, calls, concat, has_match);
                    }
                }
                Block(stmts) => {
                    for s in stmts {
                        walk(s, calls, concat, has_match);
                    }
                }
                Let { value, body, .. } => {
                    walk(value, calls, concat, has_match);
                    walk(body, calls, concat, has_match);
                }
                If { condition, then_branch, else_branch } => {
                    walk(condition, calls, concat, has_match);
                    walk(then_branch, calls, concat, has_match);
                    walk(else_branch, calls, concat, has_match);
                }
                While { condition, body, .. } => {
                    walk(condition, calls, concat, has_match);
                    walk(body, calls, concat, has_match);
                }
                For { start, end, body, .. } => {
                    walk(start, calls, concat, has_match);
                    walk(end, calls, concat, has_match);
                    walk(body, calls, concat, has_match);
                }
                Return(e) => walk(e, calls, concat, has_match),
                UnaryOp(_, e) => walk(e, calls, concat, has_match),
                Lambda { body, .. } | Closure { body, .. } => {
                    walk(body, calls, concat, has_match)
                }
                Collection { elements, .. } => {
                    for e in elements {
                        walk(e, calls, concat, has_match);
                    }
                }
                Map { function, over } | Filter { predicate: function, over } => {
                    walk(function, calls, concat, has_match);
                    walk(over, calls, concat, has_match);
                }
                Fold { operation, initial, over } => {
                    walk(operation, calls, concat, has_match);
                    walk(initial, calls, concat, has_match);
                    walk(over, calls, concat, has_match);
                }
                Sort { comparator, collection } => {
                    walk(comparator, calls, concat, has_match);
                    walk(collection, calls, concat, has_match);
                }
                Alloc { initial, .. } => walk(initial, calls, concat, has_match),
                Assign { target, value } => {
                    walk(target, calls, concat, has_match);
                    walk(value, calls, concat, has_match);
                }
                Typed { node, .. } => walk(node, calls, concat, has_match),
                Index(c, i) => {
                    walk(c, calls, concat, has_match);
                    walk(i, calls, concat, has_match);
                }
                Loop { body } => walk(body, calls, concat, has_match),
                Break { value } => {
                    if let Some(v) = value {
                        walk(v, calls, concat, has_match);
                    }
                }
                StructDef { body, .. } => walk(body, calls, concat, has_match),
                TryPropagate(e) => walk(e, calls, concat, has_match),
                Hole { .. } | Literal(_) | Var(_) => {}
            }
        }
        let root = match node {
            Block(_) => "Block",
            Let { .. } => "Let",
            If { .. } => "If",
            While { .. } => "While",
            For { .. } => "For",
            Match { .. } => "Match",
            Call { .. } => "Call",
            Return(_) => "Return",
            BinOp(..) => "BinOp",
            UnaryOp(..) => "UnaryOp",
            Lambda { .. } => "Lambda",
            Closure { .. } => "Closure",
            Collection { .. } => "Collection",
            Map { .. } => "Map",
            Filter { .. } => "Filter",
            Fold { .. } => "Fold",
            Sort { .. } => "Sort",
            Alloc { .. } => "Alloc",
            Assign { .. } => "Assign",
            Typed { .. } => "Typed",
            Index(..) => "Index",
            Loop { .. } => "Loop",
            Break { .. } => "Break",
            StructDef { .. } => "StructDef",
            TryPropagate(_) => "TryPropagate",
            Hole { .. } => "Hole",
            Literal(_) => "Literal",
            Var(_) => "Var",
        };
        let mut calls = Vec::new();
        let mut concat = false;
        let mut has_match = false;
        walk(node, &mut calls, &mut concat, &mut has_match);
        calls.sort();
        (calls, root, concat, has_match)
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

                // Seedless-watch pool dump (Phase 27): for the
                // four slice-3 specs predicted to have no
                // constraint seed, characterize EVERY candidate
                // in the verified pool — provenance, verdict,
                // measured tpr, called spec names, root shape.
                if SEEDLESS_WATCH.contains(&name.as_str()) {
                    for c in &verified {
                        let (calls, root, concat, has_match) = body_facts(&c.body);
                        println!(
                            "CAND3|run={}|{}|prov={}|verified={}|status={:?}|tpr={:.3}|calls=[{}]|root={}|concat={}|match={}",
                            run,
                            name,
                            provenance_name(&c.provenance),
                            c.verified,
                            c.verification_status,
                            c.scores.test_pass_ratio,
                            calls.join(","),
                            root,
                            concat,
                            has_match
                        );
                    }
                }

                // Phase 5: selection, exactly as `morphic build` does.
                let winner = selector::select_best(verified, func).expect("select_best");
                let holes = count_holes(&winner.body);
                let winner_is_seed = matches!(winner.provenance, Provenance::ConstraintInduced { .. });
                let (winner_calls, winner_root, winner_concat, _) = body_facts(&winner.body);

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
                    "PROBE3|run={}|{}|synth=ok(n={})|ms={}|verified={}/{}|seeds=[{}]|seed_verified={}|winner_verified={}|winner_status={:?}|winner_prov={}|winner_is_seed={}|winner_holes={}|winner_score={:.3}|winner_tpr={:.3}|winner_calls=[{}]|winner_root={}|winner_concat={}|codegen={}|compiles={}",
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
                    winner_calls.join(","),
                    winner_root,
                    winner_concat,
                    codegen_status,
                    compiles
                );
            }
        }
    }
}
