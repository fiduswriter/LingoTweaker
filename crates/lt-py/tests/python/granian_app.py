"""Minimal ASGI app exposing lt-py's engine, used by the Granian smoke test.

    granian --interface asgi crates/lt-py/tests/python/granian_app.py:app

`GET /check?text=...` → JSON `{"matches": [...], "language": "en-US"}`.
"""

import json
import os

import lt_py

_ENGINE = lt_py.Engine(os.environ.get("LT_PY_LANG", "en-US"))


async def app(scope, receive, send):
    if scope["type"] != "http":
        return
    path = scope.get("path", "/")
    if path == "/health":
        payload = {"status": "ok", "rules": _ENGINE.rule_count}
    elif path == "/check":
        from urllib.parse import parse_qs

        query = parse_qs(scope.get("query_string", b"").decode("utf-8"))
        text = query.get("text", [""])[0]
        matches = _ENGINE.check(text)
        payload = {"language": _ENGINE.lang, "matches": matches}
    else:
        payload = {"error": "not found"}
    body = json.dumps(payload).encode("utf-8")
    await send(
        {
            "type": "http.response.start",
            "status": 200 if "error" not in payload else 404,
            "headers": [
                (b"content-type", b"application/json"),
                (b"content-length", str(len(body)).encode()),
            ],
        }
    )
    await send({"type": "http.response.body", "body": body})
