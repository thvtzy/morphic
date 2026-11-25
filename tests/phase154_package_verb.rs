//! Phase 154 — the user-package verb (DISEBAR_15_SCOPE
//! §4, D2a): `morphic build --package-from <file.morph>`.
//!
//! The verb is the one place user packages are allowed
//! to come from: it feeds the binding generator ONLY
//! rows its own seeded run measured (the PCC-1 fact is
//! the contract's core — the generator otherwise prints
//! caller-stated claims verbatim). These pins hold the
//! verb to that contract:
//!
//! * a small user module with admissible specs (two
//!   crowning Verified — one of them higher-order — and
//!   one executed-exact) assembles; the manifest rows
//!   match the run's measurements exactly; a clean host
//!   project imports and calls the exports battery-exact;
//! * in a build without the verifier the SAME module
//!   measures executed-exact rows instead — the rows say
//!   which evidence they are;
//! * a module containing one spec whose battery is
//!   partial (a spec-silent point on an Inconclusive
//!   crown) is refused BY NAME with nothing written —
//!   the stricter reading of the freeze's refusal rule:
//!   one refused spec refuses the whole assembly;
//! * there is no CLI surface by which a tier, a verdict
//!   basis, or a battery figure can be supplied (the
//!   flag surface is asserted by name);
//! * the verb-built package serves through the Phase 153
//!   template's no-globals sandbox smoke (the gate's
//!   clause-1 rehearsal);
//! * two runs over the same module produce byte-
//!   identical packages.

use std::process::Command;
use std::sync::Mutex;

use morphic::codegen::package::{
    assemble_user_package, unpacked_mass, NPM_UNPACKED_BUDGET,
};

/// Crown-bearing tests serialize on this lock (the
/// recorded load class of Phases 142–144: heavy crowns
/// flip under parallel load — the verifier carries a
/// 5 s wall-clock per-constraint timeout — each passing
/// solo). The guard is recovered from poisoning, the
/// Phase 144/150 pattern. No assertion is weakened.
static LOCK: Mutex<()> = Mutex::new(());

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn manifest_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase154-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The admissible exhibit module: a range predicate and
/// a higher-order spec that crown Verified at seed 42
/// (5/5/5 and 2/2/2), and a sortedness spec whose crown
/// stands Inconclusive but executed-exact (5/5/5) — the
/// two admission branches of freeze §4.2, measured at
/// this phase before pinning.
const MODULE_A: &str = r#"WidgetMath {
spec is_widget_code {
    doc: "A widget code is known when it names one of the six widget kinds."
    input: code: Int
    output: Bool
    constraint: output == (code>=1&&code<=6)
    test "first" { input: 1 expect: true }
    test "last" { input: 6 expect: true }
    test "zero" { input: 0 expect: false }
    test "above" { input: 7 expect: false }
    test "far" { input: 99 expect: false }
}
spec weights_sorted {
    doc: "weights_sorted(xs) = true when the weights are in non-decreasing order."
    input: xs: List<Int>
    output: Bool
    constraint: len(xs) <= 1 implies output == true
    constraint: forall i: Int in range(0, len(xs) - 1):
        output == true implies xs[i] <= xs[i + 1]
    constraint: forall i: Int in range(0, len(xs) - 1):
        xs[i] > xs[i + 1] implies output == false
    test "empty" { input: []; expect: true }
    test "single" { input: [5]; expect: true }
    test "sorted" { input: [1, 2, 3]; expect: true }
    test "unsorted" { input: [3, 1, 2]; expect: false }
    test "equal" { input: [5, 5, 5]; expect: true }
}
spec apply_succ_twice {
    doc: "apply_succ_twice(f, x) = f applied twice to x."
    input: f: fn(Int) -> Int, x: Int
    output: Int
    constraint: output == f(f(x))
    test { input: (|n| n + 1, 40); expect: 42 }
    test { input: (|n| n * 2, 3); expect: 12 }
}
}"#;

/// The refusal exhibit: one admissible spec beside one
/// whose battery is partial BY CONSTRUCTION — the
/// "over the cap" point violates the spec's own
/// `require:`, so it is spec-silent for every body
/// (evaluable < total forever), and its fold-shaped
/// crown stands Inconclusive through the pipeline
/// route. Neither admission branch can take it.
const MODULE_B: &str = r#"BatchTotals {
spec is_batch_code {
    doc: "A batch code is known when it names one of the four batch kinds."
    input: code: Int
    output: Bool
    constraint: output == (code>=1&&code<=4)
    test "first" { input: 1 expect: true }
    test "last" { input: 4 expect: true }
    test "zero" { input: 0 expect: false }
}
spec batch_total {
    doc: "batch_total(xs) = the sum of a batch of at most three weights."
    input: xs: List<Int>
    output: Int
    require: len(xs) <= 3
    constraint: output == fold(xs, 0, |acc, v| acc + v)
    test "within" { input: [1, 2] expect: 3 }
    test "over the cap" { input: [1, 2, 3, 4] expect: 10 }
}
}"#;

fn assemble_a() -> morphic::codegen::package::UserPackageAssembly {
    assemble_user_package(MODULE_A, "widget-math", "0.2.0", "widget_math.morph", 42)
        .expect("module A assembles")
}

fn file<'a>(
    assembly: &'a morphic::codegen::package::UserPackageAssembly,
    path: &str,
) -> &'a str {
    assembly
        .files
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("the package carries {path}"))
        .contents
        .as_str()
}

// ── Admission: the measured rows ────────────────────────

/// Z3 builds: the two Verified crowns enter as tier-1 /
/// z3-verified, the executed-exact crown as tier-2, and
/// every manifest row equals the run's own measurements.
#[test]
#[cfg(all(feature = "z3-support", not(feature = "wasm-playground")))]
fn p154_module_a_assembles_with_measured_rows() {
    let _guard = crown_serial();
    let assembly = assemble_a();
    let rows: Vec<(&str, &str, &str, &str, usize, usize, usize, usize)> = assembly
        .standings
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.tier.as_str(),
                s.verdict_basis.as_str(),
                s.status.as_str(),
                s.passed,
                s.evaluable,
                s.total,
                s.spec_silent,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("is_widget_code", "tier-1", "z3-verified", "Verified", 5, 5, 5, 0),
            ("weights_sorted", "tier-2", "executed-exact", "Inconclusive", 5, 5, 5, 0),
            ("apply_succ_twice", "tier-1", "z3-verified", "Verified", 2, 2, 2, 0),
        ],
        "the standings are the run's own measurements"
    );

    // The manifest rows match the standings exactly —
    // the package claims nothing the run did not measure.
    let manifest: serde_json::Value =
        serde_json::from_str(file(&assembly, "morphic.manifest.json"))
            .expect("the manifest parses");
    assert_eq!(manifest["package"], "widget-math");
    assert_eq!(manifest["packageVersion"], "0.2.0");
    assert_eq!(manifest["morphicVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest["scope"], "widget_math.morph");
    let exports = manifest["exports"].as_array().expect("exports list");
    assert_eq!(exports.len(), assembly.standings.len());
    for (row, standing) in exports.iter().zip(&assembly.standings) {
        assert_eq!(row["name"], standing.name);
        assert_eq!(row["tier"], standing.tier);
        assert_eq!(row["verdictBasis"], standing.verdict_basis);
        assert_eq!(row["battery"]["pinned"], standing.passed);
        assert_eq!(row["battery"]["specSilent"], standing.spec_silent);
    }

    // The file set is the standing layout, and
    // package.json carries the author's facts only —
    // no license and no project links are asserted on
    // the author's behalf.
    let mut paths: Vec<&str> = assembly.files.iter().map(|f| f.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        [
            "README.md",
            "apply_succ_twice.d.ts",
            "apply_succ_twice.js",
            "index.d.ts",
            "index.js",
            "is_widget_code.d.ts",
            "is_widget_code.js",
            "morphic.manifest.json",
            "package.json",
            "weights_sorted.d.ts",
            "weights_sorted.js",
        ]
    );
    let package_json: serde_json::Value =
        serde_json::from_str(file(&assembly, "package.json"))
            .expect("package.json is valid JSON");
    assert_eq!(package_json["name"], "widget-math");
    assert_eq!(package_json["version"], "0.2.0");
    assert!(package_json.get("license").is_none(), "no license is asserted");
    assert!(
        package_json.get("repository").is_none(),
        "no project links are asserted"
    );

    // The higher-order export's declaration carries
    // EMBED_SCOPE §3's standing stance: the function
    // parameter is `never`, the true signature rides
    // the manifest.
    let dts = file(&assembly, "index.d.ts");
    assert!(
        dts.contains("export declare function apply_succ_twice(f: never, x: bigint): bigint;"),
        "the function parameter is declared never:\n{dts}"
    );

    // The README the verb writes: the provenance
    // language (measured rows only, the seed named, the
    // byte-for-byte re-run as the re-judgment route)
    // and the `never` stance stated.
    let readme = file(&assembly, "README.md");
    for named in [
        "morphic build --package-from widget_math.morph",
        "seed 42",
        "byte-for-byte",
        "declared `never`",
        "no argument, flag, file or caller statement",
    ] {
        assert!(readme.contains(named), "the README states: {named}");
    }

    // Freeze §7: user packages are measured against the
    // standing npm budget.
    let mass = unpacked_mass(&assembly.files);
    assert!(
        mass <= NPM_UNPACKED_BUDGET,
        "the user package is {mass} B, inside the {NPM_UNPACKED_BUDGET} B budget"
    );
    println!(
        "P154|verb|admit is_widget_code=tier-1/Verified/5-5-5 \
         weights_sorted=tier-2/executed-exact/5-5-5 \
         apply_succ_twice=tier-1/Verified/2-2-2 mass={mass}B"
    );
}

/// Without the verifier the same module still assembles
/// — every crown measures Inconclusive, so every row is
/// the executed-exact row its evidence supports. The
/// package never dresses executed evidence as a verdict.
#[test]
#[cfg(all(not(feature = "z3-support"), not(feature = "wasm-playground")))]
fn p154_module_a_measures_executed_rows_without_a_verifier() {
    let _guard = crown_serial();
    let assembly = assemble_a();
    let rows: Vec<(&str, &str, &str, &str)> = assembly
        .standings
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.tier.as_str(),
                s.verdict_basis.as_str(),
                s.status.as_str(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("is_widget_code", "tier-2", "executed-exact", "Inconclusive"),
            ("weights_sorted", "tier-2", "executed-exact", "Inconclusive"),
            ("apply_succ_twice", "tier-2", "executed-exact", "Inconclusive"),
        ]
    );
    let manifest: serde_json::Value =
        serde_json::from_str(file(&assembly, "morphic.manifest.json"))
            .expect("the manifest parses");
    for row in manifest["exports"].as_array().expect("exports list") {
        assert_eq!(row["tier"], "tier-2");
        assert_eq!(row["verdictBasis"], "executed-exact");
    }
    println!(
        "P154|verb|no-verifier-rows all=tier-2/executed-exact \
         (the same module, the evidence this build has)"
    );
}

// ── Refusal: by name, nothing written ───────────────────

/// One refused spec refuses the whole assembly (the
/// stricter reading, recorded in package.rs): the
/// in-process verb names `batch_total` with its measured
/// standing, and the CLI — the same discipline at the
/// process boundary — exits non-zero and the output
/// directory is never created.
#[test]
#[cfg(not(feature = "wasm-playground"))]
fn p154_refusal_names_the_spec_and_writes_nothing() {
    let _guard = crown_serial();
    let err = assemble_user_package(
        MODULE_B,
        "batch-totals",
        "0.1.0",
        "batch_totals.morph",
        42,
    )
    .expect_err("module B is refused");
    assert!(err.contains("batch_total"), "the refusal names the spec: {err}");
    assert!(
        err.contains("nothing is written"),
        "the refusal states the discipline: {err}"
    );
    assert!(
        err.contains("spec-silent"),
        "the refusal states the measured standing: {err}"
    );

    let dir = scratch_dir("refusal");
    let module_path = dir.join("batch_totals.morph");
    std::fs::write(&module_path, MODULE_B).unwrap();
    let out_dir = dir.join("batch-pkg");
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("build")
        .arg("--package-from")
        .arg(&module_path)
        .arg("--out")
        .arg(&out_dir)
        .output()
        .expect("the binary runs");
    assert!(!out.status.success(), "the CLI refuses: {}", out.status);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        stderr.contains("batch_total"),
        "the CLI refusal names the spec:\n{stderr}"
    );
    assert!(
        !out_dir.exists(),
        "nothing is written: the output directory is never created"
    );
    let _ = std::fs::remove_dir_all(&dir);
    println!("P154|verb|refuse batch_total named, nothing written (in-process + CLI)");
}

/// The CLI assembles module A end-to-end, and the
/// naming defaults are the recorded rule: package name
/// = the module file's stem, version = the Morphic
/// version, output directory = `<stem>-package`.
#[test]
#[cfg(not(feature = "wasm-playground"))]
fn p154_cli_assembles_module_a_with_the_naming_rule() {
    let _guard = crown_serial();
    let dir = scratch_dir("cli");
    let module_path = dir.join("widget_math.morph");
    std::fs::write(&module_path, MODULE_A).unwrap();
    let out_dir = dir.join("chosen-out");
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("build")
        .arg("--package-from")
        .arg(&module_path)
        .arg("--package-name")
        .arg("widget-math")
        .arg("--package-version")
        .arg("0.2.0")
        .arg("--out")
        .arg(&out_dir)
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "the CLI assembles:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("Assembled widget-math"), "{stdout}");
    for name in ["is_widget_code", "weights_sorted", "apply_succ_twice"] {
        assert!(stdout.contains(name), "the CLI reports the standing: {name}");
    }
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("morphic.manifest.json"))
            .expect("the manifest is written"),
    )
    .expect("the written manifest parses");
    assert_eq!(manifest["package"], "widget-math");
    assert_eq!(manifest["packageVersion"], "0.2.0");
    assert_eq!(manifest["exports"].as_array().map(|a| a.len()), Some(3));

    // Defaults: no name/version/out flags at all.
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("build")
        .arg("--package-from")
        .arg(&module_path)
        .current_dir(&dir)
        .output()
        .expect("the binary runs");
    assert!(
        out.status.success(),
        "the CLI assembles on defaults:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let default_dir = dir.join("widget_math-package");
    let package_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(default_dir.join("package.json"))
            .expect("the default output directory is <stem>-package"),
    )
    .expect("package.json parses");
    assert_eq!(package_json["name"], "widget_math");
    assert_eq!(package_json["version"], env!("CARGO_PKG_VERSION"));
    let _ = std::fs::remove_dir_all(&dir);
    println!("P154|verb|cli assembled; defaults name=<stem> version=morphic out=<stem>-package");
}

/// In the browser-feature build the CLI refuses the
/// verb by name (assembly verbs are native-build acts
/// in this tree — the --package precedent). Only
/// compiled under that feature unification.
#[test]
#[cfg(feature = "wasm-playground")]
fn p154_cli_refuses_in_the_browser_feature_build() {
    let dir = scratch_dir("browser-refusal");
    let module_path = dir.join("widget_math.morph");
    std::fs::write(&module_path, MODULE_A).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("build")
        .arg("--package-from")
        .arg(&module_path)
        .output()
        .expect("the binary runs");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        stderr.contains("native build"),
        "the refusal names the build class:\n{stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── The claim surface that does not exist ───────────────

/// The PCC-1 pin at the CLI surface: the verb's flags
/// are exactly the input, the naming, the seed and the
/// output — there is no flag by which a caller could
/// state a standing for the run to print.
#[test]
fn p154_flag_surface_carries_no_claim_flags() {
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args(["build", "--help"])
        .output()
        .expect("the binary runs");
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout).to_string();
    for flag in [
        "--package-from",
        "--package-name",
        "--package-version",
        "--seed",
        "--out",
    ] {
        assert!(help.contains(flag), "the help carries {flag}:\n{help}");
    }
    for claim in ["--tier", "--verdict", "--basis", "--battery"] {
        assert!(
            !help.contains(claim),
            "no caller-stated-claims flag exists: {claim} must be absent:\n{help}"
        );
    }
    println!("P154|verb|flags package-from/name/version/seed/out; no claim flags exist");
}

// ── The package at work: host + template ────────────────

fn write_package(dir: &std::path::Path) {
    let assembly = assemble_a();
    for f in &assembly.files {
        std::fs::write(dir.join(&f.path), &f.contents).unwrap();
    }
}

/// A clean host project imports the verb-built package
/// and calls its exports battery-exact — including the
/// higher-order export with a host-supplied function.
#[test]
#[cfg(not(feature = "wasm-playground"))]
fn p154_clean_host_calls_the_package_battery_exact() {
    let _guard = crown_serial();
    let dir = scratch_dir("host");
    let pkg = dir.join("pkg");
    std::fs::create_dir_all(&pkg).unwrap();
    write_package(&pkg);
    std::fs::write(
        dir.join("driver.mjs"),
        r#"
const pkg = await import(process.argv[2]);
console.log(JSON.stringify({
  code_first: pkg.is_widget_code(1n),
  code_last: pkg.is_widget_code(6n),
  code_above: pkg.is_widget_code(7n),
  sorted: pkg.weights_sorted([1n, 2n, 3n]),
  unsorted: pkg.weights_sorted([3n, 1n, 2n]),
  equal: pkg.weights_sorted([5n, 5n, 5n]),
  twice_inc: String(pkg.apply_succ_twice((n) => n + 1n, 40n)),
  twice_dbl: String(pkg.apply_succ_twice((n) => n * 2n, 3n)),
}));
"#,
    )
    .unwrap();
    let index_url = format!("file://{}", pkg.join("index.js").display());
    let out = Command::new("node")
        .arg(dir.join("driver.mjs"))
        .arg(&index_url)
        .current_dir(&dir)
        .output()
        .expect("node runs");
    assert!(
        out.status.success(),
        "the host calls run:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_str(
        String::from_utf8_lossy(&out.stdout).lines().last().expect("one JSON line"),
    )
    .expect("the driver's JSON parses");
    assert_eq!(v["code_first"], true);
    assert_eq!(v["code_last"], true);
    assert_eq!(v["code_above"], false);
    assert_eq!(v["sorted"], true);
    assert_eq!(v["unsorted"], false);
    assert_eq!(v["equal"], true);
    assert_eq!(v["twice_inc"], "42");
    assert_eq!(v["twice_dbl"], "12");
    let _ = std::fs::remove_dir_all(&dir);
    println!("P154|verb|host clean-host calls battery-exact (incl. HO with host callback)");
}

/// The gate's clause-1 rehearsal: the Phase 153
/// template's worker, its `@thvtzy/morphic` import
/// swapped from the shipped library to the verb-built
/// package, serves in the no-globals sandbox — the
/// verb's exports answer battery-exact through the
/// fetch handler.
#[test]
#[cfg(not(feature = "wasm-playground"))]
fn p154_template_serves_the_verb_built_package() {
    let _guard = crown_serial();
    let dir = scratch_dir("template");
    let pkg = dir.join("pkg");
    std::fs::create_dir_all(&pkg).unwrap();
    write_package(&pkg);
    // The Phase 153 harness, unchanged but for the
    // calls: the worker links its one import to the
    // package this phase's verb built.
    let harness = r#"
import vm from "node:vm";
import { readFileSync } from "node:fs";
const [workerPath, libraryPath] = process.argv.slice(2);
const context = vm.createContext(Object.create(null));
const library = new vm.SourceTextModule(readFileSync(libraryPath, "utf8"), {
  context, identifier: "morphic-library",
});
await library.link(() => { throw new Error("the library imports nothing"); });
const worker = new vm.SourceTextModule(readFileSync(workerPath, "utf8"), {
  context, identifier: "morphic-worker",
});
await worker.link((specifier) => {
  if (specifier === "@thvtzy/morphic") return library;
  throw new Error(`unexpected import: ${specifier}`);
});
await worker.evaluate();
const handler = worker.namespace.default.fetch;
const post = (name, body) => ({
  method: "POST",
  url: `https://worker.test/fn/${name}`,
  json: async () => body,
});
const call = async (request) => {
  const res = await handler(request, {}, {});
  return { status: res.status, payload: JSON.parse(res.body) };
};
const sorted = await call(post("weights_sorted", { args: [[1, 2, 3]] }));
const unsorted = await call(post("weights_sorted", { args: [[3, 1, 2]] }));
const code = await call(post("is_widget_code", { args: [6] }));
console.log(JSON.stringify({
  process_in_ctx: vm.runInContext("typeof process", context),
  fetch_in_ctx: vm.runInContext("typeof fetch", context),
  sorted, unsorted, code,
}));
"#;
    std::fs::write(dir.join("smoke.mjs"), harness).unwrap();
    let root = manifest_dir();
    let out = Command::new("node")
        .arg("--experimental-vm-modules")
        .arg(dir.join("smoke.mjs"))
        .arg(root.join("templates/cloudflare-workers/src/index.js"))
        .arg(pkg.join("index.js"))
        .output()
        .expect("node runs");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "the template smoke runs:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_str(
        stdout.lines().last().expect("one JSON line"),
    )
    .expect("the harness JSON parses");
    assert_eq!(v["process_in_ctx"], "undefined");
    assert_eq!(v["fetch_in_ctx"], "undefined");
    assert_eq!(v["sorted"]["status"], 200);
    assert_eq!(v["sorted"]["payload"]["result"], true);
    assert_eq!(v["unsorted"]["status"], 200);
    assert_eq!(v["unsorted"]["payload"]["result"], false);
    assert_eq!(v["code"]["status"], 200);
    assert_eq!(v["code"]["payload"]["result"], true);
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "P154|verb|template gate-clause-1 rehearsal: verb-built package serves \
         battery-exact in the no-globals sandbox"
    );
}

// ── Determinism + the early refusals ────────────────────

/// The seeded discipline at the package level: two
/// runs over the same module produce byte-identical
/// packages — the property the README names as the
/// re-judgment route.
#[test]
#[cfg(not(feature = "wasm-playground"))]
fn p154_two_runs_are_byte_identical() {
    let _guard = crown_serial();
    let first = assemble_a();
    let second = assemble_a();
    assert_eq!(first.standings, second.standings);
    assert_eq!(
        first.files, second.files,
        "two verb runs over the same module are byte-identical"
    );
    println!("P154|verb|determinism two runs byte-identical");
}

/// A module that does not parse or does not typecheck
/// is refused with its stage named, before any run.
#[test]
fn p154_uncheckable_module_is_refused_by_stage() {
    let parse_err = assemble_user_package(
        "Broken { spec f { input: x: Int output: ",
        "x",
        "0.1.0",
        "broken.morph",
        42,
    )
    .expect_err("a parse failure refuses");
    assert!(parse_err.contains("does not parse"), "{parse_err}");

    let type_err = assemble_user_package(
        "Typed {\nspec f {\n    input: a: Int\n    output: Int\n    \
         constraint: output == a ++ 2\n}\n}",
        "x",
        "0.1.0",
        "typed.morph",
        42,
    )
    .expect_err("a typecheck failure refuses");
    assert!(type_err.contains("does not typecheck"), "{type_err}");

    let empty = assemble_user_package("Empty { }", "x", "0.1.0", "empty.morph", 42)
        .expect_err("a spec-less module refuses");
    assert!(empty.contains("no specs"), "{empty}");
    println!("P154|verb|early-refusals parse/typecheck/no-specs named by stage");
}
