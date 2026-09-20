#!/usr/bin/env python3
"""Compare CheckDump.java TSV with lt-cli check --lines TSV.

Prints per-line differences: matches only in Java, only in Rust, and
field-level diffs (rule/sub/from/to/message/suggestions) for shared matches.

Usage: compare-checks.py <java.tsv> <rust.tsv> [show]
       [--fail-on-diff]
       [--expect-field-diffs=RULE=N] [--expect-only-java=RULE=N]
       [--expect-only-rust=RULE=N]

`show` limits the printed example diffs (default 60). By default the exit
status is always 0 (triage usage); with `--fail-on-diff` the script exits 1
on any only-Java / only-Rust / missing-line / unexpected field diff. Rules in
`--expect-field-diffs=RULE=N`, `--expect-only-java=RULE=N` and
`--expect-only-rust=RULE=N` (all repeatable) must have exactly N matching
diffs; those are excluded from the reported `field diffs` / `only Java` /
`only Rust` counts (documented divergences, e.g. the English
ADVERB_VERB_ADVERB_REPETITION case, the French
FRENCH_WORD_REPEAT_RULE/SUJET_AUXILIAIRE false positives, or the Spanish
hand-authored AGREEMENT_DEMONSTRATIVE_VERB rules). Both counts are
validated exactly: a rule with fewer or more diffs than expected fails the
gate.
"""
import sys
from collections import defaultdict


def load(path):
    lines = {}
    matches = defaultdict(list)
    cur = None
    for raw in open(path):
        parts = raw.rstrip('\n').split('\t')
        if parts[0] == 'L':
            cur = int(parts[1])
            lines[cur] = parts[2] if len(parts) > 2 else ''
        elif parts[0] == 'M':
            rec = {
                'line': int(parts[1]),
                'rule': parts[2],
                'sub': parts[3],
                'from': int(parts[4]),
                'to': int(parts[5]),
                'type': parts[6],
                'message': parts[7],
                'suggestions': parts[8] if len(parts) > 8 else '',
            }
            matches[int(parts[1])].append(rec)
    return lines, matches


def key(rec):
    return (rec['rule'], rec['sub'], rec['from'], rec['to'])


def main(java_path, rust_path, show=60, fail_on_diff=False, expect=None,
         expect_only_java=None, expect_only_rust=None):
    expect = expect or {}
    expect_only_java = expect_only_java or {}
    expect_only_rust = expect_only_rust or {}
    jlines, jm = load(java_path)
    rlines, rm = load(rust_path)
    only_java = 0
    only_rust = 0
    field_diffs = 0
    only_java_by_rule = defaultdict(int)
    only_rust_by_rule = defaultdict(int)
    field_diffs_by_rule = defaultdict(int)
    missing_lines = 0
    examples = []
    for lineno in sorted(set(jlines) | set(rlines)):
        if lineno not in jlines or lineno not in rlines:
            missing_lines += 1
            continue
        jset = {key(m): m for m in jm.get(lineno, [])}
        rset = {key(m): m for m in rm.get(lineno, [])}
        for k in jset:
            if k not in rset:
                only_java += 1
                only_java_by_rule[jset[k]['rule']] += 1
                if len(examples) < show:
                    m = jset[k]
                    examples.append((lineno, 'ONLY-JAVA', m['rule'], m['from'], m['to'],
                                     m['message'][:70], jlines[lineno][:80]))
        for k in rset:
            if k not in jset:
                only_rust += 1
                only_rust_by_rule[rset[k]['rule']] += 1
                if len(examples) < show:
                    m = rset[k]
                    examples.append((lineno, 'ONLY-RUST', m['rule'], m['from'], m['to'],
                                     m['message'][:70], rlines[lineno][:80]))
        for k in jset:
            if k not in rset:
                continue
            a, b = jset[k], rset[k]
            diffs = []
            if a['type'] != b['type']:
                diffs.append(f"type J:{a['type']!r} R:{b['type']!r}")
            if a['message'] != b['message']:
                diffs.append(f"message J:{a['message']!r} R:{b['message']!r}")
            if a['suggestions'] != b['suggestions']:
                diffs.append(f"suggestions J:{a['suggestions']!r} R:{b['suggestions']!r}")
            if diffs:
                field_diffs += 1
                field_diffs_by_rule[a['rule']] += 1
                if len(examples) < show:
                    examples.append((lineno, 'FIELDS ' + '; '.join(diffs),
                                     a['rule'], a['from'], a['to'], '', jlines[lineno][:80]))
    expect_failures = []
    allowed = 0
    for rule, want in expect.items():
        got = field_diffs_by_rule.get(rule, 0)
        allowed += min(got, want)
        if got != want:
            expect_failures.append(('field diffs', rule, got, want))
    unexpected = field_diffs - allowed
    expect_oj_failures = []
    allowed_oj = 0
    for rule, want in expect_only_java.items():
        got = only_java_by_rule.get(rule, 0)
        allowed_oj += min(got, want)
        if got != want:
            expect_oj_failures.append(('only-Java', rule, got, want))
    unexpected_oj = only_java - allowed_oj
    expect_or_failures = []
    allowed_or = 0
    for rule, want in expect_only_rust.items():
        got = only_rust_by_rule.get(rule, 0)
        allowed_or += min(got, want)
        if got != want:
            expect_or_failures.append(('only-Rust', rule, got, want))
    unexpected_or = only_rust - allowed_or
    total_j = sum(len(v) for v in jm.values())
    total_r = sum(len(v) for v in rm.values())
    print(f"lines: {len(jlines)}; matches Java: {total_j}, Rust: {total_r}")
    print(f"only Java: {unexpected_oj}; only Rust: {unexpected_or}; field diffs: {unexpected}; missing lines: {missing_lines}")
    for rule, want in expect.items():
        print(f"allowed field diffs: {field_diffs_by_rule.get(rule, 0)}/{want} {rule}")
    for rule, want in expect_only_java.items():
        print(f"allowed only-Java: {only_java_by_rule.get(rule, 0)}/{want} {rule}")
    for rule, want in expect_only_rust.items():
        print(f"allowed only-Rust: {only_rust_by_rule.get(rule, 0)}/{want} {rule}")
    for kind, rule, got, want in expect_failures + expect_oj_failures + expect_or_failures:
        print(f"EXPECTATION FAILED: {kind} for {rule}: got {got}, want {want}")
    for e in examples:
        lineno, kind, rule, frm, to, msg, text = e
        print(f"[line {lineno}] {kind} {rule} {frm}-{to} {msg} :: {text}")
    failed = bool(unexpected_oj or unexpected_or or unexpected or missing_lines
                  or expect_failures or expect_oj_failures or expect_or_failures)
    return 1 if (fail_on_diff and failed) else 0


if __name__ == '__main__':
    args = sys.argv[1:]
    fail_on_diff = False
    expect = {}
    expect_only_java = {}
    expect_only_rust = {}
    rest = []
    for arg in args:
        if arg == '--fail-on-diff':
            fail_on_diff = True
        elif arg.startswith('--expect-field-diffs='):
            rule, _, count = arg.split('=', 1)[1].rpartition('=')
            expect[rule] = int(count)
        elif arg.startswith('--expect-only-java='):
            rule, _, count = arg.split('=', 1)[1].rpartition('=')
            expect_only_java[rule] = int(count)
        elif arg.startswith('--expect-only-rust='):
            rule, _, count = arg.split('=', 1)[1].rpartition('=')
            expect_only_rust[rule] = int(count)
        else:
            rest.append(arg)
    show = int(rest[2]) if len(rest) > 2 else 60
    sys.exit(main(rest[0], rest[1], show, fail_on_diff, expect, expect_only_java,
                  expect_only_rust))
