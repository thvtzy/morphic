//! Phase 165 — discovery at the prelude funnel
//! (DISEBAR_17_SCOPE §7): installed registry bundles join
//! the prelude as a layer behind `stdlib::{prelude_specs,
//! shipped_body}`, under precedence user > stdlib >
//! installed.
//!
//! The install state is produced by the Phase 164 verbs
//! themselves — the shipped binary, driven as a
//! subprocess against the repo's own catalog index, with
//! `MORPHIC_REGISTRY_DIR` scoped to a scratch dir — plus
//! one hand-planted stdlib-named bundle (file + forged
//! receipt row, the file's true sha256) for the
//! inertness pin. In a no-Z3 build the Tier-1 verb
//! refuses by name (the claim stands unexamined — Phase
//! 164's posture), so there the pair is planted exactly
//! as a Z3 install writes it: the state discovery reads
//! is the same bytes either way, which is discovery's
//! trust boundary stated as a setup. The in-process environment variable is
//! set once, before any funnel read in this process (the
//! layer reads the directory once per process, §7), and
//! every pin runs against that one state.
//!
//! Pins, in order:
//! 1. the funnel yields the stdlib's 35 then the
//!    installed pair; a hand-planted `int_abs` install is
//!    inert (one `int_abs` in the prelude, the stdlib's
//!    body answering, a diagnostic naming the skip);
//! 2. a user spec that *calls* `is_widget_code`
//!    typechecks, judges executed-exact through the
//!    index with the installed body, and renders to Rust
//!    with the installed body emitted;
//! 3. a user module defining `is_widget_code` itself
//!    shadows the installed entry at the merge (the
//!    user's semantics judge, the installed semantics do
//!    not);
//! 4. the Tier-2 entry `weights_sorted` is equally
//!    callable through the funnel.

#![cfg(not(feature = "wasm-playground"))]

use std::process::Command;
use std::sync::{Mutex, Once};

use morphic::spec::ast::FunctionSpec;
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{IRNode, SpecIndex};
use morphic::synthesis::eval;
use morphic::{api, stdlib};

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("morphic-phase165-discovery-{}", std::process::id()))
}

/// Plant one catalog entry's installed state exactly as
/// a successful install writes it: the bundle file
/// byte-identical to the catalog's, and a receipt row
/// recording the bundle's own standings (which is what
/// the install's re-run measures and records, §5.3).
fn plant_install(dir: &std::path::Path, name: &str) -> serde_json::Value {
    let bytes = std::fs::read(
        repo_root().join("registry").join("bundles").join(format!("{name}.bundle.json")),
    )
    .expect("the catalog bundle");
    std::fs::write(dir.join(format!("{name}.bundle.json")), &bytes).unwrap();
    let bundle: serde_json::Value =
        serde_json::from_slice(&bytes).expect("the catalog bundle parses");
    serde_json::json!({
        "entry": name,
        "version": "1.7.0",
        "sha256": morphic::registry::sha256_hex(&bytes),
        "tier": bundle["tier"],
        "verdictBasis": bundle["verdictBasis"],
        "verdict": bundle["verdict"],
        "crownRoute": bundle["crownRoute"],
        "battery": bundle["battery"],
        "index": "planted exactly as install writes it (phase165 pin)",
        "installed": "2026-11-26",
    })
}

/// Write the receipt ledger for planted rows, in the
/// verbs' own format and order.
fn write_planted_ledger(dir: &std::path::Path, mut rows: Vec<serde_json::Value>) {
    rows.sort_by(|a, b| a["entry"].as_str().cmp(&b["entry"].as_str()));
    let ledger = serde_json::json!({
        "format": morphic::registry::LEDGER_FORMAT,
        "entries": rows,
    });
    std::fs::write(
        dir.join("installed.json"),
        serde_json::to_string_pretty(&ledger).unwrap(),
    )
    .unwrap();
}

/// Install the widget pair — through the Phase 164
/// verbs (the shipped binary) in a Z3 build. In a
/// no-Z3 build the Tier-1 half cannot be installed at
/// all (Phase 164's standing posture: the claim stands
/// *unexamined*, refused by name), so the pair is
/// planted exactly as a Z3 install writes it — the
/// cross-build state discovery exists to read, since
/// discovery trusts the receipt in every build. Then
/// hand-plant a stdlib-named bundle: the catalog's own
/// `int_abs` bundle file plus a forged receipt row
/// carrying the file's true sha256 — so its inertness
/// can only come from the funnel's layer order, never
/// from a hash or parse failure. Finally point this
/// process at the directory.
fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let dir = scratch_dir();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        if cfg!(feature = "z3-support") {
            let index = repo_root().join("registry").join("index.json");
            for name in ["is_widget_code", "weights_sorted"] {
                let out = Command::new(env!("CARGO_BIN_EXE_morphic"))
                    .args(["registry", "add", name, "--index"])
                    .arg(&index)
                    .env("MORPHIC_REGISTRY_DIR", &dir)
                    .output()
                    .expect("the registry add runs");
                assert!(
                    out.status.success(),
                    "registry add {name}: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        } else {
            write_planted_ledger(
                &dir,
                vec![
                    plant_install(&dir, "is_widget_code"),
                    plant_install(&dir, "weights_sorted"),
                ],
            );
        }
        // The hand-planted stdlib name.
        let planted = std::fs::read(
            repo_root().join("registry").join("bundles").join("int_abs.bundle.json"),
        )
        .expect("the catalog's int_abs bundle");
        std::fs::write(dir.join("int_abs.bundle.json"), &planted).unwrap();
        let bundle: serde_json::Value =
            serde_json::from_slice(&planted).expect("the planted bundle parses");
        let ledger_path = dir.join("installed.json");
        let mut ledger: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&ledger_path).unwrap())
                .expect("the verbs' ledger parses");
        ledger["entries"].as_array_mut().unwrap().push(serde_json::json!({
            "entry": "int_abs",
            "version": "1.7.0",
            "sha256": morphic::registry::sha256_hex(&planted),
            "tier": bundle["tier"],
            "verdictBasis": bundle["verdictBasis"],
            "verdict": bundle["verdict"],
            "crownRoute": bundle["crownRoute"],
            "battery": bundle["battery"],
            "index": "planted by hand (phase165 pin)",
            "installed": "2026-11-26",
        }));
        ledger["entries"]
            .as_array_mut()
            .unwrap()
            .sort_by(|a, b| a["entry"].as_str().cmp(&b["entry"].as_str()));
        std::fs::write(
            &ledger_path,
            serde_json::to_string_pretty(&ledger).unwrap(),
        )
        .unwrap();
        std::env::set_var("MORPHIC_REGISTRY_DIR", &dir);
    });
}

fn catalog_bundle(name: &str) -> morphic::pcc::Bundle {
    let text = std::fs::read_to_string(
        repo_root().join("registry").join("bundles").join(format!("{name}.bundle.json")),
    )
    .unwrap();
    serde_json::from_str(&text).expect("the catalog bundle parses")
}

fn spec_named<'a>(specs: &'a [FunctionSpec], name: &str) -> &'a FunctionSpec {
    specs.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("{name} is a spec"))
}

const CALLER_MODULE: &str = r#"RegistryCaller {
spec code_is_known {
    doc: "code_is_known(code) = the installed is_widget_code, called."
    input: code: Int
    output: Bool
    constraint: output == is_widget_code(code)
    test "in" { input: 3 expect: true }
    test "edge" { input: 6 expect: true }
    test "out" { input: 9 expect: false }
}
spec weights_are_sorted {
    doc: "weights_are_sorted(xs) = the installed weights_sorted, called."
    input: xs: List<Int>
    output: Bool
    constraint: output == weights_sorted(xs)
    test "sorted" { input: [1, 2, 2]; expect: true }
    test "unsorted" { input: [3, 1]; expect: false }
    test "empty" { input: []; expect: true }
}
}"#;

const SHADOW_MODULE: &str = r#"Shadow {
spec is_widget_code {
    doc: "The user's own: a widget code is known only when it is 42."
    input: code: Int
    output: Bool
    constraint: output == (code == 42)
    test "answer" { input: 42 expect: true }
    test "three" { input: 3 expect: false }
}
spec probe {
    doc: "probe(code) = is_widget_code(code), whoever defines it."
    input: code: Int
    output: Bool
    constraint: output == is_widget_code(code)
    test "answer" { input: 42 expect: true }
    test "three" { input: 3 expect: false }
}
spec probe_installed_semantics {
    doc: "The installed entry's semantics, stated as a spec: 3 is known."
    input: code: Int
    output: Bool
    constraint: output == is_widget_code(code)
    test "three" { input: 3 expect: true }
}
}"#;

fn call_body(name: &str, arg: &str) -> IRNode {
    IRNode::Call {
        function: name.to_string(),
        args: vec![IRNode::Var(arg.to_string())],
    }
}

// ── 1. The funnel's shape ───────────────────────────────

#[test]
fn p165_funnel_yields_stdlib_then_installed_and_planted_stdlib_name_is_inert() {
    init();
    let _serial = crown_serial();
    let specs = stdlib::prelude_specs();
    let stdlib = stdlib::stdlib_specs();
    let stdlib_names: Vec<&str> = stdlib.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(stdlib_names.len(), 35, "the stdlib layer stands at 35");
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        &names[..35],
        &stdlib_names[..],
        "the prelude yields the stdlib first, in its own order"
    );
    assert_eq!(
        &names[35..],
        ["is_widget_code", "weights_sorted"],
        "then the installed layer, in ledger order"
    );
    assert_eq!(
        names.iter().filter(|n| **n == "int_abs").count(),
        1,
        "the hand-planted int_abs install yields no second int_abs"
    );
    // The installed bodies answer at the funnel, read
    // back through the standing IR text reader.
    for name in ["is_widget_code", "weights_sorted"] {
        let want = morphic::synthesis::irtext::parse_ir(&catalog_bundle(name).body)
            .expect("the catalog body reads");
        let got = stdlib::shipped_body(name).expect("the installed body answers");
        assert_eq!(
            format!("{got:?}"),
            format!("{want:?}"),
            "{name}: shipped_body is the bundle's body"
        );
    }
    // And the stdlib's int_abs body — not the planted
    // bundle's — is what the funnel answers for int_abs.
    let asset = stdlib::library_asset();
    let (_, stdlib_abs) =
        asset.bodies.iter().find(|(n, _)| n == "int_abs").expect("stdlib int_abs");
    let funnel_abs = stdlib::shipped_body("int_abs").expect("int_abs answers");
    assert_eq!(
        format!("{funnel_abs:?}"),
        format!("{stdlib_abs:?}"),
        "int_abs answers with the stdlib's body"
    );
    // The skip is named, not silent.
    let diagnostics = stdlib::installed_diagnostics();
    assert!(
        diagnostics.iter().any(|d| d.contains("int_abs") && d.contains("inert")),
        "a diagnostic names the inert planted entry: {diagnostics:?}"
    );
}

// ── 2. Calling an installed Tier-1 entry ────────────────

#[test]
fn p165_caller_typechecks_judges_and_renders_with_the_installed_body() {
    init();
    let _serial = crown_serial();
    let module = parser::parse(CALLER_MODULE).expect("the caller module parses");
    let checked = typeck::check(module.clone());
    assert!(
        checked.is_ok(),
        "a call to the installed entry typechecks: {:?}",
        checked.err()
    );
    let index = SpecIndex::from_module(&module);
    let caller = spec_named(&module.functions, "code_is_known");
    // The index carries the installed body as the
    // callee's denotation.
    let denotation = index
        .callee_body("is_widget_code")
        .expect("the installed entry denotes its shipped body in the index");
    let want = morphic::synthesis::irtext::parse_ir(&catalog_bundle("is_widget_code").body)
        .expect("the catalog body reads");
    assert_eq!(format!("{denotation:?}"), format!("{want:?}"));
    // And a caller body judges executed-exact against
    // rows stating the installed semantics — through
    // the standing resolver route (Phase 110:
    // `inline_calls` substitutes the callee's shipped
    // body, the mechanism every judging path rides;
    // the inlined body then runs under the interpreter).
    let inlined = index
        .inline_calls(&call_body("is_widget_code", "code"))
        .expect("the call inlines the installed body");
    assert_eq!(
        format!("{inlined:?}"),
        format!("{want:?}"),
        "inlining substitutes the installed body itself"
    );
    let run = eval::run_spec_tests_in(&inlined, caller, &index);
    assert_eq!(
        run,
        eval::TestRun { evaluable: 3, passed: 3 },
        "the caller judges through the installed body"
    );
    // Emitted code carries the installed body.
    let candidate = morphic::synthesis::engine::CandidateImplementation {
        id: 165,
        body: call_body("is_widget_code", "code"),
        spec_name: caller.name.clone(),
        spec: caller.clone(),
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
    };
    let rust = api::render(&candidate, "rust").expect("the caller renders to Rust");
    assert!(
        rust.contains("is_widget_code"),
        "the emitted Rust names the installed entry:\n{rust}"
    );
    assert!(
        rust.contains(">= 1") && rust.contains("<= 6"),
        "the emitted Rust carries the installed body's bounds:\n{rust}"
    );
}

// ── 3. The user's own definition shadows ────────────────

#[test]
fn p165_user_definition_shadows_the_installed_entry() {
    init();
    let _serial = crown_serial();
    let module = parser::parse(SHADOW_MODULE).expect("the shadow module parses");
    assert!(
        typeck::check(module.clone()).is_ok(),
        "the shadowing module typechecks"
    );
    let index = SpecIndex::from_module(&module);
    // The index's is_widget_code is the module's own
    // spec — the installed spec never merges over it.
    let in_index = index.get("is_widget_code").expect("the name resolves");
    assert!(
        in_index.doc.as_deref().unwrap_or("").contains("user's own"),
        "the index holds the user's spec, not the installed one: {:?}",
        in_index.doc
    );
    // Judged through the resolver route: the call
    // inlines whoever's denotation the index holds —
    // here the user's own equation body, because the
    // merge never registered the installed body over
    // the module's name.
    let inlined = index
        .inline_calls(&call_body("is_widget_code", "code"))
        .expect("the call inlines the index's denotation");
    let probe = spec_named(&module.functions, "probe");
    let run = eval::run_spec_tests_in(&inlined, probe, &index);
    assert_eq!(
        run,
        eval::TestRun { evaluable: 2, passed: 2 },
        "the user's semantics judge (42 known, 3 not)"
    );
    let installed_semantics = spec_named(&module.functions, "probe_installed_semantics");
    let run = eval::run_spec_tests_in(&inlined, installed_semantics, &index);
    assert!(
        run.passed < run.evaluable || run.evaluable == 0,
        "the installed semantics do NOT judge under the user's definition: {run:?}"
    );
}

// ── 4. The Tier-2 entry is equally callable ─────────────

#[test]
fn p165_tier2_installed_entry_is_equally_callable() {
    init();
    let _serial = crown_serial();
    let module = parser::parse(CALLER_MODULE).expect("the caller module parses");
    assert!(
        typeck::check(module.clone()).is_ok(),
        "a call to the installed Tier-2 entry typechecks"
    );
    let index = SpecIndex::from_module(&module);
    let caller = spec_named(&module.functions, "weights_are_sorted");
    // The index carries the installed body as the
    // callee's denotation, exactly as for a Tier-1 entry.
    let denotation = index
        .callee_body("weights_sorted")
        .expect("the installed Tier-2 entry denotes its shipped body in the index");
    let want = morphic::synthesis::irtext::parse_ir(&catalog_bundle("weights_sorted").body)
        .expect("the catalog body reads");
    assert_eq!(format!("{denotation:?}"), format!("{want:?}"));
    // Judged the Tier-2 route (Phase 115's executed
    // standard): the shipped body itself against the
    // rows — here the CALLER's rows, whose parameter is
    // the callee's own. The resolver route of tooth 2
    // does not apply: `inline_calls`' shape set is the
    // expression forms, and a statement body (Block /
    // Alloc / For) declines it — the standing boundary
    // every Tier-2 stdlib body also stands behind.
    let run = eval::run_spec_tests_in(&denotation, caller, &index);
    assert_eq!(
        run,
        eval::TestRun { evaluable: 3, passed: 3 },
        "the Tier-2 installed body judges the caller's rows"
    );
}
