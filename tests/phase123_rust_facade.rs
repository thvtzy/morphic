//! Phase 123 (1.1 Embeddable, EMBED_SCOPE §5): the Rust
//! facade crate (D7a) — assembly + publish-readiness
//! (dry-run only, R5). The pins:
//!
//! * the assembly is deterministic — two assemblies are
//!   byte-identical file sets, in the same order;
//! * the checked-in crate in `packages/rust/` equals a
//!   fresh assembly byte-for-byte, file set included —
//!   no drift and no stray files (the Phase 112/122
//!   discipline; the tarball hygiene the release will
//!   publish from is this pin's subject). Build residue
//!   (`target/`, `Cargo.lock`) is not package surface
//!   and is tolerated by name;
//! * `Cargo.toml` carries the frozen shape: the
//!   crates.io name `morphic` (D8a), the version
//!   lockstep (the compiler crate's version, which
//!   Phase 125 stamps to 1.1.0 by bumping the compiler
//!   and re-running the assembly), its own `[workspace]`
//!   root (the naming resolution: the repo's root
//!   package is the unpublished compiler of the same
//!   name; the facade is a separate workspace), and NO
//!   dependency sections — the crate is dependency-free
//!   by construction (no compiler, no Z3);
//! * the crate's manifest rows correspond exactly to
//!   the npm package's manifest (§2.4): identical rows,
//!   only the `package` field differing (`morphic`,
//!   not `@thvtzy/morphic`);
//! * the generated module carries exactly the 29 frozen
//!   functions in §2.1's order, each with its tier +
//!   verdict basis + battery in its rustdoc, under the
//!   generated module's scoped lint allowances;
//! * NF-116-2 is taken at the crate's surface: no
//!   `debug_assert!(true` placeholder anywhere; the
//!   three frozen preconditions render genuinely
//!   (verbatim pins), and no other function carries a
//!   precondition block;
//! * THE CHAR-PATTERN BLOCKER, pinned as it stands in
//!   the Phase 118 probe tradition: `char_to_digit`'s
//!   generated form is the widened one (eleven `_`
//!   arms), its defect note stands at its rustdoc, the
//!   crate docs and the README — exactly one function
//!   carries a defect note — and the compiled harness
//!   asserts the measured wrong behaviour as today's
//!   truth. When the widening closes (the lockstep
//!   cascade, route (i), or the facade-local rewrite,
//!   route (ii)), these pins fail loudly and are
//!   re-aimed at the new truth at their sites;
//! * the generated module compiles standalone under
//!   `rustc --test` and answers the representative
//!   frozen battery (the Phase 121/122 host set minus
//!   the blocked entry: 116 points) at 100%, with the
//!   three genuine asserts panicking on out-of-domain
//!   calls under debug assertions.
//!
//! Configuration note (the standing one): pins that read
//! the NATIVE prelude are gated
//! `#[cfg(not(feature = "wasm-playground"))]`.

#[cfg(not(feature = "wasm-playground"))]
use std::collections::BTreeMap;
#[cfg(not(feature = "wasm-playground"))]
use std::path::{Path, PathBuf};

#[cfg(not(feature = "wasm-playground"))]
use morphic::codegen::bindings;
#[cfg(not(feature = "wasm-playground"))]
use morphic::codegen::facade;
#[cfg(not(feature = "wasm-playground"))]
use morphic::codegen::package::PackageFile;

#[cfg(not(feature = "wasm-playground"))]
fn assembled() -> BTreeMap<String, String> {
    facade::assemble_rust_facade()
        .expect("the facade assembles")
        .into_iter()
        .map(|f| (f.path, f.contents))
        .collect()
}

#[cfg(not(feature = "wasm-playground"))]
fn crate_home() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(facade::RUST_PACKAGE_DIR)
}

// ── Determinism + the file set ───────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn assembly_is_deterministic() {
    let a: Vec<PackageFile> = facade::assemble_rust_facade().expect("assembly runs");
    let b: Vec<PackageFile> = facade::assemble_rust_facade().expect("assembly re-runs");
    assert_eq!(a, b, "two assemblies are byte-identical file sets");
    let paths: Vec<&str> = a.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "Cargo.toml",
            "README.md",
            "LICENSE-MIT",
            "LICENSE-APACHE",
            "morphic.manifest.json",
            "src/lib.rs",
            "src/stdlib.rs",
        ],
        "the facade file set: crate furniture + manifest + the two sources"
    );
}

// ── The checked-in crate ─────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
fn collect_checked_in(dir: &Path, prefix: &str, out: &mut BTreeMap<String, String>) {
    for entry in std::fs::read_dir(dir).expect("crate home reads") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if entry.file_type().expect("file type").is_dir() {
            if name == "target" {
                continue; // build residue, not package surface
            }
            collect_checked_in(&entry.path(), &rel, out);
            continue;
        }
        if name == "Cargo.lock" {
            continue; // build residue, not package surface
        }
        let bytes = std::fs::read(entry.path()).expect("crate file reads");
        out.insert(
            rel,
            String::from_utf8(bytes).expect("crate files are text"),
        );
    }
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn checked_in_facade_matches_fresh_assembly() {
    let fresh = assembled();
    let mut on_disk: BTreeMap<String, String> = BTreeMap::new();
    collect_checked_in(&crate_home(), "", &mut on_disk);
    assert_eq!(
        on_disk, fresh,
        "the checked-in crate is the fresh assembly, byte-for-byte, \
         with no stray files (regenerate with `morphic build --facade`)"
    );
}

// ── Cargo.toml ───────────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn cargo_toml_carries_the_frozen_shape() {
    let files = assembled();
    let toml = &files["Cargo.toml"];
    assert!(
        toml.contains(&format!("name = \"{}\"", facade::CRATE_NAME)),
        "the crates.io name (D8a)"
    );
    // The lockstep (R6): the crate version, the
    // manifest's two version fields and the compiler
    // crate's version are one value from one source.
    assert!(
        toml.contains(&format!("version = \"{}\"", env!("CARGO_PKG_VERSION"))),
        "the crate version is the compiler crate's version"
    );
    assert!(toml.contains("license = \"MIT OR Apache-2.0\""));
    assert!(toml.contains("edition = \"2021\""));
    assert!(
        toml.contains("[workspace]"),
        "its own workspace root (the naming resolution)"
    );
    for section in ["[dependencies]", "[dev-dependencies]", "[build-dependencies]"] {
        assert!(
            !toml.contains(section),
            "the facade is dependency-free by construction: no {section}"
        );
    }
    assert!(!toml.contains("build ="), "no build script (docs.rs-safe)");
}

// ── The manifest correspondence (§2.4) ───────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn manifest_rows_match_the_npm_manifest() {
    let files = assembled();
    let facade_manifest: serde_json::Value =
        serde_json::from_str(&files["morphic.manifest.json"]).expect("facade manifest parses");
    assert_eq!(facade_manifest["package"], "morphic", "this package's name");
    assert_eq!(facade_manifest["packageVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(facade_manifest["morphicVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(facade_manifest["scope"], "EMBED_SCOPE.md");

    let npm = bindings::stdlib_bindings().expect("the npm binding set generates");
    let npm_manifest: serde_json::Value =
        serde_json::from_str(&npm.manifest_json).expect("npm manifest parses");
    assert_eq!(npm_manifest["package"], "@thvtzy/morphic");
    assert_eq!(
        facade_manifest["exports"], npm_manifest["exports"],
        "the rows are the npm package's rows exactly — names, signatures, \
         tiers, verdict bases, batteries, docs verbatim (§2.4)"
    );
    assert_eq!(
        facade_manifest["exports"].as_array().expect("rows").len(),
        29,
        "the frozen surface"
    );
}

// ── Licenses ─────────────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn license_files_are_the_repo_texts() {
    let files = assembled();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for name in ["LICENSE-MIT", "LICENSE-APACHE"] {
        let repo_text = std::fs::read_to_string(root.join(name)).expect("repo license reads");
        assert_eq!(
            files[name], repo_text,
            "{name} in the crate is the repo's text, byte for byte"
        );
    }
}

// ── The generated module's shape ─────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn stdlib_module_carries_the_29_with_rustdoc_tiers() {
    let files = assembled();
    let module = &files["src/stdlib.rs"];
    assert!(
        module.contains("#![allow(unused_parens, unreachable_patterns, unused_mut)]"),
        "the generated module's scoped lint allowances stand"
    );
    let inputs = bindings::stdlib_inputs();
    assert_eq!(inputs.len(), 29);
    // Exactly the 29 definitions, in §2.1's order.
    let mut cursor = 0usize;
    for input in &inputs {
        let needle = format!("pub fn {}(", input.spec.name);
        let at = module[cursor..]
            .find(&needle)
            .unwrap_or_else(|| panic!("{} is defined, in §2.1 order", input.spec.name))
            + cursor;
        // Its rustdoc tier lines stand just before it
        // (the doc block + tier lines precede the
        // signature; the window covers the longest
        // frozen doc).
        let window = &module[at.saturating_sub(1600)..at];
        assert!(
            window.contains(&format!("Tier: {}", input.tier)),
            "{}: rustdoc carries its tier",
            input.spec.name
        );
        assert!(
            window.contains(&format!("Verdict basis: {}", input.verdict_basis)),
            "{}: rustdoc carries its verdict basis",
            input.spec.name
        );
        assert!(
            window.contains(&format!(
                "Battery: {} pinned, {} spec-silent",
                input.battery_pinned, input.battery_silent
            )),
            "{}: rustdoc carries its battery",
            input.spec.name
        );
        cursor = at + needle.len();
    }
    assert_eq!(
        module.matches("pub fn ").count(),
        29,
        "exactly the 29 frozen functions, no helper, no extra name"
    );
}

// ── NF-116-2: taken at this surface ──────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn nf116_2_genuine_asserts_replace_the_placeholder() {
    let files = assembled();
    let module = &files["src/stdlib.rs"];
    assert!(
        !module.contains("debug_assert!(true"),
        "no precondition degrades to the placeholder in this crate"
    );
    // The three frozen preconditions, rendered
    // genuinely (verbatim pins — the facade renderer
    // of codegen::facade, incl. the clone rule for the
    // predicate's List argument).
    for genuine in [
        "debug_assert!((x != ((0 - 9223372036854775807) - 1)), \"Precondition violated\");",
        "debug_assert!((lo <= hi), \"Precondition violated\");",
        "debug_assert!(list_is_sorted(xs.clone()), \"Precondition violated\");",
    ] {
        assert!(
            module.contains(genuine),
            "the genuine assert stands: {genuine}"
        );
    }
    assert_eq!(
        module.matches("\"Precondition violated\"").count(),
        3,
        "exactly the three preconditioned entries carry an assert"
    );
    assert_eq!(
        module.matches("#[cfg(debug_assertions)]").count(),
        3,
        "the standing Phase 55 form: debug-only blocks"
    );
}

// ── The Char-pattern blocker, pinned as it stands ────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn char_pattern_widening_is_pinned_as_it_stands() {
    let files = assembled();
    let module = &files["src/stdlib.rs"];
    // char_to_digit's generated form: the match over
    // eleven widened arms (the pattern renderer's
    // Literal catch-all — see codegen::facade's
    // header for the finding and the priced routes).
    let start = module
        .find("pub fn char_to_digit(")
        .expect("char_to_digit is carried");
    let end = module[start..]
        .find("\npub fn ")
        .map(|i| start + i)
        .unwrap_or(module.len());
    let segment = &module[start..end];
    assert!(segment.contains("match c {"), "{segment}");
    assert_eq!(
        segment.matches("_ => {").count(),
        11,
        "all eleven Char literal arms widen to `_` as the emitter stands"
    );
    // The defect is named at every surface the crate
    // owns, exactly for this one entry.
    assert_eq!(
        module.matches("Known defect").count(),
        1,
        "exactly one function carries a defect note: char_to_digit"
    );
    assert!(files["src/lib.rs"].contains("Known defect — `char_to_digit`"));
    assert!(files["README.md"].contains("## Known defect"));
    assert!(files["README.md"].contains("`char_to_digit`"));
}

// ── The compiled battery (rustc --test harness) ──────

/// The harness battery: the Phase 121/122 clean-host
/// set minus the blocked entry — int_clamp 13,
/// option_unwrap_or 28, option_ok_or 9,
/// result_unwrap_or 36, list_sum 15, list_min 15 =
/// 116 frozen points, expectations by the batteries'
/// own mirrors — plus the widening pin (today's
/// measured truth) and the three assert panics.
#[cfg(not(feature = "wasm-playground"))]
const HARNESS_TESTS: &str = r##"
#[cfg(test)]
mod facade_battery {
    use super::*;

    #[test]
    fn battery_int_clamp() {
        let triples: [(i64, i64, i64); 13] = [
            (5, 0, 10), (-3, 0, 10), (99, 0, 10), (0, 0, 10), (10, 0, 10),
            (5, 5, 5), (-7, -10, -5), (-12, -10, -5), (0, -5, 5),
            (i64::MAX, 0, i64::MAX), (i64::MIN, 0, i64::MAX),
            (7, 7, 7), (-1, 0, 0),
        ];
        for (x, lo, hi) in triples {
            assert_eq!(int_clamp(x, lo, hi), x.clamp(lo, hi));
        }
    }

    #[test]
    fn battery_option_unwrap_or() {
        let options: Vec<Option<i64>> = vec![
            Some(0), Some(1), Some(-1), Some(42), Some(i64::MAX), Some(i64::MIN), None,
        ];
        let defaults = [0, 5, -3, i64::MIN];
        for o in &options {
            for d in defaults {
                assert_eq!(option_unwrap_or(*o, d), o.unwrap_or(d));
            }
        }
    }

    #[test]
    fn battery_option_ok_or() {
        let cases: Vec<(Option<i64>, String)> = vec![
            (Some(0), String::new()),
            (Some(1), "x".into()),
            (Some(-1), "missing".into()),
            (Some(i64::MAX), "e".into()),
            (Some(i64::MIN), "unused".into()),
            (None, String::new()),
            (None, "missing".into()),
            (None, "a longer error message".into()),
            (None, "with \"quotes\" inside".into()),
        ];
        for (o, e) in &cases {
            assert_eq!(option_ok_or(*o, e.clone()), o.ok_or(e.clone()));
        }
    }

    #[test]
    fn battery_result_unwrap_or() {
        let results: Vec<Result<i64, String>> = vec![
            Ok(0), Ok(1), Ok(-1), Ok(42), Ok(i64::MAX), Ok(i64::MIN),
            Err(String::new()),
            Err("bad".into()),
            Err("a longer error message with spaces".into()),
        ];
        let defaults = [0, 3, -7, i64::MAX];
        for r in &results {
            for d in defaults {
                assert_eq!(result_unwrap_or(r.clone(), d), r.clone().unwrap_or(d));
            }
        }
    }

    #[test]
    fn battery_list_sum_min() {
        let lists: Vec<Vec<i64>> = vec![
            vec![], vec![0], vec![1], vec![-1], vec![5],
            vec![1, 2, 3], vec![3, 2, 1], vec![2, 2, 2], vec![1, 2, 2, 3],
            vec![5, 4, 3, 2, 1], vec![3, 1, 4, 1, 5, 9, 2, 6],
            vec![10, -3, 7, -7], vec![i64::MAX], vec![i64::MIN],
            vec![i64::MAX, i64::MIN],
        ];
        for xs in &lists {
            assert_eq!(list_sum(xs.clone()), xs.iter().sum::<i64>());
            assert_eq!(list_min(xs.clone()), xs.iter().min().copied());
        }
    }

    /// The blocker, as it stands: the widened match
    /// returns the first arm for every input. These
    /// assertions state today's measured truth (the
    /// Phase 118 tradition); the cascade that closes
    /// the widening re-aims them.
    #[test]
    fn widening_pin_char_to_digit_as_it_stands() {
        assert_eq!(char_to_digit('0'), Some(0), "the one agreeing point");
        assert_eq!(char_to_digit('5'), Some(0), "widened: the first arm answers");
        assert_eq!(char_to_digit(' '), Some(0), "widened: the first arm answers");
        assert_eq!(char_to_digit('a'), Some(0), "widened: the first arm answers");
    }

    #[test]
    #[should_panic(expected = "Precondition violated")]
    fn assert_int_abs_minimum() {
        let _ = int_abs(i64::MIN);
    }

    #[test]
    #[should_panic(expected = "Precondition violated")]
    fn assert_int_clamp_inverted() {
        let _ = int_clamp(5, 10, 0);
    }

    #[test]
    #[should_panic(expected = "Precondition violated")]
    fn assert_list_binary_search_unsorted() {
        let _ = list_binary_search(vec![2, 1], 1);
    }
}
"##;

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn facade_module_compiles_and_answers_its_battery() {
    let files = assembled();
    let mut source = files["src/stdlib.rs"].clone();
    source.push_str(HARNESS_TESTS);
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase123-facade-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join("facade_stdlib.rs");
    std::fs::write(&src_path, &source).unwrap();
    let bin_path = dir.join("facade_stdlib_test");
    let compile = std::process::Command::new("rustc")
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
        "the generated module must compile standalone:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = std::process::Command::new(&bin_path)
        .output()
        .expect("the compiled test binary runs");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && stdout.contains("test result: ok. 9 passed"),
        "the compiled facade must pass 9/9 harness tests, got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
