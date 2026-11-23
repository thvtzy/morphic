# @thvtzy/morphic

Morphic's standard library as ready-to-import functions
for JavaScript and TypeScript hosts. Every function in
this package was generated from a Morphic specification
by the Morphic compiler's native pipeline — the pipeline
that judged it — and ships with the standing that
pipeline produced, recorded per function in
`morphic.manifest.json` (name, frozen signature, tier,
verdict basis, battery counts, and the function's own
documentation, verbatim).

This package is an artifact, not a compiler: it contains
no synthesizer and makes no runtime verification claims.
The functions are plain, environment-neutral ES modules
(BigInt, ES2020, is the only versioned requirement).

## The functions

Twenty-nine exports under their Morphic entry names.
The root module exports all of them; each is also
importable from its own subpath (for example
`@thvtzy/morphic/int_clamp`).

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

Tier 2 — executed-exact on the full frozen battery, the
tier printed here and in the manifest (the tier is a
property of the proof standard, not of the behavior;
each function is held to 100% agreement on every
battery point):

- List: `list_reverse`, `list_contains`, `list_sum`,
  `list_product`, `list_min`, `list_max`, `list_take`,
  `list_drop`, `list_is_sorted`, `list_sort`,
  `list_binary_search`

`list_binary_search` is a first-match scan: its contract
is search semantics — an index naming the target, or
`null` — and that disclosure travels with the function
(see its manifest row). With duplicate targets, any
valid index satisfies the contract.

The builtins `len` and `int_to_str` are not exports:
they are language builtins, rendered inside generated
code. A host uses its native equivalent — for `len`, an
array's element count or a string's code-point count;
for `int_to_str`, `String(x)`.

## Types at the boundary

- Morphic `Int` is TypeScript `bigint` — exact inside
  the i64 domain [-2^63, 2^63 - 1]. The functions carry
  no runtime precondition checks: a host value outside
  that domain is out-of-domain input, outside the
  battery-exactness promise. BigInt `/` truncates
  toward zero and `%` takes the dividend's sign,
  matching Morphic Int semantics.
- `Char` is a `string` of exactly one Unicode code
  point (an astral character has `.length` 2; the
  contract is the code point, not the length).
- Morphic counts string length in code points; JS
  `.length` counts UTF-16 code units. Never use
  `.length` where a Morphic length is meant.
- `Option<T>` is `T | null`. A nested
  `Option<Option<T>>` would flatten under this form;
  no export carries a nested-Option signature.
- `Result<T, E>` is the discriminated union
  `{ tag: "Ok", value: T } | { tag: "Err", error: E }`.

Preconditions frozen in the specifications (for
example: `int_abs` excludes the Int minimum;
`int_clamp` requires `lo <= hi`; `list_binary_search`
requires a sorted list) are stated in each function's
JSDoc and stand as contract.

## Versioning

This package versions in lockstep with Morphic itself:
the package version is the Morphic version whose
pipeline generated and verified the contents. The
manifest records it as both `packageVersion` and
`morphicVersion`; the two are equal at any release.

## Provenance

Generated — do not edit. The package is assembled by
`morphic build --package` from the standard library's
shipped bodies; assembly re-runs the crowning checks
and refuses to write if a crown has moved.

## License

Dual-licensed under MIT OR Apache-2.0, as Morphic
itself: see LICENSE-MIT and LICENSE-APACHE.
