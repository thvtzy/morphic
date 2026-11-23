# EMBED_SCOPE.md — Phase 119: the frozen 1.1 Embeddable scope + gate

The ratification + freeze document for the roadmap target **1.1 —
Embeddable Morphic**: verified functions as ready-to-import
packages (npm/WASM + Rust crate), typed host bindings, "5-minute"
embedding docs/examples. Source material:
[`EMBED_AUDIT.md`](EMBED_AUDIT.md) (Phase 118 — the measured
embedding surfaces of its §1, the priced routes of its §2, the
type-mapping analysis of its §3, the emitter grading of its §4,
the packaging analysis of its §5, the graded quickstart draft of
its §6, the dependency verdicts of its §7, the gate sketch of its
§8), over the library frozen by
[`STDLIB_SCOPE.md`](STDLIB_SCOPE.md) as amended by its
Amendment 1.

**Ratification.** On 2026-11-23 the owner ratified the audit's
recommended package whole — *"Gas rekomendasi audit — D1a sampe
D8a semua"* — adopting decisions **D1a–D8a** as stated in §1
below. This document freezes three things, and later phases tick
them by measurement only:

- **the package surface** (§2): every 1.1 export named, with
  signature and tier; the manifest form the tiers travel in.
  Nothing unnamed is exported as a verified function; nothing
  named is silently dropped — a dropped or re-tiered export is
  a gate amendment, recorded.
- **the type table** (§3): every type of the 1.0 surface mapped
  to its TypeScript and Rust forms, every loss named with its
  boundary, written in WIT-compatible terms so the mapping
  transfers to the Component Model step deferred by D6a.
- **the gate** (§4): the audit's 7-clause sketch finalized into
  gate text in the M1-gate style, closing line included, with
  the npm package budget set numerically from measured sizes.

Docs only: this phase changes no production code, no spec, and
no suite. The campaign that executes against this freeze is
frozen in §5 (Phases 120–125; the audit's plan numbering stands,
119 being this phase).

---

## 1. The ratified decisions (D1a–D8a)

**D1 — Package contents: entries first.** (a) The gated package
is the 29 entries of the 1.0 library (§2); the same generator
accepts user modules, ungated — a host packaging its own spec
uses the tool, and the gate measures the entries. Rejected: (b)
entries + the runtime-compiler package, both gated — +M–L, and
the wasm form's verdict cap (no Z3 in the browser build, so no
Verified verdicts at runtime; audit §1.1) would have to be
carried as a named honesty clause inside the gate itself; (c)
runtime compiler only — rejected by the audit's evidence: it
cannot call the library it installs and cannot verify at
runtime. *Reason in one line: the package is the artifact the
native pipeline verified, not the compiler that produced it.*

**D2 — Route order: pre-generated JS/TS first, Rust crate
second, runtime deferred.** (a) As stated. Rejected: (b) Rust
crate first — the only target that works today, but it spends
the milestone on the smaller host population and leaves the
ratified npm promise unbuilt; (c) runtime first — see D1c.
*Reason in one line: the order is what the audit's honesty and
size measurements dictate (tens of KB of generated source
against 680,280 B of compiler WASM, with the verdicts produced
where Z3 lives).*

**D3 — Int at the JS boundary: BigInt end-to-end.** (a) Morphic
Int is TypeScript `bigint` everywhere in the package — params,
returns, list elements, record fields. Rejected: (b) `number`
with the 2^53 boundary documented — smaller friction, but a
correctness cliff at exactly the magnitudes a *verified*
library should never cliff at (the audit measured the literal
`9223372036854775807` emitted bare and read by JS as
9223372036854775808); (c) both representations — doubles the
binding surface and the battery. *Reason in one line: BigInt is
exact, its `/` truncates toward zero and its `%` takes the
dividend's sign — both matching Morphic Int semantics — and its
friction (`15n` literals, JSON conversion) is the host's
visible choice, where a silent 2^53 cliff is not a choice at
all.*

**D4 — Option/Result representation.** (a) Option = `T | null`;
Result = the discriminated union
`{ tag: "Ok", value: T } | { tag: "Err", error: E }`.
Rejected: (b) Option = `T | undefined` — `undefined` is what
the broken generator already emits for *everything*, it
collides with Unit's rendering, and it would make `Some(x)`
and `None` indistinguishable exactly where the representation
must distinguish them; Result as `{ ok: boolean, … }` switches
less safely than a tag; (c) wrapper classes (`Some`/`None`/
`Ok`/`Err`) — closest to the Morphic surface, foreign to
TypeScript hosts, and the heaviest marshalling. *Reason in one
line: idiomatic TypeScript, exhaustive switching, no wrapper
allocation, and `null` keeps `None` distinct from every payload
and from `undefined`.*

**D5 — JS emitter completion scope: the embeddable subset.**
(a) Every IR node the 29 shipped bodies use, plus the
plausible-winner set — the audit §4 inventory (tail renderer;
statement arms Let/Alloc/Assign/Match; expression arms
If/Match/Index/constructors/list literals; operator arms
Mod/Le/Ge with D3a's Int semantics), pinned per node.
Rejected: (b) full parity with the Rust emitter — roughly
another M–L for the long tail of nodes no library body uses
(records' full surface, streams, refs), coverage 1.1 cannot
exercise; (c) tail renderer only — leaves Match bodies
unemittable, and the milestone would ship without its
Option/Result entries. *Reason in one line: the bodies the
milestone must emit, no more — the subset is a measured list,
not a guess.*

**D6 — The Component Model: deferred, in the open.** (a) Named
in the roadmap as the post-1.1 Disebar step; this freeze's
type decisions are recorded in WIT-compatible terms (§3) so
the work transfers unchanged. Rejected: (b) a component
package inside 1.1's gate — +XL (a new crate, a WIT world, a
second build pipeline, no toolchain in the tree) before the
first pipeline has shipped; (c) Component *instead of* the npm
package — abandons the ratified npm route and the largest host
population for the horizon's interface; the ladder's order
inverted. *Reason in one line: wasm-bindgen-era packaging is
how 1.1 ships on a sane budget; the Component Model is the
named horizon and the better long-term interface — choosing
(a) buys distribution this milestone and defers the interface,
and the roadmap says so rather than pretending the npm package
ends the distribution story.*

**D7 — Rust crate shape: a slim facade.** (a) The published
crate carries the pre-generated stdlib functions as real Rust
code (route ii for Rust) plus the marshalling a host needs —
no compiler dependency at host build time, no Z3 download,
docs.rs-safe; the compiler crate stays the repo's own tool,
unpublished. Rejected: (b) publish the compiler crate as-is
and document the IR-level eval route — ships the audit §1.2's
three structural problems (CLI-grade default features pulling
`reqwest`/`indicatif`; the build-time Z3 download that also
defeats docs.rs; the whole internal surface nominally under
SemVer) to every downstream build; (c) a separate
`morphic-stdlib` crate beside a published compiler crate —
two publishes to keep in lockstep for a separation (a) already
gives. *Reason in one line: the name is spent on the functions
hosts call, not on the compiler that made them.*

**D8 — Package names.** (a) npm scoped **`@thvtzy/morphic`**;
crates.io **`morphic`** — grounded in the audit §5's facts
verified live on 2026-11-23 (the unscoped npm name is taken by
an unrelated package, published since 2015, latest 1.0.16;
crates.io `morphic` does not exist). Rejected: (b)
`morphic-lang` on both registries — availability unverified in
the audit, and it spends the memorable name for symmetry; (c)
an `@morphic` npm organisation — does not exist, creating it
is an owner action outside the build, and the scope buys
nothing (a) lacks. *Reason in one line: the names the verified
facts leave standing.*

---

## 2. The frozen package surface

### 2.1 The exports — 29 entries, by family and tier

The gated package surface is exactly the 1.0 library as frozen
in `STDLIB_SCOPE.md` as amended by its Amendment 1 — 29 entries,
**18 Tier-1 / 11 Tier-2**. Signatures are the frozen Morphic
signatures; the TypeScript and Rust forms of every export
derive mechanically from §3's table. Export names are the
entry names, unchanged, in both packages — renaming would
break the manifest↔scope correspondence the gate checks
(§4 clause 1), and Rust names are native in this form.

**Int — Tier-1 (7):**

| Export | Signature | Boundary notes |
|---|---|---|
| `int_abs` | `Int → Int` | Precondition (frozen in the 1.0 scope): the Int minimum is excluded — see §6 R3. |
| `int_sign` | `Int → Int` | The Int minimum maps to −1 (no exclusion). |
| `int_min` | `(Int, Int) → Int` | — |
| `int_max` | `(Int, Int) → Int` | — |
| `int_clamp` | `(Int, Int, Int) → Int` | `require: lo <= hi` — see §6 R3. |
| `int_is_even` | `Int → Bool` | Sign-independent. |
| `int_is_odd` | `Int → Bool` | Sign-independent. |

**Char — Tier-1 (6):**

| Export | Signature | Boundary notes |
|---|---|---|
| `char_is_digit` | `Char → Bool` | ASCII ranges, as frozen in 1.0. |
| `char_is_alpha` | `Char → Bool` | ASCII letters only. |
| `char_is_whitespace` | `Char → Bool` | The standing tokenizer spec's set, adopted unchanged in 1.0. |
| `char_is_uppercase` | `Char → Bool` | Non-letters → false. |
| `char_is_lowercase` | `Char → Bool` | Non-letters → false. |
| `char_to_digit` | `Char → Option<Int>` | Non-digits → `None` (`null` in TypeScript, §3). |

**String basics — Tier-1 (1):**

| Export | Signature | Boundary notes |
|---|---|---|
| `string_is_empty` | `String → Bool` | True iff the string has zero code points (§3). |

**Result / Option — Tier-1 (3):**

| Export | Signature | Boundary notes |
|---|---|---|
| `result_unwrap_or` | `(Result<Int, String>, Int) → Int` | `Err(_)` → the default argument. |
| `option_unwrap_or` | `(Option<Int>, Int) → Int` | `None` → the default argument. |
| `option_ok_or` | `(Option<Int>, String) → Result<Int, String>` | `None` → `Err(e)`, e unchanged. |

**List — Tier-1 (1):**

| Export | Signature | Boundary notes |
|---|---|---|
| `list_is_empty` | `List<Int> → Bool` | Length-level contract, as frozen in 1.0. |

**List — Tier-2 (11)** (executed-exact on the full battery,
tier printed in the package surface per §2.4):

| Export | Signature | Boundary notes |
|---|---|---|
| `list_reverse` | `List<Int> → List<Int>` | Empty → empty. |
| `list_contains` | `(List<Int>, Int) → Bool` | Empty → false. |
| `list_sum` | `List<Int> → Int` | Empty → 0. Overflow is out-of-domain (§6 R3). |
| `list_product` | `List<Int> → Int` | Empty → 1. Overflow is out-of-domain (§6 R3). |
| `list_min` | `List<Int> → Option<Int>` | Empty → `None` — a value, not a precondition. |
| `list_max` | `List<Int> → Option<Int>` | Empty → `None`. |
| `list_take` | `(List<Int>, Int) → List<Int>` | n clamped into `[0, len(xs)]`, as frozen in 1.0. |
| `list_drop` | `(List<Int>, Int) → List<Int>` | Same clamping. |
| `list_is_sorted` | `List<Int> → Bool` | Non-strictly ascending; empty/singleton → true. |
| `list_sort` | `List<Int> → List<Int>` | The permutation property is part of its executed battery, as frozen in 1.0. |
| `list_binary_search` | `(List<Int>, Int) → Option<Int>` | `require:` the list is sorted — see §6 R3. Ships as the first-match scan disclosed in its 1.0 documentation; that disclosure travels with the package (§2.4). With duplicate targets, any valid index satisfies the contract — the specific index is spec-silent in its battery, and the manifest records it as such. |

**Worked forms** (the audit §6 quickstart's three, fully
mapped by §3 — every other export maps the same way):

| Export | TypeScript | Rust |
|---|---|---|
| `int_clamp` | `(x: bigint, lo: bigint, hi: bigint) => bigint` | `fn int_clamp(x: i64, lo: i64, hi: i64) -> i64` |
| `list_min` | `(xs: bigint[]) => bigint \| null` | `fn list_min(xs: Vec<i64>) -> Option<i64>` |
| `option_ok_or` | `(o: bigint \| null, e: string) => { tag: "Ok", value: bigint } \| { tag: "Err", error: string }` | `fn option_ok_or(o: Option<i64>, e: String) -> Result<i64, String>` |

### 2.2 Names and module shape

- **npm:** the package is **`@thvtzy/morphic`**, ESM-only
  (§6 R7). Its root module exports all 29 entries; each entry
  is also importable from its own per-function module
  (tree-shakeable, the audit's Route A shape). The exact
  subpath layout is Phase 122's, recorded there.
- **crates.io:** the package is the facade crate **`morphic`**
  (D7a), its functions under **`morphic::stdlib`** — the shape
  of the audit §6 Rust draft. The repo's compiler crate is not
  published; the name is spent on the facade.
- Both packages are published only as release acts of
  Phase 125, from the tagged v1.1.0 tree (§6 R5, R6).

### 2.3 Not exports: the builtins and the method table

The 1.0 builtin tier — **`len`** and **`int_to_str`** — is
**not** part of the package surface in either package
(§6 R1). They are language builtins, not library entries:

- Inside generated code they are implementation details,
  rendered by the emitters' standing arms (Phase 116: JS
  `len` as code-point/element length, `int_to_str` as
  `String(x)` over the BigInt form — exact on the i64 domain;
  Rust `len`/`to_string()` natively).
- A host needing them uses the host-native equivalent under
  §3's stated semantics: for a `List<T>` value, the array's
  element count is the same number `len` returns; for a
  String, the length is a **code-point** count (`[...s].length`
  in JS), never `.length` (§6 R10); for `int_to_str`, JS
  `String(x)` / Rust `x.to_string()`.
- The closed method table (`Result::{is_ok, is_err, unwrap}`,
  `Option::{is_some, is_none, unwrap}`, `List::len`) is
  likewise spec-side surface, not package surface: hosts call
  the exported functions; method calls appear only inside
  generated code.

### 2.4 The tier manifest (frozen form)

An entry's tier must be a fact of the package surface, not of
the repo's docs (audit §2, gate clause 4). Both packages
therefore carry a machine-readable manifest,
**`morphic.manifest.json`**, plus the tier in prose at every
function (JSDoc in the `.d.ts`; rustdoc in the facade):

- **Location:** the npm package root; the same JSON file
  inside the facade crate's package.
- **Package-level fields:** `package` (the package name),
  `packageVersion`, `morphicVersion` (the project version
  whose pipeline generated and verified the contents — equal
  to `packageVersion` at any release, §6 R6), `scope` (this
  document's frozen surface, by name).
- **Per-export fields:** `name`, `signature` (the frozen
  Morphic signature of §2.1), `tier` (`"tier-1"` |
  `"tier-2"`), `verdictBasis` (`"z3-verified"` for Tier-1,
  `"executed-exact"` for Tier-2), `battery` (the entry's
  frozen cross-validation battery as `{ pinned, specSilent }`
  point counts, from the standing Phase 111/112/115
  batteries), `doc` (the entry's doc string, carried verbatim
  — entry-level disclosures, e.g. `list_binary_search`'s
  first-match scan, travel with the package by this field and
  by the JSDoc/rustdoc carriage of the same text).

The field names above are frozen; the values are data, filled
by Phase 121 (npm) and Phase 123 (crate) from the standing
batteries. A manifest whose rows do not correspond exactly to
§2.1's table fails gate clause 1.

---

## 3. The frozen type table

Every type of the 1.0 surface, mapped per D3a/D4a. The WIT
column states the same mapping in Component Model terms
(D6a): the decisions transfer to a future WIT world unchanged;
only the pipeline is deferred. **Losses are named with their
boundaries** — this table is gate clause 3's checklist, and
gate clause 7 requires it, losses included, in the embedding
guide.

| Morphic | TypeScript | Rust | WIT | Losses and boundaries |
|---|---|---|---|---|
| `Int` | `bigint` | `i64` | `s64` | **TS: exact inside the i64 domain.** JS BigInt is unbounded, so the representation itself is lossless; the *domain* is the boundary — a host value outside `[−2^63, 2^63−1]` is an out-of-domain input (§6 R3), not a mapping loss. BigInt `/` truncates toward zero and `%` takes the dividend's sign, matching Morphic Int semantics (audit §3). Friction, recorded not hidden: `15n` literals; `JSON.stringify` throws on bigint — both are the host's visible choices (D3a). Rust/WIT: exact. |
| `Float` | `number` | `f64` | `f64` | Exact (the same IEEE-754 double on every side; NaN payloads aside). No Float entry is in the frozen surface (1.0's D4a); this row binds the ungated user-module generator only. |
| `Bool` | `boolean` | `bool` | `bool` | Exact. |
| `Char` | `string` — exactly one code point | `char` | `char` | **TS boundary:** JS strings are UTF-16, so a Char is a string of exactly one Unicode code point (an astral Char has `.length` 2). The binding trusts this contract — no runtime policing in 1.1 (§6 R2); a string that is not exactly one code point is out-of-domain (§6 R3). Rust/WIT: exact. |
| `String` | `string` | `String` | `string` | Values exact on every side. **Length semantics differ and are named:** Morphic counts code points; JS `.length` counts UTF-16 code units. Any host-visible length comes from generated code or §2.3's documented forms, never from `.length` (§6 R10). A string containing lone surrogates is out-of-domain (it has no Morphic String value). Rust/WIT: exact as values. |
| `List<T>` | `T[]` | `Vec<T>` | `list<T>` | Structurally exact; element losses inherit recursively from T's row (a `List<Int>` is a `bigint[]`). |
| `Option<T>` | `T \| null` | `Option<T>` | `option<T>` | TS: `None` is `null`, distinct from every payload and from `undefined` (D4a). **Named composition boundary:** `Option<Option<T>>` flattens under `T \| null` — `Some(None)` and `None` are indistinguishable. No frozen export carries a nested-Option signature (§2.1, checked), so the gated surface is exact; the ungated generator inherits the flattening for user specs, recorded here (§6 R4) so it is a known boundary, not a silent wrong. Rust/WIT: exact, nesting included. |
| `Result<T, E>` | `{ tag: "Ok", value: T } \| { tag: "Err", error: E }` | `Result<T, E>` | `result<T, E>` | Exact as a discriminated union (D4a); nests without collision — the tags distinguish every level. Rust/WIT: exact. |
| Record (named) | object type / interface with the field types | generated struct | `record` | Field types inherit the rows above; field order is not semantic. No record-typed export is in the frozen 29 (records bind the ungated generator); the JS walker's RecordLit/FieldAccess arms are the closest-to-working surface the audit found. |
| `Unit` | `undefined` | `()` | — (no value) | The standing rendering; exact enough. User-module specs only. |
| Function types | — | — | — | **No mapping exists.** No first-class functions are in the 1.1 surface (G4 stands out, §7); a Morphic function cannot take a host callback, and no export is higher-order. |

---

## 4. The 1.1 gate (frozen text)

*This is the gate. It is ticked by measurement on the final
tree, in the M1 gate's manner; its text is amended only by a
recorded owner decision, never at the gate run.*

**The npm package budget, frozen here (for clause 6).** The
budget binds the npm package's **unpacked mass** — the
"unpacked size" `npm publish --dry-run` reports — because the
derivation is in source-mass terms; the compressed tarball is
a transport property and is reported at the gate for the
record, not budgeted. Derivation, by the W4 discipline
(measurement + margin, rounded up to a whole KiB, recorded as
a deliberate figure):

| Component | Basis | Bytes |
|---|---|---|
| Generated code (`.js`) | Audit §2's measurement: all 29 entries generated as standalone Rust files sum to 37,149 B — each file carrying its own test module and duplicated transitive entry deps, so a deliberate over-bracket for one shared JS module without test modules | 37,149 |
| Typings (`.d.ts`) | The entries' doc text measured at 6,205 B (the `doc:` strings Phase 121 carries into JSDoc) + declarations, JSDoc framing, tier lines and shared type aliases, bracketed | 12,400 |
| Tier manifest | §2.4's form: 29 rows at ~200 B + package-level header, bracketed | 6,200 |
| Package furniture | `package.json` + README, bracketed 4,100 B; the license texts measured — LICENSE-MIT 1,083 B + LICENSE-APACHE 11,336 B (the crate is dual-licensed "MIT OR Apache-2.0" and the package carries both texts; the Apache text is the largest single furniture item) | 16,519 |
| **Composite measurement** | | **72,268** |
| + 10% margin, rounded up to a whole KiB | 72,268 × 1.10 = 79,494.8 → 78 KiB | **79,872 B** |

The Rust crate carries no size budget (§6 R8).

**The clauses:**

1. **Frozen package surface.** The npm package
   `@thvtzy/morphic` and the Rust crate `morphic` export
   exactly the frozen 1.1 surface — the 29 entries of §2.1,
   each named with its signature in `morphic.manifest.json`
   (§2.4), the manifest's rows corresponding exactly to
   §2.1's table. Nothing else is exported as a verified
   function; no frozen export is missing. The user-module
   generator (D1a) is a tool, not package surface: clauses
   2–5 do not measure it, and its output carries no tier
   claim the native pipeline did not produce (§6 R12).
2. **Host-executed exactness.** Every exported function,
   imported into a *clean* host project (fresh npm project;
   fresh cargo project) and called there, agrees 100% with
   its frozen cross-validation battery (the Phase 111/112/115
   pinned points; spec-silent points excluded exactly as the
   batteries record them) — measured in the host, not
   in-process.
3. **Type honesty.** The generated typings compile under
   `tsc --strict`; the Rust facade builds as a dependency
   with no warnings from the facade. Every loss named in
   §3's table is either exact by construction or named, with
   its boundary, in the package's own documentation (typings
   JSDoc and the embedding guide) — §3's table is the
   checklist.
4. **Verification honesty.** The package build re-runs the
   crowning pins; a moved crown fails the build. Tier-1
   exports carry the Verified standing the native pipeline
   gave them; Tier-2 exports are labelled executed-exact
   **in the package surface** (typings JSDoc + manifest),
   not only in repo docs. No artifact in either package
   claims a verdict the no-Z3 runtime could not produce;
   the runtime compiler is not in the package (§7).
5. **The 5-minute test.** A developer following only the
   shipped quickstart — install → import → typed call of one
   Tier-1 function and one Option/Result-returning function,
   in the shapes frozen by D3a/D4a (the audit §6 draft as
   amended by this freeze) — reaches a working call in five
   minutes, timed in the gate run.
6. **Budgets.** The npm package's unpacked mass is inside
   **79,872 B** (frozen above; the compressed tarball size
   is reported for the record). The playground bundle budget
   — 771,072 B, standing — is untouched: the runtime package
   is not scoped in (D1a/D2a), so nothing in 1.1 spends it.
7. **Documentation.** The embedding guide states §3's type
   table with its losses and boundaries, what each tier
   means at the host boundary, and the versioning lockstep
   (§6 R6); and it names the deferred items in the open —
   the Component Model as the post-1.1 Disebar step (D6a)
   and the runtime-compiler package as deferred (D1a/D2a).
   The packages do not present themselves as the end of the
   distribution story.

**Closing line.** *Every function in the frozen 1.1 package,
imported into a clean host project and called with typed
values, returns battery-exact results with its tier and every
type-mapping loss named in the package itself — and the
shipped quickstart takes a stranger from install to that call
in five minutes.*

---

## 5. The frozen phase plan (119–125)

Each phase branches from the previous phase's tip (the
standing stack discipline); a phase's entry condition is the
prior phase landed with its exit evidence green. Prices are
the audit's, standing.

| Phase | Work | Price | Entry condition → deliverables |
|---|---|---|---|
| 119 | **Ratify + freeze** (this phase): D1a–D8a recorded; this document freezes the surface, the type table, and the gate. Docs only. | S | Owner ratification → `EMBED_SCOPE.md`. |
| 120 | **JS emitter completion, embeddable subset** (D5a): tail renderer; statement arms Let/Alloc/Assign/Match; expression arms If/Match/Index/constructors/list literals; operator arms Mod/Le/Ge with D3a's BigInt Int semantics. | L | 119 frozen → every one of the 29 shipped bodies generates to JS that parses (`node --check`) and returns correct results on its in-spec tests under node; the Phase 118 JS probes flip per node, at their sites; suites green. |
| 121 | **Typed bindings**: spec → `.d.ts` generation implementing §3; Option/Result/record marshalling in generated code as §3 requires; tier + verdict into JSDoc and `morphic.manifest.json` generation per §2.4. | M | 120 landed → generated typings for all 29 exports compile under `tsc --strict`; the manifest generates with rows exactly §2.1's table. |
| 122 | **Package assembly + npm publish-readiness**: `morphic build --package` (multi-function module assembly, `package.json`, README, manifest included, per §2.2). | M | 121 landed → the `@thvtzy/morphic` package assembled; unpacked mass measured against §4's 79,872 B; `npm publish --dry-run` clean. The publish itself is Phase 125's (§6 R5). |
| 123 | **Rust crate route** (D7a): the facade crate `morphic` — the 29 functions as real Rust code under `morphic::stdlib` + host marshalling; library-grade posture (no compiler dependency, no Z3 download, docs.rs-buildable, verified by an offline docs build). NF-116-2 taken, or deferred in the open (§6 R9). | M | 122's generator machinery → facade builds as a dependency with no warnings; `cargo publish --dry-run` clean. The publish itself is Phase 125's (§6 R5). |
| 124 | **Quickstart + embedding guide**: the audit §6 draft made real as the shipped quickstart; the guide carries clause 7's contents. | S–M | 122 + 123 assembled (dry-run artifacts suffice) → the quickstart executed as a timed test in a clean project: install from the locally packed tarball → import → typed calls, including one Option/Result-returning function, inside five minutes. |
| 125 | **Gate run + merge + v1.1.0**: the frozen gate (§4) run clause-by-clause on the final tree; the embed stack merged (PR + merge commit, the Phase 117 shape); release v1.1.0; the npm and crates.io publishes executed as release acts after a PASS verdict. | M | 124 landed → gate verdict recorded; on PASS, merge + tag + GitHub Release + both registry publishes. On any clause failing measurement: stop, record, no merge, no release, no publish. |

**Total ≈ L** on the ratified branch (the audit's price,
standing). Gating the Component Model or the
runtime-compiler package in would have raised it to XL;
both stand as named post-1.1 steps (§7).

---

## 6. Resolutions (freeze-time, recorded)

The audit left the following open; each is resolved here,
conservatively, from the audit's evidence, and recorded so
the freeze is interpretable. None is a silent guess: each
states its basis, and R12 records what remains the owner's.

- **R1 — The builtins are not exports.** `len` and
  `int_to_str` do not appear in either package's surface
  (§2.3). Basis: they are language builtins, not library
  entries; Phase 116 already renders them correctly inside
  generated code on the routes 1.1 ships; and a host's
  native equivalent exists for both, under §3's stated
  semantics. Exporting them would put a one-line host-native
  operation under a verification claim it does not need.
- **R2 — Char is trusted, not policed.** The typings declare
  Char as `string` with the one-code-point contract stated
  in JSDoc (§3); generated code performs no runtime check in
  1.1. Basis: a per-call check taxes every call for a
  contract the host can see in the types' documentation,
  and a branded/opaque Char type was not among the audit's
  candidates — it would also break the §6 quickstart's
  shape. The boundary is named in §3 instead.
- **R3 — Out-of-domain inputs are contract violations, not
  runtime errors.** Generated functions perform no runtime
  precondition checks in 1.1. The frozen preconditions
  (`int_abs` at the Int minimum; `int_clamp` with
  `lo > hi`; `list_binary_search` on an unsorted list) are
  stated in each export's JSDoc and stand as contract;
  behaviour on out-of-domain inputs is deterministic —
  whatever the generated body computes — but outside the
  battery-exactness promise of clause 2. The same boundary
  covers host BigInts outside the i64 domain (§3), strings
  that are not exactly one code point where a Char is
  expected (R2), lone-surrogate strings, and arithmetic
  overflow: the 1.0 batteries exclude overflowing
  sum/product points by design (the language's Int
  arithmetic is checked — such points have no value to agree
  on), so computations that would overflow are out-of-domain
  for the affected exports. Basis: uniformity — checking one
  precondition and not another would imply a promise the
  package does not make; the batteries define the domain,
  and the domain is printed.
- **R4 — Nested compositions.** `Option<Option<T>>`
  flattens under D4a's `T | null` (§3): a named boundary of
  the TypeScript mapping, not a defect of the gated surface
  — no frozen export carries a nested-Option signature.
  `Result` nests exactly (its tags distinguish every level);
  `List<Option<T>>` and `Option<List<T>>` are exact. The
  ungated user-module generator inherits the flattening for
  user specs with nested Options; it is recorded here so
  that inheritance is a known boundary, and a future
  amendment (a wrapper form for nested Options) is the
  route if it ever matters.
- **R5 — No registry publish before the gate.** Phases 122
  and 123 assemble and dry-run only (`npm publish --dry-run`,
  `cargo publish --dry-run`); the actual publishes are
  release acts of Phase 125, executed after a PASS verdict.
  This resolves the audit plan's wording (which said
  "publish" inside 122/123) conservatively, on the
  Phase 117 precedent: the release follows the gate, never
  precedes it — a registry publish cannot be un-merged.
- **R6 — Versioning lockstep, mechanics.** Both packages
  carry the project version — **1.1.0** at v1.1.0 — published
  only from the tagged release tree, under one changelog
  (this repo's). The manifest records `packageVersion` and
  `morphicVersion` separately so a host can see both; they
  are equal at any release by construction. No independent
  package cadence, and no pre-release builds are published
  to the registries under the frozen names. Basis: audit §5
  — packages versioned on their own cadence are how a host
  ends up unable to tell which compiler verified their
  functions.
- **R7 — The npm package is ESM-only.** `"type": "module"`,
  per-function modules, environment-neutral generated code
  (no Node-only APIs; BigInt is ES2020). A CommonJS form is
  not shipped in 1.1: a second module form doubles the
  binding surface the gate must measure. Recorded as an
  exclusion (§7) and a first amendment candidate if hosts
  ask.
- **R8 — The budget binds the npm package only.** §4's
  figure is on the npm package's unpacked mass. The Rust
  crate carries no size budget: crates.io distributes
  source, the facade's generated module is bracketed by the
  same 37,149 B measurement as the JS code mass, and no
  measured constraint binds. Recorded so the absence is a
  decision, not an oversight.
- **R9 — NF-116-2's decision point is Phase 123.** The
  generated Rust precondition asserts degrade every
  `require:` row to `true` (audit §4: the generated
  `int_abs` carries `debug_assert!(true, "Precondition
  violated")` verbatim — a documentation-grade defect on the
  "verified function" claim). Phase 123 either takes the fix
  (priced S–M in the audit) or defers it in the open, with
  the deferral and its reason named in the crate's own
  documentation. Gate clauses 3 and 7 measure whichever was
  chosen; silence is the only failing outcome.
- **R10 — Length at the boundary.** No `len` export exists
  (R1), so the guide states the rule (§3, clause 7): for a
  List value, the host array's element count is the number
  `len` returns; for a String, the length is a code-point
  count (`[...s].length`), never `.length`. Generated code
  that needs a length internally uses the Phase 116 arms,
  which already implement exactly this.
- **R11 — Tier-2 labels upgrade without byte changes.**
  When the §3.9 List-element encoder lands (the headline
  post-1.0 campaign on the record), Tier-2 entries it
  verifies re-issue with upgraded `verdictBasis` in the
  manifest and JSDoc/rustdoc, at a lockstep release (R6);
  the functions' behaviour and bytes do not change (audit
  §7). The manifest form of §2.4 already carries the field
  this upgrade writes to — no format change is needed.
- **R12 — What remains the owner's (prerequisites, not
  decisions).** No open item in this freeze requires an
  owner decision: D1a–D8a settled the decision space the
  audit opened, and §§2–5 are frozen from them. Two
  execution prerequisites are the owner's and cannot be
  settled here, recorded so Phases 122/123/125 plan around
  them: **control of the `thvtzy` npm scope** (scoped-name
  availability is a property of the owner's npm account —
  audit §5) and **registry publish credentials** for npm
  and crates.io at Phase 125's publishes. The ungated
  user-module generator's standing is also frozen here, per
  D1a: it is a tool whose output carries the verdicts the
  native pipeline produced for those specs — never a
  Verified claim minted by a no-Z3 route, and never this
  gate's subject.

---

## 7. Explicit exclusions

Named here so their absence from §§2–5 is a decision, in
the open:

- **The runtime-compiler package** (deferred per D1a/D2a).
  Its wasm form cannot produce Verified verdicts (no Z3 in
  the browser build) and cannot call the library it
  installs; it must never be sold as "verified functions".
  The playground shell it would ship from stays the
  playground's.
- **The Component Model** (D6a) — the post-1.1 Disebar
  step, named in the roadmap and in the embedding guide
  (clause 7). §3's decisions are recorded in WIT-compatible
  terms so the step inherits them.
- **G4-dependent contents** — higher-order entries
  (`map`/`filter`/`fold` class, already out of the library
  per 1.0's D6a) and host callbacks. G4 remains the first
  post-1.0 build on its own record; 1.1 neither needs it nor
  blocks it (audit §7).
- **`int_gcd`** — `STDLIB_SCOPE.md` Amendment 1 stands: out
  of the library, returning with the post-1.0 quantified-mod
  campaign; therefore absent from the packages, as it is
  absent from the prelude.
- **Python as a package target** — its emitter shares the
  JS gaps (audit §4) and it was a stretch the audit priced
  outside the ratified routes; not in 1.1.
- **WAT as a package vehicle** — a text preview (v0.6 D6)
  whose type map truncates Morphic Int to `i32` (audit
  §1.3); not an embedding surface.
- **A CommonJS module form** (§6 R7) and **API-stability
  promises beyond the 1.1 release** (the gate does not
  pre-commit 1.x policy, as in 1.0).

---

## 8. The count

**29 exports + 0 builtin exports = the frozen 1.1 package
surface**, in two packages (`@thvtzy/morphic` on npm,
`morphic` on crates.io), at one version (1.1.0, lockstep).

| Family | Exports | Tier-1 | Tier-2 |
|---|---|---|---|
| Int (§2.1) | 7 | 7 | — |
| Char (§2.1) | 6 | 6 | — |
| String basics (§2.1) | 1 | 1 | — |
| Result / Option (§2.1) | 3 | 3 | — |
| List (§2.1) | 12 | 1 | 11 |
| **Total** | **29** | **18** | **11** |

(The 2 builtins, `len` and `int_to_str`, are not exports —
§2.3, §6 R1. The tier split is the 1.0 scope's, as amended:
Tier-1 18 / Tier-2 11.)
