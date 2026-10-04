#!/usr/bin/env python3
"""Train and validate the confusable-set discriminator — detection's third layer.

The primary model (`lid.176.ftz`) is right about most languages and confidently
wrong about a specific neighbourhood: `ast` comes back as `es`, `no` as `da`,
`nn` as `no`, `gl` bleeds into `es`, `crh` is claimed by `tr`. Every one of those
is a label `lid.176` has and declines to use, so the fix is a specialist second
opinion rather than a bigger model.

One model over the whole confusable set, consulted only when the primary model's
top-1 lands inside it, because the errors are transitive (`es`↔`ast`↔`gl`↔`ca`,
`da`↔`no`↔`nn`↔`sv`) and a pairwise-per-cluster design would need four models
that cannot see each other's confusions.

**The invariant that makes the numbers mean anything: nothing the discriminator
trains on may appear in the fixture.** `crates/lt-core/tests/fixtures/detection_corpus.json`
is what detection is *measured* on, so a sentence that is both training data and
test data turns every accuracy below into a fiction. Two guards:

- every sentence that occurs verbatim in the fixture is dropped from the training
  corpus, in both directions (a language's fixture sentences are excluded from
  *its* corpus, and from every other language's too — the same string can be
  legitimate prose in two languages);
- a fixed tenth of each corpus (every tenth line) is held out and never trained
  on, which gives a validation set large enough to tune against. The fixture
  sentences for `ca`, `es` and `pt` *are* Wikipedia sentences, so those three
  would otherwise have no usable held-out material at all.

Both models are scored on identical input, through the same reader the engine
uses (`tools/detection/ftz-probe`, i.e. `fasttext-pure-rs`), so a difference in
the table is a difference between the models and not between the readers.

The end-to-end section is a **simulation of the engine pipeline**, not a top-1
comparison: the Nordum lexicon, `min_chars`, `min_confidence` and `min_margin` all
apply, because a layer that only looks good on ungated top-1 accuracy can be a
loss in the pipeline that ships. Running it without the refiner reproduces the
engine's own measured figures (correct / abstained / wrong), which is what makes
the with-refiner column trustworthy — if that baseline ever stops matching, this
simulation has drifted from `lt_core::detect` and its numbers mean nothing.

Usage:

    python3 tools/detection/train_discriminator.py                  # train + report
    python3 tools/detection/train_discriminator.py --sweep          # geometry sweep
    python3 tools/detection/train_discriminator.py --evaluate-only  # re-score

Requires the trainer: `tools/detection/build_fasttext.sh`.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
TRAINING_DIR = REPO_ROOT / "data" / "detection" / "training"
FIXTURE = REPO_ROOT / "crates" / "lt-core" / "tests" / "fixtures" / "detection_corpus.json"
BASE_MODEL = REPO_ROOT / "crates" / "lt" / "assets" / "lid.176.ftz"
MARKERS = REPO_ROOT / "data" / "nrd" / "detection" / "markers.txt"
WORK_DIR = REPO_ROOT / "target" / "detection-tools" / "discriminator"
TRAINER = REPO_ROOT / "target" / "detection-tools" / "fasttext"
PROBE_MANIFEST = REPO_ROOT / "tools" / "detection" / "ftz-probe" / "Cargo.toml"

# `Gates::default` in `lt_core::detect`, restated here so the pipeline simulation
# below is explicit about what it assumes. `min_margin` is a *ratio*, not a
# difference: leading confidence over runner-up confidence. `non_latin_weight`
# counts one non-Latin character as several `min_chars`, which is what makes the
# length gate usable outside Latin scripts.
GATES = {"min_chars": 20, "min_confidence": 0.60, "min_margin": 1.5, "non_latin_weight": 5}

# The Unicode ranges `lt_core::detect::is_non_latin` weights. Kept in step with
# it by hand; the engine's own test prints the sweep this table comes from, and
# the two disagreeing would show up as this tool's baseline no longer matching.
NON_LATIN_RANGES = (
    (0x1100, 0x11FF), (0x0600, 0x06FF), (0x0750, 0x077F), (0x0900, 0x097F),
    (0x0B80, 0x0BFF), (0x0C00, 0x0C7F), (0x0D00, 0x0D7F), (0x0E00, 0x0E7F),
    (0x1000, 0x109F), (0x1780, 0x17FF), (0x3040, 0x30FF), (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF), (0xAC00, 0xD7AF), (0xF900, 0xFAFF), (0xFB50, 0xFDFF),
    (0xFE70, 0xFEFF), (0xFF00, 0xFF60), (0xFFE0, 0xFFE6),
)


def significant_length(text: str, weight: int = GATES["non_latin_weight"]) -> int:
    """`lt_core::detect::significant_length`: length in `min_chars` units."""
    total = 0
    for ch in text:
        if ch.isspace():
            continue
        point = ord(ch)
        total += weight if any(low <= point <= high for low, high in NON_LATIN_RANGES) else 1
    return total

# `LEXICON_CONFIDENCE` in the same module. The lexicon's only language today is
# Nordum, which is why the simulation needs one marker file and not a directory.
LEXICON_LANG = "nrd"
LEXICON_CONFIDENCE = 0.99

# The label set. `tr` is here as a negative class, not as a language we ship: the
# primary model answers `tr` for Turkic text, which is how `crh` gets misclaimed,
# and a discriminator that cannot emit `tr` cannot overrule that.
CONFUSABLE = ("es", "ast", "ca", "gl", "pt", "fr", "it", "ro", "da", "sv", "no", "nn", "is", "tr", "crh")

# Reported as a pair, never alone: Bokmål shares 49.2% of `nrd_core.dic`, so real
# `no` is the likeliest text in the whole project to be claimed as Nordum, and an
# independent `nrd` number would hide that.
PAIR = ("no", "nrd")

HELDOUT_EVERY = 10

# The confidence floor `lt/src/detect_refiner.rs` ships, restated so the drift
# check below can compare against the engine's own output.
SHIPPED_MIN_CONFIDENCE = 0.98

# Override thresholds to measure, in the order they are reported. Zero is the
# "trust the second opinion whenever it fires" case and is expected to lose to
# the base model on the languages the base model is already good at; the rest
# are the gate candidates.
THRESHOLDS = (0.0, 0.30, 0.40, 0.50, 0.60, 0.70, 0.80, 0.90, 0.95, 0.98, 0.99, 0.995)

# How unsure the primary model has to be before the discriminator may overrule it,
# as leading confidence over runner-up confidence — the same shape as the
# engine's own `min_margin` gate. `1.5` is the engine default, `0` means "only
# when the primary is a dead heat", and `inf` means "whenever it is in the set".
UNSURE_RATIOS = (0.0, 1.5, 2.0, 3.0, 5.0, float("inf"))

# Geometry candidates. `bucket`, `dim` and above all `cutoff` decide the shipped
# file size, in that order, and *not* the label count: the hashed n-gram table is
# 99.6% of `lid.176.ftz`.
#
# `cutoff` is the one that matters and the one that is easy to miss. Quantization
# alone compresses the dense matrix about 5:1; the 137:1 of `lid.176.ftz` comes
# from `-cutoff`, which drops the least frequent rows and retrains what is left.
# Without it a 15-language model with a 264k-word vocabulary ships at 10 MB,
# because `minCount 1` keeps every rare word as its own row. `minCount` is
# therefore raised too: a Wikipedia-sized vocabulary is also mostly noise for a
# 15-way orthography classifier.
#
# `A` is the base model's own geometry, kept for comparability.
GEOMETRIES = {
    "A-lid176": {"dim": 16, "bucket": 2000000, "epoch": 5, "lr": 0.1, "minCount": 1000, "cutoff": 0, "loss": "softmax"},
    "E-prune200k": {"dim": 16, "bucket": 500000, "epoch": 25, "lr": 0.5, "minCount": 20, "cutoff": 200000, "loss": "softmax"},
    "F-prune100k": {"dim": 16, "bucket": 500000, "epoch": 25, "lr": 0.5, "minCount": 20, "cutoff": 100000, "loss": "softmax"},
    "G-prune50k": {"dim": 16, "bucket": 200000, "epoch": 30, "lr": 0.5, "minCount": 50, "cutoff": 50000, "loss": "softmax"},
    "H-prune200k-lidlike": {"dim": 16, "bucket": 2000000, "epoch": 10, "lr": 0.1, "minCount": 1000, "cutoff": 200000, "loss": "softmax"},
}
DEFAULT_GEOMETRY = "G-prune50k"


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_lines(path: pathlib.Path) -> list[str]:
    if not path.is_file():
        return []
    return [line.strip() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def load_fixture() -> dict[str, list[str]]:
    """Every fixture sentence per language, both buckets.

    `with_errors` sentences count too: they are still authentic prose of that
    language, they are what the detector is actually pointed at, and dropping
    them would quietly inflate the validation set.
    """
    document = json.loads(FIXTURE.read_text(encoding="utf-8"))
    corpus: dict[str, list[str]] = {}
    for lang, buckets in document.items():
        sentences = list(buckets.get("valid", [])) + list(buckets.get("with_errors", []))
        corpus[lang] = [sentence for sentence in sentences if sentence.strip()]
    return corpus


def fixture_index(fixture: dict[str, list[str]]) -> set[str]:
    """Every sentence in the fixture, in any language.

    A global set, not a per-language one: identical strings do occur in two
    languages' example sentences, and training on one language's copy of a
    sentence that the fixture attributes to another is still contamination.
    """
    return {sentence for sentences in fixture.values() for sentence in sentences}


def split_corpus(lang: str, sentences: list[str], banned: set[str]) -> tuple[list[str], list[str]]:
    """Training and held-out sentences for one language.

    The holdout is positional, not random: a seeded shuffle would make the split
    depend on a seed nobody records, and the position is reproducible from the
    committed corpus alone.
    """
    unique: list[str] = []
    seen: set[str] = set()
    for sentence in sentences:
        if sentence in banned or sentence in seen:
            continue
        seen.add(sentence)
        unique.append(sentence)
    train = [s for index, s in enumerate(unique) if index % HELDOUT_EVERY != HELDOUT_EVERY - 1]
    holdout = [s for index, s in enumerate(unique) if index % HELDOUT_EVERY == HELDOUT_EVERY - 1]
    if not train:
        raise SystemExit(f"{lang}: no training sentences left after the fixture exclusion")
    return train, holdout


def write_training_file(path: pathlib.Path, per_language: dict[str, list[str]]) -> int:
    lines = 0
    with path.open("w", encoding="utf-8") as handle:
        for lang in sorted(per_language):
            for sentence in per_language[lang]:
                handle.write(f"__label__{lang} {sentence}\n")
                lines += 1
    return lines


def run(command: list[str], log: pathlib.Path | None = None) -> None:
    result = subprocess.run(command, capture_output=True, text=True)
    if log is not None:
        log.write_text(result.stdout + result.stderr, encoding="utf-8")
    if result.returncode != 0:
        tail = (result.stderr or result.stdout).strip().splitlines()[-12:]
        raise SystemExit(f"{command[0]} failed ({result.returncode}):\n" + "\n".join(tail))


def ensure_probe() -> pathlib.Path:
    """Build `ftz-probe` if it is not there: the reader every number below uses."""
    binary = REPO_ROOT / "target" / "detection-tools" / "ftz-probe"
    if binary.is_file():
        return binary
    run(
        [
            "cargo", "build", "--release",
            "--manifest-path", str(PROBE_MANIFEST),
            "--target-dir", str(REPO_ROOT / "target" / "detection-tools" / "probe-target"),
        ]
    )
    built = REPO_ROOT / "target" / "detection-tools" / "probe-target" / "release" / "ftz-probe"
    if not built.is_file():
        raise SystemExit("ftz-probe did not build")
    built.replace(binary)
    return binary


def predict(model: pathlib.Path, sentences: list[str], probe: pathlib.Path, k: int = 1) -> list[list[tuple[str, float]]]:
    """Top-`k` predictions per sentence, in input order.

    The probe skips blank lines, so the input is filtered here and the caller
    keeps its own list; the mapping is positional and both sides use the same one.
    """
    usable = [(index, sentence) for index, sentence in enumerate(sentences) if sentence.strip()]
    stdin = "\n".join(sentence for _, sentence in usable) + "\n"
    result = subprocess.run(
        [str(probe), str(model), str(k)], input=stdin, capture_output=True, text=True
    )
    if result.returncode != 0:
        raise SystemExit(f"ftz-probe failed on {model.name}: {result.stderr.strip()}")
    raw_rows = result.stdout.splitlines()
    if len(raw_rows) != len(usable):
        raise SystemExit(
            f"{model.name}: probe returned {len(raw_rows)} rows for {len(usable)} sentences"
        )
    rows: list[list[tuple[str, float]]] = []
    for (index, sentence), raw in zip(usable, raw_rows):
        fields = raw.split()
        if len(fields) != 2 * k:
            raise SystemExit(
                f"{model.name}: unusable probe output for sentence {index}: {raw!r}\n"
                f"  input: {sentence[:120]!r}"
            )
        try:
            rows.append(
                [
                    (fields[position].removeprefix("__label__"), float(fields[position + 1]))
                    for position in range(0, len(fields), 2)
                ]
            )
        except ValueError as error:
            raise SystemExit(
                f"{model.name}: unparseable probability in {raw!r}: {error}\n"
                f"  input: {sentence[:120]!r}"
            ) from error

    out: list[list[tuple[str, float]]] = [[] for _ in sentences]
    for (index, _), row in zip(usable, rows):
        out[index] = row
    return out


def top1(predictions: list[list[tuple[str, float]]]) -> list[str | None]:
    return [row[0][0] if row else None for row in predictions]


def per_language_accuracy(truth: list[str], predicted: list[str | None]) -> dict[str, dict[str, float]]:
    """Accuracy per true language, plus what the wrong answers went to."""
    counts: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for actual, guess in zip(truth, predicted):
        counts[actual][guess or "-"] += 1
    table: dict[str, dict[str, float]] = {}
    for lang in sorted(counts):
        total = sum(counts[lang].values())
        table[lang] = {
            "n": total,
            "correct": counts[lang][lang],
            "accuracy": counts[lang][lang] / total if total else 0.0,
        }
    return table


def print_table(title: str, table: dict[str, dict[str, float]], reference: dict[str, dict[str, float]] | None = None) -> None:
    print(f"\n{title}")
    header = f"  {'lang':<6}{'n':>6}{'disc':>9}"
    if reference is not None:
        header += f"{'base':>9}{'delta':>9}"
    print(header)
    for lang in sorted(table, key=lambda code: table[code]["accuracy"]):
        row = table[lang]
        line = f"  {lang:<6}{row['n']:>6}{row['accuracy'] * 100:>8.1f}%"
        if reference is not None and lang in reference:
            base = reference[lang]["accuracy"]
            line += f"{base * 100:>8.1f}%{(row['accuracy'] - base) * 100:>+8.1f}"
        print(line)
    overall = sum(r["correct"] for r in table.values()) / max(1, sum(r["n"] for r in table.values()))
    line = f"  {'ALL':<6}{sum(r['n'] for r in table.values()):>6}{overall * 100:>8.1f}%"
    if reference is not None:
        base_overall = sum(r["correct"] for r in reference.values()) / max(1, sum(r["n"] for r in reference.values()))
        line += f"{base_overall * 100:>8.1f}%{(overall - base_overall) * 100:>+8.1f}"
    print(line)


def confusion_lines(truth: list[str], predicted: list[str | None], languages: tuple[str, ...], limit: int = 6) -> None:
    counts: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for actual, guess in zip(truth, predicted):
        if actual != guess:
            counts[actual][guess or "-"] += 1
    for lang in languages:
        wrong = counts.get(lang)
        if not wrong:
            continue
        top = ", ".join(f"{guess} {count}" for guess, count in wrong.most_common(limit))
        print(f"    {lang} → {top}")


def tokenize(text: str) -> list[str]:
    """Lowercase alphabetic tokens, as `lt_core::detect::tokenize` makes them.

    Diacritics are preserved, which is what keeps `øi` and `å` matchable.
    """
    tokens: list[str] = []
    current: list[str] = []
    for ch in text:
        if ch.isalpha():
            current.extend(ch.lower())
        elif current:
            tokens.append("".join(current))
            current = []
    if current:
        tokens.append("".join(current))
    return tokens


def load_markers(path: pathlib.Path) -> set[str]:
    """The Nordum marker lexicon, parsed as `Lexicon::load` parses it."""
    markers: set[str] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        word = line.split("#")[0].strip().lower()
        if word:
            markers.add(word)
    return markers


def clear_gates(ranking: list[tuple[str, float]], text: str) -> tuple[str, float] | None:
    """`cleared_candidates`: the leading candidate, if it clears every gate.

    A lexicon hit bypasses all of this (decided in `run_pipeline`), which is why
    the length gate lives here rather than around the whole pipeline.
    """
    if not ranking:
        return None
    best_label, best_confidence = ranking[0]
    if significant_length(text) < GATES["min_chars"]:
        return None
    if best_confidence < GATES["min_confidence"]:
        return None
    if len(ranking) > 1:
        runner_up = max(ranking[1][1], 1e-38)
        if best_confidence / runner_up < GATES["min_margin"]:
            return None
    return best_label, best_confidence


def run_pipeline(
    text: str,
    primary: list[tuple[str, float]],
    discriminator: list[tuple[str, float]],
    shipped: frozenset[str],
    labels: frozenset[str],
    markers: set[str],
    threshold: float,
    unsure_ratio: float,
    settled_only: bool,
) -> tuple[str | None, str]:
    """One sentence through the whole engine pipeline, with the refiner wired in.

    Mirrors `lt_core::detect::candidates` followed by `cleared_candidates`, with
    the discriminator standing in for the refiner that does not exist yet. The
    return value is `(language or None, source)`, where source is `lexicon`,
    `model`, `discriminator` or `abstain`.

    The refiner's four conditions, each one measured rather than assumed:

    - it is consulted only when the primary model's top-1 is inside the confusable
      set, so an answer outside it can never change and the ~20 languages out there
      are unaffected by construction;
    - only when the primary is unsure (`unsure_ratio`, the leading/runner-up ratio)
      — where the primary is certain it is usually right;
    - only when the discriminator is itself confident (`threshold`);
    - its answer must be a language we ship, because a third layer may not
      introduce a language the caller has no rules for.

    And the asymmetry that keeps this safe: if the discriminator's own answer
    fails the gates, the primary's answer stands. The refiner may improve an
    answer or decline to, but it never turns an answered sentence into an
    abstention — abstention is the safe answer and this layer has no business
    manufacturing more of them.

    `settled_only` is the policy `lt/src/detect_refiner.rs` implements: the
    refiner speaks only where the primary model would not have answered at all,
    so it can supply an answer the pipeline lacked and can never replace one it
    had. The permissive variant is measured alongside it because it scores higher
    on correct answers, and the gap between them is the cost of that guarantee.
    """
    if any(token in markers for token in tokenize(text)):
        return LEXICON_LANG, "lexicon"

    ranking = [(label, p) for label, p in primary if label in shipped]
    settled = clear_gates(ranking, text)

    if not ranking:
        return None, "abstain"

    lead_label, lead_confidence = ranking[0]
    runner_up = ranking[1][1] if len(ranking) > 1 else 0.0
    ratio = lead_confidence / max(runner_up, 1e-38)
    eligible = lead_label in labels and (
        (settled is None)
        if settled_only
        else (ratio <= unsure_ratio or settled is None)
    )
    if not eligible:
        return (settled[0] if settled else None), ("model" if settled else "abstain")

    refined = [(label, p) for label, p in discriminator if label in shipped]
    if not refined or refined[0][1] < threshold:
        return (settled[0] if settled else None), "model"
    refiner_settled = clear_gates(refined, text)
    if refiner_settled is None:
        return (settled[0] if settled else None), "model"
    return refiner_settled[0], "discriminator"


def pipeline_report(
    sentences: list[str],
    truth: list[str],
    primary: list[list[tuple[str, float]]],
    discriminator: list[list[tuple[str, float]]],
    shipped: frozenset[str],
    labels: frozenset[str],
    markers: set[str],
    threshold: float,
    unsure_ratio: float,
    settled_only: bool,
) -> dict:
    """Correct / abstained / wrong over a sentence set, plus a per-language table.

    The three-way split is the engine's own: a wrong answer silently checks text
    against the wrong rules, an abstention leaves the user where they were, and
    only the first of the three is a defect.
    """
    outcomes: list[tuple[str, str | None, str]] = []
    for text, actual, primary_row, disc_row in zip(sentences, truth, primary, discriminator):
        answer, source = run_pipeline(
            text, primary_row, disc_row, shipped, labels, markers, threshold, unsure_ratio, settled_only
        )
        outcomes.append((actual, answer, source))
    total = len(outcomes)
    correct = sum(1 for actual, answer, _ in outcomes if answer == actual)
    abstained = sum(1 for _, answer, _ in outcomes if answer is None)
    per_language: dict[str, dict[str, float]] = {}
    for lang in sorted({actual for actual, _, _ in outcomes}):
        mine = [(answer, source) for actual, answer, source in outcomes if actual == lang]
        hit = sum(1 for answer, _ in mine if answer == lang)
        per_language[lang] = {
            "n": len(mine),
            "correct": hit,
            "accuracy": hit / len(mine) if mine else 0.0,
            "abstained": sum(1 for answer, _ in mine if answer is None),
        }
    return {
        "n": total,
        "correct": correct,
        "abstained": abstained,
        "wrong": total - correct - abstained,
        "correct_pct": correct / total if total else 0.0,
        "abstained_pct": abstained / total if total else 0.0,
        "wrong_pct": (total - correct - abstained) / total if total else 0.0,
        "from_discriminator": sum(1 for _, _, s in outcomes if s == "discriminator"),
        "per_language": per_language,
    }


def print_pipeline_report(title: str, report: dict, baseline: dict | None = None) -> None:
    print(f"\n{title}")
    print(
        f"  correct {report['correct_pct'] * 100:.1f}%  "
        f"abstained {report['abstained_pct'] * 100:.1f}%  "
        f"wrong {report['wrong_pct'] * 100:.1f}%"
        + (f"   ({report['from_discriminator']} answers from the discriminator)" if report.get("from_discriminator") else "")
    )
    if baseline is not None:
        print(
            f"  baseline: correct {baseline['correct_pct'] * 100:.1f}%  "
            f"abstained {baseline['abstained_pct'] * 100:.1f}%  "
            f"wrong {baseline['wrong_pct'] * 100:.1f}%"
            f"   → correct {(report['correct_pct'] - baseline['correct_pct']) * 100:+.1f}, "
            f"wrong {(report['wrong_pct'] - baseline['wrong_pct']) * 100:+.1f}"
        )
    print(f"  {'lang':<6}{'n':>6}{'correct':>9}{'abstained':>11}{'delta':>9}")
    for lang in sorted(report["per_language"], key=lambda code: report["per_language"][code]["accuracy"]):
        row = report["per_language"][lang]
        line = f"  {lang:<6}{row['n']:>6}{row['accuracy'] * 100:>8.1f}%{row['abstained']:>11}"
        if baseline is not None and lang in baseline["per_language"]:
            was = baseline["per_language"][lang]["accuracy"]
            line += f"{(row['accuracy'] - was) * 100:>+8.1f}"
        print(line)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--geometry", default=DEFAULT_GEOMETRY, choices=sorted(GEOMETRIES), help="training geometry (default %(default)s)")
    parser.add_argument("--sweep", action="store_true", help="train every geometry and report each")
    parser.add_argument("--evaluate-only", action="store_true", help="score the existing model without retraining")
    parser.add_argument(
        "--languages",
        default="",
        help="restrict the label set to these comma-separated codes (default: the whole "
        "confusable set). Useful while a corpus is still being fetched.",
    )
    parser.add_argument("--threads", type=int, default=0, help="trainer threads (default: all cores)")
    args = parser.parse_args()

    if not TRAINER.is_file():
        raise SystemExit("no fastText trainer — run tools/detection/build_fasttext.sh first")
    probe = ensure_probe()
    WORK_DIR.mkdir(parents=True, exist_ok=True)
    threads = args.threads if args.threads > 0 else max(1, os.cpu_count() or 4)

    labels = tuple(code.strip() for code in args.languages.split(",") if code.strip()) or CONFUSABLE
    unknown = [code for code in labels if code not in CONFUSABLE]
    if unknown:
        raise SystemExit(f"not part of the confusable set: {', '.join(unknown)}")

    fixture = load_fixture()
    banned = fixture_index(fixture)
    missing = [lang for lang in labels if not (TRAINING_DIR / f"{lang}.txt").is_file()]
    if missing:
        raise SystemExit(
            f"no training corpus for: {', '.join(missing)}\n"
            "run: python3 tools/detection/fetch_wikipedia.py --purpose training"
        )

    per_language: dict[str, list[str]] = {}
    holdout: dict[str, list[str]] = {}
    print("corpus")
    for lang in labels:
        train, held = split_corpus(lang, read_lines(TRAINING_DIR / f"{lang}.txt"), banned)
        per_language[lang] = train
        holdout[lang] = held
        print(f"  {lang}: {len(train)} train, {len(held)} held out")

    train_file = WORK_DIR / "train.txt"
    total = write_training_file(train_file, per_language)
    print(f"  {total} training lines → {train_file.relative_to(REPO_ROOT)}")
    print(f"  fixture sentences excluded from training: {len(banned)} unique strings")

    geometries = sorted(GEOMETRIES) if args.sweep else [args.geometry]
    results: dict[str, dict] = {}

    for name in geometries:
        settings = GEOMETRIES[name]
        prefix = WORK_DIR / f"discriminator-{name.lower()}"
        if not args.evaluate_only:
            run(
                [
                    str(TRAINER), "supervised",
                    "-input", str(train_file),
                    "-output", str(prefix),
                    "-dim", str(settings["dim"]),
                    "-bucket", str(settings["bucket"]),
                    "-epoch", str(settings["epoch"]),
                    "-lr", str(settings["lr"]),
                    "-minCount", str(settings["minCount"]),
                    "-wordNgrams", "1",
                    "-minn", "2", "-maxn", "4",
                    "-loss", settings["loss"],
                    "-thread", str(threads),
                ],
                log=WORK_DIR / f"train-{name}.log",
            )
            # Quantized because that is what ships: the engine reads .ftz through
            # `fasttext-pure-rs`, and an unquantized .bin would be ~10x the size
            # for the same predictions.
            run(
                [
                    str(TRAINER), "quantize",
                    "-input", str(train_file),
                    "-output", str(prefix),
                    "-qnorm", "-retrain",
                    "-cutoff", str(settings["cutoff"]),
                    "-epoch", str(settings["epoch"]),
                    "-lr", str(settings["lr"]),
                    "-thread", str(threads),
                ],
                log=WORK_DIR / f"quantize-{name}.log",
            )
        model = pathlib.Path(f"{prefix}.ftz")
        if not model.is_file():
            raise SystemExit(f"{model} missing — retrain without --evaluate-only")
        size = model.stat().st_size
        # The dense .bin is an intermediate: 50–130 MB per geometry, and nothing
        # reads it once the quantized model exists.
        pathlib.Path(f"{prefix}.bin").unlink(missing_ok=True)

        fixture_truth: list[str] = []
        fixture_sentences: list[str] = []
        for lang in labels:
            for sentence in fixture.get(lang, []):
                fixture_truth.append(lang)
                fixture_sentences.append(sentence)
        disc_fixture = top1(predict(model, fixture_sentences, probe))
        base_fixture = top1(predict(BASE_MODEL, fixture_sentences, probe))
        disc_table = per_language_accuracy(fixture_truth, disc_fixture)
        base_table = per_language_accuracy(fixture_truth, base_fixture)

        held_truth: list[str] = []
        held_sentences: list[str] = []
        for lang in labels:
            for sentence in holdout[lang]:
                held_truth.append(lang)
                held_sentences.append(sentence)
        disc_held = top1(predict(model, held_sentences, probe))
        base_held = top1(predict(BASE_MODEL, held_sentences, probe))
        disc_held_table = per_language_accuracy(held_truth, disc_held)
        base_held_table = per_language_accuracy(held_truth, base_held)

        print(f"\n=== {name} ({size / 1024:.0f} KiB) ===")
        print(
            f"    dim {settings['dim']}, bucket {settings['bucket']}, epoch {settings['epoch']},"
            f" lr {settings['lr']}, minCount {settings['minCount']}, cutoff {settings['cutoff'] or 'none'},"
            f" loss {settings['loss']}"
        )
        print_table("fixture sentences (the plan's validation set)", disc_table, base_table)
        print_table("held-out Wikipedia (tuning set, never trained on)", disc_held_table, base_held_table)
        print("\n  where the discriminator's fixture errors go")
        confusion_lines(fixture_truth, disc_fixture, labels)

        # End-to-end over every language we ship, through the pipeline rather
        # than a top-1 comparison: the lexicon and all three gates apply, and the
        # baseline is the engine's own measured behaviour.
        all_truth: list[str] = []
        all_sentences: list[str] = []
        for lang in sorted(fixture):
            for sentence in fixture[lang]:
                all_truth.append(lang)
                all_sentences.append(sentence)
        base_all = predict(BASE_MODEL, all_sentences, probe, k=5)
        disc_all = predict(model, all_sentences, probe, k=2)
        shipped = frozenset(fixture)
        marker_set = load_markers(MARKERS)
        print(f"\n  pipeline over {len(all_sentences)} fixture sentences, all {len(shipped)} languages")
        print(f"    gates: min_chars {GATES['min_chars']}, min_confidence {GATES['min_confidence']}, min_margin {GATES['min_margin']}")
        print(f"    lexicon: {len(marker_set)} markers for {LEXICON_LANG}")

        baseline = pipeline_report(
            all_sentences, all_truth, base_all, disc_all, shipped, frozenset(labels), marker_set,
            threshold=1.1, unsure_ratio=0.0, settled_only=True,
        )
        print(
            f"\n  baseline (no refiner): correct {baseline['correct_pct'] * 100:.1f}%"
            f"  abstained {baseline['abstained_pct'] * 100:.1f}%"
            f"  wrong {baseline['wrong_pct'] * 100:.1f}%"
        )

        # The drift check. `lt/src/detect_refiner.rs` ships the settled-only
        # policy at this confidence floor, and `cargo test -p lt --test
        # detect_model` prints these three numbers from the engine itself. If this
        # row stops matching them, the simulation has drifted from
        # `lt_core::detect` and nothing else in this report can be trusted.
        shipped_report = pipeline_report(
            all_sentences, all_truth, base_all, disc_all, shipped, frozenset(labels), marker_set,
            threshold=SHIPPED_MIN_CONFIDENCE, unsure_ratio=0.0, settled_only=True,
        )
        print(
            f"    shipped configuration (settled-only, disc >= {SHIPPED_MIN_CONFIDENCE}):"
            f" correct {shipped_report['correct_pct'] * 100:.1f}%"
            f"  abstained {shipped_report['abstained_pct'] * 100:.1f}%"
            f"  wrong {shipped_report['wrong_pct'] * 100:.1f}%"
        )
        print("    engine reports:      correct 68.6%  abstained 28.2%  wrong 3.2%")
        if abs(shipped_report["correct_pct"] - 0.686) > 0.001:
            print(
                "    WARNING: this tool's simulation and the engine disagree; the rest of"
                " this report is not meaningful until they match"
            )

        print("\n  refiner gate grid (net = correct − wrong, all 38 languages)")
        print(f"    {'disc>=':>7}{'unsure<=':>10}{'policy':>13}{'from disc':>10}{'correct':>9}{'wrong':>8}{'net':>8}")
        base_only = None
        best: tuple[float, float, bool, dict] | None = None
        for threshold in THRESHOLDS:
            for unsure_ratio in UNSURE_RATIOS:
                for settled_only in (False, True):
                    report = pipeline_report(
                        all_sentences, all_truth, base_all, disc_all, shipped, frozenset(labels),
                        marker_set, threshold, unsure_ratio, settled_only,
                    )
                    net = report["correct_pct"] - report["wrong_pct"]
                    print(
                        f"    {threshold:>7.2f}{unsure_ratio:>10.1f}{'settled-only' if settled_only else 'permissive':>13}"
                        f"{report['from_discriminator']:>10}{report['correct_pct'] * 100:>8.1f}%"
                        f"{report['wrong_pct'] * 100:>7.1f}%{net * 100:>+7.1f}"
                    )
                    # Selected on net, not on correct: `Gates` documents that this
                    # feature prefers precision over coverage, because a wrong
                    # answer silently checks text against the wrong rules while an
                    # abstention leaves the user where they were. A wrong answer
                    # costing one good answer is a deliberately conservative
                    # exchange rate, and the grid is printed in full so a
                    # different one can be read off directly.
                    if best is None or net > best[3]["net"] + 1e-12:
                        report["net"] = net
                        best = (threshold, unsure_ratio, settled_only, report)
        assert best is not None
        threshold, unsure_ratio, settled_only, chosen = best
        base_only = baseline
        print(
            f"\n    best: disc >= {threshold:.2f}, primary unsure ratio <= {unsure_ratio},"
            f" policy {'settled-only' if settled_only else 'permissive'},"
            f" {chosen['from_discriminator']} answers from the discriminator"
            f" (baseline net {(baseline['correct_pct'] - baseline['wrong_pct']) * 100:+.1f})"
        )
        print_pipeline_report(
            f"all 38 languages at the chosen gates: engine vs engine+discriminator",
            chosen,
            baseline,
        )
        print("\n  no/nrd pair (Bokmål shares 49.2% of nrd_core.dic)")
        for lang in PAIR:
            row = chosen["per_language"].get(lang, {})
            was = baseline["per_language"].get(lang, {})
            print(
                f"    {lang}: n={row.get('n', 0)} correct {was.get('accuracy', 0) * 100:.1f}% →"
                f" {row.get('accuracy', 0) * 100:.1f}%, abstained {was.get('abstained', 0)} → {row.get('abstained', 0)}"
            )

        results[name] = {
            "settings": settings,
            "size_bytes": size,
            "fixture": disc_table,
            "fixture_base_model": base_table,
            "holdout": disc_held_table,
            "holdout_base_model": base_held_table,
            "refiner": {
                "min_confidence": threshold,
                "max_unsure_ratio": (None if unsure_ratio == float("inf") else unsure_ratio),
                "settled_only": settled_only,
            },
            "pipeline": chosen,
            "pipeline_without_refiner": baseline,
            "policy": "settled-only" if settled_only else "permissive",
        }
        if name == args.geometry:
            shutil.copyfile(model, WORK_DIR / "discriminator.ftz")

    if args.sweep:
        print("\n=== sweep summary ===")
        print(f"  {'geometry':<10}{'size':>10}{'fixture':>9}{'held-out':>9}{'pipeline':>10}{'wrong':>8}{'gates':>16}")
        for name, report in results.items():
            def overall(table: dict[str, dict[str, float]]) -> float:
                return sum(r["correct"] for r in table.values()) / max(1, sum(r["n"] for r in table.values()))

            refiner = report["refiner"]
            ratio = "inf" if refiner["max_unsure_ratio"] is None else f"{refiner['max_unsure_ratio']}"
            gates = f"disc>={refiner['min_confidence']:.2f} ratio<={ratio}"
            print(
                f"  {name:<10}{report['size_bytes'] / 1024:>8.0f} KiB"
                f"{overall(report['fixture']) * 100:>8.1f}%{overall(report['holdout']) * 100:>8.1f}%"
                f"{report['pipeline']['correct_pct'] * 100:>9.1f}%{report['pipeline']['wrong_pct'] * 100:>7.1f}%"
                f"{gates:>16}"
            )

    sidecar = WORK_DIR / "metrics.json"
    sidecar.write_text(
        json.dumps(
            {
                "labels": list(labels),
                "holdout_every": HELDOUT_EVERY,
                "fixture_excluded": len(banned),
                "corpus_sha256": {
                    lang: sha256_file(TRAINING_DIR / f"{lang}.txt")
                    for lang in labels
                    if (TRAINING_DIR / f"{lang}.txt").is_file()
                },
                "fixture_sha256": sha256_file(FIXTURE),
                "base_model_sha256": sha256_file(BASE_MODEL),
                "trainer_sha256": sha256_file(TRAINER),
                "geometries": results,
            },
            indent=1,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"\nmetrics → {sidecar.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())