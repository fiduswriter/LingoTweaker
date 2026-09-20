#!/usr/bin/env bash
# Dump Java LT per-token readings for a sentence list and diff them against
# `lt-cli analyze`. Requires Docker.
#
# Usage: scripts/oracle/dump-readings.sh <sentences.txt> [out-prefix]
# Writes <out-prefix>.java.tsv and <out-prefix>.rust.tsv.
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
SENTENCES="$(readlink -f "$1")"
PREFIX="${2:-/tmp/lt-oracle-readings}"

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
javac -cp "$CP" -d /tmp/oracle-classes /oracle-src/Dump.java
java -cp "/tmp/oracle-classes:$CP" Dump /in/sentences.txt > /out/readings.java.tsv
EOS
chmod +x "$WORK/inner.sh"

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -v "$WORK":/in:ro \
  -v "$WORK/out":/out \
  -w /lt "$IMAGE" bash /in/inner.sh

"$RS_ROOT/target/release/lt-cli" analyze --lang en --file "$SENTENCES" \
  > "$WORK/out/readings.rust.tsv"
cp "$WORK/out/readings.java.tsv" "${PREFIX}.java.tsv"
cp "$WORK/out/readings.rust.tsv" "${PREFIX}.rust.tsv"
echo "wrote ${PREFIX}.java.tsv and ${PREFIX}.rust.tsv"
