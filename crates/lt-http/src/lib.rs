//! Legacy v2-compatible HTTP API (`ApiV2.java` drop-in, P2.3).
//!
//! The response schema mirrors `RuleMatchesAsJsonSerializer` (plan
//! Appendix A): UTF-16 code-unit offsets, `ContextTools`-style context
//! windows (±40 characters), sentence, rule/category metadata, and the
//! `software`/`warnings`/`language` sections.
//!
//! Language detection is real on both surfaces but shaped differently. v2 has no
//! detection endpoint — LT has none either, it happens inside `POST /v2/check`
//! when `language=auto` — so detection there is reported through
//! `detectedLanguage`, `sentenceRanges` and `extendedSentenceRanges` exactly as
//! `RuleMatchesAsJsonSerializer.java:164-264` does, and no v2 route is added.
//! `/v3/detect` is the native surface: the decision, the ranking behind it and
//! an honest `null` when the gates abstain. See [`detect`] for the layers and
//! for what `source` reports on each.
//!
//! Browser clients such as the official LanguageTool extension hold no host
//! permissions for the server, so every request runs as a CORS-mode `fetch`
//! and fails unless the response carries `Access-Control-Allow-Origin`
//! (LT `ServerTools.setAllowOrigin`, `--allow-origin`). The preflight
//! handler mirrors `ApiV2.handlePreflight`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use axum::extract::{Query, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use serde_json::{json, Value};

use lt::DataDir;
use lt::Engine;
use lt_core::detect::Gates;
use lt_core::{Lang, Match, TextRange};

pub mod detect;

pub const API_VERSION: i32 = 1;

/// Maximum `text=` length, in characters.
///
/// 100,000 rather than LanguageTool's advertised `Integer.MAX_VALUE`, which the
/// Java server does not enforce either — past roughly this size a check stops
/// being interactive (the per-rule work is linear, and `/v2/check` is a
/// synchronous request). Enforcing an advertised limit beats advertising one
/// that is not enforced: a caller reading `/v2/maxtextlength` gets the number
/// that will actually be honoured, and overshooting is a clear 400 rather than
/// a minutes-long hang.
pub const MAX_TEXT_LENGTH: u32 = 100_000;
/// LT `TextChecker.CONTEXT_SIZE`
pub const CONTEXT_SIZE: i32 = 40;
pub const SOFTWARE_NAME: &str = "LingoTweaker";

/// Language `language=auto` falls back to when detection abstains.
///
/// LT `TextChecker.detectLanguageOfString` is called with `fallbackLanguage =
/// null` and turns that into `parseLanguage("en")` (`TextChecker.java:1004`),
/// which is `en-US` in this build — the first variant listed, as everywhere else.
const AUTO_FALLBACK: &str = "en-US";

pub struct AppState {
    pub version: String,
    pub build_date: String,
    /// `Access-Control-Allow-Origin` sent on every response. `Some("*")` by
    /// default so browser clients such as the LanguageTool extension can
    /// reach a server on localhost; `None` sends no CORS header (the Java
    /// server's behaviour without `--allow-origin`).
    pub allow_origin: Option<String>,
    /// Engines built on first use, kept least-recently-used up to
    /// [`MAX_CACHED_ENGINES`].
    ///
    /// Building every vendored language eagerly costs ~5.6 GB RSS (43 engines ×
    /// dictionaries + compiled rule regexes) — measured — and lazy building
    /// alone only defers that: a server that answers one sentence in each
    /// language ends in the same place, because nothing was ever freed. The
    /// cache is therefore bounded. A language evicted is rebuilt on its next
    /// use, at its build cost (well under a second for most, a couple for the
    /// largest); an engine held by a request in flight stays alive through its
    /// `Arc` until that request finishes. Keyed by lowercased long code; variant
    /// resolution therefore goes through [`Lang::info`] rather than list order.
    engines: RwLock<BTreeMap<String, CachedEngine>>,
    /// Languages whose engine failed to build, with the error. A negative cache:
    /// without it a broken language would re-attempt its build on every request
    /// instead of answering 501 immediately.
    failed: RwLock<BTreeMap<String, String>>,
    /// Monotonic clock for the LRU order.
    used: AtomicU64,
}

struct CachedEngine {
    engine: Arc<Engine>,
    used_at: u64,
}

/// How many language engines to keep in memory at once.
///
/// Eight covers a checking session that wanders across a few languages with
/// room to spare, at a worst case near the reference Java server's idle
/// footprint (~1.7 GB with every module loaded); beyond the cap the least
/// recently used engine is dropped and rebuilt on demand. `LT_MAX_ENGINES`
/// overrides it: `0` disables caching entirely (every check pays its build),
/// and a large value approaches the unbounded 5.6 GB.
pub const MAX_CACHED_ENGINES: usize = 8;

/// Spelling variants served in addition to each language's default long code,
/// in their canonical spelling. Only languages whose variant data is vendored
/// are listed; Java also serves variants we have no data for (`fr-CA`,
/// `nl-BE`, `ca-ES-valencia`), and an unbuildable variant would put a 501 in
/// `/v2/languages`, which is worse than not listing it.
const VARIANTS: &[(Lang, &str)] = &[
    (Lang::En, "en-GB"),
    (Lang::De, "de-AT"),
    (Lang::De, "de-CH"),
    (Lang::De, "de-DE-x-simple-language"),
    (Lang::Pt, "pt-BR"),
];

/// Served default variants that differ from `Lang::info`'s long code.
///
/// Portuguese is the case: `info().long_code` is the bare `pt`, but the
/// language's European variant is what a bare `pt` has always resolved to,
/// LT-style (`getLanguageForLanguageCode` plus the server's default-variant
/// fallback), and `pt-PT` is what the old engine list served first.
const DEFAULT_VARIANTS: &[(Lang, &str)] = &[(Lang::Pt, "pt-PT")];

/// The canonical long code for a request, resolving a bare base language to its
/// default variant (`de` → `de-DE`, LT-style) and matching variants
/// case-insensitively, so a caller's `DE-at` is answered as `de-AT` rather than
/// echoed back lowercased. Unknown variant spellings pass through and fail at
/// build time, which answers 501 with the reason.
/// The codes `/v2/languages` serves, comma-separated, for the unknown-language
/// error. Java names its own modules in the same place.
fn supported_codes() -> String {
    let mut codes: Vec<String> = Lang::ALL
        .iter()
        .map(|lang| lang.info().long_code.to_string())
        .collect();
    codes.extend(VARIANTS.iter().map(|(_, code)| code.to_string()));
    // A bare variant language (`de`) is servable too, and Java lists it.
    codes.extend(
        VARIANTS
            .iter()
            .map(|(lang, _)| lang.info().code.to_string())
            .collect::<Vec<_>>(),
    );
    codes.sort();
    codes.dedup();
    codes.join(", ")
}

fn canonical_long_code(code: &str) -> Option<String> {
    let lang = Lang::from_long_code(code)?;
    let default = DEFAULT_VARIANTS
        .iter()
        .find(|(l, _)| *l == lang)
        .map(|(_, code)| (*code).to_string())
        .unwrap_or_else(|| lang.info().long_code.to_string());
    // A bare base code, the long code in any casing, or the default variant in
    // any casing all mean the same engine; a listed variant matches itself.
    if code.eq_ignore_ascii_case(lang.info().code)
        || code.eq_ignore_ascii_case(lang.info().long_code)
        || code.eq_ignore_ascii_case(&default)
    {
        return Some(default);
    }
    for (variant_lang, variant) in VARIANTS {
        if *variant_lang == lang && variant.eq_ignore_ascii_case(code) {
            return Some((*variant).to_string());
        }
    }
    Some(code.to_string())
}

fn cache_cap() -> usize {
    std::env::var("LT_MAX_ENGINES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(MAX_CACHED_ENGINES)
}

impl AppState {
    /// Discover the data directory and nothing else: engines are built on first
    /// use, so server startup stays instant and idle RSS stays at the runtime's
    /// own footprint. The one thing verified up front is that the data is there
    /// at all — without it every check would 501 with a discovery error, which
    /// [`lt_cli serve`] turns into its validation-only fallback.
    ///
    /// Every language this project vendors data for is servable, plus each
    /// spelling variant that data covers. That includes languages the pinned
    /// Java build has no module for — Nordum (ours, constructed) and Lithuanian
    /// (whose upstream module throws on every check) — served under the same API
    /// as everything else.
    pub fn new(
        version: impl Into<String>,
        build_date: impl Into<String>,
        allow_origin: Option<String>,
    ) -> Result<Self, String> {
        DataDir::discover().map_err(|error| {
            format!("no data directory found; set LT_DATA_DIR or run from the repository root ({error})")
        })?;
        Ok(Self {
            version: version.into(),
            build_date: build_date.into(),
            allow_origin,
            engines: RwLock::new(BTreeMap::new()),
            failed: RwLock::new(BTreeMap::new()),
            used: AtomicU64::new(0),
        })
    }

    /// Whether `code` names a language this server can check.
    ///
    /// Cheap on purpose and *not* a statement about engines already built: the
    /// build is deferred to [`AppState::engine`], so existence checks (the
    /// `language=auto` path asking whether a detection result can be honoured)
    /// never trigger a dictionary compile as a side effect.
    pub fn is_servable(&self, code: &str) -> bool {
        Lang::from_long_code(code).is_some()
    }

    /// The engine for `code`, building it on first use.
    ///
    /// `code` may name a base language (`de`, which resolves to the default
    /// variant `de-DE`, LT-style) or a specific one (`de-AT`). Builds happen
    /// outside the lock so concurrent first requests for *different* languages
    /// do not serialise behind each other's compile time; on a cache hit the
    /// LRU clock advances, and when the cache is full the least recently used
    /// engine is dropped before the new one is inserted.
    pub fn engine(&self, code: &str) -> Result<(String, Arc<Engine>), String> {
        let Some(lang) = Lang::from_long_code(code) else {
            return Err(format!(
                "{code} is not a language code known to LingoTweaker."
            ));
        };
        let long_code = canonical_long_code(code)
            .ok_or_else(|| format!("{code} is not a language code known to LingoTweaker."))?;

        {
            let engines = self.engines.read().expect("engines lock");
            if let Some(cached) = engines.get(&long_code) {
                let engine = Arc::clone(&cached.engine);
                drop(engines);
                let stamp = self.used.fetch_add(1, Ordering::Relaxed) + 1;
                self.engines
                    .write()
                    .expect("engines lock")
                    .get_mut(&long_code)
                    .expect("just read it")
                    .used_at = stamp;
                return Ok((long_code, engine));
            }
        }
        if let Some(error) = self.failed.read().expect("failed lock").get(&long_code) {
            return Err(error.clone());
        }

        let mut builder = Engine::builder(lang).map_err(|error| error.to_string())?;
        let default = DEFAULT_VARIANTS
            .iter()
            .find(|(l, _)| *l == lang)
            .map(|(_, code)| (*code).to_string())
            .unwrap_or_else(|| lang.info().long_code.to_string());
        if long_code != default {
            builder = builder.variant(&long_code);
        }
        let built = builder
            .build()
            .map_err(|error| format!("{long_code}: {error}"))?;
        let engine = Arc::new(built);

        let mut engines = self.engines.write().expect("engines lock");
        if engines.len() >= cache_cap().max(1) {
            // Evict the least recently used. With the cap at 0 the map never
            // fills and every check builds its own engine.
            if let Some(evicted) = engines
                .iter()
                .min_by_key(|(_, cached)| cached.used_at)
                .map(|(key, _)| key.clone())
            {
                engines.remove(&evicted);
            }
        }
        let stamp = self.used.fetch_add(1, Ordering::Relaxed) + 1;
        engines.insert(
            long_code.clone(),
            CachedEngine {
                engine: Arc::clone(&engine),
                used_at: stamp,
            },
        );
        Ok((long_code, engine))
    }

    /// The languages that can be checked.
    pub fn servable_languages(&self) -> Vec<Lang> {
        Lang::ALL.to_vec()
    }

    /// State that never builds engines: validation-only endpoints still work,
    /// and every check answers 501 naming the missing data.
    pub fn without_engines(
        version: impl Into<String>,
        build_date: impl Into<String>,
        allow_origin: Option<String>,
    ) -> Self {
        Self {
            version: version.into(),
            build_date: build_date.into(),
            allow_origin,
            engines: RwLock::new(BTreeMap::new()),
            failed: RwLock::new(BTreeMap::new()),
            used: AtomicU64::new(0),
        }
    }
}

/// `/v2/*` is the legacy v2 drop-in API (UTF-16 code-unit offsets,
/// exactly like LT). `/v3/*` is the native API: UTF-8 byte offsets and room
/// for further changes without breaking drop-in clients.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v2/languages", get(languages))
        .route("/v2/check", post(check_v2))
        .route("/v2/info", get(info))
        .route("/v2/maxtextlength", get(max_text_length))
        .route("/v2/configinfo", get(config_info))
        .route("/v2/words", post(words_not_implemented))
        .route("/v2/words/add", post(words_not_implemented))
        .route("/v2/words/delete", post(words_not_implemented))
        .route("/v3/languages", get(languages))
        .route("/v3/check", post(check_v3))
        .route("/v3/info", get(info))
        .route("/v3/maxtextlength", get(max_text_length))
        .route("/v3/configinfo", get(config_info))
        .route("/v3/words", post(words_not_implemented))
        .route("/v3/words/add", post(words_not_implemented))
        .route("/v3/words/delete", post(words_not_implemented))
        .route("/v3/detect", post(detect_language))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, cors_middleware))
}

/// Run the server on `addr` (e.g. `0.0.0.0:8081`).
pub async fn serve(addr: &str, state: AppState) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(Arc::new(state))).await
}

/// Attach the CORS headers browser clients need and answer OPTIONS
/// preflights (`ApiV2.handlePreflight`: 204, `Access-Control-Allow-Methods`,
/// and an echo of the requested headers).
async fn cors_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let preflight = request.method() == Method::OPTIONS;
    let requested_headers = request
        .headers()
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .cloned();
    let mut response = if preflight {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(request).await
    };
    if let Some(origin) = &state.allow_origin {
        let headers = response.headers_mut();
        if let Ok(value) = HeaderValue::from_str(origin) {
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
        }
        if preflight {
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, POST, OPTIONS"),
            );
            if let Some(requested) = requested_headers {
                headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, requested);
            }
        }
    }
    response
}

async fn languages() -> Json<Value> {
    // Alphabetical by English display name (plain byte-wise `str` ordering:
    // deterministic, locale-independent) instead of `Lang::ALL`'s
    // engine-default order. This keeps "Norwegian (Nynorsk)" directly after
    // "Norwegian (Bokmål)" and places "Nordum" before both. `Lang::ALL`
    // itself keeps its order (default-variant semantics); only this
    // presentation is sorted.
    let mut sorted: Vec<Lang> = Lang::ALL.to_vec();
    sorted.sort_by(|a, b| a.info().name.cmp(b.info().name));
    let langs: Vec<Value> = sorted
        .iter()
        .map(|l| {
            json!({
                "name": l.info().name,
                "code": l.info().code,
                "longCode": l.info().long_code,
            })
        })
        .collect();
    Json(Value::Array(langs))
}

#[derive(Debug, Default, Deserialize)]
struct CheckParams {
    text: Option<String>,
    data: Option<String>,
    language: Option<String>,
    #[serde(rename = "preferredVariants")]
    preferred_variants: Option<String>,
    #[serde(rename = "enabledRules")]
    enabled_rules: Option<String>,
    #[serde(rename = "disabledRules")]
    disabled_rules: Option<String>,
    #[serde(rename = "enabledCategories")]
    enabled_categories: Option<String>,
    #[serde(rename = "disabledCategories")]
    disabled_categories: Option<String>,
    #[serde(rename = "enabledOnly")]
    enabled_only: Option<String>,
    /// `level=picky` activates `tags="picky"` rules (LT `Level.PICKY`)
    level: Option<String>,
    #[allow(dead_code)]
    /// accepted but not yet meaningful (v1 has a single engine pass)
    mode: Option<String>,
    // legacy parameters that LT rejects
    enabled: Option<String>,
    disabled: Option<String>,
    preferredvariants: Option<String>,
    autodetect: Option<String>,
}

fn split_csv(value: &Option<String>) -> Vec<String> {
    value
        .as_deref()
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Build the flat text from the `data` parameter, mirroring `ApiV2`: either
/// a top-level `"text"` string with optional `metaData` (`{"text": ...}` —
/// the shape the LanguageTool browser extension sends for every check) or
/// the annotation JSON (`ApiV2.getAnnotatedTextFromJson`): `{"text": ...}`
/// parts concatenate, `{"markup": ..., "interpretAs": ...}` contributes the
/// interpreted text.
fn text_from_data(json: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| format!("'data' is not valid JSON: {e}"))?;
    let text = value.get("text");
    let annotation = value.get("annotation");
    match (text, annotation) {
        (Some(_), Some(_)) => Err(
            "'data' key in JSON requires either 'text' or 'annotation' key, not both".to_string(),
        ),
        (Some(t), None) => Ok(match t {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }),
        (None, Some(annotation)) => {
            let Some(annotation) = annotation.as_array() else {
                return Err(
                    "Error parsing annotation JSON: 'annotation' must be a list".to_string()
                );
            };
            let mut text = String::new();
            for part in annotation {
                if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                    text.push_str(t);
                } else if let Some(markup) = part.get("markup").and_then(|v| v.as_str()) {
                    if let Some(interpret) = part.get("interpretAs").and_then(|v| v.as_str()) {
                        text.push_str(interpret);
                    } else {
                        let _ = markup;
                    }
                } else {
                    return Err(
                        "Error parsing annotation JSON: Elements need to be of type 'text' or 'markup'"
                            .to_string(),
                    );
                }
            }
            Ok(text)
        }
        (None, None) => Err("'data' key in JSON requires 'text' or 'annotation' key".to_string()),
    }
}

async fn check_v2(
    state: State<Arc<AppState>>,
    query: Query<BTreeMap<String, String>>,
    body: String,
) -> Response {
    check_impl(state, query, body, false).await
}

async fn check_v3(
    state: State<Arc<AppState>>,
    query: Query<BTreeMap<String, String>>,
    body: String,
) -> Response {
    check_impl(state, query, body, true).await
}

async fn check_impl(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<BTreeMap<String, String>>,
    body: String,
    utf8_offsets: bool,
) -> Response {
    let params: CheckParams = serde_urlencoded::from_str(&body).unwrap_or_default();

    for legacy in ["enabled", "disabled", "preferredvariants", "autodetect"] {
        // Named in Java's own order, one per response, with Java's wording.
        let present = match legacy {
            "enabled" => params.enabled.is_some(),
            "disabled" => params.disabled.is_some(),
            "preferredvariants" => params.preferredvariants.is_some(),
            _ => params.autodetect.is_some(),
        };
        if present {
            if utf8_offsets {
                return error_response(StatusCode::BAD_REQUEST, &legacy_parameter_error(legacy));
            }
            return error_response_v2(StatusCode::BAD_REQUEST, &legacy_parameter_error(legacy));
        }
    }
    if params.text.is_some() && params.data.is_some() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "You cannot use 'text' and 'data' at the same time.",
        );
    }
    // v2 errors are Java's: `text/plain`, `Error: <message>`, his exact wording
    // (captured from the pinned oracle). v3 is the native surface and keeps
    // `error_response`'s JSON.
    let fail = |status: StatusCode, message: String| {
        if utf8_offsets {
            error_response(status, &message)
        } else {
            error_response_v2(status, &message)
        }
    };
    let Some(language) = params.language.as_deref() else {
        return fail(
            StatusCode::BAD_REQUEST,
            "Missing 'language' parameter, e.g. 'language=en-US' for American English or 'language=fr' for French".into(),
        );
    };
    if params.preferred_variants.is_some() && language != "auto" {
        return fail(
            StatusCode::BAD_REQUEST,
            "The 'preferredVariants' parameter can only be used if 'language' is set to 'auto'."
                .into(),
        );
    }
    if params.text.is_some() && params.data.is_some() {
        return fail(
            StatusCode::BAD_REQUEST,
            "Set only 'text' or 'data' parameter, not both".into(),
        );
    }
    if params.text.is_none() && params.data.is_none() {
        return fail(
            StatusCode::BAD_REQUEST,
            "Missing 'text' or 'data' parameter".into(),
        );
    }
    let text = match (&params.text, &params.data) {
        (Some(t), _) => t.clone(),
        (None, Some(d)) => match text_from_data(d) {
            Ok(t) => t,
            Err(e) => return fail(StatusCode::BAD_REQUEST, e),
        },
        (None, None) => unreachable!(),
    };
    if text.chars().count() > MAX_TEXT_LENGTH as usize {
        return fail(
            StatusCode::BAD_REQUEST,
            format!("Text too long: limit is {MAX_TEXT_LENGTH} characters"),
        );
    }
    // Java rejects unknown `level` values outright
    // (`TextChecker.getLevel`); ours accepted anything as "default", silently.
    // The engine implements two of Java's ten levels, and the message names the
    // two that exist here.
    if let Some(level) = params.level.as_deref() {
        if !level.eq_ignore_ascii_case("default") && !level.eq_ignore_ascii_case("picky") {
            return fail(
                StatusCode::BAD_REQUEST,
                format!(
                    "Unknown value '{level}' for parameter 'level'. Valid values: default, picky"
                ),
            );
        }
    }

    let auto = language == "auto";
    let preferred_variants = split_csv(&params.preferred_variants);
    // Detection runs only for `language=auto` (owner decision, 2026-10-05).
    // LanguageTool always detects, even for an explicit language —
    // `V2TextChecker.getLanguage` calls `detectLanguageOfString` and reports the
    // answer under `detectedLanguage` while checking with the language that was
    // asked for — but re-guessing a language the caller has already stated is
    // work whose only output is a field that contradicts what they said. On an
    // explicit request, `detectedLanguage` therefore reports the checked
    // language at confidence 1.0 with no source, and the fixture gate carries
    // the divergence (`check_data`, where Java answers `nl` for a request
    // checked as `en-US`).
    let outcome = auto.then(|| detect::language_of(&text, None, None));
    let detected = outcome.as_ref().and_then(|outcome| outcome.detected);

    // The language the check itself runs in. LT
    // `TextChecker.detectLanguageOfString` (`:1002-1031`): detection decides it,
    // `preferredVariants` then only selects the *variant* of that language (it
    // is not a candidate list — that is `preferredLanguages`), and an abstention
    // falls back to `en`.
    let long_code = if auto {
        // v3 has no LanguageTool compatibility duty, so it answers the same way
        // for a detected language as for a named one: if this build has no
        // engine for what detection found, say 501 rather than check the text
        // against different rules. Since the engine set became "every vendored
        // language", detection cannot answer a language this server refuses —
        // its labels are mapped through `Lang::from_long_code` — so this is
        // defence in depth rather than a reachable answer.
        if utf8_offsets {
            if let Some(hit) = detected.filter(|hit| !state.is_servable(hit.language.base_code())) {
                return error_response(
                    StatusCode::NOT_IMPLEMENTED,
                    &format!(
                        "The text was detected as {} ({}) but this LingoTweaker build has no rules for it; pass an explicit 'language' to check it anyway.",
                        hit.language.info().name,
                        hit.language.base_code()
                    ),
                );
            }
        }
        for variant in &preferred_variants {
            if !variant.contains('-') {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    &format!(
                        "Invalid format for 'preferredVariants', expected a dash as in 'en-GB': '{variant}'"
                    ),
                );
            }
        }
        let requested = variant_for(
            &preferred_variants,
            detected.map_or(Lang::En, |hit| hit.language),
        );
        if state.is_servable(&requested) {
            requested
        } else if let Some(fallback) = preferred_variants
            .iter()
            .find(|variant| state.is_servable(variant))
            .cloned()
        {
            // This build builds engines for a subset of the languages the
            // identifier can return, so a detection it cannot check falls back
            // instead of failing the whole request; `detectedLanguage` still
            // reports what detection decided, which is the part a client can act
            // on.
            fallback
        } else {
            AUTO_FALLBACK.to_string()
        }
    } else {
        language.to_string()
    };
    let Some(lang) = Lang::from_long_code(&long_code) else {
        return fail(
            StatusCode::BAD_REQUEST,
            format!(
                "'{long_code}' is not a language code known to LingoTweaker. Supported language codes are: {}. The list of languages is what GET /v2/languages reports.",
                supported_codes()
            ),
        );
    };
    let Ok((long_code, engine)) = state.engine(&long_code) else {
        return fail(
            StatusCode::NOT_IMPLEMENTED,
            format!("The language {long_code} is not supported by this LingoTweaker build yet."),
        );
    };

    let options = lt::EngineOptions {
        enabled_rules: split_csv(&params.enabled_rules),
        disabled_rules: split_csv(&params.disabled_rules),
        enabled_categories: split_csv(&params.enabled_categories),
        disabled_categories: split_csv(&params.disabled_categories),
        enabled_only: params
            .enabled_only
            .as_deref()
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        picky: params
            .level
            .as_deref()
            .map(|v| v.eq_ignore_ascii_case("picky"))
            .unwrap_or(false),
    };
    let result = match engine.check_with_options(&text, &options) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let matches: Vec<Value> = result
        .matches
        .iter()
        .map(|m| match_to_json(&text, &result, m, utf8_offsets))
        .collect();

    let name = lang.info().name;
    let detected_json = detected_language_json(&long_code, lang, outcome.as_ref());

    let response = json!({
        "software": {
            "name": SOFTWARE_NAME,
            "version": state.version,
            "buildDate": state.build_date,
            "apiVersion": API_VERSION,
            "premium": false,
            "status": "",
        },
        "warnings": {
            "incompleteResults": false
        },
        "language": {
            "name": name,
            "code": long_code,
            "detectedLanguage": detected_json,
        },
        "matches": matches,
        "sentenceRanges": sentence_ranges(&text, &result, utf8_offsets),
        "extendedSentenceRanges": extended_sentence_ranges(&text, &result, lang, utf8_offsets),
    });
    (StatusCode::OK, Json(response)).into_response()
}

/// LT `V2TextChecker.getLanguage` (`:116-125`) + `TextChecker.detectLanguageOfString`
/// (`:1014-1031`): a `preferredVariants` entry upgrades the detected language
/// when its base code matches.
///
/// The Java loop does not break, so the *last* matching entry wins. When
/// nothing matches the detected language the language keeps its default variant
/// — LT's `getDefaultLanguageVariant()`, which for every language here is what
/// `Lang::info().long_code` already carries.
fn variant_for(preferred_variants: &[String], lang: Lang) -> String {
    let base = lang.base_code();
    preferred_variants
        .iter()
        .rfind(|variant| {
            variant
                .split('-')
                .next()
                .is_some_and(|code| code.eq_ignore_ascii_case(base))
        })
        .cloned()
        .unwrap_or_else(|| lang.info().long_code.to_string())
}

/// The `language.detectedLanguage` object.
///
/// With no detection — every explicit `language=` — this is the language that
/// was checked, reported with `confidence: 1.0` and a null `source`. The null is
/// LT's own value for "no detector produced an answer"
/// (`TextChecker.java:1033`), and it is what keeps this response distinguishable
/// from a real one.
///
/// With detection, the answer and its real confidence. An abstention reports the
/// fallback language at confidence `0.0` with a null source, which is what LT
/// emits in the same situation (`DetectedLanguage(lang, lang, 0f, null)`):
/// saying "nothing cleared the gates" beats a confident-looking 1.0 for a
/// language nobody identified.
fn detected_language_json(
    checked_code: &str,
    checked: Lang,
    outcome: Option<&detect::Outcome>,
) -> Value {
    let Some(outcome) = outcome else {
        return json!({
            "name": checked.info().name,
            "code": checked_code,
            "confidence": 1.0,
            "source": Value::Null,
        });
    };
    // The language detection settled on, or the checked language when the gates
    // abstained — LT's own abstention is `DetectedLanguage(lang, lang, 0f,
    // null)`, and reporting `en` for an abstained German check would be a
    // third answer nobody asked for.
    let base = outcome.detected.map_or(checked, |hit| hit.language);
    let (confidence, source) = match outcome.detected {
        Some(hit) => (hit.confidence, Some(detect::v2_source(hit.source))),
        // An abstention reports the fallback language at confidence `0.0` with a
        // null source, which is what LT emits in the same situation
        // (`DetectedLanguage(lang, lang, 0f, null)`): saying "nothing cleared
        // the gates" beats a confident-looking 1.0 for a language nobody
        // identified.
        None => (0.0, None),
    };
    // `code` follows the convention the rest of the v2 response uses: the long
    // code with its variant, not the base code. When the checked language *is*
    // the detected one, that is the engine's code, so a `preferredVariants`
    // upgrade is visible in both places at once.
    let code = if base == checked {
        checked_code.to_string()
    } else {
        base.info().long_code.to_string()
    };
    json!({
        "name": base.info().name,
        "code": code,
        "confidence": confidence,
        "source": source,
    })
}

/// LT `RuleMatchesAsJsonSerializer.writeSentenceRanges` (`:230-241`): the
/// sentence positions as `[from, to]` pairs, in this surface's offset
/// convention — UTF-16 code units on v2, UTF-8 bytes on v3.
fn sentence_ranges(text: &str, result: &lt_core::CheckResult, utf8_offsets: bool) -> Vec<Value> {
    result
        .sentences
        .iter()
        .map(|sentence| {
            let (from, to) = sentence_span(text, sentence.range, utf8_offsets);
            json!([from, to])
        })
        .collect()
}

/// LT `RuleMatchesAsJsonSerializer.writeExtendedSentenceRanges` (`:243-264`):
/// each sentence with the languages detected for it.
///
/// LT only fills this in multilingual mode, where it is where the per-sentence
/// languages come from; this build has no multilingual mode, so it is filled
/// from the detector on the `language=auto` path — the one that has already run
/// it — and left empty otherwise, which is what LT emits for a single-language
/// check. Each entry costs one prediction, so the list is capped.
fn extended_sentence_ranges(
    text: &str,
    result: &lt_core::CheckResult,
    checked: Lang,
    utf8_offsets: bool,
) -> Vec<Value> {
    // `JLanguageTool.checkAnalyzedSentenceStream:2195`: one range per sentence,
    // carrying the checked language at rate 1.0. The rates change only when a
    // rule match carries `getNewLanguageMatches()` — LanguageTool premium's
    // per-sentence language switching — which no open-source rule populates, so
    // the initial value is the value. Our first cut ran the detector per
    // sentence instead; that produced five fractional rates per sentence where
    // Java produces one 1.0, and it cost a prediction per sentence on texts
    // where the answer is definitionally the language already checked.
    result
        .sentences
        .iter()
        .map(|sentence| {
            let (from, to) = sentence_span(text, sentence.range, utf8_offsets);
            json!({
                "from": from,
                "to": to,
                "detectedLanguages": [
                    {
                        // LT uses the short code here
                        // (`ExtendedSentenceRange`), not the long code.
                        "language": checked.base_code(),
                        "rate": 1.0,
                    }
                ],
            })
        })
        .collect()
}

/// A sentence's span in this surface's offset convention: UTF-16 code units on
/// v2 (`lt::to_utf16_range`, as for every other v2 offset), UTF-8 bytes on v3.
///
/// Leading and trailing whitespace is trimmed, as LT does
/// (`SentenceRange.getRangesFromSentences:58-62`): the engine's sentence spans
/// include the whitespace that follows a sentence, and a range a client
/// highlights should not end in it.
fn sentence_span(text: &str, range: TextRange, utf8_offsets: bool) -> (usize, usize) {
    let slice = &text[range.start..range.end];
    let start = range.start + slice.len() - slice.trim_start().len();
    let end = start + slice.trim().len();
    if utf8_offsets {
        (start, end)
    } else {
        (
            lt::to_utf16_offset(text, start),
            lt::to_utf16_offset(text, end),
        )
    }
}

/// `POST /v3/detect` — the native detection surface.
///
/// LT has no detection endpoint, so v2 must not grow one
/// (`ApiV2.handleRequest` dispatches a fixed list and an unknown path is a 404).
/// Here the decision, the ranking behind it and the honest `null` can be asked
/// for directly, without checking anything.
///
/// `text=<string>  [&restrict=sv,en,de]` — the response shape is the plan's §4:
/// `{"detected": {language, confidence, source}, "candidates": [...],
/// "resolved": …}`.
async fn detect_language(body: String) -> Response {
    let params: DetectParams = serde_urlencoded::from_str(&body).unwrap_or_default();
    let gates = params.gates();
    let Some(text) = params.text else {
        return error_response(StatusCode::BAD_REQUEST, "Missing required parameter: text");
    };
    if text.chars().count() > MAX_TEXT_LENGTH as usize {
        return error_response(
            StatusCode::BAD_REQUEST,
            &format!("Text too long: limit is {MAX_TEXT_LENGTH} characters"),
        );
    }
    // `restrict` narrows the candidates to the languages the caller has data
    // for. Present but naming nothing we ship narrows to nothing, which answers
    // `null` rather than quietly detecting everything — see `language_of`.
    let restrict = params
        .restrict
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| detect::parse_restrict(Some(value)));
    let outcome = detect::language_of(&text, restrict.as_deref(), gates.as_ref());
    (StatusCode::OK, Json(detect::report_json(&outcome))).into_response()
}

#[derive(Debug, Default, Deserialize)]
struct DetectParams {
    text: Option<String>,
    /// `sv,en,de`: only consider languages the caller has data for.
    restrict: Option<String>,
    /// Gate overrides. Each is optional and defaults to the same field's
    /// `Gates::default()` value, so naming none of them is the calibrated
    /// behaviour and naming one leaves the rest alone. These are the same four
    /// knobs `lt_wasm::detect_json` accepts, spelled the same way, because they
    /// are the same settings: a caller that has tuned them for the browser has
    /// no reason to learn a second vocabulary.
    #[serde(rename = "minChars")]
    min_chars: Option<usize>,
    #[serde(rename = "minConfidence")]
    min_confidence: Option<f32>,
    #[serde(rename = "minMargin")]
    min_margin: Option<f32>,
    #[serde(rename = "nonLatinWeight")]
    non_latin_weight: Option<usize>,
}

impl DetectParams {
    /// The requested gates, or `None` if the caller named none of the four.
    fn gates(&self) -> Option<Gates> {
        let defaults = Gates::default();
        let gates = Gates {
            min_chars: self.min_chars.unwrap_or(defaults.min_chars),
            min_confidence: self.min_confidence.unwrap_or(defaults.min_confidence),
            min_margin: self.min_margin.unwrap_or(defaults.min_margin),
            non_latin_weight: self.non_latin_weight.unwrap_or(defaults.non_latin_weight),
        };
        let untouched = gates == defaults;
        (!untouched).then_some(gates)
    }
}

/// LT `ContextTools.getContext`: a ±`CONTEXT_SIZE` character (UTF-16) window
/// around the match, `...` markers at clipped edges, newlines replaced by
/// spaces, and the match position marked (here: reported as `offset`).
fn context_window(text: &str, utf16_from: usize, utf16_to: usize) -> (String, usize, usize) {
    let units: Vec<u16> = text.encode_utf16().collect();
    let len = units.len() as i32;
    let mut start = utf16_from as i32 - CONTEXT_SIZE;
    let mut prefix = "...";
    if start < 0 {
        prefix = "";
        start = 0;
    }
    let mut end = utf16_to as i32 + CONTEXT_SIZE;
    let mut postfix = "...";
    if end > len {
        postfix = "";
        end = len;
    }
    let context_units: Vec<u16> = units[start as usize..end as usize]
        .iter()
        .map(|&u| if u == b'\n' as u16 { b' ' as u16 } else { u })
        .collect();
    let context: String = String::from_utf16_lossy(&context_units);
    let offset = prefix.len() + (utf16_from - start as usize);
    let _ = (postfix, &mut end, &mut start);
    (context, offset, utf16_to - utf16_from)
}

/// Byte-offset variant of `context_window` (UTF-8 mode): ±`CONTEXT_SIZE`
/// characters (code points) around the match.
fn context_window_utf8(text: &str, from: usize, to: usize) -> (String, usize, usize) {
    let char_boundary = |mut pos: usize, backwards: bool| -> usize {
        let mut remaining = CONTEXT_SIZE;
        while remaining > 0 {
            if backwards {
                if pos == 0 {
                    break;
                }
                pos -= 1;
                while !text.is_char_boundary(pos) {
                    pos -= 1;
                }
            } else {
                if pos >= text.len() {
                    break;
                }
                pos += 1;
                while pos < text.len() && !text.is_char_boundary(pos) {
                    pos += 1;
                }
            }
            remaining -= 1;
        }
        pos
    };
    let start = char_boundary(from, true);
    let end = char_boundary(to, false);
    let prefix = if start > 0 { "..." } else { "" };
    let postfix = if end < text.len() { "..." } else { "" };
    let mut context = String::with_capacity(prefix.len() + postfix.len() + end - start);
    context.push_str(prefix);
    context.push_str(&text[start..end].replace('\n', " "));
    context.push_str(postfix);
    (context, prefix.len() + from - start, to - from)
}

fn match_to_json(
    text: &str,
    result: &lt_core::CheckResult,
    m: &Match,
    utf8_offsets: bool,
) -> Value {
    let ((from, to), (context, context_offset, context_length)) = if utf8_offsets {
        (
            (m.range.start, m.range.end),
            context_window_utf8(text, m.range.start, m.range.end),
        )
    } else {
        let (utf16_from, utf16_to) = lt::to_utf16_range(text, m.range);
        (
            (utf16_from, utf16_to),
            context_window(text, utf16_from, utf16_to),
        )
    };
    let sentence = result
        .sentences
        .iter()
        .find(|s| s.range.contains(m.range.start))
        .map(|s| s.text.trim().to_string());

    let mut match_json = json!({
        "message": m.message,
        "replacements": m
            .suggestions
            .iter()
            .map(|s| {
                let mut r = json!({ "value": s.value });
                if let Some(sd) = &s.short_description {
                    r["shortDescription"] = json!(sd);
                }
                r
            })
            .collect::<Vec<Value>>(),
        "offset": from,
        "length": to - from,
        "context": {
            "text": context,
            "offset": context_offset,
            "length": context_length,
        },
        "type": { "typeName": m.match_type },
        "rule": {
            "id": m.specific_rule_id.as_deref().unwrap_or(&m.rule_id),
            "description": m.description,
            "issueType": m.issue_type,
            "category": {
                "id": m.category_id,
                "name": m.category_name,
            },
        },
        "ignoreForIncompleteSentence":
            m.context_for_sure_match == -1 || m.context_for_sure_match > 3,
        "contextForSureMatch": m.context_for_sure_match,
    });
    if let Some(sub_id) = &m.sub_id {
        match_json["rule"]["subId"] = json!(sub_id);
    }
    if let Some(short) = &m.short_message {
        match_json["shortMessage"] = json!(short);
    }
    if let Some(sentence) = sentence {
        match_json["sentence"] = json!(sentence);
    }
    match_json
}

async fn info(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Java's `/v2/info` answers `{"software":{...}}` — the same object its check
    // response sends. Our first cut flattened those fields at the top level,
    // which no LT client could read.
    Json(json!({
        "software": {
            "name": SOFTWARE_NAME,
            "version": state.version,
            "buildDate": state.build_date,
            "premium": false,
        }
    }))
}

async fn max_text_length() -> impl IntoResponse {
    MAX_TEXT_LENGTH.to_string()
}

async fn config_info(Query(params): Query<BTreeMap<String, String>>) -> Response {
    let Some(_language) = params.get("language") else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Missing required parameter: language",
        );
    };
    Json(json!({
        "apiVersion": API_VERSION,
        "allowOriginUrl": null,
        "hiddenRules": [],
        "hiddenFalseFriends": [],
        "premium": false,
    }))
    .into_response()
}

async fn words_not_implemented() -> Response {
    error_response(
        StatusCode::NOT_IMPLEMENTED,
        "The personal word list endpoints are not implemented in LingoTweaker v1 (see docs).",
    )
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({
            "error": { "message": message }
        })),
    )
        .into_response()
}

/// Java-shaped error for the v2 surface: a `text/plain` body of
/// `Error: <message>`. That is what LanguageTool's server answers with, and a
/// drop-in client written against it parses exactly that — JSON with our own
/// wording would be two divergences where one is asked for. v3 keeps
/// [`error_response`]'s JSON: it is the native surface.
fn error_response_v2(status: StatusCode, message: &str) -> Response {
    (
        status,
        [("content-type", "text/plain; charset=utf-8")],
        format!("Error: {message}"),
    )
        .into_response()
}

/// The v2 wording for a retired v1 parameter, as
/// `TextChecker.getParameter` rejects them: one parameter named per response,
/// not all four at once. `autodetect` has its own sentence, because its v2
/// replacement is a value of `language` rather than a renamed parameter.
fn legacy_parameter_error(param: &str) -> String {
    match param {
        "autodetect" => String::from(
            "You specified 'autodetect' but automatic language detection is now activated with 'language=auto' in v2 of the API",
        ),
        "preferredvariants" => String::from(
            "You specified 'preferredvariants' but the parameter is now called 'preferredVariants' (uppercase 'V') in v2 of the API",
        ),
        other => format!(
            "You specified '{other}' but the parameter is now called '{other}Rules' in v2 of the API"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    /// Build the engines once per test binary: engine construction takes
    /// seconds (thousands of rule regexes), so every test shares the state.
    static SHARED_STATE: std::sync::OnceLock<Arc<AppState>> = std::sync::OnceLock::new();

    fn state() -> Arc<AppState> {
        std::sync::Arc::clone(SHARED_STATE.get_or_init(|| {
            match AppState::new("0.1.0", "unknown", Some("*".into())) {
                Ok(s) => Arc::new(s),
                Err(_) => Arc::new(AppState::without_engines(
                    "0.1.0",
                    "unknown",
                    Some("*".into()),
                )),
            }
        }))
    }

    fn has_engines(s: &AppState) -> bool {
        s.engine("en-US").is_ok()
    }

    async fn send(router: Router, method: &str, uri: &str, body: &str) -> (StatusCode, Value) {
        let (status, value, _) = send_with_headers(router, method, uri, body, &[]).await;
        (status, value)
    }

    async fn send_with_headers(
        router: Router,
        method: &str,
        uri: &str,
        body: &str,
        headers: &[(&str, &str)],
    ) -> (StatusCode, Value, axum::http::HeaderMap) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/x-www-form-urlencoded");
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let resp = router
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let response_headers = resp.headers().clone();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, value, response_headers)
    }

    #[tokio::test]
    async fn languages_endpoint_lists_v1_languages() {
        let router = router(state());
        let (status, value) = send(router, "GET", "/v2/languages", "").await;
        assert_eq!(status, StatusCode::OK);
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 38);
        // languages are listed alphabetically by display name, not in
        // `Lang::ALL` order (English (US) remains the `/check` default via
        // the `en`/`auto` fallback, but is no longer listed first)
        let names: Vec<&str> = arr.iter().map(|l| l["name"].as_str().unwrap()).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort_unstable();
        assert_eq!(names, sorted_names);
        assert_eq!(names[0], "Arabic");
        // the Norwegian-area entries stay adjacent, Bokmål before Nynorsk
        let nynorsk = names
            .iter()
            .position(|n| *n == "Norwegian (Nynorsk)")
            .unwrap();
        assert_eq!(names[nynorsk - 1], "Norwegian (Bokmål)");
        assert_eq!(names[nynorsk - 2], "Nordum");
        assert!(arr.iter().any(|l| l["longCode"] == "en-US"));
        assert!(arr.iter().any(|l| l["longCode"] == "it"));
        assert!(arr.iter().any(|l| l["longCode"] == "pt"));
        assert!(arr.iter().any(|l| l["longCode"] == "nl"));
        assert!(arr.iter().any(|l| l["longCode"] == "ca"));
        assert!(arr.iter().any(|l| l["longCode"] == "gl"));
        assert!(arr.iter().any(|l| l["longCode"] == "ro"));
        assert!(arr.iter().any(|l| l["longCode"] == "pl"));
        assert!(arr.iter().any(|l| l["longCode"] == "sk"));
        assert!(arr.iter().any(|l| l["longCode"] == "sl"));
        assert!(arr.iter().any(|l| l["longCode"] == "el"));
        assert!(arr.iter().any(|l| l["longCode"] == "da-DK"));
        assert!(arr.iter().any(|l| l["longCode"] == "sv"));
        assert!(arr.iter().any(|l| l["longCode"] == "is-IS"));
        assert!(arr.iter().any(|l| l["longCode"] == "eo"));
        assert!(arr.iter().any(|l| l["longCode"] == "ast-ES"));
        assert!(arr.iter().any(|l| l["longCode"] == "br-FR"));
        assert!(arr.iter().any(|l| l["longCode"] == "tl-PH"));
        assert!(arr.iter().any(|l| l["longCode"] == "lt-LT"));
        assert!(arr.iter().any(|l| l["longCode"] == "crh-UA"));
        assert!(arr.iter().any(|l| l["longCode"] == "be-BY"));
        assert!(arr.iter().any(|l| l["longCode"] == "ru-RU"));
        assert!(arr.iter().any(|l| l["longCode"] == "uk-UA"));
        assert!(arr.iter().any(|l| l["longCode"] == "sr-RS"));
        assert!(arr.iter().any(|l| l["longCode"] == "no"));
        assert!(arr.iter().any(|l| l["longCode"] == "nrd"));
        assert!(arr.iter().any(|l| l["longCode"] == "nn"));
        assert!(arr.iter().any(|l| l["longCode"] == "gn"));
        assert!(arr.iter().any(|l| l["longCode"] == "ar"));
        assert!(arr.iter().any(|l| l["longCode"] == "fa-IR"));
        assert!(arr.iter().any(|l| l["longCode"] == "km-KH"));
        assert!(arr.iter().any(|l| l["longCode"] == "ml-IN"));
        assert!(arr.iter().any(|l| l["longCode"] == "ta-IN"));
        assert!(arr.iter().any(|l| l["longCode"] == "ja-JP"));
        assert!(arr.iter().any(|l| l["longCode"] == "zh-CN"));
    }

    #[tokio::test]
    async fn check_rejects_legacy_params() {
        let router = router(state());
        // v2 answers in Java's shape: text/plain `Error: …`, which `send`
        // surfaces as a JSON parse failure (Value::Null) — the wording itself is
        // pinned byte-for-byte by the oracle fixture gate.
        let (status, value) = send(
            router.clone(),
            "POST",
            "/v2/check",
            "text=hi&language=en-US&enabled=FOO",
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(value.is_null(), "v2 errors are text/plain: {value}");

        // v3 keeps the JSON envelope and the per-parameter wording.
        let (status, value) = send(
            router,
            "POST",
            "/v3/check",
            "text=hi&language=en-US&enabled=FOO",
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            value["error"]["message"],
            "You specified 'enabled' but the parameter is now called 'enabledRules' in v2 of the API"
        );
    }

    #[tokio::test]
    async fn check_rejects_text_and_data() {
        let router = router(state());
        let (status, _) = send(
            router,
            "POST",
            "/v2/check",
            "text=hi&data=%7B%7D&language=en-US",
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn check_builds_text_from_annotations() {
        let router = router(state());
        let data = serde_json::json!({
            "annotation": [
                {"text": "This is a "},
                {"markup": "<b>", "interpretAs": ""},
                {"text": "testt"},
                {"markup": "</b>", "interpretAs": ""},
                {"text": "."}
            ]
        })
        .to_string();
        let body = format!(
            "{}&language=en-US",
            serde_urlencoded::to_string([("data", data.as_str())]).unwrap()
        );
        let (status, value) = send(router, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        if has_engines(&state()) {
            // "testt" triggers a spelling rule
            let ids: Vec<&str> = value["matches"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m["rule"]["id"].as_str().unwrap())
                .collect();
            assert!(
                ids.iter().any(|i| i.contains("SPELL")
                    || i.contains("TYPOS")
                    || i.contains("MORFOLOGIK")),
                "expected a spelling rule on 'testt', got {ids:?}"
            );
        }
    }

    #[tokio::test]
    async fn check_returns_engine_matches_with_lt_schema() {
        let s = state();
        let router = router(s.clone());
        let (status, value) = send(
            router,
            "POST",
            "/v2/check",
            // ORDINAL_NUMBER_SUFFIX is `tags="picky"`; Java needs level=picky
            "text=This+was+my+1nd+try.&language=en-US&level=picky",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["software"]["apiVersion"], 1);
        assert_eq!(value["language"]["code"], "en-US");
        assert!(value["language"]["detectedLanguage"].is_object());
        if !has_engines(&s) {
            return;
        }
        let matches = value["matches"].as_array().unwrap();
        let ordinal = matches
            .iter()
            .find(|m| m["rule"]["id"] == "ORDINAL_NUMBER_SUFFIX")
            .expect("ORDINAL_NUMBER_SUFFIX should fire");
        // offsets are UTF-16 code units; "1nd" starts at char 12
        assert_eq!(ordinal["offset"], 12);
        assert_eq!(ordinal["length"], 3);
        assert!(ordinal["rule"]["category"]["id"].is_string());
        assert!(ordinal["rule"]["description"].is_string());
        assert!(ordinal["rule"]["issueType"].is_string());
        assert!(ordinal["context"]["text"].is_string());
        assert!(ordinal["context"]["offset"].as_i64().unwrap() >= 0);
        assert!(ordinal["sentence"].is_string());
        assert!(ordinal["ignoreForIncompleteSentence"].is_boolean());
        assert!(ordinal["contextForSureMatch"].is_i64());
        // replacement suggestion "1st"
        let values: Vec<&str> = ordinal["replacements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["value"].as_str().unwrap())
            .collect();
        assert!(values.contains(&"1st"), "suggestions: {values:?}");
    }

    #[tokio::test]
    async fn check_utf16_offsets() {
        let s = state();
        if !has_engines(&s) {
            return;
        }
        // "é" is 2 UTF-8 bytes but 1 UTF-16 unit; the error offset must be
        // reported in UTF-16 units
        let router = router(s);
        let (status, value) = send(
            router,
            "POST",
            "/v2/check",
            "text=%C3%A9%C3%A9%C3%A9+This+was+my+1nd+try.&language=en-US&level=picky",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let ordinal = value["matches"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["rule"]["id"] == "ORDINAL_NUMBER_SUFFIX")
            .expect("rule fires");
        assert_eq!(ordinal["offset"], 16);
    }

    #[tokio::test]
    async fn check_v3_reports_utf8_offsets() {
        let s = state();
        if !has_engines(&s) {
            return;
        }
        // `/v3/check` reports UTF-8 byte offsets (2 per "é")
        let router = router(s);
        let (status, value) = send(
            router,
            "POST",
            "/v3/check",
            "text=%C3%A9%C3%A9%C3%A9+This+was+my+1nd+try.&language=en-US&level=picky",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let ordinal = value["matches"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["rule"]["id"] == "ORDINAL_NUMBER_SUFFIX")
            .expect("rule fires");
        assert_eq!(ordinal["offset"], 19);
    }

    #[tokio::test]
    async fn check_unknown_language_is_400() {
        let router = router(state());
        let (status, _) = send(router, "POST", "/v2/check", "text=hi&language=xx-XX").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn words_endpoints_return_501() {
        let router = router(state());
        let (status, _) = send(router, "POST", "/v2/words", "username=u&apikey=k").await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    }

    /// The LanguageTool browser extension sends every check as
    /// `data={"text": ...}` (never plain `text=`) with lowercase language
    /// codes and extra parameters like `mode`/`textSessionId`.
    #[tokio::test]
    async fn check_accepts_extension_request_shape() {
        let s = state();
        let router = router(s.clone());
        let body = serde_urlencoded::to_string([
            ("data", r#"{"text":"This is a testt sentence."}"#),
            ("textSessionId", "instance-1"),
            ("language", "en-us"),
            ("mode", "textLevelOnly"),
            ("disabledRules", "WHITESPACE_RULE"),
            ("preferredLanguages", "en-us,en-gb"),
            ("level", "picky"),
        ])
        .unwrap();
        let (status, value) =
            send(router, "POST", "/v2/check?c=1&instanceId=instance-1", &body).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        assert_eq!(value["language"]["code"], "en-US");
        if !has_engines(&s) {
            return;
        }
        let ids: Vec<&str> = value["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["rule"]["id"].as_str().unwrap())
            .collect();
        assert!(
            ids.iter()
                .any(|i| i.contains("SPELL") || i.contains("TYPOS") || i.contains("MORFOLOGIK")),
            "expected a spelling rule on 'testt', got {ids:?}"
        );
    }

    #[tokio::test]
    async fn data_rejects_text_and_annotation_together() {
        let router = router(state());
        let data = r#"{"text":"hi","annotation":[{"text":"hi"}]}"#;
        let body = format!(
            "{}&language=en-US",
            serde_urlencoded::to_string([("data", data)]).unwrap()
        );
        let (status, value) = send(router, "POST", "/v3/check", &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            value["error"]["message"],
            "'data' key in JSON requires either 'text' or 'annotation' key, not both"
        );
    }

    #[tokio::test]
    async fn data_requires_text_or_annotation() {
        let router = router(state());
        let (status, value) = send(
            router,
            "POST",
            "/v3/check",
            "data=%7B%7D&language=en-US", // `{}`
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            value["error"]["message"],
            "'data' key in JSON requires 'text' or 'annotation' key"
        );
    }

    /// The extension holds no host permissions, so its `fetch`es fail unless
    /// responses carry `Access-Control-Allow-Origin` and OPTIONS preflights
    /// are answered.
    #[tokio::test]
    async fn cors_headers_and_preflight() {
        let router_languages = router(state());
        let (status, _, headers) = send_with_headers(
            router_languages,
            "GET",
            "/v2/languages",
            "",
            &[(
                "origin",
                "moz-extension://62b5d4ad-6f57-4e42-951b-d6b5e6cf9e55",
            )],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["access-control-allow-origin"], "*");

        let router_preflight = router(state());
        let (status, _, headers) = send_with_headers(
            router_preflight,
            "OPTIONS",
            "/v2/check",
            "",
            &[
                (
                    "origin",
                    "moz-extension://62b5d4ad-6f57-4e42-951b-d6b5e6cf9e55",
                ),
                ("access-control-request-method", "POST"),
                ("access-control-request-headers", "content-type"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(headers["access-control-allow-origin"], "*");
        assert_eq!(
            headers["access-control-allow-methods"],
            "GET, POST, OPTIONS"
        );
        assert_eq!(headers["access-control-allow-headers"], "content-type");
    }

    #[tokio::test]
    async fn language_codes_are_case_insensitive_with_variant_fallback() {
        let s = state();
        if !has_engines(&s) {
            return;
        }
        for (requested, expected) in [
            ("en-us", "en-US"),
            ("en", "en-US"),
            ("de", "de-DE"),
            ("de-de", "de-DE"),
            ("DE-AT", "de-AT"),
            ("pt", "pt-PT"),
            ("pt-pt", "pt-PT"),
        ] {
            let router = router(s.clone());
            let body =
                serde_urlencoded::to_string([("text", "test"), ("language", requested)]).unwrap();
            let (status, value) = send(router, "POST", "/v2/check", &body).await;
            assert_eq!(status, StatusCode::OK, "language {requested}");
            assert_eq!(value["language"]["code"], expected, "language {requested}");
        }
    }

    /// `language=auto` runs real detection; `preferredVariants` then selects
    /// the *variant* of the detected language (LT
    /// `TextChecker.detectLanguageOfString`), and is the tie-break when the
    /// gates abstain on `en`.
    #[tokio::test]
    async fn auto_uses_preferred_variants() {
        let s = state();
        if !has_engines(&s) {
            return;
        }
        // "test" is four characters, so the length gate abstains and the answer
        // falls back to `en` — as in Java, where `preferredVariants` cannot
        // make the detector prefer a language, only pick its variant. The last
        // matching entry wins, because the Java loop does not break.
        let router_preferred = router(s.clone());
        let body = serde_urlencoded::to_string([
            ("text", "test"),
            ("language", "auto"),
            ("preferredVariants", "xx-XX,de-DE,en-US"),
        ])
        .unwrap();
        let (status, value) = send(router_preferred, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["language"]["code"], "en-US");

        // without usable preferred variants the default stays en-US
        let router_default = router(s.clone());
        let body = serde_urlencoded::to_string([("text", "test"), ("language", "auto")]).unwrap();
        let (status, value) = send(router_default, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["language"]["code"], "en-US");

        // A detected language is upgraded to the variant the caller asked for.
        let router_variant = router(s);
        let body = serde_urlencoded::to_string([
            (
                "text",
                "Ich komme nicht nach Hause, weil es schon dunkel geworden ist.",
            ),
            ("language", "auto"),
            ("preferredVariants", "de-CH"),
        ])
        .unwrap();
        let (status, value) = send(router_variant, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["language"]["code"], "de-CH");
        assert_eq!(value["language"]["detectedLanguage"]["code"], "de-CH");
    }

    /// LT `TextChecker.java:1017`: every `preferredVariants` entry must name a
    /// variant, not a bare language.
    #[tokio::test]
    async fn auto_rejects_a_preferred_variant_without_a_dash() {
        let router = router(state());
        let body = serde_urlencoded::to_string([
            ("text", "Hej."),
            ("language", "auto"),
            ("preferredVariants", "de"),
        ])
        .unwrap();
        let (status, value) = send(router, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            value["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("expected a dash")),
            "message: {}",
            value["error"]["message"]
        );
    }

    /// The whole point of the change: `language=auto` identifies the language
    /// instead of guessing, and says so honestly in `detectedLanguage`.
    #[tokio::test]
    async fn auto_detects_the_language_of_the_text() {
        let s = state();
        let router = router(s);
        let body = serde_urlencoded::to_string([
            (
                "text",
                "Jag arbetar inte i dag, men jag kommer hem efter jobbet.",
            ),
            ("language", "auto"),
        ])
        .unwrap();
        let (status, value) = send(router, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        let detected = &value["language"]["detectedLanguage"];
        assert_eq!(detected["name"], "Swedish");
        // the same code convention as the rest of the response: the long code
        // from `/v2/languages`, not the base code
        assert_eq!(detected["code"], "sv");
        let confidence = detected["confidence"].as_f64().expect("a confidence");
        assert!(
            (0.0..1.0).contains(&confidence),
            "a real confidence, not a stub: {confidence}"
        );
        assert!(
            confidence >= 0.9,
            "clear Swedish should be well above the gate: {confidence}"
        );
        // LT's own label for a statistical detector (`DefaultLanguageIdentifier`)
        assert_eq!(detected["source"], "ngram");
    }

    /// Nordum has no label in the statistical model, so the exclusive-word
    /// lexicon decides it — and v2 says which layer answered.
    #[tokio::test]
    async fn auto_detects_nordum_from_the_lexicon() {
        let router = router(state());
        let body = serde_urlencoded::to_string([
            (
                "text",
                "Jei vet at det er viktig å lære språket i dag, og det går bra.",
            ),
            ("language", "auto"),
        ])
        .unwrap();
        let (status, value) = send(router, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        let detected = &value["language"]["detectedLanguage"];
        assert_eq!(detected["code"], "nrd");
        assert_eq!(detected["name"], "Nordum");
        assert_eq!(detected["source"], "lexicon");
        assert_eq!(value["language"]["code"], "nrd");
    }

    /// Short text is not enough evidence. The response says so instead of
    /// claiming a certain answer for a language nobody identified.
    #[tokio::test]
    async fn auto_abstains_on_short_text_and_says_so() {
        let s = state();
        if !has_engines(&s) {
            return;
        }
        let router = router(s);
        let body = serde_urlencoded::to_string([("text", "Hej."), ("language", "auto")]).unwrap();
        let (status, value) = send(router, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        let detected = &value["language"]["detectedLanguage"];
        assert_eq!(detected["confidence"], 0.0);
        assert_eq!(detected["source"], Value::Null);
        // the fallback, which is what LT does too (`parseLanguage("en")`)
        assert_eq!(value["language"]["code"], "en-US");
        assert_eq!(detected["code"], "en-US");
    }

    /// An explicit language must not be second-guessed: no detection, so
    /// `detectedLanguage` keeps the "this is what was asked for" shape with a
    /// null `source`, exactly as before.
    #[tokio::test]
    async fn explicit_language_reports_the_checked_language() {
        let s = state();
        if !has_engines(&s) {
            return;
        }
        let router = router(s);
        // Swedish text, checked as German: detection does not run, because the
        // caller has already said which language it wants (owner decision,
        // 2026-10-05). `detectedLanguage` therefore reports the checked language
        // at 1.0 with no source — "certain, because you specified it" — rather
        // than a guess that contradicts the caller.
        let body = serde_urlencoded::to_string([
            (
                "text",
                "Jag arbetar inte i dag, men jag kommer hem efter jobbet.",
            ),
            ("language", "de-DE"),
        ])
        .unwrap();
        let (status, value) = send(router, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        assert_eq!(value["language"]["code"], "de-DE");
        let detected = &value["language"]["detectedLanguage"];
        assert_eq!(detected["code"], "de-DE");
        assert_eq!(detected["name"], "German (Germany)");
        assert_eq!(detected["confidence"], 1.0);
        assert_eq!(detected["source"], Value::Null);
    }
    /// `sentenceRanges` (`RuleMatchesAsJsonSerializer:230-241`) and
    /// `extendedSentenceRanges` (`:243-264`) were always empty arrays; both are
    /// filled now, in the offset convention of each surface.
    #[tokio::test]
    async fn sentence_ranges_are_populated_in_utf16_on_v2() {
        let s = state();
        if !has_engines(&s) {
            return;
        }
        let router = router(s);
        // the emoji is 4 UTF-8 bytes but 2 UTF-16 code units
        let body = serde_urlencoded::to_string([
            ("text", "This is a testt. 😀 And a second sentence."),
            ("language", "en-US"),
        ])
        .unwrap();
        let (status, value) = send(router, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK);
        let ranges: Vec<(u64, u64)> = value["sentenceRanges"]
            .as_array()
            .expect("sentenceRanges is an array")
            .iter()
            .map(|range| {
                let pair = range.as_array().expect("a [from, to] pair");
                (
                    pair[0].as_u64().expect("from"),
                    pair[1].as_u64().expect("to"),
                )
            })
            .collect();
        assert_eq!(ranges.len(), 2, "ranges: {ranges:?}");
        // LT trims the whitespace out of the range, so the two ranges skip the
        // space at 16: 16 units of "This is a testt.", then the rest.
        assert_eq!(ranges[0], (0, 16));
        assert_eq!(ranges[1].0, 17);
        assert_eq!(
            ranges[1].1,
            17 + "😀 And a second sentence.".encode_utf16().count() as u64
        );
    }

    #[tokio::test]
    async fn extended_sentence_ranges_report_per_sentence_languages() {
        let s = state();
        if !has_engines(&s) {
            return;
        }
        let router_auto = router(s.clone());
        let body = serde_urlencoded::to_string([
            (
                "text",
                "Jag arbetar inte i dag, men jag kommer hem efter jobbet.",
            ),
            ("language", "auto"),
        ])
        .unwrap();
        let (status, value) = send(router_auto, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        let extended = value["extendedSentenceRanges"]
            .as_array()
            .expect("an array");
        assert_eq!(extended.len(), 1, "ranges: {extended:?}");
        let range = &extended[0];
        assert_eq!(range["from"], 0);
        assert_eq!(
            range["to"],
            "Jag arbetar inte i dag, men jag kommer hem efter jobbet."
                .chars()
                .count() as u64
        );
        let languages = range["detectedLanguages"]
            .as_array()
            .expect("an array of {language, rate}");
        // One entry per sentence: the checked language at rate 1.0, exactly as
        // `JLanguageTool:2195` initialises them — the per-sentence detector this
        // test used to assert on was our own invention, not Java's.
        assert_eq!(languages.len(), 1, "ranges: {languages:?}");
        assert_eq!(languages[0]["language"], "sv");
        assert_eq!(languages[0]["rate"], 1.0);

        // An explicit language reports *itself* at 1.0 — the ranges carry the
        // checked language, whether it came from detection or from the caller.
        let router_explicit = router(s);
        let body = serde_urlencoded::to_string([
            (
                "text",
                "Jag arbetar inte i dag, men jag kommer hem efter jobbet.",
            ),
            ("language", "en-US"),
        ])
        .unwrap();
        let (status, value) = send(router_explicit, "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK);
        let extended = value["extendedSentenceRanges"].as_array().unwrap();
        assert_eq!(extended.len(), 1);
        assert_eq!(extended[0]["detectedLanguages"][0]["language"], "en");
        assert_eq!(extended[0]["detectedLanguages"][0]["rate"], 1.0);
    }

    /// `/v3/check` reports UTF-8 byte offsets, so the sentence ranges differ
    /// from v2's for the same text.
    #[tokio::test]
    async fn sentence_ranges_on_v3_are_utf8_bytes() {
        let s = state();
        if !has_engines(&s) {
            return;
        }
        let router = router(s);
        let body = serde_urlencoded::to_string([
            ("text", "This is a testt. 😀 And a second sentence."),
            ("language", "en-US"),
        ])
        .unwrap();
        let (status, value) = send(router, "POST", "/v3/check", &body).await;
        assert_eq!(status, StatusCode::OK);
        let ranges = value["sentenceRanges"].as_array().unwrap();
        assert_eq!(ranges[0][0], 0);
        assert_eq!(ranges[0][1], 16);
        assert_eq!(ranges[1][0], 17);
        // the emoji is 4 UTF-8 bytes here and 2 UTF-16 code units on v2
        assert_eq!(ranges[1][1], 17 + "😀 And a second sentence.".len() as u64);
    }

    /// `POST /v3/detect` — the decision, the ranking behind it, and an honest
    /// `null` when nothing clears the gates.
    #[tokio::test]
    async fn v3_detect_returns_the_decision_and_the_ranking() {
        let router = router(state());
        let body = serde_urlencoded::to_string([(
            "text",
            "Jag arbetar inte i dag, men jag kommer hem efter jobbet.",
        )])
        .unwrap();
        let (status, value) = send(router, "POST", "/v3/detect", &body).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        assert_eq!(value["resolved"], "sv");
        assert_eq!(value["detected"]["language"], "sv");
        assert_eq!(value["detected"]["source"], "fasttext");
        let confidence = value["detected"]["confidence"].as_f64().unwrap();
        assert!((0.0..1.0).contains(&confidence), "confidence: {confidence}");
        let candidates = value["candidates"].as_array().unwrap();
        assert!(!candidates.is_empty());
        assert_eq!(candidates[0]["language"], "sv");
        assert_eq!(candidates[0]["source"], "fasttext");
        assert!(candidates.iter().all(|c| c["confidence"].is_number()));
    }

    #[tokio::test]
    async fn v3_detect_abstains_on_short_text() {
        let router = router(state());
        let body = serde_urlencoded::to_string([("text", "Hej.")]).unwrap();
        let (status, value) = send(router, "POST", "/v3/detect", &body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["resolved"], Value::Null);
        assert_eq!(value["detected"], Value::Null);
        // the ranking survives an abstention, so a UI can still offer it
        assert!(!value["candidates"].as_array().unwrap().is_empty());
    }

    /// The gate overrides reach the engine, and naming none of them is the
    /// calibrated behaviour. `minChars` is the clearest probe: `Hej.` is four
    /// characters, so the default length gate abstains on it and a lowered one
    /// does not.
    #[tokio::test]
    async fn v3_detect_accepts_gate_overrides() {
        let router = router(state());
        let short = serde_urlencoded::to_string([("text", "Hej.")]).unwrap();
        let (status, value) = send(router.clone(), "POST", "/v3/detect", &short).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["resolved"], Value::Null, "default gates abstain");

        let lenient = serde_urlencoded::to_string([
            ("text", "Hej."),
            ("minChars", "1"),
            ("minConfidence", "0.1"),
        ])
        .unwrap();
        let (status, value) = send(router, "POST", "/v3/detect", &lenient).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        assert!(
            value["resolved"].is_string(),
            "lowered gates should decide: {value}"
        );
    }

    /// A gate named with the value it already has is not an override, and the
    /// other three keep their defaults when one is named.
    #[tokio::test]
    async fn v3_detect_gate_overrides_default_per_field() {
        let defaults = Gates::default();
        let params = DetectParams {
            text: None,
            restrict: None,
            min_chars: None,
            min_confidence: Some(0.5),
            min_margin: None,
            non_latin_weight: None,
        };
        let gates = params.gates().expect("one field was named");
        assert_eq!(gates.min_confidence, 0.5);
        assert_eq!(gates.min_chars, defaults.min_chars);
        assert_eq!(gates.min_margin, defaults.min_margin);
        assert_eq!(gates.non_latin_weight, defaults.non_latin_weight);

        let untouched = DetectParams {
            text: None,
            restrict: None,
            min_chars: Some(defaults.min_chars),
            min_confidence: None,
            min_margin: None,
            non_latin_weight: Some(defaults.non_latin_weight),
        };
        assert!(
            untouched.gates().is_none(),
            "naming defaults is not an override"
        );
    }

    /// `language=auto` checks the language it detected — there is no substitute
    /// step any more, because every language detection can answer is one this
    /// server serves. This is the test that used to pin the 14-engine fallback,
    /// where Swedish text was *reported* as Swedish and checked as English; that
    /// divergence is gone by construction, and this pins its absence.
    #[tokio::test]
    async fn auto_checks_the_language_it_detected() {
        let router = router(state());
        let body = serde_urlencoded::to_string([
            (
                "text",
                "Jag arbetar inte i dag, men jag kommer hem efter jobbet.",
            ),
            ("language", "auto"),
        ])
        .unwrap();
        let (status, value) = send(router.clone(), "POST", "/v2/check", &body).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        assert_eq!(value["language"]["code"], "sv", "checked as detected");
        assert_eq!(value["language"]["detectedLanguage"]["code"], "sv");

        // The same request on v3, which has no compatibility duty: same answer.
        let (status, value) = send(router, "POST", "/v3/check", &body).await;
        assert_eq!(status, StatusCode::OK, "response: {value}");
        assert_eq!(value["language"]["code"], "sv");
    }

    /// `&restrict=` narrows detection to the languages the caller has data for.
    #[tokio::test]
    async fn v3_detect_honours_restrict() {
        let router = router(state());
        let body = serde_urlencoded::to_string([
            (
                "text",
                "Jag arbetar inte i dag, men jag kommer hem efter jobbet.",
            ),
            ("restrict", "en,de"),
        ])
        .unwrap();
        let (status, value) = send(router, "POST", "/v3/detect", &body).await;
        assert_eq!(status, StatusCode::OK);
        // Swedish is no longer on the table, so nothing clears the gates
        assert_eq!(value["resolved"], Value::Null);
        assert_eq!(value["detected"], Value::Null);
        let languages: Vec<&str> = value["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["language"].as_str().unwrap())
            .collect();
        assert!(!languages.contains(&"sv"), "candidates: {languages:?}");
        assert!(
            languages.iter().all(|l| *l == "en" || *l == "de"),
            "{languages:?}"
        );
    }

    /// A `restrict` naming nothing this build ships narrows to nothing and
    /// answers `null`, rather than quietly detecting everything.
    #[tokio::test]
    async fn v3_detect_restrict_to_unknown_codes_answers_nothing() {
        let router = router(state());
        let body = serde_urlencoded::to_string([
            (
                "text",
                "Jag arbetar inte i dag, men jag kommer hem efter jobbet.",
            ),
            ("restrict", "xx,yy-ZZ"),
        ])
        .unwrap();
        let (status, value) = send(router, "POST", "/v3/detect", &body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["resolved"], Value::Null);
        assert!(value["candidates"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn v3_detect_requires_text_and_bounds_its_length() {
        let (status, value) = send(router(state()), "POST", "/v3/detect", "language=sv").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            value["error"]["message"],
            "Missing required parameter: text"
        );

        let router = router(state());
        let long = "a".repeat(MAX_TEXT_LENGTH as usize + 1);
        let body = serde_urlencoded::to_string([("text", long.as_str())]).unwrap();
        let (status, value) = send(router, "POST", "/v3/detect", &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            value["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("Text too long")),
            "message: {}",
            value["error"]["message"]
        );
    }

    /// LT has no detection endpoint, so v2 must not grow one: `ApiV2` answers an
    /// unknown path with a 404 and a drop-in client relies on that.
    #[tokio::test]
    async fn v2_has_no_detect_route() {
        let router = router(state());
        let (status, _) = send(router, "POST", "/v2/detect", "text=Hej.").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn allow_origin_none_disables_cors_headers() {
        let state = Arc::new(AppState::without_engines("0.1.0", "unknown", None));
        let router = router(state);
        let (status, _, headers) = send_with_headers(
            router,
            "GET",
            "/v2/languages",
            "",
            &[(
                "origin",
                "moz-extension://62b5d4ad-6f57-4e42-951b-d6b5e6cf9e55",
            )],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers.get("access-control-allow-origin").is_none());
    }
}
