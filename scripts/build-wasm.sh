#!/usr/bin/env sh
# Builds the engine for the browser: cargo (wasm32) -> wasm-bindgen (ES module
# glue and .d.ts types) -> optional wasm-opt. Output lands in src/client/wasm/,
# which is generated and not committed; the site build copies it to js/wasm/.
# Needs: rustup target add wasm32-unknown-unknown; cargo install
# wasm-bindgen-cli --version <the version in crates/wasm/Cargo.toml>.
set -eu
cd "$(dirname "$0")/.."
PROFILE="${1:-release}"
FLAG=""; [ "$PROFILE" = release ] && FLAG="--release"
cargo build -p susbot-wasm --target wasm32-unknown-unknown $FLAG 2>&1 | grep -vE '^\s*(Compiling|Finished)' || true
IN="target/wasm32-unknown-unknown/$PROFILE/susbot_wasm.wasm"
test -f "$IN" || { echo "build failed: $IN missing" >&2; exit 1; }
rm -rf src/client/wasm && mkdir -p src/client/wasm
wasm-bindgen "$IN" --target web --out-dir src/client/wasm
if command -v wasm-opt >/dev/null 2>&1 && [ "$PROFILE" = release ]; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int -o src/client/wasm/susbot_wasm_bg.wasm src/client/wasm/susbot_wasm_bg.wasm
fi
ls -la src/client/wasm | awk '{print $5, $9}' | grep -E 'wasm|js$'
