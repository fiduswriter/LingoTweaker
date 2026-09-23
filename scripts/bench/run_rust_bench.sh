#!/usr/bin/env bash
# Time the Rust engine over the benchmark corpora (scripts/bench/results/<lang>.txt).
# Uses /usr/bin/time for peak RSS; splits init (engine build) from steady state
# by running a 1-sentence file first and subtracting.
set -euo pipefail
cd "$(dirname "$0")/../.."
BENCH=scripts/bench
BIN="${LT_CLI_BIN:-target/release/lt-cli}"

for lang in en de es pt fr; do
  case "$lang" in
    en) CODE=en-US ;; de) CODE=de-DE ;; es) CODE=es ;; pt) CODE=pt-PT ;; fr) CODE=fr ;;
  esac
  echo "=== Rust $lang ($CODE) ==="
  n=$(grep -c . "$BENCH/results/$lang.txt")
  # 1-line file for init timing
  head -1 "$BENCH/results/$lang.txt" > "$BENCH/results/$lang-1.txt"
  /usr/bin/time -f "%e s  %M KB maxrss" "$BIN" check --lang "$CODE" --lines --jobs 1 \
    --file "$BENCH/results/$lang-1.txt" > /dev/null 2>"$BENCH/results/rust-$lang-init.log" || true
  /usr/bin/time -f "%e s  %M KB maxrss" "$BIN" check --lang "$CODE" --lines --jobs 1 \
    --file "$BENCH/results/$lang.txt" > /dev/null 2>"$BENCH/results/rust-$lang.log" || true
  cat "$BENCH/results/rust-$lang-init.log" "$BENCH/results/rust-$lang.log"
done
