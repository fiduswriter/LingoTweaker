// Owns the wasm engine: fetches and decompresses a data pack, builds the
// engine (optionally with rule-selection settings), and checks paragraphs on
// request. Everything stays in this worker so typing never blocks on engine
// construction or checks.

import init, { LtEngine } from "../pkg/lt_wasm.js";
// Vite emits the binary as a hashed asset; the build ships a `.gz` sidecar
// next to it (scripts/compress-wasm.mjs) because GitHub Pages serves files
// without content compression.
import wasmUrl from "../pkg/lt_wasm_bg.wasm?url";

let initPromise = null;
let engine = null;
let packBytes = null;
let packParts = false;
let current = null;
let loadGeneration = 0;
let phase = "idle";
let activeUrl = null;
let packManifestPromise = null;
// per-paragraph check results (text -> matches): an edit burst re-checks
// every paragraph, but only the edited ones changed; capped LRU-style by
// insertion, cleared on engine rebuild (rule settings may change results)
const paragraphCache = new Map();
const PARAGRAPH_CACHE_MAX = 200;
let zstdSupported = null;

function post(message) {
  self.postMessage(message);
}

function ensureInit() {
  initPromise ??= fetchWasmBytes().then((bytes) => init(bytes));
  return initPromise;
}

/** Decompress the sniffed codec: gzip (0x1f8b) or zstd (0x28B52FFD). */
async function decompressIfCompressed(raw) {
  if (raw.length >= 4 && raw[0] === 0x28 && raw[1] === 0xb5 && raw[2] === 0x2f && raw[3] === 0xfd) {
    const stream = new Blob([raw]).stream().pipeThrough(new DecompressionStream("zstd"));
    return new Uint8Array(await new Response(stream).arrayBuffer());
  }
  if (raw.length >= 2 && raw[0] === 0x1f && raw[1] === 0x8b) {
    const stream = new Blob([raw]).stream().pipeThrough(new DecompressionStream("gzip"));
    return new Uint8Array(await new Response(stream).arrayBuffer());
  }
  return raw;
}

/** `DecompressionStream("zstd")` is Chrome/Edge-only (Safari: no, Firefox:
 * flag) — probe once so the pack fetch can prefer `.pack.zst`. */
async function ensureZstdSupport() {
  zstdSupported ??= await (async () => {
    if (typeof DecompressionStream !== "function") {
      return false;
    }
    try {
      // Safari throws InvalidStateError on construction even when the
      // constructor exists
      const probe = new DecompressionStream("zstd");
      await probe.writable.getWriter().close();
      return true;
    } catch {
      return false;
    }
  })();
  return zstdSupported;
}

/**
 * Fetch the wasm binary: try the pre-compressed sidecar first (a third of
 * the bytes on hosts without content compression), falling back to the
 * plain binary when the sidecar is absent (e.g. `vite dev`).
 */
async function fetchWasmBytes() {
  try {
    const response = await fetch(`${wasmUrl}.gz`);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    return await decompressIfCompressed(new Uint8Array(await response.arrayBuffer()));
  } catch {
    const response = await fetch(wasmUrl);
    if (!response.ok) {
      throw new Error(`cannot load wasm: HTTP ${response.status}`);
    }
    return new Uint8Array(await response.arrayBuffer());
  }
}

/** `packs/manifest.json` (content hashes) for cache-busted pack URLs. */
function ensurePackManifest() {
  packManifestPromise ??= (async () => {
    const url = new URL(
      `${import.meta.env.BASE_URL}packs/manifest.json`,
      self.location.origin,
    );
    try {
      const response = await fetch(url, { cache: "no-store" });
      return response.ok ? await response.json() : {};
    } catch {
      return {};
    }
  })();
  return packManifestPromise;
}

/** Fetch `url`, reporting progress; gunzips only when the bytes are gzip. */
async function fetchPack(url, onProgress) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`cannot load ${url}: HTTP ${response.status}`);
  }
  const total = Number(response.headers.get("content-length") ?? 0);
  const reader = response.body.getReader();
  const chunks = [];
  let received = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    chunks.push(value);
    received += value.length;
    onProgress?.(received, total);
  }
  const raw = new Uint8Array(await new Blob(chunks).arrayBuffer());
  // Servers may serve `.gz` files with `Content-Encoding: gzip` (the browser
  // then hands us the decoded pack) or as opaque gzip bytes; `.zst` files
  // are always opaque zstd frames.
  return decompressIfCompressed(raw);
}

/** Build (or rebuild) the engine from the cached pack bytes. */
async function build(options) {
  const generation = loadGeneration;
  paragraphCache.clear();
  post({ type: "status", phase: "build", lang: current.lang });
  await ensureInit();
  const started = performance.now();
  const engineOptions = JSON.stringify({
    variant: current.variant,
    today: new Date().toISOString(),
    ...(options ?? {}),
  });
  const built = Array.isArray(packBytes)
    ? LtEngine.new_multi(
        current.lang,
        packBytes.map((part) => new Uint8Array(part.bytes)),
        engineOptions,
      )
    : new LtEngine(current.lang, packBytes, engineOptions);
  if (generation !== loadGeneration) {
    return;
  }
  const failures = JSON.parse(built.compile_failures_json());
  engine = built;
  phase = "idle";
  post({
    type: "ready",
    lang: current.lang,
    rules: built.active_rule_count(),
    compileFailures: failures.length,
    packBytes: packBytes.length,
    buildMs: performance.now() - started,
  });
}

/**
 * Fetch a pack (gz or zst sidecar per manifest entry), reporting aggregate
 * progress across all concurrently-downloaded packs.
 */
function fetchPackEntry(entry, onProgress) {
  const useZst = Boolean(entry?.zst && zstdSupported === true);
  const chosen = useZst ? entry.zst : entry;
  const version = chosen?.sha256 ? `?v=${chosen.sha256.slice(0, 12)}` : "";
  const url = new URL(
    `${import.meta.env.BASE_URL}packs/${chosen?.file ?? entry.file}${version}`,
    self.location.origin,
  ).href;
  const local = { received: 0 };
  return fetchPack(url, (received, total) => {
    onProgress(local, received, total);
  });
}

async function load({ lang, variant, pack, options }) {
  const generation = ++loadGeneration;
  engine = null;
  if (!packBytes || !current || current.pack !== pack) {
    const manifest = await ensurePackManifest();
    await ensureZstdSupport();
    const entry = manifest[pack];
    phase = "download";
    post({ type: "status", phase, lang });
    if (entry?.split) {
      // split pack: base + the sidecars this engine instance needs (the
      // variant dictionary when the variant is not the default; the OpenNLP
      // chunker models when the user opted into full grammar checking)
      const parts = [{ entry: entry.split.base, key: "base" }];
      const extra = entry.split.extra ?? {};
      const variantKey = variant && variant !== "en-US" ? variant : null;
      if (variantKey && extra[variantKey]) {
        parts.push({ entry: extra[variantKey], key: variantKey });
      }
      if (options?.models && extra.models) {
        parts.push({ entry: extra.models, key: "models" });
      }
      const tracks = parts.map((part) => ({ part, received: 0, total: 0 }));
      const postAggregate = () => {
        let received = 0;
        let total = 0;
        for (const track of tracks) {
          received += track.received;
          total += track.total;
        }
        post({ type: "progress", received, total });
      };
      const fetched = await Promise.all(
        tracks.map(async (track) => {
          const bytes = await fetchPackEntry(track.part.entry, (received, total) => {
            track.received = received;
            track.total = total;
            postAggregate();
          });
          return { bytes };
        }),
      );
      if (generation !== loadGeneration) {
        return;
      }
      packBytes = fetched;
      packParts = true;
    } else {
      const bytes = await fetchPackEntry(entry ?? {}, (received, total) => {
        post({ type: "progress", received, total });
      });
      if (generation !== loadGeneration) {
        return;
      }
      packBytes = bytes;
      packParts = false;
    }
  }
  if (generation !== loadGeneration) {
    return;
  }
  current = { lang, variant, pack };
  activeUrl = null;
  await build(options);
}

function check({ id, paragraphs }) {
  if (!engine) {
    post({ type: "error", message: "engine is not loaded" });
    return;
  }
  const started = performance.now();
  const results = paragraphs.map((text, index) => {
    if (!text || text.trim().length === 0) {
      return { index, matches: [] };
    }
    let matches = paragraphCache.get(text);
    if (!matches) {
      matches = JSON.parse(engine.check_matches_json(text)).matches;
      if (paragraphCache.size >= PARAGRAPH_CACHE_MAX) {
        paragraphCache.delete(paragraphCache.keys().next().value);
      }
      paragraphCache.set(text, matches);
    }
    return { index, matches };
  });
  post({ type: "result", id, results, ms: performance.now() - started });
}

self.onmessage = async (event) => {
  const message = event.data;
  try {
    if (message.type === "load") {
      await load(message);
    } else if (message.type === "rebuild") {
      if (!packBytes || !current) {
        throw new Error("no pack loaded yet");
      }
      loadGeneration += 1;
      engine = null;
      await build(message.options);
    } else if (message.type === "check") {
      check(message);
    }
  } catch (error) {
    const failingPhase = phase;
    phase = "error";
    const detail = activeUrl ? ` [${failingPhase} · ${activeUrl}]` : ` [${failingPhase}]`;
    post({ type: "error", message: `${String(error?.message ?? error)}${detail}` });
  }
};
