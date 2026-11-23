//! Phase 122 (1.1 Embeddable, EMBED_SCOPE §5): npm
//! package assembly + publish-readiness (dry-run only,
//! R5). The pins:
//!
//! * the assembly is deterministic — two assemblies are
//!   byte-identical file sets, in the same order;
//! * the checked-in package in `packages/npm/` equals a
//!   fresh assembly byte-for-byte, file set included —
//!   no drift and no stray files (the Phase 112 asset
//!   discipline; the tarball hygiene the release will
//!   publish from is this pin's subject);
//! * `package.json` carries the frozen shape: the scoped
//!   name (D8a), ESM-only (R7), the version lockstep —
//!   the crate's version, which Phase 125 stamps to
//!   1.1.0 by bumping the crate and re-running the
//!   assembly — an exports map naming the root module,
//!   all 35 per-function subpaths (§2.2) and the
//!   manifest, every target present in the file set;
//! * the license files are the repo's own texts, byte
//!   for byte (the project is dual-licensed
//!   "MIT OR Apache-2.0" and the package carries both);
//! * per-function modules re-export exactly their entry
//!   from the root module;
//! * the assembled unpacked mass stands inside the
//!   frozen 76,800 B budget (Amendment 2 §A2.5 —
//!   clause 6's npm half, re-derived at Phase 134);
//! * the assembly's crown checks have teeth: the Tier-1
//!   check accepts the standing shipped body and refuses
//!   a moved one; the Tier-2 check accepts the shipped
//!   body and refuses a wrong one. (The full re-run over
//!   all 35 entries is the CLI's `morphic build
//!   --package`, exercised at assembly time; the standing
//!   whole-surface crowning pin is Phase 110's.)
//!
//! Configuration note (the standing one): pins that read
//! the NATIVE prelude are gated
//! `#[cfg(not(feature = "wasm-playground"))]`.

#[cfg(not(feature = "wasm-playground"))]
use std::collections::BTreeMap;
#[cfg(not(feature = "wasm-playground"))]
use std::path::PathBuf;

#[cfg(not(feature = "wasm-playground"))]
use morphic::codegen::package::{self, PackageFile};
#[cfg(not(feature = "wasm-playground"))]
use morphic::spec::parser;
#[cfg(not(feature = "wasm-playground"))]
use morphic::stdlib;
#[cfg(not(feature = "wasm-playground"))]
use morphic::synthesis::engine::{IRLiteral, IRNode};

#[cfg(not(feature = "wasm-playground"))]
fn assembled() -> BTreeMap<String, String> {
    package::assemble_npm_package()
        .expect("the package assembles")
        .into_iter()
        .map(|f| (f.path, f.contents))
        .collect()
}

#[cfg(not(feature = "wasm-playground"))]
fn package_home() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(package::NPM_PACKAGE_DIR)
}

// ── Determinism ──────────────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn assembly_is_deterministic() {
    let a: Vec<PackageFile> = package::assemble_npm_package().expect("assembly runs");
    let b: Vec<PackageFile> = package::assemble_npm_package().expect("assembly re-runs");
    assert_eq!(a, b, "two assemblies are byte-identical file sets");
    // The file set's shape: 4 furniture texts + the 3
    // binding artifacts + 2 shims per export.
    assert_eq!(a.len(), 4 + 3 + 2 * 35, "77 files in the assembled package");
}

// ── The checked-in package ───────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn checked_in_package_matches_fresh_assembly() {
    let fresh = assembled();
    let home = package_home();
    let mut on_disk: BTreeMap<String, String> = BTreeMap::new();
    for entry in std::fs::read_dir(&home).expect("the package home exists") {
        let entry = entry.expect("dir entry");
        assert!(
            entry.file_type().expect("file type").is_file(),
            "the package home holds files only (no directories): {}",
            entry.path().display()
        );
        let name = entry.file_name().to_string_lossy().into_owned();
        let bytes = std::fs::read(entry.path()).expect("package file reads");
        on_disk.insert(
            name,
            String::from_utf8(bytes).expect("package files are text"),
        );
    }
    assert_eq!(
        on_disk, fresh,
        "the checked-in package is the fresh assembly, byte-for-byte, \
         with no stray files (regenerate with `morphic build --package`)"
    );
}

// ── package.json ─────────────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn package_json_carries_the_frozen_shape() {
    let files = assembled();
    let json: serde_json::Value =
        serde_json::from_str(&files["package.json"]).expect("package.json parses");
    assert_eq!(json["name"], "@thvtzy/morphic", "the scoped name (D8a)");
    // The lockstep (R6): package.json's version, the
    // manifest's two version fields and the crate's
    // version are one value from one source — Phase 125
    // stamps 1.1.0 by moving that source alone.
    assert_eq!(
        json["version"],
        env!("CARGO_PKG_VERSION"),
        "package version is the crate's version"
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&files["morphic.manifest.json"]).expect("manifest parses");
    assert_eq!(manifest["packageVersion"], json["version"]);
    assert_eq!(manifest["morphicVersion"], json["version"]);
    assert_eq!(json["type"], "module", "ESM-only (R7)");
    assert_eq!(json["sideEffects"], false, "tree-shakeable modules");
    assert_eq!(json["license"], "MIT OR Apache-2.0");
    assert_eq!(json["main"], "./index.js");
    assert_eq!(json["types"], "./index.d.ts");

    // The exports map: the root, one subpath per frozen
    // export, the manifest and package.json — and every
    // target it names exists in the file set.
    let exports = json["exports"].as_object().expect("exports map");
    assert_eq!(
        exports.len(),
        1 + 35 + 2,
        "root + 35 subpaths + manifest + package.json"
    );
    assert_eq!(exports["."]["types"], "./index.d.ts");
    assert_eq!(exports["."]["import"], "./index.js");
    assert_eq!(
        exports["./morphic.manifest.json"], "./morphic.manifest.json",
        "the manifest is addressable in the package"
    );
    for row in manifest["exports"].as_array().expect("manifest rows") {
        let name = row["name"].as_str().expect("row name");
        let sub = &exports[&format!("./{name}")];
        assert_eq!(sub["types"], format!("./{name}.d.ts"), "{name}: types target");
        assert_eq!(sub["import"], format!("./{name}.js"), "{name}: import target");
    }
    for (subpath, target) in exports {
        let mut targets: Vec<&str> = Vec::new();
        match target {
            serde_json::Value::String(s) => targets.push(s),
            serde_json::Value::Object(map) => {
                for v in map.values() {
                    targets.push(v.as_str().expect("export target is a path"));
                }
            }
            other => panic!("exports[{subpath}] has unexpected shape: {other}"),
        }
        for t in targets {
            let rel = t.strip_prefix("./").expect("targets are package-relative");
            assert!(
                files.contains_key(rel),
                "exports[{subpath}] target {t} exists in the file set"
            );
        }
    }
    // The files bound: the tarball is the package's own
    // files. README rides npm's always-included set; the
    // license texts are listed explicitly (npm's
    // always-include does not match the suffixed
    // dual-license names — measured at the dry-run).
    let files_field: Vec<&str> = json["files"]
        .as_array()
        .expect("files field")
        .iter()
        .map(|v| v.as_str().expect("files entry"))
        .collect();
    assert_eq!(
        files_field,
        ["*.js", "*.d.ts", "morphic.manifest.json", "LICENSE-MIT", "LICENSE-APACHE"]
    );
}

// ── Licenses ─────────────────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn license_files_are_the_repo_texts() {
    let files = assembled();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for name in ["LICENSE-MIT", "LICENSE-APACHE"] {
        let repo_text = std::fs::read_to_string(root.join(name)).expect("repo license reads");
        assert_eq!(
            files[name], repo_text,
            "{name} in the package is the repo's text, byte for byte"
        );
    }
}

// ── Per-function modules ─────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn per_function_modules_reexport_their_entry() {
    let files = assembled();
    let manifest: serde_json::Value =
        serde_json::from_str(&files["morphic.manifest.json"]).expect("manifest parses");
    for row in manifest["exports"].as_array().expect("manifest rows") {
        let name = row["name"].as_str().expect("row name");
        let shim = format!("export {{ {name} }} from \"./index.js\";\n");
        assert_eq!(
            files[&format!("{name}.js")], shim,
            "{name}.js re-exports its entry from the root module"
        );
        assert_eq!(
            files[&format!("{name}.d.ts")], shim,
            "{name}.d.ts re-exports its entry from the root module"
        );
    }
}

// ── The budget (clause 6's npm half) ─────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn unpacked_mass_inside_the_frozen_budget() {
    let files: Vec<PackageFile> =
        package::assemble_npm_package().expect("the package assembles");
    let total = package::unpacked_mass(&files);
    let by_name: BTreeMap<&str, usize> =
        files.iter().map(|f| (f.path.as_str(), f.contents.len())).collect();
    let generated = by_name["index.js"] + by_name["index.d.ts"];
    let manifest = by_name["morphic.manifest.json"];
    let furniture = total - generated - manifest;
    println!(
        "P122MASS|total={total}|generated={generated}|manifest={manifest}|furniture+shims={furniture}|budget={}",
        package::NPM_UNPACKED_BUDGET
    );
    assert!(
        total <= package::NPM_UNPACKED_BUDGET,
        "the assembled package is {total} B unpacked, inside the frozen {} B budget",
        package::NPM_UNPACKED_BUDGET
    );
}

// ── The crown checks' teeth ──────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn tier2_check_accepts_the_shipped_body_and_refuses_a_wrong_one() {
    let spec = stdlib::prelude_specs()
        .into_iter()
        .find(|s| s.name == "list_sum")
        .expect("list_sum in prelude");
    let shipped = stdlib::shipped_body("list_sum").expect("list_sum ships a body");
    assert!(
        package::check_tier2_body(&spec, &shipped).is_ok(),
        "the shipped Tier-2 body stands executed-exact"
    );
    let wrong = IRNode::Literal(IRLiteral::Int(0));
    assert!(
        package::check_tier2_body(&spec, &wrong).is_err(),
        "a wrong body (constant 0) is refused by name"
    );
}

#[cfg(not(feature = "wasm-playground"))]
#[cfg(feature = "z3-support")]
#[test]
fn crown_check_refuses_a_moved_crown() {
    // int_abs's library module + spec, the Phase 110
    // pin's own materials.
    let mut found = None;
    for (_, source) in stdlib::LIBRARY_MODULES {
        let module = parser::parse(source).expect("library module parses");
        if let Some(spec) = module.functions.iter().find(|f| f.name == "int_abs") {
            found = Some((source, spec.clone()));
            break;
        }
    }
    let (source, spec) = found.expect("int_abs in the library");
    let shipped = stdlib::shipped_body("int_abs").expect("int_abs ships a body");
    assert!(
        package::check_tier1_crown(source, &spec, &shipped).is_ok(),
        "the standing crown is accepted"
    );
    let moved = IRNode::Var("x".into());
    let refusal = package::check_tier1_crown(source, &spec, &moved)
        .expect_err("a moved crown (the identity body) is refused");
    assert!(
        refusal.contains("int_abs"),
        "the refusal names the entry: {refusal}"
    );
}
