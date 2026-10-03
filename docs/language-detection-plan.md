# Language detection: research and implementation plan

**Status:** revised after empirical benchmarking (2026-10-03). Supersedes the earlier
library-only comparison; all numbers below are measured, not quoted. Model licensing is
**deferred to a release gate** by owner decision — see §7.

## Summary

Use **fastText `lid.176.ftz` bundled into the WASM build** as the primary statistical layer,
with two narrow specialists in front of and behind it: a **lexicon for Nordum** (the one
language with no corpus anywhere) and a **discriminator model for the confusable clusters**
where fastText has the right label available and declines to use it. whatlang was benchmarked
and rejected: it is unusable on short text, which is exactly what a grammar checker sees.

Neither specialist is an optional refinement. Benchmarking showed fastText assigns *high*
confidence to wrong answers for several of our languages, so a confidence gate alone cannot
catch them.

## 1. What LanguageTool exposes

Verified against the pinned Java checkout in `../languagetool`.

**There is no detection endpoint.** `ApiV2.handleRequest` (`ApiV2.java:78-100`) dispatches
exactly `languages`, `maxtextlength`, `configinfo`, `info`, `check`, `words`, `words/add`,
`words/delete`. Detection happens *inside* `POST /v2/check` when `language=auto`, and is
reported in the response (`RuleMatchesAsJsonSerializer.java:171-179`):

```json
"detectedLanguage": { "name": "…", "code": "…", "confidence": 0.0,
                      "spellCheckOnly": false, "source": "ngram" }
```

plus per-sentence `extendedSentenceRanges[].detectedLanguages[] = {language, rate}`
(`RuleMatchesAsJsonSerializer.java:244-262`).

On fastText: the **open-source server does not use it**. `TextChecker.java:993-1033`
delegates to `LanguageIdentifier`, and `TextChecker.java:381` shows `source` is the literal
`"ngram"`. fastText is a LanguageTool Premium feature. So matching OSS behaviour means an
n-gram style method — which fastText is (char/word n-grams + linear classifier).

**Our current state** (`lt-http/src/lib.rs:437-473`) returns a stub: `confidence: 1.0`,
`source: null`, empty sentence ranges, and `language=auto` (`:386-405`) does no detection —
it takes the first `preferredVariants` entry with an engine, else `en-US`.

## 2. Measured benchmark

Model: `lid.176.ftz`, **938 013 bytes (916 kB)** downloaded from `dl.fbaipublicfiles.com`.
Inference via `fasttext-pure-rs` 0.1.0 (pure Rust, single dependency `thiserror`, no rayon —
which is what makes it WASM-viable; `messense/fasttext` pulls in rayon and does not).

- `FastText::load_from_reader` on embedded bytes: **7.7 ms**
- Prediction: **~45 µs** per 660-char call, single-threaded
- Rust 1.89+ required (MSRV floor must rise from 1.88 — owner approved)

### fastText vs whatlang, identical inputs

| Input | Truth | fastText | whatlang |
|---|---|---|---|
| `Jag kommer.` | sv | **sv 1.00** | Fra (0.017) |
| `Jeg kommer.` | da | **da 1.00** | Epo (0.022) |
| `Ich komme.` | de | **de 0.99** | Deu (0.025) |
| `Je viens.` | fr | **fr 0.91** | Afr (0.046) |
| `Vengo.` | es | it 0.61 / es 0.19 | Zul (0.010) |
| `Foi al cine cola so hermana.` | ast | gl 0.40 / pt 0.27 | **Ita (0.247)** |
| `Non traballo hoxe…` | gl | **gl 0.52** | Spa (0.030) |

whatlang is not merely less accurate — on short text it is actively wrong with near-zero
confidence (`Zul`, `Epo`, `Fra`). **Rejected for our use case.** Its 568 KB size advantage is
irrelevant next to that.

### Where fastText succeeds

`sv 0.99`, `da 0.98`, `de 1.00`, `km 1.00`, `ml 1.00`, `ta 1.00`, `fa 0.96`, `is 0.94`,
`br 0.92`, `th`-class non-Latin scripts all confident. Its label set nominally covers **36 of
our 38** (only `crh` and `nrd` absent).

### Where fastText fails, despite the label set

| Language | Reported as | Confidence | Note |
|---|---|---|---|
| `nrd` Nordum | `da` / `no` / `sv` | **0.70 – 0.99** | never `nrd`; high confidence, wrong |
| `no` Bokmål | `da` | 0.57 – 1.00 | conflated with Danish; `no` wins only sometimes |
| `nn` Nynorsk | `no` / `da` | 0.55 – 0.90 | `nn` almost never wins despite being in the label set |
| `ast` Asturian | `es` / `gl` | 0.40 – 0.68 | `ast` label exists but is not chosen |
| `crh` Crimean Tatar | `tr` | 0.92 | label absent; collides with Turkish |
| `gl` Galician | `gl` | 0.52 | correct but low confidence |
| `gn` Guaraní | `gn` | 0.57 | correct but low confidence |

This is the empirical justification for the lexicon layer: for `nrd` and `crh` the answer is
confidently wrong, so no threshold can recover it.

## 3. Architecture

```
text
 ├─► lexicon pass      nrd only                    ← no corpus exists; decides outright
 ├─► fastText pass     everything else             ← lid.176.ftz, bundled
 ├─► discriminator     when the answer is a        ← refines ast/no/nn/crh/gl
 │                     known confusable
 └─► gates             length / confidence / margin
```

The layers run in that order because each exists for a measured reason:

- **Lexicon** — Nordum only. It is the one language with no corpus at all (see §3.2), and a
  734-word exclusive lexicon beats any classifier we could train on 62 sentences. Corpus
  generation is tracked separately in `~/src/nordum/CORPUS_GENERATION_PLAN.md`, including the
  Rust transducer and the future Nordum web extension. That work lives in the **Nordum repo**,
  not here: LingoTweaker has no use for a Danish→Nordum engine, and vendoring it would add
  permanent dead weight to an extension that is already ~181 MB. It is a **separate workstream
  that does not gate this plan**, and it would not improve `nrd` detection even if finished
  first.
- **Primary fastText** — correct and confident for the other ~30 languages (§2).
- **Discriminator** — the cases where the primary model has the right label available and
  declines to use it: `ast`→`es`, `no`→`da`, `nn`→`no`, `crh`→`tr`, `gl`→`es` at 0.52.

### 3.1 Bundling the model

`include_bytes!("../assets/lid.176.ftz")` + `load_from_reader(std::io::Cursor::new(MODEL))`
— verified working in this repo's toolchain. **No network request**, per the owner's
requirement; the model is part of the extension, like the language packs. Verified load time
is 7.7 ms, so it can be a static, parsed once.

The `.ftz` is CC-BY-SA 3.0 (trained on Wikipedia/Tatoeba/SETimes). Per §7 the model licence is
a release gate, not an implementation blocker.

### 3.2 The Nordum lexicon

Derived, not hand-written, so it is reproducible and reviewable:

```
nrd_markers = words(nrd_core.dic) − words(nb ∪ nn ∪ da ∪ sv)
```

Verified against the real dictionaries: **734 exclusive words** of 1,963 (`aftenar`,
`arbeidarna`, `ansigtar`, `blere`, `blier`, `bliet`, `blomstar`, `barnarna`, …). The verb forms
match the `blir` in `data/nrd/rules/grammar.xml`. A single strong hit is sufficient — nothing
else shares the lexicon — so this layer needs no confidence comparison.

### 3.3 Discriminator for the confusable clusters

The four measured failures are not missing labels; they are **available labels the primary model
rejects**. That calls for a specialist second opinion, not a bigger model.

**Design: one discriminator model over the confusable label set**, consulted only when the
primary model's top-1 falls inside it. One model rather than four pairwise binaries, because
the failures are transitive (`es`↔`ast`↔`gl`↔`ca`; `da`↔`no`↔`nn`↔`sv`) and a shared
specialist sees the whole neighbourhood.

```
confusable = { es, ast, ca, gl, pt, fr, it, ro,   # Iberian / Romance
               da, sv, no, nn, is,                # Scandinavian
               tr, crh }                          # Turkic
```

Expected size on the order of 2 MB, parsed once, same `load_from_reader` path.

**Correction (measured, 2026-10-03).** The "2 MB" above was an estimate and it was wrong.
Parsing `lid.176.ftz`'s header directly gives the real geometry:

| Parameter | Value |
|---|---|
| `dim` | 16 (not the 100 default) |
| `bucket` | **2,000,000** |
| `nwords` | 7,235 |
| `nlabels` | 176 |
| input matrix rows | `nwords + bucket` = **2,007,235** |
| dense size | 123 MB → 0.89 MB quantized (**137:1**) |

**Label count is irrelevant to size.** 176 labels × 16 dims is 2,816 values — noise. The
hashed n-gram bucket table is 99.6 % of the model. So size is governed by `bucket` and `dim`,
which means:

- a discriminator at default `bucket` is also **~0.9 MB**, not ~2 MB;
- tuned down (`bucket` 500k, `dim` 16) it is **~0.3 MB**;
- fine-tuning `lid.176` to 177 labels would still be **~0.9 MB**.

So the two-model design costs ~1.2 MB total, against ~0.9 MB for a single fine-tuned model.
**That 0.3 MB is 0.17 % of the 181 MB extension, which does not justify custom training code.**
Base + tuned discriminator remains the recommendation.

Worth recording *why* fine-tuning is awkward, since it was a reasonable suggestion:
fastText has no continued-training mode that preserves the bucket table. The buckets are
addressed by hash, not by word name, so `fasttext supervised -pretrainedVectors` cannot restore
them — and they are 99.6 % of the model. Fine-tuning therefore means writing a training loop
that keeps the whole model and continues SGD on top of it. `fasttext-pure-rs` is inference-only,
so that loop does not exist today.

Its one genuine advantage is accuracy rather than size: training `ast`/`no`/`nn`/`crh` with
correct labels could fix the misclassifications natively. But a discriminator trained on the
same Wikipedia text achieves the same thing far more simply.

**Training data.** Our own `<example>` sentences are authentic and per-language but far too few
to train on: `ast` 129, `no` 111, `crh` 98, `nn` 69, `gl` 751. So training uses external
corpora — Wikipedia dumps per language (crh alone is 29,735 articles / 10 MB), filtered to the
target language.

**Validation** uses the `<example>` sentences **held out of training**, since they are
authentic, already in the repo, and in-domain (the same prose style a user writes). Report
per-language accuracy for every language in the confusable set, not an aggregate.

**Licensing of the derived model: settled.** Per owner (2026-10-03), the discriminator may be
released publicly under the same terms as `lid.176.ftz`; the objective is accuracy, not escaping
copyleft. Note that retraining *from* `lid.176.ftz` would make the result a derivative and it
would inherit CC-BY-SA — which is fine, but it is also unnecessary, since a from-scratch model
over the confusable set needs only Wikipedia-derived text.

**Fallback.** If the discriminator is unavailable or abstains, fall through to the primary
model's answer rather than to "unknown" — it is at least a plausible reading, and the gates in
§3.4 still apply.

### 3.4 Gates

Applied after whichever layer produced a candidate. Detection returns "unknown" — never a
guess — unless:

1. ≥ `MIN_CHARS` non-whitespace characters (recommend **40**);
2. top candidate ≥ `MIN_CONFIDENCE` (recommend **0.60**);
3. top ≥ `1.5 ×` runner-up.

Below the threshold the caller keeps the site/default language — today's behaviour.

### 3.5 Where it lives

`lt-core::detect`, so native, WASM and HTTP share one implementation. `lt-wasm` exposes it as
a free function (not an `LtEngine` method — choosing the language is what picks the engine):

```rust
/// `{"resolved":"sv","detected":{"language":"sv","confidence":0.93,"source":"model"},
///   "candidates":[{"language":"sv","confidence":0.93,"source":"model"}, …]}`
#[wasm_bindgen]
pub fn detect_json(text: &str, gates: Option<String>) -> Result<String, JsError>
```

`resolved` is `null` on an abstention, which is a normal outcome; `candidates` carries the whole
ranking so a UI can offer it without treating it as a decision. `source` is `lexicon` or `model`
here — the HTTP layer maps it to LT's `ngram` in §4.

The Nordum marker list and the identifier model are embedded with `include_str!`/`include_bytes!`,
like the wasm asset story elsewhere: the browser build fetches nothing to detect. The optional
`gates` argument takes `{"minChars":…,"minConfidence":…,"minMargin":…}` and defaults per field to
`Gates::default()`.

The wasm build comes from a **pinned engine commit** (`scripts/build-wasm.sh`
`ENGINE_COMMIT`), so the engine must be released before the extension can consume it.

## 4. HTTP API

**v2 — LT-compatible; fill in what LT really returns.**

- `language=auto` performs real detection, honouring `preferredVariants` as LT's tie-break.
- `detectedLanguage` gets a real `confidence`.
- `source`: LT emits `"ngram"`. fastText *is* an n-gram method, so `"ngram"` is defensible and
  keeps clients working. Recommend `"ngram"` on v2 and `"fasttext"` on v3 (honest, and the
  only place the difference is visible). Open question.
- `sentenceRanges` / `extendedSentenceRanges` populated, the latter with
  `detectedLanguages: [{language, rate}]` — a real fidelity gain over the current stubs.
- **No new v2 routes.** LT has no `/detect`; adding one breaks drop-in compatibility.

**v3 — free to extend:**

```
POST /v3/detect   text=<string>  [&restrict=sv,en,de]
→ { "detected":  { "language": "sv", "confidence": 0.93, "source": "fasttext" },
    "candidates": [ … ],
    "resolved":  "sv" }        // null when nothing clears the gate
```

`resolved: null` is the honest answer when nothing clears the gate. `&restrict` lets the
extension narrow to the 38 it ships packs for.

## 5. Extension

Use detection **only** to seed an unset language, via the existing host worker:

```
explicit per-chat / per-site language  →  always wins
lexicon or fastText above threshold    →  initial value only
below threshold                        →  site / default language
```

Detection never writes a setting, so an explicit choice is never overwritten. This is the
direct fix for the Swedish-default problem, where a stored value looked like a default because
nothing could explain its origin.

## 6. Options considered

| | Size | Short text | Correct for nrd/no/nn/ast/crh | Build |
|---|---|---|---|---|
| **A. fastText + lexicon + discriminator** | 916 kB + ~10 kB + ~2 MB | good with gates | **yes** | pure Rust, verified |
| B. fastText + lexicon only | 916 kB + ~10 kB | good with gates | nrd only; `ast`/`no`/`nn`/`crh` still wrong | pure Rust |
| C. whatlang + lexicon | 568 kB + lexicon | **unusable** (`Zul`, `Epo`) | partially | pure Rust |
| D. fastText alone | 916 kB | good | **no** — high-confidence wrong answers | pure Rust |
| E. lingua | ~150–180 MB | good | no (`km`, `ml` missing) | pure Rust |
| F. whichlang | 285 kB | fast | no — 16 languages | pure Rust |

**A recommended.** B is a legitimate cheaper first step: the lexicon alone fixes Nordum, which
is the worst failure (0.99 confidence for "Swedish"), leaving the four discriminator cases
known-wrong. D ships known-wrong answers for five of our own languages.

## 7. Model licensing — resolved

Owner decision (2026-10-03): **CC BY-SA 3.0 is accepted.** The model is vendored at
`crates/lt/assets/lid.176.ftz` with provenance in `crates/lt/assets/README.md`. Not a new
posture: `data/manifest.json` already records 14 CC-BY-SA-4.0 and ~54 GPL files, and the engine
is `LGPL-2.1-or-later`.

Two consequences from the earlier discussion are settled rather than open:

- A derived (fine-tuned) model may be published under the same terms.
- **Order of work: discriminator first, fine-tuning second** — build the confusable-cluster
  discriminator, then try fine-tuning and keep it only if it measures better or smaller.

Wikipedia CC-BY-SA 3.0 is also accepted as training data for the five languages still missing
from the corpus.

### What CC-BY-SA 3.0 actually obliges

The project already ships copyleft, so this is not a new posture. Of the 1,028 files in
`data/manifest.json`: 776 LGPL-2.1-or-later, ~54 GPL-2.0/GPL-3.0, **14 already CC-BY-SA-4.0**,
13 CC BY 4.0. The engine itself is `LGPL-2.1-or-later`.

CC-BY-SA's share-alike attaches to **adaptation**, not **aggregation**. Shipping the model
verbatim as a separate file that the engine reads at runtime is aggregation/linking — our own
code is not required to become CC-BY-SA. **Modifying it** (retraining, pruning to our 38
languages, requantising) *is* an adaptation and that artifact would have to be released under
CC-BY-SA, which is what rules out "start from lid.176 and trim it".

Creative Commons advise against using CC licences for software and the CC↔software interaction
is unsettled, so this goes to the legal reviewer, not to us.

### Alternatives surveyed — none better

| Candidate | Licence | Verdict |
|---|---|---|
| `lid.176.ftz` (upstream) | CC-BY-SA-3.0 | under review |
| `facebook/fasttext-language-identification` (296k dl) | **CC-BY-NC-4.0** | worse — non-commercial |
| `NeuML/language-id-quantized` | CC-BY-SA-3.0 | same terms |
| `cstr/fasttext-lid176-GGUF` | CC-BY-SA-3.0 | same model |
| `HPLT/OpenLID-v3` | GPL-3.0 | acceptable, but **1.23 GB** |
| `juliensimon/xlm-v-base-language-id` | MIT | permissively licensed, ~2.2 B parameters |

No permissively-licensed drop-in exists. **Training our own** is the only route to a permissive
model: Tatoeba is CC-BY 2.0 FR (attribution only). But our own corpus is too thin — 117,801
example sentences across 36 languages, **median 315, minimum 14** (gn 14, lt 14, is 45, ml 49,
sv 61, nrd 62) — so niche-language coverage would be materially worse, and no public corpus
exists for Nordum at all.

### Consequence for the implementation

Nothing, beyond one discipline: **`detect` must take its statistical model as a pluggable
source**, not hard-wire `lid.176.ftz`, so the release gate can swap in a self-trained model
without touching the lexicon, the gates, the HTTP layer or the extension. §3.4's trait is what
makes this possible — treat it as a hard requirement, not a nicety.

### Separate gap, independent of the decision

Attribution currently lives only in the untracked licence dossier, which will not ship. Add a
`THIRD-PARTY-NOTICES` file **inside the extension** listing the model, its licence and its
source. This is needed under Option A and harmless under Option B.

## 7.1 Risks

| Risk | Mitigation |
|---|---|
| `nrd` answered confidently as Danish/Norwegian/Swedish | Lexicon layer runs first and short-circuits |
| Nordum corpus workstream attempted to unblock detection | Decoupled — it is a test/tagger resource, not a detection dependency (§3.2) |
| `nn`/`no` conflated with `da` | Pair-scored nn/nb lexicon |
| `ast`→`es`, `crh`→`tr` | Lexicon layer |
| fasttext-pure-rs is young (v0.1.0, 16k downloads) | Pin version; keep `detect` behind a trait so lingua/whatlang can swap in |
| Parity fixtures break once `detectedLanguage` becomes real | `docs/parity/api-fixtures/` is **empty**; capture Java responses *before* changing the stub |
| MSRV 1.88 → 1.89 | Owner approved |
| Engine consumed from a pinned commit | Release the engine first, then bump `ENGINE_COMMIT` |
| Model licence undecided at release | §7 — gate the release, keep the model source pluggable |

## 8. Open questions

1. **`source` on v2**: `"ngram"` for LT compatibility, or `"fasttext"` for honesty?
2. **Thresholds** — 40 chars / 0.60 / 1.5× are proposals; calibrate on a corpus built from the
   `<example>` sentences already in `data/*/rules/grammar.xml` (authentic, per-language, and
   already in the repo). Five languages (`ca`, `es`, `pt`, `sr`, `uk`) have no such text — their
   grammars do not parse or ship no examples — and come from the vendored Wikipedia sentences in
   `data/detection/wikipedia/` instead (`tools/detection/fetch_wikipedia.py`), so they are
   scraped prose rather than curated rule examples and their rates are not directly comparable
   with the other 33.
3. **Should the lexicon also cover `gl` and `gn`?** fastText gets them right but at ~0.55.
4. **Lexicon size budget** — the full 734 Nordum words is ~10 kB raw; acceptable, but confirm.

## 9. Suggested order

1. Build the calibration/validation corpus from `data/*/rules/grammar.xml` examples (with the
   Wikipedia fallback for the five languages that ship none), held out from any training.
2. Derive and commit the Nordum marker set as a **generated** artefact (script in
   `LingoTweaker/tools/`), not hand-edited.
3. `lt_core::detect` with the lexicon + primary model + gates; unit tests per language.
4. Calibrate thresholds; fix the numbers.
5. Bundle `lid.176.ftz`; `lt-wasm` binding; rebuild; smoke test.
6. Collect corpora and train the discriminator; validate per-language on the held-out set.
7. `lt-http`: real `detectedLanguage`, sentence ranges, then `/v3/detect`.
8. Licence-review decision (§7), and ship a `THIRD-PARTY-NOTICES` inside the extension.
9. Extension: worker method, then gated seeding.