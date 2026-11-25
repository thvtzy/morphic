//! Phase 160 — the callback component + both runner
//! rehearsals (DISEBAR_16_SCOPE §5, decision D2a).
//!
//! Pins, in order:
//! 1. the freeze's §5 shape + the home choice recorded
//!    (the crate at `packages/component-callbacks/`, a
//!    sibling of `packages/component/`) + the README's
//!    honesty surface (the import contract, the
//!    correspondence table, the separate-artifact
//!    reason, the tier note with the callback's
//!    correctness named as the host's);
//! 2. the artifact of record at
//!    `packages/component-callbacks/morphic-callbacks.wasm`
//!    — a true component header, inside its frozen
//!    budget, sha-pinned;
//! 3. the WIT surface — the in-tree `wit/world.wit` and,
//!    where the pinned wasm-tools is checkable, the
//!    artifact's own printed world carry exactly the
//!    three frozen imports and the five frozen exports;
//! 4. the neighbours unmoved — the main Cargo set is
//!    the standing 12, the root lockfile knows neither
//!    component crate, packages/npm and packages/rust
//!    stand at their Phase 157 masses, and the library
//!    component's artifact bytes are unmoved;
//! 5. the wasmtime-CLI negative pin — the library
//!    component's runner refuses this artifact at
//!    instantiation, naming the unsatisfied import:
//!    the two-component split is structural;
//! 6. the jco rehearsal — where node + npm are
//!    checkable, the shipped artifact is transpiled by
//!    the pinned jco and driven in Node with a host
//!    object supplying the three imports; all 40 points
//!    of the five frozen batteries answer exactly
//!    (the table below);
//! 7. build reproduction — where cargo + the wasip2
//!    target are checkable, a fresh plain-cargo build
//!    reproduces the committed artifact byte-for-byte.
//!
//! The battery table's provenance: the five entries'
//! frozen test rows (stdlib/list.morph, Amendment 2's
//! batteries), each row's `expect:` taken verbatim —
//! 8 rows per entry, 40 total, cross-checked against
//! packages/bundles' battery fields (8 total / 8
//! evaluable / 8 passed per entry). A row's function
//! argument cannot cross the boundary (WIT has no
//! function-typed values), so it is supplied as the
//! host callback it denotes: a lambda row maps to a
//! host implementation of the same lambda, a named row
//! to a host implementation of the named stdlib
//! function — each named per group in the table. The
//! three imports are Int-typed (s64) as frozen, so
//! every row exercises Int only; no element type is
//! widened. The empty-list edges stand in the table:
//! list-all [] → true, list-any [] → false,
//! list-fold [] → init, list-map/list-filter [] → [].

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

const WASMTIME: &str = "workspace/tools/wasmtime-51.0.0/wasmtime";
const WASM_TOOLS: &str = "workspace/tools/wasm-tools-1.263.0/wasm-tools";
const ARTIFACT: &str = "packages/component-callbacks/morphic-callbacks.wasm";
const LIBRARY_ARTIFACT: &str = "packages/component/morphic-library.wasm";
/// The frozen budget (DISEBAR_16_SCOPE §6): the Phase
/// 158 measurement (81,778 B) +10%, rounded up to a
/// whole KiB.
const CALLBACK_BUDGET: usize = 90_112;
/// The artifact's measured size + sha256 at this phase.
const CALLBACK_SIZE: usize = 81_840;
const CALLBACK_SHA256: &str =
    "9d8f7e972775cdb6e947da2681a0d4937316019121e313ba90481d49b64155a8";
const LIBRARY_SHA256: &str =
    "f08c214c1a56ad360b5a90a42e04371524258a6a713a44cedec480938738a212";

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

/// The frozen world (DISEBAR_16_SCOPE §5): three
/// imports, five exports, verbatim shapes.
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
fn p160_freeze_and_home() {
    let scope = flattened(&read_repo("DISEBAR_16_SCOPE.md"));
    for needle in [
        "import map-apply: func(x: s64) -> s64;",
        "import pred: func(x: s64) -> bool;",
        "import combine: func(acc: s64, x: s64) -> s64;",
        "export list-map: func(xs: list<s64>) -> list<s64>;",
        "export list-fold: func(xs: list<s64>, init: s64) -> s64;",
        "90,112",
        "imports satisfy at instantiation (D2a)",
    ] {
        assert!(scope.contains(needle), "the freeze's §5 carries {needle}");
    }
    // The home choice, recorded: a sibling crate beside
    // the library component, its own workspace root.
    assert!(
        repo_root().join("packages/component-callbacks/Cargo.toml").exists()
            && repo_root().join("packages/component-callbacks/wit/world.wit").exists(),
        "the callback crate stands at packages/component-callbacks/"
    );
    let cargo = read_repo("packages/component-callbacks/Cargo.toml");
    assert!(cargo.contains("[workspace]"), "its own workspace root");
    assert!(
        cargo.contains("wit-bindgen = \"=0.63.0\""),
        "wit-bindgen pinned =0.63.0"
    );
    // The README carries the phase's honesty surface.
    let readme = flattened(&read_repo("packages/component-callbacks/README.md"));
    for needle in [
        "imports satisfy at instantiation",
        "map-apply",
        "list-map",
        "combine",
        "accumulator first",
        "LEFT fold",
        "The callback's correctness is the host's",
        "Tier-1",
        "packages/component-callbacks/",
        "was not found in the linker",
    ] {
        assert!(readme.contains(needle), "the README carries {needle}");
    }
    println!("P160|freeze|§5 world + budget, home = packages/component-callbacks (recorded), README honesty surface pinned");
}

#[test]
fn p160_artifact_of_record() {
    let bytes = std::fs::read(repo_root().join(ARTIFACT))
        .unwrap_or_else(|e| panic!("read {ARTIFACT}: {e}"));
    assert_eq!(
        &bytes[..8],
        &[0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00],
        "a true component header (version 0x0d, layer 0x01), not a core module"
    );
    assert_eq!(bytes.len(), CALLBACK_SIZE, "the artifact's measured size");
    assert!(
        bytes.len() <= CALLBACK_BUDGET,
        "inside the frozen budget of {CALLBACK_BUDGET} B"
    );
    if let Some(sha) = sha256_of(&repo_root().join(ARTIFACT)) {
        assert_eq!(sha, CALLBACK_SHA256, "the artifact's sha256 pin");
    }
    println!(
        "P160|artifact|component header ok, {} B <= {CALLBACK_BUDGET} B budget, sha pinned",
        bytes.len()
    );
}

#[test]
fn p160_wit_surface() {
    let wit = read_repo("packages/component-callbacks/wit/world.wit");
    let rows: Vec<String> = wit
        .lines()
        .map(str::trim)
        .filter(|l| l.contains(": func("))
        .map(str::to_string)
        .collect();
    assert_eq!(rows.len(), 8, "the in-tree WIT carries 3 imports + 5 exports");
    for (name, ty) in FROZEN_IMPORTS {
        let want = format!("import {name}: {ty};");
        assert!(rows.contains(&want), "the in-tree WIT carries {want}");
    }
    for (name, ty) in FROZEN_EXPORTS {
        let want = format!("export {name}: {ty};");
        assert!(rows.contains(&want), "the in-tree WIT carries {want}");
    }

    let Some(wasm_tools) = tool_path(WASM_TOOLS) else {
        println!("P160|wit|wasm-tools ABSENT in this environment; in-tree WIT pinned only");
        return;
    };
    let out = Command::new(&wasm_tools)
        .args(["component", "wit"])
        .arg(repo_root().join(ARTIFACT))
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
    println!("P160|wit|in-tree WIT + artifact world agree: 3 imports, 5 exports, frozen types");
}

#[test]
fn p160_neighbours_unmoved() {
    let root = repo_root();
    let cargo_toml = read_repo("Cargo.toml");
    let mut in_deps = false;
    let mut deps: Vec<String> = Vec::new();
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]" {
            in_deps = true;
            continue;
        }
        if in_deps && trimmed.starts_with('[') {
            break;
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
            "serde", "serde_json", "thiserror", "tokio", "z3",
        ],
        "the Cargo dependency set is the standing set"
    );
    let lock = read_repo("Cargo.lock");
    for absent in ["morphic-component-callbacks", "morphic-component"] {
        assert!(
            !lock.contains(&format!("name = \"{absent}\"")),
            "the root lockfile knows no {absent} — the component crates are their own workspace roots"
        );
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
    let (npm_files, npm_bytes) = dir_mass(&root.join("packages/npm"));
    assert_eq!((npm_files, npm_bytes), (77, 71_873), "packages/npm unmoved");
    let (rust_files, rust_bytes) = dir_mass(&root.join("packages/rust"));
    assert_eq!((rust_files, rust_bytes), (7, 57_613), "packages/rust unmoved");
    // The library component's artifact bytes unmoved.
    let lib = std::fs::read(root.join(LIBRARY_ARTIFACT)).expect("read the library artifact");
    assert_eq!(lib.len(), 96_100, "the library artifact's size unmoved");
    if let Some(sha) = sha256_of(&root.join(LIBRARY_ARTIFACT)) {
        assert_eq!(sha, LIBRARY_SHA256, "the library artifact's bytes unmoved (sha)");
    }
    println!("P160|neighbours|cargo=standing-12, root lock knows neither component crate, npm 77/71873, rust 7/57613, library artifact bytes unmoved");
}

#[test]
fn p160_wasmtime_negative() {
    let Some(wasmtime) = tool_path(WASMTIME) else {
        println!("P160|negative|wasmtime ABSENT in this environment; refusal not run");
        return;
    };
    let out = Command::new(&wasmtime)
        .args(["run", "--invoke", "list-map([1, 2, 3])"])
        .arg(repo_root().join(ARTIFACT))
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
        "the refusal names the unsatisfied import (the linker-error class): {stderr}"
    );
    println!("P160|negative|wasmtime refuses at instantiation, naming `map-apply` — the split is structural");
}

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

#[test]
fn p160_jco_rehearsal() {
    // node + npm are the rehearsal's platform; where
    // either is absent the battery is reported, not run
    // (the standing probe discipline).
    let node_ok = Command::new("node").arg("--version").output().is_ok();
    let npm_ok = Command::new("npm").arg("--version").output().is_ok();
    if !node_ok || !npm_ok {
        println!("P160|jco|node/npm ABSENT in this environment; rehearsal not run");
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
        .arg(repo_root().join(ARTIFACT))
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
    println!("P160|jco|40/40 battery points exact across the five exports (8 each), jco 1.42.0 + node");
}

#[test]
fn p160_build_reproduces() {
    let home = std::env::var("HOME").expect("HOME");
    let cargo = PathBuf::from(&home).join(".cargo/bin/cargo");
    let rustup = PathBuf::from(&home).join(".cargo/bin/rustup");
    if !cargo.exists() || !rustup.exists() {
        println!("P160|build|cargo/rustup ABSENT in this environment; reproduction not run");
        return;
    }
    let targets = Command::new(&rustup)
        .args(["target", "list", "--installed"])
        .output()
        .expect("rustup target list");
    if !String::from_utf8(targets.stdout).unwrap().contains("wasm32-wasip2") {
        println!("P160|build|wasm32-wasip2 target ABSENT in this environment; reproduction not run");
        return;
    }
    let crate_dir = repo_root().join("packages/component-callbacks");
    let out = Command::new(&cargo)
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .current_dir(&crate_dir)
        .output()
        .expect("cargo build the callback crate");
    assert!(
        out.status.success(),
        "plain cargo builds the callback component: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let fresh = std::fs::read(
        crate_dir.join("target/wasm32-wasip2/release/morphic_component_callbacks.wasm"),
    )
    .expect("read the fresh artifact");
    let committed = std::fs::read(repo_root().join(ARTIFACT)).expect("read the committed artifact");
    assert_eq!(
        fresh, committed,
        "a fresh build reproduces the artifact of record byte-for-byte"
    );
    println!("P160|build|plain cargo reproduces the committed artifact byte-for-byte");
}
