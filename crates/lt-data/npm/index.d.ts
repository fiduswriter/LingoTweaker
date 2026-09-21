/** Runtime data for the LingoTweaker engine, one gzipped pack per language. */

export interface DataFileEntry {
  /** Path relative to the package root, e.g. `packs/en.pack.gz`. */
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

/** Languages shipped in this package (base codes: `en`, `de`, `gn`, …). */
export function languages(): string[];

/** Absolute path of the gzipped data pack for `lang`. */
export function packPath(lang: string): string;

/** The full data manifest (version, upstream commit, per-language sha256). */
export function dataManifest(): LingoTweakerDataManifest;

/** Directory holding the gzipped packs. */
export const packsDir: string;