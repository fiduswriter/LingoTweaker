#!/usr/bin/env python3
"""Exact membership check for candidate NN_BOKMAAL_FORMS pairs.

Reads `wrong=suggestion` pairs (one per line, `#` comments kept) and reports
for each pair whether the wrong form is accepted by the Nynorsk dictionary
(it must NOT be) and whether every suggestion is accepted (it must be).
Used while curating `data/nn/rules/bokmaal_forms.txt`; the engine tests are
the final proof.

    python3 tools/nn-dict/check_pairs.py data/nn/rules/bokmaal_forms.txt
"""

from __future__ import annotations

import argparse
import sys


def read_wordlist(path: str) -> set[str]:
    words: set[str] = set()
    with open(path, encoding="utf-8") as fh:
        for i, line in enumerate(fh):
            if i == 0:
                continue
            word = line.split("\t")[0].split("/")[0].strip()
            if word:
                words.add(word)
    return words


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("pairs")
    ap.add_argument("--nn", default="data/nn/hunspell/nn_NO.dic")
    args = ap.parse_args()

    nn = read_wordlist(args.nn)
    bad = 0
    for raw in open(args.pairs, encoding="utf-8"):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        wrong, _, right = line.partition("=")
        wrong = wrong.strip().lower()
        suggestions = [s.strip().lower() for s in right.split("|")]
        problems = []
        if wrong in nn:
            problems.append("wrong form IS valid nn")
        for s in suggestions:
            if s not in nn:
                problems.append(f"suggestion {s!r} NOT valid nn")
        if problems:
            bad += 1
            print(f"FAIL {line}: {'; '.join(problems)}", file=sys.stderr)
        else:
            print(f"ok   {line}")
    if bad:
        print(f"{bad} pair(s) failed", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
