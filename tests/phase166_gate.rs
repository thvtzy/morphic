//! Phase 166 — the 1.7 gate (DISEBAR_17_SCOPE §10),
//! run clause by clause on the gate tree with fresh
//! instruments, in the Phase 151/157/161 gate shape:
//! nothing here quotes Phases 163–165 — the stranger
//! installs, the tamper battery, the collision pins
//! and the catalog sweep below are this file's own,
//! driven against the shipped binary and the repo's
//! own `registry/` catalog.
//!
//! The gate text (§10, never amended):
//! 1. **The stranger test.** From an empty install
//!    dir, with the shipped binary: a user-module
//!    bundle (the widget exhibit) is fetched from the
//!    catalog, installed, and then called **by name**
//!    from a fresh spec — which checks against the
//!    merged prelude, is judged through the index (the
//!    installed body is the name's denotation), and
//!    generates code that computes battery-exact.
//! 2. **The install is the verification.** Each
//!    tamper shape — flipped verdict, moved battery
//!    count, swapped body, sha256 mismatch against
//!    the index, unsupported bundle version — is
//!    refused by name at install with **nothing
//!    written**; a Tier-1 bundle in the no-Z3 build is
//!    refused by name; a stdlib-named bundle is
//!    refused by name even though its claim
//!    reproduces.
//! 3. **Collision discipline.** An installed entry
//!    never shadows a stdlib name (a hand-planted file
//!    is equally inert); a user's own definition
//!    shadows an installed entry; `registry remove`
//!    restores the pre-install standings exactly —
//!    the name declines again.
//! 4. **The catalog is consistent.** Every index
//!    entry's file exists, hashes to the row's
//!    sha256, and re-verifies with the shipped binary
//!    (the CI check of §9, run here as a gate
//!    instrument over the repo's own `registry/`);
//!    every receipt's recorded standing equals the
//!    standing its install re-measured.
//! 5. **Footprint + suites.** Index + catalog masses
//!    inside §9's figures; `sha2` the milestone's only
//!    new dependency, absent from the browser
//!    profile; playground bundle within 771,072 B;
//!    suites green in the three frozen configurations.
//!    (The suites and the rebuilt playground mass are
//!    the gate record's evidence in
//!    ROAD_TO_SELF_HOSTING.md, as at Phases 157/161;
//!    this file pins the masses, the dependency set
//!    and the lockstep it can judge in-process.)
//!
//! Configuration postures (the standing ones, stated
//! here so the gate's readings are auditable): in a
//! no-Z3 build a Tier-1 claim stands *unexamined* —
//! refused by name at install and at verify-bundle —
//! so clause 1's install legs run on the Tier-2
//! exhibit there, clause 2's stdlib shape is refused
//! at the re-verification step (by name) before the
//! collision step can be reached, and clause 4's
//! sweep expects the unexamined refusal for Tier-1
//! rows. The in-process discovery state is the same
//! bytes in every build: installed through the verbs
//! where the build's verbs can install, planted
//! byte-exactly as an install writes it where they
//! cannot (the Phase 165 setup discipline) — plus
//! one hand-planted, body-swapped stdlib bundle for
//! clause 3, whose inertness can only come from the
//! funnel's layer order.
//!
//! Every crown-bearing pin in this binary serializes
//! on one lock (the Phase 150 discipline, extended to
//! the subprocess crowns: the box is small and a
//! flipped crown is worthless evidence).

#![cfg(not(feature = "wasm-playground"))]

use std::io::{Read, Write};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};

use morphic::spec::ast::FunctionSpec;
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{IRNode, SpecIndex};
use morphic::synthesis::eval;
use morphic::{api, pcc, registry, stdlib};

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase166-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ── The catalog, copied hermetically ────────────────────

/// Copy the repo's own catalog into a scratch root:
/// `<dir>/registry/index.json` + the bundle files at
/// their repo-relative paths (R1 resolution reads the
/// copy exactly as it reads the repo's).
fn scratch_catalog(tag: &str) -> std::path::PathBuf {
    let root = scratch_dir(&format!("{tag}-catalog"));
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("registry/index.json")).unwrap(),
    )
    .unwrap();
    let dst = root.join("registry");
    std::fs::create_dir_all(dst.join("bundles")).unwrap();
    std::fs::copy(
        repo_root().join("registry/index.json"),
        dst.join("index.json"),
    )
    .unwrap();
    for row in index["entries"].as_array().unwrap() {
        let rel = row["file"].as_str().unwrap();
        std::fs::copy(repo_root().join(rel), root.join(rel)).unwrap();
    }
    root
}

fn catalog_index(catalog_root: &std::path::Path) -> String {
    catalog_root
        .join("registry/index.json")
        .to_string_lossy()
        .into_owned()
}

/// A scratch catalog with one bundle doctored. Unless
/// `rebind` is false, the index row is re-bound to the
/// doctored bytes (sha256 + bytes), so the refusal the
/// pin hears is the re-verification's, not the hash's.
fn doctored_catalog(
    tag: &str,
    entry: &str,
    rebind: bool,
    doctor: impl Fn(&mut serde_json::Value),
) -> std::path::PathBuf {
    let root = scratch_catalog(tag);
    let bundle_path = root
        .join("registry")
        .join("bundles")
        .join(format!("{entry}.bundle.json"));
    let mut bundle: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&bundle_path).unwrap()).unwrap();
    doctor(&mut bundle);
    let text = serde_json::to_string_pretty(&bundle).unwrap();
    std::fs::write(&bundle_path, &text).unwrap();
    if rebind {
        let index_path = root.join("registry").join("index.json");
        let mut index: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&index_path).unwrap()).unwrap();
        let bytes = std::fs::read(&bundle_path).unwrap();
        for row in index["entries"].as_array_mut().unwrap() {
            if row["entry"].as_str() == Some(entry) {
                row["sha256"] = serde_json::json!(registry::sha256_hex(&bytes));
                row["bytes"] = serde_json::json!(bytes.len());
            }
        }
        std::fs::write(&index_path, serde_json::to_string_pretty(&index).unwrap()).unwrap();
    }
    root
}

fn catalog_bundle(name: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(
        repo_root()
            .join("registry")
            .join("bundles")
            .join(format!("{name}.bundle.json")),
    )
    .unwrap();
    serde_json::from_str(&text).expect("the catalog bundle parses")
}

// ── The shipped binary, driven as a stranger drives it ──

fn run_cli(args: &[&str], install_dir: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args(args)
        .env("MORPHIC_REGISTRY_DIR", install_dir)
        .output()
        .expect("run the morphic binary")
}

fn run_registry(args: &[&str], install_dir: &std::path::Path) -> Output {
    let mut full: Vec<&str> = vec!["registry"];
    full.extend_from_slice(args);
    run_cli(&full, install_dir)
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The install dir's ledger, parsed (an absent ledger
/// reads as no rows).
fn ledger_rows(install_dir: &std::path::Path) -> Vec<serde_json::Value> {
    match std::fs::read_to_string(install_dir.join("installed.json")) {
        Ok(text) => {
            let ledger: serde_json::Value = serde_json::from_str(&text).unwrap();
            ledger["entries"].as_array().cloned().unwrap_or_default()
        }
        Err(_) => Vec::new(),
    }
}

/// Clause 2's standing assertion: a refused install
/// wrote nothing — no bundle file for the entry, no
/// ledger row naming it.
fn assert_nothing_written(install_dir: &std::path::Path, entry: &str) {
    assert!(
        !install_dir.join(format!("{entry}.bundle.json")).exists(),
        "a refused install wrote no bundle file for '{entry}'"
    );
    assert!(
        ledger_rows(install_dir)
            .iter()
            .all(|r| r["entry"].as_str() != Some(entry)),
        "a refused install wrote no receipt for '{entry}'"
    );
}

/// A receipt records what the install's re-run
/// measured (§5.3): field for field, the installed
/// bundle's own claim — and the claim reproduces
/// under the per-bundle verb in this build.
fn assert_receipt_is_the_reproduction(install_dir: &std::path::Path, entry: &str) {
    let rows = ledger_rows(install_dir);
    let receipt = rows
        .iter()
        .find(|r| r["entry"].as_str() == Some(entry))
        .unwrap_or_else(|| panic!("a receipt row for '{entry}'"));
    let file_bytes =
        std::fs::read(install_dir.join(format!("{entry}.bundle.json"))).unwrap();
    let bundle: serde_json::Value = serde_json::from_slice(&file_bytes).unwrap();
    assert_eq!(receipt["version"], "1.7.0", "the catalog version installed from");
    assert_eq!(
        receipt["sha256"].as_str().unwrap(),
        registry::sha256_hex(&file_bytes),
        "the receipt's hash is the installed file's"
    );
    for field in ["tier", "verdictBasis", "verdict", "crownRoute"] {
        assert_eq!(
            receipt[field], bundle[field],
            "the receipt's {field} is the reproduced standing, not a copied claim"
        );
    }
    assert_eq!(
        receipt["battery"], bundle["battery"],
        "the receipt's battery is the reproduced battery"
    );
    let text = String::from_utf8(file_bytes).unwrap();
    assert!(
        pcc::verify_bundle_text(&text).is_ok(),
        "the installed bundle's claim reproduces in this build"
    );
}

// ── The gate's own spec modules ─────────────────────────

/// A fresh spec calling both exhibits by name. The
/// `known_code` rows restate the exhibit's own battery
/// (1/6 known; 0/7/99 not) through the call, so the
/// compiled route below computes the battery exactly.
const CALLER_MODULE: &str = r#"GateCaller {
spec known_code {
    doc: "known_code(code) = the installed is_widget_code, called by name."
    input: code: Int
    output: Bool
    constraint: output == is_widget_code(code)
    test "first kind" { input: 1 expect: true }
    test "last kind" { input: 6 expect: true }
    test "zero" { input: 0 expect: false }
    test "above" { input: 7 expect: false }
    test "far above" { input: 99 expect: false }
}
spec sorted_weights {
    doc: "sorted_weights(xs) = the installed weights_sorted, called by name."
    input: xs: List<Int>
    output: Bool
    constraint: output == weights_sorted(xs)
    test "sorted" { input: [1, 2, 2]; expect: true }
    test "unsorted" { input: [3, 1]; expect: false }
    test "empty" { input: []; expect: true }
}
}"#;

/// A caller naming only the Tier-2 exhibit — the
/// module a no-Z3 stranger can check end-to-end.
const WEIGHTS_CALLER_MODULE: &str = r#"GateWeights {
spec sorted_weights {
    doc: "sorted_weights(xs) = the installed weights_sorted, called by name."
    input: xs: List<Int>
    output: Bool
    constraint: output == weights_sorted(xs)
    test "sorted" { input: [1, 2, 2]; expect: true }
    test "unsorted" { input: [3, 1]; expect: false }
    test "empty" { input: []; expect: true }
}
}"#;

/// A caller of the stdlib's `int_abs` — clause 3's
/// behavioural probe: judged against these rows, the
/// stdlib body passes and the planted body (the
/// sign function's) does not.
const ABS_CALLER_MODULE: &str = r#"GateAbs {
spec abs_probe {
    doc: "abs_probe(x) = int_abs(x), whoever the prelude says that is."
    input: x: Int
    output: Int
    constraint: output == int_abs(x)
    test "negative five" { input: (0 - 5) expect: 5 }
    test "negative one" { input: (0 - 1) expect: 1 }
    test "positive" { input: 4 expect: 4 }
}
}"#;

/// The user's own `is_widget_code`, shadowing the
/// installed entry (clause 3's second half).
const SHADOW_MODULE: &str = r#"GateShadow {
spec is_widget_code {
    doc: "The gate's own user definition: a code is known only when it is 42."
    input: code: Int
    output: Bool
    constraint: output == (code == 42)
    test "the answer" { input: 42 expect: true }
    test "three" { input: 3 expect: false }
}
spec shadow_probe {
    doc: "shadow_probe(code) = is_widget_code(code), whoever defines it."
    input: code: Int
    output: Bool
    constraint: output == is_widget_code(code)
    test "the answer" { input: 42 expect: true }
    test "three" { input: 3 expect: false }
}
spec installed_semantics_probe {
    doc: "The installed entry's semantics, stated as a row: 3 is a known code."
    input: code: Int
    output: Bool
    constraint: output == is_widget_code(code)
    test "three" { input: 3 expect: true }
}
}"#;

fn spec_named<'a>(specs: &'a [FunctionSpec], name: &str) -> &'a FunctionSpec {
    specs
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("{name} is a spec"))
}

fn call_body(name: &str, arg: &str) -> IRNode {
    IRNode::Call {
        function: name.to_string(),
        args: vec![IRNode::Var(arg.to_string())],
    }
}

fn candidate(spec: &FunctionSpec, body: IRNode) -> morphic::synthesis::engine::CandidateImplementation {
    morphic::synthesis::engine::CandidateImplementation {
        id: 166,
        body,
        spec_name: spec.name.clone(),
        spec: spec.clone(),
        score: 0.0,
        scores: morphic::synthesis::engine::ScoreBreakdown {
            test_pass_ratio: 0.0,
            constraint_score: 0.0,
            complexity_score: 0.0,
            quality_score: 0.0,
            synthesis_time_us: 0,
        },
        generation: 0,
        provenance: morphic::synthesis::engine::Provenance::HumanGuided,
        verified: false,
        verification_status: None,
    }
}

// ── The in-process discovery state (clauses 1 + 3) ──────

fn gate_state_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("morphic-phase166-state-{}", std::process::id()))
}

/// Plant one entry's installed state byte-exactly as
/// a successful install writes it: the bundle file
/// (optionally with a doctored body) and a receipt row
/// recording the bundle's own standings.
fn plant_row(dir: &std::path::Path, name: &str, body_override: Option<String>) -> serde_json::Value {
    let mut bundle = catalog_bundle(name);
    if let Some(body) = body_override {
        bundle["body"] = serde_json::json!(body);
    }
    let text = serde_json::to_string_pretty(&bundle).unwrap();
    let bytes = text.into_bytes();
    std::fs::write(dir.join(format!("{name}.bundle.json")), &bytes).unwrap();
    serde_json::json!({
        "entry": name,
        "version": "1.7.0",
        "sha256": registry::sha256_hex(&bytes),
        "tier": bundle["tier"],
        "verdictBasis": bundle["verdictBasis"],
        "verdict": bundle["verdict"],
        "crownRoute": bundle["crownRoute"],
        "battery": bundle["battery"],
        "index": "planted by the Phase 166 gate instrument",
        "installed": "2026-11-26",
    })
}

/// Build this process's one discovery state, then
/// point the process at it. Every pin in this binary
/// calls `init()` first: the funnel reads the install
/// directory once per process, and no read may
/// precede the pointer being set.
fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let dir = gate_state_dir();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let index = repo_root().join("registry").join("index.json");
        let index_arg = index.to_string_lossy().into_owned();
        let mut planted: Vec<serde_json::Value> = Vec::new();
        if cfg!(feature = "z3-support") {
            // Through the verbs, against the repo's own
            // catalog: the genuine stranger route.
            for name in ["is_widget_code", "weights_sorted"] {
                let out = run_registry(&["add", name, "--index", &index_arg], &dir);
                assert!(
                    out.status.success(),
                    "the gate state's install of {name}: {}",
                    stderr_of(&out)
                );
            }
        } else {
            // The Tier-2 entry installs through the
            // verbs on executed evidence; the Tier-1
            // entry's verb refuses unexamined in this
            // build, so it is planted byte-exactly as a
            // Z3 install writes it.
            let out = run_registry(&["add", "weights_sorted", "--index", &index_arg], &dir);
            assert!(
                out.status.success(),
                "the gate state's install of weights_sorted: {}",
                stderr_of(&out)
            );
            planted.push(plant_row(&dir, "is_widget_code", None));
        }
        // Clause 3's plant: the catalog's own `int_abs`
        // bundle with its body swapped for `int_sign`'s
        // — file and receipt consistent, claim intact.
        // Only the funnel's layer order can make this
        // inert.
        let sign_body = catalog_bundle("int_sign")["body"]
            .as_str()
            .unwrap()
            .to_string();
        planted.push(plant_row(&dir, "int_abs", Some(sign_body)));
        if !planted.is_empty() {
            let mut rows = ledger_rows(&dir);
            rows.extend(planted);
            rows.sort_by(|a, b| a["entry"].as_str().cmp(&b["entry"].as_str()));
            let ledger = serde_json::json!({
                "format": registry::LEDGER_FORMAT,
                "entries": rows,
            });
            std::fs::write(
                dir.join("installed.json"),
                serde_json::to_string_pretty(&ledger).unwrap(),
            )
            .unwrap();
        }
        std::env::set_var("MORPHIC_REGISTRY_DIR", &dir);
    });
}

// ── Clause 1: the stranger test ─────────────────────────

#[test]
fn p166_clause1_stranger_installs_from_the_catalog_and_the_name_checks() {
    init();
    let _serial = crown_serial();
    let catalog = scratch_catalog("c1");
    let index = catalog_index(&catalog);
    let dir = scratch_dir("c1-install");
    let _ = std::fs::remove_dir_all(&dir); // the stranger starts from nothing

    if cfg!(feature = "z3-support") {
        // The centrepiece: the Tier-1 user-module
        // bundle, fetched from the catalog into an
        // empty install dir.
        let out = run_registry(&["add", "is_widget_code", "--index", &index], &dir);
        assert!(
            out.status.success(),
            "the stranger install succeeds: {}",
            stderr_of(&out)
        );
        let line = stdout_of(&out);
        assert!(
            line.contains("tier-1") && line.contains("z3-verified") && line.contains("Verified"),
            "the install reports the measured standing: {line}"
        );
        assert_receipt_is_the_reproduction(&dir, "is_widget_code");

        // The Tier-2 exhibit installs beside it, on
        // executed evidence, through the same door.
        let out = run_registry(&["add", "weights_sorted", "--index", &index], &dir);
        assert!(
            out.status.success(),
            "the Tier-2 stranger install succeeds: {}",
            stderr_of(&out)
        );
        assert_receipt_is_the_reproduction(&dir, "weights_sorted");

        // The verbs around it: list shows the measured
        // standing; verify, named and swept, passes.
        let out = run_registry(&["list"], &dir);
        assert!(out.status.success());
        assert!(
            stdout_of(&out).contains("is_widget_code"),
            "list shows the installed entry"
        );
        let out = run_registry(&["verify", "is_widget_code"], &dir);
        assert!(
            out.status.success(),
            "verify names the reproduction: {}",
            stderr_of(&out)
        );
        let out = run_registry(&["verify"], &dir);
        assert!(out.status.success(), "the sweep passes: {}", stderr_of(&out));
    } else {
        // This build cannot re-judge a Z3 verdict: the
        // Tier-1 stranger install is refused by name,
        // and the Tier-2 exhibit carries the flow.
        let out = run_registry(&["add", "is_widget_code", "--index", &index], &dir);
        assert!(!out.status.success(), "the Tier-1 install refuses in a no-Z3 build");
        assert!(
            stderr_of(&out).contains("unexamined"),
            "the refusal names the standing: {}",
            stderr_of(&out)
        );
        assert_nothing_written(&dir, "is_widget_code");

        let out = run_registry(&["add", "weights_sorted", "--index", &index], &dir);
        assert!(
            out.status.success(),
            "the Tier-2 install succeeds on executed evidence: {}",
            stderr_of(&out)
        );
        assert_receipt_is_the_reproduction(&dir, "weights_sorted");
        let out = run_registry(&["verify"], &dir);
        assert!(out.status.success(), "the sweep passes: {}", stderr_of(&out));
    }

    // Called by name from a fresh spec: the shipped
    // binary's own check, against the merged prelude
    // this install state produces. (The module checked
    // is the one whose every name this build installed
    // above.)
    let caller_path = catalog.join("caller.morph");
    let caller_text = if cfg!(feature = "z3-support") {
        CALLER_MODULE
    } else {
        WEIGHTS_CALLER_MODULE
    };
    std::fs::write(&caller_path, caller_text).unwrap();
    let caller_arg = caller_path.to_string_lossy().into_owned();
    let out = run_cli(&["check", &caller_arg], &dir);
    assert!(
        out.status.success(),
        "a fresh spec calling the installed entry checks: {}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).contains("Specification is valid."),
        "the check answers valid"
    );
}

/// Clause 1's second fetch path: the same catalog
/// served over HTTP from a hermetic localhost server —
/// the canonical URL's shape at bench scale. A build
/// without the HTTP client declines by name instead
/// (the §5.5 posture).
#[test]
fn p166_clause1_the_url_fetch_path_installs() {
    init();
    let _serial = crown_serial();
    let catalog = scratch_catalog("c1url");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let server = {
        let stop = Arc::clone(&stop);
        let root = catalog.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                        let mut buf = [0u8; 4096];
                        let n = stream.read(&mut buf).unwrap_or(0);
                        let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                        let path = request
                            .lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .unwrap_or("")
                            .trim_start_matches('/');
                        match std::fs::read(root.join(path)) {
                            Ok(bytes) => {
                                let head = format!(
                                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                                    bytes.len()
                                );
                                let _ = stream.write_all(head.as_bytes());
                                let _ = stream.write_all(&bytes);
                            }
                            Err(_) => {
                                let _ = stream.write_all(
                                    b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                                );
                            }
                        }
                        let _ = stream.flush();
                        let _ = stream.shutdown(std::net::Shutdown::Write);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        })
    };

    let dir = scratch_dir("c1url-install");
    let _ = std::fs::remove_dir_all(&dir);
    let url = format!("http://127.0.0.1:{port}/registry/index.json");
    let out = run_registry(&["add", "weights_sorted", "--index", &url], &dir);
    if cfg!(feature = "llm-client") {
        assert!(
            out.status.success(),
            "the URL install succeeds: {}",
            stderr_of(&out)
        );
        let rows = ledger_rows(&dir);
        let receipt = rows
            .iter()
            .find(|r| r["entry"].as_str() == Some("weights_sorted"))
            .expect("a receipt for the URL install");
        assert_eq!(
            receipt["index"].as_str().unwrap(),
            url,
            "the receipt names the index it resolved against"
        );
        assert_receipt_is_the_reproduction(&dir, "weights_sorted");
    } else {
        assert!(!out.status.success(), "a build with no HTTP client declines the fetch");
        assert!(
            stderr_of(&out).contains("no HTTP client"),
            "the decline names its reason: {}",
            stderr_of(&out)
        );
        assert_nothing_written(&dir, "weights_sorted");
    }
    stop.store(true, Ordering::SeqCst);
    let _ = server.join();
}

#[test]
fn p166_clause1_the_caller_judges_through_the_index_and_generated_code_computes() {
    init();
    let _serial = crown_serial();
    let module = parser::parse(CALLER_MODULE).expect("the caller module parses");
    let checked = typeck::check(module.clone());
    assert!(
        checked.is_ok(),
        "calls to the installed entries typecheck against the merged prelude: {:?}",
        checked.err()
    );
    let index = SpecIndex::from_module(&module);

    // The installed body is the name's denotation.
    let caller = spec_named(&module.functions, "known_code");
    let denotation = index
        .callee_body("is_widget_code")
        .expect("the installed entry denotes its body in the index");
    let want = morphic::synthesis::irtext::parse_ir(
        catalog_bundle("is_widget_code")["body"].as_str().unwrap(),
    )
    .expect("the catalog body reads");
    assert_eq!(
        format!("{denotation:?}"),
        format!("{want:?}"),
        "the denotation is the installed body itself"
    );

    // Judged through the index: the call inlines the
    // installed body, and the caller's rows — the
    // exhibit's battery restated through the call —
    // judge exact.
    let inlined = index
        .inline_calls(&call_body("is_widget_code", "code"))
        .expect("the call inlines the installed body");
    assert_eq!(format!("{inlined:?}"), format!("{want:?}"));
    let run = eval::run_spec_tests_in(&inlined, caller, &index);
    assert_eq!(
        run,
        eval::TestRun { evaluable: 5, passed: 5 },
        "the caller judges battery-exact through the installed body"
    );

    // The Tier-2 entry judges the same way, by its
    // route: the denotation itself against the rows
    // (its statement body declines the expression
    // inliner — the standing Tier-2 boundary).
    let weights_caller = spec_named(&module.functions, "sorted_weights");
    let weights_body = index
        .callee_body("weights_sorted")
        .expect("the installed Tier-2 entry denotes its body");
    let run = eval::run_spec_tests_in(&weights_body, weights_caller, &index);
    assert_eq!(
        run,
        eval::TestRun { evaluable: 3, passed: 3 },
        "the Tier-2 caller judges battery-exact through the installed body"
    );

    // Generated code computes battery-exact: the
    // caller's rendered Rust carries the installed
    // body as a function, compiles standalone, and
    // its own test module — the caller's five rows —
    // passes under rustc's harness.
    let rust = api::render(&candidate(caller, call_body("is_widget_code", "code")), "rust")
        .expect("the caller renders to Rust");
    assert!(
        rust.contains("pub fn is_widget_code"),
        "the emitted Rust carries the installed entry as a function:\n{rust}"
    );
    assert!(
        rust.contains(">= 1") && rust.contains("<= 6"),
        "the emitted Rust carries the installed body's bounds:\n{rust}"
    );
    let dir = scratch_dir("c1-compiled");
    let src_path = dir.join("caller.rs");
    std::fs::write(&src_path, &rust).unwrap();
    let bin_path = dir.join("caller_test");
    let compile = Command::new("rustc")
        .args([
            "--edition",
            "2021",
            "--test",
            src_path.to_str().unwrap(),
            "-o",
            bin_path.to_str().unwrap(),
        ])
        .output()
        .expect("rustc runs");
    assert!(
        compile.status.success(),
        "the generated code compiles standalone:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = Command::new(&bin_path).output().expect("the compiled battery runs");
    let stdout = stdout_of(&run);
    assert!(
        run.status.success() && stdout.contains("5 passed"),
        "the generated code computes the battery exactly:\n{stdout}"
    );
}

// ── Clause 2: the install is the verification ───────────

#[test]
fn p166_clause2_the_tamper_battery_refuses_with_nothing_written() {
    init();
    let _serial = crown_serial();
    // The exhibit whose route this build judges: the
    // Tier-1 entry under Z3, the Tier-2 entry without.
    let (exhibit, other) = if cfg!(feature = "z3-support") {
        ("is_widget_code", "weights_sorted")
    } else {
        ("weights_sorted", "is_widget_code")
    };
    let other_body = catalog_bundle(other)["body"].as_str().unwrap().to_string();

    let mut refusals: Vec<String> = Vec::new();
    let mut shape = |tag: &str,
                     catalog: std::path::PathBuf,
                     entry: &str,
                     marker: &str|
     -> String {
        let dir = scratch_dir(&format!("c2-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        let out = run_registry(
            &["add", entry, "--index", &catalog_index(&catalog)],
            &dir,
        );
        assert!(
            !out.status.success(),
            "the {tag} shape refuses at install"
        );
        let err = stderr_of(&out);
        assert!(
            err.contains(marker),
            "the {tag} refusal names its shape ('{marker}'):\n{err}"
        );
        assert_nothing_written(&dir, entry);
        err
    };

    // sha256 mismatch against the index: the bundle
    // bytes edited, the row's hash left standing.
    refusals.push(shape(
        "sha",
        doctored_catalog("c2-sha", exhibit, false, |b| {
            b["spec"]["doc"] = serde_json::json!("A tampered doc line.");
        }),
        exhibit,
        "sha256 mismatch",
    ));
    // Flipped verdict: the claim triple no longer
    // coheres.
    refusals.push(shape(
        "verdict",
        doctored_catalog("c2-verdict", exhibit, true, |b| {
            b["verdict"] = serde_json::json!(if exhibit == "is_widget_code" {
                "ExecutedExact"
            } else {
                "Verified"
            });
        }),
        exhibit,
        "incoherent claim",
    ));
    // Moved battery count: the claim the route does
    // not reproduce.
    refusals.push(shape(
        "battery",
        doctored_catalog("c2-battery", exhibit, true, |b| {
            b["battery"]["passed"] = serde_json::json!(4);
        }),
        exhibit,
        "battery count moved",
    ));
    // Swapped body: another entry's body under this
    // entry's claim. Each route names it in its own
    // words — the pipeline route crowns a different
    // body than the bundle carries; the executed
    // route finds the body not executed-exact.
    refusals.push(shape(
        "body",
        doctored_catalog("c2-body", exhibit, true, |b| {
            b["body"] = serde_json::json!(other_body);
        }),
        exhibit,
        if cfg!(feature = "z3-support") {
            "crowns a different body"
        } else {
            "not executed-exact"
        },
    ));
    // Unsupported bundle version.
    refusals.push(shape(
        "version",
        doctored_catalog("c2-version", exhibit, true, |b| {
            b["bundleVersion"] = serde_json::json!(2);
        }),
        exhibit,
        "unsupported bundle version 2",
    ));
    // The five refusals are pairwise distinct — each
    // shape is refused by its own name.
    let mut distinct = refusals.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        refusals.len(),
        "the tamper refusals are pairwise distinct: {refusals:?}"
    );

    // A stdlib-named bundle, from the genuine catalog
    // — its claim is the shipped one. Under Z3 the
    // refusal is the collision step's; without Z3 the
    // Tier-1 claim is refused unexamined one step
    // earlier. Either way: refused by name, nothing
    // written.
    let catalog = scratch_catalog("c2-stdlib");
    let dir = scratch_dir("c2-stdlib-install");
    let _ = std::fs::remove_dir_all(&dir);
    let out = run_registry(
        &["add", "int_abs", "--index", &catalog_index(&catalog)],
        &dir,
    );
    assert!(!out.status.success(), "a stdlib-named bundle refuses at install");
    let err = stderr_of(&out);
    if cfg!(feature = "z3-support") {
        assert!(
            err.contains("standard-library entry"),
            "the collision refusal names itself:\n{err}"
        );
    } else {
        assert!(
            err.contains("unexamined"),
            "the Tier-1 refusal names itself:\n{err}"
        );
    }
    assert_nothing_written(&dir, "int_abs");

    // The no-Z3 Tier-1 refusal, stated as its own
    // shape (in a Z3 build the Tier-1 exhibit above
    // already carried the battery).
    if !cfg!(feature = "z3-support") {
        let catalog = scratch_catalog("c2-tier1");
        let dir = scratch_dir("c2-tier1-install");
        let _ = std::fs::remove_dir_all(&dir);
        let out = run_registry(
            &["add", "is_widget_code", "--index", &catalog_index(&catalog)],
            &dir,
        );
        assert!(!out.status.success(), "a Tier-1 bundle refuses in a no-Z3 build");
        assert!(
            stderr_of(&out).contains("unexamined"),
            "the refusal names the standing: {}",
            stderr_of(&out)
        );
        assert_nothing_written(&dir, "is_widget_code");
    }
}

// ── Clause 3: collision discipline ──────────────────────

#[test]
fn p166_clause3_the_planted_stdlib_name_is_inert_and_the_user_shadows() {
    init();
    let _serial = crown_serial();

    // The funnel's shape: the stdlib's 35, then the
    // installed pair — and exactly one `int_abs`, the
    // stdlib's, despite the planted bundle in the
    // install dir whose file and receipt agree.
    let specs = stdlib::prelude_specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names.iter().filter(|n| **n == "int_abs").count(),
        1,
        "one int_abs stands in the prelude"
    );
    assert_eq!(
        names.len(),
        37,
        "the prelude is the 35 + the installed pair: {names:?}"
    );
    assert!(
        names[35] == "is_widget_code" && names[36] == "weights_sorted",
        "the installed layer follows the stdlib layer"
    );
    assert!(
        stdlib::installed_diagnostics()
            .iter()
            .any(|line| line.contains("int_abs")),
        "the planted stdlib name's skip is diagnosed by name: {:?}",
        stdlib::installed_diagnostics()
    );

    // Behaviourally: `int_abs` denotes the shipped
    // body — the bundles' evidence copy — not the
    // planted sign body; and a caller judges the
    // stdlib's semantics through it.
    let index = SpecIndex::from_module(&parser::parse(ABS_CALLER_MODULE).unwrap());
    let denotation = index
        .callee_body("int_abs")
        .expect("int_abs denotes a body in the index");
    let shipped = morphic::synthesis::irtext::parse_ir(
        catalog_bundle("int_abs")["body"].as_str().unwrap(),
    )
    .unwrap();
    let planted = morphic::synthesis::irtext::parse_ir(
        catalog_bundle("int_sign")["body"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(
        format!("{denotation:?}"),
        format!("{shipped:?}"),
        "the denotation is the shipped body"
    );
    assert_ne!(
        format!("{denotation:?}"),
        format!("{planted:?}"),
        "the denotation is not the planted body"
    );
    let module = parser::parse(ABS_CALLER_MODULE).unwrap();
    let probe = spec_named(&module.functions, "abs_probe");
    let run = eval::run_spec_tests_in(&denotation, probe, &index);
    assert_eq!(
        run,
        eval::TestRun { evaluable: 3, passed: 3 },
        "the stdlib's semantics judge (the sign body would fail two rows)"
    );

    // The user's own definition shadows the installed
    // entry: the probe judges the user's semantics,
    // and the installed semantics do not judge under
    // the user's name.
    let module = parser::parse(SHADOW_MODULE).expect("the shadow module parses");
    assert!(
        typeck::check(module.clone()).is_ok(),
        "the shadowing module typechecks"
    );
    let index = SpecIndex::from_module(&module);
    let in_index = index.get("is_widget_code").expect("the name resolves");
    assert!(
        in_index
            .doc
            .as_deref()
            .unwrap_or("")
            .contains("user definition"),
        "the index holds the user's spec, not the installed one"
    );
    let inlined = index
        .inline_calls(&call_body("is_widget_code", "code"))
        .expect("the call inlines the index's denotation");
    let probe = spec_named(&module.functions, "shadow_probe");
    let run = eval::run_spec_tests_in(&inlined, probe, &index);
    assert_eq!(
        run,
        eval::TestRun { evaluable: 2, passed: 2 },
        "the user's semantics judge (42 known, 3 not)"
    );
    let installed_probe = spec_named(&module.functions, "installed_semantics_probe");
    let run = eval::run_spec_tests_in(&inlined, installed_probe, &index);
    assert!(
        run.passed < run.evaluable,
        "the installed semantics do NOT judge under the user's definition: {run:?}"
    );
}

#[test]
fn p166_clause3_remove_restores_the_pre_install_standings() {
    init();
    let _serial = crown_serial();
    // The entry this build can install end-to-end.
    let entry = if cfg!(feature = "z3-support") {
        "is_widget_code"
    } else {
        "weights_sorted"
    };
    // A caller module naming only the one entry, so
    // the check's answer turns on that entry alone.
    let single = if entry == "is_widget_code" {
        r#"GateRemove {
spec known_code {
    doc: "known_code(code) = the installed is_widget_code, called by name."
    input: code: Int
    output: Bool
    constraint: output == is_widget_code(code)
    test "first kind" { input: 1 expect: true }
}
}"#
    } else {
        r#"GateRemove {
spec sorted_weights {
    doc: "sorted_weights(xs) = the installed weights_sorted, called by name."
    input: xs: List<Int>
    output: Bool
    constraint: output == weights_sorted(xs)
    test "sorted" { input: [1, 2, 2]; expect: true }
}
}"#
    };
    let catalog = scratch_catalog("c3");
    let scratch = scratch_dir("c3-files");
    let caller_path = scratch.join("caller.morph");
    std::fs::write(&caller_path, single).unwrap();
    let caller_arg = caller_path.to_string_lossy().into_owned();
    let dir = scratch_dir("c3-install");
    let _ = std::fs::remove_dir_all(&dir);

    // Pre-install: the name declines.
    let out = run_cli(&["check", &caller_arg], &dir);
    assert!(!out.status.success(), "before install, the caller does not check");
    assert!(
        stderr_of(&out).contains(entry),
        "the decline names the entry: {}",
        stderr_of(&out)
    );

    // Install: the name checks.
    let out = run_registry(
        &["add", entry, "--index", &catalog_index(&catalog)],
        &dir,
    );
    assert!(out.status.success(), "the install succeeds: {}", stderr_of(&out));
    let out = run_cli(&["check", &caller_arg], &dir);
    assert!(
        out.status.success(),
        "after install, the caller checks: {}",
        stderr_of(&out)
    );

    // Remove: file + receipt gone, the pre-install
    // standings restored exactly — the name declines
    // again, and verify finds nothing to verify.
    let out = run_registry(&["remove", entry], &dir);
    assert!(out.status.success(), "the remove succeeds: {}", stderr_of(&out));
    assert!(
        !dir.join(format!("{entry}.bundle.json")).exists(),
        "the bundle file is gone"
    );
    assert!(
        ledger_rows(&dir).iter().all(|r| r["entry"].as_str() != Some(entry)),
        "the receipt row is gone"
    );
    let out = run_cli(&["check", &caller_arg], &dir);
    assert!(!out.status.success(), "after remove, the caller declines again");
    assert!(
        stderr_of(&out).contains(entry),
        "the decline names the entry again: {}",
        stderr_of(&out)
    );
    let out = run_registry(&["verify", entry], &dir);
    assert!(!out.status.success(), "verify finds nothing installed");
    assert!(
        stderr_of(&out).contains("not installed"),
        "the verify refusal names the standing: {}",
        stderr_of(&out)
    );
}

// ── Clause 4: the catalog is consistent ─────────────────

#[test]
fn p166_clause4_the_catalog_is_consistent_and_receipts_equal_reproductions() {
    init();
    let _serial = crown_serial();
    let index_text =
        std::fs::read_to_string(repo_root().join("registry/index.json")).unwrap();
    let index: serde_json::Value = serde_json::from_str(&index_text).unwrap();
    assert_eq!(index["format"], "morphic-registry-index");
    assert_eq!(index["indexVersion"], 1);
    let rows = index["entries"].as_array().unwrap();
    assert_eq!(rows.len(), 37, "the seed catalog carries 37 entries");
    let names: Vec<&str> = rows.iter().map(|r| r["entry"].as_str().unwrap()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "the rows stand sorted by entry");

    // Every row: the file exists at its repo-relative
    // path, its bytes are the row's bytes, the product
    // hash binds them to the row's sha256, the bundle
    // inside carries the row's entry and labelled
    // claims — and the shipped binary's consumer verb
    // re-verifies it (in a no-Z3 build, a Tier-1 row
    // is refused as unexamined instead: the standing
    // posture, asserted by name).
    for row in rows {
        let entry = row["entry"].as_str().unwrap();
        assert_eq!(
            row["version"], "1.7.0",
            "{entry}: the seed rows carry the cataloguing version (R2)"
        );
        let path = repo_root().join(row["file"].as_str().unwrap());
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{entry}: the catalogued file exists: {e}"));
        assert_eq!(
            bytes.len() as u64,
            row["bytes"].as_u64().unwrap(),
            "{entry}: the file's mass is the row's"
        );
        assert_eq!(
            registry::sha256_hex(&bytes),
            row["sha256"].as_str().unwrap(),
            "{entry}: the file hashes to the row's sha256"
        );
        let bundle: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(bundle["entry"].as_str().unwrap(), entry, "{entry}: the bundle names its row");
        assert_eq!(bundle["tier"], row["tier"], "{entry}: the row's tier claim is the bundle's");
        assert_eq!(
            bundle["verdictBasis"], row["verdictBasis"],
            "{entry}: the row's basis claim is the bundle's"
        );
        let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
            .args(["verify-bundle", path.to_str().unwrap()])
            .output()
            .expect("run verify-bundle");
        if cfg!(feature = "z3-support") || bundle["tier"] == "tier-2" {
            assert!(
                out.status.success(),
                "{entry}: the shipped binary re-verifies the catalogued bundle: {}",
                stderr_of(&out)
            );
            assert!(
                stdout_of(&out).contains("the claim reproduces"),
                "{entry}: the verb answers reproduction"
            );
        } else {
            assert!(
                !out.status.success(),
                "{entry}: a Tier-1 bundle is not passed by a no-Z3 build"
            );
            assert!(
                stderr_of(&out).contains("unexamined"),
                "{entry}: the refusal names the standing: {}",
                stderr_of(&out)
            );
        }
    }

    // Every receipt's recorded standing equals the
    // standing its install re-measured: install the
    // pair this build can install, and compare each
    // receipt against an independent reproduction of
    // the installed file.
    let catalog = scratch_catalog("c4");
    let dir = scratch_dir("c4-install");
    let _ = std::fs::remove_dir_all(&dir);
    let installable: &[&str] = if cfg!(feature = "z3-support") {
        &["is_widget_code", "weights_sorted"]
    } else {
        &["weights_sorted"]
    };
    for name in installable {
        let out = run_registry(
            &["add", name, "--index", &catalog_index(&catalog)],
            &dir,
        );
        assert!(out.status.success(), "the install of {name}: {}", stderr_of(&out));
        assert_receipt_is_the_reproduction(&dir, name);
    }
    let out = run_registry(&["verify"], &dir);
    assert!(
        out.status.success(),
        "the sweep over the gate's own installs passes: {}",
        stderr_of(&out)
    );
}

// ── Clause 5: footprint + the dependency ────────────────

/// The frozen figures (DISEBAR_17_SCOPE §9 + the
/// standing budgets).
const INDEX_BUDGET: usize = 12_288;
const CATALOG_BUDGET: usize = 295_000;
const NPM_BUDGET: usize = 76_800;
const LIBRARY_COMPONENT_BUDGET: usize = 106_496;
const CALLBACK_COMPONENT_BUDGET: usize = 90_112;
/// The masses + hashes measured at the seed and the
/// 1.6 stamp; the stamp re-pins the component pair.
const INDEX_MASS: usize = 10_927;
const CATALOG_MASS: usize = 276_678;
const LIBRARY_COMPONENT_MASS: usize = 80_558;
const LIBRARY_COMPONENT_SHA256: &str =
    "64b99919caf2f8bba9823c1fa26261ef4cdbaf7f02f48793162372eb011b1fd5";
const CALLBACK_COMPONENT_MASS: usize = 66_011;
const CALLBACK_COMPONENT_SHA256: &str =
    "15f3f0a2cdbd4d043748a2d78b2bc96e9b6c5ec1bc1f99d2ddd57e90f11a5ba8";

fn dir_mass(dir: &std::path::Path) -> (usize, usize) {
    let mut files = 0;
    let mut bytes = 0usize;
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            let (f, b) = dir_mass(&path);
            files += f;
            bytes += b;
        } else {
            files += 1;
            bytes += entry.metadata().unwrap().len() as usize;
        }
    }
    (files, bytes)
}

fn sha256_of_file(path: &std::path::Path) -> String {
    if let Ok(out) = Command::new("sha256sum").arg(path).output() {
        if out.status.success() {
            return String::from_utf8(out.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .to_string();
        }
    }
    let bytes = std::fs::read(path).expect("read file for sha256");
    registry::sha256_hex(&bytes)
}

/// The dependency names a Cargo.toml declares, across
/// every dependency table (plain, target-gated, dev,
/// build) — for the milestone's one-new-dependency
/// comparison against the v1.6.0 tree.
fn dependency_names(toml: &str) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    let mut in_deps = false;
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line.ends_with("dependencies]");
            continue;
        }
        if in_deps && !line.is_empty() && !line.starts_with('#') {
            if let Some((key, _)) = line.split_once('=') {
                let name = key.trim().trim_matches('"').split('.').next().unwrap().trim();
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names
}

#[test]
fn p166_clause5_footprint_dependency_and_lockstep() {
    init();
    // The catalog's masses, inside §9's figures.
    let index_bytes = std::fs::read(repo_root().join("registry/index.json")).unwrap();
    assert_eq!(index_bytes.len(), INDEX_MASS, "the index's measured mass");
    assert!(index_bytes.len() <= INDEX_BUDGET, "the index is inside its budget");
    let (files, catalog_bytes) = dir_mass(&repo_root().join("registry/bundles"));
    assert_eq!(files, 37, "37 bundle files");
    assert_eq!(catalog_bytes, CATALOG_MASS, "the catalog's measured mass");
    assert!(catalog_bytes <= CATALOG_BUDGET, "the catalog is inside its budget");

    // `sha2` is the milestone's only new dependency,
    // and it does not enter the browser profile: the
    // root manifest's dependency set is v1.6.0's plus
    // exactly `sha2`, declared under the
    // not-wasm32 target table and nowhere else.
    let head_toml = std::fs::read_to_string(repo_root().join("Cargo.toml")).unwrap();
    let base = Command::new("git")
        .args(["show", "v1.6.0:Cargo.toml"])
        .current_dir(repo_root())
        .output()
        .expect("git show the v1.6.0 manifest");
    assert!(base.status.success(), "the v1.6.0 tag is readable");
    let base_toml = String::from_utf8(base.stdout).unwrap();
    let mut want = dependency_names(&base_toml);
    want.insert("sha2".to_string());
    assert_eq!(
        dependency_names(&head_toml),
        want,
        "the dependency set moved by exactly sha2 since v1.6.0"
    );
    let mut sha2_tables: Vec<String> = Vec::new();
    let mut table = String::new();
    for line in head_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            table = trimmed.to_string();
        } else if trimmed.starts_with("sha2") {
            sha2_tables.push(table.clone());
        }
    }
    assert_eq!(sha2_tables.len(), 1, "sha2 is declared exactly once: {sha2_tables:?}");
    assert!(
        sha2_tables[0].contains("cfg(not(target_arch = \"wasm32\"))"),
        "sha2 sits in the native target table, out of the browser graph: {}",
        sha2_tables[0]
    );

    // The npm package stands inside its standing
    // budget.
    let (_npm_files, npm_bytes) = dir_mass(&repo_root().join("packages/npm"));
    assert!(
        npm_bytes <= NPM_BUDGET,
        "the npm package mass {npm_bytes} is inside {NPM_BUDGET}"
    );

    // The components stand inside theirs, at their
    // pinned masses + hashes, true components both.
    for (rel, mass, sha, budget) in [
        (
            "packages/component/morphic-library.wasm",
            LIBRARY_COMPONENT_MASS,
            LIBRARY_COMPONENT_SHA256,
            LIBRARY_COMPONENT_BUDGET,
        ),
        (
            "packages/component-callbacks/morphic-callbacks.wasm",
            CALLBACK_COMPONENT_MASS,
            CALLBACK_COMPONENT_SHA256,
            CALLBACK_COMPONENT_BUDGET,
        ),
    ] {
        let path = repo_root().join(rel);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), mass, "{rel}: the pinned mass");
        assert!(bytes.len() <= budget, "{rel}: inside its budget");
        assert_eq!(
            &bytes[..8],
            &[0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00],
            "{rel}: a true component header"
        );
        assert_eq!(sha256_of_file(&path), sha, "{rel}: the pinned sha256");
    }

    // The lockstep: every version-bearing surface
    // states the same version — the crate, its lock
    // entry, both packages, both WIT packages, the
    // extension.
    let version_of = |toml: &str| -> String {
        toml.lines()
            .find_map(|l| l.trim().strip_prefix("version = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .expect("a version line")
            .to_string()
    };
    let crate_version = version_of(&head_toml);
    let lock = std::fs::read_to_string(repo_root().join("Cargo.lock")).unwrap();
    let lock_version = lock
        .split("[[package]]")
        .find(|block| block.contains("name = \"morphic\""))
        .map(|block| version_of(block))
        .expect("the lock carries the crate");
    assert_eq!(lock_version, crate_version, "the lock agrees with the crate");
    let npm_pkg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("packages/npm/package.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        npm_pkg["version"].as_str().unwrap(),
        crate_version,
        "the npm package stands at the lockstep"
    );
    let rust_toml =
        std::fs::read_to_string(repo_root().join("packages/rust/Cargo.toml")).unwrap();
    assert_eq!(version_of(&rust_toml), crate_version, "the facade stands at the lockstep");
    for wit in [
        "packages/component/wit/world.wit",
        "packages/component-callbacks/wit/world.wit",
    ] {
        let text = std::fs::read_to_string(repo_root().join(wit)).unwrap();
        assert!(
            text.contains(&format!("package morphic:library@{crate_version};")),
            "{wit} stands at the lockstep"
        );
    }
    let vscode_pkg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("packages/vscode/package.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        vscode_pkg["version"].as_str().unwrap(),
        crate_version,
        "the extension stands at the lockstep"
    );
}
