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

langs="${LT_DEMO_LANGS:-en de es fr it pt nl ca gl ro pl sk sl el da sv is eo ast br tl lt crh be ru uk sr ar fa km ml ta ja zh no nrd nn gn}"
out="demo/public/packs"
rules_out="demo/public/rules"

cargo build --release -p lt-cli
# NL_VARIANTS=0 skips the Dutch fold/nl-light variant builds: the demo serves
# the full nl pack only, and the fold's dictionary recompiles would add ~15
# min to the Pages build for no demo benefit (the variants stay available in
# the release artifacts).
NL_VARIANTS=0 scripts/data/build-packs.sh data "$out" $langs

rm -rf "$rules_out"
mkdir -p "$rules_out"
for lang in $langs; do
  target/release/lt-cli inventory --lang "$lang" --json >"$rules_out/$lang.json"
done

echo
ls -lh "$out"
ls -lh "$rules_out"