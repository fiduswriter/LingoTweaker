#!/usr/bin/env python3
"""Regenerate the German hunspell differential word list.

The list feeds `scripts/oracle/de/probe-hunspell.sh` (Java, native
libhunspell) and `cargo run --release --example hunspell_probe -- <aff> <dic>
<words.txt>` (in-tree Rust port); both emit `word<TAB>spell()` lines that can
be diffed directly. Deterministic (`random.seed(42)`).

Usage: python3 scripts/oracle/de/gen-hunspell-words.py [out]
       (default /tmp/de-hunspell-words.txt)
"""
import random
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DE = ROOT / "data" / "de"


def main() -> None:
    out_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/de-hunspell-words.txt")
    out: list[str] = []
    seen: set[str] = set()

    def add(word: str) -> None:
        word = word.strip()
        if not word or word.startswith("#") or " " in word or "\t" in word:
            return
        if len(word) > 60 or word in seen:
            return
        seen.add(word)
        out.append(word)

    for i, line in enumerate((DE / "hunspell" / "de_DE.dic").read_text(encoding="utf-8").splitlines()):
        if i == 0 or not line or line.startswith(("\t", "#")):
            continue
        add(line.split("/", 1)[0])
    for name in [
        "hunspell/spelling.txt",
        "hunspell/spelling_merged.txt",
        "hunspell/ignore.txt",
        "hunspell/spelling-de-AT.txt",
        "hunspell/spelling-de-CH.txt",
    ]:
        for line in (DE / name).read_text(encoding="utf-8").splitlines():
            add(line)
    for name in [
        "words/added.txt",
        "words/removed.txt",
        "words/words_infix_s.txt",
        "words/compounds.txt",
        "words/eigennamen_gross.txt",
        "words/multitoken-suggest.txt",
    ]:
        path = DE / name
        if not path.exists():
            continue
        for line in path.read_text(encoding="utf-8").splitlines():
            add(line.split("\t")[0])

    random.seed(42)
    base = [w for w in out if 5 <= len(w) <= 18 and re.fullmatch(r"[A-Za-zÄÖÜäöüß]+", w)]
    sample = random.sample(base, min(40000, len(base)))
    letters = "abcdefghijklmnopqrstuvwxyzäöüß"
    for word in sample:
        kind = random.choice(["del", "swap", "ins", "sub", "case"])
        i = random.randrange(len(word))
        if kind == "del":
            add(word[:i] + word[i + 1 :])
        elif kind == "swap" and i + 1 < len(word):
            add(word[:i] + word[i + 1] + word[i] + word[i + 2 :])
        elif kind == "ins":
            add(word[:i] + random.choice(letters) + word[i:])
        elif kind == "sub":
            add(word[:i] + random.choice(letters) + word[i + 1 :])
        else:
            add(word.lower())
            add(word.upper())
            add(word.capitalize())

    out_path.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"{len(out)} words -> {out_path}")


if __name__ == "__main__":
    main()
