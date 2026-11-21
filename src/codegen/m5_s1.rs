// ─────────────────────────────────────────────────────────────
//  M5-S1 PROOF (test-only) — Phase 51: the checked-in bootstrap
//  artifact + the compiled adapter, judged.
//
//  THREE CLAIMS, three instruments:
//
//  1. THE ARTIFACT IS CURRENT. `m5_s1_artifact_byte_identity`
//     extracts the Codegen winner set TWICE through the
//     original pipeline (asserting the M3 gate's two-pass
//     identity: same bodies, same provenance), renders the
//     gate's module text, applies the shipped-artifact
//     transform, and asserts the result is BYTE-IDENTICAL to
//     the checked-in src/codegen/synthesized_codegen.rs.
//     `m5_s1_regen_artifact` runs the same production and
//     writes the file in place — the documented regeneration
//     command (always from the ORIGINAL pipeline; the
//     generated compiler never generates a compiler).
//
//     THE TRANSFORM (the only difference between the gate's
//     raw render and the shipped file; winner function bodies
//     are byte-identical): the gate header is replaced by the
//     shipped header; each wrapper `mod` is promoted to
//     `pub(crate) mod` (the gate render is a standalone crate
//     root; the adapter calls the winners through their
//     wrappers — M5 audit item B); each winner's generated
//     `#[cfg(test)] mod tests` block is stripped (60 of 123
//     winners carry one — verbatim inclusion breaks
//     `cargo test` compilation, E0308 on &str → String in the
//     generated tests, the Phase 20 record; and the gate
//     compiles the module with rustc WITHOUT --test, so the
//     blocks are inert in the judged artifact).
//
//  2. THE COMPILED ADAPTER ≡ THE ORIGINAL EMITTER, over both
//     corpora (m5_s1_adapter_equivalence):
//       A. the M3 gate corpus (2,639 cases, built by the
//          gate's own corpus builders): adapter vs original
//          vs the interpreter composition (mode B) — three
//          ways, byte-compared per case, tallied per class;
//       B. the M4 whole-file corpus (149 specs, the harness's
//          own enumeration): one pipeline extraction per spec,
//          that single winner rendered by the original
//          `generate`, the interpreter composition's driver,
//          and the compiled adapter — three ways, whole files
//          byte-compared.
//     Report at target/m5-s1/equivalence-report.md.
//
//  3. MECHANICS (not ignored): the adapter's entry points and
//     walks on hand-built inputs with known output, and the
//     artifact's compiled winners called directly — the
//     instruments above must provably measure the real thing.
//
//  Run with:
//    cargo test -- --ignored --nocapture m5_s1
// ─────────────────────────────────────────────────────────────

use super::bootstrap_adapter::Adapter;
use super::generate;
use super::m3_gate::{
    self, Case, CaseKind, Composed, WinnerSet,
};
use super::m4_harness;
use crate::spec::ast::TypeRef;
use crate::synthesis::engine::{CandidateImplementation, IRNode};
use std::fmt::Write as _;

// ── The artifact production ───────────────────────────────

/// The M3 gate's module header, byte for byte (m3_gate.rs,
/// the artifact production in `m3_gate`).
const GATE_HEADER: &str = "// The synthesized codegen: the Codegen module's 123 winners,\n// each rendered by the original `generate` and wrapped in its\n// own module (so the per-file `mod tests` cannot collide).\n// Judged by the M3 gate harness (src/codegen/m3_gate.rs).\n\n";

/// The shipped artifact's header: the provenance, the
/// regeneration command, and the two adaptations — carried
/// verbatim at the top of src/codegen/synthesized_codegen.rs.
const SHIPPED_HEADER: &str = r##"// ─────────────────────────────────────────────────────────────
//  SYNTHESIZED CODEGEN — GENERATED FILE. DO NOT EDIT.
//
//  The bootstrap artifact of M5-S1 (Phase 51): the M3 Codegen
//  module's 123 synthesized winners, compiled into this crate
//  and called directly by the bootstrap adapter
//  (src/codegen/bootstrap_adapter.rs). Each winner is rendered
//  through the ORIGINAL `codegen::generate` and wrapped in its
//  own module (so names cannot collide).
//
//  WINNER-SET PROVENANCE (fixed, run-independent — M3 gate,
//  Phases 41–42; re-derived in Phase 56 after the Phase 55
//  spec amendments; re-derived in Phase 62 after the
//  Clone-bound amendment; re-derived in Phase 78 after the
//  new-kind amendment): 119 constraint-derived seeds —
//  equation 63, guard_table 39, composition_guard 7,
//  pair_guard_table 4, expr_pin_guard 3, guarded_identity 1,
//  multi_guard 1, mixed_pair_guard_table 1 — plus the 4
//  by-design seedless pool winners (rust_ident_empty,
//  rust_target_route, rust_expr_call_form,
//  rust_expr_method_form; Mutation provenance, deterministic
//  across extraction passes).
//
//  REGENERATION (always from the ORIGINAL pipeline — the
//  generated compiler never generates a compiler):
//      cargo test -- --ignored --nocapture m5_s1_regen_artifact
//  rewrites this file in place; the companion test
//  `m5_s1_artifact_byte_identity` proves a fresh extraction +
//  render reproduces this file byte-for-byte.
//
//  Two mechanical adaptations separate this file from the M3
//  gate's raw render (target/m3-gate/synthesized_codegen.rs,
//  145,404 B); winner function bodies are byte-identical:
//  (1) each winner's generated `#[cfg(test)] mod tests` block
//      is stripped (60 of 123 winners carry one): verbatim
//      inclusion breaks `cargo test` compilation — the
//      generated tests pass &str literals to String parameters
//      (E0308, the Phase 20 record) — and the M3 gate compiles
//      this module with rustc WITHOUT --test, so the blocks
//      are inert in the judged artifact;
//  (2) each wrapper `mod` is promoted to `pub(crate) mod`: the
//      gate render is a standalone crate root, while here the
//      adapter must call the winners through their wrappers
//      (M5 audit, item B).
//
//  Judged by: the M3 gate (src/codegen/m3_gate.rs) for the
//  winner set; the M5-S1 equivalence proof
//  (src/codegen/m5_s1.rs) for this compiled form.
// ─────────────────────────────────────────────────────────────

#![allow(dead_code, unused)]

"##;

/// The gate's artifact production, verbatim (m3_gate.rs):
/// the 123 winners rendered through the ORIGINAL `generate`,
/// each wrapped in its own `mod`, in module order.
fn gate_module_text(winners: &WinnerSet, order: &[String]) -> String {
    let mut module_text = String::new();
    module_text.push_str(GATE_HEADER);
    for name in order {
        let entry = winners.get(name).unwrap();
        let file = generate(&entry.candidate, "rust").expect("generate winner file");
        let _ = writeln!(module_text, "mod {} {{", name);
        module_text.push_str(&file);
        module_text.push_str("}\n\n");
    }
    module_text
}

/// The shipped-artifact transform over the gate's raw render:
/// header replacement, wrapper promotion, test-module strip.
/// Block-wise: a winner block starts at a `mod <name> {` line
/// preceded by a blank line (or at the body's start) — the
/// generated `mod tests {` line is preceded by `#[cfg(test)]`
/// instead — and ends at its last non-blank line, the winner
/// close; a carried test module occupies the lines from
/// `#[cfg(test)]` through the non-blank line just before the
/// winner close (the test module's own close).
fn shipped_artifact_text(gate_text: &str) -> String {
    assert!(
        gate_text.starts_with(GATE_HEADER),
        "gate render does not carry the gate header"
    );
    let body = &gate_text[GATE_HEADER.len()..];
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let is_winner_start = |idx: usize| -> bool {
        let line = lines[idx].trim_end_matches('\n');
        line.starts_with("mod ")
            && line.ends_with('{')
            && line != "mod tests {"
            && (idx == 0 || lines[idx - 1].trim().is_empty())
    };
    let mut starts: Vec<usize> = (0..lines.len()).filter(|&i| is_winner_start(i)).collect();
    assert_eq!(starts.len(), 123, "winner block count in the gate render");
    starts.push(lines.len());
    let mut out = String::from(SHIPPED_HEADER);
    let mut stripped = 0usize;
    for b in 0..123 {
        let block = &lines[starts[b]..starts[b + 1]];
        let close_idx = block
            .iter()
            .rposition(|l| !l.trim().is_empty())
            .expect("winner block has content");
        assert_eq!(
            block[close_idx].trim_end_matches('\n'),
            "}",
            "winner block does not end at its close"
        );
        out.push_str("pub(crate) ");
        match block
            .iter()
            .position(|l| l.trim_end_matches('\n') == "#[cfg(test)]")
        {
            Some(ti) => {
                assert_eq!(
                    block[ti + 1].trim_end_matches('\n'),
                    "mod tests {",
                    "test-module marker shape"
                );
                assert_eq!(
                    block[close_idx - 1].trim_end_matches('\n'),
                    "}",
                    "test module does not end just before the winner close"
                );
                stripped += 1;
                for l in &block[..ti] {
                    out.push_str(l);
                }
                out.push_str(block[close_idx]);
                // Any lines after the winner close inside the
                // block slice (the blank separator) ride along.
                for l in &block[close_idx + 1..] {
                    out.push_str(l);
                }
            }
            None => {
                for l in block {
                    out.push_str(l);
                }
            }
        }
    }
    assert_eq!(stripped, 60, "test-module strip count");
    out
}

/// Extract the Codegen winner set twice and assert the M3
/// gate's two-pass identity (bodies + provenance), returning
/// the first pass and the module order.
fn extract_codegen_winners_twice() -> (WinnerSet, Vec<String>) {
    let functions = m3_gate::load_module("codegen.morph");
    assert_eq!(functions.len(), 123, "Codegen module spec count");
    let order: Vec<String> = functions.iter().map(|f| f.name.clone()).collect();
    let pass1 = m3_gate::extract_winners(&functions);
    let pass2 = m3_gate::extract_winners(&functions);
    for name in &order {
        let (a, b) = (pass1.get(name).unwrap(), pass2.get(name).unwrap());
        assert_eq!(
            a.body_key, b.body_key,
            "{}: winner body differs across extraction passes",
            name
        );
        assert_eq!(
            a.provenance, b.provenance,
            "{}: winner provenance differs across extraction passes",
            name
        );
    }
    (pass1, order)
}

#[test]
#[ignore = "M5-S1 artifact identity: 2 synthesis passes x 123 specs; run explicitly with --ignored --nocapture m5_s1_artifact_byte_identity"]
fn m5_s1_artifact_byte_identity() {
    let (winners, order) = extract_codegen_winners_twice();
    let produced = shipped_artifact_text(&gate_module_text(&winners, &order));
    let root = env!("CARGO_MANIFEST_DIR");
    let checked_in =
        std::fs::read_to_string(format!("{}/src/codegen/synthesized_codegen.rs", root))
            .expect("checked-in artifact must exist");
    println!(
        "M5S1|artifact|produced-bytes={}|checked-in-bytes={}",
        produced.len(),
        checked_in.len()
    );
    assert_eq!(
        produced, checked_in,
        "the checked-in synthesized_codegen.rs is NOT what a fresh extraction + render \
         produces — regenerate it (m5_s1_regen_artifact) or record the winner-set change"
    );
    println!("M5S1-ARTIFACT|PASS|checked-in artifact is byte-identical to a fresh gate-style extraction + render");
}

#[test]
#[ignore = "M5-S1 artifact regeneration: rewrites src/codegen/synthesized_codegen.rs in place; run explicitly with --ignored --nocapture m5_s1_regen_artifact"]
fn m5_s1_regen_artifact() {
    let functions = m3_gate::load_module("codegen.morph");
    let order: Vec<String> = functions.iter().map(|f| f.name.clone()).collect();
    let winners = m3_gate::extract_winners(&functions);
    let produced = shipped_artifact_text(&gate_module_text(&winners, &order));
    let root = env!("CARGO_MANIFEST_DIR");
    let path = format!("{}/src/codegen/synthesized_codegen.rs", root);
    let before = std::fs::read_to_string(&path).unwrap_or_default();
    std::fs::write(&path, &produced).unwrap();
    println!(
        "M5S1|regen|path={}|bytes={}|changed={}",
        path,
        produced.len(),
        before != produced
    );
}

// ── The equivalence proof ─────────────────────────────────

/// Render one M3-corpus case through the compiled adapter —
/// the `m3_gate::render` dispatch with the adapter in the
/// composition's seat.
fn render_adapter(adapter: &Adapter, case: &Case) -> Result<String, String> {
    Ok(match &case.kind {
        CaseKind::Ty(ty) => adapter.ty_text(ty)?,
        CaseKind::Pat(p) => adapter.pattern_text(p)?,
        CaseKind::SExpr(e) => adapter.sexpr_text(e)?,
        CaseKind::Cons(c) => adapter.constraint_text(c)?,
        CaseKind::Expr(n) => adapter.expr_text(n)?,
        CaseKind::Stmt(n, indent, ret) => adapter.stmt_text(n, *indent, ret)?,
        CaseKind::Tail(n, indent, ret, expected) => {
            adapter.tail_text(n, *indent, ret, expected.as_ref())?
        }
        CaseKind::File(c) => adapter.driver_file(c)?,
        CaseKind::Dispatch(t) => match adapter.dispatch(t) {
            Ok(()) => "Ok".to_string(),
            Err(text) => format!("Err:{}", text),
        },
    })
}

/// The adapter side of one case, panic-isolated (the M4
/// harness precedent): a compiled winner panicking is that
/// side's recorded outcome, never a harness failure.
fn render_adapter_isolated(adapter: &Adapter, case: &Case) -> Result<String, String> {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        render_adapter(adapter, case)
    }));
    match outcome {
        Ok(result) => result,
        Err(payload) => Err(format!(
            "adapter panicked: {}",
            m4_harness::panic_text(&payload)
        )),
    }
}

#[derive(Default)]
struct ClassTally {
    total: usize,
    adapter_match: usize,
    composed_match: usize,
    adapter_vs_composed: usize,
    adapter_fail: usize,
    composed_fail: usize,
    details: Vec<String>,
}

#[test]
#[ignore = "M5-S1 equivalence: M3 corpus (2,639 cases) + M4 corpus (149 whole files, 149 pipeline runs); run explicitly with --ignored --nocapture m5_s1_adapter_equivalence"]
fn m5_s1_adapter_equivalence() {
    let root = env!("CARGO_MANIFEST_DIR");
    let out_dir = format!("{}/target/m5-s1", root);
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut report = String::new();
    let mut writelnr = |report: &mut String, s: &str| {
        println!("{}", s);
        report.push_str(s);
        report.push('\n');
    };

    // ── Part A: the M3 gate corpus, three ways ──
    let functions3 = m3_gate::load_module("codegen.morph");
    let winners = m3_gate::extract_winners(&functions3);
    assert_eq!(winners.spec_count(), 123, "Codegen winner set");
    let mut m2_sets: Vec<(&str, WinnerSet)> = Vec::new();
    for file in ["validator.morph", "printer.morph"] {
        let functions = m3_gate::load_module(file);
        m2_sets.push((file, m3_gate::extract_winners(&functions)));
    }
    let dummy = m3_gate::make_candidate(
        "dummy",
        None,
        vec![],
        vec![],
        TypeRef::Unit,
        vec![],
        IRNode::Literal(crate::synthesis::engine::IRLiteral::Unit),
        vec![],
    );
    let mut cases = m3_gate::proof_corpus();
    cases.extend(m3_gate::driver_corpus());
    cases.extend(m3_gate::dispatch_corpus(&dummy));
    let mut sets: Vec<(&str, &WinnerSet)> = vec![("codegen", &winners)];
    for (file, set) in &m2_sets {
        sets.push((file, set));
    }
    cases.extend(m3_gate::winner_corpus(&sets));
    assert_eq!(cases.len(), 2639, "the M3 gate corpus size (Phase 78 re-derivation: Phase 62's 2,573 + 66 — 18 new proof cases for the new kinds (9 spec-expr incl. the re-stated Field case, 7 IR-expr, 2 tail), the 6 new winners' whole files, and 14 new unique winner subtrees x the corpus's 3 case kinds)");

    let composed_b = Composed {
        w: &winners,
        escape_literals: true,
    };
    let adapter = Adapter;
    let mut classes: Vec<(String, ClassTally)> = Vec::new();
    let mut transcription_failures = 0usize;
    for case in &cases {
        if let Some(expected) = case.expected {
            if case.original != expected {
                transcription_failures += 1;
                writelnr(
                    &mut report,
                    &format!(
                        "M5S1|TRANSCRIPTION|{} ({}): original does not reproduce the proof golden",
                        case.class, case.label
                    ),
                );
            }
        }
        let idx = match classes.iter().position(|(c, _)| c == case.class) {
            Some(i) => i,
            None => {
                classes.push((case.class.to_string(), ClassTally::default()));
                classes.len() - 1
            }
        };
        let tally = &mut classes[idx].1;
        tally.total += 1;
        let composed = m3_gate::render(&composed_b, case);
        let adapted = render_adapter_isolated(&adapter, case);
        match (&composed, &adapted) {
            (Ok(c), Ok(a)) => {
                if *c == case.original {
                    tally.composed_match += 1;
                } else if tally.details.len() < 4 {
                    tally.details.push(format!(
                        "composed != original: {}",
                        m3_gate::diff_detail(&case.label, &case.original, c)
                    ));
                }
                if *a == case.original {
                    tally.adapter_match += 1;
                } else if tally.details.len() < 4 {
                    tally.details.push(format!(
                        "adapter != original: {}",
                        m3_gate::diff_detail(&case.label, &case.original, a)
                    ));
                }
                if a == c {
                    tally.adapter_vs_composed += 1;
                } else if tally.details.len() < 4 {
                    tally.details.push(format!(
                        "adapter != composed: {}",
                        m3_gate::diff_detail(&case.label, c, a)
                    ));
                }
            }
            _ => {
                if composed.is_err() {
                    tally.composed_fail += 1;
                    if tally.details.len() < 4 {
                        tally.details.push(format!(
                            "{}: composed EXECUTION FAILURE: {}",
                            case.label,
                            composed.as_ref().unwrap_err()
                        ));
                    }
                }
                if adapted.is_err() {
                    tally.adapter_fail += 1;
                    if tally.details.len() < 4 {
                        tally.details.push(format!(
                            "{}: adapter EXECUTION FAILURE: {}",
                            case.label,
                            adapted.as_ref().unwrap_err()
                        ));
                    }
                }
            }
        }
    }
    let (mut a_total, mut a_adapter, mut a_composed, mut a_both) = (0, 0, 0, 0);
    let (mut a_adapter_fail, mut a_composed_fail) = (0, 0);
    for (class, tally) in &classes {
        a_total += tally.total;
        a_adapter += tally.adapter_match;
        a_composed += tally.composed_match;
        a_both += tally.adapter_vs_composed;
        a_adapter_fail += tally.adapter_fail;
        a_composed_fail += tally.composed_fail;
        writelnr(
            &mut report,
            &format!(
                "M5S1|m3-class={}|total={}|adapter-match={}|composed-match={}|adapter-vs-composed={}|adapter-fail={}|composed-fail={}",
                class,
                tally.total,
                tally.adapter_match,
                tally.composed_match,
                tally.adapter_vs_composed,
                tally.adapter_fail,
                tally.composed_fail
            ),
        );
        for d in &tally.details {
            writelnr(&mut report, &format!("M5S1|m3-detail|{}", d));
        }
    }
    writelnr(
        &mut report,
        &format!(
            "M5S1|m3-totals|cases={}|adapter-match={}|composed-match={}|adapter-vs-composed={}|adapter-fail={}|composed-fail={}|transcription-failures={}",
            a_total, a_adapter, a_composed, a_both, a_adapter_fail, a_composed_fail,
            transcription_failures
        ),
    );

    // ── Part B: the M4 whole-file corpus, three ways ──
    let (corpus, _excluded) = m4_harness::enumerate_corpus();
    let mut cache = m4_harness::WinnerCache::default();
    let (mut b_total, mut b_identical) = (0usize, 0usize);
    let mut b_details: Vec<String> = Vec::new();
    for file in &corpus {
        for spec_name in &file.spec_names {
            b_total += 1;
            let functions = m4_harness::load_functions(&file.rel_path)
                .expect("corpus file loads");
            let key = format!("{}::{}", file.rel_path, spec_name);
            let winner = cache
                .winner_for(&key, || {
                    m4_harness::run_pipeline_once(&functions, spec_name)
                })
                .unwrap_or_else(|e| panic!("{} extraction failed: {}", key, e));
            let original = generate(&winner, "rust")
                .unwrap_or_else(|e| panic!("{} original render failed: {}", key, e));
            let composed = {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || composed_b.driver_file(&winner),
                ));
                match outcome {
                    Ok(r) => r.map_err(|e| format!("composed error: {}", e)),
                    Err(p) => Err(format!(
                        "composed panicked: {}",
                        m4_harness::panic_text(&p)
                    )),
                }
            };
            let adapted = {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || adapter.driver_file(&winner),
                ));
                match outcome {
                    Ok(r) => r.map_err(|e| format!("adapter error: {}", e)),
                    Err(p) => Err(format!(
                        "adapter panicked: {}",
                        m4_harness::panic_text(&p)
                    )),
                }
            };
            match (&composed, &adapted) {
                (Ok(c), Ok(a)) if *c == original && *a == original => {
                    b_identical += 1;
                }
                (Ok(c), Ok(a)) => {
                    if b_details.len() < 6 {
                        if *c != original {
                            b_details.push(format!(
                                "{}: composed != original: {}",
                                key,
                                m3_gate::diff_detail(&key, &original, c)
                            ));
                        }
                        if *a != original {
                            b_details.push(format!(
                                "{}: adapter != original: {}",
                                key,
                                m3_gate::diff_detail(&key, &original, a)
                            ));
                        }
                        if a != c {
                            b_details.push(format!(
                                "{}: adapter != composed: {}",
                                key,
                                m3_gate::diff_detail(&key, c, a)
                            ));
                        }
                    }
                }
                _ => {
                    if b_details.len() < 6 {
                        b_details.push(format!(
                            "{}: side failure — composed: {:?}, adapter: {:?}",
                            key,
                            composed.as_ref().err(),
                            adapted.as_ref().err()
                        ));
                    }
                }
            }
        }
    }
    writelnr(
        &mut report,
        &format!(
            "M5S1|m4-totals|specs={}|three-way-identical={}|pipeline-runs={}",
            b_total,
            b_identical,
            cache.pipeline_runs()
        ),
    );
    for d in &b_details {
        writelnr(&mut report, &format!("M5S1|m4-detail|{}", d));
    }

    // ── Verdict ──
    assert_eq!(transcription_failures, 0, "corpus transcription failures");
    assert_eq!(a_adapter_fail, 0, "adapter execution failures on the M3 corpus");
    assert_eq!(a_composed_fail, 0, "composed execution failures on the M3 corpus");
    assert_eq!(
        a_adapter, a_total,
        "the compiled adapter is not byte-identical to the original emitter on the M3 corpus"
    );
    assert_eq!(
        a_composed, a_total,
        "the interpreter composition is not byte-identical on the M3 corpus (gate regression)"
    );
    assert_eq!(
        a_both, a_total,
        "the compiled adapter differs from the interpreter composition it transcribes"
    );
    assert_eq!(b_total, 149, "the M4 corpus size");
    assert_eq!(
        cache.pipeline_runs(),
        149,
        "the pipeline must run exactly once per corpus spec"
    );
    assert_eq!(
        b_identical, b_total,
        "the compiled adapter is not byte-identical on the M4 whole-file corpus"
    );
    writelnr(
        &mut report,
        &format!(
            "M5S1-VERDICT|PASS|compiled adapter byte-identical to the original emitter over {} M3 cases and {} M4 whole files; adapter == interpreter composition on every case",
            a_total, b_total
        ),
    );
    std::fs::write(format!("{}/equivalence-report.md", out_dir), &report).unwrap();
}

// ── Mechanics tests (not ignored) ─────────────────────────
//
// The instruments above must provably measure the real thing:
// the adapter's entry points and walks on hand-built inputs
// with known output (expected values read off the artifact's
// own winner bodies), and the artifact's winners called
// directly.

#[test]
fn m5_adapter_driver_hand_built() {
    // The M3 driver corpus's "bare" case, expected text from
    // tests/m3_proof.rs via m3_gate::driver_corpus.
    let cand = m3_gate::make_candidate(
        "answer",
        None,
        vec![],
        vec![("n", TypeRef::Int)],
        TypeRef::Bool,
        vec![],
        m3_gate::lb(true),
        vec![],
    );
    let expected = "pub fn answer(n: i64) -> bool {\n    true\n}\n";
    assert_eq!(Adapter.driver_file(&cand).unwrap(), expected);
    assert_eq!(
        super::bootstrap_adapter::render_rust_synthesized(&cand).unwrap(),
        expected
    );
    assert_eq!(
        super::bootstrap_adapter::generate_synthesized(&cand, "rust").unwrap(),
        expected
    );
    // Dispatch: the compiled route/form winners' predictions.
    assert_eq!(Adapter.dispatch("rust"), Ok(()));
    assert_eq!(
        Adapter.dispatch("java"),
        Err("Unsupported target language: java".to_string())
    );
    let err = super::bootstrap_adapter::generate_synthesized(&cand, "java")
        .expect_err("java must be unsupported");
    assert_eq!(format!("{}", err), "Unsupported target language: java");
    // A non-rust Ok route stays with the original emitter and
    // agrees with it byte-for-byte.
    assert_eq!(
        super::bootstrap_adapter::generate_synthesized(&cand, "c").unwrap(),
        generate(&cand, "c").unwrap()
    );
}

#[test]
fn m5_adapter_walks_hand_built() {
    // Type walk: prefix(16) "Vec<" + name(0) "i64" +
    // suffix(16) ">" — the artifact's own table values.
    let list_int = TypeRef::List(Box::new(TypeRef::Int));
    assert_eq!(Adapter.ty_text(&list_int).unwrap(), "Vec<i64>");
    // Literal walk: the F5 completion is applied — the
    // escaped contents reach the literal winner, which wraps
    // them in quotes.
    let lit = m3_gate::ls("a\"b");
    assert_eq!(Adapter.expr_text(&lit).unwrap(), "\"a\\\"b\"");
    // Statement walk: the hole template at indent 1 (pad is
    // the shim-6 repetition).
    let hole = IRNode::Hole {
        id: 0,
        expected_type: None,
    };
    assert_eq!(
        Adapter.stmt_text(&hole, 1, &TypeRef::Unit).unwrap(),
        "    // TODO: synthesis incomplete\n    panic!(\"Hole not filled\");\n"
    );
}

#[test]
fn m5_artifact_winners_callable() {
    use super::synthesized_codegen as syn;
    // The checked-in artifact is linked into the crate and
    // its compiled winners answer directly.
    assert_eq!(syn::rust_type_name::rust_type_name(0), "i64");
    assert_eq!(syn::rust_type_name::rust_type_name(3), "String");
    assert!(syn::rust_keyword::rust_keyword("fn".to_string()));
    assert!(!syn::rust_keyword::rust_keyword("sort".to_string()));
    assert_eq!(
        syn::rust_unsupported_target_text::rust_unsupported_target_text("java".to_string()),
        "Unsupported target language: java"
    );
}
