// lt-node smoke and worker-thread tests.
//
// Run against a built addon (see scripts/bindings/smoke.sh):
//   cargo build --release -p lt-node
//   cp target/release/liblt_node.so /tmp/lt-node/lt_node.node
//   node crates/lt-node/tests/node/test_smoke.mjs
//
// Run from the repository root (the engine finds ./data) or set LT_DATA_DIR.

import { createRequire } from "node:module";
import { Worker, isMainThread, parentPort } from "node:worker_threads";
import { fileURLToPath } from "node:url";
import process from "node:process";

const require = createRequire(import.meta.url);
const ADDON = process.env.LT_NODE_ADDON || "/tmp/lt-node/lt_node.node";
const { Engine } = require(ADDON);

if (!isMainThread) {
  // worker mode: every worker uses its own engine (locked decision 10)
  const engine = new Engine("en-US");
  const result = engine.check("I can heard you.");
  parentPort.postMessage({
    ok: result.matches.some((m) => m.rule_id === "MD_BASEFORM"),
    lang: engine.lang,
  });
} else {
  let failures = 0;
  const check = (name, fn) => {
    try {
      fn();
      console.log(`ok ${name}`);
    } catch (e) {
      failures += 1;
      console.error(`FAIL ${name}: ${e && e.stack ? e.stack : e}`);
    }
  };

  check("basic check", () => {
    const engine = new Engine("en-US");
    if (engine.lang !== "en-US") throw new Error(`lang ${engine.lang}`);
    if (!(engine.ruleCount > 5000)) {
      throw new Error(`ruleCount ${engine.ruleCount}`);
    }
    const result = engine.check("I can heard you.");
    const m = result.matches.find((m) => m.rule_id === "MD_BASEFORM");
    if (!m) throw new Error(`matches ${JSON.stringify(result.matches)}`);
    if (m.range.start !== 6 || m.range.end !== 11) {
      throw new Error(`range ${JSON.stringify(m.range)}`);
    }
    if (!Array.isArray(m.suggestions)) {
      throw new Error("suggestions not an array");
    }
  });

  check("utf8 offsets", () => {
    const engine = new Engine("en-US");
    const result = engine.check("😀 definately misspelled");
    const spell = result.matches.find(
      (m) => m.rule_id === "MORFOLOGIK_RULE_EN_US",
    );
    if (!spell) throw new Error(JSON.stringify(result.matches));
    if (spell.range.start !== 5) {
      throw new Error(`offset ${spell.range.start}`);
    }
  });

  check("picky and disabled rules", () => {
    const defaultEngine = new Engine("en-US");
    const text = "The problem is big. Another problem appears.";
    if (
      defaultEngine
        .check(text)
        .matches.some((m) => m.rule_id === "EN_REPEATEDWORDS")
    ) {
      throw new Error("EN_REPEATEDWORDS fired without picky");
    }
    const picky = new Engine("en-US", { picky: true });
    if (
      !picky.check(text).matches.some((m) => m.rule_id === "EN_REPEATEDWORDS")
    ) {
      throw new Error("EN_REPEATEDWORDS missing with picky");
    }
    const noSpelling = new Engine("en-US", {
      disabledRules: ["MORFOLOGIK_RULE_EN_US"],
    });
    if (
      noSpelling
        .check("This is definately wrong.")
        .matches.some((m) => m.rule_id === "MORFOLOGIK_RULE_EN_US")
    ) {
      throw new Error("spelling rule not disabled");
    }
  });

  check("checkJson", () => {
    const engine = new Engine("en-US");
    const result = JSON.parse(engine.checkJson("I can heard you."));
    if (result.text !== "I can heard you.") throw new Error(result.text);
    if (!result.matches.some((m) => m.rule_id === "MD_BASEFORM")) {
      throw new Error(JSON.stringify(result.matches));
    }
  });

  const workers = [0, 1, 2, 3].map(
    () =>
      new Promise((resolve, reject) => {
        const w = new Worker(fileURLToPath(import.meta.url));
        w.once("message", resolve);
        w.once("error", reject);
      }),
  );
  const results = await Promise.all(workers);
  check("worker threads", () => {
    if (!results.every((r) => r.ok && r.lang === "en-US")) {
      throw new Error(JSON.stringify(results));
    }
  });

  if (failures > 0) {
    console.error(`${failures} lt-node test(s) failed`);
    process.exit(1);
  }
  console.log("lt-node tests passed");
}
