// Type definitions for the data-pack helpers in pack.js.

/** The URL of the data manifest (`{ <lang>: { file, bytes, sha256 } }`) under `baseUrl`. */
export function manifestUrl(baseUrl: string): string

/** The URL of the gzipped pack for `lang` under `baseUrl`. */
export function packUrl(lang: string, baseUrl: string): string

/** Inflate a gzipped pack (browser `DecompressionStream`, else Node `zlib`). */
export function decompressPack(
    bytes: Uint8Array | ArrayBuffer
): Promise<Uint8Array>

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
export function fetchPack(
    lang: string,
    options?: {baseUrl?: string; fetch?: typeof fetch}
): Promise<Uint8Array>

/**
 * Read and inflate a pack from the locally installed data packages (the
 * code-only `lingotweaker-data` loader resolving `lingotweaker-data-<lang>`).
 * Returns `null` outside Node.js, or when the package/language is missing.
 */
export function localPack(lang: string): Promise<Uint8Array | null>
