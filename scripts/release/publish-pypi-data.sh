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

# Generated CJK dictionaries are gitignored; generate any that are missing so
# the data artifacts include their tokenizer data. Set LT_SKIP_DICT_BUILD=1 to
# skip (offline / already generated).
if [ -z "${LT_SKIP_DICT_BUILD:-}" ]; then
  for _l in ja zh; do
    [ -d "$ROOT/data/$_l/rules" ] && python3 "$ROOT/tools/lindera/build-dict.py" "$_l"
  done
fi
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

# Each `lingotweaker-data-<lang>` distribution is versioned independently of
# the engine release. `data/pypi-versions.json` records every language's
# version and the content hash it was bumped for; refresh it with
# scripts/release/update-pypi-data-versions.py after changing data/ (that bumps
# only the languages whose contents changed). Because unchanged languages keep
# their version, the upload below skips files already on PyPI and therefore
# publishes only what actually changed.
registry="$ROOT/data/pypi-versions.json"
if [ ! -f "$registry" ]; then
  echo "publish-pypi-data: missing $registry; run scripts/release/update-pypi-data-versions.py first" >&2
  exit 1
fi

# PEP 440 version for one language, straight from the registry.
pep440_version() {
  python3 - "$registry" "$1" <<'PY'
import json, sys
registry, lang = sys.argv[1], sys.argv[2]
languages = json.load(open(registry)).get("languages", {})
try:
    version = languages[lang]["version"]
except KeyError:
    raise SystemExit(
        f"publish-pypi-data: {lang} is not in {registry}; "
        "run scripts/release/update-pypi-data-versions.py first"
    )
core, _, pre = version.partition("-")
print(f"{core}a{pre.split('.')[1]}" if pre.startswith("alpha.") else version)
PY
}

# Is <filename> already published under <pkg> on PyPI?
on_pypi() {
  python3 - "$1" "$2" <<'PY'
import json, sys, urllib.request
pkg, filename = sys.argv[1], sys.argv[2]
try:
    with urllib.request.urlopen(f"https://pypi.org/pypi/{pkg}/json", timeout=20) as r:
        data = json.load(r)
except Exception:
    sys.exit(1)  # unknown -> attempt the upload
for files in data.get("releases", {}).values():
    for f in files:
        if f.get("filename") == filename:
            sys.exit(0)
sys.exit(1)
PY
}

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
  lang_pyver="$(pep440_version "$lang")"
  echo "== source $pkg ($lang_pyver)"
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
version = "$lang_pyver"
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

echo "== twine upload (only files not already on PyPI)"
# Upload one file at a time, skip anything already on the index (so unchanged
# languages are never re-uploaded), and back off-and-retry transient failures
# (PyPI's new-project limit and burst uploads return HTTP 429) so the step is
# resumable. A package that exhausts its retries does NOT abort the run: the
# loop continues with the next package and failed ones are retried in later
# passes, because the new-project limit is account-wide and a package that
# already got through does not consume it again.
upload_one() {
  f="$1"
  attempt=1
  while [ "$attempt" -le 8 ]; do
    if "$VENV/bin/twine" upload --skip-existing --disable-progress-bar "$f"; then
      return 0
    fi
    delay=$((attempt * 60))
    [ "$delay" -gt 300 ] && delay=300
    echo "== upload failed for $(basename "$f") (attempt $attempt); retrying in ${delay}s" >&2
    sleep "$delay"
    attempt=$((attempt + 1))
  done
  return 1
}

# distribution name / version from a wheel or sdist filename
file_pkg() {
  base="$(basename "$1")"
  base="${base%.whl}"
  base="${base%.tar.gz}"
  printf '%s' "${base%%-*}" | tr '_' '-'
}
file_ver() {
  base="$(basename "$1")"
  base="${base%.whl}"
  base="${base%.tar.gz}"
  rest="${base#*-}"
  printf '%s' "${rest%%-*}"
}

uploaded=0
skipped=0
failed_files=""
for f in "$out"/dist/*.whl "$out"/dist/*.tar.gz; do
  [ -e "$f" ] || continue
  name="$(basename "$f")"
  pkg="$(file_pkg "$name")"
  ver="$(file_ver "$name")"
  if on_pypi "$pkg" "$name"; then
    skipped=$((skipped + 1))
    echo "== up to date: $name"
    continue
  fi
  echo "== upload $pkg $ver ($name)"
  if upload_one "$f"; then
    uploaded=$((uploaded + 1))
  else
    failed_files="$failed_files $f"
  fi
  sleep 3
done

# Up to 3 passes over whatever failed: each pass re-checks the PyPI index
# first (an earlier attempt may have published it after all) and waits
# between passes so the account-wide new-project rate limit can recover.
pass=1
while [ -n "$failed_files" ] && [ "$pass" -le 3 ]; do
  echo "== retry pass $pass over failed uploads (waiting 120s first)"
  sleep 120
  remaining=""
  for f in $failed_files; do
    name="$(basename "$f")"
    pkg="$(file_pkg "$name")"
    if on_pypi "$pkg" "$name"; then
      skipped=$((skipped + 1))
      echo "== up to date: $name"
      continue
    fi
    if upload_one "$f"; then
      uploaded=$((uploaded + 1))
      sleep 3
    else
      remaining="$remaining $f"
    fi
  done
  failed_files="$remaining"
  pass=$((pass + 1))
done

echo "== PyPI data publish complete ($uploaded uploaded, $skipped already present)"
if [ -n "$failed_files" ]; then
  echo "== FAILED uploads (re-run this workflow to resume):" >&2
  for f in $failed_files; do echo "  $(basename "$f")" >&2; done
  exit 1
fi