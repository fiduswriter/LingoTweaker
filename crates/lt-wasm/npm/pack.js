// Data helpers for the `lingotweaker-wasm` package.
//
// The engine itself is loaded from an in-memory data pack
// (`LtEngine(lang, pack, optionsJson)`); this module fetches those packs from
// the per-release GitHub Release assets and inflates the gzip wrapper.
//
//   import { fetchPack } from "lingotweaker-wasm/pack";
//   const pack = await fetchPack("en");
//   const engine = new LtEngine("en-US", pack, JSON.stringify({ today: new Date().toISOString() }));

// Updated by scripts/release/set-version.sh together with the package version.
export const DEFAULT_DATA_BASE_URL =
  "https://github.com/fiduswriter/LingoTweaker/releases/download/v0.1.0-alpha.1";

/** The URL of the release data manifest (`{ <lang>: { file, bytes, sha256 } }`). */
export function manifestUrl(baseUrl = DEFAULT_DATA_BASE_URL) {
  return `${baseUrl}/manifest.json`;
}

/** The URL of the gzipped pack for `lang`. */
export function packUrl(lang, baseUrl = DEFAULT_DATA_BASE_URL) {
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
 * Fetch and inflate the pack for `lang` (`en`, `de`, `gn`, …).
 *
 * Node.js prefers the locally installed `lingotweaker-data` package; browsers
 * (and any environment without it) fall back to the release assets for this
 * package version. Pass `baseUrl` to use different data — the Pages demo
 * (`https://fiduswriter.github.io/LingoTweaker`) or a local directory.
 */
export async function fetchPack(lang, options = {}) {
  const { fetch: fetchImpl = fetch } = options;
  let { baseUrl } = options;
  if (!baseUrl) {
    const local = await localPack(lang);
    if (local) return local;
    baseUrl = DEFAULT_DATA_BASE_URL;
  }
  const url = packUrl(lang, baseUrl);
  const response = await fetchImpl(url);
  if (!response.ok) {
    throw new Error(`cannot load ${url}: HTTP ${response.status}`);
  }
  return decompressPack(await response.arrayBuffer());
}

/**
 * Read and inflate a pack from the installed `lingotweaker-data` package.
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