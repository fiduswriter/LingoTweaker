#!/usr/bin/env python3
"""Capture golden HTTP fixtures from a running Java LT oracle.

Reads the request set from `docs/parity/api-fixtures/requests.json` — the same
file `crates/lt-http/tests/api_fixtures.rs` replays — so the captured responses
and the Rust contract test can never disagree about what was asked.

Raw responses land in `responses/` and are what the oracle actually said.
Normalised ones land in `expected/` and are what the contract test compares
against; run `normalize-fixtures.py` after this.

    scripts/oracle/capture-fixtures.sh [base-url]      # default http://localhost:8123
"""

from __future__ import annotations

import json
import pathlib
import sys
import urllib.error
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "docs" / "parity" / "api-fixtures"
BASE = (sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123").rstrip("/")


def capture(entry: dict) -> None:
    url = f"{BASE}{entry['path']}"
    data = entry.get("body")
    request = urllib.request.Request(url, method=entry.get("method", "GET"))
    if data is not None:
        request.data = data.encode("utf-8")
        request.add_header("content-type", "application/x-www-form-urlencoded")
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            status, body = response.status, response.read()
    except urllib.error.HTTPError as error:
        # A 4xx is a legitimate answer to capture — the error *paths* are half the
        # contract — so it is recorded rather than raised.
        status, body = error.code, error.read()

    suffix = ".json" if body.lstrip()[:1] in (b"{", b"[") else ".txt"
    path = FIXTURES / "responses" / f"{entry['name']}{suffix}"
    path.write_bytes(body if suffix == ".json" else body + b"\n")
    print(f"  {entry['name']:<28} {status} {suffix} ({len(body)} bytes)")


def main() -> int:
    spec = json.loads((FIXTURES / "requests.json").read_text(encoding="utf-8"))
    (FIXTURES / "responses").mkdir(parents=True, exist_ok=True)
    print(f"capturing {len(spec['requests'])} requests from {BASE}")
    for entry in spec["requests"]:
        capture(entry)
    print(f"raw responses -> {FIXTURES.relative_to(ROOT)}/responses/")
    print("now run scripts/oracle/normalize-fixtures.py")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())