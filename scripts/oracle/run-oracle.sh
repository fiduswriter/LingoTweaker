#!/usr/bin/env bash
# Build and run the pinned legacy Java oracle in Docker.
#
# Two things this script needs that a plain `mvn package` does not give, both of
# which used to make it fail on a clean checkout:
#
#   - the `fat-jar` profile. Without it the server jar is ~200 kB and has no
#     dependencies, so the JVM cannot start. The profile shades the module's
#     main artifact *in place*, keeping the pre-shade copy as `original-*.jar`,
#     so the runnable jar is `languagetool-server-<version>.jar` and NOT the
#     `*-jar-with-dependencies.jar` name this script used to assume.
#   - the `lt-m2` volume. The dependency cache is ~1 GB; without it every run
#     re-downloads the world. `scripts/oracle/dump-readings.sh` already uses it.
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
PORT="${PORT:-8080}"
IMAGE="maven:3.9-eclipse-temurin-21"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  echo "Building LanguageTool from pinned checkout: $LT_CHECKOUT (this can take a while)"
  docker run --rm -v "$LT_CHECKOUT":/lt -v lt-m2:/root/.m2 -w /lt "$IMAGE" \
    mvn -q -DskipTests -Pfat-jar package
fi

# `original-*.jar` is the pre-shade module jar; the other `languagetool-server-*`
# jar is the shaded one the server class needs.
jar=$(ls -1 "$LT_CHECKOUT"/languagetool-server/target/languagetool-server-*.jar 2>/dev/null \
  | grep -v '/original-' \
  | head -1)
if [ -z "$jar" ]; then
  echo "no runnable server jar in $LT_CHECKOUT/languagetool-server/target — build with -Pfat-jar" >&2
  exit 2
fi
echo "Using $(basename "$jar") ($(du -h "$jar" | cut -f1))"

# Fail early and legibly if something else already holds the port: a busy port
# answers with *that* service, and a fixture captured from it would be silently
# wrong rather than obviously broken.
if curl -s -m 3 -o /dev/null "http://localhost:$PORT/v2/info" 2>/dev/null; then
  echo "port $PORT is already serving; set PORT= to something free" >&2
  exit 2
fi

echo "Starting server on :$PORT"
exec docker run --rm -p "$PORT:8080" -v "$LT_CHECKOUT":/lt -w /lt "$IMAGE" \
  java -cp "languagetool-server/target/$(basename "$jar")" \
  org.languagetool.server.HTTPServer --port 8080 --public --allow-origin-urls '*'