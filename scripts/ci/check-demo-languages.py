#!/usr/bin/env python3
"""Guard: every wired language must be in the Pages demo.

The demo picker (`demo/src/languages.js`) and the pack build
(`demo/scripts/build-packs.sh`) each carry a language list. `data/<cc>/`
directories are the single source of truth for the wired set (they are
produced by the per-language import), so this check fails when a language was
added to the engine but forgotten in the demo.

Usage: scripts/ci/check-demo-languages.py
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SKIP = {"core", "messages", "schemas"}  # shared data, not languages


def wired() -> set[str]:
    langs = set()
    for path in (ROOT / "data").iterdir():
        if not path.is_dir() or path.name in SKIP:
            continue
        if (path / "rules").is_dir() or (path / "hunspell").is_dir() or (path / "dictionaries").is_dir():
            langs.add(path.name)
    return langs


def demo_codes() -> set[str]:
    text = (ROOT / "demo/src/languages.js").read_text(encoding="utf-8")
    return set(re.findall(r'pack:\s*"([a-z]{2,3})"', text))


def pack_langs() -> set[str]:
    text = (ROOT / "demo/scripts/build-packs.sh").read_text(encoding="utf-8")
    m = re.search(r'LT_DEMO_LANGS:-([^}"]+)\}', text)
    return set(m.group(1).split()) if m else set()


def main() -> int:
    wired_langs = wired()
    missing_demo = sorted(wired_langs - demo_codes())
    missing_packs = sorted(wired_langs - pack_langs())
    extra = sorted((demo_codes() | pack_langs()) - wired_langs)
    if missing_demo:
        print(f"demo picker missing: {' '.join(missing_demo)}", file=sys.stderr)
    if missing_packs:
        print(f"build-packs.sh missing: {' '.join(missing_packs)}", file=sys.stderr)
    if extra:
        print(f"demo lists unknown data dirs: {' '.join(extra)}", file=sys.stderr)
    if missing_demo or missing_packs or extra:
        return 1
    print(f"demo languages ok ({len(wired_langs)}): {' '.join(sorted(wired_langs))}")
    return 0


if __name__ == "__main__":
    sys.exit(main())