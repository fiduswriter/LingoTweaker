# LingoTweaker (WebAssembly)

WebAssembly bindings for the LingoTweaker proofreading engine (wasm-bindgen),
for browsers and Node.js. The engine runs entirely from an in-memory data pack,
so no file system or native addon is required.

```sh
npm install lingotweaker-wasm@next
```

## Browser

The engine's language data packs must be served from your own server —
browsers block cross-origin fetches, so the data helper has no built-in CDN
default. Install the `lingotweaker-data-<lang>` npm packages for the languages
you want, copy their `packs/` directory into your static assets, and pass its
URL as `baseUrl`:

```js
import init, { LtEngine } from "lingotweaker-wasm";
import { fetchPack } from "lingotweaker-wasm/pack";

await init();

const pack = await fetchPack("en", { baseUrl: "/static/lingotweaker-packs" });
const engine = new LtEngine(
  "en-US",
  pack,
  JSON.stringify({ today: new Date().toISOString() }),
);
const result = JSON.parse(engine.check_json("I can heard you."));
for (const match of result.matches) {
  console.log(match.rule_id, match.suggestions);
}
```

## Node.js

The Node build is CommonJS (`lingotweaker-wasm/node`); the data helper is ESM,
so import it dynamically from CJS:

```js
const { LtEngine } = require("lingotweaker-wasm/node");

(async () => {
  const { fetchPack } = await import("lingotweaker-wasm/pack");
  const pack = await fetchPack("en");
  const engine = new LtEngine(
    "en-US",
    pack,
    JSON.stringify({ today: new Date().toISOString() }),
  );
  console.log(engine.check_json("I can heard you."));
})();
```

Or, from an ES module: `import { LtEngine } from "lingotweaker-wasm/node"`.

`today` is required in practice: WebAssembly has no clock, so the date is
passed in and pinned for the engine's date filters.

## Language detection

`detect_json` is a free function, not an `LtEngine` method: choosing the
language is what picks the engine, so you ask before any engine exists. It
needs no data pack — the identifier model and the Nordum marker list are
embedded in the wasm binary.

```js
import init, { detect_json, LtEngine } from "lingotweaker-wasm";

await init();

const { resolved, detected, candidates } = JSON.parse(
  detect_json("Jag arbetar inte i dag, men jag kommer hem efter jobbet."),
);
// resolved: "sv"  detected: {language: "sv", confidence: 0.99, source: "model"}
// candidates: the five best-scoring languages, best first

const engine = new LtEngine(
  resolved ?? "en-US",
  pack,
  JSON.stringify({ today: new Date().toISOString() }),
);
```

`resolved` is `null` when there is not enough evidence to answer — that is a
normal outcome, not an error, and the caller keeps the language it already had.
`detected.source` says which layer decided: `"lexicon"` for a word no other
language we ship contains (this is how Nordum is detected), `"model"` for the
statistical model.

Detection is deliberately conservative on short fragments, which is what an
editor sees. The optional second argument overrides the thresholds:

```js
detect_json(text, JSON.stringify({ minChars: 40, minConfidence: 0.3, minMargin: 2 }));
```

Each field defaults to the engine default, so `{}` is the same call as no
argument.

## Data

The package ships **code only** and depends on the code-only
`lingotweaker-data` loader; the packs ship in one package per language
(`lingotweaker-data-<lang>`). In Node.js the data helper resolves installed
`lingotweaker-data-<lang>` packages automatically. In the browser you serve
the packs yourself (see above) and pass the directory URL as `baseUrl`:

```js
import { fetchPack, packUrl, manifestUrl } from "lingotweaker-wasm/pack";

await fetchPack("en"); // installed lingotweaker-data-en (Node.js)
await fetchPack("gn", { baseUrl: "/static/lingotweaker-packs" }); // browser: your own server
packUrl("de", "/static/lingotweaker-packs"); // build a URL yourself
```

`baseUrl` points at a directory with the same layout as the
`lingotweaker-data-<lang>` packages: a `packs/` subdirectory containing
`<lang>.pack.gz` plus a `manifest.json` listing file sizes and sha256 hashes.
Same-origin is the usual choice; any CORS-enabled static server works. Packs
are downloaded individually per language, on demand.

LingoTweaker is an independent project and includes a port of the legacy
LanguageTool proofreading engine and its rule data. Upstream references and
licenses are kept; see `THIRD_PARTY_NOTICES.md` in the repository.

LingoTweaker is an independent project and includes a port of the legacy
LanguageTool proofreading engine and its rule data. Upstream references and
licenses are kept; see `THIRD_PARTY_NOTICES.md` in the repository.

License: LGPL-2.1-or-later.