#!/usr/bin/env bash
# Java probe for the raw German `MorfologikMultiSpeller.getSuggestions`
# (word/weight pairs) and the rule's isMisspelled decision.
#
# Usage: scripts/oracle/de/probe-morfo.sh words.txt [de-DE|de-AT|de-CH]
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
FILE="$1"
VARIANT="${2:-de-DE}"

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -v "$(cd "$(dirname "$FILE")" && pwd)":/input:ro \
  -w /lt "$IMAGE" sh -c '
set -e
cd /lt/languagetool-language-modules/de
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt >/dev/null 2>&1
CP="$(cat /tmp/cp.txt):/root/.m2/repository/org/languagetool/language-de/6.9-SNAPSHOT/language-de-6.9-SNAPSHOT.jar"
mkdir -p /tmp/classes
javac -cp "$CP" -d /tmp/classes /oracle-src/de/ProbeMorfo.java
java -cp "/tmp/classes:$CP" ProbeMorfo "/input/'"$(basename "$FILE")"'" "'"$VARIANT"'"
' 2>/dev/null
