# 1.6 Scope Audit — the WASM Component Model, measured

**Phase 158** · branch `docs/disebar-158-component-audit`
from master @ `a57093b` (v1.5.0) · probes in
`tests/phase158_component_probes.rs` (5, `P158|...`
lines) · prototypes under `~/workspace/comp-proto-158/`
(scratch, paths cited per measurement).

This audit re-measures DISEBAR_AUDIT §2 — the one
Disebar component whose price was set by an *absent
pipeline* — against the v1.5.0 tree, now that the
toolchain question can be settled by installing things
and building with them. The owner has delegated this
campaign's decisions to Muse (2026-11-25); the D-list
in §8 is therefore written as recommendations with
grounds, for ratification in the open, exactly as the
Phase 127 G4 audit was.

**Headline: no premise fails, and the pipeline is
cheaper than §2 feared.** §2 priced L on the assumption
of "a second build pipeline" beside the playground's.
Measured: the component pipeline is **plain cargo** —
`cargo build --target wasm32-wasip2` over a wit-bindgen
crate produces a true component in ~40 s on this box.
There is no second build *system*; there are two pinned
*binaries* (wasm-tools, wasmtime CLI) that install from
upstream prebuilt tarballs in seconds, by the Binaryen
precedent. The one tool with no prebuilt —
cargo-component — turns out to be unnecessary, which
is this audit's only real finding (§1.3).

---

## 1. The toolchain gap, precisely

### 1.1 What the sandbox held at audit start

| Piece | Standing at start | Standing now |
|---|---|---|
| rustc | 1.100.0 | unchanged |
| `wasm32-unknown-unknown` | installed (playground) | unchanged |
| `wasm32-wasip2` | **not installed** | installed (§1.2) |
| `cargo-component` | absent | absent — not needed (§1.3) |
| `wasm-tools` | absent | **1.263.0**, pinned (§1.4) |
| `wasmtime` CLI | absent | **51.0.0**, pinned (§1.4) |
| `wit-bindgen` | nowhere in any lockfile | resolved **0.63.0** in the prototypes |
| node | v24.22.1 | unchanged — not a component host (§4.3) |

The playground artifact itself re-verified as §2 left
it: `wasm-tools print` on `playground/pkg/`'s module
opens `(module` — a core module (752,223 B as rebuilt
at Phase 157). Nothing in-tree converts it; no `.wit`
file exists anywhere in the tree (probe
`p158_pipeline_is_core_module_and_wit_absent`).

### 1.2 The target: `rustup target add wasm32-wasip2`, 37 s

One command, one download (`rust-std` for the target),
**37 s** wall on this box. The probe
`p158_wasip2_target_installed` pins the standing via
the sysroot's rustlib listing. This is the entire
"missing target" story: S, once, durable in
`~/.rustup` beside the standing targets.

### 1.3 cargo-component: no prebuilt exists — and none is needed

Measured against the upstream release (v0.21.1, the
latest): **the release carries zero assets.** The only
install route is `cargo install cargo-component` —
a from-source build of a tool whose dependency closure
includes cargo itself as a library (hundreds of
crates). That build was *not* run: it is bounded below
by every other large build on this box and would be
measured in tens of minutes, and §1.5's prototypes make
it moot —

**Plain cargo builds the component.** A cdylib crate
depending on `wit-bindgen` 0.63.0 and the facade,
built with `cargo build --target wasm32-wasip2
--release`, emits a file whose header is
`00 61 73 6d 0d 00 01 00` — component version `0x0d`,
layer `0x01` — which `wasm-tools validate` accepts and
`wasm-tools component wit` prints. cargo-component's
remaining value (WIT dependency fetching, `cargo
component` ergonomics) is convenience over a pipeline
that already works; adopting it would pin a
tens-of-minutes from-source build into every fresh
environment for that convenience. **Recommendation:
not adopted (D5).**

### 1.4 The two pinned binaries (Binaryen precedent, verbatim)

Installed under `~/workspace/tools/`, tarball +
sha256 recorded beside each, exactly as
`binaryen-version_133` stands:

- `wasm-tools-1.263.0/` — upstream
  `wasm-tools-1.263.0-x86_64-linux.tar.gz`
  (5,976,062 B; download ≈5 s), binary reports
  `wasm-tools 1.263.0`. sha256
  `21c036c1…6431165431`.
- `wasmtime-51.0.0/` — upstream
  `wasmtime-v51.0.0-x86_64-linux.tar.xz`
  (download ≈5 s), binary reports `wasmtime 51.0.0`.
  sha256 `a1802067…2e051123d32`.

Probe `p158_pinned_component_tools` pins both versions
where the binaries are checkable.

### 1.5 The prototype builds (the pipeline, exercised)

Three scratch crates at `~/workspace/comp-proto-158/`,
each depending on the facade by path + `wit-bindgen`:

| Prototype | Contents | Build (release, this box) | Artifact |
|---|---|---|---|
| `a` | 5 exports (one per type row) | 41 s | 81,959 B |
| `full` | all 30 non-HO exports | ~35 s (incremental tree) | **96,096 B** (31,752 B gzip) |
| `callback` | 3 imports + the 5 HO exports | 34 s | **81,778 B** |
| `b` | 1 import + 1 HO export (first light) | 35 s | 78,793 B |

The fixed cost dominates: ~79 KB of every artifact is
the wasip2 std runtime + glue; the marginal cost is
≈470 B per export. (One recorded sandbox lesson,
re-learned: the first `full` build died with
`No space left on device` because the scratch sat in
`/tmp` — the 512 MB tmpfs. The scratch moved to the
home disk; nothing about the pipeline was at fault.)

---

## 2. The artifact shape

### 2.1 Source of truth: the facade, wrapped — not close

The component wraps **`packages/rust`** (the facade:
zero dependencies, the 35 real functions, already the
gated Rust surface). Every prototype above is the
facade's functions behind wit-bindgen glue, one line
per export. The alternative — a fresh crate over the
compiler — would drag the compiler (and Z3's build)
into a wasm target it has never built for, to produce
the same functions the facade already ships and the
gate already crowned. The facade's own workspace
pattern carries the packaging: the component crate is
its **own workspace root**, so the main tree's
`Cargo.toml`/`Cargo.lock` never move (D1).

Home: **`packages/component/`** — a distribution
artifact, sibling to the packages it joins (npm, rust,
vscode, bundles), not a `crates/` build-time shell
like `crates/morphic-wasm`.

### 2.2 The WIT surface, as prototyped

```wit
package morphic:library@0.1.0;

interface functions {
  int-clamp: func(x: s64, lo: s64, hi: s64) -> s64;
  list-sort: func(xs: list<s64>) -> list<s64>;
  char-to-digit: func(c: char) -> option<s64>;
  option-ok-or: func(o: option<s64>, e: string)
      -> result<s64, string>;
  // … all 30 non-HO exports, one row per manifest entry
}

world library {
  export functions;
}
```

`wasm-tools component wit` on the built artifact
prints the export as `morphic:library/functions@0.1.0`
— the WIT package version rides in the interface id,
so the freeze must version the WIT deliberately:
**the WIT package version takes the lockstep version
at the stamp** (`morphic:library@1.6.0`), and WIT
semver governs breaking changes from there (part of
D1's recommendation).

### 2.3 EMBED_SCOPE §3, re-measured row by row

§3's WIT column transfers — with one row amended and
one rule discharged:

- **Int→s64, Bool→bool, Char→char, String→string,
  List→list, Option→option, Result→result**: all
  exercised live through the runner (§4.1), values
  exact, nesting included. The JS-boundary losses §3
  named (BigInt domain, `T | null` flattening) do not
  exist at this boundary, as §3 predicted.
- **Char is a scalar on both sides, measured:**
  `char-to-digit('٥')` (Arabic-Indic five, U+0665)
  crosses and returns `none` — carried exactly,
  correctly not an ASCII digit; `char-is-alpha('é')`
  returns `false` under the entry's ASCII-only
  contract. No encoding loss is available to this
  boundary.
- **The R10 code-point rule has no component-side
  analogue.** R10 governed a host (JS) whose
  `.length` counts UTF-16 units. The component
  protocol passes whole strings; no host-visible
  length crosses in either direction for any of the
  35 (the one length-shaped entry, `string_is_empty`,
  returns a bool). Morphic's internal code-point
  counting is the facade's own business and does not
  change. The rule evaporates at this boundary —
  recorded here so the freeze need not re-derive it.
- **The Function-types row is the amendment.** §3
  states "No mapping exists", written when no export
  was higher-order (probe
  `p158_embed_scope_wit_column_and_the_function_row`
  pins the text). Since G4, five exports carry
  `fn(...)` parameters (probe
  `p158_facade_surface_inventory`: exactly
  `list_map`, `list_filter`, `list_fold`, `list_all`,
  `list_any` — all five Tier-1). §3 is a frozen text;
  the 1.6 freeze takes the fork in its own document
  (§3 below), it does not reinterpret §3.

### 2.4 The WASI import surface (a named wart, priced at D3)

Every prototype component **imports the WASI p2
surface** (`wasi:io/*`, `wasi:cli/*`,
`wasi:clocks/*` @0.2.12) — std's wasip2 runtime links
it whether or not the code does I/O. Consequences,
measured: wasmtime's default WASI satisfies it
silently (§4.1's runs configure nothing); jco maps it
to the preview2-shim (§4.2). A host that provides no
WASI at all cannot instantiate these components until
it stubs fifteen interfaces it will never call.
Stripping the surface means a second pipeline variant
(core-module build + `wasm-tools component new`),
unmeasured and unpursued here. D3 recommends accepting
the surface for 1.6 with the strip as a named
follow-up; the wart is stated in the open either way.

---

## 3. The G4 fork, priced three ways

WIT interfaces have no function-typed values. The five
HO exports therefore cannot cross as they stand. The
options, honestly decomposed:

**(i) Exclude the five.** The component ships the 30.
Price: zero extra. Cost: the component surface is not
the library surface — five Tier-1 entries, the ones
G4 was built for, absent from the milestone whose
theme is "the same proven functions". The absence
would need naming in the WIT package forever.

**(ii) Host-imported callbacks (measured, §1.5
`callback`).** The world imports the three callback
shapes the five entries need —

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

— and exports the five over them. The prototype's WIT
prints exactly this; §4.2 exercises it end-to-end.
**The decisive structural fact:** a component's
imports are satisfied at *instantiation*, for the
whole component. Folding these imports into the
library component would make **every** consumer —
including one that only wants `int_clamp` — unable to
instantiate until it supplies three callbacks. So
(ii) ships as a **second component** beside the
import-free library component: two artifacts, each
instantiable by exactly the hosts that can use it.
Price: S–M (one more small crate over the same
facade; the glue is one line per export, as
prototyped).

**(iii) Baked monomorphic instantiations.** Export
`list-map-succ`, `list-map-double`, … with the
function compiled in. This is sometimes what
"monomorphic instantiation" means, so it is priced
rather than waved at: the instantiations are **not
the five entries** — each is a new export with its
own, narrower contract, and the set is unbounded
(every function a user might pass is another export).
Shipping the batteries' lambdas as the set would
certify the battery, not the entry — the R6 lesson
from Phase 143 (Tier-1 certifies the frozen contract,
never a gloss of it). Rejected on those grounds, not
on price.

**Recommendation: (ii)** — the five cross as what
they are: functions whose callback the host supplies,
named as imports in the WIT itself (D2).

---

## 4. The host/runner story

The gate needs a runner that loads each component and
calls its exports battery-exact. Three candidates,
measured:

### 4.1 wasmtime CLI 51.0.0 — complete for the library component

`wasmtime run --invoke '<name>(<args>)'` (the
call-expression syntax; the bare-name form of older
versions is gone) against prototype `a`/`full`:

```
int-clamp(15, 0, 10)        → 10
char-to-digit('7')          → some(7)
char-to-digit('x')          → none
string-is-empty("")         → true
option-ok-or(none, "e1")    → err("e1")
option-ok-or(some(5), "e1") → ok(5)
list-sort([3, 1, 2])        → [1, 2, 3]
list-binary-search([1,3,5], 3) → some(1)
range(2, 5)                 → [2, 3, 4]
```

Every type row, battery-correct, zero configuration
(the default WASI satisfies §2.4's imports). The CLI
is a ~35 MB pinned binary that installs in seconds.
**It is the gate instrument for the library
component.**

Its limit is equally measured: against the callback
component it stops at instantiation —

```
component imports function `apply`, but a matching
implementation was not found in the linker
```

The CLI supplies WASI and nothing else. It cannot
exercise an imported callback, by design.

### 4.2 jco + Node — the callback component, exercised

`@bytecodealliance/jco` 1.42.0 (npm; install 71 s
wall, mostly network) transpiles the callback
component in ~6 s; with the three callbacks supplied
as JS modules through jco's `--map` (plus the
preview2-shim carrying WASI), Node runs:

```
list-map([1,2,3])    = 11,12,13     (callback x => x + 10n)
list-fold([1,2,3],100) = 106        (callback acc + x)
list-filter([1,2,3]) = 2,3          (callback x => x > 1n)
```

All battery-correct. The harness is an S: three
one-line callback modules and an import map. **This
is the gate instrument for the callback component**
(D4) — a JS host, which is also honest evidence the
component model reaches beyond Rust hosts.

### 4.3 The other two candidates

- **Node natively: not a component host.**
  `WebAssembly.compile` on the component refuses —
  *"expected version 01 00 00 00, found 0d 00 01 00"* —
  and the `WebAssembly` object in v24.22.1 carries no
  component API. Node reaches components only through
  jco (§4.2).
- **A Rust host on the `wasmtime` crate:** the native
  route — one ~40-line host (bindgen over the same
  WIT, the three imports as Rust closures) both
  instantiates and calls. Its price is the compile,
  and the compile is this audit's most expensive
  measurement: the `wasmtime` crate (v51, to match
  the CLI pin) pulls a several-hundred-crate closure
  (cranelift included), and a release build on this
  box **did not finish in 10.5 minutes** — 143 crates
  compiled when a runtime restart killed the build,
  under shared load (load average ~7), so the figure
  is a lower bound with the load stated, not a clean
  wall time. Even read generously, it is an order of
  magnitude beyond every other step in the campaign
  (all of which measure in seconds). It buys a
  second, non-JS host rehearsal; it is not needed
  for gate evidence once §4.1 + §4.2 stand. D4 names
  it as the optional rehearsal, not the instrument.

---

## 5. Budgets, versioning, CI

**Sizes (measured, §1.5).** Library component
96,096 B; callback component 81,778 B. House formula
(measurement +10%, rounded up to a whole KiB, the W4
precedent), per artifact:

- **library component ≤ 106,496 B** (104 KiB)
- **callback component ≤ 90,112 B** (88 KiB)

For scale: the playground's core module stands at
752,223 B against its 771,072 B budget; the whole
component surface is ~24% of that, because the
facade's functions are small and the runtime is the
cost. No existing budget moves: the npm figure
(76,800 B) and the playground figure (771,072 B) are
untouched by this milestone (D6).

**Versioning.** Lockstep **1.6.0** — crate, packages,
release, and the WIT package version (§2.2). The
component artifacts are new files in the release's
orbit, not a registry publish: no registry takes a
bare component today, so 1.6's distribution is the
repo + the GitHub release assets (the freeze states
this; a component registry is 1.7's neighbouring
question, not this milestone's).

**CI.** The component build is one more release build
per artifact (~35–45 s on this throttled box; CI
runners are faster) plus two prebuilt-tool downloads
(~10 s measured here). Estimate **≈ +1 minute** on
the Test job. The gate's runner steps (wasmtime CLI
invocations, the jco harness) are seconds each.

---

## 6. Recommended shape

One milestone **1.6**, one gate, lockstep 1.6.0,
campaign ≈ **L** (the pipeline is cheaper than §2
priced; the WIT/fork/runner design work is the
substance):

- **159** — freeze (`COMPONENT_SCOPE.md`) + the
  library component (crate at `packages/component/`,
  the 30 exports, WIT as §2.2, budgets as §5).
- **160** — the callback component (the five over
  three imports) + both runner rehearsals stood up
  as the gate's instruments (wasmtime CLI battery
  over the library component; the jco harness over
  the callback component).
- **161** — gate + merge + v1.6.0.

## 7. Gate sketch (5 clauses)

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
   (§5); the main Cargo graph and lockfile unchanged;
   the build used the pinned tools and no others.
5. **Standings hold.** Suites green in the three
   frozen configurations on the final tree; the npm
   and playground budgets stand; nothing published.

**Closing line, drafted:** *"1.6 closes when the same
proven functions answer from a boundary no single
language owns — every export the library ships, the
higher-order five carried honestly as the imports
they are, and no type crossing that boundary with a
loss nobody named."*

## 8. The D-list (recommendations, for ratification)

- **D1 — source of truth + home.** (a) Wrap the
  facade from a new crate at `packages/component/`,
  its own workspace root; the WIT package is
  `morphic:library`, versioned at the lockstep.
  *Grounds: §2.1 — the facade is the gated surface;
  the compiler route drags Z3 into a wasm target for
  the same functions.*
- **D2 — the G4 fork.** (a) Host-imported callbacks,
  shipped as a **second component** (`library` +
  `library-callbacks` worlds); the library component
  stays import-free but for §2.4's surface.
  *Grounds: §3 — instantiation is whole-component;
  exclusion amputates five Tier-1 entries; baked
  instantiations certify batteries, not entries.*
- **D3 — the WASI import surface.** (a) Accept std's
  wasip2 imports for 1.6, named in the package docs;
  the strip (core-module + `component new` pipeline)
  is a named follow-up. *Grounds: §2.4 — both gate
  runners satisfy the surface today; the strip is a
  second pipeline bought for hosts not yet named.*
- **D4 — the runners.** (a) wasmtime CLI (pinned) as
  the library component's gate instrument; the
  jco/Node harness as the callback component's; the
  wasmtime-crate host as an optional rehearsal, not
  gate evidence. *Grounds: §4 — both instruments are
  measured end-to-end; the crate host's compile is
  the most expensive object in the campaign and buys
  a second host, not a stronger verdict.*
- **D5 — toolchain pins.** (a) wasm-tools 1.263.0 +
  wasmtime 51.0.0 as prebuilt pins under
  `~/workspace/tools/` (CI downloads the same
  tarballs); `wit-bindgen` pinned `=0.63.0` in the
  component crate; `wasm32-wasip2` via rustup.
  cargo-component **not** adopted. *Grounds: §1 —
  every pin is installed and exercised; the one
  from-source tool is the one the pipeline provably
  does not need.*
- **D6 — budgets.** (a) Per-artifact: library
  ≤ 106,496 B, callbacks ≤ 90,112 B (measured +10%,
  whole KiB). Standing budgets unmoved. *Grounds:
  §5's measurements and the W4 formula.*
- **D7 — shape.** (a) One milestone 1.6, phases
  159–161 as §6, lockstep 1.6.0, one gate (§7).
  *Grounds: the components share one WIT, one facade,
  and one honesty story; splitting them would gate
  the fork twice.*

## 9. Stopped / findings

- **cargo-component's empty release (§1.3)** — a
  finding that reshaped the toolchain, not a blocker:
  the pipeline routes around it entirely.
- **The `/tmp` tmpfs overflow (§1.5)** — a scratch
  placement error, corrected to the home disk; the
  standing sandbox rule, re-recorded.
- Nothing else stopped. No premise failed; no price
  was inflated to protect the plan — the campaign
  came in *under* its §2 estimate, and the difference
  is the plain-cargo measurement, not optimism.
