#!/usr/bin/env bash
# Builds the Python and Node bindings and runs their smoke/thread tests.
# Usage: scripts/bindings/smoke.sh   (run from the repository root)
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="${LT_BINDINGS_TMP:-/tmp/lt-bindings}"
mkdir -p "$WORK/lt-py" "$WORK/lt-node"

echo "== building lt-py / lt-node (release)"
cargo build --release -p lt-py -p lt-node --manifest-path "$RS_ROOT/Cargo.toml"

cp "$RS_ROOT/target/release/liblt_py.so" "$WORK/lt-py/lt_py.so"
cp "$RS_ROOT/target/release/liblt_node.so" "$WORK/lt-node/lt_node.node"

echo "== lt-py"
cd "$RS_ROOT"
PYTHONPATH="$WORK/lt-py${PYTHONPATH:+:$PYTHONPATH}" \
  python3 "$RS_ROOT/crates/lt-py/tests/python/test_smoke.py"

echo "== lt-node"
LT_NODE_ADDON="$WORK/lt-node/lt_node.node" \
  node "$RS_ROOT/crates/lt-node/tests/node/test_smoke.mjs"

if python3 -c "import granian" 2>/dev/null; then
  echo "== granian smoke"
  LT_BINDINGS_TMP="$WORK" bash "$RS_ROOT/scripts/bindings/granian-smoke.sh"
else
  echo "== granian smoke skipped (pip install granian to enable)"
fi
