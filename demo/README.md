# LingoTweaker browser demo

A ProseMirror editor that checks spelling and grammar with the LingoTweaker
engine compiled to WebAssembly. The engine and all data run in the browser;
the text never leaves the page.

Features:

- 15 language entries (en-US/en-GB, de-DE/de-AT/de-CH, es, fr, it, pt-PT/pt-BR,
  nl, ca, no, nrd, gn) built from lazily fetched, gzipped data packs;
- issues are underlined, coloured by `issue_type` (misspelling, grammar,
  style, typographical, duplication, …);
- right-click an underlined word for a popup with replacement suggestions
  (or an OK button when a rule has none);
- a **Rules…** panel: include/exclude picky rules and toggle individual
  rules (default-off rules can be switched on, default-on rules off); the
  engine is rebuilt from the cached pack, so nothing is re-downloaded;
- checks run in a Web Worker, debounced while typing, plus a manual button;
- the browser's native spellchecker is disabled so only the LingoTweaker
  underlines show.

## Local development

Build the wasm bindings and the demo data once (packs + rule inventories):

```sh
demo/scripts/build-packs.sh
wasm-pack build crates/lt-wasm --target web --out-dir ../../demo/pkg
```

Then run the dev server from this directory:

```sh
npm install
npm run dev
```

The default Vite base is `/LingoTweaker/` (the GitHub Pages project path).
For a different root, set `BASE_URL`:

```sh
BASE_URL=/ npm run dev
```

## Production build

```sh
demo/scripts/build.sh          # wasm + data + `vite build` into demo/dist
```

GitHub Pages deployment runs the same steps in
`.github/workflows/pages.yml` on every push to `main` (Pages must be enabled
for the repository with "GitHub Actions" as the source).

## Data

`demo/scripts/build-packs.sh` produces two generated trees (both gitignored):

- `public/packs/<lang>.pack.gz` — `lt-data`'s `pack_data` output, gzipped; the
  worker fetches `<base>/packs/<lang>.pack.gz`, inflates it only if the server
  did not already (`DecompressionStream("gzip")`), and passes the bytes to
  `LtEngine`.
- `public/packs/manifest.json` — content hash per pack; the worker appends
  `?v=<hash>` to the pack URL so a redeployed pack is never served from a
  stale cache.
- `public/rules/<lang>.json` — `lt-cli inventory --lang <lang> --json`; the
  Rules panel groups these by rule id (rulegroups toggle as one entry) and
  shows name, category, "default off", "picky" and "unsupported" badges.

Pack sizes range from 0.35 MB (Guaraní) to 37 MB (Dutch, whose synthesis and
spelling FSA dictionaries dominate); the picker shows the size of each
language before it is downloaded.
