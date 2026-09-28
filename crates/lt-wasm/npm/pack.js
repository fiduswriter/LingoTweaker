// Data helpers for the `lingotweaker-wasm` package.
//
// The engine itself is loaded from an in-memory data pack
// (`LtEngine(lang, pack, optionsJson)`); this module obtains those packs —
// either from the locally installed `lingotweaker-data-<lang>` npm packages
// (Node.js) or fetched from a `baseUrl` your server serves them from
// (browsers). Packs are gzip-compressed and inflated after loading.
//
//   import { fetchPack } from "lingotweaker-wasm/pack";
//   const pack = await fetchPack("en", { baseUrl: "/static/lingotweaker-packs" });
//   const engine = new LtEngine("en-US", pack, JSON.stringify({ today: new Date().toISOString() }));
//
// There is intentionally no built-in CDN default: pack files must be served
// from the same origin as the page (or any CORS-enabled server you pass as
// `baseUrl`), because browsers refuse cross-origin fetches otherwise.

/** The URL of the data manifest (`{ <lang>: { file, bytes, sha256 } }`) under `baseUrl`. */
export function manifestUrl(baseUrl) {
  return `${baseUrl}/manifest.json`;
}

/** The URL of the gzipped pack for `lang` under `baseUrl`. */
export function packUrl(lang, baseUrl) {
  return `${baseUrl}/packs/${lang}.pack.gz`;
}

/** Inflate a gzipped pack (browser `DecompressionStream`, else Node `zlib`). */
export async function decompressPack(bytes) {
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  if (typeof DecompressionStream === "function" && typeof Blob === "function") {
    const stream = new Blob([u8]).stream().pipeThrough(new DecompressionStream("gzip"));
    return new Uint8Array(await new Response(stream).arrayBuffer());
  }
  const { gunzipSync } = await import("node:zlib");
  return new Uint8Array(gunzipSync(u8));
}

/**
 * Load and inflate the pack for `lang` (`en`, `de`, `gn`, …).
 *
 * Without `options.baseUrl` this resolves the pack from the locally
 * installed `lingotweaker-data-<lang>` npm packages (Node.js only). In a
 * browser you must pass `baseUrl` pointing at a directory you serve with the
 * same layout as the `lingotweaker-data-<lang>` packages (a `packs/`
 * subdirectory containing `<lang>.pack.gz`); same-origin is the usual choice,
 * since browsers block cross-origin fetches.
 */
export async function fetchPack(lang, options = {}) {
  const { fetch: fetchImpl = fetch, baseUrl } = options;
  if (!baseUrl) {
    const local = await localPack(lang);
    if (local) return local;
    throw new Error(
      `lingotweaker-wasm: no locally installed lingotweaker-data-${lang} package; ` +
        `serve the packs/ directory of the lingotweaker-data-${lang} npm package from your ` +
        `server and pass its URL as the fetchPack baseUrl option`
    );
  }
  const url = packUrl(lang, baseUrl);
  const response = await fetchImpl(url);
  if (!response.ok) {
    throw new Error(`cannot load ${url}: HTTP ${response.status}`);
  }
  return decompressPack(await response.arrayBuffer());
}

/**
 * Read and inflate a pack from the locally installed data packages (the
 * code-only `lingotweaker-data` loader resolving `lingotweaker-data-<lang>`).
 * Returns `null` outside Node.js, or when the package/language is missing.
 */
export async function localPack(lang) {
  if (typeof process === "undefined" || !process.versions?.node) return null;
  try {
    const { createRequire } = await import("node:module");
    const require = createRequire(import.meta.url);
    const data = require("lingotweaker-data");
    const { readFile } = await import("node:fs/promises");
    const bytes = await readFile(data.packPath(lang));
    return decompressPack(bytes);
  } catch {
    return null;
  }
}
