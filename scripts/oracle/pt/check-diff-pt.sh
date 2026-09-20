#!/usr/bin/env bash
# Exact Portuguese match-set diff: Java LT `check()` vs `lt-cli check -l pt
# --lines`. Requires Docker for the pinned Java build. The Java side runs the
# default variant (pt-PT); the corpus also carries pt-BR/pt-AO/pt-MZ rule
# examples, whose rules are inactive in Java and Rust alike.
#
# Usage: scripts/oracle/pt/check-diff-pt.sh <sentences.txt> [out-prefix]
# Writes <prefix>.java.tsv / <prefix>.rust.tsv and prints a difference summary.
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
SENTENCES="$(readlink -f "$1")"
PREFIX="${2:-/tmp/lt-pt-oracle-checks}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cp "$SENTENCES" "$WORK/sentences.txt"
mkdir -p "$WORK/out"

if [ -f "${PREFIX}.java.tsv" ] && [ "${PT_REUSE_JAVA:-0}" = "1" ]; then
  # the Java side only depends on the input file + pinned checkout
  cp "${PREFIX}.java.tsv" "$WORK/out/checks.java.tsv"
else

cat > "$WORK/inner.sh" <<'EOS'
set -e
cd /lt/languagetool-language-modules/pt
mvn -B -q -DskipTests compile
mvn -B -q dependency:build-classpath -Dmdep.outputFile=/tmp/cp.txt
CP="$(cat /tmp/cp.txt):target/classes"
mkdir -p /tmp/oracle-classes
javac -cp "$CP" -d /tmp/oracle-classes /oracle-src/CheckDump.java
LT_LANG=pt-PT java -cp "/tmp/oracle-classes:$CP" CheckDump /in/sentences.txt > /out/checks.java.tsv
EOS
chmod +x "$WORK/inner.sh"

docker run --rm \
  -v "$LT_CHECKOUT":/lt \
  -v lt-m2:/root/.m2 \
  -v "$RS_ROOT/scripts/oracle":/oracle-src:ro \
  -v "$WORK":/in:ro \
  -v "$WORK/out":/out \
  -w /lt "$IMAGE" bash /in/inner.sh
fi

"$RS_ROOT/target/release/lt-cli" check -l pt --lines --jobs "${PT_JOBS:-8}" \
  --file "$SENTENCES" > "$WORK/out/checks.rust.tsv"
cp "$WORK/out/checks.java.tsv" "${PREFIX}.java.tsv"
cp "$WORK/out/checks.rust.tsv" "${PREFIX}.rust.tsv"

python3 "$RS_ROOT/scripts/oracle/compare-checks.py" \
  "${PREFIX}.java.tsv" "${PREFIX}.rust.tsv" "${COMPARE_SHOW:-60}"
