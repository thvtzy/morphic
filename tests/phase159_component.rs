//! Phase 159 — the 1.6 freeze + the library component
//! (DISEBAR_16_SCOPE §§1–4, decisions D1a–D7a).
//!
//! Pins, in order:
//! 1. the freeze's shape (ratification under the owner's
//!    delegation, lockstep, budgets, closing line verbatim);
//! 2. the artifact of record at
//!    `packages/component/morphic-library.wasm` — a true
//!    component header, inside its frozen budget;
//! 3. the WIT surface — the in-tree `wit/world.wit` and,
//!    where the pinned wasm-tools is checkable, the
//!    artifact's own printed world carry exactly the 30
//!    frozen exports with the frozen types;
//! 4. the neighbours unmoved — the main Cargo dependency
//!    set is the standing 12 and the root lockfile knows
//!    no wit-bindgen; packages/npm and packages/rust stand
//!    at their Phase 157 masses;
//! 5. the wasmtime rehearsal — where the pinned wasmtime
//!    is checkable, every evaluable point of the 30
//!    exports' frozen batteries answers exactly through
//!    the shipped artifact (the table below);
//! 6. build reproduction — where cargo + the wasip2
//!    target are checkable, a fresh plain-cargo build
//!    reproduces the committed artifact byte-for-byte.
//!
//! The battery table's provenance: derived at Phase 159
//! from the stdlib specs' own test rows
//! (stdlib/*.morph), classified by the interpreter's
//! `test_outcomes` over the shipped bodies — Passed rows
//! only. The 30 batteries total 203 rows: 201 evaluable,
//! all passed; the 2 spec-silent rows (int_abs at the
//! Int minimum, int_clamp on an inverted range — both
//! `require:` exclusions) carry no component obligation
//! and are not in the table. Counts cross-checked
//! against packages/bundles' battery fields (203 total /
//! 201 evaluable / 201 passed). Sandbox-local standings
//! (pinned tools, the rustup target) are asserted where
//! they are checkable and reported where they are not,
//! in the standing probe discipline (Phase 158).

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
const ARTIFACT: &str = "packages/component/morphic-library.wasm";
/// The frozen budget (DISEBAR_16_SCOPE §6): the Phase 158
/// measurement (96,096 B) +10%, rounded up to a whole KiB.
const LIBRARY_BUDGET: usize = 106_496;
/// The artifact's measured size at this phase.
const LIBRARY_SIZE: usize = 96_100;

/// The 30 frozen exports with their frozen WIT types
/// (EMBED_SCOPE §3's column as re-measured at Phase 158).
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

#[test]
fn p159_freeze_shape() {
    let scope = flattened(&read_repo("DISEBAR_16_SCOPE.md"));
    for needle in [
        "D1a", "D2a", "D3a", "D4a", "D5a", "D6a", "D7a",
        "ratified by Muse under the owner's delegation",
        "lockstep 1.6.0",
        "106,496",
        "90,112",
        "wasm-tools 1.263.0",
        "wasmtime 51.0.0",
        "wit-bindgen",
        "=0.63.0",
        "21c036c1",
        "a1802067",
        "packages/component/",
    ] {
        assert!(scope.contains(needle), "the freeze carries {needle}");
    }
    assert!(
        scope.contains(
            "\"1.6 closes when the same proven functions answer from a boundary \
             no single language owns — every export the library ships, the \
             higher-order five carried honestly as the imports they are, and \
             no type crossing that boundary with a loss nobody named.\""
        ),
        "the closing line stands frozen verbatim"
    );
    println!("P159|freeze|D1a-D7a under delegation, lockstep 1.6.0, budgets + closing line verbatim");
}

#[test]
fn p159_artifact_of_record() {
    let bytes = std::fs::read(repo_root().join(ARTIFACT))
        .unwrap_or_else(|e| panic!("read {ARTIFACT}: {e}"));
    assert_eq!(
        &bytes[..8],
        &[0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00],
        "a true component header (version 0x0d, layer 0x01), not a core module"
    );
    assert_eq!(bytes.len(), LIBRARY_SIZE, "the artifact's measured size");
    assert!(
        bytes.len() <= LIBRARY_BUDGET,
        "inside the frozen budget of {LIBRARY_BUDGET} B"
    );
    println!(
        "P159|artifact|component header ok, {} B <= {LIBRARY_BUDGET} B budget",
        bytes.len()
    );
}

#[test]
fn p159_wit_surface() {
    // The in-tree WIT carries exactly the frozen surface.
    let wit = read_repo("packages/component/wit/world.wit");
    let rows: Vec<String> = wit
        .lines()
        .map(str::trim)
        .filter(|l| l.contains(": func("))
        .map(str::to_string)
        .collect();
    assert_eq!(rows.len(), 30, "the in-tree WIT exports exactly 30 functions");
    for (name, ty) in FROZEN_SURFACE {
        let want = format!("{name}: {ty};");
        assert!(rows.contains(&want), "the in-tree WIT carries {want}");
    }

    // Where the pinned wasm-tools is checkable, the
    // artifact's own printed world agrees.
    let Some(wasm_tools) = tool_path(WASM_TOOLS) else {
        println!("P159|wit|wasm-tools ABSENT in this environment; in-tree WIT pinned only");
        return;
    };
    let out = Command::new(&wasm_tools)
        .args(["component", "wit"])
        .arg(repo_root().join(ARTIFACT))
        .output()
        .expect("run wasm-tools component wit");
    assert!(out.status.success(), "wasm-tools component wit succeeds");
    let printed = flattened(&String::from_utf8(out.stdout).unwrap());
    let version = env!("CARGO_PKG_VERSION");
    assert!(
        printed.contains(&format!(
            "export morphic:library/functions@{version};"
        )),
        "the world exports morphic:library/functions at the standing lockstep version"
    );
    let section = printed
        .split("package morphic:library@")
        .nth(1)
        .expect("the printed world carries the morphic:library package");
    assert_eq!(
        section.matches(": func(").count(),
        30,
        "the artifact's morphic:library interface carries exactly 30 functions"
    );
    for (name, ty) in FROZEN_SURFACE {
        let want = format!("{name}: {ty};");
        assert!(section.contains(&want), "the artifact's world carries {want}");
    }
    println!("P159|wit|in-tree WIT + artifact world agree: 30 exports, frozen types");
}

#[test]
fn p159_neighbours_unmoved() {
    let root = repo_root();

    // The main Cargo dependency set is the standing 12
    // (the Phase 157 pin, carried).
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
    assert!(
        !lock.contains("morphic-component"),
        "the component crate never enters the root lockfile — it is its own workspace root"
    );
    let lock_versions: Vec<&str> = lock
        .split("name = \"wit-bindgen\"")
        .skip(1)
        .filter_map(|block| {
            block
                .lines()
                .find(|l| l.starts_with("version = "))
                .map(|l| l.trim_start_matches("version = \"").trim_end_matches('"'))
        })
        .collect();
    assert_eq!(
        lock_versions,
        ["0.51.0", "0.57.1"],
        "the root lockfile's wit-bindgen entries are the pre-existing playground-graph \
         versions only — the component crate's =0.63.0 pin lives in its own lockfile"
    );

    // Sibling packages stand at their Phase 157 masses.
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
    println!("P159|neighbours|cargo=standing-12, component absent from root lock (its wit-bindgen set pre-existing only), npm 77/71873, rust 7/57613");
}

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

#[test]
fn p159_wasmtime_rehearsal() {
    let Some(wasmtime) = tool_path(WASMTIME) else {
        println!("P159|rehearsal|wasmtime ABSENT in this environment; battery not run");
        return;
    };
    let artifact = repo_root().join(ARTIFACT);
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
        "P159|rehearsal|{}/{} battery points exact across 30 exports",
        BATTERY.len(),
        BATTERY.len()
    );
}

#[test]
fn p159_build_reproduces() {
    let home = std::env::var("HOME").expect("HOME");
    let cargo = PathBuf::from(&home).join(".cargo/bin/cargo");
    let rustup = PathBuf::from(&home).join(".cargo/bin/rustup");
    if !cargo.exists() || !rustup.exists() {
        println!("P159|build|cargo/rustup ABSENT in this environment; reproduction not run");
        return;
    }
    let targets = Command::new(&rustup)
        .args(["target", "list", "--installed"])
        .output()
        .expect("rustup target list");
    if !String::from_utf8(targets.stdout).unwrap().contains("wasm32-wasip2") {
        println!("P159|build|wasm32-wasip2 target ABSENT in this environment; reproduction not run");
        return;
    }
    let crate_dir = repo_root().join("packages/component");
    let out = Command::new(&cargo)
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .current_dir(&crate_dir)
        .output()
        .expect("cargo build the component crate");
    assert!(
        out.status.success(),
        "plain cargo builds the component: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let fresh = std::fs::read(
        crate_dir.join("target/wasm32-wasip2/release/morphic_component.wasm"),
    )
    .expect("read the fresh artifact");
    let committed = std::fs::read(repo_root().join(ARTIFACT)).expect("read the committed artifact");
    assert_eq!(
        fresh, committed,
        "a fresh build reproduces the artifact of record byte-for-byte"
    );
    println!("P159|build|plain cargo reproduces the committed artifact byte-for-byte");
}
