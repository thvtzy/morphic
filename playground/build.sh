#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────
#  Build the Morphic playground's WASM package (v0.6 W2).
#
#  Produces playground/pkg/ (wasm-bindgen --target web
#  output) from crates/morphic-wasm — the browser
#  configuration of the same compiler the CLI ships
#  (no Z3, no compiled runner, no LLM; seeded profile).
#  pkg/ is GENERATED: never hand-edit it; it is gitignored
#  and rebuilt by this script.
#
#  Requires: cargo with the wasm32-unknown-unknown target,
#  and a wasm-bindgen CLI matching the crate's bindgen
#  version (0.2.123). Resolution order for the CLI:
#  $WASM_BINDGEN, PATH, target/tools/wasm-bindgen, else a
#  pinned prebuilt download into target/tools/.
# ─────────────────────────────────────────────────────────
set -euo pipefail

PLAYGROUND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLAYGROUND_DIR/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
BINDGEN_VERSION="0.2.123"

cd "$REPO_ROOT"
echo "== cargo build (release, wasm32-unknown-unknown, morphic-wasm)"
cargo build --release --target wasm32-unknown-unknown -p morphic-wasm

BINDGEN="${WASM_BINDGEN:-}"
if [ -z "$BINDGEN" ] && command -v wasm-bindgen >/dev/null 2>&1; then
  BINDGEN="$(command -v wasm-bindgen)"
fi
if [ -z "$BINDGEN" ] && [ -x "$REPO_ROOT/target/tools/wasm-bindgen" ]; then
  BINDGEN="$REPO_ROOT/target/tools/wasm-bindgen"
fi
if [ -z "$BINDGEN" ]; then
  echo "== downloading pinned wasm-bindgen $BINDGEN_VERSION (prebuilt) into target/tools/"
  mkdir -p "$REPO_ROOT/target/tools"
  TARBALL="wasm-bindgen-${BINDGEN_VERSION}-x86_64-unknown-linux-musl.tar.gz"
  curl -sL --fail -o "$REPO_ROOT/target/tools/$TARBALL" \
    "https://github.com/rustwasm/wasm-bindgen/releases/download/${BINDGEN_VERSION}/${TARBALL}"
  tar --no-same-owner -xzf "$REPO_ROOT/target/tools/$TARBALL" -C "$REPO_ROOT/target/tools"
  mv "$REPO_ROOT/target/tools/wasm-bindgen-${BINDGEN_VERSION}-x86_64-unknown-linux-musl/wasm-bindgen" \
     "$REPO_ROOT/target/tools/wasm-bindgen"
  rm -rf "$REPO_ROOT/target/tools/$TARBALL" \
         "$REPO_ROOT/target/tools/wasm-bindgen-${BINDGEN_VERSION}-x86_64-unknown-linux-musl"
  BINDGEN="$REPO_ROOT/target/tools/wasm-bindgen"
fi
echo "== wasm-bindgen: $("$BINDGEN" --version)"

echo "== generating playground/pkg/ (--target web)"
"$BINDGEN" --target web --out-dir "$PLAYGROUND_DIR/pkg" \
  "$REPO_ROOT/target/wasm32-unknown-unknown/release/morphic_wasm.wasm"

WASM_BYTES="$(stat -c%s "$PLAYGROUND_DIR/pkg/morphic_wasm_bg.wasm")"
echo "== done: playground/pkg/morphic_wasm_bg.wasm is ${WASM_BYTES} bytes"
echo "   (W1 post-bindgen baseline: 546229 bytes; W4 owns the size budget)"
