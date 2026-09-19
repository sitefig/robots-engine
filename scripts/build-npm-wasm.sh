#!/usr/bin/env sh
# Builds the engine for the npm package: like build-wasm.sh, plus the diff
# bindings (feature `diff`), into npm/susbot/wasm/ (generated, not committed).
set -eu
cd "$(dirname "$0")/.."
cargo build -p susbot-wasm --features diff --target wasm32-unknown-unknown --release 2>&1 | grep -vE '^\s*(Compiling|Finished)' || true
IN="target/wasm32-unknown-unknown/release/susbot_wasm.wasm"
test -f "$IN" || { echo "build failed: $IN missing" >&2; exit 1; }
OUT=npm/susbot/wasm
rm -rf "$OUT" && mkdir -p "$OUT"
wasm-bindgen "$IN" --target web --out-dir "$OUT"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int -o "$OUT/susbot_wasm_bg.wasm" "$OUT/susbot_wasm_bg.wasm"
fi
rm -f "$OUT/.gitignore"
ls -la "$OUT" | awk '{print $5, $9}' | grep -E 'wasm$|js$'
