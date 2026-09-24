#!/usr/bin/env python3
"""Build the Nordum POS tagger dictionary (`nrd.dict` + `nrd.info`).

The Nordum word list (`wordlist.txt` + `dictionary.json`) is authoritative:
its entries carry POS, gender and inflections, and their forms (generated per
NORDUM_LANGUAGE_SPECIFICATION.md §4 by ``nordum_convert``) are tagged from
that data. Source-language tagger dictionaries (Danish Stavekontrolden,
Swedish SUC-style) are converted with the Nordum orthographic rules
(`nordum_convert`) and merged only where they pass the conservative
confidence criteria:

- **cognate gate** (spec §5.2 majority principle): the converted primary
  form must occur in at least two of the three source-language speller
  dictionaries (nb/da/sv) — a word that only one source language has is not
  accepted as Nordum vocabulary by the majority rule, so converting its tags
  would manufacture readings for words that are merely source-language
  spellings;
- **cross-source agreement**: when both converted sources tag the same
  surface form, the readings must agree (same lemma + tag set); a form the
  sources read differently is dropped entirely (it stays speller data);
- a converted triple is dropped when the same surface form already carries a
  different part of speech in the authoritative lexicon;
- the conversion must be conservative (simple lowercase letter words only).

The authoritative Nordum entries always win: they are never gated.

This conservative selection keeps the dictionary small and precise; recall is
added later by growing the authoritative word list, not by loosening the
conversion.

Usage:
  python3 build-nordum-tagger.py \
      --nordum-dictionary /path/to/nordum/build/assets/data/dictionary.json \
      --da-export data/da/dictionaries/danish.dict \
      --sv-export data/sv/dictionaries/swedish.dict \
      --gate-sources data/no/hunspell/nb_NO.dic data/da/hunspell/da_DK.dic \
          data/sv/hunspell/sv_SE.dic \
      --out data/nrd/dictionaries/nrd.txt \
      --report data/nrd/dictionaries/build-report.txt

  python3 build-nordum-tagger.py --self-test

The ``--da-export``/``--sv-export`` arguments point at the tagger
dictionaries; they are decompiled with ``lt_morfologik.py dict_decompile``
(cached under the system temp directory, not in ``data/``). The plain-text
output (``word<TAB>lemma<TAB>tag`` triples) is compiled with
``lt_morfologik.py pos`` (see tools/morfologik/README.md):

  python3 tools/morfologik/lt_morfologik.py pos \
      -i data/nrd/dictionaries/nrd.txt \
      --info data/nrd/dictionaries/nrd.info \
      -o data/nrd/dictionaries/nrd.dict
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import nordum_convert as nc

REPO_ROOT = Path(__file__).resolve().parents[2]

# The cognate gate (spec §5.2): per-source variant sets of the source-language
# speller dictionaries, filled in by ``main`` (or the self-test).
COGNATE_SETS: dict[str, set[str]] = {}
# Nordum vocabulary is what a majority of the source languages share.
MIN_COGNATE_SOURCES = 2

# The Nordum tagset (data/nrd/words/tagset.txt): Stavekontrolden-style
# part-of-speech codes with colon-separated features. Nordum keeps only
# gender (simplified, spec §4.3.3) and number — the Danish definiteness and
# case features are dropped, the Swedish SUC features are mapped.

# Danish (Stavekontrolden) -> Nordum
DA_TAG_MAP = {
    "adv": "adv",
    "pra": "prep",
    "kon": "kon",
    "int": "int",
    "art": "det",
    "ono": "int",
}

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

VERB_ROLES_TO_TAGS = {
    "infinitive": "ver:inf",
    "present": "ver:præ",
    "past": "ver:dat",
    "supine": "ver:kor",
    "pastParticiple": "ver:kor",
    "pastInformal": "ver:dat",
    "presentParticiple": "ver:lan",
    "imperative": "ver:imp",
}

NOUN_ROLE_TO_TAG = {
    "singularDefinite": "sub:bes:sin",
    "pluralIndefinite": "sub:ube:plu",
    "pluralDefinite": "sub:bes:plu",
}

ADJ_ROLE_TO_TAG = {
    "positiveNeuter": "adj:pos",
    "positivePlural": "adj:pos",
    "positiveDefinite": "adj:pos",
    "comparative": "adj:kom",
    "superlative": "adj:sup",
}

# The Nordum entry POS -> tag prefix for the headword itself.
POS_TO_TAG = {
    "noun": "sub:ube:sin",
    "verb": "ver:inf",
    "adjective": "adj:pos",
    "adverb": "adv",
    "pronoun": "pron",
    "preposition": "prep",
    "conjunction": "kon",
    "interjection": "int",
    "numeral": "num",
}

# Words the conversion must never tag. Function words where the Nordum
# reading differs from the source reading (spec §3.3.4, §4.2.4, §4.6):
# converting them would tag source spellings with readings that do not hold
# in Nordum (e.g. Danish "at" is the infinitive marker/conjunction, but Nordum
# writes «att»/«å», spec §4.2.4). The authoritative entries cover the Nordum
# forms; these entries only keep the converter from tagging the source forms.
# The list is deliberately explicit rather than a heuristic.
CONVERSION_BLOCKLIST = frozenset(
    {
        # infinitive marker / conjunction: Nordum distinguishes å / att
        # (spec §4.2.4) — the source forms carry the wrong reading.
        "at", "att", "å", "om", "att",
        # prepositions where Nordum follows the Bokmål pattern or the
        # documented exceptions (spec §3.3.4) — tagged authoritatively.
        "til", "till", "fra", "med", "av", "for", "ved", "mot", "før",
        "siden", "uten", "innen", "i", "på", "over", "under",
        # pronouns and possessives: the Nordum system (spec §4.6) differs
        # from all three sources; the authoritative entries cover them.
        "jeg", "mig", "dig", "sig", "du", "han", "hun", "ham", "henne",
        "hende", "hendes", "vi", "oss", "de", "dem", "der", "deres",
        "deras", "dets", "dens", "din", "dina", "ditt", "min", "mina",
        "mitt", "ni", "jer", "er", "ert", "era", "vår", "våra", "vårt",
        "hans", "hennas", "mei", "dei", "sei", "hun",
        # question words: Nordum uses the v- pattern with lexical selections
        # (spec §3.5); the converted source forms would get the wrong lemma.
        "vad", "vem", "vilken", "vilket", "vilke", "vordan", "var", "när",
        "når", "ven", "vornår", "vorfor", "varfør", "vort", "vore",
        # other function words with divergent Nordum forms.
        "og", "ikke", "inte", "ja", "nei", "nej", "som", "hvis", "da",
        "mens", "fordi", "eftersom", "men", "eller", "ellers",
        "et", "en", "den", "det",
    }
)


def self_test() -> int:
    failures = 0
    # Danish conversion applies the orthographic rules to form and lemma.
    triples = convert_source_triple("arbejde", "arbejde", "ver:inf:akt", "da")
    if triples != [("arbeide", "arbeide", "ver:inf")]:
        failures += 1
        print(f"FAIL da verb conversion: {triples}")
    triples = convert_source_triple("arbetar", "arbeta", "VB:PRS:AKT", "sv")
    if triples != [("arbetar", "arbeta", "ver:præ")]:
        failures += 1
        print(f"FAIL sv present conversion: {triples}")
    triples = convert_source_triple("flickorna", "flicka", "NN:BF:PLU:GEN:NEU", "sv")
    if triples != [("flikkorna", "flikka", "sub:bes:plu:neu:gen")]:
        failures += 1
        print(f"FAIL sv noun conversion: {triples}")
    # A non-simple word (compounds with digits etc.) is dropped.
    if convert_source_triple("e-post", "e-post", "sub:ube:sin:utr:nom", "da") != []:
        failures += 1
        print("FAIL hyphenated form must be dropped")
    # Tag mapping.
    if map_source_tag("sub:ube:sin:utr:nom") != "sub:ube:sin:com:nom":
        failures += 1
        print("FAIL da noun tag map")
    if map_source_tag("NN:BF:SIN:NOM:UTR") != "sub:bes:sin:com:nom":
        failures += 1
        print("FAIL sv noun mapping")
    # Danish adjective tags carry the grade in the fifth column.
    if map_source_tag("adj:ube:sin:utr:pos") != "adj:pos":
        failures += 1
        print("FAIL da adjective tag map")
    if map_source_tag("adj:bes:plu:neu:kom") != "adj:kom":
        failures += 1
        print("FAIL da adjective comparative map")
    if map_source_tag("ver:dat:pas"):
        failures += 1
        print("FAIL passive verb tags must be dropped")
    if map_source_tag("VB:PRT:PF"):
        failures += 1
        print("FAIL passively-used Swedish verb tags must be dropped")
    if map_source_tag("NN:BF:SIN:NOM:NON"):
        failures += 1
        print("FAIL gender-unspecified Swedish noun tags must be dropped")
    # The cognate gate: a form only one source language has is not Nordum.
    global COGNATE_SETS
    old_sets = COGNATE_SETS
    COGNATE_SETS = {"nb": {"hus"}, "da": {"arbeide", "hus"},
                    "sv": {"arbeide", "hus"}}
    if cognate_sources("hus") != 3 or cognate_sources("arbeide") != 2:
        failures += 1
        print("FAIL cognate gate counts")
    if cognate_sources("abalienation") != 0:
        failures += 1
        print("FAIL cognate gate: unknown word is in no source")
    # Cross-source agreement: disagreeing readings drop the whole form.
    kept = cross_source_consistent(
        "hus", {"da": {("hus", "sub:ube:sin:com:nom")},
                "sv": {("hus", "sub:ube:sin:com:nom")}})
    if kept != {("hus", "hus", "sub:ube:sin:com:nom")}:
        failures += 1
        print(f"FAIL agreeing readings kept: {kept}")
    if cross_source_consistent(
            "ren", {"da": {("ren", "adj:pos")},
                    "sv": {("ren", "sub:ube:sin:com:nom")}}) != set():
        failures += 1
        print("FAIL disagreeing readings must drop the form")
    # The authoritative lexicon wins: a conversion that disagrees with an
    # authoritative reading of the same form is dropped.
    if not authoritative_conflict(
            "har", "have", "ver:præ", {"har": {("ha", "ver")}}):
        failures += 1
        print("FAIL authoritative lemma conflict must drop the triple")
    if authoritative_conflict(
            "arbeider", "arbeide", "ver:præ",
            {"arbeider": {("arbeide", "ver"), ("arbeide", "sub")}}):
        failures += 1
        print("FAIL agreeing authoritative reading must be kept")
    COGNATE_SETS = old_sets
    if failures:
        print(f"{failures} self-test failure(s)")
        return 1
    print("self-test ok")
    return 0


def cognate_sources(form: str) -> int:
    """How many source-language spellers accept the form (spec §5.2: Nordum
    vocabulary is what a majority of the source languages share)."""
    return sum(1 for words in COGNATE_SETS.values() if form in words)


def cross_source_consistent(
    form: str,
    readings: dict[str, set[tuple[str, str]]],
) -> set[tuple[str, str, str]]:
    """The converted triples of one form after the confidence criteria.

    ``readings`` maps each contributing source language to its (lemma, tag)
    set for the form. The form is kept when the cognate gate accepts it and
    the contributing languages agree on the readings; otherwise the form is
    dropped entirely (it stays speller data).
    """
    if not readings or cognate_sources(form) < MIN_COGNATE_SOURCES:
        return set()
    languages = sorted(readings)
    reference = readings[languages[0]]
    for lang in languages[1:]:
        if readings[lang] != reference:
            return set()
    return {(form, lemma, tag) for lemma, tag in reference}


def authoritative_conflict(
    form: str, lemma: str, tag: str, form_readings: dict[str, set[tuple[str, str]]]
) -> bool:
    """A converted triple that disagrees with an authoritative reading of the
    same form (different lemma or different part of speech) is dropped: the
    Nordum word list wins (spec §5)."""
    known = form_readings.get(form)
    if not known:
        return False
    pos = tag.split(":")[0]
    return (lemma, pos) not in known


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


SIMPLE_TAGGER_WORD = __import__("re").compile(r"^[a-zæøå]+$")


def conservative(form: str, converted: str) -> bool:
    """The tagger only carries simple lowercase letter words: no hyphens,
    apostrophes, digits or internal capitals."""
    return bool(SIMPLE_TAGGER_WORD.match(converted))


def convert_source_triple(form: str, lemma: str, tag: str, lang: str) -> list[tuple[str, str, str]]:
    """One source triple -> the Nordum triples it conservatively converts to."""
    nordum_tag = map_source_tag(tag)
    if not nordum_tag:
        return []
    if form.lower() in CONVERSION_BLOCKLIST or lemma.lower() in CONVERSION_BLOCKLIST:
        return []
    form_variants = nc.variants(form)
    lemma_variants = nc.variants(lemma)
    if not form_variants or not lemma_variants:
        return []
    # Tag only the primary (æ/ø) spelling; the lemma is the primary
    # spelling of the converted lemma (variants stay speller data).
    converted_form = nc.primary_vowels(nc.normalize_diphthongs(
        nc.apply_mandatory(form)))
    converted_lemma = nc.primary_vowels(nc.normalize_diphthongs(
        nc.apply_mandatory(lemma)))
    if converted_form not in form_variants or converted_lemma not in lemma_variants:
        return []
    if not conservative(form, converted_form) or not conservative(lemma, converted_lemma):
        return []
    return [(converted_form, converted_lemma, nordum_tag)]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nordum-dictionary", type=Path)
    parser.add_argument("--da-export", type=Path)
    parser.add_argument("--sv-export", type=Path)
    parser.add_argument(
        "--gate-sources", type=Path, nargs="*", default=[
            REPO_ROOT / "data/no/hunspell/nb_NO.dic",
            REPO_ROOT / "data/da/hunspell/da_DK.dic",
            REPO_ROOT / "data/sv/hunspell/sv_SE.dic",
        ],
        help="source-language Hunspell dictionaries for the cognate gate "
             "(spec §5.2 majority); order: nb, da, sv")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if not args.out:
        parser.error("--out is required")

    entries: list[dict] = nc.load_dictionary(args.nordum_dictionary)

    # 1. Authoritative triples from the Nordum word list. These are never
    # gated: the word list is the definition of Nordum (spec §5).
    triples: set[tuple[str, str, str]] = set()
    form_readings: dict[str, set[tuple[str, str]]] = {}
    stats: Counter[str] = Counter()
    for entry in entries:
        pos = entry.get("pos")
        headword = entry["nordum"]
        if "_" in headword:
            stats["skipped_multiword"] += 1
            continue
        tag = POS_TO_TAG.get(pos)
        if not tag:
            stats[f"skipped_pos_{pos}"] += 1
            continue
        if pos == "noun":
            gender = {"common": "com", "neuter": "neu"}.get(entry.get("gender"))
            if not gender:
                stats["noun_without_gender"] += 1
                continue
            tag = f"sub:ube:sin:{gender}:nom"
            triples.add((headword, headword, tag))
            stats["nordum_noun"] += 1
            forms, _counts = nc.noun_forms(entry.get("inflections") or {}, entry.get("gender"))
            for role, form in forms.items():
                if role == "singularIndefinite":
                    continue
                base_tag = NOUN_ROLE_TO_TAG[role]
                triples.add((form, headword, f"{base_tag}:{gender}:nom"))
                stats["nordum_noun_forms"] += 1
        elif pos == "verb":
            triples.add((headword, headword, "ver:inf"))
            stats["nordum_verb"] += 1
            forms, _counts = nc.verb_forms(entry.get("inflections") or {})
            for role, form in forms.items():
                if role == "infinitive":
                    continue
                triples.add((form, headword, VERB_ROLES_TO_TAGS[role]))
                stats["nordum_verb_forms"] += 1
        elif pos == "adjective":
            triples.add((headword, headword, "adj:pos"))
            stats["nordum_adj"] += 1
            forms, _counts = nc.adjective_forms(entry.get("inflections") or {}, headword)
            for role in ("positiveNeuter", "positivePlural", "positiveDefinite",
                         "comparative", "superlative"):
                form = forms.get(role)
                if form:
                    triples.add((form, headword, ADJ_ROLE_TO_TAG[role]))
                    stats["nordum_adj_forms"] += 1
        else:
            tag = POS_TO_TAG[pos]
            triples.add((headword, headword, tag))
            stats[f"nordum_{pos}"] += 1
    for form, lemma, tag in triples:
        form_readings.setdefault(form, set()).add((lemma, tag.split(":")[0]))

    # 2. The cognate-gate source sets: per-source accepted spellings.
    for name, path in zip(("nb", "da", "sv"), args.gate_sources):
        words: set[str] = set()
        for word in nc.read_dic(path):
            for variant in nc.variants(word):
                if conservative(word, variant):
                    words.add(variant)
        COGNATE_SETS[name] = words
        stats[f"gate_words_{name}"] = len(words)

    # 3. Converted source triples, grouped per form and language.
    per_form: dict[str, dict[str, set[tuple[str, str]]]] = {}
    for lang, flag in (("da", "--da-export"), ("sv", "--sv-export")):
        source = getattr(args, f"{lang}_export")
        if not source:
            continue
        cache = decompile(Path(source))
        stats[f"{lang}_source_triples"] = 0
        for surface, lemma, tag in read_export(cache):
            stats[f"{lang}_source_triples"] += 1
            for form, conv_lemma, conv_tag in convert_source_triple(
                    surface, lemma, tag, lang):
                per_form.setdefault(form, {}).setdefault(lang, set()).add(
                    (conv_lemma, conv_tag))
                stats[f"{lang}_converting"] += 1

    # 4. Confidence criteria: the cognate gate (≥2 source spellers, spec
    # §5.2) plus cross-source agreement, then the authoritative lexicon wins.
    for form, langs in per_form.items():
        converted = cross_source_consistent(form, langs)
        if not langs:
            continue
        if cognate_sources(form) < MIN_COGNATE_SOURCES:
            stats["dropped_cognate_gate"] += 1
        elif len(langs) > 1 and any(
                readings != langs[sorted(langs)[0]] for readings in langs.values()):
            stats["dropped_cross_source_conflict"] += 1
        else:
            kept = 0
            for _form, lemma, tag in converted:
                if authoritative_conflict(form, lemma, tag, form_readings):
                    stats["dropped_authoritative_conflict"] += 1
                    continue
                triples.add((form, lemma, tag))
                kept += 1
            stats["converted_kept"] += kept
    COGNATE_SETS.clear()

    args.out.parent.mkdir(parents=True, exist_ok=True)
    lines = [f"{form}\t{lemma}\t{tag}" for (form, lemma, tag) in sorted(triples)]
    args.out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    if args.report:
        stats_lines = "\n".join(f"  {key}: {value}" for key, value in sorted(stats.items()))
        args.report.write_text(
            "Nordum tagger dictionary build\n"
            f"authoritative entries (dictionary.json): {len(entries)}\n"
            f"tagger triples written: {len(triples)}\n"
            f"confidence criteria: cognate gate >= {MIN_COGNATE_SOURCES} of "
            f"{len(('nb', 'da', 'sv'))} source spellers, cross-source "
            "agreement, authoritative lexicon wins\n"
            f"{stats_lines}\n",
            encoding="utf-8",
        )
    print(f"wrote {args.out} ({len(triples)} triples)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
