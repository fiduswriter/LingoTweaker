#!/usr/bin/env bash
# Java probe: pre-disambiguation vs final token readings (raw_pos reference).
# One input sentence per line.
#
# Usage: scripts/oracle/fr/probe-raw-pos.sh sentences.txt
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
FILE="$1"

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -v "$(cd "$(dirname "$FILE")" && pwd)":/input:ro \
  -w /lt "$IMAGE" sh -c '
set -e
cd /lt/languagetool-language-modules/fr
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt >/dev/null 2>&1
CP="$(cat /tmp/cp.txt):/root/.m2/repository/org/languagetool/language-fr/6.9-SNAPSHOT/language-fr-6.9-SNAPSHOT.jar"
mkdir -p /tmp/classes
javac -cp "$CP" -d /tmp/classes /oracle-src/fr/ProbeRawPos.java
java -cp "/tmp/classes:$CP" ProbeRawPos "/input/'"$(basename "$FILE")"'"
' 2>/dev/null
