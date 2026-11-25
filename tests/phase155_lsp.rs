//! Phase 155 — the LSP diagnostics-only server
//! (DISEBAR_15_SCOPE.md §5): `morphic lsp` as the real
//! server — Content-Length framed JSON-RPC 2.0 over
//! stdio, full-document sync, push diagnostics from
//! api::check.
//!
//! Pins: the frozen surface (initialize / shutdown /
//! exit + didOpen / didChange / didClose), the
//! capabilities answer (full-document sync, diagnostics
//! only — no other capability is claimed), the
//! diagnostics mapping (parse errors at their true
//! positions, 1-based → 0-based; typecheck errors as
//! ONE file-level diagnostic at (0,0)–(0,0) whose
//! message is the verbatim typecheck message under the
//! frozen `unpositioned: ` prefix, R3; a clean file
//! publishes an empty set), the lifecycle exit codes
//! (0 after shutdown, 1 without), the malformed-frame
//! discipline (answered −32700 where recoverable, the
//! server alive for the next well-formed message), and
//! — over the shipped binary's stdio, with real
//! Content-Length framing — that no banner byte ever
//! touches the stream: the first bytes out are a
//! Content-Length header. This transcript is the
//! instrument Phase 156's VS Code smoke reuses.

use morphic::api;
use morphic::lsp::LspServer;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const CLEAN: &str = "Clean {\n    spec plus_one {\n        input: x: Int\n        output: Int\n        constraint: output == x + 1\n        test \"one\" { input: 0 expect: 1 }\n    }\n}";
// Measured through api::check (the Phase 152 surface):
// a parse error at line 6, column 5 (1-based) — the
// closing brace where an expression was expected.
const PARSE_BROKEN: &str =
    "ParseBroken {\n    spec f {\n        input: a: Int\n        output: Int\n        constraint: output == a +\n    }\n}";
// Parses; the typechecker refuses `++` on Ints. Its
// message names spec 'f' and carries no position.
const TYPE_ERR: &str =
    "Typed {\n    spec f {\n        input: a: Int\n        output: Int\n        constraint: output == a ++ 2\n    }\n}";

const CLEAN_URI: &str = "file:///clean.morph";
const BROKEN_URI: &str = "file:///broken.morph";

// ── In-process helpers ──────────────────────────────────

fn send(server: &mut LspServer, message: Value) -> Vec<Value> {
    server
        .handle_message(&message.to_string())
        .iter()
        .map(|s| serde_json::from_str(s).expect("every output is one JSON value"))
        .collect()
}

fn request(server: &mut LspServer, id: u64, method: &str, params: Value) -> Vec<Value> {
    send(server, json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
}

fn notify(server: &mut LspServer, method: &str, params: Value) -> Vec<Value> {
    send(server, json!({"jsonrpc": "2.0", "method": method, "params": params}))
}

fn did_open(server: &mut LspServer, uri: &str, text: &str) -> Vec<Value> {
    notify(
        server,
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "morphic", "version": 1, "text": text}}),
    )
}

fn did_change(server: &mut LspServer, uri: &str, text: &str) -> Vec<Value> {
    notify(
        server,
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri, "version": 2}, "contentChanges": [{"text": text}]}),
    )
}

fn the_diagnostics(publish: &Value) -> &Vec<Value> {
    assert_eq!(publish["method"], "textDocument/publishDiagnostics");
    publish["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
}

// ── The surface and the diagnostics mapping ─────────────

#[test]
fn p155_initialize_answers_full_sync_and_nothing_more() {
    let mut server = LspServer::new();
    let out = request(
        &mut server,
        1,
        "initialize",
        json!({"processId": null, "rootUri": null, "capabilities": {}}),
    );
    assert_eq!(out.len(), 1, "initialize answers once: {out:?}");
    let result = &out[0]["result"];
    assert_eq!(result["serverInfo"]["name"], "morphic");
    assert!(
        result["serverInfo"]["version"].as_str().is_some_and(|v| !v.is_empty()),
        "the server names its build version"
    );
    // Full-document sync (D5a): open/close tracking and
    // change kind 1 (Full). Diagnostics-only (§5.4):
    // the capabilities object claims exactly this and
    // nothing else — no hover, completion, definition,
    // formatting or code-action provider exists to claim.
    let capabilities = result["capabilities"].as_object().expect("capabilities is an object");
    assert_eq!(
        capabilities.len(),
        1,
        "diagnostics-only claims one capability group: {capabilities:?}"
    );
    assert_eq!(capabilities["textDocumentSync"]["openClose"], true);
    assert_eq!(capabilities["textDocumentSync"]["change"], 1);
    println!("P155|initialize|sync=full(openClose,change=1) capabilities=textDocumentSync-only");
}

#[test]
fn p155_parse_diagnostic_at_its_true_position() {
    // The API's own facts, 1-based (Phase 152's surface).
    let report = api::check(PARSE_BROKEN);
    let error = report.error.expect("the fixture fails to parse");
    assert_eq!(error.stage, "parse");
    assert_eq!((error.line, error.column), (Some(6), Some(5)));

    let mut server = LspServer::new();
    let out = did_open(&mut server, BROKEN_URI, PARSE_BROKEN);
    assert_eq!(out.len(), 1, "didOpen publishes once: {out:?}");
    assert_eq!(out[0]["params"]["uri"], BROKEN_URI);
    let diagnostics = the_diagnostics(&out[0]);
    assert_eq!(diagnostics.len(), 1, "at most one diagnostic per check: {diagnostics:?}");
    let d = &diagnostics[0];
    // 1-based (6,5) → 0-based (5,4), exactly.
    assert_eq!(d["range"]["start"], json!({"line": 5, "character": 4}));
    assert_eq!(d["range"]["end"], json!({"line": 5, "character": 4}));
    assert_eq!(d["severity"], 1);
    assert_eq!(d["source"], "morphic");
    assert_eq!(d["message"], error.message, "the parse message rides verbatim");
    println!("P155|parse-diagnostic|api=(6,5)1-based lsp=(5,4)0-based severity=Error source=morphic");
}

#[test]
fn p155_typecheck_diagnostic_unpositioned_at_zero() {
    let report = api::check(TYPE_ERR);
    let error = report.error.expect("the fixture fails to typecheck");
    assert_eq!(error.stage, "typecheck");
    assert_eq!((error.line, error.column), (None, None));
    assert!(error.message.contains("'f'"), "the message names its spec: {}", error.message);

    let mut server = LspServer::new();
    let out = did_open(&mut server, BROKEN_URI, TYPE_ERR);
    assert_eq!(out.len(), 1);
    let diagnostics = the_diagnostics(&out[0]);
    assert_eq!(diagnostics.len(), 1, "one file-level diagnostic: {diagnostics:?}");
    let d = &diagnostics[0];
    assert_eq!(d["range"]["start"], json!({"line": 0, "character": 0}));
    assert_eq!(d["range"]["end"], json!({"line": 0, "character": 0}));
    assert_eq!(d["severity"], 1);
    assert_eq!(d["source"], "morphic");
    // The frozen form (R3): the label prefix, then the
    // typecheck message verbatim — spec name and all.
    assert_eq!(d["message"], format!("unpositioned: {}", error.message));
    println!("P155|typecheck-diagnostic|range=(0,0)-(0,0) message='unpositioned: <verbatim>'");
}

#[test]
fn p155_document_lifecycle_and_protocol_discipline() {
    let mut server = LspServer::new();
    request(&mut server, 1, "initialize", json!({"capabilities": {}}));
    assert!(
        notify(&mut server, "initialized", json!({})).is_empty(),
        "initialized is never answered"
    );

    // A clean file publishes an empty set.
    let out = did_open(&mut server, CLEAN_URI, CLEAN);
    assert_eq!(out.len(), 1);
    assert!(the_diagnostics(&out[0]).is_empty(), "clean → empty set");

    // Change to a type-broken text: the 0:0 diagnostic.
    let out = did_change(&mut server, CLEAN_URI, TYPE_ERR);
    let diagnostics = the_diagnostics(&out[0]).clone();
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]["message"].as_str().unwrap().starts_with("unpositioned: "),
        "the change re-checks and re-publishes: {diagnostics:?}"
    );

    // Change back to clean: the empty set again.
    let out = did_change(&mut server, CLEAN_URI, CLEAN);
    assert!(the_diagnostics(&out[0]).is_empty(), "fixed → empty set");

    // Close clears the file's diagnostics.
    let out = notify(
        &mut server,
        "textDocument/didClose",
        json!({"textDocument": {"uri": CLEAN_URI}}),
    );
    assert_eq!(out.len(), 1, "didClose publishes the clear");
    assert_eq!(out[0]["params"]["uri"], CLEAN_URI);
    assert!(the_diagnostics(&out[0]).is_empty());

    // Unknown requests decline −32601 naming the method;
    // unknown notifications are ignored, per protocol.
    let out = request(&mut server, 9, "textDocument/hover", json!({}));
    assert_eq!(out[0]["error"]["code"], -32601);
    assert!(
        out[0]["error"]["message"].as_str().unwrap().contains("textDocument/hover"),
        "the refusal names the method: {out:?}"
    );
    assert!(
        notify(&mut server, "workspace/didChangeConfiguration", json!({})).is_empty(),
        "an unknown notification gets no answer"
    );

    // Shutdown answers null; exit then ends the session
    // with the lifecycle's success code.
    let out = request(&mut server, 10, "shutdown", json!({}));
    assert_eq!(out[0]["result"], Value::Null);
    assert!(notify(&mut server, "exit", json!({})).is_empty(), "exit is never answered");
    assert!(server.should_exit());
    assert_eq!(server.exit_code(), 0, "shutdown preceded exit → 0");
    println!("P155|lifecycle|clean→empty type→0:0 fixed→empty close→clear unknown-request=-32601 exit-after-shutdown=0");
}

#[test]
fn p155_malformed_input_answered_and_server_lives() {
    let mut server = LspServer::new();
    let out = server.handle_message("{garbage");
    assert_eq!(out.len(), 1);
    let parsed: Value = serde_json::from_str(&out[0]).unwrap();
    assert_eq!(parsed["error"]["code"], -32700, "garbage JSON → parse error: {parsed}");

    let out = server.handle_message("[1,2]");
    let parsed: Value = serde_json::from_str(&out[0]).unwrap();
    assert_eq!(parsed["error"]["code"], -32600, "a batch is not a message: {parsed}");

    // The server lives on: the next well-formed message
    // is answered as if nothing happened.
    let out = request(&mut server, 1, "initialize", json!({"capabilities": {}}));
    assert_eq!(out[0]["result"]["serverInfo"]["name"], "morphic");
    assert_eq!(server.exit_code(), 1, "no shutdown yet → the lifecycle code is 1");
    println!("P155|malformed|garbage=-32700 batch=-32600 server-alive exit-without-shutdown=1");
}

// ── The stdio transcript (the Phase 156 instrument) ─────

fn frame(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
}

fn parse_frames(bytes: &[u8]) -> Vec<Value> {
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

/// Spawn the shipped binary on `lsp`, write the script,
/// close stdin (EOF), and collect (raw stdout, status).
fn run_transcript(script: Vec<Vec<u8>>) -> (Vec<u8>, std::process::ExitStatus) {
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
        // stdin drops here: EOF follows the script.
    }
    let bytes = rx
        .recv_timeout(Duration::from_secs(180))
        .expect("the server answers and the stream closes");
    let _ = reader.join();
    let status = child.wait().expect("the child reaps");
    (bytes, status)
}

#[test]
fn p155_stdio_transcript_end_to_end() {
    let script: Vec<Vec<u8>> = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"capabilities": {}}})
            .to_string(),
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": CLEAN_URI, "languageId": "morphic", "version": 1, "text": CLEAN}}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": BROKEN_URI, "languageId": "morphic", "version": 1, "text": PARSE_BROKEN}}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"textDocument": {"uri": BROKEN_URI, "version": 2}, "contentChanges": [{"text": TYPE_ERR}]}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"textDocument": {"uri": BROKEN_URI, "version": 3}, "contentChanges": [{"text": CLEAN}]}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": BROKEN_URI}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": {}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "exit"}).to_string(),
    ]
    .iter()
    .map(|body| frame(body))
    .collect();

    let (bytes, status) = run_transcript(script);
    assert!(status.success(), "shutdown then exit → status 0: {status}");
    // No banner byte ever touches the stream: the first
    // bytes out are a Content-Length header.
    assert!(
        bytes.starts_with(b"Content-Length: "),
        "the stream opens with framing, not the banner: {:?}",
        String::from_utf8_lossy(&bytes[..bytes.len().min(64)])
    );

    let frames = parse_frames(&bytes);
    assert_eq!(frames.len(), 7, "one answer per step of the script: {frames:?}");

    // initialize: full-document sync claimed.
    assert_eq!(frames[0]["id"], 1);
    assert_eq!(frames[0]["result"]["capabilities"]["textDocumentSync"]["change"], 1);
    assert_eq!(frames[0]["result"]["capabilities"]["textDocumentSync"]["openClose"], true);

    // didOpen clean → empty set.
    assert_eq!(frames[1]["params"]["uri"], CLEAN_URI);
    assert_eq!(frames[1]["params"]["diagnostics"], json!([]));

    // didOpen parse-broken → one diagnostic at the true
    // position: the API's (6,5) 1-based as (5,4) 0-based.
    let parse_error = api::check(PARSE_BROKEN).error.unwrap();
    let diagnostics = frames[2]["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["range"]["start"], json!({"line": 5, "character": 4}));
    assert_eq!(diagnostics[0]["range"]["end"], json!({"line": 5, "character": 4}));
    assert_eq!(diagnostics[0]["message"], parse_error.message);

    // didChange to type-broken → the 0:0 diagnostic
    // carrying the unpositioned name, the spec named in
    // the verbatim message that follows it.
    let type_error = api::check(TYPE_ERR).error.unwrap();
    let diagnostics = frames[3]["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["range"]["start"], json!({"line": 0, "character": 0}));
    assert_eq!(diagnostics[0]["range"]["end"], json!({"line": 0, "character": 0}));
    assert_eq!(
        diagnostics[0]["message"],
        format!("unpositioned: {}", type_error.message)
    );
    assert!(diagnostics[0]["message"].as_str().unwrap().contains("'f'"));

    // didChange back to clean → empty set; didClose →
    // the clear; shutdown → null.
    assert_eq!(frames[4]["params"]["diagnostics"], json!([]));
    assert_eq!(frames[5]["params"]["uri"], BROKEN_URI);
    assert_eq!(frames[5]["params"]["diagnostics"], json!([]));
    assert_eq!(frames[6]["id"], 2);
    assert_eq!(frames[6]["result"], Value::Null);
    println!("P155|transcript|frames=7 init→clean→parse@(5,4)→type@0:0-unpositioned→clean→close→shutdown exit=0 banner=absent");
}

#[test]
fn p155_stdio_exit_without_shutdown_exits_1() {
    let script = vec![frame(
        &json!({"jsonrpc": "2.0", "method": "exit"}).to_string(),
    )];
    let (bytes, status) = run_transcript(script);
    assert_eq!(status.code(), Some(1), "exit without shutdown → 1: {status}");
    assert!(bytes.is_empty(), "exit is never answered: {bytes:?}");
    println!("P155|exit-without-shutdown|status=1 output=none");
}

#[test]
fn p155_stdio_malformed_frames_survived() {
    let mut script: Vec<Vec<u8>> = Vec::new();
    // A well-framed body that is not JSON.
    script.push(frame("{garbage"));
    // A header block whose Content-Length is unusable,
    // with no body: reading resumes at the next block.
    script.push(b"Content-Length: nope\r\n\r\n".to_vec());
    script.push(frame(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"capabilities": {}}})
            .to_string(),
    ));
    script.push(frame(
        &json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": {}}).to_string(),
    ));
    script.push(frame(&json!({"jsonrpc": "2.0", "method": "exit"}).to_string()));

    let (bytes, status) = run_transcript(script);
    assert!(status.success(), "the session still closes cleanly: {status}");
    let frames = parse_frames(&bytes);
    assert_eq!(frames.len(), 4, "two refusals, then the session: {frames:?}");
    assert_eq!(frames[0]["error"]["code"], -32700, "garbage JSON body refused");
    assert_eq!(frames[1]["error"]["code"], -32700, "unusable Content-Length refused");
    assert_eq!(frames[2]["result"]["serverInfo"]["name"], "morphic");
    assert_eq!(frames[3]["result"], Value::Null);
    println!("P155|malformed-frames|garbage-json=-32700 bad-length=-32700 then initialize+shutdown answered exit=0");
}
