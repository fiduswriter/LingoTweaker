# Chinese (`zh`) data

## Rules

`rules/grammar.xml` — imported from the upstream LanguageTool `zh` module
(`languagetool-language-modules/zh/src/main/resources/org/languagetool/rules/zh/grammar.xml`,
LGPL-2.1-or-later). The upstream file had 1,863 rule definitions (634 top-level
rules + 345 rulegroups) and 3,730 examples; a per-rule triage removed 41 rules
that could not work with the jieba tagger (see `docs/differences.md` §14), so
the file now has 1,822 rule definitions.

The upstream module also ships `resource/zh/common_words.txt` and
`resource/zh/confusion_sets.txt`, used by `ChineseConfusionProbabilityRule`, a
**language-model** rule (`getRelevantLanguageModelRules`). It needs an n-gram
language model, is not shipped with the data packs and is not ported.

## Dictionary (`dictionary/`, generated — not in git)

Segmentation/tagging use Lindera 6 with the mecab-jieba dictionary. The
compiled dictionary is a **generated artifact** and is gitignored
(`data/*/dictionary/`). Generate or verify it with:

```sh
python3 tools/lindera/build-dict.py zh          # download + extract if missing
python3 tools/lindera/build-dict.py --check zh  # byte-verify against the pinned archive
```

Provenance:

- Lindera version: 6.0.0
- Release asset: `lindera-jieba-6.0.0.zip`
- SHA-256: `ad37064b2ff8c738342c80229e59dac2845747f8e475b4223f07c926db72a9fd`
- Dictionary: mecab-jieba 0.1.1 — jieba's `dict.txt.big` word-frequency
  dictionary (MIT) enriched with CC-CEDICT data (pinyin/traditional/simplified/
  English definitions, CC BY-SA 4.0). See `dictionary/NOTICE.txt`.

At runtime the dictionary is read through `lt_data::fs`, so it loads from a
data pack / on wasm. Sentence splitting ports HanLP's `SentencesUtil` (Java's
`ChineseSentenceTokenizer` does not use SRX).