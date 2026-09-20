# Third-party notices

LingoTweaker is an independent Rust port of the [LanguageTool](https://languagetool.org)
engine and rule data. It is **not affiliated with or endorsed by** the LanguageTool
project; the LanguageTool name and trademark belong to their owners. The `LT` / `lt-`
abbreviations (crate names, binaries, `LT_DATA_DIR`, `/v2` API paths) are kept
deliberately for upstream compatibility (plan locked decision 13).

The project's own port and glue code is licensed under the **GNU Lesser General
Public License v2.1 or later** (see [`LICENSE`](LICENSE)). Vendored data is *not*
relicensed: it keeps the license it has upstream. Every vendored file carries its
license in `data/manifest.json` (`source.license`, `source.license_verified`,
`source.license_source`); this page is the human-readable summary. Where upstream
does not document a license, the component below is marked **license to be
confirmed** instead of guessing.

## LanguageTool (upstream)

- Source: <https://github.com/languagetool-org/languagetool>
- Pinned commit: `01d07e1f61651d5ad7ea015cce36573314754ad8` (6.9-SNAPSHOT), see
  [`upstream.json`](upstream.json).
- License: the pinned `COPYING.txt` contains the LGPL-2.1 text; upstream
  `README.md` and `pom.xml` declare the "GNU Lesser General Public License"
  (LGPL). Upstream's `pom.xml` additionally notes: *"The license refers to the
  source code, resources may be under different licenses"* — which is why every
  resource below has its own record.
- Vendored under `data/core/**`, `data/schemas/**`, `data/messages/**`,
  `data/<lang>/rules/**`, `data/<lang>/disambiguation.xml`,
  `data/<lang>/words/**` and `data/<lang>/*.sor`: rule/style XML, disambiguation
  data, XSD schemas, message bundles, LT-authored word lists, serialized
  resources. Default manifest license:
  `LGPL-2.1-or-later (LanguageTool resource; per-file license varies)`,
  `license_verified=false`; the exceptions below are recorded explicitly.

## Components

"Confirmed" means the license is stated at the cited source; "to be confirmed"
means no upstream statement was found (or two statements conflict) and the owner
must decide. Data paths are relative to `data/`.

| Component | Vendored data | License | Evidence | Status |
|-----------|---------------|---------|----------|--------|
| LanguageTool core + rule resources | `core/**`, `schemas/**`, `messages/**`, `<lang>/rules/**`, `<lang>/disambiguation.xml`, `<lang>/*.sor`, LT-authored `<lang>/words/**` | LGPL-2.1-or-later | upstream `COPYING.txt`, `pom.xml` (`<licenses>`); resources may differ per file | Confirmed (default; per-file exceptions below) |
| English POS/spelling dictionaries (`org.languagetool:english-pos-dict:0.6`) | `en/dictionaries/*` | LGPL-2.1-only | [artifact POM](https://repo1.maven.org/maven2/org/languagetool/english-pos-dict/0.6/english-pos-dict-0.6.pom) (`<licenses>`, LGPL 2.1) | Confirmed |
| English hunspell dictionaries (inside the same artifact) | `en/hunspell/*.dict`, `en/hunspell/*.info` | artifact POM: LGPL-2.1-only; underlying hunspell data (SCOWL-derived) | no license note for the hunspell files in the LT checkout or artifact | **License to be confirmed** |
| English word-list provenance docs | `en/words/agid-readme.txt`, `en/words/pos-readme.txt` | AGID © Kevin Atkinson; Moby Part-of-Speech II / WordNet terms | the readmes state copyright, not a license | **License to be confirmed** |
| German POS dictionary (`de.danielnaber:german-pos-dict:1.2.4`) | `de/dictionaries/*` | CC-BY-SA-4.0 | [artifact POM](https://repo1.maven.org/maven2/de/danielnaber/german-pos-dict/1.2.4/german-pos-dict-1.2.4.pom) ("data originally based on Morphy"); upstream `resource/de/README.txt` (= `de/words/README.txt`) | Confirmed |
| German hunspell dictionary (igerman98, frami extension) | `de/hunspell/de_*.aff`, `de_*.dic`, `de_DE.README`, `de_*_frami_README.txt`, `COPYING_GPLv2/3.txt`, converted `de_*.dict`/`de_*.info` | GPL-2.0-or-later OR GPL-3.0-or-later | `de_DE_frami_README.txt` ("GNU GPL, Version 2 oder 3"), vendored `COPYING_GPLv2/3.txt` | Confirmed for raw/dictionary docs; derived `.dict`/`.info` tracked as unverified |
| jWordSplitter compound word lists (`de.danielnaber:jwordsplitter:4.7`) | `de/compound/*.txt` | Apache-2.0 (Sven Abels / Daniel Naber) | [artifact POM](https://repo1.maven.org/maven2/de/danielnaber/jwordsplitter/4.7/jwordsplitter-4.7.pom) | Confirmed |
| Spanish POS dictionary (`org.softcatala:spanish-pos-dict:2.5`) | `es/dictionaries/*` | LGPL-2.1-only | [artifact POM](https://repo1.maven.org/maven2/org/softcatala/spanish-pos-dict/2.5/spanish-pos-dict-2.5.pom) | Confirmed |
| French POS dictionary (`org.languagetool:french-pos-dict:0.7`) | `fr/dictionaries/*` | POM: CC-BY-SA-4.0; bundled Dicollecte docs: MPL-2.0 | [artifact POM](https://repo1.maven.org/maven2/org/languagetool/french-pos-dict/0.7/french-pos-dict-0.7.pom); `fr/words/README_lexique.txt`, `fr/hunspell/README_fr.txt` | **License to be confirmed** (conflicting statements) |
| Dicollecte lexicon documentation | `fr/words/README_lexique.txt`, `fr/hunspell/README_fr.txt` | MPL-2.0 | the readmes state "MPL : Mozilla Public License version 2.0" | Confirmed (docs) |
| Italian POS/synthesis dictionaries (Morph-it!) | `it/dictionaries/*` | dual: CC-BY-SA-2.0 OR LGPL | upstream `resource/it/README.txt`, `resource/it/tagset.txt` ("LICENSING INFORMATION") | **License to be confirmed** (dual choice / build artifact) |
| Italian tagset | `it/words/tagset.txt` | dual: CC-BY-SA-2.0 OR LGPL | "LICENSING INFORMATION" section in the file (Morph-it!) | Confirmed (dual) |
| Italian hunspell dictionary | `it/hunspell/it_IT.dict`, `it_IT.info`, `README_it_IT.txt` | GPL-3.0-only | `README_it_IT.txt` ("License: GNU GPL 3", © 2010/2011 Andrea Pescetti) | Confirmed for the README; converted `.dict`/`.info` tracked as unverified |
| Portuguese POS dictionary + speller dictionaries (`org.languagetool:portuguese-pos-dict:1.2.0`) | `pt/dictionaries/*`, `pt/spelling/*` | LGPL-2.1-only | [artifact POM](https://repo1.maven.org/maven2/org/languagetool/portuguese-pos-dict/1.2.0/portuguese-pos-dict-1.2.0.pom) | Confirmed |
| Dutch POS dictionary (`org.languagetool:dutch-pos-dict:0.1`) | `nl/dictionaries/*` | README: CC-BY-3.0-or-later OR BSD (TaalTik); artifact POM: LGPL-2.1 | bundled `nl/dictionaries/README.txt`; [artifact POM](https://repo1.maven.org/maven2/org/languagetool/dutch-pos-dict/0.1/dutch-pos-dict-0.1.pom) | **License to be confirmed** (conflicting statements) |
| Dutch speller dictionary (TaalTik) | `nl/spelling/nl_NL.dict`, `nl_NL.info`, `README.txt` | LGPL-2.1-or-later | bundled `nl/spelling/README.txt` (Ruud Baars / TaalTik) | Confirmed |
| Norwegian Bokmål Hunspell dictionary | `no/hunspell/nb_NO.dic` | CC BY 4.0 (Nasjonalbiblioteket/Ordbanken) + CLARIN PUB+BY (nyordslister 2018–2024, CLARINO Bergen) | [LibreOffice dictionaries `no/README_NO.txt`](https://github.com/LibreOffice/dictionaries/blob/master/no/README_NO.txt) (v3.0) | Confirmed |
| Norwegian Bokmål affix rules | `no/hunspell/nb_NO.aff` | GPL-2.0 (spell-norwegian project) | same README and `no/COPYING`; `CHECKCOMPOUNDTRIPLE`/`SIMPLIFIEDTRIPLE` stripped because `lt-spell` rejects them | Confirmed; modification notice now in the file header (upstream, copyright, removed directives, date) |
| Guaraní Hunspell dictionary (huracán) | `gn/hunspell/gug.dic`, `gn/hunspell/gug.aff` | GFDL-1.2-or-later | [LibreOffice dictionaries `gug/`](https://github.com/LibreOffice/dictionaries/tree/master/gug) (2016, unmaintained); the statement is in the package's thesaurus README, the `.dic`/`.aff` carry no license header | **Probable; evidence gap** (the license statement is only in the package's thesaurus README) |
| Nordum speller dictionary (generated) | `nrd/hunspell/nrd.dic` | derived from the `.dic` **word lists only** of CC BY 4.0 (Nordum word list; `nb_NO.dic` — Nasjonalbiblioteket/Ordbanken + CLARIN PUB+BY), GPL-2.0/LGPL-2.1/MPL-1.1 (da_DK.dic, Stavekontrolden) and LGPL-3.0 (sv_SE.dic, Göran Andersson); no `.aff` rules incorporated | `data/nrd/README.md` (provenance and modifications); generator `tools/nordum-dict/build-nordum-dict.py`, which reads word lists only | **License to be confirmed** (derived data; D14 owner/legal review) |
| Nordum core word list (generated) | `nrd/hunspell/nrd_core.dic` | authoritative Nordum word list + accepted alternatives + `nrd/words/core_additions.txt` (CC BY 4.0 owner data; no source-dictionary words) | `data/nrd/README.md`; generator `tools/nordum-dict/build-nordum-dict.py` | CC BY 4.0 attribution recorded |
| Hand-authored `no`/`nrd`/`gn` rules, word lists and speller additions | `no/rules/**`, `no/hunspell/ignore.txt`, `no/hunspell/prohibit.txt`, `nrd/rules/**`, `nrd/words/**`, `nrd/hunspell/ignore.txt`, `gn/rules/**`, `gn/hunspell/ignore.txt` | LGPL-2.1-or-later | project `LICENSE` | Confirmed |
| Wikipedia Norwegian typo list | `no/rules/typos_wikipedia.txt` (loaded by `NB_TYPOS` together with the LGPL `no/rules/typos.txt`) | CC BY-SA 4.0 | [Wikipedia:Liste over alminnelige stavefeil](https://no.wikipedia.org/wiki/Wikipedia:Liste_over_alminnelige_stavefeil) (retrieved 2026-09-20); [Wikimedia Terms of Use](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use) | Confirmed; modifications: wiki markup stripped, whitespace normalized, entries filtered against `no/hunspell/nb_NO.dic` and the hand-picked list; attribution is in the file header |
| OpenNLP model containers (`edu.washington.cs.knowitall:opennlp-{tokenize,postag,chunk}-models:1.5`) | `en/models/en-token.bin`, `en-pos-maxent.bin`, `en-chunker.bin` | Apache-2.0 | POMs: [tokenize](https://repo1.maven.org/maven2/edu/washington/cs/knowitall/opennlp-tokenize-models/1.5/opennlp-tokenize-models-1.5.pom), [postag](https://repo1.maven.org/maven2/edu/washington/cs/knowitall/opennlp-postag-models/1.5/opennlp-postag-models-1.5.pom), [chunk](https://repo1.maven.org/maven2/edu/washington/cs/knowitall/opennlp-chunk-models/1.5/opennlp-chunk-models-1.5.pom) | Confirmed (model training-data provenance not further documented upstream) |
| OpenRegex (Java library ported to Rust; not vendored data) | `crates/lt-chunk/src/openregex.rs` | LGPL-2.1-or-later | `edu.washington.cs.knowitall:openregex:1.1.1` Maven POM `<license>`; sources jar sha256 pinned in `licenses/README.md` | Confirmed (D-027) |

Notices for the German/Italian/French/Dutch dictionaries in the table come from
files that are themselves vendored, so the notice texts travel with the data
(`data/de/hunspell/COPYING_GPLv2.txt`, `data/fr/hunspell/README_fr.txt`,
`data/it/hunspell/README_it_IT.txt`, `data/nl/dictionaries/README.txt`,
`data/nl/spelling/README.txt`).

## License to be confirmed — owner decisions

1. **German POS dictionary (`german-pos-dict`)** is CC-BY-SA-4.0 per its POM and
   upstream README, not LGPL as the Phase-0 assumption recorded. CC-BY-SA
   requires attribution/share-alike for the dictionary data; whether to keep
   shipping it in an LGPL distribution needs the owner's sign-off.
2. **French POS dictionary (`french-pos-dict`)** has conflicting statements:
   POM CC-BY-SA-4.0 vs bundled Dicollecte lexicon docs MPL-2.0. The Dicollecte
   dictionary sources are the likely true origin.
3. **Dutch POS dictionary (`dutch-pos-dict`)** licenses the dictionaries as
   CC-BY-3.0-or-later OR BSD in the bundled README, while the artifact POM says
   LGPL-2.1.
4. **English hunspell dictionaries** bundled in `english-pos-dict` carry no
   license note; their SCOWL/hunspell origin should be confirmed.
5. **English word-list provenance docs** (AGID/Moby/WordNet) state copyrights
   but no license terms.
6. **GPL dictionaries in an LGPL distribution**: the German igerman98/frami
   hunspell dictionaries (GPL-2.0-or-later OR GPL-3.0-or-later) and the Italian
   it_IT hunspell dictionary (GPL-3.0-only) are copyleft data loaded at runtime.
   The LGPL allows conversion to the GPL (LGPL-2.1 §3), but the owner/legal
   review should confirm the distribution model before publishing binaries.
7. **Italian Morph-it! dictionaries/tagsets** are dual-licensed; pick a branch
   (CC-BY-SA-2.0 vs LGPL) and confirm it covers the derived Morfologik builds.
8. **Nordum speller dictionary** (`nrd/hunspell/nrd.dic`) is generated by
   transforming the **word lists** of the Norwegian (`nb_NO.dic`), Danish
   (`da_DK.dic`) and Swedish (`sv_SE.dic`) dictionaries with the Nordum
   orthographic rules (no `.aff` rules are read or incorporated, see
   `data/nrd/README.md`). The derived data still inherits the source word-list
   obligations (CC BY 4.0 attribution, and the copyleft options GPL-2.0/
   LGPL-2.1/MPL-1.1 and LGPL-3.0); the owner approved continuing with notices
   (D14) but a legal review of the derived-data distribution — including which
   license the derived `nrd.dic` itself can be distributed under — is still
   advisable. The Nordum word list is now dual-licensed (CC BY 4.0 **or**
   LGPL-2.1-or-later, author decision), which removes the Nordum side of the
   mixing question.

## Why this file exists

The repository is public and redistributes the pinned LanguageTool resources and
the dictionaries above, so the LGPL distribution model (plan P5.1) requires
attribution and license notices next to the engine. The machine-readable record
is `data/manifest.json`; the import mapping lives in
`tools/lt-sync/lt_sync.py` (`MAVEN_ARTIFACTS`, `IN_TREE_LICENSES`,
`UPSTREAM_FILE_LICENSES`, `hunspell_license()`), documented in
[`data/README.md`](data/README.md) and [`licenses/README.md`](licenses/README.md).

Canonical texts of the third-party licenses referenced above are vendored in
[`licenses/`](licenses/README.md) with their source URLs and retrieval dates
(CC BY 4.0, CC BY-SA 4.0, GFDL-1.2, LGPL-3.0, MPL-1.1, GPL-2.0 and LGPL-2.1).
This page is a technical compliance summary, not legal advice.
