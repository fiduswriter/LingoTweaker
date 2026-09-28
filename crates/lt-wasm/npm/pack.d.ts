// Type definitions for the data-pack helpers in pack.js.

/** Default base URL of the per-release GitHub Release data assets. */
export const DEFAULT_DATA_BASE_URL: string

/** The URL of the release data manifest (`{ <lang>: { file, bytes, sha256 } }`). */
export function manifestUrl(baseUrl?: string): string

/** The URL of the gzipped pack for `lang`. */
export function packUrl(lang: string, baseUrl?: string): string

/** Inflate a gzipped pack (browser `DecompressionStream`, else Node `zlib`). */
export function decompressPack(
    bytes: Uint8Array | ArrayBuffer
): Promise<Uint8Array>

/**
 * Fetch and inflate the pack for `lang` (`en`, `de`, `gn`, …).
 *
 * Node.js prefers the locally installed `lingotweaker-data-<lang>` data
 * packages; browsers (and any environment without them) fall back to the
 * release assets for this package version. Pass `baseUrl` to use different
 * data.
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
