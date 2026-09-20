#!/usr/bin/env bash
# Match-level parity gate: the 2,000-example sample must produce exactly the
# same check() match sets as Java (0 only-Java, 0 only-Rust, 0 field diffs).
#
# Requires Docker (see check-diff.sh) and a release lt-cli build.
#
# Usage: scripts/oracle/gate.sh [sample-size]
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LIMIT="${1:-2000}"
CORPUS="$RS_ROOT/docs/parity/corpora/en-examples.jsonl"
SAMPLE="${GATE_SAMPLE:-/tmp/lt-gate-sample.txt}"
OUT="${GATE_OUT:-/tmp/lt-gate}"

if [ ! -f "$CORPUS" ]; then
  echo "corpus not found: $CORPUS" >&2
  exit 2
fi

python3 - "$CORPUS" "$SAMPLE" "$LIMIT" <<'PY'
import json, sys

corpus, out_path, limit = sys.argv[1], sys.argv[2], int(sys.argv[3])
n = 0
with open(out_path, "w") as f:
    for line in open(corpus):
        e = json.loads(line)
        if e.get("correct") is False and not e.get("triggers_error"):
            f.write(e["text"].replace("\n", " ") + "\n")
            n += 1
            if n >= limit:
                break
print(f"gate sample: {n} examples -> {out_path}")
PY

cargo build --release -p lt-cli --manifest-path "$RS_ROOT/Cargo.toml"

SUMMARY="$("$RS_ROOT/scripts/oracle/check-diff.sh" "$SAMPLE" "$OUT" | tee /dev/stderr | grep 'only Java:' | tail -1)"
echo "$SUMMARY"

if ! echo "$SUMMARY" | grep -q "only Java: 0; only Rust: 0; field diffs: 0"; then
  echo "GATE FAILED: Java/Rust match sets differ (see above)" >&2
  exit 1
fi
echo "GATE OK"
