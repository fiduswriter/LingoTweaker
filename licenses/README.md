# Licenses and provenance

The project license is the root [`LICENSE`](../LICENSE) (**LGPL-2.1-or-later**).
The component-by-component table — including which licenses are confirmed and
which are still to be confirmed by the owner — is
[`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md). The machine-readable
record is the per-file `source.license` / `source.license_verified` /
`source.license_source` in [`data/manifest.json`](../data/manifest.json); the
data generation workflow is documented in [`data/README.md`](../data/README.md).

## Port and glue code

All Rust code in this repository is licensed under the **GNU Lesser General Public
License v2.1 or later** (`LICENSE`), matching LanguageTool's license. The workspace
`license` field in `Cargo.toml` declares `LGPL-2.1-or-later` and must be updated if
the owner ever changes the project license.

## Vendored upstream data (pinned via `upstream.json`)

- Source: <https://github.com/languagetool-org/languagetool>
- Pinned commit: `7bd1f99b849b1de66845113010b3c9261c3f37e0` (recorded in
  `upstream.json` and `data/manifest.json`; sha256/size per file).
- Default license: LGPL-2.1-or-later. Upstream's `pom.xml` notes that the license
  refers to the source code and that resources may be under different licenses, so
  each vendored file carries its own `license`/`license_verified` record; the
  exceptions are listed in `THIRD_PARTY_NOTICES.md`.

### German hunspell dictionaries (`data/de/hunspell/`)

Vendored from the pinned checkout's
`languagetool-language-modules/de/src/main/resources/org/languagetool/resource/de/hunspell/`
(sha256/size in `data/manifest.json`). These are the raw hunspell files that
`GermanSpellerRule` loads through the native libhunspell binding at runtime (the
morfologik `.dict` files are used only for suggestions).

- Provenance: igerman98 dictionary by Björn Jacke, extended by Franz Michael
  Baumann ("frami"); see `de_DE_frami_README.txt`, `de_DE.README` and `de_DE.info`
  in the same directory.
- License: **GPL-2.0-or-later OR GPL-3.0-or-later** (the upstream
  `de_DE_frami_README.txt` states "lizenziert unter der GNU GPL, Version 2 oder 3";
  `COPYING_GPLv2.txt`/`COPYING_GPLv3.txt` are vendored next to them). Unlike the
  LT-authored word lists, these files are not LGPL. The manifest records the raw
  files as verified and the derived `.dict`/`.info` conversions as unverified.
- The `.dic.header` files in upstream are build artifacts of `create_dict.sh` and
  are not used at runtime, so they are not vendored.

## Maven artifacts (pinned in tools/lt-sync/lt_sync.py)

Licenses below are the artifacts' own declarations; the POM URLs are recorded per
file in the manifest (`license_source`). See `THIRD_PARTY_NOTICES.md` for the
"to be confirmed" items.

| artifact | coordinates | license | evidence | confirmed |
|----------|-------------|---------|----------|-----------|
| english-pos-dict | org.languagetool:english-pos-dict:0.6 | LGPL-2.1-only | artifact POM | yes |
| german-pos-dict | de.danielnaber:german-pos-dict:1.2.4 | CC-BY-SA-4.0 (Morphy-derived) | artifact POM + upstream `resource/de/README.txt` | yes (owner review advised) |
| spanish-pos-dict | org.softcatala:spanish-pos-dict:2.5 | LGPL-2.1-only | artifact POM | yes |
| french-pos-dict | org.languagetool:french-pos-dict:0.7 | POM CC-BY-SA-4.0; bundled Dicollecte docs MPL-2.0 | artifact POM + bundled READMEs | no (conflict) |
| portuguese-pos-dict | org.languagetool:portuguese-pos-dict:1.2.0 | LGPL-2.1-only | artifact POM | yes |
| dutch-pos-dict | org.languagetool:dutch-pos-dict:0.1 | README CC-BY-3.0-or-later OR BSD; POM LGPL-2.1 | bundled READMEs + artifact POM | no (conflict) |
| opennlp-tokenize/postag/chunk-models | edu.washington.cs.knowitall:opennlp-*-models:1.5 | Apache-2.0 | artifact POMs | yes |
| jwordsplitter | de.danielnaber:jwordsplitter:4.7 | Apache-2.0 | artifact POM | yes (data files only) |

Rust-level in-tree dictionaries and word-list provenance notes (nl, it, en, fr)
are recorded per file in `data/manifest.json` via the same fields.

## Third-party code ported into this repository

| upstream | version | license | ported in | verified |
|----------|---------|---------|-----------|----------|
| edu.washington.cs.knowitall:openregex | 1.1.1 | LGPL-2.1-or-later (Maven POM `<license>`; sources jar sha256 `aacc5c8b962b78fa2a5b758daa7403313af006afb6e1c6f9e02dd42538190157`, binary jar `33021c9cca70c6292d53ff7dac5d1832d422e986aba52ec998532a5f47f921f8`) | `crates/lt-chunk/src/openregex.rs` (parser/NFA subset used by `GermanChunker`) | yes (D-027) |

## Vendored license texts

Canonical license texts are stored next to this README (retrieved **2026-09-20**
with `curl` from the publishers below; no license text is reproduced from
memory). They cover the third-party licenses of the vendored dictionaries and
data.

| File | License | Canonical URL | Retrieved |
|------|---------|---------------|-----------|
| `COPYING.LGPL-2.1` | LGPL-2.1 (project license) | already present at HEAD (project/LanguageTool `COPYING.txt`); not re-fetched | — |
| `COPYING.GPL-2.0` | GPL-2.0 (nb_NO.aff; da_DK tri-license option) | <https://www.gnu.org/licenses/old-licenses/gpl-2.0.txt> | 2026-09-20 |
| `COPYING.LGPL-3.0` | LGPL-3.0 (sv_SE dictionary word list, build input) | <https://www.gnu.org/licenses/lgpl-3.0.txt> | 2026-09-20 |
| `COPYING.GFDL-1.2` | GFDL-1.2 (gug.aff/gug.dic) | <https://www.gnu.org/licenses/old-licenses/fdl-1.2.txt> | 2026-09-20 |
| `MPL-1.1.txt` | MPL-1.1 (da_DK tri-license option) | <https://www.mozilla.org/media/MPL/1.1/index.txt> | 2026-09-20 |
| `CC-BY-4.0.txt` | CC BY 4.0 (nb_NO.dic, Nordum data) | <https://creativecommons.org/licenses/by/4.0/legalcode.txt> | 2026-09-20 |
| `CC-BY-SA-4.0.txt` | CC BY-SA 4.0 (Wikipedia typo import) | <https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt> | 2026-09-20 |

The CLARIN PUB+BY terms (nyordslister 2018–2024) are a data-distribution
license; no text is vendored here. The terms are linked from
[`data/no/README.md`](../data/no/README.md) and `THIRD_PARTY_NOTICES.md`
(Persistent identifier <http://urn.fi/urn:nbn:fi:lb-2019071721>).

## Distribution model (P5.1)

Default: dynamically linked core + source/object offer in releases, pending legal
review before publishing wheels/add-ons (plan §11/§12). The notices and per-file
license records required by that model are in place (`LICENSE`,
`THIRD_PARTY_NOTICES.md`, `data/README.md`, `data/manifest.json`).

**Owner sign-off (2026-09-23):** the owner approved the distribution model for
the reviewed components — the copyleft and CC-BY-SA dictionaries/data are
shipped as separately licensed data components (aggregation) alongside the
LGPL-2.1-or-later engine. Cleared: `km` (CC-BY-NC-SA-3.0 POS component +
GPL-3.0 SBBIC speller), `ml` (GPL data), `ta` (GPLv3), `be` (CC-BY-SA-4.0),
`el` (CC-BY-SA-4.0 analyzer + GPL/LGPL/MPL hunspell), `de` (CC-BY-SA-4.0 POS +
GPL hunspell) and the German/Greek/Italian hunspell "related items"; the
`no`/`nrd`/`gn` items were recorded earlier (D-163/D-164). Still open (not
covered): the conflicting-statement `fr`/`nl` POS dictionaries, the Italian
Morph-it! dual license, the undocumented `en` hunspell/word-list provenance and
the `ar`/`br`/`ca`/`gl` entries. See `THIRD_PARTY_NOTICES.md`.
