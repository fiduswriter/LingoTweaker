#!/usr/bin/env python3
"""Group a CheckDump diff by rule id / field (French corpus triage).

Usage: scripts/oracle/fr/analyze-diff.py <java.tsv> <rust.tsv>
Prints ONLY-JAVA / ONLY-RUST / FIELD-DIFF counts per rule plus examples.
"""
import collections
import sys

FIELDS = ["rule", "sub", "from", "to", "type", "msg", "sugs"]


def load(path):
    out = collections.defaultdict(list)
    for line in open(path, encoding="utf-8"):
        parts = line.rstrip("\n").split("\t")
        if parts[0] == "L":
            out[int(parts[1])]
        elif parts[0] == "M":
            out[int(parts[1])].append(tuple(parts[2:]))
    return out


def main():
    java, rust = load(sys.argv[1]), load(sys.argv[2])
    only_java = collections.Counter()
    only_rust = collections.Counter()
    field_diffs = collections.Counter()
    field_names = collections.Counter()
    examples = collections.defaultdict(list)
    for ln in java:
        jm, rm = java[ln], rust.get(ln, [])
        key = lambda m: (m[0], m[1], m[2], m[3])
        jk, rk = [key(m) for m in jm], [key(m) for m in rm]
        for m in jm:
            if key(m) not in rk:
                only_java[m[0]] += 1
                if len(examples[("J", m[0])]) < 4:
                    examples[("J", m[0])].append((ln, m[0], m[2], m[3], m[6][:60]))
        for m in rm:
            if key(m) not in jk:
                only_rust[m[0]] += 1
                if len(examples[("R", m[0])]) < 4:
                    examples[("R", m[0])].append((ln, m[0], m[2], m[3], m[6][:60]))
        jmap = {key(m): m for m in jm}
        for m in rm:
            j = jmap.get(key(m))
            if j is not None and j != m:
                field_diffs[m[0]] += 1
                for idx in range(max(len(j), len(m))):
                    a = j[idx] if idx < len(j) else ""
                    b = m[idx] if idx < len(m) else ""
                    if a != b:
                        field_names[(m[0], FIELDS[idx] if idx < len(FIELDS) else str(idx))] += 1
                if len(examples[("F", m[0])]) < 6:
                    examples[("F", m[0])].append(
                        (ln, m[0], j[6][:50], j[7][:70] if len(j) > 7 else "",
                         m[6][:50], m[7][:70] if len(m) > 7 else "")
                    )
    print("== ONLY-JAVA by rule ==")
    for k, v in only_java.most_common(25):
        print(f"{v:5d} {k}")
    print("== ONLY-RUST by rule ==")
    for k, v in only_rust.most_common(25):
        print(f"{v:5d} {k}")
    print("== FIELD DIFFS by rule ==")
    for k, v in field_diffs.most_common(25):
        print(f"{v:5d} {k}")
    print("== FIELD DIFFS by (rule,field) ==")
    for k, v in field_names.most_common(20):
        print(f"{v:5d} {k}")
    print("== examples ==")
    for kind_rule, exs in sorted(examples.items()):
        print(f"-- {kind_rule[0]} {kind_rule[1]}")
        for e in exs[:4]:
            print("   ", e)


if __name__ == "__main__":
    main()
