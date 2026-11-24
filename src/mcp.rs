// ─────────────────────────────────────────────────────────────
//  MCP SERVER (Phase 149, DISEBAR_SCOPE §3)
//
//  A hand-rolled Model Context Protocol server over
//  stdio: newline-delimited JSON-RPC 2.0, the standing
//  subset (initialize / ping / tools/list / tools/call),
//  zero new dependencies — serde_json was already the
//  tree's JSON. The server is a shell over src/api.rs's
//  DTOs, exactly as the browser shell is; the CLI owns
//  the process, this module owns the protocol.
//
//  HONESTY CONTRACT (§3): every verdict the server
//  serves is the native verifier's own, produced
//  in-process at call time from the arguments the caller
//  sent — nothing is cached, precomputed, or accepted
//  from the caller. The server ships only in the
//  Z3-enabled native build (the CLI refuses it by name
//  otherwise, §3 R3): a server that cannot verify could
//  only dress executed evidence as verdicts.
//
//  SURFACE DISCIPLINE (§3): tools take source text as
//  arguments. No tool accepts a filesystem path, the
//  server never shells out and never reads files, and
//  synthesis runs only the seeded, bounded, LLM-free
//  playground profile — there is no LLM key in that
//  profile for a call to reach.
//
//  THE THIRD BODY SOURCE (§3.3 / R1, Phase 150): the
//  freeze named `verify`'s two sources and recorded that
//  a third — a body carried as a serialization — would
//  arrive with Phase 150's bundle format. It has: kind
//  "ir-text" carries the body in the morphic-ir text
//  serialization (synthesis::irtext), parsed by the same
//  reader the bundles use, declining malformed or
//  foreign-version text by name. The widening is this
//  note and the §4 freeze text that ordered it — never
//  silent.
// ─────────────────────────────────────────────────────────────

use serde_json::{json, Value};

use crate::api::{self, BodySource, StageError};

/// The spec-size cap (DISEBAR_SCOPE §3 R2): every tool
/// that takes a `source` argument declines a larger one
/// by name before any stage runs. Sized to cover the
/// largest module in the standing corpus
/// (examples/self_host/codegen.morph, 152,251 B) with
/// headroom.
pub const MAX_SOURCE_BYTES: usize = 262_144;

/// The protocol version answered when a client names
/// none at initialize.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// The frozen tool surface (§3.2), in tools/list order.
const TOOL_NAMES: [&str; 4] = ["check", "synthesize", "verify", "library"];

/// The protocol handler. Stateless across calls except
/// for the connection itself: each line is judged on
/// its own, and no verdict or report is ever carried
/// from one call to the next.
pub struct McpServer;

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    pub fn new() -> Self {
        McpServer
    }

    /// Handle one line of stdio input. Returns the
    /// response line, or None for input that takes no
    /// response (blank lines; notifications and other
    /// id-less messages, which are never executed).
    /// Never panics: malformed input is answered with a
    /// protocol error and the server lives on.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                return Some(error_value(Value::Null, -32700, &format!("parse error: {e}")).to_string());
            }
        };
        self.handle_value(&value).map(|v| v.to_string())
    }

    fn handle_value(&mut self, value: &Value) -> Option<Value> {
        let Some(obj) = value.as_object() else {
            return Some(error_value(
                Value::Null,
                -32600,
                "invalid request: expected a JSON-RPC object (batch requests are not supported)",
            ));
        };
        // An id-less message is a notification: never
        // answered, never executed.
        let id = obj.get("id")?.clone();
        let Some(method) = obj.get("method").and_then(Value::as_str) else {
            return Some(error_value(id, -32600, "invalid request: no 'method'"));
        };
        match method {
            "initialize" => {
                let requested = obj
                    .get("params")
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(Value::as_str);
                Some(result_value(
                    id,
                    json!({
                        "protocolVersion": requested.unwrap_or(PROTOCOL_VERSION),
                        "capabilities": {"tools": {}},
                        "serverInfo": {
                            "name": "morphic",
                            "version": env!("CARGO_PKG_VERSION"),
                        },
                    }),
                ))
            }
            "ping" => Some(result_value(id, json!({}))),
            "tools/list" => Some(result_value(id, json!({"tools": tool_definitions()}))),
            "tools/call" => Some(self.handle_tools_call(id, obj.get("params"))),
            other => Some(error_value(
                id,
                -32601,
                &format!(
                    "method not found: '{other}' \
                     (this server speaks initialize, ping, tools/list, tools/call)"
                ),
            )),
        }
    }

    fn handle_tools_call(&self, id: Value, params: Option<&Value>) -> Value {
        let Some(params) = params.and_then(Value::as_object) else {
            return error_value(
                id,
                -32602,
                "invalid params: tools/call requires an object with a tool 'name'",
            );
        };
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return error_value(
                id,
                -32602,
                "invalid params: tools/call requires a tool 'name'",
            );
        };
        if !TOOL_NAMES.contains(&name) {
            return error_value(
                id,
                -32602,
                &format!("unknown tool: '{name}' (tools: {})", TOOL_NAMES.join(", ")),
            );
        }
        let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        match self.call_tool(name, &arguments) {
            Ok(payload) => result_value(
                id,
                json!({
                    "content": [{"type": "text", "text": payload.to_string()}],
                    "isError": false,
                }),
            ),
            Err(e) => result_value(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&e)
                            .unwrap_or_else(|_| e.message.clone()),
                    }],
                    "isError": true,
                }),
            ),
        }
    }

    /// One tool execution. The Ok payload is the tool's
    /// DTO as JSON (carried as the result's text content);
    /// an Err is a tool error — a StageError in the API's
    /// own shape, carried as an isError result, never a
    /// protocol failure.
    fn call_tool(&self, name: &str, args: &Value) -> Result<Value, StageError> {
        match name {
            "check" => {
                let source = arg_source(args)?;
                Ok(serde_json::to_value(api::check(source)).expect("CheckReport serializes"))
            }
            "synthesize" => {
                let source = arg_source(args)?;
                let seed = opt_seed(args)?;
                let report = match opt_str(args, "spec")? {
                    Some(spec) => api::synthesize_seeded_spec(source, spec, seed)?,
                    None => api::synthesize_seeded(source, seed)?,
                };
                Ok(serde_json::to_value(report).expect("SynthesisReport serializes"))
            }
            "verify" => {
                let source = arg_source(args)?;
                let spec = req_str(args, "spec")?;
                let body = body_source_arg(args)?;
                let report = api::verify_body(source, spec, &body)?;
                Ok(serde_json::to_value(report).expect("VerifyReport serializes"))
            }
            "library" => library_tool(args),
            _ => unreachable!("tool names are gated by TOOL_NAMES"),
        }
    }
}

fn result_value(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_value(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn arguments_error(message: String) -> StageError {
    StageError { stage: "arguments".into(), message, line: None, column: None }
}

fn req_str<'a>(args: &'a Value, name: &str) -> Result<&'a str, StageError> {
    match args.get(name) {
        Some(Value::String(s)) => Ok(s),
        Some(_) => Err(arguments_error(format!("argument '{name}' must be a string"))),
        None => Err(arguments_error(format!("missing required argument '{name}'"))),
    }
}

fn opt_str<'a>(args: &'a Value, name: &str) -> Result<Option<&'a str>, StageError> {
    match args.get(name) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(arguments_error(format!("argument '{name}' must be a string"))),
    }
}

/// The `source` argument, under the frozen size cap:
/// an over-cap source declines by name before any
/// stage runs.
fn arg_source<'a>(args: &'a Value) -> Result<&'a str, StageError> {
    let source = req_str(args, "source")?;
    if source.len() > MAX_SOURCE_BYTES {
        return Err(arguments_error(format!(
            "source is {} bytes, over the server's cap of {MAX_SOURCE_BYTES} bytes \
             (DISEBAR_SCOPE §3) — split the module, or check it with the CLI",
            source.len()
        )));
    }
    Ok(source)
}

fn opt_seed(args: &Value) -> Result<u64, StageError> {
    match args.get("seed") {
        None => Ok(api::PLAYGROUND_DEFAULT_SEED),
        Some(v) => v
            .as_u64()
            .ok_or_else(|| arguments_error("argument 'seed' must be a non-negative integer".into())),
    }
}

/// The verify tool's `body` argument: the body sources
/// of DISEBAR_SCOPE §3.3 — library and denoted from the
/// freeze, plus ir-text, the third source the freeze's
/// §3.3/R1 recorded as Phase 150's consequence.
fn body_source_arg(args: &Value) -> Result<BodySource, StageError> {
    let Some(body) = args.get("body") else {
        return Err(arguments_error("missing required argument 'body'".into()));
    };
    let Some(kind) = body.get("kind").and_then(Value::as_str) else {
        return Err(arguments_error(
            "argument 'body' requires a 'kind' (\"library\" | \"denoted\" | \"ir-text\")"
                .into(),
        ));
    };
    match kind {
        "library" => {
            let Some(entry) = body.get("entry").and_then(Value::as_str) else {
                return Err(arguments_error(
                    "body kind 'library' requires an 'entry' naming a shipped library entry"
                        .into(),
                ));
            };
            Ok(BodySource::Library(entry.to_string()))
        }
        "denoted" => Ok(BodySource::Denoted),
        "ir-text" => {
            let Some(text) = body.get("text").and_then(Value::as_str) else {
                return Err(arguments_error(
                    "body kind 'ir-text' requires a 'text' carrying the body in the \
                     morphic-ir text serialization"
                        .into(),
                ));
            };
            Ok(BodySource::IrText(text.to_string()))
        }
        other => Err(arguments_error(format!(
            "unknown body kind: '{other}' (kinds: library, denoted, ir-text)"
        ))),
    }
}

/// The library tool: a faithful read of the shipped
/// data — the generated manifest's own rows for the
/// list form (EMBED_SCOPE §2.4), and for the fetch form
/// the entry's manifest row + its spec (the AST's own
/// serialization) + its shipped body in the pipeline's
/// identity form (the Debug rendering, the same form
/// the synthesize tool reports). No field is invented:
/// a row the manifest does not carry is not served.
fn library_tool(args: &Value) -> Result<Value, StageError> {
    let set = crate::codegen::bindings::stdlib_bindings().map_err(|e| StageError {
        stage: "library".into(),
        message: e.to_string(),
        line: None,
        column: None,
    })?;
    let row_json = |e: &crate::codegen::bindings::ManifestExport| {
        json!({
            "name": e.name,
            "signature": e.signature,
            "tier": e.tier,
            "verdictBasis": e.verdict_basis,
            "battery": {"pinned": e.battery.pinned, "specSilent": e.battery.spec_silent},
            "doc": e.doc,
        })
    };
    match opt_str(args, "entry")? {
        None => Ok(json!({
            "package": set.manifest.package,
            "packageVersion": set.manifest.package_version,
            "morphicVersion": set.manifest.morphic_version,
            "exports": set.manifest.exports.iter().map(row_json).collect::<Vec<_>>(),
        })),
        Some(entry) => {
            let row = set.manifest.exports.iter().find(|e| e.name == entry);
            let input = crate::codegen::bindings::stdlib_inputs()
                .into_iter()
                .find(|i| i.spec.name == entry);
            match (row, input) {
                (Some(row), Some(input)) => {
                    let mut payload = row_json(row);
                    payload["spec"] =
                        serde_json::to_value(&input.spec).expect("FunctionSpec serializes");
                    payload["body_debug"] = json!(format!("{:?}", input.body));
                    Ok(payload)
                }
                _ => Err(StageError {
                    stage: "library".into(),
                    message: format!(
                        "no shipped library entry named '{entry}' \
                         ({} entries ship; call the library tool without \
                         'entry' to list them)",
                        set.manifest.exports.len()
                    ),
                    line: None,
                    column: None,
                }),
            }
        }
    }
}

/// The frozen tool definitions (tools/list). The
/// argument sets are the freeze's: source text is the
/// only way in — no path, no key, no model argument
/// exists on any tool (§3.2).
fn tool_definitions() -> Value {
    json!([
        {
            "name": "check",
            "description": "Parse + typecheck one Morphic module given as source text, and report its specs with their test inventories. A source that fails checking is a successful call carrying the failing report.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "The module's source text."}
                },
                "required": ["source"],
                "additionalProperties": false
            }
        },
        {
            "name": "synthesize",
            "description": "Run the seeded, bounded, LLM-free synthesis profile over one spec and return the crowned body with its verdict and executed results. The verdict is the native verifier's own, produced at call time; the same (source, seed) crowns the same body.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "The module's source text."},
                    "spec": {"type": "string", "description": "The spec to synthesize (default: the module's first spec)."},
                    "seed": {"type": "integer", "minimum": 0, "description": "The engine seed (default: the playground's pinned seed, 42)."}
                },
                "required": ["source"],
                "additionalProperties": false
            }
        },
        {
            "name": "verify",
            "description": "Judge one candidate body against one spec with the native verifier, in-process: the verdict, the per-row results, and the executed test counts. The body is named by source: a shipped library body, the body the spec's own rows denote through the resolver, or the body itself carried as IR text (the morphic-ir serialization of Phase 150's bundles).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "The module's source text."},
                    "spec": {"type": "string", "description": "The spec to judge against."},
                    "body": {
                        "type": "object",
                        "properties": {
                            "kind": {"type": "string", "enum": ["library", "denoted", "ir-text"]},
                            "entry": {"type": "string", "description": "For kind 'library': the shipped entry whose body is judged."},
                            "text": {"type": "string", "description": "For kind 'ir-text': the body in the morphic-ir text serialization."}
                        },
                        "required": ["kind"],
                        "additionalProperties": false
                    }
                },
                "required": ["source", "spec", "body"],
                "additionalProperties": false
            }
        },
        {
            "name": "library",
            "description": "List the shipped standard-library entries with their tier, verdict basis and battery (the manifest's own rows), or fetch one entry's manifest row, spec and shipped body.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entry": {"type": "string", "description": "Omit to list all entries."}
                },
                "additionalProperties": false
            }
        }
    ])
}

/// Serve MCP over this process's stdio until EOF:
/// one JSON-RPC message per line in, one response line
/// per request out, flushed per response. The CLI's
/// banner never touches this stream (main.rs keeps it
/// off this subcommand).
pub fn run_stdio() -> std::io::Result<()> {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    let mut server = McpServer::new();
    for line in stdin.lock().lines() {
        let line = line?;
        if let Some(response) = server.handle_line(&line) {
            writeln!(out, "{response}")?;
            out.flush()?;
        }
    }
    Ok(())
}
