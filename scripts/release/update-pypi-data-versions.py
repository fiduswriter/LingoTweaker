#!/usr/bin/env python3
"""Bump the per-language PyPI data versions when a language's data changes.

The `lingotweaker-data-<lang>` distributions are versioned independently of the
engine release. This script hashes everything that goes into each language's
wheel (`data/manifest.json`, `data/core/**`, `data/messages/**`,
`data/<lang>/**`) and compares it with the hash recorded in
`data/pypi-versions.json`:

* hash unchanged -> version stays, nothing to re-upload;
* hash changed   -> patch-bump that language's version and record the new hash;
* language new   -> start at the seed version (`data/pypi-version`).

Run it after changing data, then commit `data/pypi-versions.json` and publish
with `scripts/release/publish-pypi-data.sh` (which uploads only the versions
that are not on PyPI yet).

Usage:
  scripts/release/update-pypi-data-versions.py              # detect + bump
  scripts/release/update-pypi-data-versions.py --check      # exit 1 if changed
  scripts/release/update-pypi-data-versions.py --lang de    # one language
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_DATA = ROOT / "data"
DEFAULT_REGISTRY = ROOT / "data" / "pypi-versions.json"
DEFAULT_SEED = ROOT / "data" / "pypi-version"

# The shared parts go into every wheel, so a change there bumps every language.
SHARED = ("core", "messages")


def data_languages(data_dir: Path) -> list[str]:
    """Languages that have a rule data directory (`data/<lang>/rules`)."""
    return sorted(
        d.name for d in data_dir.iterdir() if d.is_dir() and (d / "rules").is_dir()
    )


def wheel_files(data_dir: Path, lang: str) -> list[Path]:
    """The files `publish-pypi-data.sh` copies into the `<lang>` wheel."""
    files: list[Path] = []
    manifest = data_dir / "manifest.json"
    if manifest.is_file():
        files.append(manifest)
    for sub in (*SHARED, lang):
        base = data_dir / sub
        if base.is_dir():
            files.extend(
                p
                for p in base.rglob("*")
                if p.is_file() and "dictionary" not in p.relative_to(base).parts
            )
    return sorted(files)


def hash_language(data_dir: Path, lang: str) -> str:
    """Deterministic content hash (relative path + bytes), ignoring mtimes."""
    digest = hashlib.sha256()
    for path in wheel_files(data_dir, lang):
        digest.update(path.relative_to(data_dir).as_posix().encode())
        digest.update(b"\0")
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1 << 20), b""):
                digest.update(chunk)
        digest.update(b"\0")
    return "sha256:" + digest.hexdigest()


def bump(version: str) -> str:
    """Increment the last numeric component (`0.1.1` -> `0.1.2`)."""
    parts = version.split(".")
    for i in range(len(parts) - 1, -1, -1):
        if parts[i].isdigit():
            parts[i] = str(int(parts[i]) + 1)
            return ".".join(parts)
    return version + ".1"


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--data", type=Path, default=DEFAULT_DATA)
    parser.add_argument("--registry", type=Path, default=DEFAULT_REGISTRY)
    parser.add_argument(
        "--seed",
        type=Path,
        default=DEFAULT_SEED,
        help="version used for languages not in the registry yet",
    )
    parser.add_argument(
        "--lang", action="append", help="only this language (repeatable)"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="do not write; exit non-zero when anything changed or is new",
    )
    args = parser.parse_args()

    seed = args.seed.read_text().strip() if args.seed.is_file() else "0.1.1"

    registry: dict = {"languages": {}}
    if args.registry.is_file():
        registry = json.loads(args.registry.read_text())
    languages_state: dict = registry.setdefault("languages", {})

    langs = sorted(set(args.lang) if args.lang else data_languages(args.data))
    if not langs:
        print("no languages found", file=sys.stderr)
        return 1

    added: list[tuple[str, str]] = []
    changed: list[tuple[str, str, str]] = []
    unchanged: list[str] = []
    for lang in langs:
        digest = hash_language(args.data, lang)
        entry = languages_state.get(lang)
        if entry is None:
            languages_state[lang] = {"version": seed, "hash": digest}
            added.append((lang, seed))
        elif entry.get("hash") != digest:
            old = entry.get("version", seed)
            new = bump(old)
            languages_state[lang] = {"version": new, "hash": digest}
            changed.append((lang, old, new))
        else:
            unchanged.append(lang)

    for lang, version in added:
        print(f"new:       {lang} -> {version}")
    for lang, old, new in changed:
        print(f"changed:   {lang} {old} -> {new}")
    if unchanged:
        print(f"unchanged: {len(unchanged)} language(s)")

    if args.check:
        return 1 if (added or changed) else 0

    registry["generated_by"] = "scripts/release/update-pypi-data-versions.py"
    args.registry.write_text(json.dumps(registry, indent=2, sort_keys=True) + "\n")
    print(f"wrote {args.registry}")
    return 0


if __name__ == "__main__":
    sys.exit(main())