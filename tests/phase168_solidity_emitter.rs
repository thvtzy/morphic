//! Phase 168 — 1.8 Core Solidity Emitter (`--target solidity` / `sol`)
//! TDD verification suite against `IKONIK_18_SCOPE.md` §3, §4, §5.1, §6.

#![cfg(not(feature = "wasm-playground"))]

use std::process::Command;
use std::sync::Mutex;

use morphic::api;
use morphic::codegen;
use morphic::spec::ast::{FunctionSpec, Param, TypeRef};
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, Provenance, ScoreBreakdown,
};

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn make_candidate(spec: FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 168,
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

fn assert_balanced_delimiters(code: &str, label: &str) {
    let mut braces = 0i32;
    let mut parens = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for ch in code.chars() {
        if in_str {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            '{' => braces += 1,
            '}' => {
                braces -= 1;
                assert!(braces >= 0, "{label}: unmatched '}}'\n{code}");
            }
            '(' => parens += 1,
            ')' => {
                parens -= 1;
                assert!(parens >= 0, "{label}: unmatched ')'\n{code}");
            }
            _ => {}
        }
    }
    assert_eq!(braces, 0, "{label}: unbalanced braces\n{code}");
    assert_eq!(parens, 0, "{label}: unbalanced parens\n{code}");
}

const DEFI_EXHIBITS: &str = r#"DeFiMath {
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

// ── 1. Dispatch, alias, api::render, and D6a 6-target pin ──

#[test]
fn p168_dispatch_solidity_and_sol_aliases_and_api_render() {
    let _guard = crown_serial();
    let src = r#"IdMod {
spec id_int {
    input: x: Int
    output: Int
    constraint: output == x
    test "zero" { input: 0 expect: 0 }
}
}"#;
    let checked = typeck::check(parser::parse(src).unwrap()).unwrap();
    let cand = make_candidate(checked.spec.functions[0].clone(), IRNode::Var("x".into()));

    let sol_full = codegen::generate(&cand, "solidity").expect("solidity target renders");
    let sol_alias = codegen::generate(&cand, "sol").expect("sol alias renders");
    let sol_api = api::render(&cand, "solidity").expect("api::render supports solidity");
    assert_eq!(sol_full, sol_alias, "solidity and sol must be byte-identical");
    assert_eq!(sol_full, sol_api, "api::render delegates to codegen::generate");
    assert!(sol_full.contains("// SPDX-License-Identifier: MIT"));
    assert!(sol_full.contains("pragma solidity ^0.8.24;"));
    assert!(sol_full.contains("function id_int(int256 x) public pure returns (int256)"));
    assert_balanced_delimiters(&sol_full, "id_int");

    // D6a: WinnerReport.renders stays at the standing 6 targets in 1.8.
    let report = api::synthesize_seeded(src, 42).unwrap();
    assert_eq!(report.winner.renders.len(), 6);
}

// ── 2. All 6 DeFi exhibits render cleanly with requires + boundary ──

#[test]
fn p168_all_six_defi_exhibits_render_with_requires_and_boundary() {
    let _guard = crown_serial();
    let expected = [
        ("fee_bps", 3usize, "returns (int256)"),
        ("debit_balance", 2, "returns (int256)"),
        ("bounded_payout", 1, "returns (int256)"),
        ("linear_vested", 2, "returns (int256)"),
        ("can_withdraw", 2, "returns (bool)"),
        ("non_empty_batch", 0, "returns (bool)"),
    ];
    for (name, req_count, ret_sig) in expected {
        let (_report, winner) =
            api::synthesize_seeded_spec_winner(DEFI_EXHIBITS, name, 42)
                .unwrap_or_else(|e| panic!("{name}: seeded run failed: {e:?}"));
        let code = codegen::generate(&winner, "solidity")
            .unwrap_or_else(|e| panic!("{name}: solidity codegen failed: {e}"));

        assert!(code.starts_with("// SPDX-License-Identifier: MIT\n"));
        assert!(code.contains("pragma solidity ^0.8.24;"));
        assert!(code.contains("Panic(0x11)"), "{name}: missing overflow boundary disclosure");
        assert!(code.contains("Euclidean"), "{name}: missing neg-div boundary disclosure");
        assert!(code.contains("public pure"), "{name}: must emit public pure");
        assert!(!code.contains("public view"), "{name}: stateless fn must not be view");
        assert!(code.contains(ret_sig), "{name}: expected {ret_sig}\n{code}");
        assert!(!code.contains("debug_assert"), "{name}: no Rust asserts");
        assert!(!code.contains("??"), "{name}: no catch-all placeholders");
        assert!(!code.contains("require(true"), "{name}: never emit require(true)");

        let actual_reqs = code.matches("require(").count();
        assert_eq!(actual_reqs, req_count, "{name}: require count mismatch\n{code}");
        assert_balanced_delimiters(&code, name);
    }
}

// ── 3. All 18 direct-ABI stdlib entries render cleanly ──

#[test]
fn p168_all_18_direct_abi_stdlib_entries_render_cleanly() {
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
    let needs_append = ["list_reverse", "list_take", "list_drop", "list_sort", "range"];

    let specs = stdlib::stdlib_specs();
    for name in direct_abi {
        let spec = specs
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("missing stdlib spec {name}"))
            .clone();
        let body = stdlib::shipped_body(name)
            .unwrap_or_else(|| panic!("missing shipped body {name}"));
        let cand = make_candidate(spec, body);
        let code = codegen::generate(&cand, "solidity")
            .unwrap_or_else(|e| panic!("stdlib {name} failed solidity codegen: {e}"));

        assert!(code.contains(&format!("function {name}(")), "{name}: missing fn decl\n{code}");
        assert!(code.contains("public pure"), "{name}: missing public pure\n{code}");
        assert!(!code.contains("??"), "{name}: placeholder found\n{code}");
        assert_balanced_delimiters(&code, name);

        let has_append_helper = code.contains("function _morphic_append_int256(");
        assert_eq!(
            has_append_helper,
            needs_append.contains(&name),
            "{name}: append helper presence mismatch\n{code}"
        );

        if name == "int_abs" {
            assert!(code.contains("require("), "int_abs must enforce require:");
            assert!(code.contains("9223372036854775807"));
        }
        if name == "int_clamp" {
            assert!(code.contains("require((lo <= hi)"), "int_clamp must enforce lo <= hi\n{code}");
        }
        if name == "string_is_empty" {
            assert!(code.contains("int256(bytes(s).length)"), "string_is_empty must use bytes(s).length\n{code}");
        }
    }
}

// ── 4. All 17 non-direct-ABI stdlib entries refuse by name ──

#[test]
fn p168_all_17_non_direct_abi_stdlib_entries_refuse_by_name() {
    let refused = [
        "char_is_digit",
        "char_is_alpha",
        "char_is_whitespace",
        "char_is_uppercase",
        "char_is_lowercase",
        "char_to_digit",
        "result_unwrap_or",
        "option_unwrap_or",
        "option_ok_or",
        "list_min",
        "list_max",
        "list_binary_search",
        "list_map",
        "list_filter",
        "list_fold",
        "list_all",
        "list_any",
    ];
    let specs = stdlib::stdlib_specs();
    for name in refused {
        let spec = specs
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("missing stdlib spec {name}"))
            .clone();
        let body = stdlib::shipped_body(name).unwrap();
        let cand = make_candidate(spec, body);
        let err = codegen::generate(&cand, "solidity")
            .expect_err(&format!("{name} must refuse solidity codegen"));
        let msg = err.to_string();
        assert!(
            !msg.contains("Unsupported target language"),
            "{name}: must refuse on type/IR, not unsupported target: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("solidity"),
            "{name}: refusal should mention solidity, got: {msg}"
        );
    }
}

// ── 5. Hard refusals: types, quantified requires, unsupported IR ──

#[test]
fn p168_hard_refusals_types_quantified_require_and_ir() {
    // (a) Unsupported types
    let bad_types = [
        TypeRef::Float,
        TypeRef::Char,
        TypeRef::Option(Box::new(TypeRef::Int)),
        TypeRef::Result(Box::new(TypeRef::Int), Box::new(TypeRef::String)),
        TypeRef::Map(Box::new(TypeRef::Int), Box::new(TypeRef::Int)),
        TypeRef::Set(Box::new(TypeRef::Int)),
        TypeRef::Stream(Box::new(TypeRef::Int)),
        TypeRef::Tuple(vec![TypeRef::Int, TypeRef::Bool]),
        TypeRef::Generic("T".into()),
        TypeRef::Ref(Box::new(TypeRef::Int)),
        TypeRef::Named("U256".into()),
        TypeRef::List(Box::new(TypeRef::List(Box::new(TypeRef::Int)))),
        TypeRef::List(Box::new(TypeRef::String)),
        TypeRef::Unit, // refused in parameter position
    ];
    for ty in bad_types {
        let spec = FunctionSpec {
            name: "bad_ty".into(),
            doc: None,
            generics: vec![],
            params: vec![Param {
                name: "x".into(),
                ty: ty.clone(),
                annotations: Default::default(),
            }],
            return_type: TypeRef::Int,
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: Default::default(),
        };
        let cand = make_candidate(spec, IRNode::Literal(IRLiteral::Int(0)));
        assert!(
            codegen::generate(&cand, "solidity").is_err(),
            "type {ty} must be refused in solidity"
        );
    }

    // (b) Quantified require: MUST refuse, never emit require(true)
    let forall_src = r#"QMod {
spec all_pos {
    input: xs: List<Int>
    output: Int
    require: forall i: Int in range(0, len(xs)): xs[i] >= 0
    constraint: output == len(xs)
}
}"#;
    let checked = typeck::check(parser::parse(forall_src).unwrap()).unwrap();
    let cand = make_candidate(
        checked.spec.functions[0].clone(),
        IRNode::Call {
            function: "len".into(),
            args: vec![IRNode::Var("xs".into())],
        },
    );
    let err = codegen::generate(&cand, "solidity")
        .expect_err("quantified require: must refuse solidity codegen");
    assert!(
        err.to_string().to_lowercase().contains("quantif")
            || err.to_string().to_lowercase().contains("precondition"),
        "unexpected refusal message: {err}"
    );

    // (c) Unsupported IR nodes (Hole, unknown Call, int_to_str, TryPropagate)
    let ok_src = r#"OkMod {
spec f {
    input: x: Int
    output: Int
    constraint: output == x
}
}"#;
    let ok_spec = typeck::check(parser::parse(ok_src).unwrap())
        .unwrap()
        .spec
        .functions[0]
        .clone();
    let bad_bodies = [
        IRNode::Hole {
            id: 1,
            expected_type: Some(TypeRef::Int),
        },
        IRNode::Call {
            function: "mystery_external_fn".into(),
            args: vec![IRNode::Var("x".into())],
        },
        IRNode::Call {
            function: "int_to_str".into(),
            args: vec![IRNode::Var("x".into())],
        },
        IRNode::TryPropagate(Box::new(IRNode::Var("x".into()))),
        IRNode::Loop {
            body: Box::new(IRNode::Break { value: None }),
            invariant: None,
        },
    ];
    for body in bad_bodies {
        let cand = make_candidate(ok_spec.clone(), body);
        assert!(
            codegen::generate(&cand, "solidity").is_err(),
            "unsupported IR body must be refused in solidity"
        );
    }
}

// ── 6. Transitive stdlib helpers in body AND require: ──

#[test]
fn p168_transitive_stdlib_helpers_in_body_and_require() {
    // (a) Body calls stdlib `int_clamp` directly -> emits internal pure int_clamp
    let body_src = r#"HelperBody {
spec clamp_pos {
    input: x: Int, hi: Int
    output: Int
    require: hi >= 0
    constraint: output == int_clamp(x, 0, hi)
}
}"#;
    let checked = typeck::check(parser::parse(body_src).unwrap()).unwrap();
    let cand = make_candidate(
        checked.spec.functions[0].clone(),
        IRNode::Call {
            function: "int_clamp".into(),
            args: vec![
                IRNode::Var("x".into()),
                IRNode::Literal(IRLiteral::Int(0)),
                IRNode::Var("hi".into()),
            ],
        },
    );
    let code = codegen::generate(&cand, "solidity").expect("helper body renders");
    assert!(code.contains("function clamp_pos(int256 x, int256 hi) public pure returns (int256)"));
    assert!(code.contains("function int_clamp(int256 x, int256 lo, int256 hi) internal pure returns (int256)"));
    assert_balanced_delimiters(&code, "clamp_pos");

    // (b) Precondition calls `list_is_sorted(xs)` -> emits require + internal pure helper
    let req_src = r#"HelperReq {
spec first_of_sorted {
    input: xs: List<Int>
    output: Int
    require: len(xs) > 0
    require: list_is_sorted(xs)
    constraint: output == xs[0]
}
}"#;
    let checked2 = typeck::check(parser::parse(req_src).unwrap()).unwrap();
    let cand2 = make_candidate(
        checked2.spec.functions[0].clone(),
        IRNode::Index(
            Box::new(IRNode::Var("xs".into())),
            Box::new(IRNode::Literal(IRLiteral::Int(0))),
        ),
    );
    let code2 = codegen::generate(&cand2, "solidity").expect("helper in require renders");
    assert!(code2.contains("require((int256(xs.length) > 0)"), "{code2}");
    assert!(code2.contains("require(list_is_sorted(xs)"), "{code2}");
    assert!(code2.contains("function list_is_sorted(int256[] memory xs) internal pure returns (bool)"), "{code2}");
    assert_balanced_delimiters(&code2, "first_of_sorted");
}

// ── 7. Fixed-width ints, string equality, and Solidity keyword sanitization ──

#[test]
fn p168_fixed_width_ints_string_eq_and_keywords() {
    // Fixed-width U64 / I32
    let fw_src = r#"FwMod {
spec add_u64 {
    input: a: U64, b: U64
    output: U64
    constraint: output == a + b
}
}"#;
    let checked = typeck::check(parser::parse(fw_src).unwrap()).unwrap();
    let cand = make_candidate(
        checked.spec.functions[0].clone(),
        IRNode::BinOp(
            IRBinOp::Add,
            Box::new(IRNode::Var("a".into())),
            Box::new(IRNode::Var("b".into())),
        ),
    );
    let code = codegen::generate(&cand, "solidity").expect("U64 renders");
    assert!(code.contains("function add_u64(uint64 a, uint64 b) public pure returns (uint64)"));

    // String equality uses keccak256(bytes(...))
    let str_src = r#"StrMod {
spec same_str {
    input: a: String, b: String
    output: Bool
    constraint: output == (a == b)
}
}"#;
    let checked_s = typeck::check(parser::parse(str_src).unwrap()).unwrap();
    let cand_s = make_candidate(
        checked_s.spec.functions[0].clone(),
        IRNode::BinOp(
            IRBinOp::Eq,
            Box::new(IRNode::Var("a".into())),
            Box::new(IRNode::Var("b".into())),
        ),
    );
    let code_s = codegen::generate(&cand_s, "solidity").expect("String eq renders");
    assert!(
        code_s.contains("keccak256(bytes(a)) == keccak256(bytes(b))"),
        "expected keccak256 string eq\n{code_s}"
    );

    // Solidity keyword parameter sanitization (`address`, `memory`)
    let kw_src = r#"KwMod {
spec kw_safe {
    input: address: Int, memory: Int
    output: Int
    require: address >= 0
    constraint: output == address + memory
}
}"#;
    let checked_kw = typeck::check(parser::parse(kw_src).unwrap()).unwrap();
    let cand_kw = make_candidate(
        checked_kw.spec.functions[0].clone(),
        IRNode::BinOp(
            IRBinOp::Add,
            Box::new(IRNode::Var("address".into())),
            Box::new(IRNode::Var("memory".into())),
        ),
    );
    let code_kw = codegen::generate(&cand_kw, "solidity").expect("keyword params sanitize");
    assert!(code_kw.contains("int256 address_, int256 memory_"), "{code_kw}");
    assert!(code_kw.contains("require((address_ >= 0)"), "{code_kw}");
    assert!(code_kw.contains("(address_ + memory_)"), "{code_kw}");
}

// ── 8. CLI `morphic build <file> --target solidity` writes `.sol` ──

#[test]
fn p168_cli_build_target_solidity_writes_dot_sol_extension() {
    let _guard = crown_serial();
    let tmp = std::env::temp_dir().join(format!(
        "morphic-p168-cli-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let morph_path = tmp.join("fee_cli.morph");
    let sol_path = tmp.join("fee_cli.sol");
    let wrong_path = tmp.join("fee_cli.solidity");

    let src = r#"FeeCli {
spec fee_bps {
    input: amount: Int, bps: Int
    output: Int
    require: amount >= 0
    require: bps >= 0
    require: bps <= 10000
    constraint: output == (amount * bps) / 10000
    test "30 bps" { input: (10000, 30) expect: 30 }
}
}"#;
    std::fs::write(&morph_path, src).unwrap();

    let bin = env!("CARGO_BIN_EXE_morphic");
    let out = Command::new(bin)
        .args([
            "build",
            morph_path.to_str().unwrap(),
            "-O",
            "1",
            "--target",
            "solidity",
        ])
        .output()
        .expect("morphic build runs");
    assert!(
        out.status.success(),
        "morphic build --target solidity failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(sol_path.exists(), "expected .sol output at {}", sol_path.display());
    assert!(!wrong_path.exists(), "must not write .solidity extension");

    let generated = std::fs::read_to_string(&sol_path).unwrap();
    assert!(generated.contains("pragma solidity ^0.8.24;"));
    assert!(generated.contains("function fee_bps("));
    assert!(generated.contains("require("));

    let _ = std::fs::remove_dir_all(&tmp);
}
