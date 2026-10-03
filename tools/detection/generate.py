#!/usr/bin/env python3
"""Generate the language-detection fixtures from the vendored `data/` tree.

Two artefacts, both derived (never hand-edited) so they stay reproducible:

1. `crates/lt-core/tests/fixtures/detection_corpus.json` — authentic per-language
   sentences taken from the `<example>` elements in `data/*/rules/grammar.xml`.
   Used to calibrate and regression-test detection thresholds.

   Examples carrying a `correction` attribute show the *erroneous* form (that is
   what the rule matched), so they are kept in a separate bucket from the plain
   ones. Both are useful: real user text contains mistakes, but a detector
   should not be tuned only on them.

2. `data/nrd/detection/markers.txt` — the Nordum-only lexicon, computed as
   `nrd_core.dic` minus the Bokmal, Nynorsk, Danish and Swedish Hunspell
   word lists. Nordum is absent from every off-the-shelf language identifier, and
   a statistical model reports its Scandinavian source languages with high
   confidence, so it is decided by this list instead.

Run from the repository root:

    python3 tools/detection/generate.py

The Markdown/HTML/XHTML files under `data/*/` are ignored; only
`rules/grammar.xml` is read.
"""

from __future__ import annotations

import argparse
import html.entities
import json
import pathlib
import re
import sys
import xml.etree.ElementTree as ET
from collections import defaultdict

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
DATA_DIR = REPO_ROOT / "data"
CORPUS_OUT = REPO_ROOT / "crates" / "lt-core" / "tests" / "fixtures" / "detection_corpus.json"
MARKERS_OUT = DATA_DIR / "nrd" / "detection" / "markers.txt"

# XML only predefines these five; the grammar files also contain HTML entities
# such as `&nbsp;` inside example text, which makes ElementTree abort the whole
# document. Expanding them first keeps those languages in the corpus.
_XML_ENTITIES = {"amp", "lt", "gt", "quot", "apos"}
_ENTITY_RE = re.compile(r"&([A-Za-z][A-Za-z0-9_]{1,31});")
# The grammar files declare their own entities via an external subset, e.g.
#   <!ENTITY % entities SYSTEM "../../resource/es/entities.ent" >
# that path does not resolve inside our `data/` layout, so the file is looked up
# by name under the language's own directory instead.
_SYSTEM_ENTITY_RE = re.compile(r'<!ENTITY\s+%\s+\w+\s+SYSTEM\s+"([^"]+)"', re.IGNORECASE)
_ENTITY_DECL_RE = re.compile(r'<!ENTITY\s+([A-Za-z][A-Za-z0-9_.-]*)\s+"(.*?)"\s*>', re.DOTALL)

# Cap per language per bucket. The low-resource languages in this project have
# only a few dozen examples, so the cap mostly bounds the big ones while
# leaving the rare ones complete.
DEFAULT_CAP = 400

# Nordum's authoritative word list. The much larger `nrd.dic` beside it is the
# *derived* spell-checking dictionary (it folds in the source-language word
# lists), so using it as the Nordum side would leave nothing to subtract.
NORDUM_CORE = DATA_DIR / "nrd" / "hunspell" / "nrd_core.dic"

# Languages Nordum is contrasted against, and why each group is here.
#
# `no`, `nn`, `da`, `sv` — the three source languages plus Nynorsk. These are the
# real confusions: fastText answers `da`/`no`/`sv` for Nordum with high
# confidence, so a word shared with any of them identifies nothing.
#
# `de`, `gl` — not confusable, but they share Nordum's *international* vocabulary.
# A Nordum-only list still contains loanwords and endonyms that are equally
# valid elsewhere: `chocolate` is a perfectly good Nordum word that is also
# German, and `España` is the endonym Galician uses too. Without these two,
# both become markers and the lexicon claims languages it cannot distinguish.
#
# Deliberately *not* every vendored dictionary. Lithuanian independently has
# `jei` ("if"), so excluding against it would discard the single most
# distinctive Nordum word to guard against a confusion that cannot arise —
# nothing ever asks the detector to separate Nordum from Lithuanian.
NORDUM_CONTRAST = ("no", "nn", "da", "sv", "de", "gl")


def contrast_dictionaries() -> list[tuple[str, pathlib.Path]]:
    """The vendored Hunspell dictionaries Nordum must be distinguished from."""
    found = []
    for language in NORDUM_CONTRAST:
        paths = sorted((DATA_DIR / language / "hunspell").glob("*.dic"))
        if not paths:
            print(f"warning: no dictionary vendored for {language}", file=sys.stderr)
            continue
        found.extend((language, path) for path in paths)
    return found


def read_hunspell(path: pathlib.Path) -> set[str]:
    """Base forms from a Hunspell `.dic`.

    The first line is a word count; each later line is `word/FLAGS`, and some
    dictionaries carry alias or morphological lines starting with `-`, which are
    skipped.
    """
    words: set[str] = set()
    with path.open(encoding="utf-8", errors="ignore") as handle:
        for index, line in enumerate(handle):
            if index == 0 and line.strip().isdigit():
                continue
            word = line.split("/", 1)[0].strip()
            if not word or word.startswith("#") or word.startswith("-"):
                continue
            words.add(word.lower())
    return words


def derive_nordum_markers(corpus_words: set[str] | None = None) -> list[str]:
    """Nordum words usable as identification markers.

    Two filters, in order:

    1. Subtract every word in the contrast dictionaries above.
    2. Subtract any word that actually occurs in the example sentences of another
       shipped language. The dictionaries are the primary filter, but they cover
       only part of what we ship, so this catches short function words that
       collide (`dre` in Breton, for instance).
    """
    core = read_hunspell(NORDUM_CORE)
    contrast: set[str] = set()
    for _lang, path in contrast_dictionaries():
        contrast |= read_hunspell(path)
    markers = core - contrast
    if corpus_words:
        markers -= corpus_words
    return sorted(markers)


def inline_external_entities(raw: str, grammar: pathlib.Path) -> str:
    """Replace external DTD entity references with their declarations, inline.

    ElementTree parses an internal subset's own `<!ENTITY name "value">`
    declarations, but cannot follow `<!ENTITY % name SYSTEM "file">`. The
    referenced path also does not resolve inside our `data/` layout (the file
    lives at `data/<lang>/words/entities.ent`), so it is located by name and its
    declarations are substituted into the subset.

    The subset must be kept, not stripped: several grammars (`en`, `sv`, …)
    declare entities inline and rely on them.
    """
    declarations: list[str] = []

    def substitute(match: re.Match[str]) -> str:
        referenced = match.group(1)
        candidates = [(grammar.parent / referenced).resolve()]
        candidates.extend(sorted(grammar.parent.parent.glob("**/*.ent")))
        for candidate in candidates:
            if not candidate.is_file():
                continue
            body = candidate.read_text(encoding="utf-8", errors="ignore")
            for name, value in _ENTITY_DECL_RE.findall(body):
                # An entity value may itself contain `&` or `"`. Both have to be
                # escaped before substitution: an unescaped quote terminates the
                # declaration early and the document stops parsing.
                escaped = expand_entities(value).replace("&", "&amp;").replace('"', "&quot;")
                declarations.append(f'<!ENTITY {name} "{escaped}">')
            break
        else:
            print(
                f"warning: {grammar}: unresolved entity file {referenced}",
                file=sys.stderr,
            )
        return " ".join(declarations)

    return _SYSTEM_ENTITY_RE.sub(substitute, raw, count=1)


def expand_entities(text: str, local: dict[str, str] | None = None) -> str:
    """Replace HTML and locally declared entities with their characters."""

    def replace(match: re.Match[str]) -> str:
        name = match.group(1)
        if name in _XML_ENTITIES:
            return match.group(0)
        if local and name in local:
            return local[name]
        char = html.entities.html5.get(f"{name};")
        return char if char else match.group(0)

    return _ENTITY_RE.sub(replace, text)


def expand_html_entities(text: str) -> str:
    """Replace HTML named entities only (no grammar-local declarations)."""
    return expand_entities(text)


def extract_examples(path: pathlib.Path) -> tuple[list[str], list[str], bool]:
    """Return `(valid, with_errors, parsed)` for one grammar file."""
    raw = path.read_text(encoding="utf-8", errors="ignore")

    # Split the internal DTD subset off before expanding anything. HTML entities
    # appear in example *text*, but the entity *declarations* must survive
    # verbatim: expanding `&quot;` back to `"` inside a declaration would
    # terminate it early and the document would stop parsing.
    subset = ""
    start = raw.find("<!DOCTYPE")
    if start != -1:
        end = raw.find("]>", start)
        if end != -1:
            subset = raw[start : end + 2]
            raw = raw[end + 2 :]

    raw = expand_entities(raw)
    if subset:
        subset = inline_external_entities(subset, path)
        raw = subset + raw

    try:
        root = ET.fromstring(raw)
    except ET.ParseError as error:
        print(f"warning: {path}: {error}", file=sys.stderr)
        return [], [], False

    valid: list[str] = []
    erroneous: list[str] = []
    for element in root.iter("example"):
        text = " ".join("".join(element.itertext()).split())
        if not text:
            continue
        (erroneous if element.get("correction") else valid).append(text)
    return valid, erroneous, True


def collect_corpus(cap: int) -> tuple[dict[str, dict[str, list[str]]], dict[str, str]]:
    buckets: dict[str, dict[str, list[str]]] = defaultdict(lambda: {"valid": [], "with_errors": []})
    failures: dict[str, str] = {}
    for grammar in sorted(DATA_DIR.glob("*/rules/grammar.xml")):
        lang = grammar.parent.parent.name
        valid, erroneous, parsed = extract_examples(grammar)
        if not parsed:
            # Record why, so a grammar that stops parsing is visible in the
            # fixture rather than showing up as a language that mysteriously has
            # no data. Three grammars (`ca`, `es`, `pt`) declare very large
            # external entity values that ElementTree cannot round-trip through
            # substitution; they are sourced from the web instead.
            failures[lang] = "grammar.xml could not be parsed"
        elif not valid and not erroneous:
            failures[lang] = "grammar.xml ships no <example> text"
        # De-duplicate while preserving order: the same sentence appears as an
        # example under many rules.
        for key, items in (("valid", valid), ("with_errors", erroneous)):
            seen = set(buckets[lang][key])
            for sentence in items:
                if len(buckets[lang][key]) >= cap:
                    break
                if sentence not in seen:
                    seen.add(sentence)
                    buckets[lang][key].append(sentence)
    return dict(sorted(buckets.items())), failures


def collect_corpus_words(corpus: dict[str, dict[str, list[str]]]) -> set[str]:
    """Every word appearing in another language's example sentences.

    Presence is evidence that a word is not distinctive; absence is not
    evidence that it is, so this only ever removes markers.
    """
    words: set[str] = set()
    for lang, buckets in corpus.items():
        if lang == "nrd":
            continue
        for sentence in buckets["valid"] + buckets["with_errors"]:
            for token in re.findall(r"[^\W\d_]+", sentence, flags=re.UNICODE):
                words.add(token.lower())
    return words


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--cap",
        type=int,
        default=DEFAULT_CAP,
        help=f"maximum sentences per language per bucket (default {DEFAULT_CAP})",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the committed artefacts are out of date instead of writing them",
    )
    args = parser.parse_args()

    corpus, failures = collect_corpus(args.cap)
    total = sum(len(b["valid"]) + len(b["with_errors"]) for b in corpus.values())
    empty = [lang for lang, buckets in corpus.items() if not buckets["valid"] and not buckets["with_errors"]]
    corpus_json = json.dumps(corpus, ensure_ascii=False, indent=1, sort_keys=True) + "\n"

    markers = derive_nordum_markers(collect_corpus_words(corpus))
    markers_text = "\n".join(markers) + "\n"

    if args.check:
        stale = []
        if not CORPUS_OUT.exists() or CORPUS_OUT.read_text(encoding="utf-8") != corpus_json:
            stale.append(str(CORPUS_OUT.relative_to(REPO_ROOT)))
        if not MARKERS_OUT.exists() or MARKERS_OUT.read_text(encoding="utf-8") != markers_text:
            stale.append(str(MARKERS_OUT.relative_to(REPO_ROOT)))
        if stale:
            print("out of date: " + ", ".join(stale), file=sys.stderr)
            return 1
        print("fixtures up to date")
        return 0

    CORPUS_OUT.parent.mkdir(parents=True, exist_ok=True)
    CORPUS_OUT.write_text(corpus_json, encoding="utf-8")
    MARKERS_OUT.parent.mkdir(parents=True, exist_ok=True)
    MARKERS_OUT.write_text(markers_text, encoding="utf-8")

    print(f"corpus   {CORPUS_OUT.relative_to(REPO_ROOT)}")
    print(f"         {len(corpus)} languages, {total} sentences (cap {args.cap}/bucket)")
    if empty:
        # Not a failure: some modules simply ship no `<example>` text. Worth
        # saying out loud, because those languages cannot be calibrated and
        # will be relying on the statistical model alone.
        print(f"         no examples for: {', '.join(empty)}")
        for lang in empty:
            reason = failures.get(lang, "grammar.xml ships no <example> text")
            print(f"           {lang}: {reason}")
    print(f"markers  {MARKERS_OUT.relative_to(REPO_ROOT)}")
    print(f"         {len(markers)} Nordum-only words")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())