#!/usr/bin/env python3
"""Build the Norwegian (Bokmål) POS tagger dictionary (`no_pos.dict` inputs).

The Ordbøkene open data (Bokmålsordboka; Ordbanken/Nasjonalbiblioteket +
Språkbanken, CC BY 4.0) is authoritative: its lemma table carries POS, word
class and, per entry, the inflected forms laid out in the slot order of the
shared inflection templates (`inflection_tags.json`):

  https://ord.uib.no/bm/fil/lemma_expanded.json
  https://ord.uib.no/bm/fil/inflection_tags.json
  https://ord.uib.no/bm/fil/word_class.json

Each entry is [word, lemma_id, word_class, sub_word_class, [paradigm codes],
[[form slots]]]. The exported form lists drop the *leading lemma slot* of
the article template (nouns: Sing/Ind; verbs: the first Inf slot; adjectives
and adjectival adverbs/determiners: the positive masc/fem form) and export
the remaining slots in template order — the lemma slot comes through EMPTY
and is reconstructed from the entry word. Verified against the article JSON
(`ord.uib.no/bm/article/<id>.json`), whose `paradigm_info` slots carry
explicit grammatical tags, for every locally extracted article.

Emission (see data/no/words/tagset.txt for the tagset): every non-empty form
of an accepted word class becomes a `form<TAB>lemma<TAB>tag` triple; nouns
are tagged sub:<def><number> (gender is not encoded — Ordbøkene lists both
masculine and feminine definite variants for many lemmas, so the gender
signal stays with the FORM, which is what the built-in heuristics use), verbs
ver:<tense> (the s-passive slots carry their tense with no passive marker),
adjectives adj:<grade> with adj:pos:BF for the bestemt-form/-e slots
(definite singular + plural); uninflected classes carry the bare POS
code. Excluded classes (ABBR, SYM, EXPR, PFX/SFX/COMPPFX/VSTEM, UNKN) have no
rule-relevant readings.

Usage:
  python3 tools/no-dict/build-no-tagger.py \
      --lemma-expanded /tmp/ordbok-cache/bm_lemma_expanded.json \
      --out data/no/dictionaries/no_pos.txt \
      --report data/no/dictionaries/build-report.txt

  python3 tools/no-dict/build-no-tagger.py --self-test

The plain-text output is compiled with the Morfologik tool (no JVM):

  python3 tools/morfologik/lt_morfologik.py pos \
      -i data/no/dictionaries/no_pos.txt \
      --info data/no/dictionaries/no_pos.info \
      -o data/no/dictionaries/no_pos.dict
"""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

ORD_UIB = "https://ord.uib.no/bm/fil"
DEFAULT_CACHE = Path("/tmp/ordbok-cache")

# Ordbøkene word class -> tagset POS prefix (data/no/words/tagset.txt).
# DET_Q (tallord) arrives with word_class "DET" and is tagged det.
WC_TO_TAG = {
    "NOUN": "sub",
    "VERB": "ver",
    "ADJ": "adj",
    "PRON": "pron",
    "DET": "det",
    "ADV": "adv",
    "ADP": "prep",
    "CCONJ": "kon",
    "SCONJ": "skon",
    "INTJ": "int",
    "PROPN": "pro",
    "INFM": "infm",
}

# Word classes whose entries carry no readings worth tagging (tagset.txt:
# "Not tagged"): abbreviations, symbols, multi-word expressions, affixes,
# compound stems and unknowns.
EXCLUDED_WC = frozenset(
    {"ABBR", "SYM", "EXPR", "PFX", "SFX", "COMPPFX", "VSTEM", "UNKN"}
)

# The template slots of the EXPORTED form lists (the leading lemma slot of
# the article template is dropped and reconstructed from the entry word).
# Nouns (NOUN_regular, NOUN_reg_fem): Sing/Ind exported empty.
NOUN_SLOTS = ["ube:sin", "bes:sin", "ube:plu", "bes:plu"]
# Verbs (VERB_regular): the article template has 16 slots [Inf(lemma), Inf,
# Pres, Inf+Pass, Pres+Pass, Past, <PerfPart>, Adj+<PerfPart> x5, PresPart,
# Imp x3]; the leading lemma Inf slot is dropped, so the 15 exported slots
# start at the SECOND Inf slot (empty in every current export). The
# s-passive slots (Inf+Pass/Pres+Pass) carry their tense with no separate
# passive marker (flat ver:<tense>, see tagset.txt).
VERB_SLOTS = [
    "inf",  # Inf (second Inf slot; exported, empty in every current entry)
    "præ",  # Pres
    "inf",  # Inf+Pass (-es)
    "præ",  # Pres+Pass (-es)
    "dat",  # Past
    "kor",  # <PerfPart>
    "kor",  # Adj <PerfPart> neuter
    "kor",  # Adj <PerfPart> masc/fem
    "kor",  # Adj <PerfPart> fem
    "kor",  # Adj <PerfPart> definite
    "kor",  # Adj <PerfPart> plural
    "lan",  # Adj <PresPart> (-ende)
    "imp",  # Imp
    "imp",  # Imp (second variant)
    "imp",  # Imp (third variant)
]
# The dropped leading lemma slot of the verb template (the infinitive),
# reconstructed from the entry word — NOT part of the exported list.
VERB_LEMMA_SLOT = "inf"
# s-passive verbs (VERB_sPass): the Inf+<SPass> slot is dropped (lemma).
VERB_SPASS_SLOTS = ["præ", "dat", "kor", "imp"]
# Adjectives (ADJ_regular): positive masc/fem slot (the lemma) dropped.
# The Def+Sing and Plur slots are the "bestemt form" (-e) reading; the
# positive masc/fem (lemma) stays the plain adj:pos and the neuter slot
# carries its own adj:pos:neu tag (the tagset doc reserves the neuter
# feature "when a rule needs it": the NB_NEUTER_T agreement rule must know
# whether the surface is already a valid neuter form).
ADJ_SLOTS = ["pos:BF", "pos:BF", "pos:neu", "kom", "sup", "sup"]
# The three masc/fem-mismatched adjectives export the feminine paradigm
# order instead: [Pos+Fem, Pos+Neuter, Pos+Def, Pos+Plur, Cmp, Sup+Ind,
# Sup+Def].
ADJ_MF_SLOTS = ["pos", "pos:neu", "pos:BF", "pos:BF", "kom", "sup", "sup"]
# Adjectival adverbs (ADV_adj: gjerne/heller/helst): positive slot dropped.
ADV_ADJ_SLOTS = ["kom", "sup"]

# Simple Bokmål word for the tagger: lowercase letters (incl. accented) and
# internal hyphens; no digits, apostrophes, capitals or whitespace.
SIMPLE_WORD = re.compile(r"^[a-zæøåéèêëíìîïóòôöúùûüýÿ-]+$")


def self_test() -> int:
    failures = 0

    def check(cond: bool, msg: str) -> None:
        nonlocal failures
        if not cond:
            failures += 1
            print(f"FAIL {msg}")

    # Slot interpretation: a 3-list noun entry with an empty first slot uses
    # the lemma; the slots are Sing/Def, Plur/Ind, Plur/Def. The lemma is
    # also the Sing/Ind reading; "hus" doubles as the Plur/Ind form.
    triples = noun_triples("hus", [["huset", "hus", "husa"], ["huset", "hus", "husene"]])
    check(
        set(triples)
        == {
            ("hus", "hus", "sub:ube:sin"),
            ("huset", "hus", "sub:bes:sin"),
            ("hus", "hus", "sub:ube:plu"),
            ("husa", "hus", "sub:bes:plu"),
            ("husene", "hus", "sub:bes:plu"),
        },
        f"noun triples for hus: {triples}",
    )
    # An empty Plur/Def slot is filled from a variant list that has it.
    triples = noun_triples("bamsefar", [["bamsefaren", "bamsefedre", ""]])
    check(
        set(triples)
        == {
            ("bamsefar", "bamsefar", "sub:ube:sin"),
            ("bamsefaren", "bamsefar", "sub:bes:sin"),
            ("bamsefedre", "bamsefar", "sub:ube:plu"),
        },
        f"noun gap fill: {triples}",
    )
    # Verbs: the export drops the leading lemma Inf slot; the 15 exported
    # slots are [Inf, Pres, Inf+Pass, Pres+Pass, Past, PerfPart,
    #  Adj<PerfPart> x5, PresPart, Imp, Imp, Imp].
    verb = ["", "løper", "løpes", "løpes", "løp", "løpt", "løpt", "løpt", "", "løpte", "løpte",
            "løpende", "løp", "", ""]
    triples = verb_triples("løpe", [verb])
    tags = sorted({t for _f, _l, t in triples})
    check(
        tags == ["ver:dat", "ver:imp", "ver:inf", "ver:kor", "ver:lan", "ver:præ"],
        f"verb tags for løpe: {tags}",
    )
    check(("løper", "løpe", "ver:præ") in triples, "verb present slot")
    check(("løpt", "løpe", "ver:kor") in triples, "verb perfect participle slot")
    check(("løpe", "løpe", "ver:inf") in triples, "verb infinitive reconstruction")
    check(("løpes", "løpe", "ver:inf") in triples, "verb s-passive infinitive")
    check(("løpes", "løpe", "ver:præ") in triples, "verb s-passive present")
    check(("løp", "løpe", "ver:dat") in triples, "verb preterite slot")
    check(("løpende", "løpe", "ver:lan") in triples, "verb present participle slot")
    check(("løp", "løpe", "ver:imp") in triples, "verb imperative slot")
    # The exported empty Inf slot is reconstructed; an inflected slot that
    # equals the lemma does not lose its own reading.
    # s-passive verbs: [Pres, Past, PerfPart, Imp], Inf = lemma.
    triples = verb_spass_triples("aldres", [["aldres", "aldredes", "aldres", "aldres"]])
    check(
        set(triples)
        == {
            ("aldres", "aldres", "ver:inf"),
            ("aldres", "aldres", "ver:præ"),
            ("aldredes", "aldres", "ver:dat"),
            ("aldres", "aldres", "ver:kor"),
            ("aldres", "aldres", "ver:imp"),
        },
        f"s-passive triples: {triples}",
    )
    # Adjectives: the export drops the leading Pos slot (the lemma); slots
    # are [Pos+Plur, Pos+Def, Pos+Neuter, Cmp, Sup, Sup+Def]. The Def/Plur
    # slots are the bestemt form (-e) reading; the neuter slot is
    # adj:pos:neu.
    triples = adj_triples("fin", [["fine", "fine", "fint", "finere", "finest", "fineste"]])
    check(
        set(triples)
        == {
            ("fin", "fin", "adj:pos"),
            ("fine", "fin", "adj:pos:BF"),
            ("fint", "fin", "adj:pos:neu"),
            ("finere", "fin", "adj:kom"),
            ("finest", "fin", "adj:sup"),
            ("fineste", "fin", "adj:sup"),
        },
        f"adjective triples for fin: {triples}",
    )
    # masc/fem-mismatched adjectives: [Pos+Fem, Pos+Neuter, Pos+Def,
    # Pos+Plur, Cmp, Sup+Ind, Sup+Def].
    triples = adj_mf_triples(
        "knøttliten",
        [["knøttlita", "knøttsmått", "knøttlille", "knøttsmå", "", "", ""]],
    )
    check(
        set(triples)
        == {
            ("knøttliten", "knøttliten", "adj:pos"),
            ("knøttlita", "knøttliten", "adj:pos"),
            ("knøttsmått", "knøttliten", "adj:pos:neu"),
            ("knøttlille", "knøttliten", "adj:pos:BF"),
            ("knøttsmå", "knøttliten", "adj:pos:BF"),
        },
        f"m/f adjective triples: {triples}",
    )
    # Adjective arbitrations: a lemma with no bestemt-form slot (the
    # defective indre/øvre/verre class) loses its adj:pos reading — the
    # -e agreement rule must not flag what has no corrected form to
    # suggest; a lemma with no distinct neuter form (fornøyd, past
    # participles) doubles the bare form as the neuter.
    triples = {
        ("glad", "glad", "adj:pos"),
        ("glad", "glad", "sub:ube:sin"),
        ("fint", "fin", "adj:pos:neu"),
        ("stort", "fin", "adj:pos:neu"),
    }
    adj_arbitrate(triples)
    check(
        triples
        == {
            ("glad", "glad", "adj:pos:neu"),
            ("glad", "glad", "sub:ube:sin"),
            ("fint", "fin", "adj:pos:neu"),
            ("stort", "fin", "adj:pos:neu"),
        },
        f"neuter arbitration: {triples}",
    )
    triples = {
        ("indre", "indre", "adj:pos"),
        ("indre", "indre", "adj:pos:neu"),
        ("stor", "stor", "adj:pos"),
        ("stort", "stor", "adj:pos:neu"),
        ("store", "stor", "adj:pos:BF"),
    }
    adj_arbitrate(triples)
    check(
        triples
        == {
            ("indre", "indre", "adj:pos:neu"),
            ("stor", "stor", "adj:pos"),
            ("stort", "stor", "adj:pos:neu"),
            ("store", "stor", "adj:pos:BF"),
        },
        f"defective-adjective arbitration: {triples}",
    )
    # Simple-word filter: internal hyphens stay, digits/apostrophes go.
    if not simple_word("e-post") or simple_word("50-årsdag") or simple_word("Occam's"):
        failures += 1
        print("FAIL simple_word filter")
    if failures:
        print(f"{failures} self-test failure(s)")
        return 1
    print("self-test ok")
    return 0


def simple_word(word: str) -> bool:
    """The tagger carries simple lowercase words (internal hyphens allowed;
    no digits, apostrophes, capitals or whitespace)."""
    return bool(SIMPLE_WORD.match(word)) and word == word.lower()


def _slot_map(
    prefix: str, features: list[str], form_lists: list[list[str]], lemma_form: str,
    lemma_feature: str,
) -> list[tuple[str, str, str]]:
    """Shared emission: collect the forms per slot feature, reconstruct the
    lemma slot, then emit one triple per (form, feature)."""
    slots: dict[str, set[str]] = {}
    for fl in form_lists:
        for idx, form in enumerate(fl):
            if not form or idx >= len(features):
                continue
            slots.setdefault(features[idx], set()).add(form)
    slots.setdefault(lemma_feature, set()).add(lemma_form)
    out = []
    for feats, forms in sorted(slots.items()):
        for form in sorted(forms):
            out.append((form, lemma_form, f"{prefix}:{feats}" if feats else prefix))
    return out


def noun_triples(word: str, form_lists: list[list[str]]) -> list[tuple[str, str, str]]:
    """Noun triples: the first template slot (Sing/Ind) is the lemma itself
    and is exported empty; a variant list that leaves a later slot empty
    (feminine form of a masc/fem lemma etc.) fills it from the variants."""
    return _slot_map("sub", NOUN_SLOTS[1:], form_lists, word, NOUN_SLOTS[0])


def verb_triples(word: str, form_lists: list[list[str]]) -> list[tuple[str, str, str]]:
    """Verb triples: the 15 exported slots start at the template's second
    Inf slot; the dropped leading lemma Inf slot is reconstructed from the
    word."""
    return _slot_map("ver", VERB_SLOTS, form_lists, word, VERB_LEMMA_SLOT)


def verb_spass_triples(
    word: str, form_lists: list[list[str]]
) -> list[tuple[str, str, str]]:
    """s-passive verb triples: the Inf slot is the (exported-dropped) lemma."""
    return _slot_map("ver", VERB_SPASS_SLOTS, form_lists, word, "inf")


def adj_triples(word: str, form_lists: list[list[str]]) -> list[tuple[str, str, str]]:
    """Adjective triples: a form that equals the lemma is the positive
    reading; a variant list that leaves a slot empty (e.g. no comparative)
    simply contributes nothing for it."""
    return _slot_map("adj", ADJ_SLOTS, form_lists, word, "pos")


def adj_mf_triples(word: str, form_lists: list[list[str]]) -> list[tuple[str, str, str]]:
    """The masc/fem-mismatched adjectives (feminine paradigm order)."""
    return _slot_map("adj", ADJ_MF_SLOTS, form_lists, word, "pos")


def adj_arbitrate(triples: set[tuple[str, str, str]]) -> None:
    """Dictionary-level adjective arbitrations over the collected triples,
    in place:

    - a lemma that carries adj:pos but has no adj:pos:neu form (Ordbøkene
      lists no distinct Pos+Neuter slot: glad, fornøyd, the past
      participles) doubles the bare form as the neuter — tagged
      adj:pos:neu, so the neuter-agreement rule stays silent on it;
    - a lemma that carries adj:pos but has no adj:pos:BF form (the
      defective adjectives indre/øvre/verre, whose positive does not
      agree) loses its adj:pos reading: the -e agreement rule must not
      flag what has no corrected form to suggest.
    """
    bf_lemmas = {lemma for _f, lemma, t in triples if t == "adj:pos:BF"}
    # No bestemt-form slot (glad-defectives, indre/øvre/verre): the positive
    # does not agree, so the -e agreement rule must not flag it; the bare
    # form doubles as the neuter.
    for triple in [t for t in triples if t[2] == "adj:pos" and t[1] not in bf_lemmas]:
        triples.remove(triple)
        triples.add((triple[1], triple[1], "adj:pos:neu"))
    # A lemma with readings but no adj:pos:neu form (fornøyd, the past
    # participles, the masc/fem-mismatched without a neuter slot) doubles
    # the bare form as the neuter.
    neu_lemmas = {lemma for _f, lemma, t in triples if t == "adj:pos:neu"}
    for lemma in sorted({l for _f, l, t in triples if t == "adj:pos"} - neu_lemmas):
        triples.add((lemma, lemma, "adj:pos:neu"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--lemma-expanded",
        type=Path,
        default=DEFAULT_CACHE / "bm_lemma_expanded.json",
        help="downloaded lemma_expanded.json (default: cached under /tmp)",
    )
    parser.add_argument("--out", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if not args.out:
        parser.error("--out is required")

    entries = json.loads(args.lemma_expanded.read_text(encoding="utf-8"))

    triples: set[tuple[str, str, str]] = set()
    stats: Counter[str] = Counter()
    for word, _lemma_id, wc, swc, _pcls, form_lists in entries:
        stats[f"lemmas_{wc}"] += 1
        if wc in EXCLUDED_WC:
            stats[f"skipped_wc_{wc}"] += 1
            continue
        tag = WC_TO_TAG.get(wc)
        if not tag:
            stats[f"skipped_wc_{wc}"] += 1
            continue
        if not simple_word(word):
            stats["skipped_not_simple"] += 1
            continue
        inflected = any(any(fl) for fl in form_lists)
        if wc == "NOUN":
            if not inflected:
                # uninflected nouns (NOUN_uninfl): the bare singular reading
                triples.add((word, word, "sub:ube:sin"))
                stats["tag_sub"] += 1
                stats["nouns_uninflected"] += 1
                continue
            for form, lemma, t in noun_triples(word, form_lists):
                if simple_word(form):
                    triples.add((form, lemma, t))
                    stats["tag_sub"] += 1
        elif wc == "VERB":
            if not inflected:
                triples.add((word, word, "ver:inf"))
                stats["tag_ver"] += 1
                stats["verbs_uninflected"] += 1
            elif swc == "VERB_sPass":
                for form, lemma, t in verb_spass_triples(word, form_lists):
                    if simple_word(form):
                        triples.add((form, lemma, t))
                        stats["tag_ver"] += 1
            else:
                for form, lemma, t in verb_triples(word, form_lists):
                    if simple_word(form):
                        triples.add((form, lemma, t))
                        stats["tag_ver"] += 1
        elif wc == "ADJ":
            if not inflected:
                # uninflected adjectives: the bare form serves every slot
                triples.add((word, word, "adj:pos"))
                triples.add((word, word, "adj:pos:BF"))
                triples.add((word, word, "adj:pos:neu"))
                stats["tag_adj"] += 1
                stats["adjs_uninflected"] += 1
            elif swc == "ADJ_masc/fem_fem":
                for form, lemma, t in adj_mf_triples(word, form_lists):
                    if simple_word(form):
                        triples.add((form, lemma, t))
                        stats["tag_adj"] += 1
            else:
                for form, lemma, t in adj_triples(word, form_lists):
                    if simple_word(form):
                        triples.add((form, lemma, t))
                        stats["tag_adj"] += 1
        elif wc == "ADV" and swc == "ADV_adj":
            # comparative/superlative adverbs (gjerne/heller/helst)
            for form, lemma, t in _slot_map(
                "adv", ADV_ADJ_SLOTS, form_lists, word, "pos"
            ):
                triples.add((form, lemma, t))
                stats["tag_adv"] += 1
        elif wc == "DET" and swc == "DET_adj":
            # adjectival determiners (annen/annet/andre): every form carries
            # the plain det reading
            for fl in form_lists:
                for form in fl:
                    if form and simple_word(form):
                        triples.add((form, word, "det"))
                        stats["tag_det"] += 1
            triples.add((word, word, "det"))
            stats["tag_det"] += 1
        else:
            # uninflected classes (pron, det, adv, prep, kon, skon, int,
            # infm, PROPN): the bare POS reading.
            triples.add((word, word, tag))
            stats[f"tag_{tag}"] += 1

    adj_arbitrate(triples)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    lines = [f"{form}\t{lemma}\t{t}" for (form, lemma, t) in sorted(triples)]
    args.out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    if args.report:
        per_tag = Counter(t for _f, _l, t in triples)
        per_pos = Counter()
        for t, count in per_tag.items():
            per_pos[t.split(":")[0]] += count
        pos_lines = "\n".join(
            f"  {pos}: {count}" for pos, count in sorted(per_pos.items())
        )
        stats_lines = "\n".join(
            f"  {key}: {value}" for key, value in sorted(stats.items())
        )
        args.report.write_text(
            "Norwegian (Bokmål) POS tagger dictionary build\n"
            f"source: Ordbøkene lemma_expanded.json ({ORD_UIB}/lemma_expanded.json), "
            "CC BY 4.0 (Ordbanken/Ordbøkene)\n"
            f"Ordbøkene lemma entries: {len(entries)}\n"
            f"tagger triples written: {len(triples)}\n"
            "triples per tag:\n"
            f"{pos_lines}\n"
            f"{stats_lines}\n",
            encoding="utf-8",
        )
    print(f"wrote {args.out} ({len(triples)} triples)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
