// ─────────────────────────────────────────────────────────────
//  TOKENIZER SYNTHESIS PROBE (test-only) — Phase 48 (M4-S4
//  part 2: measurement of the re-authored tokenizer module)
//
//  The M2/M3 probe pattern (src/m2_synthesis_probe.rs,
//  src/m3_synthesis_probe.rs) applied to
//  examples/functional_tokenizer.morph (module
//  FunctionalTokenizer, 8 specs, re-authored in Phase 47):
//  runs the real build pipeline (parse → typecheck →
//  synthesize_with_module at opt level 1 → verify_all (Z3,
//  module-aware) → select_best → codegen) for every spec,
//  3 runs, and prints a per-spec report.
//
//  Unlike the M2/M3 probes, EVERY spec's verified pool is
//  dumped candidate by candidate (CANDT lines): the pools
//  here are small (tens, not hundreds) and the pool IS the
//  story for this module. Whether a seed reaches any
//  pool, and what selection crowns from each pool, is
//  measured here; executed agreement against the Phase 47
//  mirrors is the cross-validation harness's job
//  (src/tokenizer_cross_validation.rs).
//
//  Phase 49 update (historical): the three Char
//  classifiers gained a constraint seed — the
//  char_equation family admitted the postcondition's own
//  right-hand side for a single Char parameter, knowing
//  Z3 could not judge it (Char was Opaque to the encoder;
//  the verdict was Inconclusive by construction) — and
//  the cross-validation harness recorded the classifiers
//  EXACT by executed agreement (774/774, all runs).
//
//  Phase 79 re-instrumentation (the M1-features campaign,
//  Phases 73–78, decisions D1a–D5a): the module this probe
//  loads is the COMPLETED tokenizer — tokenize_one /
//  tokenize_all carry their restored constraints (the D1a
//  method calls), and all 18 original tests are in-spec
//  (the 9 record tests in the D3a named form). The
//  pre-campaign framing no longer holds anywhere in this
//  instrument: field access / indexing lower to real IR
//  (Phase 73; only MethodCall still lowers to Hole — no
//  IR kind exists for it, the re-pinned design fact — so
//  a constraint-derived seed built from a method-call
//  constraint carries that hole); Char encodes as its
//  Unicode scalar value (D5a, Phase 76), so the equation
//  family owns the classifiers and their seeds are
//  Z3-Verified — char_equation's territory is empty by
//  construction and its builder remains only as the
//  recorded fallback; and verification here is
//  module-aware in the full Phase 76 sense
//  (Verifier::with_module: specs + type declarations, so
//  record parameters bind symbolically instead of as
//  opaque blobs).
//
//  The probe never asserts on synthesis *quality* — the
//  engine is randomized — only on pipeline invariants
//  (parse/typecheck succeed, synthesize returns Ok with
//  at least one candidate) and, since Phase 79, on one
//  deterministic verdict fact: for each Char classifier,
//  in every run, a constraint seed reaches the verified
//  pool and every such seed is Verified. That is a Z3
//  fact about the fixed seed bodies (Phase 76), not a
//  claim about what selection crowns. Winner code is
//  written to target/tokenizer-synthesis/<spec>_run<N>.rs.
//  Run with:
//
//    cargo test -- --ignored --nocapture tokenizer_synthesis_probe
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod probe_tok {
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
            Loop { body } => count_holes(body),
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

    /// Body facts for the CANDT pool dump: the spec names
    /// this body calls (sorted, deduplicated), its root
    /// shape, and whether it contains a Concat or a Match
    /// anywhere. (Copied from the M3 probe's body_facts.)
    fn body_facts(node: &IRNode) -> (Vec<String>, &'static str, bool, bool) {
        use crate::synthesis::engine::IRBinOp;
        use IRNode::*;
        fn walk(node: &IRNode, calls: &mut Vec<String>, concat: &mut bool, has_match: &mut bool) {
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
                Lambda { body, .. } | Closure { body, .. } => walk(body, calls, concat, has_match),
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
                // M1 features (Phase 73): structural recursion.
                FieldAccess { base, .. } => walk(base, calls, concat, has_match),
                RecordLit { fields, .. } => {
                    for (_, v) in fields {
                        walk(v, calls, concat, has_match);
                    }
                }
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
            // M1 features (Phase 73).
            FieldAccess { .. } => "FieldAccess",
            RecordLit { .. } => "RecordLit",
        };
        let mut calls = Vec::new();
        let mut concat = false;
        let mut has_match = false;
        walk(node, &mut calls, &mut concat, &mut has_match);
        calls.sort();
        (calls, root, concat, has_match)
    }

    fn load_tokenizer_module() -> spec::ast::Spec {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/functional_tokenizer.morph", root);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = spec::parser::parse(&source).expect("parse must succeed");
        let checked = spec::typeck::check(parsed).expect("typeck must succeed");
        checked.spec
    }

    #[test]
    #[ignore = "synthesis probe: randomized engine, prints a report; run explicitly with --ignored --nocapture"]
    fn tokenizer_synthesis_probe() {
        let root = env!("CARGO_MANIFEST_DIR");
        let out_dir = format!("{}/target/tokenizer-synthesis", root);
        std::fs::create_dir_all(&out_dir).unwrap();
        let module = load_tokenizer_module();
        let functions = &module.functions;
        assert_eq!(functions.len(), 8, "tokenizer module spec count");

        for run in 1..=3usize {
            for func in functions {
                let name = func.name.clone();
                let started = Instant::now();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    engine::synthesize_with_module(func, 1, functions)
                }));
                let synth_ms = started.elapsed().as_millis();

                let candidates = match outcome {
                    Ok(Ok(c)) => c,
                    Ok(Err(e)) => {
                        println!("PROBET|run={}|{}|synth=ERR({})|ms={}", run, name, e, synth_ms);
                        continue;
                    }
                    Err(_) => {
                        println!("PROBET|run={}|{}|synth=PANIC|ms={}", run, name, synth_ms);
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
                // with the whole module in scope — specs AND
                // type declarations (Phase 76), so record
                // parameters bind symbolically.
                let verified = verify::verifier::Verifier::with_module(&module)
                    .verify_all(candidates, func)
                    .expect("verify_all must not error");
                let n_verified = verified.iter().filter(|c| c.verified).count();
                let n_unrefuted = verified
                    .iter()
                    .filter(|c| {
                        c.verification_status
                            != Some(verify::VerificationStatus::VerificationFailed)
                    })
                    .count();

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
                            "SEEDT|run={}|{}|kind={}|verified={}|status={:?}|tpr={:.3}",
                            run, name, kind, c.verified, c.verification_status,
                            c.scores.test_pass_ratio
                        );
                    }
                }

                // Phase 79 assertion (the post-D5a reality):
                // for each Char classifier, a constraint seed
                // reaches the verified pool in every run and
                // every such seed is Verified — a deterministic
                // Z3 fact about the fixed seed bodies
                // (Phase 76), asserted so an encoder or seed-
                // admission regression is loud. (Pre-D5a the
                // classifiers' only seed was char_equation's,
                // Inconclusive by construction; post-D5a the
                // equation family owns them.)
                if matches!(
                    name.as_str(),
                    "is_whitespace" | "is_ident_start" | "is_ident_continue"
                ) {
                    assert!(
                        !seed_kinds.is_empty(),
                        "classifier {}: no constraint seed reached the pool",
                        name
                    );
                    assert_eq!(
                        seed_verified,
                        seed_kinds.len(),
                        "classifier {}: constraint seed(s) not all Verified: {:?}",
                        name,
                        seed_kinds
                    );
                }

                // Full verified-pool dump for EVERY spec (the
                // pools are small and are this module's
                // measurement story — see the header).
                for c in &verified {
                    let (calls, root, concat, has_match) = body_facts(&c.body);
                    println!(
                        "CANDT|run={}|{}|prov={}|verified={}|status={:?}|tpr={:.3}|holes={}|calls=[{}]|root={}|concat={}|match={}",
                        run,
                        name,
                        provenance_name(&c.provenance),
                        c.verified,
                        c.verification_status,
                        c.scores.test_pass_ratio,
                        count_holes(&c.body),
                        calls.join(","),
                        root,
                        concat,
                        has_match
                    );
                }

                // Phase 5: selection, exactly as `morphic build` does.
                let winner = selector::select_best(verified, func).expect("select_best");
                let holes = count_holes(&winner.body);
                let winner_is_seed =
                    matches!(winner.provenance, Provenance::ConstraintInduced { .. });
                let (winner_calls, winner_root, winner_concat, _) = body_facts(&winner.body);

                // Phase 6: codegen (rust target), saved for
                // external checks (the cross-validation
                // harness compiles + executes from these).
                let codegen_status = match codegen::generate(&winner, "rust") {
                    Ok(code) => {
                        let out = format!("{}/{}_run{}.rs", out_dir, name, run);
                        std::fs::write(&out, code).unwrap();
                        "ok"
                    }
                    Err(_) => "ERR",
                };

                println!(
                    "PROBET|run={}|{}|synth=ok(n={})|ms={}|verified={}/{}|unrefuted={}|seeds=[{}]|seed_verified={}|winner_verified={}|winner_status={:?}|winner_prov={}|winner_is_seed={}|winner_holes={}|winner_score={:.3}|winner_tpr={:.3}|winner_calls=[{}]|winner_root={}|winner_concat={}|codegen={}",
                    run,
                    name,
                    n_candidates,
                    synth_ms,
                    n_verified,
                    n_candidates,
                    n_unrefuted,
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
                    codegen_status
                );
            }
        }
    }
}
