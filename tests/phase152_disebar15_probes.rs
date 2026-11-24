//! Phase 152 — scope-audit probes for the 1.5 follow-ons
//! (edge deploy template · LSP diagnostics-only · VS Code
//! extension), named as 1.5 in DISEBAR_SCOPE §6 and
//! measured component by component in DISEBAR_AUDIT
//! §§4–5. This phase re-measures those two sections
//! against the v1.4.0 tree and prices the campaign.
//!
//! Every test prints a `P152|...` line with the measured
//! standing; assertions pin exactly what the line
//! reports. Verdict-bearing pins branch on `z3-support`
//! exactly as the Phase 127/133/137/148 probes do.

use morphic::api;
use morphic::codegen::bindings::{self, BindingInput, CrownRoute};
use morphic::mcp::McpServer;
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::IRNode;
use morphic::synthesis::eval;
use std::io::Read as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const VALIDATOR: &str = include_str!("../examples/self_host/validator.morph");
const BIG: &str = include_str!("../examples/self_host/codegen.morph");
const INVALID: &str = "spec broken { input: x: Int output: Int constraint: ";
const TYPE_ERR: &str = "Typed {\n\
    spec f {\n\
        input: a: Int\n\
        output: Int\n\
        constraint: output == a ++ 2\n\
    }\n\
    }";
const TWO_TYPE_ERRS: &str = "Two {\n\
    spec f {\n\
        input: a: Int\n\
        output: Int\n\
        constraint: output == a ++ 2\n\
    }\n\
    spec g {\n\
        input: b: Int\n\
        output: Int\n\
        constraint: output == b ++ 3\n\
    }\n\
    }";
const PLUS_ONE: &str = "Probe {\n\
    spec plus_one {\n\
        input: x: Int\n\
        output: Int\n\
        constraint: output == x + 1\n\
        test \"one\" { input: 0 expect: 1 }\n\
        test \"two\" { input: 41 expect: 42 }\n\
    }\n\
    }";

fn manifest_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// ── Edge ─────────────────────────────────────────────────

/// EDGE-1: the shipped npm artifact's purity and mass,
/// re-measured at 1.4.0 — every `.js` file in
/// packages/npm is free of Node-surface tokens, and the
/// package declares no dependency surface at all. This
/// is the fact the deploy template's platform choice
/// rests on.
#[test]
fn p152_edge_artifact_purity_and_mass() {
    let dir = manifest_dir().join("packages/npm");
    let forbidden = [
        "require(", "node:", "process.", "Buffer", "__dirname", "__filename",
        "\"fs\"", "'fs'",
    ];
    let mut js_files = 0usize;
    let mut total_js = 0usize;
    let mut index_js = 0usize;
    for entry in std::fs::read_dir(&dir).expect("packages/npm reads") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|e| e == "js") {
            let text = std::fs::read_to_string(&path).expect("js file reads");
            for tok in forbidden {
                assert!(
                    !text.contains(tok),
                    "{} must not contain the Node-surface token {tok:?}",
                    path.display()
                );
            }
            js_files += 1;
            total_js += text.len();
            if path.file_name().is_some_and(|n| n == "index.js") {
                index_js = text.len();
            }
        }
    }
    let pkg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("package.json")).expect("package.json reads"),
    )
    .expect("package.json parses");
    assert_eq!(pkg["type"], "module");
    assert!(pkg.get("dependencies").is_none(), "no runtime dependencies");
    assert!(pkg.get("devDependencies").is_none(), "no dev dependencies");
    println!(
        "P152|edge|artifact js_files={js_files} index_js_bytes={index_js} \
         total_js_bytes={total_js} node_surface=none dependencies=none"
    );
}

/// EDGE-2: the module loads and computes in a
/// Workers-style sandbox — a bare V8 context (node's
/// `vm`) with NO platform globals at all: no `process`,
/// no `require`, no `Buffer`, not even `fetch`. This is
/// the strictest of the candidate platforms' shapes,
/// measured rather than asserted: whatever runs here
/// runs on any of them.
#[test]
fn p152_edge_sandbox_loads_without_node_globals() {
    let harness = r#"
import vm from "node:vm";
import { readFileSync } from "node:fs";
const context = vm.createContext(Object.create(null));
const src = readFileSync(process.argv[2], "utf8");
const mod = new vm.SourceTextModule(src, { context, identifier: "morphic-index" });
await mod.link(() => { throw new Error("no imports expected"); });
await mod.evaluate();
const ns = mod.namespace;
console.log(JSON.stringify({
  process_in_ctx: vm.runInContext("typeof process", context),
  require_in_ctx: vm.runInContext("typeof require", context),
  buffer_in_ctx: vm.runInContext("typeof Buffer", context),
  fetch_in_ctx: vm.runInContext("typeof fetch", context),
  int_clamp: ns.int_clamp(15n, 0n, 10n).toString(),
  list_min: ns.list_min([3n, 1n, 2n]).toString(),
  list_map: ns.list_map([1n, 2n], (x) => x + 1n).map(String),
  exports: Object.keys(ns).length,
}));
"#;
    let dir = std::env::temp_dir().join(format!("morphic-phase152-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("sandbox.mjs"), harness).unwrap();
    let index = manifest_dir().join("packages/npm/index.js");
    let out = Command::new("node")
        .arg("--experimental-vm-modules")
        .arg(dir.join("sandbox.mjs"))
        .arg(&index)
        .output()
        .unwrap_or_else(|e| panic!("node must run (node v24 on PATH): {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "sandbox harness failed\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    let line = stdout.lines().last().expect("one JSON line").to_string();
    let v: serde_json::Value = serde_json::from_str(&line).expect("harness JSON parses");
    for global in ["process_in_ctx", "require_in_ctx", "buffer_in_ctx", "fetch_in_ctx"] {
        assert_eq!(v[global], "undefined", "{global} must be absent in the sandbox");
    }
    assert_eq!(v["int_clamp"], "10");
    assert_eq!(v["list_min"], "1");
    assert_eq!(v["list_map"], serde_json::json!(["2", "3"]));
    assert_eq!(v["exports"], 35);
    println!(
        "P152|edge|sandbox globals=none load=ok exports=35 int_clamp=10n list_min=1n \
         list_map=ho-callback-ok"
    );
}

/// EDGE-3: the binding generator already accepts USER
/// modules in-process, ungated — and (the PCC-1
/// discipline, re-pinned on this tree) it prints the
/// caller's stated tier/basis verbatim. So the
/// user-package verb's build is admission + assembly,
/// not generation; and its honesty rule is forced by
/// this fact: it may only ever feed the generator rows
/// it measured itself.
#[test]
fn p152_edge_generator_accepts_user_modules_ungated() {
    let module = parser::parse(PLUS_ONE).expect("probe module parses");
    let spec = module.functions.into_iter().next().expect("one spec");
    // The identity body does NOT satisfy `output == x + 1`;
    // the stated claim below is deliberately false, as in
    // P148 PCC-1 — the generator checks nothing.
    let input = BindingInput {
        spec,
        body: IRNode::Var("x".into()),
        tier: "tier-1".into(),
        verdict_basis: "z3-verified".into(),
        battery_pinned: 0,
        battery_silent: 0,
        crown_route: CrownRoute::Pipeline,
    };
    let set = bindings::generate_bindings(&[input], "probe-user", "0.0.0", "P152")
        .expect("bindings generate from a user module's inputs");
    assert_eq!(set.manifest.exports.len(), 1);
    assert_eq!(set.manifest.exports[0].name, "plus_one");
    assert!(set.js_module.contains("function plus_one"));
    assert_eq!(set.manifest.exports[0].verdict_basis, "z3-verified");
    println!(
        "P152|edge|generator ungated=true accepts_user_modules=true claims=caller-stated"
    );
}

/// EDGE-4: the shipped verb is stdlib-only today,
/// measured at the CLI surface — `build --package`
/// refuses an input file by name before any assembly
/// runs. The user-package verb does not exist yet in
/// any form a user can invoke.
#[test]
fn p152_edge_stdlib_verb_refuses_an_input_file() {
    let input = manifest_dir().join("examples/self_host/validator.morph");
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("build")
        .arg("--package")
        .arg(&input)
        .output()
        .expect("the binary runs");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.status.success(), "the stdlib verb must refuse an input file");
    assert!(
        text.contains("takes no input file"),
        "the refusal names itself: {text}"
    );
    println!("P152|edge|build_package input_file=refused-by-name stdlib_only=true");
}

/// EDGE-5: the admission inputs a user-package verb
/// would read are all MEASURED in-process today — a
/// seeded synthesis run reports the crowned body's
/// verdict and its raw executed counts, and a wrong
/// body measurably fails the executed battery. The
/// verb's admission rule (Verified, or executed-exact
/// with passed == evaluable == total) has something
/// real to read; nothing about it needs inventing.
#[test]
fn p152_edge_verb_admission_inputs_are_measured() {
    let report = api::synthesize_seeded_spec(VALIDATOR, "is_known_type_tag", 42)
        .expect("named-spec seeded run completes");
    let w = &report.winner;
    assert_eq!(w.tests_total, 5);
    assert_eq!(w.tests_passed, w.tests_evaluable);
    assert_eq!(w.tests_evaluable, w.tests_total);
    #[cfg(feature = "z3-support")]
    assert_eq!(w.verification_status, "Verified");
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(w.verification_status, "Inconclusive");

    // The refusal side, in every build: the identity
    // body against plus_one's own tests passes nothing.
    let module = parser::parse(PLUS_ONE).expect("probe module parses");
    let spec = module.functions.into_iter().next().expect("one spec");
    let run = eval::run_spec_tests(&IRNode::Var("x".into()), &spec);
    assert_eq!(run.evaluable, 2);
    assert_eq!(run.passed, 0);
    println!(
        "P152|edge|admission crown_status={} crown_counts={}/{}/{} \
         wrong_body_counts={}/{}",
        w.verification_status, w.tests_passed, w.tests_evaluable, w.tests_total,
        run.passed, run.evaluable
    );
}

// ── LSP ──────────────────────────────────────────────────

/// LSP-1: the `lsp` subcommand's current behavior,
/// re-measured on the shipped binary — it prints the
/// CLI banner and one "Starting" line to STDOUT (the
/// stream a real server would own), reads nothing, and
/// exits 0 immediately. It is a reserved name with a
/// banner, not a server in any degraded sense.
#[test]
fn p152_lsp_stub_prints_banner_and_exits() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary spawns");
    drop(child.stdin.take());
    let mut out_pipe = child.stdout.take().expect("stdout piped");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        buf
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(s) => break s,
            None => {
                assert!(
                    Instant::now() < deadline,
                    "morphic lsp did not exit within 15 s — it became a server?"
                );
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    };
    let stdout = String::from_utf8_lossy(&reader.join().expect("reader joins")).to_string();
    assert!(status.success(), "the stub exits 0: {status}");
    assert!(stdout.contains("Starting Morphic Language Server"));
    assert!(stdout.contains('╔'), "the CLI banner rides the same stdout");
    assert!(!stdout.contains("\"jsonrpc\""), "no protocol bytes are spoken");
    println!(
        "P152|lsp|stub stdout_bytes={} banner_on_stdout=true protocol=none exit=0-immediate",
        stdout.len()
    );
}

/// LSP-2: the transfer boundary between the MCP core
/// and an LSP server, measured at the handler — the
/// message-level dispatch IS reusable (a bare JSON-RPC
/// request line is answered), and exactly two things
/// do NOT transfer: Content-Length framing (the same
/// request, framed as LSP frames it, is a parse error
/// to the MCP core — no framing code exists anywhere
/// in the tree) and the notification rule (an id-less
/// request is never executed by MCP discipline; LSP's
/// didOpen/didChange ARE id-less and must execute).
#[test]
fn p152_lsp_framing_and_notification_boundary() {
    let mut server = McpServer::new();
    let bare = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
    let answered = server
        .handle_line(bare)
        .expect("a bare request line is answered at the message level");
    assert!(answered.contains("\"result\""));
    assert!(answered.contains("\"protocolVersion\""));

    let framed = format!("Content-Length: {}\r\n\r\n{}", bare.len(), bare);
    let refused = server
        .handle_line(&framed)
        .expect("framed bytes still get an answer (the server lives on)");
    assert!(
        refused.contains("-32700"),
        "no Content-Length framing exists to parse: {refused}"
    );

    let idless = r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"check","arguments":{"source":""}}}"#;
    assert!(
        server.handle_line(idless).is_none(),
        "MCP discipline: an id-less message is never executed"
    );
    println!(
        "P152|lsp|transfer message_dispatch=reusable content_length_framing=absent \
         idless_rule=mcp-never-executes (lsp must invert for didOpen/didChange)"
    );
}

/// LSP-3: the diagnostics surface, re-measured — a
/// check yields AT MOST ONE diagnostic (parse: 1-based
/// line + column; typecheck: no position, but the
/// error names its spec), a second broken spec in the
/// same file is never reached, and whole-source check
/// is fast enough on the corpus's largest module that
/// full-document sync needs no incremental machinery.
#[test]
fn p152_lsp_diagnostic_surface_remeasured() {
    let bad = api::check(INVALID);
    let e = bad.error.expect("parse error");
    assert_eq!(e.stage, "parse");
    assert_eq!(e.line, Some(1));
    assert!(e.column.is_some_and(|c| c >= 1));

    let terr = api::check(TYPE_ERR);
    let e = terr.error.expect("typecheck error");
    assert_eq!(e.stage, "typecheck");
    assert_eq!(e.line, None);
    assert_eq!(e.column, None);

    // The positionless error still names its spec —
    // TypeError.function is populated even though
    // TypeError.span never is.
    let parsed = parser::parse(TYPE_ERR).expect("the module parses");
    let err = match typeck::check(parsed) {
        Ok(_) => panic!("the module does not check"),
        Err(e) => e,
    };
    assert_eq!(err.function.as_deref(), Some("f"));
    assert!(err.span.is_none());

    // One diagnostic per check, by construction and in
    // fact: the second broken spec is never reported.
    let two = api::check(TWO_TYPE_ERRS);
    let e = two.error.expect("one typecheck error");
    assert!(e.message.contains("'f'"), "the first error is the report: {}", e.message);
    assert!(!e.message.contains("'g'"), "the second error is never reached");

    let t0 = Instant::now();
    let whole = api::check(BIG);
    let big_ms = t0.elapsed().as_millis();
    assert!(whole.ok);
    // The warm figure is the one a recheck-on-change
    // server lives on; the cold call above carries the
    // process's one-time initialization.
    let t0 = Instant::now();
    let whole2 = api::check(BIG);
    let big_warm_ms = t0.elapsed().as_millis();
    assert!(whole2.ok);
    let t0 = Instant::now();
    let small = api::check(VALIDATOR);
    let small_ms = t0.elapsed().as_millis();
    assert!(small.ok);
    assert!(big_ms < 5_000, "even a loaded debug build stays far inside sync viability");
    println!(
        "P152|lsp|diagnostics max_per_check=1 parse=line+column \
         typeck=no-position-names-spec largest_module_bytes={} largest_check_ms={} \
         largest_check_warm_ms={} small_check_ms={}",
        BIG.len(),
        big_ms,
        big_warm_ms,
        small_ms
    );
}

// ── VS Code ──────────────────────────────────────────────

/// VSC-1: the extension surface is greenfield, measured
/// — the tree's only package.json is the library's; no
/// packages/vscode, no TextMate grammar, no .vscode
/// directory exists. And the toolchain the client would
/// be built with (node, for vscode-languageclient +
/// vsce) is present in this sandbox. The client is a
/// new small package, not a refactor of anything.
#[test]
fn p152_vscode_surface_is_greenfield() {
    let root = manifest_dir();
    assert!(!root.join("packages/vscode").exists(), "no extension package yet");
    assert!(!root.join(".vscode").exists(), "no .vscode directory yet");
    for absent in [
        "package.json",
        "playground/package.json",
        "packages/vscode/package.json",
    ] {
        assert!(!root.join(absent).exists(), "{absent} must not exist yet");
    }
    assert!(root.join("packages/npm/package.json").exists());
    let mut grammars = 0usize;
    for entry in std::fs::read_dir(root.join("packages")).expect("packages reads") {
        let name = entry.expect("dir entry").file_name().to_string_lossy().to_string();
        if name.contains("tmLanguage") || name == "vscode" {
            grammars += 1;
        }
    }
    assert_eq!(grammars, 0, "no grammar or extension entries under packages/");
    let out = Command::new("node")
        .arg("--version")
        .output()
        .expect("node runs");
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(version.starts_with('v'), "node reports a version: {version}");
    println!(
        "P152|vscode|greenfield=true only_package_json=packages/npm node={version}"
    );
}
