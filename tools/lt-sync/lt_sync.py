#!/usr/bin/env python3
"""lt-sync — upstream sync and vendored-data management.

Subcommands:
  baseline  Pin the upstream baseline commit into upstream.json.
  import    Vendor rule/data files from an upstream checkout (+ pre-downloaded
            Maven artifacts) into data/ and write data/manifest.json.
  status    Classify upstream changes since the pinned manifest.
  report    Emit an agent-readable porting checklist skeleton.
  fetch     Clone/fetch the upstream repository up to a commit or tag.

License metadata (P5.1): every manifest entry records `source.license`
(SPDX-style expression plus a short note), `source.license_verified` (True only
when the license is documented at `source.license_source`) and, where known,
`source.license_source` (POM URL and/or upstream README path). The mapping
lives in MAVEN_ARTIFACTS, IN_TREE_LICENSES, UPSTREAM_FILE_LICENSES and
hunspell_license(); the human-readable component table is
THIRD_PARTY_NOTICES.md. Files without an explicit override keep the default
LanguageTool resource license.

Stdlib only. Run from the languagetool-rs repository root.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
UPSTREAM_JSON = REPO_ROOT / "upstream.json"
DATA_DIR = REPO_ROOT / "data"
MANIFEST_JSON = DATA_DIR / "manifest.json"

UPSTREAM_REPO_URL = "https://github.com/languagetool-org/languagetool.git"

LANGS = ["en", "de", "es", "fr", "it", "pt", "nl", "ca", "gl", "ro", "pl", "sk", "sl", "el", "da", "sv", "is", "eo", "ast", "br"]

# Manifest kinds that are not imported from upstream. `add-local` records
# them; `import` preserves them (upstream sync must never drop hand-authored
# or vendored data).
LOCAL_KINDS = ("hand-authored", "vendor", "generated")

# Maven artifacts pinned via the upstream root pom.xml (dependencyManagement).
#
# License mapping (P5.1; the authoritative component table is
# THIRD_PARTY_NOTICES.md):
#   - `license` is the license declared for the artifact, taken from the
#     published POM `<licenses>` section where one exists (cited verbatim via
#     `license_source`), or from a README bundled in the artifact.
#   - `license_verified` is True only when that declaration exists at
#     `license_source`; False means "license to be confirmed" (an explicit
#     "to be confirmed" note is part of `license`).
#   - `license_source` is a human-readable evidence pointer (POM URL and/or
#     upstream resource path).
# The POM URLs are on Maven Central (https://repo1.maven.org/maven2).
MAVEN_CENTRAL = "https://repo1.maven.org/maven2"
MAVEN_ARTIFACTS = {
    "english-pos-dict-0.6.jar": {
        "coords": "org.languagetool:english-pos-dict:0.6",
        # POM: "GNU Lesser General Public License, Version 2.1, February 1999"
        # (no later-version clause).
        "license": "LGPL-2.1-only (artifact POM)",
        "license_verified": True,
        "license_source": f"{MAVEN_CENTRAL}/org/languagetool/english-pos-dict/0.6/english-pos-dict-0.6.pom",
    },
    "german-pos-dict-1.2.4.jar": {
        "coords": "de.danielnaber:german-pos-dict:1.2.4",
        # POM: CC-BY-SA-4.0, "data originally based on Morphy"; matches the
        # upstream resource README (de/words/README.txt).
        "license": "CC-BY-SA-4.0 (artifact POM, data based on Morphy)",
        "license_verified": True,
        "license_source": f"{MAVEN_CENTRAL}/de/danielnaber/german-pos-dict/1.2.4/german-pos-dict-1.2.4.pom",
    },
    "spanish-pos-dict-2.5.jar": {
        "coords": "org.softcatala:spanish-pos-dict:2.5",
        # POM: "GNU Lesser General Public License, Version 2.1, February 1999".
        "license": "LGPL-2.1-only (artifact POM)",
        "license_verified": True,
        "license_source": f"{MAVEN_CENTRAL}/org/softcatala/spanish-pos-dict/2.5/spanish-pos-dict-2.5.pom",
    },
    "french-pos-dict-0.7.jar": {
        "coords": "org.languagetool:french-pos-dict:0.7",
        # POM declares CC-BY-SA-4.0, but the bundled lexicon documentation
        # (fr/words/README_lexique.txt, fr/hunspell/README_fr.txt) states
        # MPL-2.0 for the Dicollecte lexique -> owner must confirm.
        "license": "CC-BY-SA-4.0 (artifact POM); Dicollecte lexicon docs state MPL-2.0 - to be confirmed",
        "license_verified": False,
        "license_source": f"{MAVEN_CENTRAL}/org/languagetool/french-pos-dict/0.7/french-pos-dict-0.7.pom; upstream fr/words/README_lexique.txt",
    },
    "portuguese-pos-dict-1.2.0.jar": {
        "coords": "org.languagetool:portuguese-pos-dict:1.2.0",
        # POM: "GNU Lesser General Public License, Version 2.1, February 1999".
        "license": "LGPL-2.1-only (artifact POM)",
        "license_verified": True,
        "license_source": f"{MAVEN_CENTRAL}/org/languagetool/portuguese-pos-dict/1.2.0/portuguese-pos-dict-1.2.0.pom",
    },
    "dutch-pos-dict-0.1.jar": {
        "coords": "org.languagetool:dutch-pos-dict:0.1",
        # The bundled nl/dictionaries/README.txt licenses the dictionaries as
        # CC-BY-3.0-or-later OR BSD (TaalTik); the artifact POM declares
        # LGPL-2.1 -> both statements recorded, owner must confirm. The
        # nl/spelling/README.txt dicts are LGPL-2.1-or-later (TaalTik).
        "license": "CC-BY-3.0-or-later OR BSD (nl/dictionaries/README.txt); LGPL-2.1-or-later (nl/spelling/README.txt); POM declares LGPL-2.1 - to be confirmed",
        "license_verified": False,
        "license_source": f"{MAVEN_CENTRAL}/org/languagetool/dutch-pos-dict/0.1/dutch-pos-dict-0.1.pom; bundled nl/dictionaries/README.txt, nl/spelling/README.txt",
    },
    "catalan-pos-dict-3.3.jar": {
        "coords": "org.softcatala:catalan-pos-dict:3.3",
        # POM declares "GNU General Public License, Version 2" (GPL-2.0-only);
        # the upstream module README (resource/ca/README.txt) documents the
        # Softcatalà dictionaries as dual LGPL-2.1/GPL-2.0 -> owner must
        # confirm. The dictionary conversion is a build of those sources.
        "license": "GPL-2.0-only (artifact POM); upstream ca/README.txt documents dual LGPL-2.1 OR GPL-2.0 - to be confirmed",
        "license_verified": False,
        "license_source": f"{MAVEN_CENTRAL}/org/softcatala/catalan-pos-dict/3.3/catalan-pos-dict-3.3.pom; upstream ca/README.txt",
    },
    "morphology-el-1.0.0.jar": {
        "coords": "org.ioperm:morphology-el:1.0.0",
        # POM: "Apache License, Version 2.0" for the source code and
        # CC-BY-SA-4.0 for the linguistic data (analysis.dict); only the data
        # dictionary is vendored (GreekTagger's analyzer).
        "license": "CC-BY-SA-4.0 (artifact POM, linguistic data; source code Apache-2.0)",
        "license_verified": True,
        "license_source": f"{MAVEN_CENTRAL}/org/ioperm/morphology-el/1.0.0/morphology-el-1.0.0.pom",
    },
    "jwordsplitter-4.7.jar": {
        "coords": "de.danielnaber:jwordsplitter:4.7",
        # POM: "The Apache Software License, Version 2.0"; Copyright Sven
        # Abels / Daniel Naber (only the compound word lists are vendored).
        "license": "Apache-2.0 (artifact POM; jWordSplitter, Sven Abels / Daniel Naber)",
        "license_verified": True,
        "license_source": f"{MAVEN_CENTRAL}/de/danielnaber/jwordsplitter/4.7/jwordsplitter-4.7.pom",
    },
    "asturian-pos-dict-0.1.jar": {
        "coords": "org.languagetool:asturian-pos-dict:0.1",
        # POM: "GNU GENERAL PUBLIC LICENSE Version 3", data based on Morphy;
        # the bundled ast/hunspell/LICENCES-ast.txt confirms GPL v3 for the
        # Softastur dictionary.
        "license": "GPL-3.0-only (artifact POM; data originally based on Morphy; ast/hunspell/LICENCES-ast.txt)",
        "license_verified": True,
        "license_source": f"{MAVEN_CENTRAL}/org/languagetool/asturian-pos-dict/0.1/asturian-pos-dict-0.1.pom",
    },
    "opennlp-chunk-models-1.5.jar": {
        "coords": "edu.washington.cs.knowitall:opennlp-chunk-models:1.5",
        # POM: "The Apache Software License, Version 2.0" (stock OpenNLP
        # 1.5-era models; en-chunker.bin).
        "license": "Apache-2.0 (artifact POM)",
        "license_verified": True,
        "license_source": f"{MAVEN_CENTRAL}/edu/washington/cs/knowitall/opennlp-chunk-models/1.5/opennlp-chunk-models-1.5.pom",
    },
    "opennlp-postag-models-1.5.jar": {
        "coords": "edu.washington.cs.knowitall:opennlp-postag-models:1.5",
        # POM: "The Apache Software License, Version 2.0" (en-pos-maxent.bin).
        "license": "Apache-2.0 (artifact POM)",
        "license_verified": True,
        "license_source": f"{MAVEN_CENTRAL}/edu/washington/cs/knowitall/opennlp-postag-models/1.5/opennlp-postag-models-1.5.pom",
    },
    "opennlp-tokenize-models-1.5.jar": {
        "coords": "edu.washington.cs.knowitall:opennlp-tokenize-models:1.5",
        # POM: "The Apache Software License, Version 2.0" (en-token.bin).
        "license": "Apache-2.0 (artifact POM)",
        "license_verified": True,
        "license_source": f"{MAVEN_CENTRAL}/edu/washington/cs/knowitall/opennlp-tokenize-models/1.5/opennlp-tokenize-models-1.5.pom",
    },
}

# inner-path-in-jar -> destination relative to data/
JAR_EXTRACTIONS = {
    # `AsturianTagger` (`asturian.dict`) and `MorfologikAsturianSpellerRule`
    # (`ast/hunspell/ast_ES.dict`); the LICENCES files document the GPL-3.0
    # Softastur dictionary.
    "asturian-pos-dict-0.1.jar": {
        "org/languagetool/resource/ast/asturian.dict": "ast/dictionaries/asturian.dict",
        "org/languagetool/resource/ast/asturian.info": "ast/dictionaries/asturian.info",
        "org/languagetool/resource/ast/hunspell/ast_ES.dict": "ast/hunspell/ast_ES.dict",
        "org/languagetool/resource/ast/hunspell/ast_ES.info": "ast/hunspell/ast_ES.info",
        "org/languagetool/resource/ast/hunspell/LICENCES-ast.txt": "ast/hunspell/LICENCES-ast.txt",
        "org/languagetool/resource/ast/hunspell/LICENSES-en.txt": "ast/hunspell/LICENSES-en.txt",
    },
    "english-pos-dict-0.6.jar": {
        "org/languagetool/resource/en/english.dict": "en/dictionaries/english.dict",
        "org/languagetool/resource/en/english.info": "en/dictionaries/english.info",
        "org/languagetool/resource/en/english_synth.dict": "en/dictionaries/english_synth.dict",
        "org/languagetool/resource/en/english_synth.info": "en/dictionaries/english_synth.info",
        "org/languagetool/resource/en/english_tags.txt": "en/dictionaries/english_tags.txt",
        "org/languagetool/resource/en/hunspell/en_US.dict": "en/hunspell/en_US.dict",
        "org/languagetool/resource/en/hunspell/en_US.info": "en/hunspell/en_US.info",
        "org/languagetool/resource/en/hunspell/en_GB.dict": "en/hunspell/en_GB.dict",
        "org/languagetool/resource/en/hunspell/en_GB.info": "en/hunspell/en_GB.info",
        "org/languagetool/resource/en/hunspell/en_AU.dict": "en/hunspell/en_AU.dict",
        "org/languagetool/resource/en/hunspell/en_AU.info": "en/hunspell/en_AU.info",
        "org/languagetool/resource/en/hunspell/en_CA.dict": "en/hunspell/en_CA.dict",
        "org/languagetool/resource/en/hunspell/en_CA.info": "en/hunspell/en_CA.info",
        "org/languagetool/resource/en/hunspell/en_NZ.dict": "en/hunspell/en_NZ.dict",
        "org/languagetool/resource/en/hunspell/en_NZ.info": "en/hunspell/en_NZ.info",
    },
    "german-pos-dict-1.2.4.jar": {
        "org/languagetool/resource/de/german.dict": "de/dictionaries/german.dict",
        "org/languagetool/resource/de/german.info": "de/dictionaries/german.info",
        "org/languagetool/resource/de/german_synth.dict": "de/dictionaries/german_synth.dict",
        "org/languagetool/resource/de/german_synth.info": "de/dictionaries/german_synth.info",
        "org/languagetool/resource/de/german_tags.txt": "de/dictionaries/german_tags.txt",
    },
    "spanish-pos-dict-2.5.jar": {
        "org/languagetool/resource/es/es-ES.dict": "es/dictionaries/es-ES.dict",
        "org/languagetool/resource/es/es-ES.info": "es/dictionaries/es-ES.info",
        "org/languagetool/resource/es/es-ES_synth.dict": "es/dictionaries/es-ES_synth.dict",
        "org/languagetool/resource/es/es-ES_synth.info": "es/dictionaries/es-ES_synth.info",
        "org/languagetool/resource/es/es-ES_tags.txt": "es/dictionaries/es-ES_tags.txt",
    },
    "french-pos-dict-0.7.jar": {
        "org/languagetool/resource/fr/french.dict": "fr/dictionaries/french.dict",
        "org/languagetool/resource/fr/french.info": "fr/dictionaries/french.info",
        "org/languagetool/resource/fr/french_synth.dict": "fr/dictionaries/french_synth.dict",
        "org/languagetool/resource/fr/french_synth.info": "fr/dictionaries/french_synth.info",
        "org/languagetool/resource/fr/french_tags.txt": "fr/dictionaries/french_tags.txt",
    },
    "portuguese-pos-dict-1.2.0.jar": {
        "org/languagetool/resource/pt/portuguese.dict": "pt/dictionaries/portuguese.dict",
        "org/languagetool/resource/pt/portuguese.info": "pt/dictionaries/portuguese.info",
        "org/languagetool/resource/pt/portuguese_synth.dict": "pt/dictionaries/portuguese_synth.dict",
        "org/languagetool/resource/pt/portuguese_synth.info": "pt/dictionaries/portuguese_synth.info",
        "org/languagetool/resource/pt/portuguese_tags.txt": "pt/dictionaries/portuguese_tags.txt",
        # `MorfologikPortugueseSpellerRule` dictionaries, selected per variant
        # (pt-BR, pt-PT-90, pt-PT-45 for AO/MZ).
        "org/languagetool/resource/pt/spelling/pt-BR.dict": "pt/spelling/pt-BR.dict",
        "org/languagetool/resource/pt/spelling/pt-BR.info": "pt/spelling/pt-BR.info",
        "org/languagetool/resource/pt/spelling/pt-PT-90.dict": "pt/spelling/pt-PT-90.dict",
        "org/languagetool/resource/pt/spelling/pt-PT-90.info": "pt/spelling/pt-PT-90.info",
        "org/languagetool/resource/pt/spelling/pt-PT-45.dict": "pt/spelling/pt-PT-45.dict",
        "org/languagetool/resource/pt/spelling/pt-PT-45.info": "pt/spelling/pt-PT-45.info",
    },
    "dutch-pos-dict-0.1.jar": {
        "org/languagetool/resource/nl/dutch.dict": "nl/dictionaries/dutch.dict",
        "org/languagetool/resource/nl/dutch.info": "nl/dictionaries/dutch.info",
        "org/languagetool/resource/nl/dutch_synth.dict": "nl/dictionaries/dutch_synth.dict",
        "org/languagetool/resource/nl/dutch_synth.info": "nl/dictionaries/dutch_synth.info",
        "org/languagetool/resource/nl/dutch_tags.txt": "nl/dictionaries/dutch_tags.txt",
        "org/languagetool/resource/nl/README.txt": "nl/dictionaries/README.txt",
        # `MorfologikDutchSpellerRule` dictionary (`/nl/spelling/nl_NL.dict`).
        "org/languagetool/resource/nl/spelling/nl_NL.dict": "nl/spelling/nl_NL.dict",
        "org/languagetool/resource/nl/spelling/nl_NL.info": "nl/spelling/nl_NL.info",
        "org/languagetool/resource/nl/spelling/README.txt": "nl/spelling/README.txt",
    },
    # `CatalanTagger` (`ca-ES.dict`), `CatalanSynthesizer`
    # (`ca-ES_synth.dict`, `ca-ES_tags.txt`) and `MorfologikCatalanSpellerRule`
    # (`ca-ES_spelling.dict`); the multitoken speller dictionary is used by
    # `CatalanMorfologikMultitokenSpeller`.
    "catalan-pos-dict-3.3.jar": {
        "org/languagetool/resource/ca/ca-ES.dict": "ca/dictionaries/ca-ES.dict",
        "org/languagetool/resource/ca/ca-ES.info": "ca/dictionaries/ca-ES.info",
        "org/languagetool/resource/ca/ca-ES_synth.dict": "ca/dictionaries/ca-ES_synth.dict",
        "org/languagetool/resource/ca/ca-ES_synth.info": "ca/dictionaries/ca-ES_synth.info",
        "org/languagetool/resource/ca/ca-ES_tags.txt": "ca/dictionaries/ca-ES_tags.txt",
        "org/languagetool/resource/ca/ca-ES_spelling.dict": "ca/spelling/ca-ES_spelling.dict",
        "org/languagetool/resource/ca/ca-ES_spelling.info": "ca/spelling/ca-ES_spelling.info",
        "org/languagetool/resource/ca/ca-ES_spelling_multitoken.dict": "ca/spelling/ca-ES_spelling_multitoken.dict",
        "org/languagetool/resource/ca/ca-ES_spelling_multitoken.info": "ca/spelling/ca-ES_spelling_multitoken.info",
    },
    # `GreekTagger`'s `GreekAnalyzer` (org.ioperm:morphology-el) POS data.
    "morphology-el-1.0.0.jar": {
        "org/ioperm/morphology/el/analysis.dict": "el/morphology/analysis.dict",
        "org/ioperm/morphology/el/analysis.info": "el/morphology/analysis.info",
    },
    "jwordsplitter-4.7.jar": {
        "de/danielnaber/jwordsplitter/wordsGerman.txt": "de/compound/wordsGerman.txt",
        "de/danielnaber/jwordsplitter/exceptionsGerman.txt": "de/compound/exceptionsGerman.txt",
    },
    "opennlp-tokenize-models-1.5.jar": {"en-token.bin": "en/models/en-token.bin"},
    "opennlp-postag-models-1.5.jar": {"en-pos-maxent.bin": "en/models/en-pos-maxent.bin"},
    "opennlp-chunk-models-1.5.jar": {"en-chunker.bin": "en/models/en-chunker.bin"},
}

# Language modules that ship their POS/synthesis dictionaries in-tree instead
# of a Maven artifact: module-inner path -> destination relative to data/.
# Italian is the first such language (its morph-it! derived dictionaries live
# under `<module>/src/main/resources/org/languagetool/resource/it/`); the
# tags file lands in `dictionaries/` like the es/fr artifact extracts.
IN_TREE_ARTIFACTS = {
    "it": {
        "resource/it/italian.dict": "it/dictionaries/italian.dict",
        "resource/it/italian.info": "it/dictionaries/italian.info",
        "resource/it/italian_synth.dict": "it/dictionaries/italian_synth.dict",
        "resource/it/italian_synth.info": "it/dictionaries/italian_synth.info",
        "resource/it/italian_tags.txt": "it/dictionaries/italian_tags.txt",
    },
    # Galician ships its Freeling/Apertium-derived POS/synthesis dictionaries
    # in-tree (`GalicianTagger` / `GalicianSynthesizer`).
    "gl": {
        "resource/gl/galician.dict": "gl/dictionaries/galician.dict",
        "resource/gl/galician.info": "gl/dictionaries/galician.info",
        "resource/gl/galician_synth.dict": "gl/dictionaries/galician_synth.dict",
        "resource/gl/galician_synth.info": "gl/dictionaries/galician_synth.info",
        "resource/gl/galician_tags.txt": "gl/dictionaries/galician_tags.txt",
    },
    # Romanian ships its POS/synthesis dictionaries in-tree
    # (`RomanianTagger` / `RomanianSynthesizer`).
    "ro": {
        "resource/ro/romanian.dict": "ro/dictionaries/romanian.dict",
        "resource/ro/romanian.info": "ro/dictionaries/romanian.info",
        "resource/ro/romanian_synth.dict": "ro/dictionaries/romanian_synth.dict",
        "resource/ro/romanian_synth.info": "ro/dictionaries/romanian_synth.info",
        "resource/ro/romanian_tags.txt": "ro/dictionaries/romanian_tags.txt",
    },
    # Polish ships the PoliMorf 2.0 POS/synthesis dictionaries in-tree
    # (`PolishTagger` / `PolishSynthesizer`).
    "pl": {
        "resource/pl/polish.dict": "pl/dictionaries/polish.dict",
        "resource/pl/polish.info": "pl/dictionaries/polish.info",
        "resource/pl/polish_synth.dict": "pl/dictionaries/polish_synth.dict",
        "resource/pl/polish_synth.info": "pl/dictionaries/polish_synth.info",
        "resource/pl/polish_tags.txt": "pl/dictionaries/polish_tags.txt",
    },
    # Slovak ships the Slovak National Corpus POS/synthesis dictionaries
    # in-tree (`SlovakTagger` / `SlovakSynthesizer`).
    "sk": {
        "resource/sk/slovak.dict": "sk/dictionaries/slovak.dict",
        "resource/sk/slovak.info": "sk/dictionaries/slovak.info",
        "resource/sk/slovak_synth.dict": "sk/dictionaries/slovak_synth.dict",
        "resource/sk/slovak_synth.info": "sk/dictionaries/slovak_synth.info",
        "resource/sk/slovak_tags.txt": "sk/dictionaries/slovak_tags.txt",
    },
    # Greek ships its `BaseTagger` POS dictionary (`GreekTagger`, a small
    # LGPL test lexicon) and the `GreekSynthesizer` data in-tree; the larger
    # `org.ioperm:morphology-el` analyzer dictionaries come from a Maven
    # artifact (see `MAVEN_ARTIFACTS`/`JAR_EXTRACTIONS`).
    "el": {
        "resource/el/greek.dict": "el/dictionaries/greek.dict",
        "resource/el/greek.info": "el/dictionaries/greek.info",
        "resource/el/greek_synth.dict": "el/dictionaries/greek_synth.dict",
        "resource/el/greek_synth.info": "el/dictionaries/greek_synth.info",
        "resource/el/greek_tags.txt": "el/dictionaries/greek_tags.txt",
    },
    # Danish ships its `BaseTagger` Morfologik dictionary in-tree
    # (`DanishTagger`); the speller is hunspell (`da_DK`).
    "da": {
        "resource/da/danish.dict": "da/dictionaries/danish.dict",
        "resource/da/danish.info": "da/dictionaries/danish.info",
    },
    # Swedish ships its `BaseTagger` and `BaseSynthesizer` Morfologik
    # dictionaries in-tree (`SwedishTagger`/`SwedishSynthesizer`).
    "sv": {
        "resource/sv/swedish.dict": "sv/dictionaries/swedish.dict",
        "resource/sv/swedish.info": "sv/dictionaries/swedish.info",
        "resource/sv/swedish_synth.dict": "sv/dictionaries/swedish_synth.dict",
        "resource/sv/swedish_synth.info": "sv/dictionaries/swedish_synth.info",
        "resource/sv/swedish_synth.dict_tags.txt": "sv/dictionaries/swedish_synth_tags.txt",
    },
    # Breton ships its `BaseTagger` Morfologik dictionary (FSA5 format) in-tree
    # (`BretonTagger`); the FSA spelling dictionary (`hunspell/br_FR.dict`) is
    # imported with the rest of the `hunspell/` directory.
    "br": {
        "resource/br/breton.dict": "br/dictionaries/breton.dict",
        "resource/br/breton.info": "br/dictionaries/breton.info",
    },
}

# Language-specific resource subdirectories whose word lists are referenced
# by the language's Java rule classes (not by the XML): the files keep their
# module-relative path under data/<lang>/.
LANG_RESOURCE_SUBDIRS = {
    # `MorfologikDutchSpellerRule` ignore/prohibit/spelling lists and
    # `CompoundAcceptor` tables.
    "nl": ["spelling", "compound_acceptor"],
}

# Default license for files copied from the LanguageTool checkout (rule XML,
# disambiguation, schemas, message bundles, LT-authored word lists, core
# resources). Upstream's pom.xml notes that the LGPL refers to the source code
# while individual resources may be under different licenses, hence
# `license_verified=False` and the per-file exceptions below.
DEFAULT_UPSTREAM_LICENSE = (
    "LGPL-2.1-or-later (LanguageTool resource; per-file license varies, "
    "see licenses/README.md and THIRD_PARTY_NOTICES.md)"
)

# Licenses documented in an upstream file itself, keyed by destination path.
# Values: (license, license_verified, license_source).
UPSTREAM_FILE_LICENSES = {
    "fr/hunspell/README_fr.txt": (
        "MPL-2.0 (Dicollecte French dictionary documentation)",
        True,
        "upstream fr/hunspell/README_fr.txt",
    ),
    "fr/words/README_lexique.txt": (
        "MPL-2.0 (Dicollecte lexique documentation; version 6.4.1)",
        True,
        "upstream fr/README_lexique.txt",
    ),
    "en/words/agid-readme.txt": (
        "Word-list provenance documentation (AGID (c) Kevin Atkinson; "
        "Moby/WordNet terms); exact terms to be confirmed",
        False,
        "upstream en/agid-readme.txt",
    ),
    "en/words/pos-readme.txt": (
        "Word-list provenance documentation (Moby Part-of-Speech II / "
        "WordNet); exact terms to be confirmed",
        False,
        "upstream en/pos-readme.txt",
    ),
    "it/words/tagset.txt": (
        "CC-BY-SA-2.0 OR LGPL (Morph-it! tagset; LICENSING INFORMATION "
        "section in the file)",
        True,
        "upstream it/tagset.txt",
    ),
    "da/words/README.txt": (
        "GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license (Danish tagger data based "
        "on Stavekontrolden; da/README.txt states the tri-license)",
        True,
        "upstream da/README.txt",
    ),
    "sv/words/README.txt": (
        "LGPL-2.1-or-later (Swedish POS data based on DSSO; sv/README.txt "
        "states LGPL-2.1-or-later)",
        True,
        "upstream sv/README.txt",
    ),
    "br/words/README.txt": (
        "GPL (Breton POS data based on the Apertium Breton dictionary, with "
        "permission of its authors; the FSA spelling dictionary is LGPL, "
        "derived from the Drouizig hunspell dictionary)",
        True,
        "upstream br/README.txt",
    ),
}

# In-tree (module resource) dictionaries that are not Maven artifacts.
# Values: (license, license_verified, license_source).
IN_TREE_LICENSES = {
    "it": (
        "CC-BY-SA-2.0 OR LGPL (Morph-it! dual license; upstream it/README.txt, "
        "it/tagset.txt)",
        False,  # dual-licensed; the artifact is a build, owner must confirm terms
        "upstream it/README.txt (Morph-it! statement), it/tagset.txt "
        "(LICENSING INFORMATION)",
    ),
    "gl": (
        "GPL (Freeling/Apertium-derived dictionaries; upstream gl/README.txt "
        "states both source dictionaries are GPL) - to be confirmed",
        False,
        "upstream gl/README.txt",
    ),
    "ro": (
        "LGPL (Romanian POS/synthesis dictionaries; upstream ro/README.txt "
        'states "released here on LGPL license")',
        True,
        "upstream ro/README.txt",
    ),
    "pl": (
        "BSD-2-Clause-style (PoliMorf 2.0 Polish POS/synthesis dictionaries; "
        "upstream pl/README.txt copyright Marcin Miłkowski, redistribution "
        "permitted with the notice)",
        True,
        "upstream pl/README.txt (LICENCE section)",
    ),
    "sk": (
        "LGPL (Slovak POS/synthesis dictionaries built from Slovak National "
        'Corpus data; upstream sk/README.txt states "released here on LGPL '
        'license")',
        True,
        "upstream sk/README.txt",
    ),
    "el": (
        "LGPL (Greek POS/synthesis dictionaries; upstream el/README.txt "
        'states the few test entries are "made available here under LGPL")',
        True,
        "upstream el/README.txt",
    ),
    "da": (
        "GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license (Danish tagger data based on "
        "Stavekontrolden; upstream da/README.txt states the tri-license)",
        True,
        "upstream da/README.txt",
    ),
    "sv": (
        "LGPL-2.1-or-later (Swedish POS/synthesis dictionaries based on DSSO; "
        "upstream sv/README.txt states LGPL-2.1-or-later)",
        True,
        "upstream sv/README.txt",
    ),
    "br": (
        "GPL (Breton POS dictionary based on the Apertium Breton dictionary, "
        "with permission of its authors; upstream br/README.txt states GPL) - "
        "to be confirmed",
        False,
        "upstream br/README.txt",
    ),
}

# Per-language hunspell dictionaries with documented third-party licenses.
# Returns (license, license_verified, license_source); None means the file is
# an LT-authored word list covered by DEFAULT_UPSTREAM_LICENSE.
DE_HUNSPELL_LICENSE = (
    "GPL-2.0-or-later OR GPL-3.0-or-later (igerman98/frami hunspell dictionary; "
    "see hunspell/de_DE_frami_README.txt, COPYING_GPLv2.txt, COPYING_GPLv3.txt)"
)


def hunspell_license(lang: str, name: str):
    if lang == "de" and name.startswith(("de_AT", "de_CH", "de_DE", "COPYING_GPL")):
        # Raw .aff/.dic/.README and the frami/COPYING documentation state the
        # license directly; the morfologik .dict/.info conversions are derived
        # from the same dictionary (documented, but not separately asserted).
        verified = name.endswith((".aff", ".dic", ".README", "_README.txt")) or name.startswith("COPYING_GPL")
        return (DE_HUNSPELL_LICENSE, verified, "upstream de/hunspell/de_DE_frami_README.txt")
    if lang == "it":
        if name == "README_it_IT.txt":
            return (
                "GPL-3.0-only (it_IT hunspell dictionary documentation; "
                "README_it_IT.txt)",
                True,
                "upstream it/hunspell/README_it_IT.txt",
            )
        if name.startswith("it_IT."):
            # Morfologik conversions of the GPL-3 it_IT hunspell dictionary.
            return (
                "GPL-3.0-only (converted from the GPL-3 it_IT hunspell "
                "dictionary; README_it_IT.txt)",
                False,
                "upstream it/hunspell/README_it_IT.txt",
            )
    if lang == "gl":
        if name == "README-gl-ES.txt":
            return (
                "GPL (hunspell-gl_ES / VOLGa dictionary documentation; "
                "README states GPL, LICENCES-en.txt is GPL-3.0) - to be confirmed",
                False,
                "upstream gl/hunspell/README-gl-ES.txt",
            )
        if name.startswith("gl_ES."):
            # Raw .aff/.dic of the VOLGa-based hunspell dictionary.
            return (
                "GPL (hunspell gl_ES VOLGa dictionary; README-gl-ES.txt) - "
                "to be confirmed",
                False,
                "upstream gl/hunspell/README-gl-ES.txt",
            )
    if lang == "ro":
        if name.startswith("README") or name.startswith("COPYING"):
            return (
                "GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license (Romanian spelling "
                "dictionary; ro/hunspell/README_EN.txt + COPYING.*)",
                True,
                "upstream ro/hunspell/README_EN.txt",
            )
        if name.startswith("ro_RO."):
            # Morfologik conversion of the tri-licensed ro_RO hunspell
            # dictionary; the conversion is a derived build.
            return (
                "GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license (converted from the "
                "ro_RO spelling dictionary; ro/hunspell/README_EN.txt)",
                False,
                "upstream ro/hunspell/README_EN.txt",
            )
    if lang == "pl":
        if name.startswith("README"):
            return (
                "GPL / LGPL / MPL / CC-BY-SA (sjp.pl-derived Polish spelling "
                "dictionary; pl/hunspell/README_en.txt)",
                True,
                "upstream pl/hunspell/README_en.txt",
            )
        if name.startswith("pl_PL."):
            # Morfologik conversion of the multi-licensed pl_PL hunspell
            # dictionary; the conversion is a derived build.
            return (
                "GPL / LGPL / MPL / CC-BY-SA (converted from the pl_PL "
                "spelling dictionary; pl/hunspell/README_en.txt)",
                False,
                "upstream pl/hunspell/README_en.txt",
            )
    if lang == "sk":
        if name.startswith("README"):
            return (
                "GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license (sk-spell Slovak "
                "spelling dictionary; sk/hunspell/README_en.txt)",
                True,
                "upstream sk/hunspell/README_en.txt",
            )
        if name.startswith("sk_SK."):
            # Morfologik conversion of the tri-licensed sk_SK hunspell
            # dictionary; the conversion is a derived build.
            return (
                "GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license (converted from the "
                "sk_SK spelling dictionary; sk/hunspell/README_en.txt)",
                False,
                "upstream sk/hunspell/README_en.txt",
            )
    if lang == "sl":
        if name.startswith("README"):
            return (
                "LGPL / GPL dual license (Amebis/Erjavec/Košir/Peterlin "
                "Slovenian spelling dictionary; sl/hunspell/README_sl_SI.txt)",
                True,
                "upstream sl/hunspell/README_sl_SI.txt",
            )
        if name.startswith("sl_SI."):
            # Morfologik conversion of the dual-licensed sl_SI hunspell
            # dictionary; the conversion is a derived build.
            return (
                "LGPL / GPL dual license (converted from the sl_SI spelling "
                "dictionary; sl/hunspell/README_sl_SI.txt)",
                False,
                "upstream sl/hunspell/README_sl_SI.txt",
            )
    if lang == "el":
        if name.startswith("README"):
            return (
                "GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license (el_GR Greek "
                "spelling dictionary; el/hunspell/README_el_GR.txt)",
                True,
                "upstream el/hunspell/README_el_GR.txt",
            )
        if name.startswith("el_GR."):
            # Morfologik conversion of the tri-licensed el_GR hunspell
            # dictionary; the conversion is a derived build.
            return (
                "GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license (converted from the "
                "el_GR spelling dictionary; el/hunspell/README_el_GR.txt)",
                False,
                "upstream el/hunspell/README_el_GR.txt",
            )
    if lang == "da":
        # Stavekontrolden `da_DK` hunspell dictionary; the README and the
        # `.aff` header both state the GPL-2.0/LGPL-2.1/MPL-1.1 tri-license.
        if name.startswith(("README", "da_DK.")):
            verified = name == "README_da_DK.txt" or name.endswith(".aff")
            return (
                "GPL-2.0 / LGPL-2.1 / MPL-1.1 tri-license (Stavekontrolden "
                "da_DK dictionary; da/hunspell/README_da_DK.txt)",
                verified,
                "upstream da/hunspell/README_da_DK.txt",
            )
    if lang == "sv":
        # Swedish `sv_SE` hunspell dictionary (den stora svenska ordlistan);
        # the bundled LICENSE files state LGPL-3.0.
        if name.startswith("sv_SE.") or name.startswith("LICENSE_"):
            verified = name.startswith("LICENSE_") or name.endswith(".aff")
            return (
                "LGPL-3.0-only (Swedish sv_SE spelling dictionary; "
                "sv/hunspell/LICENSE_sv_SE.txt)",
                verified,
                "upstream sv/hunspell/LICENSE_sv_SE.txt",
            )
    if lang == "is":
        # Icelandic `is_IS` hunspell dictionary: the Orðabók Háskólans /
        # Reiknistofnun Háskóla Íslands wordlist is public domain, the
        # Wiktionary-derived morphological additions are CC-BY-SA-3.0;
        # is/hunspell/license.txt documents both.
        if name.startswith(("license", "is_IS.")):
            verified = name.startswith("license") or name.endswith(".aff")
            return (
                "Public domain (Orðabók Háskólans / Reiknistofnun Háskóla "
                "Íslands wordlist) OR CC-BY-SA-3.0 (Icelandic "
                "Wiktionary-derived words); is/hunspell/license.txt",
                verified,
                "upstream is/hunspell/license.txt",
            )
    if lang == "eo":
        # Esperanto `eo` hunspell dictionary from the Esperantilo OpenOffice
        # extension; the `eo.aff` header states GPL-2.0-or-later and
        # `README_eo.txt` documents the source and the Pellé patch.
        if name.startswith("README_eo"):
            return (
                "GPL-2.0-or-later (Esperanto dictionary documentation; "
                "eo/hunspell/README_eo.txt)",
                True,
                "upstream eo/hunspell/README_eo.txt",
            )
        if name.startswith("eo."):
            verified = name.endswith(".aff")
            return (
                "GPL-2.0-or-later (Esperantilo eo spelling dictionary; eo.aff "
                "header, eo/hunspell/README_eo.txt)",
                verified,
                "upstream eo/hunspell/README_eo.txt",
            )
    if lang == "br":
        # The Breton FSA spelling dictionary is generated from the Drouizig
        # hunspell dictionary 0.13 (LGPL); br/hunspell/README.txt points at
        # br/README.txt, which states the LGPL for the FSA dictionary.
        if name.startswith(("README", "br_FR.")):
            verified = name.startswith("README")
            return (
                "LGPL (Breton FSA spelling dictionary generated from the "
                "Drouizig hunspell dictionary 0.13; br/hunspell/README.txt, "
                "br/README.txt)",
                verified,
                "upstream br/hunspell/README.txt; br/README.txt",
            )
    return None


def jar_inner_license(inner: str):
    """License overrides for files inside pinned Maven artifacts."""
    if inner.startswith("org/languagetool/resource/en/hunspell/"):
        # The english-pos-dict artifact bundles hunspell conversions; upstream
        # documents no license for the underlying hunspell dictionaries
        # (SCOWL-derived), so they stay "to be confirmed".
        return (
            "LGPL-2.1-only (artifact POM); underlying hunspell dictionary "
            "provenance not documented upstream - to be confirmed",
            False,
            "upstream english-pos-dict POM; no hunspell license note in the LT checkout",
        )
    if inner.startswith("org/languagetool/resource/ast/"):
        return (
            "GPL-3.0-only (asturian-pos-dict POM; Softastur dictionary, "
            "ast/hunspell/LICENCES-ast.txt)",
            True,
            "asturian-pos-dict POM; bundled ast/hunspell/LICENCES-ast.txt",
        )
    if inner.startswith("org/languagetool/resource/nl/spelling/"):
        return (
            "LGPL-2.1-or-later (nl/spelling/README.txt, TaalTik)",
            True,
            "bundled nl/spelling/README.txt",
        )
    if inner.startswith("org/languagetool/resource/nl/dictionaries/"):
        return (
            "CC-BY-3.0-or-later OR BSD (nl/dictionaries/README.txt, TaalTik); "
            "artifact POM declares LGPL-2.1 - to be confirmed",
            False,
            "bundled nl/dictionaries/README.txt; artifact POM",
        )
    return None


WORDLIST_EXCLUDE = re.compile(r"\.(awk|html|ent)$|^(test|.*test)_", re.IGNORECASE)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def upstream_upstream_dir(upstream: Path) -> Path:
    return upstream.resolve()


def upstream_lang_dir(upstream: Path, lang: str) -> Path:
    return upstream / f"languagetool-language-modules/{lang}/src/main/resources/org/languagetool"


def cmd_baseline(args: argparse.Namespace) -> None:
    upstream = Path(args.upstream).resolve()
    sha = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=upstream, check=True, capture_output=True, text=True
    ).stdout.strip()
    date = subprocess.run(
        ["git", "log", "-1", "--format=%cI"], cwd=upstream, check=True, capture_output=True, text=True
    ).stdout.strip()
    pom = (upstream / "pom.xml").read_text(encoding="utf-8")
    m = re.search(r"<revision>([^<]+)</revision>", pom)
    version = m.group(1) if m else "unknown"
    url = subprocess.run(
        ["git", "remote", "get-url", "origin"], cwd=upstream, check=False, capture_output=True, text=True
    ).stdout.strip() or UPSTREAM_REPO_URL
    url = re.sub(r"^git@github\.com:", "https://github.com/", url)
    doc = {
        "repository": url,
        "baseline_commit": sha,
        "baseline_date": date,
        "lt_version": version,
        "note": "Pinned baseline for all vendored data; rule/category/message ids must never be renumbered.",
    }
    UPSTREAM_JSON.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    print(f"baseline pinned: {sha[:12]} ({date}) version={version} -> {UPSTREAM_JSON}")


def manifest_entry(rel_dest: str, abs_path: Path, source: dict) -> dict:
    return {
        "path": rel_dest,
        "sha256": sha256_file(abs_path),
        "size": abs_path.stat().st_size,
        "source": source,
    }


def load_manifest() -> dict:
    if MANIFEST_JSON.exists():
        return json.loads(MANIFEST_JSON.read_text())
    return {"upstream_commit": None, "file_count": 0, "files": []}


def write_manifest(manifest: dict, generated_by: str) -> None:
    manifest["generated_by"] = generated_by
    manifest["files"].sort(key=lambda e: e["path"])
    manifest["file_count"] = len(manifest["files"])
    MANIFEST_JSON.parent.mkdir(parents=True, exist_ok=True)
    MANIFEST_JSON.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def cmd_add_local(args: argparse.Namespace) -> None:
    manifest = load_manifest()
    files = {e["path"]: e for e in manifest.get("files", [])}
    for raw in args.paths:
        path = Path(raw)
        abs_path = path if path.is_absolute() else REPO_ROOT / path
        if not abs_path.exists():
            print(f"ERROR: not found: {path}", file=sys.stderr)
            sys.exit(1)
        rel = str(abs_path.resolve().relative_to(DATA_DIR.resolve()))
        source: dict = {"kind": args.kind}
        for key in ("license", "license_source", "upstream_path", "url", "generator", "note"):
            value = getattr(args, key)
            if value:
                source[key] = value
        if args.generated_from:
            source["generated_from"] = args.generated_from
        files[rel] = manifest_entry(rel, abs_path, source)
    manifest["files"] = list(files.values())
    write_manifest(manifest, "tools/lt-sync/lt_sync.py add-local")
    print(f"manifest: {manifest['file_count']} files")


def copy_upstream_entry(
    upstream: Path,
    src: Path,
    rel_dest: str,
    entries: list,
    license: str | None = None,
    license_verified: bool = False,
    license_source: str | None = None,
) -> None:
    if license is None:
        override = UPSTREAM_FILE_LICENSES.get(rel_dest)
        if override is not None:
            license, license_verified, license_source = override
    source = {
        "kind": "upstream",
        "upstream_path": str(src.relative_to(upstream)),
        "license": license or DEFAULT_UPSTREAM_LICENSE,
        "license_verified": license_verified,
    }
    if license_source:
        source["license_source"] = license_source
    dest = DATA_DIR / rel_dest
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(src.read_bytes())
    entries.append(manifest_entry(rel_dest, dest, source))


def extract_jar_entries(artifacts_dir: Path, entries: list) -> None:
    import zipfile

    for jar_name, mapping in JAR_EXTRACTIONS.items():
        jar_path = artifacts_dir / jar_name
        if not jar_path.exists():
            print(f"WARNING: artifact not found: {jar_path}", file=sys.stderr)
            continue
        meta = MAVEN_ARTIFACTS[jar_name]
        jar_sha = sha256_file(jar_path)
        with zipfile.ZipFile(jar_path) as zf:
            for inner, rel_dest in mapping.items():
                dest = DATA_DIR / rel_dest
                dest.parent.mkdir(parents=True, exist_ok=True)
                dest.write_bytes(zf.read(inner))
                entry_source = {
                    "kind": "maven-artifact",
                    "coords": meta["coords"],
                    "artifact_sha256": jar_sha,
                    "artifact_inner_path": inner,
                    "license": meta["license"],
                    "license_verified": meta["license_verified"],
                }
                if meta.get("license_source"):
                    entry_source["license_source"] = meta["license_source"]
                override = jar_inner_license(inner)
                if override is not None:
                    entry_source["license"] = override[0]
                    entry_source["license_verified"] = override[1]
                    entry_source["license_source"] = override[2]
                entries.append(manifest_entry(rel_dest, dest, entry_source))


def cmd_import(args: argparse.Namespace) -> None:
    upstream = Path(args.upstream).resolve()
    artifacts_dir = Path(args.artifacts).resolve() if args.artifacts else None
    entries: list = []

    # Schemas
    core_res = upstream / "languagetool-core/src/main/resources/org/languagetool"
    for name in ["rules/rules.xsd", "rules/pattern.xsd", "rules/bitext.xsd", "resource/disambiguation.xsd"]:
        src = core_res / name
        if src.exists():
            copy_upstream_entry(upstream, src, f"schemas/{Path(name).name}", entries)

    # Core runtime resources
    for name in [
        "resource/segment.srx",
        "resource/spelling_global.txt",
        "resource/disambiguation-global.xml",
        # `BaseSynthesizer.createRomanNumberer` reads `/Roman.sor`; the
        # Portuguese `RomanNumeralFilter` needs it (it is the first language
        # to reference Roman numeral synthesis).
        "resource/Roman.sor",
    ]:
        src = core_res / name
        if src.exists():
            copy_upstream_entry(upstream, src, f"core/{Path(name).name}", entries)

    # Message bundles for our languages
    messages = core_res / "MessagesBundle"
    for suffix in [""] + [f"_{lang}" for lang in args.langs]:
        src = Path(f"{messages}{suffix}.properties")
        if src.exists():
            copy_upstream_entry(upstream, src, f"messages/{src.name}", entries)

    # Per-language data
    for lang in args.langs:
        base = upstream_lang_dir(upstream, lang)
        lang_bundle = base / f"MessagesBundle_{lang}.properties"
        if lang_bundle.exists():
            copy_upstream_entry(
                upstream, lang_bundle, f"messages/{lang_bundle.name}", entries
            )
        # Variant-level bundles (`MessagesBundle_pt_PT.properties` etc.; pt
        # ships only these, there is no language-level bundle).
        for variant_bundle in sorted(base.glob(f"MessagesBundle_{lang}_*.properties")):
            copy_upstream_entry(
                upstream, variant_bundle, f"messages/{variant_bundle.name}", entries
            )
        rules_dir = base / "rules" / lang
        if rules_dir.exists():
            for src in sorted(rules_dir.rglob("*")):
                if not src.is_file() or src.suffix not in {".xml", ".txt"}:
                    continue
                rel = src.relative_to(rules_dir)
                copy_upstream_entry(upstream, src, f"{lang}/rules/{rel.as_posix()}", entries)
        disamb = base / "resource" / lang / "disambiguation.xml"
        if disamb.exists():
            copy_upstream_entry(upstream, disamb, f"{lang}/disambiguation.xml", entries)
        res_dir = base / "resource" / lang
        in_tree = IN_TREE_ARTIFACTS.get(lang, {})
        claimed_names = {Path(inner).name for inner in in_tree}
        if res_dir.exists():
            words_dir = DATA_DIR / lang / "words"
            words_dir.mkdir(parents=True, exist_ok=True)
            for src in sorted(res_dir.glob("*.txt")):
                if WORDLIST_EXCLUDE.match(src.name) or src.name in claimed_names:
                    continue
                copy_upstream_entry(upstream, src, f"{lang}/words/{src.name}", entries)
            for src in sorted(res_dir.glob("*.ent")):
                copy_upstream_entry(upstream, src, f"{lang}/words/{src.name}", entries)
            # German extra resources (old-spelling table, number speller)
            for src in sorted(res_dir.glob("*.csv")):
                copy_upstream_entry(upstream, src, f"{lang}/words/{src.name}", entries)
            for src in sorted(res_dir.glob("*.sor")):
                copy_upstream_entry(upstream, src, f"{lang}/{src.name}", entries)
            # Language-specific resource subdirectories that rule data references
            # by relative path: XML entity files (`<lang>/entities/*.ent`, pt)
            # and tab-separated data tables (pt `brazilian_municipalities/*.tsv`).
            for src in sorted(res_dir.rglob("*")):
                if not src.is_file() or src.suffix not in {".ent", ".tsv"}:
                    continue
                if src.parent == res_dir:
                    continue  # flat *.ent already handled above
                rel = src.relative_to(res_dir)
                copy_upstream_entry(upstream, src, f"{lang}/{rel.as_posix()}", entries)
            # Word lists referenced by Java rule classes (nl `spelling/*.txt`,
            # `compound_acceptor/*.txt`); the module-relative path is kept.
            for sub in LANG_RESOURCE_SUBDIRS.get(lang, []):
                sub_dir = res_dir / sub
                if not sub_dir.exists():
                    continue
                for src in sorted(sub_dir.rglob("*")):
                    if not src.is_file() or src.suffix not in {".txt", ".csv"}:
                        continue
                    rel = src.relative_to(res_dir)
                    copy_upstream_entry(upstream, src, f"{lang}/{rel.as_posix()}", entries)
            hunspell = res_dir / "hunspell"
            if hunspell.exists():
                for src in sorted(hunspell.rglob("*")):
                    # German ships the raw hunspell dictionaries (.aff/.dic)
                    # alongside the morfologik .dict files; the rules use the
                    # raw pair via the native hunspell binding. The
                    # `.dic.header` build artifacts are not used at runtime and
                    # stay unvendored.
                    if src.is_file() and src.suffix in {
                        ".txt", ".dict", ".info", ".aff", ".dic", ".README",
                    }:
                        rel = src.relative_to(hunspell)
                        # The igerman98/frami dictionary files carry their own
                        # GPLv2/GPLv3 license; the it_IT dictionary is GPL-3.
                        # The LT-authored word lists around them stay under the
                        # default resource license.
                        hunspell_meta = hunspell_license(lang, src.name)
                        lic, verified, lic_source = (
                            hunspell_meta if hunspell_meta is not None else (None, False, None)
                        )
                        copy_upstream_entry(
                            upstream,
                            src,
                            f"{lang}/hunspell/{rel.as_posix()}",
                            entries,
                            license=lic,
                            license_verified=verified,
                            license_source=lic_source,
                        )

        # In-tree Morfologik dictionaries instead of a Maven artifact (it).
        in_tree_license = IN_TREE_LICENSES.get(lang, (None, False, None))
        for inner, rel_dest in in_tree.items():
            src = base / inner
            if src.exists():
                copy_upstream_entry(
                    upstream,
                    src,
                    rel_dest,
                    entries,
                    license=in_tree_license[0],
                    license_verified=in_tree_license[1],
                    license_source=in_tree_license[2],
                )
            else:
                print(f"WARNING: in-tree artifact not found: {src}", file=sys.stderr)

    # Dictionaries / models from Maven artifacts
    if artifacts_dir:
        extract_jar_entries(artifacts_dir, entries)

    # Keep data that is not imported from upstream (hand-authored, vendor,
    # generated) across imports.
    previous = load_manifest()
    imported_paths = {e["path"] for e in entries}
    for entry in previous.get("files", []):
        if entry.get("source", {}).get("kind") in LOCAL_KINDS and entry["path"] not in imported_paths:
            entries.append(entry)

    manifest = {
        "upstream_commit": json.loads(UPSTREAM_JSON.read_text())["baseline_commit"]
        if UPSTREAM_JSON.exists()
        else None,
        "file_count": len(entries),
        "files": entries,
    }
    write_manifest(manifest, "tools/lt-sync/lt_sync.py import")
    print(f"imported {len(entries)} files -> {MANIFEST_JSON}")


CLASS_RULE_XML = "rule-xml"
CLASS_DISAMBIG_XML = "disambiguation-xml"
CLASS_WORDLIST = "wordlist"
CLASS_DICT_MODEL = "dictionary-or-model"
CLASS_SCHEMA = "schema"
CLASS_MESSAGES = "messages"
CLASS_JAVA = "java"
CLASS_TEST = "test"
CLASS_OTHER = "other"


def classify_upstream_path(rel: str) -> str:
    p = rel.lower()
    if p.endswith(".xsd"):
        return CLASS_SCHEMA
    if "disambiguation" in p and p.endswith(".xml"):
        return CLASS_DISAMBIG_XML
    if re.search(r"rules/(en|de|es|fr|it|pt|nl|ca|gl|ro|pl|sk|sl|el|da|sv|is|eo|ast|br)/.*\.xml$", p):
        return CLASS_RULE_XML
    if p.endswith((".dict", ".info", ".bin")):
        return CLASS_DICT_MODEL
    if "/messagesbundle" in p:
        return CLASS_MESSAGES
    if p.endswith(".java"):
        return CLASS_TEST if "src/test" in p else CLASS_JAVA
    if p.endswith(".txt"):
        return CLASS_WORDLIST
    return CLASS_OTHER


def cmd_status(args: argparse.Namespace) -> None:
    if not MANIFEST_JSON.exists():
        print("no manifest; run import first", file=sys.stderr)
        sys.exit(2)
    manifest = json.loads(MANIFEST_JSON.read_text())
    if args.upstream:
        upstream = Path(args.upstream).resolve()
    else:
        upstream = Path(args.upstream) if args.upstream else None
    if upstream is None:
        print("status requires --upstream", file=sys.stderr)
        sys.exit(2)

    changed, removed, same, missing_local = [], [], [], []
    for entry in manifest["files"]:
        src = entry["source"]
        local = DATA_DIR / entry["path"]
        if not local.exists():
            missing_local.append(entry["path"])
            continue
        if src.get("kind") != "upstream":
            continue
        up = upstream / src["upstream_path"]
        if not up.exists():
            removed.append((entry["path"], classify_upstream_path(src["upstream_path"])))
        elif sha256_file(up) != entry["sha256"]:
            changed.append((entry["path"], classify_upstream_path(src["upstream_path"])))
        else:
            same.append(entry["path"])

    # New upstream files not yet vendored
    added = []
    for lang in LANGS:
        base = upstream_lang_dir(upstream, lang)
        rules_dir = base / "rules" / lang
        if rules_dir.exists():
            vendored = {e["path"] for e in manifest["files"]}
            for src in rules_dir.rglob("*.xml"):
                rel = f"{lang}/rules/{src.relative_to(rules_dir).as_posix()}"
                if rel not in vendored:
                    added.append((rel, CLASS_RULE_XML))

    summary = {
        "unchanged": len(same),
        "changed": len(changed),
        "removed": len(removed),
        "added_upstream": len(added),
        "missing_local": len(missing_local),
    }
    by_class: dict = {}
    for path, cls in changed + removed + added:
        by_class.setdefault(cls, []).append(path)
    print(json.dumps({"summary": summary, "by_class": {k: len(v) for k, v in sorted(by_class.items())}}, indent=2))
    detail_path = REPO_ROOT / "docs/parity/lt-sync-status.json"
    detail_path.parent.mkdir(parents=True, exist_ok=True)
    detail_path.write_text(
        json.dumps(
            {"summary": summary, "changed": changed, "removed": removed, "added": added, "missing_local": missing_local},
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"details -> {detail_path}")


def cmd_report(args: argparse.Namespace) -> None:
    out = REPO_ROOT / "docs/parity/porting-checklist.md"
    out.parent.mkdir(parents=True, exist_ok=True)
    status_detail = REPO_ROOT / "docs/parity/lt-sync-status.json"
    lines = [
        "# Porting checklist (generated by lt-sync report)",
        "",
        "Regenerate with `python3 tools/lt-sync/lt_sync.py status --upstream <checkout>` then `report`.",
        "",
    ]
    if status_detail.exists():
        d = json.loads(status_detail.read_text())
        for path, cls in d.get("changed", []) + d.get("added", []):
            lines.append(f"- [ ] ({cls}) {path}")
    else:
        lines.append("No status data yet; run `lt-sync status` first.")
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"report -> {out}")


def cmd_fetch(args: argparse.Namespace) -> None:
    target = Path(args.to or "HEAD")
    tmp = Path(args.dir or "/tmp/kilo/lt-upstream")
    if not (tmp / ".git").exists():
        subprocess.run(["git", "clone", UPSTREAM_REPO_URL, str(tmp)], check=True)
    subprocess.run(["git", "-C", str(tmp), "fetch", "origin", target], check=True)
    print(f"fetched {target} into {tmp}; use `lt-sync status --upstream {tmp}` to classify the delta")


def main() -> None:
    ap = argparse.ArgumentParser(prog="lt-sync", description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("baseline", help="pin upstream baseline commit")
    p.add_argument("--upstream", required=True, help="path to the upstream git checkout")
    p.set_defaults(fn=cmd_baseline)

    p = sub.add_parser("import", help="vendor data from upstream checkout + maven artifacts")
    p.add_argument("--upstream", required=True)
    p.add_argument("--artifacts", help="directory with pre-downloaded Maven artifact jars")
    p.add_argument("--langs", nargs="+", default=LANGS, choices=LANGS)
    p.set_defaults(fn=cmd_import, langs=LANGS)

    p = sub.add_parser("status", help="classify upstream delta vs pinned manifest")
    p.add_argument("--upstream", required=True)
    p.set_defaults(fn=cmd_status)

    p = sub.add_parser(
        "add-local",
        help="record hand-authored/vendor/generated data in the manifest",
    )
    p.add_argument("kind", choices=LOCAL_KINDS)
    p.add_argument("paths", nargs="+")
    p.add_argument("--license")
    p.add_argument("--license-source")
    p.add_argument("--upstream-path")
    p.add_argument("--url")
    p.add_argument("--generator")
    p.add_argument("--generated-from", nargs="*")
    p.add_argument("--note")
    p.set_defaults(fn=cmd_add_local)

    p = sub.add_parser("report", help="emit porting checklist")
    p.set_defaults(fn=cmd_report)

    p = sub.add_parser("fetch", help="clone/fetch upstream repository")
    p.add_argument("--to", help="commit or tag to fetch")
    p.add_argument("--dir", help="target directory (default /tmp/kilo/lt-upstream)")
    p.set_defaults(fn=cmd_fetch)

    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
