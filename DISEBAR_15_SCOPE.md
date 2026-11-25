# DISEBAR_15_SCOPE.md — Phase 153: the frozen 1.5 scope + the gate

The ratification + freeze document for **1.5 — a proven
function, reachable**: the Disebar rung's second milestone,
carrying the **Cloudflare Workers edge deploy template**,
the **user-package verb**, the **LSP diagnostics-only
server**, and the **VS Code extension**, and nothing else.
Source material: [`DISEBAR_15_AUDIT.md`](DISEBAR_15_AUDIT.md)
(Phase 152 — the three components named as 1.5 follow-ons
in [`DISEBAR_SCOPE.md`](DISEBAR_SCOPE.md) §6, re-measured
against the v1.4.0 tree; probe artifact
`tests/phase152_disebar15_probes.rs`, 9 probes, every audit
claim a `P152|` line).

**Ratification.** On 2026-11-25 the owner ratified the
audit's recommended package whole — *"Gas rekomendasi
audit — D1a sampe D7a semua"* — adopting decisions
**D1a–D7a** as stated in §1 below. Per D7a this milestone
is one reach story under one gate: *a proven function,
reachable* — served at the edge from a template anyone can
copy, packaged from a user's own specs under a measured
admission rule, and edited against a checker that tells
the truth about what it knows. This document freezes:

- **the milestone's contents** (§2): the four components,
  the lockstep version, and what stays the owner's act.
- **the edge template** (§3): Workers first, the template's
  shape, and its evidence instrument.
- **the user-package verb** (§4): `--package-from`
  generalized from `build --package`, with admission built
  only from measured rows.
- **the LSP contract** (§5): `morphic lsp` as the real
  diagnostics-only server — framing, lifecycle, document
  sync, and the honest surfacing of both error shapes.
- **the VS Code client** (§6): the thin client in
  `packages/vscode`, its quarantined dependency, and its
  distribution.
- **the budgets** (§7): the figures the gate measures.
- **the exclusions** (§8): what is not in this milestone.
- **the gate** (§9): the audit's 5-clause sketch finalized
  into gate text, closing line verbatim from the audit's
  draft.
- **the phase plan** (§10): Phases 153–157.

This phase (153) lands the freeze and the edge template
against §3. Phase 154 builds §4, Phase 155 §5, Phase 156
§6. Phase 157 runs §9.

---

## 1. The ratified decisions (D1a–D7a)

**D1 — Edge platform first: (a) Cloudflare Workers.** The
artifact's native shape: plain ESM, no Node builtins, no
`nodejs_compat` needed — measured loading and computing in
a bare V8 context with no platform globals at all
(`P152|edge|sandbox`). Vercel Edge (b) is
framework-entangled; Deno Deploy (c) leans on the `Deno`
global the artifact provably does not need.

**D2 — The user-package verb: (a) generalize
`build --package` into `--package-from <file>`** with
measured-rows-only admission (§4). A separate `morphic
package` subcommand (b) splits the build surface for no
measured gain; no verb in 1.5 (c) leaves user packages
unreachable while the template ships.

**D3 — Typecheck diagnostics in the editor: (a) a
file-level 0:0 diagnostic, named unpositioned, message
verbatim** (§5.3). Withholding typecheck diagnostics (b)
shows a clean editor over a failing `morphic check` — the
silent-wrong class refused at every surface since
Phase 116.

**D4 — The `lsp` stub's fate: (a) `morphic lsp` becomes
the real server** in 1.5; the banner leaves its stdout.
The name was reserved for it, and the precedent stands in
`main.rs`, which already suppresses the banner exactly
where stdout is a machine surface. Hiding or renaming the
stub (b) is two user-visible disruptions and a `--help`
that lies by omission in between.

**D5 — Document sync: (a) full-document.** The check is
whole-source by construction (no incremental check API
exists to feed), the pipe is local stdio, and the measured
recheck cost (4 ms typical; 457 ms cold / 215 ms warm on
the corpus's largest module, debug build) is the floor
under either sync kind. Incremental sync (b) adds an
edit-application layer's risk and no capability.

**D6 — VS Code distribution: (a) `.vsix` sideload in
1.5**; marketplace publishing is the owner's later act
(§6). Marketplace during 1.5 (b) makes the campaign wait
on a publisher account and an Azure DevOps PAT — account
acts, not build steps.

**D7 — Milestone shape: (a) one milestone, one gate,
phases 153–157, lockstep 1.5.0.** Splitting (b) — an edge
milestone, then an editor milestone — gates one honesty
story twice and blurs both; the 1.4 discipline (one story
per gate) stands.

---

## 2. The milestone

**Contents.** Four components, and no others: the edge
deploy template (§3, built at Phase 153), the user-package
verb (§4, Phase 154), the LSP diagnostics-only server
(§5, Phase 155), and the VS Code extension (§6,
Phase 156).

**Version.** Lockstep **1.5.0** — crate, both packages,
the release. The stamp is Phase 157's act, after the
gate; Phases 153–156 change no version string.

**The owner's acts.** A live edge deploy (the Cloudflare
account, `wrangler deploy`, any route/DNS binding) and
marketplace publishing (the publisher account + PAT)
remain the owner's, as every registry publish since 1.1
has been; so do the still-pending registry publishes for
1.2.0 / 1.3.0 / 1.4.0. No phase in this milestone touches
a credential, an account, or a live deployment.

**Non-movers.** The library stands at 35 entries
(33 Tier-1 / 2 Tier-2) with its frozen standings; the MCP
server's frozen four-tool surface, the PCC bundle format,
and every frozen text stand as v1.4.0 left them. The 1.5
components consume those surfaces; they do not re-crown,
re-tier, or re-state any of it.

---

## 3. The edge template (D1a)

### 3.1 Home and shape

The template lives at **`templates/cloudflare-workers/`**
in the repo. What a user copies is exactly:

- **`wrangler.toml`** — the Worker's name, its main
  module (`src/index.js`), and a compatibility date.
  Wrangler itself is **not** a repo dependency: it runs
  on the owner's machine (`npx wrangler deploy`); nothing
  in the repo's build invokes it.
- **`src/index.js`** — the worker entry: a module worker
  whose fetch handler imports the shipped package
  (`@thvtzy/morphic`) and routes `POST /fn/<name>` to the
  named export. The handler and the config are the
  template's only platform-shaped code.
- **`package.json`** — depending on `@thvtzy/morphic`
  at the standing package version; the template carries
  no other dependency.
- **`README.md`** — the copy steps (§3.3), the request /
  response forms, and the honest limits (§3.2).

The template must work against the package **as assembled
in-tree** (`packages/npm`) — the Phase 153 smoke (§3.4)
links its `@thvtzy/morphic` import to that assembly, not
to a hypothetical registry copy.

### 3.2 The boundary the template ships as code

JSON cannot carry BigInt, and Morphic `Int` is BigInt
end-to-end at the JS boundary (EMBED_SCOPE D3a). The
handler therefore ships the boundary as code, not as a
warning: on the way in, JSON numbers decode to BigInt
(the package has no Float exports, so a JSON number at
this boundary is an Int; nested arrays and Result values
decode recursively; strings, booleans, `null`, and
functions pass through untouched); on the way out,
BigInts serialize as decimal strings, recursively.
Callback-taking (higher-order) exports cannot receive a
function over JSON — no serialization carries one — so
the fetch route serves data arguments; a host that needs
a callback supplies it in-process, as the §3.4 smoke
demonstrates at the handler. Malformed requests, unknown
export names, and wrong methods are answered with a
**named error shape** (`{error: {code, message}}` with a
matching status) — the handler never throws across the
fetch boundary.

### 3.3 What the user does

Copy the directory, `npm install`, then either
`npx wrangler dev` (local) or `npx wrangler deploy`
(the owner's account act, §2). The README states the
deploy as the owner's act in so many words; no phase of
this milestone deploys anything, and no live-edge
evidence (cold start, real-account behaviour) is claimed
anywhere in the campaign.

### 3.4 The evidence instrument

The template's evidence is the **local no-globals sandbox
smoke**, pinned as a test (Phase 153:
`tests/phase153_edge_template.rs`): the template's worker
module is loaded with the shipped library in a bare V8
context with **no platform globals** (no `process`, no
`require`, no `Buffer`, not even `fetch` — the audit's
`P152|edge|sandbox` harness approach, generalized from
the library alone to the template's entry), and its
handler is driven with synthetic fetch events: a Tier-1
entry, a Tier-2 entry, and a higher-order entry with a
host-supplied callback answer battery-exact, and a
malformed request is answered with the named error shape,
not a throw. The `vm` context is the honest local proxy
for a Workers isolate — it proves the module needs
nothing beyond JavaScript intrinsics; it does not measure
workerd, and the gate (§9, clause 1) rests on the proxy,
named as such.

---

## 4. The user-package verb (D2a)

### 4.1 The verb

`morphic build --package-from <file.morph>` — the
standing `build --package` generalized from the stdlib to
a user module, with package name/version supplied by
flags. It produces the standing npm layout (the
`codegen::package` shell, whose input source is the only
stdlib-specific part) for the user's own specs.

### 4.2 Admission — measured rows only

The binding generator accepts user modules in-process
today, ungated — and prints caller-stated tier/basis
claims **verbatim** (the PCC-1 fact, re-pinned at
`P152|edge|generator`). The verb is therefore the one
place user packages are allowed to come from, and it
feeds the generator **only rows it measured itself**:

- Parse + typecheck the user's module; for each spec, run
  the **seeded, bounded, LLM-free profile** (the standing
  D3a discipline the MCP server also runs).
- The generator's inputs are built **only from that
  run's own measured rows**: the verdict from the run's
  `verification_status`, the battery from its executed
  counts (`passed` / `evaluable` / `total`). No argument,
  flag, file, or caller statement supplies a tier, a
  verdict basis, or a battery figure.
- **Admission rule:** a spec enters the package iff its
  crown is **Verified** (Tier-1 / `z3-verified`) or
  **executed-exact** (`passed == evaluable == total`,
  `total > 0`; Tier-2 / `executed-exact`). Anything else
  — Inconclusive, VerificationFailed, a partial battery,
  an unevaluable spec — is **refused by name, and nothing
  is written**: the `--package` assembly discipline (a
  moved crown fails before any file lands) carried to
  user modules.

The stdlib's crown checks (`verify_crowns`,
`check_tier1_crown`) do **not** transfer — they re-judge
the *shipped* bodies against the prelude. What transfers
is the refusal discipline, the generator, and the layout
shell. Denoted or hand bodies are not admitted by this
verb in 1.5; their re-judgment route (`api::verify_body`,
Phase 149) exists and is named here as the future
widening, not silently folded in.

---

## 5. The LSP diagnostics-only server (D4a, D5a, D3a)

### 5.1 The subcommand and the transport

`morphic lsp` **becomes the real server**: a stdio
language server speaking JSON-RPC 2.0 framed the LSP way
— `Content-Length: N` headers + exactly N bytes — the
framing the MCP core measurably lacks
(`P152|lsp|transfer`). The CLI banner never touches the
stream for this subcommand (the `main.rs` machine-surface
precedent). The dispatch pattern and error vocabulary of
the MCP core (−32700 / −32600 / −32601 / −32602) are
reused; its notification discipline is **not**: LSP's
`didOpen` / `didChange` / `didClose` are id-less and
**must execute** — they carry the document state — and
`publishDiagnostics` is itself an id-less server→client
notification. The inversion is frozen here so no phase
re-derives it.

### 5.2 Surface and lifecycle

Requests: **`initialize`** (answer capabilities:
full-document sync, push diagnostics), **`shutdown`**.
Notifications: `initialized`, **`textDocument/didOpen`**,
**`textDocument/didChange`** (full text), **`textDocument/didClose`**
(clear that file's diagnostics), **`exit`**. Unknown
methods decline −32601 naming the method; malformed
frames are answered −32700 where recoverable; **no input
panics the server**. Document state is a map from URI to
source text. Sync is **full-document** (D5a): every
`didChange` carries the whole text and triggers a
whole-source recheck through `api::check`.

### 5.3 Diagnostics — the two error shapes, honestly

Diagnostics come from the already-shipping `api::check`,
whose measured shape is part of this contract:

- **At most one diagnostic per check.** `CheckReport`'s
  error is singular; the typechecker returns its first
  error and stops. `publishDiagnostics` therefore carries
  0 or 1 entries. Multi-error reporting is checker
  surgery, named in §8 and not in this milestone.
- **Parse errors publish at their true positions**:
  `api::check`'s 1-based line + column convert
  mechanically (line−1, column−1) into the diagnostic's
  range.
- **Typecheck errors publish as ONE file-level
  diagnostic at range (0,0)–(0,0)**, severity Error,
  source `morphic`, whose message is the typecheck
  message **verbatim** (it names its spec), prefixed
  with the label **`unpositioned`** — the frozen form is
  `unpositioned: <verbatim message>`. The position is
  absent because the compiler does not carry it
  (`TypeError.span` is `None` at all 7 construction
  sites; the AST carries no spans), and the diagnostic
  says so in its message rather than letting a squiggle
  at 1:1 imply a position the compiler does not know.
- A clean file publishes an **empty diagnostic set**.

### 5.4 Diagnostics-only means diagnostics-only

No hover, no completion, no go-to-definition, no
formatting, no code actions: the server offers none and
no client invents any (§6). The milestone's label —
diagnostics-only — rides the server's own documentation.

---

## 6. The VS Code extension (D6a)

**Home: `packages/vscode/`** — a `package.json`
contributing the `morphic` language for `.morph` files,
a language configuration (brackets and comment tokens
from the LANGUAGE_REFERENCE's own grammar), a minimal
TextMate grammar for highlighting (declarative JSON —
decoration, with no honesty surface), and a TypeScript
entry (~100 lines) that starts VS Code's generic
LanguageClient against `morphic lsp` (server path
configurable via `morphic.serverPath`, defaulting to a
PATH lookup).

**The one real new dependency** is
`vscode-languageclient` (npm), and it is **quarantined**
in the extension's own `package.json` and `node_modules`,
shipped inside the `.vsix`. The compiler tree's Cargo
graph gains nothing; the library's npm package gains
nothing.

**What the client does NOT do** mirrors §5.4:
diagnostics only — no hover (including no
hover-from-manifest; the manifest/bundle evidence story
is not surfaced in-editor in 1.5), no completion, no
definition, no formatting, no code actions.

**Distribution:** `vsce package` → `.vsix` →
`code --install-extension` (sideload) — zero accounts.
Marketplace publishing is the owner's later act (§2).

**Smoke instrument:** the **scripted LSP-session
transcript** against the real `morphic lsp` binary —
Content-Length-framed `initialize` → `didOpen` a
parse-broken file → the diagnostic at its true position →
`didChange` to a type-broken file → the 0:0 diagnostic
named unpositioned → `didChange` to clean → an empty set →
`shutdown` → `exit` — the Phase 151 gate-transcript
instrument class pointed at the new server. It pins
exactly the wire behavior the thin client depends on;
`@vscode/test-electron` is rejected as gate evidence in
this sandbox (100+ MB download + a display story it does
not have) and remains an owner-machine option.

---

## 7. Budgets

- **npm package: 76,800 B stands** (the figure frozen at
  the 1.3 amendment for the library's package and
  measured at every gate since; the 1.4.0 assembly stands
  at 71,873 B). Any npm package this milestone issues or
  re-issues — the library's 1.5.0 re-issue at Phase 157,
  and user packages assembled by the §4 verb — is measured
  against it at the gate.
- **Playground bundle: 771,072 B stands** (the standing
  budget since Phase 89's owner-approved re-baseline).
  Nothing in 1.5 is expected to move it; the gate
  measures it.
- **The Cargo dependency set is unchanged.** The LSP
  server is a subcommand of the existing binary over the
  existing dependency set (hand-rolled framing +
  `serde_json`, both standing); the extension's
  dependency lives entirely outside the Cargo graph
  (§6). The gate measures the lock graph: no new runtime
  dependency in the default build.
- **No new mass budget is created.** The edge template's
  payload is the package's own (measured at the audit:
  17,278 B of JS), already bound by the npm figure above;
  the `.vsix` and the bundles-style artifacts of this
  milestone carry no frozen figure — freezing a number
  for an unbuilt format would be inventing a budget, and
  the gate binds their *behavior* (§9), not their size.

---

## 8. Exclusions and named follow-ons

**Not in 1.5, recorded in the open:**

- **The span build** — populating `TypeError.span` and
  threading spans through the AST: parser + AST + checker
  surgery at **M–L**, a separate named item sequenced
  only as "after diagnostics-only". The 1.5 server
  touches neither parser nor checker.
- **Multi-error reporting** — a compiler property, not a
  server choice (§5.3); it sits, unsequenced, beside the
  span build.
- **A live deploy** and **marketplace publishing** — the
  owner's account acts (§2), not campaign steps; no
  live-edge evidence is claimed.
- **Anything beyond diagnostics in the editor** — hover,
  completion, definition, formatting, code actions, and
  any in-editor manifest surface (§§5.4, 6).

**Named follow-on milestones** (the standing ladder,
unchanged): **1.6** — the WASM Component Model (L);
**1.7** — the registry, meaning (a): prelude bundles with
install-time re-verification (L), hard-dependent on the
1.4 bundle + verbs. Neither is 1.5 scope, and no 1.5 phase
inherits either silently.

---

## 9. The 1.5 gate

Run by Phase 157 on the final tree, clause by clause, in
the M1-gate style. A failing clause stops the merge.

1. **Edge serves.** The template, populated once with the
   shipped library and once with a user package built by
   the verb, loads in the no-globals sandbox and answers
   its smoke calls battery-exact; the template's only
   platform-shaped code is its handler + config.
2. **The verb is honest.** A user module's package
   carries only measured rows (verdict and battery from
   the synthesis run itself); an uncrownable spec is
   refused by name with nothing written; the package's
   claims reproduce under the standing re-judgment
   routes.
3. **The editor tells the truth.** The scripted LSP
   transcript against the shipped binary: a parse
   diagnostic at its true position; a typecheck
   diagnostic at 0:0 carrying its unpositioned name; a
   clean file yields an empty set; shutdown/exit are
   clean; no banner byte ever touches the stream.
4. **The client is thin.** The `.vsix` packages from the
   extension's own tree; its dependency set is confined
   to `packages/vscode`; clause 3's transcript stands as
   its server-contract evidence; the marketplace is
   untouched.
5. **Footprint + suites.** The compiler tree's dependency
   set is unchanged (Cargo.toml/Cargo.lock diff empty);
   budgets stand (npm ≤ 76,800 B; playground bundle
   ≤ 771,072 B); suites green in the three frozen
   configurations (Phase 128 §9: default /
   bootstrap-codegen / `--no-default-features`) on the
   gate tree, solo runs.

*Closing line (frozen verbatim from the audit's draft):*
"1.5 closes when a proven function can be serving at the
edge from a template anyone can copy — and when the editor
tells the truth about a broken spec: exactly where it broke
when the compiler knows, and plainly unpositioned when it
doesn't."

---

## 10. Phase plan

| Phase | Work | Exit evidence |
|---|---|---|
| 153 | This freeze + the edge template (§3) with its no-globals sandbox smoke | The freeze text; the template at `templates/cloudflare-workers/`; the pinned smoke: Tier-1 / Tier-2 / HO-with-host-callback battery-exact through the handler, malformed answered by name |
| 154 | The user-package verb (§4): `build --package-from`, measured-rows-only admission | A user module packaged end-to-end; an uncrownable spec refused by name with nothing written; generator inputs proven measured-only |
| 155 | The LSP diagnostics-only server (§5): `morphic lsp`, Content-Length framing, full-document sync | The scripted transcript: parse diagnostic positioned, typecheck diagnostic 0:0 named unpositioned, clean → empty set, no banner byte on the stream |
| 156 | The VS Code extension (§6): `packages/vscode`, .vsix packaging | The `.vsix` packages from its own tree; dependency confinement pinned; clause-3-class transcript stands as its server-contract evidence |
| 157 | The 1.5 gate (§9) + merge + release | The gate run clause by clause on the gate tree; the 1.5.0 stamp in lockstep; PR merged on green CI; tag + GitHub Release; publishes, the deploy, and the marketplace left as the owner's acts |

---

## 11. Resolutions

**R1 — The template's home is
`templates/cloudflare-workers/`.** *Basis:* the platform
is the template's identity (D1a), and the repo gains a
`templates/` tree whose later members (other platforms,
other milestones) name themselves the same way. The
audit's sketch name (`templates/edge-cloudflare/`) named
the rung first and the platform second; the freeze names
the platform first, because that is what a user copies.

**R2 — The JSON/BigInt boundary is shipped as handler
code, never as documentation alone.** *Basis:* the
audit's template shape (§1): a boundary every caller hits
on the first request is part of the template's behavior,
and a described-but-unshipped guard is a claim of the
class this project refuses. The decode/encode pair is a
few lines, pinned by the §3.4 smoke's battery-exact
answers.

**R3 — The unpositioned label's frozen form is the
message prefix `unpositioned: `.** *Basis:* D3a names the
cap *in* the diagnostic; of the audit's two carriers
(a `code` field or the message), the message is the one
every client renders, including clients that ignore
`code`. The verbatim typecheck message follows the
prefix unaltered, so the spec name it carries survives.
