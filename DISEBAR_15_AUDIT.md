# DISEBAR 1.5 AUDIT — the reach rung, measured (Phase 152)

The scope audit for **1.5**, the Disebar rung's second
milestone: the three components [`DISEBAR_SCOPE.md`](DISEBAR_SCOPE.md)
§6 named as its follow-ons — **edge deploy template**,
**LSP diagnostics-only**, **VS Code extension** —
re-measured against the v1.4.0 tree (master @ `bc8fafd`)
and priced as a campaign. Source material:
[`DISEBAR_AUDIT.md`](DISEBAR_AUDIT.md) §§4–5 (Phase 148's
measurements of the LSP and edge components, taken when
neither the MCP server nor PCC existed yet) and the
Phase 148 record in
[`ROAD_TO_SELF_HOSTING.md`](ROAD_TO_SELF_HOSTING.md).
Probe artifact: `tests/phase152_disebar15_probes.rs`,
9 probes, every audit claim a `P152|` line.

**Headline.** No component's premise fails measurement
— the contrast with Phase 148's registry is the first
finding. All three components are what the 148 audit
said they were, and two of them got cheaper or sharper
since: the edge artifact now provably loads in a bare
V8 context with *no platform globals at all*, and the
LSP's diagnostics engine turns out to be a single
already-shipping function (`api::check`, the MCP
`check` tool's DTO) whose singular-error shape bounds
the whole server design. The campaign prices at
**≈ L total**: edge template **S** · user-package verb
**S–M** · LSP diagnostics-only **M** · VS Code client
**S** · gate + merge + release **S–M**. The decisions
are the owner's this round (D-list in the Synthesis);
this audit recommends, it does not ratify.

---

## 1. Edge deploy — the template and the verb

**What exists, re-measured.** The deployable artifact
at 1.4.0: `packages/npm` holds 36 `.js` files (the
assembled module + 35 per-entry shims); `index.js` is
**15,802 B**, the whole JS payload **17,278 B**, the
package 71,873 B unpacked. Purity, scanned file by
file: **zero Node-surface tokens** in any `.js` file
(no `require(`, `node:`, `process.`, `Buffer`,
`__dirname`, no `fs` import), and `package.json`
declares `"type": "module"` with **no `dependencies`
and no `devDependencies` keys at all**
(`P152|edge|artifact`).

**The sandbox measurement.** Phase 148 asserted the
shape fits Workers; this phase loads it. The shipped
`index.js` was evaluated inside a bare node `vm`
context — a fresh V8 global scope with **no `process`,
no `require`, no `Buffer`, not even `fetch`** — linked
with an importer that refuses everything, and it
computes: all **35 exports** present, `int_clamp(15n,
0n, 10n) = 10n`, `list_min([3n,1n,2n]) = 1n`, and
`list_map` applying a host-supplied callback
(`P152|edge|sandbox`). The `vm` context is the honest
local proxy for a Workers isolate — it proves the
module needs *nothing beyond JavaScript intrinsics*;
it does not measure workerd cold start, and this audit
does not claim it (a live deploy remains the owner's
account act, §1.4 below).

**Platform pick: Cloudflare Workers.** The evidence,
per candidate:

- **Workers** — module-format Workers consume exactly
  this shape: plain ESM, no Node builtins unless the
  `nodejs_compat` flag is opted into (the artifact
  provably doesn't need it — the sandbox had *fewer*
  globals than workerd provides). Size is a non-issue:
  the entire JS payload (17,278 B) is ~1.7% of the
  free tier's 1 MB compressed script ceiling (10 MB
  paid). The entry convention is the thinnest of the
  three: a module worker needs **no platform global at
  load** — the template's fetch handler is the only
  platform-shaped code.
- **Vercel Edge** — hostable, but its shape is
  framework-entangled (vercel.json / `runtime: 'edge'`
  conventions); a template there teaches a framework's
  routing, not a function package.
- **Deno Deploy** — ESM-native, but its idiomatic
  entry leans on the `Deno` global and `npm:`
  specifiers; the template would carry a platform
  runtime the artifact provably doesn't need.

**The template (the deliverable shape).**
`templates/edge-cloudflare/`: a `wrangler.toml`
(name, main module, compatibility date), a
`src/index.js` fetch handler that imports the package
and routes `POST /fn/<name>` to the named export, a
`package.json` depending on `@thvtzy/morphic`, a
README with the deploy steps, and a **local smoke
script** — the `P152|edge|sandbox` harness generalized
to load the template's entry in the no-globals sandbox
and call through the handler. The template also ships,
as code, the one documented boundary EMBEDDING.md
names: JSON cannot carry BigInt, so the handler
coerces numeric/string inputs to BigInt on the way in
and serializes BigInt to string on the way out — the
guard is a few lines, shipped rather than described.
Wrangler itself is **not** a repo dependency: it runs
on the owner's machine (`npx wrangler deploy`); the
in-repo smoke needs only node. **Price: S.**

**The user-package verb.** What a user package looks
like *today*, measured: the binding generator
(`bindings::generate_bindings`) is public, ungated,
and **already accepts user modules in-process** — a
probe module's spec + hand body produced a module and
a manifest row (`P152|edge|generator`, re-pinning
Phase 148's PCC-1 on this tree). The package layout
shell (`codegen::package::assemble_npm_package`) is
input-agnostic except for its input source
(`bindings::stdlib_bindings()`) and its stdlib
README/name constants. But no user can reach any of
it: `build --package` **refuses an input file by
name** at the CLI (`P152|edge|build_package`) — it is
stdlib-only in exactly the way the 148 audit said.

The verb (working shape for the freeze:
`morphic build --package-from <file.morph>`, with
package name/version flags) must therefore add
admission and wiring, not generation:

- Parse + typecheck the user's module; for each spec,
  run the **seeded, bounded, LLM-free profile** (the
  standing D3a discipline the MCP server also runs)
  and build the generator's inputs **only from
  measured rows**: the verdict from the run's own
  `verification_status`, the battery from its own
  executed counts. Both are reported per run today
  (`P152|edge|admission`: the standing exhibit crowns
  **Verified at 5/5/5**; a wrong body against a spec's
  own tests measures **0/2**).
- **Admission rule:** a spec enters the package iff
  its crown is **Verified** (tier-1 / `z3-verified`)
  or **executed-exact** (`passed == evaluable ==
  total`, `total > 0`; tier-2 / `executed-exact`).
  Anything else — Inconclusive, VerificationFailed, a
  partial battery — is **refused by name and nothing
  is written**, the `--package` assembly discipline
  (a moved crown fails before any file lands) carried
  to user modules.
- The PCC-1 fact forces this design: the generator
  prints caller-stated tier/basis **verbatim**
  (`P152|edge|generator` — the probe's deliberately
  false claim sailed through, as it did at Phase
  148). The verb is therefore the one place user
  packages are allowed to come from: it feeds the
  generator only rows it measured itself. The
  stdlib's crown checks (`verify_crowns`,
  `check_tier1_crown`) do **not** transfer — they
  re-judge the *shipped* bodies against the prelude;
  what transfers is the refusal discipline, the
  generator, and the layout shell. (A freeze that
  wants denoted/hand bodies admitted too has its
  re-judgment route ready: `api::verify_body`,
  Phase 149.)

**Price: S–M** (a CLI arm + per-spec synthesis +
admission + the layout shell's input swap).

**What remains the owner's act.** The Cloudflare
account, `wrangler deploy`, any route/DNS binding,
and with them all live evidence (cold start, real
size on a real account). The campaign's deliverable
is template + verb + local smoke — **a deploy is not
in scope**, and the gate's edge clause (§Synthesis)
rests on the sandbox proxy, named as such.

**Decisive fact.** *The artifact fits the strictest
sandbox on the market with room to spare and the
generator already speaks user modules — what's
missing is a template, an admission rule, and a CLI
arm, in that order of honesty risk.*

---

## 2. LSP diagnostics-only — the editor tells the truth

**The stub, re-measured.** `morphic lsp` on the
shipped binary: stdout receives **1,447 bytes** — the
full ASCII banner plus one line, `🔌 Starting Morphic
Language Server...` — the process reads nothing and
**exits 0 immediately** (`P152|lsp|stub`). It is not a
degraded server; it is a reserved name with a banner
on the very stream a real server would own.

**The diagnostics engine already exists.** It is one
function — `api::check(source) -> CheckReport`, the
DTO the MCP `check` tool already serves — and its
shape bounds the server's whole design
(`P152|lsp|diagnostics`):

- **At most one diagnostic per check.** `CheckReport`'s
  error is a singular `StageError`; the typechecker
  returns its *first* `TypeError` and stops. Measured:
  a file with two broken specs reports `'f'` and never
  mentions `'g'`. So `publishDiagnostics` carries 0 or
  1 entries — no error-recovery machinery exists, and
  none is needed for this milestone. Multi-error
  reporting is parser/checker surgery, named here and
  **not** priced into 1.5.
- **Parse errors are positioned**: 1-based line +
  column + snippet (the LSP range conversion is
  mechanical: line−1, column−1).
- **Typecheck errors are positionless, re-measured**:
  line/column `None` through the API; `TypeError.span`
  is `None` at **all 7 construction sites** in
  `src/spec/typeck.rs` (grep, this tree); the AST
  carries **no span fields at all** (grep: zero
  occurrences of `span` in `src/spec/ast.rs`). But the
  error is not anonymous: `TypeError.function` **is**
  populated — the message itself names the spec
  ("In spec 'f': …"). A typecheck error is locatable
  to a spec, just not to a point.
- **Latency**: whole-source check, in-process, debug
  build on this 1.5-vCPU box — **4 ms** on a typical
  module (validator, 8 specs); the corpus's largest
  module (`codegen.morph`, 152,251 B, 130 specs)
  **457 ms cold / 215 ms warm**. Release builds are
  faster and editors debounce; full-document recheck
  on change is viable with wide margin.

**Surfacing the positionless error — both options
priced.**

- **(a) File-level diagnostic**: one diagnostic at
  range (0,0)–(0,0), severity Error, source
  `morphic`, the typecheck message **verbatim** (it
  names the spec), and the cap named *in* the
  diagnostic (a `code` of `unpositioned`, or the
  message carrying it — the freeze picks the exact
  label). Cost: nothing beyond the publish path
  itself. The residual risk — a squiggle at file head
  misread as "the error is at 1:1" — is exactly what
  naming the cap in the diagnostic answers.
- **(b) Withhold**: publish only parse diagnostics.
  Cost: zero. But the editor then shows a **clean
  file while `morphic check` fails** — a silent-wrong
  of precisely the class this project has refused at
  every surface since Phase 116's loud-error rule.

**Recommendation: (a).** The project's honesty rule —
name the cap, don't dress it, and never let a surface
claim less trouble than there is — decides it.

**What transfers from the MCP core, and what does
not** (`P152|lsp|transfer`). `McpServer::handle_line`
answers a bare JSON-RPC request line at the message
level: the dispatch pattern, the serde_json
discipline, and the error vocabulary (−32700 /
−32600 / −32601 / −32602) all transfer. Two things
measurably do **not**:

1. **Framing.** LSP frames messages as
   `Content-Length: N\r\n\r\n` + N bytes. Fed exactly
   that, the MCP core answers **−32700** — no
   Content-Length framing code exists anywhere in
   the tree. The byte-exact framing layer (parse
   headers, read precisely N bytes, tolerate
   split/coalesced writes) is new work: small, and
   the server's only genuinely new machinery besides
   document state.
2. **The notification rule inverts.** MCP discipline:
   an id-less message is never answered **and never
   executed** (measured: an id-less `tools/call`
   returns no response and runs nothing). LSP's
   `didOpen` / `didChange` / `didClose` are id-less
   and **must** execute — they carry the document
   state — and the server's main output,
   `publishDiagnostics`, is itself an id-less
   server→client notification. An LSP handler that
   inherited the MCP rule would never see a document.
   The reuse is the dispatch *pattern*, not the
   discipline.

**Server scope (diagnostics-only, labelled as
such).** Requests: `initialize` (answer capabilities:
full-document sync, push diagnostics), `shutdown`.
Notifications: `initialized`, `textDocument/didOpen`,
`didChange` (full text), `didClose` (clear that
file's diagnostics), `exit`. Unknown methods decline
−32601 naming the method; malformed frames are
answered −32700 where recoverable; **nothing panics
the server**. Document state is a `HashMap<uri,
String>`. **Sync kind: full-document** — incremental
sync buys nothing here: the check is whole-source by
construction (no incremental check API exists to
feed), the pipe is local stdio (bandwidth is free),
and the measured recheck cost is the floor under
either sync kind; an edit-application layer would add
risk and no capability. **Price: M** (framing S +
handler/state S + diagnostics mapping S, integrated
and transcript-tested).

**The stub's fate — a freeze decision (D4).** (a)
`morphic lsp` *becomes* the real server: the name was
reserved for it, and the banner leaves stdout for
this subcommand — the precedent already exists in
`main.rs`, which suppresses the banner exactly where
stdout is a machine surface (`StdlibAsset` | `Mcp`).
(b) Hide or rename the stub in 1.5 and ship the
server under a new name later: two user-visible
disruptions, and `--help` lies by omission in
between. **Recommendation: (a).**

**The span build: price confirmed standing, not in
1.5.** The evidence is byte-for-byte the Phase 148
evidence (span-free AST, 7/7 `span: None` sites, a
char-cursor parser whose positions exist only at the
moment an error is raised). Populating spans remains
parser + AST + checker surgery at **M–L**, a separate
named item sequenced only as "after diagnostics-only"
— the 1.5 server touches neither parser nor checker.

**Decisive fact.** *The server's diagnostics engine,
error vocabulary, and dispatch pattern already ship;
its honest design is dictated by two measured shapes
— one diagnostic per check, and a typecheck error
that names its spec but not its point.*

---

## 3. VS Code extension — the thin client

**Greenfield, measured** (`P152|vscode|greenfield`).
The tree's only `package.json` is the library's
(`packages/npm/package.json`); there is no
`packages/vscode`, no `.vscode` directory, no TextMate
grammar anywhere. Nothing is refactored; a new small
package is created. The toolchain it builds with is
present in this sandbox (node v24.22.1).

**Shape.** `packages/vscode/`: a `package.json`
contributing the `morphic` language for `.morph`
files, a language configuration (brackets and comment
tokens from the LANGUAGE_REFERENCE's own grammar:
`//` line comments, `///` doc comments, `/* */` block
comments), a minimal TextMate grammar for
highlighting (declarative JSON — decoration, with no
honesty surface), and an extension entry in
TypeScript (~100 lines) that starts VS Code's generic
LanguageClient against `morphic lsp` (server path
configurable, `morphic.serverPath`, defaulting to a
PATH lookup).

**The one real new dependency** is
`vscode-languageclient` (npm) — and the boundary is
the finding: it lives **only in the extension's own
`package.json`**, inside `packages/vscode`'s own
`node_modules`, shipped inside the `.vsix`. The
compiler tree's Cargo graph gains nothing; the
library's npm package gains nothing. It is the first
dependency this project has taken that it does not
control end-to-end, and it is quarantined to the one
package that cannot work without it (VS Code's client
API *is* that library).

**What the client does NOT do.** Diagnostics-only
means diagnostics-only: no hover, no completion, no
go-to-definition, no formatting, no code actions —
the server offers none and the client invents none.
The manifest/bundle evidence story is **not**
surfaced in-editor in 1.5 (no hover-from-manifest);
that temptation is named and deferred past the span
build, where it would belong.

**Packaging and distribution.** `vsce package` →
`.vsix` → `code --install-extension` (sideload):
works with zero accounts, today. Marketplace
publishing requires a publisher account and an Azure
DevOps PAT — **the owner's account act**, deferred
exactly like the registry publishes (D6).

**Smoke test — both shapes priced.**

- **(a) Headless VS Code** (`@vscode/test-electron`):
  downloads a full VS Code build (100+ MB), needs a
  display/xvfb story this sandbox does not have, and
  tests client + server together. Price **M**, and
  fragile *here*; it may be the right instrument on
  the owner's machine later.
- **(b) Scripted LSP-session transcript** against the
  real `morphic lsp` binary: Content-Length-framed
  `initialize` → `didOpen` a parse-broken file →
  `publishDiagnostics` with the error at its true
  position → `didChange` to a type-broken file → the
  0:0 diagnostic named unpositioned → `didChange` to
  clean → an empty diagnostic set → `shutdown` →
  `exit`. This is the Phase 151 gate-transcript
  instrument class pointed at the new server, and it
  pins exactly the wire behavior the thin client
  depends on — the client's own code is VS Code's
  generic LanguageClient doing standard work. Price
  **S**, hermetic.

**Recommendation: (b)** as the campaign's instrument;
(a) named as an optional owner-machine check, never
gate evidence in this sandbox. **Extension price: S.**

**Decisive fact.** *The extension is a configuration
file, a hundred lines of standard client, and one
quarantined dependency — its entire risk lives in the
server's wire behavior, which the transcript pins
without it.*

---

## Synthesis

### The recommended 1.5 shape

**One milestone, one gate, lockstep 1.5.0** — the
D8a discipline (one honesty story per gate) applied
to a reach story: *a proven function, reachable* —
served at the edge from a template anyone can copy,
packaged from your own specs under a measured
admission rule, and edited against a checker that
tells the truth about what it knows. Total price
**≈ L**.

### Phase plan

| Phase | Contents | Price |
|---|---|---|
| 153 | Freeze (`DISEBAR_15_SCOPE.md`) + the edge template + its local sandbox smoke | S–M |
| 154 | The user-package verb (`build --package-from`, measured-rows-only admission) | S–M |
| 155 | The LSP diagnostics-only server (`morphic lsp`, Content-Length framing, full sync) | M |
| 156 | The VS Code extension (`packages/vscode`, .vsix packaging) | S |
| 157 | The 1.5 gate + merge + v1.5.0 | S–M |

### Gate sketch (5 clauses)

1. **Edge serves.** The template, populated once with
   the shipped library and once with a user package
   built by the verb, loads in the no-globals sandbox
   and answers its smoke calls battery-exact; the
   template's only platform-shaped code is its
   handler + config.
2. **The verb is honest.** A user module's package
   carries only measured rows (verdict and battery
   from the synthesis run itself); an uncrownable
   spec is refused by name with nothing written; the
   package's claims reproduce under the standing
   re-judgment routes.
3. **The editor tells the truth.** The scripted LSP
   transcript against the shipped binary: a parse
   diagnostic at its true position; a typecheck
   diagnostic at 0:0 carrying its unpositioned name;
   a clean file yields an empty set; shutdown/exit
   are clean; no banner byte ever touches the
   stream.
4. **The client is thin.** The `.vsix` packages from
   the extension's own tree; its dependency set is
   confined to `packages/vscode`; clause 3's
   transcript stands as its server-contract evidence;
   the marketplace is untouched.
5. **Footprint + suites.** The compiler tree's
   dependency set is unchanged (Cargo.toml/Cargo.lock
   diff empty); budgets stand (npm ≤ 76,800 B;
   playground bundle ≤ 771,072 B); suites green in
   the three frozen configurations on the gate tree.

Closing line, drafted for the freeze: *"1.5 closes
when a proven function can be serving at the edge
from a template anyone can copy — and when the editor
tells the truth about a broken spec: exactly where it
broke when the compiler knows, and plainly
unpositioned when it doesn't."*

### Decisions (recommendations in the house style)

The decisions are the **owner's** this round.

- **D1 — Edge platform first:** **(a) Cloudflare
  Workers** (the artifact's native shape; the
  strictest sandbox provably fit) · (b) Vercel Edge ·
  (c) Deno Deploy.
- **D2 — The user-package verb:** **(a) generalize
  `build --package`** into `--package-from <file>`
  with measured-rows-only admission · (b) a separate
  `morphic package` subcommand · (c) no verb in 1.5
  (template only; user packages wait).
- **D3 — Typecheck diagnostics in the editor:**
  **(a) a file-level 0:0 diagnostic, named
  unpositioned, message verbatim** · (b) withhold
  typecheck diagnostics until the span build.
- **D4 — The `lsp` stub's fate:** **(a) `morphic lsp`
  becomes the real server** in 1.5 (the banner leaves
  its stdout) · (b) hide/rename the stub now; the
  server ships under a new name later.
- **D5 — Document sync:** **(a) full-document** ·
  (b) incremental.
- **D6 — VS Code distribution:** **(a) `.vsix`
  sideload in 1.5**; marketplace publishing the
  owner's later act · (b) marketplace during 1.5
  (the owner creates the publisher + PAT
  mid-campaign).
- **D7 — Milestone shape:** **(a) one milestone,
  one gate, phases 153–157, lockstep 1.5.0** ·
  (b) split: an edge milestone, then an editor
  milestone.

### Named findings and deferrals

- **No premise failed.** All three components stand
  as measured; nothing was re-founded and no price
  was inflated to protect the plan.
- **The one-diagnostic ceiling is a compiler
  property, not a server choice** (§2): multi-error
  reporting needs checker surgery and sits,
  unsequenced, beside the span build.
- **The MCP notification discipline must not be
  reused by the LSP** (§2, measured inversion): the
  framing and the id-less rule are the two places the
  "shared JSON-RPC core" story stops.
- **Live-edge evidence stays unclaimed**: cold start
  and real-account behaviour are the owner's deploy
  act; the campaign's edge evidence is the sandbox
  proxy, named as such in the gate sketch itself.
- **The span build stands at M–L**, deliberately
  unsequenced beyond "after diagnostics-only"; 1.5
  touches neither parser nor checker.
- **`@vscode/test-electron` is rejected as gate
  evidence in this sandbox** (download mass + display
  story); it remains an owner-machine option.
