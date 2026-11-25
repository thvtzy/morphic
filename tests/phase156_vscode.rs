//! Phase 156 — the VS Code extension
//! (DISEBAR_15_SCOPE.md §6): the thin client at
//! `packages/vscode/` — a manifest contributing exactly
//! the frozen surface (the `morphic` language for
//! `.morph` files, a language configuration, a minimal
//! highlighting grammar, one configuration key), a
//! TypeScript entry that starts `morphic lsp` through
//! `vscode-languageclient`, and a `.vsix` packaged by
//! the package's own devDependencies.
//!
//! Pins: the manifest's contributed surface, exactly
//! (no commands, no views, no hover/completion —
//! diagnostics-only is the product); the client's
//! launch contract, pinned at its source (`morphic.
//! serverPath`, defaulting to `morphic` on PATH, run
//! with the single argument `lsp` over stdio) and then
//! exercised for real — the Phase 155 scripted
//! transcript driven against the shipped binary
//! launched exactly that way (a headless VS Code run
//! is not gate evidence in this sandbox, per the
//! audit; the transcript pins the wire behavior the
//! client depends on); the `.vsix` on disk — it
//! packages, and the manifest inside it contributes
//! the same frozen surface; and the dependency
//! quarantine — the Cargo graph's standing set, the
//! library npm package's standing mass, and the one
//! new dependency confined to `packages/vscode`.

use morphic::api;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn vscode_dir() -> std::path::PathBuf {
    root().join("packages/vscode")
}

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

/// The frozen contributed surface, asserted identically
/// against the source manifest and the manifest inside
/// the .vsix.
fn assert_frozen_surface(manifest: &Value, where_: &str) {
    assert_eq!(manifest["name"], "morphic", "{where_}: the package name");
    assert_eq!(
        manifest["main"], "./out/extension.js",
        "{where_}: the entry point"
    );
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
        "{where_}: contributes is exactly languages + grammars + configuration \
         — no commands, no views, no menus (diagnostics-only)"
    );

    let languages = contributes["languages"].as_array().unwrap();
    assert_eq!(languages.len(), 1, "{where_}: one language");
    assert_eq!(languages[0]["id"], "morphic");
    assert_eq!(languages[0]["extensions"], json!([".morph"]));
    assert_eq!(
        languages[0]["configuration"], "./language-configuration.json",
        "{where_}: the language configuration rides along"
    );

    let grammars = contributes["grammars"].as_array().unwrap();
    assert_eq!(grammars.len(), 1, "{where_}: one grammar");
    assert_eq!(grammars[0]["language"], "morphic");
    assert_eq!(grammars[0]["scopeName"], "source.morph");
    assert_eq!(
        grammars[0]["path"], "./syntaxes/morphic.tmLanguage.json",
        "{where_}: the highlighting grammar's path"
    );

    let properties = &contributes["configuration"]["properties"];
    let prop_keys: Vec<&str> = properties
        .as_object()
        .expect("configuration properties is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        prop_keys,
        ["morphic.serverPath"],
        "{where_}: exactly one configuration key"
    );
    let server_path = &properties["morphic.serverPath"];
    assert_eq!(server_path["type"], "string");
    assert_eq!(
        server_path["default"], "morphic",
        "{where_}: the server path defaults to a PATH lookup of `morphic`"
    );
    assert_eq!(
        server_path["scope"], "machine",
        "{where_}: a server path is a machine fact, not a workspace one"
    );
}

// ── The manifest and its furniture ──────────────────────

#[test]
fn p156_manifest_contributes_exactly_the_frozen_surface() {
    let dir = vscode_dir();
    let manifest = read_json(&dir.join("package.json"));
    assert_frozen_surface(&manifest, "source manifest");

    // The extension's version rides the lockstep: today
    // it is the standing crate version; the 1.5.0 stamp
    // (Phase 157) moves both.
    assert_eq!(
        manifest["version"].as_str().unwrap(),
        crate_version(),
        "the extension version tracks the crate version"
    );

    // The dependency surface, exactly: one runtime
    // dependency (quarantined here), the build furniture
    // as devDependencies, nothing else.
    let deps: Vec<&str> = manifest["dependencies"]
        .as_object()
        .expect("dependencies is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        deps,
        ["vscode-languageclient"],
        "the one real new dependency, and only here"
    );
    let mut dev_deps: Vec<&str> = manifest["devDependencies"]
        .as_object()
        .expect("devDependencies is an object")
        .keys()
        .map(String::as_str)
        .collect();
    dev_deps.sort();
    assert_eq!(
        dev_deps,
        ["@types/node", "@types/vscode", "@vscode/vsce", "typescript"],
        "tsc + vsce are this package's own dev furniture"
    );

    // The language configuration: comment tokens and
    // brackets from the LANGUAGE_REFERENCE's own grammar
    // (// line, /* */ block).
    let lang_config = read_json(&dir.join("language-configuration.json"));
    assert_eq!(lang_config["comments"]["lineComment"], "//");
    assert_eq!(
        lang_config["comments"]["blockComment"],
        json!(["/*", "*/"])
    );
    let brackets = lang_config["brackets"].as_array().unwrap();
    for pair in [("{", "}"), ("[", "]"), ("(", ")")] {
        assert!(
            brackets.contains(&json!([pair.0, pair.1])),
            "the bracket pair {} {} is configured",
            pair.0,
            pair.1
        );
    }

    // The grammar is a highlighting grammar only:
    // declarative, scoped source.morph, with patterns —
    // decoration, carrying no diagnostic claim.
    let grammar = read_json(&dir.join("syntaxes/morphic.tmLanguage.json"));
    assert_eq!(grammar["scopeName"], "source.morph");
    assert!(
        !grammar["patterns"].as_array().unwrap().is_empty(),
        "the grammar has patterns"
    );
    println!("P156|manifest|surface=languages+grammars+configuration(1 key) deps=vscode-languageclient only");
}

// ── The client: thin, and launching the frozen contract ─

#[test]
fn p156_client_is_thin_and_launches_the_frozen_contract() {
    let source = std::fs::read_to_string(vscode_dir().join("src/extension.ts"))
        .expect("the client source reads");

    // The launch contract, at the client's own source:
    // the server path comes from the one configuration
    // key, the server runs with the single argument
    // `lsp`, over stdio, for morphic documents.
    for needle in [
        "vscode-languageclient/node",
        "getConfiguration('morphic')",
        "'serverPath'",
        "'lsp'",
        "TransportKind.stdio",
        "language: 'morphic'",
    ] {
        assert!(
            source.contains(needle),
            "the client source carries {needle:?}"
        );
    }
    // Diagnostics-only at the client too: it registers
    // no feature provider of its own.
    for absent in [
        "HoverProvider",
        "CompletionItemProvider",
        "DefinitionProvider",
        "registerHover",
        "registerCompletionItem",
        "registerDefinition",
    ] {
        assert!(
            !source.contains(absent),
            "the client registers no {absent} — diagnostics come from the server"
        );
    }
    println!("P156|client|launch=<serverPath> lsp over stdio, selector=morphic, providers=none");
}

// ── The smoke: the Phase 155 transcript, launched the
//    way the extension launches it ──────────────────────

const CLEAN: &str = "Clean {\n    spec plus_one {\n        input: x: Int\n        output: Int\n        constraint: output == x + 1\n        test \"one\" { input: 0 expect: 1 }\n    }\n}";
const PARSE_BROKEN: &str =
    "ParseBroken {\n    spec f {\n        input: a: Int\n        output: Int\n        constraint: output == a +\n    }\n}";
const TYPE_ERR: &str =
    "Typed {\n    spec f {\n        input: a: Int\n        output: Int\n        constraint: output == a ++ 2\n    }\n}";

const CLEAN_URI: &str = "file:///clean.morph";
const BROKEN_URI: &str = "file:///broken.morph";

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

/// Spawn `<server_path> lsp` — the client's launch line,
/// with the server path standing where a user's
/// `morphic.serverPath` (or PATH-resolved `morphic`)
/// stands — write the script, close stdin, collect.
fn run_client_launch(server_path: &str, script: Vec<Vec<u8>>) -> (Vec<u8>, std::process::ExitStatus) {
    let mut child: Child = Command::new(server_path)
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the server spawns the way the client spawns it");
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
fn p156_launch_contract_transcript_smoke() {
    // The extension's launch line, with serverPath
    // pointed at the shipped binary — the same position
    // a user's configured path occupies.
    let server_path = env!("CARGO_BIN_EXE_morphic");
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

    let (bytes, status) = run_client_launch(server_path, script);
    assert!(status.success(), "shutdown then exit → status 0: {status}");
    assert!(
        bytes.starts_with(b"Content-Length: "),
        "the stream opens with framing, not a banner"
    );

    let frames = parse_frames(&bytes);
    assert_eq!(frames.len(), 7, "one answer per step of the script: {frames:?}");
    assert_eq!(frames[0]["result"]["capabilities"]["textDocumentSync"]["change"], 1);
    assert_eq!(frames[1]["params"]["diagnostics"], json!([]));

    let parse_error = api::check(PARSE_BROKEN).error.unwrap();
    let diagnostics = frames[2]["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["range"]["start"], json!({"line": 5, "character": 4}));
    assert_eq!(diagnostics[0]["message"], parse_error.message);

    let type_error = api::check(TYPE_ERR).error.unwrap();
    let diagnostics = frames[3]["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["range"]["start"], json!({"line": 0, "character": 0}));
    assert_eq!(
        diagnostics[0]["message"],
        format!("unpositioned: {}", type_error.message)
    );

    assert_eq!(frames[4]["params"]["diagnostics"], json!([]));
    assert_eq!(frames[5]["params"]["diagnostics"], json!([]));
    assert_eq!(frames[6]["result"], Value::Null);
    println!("P156|smoke|launch=<serverPath> lsp frames=7 parse@(5,4) type@0:0-unpositioned exit=0");
}

// ── The .vsix ───────────────────────────────────────────

#[test]
fn p156_vsix_packages_and_contains_the_frozen_surface() {
    let dir = vscode_dir();
    let manifest = read_json(&dir.join("package.json"));
    let version = manifest["version"].as_str().unwrap();
    let vsix = dir.join(format!("morphic-{version}.vsix"));
    assert!(
        vsix.exists(),
        "the packaged extension stands at {}",
        vsix.display()
    );

    let listing = Command::new("unzip")
        .arg("-Z1")
        .arg(&vsix)
        .output()
        .expect("unzip lists the .vsix");
    assert!(listing.status.success(), "the .vsix is a readable zip");
    let listing = String::from_utf8(listing.stdout).unwrap();
    for entry in [
        "extension/package.json",
        "extension/out/extension.js",
        "extension/language-configuration.json",
        "extension/syntaxes/morphic.tmLanguage.json",
        "extension/node_modules/vscode-languageclient/package.json",
    ] {
        assert!(
            listing.lines().any(|line| line == entry),
            "the .vsix contains {entry}"
        );
    }

    let contained = Command::new("unzip")
        .arg("-p")
        .arg(&vsix)
        .arg("extension/package.json")
        .output()
        .expect("unzip reads the contained manifest");
    assert!(contained.status.success());
    let contained: Value = serde_json::from_slice(&contained.stdout)
        .expect("the contained manifest parses");
    assert_frozen_surface(&contained, "contained manifest");
    println!("P156|vsix|morphic-{version}.vsix contained-manifest=frozen-surface client-bundled=yes");
}

// ── The quarantine ──────────────────────────────────────

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
fn p156_dependency_quarantine_holds() {
    let root = root();

    // The Cargo graph gains nothing: the default build's
    // dependency set is the standing set (Phase 151's pin,
    // re-asserted here because §6's quarantine names it).
    let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
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
            "serde", "serde_json", "thiserror", "tokio", "z3",
        ],
        "the Cargo dependency set is the standing set"
    );

    // The library's npm package gains nothing: its file
    // set and mass stand where Phase 151 measured them.
    let (npm_files, npm_bytes) = dir_mass(&root.join("packages/npm"));
    assert_eq!(npm_files, 77, "the library package's file set unmoved");
    assert_eq!(npm_bytes, 71_873, "the library package's mass unmoved");

    // The extension's dependency world lives only in
    // packages/vscode: its lockfile stands there (and
    // names the quarantined dependency), and no Node
    // furniture leaks to the root or the sibling
    // packages.
    let lock = read_json(&vscode_dir().join("package-lock.json"));
    assert_eq!(lock["name"], "morphic");
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
    println!("P156|quarantine|cargo=standing-12 npm=77-files/71873B lockfile=packages/vscode only");
}
