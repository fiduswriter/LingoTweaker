#!/usr/bin/env python3
"""Generate crates/lt/src/de/spelling_patterns.rs from the pinned Java source.

Reads `GermanSpellerRule.java` from the LanguageTool checkout and emits the
regex constants used by `ignorePotentiallyMisspelledWord` and its helpers as
`LazyLock<Regex>` statics (full-match anchored exactly where Java calls
`Matcher.matches()`), so the Rust port cannot drift from the Java patterns.

Usage:
  tools/gen-de-speller-patterns.py [--upstream <lt-checkout>] [--out <file>]
"""

import argparse
import os
import re
import sys

DEFAULT_UPSTREAM = os.environ.get(
    "LT_CHECKOUT",
    os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "languagetool"))
)
JAVA_REL = (
    "languagetool-language-modules/de/src/main/java/org/languagetool/rules/de/"
    "GermanSpellerRule.java"
)
DEFAULT_OUT = "crates/lt/src/de/spelling_patterns.rs"

# (Java constant name, full-match anchored)
WANTED = [
    ("adjSuffix", None),
    ("missingAdjPattern", True),
    ("SPECIAL_CASE", True),
    ("SPECIAL_CASE_WITH_S", True),
    ("COMPOUND_TYPOS", True),
    ("COMPOUND_END_TYPOS", True),
    ("FILE_UNDERLINE_PATTERN", False),
    ("MENTION_UNDERLINE_PATTERN", False),
    ("GENDER_STAR_PATTERN", False),
    ("INFIX_S_SUFFIXES", True),
    ("WECHSELINFIX", True),
    ("CONFUSED_PREFIXES", True),
    ("NEEDS_TO_BE_PLURAL", True),
    ("SUBNOMPLUFEM_EXCEPTIONS", True),
    ("CITIES_EXCEPTIONS", True),
    ("INVALID_COMP_PART_1", True),
    ("INVALID_COMP_PART_2", True),
    ("SUBINF_SINGULAR_OBJECT", True),
    ("ARBEIT_COMP", True),
    ("BACH_COMP", True),
    ("BAD_COMP", True),
    ("LINK_COMP", True),
    ("LINKS_COMP", True),
    ("PERSON_SUFFIXES", True),
    ("RECHT_COMP", True),
    ("RECHTS_COMP", True),
    ("VERBAND_COMP", True),
    ("VERBANDS_COMP", True),
    ("WIDER_COMP", True),
    ("WOCHENTAG_COMP", True),
    ("WECHSELNUMERUS", True),
    ("WELTEN_COMP", True),
    ("WOERTER_COMP", True),
    ("WOCHENTAGE", True),
    ("WOCHENTAGE_S", True),
    ("DIRECTION", True),
    ("CAMEL_CASE", True),
    ("ENDS_WITH_IBELKEIT_IBLICHKEIT", True),
]


def split_top_level(s: str, sep: str = ","):
    parts, depth, in_str, esc, cur = [], 0, False, False, []
    for c in s:
        if in_str:
            cur.append(c)
            if esc:
                esc = False
            elif c == "\\":
                esc = True
            elif c == '"':
                in_str = False
            continue
        if c == '"':
            in_str = True
            cur.append(c)
        elif c in "([":
            depth += 1
            cur.append(c)
        elif c in ")]":
            depth -= 1
            cur.append(c)
        elif c == sep and depth == 0:
            parts.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
    parts.append("".join(cur).strip())
    return parts


def java_string_literal(raw: str) -> str:
    """Evaluate the (limited) Java string concatenation used in the source."""
    parts = []
    for piece in split_top_level(raw, "+"):
        if not piece:
            continue
        m = re.fullmatch(r'"((?:[^"\\]|\\.)*)"', piece)
        if not m:
            raise ValueError(f"unsupported string piece: {piece!r}")
        body = m.group(1)
        out = []
        i = 0
        while i < len(body):
            c = body[i]
            if c == "\\" and i + 1 < len(body):
                n = body[i + 1]
                if n == "u" and re.fullmatch(r"[0-9a-fA-F]{4}", body[i + 2 : i + 6]):
                    out.append(chr(int(body[i + 2 : i + 6], 16)))
                    i += 6
                    continue
                out.append(
                    {"n": "\n", "t": "\t", "r": "\r", "\\": "\\"}.get(n, "\\" + n)
                )
                i += 2
                continue
            out.append(c)
            i += 1
        parts.append("".join(out))
    return "".join(parts)


def extract_constants(text: str):
    """Return {name: java expression string} for Pattern/String constants."""
    consts = {}
    for m in re.finditer(
        r"private static final (?:Pattern|String)\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*"
        r"(?:Pattern\.)?compile\((.*?)\);",
        text,
        re.S,
    ):
        consts[m.group(1)] = m.group(2)
    for m in re.finditer(
        r"private static final String\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?);",
        text,
        re.S,
    ):
        consts.setdefault(m.group(1), m.group(2))
    return consts


def resolve(expr: str, consts, depth: int = 0) -> str:
    """Resolve identifier references (e.g. `adjSuffix`) in a concat expr."""
    if depth > 5:
        raise ValueError("constant recursion too deep")
    parts = []
    for piece in split_top_level(expr, "+"):
        if not piece:
            continue
        if re.fullmatch(r'"(?:[^"\\]|\\.)*"', piece):
            parts.append(java_string_literal(piece))
        elif re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", piece):
            if piece not in consts:
                raise ValueError(f"unknown constant reference {piece!r}")
            parts.append(resolve(consts[piece], consts, depth + 1))
        else:
            raise ValueError(f"unsupported expression piece {piece!r}")
    return "".join(parts)


def rust_regex_literal(pattern: str) -> str:
    """Emit a Rust raw string literal for a regex."""
    if '"""' in pattern:
        raise ValueError("pattern contains a triple quote")
    return 'r#"' + pattern + '"#'


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--upstream", default=DEFAULT_UPSTREAM)
    ap.add_argument("--out", default=DEFAULT_OUT)
    args = ap.parse_args()

    java_path = os.path.join(args.upstream, JAVA_REL)
    with open(java_path, encoding="utf-8") as f:
        text = f.read()
    consts = extract_constants(text)

    out = [
        "// Generated by tools/gen-de-speller-patterns.py from the pinned",
        "// GermanSpellerRule.java — do not edit by hand.",
        "//",
        "// Full-match anchored (`^(?:…)$`) exactly where Java calls",
        "// `Matcher.matches()`; the rest keep Java's `find()` semantics.",
        "",
        "#![allow(dead_code)]",
        "",
        "use regex::Regex;",
        "use std::sync::LazyLock;",
        "",
    ]
    for name, anchored in WANTED:
        if name not in consts:
            raise SystemExit(f"constant {name} not found in {java_path}")
        if anchored is None:
            value = resolve(consts[name], consts)
            allow = "" if name.isupper() else "#[allow(non_upper_case_globals)]\n"
            out.append(
                f"{allow}pub(crate) static {name}: &str = "
                f"{rust_regex_literal(value)};"
            )
            continue
        pattern = resolve(consts[name], consts)
        if anchored:
            pattern = "^(?:" + pattern + ")$"
        allow = "" if name.isupper() else "#[allow(non_upper_case_globals)]\n"
        out.append(
            f"{allow}pub(crate) static {name}: LazyLock<Regex> = "
            f"LazyLock::new(|| Regex::new({rust_regex_literal(pattern)})"
            ".unwrap());"
        )
    out.append("")

    with open(args.out, "w", encoding="utf-8") as f:
        f.write("\n".join(out))
    print(f"wrote {args.out}: {len(WANTED)} patterns", file=sys.stderr)


if __name__ == "__main__":
    main()
