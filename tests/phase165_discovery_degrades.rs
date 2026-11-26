//! Phase 165 — discovery degradation, layer-wide shape
//! (DISEBAR_17_SCOPE §7): a corrupt receipt ledger
//! empties the installed layer BY NAME — a diagnostic
//! states the fact and the reason — while the stdlib and
//! the user's module stay fully functional.
//!
//! This binary's one install state: a scratch
//! `MORPHIC_REGISTRY_DIR` whose `installed.json` is not
//! JSON at all. The environment is set once, before any
//! funnel read in this process (the layer reads the
//! directory once per process, §7).

#![cfg(not(feature = "wasm-playground"))]

use std::process::Command;
use std::sync::Once;

use morphic::stdlib;

fn scratch_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("morphic-phase165-degrades-{}", std::process::id()))
}

fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let dir = scratch_dir();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("installed.json"), "this ledger is not json {").unwrap();
        std::env::set_var("MORPHIC_REGISTRY_DIR", &dir);
    });
}

const STDLIB_CALLER: &str = r#"StdlibCaller {
spec magnitude {
    doc: "magnitude(x) = int_abs(x), a stdlib call."
    input: x: Int
    output: Int
    constraint: output == int_abs(x)
    test { input: (0 - 3); expect: 3 }
}
}"#;

const INSTALLED_CALLER: &str = r#"InstalledCaller {
spec code_is_known {
    doc: "code_is_known(code) = is_widget_code(code), an installed call."
    input: code: Int
    output: Bool
    constraint: output == is_widget_code(code)
    test "in" { input: 3 expect: true }
}
}"#;

#[test]
fn p165_corrupt_ledger_empties_the_layer_by_name_and_stdlib_still_answers() {
    init();
    // The funnel is exactly the stdlib layer.
    let specs = stdlib::prelude_specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    let stdlib = stdlib::stdlib_specs();
    let stdlib_names: Vec<&str> = stdlib.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, stdlib_names, "a corrupt ledger leaves the stdlib prelude, whole");
    assert!(
        stdlib::shipped_body("int_abs").is_some(),
        "stdlib bodies still answer"
    );
    assert!(
        stdlib::shipped_body("is_widget_code").is_none(),
        "no installed body leaks through a corrupt ledger"
    );
    // The degradation is named: the diagnostic carries
    // the ledger's own refusal (its path and reason).
    let diagnostics = stdlib::installed_diagnostics();
    assert!(
        diagnostics.iter().any(|d| d.contains("installed.json")),
        "a diagnostic names the corrupt ledger: {diagnostics:?}"
    );
    // The user's module and the stdlib layer are
    // unaffected: a stdlib call still checks; an
    // installed call now fails by name (unknown
    // function), because the layer stands empty — said,
    // not silent.
    assert!(
        morphic::api::check(STDLIB_CALLER).ok,
        "a user spec calling the stdlib still checks"
    );
    assert!(
        !morphic::api::check(INSTALLED_CALLER).ok,
        "a user spec calling an installed entry no longer checks — the layer is empty"
    );
}

#[test]
fn p165_cli_check_names_the_degradation_on_stderr() {
    init();
    let dir = scratch_dir();
    let spec_path = dir.join("magnitude.morph");
    std::fs::write(&spec_path, STDLIB_CALLER).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("check")
        .arg(&spec_path)
        .env("MORPHIC_REGISTRY_DIR", &dir)
        .output()
        .expect("the check runs");
    assert!(
        out.status.success(),
        "check over a corrupt install still succeeds on the stdlib: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("installed.json"),
        "the CLI names the degradation on stderr: {stderr}"
    );
    // And the installed caller fails the same shell.
    let spec_path = dir.join("known.morph");
    std::fs::write(&spec_path, INSTALLED_CALLER).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("check")
        .arg(&spec_path)
        .env("MORPHIC_REGISTRY_DIR", &dir)
        .output()
        .expect("the check runs");
    assert!(
        !out.status.success(),
        "check of an installed call fails while the layer stands empty"
    );
}
