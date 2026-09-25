#!/usr/bin/env bash
# Publish the crates.io-visible crates in dependency order, then the
# `lingotweaker` facade (which lives in its own workspace under
# crates/lingotweaker and shares the engine source with crates/lt).
#
# `lt` itself cannot be published: the crates.io name `lt` is owned by a third
# party. `lt-cli`, `lt-http`, `lt-py`, `lt-node` and `lt-wasm` set
# `publish = false` (they depend on `lt` and/or ship through other channels).
#
# crates.io rate-limits new crates ("published too many new crates"); the
# server response names a retry time. Re-run this script to continue where it
# stopped: already-published versions upload as "already exists" and the loop
# keeps going.
#
# Usage: scripts/release/publish-crates.sh [cargo publish args...]
#   scripts/release/publish-crates.sh --dry-run
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
if [ -n "${CARGO:-}" ]; then
  CARGO_BIN=("$CARGO")
else
  CARGO_BIN=(cargo +1.98.1)
fi

ORDER=(lt-core lt-data lt-lindera lt-tokenize lt-tagger lt-pattern lt-disambig lt-spell lt-chunk)

for c in "${ORDER[@]}"; do
  echo "== publish $c"
  if ! out="$("${CARGO_BIN[@]}" publish -p "$c" "$@" 2>&1)"; then
    # A re-run hits "already exists" for every crate published before; that is
    # not a failure -- keep going (this is what makes the script resumable).
    if grep -q "already exists" <<<"$out"; then
      echo "== $c already published; skipping"
    else
      printf '%s\n' "$out" >&2
      exit 1
    fi
  fi
done

echo "== publish lingotweaker (facade)"
if ! out="$(cd "$ROOT/crates/lingotweaker" && "${CARGO_BIN[@]}" publish "$@" 2>&1)"; then
  if grep -q "already exists" <<<"$out"; then
    echo "== lingotweaker already published; skipping"
  else
    printf '%s\n' "$out" >&2
    exit 1
  fi
fi

echo "== crates.io publish complete"