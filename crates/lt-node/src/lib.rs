//! `lt-node` — Node.js bindings (napi-rs) for LingoTweaker.
//!
//! Thin wrapper over the public Rust API: one immutable [`Engine`] per
//! loaded language. Each Node worker thread should create its own engine
//! (locked decision 10); a single engine is still safe to share within a
//! thread because it is immutable.
//!
//! Offsets are UTF-8 byte offsets (the native engine format); the `/v2`
//! compatibility layer with UTF-16 offsets lives in `lt-http`.
//!
//! Data discovery: set `LT_DATA_DIR` (or pass `dataDir`) when the process is
//! not started from the repository root.

use napi::bindgen_prelude::*;
use napi_derive::napi;

fn to_napi_err(e: lt::CoreError) -> Error {
    Error::new(Status::GenericFailure, e.to_string())
}

#[napi(object)]
pub struct EngineOptions {
    /// Language variant (e.g. `en-GB`) selecting the spelling dictionary.
    pub variant: Option<String>,
    /// Data directory override (`LT_DATA_DIR` / `./data` otherwise).
    pub data_dir: Option<String>,
    /// include `tags="picky"` rules (Java `Level.PICKY`).
    pub picky: Option<bool>,
    pub enabled_rules: Option<Vec<String>>,
    pub disabled_rules: Option<Vec<String>>,
    pub enabled_categories: Option<Vec<String>>,
    pub disabled_categories: Option<Vec<String>>,
    pub enabled_only: Option<bool>,
}

/// An immutable, thread-safe checking engine.
#[napi]
pub struct Engine {
    inner: lt::Engine,
}

#[napi]
impl Engine {
    /// `new Engine("en-US", options?)`
    #[napi(constructor)]
    pub fn new(lang: String, options: Option<EngineOptions>) -> Result<Self> {
        let lang_enum = lt::Lang::from_long_code(&lang).ok_or_else(|| {
            Error::new(Status::InvalidArg, format!("unsupported language: {lang}"))
        })?;
        let mut builder = lt::Engine::builder(lang_enum).map_err(to_napi_err)?;
        let options = options.unwrap_or(EngineOptions {
            variant: None,
            data_dir: None,
            picky: None,
            enabled_rules: None,
            disabled_rules: None,
            enabled_categories: None,
            disabled_categories: None,
            enabled_only: None,
        });
        if let Some(dir) = options.data_dir {
            builder = builder.data_dir(lt::DataDir::new(dir));
        }
        if let Some(variant) = options.variant {
            builder = builder.variant(variant);
        }
        builder = builder.options(lt::EngineOptions {
            enabled_rules: options.enabled_rules.unwrap_or_default(),
            disabled_rules: options.disabled_rules.unwrap_or_default(),
            enabled_categories: options.enabled_categories.unwrap_or_default(),
            disabled_categories: options.disabled_categories.unwrap_or_default(),
            enabled_only: options.enabled_only.unwrap_or(false),
            picky: options.picky.unwrap_or(false),
        });
        let inner = builder.build().map_err(to_napi_err)?;
        Ok(Self { inner })
    }

    /// Language long code (e.g. `en-US`).
    #[napi(getter)]
    pub fn lang(&self) -> String {
        self.inner.lang().info().long_code.to_string()
    }

    /// Number of actively matched pattern rules.
    #[napi(getter)]
    pub fn rule_count(&self) -> u32 {
        self.inner.active_rule_count() as u32
    }

    /// Check `text` and return the full result as a JS object (the same
    /// schema as `lt-cli check --json`; UTF-8 byte offsets).
    #[napi]
    pub fn check(&self, text: String) -> Result<serde_json::Value> {
        let result = self.inner.check(&text).map_err(to_napi_err)?;
        serde_json::to_value(&result).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }

    /// Check `text` and return the result as a JSON string.
    #[napi]
    pub fn check_json(&self, text: String) -> Result<String> {
        let result = self.inner.check(&text).map_err(to_napi_err)?;
        serde_json::to_string(&result)
            .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
    }
}
