#!/usr/bin/env bash
# Regenerate the served brand assets in demo/public/ from the SVG sources in
# this directory. Run it after editing either source file:
#
#   demo/brand/build-assets.sh
#
# Requires Inkscape (SVG rasterizing and text outlining) and ImageMagick
# (multi-size .ico). The generated files are committed, so CI does not need
# these tools.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="$(cd "$here/../public" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# The mark contains no text, so the source doubles as the served SVG.
cp "$here/logo-mark.svg" "$out/logo.svg"

# The wordmark source keeps live <text>; outline it into a font-independent,
# self-contained asset and drop Inkscape's editor metadata.
inkscape "$here/logo-wordmark.src.svg" --export-type=svg \
  --export-filename="$tmp/wordmark.svg" -T >/dev/null
python3 - "$tmp/wordmark.svg" "$out/logo-wordmark.svg" <<'PY'
import re
import sys

src, dst = sys.argv[1], sys.argv[2]
svg = open(src, encoding="utf-8").read()
svg = re.sub(r"<sodipodi:namedview.*?/>", "", svg, flags=re.S)
svg = re.sub(r"<!--.*?-->", "", svg, flags=re.S)
svg = re.sub(r"\s+xmlns:(?:inkscape|sodipodi)=\"[^\"]*\"", "", svg)
svg = re.sub(r"\s+(?:inkscape|sodipodi):[A-Za-z-]+=\"[^\"]*\"", "", svg)
svg = re.sub(r"\n\s*\n", "\n", svg)
open(dst, "w", encoding="utf-8").write(svg)
PY

# Browser tab (SVG + PNG + legacy .ico) and iOS home-screen icon.
inkscape "$out/logo.svg" -w 16 -o "$out/favicon-16.png" >/dev/null
inkscape "$out/logo.svg" -w 32 -o "$out/favicon-32.png" >/dev/null
inkscape "$out/logo.svg" -w 48 -o "$tmp/favicon-48.png" >/dev/null
inkscape "$out/logo.svg" -w 180 -o "$out/apple-touch-icon.png" >/dev/null
magick "$tmp/favicon-48.png" "$out/favicon-32.png" "$out/favicon-16.png" \
  "$out/favicon.ico"

# Wide lockup used by the README (absolute URL on GitHub Pages).
inkscape "$out/logo-wordmark.svg" -w 460 -o "$out/logo-wordmark.png" >/dev/null

echo "brand assets written to $out"
