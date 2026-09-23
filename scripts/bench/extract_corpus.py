#!/usr/bin/env python3
"""Extract unique sentences from the parity corpora for benchmarking.

Reads docs/parity/corpora/<lang>-examples.jsonl, dedupes the `text` field,
caps the number of sentences, and writes one sentence per line to
scripts/bench/results/<lang>.txt.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CORPORA = ROOT / "docs" / "parity" / "corpora"
OUT = Path(__file__).resolve().parent / "results"
CAP = int(sys.argv[1]) if len(sys.argv) > 1 else 5000


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for lang in ["en", "de", "es", "pt", "fr"]:
        src = CORPORA / f"{lang}-examples.jsonl"
        seen = set()
        lines = []
        with src.open(encoding="utf-8") as f:
            for row in f:
                text = json.loads(row).get("text", "").replace("\r", "").strip()
                if not text or text in seen:
                    continue
                seen.add(text)
                lines.append(text)
                if len(lines) >= CAP:
                    break
        dst = OUT / f"{lang}.txt"
        dst.write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"{lang}: {len(lines)} unique sentences (cap {CAP}) -> {dst}")


if __name__ == "__main__":
    main()
