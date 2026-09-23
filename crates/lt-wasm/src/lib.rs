//! WebAssembly bindings for the LingoTweaker engine (`wasm-bindgen`).
//!
//! The engine reads its data from an in-memory [data pack](lt::DataDir::from_pack),
//! so a web page fetches one `.pack` per language and constructs an engine
//! without any file system:
//!
//! ```js
//! import init, { LtEngine } from "./pkg/lt_wasm.js";
//! await init();
//! const pack = new Uint8Array(await (await fetch("/lt-data-gn.pack")).arrayBuffer());
//! const engine = new LtEngine("gn-ES", pack, JSON.stringify({ today: new Date().toISOString() }));
//! const result = JSON.parse(engine.check_json("Mba'éichapa."));
//! ```
//!
//! `today` is required in practice: `wasm32-unknown-unknown` has no clock
//! (`SystemTime::now()` traps), so the browser date is passed in and pinned
//! for the date filters.
//!
//! Build with `wasm-pack build crates/lt-wasm --target web` (or `--target
//! nodejs` for the Node smoke test).

use serde::Deserialize;
use wasm_bindgen::prelude::*;

/// Engine options accepted as JSON by [`LtEngine::new`].
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WasmOptions {
    /// language variant (e.g. `en-GB`); selects the spelling dictionary
    variant: Option<String>,
    /// ISO date (or timestamp) pinned for the date filters
    today: Option<String>,
    /// include `tags="picky"` rules
    picky: bool,
    /// default-off rule ids to compile and enable
    enabled_rules: Vec<String>,
    /// rule ids to disable
    disabled_rules: Vec<String>,
    enabled_categories: Vec<String>,
    disabled_categories: Vec<String>,
    enabled_only: bool,
}

impl WasmOptions {
    fn into_engine(self) -> lt::EngineOptions {
        lt::EngineOptions {
            enabled_rules: self.enabled_rules,
            disabled_rules: self.disabled_rules,
            enabled_categories: self.enabled_categories,
            disabled_categories: self.disabled_categories,
            enabled_only: self.enabled_only,
            picky: self.picky,
        }
    }
}

fn js_error(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}

/// JSON body of [`LtEngine::check_matches_json`].
#[derive(serde::Serialize)]
struct MatchesOnly<'a> {
    matches: &'a [lt::Match],
}

/// Parse `YYYY-MM-DD` (an ISO timestamp may carry a time suffix).
fn parse_ymd(text: &str) -> Option<(i32, u32, u32)> {
    let date = text.split(['T', ' ']).next().unwrap_or(text);
    let mut parts = date.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    Some((year, month, day))
}

#[wasm_bindgen(start)]
pub fn start() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
}

/// An immutable engine loaded from a data pack.
#[wasm_bindgen]
pub struct LtEngine {
    engine: lt::Engine,
    lang: String,
    variant: Option<String>,
}

#[wasm_bindgen]
impl LtEngine {
    /// Build an engine for `lang` (e.g. `en-US`, `gn-ES`) from `pack`.
    ///
    /// `options` is a JSON object; all fields are optional:
    /// `variant` (`en-GB`) selects variant resources, `today` (ISO date or
    /// timestamp) pins the date filters, and `picky`, `enabledRules`,
    /// `disabledRules`, `enabledCategories`, `disabledCategories`,
    /// `enabledOnly` configure rule selection.
    #[wasm_bindgen(constructor)]
    pub fn new(lang: &str, pack: Vec<u8>, options: Option<String>) -> Result<LtEngine, JsError> {
        let code = lt::Lang::from_long_code(lang)
            .ok_or_else(|| JsError::new(&format!("unknown language: {lang}")))?;
        let data = lt::DataDir::from_pack_bytes(pack).map_err(js_error)?;
        let options = match &options {
            Some(json) => serde_json::from_str::<WasmOptions>(json).map_err(js_error)?,
            None => WasmOptions::default(),
        };
        let variant = options.variant.clone();
        let today = options.today.clone();
        let mut builder = lt::Engine::builder(code)
            .map_err(js_error)?
            .data_dir(data)
            .options(options.into_engine());
        if let Some(variant) = &variant {
            builder = builder.variant(variant.clone());
        }
        if let Some(today) = &today {
            let (year, month, day) = parse_ymd(today)
                .ok_or_else(|| JsError::new(&format!("invalid today date: {today}")))?;
            builder = builder.today(year, month, day);
        }
        Ok(LtEngine {
            engine: builder.build().map_err(js_error)?,
            lang: lang.to_string(),
            variant,
        })
    }

    /// Check `text` and return the LT-shaped JSON of `CheckResult`.
    pub fn check_json(&self, text: &str) -> Result<String, JsError> {
        let result = self.engine.check(text).map_err(js_error)?;
        serde_json::to_string(&result).map_err(js_error)
    }

    /// Check `text` and return only the matches as JSON: `{"matches":[…]}`.
    ///
    /// The demo (and any UI that only renders issues) never needs the input
    /// text and per-sentence text spans that [`Self::check_json`] also
    /// serializes; for long paragraphs this JSON round-trip is a measurable
    /// share of the per-check latency.
    pub fn check_matches_json(&self, text: &str) -> Result<String, JsError> {
        let result = self.engine.check(text).map_err(js_error)?;
        serde_json::to_string(&MatchesOnly {
            matches: &result.matches,
        })
        .map_err(js_error)
    }

    /// The language code this engine was built for.
    pub fn lang(&self) -> String {
        self.lang.clone()
    }

    /// The variant override, if any.
    pub fn variant(&self) -> Option<String> {
        self.variant.clone()
    }

    /// Number of rules actively matched by the pattern engine.
    pub fn active_rule_count(&self) -> usize {
        self.engine.active_rule_count()
    }

    /// Rules that failed to compile, as `[{rule, error}, …]` JSON.
    pub fn compile_failures_json(&self) -> Result<String, JsError> {
        let failures: Vec<serde_json::Value> = self
            .engine
            .compile_failures()
            .iter()
            .map(|(rule, error)| serde_json::json!({ "rule": rule, "error": error }))
            .collect();
        serde_json::to_string(&failures).map_err(js_error)
    }
}
