// ─────────────────────────────────────────────────────────────
//  NPM PACKAGE ASSEMBLY (Phase 122, 1.1 Embeddable —
//  EMBED_SCOPE §5, decisions D2a/D8a, resolutions
//  R5/R6/R7)
//
//  The assembly: Phase 121's binding set (the assembled
//  ESM module + `.d.ts` + `morphic.manifest.json`)
//  becomes the publishable npm package `@thvtzy/morphic`
//  in its repo home, `packages/npm/` — plus the furniture
//  a registry package carries (`package.json`, a README,
//  the project's two license texts, byte-copies of the
//  repo root's) and one per-function module per export
//  (§2.2: every entry is also importable from its own
//  subpath). The per-function modules are re-export shims
//  over the root module, so a subpath import denotes
//  exactly the root export — there is one generated body
//  per entry, never a second transcription.
//
//  THE FILE SET IS THE ASSEMBLY: `assemble_npm_package`
//  returns every file (path, contents) deterministically
//  — two assemblies are byte-identical (pinned). The
//  checked-in package in `packages/npm/` is this set,
//  written by `morphic build --package`; a pin holds the
//  checked-in files byte-equal to a fresh assembly, so
//  the two cannot drift (the Phase 112 asset discipline).
//
//  REGENERATION DISCIPLINE (gate clause 4's machinery):
//  the checked assembly — `assemble_npm_package_checked`,
//  what the CLI runs — FIRST re-runs the crowning surface
//  in-process. For each Tier-1 entry, its crown is
//  re-verified by the route that produced it (the
//  entry's `CrownRoute`, bindings.rs): the seeded
//  production pipeline (seed 42, the Phase 110 pin's own
//  call) must crown exactly the shipped body, Verified
//  and golden-exact — or, for the fifteen entries the
//  1.3 list-encoder campaign crowned (Phase 144), the
//  shipped body must Verify through `verify_one` over
//  the prelude index, every row Proven, battery-exact;
//  for each Tier-2 entry, the shipped
//  body must stand executed-exact on its full in-spec
//  battery through the interpreter (the in-process half
//  of the Phase 115 pin; its compiled-route half stands
//  in the suite). A moved crown fails the assembly by
//  name and nothing is written: the verdicts the package
//  claims are re-measured at assembly time, never
//  inherited from a stale transcription.
//
//  VERSIONING (R6): the package version is the crate's
//  version — `env!("CARGO_PKG_VERSION")`, the source the
//  manifest's `packageVersion`/`morphicVersion` already
//  carry — stamped into `package.json` by the assembly.
//  On this tree that is 1.0.0; Phase 125's release bumps
//  the crate to 1.1.0 and re-runs the assembly, so the
//  release is a stamp (one version edit + one
//  regeneration), not a rebuild of the package.
//
//  BUDGET (§4, clause 6's npm half): the package's
//  unpacked mass — the sum of the file set's bytes, the
//  same files `npm publish --dry-run` measures — is
//  frozen at 79,872 B; a pin holds the assembly inside
//  it and the CLI refuses to write an over-budget
//  package.
//
//  DRY-RUN ONLY (R5): nothing here publishes, logs in or
//  touches a credential. The registry publishes are
//  Phase 125's release acts, after the gate.
// ─────────────────────────────────────────────────────────────

use crate::spec::ast::FunctionSpec;
use crate::synthesis::engine::IRNode;
use crate::synthesis::eval;

use super::bindings::{self, BindingSet};
use super::CodegenError;

/// The package's repo home, relative to the repo root.
pub const NPM_PACKAGE_DIR: &str = "packages/npm";

/// The frozen npm budget: the assembled package's
/// unpacked mass must not exceed this. EMBED_SCOPE §4
/// froze 79,872 B for the 29-export package;
/// STDLIB_SCOPE.md Amendment 2 §A2.5 re-derived the
/// figure from measured actuals for the 35-export
/// package — 76,800 B, deliberately below its
/// predecessor — and the re-derived figure is the one
/// that binds from 1.2.
pub const NPM_UNPACKED_BUDGET: usize = 76_800;

/// One file of the assembled package: its path inside
/// the package and its full contents.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageFile {
    pub path: String,
    pub contents: String,
}

/// The package README: the factual surface — what the
/// package is, the export list with its tiers, the type
/// table's named losses in brief, the lockstep. (The
/// timed quickstart is the embedding guide's, Phase 125;
/// it does not live here.)
const PACKAGE_README: &str = r#"# @thvtzy/morphic

Morphic's standard library as ready-to-import functions
for JavaScript and TypeScript hosts. Every function in
this package was generated from a Morphic specification
by the Morphic compiler's native pipeline — the pipeline
that judged it — and ships with the standing that
pipeline produced, recorded per function in
`morphic.manifest.json` (name, frozen signature, tier,
verdict basis, battery counts, and the function's own
documentation, verbatim).

This package is an artifact, not a compiler: it contains
no synthesizer and makes no runtime verification claims.
The functions are plain, environment-neutral ES modules
(BigInt, ES2020, is the only versioned requirement).

## Embedding guide

The [embedding guide](https://github.com/thvtzy/morphic/blob/master/EMBEDDING.md)
takes a host from install to a typed call: the
five-minute quickstart, the import shapes, the full
type table with its named losses, and what the tiers
mean at the boundary.

## The functions

Thirty-five exports under their Morphic entry names.
The root module exports all of them; each is also
importable from its own subpath (for example
`@thvtzy/morphic/int_clamp`).

Tier 1 — Z3-verified against the function's
specification, and golden-exact on its frozen battery:

- Int: `int_abs`, `int_sign`, `int_min`, `int_max`,
  `int_clamp`, `int_is_even`, `int_is_odd`
- Char: `char_is_digit`, `char_is_alpha`,
  `char_is_whitespace`, `char_is_uppercase`,
  `char_is_lowercase`, `char_to_digit`
- String: `string_is_empty`
- Result / Option: `result_unwrap_or`,
  `option_unwrap_or`, `option_ok_or`
- List: `list_is_empty`, `list_reverse`,
  `list_contains`, `list_sum`, `list_product`,
  `list_min`, `list_max`, `list_take`, `list_drop`,
  `list_is_sorted`, `list_map`, `list_filter`,
  `list_fold`, `list_all`, `list_any`, `range`

Fifteen of the List entries upgraded at 1.3, when
the list-element encoder landed (the 1.3 list-encoder
campaign): their labels upgraded in place, exactly as
the tier mechanism always stated — the functions'
bytes did not change.

Tier 2 — executed-exact on the full frozen battery, the
tier printed here and in the manifest (the tier is a
property of the proof standard, not of the behavior;
each function is held to 100% agreement on every
battery point):

- List: `list_sort`, `list_binary_search`

The two carry named residues, stated in their
manifest rows: `list_sort`'s contract opens with the
`is_permutation` predicate form, which the encoder
does not carry, and closes with a two-binder
quantifier past its envelope; `list_binary_search`'s
`require:` names a quantifier-shaped contract the
crown route cannot assume.

The last six Tier-1 List entries are the 1.2
amendment's entries (STDLIB_SCOPE.md Amendment 2).
Five of them are
higher-order: they take a function argument, and at
runtime any JavaScript function of the right shape
answers — including this package's own exports
(`int_abs`, `int_is_even`, …). In the TypeScript
declarations a function-typed parameter is typed
`never` in 1.2: no callable TypeScript function type
is frozen, and the manifest's `signature` field
carries the true Morphic signature, so the boundary
is stated in the package's own surface.

`list_binary_search` is a first-match scan: its contract
is search semantics — an index naming the target, or
`null` — and that disclosure travels with the function
(see its manifest row). With duplicate targets, any
valid index satisfies the contract.

The builtins `len` and `int_to_str` are not exports:
they are language builtins, rendered inside generated
code. A host uses its native equivalent — for `len`, an
array's element count or a string's code-point count;
for `int_to_str`, `String(x)`.

## Types at the boundary

- Morphic `Int` is TypeScript `bigint` — exact inside
  the i64 domain [-2^63, 2^63 - 1]. The functions carry
  no runtime precondition checks: a host value outside
  that domain is out-of-domain input, outside the
  battery-exactness promise. BigInt `/` truncates
  toward zero and `%` takes the dividend's sign,
  matching Morphic Int semantics.
- `Char` is a `string` of exactly one Unicode code
  point (an astral character has `.length` 2; the
  contract is the code point, not the length).
- Morphic counts string length in code points; JS
  `.length` counts UTF-16 code units. Never use
  `.length` where a Morphic length is meant.
- `Option<T>` is `T | null`. A nested
  `Option<Option<T>>` would flatten under this form;
  no export carries a nested-Option signature.
- `Result<T, E>` is the discriminated union
  `{ tag: "Ok", value: T } | { tag: "Err", error: E }`.

Preconditions frozen in the specifications (for
example: `int_abs` excludes the Int minimum;
`int_clamp` requires `lo <= hi`; `list_binary_search`
requires a sorted list) are stated in each function's
JSDoc and stand as contract.

## Versioning

This package versions in lockstep with Morphic itself:
the package version is the Morphic version whose
pipeline generated and verified the contents. The
manifest records it as both `packageVersion` and
`morphicVersion`; the two are equal at any release.

## Provenance

Generated — do not edit. The package is assembled by
`morphic build --package` from the standard library's
shipped bodies; assembly re-runs the crowning checks
and refuses to write if a crown has moved.

The manifest's tier and verdict-basis rows are claims,
and their evidence ships as re-verification bundles:
one bundle per entry — its spec, its body in the
morphic-ir text serialization, the claimed verdict,
the executed battery, and the crown route that judged
it — in the Morphic repository at `packages/bundles/`,
produced by `morphic build --bundles`. Any holder of
the Morphic binary can re-judge a bundle from its
(spec, body) alone with `morphic verify-bundle
<file>`, or re-verify every shipped crown at once with
`morphic verify --library`; a claim that does not
reproduce is refused by name.

## License

Dual-licensed under MIT OR Apache-2.0, as Morphic
itself: see LICENSE-MIT and LICENSE-APACHE.
"#;

/// The `package.json` description of the stdlib package.
const STDLIB_DESCRIPTION: &str = "Morphic's verified standard library as ready-to-import functions: pre-generated from the specifications the Morphic pipeline verified (Tier-1) or proved executed-exact (Tier-2). Per-function tiers and batteries: morphic.manifest.json.";

/// The `package.json` furniture that does NOT derive
/// from the binding set (Phase 154: the shell generalized
/// for the user-package verb — the set-derived parts,
/// name / version / exports map, were never
/// stdlib-specific; this furniture was). The stdlib
/// package's furniture states Morphic's own facts about
/// Morphic's package (its license, its project links); a
/// user package assembled by `build --package-from`
/// carries the author's facts instead — no license field
/// and no project links are asserted on their behalf.
pub struct PackageFurniture {
    /// The `description` field's text.
    pub description: String,
    /// The `license` field's value, when the package
    /// carries one.
    pub license_field: Option<String>,
    /// License files riding the package — (file name,
    /// contents), in `files`-listing order.
    pub licenses: Vec<(String, String)>,
    /// Whether the project-links block (repository /
    /// homepage / keywords — the Morphic project's own
    /// facts) is emitted.
    pub project_links: bool,
}

/// The stdlib package's furniture (Phase 122's shape,
/// unchanged): Morphic's description of its own package,
/// its dual license, its project links.
fn stdlib_furniture() -> PackageFurniture {
    PackageFurniture {
        description: STDLIB_DESCRIPTION.to_string(),
        license_field: Some("MIT OR Apache-2.0".to_string()),
        licenses: vec![
            (
                "LICENSE-MIT".to_string(),
                include_str!("../../LICENSE-MIT").to_string(),
            ),
            (
                "LICENSE-APACHE".to_string(),
                include_str!("../../LICENSE-APACHE").to_string(),
            ),
        ],
        project_links: true,
    }
}

/// The `package.json`: ESM-only (R7), the exports map
/// naming the root module, every per-function subpath
/// (§2.2), the manifest and the package file itself;
/// `files` bounds the tarball to the package's own
/// files. The README rides npm's always-included set;
/// the two license texts are listed explicitly —
/// measured at this phase's dry-run, npm's always-
/// include does not match the suffixed dual-license
/// names (LICENSE-MIT / LICENSE-APACHE), and without
/// the listing the tarball silently carried neither.
/// The version is the manifest's
/// — the crate's — so one source stamps both (R6).
fn package_json(set: &BindingSet, furniture: &PackageFurniture) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"name\": \"{}\",\n", set.manifest.package));
    out.push_str(&format!(
        "  \"version\": \"{}\",\n",
        set.manifest.package_version
    ));
    out.push_str(&format!("  \"description\": \"{}\",\n", furniture.description));
    out.push_str("  \"type\": \"module\",\n");
    out.push_str("  \"main\": \"./index.js\",\n");
    out.push_str("  \"types\": \"./index.d.ts\",\n");
    out.push_str("  \"exports\": {\n");
    out.push_str(
        "    \".\": {\n      \"types\": \"./index.d.ts\",\n      \"import\": \"./index.js\"\n    },\n",
    );
    for e in &set.manifest.exports {
        out.push_str(&format!(
            "    \"./{0}\": {{\n      \"types\": \"./{0}.d.ts\",\n      \"import\": \"./{0}.js\"\n    }},\n",
            e.name
        ));
    }
    out.push_str("    \"./morphic.manifest.json\": \"./morphic.manifest.json\",\n");
    out.push_str("    \"./package.json\": \"./package.json\"\n");
    out.push_str("  },\n");
    out.push_str("  \"files\": [\n    \"*.js\",\n    \"*.d.ts\",\n    \"morphic.manifest.json\"");
    for (name, _) in &furniture.licenses {
        out.push_str(&format!(",\n    \"{name}\""));
    }
    out.push_str("\n  ],\n");
    out.push_str("  \"sideEffects\": false");
    if let Some(license) = &furniture.license_field {
        out.push_str(&format!(",\n  \"license\": \"{license}\""));
    }
    if furniture.project_links {
        out.push_str(
            ",\n  \"repository\": {\n    \"type\": \"git\",\n    \"url\": \"git+https://github.com/thvtzy/morphic.git\"\n  },\n",
        );
        out.push_str("  \"homepage\": \"https://thvtzy.github.io/morphic/\",\n");
        out.push_str(
            "  \"keywords\": [\"morphic\", \"verified\", \"formal-verification\", \"standard-library\"]",
        );
    }
    out.push_str("\n}\n");
    out
}

/// The per-function module (§2.2): a re-export shim, so
/// the subpath denotes exactly the root export.
fn per_function_shim(name: &str) -> String {
    format!("export {{ {name} }} from \"./index.js\";\n")
}

/// The package shell (Phase 154): a binding set +
/// README + furniture becomes the standing npm layout —
/// `package.json`, the README, any license files, the
/// root module + declarations + manifest, and one
/// per-function module per export. Pure generation, no
/// filesystem contact. The stdlib assembly and the
/// user-package verb are this one shell over different
/// inputs — the layout cannot drift between them.
pub fn assemble_package_files(
    set: &BindingSet,
    readme: &str,
    furniture: &PackageFurniture,
) -> Vec<PackageFile> {
    let mut files: Vec<PackageFile> = Vec::new();
    let mut push = |path: &str, contents: String| {
        files.push(PackageFile {
            path: path.to_string(),
            contents,
        })
    };
    push("package.json", package_json(set, furniture));
    push("README.md", readme.to_string());
    for (name, contents) in &furniture.licenses {
        push(name, contents.clone());
    }
    push("index.js", set.js_module.clone());
    push("index.d.ts", set.declarations.clone());
    push("morphic.manifest.json", set.manifest_json.clone());
    for e in &set.manifest.exports {
        push(&format!("{}.js", e.name), per_function_shim(&e.name));
        push(&format!("{}.d.ts", e.name), per_function_shim(&e.name));
    }
    files
}

/// Assemble the npm package: the full file set, in a
/// stable order, generated from the shipped bodies via
/// Phase 121's binding set. Pure generation — no
/// crowning re-verification (that is
/// `assemble_npm_package_checked`, the CLI's route) and
/// no filesystem contact.
pub fn assemble_npm_package() -> Result<Vec<PackageFile>, CodegenError> {
    let set = bindings::stdlib_bindings()?;
    Ok(assemble_package_files(
        &set,
        PACKAGE_README,
        &stdlib_furniture(),
    ))
}

/// The package's unpacked mass: the file set's total
/// bytes — the quantity `npm publish --dry-run` reports
/// as the unpacked size of the same files, and the
/// quantity §4's budget binds.
pub fn unpacked_mass(files: &[PackageFile]) -> usize {
    files.iter().map(|f| f.contents.len()).sum()
}

// ── The crowning re-verification (clause 4) ─────────────
//
// Native builds only: the library's module sources are
// the embedded form (under `wasm-playground` the data
// arrives as a runtime asset, and package assembly —
// like the stdlib-asset generator — is a native-build
// act; the CLI refuses there by name).

/// One Tier-1 entry's crown check — the Phase 110 pin's
/// own procedure, factored for the assembly: the seeded
/// production pipeline (seed 42) over the entry's
/// library module must crown exactly the shipped body,
/// Verified, golden-exact on its evaluable in-spec
/// tests. The `Err` carries the reason, for the
/// assembly's refusal message.
#[cfg(not(feature = "wasm-playground"))]
pub fn check_tier1_crown(
    module_source: &str,
    spec: &FunctionSpec,
    shipped: &IRNode,
) -> Result<(), String> {
    let name = &spec.name;
    let report = crate::api::synthesize_seeded_spec(module_source, name, 42)
        .map_err(|e| format!("{name}: the pipeline run failed: {e:?}"))?;
    judge_pipeline_report(&report, spec, shipped)
}

/// The pipeline crown's judgment over one seeded run's
/// report: the crowned body is exactly the shipped body,
/// Verified, golden-exact on its evaluable tests. Factored
/// out of `check_tier1_crown` at Phase 150 so the bundle
/// re-verification (src/pcc.rs) applies the assembly's own
/// comparisons to a run it drives over ASTs — one judgment,
/// two drivers, no drift between them.
#[cfg(not(feature = "wasm-playground"))]
pub fn judge_pipeline_report(
    report: &crate::api::SynthesisReport,
    spec: &FunctionSpec,
    shipped: &IRNode,
) -> Result<(), String> {
    let name = &spec.name;
    if report.winner.body_debug != format!("{shipped:?}") {
        return Err(format!(
            "{name}: the pipeline crown has moved — the shipped body is no longer \
             the body the pipeline crowns"
        ));
    }
    if report.winner.verification_status != "Verified" {
        return Err(format!(
            "{name}: the crowned body is not Verified in this build (status: {}) — \
             package assembly requires the Z3-enabled native build",
            report.winner.verification_status
        ));
    }
    if report.winner.tests_passed != report.winner.tests_evaluable {
        return Err(format!(
            "{name}: the crowned body is not golden-exact on its evaluable tests \
             ({}/{})",
            report.winner.tests_passed, report.winner.tests_evaluable
        ));
    }
    Ok(())
}

/// One encoder-crowned Tier-1 entry's crown check
/// (Phase 144, the 1.3 re-tier): the crown these
/// entries hold is the one Phases 141–143 measured —
/// the shipped hand body, judged by `verify_one` over
/// the prelude index, Verified with every postcondition
/// row Proven, and battery-exact on its in-spec tests
/// through the index-carrying route (the executed half,
/// shared with the Tier-2 check). The seeded synthesis
/// pipeline is NOT this crown's route: its pool holds
/// no candidate for these specs and it crowns a
/// different, unverified body (measured at Phase 144),
/// so the Phase 110 procedure cannot re-verify them.
/// The `Err` carries the reason, for the assembly's
/// refusal message.
#[cfg(not(feature = "wasm-playground"))]
pub fn check_encoder_crown(spec: &FunctionSpec, shipped: &IRNode) -> Result<(), String> {
    let name = &spec.name;
    let specs = crate::stdlib::prelude_specs();
    let verifier = crate::verify::verifier::Verifier::with_specs(&specs);
    let candidate = crate::synthesis::engine::CandidateImplementation {
        id: 0,
        body: shipped.clone(),
        spec_name: name.clone(),
        spec: spec.clone(),
        score: 0.0,
        scores: crate::synthesis::engine::ScoreBreakdown {
            test_pass_ratio: 0.0,
            constraint_score: 0.0,
            complexity_score: 0.0,
            quality_score: 0.0,
            synthesis_time_us: 0,
        },
        generation: 0,
        provenance: crate::synthesis::engine::Provenance::HumanGuided,
        verified: false,
        verification_status: None,
    };
    let result = verifier.verify_one(&candidate, spec);
    if result.status != crate::verify::verdict::VerificationStatus::Verified {
        return Err(format!(
            "{name}: the shipped body is not Verified in this build (status: {:?}) — \
             package assembly requires the Z3-enabled native build",
            result.status
        ));
    }
    if !result.constraint_results.iter().all(|r| r.holds) {
        return Err(format!(
            "{name}: not every postcondition row is Proven for the shipped body"
        ));
    }
    check_tier2_body(spec, shipped).map_err(|e| format!("{name}: executed half: {e}"))?;
    Ok(())
}

/// One Tier-2 entry's executed check — the in-process
/// half of the Phase 115 pin: the shipped body stands
/// executed-exact on its full in-spec battery, every
/// point evaluable (no spec-silent points at this tier).
///
/// Phase 135 (Amendment 2 §A2.4.3, Phase 132's recorded
/// hand-off): the judging route is the INDEX-CARRYING
/// one (`run_spec_tests_in` over the prelude-merged
/// SpecIndex), so an entry whose in-spec tests supply a
/// function by named reference (the Amendment 2
/// entries) binds here — on the context-free route those
/// rows were Unbindable and no such entry could ever
/// verify. Non-mover, measured at Phase 135: the 29
/// standing entries' outcomes are identical on both
/// routes (their points carry no named supply), pinned
/// in tests/phase135_amendment_build.rs.
#[cfg(not(feature = "wasm-playground"))]
pub fn check_tier2_body(spec: &FunctionSpec, body: &IRNode) -> Result<(), String> {
    let index =
        crate::synthesis::engine::SpecIndex::from_specs(&crate::stdlib::prelude_specs());
    let run = eval::run_spec_tests_in(body, spec, &index);
    if run.passed == run.evaluable && run.evaluable == spec.tests.len() {
        Ok(())
    } else {
        Err(format!(
            "{}: the shipped body is not executed-exact on its full in-spec battery \
             ({} passed of {} evaluable, {} in spec)",
            spec.name,
            run.passed,
            run.evaluable,
            spec.tests.len()
        ))
    }
}

/// The whole crowning surface, re-run: every Tier-1
/// entry's crown and every Tier-2 entry's executed
/// standing, against the library's own modules. The
/// first failure names the entry and ends the assembly.
#[cfg(not(feature = "wasm-playground"))]
pub fn verify_crowns() -> Result<(), CodegenError> {
    // Entry name → (module source, the module's own
    // spec): the library's modules, parsed once.
    let mut by_name: std::collections::HashMap<String, (&str, FunctionSpec)> =
        std::collections::HashMap::new();
    for (_, source) in crate::stdlib::LIBRARY_MODULES {
        let module = crate::spec::parser::parse(source).map_err(|e| {
            CodegenError::InternalError(format!("package assembly: library parses: {e}"))
        })?;
        for spec in module.functions {
            by_name.insert(spec.name.clone(), (source, spec));
        }
    }
    for input in bindings::stdlib_inputs() {
        let (source, spec) = by_name.get(&input.spec.name).ok_or_else(|| {
            CodegenError::InternalError(format!(
                "package assembly: {} is not a library entry",
                input.spec.name
            ))
        })?;
        let checked = if input.tier == "tier-1" {
            match input.crown_route {
                bindings::CrownRoute::Pipeline => {
                    check_tier1_crown(source, spec, &input.body)
                }
                bindings::CrownRoute::Encoder => {
                    check_encoder_crown(spec, &input.body)
                }
            }
        } else {
            check_tier2_body(spec, &input.body)
        };
        checked.map_err(|reason| {
            CodegenError::InternalError(format!("package assembly refused: {reason}"))
        })?;
    }
    Ok(())
}

/// The checked assembly — the CLI's route: crowns
/// re-verified first, then the file set generated. A
/// moved crown fails here and no package is produced.
#[cfg(not(feature = "wasm-playground"))]
pub fn assemble_npm_package_checked() -> Result<Vec<PackageFile>, CodegenError> {
    verify_crowns()?;
    assemble_npm_package()
}

// ── The user-package verb (Phase 154, DISEBAR_15_SCOPE
// §4 — D2a: `build --package` generalized to a user
// module as `--package-from <file.morph>`) ─────────────
//
// The binding generator accepts user modules in-process,
// ungated — and prints caller-stated tier/basis claims
// verbatim (the PCC-1 fact, re-pinned at Phase 152). The
// verb is therefore the one place user packages are
// allowed to come from, and it feeds the generator ONLY
// rows it measured itself, in one seeded run per spec
// (`api::synthesize_seeded_spec_winner` — the report and
// the crowned body from the SAME run, so the packaged
// body is the body the rows describe):
//
//   * the verdict is the run's own `verification_status`;
//   * the battery is the run's executed counts — pinned
//     points are the tests the crown passed, spec-silent
//     points are the run's per-test `spec_silent`
//     outcomes (points the spec's own `require:` excludes,
//     Phase 109), counted from the same run's outcomes.
//
// ADMISSION (freeze §4.2): a spec enters the package iff
// its crown is Verified with an exact battery on its
// evaluable points (passed == evaluable — the assembly's
// own Tier-1 judgment, package.rs's
// `judge_pipeline_report`) → tier-1 / z3-verified; or
// executed-exact (passed == evaluable == total,
// total > 0 — the standing Tier-2 strictness of
// `check_tier2_body`: no spec-silent points at this
// tier) → tier-2 / executed-exact. Anything else —
// Inconclusive, VerificationFailed, a partial battery, an
// unevaluable spec — is REFUSED BY NAME, and the reading
// taken of the freeze's "refused by name, and nothing is
// written" is the stricter one: if ANY spec of the
// module is refused, the whole assembly is refused and
// no file set exists. (The looser reading — refuse the
// spec, package the rest — would let a module's package
// silently differ from the module; a package that claims
// to be a module's exports and quietly omits the spec
// that failed is a claim of the class this project
// refuses.)
//
// The stdlib crown checks do NOT transfer (they re-judge
// shipped bodies against the prelude); what transfers is
// the refusal discipline, the generator, and the layout
// shell above. In a build without the verifier, no crown
// stands Verified and the same module measures
// executed-exact rows instead — the rows say which
// evidence they are (the api's D2 honesty contract,
// carried into the manifest).

/// One admitted spec's measured standing, as the CLI
/// reports it and the README records it.
#[derive(Debug, Clone, PartialEq)]
pub struct UserStanding {
    pub name: String,
    /// `"tier-1"` | `"tier-2"` (the §2.4 vocabulary).
    pub tier: String,
    /// `"z3-verified"` | `"executed-exact"`.
    pub verdict_basis: String,
    /// The run's verification status, verbatim
    /// ("Verified" | "Inconclusive" | …).
    pub status: String,
    /// The run's executed counts for the crowned body.
    pub passed: usize,
    pub evaluable: usize,
    pub total: usize,
    /// Points the spec's own `require:` excludes
    /// (the run's `spec_silent` outcomes).
    pub spec_silent: usize,
}

/// A user package assembled by the verb: the file set
/// (the standing layout, via `assemble_package_files`)
/// and the measured standings it was admitted on.
#[derive(Debug, Clone)]
pub struct UserPackageAssembly {
    pub files: Vec<PackageFile>,
    pub standings: Vec<UserStanding>,
}

/// The user package's README: the factual surface,
/// generated from the assembly's own manifest rows —
/// what the package is, the export list with its tiers,
/// the admission + provenance language, the boundary
/// notes that are the generator's own facts (including
/// EMBED_SCOPE §3's `never` stance for function-typed
/// parameters), and the two version fields' separate
/// facts. No license is asserted: licensing a user
/// package is its author's act, not the verb's.
fn user_package_readme(
    package_name: &str,
    scope: &str,
    seed: u64,
    set: &BindingSet,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {package_name}\n\n"));
    out.push_str(&format!(
        "Functions generated from the Morphic module `{scope}` by \
         `morphic build --package-from` — every export synthesized from its \
         specification by the Morphic pipeline and admitted on the standing \
         the pipeline itself measured (below). The root module exports all \
         of them; each is also importable from its own subpath (for example \
         `{package_name}/{}`).\n\n",
        set.manifest.exports.first().map(|e| e.name.as_str()).unwrap_or("")
    ));
    out.push_str(
        "This package is an artifact, not a compiler: it contains no \
         synthesizer and makes no runtime verification claims. The \
         functions are plain, environment-neutral ES modules (BigInt, \
         ES2020, is the only versioned requirement).\n\n",
    );
    out.push_str("## The functions\n\n");
    for e in &set.manifest.exports {
        out.push_str(&format!(
            "- `{}` — `{}` — {} ({}) — battery: {} pinned, {} spec-silent\n",
            e.name, e.signature, e.tier, e.verdict_basis, e.battery.pinned,
            e.battery.spec_silent
        ));
    }
    out.push_str(
        "\nTier 1 — Z3-verified against the function's specification, and \
         exact on every evaluable battery point. Tier 2 — executed-exact \
         on the function's full in-spec battery (every point evaluable, \
         every point passed); the tier is a property of the proof \
         standard, not of the behavior.\n\n",
    );
    out.push_str("## Provenance — measured rows only\n\n");
    out.push_str(&format!(
        "Generated — do not edit. This package was assembled by `morphic \
         build --package-from {scope}` (seed {seed}, the seeded bounded \
         LLM-free profile). Every tier, verdict basis and battery figure \
         in `morphic.manifest.json` was measured by that assembly's own \
         synthesis run: no argument, flag, file or caller statement \
         supplies a tier, a verdict basis, or a battery figure. A spec \
         whose crown stands neither Verified (with an exact battery) nor \
         executed-exact is refused by name and the whole assembly is \
         refused — nothing is written. Re-running the verb over the same \
         module with the same seed reproduces this package byte-for-byte; \
         that re-run is the package's re-judgment route.\n\n"
    ));
    out.push_str("## Types at the boundary\n\n");
    out.push_str(
        "- Morphic `Int` is TypeScript `bigint` — exact inside the i64 \
         domain [-2^63, 2^63 - 1]. The functions carry no runtime \
         precondition checks: a host value outside that domain is \
         out-of-domain input, outside the battery-exactness promise. \
         BigInt `/` truncates toward zero and `%` takes the dividend's \
         sign, matching Morphic Int semantics.\n\
         - `Char` is a `string` of exactly one Unicode code point (an \
         astral character has `.length` 2; the contract is the code \
         point, not the length). Morphic counts string length in code \
         points; JS `.length` counts UTF-16 code units — never use \
         `.length` where a Morphic length is meant.\n\
         - `Option<T>` is `T | null`. A nested `Option<Option<T>>` would \
         flatten under this form.\n\
         - `Result<T, E>` is the discriminated union \
         `{ tag: \"Ok\", value: T } | { tag: \"Err\", error: E }`.\n\
         - A function-typed parameter is declared `never` in the \
         TypeScript declarations (EMBED_SCOPE §3): no callable TypeScript \
         function type is frozen, and the manifest's `signature` field \
         carries the true Morphic signature. At runtime any JavaScript \
         function of the right shape answers for such a parameter — \
         including this package's own exports.\n\n",
    );
    out.push_str("## Versioning\n\n");
    out.push_str(&format!(
        "The package version (`{}`) is the module author's, supplied to \
         the verb by flag. The manifest's `morphicVersion` records the \
         version of Morphic whose pipeline generated and judged the \
         contents; the two are separate facts and neither borrows the \
         other.\n",
        set.manifest.package_version
    ));
    out
}

/// The admission loop both user-module verbs share —
/// this package verb and the bundle production of
/// Phase 163 (`crate::pcc::assemble_user_bundles`, the
/// CLI's `build --bundles-from` arm): parse + typecheck
/// the user's module, run the seeded profile per spec,
/// admit on measured rows only (the discipline of the
/// section header above). `flag` and `noun` name the
/// calling arm in the refusal texts ("--package-from" /
/// "package assembly"; "--bundles-from" / "bundle
/// production") — the discipline is one; only the names
/// are the caller's. On any refusal the `Err` names
/// every refused spec with its measured standing, and
/// no artifact of either verb exists.
pub fn admit_user_specs(
    source: &str,
    seed: u64,
    flag: &str,
    noun: &str,
) -> Result<(Vec<bindings::BindingInput>, Vec<UserStanding>), String> {
    let parsed = crate::spec::parser::parse(source).map_err(|e| {
        format!("{flag}: the module does not parse: {e}")
    })?;
    let checked = crate::spec::typeck::check(parsed).map_err(|e| {
        format!("{flag}: the module does not typecheck: {e}")
    })?;
    let specs = checked.spec.functions;
    if specs.is_empty() {
        return Err(format!(
            "{flag}: the module declares no specs — nothing to admit"
        ));
    }

    let mut inputs: Vec<bindings::BindingInput> = Vec::new();
    let mut standings: Vec<UserStanding> = Vec::new();
    let mut refusals: Vec<String> = Vec::new();
    for spec in &specs {
        let (report, winner) =
            match crate::api::synthesize_seeded_spec_winner(source, &spec.name, seed) {
                Ok(pair) => pair,
                Err(e) => {
                    refusals.push(format!(
                        "{}: the seeded run failed at the {} stage: {}",
                        spec.name, e.stage, e.message
                    ));
                    continue;
                }
            };
        let w = &report.winner;
        let spec_silent = w
            .test_outcomes
            .iter()
            .filter(|o| o.outcome == "spec_silent")
            .count();
        let (tier, verdict_basis) =
            if w.verification_status == "Verified" && w.tests_passed == w.tests_evaluable {
                ("tier-1", "z3-verified")
            } else if w.tests_passed == w.tests_evaluable
                && w.tests_evaluable == w.tests_total
                && w.tests_total > 0
            {
                ("tier-2", "executed-exact")
            } else {
                refusals.push(format!(
                    "{}: its crown stands {} at {}/{} executed \
                     ({} of {} in-spec tests evaluable, {} spec-silent) — \
                     neither Verified with an exact battery nor executed-exact",
                    spec.name,
                    w.verification_status,
                    w.tests_passed,
                    w.tests_evaluable,
                    w.tests_evaluable,
                    w.tests_total,
                    spec_silent
                ));
                continue;
            };
        standings.push(UserStanding {
            name: spec.name.clone(),
            tier: tier.to_string(),
            verdict_basis: verdict_basis.to_string(),
            status: w.verification_status.clone(),
            passed: w.tests_passed,
            evaluable: w.tests_evaluable,
            total: w.tests_total,
            spec_silent,
        });
        inputs.push(bindings::BindingInput {
            spec: spec.clone(),
            body: winner.body.clone(),
            tier: tier.to_string(),
            verdict_basis: verdict_basis.to_string(),
            battery_pinned: w.tests_passed,
            battery_silent: spec_silent,
            crown_route: bindings::CrownRoute::Pipeline,
        });
    }
    if !refusals.is_empty() {
        return Err(format!(
            "{noun} refused — nothing is written: {}",
            refusals.join("; ")
        ));
    }
    Ok((inputs, standings))
}

/// The verb's core (the CLI arm is a shell over this):
/// the shared admission loop (`admit_user_specs`), then
/// the standing package layout over the retained
/// inputs. On any refusal the `Err` names every refused
/// spec with its measured standing, and no file set
/// exists.
pub fn assemble_user_package(
    source: &str,
    package_name: &str,
    package_version: &str,
    scope: &str,
    seed: u64,
) -> Result<UserPackageAssembly, String> {
    let (inputs, standings) =
        admit_user_specs(source, seed, "--package-from", "package assembly")?;

    let set = bindings::generate_bindings_versioned(
        &inputs,
        package_name,
        package_version,
        env!("CARGO_PKG_VERSION"),
        scope,
    )
    .map_err(|e| format!("--package-from: binding generation failed: {e}"))?;
    let readme = user_package_readme(package_name, scope, seed, &set);
    let furniture = PackageFurniture {
        description: format!(
            "Functions generated from the Morphic module {scope} by \
             `morphic build --package-from`: every export was synthesized \
             by the seeded pipeline and admitted on its own measured \
             standing (Tier-1 Z3-verified or Tier-2 executed-exact). \
             Per-function tiers and batteries: morphic.manifest.json."
        ),
        license_field: None,
        licenses: Vec::new(),
        project_links: false,
    };
    let files = assemble_package_files(&set, &readme, &furniture);
    Ok(UserPackageAssembly { files, standings })
}
