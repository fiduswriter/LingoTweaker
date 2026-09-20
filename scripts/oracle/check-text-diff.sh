#!/usr/bin/env bash
# Whole-text match-set diff for text-level/paragraph rules: Java LT
# `check(text)` vs `lt-cli check --lines --whole`.
# Requires Docker for the pinned Java build (see check-diff.sh).
#
# Usage: scripts/oracle/check-text-diff.sh <text-file> [picky] [enableRule ...]
# Writes <prefix>.java.tsv / <prefix>.rust.tsv ($CHECK_TEXT_OUT prefix) and
# prints the difference summary.
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
TEXT_FILE="$(readlink -f "$1")"
shift
PREFIX="${CHECK_TEXT_OUT:-/tmp/lt-oracle-text}"

ARGS=()
for arg in "$@"; do
  ARGS+=("$arg")
done

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cp "$TEXT_FILE" "$WORK/input.txt"
mkdir -p "$WORK/out"

cat > "$WORK/inner.sh" <<'EOS'
set -e
cd /lt/languagetool-language-modules/en
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt
CP="$(cat /tmp/cp.txt):/root/.m2/repository/org/languagetool/language-en/6.9-SNAPSHOT/language-en-6.9-SNAPSHOT.jar"
mkdir -p /tmp/oracle-classes
javac -cp "$CP" -d /tmp/oracle-classes /oracle-src/CheckDumpText.java
java -cp "/tmp/oracle-classes:$CP" CheckDumpText /in/input.txt "$@" > /out/checks.java.tsv
EOS
chmod +x "$WORK/inner.sh"

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -v "$WORK":/in:ro \
  -v "$WORK/out":/out \
  -w /lt "$IMAGE" bash /in/inner.sh "${ARGS[@]}"

RUST_ARGS=("--lines" "--whole")
for arg in "$@"; do
  if [ "$arg" = "picky" ]; then
    RUST_ARGS+=("--picky")
  else
    RUST_ARGS+=("--enable-rule" "$arg")
  fi
done
"$RS_ROOT/target/release/lt-cli" check -l en-US "${RUST_ARGS[@]}" \
  --file "$TEXT_FILE" > "$WORK/out/checks.rust.tsv"
cp "$WORK/out/checks.java.tsv" "${PREFIX}.java.tsv"
cp "$WORK/out/checks.rust.tsv" "${PREFIX}.rust.tsv"

python3 "$RS_ROOT/scripts/oracle/compare-checks.py" \
  "${PREFIX}.java.tsv" "${PREFIX}.rust.tsv"
