//! Phase 151 — the 1.4 gate run's own instrument: the
//! MCP channel, verdict honesty at the boundary, the
//! PCC evidence, and the footprint, re-run fresh on the
//! gate tree, clause by clause (DISEBAR_SCOPE.md §7).
//! Nothing here quotes Phase 149/150's evidence: every
//! pin below executes on this tree.
//!
//! What this file pins, by gate clause (§7):
//!
//! - **Clause 1 — the channel works.** A scripted
//!   JSON-RPC client speaks to the shipped binary over
//!   stdio: initialize → tools/list → check →
//!   synthesize (crowns Verified, deterministic across
//!   calls) → verify (a library body, then the ir-text
//!   body source) → library. The tools' answers agree
//!   with the library API's own outputs on the pinned
//!   corpus: check byte-for-byte (the CheckReport is
//!   fully deterministic), synthesize and verify on
//!   their deterministic projections (the reports carry
//!   wall-clock fields the freeze's "where the API is
//!   deterministic" carves out), library row-for-row
//!   against the generated manifest the tool reads.
//! - **Clause 2 — verdict honesty at the boundary.** A
//!   wrong body stands VerificationFailed through the
//!   tool — including when the caller helpfully states
//!   the verdict it wants as an extra argument: no
//!   tool argument states a verdict, and an offered one
//!   changes nothing. `verification_available` is the
//!   build's own fact on both verdict-bearing tools;
//!   in a no-Z3 build the MCP subcommand refuses by
//!   name at the CLI (R3's posture), pinned against
//!   that build's binary.
//! - **Clause 3 — the evidence re-checks.** As a
//!   consumer would: `verify --library`'s library
//!   function stands 35/35; every checked-in bundle in
//!   `packages/bundles/` re-verifies from its file
//!   bytes; the `verify-bundle` CLI verb exits 0 on
//!   honest bundles of both crown routes and the
//!   Tier-2 entry, and refuses a tampered copy by name
//!   with a non-zero exit; the 8-shape tamper battery
//!   refuses with pairwise-distinct named refusals;
//!   both packages' READMEs and EMBEDDING.md point at
//!   the bundles as the rows' evidence.
//! - **Clause 4 — footprint.** The compiler's
//!   [dependencies] set is the standing set (the MCP
//!   server and PCC added no runtime dependency — the
//!   lock graph's zero diff against v1.3.0 is the gate
//!   record's shell evidence beside this pin); the
//!   bundles stand at their recorded mass (35 files,
//!   267,156 B, a sibling artifact in neither package's
//!   file set); the npm package stands at 71,873 B
//!   unpacked inside the frozen 76,800 B budget. (The
//!   playground bundle is rebuilt and measured by the
//!   standing build at the gate — the gate record's
//!   evidence, not a pin: this binary cannot rebuild
//!   WASM.)
//! - **Clause 5** is the suites themselves, run solo
//!   in the three frozen configurations; this file is
//!   part of them.
//!
//! Crown-bearing tests serialize on one lock with the
//! guard recovered from poisoning (the Phase 144/150
//! discipline: heavy crowns flip under parallel load
//! on this box; no assertion is weakened, and the
//! gate's evidence rests on solo runs regardless).
//!
//! Configuration shape: the transcript and the PCC
//! re-checks need the native prelude + Z3, so they are
//! `not(wasm-playground)` + `z3-support` gated (under
//! CI's feature-unified build the prelude is the
//! uninstalled runtime asset — the Phase 144 lesson);
//! the honesty test is prelude-shaped the same way;
//! the no-Z3 refusal pin exists exactly in the builds
//! it describes; the footprint pins are
//! configuration-independent.

use morphic::api::BodySource;
use morphic::codegen::bindings;
use morphic::codegen::package;
use morphic::mcp::McpServer;
use morphic::pcc;
use morphic::synthesis::engine::{IRBinOp, IRLiteral, IRNode};
use morphic::synthesis::irtext::write_ir;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

static LOCK: Mutex<()> = Mutex::new(());

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

const VALIDATOR: &str = include_str!("../examples/self_host/validator.morph");
const STDLIB_INT: &str = include_str!("../stdlib/int.morph");

const INC_MODULE: &str = r#"Inc {
spec inc {
    input: x: Int
    output: Int

    constraint: output == x + 1

    test {
        input: 1
        expect: 2
    }

    test {
        input: 41
        expect: 42
    }
}
}
"#;

fn v(name: &str) -> IRNode {
    IRNode::Var(name.into())
}

fn il(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}

fn bx(n: IRNode) -> Box<IRNode> {
    Box::new(n)
}

// ── In-process server helpers (the Phase 149 shapes) ────

fn request(server: &mut McpServer, id: u64, method: &str, params: Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
        .to_string();
    let response = server
        .handle_line(&line)
        .expect("a request with an id always gets a response");
    serde_json::from_str(&response).expect("the response is one JSON value")
}

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
        json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "phase151-gate", "version": "0"}}),
    );
    assert!(response.get("result").is_some(), "initialize answers: {response}");
    server
}

/// The deterministic projection of a SynthesisReport:
/// everything except the wall-clock and float-score
/// fields — the freeze's "byte-for-byte where the API
/// is deterministic", stated as data.
fn synth_projection(p: &Value) -> Value {
    json!({
        "spec_name": p["spec_name"],
        "verification_available": p["verification_available"],
        "winner": {
            "body_debug": p["winner"]["body_debug"],
            "verification_status": p["winner"]["verification_status"],
            "tests_passed": p["winner"]["tests_passed"],
            "tests_evaluable": p["winner"]["tests_evaluable"],
            "tests_total": p["winner"]["tests_total"],
        },
    })
}

/// The deterministic projection of a VerifyReport (the
/// same carve-out: `verification_time_ms` excluded).
fn verify_projection(p: &Value) -> Value {
    json!({
        "spec_name": p["spec_name"],
        "body_source": p["body_source"],
        "verification_available": p["verification_available"],
        "status": p["status"],
        "rows": p["rows"],
        "tests_passed": p["tests_passed"],
        "tests_evaluable": p["tests_evaluable"],
        "tests_total": p["tests_total"],
    })
}

// ── Clause 1: the channel works ─────────────────────────

/// The gate's scripted client: the shipped binary,
/// spoken to over its stdio, end to end — and every
/// answer checked against the library API's own output
/// for the same call, computed in this process.
#[test]
#[cfg(all(feature = "z3-support", not(feature = "wasm-playground")))]
fn gate_clause1_stdio_transcript_and_api_agreement() {
    let _guard = crown_serial();
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

    let inc_body = IRNode::BinOp(IRBinOp::Add, bx(v("x")), bx(il(1)));
    let script = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "phase151-gate-client", "version": "0"}}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "check", "arguments": {"source": VALIDATOR}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "synthesize", "arguments": {"source": VALIDATOR, "spec": "is_known_type_tag", "seed": 42}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "synthesize", "arguments": {"source": VALIDATOR, "spec": "is_known_type_tag", "seed": 42}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "verify", "arguments": {"source": STDLIB_INT, "spec": "int_abs", "body": {"kind": "library", "entry": "int_abs"}}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "verify", "arguments": {"source": INC_MODULE, "spec": "inc", "body": {"kind": "ir-text", "text": write_ir(&inc_body)}}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": {"name": "library", "arguments": {}}}).to_string(),
        "{garbage".to_string(),
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
        .recv_timeout(Duration::from_secs(300))
        .expect("the server answers and exits on EOF");
    let _ = reader.join();
    let status = child.wait().expect("the child reaps");
    let ms = t0.elapsed().as_millis();
    assert!(status.success(), "EOF closes the server cleanly: {status}");

    // Nine responses in order (the notification gets
    // none). Every line is JSON — the CLI banner never
    // touches this stream.
    assert_eq!(lines.len(), 9, "one response per request: {lines:?}");
    let parsed: Vec<Value> = lines
        .iter()
        .map(|l| serde_json::from_str(l).expect("every stdout line is JSON-RPC"))
        .collect();
    let tool_payload = |i: usize| -> Value {
        let result = &parsed[i]["result"];
        assert_eq!(result["isError"], false, "the call succeeds: {}", parsed[i]);
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap())
            .expect("tool text is DTO JSON")
    };

    // initialize: the build names itself and its version.
    assert_eq!(parsed[0]["result"]["serverInfo"]["name"], "morphic");
    assert_eq!(
        parsed[0]["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION"),
        "the server names the build it is"
    );
    assert_eq!(parsed[0]["result"]["protocolVersion"], "2024-11-05");

    // tools/list: the frozen four-tool surface, schemas
    // closed, the verify body kinds exactly the three.
    let tools = parsed[1]["result"]["tools"].as_array().expect("a tool array");
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["check", "synthesize", "verify", "library"]);
    for t in tools {
        assert_eq!(
            t["inputSchema"]["additionalProperties"], false,
            "{}'s argument set is closed",
            t["name"]
        );
    }
    let verify_tool = tools.iter().find(|t| t["name"] == "verify").unwrap();
    assert_eq!(
        verify_tool["inputSchema"]["properties"]["body"]["properties"]["kind"]["enum"],
        json!(["library", "denoted", "ir-text"]),
        "the three body sources, the third being Phase 150's"
    );

    // check: byte-for-byte the API's own report.
    let check_payload = tool_payload(2);
    let api_check = serde_json::to_value(morphic::api::check(VALIDATOR))
        .expect("CheckReport serializes");
    assert_eq!(
        check_payload, api_check,
        "the check tool's answer is the API's own output, byte-for-byte"
    );

    // synthesize: crowns Verified; the deterministic
    // projection is the API's own; a second identical
    // call crowns the identical body (D3a).
    let synth1 = tool_payload(3);
    assert_eq!(synth1["winner"]["verification_status"], "Verified");
    let api_synth = serde_json::to_value(
        morphic::api::synthesize_seeded_spec(VALIDATOR, "is_known_type_tag", 42)
            .expect("the API run completes"),
    )
    .expect("SynthesisReport serializes");
    assert_eq!(
        synth_projection(&synth1),
        synth_projection(&api_synth),
        "the synthesize tool's answer agrees with the API's own run"
    );
    let synth2 = tool_payload(4);
    assert_eq!(
        synth1["winner"]["body_debug"], synth2["winner"]["body_debug"],
        "same (source, seed) → same crowned body, through the server"
    );

    // verify (library body): Verified, and the
    // projection is the standalone entry point's own.
    let verify1 = tool_payload(5);
    assert_eq!(verify1["status"], "Verified");
    let api_verify = serde_json::to_value(
        morphic::api::verify_body(
            STDLIB_INT,
            "int_abs",
            &BodySource::Library("int_abs".to_string()),
        )
        .expect("the API judgment runs"),
    )
    .expect("VerifyReport serializes");
    assert_eq!(
        verify_projection(&verify1),
        verify_projection(&api_verify),
        "the verify tool's answer agrees with api::verify_body's own"
    );

    // verify (ir-text body): the third source, judged
    // in-process, Verified.
    let verify2 = tool_payload(6);
    assert_eq!(verify2["body_source"], "ir-text");
    assert_eq!(verify2["status"], "Verified");

    // library: the manifest's own rows, row for row,
    // at this build's version.
    let library = tool_payload(7);
    assert_eq!(library["packageVersion"], env!("CARGO_PKG_VERSION"));
    let set = bindings::stdlib_bindings().expect("the manifest generates");
    let manifest_rows: Vec<Value> = set
        .manifest
        .exports
        .iter()
        .map(|e| {
            json!({
                "name": e.name,
                "signature": e.signature,
                "tier": e.tier,
                "verdictBasis": e.verdict_basis,
                "battery": {"pinned": e.battery.pinned, "specSilent": e.battery.spec_silent},
                "doc": e.doc,
            })
        })
        .collect();
    assert_eq!(
        library["exports"],
        Value::Array(manifest_rows),
        "the library tool serves the generated manifest's rows, never restated ones"
    );
    assert_eq!(library["exports"].as_array().unwrap().len(), 35);

    // The garbage line is answered in order, -32700.
    assert_eq!(parsed[8]["error"]["code"], -32700);

    println!(
        "P151|clause1|transcript=9-responses check=api-byte-equal synthesize=Verified+api-agree verify=Verified+api-agree ir-text=Verified library=35-rows wall_ms={ms}"
    );
}

// ── Clause 2: verdict honesty at the boundary ───────────

#[test]
#[cfg(not(feature = "wasm-playground"))]
fn gate_clause2_verdict_honesty_at_the_boundary() {
    let _guard = crown_serial();
    let mut server = handshaken_server();

    // The shipped body verifies; the wrong body stands
    // VerificationFailed through the tool, never dressed.
    let (is_error, payload) = tool_call(
        &mut server,
        2,
        "verify",
        json!({"source": STDLIB_INT, "spec": "int_abs", "body": {"kind": "library", "entry": "int_abs"}}),
    );
    assert!(!is_error);
    assert_eq!(
        payload["verification_available"],
        cfg!(feature = "z3-support"),
        "availability is the build's own fact"
    );
    #[cfg(feature = "z3-support")]
    assert_eq!(payload["status"], "Verified");
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(payload["status"], "Inconclusive");

    let wrong = json!({"source": STDLIB_INT, "spec": "int_abs", "body": {"kind": "library", "entry": "int_sign"}});
    let (is_error, payload) = tool_call(&mut server, 3, "verify", wrong.clone());
    assert!(!is_error);
    #[cfg(feature = "z3-support")]
    assert_eq!(payload["status"], "VerificationFailed");
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(payload["status"], "Inconclusive");

    // A caller-stated claim has no channel: the same
    // wrong body, now with the verdict the caller wants
    // riding along as extra arguments, is judged
    // exactly as before — the offered claim is not an
    // input the verdict reads.
    let mut offered = wrong.clone();
    offered["verdict"] = json!("Verified");
    offered["status"] = json!("Verified");
    offered["verification_status"] = json!("Verified");
    let (is_error, payload) = tool_call(&mut server, 4, "verify", offered);
    assert!(!is_error, "the call still runs: {payload}");
    #[cfg(feature = "z3-support")]
    assert_eq!(
        payload["status"], "VerificationFailed",
        "an offered verdict changes nothing: {payload}"
    );
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(payload["status"], "Inconclusive");

    // No tool's schema carries a verdict-shaped
    // argument at all (the frozen surface, re-read at
    // the gate).
    let list = request(&mut server, 5, "tools/list", json!({}));
    for tool in list["result"]["tools"].as_array().unwrap() {
        let schema = tool["inputSchema"].to_string();
        for banned in ["verdict", "verification_status", "claimed"] {
            assert!(
                !schema.contains(&format!("\"{banned}\"")),
                "{}'s schema must not carry a '{banned}' argument: {schema}",
                tool["name"]
            );
        }
    }

    // Synthesis determinism in-process: same (source,
    // seed) → same crowned body, and the availability
    // fact rides the report.
    let args = json!({"source": VALIDATOR, "spec": "is_known_type_tag", "seed": 42});
    let (_, first) = tool_call(&mut server, 6, "synthesize", args.clone());
    let (_, second) = tool_call(&mut server, 7, "synthesize", args);
    assert_eq!(
        first["winner"]["body_debug"], second["winner"]["body_debug"],
        "the crown is the run's own, reproducibly"
    );
    assert_eq!(
        first["verification_available"],
        cfg!(feature = "z3-support")
    );
    println!(
        "P151|clause2|wrong_body={} offered_claim=ignored availability={}",
        payload["status"], first["verification_available"]
    );
}

/// The no-Z3 posture at the CLI, by name (DISEBAR_SCOPE
/// §3 R3): a build without the verifier refuses the
/// subcommand rather than dressing executed evidence
/// as verdicts. This pin exists exactly in the builds
/// it describes; in the Z3 builds the transcript above
/// is the corresponding evidence.
#[test]
#[cfg(not(feature = "z3-support"))]
fn gate_clause2_no_z3_build_refuses_mcp_by_name() {
    let _guard = crown_serial();
    let mut child: Child = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary spawns");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("the child reaps");
    assert!(
        !output.status.success(),
        "the refusal is a failure exit: {}",
        output.status
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Z3-enabled native build"),
        "the refusal names the build the server ships in: {stderr}"
    );
    println!("P151|clause2|no_z3_cli_refusal=named");
}

// ── Clause 3: the evidence re-checks ────────────────────

#[test]
#[cfg(all(not(feature = "wasm-playground"), feature = "z3-support"))]
fn gate_clause3_evidence_rechecks_as_a_consumer() {
    let _guard = crown_serial();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // verify --library, as its library function: every
    // shipped crown reproduces from the shipped data.
    let standings = pcc::verify_library().expect("the self-check runs");
    assert_eq!(standings.len(), 35, "one standing per shipped entry");
    for s in &standings {
        assert!(
            s.reproduced,
            "{}: the crown reproduces ({})",
            s.name, s.detail
        );
    }

    // Every checked-in bundle re-verifies from its
    // file bytes — the consumer's starting point.
    let dir = root.join(pcc::BUNDLES_DIR);
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the bundles directory reads")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 35, "one bundle per shipped entry");
    for path in &files {
        let text = std::fs::read_to_string(path).expect("the bundle reads");
        let summary = pcc::verify_bundle_text(&text).unwrap_or_else(|e| {
            panic!(
                "{}: the honest bundle re-verifies ({e})",
                path.display()
            )
        });
        assert!(summary.contains("reproduces"), "{}: {summary}", path.display());
    }

    // The consumer verb itself, through the CLI: both
    // crown routes and the Tier-2 entry exit 0.
    for file in ["int_abs.bundle.json", "list_map.bundle.json", "list_sort.bundle.json"] {
        let output = Command::new(env!("CARGO_BIN_EXE_morphic"))
            .arg("verify-bundle")
            .arg(dir.join(file))
            .output()
            .expect("the verb runs");
        assert!(
            output.status.success(),
            "{file}: verify-bundle exits 0 ({})",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("reproduces"), "{file}: {stdout}");
    }

    // A tampered copy through the same verb: refused
    // by name, non-zero exit. (The flipped verdict
    // makes the claim incoherent on its own terms.)
    let honest = std::fs::read_to_string(dir.join("int_abs.bundle.json")).unwrap();
    let mut value: Value = serde_json::from_str(&honest).unwrap();
    value["verdict"] = json!("ExecutedExact");
    let tampered_path = std::env::temp_dir()
        .join(format!("morphic-gate151-tampered-{}.json", std::process::id()));
    std::fs::write(&tampered_path, serde_json::to_string(&value).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("verify-bundle")
        .arg(&tampered_path)
        .output()
        .expect("the verb runs");
    let _ = std::fs::remove_file(&tampered_path);
    assert!(
        !output.status.success(),
        "a tampered bundle never passes"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bundle refused"), "the refusal is the verb's: {stderr}");
    assert!(
        stderr.contains("incoherent claim"),
        "the refusal names the shape: {stderr}"
    );

    // The manifests' language: both packages' READMEs
    // and the embedding guide point at the bundles as
    // the rows' evidence (§4.4's surface work).
    for rel in ["packages/npm/README.md", "packages/rust/README.md", "EMBEDDING.md"] {
        let text = std::fs::read_to_string(root.join(rel)).expect("the surface reads");
        assert!(
            text.contains("verify-bundle"),
            "{rel} names the consumer verb as the rows' evidence"
        );
        assert!(
            text.contains("packages/bundles"),
            "{rel} names the bundles' home"
        );
    }
    println!("P151|clause3|verify_library=35/35 bundles=35-reverify cli=3-exit-0 tamper_cli=refused-by-name manifest_language=3-surfaces");
}

#[cfg(all(not(feature = "wasm-playground"), feature = "z3-support"))]
fn tamper(bundle_text: &str, mutate: impl Fn(&mut Value)) -> String {
    let mut value: Value = serde_json::from_str(bundle_text).expect("the bundle is JSON");
    mutate(&mut value);
    let text = serde_json::to_string(&value).expect("the mutation serializes");
    pcc::verify_bundle_text(&text).expect_err("a tampered bundle never passes")
}

/// The tamper battery, re-run at the gate: eight
/// shapes, each refused, no two refusals alike.
#[test]
#[cfg(all(not(feature = "wasm-playground"), feature = "z3-support"))]
fn gate_clause3_tamper_battery_refuses_by_name() {
    let _guard = crown_serial();
    let inputs = bindings::stdlib_inputs();
    let bundle_of = |name: &str| -> String {
        let input = inputs.iter().find(|i| i.spec.name == name).unwrap();
        serde_json::to_string(&pcc::produce_bundle(input).expect("production"))
            .expect("the bundle serializes")
    };
    let int_abs = bundle_of("int_abs");
    let list_map = bundle_of("list_map");
    let sign_body = write_ir(
        &inputs.iter().find(|i| i.spec.name == "int_sign").unwrap().body,
    );

    let mut refusals: Vec<String> = Vec::new();
    refusals.push(tamper(&int_abs, |b| {
        b["verdict"] = json!("ExecutedExact");
    }));
    assert!(refusals[0].contains("incoherent claim"), "{}", refusals[0]);
    refusals.push(tamper(&int_abs, |b| {
        b["tier"] = json!("tier-2");
        b["verdictBasis"] = json!("executed-exact");
        b["verdict"] = json!("ExecutedExact");
    }));
    assert!(refusals[1].contains("claim mismatch for shipped entry"), "{}", refusals[1]);
    refusals.push(tamper(&int_abs, |b| {
        b["body"] = json!(sign_body);
    }));
    assert!(refusals[2].contains("different body"), "{}", refusals[2]);
    refusals.push(tamper(&list_map, |b| {
        b["crownRoute"] = json!("pipeline");
    }));
    assert!(refusals[3].contains("crown route"), "{}", refusals[3]);
    refusals.push(tamper(&int_abs, |b| {
        b["bundleVersion"] = json!(2);
    }));
    assert!(refusals[4].contains("bundle version"), "{}", refusals[4]);
    refusals.push(tamper(&int_abs, |b| {
        let body = b["body"]
            .as_str()
            .unwrap()
            .replacen(morphic::synthesis::irtext::IR_TEXT_VERSION, "morphic-ir/2", 1);
        b["body"] = json!(body);
    }));
    assert!(refusals[5].contains("morphic-ir/2"), "{}", refusals[5]);
    refusals.push(tamper(&int_abs, |b| {
        let passed = b["battery"]["passed"].as_u64().unwrap();
        b["battery"]["passed"] = json!(passed + 1);
    }));
    assert!(refusals[6].contains("battery"), "{}", refusals[6]);
    refusals.push(tamper(&int_abs, |b| {
        b["body"] = json!("(block");
    }));
    assert!(refusals[7].contains("ir text"), "{}", refusals[7]);

    let mut distinct = refusals.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        refusals.len(),
        "every shape's refusal is its own: {refusals:?}"
    );
    println!("P151|clause3|tamper_shapes=8 refusals=pairwise-distinct");
}

// ── Clause 4: footprint ─────────────────────────────────

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
fn gate_clause4_footprint() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // The compiler's runtime dependency set is the
    // standing set: the MCP server is a hand-rolled
    // shell over serde_json (already present) and PCC
    // is in-tree — neither added a dependency. (The
    // lock graph's byte-diff against v1.3.0 is the
    // gate record's companion evidence.)
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
            let name = trimmed
                .split(['=', ' ', '{'])
                .next()
                .unwrap()
                .trim_matches('"');
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
        "the default build's dependency set is the standing set"
    );

    // The bundles: a sibling artifact at its recorded
    // mass — 35 files, 267,156 B — in neither package's
    // file set, so neither frozen budget is charged.
    let (bundle_files, bundle_bytes) = dir_mass(&root.join(pcc::BUNDLES_DIR));
    assert_eq!(bundle_files, 35, "one bundle per shipped entry");
    assert_eq!(bundle_bytes, 267_156, "the bundles' recorded mass");

    // The npm package inside its frozen budget.
    let (npm_files, npm_bytes) = dir_mass(&root.join("packages/npm"));
    assert_eq!(npm_files, 77, "the assembled package's file set");
    assert_eq!(npm_bytes, 71_873, "the package's standing unpacked mass");
    assert!(
        npm_bytes <= package::NPM_UNPACKED_BUDGET,
        "{npm_bytes} B inside the frozen {} B budget",
        package::NPM_UNPACKED_BUDGET
    );
    println!(
        "P151|clause4|deps=standing-13 bundles=35-files/267156B npm={npm_files}-files/{npm_bytes}B<={}",
        package::NPM_UNPACKED_BUDGET
    );
}
