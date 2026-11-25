//! Phase 153 — the 1.5 freeze + the Cloudflare Workers
//! edge template (DISEBAR_15_SCOPE §§3, 9). Two pins:
//! the freeze/template shape as frozen, and the
//! template's evidence instrument — the no-globals
//! sandbox smoke: the worker module, linked against the
//! shipped library AS ASSEMBLED IN-TREE (packages/npm),
//! loaded in a bare V8 context with no platform globals
//! and driven through its fetch handler with synthetic
//! events (Tier-1, Tier-2, a higher-order entry with a
//! host-supplied callback, and a malformed request
//! answered by name rather than thrown).

use std::process::Command;

fn manifest_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FREEZE-1: DISEBAR_15_SCOPE.md freezes the milestone as
/// ratified (D1a–D7a) — the four components and nothing
/// else, lockstep 1.5.0, the gate's closing line verbatim
/// — and the template exists at its frozen home with its
/// frozen contents.
#[test]
fn p153_freeze_and_template_shape() {
    let root = manifest_dir();
    let scope = std::fs::read_to_string(root.join("DISEBAR_15_SCOPE.md"))
        .expect("the 1.5 freeze exists");
    // Whitespace-flattened: the freeze's prose wraps, its
    // clauses don't change.
    let scope = scope.split_whitespace().collect::<Vec<_>>().join(" ");
    for clause in [
        "D1a–D7a",
        "Lockstep **1.5.0**",
        "templates/cloudflare-workers/",
        "--package-from <file.morph>",
        "unpositioned: <verbatim message>",
        "packages/vscode/",
        "76,800 B stands",
        "771,072 B stands",
        // The gate's closing line, frozen verbatim.
        "\"1.5 closes when a proven function can be serving at the \
         edge from a template anyone can copy — and when the editor \
         tells the truth about a broken spec: exactly where it broke \
         when the compiler knows, and plainly unpositioned when it \
         doesn't.\"",
    ] {
        assert!(scope.contains(clause), "the freeze carries: {clause}");
    }

    let template = root.join("templates/cloudflare-workers");
    let worker = std::fs::read_to_string(template.join("src/index.js"))
        .expect("the worker entry exists");
    assert!(worker.contains("from \"@thvtzy/morphic\""), "imports the shipped package");
    assert!(worker.contains("/fn/"), "routes /fn/<name>");
    let wrangler = std::fs::read_to_string(template.join("wrangler.toml"))
        .expect("wrangler.toml exists");
    assert!(wrangler.contains("main = \"src/index.js\""));
    assert!(wrangler.contains("compatibility_date"));
    let pkg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(template.join("package.json"))
            .expect("the template package.json exists"),
    )
    .expect("package.json parses");
    assert!(
        pkg["dependencies"]["@thvtzy/morphic"].is_string(),
        "the template's one dependency is the shipped package"
    );
    let readme = std::fs::read_to_string(template.join("README.md"))
        .expect("the template README exists");
    for named in ["account act", "BigInt", "Tier-1", "Tier-2"] {
        assert!(readme.contains(named), "the README names: {named}");
    }
    println!(
        "P153|freeze scope=frozen-d1a-d7a lockstep=1.5.0 closing_line=verbatim \
         template=templates/cloudflare-workers"
    );
}

/// EDGE-1: the sandbox smoke. The worker module is
/// evaluated in a bare node `vm` context — no `process`,
/// no `require`, no `Buffer`, not even `fetch` — with its
/// `@thvtzy/morphic` import linked to the in-tree
/// assembly. Its fetch handler is then driven with
/// synthetic events; every answer is checked
/// battery-exact against the entries' known values.
#[test]
fn p153_edge_template_sandbox_smoke() {
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
const tier1 = await call(post("int_clamp", { args: [15, 0, 10] }));
const tier2 = await call(post("list_sort", { args: [[3, 1, 2]] }));
const ho = await call(post("list_map", { args: [[1, 2, 3], (x) => x + 10n] }));
const unknown = await call(post("nope", { args: [] }));
const malformed = await call({
  method: "POST",
  url: "https://worker.test/fn/int_clamp",
  json: async () => { throw new SyntaxError("Unexpected end of JSON input"); },
});
console.log(JSON.stringify({
  process_in_ctx: vm.runInContext("typeof process", context),
  require_in_ctx: vm.runInContext("typeof require", context),
  buffer_in_ctx: vm.runInContext("typeof Buffer", context),
  fetch_in_ctx: vm.runInContext("typeof fetch", context),
  response_in_ctx: vm.runInContext("typeof Response", context),
  tier1, tier2, ho, unknown, malformed,
}));
"#;
    let dir = std::env::temp_dir().join(format!("morphic-phase153-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("smoke.mjs"), harness).unwrap();
    let root = manifest_dir();
    let out = Command::new("node")
        .arg("--experimental-vm-modules")
        .arg(dir.join("smoke.mjs"))
        .arg(root.join("templates/cloudflare-workers/src/index.js"))
        .arg(root.join("packages/npm/index.js"))
        .output()
        .unwrap_or_else(|e| panic!("node must run (node v24 on PATH): {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "sandbox smoke failed\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    let line = stdout.lines().last().expect("one JSON line").to_string();
    let v: serde_json::Value = serde_json::from_str(&line).expect("harness JSON parses");
    for global in [
        "process_in_ctx",
        "require_in_ctx",
        "buffer_in_ctx",
        "fetch_in_ctx",
        "response_in_ctx",
    ] {
        assert_eq!(v[global], "undefined", "{global} must be absent in the sandbox");
    }
    // Tier-1 (z3-verified): int_clamp(15, 0, 10) = 10,
    // BigInt out as a decimal string.
    assert_eq!(v["tier1"]["status"], 200);
    assert_eq!(v["tier1"]["payload"]["result"], "10");
    // Tier-2 (executed-exact): list_sort([3,1,2]).
    assert_eq!(v["tier2"]["status"], 200);
    assert_eq!(v["tier2"]["payload"]["result"], serde_json::json!(["1", "2", "3"]));
    // Higher-order with a host-supplied callback:
    // list_map([1,2,3], x => x + 10).
    assert_eq!(v["ho"]["status"], 200);
    assert_eq!(v["ho"]["payload"]["result"], serde_json::json!(["11", "12", "13"]));
    // Unknown export: answered by name, not thrown.
    assert_eq!(v["unknown"]["status"], 404);
    assert_eq!(v["unknown"]["payload"]["error"]["code"], "unknown_function");
    // Malformed JSON: the named error shape, not a throw
    // (the harness call itself returned normally).
    assert_eq!(v["malformed"]["status"], 400);
    assert_eq!(v["malformed"]["payload"]["error"]["code"], "malformed_request");
    println!(
        "P153|edge|smoke globals=none tier1=int_clamp->\"10\" tier2=list_sort->[1,2,3] \
         ho=list_map+host-callback->[11,12,13] malformed=malformed_request-400 \
         unknown=unknown_function-404"
    );
}
