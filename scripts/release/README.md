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
scripts/release/publish-npm.sh  [--build-only] # napi build, smoke, npm --tag next
scripts/release/verify-crates.sh 0.1.0-alpha.1 # scratch `cargo add` build check
```

Notes:

- `crates.io` rate-limits *new* crates and returns a retry time in the 429
  response. Re-run `publish-crates.sh`; it continues where it stopped.
- PyPI/npm dist-tags: prereleases go to npm `next` so `latest` stays on the
  last stable release. PyPI marks prereleases via the PEP 440 version itself.
- No engine data is bundled in any artifact. Runtime data is supplied via
  `LT_DATA_DIR`; per-language data packs are still an open owner question.

Secrets used by the workflow (set in the GitHub repo):

| secret | registry | notes |
|--------|----------|-------|
| `CARGO_REGISTRY_TOKEN` | crates.io | `cargo login` token |
| `NPM_TOKEN` | npm | automation token for `johanneswilm` |
| `NPM_OTP` | npm | optional, only if the token still needs a 2FA code |

PyPI uses trusted publishing (`id-token: write`), so no PyPI secret is stored.