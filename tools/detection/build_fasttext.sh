#!/usr/bin/env bash
# Build the fastText *trainer* (the C++ CLI) into target/detection-tools/.
#
# Inference is `fasttext-pure-rs`, which is inference-only: it loads a model and
# predicts, and cannot train one. Training the confusable-set discriminator
# therefore needs the reference C++ implementation, which no package on this
# machine provides — the Python `fasttext` module is source-only (no cp3xx wheel),
# and `messense/fasttext` is a crate that pulls rayon, which breaks the wasm
# build.
#
# So: build it from a pinned upstream commit. Nothing here is committed; the
# binary is a build artifact under `target/`, and only the training *inputs* and
# the trained model are tracked data.
#
#   tools/detection/build_fasttext.sh          # build if missing
#   FORCE=1 tools/detection/build_fasttext.sh  # rebuild from scratch
#
# A model trained and quantized by this CLI loads in `fasttext-pure-rs` and
# returns bit-identical probabilities — see `verify_fasttext.sh`.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALL_DIR="${FASTTEXT_DIR:-$REPO_ROOT/target/detection-tools}"
SRC_DIR="$INSTALL_DIR/fastText-src"
BINARY="$INSTALL_DIR/fasttext"

# v0.9.2, the last upstream release, pinned by commit: a GitHub-generated
# release tarball is not byte-stable, a commit is.
UPSTREAM_URL="https://github.com/facebookresearch/fastText"
UPSTREAM_TAG="v0.9.2"
UPSTREAM_COMMIT="5b5943c118b0ec5fb9cd8d20587de2b2d3966dfe"

if [ -n "${FASTTEXT_BIN:-}" ]; then
	# An externally installed trainer; nothing to build.
	printf 'fastText trainer: %s (FASTTEXT_BIN)\n' "$FASTTEXT_BIN"
	exit 0
fi

if [ -x "$BINARY" ] && [ -z "${FORCE:-}" ]; then
	printf 'fastText trainer: %s (already built; FORCE=1 to rebuild)\n' "$BINARY"
	exit 0
fi

mkdir -p "$INSTALL_DIR"

if [ ! -d "$SRC_DIR/.git" ]; then
	rm -rf "$SRC_DIR"
	git init -q "$SRC_DIR"
	git -C "$SRC_DIR" remote add origin "$UPSTREAM_URL"
fi

git -C "$SRC_DIR" fetch -q --depth 1 origin "refs/tags/$UPSTREAM_TAG"
got="$(git -C "$SRC_DIR" rev-parse FETCH_HEAD)"
if [ "$got" != "$UPSTREAM_COMMIT" ]; then
	printf 'tag %s is %s, expected %s — refusing to build\n' \
		"$UPSTREAM_TAG" "$got" "$UPSTREAM_COMMIT" >&2
	exit 1
fi
git -C "$SRC_DIR" checkout -q --detach "$UPSTREAM_COMMIT"

# `args.cc` uses uint64_t without including <cstdint>, which older libstdc++
# headers pulled in transitively. GCC 13+ (here: 15.2) does not, so the upstream
# tree does not compile as shipped.
python3 - "$SRC_DIR/src/args.cc" <<'PY'
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as fh:
    text = fh.read()
if "#include <cstdint>" in text:
    sys.exit(0)
text = text.replace(
    "#include <iostream>",
    "#include <cstdint>\n#include <iostream>",
    1,
)
with open(path, "w", encoding="utf-8") as fh:
    fh.write(text)
PY

make -C "$SRC_DIR" -j"$(nproc 2>/dev/null || echo 4)"

# Upstream's Makefile leaves the binary in the source directory; move it out so
# `SRC_DIR` can be deleted and rebuilt without confusing a stale copy.
cp "$SRC_DIR/fasttext" "$BINARY"

printf 'fastText trainer: %s\n' "$BINARY"
"$BINARY" 2>&1 | head -2 || true
