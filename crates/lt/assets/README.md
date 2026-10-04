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

Nothing here is modified — the file is upstream's, byte for byte, and the
licence above is the licence it ships under. A fine-tuned or retrained
derivative would be a separate asset with its own provenance.

## Measured behaviour

Worth recording, because it drove the design:

- Loads in 7.7 ms; predicts ~45 µs for 660 characters, single-threaded.
- `dim = 16`, `bucket = 2,000,000`, `nwords = 7,235`, `nlabels = 176`.
- The hashed n-gram bucket table is 99.6 % of the model, so **label count does
  not drive file size** — `bucket` and `dim` do.

It does not cover every language we ship. Nominal coverage is 36 of 38: Crimean
Tatar is absent, and so is Nordum (our own constructed language). It also has the
right label available and declines to use it for several languages — `ast`
reported as `es`/`gl`, `no` as `da`, `nn` as `no`. `cargo test -p lt --test
detect_model -- --nocapture` prints the per-language measurements.

## `discriminator.ftz`

Our own fastText model, covering the confusable neighbourhood `lid.176.ftz` gets
wrong. Trained from scratch; not derived from `lid.176.ftz` in any way.

- **Trained by**: `tools/detection/train_discriminator.py`, using the upstream C++
  trainer that `tools/detection/build_fasttext.sh` builds
- **Training data**: the vendored Wikipedia sentence caches in
  `data/detection/training/` — 49 304 sentences across the 15 confusable labels,
  of which a fixed tenth per language is held out and never trained on, and every
  sentence that appears verbatim in `detection_corpus.json` is dropped so the
  fixture stays an independent measurement
- **Geometry**: `dim 16`, `bucket 200000`, `epoch 30`, `lr 0.5`, `minCount 50`,
  `-cutoff 50000`, quantized with `-qnorm -retrain`
- **Size**: 875 114 bytes (855 kB)
- **Licence**: CC BY-SA 3.0, inherited from the Wikipedia text it is trained on —
  the same terms as `lid.176.ftz`, and the owner approved releasing a derivative
  on those terms
- **Labels**: `es ast ca gl pt fr it ro da sv no nn is tr crh` — the confusable set
  plus `tr` as a negative class

Two things about the size, because both are counter-intuitive. Label count does
not drive it (the bucket table dominates, as above). And quantization alone is not
enough: without `-cutoff`, `minCount 1` keeps a 264 000-word vocabulary and the
same model ships at 10 MB instead of 855 kB. Pruning is the lever, and it is why
`minCount` is 50.

Measured against the primary model on the fixture, with the engine's gates:
correct answers 66.5 % → 68.6 %, wrong 2.8 % → 3.2 %, at 855 kB. Per-language
figures and the operating-point grid are in the `detect_refiner` module docs and
printed by `tools/detection/train_discriminator.py`.

Regenerating it means re-running `tools/detection/fetch_wikipedia.py --purpose
training` and then `tools/detection/train_discriminator.py`; the tool records the
sha256 of every corpus file it trained against in `metrics.json`, so a model and
the data it came from can always be matched up.