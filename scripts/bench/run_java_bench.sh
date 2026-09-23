#!/usr/bin/env bash
# Compile + run the Java timing harness inside the oracle Docker image
# (the host has only a JRE). Results go to scripts/bench/results/java-<lang>.log.
set -euo pipefail
cd "$(dirname "$0")/../.."   # repo root

IMG=maven:3.9-eclipse-temurin-21
ROOT="$(pwd)"
BENCH=scripts/bench

docker run --rm -v "$ROOT/..:/src" -w /src/languagetool-rs "$IMG" sh "$BENCH/bench_in_docker.sh"
