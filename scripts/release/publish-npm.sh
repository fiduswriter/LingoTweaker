#!/usr/bin/env bash
# Build the napi-rs Node addon, smoke-test it and publish to npm under a
# dist-tag: stable versions go to `latest`, prereleases to `next` (so `latest`
# keeps pointing at the last stable release). Override with NPM_TAG.
#
# Auth: `npm login` must be logged in for the `johanneswilm` account. With 2FA
# set NPM_OTP (e.g. NPM_OTP="$(otp npm)") to pass --otp non-interactively.
#
# Usage: scripts/release/publish-npm.sh [--build-only] [extra npm publish args]
#   --build-only : build + smoke-test, do not publish
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
NODE_DIR="$ROOT/crates/lt-node"
BUILD_ONLY=0
if [ "${1:-}" = "--build-only" ]; then BUILD_ONLY=1; shift; fi

cd "$NODE_DIR"
if [ ! -d node_modules ]; then
  echo "== npm install (@napi-rs/cli)"
  npm install --no-audit --no-fund
fi

echo "== napi build"
npx napi build --platform --release

addon="$(ls "$NODE_DIR"/*.node | head -1)"
echo "== node smoke test ($addon)"
LT_DATA_DIR="${LT_DATA_DIR:-$ROOT/data}" \
  LT_NODE_ADDON="$addon" \
  node "$NODE_DIR/tests/node/test_smoke.mjs"

if [ "$BUILD_ONLY" = 1 ]; then
  echo "== build-only: $addon (not published)"
  exit 0
fi

TAG="${NPM_TAG:-$(
  case "$(sed -nE 's/^version = "(.*)"/\1/p' "$ROOT/Cargo.toml" | head -1)" in
    *-*) echo next ;;
    *) echo latest ;;
  esac
)}"
args=(--tag "$TAG")
if [ -n "${NPM_OTP:-}" ]; then
  args+=(--otp "$NPM_OTP")
fi
echo "== npm publish (dist-tag: $TAG)"
npm publish "${args[@]}" "$@"
echo "== npm publish complete"