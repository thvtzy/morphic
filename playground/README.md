# Morphic Playground (v0.6 W2 — check MVP)

A static page that runs the Morphic compiler — compiled to WebAssembly
from the same codebase as the CLI — in the browser. W2 scope is
**check only**: parse + typecheck with positioned diagnostics, the
module's spec list, and each spec's in-spec test inventory with
per-test interpreter bindability. The test panel is an *inventory*:
a check has no candidate implementation, so nothing is executed.
Synthesis arrives in W3; formal verification (Z3) and the compiled
runner are native-only and are labelled as such on the page.

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

## Bundled examples

`examples/validator.morph`, `examples/tokenizer.morph`, and
`examples/sort.morph` are byte-identical snapshots of the repo files
`examples/self_host/validator.morph`, `examples/functional_tokenizer.morph`,
and `examples/sort.morph` at Phase 67. `examples/invalid-syntax.morph`
and `examples/invalid-type.morph` are deliberate invalid examples,
labelled as such in the picker and in their own headers.
