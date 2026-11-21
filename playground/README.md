# Morphic Playground (v0.6 W4 — check + synthesis in Web Workers)

A static page that runs the Morphic compiler — compiled to WebAssembly
from the same codebase as the CLI — in the browser. W2 scope was
**check only**; W3 added **synthesis**: per spec, a Synthesize button runs
the seeded playground profile (seed editable in the toolbar, default
`api::PLAYGROUND_DEFAULT_SEED` = 42) and presents the crowned winner with
its executed-test evidence as the exact fraction
passed / evaluable / total, per-test outcomes with not-evaluable reasons,
a verification panel stating that formal verification is unavailable in
this build, and the winner rendered to Rust / C / WAT / Python /
JavaScript / IR with copy + download (the WAT pane is a text preview —
nothing is assembled or executed). W4 moves check and synthesis off the
UI thread into Web Workers, adds Cancel, an enforced bundle budget, and
draft persistence. The check panel's test list remains an *inventory*: a
check has no candidate, so nothing there has been run. Formal
verification (Z3) and the compiled runner are native-only and are
labelled as such on the page.

## The worker shell (W4)

`src/worker.js` is one generic shell, instantiated **twice** by the page:
a check worker and a synthesis worker, so a long synthesis never queues
a check behind it (checks stay live mid-run — the Phase 69 smoke proves
it). Each instance loads the same `pkg/` module the corpus runner
exercises. The page's own draft state never enters the protocol beyond
the source text of the request itself.

Protocol (page ⇄ worker, JSON-shaped messages):

| Direction | Message |
|---|---|
| page → worker | `{ id, kind: 'check', source }` |
| page → worker | `{ id, kind: 'synthesize', source, specName, seed }` |
| worker → page | `{ kind: 'ready' }` — once, after the WASM module initialises |
| worker → page | `{ id, ok: true, report }` — `report` is the pkg function's JSON string |
| worker → page | `{ id, ok: false, error }` |

Requests are processed one at a time, in arrival order, per worker. The
page matches responses to requests by `id` and renders only the newest
check result.

**Cancel** is not a message: the Cancel button terminates the synthesis
worker and spawns a fresh instance. Pending requests reject as
cancelled, no winner is crowned for the cancelled run, and the page is
immediately usable (the Phase 69 smoke measured 88 ms from click to the
cancelled status on the stress case, and a post-cancel synthesis
crowning normally on the respawned worker).

**Fallback.** If Web Workers are unavailable (or a worker fails to
become ready), a visible notice says so and the page calls the same
`pkg/` module directly on the UI thread — the pre-W4 behaviour. In
fallback mode a long synthesis pauses the page and Cancel is
unavailable; the notice states both.

## Draft persistence (W4)

The editor text and the seed persist in the browser's **local storage**
(key `morphic-playground:draft:v1`) and are restored on reload; the
picker shows a "Saved draft (this device only)" state for a restored
draft. Picking a bundled example resets the editor to it (and the draft
then mirrors the editor). An explicit `?example=<file>` link always wins
for display and never overwrites a stored draft by itself. Drafts never
leave the device: the site's only network request is the relative fetch
of the bundled example files themselves — there is no other fetch, XHR,
WebSocket, or beacon in the site sources (grep them), and no absolute
URL appears in `index.html`, `src/`, or `styles.css`.

## Build

```sh
playground/build.sh        # cargo release build + wasm-bindgen into playground/pkg/
```

`pkg/` is generated output (gitignored, never hand-edited). The
wasm-bindgen CLI must match the crate's version (0.2.123); the script
finds it via `$WASM_BINDGEN`, PATH, `target/tools/`, or a pinned
prebuilt download, in that order. The script reports the bundle size
against the budget on every build.

## Bundle budget (W4)

`budget.txt` caps `pkg/morphic_wasm_bg.wasm` at **673,792 bytes** — the
W4 measurement (611,728 B) plus 10% headroom, rounded up to a whole
KiB. `.github/workflows/playground.yml` builds the playground on every
push/PR touching the playground path, fails if the bundle exceeds the
budget, and runs the corpus agreement runner below. History: W1
546,229 B; W2 547,342 B; W3 611,728 B (synthesis renders became
reachable); W4 611,728 B (the dependency diet — rayon removed,
indicatif feature-gated — shrank the wasm *graph* from 67 to 59
packages but changed the bytes by exactly zero: unreachable code was
never linked in). No wasm-opt pass runs anywhere yet; the budget is
enforced on exactly what `build.sh` produces.

## Preview locally

```sh
python3 -m http.server 8123 --directory playground
# open http://localhost:8123/  (add ?example=sort.morph etc. to preselect)
```

Any static server works; the page has no backend and no external
dependencies, and functions offline once loaded.

## The pinned check corpus (W2 gate)

`corpus/cases.json` pins valid + invalid sources (the bundled
examples plus corpus-only cases). The gate runner checks every case
with **both** the native CLI and the WASM module the page loads —
accept/reject must agree on every case:

```sh
cargo build --bin morphic            # native CLI (or set MORPHIC_BIN)
node playground/corpus/run-agreement.mjs
```

## The pinned synthesis corpus (W3 gate)

`corpus/cases.json` also carries `synthesisCases`: four seeded
synthesis runs whose fractions, per-test outcome kinds, winner
status, and winner-body SHA-256 were pinned from the **native
interpreter first** — the config-gated Rust harness
`api::tests::w3_gate_fractions_match_native_interpreter` (run it with
`cargo test --no-default-features --features wasm-playground --lib api::`)
asserts the API's winner body is the pipeline's crowned body and that
`run_spec_tests` on that identical body reproduces the fraction. The
Node runner then asserts the WASM build reproduces every pin, and
audits the site copy: the word "Verified" must not appear in
`index.html`, `src/main.js`, or `styles.css`.

## The stress fixture (W4)

`corpus/stress-biglen.morph` is a deliberately slow case for the
worker/cancel demonstration: `stress_biglen` (`output == len(xs)`)
carries six tests over length-4000 lists, so every candidate evaluation
re-maps and re-traverses them and one seeded run takes ≈6 s — the
slowest case in the playground profile. It is a check case in
`cases.json` (native/WASM agreement) and the Phase 69 browser smoke
drives it through the page: responsiveness during the run, a mid-run
cancel, recovery, and a full run crowning 6/6/6. It is generated by
rule (see its header); do not hand-edit the number rows.

## Bundled examples

`examples/validator.morph`, `examples/tokenizer.morph`, and
`examples/sort.morph` are byte-identical snapshots of the repo files
`examples/self_host/validator.morph`, `examples/functional_tokenizer.morph`,
and `examples/sort.morph` at Phase 67. `examples/invalid-syntax.morph`
and `examples/invalid-type.morph` are deliberate invalid examples,
labelled as such in the picker and in their own headers.
