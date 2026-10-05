# API fixtures (pinned Java oracle)

The v2 HTTP contract, captured from the LanguageTool commit pinned in
`upstream.json`, and the test that holds LingoTweaker to it.

- `requests.json` — the request set. One file, read by both the capture and the
  test (`crates/lt-http/tests/api_fixtures.rs`), so the two can never disagree
  about what was asked.
- `responses/` — what the oracle actually said, byte for byte, including its
  `software.commit`. Committed so a capture can be tied to a pin without
  reaching the oracle.
- `expected/` — the same responses normalised into what the contract is allowed
  to assert on. This is what the test compares against.

## Regenerating

```sh
scripts/oracle/run-oracle.sh                     # builds with -Pfat-jar; needs Docker
python3 scripts/oracle/capture-fixtures.py http://localhost:8123
python3 scripts/oracle/normalize-fixtures.py
cargo test -p lt-http --test api_fixtures
```

`run-oracle.sh` prints the port it wants; set `PORT=` when 8080 is taken, and
`SKIP_BUILD=1` to reuse a built jar. The oracle answers
`/v2/info` with its own commit — the test asserts that commit is a prefix of
`upstream.json`'s `baseline_commit`, so a fixture set captured from the wrong
checkout fails here rather than poisoning the comparison.

## What normalisation removes, and why

`normalize-fixtures.py` (Java side) and `normalize` (Rust side) strip the same
fields, and the two lists must stay in step:

- `software.version`, `buildDate`, `commit`, `premium`, `premiumHint`, `status`
  — properties of the build, not of the API.
- `detectedLanguage.source` and `detectedLanguage.confidence` — the first is a
  free-form diagnostic LanguageTool composes from several detectors; the second
  is the detector's own probability, where Java's n-gram detector and our
  fastText models disagree by construction. Which language was detected is the
  contract; how sure each detector was is not. Both replaced by
  `<implementation-defined>` so their presence is still asserted.
- `matches[].replacements` are sorted — suggestion *order* comes from rule
  internals, not from the API. Suggestion *content* is compared.
- Error bodies that are plain text (`.txt` fixtures) are compared verbatim:
  their wording is part of the drop-in contract.

## The allowlist

`crates/lt-http/tests/api_fixtures.rs` carries `ALLOWED`: every known
divergence, as a pattern for the diff line it produces, with the verdict the
divergence policy (D-310) requires — `residue:` when Java is more correct and
the gap should close, `intentional:` when Rust is more correct and the
difference is kept. A *new* difference fails even inside a fixture that already
has allowed ones, so the gate cannot rot silently.
