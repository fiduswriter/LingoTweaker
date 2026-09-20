#!/usr/bin/env bash
# Java probe for `MorfologikPolishSpellerRule.match` (the tokenizing pattern,
# `isNotCompound` and `pruneSuggestions` overrides). Prints
# `line\tfrom\tto\tsuggestions` with UTF-16 offsets.
#
# Usage: scripts/oracle/pl/probe-speller.sh <sentences.txt>
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
SENTENCES="$(readlink -f "$1")"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cp "$SENTENCES" "$WORK/sentences.txt"

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -v "$WORK":/in:ro \
  -w /lt "$IMAGE" sh -c '
set -e
cd /lt/languagetool-language-modules/pl
mvn -B -q -DskipTests compile >/dev/null 2>&1
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt >/dev/null 2>&1
CP="$(cat /tmp/cp.txt):target/classes"
mkdir -p /tmp/classes
javac -cp "$CP" -d /tmp/classes /oracle-src/pl/PlSpellerProbe.java
java -cp "/tmp/classes:$CP" PlSpellerProbe /in/sentences.txt 2>/dev/null
'
