#!/usr/bin/env python3
"""Generate crates/lt/src/de/speller_data.rs from the pinned Java source.

Reads `GermanSpellerRule.java` from the LanguageTool checkout and emits the
`ADDITIONAL_SUGGESTIONS` map (1,488 entries, insertion order) and the
`PREVENT_SUGGESTION_PATTERNS` list as Rust statics.

Usage:
  tools/gen-de-speller-data.py [--upstream <lt-checkout>] [--out <file>]
"""

import argparse
import os
import re
import sys

DEFAULT_UPSTREAM = os.environ.get(
    "LT_CHECKOUT",
    os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "languagetool"))
)
JAVA_REL = "languagetool-language-modules/de/src/main/java/org/languagetool/rules/de/GermanSpellerRule.java"
DEFAULT_OUT = "crates/lt/src/de/speller_data.rs"


def rust_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def split_top_level(s: str, sep: str = ","):
    """Split on `sep` outside of string literals and parentheses."""
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
                out.append({"n": "\n", "t": "\t", "r": "\r"}.get(n, n))
                i += 2
                continue
            out.append(c)
            i += 1
        parts.append("".join(out))
    return "".join(parts)


def extract_static_block(lines):
    start = next(
        i for i, l in enumerate(lines) if "ADDITIONAL_SUGGESTIONS = new LinkedHashMap" in l
    )
    anchor = next(
        i for i, l in enumerate(lines) if "private static final GermanWordSplitter splitter" in l
    )
    entries = []
    i = start + 1
    while i < anchor:
        line = lines[i].strip()
        m = re.match(r"put(Repl)?\(", line)
        if m:
            buf = [line]
            depth = line.count("(") - line.count(")")
            while depth > 0:
                i += 1
                buf.append(lines[i].strip())
                depth += lines[i].count("(") - lines[i].count(")")
            entries.append(" ".join(buf))
        i += 1
    return entries


def parse_expr(s: str):
    """Parse one suggestion expression inside a lambda body."""
    s = s.strip()
    m = re.fullmatch(r'"(?:[^"\\]|\\.)*"', s)
    if m:
        return ("Lit", java_string_literal(s), None)
    m = re.fullmatch(r"w\.replaceFirst\((.*)\)", s)
    if m:
        args = split_top_level(m.group(1))
        return ("ReplaceFirst", java_string_literal(args[0]), java_string_literal(args[1]))
    m = re.fullmatch(r"StringUtils\.replaceOnce\(w\s*,\s*(.*?)\s*,\s*(.*?)\)", s)
    if m:
        return ("ReplaceOnce", java_string_literal(m.group(1)), java_string_literal(m.group(2)))
    m = re.fullmatch(r"uppercaseFirstChar\((.*)\)", s)
    if m:
        return ("UpperFirst", parse_expr(m.group(1)), None)
    raise ValueError(f"unsupported suggestion expression: {s!r}")


def parse_entry(entry: str):
    """Return (pattern, [expr, ...]) for one put/putRepl entry."""
    m = re.match(r"(putRepl|put)\(", entry)
    kind = m.group(1)
    body = entry[len(kind) + 1 : entry.rindex(")")].strip()
    args = split_top_level(body)
    pattern = java_string_literal(args[0])
    if kind == "putRepl":
        return pattern, [
            ("ReplaceFirst", java_string_literal(args[1]), java_string_literal(args[2]))
        ]
    val = args[1]
    if val.startswith("w ->") or val.startswith("w->"):
        val = val.split("->", 1)[1].strip()
    m = re.fullmatch(r"(?:singletonList|Arrays\.asList)\((.*)\)", val, re.S)
    if m:
        exprs = [parse_expr(a) for a in split_top_level(m.group(1)) if a]
    else:
        exprs = [parse_expr(val)]
    return pattern, exprs


def extract_prevent_patterns(lines):
    patterns = []
    for i, line in enumerate(lines):
        if "PREVENT_SUGGESTION_PATTERNS.add(" in line:
            buf = line.strip()
            depth = buf.count("(") - buf.count(")")
            while depth > 0:
                i += 1
                buf += " " + lines[i].strip()
                depth += lines[i].count("(") - lines[i].count(")")
            inner_start = buf.index("compile(") + len("compile(")
            depth2, in_str, esc, end = 0, False, False, None
            for j, c in enumerate(buf[inner_start:], start=inner_start):
                if in_str:
                    if esc:
                        esc = False
                    elif c == "\\":
                        esc = True
                    elif c == '"':
                        in_str = False
                    continue
                if c == '"':
                    in_str = True
                elif c == "(":
                    depth2 += 1
                elif c == ")":
                    if depth2 == 0:
                        end = j
                        break
                    depth2 -= 1
            patterns.append(java_string_literal(buf[inner_start:end]))
    return patterns


def rust_expr(expr) -> str:
    kind, a, b = expr
    if kind == "Lit":
        return f"SuggestExpr::Lit({rust_str(a)})"
    if kind == "UpperFirst":
        inner = rust_expr(a)
        return f"SuggestExpr::UpperFirst(&[{inner}])"
    if kind == "ReplaceFirst":
        return f"SuggestExpr::ReplaceFirst {{ pattern: {rust_str(a)}, replacement: {rust_str(b)} }}"
    if kind == "ReplaceOnce":
        return f"SuggestExpr::ReplaceOnce {{ from: {rust_str(a)}, to: {rust_str(b)} }}"
    raise ValueError(kind)


def extract_only_suggestions(lines):
    """`getOnlySuggestions` switch cases: exact word -> replacement values."""
    start = next(
        i
        for i, l in enumerate(lines)
        if "protected List<SuggestedReplacement> getOnlySuggestions" in l
    )
    end = next(
        i for i, l in enumerate(lines) if i > start and l.rstrip() == "  }"
    )
    body = "\n".join(lines[start : end + 1])
    entries = []
    for m in re.finditer(
        r'case ("(?:[^"\\]|\\.)*"):\s*\n?\s*return topMatch\((.*?)\);', body, re.S
    ):
        word = java_string_literal(m.group(1))
        args = split_top_level(m.group(2))
        values = [java_string_literal(args[0])]
        entries.append((word, values))
    return entries


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--upstream", default=DEFAULT_UPSTREAM)
    ap.add_argument("--out", default=DEFAULT_OUT)
    args = ap.parse_args()

    path = os.path.join(args.upstream, JAVA_REL)
    lines = open(path, encoding="utf-8").read().split("\n")

    entries = [parse_entry(e) for e in extract_static_block(lines)]
    prevents = extract_prevent_patterns(lines)
    only = extract_only_suggestions(lines)

    out = []
    out.append("//! Generated data tables from LT's `GermanSpellerRule`")
    out.append("//! (pinned commit 01d07e1f6165). Regenerate with")
    out.append("//! `tools/gen-de-speller-data.py [--upstream <lt-checkout>]`.")
    out.append("")
    out.append("/// One suggestion expression of `GermanSpellerRule`'s curated")
    out.append("/// `ADDITIONAL_SUGGESTIONS` map (Java lambdas over the word).")
    out.append("pub(crate) enum SuggestExpr {")
    out.append("    /// A literal replacement.")
    out.append("    Lit(&'static str),")
    out.append("    /// `w.replaceFirst(pattern, replacement)` (Java regex, `$1` groups).")
    out.append("    ReplaceFirst {")
    out.append("        pattern: &'static str,")
    out.append("        replacement: &'static str,")
    out.append("    },")
    out.append("    /// `StringUtils.replaceOnce(w, from, to)` (literal substring).")
    out.append("    ReplaceOnce { from: &'static str, to: &'static str },")
    out.append("    /// `uppercaseFirstChar(<one inner expression>)`.")
    out.append("    UpperFirst(&'static [SuggestExpr]),")
    out.append("}")
    out.append("")
    out.append("/// `GermanSpellerRule.ADDITIONAL_SUGGESTIONS` in insertion order")
    out.append("/// (the first matching pattern wins; Java `StringMatcher.regexp` =")
    out.append("/// full match, case-sensitive).")
    out.append("pub(crate) static ADDITIONAL_SUGGESTIONS: &[(&str, &[SuggestExpr])] = &[")
    for pattern, exprs in entries:
        items = ", ".join(rust_expr(e) for e in exprs)
        out.append(f"    ({rust_str(pattern)}, &[{items}]),")
    out.append("];")
    out.append("")
    out.append("/// `GermanSpellerRule.getOnlySuggestions` switch cases (replace all")
    out.append("/// other suggestions; descriptions dropped, they do not affect the")
    out.append("/// suggestion values).")
    out.append("pub(crate) static ONLY_SUGGESTIONS: &[(&str, &[&str])] = &[")
    for word, values in only:
        items = ", ".join(rust_str(v) for v in values)
        out.append(f"    ({rust_str(word)}, &[{items}]),")
    out.append("];")
    out.append("")
    out.append("/// `GermanSpellerRule.PREVENT_SUGGESTION_PATTERNS` (Java regex,")
    out.append("/// full-match semantics).")
    out.append("pub(crate) static PREVENT_SUGGESTION_PATTERNS: &[&str] = &[")
    for p in prevents:
        out.append(f"    {rust_str(p)},")
    out.append("];")
    out.append("")

    with open(args.out, "w", encoding="utf-8") as f:
        f.write("\n".join(out))
    print(
        f"wrote {args.out}: {len(entries)} suggestions, {len(only)} only-suggestions, {len(prevents)} prevent patterns",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
