// ─────────────────────────────────────────────────────────────
//  PROOF-CARRYING CODE (Phase 150, 1.4 — DISEBAR_SCOPE §4)
//
//  The mechanism is RE-VERIFICATION (decision D4a): the
//  evidence for a verdict is a bundle a consumer re-judges
//  with the shipped binary — not a Z3 proof object (the
//  pinned solver exposes no portable, independently
//  checkable proof format; §4.4) and not a signature over
//  a claim (a signed claim is still a claim). The Phase
//  148 audit measured the gap this closes: the manifest's
//  `verdictBasis` is caller-stated and printed verbatim
//  even for a body that fails its spec, because the crown
//  checks that would catch the lie run producer-side at
//  assembly and ship nowhere. Here they ship — as the
//  bundle's re-judgment, below.
//
//  THE BUNDLE (§4.1) — one function's evidence, as a JSON
//  file a stranger can hold:
//
//    format         "morphic-reverification-bundle"
//    bundleVersion  1 (a reader refuses other versions
//                   by name)
//    entry          the function's name
//    tier           "tier-1" | "tier-2" (EMBED_SCOPE §2.4)
//    verdictBasis   "z3-verified" | "executed-exact"
//    verdict        the claim the route certifies:
//                   "Verified" (Tier-1) | "ExecutedExact"
//                   (Tier-2)
//    crownRoute     "pipeline" | "encoder" — the judgment
//                   that crowned the body (EMBED_SCOPE R11
//                   routes); part of the evidence, because
//                   the two routes are different judgments
//                   and the consumer re-runs the named one
//    battery        the executed battery's standing as
//                   measured at production by the route's
//                   own executed half: {total, evaluable,
//                   passed} over the spec's in-spec tests
//    spec           the function's spec, in the AST's own
//                   serialization (the form the API and
//                   the MCP library tool already serve)
//    body           the body in the IR text serialization
//                   (synthesis::irtext, §4.2)
//
//  PRODUCTION is the checked route, as assembly is:
//  `produce_bundle` measures the claim by re-running the
//  crown (`reproduce`) — a body the named route does not
//  crown is refused, never labelled — and the CLI's
//  `build --bundles` first re-runs the whole crowning
//  surface (`codegen::package::verify_crowns`, the
//  assembly's own gate) before producing anything. The
//  bundles live in `packages/bundles/`, a sibling of both
//  packages: they are evidence FOR the packages' manifest
//  rows, not package contents — neither package's file
//  set includes them, so neither package's frozen budget
//  is charged for them (measured and stated in the ROAD
//  Phase 150 note).
//
//  CONSUMPTION (`verify_bundle_text`, the CLI's
//  `verify-bundle`) re-judges (spec, body) through the
//  named route and compares verdict and battery counts
//  against the claim. Every failure is a named refusal,
//  distinct per shape: an unsupported bundle version, an
//  incoherent claim triple, an unknown route, body text
//  the IR reader declines (its own named refusals,
//  version included), a claim that contradicts the
//  shipped manifest row for a shipped entry (tier /
//  verdict basis / crown route / the spec itself — the
//  claim is about THAT contract), a body the route does
//  not crown, a moved battery count. In a no-Z3 build a
//  Tier-1 claim is refused by name — it stands
//  unexamined, never passed (the PCC-2 posture); a
//  Tier-2 claim is re-judged on executed evidence, which
//  that build has.
//
//  THE STDLIB SELF-CHECK (§4.3) — `verify_library`, the
//  library function the CLI's `verify --library` calls:
//  every shipped entry's crown re-run from the shipped
//  data alone, through the assembly's own checks
//  (codegen::package's three), reported per entry. The
//  1.7 registry's install-time re-verification (D5a)
//  reuses this function and the bundle verbs — they are
//  built here, in the library, not as CLI-only logic.
//
//  Native builds only, like package assembly: the crown
//  checks read the embedded library data, which the
//  browser profile carries as a runtime asset instead —
//  the functions below are absent there, and the CLI
//  refuses by name (the --package precedent).
// ─────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

use crate::codegen::bindings::{BindingInput, CrownRoute};
use crate::spec::ast::FunctionSpec;
use crate::synthesis::engine::IRNode;

/// The bundle format's name, carried in every bundle.
pub const BUNDLE_FORMAT: &str = "morphic-reverification-bundle";

/// The bundle format's version. A reader refuses any
/// other version by name; the format changes only by
/// bumping this and the reader together.
pub const BUNDLE_VERSION: u32 = 1;

/// The bundles' repo home, relative to the repo root: a
/// sibling of `packages/npm` and `packages/rust`, beside
/// the manifests whose rows they evidence.
pub const BUNDLES_DIR: &str = "packages/bundles";

/// A bundle's battery claim: the executed standing of
/// the spec's in-spec battery, measured at production by
/// the crown route's own executed half — the counts a
/// consumer re-runs and compares, one by one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleBattery {
    /// Tests in the spec.
    pub total: usize,
    /// Tests the interpreter could bind and judge.
    pub evaluable: usize,
    /// Evaluable tests the body passed.
    pub passed: usize,
}

/// One function's re-verification bundle (DISEBAR_SCOPE
/// §4.1). Serde field names follow the manifest's
/// camelCase discipline where the vocabularies overlap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    /// [`BUNDLE_FORMAT`], verbatim.
    pub format: String,
    /// [`BUNDLE_VERSION`] at production.
    #[serde(rename = "bundleVersion")]
    pub bundle_version: u32,
    /// The function's name.
    pub entry: String,
    /// `"tier-1"` | `"tier-2"` (EMBED_SCOPE §2.4).
    pub tier: String,
    /// `"z3-verified"` | `"executed-exact"` (§2.4).
    #[serde(rename = "verdictBasis")]
    pub verdict_basis: String,
    /// The verdict the named route certifies for this
    /// body: `"Verified"` (Tier-1) | `"ExecutedExact"`
    /// (Tier-2).
    pub verdict: String,
    /// `"pipeline"` | `"encoder"` — the crown route the
    /// consumer re-runs (EMBED_SCOPE R11).
    #[serde(rename = "crownRoute")]
    pub crown_route: String,
    /// The battery claim (see [`BundleBattery`]).
    pub battery: BundleBattery,
    /// The spec, in the AST's own serialization.
    pub spec: FunctionSpec,
    /// The body, in the IR text serialization
    /// (`morphic-ir/1`).
    pub body: String,
}

/// One shipped entry's standing under `verify_library`.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryStanding {
    pub name: String,
    /// `"tier-1"` | `"tier-2"`.
    pub tier: String,
    /// `"pipeline"` | `"encoder"`.
    pub crown_route: String,
    /// Whether the crown reproduced in this build.
    pub reproduced: bool,
    /// The standing in words: the reproduction summary,
    /// or the check's refusal reason verbatim.
    pub detail: String,
}

/// A crown route's name in the bundle vocabulary.
fn route_name(route: CrownRoute) -> &'static str {
    match route {
        CrownRoute::Pipeline => "pipeline",
        CrownRoute::Encoder => "encoder",
    }
}

// ── The crown, re-run from (spec, body) alone ───────────

/// What a crown route certifies for one (spec, body):
/// the verdict it stands behind and the executed
/// battery's counts, measured by the route's own
/// executed half. This is the single judgment both
/// production (which measures the claim with it) and
/// consumption (which re-runs and compares) call — the
/// two can never drift into different standards.
#[cfg(not(feature = "wasm-playground"))]
struct Reproduction {
    verdict: &'static str,
    battery: BundleBattery,
}

#[cfg(not(feature = "wasm-playground"))]
fn reproduce(
    spec: &FunctionSpec,
    body: &IRNode,
    tier: &str,
    route: CrownRoute,
) -> Result<Reproduction, String> {
    match (tier, route) {
        ("tier-1", CrownRoute::Pipeline) => {
            let battery = pipeline_crown(spec, body)?;
            Ok(Reproduction { verdict: "Verified", battery })
        }
        ("tier-1", CrownRoute::Encoder) => {
            crate::codegen::package::check_encoder_crown(spec, body)?;
            Ok(Reproduction { verdict: "Verified", battery: executed_battery(spec, body) })
        }
        ("tier-2", _) => {
            crate::codegen::package::check_tier2_body(spec, body)?;
            Ok(Reproduction { verdict: "ExecutedExact", battery: executed_battery(spec, body) })
        }
        _ => Err(format!(
            "{}: no crown route judges a claim of tier '{tier}' \
             (tiers: tier-1, tier-2)",
            spec.name
        )),
    }
}

/// The executed half, as the encoder and Tier-2 checks
/// measure it: the shipped body judged on the spec's
/// in-spec battery through the index-carrying route
/// (Phase 135's judging route, over the prelude index).
#[cfg(not(feature = "wasm-playground"))]
fn executed_battery(spec: &FunctionSpec, body: &IRNode) -> BundleBattery {
    let index =
        crate::synthesis::engine::SpecIndex::from_specs(&crate::stdlib::prelude_specs());
    let run = crate::synthesis::eval::run_spec_tests_in(body, spec, &index);
    BundleBattery { total: spec.tests.len(), evaluable: run.evaluable, passed: run.passed }
}

/// The pipeline crown, re-run from (spec, body) alone:
/// the seeded pipeline (seed 42, the standing pin) over
/// the module the spec was crowned in, judged by the
/// assembly's own comparisons
/// (`codegen::package::judge_pipeline_report`).
#[cfg(not(feature = "wasm-playground"))]
fn pipeline_crown(spec: &FunctionSpec, body: &IRNode) -> Result<BundleBattery, String> {
    let functions = module_functions(spec)?;
    let report = crate::api::synthesize_seeded_functions(&functions, &spec.name, 42)
        .map_err(|e| format!("{}: the pipeline run failed: {e:?}", spec.name))?;
    if report.winner.body_debug != format!("{body:?}") {
        return Err(format!(
            "{}: the pipeline crowns a different body than the bundle carries \
             — the crowned body is not this one",
            spec.name
        ));
    }
    crate::codegen::package::judge_pipeline_report(&report, spec, body)?;
    Ok(BundleBattery {
        total: report.winner.tests_total,
        evaluable: report.winner.tests_evaluable,
        passed: report.winner.tests_passed,
    })
}

/// The module a spec was crowned in, reconstructed from
/// ASTs: for a shipped entry, its own library module
/// (parsed from the embedded sources) with the given
/// spec substituted for its namesake — so a bundle's
/// spec is judged as itself, never as the shipped
/// namesake it may have been tampered away from; for
/// any other spec, a one-spec module of its own (the
/// prelude joins through the index merge, as it does
/// for every module). The reconstruction is typechecked,
/// exactly as the text route's module is.
#[cfg(not(feature = "wasm-playground"))]
fn module_functions(spec: &FunctionSpec) -> Result<Vec<FunctionSpec>, String> {
    use crate::spec::{parser, typeck};
    for (_, source) in crate::stdlib::LIBRARY_MODULES {
        let module = parser::parse(source)
            .map_err(|e| format!("pcc: the embedded library parses: {e}"))?;
        if module.functions.iter().any(|f| f.name == spec.name) {
            let mut module = module;
            for f in module.functions.iter_mut() {
                if f.name == spec.name {
                    *f = spec.clone();
                }
            }
            let checked = typeck::check(module).map_err(|e| {
                format!("{}: the bundle's spec does not check in its module: {e}", spec.name)
            })?;
            return Ok(checked.spec.functions);
        }
    }
    let module = crate::spec::ast::Spec {
        name: "bundle".to_string(),
        imports: Vec::new(),
        functions: vec![spec.clone()],
        types: Vec::new(),
        invariants: Vec::new(),
    };
    let checked = typeck::check(module).map_err(|e| {
        format!("{}: the bundle's spec does not check: {e}", spec.name)
    })?;
    Ok(checked.spec.functions)
}

// ── Production (the checked route) ──────────────────────

/// Produce one entry's bundle: the claim inside is
/// MEASURED by re-running the crown (`reproduce`) before
/// anything is written — a body the named route does not
/// crown is refused here, by the route's own reason. The
/// battery counts are the reproduction's, the body text
/// is the IR serialization of the given body.
#[cfg(not(feature = "wasm-playground"))]
pub fn produce_bundle(input: &BindingInput) -> Result<Bundle, String> {
    let repro = reproduce(&input.spec, &input.body, &input.tier, input.crown_route)?;
    Ok(Bundle {
        format: BUNDLE_FORMAT.to_string(),
        bundle_version: BUNDLE_VERSION,
        entry: input.spec.name.clone(),
        tier: input.tier.clone(),
        verdict_basis: input.verdict_basis.clone(),
        verdict: repro.verdict.to_string(),
        crown_route: route_name(input.crown_route).to_string(),
        battery: repro.battery,
        spec: input.spec.clone(),
        body: crate::synthesis::irtext::write_ir(&input.body),
    })
}

/// A bundle as its file's bytes: pretty JSON, field
/// order as declared, one trailing newline. Deterministic
/// — two productions of the same entry are byte-identical
/// (pinned against the checked-in files).
#[cfg(not(feature = "wasm-playground"))]
pub fn bundle_json(bundle: &Bundle) -> String {
    let mut text =
        serde_json::to_string_pretty(bundle).expect("a Bundle always serializes");
    text.push('\n');
    text
}

/// Every shipped entry's bundle: (file name, contents)
/// for the whole library, in the frozen surface's order.
#[cfg(not(feature = "wasm-playground"))]
pub fn produce_all_bundles() -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for input in crate::codegen::bindings::stdlib_inputs() {
        let bundle = produce_bundle(&input)?;
        out.push((format!("{}.bundle.json", bundle.entry), bundle_json(&bundle)));
    }
    Ok(out)
}

/// The checked production — the CLI's route, mirroring
/// `assemble_npm_package_checked`: the whole crowning
/// surface re-runs first (the assembly's own checks over
/// the shipped data), and only then is any bundle
/// produced. A moved crown fails here and no bundle is
/// written.
#[cfg(not(feature = "wasm-playground"))]
pub fn produce_all_bundles_checked() -> Result<Vec<(String, String)>, String> {
    crate::codegen::package::verify_crowns().map_err(|e| e.to_string())?;
    produce_all_bundles()
}

/// A user module's bundles, produced by the verb: the
/// file set (`<entry>.bundle.json`, contents) in module
/// order, and the measured standings the admission
/// admitted them on.
#[cfg(not(feature = "wasm-playground"))]
pub struct UserBundleAssembly {
    pub files: Vec<(String, String)>,
    pub standings: Vec<crate::codegen::package::UserStanding>,
}

/// `morphic build --bundles-from`'s core (Phase 163,
/// DISEBAR_17_SCOPE §4): the `--package-from` admission
/// loop, shared verbatim (`admit_user_specs` — measured
/// rows only, the same refusal shapes), then one bundle
/// per admitted spec through `produce_bundle`'s checked
/// route over the retained inputs. Production is itself
/// a refusal surface: a spec whose constraints name a
/// sibling user spec is admitted in context but cannot
/// be bundled standalone (the format's self-containment
/// boundary — production re-judges (spec, body) against
/// the prelude alone), and a crown that moved between
/// the admission run and production refuses by the
/// route's own reason. Any refusal — admission's or
/// production's — fails the whole production and no
/// file set exists: the files live only in the `Ok`.
#[cfg(not(feature = "wasm-playground"))]
pub fn assemble_user_bundles(
    source: &str,
    seed: u64,
) -> Result<UserBundleAssembly, String> {
    let (inputs, standings) = crate::codegen::package::admit_user_specs(
        source,
        seed,
        "--bundles-from",
        "bundle production",
    )?;
    let mut files = Vec::new();
    for input in &inputs {
        let bundle = produce_bundle(input).map_err(|e| {
            format!("bundle production refused — nothing is written: {e}")
        })?;
        files.push((format!("{}.bundle.json", bundle.entry), bundle_json(&bundle)));
    }
    Ok(UserBundleAssembly { files, standings })
}

// ── Consumption (`verify-bundle`) ───────────────────────

/// Re-judge one bundle from its text: parse it, check
/// the claim's shape, re-run the crown through the named
/// route from (spec, body) alone, and compare the verdict
/// and every battery count against the claim. Ok carries
/// the one-line summary of what reproduced; every failure
/// is a named refusal (see the module header for the
/// shapes). Never panics on hostile input.
#[cfg(not(feature = "wasm-playground"))]
pub fn verify_bundle_text(text: &str) -> Result<String, String> {
    let bundle: Bundle = serde_json::from_str(text).map_err(|e| {
        format!("the bundle does not parse as a re-verification bundle: {e}")
    })?;
    verify_bundle(&bundle)
}

#[cfg(not(feature = "wasm-playground"))]
fn verify_bundle(bundle: &Bundle) -> Result<String, String> {
    let entry = &bundle.entry;
    if bundle.format != BUNDLE_FORMAT {
        return Err(format!(
            "not a Morphic re-verification bundle: its format is '{}', \
             expected '{BUNDLE_FORMAT}'",
            bundle.format
        ));
    }
    if bundle.bundle_version != BUNDLE_VERSION {
        return Err(format!(
            "unsupported bundle version {} — this build reads bundle version \
             {BUNDLE_VERSION}; the bundle's claim stands unexamined",
            bundle.bundle_version
        ));
    }
    // The claim triple must cohere: the format carries
    // exactly two claim shapes, and a verdict flipped on
    // its own (or a tier/basis pair that names no real
    // standing) is refused before any judging.
    let coherent = matches!(
        (bundle.tier.as_str(), bundle.verdict_basis.as_str(), bundle.verdict.as_str()),
        ("tier-1", "z3-verified", "Verified") | ("tier-2", "executed-exact", "ExecutedExact")
    );
    if !coherent {
        return Err(format!(
            "incoherent claim: tier '{}' with verdict basis '{}' and verdict \
             '{}' is not a claim shape the bundle format carries \
             (tier-1 / z3-verified / Verified, or tier-2 / executed-exact / \
             ExecutedExact)",
            bundle.tier, bundle.verdict_basis, bundle.verdict
        ));
    }
    let route = match bundle.crown_route.as_str() {
        "pipeline" => CrownRoute::Pipeline,
        "encoder" => CrownRoute::Encoder,
        other => {
            return Err(format!(
                "unknown crown route '{other}' (routes: pipeline, encoder)"
            ))
        }
    };
    // The body is read before it is judged: the IR
    // reader's refusals (version, tag, shape) are the
    // bundle's refusals, by name.
    let body = crate::synthesis::irtext::parse_ir(&bundle.body)?;
    // For a shipped entry, the claim is about the shipped
    // contract: the bundle's tier, basis, route and spec
    // must be the shipped row's, or the bundle evidences
    // something other than what the manifest states.
    if let Some(input) =
        crate::codegen::bindings::stdlib_inputs().into_iter().find(|i| i.spec.name == *entry)
    {
        if bundle.tier != input.tier {
            return Err(format!(
                "claim mismatch for shipped entry '{entry}': the bundle states \
                 tier '{}', but the shipped manifest row states '{}'",
                bundle.tier, input.tier
            ));
        }
        if bundle.verdict_basis != input.verdict_basis {
            return Err(format!(
                "claim mismatch for shipped entry '{entry}': the bundle states \
                 verdict basis '{}', but the shipped manifest row states '{}'",
                bundle.verdict_basis, input.verdict_basis
            ));
        }
        if route != input.crown_route {
            return Err(format!(
                "claim mismatch for shipped entry '{entry}': the bundle states \
                 crown route '{}', but the shipped row states '{}'",
                bundle.crown_route,
                route_name(input.crown_route)
            ));
        }
        let claimed_spec = serde_json::to_value(&bundle.spec)
            .map_err(|e| format!("{entry}: the bundle's spec does not serialize: {e}"))?;
        let shipped_spec = serde_json::to_value(&input.spec)
            .map_err(|e| format!("{entry}: the shipped spec does not serialize: {e}"))?;
        if claimed_spec != shipped_spec {
            return Err(format!(
                "spec mismatch for shipped entry '{entry}': the bundle's spec \
                 is not the shipped spec — its claim is about a different contract"
            ));
        }
    }
    // The no-Z3 posture (PCC-2): a Tier-1 claim in a
    // build without the verifier is refused by name. It
    // stands unexamined, never passed. A Tier-2 claim is
    // re-judged below on executed evidence, which this
    // build has.
    if bundle.tier == "tier-1" && !cfg!(feature = "z3-support") {
        return Err(format!(
            "{entry}: Tier-1 re-verification requires the Z3-enabled native \
             build — this build cannot re-judge a Z3 verdict, so the claim \
             stands unexamined, not passed"
        ));
    }
    let repro = reproduce(&bundle.spec, &body, &bundle.tier, route)?;
    if repro.verdict != bundle.verdict.as_str() {
        return Err(format!(
            "claimed verdict '{}' does not reproduce for '{entry}': the {} \
             route judges this body '{}'",
            bundle.verdict,
            bundle.crown_route,
            repro.verdict
        ));
    }
    for (field, claimed, actual) in [
        ("total", bundle.battery.total, repro.battery.total),
        ("evaluable", bundle.battery.evaluable, repro.battery.evaluable),
        ("passed", bundle.battery.passed, repro.battery.passed),
    ] {
        if claimed != actual {
            return Err(format!(
                "battery count moved for '{entry}': claimed {field} {claimed}, \
                 reproduced {actual} — the claim does not reproduce"
            ));
        }
    }
    Ok(format!(
        "{entry}: the claim reproduces — {} / {} / {} via the {} route; \
         battery {} passed of {} evaluable ({} in spec)",
        bundle.tier,
        bundle.verdict_basis,
        bundle.verdict,
        bundle.crown_route,
        repro.battery.passed,
        repro.battery.evaluable,
        repro.battery.total
    ))
}

// ── The stdlib self-check (`verify --library`) ──────────

/// Re-verify every shipped entry's crown from the
/// shipped data alone, through the assembly's own checks
/// (the pipeline crown over the entry's module, the
/// encoder crown, the Tier-2 executed check — the
/// dispatch of `codegen::package::verify_crowns`, made
/// per-entry and non-fatal so every standing is
/// reported). This is the library function the CLI's
/// `verify --library` verb calls, and the instrument the
/// 1.7 registry's install-time re-verification reuses.
#[cfg(not(feature = "wasm-playground"))]
pub fn verify_library() -> Result<Vec<EntryStanding>, String> {
    use crate::spec::parser;
    // Entry name → (module source, the module's own
    // spec): the library's modules, parsed once — the
    // construction of package::verify_crowns.
    let mut by_name: std::collections::HashMap<String, (&str, FunctionSpec)> =
        std::collections::HashMap::new();
    for (_, source) in crate::stdlib::LIBRARY_MODULES {
        let module = parser::parse(source)
            .map_err(|e| format!("verify --library: the library parses: {e}"))?;
        for spec in module.functions {
            by_name.insert(spec.name.clone(), (source, spec));
        }
    }
    let mut standings = Vec::new();
    for input in crate::codegen::bindings::stdlib_inputs() {
        let (source, spec) = by_name.get(&input.spec.name).ok_or_else(|| {
            format!("verify --library: {} is not a library entry", input.spec.name)
        })?;
        let checked = if input.tier == "tier-1" {
            match input.crown_route {
                CrownRoute::Pipeline => {
                    crate::codegen::package::check_tier1_crown(source, spec, &input.body)
                }
                CrownRoute::Encoder => {
                    crate::codegen::package::check_encoder_crown(spec, &input.body)
                }
            }
        } else {
            crate::codegen::package::check_tier2_body(spec, &input.body)
        };
        standings.push(match checked {
            Ok(()) => EntryStanding {
                name: input.spec.name.clone(),
                tier: input.tier.clone(),
                crown_route: route_name(input.crown_route).to_string(),
                reproduced: true,
                detail: "the crown reproduces from the shipped data".to_string(),
            },
            Err(reason) => EntryStanding {
                name: input.spec.name.clone(),
                tier: input.tier.clone(),
                crown_route: route_name(input.crown_route).to_string(),
                reproduced: false,
                detail: reason,
            },
        });
    }
    Ok(standings)
}
