#!/usr/bin/env python3
"""Derive Bokmål-form candidates for the Nynorsk interference rule.

`data/nn/rules/bokmaal_forms.txt` (rule `NN_BOKMAAL_FORMS`) flags common
Bokmål-only forms in Nynorsk text. This tool generates the *candidate* list:
words that are accepted by the Bokmål speller (`nb_NO.dic`) but rejected by
the Nynorsk speller (`nn_NO.dic`), i.e. forms that the `nn` speller alone
would flag as unknown without offering the Nynorsk equivalent.

Both dictionaries are full-form word lists (no affix flags on the entries,
see `data/nn/README.md`), so membership is a plain set lookup after stripping
the count line, affix flags (`word/FLAGS`) and optional tab-separated
morphology fields.

The candidates are **not** vendored verbatim: the generated list contains
hundreds of technical/proper-noun entries that would produce noise. The final
curated list in `data/nn/rules/bokmaal_forms.txt` is hand-picked from this
output (shared forms that are valid in both standards must not be listed;
each listed form must be rejected by the nn dictionary and each suggestion
accepted by it — `cargo test -p lt --test nynorsk` proves both against the
engine's own speller).

Usage:

    python3 tools/nn-dict/derive_bokmaal_forms.py \
        --nb data/no/hunspell/nb_NO.dic --nn data/nn/hunspell/nn_NO.dic \
        [--min-len 3] [--limit 200] [--grep ord]

Options after the candidates are printed: pipe through `grep -w ord` to check
a specific word family, e.g. `--grep jente` shows `jenter`/`jentene` and
their Nynorsk counterparts.
"""

from __future__ import annotations

import argparse


def read_wordlist(path: str) -> set[str]:
    words: set[str] = set()
    with open(path, encoding="utf-8") as fh:
        for i, line in enumerate(fh):
            if i == 0:
                continue  # entry count
            word = line.split("\t")[0].split("/")[0].strip()
            if word:
                words.add(word)
    return words


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--nb", required=True, help="Bokmål word list (nb_NO.dic)")
    ap.add_argument("--nn", required=True, help="Nynorsk word list (nn_NO.dic)")
    ap.add_argument("--min-len", type=int, default=3)
    ap.add_argument("--limit", type=int, default=0, help="0 = unlimited")
    ap.add_argument(
        "--grep",
        default="",
        help="only print candidate pairs containing this substring",
    )
    args = ap.parse_args()

    nb = read_wordlist(args.nb)
    nn = read_wordlist(args.nn)
    print(f"nb_NO: {len(nb)} words; nn_NO: {len(nn)} words", flush=True)

    candidates = [
        w
        for w in nb - nn
        if len(w) >= args.min_len and w.isalpha() and w.islower()
    ]
    candidates.sort()
    if args.grep:
        candidates = [w for w in candidates if args.grep in w]
    if args.limit and len(candidates) > args.limit:
        candidates = candidates[: args.limit]
    for w in candidates:
        print(w)
    print(f"# {len(candidates)} candidate(s)", flush=True)


if __name__ == "__main__":
    main()
