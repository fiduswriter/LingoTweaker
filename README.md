# LingoTweaker

LingoTweaker is a proofreader of numerous languages written in Rust, with bindings with 
Python and Node and compilable to WebAssembly.

**[Try it out here!](https://fiduswriter.github.io/LingoTweaker/)**

LingoTweaker includes a port of the [LanguageTool](https://languagetool.org) 
proofreading engine and its rule data. Additional languages and improvements to existing 
languages will be added over time.

*LingoTweaker* is an independent project, not affiliated with or endorsed by the
LanguageTool project, whose name and trademark belong to their owners. Upstream
references and licenses are kept.

- **Rust-first**: `cargo add lt` and embed the engine directly (`Engine::check`).
- **v2 HTTP drop-in**: same `/v2/*` paths, parameters, and JSON schema, so existing
  clients switch without changes.
- **WebAssembly**: `lt-wasm` (wasm-bindgen) runs the engine in the browser from
  fetchable per-language data packs.
- **Vendored data**: everything needed at runtime is copied into `data/` with per-file
  sha256 provenance, pinned to one upstream commit (`upstream.json`).
- **Parity-driven**: the legacy `<example>` tests are the primary gate; a pinned
  Java build of the legacy engine provides differential fixtures.

## Quick start

```sh
cargo test --workspace                 # engine + data manifest + HTTP contract tests
cargo run -p lt-cli -- inventory --lang en
cargo run -p lt-cli -- examples --lang en --out en-examples.jsonl
cargo run -p lt-cli -- serve --addr 0.0.0.0:8081
curl -s localhost:8081/v2/languages
curl -s -X POST localhost:8081/v2/check -d 'text=Hello+world.&language=en-US'
```

The engine reads data from `LT_DATA_DIR`, or `./data` when run inside the repository.

Build the minimal browser demo (needs `rustup target add wasm32-unknown-unknown`
and [wasm-pack](https://rustwasm.github.io/wasm-pack/)):

```sh
tools/wasm/build-demo.sh
python3 -m http.server -d crates/lt-wasm/www 8000
```

The full GitHub Pages demo (ProseMirror editor, all languages, rule settings)
lives in `demo/`:

```sh
demo/scripts/build.sh                  # wasm + data + Vite bundle
python3 -m http.server -d demo/dist 8000
```

## Workspace

| crate | purpose |
|-------|---------|
| `lt` | public facade: `Engine`, `EngineBuilder`, re-exports |
| `lt-core` | core types: languages, matches, check results, offsets |
| `lt-tokenize` | sentence splitter + tokenizer (UTF-8 byte offsets) |
| `lt-tagger` | Morfologik FSA tagger (spike in progress) |
| `lt-disambig` | disambiguation XML interpreter |
| `lt-pattern` | XML pattern-rule engine |
| `lt-spell` | speller + confusion sets |
| `lt-data` | vendored-data discovery, manifest verification |
| `lt-http` | LT v2-compatible HTTP API (axum) |
| `lt-cli` | batch/JSON CLI, corpus extraction, parity workhorse |
| `lt-wasm` | wasm-bindgen bindings: browser engine over in-memory data packs |

PyO3 (`lt-py`) and napi-rs (`lt-node`) wrappers arrive in Phase 2.

## Data and upstream sync

`upstream.json` pins the upstream baseline commit. `data/manifest.json` records
every vendored file's upstream path, sha256, size, and license. External artifacts
(tagger/speller dictionaries, OpenNLP chunker models) are pinned Maven artifacts
recorded in `tools/lt-sync/lt_sync.py`. The data layout, generation workflow and
manifest schema are documented in `data/README.md`.

```sh
python3 tools/lt-sync/lt_sync.py baseline --upstream /path/to/languagetool
python3 tools/lt-sync/lt_sync.py import  --upstream /path/to/languagetool --artifacts /tmp/kilo/maven
python3 tools/lt-sync/lt_sync.py status  --upstream /path/to/languagetool
python3 tools/lt-sync/lt_sync.py report
```

Rule, category, and message ids are never renumbered.

## License

Port and glue code: **LGPL-2.1-or-later** — see [`LICENSE`](LICENSE).
Vendored rule data, dictionaries and models keep their upstream licenses; the
component table (including licenses still to be confirmed) is
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), with per-file entries in
`data/manifest.json` and license texts in `licenses/README.md`.
