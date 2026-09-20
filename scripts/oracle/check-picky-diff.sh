#!/usr/bin/env bash
# Picky/per-rule match-set diff: Java LT `check(line, level)` via
# CheckDumpPicky vs `lt-cli check --lines`. One input line = one check.
# Requires Docker for the pinned Java build (see check-diff.sh).
#
# Usage: scripts/oracle/check-picky-diff.sh <sentences.txt> [picky] [enableRule ...]
# Writes <prefix>.java.tsv / <prefix>.rust.tsv ($CHECK_PICKY_OUT prefix).
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
SENTENCES="$(readlink -f "$1")"
shift
PREFIX="${CHECK_PICKY_OUT:-/tmp/lt-oracle-picky}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cp "$SENTENCES" "$WORK/sentences.txt"
mkdir -p "$WORK/out"

cat > "$WORK/inner.sh" <<'EOS'
set -e
cd /lt/languagetool-language-modules/en
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt
CP="$(cat /tmp/cp.txt):/root/.m2/repository/org/languagetool/language-en/6.9-SNAPSHOT/language-en-6.9-SNAPSHOT.jar"
mkdir -p /tmp/oracle-classes
javac -cp "$CP" -d /tmp/oracle-classes /oracle-src/CheckDumpPicky.java
java -cp "/tmp/oracle-classes:$CP" CheckDumpPicky /in/sentences.txt "$@" > /out/checks.java.tsv
EOS
chmod +x "$WORK/inner.sh"

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -v "$WORK":/in:ro \
  -v "$WORK/out":/out \
  -w /lt "$IMAGE" bash /in/inner.sh "$@"

RUST_ARGS=("--lines")
for arg in "$@"; do
  if [ "$arg" = "picky" ]; then
    RUST_ARGS+=("--picky")
  else
    RUST_ARGS+=("--enable-rule" "$arg")
  fi
done
"$RS_ROOT/target/release/lt-cli" check -l en-US "${RUST_ARGS[@]}" \
  --file "$SENTENCES" > "$WORK/out/checks.rust.tsv"
cp "$WORK/out/checks.java.tsv" "${PREFIX}.java.tsv"
cp "$WORK/out/checks.rust.tsv" "${PREFIX}.rust.tsv"

python3 "$RS_ROOT/scripts/oracle/compare-checks.py" \
  "${PREFIX}.java.tsv" "${PREFIX}.rust.tsv"
