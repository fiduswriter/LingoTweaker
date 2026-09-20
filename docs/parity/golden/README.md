# Golden corpus gate (CI, no Docker)

Pinned Java `CheckDump` output for the full per-language example corpora,
committed so CI can verify parity without Docker. The Java side is captured
from the pinned checkout (`01d07e1f6165`, `maven:3.9-eclipse-temurin-21`)
with the same scripts as the interactive oracles; `scripts/ci/parity.sh`
re-runs the Rust side and diffs against the golden.

| language | input (lines) | Java golden |
|---|---|---|
| en | `en-full.txt` (23,818) | `en-full.java.tsv` |
| de | `de-full.txt` (11,591) | `de-full.java.tsv` |
| es | `es-full.txt` (6,944) | `es-full.java.tsv` |
| fr | `fr-full.txt` (17,887) | `fr-full.java.tsv` |
| it | `it-full.txt` (275) | `it-full.java.tsv` |
| pt | `pt-full.txt` (9,151) | `pt-full.java.tsv` |
| nl | `nl-full.txt` (7,264) | `nl-full.java.tsv` |
| ca | `ca-full.txt` (25,626) | `ca-full.java.tsv` |
| gl | `gl-full.txt` (717) | `gl-full.java.tsv` |
| ro | `ro-full.txt` (1,154) | `ro-full.java.tsv` |

- Inputs are the exact corpus extractions: en = incorrect, non-trigger
  examples of `en-examples.jsonl`; de/es/fr/pt/gl/ro = all example texts
  (embedded newlines replaced by spaces; gl excludes the parallel-corpus
  `bitext.xml`, which the engine does not load as a rule file). pt runs
  Java/the Rust CLI on the default variant (pt-PT); the corpus also carries
  pt-BR/pt-AO/pt-MZ rule examples whose rules are inactive on both sides.
- The TSVs are `CheckDump.java` line format (`L`/`M` records, UTF-16
  offsets), as produced by `scripts/oracle/check-diff*.sh`.
- The date filters are pinned in `scripts/ci/parity.sh`: en/de/es use
  `2026-09-18` (the date at which those goldens reproduce; the Rust corpus
  runs passed with the container clock on 2026-09-16/18), fr/it/pt/nl use
  their capture date `2026-09-19`, and ca/gl/ro use `2026-09-20`.
- Expected state: de/es/it/nl/ca/ro exactly 0 only-Java / 0 only-Rust / 0 field
  diffs;
  en has exactly one documented field diff
  (`ADVERB_VERB_ADVERB_REPETITION`, `docs/differences.md` #1); fr has exactly
  the documented deliberate divergences `FRENCH_WORD_REPEAT_RULE` = 7
  only-Java (#3), `SUJET_AUXILIAIRE` = 1 only-Java (#4) and
  `AGREEMENT_PARTICULAR` = 1 field diff (#5); pt has exactly the documented
  `PODER_SER_POSSIVEL` = 1 field diff (#6). gl has 0 only-Java / 0 only-Rust
  and exactly the documented `HUNSPELL_RULE` = 83 field diffs (#7: the match
  set is identical, only the speller suggestions come from the bounded search
  instead of the unported native `hunspell.suggest` ranking). All allowances
  are encoded with
  `--expect-field-diffs`/`--expect-only-java` in the gate and validated
  exactly (fewer *or* more diffs than expected fail). The CI `parity` matrix
  runs the affected languages, not a static list (all ten on shared
  changes, one language on language-local changes, none for docs-only
  changes; P6.3/D-134). nl is exactly 0/0/0 (D-133); ca is exactly 0/0/0
  (D-188…D-190); gl is 0/0/83 (D-192…D-198); ro is exactly 0/0/0 (D-199).

Regenerate one language after an intentional corpus change (Docker):

```sh
scripts/ci/update-golden.sh en|de|es|fr|it|pt|nl|ca|gl|ro   # rewrites <lang>-full.java.tsv
```

If the capture date differs from `PARITY_TODAY`, update the pin in
`scripts/ci/parity.sh` (and this README) in the same commit.
