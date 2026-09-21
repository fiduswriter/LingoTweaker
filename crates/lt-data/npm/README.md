# LingoTweaker data

Runtime language data for the LingoTweaker proofreading engine, as one gzipped
data pack per language (`packs/<lang>.pack.gz`). Both engine packages depend on
this package:

- `lingotweaker` (Node.js native addon) reads the pack file directly via
  `dataDir`;
- `lingotweaker-wasm` (browser/Node WebAssembly) reads or fetches the pack and
  passes the bytes to `LtEngine`.

```js
const data = require("lingotweaker-data");
const { Engine } = require("lingotweaker");

const engine = new Engine("en-US", { dataDir: data.packPath("en") });
```

```js
import init, { LtEngine } from "lingotweaker-wasm";
import { packPath } from "lingotweaker-data";
import { readFileSync } from "node:fs";

await init();
const pack = readFileSync(packPath("en"));
const engine = new LtEngine("en-US", pack, JSON.stringify({ today: new Date().toISOString() }));
```

- `packPath(lang)` — absolute path of a language pack
- `languages()` — the base codes shipped here
- `dataManifest()` — version, upstream commit and per-language `bytes`/`sha256`

The packs are the same artifacts attached to each GitHub Release
(`packs/<lang>.pack.gz`) and used by the browser demo, so all channels serve
identical data. This package ships data only, no code.

Vendored rule data, dictionaries and models keep their upstream licenses; see
`THIRD_PARTY_NOTICES.md` in the repository.

License (packaging code): LGPL-2.1-or-later.