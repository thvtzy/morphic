# DISEBAR_16_SCOPE.md — Phase 159: the frozen 1.6 scope + the gate

The ratification + freeze document for **1.6 — the same
proven functions, at the Component Model boundary**: the
Disebar rung's third milestone, carrying the **library
component** and the **callback component**, and nothing
else. Source material: [`DISEBAR_16_AUDIT.md`](DISEBAR_16_AUDIT.md)
(Phase 158 — the one Disebar component whose price was
set by an absent pipeline, re-measured against the
v1.5.0 tree with the real toolchain installed and
exercised; probe artifact
`tests/phase158_component_probes.rs`, 5 probes, every
audit claim a `P158|` line; prototypes under
`~/workspace/comp-proto-158/`).

**Ratification.** On 2026-11-25 the owner delegated this
campaign's decisions to Muse — *"buat semua aku kasih
keputusannya ke kamu"* — and the audit's recommended
package **D1a–D7a** stands ratified by Muse under the
owner's delegation, in the open, each decision with its
grounds stated in §1. The delegation is scoped to this
campaign, as the G4 delegation was scoped to its round
(Phase 127 precedent); the ratification is recorded
here so the owner can veto any item at any time. Per
D7a this milestone is one boundary story under one
gate: the library's proven functions answering from a
boundary no single language owns — the thirty that
cross as values, and the five that cross honestly as
the imports they are. This document freezes:

- **the milestone's contents** (§2): the two components,
  the lockstep version, and what stays the owner's act.
- **the toolchain** (§3): the two prebuilt pins, the one
  crate pin, the target — and the tool not adopted.
- **the library component** (§4): the facade wrapped,
  the WIT world, the frozen surface, the artifact of
  record.
- **the callback component** (§5): Phase 160's build,
  frozen here — the five over three imports, a separate
  artifact.
- **the budgets** (§6): the figures the gate measures.
- **the exclusions** (§7): what is not in this milestone.
- **the gate** (§8): the audit's 5-clause sketch
  finalized into gate text, closing line verbatim from
  the audit's draft.
- **the phase plan** (§9): Phases 159–161.

This phase (159) lands the freeze and the library
component against §4. Phase 160 builds §5 and stands
both runner rehearsals up as the gate's instruments.
Phase 161 runs §8.

---

## 1. The ratified decisions (D1a–D7a)

**D1a — Source of truth + home: wrap the facade**
from a new crate at `packages/component/`, its own
workspace root; the WIT package is `morphic:library`,
versioned at the lockstep (§4). *Grounds:* the facade
(`packages/rust`) is the gated Rust surface — zero
dependencies, the 35 real functions; the compiler route
would drag the compiler (and Z3's build) into a wasm
target it has never built for, to produce the same
functions (audit §2.1). A fresh crate over the compiler
(b) and a `crates/` build-time shell (c) were both
priced and rejected on those grounds.

**D2a — The G4 fork: host-imported callbacks,
shipped as a second component** — the `library` world
stays import-free but for §3's named WASI surface, and
the five higher-order entries cross in the sibling
`library-callbacks` world over three host imports
(§5). *Grounds:* a component's imports are satisfied
at instantiation, for the whole component — folding
the imports into the library component would lock out
every consumer that only wants `int_clamp` (audit §3).
Excluding the five (b) amputates five Tier-1 entries
from the milestone whose theme is "the same proven
functions"; baked monomorphic instantiations (c)
certify the batteries' lambdas, not the entries — the
R6 lesson of Phase 143.

**D3a — The WASI import surface: accept std's
wasip2 imports for 1.6**, named in the package README
(§4.4); the strip (a core-module build +
`wasm-tools component new` pipeline) is a named
follow-up (§7). *Grounds:* both gate runners satisfy
the surface today with zero configuration (audit
§2.4); the strip is a second pipeline bought for hosts
not yet named.

**D4a — The runners: the pinned wasmtime CLI** as
the library component's gate instrument, **the
jco/Node harness** as the callback component's; the
wasmtime-crate host is an optional rehearsal, not gate
evidence. *Grounds:* both instruments are measured
end-to-end (audit §§4.1–4.2); the CLI cannot exercise
an imported callback by design, and the crate host's
compile is the most expensive object in the campaign
(a several-hundred-crate closure that did not finish
in 10.5 minutes under load) and buys a second host,
not a stronger verdict (audit §4.3).

**D5a — Toolchain pins: wasm-tools 1.263.0 +
wasmtime 51.0.0 as prebuilt pins** under
`~/workspace/tools/` (CI downloads the same tarballs);
**wit-bindgen pinned "=0.63.0"** in the component
crate; `wasm32-wasip2` via rustup. **cargo-component
is NOT adopted.** *Grounds:* every pin is installed
and exercised (audit §1); cargo-component's latest
release (v0.21.1) ships zero assets — its only install
route is a from-source build of a tool whose closure
includes cargo itself — and plain cargo provably
builds the component without it (audit §1.3).

**D6a — Budgets: per-artifact** — library component
≤ 106,496 B, callback component ≤ 90,112 B (the Phase
158 measurements +10%, rounded up to a whole KiB, the
W4 formula). Standing budgets unmoved (§6). *Grounds:*
audit §5's measurements.

**D7a — Milestone shape: one milestone, one gate,
phases 159–161, lockstep 1.6.0.** *Grounds:* the two
components share one WIT lineage, one facade, and one
honesty story; splitting them would gate the fork
twice (audit §6).

---

## 2. The milestone

**1.6 = the library component + the callback
component, and nothing else.** Lockstep **1.6.0**:
crate, packages, the WIT package version, and the
release move together at the Phase 161 stamp; until
then every surface stands at the tree's version.

**Distribution.** No registry takes a bare component
today, so 1.6's distribution is the repo + the GitHub
release assets (audit §5). Registry publishing of the
JS/Rust packages remains the owner's act, as it has
stood since 1.1; nothing in this milestone publishes
anything anywhere. A component registry is 1.7's
neighbouring question, not this milestone's (§7).

**Non-movers.** The library's standings are untouched:
35 entries (33 Tier-1 / 2 Tier-2), the manifest rows,
the PCC bundles, the MCP server, the LSP server, the
edge template, the VS Code extension. The components
are new surfaces over the same crowned functions; no
body, verdict, or battery moves.

---

## 3. The toolchain

The pipeline is **plain cargo** — there is no second
build system (audit §1.5). The pins, as measured:

| Piece | Pin | Source / sha256 |
|---|---|---|
| target | `wasm32-wasip2` | `rustup target add` (37 s, once, durable in `~/.rustup`) |
| wasm-tools | **wasm-tools 1.263.0** | upstream `wasm-tools-1.263.0-x86_64-linux.tar.gz`, sha256 `21c036c1740a0552124c79c6e2170cb61c5b016005671a6d5bc06cf642165431`, under `~/workspace/tools/wasm-tools-1.263.0/` |
| wasmtime | **wasmtime 51.0.0** | upstream `wasmtime-v51.0.0-x86_64-linux.tar.xz`, sha256 `a18020674fac4107f329bc9ed720ea9c88e4b30a61e8a527787bcce051123d32`, under `~/workspace/tools/wasmtime-51.0.0/` |
| wit-bindgen | **"=0.63.0"** | crates.io, the component crate's dependency pin (D5a) |

The tarball + sha256 sit beside each binary, by the
Binaryen precedent (`binaryen-version_133`). CI
downloads the same tarballs and builds the component
crate with the same target: an estimated **≈ +1
minute** on the Test job (audit §5), wired at the gate
phase and verified there.

**Not adopted: cargo-component** (D5a) — its latest
release carries zero assets, and the pipeline provably
does not need it. No other tool enters the pipeline.

**Workspace discipline.** The component crate is its
own workspace root (the facade pattern): the main
workspace's `Cargo.toml`/`Cargo.lock` and its standing
12-dependency set never move, and the component
crate's own lockfile lives in `packages/component/`.

---

## 4. The library component

### 4.1 Home and shape

`packages/component/` — a distribution artifact,
sibling to the packages it joins (npm, rust, vscode,
bundles), not a `crates/` build-time shell. Contents:
`Cargo.toml` (own workspace root), `Cargo.lock`,
`wit/world.wit`, `src/lib.rs` (the glue), `README.md`,
and the artifact of record (§4.4).

The source of truth is **the facade, wrapped**: the
glue is one line per export over `packages/rust`'s
functions, behind `wit-bindgen`'s generated `Guest`
trait. No function is re-implemented; the component's
bodies are the facade's bodies by construction.

Build (the whole pipeline):

```
cd packages/component
cargo build --target wasm32-wasip2 --release
```

### 4.2 The WIT world

```wit
package morphic:library@<lockstep>;

interface functions {
  // … the 30 rows of §4.3 …
}

world library {
  export functions;
}
```

The WIT package version **takes the lockstep version**
(R2): it stands at the tree's version until the Phase
161 stamp moves it to 1.6.0 with everything else, and
WIT semver governs breaking changes from there. The
interface id a host sees is
`morphic:library/functions@<lockstep>`.

### 4.3 The frozen surface — the 30 non-HO exports

The surface is the manifest's 35 exports minus the
five higher-order entries (§5). Types are EMBED_SCOPE
§3's WIT column as re-measured at Phase 158:

| Morphic | WIT |
|---|---|
| Int | `s64` |
| Bool | `bool` |
| Char | `char` (a scalar on both sides — measured exact, astral planes included) |
| String | `string` |
| List\<T\> | `list<T>` |
| Option\<T\> | `option<T>` |
| Result\<T, E\> | `result<T, E>` |

The JS-boundary losses EMBED_SCOPE §3 named (the
BigInt domain, the `T | null` flattening) do not exist
at this boundary, as §3 predicted. **The R10
code-point rule evaporates here**: R10 governed a host
whose `.length` counts UTF-16 units; the component
protocol passes whole strings, no host-visible length
crosses in either direction for any of the 35, and
Morphic's internal code-point counting stays the
facade's own business (audit §2.3). The freeze states
this so no later phase re-derives it.

The 30 rows themselves are the `functions` interface
in `packages/component/wit/world.wit`, pinned by
`tests/phase159_component.rs` against both the in-tree
WIT and the artifact's printed world.

### 4.4 The artifact of record, and the honesty surface

The built component is committed at
**`packages/component/morphic-library.wasm`** (R1) —
~96 KB, reproducible from the committed lockfile: a
fresh plain-cargo build reproduces it byte-for-byte,
pinned by test. Phase 159's measurement: **96,100 B**
against the §6 budget of 106,496 B.

**Tiers.** The WIT carries types, not tiers — that is
the boundary's honesty surface, named: of the 30
exports, 28 stand Tier-1 (`z3-verified`) and 2 stand
Tier-2 (`list-sort`, `list-binary-search`,
executed-exact). The tier rows live in the manifest
(`packages/rust/morphic.manifest.json`, mirrored in
the npm package), and their evidence is the PCC
bundles at `packages/bundles/`. The component README
states this in so many words: a host that needs a
tier reads the manifest; the component does not carry
claims it cannot re-state.

**The WASI wart (D3a).** The component imports std's
WASI p2 surface (`wasi:io/*`, `wasi:cli/*`,
`wasi:clocks/*` @0.2.12) whether or not the code does
I/O — fifteen interfaces a host must satisfy (or stub)
at instantiation. Both gate runners satisfy it
silently. The README names it; the strip is a
follow-up (§7).

---

## 5. The callback component (Phase 160's build, frozen here)

A **separate artifact**, because imports satisfy at
instantiation (D2a): a host that cannot supply the
callbacks must still be able to instantiate the
library component, and vice versa.

```wit
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

The import names and shapes are the audit's, as
prototyped (`map-apply` serves `list-map`, `pred`
serves `list-filter`/`list-all`/`list-any`, `combine`
serves `list-fold`). The five exports are the facade's
five Tier-1 higher-order entries with their callback
parameters carried by the imports — **no exported
signature anywhere carries a function-typed
parameter**; the WIT says what crosses, and what
crosses is values.

Home: a sibling crate beside the library component
(Phase 160 names it under `packages/component/`'s
roof or its own, and records the choice); its own
artifact of record; its own budget (§6). Its gate
instrument is the jco/Node harness (D4a): the three
callbacks supplied as JS modules through jco's map,
the five batteries answered exactly (audit §4.2). The
wasmtime-crate host stays an optional rehearsal,
never gate evidence.

---

## 6. Budgets

| Figure | Value | Basis |
|---|---|---|
| Library component | **≤ 106,496 B** (104 KiB) | Phase 158 measurement 96,096 B +10%, whole KiB (W4 formula) |
| Callback component | **≤ 90,112 B** (88 KiB) | Phase 158 measurement 81,778 B +10%, whole KiB |
| npm package | ≤ 76,800 B | standing, unmoved |
| Playground bundle | ≤ 771,072 B | standing, unmoved |

No existing budget moves. A measurement above a
component budget at the gate **fails the gate's
footprint clause** rather than re-basing the budget —
the standing discipline since Phase 138.

---

## 7. Exclusions and named follow-ons

- **The WASI strip** (D3a): a core-module +
  `wasm-tools component new` pipeline variant that
  drops the §4.4 import surface. A named follow-up,
  unmeasured and unpursued in 1.6.
- **The wasmtime-crate host** (D4a): an optional
  rehearsal a future phase may stand up for a second,
  non-JS host story; never gate evidence in 1.6.
- **User-spec components**: a component sibling of the
  `--package-from` verb (a user's own specs wrapped as
  a component) is **not in 1.6** — the milestone ships
  the library's components; the verb's component form
  is a named follow-on question for a later milestone.
- **A component registry**: 1.7's neighbouring
  question (the registry milestone, prelude bundles
  with install-time re-verification), not this one's.
- **The span build** and every other standing named
  item keep their own standings; 1.6 touches none.

---

## 8. The 1.6 gate

Run by Phase 161 on the final tree, clause by clause,
in the M1-gate style. A failing clause stops the merge.

1. **The library component answers.** Loaded by the
   pinned wasmtime from the shipped artifact, all 30
   non-HO exports answer their frozen batteries
   exactly, at the tiers the manifest states.
2. **The five cross honestly.** The callback
   component, exercised through a host supplying the
   three imports, answers the five HO batteries
   exactly; no exported signature anywhere carries a
   function-typed parameter — the callbacks are
   imports, and the WIT says so.
3. **The WIT is the contract.** `wasm-tools component
   wit` over both artifacts matches the frozen WIT
   (modulo the stamp's package version); every loss
   and boundary the surface carries — the WASI import
   surface included — is named in the WIT's docs or
   the package README.
4. **Footprint.** Both artifacts inside their budgets
   (§6); the main Cargo graph and lockfile unchanged;
   the build used the pinned tools and no others.
5. **Standings hold.** Suites green in the three
   frozen configurations (Phase 128 §9: default /
   bootstrap-codegen / `--no-default-features`) on the
   final tree; the npm and playground budgets stand;
   nothing published.

*Closing line (frozen verbatim from the audit's
draft):* "1.6 closes when the same proven functions
answer from a boundary no single language owns — every
export the library ships, the higher-order five
carried honestly as the imports they are, and no type
crossing that boundary with a loss nobody named."

---

## 9. Phase plan

| Phase | Work | Exit evidence |
|---|---|---|
| 159 | This freeze + the library component (§4): the crate at `packages/component/`, the 30 exports, the artifact of record | The freeze text; the artifact at 96,100 B ≤ 106,496 B; the wasmtime rehearsal: the batteries' 201 evaluable points exact through the shipped artifact; a fresh build reproducing the artifact byte-for-byte |
| 160 | The callback component (§5) + both runner rehearsals stood up as the gate's instruments | The callback artifact inside 90,112 B; the jco harness answering the five HO batteries exactly; the wasmtime battery over the library component re-run as an instrument, not a quote |
| 161 | The 1.6 gate (§8) + merge + release | The gate run clause by clause on the gate tree; the 1.6.0 stamp in lockstep (crate, packages, WIT version); PR merged on green CI; tag + GitHub Release; registry publishes left as the owner's acts |

---

## 10. Resolutions

**R1 — The artifact of record is the committed
`packages/component/morphic-library.wasm`.** *Basis:*
the .vsix/bundles precedent — a generated artifact the
gate measures is committed in its package home under a
name that says what it is. A wasm binary of ~96 KB is
committable, and the build is reproducible from the
committed lockfile (byte-for-byte, pinned), so the
commitment costs nothing in honesty: the artifact in
the tree is always one build away from verification.

**R2 — The WIT package version stands at the standing
lockstep until the stamp.** *Basis:* every other
surface (facade, npm package, VS Code extension)
carries the tree's version between releases and moves
at the stamp; the WIT follows the same discipline, so
at this freeze it reads `morphic:library@1.5.0` and
the Phase 161 stamp moves it to `@1.6.0`. The audit's
§2.2 wording — "takes the lockstep version at the
stamp" — is this discipline, stated from the stamp's
side.
