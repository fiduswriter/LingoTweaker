#!/usr/bin/env bash
# Build the wasm-bindgen bindings for the browser (ESM) and Node (CJS),
# smoke-test the Node build and publish the `lingotweaker-wasm` npm package
# under a dist-tag. Prereleases default to the `next` tag.
#
# Data is not bundled: the package depends on `lingotweaker-data`, and
# `lingotweaker-wasm/pack` resolves that package locally in Node or fetches the
# per-release GitHub Release assets everywhere else.
#
# Auth: `npm login` for the `johanneswilm` account. With 2FA set NPM_OTP
# (e.g. NPM_OTP="$(otp npm)") to pass --otp non-interactively.
#
# Usage: scripts/release/publish-wasm.sh [--build-only] [extra npm publish args]
#   --build-only : build + smoke-test, do not publish
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
STAGE="${WASM_NPM_DIR:-$ROOT/target/wasm-npm}"
BUILD_ONLY=0
if [ "${1:-}" = "--build-only" ]; then BUILD_ONLY=1; shift; fi

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true

echo "== wasm-pack build (web + nodejs)"
rm -rf "$STAGE"
mkdir -p "$STAGE/web" "$STAGE/node"
wasm-pack build "$ROOT/crates/lt-wasm" --target web --out-dir "$STAGE/web"
wasm-pack build "$ROOT/crates/lt-wasm" --target nodejs --out-dir "$STAGE/node"

# Scope the module system per subdirectory: the root package is ESM while the
# Node build is CommonJS.
printf '{"type":"module"}\n' >"$STAGE/web/package.json"
printf '{"type":"commonjs"}\n' >"$STAGE/node/package.json"
cp "$ROOT/crates/lt-wasm/npm/package.json" "$STAGE/package.json"
cp "$ROOT/crates/lt-wasm/npm/README.md" "$STAGE/README.md"
cp "$ROOT/crates/lt-wasm/npm/pack.js" "$STAGE/pack.js"

echo "== node smoke test"
cargo run --release -p lt-data --bin pack_data --manifest-path "$ROOT/Cargo.toml" \
  -- "$ROOT/data" gn "$STAGE/lt-gn.pack"
LT_WASM_PKG="$STAGE/node" node "$ROOT/tools/wasm/smoke.mjs" "$STAGE/lt-gn.pack"
rm -f "$STAGE/lt-gn.pack"

if [ "$BUILD_ONLY" = 1 ]; then
  echo "== build-only: $STAGE (not published)"
  exit 0
fi

TAG="${NPM_TAG:-next}"
args=(--tag "$TAG")
if [ -n "${NPM_OTP:-}" ]; then
  args+=(--otp "$NPM_OTP")
fi
echo "== npm publish lingotweaker-wasm (dist-tag: $TAG)"
npm publish "$STAGE" "${args[@]}" "$@"
echo "== lingotweaker-wasm publish complete"