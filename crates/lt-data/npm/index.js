"use strict";

// Loader for the LingoTweaker engine's runtime data.
//
// This package is code-only: the packs ship in one small package per language
// (`lingotweaker-data-<lang>`, mirroring the per-language PyPI data
// distributions). Install the languages you need alongside it:
//
//   npm install lingotweaker-data lingotweaker-data-en
//
//   const data = require("lingotweaker-data");
//   const { Engine } = require("lingotweaker");
//   const engine = new Engine("en-US", { dataDir: data.packPath("en") });

const { join } = require("node:path");
const { createRequire } = require("node:module");
const manifest = require("./manifest.json");

// Resolve from this package's location first (npm's flat node_modules puts the
// per-language packages next to it); fall back to the process working
// directory for layouts where they are not siblings (pnpm, workspace hoisting
// differences), where the per-language package is resolvable from the
// consuming project but not from inside this package's store path.
const hereRequire = createRequire(__filename);
const cwdRequire = createRequire(join(process.cwd(), "index.js"));

/** Languages with a published data pack (base codes: `en`, `de`, `gn`, …). */
function languages() {
  return Object.keys(manifest.languages ?? {}).sort();
}

/**
 * Absolute path of the gzipped data pack for `lang`.
 *
 * The pack lives in the per-language package `lingotweaker-data-<lang>` at
 * the manifest's `file` path (e.g. `packs/en.pack.gz`); it must be installed
 * separately (`npm install lingotweaker-data-<lang>`).
 */
function packPath(lang) {
  const entry = manifest.languages?.[lang];
  if (!entry?.pack?.file) {
    throw new Error(
      `lingotweaker-data: no pack for ${JSON.stringify(lang)}; ` +
        `available: ${languages().join(", ")}`,
    );
  }
  const name = `lingotweaker-data-${lang}`;
  let cause;
  for (const req of [hereRequire, cwdRequire]) {
    try {
      return req.resolve(`${name}/${entry.pack.file}`);
    } catch (err) {
      cause = err;
    }
  }
  throw new Error(
    `lingotweaker-data: the ${JSON.stringify(lang)} pack is not installed; ` +
      `run \`npm install ${name}\` (available: ${languages().join(", ")})`,
    { cause },
  );
}

/**
 * The data manifest (version, upstream commit, per-language sha256).
 *
 * The `file` paths are relative to the per-language package
 * `lingotweaker-data-<lang>`, not to this package.
 */
function dataManifest() {
  return manifest;
}

module.exports = {
  languages,
  packPath,
  manifest: dataManifest,
};
