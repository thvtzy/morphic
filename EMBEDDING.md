# Embedding Morphic — the 1.1 guide

Morphic 1.1 ships its verified standard library as two
ready-to-import packages:

| Package | Host | Contents |
|---|---|---|
| `@thvtzy/morphic` (npm) | JavaScript / TypeScript | 35 functions as ES modules, with generated TypeScript typings |
| `morphic` (crates.io) | Rust | The same 35 functions as real Rust code under `morphic::stdlib` |

Every function was generated from a Morphic specification
by the compiler's native pipeline — the pipeline that
judged it — and ships with the standing that pipeline
produced, recorded per function in each package's
`morphic.manifest.json` and in the function's own
documentation (JSDoc in the typings, rustdoc in the
crate).

The packages are artifacts, not compilers. They contain
no synthesizer and no verifier, and they make no runtime
verification claims: the verdicts were produced natively,
at package-build time, where Z3 lives — and package
assembly re-runs the crowning checks and refuses to
write if a crown has moved. What you import is plain
generated code that carries its proof record with it.

---

## Quickstart — JavaScript / TypeScript (npm)

You need Node with BigInt support (ES2020). To run the
TypeScript file below directly, Node ≥ 23.6 (built-in
type stripping); on older Node, compile it with `tsc`
first — the typings are the same either way.

```bash
mkdir morphic-hello && cd morphic-hello
npm init -y
npm pkg set type=module
npm install @thvtzy/morphic
```

Write `main.ts`:

```ts
import { int_clamp, list_min, option_ok_or } from "@thvtzy/morphic";

// A Tier-1 (Z3-verified) function, called like any function.
// Morphic Int is a TypeScript bigint — note the n suffixes.
const clamped: bigint = int_clamp(15n, 0n, 10n);
console.log(clamped); // 10n

// A Tier-1 function returning an Option: T | null.
const smallest: bigint | null = list_min([3n, 1n, 2n]);
console.log(smallest); // 1n
console.log(list_min([])); // null — the empty case is a value

// Option in, Result out: a discriminated union (see below).
const r = option_ok_or(smallest, "empty");
switch (r.tag) {
  case "Ok":
    console.log("value:", r.value); // value: 1n
    break;
  case "Err":
    console.log("error:", r.error);
    break;
}
```

Run it:

```bash
node main.ts
```

That is the whole path: install → import → typed calls,
one Tier-1 function, one Option-returning function, one
Result-returning function. (This quickstart is executed
as a timed test in the project's records; the measured
run is in the Phase 125 note in
[ROAD_TO_SELF_HOSTING.md](ROAD_TO_SELF_HOSTING.md).)

## Quickstart — Rust

Add the crate:

```toml
[dependencies]
morphic = "1.1"
```

Write `src/main.rs`:

```rust
use morphic::stdlib::{int_clamp, list_min, option_ok_or};

fn main() {
    // A Tier-1 (Z3-verified) function. Morphic Int is i64 — exact.
    let clamped: i64 = int_clamp(15, 0, 10);
    println!("{clamped}"); // 10

    // A Tier-1 function returning an Option.
    let smallest: Option<i64> = list_min(vec![3, 1, 2]);
    println!("{smallest:?}"); // Some(1)
    println!("{:?}", list_min(vec![])); // None — a value, not an error

    // Option in, Result out.
    let r: Result<i64, String> = option_ok_or(smallest, "empty".to_string());
    match r {
        Ok(v) => println!("value: {v}"),
        Err(e) => println!("error: {e}"),
    }
}
```

```bash
cargo run
```

The crate has no dependencies, no build script and no
downloads: it builds offline, and it is not the compiler
(the compiler crate is a separate tool and is not
published).

---

## Import shapes

**npm.** The root module exports all 35 functions:

```ts
import { int_clamp, list_sort, char_to_digit } from "@thvtzy/morphic";
```

Each function is also importable from its own subpath —
the subpath serves exactly the root export, so the two
forms are interchangeable:

```ts
import { int_clamp } from "@thvtzy/morphic/int_clamp";
```

The package is ESM-only (`"type": "module"`); no
CommonJS form is shipped in 1.1. The manifest is
importable as data at `@thvtzy/morphic/morphic.manifest.json`.

**Rust.** All 35 functions live in one module:

```rust
use morphic::stdlib::*;
use morphic::stdlib::{int_clamp, list_min};
```

The manifest ships inside the crate package and is also
addressable in code as `morphic::MANIFEST_JSON`.

Export names are the Morphic entry names, unchanged, in
both packages.

## The 1.2 higher-order exports

Six exports are new in 1.2 (STDLIB_SCOPE.md
Amendment 2); all six stand at Tier-1 since the
1.3 re-tier:

| Export | Morphic signature | What it does |
|---|---|---|
| `list_map` | `(List<Int>, fn(Int) -> Int) → List<Int>` | `f` applied to each element, in order |
| `list_filter` | `(List<Int>, fn(Int) -> Bool) → List<Int>` | The elements for which `p` holds, in original order |
| `list_fold` | `(List<Int>, Int, fn(Int, Int) -> Int) → Int` | The **left** fold over the list, starting from the accumulator |
| `list_all` | `(List<Int>, fn(Int) -> Bool) → Bool` | True iff `p` holds of every element (empty → true) |
| `list_any` | `(List<Int>, fn(Int) -> Bool) → Bool` | True iff `p` holds of at least one element (empty → false) |
| `range` | `(Int, Int) → List<Int>` | The ascending list `[lo, lo + 1, …, hi − 1]`; `lo >= hi` → empty |

The five higher-order exports take a function argument,
and they answer at runtime in both packages:

```ts
import { list_map, int_abs } from "@thvtzy/morphic";

list_map([3n, -4n], (x) => x + 1n); // [4n, -3n] — a host lambda
list_map([3n, -4n], int_abs);       // [3n, 4n] — the package's own export
```

```rust
use morphic::stdlib::{int_abs, list_map};

assert_eq!(list_map(vec![3, -4], |x| x + 1), vec![4, -3]); // a closure
assert_eq!(list_map(vec![3, -4], int_abs), vec![3, 4]);     // a function pointer
```

**The TypeScript boundary, stated where it binds:** in
the npm package's TypeScript declarations, a
function-typed parameter is typed `never` — no callable
TypeScript function type is frozen in 1.2
(Amendment 2 §A2.4.2's recorded interpretation), so a
typed TypeScript host cannot name a callback type for
these parameters even though the runtime answers any
JavaScript function of the right shape. The manifest's
`signature` field carries the true Morphic signature
(the table above), so the boundary is stated in the
package's own surface, not hidden by it. The Rust
facade has no such boundary: its function parameters
are fully typed `fn` pointers, as the snippet shows.

## Reading Option and Result results

At the TypeScript boundary (decision D4a of the frozen
scope):

- `Option<T>` is `T | null`. `None` is `null` — distinct
  from every payload and from `undefined`. A function
  declared `bigint | null` returns a `bigint`, or `null`
  for the empty case; there is no wrapper to unwrap.
- `Result<T, E>` is the discriminated union
  `{ tag: "Ok", value: T } | { tag: "Err", error: E }`.
  Switch on `tag`; TypeScript narrows each branch, and
  the switch is exhaustive — the form the quickstart
  uses.

At the Rust boundary both are the native `Option<T>` /
`Result<T, E>`, nesting included.

## The type table — with its losses named

Every type of the 1.0 surface, as it crosses the host
boundary. A mapping is either exact, or its loss is
named here with its boundary. (The same decisions are
recorded in WIT terms in the frozen scope, so they
transfer to the Component Model step unchanged.)

| Morphic | TypeScript | Rust | Losses and boundaries |
|---|---|---|---|
| `Int` | `bigint` | `i64` | **TS: exact inside the i64 domain.** JS BigInt is unbounded, so the representation itself is lossless; the *domain* is the boundary — a host value outside [−2^63, 2^63−1] is an out-of-domain input, not a mapping loss. BigInt `/` truncates toward zero and `%` takes the dividend's sign, matching Morphic Int semantics. Friction, recorded not hidden: `15n` literals; `JSON.stringify` throws on bigint. Rust: exact. |
| `Bool` | `boolean` | `bool` | Exact. |
| `Char` | `string` — exactly one code point | `char` | **TS boundary:** JS strings are UTF-16, so a Char is a string of exactly one Unicode code point (an astral Char has `.length` 2). The binding trusts this contract — no runtime policing in 1.1; a string that is not exactly one code point is out-of-domain. Rust: exact. |
| `String` | `string` | `String` | Values exact on every side. **Length semantics differ and are named:** Morphic counts code points; JS `.length` counts UTF-16 code units (see the length rule below). A string containing lone surrogates is out-of-domain. Rust: exact as values. |
| `List<T>` | `T[]` | `Vec<T>` | Structurally exact; element losses inherit recursively from T's row (a `List<Int>` is a `bigint[]`). |
| `Option<T>` | `T \| null` | `Option<T>` | TS: `None` is `null`, distinct from every payload and from `undefined`. **Named composition boundary:** `Option<Option<T>>` flattens under `T \| null` — `Some(None)` and `None` are indistinguishable. No export in this package carries a nested-Option signature, so the shipped surface is exact; the boundary binds user specs with nested Options. Rust: exact, nesting included. |
| `Result<T, E>` | `{ tag: "Ok", value: T } \| { tag: "Err", error: E }` | `Result<T, E>` | Exact as a discriminated union; nests without collision — the tags distinguish every level. Rust: exact. |
| Record (named) | object type with the field types | generated struct | Field types inherit the rows above; field order is not semantic. No record-typed export is in the 35; this row binds generated user modules. |
| `Unit` | `undefined` | `()` | The standing rendering. User-module specs only. |
| Function types | `never` in the declaration; any JS function at runtime | `fn(A) -> R` | **The 1.2 boundary (Amendment 2 §A2.4.2):** six exports are higher-order since 1.2 and answer at runtime (see "The 1.2 higher-order exports" above). The TypeScript declaration types a function-typed parameter `never` — no callable TypeScript function type is frozen in 1.2 — while the manifest's `signature` field carries the true Morphic signature. Rust is exact: the parameter is a fully typed `fn` pointer. |

**The length rule.** There is no `len` export (below),
so when a host needs a length: for a List value, the
host array's element count is the number `len` returns;
for a String, the length is a **code-point count** —
`[...s].length` in JavaScript, `s.chars().count()` in
Rust — **never `.length`**, which counts UTF-16 code
units (JavaScript) or bytes (Rust). Generated code that
needs a length internally already implements exactly
this.

## What the tiers mean at the boundary

Each function carries one of two standings, produced by
the native pipeline before the package existed:

- **Tier-1 — `z3-verified`.** The function's body was
  proven against its specification by Z3, and is
  golden-exact on its frozen battery. 33 of the 35
  exports are Tier-1.
- **Tier-2 — `executed-exact`.** The function's body is
  held to 100% agreement with its frozen battery at
  every point, executed — but no Verified claim is
  made. 2 of the 35 exports are Tier-2: `list_sort`
  and `list_binary_search`, each with a named residue
  in its manifest row (a contract form the encoder
  does not carry; a `require:` the crown route cannot
  assume). The tier is a property of the proof
  standard, not of the behaviour: a Tier-2 function is
  not "less correct" on any measured point, it is less
  *proven*. The List encoder landed at 1.3, and
  fifteen Tier-2 labels upgraded in place at that
  lockstep release, exactly as this paragraph always
  stated they would; the functions' bytes did not
  change.

The tier is a fact of the package surface, not only of
the project's docs. Read it in three places, all
generated from the same record:

- the function's JSDoc (TypeScript) or rustdoc (Rust);
- the package README's function list;
- `morphic.manifest.json` — one row per export with
  `name`, `signature`, `tier`, `verdictBasis`
  (`"z3-verified"` / `"executed-exact"`), `battery`
  (the frozen cross-validation battery as
  `{ pinned, specSilent }` point counts), and `doc`,
  the function's own documentation carried verbatim.

Entry-level disclosures travel by that `doc` field. The
standing example: `list_binary_search` is a first-match
scan — its contract is search semantics (an index
naming the target, or `null`/`None`), and with
duplicate targets any valid index satisfies it; the
scan is disclosed in its doc, its JSDoc/rustdoc and its
manifest row, not hidden behind the name.

The rows are claims, and since 1.4 they carry their
evidence. Each entry has a re-verification bundle in
the repository at `packages/bundles/<entry>.bundle.json`
— its spec, its body in the `morphic-ir/1` text
serialization, the claimed verdict and tier, the
executed battery's counts, and the crown route that
judged it. `morphic verify-bundle <file>` re-runs that
route from the bundle's (spec, body) alone and answers
whether the claim reproduces; `morphic verify
--library` re-verifies all 35 shipped crowns the same
way. A tampered bundle — a flipped verdict, a swapped
body, a wrong route, a stale format version — is
refused by name, never silently passed.

## Contracts: preconditions and out-of-domain inputs

The generated functions perform no runtime precondition
checks in the JavaScript package. A frozen
`require:` in a specification is stated in the
function's documentation and stands as contract:

- `int_abs` — the Int minimum (−2^63) is excluded.
- `int_clamp` — requires `lo <= hi`.
- `list_binary_search` — requires a sorted list.

The Rust crate renders those three rows as genuine
`debug_assert!`s: an out-of-domain call panics on the
assert in a debug build; in a release build the
behaviour is whatever the body computes.

Everything outside a function's stated domain is a
contract violation, not a runtime error: host BigInts
outside the i64 domain, strings that are not exactly
one code point where a Char is expected, lone-surrogate
strings, and inputs on which the arithmetic would
overflow (the batteries exclude overflowing
sum/product points by design — the language's Int
arithmetic is checked, so such points have no value to
agree on). Behaviour there is deterministic but outside
the battery-exactness promise.

## Not exports: `len` and `int_to_str`

The 1.0 builtins are language builtins, not library
entries, and neither package exports them. Inside
generated code they are rendered by the emitters; a
host that needs them uses the host-native equivalent
under the semantics above: `len` is an array's element
count or a string's code-point count (the length rule);
`int_to_str` is `String(x)` in JavaScript, `x.to_string()`
in Rust. The closed method table (`Option::is_some`,
`Result::unwrap`, …) is likewise spec-side surface:
hosts call the exported functions.

## Versioning

Both packages version in **lockstep** with Morphic
itself: the package version is the Morphic version
whose pipeline generated and verified the contents —
1.1.0 for the 1.1 release — published only from the
tagged release tree, under this repo's one changelog.
The manifest records the version twice, as
`packageVersion` and `morphicVersion`; the two are
equal at any release, so a host can always tell which
compiler verified its functions. There is no
independent package cadence and no pre-release build is
published under the frozen names.

## What is not in 1.1 — named in the open

- **No runtime-compiler package.** Morphic also exists
  as a compiler that runs in the browser (the
  playground). It is not shipped as an embedding
  package, for two measured reasons: the browser build
  has no Z3, so it can produce no Verified verdicts at
  runtime; and it cannot call the library it installs.
  A runtime package could therefore never honestly be
  sold as "verified functions" — the verified artifact
  is the pre-generated package above, whose verdicts
  were produced natively. The deferral is a decision of
  the frozen scope (D1a/D2a), not an omission.
- **The Component Model is the named next step.** A
  WASM Component Model package (a WIT world) is the
  post-1.1 distribution step on the roadmap. It is not
  in 1.1; the type decisions above are recorded in
  WIT-compatible terms precisely so that step inherits
  them unchanged. These packages are not presented as
  the end of the distribution story.
- **No CommonJS form** (ESM only, above), **no
  higher-order exports and no host callbacks** (they
  wait on G4), and **no `int_gcd`** — it stands amended
  out of the library and returns with the post-1.0
  quantified-mod campaign, so it is absent here as it
  is absent from the prelude.

## Provenance

Generated — do not edit the packages. The npm package
is assembled by `morphic build --package` and the
crate by `morphic build --facade`, each from the
standard library's shipped bodies; assembly re-runs the
crowning checks before writing and refuses, naming the
entry, if a crown has moved. The frozen scope these
packages answer to is [EMBED_SCOPE.md](EMBED_SCOPE.md);
the audit that priced them is
[EMBED_AUDIT.md](EMBED_AUDIT.md).
