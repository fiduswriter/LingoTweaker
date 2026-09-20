#!/usr/bin/env bash
# Capture golden HTTP fixtures from a running Java LT oracle.
set -euo pipefail

BASE="${1:-http://localhost:8080}"
OUT="$(cd "$(dirname "$0")/../.." && pwd)/docs/parity/api-fixtures"
mkdir -p "$OUT/responses"

capture() {
  local name="$1" path="$2" data="${3:-}"
  if [ -n "$data" ]; then
    curl -sS -X POST "$BASE$path" -d "$data" -o "$OUT/responses/$name.json"
  else
    curl -sS "$BASE$path" -o "$OUT/responses/$name.json"
  fi
  echo "captured $name"
}

capture languages /v2/languages
capture info /v2/info
capture maxtextlength /v2/maxtextlength
capture configinfo "/v2/configinfo?language=en-US"
capture check_empty /v2/check "text=&language=en-US"
capture check_ascii /v2/check "text=This+is+a+test.&language=en-US"
capture check_emoji /v2/check "text=Hello+%F0%9F%98%80+world.&language=en-US"
capture check_de /v2/check "text=Das+ist+ein+Test.&language=de-DE"
capture check_es /v2/check "text=Esto+es+una+prueba.&language=es"
capture check_fr /v2/check "text=C%27est+un+test.&language=fr"
capture check_legacy_param /v2/check "text=Hi&language=en-US&enabled=FOO"
capture check_missing_language /v2/check "text=Hi"
capture check_data /v2/check "data=%7B%22annotation%22%3A%5B%7B%22text%22%3A%22Hi%22%7D%5D%7D&language=en-US"

echo "done -> $OUT/responses/"
