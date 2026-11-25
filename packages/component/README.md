# morphic-component — the library at the Component Model boundary

Morphic's verified standard library as a **WASM
component**: the same functions the npm package and the
Rust facade ship, answering from a boundary no single
language owns. The artifact of record is
[`morphic-library.wasm`](morphic-library.wasm) in this
directory (96,100 B at the 1.6 freeze, inside its
frozen budget of 106,496 B), reproducible byte-for-byte
from the committed lockfile:

```sh
cargo build --target wasm32-wasip2 --release
# → target/wasm32-wasip2/release/morphic_component.wasm
```

The pipeline is plain cargo over a wit-bindgen crate
that wraps the facade ([`packages/rust`](../rust)) —
one glue line per export, no function re-implemented.
No cargo-component is involved (its latest release
ships no prebuilt; the pipeline provably does not need
it — DISEBAR_16_AUDIT §1.3).

## The world

```wit
package morphic:library@<lockstep>;

interface functions {
  int-abs: func(x: s64) -> s64;
  // … 30 exports: the library minus the higher-order five
}

world library {
  export functions;
}
```

The full WIT is [`wit/world.wit`](wit/world.wit); the
artifact's own printing (`wasm-tools component wit
morphic-library.wasm`) agrees with it. The package
version tracks the project lockstep. Types cross
exactly: Int is `s64`, Char is a Unicode scalar
`char`, Option/Result/List are the component model's
own — no mapping at this boundary loses anything, and
the JS boundary's code-point length rule (R10) has no
analogue here, because whole strings cross and no
host-visible length does.

## Calling it

With the wasmtime CLI (51.0.0):

```sh
wasmtime run --invoke 'int-clamp(15, 0, 10)' morphic-library.wasm
# 10
wasmtime run --invoke 'list-sort([3, 1, 2])' morphic-library.wasm
# [1, 2, 3]
wasmtime run --invoke 'option-ok-or(none, "e1")' morphic-library.wasm
# err("e1")
```

From JavaScript, transpile with
[@bytecodealliance/jco](https://github.com/bytecodealliance/jco)
and import the generated module — the exports are
ordinary functions over BigInt/arrays/strings. From
Rust, the `wasmtime` crate's component bindgen over
the same WIT instantiates it natively. Any
Component-Model host works; the component is not
tied to one.

## Tiers — read the manifest, not the WIT

The WIT carries **types, not tiers**. Of the 30
exports, 28 stand **Tier-1** (Z3-verified) and 2 stand
**Tier-2** (`list-sort`, `list-binary-search` —
executed-exact). The per-export tier rows live in the
manifest ([`packages/rust/morphic.manifest.json`](../rust/morphic.manifest.json),
mirrored in the npm package), and their evidence is
the proof-carrying bundles at
[`packages/bundles`](../bundles) — re-checkable with
`morphic verify-bundle`. The component deliberately
does not carry claims it cannot re-state.

## The WASI import surface (a named wart)

The component imports std's WASI p2 surface
(`wasi:io/*`, `wasi:cli/*`, `wasi:clocks/*` @0.2.12)
whether or not the code does I/O — fifteen interfaces
a host must satisfy at instantiation. wasmtime's
default WASI and jco's preview2-shim both satisfy it
silently; a bare host must stub interfaces it will
never call. Stripping the surface (a core-module +
`wasm-tools component new` variant) is a named
follow-up, not part of 1.6 (DISEBAR_16_SCOPE §7).

## The higher-order five live next door

`list-map`, `list-filter`, `list-fold`, `list-all`,
and `list-any` are **not** in this component: WIT has
no function-typed values, so they cannot cross as
exports. They ship in the sibling **callback
component** (Phase 160), which imports the three
callbacks it needs from the host — `map-apply`,
`pred`, `combine` — and exports the five over them.
Two artifacts, each instantiable by exactly the hosts
that can use it: folding the imports in here would
have locked out every consumer that only wants
`int-clamp`.
