# LingoTweaker (WebAssembly)

WebAssembly bindings for the LingoTweaker proofreading engine (wasm-bindgen),
for browsers and Node.js. The engine runs entirely from an in-memory data pack,
so no file system or native addon is required.

```sh
npm install lingotweaker-wasm@next
```

## Browser

```js
import init, { LtEngine } from "lingotweaker-wasm";
import { fetchPack } from "lingotweaker-wasm/pack";

await init();

const pack = await fetchPack("en"); // release asset for this package version
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

## Data

The package ships **code only** and depends on the code-only
`lingotweaker-data` loader; the packs ship in one small package per language
(`lingotweaker-data-<lang>`). In Node.js the data helper resolves installed
`lingotweaker-data-<lang>` packages locally; in the browser (or without them)
it falls back to the per-release GitHub Release assets:

```js
import { fetchPack, packUrl, manifestUrl } from "lingotweaker-wasm/pack";

await fetchPack("en");                  // installed lingotweaker-data-en in Node, else release assets
await fetchPack("gn", { baseUrl: "https://fiduswriter.github.io/LingoTweaker" });
packUrl("de");                           // build a URL yourself
```

`baseUrl` points at a directory that contains `packs/<lang>.pack.gz` and
`manifest.json`. All languages are attached to every release; `manifest.json`
lists file sizes and sha256 hashes. Each pack is also served from any npm CDN
(`https://cdn.jsdelivr.net/npm/lingotweaker-data-<lang>/packs/<lang>.pack.gz`),
and you can host the packs yourself.

LingoTweaker is an independent project and includes a port of the legacy
LanguageTool proofreading engine and its rule data. Upstream references and
licenses are kept; see `THIRD_PARTY_NOTICES.md` in the repository.

License: LGPL-2.1-or-later.