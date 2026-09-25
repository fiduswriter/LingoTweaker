# LingoTweaker data

Runtime data for the LingoTweaker proofreading engine. This package is the
**code-only loader**: the actual language packs ship in one small package per
language (`lingotweaker-data-<lang>`, mirroring the per-language PyPI data
distributions), so npm never has to carry a single multi-language tarball.
Install the loader plus the languages you need:

```sh
npm install lingotweaker-data lingotweaker-data-en
```

```js
const data = require("lingotweaker-data");
const { Engine } = require("lingotweaker");

const engine = new Engine("en-US", { dataDir: data.packPath("en") });
for (const match of engine.check("I can heard you.").matches) {
  console.log(match.rule_id, match.suggestions);
}
```

```js
import init, { LtEngine } from "lingotweaker-wasm";
import { packPath } from "lingotweaker-data";
import { readFileSync } from "node:fs";

await init();
const pack = readFileSync(packPath("en"));
const engine = new LtEngine("en-US", pack, JSON.stringify({ today: new Date().toISOString() }));
```

- `packPath(lang)` — absolute path of a language pack, resolved from the
  installed `lingotweaker-data-<lang>` package (which must be installed
  separately; resolution starts next to this package and falls back to the
  process working directory for pnpm/workspace layouts)
- `languages()` — the base codes with a published pack
- `dataManifest()` — version, upstream commit and per-language `bytes`/`sha256`;
  the `file` paths are relative to the per-language package

The packs are the same artifacts attached to each GitHub Release
(`packs/<lang>.pack.gz`) and used by the browser demo, so all channels serve
identical data.

Note for layouts where package resolution cannot reach the per-language
packages from inside this one (strict pnpm), declare
`lingotweaker-data-<lang>` as a dependency of the consuming package so the
package manager links it.

Vendored rule data, dictionaries and models keep their upstream licenses; see
`THIRD_PARTY_NOTICES.md` in the repository.

License (packaging code): LGPL-2.1-or-later.