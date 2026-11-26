//! Phase 164 — the registry verbs + the install flow
//! (DISEBAR_17_SCOPE §5, §8): `morphic registry` add /
//! list / remove / verify over a local index, the sha256
//! binding (`sha2`, the milestone's one new dependency),
//! and the receipt ledger of measured standings.
//!
//! Pins, in order:
//! 1. `sha2`'s placement (the native target table, not
//!    [dependencies] — §8) and the product hash itself:
//!    the standard vectors, and every one of the 37
//!    catalog rows' sha256 reproduced by
//!    `registry::sha256_hex` (the catalogue's hashes were
//!    computed with the system tool at Phase 163 — the
//!    product hash now answers for them);
//! 2. the stranger rehearsal against a LOCAL index built
//!    from the repo's own `registry/` (hermetic — no
//!    network in the suites): the widget pair installs
//!    into a scratch `MORPHIC_REGISTRY_DIR`, receipts
//!    record the re-measured standings, `list` shows
//!    them, `verify` passes named and swept, `remove`
//!    restores the pre-install standings; a stdlib entry
//!    from the same catalog refuses at the collision
//!    step, by name, with nothing written. In a no-Z3
//!    build the Tier-1 half of the rehearsal is the
//!    standing unexamined-by-name refusal and the
//!    Tier-2 entry installs on executed evidence;
//! 3. the tamper battery: hash mismatch (fetched bytes
//!    edited), flipped verdict, unsupported bundle
//!    version, a stdlib-named row in a doctored index,
//!    already-installed, unknown name, unknown version —
//!    each refused by name with nothing written, the
//!    refusals pairwise distinct;
//! 4. `verify` against an installed file edited after
//!    install (the receipt's hash names it) and against
//!    a receipt whose recorded standing was edited (the
//!    reproduction names it); a receipt-less bundle file
//!    is refused as not-an-install;
//! 5. the URL code path, hermetically: a local HTTP
//!    server (std TcpListener) serves the scratch
//!    catalog and `add --index http://…` installs
//!    through it; a 404 index refuses by its status. In
//!    a build without the HTTP client (no `llm-client`)
//!    a network fetch — default or --index URL —
//!    declines by name instead.
//!
//! Crown-bearing pins serialize on one lock (the Phase
//! 150 discipline: the engine's RNG is process-global).

#![cfg(not(feature = "wasm-playground"))]

use std::io::{Read, Write};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase164-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Copy the repo's own catalog into a scratch root:
/// `<dir>/registry/index.json` + the bundle files at
/// their repo-relative paths — the local index's
/// hermetic form (R1 resolution reads it exactly as it
/// reads the repo's).
fn scratch_catalog(tag: &str) -> std::path::PathBuf {
    let root = scratch_dir(&format!("{tag}-catalog"));
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("registry/index.json")).unwrap(),
    )
    .unwrap();
    let dst_index_dir = root.join("registry");
    std::fs::create_dir_all(dst_index_dir.join("bundles")).unwrap();
    std::fs::copy(
        repo_root().join("registry/index.json"),
        dst_index_dir.join("index.json"),
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

/// Run the shipped binary's registry family against a
/// scratch install dir.
fn run_registry(
    args: &[&str],
    install_dir: &std::path::Path,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_morphic"))
        .arg("registry")
        .args(args)
        .env("MORPHIC_REGISTRY_DIR", install_dir)
        .output()
        .expect("run the morphic binary")
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The scratch install dir's receipt ledger, parsed.
fn ledger(install_dir: &std::path::Path) -> serde_json::Value {
    let text = std::fs::read_to_string(install_dir.join("installed.json"))
        .expect("the receipt ledger exists");
    serde_json::from_str(&text).expect("the receipt ledger parses")
}

fn ledger_row<'a>(
    ledger: &'a serde_json::Value,
    entry: &str,
) -> Option<&'a serde_json::Value> {
    ledger["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["entry"] == entry)
}

/// Fix a doctored catalog's index row for `entry` to
/// bind the doctored bundle file's actual bytes (so the
/// shapes below reach the step they mean to exercise,
/// not the hash step).
fn rebind_row(catalog_root: &std::path::Path, entry: &str) {
    let index_path = catalog_root.join("registry/index.json");
    let mut index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).unwrap()).unwrap();
    for row in index["entries"].as_array_mut().unwrap() {
        if row["entry"] == entry {
            let bytes = std::fs::read(catalog_root.join(row["file"].as_str().unwrap()))
                .unwrap();
            row["sha256"] =
                serde_json::Value::String(morphic::registry::sha256_hex(&bytes));
            row["bytes"] = serde_json::Value::from(bytes.len() as u64);
        }
    }
    std::fs::write(
        &index_path,
        serde_json::to_string_pretty(&index).unwrap(),
    )
    .unwrap();
}

// ── 1. sha2: placement + the product hash ───────────────

#[test]
fn p164_sha2_placement_and_catalog_digests() {
    let root = repo_root();
    let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();

    // §8's placement: sha2 is declared exactly once, in
    // the native target table — never in [dependencies],
    // whose graph the browser profile shares.
    assert_eq!(
        cargo_toml.matches("sha2").count(),
        1,
        "sha2 is named exactly once in Cargo.toml (its dependency line)"
    );
    let target_at = cargo_toml
        .find("[target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]")
        .expect("the native target table exists");
    let sha2_at = cargo_toml.find("sha2 = ").expect("the sha2 line exists");
    assert!(sha2_at > target_at, "sha2 is declared under the native target table");
    let deps_section = cargo_toml
        .split("[dependencies]")
        .nth(1)
        .unwrap()
        .split('[')
        .next()
        .unwrap();
    assert!(
        !deps_section.contains("sha2"),
        "[dependencies] carries no sha2 — the browser profile's graph excludes it"
    );

    // The lock records the direct dependency on the
    // morphic package itself. (The block is found by
    // name and its version asserted against the
    // crate's own — the lockstep — rather than by a
    // literal version: the Phase 166 stamp moved the
    // tree to 1.7.0 and the literal went stale, the
    // Phase 161 re-pin class.)
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap();
    let crate_version = cargo_toml
        .lines()
        .find_map(|l| l.trim().strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("the crate's version line");
    let morphic_block = lock
        .split("[[package]]")
        .find(|b| b.contains("name = \"morphic\""))
        .expect("the lock carries the morphic package");
    assert!(
        morphic_block.contains(&format!("version = \"{crate_version}\"")),
        "the lock's morphic entry stands at the crate's version ({crate_version})"
    );
    assert!(
        morphic_block.contains("\"sha2\""),
        "the lock's morphic entry names sha2 as a direct dependency"
    );

    // The standard vectors.
    assert_eq!(
        morphic::registry::sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        morphic::registry::sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    // Every catalog row's hash — computed at Phase 163
    // with the system tool — is reproduced by the
    // product's own hash.
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("registry/index.json")).unwrap(),
    )
    .unwrap();
    let entries = index["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 37, "the seed catalog stands at 37 entries");
    for row in entries {
        let bytes = std::fs::read(root.join(row["file"].as_str().unwrap())).unwrap();
        assert_eq!(
            morphic::registry::sha256_hex(&bytes),
            row["sha256"].as_str().unwrap(),
            "{}: the product hash reproduces the catalogued hash",
            row["entry"]
        );
    }
    println!(
        "P164|sha2|placement=native-target-table (absent from [dependencies]); \
         vectors ok; 37/37 catalog hashes reproduced by registry::sha256_hex"
    );
}

// ── 2. The stranger rehearsal (local index) ─────────────

#[test]
fn p164_install_list_verify_remove() {
    let _guard = crown_serial();
    let catalog = scratch_catalog("flow");
    let index = catalog_index(&catalog);
    let install = scratch_dir("flow-install");

    if cfg!(feature = "z3-support") {
        // The widget pair installs.
        let out = run_registry(
            &["add", "is_widget_code", "--index", &index],
            &install,
        );
        assert!(
            out.status.success(),
            "add is_widget_code: {}",
            stderr_of(&out)
        );
        assert!(
            stdout_of(&out).contains("installed is_widget_code 1.7.0"),
            "the install reports its measured standing: {}",
            stdout_of(&out)
        );
        let out = run_registry(
            &["add", "weights_sorted", "--index", &index],
            &install,
        );
        assert!(out.status.success(), "add weights_sorted: {}", stderr_of(&out));

        // The installed files are the catalogued bytes.
        for entry in ["is_widget_code", "weights_sorted"] {
            assert_eq!(
                std::fs::read(install.join(format!("{entry}.bundle.json"))).unwrap(),
                std::fs::read(
                    repo_root().join(format!("registry/bundles/{entry}.bundle.json"))
                )
                .unwrap(),
                "{entry}: the installed file is the catalogued bundle, byte for byte"
            );
        }

        // A stdlib entry from the same catalog refuses at
        // the collision step — its claim reproduces, and
        // the name is still refused (§5.4 step 4, §6).
        let out = run_registry(&["add", "int_abs", "--index", &index], &install);
        assert!(!out.status.success(), "add int_abs must refuse");
        assert!(
            stderr_of(&out).contains("standard-library entry"),
            "the collision refusal names itself: {}",
            stderr_of(&out)
        );
        assert!(
            !install.join("int_abs.bundle.json").exists(),
            "a refused install writes no bundle file"
        );

        // The receipts record what the re-runs measured:
        // the bundles' own fields, the index's version +
        // hash, the index named, a date.
        let book = ledger(&install);
        assert_eq!(
            book["entries"].as_array().unwrap().len(),
            2,
            "exactly the two installs stand in the ledger (int_abs refused)"
        );
        let index_rows: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(repo_root().join("registry/index.json"))
                .unwrap(),
        )
        .unwrap();
        for entry in ["is_widget_code", "weights_sorted"] {
            let row = ledger_row(&book, entry).expect("a receipt row");
            let bundle: serde_json::Value = serde_json::from_slice(
                &std::fs::read(
                    repo_root().join(format!("registry/bundles/{entry}.bundle.json")),
                )
                .unwrap(),
            )
            .unwrap();
            let irow = index_rows["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["entry"] == entry)
                .unwrap();
            assert_eq!(row["version"], "1.7.0", "{entry}: the catalog version");
            assert_eq!(row["sha256"], irow["sha256"], "{entry}: the bound hash");
            for field in ["tier", "verdictBasis", "verdict", "crownRoute"] {
                assert_eq!(
                    row[field], bundle[field],
                    "{entry}: the receipt's {field} is the reproduced standing"
                );
            }
            assert_eq!(row["battery"], bundle["battery"], "{entry}: the battery");
            assert_eq!(row["index"], index, "{entry}: the index it resolved against");
            let date = row["installed"].as_str().unwrap();
            assert!(
                date.len() == 10 && date.as_bytes()[4] == b'-' && date.as_bytes()[7] == b'-',
                "{entry}: the receipt carries an ISO install date: {date}"
            );
        }

        // list shows the installed pair at their measured
        // tiers — and not the refused name.
        let out = run_registry(&["list"], &install);
        assert!(out.status.success());
        let text = stdout_of(&out);
        assert!(text.contains("is_widget_code") && text.contains("tier-1"), "{text}");
        assert!(text.contains("weights_sorted") && text.contains("tier-2"), "{text}");
        assert!(!text.contains("int_abs"), "the refused name is not listed: {text}");

        // verify: named and swept.
        let out = run_registry(&["verify", "is_widget_code"], &install);
        assert!(out.status.success(), "verify named: {}", stderr_of(&out));
        let out = run_registry(&["verify"], &install);
        assert!(out.status.success(), "verify sweep: {}", stderr_of(&out));
        let text = stdout_of(&out);
        assert!(
            text.contains("is_widget_code") && text.contains("weights_sorted"),
            "the sweep reports both entries: {text}"
        );
    } else {
        // The no-Z3 posture: a Tier-1 install refuses by
        // name (the claim stands unexamined) and writes
        // nothing; a Tier-2 entry installs on executed
        // evidence.
        let out = run_registry(
            &["add", "is_widget_code", "--index", &index],
            &install,
        );
        assert!(!out.status.success(), "a Tier-1 add must refuse without Z3");
        assert!(
            stderr_of(&out).contains("unexamined"),
            "the refusal is the standing unexamined shape: {}",
            stderr_of(&out)
        );
        assert!(
            !install.join("is_widget_code.bundle.json").exists(),
            "the refused install writes no bundle file"
        );
        assert!(
            !install.join("installed.json").exists(),
            "the refused install writes no ledger"
        );

        let out = run_registry(
            &["add", "weights_sorted", "--index", &index],
            &install,
        );
        assert!(
            out.status.success(),
            "a Tier-2 add installs on executed evidence: {}",
            stderr_of(&out)
        );
        let book = ledger(&install);
        let row = ledger_row(&book, "weights_sorted").expect("a receipt row");
        assert_eq!(row["tier"], "tier-2", "the receipt records the executed tier");
        let out = run_registry(&["list"], &install);
        assert!(stdout_of(&out).contains("weights_sorted"));
        let out = run_registry(&["verify", "weights_sorted"], &install);
        assert!(out.status.success(), "verify: {}", stderr_of(&out));
    }

    // remove restores the pre-install standings, in
    // every build: the file and the row go, the name
    // declines again, and removing it again declines.
    let out = run_registry(&["remove", "weights_sorted"], &install);
    assert!(out.status.success(), "remove: {}", stderr_of(&out));
    assert!(
        !install.join("weights_sorted.bundle.json").exists(),
        "the bundle file is removed"
    );
    let book = ledger(&install);
    assert!(
        ledger_row(&book, "weights_sorted").is_none(),
        "the receipt row is removed"
    );
    let out = run_registry(&["list"], &install);
    assert!(
        !stdout_of(&out).contains("weights_sorted"),
        "list no longer shows the removed entry"
    );
    let out = run_registry(&["verify", "weights_sorted"], &install);
    assert!(!out.status.success(), "verify of a removed name declines");
    assert!(
        stderr_of(&out).contains("not installed"),
        "the decline names itself: {}",
        stderr_of(&out)
    );
    let out = run_registry(&["remove", "weights_sorted"], &install);
    assert!(!out.status.success(), "removing an absent name declines");
    assert!(
        stderr_of(&out).contains("not installed"),
        "the decline names itself: {}",
        stderr_of(&out)
    );
    println!(
        "P164|flow|widget pair installed + receipts measured + list/verify/remove \
         (z3={}); the stdlib collision refused by name with nothing written",
        cfg!(feature = "z3-support")
    );
}

// ── 3. The tamper battery ───────────────────────────────

#[test]
fn p164_tamper_battery_refused_by_name() {
    let _guard = crown_serial();
    let mut refusals: Vec<String> = Vec::new();
    let refuse = |out: &Output, what: &str| -> String {
        assert!(!out.status.success(), "{what} must refuse");
        let err = stderr_of(out).trim().to_string();
        assert!(!err.is_empty(), "{what} refuses with a named reason");
        err
    };

    // (a) The fetched bytes are edited: the hash step
    // binds before any judging.
    {
        let catalog = scratch_catalog("tamper-hash");
        let bundle_path =
            catalog.join("registry/bundles/is_widget_code.bundle.json");
        let mut bytes = std::fs::read(&bundle_path).unwrap();
        bytes.push(b'\n');
        std::fs::write(&bundle_path, bytes).unwrap();
        let install = scratch_dir("tamper-hash-install");
        let out = run_registry(
            &["add", "is_widget_code", "--index", &catalog_index(&catalog)],
            &install,
        );
        let err = refuse(&out, "hash-mismatched add");
        assert!(err.contains("sha256 mismatch"), "the hash refusal: {err}");
        assert!(
            !install.join("is_widget_code.bundle.json").exists()
                && !install.join("installed.json").exists(),
            "a hash refusal writes nothing"
        );
        refusals.push(err);
    }

    // (b) The verdict is flipped (the index re-bound to
    // the doctored bytes, so the shape reaches the
    // re-judgment): the claim triple no longer coheres.
    {
        let catalog = scratch_catalog("tamper-verdict");
        let bundle_path =
            catalog.join("registry/bundles/weights_sorted.bundle.json");
        let text = std::fs::read_to_string(&bundle_path).unwrap();
        assert!(text.contains("\"verdict\": \"ExecutedExact\""));
        std::fs::write(
            &bundle_path,
            text.replace("\"verdict\": \"ExecutedExact\"", "\"verdict\": \"Verified\""),
        )
        .unwrap();
        rebind_row(&catalog, "weights_sorted");
        let install = scratch_dir("tamper-verdict-install");
        let out = run_registry(
            &["add", "weights_sorted", "--index", &catalog_index(&catalog)],
            &install,
        );
        let err = refuse(&out, "flipped-verdict add");
        assert!(err.contains("incoherent claim"), "the coherence refusal: {err}");
        assert!(
            !install.join("weights_sorted.bundle.json").exists()
                && !install.join("installed.json").exists(),
            "a re-judgment refusal writes nothing"
        );
        refusals.push(err);
    }

    // (c) An unsupported bundle version (index re-bound
    // likewise): the reader refuses the version by name.
    {
        let catalog = scratch_catalog("tamper-version");
        let bundle_path =
            catalog.join("registry/bundles/is_widget_code.bundle.json");
        let text = std::fs::read_to_string(&bundle_path).unwrap();
        assert!(text.contains("\"bundleVersion\": 1,"));
        std::fs::write(
            &bundle_path,
            text.replace("\"bundleVersion\": 1,", "\"bundleVersion\": 2,"),
        )
        .unwrap();
        rebind_row(&catalog, "is_widget_code");
        let install = scratch_dir("tamper-version-install");
        let out = run_registry(
            &["add", "is_widget_code", "--index", &catalog_index(&catalog)],
            &install,
        );
        let err = refuse(&out, "unsupported-version add");
        assert!(
            err.contains("unsupported bundle version"),
            "the version refusal: {err}"
        );
        assert!(
            !install.join("is_widget_code.bundle.json").exists()
                && !install.join("installed.json").exists(),
            "a version refusal writes nothing"
        );
        refusals.push(err);
    }

    // (d) A doctored index carrying only a stdlib-named
    // row: refused by name — at the collision step in a
    // Z3 build (the claim reproduces first), at the
    // re-judgment in a no-Z3 build (the Tier-1 claim
    // stands unexamined). Either way the name is
    // refused and nothing is written.
    {
        let catalog = scratch_catalog("tamper-stdlib");
        let index_path = catalog.join("registry/index.json");
        let mut index: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&index_path).unwrap())
                .unwrap();
        index["entries"] = serde_json::Value::Array(
            index["entries"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["entry"] == "int_abs")
                .cloned()
                .collect(),
        );
        std::fs::write(&index_path, serde_json::to_string_pretty(&index).unwrap())
            .unwrap();
        let install = scratch_dir("tamper-stdlib-install");
        let out = run_registry(
            &["add", "int_abs", "--index", &catalog_index(&catalog)],
            &install,
        );
        let err = refuse(&out, "stdlib-named add");
        if cfg!(feature = "z3-support") {
            assert!(
                err.contains("standard-library entry"),
                "the collision refusal: {err}"
            );
        } else {
            assert!(err.contains("unexamined"), "the unexamined refusal: {err}");
        }
        assert!(
            !install.join("int_abs.bundle.json").exists()
                && !install.join("installed.json").exists(),
            "a stdlib-named refusal writes nothing"
        );
        refusals.push(err);
    }

    // (e) Already installed: the second add refuses by
    // name (the update path is remove + add).
    {
        let catalog = scratch_catalog("tamper-twice");
        let index = catalog_index(&catalog);
        let install = scratch_dir("tamper-twice-install");
        let entry = if cfg!(feature = "z3-support") {
            "is_widget_code"
        } else {
            "weights_sorted"
        };
        let out = run_registry(&["add", entry, "--index", &index], &install);
        assert!(out.status.success(), "the first add installs: {}", stderr_of(&out));
        let out = run_registry(&["add", entry, "--index", &index], &install);
        let err = refuse(&out, "second add");
        assert!(err.contains("already installed"), "the refusal: {err}");
        refusals.push(err);
    }

    // (f) An unknown name, and (g) an unknown version of
    // a known name: resolution refuses both by name.
    {
        let catalog = scratch_catalog("tamper-unknown");
        let index = catalog_index(&catalog);
        let install = scratch_dir("tamper-unknown-install");
        let out = run_registry(
            &["add", "no_such_entry", "--index", &index],
            &install,
        );
        let err = refuse(&out, "unknown-name add");
        assert!(
            err.contains("carries no entry named 'no_such_entry'"),
            "the refusal: {err}"
        );
        refusals.push(err);

        let out = run_registry(
            &["add", "is_widget_code@9.9.9", "--index", &index],
            &install,
        );
        let err = refuse(&out, "unknown-version add");
        assert!(
            err.contains("at version 1.7.0, not 9.9.9"),
            "the refusal: {err}"
        );
        refusals.push(err);

        // …and the named version that IS carried resolves.
        if cfg!(feature = "z3-support") {
            let out = run_registry(
                &["add", "is_widget_code@1.7.0", "--index", &index],
                &install,
            );
            assert!(
                out.status.success(),
                "add at the carried version installs: {}",
                stderr_of(&out)
            );
        }
    }

    let distinct: std::collections::HashSet<&String> = refusals.iter().collect();
    assert_eq!(
        distinct.len(),
        refusals.len(),
        "every shape's refusal is its own: {refusals:?}"
    );
    println!(
        "P164|tamper|{} refusal shapes, pairwise distinct, nothing written by any",
        refusals.len()
    );
}

// ── 4. verify against post-install tampering ────────────

#[test]
fn p164_verify_detects_installed_tamper() {
    let _guard = crown_serial();
    let catalog = scratch_catalog("vtamper");
    let index = catalog_index(&catalog);
    let install = scratch_dir("vtamper-install");

    let out = run_registry(&["add", "weights_sorted", "--index", &index], &install);
    assert!(out.status.success(), "install: {}", stderr_of(&out));
    let out = run_registry(&["verify", "weights_sorted"], &install);
    assert!(out.status.success(), "verify before tamper: {}", stderr_of(&out));

    // The installed file is edited: the receipt's hash
    // names the change before any judging runs.
    let installed_file = install.join("weights_sorted.bundle.json");
    let pristine = std::fs::read(&installed_file).unwrap();
    let mut edited = pristine.clone();
    edited.push(b'\n');
    std::fs::write(&installed_file, edited).unwrap();
    let out = run_registry(&["verify", "weights_sorted"], &install);
    assert!(!out.status.success(), "verify of an edited file must refuse");
    assert!(
        stderr_of(&out).contains("changed after install"),
        "the refusal names the post-install change: {}",
        stderr_of(&out)
    );
    let out = run_registry(&["verify"], &install);
    assert!(!out.status.success(), "the sweep refuses too");

    // The file restored, the receipt's recorded standing
    // edited instead: the reproduction names the
    // difference.
    std::fs::write(&installed_file, &pristine).unwrap();
    let ledger_path = install.join("installed.json");
    let book = std::fs::read_to_string(&ledger_path).unwrap();
    assert!(book.contains("\"passed\": 5"), "the receipt stands as installed");
    std::fs::write(
        &ledger_path,
        book.replacen("\"passed\": 5", "\"passed\": 4", 1),
    )
    .unwrap();
    let out = run_registry(&["verify", "weights_sorted"], &install);
    assert!(!out.status.success(), "verify against an edited receipt must refuse");
    assert!(
        stderr_of(&out).contains("differs from the receipt"),
        "the refusal names the standing difference: {}",
        stderr_of(&out)
    );

    // A receipt-less bundle file is not an install and
    // cannot be verified as one — named, or swept.
    let stray_dir = scratch_dir("vtamper-stray");
    std::fs::copy(
        repo_root().join("registry/bundles/is_widget_code.bundle.json"),
        stray_dir.join("is_widget_code.bundle.json"),
    )
    .unwrap();
    let out = run_registry(&["verify", "is_widget_code"], &stray_dir);
    assert!(!out.status.success(), "verify of a receipt-less file declines");
    assert!(
        stderr_of(&out).contains("no receipt"),
        "the decline names itself: {}",
        stderr_of(&out)
    );
    let out = run_registry(&["verify"], &stray_dir);
    assert!(!out.status.success(), "the sweep names the stray file too");
    assert!(
        stderr_of(&out).contains("no receipt"),
        "the sweep's refusal: {}",
        stderr_of(&out)
    );
    println!(
        "P164|verify-tamper|edited file → 'changed after install'; edited receipt \
         → 'differs from the receipt'; receipt-less file → not an install"
    );
}

// ── 5. The URL code path, hermetically ──────────────────

/// A minimal HTTP file server over the scratch catalog
/// root (std only): GET <path> serves the file at that
/// path under the root, 404 otherwise. Runs until the
/// stop flag is set.
fn serve_root(
    root: std::path::PathBuf,
) -> (u16, Arc<AtomicBool>, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    let handle = std::thread::spawn(move || {
        while !stop_flag.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream
                        .set_read_timeout(Some(std::time::Duration::from_secs(5)));
                    let mut buf = [0u8; 8192];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]);
                    let path = request
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or("/")
                        .to_string();
                    let file = root.join(path.trim_start_matches('/'));
                    match std::fs::read(&file) {
                        Ok(bytes) => {
                            let head = format!(
                                "HTTP/1.0 200 OK\r\ncontent-length: {}\r\n\r\n",
                                bytes.len()
                            );
                            let _ = stream.write_all(head.as_bytes());
                            let _ = stream.write_all(&bytes);
                        }
                        Err(_) => {
                            let _ = stream.write_all(
                                b"HTTP/1.0 404 Not Found\r\ncontent-length: 0\r\n\r\n",
                            );
                        }
                    }
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(_) => break,
            }
        }
    });
    (port, stop, handle)
}

#[test]
fn p164_url_index_and_no_http_posture() {
    let _guard = crown_serial();
    let catalog = scratch_catalog("url");
    let install = scratch_dir("url-install");

    if cfg!(feature = "llm-client") {
        let (port, stop, server) = serve_root(catalog.clone());
        let index_url = format!("http://127.0.0.1:{port}/registry/index.json");

        // A 404 index refuses by its status.
        let missing = format!("http://127.0.0.1:{port}/missing/registry/index.json");
        let out = run_registry(
            &["add", "is_widget_code", "--index", &missing],
            &install,
        );
        assert!(!out.status.success(), "a 404 index must refuse");
        assert!(
            stderr_of(&out).contains("HTTP 404"),
            "the refusal names the status: {}",
            stderr_of(&out)
        );

        if cfg!(feature = "z3-support") {
            let out = run_registry(
                &["add", "is_widget_code", "--index", &index_url],
                &install,
            );
            assert!(
                out.status.success(),
                "add over the URL index installs: {}",
                stderr_of(&out)
            );
            let book = ledger(&install);
            let row = ledger_row(&book, "is_widget_code").expect("a receipt row");
            assert_eq!(
                row["index"], index_url,
                "the receipt records the URL it resolved against"
            );
            let out = run_registry(&["verify", "is_widget_code"], &install);
            assert!(out.status.success(), "verify: {}", stderr_of(&out));
        }
        stop.store(true, Ordering::SeqCst);
        let _ = server.join();
        println!(
            "P164|url|the URL code path installs through a local HTTP server; \
             a 404 index refuses by status (z3={})",
            cfg!(feature = "z3-support")
        );
    } else {
        // No HTTP client in this build: a network fetch —
        // an --index URL or the canonical default alike —
        // declines by name, while the local path (pin 2)
        // works.
        for args in [
            vec!["add", "is_widget_code", "--index", "http://127.0.0.1:9/registry/index.json"],
            vec!["add", "is_widget_code"],
        ] {
            let out = run_registry(&args, &install);
            assert!(!out.status.success(), "a network add must decline: {args:?}");
            assert!(
                stderr_of(&out).contains("network fetch is unavailable"),
                "the decline names the build's posture: {}",
                stderr_of(&out)
            );
        }
        assert!(
            !install.join("installed.json").exists(),
            "a declined fetch writes nothing"
        );
        println!(
            "P164|url|no HTTP client in this build — network fetch declines by \
             name (URL index and canonical default alike)"
        );
    }
}
