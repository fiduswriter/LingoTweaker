#!/usr/bin/env bash
# Build the per-language data packs and publish them to npm as
#
#   lingotweaker-data          code-only loader (packPath/languages/manifest)
#   lingotweaker-data-<lang>   one package per language (packs/<lang>.pack.gz
#                              plus that language's manifest slice)
#
# npm's registry rejects one big multi-language tarball with E413 Payload Too
# Large (the 38-language package passed at 175 MB but no longer at ~207 MB), so
# each language is published on its own — the same model the PyPI data
# distributions use. The base package keeps the `lingotweaker-data` name, so
# the engine packages (`lingotweaker`, `lingotweaker-wasm`) keep depending on
# it; it no longer bundles packs, only the manifest and the loader code. The
# per-language packages use the engine version, and a package whose exact
# version is already on the registry is skipped (like the PyPI script's
# `on_pypi`), so re-running the script is resumable.
#
# Auth: `npm login` for the `johanneswilm` account. With 2FA set NPM_OTP
# (e.g. NPM_OTP="$(otp npm)") to pass --otp non-interactively. With npm trusted
# publishing (OIDC), every package name — including each
# `lingotweaker-data-<lang>` — needs its Trusted Publisher configured once on
# npmjs.com before its first publish.
#
# Usage: scripts/release/publish-npm-data.sh [--build-only] [extra npm publish args]
#   --build-only : build + stage, do not publish
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
STAGE="${NPM_DATA_DIR:-$ROOT/target/npm-data}"
LANG_STAGE="${STAGE}-langs"
BUILD_ONLY=0
if [ "${1:-}" = "--build-only" ]; then BUILD_ONLY=1; shift; fi

scripts/release/build-data.sh --packs-only

DIST="${DIST_DIR:-$ROOT/target/data-dist}"
manifest="$DIST/manifest.json"
version="$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version'])" "$manifest")"
langs="$(python3 -c 'import json,sys; print(" ".join(sorted(json.load(open(sys.argv[1]))["languages"])))' "$manifest")"

echo "== stage base $STAGE"
rm -rf "$STAGE" "$LANG_STAGE"
mkdir -p "$STAGE" "$LANG_STAGE"
cp "$ROOT/crates/lt-data/npm/package.json" "$STAGE/package.json"
cp "$ROOT/crates/lt-data/npm/index.js" "$STAGE/index.js"
cp "$ROOT/crates/lt-data/npm/index.mjs" "$STAGE/index.mjs"
cp "$ROOT/crates/lt-data/npm/index.d.ts" "$STAGE/index.d.ts"
cp "$ROOT/crates/lt-data/npm/README.md" "$STAGE/README.md"
# The manifest stays in the base package: it describes every language's pack
# (bytes/sha256) without bundling any of them, and `file` paths resolve inside
# the per-language packages.
cp "$manifest" "$STAGE/manifest.json"

for lang in $langs; do
  pkg="lingotweaker-data-$lang"
  dir="$LANG_STAGE/$pkg"
  echo "== stage $pkg"
  mkdir -p "$dir/packs"
  cp "$DIST/packs/$lang.pack.gz" "$dir/packs/"
  # The language's manifest slice: the full manifest restricted to this
  # language, so each package describes exactly what it ships and stays
  # independent of the other languages' data changes.
  python3 - "$manifest" "$dir/manifest.json" "$lang" <<'PY'
import json, sys

src, dst, lang = sys.argv[1:4]
m = json.load(open(src))
m["languages"] = {lang: m["languages"][lang]}
open(dst, "w").write(json.dumps(m, indent=2) + "\n")
PY
  cat >"$dir/package.json" <<EOF
{
  "name": "$pkg",
  "version": "$version",
  "description": "LingoTweaker runtime data pack for language '$lang'",
  "license": "LGPL-2.1-or-later",
  "repository": {
    "type": "git",
    "url": "git+https://github.com/fiduswriter/LingoTweaker.git"
  },
  "homepage": "https://fiduswriter.github.io/LingoTweaker/",
  "keywords": [
    "proofreading",
    "grammar",
    "spellcheck",
    "nlp",
    "language",
    "data"
  ],
  "exports": {
    "./packs/*": "./packs/*",
    "./manifest.json": "./manifest.json",
    "./package.json": "./package.json"
  },
  "files": [
    "packs",
    "manifest.json",
    "README.md"
  ],
  "engines": {
    "node": ">= 18"
  }
}
EOF
  cat >"$dir/README.md" <<EOF
# LingoTweaker data ($lang)

Runtime data for the LingoTweaker proofreading engine, language \`$lang\`.
Installing this package next to the code-only \`lingotweaker-data\` loader is
enough for \`packPath("$lang")\` to find the pack:

\`\`\`js
const { packPath } = require("lingotweaker-data");
const { Engine } = require("lingotweaker");

const engine = new Engine("$lang-US", { dataDir: packPath("$lang") });
\`\`\`

This is the same pack attached to each LingoTweaker GitHub Release. Vendored
rule data, dictionaries and models keep their upstream licenses; see
\`THIRD_PARTY_NOTICES.md\` in the repository.

License (packaging code): LGPL-2.1-or-later.
EOF
done

if [ "$BUILD_ONLY" = 1 ]; then
  echo "== build-only: $STAGE (base) + $LANG_STAGE (per-language, $(echo $langs | wc -w) languages); not published"
  du -sh "$STAGE"
  du -sh "$LANG_STAGE"/* | sort -k2
  exit 0
fi

TAG="${NPM_TAG:-$(
  case "$version" in
    *-*) echo next ;;
    *) echo latest ;;
  esac
)}"
args=(--tag "$TAG")
if [ -n "${NPM_OTP:-}" ]; then
  args+=(--otp "$NPM_OTP")
fi
args+=("$@")

# Is <name>@<version> already on the registry? (Like the PyPI script's
# on_pypi: any lookup failure counts as "not there", and a publish that raced
# another run is caught by the post-failure re-check below.)
on_npm() {
  npm view "$1@$2" version >/dev/null 2>&1
}

# Publish one staged package, backing off and retrying transient failures so
# the step is resumable.
publish_one() {
  dir="$1"
  name="$2"
  attempt=1
  while [ "$attempt" -le 5 ]; do
    if npm publish "$dir" "${args[@]}"; then
      return 0
    fi
    if on_npm "$name" "$version"; then
      echo "== $name@$version is on the registry (published after all)"
      return 0
    fi
    delay=$((attempt * 15))
    echo "== npm publish failed for $name (attempt $attempt); retrying in ${delay}s" >&2
    sleep "$delay"
    attempt=$((attempt + 1))
  done
  return 1
}

# Skip anything whose exact version is already on the registry, so unchanged
# packages (and re-runs after a partial failure) are never re-published.
publish_pkg() {
  dir="$1"
  name="$2"
  if on_npm "$name" "$version"; then
    echo "== up to date: $name@$version"
    return 0
  fi
  echo "== npm publish $name@$version (dist-tag: $TAG)"
  if publish_one "$dir" "$name"; then
    sleep 3
    return 0
  fi
  return 1
}

failed=""

# The base package first: the engine packages depend on it and are published
# by later workflow jobs.
if ! publish_pkg "$STAGE" lingotweaker-data; then
  failed="$failed lingotweaker-data"
fi

for lang in $langs; do
  pkg="lingotweaker-data-$lang"
  if ! publish_pkg "$LANG_STAGE/$pkg" "$pkg"; then
    failed="$failed $pkg"
  fi
done

echo "== npm data publish complete"
if [ -n "$failed" ]; then
  echo "== FAILED publishes (re-run this workflow to resume):$failed" >&2
  exit 1
fi