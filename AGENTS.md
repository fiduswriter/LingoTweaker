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

Offline corpus gate (CI, no Docker):

```sh
cargo build --release -p lt-cli
scripts/ci/parity.sh en|de|es|fr|it|pt|nl|ca|gl|ro|pl|sk   # Java-golden languages
scripts/ci/parity.sh no|nrd|gn                 # tests-only gate
```

Diffs the full per-language corpus against the pinned Java `CheckDump` goldens
in `docs/parity/golden/` (captured once with Docker); the CI `parity` matrix
runs the affected languages per push/PR (all gated languages when shared code
changed, no language job for docs-only changes). de/es/it/nl/ca/ro/sk must be 0
only-Java / 0 only-Rust / 0 field diffs; en allows exactly the one documented
`ADVERB_VERB_ADVERB_REPETITION` field diff, fr the documented divergences
#3/#4/#5, pt #6 and gl the documented `HUNSPELL_RULE` = 83 suggestion field
diffs (#7: same match set, suggestions from the bounded search instead of the
unported native `hunspell.suggest`); pl the documented known fidelity gaps
#9 (4 only-Java / 5 only-Rust / 0 field diffs, the `<unify negate="yes">`
agreement rules, the ZDANIA_ZLOZONE comp:comma disambiguation context and
the PCON_VERB participle rule). `no`, `nrd` and `gn` are hand-authored
languages with no legacy Java module, so they run the same matrix with a
tests-only gate (integration test + `lt-cli inventory`, no Java oracle).

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
demo/scripts/build-packs.sh                   # full demo: packs + rule inventories
demo/scripts/build.sh                         # full demo bundle into demo/dist
```

Engine data reads go through `lt_data::fs` (mount-aware): use
`lt_data::PathExt::lt_exists`/`lt_is_dir`/`lt_is_file` instead of the `Path`
predicates, and gate `SystemTime`/`Instant` uses so wasm builds cannot trap.

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
`lt-wasm` set `publish = false`. Package readmes must keep stating that engine
packages ship without data and read `LT_DATA_DIR` at runtime.

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
