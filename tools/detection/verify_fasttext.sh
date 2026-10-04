#!/usr/bin/env bash
# Prove that a model trained and quantized by the C++ trainer loads in the
# engine's inference path (`fasttext-pure-rs`) and returns the same predictions
# the trainer's own CLI does.
#
# This is the gate the discriminator work depends on: the engine can only ship a
# trained model if the pure-Rust reader accepts the bytes the trainer produced.
# `fasttext-pure-rs` is inference-only, so nothing in the Rust build can produce
# a model — this closes the loop.
#
#   tools/detection/build_fasttext.sh && tools/detection/verify_fasttext.sh
#
# Trains a throwaway 5-language model on the vendored Wikipedia cache
# (`data/detection/wikipedia/`, one of the two corpora the real discriminator
# will use) and diffs trainer output against probe output. Nothing is written
# outside `target/`.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALL_DIR="${FASTTEXT_DIR:-$REPO_ROOT/target/detection-tools}"
WORK="$INSTALL_DIR/verify"
TRAINER="${FASTTEXT_BIN:-$INSTALL_DIR/fasttext}"
PROBE_MANIFEST="$REPO_ROOT/tools/detection/ftz-probe/Cargo.toml"
PROBE_BIN="$REPO_ROOT/target/detection-tools/ftz-probe"

if [ ! -x "$TRAINER" ]; then
	printf 'no trainer at %s — run tools/detection/build_fasttext.sh first\n' "$TRAINER" >&2
	exit 1
fi

rm -rf "$WORK"
mkdir -p "$WORK"

for lang in ca es pt sr uk; do
	sed "s/^/__label__$lang /" "$REPO_ROOT/data/detection/wikipedia/$lang.txt"
done >"$WORK/train.txt"

# Geometry deliberately unlike the default: the point is that a *small* model
# round-trips, since that is what the discriminator will be.
"$TRAINER" supervised \
	-input "$WORK/train.txt" \
	-output "$WORK/model" \
	-dim 16 \
	-bucket 50000 \
	-minn 2 \
	-maxn 4 \
	-epoch 20 \
	-loss softmax \
	-thread "$(nproc 2>/dev/null || echo 4)" >/dev/null 2>"$WORK/trainer.log"

"$TRAINER" quantize \
	-input "$WORK/train.txt" \
	-output "$WORK/model" \
	-qnorm \
	-retrain \
	-epoch 20 \
	-thread "$(nproc 2>/dev/null || echo 4)" >/dev/null 2>>"$WORK/trainer.log"

test -f "$WORK/model.ftz" || {
	printf 'quantize produced no model.ftz; trainer log:\n' >&2
	cat "$WORK/trainer.log" >&2
	exit 1
}

# Held-out-ish probe sentences: one per label, none of them in the cache files
# verbatim, two labels asked for so ordering is compared too.
cat >"$WORK/probe.txt" <<'PROBE'
A poboación no ano 2020 era de 1034 133 habitantes.
Wikimedia Commons alberga unha categoría multimedia sobre eses escritor.
No мне известно, як гэта адбываецца насамрэч.
PROBE

"$TRAINER" predict-prob "$WORK/model.ftz" - 2 <"$WORK/probe.txt" >"$WORK/from-trainer.txt"

if [ ! -x "$PROBE_BIN" ]; then
	cargo build --release --manifest-path "$PROBE_MANIFEST" --target-dir "$INSTALL_DIR/probe-target" >&2
	PROBE_BIN="$(find "$INSTALL_DIR/probe-target/release" -maxdepth 1 -name ftz-probe -type f | head -1)"
	cp "$PROBE_BIN" "$INSTALL_DIR/ftz-probe"
fi
"$INSTALL_DIR/ftz-probe" "$WORK/model.ftz" 2 <"$WORK/probe.txt" >"$WORK/from-probe.txt"

# Numeric comparison, not a text diff: the trainer prints six significant digits
# (and trims trailing zeros), the probe prints the full f32. Labels and ordering
# must match exactly; probabilities to within f32 noise.
python3 - "$WORK/from-trainer.txt" "$WORK/from-probe.txt" <<'PY'
import sys

TOLERANCE = 1e-6


def parse(path):
    rows = []
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            fields = line.split()
            rows.append([(fields[i], float(fields[i + 1])) for i in range(0, len(fields), 2)])
    return rows


trainer, probe = (parse(path) for path in sys.argv[1:3])
if len(trainer) != len(probe):
    sys.exit(f"line count differs: trainer {len(trainer)}, probe {len(probe)}")

for index, (left, right) in enumerate(zip(trainer, probe)):
    if [label for label, _ in left] != [label for label, _ in right]:
        sys.exit(f"line {index + 1}: labels differ: {left} vs {right}")
    for (label, a), (_, b) in zip(left, right):
        if abs(a - b) > TOLERANCE:
            sys.exit(f"line {index + 1}: {label} probability {a} vs {b}")

print(f"compared {len(trainer)} lines, labels identical, probabilities within {TOLERANCE}")
PY

printf 'OK: %s (%s bytes) loads in fasttext-pure-rs with identical predictions\n' \
	"$WORK/model.ftz" "$(stat -c%s "$WORK/model.ftz")"
