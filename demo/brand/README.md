# Brand assets

Logo sources for LingoTweaker. The mark is a cream "L" with an amber wavy
underline — the proofreading underline the checker draws in the editor.

| source | purpose |
|--------|---------|
| `logo-mark.svg` | square mark; served as-is at `demo/public/logo.svg` |
| `logo-wordmark.src.svg` | wide lockup with **live text**; edit this to change the wordmark |

Regenerate everything under `demo/public/` after editing a source:

```sh
demo/brand/build-assets.sh
```

The script copies the mark, outlines the wordmark text into
`demo/public/logo-wordmark.svg` (so the shipped asset needs no font), and
rasterizes the favicons (`favicon.svg`/`logo.svg`, `favicon.ico`,
`favicon-16.png`, `favicon-32.png`, `apple-touch-icon.png`) plus the README
lockup `logo-wordmark.png`. It needs Inkscape and ImageMagick; the generated
files are committed, so the Pages build does not depend on them.

The README references `logo-wordmark.png` through its absolute GitHub Pages
URL (`https://fiduswriter.github.io/LingoTweaker/logo-wordmark.png`) because
relative image paths break when the README is rendered on PyPI.
