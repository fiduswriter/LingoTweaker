#!/usr/bin/env bash
# Java probe for the Catalan tokenizer + tagger readings. One input
# sentence per line; prints the same S/T lines as
# `cargo run -p lt-tagger --example dump_tags_ca`.
#
# Usage: scripts/oracle/ca/dump-tags.sh sentences.txt
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
cd /lt/languagetool-language-modules/ca
mvn -B -q -DskipTests compile >/dev/null 2>&1
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt >/dev/null 2>&1
CP="$(cat /tmp/cp.txt):target/classes"
mkdir -p /tmp/classes
javac -cp "$CP" -d /tmp/classes /oracle-src/ca/DumpTags.java
java -cp "/tmp/classes:$CP" DumpTags "/input/'"$(basename "$FILE")"'"
'
