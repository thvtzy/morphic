//! Phase 162 — the 1.7 registry (a) scope audit probes:
//! the rung re-founded at Phase 148 §3 / DISEBAR_SCOPE D5a
//! as **prelude bundles with install-time
//! re-verification**, measured against the tree as it
//! stands at v1.6.0. Every claim in DISEBAR_17_AUDIT.md
//! carries one of this file's `P162|` lines or a
//! file/line citation.
//!
//! What is measured, in order:
//! 1. The bundle machinery's inventory: the 35 shipped
//!    bundles on disk, their masses, and the per-bundle
//!    consumer verb (`verify_bundle_text`) re-judging a
//!    checked-in bundle file — the exact call an install
//!    step would make.
//! 2. `verify_library()`'s actual scope: it takes no
//!    argument and iterates the shipped library only —
//!    the Phase 150 header's "1.7 reuses this function"
//!    claim, verified against its signature: the reusable
//!    unit is the per-bundle verb, not the self-check.
//! 3. The production seam: the Phase 154 verb's own
//!    admission winner (via the same API call
//!    `assemble_user_package` makes), carried into
//!    `pcc::produce_bundle` exactly as package.rs builds
//!    its `BindingInput` — a user bundle is produced and
//!    re-verified TODAY, with zero new production code.
//!    The gap is the CLI arm and the assembly discarding
//!    its inputs, nothing else.
//! 4. The tamper shape on a user bundle (the install
//!    flow's refusal vocabulary is the standing PCC one).
//! 5. The format's self-containment boundary: a spec
//!    whose constraints name a sibling user spec is
//!    refused at bundle production on the Tier-1 route —
//!    production re-judges (spec, body) standalone
//!    against the prelude, and the one-spec module
//!    reconstruction does not check.
//! 6. The prelude funnel the registry must extend: two
//!    functions (`stdlib::prelude_specs`,
//!    `stdlib::shipped_body`) feed the typechecker's
//!    signature merge, the SpecIndex body registry and
//!    codegen's emission; the collision rule (user wins,
//!    stdlib never silently shadowed by the merge); and
//!    the standing absence of any runtime directory or
//!    environment input to the prelude.
//!
//! Crown-heavy probes serialize on one lock (the Phase
//! 150 discipline: the engine's RNG is process-global).

#![cfg(not(feature = "wasm-playground"))]

use std::sync::Mutex;

use morphic::codegen::bindings::{BindingInput, CrownRoute};
use morphic::pcc;
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::SpecIndex;

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// The Phase 154 exhibit module (tests/phase154_package_verb.rs
/// MODULE_A), verbatim: one Tier-1 spec, one Tier-2 spec,
/// one higher-order Tier-1 spec under the Z3 discipline.
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
/// first — the self-containment boundary's exhibit.
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

/// The admission rule of `assemble_user_package`
/// (src/codegen/package.rs), restated over the verb's own
/// API call — the (tier, verdict_basis) a measured winner
/// carries, or None when the verb would refuse the spec.
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

/// Produce one exhibit spec's bundle through the seam an
/// install/catalogue production route would use: the
/// verb's winner, the BindingInput package.rs builds,
/// `pcc::produce_bundle`. Returns the bundle JSON text.
fn produce_exhibit_bundle(source: &str, name: &str) -> String {
    let (report, winner) = morphic::api::synthesize_seeded_spec_winner(source, name, 42)
        .unwrap_or_else(|e| panic!("{name}: the seeded run runs: {e:?}"));
    let (tier, basis) = admission(&report).unwrap_or_else(|| {
        panic!(
            "{name}: the verb's admission takes the winner (status {}, {}/{})",
            report.winner.verification_status,
            report.winner.tests_passed,
            report.winner.tests_evaluable
        )
    });
    let checked = typeck::check(parser::parse(source).expect("the module parses"))
        .expect("the module checks");
    let spec = checked
        .spec
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap()
        .clone();
    let input = BindingInput {
        spec,
        body: winner.body.clone(),
        tier: tier.to_string(),
        verdict_basis: basis.to_string(),
        battery_pinned: report.winner.tests_passed,
        battery_silent: 0,
        crown_route: CrownRoute::Pipeline,
    };
    let bundle = pcc::produce_bundle(&input)
        .unwrap_or_else(|e| panic!("{name}: production re-runs the crown: {e}"));
    println!(
        "P162|produce|{name} tier={} basis={} verdict={} battery={}/{}/{}",
        bundle.tier,
        bundle.verdict_basis,
        bundle.verdict,
        bundle.battery.passed,
        bundle.battery.evaluable,
        bundle.battery.total
    );
    pcc::bundle_json(&bundle)
}

// ── 1. The shipped bundles: inventory + the consumer verb ──

#[test]
fn p162_shipped_bundles_inventory_and_consumer_verb() {
    let _guard = crown_serial();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(pcc::BUNDLES_DIR);
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the bundles directory reads")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".bundle.json"))
        .collect();
    files.sort_by_key(|e| e.file_name());
    assert_eq!(files.len(), 35, "35 shipped bundles, one per library entry");
    let mut total = 0u64;
    let mut min = u64::MAX;
    let mut max = 0u64;
    for f in &files {
        let n = f.metadata().unwrap().len();
        total += n;
        min = min.min(n);
        max = max.max(n);
    }
    println!(
        "P162|bundles|count=35 total={total} min={min} max={max} mean={}",
        total / 35
    );
    // The consumer verb on a checked-in file — the exact
    // call an install step makes on fetched bytes.
    let text = std::fs::read_to_string(dir.join("int_abs.bundle.json"))
        .expect("a shipped bundle file reads");
    match pcc::verify_bundle_text(&text) {
        Ok(summary) => println!("P162|verify-file|int_abs ok: {summary}"),
        Err(e) => {
            // In a no-Z3 build a Tier-1 file is refused by
            // name (the standing PCC posture) — that IS the
            // measured behaviour, reported not asserted.
            assert!(
                !cfg!(feature = "z3-support") && e.contains("unexamined"),
                "a Z3 build re-verifies the shipped file: {e}"
            );
            println!("P162|verify-file|int_abs refused-by-name (no-Z3): {e}");
        }
    }
}

// ── 2. verify_library's actual scope ─────────────────────

#[test]
fn p162_verify_library_scope_is_the_shipped_library_only() {
    let _guard = crown_serial();
    let standings = pcc::verify_library().expect("the self-check runs natively");
    assert_eq!(standings.len(), 35, "one standing per shipped entry");
    let reproduced = standings.iter().filter(|s| s.reproduced).count();
    // In a no-Z3 build the Tier-1 crowns cannot reproduce
    // (they stand unexamined); the Tier-2 standings do.
    if cfg!(feature = "z3-support") {
        assert_eq!(reproduced, 35, "every shipped crown reproduces");
    }
    println!(
        "P162|verify-library|standings=35 reproduced={reproduced} \
         signature=verify_library() -> Vec<EntryStanding> (no bundle parameter; \
         iterates bindings::stdlib_inputs only)"
    );
}

// ── 3. The production seam: the verb's winner, bundled ───

#[test]
fn p162_package_from_winner_bundles_and_reverifies() {
    let _guard = crown_serial();
    for name in ["is_widget_code", "weights_sorted"] {
        let text = produce_exhibit_bundle(WIDGET_MODULE, name);
        match pcc::verify_bundle_text(&text) {
            Ok(summary) => println!("P162|seam|{name} re-verifies: {summary}"),
            Err(e) => {
                // The one admissible failure: a Tier-1
                // claim in a no-Z3 build is refused by
                // name — but this probe's admission derives
                // tiers from the run itself, which in a
                // no-Z3 build yields Tier-2 for both
                // exhibits (the Phase 154 standing), so any
                // refusal here is a real seam failure.
                panic!("{name}: the produced bundle re-verifies: {e}");
            }
        }
    }
    println!(
        "P162|seam|the --package-from admission winner bundles via \
         pcc::produce_bundle with zero new production code; the gap is \
         the CLI arm + assemble_user_package discarding its Vec<BindingInput>"
    );
}

// ── 4. The tamper shape on a user bundle ─────────────────

#[test]
fn p162_user_bundle_tamper_refused_by_name() {
    let _guard = crown_serial();
    let text = produce_exhibit_bundle(WIDGET_MODULE, "weights_sorted");
    let tampered = text.replacen(
        "\"verdict\": \"ExecutedExact\"",
        "\"verdict\": \"Verified\"",
        1,
    );
    assert_ne!(tampered, text, "the tamper actually moves the claim");
    let err = pcc::verify_bundle_text(&tampered)
        .expect_err("a flipped verdict claim is refused");
    assert!(err.contains("incoherent claim"), "{err}");
    println!("P162|tamper|flipped verdict refused: {err}");
}

// ── 5. The self-containment boundary (Tier-1 route) ──────

#[cfg(feature = "z3-support")]
#[test]
fn p162_sibling_calling_spec_refused_at_production() {
    let _guard = crown_serial();
    // The full module checks and crowns in context…
    let (report, winner) =
        morphic::api::synthesize_seeded_spec_winner(SIBLING_MODULE, "quadruple_it", 42)
            .expect("the seeded run runs over the full module");
    let (tier, basis) = admission(&report).expect("the winner is admitted in context");
    assert_eq!(tier, "tier-1", "in context the crown stands Verified");
    let checked = typeck::check(parser::parse(SIBLING_MODULE).unwrap())
        .expect("the full module checks");
    let spec = checked
        .spec
        .functions
        .iter()
        .find(|f| f.name == "quadruple_it")
        .unwrap()
        .clone();
    let input = BindingInput {
        spec,
        body: winner.body.clone(),
        tier: tier.to_string(),
        verdict_basis: basis.to_string(),
        battery_pinned: report.winner.tests_passed,
        battery_silent: 0,
        crown_route: CrownRoute::Pipeline,
    };
    // …but production re-judges (spec, body) standalone
    // against the prelude, and the one-spec reconstruction
    // does not check: the sibling is not there.
    let err = pcc::produce_bundle(&input)
        .expect_err("a sibling-naming spec cannot be bundled standalone");
    assert!(err.contains("does not check"), "{err}");
    println!("P162|boundary|sibling-naming spec refused at production: {err}");
}

// ── 6. The prelude funnel + the collision rule ───────────

#[test]
fn p162_prelude_funnel_and_collision_rule() {
    let specs = morphic::stdlib::prelude_specs();
    assert_eq!(specs.len(), 35, "the prelude carries the 35 shipped specs");
    let with_bodies = specs
        .iter()
        .filter(|s| morphic::stdlib::shipped_body(&s.name).is_some())
        .count();
    assert_eq!(with_bodies, 35, "every prelude spec carries a shipped body");
    let index = SpecIndex::from_specs(&[]);
    let int_abs = index.get("int_abs").expect("the index merges the prelude");
    assert!(
        index.callee_body("int_abs").is_some(),
        "a prelude name denotes its shipped body in the index"
    );
    println!(
        "P162|funnel|prelude_specs=35 shipped_bodies=35; typeck, SpecIndex \
         and codegen all read through stdlib::{{prelude_specs, shipped_body}}; \
         int_abs doc={:?}",
        int_abs.doc
    );

    // The collision rule, measured at the merge itself: a
    // module defining int_abs keeps its own — the prelude
    // entry is not merged at all.
    let source = r#"Mine {
spec int_abs {
    doc: "the module's own int_abs, deliberately different"
    input: x: Int
    output: Int
    constraint: output == x
    test { input: 3 expect: 3 }
}
}"#;
    let checked = typeck::check(parser::parse(source).unwrap())
        .expect("a module may define a prelude name");
    let index = SpecIndex::from_module(&checked.spec);
    let own = index.get("int_abs").expect("the module's own spec stands");
    assert!(
        own.doc.as_deref().unwrap_or("").contains("deliberately different"),
        "the module's own definition wins the name"
    );
    let shipped = morphic::stdlib::shipped_body("int_abs").unwrap();
    let denoted = index.callee_body("int_abs").expect("the name denotes");
    assert_ne!(
        format!("{denoted:?}"),
        format!("{shipped:?}"),
        "the colliding name does NOT denote the shipped body"
    );
    assert!(
        index.get("int_clamp").is_some(),
        "non-colliding prelude names still merge beside it"
    );
    println!(
        "P162|collision|user definition wins: the prelude entry of a \
         colliding name is not merged (typeck registers user sigs last; \
         SpecIndex::merge_prelude skips module names)"
    );
}

// ── 7. No runtime input reaches the prelude today ────────

#[test]
fn p162_no_directory_or_env_input_to_the_prelude() {
    // Plant the two plausible discovery inputs — an env
    // var naming a directory, and a bundle file inside it
    // carrying a fake entry name — and measure that the
    // prelude does not move. (No product code reads either;
    // the audit's grep evidence is in the document. This
    // probe pins the standing so the 1.7 build's discovery
    // layer is a visible change, not a silent one.)
    let dir = std::env::temp_dir().join("morphic-p162-planted-registry");
    std::fs::create_dir_all(&dir).unwrap();
    let a_bundle = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(pcc::BUNDLES_DIR)
            .join("int_abs.bundle.json"),
    )
    .unwrap()
    .replacen("\"entry\": \"int_abs\"", "\"entry\": \"zzz_planted\"", 1);
    std::fs::write(dir.join("zzz_planted.bundle.json"), &a_bundle).unwrap();
    std::env::set_var("MORPHIC_REGISTRY_DIR", &dir);
    std::env::set_var("MORPHIC_HOME", &dir);
    let specs = morphic::stdlib::prelude_specs();
    assert_eq!(specs.len(), 35, "the prelude is exactly the compiled-in library");
    assert!(
        specs.iter().all(|s| s.name != "zzz_planted"),
        "nothing planted becomes a prelude spec"
    );
    let index = SpecIndex::from_specs(&[]);
    assert!(index.get("zzz_planted").is_none(), "nor an index entry");
    std::env::remove_var("MORPHIC_REGISTRY_DIR");
    std::env::remove_var("MORPHIC_HOME");
    println!(
        "P162|discovery|planted dir + env vars move nothing: the prelude is \
         compiled-in (include_str! + a transcribed body registry); installed \
         bundles need a new runtime layer at the funnel"
    );
}
