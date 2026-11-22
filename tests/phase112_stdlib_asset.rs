//! Phase 112: the stdlib asset pin.
//!
//! The playground ships the standard library as a
//! separately-loaded binary asset (playground/
//! stdlib-asset.bin) instead of data compiled into the
//! WASM bundle (the bundle route, pulled forward from
//! Phase 114 — see the banner in src/stdlib.rs). The
//! asset is generated from the native embedded data in
//! a hand-rolled stack-machine wire format (NOT
//! JSON/serde and not a text grammar — the measured
//! reasons are on IRNode's docs in synthesis/
//! engine.rs), and this pin is what keeps the two
//! forms one library:
//!
//! * the checked-in asset is byte-for-byte what the
//!   native data generates today (regenerate with
//!   `morphic stdlib-asset` — a stale asset is a red
//!   suite, never a silent drift);
//! * parsed back through the production parser, its
//!   modules are exactly the embedded modules, and
//!   its bodies are exactly the shipped bodies, in
//!   prelude order, complete over the prelude (the
//!   codec round-trips every shipped body);
//! * installing it through the public seam succeeds,
//!   and malformed bytes are refused.
//!
//! Configuration note: the first two pins compare against
//! the EMBEDDED data, which exists only in native builds
//! (`#[cfg(not(feature = "wasm-playground"))]` in
//! src/stdlib.rs — the browser build installs this very
//! asset at runtime instead). They are gated to match, so
//! a workspace `--all-targets` build (which unifies the
//! morphic-wasm member's `wasm-playground` feature into
//! this package) still compiles; in the standing native
//! configurations all three pins run.

use morphic::stdlib::{self, LibraryAsset};

fn checked_in_asset() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/playground/stdlib-asset.bin"
    ))
    .expect("the checked-in stdlib asset exists")
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn checked_in_asset_is_byte_fresh() {
    assert_eq!(
        checked_in_asset(),
        stdlib::library_asset_bytes(),
        "playground/stdlib-asset.bin is stale — regenerate it with `morphic stdlib-asset`"
    );
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn asset_carries_the_whole_library() {
    let asset: LibraryAsset =
        stdlib::parse_library_asset(&checked_in_asset()).expect("the asset parses");
    // Modules: exactly the embedded table, in order.
    let embedded: Vec<(&str, &str)> = stdlib::LIBRARY_MODULES.to_vec();
    assert_eq!(asset.modules.len(), embedded.len());
    for (module, (name, source)) in asset.modules.iter().zip(embedded.iter()) {
        assert_eq!(module.name, *name);
        assert_eq!(module.source, *source);
    }
    // Bodies: exactly the prelude's shipped bodies, in
    // prelude order, complete — every prelude spec ships
    // one. (Debug-string identity, as in the crowning
    // pin: IRNode has no PartialEq.)
    let specs = stdlib::prelude_specs();
    assert_eq!(asset.bodies.len(), specs.len());
    for ((name, body), spec) in asset.bodies.iter().zip(specs.iter()) {
        assert_eq!(name, &spec.name);
        let shipped = stdlib::shipped_body(name).expect("every entry ships a body");
        assert_eq!(
            format!("{body:?}"),
            format!("{shipped:?}"),
            "{name}: the asset body is the shipped body"
        );
    }
}

#[test]
fn asset_installs_through_the_public_seam() {
    // Malformed bytes are refused with a named error.
    assert!(stdlib::set_library_asset(b"not an asset").is_err());
    assert!(stdlib::set_library_asset(b"MORPHIC-STDLIB-ASSET 1\nX").is_err());
    // The checked-in asset installs. (One install per
    // process by design; this is the only test here that
    // installs.)
    stdlib::set_library_asset(&checked_in_asset()).expect("the asset installs");
    // A second install is refused, not silently swapped.
    assert!(stdlib::set_library_asset(&checked_in_asset()).is_err());
}
