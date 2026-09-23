# Golden corpus gate (CI, no Docker)

Pinned Java `CheckDump` output for the full per-language example corpora,
committed so CI can verify parity without Docker. The Java side is captured
from the pinned checkout (`7bd1f99b849b`, `maven:3.9-eclipse-temurin-21`)
with the same scripts as the interactive oracles; `scripts/ci/parity.sh`
re-runs the Rust side and diffs against the golden.

Parity policy (D-310, owner 2026-09-23): a divergence is fixed only where the
Java implementation is more correct; where the Rust implementation is more
correct it is kept and recorded as an intentional divergence. Every allowance
below is therefore either **residue to fix** (a Java-reference fidelity gap /
unported feature) or **intentional: Rust more correct** (Java's output is wrong
or an artifact), and `docs/differences.md` states the verdict per entry.

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
| br | `br-full.txt` (1,835) | `br-full.java.tsv` |
| tl | `tl-full.txt` (99) | `tl-full.java.tsv` |
| crh | `crh-full.txt` (98) | `crh-full.java.tsv` |
| be | `be-full.txt` (161) | `be-full.java.tsv` |
| ru | `ru-full.txt` (2,457) | `ru-full.java.tsv` |
| uk | `uk-full.txt` (4,437) | `uk-full.java.tsv` |
| sr | `sr-full.txt` (49) | `sr-full.java.tsv` |
| ar | `ar-full.txt` (1,045) | `ar-full.java.tsv` |
| fa | `fa-full.txt` (566) | `fa-full.java.tsv` |
| km | `km-full.txt` (66) | `km-full.java.tsv` |
| ml | `ml-full.txt` (38) | `ml-full.java.tsv` |
| ta | `ta-full.txt` (216) | `ta-full.java.tsv` |
| de-x-simple | `de-x-simple-full.txt` (238) | `de-x-simple-full.java.tsv` |

`lt` has no golden: the legacy module references a `lt_LT.dict` that is not
shipped, so the legacy engine throws on every check. The Rust engine vendors a
third-party ispell-lt dictionary under the unchanged `MORFOLOGIK_RULE_LT_LT`
id; there is no Java baseline, so `lt` runs the tests-only gate
(`docs/differences.md` #12).

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
  sk/sl/el/da/sv/is/eo/ast/br/tl/crh `2026-09-21`, and be/ru/uk/sr/ar `2026-09-22`,
and fa `2026-09-23` (Persian has no date-filter rules, so the value is inert),
and km `2026-09-23` (Khmer has no date-filter rules either), and ta
`2026-09-23` (Tamil has no date-filter rules either).
`de-x-simple` is the Simple German variant (`de-DE-x-simple-language`), gated
as `de-x-simple`; its input is the 238 `grammar.xml` example texts (the
variant shares the German disambiguation, so no German disambiguation
examples enter its corpus), captured `2026-09-23` (the variant has no
date-filter rules either).
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
  br is exactly 0/0/0 (D-236; `BretonTagger`, `BretonWordTokenizer` and the
  `br/disambiguation.xml` are wired, `compile_failures()` is empty, and the
  two shared gaps the corpus surfaced are fixed: `<match no="0"
  regexp_match=... regexp_replace=...>` token references in patterns and the
  multi-word `IGNORE_SPELLING` anti-patterns from the spelling word lists).
  tl is 0 only-Java / 0 only-Rust / exactly the documented
  `MORFOLOGIK_RULE_TL` = 5 suggestion-order field diffs (docs/differences.md
  #11; the frequency-included `tl_PH` dictionary orders the weighted
  candidates differently, the match and suggestion sets are identical).
  crh is 0 only-Rust / 0 field diffs and exactly the documented
  `COMPLEX_NUMBER_DEFIS_MISSING` = 1 only-Java match (docs/differences.md
  #13; Java's `UNICODE_CASE` folds `ı` into `[A-Za-z]`, Rust's simple case
  folding does not).
  ru is exactly 0/0/0 (D-247; the `RussianTagger` stress-mark/`MayMissingYO`
  handling, `RussianWordTokenizer`, hybrid disambiguator, `RussianChunker`,
  both Morfologik spellers, the six XML filter classes, the `RU_*` Java rules
  and `compile_failures()` is empty).
  uk is 3 only-Java / 1 only-Rust / 0 field diffs, the documented known
  fidelity gaps of `docs/differences.md` #14 (a prep+`не`+noun
  case-government gap, the `т. 2 ч. 1` abbreviation sentence segmentation and
  one plural-adjective/proper-name overlap tie-break; the XML disambiguation
  forward-scan cascade #14a is fixed), pinned exactly with
  `--expect-only-java`/`--expect-only-rust` in
  `scripts/ci/parity.sh` (D-273…D-281).
  sr is exactly 0/0/0 (D-288): the pinned `sr` module is excluded from the LT
  reactor and does not compile against any released core (D-285), so the
  golden is captured from a forward-ported in-container copy
  (`scripts/oracle/sr/sr-module-6.9.patch` + `check-diff-sr.sh`, D-287). The
  12 XML rules, the generic built-ins, the ekavian tagger/synthesizer, the
  hybrid disambiguator, the `MorfologikEkavianSpellerRule` and the two legacy
  replace rules match Java exactly.
  ar is 14 only-Java / 8 only-Rust / 0 field diffs, the documented known
  fidelity gaps of `docs/differences.md` #15 (the `syntax_numeric_0003`
  number-phrase rule, whose `ArabicNumbersWords` number-to-words engine is not
  ported so the filter rejects and the rule is inert, plus the two unported
  rule classes `AR_INFLECTED_ONE_WORD`/`AR_VERB_TRANSITIVE_IINDIRECT` and two
  Hunspell range/wrong-split residues), pinned exactly with
  `--expect-only-java`/`--expect-only-rust` in `scripts/ci/parity.sh` (D-293+).
  fa is exactly 0/0/0: the documented shared regex-semantics divergence of
  `docs/differences.md` #16 is resolved (Java's token `\w` is ASCII, the Rust
  `regex` crate's is Unicode; the shared `lt_pattern` translation now rewrites
  `\w`/`\W`/`\d`/`\D`/`\s`/`\S`/`\b`/`\B` to Java's ASCII definitions, which
  removed the 258 `Bad_ZWNJ` false positives).
  km is exactly 0/0/0: the documented speller divergence of
  `docs/differences.md` #17 is resolved (the `IGNORE ៗ` directive is now
  implemented in `lt-spell`, so the swapchar/extrachar candidates `ញប`/`មៃ`
  match the stored `ញបៗ`/`មៃៗ` entries like Java's hunspell).
  ml is exactly 0/0/0: the 18 active XML rules, the six generic built-ins and
  the `MORFOLOGIK_RULE_ML_IN` speller match the pinned Java module, captured
  from the unpatched checkout (`scripts/oracle/ml/check-diff-ml.sh`). The
  speller is inert for Malayalam script (`isLatinScript() = true`), exactly
  like Java.
  ta is exactly 0/0/0: the 210 active XML rules (including the rulegroup
  sub-rules), the `TamilTagger` (`ta/dictionaries/tamil.dict`) and the five
  `Tamil.getRelevantRules` generic built-ins (the ta-localized
  `CommaWhitespaceRule`, `DoublePunctuationRule`, `MultipleWhitespaceRule`,
  `LongSentenceRule(…, 50)` and `SentenceWhitespaceRule`) match the pinned
  Java module, captured from the unpatched checkout
  (`scripts/oracle/ta/check-diff-ta.sh`). Tamil has no speller, no
  disambiguator and no synthesizer; `compile_failures()` is empty.
  de-x-simple is exactly 0/0/0: the Simple German variant
  (`de-DE-x-simple-language`) reuses the German foundations
  (tagger/synthesizer/disambiguator/chunker) and runs only its 92 active
  `grammar.xml` rules, matching the pinned `SimpleGerman` module
  (`scripts/oracle/de-x-simple/check-diff-de-x-simple.sh`); the 12-word
  `TOO_LONG_SENTENCE_DE` is `tags="picky"` and does not run at the default
  level.

Regenerate one language after an intentional corpus change (Docker):

```sh
scripts/ci/update-golden.sh en|de|es|fr|it|pt|nl|ca|gl|ro|pl|sk|sl|el|da|sv|is|eo|ast|br|tl|crh|be|ru|uk|sr|ar|fa|km|ml|ta|de-x-simple   # rewrites <lang>-full.java.tsv
```

If the capture date differs from `PARITY_TODAY`, update the pin in
`scripts/ci/parity.sh` (and this README) in the same commit.
