#!/usr/bin/env bash
# Set the LingoTweaker prerelease version everywhere it is hardcoded:
#   - Cargo.toml            [workspace.package].version + lt-* path-dep versions
#   - crates/lingotweaker/Cargo.toml   (own workspace: version + dep versions)
#   - crates/lt-node/package.json      (npm)
#
# The Python version is derived from the Cargo version by maturin
# (0.1.0-alpha.1 -> 0.1.0a1), so pyproject.toml stays `dynamic`.
#
# Usage: scripts/release/set-version.sh 0.1.0-alpha.2
set -euo pipefail

NEW="${1:?usage: set-version.sh <semver, e.g. 0.1.0-alpha.2>}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

OLD="$(sed -nE 's/^version = "(.*)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
if [ -z "$OLD" ]; then
  echo "set-version: cannot read workspace version from $ROOT/Cargo.toml" >&2
  exit 1
fi
if [ "$OLD" = "$NEW" ]; then
  echo "set-version: already $NEW"
  exit 0
fi

# Escape dots for use as a sed pattern; the versions are plain semver here.
old_re="${OLD//./\\.}"
sed -i "s/${old_re}/${NEW}/g" "$ROOT/Cargo.toml"
sed -i "s/${old_re}/${NEW}/g" "$ROOT/crates/lingotweaker/Cargo.toml"

node - "$ROOT/crates/lt-node/package.json" "$NEW" <<'NODE'
const fs = require("fs");
const [path, version] = process.argv.slice(2);
const pkg = JSON.parse(fs.readFileSync(path, "utf8"));
pkg.version = version;
fs.writeFileSync(path, JSON.stringify(pkg, null, 2) + "\n");
NODE

python_version="$(python3 - "$NEW" <<'PY'
import sys
v = sys.argv[1]
core, _, pre = v.partition("-")
if pre.startswith("alpha."):
    print(f"{core}a{pre.split('.')[1]}")
else:
    print(v)
PY
)"

cat <<EOF
set-version: $OLD -> $NEW
  Cargo workspace / facade: $NEW
  npm:                     $NEW
  PyPI (maturin-derived):  $python_version
EOF