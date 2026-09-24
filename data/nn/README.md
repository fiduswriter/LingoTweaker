# data/nn — Norwegian Nynorsk

Hand-authored language data (the legacy engine has no Nynorsk module; it
ships only a spell-check-only dynamic language for Norwegian). ISO 639-1
code `nn` (alias `nno`). Module: `crates/lt/src/nn.rs` + `crates/lt/src/nn/`;
tests: `crates/lt/tests/nynorsk.rs`. The closest relative in this repository
is the Norwegian Bokmål module `data/no/` — the two written standards share
most grammar and vocabulary, so the rule set mirrors it where the norms
coincide and diverges where they do not (særskrift/ endings, `ikkje`,
question words in `kv-`, possessive placement, …).

## Contents

| Path | Source | License |
|---|---|---|
| `hunspell/nn_NO.dic` | LibreOffice dictionaries `no/` (v3.0; full forms from Ordbanken, Nasjonalbiblioteket/Språkbanken, plus the 2018–2024 Nynorskordboka nyordsliste) | CC BY 4.0 (Nasjonalbiblioteket/Ordbanken) + CLARIN PUB+BY (nyordsliste 2018–2024, CLARINO Bergen; attribution) |
| `hunspell/nn_NO.aff` | LibreOffice dictionaries `no/` (spell-norwegian affixes, recoded to UTF-8) | GPL-2.0 (spell-norwegian project) |
| `hunspell/ignore.txt`, `hunspell/repetition_exceptions.txt` | hand-authored | LGPL-2.1-or-later |
| `rules/grammar.xml` | hand-authored XML rules (Språkrådet-guided) | LGPL-2.1-or-later |
| `rules/bokmaal_forms.txt`, `rules/word_division.txt`, `rules/typos.txt`, `rules/check_case.txt` | hand-authored list rules; every pair is verified against `hunspell/nn_NO.dic` with `tools/nn-dict/check_pairs.py` | LGPL-2.1-or-later |

The same LibreOffice package also provides the already-vendored Bokmål
dictionary (`data/no/hunspell/nb_NO.*`); the provenance README is
<https://github.com/LibreOffice/dictionaries/blob/master/no/README_NO.txt>.
`nn_NO.dic` is a **full-form list**: entries carry no affix flags (all
540,664 inflected forms are enumerated), so dictionary membership is a plain
word lookup and the `tools/nn-dict/` checks need no affix expansion. The
`.aff` file (only `SET`, `TRY`, `COMPOUNDMIN`, `COMPOUNDFLAG`, `PFX`, `SFX`)
is loaded unmodified — no directives had to be stripped.

## Modifying this language

There is **no tagger and no POS dictionary** for Nynorsk: everything works on
the surface token stream, exactly as in the Bokmål module
(`data/no/README.md` — the rule kinds, the checklist and the gotchas there
apply here verbatim).

### Rule kinds

1. **XML pattern rules** — `rules/grammar.xml` (`NN_*` ids; keep them stable).
   The og/å confusion rules, negation placement, V2 inversion, agreement and
   the hyphenation/comma rules mirror `data/no/rules/grammar.xml` with
   Nynorsk token sets and messages; every `<example>` word is verified
   against `nn_NO.dic` so correct sentences stay clean.
2. **List rules** — `rules/*.txt` + a `SimpleReplaceRule` entry in
   `crates/lt/src/nn/rules.rs`:
   - `NN_BOKMAAL_FORMS` (`bokmaal_forms.txt`) — the Bokmål-interference rule;
     see below.
   - `NN_WORD_DIVISION`, `NN_TYPOS` — særskriving and common misspellings.
   - `NN_CAPITALIZATION` (`check_case.txt`) — lists the *correct* lowercase
     forms (language names, months, Nynorsk day names); any other casing is
     flagged except at sentence start.
3. **Speller word lists** — `hunspell/ignore.txt` (accepted words missing
   from `nn_NO.dic`), `hunspell/repetition_exceptions.txt` (legitimate
   repetitions).
4. **Shared Rust built-ins** — `NN_WORD_REPETITION` (word repetition), wired
   in `crates/lt/src/pipeline.rs`. The Bokmål-specific context heuristics
   (`NB_EN_ET_GENDER`, `NB_SIN_HANS`, …) have no Nynorsk counterpart yet; the
   Nynorsk gender system (hankjønn/hunkjønn/intetkjønn, `-a` feminines)
   needs more than a surface heuristic (deferred).

### The Bokmål-interference rule (`NN_BOKMAAL_FORMS`)

The most Nynorsk-specific check: it flags common **Bokmål-only forms** in
Nynorsk text and suggests the Nynorsk equivalent (`jeg→eg`, `ikke→ikkje`,
`hvordan→korleis`, `gutter→gutar`, …). It is a curated list rule, derived
offline from the two speller dictionaries:

```sh
# candidates: accepted by nb_NO.dic, rejected by nn_NO.dic
python3 tools/nn-dict/derive_bokmaal_forms.py \
    --nb data/no/hunspell/nb_NO.dic --nn data/nn/hunspell/nn_NO.dic
# verify a curated list: wrong form must NOT be valid Nynorsk, every
# suggestion must be
python3 tools/nn-dict/check_pairs.py data/nn/rules/bokmaal_forms.txt
```

Known limitations (all deliberate, false positives on valid Nynorsk are
worse than misses):

- **Only unambiguous forms are listed.** Many Bokmål-looking words are valid
  Nynorsk variants accepted by `nn_NO.dic` (`også`, `hun`, `dra`, `være`,
  `ble`, `derfor`, `verden`, `arbeider`, `biler`, …) and must not be flagged.
- **The rule is token-exact**: inflected Bokmål forms that are not in the
  list are only caught by the speller (`NN_SPELLER` flags them as unknown
  words), not with a targeted Nynorsk suggestion.
- Because every listed form is rejected by the Nynorsk dictionary, the rule
  never competes with the speller on valid words; for the overlapping range
  the list rule wins via the priority table
  (`crates/lt/src/nn/priorities.rs`: speller `-1000`, style `-50`, rest 0).

## Regenerating / re-verifying the dictionary

```sh
curl -sL -o nn_NO.aff https://raw.githubusercontent.com/LibreOffice/dictionaries/master/no/nn_NO.aff
curl -sL -o nn_NO.dic https://raw.githubusercontent.com/LibreOffice/dictionaries/master/no/nn_NO.dic
cp nn_NO.aff data/nn/hunspell/nn_NO.aff
cp nn_NO.dic data/nn/hunspell/nn_NO.dic
python3 tools/lt-sync/lt_sync.py add-local vendor data/nn/hunspell/nn_NO.dic \
    --license "CC BY 4.0" --url https://github.com/LibreOffice/dictionaries/tree/master/no
python3 tools/lt-sync/lt_sync.py add-local vendor data/nn/hunspell/nn_NO.aff \
    --license "GPL-2.0" --url https://github.com/LibreOffice/dictionaries/tree/master/no
cargo test -p lt-data
```

## Registering data changes

```sh
python3 tools/lt-sync/lt_sync.py add-local hand-authored <changed files> \
    --license "LGPL-2.1-or-later" --license-source "LICENSE"
cargo test -p lt-data     # sha256/size verification must pass
```

Tests: `cargo test -p lt --test nynorsk`.
