#!/usr/bin/env bash
# Unit tests for the P6.3 affected-language filter (P6.3/D-134).
# Run: scripts/ci/tests/affected-languages-test.sh
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/affected-languages.sh"
ALL='["en","de","es","fr","it","pt","nl","ca","gl","ro","pl","sk","no","nrd","gn"]'
fails=0

check() {
  local name="$1" expected="$2"
  shift 2
  local out
  if ! out="$("$SCRIPT" --files "$@")"; then
    printf 'FAIL %s: script exited non-zero\n' "$name" >&2
    fails=$((fails + 1))
    return
  fi
  if [ "$out" = "$expected" ]; then
    printf 'ok   %s\n' "$name"
  else
    printf 'FAIL %s\n  expected: %s\n  got:      %s\n' "$name" "$expected" "$out" >&2
    fails=$((fails + 1))
  fi
}

# shared paths gate every language
check "shared engine crate" "$ALL" crates/lt-core/src/lib.rs
check "shared lt root module" "$ALL" crates/lt/src/pipeline.rs
check "language entry file (.rs beside the module) is conservatively shared" "$ALL" crates/lt/src/pt.rs
check "shared spellchecker path" "$ALL" crates/lt/src/hunspell_spelling.rs
check "other tools stay shared" "$ALL" tools/lt-sync/lt_sync.py
check "workflow change" "$ALL" .github/workflows/ci.yml
check "manifest change" "$ALL" Cargo.lock
check "common oracle script" "$ALL" scripts/oracle/compare-checks.py
check "unrecognized path" "$ALL" new/unknown/file.txt

# language-local paths gate one language
check "de data" '["de"]' data/de/grammar.xml
check "es oracle" '["es"]' scripts/oracle/es/probe-rule.sh
check "fr integration test" '["fr"]' crates/lt/tests/french.rs
check "it golden" '["it"]' docs/parity/golden/it-full.java.tsv
check "pt module" '["pt"]' crates/lt/src/pt/rules.rs
check "nl integration test" '["nl"]' crates/lt/tests/dutch.rs
check "ca integration test" '["ca"]' crates/lt/tests/catalan.rs
check "gl integration test" '["gl"]' crates/lt/tests/galician.rs
check "ro integration test" '["ro"]' crates/lt/tests/romanian.rs
check "ro module" '["ro"]' crates/lt/src/ro/rules.rs
check "ro golden" '["ro"]' docs/parity/golden/ro-full.java.tsv
check "pl integration test" '["pl"]' crates/lt/tests/polish.rs
check "pl module" '["pl"]' crates/lt/src/pl/rules.rs
check "pl golden" '["pl"]' docs/parity/golden/pl-full.java.tsv
check "sk integration test" '["sk"]' crates/lt/tests/slovak.rs
check "sk module" '["sk"]' crates/lt/src/sk/rules.rs
check "sk golden" '["sk"]' docs/parity/golden/sk-full.java.tsv

# hand-authored languages (tests-only parity gate): local layout plus the
# no/nrd/gn module entry files and the Nordum dictionary generator
check "no data" '["no"]' data/no/words/ignore.txt
check "no integration test" '["no"]' crates/lt/tests/norwegian.rs
check "no module entry file" '["no"]' crates/lt/src/no.rs
check "nrd module" '["nrd"]' crates/lt/src/nrd/rules.rs
check "nrd module entry file" '["nrd"]' crates/lt/src/nrd.rs
check "nrd integration test" '["nrd"]' crates/lt/tests/nordum.rs
check "nordum dictionary generator" '["nrd"]' tools/nordum-dict/build-nordum-dict.py
check "gn data" '["gn"]' data/gn/rules/grammar.xml
check "gn module entry file" '["gn"]' crates/lt/src/gn.rs
check "gn integration test" '["gn"]' crates/lt/tests/guarani.rs

# multiple languages, emitted in matrix order
check "multi language (fr + nl)" '["fr","nl"]' crates/lt/src/nl/rules.rs data/fr/grammar.xml
check "multi language (no + gn)" '["no","gn"]' crates/lt/src/gn/rules.rs data/no/hunspell/index.dic
check "comma-separated path list" '["nl"]' "crates/lt/src/nl/rules.rs,README.md"

# docs-only changes gate nothing
check "docs only" '[]' README.md docs/parity/golden/README.md THIRD_PARTY_NOTICES.md

# --files requires at least one path
if "$SCRIPT" --files >/dev/null 2>&1; then
  printf 'FAIL --files without paths must exit non-zero\n' >&2
  fails=$((fails + 1))
else
  printf 'ok   --files without paths exits non-zero\n'
fi

# git mode: env-driven base/head and conservative fallbacks
check_out() {
  local name="$1" expected="$2"
  shift 2
  local out
  if ! out="$("$@" 2>/dev/null)"; then
    printf 'FAIL %s: script exited non-zero\n' "$name" >&2
    fails=$((fails + 1))
    return
  fi
  if [ "$out" = "$expected" ]; then
    printf 'ok   %s\n' "$name"
  else
    printf 'FAIL %s\n  expected: %s\n  got:      %s\n' "$name" "$expected" "$out" >&2
    fails=$((fails + 1))
  fi
}

check_out "AFFECTED_BASE/AFFECTED_HEAD with no changes" '[]' \
  env AFFECTED_BASE=HEAD AFFECTED_HEAD=HEAD "$SCRIPT"
check_out "unresolvable base falls back to all languages" "$ALL" \
  "$SCRIPT" deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef HEAD
check_out "all-zero GITHUB_EVENT_BEFORE falls back to all languages" "$ALL" \
  env GITHUB_EVENT_BEFORE=0000000000000000000000000000000000000000 "$SCRIPT"

# the summary goes to stderr, the JSON to stdout
err="$("$SCRIPT" --files crates/lt/src/pt/rules.rs 2>&1 >/dev/null)"
case "$err" in
  *"crates/lt/src/pt/rules.rs -> pt"*) printf 'ok   human-readable summary on stderr\n' ;;
  *)
    printf 'FAIL stderr summary missing the per-file classification:\n%s\n' "$err" >&2
    fails=$((fails + 1))
    ;;
esac

# every emitted array is valid JSON
json_ok=1
for paths in "crates/lt-core/src/lib.rs" "crates/lt/src/pt/rules.rs" "README.md" "crates/lt/src/nl/rules.rs,data/fr/grammar.xml" "crates/lt/src/no.rs,tools/nordum-dict/build-nordum-dict.py"; do
  if ! "$SCRIPT" --files "$paths" | python3 -c 'import json,sys; data=json.load(sys.stdin); assert isinstance(data, list); assert all(isinstance(x, str) for x in data)'; then
    json_ok=0
  fi
done
if [ "$json_ok" -eq 1 ]; then
  printf 'ok   emitted JSON parses (python3 json)\n'
else
  printf 'FAIL emitted JSON did not parse\n' >&2
  fails=$((fails + 1))
fi

if [ "$fails" -ne 0 ]; then
  printf '%d affected-language test(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'all affected-language tests passed\n'
