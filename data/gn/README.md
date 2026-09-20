# data/gn — Guaraní (Paraguayan)

Hand-authored language (upstream LanguageTool has no Guaraní module). Module:
`crates/lt/src/gn.rs` + `crates/lt/src/gn/`; tests:
`crates/lt/tests/guarani.rs`; the owner-approved rule list is the
`new-languages/guarani/proposed-rules.md` sign-off package (outside this repo
until the staging folder is retired). Constructed examples there still need
owner confirmation before public release.

## Contents

| Path | Source | License |
|---|---|---|
| `hunspell/gug.dic`, `hunspell/gug.aff` | LibreOffice dictionaries `gug/` (huracán, 2016, unmaintained) | GFDL-1.2-or-later (stated in the package's thesaurus README; the dictionary files carry no license header — evidence gap, see `docs/licensing/review-2026-09.md`) |
| `hunspell/ignore.txt` | hand-authored core vocabulary the flat `gug.dic` lacks (it has no affix morphology) and accepted inflected forms | LGPL-2.1-or-later |
| `hunspell/repetition_exceptions.txt` | hand-authored | LGPL-2.1-or-later |
| `rules/grammar.xml` | hand-authored XML rules | LGPL-2.1-or-later |
| `rules/orthography.txt`, `postpositions.txt` | hand-authored list rules | LGPL-2.1-or-later |

References: ALG «Guaraní Ñe'ẽtekuaa» (official orthography; copyrighted, cite
only), Estigarribia's *A Grammar of Paraguayan Guarani* (open access), the
`gug` dictionary. The ALG is the normative source; do not copy its text.

## Modifying this language

No tagger. Three language-specific mechanisms matter:

1. **Puso-aware tokenizer** (`crates/lt/src/gn.rs::tokenize_guarani`): the base
   tokenizer splits on apostrophes, so the puso (`'`, `’`, `ʼ`) is fused back
   into the word *without changing the byte lengths* (offsets stay correct).
   Token text keeps the original character; the speller retries the
   ASCII-normalized form at lookup. **Never replace a multi-byte puso with a
   one-byte `'` in the tokenizer** — that would shift every following offset.
   ASCII `'` is the canonical character in suggestions (owner decision D13).
2. **`GN_ACCENTS`** (`crates/lt/src/gn/accents.rs`): builds an index of
   `gug.dic` + `ignore.txt` words with the diacritics stripped (acute accents,
   nasal tildes, the combining tilde of `g̃`; `ñ` and the puso stay distinct).
   A token that is not a known word but matches a known word after stripping
   gets the dictionary spelling as suggestion (`ara` → `ára`, `mokói` →
   `mokõi`). It runs at priority `-1` in `crates/lt/src/gn/priorities.rs`:
   curated list rules win over it, it wins over the speller.
3. **`GN_HARMONY`** (`crates/lt/src/gn/context.rs`): the dictionary-driven
   nasal-harmony rule. From the same two word sources (`gug.dic` +
   `ignore.txt`) it indexes the oral/nasal swapped form of every known word
   for the productive alternations: suffix `-pe`/`-me`, `-pa`/`-mba`,
   `-kuéra`/`-nguéra`; prefix `jo-`/`ño-`, `ja-`/`ña-`; and the negation
   `nd-`/`n-` before `-i` (conservative: only `…-i` words are swapped).
   An unknown token whose swapped form is known gets the known form as
   suggestion (`ñúpe` → `ñúme`, `johetũ` → `ñohetũ`, `osẽpa` → `osẽmba`).
   Both the known word and the swapped variant need ≥ 4 characters, so short
   coincidental pairs (`upe`/`ume`, `ñai`/`jai`) stay with the speller. The
   rule has no ambiguity guard beyond the index (there are no keys with more
   than one known candidate in the current data), so a correct form missing
   from the word lists can still receive a harmony suggestion; add the word
   to `ignore.txt` instead of tuning the rule. Runs at priority `-1`, like
   `GN_ACCENTS`.

### Rule kinds

1. **XML pattern rules** — `rules/grammar.xml` (`GN_PE_ME` suggests the `-me`
   form with `regexp_replace`; `GN_NDE_NE` uses a nasal-stem list;
   `GN_PARTICLES` is message-only because joining shifts the accent;
   `GN_FOREIGN_QU`/`GN_FOREIGN_Z`/`GN_FOREIGN_C` adapt Spanish letter
   patterns and are **default-off**).
2. **List rules** — `rules/orthography.txt`, `postpositions.txt`,
   `jopara.txt` (**default-off** Spanish-connector style rule; suggestions
   are placeholders pending review), `check_case.txt` (lowercase `guarani`)
   + entries in `crates/lt/src/gn/rules.rs`. Curated nasal-harmony and
   negation examples stay here as the specific overrides
   (`ñee=ñe'ẽ`, `ñúpe=ñúme`, `ndojapoi=ndojapói`,
   `mitãkuéra=mitãnguéra`, …); `GN_HARMONY` generalizes them
   dictionary-driven and wins only when no curated entry matches.
3. **Speller word lists** — `hunspell/ignore.txt` is also `GN_ACCENTS`' second
   word source: adding a word there makes it accepted *and* accent-checked.
   The flat `gug.dic` lacks inflected/agglutinated forms, so add frequent
   valid forms here rather than accepting false positives. The file ends
   with 400 corpus-dominant forms extracted from Jojajovai (MIT); regenerate
   by re-running the extraction (see the staging `new-languages/` notes) and
   keep wrong forms targeted by the rules out of the list.
4. **Rust built-ins** — `GN_ACCENTS` (`crates/lt/src/gn/accents.rs`),
   `GN_HARMONY` (`crates/lt/src/gn/context.rs`) and the shared
   `WordRepetitionRule` (`GN_WORD_REPETITION`).

### Adding a rule: checklist

1. Add the rule/list entry; keep rule ids stable.
2. Extend `guarani_rules_fire` / `guarani_accents` / `guarani_harmony` /
   `guarani_harmony_priority` / `guarani_correct_sentences` in
   `crates/lt/tests/guarani.rs`; update `active_rule_count()` in
   `guarani_engine_state` (currently 3) for default-on XML rules, and the
   `GN_HARMONY` unit tests in `crates/lt/src/gn/context.rs` when the
   alternations or the minimum length change. The harmony index is derived
   from `gug.dic` + `ignore.txt`, so adding an accepted word extends it.
3. Register changed data (`lt-sync add-local hand-authored <paths>`, including
   this README), run tests and clippy.

### Notes / gotchas

- A blanket nasal-harmony rewrite is **not** safe without a nasal-feature
  lexicon: `johecha` (oral) is correct while `johetũ` must become `ñohetũ`.
  `GN_HARMONY` therefore only suggests forms that exist in the word lists and
  never rewrites blind; when both harmony forms are known nothing is reported.
  Residual risk: a valid word that is absent from `gug.dic`/`ignore.txt` can
  get the known opposite-harmony form as suggestion, so grow the word lists
  rather than loosening the index.
- `GN_ACCENTS` only suggests when the stripped token matches a dictionary
  word; if both `tupa` and `tupã` are valid words, nothing is flagged (correct
  behaviour).
- Suggestions use ASCII puso; keep dictionary/list entries ASCII (`'`) and let
  the speller normalize typographic input.

## Regenerating the vendored dictionary

```sh
curl -sL -o gug.dic https://raw.githubusercontent.com/LibreOffice/dictionaries/master/gug/gug.dic
curl -sL -o gug.aff https://raw.githubusercontent.com/LibreOffice/dictionaries/master/gug/gug.aff
python3 tools/lt-sync/lt_sync.py add-local vendor data/gn/hunspell/gug.dic data/gn/hunspell/gug.aff \
    --license "GFDL-1.2-or-later" --url https://github.com/LibreOffice/dictionaries/tree/master/gug
cargo test -p lt-data
```

Tests: `cargo test -p lt --test guarani`.
