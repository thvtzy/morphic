# EMBED_AUDIT.md — Phase 118: the 1.1 Embeddable scope audit

> Status: **AUDIT — measurement + design only** (the Phase 106/99
> lineage). No production code changed, no package published, no
> gate defined beyond the sketch in §8. The only code is the probe
> artifact `tests/phase118_probes.rs` (12 probes pinning today's
> measured truth; a future phase that closes a gap re-aims the
> corresponding pin at its site). Branch `docs/embed-118-audit`
> from master @ `7cd2920` (v1.0.0), unmerged — the branch awaits
> the owner's D1–D8 calls (§9.2), which a ratify phase then
> freezes into a scope + gate document, as Phase 108 did for 1.0.
>
> The milestone as ratified (Phase 105, owner decision
> 2026-11-22): **1.1 — Embeddable Morphic**: verified functions
> as ready-to-import packages (npm/WASM + Rust crate), typed
> host bindings, "5-minute" embedding docs/examples. Seeds named
> at ratification: the JS/WASM codegen targets and
> `crates/morphic-wasm/` from v0.6. On the north-star ladder
> (Phase 107) 1.1 is the first step of the **Disebar** rung —
> distribution — after 1.0 closed the **Dipakai** rung's library
> half.

## 0. Headline verdict

**1.1 is achievable as ratified — but its centre of gravity is
not where the ratification sketch assumed.** The sketch named
two seeds, the JS/WASM codegen targets and `crates/morphic-wasm/`.
Measured, neither seed is load-bearing in its current form:

- **The JS codegen target cannot yet emit a single shipped
  library body that runs.** Of the representative bodies graded
  in §4 (Tier-1, Tier-2, Match-shaped, builtin-only), **zero**
  produce JavaScript that both parses and returns the right
  answer: the simplest Tier-1 body in the library (`int_abs`)
  generates a function with **no `return` statement at all**
  (node-run: `int_abs(-5) === undefined`); a Tier-2 body
  (`list_sum`) generates code that **throws** (`ReferenceError:
  acc is not defined`); bodies using `%` or `<=` generate
  **unparseable** JS (`return (x ? 2);` — SyntaxError). This is
  NF-116-1, measured at the exact boundary 1.1 cares about.
  The 1.1 build is therefore, first, **a JavaScript emitter
  completion** (priced L, §9.1 Phase 120) — the packaging
  machinery around it is the smaller half.
- **`crates/morphic-wasm/` is a playground shell, not an
  embedding surface.** Its five exports are whole-source-in /
  JSON-string-out (`check(source: string): string` is the
  typed surface a TypeScript host sees); one of the five is a
  Phase 65 timing *probe* still riding in the public API; and
  the runtime-synthesis route it offers is capped by the v0.6
  D2 honesty rule — **no Z3 in the browser build means no
  Verified verdicts at runtime** (`verification_available:
  false`). Verified functions must be synthesized where Z3
  lives (natively) and shipped as artifacts. The wasm shell is
  a fine *playground*; as the 1.1 vehicle it demotes to an
  optional runtime extra (§2, route iii).
- **The route that carries the milestone is pre-generation:**
  synthesize + verify natively, generate the function,
  package the artifact (route i), with the 29 entries of the
  1.0 library as the first package's contents (route ii).
  That route exists today in exactly one target: **Rust**
  generation is real, typed, compilable code (Phase 116),
  and the sum of all 29 entries generated as standalone Rust
  files is **37,149 B** of source. For JS — the ratified
  npm route — everything between "generated text" and
  "importable typed module" is still to build: the emitter
  (§4), the typings (no `.d.ts` is generated from a spec
  anywhere; §3), the Option/Result representation (none
  exists; §3), and the package assembly (`morphic build`
  emits one function to one file; §1.4).

Two registry facts, verified live on 2026-11-23 (§5): the
unscoped npm name **`morphic` is taken** (an unrelated
package, latest 1.0.16); the crates.io name **`morphic` is
free** ("crate `morphic` does not exist").

Total price of the recommended shape (§9.1): **≈ L** —
seven phases, 119–125. Making the Component Model or the
runtime-compiler package part of the 1.1 gate raises it to
XL; both are priced as deferrals with the reasoning stated
(D1, D6).

## 1. The embedding surfaces as they stand

### 1.1 `crates/morphic-wasm/` — a playground shell

The crate (97 lines, `publish = false`, version 0.1.0) is a
thin wasm-bindgen shell over `morphic::api`. Its complete
export list:

| Export | Shape | Embedding value |
|---|---|---|
| `start` | wasm-bindgen start hook | none (page plumbing) |
| `init_stdlib(asset_bytes: Uint8Array): boolean` | installs the library asset; **throws a string** on malformed bytes or a **second install** | the asset half of the story (below) |
| `check(source): string` | parse + typecheck, JSON CheckReport out | reusable as-is by a tooling host |
| `synthesize_seeded(source, seed: number): string` | seeded synthesis of the first spec, JSON out | playground-shaped (below) |
| `synthesize_seeded_spec(source, spec_name, seed): string` | same, named spec | playground-shaped |
| `probe_timing(): string` | a Phase 65 S6 *measurement probe* (`Instant` availability) | none — a probe riding in a public surface |

Findings:

- **DTOs cross as JSON strings.** The generated
  `morphic_wasm.d.ts` types every export as returning
  `string`. A TypeScript host's "typed bindings" today are
  `JSON.parse` and hope. There is no export that calls a
  function, takes a typed value, or returns one.
- **The init story is three steps, hand-duplicated.** A host
  must (1) `await` the bindgen default init (async), (2)
  `fetch` the stdlib asset (33,476 B) itself — the package
  does not fetch or embed it, (3) call `init_stdlib` once,
  before any other call; a second install is an error, so
  test suites and hot-reload hosts that re-init break. The
  playground performs this sequence **twice, in two files**
  (`playground/src/main.js` and `playground/src/worker.js`) —
  no shared init helper is exported for hosts to reuse.
- **Synthesis is synchronous CPU work.** The playground runs
  it in a Web Worker for exactly that reason; a host calling
  `synthesize_seeded_spec` on its main thread blocks it.
  Nothing in the surface says so.
- **Sizes.** The post-wasm-opt bundle is **680,280 B** of
  WASM + 16,680 B of JS glue (+ 5,082 B of `.d.ts`), against
  the standing 771,072 B budget; the asset is a further
  33,476 B fetched separately. That is the price of shipping
  **the whole compiler** (parser, engine, interpreter,
  codegen) to a host — measured against route (i)'s
  alternative in §2.
- **The verdict cap (D2).** This build has no Z3 and no
  compiled runner, by v0.6 design: every winner it crowns is
  labelled Inconclusive and the report says
  `verification_available: false`. A host embedding this
  shell to "get verified functions" gets executed-test
  evidence only. That honesty is correct — and it decides
  the route question in §2.

Playground-specific vs reusable: `check` and the two
synthesize calls are reusable *machinery* wrapped in a
playground *shape* (whole module source in, report JSON
out, seeded profile only). `probe_timing` and the
fetch-it-yourself asset step are playground scaffolding.

### 1.2 The Rust API surface — the whole compiler, uncurated

`src/lib.rs` exposes seven public modules — `api`, `codegen`,
`spec`, `stdlib`, `synthesis`, `timeutil`, `verify` — i.e.
**the entire compiler is the public API**: AST types, engine
internals, IR, the verifier. There is no curated facade, no
re-export discipline, no stability note anywhere. The
host-shaped part is `morphic::api`:

- `check(source) -> CheckReport`
- `synthesize_seeded(source, seed) -> Result<SynthesisReport, StageError>`
- `synthesize_seeded_spec(source, spec_name, seed) -> …`
- `render(candidate, target) -> Result<String, String>`
- `playground_config() -> SynthesisConfig`

All DTOs are serde structs (JSON-ready). Two things a host
embedding *functions* needs are absent from `api` and exist
only as internals:

- **No call-a-function entry point.** `stdlib::shipped_body(name)
  -> Option<IRNode>` and `synthesis::eval::eval(node, env)`
  are public, and the audit's probe (§10) measures that they
  compose: `eval(shipped_body("int_abs"), {x: -5}) ==
  Int(5)`, `int_clamp` and the Tier-2 `list_sum` likewise.
  So a Rust host *can* call library entries today — at IR
  level, building a `HashMap<String, Value>` by parameter
  name, with no input type-checking, no arity checking, and
  `EvalError::NotEvaluable(String)` as the failure surface.
  It is a mechanism, not an API.
- **No generated-functions module.** The 29 entries do not
  exist as Rust `fn` items anywhere. They exist as `.morph`
  prelude source, as IR data, and as generated text when a
  *caller's* candidate is rendered (Phase 116 emits the used
  entries into the caller's file). A `morphic::stdlib`
  module of real functions is a generation step that has
  not been taken (§2, route ii).

**Publish-readiness of the crate itself** (`morphic` 1.0.0
in the root `Cargo.toml`): description, license and
repository are present; `readme`, `keywords`, `categories`,
`documentation` are absent. Three structural problems:

1. **The default feature set is a CLI's, not a library's.**
   `default = ["z3-support", "compiled-runner", "interactive",
   "llm-client"]` pulls `reqwest` (an HTTP client) and
   `indicatif` (TUI progress bars) into every downstream
   build that takes defaults.
2. **Z3 is a build-time download.** `z3-support` uses the
   `z3` crate's `gh-release` feature — building the crate
   fetches a native Z3 binary from GitHub at build time.
   Downstream builds inherit a network dependency, and
   docs.rs (which builds offline) cannot build the crate
   with default features at all.
3. **Semver exposure.** The CHANGELOG declares Semantic
   Versioning; publishing this crate at 1.0.0 puts the
   *whole exposed surface* — engine internals included —
   under that promise. 1.1 must decide what the crate's
   public contract is before the name is spent (D7).

### 1.3 The JS and WAT codegen targets as "importable modules"

`generate_javascript` emits a syntactically module-shaped
file — `function name(params) { … }` plus `export { name };`
— with **no types at all**: there is no `ty_to_javascript`
in the tree (the other targets each have a `ty_to_*`; JS
params are bare names, and the only header is the spec's
prose `doc` as a JSDoc comment). What the body contains is
graded in §4; the short form is that the module shape is
the only part of the JS story that exists.

The WAT target does export by name — `(export "int_abs"
(func $int_abs))` — and is the one target whose output is
a real WASM module today. But its type map truncates:
`ty_to_wasm` maps Morphic `Int` (and `I32`, `I64`) to WAT
**`i32`**, and the measured `int_abs` module is

```wat
(module
  (export "int_abs" (func $int_abs))
  (func $int_abs (param x i32)
    (result i32)
    ;; node
  )
)
```

— a signature that cannot represent an Int, wrapping a
body that is a comment (the scalar model's node coverage,
NF-116-1). The WAT target is a text *preview* (v0.6 D6
says exactly this); as an embedding vehicle it is not one.

### 1.4 The CLI's `build` — one function, one file

`morphic build <spec.morph> --target <t>` runs the full
pipeline over the file's **first spec** and writes **one
function** to one file. It is the natural seed of a
packager and currently none of one: no multi-function
module, no package manifest, no typings, no library
bundling, and its `--target` help names only
`rust, c, wasm` (the dispatch in fact accepts all six
targets). The Phase 105 sketch's "`morphic build` emits
ready-to-import packages" describes a tool that does not
exist yet; §9 prices building it (Phase 122).


## 2. What a host actually embeds — the three routes, priced

### Route (i): pre-generated functions

*Synthesize + verify at build time (natively, where Z3
lives); ship the generated code as the package.*

- **Machinery that exists:** the full pipeline; Rust
  generation correct-by-construction for library-involving
  bodies (Phase 116: compiles + runs, rustc-pinned).
- **Machinery missing:** the JS emitter (§4 — the L item),
  typings generation (§3), package assembly (§1.4).
- **Size:** source-scale, not compiler-scale. Measured
  bracket: all 29 entries generated as standalone Rust
  files sum to **37,149 B** of source (each file carries
  its own test module and transitive entry deps, so a
  shared module is smaller). A JS package of the same
  functions is the same order — tens of KB, against route
  (iii)'s 680,280 B of WASM before the asset.
- **Honesty:** the strongest of the three routes. The
  verdict a package claims was produced by the native
  pipeline at package-build time and can be re-run there
  (the crowning pins are exactly this discipline for the
  library itself). Nothing at the host's runtime pretends
  to verify.
- **Price:** the emitter completion (L) + bindings (M) +
  packaging (M), shared with route (ii) — §9.1 Phases
  120–122.

### Route (ii): the 1.0 library's 29 entries as ready-made imports

*Route (i) with the contents frozen to the shipped library.*

This is route (i)'s first package, not a separate build:
the generator that packages a user module packages the
prelude unchanged, and the 29 entries are the gate corpus
(batteries already pinned: Phase 111/112/115
cross-validation, 100% on every pinned point). Two
route-specific facts:

- **The tier at the boundary.** Today an entry's tier
  exists in exactly one place that survives generation:
  its `.morph` `doc:` string. The JS generator prints
  that doc as the file's JSDoc comment and the Rust
  generator as its doc head — so a generated `list_sum`
  literally carries *"Tier-2: executed-exact on the full
  battery; element-level verification awaits the
  post-1.0 list encoder"* as a comment (visible in §4's
  captures). That is a comment, not metadata: nothing a
  host's tooling can read, filter, or warn on. A typed
  binding layer (§3) should carry the tier into the
  `.d.ts` JSDoc **and** a machine-readable package
  manifest, so "this export is executed-exact, not
  Verified" is a fact of the package surface, not of the
  repo's docs. The gate sketch (§8, clause 4) makes this
  a gate condition.
- **What the entries cost a host:** nothing at runtime
  beyond the functions themselves — no asset, no init,
  no compiler. This is the only route whose embedding
  story is *smaller* than the playground's.

### Route (iii): the runtime compiler in a package

*Ship the compiler; the host synthesizes (and maybe calls)
at runtime.* Two sub-routes with different honesty:

- **(iii-wasm)** the morphic-wasm shell, §1.1: 680,280 B +
  33,476 B asset + the three-step init. Verdict cap: **no
  runtime verification exists in this build** (D2). What a
  host *can* honestly do with it today: `check` user specs
  in the browser, and run seeded synthesis for
  executed-test evidence. As a *functions* vehicle it
  cannot even call the library it installs — the asset
  feeds the prelude for check/synthesis; no export
  evaluates an entry.
- **(iii-native)** a Rust host taking the `morphic` crate:
  the full pipeline *with* Z3 (real Verified verdicts at
  runtime) plus the IR-level eval route measured in §1.2.
  This sub-route works today at mechanism level; its
  costs are the publish-readiness problems of §1.2 (the
  host's build downloads Z3 and compiles a compiler) and
  the absence of a designed call API.

**Route verdict:** (i)+(ii) carry the milestone; (iii) is
an optional extra whose wasm form must never be sold as
"verified functions" — D1/D2 put this to the owner.

## 3. Type mapping — Morphic → TypeScript and Morphic → Rust

Rust first, because it is nearly free: `ty_to_rust` is
**exact for the whole 1.0 surface** — Int→`i64`,
Float→`f64`, Bool→`bool`, Char→`char`, String→`String`,
List→`Vec<T>`, Option→`Option<T>`, Result→`Result<T, E>`,
records→generated structs, Unit→`()`. A Rust host binding
is a naming exercise, not a semantics negotiation. (The
losses on the Rust side are not in the types but in the
*contract*: NF-116-2, §4.)

TypeScript has no `ty_to_*` at all today — the mapping
below is what a binding layer must *decide*, with the
losses measured or named:

| Morphic | TypeScript candidate | Verdict |
|---|---|---|
| Int (i64) | `number` | **Lossy.** Values exact only to 2^53; measured: the literal `9223372036854775807` is emitted bare and a JS engine reads it as `9223372036854775808`. Arithmetic also diverges: Morphic Int division truncates, JS `/` is float division (measured `return (x / 2);`); `%` has no rendering at all today (§4). |
| Int (i64) | `bigint` | **Exact for values**, and JS BigInt `/` truncates toward zero and `%` takes the dividend's sign — both matching Morphic Int semantics. Costs: hosts must write `15n`; `JSON.stringify` throws on bigint; interop with number-typed APIs needs conversion; `[...(x)].length` returns `number`, so even the standing `len` arm mixes the two. A real decision (D3), not a default. |
| Float (f64) | `number` | Exact (same IEEE-754 double; NaN payloads aside). No Float entries ship in 1.0 (D4a), so this binds only user specs. |
| Bool | `boolean` | Exact. |
| Char | `string` (one code point) | **Lossy at the edges.** JS strings are UTF-16: an astral Char has `.length` 2. The Phase 116 `len` arm already chose code-point counting (`[...(x)].length`); the binding must state Char = a string of exactly one code point and police it or trust it (a decision). |
| String | `string` | Values exact; **length semantics differ** — Morphic `len` counts chars (code points, per the standing JS arm), JS `.length` counts UTF-16 units. Any host-visible length must come from the binding, not `.length`. |
| List\<T\> | `T[]` | Structurally exact; element losses inherit (a `List<Int>` is a `bigint[]` or a `number[]` per D3). |
| Option\<T\> | `T \| null` (or `T \| undefined`, or a wrapper) | **Undecided — and today unrepresented.** Generated JS renders constructors as `undefined` (§4), which would make `Some(x)` and `None` indistinguishable under the `undefined` option: the one choice that is *wrong*, not merely unfashionable. D4. |
| Result\<T, E\> | discriminated union `{ tag: "Ok", value: T } \| { tag: "Err", error: E }` | **Undecided — unrepresented today** (same constructor gap). The union form is the only candidate that keeps `option_ok_or`'s output honestly typed. D4. |
| Record (Named) | object / interface with the field types | Closest to working: the JS expression walker already has RecordLit and FieldAccess arms. Field types inherit the rows above. |
| Unit | `undefined` | Already the standing rendering; exact enough. |
| Function types | — | No mapping; no first-class functions exist to embed (G4, §7). Out of the 1.1 surface. |

What the binding layer must therefore decide, in one
list: the Int representation (D3); the Option/Result
representations (D4); Char policing; the length-semantics
note for String/List; and where the tier lives (§2).
Everything else is transcription.


## 4. NF-116-1 / NF-116-2 at the embedding boundary

Phase 116 recorded two findings wider than its clause.
1.1 walks straight into both; here is what they cost,
measured on the bodies a host would plausibly embed —
the shipped entries themselves, generated to JavaScript
by the production `generate()` route and graded with
node v24 (`--check` for parse, direct import for
behaviour). Verbatim captures; the pins are in
`tests/phase118_probes.rs`.

**`int_abs` (Tier-1 — the simplest body in the library):**

```js
function int_abs(x) {
    if ((x < 0)) {
        // unsupported
} else {
        // unsupported
}
}
export { int_abs };
```

Parses. Runs. `int_abs(-5)` returns **`undefined`**. The
If renders; its value branches fall into the statement
catch-all because the JS walker has **no tail renderer**
(a bare expression in tail position is not a statement
it knows). Grade: **wrong answer, silently**.

**`string_is_empty` / `list_is_empty` / `int_is_even`:**
the whole body is one bare expression, so the whole
function is `// unsupported` with no `return` — despite
`len` having a correct JS expression arm since Phase
116. The arm is unreachable from a body whose shape is
"return this expression", because that shape is the one
the walker cannot say. Grade: **wrong answer, silently**.

**`list_sum` (Tier-2):** the For loop and its
`[...(xs)].length` bound render; the accumulator's
`Alloc` and `Assign` statements are `// unsupported`;
the file ends `return acc;` — a variable nothing
declares. Parses; **throws `ReferenceError: acc is not
defined`** on any input. Grade: **not runnable**.

**`option_unwrap_or`, `char_to_digit` (Match bodies):**
`// unsupported` only. Match has no JS arm in either
walker. Grade: **wrong answer, silently**.

**`int_clamp`:** renders nested Ifs with the inner
condition as `(undefined > hi)` — an If in expression
position has no arm, so it becomes the literal
`undefined`, and both branches are `// unsupported`.
Grade: **wrong answer, silently**.

**Expression-level losses** (hand bodies, pinned):
`x % 2` and `x <= 2` render as `return (x ? 2);` — the
operator fallback is a bare `?`, and node rejects the
file with a **SyntaxError**; `Some(x)`, a list literal,
and `xs[0]` each render as `return undefined;`;
`9223372036854775807` renders as itself, a bare JS
number (§3's precision boundary, in generated code).

**The asymmetry that frames the work:** since Phase
116, a body that *calls* an entry refuses loudly on JS
(`CodegenError::UnrenderableLibraryCall`, pinned again
in this audit) — while the entries' *own bodies*
silently break. Phase 116's clause was about calls;
1.1 is about bodies. **NF-116-1's JS slice is therefore
not adjacent to 1.1 — it is 1.1's first build phase.**
The missing inventory, counted from the walkers: a tail
renderer; statement arms for `Let`, `Alloc`, `Assign`,
`While`, `Loop`, `Break`, `Match`; expression arms for
`If`, `Match`, `Index`, constructors, list literals;
operator arms for `Mod`, `Le`, `Ge` (and the Int
division/modulo semantics of D3). Priced **L** as one
phase (120), scoped to the embeddable subset in D5 —
the nodes the 29 shipped bodies use (all of the above
except `While`/`Loop`, which the Tier-2 bodies' For
vocabulary avoids) plus the plausible-winner set.

For completeness, the other targets' grades on the same
bodies: **Python** has type hints (`def list_sum(xs:
list[int]) -> int:`) but the same tail/Alloc/Assign
gaps (`pass # unsupported`, `return acc` undeclared —
NameError at runtime). **C** and **WAT** refuse library
calls loudly (Phase 116) and are not embedding vehicles
for these bodies at all (§1.3). **Rust** is the only
target whose generated library code runs today.

**NF-116-2 on the Rust route:** the generated `int_abs`
carries its contract enforcement verbatim —

```rust
pub fn int_abs(x: i64) -> i64 {
#[cfg(debug_assertions)]
{
    debug_assert!(true, "Precondition violated");
}
```

The precondition (`x` ≠ Int minimum) asserts `true`:
in a debug build the host is *told* a precondition is
being checked and it is not. For route (i)/(ii) Rust
packages this is a documentation-grade defect, not a
correctness one (the function's behaviour on its domain
is exact) — but a "verified function" whose generated
assert asserts nothing is precisely the kind of claim
this project's honesty discipline exists to catch.
Pricing: NF-116-2's fix belongs to the spec-position
rendering pass Phase 116 named; it is **not** required
for the JS-first order in D2, and is recorded here as a
priced line (S–M) the Rust-crate phase (123) should
either take or explicitly defer in the open.

## 5. Packaging routes — npm/wasm-bindgen vs Component Model vs crates.io

**Registry facts (verified live, 2026-11-23):**

- npm: the unscoped name **`morphic` is taken** — an
  unrelated package ("Ad-hoc polymorphism / pattern
  matching / destructuring for function parameters"),
  published since 2015, latest **1.0.16**. Any npm route
  needs a scoped name (`@thvtzy/morphic`) or a different
  name. (Scoped-name availability is a property of the
  owner's npm account, not publicly checkable from here.)
- crates.io: **`morphic` does not exist** (the API,
  asked with a proper User-Agent, returns "crate
  `morphic` does not exist"). The exact name is free for
  the Rust route. (The bare API endpoint 403s without a
  User-Agent — crates.io's bot policy; the first probe
  hit that and the check was re-run properly.)

**Route A — npm via the standing wasm-bindgen machinery.**
What exists: a pinned, self-fetching build (wasm-bindgen
CLI 0.2.123, Binaryen 133 wasm-opt) producing a `--target
web` package for the playground. What an npm package of
*functions* needs beyond it: the generated functions
themselves are **plain JS/TS source** (route i) — the
wasm machinery is only involved if the package ships
the runtime (route iii-wasm) or WASM-compiled functions,
neither of which the recommended order does first. So
Route A's real content is: generated `.js` + generated
`.d.ts` + `package.json` + the manifest of tiers (§2),
published under a scoped name. The bindgen build stays
what it is — the playground's — and is *not* the 1.1
package's build. Gives a host: `npm install`, ESM
imports, TS types, tree-shakeable per-function modules,
a download measured in tens of KB. Costs: Phases
120–122 (the emitter is the cost; the packaging is M).

**Route B — the WASM Component Model (WIT/WASI).** The
north-star's named distribution technology (Phase 107,
Disebar rung). What it would give a host: a
language-neutral, *typed* interface definition — WIT
has `s64`, `list<T>`, `option<T>`, `result<T, E>` and
records as native types, so §3's mapping losses mostly
**evaporate at the boundary** (Int is exactly s64;
Option/Result need no representation invention) — and
hosts in Rust/JS(via jco)/Python/Go via wasmtime-class
runtimes. What it costs from here, measured against the
tree: **no WIT file, no component toolchain, no
component-shaped crate exists** (`cargo-component`,
`wasm-tools`, `wit-bindgen` are not installed; the
wasm-bindgen shell's output is a core module, not a
component, and does not convert). A component route
means a new crate, a WIT world design, a second build
pipeline, and hosts that are (today) a smaller, more
specialist population than npm's. Priced **XL as the
1.1 route**; **M–L later**, because every decision in
§3 transfers to WIT unchanged — the type work is not
wasted by deferring, only the pipeline is postponed.
The honest tension, stated plainly: wasm-bindgen-era
packaging (Route A) is how 1.1 ships on a sane budget;
the Component Model is the *named horizon* and the
better long-term interface — choosing A now is buying
distribution this milestone and deferring the interface
this decade prefers, and the audit's recommendation
(D6) is to say exactly that in the roadmap rather than
to pretend the npm package is the distribution story's
end.

**Route C — the Rust crate.** §1.2's publish-readiness
list is the cost: a function-level surface (D7's
shape), feature posture a library can default to,
docs.rs-buildability. The name is free (above). Gives
a host: `cargo add`, native speed, exact types (§3),
and — uniquely — the option of runtime synthesis with
real verdicts (route iii-native). Semver posture to
decide at publish: the compiler crate at 1.0.0 has
already spent the name's first major on the *language*
release; the audit recommends the published crate's
version track the project version (1.1.0 with v1.1.0)
with the public contract being the facade of D7, not
the compiler's internals.

**Versioning, both routes:** package versions locked
to project releases (the 1.1 packages publish at
**1.1.0**), one changelog, no independent package
versioning — the alternative (packages versioned on
their own cadence) is how a host ends up unable to
tell which compiler verified their functions.

## 6. The 5-minute test — the quickstart, drafted and graded

The literal draft a host developer would follow for
the recommended route (D2a: pre-generated npm package,
entries first). Each step is graded against today.

```bash
npm install @thvtzy/morphic
```

```ts
import { int_clamp, list_min, option_ok_or } from "@thvtzy/morphic";

// A Tier-1 verified function, called like any function:
const clamped = int_clamp(15n, 0n, 10n);        // 10n

// A Tier-2 function returning Option:
const smallest: bigint | null = list_min([3n, 1n, 2n]);  // 1n

// Option in, Result out:
const r = option_ok_or(smallest, "empty");      // { tag: "Ok", value: 1n }
```

| Step | Today |
|---|---|
| 1. Install | ✗ **No package exists.** The unscoped name is taken (§5); nothing is published under any name. Needs: D8 + Phase 122. |
| 2. Import | ✗ **No module to import.** `morphic build` emits one function to one file (§1.4); no package assembly exists. Needs: Phase 122. |
| 3. Call a Tier-1 function with typed values | ✗ **The generated JS does not run** (§4: `int_clamp` generates `(undefined > hi)` and unsupported branches), and there are **no typings** — no `.d.ts` is generated from a spec anywhere in the tree. Needs: Phase 120 (emitter) + Phase 121 (bindings). |
| 4. Receive an Option | ✗ **No representation exists** in generated code (constructors render `undefined`); the `bigint \| null` shape above is D4's *proposal*, not a fact. Needs: D3/D4 + Phase 121. |
| 5. Receive a Result | ✗ Same constructor gap; the discriminated-union shape is D4's proposal. Needs: D4 + Phase 121. |

Score: **0 of 5 steps work today.** The draft is
nevertheless the right shape — every ✗ names a priced
phase, and none of them is research. (The Rust
variant of the draft — `cargo add morphic`, call
`morphic::stdlib::int_abs(-5)` — grades: install ✓ in
principle (name free, §5) but the crate is unpublished
and is the whole compiler (§1.2); the call step ✗ —
no such function item exists; the IR-level eval route
of §1.2 is a mechanism a 5-minute guide cannot ask a
stranger to assemble.)


## 7. Dependencies — does 1.1 need G4, §3.9, or quantified-mod?

**G4 (interpreter function values) — NOT needed.**
Every entry in the 1.0 library is first-order, and a
host embeds first-order functions. What G4's absence
caps: the package cannot grow higher-order entries
(`map`/`filter`/`fold` were removed from the library
in 1.0 for exactly this reason), and a Morphic
function cannot take a host callback. Neither is in
the ratified 1.1 scope. G4 remains the first
post-1.0 build on its own record (D6a of 1.0); 1.1
neither needs it nor blocks it — the campaigns are
independent and the owner may order them either way.

**§3.9 (the List-element encoder) — NOT needed.**
Embedding consumes *executed artifacts*: a Tier-2
entry's executed-exact battery is precisely the
evidence a generated function can honestly carry, and
§3.9 changes no generated byte — it changes the
*label* the entry may wear (Inconclusive →
Verified). What its absence caps is therefore
presentational, and the cap is handled by honesty
machinery, not by the encoder: the tier must be
printed in the package surface (§2, §8 clause 4).
When §3.9 lands (it is the headline post-1.0
campaign on the record), the same packages re-issue
with upgraded labels and identical behaviour.

**Quantified-mod (`int_gcd`'s return) — NOT
needed.** Amendment 1 already took `int_gcd` out of
the library 1.1 would package; it is simply absent
from the package, as it is absent from the prelude.
Its return route is unchanged and independent.

## 8. The 1.1 gate — a sketch (for the owner to amend, then freeze)

In the M1/stdlib style: clauses ticked by measurement
on the final tree, and a closing line. This sketch
assumes the recommended D-answers; the ratify phase
rewrites it against the answers actually taken.

1. **Frozen package surface.** The package's exported
   functions are exactly the frozen 1.1 scope (the 29
   entries, plus any user-module packaging the owner
   scopes in), each named with its signature in the
   package's own manifest.
2. **Host-executed exactness.** Every exported
   function, imported into a *clean* host project
   (fresh `npm` project; fresh `cargo` project) and
   called there, agrees 100% with its frozen
   cross-validation battery (the Phase 111/112/115
   pinned points) — measured in the host, not
   in-process.
3. **Type honesty.** The generated typings compile
   under `tsc --strict` (resp. the Rust crate builds
   as a dependency with no warnings from the facade);
   every lossy mapping in the frozen type table is
   either exact by construction or named, with its
   boundary, in the package's own documentation.
4. **Verification honesty.** Tier-1 exports carry the
   Verified standing the native pipeline gave them —
   the package build re-runs the crowning pins and a
   moved crown fails the build; Tier-2 exports are
   labelled executed-exact **in the package surface**
   (typings + manifest), not only in repo docs; no
   artifact in the package claims a verdict the
   no-Z3 runtime could not produce.
5. **The 5-minute test.** A developer following only
   the shipped quickstart — install → import → typed
   call of one Tier-1 function and one
   Option/Result-returning function — reaches a
   working call in five minutes, timed in the gate
   run against §6's draft as amended.
6. **Budgets.** Package sizes inside the frozen
   numbers (an npm tarball budget set at freeze time
   from the Phase 122 measurement; the playground
   bundle budget stands untouched unless the runtime
   package is scoped in).
7. **Documentation.** The embedding guide states the
   type-mapping table with its losses, what each tier
   means at the host boundary, and the versioning
   lockstep (§5).

> **Closing line (sketch):** *Every function in the
> frozen 1.1 package, imported into a clean host
> project and called with typed values, returns
> battery-exact results with its tier and every
> type-mapping loss named in the package itself —
> and the shipped quickstart takes a stranger from
> install to that call in five minutes.*

Deliberately excluded from the sketch: the Component
Model (D6), the runtime-compiler package (D1),
higher-order entries (G4), Python as a package
target (its emitter shares the JS gaps and is a
stretch the owner may add at freeze time), and any
API-stability promise beyond the release.

## 9. Build plan and owner decisions

### 9.1 Phase plan (numbering continues from this audit)

| Phase | Work | Price |
|---|---|---|
| 119 | **Ratify + freeze**: the owner's D1–D8 calls recorded; `EMBED_SCOPE.md` freezes the 1.1 scope, the type table, and the gate (the Phase 108 shape). Docs only. | S |
| 120 | **JS emitter completion** for the embeddable subset (D5): tail renderer; statement arms (Let/Alloc/Assign/While/Match); expression arms (If/Match/Index/constructors/list literals); operator completion (Mod/Le/Ge + D3's Int semantics). This audit's JS probes flip to the new truth, per node. | L |
| 121 | **Typed bindings**: spec → `.d.ts` generation; Option/Result/record marshalling per D4; tier + verdict into JSDoc and a machine-readable manifest; the String/Char length-semantics notes of §3. | M |
| 122 | **Package assembly + npm publish**: `morphic build --package` (multi-function module, package.json, README, tarball budget measured); publish the entries package under D8's name at 1.1.0. | M |
| 123 | **Rust crate route**: D7's shape — the generated stdlib module + curated facade, library-grade feature defaults, docs.rs-buildable; NF-116-2 taken or explicitly deferred in the open (§4); publish `morphic` on crates.io at 1.1.0. | M |
| 124 | **Quickstart + embedding guide**: §6's draft made real and executed as a test (clean project, timed); GUIDE/docs surface. | S–M |
| 125 | **Gate run + merge + v1.1.0**: the frozen gate clause-by-clause on the final tree (the Phase 117 shape), PR, merge, release. | M |

**Total ≈ L** on the recommended branch. Gating the
Component Model into 1.1 (D6b/c) or the
runtime-compiler package (D1b) raises the total to
**XL**; both are recorded as named post-1.1 steps
under the recommended answers, with §3's and §5's
work transferring.

### 9.2 Owner decisions (D-style, recommendations marked)

**D1 — Package contents.**
(a) **Entries first [recommended]:** the gated
package is the 29 entries of the 1.0 library; the
same generator accepts user modules, ungated (a
host packaging its own spec uses the tool; the gate
measures the entries). (b) Entries + the
runtime-compiler package, both gated (route iii
joins the milestone; +M–L, and the wasm form's
verdict cap must be carried as a named honesty
clause). (c) Runtime compiler only — rejected by
the evidence of §1.1/§2: it cannot call the library
it installs and cannot verify at runtime.

**D2 — Route order.**
(a) **Pre-generated JS/TS package first, Rust crate
second, runtime deferred [recommended]** — the
order §2's honesty and size measurements dictate.
(b) Rust crate first (the only target that works
today; but it spends the milestone on the smaller
host population and leaves the ratified npm promise
unbuilt). (c) Runtime first — see D1c.

**D3 — Int at the JS boundary.**
(a) **BigInt end-to-end [recommended]:** exact
values; JS BigInt division/modulo semantics match
Morphic Int's; the friction (`15n` literals, JSON
conversion) is real but it is the host's *visible*
choice, where a silent 2^53 cliff is not a choice
at all. (b) `number`, with the 2^53 boundary named
in the docs — smaller friction, a correctness cliff
at exactly the magnitudes a *verified* library
should never cliff at. (c) Both: `number` by
default, a BigInt entry point per function —
doubles the binding surface and the battery.

**D4 — Option/Result representation.**
(a) **Option = `T | null`; Result = discriminated
union `{ tag: "Ok", value } | { tag: "Err", error }`
[recommended]** — idiomatic TS, exhaustive
switching, no wrapper allocation; `null` keeps
`None` distinct from every payload and from
`undefined`. (b) Option = `T | undefined`; Result =
`{ ok: boolean, … }` — `undefined` is what the
broken generator already emits for *everything*,
and it collides with Unit's rendering; the boolean
form switches less safely. (c) Wrapper classes
(`Some`/`None`/`Ok`/`Err`) — closest to the
Morphic surface, foreign to TS hosts, and the
heaviest marshalling.

**D5 — JS emitter completion scope.**
(a) **The embeddable subset [recommended]:** every
IR node the 29 shipped bodies use, plus the
plausible-winner set (the §4 inventory), pinned per
node — the bodies the milestone must emit, no
more. (b) Full parity with the Rust emitter —
prices the long tail of nodes no library body
uses (records' full surface, streams, refs) at
roughly another M–L for coverage 1.1 cannot
exercise. (c) Tail renderer only — leaves Match
bodies (three shipped families) unemittable; the
milestone would ship without its Option/Result
entries.

**D6 — The Component Model.**
(a) **Deferred, in the open [recommended]:** named
in the roadmap as the post-1.1 Disebar step; §3's
type decisions are recorded in WIT-compatible
terms so the work transfers (§5). (b) A component
package inside 1.1's gate (+XL pressure on the
milestone; a second pipeline before the first has
shipped). (c) Component *instead of* the npm
package — abandons the ratified npm route and the
largest host population for the horizon's
interface; the ladder's order inverted.

**D7 — Rust crate shape.**
(a) **A slim facade [recommended]:** the published
crate carries the pre-generated stdlib functions
as real Rust code (route ii for Rust) plus the
marshalling a host needs — no compiler dependency
at host build time, no Z3 download, docs.rs-safe;
the compiler crate stays the repo's own tool.
(b) Publish the compiler crate as-is and document
the IR-level eval route — ships §1.2's three
structural problems to every downstream build.
(c) A separate `morphic-stdlib` crate beside a
published compiler crate — two publishes to keep
in lockstep (§5's versioning rule), for a
separation (a) already gives.

**D8 — Package names.**
(a) **npm scoped `@thvtzy/morphic` + crates.io
`morphic` [recommended]** — both grounded in
§5's verified facts (unscoped npm taken;
crates.io free). (b) `morphic-lang` on both
registries — availability *unverified* in this
audit; spends the memorable name for symmetry.
(c) An `@morphic` npm organisation — does not
exist; creating it is an owner action outside
the build, and the scope buys nothing (a) lacks.

## 10. Probe inventory + suite record

Probes: `tests/phase118_probes.rs` — 12 tests,
committed ahead of this document (commit `f186f51`):

- *JS bodies:* `js_tier1_int_abs_has_no_return`,
  `js_expression_bodies_vanish` (string/list
  emptiness, int_is_even), `js_match_bodies_vanish`
  (option_unwrap_or, char_to_digit),
  `js_tier2_list_sum_drops_its_accumulator`,
  `js_entry_calls_refuse_loudly` (the Phase 116
  asymmetry, restated).
- *JS expression losses:*
  `js_mod_and_le_render_a_bare_question_mark`,
  `js_int_literal_is_a_bare_js_number`,
  `js_int_division_renders_float_division`,
  `js_constructors_and_index_render_undefined`.
- *WAT:* `wat_int_abs_is_i32_and_comment_bodied`.
- *Interpreter route:*
  `eval_route_calls_shipped_entries_at_ir_level`
  (int_abs / int_clamp / list_sum through the
  public `eval` with hand-built Envs).
- *Rust contrast:*
  `rust_int_abs_is_real_code_with_the_nf116_2_marker`.

Node grading (§4) was performed out-of-suite (node
v24.22.1 in this sandbox; CI does not promise node):
`int_abs` parses/runs/returns `undefined`;
`string_is_empty` likewise; `list_sum` parses and
throws `ReferenceError`; the `%` body fails
`node --check` with a SyntaxError. Registry checks
(§5) were performed live on 2026-11-23 against
registry.npmjs.org and the crates.io API.

Suites on this branch: **678/0/30 default, 678/0/30
bootstrap-codegen, 545/0/22 no-Z3** — baselines at
v1.0.0 (666/0/30, 666/0/30, 533/0/22) plus exactly
the 12 probes in every configuration, zero failures
(the prelude-reading pins are gated
`#[cfg(not(feature = "wasm-playground"))]` per the
Phase 116/117 precedent; the no-Z3 configuration
builds the root package without that feature, so
all 12 run there too). No merge, no push — the
branch awaits the owner's D1–D8 calls.
