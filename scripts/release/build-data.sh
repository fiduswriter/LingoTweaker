#!/usr/bin/env bash
# Build the per-release data artifacts:
#
#   target/data-dist/packs/<lang>.pack.gz   gzipped in-memory data packs (wasm,
#                                           Node/browser); see crates/lt-data
#   target/data-dist/data/<lang>.tar.gz     extractable native data tree for
#                                           LT_DATA_DIR (same file set as a pack)
#   target/data-dist/manifest.json          version, upstream pin and per-file
#                                           bytes/sha256 for both forms
#
# The pack + manifest formatting is shared with the Pages demo
# (scripts/data/build-packs.sh); the native archives contain exactly the pack
# contents (`manifest.json`, `core/**`, `messages/**`, `<lang>/**`), so wasm and
# native consumers see identical data.
#
# Usage: scripts/release/build-data.sh [--packs-only | --native-only]
# Env:   LT_DATA_LANGS="en de"   restrict the language set
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIST="${DIST_DIR:-$ROOT/target/data-dist}"

PACKS_ONLY=0
NATIVE_ONLY=0
case "${1:-}" in
  --packs-only) PACKS_ONLY=1 ;;
  --native-only) NATIVE_ONLY=1 ;;
  "") ;;
  *) echo "usage: build-data.sh [--packs-only | --native-only]" >&2; exit 2 ;;
esac

if [ -n "${LT_DATA_LANGS:-}" ]; then
  langs="$LT_DATA_LANGS"
else
  langs=""
  for d in "$ROOT/data"/*/; do
    [ -d "$d/rules" ] || continue
    langs="$langs $(basename "$d")"
  done
fi

version="$(sed -nE 's/^version = "(.*)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
upstream_commit="$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['baseline_commit'])" "$ROOT/upstream.json")"

mkdir -p "$DIST"

if [ "$NATIVE_ONLY" = 0 ]; then
  echo "== packs"
  scripts/data/build-packs.sh "$ROOT/data" "$DIST/packs" $langs
fi

if [ "$PACKS_ONLY" = 0 ]; then
  echo "== native archives"
  rm -rf "$DIST/data" "$DIST/stage"
  mkdir -p "$DIST/data" "$DIST/stage"
  for lang in $langs; do
    stage="$DIST/stage/$lang"
    mkdir -p "$stage"
    cp "$ROOT/data/manifest.json" "$stage/"
    cp -r "$ROOT/data/core" "$ROOT/data/messages" "$stage/"
    cp -r "$ROOT/data/$lang" "$stage/"
    tar --sort=name --mtime='@0' --owner=0 --group=0 --numeric-owner \
      -czf "$DIST/data/$lang.tar.gz" -C "$stage" .
  done
  rm -rf "$DIST/stage"
fi

python3 - "$DIST" "$version" "$upstream_commit" <<'PY'
import hashlib
import json
import pathlib
import sys

dist = pathlib.Path(sys.argv[1])
version, upstream_commit = sys.argv[2], sys.argv[3]

def entry(path: pathlib.Path) -> dict:
    return {
        "file": path.relative_to(dist).as_posix(),
        "bytes": path.stat().st_size,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }

languages = {}
for pack in sorted(dist.glob("packs/*.pack.gz")):
    languages.setdefault(pack.name[: -len(".pack.gz")], {})["pack"] = entry(pack)
for native in sorted(dist.glob("data/*.tar.gz")):
    languages.setdefault(native.name[: -len(".tar.gz")], {})["data"] = entry(native)

manifest = {
    "version": version,
    "upstream_commit": upstream_commit,
    "languages": languages,
}
(dist / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"build-data: {len(languages)} languages, version {version} -> {dist}")
PY

echo
ls -lh "$DIST"