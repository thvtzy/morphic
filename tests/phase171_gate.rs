//! Phase 171 — the 1.8 gate (`IKONIK_18_SCOPE.md` §8),
//! run clause by clause on the gate tree with fresh
//! instruments, in the Phase 151/157/161/166 gate shape.
//!
//! The frozen gate text (`IKONIK_18_SCOPE.md` §8, never amended):
//! 1. **Core emitter & honesty refusals.**
//!    `codegen::generate(cand, "solidity")` (and `"sol"`) renders all
//!    6 `DeFiMath` exhibits and all 18 direct-ABI stdlib entries cleanly;
//!    all 17 non-direct-ABI stdlib entries, unsupported types (`Float`,
//!    `Char`, `Option`, `Result`, `Function`, `Map`, `Set`, `Generic`,
//!    unknown `Named`), and unrenderable preconditions (`Forall`,
//!    `Exists`) refuse by name with `CodegenError` — zero silent
//!    `require(true)` or placeholder types.
//! 2. **Real `solc` compilation (zero warnings).** Every generated
//!    exhibit contract and direct-ABI stdlib contract compiles cleanly
//!    under the pinned `solc` with **0 errors and 0 warnings**.
//! 3. **Real EVM execution + runtime `require(...)` reverts.** The
//!    compiled EVM bytecode answers **100% of every exhibit's and
//!    direct-ABI stdlib entry's frozen battery points** identically to
//!    Morphic's interpreter/Rust output, AND **reverts** on
//!    out-of-domain inputs that violate `require:` (including
//!    `fee_bps(100, 10001)`, `debit_balance(100, 200)`,
//!    `int_clamp(5, 10, 0)`, `linear_vested(1000, 50, 0)`).
//! 4. **Proof-carrying contract loop.** `morphic build --contract-from`
//!    emits `<Module>.sol` alongside `<entry>.bundle.json` files whose
//!    sha256 hashes match the contract's NatSpec provenance lines;
//!    `morphic verify-bundle` independently re-verifies every companion
//!    bundle (Tier-1 under Z3; honest refusal without Z3), and a
//!    tampered bundle refuses by name.
//! 5. **Footprint, standings, and suites.** Zero new Cargo dependencies
//!    (standing-13 unchanged); playground WASM bundle within frozen
//!    771,072 B; npm/component/registry budgets unmoved; CI Solidity
//!    step green; all three frozen test configurations green solo.

#![cfg(not(feature = "wasm-playground"))]

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use morphic::codegen;
use morphic::codegen::solidity;
use morphic::spec::ast::{BinOp, Constraint, Expr, FunctionSpec, Param, TypeRef, UnaryOp};
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{
    CandidateImplementation, IRLiteral, IRNode, Provenance, ScoreBreakdown,
};
use morphic::synthesis::eval::{self, Value};
use morphic::{api, registry, stdlib};
use serde_json::{json, Value as JsonValue};

fn gate_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("morphic-p171-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn npm_cmd() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

fn ensure_evm_harness() -> Option<PathBuf> {
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let npm_ok = Command::new(npm_cmd())
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !node_ok || !npm_ok {
        println!("P171|evm|node/npm ABSENT in this environment; EVM checks skipped");
        return None;
    }

    let dir = repo_root().join("target").join("solidity-harness");
    let ready = dir.join("node_modules/solc/package.json").exists()
        && dir
            .join("node_modules/@ethereumjs/evm/package.json")
            .exists()
        && dir.join("node_modules/ethers/package.json").exists();
    if !ready {
        std::fs::create_dir_all(&dir).expect("create target/solidity-harness");
        let out = Command::new(npm_cmd())
            .args([
                "--prefix",
                dir.to_str().unwrap(),
                "install",
                "--no-save",
                "--no-audit",
                "--no-fund",
                "solc@0.8.28",
                "@ethereumjs/evm@3.1.1",
                "ethers@6.13.4",
            ])
            .output()
            .expect("npm install pinned solidity harness");
        assert!(
            out.status.success(),
            "npm install pinned solidity harness failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Some(dir)
}

fn run_evm_runner(harness_dir: &Path, label: &str, plan: &JsonValue) -> String {
    let plan_path = std::env::temp_dir().join(format!(
        "morphic-p171-plan-{label}-{}.json",
        std::process::id()
    ));
    std::fs::write(&plan_path, serde_json::to_string_pretty(plan).unwrap()).unwrap();
    let runner = repo_root().join("tests/solidity_evm_runner.cjs");
    let out = Command::new("node")
        .args([
            runner.to_str().unwrap(),
            harness_dir.to_str().unwrap(),
            plan_path.to_str().unwrap(),
        ])
        .output()
        .expect("run solidity_evm_runner.cjs");
    let _ = std::fs::remove_file(&plan_path);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "EVM runner `{label}` failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("EVM-OK") && stdout.contains("warnings=0"),
        "EVM runner `{label}` missing EVM-OK/warnings=0:\n{stdout}"
    );
    stdout.trim().to_string()
}

fn make_cand(spec: FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 171,
        body,
        spec_name: spec.name.clone(),
        spec,
        score: 0.0,
        scores: ScoreBreakdown {
            test_pass_ratio: 0.0,
            constraint_score: 0.0,
            complexity_score: 0.0,
            quality_score: 0.0,
            synthesis_time_us: 0,
        },
        generation: 0,
        provenance: Provenance::HumanGuided,
        verified: false,
        verification_status: None,
    }
}

fn pascal_name(name: &str) -> String {
    let mut out = String::new();
    for part in name.split(|c: char| !c.is_ascii_alphanumeric()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    out
}

const DIRECT_ABI_STDLIB: &[&str] = &[
    "int_abs",
    "int_sign",
    "int_min",
    "int_max",
    "int_clamp",
    "int_is_even",
    "int_is_odd",
    "string_is_empty",
    "list_is_empty",
    "list_reverse",
    "list_contains",
    "list_sum",
    "list_product",
    "list_take",
    "list_drop",
    "list_is_sorted",
    "list_sort",
    "range",
];

const DEFI_EXHIBITS: &[&str] = &[
    "fee_bps",
    "debit_balance",
    "bounded_payout",
    "linear_vested",
    "can_withdraw",
    "non_empty_batch",
];

fn eval_const_i64(expr: &Expr) -> i64 {
    match expr {
        Expr::IntLit(n) => *n,
        Expr::UnaryOp(UnaryOp::Neg, inner) => -eval_const_i64(inner),
        Expr::BinOp(BinOp::Add, l, r) => eval_const_i64(l).checked_add(eval_const_i64(r)).unwrap(),
        Expr::BinOp(BinOp::Sub, l, r) => eval_const_i64(l).checked_sub(eval_const_i64(r)).unwrap(),
        Expr::BinOp(BinOp::Mul, l, r) => eval_const_i64(l).checked_mul(eval_const_i64(r)).unwrap(),
        Expr::BinOp(BinOp::Div, l, r) => eval_const_i64(l).checked_div(eval_const_i64(r)).unwrap(),
        Expr::BinOp(BinOp::Mod, l, r) => eval_const_i64(l).checked_rem(eval_const_i64(r)).unwrap(),
        other => panic!("expected const int expr, got {other:?}"),
    }
}

fn eval_const_value(expr: &Expr) -> Value {
    match expr {
        Expr::IntLit(n) => Value::Int(*n),
        Expr::BoolLit(b) => Value::Bool(*b),
        Expr::StringLit(s) => Value::Str(s.clone()),
        Expr::UnaryOp(UnaryOp::Neg, inner) => Value::Int(-eval_const_i64(inner)),
        Expr::UnaryOp(UnaryOp::Not, inner) => match eval_const_value(inner) {
            Value::Bool(b) => Value::Bool(!b),
            other => panic!("expected bool under Not, got {other:?}"),
        },
        Expr::BinOp(BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod, _, _) => {
            Value::Int(eval_const_i64(expr))
        }
        Expr::Call(name, args) if name == "List" => {
            Value::List(args.iter().map(eval_const_value).collect())
        }
        other => panic!("unsupported const value expr: {other:?}"),
    }
}

fn value_to_evm_json(val: &Value) -> JsonValue {
    match val {
        Value::Int(n) => json!(n.to_string()),
        Value::Bool(b) => json!(b),
        Value::Str(s) => json!({ "str": s }),
        Value::List(xs) => JsonValue::Array(xs.iter().map(value_to_evm_json).collect()),
        other => panic!("unsupported EVM value: {other:?}"),
    }
}

fn test_input_values(spec: &FunctionSpec, input: &Expr) -> Vec<Value> {
    match input {
        Expr::Call(name, args) if name == "__tuple__" && args.len() == spec.params.len() => {
            args.iter().map(eval_const_value).collect()
        }
        other if spec.params.len() == 1 => vec![eval_const_value(other)],
        other => panic!(
            "spec {}: cannot spread input {other:?} across {} params",
            spec.name,
            spec.params.len()
        ),
    }
}

// ── Clause 1: Core emitter & honesty refusals ──────────────

#[test]
fn p171_clause1_core_emitter_and_honesty_refusals() {
    let _serial = gate_serial();

    // 1a. All 18 direct-ABI stdlib entries render cleanly; "solidity" == "sol" == api::render;
    //     all 17 non-direct-ABI stdlib entries refuse by name.
    let specs = stdlib::stdlib_specs();
    assert_eq!(specs.len(), 35, "standing stdlib holds 35 entries");
    let direct: BTreeSet<&str> = DIRECT_ABI_STDLIB.iter().copied().collect();
    let mut ok_count = 0usize;
    let mut refused_count = 0usize;

    for spec in &specs {
        let body = stdlib::shipped_body(&spec.name)
            .unwrap_or_else(|| panic!("stdlib entry {} has shipped body", spec.name));
        let cand = make_cand(spec.clone(), body);
        let res_sol_full = codegen::generate(&cand, "solidity");
        let res_sol_alias = codegen::generate(&cand, "sol");
        let res_api = api::render(&cand, "solidity");

        if direct.contains(spec.name.as_str()) {
            let src = res_sol_full
                .unwrap_or_else(|e| panic!("direct-ABI stdlib {} must render: {e}", spec.name));
            assert_eq!(
                src,
                res_sol_alias.unwrap(),
                "{}: 'solidity' and 'sol' byte-identical",
                spec.name
            );
            assert_eq!(
                src,
                res_api.unwrap(),
                "{}: api::render matches codegen::generate",
                spec.name
            );
            assert!(src.contains("SPDX-License-Identifier: MIT"));
            assert!(src.contains("pragma solidity ^0.8.24;"));
            assert!(src.contains("Proof boundary (D8a):"));
            assert!(src.contains("Panic(0x11)"));
            assert!(
                !src.contains("require(true"),
                "{}: must never emit require(true)",
                spec.name
            );
            ok_count += 1;
        } else {
            let err = res_sol_full
                .expect_err(&format!("non-direct-ABI stdlib {} must refuse", spec.name))
                .to_string();
            assert!(
                res_sol_alias.is_err() && res_api.is_err(),
                "{}: alias and api::render must refuse too",
                spec.name
            );
            assert!(
                !err.trim().is_empty(),
                "{}: refusal names its reason",
                spec.name
            );
            refused_count += 1;
        }
    }
    assert_eq!(ok_count, 18, "exactly 18 direct-ABI stdlib entries render");
    assert_eq!(
        refused_count, 17,
        "exactly 17 non-direct-ABI stdlib entries refuse"
    );

    // 1b. All 6 DeFiMath exhibits render cleanly through checked contract assembly.
    let defi_src =
        std::fs::read_to_string(repo_root().join("examples/contracts/defi_math.morph")).unwrap();
    let assembly = solidity::assemble_user_contract(&defi_src, 42)
        .expect("DeFiMath assembles cleanly into Solidity");
    assert_eq!(assembly.standings.len(), 6);
    for name in DEFI_EXHIBITS {
        assert!(
            assembly.standings.iter().any(|e| e.name == *name),
            "DeFiMath assembly contains {name}"
        );
        assert!(
            assembly
                .contract_source
                .contains(&format!("function {name}(")),
            "DeFiMath.sol emits function {name}"
        );
    }
    assert!(assembly.contract_source.contains("Proof boundary (D8a):"));
    assert!(!assembly.contract_source.contains("require(true"));

    // 1c. Unsupported types all refuse by name with CodegenError (zero placeholder types).
    let bad_types: &[(&str, TypeRef)] = &[
        ("Float", TypeRef::Float),
        ("Char", TypeRef::Char),
        ("Option", TypeRef::Option(Box::new(TypeRef::Int))),
        (
            "Result",
            TypeRef::Result(Box::new(TypeRef::Int), Box::new(TypeRef::String)),
        ),
        (
            "Function",
            TypeRef::Function(vec![TypeRef::Int], Box::new(TypeRef::Int)),
        ),
        (
            "Map",
            TypeRef::Map(Box::new(TypeRef::Int), Box::new(TypeRef::Int)),
        ),
        ("Set", TypeRef::Set(Box::new(TypeRef::Int))),
        ("Generic", TypeRef::Generic("T".into())),
        ("Named", TypeRef::Named("CustomAccount".into())),
    ];
    for (label, ty) in bad_types {
        let spec = FunctionSpec {
            name: format!("bad_{}", label.to_ascii_lowercase()),
            doc: None,
            generics: vec![],
            params: vec![Param {
                name: "x".into(),
                ty: ty.clone(),
                annotations: HashMap::new(),
            }],
            return_type: TypeRef::Int,
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: HashMap::new(),
        };
        let cand = make_cand(spec, IRNode::Literal(IRLiteral::Int(0)));
        let err = codegen::generate(&cand, "solidity")
            .expect_err(&format!("type {label} must refuse"))
            .to_string();
        assert!(
            err.to_ascii_lowercase().contains("solidity"),
            "type {label} refusal names solidity: {err}"
        );
    }

    // 1d. Unrenderable preconditions (Forall, Exists) refuse by name — zero silent require(true).
    for (label, pre) in [
        (
            "Forall",
            Constraint::Forall {
                vars: vec![("i".into(), TypeRef::Int)],
                body: Box::new(Constraint::True),
            },
        ),
        (
            "Exists",
            Constraint::Exists {
                vars: vec![("i".into(), TypeRef::Int)],
                body: Box::new(Constraint::True),
            },
        ),
    ] {
        let spec = FunctionSpec {
            name: format!("quant_{}", label.to_ascii_lowercase()),
            doc: None,
            generics: vec![],
            params: vec![Param {
                name: "x".into(),
                ty: TypeRef::Int,
                annotations: HashMap::new(),
            }],
            return_type: TypeRef::Int,
            preconditions: vec![pre],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: HashMap::new(),
        };
        let cand = make_cand(spec, IRNode::Var("x".into()));
        let err = codegen::generate(&cand, "solidity")
            .expect_err(&format!("{label} precondition must refuse"))
            .to_string();
        assert!(
            err.to_ascii_lowercase().contains("quantifier")
                || err.to_ascii_lowercase().contains(&label.to_ascii_lowercase()),
            "{label} refusal names the quantifier: {err}"
        );
    }

    // 1e. D6a: WinnerReport.renders stays at the standing 6 targets.
    let single_spec = r#"
GateCheck {
spec inc {
    input: x: Int
    output: Int
    constraint: output == x + 1
    test "zero" { input: 0 expect: 1 }
}
}
"#;
    let report = api::synthesize_seeded_spec(single_spec, "inc", 42).expect("inc crowns");
    let targets: Vec<&str> = report
        .winner
        .renders
        .iter()
        .map(|r| r.target.as_str())
        .collect();
    assert_eq!(
        targets,
        ["rust", "c", "wasm", "python", "javascript", "ir"],
        "D6a: WinnerReport.renders remains at 6 standing targets"
    );

    println!("P171|clause1|PASS direct-abi=18/18 refused=17/17 defi=6/6 bad-types=9/9 quantifiers=2/2 renders=6");
}

// ── Clause 2: Real solc compilation (zero warnings) ────────

#[test]
fn p171_clause2_real_solc_zero_warnings() {
    let _serial = gate_serial();
    let Some(harness_dir) = ensure_evm_harness() else {
        return;
    };

    // Assemble DeFiMath.sol fresh through the shipped CLI binary.
    let out_dir = scratch_dir("c2-defi");
    let defi_path = repo_root().join("examples/contracts/defi_math.morph");
    let cli = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args([
            "build",
            "--contract-from",
            defi_path.to_str().unwrap(),
            "--out",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run morphic build --contract-from");
    assert!(
        cli.status.success(),
        "CLI --contract-from failed: {}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let defi_sol = std::fs::read_to_string(out_dir.join("DeFiMath.sol")).unwrap();

    let mut contracts = vec![json!({
        "file": "DeFiMath.sol",
        "contract": "DeFiMath",
        "source": defi_sol,
        "calls": [],
        "reverts": [],
    })];

    let specs = stdlib::stdlib_specs();
    for entry in DIRECT_ABI_STDLIB {
        let spec = specs.iter().find(|s| s.name == *entry).unwrap().clone();
        let body = stdlib::shipped_body(entry).unwrap();
        let src = codegen::generate(&make_cand(spec, body), "solidity").unwrap();
        let cname = pascal_name(entry);
        contracts.push(json!({
            "file": format!("{cname}.sol"),
            "contract": cname,
            "source": src,
            "calls": [],
            "reverts": [],
        }));
    }

    let summary = run_evm_runner(
        &harness_dir,
        "clause2-zero-warnings",
        &json!({ "contracts": contracts }),
    );
    assert!(
        summary.contains("contracts=19") && summary.contains("warnings=0"),
        "expected 19 contracts with 0 warnings: {summary}"
    );
    let _ = std::fs::remove_dir_all(&out_dir);
    println!("P171|clause2|PASS {summary}");
}

// ── Clause 3: Real EVM execution + runtime require reverts ─

#[test]
fn p171_clause3_real_evm_execution_and_reverts() {
    let _serial = gate_serial();
    let Some(harness_dir) = ensure_evm_harness() else {
        return;
    };

    // 3a. DeFiMath: assemble via shipped CLI, execute all 28 spec test rows + gate-named reverts.
    let out_dir = scratch_dir("c3-defi");
    let defi_path = repo_root().join("examples/contracts/defi_math.morph");
    let cli = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args([
            "build",
            "--contract-from",
            defi_path.to_str().unwrap(),
            "--out",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run morphic build --contract-from");
    assert!(
        cli.status.success(),
        "CLI --contract-from failed: {}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let defi_sol = std::fs::read_to_string(out_dir.join("DeFiMath.sol")).unwrap();

    let defi_src = std::fs::read_to_string(&defi_path).unwrap();
    let module = parser::parse(&defi_src).expect("parse defi_math.morph");
    let checked = typeck::check(module).expect("typecheck defi_math.morph");

    let mut defi_calls = Vec::new();
    for spec in &checked.spec.functions {
        for tc in &spec.tests {
            let args: Vec<JsonValue> = test_input_values(spec, &tc.input)
                .iter()
                .map(value_to_evm_json)
                .collect();
            let expect = value_to_evm_json(&eval_const_value(&tc.expected_output));
            defi_calls.push(json!({
                "fn": spec.name,
                "args": args,
                "expect": expect,
            }));
        }
    }
    assert_eq!(defi_calls.len(), 28, "DeFiMath carries 28 battery points");

    let defi_reverts = vec![
        // Gate Clause 3 explicitly named reverts:
        json!({ "fn": "fee_bps", "args": ["100", "10001"], "reason": "Precondition violated" }),
        json!({ "fn": "debit_balance", "args": ["100", "200"], "reason": "Precondition violated" }),
        json!({ "fn": "linear_vested", "args": ["1000", "50", "0"], "reason": "Precondition violated" }),
        // Additional domain boundary & overflow checks:
        json!({ "fn": "fee_bps", "args": ["-1", "100"], "reason": "Precondition violated" }),
        json!({ "fn": "fee_bps", "args": ["100", "-1"], "reason": "Precondition violated" }),
        json!({ "fn": "debit_balance", "args": ["100", "-5"], "reason": "Precondition violated" }),
        json!({ "fn": "bounded_payout", "args": ["50", "100", "10"], "reason": "Precondition violated" }),
        json!({ "fn": "linear_vested", "args": ["-1", "50", "100"], "reason": "Precondition violated" }),
        json!({ "fn": "can_withdraw", "args": ["-1", "10", "100"], "reason": "Precondition violated" }),
        json!({ "fn": "can_withdraw", "args": ["100", "10", "-1"], "reason": "Precondition violated" }),
        json!({
            "fn": "fee_bps",
            "args": ["28948022309329048855892746252171976963317496166410141009864396001978282409984", "10000"],
            "panic": 17
        }),
    ];

    let mut contracts = vec![json!({
        "file": "DeFiMath.sol",
        "contract": "DeFiMath",
        "source": defi_sol,
        "calls": defi_calls,
        "reverts": defi_reverts,
    })];

    // 3b. All 18 direct-ABI stdlib entries: cross-check every frozen in-domain battery point
    //     against Morphic's own interpreter (`eval::eval`) AND execute identically on EVM.
    let specs = stdlib::stdlib_specs();
    let mut stdlib_call_count = 0usize;
    let mut stdlib_revert_count = 0usize;

    for entry in DIRECT_ABI_STDLIB {
        let spec = specs.iter().find(|s| s.name == *entry).unwrap().clone();
        let body = stdlib::shipped_body(entry).unwrap();
        let src = codegen::generate(&make_cand(spec.clone(), body.clone()), "solidity").unwrap();

        let mut calls = Vec::new();
        let mut reverts = Vec::new();
        for tc in &spec.tests {
            let input_vals = test_input_values(&spec, &tc.input);
            let expect_val = eval_const_value(&tc.expected_output);
            let env: HashMap<String, Value> = spec
                .params
                .iter()
                .zip(input_vals.iter())
                .map(|(p, v)| (p.name.clone(), v.clone()))
                .collect();

            // Two spec-silent require: exclusions (recorded since Phase 159/170).
            if (*entry == "int_abs" && input_vals == vec![Value::Int(i64::MIN)])
                || (*entry == "int_clamp"
                    && input_vals == vec![Value::Int(5), Value::Int(10), Value::Int(0)])
            {
                reverts.push(json!({
                    "fn": entry,
                    "args": input_vals.iter().map(value_to_evm_json).collect::<Vec<_>>(),
                    "reason": "Precondition violated",
                }));
                stdlib_revert_count += 1;
                continue;
            }

            // Cross-check against Morphic's interpreter directly!
            let interp = eval::eval(&body, &env)
                .unwrap_or_else(|e| panic!("{entry}: interpreter eval failed: {e:?}"));
            assert_eq!(
                interp, expect_val,
                "{entry}: interpreter agrees with spec test row"
            );

            calls.push(json!({
                "fn": entry,
                "args": input_vals.iter().map(value_to_evm_json).collect::<Vec<_>>(),
                "expect": value_to_evm_json(&interp),
            }));
            stdlib_call_count += 1;
        }

        let cname = pascal_name(entry);
        contracts.push(json!({
            "file": format!("{cname}.sol"),
            "contract": cname,
            "source": src,
            "calls": calls,
            "reverts": reverts,
        }));
    }

    assert_eq!(stdlib_call_count, 122, "18 direct-ABI stdlib entries carry 122 in-domain points");
    assert_eq!(stdlib_revert_count, 2, "2 spec-silent require: exclusions revert on EVM");

    let summary = run_evm_runner(
        &harness_dir,
        "clause3-evm-exec",
        &json!({ "contracts": contracts }),
    );
    assert!(
        summary.contains("contracts=19")
            && summary.contains("calls=150")
            && summary.contains("reverts=13")
            && summary.contains("warnings=0"),
        "expected 19 contracts, 150 calls, 13 reverts, 0 warnings: {summary}"
    );
    let _ = std::fs::remove_dir_all(&out_dir);
    println!("P171|clause3|PASS {summary}");
}

// ── Clause 4: Proof-carrying contract loop ─────────────────

#[test]
fn p171_clause4_proof_carrying_contract_loop() {
    let _serial = gate_serial();
    let out_dir = scratch_dir("c4-pcc");
    let defi_path = repo_root().join("examples/contracts/defi_math.morph");

    let build = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args([
            "build",
            "--contract-from",
            defi_path.to_str().unwrap(),
            "--out",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run morphic build --contract-from");
    assert!(
        build.status.success(),
        "build --contract-from failed:\nstderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let sol_path = out_dir.join("DeFiMath.sol");
    let sol_text = std::fs::read_to_string(&sol_path).expect("DeFiMath.sol exists");

    for entry in DEFI_EXHIBITS {
        let bundle_path = out_dir.join(format!("{entry}.bundle.json"));
        let bytes = std::fs::read(&bundle_path)
            .unwrap_or_else(|e| panic!("companion bundle {entry}.bundle.json missing: {e}"));
        let digest = registry::sha256_hex(&bytes);
        let bundle: JsonValue = serde_json::from_slice(&bytes).expect("bundle parses as JSON");
        assert_eq!(
            bundle["bundleVersion"], 1,
            "{entry}: bundleVersion stays 1"
        );
        assert_eq!(
            bundle["entry"].as_str(),
            Some(*entry),
            "{entry}: bundle names its entry"
        );
        assert!(
            sol_text.contains(&format!("bundle: {entry}.bundle.json | bundleSha256: {digest}")),
            "{entry}: DeFiMath.sol NatSpec carries exact sha256 {digest}"
        );

        let verify = Command::new(env!("CARGO_BIN_EXE_morphic"))
            .args(["verify-bundle", bundle_path.to_str().unwrap()])
            .output()
            .expect("run morphic verify-bundle");
        assert!(
            verify.status.success(),
            "{entry}: verify-bundle failed: {}",
            String::from_utf8_lossy(&verify.stderr)
        );
        let vstdout = String::from_utf8_lossy(&verify.stdout);
        assert!(
            vstdout.contains("the claim reproduces"),
            "{entry}: verify-bundle reports reproduction: {vstdout}"
        );
        if cfg!(feature = "z3-support") {
            assert_eq!(
                bundle["tier"], "tier-1",
                "{entry}: Z3 build emits Tier-1 companion bundle"
            );
            assert_eq!(bundle["verdictBasis"], "z3-verified");
        } else {
            assert_eq!(
                bundle["tier"], "tier-2",
                "{entry}: no-Z3 build emits Tier-2 companion bundle honestly"
            );
        }
    }

    // Without Z3, a Tier-1 shipped bundle MUST refuse honestly by name as unexamined.
    if !cfg!(feature = "z3-support") {
        let tier1_path = repo_root().join("packages/bundles/int_clamp.bundle.json");
        let verify_t1 = Command::new(env!("CARGO_BIN_EXE_morphic"))
            .args(["verify-bundle", tier1_path.to_str().unwrap()])
            .output()
            .expect("run verify-bundle on Tier-1 bundle without Z3");
        assert!(
            !verify_t1.status.success(),
            "no-Z3 build must refuse Tier-1 bundle"
        );
        let err = String::from_utf8_lossy(&verify_t1.stderr);
        assert!(
            err.contains("unexamined"),
            "no-Z3 Tier-1 refusal names unexamined: {err}"
        );
    }

    // Tamper test: modify `fee_bps.bundle.json` body IR to constant `0`.
    let fee_path = out_dir.join("fee_bps.bundle.json");
    let orig_bytes = std::fs::read(&fee_path).unwrap();
    let orig_sha = registry::sha256_hex(&orig_bytes);
    let mut tampered: JsonValue = serde_json::from_slice(&orig_bytes).unwrap();
    tampered["body"] = json!("morphic-ir/1\n(int 0)\n");
    let tampered_bytes = serde_json::to_vec_pretty(&tampered).unwrap();
    let tampered_sha = registry::sha256_hex(&tampered_bytes);
    assert_ne!(
        tampered_sha, orig_sha,
        "tampered bundle hash diverges from DeFiMath.sol NatSpec"
    );
    assert!(
        !sol_text.contains(&format!("bundleSha256: {tampered_sha}")),
        "DeFiMath.sol NatSpec does not match tampered sha256"
    );
    let tampered_path = out_dir.join("fee_bps.tampered.bundle.json");
    std::fs::write(&tampered_path, &tampered_bytes).unwrap();
    let tamper_out = Command::new(env!("CARGO_BIN_EXE_morphic"))
        .args(["verify-bundle", tampered_path.to_str().unwrap()])
        .output()
        .expect("run verify-bundle on tampered bundle");
    assert!(
        !tamper_out.status.success(),
        "tampered bundle must be refused by verify-bundle"
    );
    let tamper_err = String::from_utf8_lossy(&tamper_out.stderr);
    assert!(
        !tamper_err.trim().is_empty(),
        "tampered bundle refusal states its reason"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
    println!("P171|clause4|PASS 6/6 companion bundles sha256-bound & re-verified; tamper refused");
}

// ── Clause 5: Footprint, standings, CI wiring, and lockstep ─

fn dependency_names(toml: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut in_deps = false;
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line.ends_with("dependencies]");
            continue;
        }
        if in_deps && !line.is_empty() && !line.starts_with('#') {
            if let Some((key, _)) = line.split_once('=') {
                let name = key
                    .trim()
                    .trim_matches('"')
                    .split('.')
                    .next()
                    .unwrap()
                    .trim();
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names
}

fn lock_package_names(lock: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for block in lock.split("[[package]]").skip(1) {
        if let Some(name) = block.lines().find_map(|l| {
            l.trim()
                .strip_prefix("name = \"")
                .and_then(|r| r.strip_suffix('"'))
        }) {
            names.insert(name.to_string());
        }
    }
    names
}

fn dir_mass(dir: &Path) -> (usize, usize) {
    let mut files = 0usize;
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

fn version_of(toml: &str) -> String {
    toml.lines()
        .find_map(|l| l.trim().strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("a version line")
        .to_string()
}

#[test]
fn p171_clause5_footprint_standings_and_lockstep() {
    let root = repo_root();

    // 5a. Frozen closing line in IKONIK_18_SCOPE.md stands verbatim and unamended.
    let scope = std::fs::read_to_string(root.join("IKONIK_18_SCOPE.md")).unwrap();
    let flat_scope: String = scope.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat_scope.contains(
            "\"1.8 closes when a Morphic specification compiles to a zero-warning Solidity \
             `pure` contract whose `require:` preconditions revert on-chain, whose EVM bytecode \
             agrees point-for-point with the verified battery, and whose accompanying PCC bundles \
             let any auditor re-run the Z3 proof from scratch.\""
        ),
        "IKONIK_18_SCOPE.md closing line stands verbatim"
    );

    // 5b. Zero new Cargo dependencies since v1.7.0 (standing-13 + wasm32 getrandom;
    //     plus exact Cargo.toml / Cargo.lock diff when git tag v1.7.0 is present).
    let head_toml = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let expected_deps: BTreeSet<String> = [
        "anyhow",
        "clap",
        "colored",
        "getrandom",
        "indicatif",
        "llama-cpp-2",
        "rand",
        "reqwest",
        "serde",
        "serde_json",
        "sha2",
        "thiserror",
        "tokio",
        "z3",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(
        dependency_names(&head_toml),
        expected_deps,
        "Cargo.toml holds exactly the standing-13 native + wasm32 getrandom dependencies"
    );

    let head_lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap();
    let base_toml_out = Command::new("git")
        .args(["show", "v1.7.0:Cargo.toml"])
        .current_dir(&root)
        .output()
        .expect("git show v1.7.0:Cargo.toml");
    if base_toml_out.status.success() {
        let base_toml = String::from_utf8(base_toml_out.stdout).unwrap();
        assert_eq!(
            dependency_names(&head_toml),
            dependency_names(&base_toml),
            "zero new Cargo dependencies since v1.7.0"
        );
        let base_lock_out = Command::new("git")
            .args(["show", "v1.7.0:Cargo.lock"])
            .current_dir(&root)
            .output()
            .expect("git show v1.7.0:Cargo.lock");
        assert!(base_lock_out.status.success());
        let base_lock = String::from_utf8(base_lock_out.stdout).unwrap();
        assert_eq!(
            lock_package_names(&head_lock),
            lock_package_names(&base_lock),
            "Cargo.lock carries zero new packages since v1.7.0"
        );
    }

    // 5c. Standing budgets unmoved.
    let budget_text = std::fs::read_to_string(root.join("playground/budget.txt")).unwrap();
    let budget: usize = budget_text.lines().next().unwrap().trim().parse().unwrap();
    assert_eq!(budget, 771_072, "playground budget frozen at 771,072 B");

    let (npm_files, npm_bytes) = dir_mass(&root.join("packages/npm"));
    assert_eq!(npm_files, 77, "npm package carries 77 files");
    assert!(
        npm_bytes <= 76_800,
        "npm package {npm_bytes} B <= 76,800 B"
    );

    let lib_wasm = std::fs::read(root.join("packages/component/morphic-library.wasm")).unwrap();
    assert_eq!(
        &lib_wasm[..8],
        &[0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00]
    );
    assert!(
        lib_wasm.len() <= 106_496,
        "library component {} B <= 106,496 B",
        lib_wasm.len()
    );

    let cb_wasm =
        std::fs::read(root.join("packages/component-callbacks/morphic-callbacks.wasm")).unwrap();
    assert_eq!(
        &cb_wasm[..8],
        &[0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00]
    );
    assert!(
        cb_wasm.len() <= 90_112,
        "callback component {} B <= 90,112 B",
        cb_wasm.len()
    );

    let index_bytes = std::fs::read(root.join("registry/index.json")).unwrap();
    assert!(
        index_bytes.len() <= 12_288,
        "registry index {} B <= 12,288 B",
        index_bytes.len()
    );
    let (reg_files, reg_bytes) = dir_mass(&root.join("registry/bundles"));
    assert_eq!(reg_files, 37, "registry catalog holds 37 bundles");
    assert!(
        reg_bytes <= 295_000,
        "registry catalog {reg_bytes} B <= 295,000 B"
    );

    // 5d. CI Solidity + EVM step wired in .github/workflows/ci.yml.
    let ci_yml = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    assert!(
        ci_yml.contains("Solidity compile + EVM execution (1.8)")
            && ci_yml.contains("phase170_evm_harness")
            && ci_yml.contains("phase171_gate")
            && ci_yml.contains("--contract-from examples/contracts/defi_math.morph"),
        "CI workflow wires the 1.8 Solidity + EVM step"
    );

    // 5e. Lockstep across every version-bearing surface.
    let crate_version = version_of(&head_toml);
    let lock_version = head_lock
        .split("[[package]]")
        .find(|b| b.contains("name = \"morphic\""))
        .map(version_of)
        .expect("Cargo.lock has morphic package");
    assert_eq!(lock_version, crate_version, "Cargo.lock lockstep");

    let npm_pkg: JsonValue = serde_json::from_str(
        &std::fs::read_to_string(root.join("packages/npm/package.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(npm_pkg["version"].as_str(), Some(crate_version.as_str()));
    let npm_manifest: JsonValue = serde_json::from_str(
        &std::fs::read_to_string(root.join("packages/npm/morphic.manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        npm_manifest["packageVersion"].as_str(),
        Some(crate_version.as_str())
    );
    assert_eq!(
        npm_manifest["morphicVersion"].as_str(),
        Some(crate_version.as_str())
    );

    let rust_toml = std::fs::read_to_string(root.join("packages/rust/Cargo.toml")).unwrap();
    assert_eq!(version_of(&rust_toml), crate_version, "facade lockstep");
    let rust_manifest: JsonValue = serde_json::from_str(
        &std::fs::read_to_string(root.join("packages/rust/morphic.manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        rust_manifest["packageVersion"].as_str(),
        Some(crate_version.as_str())
    );

    for (cargo_rel, wit_rel) in [
        (
            "packages/component/Cargo.toml",
            "packages/component/wit/world.wit",
        ),
        (
            "packages/component-callbacks/Cargo.toml",
            "packages/component-callbacks/wit/world.wit",
        ),
    ] {
        let c = std::fs::read_to_string(root.join(cargo_rel)).unwrap();
        assert_eq!(version_of(&c), crate_version, "{cargo_rel} lockstep");
        let w = std::fs::read_to_string(root.join(wit_rel)).unwrap();
        assert!(
            w.contains(&format!("package morphic:library@{crate_version};")),
            "{wit_rel} lockstep"
        );
    }

    let vscode_pkg: JsonValue = serde_json::from_str(
        &std::fs::read_to_string(root.join("packages/vscode/package.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(vscode_pkg["version"].as_str(), Some(crate_version.as_str()));
    assert!(
        root.join(format!("packages/vscode/morphic-{crate_version}.vsix"))
            .exists(),
        "packages/vscode/morphic-{crate_version}.vsix exists"
    );

    println!(
        "P171|clause5|PASS version={crate_version} deps=unchanged(13) npm={npm_bytes}B lib={}B cb={}B",
        lib_wasm.len(),
        cb_wasm.len()
    );
}
