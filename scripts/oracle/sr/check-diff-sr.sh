#!/usr/bin/env bash
# Exact Serbian match-set diff: Java LT `check()` vs `lt-cli check -l sr
# --lines`. Requires Docker for the pinned Java build.
#
# The pinned `sr` module is excluded from the LanguageTool reactor and stale
# (parent `4.5-SNAPSHOT`, core `5.8`); `sr-module-6.9.patch` forward-ports it
# to the pinned 6.9-SNAPSHOT core (see `attic/docs/parity/sr-rule-port.md`).
# The patch is applied to an isolated copy of `/lt/pom.xml` +
# `/lt/languagetool-language-modules/sr` under `/tmp/lt` (recreated on every
# run, so it is non-destructive and idempotent: the user's checkout and every
# other language's oracle are untouched).
#
# The pinned `languagetool-core:6.9-SNAPSHOT` must be in the shared `lt-m2`
# cache (the ru/uk oracles use it). If it is missing, install it once:
#   docker run --rm -v "$LT_CHECKOUT":/lt -v lt-m2:/root/.m2 -w /lt \
#     maven:3.9-eclipse-temurin-21 \
#     mvn -B -q -DskipTests -pl languagetool-core -am install
#
# Usage: scripts/oracle/sr/check-diff-sr.sh <sentences.txt> [out-prefix]
# Writes <prefix>.java.tsv / <prefix>.rust.tsv and prints a difference summary.
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
SENTENCES="$(readlink -f "$1")"
PREFIX="${2:-/tmp/lt-sr-oracle-checks}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cp "$SENTENCES" "$WORK/sentences.txt"
mkdir -p "$WORK/out"

if [ -f "${PREFIX}.java.tsv" ] && [ "${SR_REUSE_JAVA:-0}" = "1" ]; then
  # the Java side only depends on the input file + pinned checkout + patch
  cp "${PREFIX}.java.tsv" "$WORK/out/checks.java.tsv"
else

cat > "$WORK/inner.sh" <<'EOS'
set -e
# Isolated, patched copy of the pinned sr module (never touches /lt).
rm -rf /tmp/lt && mkdir -p /tmp/lt/languagetool-language-modules
cp /lt/pom.xml /tmp/lt/pom.xml
cp -r /lt/languagetool-language-modules/sr /tmp/lt/languagetool-language-modules/sr
cd /tmp/lt && git apply /oracle-src/sr/sr-module-6.9.patch
cd /tmp/lt/languagetool-language-modules/sr
mvn -B -q -DskipTests compile
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt
CP="$(cat /tmp/cp.txt):target/classes"
mkdir -p /tmp/oracle-classes
javac -cp "$CP" -d /tmp/oracle-classes /oracle-src/CheckDump.java
LT_LANG=sr java -cp "/tmp/oracle-classes:$CP" CheckDump /in/sentences.txt 2>/dev/null > /out/checks.java.tsv
EOS
chmod +x "$WORK/inner.sh"

docker run --rm \
  -v "$LT_CHECKOUT":/lt:ro \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -v "$WORK":/in:ro \
  -v "$WORK/out":/out \
  -w /lt "$IMAGE" bash /in/inner.sh
fi

"$RS_ROOT/target/release/lt-cli" check -l sr --lines --jobs "${SR_JOBS:-8}" \
  --file "$SENTENCES" > "$WORK/out/checks.rust.tsv"
cp "$WORK/out/checks.java.tsv" "${PREFIX}.java.tsv"
cp "$WORK/out/checks.rust.tsv" "${PREFIX}.rust.tsv"

python3 "$RS_ROOT/scripts/oracle/compare-checks.py" \
  "${PREFIX}.java.tsv" "${PREFIX}.rust.tsv" "${COMPARE_SHOW:-60}"
