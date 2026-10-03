# Wikipedia detection corpus

Authentic sentences used to calibrate the language-detection feature for the
languages whose LanguageTool grammar file yields no `<example>` text. They are
the fallback source for `tools/detection/generate.py`, which prefers
`data/<lang>/rules/grammar.xml` and reads this cache only when that file produces
nothing.

## Licence

Wikipedia article text is licensed **CC BY-SA 3.0** ([Wikimedia Terms of
Use](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use)); the owner
approved vendoring it for this project. This file and the per-file record in
`data/manifest.json` (`license`, `license_verified`, `license_source`, exact API
`url`, `sha256`) are that attribution.

## Files

- [`ca.txt`](ca.txt) — `ca.wikipedia.org`
- [`es.txt`](es.txt) — `es.wikipedia.org`
- [`pt.txt`](pt.txt) — `pt.wikipedia.org`
- [`sr.txt`](sr.txt) — `sr.wikipedia.org`
- [`uk.txt`](uk.txt) — `uk.wikipedia.org`

One sentence per line, de-duplicated, 40–300 characters. `<lang>.json` beside each
file records the exact API request, the articles the sentences came from and when
they were fetched. Retrieved 2026-10-03.

## Regenerating

```sh
python3 tools/detection/fetch_wikipedia.py            # these languages
python3 tools/detection/generate.py                   # regenerate the fixtures
python3 tools/detection/generate.py --check           # committed artefacts current?
```

The fetch needs network access; `generate.py --check` does not, because this
cache is committed. Re-fetching changes the sentences and therefore the fixture
and the manifest's sha256 values.
