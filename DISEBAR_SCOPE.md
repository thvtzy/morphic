# DISEBAR_SCOPE.md — Phase 149: the frozen 1.4 scope + the gate

The ratification + freeze document for **1.4 — the agent
channel + the honesty engine**: the Disebar rung's first
milestone, carrying the **MCP server** and **proof-carrying
code** and nothing else. Source material:
[`DISEBAR_AUDIT.md`](DISEBAR_AUDIT.md) (Phase 148 — the rung
measured component by component: the MCP surface of its §1,
the PCC boundary of its §6, the sequence and milestone shape
of its Synthesis, the gate sketch of its S-gate; probe
artifact `tests/phase148_disebar_probes.rs`, 7 probes, every
audit claim a `P148|` line), over the API frozen by the v0.6
work (`src/api.rs`, Phase 66) and the package surface frozen
by [`EMBED_SCOPE.md`](EMBED_SCOPE.md) (§2.4 the manifest).

**Ratification.** On 2026-11-24 the owner ratified the
audit's recommended package whole — *"Gas rekomendasi
audit — D1a sampe D8a semua"* — adopting decisions
**D1a–D8a** as stated in §1 below. Per D8a this milestone is
one honesty story: **1.4 = MCP + PCC, one gate.** The rung's
other components are named follow-on milestones (§6),
recorded here as deferrals with their prices — they are
**not** 1.4 scope, and no 1.4 phase inherits them silently.
This document freezes:

- **the milestone's contents** (§2): MCP + PCC only, the
  lockstep version, and what stays the owner's act.
- **the MCP contract** (§3): the transport and protocol
  subset, the frozen four-tool surface with its argument
  sets, the verify tool's body sources, the calling
  discipline, and the honesty semantics of verdicts served
  over the protocol.
- **the PCC obligations** (§4): what Phase 150 must build —
  the re-verification bundle, the two consumer verbs, the
  tamper evidence — stated now so 150 executes against a
  frozen contract rather than inventing one.
- **the budgets** (§5): the figures the gate measures.
- **the exclusions** (§6): what is not in this milestone,
  and what is not in the rung at all.
- **the gate** (§7): the audit's 5-clause sketch finalized
  into gate text, closing line verbatim from the audit's
  draft.
- **the phase plan** (§8): Phases 149–151.

This phase (149) lands the freeze and the MCP server
against §3. Phase 150 builds §4. Phase 151 runs §7.

---

## 1. The ratified decisions (D1a–D8a)

**D1 — First component: (a) the MCP server.** The wedge's
own channel: the highest distribution leverage per price
on the rung, zero new dependencies, and its protocol core
is reused by the LSP later.

**D2 — MCP protocol route: (a) hand-rolled JSON-RPC 2.0
over stdio, zero new dependencies.** `serde_json` is
already in the tree; the needed subset is initialize /
tools-list / tools-call / ping. The `rmcp` SDK (b) remains
available if the protocol surface outgrows the subset;
nothing in (a) forecloses it. HTTP transport (c) is not
first: MCP clients live on stdio today.

**D3 — The server's synthesis profile: (a) the seeded,
bounded, LLM-free profile only** (`api::playground_config`
+ the §3.4 size cap). A tool other programs call must be
bounded and reproducible: same (source, seed) → same
winner. LLM seeds are neither, and a server that phones
an LLM is a different trust surface.

**D4 — The PCC mechanism: (a) re-verification.** A bundle
of (spec, IR-serialized body, battery, claimed verdict) +
a consumer verb that re-judges it; the stdlib covered
first by the S-sized `verify --library` self-check. Z3
proof objects (b) are rejected on the evidence story
(§4.4); signed manifests (c) are a claim with a signature
— still a claim.

**D5 — The registry's meaning: (a) prelude bundles with
install-time re-verification.** Recorded here because it
fixes §4's customer: the bundle format Phase 150 builds
is the registry's future package shape. The registry
itself is milestone 1.7 (§6), not 1.4.

**D6 — LSP depth in the rung: (a) diagnostics-only
first**, labelled as such; the span build a separate
later phase. A 1.5 matter (§6); 1.4 does not touch the
`lsp` subcommand or its stub.

**D7 — Component Model timing: (a) after MCP / PCC /
edge, as its own milestone** (1.6, §6). Its type decisions
stand frozen in EMBED_SCOPE §3 and do not rot.

**D8 — Milestone shape: (a) 1.4 = MCP + PCC, one gate;**
the rest of the rung as named follow-on milestones. One
milestone per honesty story (the 1.3/quantified-mod
lesson): gating unlike stories together (b) blurs them;
ad-hoc components (c) abandon the freeze/gate discipline
that makes this project's claims checkable.

---

## 2. The milestone

**Contents.** Two components: the MCP server (§3, built
at Phase 149) and proof-carrying code (§4, built at
Phase 150). No other Disebar component is in 1.4.

**Version.** Lockstep **1.4.0** — crate, both packages,
the release. The number is free: the quantified-mod
campaign closed on 2026-11-24 without using it (its
closure record stands on its branch), and this milestone
takes it. The stamp is Phase 151's act, after the gate;
Phases 149–150 change no version string.

**The owner's acts.** The registry publishes for 1.4.0
(npm + crates.io, from the tagged tree) remain the
owner's, as every publish since 1.1 has been; so do the
still-pending 1.2.0 / 1.3.0 publishes. No phase in this
milestone touches a credential or a registry.

**Non-movers.** The library stands at 35 entries
(33 Tier-1 / 2 Tier-2) with its frozen standings; the
playground, the packages' contents, and every frozen
text stand as v1.3.0 left them. The MCP server consumes
the library's data; it does not re-crown, re-tier, or
re-state any of it.

---

## 3. The MCP contract

### 3.1 Transport and protocol subset

The server is a subcommand — `morphic mcp` — serving
**stdio**: newline-delimited JSON-RPC 2.0, one message
per line in, one response line per request out, flushed
per response, EOF a clean exit. The CLI banner never
touches the stream (the `stdlib-asset` precedent: a
subcommand whose stdout is a machine surface runs
without it).

The frozen method subset:

| Method | Answer |
|---|---|
| `initialize` | the client's protocol version echoed (or `2024-11-05` when unnamed), `capabilities: {tools: {}}`, `serverInfo: {name: "morphic", version: <build version>}` |
| `ping` | an empty result |
| `tools/list` | the frozen tool definitions (§3.2) |
| `tools/call` | a tool result: `content` (one text item carrying the tool's DTO as JSON) + `isError` |
| `notifications/*` | no response, ever |

Discipline, all pinned: a message without an `id` is a
notification — never answered, never executed. A batch
array is declined `-32600` (the stdio shape is one
message per line). Malformed JSON is answered `-32700`
with a null id and the server lives on. An unknown
method is declined `-32601`, naming it. An unknown tool
is declined `-32602`, naming the tool and the frozen
surface. **No input panics the server**: every failure
is a protocol error or a tool error (§3.2), and the
process answers the next line.

### 3.2 The tool surface (frozen)

Exactly four tools. The argument sets below are the
whole surface — every schema carries
`additionalProperties: false`, and **no tool accepts a
filesystem path, a key, or a model argument**: tools
take source text as arguments, and that is the only
way in (§3.4).

**`check`** — `{source}` → the API's `CheckReport`
(parse + typecheck, spec inventory with per-test
bindability). A source that fails checking is a
*successful* call carrying the failing report: the
report is the tool's product, not a protocol failure.

**`synthesize`** — `{source, spec?, seed?}` → the
API's `SynthesisReport` for the named spec (default:
the module's first) under the §3.4 profile: the crowned
body in the pipeline's identity form (its Debug
rendering), its verdict, its executed counts and
per-test outcomes, its renders. A stage failure (parse,
typecheck, synthesis, selection) is a tool error
carrying the `StageError`.

**`verify`** — `{source, spec, body}` → the standalone
verify entry point (§3.5): the verifier's verdict for
one candidate body against one spec, the per-row
results, and the executed counts. Body sources are
§3.3's two.

**`library`** — `{entry?}` → without `entry`: the
shipped library's manifest rows (name, signature,
tier, verdictBasis, battery, doc — the manifest's own
data, EMBED_SCOPE §2.4, read from the generated
manifest, never restated). With `entry`: that entry's
manifest row + its spec (the AST's own serialization)
+ its shipped body in the pipeline's identity form.
This is the surface §4's bundles ride on at Phase 150.
An unknown entry declines by name.

### 3.3 The verify tool's body sources

A candidate body is never transported as text in this
phase — no body text format exists yet (§4 builds it).
The verify tool names its body from exactly two
sources:

- **`{"kind": "library", "entry": "<name>"}`** — the
  shipped body of a standard-library entry
  (`stdlib::shipped_body`). An entry that ships no body
  declines by name.
- **`{"kind": "denoted"}`** — the body the named spec's
  own rows denote through the standing resolver
  (`SpecIndex::callee_body`: equation rows, guard
  tables, and the resolver chain's other denoting
  forms). A spec whose rows denote no body declines by
  name, saying so.

Any other `kind` declines by name. The third source —
a body carried as serialized IR text — is a named
consequence of §4's format and arrives with Phase 150,
as a freeze amendment or as 150's recorded extension,
never as a silent widening of this section.

### 3.4 The calling discipline (D3a)

Synthesis through the server runs **only** the seeded,
bounded, LLM-free profile — `api::playground_config`,
the playground's own: LLM-free by construction (the
profile carries no key and no LLM call exists in it),
iteration/population caps as the profile freezes them,
the engine RNG seeded per call. The `seed` argument
defaults to the playground's pinned seed
(`PLAYGROUND_DEFAULT_SEED`, 42). **Same (source, seed)
→ same crowned body, through the server** — the
reproducibility a tool owes its callers.

**Size cap.** Every `source` argument is capped at
**262,144 bytes** (R2): a larger source declines by
name, before any stage runs, directed at the CLI. The
cap bounds a tool call's latency; it is a server
discipline, not a language limit.

**No ambient authority.** The server never shells out,
never reads or writes files, and holds no state between
calls that any verdict depends on. Its tools are pure
functions of their arguments.

### 3.5 Verdict honesty at the boundary

**A verdict the server serves is the native pipeline's
own, produced in-process at call time** from the
arguments the caller sent: the synthesize tool's winner
status is the pipeline's verdict for that run; the
verify tool's status is `Verifier::verify_one`'s output
for that body and that spec, now. Nothing is cached
across calls, nothing is precomputed at build time,
and **no verdict is ever accepted from the caller** —
there is no argument on any tool that states one.

`verification_available` rides both verdict-bearing
reports and states the build's own fact. The server
**ships only in the Z3-enabled native build**: the CLI
refuses the subcommand by name in any other build (R3),
because a server that cannot verify could only dress
executed evidence as verdicts. Inside the shipped
build, a wrong body stands **VerificationFailed**
through the tool, never dressed — the boundary's teeth,
pinned at Phase 149 and re-run at the gate.

The standalone verify entry point itself —
`api::verify_body` — is part of this frozen surface:
verdict + per-row results from `verify_one`, executed
counts from the interpreter through the index-carrying
route (Phase 135's judging route), the body's source
named in the report (`library:<entry>` | `denoted`).

---

## 4. The PCC obligations (Phase 150's contract)

The mechanism is **re-verification** (D4a): the
evidence for a verdict is a bundle a consumer re-judges
with the shipped binary — not a proof object, not a
signature over a claim. Phase 150 builds exactly this:

### 4.1 The bundle

A **re-verification bundle** for one function carries:
its spec, its body in the IR text serialization
(§4.2), the claimed verdict, its battery, and its
crown route (the EMBED_SCOPE R11 routes: the seeded
pipeline's crown, or the production verifier's crown
check — the route is part of the evidence, because the
two routes are different judgments and the consumer
re-runs the one the bundle names). A bundle is a file
a stranger can hold: `morphic verify-bundle <file>`
re-judges (spec, body, battery) through the named
route and compares against the claimed verdict —
**exit 0 is the evidence holding; a mismatch fails by
name**, naming the row or the count that moved.

### 4.2 The IR text serialization

The one real build inside PCC (audit §6, priced M):
the IR's Debug rendering is the pipeline's body
identity fingerprint but is not a parser input and is
not versioned. Phase 150 makes a text serialization of
`IRNode` that is **stable** (the same body serializes
byte-identically), **versioned** (the format carries
its version; a reader refuses a version it does not
know, by name), and **parseable** (a reader reconstructs
the body the writer held — round-trip identity on the
shipped bodies is the format's own pin). Bodies the
format cannot carry decline by name; nothing is
approximated into the format.

### 4.3 The stdlib self-check (the S half)

For the library's own entries the consumer's compiler
already contains the bodies (the prelude), so no
format work is needed to cover the shipped packages'
claims: **`morphic verify --library`** re-runs every
shipped entry's crown check against the installed
compiler's own library and reports per-entry standings
— the packages' manifest rows, re-judged by the
holder's own binary. It ships at Phase 150 alongside
the bundle; the bundle is what *user* functions (and
1.7's registry bundles) need.

### 4.4 What is excluded, and why

**Z3 proof objects are not the certificate** — rejected
as a mechanism, not deferred (audit §6): the pinned z3
0.20 crate exposes no portable, independently checkable
proof format a stranger could validate without the
same solver build; a solver transcript is one solver's
derivation, checkable in practice only by re-solving —
which is re-verification with extra steps. **Signed
manifests** are likewise not evidence: a claim with a
signature is still a claim. The manifest's own language
is part of 150's surface work: its rows point at the
bundle (and at `verify --library`) as the evidence for
what they state.

---

## 5. Budgets

- **Playground bundle: 771,072 B stands** (the standing
  budget since Phase 89's owner-approved re-baseline).
  Nothing in 1.4 is expected to move it; the gate
  measures it.
- **The MCP server adds no artifact and no
  dependency.** It is a subcommand of the existing
  binary over the existing dependency set (gate clause
  4 measures the lock graph: no new runtime dependency
  in the default build). Its mass effect is the
  binary's own, reported at the gate for the record —
  no figure is frozen here because no budget has ever
  bound the native binary.
- **The npm package budget stands at 76,800 B** for
  its 1.4.0 re-issue at Phase 151 (the figure the 1.3
  gate measured against; the 1.3.0 assembly stands at
  71,265 B). The facade carries no size budget
  (EMBED_SCOPE R8, standing).
- **Phase 150's bundle format** gets its per-entry mass
  measured and stated at 150; no figure is frozen now
  because the format does not exist yet — freezing a
  number for an unbuilt format would be inventing a
  budget, and the gate (clause 3) binds the bundles'
  *re-verification*, not their size.

---

## 6. Exclusions and named follow-ons

**Named follow-on milestones** (recorded, priced by the
audit, frozen at their own freezes — not 1.4 scope):

- **1.5** — edge deploy (the deploy template + the
  user-package verb, S–M) and LSP diagnostics-only +
  the VS Code client (M + S). The 1.5 freeze decides
  the `lsp` stub's fate deliberately; **Phase 149 does
  not touch the `lsp` subcommand.**
- **1.6** — the WASM Component Model (L), inheriting
  EMBED_SCOPE §3's WIT column and the audit's post-G4
  fork (WIT has no function-typed values: the six HO
  exports arrive as monomorphic instantiations or
  host-imported callbacks).
- **1.7** — the registry, meaning (a): prelude bundles
  with install-time re-verification (L), hard-dependent
  on §4's bundle + verbs.
- **The span build + full LSP** (M–L + M): named,
  priced, deliberately unsequenced beyond "after the
  diagnostics-only server".

**Not in the rung at all:** a real module system
(audit §3b — a language milestone with its own gate,
not a distribution step; the registry's import-based
meaning stays blocked on it, plainly); the full MCP
surface (HTTP transport, auth, streaming, prompts and
resources beyond the frozen tool set); Z3 proof objects
(§4.4 — rejected, not deferred).

**Not in 1.4:** any change to the library's entries,
tiers, or bodies; any package re-issue before the
gate; any version stamp before Phase 151; the §15
grammar buglet (a one-line doc fix, the owner's to
schedule — recorded by the audit, not this milestone's
act).

---

## 7. The 1.4 gate

Run by Phase 151 on the final tree, clause by clause,
in the M1-gate style. A failing clause stops the merge.

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
   seeded bounded profile — same (source, seed) → same
   winner body, through the server; no served verdict
   is cached or caller-stated.
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
5. **Suites green** in the three frozen configurations
   (Phase 128 §9: default / bootstrap-codegen /
   `--no-default-features`), solo runs.

*Closing line (frozen verbatim from the audit's
draft):* "1.4 closes when an AI agent can ask Morphic
for a proven function through the tools agents already
speak — and when anyone holding the artifact can
re-run the proof obligation themselves instead of
trusting the row that claims it."

---

## 8. Phase plan

| Phase | Work | Exit evidence |
|---|---|---|
| 149 | This freeze + the MCP server (§3): the stdio JSON-RPC core, the four tools over `api`, the standalone verify entry point | The protocol/tool pins + an end-to-end stdio transcript: a synthesize call returns a Verified crown for a pinned corpus spec through the shipped binary |
| 150 | PCC (§4): `verify --library` (S) + the IR text serialization, the bundle format, `verify-bundle` (M) | Every shipped entry's bundle re-verifies; a tampered bundle fails by name; the self-check passes |
| 151 | The 1.4 gate (§7) + merge + release | The gate run clause by clause on the gate tree; the 1.4.0 stamp in lockstep; PR merged on green CI; tag + GitHub Release; publishes left as the owner's acts |

---

## 9. Resolutions

**R1 — The verify tool's body sources are exactly
{library, denoted} at 149.** *Basis:* no body text
transport exists in the tree (the IR's Debug form is
an identity fingerprint, not a parser input), and the
audit priced the serialization as Phase 150's build.
Inventing an ad-hoc body syntax at 149 would freeze a
second format by accident; the two named sources cover
the tool's measured uses (re-judging shipped bodies;
judging denoted candidates), and §3.3 names the third
source as 150's consequence.

**R2 — The source cap is 262,144 bytes.** *Basis:*
measurement — the largest module in the standing
corpus (`examples/self_host/codegen.morph`) is
152,251 B, so the cap covers every real module with
headroom while still bounding a call's latency (the
audit's §1(d) discipline). An over-cap decline names
the cap and directs the caller to the CLI, whose
surface has no cap.

**R3 — The server ships in the Z3-enabled build only,
refusing by name elsewhere.** *Basis:* the audit's §1
honesty ruling ("the server should ship only in the
native (Z3) configuration, and say so"), on the
`stdlib-asset` / `--package` precedent of refusing by
name in a build that cannot serve honestly. The
protocol handler itself stays configuration-free code,
testable in-process in every configuration — the
refusal lives at the subcommand, where the shipping
decision lives.

**R4 — Four tools, not the audit sketch's ~six.**
*Basis:* the sketch's render tool already rides inside
the synthesis report (the DTO has carried all six
renders since W3 — a separate render tool would serve
the same bytes twice); its list/fetch pair is one
library tool's two forms; its spec-template resource
is not built — nothing measured needs it, and the
frozen surface is the smaller for it. Widening later
is an amendment; narrowing a shipped surface is not.

**R5 — Framing is newline-delimited JSON.** *Basis:*
the MCP stdio transport's standing shape (one JSON
object per line); `Content-Length` framing is the
LSP's dialect. The two shells will share the JSON-RPC
*core* (D1's reuse) while each keeps its own framing.

**R6 — This milestone takes version 1.4.0.** *Basis:*
the quantified-mod campaign closed on 2026-11-24
recording its gate unmet and using no version; the
number is therefore this milestone's in lockstep, and
no artifact in the tree carries a conflicting 1.4.0
claim.
