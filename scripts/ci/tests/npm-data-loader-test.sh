#!/usr/bin/env bash
# Smoke test for the code-only `lingotweaker-data` npm loader: stage the loader
# from crates/lt-data/npm plus fake per-language packages into fake node_modules
# trees and assert that `packPath` resolves the installed packs — CommonJS and
# ESM entries, sibling layout (npm's flat node_modules) and non-sibling layout
# (pnpm-style store + project symlink, exercising the cwd fallback). Needs no
# npm install and no network; everything is faked locally.
# Run: scripts/ci/tests/npm-data-loader-test.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
NPM_SRC="$ROOT/crates/lt-data/npm"
fails=0

check() { # <exit-code> <name>
  if [ "$1" = 0 ]; then
    printf 'ok   %s\n' "$2"
  else
    printf 'FAIL %s\n' "$2" >&2
    fails=$((fails + 1))
  fi
}

# Stage the loader (crates/lt-data/npm) into <base>/lingotweaker-data and fake
# `lingotweaker-data-<lang>` packages for <langs...> under <langdir>. The
# loader manifest always lists both `da` and `sv` (the real manifest.json is
# produced by scripts/release/build-data.sh at staging time and lists every
# published language), while only <langs...> get an installed package — so a
# listed language without its package exercises the install-hint error path.
stage_tree() {
  base="$1"
  langdir="$2"
  shift 2
  langs="$*"
  mkdir -p "$base/lingotweaker-data"
  cp "$NPM_SRC/index.js" "$NPM_SRC/index.mjs" "$NPM_SRC/index.d.ts" \
    "$base/lingotweaker-data/"
  cat >"$base/lingotweaker-data/package.json" <<'EOF'
{
  "name": "lingotweaker-data",
  "version": "0.0.0-test",
  "main": "./index.js",
  "types": "./index.d.ts"
}
EOF
  python3 - "$base/lingotweaker-data/manifest.json" <<'PY'
import json, sys

dst = sys.argv[1]
m = {
    "version": "0.0.0-test",
    "upstream_commit": "test",
    "languages": {
        lang: {"pack": {"file": f"packs/{lang}.pack.gz", "bytes": 5, "sha256": lang}}
        for lang in ("da", "sv")
    },
}
open(dst, "w").write(json.dumps(m, indent=2) + "\n")
PY
  for lang in $langs; do
    pkg="lingotweaker-data-$lang"
    mkdir -p "$langdir/$pkg/packs"
    printf 'pack\n' | gzip -n >"$langdir/$pkg/packs/$lang.pack.gz"
    cat >"$langdir/$pkg/package.json" <<EOF
{
  "name": "$pkg",
  "version": "0.0.0-test",
  "exports": {
    "./packs/*": "./packs/*",
    "./manifest.json": "./manifest.json",
    "./package.json": "./package.json"
  }
}
EOF
  done
}

# Shared assertion body (uses `require`; `loader` is bound by the prologue —
# CJS: require("lingotweaker-data"), ESM: await import(<index.mjs>)).
assert_body() {
  cat <<'EOF'
const assert = require("node:assert");
const { existsSync } = require("node:fs");

assert.deepStrictEqual(loader.languages(), process.env.EXPECT_LANGS.split(","));
assert.strictEqual((loader.dataManifest ?? loader.manifest)().version, "0.0.0-test");
assert.strictEqual(
  (loader.dataManifest ?? loader.manifest)().languages.da.pack.file,
  "packs/da.pack.gz",
);
for (const lang of process.env.PACK_LANGS.split(",")) {
  const p = loader.packPath(lang);
  assert.ok(p.endsWith(`lingotweaker-data-${lang}/packs/${lang}.pack.gz`), p);
  assert.ok(existsSync(p), p);
}
assert.throws(() => loader.packPath("zz"), /no pack for "zz"/);
if (process.env.EXPECT_MISSING) {
  const missing = process.env.EXPECT_MISSING;
  assert.throws(
    () => loader.packPath(missing),
    new RegExp(`not installed.*lingotweaker-data-${missing}`),
  );
}
console.log("ok");
EOF
}

# 1. Flat layout: per-language packages are siblings of the loader, as in npm's
#    default node_modules. CJS and ESM entries.
tmp1="$(mktemp -d)"
stage_tree "$tmp1/node_modules" "$tmp1/node_modules" da sv
{
  printf '%s\n' 'const loader = require("lingotweaker-data");'
  assert_body
} >"$tmp1/test.cjs"
if (cd "$tmp1" && PACK_LANGS="da,sv" EXPECT_LANGS="da,sv" node test.cjs); then
  check 0 "flat layout: CJS entry resolves sibling packages"
else
  check 1 "flat layout: CJS entry resolves sibling packages"
fi
{
  printf '%s\n' 'import { createRequire } from "node:module";'
  printf '%s\n' 'const require = createRequire(import.meta.url);'
  printf '%s\n' "const loader = await import(\"$tmp1/node_modules/lingotweaker-data/index.mjs\");"
  assert_body
} >"$tmp1/test.mjs"
if (cd "$tmp1" && PACK_LANGS="da,sv" EXPECT_LANGS="da,sv" node test.mjs); then
  check 0 "flat layout: ESM entry resolves sibling packages"
else
  check 1 "flat layout: ESM entry resolves sibling packages"
fi

# 2. pnpm-style layout: the loader lives in a store that does NOT contain the
#    per-language packages; the project symlinks the loader and installs
#    lingotweaker-data-da directly. packPath("da") only resolves through the
#    process.cwd() fallback, and packPath("sv") fails with the install hint.
tmp2="$(mktemp -d)"
stage_tree "$tmp2/store/node_modules" "$tmp2/project/node_modules" da
ln -s "$tmp2/store/node_modules/lingotweaker-data" \
  "$tmp2/project/node_modules/lingotweaker-data"
{
  printf '%s\n' 'const loader = require("lingotweaker-data");'
  assert_body
} >"$tmp2/project/test.cjs"
if (cd "$tmp2/project" && PACK_LANGS="da" EXPECT_LANGS="da,sv" EXPECT_MISSING="sv" node test.cjs); then
  check 0 "non-sibling layout: cwd fallback resolves, missing package errors"
else
  check 1 "non-sibling layout: cwd fallback resolves, missing package errors"
fi

rm -rf "$tmp1" "$tmp2"

if [ "$fails" -ne 0 ]; then
  printf '%d npm data loader test(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'all npm data loader tests passed\n'