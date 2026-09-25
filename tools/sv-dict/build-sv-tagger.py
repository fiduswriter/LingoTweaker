#!/usr/bin/env python3
"""Build the Swedish POS tagger dictionary inputs (`swedish.dict` triples).

The Språkbanken SALDO morphological lexicon (saldom.xml, the development
export of the morphological SALDO, CC BY 4.0) is authoritative:

  https://svn.spraakbanken.gu.se/sb-arkiv/pub/lexikon/saldom/saldom.xml
  (downloaded 2026-09-23; the official SALDO distribution page,
  https://sprakbanken.se/en/resources/saldo, lists the SALDO lexicon under
  CC-BY-4.0 and exposes the `saldom` morphological lexicon through the same
  resource; the per-lemma lexicon `saldo.xml` (LMF, 131,020 entries) is the
  published counterpart without inflection tables)

Each LexicalEntry carries `<gf>` (grundform/lemma), `<pos>` (word class,
`*m`-suffixed classes are multi-word expressions), `<inhs>` (inherent
features: nouns `u`=uter, `n`=neuter, `v`/`p`=varies -> indeterminate) and a
`<table>` of `<form param=... wf=...>` inflection slots in Swedish
terminology. The emitted tag is the existing SUC-style tagset of
`data/sv/words/tagset.txt` — the same tags the DSSO-derived dict used, keyed
by the 32 XML rules, the disambiguator unification (`sv/disambiguation.xml`)
and the MultiWordChunker:

  nn family (nn/nnm/nna/nnh)   NN:<BF|OF>:<SIN|PLU>:<NOM|GEN>:<UTR|NEU|NON>
                               (sg indef -> OF, sg def -> BF, pl indef -> OF,
                               pl def -> BF; gender from `<inhs>`,
                               NON for `v`/`p`)
  nl (numerals)                NN:OF:SIN:NOM/GEN:UTR/NEU (num forms; ordinals
                               -> UTR; the `c` compound-member forms are
                               dropped)
  av (adjectives)              pos indef sg u -> JJ:PU, pos indef sg n ->
                               JJ:PN, pos indef pl -> JJ:P, pos def sg
                               no_masc + pos def pl -> JJ:BF, pos def sg
                               masc -> JJ:M, komp -> JJ:K, super indef ->
                               JJ:S, super def masc -> JJ:S:BF:M, super def
                               no_masc -> JJ:S:BF:NM, invar -> the full
                               JJ:PU/JJ:PN/JJ:P/JJ:BF set (the DSSO
                               convention for invariant adjectives, e.g.
                               `bra`)
  ab                           AB (invar/komp/super/pos forms)
  vb (verbs)                   inf aktiv -> VB:INF, inf s-form -> VB:INF:PF,
                               pres ind aktiv -> VB:PRS, pres ind s-form ->
                               VB:PRS:PF, pret ind aktiv -> VB:PRT,
                               pret ind s-form -> VB:PRT:PF, sup aktiv ->
                               VB:SUP, sup s-form -> VB:SUP:PF, imper ->
                               VB:IMP, konj (pres/pret, aktiv/s-form) ->
                               VB:KON, pres_part nom/gen -> VB:PREPC,
                               pret_part indef sg u -> VB:PPC:UTR,
                               pret_part indef sg n -> VB:PPC:NEU,
                               pret_part pl/def -> VB:PPC:PLU
  vba                          invar -> VB:INF
  pm (proper nouns)            nom -> PM:NOM, gen -> PM:GEN (the personal-name
                               noun paradigms, compound-member `c`/`ci`/`cm`
                               and `sms` forms are dropped)
  pn/pnm/al                    PN, pp -> PP, kn/knm -> KN, in/inm -> IN,
                               sn/snm -> KN

Multi-word entries (`gf` contains a space: the `*m` multiword classes) are
skipped entirely — their whole-expression forms cannot be tagged by a
per-token tagger and their per-part fragments would only add noise. The
compound-relevant `c`/`ci`/`cm`/`sms`/`frag` params (compound member/stem
forms, usually ending in `-`) are likewise dropped: the tagger dictionary
carries lexical words, compound analysis stays with the hunspell speller
(`sv_SE.dic`, untouched).

Emission: every surviving (wf, gf, tag) becomes a `form<TAB>lemma<TAB>tag`
triple, deduplicated. The seven empty-tag punctuation entries of the DSSO
dict are carried over unchanged (they give the sentence punctuation tokens
their present tagger behavior).

Usage:
  python3 tools/sv-dict/build-sv-tagger.py \
      --saldom /tmp/saldom.xml \
      --old-dict /tmp/sv-old-dict.txt \
      --out data/sv/dictionaries/swedish_triples.txt \
      --report data/sv/dictionaries/build-report.txt

  python3 tools/sv-dict/build-sv-tagger.py --self-test

The plain-text output is compiled with the Morfologik tool (no JVM); the
synthesizer dictionary is built from the same triples:

  python3 tools/morfologik/lt_morfologik.py pos \
      -i data/sv/dictionaries/swedish_triples.txt \
      --info data/sv/dictionaries/swedish.info \
      -o data/sv/dictionaries/swedish.dict
  python3 tools/morfologik/lt_morfologik.py synth \
      -i data/sv/dictionaries/swedish_triples.txt \
      --info data/sv/dictionaries/swedish_synth.info \
      -o data/sv/dictionaries/swedish_synth.dict
"""

from __future__ import annotations

import argparse
import re
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

# noun slot order: NN:<def>:<num>:<case>:<gender>
_NOUN_DEF = {"sg indef": "OF", "sg def": "BF", "pl indef": "OF", "pl def": "BF"}
_NOUN_NUM = {"sg": "SIN", "pl": "PLU"}
_NOUN_CASE = {"nom": "NOM", "gen": "GEN"}

_GENDER_INHS = {"u": "UTR", "n": "NEU"}

# verb params -> tag (nom/gen and position-indexed params collapse as noted)
_VERB_TAGS = {
    "inf aktiv": "VB:INF",
    "inf s-form": "VB:INF:PF",
    "pres ind aktiv": "VB:PRS",
    "pres ind s-form": "VB:PRS:PF",
    "pret ind aktiv": "VB:PRT",
    "pret ind s-form": "VB:PRT:PF",
    "sup aktiv": "VB:SUP",
    "sup s-form": "VB:SUP:PF",
    "imper": "VB:IMP",
    "pres konj aktiv": "VB:KON",
    "pres konj s-form": "VB:KON",
    "pret konj aktiv": "VB:KON",
    "pret konj s-form": "VB:KON",
    "pres_part": "VB:PREPC",
}

# perfect participle params -> tag (nom/gen collapse into the same tag)
_PRET_PART_TAGS = {
    "pret_part indef sg u": "VB:PPC:UTR",
    "pret_part indef sg n": "VB:PPC:NEU",
    "pret_part indef pl": "VB:PPC:PLU",
    "pret_part def pl": "VB:PPC:PLU",
    "pret_part def sg no_masc": "VB:PPC:PLU",
    "pret_part def sg masc": "VB:PPC:PLU",
}

# adjective params -> tag (SALDO param -> SUC-style JJ tag)
_ADJ_TAGS = {
    "pos indef sg u": "JJ:PU",
    "pos indef sg n": "JJ:PN",
    "pos indef pl": "JJ:P",
    "pos def sg no_masc": "JJ:BF",
    "pos def sg masc": "JJ:M",
    "pos def pl": "JJ:BF",
    "komp": "JJ:K",
    "super indef": "JJ:S",
    "super def masc": "JJ:S:BF:M",
    "super def no_masc": "JJ:S:BF:NM",
}

_INVARIANT_ADJ_TAGS = ("JJ:PU", "JJ:PN", "JJ:P", "JJ:BF")

_NUM_TAGS = {
    "nom num u": "NN:OF:SIN:NOM:UTR",
    "nom num n": "NN:OF:SIN:NOM:NEU",
    "gen num u": "NN:OF:SIN:GEN:UTR",
    "gen num n": "NN:OF:SIN:GEN:NEU",
    "nom ord no_masc": "NN:OF:SIN:NOM:UTR",
    "nom ord masc": "NN:OF:SIN:NOM:UTR",
    "gen ord no_masc": "NN:OF:SIN:GEN:UTR",
    "gen ord masc": "NN:OF:SIN:GEN:UTR",
}

# compound-member/fragment slots (these forms carry a trailing '-' or exist
# only to build compounds)
_SKIP_PARAMS = {"c", "ci", "cm", "sms", "frag"}

# closed classes: every surviving form gets the bare tag
_BARE_TAGS = {
    "pn": "PN",
    "pnm": "PN",
    "al": "PN",
    "pp": "PP",
    "ppm": "PP",
    "kn": "KN",
    "knm": "KN",
    "in": "IN",
    "inm": "IN",
    "sn": "KN",
    "snm": "KN",
    "ab": "AB",
    "vba": "VB:INF",
}


def _noun_tag(param: str, gender: str) -> str | None:
    """Map a noun-inflection param (`sg def nom`, ...) to a NN tag."""
    match = re.fullmatch(r"(sg|pl) (indef|def) (nom|gen)", param)
    if match is None:
        return None
    num, indef, case = match.groups()
    return f"NN:{_NOUN_DEF[f'{num} {indef}']}:{_NOUN_NUM[num]}:{_NOUN_CASE[case]}:{gender}"


def emit_for_entry(pos: str, gf: str, inhs: str, param: str, wf: str) -> list[tuple[str, str, str]]:
    """Map one (pos, gf, inhs, param, wf) form to (form, lemma, tag) triples.

    Verb and adjective genitive slots are dropped: the DSSO dict the rules and
    disambiguator were built against has no participle or adjective genitive
    forms, and SALDO's extra gen readings would surface as extra synth
    suggestions for the <match>-based XML rules.
    """
    if pos in ("nn", "nnm", "nna", "nnh"):
        tag = _noun_tag(param, _GENDER_INHS.get(inhs, "NON"))
        return [(wf, gf, tag)] if tag else []
    if pos == "av":
        if param.endswith(" gen"):
            return []
        base = param.removesuffix(" nom")
        if base == "invar":
            return [(wf, gf, tag) for tag in _INVARIANT_ADJ_TAGS]
        tag = _ADJ_TAGS.get(base)
        return [(wf, gf, tag)] if tag else []
    if pos in ("vb", "vbm"):
        if param.endswith(" gen"):
            return []
        if param == "imper":
            return [(wf, gf, "VB:IMP")]
        base = param.removesuffix(" nom")
        tag = _VERB_TAGS.get(base) or _PRET_PART_TAGS.get(base)
        return [(wf, gf, tag)] if tag else []
    if pos == "ab":
        return [(wf, gf, "AB")] if param in ("invar", "komp", "super", "pos") else []
    if pos == "pm":
        if param == "nom":
            return [(wf, gf, "PM:NOM")]
        if param == "gen":
            return [(wf, gf, "PM:GEN")]
        return []
    if pos == "nl":
        tag = _NUM_TAGS.get(param)
        return [(wf, gf, tag)] if tag else []
    bare = _BARE_TAGS.get(pos)
    if bare is not None:
        # closed classes: any surviving (non-compound) form gets the bare tag
        return [(wf, gf, bare)]
    return []


# citation forms that give a multi-word entry's fused sub-paradigm its lemma
_FUSED_HEAD_PARAMS = (
    "sg indef nom",
    "pos indef sg u nom",
    "pret_part indef sg u nom",
    "nom",
    "nom num u",
    "inf aktiv",
)

_FUSED_SUFFIX = re.compile(r" \d+:1-1$")
_MULTI_PART_SUFFIX = re.compile(r" \d+:\d+-\d+$")


def _entry_forms(elem) -> list[tuple[str, str, bool]]:
    """Collect (base_param, wf, fused) for the forms a tagger reading can use.

    Multi-word entries (gf with a space) contribute only their fused
    single-part variants (e.g. the verb `ladda ur` fuses its particle into
    `urladdad`/`urladdat`), whose word forms are words in their own right; the
    per-part indexed forms (`pres ind aktiv 1:1-2` -> `laddar`) are phrase
    fragments and are covered by the standalone lemmas. Compound-member slots
    (`c`/`ci`/`cm`/`sms`/`frag`) are dropped in every entry.
    """
    multiword = " " in (elem.findtext("gf") or "")
    forms: list[tuple[str, str, bool]] = []
    for form in elem.iter("form"):
        param = (form.findtext("param") or "").strip()
        wf = (form.findtext("wf") or "").strip()
        if not wf or " " in wf or wf.endswith("-") or param in _SKIP_PARAMS:
            continue
        if _FUSED_SUFFIX.search(param):
            # fused single-part variant of a multi-word expression
            forms.append((_FUSED_SUFFIX.sub("", param), wf, True))
            continue
        if _MULTI_PART_SUFFIX.search(param):
            # part of a multi-word expression, not a word in its own right
            continue
        if multiword:
            continue
        forms.append((param, wf, False))
    return forms


def build(saldom_path: Path, old_dict_path: Path, out_path: Path, report_path: Path) -> None:
    triples: set[tuple[str, str, str]] = set()
    triples |= _punctuation_triples(old_dict_path)

    for _event, elem in ET.iterparse(saldom_path, events=("end",)):
        if elem.tag != "LexicalEntry":
            continue
        pos = (elem.findtext("pos") or "").strip()
        gf = (elem.findtext("gf") or "").strip()
        inhs = (elem.findtext("inhs") or "-").strip()
        if "+" in gf:
            # the dict separator (+) cannot occur in a word or lemma
            elem.clear()
            continue
        forms = _entry_forms(elem)
        if forms and " " in gf:
            # fused sub-paradigm of a multi-word expression (the verb `ladda
            # ur` fuses its particle into `urladdad`/`urladdat`): emit the
            # forms under the family's citation form as lemma, so the
            # synthesizer inflects them like the DSSO dict did
            head = next(
                (wf for base, wf, fused in forms if fused and base in _FUSED_HEAD_PARAMS),
                None,
            ) or next((wf for base, wf, fused in forms if fused), None)
            for base, wf, fused in forms:
                if fused:
                    triples.update(emit_for_entry(pos, head, inhs, base, wf))
        else:
            for base, wf, fused in forms:
                # the `al` entry is a closed-class paradigm
                # (en/den/ett/det/de/dom) whose gf is only the head form: each
                # inflected pronoun form keeps itself as lemma, like the DSSO
                # dict
                lemma = wf if pos == "al" else gf
                triples.update(emit_for_entry(pos, lemma, inhs, base, wf))
        elem.clear()

    lines = sorted((f"{word}\t{lemma}\t{tag}" for word, lemma, tag in triples))
    with open(out_path, "w", encoding="utf-8", newline="\n") as handle:
        for line in lines:
            handle.write(line + "\n")

    pos_counts = Counter(tag.split(":")[0] for _, _, tag in triples)
    tag_counts = Counter(tag for _, _, tag in triples)
    forms = {word for word, _, _ in triples}
    lemmas = {lemma for _, lemma, _ in triples}
    with open(report_path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(f"source: {saldom_path}\n")
        handle.write(f"triples: {len(triples)}\n")
        handle.write(f"distinct forms: {len(forms)}\n")
        handle.write(f"distinct lemmas: {len(lemmas)}\n")
        handle.write("POS:\n")
        for pos, count in sorted(pos_counts.items(), key=lambda kv: -kv[1]):
            handle.write(f"  {pos:8} {count}\n")
        handle.write("tags:\n")
        for tag, count in sorted(tag_counts.items()):
            handle.write(f"  {tag:24} {count}\n")


def _punctuation_triples(old_dict_path: Path) -> set[tuple[str, str, str]]:
    """Carry the punctuation entries of the DSSO dict over verbatim.

    The old dict gives the seven sentence punctuation tokens readings whose
    tag is the punctuation character itself (`.`, `,`, ...; `"` has an empty
    tag). Reproduce those entries so the tagger behavior for punctuation is
    unchanged.
    """
    out: set[tuple[str, str, str]] = set()
    with open(old_dict_path, encoding="utf-8") as handle:
        for line in handle:
            parts = line.rstrip("\n").split("+")
            if len(parts) == 3 and parts[0] == parts[1] == parts[2]:
                out.add((parts[1], parts[0], parts[2]))
            elif len(parts) == 2 and parts[0] == parts[1]:
                out.add((parts[1], parts[0], ""))
    return out


def _self_test() -> int:
    # nouns
    assert list(emit_for_entry("nn", "bil", "u", "sg indef nom", "bil")) == [
        ("bil", "bil", "NN:OF:SIN:NOM:UTR")
    ]
    assert list(emit_for_entry("nn", "bil", "u", "sg def gen", "bilens")) == [
        ("bilens", "bil", "NN:BF:SIN:GEN:UTR")
    ]
    assert list(emit_for_entry("nn", "hus", "n", "pl def nom", "husen")) == [
        ("husen", "hus", "NN:BF:PLU:NOM:NEU")
    ]
    assert list(emit_for_entry("nn", "x", "v", "sg indef nom", "x")) == [
        ("x", "x", "NN:OF:SIN:NOM:NON")
    ]
    # compound-member slots are dropped
    assert list(emit_for_entry("nn", "bil", "u", "ci", "bil-")) == []
    assert list(emit_for_entry("nn", "bil", "u", "sms", "bil-")) == []
    # adjectives
    assert list(emit_for_entry("av", "stor", "-", "pos indef sg u nom", "stor")) == [
        ("stor", "stor", "JJ:PU")
    ]
    assert list(emit_for_entry("av", "stor", "-", "pos indef sg n nom", "stort")) == [
        ("stort", "stor", "JJ:PN")
    ]
    assert list(emit_for_entry("av", "stor", "-", "pos indef pl nom", "stora")) == [
        ("stora", "stor", "JJ:P")
    ]
    assert list(emit_for_entry("av", "stor", "-", "pos def sg no_masc nom", "stora")) == [
        ("stora", "stor", "JJ:BF")
    ]
    assert list(emit_for_entry("av", "stor", "-", "pos def sg masc nom", "store")) == [
        ("store", "stor", "JJ:M")
    ]
    assert list(emit_for_entry("av", "stor", "-", "komp nom", "större")) == [
        ("större", "stor", "JJ:K")
    ]
    assert list(emit_for_entry("av", "fin", "-", "super indef nom", "finast")) == [
        ("finast", "fin", "JJ:S")
    ]
    assert list(emit_for_entry("av", "fin", "-", "super def masc nom", "finaste")) == [
        ("finaste", "fin", "JJ:S:BF:M")
    ]
    assert list(emit_for_entry("av", "fin", "-", "super def no_masc nom", "finaste")) == [
        ("finaste", "fin", "JJ:S:BF:NM")
    ]
    # invariant adjective: full feature set like the DSSO convention
    assert list(emit_for_entry("av", "bra", "-", "invar", "bra")) == [
        ("bra", "bra", "JJ:PU"),
        ("bra", "bra", "JJ:PN"),
        ("bra", "bra", "JJ:P"),
        ("bra", "bra", "JJ:BF"),
    ]
    # verbs
    assert list(emit_for_entry("vb", "gå", "-", "inf aktiv", "gå")) == [("gå", "gå", "VB:INF")]
    assert list(emit_for_entry("vb", "gå", "-", "inf s-form", "gås")) == [
        ("gås", "gå", "VB:INF:PF")
    ]
    assert list(emit_for_entry("vb", "gå", "-", "pres ind s-form", "gås")) == [
        ("gås", "gå", "VB:PRS:PF")
    ]
    assert list(emit_for_entry("vb", "gå", "-", "pret ind s-form", "gicks")) == [
        ("gicks", "gå", "VB:PRT:PF")
    ]
    assert list(emit_for_entry("vb", "gå", "-", "sup s-form", "gåtts")) == [
        ("gåtts", "gå", "VB:SUP:PF")
    ]
    assert list(emit_for_entry("vb", "gå", "-", "pres konj aktiv", "gå")) == [
        ("gå", "gå", "VB:KON")
    ]
    assert list(emit_for_entry("vb", "gå", "-", "pret konj s-form", "ginge")) == [
        ("ginge", "gå", "VB:KON")
    ]
    assert list(emit_for_entry("vb", "gå", "-", "imper", "gå")) == [("gå", "gå", "VB:IMP")]
    assert list(emit_for_entry("vb", "se", "-", "pres_part nom", "seende")) == [
        ("seende", "se", "VB:PREPC")
    ]
    assert list(emit_for_entry("vb", "se", "-", "pret_part indef sg u nom", "sedd")) == [
        ("sedd", "se", "VB:PPC:UTR")
    ]
    assert list(emit_for_entry("vb", "se", "-", "pret_part indef sg n nom", "sett")) == [
        ("sett", "se", "VB:PPC:NEU")
    ]
    assert list(emit_for_entry("vb", "se", "-", "pret_part def pl nom", "sedda")) == [
        ("sedda", "se", "VB:PPC:PLU")
    ]
    # verb/adjective genitive slots are dropped (absent from the DSSO dict)
    assert list(emit_for_entry("vb", "se", "-", "pret_part indef pl gen", "seddas")) == []
    assert list(emit_for_entry("vb", "se", "-", "pres_part gen", "seendes")) == []
    assert list(emit_for_entry("av", "svart", "-", "pos indef pl gen", "svartas")) == []
    # nouns keep their genitive forms
    assert list(emit_for_entry("nn", "bil", "u", "sg indef gen", "bils")) == [
        ("bils", "bil", "NN:OF:SIN:GEN:UTR")
    ]
    # position-indexed verb params (multiword verbs) do not map
    assert list(emit_for_entry("vb", "gå", "-", "pres ind aktiv 1:1-2", "gå")) == []
    # bare classes
    assert list(emit_for_entry("pn", "här", "-", "ack", "här")) == [("här", "här", "PN")]
    assert list(emit_for_entry("al", "den", "-", "pl def", "de")) == [("de", "den", "PN")]
    assert list(emit_for_entry("pp", "av", "-", "invar", "av")) == [("av", "av", "PP")]
    assert list(emit_for_entry("sn", "att", "-", "invar", "att")) == [("att", "att", "KN")]
    assert list(emit_for_entry("ab", "ibland", "-", "komp", "iblandare")) == [
        ("iblandare", "ibland", "AB")
    ]
    # numerals
    assert list(emit_for_entry("nl", "två", "-", "nom num u", "två")) == [
        ("två", "två", "NN:OF:SIN:NOM:UTR")
    ]
    assert list(emit_for_entry("nl", "två", "-", "gen num n", "tvås")) == [
        ("tvås", "två", "NN:OF:SIN:GEN:NEU")
    ]
    print("self-test OK")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--saldom", type=Path)
    parser.add_argument("--old-dict", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return _self_test()
    if not (args.saldom and args.old_dict and args.out and args.report):
        parser.error("--saldom/--old-dict/--out/--report are required")
    build(args.saldom, args.old_dict, args.out, args.report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())