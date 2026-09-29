#!/usr/bin/env python3
"""Transform the vendored Dutch data into size-optimized pack trees.

Two modes:

* ``fold``    -- apply the ``added.txt``/``removed.txt`` manual readings at
  dictionary-build time instead of runtime: the ``removed`` triples are
  subtracted from the compiled ``dutch.dict``/``dutch_synth.dict`` and the
  ``removed*.txt`` files are dropped; ``added.txt`` is rewritten as
  ``added - removed`` (6 triples are in both lists; at runtime the removal
  tagger filters manual readings too, so those net to absent). Engine
  behavior is unchanged: ``ManualTagger``/``ManualSynthesizer`` skip missing
  files, surviving readings keep their relative order, and the runtime-added
  readings still come first.

* ``light``   -- everything ``fold`` does, plus dictionary pruning for the
  size-optimized ``nl-light`` pack: speller entries below the frequency floor
  are dropped unless they are known vocabulary (tagger surface forms and
  lemmas, runtime-added words, prohibited words); the tagger and synthesizer
  are then trimmed to the same keep-set.

Usage:

    python3 scripts/data/nl_pack_transform.py fold <data-dir> <out-tree> \
        [--cache <dir>]
    python3 scripts/data/nl_pack_transform.py light <data-dir> <out-tree> \
        [--cache <dir>] [--min-freq <char>]
    python3 scripts/data/nl_pack_transform.py roundtrip <data-dir> <work-dir>

``<data-dir>`` is the repository data tree; ``<out-tree>`` receives a full
copy of ``core/**`` and ``nl/**`` with the transformed files overlaid. The
heavy recompiles are cached in ``<dir>/<content-hash>/`` so pack rebuilds are
cheap; pass ``--cache`` to share the cache across builds (build-packs.sh uses
``target/nl-variant-cache``).

The ``roundtrip`` mode recompiles the unmodified decompiled dictionaries and
fails unless they are byte-identical to the vendored ones; it proves the
recompile path reproduces the Java-built originals exactly.

Dictionary (re)compilation uses ``tools/morfologik`` (pure-Python port of the
LanguageTool/Morfologik builders, byte-identical to the Java tooling).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
import tempfile
import unicodedata

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "tools", "morfologik"))

from morfologik import lt, tools  # noqa: E402
from morfologik.dictionary import Dictionary, DictionaryIterator  # noqa: E402
from morfologik.fsa import read_automaton  # noqa: E402
from morfologik.metadata import DictionaryMetadata  # noqa: E402

SCRIPT = os.path.abspath(__file__)

NL_FILES = {
    "tagger": "nl/dictionaries/dutch.dict",
    "tagger_info": "nl/dictionaries/dutch.info",
    "synth": "nl/dictionaries/dutch_synth.dict",
    "synth_info": "nl/dictionaries/dutch_synth.dict".replace(".dict", ".info"),
    "speller": "nl/spelling/nl_NL.dict",
    "added": "nl/words/added.txt",
    "removed": "nl/words/removed.txt",
    "added_custom": "nl/words/added_custom.txt",
    "removed_custom": "nl/words/removed_custom.txt",
    "prohibit": "nl/spelling/prohibit.txt",
}

# inputs that determine each mode's outputs (hashed for the cache key)
FOLD_INPUTS = ["tagger", "tagger_info", "synth", "synth_info", "added", "removed", "added_custom", "removed_custom"]
LIGHT_INPUTS = FOLD_INPUTS + ["speller", "prohibit"]


def log(msg: str) -> None:
    print(f"nl-transform: {msg}", file=sys.stderr)


def sha256_file(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def cache_key(mode: str, data_dir: str, min_freq: str) -> str:
    digest = hashlib.sha256()
    digest.update(mode.encode())
    digest.update(min_freq.encode())
    digest.update(open(SCRIPT, "rb").read())
    for name in LIGHT_INPUTS:
        digest.update(name.encode())
        digest.update(sha256_file(os.path.join(data_dir, NL_FILES[name])).encode())
    return digest.hexdigest()[:24]


# -- manual reading lists ----------------------------------------------------


def parse_manual_triples(*paths: str) -> set[tuple[str, str, str]]:
    """`(word, lemma, tag)` triples of `added.txt`/`removed.txt` files.

    Mirrors `ManualTagger`/`ManualSynthesizer`: tab-separated
    `fullform baseform postags`, `#` comments skipped.
    """
    triples: set[tuple[str, str, str]] = set()
    for path in paths:
        with open(path, encoding="utf-8") as handle:
            for line in handle:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                parts = line.split("\t")
                if len(parts) != 3:
                    continue
                triples.add((parts[0], parts[1], parts[2]))
    return triples


def load_tagger_triples(dict_path: str) -> list[tuple[str, str, str]]:
    """`(word, lemma, tag)` triples of the compiled tagger dictionary."""
    dictionary = Dictionary.read(dict_path)
    return [(inflected.decode("utf-8"), stem.decode("utf-8"), tag.decode("utf-8")) for stem, inflected, tag in DictionaryIterator(dictionary)]


def load_synth_triples(dict_path: str) -> list[tuple[str, str, str]]:
    """`(form, lemma, tag)` triples of the compiled synthesizer dictionary."""
    dictionary = Dictionary.read(dict_path)
    triples = []
    for stem, inflected, _tag in DictionaryIterator(dictionary):
        lemma, _, tag = inflected.decode("utf-8").partition("|")
        triples.append((stem.decode("utf-8"), lemma, tag))
    return triples


def load_speller_entries(dict_path: str) -> list[tuple[str, str]]:
    """`(word, freq_char)` of every speller FSA sequence."""
    with open(dict_path, "rb") as handle:
        fsa = read_automaton(handle.read())
    entries = []
    for sequence in fsa.iter_sequences():
        word, _, freq = sequence.rpartition(b"~")
        entries.append((word.decode("utf-8"), freq.decode("utf-8")))
    return entries


# -- dictionary (re)compilation ----------------------------------------------


def compile_tagger(triples: list[tuple[str, str, str]], info_path: str, out_dict: str, work_dir: str) -> None:
    """Recompile `dutch.dict` from `(word, lemma, tag)` triples."""
    tab = os.path.join(work_dir, "tagger_input.txt")
    with open(tab, "w", encoding="utf-8", newline="\n") as handle:
        for word, lemma, tag in triples:
            handle.write(f"{word}\t{lemma}\t{tag}\n")
    lt.pos_dictionary_builder(tab, info_path, out_dict)


def compile_synth(triples: list[tuple[str, str, str]], info_path: str, out_dict: str, work_dir: str) -> None:
    """Recompile `dutch_synth.dict` from `(form, lemma, tag)` triples.

    Feeds the same `form~lemma|tag` rows `SynthDictionaryBuilder` produces
    (after its reversal) straight into `dict_compile`, so the resulting
    automaton spells `lemma|tag` + encoded form exactly like the original.
    """
    rows = os.path.join(work_dir, "synth_input.txt")
    with open(rows, "w", encoding="utf-8", newline="\n") as handle:
        for form, lemma, tag in triples:
            handle.write(f"{form}~{lemma}|{tag}\n")
    shutil.copyfile(info_path, os.path.join(work_dir, "synth_input.info"))
    tools.dict_compile(rows, output_path=out_dict, overwrite=True, validate=False)


def compile_speller(entries: list[tuple[str, str]], out_dict: str, work_dir: str) -> None:
    sequences = os.path.join(work_dir, "speller_input.txt")
    with open(sequences, "w", encoding="utf-8", newline="\n") as handle:
        for word, freq in entries:
            handle.write(f"{word}~{freq}\n")
    tools.fsa_compile(sequences, out_dict, ignore_empty=True)


# -- the transform ------------------------------------------------------------


def fold_word(word: str) -> str:
    """Case/diacritic fold matching the speller's `convert-case` +
    `ignore-diacritics` matching closely enough for keep-set membership."""
    decomposed = unicodedata.normalize("NFD", word)
    stripped = "".join(c for c in decomposed if unicodedata.category(c) != "Mn")
    return unicodedata.normalize("NFC", stripped).casefold()


def build_artifacts(mode: str, data_dir: str, cache_dir: str, min_freq: str) -> str:
    """Build (or reuse) the transformed artifacts; return their directory."""
    key = cache_key(mode, data_dir, min_freq)
    artifacts = os.path.join(cache_dir, key)
    marker = os.path.join(artifacts, "MANIFEST.json")
    if os.path.isfile(marker):
        log(f"cache hit ({mode}, {key})")
        return artifacts
    log(f"cache miss ({mode}, {key}); decompiling + recompiling, this takes ~10-15 min")
    # build into a private dir and atomically publish as <key>/ so an
    # interrupted run can never leave a half-written cache entry behind
    staging = tempfile.mkdtemp(prefix=f"nl-transform-{key}-", dir=cache_dir)
    work = os.path.join(staging, "work")
    os.makedirs(work)
    artifacts = os.path.join(cache_dir, key)
    marker = os.path.join(artifacts, "MANIFEST.json")

    removed = parse_manual_triples(
        os.path.join(data_dir, NL_FILES["removed"]),
        os.path.join(data_dir, NL_FILES["removed_custom"]),
    )
    added = parse_manual_triples(os.path.join(data_dir, NL_FILES["added"]))

    tagger = load_tagger_triples(os.path.join(data_dir, NL_FILES["tagger"]))
    synth = load_synth_triples(os.path.join(data_dir, NL_FILES["synth"]))
    log(f"loaded {len(tagger)} tagger + {len(synth)} synth triples, {len(removed)} removals")

    tagger = [t for t in tagger if t not in removed]
    synth = [t for t in synth if t not in removed]
    log(f"fold: {len(tagger)} tagger + {len(synth)} synth triples remain")

    stats: dict[str, object] = {"mode": mode, "removed_triples": len(removed)}

    if mode == "light":
        min_rank = ord(min_freq)

        speller = load_speller_entries(os.path.join(data_dir, NL_FILES["speller"]))

        def signed_rank(freq: str) -> int:
            rank = ord(freq)
            return rank - 256 if rank >= 128 else rank

        freqset = {word for word, freq in speller if signed_rank(freq) >= min_rank}
        # Words the runtime or the compound acceptor needs regardless of
        # frequency: runtime-added readings, prohibited words (pruning one
        # would change its "prohibited" message into a generic misspelling)
        # and the compound-acceptor exception lists.
        rescue = {word for word, _lemma, _tag in added - removed}
        rescue |= {
            line.strip()
            for line in open(os.path.join(data_dir, NL_FILES["prohibit"]), encoding="utf-8")
            if line.strip() and not line.startswith("#")
        }
        acceptor_dir = os.path.join(data_dir, "nl/compound_acceptor")
        for name in os.listdir(acceptor_dir):
            with open(os.path.join(acceptor_dir, name), encoding="utf-8") as handle:
                rescue |= {line.split("#")[0].strip() for line in handle if line.split("#")[0].strip()}
        folded_rescue = {fold_word(w) for w in rescue}
        folded_freqset = {fold_word(w) for w in freqset}

        def keep(word: str) -> bool:
            return (
                word in freqset
                or word in rescue
                or fold_word(word) in folded_freqset
                or fold_word(word) in folded_rescue
            )

        speller_kept = [(w, f) for w, f in speller if keep(w)]
        tagger = [t for t in tagger if keep(t[0])]
        synth = [t for t in synth if keep(t[0])]
        log(f"light: speller {len(speller_kept)}/{len(speller)}, tagger {len(tagger)}, synth {len(synth)}")
        compile_speller(speller_kept, os.path.join(staging, "nl_NL.dict"), work)
        stats.update(
            speller_total=len(speller),
            speller_kept=len(speller_kept),
            tagger_kept=len(tagger),
            synth_kept=len(synth),
        )

    compile_tagger(tagger, os.path.join(data_dir, NL_FILES["tagger_info"]), os.path.join(staging, "dutch.dict"), work)
    compile_synth(synth, os.path.join(data_dir, NL_FILES["synth_info"]), os.path.join(staging, "dutch_synth.dict"), work)

    # added' = added - removed, preserving the original header comments
    added_kept = added - removed
    added_lines = []
    with open(os.path.join(data_dir, NL_FILES["added"]), encoding="utf-8") as handle:
        for line in handle:
            stripped = line.strip()
            if stripped.startswith("#"):
                added_lines.append(stripped)
    for word, lemma, tag in sorted(added_kept):
        added_lines.append(f"{word}\t{lemma}\t{tag}")
    with open(os.path.join(staging, "added.txt"), "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(added_lines) + "\n")
    stats["added_kept"] = len(added_kept)

    shutil.rmtree(work, ignore_errors=True)
    with open(os.path.join(staging, "MANIFEST.json"), "w", encoding="utf-8") as handle:
        json.dump(stats, handle, indent=2)
        handle.write("\n")
    try:
        os.rename(staging, artifacts)
    except OSError:
        # another concurrent build published first; keep the winner
        shutil.rmtree(staging, ignore_errors=True)
        if not os.path.isfile(marker):
            raise
    return artifacts


OVERLAY = {
    "dutch.dict": "nl/dictionaries/dutch.dict",
    "dutch_synth.dict": "nl/dictionaries/dutch_synth.dict",
    "nl_NL.dict": "nl/spelling/nl_NL.dict",
    "added.txt": "nl/words/added.txt",
}


def transform(mode: str, data_dir: str, out_tree: str, cache_dir: str, min_freq: str) -> None:
    data_dir = os.path.abspath(data_dir)
    out_tree = os.path.abspath(out_tree)
    os.makedirs(cache_dir, exist_ok=True)
    artifacts = build_artifacts(mode, data_dir, os.path.abspath(cache_dir), min_freq)

    if os.path.exists(out_tree):
        shutil.rmtree(out_tree)
    os.makedirs(out_tree)
    shutil.copytree(os.path.join(data_dir, "core"), os.path.join(out_tree, "core"))
    shutil.copytree(os.path.join(data_dir, "nl"), os.path.join(out_tree, "nl"))
    for name, rel in OVERLAY.items():
        src = os.path.join(artifacts, name)
        if os.path.isfile(src):
            shutil.copyfile(src, os.path.join(out_tree, rel))
    # removals are baked into the dictionaries now
    for rel in ("nl/words/removed.txt", "nl/words/removed_custom.txt"):
        path = os.path.join(out_tree, rel)
        if os.path.exists(path):
            os.unlink(path)
    log(f"wrote {out_tree} ({mode})")


def roundtrip(data_dir: str, work_dir: str) -> int:
    data_dir = os.path.abspath(data_dir)
    os.makedirs(work_dir, exist_ok=True)
    ok = True
    checks = [
        ("tagger", load_tagger_triples, lambda triples, d, w: compile_tagger(triples, os.path.join(data_dir, NL_FILES["tagger_info"]), os.path.join(w, "dutch.dict"), w), NL_FILES["tagger"]),
        ("synth", load_synth_triples, lambda triples, d, w: compile_synth(triples, os.path.join(data_dir, NL_FILES["synth_info"]), os.path.join(w, "dutch_synth.dict"), w), NL_FILES["synth"]),
    ]
    for name, loader, compiler, rel in checks:
        out = os.path.join(work_dir, os.path.basename(rel))
        compiler(loader(os.path.join(data_dir, rel)), data_dir, work_dir)
        same = sha256_file(out) == sha256_file(os.path.join(data_dir, rel))
        log(f"roundtrip {name}: {'byte-identical' if same else 'DIFFERS'}")
        ok = ok and same
    speller_out = os.path.join(work_dir, "nl_NL.dict")
    compile_speller(load_speller_entries(os.path.join(data_dir, NL_FILES["speller"])), speller_out, work_dir)
    same = sha256_file(speller_out) == sha256_file(os.path.join(data_dir, NL_FILES["speller"]))
    log(f"roundtrip speller: {'byte-identical' if same else 'DIFFERS'}")
    ok = ok and same
    return 0 if ok else 1


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["fold", "light", "roundtrip"])
    parser.add_argument("data_dir")
    parser.add_argument("out_tree")
    parser.add_argument("--cache", default=None, help="artifact cache directory")
    parser.add_argument("--min-freq", default="B", help="keep speller words at or above this frequency class (light)")
    args = parser.parse_args(argv)

    if args.command == "roundtrip":
        return roundtrip(args.data_dir, args.out_tree)
    cache = args.cache or os.path.join(args.data_dir, "..", "target", "nl-variant-cache")
    transform(args.command, args.data_dir, args.out_tree, cache, args.min_freq)
    return 0


if __name__ == "__main__":
    sys.exit(main())
