#!/usr/bin/env python3
"""Vendor a pristine crates.io release of `lindera-dictionary`, then patch its
file reads to go through `lt_data::fs` so dictionary loading works from an
in-memory data pack (wasm) as well as from disk.

The patched copy is written to `vendor/lindera-dictionary/` and wired into the
workspace with a `[patch.crates-io]` entry. Only the *runtime read* path is
touched (the loader's `read_file` and the `exists`/`is_dir` probes); the
build-time `builder`/`assets` code is left alone.

Usage:
    python3 tools/vendor/patch_lindera.py [--version 6.0.0] [--src DIR]

`--src` reuses an already-extracted pristine source tree instead of
downloading from crates.io (used by the test below and offline rebuilds).
"""

from __future__ import annotations

import argparse
import io
import pathlib
import re
import shutil
import sys
import tarfile
import urllib.request

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
VENDOR_DIR = REPO_ROOT / "vendor" / "lindera-dictionary"

# (file, expected substring, replacement) for the runtime read path only.
# Every entry asserts its `expected` substring exists before replacing, so an
# upstream change fails loudly rather than silently vendoring unpatched code.
RUNTIME_PATCHES: list[tuple[str, str, str]] = [
    # 1. Route the one file-read helper the loaders use through lt_data::fs.
    (
        "src/util.rs",
        """pub fn read_file(filename: &Path) -> LinderaResult<Vec<u8>> {
    let mut input_read = File::open(filename).map_err(|err| {
        LinderaErrorKind::Io
            .with_error(err)
            .add_context(format!("Failed to open file: {}", filename.display()))
    })?;
    let mut buffer = Vec::new();
    input_read.read_to_end(&mut buffer).map_err(|err| {
        LinderaErrorKind::Io.with_error(err).add_context(format!(
            "Failed to read file contents: {}",
            filename.display()
        ))
    })?;
    Ok(buffer)
}""",
        """pub fn read_file(filename: &Path) -> LinderaResult<Vec<u8>> {
    // Patched by tools/vendor/patch_lindera.py: read through lt_data::fs so a
    // mounted in-memory data pack is used when no real file system exists.
    lt_data::fs::read(filename).map_err(|err| {
        LinderaErrorKind::Io.with_error(err).add_context(format!(
            "Failed to read file contents: {}",
            filename.display()
        ))
    })
}""",
    ),
    # 2. Keep imports warning-free now that read_file no longer opens a File.
    (
        "src/util.rs",
        "use std::fs::File;\nuse std::io::{Read, Write};",
        "use std::io::Write;\n#[cfg(feature = \"mmap\")]\nuse std::fs::File;\n#[cfg(feature = \"mmap\")]\nuse std::io::Read;",
    ),
    # 3. Directory probes must also resolve inside a mount.
    (
        "src/dictionary.rs",
        "if !dict_path.exists() {",
        "if !lt_data::fs::exists(dict_path) {",
    ),
    (
        "src/dictionary.rs",
        "if !dict_path.is_dir() {",
        "if !lt_data::fs::is_dir(dict_path) {",
    ),
    # 4. Keep the crate warning-free with the `mmap` feature off.
    (
        "src/util.rs",
        "use std::sync::Arc;",
        "#[cfg(feature = \"mmap\")]\nuse std::sync::Arc;",
    ),
    (
        "src/dictionary.rs",
        """    pub fn load_from_path_with_options(dict_path: &Path, use_mmap: bool) -> LinderaResult<Self> {
        // Verify that the dictionary directory exists""",
        """    pub fn load_from_path_with_options(dict_path: &Path, use_mmap: bool) -> LinderaResult<Self> {
        // Patched by tools/vendor/patch_lindera.py: unused with the `mmap`
        // feature off (the loaders always take the plain-read path).
        let _ = use_mmap;
        // Verify that the dictionary directory exists""",
    ),
]

LT_DATA_DEP = (
    '\n[dependencies.lt-data]\n'
    'path = "../../crates/lt-data"\n'
    'default-features = false\n'
)


def download_crate(version: str, dest: pathlib.Path) -> pathlib.Path:
    """Download and extract the crate into `dest`, returning the source root."""
    url = (
        f"https://static.crates.io/crates/lindera-dictionary/"
        f"lindera-dictionary-{version}.crate"
    )
    print(f"downloading {url}")
    with urllib.request.urlopen(url, timeout=60) as resp:
        data = resp.read()
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tf:
        tf.extractall(dest, filter="data")
    return dest / f"lindera-dictionary-{version}"


def apply_patches(src: pathlib.Path) -> None:
    for rel, expected, replacement in RUNTIME_PATCHES:
        path = src / rel
        text = path.read_text(encoding="utf-8")
        if expected not in text:
            sys.exit(
                f"patch anchor not found in {rel!r}:\n---\n{expected}\n---\n"
                "Upstream lindera-dictionary changed; update tools/vendor/patch_lindera.py."
            )
        path.write_text(text.replace(expected, replacement, 1), encoding="utf-8")
        print(f"patched {rel}: {re.split(r'[ (]', expected.strip())[0]} ...")


def add_dependency(src: pathlib.Path) -> None:
    path = src / "Cargo.toml"
    text = path.read_text(encoding="utf-8")
    # Turn off the default `mmap` feature: `lindera` requests this package with
    # default features on, and memmap2 cannot work from an in-memory mount (or
    # on wasm). The loaders then always take their plain-read path, which is
    # the one routed through `lt_data::fs` above.
    if 'default = ["mmap"]' not in text:
        sys.exit(
            "expected `default = [\"mmap\"]` in vendored Cargo.toml; update "
            "tools/vendor/patch_lindera.py"
        )
    text = text.replace('default = ["mmap"]', "default = []", 1)
    print("disabled default mmap feature")
    if "lt-data" in text:
        print("lt-data dependency already present")
    else:
        text = text.rstrip("\n") + "\n" + LT_DATA_DEP
        print("added lt-data dependency to vendored Cargo.toml")
    path.write_text(text, encoding="utf-8")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", default="6.0.0")
    ap.add_argument("--src", type=pathlib.Path, default=None)
    args = ap.parse_args()

    if VENDOR_DIR.exists():
        shutil.rmtree(VENDOR_DIR)
    VENDOR_DIR.mkdir(parents=True)

    if args.src is not None:
        print(f"copying pristine source from {args.src}")
        shutil.copytree(args.src, VENDOR_DIR, dirs_exist_ok=True)
    else:
        tmp = REPO_ROOT / "vendor" / ".lindera-download"
        if tmp.exists():
            shutil.rmtree(tmp)
        tmp.mkdir(parents=True)
        extracted = download_crate(args.version, tmp)
        shutil.copytree(extracted, VENDOR_DIR, dirs_exist_ok=True)
        shutil.rmtree(tmp)

    apply_patches(VENDOR_DIR)
    add_dependency(VENDOR_DIR)
    print(f"vendored + patched lindera-dictionary-{args.version} -> {VENDOR_DIR}")


if __name__ == "__main__":
    main()