//! `lt-py` — Python bindings (PyO3) for LingoTweaker.
//!
//! Thin wrapper over the public Rust API: one immutable [`Engine`] per
//! loaded language, `check()`/`check_json()` releasing the GIL while the
//! engine runs (the engine is `Send + Sync`).
//!
//! Offsets returned by `check()` are UTF-8 byte offsets (the native engine
//! format, like `lt-cli --json` and `/v3`); the `/v2` compatibility layer
//! with UTF-16 offsets lives in `lt-http`.
//!
//! Data discovery: the engine reads the vendored `data/` directory; set
//! `LT_DATA_DIR` (or pass `data_dir=`) when the process is not started from
//! the repository root. Without either, an installed per-language data
//! distribution (`pip install lingotweaker-data-en`, exposing
//! `lingotweaker_data_en.data_dir()`) is used automatically.

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

fn to_py_err(e: lt::CoreError) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

fn parse_lang(lang: &str) -> PyResult<lt::Lang> {
    lt::Lang::from_long_code(lang)
        .ok_or_else(|| PyValueError::new_err(format!("unsupported language: {lang}")))
}

/// The data directory of an installed `lingotweaker-data-<base>` package.
fn installed_data_dir(py: Python<'_>, base: &str) -> Option<String> {
    let module = format!("lingotweaker_data_{base}");
    let module = py.import(&module).ok()?;
    module.call_method0("data_dir").ok()?.extract().ok()
}

/// An immutable, thread-safe checking engine.
///
/// ```python
/// import lt_py
/// engine = lt_py.Engine("en-US")
/// for match in engine.check("I can heard you."):
///     print(match["rule_id"], match["offset"], match["message"])
/// ```
#[pyclass(module = "lt_py", frozen)]
struct Engine {
    inner: lt::Engine,
}

#[pymethods]
impl Engine {
    #[new]
    #[pyo3(signature = (
        lang = "en-US".to_string(),
        *,
        variant = None,
        data_dir = None,
        picky = false,
        enabled_rules = Vec::new(),
        disabled_rules = Vec::new(),
        enabled_categories = Vec::new(),
        disabled_categories = Vec::new(),
        enabled_only = false
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        lang: String,
        variant: Option<String>,
        data_dir: Option<String>,
        picky: bool,
        enabled_rules: Vec<String>,
        disabled_rules: Vec<String>,
        enabled_categories: Vec<String>,
        disabled_categories: Vec<String>,
        enabled_only: bool,
    ) -> PyResult<Self> {
        let lang_enum = parse_lang(&lang)?;
        let mut builder = lt::Engine::builder(lang_enum).map_err(to_py_err)?;
        let data_dir = data_dir.or_else(|| {
            if std::env::var_os("LT_DATA_DIR").is_none() && !std::path::Path::new("data").is_dir() {
                installed_data_dir(py, lang_enum.base_code())
            } else {
                None
            }
        });
        if let Some(dir) = data_dir {
            builder = builder.data_dir(lt::DataDir::new(dir));
        }
        if let Some(variant) = variant {
            builder = builder.variant(variant);
        }
        builder = builder.options(lt::EngineOptions {
            enabled_rules,
            disabled_rules,
            enabled_categories,
            disabled_categories,
            enabled_only,
            picky,
        });
        let engine = builder.build().map_err(to_py_err)?;
        Ok(Self { inner: engine })
    }

    /// Language long code (e.g. `en-US`).
    #[getter]
    fn lang(&self) -> String {
        self.inner.lang().info().long_code.to_string()
    }

    /// Number of actively matched pattern rules.
    #[getter]
    fn rule_count(&self) -> usize {
        self.inner.active_rule_count()
    }

    /// Check `text` and return one dict per match.
    ///
    /// Keys: `rule_id`, `sub_id`, `message`, `short_message`, `offset`,
    /// `length`, `suggestions`, `category_id`, `category_name`, `issue_type`.
    /// Offsets are UTF-8 bytes into `text`.
    fn check<'py>(&self, py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyList>> {
        let result = py.detach(|| self.inner.check(text)).map_err(to_py_err)?;
        let list = PyList::empty(py);
        for m in &result.matches {
            let dict = PyDict::new(py);
            dict.set_item("rule_id", &m.rule_id)?;
            dict.set_item("sub_id", m.sub_id.as_deref())?;
            dict.set_item("message", &m.message)?;
            dict.set_item("short_message", m.short_message.as_deref())?;
            dict.set_item("offset", m.range.start)?;
            dict.set_item("length", m.range.len())?;
            dict.set_item(
                "suggestions",
                m.suggestions
                    .iter()
                    .map(|s| s.value.as_str())
                    .collect::<Vec<_>>(),
            )?;
            dict.set_item("category_id", &m.category_id)?;
            dict.set_item("category_name", &m.category_name)?;
            dict.set_item("issue_type", &m.issue_type)?;
            list.append(dict)?;
        }
        Ok(list)
    }

    /// Check `text` and return the full result as a JSON string (the same
    /// schema as `lt-cli check --json`; UTF-8 byte offsets).
    fn check_json(&self, py: Python<'_>, text: &str) -> PyResult<String> {
        let result = py.detach(|| self.inner.check(text)).map_err(to_py_err)?;
        serde_json::to_string(&result).map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }
}

#[pymodule]
fn lt_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    Ok(())
}
