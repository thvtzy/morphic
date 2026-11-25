# morphic-component-callbacks — the higher-order five at the Component Model boundary

The five higher-order entries of Morphic's verified
standard library as a **WASM component**: `list-map`,
`list-filter`, `list-fold`, `list-all`, `list-any`. The
artifact of record is
[`morphic-callbacks.wasm`](morphic-callbacks.wasm) in
this directory (81,840 B at Phase 160, inside its
frozen budget of 90,112 B — DISEBAR_16_SCOPE §6),
reproducible byte-for-byte from the committed lockfile:

```sh
cargo build --target wasm32-wasip2 --release
# → target/wasm32-wasip2/release/morphic_component_callbacks.wasm
```

The pipeline is plain cargo over a wit-bindgen crate
that wraps the facade ([`packages/rust`](../rust)) —
one glue line per export, no function re-implemented.
No cargo-component is involved.

## Why a separate artifact

This component is a **sibling** of the library
component ([`packages/component`](../component)), not
part of it, and the reason is structural: **imports
satisfy at instantiation**. A host that cannot supply
the callbacks must still be able to instantiate the
library component — and a host that only wants the
five must not have to satisfy the library's surface
either. Folding the five into the library component
would lock out every consumer that only wants
`int-clamp`. The home records the freeze's open
choice (§5): the crate stands at
`packages/component-callbacks/`, its own workspace
root beside `packages/component/`, so the compiler
tree's `Cargo.toml`/`Cargo.lock` never move.

## The world

```wit
package morphic:library@<lockstep>;

world library-callbacks {
  import map-apply: func(x: s64) -> s64;
  import pred: func(x: s64) -> bool;
  import combine: func(acc: s64, x: s64) -> s64;

  export list-map: func(xs: list<s64>) -> list<s64>;
  export list-filter: func(xs: list<s64>) -> list<s64>;
  export list-fold: func(xs: list<s64>, init: s64) -> s64;
  export list-all: func(xs: list<s64>) -> bool;
  export list-any: func(xs: list<s64>) -> bool;
}
```

The full WIT is [`wit/world.wit`](wit/world.wit); the
artifact's own printing (`wasm-tools component wit
morphic-callbacks.wasm`) agrees with it. The package
version tracks the project lockstep.

WIT has no function-typed values, so **no exported
signature anywhere carries a function-typed
parameter**. Each entry's function argument crosses
as one of the three host imports instead — the import
IS the callback:

| Import | Serves | The facade argument it supplies |
|---|---|---|
| `map-apply: func(x: s64) -> s64` | `list-map` | `f: fn(Int) -> Int` |
| `pred: func(x: s64) -> bool` | `list-filter`, `list-all`, `list-any` | `p: fn(Int) -> Bool` |
| `combine: func(acc: s64, x: s64) -> s64` | `list-fold` | `f: fn(Int, Int) -> Int` |

`combine`'s argument order — accumulator first, then
the element — mirrors the frozen LEFT fold:
`list-fold(xs, init)` computes
`combine(combine(combine(init, x0), x1), ...)`.

The imports' element type is `s64` (Morphic `Int`) —
that is the frozen surface; the five entries are
`List<Int>` entries and no other element type crosses.

Like every wasip2 build, the component also imports
std's WASI p2 surface (`wasi:io/*`, `wasi:cli/*`,
`wasi:clocks/*` @0.2.12) whether or not the code does
I/O — the §4.4 wart of the library component, carried
here too and named the same way; both runners below
satisfy it silently.

## The host contract

A host must supply the three imports at instantiation
— all three, even if it only calls one export (that
is what "imports satisfy at instantiation" means).
With jco (the rehearsal's instrument, and the
recommended JS route):

```sh
npm install @bytecodealliance/jco@1.42.0
npx jco transpile morphic-callbacks.wasm -o out \
  --map 'map-apply=./map-apply.js' \
  --map 'pred=./pred.js' \
  --map 'combine=./combine.js'
```

where each mapped module default-exports the function:

```js
// map-apply.js
export default function mapApply(x) { return x + 10n; }
// pred.js
export default function pred(x) { return x > 1n; }
// combine.js
export default function combine(acc, x) { return acc + x; }
```

```js
import { listMap, listFold, listFilter } from "./out/morphic-callbacks.js";
listMap([1n, 2n, 3n]);        // [11n, 12n, 13n]
listFold([1n, 2n, 3n], 100n); // 106n
listFilter([1n, 2n, 3n]);     // [2n, 3n]
```

(s64 crosses JS as BigInt; `list<s64>` comes back as
a `BigInt64Array`.)

The wasmtime CLI **cannot** run this component — it
supplies WASI and nothing else, so instantiation stops
at the first import, by design:

```
component imports function `map-apply`, but a matching
implementation was not found in the linker
```

That refusal is the two-component split made visible:
it is structural, not packaging.

## Tiers — what the component certifies, and what it does not

The five entries are **Tier-1** (`z3-verified`,
executed-exact on their full batteries) in the
manifest (`packages/rust/morphic.manifest.json`,
mirrored in the npm package); their evidence is the
PCC bundles at `packages/bundles/`. The WIT carries
types, not tiers — the boundary's honesty surface, as
with the library component.

One boundary is new here and is stated plainly: the
five are Tier-1 entries **whose function argument
lives with the host**. The component certifies each
entry's own logic — `list-map` applies its callback
to every element in order, `list-fold` folds left
from `init`, `list-all` of the empty list is `true`,
`list-any` of the empty list is `false` — under the
frozen contracts. **The callback's correctness is the
host's**: if the supplied `pred` is not the predicate
the caller means, the answers are exactly as wrong as
the callback is, and no tier says otherwise.
