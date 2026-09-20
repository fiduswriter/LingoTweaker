#!/usr/bin/env bash
# Java probe: reproduce the `SUJET_AUXILIAIRE[1]` cross-rule mutation
# (docs/differences.md #4). Prints the rule's check matches, each rule-loop
# mutation of the shared sentence, and the in-order match length.
#
# Usage: scripts/oracle/fr/probe-mutation.sh "<text>" <ruleId> [subId]
set -euo pipefail
RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -w /lt "$IMAGE" sh -c '
set -e
cd /lt/languagetool-language-modules/fr
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt >/dev/null 2>&1
CP="$(cat /tmp/cp.txt):/root/.m2/repository/org/languagetool/language-fr/6.9-SNAPSHOT/language-fr-6.9-SNAPSHOT.jar"
mkdir -p /tmp/classes
javac -cp "$CP" -d /tmp/classes /oracle-src/fr/AntiProbe.java
java -cp "/tmp/classes:$CP" AntiProbe "$@"
' sh "$@"
