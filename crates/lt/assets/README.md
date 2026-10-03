# Bundled model assets

## `lid.176.ftz`

fastText's language-identification model, by Facebook AI Research.

- **Upstream**: <https://fasttext.cc/docs/en/language-identification.html>
- **Downloaded from**: `https://dl.fbaipublicfiles.com/fasttext/supervised-models/lid.176.ftz`
- **SHA-256**: `8f3472cfe8738a7b6099e8e999c3cbfae0dcd15696aac7d7738a8039db603e83`
- **Size**: 938 013 bytes (916 kB), the compressed `.ftz` variant rather than the
  126 MB `lid.176.bin`
- **Licence**: CC BY-SA 3.0 — trained on Wikipedia, Tatoeba and SETimes data
- **Coverage**: 176 languages

It is committed rather than fetched so the browser build never makes a network
request for it, in line with the add-on's offline guarantee. That also makes the
licence a property of this repository: CC BY-SA 3.0 is already the licence of
data we ship (`data/manifest.json` records 14 CC-BY-SA-4.0 files and ~54 GPL
files), and the engine itself is `LGPL-2.1-or-later`.

Nothing here is modified. `docs/language-detection-plan.md` §7 records the
review; a fine-tuned derivative (§3.3) would be a separate asset with its own
provenance.

## Measured behaviour

Worth recording, because it drove the design:

- Loads in 7.7 ms; predicts ~45 µs for 660 characters, single-threaded.
- `dim = 16`, `bucket = 2,000,000`, `nwords = 7,235`, `nlabels = 176`.
- The hashed n-gram bucket table is 99.6 % of the model, so **label count does
  not drive file size** — `bucket` and `dim` do.

It does not cover every language we ship. Nominal coverage is 36 of 38: Crimean
Tatar is absent, and so is Nordum (our own constructed language). It also has the
right label available and declines to use it for several languages — `ast`
reported as `es`/`gl`, `no` as `da`, `nn` as `no`. See
`docs/language-detection-plan.md` §2 for the measurements.