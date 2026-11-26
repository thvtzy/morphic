//! Phase 157 — the 1.5 gate run's own instrument: the
//! edge template serving from both populations, the
//! verb's honesty, the editor-truth transcript, the
//! thin client, and the footprint, re-run fresh on the
//! gate tree, clause by clause (DISEBAR_15_SCOPE.md
//! §9). Nothing here quotes Phase 153–156's evidence:
//! every pin below executes on this tree.
//!
//! What this file pins, by gate clause (§9):
//!
//! - **Clause 1 — edge serves.** The Cloudflare
//!   Workers template, populated once with the shipped
//!   library (packages/npm as assembled in-tree) and
//!   once with a user package built by the verb on
//!   this tree, loads in the no-globals sandbox (a
//!   bare node `vm` context: no process / require /
//!   Buffer / fetch / Response) and answers its smoke
//!   calls battery-exact through the fetch handler;
//!   the template's file set is exactly its four
//!   frozen files, of which the only platform-shaped
//!   code is the handler (src/index.js) + the config
//!   (wrangler.toml).
//! - **Clause 2 — the verb is honest.** A fresh verb
//!   assembly of the exhibit module: every manifest
//!   row equals the assembly's own standings, and the
//!   standings reproduce under an independent
//!   re-judgment — one `synthesize_seeded_spec_winner`
//!   call per spec, its winner's status and counts
//!   recomputed here, not read from the package. The
//!   uncrownable exhibit is refused BY NAME at the
//!   CLI with the output directory never created. No
//!   claim-shaped flag exists on the build surface.
//! - **Clause 3 — the editor tells the truth.** The
//!   scripted LSP transcript against the gate tree's
//!   shipped binary: the stream's first bytes are a
//!   Content-Length header (no banner byte); a parse
//!   diagnostic at its true position; a typecheck
//!   diagnostic at 0:0 carrying the `unpositioned: `
//!   prefix over the verbatim message; a clean file
//!   yields an empty set; shutdown → exit 0.
//! - **Clause 4 — the client is thin.** The extension
//!   version equals the crate version (the lockstep
//!   pin — the stamp moves both, this pin is how the
//!   move is proven); the `.vsix` named for that
//!   version packages from `packages/vscode`' own
//!   tree and the manifest inside it contributes
//!   exactly the frozen surface, asserted identically
//!   against the source manifest; the dependency
//!   world is confined to `packages/vscode`; the
//!   README names marketplace publishing as the
//!   owner's later act (untouched here).
//! - **Clause 5 — footprint.** The compiler's
//!   [dependencies] set is the standing set; the
//!   library npm package stands at 77 files inside
//!   the frozen 76,800 B budget. (The Cargo.lock
//!   zero-diff against v1.4.0 and the playground
//!   bundle's rebuilt mass are the gate record's
//!   shell evidence beside these pins: a test binary
//!   cannot diff a tag or rebuild WASM. The suites
//!   themselves, in the three frozen configurations,
//!   are the clause's other half.)
//!
//! Crown-bearing tests serialize on one lock with the
//! guard recovered from poisoning (the standing
//! discipline: heavy crowns flip under parallel load
//! on this box; no assertion is weakened, and the
//! gate's evidence rests on solo runs regardless).
//!
//! Configuration shape: clauses 1–2 need the native
//! prelude (the verb's seeded runs), so they are
//! `not(wasm-playground)` gated (under CI's
//! feature-unified build the prelude is the
//! uninstalled runtime asset — the Phase 144 lesson);
//! clause 2's Verified standings additionally need
//! the verifier, so it is `z3-support` gated (the
//! no-Z3 executed-rows standing is Phase 154's own
//! pin). Clauses 3–5 are configuration-independent:
//! the LSP serves parse+typecheck identically in
//! every build, and the footprint is files on disk.

use morphic::api;
use morphic::codegen::package::assemble_user_package;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

static LOCK: Mutex<()> = Mutex::new(());

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase157-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The Phase 154 exhibit modules, re-stated here so
/// the gate's evidence is this file's own: module A is
/// admissible (two Verified crowns — one higher-order —
/// and one executed-exact at seed 42); module B carries
/// one spec whose battery is partial by construction
/// (a `require:`-violating point is spec-silent for
/// every body, on an Inconclusive fold crown), which
/// neither admission branch can take.
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

// ── Clause 1: edge serves, both populations ─────────────

/// The no-globals sandbox harness (the Phase 153/154
/// instrument, re-stated): the worker module evaluates
/// in a bare node `vm` context and its fetch handler
/// answers the calls the script names.
fn sandbox_run(worker: &std::path::Path, library: &std::path::Path, calls: &str) -> Value {
    let harness = format!(
        r#"
import vm from "node:vm";
import {{ readFileSync }} from "node:fs";
const [workerPath, libraryPath] = process.argv.slice(2);
const context = vm.createContext(Object.create(null));
const library = new vm.SourceTextModule(readFileSync(libraryPath, "utf8"), {{
  context, identifier: "morphic-library",
}});
await library.link(() => {{ throw new Error("the library imports nothing"); }});
const worker = new vm.SourceTextModule(readFileSync(workerPath, "utf8"), {{
  context, identifier: "morphic-worker",
}});
await worker.link((specifier) => {{
  if (specifier === "@thvtzy/morphic") return library;
  throw new Error(`unexpected import: ${{specifier}}`);
}});
await worker.evaluate();
const handler = worker.namespace.default.fetch;
const post = (name, body) => ({{
  method: "POST",
  url: `https://worker.test/fn/${{name}}`,
  json: async () => body,
}});
const call = async (request) => {{
  const res = await handler(request, {{}}, {{}});
  return {{ status: res.status, payload: JSON.parse(res.body) }};
}};
{calls}
console.log(JSON.stringify({{
  process_in_ctx: vm.runInContext("typeof process", context),
  require_in_ctx: vm.runInContext("typeof require", context),
  buffer_in_ctx: vm.runInContext("typeof Buffer", context),
  fetch_in_ctx: vm.runInContext("typeof fetch", context),
  response_in_ctx: vm.runInContext("typeof Response", context),
  ...results,
}}));
"#
    );
    let dir = scratch_dir("sandbox");
    std::fs::write(dir.join("smoke.mjs"), harness).unwrap();
    let out = Command::new("node")
        .arg("--experimental-vm-modules")
        .arg(dir.join("smoke.mjs"))
        .arg(worker)
        .arg(library)
        .output()
        .expect("node runs (node v24 on PATH)");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "the sandbox smoke runs\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    let v: Value = serde_json::from_str(stdout.lines().last().expect("one JSON line"))
        .expect("the harness JSON parses");
    for global in [
        "process_in_ctx",
        "require_in_ctx",
        "buffer_in_ctx",
        "fetch_in_ctx",
        "response_in_ctx",
    ] {
        assert_eq!(v[global], "undefined", "{global} must be absent in the sandbox");
    }
    v
}

#[test]
#[cfg(not(feature = "wasm-playground"))]
fn p157_clause1_edge_serves_both_populations() {
    let _guard = crown_serial();
    let root = root();
    let template = root.join("templates/cloudflare-workers");

    // The template's file set is exactly its frozen
    // four files; of those, the only platform-shaped
    // code is the handler + the config (package.json
    // is a dependency fact, the README is prose).
    let mut files: Vec<String> = Vec::new();
    let mut stack = vec![template.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("the template reads") {
            let p = entry.expect("the entry reads").path();
            if p.is_dir() {
                stack.push(p);
            } else {
                files.push(
                    p.strip_prefix(&template)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    files.sort();
    assert_eq!(
        files,
        ["README.md", "package.json", "src/index.js", "wrangler.toml"],
        "the template is exactly its four frozen files"
    );
    let worker_src = std::fs::read_to_string(template.join("src/index.js")).unwrap();
    assert!(worker_src.contains("from \"@thvtzy/morphic\""), "the handler imports the package");
    let wrangler = std::fs::read_to_string(template.join("wrangler.toml")).unwrap();
    assert!(wrangler.contains("main = \"src/index.js\""), "the config points at the handler");

    // Population 1: the shipped library, as assembled
    // in-tree. Battery-exact through the handler:
    // Tier-1, Tier-2, a higher-order entry with a
    // host-supplied callback, and the named refusals.
    let v = sandbox_run(
        &template.join("src/index.js"),
        &root.join("packages/npm/index.js"),
        r#"
const results = {
  tier1: await call(post("int_clamp", { args: [15, 0, 10] })),
  tier2: await call(post("list_sort", { args: [[3, 1, 2]] })),
  ho: await call(post("list_map", { args: [[1, 2, 3], (x) => x + 10n] })),
  unknown: await call(post("nope", { args: [] })),
  malformed: await call({
    method: "POST",
    url: "https://worker.test/fn/int_clamp",
    json: async () => { throw new SyntaxError("Unexpected end of JSON input"); },
  }),
};
"#,
    );
    assert_eq!(v["tier1"]["status"], 200);
    assert_eq!(v["tier1"]["payload"]["result"], "10", "Tier-1 int_clamp(15,0,10) as a decimal string");
    assert_eq!(v["tier2"]["status"], 200);
    assert_eq!(v["tier2"]["payload"]["result"], json!(["1", "2", "3"]), "Tier-2 list_sort([3,1,2])");
    assert_eq!(v["ho"]["status"], 200);
    assert_eq!(v["ho"]["payload"]["result"], json!(["11", "12", "13"]), "list_map with a host callback");
    assert_eq!(v["unknown"]["status"], 404);
    assert_eq!(v["unknown"]["payload"]["error"]["code"], "unknown_function");
    assert_eq!(v["malformed"]["status"], 400);
    assert_eq!(v["malformed"]["payload"]["error"]["code"], "malformed_request");

    // Population 2: a user package built by the verb
    // on this tree, written to a scratch directory and
    // linked the same way.
    let dir = scratch_dir("userpkg");
    let pkg = dir.join("pkg");
    std::fs::create_dir_all(&pkg).unwrap();
    let assembly = assemble_user_package(MODULE_A, "widget-math", "0.2.0", "widget_math.morph", 42)
        .expect("the exhibit module assembles on the gate tree");
    for f in &assembly.files {
        std::fs::write(pkg.join(&f.path), &f.contents).unwrap();
    }
    let v = sandbox_run(
        &template.join("src/index.js"),
        &pkg.join("index.js"),
        r#"
const results = {
  sorted: await call(post("weights_sorted", { args: [[1, 2, 3]] })),
  unsorted: await call(post("weights_sorted", { args: [[3, 1, 2]] })),
  code: await call(post("is_widget_code", { args: [6] })),
  code_out: await call(post("is_widget_code", { args: [7] })),
};
"#,
    );
    assert_eq!(v["sorted"]["status"], 200);
    assert_eq!(v["sorted"]["payload"]["result"], true);
    assert_eq!(v["unsorted"]["status"], 200);
    assert_eq!(v["unsorted"]["payload"]["result"], false);
    assert_eq!(v["code"]["status"], 200);
    assert_eq!(v["code"]["payload"]["result"], true);
    assert_eq!(v["code_out"]["status"], 200);
    assert_eq!(v["code_out"]["payload"]["result"], false);
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "P157|clause1|edge template=4-files handler+config only; library: int_clamp->\"10\" \
         list_sort->[1,2,3] list_map+callback->[11,12,13] unknown=404 malformed=400; \
         verb-built user package: weights_sorted true/false, is_widget_code true/false — \
         all battery-exact in the no-globals sandbox"
    );
}

// ── Clause 2: the verb is honest ────────────────────────

#[test]
#[cfg(all(feature = "z3-support", not(feature = "wasm-playground")))]
fn p157_clause2_verb_honesty() {
    let _guard = crown_serial();

    // A fresh assembly on this tree: the manifest rows
    // are the assembly's own standings, exactly.
    let assembly = assemble_user_package(MODULE_A, "widget-math", "0.2.0", "widget_math.morph", 42)
        .expect("the exhibit module assembles on the gate tree");
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
    let manifest_file = assembly
        .files
        .iter()
        .find(|f| f.path == "morphic.manifest.json")
        .expect("the package carries its manifest");
    let manifest: Value =
        serde_json::from_str(&manifest_file.contents).expect("the manifest parses");
    let exports = manifest["exports"].as_array().expect("exports list");
    assert_eq!(exports.len(), assembly.standings.len());
    for (row, standing) in exports.iter().zip(&assembly.standings) {
        assert_eq!(row["name"], standing.name);
        assert_eq!(row["tier"], standing.tier);
        assert_eq!(row["verdictBasis"], standing.verdict_basis);
        assert_eq!(row["battery"]["pinned"], standing.passed);
        assert_eq!(row["battery"]["specSilent"], standing.spec_silent);
    }

    // The claims reproduce under an independent
    // re-judgment: one seeded run per spec through the
    // API, its winner's status and counts recomputed
    // here from the report — not read from the package.
    for standing in &assembly.standings {
        let (report, _winner) =
            api::synthesize_seeded_spec_winner(MODULE_A, &standing.name, 42)
                .unwrap_or_else(|e| panic!("the re-judgment run for {} succeeds", standing.name));
        let w = &report.winner;
        let spec_silent = w
            .test_outcomes
            .iter()
            .filter(|o| o.outcome == "spec_silent")
            .count();
        assert_eq!(w.verification_status, standing.status, "{}: the verdict reproduces", standing.name);
        assert_eq!(w.tests_passed, standing.passed, "{}: passed reproduces", standing.name);
        assert_eq!(w.tests_evaluable, standing.evaluable, "{}: evaluable reproduces", standing.name);
        assert_eq!(w.tests_total, standing.total, "{}: total reproduces", standing.name);
        assert_eq!(spec_silent, standing.spec_silent, "{}: spec-silent reproduces", standing.name);
    }

    // The uncrownable exhibit refuses BY NAME at the
    // CLI, and nothing is written: the output
    // directory is never created.
    let dir = scratch_dir("refusal");
    let module_path = dir.join("batch_totals.morph");
    std::fs::write(&module_path, MODULE_B).unwrap();
    let out_dir = dir.join("must-not-exist");
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args([
            "build",
            "--package-from",
            module_path.to_str().unwrap(),
            "--out",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .expect("the binary runs");
    assert!(!out.status.success(), "the uncrownable module refuses");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        stderr.contains("batch_total"),
        "the refusal names the spec: {stderr}"
    );
    assert!(
        !out_dir.exists(),
        "nothing is written on refusal — the output directory was never created"
    );
    let _ = std::fs::remove_dir_all(&dir);

    // No claim-shaped flag exists on the build
    // surface: a tier, a verdict, a basis, or a
    // battery figure cannot be supplied by a caller.
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args(["build", "--help"])
        .output()
        .expect("the binary runs");
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout).to_string();
    for flag in ["--package-from", "--package-name", "--package-version", "--seed", "--out"] {
        assert!(help.contains(flag), "the help carries {flag}");
    }
    for claim in ["--tier", "--verdict", "--basis", "--battery"] {
        assert!(
            !help.contains(claim),
            "no caller-stated-claims flag exists: {claim} must be absent"
        );
    }
    println!(
        "P157|clause2|verb manifest rows == the run's measurements; independent \
         re-judgment reproduces every standing; batch_total refused by name at the \
         CLI with nothing written; no claim-shaped flag exists"
    );
}

// ── Clause 3: the editor tells the truth ────────────────

const LSP_CLEAN: &str = "Clean {\n    spec plus_one {\n        input: x: Int\n        output: Int\n        constraint: output == x + 1\n        test \"one\" { input: 0 expect: 1 }\n    }\n}";
const LSP_PARSE_BROKEN: &str =
    "ParseBroken {\n    spec f {\n        input: a: Int\n        output: Int\n        constraint: output == a +\n    }\n}";
const LSP_TYPE_ERR: &str =
    "Typed {\n    spec f {\n        input: a: Int\n        output: Int\n        constraint: output == a ++ 2\n    }\n}";
const LSP_CLEAN_URI: &str = "file:///clean.morph";
const LSP_BROKEN_URI: &str = "file:///broken.morph";

fn lsp_frame(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
}

fn lsp_parse_frames(bytes: &[u8]) -> Vec<Value> {
    let mut frames = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let header_end = rest
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("a frame header terminates");
        let header = std::str::from_utf8(&rest[..header_end]).expect("headers are ASCII");
        let len: usize = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.trim().eq_ignore_ascii_case("content-length") {
                    value.trim().parse().ok()
                } else {
                    None
                }
            })
            .expect("every frame names its length");
        let body = &rest[header_end + 4..header_end + 4 + len];
        frames.push(serde_json::from_slice(body).expect("every frame body is JSON"));
        rest = &rest[header_end + 4 + len..];
    }
    frames
}

fn lsp_run_transcript(script: Vec<Vec<u8>>) -> (Vec<u8>, std::process::ExitStatus) {
    let mut child: Child = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the binary spawns");
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        let _ = tx.send(bytes);
    });
    {
        let mut stdin = child.stdin.take().unwrap();
        for bytes in &script {
            stdin.write_all(bytes).expect("the script writes");
        }
    }
    let bytes = rx
        .recv_timeout(Duration::from_secs(180))
        .expect("the server answers and the stream closes");
    let _ = reader.join();
    let status = child.wait().expect("the child reaps");
    (bytes, status)
}

#[test]
fn p157_clause3_editor_truth_transcript() {
    let script: Vec<Vec<u8>> = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"capabilities": {}}})
            .to_string(),
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": LSP_CLEAN_URI, "languageId": "morphic", "version": 1, "text": LSP_CLEAN}}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": LSP_BROKEN_URI, "languageId": "morphic", "version": 1, "text": LSP_PARSE_BROKEN}}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"textDocument": {"uri": LSP_BROKEN_URI, "version": 2}, "contentChanges": [{"text": LSP_TYPE_ERR}]}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"textDocument": {"uri": LSP_BROKEN_URI, "version": 3}, "contentChanges": [{"text": LSP_CLEAN}]}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": LSP_BROKEN_URI}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": {}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "exit"}).to_string(),
    ]
    .iter()
    .map(|body| lsp_frame(body))
    .collect();

    let (bytes, status) = lsp_run_transcript(script);
    assert!(status.success(), "shutdown then exit → status 0: {status}");
    // No banner byte ever touches the stream: the
    // first bytes out are a Content-Length header.
    assert!(
        bytes.starts_with(b"Content-Length: "),
        "the stream opens with framing, not the banner: {:?}",
        String::from_utf8_lossy(&bytes[..bytes.len().min(64)])
    );

    let frames = lsp_parse_frames(&bytes);
    assert_eq!(frames.len(), 7, "one answer per step of the script: {frames:?}");
    assert_eq!(frames[0]["id"], 1);
    assert_eq!(frames[0]["result"]["capabilities"]["textDocumentSync"]["change"], 1);
    assert_eq!(frames[0]["result"]["capabilities"]["textDocumentSync"]["openClose"], true);

    // A clean file yields an empty set.
    assert_eq!(frames[1]["params"]["uri"], LSP_CLEAN_URI);
    assert_eq!(frames[1]["params"]["diagnostics"], json!([]));

    // A parse diagnostic at its true position: the
    // API's (6,5) 1-based as (5,4) 0-based, the
    // message the API's own.
    let parse_error = api::check(LSP_PARSE_BROKEN).error.unwrap();
    let diagnostics = frames[2]["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["range"]["start"], json!({"line": 5, "character": 4}));
    assert_eq!(diagnostics[0]["range"]["end"], json!({"line": 5, "character": 4}));
    assert_eq!(diagnostics[0]["message"], parse_error.message);

    // A typecheck diagnostic at 0:0 carrying its
    // unpositioned name over the verbatim message.
    let type_error = api::check(LSP_TYPE_ERR).error.unwrap();
    let diagnostics = frames[3]["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["range"]["start"], json!({"line": 0, "character": 0}));
    assert_eq!(diagnostics[0]["range"]["end"], json!({"line": 0, "character": 0}));
    assert_eq!(
        diagnostics[0]["message"],
        format!("unpositioned: {}", type_error.message)
    );

    // Back to clean → the empty set; the close → the
    // clear; shutdown → null.
    assert_eq!(frames[4]["params"]["diagnostics"], json!([]));
    assert_eq!(frames[5]["params"]["uri"], LSP_BROKEN_URI);
    assert_eq!(frames[5]["params"]["diagnostics"], json!([]));
    assert_eq!(frames[6]["id"], 2);
    assert_eq!(frames[6]["result"], Value::Null);
    println!(
        "P157|clause3|transcript frames=7 first-bytes=Content-Length (no banner) \
         parse@(5,4) type@0:0 'unpositioned: ' clean=empty shutdown->exit=0"
    );
}

// ── Clause 4: the client is thin ────────────────────────

fn read_json(path: &std::path::Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} reads: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} parses as JSON: {e}", path.display()))
}

fn crate_version() -> String {
    let cargo_toml = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("version = ") {
            return rest.trim().trim_matches('"').to_string();
        }
    }
    panic!("Cargo.toml names a version");
}

/// The frozen contributed surface (Phase 156's
/// assertion, re-stated): contributes is exactly
/// languages + grammars + configuration; the one
/// configuration key is morphic.serverPath.
fn assert_frozen_surface(manifest: &Value, where_: &str) {
    assert_eq!(manifest["name"], "morphic", "{where_}: the package name");
    assert_eq!(manifest["main"], "./out/extension.js", "{where_}: the entry point");
    assert_eq!(
        manifest["activationEvents"],
        json!(["onLanguage:morphic"]),
        "{where_}: activation is on the morphic language, nothing else"
    );
    let contributes = &manifest["contributes"];
    let mut keys: Vec<&str> = contributes
        .as_object()
        .expect("contributes is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        ["configuration", "grammars", "languages"],
        "{where_}: contributes is exactly languages + grammars + configuration"
    );
    let languages = contributes["languages"].as_array().unwrap();
    assert_eq!(languages.len(), 1, "{where_}: one language");
    assert_eq!(languages[0]["id"], "morphic");
    assert_eq!(languages[0]["extensions"], json!([".morph"]));
    let grammars = contributes["grammars"].as_array().unwrap();
    assert_eq!(grammars.len(), 1, "{where_}: one grammar");
    assert_eq!(grammars[0]["scopeName"], "source.morph");
    let properties = &contributes["configuration"]["properties"];
    let prop_keys: Vec<&str> = properties
        .as_object()
        .expect("configuration properties is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(prop_keys, ["morphic.serverPath"], "{where_}: exactly one configuration key");
    assert_eq!(properties["morphic.serverPath"]["default"], "morphic");
    assert_eq!(properties["morphic.serverPath"]["scope"], "machine");
}

#[test]
fn p157_clause4_thin_client() {
    let root = root();
    let dir = root.join("packages/vscode");
    let manifest = read_json(&dir.join("package.json"));

    // The lockstep pin: the extension's version is the
    // crate's version. The stamp moves both; this pin
    // is how the move is proven on the stamped tree.
    let version = crate_version();
    assert_eq!(
        manifest["version"], version,
        "the extension version moves with the crate version"
    );
    assert_frozen_surface(&manifest, "source manifest");

    // The .vsix named for that version packages from
    // the extension's own tree, and the manifest
    // inside it contributes the same frozen surface.
    let vsix = dir.join(format!("morphic-{version}.vsix"));
    assert!(vsix.exists(), "the packaged extension stands at {}", vsix.display());
    let contained = if cfg!(windows) {
        Command::new("tar")
            .arg("-xOf")
            .arg(&vsix)
            .arg("extension/package.json")
            .output()
            .expect("tar reads the contained manifest")
    } else {
        Command::new("unzip")
            .arg("-p")
            .arg(&vsix)
            .arg("extension/package.json")
            .output()
            .expect("unzip reads the contained manifest")
    };
    assert!(contained.status.success(), "the .vsix is a readable zip");
    let contained: Value =
        serde_json::from_slice(&contained.stdout).expect("the contained manifest parses");
    assert_eq!(contained["version"], version, "the contained manifest is at the crate version");
    assert_frozen_surface(&contained, "contained manifest");

    // The dependency world is confined to
    // packages/vscode: its lockfile stands there and
    // names the quarantined dependency; no Node
    // furniture leaks to the root or the siblings.
    let lock = read_json(&dir.join("package-lock.json"));
    assert!(
        lock.to_string().contains("vscode-languageclient"),
        "the lockfile carries the quarantined dependency"
    );
    for absent in [
        root.join("package.json"),
        root.join("package-lock.json"),
        root.join("node_modules"),
        root.join("packages/npm/node_modules"),
        root.join("packages/npm/package-lock.json"),
        root.join("packages/rust/node_modules"),
    ] {
        assert!(
            !absent.exists(),
            "no Node furniture at {} — the quarantine holds",
            absent.display()
        );
    }

    // The marketplace is untouched: the README names
    // publishing there as the owner's later act, and
    // no script in the package performs one.
    let readme = std::fs::read_to_string(dir.join("README.md")).expect("the README reads");
    assert!(
        readme.to_lowercase().contains("marketplace"),
        "the README names the marketplace as the owner's later act"
    );
    let scripts = manifest["scripts"].to_string();
    assert!(
        !scripts.contains("vsce publish"),
        "no script performs a marketplace publish: {scripts}"
    );
    println!(
        "P157|clause4|client version==crate ({version}); morphic-{version}.vsix contained \
         manifest == frozen surface; dependencies confined to packages/vscode; \
         marketplace untouched (owner's later act, named in the README)"
    );
}

// ── Clause 5: footprint ─────────────────────────────────

fn dir_mass(path: &std::path::Path) -> (usize, usize) {
    let mut files = 0usize;
    let mut bytes = 0usize;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("the directory reads") {
            let entry = entry.expect("the entry reads");
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                files += 1;
                bytes += entry.metadata().expect("metadata").len() as usize;
            }
        }
    }
    (files, bytes)
}

#[test]
fn p157_clause5_footprint() {
    let root = root();

    // The compiler tree's dependency set is unchanged:
    // the standing 13.
    let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
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

    // The library npm package stands inside the frozen
    // budget: 77 files, ≤ 76,800 B unpacked.
    let (npm_files, npm_bytes) = dir_mass(&root.join("packages/npm"));
    assert_eq!(npm_files, 77, "the library package's file set unmoved");
    assert!(
        npm_bytes <= 76_800,
        "the library package's mass {npm_bytes} B stands inside the 76,800 B budget"
    );
    println!(
        "P157|clause5|footprint cargo=standing-13 npm=77-files/{npm_bytes}B<=76800B \
         (Cargo.lock zero-diff vs v1.4.0 and the playground bundle mass are the \
         gate record's shell evidence)"
    );
}
