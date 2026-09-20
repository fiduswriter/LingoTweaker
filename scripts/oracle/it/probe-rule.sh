#!/usr/bin/env bash
# Java probe for individual Italian rules against the pinned LT build in
# Docker. Prints one TSV line per match of the requested rules (UTF-16
# offsets, Java message, suggested replacements).
#
# Usage: scripts/oracle/it/probe-rule.sh "<text>" [RULE_ID ...]
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
TEXT="$1"
shift
RULES=("$@")

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -w /lt "$IMAGE" sh -c '
set -e
cd /lt/languagetool-language-modules/it
mvn -B -q -DskipTests compile >/dev/null 2>&1
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt >/dev/null 2>&1
CP="$(cat /tmp/cp.txt):target/classes"
mkdir -p /tmp/classes
javac -cp "$CP" -d /tmp/classes /oracle-src/it/ProbeRule.java
java -cp "/tmp/classes:$CP" ProbeRule "$@" 2>/dev/null
' sh "$TEXT" "${RULES[@]}"
