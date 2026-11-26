//! Phase 170 — Real `solc` (0 errors, 0 warnings) + Real EVM Bytecode
//! Execution + Runtime `require(...)` Revert Harness
//! (`IKONIK_18_SCOPE.md` §7, §8 clauses 2 & 3).
//!
//! Drives `tests/solidity_evm_runner.cjs` with pinned `solc@0.8.28`,
//! `@ethereumjs/evm@3.1.1`, and `ethers@6.13.4` installed ONLY inside
//! scratch `target/solidity-harness/` (never global, never committed
//! `node_modules/` — the Phase 160 `jco` precedent).
//!
//! What is verified on real EVM bytecode:
//! 1. The assembled multi-spec `DeFiMath.sol` (`examples/contracts/defi_math.morph`):
//!    - Compiles under `solc@0.8.28` with 0 errors and 0 warnings.
//!    - Executes 100% of all 28 frozen battery points across all 6 functions.
//!    - Reverts with `"Precondition violated"` on 10 out-of-domain inputs
//!      violating `require:` across every guarded function.
//!    - Reverts with Solidity `Panic(0x11)` on 256-bit arithmetic overflow
//!      (verifying the D8a proof-boundary disclosure on real EVM).
//! 2. All 18 direct-ABI stdlib entries (`int_abs..int_is_odd`,
//!    `string_is_empty`, `list_is_empty..range`):
//!    - Every single contract compiles under `solc@0.8.28` with 0 errors
//!      and 0 warnings.
//!    - 100% of every entry's frozen in-domain battery points from
//!      `stdlib/*.morph` execute identically on EVM.
//!    - The 2 spec-silent `require:` exclusions (`int_abs(i64::MIN)` and
//!      `int_clamp(5, 10, 0)`) revert at runtime with `"Precondition violated"`.
//! 3. Transitive `require:` helper (`list_is_sorted`), string equality
//!    (`keccak256(bytes(...))`), and fixed-width `U64` (`uint64`):
//!    - Compile with 0 warnings, answer exact values, and revert when
//!      the transitive `require: list_is_sorted(xs)` guard fails.

#![cfg(not(feature = "wasm-playground"))]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use morphic::codegen;
use morphic::codegen::solidity;
use morphic::spec::ast::{BinOp, Expr, FunctionSpec, UnaryOp};
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, Provenance, ScoreBreakdown,
};
use morphic::synthesis::eval;
use serde_json::{json, Value as JsonValue};

fn harness_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn npm_cmd() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

/// Ensure pinned `solc@0.8.28`, `@ethereumjs/evm@3.1.1`, and `ethers@6.13.4`
/// exist in `target/solidity-harness/`. Returns `None` if `node`/`npm` are
/// absent on the host (reporting honestly, never panicking on host absence).
fn ensure_harness_dir() -> Option<PathBuf> {
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
        println!("P170|evm|node/npm ABSENT in this environment; EVM harness skipped");
        return None;
    }

    let dir = repo_root().join("target").join("solidity-harness");
    let solc_marker = dir.join("node_modules").join("solc").join("package.json");
    let evm_marker = dir
        .join("node_modules")
        .join("@ethereumjs")
        .join("evm")
        .join("package.json");
    let ethers_marker = dir.join("node_modules").join("ethers").join("package.json");

    if !(solc_marker.exists() && evm_marker.exists() && ethers_marker.exists()) {
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

fn run_evm_plan(harness_dir: &Path, label: &str, plan: &JsonValue) -> String {
    let plan_path = std::env::temp_dir().join(format!(
        "morphic-p170-{label}-{}.json",
        std::process::id()
    ));
    std::fs::write(&plan_path, serde_json::to_string_pretty(plan).unwrap())
        .expect("write evm plan json");

    let runner = repo_root().join("tests").join("solidity_evm_runner.cjs");
    let out = Command::new("node")
        .args([
            runner.to_str().unwrap(),
            harness_dir.to_str().unwrap(),
            plan_path.to_str().unwrap(),
        ])
        .output()
        .expect("node solidity_evm_runner.cjs runs");

    let _ = std::fs::remove_file(&plan_path);

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "EVM harness `{label}` failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("EVM-OK") && stdout.contains("warnings=0"),
        "EVM harness `{label}` missing EVM-OK/warnings=0 marker:\n{stdout}"
    );
    stdout.trim().to_string()
}

/// Evaluate a constant Morphic spec expression (test input or expected
/// output) into the JSON encoding consumed by `solidity_evm_runner.cjs`.
fn eval_const_expr_to_json(expr: &Expr) -> JsonValue {
    match expr {
        Expr::IntLit(n) => json!(n.to_string()),
        Expr::BoolLit(b) => json!(b),
        Expr::StringLit(s) => json!({ "str": s }),
        Expr::UnaryOp(UnaryOp::Neg, inner) => {
            let v = eval_const_i64(inner);
            json!((-v).to_string())
        }
        Expr::UnaryOp(UnaryOp::Not, inner) => {
            if let JsonValue::Bool(b) = eval_const_expr_to_json(inner) {
                json!(!b)
            } else {
                panic!("non-bool unary not in test literal: {expr:?}")
            }
        }
        Expr::BinOp(op, l, r) => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                let a = eval_const_i64(l);
                let b = eval_const_i64(r);
                let v = match op {
                    BinOp::Add => a.checked_add(b).expect("const add"),
                    BinOp::Sub => a.checked_sub(b).expect("const sub"),
                    BinOp::Mul => a.checked_mul(b).expect("const mul"),
                    BinOp::Div => a.checked_div(b).expect("const div"),
                    BinOp::Mod => a.checked_rem(b).expect("const mod"),
                    _ => unreachable!(),
                };
                json!(v.to_string())
            }
            _ => panic!("unsupported binop in test literal: {expr:?}"),
        },
        Expr::Call(name, args) if name == "List" => {
            JsonValue::Array(args.iter().map(eval_const_expr_to_json).collect())
        }
        other => panic!("unsupported test literal expression for EVM: {other:?}"),
    }
}

fn eval_const_i64(expr: &Expr) -> i64 {
    match expr {
        Expr::IntLit(n) => *n,
        Expr::UnaryOp(UnaryOp::Neg, inner) => -eval_const_i64(inner),
        Expr::BinOp(BinOp::Add, l, r) => eval_const_i64(l).checked_add(eval_const_i64(r)).unwrap(),
        Expr::BinOp(BinOp::Sub, l, r) => eval_const_i64(l).checked_sub(eval_const_i64(r)).unwrap(),
        Expr::BinOp(BinOp::Mul, l, r) => eval_const_i64(l).checked_mul(eval_const_i64(r)).unwrap(),
        Expr::BinOp(BinOp::Div, l, r) => eval_const_i64(l).checked_div(eval_const_i64(r)).unwrap(),
        Expr::BinOp(BinOp::Mod, l, r) => eval_const_i64(l).checked_rem(eval_const_i64(r)).unwrap(),
        other => panic!("expected integer constant in test literal, got: {other:?}"),
    }
}

fn test_input_args_json(spec: &FunctionSpec, input: &Expr) -> Vec<JsonValue> {
    match input {
        Expr::Call(name, args) if name == "__tuple__" && args.len() == spec.params.len() => {
            args.iter().map(eval_const_expr_to_json).collect()
        }
        other if spec.params.len() == 1 => vec![eval_const_expr_to_json(other)],
        other => panic!(
            "spec {}: cannot spread test input {other:?} across {} params",
            spec.name,
            spec.params.len()
        ),
    }
}

fn make_candidate(spec: FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 170,
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

fn pascal_contract_name(name: &str) -> String {
    let mut out = String::new();
    for part in name.split(|c: char| !c.is_ascii_alphanumeric()) {
        if part.is_empty() {
            continue;
        }
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    out
}

// ── 1. DeFiMath multi-spec contract: 0 warnings, 28 battery points, 10 reverts + Panic(0x11) ──

#[test]
fn p170_defi_math_compiles_zero_warnings_and_executes_on_evm() {
    let _guard = harness_serial();
    let Some(harness_dir) = ensure_harness_dir() else {
        return;
    };

    let src = std::fs::read_to_string(
        repo_root().join("examples/contracts/defi_math.morph"),
    )
    .expect("defi_math.morph reads");
    let assembly = solidity::assemble_user_contract(&src, 42)
        .expect("DeFiMath assembles cleanly");
    let parsed = typeck::check(parser::parse(&src).unwrap()).unwrap();

    let mut calls = Vec::new();
    for spec in &parsed.spec.functions {
        for t in &spec.tests {
            calls.push(json!({
                "fn": spec.name,
                "args": test_input_args_json(spec, &t.input),
                "expect": eval_const_expr_to_json(&t.expected_output),
            }));
        }
    }
    assert_eq!(calls.len(), 28, "28 frozen DeFiMath battery points");

    // 10 out-of-domain precondition reverts + 1 Solidity 0.8 Panic(0x11) overflow test.
    let reverts = vec![
        json!({ "fn": "fee_bps", "args": ["-1", "30"], "expectReason": "Precondition violated" }),
        json!({ "fn": "fee_bps", "args": ["100", "-1"], "expectReason": "Precondition violated" }),
        json!({ "fn": "fee_bps", "args": ["100", "10001"], "expectReason": "Precondition violated" }),
        json!({ "fn": "debit_balance", "args": ["100", "200"], "expectReason": "Precondition violated" }),
        json!({ "fn": "debit_balance", "args": ["100", "-5"], "expectReason": "Precondition violated" }),
        json!({ "fn": "bounded_payout", "args": ["500", "1000", "100"], "expectReason": "Precondition violated" }),
        json!({ "fn": "linear_vested", "args": ["-1", "50", "100"], "expectReason": "Precondition violated" }),
        json!({ "fn": "linear_vested", "args": ["1000", "50", "0"], "expectReason": "Precondition violated" }),
        json!({ "fn": "can_withdraw", "args": ["-1", "10", "100"], "expectReason": "Precondition violated" }),
        json!({ "fn": "can_withdraw", "args": ["100", "10", "-1"], "expectReason": "Precondition violated" }),
        // Overflow boundary (D8a): (2^254) * 100 overflows int256 -> Panic(0x11)
        json!({
            "fn": "fee_bps",
            "args": ["28948022309329048855892746252171976963317496166410141009864396001978282409984", "100"],
            "expectPanic": "0x11"
        }),
    ];

    let plan = json!({
        "contracts": [{
            "file": assembly.contract_file,
            "contract": assembly.contract_name,
            "source": assembly.contract_source,
            "calls": calls,
            "reverts": reverts,
        }]
    });

    let line = run_evm_plan(&harness_dir, "defi-math", &plan);
    println!("P170|defi-math|{line}");
}

// ── 2. All 18 direct-ABI stdlib entries: 0 warnings + 100% battery + 2 spec-silent reverts ──

#[test]
fn p170_all_18_direct_abi_stdlib_entries_compile_and_execute_on_evm() {
    let _guard = harness_serial();
    let Some(harness_dir) = ensure_harness_dir() else {
        return;
    };

    let direct_abi = [
        "int_abs",
        "int_min",
        "int_max",
        "int_clamp",
        "int_sign",
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

    let specs = stdlib::stdlib_specs();
    let mut contracts = Vec::new();
    let mut total_calls = 0usize;
    let mut total_reverts = 0usize;

    for name in direct_abi {
        let spec = specs
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("missing stdlib spec {name}"))
            .clone();
        let body = stdlib::shipped_body(name)
            .unwrap_or_else(|| panic!("missing shipped body {name}"));
        let cand = make_candidate(spec.clone(), body.clone());
        let sol_src = codegen::generate(&cand, "solidity")
            .unwrap_or_else(|e| panic!("stdlib {name} solidity codegen failed: {e}"));

        // Classify each frozen test row via Morphic's own test_outcomes:
        // Passed -> EVM call assertion; SpecSilent (precondition excluded) -> EVM revert assertion!
        let outcomes = eval::test_outcomes(&body, &spec);
        let mut calls = Vec::new();
        let mut reverts = Vec::new();

        for (t, outcome) in spec.tests.iter().zip(outcomes.iter()) {
            let args = test_input_args_json(&spec, &t.input);
            match outcome {
                eval::TestOutcome::Passed => {
                    calls.push(json!({
                        "fn": name,
                        "args": args,
                        "expect": eval_const_expr_to_json(&t.expected_output),
                    }));
                    total_calls += 1;
                }
                eval::TestOutcome::SpecSilent(_) => {
                    reverts.push(json!({
                        "fn": name,
                        "args": args,
                        "expectReason": "Precondition violated",
                    }));
                    total_reverts += 1;
                }
                other => panic!("stdlib {name}: unexpected test outcome {other:?}"),
            }
        }

        let cname = pascal_contract_name(name);
        contracts.push(json!({
            "file": format!("{cname}.sol"),
            "contract": cname,
            "source": sol_src,
            "calls": calls,
            "reverts": reverts,
        }));
    }

    assert_eq!(contracts.len(), 18);
    assert_eq!(total_reverts, 2, "int_abs(i64::MIN) + int_clamp(5,10,0)");
    assert!(total_calls >= 120, "expected full stdlib batteries, got {total_calls}");

    let plan = json!({ "contracts": contracts });
    let line = run_evm_plan(&harness_dir, "stdlib-18", &plan);
    println!("P170|stdlib-18|calls={total_calls} reverts={total_reverts} {line}");
}

// ── 3. Transitive require: helper, string keccak256 eq, and fixed-width U64 on real EVM ──

#[test]
fn p170_transitive_require_string_eq_and_u64_execute_on_evm() {
    let _guard = harness_serial();
    let Some(harness_dir) = ensure_harness_dir() else {
        return;
    };

    // (a) Transitive helper in `require: list_is_sorted(xs)`
    let sorted_src = r#"SortedHead {
spec first_of_sorted {
    input: xs: List<Int>
    output: Int
    require: len(xs) > 0
    require: list_is_sorted(xs)
    constraint: output == xs[0]
}
}"#;
    let c1 = make_candidate(
        typeck::check(parser::parse(sorted_src).unwrap()).unwrap().spec.functions[0].clone(),
        IRNode::Index(
            Box::new(IRNode::Var("xs".into())),
            Box::new(IRNode::Literal(IRLiteral::Int(0))),
        ),
    );
    let sol1 = codegen::generate(&c1, "solidity").unwrap();

    // (b) String equality via keccak256(bytes(...))
    let str_src = r#"StrEq {
spec same_str {
    input: a: String, b: String
    output: Bool
    constraint: output == (a == b)
}
}"#;
    let c2 = make_candidate(
        typeck::check(parser::parse(str_src).unwrap()).unwrap().spec.functions[0].clone(),
        IRNode::BinOp(
            IRBinOp::Eq,
            Box::new(IRNode::Var("a".into())),
            Box::new(IRNode::Var("b".into())),
        ),
    );
    let sol2 = codegen::generate(&c2, "solidity").unwrap();

    // (c) Fixed-width U64 + Solidity keyword params
    let fw_src = r#"FixedMath {
spec add_u64 {
    input: address: U64, memory: U64
    output: U64
    constraint: output == address + memory
}
}"#;
    let c3 = make_candidate(
        typeck::check(parser::parse(fw_src).unwrap()).unwrap().spec.functions[0].clone(),
        IRNode::BinOp(
            IRBinOp::Add,
            Box::new(IRNode::Var("address".into())),
            Box::new(IRNode::Var("memory".into())),
        ),
    );
    let sol3 = codegen::generate(&c3, "solidity").unwrap();

    let plan = json!({
        "contracts": [
            {
                "file": "FirstOfSorted.sol",
                "contract": "FirstOfSorted",
                "source": sol1,
                "calls": [
                    { "fn": "first_of_sorted", "args": [["10", "20", "30"]], "expect": "10" },
                    { "fn": "first_of_sorted", "args": [["-5", "0", "5"]], "expect": "-5" }
                ],
                "reverts": [
                    { "fn": "first_of_sorted", "args": [[]], "expectReason": "Precondition violated" },
                    { "fn": "first_of_sorted", "args": [["3", "1", "2"]], "expectReason": "Precondition violated" }
                ]
            },
            {
                "file": "SameStr.sol",
                "contract": "SameStr",
                "source": sol2,
                "calls": [
                    { "fn": "same_str", "args": [{ "str": "morphic" }, { "str": "morphic" }], "expect": true },
                    { "fn": "same_str", "args": [{ "str": "morphic" }, { "str": "solidity" }], "expect": false },
                    { "fn": "same_str", "args": [{ "str": "" }, { "str": "" }], "expect": true }
                ],
                "reverts": []
            },
            {
                "file": "AddU64.sol",
                "contract": "AddU64",
                "source": sol3,
                "calls": [
                    { "fn": "add_u64", "args": ["10", "32"], "expect": "42" },
                    { "fn": "add_u64", "args": ["0", "18446744073709551615"], "expect": "18446744073709551615" }
                ],
                "reverts": [
                    { "fn": "add_u64", "args": ["18446744073709551615", "1"], "expectPanic": "0x11" }
                ]
            }
        ]
    });

    let line = run_evm_plan(&harness_dir, "extras", &plan);
    println!("P170|extras|{line}");
}
