# DISEBAR_17_SCOPE.md — Phase 163: the frozen 1.7 scope + the gate

The ratification + freeze document for **1.7 — the
registry, in meaning (a)**: a registry of prelude
bundles with install-time re-verification, the Disebar
rung's last named component. Source material:
[`DISEBAR_17_AUDIT.md`](DISEBAR_17_AUDIT.md) (Phase 162
— the rung re-founded at Phase 148 §3 and frozen as
DISEBAR_SCOPE D5a, re-measured against the v1.6.0 tree;
probe artifact `tests/phase162_registry_probes.rs`,
7 probes, every audit claim a `P162|` line).

**Ratification.** On 2026-11-25 the owner delegated
this campaign's decisions to Muse — *"buat semua
aku kasih keputusannya ke kamu"* — reaffirmed
2026-11-26 when the campaign launched, and the
audit's recommended package **D1a–D8a** stands
ratified by Muse under the owner's delegation, in
the open, each decision with its grounds stated in
§1. The delegation is scoped to this campaign, as the
G4 delegation was scoped to its round (Phase 127
precedent); the ratification is recorded here so the
owner can veto any item at any time. Per D8a this
milestone is one honesty story under one gate: a
bundle is only as good as its install. This document
freezes:

- **the milestone's contents** (§2): the registry and
  nothing else, the lockstep version, the named
  non-scope.
- **the catalog** (§3): its home, the index format
  field by field, the serving URL, the curation act,
  the seed.
- **production** (§4): the `build --bundles-from`
  arm — built by this phase against this section.
- **the verbs + the install flow** (§5): Phase 164's
  build, frozen here — the command family, the
  install directory, the receipt ledger, the five
  steps and their refusals.
- **precedence + collisions** (§6): the three-layer
  rule and the install-door refusals.
- **discovery** (§7): Phase 165's build, frozen here
  — the installed layer at the prelude funnel.
- **the dependency** (§8): `sha2`, the milestone's
  one new dependency, and where it lands.
- **the budgets** (§9): the catalog's figures and
  the standing budgets; the CI consistency
  instrument.
- **the gate** (§10): the audit's 5-clause sketch
  finalized into gate text, closing line verbatim
  from the audit's draft.
- **the phase plan** (§11): Phases 163–166.

This phase (163) lands the freeze, the production arm
(§4) and the seed catalog (§3.4). Phase 164 builds
§5, Phase 165 builds §7, Phase 166 runs §10.

---

## 1. The ratified decisions (D1a–D8a)

**D1a — Distribution home: in-repo `registry/` + a
static JSON index over HTTPS raw, PR-curated.**
*Grounds:* release assets (b) foreclose third-party
publishing — only the maintainer can attach assets to
a release; a Pages-hosted index (c) buys a second
publishing surface to maintain in exchange for a
prettier URL. (a) works today at zero infrastructure,
its curation is the repo's existing review flow, and
the format is host-agnostic if (c) is ever wanted
(audit §2).

**D2a — Client surface: one `morphic registry`
command family — add / list / remove / verify.**
*Grounds:* four verbs on day one, more imaginable
(update, search); the group keeps the flat top level
for the compiler's own verbs and the departure from
the CLI's flat single-word verbs is contained in one
subcommand (audit §3).

**D3a — Production route: `build --bundles-from
<file.morph>`, a sibling arm sharing
`--package-from`'s admission loop.** *Grounds:* the
standing build discipline is one arm per artifact
with combinations refused by name (`--bundles`
already refuses `--package-from`); a flag on
`--package-from` (b) would couple two artifacts'
refusal semantics into one invocation. The audit
measured the gap as exactly this arm + the assembly
retaining its inputs (audit §1).

**D4a — Collision precedence: user > stdlib >
installed; install refuses stdlib names and
already-installed names by name.** *Grounds:*
installed-shadowing-stdlib behind a warning (b) is
the silent-shadow shape this rung exists to prevent —
a warning is a shadow that was announced, and the
name would still denote the wrong contract. (a) costs
a producer nothing: pick a name that is yours (audit
§5).

**D5a — The integrity binding: sha256 in the index +
the `sha2` dependency (native registry surface).**
*Grounds:* re-verification alone (b) proves the bytes
are *a* proven function; only the hash proves they
are *the catalogued* one. A substituted bundle with a
different spec for the same name passes (b) while
changing what the name denotes — the exact failure
the registry's curation exists to prevent (audit §4).

**D6a — Seed catalog: the 35 stdlib bundles + the
widget pair — 37 entries.** *Grounds:* stdlib only
(b) makes the catalog a mirror of what every install
already has; the example only (c) makes it a toy.
(a) proves both halves of the story on day one: the
library, fetchable as bundles; and a stranger's
functions, admitted by the same door (audit §6).

**D7a — Discovery: automatic — the compiler reads
the install dir at the prelude funnel, once per
process, `MORPHIC_REGISTRY_DIR` override.**
*Grounds:* the prelude's value is no-import
convenience shared by every shell at one funnel; a
per-invocation flag (b) makes installed entries check
under one invocation and decline under another, and
every shell (CLI, API, MCP, LSP) would thread the
flag separately. (a)'s cost — a directory read per
process — is named and measured in the build phase
(audit §3).

**D8a — Milestone shape: one milestone 1.7, phases
163–166, one gate, lockstep 1.7.0.** *Grounds:* the
parts share one honesty story (a bundle is only as
good as its install); the 1.3/quantified-mod lesson
cuts the other way only when the stories differ.
They do not here (audit Synthesis).

---

## 2. The milestone

**1.7 = registry (a), and nothing else**: the
production arm (`build --bundles-from`), the seed
catalog in-repo, the `morphic registry` verb family,
install-time re-verification, and automatic discovery
at the prelude funnel. Lockstep **1.7.0**: crate,
packages and the release move together at the Phase
166 stamp; until then every surface stands at the
tree's version. Registry publishing of the JS/Rust
packages remains the owner's act, as it has stood
since 1.1; the catalog's publishing act is a pull
request (§3.3), which is the repo's own flow, not a
registry account.

**Non-movers.** The library's standings are
untouched: 35 entries (33 Tier-1 / 2 Tier-2), the
manifest rows, the PCC bundles at `packages/bundles/`
(the registry's copies are copies, §3.4/R3), every
prior surface. The registry adds a fetchable home and
a consumption path for evidence that already exists;
no body, verdict, or battery moves.

**Named non-scope.**

- **A real module system / import resolution** —
  measured XL at Phase 148 and named there as
  explicitly *not* this rung (a language milestone).
  Nothing in 1.7 touches imports, namespaces, or
  §15. Installed entries are called by bare name
  because the prelude is the only consumption
  mechanism 1.x has.
- **Multi-function bundles** — a bundle is **one
  prelude-self-contained function** (measured at
  production by the audit, §4.3 below): its spec and
  body may name builtins and prelude entries, nothing
  else. A bundle carrying its module context is a
  bundle-format-v2 conversation — named, not in 1.7,
  and not to be widened silently by any phase of it.
- **Open unmoderated publishing** — the index is
  PR-curated (§3.3). Curation is a listing decision,
  never a trust decision (§3.5); there is no upload
  endpoint, account, or token anywhere in this
  milestone.
- **Any change to the bundle format** —
  `bundleVersion` **1 stands**. Version is the
  index's fact (§3.2): the bundle carries no version,
  author, or provenance fields, and none are added.
  Provenance lives at the catalog layer, exactly as
  the npm manifest already separates `packageVersion`
  (the author's) from `morphicVersion` (the
  pipeline's).

---

## 3. The catalog

### 3.1 Home and layout

`registry/` at the repo root:

```
registry/
  index.json            the listing (§3.2)
  bundles/
    <entry>.bundle.json one file per entry
  README.md             what this is, how a bundle
                        gets in, the trust model
```

### 3.2 The index format (frozen)

One JSON object:

```json
{
  "format": "morphic-registry-index",
  "indexVersion": 1,
  "entries": [
    {
      "entry": "int_abs",
      "version": "1.7.0",
      "file": "registry/bundles/int_abs.bundle.json",
      "sha256": "<64 lowercase hex>",
      "bytes": 2539,
      "tier": "tier-1",
      "verdictBasis": "z3-verified"
    }
  ]
}
```

Per-entry fields, frozen:

| Field | Fact |
|---|---|
| `entry` | the function's bare name — the flat namespace's key (§6) |
| `version` | the **catalog version** of this entry (semver string) — the index's fact; the bundle carries no version |
| `file` | the bundle's **repo-relative path** (R1) |
| `sha256` | the sha256 of the bundle file's bytes — the binding that makes curation enforceable (D5a) |
| `bytes` | the bundle file's byte count |
| `tier`, `verdictBasis` | **labelled claims**, duplicated from the bundle so a human can browse — the bundle is the evidence; a lying row is caught at install by the bundle's own reproduction (§5) |

Rows stand sorted by `entry` (the catalogue
discipline). A new version of an entry replaces its
row + file in one commit; receipts pin the version +
sha each install actually measured, so an updated
catalog never rewrites history on an installed
machine (audit §5).

### 3.3 Serving + curation

The index is served to clients over HTTPS raw from
the repo's public URL. The canonical form, named:

```
https://raw.githubusercontent.com/thvtzy/morphic/master/registry/index.json
```

Any ref — branch, tag, commit — may be named in
place of `master`; a bundle's `file` resolves
against the repo root at the same ref (R1). The
trust model makes the host's identity nearly
irrelevant: a hostile mirror can serve a *wrong*
bundle, never a *trusted* one (§3.5).

**Publishing = a pull request** adding bundle files
+ index rows to `registry/`. That is the only review
flow that exists at this scale, and it is the
milestone's whole publishing surface (D1a).

### 3.4 The seed (built by this phase)

**37 entries**: the 35 stdlib bundles, copied
**byte-identical** from `packages/bundles/` (R3) —
plus the widget pair, `is_widget_code` (Tier-1,
Verified, battery 5/5/5) and `weights_sorted`
(Tier-2, ExecutedExact, battery 5/5/5), produced by
the new `--bundles-from` arm (§4) from the Phase 154
exhibit module during this phase and committed as
produced. Neither widget name collides with the
stdlib. Every seed row carries catalog version
**1.7.0** — a *cataloguing* fact (when they were
catalogued), stated as such: the bundles record no
version and have regenerated byte-identical at every
stamp since 1.4 (R2).

Measured at the seed: **index 10,927 B; catalog
276,678 B** (the 35 at 267,156 B + the pair at
2,706 B + 6,816 B, both inside the standing bundle
range of 2,539–13,802 B). The figures the gate
measures are frozen in §9.

### 3.5 The trust model (one paragraph, as the README states it)

The index's tier/basis fields are labels; the bundle
is the evidence; install (§5) checks the bundle's
sha256 against the index row and then re-runs the
bundle's proof obligation before writing anything —
so claims are never the mechanism of trust, only its
browsing surface.

---

## 4. Production — `build --bundles-from` (built by this phase)

### 4.1 The arm

```
morphic build --bundles-from <file.morph> [--seed <n>] [--out <dir>]
```

A sibling arm of `--package-from` (D3a), with the
standing one-arm-per-artifact discipline:
combinations with `--package`, `--facade`,
`--bundles`, or `--package-from` refuse by name, and
the arm names its module with `--bundles-from`
itself — a positional input beside it refuses by
name. Defaults mirror the sibling verb (R4): the
seed is the standing **42**; the output directory is
`<stem>-bundles`; each bundle is written as
`<entry>.bundle.json`. The flags are exactly the
input, the seed and the output — **no flag supplies
a standing**; the run is the only source of one (the
Phase 154 claim-surface discipline, carried).

### 4.2 Admission — the shared loop

Admission is `--package-from`'s, verbatim: the two
verbs share one loop (`codegen::package::
admit_user_specs`), so the discipline cannot drift
between them. Per spec, the seeded bounded LLM-free
profile runs; a spec is admitted iff its crown stands
**Verified with an exact battery** on its evaluable
points (→ tier-1 / `z3-verified`) or **executed-exact**
(passed == evaluable == total, total > 0 → tier-2 /
`executed-exact`) — in a no-Z3 build no crown stands
Verified and the same module measures executed-exact
rows instead, the rows saying which evidence they
are. Anything else is **refused by name** with its
measured standing, and the reading is the stricter
one the sibling verb froze: **if any spec is refused,
the whole production is refused and nothing is
written.**

### 4.3 Production — the checked route, and its boundary

Each admitted spec's retained `BindingInput` goes
through `pcc::produce_bundle`, which *measures* the
bundle's claim by re-running the crown before
writing anything: a body the named route does not
crown is refused by the route's own reason — **a
moved crown refuses**. Production is therefore a
second refusal surface, and it is where the format's
boundary stands:

> **A bundle is one prelude-self-contained
> function.** Production re-judges (spec, body)
> standalone against the prelude; a spec whose
> constraints name a sibling user spec (the audit's
> `quadruple_it`, stated through `double_it`) is
> admitted in context and **refused at production by
> name** — `quadruple_it: the bundle's spec does not
> check: … Called undefined function: 'double_it'`.
> This is the catalog's contract, inherited from the
> format as shipped — not a defect 1.7 fixes, and
> not one it widens (§2).

The refusal is the **Tier-1 route's** boundary, and
it bites exactly where a Verified claim is at stake.
A no-Z3 build has no Tier-1 route (§4.2): the same
sibling module measures executed-exact rows there
and bundles at tier-2, whose claim — execution
evidence only — reproduces in any build (measured
at Phase 163: the no-Z3-produced `quadruple_it`
bundle re-verifies under a Z3 build as tier-2 /
executed-exact). The contract therefore stands in
every build in its exact form: **no tier-1 bundle
exists, anywhere, for a spec that is not
prelude-self-contained.**

Two productions of the same module at the same seed
are **byte-identical** (pinned).

---

## 5. The verbs + the install flow (Phase 164's build, frozen here)

### 5.1 The command family

```
morphic registry add <name>[@<version>] [--index <url-or-path>]
morphic registry list
morphic registry remove <name>
morphic registry verify [<name>]
```

`add` resolves `<name>` against the index — the
catalogued row, or the named `@<version>`'s row when
the version is given. `verify` with no name sweeps
the whole install directory (the `verify_library`
sibling the audit's correction names: it iterates
the install dir calling the per-bundle verb, exactly
as `verify_library` iterates the shipped inputs
calling the crown checks); with a name it re-judges
that entry only. `list` reads the receipt ledger
(§5.3): each installed entry with its *measured*
standing, and the catalog's claim beside it where a
catalog is fetched and they differ — they must not;
if they do, the receipt is the truth and the
difference is printed, not hidden. `remove` deletes
the bundle file + the receipt row; nothing else
references the install dir, so removal is complete
by construction — the name declines again from the
next process on.

### 5.2 Where installs live

`~/.morphic/registry/` — outside any repo, per-user
— overridable by **`MORPHIC_REGISTRY_DIR`** (one
directory, one name: the verbs and discovery (§7)
read the same override). Flat contents: one
`<entry>.bundle.json` per installed entry and one
`installed.json` **receipt ledger**. No other state.

### 5.3 The receipt

One ledger row per install, recording what the
install's re-run **measured** — never what the index
claimed:

| Field | Fact |
|---|---|
| `entry` | the installed name |
| `version` | the catalog version it was installed from |
| `sha256` | the installed file's hash (== the index row's, checked) |
| `tier`, `verdictBasis`, `verdict` | **as reproduced** by the install's re-run |
| `crownRoute` | the route the re-run judged through |
| `battery` | `{total, evaluable, passed}` **as reproduced** |
| `index` | the index the install resolved against (URL or path) |
| `installed` | the install date |

### 5.4 The install flow

Each step's failure writes **nothing**:

1. **Resolve.** Fetch the index (HTTPS) or read it
   (local path); find the entry row. Absent → decline
   by name.
2. **Fetch + bind.** Fetch the bundle bytes; check
   their **sha256 against the index row**. Mismatch →
   refuse by name. (The hash is load-bearing: only it
   binds *name → the catalogued bytes*; re-verification
   alone proves the bytes are *a* proven function,
   and a substituted bundle carrying a different spec
   for the same name would also re-verify while
   silently changing the contract the name denotes.)
3. **Parse + re-verify.** `pcc::verify_bundle_text`
   on the fetched bytes. Every standing refusal is
   the install's refusal, verbatim: unsupported bundle
   version, incoherent claim, a body the route does
   not crown, a moved battery count — and in a no-Z3
   build, a Tier-1 bundle refused by name (it stands
   unexamined; a Tier-2 bundle installs on executed
   evidence).
4. **Collision checks** (§6): a stdlib name → refuse
   by name; an already-installed name → refuse by
   name (the update path is `remove` + `add`, so the
   ledger stays an honest record of what was measured
   when).
5. **Write + receipt.** The bundle file lands in the
   install dir; the ledger gains its row (§5.3).

### 5.5 Fetch sources

The default index is the canonical URL (§3.3).
`--index` accepts an **HTTPS URL or a local path**,
distinguished by scheme; a local index makes the
whole flow offline-capable and is the gate's
hermetic form. The fetch client is `reqwest`, already
in the default build's graph — **no new HTTP
dependency**. In the `--no-default-features` build
there is no HTTP client, so a network fetch **declines
by name** there while a local `--index <path>`
install works — the honest posture, mirroring the
no-Z3 Tier-1 refusal.

---

## 6. Precedence + collisions (D4a)

The namespace is flat — a language fact, not a
registry choice: calls are bare names resolved
against one flat prelude, and no scoped form
(`@author/name`) can be called by any syntax 1.x
has. The catalog is therefore a flat namespace with
**first-come curation**: a name enters the index
once, by PR; disputes are the curator's.

The merge the registry extends is measured (audit
§5): today user > stdlib. 1.7 extends it to three
layers with one rule:

> **user > stdlib > installed**

An installed entry merges only into names neither
the module nor the stdlib holds. Two consequences,
both deliberate:

- **An installed bundle can never shadow a stdlib
  name** — not by accident (the funnel's layer order
  skips it, §7) and not by malice (a hand-planted
  file in the install dir is equally inert — the
  funnel's layer order, not the directory's contents,
  decides). Install *also* refuses stdlib names at
  the door (§5.4 step 4), by name, so the user hears
  about the collision instead of silently installing
  a dead entry. Defense in depth, both layers named.
- **A user's own definition shadows an installed
  entry**, by the same rule that already shadows the
  stdlib: their module, their name, their call. The
  installed entry is a default, never an override.

Installing a name that is already installed refuses
by name (§5.4 step 4); the update path is `remove`
then `add`.

---

## 7. Discovery (Phase 165's build, frozen here)

**Automatic, at the prelude funnel.** The installed
set becomes one more layer behind the two functions
every library consumer already reads through —
`stdlib::prelude_specs` yields the stdlib's specs
then the installed set's (an installed name the
stdlib holds is not yielded: §6's inertness, realized
at the funnel), and `stdlib::shipped_body` answers
the shipped body for stdlib names and the installed
body for installed names, never across the layer
line. The layer reads the install directory **once
per process** (the stdlib's own OnceLock pattern):
each installed bundle's spec is parsed from the
bundle's own serialization and its body from its IR
text — both parsers exist.

**Every shell inherits.** The CLI's check /
synthesize, the API, the MCP tools and the LSP all
build on the same funnel, so installed entries are
callable, judged and generated in all of them exactly
as library entries are — no shell threads a flag
(D7a).

**Native-only.** The browser build's prelude arrives
as a runtime asset and it has no filesystem: the
installed layer is gated where the funnel reads the
directory (the `wasm-playground` cfg discipline the
stdlib already uses), and the browser build never
reads the disk.

**Degradation is named, never silent.** A bundle
file in the install dir that fails to parse is
skipped with a named diagnostic (the entry and the
reason); a directory that cannot be read at all
yields an empty installed layer with a named
diagnostic. The stdlib and the user's module are
unaffected in both shapes; `registry verify` (§5.1)
reports the same facts as refusals. A corrupt install
never silently changes what a name denotes, and
never silently empties the layer either — it says
so.

---

## 8. The dependency — `sha2` (D5a)

The tree has **no SHA-256 anywhere** today (audit
§4). `sha2` is the milestone's **one new
dependency**, small and single-purpose:

- It lands at **Phase 164**, with the install flow
  that consumes it (this phase's seed hashes are
  computed at catalogue time with the system tool —
  a production-time act, like the index row's other
  facts; the consistency pin asserts them where the
  system `sha256sum` is checkable).
- It is a dependency of the **native builds** — the
  default graph and the `--no-default-features`
  graph alike: the no-default build declines
  *network fetch* by name (§5.5) but its local-index
  installs still hash-check, so the hash may not be
  feature-gated away there.
- It is **absent from the browser profile's graph**:
  the `wasm-playground` build carries no registry
  surface (§7), and the gate's footprint clause
  measures the absence.

No other dependency enters in 1.7: the fetch rides
the standing `reqwest` (§5.5).

---

## 9. Budgets + CI

| Figure | Value | Basis |
|---|---|---|
| Seed index | **≤ 12,288 B** (12 KiB) | Phase 163 measurement 10,927 B, rounded up to a whole KiB (the W4 formula's spirit) |
| Seed catalog (37 bundle files) | **≤ 295,000 B** | audit §6 figure; measured 276,678 B at the seed |
| npm package | ≤ 76,800 B | standing, unmoved |
| Playground bundle | ≤ 771,072 B | standing, unmoved |
| Components | ≤ 106,496 B / ≤ 90,112 B | standing, unmoved |

The catalog is **repo content**: like
`packages/bundles/`, it is in neither package's file
set, so it charges **no package budget**. A
measurement above a frozen figure at the gate fails
the gate's footprint clause rather than re-basing
the figure — the standing discipline since Phase
138.

**CI.** The catalog-consistency instrument (wired at
Phase 166): a step in the existing CI running the
shipped binary against the repo's own `registry/` —
the offline form of the same verbs the gate uses
(local `--index`, `registry verify` over the
checkout) — asserting for every index entry that the
file exists, its sha256 matches the row, and its
claim reproduces. Priced from the audit's datum (the
35-standing self-check ≈ 31 s solo on this 1.5-vCPU
box; CI runners are faster): a catalog PR that adds
a lying or stale bundle fails before merge.

---

## 10. The 1.7 gate

Run by Phase 166 on the final tree, clause by
clause, in the M1-gate style. A failing clause stops
the merge.

1. **The stranger test.** From an empty install dir,
   with the shipped binary: a user-module bundle (the
   widget exhibit) is fetched from the catalog,
   installed, and then called **by name** from a
   fresh spec — which checks against the merged
   prelude, is judged through the index (the installed
   body is the name's denotation), and generates code
   that computes battery-exact. End to end, no step
   trusting a claim.
2. **The install is the verification.** Each tamper
   shape — flipped verdict, moved battery count,
   swapped body, sha256 mismatch against the index,
   unsupported bundle version — is refused by name at
   install with **nothing written**; a Tier-1 bundle
   in the no-Z3 build is refused by name; a
   stdlib-named bundle is refused by name even though
   its claim reproduces.
3. **Collision discipline.** An installed entry never
   shadows a stdlib name (a hand-planted file is
   equally inert); a user's own definition shadows an
   installed entry; `registry remove` restores the
   pre-install standings exactly — the name declines
   again.
4. **The catalog is consistent.** Every index entry's
   file exists, hashes to the row's sha256, and
   re-verifies with the shipped binary (the CI check
   of §9, run as a gate instrument over the repo's
   own `registry/`); every receipt's recorded standing
   equals the standing its install re-measured.
5. **Footprint + suites.** Index + catalog masses
   inside §9's figures; `sha2` the milestone's only
   new dependency, absent from the browser profile;
   playground bundle within 771,072 B; suites green
   in the three frozen configurations (Phase 128 §9:
   default / bootstrap-codegen /
   `--no-default-features`).

*Closing line (frozen verbatim from the audit's
draft):* "1.7 closes when anyone can publish a proven
function and no one has to take its word for it — the
install re-runs the proof, the receipt records what
the re-run measured, and a name stands in the prelude
because it was verified, not because it was fetched."

---

## 11. Phase plan

| Phase | Work | Exit evidence |
|---|---|---|
| 163 | This freeze + the production arm (§4) + the seed catalog (§3.4) | The freeze text; the arm's pins (the widget pair at their measured standings, the sibling refusal, the moved crown, determinism, the CLI surface); the seeded index + 37 files, consistent and re-verifying |
| 164 | The client (§5): `morphic registry` add / list / remove / verify, the fetch (HTTPS + local index), the sha256 binding (`sha2`, §8), the install flow with its named refusals, the receipt ledger | The install flow end-to-end against a local index; the tamper battery refused by name with nothing written; receipts recording measured standings |
| 165 | The discovery layer (§7): the installed set at the prelude funnel, precedence user > stdlib > installed, native-only gating | Installed entries callable by name from a fresh spec — checked, judged, generated; the collision pins (stdlib inertness, user shadowing); the funnel's neighbours unmoved |
| 166 | The 1.7 gate (§10) + the CI consistency wiring (§9) + merge + v1.7.0 | The gate run clause by clause on the gate tree (the stranger test as centrepiece); the 1.7.0 stamp in lockstep; PR merged on green CI; tag + GitHub Release; registry publishes left as the owner's acts |

---

## 12. Resolutions

**R1 — The index's `file` field is repo-relative**
(`registry/bundles/<entry>.bundle.json`), resolving
against the repo root at the index's ref over HTTPS,
and against the parent of the `registry/` directory
holding the index file for a local index. *Basis:*
the brief's frozen field list names the repo-relative
path; a path that only resolves beside one host's
layout would make the index itself host-shaped,
against D1a's host-agnostic ground.

**R2 — Every seed row's `version` is "1.7.0", a
cataloguing fact.** *Basis:* audit §6 — the bundles
record no version and regenerate byte-identical at
every stamp, so the row claims when they were
catalogued, nothing about the functions.

**R3 — The 35 stdlib bundles are byte-identical
copies.** `packages/bundles/` stays the packages'
evidence home; `registry/bundles/` is the fetchable
home of the same artifacts. The checked route
(`build --bundles`) refreshes the former; a
cataloguing PR refreshes the copies by re-copying,
and the §9 consistency instrument catches any drift
between the homes. *Basis:* one artifact, two homes
— a second production of the same evidence would be
a second claim to keep honest.

**R4 — Production defaults mirror `--package-from`:**
seed 42, output `<stem>-bundles`, files
`<entry>.bundle.json`. *Basis:* the sibling verb's
naming discipline is the tree's standing answer to
the same questions; a second dialect buys nothing.

**R5 — The `verify_library` correction is inherited,
not repeated.** DISEBAR_SCOPE §4's machinery was
built "for 1.7's reuse" and `src/pcc.rs`'s header
names `verify_library` as part of it; measured, the
function takes no bundle and iterates the shipped
inputs only. 1.7 reuses `verify_bundle_text` and
adds the sibling sweep (§5.1). The header's sentence
is amended when Phase 165 touches the area (the
audit's naming) — recorded here so the freeze
inherits the correction, not the claim.

**R6 — The canonical index ref is `master`.** Any
ref may be substituted (§3.3); `master` is the
repo's standing branch and the URL a stranger is
handed. *Basis:* the audit's "any ref a consumer
names", with one ref named as the default so the
catalog has a front door.
