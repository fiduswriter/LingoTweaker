#!/usr/bin/env bash
# Java probe for MorfologikDutchSpellerRule.getSpellingSuggestions and
# CompoundAcceptor.acceptCompound/getParts.
#
# Usage: scripts/oracle/nl/probe-speller.sh <nl-NL|nl-BE> word1 word2 ...
#        scripts/oracle/nl/probe-speller.sh compounds word1 word2 ...
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
MODE="${1:-nl-NL}"
shift

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -w /lt "$IMAGE" sh -c '
set -e
cd /lt/languagetool-language-modules/nl
mvn -B -q -DskipTests compile >/dev/null 2>&1
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt >/dev/null 2>&1
CP="$(cat /tmp/cp.txt):target/classes"
mkdir -p /tmp/classes
javac -cp "$CP" -d /tmp/classes /oracle-src/nl/SpellerProbe.java /oracle-src/nl/CompoundProbe.java
if [ "$1" = "compounds" ]; then
  shift
  java -cp "/tmp/classes:$CP" org.languagetool.rules.nl.CompoundProbe "$@" 2>/dev/null
else
  java -cp "/tmp/classes:$CP" SpellerProbe "$@" 2>/dev/null
fi
' sh "$MODE" "$@"
