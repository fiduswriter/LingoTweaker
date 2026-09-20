#!/usr/bin/env bash
# Build the demo data: gzipped language packs plus the rule inventory JSON
# the settings panel uses.
#
#   demo/scripts/build-packs.sh
#   LT_DEMO_LANGS="en gn" demo/scripts/build-packs.sh
#
# Output:
#   demo/public/packs/<lang>.pack.gz
#   demo/public/rules/<lang>.json
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$script_dir/../.." && pwd)"
cd "$root"

langs="${LT_DEMO_LANGS:-en de es fr it pt nl ca no nrd gn}"
out="demo/public/packs"
rules_out="demo/public/rules"

cargo build --release -p lt-data --bin pack_data
cargo build --release -p lt-cli

rm -rf "$out" "$rules_out"
mkdir -p "$out" "$rules_out"
for lang in $langs; do
  target/release/pack_data data "$lang" "$out/$lang.pack"
  gzip -9 -f "$out/$lang.pack"
  target/release/lt-cli inventory --lang "$lang" --json >"$rules_out/$lang.json"
done

# Content hash per pack so the worker can cache-bust its fetch (`?v=…`):
# data packs change with rule/data edits while their URL stays the same.
python3 - "$out" <<'PY'
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
PY

echo
ls -lh "$out"
ls -lh "$rules_out"
