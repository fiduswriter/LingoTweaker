# Golden corpus gate (CI, no Docker)

Pinned Java `CheckDump` output for the full per-language example corpora,
committed so CI can verify parity without Docker. The Java side is captured
from the pinned checkout (`7bd1f99b849b`, `maven:3.9-eclipse-temurin-21`)
with the same scripts as the interactive oracles; `scripts/ci/parity.sh`
re-runs the Rust side and diffs against the golden.

| language | input (lines) | Java golden |
|---|---|---|
| en | `en-full.txt` (23,818) | `en-full.java.tsv` |
| de | `de-full.txt` (11,591) | `de-full.java.tsv` |
| es | `es-full.txt` (7,056) | `es-full.java.tsv` |
| fr | `fr-full.txt` (17,887) | `fr-full.java.tsv` |
| it | `it-full.txt` (275) | `it-full.java.tsv` |
| pt | `pt-full.txt` (9,151) | `pt-full.java.tsv` |
| nl | `nl-full.txt` (7,264) | `nl-full.java.tsv` |
| ca | `ca-full.txt` (25,667) | `ca-full.java.tsv` |
| gl | `gl-full.txt` (717) | `gl-full.java.tsv` |
| ro | `ro-full.txt` (1,154) | `ro-full.java.tsv` |
| pl | `pl-full.txt` (6,438) | `pl-full.java.tsv` |
| sk | `sk-full.txt` (352) | `sk-full.java.tsv` |
| sl | `sl-full.txt` (104) | `sl-full.java.tsv` |
| el | `el-full.txt` (118) | `el-full.java.tsv` |
| da | `da-full.txt` (284) | `da-full.java.tsv` |
| sv | `sv-full.txt` (45) | `sv-full.java.tsv` |
| is | `is-full.txt` (45) | `is-full.java.tsv` |
| eo | `eo-full.txt` (876) | `eo-full.java.tsv` |
| ast | `ast-full.txt` (125) | `ast-full.java.tsv` |

- Inputs are the exact corpus extractions: en = incorrect, non-trigger
  examples of `en-examples.jsonl`; de/es/fr/pt/gl/ro = all example texts
  (embedded newlines replaced by spaces; gl excludes the parallel-corpus
  `bitext.xml`, which the engine does not load as a rule file). pt runs
  Java/the Rust CLI on the default variant (pt-PT); the corpus also carries
  pt-BR/pt-AO/pt-MZ rule examples whose rules are inactive on both sides.
- The TSVs are `CheckDump.java` line format (`L`/`M` records, UTF-16
  offsets), as produced by `scripts/oracle/check-diff*.sh`.
- The date filters are pinned in `scripts/ci/parity.sh`: en/de use
  `2026-09-18` (the date at which those goldens reproduce; the Rust corpus
  runs passed with the container clock on 2026-09-16/18), fr/it/pt/nl use
  their capture date `2026-09-19`, and es/ca/gl/ro/pl/pt use `2026-09-20`,
  sk/sl/el/da/sv/is/eo/ast `2026-09-21`.
- Expected state: de/it/nl/ca/ro exactly 0 only-Java / 0 only-Rust / 0 field
  diffs;
  en has exactly one documented field diff
  (`ADVERB_VERB_ADVERB_REPETITION`, `docs/differences.md` #1); fr has exactly
  the documented deliberate divergences `FRENCH_WORD_REPEAT_RULE` = 7
  only-Java (#3), `SUJET_AUXILIAIRE` = 1 only-Java (#4) and
  `AGREEMENT_PARTICULAR` = 1 field diff (#5); pt has exactly the documented
  `PODER_SER_POSSIVEL` = 1 field diff (#6). gl has 0 only-Java / 0 only-Rust
  and exactly the documented `HUNSPELL_RULE` = 2 field diffs (#7: the match
  set is identical; the native hunspell suggestion engine including the
  n-gram fallback is ported, the residue is the upstream wall-clock timer
  boundary). es has 0
  only-Java / 0 field diffs and exactly the documented
  `AGREEMENT_DEMONSTRATIVE_VERB` = 11 only-Rust matches (#8: the 11 incorrect
  examples of the hand-authored demonstrative-verb rule, which Java does not
  report). All allowances
  are encoded with
  `--expect-field-diffs`/`--expect-only-java`/`--expect-only-rust` in the gate
  and validated
  exactly (fewer *or* more diffs than expected fail). The CI `parity` matrix
  runs the affected languages, not a static list (all ten on shared
  changes, one language on language-local changes, none for docs-only
  changes; P6.3/D-134). nl is exactly 0/0/0 (D-133); ca is exactly 0/0/0
  (D-188…D-190); gl is 0/0/2 (D-192…D-198 + D-224, reduced by the suggestion port); ro is exactly 0/0/0 (D-199);
  pl is 4 only-Java / 5 only-Rust / 0 field diffs,   the documented known
  fidelity gaps of `docs/differences.md` #9 (the `<unify negate="yes">`
  agreement rules, the ZDANIA_ZLOZONE comp:comma disambiguation context and
  the PCON_VERB participle rule), pinned exactly with
  `--expect-only-java`/`--expect-only-rust` in `scripts/ci/parity.sh`.
  sk is exactly 0/0/0 (D-212); the sk input excludes the parallel/unreferenced
  `grammar-nezaradene.xml` (not loaded by `Slovak.getRuleFileNames`).
  sl is exactly 0/0/0 (D-213; Slovenian has no tagger/synthesizer/
  disambiguator). el is exactly 0/0/0 (D-214; the Greek tagger, synthesizer
  and `el/disambiguation.xml` are wired, and `Pipeline::synthesizer()` gained
  the Greek branch so `<match postag>` synthesis renders like Java).
  da is exactly 0/0/0 (D-215/D-216 + D-224): the native hunspell suggestion
  engine including the n-gram fallback and the dotted-abbreviation
  suppression are ported; the XML rules, tagger and disambiguator are at
  parity.
  sv is exactly 0 only-Java / 0 only-Rust / 0 field diffs (D-218/D-219,
  resolved by the suggestion-engine port): the suggestion lists match the
  legacy engine byte-for-byte; the XML rules,
  `SwedishTagger`/`SwedishSynthesizer`, hybrid disambiguator, `SV_COMPOUNDS`
  and `SV_WORD_COHERENCY` are at parity.
  is is exactly 0/0/0 (D-228; Icelandic has no tagger/synthesizer/
  disambiguator, `HunspellNoSuggestionRule` emits no suggestions and
  `compile_failures()` is empty).
  eo is exactly 0/0/0 (D-229; the `HunspellRule` wrong-split check and the
  `twowords` UTF-8 split indexing are ported, so the `BREAK`-based
  space/hyphen recombinations match; `docs/differences.md` #10 resolved).
  ast is exactly 0/0/0 (D-230; Asturian has no disambiguator/synthesizer, the
  `AsturianTagger` reads the old CFSA (`0xc5`) Morfologik dictionary — the
  `lt-tagger` CFSA reader is new — and `compile_failures()` is empty).

Regenerate one language after an intentional corpus change (Docker):

```sh
scripts/ci/update-golden.sh en|de|es|fr|it|pt|nl|ca|gl|ro|pl|sk|sl|el|da|sv|is|eo|ast   # rewrites <lang>-full.java.tsv
```

If the capture date differs from `PARITY_TODAY`, update the pin in
`scripts/ci/parity.sh` (and this README) in the same commit.
