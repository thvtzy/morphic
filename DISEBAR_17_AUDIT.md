# DISEBAR 1.7 AUDIT — the registry, meaning (a), measured (Phase 162)

**What this is.** A scope audit of **1.7 — the registry**,
the Disebar rung's last named component, in the meaning
the rung re-founded it at Phase 148 §3 and froze at
DISEBAR_SCOPE **D5a**: a **registry of prelude bundles
with install-time re-verification**. Not a module system
(Phase 148 measured imports resolving to nothing and
priced the real thing XL — a language milestone, named
there as explicitly *not* this rung), and not an index
page standing alone (148's meaning (c), which folds into
(a) as its listing surface). A bundle is the 1.4 PCC
artifact — one function's spec + body in `morphic-ir/1`
+ claimed verdict + battery + crown route, produced by
the checked route. The registry lets a consumer *fetch*
bundles made by others and *install* them as prelude
entries their own specs can call by plain name — and
the install step re-runs the bundle's proof obligation
before anything is trusted.

**Method.** The tree as it stands at v1.6.0 (master
`5b4b8b0`): code reading where reading settles it, probes
where a claim needs one — 7 probes in
`tests/phase162_registry_probes.rs`, every claim below
carrying its `P162|` line or its file/line evidence.
Audit only: no production code changed.

**Headline.** **No premise fails, and the rung is
cheaper at every joint than its Phase 148 price (L)
suggested — because 1.4 built exactly the machinery it
waits on.** The bundle format is complete and carries a
user function's evidence today (probed: the Phase 154
verb's own admission winners bundle and re-verify with
**zero new production code**). The consumer verb an
install step calls exists and refuses by name in all
the standing shapes. The prelude the bundles must join
is a **two-function funnel** — every consumer of the
library (typechecker, SpecIndex, codegen) reads through
`stdlib::{prelude_specs, shipped_body}` — so "installed
bundles join the prelude" is one new layer at one seam,
not a compiler tour. What is genuinely missing is
small and enumerable: a production CLI arm, a
distribution index, four client verbs, a receipt, a
hash function, and the discovery layer at the funnel.
Two measurements correct the record along the way: the
Phase 150 header's claim that 1.7 reuses
`verify_library()` is **half right** (its signature
takes no bundle — the reusable unit is the per-bundle
verb), and the bundle format's **self-containment
boundary** is now measured at production, not inferred.

Suites after the audit (probes only): **951/0/29 ·
951/0/29 · 807/0/22** in the three frozen configurations
(baselines 944/944/801 + the 7/7/6 probe teeth — the
sibling-boundary probe is Z3-gated, as its route is).

---

## 1. The machinery, inventoried against the rung's needs

**The bundle format — complete, no build owed.** One
JSON file per function (`packages/bundles/`, 35 files):
`format` + `bundleVersion: 1`, `entry`, `tier`,
`verdictBasis`, `verdict`, `crownRoute`, `battery
{total, evaluable, passed}`, the spec in the AST's own
serialization, the body in `morphic-ir/1`
(`src/pcc.rs`). Measured masses: `P162|bundles|count=35
total=267156 min=2539 max=13802 mean=7633` — the whole
shipped catalog is a quarter of a megabyte; a
third-party bundle is a 2.5–14 KB file.

**Consumption — the install's core call already
exists.** `pcc::verify_bundle_text(text)` re-judges
(spec, body) through the named crown route and compares
verdict + every battery count; its refusals are named
and distinct per shape (unsupported version, incoherent
claim triple, unknown route, IR-reader declines, claim
mismatch against a shipped row, a body the route does
not crown, a moved battery count), and a Tier-1 claim
in a no-Z3 build is refused by name — it stands
unexamined, never passed. Probed on the checked-in file
itself: `P162|verify-file|int_abs ok: … the claim
reproduces — tier-1 / z3-verified / Verified via the
pipeline route`. An install step is: fetch bytes → this
call → write on Ok. Nothing in that sentence is new
machinery.

**The `verify_library` correction.** `src/pcc.rs`'s
header says the 1.7 install-time re-verification
"reuses this function and the bundle verbs". Measured
against the signature: `pub fn verify_library() ->
Result<Vec<EntryStanding>, String>` takes **no
argument** and iterates `bindings::stdlib_inputs()`
only; its one product call site is the CLI's
`verify --library` arm (`src/main.rs:435`). It cannot
verify an arbitrary bundle, and no parameter could be
added that would make it the install's verb without
changing what it is. What 1.7 actually reuses is
`verify_bundle_text` (per bundle) — and what it adds is
a *sibling*: a `verify --installed`-shaped sweep that
iterates an install directory calling the per-bundle
verb, exactly as `verify_library` iterates the shipped
inputs calling the crown checks. The self-check itself,
probed: `P162|verify-library|standings=35
reproduced=35` (35/35 in a Z3 build, ~31 s solo on this
box — the price datum the CI clause in §6 cites).

**Production — user bundles exist; the verb doesn't.**
`pcc::produce_bundle(&BindingInput)` is generic over
any (spec, body, tier, route) and *measures* its claim
by re-running the crown before writing anything; Phase
150 pinned hand-crowned user bundles through both
routes. What Phase 162 adds: the **Phase 154 verb's own
winners** bundle through it. `assemble_user_package`
(`src/codegen/package.rs:821`) runs the seeded admission
per spec and builds a `Vec<BindingInput>` (line 842) —
then generates JS bindings from it and **discards it**;
`UserPackageAssembly` returns files + standings only,
and no CLI arm calls `produce_bundle` on a user input
(`build --bundles` refuses any input file by name,
`src/main.rs:350`). Probed, mirroring the verb's own
API call and admission rule on its own exhibit module:
`P162|produce|is_widget_code tier=tier-1
basis=z3-verified verdict=Verified battery=5/5/5` and
`P162|produce|weights_sorted tier=tier-2
basis=executed-exact verdict=ExecutedExact
battery=5/5/5`, both re-verifying through
`verify_bundle_text` (`P162|seam|…`). **The gap is
exactly: one CLI arm (`build --bundles-from`, sibling
of `--package-from`, sharing its admission loop) + the
assembly retaining its inputs. Price: S.**

**The format's boundary, measured — self-containment.**
Production re-judges (spec, body) *standalone against
the prelude*: the pipeline route reconstructs the
spec's module from ASTs (`pcc::module_functions`) — for
a non-shipped spec, a one-spec module. A spec whose
**constraints name a sibling user spec** therefore
cannot be bundled: probed with a two-spec module whose
`quadruple_it` is stated through `double_it` — in
context the seeded run crowns it Verified; production
refuses by name: `P162|boundary|sibling-naming spec
refused at production: quadruple_it: the bundle's spec
does not check: … Called undefined function:
'double_it'`. The Tier-2 route reaches the same wall by
its own mechanism (`check_tier2_body` judges the body
over the prelude index only; a body calling an unknown
name is not executed-exact on its battery). **A bundle
is one prelude-self-contained function**: its spec and
body may name builtins and prelude entries, nothing
else. This is the catalog's contract, inherited from
the format as shipped — not a defect 1.7 must fix, and
not one it may silently widen (a bundle carrying its
module context is a format-v2 conversation, named in
§7's deferrals).

**The prelude funnel — the merge the registry extends.**
The stdlib is compiled in: module sources via
`include_str!` (`src/stdlib.rs:85`), bodies via a
transcribed registry behind `shipped_body` (line 1395).
Every consumer reads through two functions:
`typeck::check` registers `prelude_specs()` signatures
before the module's own (`src/spec/typeck.rs:44`);
`SpecIndex::merge_prelude` inserts each prelude spec +
its shipped body, skipping names the module defines
(`src/synthesis/engine.rs`, `merge_prelude`);
`codegen::generate` looks used entries up through
`shipped_body` / `library_entry_names`
(`src/codegen/mod.rs:118,387`). Probed:
`P162|funnel|prelude_specs=35 shipped_bodies=35`.
**What an installed layer costs**: one more source
behind that funnel — an `installed` set (spec parsed
from the bundle's own serialization, body parsed from
its IR text — both parsers exist) read once per process
from a directory, merged *between* stdlib and user at
the same two seams. Nothing else in the compiler needs
to know the directory exists: the API, MCP, LSP and CLI
all inherit installed entries exactly as they inherit
the stdlib today, because all of them build on the same
funnel. The browser build is the named exception: its
prelude arrives as a runtime asset and it has no
filesystem — installed bundles are a **native-only**
surface in 1.7, gated where the funnel reads the
directory (the `wasm-playground` cfg discipline the
stdlib already uses).

**Discovery today: none.** No product code reads a
per-user directory, a config file, or a registry-shaped
environment variable (grep over `src/`: the one `HOME`
use is a rustc-path fallback in a compile helper).
Probed by planting both: a directory holding a bundle
file for a fake entry, named by `MORPHIC_REGISTRY_DIR`
and `MORPHIC_HOME` — `P162|discovery|planted dir + env
vars move nothing: the prelude is exactly the
compiled-in library`. The discovery layer is greenfield
— but it is one layer, at one funnel, behind a OnceLock
(the stdlib's own pattern, `library_module`).

---

## 2. Distribution — where bundles live for fetching

The project's actual scale decides this: one
maintainer, GitHub-centric, no accounts anywhere, and a
trust model that makes the host's identity nearly
irrelevant (install re-verifies — a hostile mirror can
serve a *wrong* bundle, never a *trusted* one; §4's
hash binds even that). Three candidates, priced:

**(i) A directory in this repo + a static JSON index,
fetched over HTTPS raw.** `registry/index.json` +
`registry/bundles/<entry>.bundle.json`, served by
`raw.githubusercontent.com` at any ref a consumer
names. **Cost: S** — files and a format, no workflow
changes, no new infrastructure; it works today.
Publishing = a pull request to this repo (the only
review flow that exists at this scale — curation is a
*listing* decision, never a trust decision). Versioning
lives in the index: each entry pins its catalog version
+ sha256 + file path, so a catalog update is one commit
and a consumer's install is reproducible against the
index they fetched. Offline: the fetch fails by name
and the installed set is untouched; a local index path
(`--index <path>`) makes the whole flow offline-capable
and is the gate's hermetic form.

**(ii) GitHub release assets.** Rejected on shape:
third parties cannot attach assets to the maintainer's
releases, so publishing collapses to maintainer
releases only — the open-post half of the rung dies;
and 35+ loose files per release is the wrong container
for a catalog that changes one entry at a time.

**(iii) A Pages-hosted static index.** The same files
as (i) behind the playground's Pages site. Rejected
for 1.7 on cost/benefit: the Pages workflow publishes
`playground/` only (`.github/workflows/pages.yml`), so
this buys a second publishing surface to maintain in
exchange for a prettier URL — at one-maintainer scale,
nothing. Named as the index's possible future home:
the format is host-agnostic, so moving later is a URL
change, not a redesign.

**The fetch client.** `reqwest` 0.12 (blocking + json)
is already in the tree and already in the **default**
build's graph (feature `llm-client` ∈ `default`,
`Cargo.toml`); the fetch verb needs **no new HTTP
dependency** — the work is feature plumbing so the
library-side registry module can use it, plus the
decline shape where it is absent: in the
`--no-default-features` build there is no HTTP client,
so a remote fetch **declines by name** there while a
local `--index <path>` install works — the honest
posture, mirroring the no-Z3 Tier-1 refusal. **Price:
S.**

**The index format** (frozen at 163, sketched here):
one JSON object, `format: "morphic-registry-index"`,
`indexVersion: 1`, and `entries`: `{entry, version,
file, sha256, bytes, tier, verdictBasis}`. Two
disciplines, stated now: the tier/basis fields are
**listing conveniences — claims**, duplicated from the
bundle so a human can browse; the bundle is the
evidence and the install re-measures it (a lying index
row is caught at install, by the bundle's own
reproduction). And **version is the index's fact**:
the bundle format carries no version, author, or
provenance field at all (measured — its fields are the
nine of §1), so provenance lives at the catalog layer,
exactly as the npm manifest already separates
`packageVersion` (the author's) from `morphicVersion`
(the pipeline's). No bundle-format bump is needed or
recommended for 1.7.

---

## 3. The client surface

**Verbs.** One new command family, `morphic registry`,
with four actions — a deliberate, contained departure
from the CLI's flat single-word verbs, taken because
the family is four verbs on day one and the flat
alternatives (`registry-add`, `bundle-add`, …) spend
the top level on one rung's plumbing:

- `registry add <entry> [--index <url-or-path>]` — the
  install flow of §4.
- `registry list` — installed entries with their
  receipt standings (name, version, re-measured tier /
  verdict, route, sha) + the catalog's claim beside
  them where they differ (they must not; if they do,
  the receipt is the truth and the difference is
  printed, not hidden).
- `registry remove <entry>` — deletes the bundle file
  + receipt row. Nothing else references the install
  dir, so removal is complete by construction: the name
  declines again from the next process on (pinned at
  the gate).
- `registry verify` — the `verify_library` sibling the
  correction in §1 names: iterate the install dir,
  re-run `verify_bundle_text` per bundle, report
  per-entry standings. This is also the CI instrument
  of §6.

**Production.** `morphic build --bundles-from
<file.morph>` (D3's recommendation): the
`--package-from` admission loop, emitting one
`<entry>.bundle.json` per admitted spec through
`produce_bundle`'s checked route, standings printed as
the verb prints them today; a refused spec refuses the
assembly with nothing written (the standing
discipline). A producer publishes by opening a PR that
adds their bundles + index rows to `registry/` — the
index row's sha256 is computed at catalogue time; CI
re-verifies it (§6).

**Where installs live.** `~/.morphic/registry/` —
outside any repo, per-user — overridable by
`MORPHIC_REGISTRY_DIR` (the name the discovery probe
planted; it becomes real here). Flat contents: one
`<entry>.bundle.json` per installed entry and one
`installed.json` **receipt ledger**. No other state.

**How the compiler learns.** Automatic discovery at the
funnel (D7's recommendation): the installed layer reads
the directory once per process (the stdlib's OnceLock
pattern) and merges at the two seams of §1. The
alternative — a per-invocation flag — would make
installed entries second-class (every shell would have
to thread it; a spec that calls an installed entry
would check under one invocation and decline under
another). Automatic discovery makes `check`,
`synthesize`, the MCP tools and the LSP agree, because
they already agree on the prelude. The cost is named:
every native process pays one directory read + parse
of the installed set (at the seed scale: ≤ 37 small
files; the build phase measures the prelude-access
latency it adds and the freeze states a cap if one is
needed).

---

## 4. Install-time re-verification — the honesty core

The flow, each step's failure writing **nothing**:

1. **Resolve.** Fetch the index (HTTPS) or read it
   (local path); find the entry row. Absent → decline
   by name.
2. **Fetch + bind.** Fetch the bundle bytes; check
   their **sha256 against the index row**. Mismatch →
   refuse by name. The hash is load-bearing, and its
   grounds are worth stating: re-verification alone
   proves the bytes are *a* proven function — a
   substituted bundle carrying a *different spec* for
   the same name would also re-verify, while silently
   changing the contract the name denotes. Only the
   hash binds *name → the catalogued bytes*, which is
   what makes the index's curation enforceable. The
   tree today has **no SHA-256 anywhere** (grep:
   `Cargo.toml` + `src/` — none), so this is the
   milestone's **one new dependency**: `sha2`, small
   and single-purpose, gated to the native registry
   surface and absent from the browser profile's graph.
   **Price: S.**
3. **Parse + re-verify.** `verify_bundle_text` on the
   fetched bytes. Every standing refusal is the
   install's refusal, verbatim: unsupported bundle
   version, incoherent claim, a body the route does not
   crown, a moved battery count — and in a no-Z3 build,
   a Tier-1 bundle refused by name (it stands
   unexamined; a Tier-2 bundle installs on executed
   evidence). Probed shape, on a user bundle with its
   verdict flipped: `P162|tamper|flipped verdict
   refused: incoherent claim: …`.
4. **Collision checks** (§5): a stdlib name → refuse by
   name; an already-installed name → refuse by name
   (the update path is `remove` + `add`, so the ledger
   stays an honest record of what was measured when).
5. **Write + receipt.** The bundle file lands in the
   install dir; the ledger gains a row recording what
   the **re-run measured**, not what the index claimed:
   entry, catalog version, sha256, tier + verdictBasis
   + verdict *as reproduced*, crown route, battery as
   reproduced, the index it came from, and the install
   date. `registry list` reads the ledger; `registry
   verify` re-measures against it.

**The stranger test this enables** — the gate's
centrepiece (§8): a bundle produced on machine A from a
user module is catalogued, fetched by a clean machine B
(empty install dir), installed with its proof re-run at
install, and then called **by name** from B's own fresh
spec — which checks against the merged prelude, is
judged through the index (the installed body is the
name's denotation), and generates code that computes
battery-exact. Fetching is transport; the install is
the verification.

---

## 5. Naming and collision discipline

**The namespace is flat, and that is a language fact,
not a registry choice.** Calls in Morphic are bare
names resolved against one flat prelude (Phase 148's
measurement); the bundle's `entry` is a bare function
name; no scoped form (`@author/name`) can be called by
any syntax the 1.x language has. So the catalog is a
flat namespace with **first-come curation**: a name
enters the index once, by PR; disputes are the
curator's, exactly as a flat package namespace works
elsewhere. Entry *versions* are semver strings carried
by the index row (the bundle is versionless, §2); a
new version of an entry replaces its row + file in one
commit, and receipts pin the version + sha each install
actually measured, so an updated catalog never rewrites
history on an installed machine.

**Precedence, derived from the measured merge.** Today:
user > stdlib (typeck registers prelude sigs first and
the module's own overwrite; `merge_prelude` skips
module names entirely — probed: `P162|collision|user
definition wins: the prelude entry of a colliding name
is not merged`, and the colliding name does **not**
denote the shipped body). The registry extends this to
three layers with one rule: **user > stdlib >
installed**. An installed entry merges only into names
neither the module nor the stdlib holds. Two
consequences, both deliberate:

- **An installed bundle can never shadow a stdlib
  name** — not by accident (the merge skips it) and not
  by malice (a hand-planted file in the install dir is
  equally skipped). Install *also* refuses stdlib names
  at the door, by name, so the user hears about the
  collision instead of silently installing a dead entry.
  Defense in depth, both layers named.
- **A user's own definition shadows an installed
  entry**, by the same rule that already shadows the
  stdlib: their module, their name, their call. The
  installed entry is a default, never an override.

---

## 6. Seed content, budgets, versioning, CI

**The seed catalog.** The registry opens with **the 35
stdlib bundles themselves** — the checked-in
`packages/bundles/` files, copied into `registry/` and
indexed (catalog version: the 1.7.0 lockstep, stated in
the freeze as a *cataloguing* fact — the bundles record
no version and have regenerated byte-identical at every
stamp since 1.4, so the row claims when they were
catalogued, nothing about the functions) — **plus one
worked third-party-style example**: the Phase 154
widget module's `is_widget_code` (Tier-1) and
`weights_sorted` (Tier-2), produced by the new
`--bundles-from` verb during the build phase and
catalogued under their own names (neither collides with
the stdlib). The example is the catalog's proof that
the door is open to non-stdlib functions — and its
production is the verb's own rehearsal. **Seed size:
37 entries.**

**Budgets.** Index: 37 rows × ~230 B ≈ **8.5 KB**.
Catalog mass: 267,156 B (stdlib) + the two widget
bundles (in the standing 2.5–13.8 KB range) ≈ **≤ 295
KB**, all repo content — like `packages/bundles/`, the
registry charges **no package budget** (it is in
neither package's file set). Standing budgets unmoved:
npm 76,800 B, playground 771,072 B. The one new
dependency (`sha2`, §4) is named in the gate's
footprint clause and must be absent from the browser
profile's graph. **Lockstep 1.7.0** (the stamp is the
gate phase's act).

**CI.** A registry consistency check, priced from the
§1 measurement (`verify_library`'s 35 standings ≈ 31 s
solo on this 1.5-vCPU box; CI runners are faster): for
every index entry — the file exists, its sha256 matches
the row, and `registry verify`'s per-bundle
reproduction passes against a checkout-local index.
Recommended shape: a step in the existing CI running
the shipped binary against the repo's own `registry/`
(the offline form of the same verbs the gate uses), so
a catalog PR that adds a lying or stale bundle fails
before merge. **Price: S–M** (the step is small; the
cost is the re-verification time it adds per run).

---

## Synthesis

### Recommended shape

One milestone **1.7**, one gate, lockstep **1.7.0** —
the rung's last component, at the Phase 148 price for
meaning (a) (**L** in aggregate), confirmed part by
part: the format and both verbs of 1.4 carry the centre
of it, and every new build is an S–M at a named seam.

| Phase | Work | Price |
|---|---|---|
| 163 | Freeze (`DISEBAR_17_SCOPE.md`: index format, install dir + receipt, precedence, budgets) + the production verb (`build --bundles-from`) + the seed catalog (`registry/` + index: 35 stdlib copies + the widget pair produced by the verb) | M |
| 164 | The client: `morphic registry` (add / list / remove / verify), the fetch (HTTPS + local index), the sha256 binding (`sha2`), the install flow with its named refusals, the receipt ledger | M |
| 165 | The discovery layer: the installed set at the prelude funnel (typeck + SpecIndex + codegen seams), precedence user > stdlib > installed, native-only gating; installed entries callable, judged and generated like library entries | M |
| 166 | The 1.7 gate (incl. the stranger test + the CI consistency wiring) + merge + v1.7.0 | S–M |

### Gate sketch — 1.7

1. **The stranger test.** From an empty install dir,
   with the shipped binary: a user-module bundle (the
   widget exhibit) is fetched from the catalog,
   installed, and then called by name from a fresh spec
   — which checks, is judged, and generates Rust
   computing battery-exact. End to end, no step
   trusting a claim.
2. **The install is the verification.** Each tamper
   shape — flipped verdict, moved battery count,
   swapped body, sha256 mismatch against the index,
   unsupported bundle version — is refused by name at
   install with **nothing written**; a Tier-1 bundle in
   the no-Z3 build is refused by name; a stdlib-named
   bundle is refused by name even though its claim
   reproduces.
3. **Collision discipline.** An installed entry never
   shadows a stdlib name (a hand-planted file is
   equally inert); a user's own definition shadows an
   installed entry; `registry remove` restores the
   pre-install standings exactly — the name declines
   again.
4. **The catalog is consistent.** Every index entry's
   file exists, hashes to the row's sha256, and
   re-verifies with the shipped binary (the CI check,
   run as a gate instrument over the repo's own
   `registry/`); every receipt's recorded standing
   equals the standing its install re-measured.
5. **Footprint + suites.** Index + catalog masses
   inside their stated figures; `sha2` the milestone's
   only new dependency, absent from the browser
   profile; playground bundle within 771,072 B; suites
   green in the three frozen configurations.

*Closing line (draft):* "1.7 closes when anyone can
publish a proven function and no one has to take its
word for it — the install re-runs the proof, the
receipt records what the re-run measured, and a name
stands in the prelude because it was verified, not
because it was fetched."

### Decisions (recommendations in the house style)

**D1 — Distribution home.** (a) **In-repo `registry/`
+ static JSON index over HTTPS raw, PR-curated.** (b)
GitHub release assets. (c) Pages-hosted index.
*Recommendation: (a)* — (b) forecloses third-party
publishing (only the maintainer can attach assets);
(c) buys a second publishing surface for a URL; (a)
works today at zero infrastructure, its curation is
the repo's existing review flow, and the format is
host-agnostic if (c) is ever wanted.

**D2 — Client surface.** (a) **One `morphic registry`
command with add / list / remove / verify.** (b) Flat
top-level verbs. *Recommendation: (a)* — four verbs on
day one, more imaginable (update, search); the group
keeps the flat top level for the compiler's own verbs
and the departure is contained in one subcommand.

**D3 — Production route.** (a) **`build
--bundles-from <file.morph>`, a sibling arm sharing
`--package-from`'s admission loop.** (b) A flag on
`--package-from` emitting bundles beside the package.
*Recommendation: (a)* — the standing build discipline
is one arm per artifact with combinations refused by
name (`--bundles` already refuses `--package-from`);
(b) couples two artifacts' refusal semantics into one
invocation.

**D4 — Collision precedence.** (a) **user > stdlib >
installed; install refuses stdlib names and
already-installed names by name.** (b) Installed may
shadow stdlib behind a warning. *Recommendation: (a)*
— (b) is the silent-shadow shape this rung exists to
prevent; a warning is a shadow that was announced, and
the name would still denote the wrong contract. (a)
costs a producer nothing: pick a name that is yours.

**D5 — The integrity binding.** (a) **sha256 in the
index + the `sha2` dependency (native registry surface
only).** (b) No hash — re-verification is the
integrity. *Recommendation: (a)* — (b) proves the bytes
are *a* proven function; only the hash proves they are
*the catalogued* one. A substituted bundle with a
different spec for the same name passes (b) while
changing what the name denotes — the exact failure the
registry's curation exists to prevent.

**D6 — Seed catalog.** (a) **The 35 stdlib bundles +
the widget pair (37 entries).** (b) Stdlib only. (c)
The example only. *Recommendation: (a)* — (b) makes
the catalog a mirror of what every install already
has; (c) makes it a toy. (a) proves both halves of the
story on day one: the library, fetchable as bundles;
and a stranger's functions, admitted by the same door.

**D7 — Discovery.** (a) **Automatic: the compiler
reads the install dir at the prelude funnel (once per
process), `MORPHIC_REGISTRY_DIR` override.** (b) An
explicit flag per invocation. *Recommendation: (a)* —
the prelude's value is no-import convenience shared by
every shell at one funnel; (b) makes installed entries
check under one invocation and decline under another,
and every shell (CLI, API, MCP, LSP) would thread the
flag separately. (a)'s cost — a directory read per
process — is named and measured in the build phase.

**D8 — Milestone shape.** (a) **One milestone 1.7,
phases 163–166, one gate, lockstep 1.7.0.** (b) Split
production/distribution into two milestones.
*Recommendation: (a)* — the parts share one honesty
story (a bundle is only as good as its install); the
1.3/quantified-mod lesson cuts the other way only when
the stories differ. They do not here.

### Named findings and deferrals

- **The `verify_library` reuse claim, corrected.**
  DISEBAR_SCOPE §4's machinery was built "for 1.7's
  reuse", and `src/pcc.rs`'s header names
  `verify_library` as part of it. Measured: the
  function is the stdlib's self-check and takes no
  bundle; 1.7 reuses `verify_bundle_text` and adds the
  sibling sweep. The header's sentence should be
  amended when 165 touches the area — recorded here so
  the freeze inherits the correction, not the claim.
- **The self-containment boundary is the catalog's
  contract.** A bundle is one prelude-self-contained
  function (§1): specs naming sibling user specs refuse
  at production, by name, on both crown routes.
  Multi-function bundles / module-context bundles are
  a bundle-format-v2 conversation — **named, not in
  1.7**, and not to be widened silently by any phase of
  it.
- **The module system stays not-this-rung.** Nothing
  in 1.7 touches imports, namespaces, or §15 (whose
  grammar buglet also stands as recorded at Phase 148).
  Installed entries are called by bare name because the
  prelude is the only consumption mechanism 1.x has —
  the registry registers what the language can consume.
- **Provenance is the index's layer.** Author /
  description / homepage fields, if ever wanted, belong
  to index rows (listing facts), not to the bundle —
  the bundle stays pure evidence. Not built in 1.7
  beyond name + version; named so no phase invents a
  parallel scheme inside the evidence format.
- **Incidents, in the open.** The audit's first full
  default-suite tally (862/1) was read through a
  truncating pipeline and lost part of the run — the
  recorded Phase 152/157/160 tally class, this time the
  audit's own. It was discarded in full; the
  configuration was re-run with the log kept, green
  end-to-end (951/0/29), and no figure in this document
  rests on the discarded tally. One probe defect of the
  audit's own making is likewise recorded: the
  inventory probe's first form ran a pipeline crown
  without the file's serial lock and flipped the
  `verify_library` probe under the concurrency; the
  lock was taken and every figure re-measured.
