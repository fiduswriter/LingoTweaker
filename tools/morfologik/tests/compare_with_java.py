#!/usr/bin/env python3
"""Optional parity check of the Python port against the Java tooling.

Not part of CI: it needs the Morfologik 2.2.0 jars (and optionally a
LanguageTool ``languagetool-tools`` classpath). Example::

    python3 tools/morfologik/tests/compare_with_java.py \
        --morfologik-cp /tmp/morfologik-tools-2.2.0.jar:/tmp/morfologik-fsa-2.2.0.jar:/tmp/morfologik-fsa-builders-2.2.0.jar:/tmp/morfologik-stemming-2.2.0.jar:/tmp/jcommander-1.78.jar:/tmp/hppc.jar \
        --fuzz 40 \
        --dict data/no/dictionaries/no.dict \
        --dict data/en/dictionaries/english.dict
"""

import argparse
import os
import random
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from morfologik import tools  # noqa: E402
from morfologik.serializer import compile_fsa  # noqa: E402


def _java(morfologik_cp, main_class, args):
    cmd = ["java", "-cp", morfologik_cp, main_class] + args
    return subprocess.run(cmd, capture_output=True, text=True)


def fuzz_fsa_compile(morfologik_cp, trials, seed=1234):
    random.seed(seed)
    alphabet = "abcdefghijklmnopqrstuvwxyzæøåäöüß"
    failures = 0
    with tempfile.TemporaryDirectory() as tmp:
        input_path = os.path.join(tmp, "fuzz.txt")
        output_path = os.path.join(tmp, "fuzz.dict")
        for trial in range(trials):
            count = random.choice([0, 1, 2, 3, 5, 20, 100, 500])
            words = {
                "".join(random.choice(alphabet) for _ in range(random.randint(0, 12)))
                for _ in range(count)
            }
            sequences = sorted(w.encode("utf-8") for w in words)
            with open(input_path, "wb") as handle:
                handle.write(b"\n".join(sequences) + (b"\n" if sequences else b""))
            result = _java(
                morfologik_cp,
                "morfologik.tools.FSACompile",
                ["-i", input_path, "-o", output_path, "-f", "CFSA2", "--ignore-empty"],
            )
            if result.returncode != 0:
                print(f"trial {trial}: Java failed: {result.stderr.strip()}")
                failures += 1
                continue
            with open(output_path, "rb") as handle:
                java_bytes = handle.read()
            if compile_fsa(sequences) != java_bytes:
                print(f"trial {trial}: MISMATCH (n={count})")
                failures += 1
    print(f"fsa_compile fuzz: {trials - failures}/{trials} identical")
    return failures


def roundtrip_dict(morfologik_cp, dict_path):
    """Decompile with Java, recompile with Java and Python, compare."""
    info_path = os.path.splitext(dict_path)[0] + ".info"
    if not os.path.exists(info_path):
        print(f"{dict_path}: missing sibling .info, skipped")
        return 0
    with tempfile.TemporaryDirectory() as tmp:
        stem = os.path.basename(dict_path).replace(".", "_")
        work_input = os.path.join(tmp, stem + ".input")
        java_dict = os.path.join(tmp, stem + ".dict")
        py_dict = os.path.join(tmp, "py_" + stem + ".dict")
        shutil_copy(info_path, os.path.join(tmp, stem + ".info"))

        result = _java(
            morfologik_cp,
            "morfologik.tools.DictDecompile",
            ["-i", dict_path, "-o", work_input, "--overwrite"],
        )
        if result.returncode != 0:
            print(f"{dict_path}: Java decompile failed: {result.stderr.strip()}")
            return 1
        result = _java(
            morfologik_cp,
            "morfologik.tools.DictCompile",
            ["-i", work_input, "-f", "CFSA2", "--overwrite"],
        )
        if result.returncode != 0:
            print(f"{dict_path}: Java recompile failed: {result.stderr.strip()}")
            return 1
        tools.dict_compile(work_input, output_path=py_dict, overwrite=True, validate=False)

        with open(java_dict, "rb") as handle:
            java_bytes = handle.read()
        with open(py_dict, "rb") as handle:
            py_bytes = handle.read()
        if java_bytes == py_bytes:
            print(f"{dict_path}: identical ({len(py_bytes)} bytes)")
            return 0
        print(f"{dict_path}: MISMATCH java={len(java_bytes)} py={len(py_bytes)}")
        return 1


def shutil_copy(src, dst):
    with open(src, "rb") as s, open(dst, "wb") as d:
        d.write(s.read())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--morfologik-cp", required=True, help="classpath with the Morfologik jars")
    parser.add_argument("--fuzz", type=int, default=0, help="number of random FSA compile trials")
    parser.add_argument("--dict", action="append", default=[], help="a .dict to round-trip")
    args = parser.parse_args()

    failures = 0
    if args.fuzz:
        failures += fuzz_fsa_compile(args.morfologik_cp, args.fuzz)
    for dict_path in args.dict:
        failures += roundtrip_dict(args.morfologik_cp, dict_path)

    if failures:
        print(f"{failures} failure(s)")
        return 1
    print("all comparisons identical")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
