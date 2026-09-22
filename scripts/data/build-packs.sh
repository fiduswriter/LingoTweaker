#!/usr/bin/env bash
# Build gzipped per-language data packs plus a sha256 manifest.
#
#   scripts/data/build-packs.sh <data-dir> <out-dir> [lang...]
#
# When no languages are given, every directory under <data-dir> that contains
# `rules/` is packed. Output:
#
#   <out-dir>/<lang>.pack.gz
#   <out-dir>/manifest.json      { "<lang>": { file, bytes, sha256 } }
#
# The pack format is `lt-data`'s (`crates/lt-data/src/pack.rs`): one file per
# language holding `core/**` and `<lang>/**` (the engine never reads the data
# directory's `manifest.json` or `messages/**` at runtime, so they stay out).
#
# Used by the GitHub Pages demo (demo/scripts/build-packs.sh) and the release
# data artifacts (scripts/release/build-data.sh), so both serve byte-identical
# packs.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ "$#" -lt 2 ]; then
  echo "usage: build-packs.sh <data-dir> <out-dir> [lang...]" >&2
  exit 2
fi
data_dir="$1"
out_dir="$2"
shift 2

if [ "$#" -gt 0 ]; then
  langs="$*"
else
  langs=""
  for d in "$data_dir"/*/; do
    [ -d "$d/rules" ] || continue
    langs="$langs $(basename "$d")"
  done
fi
if [ -z "${langs// /}" ]; then
  echo "build-packs: no language directories found under $data_dir" >&2
  exit 1
fi

if [ -n "${PACK_DATA:-}" ]; then
  pack_data="$PACK_DATA"
else
  cargo build --release -p lt-data --bin pack_data --manifest-path "$root/Cargo.toml"
  pack_data="$root/target/release/pack_data"
fi

rm -rf "$out_dir"
mkdir -p "$out_dir"
for lang in $langs; do
  # -n keeps the output deterministic (no timestamp in the gzip header).
  "$pack_data" "$data_dir" "$lang" "$out_dir/$lang.pack"
  gzip -9 -n -f "$out_dir/$lang.pack"
done

python3 - "$out_dir" <<'PY'
import hashlib
import json
import pathlib
import sys

out = pathlib.Path(sys.argv[1])
manifest = {}
for path in sorted(out.glob("*.pack.gz")):
    lang = path.name[: -len(".pack.gz")]
    manifest[lang] = {
        "file": path.name,
        "bytes": path.stat().st_size,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }
(out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"build-packs: {len(manifest)} languages -> {out}")
PY