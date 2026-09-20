# tools/morfologik — pure-Python dictionary tooling

A JVM-free port of the dictionary builders that LanguageTool ships in
[`languagetool-tools`](https://github.com/languagetool-org/languagetool/tree/master/languagetool-tools)
and of the [Morfologik](https://github.com/morfologik/morfologik-stemming)
compile/decompile tools they wrap. It builds and reads the Morfologik `.dict`
files (`CFSA2`, plus reading the legacy `FSA5` format) that the engine's tagger
and speller load, so dictionary data can be regenerated without Java/Maven.

The output is **byte-for-byte identical** to the Java tooling for the same
input and metadata (verified against `morfologik-tools` 2.2.0 and the
LanguageTool 6.6 `languagetool-tools`; see `tests/`).

## Why Python

The builders are a development-time step, not part of the runtime engine, so a
faithful Python port keeps the toolchain small and reproducible. The hard part
is the Morfologik FSA compiler (Daciuk's incremental minimal automaton plus the
`CFSA2` serializer); it is ported directly from the Java sources.

## Commands

Run from the repository root:

```sh
# Morfologik-level tools
python3 tools/morfologik/lt_morfologik.py fsa_compile -i words.txt -o words.dict
python3 tools/morfologik/lt_morfologik.py fsa_decompile -i words.dict -o words.out
python3 tools/morfologik/lt_morfologik.py dict_compile -i dict.txt -o dict.dict
python3 tools/morfologik/lt_morfologik.py dict_decompile -i dict.dict -o dict.txt

# LanguageTool builders (wrap the commands above, like the Java classes do)
python3 tools/morfologik/lt_morfologik.py pos   -i word-lemma-tag.txt --info xx.info -o xx.dict
python3 tools/morfologik/lt_morfologik.py pos   -i ... --info xx.info --freq freq.xml -o xx.dict
python3 tools/morfologik/lt_morfologik.py spell -i words.txt --info xx.info -o xx.dict
python3 tools/morfologik/lt_morfologik.py synth -i word-lemma-tag.txt --info xx_synth.info -o xx_synth.dict
python3 tools/morfologik/lt_morfologik.py export -i xx.dict --info xx.info -o xx.txt
```

`dict_compile` reads the metadata from the input file's sibling `.info`
(extension swapped), exactly like `morfologik.tools.DictCompile`; the output
defaults to the `.info` path with `.dict`.

### Input formats

* `fsa_compile` / `spell`: one sequence per line.
* `dict_compile` / `pos` / `synth`: tab-separated `wordform<tab>lemma<tab>postag`
  for the LanguageTool builders; `dict_compile` itself takes
  `base<sep>inflected[<sep>tag]` using the separator declared in the `.info`.
* `pos`/`spell` accept an optional `--freq` Morfologik frequency wordlist
  (`<w f="N">word</w>`); the `.info` must set
  `fsa.dict.frequency-included=true`.

## Layout

| Module | Ports |
|--------|-------|
| `morfologik/fsa.py` | `morfologik.fsa.{FSA,CFSA2,FSA5,ByteSequenceIterator}` (read side) |
| `morfologik/builder.py` | `morfologik.fsa.builders.FSABuilder` + `ConstantArcSizeFSA` |
| `morfologik/serializer.py` | `morfologik.fsa.builders.CFSA2Serializer` (write side) |
| `morfologik/encoders.py` | `morfologik.stemming` sequence encoders (`SUFFIX`/`PREFIX`/`INFIX`/`NONE`) |
| `morfologik/metadata.py` | `morfologik.stemming.DictionaryMetadata` (`.info` parsing) |
| `morfologik/dictionary.py` | `Dictionary`/`DictionaryIterator` (decompilation) |
| `morfologik/tools.py` | `morfologik.tools.{FSACompile,DictCompile,FSADecompile,DictDecompile}` |
| `morfologik/lt.py` | `languagetool-tools` `POSDictionaryBuilder`, `SpellDictionaryBuilder`, `SynthDictionaryBuilder`, `DictionaryExporter` |
| `morfologik/cli.py` | the `lt_morfologik.py` command-line interface |

## Limitations

* The **writer** only emits `CFSA2` (the format LanguageTool compiles its
  dictionaries with; `languagetool-tools` hardcodes it). The legacy `FSA5`
  format is supported for **reading** old dictionaries such as the vendored
  `it/dictionaries/italian.dict`.
* Only the dictionary-relevant parts of the Morfologik API are ported; there is
  no speller/`DictionaryLookup` query surface beyond dictionary iteration.
* Performance is adequate for a development tool but not optimized: compiling
  the ~700k-entry Norwegian list takes about 20 s, and the largest vendored
  dictionaries (a few million entries) a few minutes.

## Tests

```sh
python3 -m unittest discover -s tools/morfologik/tests
```

The tests are self-contained and require no JVM; golden `.dict` blobs pin the
byte-for-byte format. To re-verify against Java (needs the `morfologik-tools`
2.2.0 jars and, for the LanguageTool builders, a `languagetool-tools` build),
use `tests/compare_with_java.py`.

## License

Project code, LGPL-2.1-or-later (see the repository `LICENSE`). The ported
algorithms come from Morfologik (BSD-3-Clause) and the LanguageTool tools
(LGPL-2.1-or-later); see `THIRD_PARTY_NOTICES.md`.
