#!/usr/bin/env python3
"""Regenerate the German corpus word list for the speller probes.

Extracts every unique whitespace-separated word from
`docs/parity/corpora/de-examples.jsonl` (all examples, correct and
incorrect), keeps tokens without spaces and at most 60 chars, and writes one
word per line (sorted, deterministic). The list feeds
`scripts/oracle/de/probe-speller.sh` / `probe-morfo.sh` and the Rust
`spell_probe` / `morfo_rule_probe` examples.

Usage: python3 scripts/oracle/de/gen-corpus-words.py [out]
       (default /tmp/de-corpus-words.txt)
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CORPUS = ROOT / "docs" / "parity" / "corpora" / "de-examples.jsonl"


def main() -> None:
    out_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/de-corpus-words.txt")
    words: set[str] = set()
    for line in CORPUS.read_text(encoding="utf-8").splitlines():
        entry = json.loads(line)
        for raw in entry["text"].split():
            word = raw.strip("\"'„“”‚‘’()[]{}«»…")
            if not word or " " in word or "\t" in word:
                continue
            if len(word) > 60:
                continue
            if not re.search(r"[A-Za-zÄÖÜäöüß]", word):
                continue
            words.add(word)
    ordered = sorted(words)
    out_path.write_text("\n".join(ordered) + "\n", encoding="utf-8")
    print(f"wrote {out_path}: {len(ordered)} words", file=sys.stderr)


if __name__ == "__main__":
    main()
