#!/usr/bin/env bash
# Build, smoke-test and upload the Python prerelease to PyPI.
#
#   wheel + sdist via maturin, wheel smoke test in a throwaway venv
#   (crates/lt-py/tests/python/test_smoke.py, engine data via LT_DATA_DIR),
#   then `twine upload` using ~/.pypirc (or TWINE_* / PYPI_API_TOKEN).
#
# Usage: scripts/release/publish-pypi.sh [--build-only]
#   --build-only : build + smoke-test, do not upload
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PYTHON="${PYTHON:-python3}"
DIST="${DIST_DIR:-$ROOT/target/release-dist}"
VENV="${RELEASE_VENV:-/tmp/lingotweaker-release-venv}"
SMOKE="${SMOKE_VENV:-/tmp/lingotweaker-smoke-venv}"
BUILD_ONLY=0
[ "${1:-}" = "--build-only" ] && BUILD_ONLY=1

mkdir -p "$DIST"

if [ ! -x "$VENV/bin/maturin" ]; then
  echo "== creating release venv at $VENV"
  "$PYTHON" -m venv "$VENV"
  "$VENV/bin/pip" install -q --upgrade pip build twine maturin
fi

echo "== maturin wheel + sdist"
"$VENV/bin/maturin" build --release --manifest-path "$ROOT/crates/lt-py/Cargo.toml" --out "$DIST"
"$VENV/bin/maturin" sdist --manifest-path "$ROOT/crates/lt-py/Cargo.toml" --out "$DIST"

echo "== wheel smoke test ($SMOKE)"
rm -rf "$SMOKE"
"$PYTHON" -m venv "$SMOKE"
"$SMOKE/bin/pip" install -q "$DIST"/lingotweaker-*.whl
LT_DATA_DIR="${LT_DATA_DIR:-$ROOT/data}" \
  "$SMOKE/bin/python" "$ROOT/crates/lt-py/tests/python/test_smoke.py"

if [ "$BUILD_ONLY" = 1 ]; then
  echo "== build-only: artifacts in $DIST (not uploaded)"
  exit 0
fi

echo "== twine upload"
"$VENV/bin/twine" upload "$DIST"/lingotweaker-*.whl "$DIST"/lingotweaker-*.tar.gz
echo "== PyPI publish complete"