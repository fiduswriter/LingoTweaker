#!/usr/bin/env bash
# Debug probe: prints `word<TAB>hunspell.spell()<TAB>rule.isMisspelled()`.
# Usage: scripts/oracle/de/probe-spell-debug.sh word...
set -euo pipefail
RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
WORDS="$*"
docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -e WORDS="$WORDS" \
  -w /lt "$IMAGE" sh -c '
set -e
cd /lt/languagetool-language-modules/de
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt >/dev/null 2>&1
CP="$(cat /tmp/cp.txt):/root/.m2/repository/org/languagetool/language-de/6.9-SNAPSHOT/language-de-6.9-SNAPSHOT.jar"
mkdir -p /tmp/classes
javac -cp "$CP" -d /tmp/classes /oracle-src/de/ProbeSpellDebug.java
java -cp "/tmp/classes:$CP" ProbeSpellDebug $WORDS
' 
