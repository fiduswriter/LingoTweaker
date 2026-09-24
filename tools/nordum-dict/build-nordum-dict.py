#!/usr/bin/env python3
"""Build the Nordum Hunspell dictionary.

Nordum's vocabulary is largely Scandinavian; its orthography is a regularized
union of Norwegian Bokmål, Danish and Swedish (NORDUM_LANGUAGE_SPECIFICATION.md
§3.3.2, §3.6, §7). This tool applies the Nordum orthographic rules to the
source-language Hunspell dictionaries and merges them with the authoritative
Nordum word list, producing:

- ``nrd.dic``       — the speller dictionary (broad coverage),
- ``nrd_core.dic``  — the authoritative core (+ accepted alternatives), used
                      as the suggestion candidate list.

The authoritative Nordum word list carries POS information
(``dictionary.json``), so its noun/verb/adjective paradigms are generated per
spec §4 (see ``nordum_convert``) and added to both dictionaries; the
converted source-language words stay morphology-free (the sources do not
carry reliable POS information, and blind re-suffixing produces wrong forms,
e.g. Swedish ``flicka`` must not become ``flicke``).

Usage:
  python3 build-nordum-dict.py \
      --nordum-wordlist /path/to/nordum/build/assets/data/wordlist.txt \
      --nordum-dictionary /path/to/nordum/build/assets/data/dictionary.json \
      --additions data/nrd/words/core_additions.txt \
      --sources nb_NO.dic da_DK.dic sv_SE.dic \
      --out data/nrd/hunspell/nrd.dic \
      --out-core data/nrd/hunspell/nrd_core.dic \
      --report data/nrd/hunspell/build-report.txt

  python3 build-nordum-dict.py --self-test
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import nordum_convert as nc

# Spec §3.3.2 / §3.6 / §4 examples the converter and the morphology generator
# must reproduce.
SELF_TEST_CASES: list[tuple[str, str]] = [
    # §3.3.2 rules 1-7 + §3.6 diphthongs
    ("tack", "takk"),
    ("backa", "bakka"),
    ("fiks", "fix"),
    ("maks", "max"),
    ("boks", "box"),
    ("centrum", "sentrum"),
    ("cirkel", "sirkel"),
    ("philosophi", "filosofi"),
    ("forskjell", "forskell"),
    ("skjære", "skære"),
    ("kald", "kall"),
    ("fuld", "full"),
    ("hvad", "vad"),
    # `hv`-drop only; the `var`/`varför` lexical selections are rule data.
    ("hvor", "vor"),
    ("hvorfor", "vorfor"),
    ("vejr", "veir"),
    ("nej", "nei"),
    ("maj", "mei"),
    ("høj", "høi"),
    ("høy", "høi"),
    # The silent-d resolution (mad → mat) is lexical-selection rule data.
    ("hvilken", "vilken"),
    ("computer", "komputer"),  # c-rule applies: `komputer` variant
    ("jentarna", "jentarna"),
]


def self_test() -> int:
    failures = 0
    for source, expected in SELF_TEST_CASES:
        if expected not in nc.variants(source):
            failures += 1
            print(f"FAIL {source!r}: {expected!r} not in {sorted(nc.variants(source))}")
    # `ch` words keep their spelling (spec §3.3.2 rule 3 note).
    if "chocolate" not in nc.variants("chocolate"):
        failures += 1
        print("FAIL chocolate must be kept unchanged")
    # Verb paradigm (§4.2): the systematic -er/-ede/-a/-et/-ende/stem forms.
    forms, counts = nc.verb_forms({"infinitive": "arbeide"})
    expected = {
        "infinitive": "arbeide",
        "present": "arbeider",
        "past": "arbeidede",
        "supine": "arbeidet",
        "presentParticiple": "arbeidende",
        "imperative": "arbeid",
        "pastInformal": "arbeida",
    }
    if forms != expected:
        failures += 1
        print(f"FAIL verb paradigm: {forms}")
    if counts.get("pastInformal:generated") != 1:
        failures += 1
        print(f"FAIL verb counts: {counts}")
    # Provided inflections win over the systematic fallbacks (irregulars).
    forms, _ = nc.verb_forms(
        {"infinitive": "gå", "present": "går", "past": "gikk", "supine": "gått",
         "pastParticiple": "gått"}
    )
    if forms.get("present") != "går" or forms.get("past") != "gikk":
        failures += 1
        print(f"FAIL irregular verb: {forms}")
    if "presentParticiple" not in forms:
        failures += 1
        print(f"FAIL irregular verb participle: {forms}")
    # No stem, no guesses (§4.2.3).
    if nc.verb_forms({}) != ({}, {}):
        failures += 1
        print("FAIL empty verb inflections must generate nothing")
    # Noun paradigm (§4.3).
    noun, _ = nc.noun_forms(
        {"singular": {"indefinite": "jente", "definite": "jenten"},
         "plural": {"indefinite": "jentar", "definite": "jentarna"}}, "common")
    if noun != {"singularDefinite": "jenten", "pluralIndefinite": "jentar",
                "pluralDefinite": "jentarna"}:
        failures += 1
        print(f"FAIL noun paradigm: {noun}")
    fallback, counts = nc.noun_forms({}, "common")
    if fallback != {"singularDefinite": "en", "pluralIndefinite": "ar",
                    "pluralDefinite": "arna"}:
        failures += 1
        print(f"FAIL noun fallback: {fallback}")
    if counts.get("pluralDefinite:generated") != 1:
        failures += 1
        print(f"FAIL noun fallback counts: {counts}")
    # Adjective paradigm (§4.4): neuter -t, comparative -ere.
    adj, _ = nc.adjective_forms({}, "stor")
    if adj["positiveNeuter"] != "stort" or adj["positivePlural"] != "store":
        failures += 1
        print(f"FAIL adjective fallback: {adj}")
    adj, _ = nc.adjective_forms(
        {"comparative": "godere", "superlative": "godest",
         "positive": {"common": "god", "neuter": "godt", "plural": "gode",
                      "definite": "gode"}}, "god")
    if adj["comparative"] != "godere" or adj["superlative"] != "godest":
        failures += 1
        print(f"FAIL adjective inflections: {adj}")
    if failures:
        print(f"{failures} self-test failure(s)")
        return 1
    print("self-test ok")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nordum-wordlist", type=Path)
    parser.add_argument("--nordum-dictionary", type=Path)
    parser.add_argument("--additions", type=Path)
    parser.add_argument("--sources", type=Path, nargs="*", default=[])
    parser.add_argument("--out", type=Path)
    parser.add_argument("--out-core", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if not (args.nordum_wordlist and args.out and args.out_core):
        parser.error("--nordum-wordlist, --out and --out-core are required")

    core: set[str] = set()
    for word in nc.read_word_file(args.nordum_wordlist):
        core.add(word)
    entries: list[dict] = []
    if args.nordum_dictionary and args.nordum_dictionary.exists():
        entries = nc.load_dictionary(args.nordum_dictionary)
        for entry in entries:
            if entry.get("alternativeOf"):
                core.add(entry["nordum"])
    additions = 0
    if args.additions and args.additions.exists():
        for word in nc.read_word_file(args.additions):
            core.add(word)
            additions += 1

    dictionary: set[str] = set(core)
    # N1: the authoritative paradigms (spec §4) enter both dictionaries as
    # full forms — the only morphology source with a reliable POS.
    generated = 0
    verb_entries = noun_entries = adj_entries = 0
    generated = 0
    for entry in entries:
        forms, _counts = nc.entry_forms(entry)
        if entry["pos"] == "verb":
            verb_entries += 1
        elif entry["pos"] == "noun":
            noun_entries += 1
        elif entry["pos"] == "adjective":
            adj_entries += 1
        for form in forms.values():
            variants = nc.lexicon_variants(form)
            dictionary.update(variants)
            core.update(variants)
            generated += len(variants)
    source_words = 0
    for source in args.sources:
        for word in nc.read_dic(source):
            source_words += 1
            dictionary.update(nc.variants(word))

    # Core: both vowel systems plus accepted source alternations (diphthongs).
    core_variants: set[str] = set()
    for word in core:
        core_variants.add(nc.primary_vowels(word))
        core_variants.add(nc.secondary_vowels(nc.primary_vowels(word)))
        core_variants.add(nc.primary_vowels(nc.normalize_diphthongs(word)))
    dictionary.update(core_variants)

    core_sorted = sorted(core_variants)
    all_sorted = sorted(dictionary)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(
        f"{len(all_sorted)}\n" + "\n".join(all_sorted) + "\n", encoding="utf-8"
    )
    args.out_core.write_text(
        f"{len(core_sorted)}\n" + "\n".join(core_sorted) + "\n", encoding="utf-8"
    )
    if args.report:
        args.report.write_text(
            "Nordum dictionary build\n"
            f"core words (wordlist + alternatives + additions): {len(core)} "
            f"(+{additions} additions)\n"
            "generated inflected forms (spec §4, dictionary.json paradigms):\n"
            f"  verb entries: {verb_entries}\n"
            f"  noun entries: {noun_entries}\n"
            f"  adjective entries: {adj_entries}\n"
            f"  generated form variants added: {generated}\n"
            f"source words read: {source_words}\n"
            f"core variants written: {len(core_sorted)}\n"
            f"dictionary entries written: {len(all_sorted)}\n"
            f"sources: {', '.join(p.name for p in args.sources)}\n",
            encoding="utf-8",
        )
    print(f"wrote {args.out} ({len(all_sorted)} entries)")
    print(f"wrote {args.out_core} ({len(core_sorted)} entries)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
