# Japanese (`ja`) data

## Rules

`rules/grammar.xml` — imported from the upstream LanguageTool `ja` module
(`languagetool-language-modules/ja/src/main/resources/org/languagetool/rules/ja/grammar.xml`,
LGPL-2.1-or-later). 735 rules; the three rules with a second spelling that
Lindera tokenizes into a different token count (`MATIDOUSII`, `ZURAI`,
`KURERU`) are `<or>`/`<rulegroup>` variants, so the file loads 737 rule
definitions.

## Dictionary (`dictionary/`, generated — not in git)

Segmentation/tagging use Lindera 6 with the mecab-IPADIC dictionary. The
compiled dictionary is a **generated artifact** (tens of MB) and is gitignored
(`data/*/dictionary/`). Generate or verify it with:

```sh
python3 tools/lindera/build-dict.py ja          # download + extract if missing
python3 tools/lindera/build-dict.py --check ja  # byte-verify against the pinned archive
```

Provenance:

- Lindera version: 6.0.0
- Release asset: `lindera-ipadic-6.0.0.zip`
- SHA-256: `8433dbbb80d7588a565fb9247c1ac7aed3ca50c7463329e495f8bd905aece356`
- Dictionary: mecab-ipadic 2.7.0-20070801 (NAIST/ICOT notice; see
  `dictionary/NOTICE.txt`, extracted from the archive)

The release archive is byte-identical to a fresh Lindera `embed-ipadic` build of
the same version (the generator verifies this on `--check`).

At runtime the dictionary is read through `lt_data::fs`, so it loads from a
data pack / on wasm; `scripts/data/build-packs.sh` packs `ja/**` including
`dictionary/**`.