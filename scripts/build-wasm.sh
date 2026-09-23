#!/usr/bin/env sh
# Builds the engine for the browser: cargo (wasm32) -> wasm-bindgen (ES module
# glue and .d.ts types) -> optional wasm-opt. Output lands in $OUT_DIR, which
# defaults to dist/wasm here; the website passes its own src/client/wasm, from
# where its build copies the files to js/wasm/. The output is generated and is
# never committed, in either repository.
# Needs: rustup target add wasm32-unknown-unknown; cargo install
# wasm-bindgen-cli --version <the version in crates/wasm/Cargo.toml>.
set -eu
cd "$(dirname "$0")/.."
PROFILE="${1:-release}"
OUT="${OUT_DIR:-$PWD/dist/wasm}"
FLAG=""; [ "$PROFILE" = release ] && FLAG="--release"
cargo build -p susbot-wasm --target wasm32-unknown-unknown $FLAG 2>&1 | grep -vE '^\s*(Compiling|Finished)' || true
IN="target/wasm32-unknown-unknown/$PROFILE/susbot_wasm.wasm"
test -f "$IN" || { echo "build failed: $IN missing" >&2; exit 1; }
rm -rf "$OUT" && mkdir -p "$OUT"
wasm-bindgen "$IN" --target web --out-dir "$OUT"
if command -v wasm-opt >/dev/null 2>&1 && [ "$PROFILE" = release ]; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int -o "$OUT/susbot_wasm_bg.wasm" "$OUT/susbot_wasm_bg.wasm"
fi
ls -la "$OUT" | awk '{print $5, $9}' | grep -E 'wasm|js$'
