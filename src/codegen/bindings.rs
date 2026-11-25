// ─────────────────────────────────────────────────────────────
//  TYPED BINDINGS + THE TIER MANIFEST (Phase 121, 1.1
//  Embeddable — EMBED_SCOPE §5, decisions D1a/D2a)
//
//  The binding generator: a set of (spec, shipped/crowned
//  body, verdict) inputs becomes the three artifacts a
//  host consumes —
//
//    * one assembled ESM module (R7: ESM-only) exporting
//      every input by name, each function generated from
//      its body through the Phase 120 JavaScript emitter;
//      entry-to-entry calls render there as plain calls
//      to the export name, so they resolve inside the
//      module by construction, transitively closed
//      because every input is in the module;
//    * TypeScript declarations for every export, in the
//      frozen §3 shapes (ts_type below), each carrying
//      the entry's doc:, its tier and its verdict basis
//      as JSDoc;
//    * `morphic.manifest.json` in the frozen §2.4 form —
//      the tier as a fact of the package surface.
//
//  ONE SOURCE OF TRUTH: the declarations and the manifest
//  both derive from the input specs (signatures via
//  morphic_signature, types via ts_type), so the two
//  cannot drift apart; the Phase 121 pin holds the stdlib
//  outputs against EMBED_SCOPE §2.1's frozen table (as
//  amended — Phase 135 added Amendment 2's six rows).
//
//  WRAPPERS — MEASURED NONE for the frozen surface: the Phase
//  120 emitter's generated functions already speak the
//  §3 boundary shapes natively (Int is BigInt, `Some(x)`
//  is the payload, `None` is `null`, `Ok`/`Err` are the
//  discriminated-union objects, a Char is a one-code-
//  point string, a List is an array), so every frozen
//  export is declared over its raw generated function
//  directly and no conversion wrapper exists. The pin
//  holds this: the assembled module defines exactly the
//  frozen exports and nothing else, and the clean-host
//  battery runs typed §3-shaped values against the raw
//  functions. The generator has no wrapper machinery —
//  if a future body shape ever needed one, it would be
//  added here, named, not smuggled into the emitter.
//
//  THE VERDICT FIELDS ARE INPUTS, NEVER INVENTED: a
//  caller states each function's tier + verdict basis
//  from the standing the native pipeline produced — for
//  the library, the frozen STDLIB_SCOPE tiers; for a
//  user module (the ungated D1a route), whatever its
//  own pipeline run measured (R12: no Verified claim is
//  minted here).
// ─────────────────────────────────────────────────────────────

use serde::Serialize;

use crate::spec::ast::{FunctionSpec, TypeRef};
use crate::synthesis::engine::{CandidateImplementation, IRNode, Provenance, ScoreBreakdown};

use super::{generate, CodegenError};

/// The npm package name (EMBED_SCOPE D8a).
pub const PACKAGE_NAME: &str = "@thvtzy/morphic";

/// The manifest's `scope` value for the stdlib package:
/// the frozen surface's document, by name (§2.4).
pub const STDLIB_SCOPE_NAME: &str = "EMBED_SCOPE.md";

/// One function to bind: its spec, the body the pipeline
/// crowned for it, and the verdict that body carries —
/// stated by the caller from the pipeline's own record.
#[derive(Debug, Clone)]
pub struct BindingInput {
    pub spec: FunctionSpec,
    pub body: IRNode,
    /// `"tier-1"` | `"tier-2"` (the §2.4 vocabulary).
    pub tier: String,
    /// `"z3-verified"` | `"executed-exact"` (§2.4).
    pub verdict_basis: String,
    /// The function's frozen battery, exactly-pinned
    /// points (manifest `battery.pinned`).
    pub battery_pinned: usize,
    /// The battery's recorded spec-silent points
    /// (manifest `battery.specSilent`).
    pub battery_silent: usize,
    /// How this entry's Tier-1 crown is re-verified at
    /// assembly (internal; not a manifest field).
    pub crown_route: CrownRoute,
}

/// The tier manifest (EMBED_SCOPE §2.4, frozen form).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Manifest {
    pub package: String,
    #[serde(rename = "packageVersion")]
    pub package_version: String,
    #[serde(rename = "morphicVersion")]
    pub morphic_version: String,
    pub scope: String,
    pub exports: Vec<ManifestExport>,
}

/// One manifest row (§2.4's per-export fields).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ManifestExport {
    pub name: String,
    pub signature: String,
    pub tier: String,
    #[serde(rename = "verdictBasis")]
    pub verdict_basis: String,
    pub battery: ManifestBattery,
    pub doc: String,
}

/// A battery's point counts (§2.4).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ManifestBattery {
    pub pinned: usize,
    #[serde(rename = "specSilent")]
    pub spec_silent: usize,
}

/// The three generated artifacts.
#[derive(Debug, Clone)]
pub struct BindingSet {
    /// The assembled ESM module (`.js`).
    pub js_module: String,
    /// The TypeScript declarations (`.d.ts`).
    pub declarations: String,
    /// The manifest, structured.
    pub manifest: Manifest,
    /// The manifest as `morphic.manifest.json`.
    pub manifest_json: String,
}

/// A Morphic type in its frozen TypeScript form
/// (EMBED_SCOPE §3). Losses are the table's, named
/// there: Int is `bigint` (exact inside the i64 domain,
/// D3a); Char is a `string` of exactly one code point;
/// `Option<T>` is `T | null` (D4a — `Option<Option<T>>`
/// flattens, R4); Result is the discriminated union.
pub fn ts_type(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Int | TypeRef::I64 => "bigint".to_string(),
        // Fixed-width ints other than i64 are outside the
        // 1.0 surface (§3 binds Int); the ungated
        // generator maps them to the JS number they fit.
        TypeRef::I8
        | TypeRef::I16
        | TypeRef::I32
        | TypeRef::U8
        | TypeRef::U16
        | TypeRef::U32
        | TypeRef::U64
        | TypeRef::Float => "number".to_string(),
        TypeRef::Bool => "boolean".to_string(),
        TypeRef::Char | TypeRef::String => "string".to_string(),
        TypeRef::Unit => "undefined".to_string(),
        TypeRef::Generic(name) | TypeRef::Named(name) => name.clone(),
        TypeRef::List(inner) => format!("{}[]", ts_type_operand(inner)),
        TypeRef::Set(inner) => format!("{}[]", ts_type_operand(inner)),
        TypeRef::Array(inner, _) => format!("{}[]", ts_type_operand(inner)),
        TypeRef::Stream(inner) => format!("{}[]", ts_type_operand(inner)),
        TypeRef::Map(_, value) => format!("Record<string, {}>", ts_type(value)),
        TypeRef::Option(inner) => format!("{} | null", ts_type_operand(inner)),
        TypeRef::Result(ok, err) => format!(
            "{{ tag: \"Ok\", value: {} }} | {{ tag: \"Err\", error: {} }}",
            ts_type(ok),
            ts_type(err)
        ),
        TypeRef::Tuple(items) => {
            let parts: Vec<String> = items.iter().map(ts_type).collect();
            format!("[{}]", parts.join(", "))
        }
        TypeRef::Ref(inner) => ts_type(inner),
        // §3: no mapping exists for function types (no
        // first-class functions in the 1.1 surface); the
        // ungated generator says so with `never` rather
        // than inventing a callable shape.
        TypeRef::Function(..) => "never".to_string(),
    }
}

/// A type in operand position (array element, Option
/// payload): union forms parenthesize so the composite
/// reads as one type.
fn ts_type_operand(ty: &TypeRef) -> String {
    let rendered = ts_type(ty);
    if rendered.contains(" | ") {
        format!("({rendered})")
    } else {
        rendered
    }
}

/// A spec's signature in the frozen §2.1 form: `T → R`
/// for one parameter, `(T, U) → R` for several.
pub fn morphic_signature(spec: &FunctionSpec) -> String {
    let ret = spec.return_type.to_string();
    match spec.params.len() {
        0 => format!("→ {ret}"),
        1 => format!("{} → {ret}", spec.params[0].ty),
        _ => {
            let parts: Vec<String> =
                spec.params.iter().map(|p| p.ty.to_string()).collect();
            format!("({}) → {ret}", parts.join(", "))
        }
    }
}

/// The candidate the emitter consumes for one input.
fn candidate_of(input: &BindingInput) -> CandidateImplementation {
    CandidateImplementation {
        id: 121,
        body: input.body.clone(),
        spec_name: input.spec.name.clone(),
        spec: input.spec.clone(),
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
        verified: input.verdict_basis == "z3-verified",
        verification_status: None,
    }
}

/// The JSDoc block a declaration carries: the entry's
/// doc verbatim, its tier + verdict basis, its battery,
/// and the boundary notes §3/R2 attach mechanically
/// (a Char parameter's one-code-point contract).
fn declaration_jsdoc(input: &BindingInput) -> String {
    let mut out = String::from("/**\n");
    if let Some(doc) = &input.spec.doc {
        for line in doc.lines() {
            out.push_str(&format!(" * {line}\n"));
        }
        out.push_str(" *\n");
    }
    out.push_str(&format!(" * Tier: {}\n", input.tier));
    out.push_str(&format!(" * Verdict basis: {}\n", input.verdict_basis));
    out.push_str(&format!(
        " * Battery: {} pinned, {} spec-silent\n",
        input.battery_pinned, input.battery_silent
    ));
    if input.spec.params.iter().any(|p| p.ty == TypeRef::Char) {
        out.push_str(
            " * Char parameters are strings of exactly one Unicode code point (EMBED_SCOPE §3).\n",
        );
    }
    out.push_str(" */\n");
    out
}

/// One export's TypeScript declaration.
fn declaration_of(input: &BindingInput) -> String {
    let params: Vec<String> = input
        .spec
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, ts_type(&p.ty)))
        .collect();
    format!(
        "{}export declare function {}({}): {};\n",
        declaration_jsdoc(input),
        input.spec.name,
        params.join(", "),
        ts_type(&input.spec.return_type)
    )
}

/// Generate the binding set for a list of inputs. The
/// manifest's package fields are the caller's: `package`
/// is the package name, `version` fills both
/// `packageVersion` and `morphicVersion` (lockstep, R6),
/// and `scope` names the surface the rows describe.
pub fn generate_bindings(
    inputs: &[BindingInput],
    package: &str,
    version: &str,
    scope: &str,
) -> Result<BindingSet, CodegenError> {
    generate_bindings_versioned(inputs, package, version, version, scope)
}

/// `generate_bindings` with the two version fields
/// stated separately (Phase 154): the lockstep packages
/// pass one version twice (R6 — the package version IS
/// the Morphic version whose pipeline generated the
/// contents); a user package assembled by
/// `build --package-from` carries the AUTHOR's version
/// as `packageVersion` while `morphicVersion` stays the
/// generating pipeline's own — the crate's — so neither
/// field borrows the other's fact.
pub fn generate_bindings_versioned(
    inputs: &[BindingInput],
    package: &str,
    package_version: &str,
    morphic_version: &str,
    scope: &str,
) -> Result<BindingSet, CodegenError> {
    // The assembled module: each input's generated
    // function (its own per-function export line
    // stripped), then one export list naming them all.
    let mut js_module = String::from(
        "// Generated by the Morphic binding generator — do not edit.\n\
         // Every function below is the artifact its manifest row describes.\n",
    );
    let mut declarations = String::from(
        "// Generated by the Morphic binding generator — do not edit.\n",
    );
    let mut exports: Vec<ManifestExport> = Vec::with_capacity(inputs.len());
    for input in inputs {
        let text = generate(&candidate_of(input), "javascript")?;
        let export_line = format!("export {{ {} }};\n", input.spec.name);
        let body = text.strip_suffix(&export_line).ok_or_else(|| {
            CodegenError::InternalError(format!(
                "bindings: generated JS for {} lacks its export line",
                input.spec.name
            ))
        })?;
        js_module.push_str(body);
        js_module.push('\n');
        declarations.push_str(&declaration_of(input));
        declarations.push('\n');
        exports.push(ManifestExport {
            name: input.spec.name.clone(),
            signature: morphic_signature(&input.spec),
            tier: input.tier.clone(),
            verdict_basis: input.verdict_basis.clone(),
            battery: ManifestBattery {
                pinned: input.battery_pinned,
                spec_silent: input.battery_silent,
            },
            doc: input.spec.doc.clone().unwrap_or_default(),
        });
    }
    let names: Vec<&str> = inputs.iter().map(|i| i.spec.name.as_str()).collect();
    js_module.push_str(&format!("export {{ {} }};\n", names.join(", ")));

    let manifest = Manifest {
        package: package.to_string(),
        package_version: package_version.to_string(),
        morphic_version: morphic_version.to_string(),
        scope: scope.to_string(),
        exports,
    };
    let mut manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| {
        CodegenError::InternalError(format!("bindings: manifest serialization: {e}"))
    })?;
    manifest_json.push('\n');
    Ok(BindingSet {
        js_module,
        declarations,
        manifest,
        manifest_json,
    })
}

// ── The frozen stdlib surface ────────────────────────────
//
// The 35 entries: EMBED_SCOPE §2.1's 29 in its order,
// then STDLIB_SCOPE.md Amendment 2's six in the
// amendment's order (Phase 135), with the tier +
// verdict basis STDLIB_SCOPE froze for each and
// the battery counts of the standing cross-validation
// batteries (Phase 111 Int/Char, Phase 112
// String/Result/Option, Phase 115 List, Phase 135
// Amendment 2 — measured, their P11xXV / P135XV
// lines). `pinned` counts exactly-pinned points;
// `specSilent` the recorded spec-silent points
// (`int_abs` at the Int minimum, `int_clamp` on inverted
// ranges, `list_binary_search`'s duplicate-target
// indices — the battery's 67 property points split
// 64 exactly-pinned + 3 index-silent).
//
// Phase 144 (the 1.3 re-tier, LIST_ENCODER_SCOPE §5.1,
// EMBED_SCOPE R11): the fifteen entries the list-
// encoder campaign crowned (Phases 141–143) carry
// tier-1 / z3-verified here — label data only; their
// bodies are the same shipped bodies. Their crown
// route is `CrownRoute::Encoder` (see the enum): the
// assembly re-verifies them by the route that crowned
// them, not by the synthesis pipeline, which crowns
// no body for their specs. `list_sort` and
// `list_binary_search` stay tier-2 with their
// residues named in their `doc:` tier sentences.

/// How an entry's Tier-1 crown is re-verified at
/// assembly (package.rs's crown checks). `Pipeline`:
/// the seeded synthesis pipeline (seed 42) must crown
/// exactly the shipped body, Verified — the Phase 110
/// procedure, the standing route for the entries the
/// pipeline itself crowned. `Encoder`: the entry was
/// crowned by the 1.3 list-encoder campaign (Phases
/// 141–143) — its shipped hand body Verifies through
/// `verify_one` over the prelude index, every row
/// Proven, battery-exact — because the synthesis pool
/// holds no candidate for these specs and the pipeline
/// crowns a different, unverified body (measured at
/// Phase 144). The field is assembly-internal: it is
/// not a manifest field (§2.4's form carries no such
/// row) and moves no package byte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrownRoute {
    Pipeline,
    Encoder,
}

struct FrozenRow {
    name: &'static str,
    tier: &'static str,
    verdict_basis: &'static str,
    battery_pinned: usize,
    battery_silent: usize,
    crown: CrownRoute,
}

const FROZEN: &[FrozenRow] = &[
    FrozenRow { name: "int_abs", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 16, battery_silent: 1, crown: CrownRoute::Pipeline },
    FrozenRow { name: "int_sign", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 11, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "int_min", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 16, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "int_max", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 16, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "int_clamp", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 13, battery_silent: 2, crown: CrownRoute::Pipeline },
    FrozenRow { name: "int_is_even", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "int_is_odd", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "char_is_digit", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 101, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "char_is_alpha", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 101, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "char_is_whitespace", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 101, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "char_is_uppercase", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 101, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "char_is_lowercase", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 101, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "char_to_digit", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 101, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "string_is_empty", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "result_unwrap_or", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 36, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "option_unwrap_or", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 28, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "option_ok_or", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 9, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "list_is_empty", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "list_reverse", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_contains", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 75, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_sum", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_product", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 13, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_min", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_max", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_take", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 28, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_drop", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 28, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_is_sorted", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_sort", tier: "tier-2", verdict_basis: "executed-exact", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Pipeline },
    FrozenRow { name: "list_binary_search", tier: "tier-2", verdict_basis: "executed-exact", battery_pinned: 64, battery_silent: 3, crown: CrownRoute::Pipeline },
    // Amendment 2 (Phase 135): the higher-order entries
    // + range, Tier-2 on the A2.3 batteries.
    FrozenRow { name: "list_map", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 20, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_filter", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 20, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_fold", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 18, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_all", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 16, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "list_any", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 16, battery_silent: 0, crown: CrownRoute::Encoder },
    FrozenRow { name: "range", tier: "tier-1", verdict_basis: "z3-verified", battery_pinned: 15, battery_silent: 0, crown: CrownRoute::Encoder },
];

/// The binding inputs for the frozen stdlib surface:
/// each §2.1 entry's prelude spec + shipped body, in
/// §2.1's order, with its frozen verdict row.
pub fn stdlib_inputs() -> Vec<BindingInput> {
    let specs = crate::stdlib::prelude_specs();
    FROZEN
        .iter()
        .map(|row| {
            let spec = specs
                .iter()
                .find(|s| s.name == row.name)
                .unwrap_or_else(|| panic!("bindings: {} in prelude", row.name))
                .clone();
            let body = crate::stdlib::shipped_body(row.name)
                .unwrap_or_else(|| panic!("bindings: {} ships a body", row.name));
            BindingInput {
                spec,
                body,
                tier: row.tier.to_string(),
                verdict_basis: row.verdict_basis.to_string(),
                battery_pinned: row.battery_pinned,
                battery_silent: row.battery_silent,
                crown_route: row.crown,
            }
        })
        .collect()
}

/// The stdlib binding set: the `@thvtzy/morphic`
/// package's module, declarations and manifest,
/// generated from the shipped bodies. Version is this
/// crate's — the pipeline that generated the contents
/// (§2.4); at a release it equals the package version
/// by the R6 lockstep.
pub fn stdlib_bindings() -> Result<BindingSet, CodegenError> {
    generate_bindings(
        &stdlib_inputs(),
        PACKAGE_NAME,
        env!("CARGO_PKG_VERSION"),
        STDLIB_SCOPE_NAME,
    )
}
