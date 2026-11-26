//! Phase 163 — the seed catalog (DISEBAR_17_SCOPE §3):
//! `registry/index.json` + the 37 bundle files it
//! lists — the 35 standard-library bundles copied
//! byte-identical from `packages/bundles/`, plus the
//! widget pair produced by the new `--bundles-from`
//! arm and committed as produced.
//!
//! Pins, in order:
//! 1. the index's consistency with its files: every
//!    row's file exists at its repo-relative path, its
//!    `bytes` is the file's length, its `sha256` is
//!    the file's hash (asserted where the system
//!    `sha256sum` is checkable — the product's own
//!    hash arrives with `sha2` at Phase 164 — and
//!    reported where it is not), and its claim fields
//!    (`entry`, `tier`, `verdictBasis`) equal the
//!    bundle's own fields; rows sorted by entry; the
//!    35 stdlib files byte-identical to their
//!    `packages/bundles/` counterparts;
//! 2. every seed bundle re-verifies under
//!    `verify_bundle_text` — in a no-Z3 build a Tier-1
//!    bundle's named refusal (the claim stands
//!    unexamined) is the standing posture and is
//!    reported as such, never counted as a pass.
//!
//! Crown-heavy pins serialize on one lock (the Phase
//! 150 discipline: the engine's RNG is process-global).

#![cfg(not(feature = "wasm-playground"))]

use std::sync::Mutex;

use morphic::pcc;

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The seed index's measured masses at Phase 163 (the
/// freeze §9 figures): 37 rows; the catalog is the 35
/// stdlib bundles (267,156 B) + the widget pair
/// (2,706 B + 6,816 B).
const INDEX_BYTES: usize = 10_927;
const CATALOG_BYTES: usize = 276_678;

#[test]
fn p163_index_is_consistent_with_its_files() {
    let root = repo_root();
    let index_text = std::fs::read_to_string(root.join("registry/index.json"))
        .expect("the seed index reads");
    assert_eq!(
        index_text.len(),
        INDEX_BYTES,
        "the index stands at its measured seed mass"
    );
    let index: serde_json::Value =
        serde_json::from_str(&index_text).expect("the index parses");
    assert_eq!(index["format"], "morphic-registry-index");
    assert_eq!(index["indexVersion"], 1);
    let entries = index["entries"].as_array().expect("entries is an array");
    assert_eq!(entries.len(), 37, "the seed catalog carries 37 entries");

    let names: Vec<&str> = entries.iter().map(|e| e["entry"].as_str().unwrap()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "rows stand sorted by entry (the catalogue discipline)");
    assert!(names.contains(&"is_widget_code") && names.contains(&"weights_sorted"),
        "the widget pair is catalogued beside the library");

    let mut catalog_bytes = 0usize;
    let mut hashes_checked = 0usize;
    for row in entries {
        let entry = row["entry"].as_str().unwrap();
        assert_eq!(row["version"], "1.7.0", "{entry}: the seed's cataloguing version");
        let rel = row["file"].as_str().unwrap();
        assert_eq!(
            rel,
            format!("registry/bundles/{entry}.bundle.json"),
            "{entry}: the file row is the repo-relative bundle path"
        );
        let bytes = std::fs::read(root.join(rel))
            .unwrap_or_else(|e| panic!("{entry}: the listed file exists: {e}"));
        assert_eq!(
            row["bytes"].as_u64().unwrap(),
            bytes.len() as u64,
            "{entry}: the row's byte count is the file's length"
        );
        catalog_bytes += bytes.len();

        // The hash binding: asserted where the system
        // tool is checkable (the product's own sha2
        // arrives at Phase 164), reported where not.
        let sha = row["sha256"].as_str().unwrap();
        assert_eq!(sha.len(), 64, "{entry}: the row carries a sha256");
        assert!(
            sha.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "{entry}: the sha256 is lowercase hex"
        );
        if let Ok(out) = std::process::Command::new("sha256sum")
            .arg(root.join(rel))
            .output()
        {
            if out.status.success() {
                let digest = String::from_utf8_lossy(&out.stdout)
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                assert_eq!(digest, sha, "{entry}: the row's sha256 is the file's hash");
                hashes_checked += 1;
            }
        }

        // The claim fields are the bundle's own fields,
        // duplicated — never a second source.
        let bundle: serde_json::Value =
            serde_json::from_slice(&bytes).expect("the bundle parses");
        assert_eq!(bundle["entry"], row["entry"], "{entry}: entry agrees");
        assert_eq!(bundle["tier"], row["tier"], "{entry}: tier label agrees");
        assert_eq!(
            bundle["verdictBasis"], row["verdictBasis"],
            "{entry}: verdictBasis label agrees"
        );
        assert_eq!(bundle["bundleVersion"], 1, "{entry}: the format stands at 1");

        // The 35 stdlib files are byte-identical copies
        // of the packages' evidence (the same artifact,
        // second home); the widget pair has no
        // counterpart there.
        let shipped = root.join(pcc::BUNDLES_DIR).join(format!("{entry}.bundle.json"));
        if shipped.exists() {
            assert_eq!(
                std::fs::read(&shipped).unwrap(),
                bytes,
                "{entry}: the registry copy is byte-identical to packages/bundles"
            );
        } else {
            assert!(
                entry == "is_widget_code" || entry == "weights_sorted",
                "{entry}: only the widget pair lacks a packages/bundles counterpart"
            );
        }
    }
    assert_eq!(
        catalog_bytes, CATALOG_BYTES,
        "the catalog stands at its measured seed mass"
    );
    println!(
        "P163|seed|index {} B, catalog {} B, 37 entries; sha256 asserted on \
         {hashes_checked}/37 rows (system sha256sum; the product hash is Phase 164's sha2)",
        index_text.len(),
        catalog_bytes
    );
}

#[test]
fn p163_every_seed_bundle_reverifies() {
    let _guard = crown_serial();
    let root = repo_root();
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("registry/index.json")).unwrap(),
    )
    .unwrap();
    let entries = index["entries"].as_array().unwrap();
    let mut reproduced = 0usize;
    let mut unexamined = 0usize;
    for row in entries {
        let entry = row["entry"].as_str().unwrap();
        let text = std::fs::read_to_string(root.join(row["file"].as_str().unwrap()))
            .unwrap();
        match pcc::verify_bundle_text(&text) {
            Ok(_) => reproduced += 1,
            Err(e) => {
                // The one admissible refusal: a Tier-1
                // claim in a no-Z3 build stands
                // unexamined, by name — reported, never
                // counted as reproduced.
                assert!(
                    !cfg!(feature = "z3-support")
                        && row["tier"] == "tier-1"
                        && e.contains("unexamined"),
                    "{entry}: the seed bundle re-verifies: {e}"
                );
                unexamined += 1;
            }
        }
    }
    if cfg!(feature = "z3-support") {
        assert_eq!(reproduced, 37, "every seed bundle reproduces in a Z3 build");
    }
    println!(
        "P163|seed-verify|reproduced={reproduced} unexamined-by-name={unexamined} of 37"
    );
}
