#!/usr/bin/env bash
# Build and run the pinned legacy Java oracle in Docker.
set -euo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
PORT="${PORT:-8080}"
IMAGE="maven:3.9-eclipse-temurin-21"

echo "Building LanguageTool from pinned checkout: $LT_CHECKOUT (this can take a while)"
docker run --rm -v "$LT_CHECKOUT":/lt -w /lt "$IMAGE" \
  mvn -q -DskipTests package

echo "Starting server on :$PORT"
docker run --rm -p "$PORT:8080" -v "$LT_CHECKOUT":/lt -w /lt "$IMAGE" \
  java -cp languagetool-server/target/languagetool-server-jar-with-dependencies.jar \
    org.languagetool.server.HTTPServer --port 8080 --public --allow-origin-urls '*'
