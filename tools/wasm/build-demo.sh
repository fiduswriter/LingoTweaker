#!/usr/bin/env bash
# Build the browser demo: per-language data packs plus the wasm bindings.
#
#   tools/wasm/build-demo.sh            # gn it es en
#   LT_WASM_LANGS="gn it" tools/wasm/build-demo.sh
#
# Serve the result over HTTP (ES modules and wasm need a server, not file://):
#
#   python3 -m http.server -d crates/lt-wasm/www 8000
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

langs="${LT_WASM_LANGS:-gn it es en}"

cargo build --release -p lt-data --bin pack_data
mkdir -p crates/lt-wasm/www/packs
for lang in $langs; do
  target/release/pack_data data "$lang" "crates/lt-wasm/www/packs/${lang}.pack"
done

wasm-pack build crates/lt-wasm --target web --out-dir www/pkg

echo
echo "Demo ready. Serve it with:"
echo "  python3 -m http.server -d crates/lt-wasm/www 8000"
