// ─────────────────────────────────────────────────────────────
//  M5-S2 (test-only) — Phase 52: the feature-flag switch and
//  the M5 roundtrip, judged.
//
//  THE SWITCH (production, in super::generate_rust_route):
//  with the Cargo feature `bootstrap-codegen` (owner decision
//  D4), `generate`'s "rust" route renders through the compiled
//  synthesized adapter; without it, through the original
//  `generate_rust`. Every other route is untouched in both
//  configurations.
//
//  THE ROUNDTRIP, in the Phase 50 audit's chosen form — the
//  literal reading (two independent binary runs producing
//  byte-equal sort output) is unmeasurable: winner selection
//  is draw-dependent (6 consecutive `-O 1` builds of
//  sort.morph produced 6 distinct outputs in the audit; no
//  RNG seed knob exists). What is judged instead:
//
//  (a) EXTRACT-ONCE BYTE ROUNDTRIP
//      (`m5_s2_roundtrip_extract_once`): sort's winner is
//      extracted ONCE through the real pipeline (the M4
//      harness's WinnerCache discipline) and rendered through
//      the original `generate_rust` AND through the feature-on
//      dispatch path (`bootstrap_adapter::generate_synthesized`
//      — exactly what `generate`'s rust route calls with the
//      feature on), byte-compared; `generate`'s own output is
//      asserted to follow the routing of the configuration
//      this test is compiled under. Repeated for the other
//      three standalone examples (binary_search, is_sorted,
//      count_matching) as supporting evidence.
//      Report at target/m5-s2/roundtrip-report.md.
//
//  (b) PROCESS-LEVEL BEHAVIORAL ROUNDTRIP
//      (`m5_s2_roundtrip_behavioral`): the real binary is
//      built BOTH ways (cargo build, with and without
//      `--features bootstrap-codegen` — the e2e CLI-test
//      precedent) and each binary builds examples/sort.morph
//      at `-O 1` five times. Per drawn output, recorded for
//      BOTH configurations: the build's end-to-end success
//      (asserted for every draw — the binary runs, synthesizes
//      and writes its output), the whole file's rustc outcome,
//      the winner function's rustc outcome (the file minus
//      its generated test module), and the function's executed
//      behavior against sort's six in-spec tests via a
//      hand-written runner (the cross-validation harness
//      pattern; the generated test module cannot be the
//      vehicle — see the report). The compile/test outcomes
//      are recorded, not asserted: they are properties of the
//      drawn winner bodies (a synthesis-stage property both
//      binaries share), and the measured control behavior
//      supports no every-draw floor — see the ROAD note.
//      Report at target/m5-s2/behavioral-report.md.
//
//  THE DIVISION, stated exactly: a flag-on/flag-off whole-file
//  byte comparison on a FIXED winner at the BINARY level is
//  not possible (each binary synthesizes its own winner), and
//  is not faked here — (a) covers the byte claim at the
//  dispatch level, (b) covers the binary level behaviorally.
//
//  Run with:
//    cargo test -- --ignored --nocapture m5_s2
// ─────────────────────────────────────────────────────────────

use super::bootstrap_adapter;
use super::generate;
use super::m3_gate;
use super::m4_harness;
use crate::spec::ast::TypeRef;
use std::fmt::Write as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ── Mechanics (not ignored) ───────────────────────────────
//
// The switch itself, pinned in BOTH configurations: this test
// is compiled with and without the feature (the two suite
// runs), and asserts `generate`'s rust route follows the
// configuration it was compiled under, while every other
// route behaves exactly as before.

#[test]
fn m5_s2_dispatch_routing() {
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
    let original = super::generate_rust(&cand).unwrap();
    let adapted = bootstrap_adapter::render_rust_synthesized(&cand).unwrap();
    assert_eq!(original, expected);
    assert_eq!(adapted, expected);
    // The dispatch follows the compiled configuration.
    let routed = if cfg!(feature = "bootstrap-codegen") {
        adapted
    } else {
        original
    };
    assert_eq!(generate(&cand, "rust").unwrap(), routed);
    // The adapter's dispatch-shaped entry agrees on "rust".
    assert_eq!(
        bootstrap_adapter::generate_synthesized(&cand, "rust").unwrap(),
        expected
    );
    // Other routes are the original's in both configurations:
    // the adapter's fallback and `generate` agree byte-for-byte.
    for target in ["c", "wasm", "python", "javascript", "ir"] {
        assert_eq!(
            generate(&cand, target).unwrap(),
            bootstrap_adapter::generate_synthesized(&cand, target).unwrap(),
            "route {} must be configuration-independent",
            target
        );
    }
    // Unsupported targets error identically in both.
    let err = generate(&cand, "java").expect_err("java must be unsupported");
    assert_eq!(format!("{}", err), "Unsupported target language: java");
}

// ── (a) The extract-once byte roundtrip ───────────────────

#[test]
#[ignore = "M5-S2 roundtrip (a): 4 pipeline extractions + three-way renders; run explicitly with --ignored --nocapture m5_s2_roundtrip_extract_once"]
fn m5_s2_roundtrip_extract_once() {
    let root = env!("CARGO_MANIFEST_DIR");
    let out_dir = format!("{}/target/m5-s2", root);
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut report = String::new();
    let mut writelnr = |report: &mut String, s: &str| {
        println!("{}", s);
        report.push_str(s);
        report.push('\n');
    };
    writelnr(
        &mut report,
        "# M5-S2 roundtrip report (a) — extract-once byte comparison (Phase 52)",
    );
    writelnr(&mut report, "");
    let _ = writeln!(
        report,
        "Configuration of this run: bootstrap-codegen feature {} (the dispatch assertion follows the compiled routing; the byte claim itself is configuration-independent).",
        if cfg!(feature = "bootstrap-codegen") {
            "ON"
        } else {
            "OFF"
        }
    );
    println!(
        "M5S2|config|bootstrap-codegen={}",
        cfg!(feature = "bootstrap-codegen")
    );
    writelnr(&mut report, "");

    let files: [(&str, &str); 4] = [
        ("examples/sort.morph", "sort"),
        ("examples/binary_search.morph", "binary_search"),
        ("examples/self_host/is_sorted.morph", "is_sorted"),
        ("examples/self_host/count_matching.morph", "count_matching"),
    ];
    let mut cache = m4_harness::WinnerCache::default();
    for (rel_path, spec_name) in files {
        let functions = m4_harness::load_functions(rel_path)
            .unwrap_or_else(|e| panic!("{}: {}", rel_path, e));
        let key = format!("{}::{}", rel_path, spec_name);
        let winner = cache
            .winner_for(&key, || {
                m4_harness::run_pipeline_once(&functions, spec_name)
            })
            .unwrap_or_else(|e| panic!("{} extraction failed: {}", key, e));
        let original = super::generate_rust(&winner)
            .unwrap_or_else(|e| panic!("{} original render failed: {}", key, e));
        let adapted = bootstrap_adapter::generate_synthesized(&winner, "rust")
            .unwrap_or_else(|e| panic!("{} adapter render failed: {}", key, e));
        let adapted_direct = bootstrap_adapter::render_rust_synthesized(&winner)
            .unwrap_or_else(|e| panic!("{} adapter direct render failed: {}", key, e));
        let dispatched = generate(&winner, "rust")
            .unwrap_or_else(|e| panic!("{} dispatch render failed: {}", key, e));
        let routed = if cfg!(feature = "bootstrap-codegen") {
            &adapted
        } else {
            &original
        };
        writelnr(
            &mut report,
            &format!(
                "- `{}` spec `{}` — winner {:?} [body {}] — {} bytes — original == adapter: {}; dispatch follows the {} route: {}",
                rel_path,
                spec_name,
                winner.provenance,
                m4_harness::short_hash(&m4_harness::body_key(&winner)),
                original.len(),
                original == adapted,
                if cfg!(feature = "bootstrap-codegen") {
                    "adapter (feature ON)"
                } else {
                    "original (feature OFF)"
                },
                dispatched == *routed,
            ),
        );
        assert_eq!(
            adapted, original,
            "{}: the feature-on dispatch path does not render the extracted winner byte-identically to the original emitter",
            key
        );
        assert_eq!(adapted_direct, adapted, "{}: adapter entries disagree", key);
        assert_eq!(
            dispatched, *routed,
            "{}: `generate`'s rust route does not follow the compiled configuration",
            key
        );
    }
    assert_eq!(
        cache.pipeline_runs(),
        4,
        "the pipeline must run exactly once per standalone file"
    );
    writelnr(&mut report, "");
    writelnr(
        &mut report,
        "Verdict: 4/4 extracted winners render byte-identically through the original emitter and the feature-on dispatch path; the pipeline ran exactly once per file.",
    );
    std::fs::write(format!("{}/roundtrip-report.md", out_dir), &report).unwrap();
    println!("M5S2-ROUNDTRIP-A|PASS|4/4 byte-identical (sort + binary_search + is_sorted + count_matching)");
}

// ── (b) The process-level behavioral roundtrip ────────────

/// Run a command to completion with a timeout, its stdout /
/// stderr redirected to files under `dir` (no pipe-buffer
/// deadlock, and the child's output survives for the report).
/// Returns None on timeout (the child is killed).
fn run_timed(
    dir: &str,
    tag: &str,
    program: &str,
    args: &[&str],
    cwd: &str,
    timeout: Duration,
) -> Option<(bool, String)> {
    let out_path = format!("{}/{}.stdout", dir, tag);
    let err_path = format!("{}/{}.stderr", dir, tag);
    let out_file = std::fs::File::create(&out_path).ok()?;
    let err_file = std::fs::File::create(&err_path).ok()?;
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::from(err_file))
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stderr = std::fs::read_to_string(&err_path).unwrap_or_default();
                return Some((status.success(), stderr));
            }
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

/// sort's six in-spec tests (examples/sort.morph), as a
/// hand-written runner main appended to the drawn winner
/// function — the cross-validation harness pattern (the
/// generated test module cannot execute these: the original
/// emitter renders the tuple input as `__tuple__(...)` and
/// the fn-typed cmp argument as `??`, which does not parse;
/// both configurations inherit that rendering byte-identically).
const SORT_RUNNER_MAIN: &str = r#"
fn main() {
    let lt = |a: i64, b: i64| a < b;
    assert_eq!(sort::<i64>(vec![], lt), Vec::<i64>::new());
    assert_eq!(sort::<i64>(vec![5], lt), vec![5]);
    assert_eq!(sort::<i64>(vec![1, 2, 3, 4, 5], lt), vec![1, 2, 3, 4, 5]);
    assert_eq!(sort::<i64>(vec![5, 4, 3, 2, 1], lt), vec![1, 2, 3, 4, 5]);
    assert_eq!(sort::<i64>(vec![3, 1, 4, 1, 5, 9, 2, 6], lt), vec![1, 1, 2, 3, 4, 5, 6, 9]);
    assert_eq!(sort::<i64>(vec![2, 2, 1, 1, 3, 3], lt), vec![1, 1, 2, 2, 3, 3]);
    println!("ALL 6 IN-SPEC TESTS PASS");
}
"#;

/// The winner-function part of a generated file: everything
/// before the generated `#[cfg(test)]` module (which
/// `generate_rust` appends last).
fn function_part(generated: &str) -> String {
    match generated
        .lines()
        .position(|l| l.trim() == "#[cfg(test)]")
    {
        Some(idx) => generated
            .lines()
            .take(idx)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
        None => generated.to_string(),
    }
}

struct DrawOutcome {
    build_ok: bool,
    bytes: usize,
    distinct_key: String,
    whole_file_compiles: bool,
    function_compiles: bool,
    tests_pass: bool,
    detail: String,
}

fn one_draw(
    root: &str,
    draws_dir: &str,
    bin: &str,
    config: &str,
    draw: usize,
) -> DrawOutcome {
    let tag = format!("{}_{}", config, draw);
    let out_rs = format!("{}/{}.rs", draws_dir, tag);
    let sort_path = format!("{}/examples/sort.morph", root);
    // The binary builds sort.morph end-to-end at -O 1 (the
    // audit: -O 3 cannot build without an LLM backend).
    let build = run_timed(
        draws_dir,
        &format!("{}_build", tag),
        bin,
        &[
            "build",
            &sort_path,
            "--target",
            "rust",
            "-O",
            "1",
            "-o",
            &out_rs,
        ],
        root,
        Duration::from_secs(120),
    );
    let build_ok = matches!(build, Some((true, _))) && std::path::Path::new(&out_rs).exists();
    if !build_ok {
        return DrawOutcome {
            build_ok: false,
            bytes: 0,
            distinct_key: String::new(),
            whole_file_compiles: false,
            function_compiles: false,
            tests_pass: false,
            detail: format!("build failed or timed out: {:?}", build.map(|(_, e)| e)),
        };
    }
    let generated = std::fs::read_to_string(&out_rs).unwrap_or_default();
    // (i) the whole produced file under rustc, as a lib.
    let whole = run_timed(
        draws_dir,
        &format!("{}_whole", tag),
        "rustc",
        &["--edition", "2021", "--crate-type", "lib", &out_rs, "-o", &format!("{}/{}.rlib", draws_dir, tag)],
        root,
        Duration::from_secs(120),
    );
    let whole_file_compiles = matches!(whole, Some((true, _)));
    // (i') the winner function under rustc (the file minus
    // its generated test module).
    let fn_rs = format!("{}/{}_fn.rs", draws_dir, tag);
    std::fs::write(&fn_rs, function_part(&generated)).unwrap();
    let func = run_timed(
        draws_dir,
        &format!("{}_func", tag),
        "rustc",
        &["--edition", "2021", "--crate-type", "lib", &fn_rs, "-o", &format!("{}/{}_fn.rlib", draws_dir, tag)],
        root,
        Duration::from_secs(120),
    );
    let function_compiles = matches!(func, Some((true, _)));
    // (ii) the winner function executed against sort's six
    // in-spec tests, via the hand-written runner.
    let run_rs = format!("{}/{}_run.rs", draws_dir, tag);
    let mut runner_src = function_part(&generated);
    runner_src.push_str(SORT_RUNNER_MAIN);
    std::fs::write(&run_rs, &runner_src).unwrap();
    let run_bin = format!("{}/{}_run", draws_dir, tag);
    let compiled = run_timed(
        draws_dir,
        &format!("{}_runcc", tag),
        "rustc",
        &["--edition", "2021", &run_rs, "-o", &run_bin],
        root,
        Duration::from_secs(120),
    );
    let tests_pass = match compiled {
        Some((true, _)) => matches!(
            run_timed(
                draws_dir,
                &format!("{}_run", tag),
                &run_bin,
                &[],
                root,
                Duration::from_secs(10),
            ),
            Some((true, _))
        ),
        _ => false,
    };
    DrawOutcome {
        build_ok: true,
        bytes: generated.len(),
        distinct_key: m4_harness::short_hash(&generated),
        whole_file_compiles,
        function_compiles,
        tests_pass,
        detail: String::new(),
    }
}

#[test]
#[ignore = "M5-S2 roundtrip (b): builds the binary both ways (nested cargo build) + 2x5 sort builds with rustc/runner per draw; run explicitly with --ignored --nocapture m5_s2_roundtrip_behavioral"]
fn m5_s2_roundtrip_behavioral() {
    let root = env!("CARGO_MANIFEST_DIR").to_string();
    let out_dir = format!("{}/target/m5-s2", root);
    let draws_dir = format!("{}/draws", out_dir);
    std::fs::create_dir_all(&draws_dir).unwrap();
    let mut report = String::new();
    let mut writelnr = |report: &mut String, s: &str| {
        println!("{}", s);
        report.push_str(s);
        report.push('\n');
    };
    writelnr(
        &mut report,
        "# M5-S2 roundtrip report (b) — process-level behavioral roundtrip (Phase 52)",
    );
    writelnr(&mut report, "");

    // Build the real binary BOTH ways (the e2e CLI-test
    // precedent: cargo inside the test). Each build lands at
    // target/debug/morphic; it is copied aside before the
    // next build overwrites it.
    let cargo = "cargo";
    let off = run_timed(
        &out_dir,
        "build_off",
        cargo,
        &["build"],
        &root,
        Duration::from_secs(900),
    );
    assert!(
        matches!(off, Some((true, _))),
        "the flag-off cargo build failed: {:?}",
        off.map(|(_, e)| e)
    );
    let bin_off = format!("{}/morphic-off", out_dir);
    std::fs::copy(format!("{}/target/debug/morphic", root), &bin_off).unwrap();
    let on = run_timed(
        &out_dir,
        "build_on",
        cargo,
        &["build", "--features", "bootstrap-codegen"],
        &root,
        Duration::from_secs(900),
    );
    assert!(
        matches!(on, Some((true, _))),
        "the flag-on cargo build failed: {:?}",
        on.map(|(_, e)| e)
    );
    let bin_on = format!("{}/morphic-on", out_dir);
    std::fs::copy(format!("{}/target/debug/morphic", root), &bin_on).unwrap();
    // Restore the default binary at target/debug/morphic.
    let _ = run_timed(
        &out_dir,
        "build_restore",
        cargo,
        &["build"],
        &root,
        Duration::from_secs(900),
    );
    writelnr(
        &mut report,
        "Both binaries built from this source tree: flag-off (the original compiler) and flag-on (`--features bootstrap-codegen`, the bootstrap binary). Each builds examples/sort.morph at `-O 1`, five draws.",
    );
    writelnr(&mut report, "");

    let mut totals: Vec<(&str, usize, usize, usize, usize)> = Vec::new();
    for (config, bin) in [("off", &bin_off), ("on", &bin_on)] {
        let (mut builds, mut wholes, mut funcs, mut passes) = (0, 0, 0, 0);
        let mut distinct: Vec<String> = Vec::new();
        for draw in 1..=5 {
            let outcome = one_draw(&root, &draws_dir, bin, config, draw);
            if outcome.build_ok {
                builds += 1;
            }
            if outcome.whole_file_compiles {
                wholes += 1;
            }
            if outcome.function_compiles {
                funcs += 1;
            }
            if outcome.tests_pass {
                passes += 1;
            }
            if !distinct.contains(&outcome.distinct_key) {
                distinct.push(outcome.distinct_key.clone());
            }
            writelnr(
                &mut report,
                &format!(
                    "- config {} draw {} — build ok: {}; {} bytes [{}]; whole file compiles: {}; winner function compiles: {}; in-spec tests pass: {}{}",
                    config,
                    draw,
                    outcome.build_ok,
                    outcome.bytes,
                    outcome.distinct_key,
                    outcome.whole_file_compiles,
                    outcome.function_compiles,
                    outcome.tests_pass,
                    if outcome.detail.is_empty() {
                        String::new()
                    } else {
                        format!(" — {}", outcome.detail)
                    },
                ),
            );
            assert!(
                outcome.build_ok,
                "config {} draw {}: the binary did not build sort.morph end-to-end",
                config, draw
            );
            // The remaining per-draw outcomes (whole-file
            // compile, function compile, executed tests) are
            // RECORDED, not asserted: they are properties of
            // the drawn winner bodies — a synthesis-stage
            // property both binaries share — and the measured
            // control behavior does not support an every-draw
            // floor (see the ROAD note: junk draws include
            // non-compiling bodies, the recorded E0689 class,
            // and tpr does not discriminate for multi-param
            // specs). The bootstrap claim judged here is the
            // comparative one: the flag-on binary's draws are
            // the flag-off binary's draws in character, which
            // the totals below state.
            // The report is rewritten after each draw, so a
            // partial run stays informative (the M4 precedent).
            std::fs::write(
                format!("{}/behavioral-report.md", out_dir),
                &report,
            )
            .unwrap();
        }
        writelnr(
            &mut report,
            &format!(
                "- config {} totals — builds 5/5 expected: {}; distinct outputs: {}/5; whole-file compiles: {}/5; function compiles: {}/5; in-spec tests pass: {}/5",
                config,
                builds,
                distinct.len(),
                wholes,
                funcs,
                passes,
            ),
        );
        writelnr(&mut report, "");
        totals.push((config, builds, wholes, funcs, passes));
    }
    writelnr(
        &mut report,
        "The division of the roundtrip claim: the byte claim lives in report (a) at the dispatch level (a binary-level byte comparison on a fixed winner is impossible — each binary synthesizes its own winner — and is not faked); this report covers the binary level behaviorally. Per-draw in-spec test outcomes are RECORDED, not asserted: the executed-test criterion is a property of the synthesis stage both binaries share (see the ROAD note for the measured control result and its cause).",
    );
    std::fs::write(format!("{}/behavioral-report.md", out_dir), &report).unwrap();
    for (config, builds, wholes, funcs, passes) in &totals {
        println!(
            "M5S2-BEHAVIORAL|config={}|builds={}/5|whole-file-compiles={}/5|function-compiles={}/5|tests-pass={}/5",
            config, builds, wholes, funcs, passes
        );
    }
    println!("M5S2-BEHAVIORAL|DONE|both binaries built sort.morph end-to-end on every draw; per-draw outcomes in target/m5-s2/behavioral-report.md");
}
