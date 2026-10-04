# Wikipedia discriminator training corpus

Authentic sentences used to train the confusable-set discriminator, the third
detection layer that is consulted only when the primary model's answer falls
inside the confusable set. The languages are the confusable set plus `tr`, which
is a negative class rather than a language we ship.

This is **not** the calibration corpus. `../wikipedia` feeds
`crates/lt-core/tests/fixtures/detection_corpus.json`, which is what the detector
is *measured* on, so nothing here may ever be read by `generate.py`: a sentence
that trains the discriminator and also calibrates it would make every reported
accuracy meaningless. `train_discriminator.py` enforces that by dropping any
sentence that appears in the fixture, and by holding out a fixed tenth of each
file for validation.

## Licence

Wikipedia article text is licensed **CC BY-SA 3.0** ([Wikimedia Terms of
Use](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use)); the owner
approved vendoring it for this project. This file and the per-file record in
`data/manifest.json` (`license`, `license_verified`, `license_source`, exact API
`url`, `sha256`) are that attribution.

## Files

- [`ast.txt`](ast.txt) — `ast.wikipedia.org`
- [`ca.txt`](ca.txt) — `ca.wikipedia.org`
- [`crh.txt`](crh.txt) — `crh.wikipedia.org`
- [`da.txt`](da.txt) — `da.wikipedia.org`
- [`es.txt`](es.txt) — `es.wikipedia.org`
- [`fr.txt`](fr.txt) — `fr.wikipedia.org`
- [`gl.txt`](gl.txt) — `gl.wikipedia.org`
- [`is.txt`](is.txt) — `is.wikipedia.org`
- [`it.txt`](it.txt) — `it.wikipedia.org`
- [`nn.txt`](nn.txt) — `nn.wikipedia.org`
- [`no.txt`](no.txt) — `no.wikipedia.org`
- [`pt.txt`](pt.txt) — `pt.wikipedia.org`
- [`ro.txt`](ro.txt) — `ro.wikipedia.org`
- [`sv.txt`](sv.txt) — `sv.wikipedia.org`
- [`tr.txt`](tr.txt) — `tr.wikipedia.org`

One sentence per line, de-duplicated, 40–300 characters. `<lang>.json` beside each
file records the exact API request, the articles the sentences came from and when
they were fetched. Retrieved 2026-10-04.

## Regenerating

```sh
python3 tools/detection/fetch_wikipedia.py --purpose training
python3 tools/detection/train_discriminator.py            # train + validate
python3 tools/detection/train_discriminator.py --evaluate # re-score a model
```

The fetch needs network access. Re-fetching changes the sentences and therefore
the trained model, so the committed model must be retrained and re-validated with
it; `train_discriminator.py` prints the manifest's sha256 values it trained
against for exactly that reason.
