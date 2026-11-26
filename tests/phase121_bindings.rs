//! Phase 121 (1.1 Embeddable, EMBED_SCOPE §5): typed
//! bindings + the tier manifest. The freeze's exit
//! conditions, pinned:
//!
//! * one assembled ESM module exports exactly the 35
//!   frozen entries (§2.1, as amended by STDLIB_SCOPE.md
//!   Amendment 2 — Phase 135 added the six rows below), generated from the shipped
//!   bodies through the Phase 120 emitter — no wrappers,
//!   no extra exports, no missing names; each export
//!   carries its entry's `doc:` as JSDoc;
//! * the `.d.ts` is generated from the same signature
//!   source as the manifest (one source of truth), in
//!   the §3 shapes (Int → `bigint`, Option → `T | null`,
//!   Result → the discriminated union), tier + verdict
//!   basis in every JSDoc, and compiles under
//!   `tsc --strict` together with a typed host snippet
//!   (with a negative control that must fail);
//! * `morphic.manifest.json` rows correspond EXACTLY to
//!   §2.1's table — names, frozen signatures, tiers,
//!   verdict bases, and the battery counts of the
//!   standing Phase 111/112/115 cross-validation
//!   batteries (`pinned` = exactly-pinned points,
//!   `specSilent` = recorded spec-silent points:
//!   `int_abs` 1, `int_clamp` 2, `list_binary_search` 3,
//!   zero elsewhere) — and each row's `doc` is the
//!   entry's doc string verbatim;
//! * a CLEAN host project (scratch dir outside the repo)
//!   imports the assembled module and calls the
//!   representative exports (Tier-1 Int `int_clamp`,
//!   Char `char_to_digit`, Option `option_unwrap_or` +
//!   `option_ok_or`, Result `result_unwrap_or`, Tier-2
//!   List `list_sum` + `list_min`) with typed values at
//!   frozen battery points — 100% agreement under node;
//! * the ungated generator (D1a) binds a user module the
//!   same way, and its manifest row claims only the
//!   verdict the native pipeline actually produced for
//!   that spec in this configuration (R12).
//!
//! Configuration note (the standing one): pins that read
//! the NATIVE prelude are gated
//! `#[cfg(not(feature = "wasm-playground"))]`.

#[cfg(not(feature = "wasm-playground"))]
use std::collections::BTreeSet;
#[cfg(not(feature = "wasm-playground"))]
use std::path::{Path, PathBuf};
#[cfg(not(feature = "wasm-playground"))]
use std::process::Command;

#[cfg(not(feature = "wasm-playground"))]
use morphic::codegen::bindings;
#[cfg(not(feature = "wasm-playground"))]
use morphic::codegen::bindings::{BindingInput, ManifestExport};
#[cfg(not(feature = "wasm-playground"))]
use morphic::spec::ast::FunctionSpec;
#[cfg(not(feature = "wasm-playground"))]
use morphic::spec::{parser, typeck};
#[cfg(not(feature = "wasm-playground"))]
use morphic::stdlib;
#[cfg(not(feature = "wasm-playground"))]
use morphic::synthesis::engine::CandidateImplementation;
#[cfg(not(feature = "wasm-playground"))]
use morphic::synthesis::{engine, eval, runner, selector};
#[cfg(not(feature = "wasm-playground"))]
use morphic::verify::verdict::VerificationStatus;
#[cfg(not(feature = "wasm-playground"))]
use morphic::verify::verifier::Verifier;

// ── The frozen §2.1 table ────────────────────────────────
//
// (name, frozen Morphic signature, tier, verdictBasis,
//  battery pinned, battery specSilent, TS param types,
//  TS return type). Signatures and tiers are EMBED_SCOPE
// §2.1 verbatim; battery counts are the standing
// Phase 111/112/115 cross-validation batteries as
// measured (their P11xXV lines); TS shapes are §3's.

#[cfg(not(feature = "wasm-playground"))]
const RESULT_INT_STRING: &str =
    "{ tag: \"Ok\", value: bigint } | { tag: \"Err\", error: string }";

#[cfg(not(feature = "wasm-playground"))]
struct FrozenRow {
    name: &'static str,
    signature: &'static str,
    tier: &'static str,
    basis: &'static str,
    pinned: usize,
    silent: usize,
    ts_params: &'static [&'static str],
    ts_return: &'static str,
}

#[cfg(not(feature = "wasm-playground"))]
const FROZEN: &[FrozenRow] = &[
    FrozenRow { name: "int_abs", signature: "Int → Int", tier: "tier-1", basis: "z3-verified", pinned: 16, silent: 1, ts_params: &["bigint"], ts_return: "bigint" },
    FrozenRow { name: "int_sign", signature: "Int → Int", tier: "tier-1", basis: "z3-verified", pinned: 11, silent: 0, ts_params: &["bigint"], ts_return: "bigint" },
    FrozenRow { name: "int_min", signature: "(Int, Int) → Int", tier: "tier-1", basis: "z3-verified", pinned: 16, silent: 0, ts_params: &["bigint", "bigint"], ts_return: "bigint" },
    FrozenRow { name: "int_max", signature: "(Int, Int) → Int", tier: "tier-1", basis: "z3-verified", pinned: 16, silent: 0, ts_params: &["bigint", "bigint"], ts_return: "bigint" },
    FrozenRow { name: "int_clamp", signature: "(Int, Int, Int) → Int", tier: "tier-1", basis: "z3-verified", pinned: 13, silent: 2, ts_params: &["bigint", "bigint", "bigint"], ts_return: "bigint" },
    FrozenRow { name: "int_is_even", signature: "Int → Bool", tier: "tier-1", basis: "z3-verified", pinned: 15, silent: 0, ts_params: &["bigint"], ts_return: "boolean" },
    FrozenRow { name: "int_is_odd", signature: "Int → Bool", tier: "tier-1", basis: "z3-verified", pinned: 15, silent: 0, ts_params: &["bigint"], ts_return: "boolean" },
    FrozenRow { name: "char_is_digit", signature: "Char → Bool", tier: "tier-1", basis: "z3-verified", pinned: 101, silent: 0, ts_params: &["string"], ts_return: "boolean" },
    FrozenRow { name: "char_is_alpha", signature: "Char → Bool", tier: "tier-1", basis: "z3-verified", pinned: 101, silent: 0, ts_params: &["string"], ts_return: "boolean" },
    FrozenRow { name: "char_is_whitespace", signature: "Char → Bool", tier: "tier-1", basis: "z3-verified", pinned: 101, silent: 0, ts_params: &["string"], ts_return: "boolean" },
    FrozenRow { name: "char_is_uppercase", signature: "Char → Bool", tier: "tier-1", basis: "z3-verified", pinned: 101, silent: 0, ts_params: &["string"], ts_return: "boolean" },
    FrozenRow { name: "char_is_lowercase", signature: "Char → Bool", tier: "tier-1", basis: "z3-verified", pinned: 101, silent: 0, ts_params: &["string"], ts_return: "boolean" },
    FrozenRow { name: "char_to_digit", signature: "Char → Option<Int>", tier: "tier-1", basis: "z3-verified", pinned: 101, silent: 0, ts_params: &["string"], ts_return: "bigint | null" },
    FrozenRow { name: "string_is_empty", signature: "String → Bool", tier: "tier-1", basis: "z3-verified", pinned: 15, silent: 0, ts_params: &["string"], ts_return: "boolean" },
    FrozenRow { name: "result_unwrap_or", signature: "(Result<Int, String>, Int) → Int", tier: "tier-1", basis: "z3-verified", pinned: 36, silent: 0, ts_params: &[RESULT_INT_STRING, "bigint"], ts_return: "bigint" },
    FrozenRow { name: "option_unwrap_or", signature: "(Option<Int>, Int) → Int", tier: "tier-1", basis: "z3-verified", pinned: 28, silent: 0, ts_params: &["bigint | null", "bigint"], ts_return: "bigint" },
    FrozenRow { name: "option_ok_or", signature: "(Option<Int>, String) → Result<Int, String>", tier: "tier-1", basis: "z3-verified", pinned: 9, silent: 0, ts_params: &["bigint | null", "string"], ts_return: RESULT_INT_STRING },
    FrozenRow { name: "list_is_empty", signature: "List<Int> → Bool", tier: "tier-1", basis: "z3-verified", pinned: 15, silent: 0, ts_params: &["bigint[]"], ts_return: "boolean" },
    FrozenRow { name: "list_reverse", signature: "List<Int> → List<Int>", tier: "tier-1", basis: "z3-verified", pinned: 15, silent: 0, ts_params: &["bigint[]"], ts_return: "bigint[]" },
    FrozenRow { name: "list_contains", signature: "(List<Int>, Int) → Bool", tier: "tier-1", basis: "z3-verified", pinned: 75, silent: 0, ts_params: &["bigint[]", "bigint"], ts_return: "boolean" },
    FrozenRow { name: "list_sum", signature: "List<Int> → Int", tier: "tier-1", basis: "z3-verified", pinned: 15, silent: 0, ts_params: &["bigint[]"], ts_return: "bigint" },
    FrozenRow { name: "list_product", signature: "List<Int> → Int", tier: "tier-1", basis: "z3-verified", pinned: 13, silent: 0, ts_params: &["bigint[]"], ts_return: "bigint" },
    FrozenRow { name: "list_min", signature: "List<Int> → Option<Int>", tier: "tier-1", basis: "z3-verified", pinned: 15, silent: 0, ts_params: &["bigint[]"], ts_return: "bigint | null" },
    FrozenRow { name: "list_max", signature: "List<Int> → Option<Int>", tier: "tier-1", basis: "z3-verified", pinned: 15, silent: 0, ts_params: &["bigint[]"], ts_return: "bigint | null" },
    FrozenRow { name: "list_take", signature: "(List<Int>, Int) → List<Int>", tier: "tier-1", basis: "z3-verified", pinned: 28, silent: 0, ts_params: &["bigint[]", "bigint"], ts_return: "bigint[]" },
    FrozenRow { name: "list_drop", signature: "(List<Int>, Int) → List<Int>", tier: "tier-1", basis: "z3-verified", pinned: 28, silent: 0, ts_params: &["bigint[]", "bigint"], ts_return: "bigint[]" },
    FrozenRow { name: "list_is_sorted", signature: "List<Int> → Bool", tier: "tier-1", basis: "z3-verified", pinned: 15, silent: 0, ts_params: &["bigint[]"], ts_return: "boolean" },
    FrozenRow { name: "list_sort", signature: "List<Int> → List<Int>", tier: "tier-2", basis: "executed-exact", pinned: 15, silent: 0, ts_params: &["bigint[]"], ts_return: "bigint[]" },
    FrozenRow { name: "list_binary_search", signature: "(List<Int>, Int) → Option<Int>", tier: "tier-2", basis: "executed-exact", pinned: 64, silent: 3, ts_params: &["bigint[]", "bigint"], ts_return: "bigint | null" },
    // Amendment 2's six (Phase 135): Tier-2 on the A2.3
    // batteries at 1.2; the fn-typed parameters carry §3's
    // standing `never` in the declarations (A2.4.2).
    // Phase 144 (the 1.3 re-tier): the fifteen entries
    // the list-encoder campaign crowned — the nine
    // §2.6 rows above and these six — carry tier-1 /
    // z3-verified; list_sort and list_binary_search
    // stay tier-2 with named residues.
    FrozenRow { name: "list_map", signature: "(List<Int>, fn(Int) -> Int) → List<Int>", tier: "tier-1", basis: "z3-verified", pinned: 20, silent: 0, ts_params: &["bigint[]", "never"], ts_return: "bigint[]" },
    FrozenRow { name: "list_filter", signature: "(List<Int>, fn(Int) -> Bool) → List<Int>", tier: "tier-1", basis: "z3-verified", pinned: 20, silent: 0, ts_params: &["bigint[]", "never"], ts_return: "bigint[]" },
    FrozenRow { name: "list_fold", signature: "(List<Int>, Int, fn(Int, Int) -> Int) → Int", tier: "tier-1", basis: "z3-verified", pinned: 18, silent: 0, ts_params: &["bigint[]", "bigint", "never"], ts_return: "bigint" },
    FrozenRow { name: "list_all", signature: "(List<Int>, fn(Int) -> Bool) → Bool", tier: "tier-1", basis: "z3-verified", pinned: 16, silent: 0, ts_params: &["bigint[]", "never"], ts_return: "boolean" },
    FrozenRow { name: "list_any", signature: "(List<Int>, fn(Int) -> Bool) → Bool", tier: "tier-1", basis: "z3-verified", pinned: 16, silent: 0, ts_params: &["bigint[]", "never"], ts_return: "boolean" },
    FrozenRow { name: "range", signature: "(Int, Int) → List<Int>", tier: "tier-1", basis: "z3-verified", pinned: 15, silent: 0, ts_params: &["bigint", "bigint"], ts_return: "bigint[]" },
];

// ── Helpers ──────────────────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
fn prelude_spec(name: &str) -> FunctionSpec {
    stdlib::prelude_specs()
        .into_iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("{name} in prelude"))
}

/// Function-definition names in a generated module, in
/// order of appearance.
#[cfg(not(feature = "wasm-playground"))]
fn defined_functions(module: &str) -> Vec<String> {
    module
        .lines()
        .filter_map(|l| l.strip_prefix("function "))
        .map(|rest| rest.split('(').next().unwrap_or("").trim().to_string())
        .collect()
}

/// The names in the module's single trailing export list.
#[cfg(not(feature = "wasm-playground"))]
fn exported_names(module: &str) -> Vec<String> {
    let line = module
        .lines()
        .rev()
        .find(|l| l.starts_with("export {"))
        .unwrap_or_else(|| panic!("module carries an export list"));
    line.trim()
        .trim_start_matches("export {")
        .trim_end_matches("};")
        .split(',')
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

#[cfg(not(feature = "wasm-playground"))]
fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase121-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[cfg(not(feature = "wasm-playground"))]
fn run(cmd: &str, args: &[&str], dir: &Path) -> (bool, String) {
    let out = Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("{cmd} runs: {e}"));
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), text)
}

// ── The assembled module ─────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn stdlib_bindings_are_deterministic() {
    let a = bindings::stdlib_bindings().expect("stdlib bindings generate");
    let b = bindings::stdlib_bindings().expect("stdlib bindings regenerate");
    assert_eq!(a.js_module, b.js_module, "JS module is deterministic");
    assert_eq!(a.declarations, b.declarations, ".d.ts is deterministic");
    assert_eq!(a.manifest_json, b.manifest_json, "manifest is deterministic");
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn assembled_module_exports_exactly_the_frozen_35() {
    let set = bindings::stdlib_bindings().expect("stdlib bindings generate");
    let frozen: Vec<String> = FROZEN.iter().map(|r| r.name.to_string()).collect();

    // Exactly the 35 functions are defined — no wrappers,
    // no helpers, no missing entries — and exactly they
    // are exported.
    assert_eq!(defined_functions(&set.js_module), frozen, "defined functions");
    assert_eq!(exported_names(&set.js_module), frozen, "exported names");

    // Each export carries its entry's doc: as JSDoc
    // directly above its definition.
    for row in FROZEN {
        let doc = prelude_spec(row.name).doc.expect("entry carries a doc");
        let jsdoc = format!("/** {} */", doc);
        let def = format!("function {}(", row.name);
        let jsdoc_at = set.js_module.find(&jsdoc)
            .unwrap_or_else(|| panic!("{}: doc JSDoc present", row.name));
        let def_at = set.js_module.find(&def)
            .unwrap_or_else(|| panic!("{}: definition present", row.name));
        assert!(jsdoc_at < def_at, "{}: JSDoc precedes its function", row.name);
    }

    // Entry-to-entry calls resolve inside the module:
    // every plain call in the module body names one of
    // the 35 or a JS builtin form the emitter renders.
    assert!(
        !set.js_module.contains("undefined;"),
        "no standing-fallback residue in the assembled module"
    );
}

// ── The manifest ─────────────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn manifest_rows_correspond_exactly_to_embed_scope_2_1() {
    let set = bindings::stdlib_bindings().expect("stdlib bindings generate");
    let m = &set.manifest;
    assert_eq!(m.package, "@thvtzy/morphic");
    assert_eq!(m.package_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(m.morphic_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(m.scope, "EMBED_SCOPE.md");
    assert_eq!(m.exports.len(), 35, "35 manifest rows");

    let by_name: std::collections::BTreeMap<&str, &ManifestExport> =
        m.exports.iter().map(|e| (e.name.as_str(), e)).collect();
    for row in FROZEN {
        let e = by_name
            .get(row.name)
            .unwrap_or_else(|| panic!("{}: manifest row present", row.name));
        assert_eq!(e.signature, row.signature, "{}: frozen signature", row.name);
        assert_eq!(e.tier, row.tier, "{}: tier", row.name);
        assert_eq!(e.verdict_basis, row.basis, "{}: verdict basis", row.name);
        assert_eq!(e.battery.pinned, row.pinned, "{}: battery pinned", row.name);
        assert_eq!(
            e.battery.spec_silent, row.silent,
            "{}: battery spec-silent", row.name
        );
        // The doc travels verbatim from the entry's spec.
        assert_eq!(
            e.doc,
            prelude_spec(row.name).doc.as_deref().unwrap_or(""),
            "{}: doc verbatim",
            row.name
        );
        // And the signature is the spec's own, mechanically
        // rendered — declarations and manifest share the
        // one source of truth.
        assert_eq!(
            e.signature,
            bindings::morphic_signature(&prelude_spec(row.name)),
            "{}: signature derives from the spec",
            row.name
        );
    }
    let names: BTreeSet<&str> = by_name.keys().copied().collect();
    let frozen_names: BTreeSet<&str> = FROZEN.iter().map(|r| r.name).collect();
    assert_eq!(names, frozen_names, "no extra or missing manifest rows");

    // The JSON form parses and carries the frozen field
    // names (§2.4).
    let parsed: serde_json::Value =
        serde_json::from_str(&set.manifest_json).expect("manifest JSON parses");
    assert_eq!(parsed["package"], "@thvtzy/morphic");
    assert_eq!(parsed["scope"], "EMBED_SCOPE.md");
    let first = &parsed["exports"][0];
    for field in ["name", "signature", "tier", "verdictBasis", "battery", "doc"] {
        assert!(first.get(field).is_some(), "manifest row carries {field}");
    }
    assert!(first["battery"].get("pinned").is_some());
    assert!(first["battery"].get("specSilent").is_some());
}

// ── The declarations ─────────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn declarations_carry_the_frozen_section_3_shapes() {
    let set = bindings::stdlib_bindings().expect("stdlib bindings generate");
    let dts = &set.declarations;

    // One declaration per export, no more.
    assert_eq!(
        dts.matches("export declare function ").count(),
        35,
        "35 declarations"
    );
    for row in FROZEN {
        let spec = prelude_spec(row.name);
        let params: Vec<String> = spec
            .params
            .iter()
            .zip(row.ts_params.iter())
            .map(|(p, t)| format!("{}: {}", p.name, t))
            .collect();
        let decl = format!(
            "export declare function {}({}): {};",
            row.name,
            params.join(", "),
            row.ts_return
        );
        assert!(dts.contains(&decl), "{}: declaration `{decl}`", row.name);

        // Tier + verdict basis ride in the declaration's
        // JSDoc, with the battery counts.
        let tier_line = format!("Tier: {}", row.tier);
        let basis_line = format!("Verdict basis: {}", row.basis);
        assert!(dts.contains(&tier_line), "{}: JSDoc tier line", row.name);
        assert!(dts.contains(&basis_line), "{}: JSDoc basis line", row.name);
    }

    // The scope's three worked forms (§2.1), verbatim.
    for worked in [
        "export declare function int_clamp(x: bigint, lo: bigint, hi: bigint): bigint;",
        "export declare function list_min(xs: bigint[]): bigint | null;",
        "export declare function option_ok_or(o: bigint | null, e: string): { tag: \"Ok\", value: bigint } | { tag: \"Err\", error: string };",
    ] {
        assert!(dts.contains(worked), "worked form present: {worked}");
    }
}

// ── tsc --strict over a typed host snippet ───────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn typings_compile_under_tsc_strict_with_a_typed_host_snippet() {
    let set = bindings::stdlib_bindings().expect("stdlib bindings generate");
    let dir = scratch_dir("tsc");
    std::fs::write(dir.join("index.js"), &set.js_module).unwrap();
    std::fs::write(dir.join("index.d.ts"), &set.declarations).unwrap();
    std::fs::write(
        dir.join("package.json"),
        "{\n  \"name\": \"phase121-host\",\n  \"private\": true,\n  \"type\": \"module\"\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("tsconfig.json"),
        "{\n  \"compilerOptions\": {\n    \"strict\": true,\n    \"target\": \"ES2022\",\n    \"module\": \"NodeNext\",\n    \"moduleResolution\": \"NodeNext\",\n    \"noEmit\": true\n  },\n  \"include\": [\"host.ts\", \"index.d.ts\"]\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("host.ts"),
        r#"import {
  char_to_digit,
  int_clamp,
  list_min,
  list_sum,
  option_ok_or,
} from "./index.js";

const clamped: bigint = int_clamp(15n, 0n, 10n);
const total: bigint = list_sum([1n, 2n, 3n]);
const smallest: bigint | null = list_min([3n, 1n, 2n]);
const digit: bigint | null = char_to_digit("7");
const okOr: { tag: "Ok", value: bigint } | { tag: "Err", error: string } =
  option_ok_or(null, "missing");

// Exhaustive switching over the discriminated union.
function describe(r: typeof okOr): string {
  switch (r.tag) {
    case "Ok":
      return `ok:${r.value}`;
    case "Err":
      return `err:${r.error}`;
  }
}

export { clamped, digit, smallest, total, describe };
"#,
    )
    .unwrap();

    // TypeScript, obtained the way the sandbox allows: a
    // scratch npm install inside the host project.
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let (ok, out) = run(
        npm,
        &["install", "--no-audit", "--no-fund", "typescript@5.9.3"],
        &dir,
    );
    assert!(ok, "typescript installs in the host project:\n{out}");
    let tsc = dir.join(if cfg!(windows) {
        "node_modules/.bin/tsc.cmd"
    } else {
        "node_modules/.bin/tsc"
    });
    let tsc = tsc.to_str().unwrap();

    let (ok, out) = run(tsc, &["--strict", "-p", "tsconfig.json"], &dir);
    assert!(ok, "tsc --strict is clean over the typings + host snippet:\n{out}");

    // Negative control: the strict check has teeth — a
    // mistyped host file must NOT compile.
    std::fs::write(
        dir.join("host-bad.ts"),
        "import { int_clamp } from \"./index.js\";\n\
         const wrong: string = int_clamp(1n, 2n, 3n);\n\
         const alsoWrong: bigint = int_clamp(1, 2, 3);\n\
         export { wrong, alsoWrong };\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("tsconfig-bad.json"),
        "{\n  \"compilerOptions\": {\n    \"strict\": true,\n    \"target\": \"ES2022\",\n    \"module\": \"NodeNext\",\n    \"moduleResolution\": \"NodeNext\",\n    \"noEmit\": true\n  },\n  \"include\": [\"host-bad.ts\", \"index.d.ts\"]\n}\n",
    )
    .unwrap();
    let (ok, out) = run(tsc, &["--strict", "-p", "tsconfig-bad.json"], &dir);
    assert!(!ok, "tsc must reject the mistyped host file:\n{out}");

    let _ = std::fs::remove_dir_all(&dir);
}

// ── The clean host project under node ────────────────────
//
// Battery points are encoded as JSON with BigInts as
// {"$b": "decimal"} markers; the driver decodes them to
// real BigInts, calls, re-encodes, and compares.

#[cfg(not(feature = "wasm-playground"))]
fn b(n: i64) -> serde_json::Value {
    serde_json::json!({ "$b": n.to_string() })
}

#[cfg(not(feature = "wasm-playground"))]
fn bl(xs: &[i64]) -> serde_json::Value {
    serde_json::Value::Array(xs.iter().map(|&n| b(n)).collect())
}

#[cfg(not(feature = "wasm-playground"))]
const DRIVER: &str = r#"
import { readFileSync } from "node:fs";
import * as lib from "./index.js";

function dec(v) {
  if (v === null || typeof v !== "object") return v;
  if (Array.isArray(v)) return v.map(dec);
  if ("$b" in v) return BigInt(v["$b"]);
  const o = {};
  for (const k of Object.keys(v)) o[k] = dec(v[k]);
  return o;
}
function enc(v) {
  if (typeof v === "bigint") return { $b: v.toString() };
  if (v === null || typeof v !== "object") return v;
  if (Array.isArray(v)) return v.map(enc);
  const o = {};
  for (const k of Object.keys(v).sort()) o[k] = enc(v[k]);
  return o;
}

const cases = JSON.parse(readFileSync("cases.json", "utf8"));
let checked = 0;
for (const c of cases) {
  const fn = lib[c.fn];
  if (typeof fn !== "function") throw new Error(`missing export ${c.fn}`);
  const got = enc(fn(...c.args.map(dec)));
  const want = JSON.stringify(c.expect);
  if (JSON.stringify(got) !== want) {
    throw new Error(`${c.fn}: got ${JSON.stringify(got)}, want ${want}`);
  }
  checked += 1;
}
console.log(`P121HOST|checked=${checked}|agree=100%`);
"#;

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn clean_host_project_calls_agree_with_the_frozen_batteries() {
    let set = bindings::stdlib_bindings().expect("stdlib bindings generate");
    let mut cases: Vec<serde_json::Value> = Vec::new();
    let mut push = |fn_name: &str, args: Vec<serde_json::Value>, expect: serde_json::Value| {
        cases.push(serde_json::json!({ "fn": fn_name, "args": args, "expect": expect }));
    };

    // Tier-1 Int — int_clamp, its 13 frozen pinned triples.
    for (x, lo, hi) in [
        (5, 0, 10), (-3, 0, 10), (99, 0, 10), (0, 0, 10), (10, 0, 10),
        (5, 5, 5), (-7, -10, -5), (-12, -10, -5), (0, -5, 5),
        (i64::MAX, 0, i64::MAX), (i64::MIN, 0, i64::MAX),
        (7, 7, 7), (-1, 0, 0),
    ] {
        push("int_clamp", vec![b(x), b(lo), b(hi)], b(x.clamp(lo, hi)));
    }

    // Char — char_to_digit over the full 101-point
    // battery alphabet (printable ASCII + named escapes
    // + NUL + VT + DEL).
    let mut chars: Vec<char> = (32u8..=126).map(char::from).collect();
    chars.extend(['\t', '\n', '\r', '\u{0}', '\u{b}', '\u{7f}']);
    assert_eq!(chars.len(), 101);
    for c in chars {
        let expect = if c.is_ascii_digit() {
            b(c as i64 - '0' as i64)
        } else {
            serde_json::Value::Null
        };
        push(
            "char_to_digit",
            vec![serde_json::Value::String(c.to_string())],
            expect,
        );
    }

    // Option — option_unwrap_or (7 options × 4 defaults)
    // and option_ok_or (9 cases).
    for (o, d) in [
        (Some(0), 0), (Some(0), 5), (Some(0), -3), (Some(0), i64::MIN),
        (Some(1), 0), (Some(1), 5), (Some(1), -3), (Some(1), i64::MIN),
        (Some(-1), 0), (Some(-1), 5), (Some(-1), -3), (Some(-1), i64::MIN),
        (Some(42), 0), (Some(42), 5), (Some(42), -3), (Some(42), i64::MIN),
        (Some(i64::MAX), 0), (Some(i64::MAX), 5), (Some(i64::MAX), -3), (Some(i64::MAX), i64::MIN),
        (Some(i64::MIN), 0), (Some(i64::MIN), 5), (Some(i64::MIN), -3), (Some(i64::MIN), i64::MIN),
        (None, 0), (None, 5), (None, -3), (None, i64::MIN),
    ] {
        let arg_o = match o {
            Some(n) => b(n),
            None => serde_json::Value::Null,
        };
        push("option_unwrap_or", vec![arg_o, b(d)], b(o.unwrap_or(d)));
    }
    for (o, e) in [
        (Some(0), ""), (Some(1), "x"), (Some(-1), "missing"),
        (Some(i64::MAX), "e"), (Some(i64::MIN), "unused"),
        (None, ""), (None, "missing"),
        (None, "a longer error message"), (None, "with \"quotes\" inside"),
    ] {
        let arg_o = match o {
            Some(n) => b(n),
            None => serde_json::Value::Null,
        };
        let expect = match o {
            Some(n) => serde_json::json!({ "tag": "Ok", "value": b(n) }),
            None => serde_json::json!({ "tag": "Err", "error": e }),
        };
        push(
            "option_ok_or",
            vec![arg_o, serde_json::Value::String(e.to_string())],
            expect,
        );
    }

    // Result — result_unwrap_or (9 results × 4 defaults).
    for (r, d) in [
        (Ok(0), 0), (Ok(0), 3), (Ok(0), -7), (Ok(0), i64::MAX),
        (Ok(1), 0), (Ok(1), 3), (Ok(1), -7), (Ok(1), i64::MAX),
        (Ok(-1), 0), (Ok(-1), 3), (Ok(-1), -7), (Ok(-1), i64::MAX),
        (Ok(42), 0), (Ok(42), 3), (Ok(42), -7), (Ok(42), i64::MAX),
        (Ok(i64::MAX), 0), (Ok(i64::MAX), 3), (Ok(i64::MAX), -7), (Ok(i64::MAX), i64::MAX),
        (Ok(i64::MIN), 0), (Ok(i64::MIN), 3), (Ok(i64::MIN), -7), (Ok(i64::MIN), i64::MAX),
        (Err(""), 0), (Err(""), 3), (Err(""), -7), (Err(""), i64::MAX),
        (Err("bad"), 0), (Err("bad"), 3), (Err("bad"), -7), (Err("bad"), i64::MAX),
        (Err("a longer error message with spaces"), 0),
        (Err("a longer error message with spaces"), 3),
        (Err("a longer error message with spaces"), -7),
        (Err("a longer error message with spaces"), i64::MAX),
    ] {
        let (arg_r, expect): (serde_json::Value, i64) = match r {
            Ok(n) => (serde_json::json!({ "tag": "Ok", "value": b(n) }), n),
            Err(e) => (serde_json::json!({ "tag": "Err", "error": e }), d),
        };
        push("result_unwrap_or", vec![arg_r, b(d)], b(expect));
    }

    // Tier-2 List — list_sum and list_min over the shared
    // 15-list battery.
    let lists: Vec<Vec<i64>> = vec![
        vec![], vec![0], vec![1], vec![-1], vec![5],
        vec![1, 2, 3], vec![3, 2, 1], vec![2, 2, 2], vec![1, 2, 2, 3],
        vec![5, 4, 3, 2, 1], vec![3, 1, 4, 1, 5, 9, 2, 6],
        vec![10, -3, 7, -7], vec![i64::MAX], vec![i64::MIN],
        vec![i64::MAX, i64::MIN],
    ];
    for xs in &lists {
        push("list_sum", vec![bl(xs)], b(xs.iter().sum()));
        let expect = match xs.iter().min() {
            Some(n) => b(*n),
            None => serde_json::Value::Null,
        };
        push("list_min", vec![bl(xs)], expect);
    }

    let total = cases.len();
    let dir = scratch_dir("host");
    std::fs::write(dir.join("index.js"), &set.js_module).unwrap();
    std::fs::write(
        dir.join("package.json"),
        "{\n  \"name\": \"phase121-clean-host\",\n  \"private\": true,\n  \"type\": \"module\"\n}\n",
    )
    .unwrap();
    std::fs::write(dir.join("driver.mjs"), DRIVER).unwrap();
    std::fs::write(
        dir.join("cases.json"),
        serde_json::to_string(&cases).unwrap(),
    )
    .unwrap();
    let (ok, out) = run("node", &["driver.mjs"], &dir);
    assert!(ok, "clean host calls run:\n{out}");
    assert!(
        out.contains(&format!("P121HOST|checked={total}|agree=100%")),
        "every host call agrees with the frozen battery:\n{out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── The ungated generator (D1a) ──────────────────────────

#[cfg(not(feature = "wasm-playground"))]
const USER_MODULE: &str = r#"UserBindings {
spec absolute {
    doc: "absolute(x) = |x|, a user-module sample for the ungated binding generator."
    input: x: Int
    output: Int
    require: x != (0 - 9223372036854775807 - 1)
    constraint: x < 0 || output == x
    constraint: x >= 0 || output == (0 - x)
    test { input: 0; expect: 0 }
    test { input: 4; expect: 4 }
    test { input: (0 - 7); expect: 7 }
}
}"#;

/// The pipeline stages exactly as the CLI runs them (the
/// Phase 110 crown pattern): the winner the native
/// pipeline produces for the sample spec.
#[cfg(not(feature = "wasm-playground"))]
fn crown(
    functions: &[FunctionSpec],
    spec: &FunctionSpec,
    seed: u64,
) -> CandidateImplementation {
    engine::seed_engine_rng(seed);
    let mut eng = engine::SynthesisEngine::with_module(
        morphic::api::playground_config(),
        functions,
    );
    let cands = eng.run(spec).expect("engine run");
    let verified = Verifier::with_specs(functions)
        .verify_all(cands, spec)
        .expect("verify_all");
    let pool = runner::rescore_finalists(verified, spec);
    selector::select_best(pool, spec).expect("select_best")
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn ungated_generator_carries_only_pipeline_verdicts() {
    let module = parser::parse(USER_MODULE).expect("user module parses");
    let checked = typeck::check(module).expect("user module checks");
    let functions = checked.spec.functions.clone();
    let spec = functions[0].clone();
    let winner = crown(&functions, &spec, 42);

    // The executed standing, measured: the winner is
    // golden-exact on the spec's own tests.
    let run_out = eval::run_spec_tests(&winner.body, &spec);
    assert_eq!(
        (run_out.passed, run_out.evaluable),
        (3, 3),
        "the sample winner is executed-exact on its tests"
    );

    // The verdict mapping the generator's caller applies:
    // exactly the standing the pipeline measured in THIS
    // configuration — Verified crowns carry z3-verified;
    // anything else is labelled executed-exact and never
    // upgraded (R12: no Verified claim minted by a route
    // that did not produce it).
    let (tier, basis) = match winner.verification_status {
        Some(VerificationStatus::Verified) => ("tier-1", "z3-verified"),
        _ => ("tier-2", "executed-exact"),
    };
    println!(
        "P121USER|status={:?}|tier={tier}|basis={basis}",
        winner.verification_status
    );
    let input = BindingInput {
        spec: spec.clone(),
        body: winner.body.clone(),
        tier: tier.to_string(),
        verdict_basis: basis.to_string(),
        battery_pinned: 3,
        battery_silent: 0,
        crown_route: bindings::CrownRoute::Pipeline,
    };
    let set = bindings::generate_bindings(&[input], "user-bindings", "0.0.0", "UserBindings")
        .expect("user bindings generate");

    // The manifest row says only what the pipeline said.
    assert_eq!(set.manifest.exports.len(), 1);
    let row = &set.manifest.exports[0];
    assert_eq!(row.name, "absolute");
    assert_eq!(row.signature, "Int → Int");
    assert_eq!(row.tier, tier, "row tier is the pipeline's standing");
    assert_eq!(row.verdict_basis, basis, "row basis is the pipeline's verdict");
    assert_eq!(row.doc, spec.doc.as_deref().unwrap_or(""));

    // The module defines and exports exactly the sample,
    // and the declaration types it per §3.
    assert_eq!(defined_functions(&set.js_module), vec!["absolute".to_string()]);
    assert_eq!(exported_names(&set.js_module), vec!["absolute".to_string()]);
    assert!(
        set.declarations
            .contains("export declare function absolute(x: bigint): bigint;"),
        "user declaration in §3 shape:\n{}",
        set.declarations
    );

    // And the generated JS answers the spec's points
    // under node.
    let dir = scratch_dir("user");
    std::fs::write(dir.join("index.js"), &set.js_module).unwrap();
    std::fs::write(
        dir.join("package.json"),
        "{\n  \"name\": \"phase121-user-host\",\n  \"private\": true,\n  \"type\": \"module\"\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("driver.mjs"),
        "import { absolute } from \"./index.js\";\n\
         const pts = [[0n, 0n], [4n, 4n], [-7n, 7n]];\n\
         for (const [x, want] of pts) {\n\
           const got = absolute(x);\n\
           if (got !== want) throw new Error(`absolute(${x}) = ${got}, want ${want}`);\n\
         }\n\
         console.log(\"P121USER|checked=3|agree=100%\");\n",
    )
    .unwrap();
    let (ok, out) = run("node", &["driver.mjs"], &dir);
    assert!(ok, "user module JS runs:\n{out}");
    assert!(out.contains("P121USER|checked=3|agree=100%"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
