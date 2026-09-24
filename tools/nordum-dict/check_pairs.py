#!/usr/bin/env python3
"""Membership check for Nordum rule-data `wrong=suggestion` pairs.

The Nordum version of the `tools/nn-dict/check_pairs.py` gate, adapted to the
Nordum data model: Nordum deliberately accepts many source-language forms as
valid alternatives (spec §5.5, §7), so the wrong side of a pair is normally a
*valid speller word* — the gate therefore checks the *recommended* lexicon:

- for the source-normalization lists (source_forms, morphology, numerals,
  prepositions, loanwords) each word of the wrong form must NOT be a
  recommended Nordum word (`dictionary.json` headword or one of its generated
  spec-§4 forms, or a core addition) in its primary spelling — otherwise the
  rule would "correct" a recommended form; multi-word wrong forms (phrase
  rules) are skipped;
- every suggestion must be accepted by the generated speller: in the core
  (recommended lexicon in any accepted spelling) or in the broad converted
  source word set (`nrd.dic`);
- the compound and endonym lists keep their wrong sides unchecked (both sides
  are valid Nordum/endonym forms by design).

    python3 tools/nordum-dict/check_pairs.py \
        --nordum-dictionary /path/to/nordum/build/assets/data/dictionary.json \
        --nordum-wordlist /path/to/nordum/build/assets/data/wordlist.txt
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import nordum_convert as nc

WRONG_CHECKED_FILES = (
    "data/nrd/rules/source_forms.txt",
    "data/nrd/rules/morphology.txt",
    "data/nrd/rules/numerals.txt",
    "data/nrd/rules/prepositions.txt",
    "data/nrd/rules/loanwords.txt",
)
ALL_PAIR_FILES = WRONG_CHECKED_FILES + (
    "data/nrd/rules/endonyms.txt",
    "data/nrd/rules/compounds.txt",
)
DEFAULT_SOURCES = (
    "data/no/hunspell/nb_NO.dic",
    "data/da/hunspell/da_DK.dic",
    "data/sv/hunspell/sv_SE.dic",
)
DEFAULT_ADDITIONS = "data/nrd/words/core_additions.txt"
# Pre-existing owner pairs whose wrong side is an accepted (non-recommended)
# Nordum form: dictionary.json keeps these source-community spellings as
# accepted words (spec §5.5) while the rules steer writers to the recommended
# forms (jei, katt, cool, gjennom, ...). Listed here, not silently ignored.
ACCEPTED_SOURCE_FORMS = frozenset(
    {
        "jeg", "høy", "mig", "dig", "dere", "hennes", "deres", "kat", "kul",
        "gjennom", "til",
        # Swedish source forms that collide with accepted Nordum words or
        # spellings already in the core (`läser`/`köper` are the Danish
        # `læse`/`købe` paradigms' vowels; `arbeidar` is also the Nordum
        # noun "worker" from dictionary.json).
        "läser", "köper", "arbeidar",
    }
)


def accepted_spellings(word: str) -> set[str]:
    """All accepted spellings of one Nordum word: both vowel systems and the
    preserved diphthong patterns (spec §7)."""
    base = word.lower()
    primary = nc.primary_vowels(base)
    normalized = nc.primary_vowels(nc.normalize_diphthongs(base))
    return {primary, nc.secondary_vowels(primary), normalized, nc.secondary_vowels(normalized)}


def read_pairs(path: Path) -> list[str]:
    pairs: list[str] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.split("#")[0].strip()
        if line and "=" in line:
            pairs.append(line)
    return pairs


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("pairs", nargs="*", default=[])
    parser.add_argument("--nordum-wordlist", type=Path, required=True)
    parser.add_argument("--nordum-dictionary", type=Path, required=True)
    parser.add_argument("--additions", type=Path, default=Path(DEFAULT_ADDITIONS))
    parser.add_argument("--sources", type=Path, nargs="*", default=list(DEFAULT_SOURCES))
    args = parser.parse_args()

    # The recommended (primary-spelling) lexicon: dictionary.json headwords,
    # their generated spec-4 inflected forms and the core additions. The
    # word list's preserved source-pattern spellings are accepted variants,
    # not recommendations, so they do not disqualify a wrong form.
    recommended: set[str] = set()
    for word in nc.read_word_file(args.additions):
        recommended.add(nc.primary_vowels(word))
    for entry in nc.load_dictionary(args.nordum_dictionary):
        # Entries marked `alternativeOf` are accepted alternative spellings
        # (spec §5.5), not recommendations.
        if entry.get("alternativeOf"):
            continue
        recommended.add(nc.primary_vowels(entry["nordum"]))
        for form in nc.entry_forms(entry)[0].values():
            recommended.add(nc.primary_vowels(form))

    # The speller accepts the recommended core (all spellings) plus the
    # converted source words.
    speller: set[str] = set()
    for word in recommended:
        speller.update(accepted_spellings(word))
    for source in args.sources:
        source_path = Path(source)
        for word in nc.read_dic(source_path):
            speller.update(nc.variants(word))

    bad = 0
    checked = 0
    pair_paths: list[Path] = [Path(p) for p in (args.pairs or ALL_PAIR_FILES)]
    for path in pair_paths:
        check_wrong = str(path).replace("\\", "/") in (
            "data/nrd/rules/source_forms.txt",
            "data/nrd/rules/morphology.txt",
            "data/nrd/rules/numerals.txt",
            "data/nrd/rules/prepositions.txt",
            "data/nrd/rules/loanwords.txt",
        )
        for line in read_pairs(path):
            checked += 1
            wrong_form, _, right = line.partition("=")
            problems: list[str] = []
            if check_wrong and " " not in wrong_form:
                word = wrong_form.strip().lower()
                if nc.primary_vowels(word) in recommended and word not in ACCEPTED_SOURCE_FORMS:
                    problems.append(f"wrong form {word!r} is a recommended Nordum word")
            for suggestion in right.split("|"):
                suggestion = suggestion.strip()
                if not suggestion:
                    continue
                suggestion_accepted = all(
                    any(spelling in speller for spelling in accepted_spellings(word))
                    for word in suggestion.lower().split()
                )
                if not suggestion_accepted:
                    problems.append(f"suggestion {suggestion!r} not accepted by the speller")
            if problems:
                bad += 1
                print(f"FAIL {line}: {'; '.join(problems)}", file=sys.stderr)
            else:
                print(f"ok   {line}")
    if bad:
        print(f"{bad} of {checked} pair(s) failed", file=sys.stderr)
        return 1
    print(f"{checked} pair(s) ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
