# Morphic Playground (v0.6 W3 — check + synthesis)

A static page that runs the Morphic compiler — compiled to WebAssembly
from the same codebase as the CLI — in the browser. W2 scope was
**check only**; W3 adds **synthesis**: per spec, a Synthesize button runs
the seeded playground profile (seed editable in the toolbar, default
`api::PLAYGROUND_DEFAULT_SEED` = 42) and presents the crowned winner with
its executed-test evidence as the exact fraction
passed / evaluable / total, per-test outcomes with not-evaluable reasons,
a verification panel stating that formal verification is unavailable in
this build, and the winner rendered to Rust / C / WAT / Python /
JavaScript / IR with copy + download (the WAT pane is a text preview —
nothing is assembled or executed). Synthesis runs synchronously on the
main thread in W3; the worker shell is W4's milestone. The check panel's
test list remains an *inventory*: a check has no candidate, so nothing
there has been run. Formal verification (Z3) and the compiled runner are
native-only and are labelled as such on the page.

## Build

```sh
playground/build.sh        # cargo release build + wasm-bindgen into playground/pkg/
```

`pkg/` is generated output (gitignored, never hand-edited). The
wasm-bindgen CLI must match the crate's version (0.2.123); the script
finds it via `$WASM_BINDGEN`, PATH, `target/tools/`, or a pinned
prebuilt download, in that order.

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

## Bundled examples

`examples/validator.morph`, `examples/tokenizer.morph`, and
`examples/sort.morph` are byte-identical snapshots of the repo files
`examples/self_host/validator.morph`, `examples/functional_tokenizer.morph`,
and `examples/sort.morph` at Phase 67. `examples/invalid-syntax.morph`
and `examples/invalid-type.morph` are deliberate invalid examples,
labelled as such in the picker and in their own headers.
