#!/usr/bin/env bash
# Unit tests for the P6.3 affected-language filter (P6.3/D-134).
# Run: scripts/ci/tests/affected-languages-test.sh
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/affected-languages.sh"
ALL='["en","de-x-simple","de","es","fr","it","pt","nl","ca","gl","ro","pl","sk","sl","el","da","sv","is","eo","ast","br","tl","lt","crh","be","ru","uk","sr","ar","fa","km","ml","ta","ja","zh","no","nrd","nn","gn"]'
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
check "de data" '["de-x-simple","de"]' data/de/grammar.xml
check "de-x-simple rule data" '["de-x-simple"]' data/de/rules/de-DE-x-simple-language/grammar.xml
check "de-x-simple integration test" '["de-x-simple"]' crates/lt/tests/de_x_simple.rs
check "de-x-simple oracle" '["de-x-simple"]' scripts/oracle/de-x-simple/check-diff-de-x-simple.sh
check "de-x-simple golden" '["de-x-simple"]' docs/parity/golden/de-x-simple-full.java.tsv
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
check "sl integration test" '["sl"]' crates/lt/tests/slovenian.rs
check "sl module" '["sl"]' crates/lt/src/sl/rules.rs
check "sl golden" '["sl"]' docs/parity/golden/sl-full.java.tsv
check "el integration test" '["el"]' crates/lt/tests/greek.rs
check "el module" '["el"]' crates/lt/src/el/rules.rs
check "el golden" '["el"]' docs/parity/golden/el-full.java.tsv
check "da integration test" '["da"]' crates/lt/tests/danish.rs
check "da module" '["da"]' crates/lt/src/da/spelling.rs
check "da golden" '["da"]' docs/parity/golden/da-full.java.tsv
check "sv integration test" '["sv"]' crates/lt/tests/swedish.rs
check "sv module" '["sv"]' crates/lt/src/sv/spelling.rs
check "sv golden" '["sv"]' docs/parity/golden/sv-full.java.tsv
check "is integration test" '["is"]' crates/lt/tests/icelandic.rs
check "is module" '["is"]' crates/lt/src/is/spelling.rs
check "is golden" '["is"]' docs/parity/golden/is-full.java.tsv
check "eo integration test" '["eo"]' crates/lt/tests/esperanto.rs
check "eo module" '["eo"]' crates/lt/src/eo/spelling.rs
check "eo golden" '["eo"]' docs/parity/golden/eo-full.java.tsv
check "ast integration test" '["ast"]' crates/lt/tests/asturian.rs
check "ast module" '["ast"]' crates/lt/src/ast/spelling.rs
check "ast golden" '["ast"]' docs/parity/golden/ast-full.java.tsv
check "br integration test" '["br"]' crates/lt/tests/breton.rs
check "br module" '["br"]' crates/lt/src/br/spelling.rs
check "br golden" '["br"]' docs/parity/golden/br-full.java.tsv
check "tl integration test" '["tl"]' crates/lt/tests/tagalog.rs
check "tl module" '["tl"]' crates/lt/src/tl/spelling.rs
check "tl golden" '["tl"]' docs/parity/golden/tl-full.java.tsv
check "lt integration test" '["lt"]' crates/lt/tests/lithuanian.rs
check "lt module" '["lt"]' crates/lt/src/lt/spelling.rs
check "crh integration test" '["crh"]' crates/lt/tests/crimean_tatar.rs
check "crh module" '["crh"]' crates/lt/src/crh/spelling.rs
check "crh golden" '["crh"]' docs/parity/golden/crh-full.java.tsv
check "be integration test" '["be"]' crates/lt/tests/belarusian.rs
check "be module" '["be"]' crates/lt/src/be/spelling.rs
check "be golden" '["be"]' docs/parity/golden/be-full.java.tsv
check "ru integration test" '["ru"]' crates/lt/tests/russian.rs
check "ru module" '["ru"]' crates/lt/src/ru/rules.rs
check "ru golden" '["ru"]' docs/parity/golden/ru-full.java.tsv
check "uk integration test" '["uk"]' crates/lt/tests/ukrainian.rs
check "uk module" '["uk"]' crates/lt/src/uk/hybrid.rs
check "uk golden" '["uk"]' docs/parity/golden/uk-full.java.tsv
check "sr integration test" '["sr"]' crates/lt/tests/serbian.rs
check "sr module" '["sr"]' crates/lt/src/sr/rules.rs
check "sr oracle" '["sr"]' scripts/oracle/sr/check-diff-sr.sh
check "sr golden" '["sr"]' docs/parity/golden/sr-full.java.tsv
check "ar integration test" '["ar"]' crates/lt/tests/arabic.rs
check "ar module" '["ar"]' crates/lt/src/ar/filters.rs
check "ar oracle" '["ar"]' scripts/oracle/ar/check-diff-ar.sh
check "ar golden" '["ar"]' docs/parity/golden/ar-full.java.tsv
check "fa integration test" '["fa"]' crates/lt/tests/persian.rs
check "fa oracle" '["fa"]' scripts/oracle/fa/check-diff-fa.sh
check "fa golden" '["fa"]' docs/parity/golden/fa-full.java.tsv
check "km integration test" '["km"]' crates/lt/tests/khmer.rs
check "km oracle" '["km"]' scripts/oracle/km/check-diff-km.sh
check "km golden" '["km"]' docs/parity/golden/km-full.java.tsv
check "ml integration test" '["ml"]' crates/lt/tests/malayalam.rs
check "ml oracle" '["ml"]' scripts/oracle/ml/check-diff-ml.sh
check "ml golden" '["ml"]' docs/parity/golden/ml-full.java.tsv
check "ta integration test" '["ta"]' crates/lt/tests/tamil.rs
check "ta oracle" '["ta"]' scripts/oracle/ta/check-diff-ta.sh
check "ta golden" '["ta"]' docs/parity/golden/ta-full.java.tsv

# CJK (tests-only parity gate; the Lindera dictionary is generated, not tracked)
check "ja data" '["ja"]' data/ja/rules/grammar.xml
check "ja module entry file" '["ja"]' crates/lt/src/ja.rs
check "ja integration test" '["ja"]' crates/lt/tests/japanese.rs
check "zh data" '["zh"]' data/zh/rules/grammar.xml
check "zh module entry file" '["zh"]' crates/lt/src/zh.rs
check "zh integration test" '["zh"]' crates/lt/tests/chinese.rs

# hand-authored languages (tests-only parity gate): local layout plus the
# no/nrd/gn module entry files and the Nordum dictionary generator
check "no data" '["no"]' data/no/words/ignore.txt
check "no integration test" '["no"]' crates/lt/tests/norwegian.rs
check "no module entry file" '["no"]' crates/lt/src/no.rs
check "nrd module" '["nrd"]' crates/lt/src/nrd/rules.rs
check "nrd module entry file" '["nrd"]' crates/lt/src/nrd.rs
check "nrd integration test" '["nrd"]' crates/lt/tests/nordum.rs
check "nordum dictionary generator" '["nrd"]' tools/nordum-dict/build-nordum-dict.py
check "nn data" '["nn"]' data/nn/rules/grammar.xml
check "nn module" '["nn"]' crates/lt/src/nn/rules.rs
check "nn module entry file" '["nn"]' crates/lt/src/nn.rs
check "nn integration test" '["nn"]' crates/lt/tests/nynorsk.rs
check "nn dictionary tools" '["nn"]' tools/nn-dict/derive_bokmaal_forms.py
check "gn data" '["gn"]' data/gn/rules/grammar.xml
check "gn module entry file" '["gn"]' crates/lt/src/gn.rs
check "gn integration test" '["gn"]' crates/lt/tests/guarani.rs

# multiple languages, emitted in matrix order
check "multi language (fr + nl)" '["fr","nl"]' crates/lt/src/nl/rules.rs data/fr/grammar.xml
check "multi language (uk + ru)" '["ru","uk"]' crates/lt/src/uk/hybrid.rs crates/lt/src/ru/rules.rs
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
