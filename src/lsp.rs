// ─────────────────────────────────────────────────────────────
//  LSP SERVER (Phase 155, DISEBAR_15_SCOPE §5)
//
//  `morphic lsp` as the real server: a hand-rolled
//  Language Server Protocol server over stdio — JSON-RPC
//  2.0 framed the LSP way (Content-Length headers, the
//  framing the MCP core measurably lacks), zero new
//  dependencies. Diagnostics-only (§5.4): the server
//  offers full-document sync and push diagnostics, and
//  nothing else — no hover, no completion, no definition,
//  no formatting, no code actions.
//
//  THE DIAGNOSTICS STORY (the engine's measured shape,
//  §5.3): diagnostics come from api::check, whose error
//  is singular — publishDiagnostics therefore carries
//  AT MOST ONE diagnostic per check (0 for a clean file).
//  The two error shapes publish differently, honestly:
//  a parse error carries its true position (the API's
//  1-based line/column, converted to LSP's 0-based); a
//  typecheck error carries no position anywhere in the
//  compiler (TypeError.span is None at every
//  construction site; the AST carries no spans), so it
//  publishes as ONE file-level diagnostic at (0,0)–(0,0)
//  whose message is the verbatim typecheck message —
//  it names its spec — prefixed with the frozen label
//  `unpositioned: ` (D3a / R3). The position is absent
//  because the compiler does not know it, and the
//  diagnostic says so rather than letting a squiggle at
//  1:1 imply one.
//
//  THE NOTIFICATION INVERSION (§5.1): the MCP core never
//  executes an id-less message; LSP's didOpen / didChange
//  / didClose ARE id-less and must execute — they carry
//  the document state — and publishDiagnostics is itself
//  an id-less server→client notification. What transfers
//  from the MCP core is the dispatch pattern and the
//  error vocabulary (−32700 / −32600 / −32601); the
//  discipline does not.
//
//  LIFECYCLE: initialize → initialized → document
//  traffic → shutdown (null result) → exit. The process
//  exit code is the lifecycle's own: 0 when shutdown
//  preceded the session's end, 1 when it did not —
//  whether the end came by `exit` or by EOF. No input
//  panics the server: malformed frames are answered
//  −32700 where recoverable and the server lives on.
//  Stdout is the protocol stream — the CLI banner never
//  touches it (main.rs keeps it off this subcommand),
//  and nothing here writes to it but frames.
// ─────────────────────────────────────────────────────────────

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::api;

/// The diagnostics-only language server. State is the
/// open documents (URI → full text, D5a full-document
/// sync) plus the lifecycle flags the exit code reads.
pub struct LspServer {
    documents: HashMap<String, String>,
    shutdown_received: bool,
    exit_requested: bool,
}

impl Default for LspServer {
    fn default() -> Self {
        Self::new()
    }
}

impl LspServer {
    pub fn new() -> Self {
        LspServer { documents: HashMap::new(), shutdown_received: false, exit_requested: false }
    }

    /// True once the client has sent `exit`: the stdio
    /// loop stops and the process takes `exit_code`.
    pub fn should_exit(&self) -> bool {
        self.exit_requested
    }

    /// The lifecycle's exit code: 0 when `shutdown`
    /// preceded the end of the session, 1 otherwise.
    pub fn exit_code(&self) -> i32 {
        if self.shutdown_received {
            0
        } else {
            1
        }
    }

    /// Handle one decoded frame body. Returns the wire
    /// messages to send back, in order — responses and
    /// publishDiagnostics notifications interleaved
    /// exactly as produced. Never panics: malformed
    /// input is answered with a protocol error and the
    /// server lives on.
    pub fn handle_message(&mut self, text: &str) -> Vec<String> {
        let value: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => {
                return vec![error_value(
                    Value::Null,
                    -32700,
                    &format!("parse error: {e}"),
                )
                .to_string()];
            }
        };
        self.handle_value(&value).iter().map(Value::to_string).collect()
    }

    fn handle_value(&mut self, value: &Value) -> Vec<Value> {
        let Some(obj) = value.as_object() else {
            return vec![error_value(
                Value::Null,
                -32600,
                "invalid request: expected a JSON-RPC object (batch requests are not supported)",
            )];
        };
        let id = obj.get("id").cloned();
        let Some(method) = obj.get("method").and_then(Value::as_str) else {
            // A message with an id but no method is a
            // malformed request; an id-less one is not a
            // message this protocol can act on at all.
            return match id {
                Some(id) => vec![error_value(id, -32600, "invalid request: no 'method'")],
                None => vec![],
            };
        };
        // `exit` is a notification by definition: it is
        // never answered, with or without an id.
        if method == "exit" {
            self.exit_requested = true;
            return vec![];
        }
        match id {
            Some(id) => self.handle_request(id, method, obj.get("params")),
            None => self.handle_notification(method, obj.get("params")),
        }
    }

    fn handle_request(&mut self, id: Value, method: &str, params: Option<&Value>) -> Vec<Value> {
        match method {
            "initialize" => vec![result_value(id, initialize_result())],
            "shutdown" => {
                self.shutdown_received = true;
                vec![result_value(id, Value::Null)]
            }
            // The document notifications are notifications
            // by spec; a client that nevertheless sends
            // one as a request gets it executed and a null
            // result after the publish it produced.
            "textDocument/didOpen" | "textDocument/didChange" | "textDocument/didClose" => {
                let mut out = self.handle_notification(method, params);
                out.push(result_value(id, Value::Null));
                out
            }
            other => vec![error_value(
                id,
                -32601,
                &format!(
                    "method not found: '{other}' \
                     (this server speaks initialize, shutdown, \
                     textDocument/didOpen, textDocument/didChange, \
                     textDocument/didClose, exit)"
                ),
            )],
        }
    }

    fn handle_notification(&mut self, method: &str, params: Option<&Value>) -> Vec<Value> {
        match method {
            "textDocument/didOpen" => {
                let Some((uri, text)) = params
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|td| {
                        let uri = td.get("uri")?.as_str()?.to_string();
                        let text = td.get("text")?.as_str()?.to_string();
                        Some((uri, text))
                    })
                else {
                    return vec![];
                };
                self.documents.insert(uri.clone(), text.clone());
                vec![publish_diagnostics(&uri, &text)]
            }
            "textDocument/didChange" => {
                // Full-document sync (D5a): every change
                // event carries the whole text; with more
                // than one event, the last is the document.
                let Some((uri, text)) = params.and_then(|p| {
                    let uri = p.get("textDocument")?.get("uri")?.as_str()?.to_string();
                    let text = p
                        .get("contentChanges")?
                        .as_array()?
                        .last()?
                        .get("text")?
                        .as_str()?
                        .to_string();
                    Some((uri, text))
                }) else {
                    return vec![];
                };
                self.documents.insert(uri.clone(), text.clone());
                vec![publish_diagnostics(&uri, &text)]
            }
            "textDocument/didClose" => {
                let Some(uri) = params
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|td| td.get("uri"))
                    .and_then(Value::as_str)
                else {
                    return vec![];
                };
                self.documents.remove(uri);
                vec![publish_empty_diagnostics(uri)]
            }
            // `initialized` needs no answer and no state;
            // unknown notifications are ignored, per
            // protocol — a notification is never answered.
            _ => vec![],
        }
    }
}

/// The initialize answer: full-document sync and push
/// diagnostics, and nothing else (§5.2, §5.4). The
/// diagnostics story — at most one per check, the two
/// error shapes — is this module's header; the wire
/// claims only what a client can act on.
fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": {"openClose": true, "change": 1},
        },
        "serverInfo": {
            "name": "morphic",
            "version": env!("CARGO_PKG_VERSION"),
        },
    })
}

/// The publishDiagnostics notification for a document's
/// current text: api::check's verdict, mapped per §5.3.
fn publish_diagnostics(uri: &str, text: &str) -> Value {
    let report = api::check(text);
    let diagnostics = match report.error {
        None => vec![],
        Some(e) if e.stage == "parse" => {
            // The API's positions are 1-based; LSP's are
            // 0-based. The conversion is mechanical and
            // total (a parse error always carries both).
            let line = e.line.unwrap_or(1).saturating_sub(1) as u64;
            let character = e.column.unwrap_or(1).saturating_sub(1) as u64;
            let point = json!({"line": line, "character": character});
            vec![json!({
                "range": {"start": point.clone(), "end": point},
                "severity": 1,
                "source": "morphic",
                "message": e.message,
            })]
        }
        Some(e) => {
            // No position exists for this shape anywhere
            // in the compiler: one file-level diagnostic
            // at (0,0)–(0,0), the verbatim message under
            // the frozen `unpositioned: ` label (R3).
            vec![json!({
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 0},
                },
                "severity": 1,
                "source": "morphic",
                "message": format!("unpositioned: {}", e.message),
            })]
        }
    };
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {"uri": uri, "diagnostics": diagnostics},
    })
}

fn publish_empty_diagnostics(uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {"uri": uri, "diagnostics": []},
    })
}

fn result_value(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_value(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

// ── The stdio transport: LSP framing ────────────────────

enum Frame {
    /// One complete frame body.
    Message(Vec<u8>),
    /// A header block naming no usable Content-Length:
    /// answered −32700, and reading resumes at the next
    /// header block — the stream stays alive.
    Malformed(String),
    /// The stream ended (cleanly, or mid-frame where
    /// nothing more can be said on it).
    Eof,
}

/// Read one frame: header lines up to the blank line,
/// then exactly Content-Length bytes. Header names are
/// matched case-insensitively; Content-Type and any
/// other header are ignored, per the base protocol.
fn read_frame(input: &mut impl std::io::BufRead) -> std::io::Result<Frame> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut raw = Vec::new();
        let n = input.read_until(b'\n', &mut raw)?;
        if n == 0 {
            return Ok(Frame::Eof);
        }
        let line = String::from_utf8_lossy(&raw);
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().ok();
            }
        }
    }
    let Some(n) = content_length else {
        return Ok(Frame::Malformed(
            "malformed frame header: no usable Content-Length".into(),
        ));
    };
    let mut body = vec![0u8; n];
    match input.read_exact(&mut body) {
        Ok(()) => Ok(Frame::Message(body)),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(Frame::Eof),
        Err(e) => Err(e),
    }
}

fn write_frame(out: &mut impl std::io::Write, body: &str) -> std::io::Result<()> {
    write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    out.flush()
}

/// Serve LSP over this process's stdio until `exit` or
/// EOF, and return the lifecycle's exit code for the
/// process to take (0 after shutdown, 1 without).
pub fn run_stdio() -> std::io::Result<i32> {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut server = LspServer::new();
    loop {
        match read_frame(&mut input)? {
            Frame::Eof => break,
            Frame::Malformed(why) => {
                let response = error_value(Value::Null, -32700, &format!("parse error: {why}"));
                write_frame(&mut out, &response.to_string())?;
            }
            Frame::Message(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                for message in server.handle_message(&text) {
                    write_frame(&mut out, &message)?;
                }
                if server.should_exit() {
                    break;
                }
            }
        }
    }
    Ok(server.exit_code())
}
