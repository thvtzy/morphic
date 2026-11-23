// ─────────────────────────────────────────────────────────────
//  RUST FACADE CRATE ASSEMBLY (Phase 123, 1.1 Embeddable
//  — EMBED_SCOPE §5, decision D7a, resolutions R5/R6/R9)
//
//  The assembly: the 29 frozen entries (EMBED_SCOPE
//  §2.1) become the slim crates.io crate `morphic` — the
//  pre-generated stdlib functions as real Rust code under
//  `morphic::stdlib`, plus the frozen tier manifest
//  (`morphic.manifest.json`, §2.4) and the furniture a
//  registry crate carries (`Cargo.toml`, a README, the
//  project's two license texts, byte-copies of the repo
//  root's). No compiler dependency, no Z3, no build-time
//  downloads: the crate is plain generated source and
//  builds offline, standalone, docs.rs-safe.
//
//  PLACEMENT + NAMING (resolved at this phase against
//  the repo's actual layout): the repo's root package is
//  itself named `morphic` — the compiler, which D7a
//  keeps unpublished ("the compiler crate stays the
//  repo's own tool"; the crates.io name is spent on the
//  facade). Cargo forbids two same-named packages in ONE
//  workspace, and the root workspace is the compiler's;
//  the facade therefore lives in its own workspace root
//  at `packages/rust/` — its generated `Cargo.toml`
//  carries an empty `[workspace]` table, the standard
//  shape for a separately publishable crate inside a
//  larger repo. The name `morphic` is carried by the
//  facade alone; the two crates never share a workspace
//  and the collision is invisible to the registry.
//
//  GENERATION: each function is rendered from its
//  shipped body by the very renderer the Phase 116 Rust
//  route uses for a standalone candidate
//  (`render_rust_function` — signature, body, doc head),
//  minus its file-head form: the spec's `doc:` moves
//  from `//!` file docs to the function's rustdoc, joined
//  by the tier + verdict basis + battery lines (the
//  §2.4 prose carriage, mirroring Phase 121's JSDoc).
//  Bodies are the pipeline's rendering byte-for-byte;
//  nothing is transcribed by hand. The generated module
//  carries scoped lint allowances for the three stylistic
//  lints the emitter's uniform style trips (measured at
//  this phase: 51 warnings over the 29 functions —
//  `unused_parens` ×40, `unreachable_patterns` ×10,
//  `unused_mut` ×1): generated code states its allowances
//  rather than editing the rendering the pipeline
//  produced, so gate clause 3's no-warnings build holds
//  without a second transcription of the bodies.
//
//  NF-116-2 — CLOSED GLOBALLY (Phase 124). The
//  finding: generated Rust degraded every parsed
//  `require:` row to `debug_assert!(true, …)`, because
//  a parsed require lands in `Constraint::Expr` /
//  `Constraint::Predicate` and the native
//  `constraint_to_rust` rendered only the engine-
//  constructed forms. Phase 123 took the fix at this
//  crate's surface only (a facade-local renderer
//  substituted genuine asserts for the placeholder
//  block), naming the global fix as residue: the
//  renderer is mirrored by the self-host Codegen
//  spec's equations, so closing it there was a
//  lockstep cascade of its own. Phase 124 ran that
//  cascade: `constraint_to_rust` now renders Expr /
//  Predicate / Or / Implies and all four Order
//  operators genuinely, in the native emitter and
//  its self-host mirror alike (the clone rule — a
//  plain-variable call argument clones at assert
//  position, since a proposition must not consume
//  the function's argument — is the renderer's own),
//  and this crate's local renderer is RETIRED: the
//  three frozen preconditions are produced by the
//  global path, byte-identical to what the local
//  renderer produced (`int_abs` asserts
//  `x != ((0 - 9223372036854775807) - 1)`, `int_clamp`
//  asserts `(lo <= hi)`, `list_binary_search` asserts
//  `list_is_sorted(xs.clone())`). The asserts keep
//  the standing Phase 55 form — `debug_assert!`
//  under `#[cfg(debug_assertions)]` — so an
//  out-of-domain call panics in a debug build (the
//  R3 contract) and the block compiles away in
//  release. The one named fallback: quantified rows
//  (Forall/Exists) have no value-level rendering a
//  debug_assert could evaluate and keep the `true`
//  text — named in the renderer's own header, in
//  the Codegen spec's family-0 doc, and in the
//  Phase 124 ROAD note; none of the 29 carries one
//  (pinned).
//
//  THE CHAR-PATTERN WIDENING — CLOSED (Phase 124).
//  Phase 123's host rehearsal stood at 28/29: the
//  failure was `char_to_digit`, whose shipped body
//  is a Match over Char literal patterns, and the
//  Rust emitter's pattern renderer
//  (`ir_pattern_to_rust`) rendered Int/Bool/String
//  literal patterns literally but Char through its
//  `_` catch-all — all eleven arms widened, the
//  first matched every input, and the function
//  returned Some(0) for every Char. The widening
//  was the F4 caveat the M4 gate recorded, held in
//  lockstep with the synthesized emitter. The owner
//  chose route (i) — the full lockstep cascade —
//  and Phase 124 closed it in both emitters: Char
//  literal patterns render literally (with the
//  expression renderer's escaping), the F4 caveat
//  is retired and F4's resolution is
//  unconditional, and this crate's host rehearsal
//  now stands at 29/29, `char_to_digit` at 101/101
//  included. The defect note this crate carried at
//  every surface (KNOWN_DEFECTS and its rustdoc
//  injection) is retired with the defect.
//
//  REGENERATION DISCIPLINE (gate clause 4's machinery,
//  the Phase 122 precedent): the checked assembly —
//  `assemble_rust_facade_checked`, what the CLI runs —
//  FIRST re-runs the crowning surface in-process
//  (`package::verify_crowns`, shared with the npm
//  assembly: Tier-1 crowns re-measured by the seeded
//  pipeline, Tier-2 bodies re-judged executed-exact). A
//  moved crown fails the assembly by name and nothing
//  is written. The checked-in crate in `packages/rust/`
//  is this assembly's output; a pin holds it byte-equal
//  to a fresh assembly.
//
//  VERSIONING (R6): the crate version is the compiler
//  crate's version — `env!("CARGO_PKG_VERSION")`, the
//  manifest's own source — stamped into the generated
//  `Cargo.toml` by the assembly. On this tree that is
//  1.0.0; Phase 125's release bumps the compiler to
//  1.1.0 and re-runs both assemblies, so the release is
//  a stamp, not a rebuild. The Rust crate carries no
//  size budget (R8).
//
//  DRY-RUN ONLY (R5): nothing here publishes, logs in
//  or touches a credential. The registry publishes are
//  Phase 125's release acts, after the gate.
// ─────────────────────────────────────────────────────────────

use crate::synthesis::engine::{CandidateImplementation, Provenance, ScoreBreakdown};

use super::bindings::{self, BindingInput};
use super::package::PackageFile;
use super::{render_rust_function, CodegenError};

/// The crate's repo home, relative to the repo root.
pub const RUST_PACKAGE_DIR: &str = "packages/rust";

/// The crates.io package name (EMBED_SCOPE D8a).
pub const CRATE_NAME: &str = "morphic";

/// The crate README: the factual surface — what the
/// crate is, the export list with its tiers, the type
/// table's Rust column in brief, the lockstep, and the
/// NF-116-2 statement. (The timed quickstart is the
/// embedding guide's, Phase 124; it does not live here.)
/// `{version}` is stamped from the crate's version at
/// assembly time (R6).
const CRATE_README: &str = r#"# morphic

Morphic's standard library as ready-to-call Rust
functions. Every function in this crate was generated
from a Morphic specification by the Morphic compiler's
native pipeline — the pipeline that judged it — and
ships with the standing that pipeline produced,
recorded per function in `morphic.manifest.json`
(name, frozen signature, tier, verdict basis, battery
counts, and the function's own documentation,
verbatim) and in each function's rustdoc.

This crate is an artifact, not a compiler: it contains
no synthesizer, no verifier and no dependencies, makes
no runtime verification claims, and builds offline.
The compiler that produced it is a separate tool and
is not published.

## Usage

```toml
[dependencies]
morphic = "{version}"
```

```rust
use morphic::stdlib::int_clamp;

assert_eq!(int_clamp(15, 0, 10), 10);
```

## The functions

Twenty-nine functions under `morphic::stdlib`, under
their Morphic entry names.

Tier 1 — Z3-verified against the function's
specification, and golden-exact on its frozen battery:

- Int: `int_abs`, `int_sign`, `int_min`, `int_max`,
  `int_clamp`, `int_is_even`, `int_is_odd`
- Char: `char_is_digit`, `char_is_alpha`,
  `char_is_whitespace`, `char_is_uppercase`,
  `char_is_lowercase`, `char_to_digit`
- String: `string_is_empty`
- Result / Option: `result_unwrap_or`,
  `option_unwrap_or`, `option_ok_or`
- List: `list_is_empty`

Tier 2 — executed-exact on the full frozen battery,
the tier printed here and in the manifest (the tier
is a property of the proof standard, not of the
behavior; each function is held to 100% agreement on
every battery point):

- List: `list_reverse`, `list_contains`, `list_sum`,
  `list_product`, `list_min`, `list_max`, `list_take`,
  `list_drop`, `list_is_sorted`, `list_sort`,
  `list_binary_search`

`list_binary_search` is a first-match scan: its
contract is search semantics — an index naming the
target, or `None` — and that disclosure travels with
the function (see its manifest row). With duplicate
targets, any valid index satisfies the contract.

The builtins `len` and `int_to_str` are not exports:
they are language builtins, rendered inside generated
code. A host uses its native equivalent — for `len`,
a slice's `.len()` or a string's code-point count
(`s.chars().count()`, never `.len()` where a Morphic
length is meant); for `int_to_str`, `x.to_string()`.

## Types at the boundary

The Rust mapping is exact: Morphic `Int` is `i64`,
`Char` is `char`, `String` is `String`, `List<T>` is
`Vec<T>`, `Option<T>` is `Option<T>`, `Result<T, E>`
is `Result<T, E>` — nesting included, no losses.

## Preconditions

Functions whose specifications freeze a `require:`
row carry it as a genuine `debug_assert!` of the
rendered condition — `int_abs` (the Int minimum is
excluded), `int_clamp` (`lo <= hi`) and
`list_binary_search` (the list is sorted). The
asserts are debug-only: an out-of-domain call panics
on the assert in a debug build, and in a release
build the behaviour is whatever the body computes —
out-of-domain inputs are contract violations, outside
the battery-exactness promise.

The asserts are the compiler pipeline's own
rendering (NF-116-2, closed globally in Phase 124:
the emitter renders parsed `require:` rows
genuinely, in the native emitter and its self-host
mirror alike).

## Versioning

This crate versions in lockstep with Morphic itself:
the crate version is the Morphic version whose
pipeline generated and verified the contents. The
manifest records it as both `packageVersion` and
`morphicVersion`; the two are equal at any release.

## Provenance

Generated — do not edit. The crate is assembled by
`morphic build --facade` from the standard library's
shipped bodies; assembly re-runs the crowning checks
and refuses to write if a crown has moved.

## License

Dual-licensed under MIT OR Apache-2.0, as Morphic
itself: see LICENSE-MIT and LICENSE-APACHE.
"#;

/// The crate's `lib.rs`: the crate docs (the crate's
/// own documentation surface — tiers, the exact Rust
/// type column, the NF-116-2 statement, the lockstep),
/// the stdlib module, and the manifest as an addressable
/// constant (the npm package's exports map names its
/// manifest the same way, §2.4).
const CRATE_LIB_RS: &str = r#"//! Morphic's standard library as ready-to-call Rust
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
//! **Preconditions (NF-116-2, closed globally in
//! Phase 124).** A function whose specification
//! freezes a `require:` row carries it as a genuine
//! `debug_assert!` of the rendered condition —
//! `int_abs` (the Int minimum is excluded),
//! `int_clamp` (`lo <= hi`), `list_binary_search`
//! (the list is sorted; the assert clones the list,
//! since a proposition must not consume the
//! argument). The asserts are debug-only: an
//! out-of-domain call panics on the assert in a
//! debug build, and in a release build the
//! behaviour is whatever the body computes —
//! out-of-domain inputs are contract violations,
//! outside the battery-exactness promise. The
//! asserts are the compiler pipeline's own
//! rendering: the emitter renders parsed `require:`
//! rows genuinely, in the native emitter and its
//! self-host mirror alike (the one named fallback
//! is a quantified row, which has no value-level
//! rendering; no function here carries one).
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
"#;

/// The generated stdlib module's head: its module doc
/// plus the scoped lint allowances for generated code
/// (measured at Phase 123 — see this file's header).
const STDLIB_MODULE_HEAD: &str = r#"//! The 29 frozen standard-library entries as real
//! Rust functions, generated from their shipped
//! bodies by the Morphic pipeline's Rust renderer —
//! do not edit (regenerate with
//! `morphic build --facade`; assembly re-runs the
//! crowning checks and refuses on a moved crown).
//!
//! Each function carries its tier and verdict basis
//! in its rustdoc; the same facts are in the crate's
//! `morphic.manifest.json`. Functions whose
//! specifications freeze a `require:` row carry it
//! as a genuine `debug_assert!` (NF-116-2, closed
//! globally in Phase 124 — see the crate docs).

// Generated code: the emitter's uniform rendering
// style (full parenthesization of expressions,
// exhaustive match arms, accumulator bindings)
// trips these stylistic lints by construction —
// measured at Phase 123 as 51 warnings over the 29
// functions (unused_parens ×40,
// unreachable_patterns ×10, unused_mut ×1). The
// allowances are scoped to this generated module so
// the functions stay byte-identical to the
// pipeline's rendering while the crate builds
// warning-free (gate clause 3).
#![allow(unused_parens, unreachable_patterns, unused_mut)]
"#;

/// The generated `Cargo.toml`: the publishable crate
/// manifest. The version is stamped by the assembly
/// from the compiler crate's version (R6); the
/// `[workspace]` table makes the crate its own
/// workspace root (see this file's header for the
/// naming resolution).
fn crate_cargo_toml(version: &str) -> String {
    format!(
        r#"# Generated by `morphic build --facade` — do not edit.
# The slim facade crate (EMBED_SCOPE D7a): the verified
# standard library as real Rust functions. Its own
# workspace root (the [workspace] table below), so it
# builds standalone, outside the compiler's workspace;
# the compiler crate of the same name is the repo's own
# tool and is never published.
[package]
name = "{CRATE_NAME}"
version = "{version}"
edition = "2021"
description = "Morphic's verified standard library as ready-to-call Rust functions: pre-generated from the specifications the Morphic pipeline verified (Tier-1) or proved executed-exact (Tier-2). Per-function tiers and batteries: morphic.manifest.json."
license = "MIT OR Apache-2.0"
repository = "https://github.com/thvtzy/morphic"
homepage = "https://thvtzy.github.io/morphic/"
keywords = ["morphic", "verified", "formal-verification", "standard-library"]

[workspace]
"#
    )
}

/// Escape a doc line for rustdoc carriage: a `[...]`
/// span whose contents carry no whitespace is parsed
/// by rustdoc as a shortcut intra-doc link (measured
/// at this phase: exactly one warning over the frozen
/// docs — the `i` of `list_binary_search`'s `xs[i]`).
/// Escaping that bracket renders the same text with
/// no link. Spans with whitespace are not link-parsed
/// and stand verbatim, as does everything else. (The
/// manifest carries the doc verbatim regardless —
/// §2.4 binds the manifest field, and it is filled
/// from the spec by Phase 121's generator, untouched
/// by this carriage rule.)
fn escape_rustdoc_line(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            let close = (i + 1..chars.len()).find(|&j| chars[j] == ']');
            if let Some(j) = close {
                let inner: String = chars[i + 1..j].iter().collect();
                if !inner.is_empty() && !inner.chars().any(char::is_whitespace) {
                    out.push_str("\\[");
                    i += 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// The candidate the renderer consumes for one input —
/// the Phase 116 helper construction (the entry is its
/// own function; no library helpers are appended in
/// the facade because every entry is present).
fn candidate_of(input: &BindingInput) -> CandidateImplementation {
    CandidateImplementation {
        id: 0,
        body: input.body.clone(),
        spec_name: input.spec.name.clone(),
        spec: input.spec.clone(),
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
        verified: input.verdict_basis == "z3-verified",
        verification_status: None,
    }
}

/// One facade function: the pipeline's own rendering
/// of the shipped body (`render_rust_function`), its
/// file-head doc block re-carried as the function's
/// rustdoc with the §2.4 prose (tier, verdict basis,
/// battery). The precondition block stands as the
/// pipeline renders it — genuinely, since Phase 124
/// closed NF-116-2 in the global renderer (the
/// facade-local substitution of Phase 123 is
/// retired; the global path produces the same three
/// asserts byte-for-byte).
fn facade_function(input: &BindingInput) -> String {
    let rendered = render_rust_function(&candidate_of(input));

    // Split the file-head doc block (`//!` lines + the
    // blank line) off the function text proper.
    let mut doc_lines: Vec<String> = Vec::new();
    let mut rest = rendered.as_str();
    while let Some(line) = rest.lines().next() {
        if let Some(text) = line.strip_prefix("//!") {
            doc_lines.push(text.strip_prefix(' ').unwrap_or(text).to_string());
            rest = &rest[line.len()..];
            rest = rest.strip_prefix('\n').unwrap_or(rest);
        } else {
            break;
        }
    }
    let function_text = rest.strip_prefix('\n').unwrap_or(rest).to_string();

    // The rustdoc: the entry's doc verbatim (with the
    // rustdoc link escape), then the §2.4 prose —
    // tier, verdict basis, battery — mirroring Phase
    // 121's JSDoc carriage.
    let mut out = String::new();
    if !doc_lines.is_empty() {
        for line in &doc_lines {
            out.push_str(&format!("/// {}\n", escape_rustdoc_line(line)));
        }
        out.push_str("///\n");
    }
    out.push_str(&format!("/// Tier: {}\n", input.tier));
    out.push_str(&format!("/// Verdict basis: {}\n", input.verdict_basis));
    out.push_str(&format!(
        "/// Battery: {} pinned, {} spec-silent\n",
        input.battery_pinned, input.battery_silent
    ));
    out.push_str(&function_text);
    out
}

/// The generated `src/stdlib.rs`: the module head plus
/// every frozen function, in §2.1's order (the input
/// order — `bindings::stdlib_inputs` is frozen in it).
fn stdlib_module(inputs: &[BindingInput]) -> String {
    let mut out = String::from(STDLIB_MODULE_HEAD);
    out.push('\n');
    for input in inputs {
        out.push_str(&facade_function(input));
        out.push('\n');
    }
    out
}

/// Assemble the Rust facade crate: the full file set,
/// in a stable order, generated from the shipped
/// bodies. Pure generation — no crowning
/// re-verification (that is
/// `assemble_rust_facade_checked`, the CLI's route)
/// and no filesystem contact.
pub fn assemble_rust_facade() -> Result<Vec<PackageFile>, CodegenError> {
    let inputs = bindings::stdlib_inputs();
    // The manifest, from Phase 121's generator with
    // this package's name — the same one-source
    // machinery, so the crate's manifest rows are the
    // npm package's rows with only the `package` field
    // differing (pinned).
    let set = bindings::generate_bindings(
        &inputs,
        CRATE_NAME,
        env!("CARGO_PKG_VERSION"),
        bindings::STDLIB_SCOPE_NAME,
    )?;
    let version = env!("CARGO_PKG_VERSION");
    let mut files: Vec<PackageFile> = Vec::new();
    let mut push = |path: &str, contents: String| {
        files.push(PackageFile {
            path: path.to_string(),
            contents,
        })
    };
    push("Cargo.toml", crate_cargo_toml(version));
    push(
        "README.md",
        CRATE_README.replace("{version}", version),
    );
    push("LICENSE-MIT", include_str!("../../LICENSE-MIT").to_string());
    push(
        "LICENSE-APACHE",
        include_str!("../../LICENSE-APACHE").to_string(),
    );
    push("morphic.manifest.json", set.manifest_json.clone());
    push("src/lib.rs", CRATE_LIB_RS.to_string());
    push("src/stdlib.rs", stdlib_module(&inputs));
    Ok(files)
}

/// The checked assembly — the CLI's route: crowns
/// re-verified first (the crowning surface shared
/// with the npm assembly), then the file set
/// generated. A moved crown fails here and no crate
/// is produced.
#[cfg(not(feature = "wasm-playground"))]
pub fn assemble_rust_facade_checked() -> Result<Vec<PackageFile>, CodegenError> {
    super::package::verify_crowns()?;
    assemble_rust_facade()
}
