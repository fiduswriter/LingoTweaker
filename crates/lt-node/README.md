# LingoTweaker (Node.js)

Node.js bindings for the LingoTweaker proofreading engine (Rust core, napi-rs).

This package ships a native addon (code only) and depends on the code-only
`lingotweaker-data` loader. The packs ship in one small package per language
(`lingotweaker-data-<lang>`, mirroring the per-language PyPI data
distributions) — install the languages you need, then point the engine at a
pack:

```sh
npm install lingotweaker lingotweaker-data-en
```

```js
const { Engine } = require("lingotweaker");
const { packPath } = require("lingotweaker-data");

const engine = new Engine("en-US", { dataDir: packPath("en") });
for (const match of engine.check("I can heard you.").matches) {
  console.log(match.rule_id, match.suggestions);
}
```

`LT_DATA_DIR` accepts a data directory or a single `.pack`/`.pack.gz` file, so
`packPath("en")` can also be exported before constructing the engine.
Per-language packs and extractable native archives are attached to each
[GitHub Release](https://github.com/fiduswriter/LingoTweaker/releases) too. See
the repository README for how the data tree is produced.

LingoTweaker is an independent project and includes a port of the legacy
LanguageTool proofreading engine and its rule data. Upstream references and
licenses are kept; see `THIRD_PARTY_NOTICES.md` in the repository.

License: LGPL-2.1-or-later.