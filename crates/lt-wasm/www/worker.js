// Runs the wasm engine in a worker so engine construction and checks never
// block the page. Packs are fetched from ./packs/<pack>.pack.

import init, { LtEngine } from "./pkg/lt_wasm.js";

let initPromise = null;
let engine = null;
let ready = Promise.resolve();

function ensureInit() {
  if (!initPromise) initPromise = init();
  return initPromise;
}

async function load({ lang, pack, variant }) {
  await ensureInit();
  const url = new URL(`packs/${pack}.pack`, self.location.href);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`cannot load ${url.pathname}: HTTP ${response.status}`);
  }
  const bytes = new Uint8Array(await response.arrayBuffer());
  engine = new LtEngine(
    lang,
    bytes,
    JSON.stringify({ variant, today: new Date().toISOString() }),
  );
  const failures = JSON.parse(engine.compile_failures_json());
  if (failures.length > 0) {
    throw new Error(`${failures.length} rules failed to compile (first: ${failures[0].rule})`);
  }
  return {
    type: "ready",
    lang: engine.lang(),
    rules: engine.active_rule_count(),
    packBytes: bytes.length,
  };
}

self.onmessage = async (event) => {
  const message = event.data;
  try {
    if (message.type === "load") {
      self.postMessage({ type: "loading", pack: message.pack });
      ready = load(message).then(
        (status) => self.postMessage(status),
        (error) => {
          engine = null;
          self.postMessage({ type: "error", message: String(error?.message ?? error) });
        },
      );
      await ready;
    } else if (message.type === "check") {
      await ready;
      if (!engine) throw new Error("engine is not loaded");
      const started = performance.now();
      const result = JSON.parse(engine.check_json(message.text));
      self.postMessage({
        type: "result",
        id: message.id,
        result,
        ms: performance.now() - started,
      });
    }
  } catch (error) {
    self.postMessage({ type: "error", message: String(error?.message ?? error) });
  }
};
