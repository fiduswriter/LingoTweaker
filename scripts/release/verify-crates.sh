#!/usr/bin/env bash
# Verify a published `lingotweaker` crate from a scratch project: `cargo add`
# the published version (which pulls the published lt-* crates) and compile a
# small program against the `lt` library name.
#
# Usage: scripts/release/verify-crates.sh 0.1.0-alpha.1
set -euo pipefail

VER="${1:?usage: verify-crates.sh <crate-version, e.g. 0.1.0-alpha.1>}"
CARGO_BIN=(cargo +1.98.1)
if [ -n "${CARGO:-}" ]; then CARGO_BIN=("$CARGO"); fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/lingotweaker-verify.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

"${CARGO_BIN[@]}" init -q --name scratch --vcs none .
"${CARGO_BIN[@]}" add "lingotweaker@=$VER"

cat > src/main.rs <<'EOF'
fn main() {
    // `[lib] name = "lt"` keeps the import name stable for consumers.
    let builder = lt::Engine::builder(lt::Lang::En).unwrap();
    let _ = builder;
    println!("lingotweaker facade ok: {:?}", lt::Lang::En);
}
EOF

"${CARGO_BIN[@]}" build
echo "== verify-crates: lingotweaker@$VER builds from crates.io"