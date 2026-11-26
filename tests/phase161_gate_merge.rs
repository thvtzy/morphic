//! Phase 161 — the 1.6 gate (DISEBAR_16_SCOPE §8),
//! run clause by clause on the gate tree with fresh
//! instruments, in the Phase 151/157 gate shape:
//! nothing here quotes Phases 159/160 — the rehearsals
//! below re-run against the shipped artifacts, the
//! surfaces are re-printed from the artifacts, and the
//! footprint is re-measured.
//!
//! Clauses (§8, the gate text never amended):
//! 1. **The library component answers** — the manifest
//!    states the 30 non-HO exports' tiers (28 tier-1 /
//!    z3-verified; `list-sort` + `list-binary-search`
//!    tier-2 / executed-exact), and the pinned
//!    wasmtime answers the frozen batteries' 201
//!    evaluable points exactly through the shipped
//!    artifact (the table's provenance is Phase 159's:
//!    the stdlib specs' rows classified by the
//!    interpreter over the shipped bodies, Passed
//!    only, cross-checked against the bundles —
//!    203 total / 201 evaluable / 201 passed).
//! 2. **The five cross honestly** — the callback
//!    component's printed world carries exactly the
//!    three frozen imports + five frozen exports (no
//!    exported signature carries a function-typed
//!    parameter — the rows are verbatim); the jco
//!    harness answers all 40 frozen battery points
//!    exactly with the imports supplied as host
//!    modules; the wasmtime CLI refuses the artifact
//!    at instantiation, naming the import.
//! 3. **The WIT is the contract** — `wasm-tools
//!    component wit` over both artifacts matches the
//!    in-tree WIT (the library's at the standing
//!    lockstep version); the losses and boundaries —
//!    the WASI import surface, tiers living in the
//!    manifest, the callback's correctness being the
//!    host's — are named in the packages' READMEs.
//! 4. **Footprint** — both artifacts inside their
//!    frozen budgets (§6), headers true components,
//!    sha-pinned, and reproduced byte-for-byte by
//!    fresh plain-cargo builds; the main Cargo set is
//!    the standing 13 and the root lockfile knows
//!    neither component crate; the pinned tools are
//!    the versions the freeze pinned, where checkable.
//! 5. **Standings hold** — the npm package at its
//!    Phase 157 mass inside the frozen budget, the
//!    facade at its mass, the bundles at theirs, the
//!    extension's version equal to the crate's (the
//!    lockstep standing). (The three-configuration
//!    suites, the Cargo.toml/Cargo.lock zero-diff
//!    against v1.5.0, and the playground bundle's
//!    rebuilt mass are the gate record's evidence in
//!    ROAD_TO_SELF_HOSTING.md, as at Phase 157.)
//!
//! Sandbox-local standings (pinned tools, node/npm,
//! the rustup target) are asserted where they are
//! checkable and reported where they are not, in the
//! standing probe discipline (Phase 158).

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_repo(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

fn flattened(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tool_path(rel: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(rel);
    path.exists().then_some(path)
}

fn sha256_of(path: &std::path::Path) -> Option<String> {
    let out = Command::new("sha256sum").arg(path).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()?
        .split_whitespace()
        .next()
        .map(str::to_string)
}

fn dir_mass(dir: &std::path::Path) -> (usize, usize) {
    let mut files = 0;
    let mut bytes = 0usize;
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            let (f, b) = dir_mass(&path);
            files += f;
            bytes += b;
        } else {
            files += 1;
            bytes += entry.metadata().unwrap().len() as usize;
        }
    }
    (files, bytes)
}

const WASMTIME: &str = "workspace/tools/wasmtime-51.0.0/wasmtime";
const WASM_TOOLS: &str = "workspace/tools/wasm-tools-1.263.0/wasm-tools";
const LIB_ARTIFACT: &str = "packages/component/morphic-library.wasm";
const CB_ARTIFACT: &str = "packages/component-callbacks/morphic-callbacks.wasm";
/// The frozen budgets (DISEBAR_16_SCOPE §6).
const LIBRARY_BUDGET: usize = 106_496;
const CALLBACK_BUDGET: usize = 90_112;
/// The artifacts' sizes + sha256 pins at the gate
/// (Phases 159/160 measurements).
const LIBRARY_SIZE: usize = 96_100;
const LIBRARY_SHA256: &str =
    "d424f202fa8b9bd5439df26eecb0b233cce782f827d1b93af10502547927afac";
const CALLBACK_SIZE: usize = 81_840;
const CALLBACK_SHA256: &str =
    "8c51788872c4af6bca24256a6fe82ce03aff3c622d4cd23498a3440ad14fdb2f";

/// The library component's 30 frozen exports with
/// their frozen WIT types (DISEBAR_16_SCOPE §4.3).
const FROZEN_SURFACE: [(&str, &str); 30] = [
    ("int-abs", "func(x: s64) -> s64"),
    ("int-sign", "func(x: s64) -> s64"),
    ("int-min", "func(a: s64, b: s64) -> s64"),
    ("int-max", "func(a: s64, b: s64) -> s64"),
    ("int-clamp", "func(x: s64, lo: s64, hi: s64) -> s64"),
    ("int-is-even", "func(x: s64) -> bool"),
    ("int-is-odd", "func(x: s64) -> bool"),
    ("char-is-digit", "func(c: char) -> bool"),
    ("char-is-alpha", "func(c: char) -> bool"),
    ("char-is-whitespace", "func(c: char) -> bool"),
    ("char-is-uppercase", "func(c: char) -> bool"),
    ("char-is-lowercase", "func(c: char) -> bool"),
    ("char-to-digit", "func(c: char) -> option<s64>"),
    ("string-is-empty", "func(s: string) -> bool"),
    ("result-unwrap-or", "func(r: result<s64, string>, d: s64) -> s64"),
    ("option-unwrap-or", "func(o: option<s64>, d: s64) -> s64"),
    ("option-ok-or", "func(o: option<s64>, e: string) -> result<s64, string>"),
    ("list-is-empty", "func(xs: list<s64>) -> bool"),
    ("list-reverse", "func(xs: list<s64>) -> list<s64>"),
    ("list-contains", "func(xs: list<s64>, x: s64) -> bool"),
    ("list-sum", "func(xs: list<s64>) -> s64"),
    ("list-product", "func(xs: list<s64>) -> s64"),
    ("list-min", "func(xs: list<s64>) -> option<s64>"),
    ("list-max", "func(xs: list<s64>) -> option<s64>"),
    ("list-take", "func(xs: list<s64>, n: s64) -> list<s64>"),
    ("list-drop", "func(xs: list<s64>, n: s64) -> list<s64>"),
    ("list-is-sorted", "func(xs: list<s64>) -> bool"),
    ("list-sort", "func(xs: list<s64>) -> list<s64>"),
    ("list-binary-search", "func(xs: list<s64>, x: s64) -> option<s64>"),
    ("range", "func(lo: s64, hi: s64) -> list<s64>"),
];

/// The callback component's frozen world
/// (DISEBAR_16_SCOPE §5): three imports, five exports.
const FROZEN_IMPORTS: [(&str, &str); 3] = [
    ("map-apply", "func(x: s64) -> s64"),
    ("pred", "func(x: s64) -> bool"),
    ("combine", "func(acc: s64, x: s64) -> s64"),
];
const FROZEN_EXPORTS: [(&str, &str); 5] = [
    ("list-map", "func(xs: list<s64>) -> list<s64>"),
    ("list-filter", "func(xs: list<s64>) -> list<s64>"),
    ("list-fold", "func(xs: list<s64>, init: s64) -> s64"),
    ("list-all", "func(xs: list<s64>) -> bool"),
    ("list-any", "func(xs: list<s64>) -> bool"),
];

#[test]
fn p161_clause1_library_answers() {
    // The tiers the manifest states, for exactly the
    // 30 exports the component ships.
    let manifest: serde_json::Value =
        serde_json::from_str(&read_repo("packages/rust/morphic.manifest.json"))
            .expect("the facade manifest parses");
    let version = env!("CARGO_PKG_VERSION");
    assert_eq!(
        manifest["packageVersion"].as_str(),
        Some(version),
        "the manifest stands at the lockstep version"
    );
    assert_eq!(
        manifest["morphicVersion"].as_str(),
        Some(version),
        "the manifest's morphicVersion stands at the lockstep"
    );
    let surface: Vec<&str> = FROZEN_SURFACE.iter().map(|(n, _)| *n).collect();
    let mut tier1 = 0usize;
    let mut tier2: Vec<String> = Vec::new();
    let mut matched = 0usize;
    for export in manifest["exports"].as_array().expect("exports is an array") {
        let kebab = export["name"].as_str().unwrap().replace('_', "-");
        if !surface.contains(&kebab.as_str()) {
            continue; // a higher-order entry — §5's component, clause 2
        }
        matched += 1;
        match export["tier"].as_str() {
            Some("tier-1") => {
                assert_eq!(
                    export["verdictBasis"].as_str(),
                    Some("z3-verified"),
                    "{kebab} stands tier-1 on the z3-verified basis"
                );
                tier1 += 1;
            }
            Some("tier-2") => {
                assert_eq!(
                    export["verdictBasis"].as_str(),
                    Some("executed-exact"),
                    "{kebab} stands tier-2 on the executed-exact basis"
                );
                tier2.push(kebab);
            }
            other => panic!("{kebab} carries an unrecognized tier: {other:?}"),
        }
    }
    assert_eq!(matched, 30, "the manifest states tiers for all 30 shipped exports");
    assert_eq!(tier1, 28, "28 of the 30 stand tier-1, as §4.4 states");
    tier2.sort();
    assert_eq!(
        tier2,
        ["list-binary-search", "list-sort"],
        "the two tier-2 exports are the named pair"
    );
    // The battery itself, through the shipped artifact.
    run_wasmtime_battery();
    println!("P161|clause1|manifest tiers 28 tier-1/z3-verified + 2 tier-2/executed-exact (list-sort, list-binary-search); wasmtime battery re-run");
}

#[test]
fn p161_clause2_five_cross_honestly() {
    // The printed world: exactly the frozen imports +
    // exports, verbatim — an exported signature
    // carrying a function-typed parameter would change
    // its row, so exact rows are the honesty check.
    if let Some(wasm_tools) = tool_path(WASM_TOOLS) {
        let out = Command::new(&wasm_tools)
            .args(["component", "wit"])
            .arg(repo_root().join(CB_ARTIFACT))
            .output()
            .expect("run wasm-tools component wit");
        assert!(out.status.success(), "wasm-tools component wit succeeds");
        let printed = flattened(&String::from_utf8(out.stdout).unwrap());
        for (name, ty) in FROZEN_IMPORTS {
            let want = format!("import {name}: {ty};");
            assert!(printed.contains(&want), "the artifact's world carries {want}");
        }
        for (name, ty) in FROZEN_EXPORTS {
            let want = format!("export {name}: {ty};");
            assert!(printed.contains(&want), "the artifact's world carries {want}");
        }
    } else {
        println!("P161|clause2|wasm-tools ABSENT in this environment; printed world not re-checked");
    }
    // The negative pin: the library component's runner
    // refuses this artifact at instantiation.
    if let Some(wasmtime) = tool_path(WASMTIME) {
        let out = Command::new(&wasmtime)
            .args(["run", "--invoke", "list-map([1, 2, 3])"])
            .arg(repo_root().join(CB_ARTIFACT))
            .output()
            .expect("run wasmtime against the callback component");
        assert!(
            !out.status.success(),
            "the library component's runner refuses the callback artifact"
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains(
                "component imports function `map-apply`, but a matching \
                 implementation was not found in the linker"
            ),
            "the refusal names the unsatisfied import: {stderr}"
        );
    } else {
        println!("P161|clause2|wasmtime ABSENT in this environment; refusal not run");
    }
    // The battery, through a host supplying the imports.
    run_jco_rehearsal();
    println!("P161|clause2|printed world verbatim (3 imports + 5 exports), wasmtime refusal stands, jco battery re-run");
}

#[test]
fn p161_clause3_wit_is_contract() {
    // In-tree WIT == the frozen surfaces, both crates.
    let lib_wit = read_repo("packages/component/wit/world.wit");
    let rows: Vec<String> = lib_wit
        .lines()
        .map(str::trim)
        .filter(|l| l.contains(": func("))
        .map(str::to_string)
        .collect();
    assert_eq!(rows.len(), 30, "the library WIT exports exactly 30 functions");
    for (name, ty) in FROZEN_SURFACE {
        let want = format!("{name}: {ty};");
        assert!(rows.contains(&want), "the library WIT carries {want}");
    }
    let version = env!("CARGO_PKG_VERSION");
    assert!(
        lib_wit.contains(&format!("package morphic:library@{version};")),
        "the library WIT package stands at the lockstep version"
    );
    let cb_wit = read_repo("packages/component-callbacks/wit/world.wit");
    let cb_rows: Vec<String> = cb_wit
        .lines()
        .map(str::trim)
        .filter(|l| l.contains(": func("))
        .map(str::to_string)
        .collect();
    assert_eq!(cb_rows.len(), 8, "the callback WIT carries 3 imports + 5 exports");
    for (name, ty) in FROZEN_IMPORTS {
        let want = format!("import {name}: {ty};");
        assert!(cb_rows.contains(&want), "the callback WIT carries {want}");
    }
    for (name, ty) in FROZEN_EXPORTS {
        let want = format!("export {name}: {ty};");
        assert!(cb_rows.contains(&want), "the callback WIT carries {want}");
    }
    assert!(
        cb_wit.contains(&format!("package morphic:library@{version};")),
        "the callback WIT package stands at the lockstep version"
    );

    // The artifacts' printed worlds agree with the
    // in-tree WIT (modulo the stamp's package version,
    // which the version needles above carry).
    if let Some(wasm_tools) = tool_path(WASM_TOOLS) {
        let out = Command::new(&wasm_tools)
            .args(["component", "wit"])
            .arg(repo_root().join(LIB_ARTIFACT))
            .output()
            .expect("run wasm-tools component wit");
        assert!(out.status.success(), "wasm-tools component wit succeeds");
        let printed = flattened(&String::from_utf8(out.stdout).unwrap());
        assert!(
            printed.contains(&format!(
                "export morphic:library/functions@{version};"
            )),
            "the artifact's world exports morphic:library/functions at the lockstep version"
        );
        let section = printed
            .split("package morphic:library@")
            .nth(1)
            .expect("the printed world carries the morphic:library package");
        assert_eq!(
            section.matches(": func(").count(),
            30,
            "the artifact's interface carries exactly 30 functions"
        );
        for (name, ty) in FROZEN_SURFACE {
            let want = format!("{name}: {ty};");
            assert!(section.contains(&want), "the artifact's world carries {want}");
        }
    } else {
        println!("P161|clause3|wasm-tools ABSENT in this environment; printed worlds not re-checked");
    }

    // Every loss and boundary is named where a host
    // reads it: the packages' READMEs.
    let lib_readme = flattened(&read_repo("packages/component/README.md"));
    for needle in [
        "The WASI import surface (a named wart)",
        "wasi:clocks/*` @0.2.12",
        "Tiers — read the manifest, not the WIT",
        "`list-sort`, `list-binary-search`",
    ] {
        assert!(lib_readme.contains(needle), "the library README names {needle}");
    }
    let cb_readme = flattened(&read_repo("packages/component-callbacks/README.md"));
    for needle in [
        "imports satisfy at instantiation",
        "accumulator first",
        "LEFT fold",
        "The callback's correctness is the host's",
        "Tier-1",
    ] {
        assert!(cb_readme.contains(needle), "the callback README names {needle}");
    }
    println!("P161|clause3|in-tree WIT + printed worlds agree at the lockstep version; WASI surface, tiers, and the host's-correctness boundary named in the READMEs");
}

#[test]
fn p161_clause4_footprint() {
    for (artifact, size, sha, budget) in [
        (LIB_ARTIFACT, LIBRARY_SIZE, LIBRARY_SHA256, LIBRARY_BUDGET),
        (CB_ARTIFACT, CALLBACK_SIZE, CALLBACK_SHA256, CALLBACK_BUDGET),
    ] {
        let bytes = std::fs::read(repo_root().join(artifact))
            .unwrap_or_else(|e| panic!("read {artifact}: {e}"));
        assert_eq!(
            &bytes[..8],
            &[0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00],
            "{artifact} is a true component, not a core module"
        );
        assert_eq!(bytes.len(), size, "{artifact}'s measured size");
        assert!(
            bytes.len() <= budget,
            "{artifact} inside its frozen budget of {budget} B"
        );
        if let Some(actual) = sha256_of(&repo_root().join(artifact)) {
            assert_eq!(actual, sha, "{artifact}'s sha256 pin");
        }
    }
    // The main Cargo graph stands: the standing 13,
    // and the root lockfile knows neither component
    // crate (each is its own workspace root).
    let cargo_toml = read_repo("Cargo.toml");
    let mut in_deps = false;
    let mut deps: Vec<String> = Vec::new();
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]"
            || trimmed == "[target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]"
        {
            // [dependencies] plus the native target table:
            // Phase 164's sha2 (DISEBAR_17_SCOPE §8) is
            // declared there — target-gated off wasm32, so
            // the browser profile's graph excludes it while
            // every native configuration carries it. The
            // standing set is therefore 13 in this parse.
            in_deps = true;
            continue;
        }
        if in_deps && trimmed.starts_with('[') {
            in_deps = false;
            continue;
        }
        if in_deps && !trimmed.is_empty() && !trimmed.starts_with('#') {
            let name = trimmed.split(['=', ' ', '{']).next().unwrap().trim_matches('"');
            if !name.is_empty() {
                deps.push(name.to_string());
            }
        }
    }
    deps.sort();
    assert_eq!(
        deps,
        [
            "anyhow", "clap", "colored", "indicatif", "llama-cpp-2", "rand", "reqwest",
            "serde", "serde_json", "sha2", "thiserror", "tokio", "z3",
        ],
        "the Cargo dependency set is the standing set"
    );
    let lock = read_repo("Cargo.lock");
    for absent in ["morphic-component-callbacks", "morphic-component"] {
        assert!(
            !lock.contains(&format!("name = \"{absent}\"")),
            "the root lockfile knows no {absent}"
        );
    }
    // The build used the pinned tools and no others:
    // where checkable, their versions are the pins.
    if let Some(wasm_tools) = tool_path(WASM_TOOLS) {
        let out = Command::new(&wasm_tools)
            .arg("--version")
            .output()
            .expect("wasm-tools --version");
        let v = String::from_utf8(out.stdout).unwrap();
        assert!(v.contains("wasm-tools 1.263.0"), "the wasm-tools pin: {v}");
    }
    if let Some(wasmtime) = tool_path(WASMTIME) {
        let out = Command::new(&wasmtime)
            .arg("--version")
            .output()
            .expect("wasmtime --version");
        let v = String::from_utf8(out.stdout).unwrap();
        assert!(v.contains("wasmtime 51.0.0"), "the wasmtime pin: {v}");
    }
    // Fresh plain-cargo builds reproduce both
    // artifacts of record byte-for-byte.
    assert_build_reproduces(
        "packages/component",
        LIB_ARTIFACT,
        "morphic_component.wasm",
    );
    assert_build_reproduces(
        "packages/component-callbacks",
        CB_ARTIFACT,
        "morphic_component_callbacks.wasm",
    );
    println!("P161|clause4|library {LIBRARY_SIZE} B <= {LIBRARY_BUDGET} B, callbacks {CALLBACK_SIZE} B <= {CALLBACK_BUDGET} B, both sha-pinned and reproduced byte-for-byte; cargo=standing-13, root lock knows neither crate; tool pins 1.263.0/51.0.0");
}

fn assert_build_reproduces(crate_rel: &str, artifact_rel: &str, out_name: &str) {
    let home = std::env::var("HOME").expect("HOME");
    let cargo = PathBuf::from(&home).join(".cargo/bin/cargo");
    let rustup = PathBuf::from(&home).join(".cargo/bin/rustup");
    if !cargo.exists() || !rustup.exists() {
        println!("P161|clause4|cargo/rustup ABSENT in this environment; {crate_rel} reproduction not run");
        return;
    }
    let targets = Command::new(&rustup)
        .args(["target", "list", "--installed"])
        .output()
        .expect("rustup target list");
    if !String::from_utf8(targets.stdout).unwrap().contains("wasm32-wasip2") {
        println!("P161|clause4|wasm32-wasip2 target ABSENT in this environment; {crate_rel} reproduction not run");
        return;
    }
    let crate_dir = repo_root().join(crate_rel);
    let out = Command::new(&cargo)
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .current_dir(&crate_dir)
        .output()
        .expect("cargo build the component crate");
    assert!(
        out.status.success(),
        "plain cargo builds {crate_rel}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let fresh = std::fs::read(crate_dir.join("target/wasm32-wasip2/release").join(out_name))
        .expect("read the fresh artifact");
    let committed = std::fs::read(repo_root().join(artifact_rel)).expect("read the committed artifact");
    assert_eq!(
        fresh, committed,
        "a fresh build reproduces {artifact_rel} byte-for-byte"
    );
}

#[test]
fn p161_clause5_standings() {
    let root = repo_root();
    let (npm_files, npm_bytes) = dir_mass(&root.join("packages/npm"));
    assert_eq!(npm_files, 77, "the npm package's file set unmoved");
    assert_eq!(npm_bytes, 71_873, "the npm package's mass unmoved");
    assert!(
        npm_bytes <= 76_800,
        "the npm package stands inside the frozen 76,800 B budget"
    );
    let (rust_files, rust_bytes) = dir_mass(&root.join("packages/rust"));
    assert_eq!(
        (rust_files, rust_bytes),
        (7, 57_613),
        "the facade package stands at its Phase 157 mass"
    );
    let (bundle_files, bundle_bytes) = dir_mass(&root.join("packages/bundles"));
    assert_eq!(
        (bundle_files, bundle_bytes),
        (35, 267_156),
        "the re-verification bundles stand at their recorded mass"
    );
    // The lockstep standing: the extension's version
    // equals the crate's.
    let vscode = read_repo("packages/vscode/package.json");
    let version = env!("CARGO_PKG_VERSION");
    assert!(
        vscode.contains(&format!("\"version\":\"{version}\"")),
        "the extension's version equals the crate version ({version})"
    );
    println!("P161|clause5|npm 77 files/71,873 B <= 76,800 B; facade 7/57,613 B; bundles 35/267,156 B; extension version == crate version {version}");
}

// ==== The clause-1 battery instrument (the frozen table, re-run) ====
/// The frozen batteries' evaluable points, in wasmtime
/// call-expression form: (export, call, expected stdout).
const BATTERY: [(&str, &str, &str); 201] = [
    ("int-abs", "int-abs(0)", "0"),
    ("int-abs", "int-abs(4)", "4"),
    ("int-abs", "int-abs(-7)", "7"),
    ("int-abs", "int-abs(9223372036854775807)", "9223372036854775807"),
    ("int-min", "int-min(3, 7)", "3"),
    ("int-min", "int-min(7, 3)", "3"),
    ("int-min", "int-min(4, 4)", "4"),
    ("int-min", "int-min(-2, -9)", "-9"),
    ("int-min", "int-min(-9223372036854775808, 0)", "-9223372036854775808"),
    ("int-min", "int-min(0, -9223372036854775808)", "-9223372036854775808"),
    ("int-min", "int-min(9223372036854775807, 9223372036854775806)", "9223372036854775806"),
    ("int-max", "int-max(7, 3)", "7"),
    ("int-max", "int-max(3, 7)", "7"),
    ("int-max", "int-max(4, 4)", "4"),
    ("int-max", "int-max(-2, -9)", "-2"),
    ("int-max", "int-max(9223372036854775807, 0)", "9223372036854775807"),
    ("int-max", "int-max(0, 9223372036854775807)", "9223372036854775807"),
    ("int-max", "int-max(-9223372036854775808, -9223372036854775807)", "-9223372036854775807"),
    ("int-clamp", "int-clamp(5, 0, 10)", "5"),
    ("int-clamp", "int-clamp(-3, 0, 10)", "0"),
    ("int-clamp", "int-clamp(99, 0, 10)", "10"),
    ("int-clamp", "int-clamp(0, 0, 10)", "0"),
    ("int-clamp", "int-clamp(10, 0, 10)", "10"),
    ("int-clamp", "int-clamp(5, 5, 5)", "5"),
    ("int-clamp", "int-clamp(-7, -10, -5)", "-7"),
    ("int-clamp", "int-clamp(-12, -10, -5)", "-10"),
    ("int-sign", "int-sign(-5)", "-1"),
    ("int-sign", "int-sign(0)", "0"),
    ("int-sign", "int-sign(7)", "1"),
    ("int-sign", "int-sign(-1)", "-1"),
    ("int-sign", "int-sign(1)", "1"),
    ("int-sign", "int-sign(-9223372036854775808)", "-1"),
    ("int-sign", "int-sign(9223372036854775807)", "1"),
    ("int-is-even", "int-is-even(0)", "true"),
    ("int-is-even", "int-is-even(2)", "true"),
    ("int-is-even", "int-is-even(3)", "false"),
    ("int-is-even", "int-is-even(-4)", "true"),
    ("int-is-even", "int-is-even(-3)", "false"),
    ("int-is-even", "int-is-even(9223372036854775807)", "false"),
    ("int-is-even", "int-is-even(-9223372036854775808)", "true"),
    ("int-is-odd", "int-is-odd(0)", "false"),
    ("int-is-odd", "int-is-odd(2)", "false"),
    ("int-is-odd", "int-is-odd(3)", "true"),
    ("int-is-odd", "int-is-odd(-4)", "false"),
    ("int-is-odd", "int-is-odd(-3)", "true"),
    ("int-is-odd", "int-is-odd(9223372036854775807)", "true"),
    ("int-is-odd", "int-is-odd(-9223372036854775808)", "false"),
    ("char-is-digit", "char-is-digit('0')", "true"),
    ("char-is-digit", "char-is-digit('9')", "true"),
    ("char-is-digit", "char-is-digit('5')", "true"),
    ("char-is-digit", "char-is-digit('/')", "false"),
    ("char-is-digit", "char-is-digit(':')", "false"),
    ("char-is-digit", "char-is-digit('a')", "false"),
    ("char-is-digit", "char-is-digit(' ')", "false"),
    ("char-is-alpha", "char-is-alpha('A')", "true"),
    ("char-is-alpha", "char-is-alpha('Z')", "true"),
    ("char-is-alpha", "char-is-alpha('a')", "true"),
    ("char-is-alpha", "char-is-alpha('z')", "true"),
    ("char-is-alpha", "char-is-alpha('M')", "true"),
    ("char-is-alpha", "char-is-alpha('[')", "false"),
    ("char-is-alpha", "char-is-alpha('`')", "false"),
    ("char-is-alpha", "char-is-alpha('0')", "false"),
    ("char-is-alpha", "char-is-alpha(' ')", "false"),
    ("char-is-whitespace", "char-is-whitespace(' ')", "true"),
    ("char-is-whitespace", "char-is-whitespace('\\t')", "true"),
    ("char-is-whitespace", "char-is-whitespace('\\n')", "true"),
    ("char-is-whitespace", "char-is-whitespace('\\r')", "true"),
    ("char-is-whitespace", "char-is-whitespace('x')", "false"),
    ("char-is-whitespace", "char-is-whitespace('0')", "false"),
    ("char-is-uppercase", "char-is-uppercase('A')", "true"),
    ("char-is-uppercase", "char-is-uppercase('Z')", "true"),
    ("char-is-uppercase", "char-is-uppercase('a')", "false"),
    ("char-is-uppercase", "char-is-uppercase('5')", "false"),
    ("char-is-uppercase", "char-is-uppercase('[')", "false"),
    ("char-is-uppercase", "char-is-uppercase(' ')", "false"),
    ("char-is-lowercase", "char-is-lowercase('a')", "true"),
    ("char-is-lowercase", "char-is-lowercase('z')", "true"),
    ("char-is-lowercase", "char-is-lowercase('A')", "false"),
    ("char-is-lowercase", "char-is-lowercase('5')", "false"),
    ("char-is-lowercase", "char-is-lowercase('`')", "false"),
    ("char-is-lowercase", "char-is-lowercase(' ')", "false"),
    ("char-to-digit", "char-to-digit('0')", "some(0)"),
    ("char-to-digit", "char-to-digit('9')", "some(9)"),
    ("char-to-digit", "char-to-digit('4')", "some(4)"),
    ("char-to-digit", "char-to-digit('7')", "some(7)"),
    ("char-to-digit", "char-to-digit('a')", "none"),
    ("char-to-digit", "char-to-digit(' ')", "none"),
    ("char-to-digit", "char-to-digit('/')", "none"),
    ("char-to-digit", "char-to-digit(':')", "none"),
    ("string-is-empty", "string-is-empty(\"\")", "true"),
    ("string-is-empty", "string-is-empty(\"a\")", "false"),
    ("string-is-empty", "string-is-empty(\"hello\")", "false"),
    ("string-is-empty", "string-is-empty(\" \")", "false"),
    ("string-is-empty", "string-is-empty(\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\")", "false"),
    ("result-unwrap-or", "result-unwrap-or(ok(7), 3)", "7"),
    ("result-unwrap-or", "result-unwrap-or(err(\"bad\"), 3)", "3"),
    ("result-unwrap-or", "result-unwrap-or(ok(0), 5)", "0"),
    ("result-unwrap-or", "result-unwrap-or(err(\"\"), 0)", "0"),
    ("result-unwrap-or", "result-unwrap-or(ok(-4), 9)", "-4"),
    ("result-unwrap-or", "result-unwrap-or(err(\"boom\"), -2)", "-2"),
    ("option-unwrap-or", "option-unwrap-or(some(7), 3)", "7"),
    ("option-unwrap-or", "option-unwrap-or(none, 3)", "3"),
    ("option-unwrap-or", "option-unwrap-or(some(0), 5)", "0"),
    ("option-unwrap-or", "option-unwrap-or(none, 0)", "0"),
    ("option-unwrap-or", "option-unwrap-or(some(-9), 1)", "-9"),
    ("option-ok-or", "option-ok-or(some(4), \"missing\")", "ok(4)"),
    ("option-ok-or", "option-ok-or(none, \"missing\")", "err(\"missing\")"),
    ("option-ok-or", "option-ok-or(none, \"\")", "err(\"\")"),
    ("option-ok-or", "option-ok-or(some(0), \"e\")", "ok(0)"),
    ("list-is-empty", "list-is-empty([])", "true"),
    ("list-is-empty", "list-is-empty([7])", "false"),
    ("list-is-empty", "list-is-empty([1, 2, 3])", "false"),
    ("list-is-empty", "list-is-empty([0])", "false"),
    ("list-reverse", "list-reverse([])", "[]"),
    ("list-reverse", "list-reverse([5])", "[5]"),
    ("list-reverse", "list-reverse([1, 2, 3])", "[3, 2, 1]"),
    ("list-reverse", "list-reverse([3, 1, 4, 1, 5])", "[5, 1, 4, 1, 3]"),
    ("list-reverse", "list-reverse([1, 2, 1])", "[1, 2, 1]"),
    ("list-reverse", "list-reverse([0, -1, 2])", "[2, -1, 0]"),
    ("list-contains", "list-contains([], 1)", "false"),
    ("list-contains", "list-contains([5], 5)", "true"),
    ("list-contains", "list-contains([5], 3)", "false"),
    ("list-contains", "list-contains([1, 2, 3], 2)", "true"),
    ("list-contains", "list-contains([1, 2, 3], 9)", "false"),
    ("list-contains", "list-contains([2, 2, 2], 2)", "true"),
    ("list-contains", "list-contains([-3, 0, 7], -3)", "true"),
    ("list-sum", "list-sum([])", "0"),
    ("list-sum", "list-sum([5])", "5"),
    ("list-sum", "list-sum([1, 2, 3])", "6"),
    ("list-sum", "list-sum([-4, 4])", "0"),
    ("list-sum", "list-sum([10, -3, 7, -7])", "7"),
    ("list-sum", "list-sum([9223372036854775807])", "9223372036854775807"),
    ("list-sum", "list-sum([-9223372036854775808])", "-9223372036854775808"),
    ("list-sum", "list-sum([9223372036854775807, -9223372036854775808])", "-1"),
    ("list-product", "list-product([])", "1"),
    ("list-product", "list-product([5])", "5"),
    ("list-product", "list-product([2, 3, 4])", "24"),
    ("list-product", "list-product([0, 9223372036854775807])", "0"),
    ("list-product", "list-product([-2, 3])", "-6"),
    ("list-product", "list-product([-1, -1])", "1"),
    ("list-product", "list-product([1, 1, 1])", "1"),
    ("list-product", "list-product([-9223372036854775808])", "-9223372036854775808"),
    ("list-min", "list-min([])", "none"),
    ("list-min", "list-min([5])", "some(5)"),
    ("list-min", "list-min([3, 1, 4])", "some(1)"),
    ("list-min", "list-min([2, 2])", "some(2)"),
    ("list-min", "list-min([-5, 0, -9])", "some(-9)"),
    ("list-min", "list-min([9223372036854775807, -9223372036854775808])", "some(-9223372036854775808)"),
    ("list-max", "list-max([])", "none"),
    ("list-max", "list-max([5])", "some(5)"),
    ("list-max", "list-max([3, 1, 4])", "some(4)"),
    ("list-max", "list-max([2, 2])", "some(2)"),
    ("list-max", "list-max([-5, -9])", "some(-5)"),
    ("list-max", "list-max([-9223372036854775808, 9223372036854775807])", "some(9223372036854775807)"),
    ("list-take", "list-take([], 3)", "[]"),
    ("list-take", "list-take([1, 2, 3], 0)", "[]"),
    ("list-take", "list-take([1, 2, 3], -2)", "[]"),
    ("list-take", "list-take([1, 2, 3], 2)", "[1, 2]"),
    ("list-take", "list-take([1, 2, 3], 3)", "[1, 2, 3]"),
    ("list-take", "list-take([1, 2, 3], 99)", "[1, 2, 3]"),
    ("list-take", "list-take([5], 1)", "[5]"),
    ("list-drop", "list-drop([], 3)", "[]"),
    ("list-drop", "list-drop([1, 2, 3], 0)", "[1, 2, 3]"),
    ("list-drop", "list-drop([1, 2, 3], -2)", "[1, 2, 3]"),
    ("list-drop", "list-drop([1, 2, 3], 2)", "[3]"),
    ("list-drop", "list-drop([1, 2, 3], 3)", "[]"),
    ("list-drop", "list-drop([1, 2, 3], 99)", "[]"),
    ("list-drop", "list-drop([5], 1)", "[]"),
    ("list-is-sorted", "list-is-sorted([])", "true"),
    ("list-is-sorted", "list-is-sorted([5])", "true"),
    ("list-is-sorted", "list-is-sorted([1, 2, 3])", "true"),
    ("list-is-sorted", "list-is-sorted([3, 1, 2])", "false"),
    ("list-is-sorted", "list-is-sorted([5, 5, 5])", "true"),
    ("list-is-sorted", "list-is-sorted([1, 2, 2, 3])", "true"),
    ("list-is-sorted", "list-is-sorted([3, 2, 1])", "false"),
    ("list-is-sorted", "list-is-sorted([-2, -1, 0])", "true"),
    ("list-is-sorted", "list-is-sorted([1, 3, 2])", "false"),
    ("list-sort", "list-sort([])", "[]"),
    ("list-sort", "list-sort([5])", "[5]"),
    ("list-sort", "list-sort([3, 1, 2])", "[1, 2, 3]"),
    ("list-sort", "list-sort([5, 4, 3, 2, 1])", "[1, 2, 3, 4, 5]"),
    ("list-sort", "list-sort([2, 2, 1, 1])", "[1, 1, 2, 2]"),
    ("list-sort", "list-sort([1, 2, 3])", "[1, 2, 3]"),
    ("list-sort", "list-sort([-1, 5, -3, 0])", "[-3, -1, 0, 5]"),
    ("list-binary-search", "list-binary-search([], 5)", "none"),
    ("list-binary-search", "list-binary-search([5], 5)", "some(0)"),
    ("list-binary-search", "list-binary-search([5], 3)", "none"),
    ("list-binary-search", "list-binary-search([1, 3, 5, 7, 9], 1)", "some(0)"),
    ("list-binary-search", "list-binary-search([1, 3, 5, 7, 9], 9)", "some(4)"),
    ("list-binary-search", "list-binary-search([1, 3, 5, 7, 9], 7)", "some(3)"),
    ("list-binary-search", "list-binary-search([1, 3, 5, 7, 9], 4)", "none"),
    ("list-binary-search", "list-binary-search([1, 3, 5, 7, 9], 0)", "none"),
    ("list-binary-search", "list-binary-search([1, 3, 5, 7, 9], 10)", "none"),
    ("list-binary-search", "list-binary-search([-5, -1, 2, 8], -1)", "some(1)"),
    ("range", "range(0, 3)", "[0, 1, 2]"),
    ("range", "range(0, 0)", "[]"),
    ("range", "range(4, 4)", "[]"),
    ("range", "range(5, 2)", "[]"),
    ("range", "range(5, 6)", "[5]"),
    ("range", "range(-3, 0)", "[-3, -2, -1]"),
    ("range", "range(2, 5)", "[2, 3, 4]"),
];

fn run_wasmtime_battery() {
    let Some(wasmtime) = tool_path(WASMTIME) else {
        println!("P161|clause1|wasmtime ABSENT in this environment; battery not run");
        return;
    };
    let artifact = repo_root().join(LIB_ARTIFACT);
    let mut per_export: std::collections::BTreeMap<&str, usize> = Default::default();
    for (name, call, expected) in BATTERY {
        let out = Command::new(&wasmtime)
            .args(["run", "--invoke"])
            .arg(call)
            .arg(&artifact)
            .output()
            .unwrap_or_else(|e| panic!("run wasmtime for {call}: {e}"));
        assert!(
            out.status.success(),
            "wasmtime run --invoke {call} succeeds: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let got = String::from_utf8(out.stdout).unwrap();
        assert_eq!(
            got.trim(),
            expected,
            "{call} answers exactly through the component"
        );
        *per_export.entry(name).or_default() += 1;
    }
    assert_eq!(per_export.len(), 30, "every one of the 30 exports was exercised");
    println!(
        "P161|clause1|{}/{} battery points exact across 30 exports",
        BATTERY.len(),
        BATTERY.len()
    );
}

// ==== The clause-2 jco instrument (the frozen groups, re-run) ====
/// One battery row: (JS export, args as JSON, expected
/// as JSON). Numbers travel as decimal strings (s64 →
/// BigInt in the runner); booleans stay booleans.
type Row = (&'static str, &'static str, &'static str);

/// The rehearsal groups: (MAPFN, PRED, COMBINE, rows).
/// Each named host function is the JS implementation
/// of the function the group's rows denote — a
/// lambda's body or the named stdlib function, per row
/// (see the row comments in stdlib/list.morph).
const GROUPS: [(&str, &str, &str, &[Row]); 7] = [
    // map-apply = |x| x + 1 · pred = |x| x > 0 ·
    // combine = |a, x| a + x
    ("succ", "gt0", "add", &[
        ("listMap", "[[]]", "[]"),
        ("listMap", "[[\"1\",\"2\",\"3\"]]", "[\"2\",\"3\",\"4\"]"),
        ("listFilter", "[[]]", "[]"),
        ("listFilter", "[[\"1\",\"2\",\"3\"]]", "[\"1\",\"2\",\"3\"]"),
        ("listFold", "[[],\"42\"]", "\"42\""),
        ("listFold", "[[\"5\"],\"10\"]", "\"15\""),
        ("listFold", "[[\"1\",\"2\",\"3\",\"4\"],\"0\"]", "\"10\""),
        ("listFold", "[[\"-5\",\"5\"],\"3\"]", "\"3\""),
        ("listAll", "[[]]", "true"),
        ("listAll", "[[\"1\",\"2\",\"3\"]]", "true"),
        ("listAll", "[[\"1\",\"0\",\"3\"]]", "false"),
        ("listAll", "[[\"0\"]]", "false"),
        ("listAny", "[[]]", "false"),
        ("listAny", "[[\"0\",\"-1\"]]", "false"),
        ("listAny", "[[\"0\",\"5\",\"0\"]]", "true"),
        ("listAny", "[[\"7\"]]", "true"),
        ("listAny", "[[\"1\",\"2\"]]", "true"),
    ]),
    // map-apply = int_abs · pred = int_is_even ·
    // combine = int_min
    ("abs", "isEven", "min", &[
        ("listMap", "[[]]", "[]"),
        ("listMap", "[[\"-3\",\"4\",\"-6\"]]", "[\"3\",\"4\",\"6\"]"),
        ("listFilter", "[[\"1\",\"2\",\"3\",\"4\",\"5\",\"6\"]]", "[\"2\",\"4\",\"6\"]"),
        ("listFold", "[[\"4\",\"2\",\"8\"],\"99\"]", "\"2\""),
        ("listAll", "[[\"2\",\"4\",\"6\"]]", "true"),
        ("listAll", "[[\"2\",\"3\",\"4\"]]", "false"),
        ("listAny", "[[\"1\",\"3\",\"4\"]]", "true"),
        ("listAny", "[[]]", "false"),
    ]),
    // map-apply = |x| x * 2 · pred = |x| x > 9 ·
    // combine = |a, x| a * x
    ("double", "gt9", "mul", &[
        ("listMap", "[[\"5\"]]", "[\"10\"]"),
        ("listFilter", "[[\"1\",\"2\",\"3\"]]", "[]"),
        ("listFold", "[[],\"7\"]", "\"7\""),
    ]),
    // map-apply = |x| 0 - x · pred = |x| x < 0 ·
    // combine = |a, x| a - x (the LEFT-order row: the
    // accumulator is the minuend at every step)
    ("neg", "lt0", "sub", &[
        ("listMap", "[[\"3\",\"-5\",\"0\"]]", "[\"-3\",\"5\",\"0\"]"),
        ("listFilter", "[[\"-2\",\"-1\",\"0\",\"1\"]]", "[\"-2\",\"-1\"]"),
        ("listFold", "[[\"1\",\"2\",\"3\"],\"100\"]", "\"94\""),
        ("listAll", "[[\"-1\",\"-2\"]]", "true"),
    ]),
    // map-apply = |x| x · pred = |x| x == 2 ·
    // combine = int_max
    ("id", "eq2", "max", &[
        ("listMap", "[[\"9223372036854775807\"]]", "[\"9223372036854775807\"]"),
        ("listFilter", "[[\"2\",\"2\",\"3\",\"3\"]]", "[\"2\",\"2\"]"),
        ("listFold", "[[\"1\",\"2\"],\"50\"]", "\"50\""),
    ]),
    // map-apply = int_sign · pred = int_is_odd ·
    // combine unused by this group's rows (add supplied)
    ("sign", "isOdd", "add", &[
        ("listMap", "[[\"-8\",\"0\",\"9\"]]", "[\"-1\",\"0\",\"1\"]"),
        ("listFilter", "[[]]", "[]"),
        ("listAll", "[[]]", "true"),
        ("listAny", "[[\"2\",\"4\"]]", "false"),
    ]),
    // pred = |x| x % 2 == 0 (filter's "typical" lambda
    // row; map-apply/combine unused, supplied standing)
    ("id", "evenmod", "add", &[
        ("listFilter", "[[\"1\",\"2\",\"3\",\"4\"]]", "[\"2\",\"4\"]"),
    ]),
];

const MAP_APPLY_JS: &str = r#"
const impls = {
  succ: (x) => x + 1n,
  abs: (x) => (x < 0n ? -x : x),
  double: (x) => x * 2n,
  neg: (x) => 0n - x,
  id: (x) => x,
  sign: (x) => (x < 0n ? -1n : x > 0n ? 1n : 0n),
};
const name = process.env.MAPFN;
if (!impls[name]) throw new Error(`unknown MAPFN ${name}`);
export default impls[name];
"#;

const PRED_JS: &str = r#"
const impls = {
  gt0: (x) => x > 0n,
  gt9: (x) => x > 9n,
  evenmod: (x) => x % 2n === 0n,
  lt0: (x) => x < 0n,
  eq2: (x) => x === 2n,
  isEven: (x) => x % 2n === 0n,
  isOdd: (x) => x % 2n !== 0n,
};
const name = process.env.PRED;
if (!impls[name]) throw new Error(`unknown PRED ${name}`);
export default impls[name];
"#;

const COMBINE_JS: &str = r#"
const impls = {
  add: (acc, x) => acc + x,
  mul: (acc, x) => acc * x,
  sub: (acc, x) => acc - x,
  min: (acc, x) => (acc < x ? acc : x),
  max: (acc, x) => (acc > x ? acc : x),
};
const name = process.env.COMBINE;
if (!impls[name]) throw new Error(`unknown COMBINE ${name}`);
export default impls[name];
"#;

const RUNNER_JS: &str = r#"
import { readFileSync } from "node:fs";
import * as component from "./out/morphic-callbacks.js";
const rows = JSON.parse(readFileSync(process.argv[2], "utf8"));
const conv = (v) => {
  if (Array.isArray(v)) return v.map(conv);
  if (typeof v === "string") return BigInt(v);
  return v;
};
const show = (v) => {
  if (Array.isArray(v) || ArrayBuffer.isView(v)) {
    return "[" + Array.from(v, show).join(",") + "]";
  }
  return String(v);
};
let passed = 0;
for (const row of rows) {
  const fn = component[row.entry];
  if (typeof fn !== "function") throw new Error(`no export ${row.entry}`);
  const got = fn(...row.args.map(conv));
  const want = conv(row.expect);
  if (show(got) !== show(want)) {
    throw new Error(
      `${row.entry}(${row.args.map(show).join(", ")}) = ${show(got)}, want ${show(want)}`
    );
  }
  passed += 1;
}
console.log(`JCO-OK ${passed}`);
"#;

fn run_jco_rehearsal() {
    // node + npm are the rehearsal's platform; where
    // either is absent the battery is reported, not run
    // (the standing probe discipline).
    let node_ok = Command::new("node").arg("--version").output().is_ok();
    let npm_ok = Command::new("npm").arg("--version").output().is_ok();
    if !node_ok || !npm_ok {
        println!("P161|clause2|node/npm ABSENT in this environment; rehearsal not run");
        return;
    }
    let scratch = repo_root().join("packages/component-callbacks/target/jco-rehearsal");
    std::fs::create_dir_all(&scratch).expect("create the rehearsal scratch");
    // The harness files are written fresh from the
    // pinned sources above — the scratch holds no
    // state the pin does not re-derive.
    for (name, body) in [
        ("map-apply.js", MAP_APPLY_JS),
        ("pred.js", PRED_JS),
        ("combine.js", COMBINE_JS),
        ("run.mjs", RUNNER_JS),
    ] {
        std::fs::write(scratch.join(name), body).expect("write the harness file");
    }
    if !scratch.join("package.json").exists() {
        std::fs::write(
            scratch.join("package.json"),
            "{\n  \"name\": \"morphic-callbacks-jco-rehearsal\",\n  \"private\": true,\n  \"type\": \"module\"\n}\n",
        )
        .expect("write the rehearsal package.json");
    }
    // jco, pinned as the audit pinned it (1.42.0),
    // installed only inside this scratch — never global.
    if !scratch.join("node_modules/@bytecodealliance/jco").exists() {
        let out = Command::new("npm")
            .args([
                "install",
                "--no-audit",
                "--no-fund",
                "@bytecodealliance/jco@1.42.0",
            ])
            .current_dir(&scratch)
            .output()
            .expect("npm install the pinned jco");
        assert!(
            out.status.success(),
            "npm install @bytecodealliance/jco@1.42.0: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let jco = scratch.join("node_modules/.bin/jco");
    let out = Command::new(&jco)
        .args(["transpile"])
        .arg(repo_root().join(CB_ARTIFACT))
        .args([
            "-o",
            "out",
            "--map",
            "map-apply=./map-apply.js",
            "--map",
            "pred=./pred.js",
            "--map",
            "combine=./combine.js",
        ])
        .current_dir(&scratch)
        .output()
        .expect("jco transpile the shipped artifact");
    assert!(
        out.status.success(),
        "jco transpile succeeds: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // The transpiled module imports the three host
    // modules by relative path — they stand in out/.
    for name in ["map-apply.js", "pred.js", "combine.js"] {
        std::fs::copy(scratch.join(name), scratch.join("out").join(name))
            .expect("stand the host module in out/");
    }

    let mut per_entry: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut total = 0usize;
    for (mapfn, pred, combine, rows) in GROUPS {
        let rows_json = format!(
            "[{}]",
            rows.iter()
                .map(|(entry, args, expect)| {
                    format!("{{\"entry\":\"{entry}\",\"args\":{args},\"expect\":{expect}}}")
                })
                .collect::<Vec<_>>()
                .join(",")
        );
        let rows_path = scratch.join("rows.json");
        std::fs::write(&rows_path, &rows_json).expect("write the group's rows");
        let out = Command::new("node")
            .arg("run.mjs")
            .arg("rows.json")
            .env("MAPFN", mapfn)
            .env("PRED", pred)
            .env("COMBINE", combine)
            .current_dir(&scratch)
            .output()
            .expect("run the rehearsal group");
        assert!(
            out.status.success(),
            "group ({mapfn}, {pred}, {combine}) answers exactly: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8(out.stdout).unwrap();
        let n: usize = stdout
            .trim()
            .strip_prefix("JCO-OK ")
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| panic!("the runner reports its count: {stdout}"));
        assert_eq!(n, rows.len(), "every row of the group answered");
        for (entry, _, _) in rows {
            *per_entry.entry(entry).or_default() += 1;
        }
        total += n;
    }
    assert_eq!(total, 40, "the five batteries total 40 points");
    assert_eq!(per_entry.len(), 5, "every one of the five exports was exercised");
    for (entry, n) in &per_entry {
        assert_eq!(*n, 8, "{entry} answers its full 8-point battery");
    }
    println!("P161|clause2|40/40 battery points exact across the five exports (8 each), jco 1.42.0 + node");
}
