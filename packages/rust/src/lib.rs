//! Morphic's standard library as ready-to-call Rust
//! functions: the 29 entries of the frozen 1.0 library
//! (EMBED_SCOPE §2.1), pre-generated from their
//! specifications by the Morphic compiler's native
//! pipeline — the pipeline that judged them — and
//! shipped with the standing that pipeline produced.
//!
//! Every function lives in [`stdlib`], under its
//! Morphic entry name, and carries rustdoc stating its
//! tier and verdict basis; the same facts travel in
//! machine-readable form in [`MANIFEST_JSON`] (the
//! frozen `morphic.manifest.json`, EMBED_SCOPE §2.4:
//! name, frozen signature, tier, verdict basis,
//! battery counts, and the function's own
//! documentation, verbatim).
//!
//! **Tiers.** Tier-1 functions are Z3-verified against
//! their specifications and golden-exact on their
//! frozen batteries. Tier-2 functions are
//! executed-exact on their full frozen batteries —
//! the tier is a property of the proof standard, not
//! of the behaviour; each function is held to 100%
//! agreement on every battery point. When the
//! post-1.0 list-element encoder verifies a Tier-2
//! entry, its label upgrades in place at a lockstep
//! release; the function's behaviour and bytes do not
//! change.
//!
//! **Types.** The Rust mapping is exact (EMBED_SCOPE
//! §3): `Int` is `i64`, `Char` is `char`, `String` is
//! `String`, `List<T>` is `Vec<T>`, `Option<T>` is
//! `Option<T>`, `Result<T, E>` is `Result<T, E>` —
//! nesting included, no losses. The builtins `len`
//! and `int_to_str` are not exports (they are language
//! builtins, rendered inside generated code); a host
//! uses `slice.len()`, `s.chars().count()` or
//! `x.to_string()` as appropriate.
//!
//! **Preconditions (NF-116-2, taken here).** A
//! function whose specification freezes a `require:`
//! row carries it as a genuine `debug_assert!` of the
//! rendered condition — `int_abs` (the Int minimum is
//! excluded), `int_clamp` (`lo <= hi`),
//! `list_binary_search` (the list is sorted; the
//! assert clones the list, since a proposition must
//! not consume the argument). The asserts are
//! debug-only: an out-of-domain call panics on the
//! assert in a debug build, and in a release build
//! the behaviour is whatever the body computes —
//! out-of-domain inputs are contract violations,
//! outside the battery-exactness promise. This crate
//! takes the NF-116-2 fix at its own surface: the
//! compiler's general Rust emitter still renders
//! parsed `require:` rows as `debug_assert!(true,
//! ...)` placeholders, because that renderer is
//! mirrored by the self-hosting Codegen spec and the
//! two emitters are held byte-equal; closing it there
//! is a separate lockstep change, recorded in the
//! project's roadmap. The asserts in this crate are
//! real.
//!
//! **Known defect — `char_to_digit`.** One entry of
//! the 29 does not stand at its battery in this
//! crate: its shipped body is a Match over Char
//! literal patterns, which the Rust emitter renders
//! as `_` patterns (the Char-pattern widening, in
//! lockstep with the synthesized emitter since the
//! M4 gate), so as generated it returns `Some(0)`
//! for every `char`. The defect is named here and
//! in the function's own rustdoc — not silently
//! shipped, and the entry is not silently dropped
//! from the frozen surface. The npm package's form
//! of the function is exact (the JavaScript
//! renderer carries Char patterns). Closing the
//! widening is a lockstep cascade recorded as the
//! assembling phase's named blocker in the
//! project's roadmap.
//!
//! **This crate is an artifact, not a compiler.** It
//! has no dependencies, no build script and no
//! synthesizer or verifier; it builds offline and
//! makes no runtime verification claims. The compiler
//! that produced it is a separate tool and is not
//! published.
//!
//! **Versioning.** The crate versions in lockstep
//! with Morphic itself: the crate version is the
//! Morphic version whose pipeline generated and
//! verified the contents (the manifest records it as
//! both `packageVersion` and `morphicVersion`).

pub mod stdlib;

/// The tier manifest (EMBED_SCOPE §2.4), verbatim:
/// `morphic.manifest.json`, the same JSON file this
/// package carries at its root.
pub const MANIFEST_JSON: &str = include_str!("../morphic.manifest.json");
