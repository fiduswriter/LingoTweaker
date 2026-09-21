#!/usr/bin/env bash
# Build the demo data: gzipped language packs plus the rule inventory JSON
# the settings panel uses.
#
#   demo/scripts/build-packs.sh
#   LT_DEMO_LANGS="en gn" demo/scripts/build-packs.sh
#
# Output:
#   demo/public/packs/<lang>.pack.gz
#   demo/public/packs/manifest.json
#   demo/public/rules/<lang>.json
#
# The pack build itself is shared with the release artifacts
# (scripts/data/build-packs.sh).
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$script_dir/../.." && pwd)"
cd "$root"

langs="${LT_DEMO_LANGS:-en de es fr it pt nl ca no nrd gn}"
out="demo/public/packs"
rules_out="demo/public/rules"

cargo build --release -p lt-cli
scripts/data/build-packs.sh data "$out" $langs

rm -rf "$rules_out"
mkdir -p "$rules_out"
for lang in $langs; do
  target/release/lt-cli inventory --lang "$lang" --json >"$rules_out/$lang.json"
done

echo
ls -lh "$out"
ls -lh "$rules_out"