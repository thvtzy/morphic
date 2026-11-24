//! Phase 148 — scope-audit probes for the Disebar rung
//! (MCP server · WASM Component Model · `.morph` package
//! registry · LSP + VS Code · edge deploy · proof-carrying
//! code).
//!
//! Every test prints a `P148|...` line with the measured
//! standing; assertions pin exactly what the line reports.
//! Verdict-bearing pins branch on `z3-support` exactly as
//! the Phase 127/133/137 probes do; prelude-bearing pins
//! carry the standing `#[cfg(not(feature =
//! "wasm-playground"))]` gate (the browser profile's
//! prelude is asset-installed and empty in a test process).

use morphic::api;
use morphic::codegen::bindings::{self, BindingInput, CrownRoute};
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::IRNode;
use std::time::Instant;

const VALID: &str = include_str!("../examples/self_host/validator.morph");
const INVALID: &str = "spec broken { input: x: Int output: Int constraint: ";
const TYPE_ERR: &str = "Typed {\n\
    spec f {\n\
        input: a: Int\n\
        output: Int\n\
        constraint: output == a ++ 2\n\
    }\n\
    }";

/// MCP-1: the surface an MCP server would wrap already
/// exists as serde DTOs over strings — `api::check`
/// returns a structured report that serializes to JSON
/// (the transport shape), with the failing stage named.
#[test]
fn p148_mcp_check_dto_serializes() {
    let report = api::check(VALID);
    assert!(report.ok);
    let json = serde_json::to_string(&report).expect("CheckReport serializes");
    assert!(json.contains("\"ok\":true"));
    let bad = api::check(INVALID);
    assert!(!bad.ok);
    assert_eq!(bad.error.as_ref().map(|e| e.stage.as_str()), Some("parse"));
    println!(
        "P148|mcp|check_dto=serializable specs={} json_bytes={}",
        report.specs.len(),
        json.len()
    );
}

/// MCP-2: a NATIVE build (what an MCP server process
/// would be) carries the real verifier — the seeded
/// synthesis entry point reports `verification_available`
/// from the build's features and crowns a winner with an
/// actual verdict, where the browser build's forced
/// "Inconclusive" stands in the no-Z3 configuration.
#[test]
fn p148_mcp_synthesis_native_verdict_and_latency() {
    let t0 = Instant::now();
    let report = api::synthesize_seeded_spec(VALID, "is_known_type_tag", 42)
        .expect("named-spec seeded run completes");
    let ms = t0.elapsed().as_millis();
    assert_eq!(report.verification_available, cfg!(feature = "z3-support"));
    #[cfg(feature = "z3-support")]
    assert_eq!(report.winner.verification_status, "Verified");
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(report.winner.verification_status, "Inconclusive");
    println!(
        "P148|mcp|synthesis status={} verification_available={} elapsed_ms={}",
        report.winner.verification_status, report.verification_available, ms
    );
}

/// MCP-3 / EDGE-1: the verified library is enumerable
/// in-process — every prelude spec pairs with its shipped
/// body, and the binding generator produces the full
/// 35-export manifest with its tier/verdictBasis rows.
/// (This enumeration is what an MCP `list_library` tool
/// and an edge bundle both consume.)
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn p148_library_surface_enumerable() {
    let specs = morphic::stdlib::prelude_specs();
    assert_eq!(specs.len(), 35);
    let bodied = specs
        .iter()
        .filter(|s| morphic::stdlib::shipped_body(&s.name).is_some())
        .count();
    assert_eq!(bodied, 35);
    let set = bindings::stdlib_bindings().expect("stdlib bindings generate");
    assert_eq!(set.manifest.exports.len(), 35);
    let t1 = set.manifest.exports.iter().filter(|e| e.tier == "tier-1").count();
    let z3 = set
        .manifest
        .exports
        .iter()
        .filter(|e| e.verdict_basis == "z3-verified")
        .count();
    let ee = set
        .manifest
        .exports
        .iter()
        .filter(|e| e.verdict_basis == "executed-exact")
        .count();
    assert_eq!((t1, z3, ee), (33, 33, 2));
    println!(
        "P148|library|entries=35 tier1={} z3_verified={} executed_exact={}",
        t1, z3, ee
    );
}

/// PCC-1: the manifest's `verdictBasis` is a CLAIM, not
/// evidence — the binding generator prints the caller's
/// stated basis verbatim, even for a body that does not
/// satisfy its spec. Nothing in `generate_bindings`
/// checks it; the checking lives in package assembly's
/// crown checks, which do not ship to consumers.
#[test]
fn p148_pcc_manifest_verdict_basis_is_caller_stated() {
    let module = parser::parse(
        "Claim {\n\
        spec plus_one {\n\
            input: x: Int\n\
            output: Int\n\
            constraint: output == x + 1\n\
        }\n\
        }",
    )
    .expect("probe module parses");
    let spec = module.functions.into_iter().next().expect("one spec");
    // The identity body does NOT satisfy `output == x + 1`.
    let input = BindingInput {
        spec,
        body: IRNode::Var("x".into()),
        tier: "tier-1".into(),
        verdict_basis: "z3-verified".into(),
        battery_pinned: 0,
        battery_silent: 0,
        crown_route: CrownRoute::Pipeline,
    };
    let set = bindings::generate_bindings(&[input], "probe", "0.0.0", "P148")
        .expect("bindings generate from stated inputs");
    assert_eq!(set.manifest.exports.len(), 1);
    assert_eq!(set.manifest.exports[0].verdict_basis, "z3-verified");
    let json = serde_json::to_string(&set.manifest).expect("manifest serializes");
    assert!(json.contains("\"verdictBasis\":\"z3-verified\""));
    println!(
        "P148|pcc|manifest_claim=verbatim verdictBasis=z3-verified for a non-verifying body"
    );
}

/// PCC-2: the re-verification machinery a certificate
/// would ship ALREADY exists in-process — package
/// assembly's encoder crown check re-judges a shipped
/// body from (spec, body) alone: it accepts the true
/// shipped body in a Z3 build, rejects a wrong body, and
/// in a no-Z3 build refuses by name rather than passing.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn p148_pcc_crown_check_reverifies_from_spec_and_body() {
    let specs = morphic::stdlib::prelude_specs();
    let spec = specs
        .iter()
        .find(|s| s.name == "list_reverse")
        .expect("list_reverse in prelude")
        .clone();
    let body = morphic::stdlib::shipped_body("list_reverse").expect("shipped body");
    let good = morphic::codegen::package::check_encoder_crown(&spec, &body);
    #[cfg(feature = "z3-support")]
    assert!(good.is_ok(), "true body re-verifies: {:?}", good.err());
    #[cfg(not(feature = "z3-support"))]
    assert!(
        good.as_ref().err().map(|e| e.contains("Z3-enabled")).unwrap_or(false),
        "no-Z3 build refuses by name: {:?}",
        good.err()
    );
    let wrong = morphic::codegen::package::check_encoder_crown(&spec, &IRNode::Var("xs".into()));
    assert!(wrong.is_err(), "a wrong body never passes the crown check");
    println!(
        "P148|pcc|crown_check true_body={} wrong_body=rejected",
        if cfg!(feature = "z3-support") { "ok" } else { "refused-no-z3" }
    );
}

/// REG-1: imports are parsed and accepted but NOT
/// resolved (LANGUAGE_REFERENCE §15) — an import line
/// changes nothing a checker can see: a name only the
/// imported file could provide is still undefined, while
/// a prelude entry needs no import at all. There is no
/// module system for a registry to address yet.
///
/// The prelude half measures the NATIVE prelude: under
/// `wasm-playground` the prelude is the wasm stub (no
/// stdlib entries), so the whole probe is native-only —
/// the Phase 136 cfg pattern, applied at Phase 151 when
/// CI's workspace --all-targets configuration (feature
/// unification) exposed the missing gate.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn p148_registry_import_parsed_not_resolved() {
    let with_import = "M {\n\
        import \"math.morph\";\n\
        spec f {\n\
            input: x: Int\n\
            output: Int\n\
            constraint: output == math_helper(x)\n\
        }\n\
        }";
    let parsed = parser::parse(with_import).expect("import line parses");
    let checked = typeck::check(parsed);
    assert!(
        checked.is_err(),
        "the imported name resolves to nothing: imports are not resolved"
    );
    let prelude_only = "M {\n\
        spec f {\n\
            input: x: Int\n\
            output: Int\n\
            constraint: output == int_abs(x)\n\
        }\n\
        }";
    let parsed = parser::parse(prelude_only).expect("prelude module parses");
    assert!(
        typeck::check(parsed).is_ok(),
        "a prelude entry is callable by plain name with no import"
    );
    println!(
        "P148|registry|import=parsed-not-resolved prelude=merged-no-import"
    );
}

/// LSP-1: the diagnostic shapes an LSP server would
/// publish, as they stand — parse errors carry 1-based
/// line + column + snippet; typecheck errors carry NO
/// position (TypeError's span is never populated), only
/// a message. Check is whole-source: one call re-parses
/// and re-checks the entire text (no incremental API
/// exists in the tree).
#[test]
fn p148_lsp_diagnostic_shapes() {
    let bad = api::check(INVALID);
    let e = bad.error.expect("parse error");
    assert_eq!(e.stage, "parse");
    assert_eq!(e.line, Some(1));
    assert!(e.column.is_some_and(|c| c >= 1));

    let terr = api::check(TYPE_ERR);
    let e = terr.error.expect("typecheck error");
    assert_eq!(e.stage, "typecheck");
    assert_eq!(e.line, None);
    assert_eq!(e.column, None);

    let t0 = Instant::now();
    let whole = api::check(VALID);
    let ms = t0.elapsed().as_millis();
    assert!(whole.ok);
    println!(
        "P148|lsp|parse_error=line+column typeck_error=no-position whole_source_check_ms={}",
        ms
    );
}
