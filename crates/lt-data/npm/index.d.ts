/** Loader for the LingoTweaker engine's runtime data (code-only package). */

export interface DataFileEntry {
  /**
   * Path relative to the per-language package `lingotweaker-data-<lang>`
   * (e.g. `packs/en.pack.gz`), not to this package.
   */
  file: string;
  bytes: number;
  sha256: string;
}

export interface DataLanguageEntry {
  pack: DataFileEntry;
}

export interface LingoTweakerDataManifest {
  version: string;
  upstream_commit: string;
  languages: Record<string, DataLanguageEntry>;
}

/** Languages with a published data pack (base codes: `en`, `de`, `gn`, …). */
export function languages(): string[];

/**
 * Absolute path of the gzipped data pack for `lang`.
 *
 * The pack lives in the per-language package `lingotweaker-data-<lang>`,
 * which must be installed separately (`npm install lingotweaker-data-<lang>`).
 */
export function packPath(lang: string): string;

/**
 * The data manifest (version, upstream commit, per-language sha256).
 *
 * The `file` paths are relative to the per-language package
 * `lingotweaker-data-<lang>`, not to this package.
 */
export function dataManifest(): LingoTweakerDataManifest;