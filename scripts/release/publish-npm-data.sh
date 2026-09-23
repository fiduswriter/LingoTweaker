#!/usr/bin/env bash
# Build the per-language data packs and publish them as the `lingotweaker-data`
# npm package. Both engine packages (`lingotweaker` and `lingotweaker-wasm`)
# depend on it, so the data is published once and shared.
#
# Auth: `npm login` for the `johanneswilm` account. With 2FA set NPM_OTP
# (e.g. NPM_OTP="$(otp npm)") to pass --otp non-interactively.
#
# Usage: scripts/release/publish-npm-data.sh [--build-only] [extra npm publish args]
#   --build-only : build + stage, do not publish
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
STAGE="${NPM_DATA_DIR:-$ROOT/target/npm-data}"
BUILD_ONLY=0
if [ "${1:-}" = "--build-only" ]; then BUILD_ONLY=1; shift; fi

scripts/release/build-data.sh --packs-only

echo "== stage $STAGE"
rm -rf "$STAGE"
mkdir -p "$STAGE/packs"
cp "$ROOT/crates/lt-data/npm/package.json" "$STAGE/package.json"
cp "$ROOT/crates/lt-data/npm/index.js" "$STAGE/index.js"
cp "$ROOT/crates/lt-data/npm/index.mjs" "$STAGE/index.mjs"
cp "$ROOT/crates/lt-data/npm/index.d.ts" "$STAGE/index.d.ts"
cp "$ROOT/crates/lt-data/npm/README.md" "$STAGE/README.md"
cp "$ROOT/target/data-dist/packs/"*.pack.gz "$STAGE/packs/"
cp "$ROOT/target/data-dist/manifest.json" "$STAGE/manifest.json"
rm -f "$STAGE/packs/manifest.json"

if [ "$BUILD_ONLY" = 1 ]; then
  echo "== build-only: $STAGE (not published)"
  du -sh "$STAGE"
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
echo "== npm publish lingotweaker-data (dist-tag: $TAG)"
npm publish "$STAGE" "${args[@]}" "$@"
echo "== lingotweaker-data publish complete"