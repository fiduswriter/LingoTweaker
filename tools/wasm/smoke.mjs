// Node smoke test for the wasm bindings: build a Guaraní engine from a data
// pack and check that a misspelled sentence produces matches, then detect the
// language of two sentences with the free detection function.
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
const text = "This are a test.";

const { LtEngine, detect_json } = require(resolve(pkgDir, "lt_wasm.js"));
const pack = readFileSync(packPath);
const bytes = new Uint8Array(pack.buffer, pack.byteOffset, pack.byteLength);

const engine = new LtEngine(
  lang,
  bytes,
  JSON.stringify({ today: new Date().toISOString() }),
);

const failures = JSON.parse(engine.compile_failures_json());
if (failures.length > 0) {
  console.error(`FAIL: ${failures.length} rules failed to compile`, failures.slice(0, 3));
  process.exit(1);
}

const result = JSON.parse(engine.check_json(text));
if (!Array.isArray(result.matches) || result.matches.length === 0) {
  console.error(`FAIL: expected at least one match for '${text}'`);
  process.exit(1);
}
const firstRule = result.matches[0].rule_id;

// Engine options: the same check with the matching rule disabled must not
// report it anymore.
const filteredEngine = new LtEngine(
  lang,
  bytes,
  JSON.stringify({ today: new Date().toISOString(), disabledRules: [firstRule] }),
);
const filtered = JSON.parse(filteredEngine.check_json(text));
if (filtered.matches.some((match) => match.rule_id === firstRule)) {
  console.error(`FAIL: disabled rule ${firstRule} still matched`);
  process.exit(1);
}

console.log(
  `OK: ${engine.lang()} engine with ${engine.active_rule_count()} rules, ` +
    `${result.matches.length} matches, first rule ${firstRule} ` +
    `(disabled again: ${result.matches.length - filtered.matches.length} fewer matches)`,
);

// Detection is a free function rather than an engine method — it is what picks
// the language an engine is then built for — and it needs no data pack: the
// model and the Nordum marker list are embedded in the wasm binary.
const detected = JSON.parse(
  detect_json("Jag arbetar inte i dag, men jag kommer hem efter jobbet."),
);
if (detected.resolved !== "sv" || detected.detected.source !== "model") {
  console.error(`FAIL: expected sv from the model, got ${JSON.stringify(detected)}`);
  process.exit(1);
}

// Nordum is decided by the embedded marker list, not by the model: no statistical
// model knows the language.
const nordum = JSON.parse(
  detect_json("Jei vet at hun arbeider i dag, og det er viktig å lære språket."),
);
if (nordum.resolved !== "nrd" || nordum.detected.source !== "lexicon") {
  console.error(`FAIL: expected nrd from the lexicon, got ${JSON.stringify(nordum)}`);
  process.exit(1);
}

console.log(
  `OK: detect_json resolved ${detected.resolved} (model) and ${nordum.resolved} (lexicon)`,
);
