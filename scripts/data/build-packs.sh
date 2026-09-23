#!/usr/bin/env bash
# Build gzipped per-language data packs plus a sha256 manifest.
#
#   scripts/data/build-packs.sh <data-dir> <out-dir> [lang...]
#
# When no languages are given, every directory under <data-dir> that contains
# `rules/` is packed. Output:
#
#   <out-dir>/<lang>.pack.gz
#   <out-dir>/<lang>.pack.zst    (sidecar; browsers with
#                                DecompressionStream("zstd") fetch it instead
#                                and save ~16-19% over gzip)
#   <out-dir>/manifest.json      { "<lang>": { file, bytes, sha256 } }
#
# The pack format is `lt-data`'s (`crates/lt-data/src/pack.rs`): one file per
# language holding `core/**` and `<lang>/**` (the engine never reads the data
# directory's `manifest.json` or `messages/**` at runtime, so they stay out).
#
# With SPLIT_PACKS=1, the demo additionally gets split packs (see
# attic/docs/wasm-demo-future-optimizations.md, item 9): for `en` a base pack
# without the optional OpenNLP chunker models and without the four
# non-default variant dictionaries, plus sidecar packs for each. The full
# `<lang>.pack.gz` is always built unchanged (release artifacts depend on
# it); the manifest lists the split parts under `extra`.
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
  pack_data="$root/${CARGO_TARGET_DIR:-target}/release/pack_data"
fi

rm -rf "$out_dir"
mkdir -p "$out_dir"
# zstd sidecars are optional: without the CLI, only gzip is produced and the
# demo falls back to `.pack.gz` (DecompressionStream("zstd") is not
# universally available anyway).
if command -v zstd >/dev/null 2>&1; then
  HAVE_ZSTD=1
else
  HAVE_ZSTD=""
  echo "build-packs: zstd not found, skipping .pack.zst sidecars" >&2
fi
for lang in $langs; do
  # Build the raw pack once; both codecs compress those exact bytes.
  "$pack_data" "$data_dir" "$lang" "$out_dir/$lang.pack"
  # -n keeps the output deterministic (no timestamp in the gzip header).
  gzip -9 -n -f -k "$out_dir/$lang.pack"
  if [ -n "$HAVE_ZSTD" ]; then
    # -19: the pack is a build artifact downloaded once, ratio is everything.
    # zstd -f refuses to overwrite via a pipe mismatch; plain file input.
    zstd -q -19 -f --no-progress "$out_dir/$lang.pack"
  fi
  rm -f "$out_dir/$lang.pack"
done

if [ -n "${SPLIT_PACKS:-}" ]; then
  # en split: base pack without the optional OpenNLP chunker models
  # (en-lite, the demo default) and without the four non-default variant
  # dictionaries (fetched on demand, ~1.9 MB gz total); the default
  # variant (en_US) stays in the base pack.
  for side in models en-GB en-AU en-CA en-NZ; do
    case "$side" in
      models) "$pack_data" "$data_dir" en "$out_dir/en.$side.pack" --only en/models ;;
      # en-GB -> en/hunspell/en_GB (the dictionary file stem)
      *) "$pack_data" "$data_dir" en "$out_dir/en.$side.pack" --only "en/hunspell/en_${side#en-}" ;;
    esac
    gzip -9 -n -f -k "$out_dir/en.$side.pack"
    if [ -n "$HAVE_ZSTD" ]; then
      zstd -q -19 -f --no-progress "$out_dir/en.$side.pack"
    fi
    rm -f "$out_dir/en.$side.pack"
  done
  "$pack_data" "$data_dir" en "$out_dir/en.base.pack" \
    --exclude en/models en/hunspell/en_GB en/hunspell/en_AU \
    en/hunspell/en_CA en/hunspell/en_NZ
  gzip -9 -n -f -k "$out_dir/en.base.pack"
  if [ -n "$HAVE_ZSTD" ]; then
    zstd -q -19 -f --no-progress "$out_dir/en.base.pack"
  fi
  rm -f "$out_dir/en.base.pack"
fi

python3 - "$out_dir" <<'PY'
import hashlib
import json
import pathlib
import sys

out = pathlib.Path(sys.argv[1])


def describe(pack_gz, suffix):
    entry = {
        "file": pack_gz.name,
        "bytes": pack_gz.stat().st_size,
        "sha256": hashlib.sha256(pack_gz.read_bytes()).hexdigest(),
    }
    zst = pathlib.Path(str(pack_gz)[: -len(suffix)] + ".pack.zst")
    if zst.exists():
        entry["zst"] = {
            "file": zst.name,
            "bytes": zst.stat().st_size,
            "sha256": hashlib.sha256(zst.read_bytes()).hexdigest(),
        }
    return entry


out = pathlib.Path(sys.argv[1])
split_en = (out / "en.base.pack.gz").exists() and (out / "en.models.pack.gz").exists()
manifest = {}
for path in sorted(out.glob("*.pack.gz")):
    lang = path.name[: -len(".pack.gz")]
    entry = describe(path, ".pack.gz")
    if lang == "en" and split_en:
        # en is additionally shipped split (see SPLIT_PACKS above): the demo
        # fetches the base pack plus the sidecars it needs; the full pack
        # stays under `file` for release consumers
        entry["split"] = {
            "base": describe(out / "en.base.pack.gz", ".pack.gz"),
            "extra": {},
        }
        for name in ("models", "en-GB", "en-AU", "en-CA", "en-NZ"):
            gz = out / f"en.{name}.pack.gz"
            if gz.exists():
                entry["split"]["extra"][name] = describe(gz, ".pack.gz")
    manifest[lang] = entry
(out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"build-packs: {len(manifest)} languages -> {out}")
PY