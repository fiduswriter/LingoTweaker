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

The morphological endings (noun plurals, verb tenses) are *not* rewritten by
the source-language transformations: the sources do not carry reliable POS
information, and blind re-suffixing produces wrong forms (e.g. Swedish
``flicka`` must not become ``flicke``). Morphology stays with the Nordum word
list; source-language morphology is normalized by rules.

Usage:
  python3 build-nordum-dict.py \
      --nordum-wordlist /path/to/nordum/build/assets/data/wordlist.txt \
      --nordum-dictionary /path/to/nordum/build/assets/data/dictionary.json \
      --additions data/nrd/words/core_additions.txt \
      --sources nb_NO.dic da_DK.dic sv_SE.dic \
      --out data/nrd/hunspell/nrd.dic \
      --out-core data/nrd/hunspell/nrd_core.dic \
      --report data/nrd/hunspell/build-report.txt
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

# Spec §3.5: `hv-` question words drop the silent `h`.
QUESTION_WORDS = {
    "hva",
    "hvad",
    "hvor",
    "hvem",
    "hvilken",
    "hvilket",
    "hvilke",
    "hvordan",
    "hvorfor",
    "hvornår",
    "hvorhen",
    "hvorvidt",
}

# Owner decision: the mandatory `ks` → `x` rule (spec §3.3.2 rule 2) applies
# to number words too, like any other word: `seks` → `sex`, `seksten` →
# `sexten`, `sekstende` → `sextende`. `data/nrd/rules/source_forms.txt` offers
# the normalized form as a suggestion for writers.

# Words with internal capitals or non-letter characters are kept verbatim (the
# case-preserving transformation only handles simple words).
SIMPLE_WORD = re.compile(r"^[A-ZÆØÅÄÖÉ][a-zæøåäöéü'’-]*$|^[a-zæøåäöéü'’-]+$")
ALLOWED = re.compile(r"^[A-Za-zÆØÅæøåÄÖäöÉéÜü'’-]+$")


def read_dic(path: Path) -> list[str]:
    """Words of a Hunspell `.dic` (skip the count line, strip flags)."""
    words: list[str] = []
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return words
    for line in text.splitlines()[1:]:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        word = line.split("\t")[0].split("/")[0].strip()
        if word:
            words.append(word)
    return words


def apply_c_rule(word: str) -> str:
    """Spec §3.3.2 rule 3: established `c` → `s` before e/i/y, else `k`.

    Words containing the `ch` digraph are left alone (the rule targets the
    established Scandinavian `c`; fresh loanwords keep their spelling).
    """
    if "ch" in word:
        return word
    out = []
    for i, ch in enumerate(word):
        if ch != "c":
            out.append(ch)
            continue
        following = word[i + 1] if i + 1 < len(word) else ""
        if following in "eiy":
            out.append("s")
        else:
            out.append("k")
    return "".join(out)


def apply_mandatory(word: str) -> str:
    """The mandatory Nordum orthography rules (spec §3.3.2)."""
    # Rule 1: ck → kk
    word = re.sub(r"ck", "kk", word, flags=re.IGNORECASE)
    # Rule 2: /ks/ → x (applies to number words too, owner decision)
    word = re.sub(r"ks", "x", word, flags=re.IGNORECASE)
    # Rule 4: ph → f
    word = re.sub(r"ph", "f", word, flags=re.IGNORECASE)
    # Rule 5: skj → sk
    word = re.sub(r"skj", "sk", word, flags=re.IGNORECASE)
    # Rule 6: ld → ll
    word = re.sub(r"ld", "ll", word, flags=re.IGNORECASE)
    # Rule 7: hv- question words → v-
    if word.lower() in QUESTION_WORDS and word[:1].lower() == "h":
        word = word[1:]
    # Rule 3: c → s/k
    return apply_c_rule(word)


def normalize_diphthongs(word: str) -> str:
    """Spec §3.6: ej/aj → ei, øj/øy → øi (normalized forms recommended)."""
    word = word.replace("ej", "ei").replace("aj", "ei")
    return word.replace("øj", "øi").replace("øy", "øi")


def primary_vowels(word: str) -> str:
    """Spec §3.1/§7.1: primary system uses æ/ø (ä/ö are accepted variants)."""
    return word.replace("ä", "æ").replace("ö", "ø").replace("Ä", "Æ").replace("Ö", "Ø")


def secondary_vowels(word: str) -> str:
    """The accepted Swedish/German vowel alternative of a primary form."""
    return word.replace("æ", "ä").replace("ø", "ö").replace("Æ", "Ä").replace("Ø", "Ö")


def variants(word: str) -> set[str]:
    """All accepted spellings derived from one source-language word."""
    if not ALLOWED.match(word) or len(word) > 40:
        return set()
    if not SIMPLE_WORD.match(word):
        return {word}
    out: set[str] = set()
    mandatory = apply_mandatory(word)
    normalized = normalize_diphthongs(mandatory)
    out.add(primary_vowels(normalized))
    # §7.3: the preserved (Danish) patterns stay accepted.
    out.add(primary_vowels(mandatory))
    # §7.1: both vowel systems are valid.
    for form in list(out):
        out.add(secondary_vowels(form))
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nordum-wordlist", type=Path, required=True)
    parser.add_argument("--nordum-dictionary", type=Path)
    parser.add_argument("--additions", type=Path)
    parser.add_argument("--sources", type=Path, nargs="*", default=[])
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--out-core", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()

    core: set[str] = set()
    for line in args.nordum_wordlist.read_text(encoding="utf-8").splitlines():
        line = line.split("#")[0]
        for word in line.split():
            core.add(word)
    if args.nordum_dictionary and args.nordum_dictionary.exists():
        entries = json.loads(args.nordum_dictionary.read_text(encoding="utf-8"))["entries"]
        for entry in entries.values():
            if entry.get("alternativeOf"):
                core.add(entry["nordum"])
    additions = 0
    if args.additions and args.additions.exists():
        for line in args.additions.read_text(encoding="utf-8").splitlines():
            line = line.split("#")[0]
            for word in line.split():
                core.add(word)
                additions += 1

    dictionary: set[str] = set(core)
    source_words = 0
    for source in args.sources:
        for word in read_dic(source):
            source_words += 1
            dictionary.update(variants(word))

    # Core: both vowel systems plus accepted source alternations (diphthongs).
    core_variants: set[str] = set()
    for word in core:
        core_variants.add(primary_vowels(word))
        core_variants.add(secondary_vowels(primary_vowels(word)))
        core_variants.add(primary_vowels(normalize_diphthongs(word)))
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
