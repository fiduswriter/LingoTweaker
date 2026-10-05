#!/usr/bin/env python3
"""Normalise captured Java responses into the fixtures the Rust test compares to.

`capture-fixtures.py` records what the oracle said; this writes what the contract
is allowed to assert on. The two differ only in fields that cannot match by
construction, and every exclusion below says why — an unexplained difference
between two implementations is information, and normalisation that hid them would
throw that away.

    scripts/oracle/normalize-fixtures.py
"""

from __future__ import annotations

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "docs" / "parity" / "api-fixtures"

# `software.*` describes the *build*, not the contract. `version` and
# `buildDate` change on every commit; `commit` is the pin, recorded once in
# upstream.json and reported by `/v2/info` at capture time; `premium` and
# `premiumHint` are the open-source build's fields.
SOFTWARE_FIELDS = ("version", "buildDate", "commit", "premium", "premiumHint", "status")

# `detectedLanguage.source` is a free-form diagnostic that LanguageTool composes
# from several detectors (`"+fallback"`, `"ngram+commonwords"`,
# `"ngram+prefLang(forced: false)"`). No client branches on it —
# `RemoteLanguageTool` reads only `code` and `name` — and LingoTweaker answers
# with its own fixed vocabulary (`"ngram"`, `"lexicon"`, `null`). Comparing it
# would compare two implementations' internals, not the API.
SOURCE_FIELDS = ("source",)

# `detectedLanguage.confidence` is the detector's own probability: Java's
# n-gram detector and our fastText models produce different numbers for the same
# sentence by construction. Which language was detected is the contract; how
# sure each detector was is not. Replaced by a marker so its presence is still
# asserted.
CONFIDENCE_FIELDS = ("confidence",)


def normalise(value):
    if isinstance(value, dict):
        out = {}
        for key, item in value.items():
            if key == "software" and isinstance(item, dict):
                out[key] = {
                    k: v for k, v in item.items() if k not in SOFTWARE_FIELDS
                }
                continue
            if key in SOURCE_FIELDS and "detectedLanguage" in value:
                out[key] = "<implementation-defined>"
                continue
            if key in CONFIDENCE_FIELDS and "detectedLanguage" in value:
                out[key] = "<implementation-defined>"
                continue
            out[key] = normalise(item)
        return out
    if isinstance(value, list):
        return [normalise(item) for item in value]
    return value


def normalise_matches(response):
    """Sort `replacements` by value so suggestion *order* is not part of the
    contract — Java's order comes from rule internals, not from the API.

    A no-op for responses with no `matches`: `/v2/languages` is a bare array and
    `/v2/configinfo` a rule catalogue.
    """
    if not isinstance(response, dict):
        return response
    for match in response.get("matches", []) or []:
        replacements = match.get("replacements")
        if isinstance(replacements, list):
            match["replacements"] = sorted(
                replacements, key=lambda r: json.dumps(r, sort_keys=True)
            )
    return response


def main() -> int:
    raw_dir = FIXTURES / "responses"
    out_dir = FIXTURES / "expected"
    out_dir.mkdir(parents=True, exist_ok=True)
    count = 0
    for path in sorted(raw_dir.iterdir()):
        if path.suffix == ".txt":
            # Plain-text error bodies are the contract verbatim: Java's wording
            # is what a drop-in client sees, so it is compared as-is.
            (out_dir / path.name).write_bytes(path.read_bytes())
            count += 1
            continue
        response = json.loads(path.read_text(encoding="utf-8"))
        response = normalise_matches(normalise(response))
        (out_dir / path.name).write_text(
            json.dumps(response, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        count += 1
    print(f"normalised {count} fixture(s) -> {out_dir.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())