# Release helpers

One prerelease version is shared by all three registries. Cargo uses
`0.1.0-alpha.N`, the tag is `v0.1.0-alpha.N`, npm uses `0.1.0-alpha.N`, and
maturin derives the PEP 440 version `0.1.0aN` from the Cargo version.

```sh
scripts/release/set-version.sh 0.1.0-alpha.2   # bump once, commit
git tag v0.1.0-alpha.2 && git push origin v0.1.0-alpha.2   # triggers Release workflow
```

Manual equivalents (what `.github/workflows/release.yml` runs):

```sh
scripts/release/publish-crates.sh              # crates.io, dependency order + facade
scripts/release/publish-pypi.sh [--build-only] # maturin wheel+sdist, smoke, twine
scripts/release/publish-npm-data.sh [--build-only]  # npm lingotweaker-data (all packs)
scripts/release/publish-npm.sh  [--build-only] # napi build, smoke, npm --tag next
scripts/release/publish-wasm.sh [--build-only] # wasm-pack web+nodejs, smoke, npm next
scripts/release/publish-pypi-data.sh [--build-only]  # PyPI lingotweaker-data-<lang>
scripts/release/publish-data.sh v0.1.0-alpha.1 [--build-only]  # GitHub Release data assets
scripts/release/verify-crates.sh 0.1.0-alpha.1 # scratch `cargo add` build check
```

## Data

Engine packages ship code only; runtime data is published per language from
one build (`scripts/release/build-data.sh`, which reuses
`scripts/data/build-packs.sh`):

| Artifact | Registry / host | Consumer |
|----------|-----------------|----------|
| `packs/<lang>.pack.gz` | GitHub Release + npm `lingotweaker-data` | wasm `LtEngine`, native Node `dataDir` |
| `data/<lang>.tar.gz` | GitHub Release | native `LT_DATA_DIR` (extract) |
| `lingotweaker-data-<lang>` | PyPI | `lt_py` (auto-discovered) |
| `manifest.json` | GitHub Release | sizes + sha256 per artifact |

`publish-data.sh` creates the tag's release if needed and uploads the assets;
`publish-npm-data.sh` stages the packs into the `lingotweaker-data` package;
`publish-pypi-data.sh` builds one `py3-none-any` wheel per language (each well
under PyPI's 100 MB/file limit). `lingotweaker` and `lingotweaker-wasm` declare
`lingotweaker-data` as a dependency.

Notes:

- `crates.io` rate-limits *new* crates and returns a retry time in the 429
  response. Re-run `publish-crates.sh`; it continues where it stopped.
- PyPI/npm dist-tags: stable versions go to npm `latest`, prereleases to `next`
  (so `latest` stays on the last stable release). PyPI marks prereleases via
  the PEP 440 version itself.
- `publish-pypi-data.sh` needs a `PYPI_API_TOKEN` secret (or `TWINE_PASSWORD`);
  without it the workflow builds the wheels and skips the upload. PyPI trusted
  publishing would have to be configured separately for each of the ~18 data
  projects, so a token is simpler.
- Data packages must be published before the npm engine packages, since the
  latter depend on `lingotweaker-data` (the workflow orders the jobs).

Secrets used by the workflow (set in the GitHub repo):

| secret | registry | notes |
|--------|----------|-------|
| `CARGO_REGISTRY_TOKEN` | crates.io | `cargo login` token |
| `NPM_TOKEN` | npm | automation token for `johanneswilm` |
| `NPM_OTP` | npm | optional, only if the token still needs a 2FA code |
| `PYPI_API_TOKEN` | PyPI | uploads the per-language data packages; without it they are only built |

PyPI uses trusted publishing (`id-token: write`) for the `lingotweaker` wheel,
so no PyPI secret is stored for it.