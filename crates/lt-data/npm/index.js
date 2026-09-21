"use strict";

// Runtime data for the LingoTweaker engine, one gzipped pack per language.
//
//   const data = require("lingotweaker-data");
//   const { Engine } = require("lingotweaker");
//   const engine = new Engine("en-US", { dataDir: data.packPath("en") });

const { join } = require("node:path");
const manifest = require("./manifest.json");

const PACKS_DIR = join(__dirname, "packs");

/** Languages shipped in this package (base codes: `en`, `de`, `gn`, …). */
function languages() {
  return Object.keys(manifest.languages ?? {}).sort();
}

/** Absolute path of the gzipped data pack for `lang`. */
function packPath(lang) {
  const entry = manifest.languages?.[lang];
  if (!entry?.pack?.file) {
    throw new Error(
      `lingotweaker-data: no pack for ${JSON.stringify(lang)}; ` +
        `available: ${languages().join(", ")}`,
    );
  }
  return join(__dirname, entry.pack.file);
}

/** The full data manifest (version, upstream commit, per-language sha256). */
function dataManifest() {
  return manifest;
}

module.exports = {
  languages,
  packPath,
  manifest: dataManifest,
  packsDir: PACKS_DIR,
};