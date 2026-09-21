#!/usr/bin/env bash
# Build the release data artifacts and attach them to the GitHub Release for a
# tag. Release assets are the versioned source for runtime data:
#
#   https://github.com/fiduswriter/LingoTweaker/releases/download/v<ver>/packs/<lang>.pack.gz
#   https://github.com/fiduswriter/LingoTweaker/releases/download/v<ver>/data/<lang>.tar.gz
#   https://github.com/fiduswriter/LingoTweaker/releases/download/v<ver>/manifest.json
#
# Native consumers extract one or more `data/<lang>.tar.gz` into a directory and
# point LT_DATA_DIR at it; wasm consumers fetch `packs/<lang>.pack.gz` and pass
# the inflated bytes to `new LtEngine(lang, pack, optionsJson)`.
#
# Auth: `gh auth login` (or GH_TOKEN) with `contents: write`.
#
# Usage: scripts/release/publish-data.sh <tag, e.g. v0.1.0-alpha.2> [--build-only]
#   --build-only : build + hash, do not create/upload the release
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
tag="${1:?usage: publish-data.sh <tag> [--build-only]}"
BUILD_ONLY=0
[ "${2:-}" = "--build-only" ] && BUILD_ONLY=1

scripts/release/build-data.sh

if [ "$BUILD_ONLY" = 1 ]; then
  echo "== build-only: $ROOT/target/data-dist (not uploaded)"
  exit 0
fi

dist="$ROOT/target/data-dist"
if ! gh release view "$tag" >/dev/null 2>&1; then
  echo "== create release $tag"
  gh release create "$tag" --verify-tag --prerelease \
    --title "$tag" \
    --notes "LingoTweaker $tag. Engine data for this release is attached below (per-language packs and native archives)."
fi

echo "== upload data assets to $tag"
gh release upload "$tag" \
  "$dist"/packs/*.pack.gz \
  "$dist"/data/*.tar.gz \
  "$dist/manifest.json" \
  --clobber
echo "== data publish complete"