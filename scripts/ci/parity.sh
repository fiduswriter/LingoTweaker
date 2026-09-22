#!/usr/bin/env bash
# Offline per-language corpus parity gate: diffs `lt-cli check --lines`
# against the pinned Java CheckDump golden in docs/parity/golden (captured
# once with Docker; CI needs none). See docs/parity/golden/README.md.
#
# Languages without a usable Java oracle (no, nrd, gn; lt has a broken
# upstream module) get a tests-only gate: the
# per-language integration test plus an `lt-cli inventory` rule-count sanity
# check. No Docker, no Java, no golden.
#
# Usage: scripts/ci/parity.sh <en|de|es|fr|it|pt|nl|ca|gl|ro|pl|sk|sl|el|da|sv|is|eo|ast|br|tl|lt|crh|be|ru|uk|sr|ar|fa|no|nrd|gn>
#   the Java-oracle languages require target/release/lt-cli; the tests-only
#   languages use target/release/lt-cli or target/debug/lt-cli
set -euo pipefail

LANG_ARG="${1:?usage: scripts/ci/parity.sh <en|de|es|fr|it|pt|nl|ca|gl|ro|pl|sk|sl|el|da|sv|is|eo|ast|br|tl|lt|crh|be|ru|uk|sr|ar|fa|no|nrd|gn>}"
RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

# Tests-only gate for languages without a Java oracle (no/nrd/gn): there is
# no pinned golden to diff, so assert the integration test passes and that
# `lt-cli inventory` loads the language rules (> 0 rules).
case "$LANG_ARG" in
no | nrd | gn | lt)
  case "$LANG_ARG" in
  no) TEST_SUITE=norwegian ;;
  nrd) TEST_SUITE=nordum ;;
  gn) TEST_SUITE=guarani ;;
  # `lt` has a legacy module but it is unusable: the referenced
  # `lt/hunspell/lt_LT.dict` is not shipped upstream, so the legacy engine
  # throws on every check (docs/differences.md #12). The Rust engine vendors a
  # third-party ispell-lt dictionary under the unchanged rule id and runs the
  # XML + generic rules; there is no Java baseline, so the gate is tests-only.
  lt) TEST_SUITE=lithuanian ;;
  esac
  LT_CLI=""
  for candidate in "$RS_ROOT/target/release/lt-cli" "$RS_ROOT/target/debug/lt-cli"; do
    if [ -x "$candidate" ]; then
      LT_CLI="$candidate"
      break
    fi
  done
  [ -n "$LT_CLI" ] || { echo "build first: cargo build -p lt-cli" >&2; exit 2; }
  echo "== tests-only gate (no Java oracle for this language): cargo test -p lt --test $TEST_SUITE"
  cargo test -p lt --test "$TEST_SUITE"
  echo "== tests-only gate (no Java oracle for this language): lt-cli inventory --lang $LANG_ARG"
  inventory="$("$LT_CLI" inventory --lang "$LANG_ARG")"
  printf '%s\n' "$inventory"
  n_rules="$(printf '%s\n' "$inventory" | awk '/^rules: /{print $2; exit}')"
  case "$n_rules" in
  '' | *[!0-9]*) echo "inventory: missing or non-numeric 'rules:' line" >&2; exit 1 ;;
  esac
  [ "$n_rules" -gt 0 ] || { echo "inventory: $LANG_ARG reports no rules" >&2; exit 1; }
  echo "TESTS-ONLY GATE OK ($LANG_ARG, $n_rules rules)"
  exit 0
  ;;
esac

GOLDEN="$RS_ROOT/docs/parity/golden"
# date filters are pinned to the (per-language) golden capture date
# (golden/README.md)
TODAY="${PARITY_TODAY:-2026-09-18}"
if [ "$LANG_ARG" = "es" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-20"
fi
if [ "$LANG_ARG" = "fr" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-19"
fi
if [ "$LANG_ARG" = "it" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-19"
fi
if [ "$LANG_ARG" = "pt" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-20"
fi
if [ "$LANG_ARG" = "nl" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-19"
fi
if [ "$LANG_ARG" = "ca" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-20"
fi
if [ "$LANG_ARG" = "gl" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-20"
fi
if [ "$LANG_ARG" = "ro" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-20"
fi
if [ "$LANG_ARG" = "pl" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-20"
fi
if [ "$LANG_ARG" = "sk" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "sl" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "el" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "da" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "sv" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "is" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "eo" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "ast" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "br" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "tl" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "crh" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-21"
fi
if [ "$LANG_ARG" = "be" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-22"
fi
if [ "$LANG_ARG" = "ru" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-22"
fi
if [ "$LANG_ARG" = "uk" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-22"
fi
if [ "$LANG_ARG" = "sr" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-22"
fi
if [ "$LANG_ARG" = "ar" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-22"
fi
if [ "$LANG_ARG" = "fa" ] && [ -z "${PARITY_TODAY:-}" ]; then
  TODAY="2026-09-23"
fi
JOBS="${PARITY_JOBS:-$(nproc 2>/dev/null || echo 4)}"
BIN="$RS_ROOT/target/release/lt-cli"

INPUT="$GOLDEN/$LANG_ARG-full.txt"
JAVA="$GOLDEN/$LANG_ARG-full.java.tsv"
[ -f "$INPUT" ] || { echo "missing golden input: $INPUT" >&2; exit 2; }
[ -f "$JAVA" ] || { echo "missing golden java dump: $JAVA" >&2; exit 2; }
[ -x "$BIN" ] || { echo "build first: cargo build --release -p lt-cli" >&2; exit 2; }

RUST="$(mktemp)"
trap 'rm -f "$RUST"' EXIT
"$BIN" check -l "$LANG_ARG" --lines --jobs "$JOBS" --today "$TODAY" --file "$INPUT" > "$RUST"

EXTRA=()
if [ "$LANG_ARG" = "en" ]; then
  # the one documented deliberate divergence (docs/differences.md #1)
  EXTRA+=(--expect-field-diffs=ADVERB_VERB_ADVERB_REPETITION=1)
elif [ "$LANG_ARG" = "es" ]; then
  # documented deliberate divergence (docs/differences.md #8): the
  # hand-authored demonstrative-verb rules in es/rules/local.xml. The corpus
  # contains their 11 incorrect examples, which Java does not report.
  EXTRA+=(--expect-only-rust=AGREEMENT_DEMONSTRATIVE_VERB=11)
elif [ "$LANG_ARG" = "pt" ]; then
  # documented deliberate divergence (docs/differences.md #6)
  EXTRA+=(--expect-field-diffs=PODER_SER_POSSIVEL=1)
elif [ "$LANG_ARG" = "gl" ]; then
  # documented speller divergence (docs/differences.md #7): the match set is
  # identical; the native hunspell suggestion engine (generators + n-gram
  # fallback) is ported, leaving a small wall-clock-boundary residue
  EXTRA+=(--expect-field-diffs=HUNSPELL_RULE=2)
elif [ "$LANG_ARG" = "fr" ]; then
  # documented deliberate divergences (docs/differences.md #3, #4, #5)
  EXTRA+=(--expect-only-java=FRENCH_WORD_REPEAT_RULE=7)
  EXTRA+=(--expect-only-java=SUJET_AUXILIAIRE=1)
  EXTRA+=(--expect-field-diffs=AGREEMENT_PARTICULAR=1)
elif [ "$LANG_ARG" = "crh" ]; then
  # documented divergence (docs/differences.md #13): Java's
  # CASE_INSENSITIVE|UNICODE_CASE folds U+0131 (dotless i) into [A-Za-z],
  # so `21fayız` matches COMPLEX_NUMBER_DEFIS_MISSING; the Rust regex crate's
  # simple case folding does not.
  EXTRA+=(--expect-only-java=COMPLEX_NUMBER_DEFIS_MISSING=1)
elif [ "$LANG_ARG" = "tl" ]; then
  # documented suggestion-order divergence (docs/differences.md #11): the
  # match and suggestion *sets* are identical; the frequency-included
  # tl_PH Morfologik dictionary orders the frequency-weighted candidates
  # differently for `nag`.
  EXTRA+=(--expect-field-diffs=MORFOLOGIK_RULE_TL=5)
elif [ "$LANG_ARG" = "pl" ]; then
  # documented known fidelity gaps (docs/differences.md #9): the
  # <unify negate="yes"> agreement rules, the ZDANIA_ZLOZONE comp:comma
  # disambiguation context and the PCON_VERB participle rule
  EXTRA+=(--expect-only-java=ADJ_SUBST_ADJ_UNIFY=1)
  EXTRA+=(--expect-only-java=SUBST_ADJ_UNIFY=1)
  EXTRA+=(--expect-only-java=PCON_VERB=1)
  EXTRA+=(--expect-only-java=ZDANIA_ZLOZONE=1)
  EXTRA+=(--expect-only-rust=ADJ_SUBST_ADJ_UNIFY=1)
  EXTRA+=(--expect-only-rust=NIEZGODNO_PRZYPADKW_PRZYMIOTNIKA_I_RZECZOWNIKA_RODZAJU_ESKIEGO=2)
  EXTRA+=(--expect-only-rust=NIEZGODNOSC_LICZBY_PODMIOTU_I_ORZECZENIA=1)
  EXTRA+=(--expect-only-rust=ZDANIA_ZLOZONE=1)
elif [ "$LANG_ARG" = "uk" ]; then
  # documented known fidelity gaps (docs/differences.md #14): the XML
  # disambiguation forward-scan cascade (`non_v_kly_2`), a prep+`не`+noun
  # case-government gap, an overlap tie-break for a proper-name list and the
  # abbreviation sentence segmentation (`т. 2 ч. 1`).
  EXTRA+=(--expect-only-java=UK_PREP_NOUN_INFLECTION_AGREEMENT=1)
  EXTRA+=(--expect-only-java=UPPERCASE_SENTENCE_START=2)
  EXTRA+=(--expect-only-rust=UK_ADJ_NOUN_INFLECTION_AGREEMENT=1)
  EXTRA+=(--expect-field-diffs=UK_ADJ_NOUN_INFLECTION_AGREEMENT=2)
elif [ "$LANG_ARG" = "ar" ]; then
  # documented known fidelity gaps (docs/differences.md #15): the
  # `syntax_numeric_0003` number-phrase rule (its `ArabicNumbersWords`
  # number-to-words engine is not ported, so the filter rejects and the rule is
  # inert) and the rule classes still unported (`AR_INFLECTED_ONE_WORD`,
  # `AR_VERB_TRANSITIVE_IINDIRECT`). The only-Rust matches are the secondary
  # rules that leak through where Java's missing number match would win the
  # overlap; the two speller boundaries are Hunspell range/wrong-split residue.
  EXTRA+=(--expect-only-java=syntax_numeric_0003=10)
  EXTRA+=(--expect-only-java=AR_INFLECTED_ONE_WORD=1)
  EXTRA+=(--expect-only-java=AR_VERB_TRANSITIVE_IINDIRECT=1)
  EXTRA+=(--expect-only-java=HUNSPELL_RULE_AR=2)
  EXTRA+=(--expect-only-rust=typo_000_tanwin_nasb=3)
  EXTRA+=(--expect-only-rust=AR_SIMPLE_REPLACE=1)
  EXTRA+=(--expect-only-rust=verb_287_yTAlhA_AlqAnwn=1)
  EXTRA+=(--expect-only-rust=grammar_0000_jar_dual=1)
  EXTRA+=(--expect-only-rust=grammar_0000_jar_plural=1)
  EXTRA+=(--expect-only-rust=number_21to99_majrour_separate_jar=1)
elif [ "$LANG_ARG" = "fa" ]; then
  # documented shared regex-semantics divergence (docs/differences.md #16):
  # Java's token `\w` is ASCII (no UNICODE_CHARACTER_CLASS), the Rust regex
  # crate's is Unicode, so the first Bad_ZWNJ rule matches the Persian letters
  # before a ZWNJ on the ZWNJ_Connection correct examples.
  EXTRA+=(--expect-only-rust=Bad_ZWNJ=258)
fi

python3 "$RS_ROOT/scripts/oracle/compare-checks.py" "$JAVA" "$RUST" 0 \
  --fail-on-diff "${EXTRA[@]}"
echo "PARITY OK ($LANG_ARG)"
