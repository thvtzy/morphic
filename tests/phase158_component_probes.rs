//! Phase 158 — scope-audit probes for the 1.6 WASM
//! Component Model milestone, named as the next rung in
//! DISEBAR_SCOPE §2 and first measured in DISEBAR_AUDIT
//! §2. This phase re-measures that section against the
//! v1.5.0 tree, prices the toolchain the pipeline needs,
//! and settles the facts the 1.6 freeze will take.
//!
//! Every test prints a `P158|...` line with the measured
//! standing; assertions pin exactly what the line
//! reports. Sandbox-local standings (installed targets,
//! pinned tool binaries under ~/workspace/tools) are
//! asserted where they are checkable and reported where
//! they are not, in the standing probe discipline.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_repo(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// The playground pipeline is a core-module pipeline:
/// wasm-bindgen over wasm32-unknown-unknown, and no WIT
/// file exists anywhere in the tree — the component
/// pipeline is entire-missing, as DISEBAR_AUDIT §2
/// measured and this phase re-verifies.
///
/// RETIRED IN PLACE at Phase 159 (the standing
/// retirement discipline — Phase 142's R10 precedent,
/// as Phase 155 retired the LSP stub probe and Phase
/// 156 the VS Code greenfield probe): Phase 159 built
/// exactly the crate and WIT this probe measured as
/// absent, so the pin now records the standing that
/// replaced the audit's — the playground half stands
/// untouched; the tree's one WIT file is the component
/// crate's, at the home the audit recommended. The
/// audit document itself is untouched.
///
/// RETIRED AGAIN IN PLACE at Phase 160, same
/// discipline: Phase 160 built the callback component
/// (DISEBAR_16_SCOPE §5), whose world is the tree's
/// second WIT file — so the pin now records exactly
/// two WIT files, the library component's and the
/// callback component's, each at its frozen home.
#[test]
fn p158_pipeline_standing_retired_at_160() {
    let build = read_repo("playground/build.sh");
    assert!(
        build.contains("wasm-bindgen"),
        "playground build is the wasm-bindgen pipeline"
    );
    let mut wit_files: Vec<String> = Vec::new();
    fn scan(dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if name != "target" && name != "node_modules" && !name.starts_with('.') {
                    scan(&path, out);
                }
            } else if name.ends_with(".wit") {
                out.push(path.to_string_lossy().into_owned());
            }
        }
    }
    scan(&repo_root(), &mut wit_files);
    println!(
        "P158|pipeline|bindgen=core-module wit_files={} (standing replaced at Phases 159+160)",
        wit_files.len()
    );
    wit_files.sort();
    assert_eq!(
        wit_files.len(),
        2,
        "exactly two WIT files stand in-tree — the two component crates': {wit_files:?}"
    );
    assert!(
        wit_files[0].ends_with("packages/component-callbacks/wit/world.wit")
            && wit_files[1].ends_with("packages/component/wit/world.wit"),
        "the WIT files are the callback component's and the library component's worlds: {wit_files:?}"
    );
    assert!(
        !repo_root().join("crates/morphic-component").exists()
            && repo_root().join("packages/component").exists()
            && repo_root().join("packages/component-callbacks").exists(),
        "the component crates stand at packages/component + packages/component-callbacks (Phases 159+160), not in crates/"
    );
}

/// The one target the component pipeline needs beyond
/// the standing set: wasm32-wasip2. Measured by the
/// sysroot's rustlib directory (no rustup dependency).
/// At campaign start only wasm32-unknown-unknown and
/// the host target were installed; the audit added
/// wasip2 via `rustup target add` in 37 s.
#[test]
fn p158_wasip2_target_installed() {
    let out = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("rustc --print sysroot");
    let sysroot = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let rustlib = Path::new(&sysroot).join("lib/rustlib");
    let mut installed: Vec<String> = std::fs::read_dir(&rustlib)
        .expect("read rustlib")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains('-'))
        .collect();
    installed.sort();
    println!("P158|targets|installed={}", installed.join(","));
    assert!(
        installed.iter().any(|t| t == "wasm32-wasip2"),
        "wasm32-wasip2 must be installed for the component pipeline"
    );
    assert!(
        installed.iter().any(|t| t == "wasm32-unknown-unknown"),
        "the playground target stands installed"
    );
}

/// The pinned prebuilt tools the audit installed under
/// ~/workspace/tools (the Binaryen precedent): wasm-tools
/// and wasmtime CLI. Where the binary is present its
/// version must match the pin exactly; absence is
/// reported, not asserted — these are sandbox standings,
/// not repo facts, and CI does not carry them.
#[test]
fn p158_pinned_component_tools() {
    let home = std::env::var("HOME").expect("HOME");
    for (rel, want) in [
        ("workspace/tools/wasm-tools-1.263.0/wasm-tools", "wasm-tools 1.263.0"),
        ("workspace/tools/wasmtime-51.0.0/wasmtime", "wasmtime 51.0.0"),
    ] {
        let path = Path::new(&home).join(rel);
        if path.exists() {
            let out = Command::new(&path)
                .arg("--version")
                .output()
                .unwrap_or_else(|e| panic!("run {}: {e}", path.display()));
            let version = String::from_utf8(out.stdout).unwrap().trim().to_string();
            println!("P158|tool|{rel} = {version}");
            assert!(
                version.starts_with(want),
                "pinned version: want {want}, got {version}"
            );
        } else {
            println!("P158|tool|{rel} ABSENT in this environment");
        }
    }
}

/// The facade surface the component would wrap, counted
/// from the shipped manifest (the same rows PCC's
/// bundles evidence): 35 exports, exactly five carrying
/// a function-typed parameter — the G4 fork's whole
/// subject — and all five standing Tier-1.
#[test]
fn p158_facade_surface_inventory() {
    let text = read_repo("packages/rust/morphic.manifest.json");
    let manifest: serde_json::Value = serde_json::from_str(&text).expect("manifest parses");
    let exports = manifest["exports"].as_array().expect("exports array");
    assert_eq!(exports.len(), 35, "the frozen surface is 35 exports");
    let mut ho: Vec<String> = Vec::new();
    let mut tier1 = 0usize;
    let mut tier2: Vec<String> = Vec::new();
    for export in exports {
        let name = export["name"].as_str().unwrap().to_string();
        let signature = export["signature"].as_str().unwrap();
        if signature.contains("fn(") {
            ho.push(name.clone());
            assert_eq!(
                export["tier"].as_str().unwrap(),
                "tier-1",
                "every higher-order export stands Tier-1: {name}"
            );
        }
        match export["tier"].as_str().unwrap() {
            "tier-1" => tier1 += 1,
            "tier-2" => tier2.push(name),
            other => panic!("unexpected tier {other}"),
        }
    }
    ho.sort();
    tier2.sort();
    println!(
        "P158|surface|exports=35 ho={} tier1={} tier2={}",
        ho.join(","),
        tier1,
        tier2.join(",")
    );
    assert_eq!(
        ho,
        ["list_all", "list_any", "list_filter", "list_fold", "list_map"]
    );
    assert_eq!(tier1, 33);
    assert_eq!(tier2, ["list_binary_search", "list_sort"]);
}

/// EMBED_SCOPE §3's WIT column covers every type
/// constructor in the frozen surface except one: the
/// Function-types row, written before G4, states no
/// mapping exists. The 1.6 freeze must take that fork
/// in the open — this pin holds the §3 text so the
/// amendment is a deliberate act, not a reinterpretation.
#[test]
fn p158_embed_scope_wit_column_and_the_function_row() {
    let scope = read_repo("EMBED_SCOPE.md");
    for wit in ["`s64`", "`char`", "`string`", "`list<T>`", "`option<T>`", "`result<T, E>`"] {
        assert!(scope.contains(wit), "§3 WIT column carries {wit}");
    }
    let fn_row = scope
        .lines()
        .find(|l| l.starts_with("| Function types"))
        .expect("the Function types row exists");
    println!("P158|wit|function-row={}", fn_row.trim());
    assert!(
        fn_row.contains("No mapping exists"),
        "§3's Function-types row predates G4 and says so"
    );
    assert!(
        scope.contains("code point"),
        "§3 names the code-point length rule the WIT boundary re-measures"
    );
}
