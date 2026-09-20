"""lt-py smoke and thread-safety tests.

Run against a built extension module:
    scripts/bindings/smoke.sh          # builds and runs everything
or directly:
    cargo build --release -p lt-py
    cp target/release/liblt_py.so /tmp/lt-py/lt_py.so
    PYTHONPATH=/tmp/lt-py python3 crates/lt-py/tests/python/test_smoke.py

Run from the repository root (the engine finds ./data) or set LT_DATA_DIR.
"""

import sys
import threading

import lt_py


def test_basic_check() -> None:
    engine = lt_py.Engine("en-US")
    assert engine.lang == "en-US"
    assert engine.rule_count > 5000, engine.rule_count
    matches = engine.check("I can heard you.")
    assert any(m["rule_id"] == "MD_BASEFORM" for m in matches), matches
    m = next(m for m in matches if m["rule_id"] == "MD_BASEFORM")
    assert m["offset"] == 6 and m["length"] == 5
    assert m["category_id"] == "GRAMMAR", m
    assert isinstance(m["suggestions"], list)


def test_utf8_offsets() -> None:
    # "😀 definately" — the emoji is 4 UTF-8 bytes, 2 UTF-16 units
    engine = lt_py.Engine("en-US")
    text = "😀 definately misspelled"
    matches = engine.check(text)
    spell = next(m for m in matches if m["rule_id"] == "MORFOLOGIK_RULE_EN_US")
    assert spell["offset"] == 5, (spell, text.encode("utf-8"))


def test_picky_and_disabled_rules() -> None:
    default = lt_py.Engine("en-US")
    assert not any(
        m["rule_id"] == "EN_REPEATEDWORDS"
        for m in default.check("The problem is big. Another problem appears.")
    )
    picky = lt_py.Engine("en-US", picky=True)
    assert any(
        m["rule_id"] == "EN_REPEATEDWORDS"
        for m in picky.check("The problem is big. Another problem appears.")
    )
    no_spelling = lt_py.Engine("en-US", disabled_rules=["MORFOLOGIK_RULE_EN_US"])
    assert not any(
        m["rule_id"] == "MORFOLOGIK_RULE_EN_US"
        for m in no_spelling.check("This is definately wrong.")
    )


def test_check_json() -> None:
    import json

    engine = lt_py.Engine("en-US")
    result = json.loads(engine.check_json("I can heard you."))
    assert result["text"] == "I can heard you."
    assert any(m["rule_id"] == "MD_BASEFORM" for m in result["matches"])


def test_gil_released_and_thread_safe() -> None:
    """One engine shared by many threads must be safe and release the GIL."""
    engine = lt_py.Engine("en-US")
    text = "I can heard you. This is definately wrong."
    expected = engine.check(text)
    errors: list[BaseException] = []
    results: list[list] = []

    def worker() -> None:
        try:
            for _ in range(25):
                results.append(engine.check(text))
        except BaseException as e:  # pragma: no cover - failure path
            errors.append(e)

    threads = [threading.Thread(target=worker) for _ in range(4)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert not errors, errors
    assert len(results) == 100
    assert all(r == expected for r in results)


def main() -> int:
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    for test in tests:
        test()
        print(f"ok {test.__name__}")
    print(f"{len(tests)} lt-py tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
