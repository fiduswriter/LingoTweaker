#!/usr/bin/env python3
"""Generate the compiled Lindera dictionaries under `data/<lang>/dictionary/`.

The dictionaries are **generated artifacts**, not vendored source: they are
large (tens of MB) and rebuilt from Lindera's pinned release archives, so they
are gitignored (`data/*/dictionary/`). The engine (`lt-lindera`) loads them
through `lt_data::fs`, and `scripts/data/build-packs.sh` packs them.

The release archives are byte-identical to a fresh `lindera` `embed-*` build
for the same version, so this avoids a slow build and pins the exact bytes by
SHA-256.

Usage:
    python3 tools/lindera/build-dict.py [lang ...]   # ja / zh; default: all
    python3 tools/lindera/build-dict.py --check      # verify, do not write
    python3 tools/lindera/build-dict.py --force ja   # re-extract even if present
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import shutil
import sys
import urllib.request
import zipfile

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
DATA_DIR = REPO_ROOT / "data"
CACHE_DIR = REPO_ROOT / "target" / "lindera-cache"

LINDERA_VERSION = "6.0.0"

# lang -> (dictionary name, release asset, sha256 of the archive)
DICTS: dict[str, tuple[str, str, str]] = {
    "ja": ("ipadic", f"lindera-ipadic-{LINDERA_VERSION}.zip",
           "8433dbbb80d7588a565fb9247c1ac7aed3ca50c7463329e495f8bd905aece356"),
    "zh": ("jieba", f"lindera-jieba-{LINDERA_VERSION}.zip",
           "ad37064b2ff8c738342c80229e59dac2845747f8e475b4223f07c926db72a9fd"),
}


def archive_url(asset: str) -> str:
    return (
        f"https://github.com/lindera/lindera/releases/download/"
        f"v{LINDERA_VERSION}/{asset}"
    )


def sha256_file(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def fetch_archive(asset: str, expected: str) -> pathlib.Path:
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    path = CACHE_DIR / asset
    if not path.exists() or sha256_file(path) != expected:
        print(f"downloading {archive_url(asset)}")
        tmp = path.with_suffix(path.suffix + ".part")
        urllib.request.urlretrieve(archive_url(asset), tmp)
        actual = sha256_file(tmp)
        if actual != expected:
            tmp.unlink(missing_ok=True)
            sys.exit(f"sha256 mismatch for {asset}: {actual} != {expected}")
        tmp.replace(path)
    else:
        print(f"cached {path}")
    return path


def install(lang: str, force: bool) -> None:
    name, asset, sha = DICTS[lang]
    dest = DATA_DIR / lang / "dictionary"
    if dest.is_dir() and not force and (dest / "dict.words").exists():
        print(f"[{lang}] {dest} already present (use --force to rebuild)")
        return

    archive = fetch_archive(asset, sha)
    staging = CACHE_DIR / f"extract-{lang}"
    if staging.exists():
        shutil.rmtree(staging)
    with zipfile.ZipFile(archive) as zf:
        zf.extractall(staging)
    # The archive root is the dictionary directory (`lindera-<name>/`).
    roots = [p for p in staging.iterdir() if p.is_dir()]
    if len(roots) != 1:
        sys.exit(f"unexpected archive layout in {archive}: {roots}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists():
        shutil.rmtree(dest)
    shutil.move(str(roots[0]), dest)
    shutil.rmtree(staging, ignore_errors=True)
    print(f"[{lang}] wrote {dest} ({name}, lindera {LINDERA_VERSION})")


def check(langs: list[str]) -> int:
    rc = 0
    for lang in langs:
        _, asset, sha = DICTS[lang]
        dest = DATA_DIR / lang / "dictionary"
        if not (dest / "dict.words").exists():
            print(f"[{lang}] MISSING {dest}")
            rc = 1
            continue
        archive = CACHE_DIR / asset
        if not archive.exists():
            print(f"[{lang}] present, archive not cached (cannot byte-verify)")
            continue
        with zipfile.ZipFile(archive) as zf:
            inner = zf.namelist()[0].split("/")[0]
            bad = 0
            for member in zf.namelist():
                rel = pathlib.PurePosixPath(member)
                if len(rel.parts) != 2:
                    continue
                on_disk = dest / rel.parts[1]
                if not on_disk.exists():
                    bad += 1
                    print(f"[{lang}] missing file {rel.parts[1]}")
                    continue
                if sha256_file(on_disk) != hashlib.sha256(zf.read(member)).hexdigest():
                    bad += 1
                    print(f"[{lang}] differs {rel.parts[1]}")
            print(f"[{lang}] {'OK' if bad == 0 and inner else f'{bad} mismatches'}")
            if bad:
                rc = 1
    return rc


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("langs", nargs="*", help="ja / zh (default: all)")
    ap.add_argument("--force", action="store_true", help="re-extract even if present")
    ap.add_argument("--check", action="store_true", help="verify only, do not write")
    args = ap.parse_args()

    langs = args.langs or list(DICTS)
    unknown = [l for l in langs if l not in DICTS]
    if unknown:
        sys.exit(f"unknown language(s) {unknown}; known: {list(DICTS)}")

    if args.check:
        sys.exit(check(langs))
    for lang in langs:
        install(lang, args.force)


if __name__ == "__main__":
    main()