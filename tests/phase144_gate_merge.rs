//! Phase 144 — the 1.3 gate run's own instrument: the
//! re-tier standings, the stretch judgment, and the
//! package surfaces, pinned on the gate tree.
//!
//! What this file pins, by gate clause
//! (LIST_ENCODER_SCOPE §6):
//!
//! - **Part 0 / clause 1 — the stretch.**
//!   `list_binary_search` stays Tier-2, and the reason
//!   is measured here, not asserted: its `require:`
//!   names `list_is_sorted(xs)`, whose contract is
//!   implication/forall-shaped — it has no
//!   expression-level denotation (`SpecIndex::
//!   callee_body` yields nothing for it), so the
//!   Phase 109 precondition machinery cannot encode
//!   the require, the assumptions decline, and the
//!   crown route returns Inconclusive for ANY body.
//!   The isolation pin below judges a trivial spec
//!   carrying only that require against a trivially
//!   true body: Inconclusive — against two controls
//!   (no require; an arithmetic require) that Verify,
//!   proving the harness and the assumption machinery
//!   work and the spec-call require is the decliner.
//!   R5's second condition fails, so the stretch is
//!   not reachable at 1.3; the gate does not notice
//!   (it was never an obligation).
//! - **Clause 1 — the fifteen at their standings.**
//!   Each of the fifteen entries crowned at Phases
//!   141–143 stands Verified ∧ battery-exact with its
//!   shipped body on this tree (the per-phase crown
//!   evidence stands in tests/phase141_quantifiers.rs,
//!   phase142_fold_form.rs, phase143_uf_composition.rs;
//!   this pin is the gate's single re-run over the
//!   set). `list_sort` stands Inconclusive — the
//!   standing residue (§3.10 Predicate row + the
//!   two-binder row), unchanged.
//! - **Clause 4 — the re-tier surfaces.** The
//!   generator's frozen rows carry tier-1 /
//!   z3-verified for exactly the fifteen (and the
//!   encoder crown route for exactly them); the two
//!   non-movers carry tier-2 / executed-exact; the
//!   generated manifest agrees row for row; each
//!   moved entry's `doc:` tier sentence is the new
//!   Tier-1 sentence and each non-mover's names its
//!   residue. (Function-byte identity against the
//!   1.2.0 assemblies and the mass measurements are
//!   the gate record's evidence, taken at assembly
//!   time in the ROAD Phase 144 note.)
//!
//! In the no-Z3 build the verdicts are the honest
//! subset (Inconclusive throughout); the surface pins
//! are configuration-independent.

use morphic::codegen::bindings::{self, CrownRoute};
use morphic::spec::ast::FunctionSpec;
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, IRNode, Provenance, ScoreBreakdown, SpecIndex,
};
use morphic::synthesis::eval;
use morphic::verify::verdict::VerificationStatus;
use morphic::verify::verifier::Verifier;

/// This binary's tests serialize on one lock. The
/// surfaces test's binding generation re-runs all 35
/// crown checks while the clause-1 test judges the
/// fifteen — on this box (1.5 shared vCPU) the two
/// racing solver loads flipped a fold crown to
/// Inconclusive twice at the 1.3.0 stamp run (the
/// recorded load class of Phases 142–143: heavy crowns
/// flip under parallel test load, each passing solo).
/// The lock removes the intra-binary race; no
/// assertion is weakened, and the gate evidence in
/// the ROAD note rests on solo runs regardless. The
/// guard is recovered from poisoning
/// (`unwrap_or_else(into_inner)`): if one test's
/// crown flips under ambient load and panics, the
/// others must still stand on their own assertions,
/// not on a PoisonError — measured at the stamp,
/// where one flip cascaded to two poison failures.
static GATE_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 144,
        body,
        spec_name: spec.name.clone(),
        spec: spec.clone(),
        score: 0.0,
        scores: ScoreBreakdown {
            test_pass_ratio: 0.0,
            constraint_score: 0.0,
            complexity_score: 0.0,
            quality_score: 0.0,
            synthesis_time_us: 0,
        },
        generation: 0,
        provenance: Provenance::HumanGuided,
        verified: false,
        verification_status: None,
    }
}

fn spec_of(specs: &[FunctionSpec], name: &str) -> FunctionSpec {
    specs
        .iter()
        .find(|s| s.name == name)
        .expect("entry ships")
        .clone()
}

/// The fifteen entries the campaign crowned (Phases
/// 141–143), in LIST_ENCODER_SCOPE §3's groups.
const FIFTEEN: &[&str] = &[
    // §3.1 — the element model (Phase 141, W3)
    "list_reverse",
    "list_contains",
    "list_min",
    "list_max",
    "list_take",
    "list_drop",
    "list_is_sorted",
    "range",
    // §3.2 — the fold-form arm (Phase 142, W4)
    "list_sum",
    "list_product",
    // §3.3 — UF composition (Phase 143, W5; list_fold
    // jointly with W4)
    "list_map",
    "list_filter",
    "list_fold",
    "list_all",
    "list_any",
];

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase144_clause1_the_fifteen_verified_and_battery_exact() {
    let _serial = GATE_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);
    let index = SpecIndex::from_specs(&specs);
    for name in FIFTEEN {
        let spec = spec_of(&specs, name);
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let run = eval::run_spec_tests_in(&body, &spec, &index);
        println!("P144|clause1|{name}|battery={}/{}", run.passed, run.evaluable);
        assert_eq!(run.passed, run.evaluable, "{name}: battery-exact");
        assert!(run.evaluable > 0, "{name}: battery is non-empty");
        let result = verifier.verify_one(&candidate(&spec, body), &spec);
        println!("P144|clause1|{name}|verify={:?}", result.status);
        #[cfg(feature = "z3-support")]
        {
            assert_eq!(result.status, VerificationStatus::Verified, "{name}");
            assert!(
                result.constraint_results.iter().all(|r| r.holds),
                "{name}: every postcondition row Proven"
            );
        }
        #[cfg(not(feature = "z3-support"))]
        assert_eq!(result.status, VerificationStatus::Inconclusive, "{name}");
    }
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase144_clause1_residue_and_stretch_stand_inconclusive() {
    let _serial = GATE_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // list_sort: the standing residue. list_binary_search:
    // the stretch, judged at Part 0 — its require is not
    // assumable in the crown route (the isolation pin
    // below), so its shipped body stands Inconclusive
    // here as it must: the verdict is the require's
    // decline, not the body's failure (its executed
    // standing is 10/10 in-spec, pinned at Phase 115).
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);
    for name in ["list_sort", "list_binary_search"] {
        let spec = spec_of(&specs, name);
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let status = verifier.verify_one(&candidate(&spec, body), &spec).status;
        println!("P144|clause1|{name}|verify={status:?}");
        assert_eq!(status, VerificationStatus::Inconclusive, "{name}");
    }
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase144_part0_the_require_is_not_assumable_in_the_crown_route() {
    let _serial = GATE_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // The stretch's blocker, isolated: a spec whose
    // only content is `require: list_is_sorted(xs)`
    // over a trivially true row and body. If the
    // require were assumable (Phase 109 machinery),
    // the verdict would be Verified; it stands
    // Inconclusive because the require declines to
    // encode — list_is_sorted's contract is
    // implication/forall rows, no equation or
    // guard-table denotation exists for it, so
    // `callee_body` yields nothing the constraint
    // encoder can inline, and precondition assumptions
    // decline (never silently assumed-true).
    let with_require = r#"Probe { spec probe_req {
        input: xs: List<Int>
        output: Bool
        require: list_is_sorted(xs)
        constraint: output == true
        test "t" { input: [1, 2] expect: true }
    } }"#;
    let no_require = r#"Probe { spec probe_req {
        input: xs: List<Int>
        output: Bool
        constraint: output == true
        test "t" { input: [1, 2] expect: true }
    } }"#;
    let arith_require = r#"Probe { spec probe_req {
        input: xs: List<Int>
        output: Bool
        require: len(xs) >= 1
        constraint: output == true
        test "t" { input: [1, 2] expect: true }
    } }"#;
    let body = IRNode::Literal(morphic::synthesis::engine::IRLiteral::Bool(true));
    for (tag, source) in [
        ("spec_call_require", with_require),
        ("no_require", no_require),
        ("arith_require", arith_require),
    ] {
        let module = parser::parse(source).expect("probe source parses");
        let checked = typeck::check(module).expect("probe source typechecks");
        let spec = checked
            .spec
            .functions
            .into_iter()
            .next()
            .expect("one spec");
        let mut specs = stdlib::prelude_specs();
        specs.push(spec.clone());
        let verifier = Verifier::with_specs(&specs);
        let status = verifier.verify_one(&candidate(&spec, body.clone()), &spec).status;
        println!("P144|part0|{tag}|verify={status:?}");
        #[cfg(feature = "z3-support")]
        match tag {
            "spec_call_require" => {
                assert_eq!(status, VerificationStatus::Inconclusive, "{tag}")
            }
            _ => assert_eq!(status, VerificationStatus::Verified, "{tag}"),
        }
        #[cfg(not(feature = "z3-support"))]
        assert_eq!(status, VerificationStatus::Inconclusive, "{tag}");
    }
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase144_clause4_retier_surfaces() {
    let _serial = GATE_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // The generator's inputs: exactly the fifteen at
    // tier-1 / z3-verified on the encoder crown route;
    // the two non-movers at tier-2 / executed-exact;
    // the eighteen standing Tier-1 entries on the
    // pipeline route, unmoved.
    let inputs = bindings::stdlib_inputs();
    assert_eq!(inputs.len(), 35, "35 binding inputs");
    let mut tier1 = 0;
    for input in &inputs {
        let name = input.spec.name.as_str();
        if FIFTEEN.contains(&name) {
            assert_eq!(input.tier, "tier-1", "{name}: re-tiered");
            assert_eq!(input.verdict_basis, "z3-verified", "{name}: basis");
            assert_eq!(
                input.crown_route,
                CrownRoute::Encoder,
                "{name}: the encoder crown route"
            );
        } else if name == "list_sort" || name == "list_binary_search" {
            assert_eq!(input.tier, "tier-2", "{name}: stays Tier-2");
            assert_eq!(input.verdict_basis, "executed-exact", "{name}: basis");
        } else {
            assert_eq!(input.tier, "tier-1", "{name}: standing Tier-1");
            assert_eq!(
                input.crown_route,
                CrownRoute::Pipeline,
                "{name}: the pipeline crown route, unmoved"
            );
        }
        if input.tier == "tier-1" {
            tier1 += 1;
        }
    }
    assert_eq!(tier1, 33, "33 of 35 at Tier-1 after the re-tier");
    println!("P144|clause4|tier1=33/35");

    // The generated manifest agrees row for row.
    let set = bindings::stdlib_bindings().expect("stdlib bindings generate");
    for row in &set.manifest.exports {
        let input = inputs
            .iter()
            .find(|i| i.spec.name == row.name)
            .expect("manifest row has an input");
        assert_eq!(row.tier, input.tier, "{}: manifest tier", row.name);
        assert_eq!(
            row.verdict_basis, input.verdict_basis,
            "{}: manifest basis",
            row.name
        );
        // The doc travels verbatim from the entry's spec.
        assert_eq!(
            row.doc,
            input.spec.doc.as_deref().unwrap_or(""),
            "{}: doc verbatim",
            row.name
        );
    }

    // The tier sentences on the entries themselves:
    // the fifteen carry the Tier-1 sentence (filter:
    // its exactly-two-rows form); the two non-movers
    // name their residues.
    let specs = stdlib::prelude_specs();
    for name in FIFTEEN {
        let doc = spec_of(&specs, name).doc.expect("entry carries a doc");
        assert!(
            doc.contains("Tier-1: Z3-verified"),
            "{name}: Tier-1 tier sentence: {doc}"
        );
        println!("P144|clause4|{name}|doc-tier=Tier-1");
    }
    let sort_doc = spec_of(&specs, "list_sort").doc.expect("doc");
    assert!(
        sort_doc.contains("Tier-2: executed-exact")
            && sort_doc.contains("is_permutation Predicate")
            && sort_doc.contains("two-binder forall"),
        "list_sort: residue named in its tier sentence: {sort_doc}"
    );
    let bs_doc = spec_of(&specs, "list_binary_search").doc.expect("doc");
    assert!(
        bs_doc.contains("Tier-2: executed-exact")
            && bs_doc.contains("not assumable in the crown route"),
        "list_binary_search: residue named in its tier sentence: {bs_doc}"
    );
    println!("P144|clause4|residues named on both non-movers");
}
