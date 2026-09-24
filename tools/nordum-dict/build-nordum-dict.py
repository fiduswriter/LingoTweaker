#!/usr/bin/env python3
"""Build the Nordum Hunspell dictionary.

Nordum's vocabulary is largely Scandinavian; its orthography is a regularized
union of Norwegian Bokmål, Danish and Swedish (NORDUM_LANGUAGE_SPECIFICATION.md
§3.3.2, §3.6, §7). This tool applies the Nordum orthographic rules to the
source-language dictionaries and merges them with the authoritative Nordum
word list, producing:

- ``nrd.dic``       — the speller dictionary (broad coverage),
- ``nrd_core.dic``  — the authoritative core (+ accepted alternatives), used
                      as the suggestion candidate list.

The authoritative Nordum word list carries POS information
(``dictionary.json``), so its noun/verb/adjective paradigms are generated per
spec §4 (see ``nordum_convert``) and added to both dictionaries.

Since the owner's strictness decision (2026-09-23) the converted source words
no longer enter the speller verbatim: a form is only accepted when it is
derivable by the spec §4 morphology from a Nordum lemma. The converted lemma
inventories (with POS) are the same ones the tagger build derives from the
da/sv tagger dictionaries — the Danish/Swedish verb tense forms (Swedish
``-ar`` presents, ``-a`` infinitives, ``-at`` supines, ``-ade`` pasts) and the
Danish/Swedish ``-er`` noun plurals are never carried over; the spec forms are
generated from the converted lemma instead. Unclassified source words (the
Norwegian word list and the da/sv words without a usable tag) are analysed by
their endings against the lemma inventory: a form whose only reading is a
spec-violating inflection of a known lemma (e.g. the Danish «biler» plural of
«bil», whose Nordum plural is «bilar») is dropped; forms without any
inflectional analysis are kept as base forms (the owner prefers strict —
ambiguous inflected forms are dropped).

Usage:
  python3 build-nordum-dict.py \
      --nordum-wordlist /path/to/nordum/build/assets/data/wordlist.txt \
      --nordum-dictionary /path/to/nordum/build/assets/data/dictionary.json \
      --additions data/nrd/words/core_additions.txt \
      --sources nb_NO.dic da_DK.dic sv_SE.dic \
      --da-export data/da/dictionaries/danish.dict \
      --sv-export data/sv/dictionaries/swedish.dict \
      --out data/nrd/hunspell/nrd.dic \
      --out-core data/nrd/hunspell/nrd_core.dic \
      --report data/nrd/hunspell/build-report.txt

  python3 build-nordum-dict.py --self-test
"""

from __future__ import annotations

import argparse
import sys
from collections import Counter
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

# Permanent word-level checks of the strict conversion (owner decision
# 2026-09-23): converted source forms whose only reading violates the spec
# §4/§4.3 morphology must not be accepted, while the class-correct forms stay.
# Checked against the converted-source inventory the strict build produces.
STRICT_ACCEPTED_WORDS = frozenset({
    "bakke",       # da noun/verb lemma base
    "bakker",      # present of the verb bakke (spec §4.2 present -er)
    "bakkar",      # plural of the noun bakke (spec §4.3 plural -ar)
    "bilar",       # plural of the noun bil (the Danish «biler» is dropped)
    "bilen",       # common definite singular of bil (§4.3 -en)
    "huset",       # neuter definite singular of hus (§4.3 -et)
    "flikkar",     # plural of the noun flikka (converted sv flicka)
    "flikka",      # the converted sv noun lemma itself
    "jentarna",    # definite plural of jente (plural + na)
    "kaller",       # present of kalle (converted sv kalla)
    "anfalle",      # the normalized -e infinitive of sv anfalla (§4.2.3)
    "arbeider",     # present of arbeide (converted da arbejde)
    # The surface keeps its own class: a form the tagger data reads only as a
    # spec-violating inflection is dropped even when another lemma's paradigm
    # could generate the same string (see STRICT_DROPPED_WORDS).
    "arbetande",    # SUC lexicalized present-participle adjective (den
                    # arbetande); spec §4.3.3 sanctions -ande/-ende words
})
STRICT_DROPPED_WORDS = frozenset({
    "katter",       # Danish/nb -er plural of katt (spec: kattar)
    "flickorna",    # Swedish plural definite (spec: flikkarna)
    "arbetar",      # sv -ar present of arbete (its own reading; the -ar noun
                    # plural of the noun arbete is not a source reading)
    "kallar",       # sv -ar present of kalla (its own reading)
    "arbetade",     # Swedish -ade past (spec: -ede/-a)
    "arbetat",      # Swedish -at supine (spec: -et)
    "bakkerne",     # Danish plural definite (spec: bakkarna)
    "bilerne",     # Danish plural definite of bil
    "husene",      # nb/sv plural definite of hus
    "personer",    # Danish -er plural of person (spec: personar)
    "gåer",        # owner-removed archaic present of gå (nordum@6be2b42)
    "gåede",       # owner-removed archaic past of gå
    "gået",        # owner-removed archaic supine of gå
    "jenter",      # -er plural of jente (spec: jentar; NDM_MORPHOLOGY data)
})


def verify_strict_morphology(
    infinitives: set[str],
    presents: set[str],
    noun_plurals: set[str],
    noun_plurdefs: set[str],
    provided_infinitives: set[str],
    provided_presents: set[str],
    provided_noun_plurals: set[str],
    provided_noun_plurdefs: set[str],
    quiet: bool = False,
) -> int:
    """The permanent spec §4/§4.3 regression checks over the generated
    inventory (the forms whose class the build knows, pre-merge): a verb
    present always ends ``-er`` except the authoritative irregulars, an
    infinitive always ends ``-e`` except ``ha``, a noun plural always ends
    ``-ar`` and a definite plural always ends ``-na`` except the authoritative
    provided forms (spec §4.3.1 invariant plurals)."""
    failures = 0
    bad = sorted(f for f in presents if not f.endswith("er"))
    expected = sorted(f for f in provided_presents if not f.endswith("er"))
    if bad != expected:
        failures += 1
        if not quiet:
            print(f"FAIL verb presents beyond the spec -er irregulars: {bad[:10]}")
    bad = sorted(f for f in infinitives if not f.endswith("e"))
    expected = sorted(f for f in provided_infinitives if not f.endswith("e"))
    if bad != expected:
        failures += 1
        if not quiet:
            print(f"FAIL infinitives beyond the spec -e irregulars: {bad[:10]}")
    bad = sorted(f for f in noun_plurals if not f.endswith("ar"))
    expected = sorted(f for f in provided_noun_plurals if not f.endswith("ar"))
    if bad != expected:
        failures += 1
        if not quiet:
            print(f"FAIL noun plurals beyond the spec -ar forms: {bad[:10]}")
    bad = sorted(f for f in noun_plurdefs if not f.endswith("na"))
    expected = sorted(f for f in provided_noun_plurdefs if not f.endswith("na"))
    if bad != expected:
        failures += 1
        if not quiet:
            print(f"FAIL definite plurals beyond the spec -na forms: {bad[:10]}")
    return failures


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
    # Strict conversion: the §4.3 stem of a converted -a noun (sv flikka).
    if nc.converted_noun_stem("flikka") != "flikk":
        failures += 1
        print("FAIL converted_noun_stem flikka")
    if nc.converted_noun_stem("jente") != "jent":
        failures += 1
        print("FAIL converted_noun_stem jente")
    if nc.converted_noun_stem("hus") != "hus":
        failures += 1
        print("FAIL converted_noun_stem hus")
    if nc.converted_noun_stem("by") != "by":
        failures += 1
        print("FAIL converted_noun_stem other-vowel stem stays unstripped")
    # §4.4.1 grade shapes: the spec-shaped suppletives pass, the systematic
    # Swedish -are/-ast and definite -ste/-sta forms do not.
    for form, grade, expected_ok in (
        ("større", "kom", True), ("størst", "sup", True),
        ("mindre", "kom", True), ("bedst", "sup", True),
        ("mer", "kom", True), ("mest", "sup", True),
        ("fler", "kom", True), ("dummere", "kom", True),
        ("kallare", "kom", False), ("godast", "sup", False),
        ("størsta", "sup", False), ("dummeste", "sup", False),
        ("nærmare", "kom", False),
    ):
        if nc.grade_shape_ok(form, grade) != expected_ok:
            failures += 1
            print(f"FAIL grade shape {form}: expected accepted={expected_ok}")
    # The permanent §4/§4.3 role checks: only the authoritative irregulars
    # (er/har/går, ha, invariant hus) may deviate from the systematic endings.
    failures += verify_strict_morphology(
        infinitives={"arbeide", "ha"},
        presents={"arbeider", "er", "har", "går"},
        noun_plurals={"jentar", "husar"},
        noun_plurdefs={"jentarna", "husarna"},
        provided_infinitives={"ha"},
        provided_presents={"er", "har", "går"},
        provided_noun_plurals=set(),
        provided_noun_plurdefs=set(),
    )
    if not verify_strict_morphology(
        infinitives={"anfalla"}, presents={"arbeidar"},
        noun_plurals={"biler"}, noun_plurdefs={"flickorna"},
        provided_infinitives=set(), provided_presents={"er", "har", "går"},
        provided_noun_plurals=set(), provided_noun_plurdefs=set(),
        quiet=True,
    ) != 4:
        failures += 1
        print("FAIL verify_strict_morphology must reject all four violations")
    if failures:
        print(f"{failures} self-test failure(s)")
        return 1
    print("self-test ok")
    return 0


def collect_authoritative_role_forms(entries: list[dict]) -> dict[str, set[str]]:
    """The authoritative generated §4 role forms and the provided (irregular)
    forms that may legitimately deviate from the systematic endings."""
    roles = {
        "infinitive": (set(), set()),
        "present": (set(), set()),
        "pluralIndefinite": (set(), set()),
        "pluralDefinite": (set(), set()),
    }
    for entry in entries:
        inflections = entry.get("inflections") or {}
        pos = entry.get("pos")
        if pos == "verb":
            forms, counts = nc.verb_forms(inflections)
        elif pos == "noun":
            forms, counts = nc.noun_forms(inflections, entry.get("gender"))
        else:
            continue
        for role in roles:
            form = forms.get(role)
            if not form:
                continue
            provided, all_forms = roles[role]
            all_forms.add(form)
            if counts.get(f"{role}:provided"):
                provided.add(form)
    return {
        "infinitives": roles["infinitive"][1],
        "presents": roles["present"][1],
        "noun_plurals": roles["pluralIndefinite"][1],
        "noun_plurdefs": roles["pluralDefinite"][1],
        "provided_infinitives": roles["infinitive"][0],
        "provided_presents": roles["present"][0],
        "provided_noun_plurals": roles["pluralIndefinite"][0],
        "provided_noun_plurdefs": roles["pluralDefinite"][0],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nordum-wordlist", type=Path)
    parser.add_argument("--nordum-dictionary", type=Path)
    parser.add_argument("--additions", type=Path)
    parser.add_argument("--sources", type=Path, nargs="*", default=[])
    parser.add_argument("--da-export", type=Path,
                        default=nc.REPO_ROOT / "data/da/dictionaries/danish.dict",
                        help="the Danish tagger dictionary (decompiled for the "
                             "converted lemma inventories; cached decompile)")
    parser.add_argument("--sv-export", type=Path,
                        default=nc.REPO_ROOT / "data/sv/dictionaries/swedish.dict",
                        help="the Swedish tagger dictionary (decompiled for the "
                             "converted lemma inventories; cached decompile)")
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

    # N2: the strict converted-source inventory (owner decision 2026-09-23):
    # spec §4 morphology generated from the converted lemma inventories only;
    # spec-violating inflected forms are dropped.
    strict = nc.build_strict_source_forms(
        args.sources, args.da_export, args.sv_export, entries, sorted(core))
    dictionary.update(strict["accepted"])

    # Permanent regression checks over the generated inventory (the classes
    # the build knows, pre-merge) and the owner-named example words.
    auth = collect_authoritative_role_forms(entries)
    failures = verify_strict_morphology(
        infinitives=strict["assert_infinitives"] | auth["infinitives"],
        presents=strict["assert_presents"] | auth["presents"],
        noun_plurals=strict["assert_noun_plurals"] | auth["noun_plurals"],
        noun_plurdefs=strict["assert_noun_plurdefs"] | auth["noun_plurdefs"],
        provided_infinitives=auth["provided_infinitives"],
        provided_presents=auth["provided_presents"],
        provided_noun_plurals=auth["provided_noun_plurals"],
        provided_noun_plurdefs=auth["provided_noun_plurdefs"],
    )
    if args.da_export is None or args.sv_export is None:
        print("note: da/sv tagger exports not given; word-level checks skipped")
    else:
        accepted = strict["accepted"]
        for word in sorted(STRICT_ACCEPTED_WORDS):
            if word not in accepted:
                failures += 1
                print(f"FAIL strict build dropped {word!r}")
        for word in sorted(STRICT_DROPPED_WORDS):
            if word in accepted:
                failures += 1
                print(f"FAIL strict build kept spec-violating {word!r}")
    if failures:
        print(f"{failures} strict-build verification failure(s)")
        return 1

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
    stats = strict["stats"]
    dropped = {k: v for k, v in stats.items() if k.startswith("dropped_")}
    report_lines = [
        "Nordum dictionary build",
        f"core words (wordlist + alternatives + additions): {len(core)} "
        f"(+{additions} additions)",
        "generated inflected forms (spec §4, dictionary.json paradigms):",
        f"  verb entries: {verb_entries}",
        f"  noun entries: {noun_entries}",
        f"  adjective entries: {adj_entries}",
        f"  generated form variants added: {generated}",
        "strict converted-source inventory (spec §4 morphology only, owner "
        "decision 2026-09-23):",
    ]
    report_lines += [f"  {key}: {value}"
                     for key, value in sorted(stats.items())
                     if not key.startswith("dropped_")]
    report_lines.append("  dropped spec-violating forms by category:")
    report_lines += [f"    {key}: {value}"
                     for key, value in sorted(dropped.items())]
    report_lines += [
        f"source words read: {stats['source_words_read']}",
        f"core variants written: {len(core_sorted)}",
        f"dictionary entries written: {len(all_sorted)}",
        f"sources: {', '.join(p.name for p in args.sources)}",
        "",
    ]
    if args.report:
        args.report.write_text("\n".join(report_lines), encoding="utf-8")
    print(f"wrote {args.out} ({len(all_sorted)} entries)")
    print(f"wrote {args.out_core} ({len(core_sorted)} entries)")
    for key in sorted(dropped):
        print(f"  dropped {key[len('dropped_'):]}: {dropped[key]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
