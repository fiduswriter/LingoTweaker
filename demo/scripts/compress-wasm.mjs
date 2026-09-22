// Emit a `.gz` sidecar next to every wasm asset in dist/. GitHub Pages
// serves files without content compression, so the worker fetches the
// sidecar and gunzips it in place (see src/worker.js): the ~6 MB engine
// binary crosses the wire as ~2.3 MB. The plain `.wasm` stays in place as
// the fallback for `vite dev` and hosts that do compress on the fly.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { join } from "node:path";

const assetsDir = new URL("../dist/assets/", import.meta.url).pathname;

for (const name of readdirSync(assetsDir)) {
  if (!name.endsWith(".wasm")) {
    continue;
  }
  const path = join(assetsDir, name);
  // mtime 0 keeps the output reproducible.
  const compressed = gzipSync(readFileSync(path), { level: 9, mtime: 0 });
  writeFileSync(`${path}.gz`, compressed);
  console.log(`compress-wasm: ${name} -> ${compressed.length} bytes gz`);
}
