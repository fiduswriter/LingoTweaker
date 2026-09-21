# data/lt — Lithuanian

Module: `crates/lt/src/lt.rs` + `crates/lt/src/lt/`; tests:
`crates/lt/tests/lithuanian.rs`; gate: tests-only (`scripts/ci/parity.sh lt`).

## Contents

| Path | Source | License |
|---|---|---|
| `rules/grammar.xml` | upstream LanguageTool `lt` module | LGPL-2.1-or-later (LanguageTool resource) |
| `hunspell/lt_LT.aff`, `hunspell/lt_LT.dic` | ispell-lt 1.3.2, vendored from [LibreOffice/dictionaries `lt_LT`](https://github.com/LibreOffice/dictionaries/tree/master/lt_LT) at `8c45ec68d6b0346467c7ee23a6901139d129e468` | BSD-3-Clause (© 2000–2020 Albertas Agejevas and contributors) |
| `hunspell/README_lt_LT.txt`, `COPYING_lt_LT.txt`, `AUTHORS_lt_LT.txt` | same source (renamed from `README`, `COPYING`, `AUTHORS`) | BSD-3-Clause |

There is no upstream `lt_LT` dictionary: `MorfologikLithuanianSpellerRule`
references `/lt/hunspell/lt_LT.dict`, which is not shipped in the pinned
checkout or any pinned Maven artifact, so the legacy engine throws on every
check. By owner request we vendor a third-party dictionary ourselves and keep
the legacy rule id `MORFOLOGIK_RULE_LT_LT`; the behaviour has **no Java
baseline** and is pinned by our own tests (`docs/differences.md` #12).

## Modifying this language

- **Wiring**: `Lang::Lt` (`lt-LT`); `Lithuanian.createDefaultTagger()` is the
  `DemoTagger` (every token untagged), so the pipeline uses `surface_sentence`
  like `sl`/`is`. No disambiguator override (base no-op), no synthesizer, no
  tokenizer override. The 4 active XML rules plus the generic built-ins
  (including `GenericUnpairedBracketsRule`) carry the
  `MessagesBundle_lt.properties` strings.
- **Speller**: the rule id stays `MORFOLOGIK_RULE_LT_LT` even though the
  vendored dictionary is a Hunspell `.aff`/`.dic` pair, so the ported native
  Hunspell speller is the authority. The `.aff` uses `SET UTF-8`, the default
  single-character `FLAG` mode and only `PFX`/`SFX` plus `REP`/`MAP` (no
  compound directives) — all supported by `lt-spell`. Suggestions come from
  the ported native hunspell `suggest()` (`native_suggestions: true`) and are
  capped at five (`cap_native_suggestions: true`). The source ships no
  ignore/spelling lists, so none are vendored.
- **Regenerating the dictionary**: `data/manifest.json` records the exact
  source and sha256. Re-vendor with
  `python3 tools/lt-sync/lt_sync.py vendor-external --dir <dir>` where `<dir>/lt`
  holds the raw source files (`lt.aff`, `lt.dic`, `README`, `COPYING`,
  `AUTHORS`); the mapping is `EXTERNAL_DICTIONARIES["lt"]` in
  `tools/lt-sync/lt_sync.py`. To bump the source, update the pinned `commit`
  (and `version`) there and in this file and the manifest.
- **Gotchas**: `lt` has no Java corpus oracle, so `scripts/ci/parity.sh lt`
  only runs `cargo test -p lt --test lithuanian` and an `lt-cli inventory`
  sanity check. When adding tests, use real Lithuanian orthography
  (ą/č/ę/ė/į/š/ų/ū/ž) and assert offsets in Java UTF-16 code units via
  `crates/lt/tests/common/mod.rs`.
