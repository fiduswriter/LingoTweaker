#!/usr/bin/env bash
# French SRX sentence-split diff: Java `DumpSentences.java` vs
# `lt-cli check -l fr --json` (whole text). Requires Docker for the pinned
# Java build.
#
# Usage: scripts/oracle/fr/dump-sentences.sh <text.txt> [java.tsv]
# With FR_REUSE_JAVA=1 and an existing <java.tsv>, the Docker run is skipped.
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
TEXT="$(readlink -f "$1")"
JAVA_OUT="${2:-/tmp/lt-fr-sentences.java.tsv}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cp "$TEXT" "$WORK/text.txt"
mkdir -p "$WORK/out"

if [ -f "$JAVA_OUT" ] && [ "${FR_REUSE_JAVA:-0}" = "1" ]; then
  cp "$JAVA_OUT" "$WORK/out/sentences.java.tsv"
else
cat > "$WORK/inner.sh" <<'EOS'
set -e
cd /lt/languagetool-language-modules/fr
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt
CP="$(cat /tmp/cp.txt):/root/.m2/repository/org/languagetool/language-fr/6.9-SNAPSHOT/language-fr-6.9-SNAPSHOT.jar"
mkdir -p /tmp/oracle-classes
javac -cp "$CP" -d /tmp/oracle-classes /oracle-src/fr/DumpSentences.java
java -cp "/tmp/oracle-classes:$CP" DumpSentences /in/text.txt > /out/sentences.java.tsv
EOS
chmod +x "$WORK/inner.sh"
docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -v "$WORK":/in:ro \
  -v "$WORK/out":/out \
  -w /lt "$IMAGE" bash /in/inner.sh
fi

"$RS_ROOT/target/release/lt-cli" check -l fr --json --file "$TEXT" |
  python3 -c "
import json, sys
for s in json.load(sys.stdin)['sentences']:
    print('S\t' + s['text'].replace(chr(10), '\\\\n'))
" > "$WORK/out/sentences.rust.tsv"

cp "$WORK/out/sentences.java.tsv" "$JAVA_OUT"
diff -u "$JAVA_OUT" "$WORK/out/sentences.rust.tsv" && echo "SENTENCES MATCH"
