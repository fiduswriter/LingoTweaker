#!/usr/bin/env python3
"""Shared Nordum orthography + morphology library.

Used by ``build-nordum-dict.py`` (speller dictionary), ``build-nordum-tagger.py``
(POS tagger dictionary) and ``check_pairs.py`` (rule-data verification), so all
tools apply exactly the same conversion and inflection rules.

Orthography: NORDUM_LANGUAGE_SPECIFICATION.md §3.3.2 (special spelling rules),
§3.6 (diphthong normalization), §7.1 (vowel systems).

Morphology: spec §4 — verbs (§4.2: present ``-er``, infinitive ``-e``, past
``-ede`` with the accepted informal ``-a``, supine/participle ``-et``,
present participle ``-ende``, stem imperative), nouns (§4.3: plural ``-ar``,
definite ``-en``/``-et``, definite plural ``plural + na``, simplified
common/neuter gender) and adjectives (§4.4: neuter ``-t``,
plural/definite ``-e``, comparative ``-ere``, superlative ``-est``).
Consonants are never inserted or removed (§3.3.3: doubling is kept as
written, no linking consonants).
"""

from __future__ import annotations

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


def read_word_file(path: Path) -> list[str]:
    """Whitespace-separated words of a `#`-commented word list."""
    words: list[str] = []
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return words
    for line in text.splitlines():
        line = line.split("#")[0]
        for word in line.split():
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


def lexicon_variants(word: str) -> set[str]:
    """Accepted spellings of an already-Nordum word: both vowel systems plus
    the preserved diphthong patterns (spec §3.6, §7.1)."""
    out = set()
    for form in (word, normalize_diphthongs(word)):
        base = primary_vowels(form)
        out.add(base)
        out.add(secondary_vowels(base))
    return out


def load_dictionary(path: Path) -> list[dict]:
    """Entries of the authoritative Nordum ``dictionary.json``."""
    data = json.loads(path.read_text(encoding="utf-8"))
    return list(data["entries"].values())


# ---------------------------------------------------------------------------
# Morphology (spec §4)
# ---------------------------------------------------------------------------


def verb_stem(infinitive: str) -> str:
    """The verb stem: drop the infinitive ``-e`` (spec §4.2.3)."""
    if len(infinitive) > 2 and infinitive.endswith("e"):
        return infinitive[:-1]
    return infinitive


def verb_forms(inflections: dict) -> tuple[dict[str, str], dict[str, int]]:
    """The complete verb paradigm (spec §4.2). The authoritative
    ``inflections`` from ``dictionary.json`` (including the irregular verbs
    ``gå``/``være``/``ha``) win; missing roles are generated systematically:
    present ``-er``, past ``-ede`` plus the accepted informal ``-a`` form,
    supine/past participle ``-et``, present participle ``-ende`` and the stem
    imperative.

    Returns (role -> form) and per-rule generation counts keyed by
    ``role:provided`` / ``role:generated``.
    """
    infinitive = inflections.get("infinitive") or ""
    if not infinitive:
        # No reliable stem: without explicit inflections nothing is
        # generated (the speller never guesses morphology).
        return {}, {}
    stem = verb_stem(infinitive)
    past = inflections.get("past") or f"{stem}ede"
    forms: dict[str, str] = {}
    counts: dict[str, int] = {}
    plan = {
        "infinitive": inflections.get("infinitive") or infinitive,
        "present": inflections.get("present") or f"{stem}er",
        "past": past,
        "supine": inflections.get("supine") or f"{stem}et",
        "pastParticiple": inflections.get("pastParticiple") or f"{stem}et",
        "presentParticiple": inflections.get("presentParticiple") or f"{stem}ende",
        "imperative": inflections.get("imperative") or stem,
    }
    for role, form in plan.items():
        if not form or form in forms.values():
            continue
        forms[role] = form
        kind = "provided" if inflections.get(role) else "generated"
        counts[f"{role}:{kind}"] = counts.get(f"{role}:{kind}", 0) + 1
    # §4.2.2: the shorter `-a` past (arbeida) is an accepted secondary form of
    # regular `-ede` verbs: the `-a` replaces the `-ede` ending.
    if past.endswith("ede"):
        informal = f"{past[:-3]}a"
        if informal not in forms.values():
            forms["pastInformal"] = informal
            counts["pastInformal:generated"] = 1
    return forms, counts


def noun_forms(inflections: dict, gender: str | None) -> tuple[dict[str, str], dict[str, int]]:
    """The complete noun paradigm (spec §4.3): definite ``-en``/``-et`` by the
    simplified gender, plural ``-ar``, definite plural ``plural + na``.
    Provided ``inflections`` win; missing roles are generated."""
    singular = inflections.get("singular") or {}
    plural = inflections.get("plural") or {}
    sing_indef = singular.get("indefinite") or ""
    stem = sing_indef[:-1] if len(sing_indef) > 2 and sing_indef.endswith("e") else sing_indef
    sing_def = singular.get("definite") or (f"{stem}en" if gender == "common" else f"{stem}et")
    plur_indef = (inflections.get("plural") or {}).get("indefinite") or f"{stem}ar"
    plur_def = (inflections.get("plural") or {}).get("definite") or f"{plur_indef}na"
    plan = {
        "singularDefinite": sing_def,
        "pluralIndefinite": plur_indef,
        "pluralDefinite": plur_def,
    }
    forms: dict[str, str] = {}
    counts: dict[str, int] = {}
    for role, form in plan.items():
        if not form or form in forms.values():
            continue
        forms[role] = form
        provided = (role == "singularDefinite" and singular.get("definite")) or (
            role == "pluralIndefinite" and (inflections.get("plural") or {}).get("indefinite")
        ) or (role == "pluralDefinite" and (inflections.get("plural") or {}).get("definite"))
        kind = "provided" if provided else "generated"
        counts[f"{role}:{kind}"] = counts.get(f"{role}:{kind}", 0) + 1
    return forms, counts


def adjective_forms(inflections: dict, nordum: str) -> tuple[dict[str, str], dict[str, int]]:
    """The adjective paradigm (spec §4.4): neuter ``-t``, plural/definite
    ``-e``, comparative ``-ere``, superlative ``-est``; provided inflections
    win (irregular pairs like ``større``/``mindre`` come from the data)."""
    positive = inflections.get("positive") or {}
    neuter = positive.get("neuter") or f"{nordum}t"
    plural = positive.get("plural") or f"{nordum}e"
    definite = positive.get("definite") or f"{nordum}e"
    comparative = inflections.get("comparative") or f"{nordum}ere"
    superlative = inflections.get("superlative") or f"{nordum}est"
    forms = {
        "positiveCommon": nordum,
        "positiveNeuter": neuter,
        "positivePlural": plural,
        "positiveDefinite": definite,
        "comparative": comparative,
        "superlative": superlative,
    }
    counts = {role: 1 for role in forms}
    for role, provided in (
        ("positiveNeuter", positive.get("neuter")),
        ("positivePlural", positive.get("plural")),
        ("positiveDefinite", positive.get("definite")),
        ("comparative", inflections.get("comparative")),
        ("superlative", inflections.get("superlative")),
    ):
        if not provided:
            counts[f"{role}:generated"] = 1
    return forms, counts


def entry_forms(entry: dict) -> tuple[dict[str, str], dict[str, int]]:
    """All Nordum forms of one ``dictionary.json`` entry (the headword itself
    is not repeated) with per-rule generation counts."""
    pos = entry.get("pos")
    inflections = entry.get("inflections") or {}
    nordum = entry["nordum"]
    if pos == "verb":
        return verb_forms(inflections)
    if pos == "noun":
        return noun_forms(inflections, entry.get("gender"))
    if pos == "adjective":
        return adjective_forms(inflections, nordum)
    return {}, {}
