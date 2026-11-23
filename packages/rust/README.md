# morphic

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

## Embedding guide

The [embedding guide](https://github.com/thvtzy/morphic/blob/master/EMBEDDING.md)
takes a host from install to a typed call: the
five-minute quickstart, the import shapes, the full
type table with its named losses, and what the tiers
mean at the boundary.

## Usage

```toml
[dependencies]
morphic = "1.1.0"
```

```rust
use morphic::stdlib::int_clamp;

assert_eq!(int_clamp(15, 0, 10), 10);
```

## The functions

Thirty-five functions under `morphic::stdlib`, under
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
  `list_binary_search`, `list_map`, `list_filter`,
  `list_fold`, `list_all`, `list_any`, `range`

The last six are the 1.2 amendment's entries
(STDLIB_SCOPE.md Amendment 2). Five of them are
higher-order: their function parameters are ordinary
Rust `fn` pointers, so a function pointer or a
non-capturing closure answers — the facade is fully
typed, with no boundary loss.

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
