//! Phase 167 — the 1.8 Smart-Contract Target (`--target solidity`)
//! scope audit probes, measured against master (`80c10a2`, v1.7.0).
//! Zero product code is modified; every empirical claim in
//! `IKONIK_18_AUDIT.md` carries one of this file's `P167|` lines or
//! an exact file/line citation.
//!
//! What is measured, in order:
//! 1. Current target dispatch & CLI surface: `codegen::generate`
//!    refuses `"solidity"` and `"sol"` as `UnsupportedTarget`; the
//!    DTO API's `WinnerReport.renders` currently holds 6 targets.
//! 2. Standard-library type & `require:` inventory across all 35
//!    shipped entries: exact counts of scalar (`Int`/`Bool`),
//!    value-mappable (`+String/+List<Int>`), `Char`, `Option/Result`,
//!    and higher-order (`fn(...)`) entries.
//! 3. `Int` vs `int256`/`uint256` semantic gaps:
//!    - Parser integer literal ceiling (`i64::MAX`, cannot parse
//!      `i64::MAX + 1` or 100 ETH in wei = 10^20).
//!    - Z3 `U64` has NO non-negativity assumption (`U64` binds to
//!      unbounded `z3::ast::Int`, refuting `x >= 0` at `x = -1`).
//!    - `U256` / `Address` parse as `TypeRef::Named(...)` and bind
//!      `Sym::Opaque` in Z3 (inconclusive).
//!    - Negative integer division divergence: Z3 `Int::div` is
//!      Euclidean (`(-7) div 2 == -4`), whereas the interpreter,
//!      Rust `i64`, and Solidity `int256` truncate toward zero
//!      (`(-7) / 2 == -3`); on non-negative operands (`x >= 0`,
//!      `d > 0`) all layers agree.
//! 4. `require:` AST shapes & Rust renderer fallback audit:
//!    quantified `require: forall ...` degrades to `true` in Rust's
//!    `constraint_to_rust`, and `len(xs)` / `xs[0]` / `-x` shapes
//!    probe exact AST variants and current Rust codegen behavior.
//! 5. Local toolchain inventory (`solc`, `solcjs`, `forge`, `anvil`,
//!    `cast`, `node`, `npm`, `npx`): reported factually, never
//!    asserting host-specific installation (CI-portable discipline).
//! 6. Candidate pure/view smart-contract exhibits: DeFi/contract
//!    specs synthesized via the standing seeded pipeline (seed 42),
//!    Z3-verified, bundled via `pcc::produce_bundle`, and
//!    re-verified via `pcc::verify_bundle_text` with zero new
//!    product code.

#![cfg(not(feature = "wasm-playground"))]

use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Mutex;

use morphic::codegen;
use morphic::codegen::bindings::{BindingInput, CrownRoute};
use morphic::pcc;
use morphic::spec::ast::{Constraint, Expr, FunctionSpec, TypeRef};
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, Provenance, ScoreBreakdown,
};
use morphic::synthesis::eval;
use morphic::verify::verifier::{VerificationStatus, Verifier};

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn dummy_candidate(spec: FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 167,
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

fn contains_type(ty: &TypeRef, pred: &dyn Fn(&TypeRef) -> bool) -> bool {
    if pred(ty) {
        return true;
    }
    match ty {
        TypeRef::List(t)
        | TypeRef::Set(t)
        | TypeRef::Option(t)
        | TypeRef::Array(t, _)
        | TypeRef::Ref(t)
        | TypeRef::Stream(t) => contains_type(t, pred),
        TypeRef::Map(k, v) | TypeRef::Result(k, v) => {
            contains_type(k, pred) || contains_type(v, pred)
        }
        TypeRef::Tuple(ts) => ts.iter().any(|t| contains_type(t, pred)),
        TypeRef::Function(args, ret) => {
            args.iter().any(|t| contains_type(t, pred)) || contains_type(ret, pred)
        }
        _ => false,
    }
}

// ── 1. Current codegen target dispatch & API renders ─────

#[test]
fn p167_target_dispatch_and_api_renders_baseline() {
    let _guard = crown_serial();
    let src = r#"Probe {
spec id_int {
    input: x: Int
    output: Int
    constraint: output == x
    test "zero" { input: 0 expect: 0 }
}
}"#;
    let checked = typeck::check(parser::parse(src).unwrap()).unwrap();
    let spec = checked.spec.functions[0].clone();
    let cand = dummy_candidate(spec, IRNode::Var("x".into()));

    for target in ["solidity", "sol"] {
        let err = codegen::generate(&cand, target)
            .expect_err("solidity target does not exist yet");
        let msg = err.to_string();
        assert!(
            msg.contains("Unsupported target language"),
            "unexpected error for {target}: {msg}"
        );
        println!("P167|target|{target} refused: {msg}");
    }

    let report = morphic::api::synthesize_seeded(src, 42).expect("seeded run succeeds");
    let targets: Vec<&str> = report
        .winner
        .renders
        .iter()
        .map(|r| r.target.as_str())
        .collect();
    assert_eq!(
        targets,
        vec!["rust", "c", "wasm", "python", "javascript", "ir"],
        "standing 6 render targets"
    );
    println!(
        "P167|api-renders|count={} targets={}",
        targets.len(),
        targets.join(",")
    );
}

// ── 2. Stdlib type & require: inventory against Solidity ──

#[test]
fn p167_stdlib_solidity_type_inventory() {
    let specs = stdlib::stdlib_specs();
    assert_eq!(specs.len(), 35, "35 frozen stdlib entries");

    let mut scalar_int_bool = Vec::new();
    let mut direct_abi_all = Vec::new();
    let mut needs_char = Vec::new();
    let mut needs_option_or_result = Vec::new();
    let mut needs_function = Vec::new();
    let mut with_require = Vec::new();

    for s in &specs {
        let all_tys: Vec<&TypeRef> = s
            .params
            .iter()
            .map(|p| &p.ty)
            .chain(std::iter::once(&s.return_type))
            .collect();

        let has_char = all_tys
            .iter()
            .any(|t| contains_type(t, &|x| matches!(x, TypeRef::Char)));
        let has_opt_res = all_tys.iter().any(|t| {
            contains_type(t, &|x| matches!(x, TypeRef::Option(_) | TypeRef::Result(_, _)))
        });
        let has_fn = all_tys
            .iter()
            .any(|t| contains_type(t, &|x| matches!(x, TypeRef::Function(_, _))));
        let only_int_bool = all_tys
            .iter()
            .all(|t| matches!(t, TypeRef::Int | TypeRef::Bool));

        if only_int_bool {
            scalar_int_bool.push(s.name.as_str());
        }
        if !has_char && !has_opt_res && !has_fn {
            direct_abi_all.push(s.name.as_str());
        }
        if has_char {
            needs_char.push(s.name.as_str());
        }
        if has_opt_res {
            needs_option_or_result.push(s.name.as_str());
        }
        if has_fn {
            needs_function.push(s.name.as_str());
        }
        if !s.preconditions.is_empty() {
            with_require.push((s.name.as_str(), s.preconditions.len()));
        }
    }

    assert_eq!(scalar_int_bool.len(), 7);
    assert_eq!(direct_abi_all.len(), 18);
    assert_eq!(needs_char.len(), 6);
    assert_eq!(needs_option_or_result.len(), 7);
    assert_eq!(needs_function.len(), 5);
    assert_eq!(with_require.len(), 3);

    println!(
        "P167|stdlib-types|total=35 scalar_int_bool={} ({}) direct_abi={} ({})",
        scalar_int_bool.len(),
        scalar_int_bool.join(","),
        direct_abi_all.len(),
        direct_abi_all.join(",")
    );
    println!(
        "P167|stdlib-gaps|char={} ({}) option_or_result={} ({}) higher_order={} ({})",
        needs_char.len(),
        needs_char.join(","),
        needs_option_or_result.len(),
        needs_option_or_result.join(","),
        needs_function.len(),
        needs_function.join(",")
    );
    println!(
        "P167|stdlib-requires|count={} entries={:?}",
        with_require.len(),
        with_require
    );
}

// ── 3. Int vs int256/uint256 semantic gap probes ─────────

#[test]
fn p167_int_literal_ceiling_and_type_surface() {
    // (a) i64::MAX parses; i64::MAX + 1 and 100 ETH in wei (10^20) fail parse.
    let ok_src = r#"LitMax {
spec max_i64 {
    input: x: Int
    output: Int
    constraint: output == 9223372036854775807
    test { input: 0 expect: 9223372036854775807 }
}
}"#;
    assert!(parser::parse(ok_src).is_ok(), "i64::MAX must parse");

    let over_i64 = r#"LitOver {
spec over_i64 {
    input: x: Int
    output: Int
    constraint: output == 9223372036854775808
}
}"#;
    let err_over = parser::parse(over_i64)
        .expect_err("i64::MAX + 1 cannot parse in Expr::IntLit(i64)");

    let wei_100_eth = r#"LitWei {
spec hundred_eth {
    input: x: Int
    output: Int
    constraint: output == 100000000000000000000
}
}"#;
    let err_wei = parser::parse(wei_100_eth)
        .expect_err("10^20 wei cannot parse in Expr::IntLit(i64)");

    println!(
        "P167|int-literal|i64_max=ok i64_max_plus_1_err=\"{}\" wei_100eth_err=\"{}\"",
        err_over, err_wei
    );

    // (b) Check how U64, U256, Address parse into TypeRef.
    let ty_src = r#"Types {
spec sig_probe {
    input: a: Int, b: U64, c: U256, d: Address
    output: Bool
    constraint: output == true
}
}"#;
    let parsed = parser::parse(ty_src).expect("unknown type names parse as Named");
    let f = &parsed.functions[0];
    assert_eq!(f.params[0].ty, TypeRef::Int);
    assert_eq!(f.params[1].ty, TypeRef::U64);
    assert_eq!(f.params[2].ty, TypeRef::Named("U256".into()));
    assert_eq!(f.params[3].ty, TypeRef::Named("Address".into()));
    println!(
        "P167|type-parse|Int={:?} U64={:?} U256={:?} Address={:?}",
        f.params[0].ty, f.params[1].ty, f.params[2].ty, f.params[3].ty
    );
}

#[cfg(feature = "z3-support")]
#[test]
fn p167_z3_unsigned_opaque_and_division_gaps() {
    let _guard = crown_serial();

    // (a) U64 parameter has NO implicit >= 0 bound in Z3!
    let u64_src = r#"U64Probe {
spec u64_nonneg {
    input: x: U64
    output: Bool
    constraint: output == true
}
}"#;
    let checked = typeck::check(parser::parse(u64_src).unwrap()).unwrap();
    let spec = checked.spec.functions[0].clone();
    let body = IRNode::BinOp(
        IRBinOp::Ge,
        Box::new(IRNode::Var("x".into())),
        Box::new(IRNode::Literal(IRLiteral::Int(0))),
    );
    let cand = dummy_candidate(spec.clone(), body);
    let res = Verifier::with_specs(&checked.spec.functions).verify_one(&cand, &spec);
    assert_eq!(
        res.status,
        VerificationStatus::VerificationFailed,
        "Z3 treats U64 as unbounded signed Int with no >= 0 bound"
    );
    println!(
        "P167|z3-u64|body=(x>=0) against output==true status={:?} counterexample={:?}",
        res.status, res.counterexample
    );

    // (b) U256 / Address parse as Named -> bind Sym::Opaque -> Inconclusive.
    let u256_src = r#"U256Probe {
spec u256_id {
    input: x: U256
    output: U256
    constraint: output == x
}
}"#;
    let checked2 = typeck::check(parser::parse(u256_src).unwrap()).unwrap();
    let spec2 = checked2.spec.functions[0].clone();
    let cand2 = dummy_candidate(spec2.clone(), IRNode::Var("x".into()));
    let res2 = Verifier::with_specs(&checked2.spec.functions).verify_one(&cand2, &spec2);
    assert_eq!(
        res2.status,
        VerificationStatus::Inconclusive,
        "Named(\"U256\") binds Sym::Opaque in Z3"
    );
    println!(
        "P167|z3-named-u256|output==x status={:?}",
        res2.status
    );

    // (c) Negative division: Z3 Euclidean div (-7 div 2 == -4) vs
    // interpreter / Rust i64 / Solidity int256 truncating div (-7 / 2 == -3).
    let div_body = IRNode::BinOp(
        IRBinOp::Div,
        Box::new(IRNode::Literal(IRLiteral::Int(-7))),
        Box::new(IRNode::Literal(IRLiteral::Int(2))),
    );
    let interp_val = eval::eval(&div_body, &eval::Env::new()).unwrap();
    assert_eq!(interp_val, eval::Value::Int(-3));

    let neg_div_trunc_src = r#"DivProbe {
spec neg_div_trunc {
    input: dummy: Int
    output: Int
    constraint: output == (0 - 3)
}
}"#;
    let checked_trunc = typeck::check(parser::parse(neg_div_trunc_src).unwrap()).unwrap();
    let spec_trunc = checked_trunc.spec.functions[0].clone();
    let cand_trunc = dummy_candidate(spec_trunc.clone(), div_body.clone());
    let res_trunc =
        Verifier::with_specs(&checked_trunc.spec.functions).verify_one(&cand_trunc, &spec_trunc);

    let neg_div_euclid_src = r#"DivProbeEuclid {
spec neg_div_euclid {
    input: dummy: Int
    output: Int
    constraint: output == (0 - 4)
}
}"#;
    let checked_euclid = typeck::check(parser::parse(neg_div_euclid_src).unwrap()).unwrap();
    let spec_euclid = checked_euclid.spec.functions[0].clone();
    let cand_euclid = dummy_candidate(spec_euclid.clone(), div_body);
    let res_euclid = Verifier::with_specs(&checked_euclid.spec.functions)
        .verify_one(&cand_euclid, &spec_euclid);

    assert_eq!(res_trunc.status, VerificationStatus::VerificationFailed);
    assert_eq!(res_euclid.status, VerificationStatus::Verified);
    println!(
        "P167|z3-neg-div|interp=(-7/2)={:?} z3_vs_trunc(-3)={:?} z3_vs_euclid(-4)={:?}",
        interp_val, res_trunc.status, res_euclid.status
    );

    // (d) Non-negative division: require: a >= 0 && b > 0 — Z3 agrees with truncating div.
    let pos_div_src = r#"PosDivProbe {
spec pos_div {
    input: a: Int, b: Int
    output: Int
    require: a >= 0
    require: b > 0
    constraint: output == a / b
}
}"#;
    let checked_pos = typeck::check(parser::parse(pos_div_src).unwrap()).unwrap();
    let spec_pos = checked_pos.spec.functions[0].clone();
    let pos_body = IRNode::BinOp(
        IRBinOp::Div,
        Box::new(IRNode::Var("a".into())),
        Box::new(IRNode::Var("b".into())),
    );
    let cand_pos = dummy_candidate(spec_pos.clone(), pos_body);
    let res_pos =
        Verifier::with_specs(&checked_pos.spec.functions).verify_one(&cand_pos, &spec_pos);
    assert_eq!(res_pos.status, VerificationStatus::Verified);
    println!(
        "P167|z3-pos-div|require(a>=0,b>0) output==a/b status={:?}",
        res_pos.status
    );
}

// ── 4. `require:` AST shapes & Rust renderer fallback audit ──

#[test]
fn p167_require_ast_shapes_and_rust_fallbacks() {
    let src = r#"ReqProbe {
spec req_shapes {
    input: x: Int, lo: Int, hi: Int, xs: List<Int>
    output: Int
    require: lo <= hi
    require: x != 0 && lo >= 0
    require: list_is_sorted(xs)
    require: len(xs) > 0
    require: xs[0] >= 0
    require: hi >= (0 - x)
    require: forall i: Int in range(0, len(xs)): xs[i] >= 0
    constraint: output == x
}
}"#;
    let checked = typeck::check(parser::parse(src).unwrap()).unwrap();
    let spec = checked.spec.functions[0].clone();
    assert_eq!(spec.preconditions.len(), 7);

    let kinds: Vec<&str> = spec
        .preconditions
        .iter()
        .map(|c| match c {
            Constraint::True => "True",
            Constraint::Expr(Expr::BinOp(..)) => "Expr(BinOp)",
            Constraint::Expr(Expr::Call(..)) => "Expr(Call)",
            Constraint::Expr(Expr::Length(..)) => "Expr(Length)",
            Constraint::Expr(Expr::Index(..)) => "Expr(Index)",
            Constraint::Expr(Expr::UnaryOp(..)) => "Expr(UnaryOp)",
            Constraint::Expr(_) => "Expr(Other)",
            Constraint::Predicate(..) => "Predicate",
            Constraint::Forall { .. } => "Forall",
            Constraint::Exists { .. } => "Exists",
            Constraint::Implies(..) => "Implies",
            Constraint::And(..) => "And",
            Constraint::Or(..) => "Or",
            Constraint::Not(..) => "Not",
            Constraint::Eq(..) => "Eq",
            Constraint::Order(..) => "Order",
        })
        .collect();

    let cand = dummy_candidate(spec, IRNode::Var("x".into()));
    let rust_code = codegen::generate(&cand, "rust").expect("rust codegen succeeds");
    let assert_lines: Vec<&str> = rust_code
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("debug_assert!("))
        .collect();
    assert_eq!(assert_lines.len(), 7);

    // Measure the two concrete Rust codegen fallbacks Solidity MUST NOT inherit:
    // (1) xs[0] hits expr_to_rust's `_ => "??"` catch-all!
    // (2) forall hits constraint_to_rust's `"true"` fallback!
    assert!(
        assert_lines[4].contains("??"),
        "expected expr_to_rust catch-all on Index, got: {}",
        assert_lines[4]
    );
    assert!(
        assert_lines[6].contains("debug_assert!(true,"),
        "expected constraint_to_rust true fallback on Forall, got: {}",
        assert_lines[6]
    );

    println!("P167|require-ast|kinds={:?}", kinds);
    for (i, line) in assert_lines.iter().enumerate() {
        println!("P167|require-rust|row={i} {line}");
    }
}

// ── 5. Toolchain availability inventory (host-neutral) ───

fn probe_tool(cmd: &str, arg: &str) -> String {
    // On Windows, `.cmd` / `.ps1` shims (like `npm` / `npx`) resolve via `cmd /C`
    // or directly for `.exe`. Try direct first, then `cmd /C` on Windows.
    let direct = Command::new(cmd).arg(arg).output();
    let output = match direct {
        Ok(o) if o.status.success() => Some(o),
        _ if cfg!(windows) => Command::new("cmd")
            .args(["/C", cmd, arg])
            .output()
            .ok()
            .filter(|o| o.status.success()),
        _ => None,
    };
    match output {
        Some(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            let first = text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("ok")
                .trim()
                .to_string();
            format!("present ({first})")
        }
        None => "absent".to_string(),
    }
}

#[test]
fn p167_toolchain_inventory() {
    let tools = [
        ("solc", "--version"),
        ("solcjs", "--version"),
        ("forge", "--version"),
        ("anvil", "--version"),
        ("cast", "--version"),
        ("node", "--version"),
        ("npm", "--version"),
        ("npx", "--version"),
    ];
    let mut map = BTreeMap::new();
    for (cmd, arg) in tools {
        let status = probe_tool(cmd, arg);
        println!("P167|toolchain|{cmd}={status}");
        map.insert(cmd, status);
    }
    // Always succeeds — reports host inventory without assuming host-specific tools.
    assert_eq!(map.len(), tools.len());
}

// ── 6. Candidate pure/view smart-contract exhibits + PCC ──

const CONTRACT_EXHIBITS: &str = r#"DeFiMath {
spec fee_bps {
    doc: "Protocol fee in basis points: floor(amount * bps / 10000) for non-negative amount and bps in [0, 10000]."
    input: amount: Int, bps: Int
    output: Int

    require: amount >= 0
    require: bps >= 0
    require: bps <= 10000

    constraint: output == (amount * bps) / 10000

    test "zero amount" { input: (0, 30) expect: 0 }
    test "30 bps of 10000" { input: (10000, 30) expect: 30 }
    test "250 bps of 1000000" { input: (1000000, 250) expect: 25000 }
    test "rounding down" { input: (999, 30) expect: 2 }
    test "max bps 100pct" { input: (5000, 10000) expect: 5000 }
}

spec debit_balance {
    doc: "Safe balance debit: returns balance - amount when 0 <= amount <= balance."
    input: balance: Int, amount: Int
    output: Int

    require: amount >= 0
    require: balance >= amount

    constraint: output == balance - amount

    test "partial debit" { input: (1000, 250) expect: 750 }
    test "full drain" { input: (500, 500) expect: 0 }
    test "zero debit" { input: (42, 0) expect: 42 }
    test "large balance" { input: (1000000000, 123456) expect: 999876544 }
}

spec bounded_payout {
    doc: "Treasury payout clamped to [min_out, max_out] via stdlib int_clamp."
    input: amount: Int, min_out: Int, max_out: Int
    output: Int

    require: min_out <= max_out

    constraint: output == int_clamp(amount, min_out, max_out)

    test "in range" { input: (500, 100, 1000) expect: 500 }
    test "below floor" { input: (50, 100, 1000) expect: 100 }
    test "above cap" { input: (2500, 100, 1000) expect: 1000 }
    test "at floor" { input: (100, 100, 1000) expect: 100 }
    test "at cap" { input: (1000, 100, 1000) expect: 1000 }
}

spec linear_vested {
    doc: "Linear token vesting: 0 before start, total after duration, proportional in between."
    input: total: Int, elapsed: Int, duration: Int
    output: Int

    require: total >= 0
    require: duration > 0

    constraint: output == (total * int_clamp(elapsed, 0, duration)) / duration

    test "before start" { input: (1000, 0, 100) expect: 0 }
    test "negative elapsed" { input: (1000, (0 - 5), 100) expect: 0 }
    test "halfway" { input: (1000, 50, 100) expect: 500 }
    test "quarter" { input: (1000, 25, 100) expect: 250 }
    test "completed" { input: (1000, 100, 100) expect: 1000 }
    test "past end" { input: (1000, 250, 100) expect: 1000 }
}

spec can_withdraw {
    doc: "Pure withdrawal guard predicate: positive amount within both account balance and per-tx limit."
    input: balance: Int, amount: Int, limit: Int
    output: Bool

    require: balance >= 0
    require: limit >= 0

    constraint: output == (amount > 0 && amount <= balance && amount <= limit)

    test "allowed" { input: (1000, 200, 500) expect: true }
    test "zero denied" { input: (1000, 0, 500) expect: false }
    test "negative denied" { input: (1000, (0 - 10), 500) expect: false }
    test "over balance" { input: (100, 200, 500) expect: false }
    test "over limit" { input: (1000, 600, 500) expect: false }
}

spec non_empty_batch {
    doc: "Guard predicate checking that a payout batch array is non-empty."
    input: amounts: List<Int>
    output: Bool

    constraint: output == (len(amounts) > 0)

    test "empty" { input: [] expect: false }
    test "single" { input: [250] expect: true }
    test "batch" { input: [100, 250, 650] expect: true }
}
}"#;

/// Exhibit measuring the standing statement-body inlining boundary:
/// calling `list_sum` (whose shipped body is Block/Alloc/For) from a
/// user spec constraint cannot inline into a constraint-induced seed
/// (the Phase 165 `weights_sorted` boundary), whereas `list_sum` itself
/// already ships as a Tier-1 encoder-crowned PCC bundle.
const STATEMENT_CALLEE_EXHIBIT: &str = r#"BatchSum {
spec batch_total {
    doc: "Total payout across a batch of transfers via stdlib list_sum."
    input: amounts: List<Int>
    output: Int

    constraint: output == list_sum(amounts)

    test "empty" { input: [] expect: 0 }
    test "single" { input: [250] expect: 250 }
    test "batch" { input: [100, 250, 650] expect: 1000 }
}
}"#;

#[test]
fn p167_contract_exhibits_synthesize_verify_and_pcc_bundle() {
    let _guard = crown_serial();
    let checked = typeck::check(parser::parse(CONTRACT_EXHIBITS).expect("exhibits parse"))
        .expect("exhibits typecheck");
    assert_eq!(checked.spec.functions.len(), 6);

    for spec in &checked.spec.functions {
        let name = &spec.name;
        let (report, winner) =
            morphic::api::synthesize_seeded_spec_winner(CONTRACT_EXHIBITS, name, 42)
                .unwrap_or_else(|e| panic!("{name}: synthesis failed: {e:?}"));

        let w = &report.winner;
        assert_eq!(
            w.tests_passed, w.tests_evaluable,
            "{name}: expected all evaluable tests to pass"
        );
        assert_eq!(
            w.tests_evaluable, w.tests_total,
            "{name}: expected all tests evaluable"
        );

        let (tier, basis) = if w.verification_status == "Verified" {
            ("tier-1", "z3-verified")
        } else {
            ("tier-2", "executed-exact")
        };

        if cfg!(feature = "z3-support") {
            // Measure whether every exhibit actually reaches Tier-1 Verified today.
            println!(
                "P167|exhibit|{name} status={} tier={tier} battery={}/{}/{} requires={} body={}",
                w.verification_status,
                w.tests_passed,
                w.tests_evaluable,
                w.tests_total,
                spec.preconditions.len(),
                w.body_debug
            );
        } else {
            println!(
                "P167|exhibit-noz3|{name} status={} tier={tier} battery={}/{}/{} requires={}",
                w.verification_status,
                w.tests_passed,
                w.tests_evaluable,
                w.tests_total,
                spec.preconditions.len()
            );
        }

        let input = BindingInput {
            spec: spec.clone(),
            body: winner.body.clone(),
            tier: tier.to_string(),
            verdict_basis: basis.to_string(),
            battery_pinned: w.tests_passed,
            battery_silent: 0,
            crown_route: CrownRoute::Pipeline,
        };
        let bundle = pcc::produce_bundle(&input)
            .unwrap_or_else(|e| panic!("{name}: produce_bundle failed: {e}"));
        let json = pcc::bundle_json(&bundle);
        let verified_summary = pcc::verify_bundle_text(&json)
            .unwrap_or_else(|e| panic!("{name}: verify_bundle_text failed: {e}"));
        println!(
            "P167|pcc|{name} bytes={} tier={} verdict={} summary=\"{verified_summary}\"",
            json.len(),
            bundle.tier,
            bundle.verdict
        );
    }

    // Measure the statement-callee boundary (`list_sum` called in a user spec)
    // vs `list_sum`'s own shipped encoder-crowned bundle.
    let (batch_report, _) =
        morphic::api::synthesize_seeded_spec_winner(STATEMENT_CALLEE_EXHIBIT, "batch_total", 42)
            .expect("batch_total seeded run completes");
    println!(
        "P167|exhibit-boundary|batch_total(calls list_sum) status={} battery={}/{}/{} body={}",
        batch_report.winner.verification_status,
        batch_report.winner.tests_passed,
        batch_report.winner.tests_evaluable,
        batch_report.winner.tests_total,
        batch_report.winner.body_debug
    );
    let list_sum_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(pcc::BUNDLES_DIR)
        .join("list_sum.bundle.json");
    let list_sum_json = std::fs::read_to_string(&list_sum_path).expect("list_sum bundle reads");
    match pcc::verify_bundle_text(&list_sum_json) {
        Ok(summary) => println!("P167|pcc-stdlib|list_sum bytes={} ok=\"{summary}\"", list_sum_json.len()),
        Err(e) => {
            assert!(
                !cfg!(feature = "z3-support") && e.contains("unexamined"),
                "list_sum bundle must verify under Z3: {e}"
            );
            println!("P167|pcc-stdlib|list_sum refused-by-name (no-Z3): {e}");
        }
    }
}

// ── 7. IR node / op inventory across Solidity-mappable stdlib bodies ──

fn collect_ir_kinds(
    node: &IRNode,
    nodes: &mut std::collections::BTreeSet<&'static str>,
    binops: &mut std::collections::BTreeSet<String>,
    unops: &mut std::collections::BTreeSet<String>,
) {
    let tag = match node {
        IRNode::Hole { .. } => "Hole",
        IRNode::Block(_) => "Block",
        IRNode::Let { .. } => "Let",
        IRNode::If { .. } => "If",
        IRNode::While { .. } => "While",
        IRNode::For { .. } => "For",
        IRNode::Match { .. } => "Match",
        IRNode::Call { .. } => "Call",
        IRNode::Return(_) => "Return",
        IRNode::Literal(_) => "Literal",
        IRNode::Var(_) => "Var",
        IRNode::BinOp(..) => "BinOp",
        IRNode::UnaryOp(..) => "UnaryOp",
        IRNode::Lambda { .. } => "Lambda",
        IRNode::Closure { .. } => "Closure",
        IRNode::Collection { .. } => "Collection",
        IRNode::Map { .. } => "Map",
        IRNode::Filter { .. } => "Filter",
        IRNode::Fold { .. } => "Fold",
        IRNode::Sort { .. } => "Sort",
        IRNode::Alloc { .. } => "Alloc",
        IRNode::Assign { .. } => "Assign",
        IRNode::Typed { .. } => "Typed",
        IRNode::Index(..) => "Index",
        IRNode::Loop { .. } => "Loop",
        IRNode::Break { .. } => "Break",
        IRNode::StructDef { .. } => "StructDef",
        IRNode::TryPropagate(_) => "TryPropagate",
        IRNode::FieldAccess { .. } => "FieldAccess",
        IRNode::RecordLit { .. } => "RecordLit",
    };
    nodes.insert(tag);
    match node {
        IRNode::Block(stmts) => {
            for s in stmts {
                collect_ir_kinds(s, nodes, binops, unops);
            }
        }
        IRNode::Let { value, body, .. } => {
            collect_ir_kinds(value, nodes, binops, unops);
            collect_ir_kinds(body, nodes, binops, unops);
        }
        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_ir_kinds(condition, nodes, binops, unops);
            collect_ir_kinds(then_branch, nodes, binops, unops);
            collect_ir_kinds(else_branch, nodes, binops, unops);
        }
        IRNode::While { condition, body, .. } => {
            collect_ir_kinds(condition, nodes, binops, unops);
            collect_ir_kinds(body, nodes, binops, unops);
        }
        IRNode::For { start, end, body, .. } => {
            collect_ir_kinds(start, nodes, binops, unops);
            collect_ir_kinds(end, nodes, binops, unops);
            collect_ir_kinds(body, nodes, binops, unops);
        }
        IRNode::Match { scrutinee, cases } => {
            collect_ir_kinds(scrutinee, nodes, binops, unops);
            for (_, body) in cases {
                collect_ir_kinds(body, nodes, binops, unops);
            }
        }
        IRNode::Call { args, .. } => {
            for a in args {
                collect_ir_kinds(a, nodes, binops, unops);
            }
        }
        IRNode::Return(inner) | IRNode::TryPropagate(inner) | IRNode::Typed { node: inner, .. } => {
            collect_ir_kinds(inner, nodes, binops, unops);
        }
        IRNode::BinOp(op, l, r) => {
            binops.insert(format!("{op:?}"));
            collect_ir_kinds(l, nodes, binops, unops);
            collect_ir_kinds(r, nodes, binops, unops);
        }
        IRNode::UnaryOp(op, inner) => {
            unops.insert(format!("{op:?}"));
            collect_ir_kinds(inner, nodes, binops, unops);
        }
        IRNode::Lambda { body, .. }
        | IRNode::Closure { body, .. }
        | IRNode::Loop { body, .. }
        | IRNode::StructDef { body, .. } => {
            collect_ir_kinds(body, nodes, binops, unops);
        }
        IRNode::Collection { elements, .. } => {
            for e in elements {
                collect_ir_kinds(e, nodes, binops, unops);
            }
        }
        IRNode::Map { function, over } | IRNode::Filter { predicate: function, over } => {
            collect_ir_kinds(function, nodes, binops, unops);
            collect_ir_kinds(over, nodes, binops, unops);
        }
        IRNode::Fold {
            operation,
            initial,
            over,
        } => {
            collect_ir_kinds(operation, nodes, binops, unops);
            collect_ir_kinds(initial, nodes, binops, unops);
            collect_ir_kinds(over, nodes, binops, unops);
        }
        IRNode::Sort {
            comparator,
            collection,
        } => {
            collect_ir_kinds(comparator, nodes, binops, unops);
            collect_ir_kinds(collection, nodes, binops, unops);
        }
        IRNode::Alloc { initial, .. } => collect_ir_kinds(initial, nodes, binops, unops),
        IRNode::Assign { target, value } => {
            collect_ir_kinds(target, nodes, binops, unops);
            collect_ir_kinds(value, nodes, binops, unops);
        }
        IRNode::Index(a, b) => {
            collect_ir_kinds(a, nodes, binops, unops);
            collect_ir_kinds(b, nodes, binops, unops);
        }
        IRNode::Break { value } => {
            if let Some(v) = value {
                collect_ir_kinds(v, nodes, binops, unops);
            }
        }
        IRNode::FieldAccess { base, .. } => collect_ir_kinds(base, nodes, binops, unops),
        IRNode::RecordLit { fields, .. } => {
            for (_, v) in fields {
                collect_ir_kinds(v, nodes, binops, unops);
            }
        }
        IRNode::Hole { .. } | IRNode::Literal(_) | IRNode::Var(_) => {}
    }
}

#[test]
fn p167_stdlib_direct_abi_ir_inventory() {
    use std::collections::BTreeSet;
    let specs = stdlib::stdlib_specs();

    let mut scalar_nodes = BTreeSet::new();
    let mut scalar_binops = BTreeSet::new();
    let mut scalar_unops = BTreeSet::new();

    let mut abi_nodes = BTreeSet::new();
    let mut abi_binops = BTreeSet::new();
    let mut abi_unops = BTreeSet::new();

    for s in &specs {
        let all_tys: Vec<&TypeRef> = s
            .params
            .iter()
            .map(|p| &p.ty)
            .chain(std::iter::once(&s.return_type))
            .collect();
        let only_int_bool = all_tys
            .iter()
            .all(|t| matches!(t, TypeRef::Int | TypeRef::Bool));
        let direct_abi = all_tys.iter().all(|t| {
            !contains_type(t, &|x| {
                matches!(
                    x,
                    TypeRef::Char
                        | TypeRef::Option(_)
                        | TypeRef::Result(_, _)
                        | TypeRef::Function(_, _)
                )
            })
        });
        let body = stdlib::shipped_body(&s.name).expect("shipped body exists");
        if only_int_bool {
            collect_ir_kinds(&body, &mut scalar_nodes, &mut scalar_binops, &mut scalar_unops);
        }
        if direct_abi {
            collect_ir_kinds(&body, &mut abi_nodes, &mut abi_binops, &mut abi_unops);
        }
    }

    assert!(!scalar_nodes.is_empty());
    assert!(!abi_nodes.is_empty());
    println!(
        "P167|ir-scalar7|nodes={:?} binops={:?} unops={:?}",
        scalar_nodes, scalar_binops, scalar_unops
    );
    println!(
        "P167|ir-abi18|nodes={:?} binops={:?} unops={:?}",
        abi_nodes, abi_binops, abi_unops
    );
}
