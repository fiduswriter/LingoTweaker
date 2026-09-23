#!/usr/bin/env bash
# Build and upload one PyPI data package per language:
#
#   lingotweaker-data-<lang>   (pure Python, py3-none-any)
#     lingotweaker_data_<lang>/data/   manifest.json, core/**, messages/**,
#                                      <lang>/**   (the native data tree)
#     lingotweaker_data_<lang>.data_dir() -> absolute path
#
# The per-file PyPI limit is 100 MB; every language tree is well below that
# (largest wheel: nl). `lt_py` auto-discovers an installed data package for the
# requested language (see crates/lt-py), so `pip install lingotweaker-data-en`
# makes `lt_py.Engine("en-US")` work with no configuration.
#
# Usage: scripts/release/publish-pypi-data.sh [--build-only] [--keep-sources]
#   --build-only   : build the wheels, do not upload
#   --keep-sources : keep the generated package sources under target/data-dist
#
# Upload credentials come from the usual twine env vars (TWINE_USERNAME /
# TWINE_PASSWORD) or ~/.pypirc. Without credentials the script builds only and
# warns, so the release workflow does not fail before the token is configured.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIST="${DIST_DIR:-$ROOT/target/data-dist}"
PYTHON="${PYTHON:-python3}"
VENV="${DATA_RELEASE_VENV:-/tmp/lingotweaker-data-venv}"
BUILD_ONLY=0
KEEP_SOURCES=0
for arg in "$@"; do
  case "$arg" in
    --build-only) BUILD_ONLY=1 ;;
    --keep-sources) KEEP_SOURCES=1 ;;
    *) echo "usage: publish-pypi-data.sh [--build-only] [--keep-sources]" >&2; exit 2 ;;
  esac
done

# The PyPI data distributions carry their own version, independent of the
# engine release: bump data/pypi-version only when the contents of data/
# actually change, so a new engine release does not republish identical data
# packages (and re-trigger PyPI's new-project rate limit for no reason).
version_file="$ROOT/data/pypi-version"
if [ ! -f "$version_file" ]; then
  echo "publish-pypi-data: missing $version_file (the PyPI data version)" >&2
  exit 1
fi
version="$(tr -d '[:space:]' < "$version_file")"
pyver="$(python3 - "$version" <<'PY'
import sys
v = sys.argv[1]
core, _, pre = v.partition("-")
if pre.startswith("alpha."):
    print(f"{core}a{pre.split('.')[1]}")
else:
    print(v)
PY
)"

if [ -n "${LT_DATA_LANGS:-}" ]; then
  langs="$LT_DATA_LANGS"
else
  langs=""
  for d in "$ROOT/data"/*/; do
    [ -d "$d/rules" ] || continue
    langs="$langs $(basename "$d")"
  done
fi

if [ ! -x "$VENV/bin/python" ]; then
  echo "== creating build venv at $VENV"
  "$PYTHON" -m venv "$VENV"
  "$VENV/bin/pip" install -q --upgrade pip build setuptools wheel twine
fi

out="$DIST/pypi"
rm -rf "$out"
mkdir -p "$out/dist"

for lang in $langs; do
  pkg="lingotweaker-data-$lang"
  mod="lingotweaker_data_$lang"
  src="$out/$pkg"
  echo "== source $pkg"
  mkdir -p "$src/$mod/data"
  cp "$ROOT/data/manifest.json" "$src/$mod/data/"
  cp -r "$ROOT/data/core" "$ROOT/data/messages" "$src/$mod/data/"
  cp -r "$ROOT/data/$lang" "$src/$mod/data/"
  cat >"$src/$mod/__init__.py" <<EOF
"""Runtime data for the LingoTweaker engine, language \`$lang\`.

\`data_dir()\` returns the absolute path of the data directory; pass it to the
binding (\`lt_py.Engine("...", data_dir=...)\`) or set \`LT_DATA_DIR\`. \`lt_py\`
discovers this package automatically for a matching language.
"""

from pathlib import Path

DATA_DIR = Path(__file__).resolve().parent / "data"


def data_dir() -> str:
    """Absolute path of the runtime data directory."""
    return str(DATA_DIR)


__all__ = ["DATA_DIR", "data_dir"]
EOF
  cat >"$src/MANIFEST.in" <<EOF
recursive-include $mod/data *
EOF
  cat >"$src/README.md" <<EOF
# LingoTweaker data ($lang)

Runtime data for the LingoTweaker proofreading engine, language \`$lang\`.
Installing this package is enough for \`lt_py\` to find the data:

\`\`\`python
import lt_py

engine = lt_py.Engine("$lang-US")   # or the appropriate long code
\`\`\`

\`data_dir()\` exposes the absolute data path if you prefer to pass it
explicitly (\`lt_py.Engine(..., data_dir=data_dir())\`) or to set \`LT_DATA_DIR\`.

This is the same per-language data attached to each LingoTweaker GitHub
Release. Vendored rule data, dictionaries and models keep their upstream
licenses; see \`THIRD_PARTY_NOTICES.md\` in the repository.

License (packaging code): LGPL-2.1-or-later.
EOF
  cat >"$src/pyproject.toml" <<EOF
[build-system]
requires = ["setuptools>=68"]
build-backend = "setuptools.build_meta"

[project]
name = "$pkg"
version = "$pyver"
description = "LingoTweaker runtime data for language '$lang'"
readme = "README.md"
requires-python = ">=3.9"
license = { text = "LGPL-2.1-or-later" }
keywords = ["proofreading", "grammar", "spellcheck", "nlp", "language", "data"]
classifiers = [
  "Programming Language :: Python :: 3",
  "Topic :: Text Processing :: Linguistic",
]

[project.urls]
Homepage = "https://fiduswriter.github.io/LingoTweaker/"
Repository = "https://github.com/fiduswriter/LingoTweaker"

[tool.setuptools]
include-package-data = true

[tool.setuptools.packages.find]
include = ["$mod*"]
EOF

  "$VENV/bin/python" -m build --no-isolation --outdir "$out/dist" "$src" >/dev/null
  if [ "$KEEP_SOURCES" = 0 ]; then rm -rf "$src"; fi
done

echo "== built $(ls "$out/dist" | wc -l) files"
ls -lh "$out/dist" | tail -n +2

if [ "$BUILD_ONLY" = 1 ]; then
  echo "== build-only: $out/dist (not uploaded)"
  exit 0
fi

if [ -z "${TWINE_PASSWORD:-}" ] && [ ! -f "$HOME/.pypirc" ] && [ -z "${PYPI_API_TOKEN:-}" ]; then
  echo "== no PyPI credentials (TWINE_PASSWORD / PYPI_API_TOKEN / ~/.pypirc); built only" >&2
  exit 0
fi
if [ -n "${PYPI_API_TOKEN:-}" ] && [ -z "${TWINE_PASSWORD:-}" ]; then
  export TWINE_USERNAME="__token__"
  export TWINE_PASSWORD="$PYPI_API_TOKEN"
fi

echo "== twine upload (per file, --skip-existing, retry on transient errors)"
# Uploading ~35 wheels in one twine call trips PyPI's rate limit (HTTP 429) and
# aborts mid-list. Upload one at a time, skip files already on the index, and
# back off-and-retry transient failures so the step is resumable.
upload_one() {
  f="$1"
  attempt=1
  while [ "$attempt" -le 6 ]; do
    if "$VENV/bin/twine" upload --skip-existing --disable-progress-bar "$f"; then
      return 0
    fi
    delay=$((attempt * 20))
    echo "== upload failed for $(basename "$f") (attempt $attempt); retrying in ${delay}s" >&2
    sleep "$delay"
    attempt=$((attempt + 1))
  done
  return 1
}
for f in "$out"/dist/*.whl "$out"/dist/*.tar.gz; do
  [ -e "$f" ] || continue
  echo "== upload $(basename "$f")"
  upload_one "$f"
  sleep 3
done
echo "== PyPI data publish complete"