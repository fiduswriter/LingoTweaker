// Runtime data for the LingoTweaker engine, one gzipped pack per language.
//
//   import { packPath } from "lingotweaker-data";
//   import { Engine } from "lingotweaker";
//   const engine = new Engine("en-US", { dataDir: packPath("en") });

import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const manifest = require("./manifest.json");
const here = dirname(fileURLToPath(import.meta.url));

/** Directory holding the gzipped packs. */
export const packsDir = join(here, "packs");

/** Languages shipped in this package (base codes: `en`, `de`, `gn`, …). */
export function languages() {
  return Object.keys(manifest.languages ?? {}).sort();
}

/** Absolute path of the gzipped data pack for `lang`. */
export function packPath(lang) {
  const entry = manifest.languages?.[lang];
  if (!entry?.pack?.file) {
    throw new Error(
      `lingotweaker-data: no pack for ${JSON.stringify(lang)}; ` +
        `available: ${languages().join(", ")}`,
    );
  }
  return join(here, entry.pack.file);
}

/** The full data manifest (version, upstream commit, per-language sha256). */
export function dataManifest() {
  return manifest;
}