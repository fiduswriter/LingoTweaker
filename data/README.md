# data/ — vendored runtime data and provenance

Everything the engine reads at runtime lives in this directory. Most of it is
**generated** by `tools/lt-sync/lt_sync.py`; hand-authored language data
(`hand-authored` entries) is edited in place and re-registered with
`add-local` (see below). The engine locates the directory via `LT_DATA_DIR` or
`./data` when run from the repository root (`lt-data` crate), and verifies every
listed file against `manifest.json` in its test suite.

## Layout

| Path | Contents |
|------|----------|
| `manifest.json` | per-file provenance: upstream path, sha256, size, license |
| `core/` | `segment.srx`, `spelling_global.txt`, `disambiguation-global.xml`, `Roman.sor` |
| `schemas/` | rule/pattern/bitext/disambiguation XSDs |
| `messages/` | `MessagesBundle` properties bundles |
| `<lang>/rules/` | rule and style XML (+ variant rule files, text tables) |
| `<lang>/disambiguation.xml` | language disambiguation file |
| `<lang>/words/` | word lists referenced by rules/Java classes |
| `<lang>/dictionaries/` | Morfologik POS/synthesis dictionaries |
| `<lang>/hunspell/` | hunspell speller dictionaries and word lists |
| `<lang>/spelling/`, `<lang>/compound_acceptor/` | speller word lists (nl) |
| `<lang>/*.sor` | serialized text resources (e.g. German special-case table) |
| `en/models/` | OpenNLP model containers (token/pos/chunk) |

Languages currently vendored: `en`, `de`, `es`, `fr`, `it`, `pt`, `nl`, `ca`,
`gl`, `ro`, `pl`, `sk`, `sl`, `el`, `no`, `nrd`, `gn`.

## How it is generated

`upstream.json` (repository root) pins the LanguageTool repository and the exact
commit all files came from. `data/` is produced by `tools/lt-sync/lt_sync.py` from
that checkout plus a directory of pre-downloaded Maven artifact jars:

```sh
python3 tools/lt-sync/lt_sync.py baseline --upstream /path/to/languagetool
python3 tools/lt-sync/lt_sync.py import  --upstream /path/to/languagetool \
                                          --artifacts /path/to/maven-jars
python3 tools/lt-sync/lt_sync.py status  --upstream /path/to/languagetool
python3 tools/lt-sync/lt_sync.py report
```

`import` copies the LT resources, extracts the pinned Maven artifacts
(English/German/Spanish/French/Portuguese/Dutch POS dictionaries, jWordSplitter
word lists, OpenNLP models) and rewrites `manifest.json`. The artifact
coordinates and their extraction maps are pinned in `lt_sync.py`
(`MAVEN_ARTIFACTS`, `JAR_EXTRACTIONS`, `IN_TREE_ARTIFACTS`); the artifact jars
themselves are not committed. `status` classifies upstream drift and writes the
gitignored `docs/parity/lt-sync-status.json`; `report` turns that into a porting
checklist.

## manifest.json

`manifest.json` records the pinned upstream commit (`upstream_commit`), the
generator, and one entry per file:

```json
{
  "path": "de/dictionaries/german.dict",
  "sha256": "…",
  "size": 123456,
  "source": {
    "kind": "maven-artifact",
    "coords": "de.danielnaber:german-pos-dict:1.2.4",
    "artifact_sha256": "…",
    "artifact_inner_path": "org/languagetool/resource/de/german.dict",
    "license": "CC-BY-SA-4.0 (artifact POM, data based on Morphy)",
    "license_verified": true,
    "license_source": "https://repo1.maven.org/maven2/de/danielnaber/german-pos-dict/1.2.4/german-pos-dict-1.2.4.pom"
  }
}
```

- `kind` is `upstream` (copied from the checkout, with `upstream_path`) or
  `maven-artifact` (extracted from a jar, with `coords`, `artifact_sha256`,
  `artifact_inner_path`).
- `kind` `hand-authored` (written for LingoTweaker), `vendor` (third-party
  dictionary copied verbatim, with `url`/`license`) or `generated` (produced by
  a tool, with `generator`/`generated_from`) records data that has no upstream
  counterpart. `lt_sync.py add-local <kind> <paths...>` writes those entries and
  `import` preserves them across upstream syncs.
- `license` is the component's license with a short note; `license_verified` is
  `true` only when the license is documented at `license_source` (POM URL and/or
  upstream README path). `license_verified: false` plus a "to be confirmed" note
  means the owner still has to decide — see `THIRD_PARTY_NOTICES.md`.
- Files copied from the LT checkout default to the LT resource license
  (`LGPL-2.1-or-later`, per-file exceptions recorded via
  `UPSTREAM_FILE_LICENSES`/`hunspell_license()` in `lt_sync.py`).

The `sha256` values are the contract: `cargo test -p lt-data` fails if a vendored
file no longer matches. Regenerating with the pinned checkout and artifacts must
therefore not change any `sha256`, `size`, or source path.

Morfologik `.dict` files can also be rebuilt without Java with the pure-Python
port of the LanguageTool/Morfologik dictionary tools
(`python3 tools/morfologik/lt_morfologik.py pos|spell|synth|dict_compile`,
see `tools/morfologik/README.md`). Its output is byte-for-byte identical to the
Java tooling, so a regenerated `.dict` keeps the manifest `sha256`.

## Licensing

Vendored files keep their upstream licenses; the project's own code is
LGPL-2.1-or-later. See `THIRD_PARTY_NOTICES.md` for the component table and
`licenses/README.md` for the license texts and the distribution model.
