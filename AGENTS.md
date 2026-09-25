# AGENTS.md

Commands for working in this repository.

## Build / test / lint

```sh
cargo check --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

CI enforces all four. Tests read the vendored `data/` directory at the repo root;
without it, data-dependent tests skip themselves.

## Parity gate

```sh
scripts/oracle/gate.sh            # 2,000-example Java match-set gate (Docker)
```

Builds the sample from `docs/parity/corpora/en-examples.jsonl`, runs
`scripts/oracle/check-diff.sh` against the pinned Java checkout and fails
unless `only Java: 0; only Rust: 0; field diffs: 0`.

Gate scope: re-run another language's gate only when the change can influence
that language (shared engine/pipeline, `lt-pattern`, `lt-disambig`, tokenizer,
overlap filters or shared rule data); language-local changes only need that
language's gates/tests. The CI `parity` matrix enforces the same scope (P6.3,
D-134): the workflow computes the affected set with
`scripts/ci/affected-languages.sh` (language-local paths → that language,
shared/unrecognized paths → all, docs-only → none). Predict a push with
`scripts/ci/affected-languages.sh <base> <head>`; the filter's tests are
`scripts/ci/tests/affected-languages-test.sh`.

Divergence policy (D-310, owner 2026-09-23): fix a Rust/Java divergence only
when Java is more correct; where Rust is more correct, keep Rust and record the
divergence as intentional. Every `docs/differences.md` entry and every
`scripts/ci/parity.sh` allowance must state the correctness verdict and
distinguish **residue to fix** (Java-reference fidelity gap) from
**intentional: Rust more correct**.

Offline corpus gate (CI, no Docker):

```sh
cargo build --release -p lt-cli
scripts/ci/parity.sh en|de|es|fr|it|pt|nl|ca|gl|ro|pl|sk|sl|el|da|sv|is|eo|ast|br|tl|crh|be|ru|uk|sr|ar|fa|km|ml|ta|de-x-simple   # Java-golden languages
scripts/ci/parity.sh no|nrd|nn|gn|lt              # tests-only gate
```

Diffs the full per-language corpus against the pinned Java `CheckDump` goldens
in `docs/parity/golden/` (captured once with Docker); the CI `parity` matrix
runs the affected languages per push/PR (all gated languages when shared code
changed, no language job for docs-only changes). de/es/it/nl/ca/ro/sk/sl/el must
be 0 only-Java / 0 only-Rust / 0 field diffs; en allows exactly the one documented
`ADVERB_VERB_ADVERB_REPETITION` field diff, fr the documented divergences
#3/#4/#5 and pt #6; gl is at 0/0/0 (the former `HUNSPELL_RULE` = 2 suggestion
field diffs #7, resolved by the deterministic work-budget emulation of the
native hunspell suggestion timers: work is counted in affix-entry trials —
MAP-generated candidates weighted at a quarter, their measured per-check
trial cost being ~8x lower — with the per-pass `WORK_SUGGESTION` budget
checked at upstream's clock-check positions, so the two Galician words now
cut exactly where the pinned run's wall clocks did); pl is at 0/0/0 (the
former known fidelity gaps #9, resolved by the
`<unify>` engine fixes: negated/unified `<unify>` matching incl. per-token
reading sets for `max`-run elements, antipattern matching with the unifier and
marker spans, the `<match no="N">lemma</match>` disambiguation filter and the
Java possessive-quantifier regex semantics); da is at 0/0/0 (#10, resolved by the
suggestion-engine and dotted-abbreviation ports, plus the owner-added
`DANISH_TYPOS` Wikipedia typo-list rule (#14) pinned at an exact-count only-Rust
allowance of 0 corpus matches and 16 owner-added DanNet-derived
confusable-word rulegroups (#15) that the corpus does not trigger; the M4
refresh moved the tagger dict to the current Stavekontrolden 2.9.137 data and
added five owner-authored disambiguation rulegroups (#18) — the dict refresh
changes 2 corpus lines (same corrections, current-upstream readings the stale
Java dict lacks) pinned exactly as `Ordgentagelse`/`unde` 1+1 only-Java/
only-Rust, the new rulegroups change none); sv is at 0/0/0 (#11,
resolved by the suggestion-engine port, plus the owner-added
SALDO-derived confusable-word and coherency pairs (#16) and the owner-added
default-off `VECKODAG_DATUM` `<filter>` rules — the sv module's first filter
class `org.languagetool.rules.sv.DateCheckFilter` (#17) — that the corpus
does not trigger; the B1 lexicon swap rebuilt both Swedish dictionaries from
the Språkbanken SALDO morphological lexicon (CC BY 4.0, `data/sv/README.md`)
onto the unchanged SUC-style tagset and keeps the golden at 0/0/0 with no
allowance); is is at 0/0/0 (#12); eo is at 0/0/0
(#10, resolved by the wrong-split and `twowords` ports); ast is at 0/0/0 (#13); br is at 0/0/0 (#14); tl is at 0/0/0 (#15, resolved: the morfologik speller `getFrequency` now
reproduces Java's signed-byte frequency arithmetic, so the
frequency-weighted `MORFOLOGIK_RULE_TL` suggestions order identically); crh is at 0/0/0 (#16, resolved: Java's `UNICODE_CASE` folds `ı`/`İ` into the ASCII `i`/`I` class and `lt_pattern` now adds them); be is at 0/0/0 (#17); ru is at 0/0/0 (#18); uk is at 0/0/0 (#19, resolved); sr is at 0/0/0 (#20; the golden is captured from a forward-ported in-container copy of the reactor-excluded `sr` module, `scripts/oracle/sr/check-diff-sr.sh`, D-285/D-287/D-288). `no`, `nrd`, `nn` and `gn` are hand-authored
languages with no legacy Java module, so they run the same matrix with a
tests-only gate (integration test + `lt-cli inventory`, no Java oracle); `lt`
runs the same tests-only gate because its legacy module references an
unshipped `lt_LT.dict` and throws on every check, so the Rust engine vendors a
third-party ispell-lt dictionary under the unchanged `MORFOLOGIK_RULE_LT_LT`
id (docs/differences.md #11). `uk` is at 0 only-Java / 0 only-Rust / 0 field
diffs (#19, resolved: the prep-noun rule keeps its prep state across skip-type
exceptions like Java, so an intervening `part` token does not abort the check;
the Ukrainian `UppercaseSentenceStartRule` list exception requires `)` after
the lowercase letter, matching Java's override; and the adj-noun exception
helper's `forwardConjFind` branch (plural adjective + proper-name/conjunction
list, e.g. `молодші Олександр Ірванець, Оксана Луцишина`) is ported). `ar` is at 0 only-Java / 0 only-Rust / 0 field diffs (#21; resolved: the
`ArabicNumbersWords` number engine (with `ArabicNumberPhraseFilter`), the
`AR_INFLECTED_ONE_WORD` / `AR_VERB_TRANSITIVE_IINDIRECT` rule classes and the
speller wrong-split range logic are ported). `fa` is at 0 only-Java / 0 only-Rust / 0 field diffs (#22; resolved: the
former 258 only-Rust `Bad_ZWNJ` false positives came from
Java's token `\w` being ASCII while the Rust `regex` crate's is Unicode; the
shared `lt_pattern` translation now rewrites the shorthands to Java's ASCII
classes). `km` is at 0 only-Java / 0
only-Rust / 0 field diffs (#23; resolved: the
`IGNORE ៗ` directive is now implemented in `lt-spell`, so the swapchar/
extrachar candidates `ញប`/`មៃ` match the stored `ញបៗ`/`មៃៗ` entries like
Java's hunspell).
`ml` is at 0 only-Java / 0 only-Rust / 0 field diffs (#24; the 18 active XML
rules, the six generic built-ins and the Morfologik `MORFOLOGIK_RULE_ML_IN`
speller match the pinned module exactly; the speller is inert for Malayalam
script, `isLatinScript() = true`, as in Java).
`ta` is at 0 only-Java / 0 only-Rust / 0 field diffs (#25; the 210 active XML
rules, the `TamilTagger` over `ta/dictionaries/tamil.dict` and the five
`Tamil.getRelevantRules` generic built-ins (the ta-localized copy/punctuation/
whitespace/long-sentence/sentence-whitespace rules) match the pinned module
exactly; Tamil has no speller, disambiguator or synthesizer).
`de-x-simple` is at 0 only-Java / 0 only-Rust / 0 field diffs (#26; the Simple
German variant `de-DE-x-simple-language` is a `Lang::De` variant string that
reuses the German tagger/synthesizer/disambiguator/chunker but runs only its
92 active XML rules — never the German rule classes or speller; the 12-word
`TOO_LONG_SENTENCE_DE` is `tags="picky"`).

## Data tooling

```sh
python3 tools/lt-sync/lt_sync.py baseline --upstream <lt-checkout>   # pin upstream commit
python3 tools/lt-sync/lt_sync.py import  --upstream <lt-checkout> --artifacts <jars-dir>
python3 tools/lt-sync/lt_sync.py status  --upstream <lt-checkout>    # classify delta
python3 tools/lt-sync/lt_sync.py report
```

`lt-sync status` also writes `docs/parity/lt-sync-status.json` (gitignored artifact).

Dictionary tooling (pure-Python port of `languagetool-tools` + Morfologik; no JVM):

```sh
python3 tools/morfologik/lt_morfologik.py pos   -i word-lemma-tag.txt --info xx.info -o xx.dict
python3 tools/morfologik/lt_morfologik.py spell -i words.txt --info xx.info -o xx.dict
python3 tools/morfologik/lt_morfologik.py synth -i word-lemma-tag.txt --info xx_synth.info -o xx_synth.dict
python3 tools/morfologik/lt_morfologik.py dict_decompile -i xx.dict -o xx.txt
python3 -m unittest discover -s tools/morfologik/tests
```

Output is byte-for-byte identical to the Java tooling (`tools/morfologik/README.md`,
optional cross-check `tools/morfologik/tests/compare_with_java.py`).

Corpus extraction (regenerates `docs/parity/corpora/*.jsonl`):

```sh
cargo run -p lt-cli -- examples --lang en --out docs/parity/corpora/en-examples.jsonl
cargo run -p lt-cli -- inventory --lang en
```

## Wasm

```sh
rustup target add wasm32-unknown-unknown      # once
cargo run --release -p lt-data --bin pack_data -- data gn /tmp/lt-gn.pack
wasm-pack build crates/lt-wasm --target nodejs --out-dir ../../target/wasm-pkg/node
node tools/wasm/smoke.mjs /tmp/lt-gn.pack     # engine built from the pack
tools/wasm/build-demo.sh                      # minimal browser demo (www/pkg + packs)
scripts/data/build-packs.sh data /tmp/packs   # gzipped packs + sha256 manifest
scripts/release/publish-wasm.sh --build-only  # lingotweaker-wasm npm package (web+nodejs)
scripts/release/publish-npm-data.sh --build-only  # lingotweaker-data npm packages (loader + per-language)
demo/scripts/build-packs.sh                   # full demo: packs + rule inventories
demo/scripts/build.sh                         # full demo bundle into demo/dist
```

Engine data reads go through `lt_data::fs` (mount-aware): use
`lt_data::PathExt::lt_exists`/`lt_is_dir`/`lt_is_file` instead of the `Path`
predicates, and gate `SystemTime`/`Instant` uses so wasm builds cannot trap.
`DataDir::new`/`discover` also accept a `.pack`/`.pack.gz` file (mounted in
memory via `DataDir::from_pack_path`), so the npm data packs work for the
native engine too.

## Releasing

One prerelease version is shared by crates.io, PyPI and npm:

```sh
scripts/release/set-version.sh 0.1.0-alpha.2
git commit -am "release: 0.1.0-alpha.2"
git tag v0.1.0-alpha.2 && git push origin main v0.1.0-alpha.2
```

`.github/workflows/release.yml` runs on `v*` tags and calls
`scripts/release/*.sh` (kept separate from `ci.yml`; it does not touch the
`parity` or `test` jobs). The publishable crates.io set is the `lt-*` libraries
plus the `lingotweaker` facade (`crates/lingotweaker`, its own workspace, shares
the engine source through a symlink and keeps `[lib] name = "lt"`). `lt` cannot
be published (the crates.io name is taken); `lt-cli`/`lt-http`/`lt-py`/`lt-node`/
`lt-wasm` set `publish = false`.

Runtime data ships separately from the engine packages (they stay code-only):
npm `lingotweaker-data` (a code-only loader; `packPath(lang)` resolves the pack
from an installed per-language `lingotweaker-data-<lang>` package, mirroring
the PyPI model), one PyPI distribution `lingotweaker-data-<lang>` per language
(auto-discovered by `lt_py`), and per-language `packs/*.pack.gz` +
`data/*.tar.gz` + `manifest.json` assets on each GitHub Release. Package
readmes must say that engine packages ship code only and how to get the data
(`LT_DATA_DIR` accepts a directory or a pack file). The npm loader's smoke
test is `scripts/ci/tests/npm-data-loader-test.sh`.

## Conventions

- UTF-8 is the default string/offset format internally and externally.
  Engine-internal offsets are UTF-8 bytes. The HTTP API exposes two versions:
  `/v2/*` is the legacy v2 drop-in surface (UTF-16 code-unit offsets,
  exactly like LT) and `/v3/*` is the native surface (UTF-8 byte offsets).
  The oracle `lt-cli check --lines` converts to UTF-16 only to compare with
  Java's `CheckDump`. Do not move UTF-16 into the engine or default outputs.
- Rule, category, and message ids must never be renumbered.
- Public enums are `#[non_exhaustive]`; `Engine` must stay `Send + Sync`.
- Development notes (implementation plan, decision log, per-language
  checklists) are kept locally outside this repository and are not published.
