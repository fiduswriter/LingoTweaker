#!/usr/bin/env bash
# Java probe for individual Serbian rules / sentences against the pinned LT
# build in Docker. Prints one TSV line per match (UTF-16 offsets, Java
# message, suggested replacements).
#
# The pinned `sr` module is excluded from the LT reactor and stale; the
# `sr-module-6.9.patch` forward-port (D-287) is applied to an isolated `/tmp/lt`
# copy inside the container, so the checkout and the other oracles are
# untouched (see `check-diff-sr.sh`).
#
# Usage: scripts/oracle/sr/probe-rule.sh "<text>" [RULE_ID ...]
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
TEXT="$1"
shift
RULES=("$@")

docker run --rm \
  -v "$LT_CHECKOUT":/lt:ro \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -w /lt "$IMAGE" sh -c '
set -e
rm -rf /tmp/lt && mkdir -p /tmp/lt/languagetool-language-modules
cp /lt/pom.xml /tmp/lt/pom.xml
cp -r /lt/languagetool-language-modules/sr /tmp/lt/languagetool-language-modules/sr
cd /tmp/lt && git apply /oracle-src/sr/sr-module-6.9.patch
cd /tmp/lt/languagetool-language-modules/sr
mvn -B -q -DskipTests compile >/dev/null 2>&1
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt >/dev/null 2>&1
CP="$(cat /tmp/cp.txt):target/classes"
mkdir -p /tmp/classes
javac -cp "$CP" -d /tmp/classes /oracle-src/sr/ProbeRule.java
java -cp "/tmp/classes:$CP" ProbeRule "$@" 2>/dev/null
' sh sr "$TEXT" "${RULES[@]}"
