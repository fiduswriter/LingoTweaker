#!/usr/bin/env bash
# Granian integration smoke test for lt-py: starts an ASGI app with Granian
# (a Rust-based server, i.e. the Django/Granian deployment shape from the
# plan) and checks a request end to end.
#
# Usage: LT_BINDINGS_TMP=/tmp/lt-bindings scripts/bindings/granian-smoke.sh
# Requires `granian` on PATH (or in the active environment).
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="${LT_BINDINGS_TMP:-/tmp/lt-bindings}"
PORT="${LT_GRANIAN_PORT:-18099}"

GRANIAN="$(command -v granian || true)"
if [ -z "$GRANIAN" ]; then
  echo "granian not found on PATH" >&2
  exit 1
fi

cd "$RS_ROOT"
PYTHONPATH="$WORK/lt-py${PYTHONPATH:+:$PYTHONPATH}" \
  "$GRANIAN" --interface asgi \
  --host 127.0.0.1 --port "$PORT" \
  crates/lt-py/tests/python/granian_app.py:app &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT

for _ in $(seq 1 50); do
  if curl -sf "http://127.0.0.1:$PORT/health" > /dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

HEALTH="$(curl -sf "http://127.0.0.1:$PORT/health")"
echo "health: $HEALTH"
CHECK="$(curl -sf "http://127.0.0.1:$PORT/check?text=I%20can%20heard%20you.")"
echo "check: $CHECK"

python3 - "$CHECK" <<'PY'
import json, sys
payload = json.loads(sys.argv[1])
assert payload["language"] == "en-US", payload
rules = [m["rule_id"] for m in payload["matches"]]
assert "MD_BASEFORM" in rules, rules
print("granian smoke ok")
PY
