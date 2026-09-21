# LingoTweaker (Python)

Python bindings for the LingoTweaker proofreading engine (Rust core, PyO3).
The extension module is imported as `lt_py`; the distribution is named
`lingotweaker`.

```python
import lt_py

engine = lt_py.Engine("en-US")
for match in engine.check("I can heard you."):
    print(match["rule_id"], match["suggestions"])
```

This wheel ships **code only**. Language data is installed separately, one PyPI
distribution per language:

```sh
pip install lingotweaker lingotweaker-data-en
```

`lt_py` then finds the data automatically for a matching language:

```python
import lt_py

engine = lt_py.Engine("en-US")
```

Each `lingotweaker-data-<lang>` package exposes its directory as
`lingotweaker_data_<lang>.data_dir()` if you prefer to pass it explicitly
(`lt_py.Engine(..., data_dir=data_dir())`) or to set `LT_DATA_DIR` (which also
accepts a single `.pack`/`.pack.gz` file). Per-language packs and archives are
attached to each [GitHub Release](https://github.com/fiduswriter/LingoTweaker/releases)
as well. See the repository README and `data/README.md` for how the data tree is
produced.

LingoTweaker is an independent project and includes a port of the legacy
LanguageTool proofreading engine and its rule data. Upstream references and
licenses are kept; see `THIRD_PARTY_NOTICES.md` in the repository.

License: LGPL-2.1-or-later.