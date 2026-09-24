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
import subprocess
import sys
import tempfile
from collections import Counter
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


VOWELS = "aeiouyæøåäö"


def nordum_infinitive(lemma: str) -> str | None:
    """The spec §4.2.3 infinitive of a converted source-language verb lemma:
    all regular Nordum infinitives end in ``-e``.

    ``-a`` infinitives (the Swedish pattern) become ``-e`` (``anfalla`` →
    ``anfalle``, ``bakka`` → ``bakke``); other vowel-final stems take ``-e``
    after the vowel, exactly like the authoritative ``bo → boe`` /
    ``få → fåe`` / ``stå → ståe`` entries; consonant-final lemmas have no
    derivable spec infinitive (``None``: the conservative conversion drops
    them). Irregular verbs (``ha``/``være``/``gå``, spec §4.2.1) are covered
    by the authoritative lexicon and never reach this function — callers
    must check the authoritative verb lemmas first.
    """
    if not lemma:
        return None
    if lemma.endswith("a"):
        return lemma[:-1] + "e"
    if lemma[-1] in VOWELS:
        return lemma if lemma.endswith("e") else lemma + "e"
    return None


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


# ---------------------------------------------------------------------------
# Source-dictionary parsing (the decompiled da/sv tagger exports)
# ---------------------------------------------------------------------------

REPO_ROOT = Path(__file__).resolve().parents[2]


def decompile(dict_path: Path, cache_dir: Path | None = None) -> Path:
    """`lt_morfologik.py dict_decompile` with a cache in the system temp
    directory (the decompiled sources are large build intermediates, never
    vendored data)."""
    cache_dir = cache_dir or (Path(tempfile.gettempdir()) / "nordum-tagger-cache")
    cache_dir.mkdir(parents=True, exist_ok=True)
    cache = cache_dir / f"{dict_path.stem}_decompiled.txt"
    if cache.exists() and cache.stat().st_mtime > dict_path.stat().st_mtime:
        return cache
    subprocess.run(
        [sys.executable, str(REPO_ROOT / "tools/morfologik/lt_morfologik.py"),
         "dict_decompile", "-i", str(dict_path), "-o", str(cache)],
        check=True,
    )
    return cache


def read_export(path: Path) -> list[tuple[str, str, str]]:
    """Triples of a decompiled tagger dictionary.

    ``dict_decompile`` writes ``lemma+wordform+tag`` (the stem first — it is
    the decoded stem of the stored ``wordform+annotation`` sequence), so the
    surface form is the *second* field and the lemma the first.
    """
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or "+" not in line:
            continue
        lemma, _, rest = line.partition("+")
        surface, _, tag = rest.rpartition("+")
        if surface and lemma and tag:
            out.append((surface, lemma, tag))
    return out


def convert_primary(word: str) -> str:
    """The primary Nordum spelling of a converted source-language word (spec
    §3.3.2 mandatory rules, §3.6 diphthong normalization, §7.1 primary vowels):
    the form the morphology analyses operate on."""
    return primary_vowels(normalize_diphthongs(apply_mandatory(word)))


# Words the strict speller conversion generates morphology for: simple
# lowercase letter words only (compounds, hyphens and capitals stay verbatim).
SIMPLE_GENERATION = re.compile(r"^[a-zæøåäöéü]+$")


# Swedish (SUC) -> Nordum
SV_TAG_MAP = {
    "AB": "adv",
    "PP": "prep",
    "KN": "kon",
    "IN": "int",
    "PB": "det",
    "HD": "det",
    "PS": "det",
    "PN": "pron",
    "PM": "pro",
}


# Danish sub-features Nordum keeps. Noun tags keep number+gender; the
# definite/indefinite feature of the source is preserved for nouns only when
# it survives the conversion (bes/ube stay, spec §4.3.2 keeps both).
PRON_TAG_MAP = {
    "pron:sin:nom": "pron:sin:nom",
    "pron:sin:akk": "pron:sin:akk",
    "pron:sin:gen": "pron:sin:gen",
    "pron:plu:nom": "pron:plu:nom",
    "pron:plu:akk": "pron:plu:akk",
    "pron:plu:gen": "pron:plu:gen",
}


def map_source_tag(tag: str) -> str:
    """Map one source tag to the Nordum tagset; '' = drop (not reliable).

    Danish `sub`/`adj` tags carry definiteness, number, gender and case;
    Nordum keeps definiteness, number and the simplified gender and drops
    case. Danish `ver` keeps voice=active only. Swedish SUC tags are mapped
    feature-wise.
    """
    if tag in ("adv", "pra", "kon", "int", "art", "ono"):
        return {"adv": "adv", "pra": "prep", "kon": "kon", "int": "int",
                "art": "det", "ono": "int"}[tag]
    if tag.startswith("pron"):
        return PRON_TAG_MAP.get(tag, "")
    if tag.startswith("sub:"):
        parts = tag.split(":")
        definiteness = parts[1] if len(parts) > 1 else ""
        number = parts[2] if len(parts) > 2 else ""
        gender = parts[3] if len(parts) > 3 else ""
        if definiteness not in ("ube", "bes") or number not in ("sin", "plu"):
            return ""
        nordum_gender = {"utr": "com", "neu": "neu"}.get(gender, "")
        if not nordum_gender:
            return ""
        return f"sub:{definiteness}:{number}:{nordum_gender}:nom"
    if tag.startswith("adj:"):
        parts = tag.split(":")
        # Danish: adj:<definiteness>:<number>:<gender>:<grade>;
        # bare 4-part tags (adj:<grade>) also occur.
        grade = parts[4] if len(parts) > 4 else parts[3]
        if grade not in ("pos", "kom", "sup"):
            return ""
        return f"adj:{grade}"
    if tag.startswith("ver:"):
        parts = tag.split(":")
        tense = parts[1] if len(parts) > 1 else ""
        voice = parts[2] if len(parts) > 2 else "akt"
        tense_map = {"inf": "ver:inf", "præ": "ver:præ", "dat": "ver:dat",
                     "imp": "ver:imp", "kor": "ver:kor", "lan": "ver:lan"}
        if voice != "akt":
            return ""
        return tense_map.get(tense, "")
    # Swedish
    if tag in SV_TAG_MAP:
        return SV_TAG_MAP[tag]
    if tag.startswith("NN:"):
        parts = tag.split(":")
        # NN:OF:SIN:NOM:UTR / NN:BF:PLU:GEN:NEU / NN:BF:SIN:MNOM:UTR ...
        state, number = parts[1], parts[2]
        case = parts[3]
        gender = parts[4] if len(parts) > 4 else ""
        if state not in ("OF", "BF") or number not in ("SIN", "PLU") or case not in ("NOM", "GEN"):
            return ""
        nordum_gender = {"UTR": "com", "NEU": "neu"}.get(gender, "")
        if not nordum_gender:
            return ""
        definiteness = "ube" if state == "OF" else "bes"
        nordum_case = "nom" if case == "NOM" else "gen"
        return f"sub:{definiteness}:{'sin' if number == 'SIN' else 'plu'}:{nordum_gender}:{nordum_case}"
    if tag.startswith("JJ:"):
        # JJ:PU (predicative/attributive positive), JJ:K (comparative),
        # JJ:S (superlative)
        form = tag.split(":")[1]
        if form in ("P", "PU", "PN", "BF"):
            return "adj:pos"
        if form == "K":
            return "adj:kom"
        if form == "S":
            return "adj:sup"
        return ""
    if tag.startswith("VB:"):
        parts = tag.split(":")
        tense = parts[1]
        voice = parts[2] if len(parts) > 2 else "AKT"
        if voice == "PF":
            return ""
        return {"INF": "ver:inf", "PRS": "ver:præ", "PRT": "ver:dat",
                "IMP": "ver:imp", "SUP": "ver:kor"}.get(tense, "")
    return ""


# ---------------------------------------------------------------------------
# Strict speller conversion (owner decision 2026-09-23)
# ---------------------------------------------------------------------------
#
# The speller dictionary is flat (no POS), so a converted source word may only
# enter it when its form is derivable by the spec §4 morphology from a Nordum
# lemma (or when the form itself is a base/lemma form). The owner's rule
# («be stricter on that, else people will never learn it»): the verb endings
# (§4.2, present -er / infinitive -e / past -ede / supine -et) and the noun
# endings (§4.3, plural -ar, definite -en/-et) must be the only ones accepted.
#
# The conversion therefore works on the *lemma inventory* the tagger build
# (build-nordum-tagger.py) already established from the da/sv tagger
# dictionaries, not on raw converted surface forms:
#
# - classified readings (da/sv tagger exports, same tag mapping) generate the
#   spec §4 paradigm of the converted lemma (verb: the normalized -e
#   infinitive + verb_forms; noun: the singular indefinite + noun_forms by the
#   simplified gender; adjective: adjective_forms). A surface form is kept
#   when some reading generates it, when it is a function-word form (adv,
#   prep, kon, int, det, pron, pro — carried verbatim), or when it is one of
#   the spec-shaped adjective grade forms (§4.4.1 suppletives like
#   større/størst keep the -ere/-est shape; the systematic Swedish -are/-ast
#   and definite -ste/-sta forms do not);
# - unclassified source words (the nb word list and the da/sv words without a
#   usable tag) are analysed by their endings against the lemma inventory: a
#   form whose only reading is a spec-violating inflection of a known lemma
#   (e.g. the Danish «biler» plural of «bil», whose spec plural is «bilar»)
#   is dropped; a form without any inflectional analysis is kept as a base
#   form (ambiguous forms are dropped — the owner prefers strict).

# Base/lemma/drop verdicts of the ending analysis.
BASE, DROPPED = 0, 1

# Ending table for the strict analysis of unclassified source words: suffix ->
# kind. Each known (stem, pos) reading either validates the surface as a
# generated spec §4 form (redundant: the paradigm already adds it) or makes it
# a violation. Valid readings also return BASE — the form is already in the
# accepted set.
SUFFIXES = {
    "ar": "present-plural",
    "er": "present-plural",
    "or": "plural",
    "ede": "past",
    "ade": "past",
    "a": "informal-past",
    "at": "supine",
    "et": "supine-definite",
    "ende": "participle",
    "ande": "participle",
    "en": "definite",
    "ene": "plural-definite",
    "erne": "plural-definite",
    "ena": "plural-definite",
    "ane": "plural-definite",
    "arna": "plural-definite",
    "erna": "plural-definite",
    "orna": "plural-definite",
}
PLURAL_DEFINITE = "plural-definite"

# Suffixes where a known noun lemma makes the surface a noun reading; valid
# when it equals the generated §4.3 form. Definite claims need the gender.
NOUN_CLAIM_SUFFIXES = {"er", "ar", "or", "a", "en", "et"}
NOUN_CLAIM_NEEDS_GENDER = {"en", "et", PLURAL_DEFINITE}
NOUN_VIOLATIONS = {
    "er": "noun_plural_er",
    "ar": "noun_plural_ar_mismatch",
    "or": "noun_plural_or",
    "a": "noun_grade_or_plural_a",
    "en": "noun_definite_en",
    "et": "noun_definite_et",
    PLURAL_DEFINITE: "noun_plural_definite",
}

# Suffixes where a known verb infinitive (stem + -e) makes the surface a verb
# reading; valid when it equals the generated §4.2 form.
VERB_CLAIM_SUFFIXES = {"er", "ar", "ede", "ade", "a", "at", "et",
                       "ende", "ande", "en"}
VERB_VIOLATIONS = {
    "ar": "verb_present_ar",
    "ade": "verb_past_ade",
    "at": "verb_supine_at",
    "ande": "verb_participle_ande",
    "ede": "verb_past_ede_mismatch",
    "et": "verb_supine_et_mismatch",
    "ende": "verb_participle_ende_mismatch",
    "a": "verb_infinitive_a",
    "en": "noun_definite_en",
    "er": "verb_present_er_mismatch",
}


def grade_shape_ok(surface: str, grade: str) -> bool:
    """§4.4.1 grade endings: a converted comparative/superlative is accepted
    when it keeps the spec shape (-ere/-est) or the sanctioned short shapes
    (større, mindre, bedst, mer, flest — the spec §4.4.1 suppletives), while
    the systematic Swedish forms (-are, -ast, -sta/-sta definites) do not."""
    if grade == "kom":
        if surface.endswith("ere"):
            return True
        if surface.endswith("are"):
            return False
        return surface.endswith("re") or surface.endswith("er")
    if surface.endswith("est"):
        return True
    if surface.endswith("ast"):
        return False
    return surface.endswith("st")


def converted_noun_stem(lemma: str) -> str | None:
    """The §4.3 stem of a converted noun lemma: the spec strips the final
    vowel of -e/-a stems (jente → jent-ar, flikka → flikk-ar); other
    vowel-final lemmas keep the unstripped stem (by → by-ar, by-en,
    by-arna — only the ending is replaced)."""
    if len(lemma) > 2 and lemma.endswith(("e", "a")):
        return lemma[:-1]
    return lemma


def build_strict_source_forms(
    sources: list[Path],
    da_export: Path | None,
    sv_export: Path | None,
    entries: list[dict],
    core_words: list[str],
    cache_dir: Path | None = None,
) -> dict:
    """The strict converted-source inventory for the speller dictionary.

    Returns a dict with:
    - ``accepted``: every accepted spelling (both vowel systems, §7) of the
      converted source words;
    - ``classified_keep``: converted surface -> whether the da/sv classified
      readings validate it;
    - ``noun_lemmas`` / ``adjective_lemmas``: the converted lemma inventories;
    - ``assert_*``: the generated spec-§4 role forms (for the permanent
      morphological regression checks);
    - ``stats``: per-category counters for the build report.
    """
    stats: Counter[str] = Counter()

    # --- authoritative lemma inventories (primary spellings) ----------------
    auth_verb_paradigms: dict[str, set[str]] = {}
    auth_verb_lemmas: set[str] = set()
    noun_lemmas: dict[str, str | None] = {}
    adjective_lemmas: set[str] = set()
    for entry in entries:
        nordum = entry.get("nordum") or ""
        if "_" in nordum or not SIMPLE_GENERATION.match(nordum):
            continue
        primary = convert_primary(nordum)
        pos = entry.get("pos")
        inflections = entry.get("inflections") or {}
        if pos == "verb":
            verb_lemma = convert_primary(inflections.get("infinitive") or nordum)
            auth_verb_lemmas.add(verb_lemma)
            forms, _counts = verb_forms(inflections)
            auth_verb_paradigms[verb_lemma] = {
                convert_primary(f) for f in forms.values()}
        elif pos == "noun":
            noun_lemmas[primary] = entry.get("gender")
        elif pos == "adjective":
            adjective_lemmas.add(primary)
    for word in core_words:
        if not SIMPLE_GENERATION.match(word) or "_" in word:
            continue
        noun_lemmas.setdefault(convert_primary(word), None)

    # --- converted readings from the da/sv tagger exports -------------------
    converted_readings: dict[str, set[tuple[str, str]]] = {}
    for lang, path in (("da", da_export), ("sv", sv_export)):
        if path is None:
            continue
        cache = decompile(path, cache_dir)
        for surface, lemma, tag in read_export(cache):
            nordum_tag = map_source_tag(tag)
            if not nordum_tag:
                continue
            cs = convert_primary(surface)
            cl = convert_primary(lemma)
            if not cs or not cl:
                continue
            if not SIMPLE_GENERATION.match(cs) or not SIMPLE_GENERATION.match(cl):
                continue
            converted_readings.setdefault(cs, set()).add((cl, nordum_tag))
            stats[f"{lang}_classified_forms"] += 1

    converted_verb_paradigms: dict[str, set[str]] = {}
    for _cs, readings in converted_readings.items():
        for cl, tag in readings:
            prefix = tag.split(":")[0]
            if prefix == "ver":
                if cl in auth_verb_lemmas:
                    continue
                ve = nordum_infinitive(cl)
                if ve is None or ve in auth_verb_paradigms:
                    continue
                if ve not in converted_verb_paradigms:
                    forms, _counts = verb_forms({"infinitive": ve})
                    converted_verb_paradigms[ve] = {
                        convert_primary(f) for f in forms.values()}
            elif prefix == "sub":
                parts = tag.split(":")
                gender = {"com": "common", "neu": "neuter"}.get(
                    parts[3] if len(parts) > 3 else "")
                if cl not in noun_lemmas or noun_lemmas[cl] is None:
                    noun_lemmas[cl] = gender
            elif prefix == "adj":
                adjective_lemmas.add(cl)

    # --- accepted set + classified surface verdicts --------------------------
    # Verdicts come first: a converted surface form is speller data only when
    # some source reading generates it, it is a base/lemma or function-word
    # form, or (unclassified) no violation-only analysis applies. A form whose
    # own tagger reading is a violation is never re-introduced by a paradigm
    # generated from another lemma (e.g. the def-plural reading of «husen»
    # loses against the def-singular the rare sv noun «husa» would generate).
    accepted: set[str] = set()
    assert_infinitives: set[str] = set()
    assert_presents: set[str] = set()
    assert_noun_plurals: set[str] = set()
    assert_noun_plurdefs: set[str] = set()

    def add_variants(form: str) -> None:
        accepted.update(lexicon_variants(form))

    classified_keep: dict[str, bool] = {}
    classified_keep: dict[str, bool] = {}
    for cs, readings in converted_readings.items():
        keep = False
        for cl, tag in readings:
            prefix = tag.split(":")[0]
            if prefix == "ver":
                ve = nordum_infinitive(cl)
                if ve in auth_verb_paradigms or cl in auth_verb_paradigms:
                    # the authoritative lexicon carries the paradigm
                    # (irregulars ha/være/gå, spec §4.2.1).
                    if cs in auth_verb_paradigms.get(
                            ve if ve in auth_verb_paradigms else cl, ()):
                        keep = True
                    continue
                if ve is None:
                    # no derivable spec infinitive: the converted lemma stays
                    # as a base form; the surface survives only as the lemma.
                    if cs == cl:
                        keep = True
                    continue
                if cs in converted_verb_paradigms.get(ve, ()):
                    keep = True
            elif prefix == "sub":
                gender = noun_lemmas.get(cl)
                stem = converted_noun_stem(cl)
                if gender in ("common", "neuter") and stem is not None:
                    forms, _counts = noun_forms(
                        {"singular": {"indefinite": stem}}, gender)
                    if cs in forms.values() or cs == cl:
                        keep = True
                elif cs == cl:
                    keep = True
            elif prefix == "adj":
                grade = tag.split(":")[1]
                forms = adjective_forms({}, cl)[0]
                if cs in forms.values():
                    keep = True
                elif grade in ("kom", "sup") and grade_shape_ok(cs, grade):
                    # §4.4.1 suppletives and §3.3.3 doubled-stem grade forms
                    # keep the spec shape (større, mindre, dummere, mer...).
                    add_variants(cs)
                    keep = True
            else:
                # function words and their forms are carried verbatim.
                add_variants(cs)
                keep = True
        classified_keep[cs] = keep

    dropped_classified = {cs for cs, ok in classified_keep.items() if not ok}

    # --- generated §4 paradigms of the converted lemmas ----------------------
    for ve, forms in converted_verb_paradigms.items():
        for form in forms:
            if form in dropped_classified:
                continue
            add_variants(form)
        assert_infinitives.add(ve)
        for form in forms:
            if form.endswith("er") and form not in dropped_classified:
                assert_presents.add(form)
    for cl, gender in list(noun_lemmas.items()):
        if gender is None and cl in auth_verb_paradigms:
            # a verb headword from the core list is never a noun base here
            # (its core spelling is added by the authoritative step anyway).
            continue
        add_variants(cl)  # the singular indefinite is the base form
        stem = converted_noun_stem(cl)
        if gender not in ("common", "neuter") or stem is None:
            continue
        forms, _counts = noun_forms({"singular": {"indefinite": stem}}, gender)
        for form in forms.values():
            if form in dropped_classified:
                continue
            add_variants(form)
            if form == forms["pluralIndefinite"]:
                assert_noun_plurals.add(form)
            elif form == forms["pluralDefinite"]:
                assert_noun_plurdefs.add(form)
    for adjective in adjective_lemmas:
        forms, _counts = adjective_forms({}, adjective)
        for form in forms.values():
            add_variants(form)

    # --- strict ending analysis of the unclassified source words ------------
    verb_paradigms = {**auth_verb_paradigms, **converted_verb_paradigms}
    verb_infinitives = set(verb_paradigms)
    memo: dict[str, tuple[int, str]] = {}

    def analyze(surface: str) -> tuple[int, str]:
        """The strict verdict of an unclassified converted surface form:
        (BASE, "") when no known-lemma reading inflects it (kept as a base
        form) and (DROPPED, category) when its only readings violate the spec
        §4 morphology. Valid readings return (BASE, "") too — a form a known
        lemma's paradigm generates is already in the accepted set."""
        if surface in memo:
            return memo[surface]
        memo[surface] = (BASE, "")  # recursion guard
        valid = False
        category = ""

        def violation(kind: str) -> None:
            nonlocal category
            if not category:
                category = kind

        def noun_paradigm(nl: str, full: bool) -> set[str]:
            gender = noun_lemmas[nl]
            stem = converted_noun_stem(nl)
            if not full or gender not in ("common", "neuter") or stem is None:
                return {nl}
            forms, _counts = noun_forms(
                {"singular": {"indefinite": stem}}, gender)
            return set(forms.values()) | {nl}

        # genitive and passive -s of an existing word: never a spec form.
        if surface.endswith("s") and len(surface) > 3:
            stem_s = surface[:-1]
            if stem_s in verb_paradigms:
                violation("verb_passive_s")
            elif (stem_s in accepted or classified_keep.get(stem_s)
                    or stem_s in noun_lemmas or stem_s in adjective_lemmas):
                violation("genitive_s")
            else:
                state, _cat = analyze(stem_s)
                if state == DROPPED:
                    violation("genitive_s")
        for suffix, kind in SUFFIXES.items():
            if not surface.endswith(suffix):
                continue
            x = surface[:-len(suffix)]
            if kind == PLURAL_DEFINITE:
                p = x
                candidates = {p}
                if p.endswith(("e", "a")):
                    candidates.add(p[:-1])
                if p.endswith("er"):
                    candidates.add(p[:-2] + "e")
                if p.endswith("ar"):
                    candidates.add(p[:-2] + "a")
                for candidate in candidates:
                    if candidate not in noun_lemmas:
                        continue
                    if noun_lemmas[candidate] not in ("common", "neuter"):
                        continue
                    if surface in noun_paradigm(candidate, True):
                        valid = True
                    else:
                        violation(NOUN_VIOLATIONS[kind])
                continue
            for stem in (x, x + "e", x + "a"):
                stem = convert_primary(stem)
                if stem in verb_paradigms and suffix in VERB_CLAIM_SUFFIXES:
                    if surface in verb_paradigms[stem]:
                        valid = True
                    else:
                        violation(VERB_VIOLATIONS[suffix])
                if stem in noun_lemmas and suffix in NOUN_CLAIM_SUFFIXES:
                    gender = noun_lemmas[stem]
                    if suffix in NOUN_CLAIM_NEEDS_GENDER and gender not in (
                            "common", "neuter"):
                        continue
                    if surface in noun_paradigm(stem, True):
                        valid = True
                    else:
                        violation(NOUN_VIOLATIONS[suffix])
        verdict = DROPPED if (category and not valid) else BASE
        memo[surface] = (verdict, category)
        return memo[surface]

    # --- the raw source word lists -------------------------------------------
    for source in sources:
        for word in read_dic(source):
            if not ALLOWED.match(word) or len(word) > 40:
                continue
            stats["source_words_read"] += 1
            if not SIMPLE_GENERATION.match(word):
                # capitals, hyphens etc. are carried verbatim (no morphology).
                accepted.update(variants(word))
                stats["source_non_simple_kept"] += 1
                continue
            cp = convert_primary(word)
            if cp in converted_readings:
                if classified_keep.get(cp):
                    accepted.update(variants(word))
                    stats["classified_kept"] += 1
                else:
                    stats["classified_dropped"] += 1
                continue
            if cp in accepted:
                accepted.update(variants(word))
                stats["source_redundant_kept"] += 1
                continue
            if not cp or not SIMPLE_GENERATION.match(cp):
                accepted.update(variants(word))
                stats["source_non_simple_kept"] += 1
                continue
            state, category = analyze(cp)
            if state == DROPPED:
                stats[f"dropped_{category}"] += 1
            else:
                accepted.update(variants(word))
                stats["source_base_kept"] += 1

    return {
        "accepted": accepted,
        "classified_keep": classified_keep,
        "verb_infinitives": verb_infinitives,
        "noun_lemmas": noun_lemmas,
        "adjective_lemmas": adjective_lemmas,
        "assert_infinitives": assert_infinitives,
        "assert_presents": assert_presents,
        "assert_noun_plurals": assert_noun_plurals,
        "assert_noun_plurdefs": assert_noun_plurdefs,
        "auth_verb_paradigms": auth_verb_paradigms,
        "stats": stats,
    }
