//! Phase 149 — the MCP server (DISEBAR_SCOPE.md §3): a
//! hand-rolled stdio JSON-RPC shell over `src/api.rs`'s
//! DTOs, zero new dependencies, the seeded bounded
//! LLM-free profile as its only synthesis discipline.
//!
//! Pins: the protocol subset (initialize / ping /
//! tools/list / tools/call), the frozen four-tool
//! surface, verdict honesty at the boundary (a verdict
//! the server serves is the native verifier's own,
//! produced in-process at call time — cfg-branched
//! exactly as the Phase 148 probes branch), the decline
//! shapes (malformed input → protocol errors, never
//! panics; unknown tools/methods/entries decline by
//! name), the no-filesystem surface (tools take source
//! text as arguments — there is no path argument), and
//! an end-to-end stdio transcript against the shipped
//! binary.
//!
//! Server calls serialize on one lock: the synthesis
//! engine's RNG is process-global (the Phase 144 gate
//! binary's discipline), so parallel tool calls inside
//! one test binary could interleave it.

use morphic::mcp::McpServer;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

static LOCK: Mutex<()> = Mutex::new(());

const VALIDATOR: &str = include_str!("../examples/self_host/validator.morph");
const STDLIB_INT: &str = include_str!("../stdlib/int.morph");
const INVALID: &str = "spec broken { input: x: Int output: Int constraint: ";

fn request(server: &mut McpServer, id: u64, method: &str, params: Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
        .to_string();
    let response = server
        .handle_line(&line)
        .expect("a request with an id always gets a response");
    serde_json::from_str(&response).expect("the response is one JSON value")
}

/// One tools/call, returning (is_error, payload) where the
/// payload is the content text parsed back as JSON (every
/// tool speaks DTO JSON as its text content).
fn tool_call(server: &mut McpServer, id: u64, name: &str, arguments: Value) -> (bool, Value) {
    let response = request(
        server,
        id,
        "tools/call",
        json!({"name": name, "arguments": arguments}),
    );
    let result = response
        .get("result")
        .unwrap_or_else(|| panic!("tools/call answers with a result, got: {response}"));
    let is_error = result
        .get("isError")
        .and_then(Value::as_bool)
        .expect("the result carries isError");
    let text = result["content"][0]["text"]
        .as_str()
        .expect("the first content item is text");
    let payload: Value = serde_json::from_str(text)
        .unwrap_or_else(|_| panic!("tool text is DTO JSON, got: {text}"));
    (is_error, payload)
}

fn handshaken_server() -> McpServer {
    let mut server = McpServer::new();
    let response = request(
        &mut server,
        1,
        "initialize",
        json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "phase149-test", "version": "0"}}),
    );
    assert!(response.get("result").is_some(), "initialize answers: {response}");
    let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string();
    assert!(
        server.handle_line(&note).is_none(),
        "a notification gets no response"
    );
    server
}

// ── The protocol subset ─────────────────────────────────

#[test]
fn p149_protocol_initialize_and_tool_list() {
    let _guard = LOCK.lock().unwrap();
    let mut server = McpServer::new();
    let response = request(
        &mut server,
        1,
        "initialize",
        json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
    );
    let result = &response["result"];
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "morphic");
    assert!(
        result["serverInfo"]["version"].as_str().is_some_and(|v| !v.is_empty()),
        "the server names its build version"
    );
    assert!(result["capabilities"]["tools"].is_object());

    let list = request(&mut server, 2, "tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().expect("a tool array");
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        ["check", "synthesize", "verify", "library"],
        "the frozen tool surface, exactly four tools"
    );
    // The frozen argument sets (DISEBAR_SCOPE §3): source
    // text is the only way in — no path, no key, no LLM
    // argument exists on any tool.
    let schema = |name: &str| {
        tools
            .iter()
            .find(|t| t["name"] == name)
            .unwrap()["inputSchema"]
            .clone()
    };
    let props = |s: &Value| {
        let mut keys: Vec<String> = s["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    };
    assert_eq!(props(&schema("check")), ["source"]);
    assert_eq!(props(&schema("synthesize")), ["seed", "source", "spec"]);
    assert_eq!(props(&schema("verify")), ["body", "source", "spec"]);
    assert_eq!(props(&schema("library")), ["entry"]);
    for name in names {
        let text = schema(name).to_string();
        for banned in ["path", "file", "llm", "api_key", "apiKey", "model"] {
            assert!(
                !text.contains(&format!("\"{banned}\"")),
                "{name}'s schema must not carry a '{banned}' argument: {text}"
            );
        }
    }

    let ping = request(&mut server, 3, "ping", json!({}));
    assert!(ping["result"].is_object(), "ping answers an empty result");
    println!("P149|protocol|tools=4 subset=initialize,ping,tools/list,tools/call");
}

#[test]
fn p149_protocol_errors_decline_never_panic() {
    let _guard = LOCK.lock().unwrap();
    let mut server = handshaken_server();

    // Malformed JSON → -32700 with a null id, and the
    // server survives to answer the next request.
    let response = server
        .handle_line("{not json")
        .expect("a parse error is answered, not crashed on");
    let response: Value = serde_json::from_str(&response).unwrap();
    assert_eq!(response["error"]["code"], -32700);
    assert!(response["id"].is_null());

    // A batch array is not the stdio shape → -32600.
    let response = server.handle_line("[]").expect("answered");
    let response: Value = serde_json::from_str(&response).unwrap();
    assert_eq!(response["error"]["code"], -32600);

    // Unknown method → -32601 naming it.
    let response = request(&mut server, 9, "resources/list", json!({}));
    assert_eq!(response["error"]["code"], -32601);
    assert!(
        response["error"]["message"].as_str().unwrap().contains("resources/list"),
        "the decline names the method: {response}"
    );

    // Unknown tool → a protocol error naming the tool and
    // the frozen surface.
    let response = request(
        &mut server,
        10,
        "tools/call",
        json!({"name": "execute", "arguments": {}}),
    );
    assert_eq!(response["error"]["code"], -32602);
    let message = response["error"]["message"].as_str().unwrap();
    assert!(message.contains("execute"), "names the tool: {message}");
    assert!(message.contains("synthesize"), "names the surface: {message}");

    // Empty lines are skipped silently; the server is
    // still alive after all of the above.
    assert!(server.handle_line("").is_none());
    assert!(server.handle_line("   ").is_none());
    let ping = request(&mut server, 11, "ping", json!({}));
    assert!(ping["result"].is_object());
    println!("P149|protocol|declines=parse:-32700 batch:-32600 method:-32601 tool:-32602 server=alive");
}

// ── check ───────────────────────────────────────────────

#[test]
fn p149_check_tool_reports_and_argument_discipline() {
    let _guard = LOCK.lock().unwrap();
    let mut server = handshaken_server();

    let (is_error, payload) = tool_call(&mut server, 2, "check", json!({"source": VALIDATOR}));
    assert!(!is_error);
    assert_eq!(payload["ok"], true);
    assert!(!payload["specs"].as_array().unwrap().is_empty());

    // A source that fails is a successful tool call
    // carrying the failing report — the report is the
    // tool's product, not a protocol failure.
    let (is_error, payload) = tool_call(&mut server, 3, "check", json!({"source": INVALID}));
    assert!(!is_error);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["stage"], "parse");

    // Missing source → a tool error naming the argument.
    let (is_error, payload) = tool_call(&mut server, 4, "check", json!({}));
    assert!(is_error);
    assert!(
        payload["message"].as_str().unwrap().contains("'source'"),
        "names the missing argument: {payload}"
    );

    // There is no filesystem surface: a caller offering a
    // path instead of source text gets the same decline —
    // the server never reads files on a caller's behalf.
    let (is_error, payload) = tool_call(
        &mut server,
        5,
        "check",
        json!({"path": "examples/self_host/validator.morph"}),
    );
    assert!(is_error);
    assert!(payload["message"].as_str().unwrap().contains("'source'"));
    println!("P149|check|ok_report=true failed_check=is_a_report no_path_argument=true");
}

// ── synthesize ──────────────────────────────────────────

#[test]
fn p149_synthesize_tool_crowns_and_is_deterministic() {
    let _guard = LOCK.lock().unwrap();
    let mut server = handshaken_server();
    let args = json!({"source": VALIDATOR, "spec": "is_known_type_tag", "seed": 42});

    let t0 = Instant::now();
    let (is_error, payload) = tool_call(&mut server, 2, "synthesize", args.clone());
    let ms = t0.elapsed().as_millis();
    assert!(!is_error, "the run completes: {payload}");
    assert_eq!(payload["spec_name"], "is_known_type_tag");
    assert_eq!(
        payload["verification_available"],
        cfg!(feature = "z3-support"),
        "verification availability is the build's own fact"
    );
    // The verdict served is the native pipeline's own,
    // produced at call time (the Phase 148 measurement,
    // re-pinned through the server): Verified in the Z3
    // build, the honest Inconclusive label without it.
    #[cfg(feature = "z3-support")]
    assert_eq!(payload["winner"]["verification_status"], "Verified");
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(payload["winner"]["verification_status"], "Inconclusive");
    assert!(payload["winner"]["body_debug"].as_str().is_some_and(|b| !b.is_empty()));

    // Same (source, seed) → same crowned body, through
    // the server (D3a's reproducibility discipline).
    let (is_error2, payload2) = tool_call(&mut server, 3, "synthesize", args);
    assert!(!is_error2);
    assert_eq!(
        payload["winner"]["body_debug"], payload2["winner"]["body_debug"],
        "same (source, seed) crowns the same body through the server"
    );
    println!(
        "P149|synthesize|status={} verification_available={} elapsed_ms={}",
        payload["winner"]["verification_status"], payload["verification_available"], ms
    );
}

#[test]
fn p149_synthesize_profile_llm_free_bounded_and_capped() {
    let _guard = LOCK.lock().unwrap();
    // The profile the server runs is the playground
    // profile: LLM-free by construction (D3a). No key
    // exists in this profile for a call to reach.
    assert!(
        !morphic::api::playground_config().use_llm,
        "the server's synthesis profile never uses an LLM"
    );

    let mut server = handshaken_server();
    // The spec-size cap (DISEBAR_SCOPE §3): an over-cap
    // source declines by name before any stage runs.
    let over_cap = "x".repeat(morphic::mcp::MAX_SOURCE_BYTES + 1);
    let (is_error, payload) = tool_call(&mut server, 2, "synthesize", json!({"source": over_cap}));
    assert!(is_error);
    let message = payload["message"].as_str().unwrap();
    assert!(
        message.contains("262144") && message.contains("cap"),
        "the decline names the cap: {message}"
    );

    // An unparseable source is a tool error carrying the
    // stage, not a dressed success.
    let (is_error, payload) = tool_call(&mut server, 3, "synthesize", json!({"source": INVALID}));
    assert!(is_error);
    assert_eq!(payload["stage"], "parse");
    println!("P149|synthesize|llm=false cap_bytes={}", morphic::mcp::MAX_SOURCE_BYTES);
}

// ── verify ──────────────────────────────────────────────

#[test]
#[cfg(not(feature = "wasm-playground"))]
fn p149_verify_tool_judges_library_and_denoted_bodies() {
    let _guard = LOCK.lock().unwrap();
    let mut server = handshaken_server();

    // The shipped body of int_abs against its own spec:
    // the verdict is verify_one's own, produced now.
    let (is_error, payload) = tool_call(
        &mut server,
        2,
        "verify",
        json!({"source": STDLIB_INT, "spec": "int_abs", "body": {"kind": "library", "entry": "int_abs"}}),
    );
    assert!(!is_error, "the judgment runs: {payload}");
    assert_eq!(payload["body_source"], "library:int_abs");
    assert_eq!(payload["verification_available"], cfg!(feature = "z3-support"));
    #[cfg(feature = "z3-support")]
    {
        assert_eq!(payload["status"], "Verified");
        assert!(
            payload["rows"].as_array().unwrap().iter().all(|r| r["holds"] == true),
            "every row Proven for the shipped body"
        );
    }
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(payload["status"], "Inconclusive");

    // Teeth: a WRONG shipped body (int_sign's, against
    // int_abs's spec — same signature, wrong function)
    // is refuted by the same route, never dressed.
    #[cfg(feature = "z3-support")]
    {
        let (is_error, payload) = tool_call(
            &mut server,
            3,
            "verify",
            json!({"source": STDLIB_INT, "spec": "int_abs", "body": {"kind": "library", "entry": "int_sign"}}),
        );
        assert!(!is_error);
        assert_eq!(payload["status"], "VerificationFailed");
    }

    // The denoted source: a spec whose equation row
    // denotes its body through the standing resolver.
    let denoted = "McpDenoted {\n\
    spec inc {\n\
        input: x: Int\n\
        output: Int\n\
        constraint: output == x + 1\n\
        test { input: 1 expect: 2 }\n\
    }\n\
    }";
    let (is_error, payload) = tool_call(
        &mut server,
        4,
        "verify",
        json!({"source": denoted, "spec": "inc", "body": {"kind": "denoted"}}),
    );
    assert!(!is_error, "the denoted judgment runs: {payload}");
    assert_eq!(payload["body_source"], "denoted");
    #[cfg(feature = "z3-support")]
    assert_eq!(payload["status"], "Verified");

    // Declines, each by name: an unknown library entry;
    // a spec whose rows denote no body; an unknown body
    // kind.
    let (is_error, payload) = tool_call(
        &mut server,
        5,
        "verify",
        json!({"source": STDLIB_INT, "spec": "int_abs", "body": {"kind": "library", "entry": "int_nope"}}),
    );
    assert!(is_error);
    assert!(payload["message"].as_str().unwrap().contains("int_nope"));

    // An inequality-only spec denotes no body through
    // the resolver (no equation row, no guard table).
    let no_denotation = "McpPlain {\n\
    spec gt {\n\
        input: x: Int\n\
        output: Bool\n\
        constraint: output != (x > 0)\n\
        test { input: 1 expect: true }\n\
    }\n\
    }";
    let (is_error, payload) = tool_call(
        &mut server,
        6,
        "verify",
        json!({"source": no_denotation, "spec": "gt", "body": {"kind": "denoted"}}),
    );
    assert!(is_error, "an undenoting spec declines: {payload}");
    assert!(payload["message"].as_str().unwrap().contains("denotes no body"));

    let (is_error, payload) = tool_call(
        &mut server,
        7,
        "verify",
        json!({"source": STDLIB_INT, "spec": "int_abs", "body": {"kind": "uploaded"}}),
    );
    assert!(is_error);
    assert!(payload["message"].as_str().unwrap().contains("uploaded"));
    println!("P149|verify|library_body=judged denoted=judged declines=named");
}

// ── library ─────────────────────────────────────────────

#[test]
#[cfg(not(feature = "wasm-playground"))]
fn p149_library_tool_lists_and_fetches_the_shipped_data() {
    let _guard = LOCK.lock().unwrap();
    let mut server = handshaken_server();

    let (is_error, payload) = tool_call(&mut server, 2, "library", json!({}));
    assert!(!is_error);
    let exports = payload["exports"].as_array().expect("the export rows");
    assert_eq!(exports.len(), 35, "the shipped library, whole");
    let tier1 = exports.iter().filter(|e| e["tier"] == "tier-1").count();
    assert_eq!(tier1, 33);
    // The rows are the manifest's own data (EMBED_SCOPE
    // §2.4) — the same rows the shipped package prints.
    let abs = exports.iter().find(|e| e["name"] == "int_abs").unwrap();
    assert_eq!(abs["verdictBasis"], "z3-verified");
    assert_eq!(abs["battery"]["pinned"], 16);
    assert_eq!(abs["battery"]["specSilent"], 1);

    // Fetch: the entry's manifest row + its spec (the
    // AST's own serialization) + its shipped body in the
    // pipeline's identity form.
    let (is_error, payload) = tool_call(&mut server, 3, "library", json!({"entry": "int_abs"}));
    assert!(!is_error, "the fetch answers: {payload}");
    assert_eq!(payload["name"], "int_abs");
    assert_eq!(payload["tier"], "tier-1");
    assert_eq!(payload["spec"]["name"], "int_abs");
    assert!(
        payload["body_debug"].as_str().is_some_and(|b| !b.is_empty()),
        "the shipped body rides along in Debug form"
    );

    // An unknown entry declines by name.
    let (is_error, payload) = tool_call(&mut server, 4, "library", json!({"entry": "list_nope"}));
    assert!(is_error);
    assert!(payload["message"].as_str().unwrap().contains("list_nope"));
    println!("P149|library|entries=35 tier1=33 fetch=int_abs+spec+body");
}

// ── The end-to-end transcript ───────────────────────────

/// Spawn the shipped binary, speak the protocol over its
/// stdio, and read the whole transcript: the synthesize
/// call returns a Verified crown for the validator
/// corpus spec (the audit's ~0.2 s measurement, through
/// the process boundary), a garbage line is answered
/// with a protocol error in order, and EOF closes the
/// server cleanly. Z3-gated: without the verifier the
/// binary refuses the subcommand by name (DISEBAR_SCOPE
/// §3 R3) — that refusal is the no-Z3 build's honesty,
/// not a transcript.
#[test]
#[cfg(feature = "z3-support")]
fn p149_stdio_transcript_end_to_end() {
    let _guard = LOCK.lock().unwrap();
    let mut child: Child = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the binary spawns");

    let (tx, rx) = mpsc::channel::<Vec<String>>();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let reader = std::thread::spawn(move || {
        let mut lines = Vec::new();
        let mut line = String::new();
        loop {
            line.clear();
            match stdout.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => lines.push(line.trim().to_string()),
            }
        }
        let _ = tx.send(lines);
    });

    let script = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "phase149-transcript", "version": "0"}}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "synthesize", "arguments": {"source": VALIDATOR, "spec": "is_known_type_tag", "seed": 42}}}).to_string(),
        "{garbage".to_string(),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "check", "arguments": {"source": INVALID}}}).to_string(),
    ];
    let t0 = Instant::now();
    {
        let mut stdin = child.stdin.take().unwrap();
        for line in &script {
            writeln!(stdin, "{line}").expect("the script writes");
        }
        // stdin drops here: EOF, the server's exit cue.
    }
    let lines = rx
        .recv_timeout(Duration::from_secs(180))
        .expect("the server answers and exits on EOF");
    let _ = reader.join();
    let status = child.wait().expect("the child reaps");
    let ms = t0.elapsed().as_millis();
    assert!(status.success(), "EOF closes the server cleanly: {status}");

    // Five responses in order (the notification gets
    // none): initialize, tools/list, synthesize, the
    // garbage line's parse error, check. Every line is
    // JSON — the CLI banner never touches this stream.
    assert_eq!(lines.len(), 5, "one response per request: {lines:?}");
    let parsed: Vec<Value> = lines
        .iter()
        .map(|l| serde_json::from_str(l).expect("every stdout line is JSON-RPC"))
        .collect();
    assert_eq!(parsed[0]["result"]["serverInfo"]["name"], "morphic");
    assert_eq!(parsed[1]["result"]["tools"].as_array().unwrap().len(), 4);

    let synth_text = parsed[2]["result"]["content"][0]["text"].as_str().unwrap();
    let synth: Value = serde_json::from_str(synth_text).unwrap();
    assert_eq!(synth["winner"]["verification_status"], "Verified");
    assert_eq!(parsed[2]["result"]["isError"], false);

    assert_eq!(parsed[3]["error"]["code"], -32700);

    let check_text = parsed[4]["result"]["content"][0]["text"].as_str().unwrap();
    let check: Value = serde_json::from_str(check_text).unwrap();
    assert_eq!(check["ok"], false);
    assert_eq!(check["error"]["stage"], "parse");

    // The transcript's total wall time, for the record
    // (the audit measured the bare API call at ~0.2 s;
    // the process boundary and startup ride on top).
    println!("P149|transcript|responses=5 synthesize=Verified wall_ms={ms}");
}
