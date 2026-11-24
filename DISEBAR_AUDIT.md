# DISEBAR AUDIT — the distribution rung, measured (Phase 148)

**What this is.** A scope audit of the **Disebar rung** of the
north star pinned 2026-11-22 (ROAD Phase 107 note): *MCP
server · WASM Component Model · `.morph` package registry ·
LSP + VS Code extension · edge deploy · proof-carrying
code*. The rung's premise, also pinned: fame is distribution
plus one killer niche, and Morphic's wedge is the AI-agent
era — generated code is abundant, *proven-correct* code is
scarce. Disebar is the rung that puts Morphic where agents
and developers already work.

**Method.** Each component is measured against the tree as
it stands at v1.3.0 (master `3f4e092`): code reading where
reading settles it, probes where a claim needs one — 7
probes in `tests/phase148_disebar_probes.rs`, every claim
below carrying its `P148|` line or its file/line evidence.
Every component is priced (S/M/L/XL), with its dependencies
and its honesty risks named. Audit only: no production code
changed; the quantified-mod branch stays parked and
untouched.

**Headline.** The rung is real but uneven, and its centre
of gravity is not the component the roadmap names first.
**The MCP server is the cheapest large lever on the rung**
— the API it would wrap was built in v0.6 for the browser
shell and already returns real Z3 verdicts natively
(measured: a seeded synthesis crowns **Verified** in
~0.2 s). **Proof-carrying code is the rung's honesty
engine**: the manifest's `verdictBasis` is measured to be a
*claim* (printed verbatim even for a body that fails its
spec), while the re-verification machinery that could turn
it into *evidence* already exists in-process and ships
nowhere. **The registry's premise fails measurement as
usually stated**: there is no module system — imports
parse but resolve to nothing — so "a package" has no
importable referent in 1.x; the only package-shaped thing
the compiler can consume is a **prelude bundle**, and the
registry recommendation is re-founded on that. **Edge
deploy is the only component whose artifact already
ships** (the npm package is pure ESM, zero Node APIs,
measured live). The Component Model's type work is done
(EMBED_SCOPE §3's WIT column) and only its pipeline is
missing. The LSP's blocker is not the protocol — it is
that **the compiler cannot currently say where a type
error is**.

Sandbox capability, measured for the prices below: node +
npm present; the `wasm32-unknown-unknown` target installed;
Binaryen 133 pinned at `~/workspace/tools/`; crates.io
reachable (index HTTP 200 — new dependencies *can* be
fetched, each becoming a pin to maintain); **absent**:
`cargo-component`, `wasm-tools`, `wit-bindgen`, any
wasmtime-class runtime, `wat2wasm`.

---

## 1. MCP server — the agent channel

**What exists.** Almost everything, one shell short.
`src/api.rs` (Phase 66) is a string/DTO API built for
embedding shells: `check(source) -> CheckReport`,
`synthesize_seeded` / `synthesize_seeded_spec`,
`render(candidate, target)` — all serde DTOs, all
filesystem-free (the CLI owns the filesystem contract;
audit H4 of Phase 66). The browser shell
(`crates/morphic-wasm`) is the standing proof that this
API carries a whole product surface. In a **native**
build the same API carries the real verifier:
`P148|mcp|synthesis status=Verified
verification_available=true elapsed_ms=212` — a seeded
run over the validator corpus spec crowns a Verified
winner in ~0.2 s, where the browser build honestly
reports `verification_available=false` / Inconclusive
(the v0.6 D2 contract, already implemented in the DTOs).
The library is enumerable in-process for a
`list_library` tool: `P148|library|entries=35 tier1=33
z3_verified=33 executed_exact=2` (prelude specs +
shipped bodies + the generated manifest, one call each).

**What is missing.** (a) *The process and the protocol*:
zero JSON-RPC or MCP code exists in the tree (grep over
`src/`, `crates/`, the lock graph: no `jsonrpc`, no
`rmcp`, no MCP SDK). The needed subset — JSON-RPC 2.0
framing over stdio, `initialize`, `tools/list`,
`tools/call` — is small, and `serde_json` is already a
dependency: **a hand-rolled server needs zero new
dependencies**. (b) *A standalone verify tool*: verdicts
today ride inside synthesis; `Verifier::verify_one`
exists at library level but the API exposes no
"verify this body against this spec" entry point — an
S-sized API addition (the PCC crown check, §6, is its
near-relative). (c) *Tool schemas*: one hand-written
JSON Schema per tool (~6 tools: check, synthesize,
verify, list_library, render, and a spec-template
resource at most). (d) *Latency discipline*: a server
tool call must be bounded — the CLI's synthesis profile
is entropy-seeded and effectively unbounded (10,000
default iterations); the **seeded playground profile**
(`api::playground_config`: LLM-free, iteration/population
caps, deterministic per seed) is the server-shaped one,
and a spec-size cap is owed.

**Safety shape.** The API's tools are pure functions of
source text — no filesystem, no network, no LLM (the
playground profile is LLM-free by construction). An MCP
server wrapping `api` inherits exactly that surface;
nothing in this component touches the user's files
unless a future tool is added that does, which this
audit does not propose.

**Honesty risks.** One, and it is already handled by the
DTO contract: a no-Z3 server build must report
`verification_available: false` rather than dress
executed evidence as verdicts. The server should ship
only in the native (Z3) configuration, and say so.

**Price.** Minimal useful server (stdio, hand-rolled
JSON-RPC, the six tools over `api`, the verify entry
point, seeded/bounded synthesis): **M**. The full
surface (HTTP transport, auth, streaming progress,
prompts/resources beyond a template): L–XL, and mostly
premature — MCP clients live on stdio today.

**Decisive fact.** *The API an MCP server needs was
built in v0.6 for a different shell; the server is a
shell, not a compiler project.*

## 2. WASM Component Model — the typed boundary

**What exists.** The *decisions*, complete: EMBED_SCOPE
§3's WIT column maps the whole frozen surface
(`Int→s64`, `Char→char`, `String→string`,
`List<T>→list<T>`, `Option<T>→option<T>`,
`Result<T,E>→result<T,E>`, records→`record`) with losses
named — under WIT most of §3's JS-boundary losses
evaporate (no BigInt domain question, no `T | null`
flattening). The wasm32 target is installed; the
playground pipeline (wasm-bindgen + Binaryen, pinned)
builds the current artifact, measured at 751,360 B
(286,522 B gzip).

**What is missing.** The *pipeline*, entire: the current
artifact's header is `00 61 73 6d 01 00 00 00` — a **core
module**, not a component (a component carries version
`0x0d`, layer `0x01`); nothing converts one to the other
in-tree. No WIT file, no component-shaped crate, and
none of `cargo-component` / `wasm-tools` / `wit-bindgen`
is installed. Each is installable (network measured
above) — and each becomes a **second pinned toolchain**
beside the wasm-bindgen + Binaryen pins the project
already maintains.

**The higher-order wrinkle (new since EMBED_SCOPE §3).**
§3's Function-types row predates G4: since 1.2 the
surface has six higher-order exports whose function
parameters are Morphic values. WIT interfaces have no
function-typed *values* — a component expresses a
callback as an **imported function**, not a parameter.
So the component surface cannot be the npm surface
row-for-row: the HO exports arrive either as
monomorphic instantiations or with the callback moved
to an import. Stating the WIT world is S; it is a real
design fork the freeze must take, not a detail.

**What a host gains over the npm package.** A
language-neutral typed boundary — Rust/Python/Go/C
hosts on wasmtime-class runtimes, exact types, no
JS anywhere. What it does not gain: audience. The
wasmtime-host population is smaller and more specialist
than npm's today (EMBED_AUDIT §5's standing point, not
re-measured away).

**Price.** **L** — a new crate, the WIT world design
(incl. the HO fork), wit-bindgen bindings generated
from the same shipped bodies, a second build pipeline,
and a host rehearsal in at least one non-JS host.
(EMBED_AUDIT priced XL *as the 1.1 route*, where it
also carried the milestone; as its own step with §3
frozen, L.)

**Decisive fact.** *The type work is done and transfers
unchanged; everything missing is a pipeline — and a
pipeline is a toolchain you pin forever.*

## 3. `.morph` package registry — the premise, measured

**The premise fails as usually stated.** A registry
presupposes packages, and a package presupposes a way
to consume one. Measured: `import "math.morph";` parses
and is accepted — and resolves to **nothing**
(`P148|registry|import=parsed-not-resolved`): no file
is loaded, no imported name becomes callable (a call to
an import-only name fails typecheck), qualified calls
(`algo::is_sorted`) are not part of the language
(LANGUAGE_REFERENCE §15, which states the consequence
itself: "A user-facing import mechanism is a language
feature with its own gate"). The only library mechanism
is the **prelude**: the stdlib's modules are merged
into every module by the compiler, entries callable by
plain name, the user's own definition winning a
collision (Phase 110). Every public API consumes
exactly one source string. (One wrinkle the probes
stepped on: §15's printed grammar omits the `;` the
parser requires after an import — a reference buglet,
recorded here; fixing it is a doc line, not this
audit's act.)

**The three meanings "registry" could take.**

(a) **A registry of prelude bundles.** A third-party
package = a bundle in the stdlib's own shape (specs +
shipped bodies + batteries + manifest — the
`LibraryAsset` format is its embryo) that the compiler
merges like the stdlib, under the standing collision
rule. What must be built: the bundle format as a
public, versioned thing; an install/merge verb
(`morphic bundle add`); and the trust story — a merged
bundle's bodies are code the compiler *trusts*, so its
tier claims are only honest if installation **re-verifies
them** (the PCC machinery of §6 — this meaning's honesty
engine, and a hard dependency). Price: **L**.

(b) **A real module system first.** Resolved imports,
namespaces, version resolution: a *language* feature
with its own gate, touching the parser, the checker
(whose fiction table and prelude merge assume one flat
namespace), `SpecIndex`, `inline_calls`, and every
emitter's name rendering. It re-opens frozen language
surface. Price: **XL**, and it is not a distribution
step — it is a language milestone (1.5/2.0-shaped) that
a future owner decision may schedule on its own
merits.

(c) **Registry-as-index.** A listing page/feed over
what already distributes (the npm + crates packages,
plus downloadable bundle files once (a) exists). No
compiler change. Price: **S–M** — and near-zero value
standing alone; it is (a)'s front door, not a rung
component of its own.

**Recommendation.** The rung takes meaning **(a)**,
gated on install-time re-verification; (b) is named
here as explicitly **not** part of Disebar; (c) folds
into (a) as its listing surface.

**Decisive fact.** *"Package" has no importable
referent in the 1.x language — the only package-shaped
thing the compiler can consume today is a prelude
bundle, so that is what the registry must register.*

## 4. LSP + VS Code — the editor surface

**What exists.** A reserved name and the diagnostic
seed corn. The CLI already carries an `lsp` subcommand
— a **stub**: it prints a banner and returns
(`src/main.rs`, `// TODO: LSP implementation`). Parse
diagnostics are LSP-shaped already: 1-based line +
column + snippet. Whole-source check is fast —
`P148|lsp|... whole_source_check_ms=4` on a real corpus
module — so a naive recheck-on-change server is viable
without incremental machinery. The check report's spec
inventory (names, params, return types, test counts)
seeds document symbols and completion.

**What is missing.** (a) **Positions for type errors.**
`P148|lsp|parse_error=line+column
typeck_error=no-position`: `TypeError` *has* a span
field and **every constructor leaves it `None`**
(grep: all construction sites in `src/spec/typeck.rs`);
the AST carries **no span fields at all** (grep over
`src/spec/ast.rs`: no span/line/column), and the parser
is a char-based recursive descent with no token stream
— positions exist only as the parser's cursor at the
moment an error is raised. Populating spans is
parser + AST + checker surgery: **M–L**, with
regression surface over every diagnostic pin in the
tree. (b) **The protocol framing** — the same
hand-rolled JSON-RPC core the MCP server needs (§1);
built once, it serves both. (c) **Document state** —
an open-buffer map; trivial beside (a). (d) **The VS
Code extension** — a thin TypeScript client (VS Code's
generic LSP client does the work): **S**, plus a
marketplace listing, which is an owner's-account act
like the registry publishes.

**The honest intermediate.** An LSP shipped *before*
the span build can underline parse errors and surface
type errors as unpositioned messages — a
**diagnostics-only** server. It must be labelled
exactly that (the project's standing rule: name the
cap, don't dress it). Hover types, go-to-definition,
and positioned type errors wait on the span build.

**Price.** Diagnostics-only server on the shared
JSON-RPC core: **M**. The span build: **M–L** (its own
phase, compiler surgery). Full-server increment on top
of spans: **M**. VS Code extension: **S** per client.

**Decisive fact.** *The blocker is not the protocol —
the compiler cannot currently say where a type error
is, and fixing that is surgery, not plumbing.*

## 5. Edge deploy — the artifact that already ships

**What exists.** The deployable artifact, measured
live: `packages/npm` is pure ESM with **zero Node API
surface** (grep over `index.js`: no `node:`, no `fs`,
no `process`), 35 exports, and it computes under node
exactly as the 1.1 stranger test measured
(`int_clamp(15n, 0n, 10n) = 10n`,
`list_min([3n,1n,2n]) = 1n`). Cloudflare Workers and
its peers accept precisely this shape (ESM in, no Node
builtins required). The *other* wasm artifact is the
wrong one for this job: the playground bundle
(286,522 B gzip — inside Workers' size limits) carries
no Z3 and exposes no typed library functions; it is a
playground, not a function package.

**What is missing.** (a) A **deploy template / verb**:
a starter (wrangler config + an entry re-exporting the
package + a smoke route) or a `morphic deploy` verb
wrapping one — **S–M**. (b) The **user-spec story**:
`morphic build --package` assembles the *stdlib*
package only; the ungated generator
(`bindings::generate_bindings`) already accepts user
modules in-process, but no CLI verb wraps it — a
`--package-from <file>`-shaped verb is **S**. (c)
**Evidence on a real deploy**: cold-start and size on
an actual Workers account — the owner's account act,
like the publishes; the audit claims the shape fits,
not that a deploy was run. (d) The standing BigInt/JSON
boundary at HTTP edges (documented in EMBEDDING.md;
a template should ship the one-line guard).

**Price.** **S–M** total (template + user-package verb
+ docs). The cheapest component on the rung after
PCC-lite — because there is nothing to build, only
something to wrap.

**Decisive fact.** *Edge is the only Disebar component
whose artifact already ships; what's missing is a
template and a verb, not a build.*

## 6. Proof-carrying code — claims vs evidence

**The boundary, measured.** The manifest's
`verdictBasis` is a **claim**: `BindingInput`'s own doc
says the basis is "stated by the caller from the
pipeline's own record", and the generator believes it —
`P148|pcc|manifest_claim=verbatim`: a binding generated
for the identity body against `output == x + 1`,
labelled `tier-1` / `z3-verified`, produces a manifest
that prints exactly that. Nothing in
`generate_bindings` checks the claim; the checks that
would catch the lie (`check_pipeline_crown` /
`check_encoder_crown`) run at *package assembly*,
producer-side, and ship nowhere. A consumer today
receives a string and the producer's good behaviour.

**What a certificate adds.** A bundle the consumer
re-checks: the spec, the body in a portable form, the
verdict claimed, and the battery — plus a verb
(`morphic verify-bundle`) that re-runs the judgment
and compares. **The machinery already exists**:
`P148|pcc|crown_check true_body=ok
wrong_body=rejected` — `check_encoder_crown` re-verifies
a shipped body from (spec, body) alone through the
production verifier, accepts the true body, rejects a
wrong one, and in a no-Z3 build **refuses by name**
("package assembly requires the Z3-enabled native
build") rather than passing — the honesty posture a
consumer verb needs, already implemented.

**The mechanism choice, stated plainly.** Z3 *proof
objects* are **not** the certificate: the pinned z3
0.20 crate exposes no portable, independently checkable
proof format a stranger could validate without the
same solver build — a solver transcript is one solver's
derivation, checkable in practice only by re-solving.
The honest certificate is **re-verification**:
deterministic re-judging of (spec, body, battery) with
the shipped binary. It is stronger where it matters —
it re-checks the *semantics*, not one solver's path —
and its cost is the verifier's own runtime (seconds,
measured across the crown checks).

**The one real build.** Bodies today live as Rust IR
constructions in `src/stdlib.rs`. A bundle needs the
body in a transport form. The IR's Debug rendering is
already the pipeline's body-identity fingerprint
(`api.rs` uses it as such) but is not a parser input
and is not versioned. Making an IR text serialization
**stable, versioned, and parseable** is the component's
core cost: **M**. One shortcut with real value: for the
*stdlib's own* entries the consumer's compiler already
contains the bodies (the prelude), so a
`morphic verify --library` self-check verb — re-run
every crown check against the installed compiler's own
library — is **S** and covers the shipped packages'
claims with no format work at all. The full bundle
(M) is what *user* functions and registry bundles (§3a)
need.

**Price.** Stdlib self-verification verb: **S**. User
bundle format + `verify-bundle`: **M**. Proof objects:
not priced — rejected as the mechanism, above.

**Decisive fact.** *The evidence a stranger can
re-check already exists as a function call; what's
missing is only the bundle format and the verb that
ships it.*

---

## Synthesis

### The sequence

Distribution leverage per unit price, with dependencies
respected:

1. **MCP server (§1, M).** First, because it is the
   wedge's own channel: the north star says the scarce
   thing in the agent era is proven-correct code, and
   MCP is where agents ask for code. It wraps an API
   that exists, needs zero new dependencies, and its
   JSON-RPC core is shared with the LSP later. It is
   also the rung's best demo: an agent calling Morphic
   and receiving a *Verified* function is the pitch, as
   a running system.
2. **Proof-carrying code (§6, S then M).** Second,
   because it is every other component's honesty engine:
   the MCP server's verdicts, the registry's tier
   claims, and the packages' manifest rows all become
   *re-checkable* instead of *stated*. The S half
   (`verify --library`) can ride with step 1; the M
   half (the bundle) is the registry's prerequisite.
3. **Edge deploy (§5, S–M).** Third and independent —
   it can interleave anywhere after step 1; it is
   placed here because the template's smoke route is a
   natural second demo and it costs almost nothing.
4. **LSP, diagnostics-only (§4, M) + VS Code client
   (S).** Fourth: real developer surface, honestly
   capped; it reuses step 1's JSON-RPC core. The span
   build that unlocks the full server is sequenced
   last (below) — it is compiler surgery with the
   least distribution leverage per unit risk.
5. **Component Model (§2, L).** Fifth: the typed
   boundary for non-JS hosts. Its decisions are frozen
   (EMBED_SCOPE §3) and do not rot; its cost is a
   second pinned toolchain, paid when the audience
   argument is strongest — after the agent channel and
   the editor surface exist.
6. **Registry, meaning (a) (§3, L).** Sixth, because
   it *depends* on step 2's bundle + re-verification:
   a registry of prelude bundles whose installs
   re-verify is honest; without that it is a registry
   of claims, which this project does not ship.
7. **The span build + full LSP (§4, M–L + M).**
   Named, priced, and deliberately unsequenced beyond
   "after the diagnostics-only server": the owner
   schedules compiler surgery when its leverage is
   wanted.

**Explicitly not in the rung:** the module system
(§3b) — a language milestone, not distribution; the
full MCP surface (HTTP/auth/streaming); Z3 proof
objects (§6). Each is named here so no later phase
inherits it silently.

### Milestone shape

**One milestone per honesty story** (the standing
lesson of the 1.3/quantified-mod split). The rung is a
ladder, not a gate:

- **1.4 — the agent channel + the honesty engine**:
  the MCP server and PCC (the `verify --library` verb
  and the user bundle + `verify-bundle`). One gate
  (sketched below), lockstep 1.4.0 for the packages.
- Follow-on milestones, named not frozen: **1.5** edge
  template + LSP diagnostics-only + VS Code client ·
  **1.6** Component Model · **1.7** registry (a). The
  span build rides whichever milestone the owner
  attaches it to.

### Decisions (recommendations in the house style)

**D1 — First component.** (a) **MCP server.** (b)
Component Model. (c) LSP. *Recommendation: (a)* — the
highest distribution leverage per price on the rung,
the wedge's native channel, zero new dependencies,
and its protocol core is reused by (c) later. (b) is
the named horizon but buys a specialist audience at L
plus a permanent second toolchain; (c)'s full form is
blocked on surgery (§4).

**D2 — MCP protocol route.** (a) **Hand-rolled
JSON-RPC 2.0 over stdio, zero new dependencies**
(`serde_json` is already in the tree; the needed
subset is initialize / tools-list / tools-call /
ping). (b) The `rmcp` SDK (a new dependency, fetchable
— measured). (c) HTTP transport first.
*Recommendation: (a)* — stdio is where MCP clients
live; the project's shells are hand-rolled on the same
posture (the wasm shell wraps `api` directly); the
framing is written once and shared with the LSP. (b)
remains available if the protocol surface outgrows
the subset; nothing in (a) forecloses it.

**D3 — The server's synthesis profile.** (a) **The
seeded, bounded, LLM-free profile only**
(`api::playground_config` + a spec-size cap), verdicts
real in the native build. (b) The full CLI profile,
LLM seeds included. *Recommendation: (a)* — a tool
other programs call must be bounded and reproducible:
same (source, seed) → same winner. LLM seeds are
neither, and a server that phones an LLM is a
different trust surface than this component proposes.

**D4 — The PCC mechanism.** (a) **Re-verification**:
a bundle of (spec, IR-serialized body, battery,
claimed verdict) + `morphic verify-bundle` re-judging
it; the stdlib covered first by the S-sized
`verify --library` self-check. (b) Z3 proof objects.
(c) Signed manifests. *Recommendation: (a)* — (b) is
rejected on the evidence story (no portable
independently-checkable proof format from the pinned
solver; a transcript is re-checkable only by
re-solving, which is (a) with extra steps); (c) is a
claim with a signature — still a claim.

**D5 — The registry's meaning.** (a) **Prelude-bundle
registry with install-time re-verification** (§3a).
(b) Module system first (§3b). (c) Index only (§3c).
*Recommendation: (a)* — it is the only meaning with a
referent in the 1.x language; (b) is a language
milestone this audit explicitly does not smuggle into
a distribution rung; (c) folds into (a) as its listing
surface.

**D6 — LSP depth in the rung.** (a) **Diagnostics-only
first**, labelled as such (parse positions real,
type errors unpositioned), the span build a separate
later phase. (b) Full LSP or nothing.
*Recommendation: (a)* — (b) makes the editor surface
wait on M–L of compiler surgery for its first user;
(a) ships the honest 80% on the shared JSON-RPC core
and names its cap on its own surface.

**D7 — Component Model timing.** (a) **After MCP /
PCC / edge, as its own milestone** (§3 decisions
already frozen in WIT-compatible terms). (b) First —
it is the named horizon. *Recommendation: (a)* — the
type work does not rot (it is frozen text, not code),
the audience is the rung's most specialist, and the
toolchain pin is permanent; leverage orders it fifth,
not first.

**D8 — Milestone shape.** (a) **1.4 = MCP + PCC, one
gate; the rest of the rung as named follow-on
milestones.** (b) One gate for all six components.
(c) No milestones — ad-hoc components.
*Recommendation: (a)* — (b) gates unlike honesty
stories together (the 1.3/quantified-mod lesson);
(c) abandons the freeze/gate discipline that makes
this project's claims checkable.

### Gate sketch — 1.4 (MCP + PCC)

1. **The channel works.** A clean MCP client (a
   scripted JSON-RPC client pinned in the gate's
   instrument set) connects over stdio to the shipped
   server and calls the frozen tool set; check /
   synthesize / verify answers agree with the library
   API's own outputs on a pinned corpus, byte-for-byte
   where the API is deterministic.
2. **Verdict honesty at the boundary.** Every verdict
   the server returns is the native pipeline's own:
   `verification_available` stated per build; a spec
   whose crowned body fails stands VerificationFailed
   through the tool, never dressed; synthesis is the
   seeded bounded profile — same (source, seed) →
   same winner body, through the server.
3. **The evidence re-checks.** `verify --library`
   passes against the shipped compiler's own library;
   the bundle for every shipped entry re-verifies with
   the shipped binary (`verify-bundle` exit 0, all
   rows); a tampered bundle (one body changed) fails
   by name; the manifest's language points at the
   bundle as the evidence for its rows.
4. **Footprint.** The server adds no new runtime
   dependency to the compiler's default build (its own
   bin target or feature); the playground bundle stays
   within 771,072 B; any package the milestone touches
   stays within its frozen budget.
5. **Suites green** in the three frozen configurations.

*Closing line (draft):* "1.4 closes when an AI agent
can ask Morphic for a proven function through the
tools agents already speak — and when anyone holding
the artifact can re-run the proof obligation
themselves instead of trusting the row that claims
it."

### Phase plan

| Phase | Work | Price |
|---|---|---|
| 149 | MCP server: hand-rolled JSON-RPC/stdio core + the tool set over `api` + the verify entry point (seeded bounded profile) | M |
| 150 | PCC: `verify --library` (S) + the IR text serialization, bundle format, `verify-bundle` (M) | M |
| 151 | 1.4 gate + merge + release (lockstep 1.4.0) | S–M |
| — | *Named follow-ons, frozen at their own audits/freezes:* edge template + user-package verb · LSP diagnostics-only + VS Code client · Component Model · registry (a) · the span build + full LSP | S–M · M+S · L · L · M–L+M |

### Named findings and deferrals

- **The `morphic lsp` stub.** The CLI has promised a
  language server by name since the CLI's first shape;
  it prints a banner and does nothing. Not fixed here
  (audit only) — recorded so the 1.5 freeze decides
  deliberately whether the stub hides until the server
  lands, rather than the promise standing by inertia.
- **§15's grammar buglet.** LANGUAGE_REFERENCE §15's
  Import production omits the `;` the parser requires.
  A one-line doc fix, the owner's to schedule; the
  audit records it because the registry component's
  premise measurement stepped on it.
- **EMBED_SCOPE §3's Function-types row predates
  G4.** It says no export is higher-order; since 1.2
  six are. The row's TS-boundary conclusion stands
  (the `.d.ts` keeps `never` for fn parameters); the
  WIT consequence — callbacks as imports, not values
  (§2) — is this audit's addition, and the Component
  Model freeze inherits it.
- **Not yet, plainly:** the registry in any
  import-based meaning (blocked on a language feature
  that is not this rung's to build); the full LSP
  (blocked on the span build); Z3 proof objects
  (rejected as a mechanism, not deferred). A phased
  rung with named deferrals is the outcome; nothing
  above inflates a component past its measurement.
