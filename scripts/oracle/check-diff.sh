#!/usr/bin/env bash
# Exact match-set diff: Java LT `check()` vs `lt-cli check --lines`.
# Requires Docker for the pinned Java build.
#
# Usage: scripts/oracle/check-diff.sh <sentences.txt> [out-prefix]
# Writes <prefix>.java.tsv / <prefix>.rust.tsv and prints a difference summary.
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
SENTENCES="$(readlink -f "$1")"
PREFIX="${2:-/tmp/lt-oracle-checks}"

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
javac -cp "$CP" -d /tmp/oracle-classes /oracle-src/CheckDump.java
java -cp "/tmp/oracle-classes:$CP" CheckDump /in/sentences.txt > /out/checks.java.tsv
EOS
chmod +x "$WORK/inner.sh"

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -v "$WORK":/in:ro \
  -v "$WORK/out":/out \
  -w /lt "$IMAGE" bash /in/inner.sh

"$RS_ROOT/target/release/lt-cli" check -l en-US --lines --file "$SENTENCES" \
  > "$WORK/out/checks.rust.tsv"
cp "$WORK/out/checks.java.tsv" "${PREFIX}.java.tsv"
cp "$WORK/out/checks.rust.tsv" "${PREFIX}.rust.tsv"

python3 "$RS_ROOT/scripts/oracle/compare-checks.py" \
  "${PREFIX}.java.tsv" "${PREFIX}.rust.tsv"
