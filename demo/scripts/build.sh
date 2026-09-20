#!/usr/bin/env bash
# Build everything the demo needs: wasm bindings, data packs and the Vite
# bundle. Used by the Pages workflow and useful locally.
#
#   demo/scripts/build.sh
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$script_dir/../.." && pwd)"
cd "$root"

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true

wasm-pack build crates/lt-wasm --target web --out-dir ../../demo/pkg
"$script_dir/build-packs.sh"

cd demo
if [ ! -d node_modules ]; then
  npm ci
fi
npm run build
