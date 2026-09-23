#!/bin/sh
# Container-side Java bench runner: expand the libs wildcard with the
# container shell, compile TimingDump, then time each language.
set -e
DIST=../languagetool/languagetool-standalone/target/LanguageTool-6.9-SNAPSHOT/LanguageTool-6.9-SNAPSHOT
CP="scripts/bench/results/classes:$DIST"
for jar in "$DIST"/libs/*.jar; do
  CP="$CP:$jar"
done
# shellcheck disable=SC2086
javac -cp "$CP" -d scripts/bench/results/classes scripts/bench/TimingDump.java 2>&1 | grep -v '^Note:' || true
for pair in "en en-US" "de de-DE" "es es" "pt pt-PT" "fr fr"; do
  set -- $pair
  LT_LANG=$2 java -Xmx3g -Dfile.encoding=UTF-8 -cp "$CP" \
    TimingDump "scripts/bench/results/$1.txt" 2>&1 |
    grep -E 'build|init_first_check|warmup|steady' > "scripts/bench/results/java-$1.log" ||
    cat "scripts/bench/results/java-$1.log"
  echo "=== Java $1 ($2) ==="
  cat "scripts/bench/results/java-$1.log"
done
