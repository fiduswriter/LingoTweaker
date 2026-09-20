# LingoTweaker (Node.js)

Node.js bindings for the LingoTweaker proofreading engine (Rust core, napi-rs).

```js
const { Engine } = require("lingotweaker");

const engine = new Engine("en-US");
for (const match of engine.check("I can heard you.").matches) {
  console.log(match.rule_id, match.suggestions);
}
```

This package ships a native addon (code only). The engine loads its language
data from a data directory at runtime:

- set the `LT_DATA_DIR` environment variable to the directory that holds the
  vendored `data/` tree, or
- point the engine at it explicitly if the binding exposes a data-dir option.

Without data the engine cannot build a pipeline; see the repository README for
how the data tree is produced.

LingoTweaker is an independent project and includes a port of the legacy
LanguageTool proofreading engine and its rule data. Upstream references and
licenses are kept; see `THIRD_PARTY_NOTICES.md` in the repository.

License: LGPL-2.1-or-later.