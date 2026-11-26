//! Phase 165 — discovery with no install directory
//! (DISEBAR_17_SCOPE §7): an absent directory is today's
//! behavior, byte-identical — the prelude is exactly the
//! frozen 35-entry stdlib, in its frozen order, and no
//! diagnostic is raised (absence is not degradation).
//!
//! This binary's one install state: `MORPHIC_REGISTRY_DIR`
//! names a path that does not exist. The environment is
//! set once, before any funnel read in this process.

#![cfg(not(feature = "wasm-playground"))]

use std::sync::Once;

use morphic::stdlib;

/// The frozen 35, in prelude order (the Phase 110 pin's
/// list, restated so this pin stands on its own).
const FROZEN_35: [&str; 35] = [
    "int_abs",
    "int_min",
    "int_max",
    "int_clamp",
    "int_sign",
    "int_is_even",
    "int_is_odd",
    "char_is_digit",
    "char_is_alpha",
    "char_is_whitespace",
    "char_is_uppercase",
    "char_is_lowercase",
    "char_to_digit",
    "string_is_empty",
    "result_unwrap_or",
    "option_unwrap_or",
    "option_ok_or",
    "list_is_empty",
    "list_reverse",
    "list_contains",
    "list_sum",
    "list_product",
    "list_min",
    "list_max",
    "list_take",
    "list_drop",
    "list_is_sorted",
    "list_sort",
    "list_binary_search",
    "list_map",
    "list_filter",
    "list_fold",
    "list_all",
    "list_any",
    "range",
];

fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join(format!(
            "morphic-phase165-absent-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("MORPHIC_REGISTRY_DIR", &dir);
    });
}

#[test]
fn p165_absent_directory_is_the_standing_prelude_byte_identical() {
    init();
    let specs = stdlib::prelude_specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names, FROZEN_35,
        "no install directory: the prelude is the frozen 35, in order"
    );
    assert!(
        stdlib::installed_diagnostics().is_empty(),
        "absence raises no diagnostic"
    );
    let asset = stdlib::library_asset();
    assert_eq!(asset.bodies.len(), 35, "every stdlib entry ships its body");
    for name in FROZEN_35 {
        assert!(
            stdlib::shipped_body(name).is_some(),
            "{name}: the stdlib body answers with no install directory"
        );
    }
}
