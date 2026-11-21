// ─────────────────────────────────────────────────────────────
//  MORPHIC IN THE BROWSER (v0.6 W1)
//
//  A thin wasm-bindgen shell over the morphic library's
//  string/DTO API (morphic::api). DTOs cross the boundary
//  as JSON strings. This crate contains no compiler logic:
//  the playground is a configuration of the same codebase
//  the CLI ships (Cargo features), never a fork (v0.6
//  anti-pattern). The build has no Z3 and no compiled
//  runner — synthesis reports say so (`verification_available:
//  false`, winners labelled Inconclusive), per D2.
// ─────────────────────────────────────────────────────────────

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Parse + typecheck a Morphic source text; returns the
/// CheckReport as JSON (ok / failing stage / per-spec test
/// inventory with bindability). The W2 playground surface.
#[wasm_bindgen]
pub fn check(source: &str) -> String {
    serde_json::to_string(&morphic::api::check(source))
        .unwrap_or_else(|e| format!("{{\"ok\":false,\"error\":{{\"stage\":\"internal\",\"message\":\"report serialization failed: {}\"}},\"specs\":[]}}", e))
}

/// One seeded playground-profile synthesis run over the
/// source's first spec; returns the SynthesisReport as
/// JSON, or the StageError as JSON on failure. `seed` is a
/// JS number; it is truncated to u64 (pass a non-negative
/// integer below 2^53 for exactness).
#[wasm_bindgen]
pub fn synthesize_seeded(source: &str, seed: f64) -> String {
    match morphic::api::synthesize_seeded(source, seed as u64) {
        Ok(report) => serde_json::to_string(&report)
            .unwrap_or_else(|e| format!("{{\"stage\":\"internal\",\"message\":\"{}\"}}", e)),
        Err(stage_error) => serde_json::to_string(&stage_error)
            .unwrap_or_else(|e| format!("{{\"stage\":\"internal\",\"message\":\"{}\"}}", e)),
    }
}

/// One seeded playground-profile synthesis run over a
/// NAMED spec of the module (W3); returns the
/// SynthesisReport as JSON, or the StageError as JSON on
/// failure. The report carries the winner's executed-test
/// fraction (passed / evaluable / total), per-test
/// outcomes, and renders to all six codegen targets.
/// `seed` is a JS number; it is truncated to u64 (pass a
/// non-negative integer below 2^53 for exactness).
#[wasm_bindgen]
pub fn synthesize_seeded_spec(source: &str, spec_name: &str, seed: f64) -> String {
    match morphic::api::synthesize_seeded_spec(source, spec_name, seed as u64) {
        Ok(report) => serde_json::to_string(&report)
            .unwrap_or_else(|e| format!("{{\"stage\":\"internal\",\"message\":\"{}\"}}", e)),
        Err(stage_error) => serde_json::to_string(&stage_error)
            .unwrap_or_else(|e| format!("{{\"stage\":\"internal\",\"message\":\"{}\"}}", e)),
    }
}

/// S6 probe (Phase 65 audit): does `std::time::Instant`
/// work in this runtime, and does elapsed time advance?
/// Returns JSON { instant_ok, elapsed_us, sink }. If
/// Instant::now() panics on this target, the call throws —
/// the harness records that as instant_ok: false, which is
/// itself the measurement: playground budgets must then be
/// iteration/population caps, never milliseconds.
#[wasm_bindgen]
pub fn probe_timing() -> String {
    let t0 = std::time::Instant::now();
    let mut sink: u64 = 0;
    for i in 0..200_000u64 {
        sink = sink.wrapping_add(i);
    }
    let elapsed_us = t0.elapsed().as_micros();
    format!(
        "{{\"instant_ok\":true,\"elapsed_us\":{},\"sink\":{}}}",
        elapsed_us, sink
    )
}
