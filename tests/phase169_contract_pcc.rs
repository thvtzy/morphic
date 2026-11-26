//! Phase 169 — 1.8 Multi-Spec Checked Contract Assembly + PCC Coupling
//! (`morphic build --contract-from <file.morph>`, IKONIK_18_SCOPE.md §5.2).

#![cfg(not(feature = "wasm-playground"))]

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use morphic::codegen::solidity;
use morphic::pcc;
use morphic::registry;

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn defi_math_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("contracts")
        .join("defi_math.morph")
}

// ── 1. Committed exhibit assembles deterministically with matching PCC bundles ──

#[test]
fn p169_committed_defi_math_exhibit_assembles_with_matching_pcc_bundles() {
    let _guard = crown_serial();
    let src = std::fs::read_to_string(defi_math_path()).expect("defi_math.morph reads");

    let first = solidity::assemble_user_contract(&src, 42)
        .expect("DeFiMath assembles cleanly");
    let second = solidity::assemble_user_contract(&src, 42)
        .expect("second run succeeds");

    assert_eq!(first.contract_name, "DeFiMath");
    assert_eq!(first.contract_file, "DeFiMath.sol");
    assert_eq!(first.contract_source, second.contract_source, "deterministic .sol");
    assert_eq!(first.bundles, second.bundles, "deterministic companion PCC bundles");
    assert_eq!(first.bundles.len(), 6);
    assert_eq!(first.standings.len(), 6);

    assert!(first.contract_source.starts_with("// SPDX-License-Identifier: MIT\n"));
    assert!(first.contract_source.contains("pragma solidity ^0.8.24;"));
    assert!(first.contract_source.contains("contract DeFiMath {"));
    assert!(first.contract_source.contains("Panic(0x11)"));
    assert!(first.contract_source.contains("Euclidean"));

    let expected_tier = if cfg!(feature = "z3-support") {
        "tier-1"
    } else {
        "tier-2"
    };
    let expected_basis = if cfg!(feature = "z3-support") {
        "z3-verified"
    } else {
        "executed-exact"
    };

    for (file_name, json, sha) in &first.bundles {
        assert_eq!(
            sha,
            &registry::sha256_hex(json.as_bytes()),
            "{file_name}: sha256 matches bundle bytes"
        );
        // Bundle version stays 1 (D5a).
        assert!(json.contains("\"bundleVersion\": 1"), "{file_name}: bundleVersion 1");
        // Provenance embedded in contract NatSpec.
        assert!(
            first.contract_source.contains(&format!("bundleSha256: {sha}")),
            "{file_name}: missing bundleSha256 in NatSpec"
        );
        assert!(
            first.contract_source.contains(&format!("bundle: {file_name}")),
            "{file_name}: missing bundle filename in NatSpec"
        );
        assert!(
            first.contract_source.contains(&format!("tier: {expected_tier}")),
            "{file_name}: missing tier in NatSpec"
        );
        assert!(
            first.contract_source.contains(&format!("verdictBasis: {expected_basis}")),
            "{file_name}: missing verdictBasis in NatSpec"
        );

        // Independent re-verification via pcc::verify_bundle_text.
        let summary = pcc::verify_bundle_text(json)
            .unwrap_or_else(|e| panic!("{file_name}: companion bundle failed verify: {e}"));
        assert!(summary.contains(expected_tier), "{file_name}: {summary}");
    }

    // Tampered companion bundle must refuse by name.
    let (_name, orig_json, _sha) = &first.bundles[0];
    let tampered = if cfg!(feature = "z3-support") {
        orig_json.replacen("\"verdict\": \"Verified\"", "\"verdict\": \"ExecutedExact\"", 1)
    } else {
        orig_json.replacen("\"verdict\": \"ExecutedExact\"", "\"verdict\": \"Verified\"", 1)
    };
    assert_ne!(&tampered, orig_json);
    let err = pcc::verify_bundle_text(&tampered).expect_err("tampered bundle refuses");
    assert!(err.contains("incoherent claim"), "unexpected tamper err: {err}");
}

// ── 2. CLI `morphic build --contract-from` writes .sol + bundles & verifies via CLI ──

#[test]
fn p169_cli_build_contract_from_writes_sol_and_bundles() {
    let _guard = crown_serial();
    let tmp = std::env::temp_dir().join(format!(
        "morphic-p169-cli-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);

    let bin = env!("CARGO_BIN_EXE_morphic");
    let out = Command::new(bin)
        .args([
            "build",
            "--contract-from",
            defi_math_path().to_str().unwrap(),
            "--out",
            tmp.to_str().unwrap(),
        ])
        .output()
        .expect("morphic build --contract-from runs");
    assert!(
        out.status.success(),
        "build --contract-from failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let sol_path = tmp.join("DeFiMath.sol");
    assert!(sol_path.exists(), "DeFiMath.sol must exist");
    let sol_text = std::fs::read_to_string(&sol_path).unwrap();

    for entry in [
        "fee_bps",
        "debit_balance",
        "bounded_payout",
        "linear_vested",
        "can_withdraw",
        "non_empty_batch",
    ] {
        let bpath = tmp.join(format!("{entry}.bundle.json"));
        assert!(bpath.exists(), "missing companion bundle {}", bpath.display());
        let bytes = std::fs::read(&bpath).unwrap();
        let sha = registry::sha256_hex(&bytes);
        assert!(
            sol_text.contains(&format!("bundleSha256: {sha}")),
            "{entry}: on-disk sha256 must match DeFiMath.sol NatSpec"
        );

        let vout = Command::new(bin)
            .args(["verify-bundle", bpath.to_str().unwrap()])
            .output()
            .expect("verify-bundle runs");
        assert!(
            vout.status.success(),
            "verify-bundle failed for {entry}:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&vout.stdout),
            String::from_utf8_lossy(&vout.stderr)
        );
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── 3. CLI refuses invalid flag combinations and positional input ──

#[test]
fn p169_cli_refuses_flag_combinations_and_positional_input() {
    let bin = env!("CARGO_BIN_EXE_morphic");
    let path = defi_math_path();
    let p = path.to_str().unwrap();

    let bad_invocations: &[&[&str]] = &[
        &["build", "--contract-from", p, "--package"],
        &["build", "--contract-from", p, "--facade"],
        &["build", "--contract-from", p, "--bundles"],
        &["build", "--contract-from", p, "--package-from", p],
        &["build", "--contract-from", p, "--bundles-from", p],
        &["build", "--contract-from", p, p],
    ];

    for args in bad_invocations {
        let out = Command::new(bin).args(*args).output().expect("cli runs");
        assert!(
            !out.status.success(),
            "expected refusal for {:?}, got success",
            args
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("--contract-from"),
            "refusal for {:?} should name --contract-from, got: {stderr}",
            args
        );
    }
}

// ── 4. Refusals write nothing (uncrownable, non-direct-ABI, quantified require) ──

#[test]
fn p169_assembly_refuses_uncrownable_and_non_direct_abi_with_nothing_written() {
    let _guard = crown_serial();
    let tmp = std::env::temp_dir().join(format!(
        "morphic-p169-refuse-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let bin = env!("CARGO_BIN_EXE_morphic");

    // (a) Uncrownable spec (contradictory test battery — cannot pass in Z3 or no-Z3 builds)
    let uncrownable = r#"BadMod {
spec impossible {
    input: x: Int
    output: Int
    constraint: output > x
    constraint: output < x
    test "a" { input: 1 expect: 2 }
    test "b" { input: 1 expect: 3 }
}
}"#;
    let p1 = tmp.join("bad.morph");
    let out1 = tmp.join("out-bad");
    std::fs::write(&p1, uncrownable).unwrap();
    let r1 = Command::new(bin)
        .args(["build", "--contract-from", p1.to_str().unwrap(), "--out", out1.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!r1.status.success(), "uncrownable spec must refuse");
    assert!(!out1.exists(), "nothing written on admission refusal");

    // (b) Admissible Tier-1 Char spec, but outside Solidity direct-ABI subset
    let char_mod = r#"CharMod {
spec is_zero_char {
    input: c: Char
    output: Bool
    constraint: output == (c == '0')
    test "yes" { input: '0' expect: true }
    test "no" { input: '1' expect: false }
}
}"#;
    let p2 = tmp.join("char_mod.morph");
    let out2 = tmp.join("out-char");
    std::fs::write(&p2, char_mod).unwrap();
    let r2 = Command::new(bin)
        .args(["build", "--contract-from", p2.to_str().unwrap(), "--out", out2.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!r2.status.success(), "Char spec must refuse Solidity assembly");
    assert!(!out2.exists(), "nothing written on Solidity type refusal");

    // (c) Quantified require: must refuse, writing nothing
    let quant_mod = r#"QuantMod {
spec len_pos {
    input: xs: List<Int>
    output: Int
    require: forall i: Int in range(0, len(xs)): xs[i] >= 0
    constraint: output == len(xs)
    test "empty" { input: [] expect: 0 }
    test "two" { input: [1, 2] expect: 2 }
}
}"#;
    let err3 = solidity::assemble_user_contract(quant_mod, 42)
        .expect_err("quantified require must refuse contract assembly");
    assert!(
        err3.to_lowercase().contains("quantif"),
        "expected quantifier refusal, got: {err3}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

// ── 5. Multi-spec module deduplicates transitive helpers ──

#[test]
fn p169_multi_spec_deduplicates_transitive_helpers() {
    let _guard = crown_serial();
    let multi = r#"SortedChecks {
spec first_sorted_len {
    input: xs: List<Int>
    output: Int
    require: list_is_sorted(xs)
    constraint: output == len(xs)
    test "empty" { input: [] expect: 0 }
    test "sorted" { input: [1, 2, 3] expect: 3 }
}
spec is_nonempty_sorted {
    input: xs: List<Int>
    output: Bool
    require: list_is_sorted(xs)
    constraint: output == (len(xs) > 0)
    test "empty" { input: [] expect: false }
    test "sorted" { input: [1, 2] expect: true }
}
}"#;
    let assembly = solidity::assemble_user_contract(multi, 42)
        .expect("multi-spec with shared require helper assembles");
    assert_eq!(assembly.contract_name, "SortedChecks");
    assert_eq!(
        assembly
            .contract_source
            .matches("function list_is_sorted(")
            .count(),
        1,
        "shared transitive helper must be emitted exactly once:\n{}",
        assembly.contract_source
    );
    assert!(assembly
        .contract_source
        .contains("function list_is_sorted(int256[] memory xs) internal pure returns (bool)"));
}
