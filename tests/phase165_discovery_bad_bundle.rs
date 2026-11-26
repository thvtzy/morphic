//! Phase 165 — discovery degradation, per-entry shape
//! (DISEBAR_17_SCOPE §7): one installed bundle file
//! damaged after install degrades THAT entry by name;
//! the rest of the layer and the stdlib stand.
//!
//! This binary's one install state: the widget pair
//! installed through the Phase 164 verbs (the shipped
//! binary, a scratch `MORPHIC_REGISTRY_DIR`, the repo's
//! own catalog index) — or, in a no-Z3 build where the
//! Tier-1 verb refuses by name, planted byte-for-byte
//! as a Z3 install writes it — after which
//! `is_widget_code.bundle.json` is overwritten with
//! garbage — the file no longer matches its receipt.
//! The environment is set once, before any funnel read
//! in this process (the layer reads the directory once
//! per process, §7).

#![cfg(not(feature = "wasm-playground"))]

use std::process::Command;
use std::sync::Once;

use morphic::stdlib;

fn scratch_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("morphic-phase165-bad-bundle-{}", std::process::id()))
}

fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let dir = scratch_dir();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        if cfg!(feature = "z3-support") {
            let index = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("registry")
                .join("index.json");
            for name in ["is_widget_code", "weights_sorted"] {
                let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
                    .args(["registry", "add", name, "--index"])
                    .arg(&index)
                    .env("MORPHIC_REGISTRY_DIR", &dir)
                    .output()
                    .expect("the registry add runs");
                assert!(
                    out.status.success(),
                    "registry add {name}: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        } else {
            // A no-Z3 build cannot install the Tier-1
            // half (it stands unexamined, refused by
            // name — Phase 164's posture), so plant the
            // pair exactly as a Z3 install writes it:
            // the bundle files byte-identical to the
            // catalog's, the receipts recording the
            // bundles' own standings.
            let mut rows = Vec::new();
            for name in ["is_widget_code", "weights_sorted"] {
                let bytes = std::fs::read(
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("registry")
                        .join("bundles")
                        .join(format!("{name}.bundle.json")),
                )
                .expect("the catalog bundle");
                std::fs::write(dir.join(format!("{name}.bundle.json")), &bytes).unwrap();
                let bundle: serde_json::Value =
                    serde_json::from_slice(&bytes).expect("the catalog bundle parses");
                rows.push(serde_json::json!({
                    "entry": name,
                    "version": "1.7.0",
                    "sha256": morphic::registry::sha256_hex(&bytes),
                    "tier": bundle["tier"],
                    "verdictBasis": bundle["verdictBasis"],
                    "verdict": bundle["verdict"],
                    "crownRoute": bundle["crownRoute"],
                    "battery": bundle["battery"],
                    "index": "planted exactly as install writes it (phase165 pin)",
                    "installed": "2026-11-26",
                }));
            }
            rows.sort_by(|a, b| a["entry"].as_str().cmp(&b["entry"].as_str()));
            let ledger = serde_json::json!({
                "format": morphic::registry::LEDGER_FORMAT,
                "entries": rows,
            });
            std::fs::write(
                dir.join("installed.json"),
                serde_json::to_string_pretty(&ledger).unwrap(),
            )
            .unwrap();
        }
        std::fs::write(dir.join("is_widget_code.bundle.json"), b"garbage, not a bundle")
            .unwrap();
        std::env::set_var("MORPHIC_REGISTRY_DIR", &dir);
    });
}

#[test]
fn p165_one_damaged_bundle_degrades_by_name_only() {
    init();
    let specs = stdlib::prelude_specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    let stdlib = stdlib::stdlib_specs();
    let stdlib_names: Vec<&str> = stdlib.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        &names[..35],
        &stdlib_names[..],
        "the stdlib layer stands whole, first"
    );
    assert_eq!(
        &names[35..],
        ["weights_sorted"],
        "the undamaged installed entry still joins the prelude"
    );
    assert!(
        stdlib::shipped_body("weights_sorted").is_some(),
        "the undamaged entry's body answers"
    );
    assert!(
        stdlib::shipped_body("is_widget_code").is_none(),
        "the damaged entry contributes no body"
    );
    let diagnostics = stdlib::installed_diagnostics();
    assert!(
        diagnostics.iter().any(|d| d.contains("is_widget_code")),
        "a diagnostic names the degraded entry: {diagnostics:?}"
    );
    assert!(
        !diagnostics.iter().any(|d| d.contains("weights_sorted")),
        "no diagnostic blames the undamaged entry: {diagnostics:?}"
    );
}
