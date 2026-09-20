// Node smoke test for the wasm bindings: build a Guaraní engine from a data
// pack and check that a misspelled sentence produces matches.
//
//   cargo run --release -p lt-data --bin pack_data -- data gn /tmp/lt-gn.pack
//   wasm-pack build crates/lt-wasm --target nodejs --out-dir ../../target/wasm-pkg/node
//   node tools/wasm/smoke.mjs /tmp/lt-gn.pack
//
// Exits non-zero when the engine cannot be built or reports no matches.

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const pkgDir = process.env.LT_WASM_PKG ?? resolve(here, "../../target/wasm-pkg/node");
const packPath = process.argv[2] ?? "/tmp/lt-gn.pack";
const lang = process.argv[3] ?? "gn-ES";

const { LtEngine } = require(resolve(pkgDir, "lt_wasm.js"));
const pack = readFileSync(packPath);
const engine = new LtEngine(
  lang,
  undefined,
  new Date().toISOString(),
  new Uint8Array(pack.buffer, pack.byteOffset, pack.byteLength),
);

const failures = JSON.parse(engine.compile_failures_json());
if (failures.length > 0) {
  console.error(`FAIL: ${failures.length} rules failed to compile`, failures.slice(0, 3));
  process.exit(1);
}

const result = JSON.parse(engine.check_json("This are a test."));
if (!Array.isArray(result.matches) || result.matches.length === 0) {
  console.error("FAIL: expected at least one match for 'This are a test.'");
  process.exit(1);
}

console.log(
  `OK: ${engine.lang()} engine with ${engine.active_rule_count()} rules, ` +
    `${result.matches.length} matches, first rule ${result.matches[0].rule_id}`,
);
