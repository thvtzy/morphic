//! Phase 163 — the 1.7 freeze's production arm:
//! `morphic build --bundles-from <file.morph>`
//! (DISEBAR_17_SCOPE §4, decision D3a). The arm is the
//! `--package-from` admission loop — shared verbatim
//! through `codegen::package::admit_user_specs` —
//! emitting one PCC bundle per admitted spec through
//! `pcc::produce_bundle` over the retained inputs.
//!
//! Pins, in order:
//! 1. the freeze's shape (D1a–D8a ratified under the
//!    owner's delegation, lockstep, the closing line
//!    verbatim, the frozen surfaces named);
//! 2. the arm end-to-end on the audit's exhibit module
//!    (the Phase 154 widget pair at their measured
//!    standings; every produced bundle re-verifying
//!    through the consumer verb);
//! 3. determinism: two productions are byte-identical;
//! 4. the self-containment boundary at the arm: on the
//!    Tier-1 route (Z3 builds) a spec whose constraints
//!    name a sibling refuses the whole production by
//!    name, nothing written (the file set exists only
//!    in the Ok); a no-Z3 build has no Tier-1 route and
//!    the same module bundles at tier-2 instead (§4.3);
//! 5. a moved crown refuses at production: an admitted
//!    spec carrying another spec's crowned body is
//!    refused by the route's own judgment;
//! 6. the CLI surface: the arm end-to-end through the
//!    shipped binary, the combination refusals, the
//!    positional-input refusal, the sibling posture at
//!    the CLI (refusal by name under Z3, with no bundle
//!    file written; tier-2 production without Z3);
//! 7. the help surface: the arm's flags present, the
//!    claim flags that do not exist still absent.
//!
//! Crown-heavy pins serialize on one lock (the Phase
//! 150 discipline: the engine's RNG is process-global).

#![cfg(not(feature = "wasm-playground"))]

use std::process::Command;
use std::sync::Mutex;

use morphic::codegen::bindings::{BindingInput, CrownRoute};
use morphic::pcc;
use morphic::spec::{parser, typeck};

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// The Phase 154 exhibit module, verbatim (the form
/// the Phase 162 probes carried): one Tier-1 spec, one
/// Tier-2 spec.
const WIDGET_MODULE: &str = r#"WidgetMath {
spec is_widget_code {
    doc: "A widget code is known when it names one of the six widget kinds."
    input: code: Int
    output: Bool
    constraint: output == (code>=1&&code<=6)
    test "first" { input: 1 expect: true }
    test "last" { input: 6 expect: true }
    test "zero" { input: 0 expect: false }
    test "above" { input: 7 expect: false }
    test "far" { input: 99 expect: false }
}
spec weights_sorted {
    doc: "weights_sorted(xs) = true when the weights are in non-decreasing order."
    input: xs: List<Int>
    output: Bool
    constraint: len(xs) <= 1 implies output == true
    constraint: forall i: Int in range(0, len(xs) - 1):
        output == true implies xs[i] <= xs[i + 1]
    constraint: forall i: Int in range(0, len(xs) - 1):
        xs[i] > xs[i + 1] implies output == false
    test "empty" { input: []; expect: true }
    test "single" { input: [5]; expect: true }
    test "sorted" { input: [1, 2, 3]; expect: true }
    test "unsorted" { input: [3, 1, 2]; expect: false }
    test "equal" { input: [5, 5, 5]; expect: true }
}
}"#;

/// A module whose second spec's constraints name the
/// first — the self-containment boundary's exhibit
/// (the Phase 162 probe's module, verbatim).
const SIBLING_MODULE: &str = r#"Sibling {
spec double_it {
    doc: "double_it(x) = twice x."
    input: x: Int
    output: Int
    constraint: output == x * 2
    test { input: 3 expect: 6 }
    test { input: 0 expect: 0 }
}
spec quadruple_it {
    doc: "quadruple_it(x) = double_it applied twice over, stated through its sibling."
    input: x: Int
    output: Int
    constraint: output == double_it(x) * 2
    test { input: 3 expect: 12 }
    test { input: 0 expect: 0 }
}
}"#;

/// The admission rule of the shared loop, restated (the
/// Phase 162 probe's helper): the (tier, verdict_basis)
/// a measured winner carries, or None when refused.
fn admission(
    report: &morphic::api::SynthesisReport,
) -> Option<(&'static str, &'static str)> {
    let w = &report.winner;
    if w.verification_status == "Verified" && w.tests_passed == w.tests_evaluable {
        Some(("tier-1", "z3-verified"))
    } else if w.tests_passed == w.tests_evaluable
        && w.tests_evaluable == w.tests_total
        && w.tests_total > 0
    {
        Some(("tier-2", "executed-exact"))
    } else {
        None
    }
}

/// One exhibit spec's retained input, built exactly as
/// the shared admission loop builds it.
fn winner_input(source: &str, name: &str) -> BindingInput {
    let (report, winner) = morphic::api::synthesize_seeded_spec_winner(source, name, 42)
        .unwrap_or_else(|e| panic!("{name}: the seeded run runs: {e:?}"));
    let (tier, basis) = admission(&report)
        .unwrap_or_else(|| panic!("{name}: the admission takes the winner"));
    let checked = typeck::check(parser::parse(source).expect("the module parses"))
        .expect("the module checks");
    let spec = checked
        .spec
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap()
        .clone();
    BindingInput {
        spec,
        body: winner.body.clone(),
        tier: tier.to_string(),
        verdict_basis: basis.to_string(),
        battery_pinned: report.winner.tests_passed,
        battery_silent: 0,
        crown_route: CrownRoute::Pipeline,
    }
}

// ── 1. The freeze's shape ───────────────────────────────

#[test]
fn p163_freeze_shape() {
    let scope = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("DISEBAR_17_SCOPE.md"),
    )
    .expect("the freeze exists");
    let flat: String = scope.split_whitespace().collect::<Vec<_>>().join(" ");
    for needle in [
        "D1a", "D2a", "D3a", "D4a", "D5a", "D6a", "D7a", "D8a",
        "ratified by Muse under the owner's delegation",
        "lockstep 1.7.0",
        "user > stdlib > installed",
        "MORPHIC_REGISTRY_DIR",
        "raw.githubusercontent.com/thvtzy/morphic",
        "registry/index.json",
        "--bundles-from",
        "sha2",
    ] {
        assert!(flat.contains(needle), "the freeze carries {needle}");
    }
    assert!(
        flat.contains(
            "\"1.7 closes when anyone can publish a proven function and \
             no one has to take its word for it — the install re-runs \
             the proof, the receipt records what the re-run measured, \
             and a name stands in the prelude because it was verified, \
             not because it was fetched.\""
        ),
        "the closing line stands frozen verbatim"
    );
    println!("P163|freeze|D1a-D8a under delegation, lockstep 1.7.0, closing line verbatim");
}

// ── 2. The arm on the exhibit module ────────────────────

#[test]
fn p163_arm_produces_the_widget_pair_at_measured_standings() {
    let _guard = crown_serial();
    let assembly = pcc::assemble_user_bundles(WIDGET_MODULE, 42)
        .expect("the exhibit module's production runs");
    let names: Vec<&str> = assembly.files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        ["is_widget_code.bundle.json", "weights_sorted.bundle.json"],
        "one bundle per admitted spec, in module order"
    );
    assert_eq!(assembly.standings.len(), 2);
    let widget = &assembly.standings[0];
    assert_eq!(widget.name, "is_widget_code");
    if cfg!(feature = "z3-support") {
        assert_eq!(
            (widget.tier.as_str(), widget.verdict_basis.as_str(), widget.status.as_str()),
            ("tier-1", "z3-verified", "Verified"),
            "the Z3 build's measured standing for the widget exhibit"
        );
    } else {
        assert_eq!(
            (widget.tier.as_str(), widget.verdict_basis.as_str()),
            ("tier-2", "executed-exact"),
            "a no-Z3 build measures the same winner executed-exact"
        );
    }
    assert_eq!((widget.passed, widget.evaluable, widget.total), (5, 5, 5));
    let weights = &assembly.standings[1];
    assert_eq!(
        (weights.name.as_str(), weights.tier.as_str(), weights.verdict_basis.as_str()),
        ("weights_sorted", "tier-2", "executed-exact"),
        "weights_sorted stands Tier-2 in every build"
    );
    assert_eq!((weights.passed, weights.evaluable, weights.total), (5, 5, 5));

    for (file_name, text) in &assembly.files {
        let bundle: serde_json::Value =
            serde_json::from_str(text).expect("a produced bundle parses");
        assert_eq!(bundle["format"], "morphic-reverification-bundle");
        assert_eq!(bundle["bundleVersion"], 1, "the format stands at version 1");
        let summary = pcc::verify_bundle_text(text)
            .unwrap_or_else(|e| panic!("{file_name}: the produced bundle re-verifies: {e}"));
        println!("P163|produce|{file_name} re-verifies: {summary}");
    }
    println!(
        "P163|arm|is_widget_code {} / {} battery {}/{}/{}; weights_sorted tier-2 \
         battery {}/{}/{}",
        widget.tier,
        widget.verdict_basis,
        widget.passed,
        widget.evaluable,
        widget.total,
        weights.passed,
        weights.evaluable,
        weights.total
    );
}

// ── 3. Determinism ──────────────────────────────────────

#[test]
fn p163_arm_production_is_deterministic() {
    let _guard = crown_serial();
    let first = pcc::assemble_user_bundles(WIDGET_MODULE, 42).expect("first run");
    let second = pcc::assemble_user_bundles(WIDGET_MODULE, 42).expect("second run");
    assert_eq!(
        first.files, second.files,
        "two productions of the same module at the same seed are byte-identical"
    );
    println!("P163|determinism|two runs byte-identical ({} files)", first.files.len());
}

// ── 4. The self-containment boundary at the arm ─────────

#[test]
fn p163_sibling_naming_spec_refuses_the_production_by_name() {
    let _guard = crown_serial();
    if cfg!(feature = "z3-support") {
        // The boundary is the Tier-1 route's: standalone
        // production re-checks the spec against the prelude,
        // the sibling is not there, and the whole production
        // refuses by name (DISEBAR_17_SCOPE §4).
        let err = match pcc::assemble_user_bundles(SIBLING_MODULE, 42) {
            Ok(_) => panic!("a sibling-naming spec cannot be bundled standalone"),
            Err(e) => e,
        };
        assert!(
            err.contains("quadruple_it"),
            "the refusal names the spec: {err}"
        );
        assert!(
            err.contains("nothing is written"),
            "the refusal carries the discipline: {err}"
        );
        assert!(
            err.contains("does not check"),
            "the Tier-1 route's boundary shape: {err}"
        );
        println!("P163|boundary|sibling-naming spec refused at the arm: {err}");
    } else {
        // A no-Z3 build has no Tier-1 route at all: the same
        // module measures by execution alone and bundles at
        // tier-2, whose claim (execution only) reproduces in
        // any build — measured at the CLI during the phase
        // and pinned here so the posture is stated, not
        // silent. No tier-1 claim can be produced in this
        // build, so the boundary has nothing to bind.
        let assembly = pcc::assemble_user_bundles(SIBLING_MODULE, 42)
            .expect("the no-Z3 arm measures the module by execution");
        assert_eq!(assembly.files.len(), 2, "both specs bundle at tier-2");
        for (name, text) in &assembly.files {
            let bundle: serde_json::Value = serde_json::from_str(text)
                .unwrap_or_else(|e| panic!("{name}: the bundle parses: {e}"));
            assert_eq!(
                bundle["tier"], "tier-2",
                "{name}: the no-Z3 standing is tier-2"
            );
            assert_eq!(bundle["verdictBasis"], "executed-exact");
            pcc::verify_bundle_text(text)
                .unwrap_or_else(|e| panic!("{name}: the tier-2 claim reproduces: {e}"));
        }
        println!(
            "P163|boundary|no-Z3: no Tier-1 route — the sibling module \
             bundles at tier-2 (executed-exact) and reproduces"
        );
    }
}

// ── 5. A moved crown refuses at production ──────────────

#[test]
fn p163_moved_crown_refuses_at_production() {
    let _guard = crown_serial();
    let widget = winner_input(WIDGET_MODULE, "is_widget_code");
    let weights = winner_input(WIDGET_MODULE, "weights_sorted");
    let mut crossed = widget.clone();
    crossed.body = weights.body.clone();
    let err = pcc::produce_bundle(&crossed)
        .expect_err("a body the route does not crown for this spec is refused");
    assert!(err.contains("is_widget_code"), "the refusal names the entry: {err}");
    if cfg!(feature = "z3-support") {
        assert!(
            err.contains("different body"),
            "the pipeline route's moved-crown shape: {err}"
        );
    }
    println!("P163|moved-crown|crossed body refused at production: {err}");
}

// ── 6. The CLI surface ──────────────────────────────────

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("morphic-p163-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn p163_cli_arm_end_to_end_and_refusals() {
    let _guard = crown_serial();
    let dir = scratch_dir("cli");
    let module = dir.join("widget_math.morph");
    std::fs::write(&module, WIDGET_MODULE).unwrap();
    let out_dir = dir.join("produced");
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args(["build", "--bundles-from"])
        .arg(&module)
        .args(["--out"])
        .arg(&out_dir)
        .output()
        .expect("the binary runs");
    assert!(
        out.status.success(),
        "the CLI production succeeds: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    for name in ["is_widget_code", "weights_sorted"] {
        assert!(stdout.contains(name), "the CLI reports the standing: {name}");
        let file = out_dir.join(format!("{name}.bundle.json"));
        assert!(file.exists(), "the bundle file is written: {}", file.display());
    }
    // The CLI's bytes are the library route's bytes (the
    // same seed, the same checked production).
    let assembly = pcc::assemble_user_bundles(WIDGET_MODULE, 42).expect("API run");
    for (name, contents) in &assembly.files {
        let on_disk = std::fs::read_to_string(out_dir.join(name)).unwrap();
        assert_eq!(&on_disk, contents, "CLI and API productions agree byte-for-byte: {name}");
    }
    // The consumer verb over a CLI-produced file.
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args(["verify-bundle"])
        .arg(out_dir.join("weights_sorted.bundle.json"))
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "verify-bundle accepts the produced file");

    // Combinations refuse by name.
    for extra in [&["--package"][..], &["--bundles"][..], &["--package-from", "x.morph"][..]] {
        let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
            .args(["build", "--bundles-from"])
            .arg(&module)
            .args(extra)
            .output()
            .expect("the binary runs");
        assert!(!out.status.success(), "--bundles-from {extra:?} refuses");
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(
            stderr.contains("--bundles-from") && stderr.contains("does not combine"),
            "the combination refusal names the arm: {stderr}"
        );
    }
    // A positional input beside the arm refuses by name.
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args(["build", "--bundles-from"])
        .arg(&module)
        .arg(&module)
        .output()
        .expect("the binary runs");
    assert!(!out.status.success(), "a positional input refuses");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        stderr.contains("takes no positional input file"),
        "the positional refusal: {stderr}"
    );

    // The sibling module at the CLI: in a Z3 build the
    // production refuses by name and no bundle file is
    // written anywhere; in a no-Z3 build there is no
    // Tier-1 route to refuse on, and the module produces
    // tier-2 bundles by execution (the §4 posture).
    let sib = dir.join("sibling.morph");
    std::fs::write(&sib, SIBLING_MODULE).unwrap();
    let sib_out = dir.join("sib-produced");
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args(["build", "--bundles-from"])
        .arg(&sib)
        .args(["--out"])
        .arg(&sib_out)
        .output()
        .expect("the binary runs");
    if cfg!(feature = "z3-support") {
        assert!(!out.status.success(), "the sibling production refuses at the CLI");
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(stderr.contains("quadruple_it"), "the CLI refusal names the spec: {stderr}");
        let written = sib_out.exists()
            && std::fs::read_dir(&sib_out)
                .unwrap()
                .filter_map(|e| e.ok())
                .any(|e| e.file_name().to_string_lossy().ends_with(".bundle.json"));
        assert!(!written, "a refused production writes no bundle file");
        println!("P163|cli|arm end-to-end; combinations + positional + sibling refuse by name");
    } else {
        assert!(
            out.status.success(),
            "the no-Z3 CLI production measures by execution: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        for name in ["double_it", "quadruple_it"] {
            assert!(
                sib_out.join(format!("{name}.bundle.json")).exists(),
                "the no-Z3 production writes the tier-2 bundle: {name}"
            );
        }
        println!("P163|cli|arm end-to-end; combinations + positional refuse by name; sibling produces at tier-2 (no-Z3)");
    }
}

// ── 7. The help surface ─────────────────────────────────

#[test]
fn p163_help_surface_carries_the_arm_and_no_claim_flags() {
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args(["build", "--help"])
        .output()
        .expect("the binary runs");
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout).to_string();
    for flag in ["--bundles-from", "--package-from", "--seed", "--out"] {
        assert!(help.contains(flag), "the help carries {flag}");
    }
    for claim in ["--tier", "--verdict", "--basis", "--battery"] {
        assert!(
            !help.contains(claim),
            "no caller-stated-claims flag exists: {claim} must be absent"
        );
    }
    println!("P163|help|--bundles-from present; no claim-shaped flag exists");
}
