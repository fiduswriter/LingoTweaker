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
- Pinned commit: `7bd1f99b849b1de66845113010b3c9261c3f37e0` (6.9-SNAPSHOT), see
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

"Confirmed" means the license is stated at the cited source. "Owner accepted"
marks a component the owner has cleared even though the underlying evidence is
an evidence gap (e.g. undocumented provenance); the manifest keeps
`license_verified=false` for those. All legal-review items are now decided
(the two 2026-09-23 sign-offs). Data paths are relative to `data/`.

| Component | Vendored data | License | Evidence | Status |
|-----------|---------------|---------|----------|--------|
| LanguageTool core + rule resources | `core/**`, `schemas/**`, `messages/**`, `<lang>/rules/**`, `<lang>/disambiguation.xml`, `<lang>/*.sor`, LT-authored `<lang>/words/**` | LGPL-2.1-or-later | upstream `COPYING.txt`, `pom.xml` (`<licenses>`); resources may differ per file | Confirmed (default; per-file exceptions below) |
| English POS/spelling dictionaries (`org.languagetool:english-pos-dict:0.6`) | `en/dictionaries/*` | LGPL-2.1-only | [artifact POM](https://repo1.maven.org/maven2/org/languagetool/english-pos-dict/0.6/english-pos-dict-0.6.pom) (`<licenses>`, LGPL 2.1) | Confirmed |
| English hunspell dictionaries (inside the same artifact) | `en/hunspell/*.dict`, `en/hunspell/*.info` | artifact POM: LGPL-2.1-only; underlying hunspell data (SCOWL-derived) | no license note for the hunspell files in the LT checkout or artifact | Owner accepted 2026-09-23 (evidence gap: underlying hunspell provenance undocumented, so **not** asserted as verified) |
| English word-list provenance docs | `en/words/agid-readme.txt`, `en/words/pos-readme.txt` | AGID © Kevin Atkinson; Moby Part-of-Speech II / WordNet terms | the readmes state copyright, not a license | Owner accepted 2026-09-23 (evidence gap: exact terms undocumented, so **not** asserted as verified) |
| German POS dictionary (`de.danielnaber:german-pos-dict:1.2.4`) | `de/dictionaries/*` | CC-BY-SA-4.0 | [artifact POM](https://repo1.maven.org/maven2/de/danielnaber/german-pos-dict/1.2.4/german-pos-dict-1.2.4.pom) ("data originally based on Morphy"); upstream `resource/de/README.txt` (= `de/words/README.txt`) | Confirmed |
| German hunspell dictionary (igerman98, frami extension) | `de/hunspell/de_*.aff`, `de_*.dic`, `de_DE.README`, `de_*_frami_README.txt`, `COPYING_GPLv2/3.txt`, converted `de_*.dict`/`de_*.info` | GPL-2.0-or-later OR GPL-3.0-or-later | `de_DE_frami_README.txt` ("GNU GPL, Version 2 oder 3"), vendored `COPYING_GPLv2/3.txt` | Confirmed for raw/dictionary docs; derived `.dict`/`.info` tracked as unverified |
| jWordSplitter compound word lists (`de.danielnaber:jwordsplitter:4.7`) | `de/compound/*.txt` | Apache-2.0 (Sven Abels / Daniel Naber) | [artifact POM](https://repo1.maven.org/maven2/de/danielnaber/jwordsplitter/4.7/jwordsplitter-4.7.pom) | Confirmed |
| Spanish POS dictionary (`org.softcatala:spanish-pos-dict:2.5`) | `es/dictionaries/*` | LGPL-2.1-only | [artifact POM](https://repo1.maven.org/maven2/org/softcatala/spanish-pos-dict/2.5/spanish-pos-dict-2.5.pom) | Confirmed |
| French POS dictionary (`org.languagetool:french-pos-dict:0.7`) | `fr/dictionaries/*` | POM: CC-BY-SA-4.0; bundled Dicollecte docs: MPL-2.0 | [artifact POM](https://repo1.maven.org/maven2/org/languagetool/french-pos-dict/0.7/french-pos-dict-0.7.pom); `fr/words/README_lexique.txt`, `fr/hunspell/README_fr.txt` | Owner signed off 2026-09-23 (conflicting POM/Dicollecte statements accepted) |
| Dicollecte lexicon documentation | `fr/words/README_lexique.txt`, `fr/hunspell/README_fr.txt` | MPL-2.0 | the readmes state "MPL : Mozilla Public License version 2.0" | Confirmed (docs) |
| Italian POS/synthesis dictionaries (Morph-it!) | `it/dictionaries/*` | dual: CC-BY-SA-2.0 OR LGPL | upstream `resource/it/README.txt`, `resource/it/tagset.txt` ("LICENSING INFORMATION") | Owner signed off 2026-09-23 (dual-license branch accepted for the derived builds) |
| Italian tagset | `it/words/tagset.txt` | dual: CC-BY-SA-2.0 OR LGPL | "LICENSING INFORMATION" section in the file (Morph-it!) | Confirmed (dual) |
| Italian hunspell dictionary | `it/hunspell/it_IT.dict`, `it_IT.info`, `README_it_IT.txt` | GPL-3.0-only | `README_it_IT.txt` ("License: GNU GPL 3", © 2010/2011 Andrea Pescetti) | Confirmed for the README; converted `.dict`/`.info` tracked as unverified |
| Portuguese POS dictionary + speller dictionaries (`org.languagetool:portuguese-pos-dict:1.2.0`) | `pt/dictionaries/*`, `pt/spelling/*` | LGPL-2.1-only | [artifact POM](https://repo1.maven.org/maven2/org/languagetool/portuguese-pos-dict/1.2.0/portuguese-pos-dict-1.2.0.pom) | Confirmed |
| Dutch POS dictionary (`org.languagetool:dutch-pos-dict:0.1`) | `nl/dictionaries/*` | README: CC-BY-3.0-or-later OR BSD (TaalTik); artifact POM: LGPL-2.1 | bundled `nl/dictionaries/README.txt`; [artifact POM](https://repo1.maven.org/maven2/org/languagetool/dutch-pos-dict/0.1/dutch-pos-dict-0.1.pom) | Owner signed off 2026-09-23 (conflicting README/POM statements accepted) |
| Dutch speller dictionary (TaalTik) | `nl/spelling/nl_NL.dict`, `nl_NL.info`, `README.txt` | LGPL-2.1-or-later | bundled `nl/spelling/README.txt` (Ruud Baars / TaalTik) | Confirmed |
| Greek POS/synthesis dictionaries (`GreekTagger`/`GreekSynthesizer`) | `el/dictionaries/*` | LGPL (upstream `el/README.txt`: the few test entries are "made available here under LGPL") | upstream `resource/el/README.txt` | Confirmed |
| Greek analyzer data (`org.ioperm:morphology-el:1.0.0`, `GreekAnalyzer`) | `el/morphology/analysis.dict`, `analysis.info` | POM: Apache-2.0 (source code) + CC-BY-SA-4.0 (linguistic data) | [artifact POM](https://repo1.maven.org/maven2/org/ioperm/morphology-el/1.0.0/morphology-el-1.0.0.pom) | **License to be confirmed** (CC-BY-SA-4.0 share-alike data in an LGPL distribution, like the German POS dictionary) |
| Greek hunspell dictionary (el_GR) | `el/hunspell/el_GR.dict`, `el_GR.info`, `README_el_GR.txt` | GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license | `el/hunspell/README_el_GR.txt` (© Steve Stavropoulos et al.) | Confirmed for the README; converted `.dict`/`.info` tracked as unverified |
| Danish POS/tagger dictionary (`DanishTagger`) | `da/dictionaries/danish.dict`, `danish.info` | GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license | upstream `resource/da/README.txt` (= `da/words/README.txt`; Stavekontrolden data, © Foreningen for frit tilgængelige sprogværktøjer) | Confirmed |
| Danish hunspell dictionary (Stavekontrolden `da_DK`) | `da/hunspell/da_DK.aff`, `da_DK.dic`, `README_da_DK.txt` | GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license | `da/hunspell/README_da_DK.txt` and the `da_DK.aff` header (version 2.4, © 2018) | Confirmed |
| Swedish POS/synthesis dictionaries (`SwedishTagger`/`SwedishSynthesizer`) | `sv/dictionaries/*` | LGPL-2.1-or-later (DSSO) | upstream `resource/sv/README.txt` (= `sv/words/README.txt`; Initial Developer Göran Andersson) | Confirmed |
| Swedish hunspell dictionary (`sv_SE`, den stora svenska ordlistan) | `sv/hunspell/sv_SE.aff`, `sv_SE.dic`, `LICENSE_sv_SE.txt`, `LICENSE_en_US.txt` | LGPL-3.0-only | `sv/hunspell/LICENSE_sv_SE.txt` (© Göran Andersson, 2003–19) | Confirmed |
| Norwegian Bokmål Hunspell dictionary | `no/hunspell/nb_NO.dic` | CC BY 4.0 (Nasjonalbiblioteket/Ordbanken) + CLARIN PUB+BY (nyordslister 2018–2024, CLARINO Bergen) | [LibreOffice dictionaries `no/README_NO.txt`](https://github.com/LibreOffice/dictionaries/blob/master/no/README_NO.txt) (v3.0) | Confirmed |
| Norwegian Bokmål affix rules | `no/hunspell/nb_NO.aff` | GPL-2.0 (spell-norwegian project) | same README and `no/COPYING`; `CHECKCOMPOUNDTRIPLE`/`SIMPLIFIEDTRIPLE` stripped because `lt-spell` rejects them | Confirmed; modification notice now in the file header (upstream, copyright, removed directives, date) |
| Guaraní Hunspell dictionary (huracán) | `gn/hunspell/gug.dic`, `gn/hunspell/gug.aff` | GFDL-1.2-or-later | [LibreOffice dictionaries `gug/`](https://github.com/LibreOffice/dictionaries/tree/master/gug) (2016, unmaintained); the statement is in the package's thesaurus README, the `.dic`/`.aff` carry no license header | **Probable; evidence gap** (the license statement is only in the package's thesaurus README) |
| Nordum speller dictionary (generated) | `nrd/hunspell/nrd.dic` | derived from the `.dic` **word lists only** of CC BY 4.0 (Nordum word list; `nb_NO.dic` — Nasjonalbiblioteket/Ordbanken + CLARIN PUB+BY), GPL-2.0/LGPL-2.1/MPL-1.1 (da_DK.dic, Stavekontrolden) and LGPL-3.0 (sv_SE.dic, Göran Andersson); no `.aff` rules incorporated | `data/nrd/README.md` (provenance and modifications); generator `tools/nordum-dict/build-nordum-dict.py`, which reads word lists only | **License to be confirmed** (derived data; D14 owner/legal review) |
| Nordum core word list (generated) | `nrd/hunspell/nrd_core.dic` | authoritative Nordum word list + accepted alternatives + `nrd/words/core_additions.txt` (CC BY 4.0 owner data; no source-dictionary words) | `data/nrd/README.md`; generator `tools/nordum-dict/build-nordum-dict.py` | CC BY 4.0 attribution recorded |
| Hand-authored `no`/`nrd`/`gn` rules, word lists and speller additions | `no/rules/**`, `no/hunspell/ignore.txt`, `no/hunspell/prohibit.txt`, `nrd/rules/**`, `nrd/words/**`, `nrd/hunspell/ignore.txt`, `gn/rules/**`, `gn/hunspell/ignore.txt` | LGPL-2.1-or-later | project `LICENSE` | Confirmed |
| Wikipedia Norwegian typo list | `no/rules/typos_wikipedia.txt` (loaded by `NB_TYPOS` together with the LGPL `no/rules/typos.txt`) | CC BY-SA 4.0 | [Wikipedia:Liste over alminnelige stavefeil](https://no.wikipedia.org/wiki/Wikipedia:Liste_over_alminnelige_stavefeil) (retrieved 2026-09-20); [Wikimedia Terms of Use](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use) | Confirmed; modifications: wiki markup stripped, whitespace normalized, entries filtered against `no/hunspell/nb_NO.dic` and the hand-picked list; attribution is in the file header |
| OpenNLP model containers (`edu.washington.cs.knowitall:opennlp-{tokenize,postag,chunk}-models:1.5`) | `en/models/en-token.bin`, `en-pos-maxent.bin`, `en-chunker.bin` | Apache-2.0 | POMs: [tokenize](https://repo1.maven.org/maven2/edu/washington/cs/knowitall/opennlp-tokenize-models/1.5/opennlp-tokenize-models-1.5.pom), [postag](https://repo1.maven.org/maven2/edu/washington/cs/knowitall/opennlp-postag-models/1.5/opennlp-postag-models-1.5.pom), [chunk](https://repo1.maven.org/maven2/edu/washington/cs/knowitall/opennlp-chunk-models/1.5/opennlp-chunk-models-1.5.pom) | Confirmed (model training-data provenance not further documented upstream) |
| OpenRegex (Java library ported to Rust; not vendored data) | `crates/lt-chunk/src/openregex.rs` | LGPL-2.1-or-later | `edu.washington.cs.knowitall:openregex:1.1.1` Maven POM `<license>`; sources jar sha256 pinned in `licenses/README.md` | Confirmed (D-027) |
| Morfologik + LanguageTool dictionary tools (Java, ported to Python; not vendored data) | `tools/morfologik/**` | Morfologik: BSD-3-Clause; LanguageTool `languagetool-tools`: LGPL-2.1-or-later | [morfologik-stemming](https://github.com/morfologik/morfologik-stemming) `LICENSE.txt` (BSD-3-Clause); upstream `COPYING.txt` / `pom.xml` | Confirmed |
| Lithuanian hunspell dictionary (ispell-lt, vendored because upstream ships none) | `lt/hunspell/lt_LT.aff`, `lt_LT.dic`, `README_lt_LT.txt`, `COPYING_lt_LT.txt`, `AUTHORS_lt_LT.txt` | BSD-3-Clause (© 2000–2020 Albertas Agejevas and contributors) | [LibreOffice/dictionaries `lt_LT`](https://github.com/LibreOffice/dictionaries/tree/master/lt_LT) at `8c45ec68d6b0346467c7ee23a6901139d129e468` (ispell-lt 1.3.2); bundled `COPYING_lt_LT.txt` | Confirmed |
| Belarusian POS/spelling dictionaries (`io.github.belarus:linguistics.grammardb.spell.languagetool:1.0.2`) | `be/hunspell/be_BY.dict`, `be_BY.info` | CC-BY-SA-4.0 (artifact POM, Grammardb) | [artifact POM](https://repo1.maven.org/maven2/io/github/belarus/linguistics.grammardb.spell.languagetool/1.0.2/linguistics.grammardb.spell.languagetool-1.0.2.pom) | Confirmed; owner signed off 2026-09-23 |
| Khmer POS dictionary (`KhmerTagger`) | `km/dictionaries/khmer.dict`, `khmer.info` | BSD (Chuon Nath/Buddhist Institute, Robert Headley, SBBIC) + CC-BY-NC-SA-3.0 (PanL10N KhmerCorpus) | upstream `km/README.txt` | Confirmed; owner signed off 2026-09-23 (non-commercial component accepted) |
| Khmer hunspell speller (SBBIC) | `km/hunspell/km_KH.aff`, `km_KH.dic`, `LICENCES-km.txt` | GPL-3.0-only | upstream `km/hunspell/LICENCES-km.txt` | Confirmed; owner signed off 2026-09-23 |
| Malayalam POS/speller dictionaries | `ml/dictionaries/malayalam.*`, `ml/hunspell/ml_IN.*` | GPL (Jithesh.V.S. / C-DIT, public sources; `ml_IN` is a derived Morfologik build) | upstream `ml/README.txt` | Confirmed; owner signed off 2026-09-23 |
| Tamil dictionary/tagset | `ta/dictionaries/tamil.dict`, `tamil.info` | GPLv3 (Ve. Elanjelian; Crubadan 2.0 corpus GPLv3) | upstream `ta/README.txt` | Confirmed; owner signed off 2026-09-23 |
| Simple German rule XML | `de/rules/de-DE-x-simple-language/grammar.xml` | LGPL-2.1-or-later (LanguageTool resource; per-file license varies) | upstream module pom (LGPL-2.1) | Confirmed; owner signed off 2026-09-23 |
| Arabic POS/synthesis dictionaries + Hunspell-ar speller | `ar/dictionaries/*`, `ar/hunspell/ar.*` | Arramooz-derived GPL (upstream `ar/README.txt`); Hunspell-ar tri-license GPL-2.0-or-later OR LGPL-2.1-or-later OR MPL-1.1-or-later (`ar/hunspell/COPYING`) | upstream `ar/README.txt`, `ar/hunspell/COPYING` | Confirmed; owner signed off 2026-09-23 (LGPL-2.1+ path recorded) |
| Breton POS dictionary | `br/dictionaries/*` | GPL (Apertium-derived, with permission of its authors; upstream `br/README.txt`) | upstream `br/README.txt` | Confirmed; owner signed off 2026-09-23 |
| Catalan POS/spelling dictionaries (`org.softcatala:catalan-pos-dict:3.3`) | `ca/dictionaries/*`, `ca/spelling/*` | POM: GPL-2.0-only; upstream `ca/README.txt` documents dual LGPL-2.1 OR GPL-2.0 | [artifact POM](https://repo1.maven.org/maven2/org/softcatala/catalan-pos-dict/3.3/catalan-pos-dict-3.3.pom); upstream `ca/README.txt` | Confirmed; owner signed off 2026-09-23 (dual-license statements accepted) |
| Galician POS/spelling dictionaries + hunspell `gl_ES` | `gl/dictionaries/*`, `gl/hunspell/gl_ES.*`, `gl/hunspell/README-gl-ES.txt` | GPL (Freeling/Apertium-derived; upstream `gl/README.txt`; VOLGa `gl_ES` per `README-gl-ES.txt`) | upstream `gl/README.txt`, `gl/hunspell/README-gl-ES.txt` | Confirmed; owner signed off 2026-09-23 |

Notices for the German/Italian/French/Dutch dictionaries in the table come from
files that are themselves vendored, so the notice texts travel with the data
(`data/de/hunspell/COPYING_GPLv2.txt`, `data/fr/hunspell/README_fr.txt`,
`data/it/hunspell/README_it_IT.txt`, `data/nl/dictionaries/README.txt`,
`data/nl/spelling/README.txt`).

## Owner sign-offs (2026-09-23)

**Decision (owner, 2026-09-23):** the owner has signed off on the legal-review
items below, accepting the current distribution model (project code stays
LGPL-2.1-or-later; each vendored component keeps its own license and notice,
and the copyleft/CC-BY-SA data loaded at runtime is treated as a separately
licensed data component rather than a combined work with the LGPL engine). This
clears the corresponding "owner decisions" items further down, and the
per-file records in `data/manifest.json` (and `tools/lt-sync/lt_sync.py`) now
carry an `owner signed off 2026-09-23` note:

- **Khmer (`km`)**: the POS dictionary mixes BSD components with a
  **CC-BY-NC-SA-3.0** component (PanL10N KhmerCorpus, non-commercial) and the
  SBBIC hunspell speller is **GPL-3.0-only**; the non-commercial component is
  accepted.
- **Malayalam (`ml`)**: the POS dictionary and the derived `ml_IN` Morfologik
  speller data are **GPL** (Jithesh.V.S. / C-DIT, public sources).
- **Tamil (`ta`)**: the dictionary, tagset and rules are **GPLv3**
  (Ve. Elanjelian; Crubadan 2.0 corpus GPLv3).
- **Belarusian (`be`)**: the `be_BY` speller dictionary is **CC-BY-SA-4.0**
  (Grammardb artifact POM).
- **Greek (`el`)**: the analyzer data (`org.ioperm:morphology-el`) is
  **CC-BY-SA-4.0** (source code Apache-2.0) and the `el_GR` hunspell dictionary
  is the **GPL-2.0 / LGPL-2.1 / MPL-1.1** tri-license.
- **German (`de`)**: the POS dictionary is **CC-BY-SA-4.0** (Morphy-derived)
  and the igerman98/frami hunspell dictionaries are **GPL-2.0-or-later OR
  GPL-3.0-or-later**; the `de-DE-x-simple-language` rule XML is an upstream
  LanguageTool resource under the default upstream license.
- **French (`fr`)**: the POS dictionary's conflicting statements (artifact POM
  **CC-BY-SA-4.0** vs bundled Dicollecte lexicon docs **MPL-2.0**) are accepted.
- **Dutch (`nl`)**: the POS dictionary's conflicting statements (bundled README
  **CC-BY-3.0-or-later OR BSD** vs artifact POM **LGPL-2.1**) are accepted.
- **Catalan (`ca`)**: the POS/spelling dictionary's dual statements (artifact
  POM **GPL-2.0-only**, upstream `ca/README.txt` **LGPL-2.1 OR GPL-2.0**) are
  accepted.
- **Italian (`it`)**: the Morph-it! POS/synthesis dictionaries' dual license
  (**CC-BY-SA-2.0 OR LGPL**) is accepted for the derived Morfologik builds.
- **Arabic (`ar`)**: the Arramooz-derived GPL POS/synthesis dictionaries and
  the Hunspell-ar **GPL-2.0-or-later OR LGPL-2.1-or-later OR MPL-1.1-or-later**
  speller are accepted (LGPL-2.1+ path recorded).
- **Breton (`br`)**: the Apertium-derived **GPL** POS dictionary (used with the
  authors' permission) is accepted.
- **Galician (`gl`)**: the Freeling/Apertium-derived **GPL** POS/synthesis
  dictionaries and the VOLGa `gl_ES` hunspell dictionary are accepted.
- **English (`en`) — evidence gap, accepted but not asserted as verified**: the
  hunspell dictionaries bundled in `english-pos-dict` carry the artifact's
  **LGPL-2.1-only** POM license but their underlying SCOWL/hunspell provenance
  is undocumented, and the `en/words/agid-readme.txt` / `pos-readme.txt`
  provenance docs state copyrights rather than license terms. The owner accepted
  these components on 2026-09-23; `license_verified` remains `false` for them
  because the provenance/terms were **not** verified.
- The earlier `no`/`nrd`/`gn` review items (D-163/D-164) remain as recorded;
  the German POS, German/Greek/Italian hunspell and Greek analyzer data are
  the "related items" covered by the same sign-off.

**All legal-review items are now covered by the owner's sign-off.** The
previously "not covered" items — the conflicting POS-dictionary statements
(`fr`, `nl`), the Catalan dual statement (`ca`), the Italian Morph-it! dual
license (`it`), the undocumented English hunspell/word-list provenance (`en`,
accepted as an evidence gap), and the `ar`/`br`/`gl` entries — are cleared
above (second owner sign-off, 2026-09-23; decision D-313). Only the `en`
provenance items keep `license_verified=false`, and only because the
underlying evidence itself remains undocumented; the owner's acceptance is
recorded in the license note.

`license_verified` in `data/manifest.json` records whether the license
declaration is found at `license_source`. Where a legal-review item's only
outstanding question was the owner decision, that flag is now `true` and the
`license` note carries the sign-off date (the conflicting/dual-statement and
`ar`/`br`/`gl` items, and the earlier `be`/`km`/`ta`/`ml` items). For the
genuine evidence gaps (`en`) the flag stays `false` and the note records the
owner's acceptance without asserting the undocumented provenance. Derived
Morfologik builds keep the unchanged derived-conversion flag convention.

## Owner decisions — license questions (all resolved 2026-09-23)

1. **German POS dictionary (`german-pos-dict`)** is CC-BY-SA-4.0 per its POM and
   upstream README, not LGPL as the Phase-0 assumption recorded. CC-BY-SA
   requires attribution/share-alike for the dictionary data; whether to keep
   shipping it in an LGPL distribution needs the owner's sign-off.
   **Owner signed off 2026-09-23** (see above).
2. **French POS dictionary (`french-pos-dict`)** has conflicting statements:
   POM CC-BY-SA-4.0 vs bundled Dicollecte lexicon docs MPL-2.0. The Dicollecte
   dictionary sources are the likely true origin. **Owner signed off
   2026-09-23** (conflicting statements accepted).
3. **Dutch POS dictionary (`dutch-pos-dict`)** licenses the dictionaries as
   CC-BY-3.0-or-later OR BSD in the bundled README, while the artifact POM says
   LGPL-2.1. **Owner signed off 2026-09-23** (conflicting statements accepted).
4. **English hunspell dictionaries** bundled in `english-pos-dict` carry no
   license note; their SCOWL/hunspell origin should be confirmed. **Owner
   accepted 2026-09-23 as an evidence gap** — the provenance remains
   undocumented, so it is recorded as accepted rather than asserted verified.
5. **English word-list provenance docs** (AGID/Moby/WordNet) state copyrights
   but no license terms. **Owner accepted 2026-09-23 as an evidence gap** —
   the exact terms remain undocumented, so recorded as accepted rather than
   asserted verified.
6. **GPL dictionaries in an LGPL distribution**: the German igerman98/frami
   hunspell dictionaries (GPL-2.0-or-later OR GPL-3.0-or-later) and the Italian
   it_IT hunspell dictionary (GPL-3.0-only) are copyleft data loaded at runtime.
   The LGPL allows conversion to the GPL (LGPL-2.1 §3), but the owner/legal
   review should confirm the distribution model before publishing binaries.
   **Owner signed off 2026-09-23**: the copyleft data is distributed as a
   separately licensed data component (aggregation), not as a combined work
   with the LGPL engine.
7. **Italian Morph-it! dictionaries/tagsets** are dual-licensed; pick a branch
   (CC-BY-SA-2.0 vs LGPL) and confirm it covers the derived Morfologik builds.
   **Owner signed off 2026-09-23** (dual-license branch accepted for the
   derived builds).
8. **Nordum speller dictionary** (`nrd/hunspell/nrd.dic`) is generated by
   transforming the **word lists** of the Norwegian (`nb_NO.dic`), Danish
   (`da_DK.dic`) and Swedish (`sv_SE.dic`) dictionaries with the Nordum
   orthographic rules (no `.aff` rules are read or incorporated, see
   `data/nrd/README.md`). The derived data still inherits the source word-list
   obligations (CC BY 4.0 attribution, and the copyleft options GPL-2.0/
   LGPL-2.1/MPL-1.1 and LGPL-3.0); the owner approved continuing with notices
   (D14) but a legal review of the derived-data distribution — including which
   license the derived `nrd.dic` itself can be distributed under — is still
   advisable. The Nordum word list is now    dual-licensed (CC BY 4.0 **or**
   LGPL-2.1-or-later, author decision), which removes the Nordum side of the
   mixing question.
9. **Greek analyzer data (`org.ioperm:morphology-el`)** is CC-BY-SA-4.0
   (linguistic data; the source code is Apache-2.0) per its POM. CC-BY-SA
   requires attribution/share-alike for the dictionary data; whether to keep
   shipping it in an LGPL distribution needs the owner's sign-off, like the
   German POS dictionary above. **Owner signed off 2026-09-23.**
10. **Belarusian dictionaries
    (`io.github.belarus:linguistics.grammardb.spell.languagetool:1.0.2`)** are
    CC-BY-SA-4.0 per the artifact POM. As with the German POS and Greek
    analyzer data, CC-BY-SA requires attribution/share-alike for the
    dictionary data; whether to keep shipping it in an LGPL distribution needs
    the owner's sign-off. (The Lithuanian ispell-lt dictionary, by contrast, is
    BSD-3-Clause and needs no such review.) **Owner signed off 2026-09-23.**

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
