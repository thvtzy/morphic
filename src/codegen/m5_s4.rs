// ─────────────────────────────────────────────────────────────
//  M5-S4 SC3 BENCHMARK (test-only) — Phase 64: the ROAD's
//  global SC3 (performance ≤ 2x slower than the Rust
//  original) measured for the M5 gate, in the Phase 50
//  audit's binding design (item E), with one recorded
//  deviation (below).
//
//  THE DESIGN (Phase 50, item E): the operation timed is the
//  codegen RENDER — the only stage that differs between the
//  two configurations (synthesis is identical original code
//  on both sides, so timing whole builds would drown the
//  signal). The corpora are the M3 gate corpus (2,573 cases,
//  built by the gate's own corpus builders) and the M4
//  whole-file corpus (143 specs, the harness's enumeration,
//  one pipeline extraction per spec). Both sides are timed
//  in the same build profile, in the same session, in
//  interleaved rounds; the reported figure per corpus is
//  the MEDIAN of the rounds (7 rounds ≥ the design's ≥ 5).
//  Extraction / corpus construction is never timed.
//
//  THE TWO SIDES, exactly as shipped:
//   - ORIGINAL: the original emitter — `generate` plus the
//     per-kind walk functions the M3 corpus builders
//     themselves use to produce `case.original`
//     (ty_to_rust / ir_pattern_to_rust / expr_to_rust /
//     constraint_to_rust / ir_to_rust_expr / ir_to_rust /
//     ir_to_rust_tail). The transcription below is VERIFIED
//     against `case.original` on every case in the warmup
//     pass, so the timed original side is provably the
//     render the corpus records.
//   - SYNTHESIZED: the compiled adapter calling the checked-in
//     artifact's winner functions directly (the Phase 51
//     design; the flag-on binary's actual render path —
//     `render_rust_synthesized` for whole files, the
//     `Adapter` walks per case). NOT the interpreter
//     composition: that was a gate measurement device, never
//     a shipped form, and the Phase 50 design's structural
//     expectation is recorded against the compiled path.
//  The warmup pass also asserts both sides byte-equal on
//  every case (the S1 equivalence claim), so the benchmark
//  provably times two renders of the same bytes.
//
//  PROFILE DEVIATION, recorded plainly: the Phase 50 design
//  specifies the release profile. In this sandbox a release
//  build cannot exist: z3-sys's build script (gh-release)
//  must download the Z3 4.16.0 release and fails ("Could not
//  find release for z3-4.16.0" — no network path to the
//  release); the debug build works only through the planted
//  build cache in target/debug/build, and transplanting it
//  into the release profile would be working around the
//  cache rule. The measurement therefore runs in the DEBUG
//  profile — the profile every number in this project has
//  been measured in — with both sides in the same profile
//  and session, so the RATIO (the SC3 quantity) is measured
//  like-for-like; the absolute times are debug numbers and
//  are reported as such. The benchmark refuses to run under
//  `--features bootstrap-codegen`: there `generate` routes
//  "rust" to the adapter and the original side would not be
//  the original.
//
//  Per owner decision D3 (Phase 50), SC3 at the M5 gate is a
//  RECORDED MEASUREMENT with a mandatory baseline, not a
//  hard gate item: the ratio is published either way; a
//  ratio above 2x would be a recorded owner finding, not a
//  gate failure.
//
//  Report at target/m5-s4/sc3-report.md.
//  Run with:
//    cargo test -- --ignored --nocapture m5_s4_sc3_benchmark
// ─────────────────────────────────────────────────────────────

use super::bootstrap_adapter::Adapter;
use super::m3_gate::{self, Case, CaseKind, WinnerSet};
use super::m4_harness;
use super::{
    constraint_to_rust, expr_to_rust, generate, ir_pattern_to_rust, ir_to_rust, ir_to_rust_expr,
    ir_to_rust_tail, ty_to_rust,
};
use crate::spec::ast::TypeRef;
use crate::synthesis::engine::{CandidateImplementation, IRNode};
use std::time::{Duration, Instant};

/// Rounds per side per corpus (≥ the design's 5); the
/// reported figure is the median round.
const ROUNDS: usize = 7;

/// The original emitter's render of one M3-corpus case: the
/// per-kind dispatch the corpus builders use to produce
/// `case.original` (verified against it in the warmup).
fn render_original(case: &Case, dummy: &CandidateImplementation) -> Result<String, String> {
    Ok(match &case.kind {
        CaseKind::Ty(ty) => ty_to_rust(ty),
        CaseKind::Pat(p) => ir_pattern_to_rust(p),
        CaseKind::SExpr(e) => expr_to_rust(e),
        CaseKind::Cons(c) => constraint_to_rust(c),
        CaseKind::Expr(n) => ir_to_rust_expr(n),
        CaseKind::Stmt(n, indent, ret) => ir_to_rust(n, *indent, ret),
        CaseKind::Tail(n, indent, ret, expected) => {
            ir_to_rust_tail(n, *indent, ret, expected.as_ref())
        }
        CaseKind::File(c) => generate(c, "rust").map_err(|e| format!("{}", e))?,
        CaseKind::Dispatch(t) => match generate(dummy, t) {
            Ok(_) => "Ok".to_string(),
            Err(e) => format!("Err:{}", e),
        },
    })
}

/// The compiled adapter's render of one M3-corpus case —
/// the m5_s1 equivalence test's dispatch, verbatim.
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

/// Run ROUNDS interleaved timing rounds over one corpus,
/// alternating which side starts each round (drift cancels
/// in the medians). Each pass renders the whole corpus and
/// returns a byte-count checksum.
fn bench_rounds(
    rounds_original: &mut Vec<Duration>,
    rounds_adapter: &mut Vec<Duration>,
    mut pass_original: impl FnMut() -> usize,
    mut pass_adapter: impl FnMut() -> usize,
) {
    for round in 0..ROUNDS {
        if round % 2 == 0 {
            let t = Instant::now();
            let n = pass_original();
            rounds_original.push(t.elapsed());
            std::hint::black_box(n);
            let t = Instant::now();
            let n = pass_adapter();
            rounds_adapter.push(t.elapsed());
            std::hint::black_box(n);
        } else {
            let t = Instant::now();
            let n = pass_adapter();
            rounds_adapter.push(t.elapsed());
            std::hint::black_box(n);
            let t = Instant::now();
            let n = pass_original();
            rounds_original.push(t.elapsed());
            std::hint::black_box(n);
        }
    }
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn rounds_text(times: &[Duration]) -> String {
    times
        .iter()
        .map(|t| format!("{:.1}", ms(*t)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
#[ignore = "M5-S4 SC3 benchmark: corpus extraction (117 + 14 specs, 143 pipeline runs) + 7 interleaved render rounds x 2 sides x 2 corpora; run explicitly with --ignored --nocapture m5_s4_sc3_benchmark"]
fn m5_s4_sc3_benchmark() {
    assert!(
        !cfg!(feature = "bootstrap-codegen"),
        "run the SC3 benchmark in the default configuration: with bootstrap-codegen on, \
         `generate` routes \"rust\" to the adapter and the original side is not measurable"
    );
    let root = env!("CARGO_MANIFEST_DIR");
    let out_dir = format!("{}/target/m5-s4", root);
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut report = String::new();
    let mut line = |report: &mut String, s: &str| {
        println!("{}", s);
        report.push_str(s);
        report.push('\n');
    };

    line(
        &mut report,
        "M5S4-SC3|method|profile=debug (release impossible in this sandbox: z3-sys gh-release download fails; see the module header)|rounds=7 interleaved, alternating start side|warmup=1 pass per side per corpus, byte-equality asserted|timed=render only; extraction and corpus construction excluded",
    );

    // ── Corpora (built once, never timed) ──
    let functions3 = m3_gate::load_module("codegen.morph");
    let winners = m3_gate::extract_winners(&functions3);
    assert_eq!(winners.spec_count(), 117, "Codegen winner set");
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
    assert_eq!(cases.len(), 2573, "the M3 gate corpus size");

    let (corpus4, _excluded) = m4_harness::enumerate_corpus();
    let mut cache = m4_harness::WinnerCache::default();
    let mut winners4: Vec<CandidateImplementation> = Vec::new();
    for file in &corpus4 {
        for spec_name in &file.spec_names {
            let functions = m4_harness::load_functions(&file.rel_path).expect("corpus file loads");
            let key = format!("{}::{}", file.rel_path, spec_name);
            let winner = cache
                .winner_for(&key, || {
                    m4_harness::run_pipeline_once(&functions, spec_name)
                })
                .unwrap_or_else(|e| panic!("{} extraction failed: {}", key, e));
            winners4.push(winner);
        }
    }
    assert_eq!(winners4.len(), 143, "the M4 corpus size");
    assert_eq!(cache.pipeline_runs(), 143, "extract-once: one pipeline run per corpus spec");

    let adapter = Adapter;

    // ── Warmup: both sides render every case; byte-equality
    //    with the corpus's recorded original is asserted, so
    //    the timed sides provably render the same bytes. ──
    let mut bytes3 = 0usize;
    for case in &cases {
        let original = render_original(case, &dummy)
            .unwrap_or_else(|e| panic!("{}: original render failed: {}", case.label, e));
        assert_eq!(
            original, case.original,
            "{}: the benchmark's original-side transcription does not reproduce case.original",
            case.label
        );
        let adapted = render_adapter(&adapter, case)
            .unwrap_or_else(|e| panic!("{}: adapter render failed: {}", case.label, e));
        assert_eq!(
            adapted, case.original,
            "{}: adapter != original in the benchmark warmup (S1 equivalence regression)",
            case.label
        );
        bytes3 += original.len();
    }
    let mut bytes4 = 0usize;
    for winner in &winners4 {
        let original = generate(winner, "rust").expect("original whole-file render");
        let adapted = super::bootstrap_adapter::render_rust_synthesized(winner)
            .expect("adapter whole-file render");
        assert_eq!(
            adapted, original,
            "adapter != original on an M4 whole file in the benchmark warmup"
        );
        bytes4 += original.len();
    }
    line(
        &mut report,
        &format!(
            "M5S4-SC3|warmup|m3-cases={}|m3-bytes={}|m4-files={}|m4-bytes={}|both sides byte-identical to the recorded originals on every case",
            cases.len(),
            bytes3,
            winners4.len(),
            bytes4
        ),
    );

    // ── Timed rounds: interleaved, alternating start side ──
    let (mut t3o, mut t3a) = (Vec::new(), Vec::new());
    bench_rounds(
        &mut t3o,
        &mut t3a,
        || {
            let mut total = 0usize;
            for case in &cases {
                total += render_original(case, &dummy).map(|s| s.len()).unwrap_or(0);
            }
            total
        },
        || {
            let mut total = 0usize;
            for case in &cases {
                total += render_adapter(&adapter, case).map(|s| s.len()).unwrap_or(0);
            }
            total
        },
    );
    let (med3o, med3a) = (median(t3o.clone()), median(t3a.clone()));
    let ratio3 = ms(med3a) / ms(med3o);
    line(
        &mut report,
        &format!(
            "M5S4-SC3|m3-corpus|cases={}|original-median-ms={:.1}|adapter-median-ms={:.1}|ratio={:.3}|original-rounds-ms=[{}]|adapter-rounds-ms=[{}]",
            cases.len(),
            ms(med3o),
            ms(med3a),
            ratio3,
            rounds_text(&t3o),
            rounds_text(&t3a)
        ),
    );

    let (mut t4o, mut t4a) = (Vec::new(), Vec::new());
    bench_rounds(
        &mut t4o,
        &mut t4a,
        || {
            let mut total = 0usize;
            for winner in &winners4 {
                total += generate(winner, "rust").map(|s| s.len()).unwrap_or(0);
            }
            total
        },
        || {
            let mut total = 0usize;
            for winner in &winners4 {
                total += super::bootstrap_adapter::render_rust_synthesized(winner)
                    .map(|s| s.len())
                    .unwrap_or(0);
            }
            total
        },
    );
    let (med4o, med4a) = (median(t4o.clone()), median(t4a.clone()));
    let ratio4 = ms(med4a) / ms(med4o);
    line(
        &mut report,
        &format!(
            "M5S4-SC3|m4-corpus|files={}|original-median-ms={:.1}|adapter-median-ms={:.1}|ratio={:.3}|original-rounds-ms=[{}]|adapter-rounds-ms=[{}]",
            winners4.len(),
            ms(med4o),
            ms(med4a),
            ratio4,
            rounds_text(&t4o),
            rounds_text(&t4a)
        ),
    );

    let combined_o = ms(med3o) + ms(med4o);
    let combined_a = ms(med3a) + ms(med4a);
    let combined_ratio = combined_a / combined_o;
    line(
        &mut report,
        &format!(
            "M5S4-SC3|combined|original-median-ms={:.1}|adapter-median-ms={:.1}|ratio={:.3}|sc3-threshold=2.000|within-threshold={}",
            combined_o,
            combined_a,
            combined_ratio,
            combined_ratio <= 2.0
        ),
    );
    line(
        &mut report,
        &format!(
            "M5S4-SC3|verdict|RECORDED MEASUREMENT (owner decision D3 — not a hard gate item): synthesized/original render ratio {:.3} on the M3 corpus, {:.3} on the M4 corpus, {:.3} combined, debug profile (see the profile deviation in the module header)",
            ratio3, ratio4, combined_ratio
        ),
    );
    std::fs::write(format!("{}/sc3-report.md", out_dir), &report).unwrap();
}
