# Vendored dependencies

## `lindera-dictionary` (patched)

`vendor/lindera-dictionary/` is a copy of the crates.io release
`lindera-dictionary 6.0.0` with one behavioural change: the **runtime
dictionary read path** goes through `lt_data::fs` instead of `std::fs`, so a
compiled dictionary can be loaded from a mounted in-memory data pack — which is
how wasm builds without a file system are served (see `crates/lt-data/src/fs.rs`).

It is pulled in with the workspace `[patch.crates-io]` entry and is excluded
from the workspace (its own tests/lints are not run).

### Regenerating

```sh
python3 tools/vendor/patch_lindera.py            # downloads 6.0.0 from crates.io
python3 tools/vendor/patch_lindera.py --src DIR  # or reuse a pristine tree
```

The script asserts every anchor it replaces, so an upstream change fails loudly
instead of silently vendoring unpatched code.

### What the patch changes

| file | change |
|---|---|
| `src/util.rs` | `read_file` reads via `lt_data::fs::read`; `std::fs::File` / `io::Read` imports become `mmap`-only |
| `src/dictionary.rs` | `dict_path.exists()` / `is_dir()` become `lt_data::fs::exists` / `is_dir`; `let _ = use_mmap` to stay warning-free |
| `Cargo.toml` | `default = []` (drops `mmap`; memmap2 cannot read a mounted pack or run on wasm); adds the `lt-data` path dependency |

Only the loader read path is touched. The build-time `builder` module keeps
using `std::fs` (it is not exercised at runtime); `assets` is behind the
`build_rs` feature, which is not enabled.

### License

`lindera-dictionary` is MIT-licensed (`https://github.com/lindera/lindera`).
The MIT text is reproduced below.

```
MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

The *dictionary data* ships separately (data packs, not this crate) and carries
its own terms: IPADIC is the NAIST/ICOT notice; the Lindera jieba dictionary is
MIT plus CC-CEDICT under CC BY-SA 4.0.