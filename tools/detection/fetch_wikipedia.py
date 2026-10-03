#!/usr/bin/env python3
"""Fetch authentic per-language sentences from Wikipedia for detection calibration.

`tools/detection/generate.py` derives the detection fixture from the `<example>`
elements in `data/*/rules/grammar.xml`. Five of the 38 shipped languages yield
nothing that way: `ca`, `es` and `pt` declare external DTD entities too large for
ElementTree to round-trip, and `sr`/`uk` ship no `<example>` elements at all. They
therefore cannot be calibrated, and detection for them rests entirely on the
statistical model.

This tool closes that gap from the source rather than from hand-picked text: it
pulls random articles from each language's Wikipedia through the MediaWiki API,
keeps the plain prose, and caches the usable sentences in
`data/detection/wikipedia/<lang>.txt`, one sentence per line. `generate.py` reads
that cache only for languages whose grammar file yields nothing, so the other 33
languages keep their existing data and their behaviour is unchanged.

Wikipedia text is CC BY-SA; the owner approved vendoring it (see
`data/detection/wikipedia/README.md`). Provenance — the exact API URL, the article
titles behind every sentence and the retrieval date — is written per language to
`data/detection/wikipedia/<lang>.json` and recorded in `data/manifest.json`.

Run from the repository root:

    python3 tools/detection/fetch_wikipedia.py                # the five languages
    python3 tools/detection/fetch_wikipedia.py --lang gl      # any other language
    python3 tools/detection/fetch_wikipedia.py --count 200     # smaller cache
    python3 tools/detection/fetch_wikipedia.py --dry-run       # no files written

This tool needs the network; `generate.py` does not, and `--check` there stays
hermetic because the cache it reads is committed.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import pathlib
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
DATA_DIR = REPO_ROOT / "data"
CACHE_DIR = DATA_DIR / "detection" / "wikipedia"
MANIFEST_JSON = DATA_DIR / "manifest.json"

# The languages `generate.py` cannot fill from a grammar file. `ca`, `es` and `pt`
# declare external DTD entities ElementTree cannot substitute; `sr` and `uk` ship
# no `<example>` elements. Their Wikipedia edition is the closest thing to the
# upstream `<example>` text the other languages contribute: real prose, written by
# native speakers, one sentence per unit.
DEFAULT_LANGUAGES = ("ca", "es", "pt", "sr", "uk")

# Wikimedia asks for a descriptive User-Agent that identifies the tool and gives a
# way to get in touch. An anonymous or generic agent is a fair-use problem, not a
# technical one.
USER_AGENT = (
    "LingoTweaker detection fixtures (https://github.com/fiduswriter/LingoTweaker); "
    "MediaWiki extracts for crates/lt-core/tests/fixtures/detection_corpus.json"
)

# `generate.py`'s per-bucket cap. The cache is fetched to the same size so the
# fixture lands on the cap rather than under it.
DEFAULT_COUNT = 400

# Articles are long but not endless. Capping per article keeps 400 sentences from
# being 400 sentences about one village, which would calibrate the detector
# against a single topic instead of the language.
DEFAULT_PER_ARTICLE = 25

# Upper bound on API round-trips per language, so a wiki that keeps returning
# unusable pages cannot spin forever.
DEFAULT_MAX_REQUESTS = 120

# Same idea, one level up: a wiki with many stubs (Serbian especially) yields
# sentences that differ only in their numbers, and three of those already tell
# the detector what it needs to know.
DEFAULT_MAX_PER_OPENING = 3

# Sentence length window. Under 40 characters there is too little signal for the
# model to work with (and the length gate abstains anyway); over 300 and it is
# usually several sentences that the splitter failed to separate, or a table that
# came through as text.
MIN_CHARS = 40
MAX_CHARS = 300

REQUEST_TIMEOUT = 30
REQUEST_RETRIES = 3
REQUEST_PAUSE = 0.2  # seconds between requests; small, but polite.

# A sentence ends at a terminator followed by whitespace and the start of a new
# sentence. This is the whole rule: `finditer` walks the terminators, and the
# break is only taken when the next character is a capital letter or an opening
# quote (`re` has no portable "is uppercase" test for all five scripts, so the
# decision is made in `split_sentences`). Anything else — a lowercase
# continuation, a number, an abbreviation — stays inside the sentence. It is
# deliberately naive: the length and junk filters downstream remove the fragments
# it leaves behind, and a proper segmenter would be a dependency with its own
# licence and maintenance for no measurable gain here.
_TERMINATOR_RE = re.compile(r"[.!?…]+[\"'»’”“)\]]*\s+")

# Words that end in a period without ending a sentence. Matched case-insensitively
# against the last token before the terminator.
ABBREVIATIONS = frozenset(
    """
    cap caps captes ca cb cf cm cmr dr dra ed eds etc fig figs kg km mg mm núm
    num pàg pàgs pág pags pp pt s sr st t tel vol vols vs aprox aprox.
    """.split()
)

# Markup and residue that survives `explaintext` in some articles. `\displaystyle`
# and friends come from math blocks; `{|`/`|}`/`{{` from tables and infoboxes; the
# zero-width space is inserted around bidi runs in the right-to-left wikis.
JUNK_SUBSTRINGS = (
    "\\",
    "{|",
    "|}",
    "{{",
    "}}",
    "<ref",
    "&nbsp;",
    "&amp;",
    "&lt;",
    "&gt;",
    "&quot;",
    "http://",
    "https://",
    "\u200b",  # zero-width space, inserted around bidi runs
    # Site boilerplate that reads like prose but is none: the archive footers the
    # list articles end with, and the "start page" links. Language-specific and
    # short; a legitimate sentence mentioning the Wayback Machine would be a
    # curiosity either way.
    "Wayback Machine",
    "Архивирано",
    "Архівовано",
    "Arquivado",
    "Arquivada",
    "Arquivado/descarregue",
    "Archivado",
    "Archivada",
    "Arxivada",
    "Pàgina d'inici",
    "Página de inicio",
)

# A sentence may open with a quotation mark or a bracket, never with these:
# `=`/`*`/`#` are section and list markers, `-`/`:`/`;` are table and metadata
# residue. They show up as whole lines, which is where most of the junk comes from.
BAD_FIRST_CHARS = "=*#:;|-•\u2013\u2014"


class FetchError(RuntimeError):
    """A language's articles could not be fetched."""


def api_params() -> dict[str, str]:
    """The query parameters of one extracts request.

    `exlimit=1` is not optional. TextExtracts caps a request for *whole* article
    extracts at one page — a larger `exlimit` is silently lowered to 1 with a
    `warnings` block, so `grnlimit` above 1 would only waste the pages it fetched.
    One article per request, paged forward with `grncontinue`, is the shape that
    actually yields full article text.

    `grnnamespace=0` restricts the random generator to main-namespace articles;
    without it the draws include category, template and portal pages, which are
    exactly the pages whose plain text is a list of links.
    """
    return {
        "action": "query",
        "format": "json",
        "formatversion": "2",
        "prop": "extracts",
        "explaintext": "1",
        "redirects": "1",
        "exlimit": "1",
        "generator": "random",
        "grnnamespace": "0",
        "grnlimit": "1",
    }


def build_url(lang: str, grncontinue: str | None = None) -> str:
    """The exact URL of one request, which is what the manifest records."""
    params = api_params()
    if grncontinue:
        params["grncontinue"] = grncontinue
    return f"https://{lang}.wikipedia.org/w/api.php?" + urllib.parse.urlencode(params)


def fetch_json(url: str) -> dict:
    """GET one URL and parse the JSON body, with a bounded retry.

    A wiki that is briefly overloaded answers 429 with a `Retry-After`, and a
    flaky network answers nothing at all; both are worth another attempt. A 404
    (unknown wiki) or any other 4xx is not, so it fails immediately.
    """
    last: Exception | None = None
    for attempt in range(1, REQUEST_RETRIES + 1):
        request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        try:
            with urllib.request.urlopen(request, timeout=REQUEST_TIMEOUT) as response:
                return json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as error:
            last = error
            if error.code != 429:
                raise FetchError(f"HTTP {error.code} {error.reason}") from error
            pause = float(error.headers.get("Retry-After", 2**attempt))
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError, OSError) as error:
            last = error
            pause = float(2**attempt)
        if attempt < REQUEST_RETRIES:
            time.sleep(min(pause, 30.0))
    raise FetchError(f"{last} (after {REQUEST_RETRIES} attempts)")


def is_abbreviation(tail: str) -> bool:
    """Whether the text before a terminator is an abbreviation, not a sentence end.

    Two cases, the only two that matter in these five languages. A lower-case
    abbreviation word from the list above (`pàg.`, `fig.`, `núm.`), and an
    initialism such as `U.S.` or `L'U.S.`: the terminator run swallows the final
    period, so what is left is a short capitalised token that still carries an
    internal one. A bare number is not an abbreviation — that is what keeps
    `... el 1820. Fou un pintor` splitting where it should — and neither is a
    short ordinary word, which is why the period is required.
    """
    stripped = tail.strip()
    if not stripped:
        return True
    word = re.sub(r"[^\w]", "", stripped).lower()
    if word in ABBREVIATIONS:
        return True
    token = stripped.split()[-1].strip("\"'“(«„ ")
    letters = re.sub(r"[^A-Za-z]", "", token)
    if not letters:
        return False
    return len(letters) <= 3 and letters[0].isupper() and "." in token


def split_sentences(line: str) -> list[str]:
    """Split one line of plain text into candidate sentences.

    See `_TERMINATOR_RE`: a terminator, any closing quotes or brackets, then
    whitespace, and the next character has to look like the start of a sentence
    (a capital letter or an opening quote). Abbreviations do not break.
    """
    sentences: list[str] = []
    start = 0
    for match in _TERMINATOR_RE.finditer(line):
        following = line[match.end() : match.end() + 1]
        if following and not (following.isupper() or following in "\"'«“„‘("):
            continue
        if is_abbreviation(line[start : match.start()]):
            continue
        sentences.append(line[start : match.end()].strip())
        start = match.end()
    tail = line[start:].strip()
    if tail:
        sentences.append(tail)
    return sentences


def is_junk(sentence: str) -> bool:
    """Whether a candidate sentence is unusable as calibration data.

    Three groups, in the order they are cheap to test: markup/residue that the
    extractor left in, length outside the window, and text that is not mostly
    prose — the last one is what catches table dumps, math blocks and lists of
    coordinates, which pass every other test.
    """
    if not sentence:
        return True
    if not (MIN_CHARS <= len(sentence) <= MAX_CHARS):
        return True
    if sentence[0] in BAD_FIRST_CHARS:
        return True
    if any(marker in sentence for marker in JUNK_SUBSTRINGS):
        return True
    # Table cells and link markup that survived as plain text.
    if any(char in sentence for char in "|{}[]<>"):
        return True
    letters = sum(char.isalpha() for char in sentence)
    if letters < 10:
        return True
    if letters / len(sentence) < 0.55:
        return True
    digits = sum(char.isdigit() for char in sentence)
    if digits > len(sentence) * 0.2:
        return True
    if sentence.count("(") != sentence.count(")"):
        return True
    return False


def sentences_from_extract(extract: str) -> list[str]:
    """Usable sentences from one article's plain-text extract.

    Lines are split first: `explaintext` keeps paragraph and list-item line
    breaks, and running the sentence splitter across them would glue a list item
    onto the sentence before it.
    """
    kept: list[str] = []
    for raw_line in extract.splitlines():
        line = " ".join(raw_line.split())
        if not line:
            continue
        for sentence in split_sentences(line):
            if not is_junk(sentence):
                kept.append(sentence)
    return kept


def opening_key(sentence: str) -> str:
    """The first few words of a sentence, used to spot formulaic repetition."""
    return " ".join(sentence.split()[:5]).lower()


def fetch_language(
    lang: str,
    count: int,
    per_article: int,
    max_per_opening: int,
    max_requests: int,
) -> tuple[list[str], list[dict]]:
    """Collect up to `count` distinct sentences of one language's Wikipedia.

    Two repetition caps keep the corpus representative. `per_article` bounds how
    much one article may contribute; `max_per_opening` bounds how many sentences
    may share their first five words. The second one matters because a random
    draw of a wiki with many stubs returns mostly census boilerplate — in
    Serbian, eighteen near-copies of one census sentence (`naselje se nalazi` /
    `prema podacima`) would otherwise be a tenth of the calibration set for that
    language, and the detector would be measured on one phrasing rather than on
    Serbian.

    Returns the sentences in the order they were drawn and a per-article record
    of what they came from, which is what the provenance file needs.
    """
    sentences: list[str] = []
    seen: set[str] = set()
    openings: dict[str, int] = {}
    articles: list[dict] = []
    grncontinue: str | None = None

    for request in range(1, max_requests + 1):
        if len(sentences) >= count:
            break
        url = build_url(lang, grncontinue)
        document = fetch_json(url)
        pages = document.get("query", {}).get("pages") or []
        if not pages:
            break
        for page in pages:
            extract = page.get("extract") or ""
            if not extract.strip():
                continue
            fresh: list[str] = []
            for sentence in sentences_from_extract(extract):
                if len(fresh) >= per_article or len(sentences) + len(fresh) >= count:
                    break
                if sentence in seen:
                    continue
                key = opening_key(sentence)
                if openings.get(key, 0) >= max_per_opening:
                    continue
                seen.add(sentence)
                openings[key] = openings.get(key, 0) + 1
                fresh.append(sentence)
            if not fresh:
                continue
            sentences.extend(fresh)
            articles.append(
                {
                    "title": page.get("title", ""),
                    "sentences": len(fresh),
                    "url": f"https://{lang}.wikipedia.org/wiki/"
                    + urllib.parse.quote(page.get("title", "").replace(" ", "_")),
                }
            )
            if len(sentences) >= count:
                break
        continuation = document.get("continue", {}).get("grncontinue")
        if not continuation or continuation == grncontinue:
            break
        grncontinue = continuation
        time.sleep(REQUEST_PAUSE)

    return sentences[:count], articles


def readme_text(languages: list[str], retrieved: str) -> str:
    """Attribution and regeneration instructions for the vendored cache."""
    listed = "\n".join(f"- [`{lang}.txt`]({lang}.txt) — `{lang}.wikipedia.org`" for lang in languages)
    return f"""# Wikipedia detection corpus

Authentic sentences used to calibrate the language-detection feature for the
languages whose LanguageTool grammar file yields no `<example>` text. They are
the fallback source for `tools/detection/generate.py`, which prefers
`data/<lang>/rules/grammar.xml` and reads this cache only when that file produces
nothing.

## Licence

Wikipedia article text is licensed **CC BY-SA 3.0** ([Wikimedia Terms of
Use](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use)); the owner
approved vendoring it for this project. This file and the per-file record in
`data/manifest.json` (`license`, `license_verified`, `license_source`, exact API
`url`, `sha256`) are that attribution.

## Files

{listed}

One sentence per line, de-duplicated, 40–300 characters. `<lang>.json` beside each
file records the exact API request, the articles the sentences came from and when
they were fetched. Retrieved {retrieved}.

## Regenerating

```sh
python3 tools/detection/fetch_wikipedia.py            # these languages
python3 tools/detection/generate.py                   # regenerate the fixtures
python3 tools/detection/generate.py --check           # committed artefacts current?
```

The fetch needs network access; `generate.py --check` does not, because this
cache is committed. Re-fetching changes the sentences and therefore the fixture
and the manifest's sha256 values.
"""


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def manifest_source(url: str, retrieved: str, count: int, articles: int) -> dict:
    """The `data/manifest.json` `source` block for one cached file.

    `kind` is `vendor`: the sentences are third-party data pulled from the web,
    not hand-authored and not generated from upstream. `license_verified` is true
    because the licence is stated at `license_source` (the Wikimedia Terms of
    Use) and the owner signed off on it.
    """
    return {
        "kind": "vendor",
        "license": "CC-BY-SA-3.0 (Wikipedia article text) - owner approved",
        "license_verified": True,
        "license_source": "https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use",
        "url": url,
        "generator": "tools/detection/fetch_wikipedia.py (MediaWiki API random-article "
        "extracts, namespace 0, explaintext)",
        "note": f"{count} sentences from {articles} random articles, retrieved {retrieved}; "
        "per-article provenance in data/detection/wikipedia/",
        "retrieved": retrieved,
    }


def update_manifest(entries: dict[str, dict]) -> None:
    """Upsert `entries` (path -> source block) into `data/manifest.json`.

    The manifest is a flat per-file provenance list; this only adds the paths it
    is given, re-sorts, and refreshes `file_count`, which is the same contract
    `tools/lt-sync/lt_sync.py add-local` provides. A missing manifest is treated as
    an empty one, as it is there.
    """
    if MANIFEST_JSON.is_file():
        manifest = json.loads(MANIFEST_JSON.read_text(encoding="utf-8"))
    else:
        manifest = {"upstream_commit": None, "file_count": 0, "files": []}
    files = {entry["path"]: entry for entry in manifest.get("files", [])}
    for rel_path, source in entries.items():
        absolute = DATA_DIR / rel_path
        files[rel_path] = {
            "path": rel_path,
            "sha256": sha256_file(absolute),
            "size": absolute.stat().st_size,
            "source": source,
        }
    ordered = [files[path] for path in sorted(files)]
    generated_by = manifest.get("generated_by", "")
    for tool in ("tools/lt-sync/lt_sync.py add-local", "tools/detection/fetch_wikipedia.py"):
        if tool not in generated_by:
            generated_by = f"{generated_by}; {tool}" if generated_by else tool
    manifest["files"] = ordered
    manifest["file_count"] = len(ordered)
    manifest["generated_by"] = generated_by
    MANIFEST_JSON.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Fetch Wikipedia sentences for detection calibration")
    parser.add_argument(
        "--lang",
        action="append",
        metavar="CODE",
        help=f"Wikipedia language code; repeatable (default: {' '.join(DEFAULT_LANGUAGES)})",
    )
    parser.add_argument(
        "--count",
        type=int,
        default=DEFAULT_COUNT,
        help=f"sentences per language (default {DEFAULT_COUNT}, generate.py's cap)",
    )
    parser.add_argument(
        "--per-article",
        type=int,
        default=DEFAULT_PER_ARTICLE,
        help=f"maximum sentences taken from one article (default {DEFAULT_PER_ARTICLE})",
    )
    parser.add_argument(
        "--max-per-opening",
        type=int,
        default=DEFAULT_MAX_PER_OPENING,
        help="maximum sentences sharing their first five words (default "
        f"{DEFAULT_MAX_PER_OPENING}); keeps stub boilerplate from dominating a wiki",
    )
    parser.add_argument(
        "--max-requests",
        type=int,
        default=DEFAULT_MAX_REQUESTS,
        help=f"API requests per language before giving up (default {DEFAULT_MAX_REQUESTS})",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="report what would be fetched without writing anything",
    )
    args = parser.parse_args()

    languages = [code.strip().lower() for code in (args.lang or DEFAULT_LANGUAGES)]
    if args.dry_run:
        print(f"dry run: {len(languages)} language(s), {args.count} sentences each")
        for lang in languages:
            print(f"  {lang}: {build_url(lang)}")
        return 0

    retrieved = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    CACHE_DIR.mkdir(parents=True, exist_ok=True)

    fetched: dict[str, list[str]] = {}
    manifest_entries: dict[str, dict] = {}
    failures: list[str] = []

    for lang in languages:
        url = build_url(lang)
        print(f"fetching {lang} from {lang}.wikipedia.org …", file=sys.stderr, flush=True)
        try:
            sentences, articles = fetch_language(
                lang, args.count, args.per_article, args.max_per_opening, args.max_requests
            )
        except FetchError as error:
            # One language failing is a warning, not a crash: the other four
            # languages are independent, and a partial cache still helps.
            print(f"warning: {lang}: {error}", file=sys.stderr)
            failures.append(lang)
            continue
        if not sentences:
            print(f"warning: {lang}: no usable sentences in the articles drawn", file=sys.stderr)
            failures.append(lang)
            continue

        cache_path = CACHE_DIR / f"{lang}.txt"
        cache_path.write_text("\n".join(sentences) + "\n", encoding="utf-8")
        provenance_path = CACHE_DIR / f"{lang}.json"
        provenance_path.write_text(
            json.dumps(
                {
                    "language": lang,
                    "wikipedia": f"https://{lang}.wikipedia.org/",
                    "api_url": url,
                    "api_params": api_params(),
                    "paging": "generator=random paged forward with continue.grncontinue",
                    "retrieved": retrieved,
                    "sentence_count": len(sentences),
                    "filters": {
                        "min_chars": MIN_CHARS,
                        "max_chars": MAX_CHARS,
                        "max_sentences_per_article": args.per_article,
                        "max_sentences_per_opening": args.max_per_opening,
                    },
                    "generator": "tools/detection/fetch_wikipedia.py",
                    "articles": articles,
                },
                indent=1,
                ensure_ascii=False,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        fetched[lang] = sentences
        manifest_entries[f"detection/wikipedia/{lang}.txt"] = manifest_source(
            url, retrieved, len(sentences), len(articles)
        )
        manifest_entries[f"detection/wikipedia/{lang}.json"] = manifest_source(
            url, retrieved, len(sentences), len(articles)
        )
        print(f"  {lang}: {len(sentences)} sentences from {len(articles)} articles", file=sys.stderr)

    if not fetched:
        print(
            "error: no language could be fetched — either the MediaWiki API is "
            "unreachable (offline, DNS, or a proxy in the way) or none of the "
            "requested codes is a Wikipedia edition; nothing was written",
            file=sys.stderr,
        )
        return 2

    readme_path = CACHE_DIR / "README.md"
    readme_path.write_text(readme_text(sorted(fetched), retrieved), encoding="utf-8")
    manifest_entries["detection/wikipedia/README.md"] = {
        "kind": "hand-authored",
        "license": "LGPL-2.1-or-later",
        "license_source": "LingoTweaker project LICENSE",
        "note": "Provenance and regeneration instructions for the vendored Wikipedia "
        "sentence cache",
    }
    update_manifest(manifest_entries)

    total = sum(len(sentences) for sentences in fetched.values())
    print(f"wrote {total} sentences for {len(fetched)} language(s) to {CACHE_DIR}")
    print("manifest: data/manifest.json updated")
    if failures:
        print(
            f"warning: {len(failures)} language(s) produced nothing: {', '.join(failures)}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
