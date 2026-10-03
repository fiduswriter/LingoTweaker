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
use std::sync::Arc;

use axum::extract::{Query, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use serde_json::{json, Value};

use lt::Engine;
use lt_core::detect::Gates;
use lt_core::{Lang, Match, TextRange};

pub mod detect;

pub const API_VERSION: i32 = 1;
pub const MAX_TEXT_LENGTH: u32 = 60_000;
/// LT `TextChecker.CONTEXT_SIZE`
pub const CONTEXT_SIZE: i32 = 40;
pub const SOFTWARE_NAME: &str = "LingoTweaker";

/// Language `language=auto` falls back to when detection abstains.
///
/// LT `TextChecker.detectLanguageOfString` is called with `fallbackLanguage =
/// null` and turns that into `parseLanguage("en")` (`TextChecker.java:1004`),
/// which is `en-US` in this build — the first variant listed, as everywhere else.
const AUTO_FALLBACK: &str = "en-US";

/// Sentences reported in `extendedSentenceRanges`.
///
/// `sentenceRanges` is complete at any size, but every extended range costs one
/// prediction and `MAX_TEXT_LENGTH` is 60,000 characters, which a caller can
/// fill with 20,000 two-character sentences. 500 covers any real document
/// (~10,000 words); past that the extended ranges stop rather than letting one
/// request spend seconds in the detector.
const MAX_EXTENDED_SENTENCE_RANGES: usize = 500;

#[derive(Clone)]
pub struct AppState {
    pub version: String,
    pub build_date: String,
    /// `Access-Control-Allow-Origin` sent on every response. `Some("*")` by
    /// default so browser clients such as the LanguageTool extension can
    /// reach a server on localhost; `None` sends no CORS header (the Java
    /// server's behaviour without `--allow-origin`).
    pub allow_origin: Option<String>,
    /// One engine per resolved long code (variant selects the spelling
    /// dictionary). Kept in declaration order: language resolution prefers
    /// the earlier variant, so plain `pt` resolves to `pt-PT`, `de` to
    /// `de-DE`, `en` to `en-US`.
    engines: Vec<(String, Arc<Engine>)>,
}

impl AppState {
    /// Build engines for every available variant. English resolves like the
    /// LT server: `en` and `en-US` share the American spelling rule;
    /// `en-GB` uses the British one. German is served as `de-DE` (default),
    /// `de-AT` and `de-CH`.
    pub fn new(
        version: impl Into<String>,
        build_date: impl Into<String>,
        allow_origin: Option<String>,
    ) -> Result<Self, String> {
        let mut engines = Vec::new();
        for (long_code, variant) in [
            ("en-US", Some("en-US")),
            ("en-GB", Some("en-GB")),
            ("de-DE", Some("de-DE")),
            ("de-AT", Some("de-AT")),
            ("de-CH", Some("de-CH")),
            ("de-DE-x-simple-language", Some("de-DE-x-simple-language")),
            ("pt-PT", Some("pt-PT")),
            ("pt-BR", Some("pt-BR")),
            ("no", None),
            ("nrd", None),
            ("nn", None),
            ("gn", None),
            ("ja-JP", None),
            ("zh-CN", None),
        ] {
            let Some(lang) = Lang::from_long_code(long_code) else {
                continue;
            };
            let Ok(builder) = Engine::builder(lang) else {
                continue;
            };
            let builder = match variant {
                Some(v) => builder.variant(v),
                None => builder,
            };
            if let Ok(engine) = builder.build() {
                engines.push((long_code.to_string(), Arc::new(engine)));
            }
        }
        if engines.is_empty() {
            return Err("no language pipelines could be built (data directory missing?)".into());
        }
        Ok(Self {
            version: version.into(),
            build_date: build_date.into(),
            allow_origin,
            engines,
        })
    }

    /// State without engines (validation-only endpoints still work).
    pub fn without_engines(
        version: impl Into<String>,
        build_date: impl Into<String>,
        allow_origin: Option<String>,
    ) -> Self {
        Self {
            version: version.into(),
            build_date: build_date.into(),
            allow_origin,
            engines: Vec::new(),
        }
    }

    /// Resolve a requested language code to an engine entry, LT-style: codes
    /// are case-insensitive (the browser extension sends `en-us`, `de-de`)
    /// and a code without variant falls back to the first listed variant of
    /// that language (`de` → `de-DE`, like `Languages.getLanguageForLanguageCode`
    /// plus the server's default-variant fallback).
    pub fn resolve_engine(&self, language: &str) -> Option<&(String, Arc<Engine>)> {
        let lower = language.to_ascii_lowercase();
        self.engines.iter().find(|(key, _)| {
            let key_lower = key.to_ascii_lowercase();
            key_lower == lower
                || key_lower
                    .strip_prefix(&lower)
                    .is_some_and(|rest| rest.starts_with('-'))
        })
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

    if params.enabled.is_some()
        || params.disabled.is_some()
        || params.preferredvariants.is_some()
        || params.autodetect.is_some()
    {
        return error_response(
            StatusCode::BAD_REQUEST,
            "The 'enabled', 'disabled', 'preferredvariants', 'autodetect' parameters are not supported anymore. Use 'enabledRules', 'disabledRules', 'preferredVariants' or set language to 'auto'.",
        );
    }
    if params.text.is_some() && params.data.is_some() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "You cannot use 'text' and 'data' at the same time.",
        );
    }
    let Some(language) = params.language.as_deref() else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Missing required parameter: language",
        );
    };
    if params.preferred_variants.is_some() && language != "auto" {
        return error_response(
            StatusCode::BAD_REQUEST,
            "The 'preferredVariants' parameter can only be used if 'language' is set to 'auto'.",
        );
    }
    if params.text.is_none() && params.data.is_none() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Missing required parameter: text or data",
        );
    }
    let text = match (&params.text, &params.data) {
        (Some(t), _) => t.clone(),
        (None, Some(d)) => match text_from_data(d) {
            Ok(t) => t,
            Err(e) => return error_response(StatusCode::BAD_REQUEST, &e),
        },
        (None, None) => unreachable!(),
    };
    if text.chars().count() > MAX_TEXT_LENGTH as usize {
        return error_response(
            StatusCode::BAD_REQUEST,
            &format!("Text too long: limit is {MAX_TEXT_LENGTH} characters"),
        );
    }

    let auto = language == "auto";
    let preferred_variants = split_csv(&params.preferred_variants);
    // Detection runs only for `language=auto`. LT also detects for an explicit
    // language — `V2TextChecker.getLanguage` always calls
    // `detectLanguageOfString` and reports the answer under `detectedLanguage`
    // while checking with the language that was asked for — but throws it away
    // here: it costs a model parse and a prediction on every request, and the
    // caller has already said which language it wants.
    let outcome = auto.then(|| detect::language_of(&text, None, &Gates::default()));
    let detected = outcome.as_ref().and_then(|outcome| outcome.detected);

    // The language the check itself runs in. LT
    // `TextChecker.detectLanguageOfString` (`:1002-1031`): detection decides it,
    // `preferredVariants` then only selects the *variant* of that language (it
    // is not a candidate list — that is `preferredLanguages`), and an abstention
    // falls back to `en`.
    let long_code = if auto {
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
        if state.resolve_engine(&requested).is_some() {
            requested
        } else if let Some(fallback) = preferred_variants
            .iter()
            .find_map(|variant| state.resolve_engine(variant).map(|(key, _)| key.clone()))
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
        return error_response(
            StatusCode::BAD_REQUEST,
            &format!("{long_code} is not a language code known to LingoTweaker."),
        );
    };
    let Some((long_code, engine)) = state
        .resolve_engine(&long_code)
        .map(|(key, engine)| (key.clone(), Arc::clone(engine)))
    else {
        return error_response(
            StatusCode::NOT_IMPLEMENTED,
            &format!("The language {long_code} is not supported by this LingoTweaker build yet."),
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
        "extendedSentenceRanges": extended_sentence_ranges(&text, &result, outcome.is_some(), utf8_offsets),
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
    // The language detection settled on, or `en` when the gates abstained.
    let base = outcome.detected.map_or(Lang::En, |hit| hit.language);
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
    detected: bool,
    utf8_offsets: bool,
) -> Vec<Value> {
    if !detected {
        return Vec::new();
    }
    result
        .sentences
        .iter()
        .take(MAX_EXTENDED_SENTENCE_RANGES)
        .map(|sentence| {
            let (from, to) = sentence_span(text, sentence.range, utf8_offsets);
            let languages: Vec<Value> =
                detect::language_of(&sentence.text, None, &Gates::default())
                    .candidates
                    .iter()
                    .map(|candidate| {
                        json!({
                            // LT uses the short code here
                            // (`ExtendedSentenceRange`), not the long code.
                            "language": candidate.lang.base_code(),
                            // LT rounds the rate to two decimals
                            // (`DefaultLanguageIdentifier:335`).
                            "rate": (candidate.confidence * 100.0).round() / 100.0,
                        })
                    })
                    .collect();
            json!({
                "from": from,
                "to": to,
                "detectedLanguages": languages,
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
    let outcome = detect::language_of(&text, restrict.as_deref(), &Gates::default());
    (StatusCode::OK, Json(detect::report_json(&outcome))).into_response()
}

#[derive(Debug, Default, Deserialize)]
struct DetectParams {
    text: Option<String>,
    /// `sv,en,de`: only consider languages the caller has data for.
    restrict: Option<String>,
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
    Json(json!({
        "name": SOFTWARE_NAME,
        "version": state.version,
        "buildDate": state.build_date,
        "apiVersion": API_VERSION,
        "premium": false,
        "status": "",
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
        !s.engines.is_empty()
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
        let (status, value) = send(
            router,
            "POST",
            "/v2/check",
            "text=hi&language=en-US&enabled=FOO",
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(value["error"]["message"].is_string());
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
        let (status, value) = send(router, "POST", "/v2/check", &body).await;
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
            "/v2/check",
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
    async fn explicit_language_bypasses_detection() {
        let s = state();
        if !has_engines(&s) {
            return;
        }
        let router = router(s);
        // Swedish text checked as German: detection would say `sv`, and saying
        // so would be a behaviour change on the path that must not detect.
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
        let detected = &value["language"]["detectedLanguage"];
        assert_eq!(value["language"]["code"], "de-DE");
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
        let best = &languages[0];
        // LT uses short codes here (`ExtendedSentenceRange`), not long codes
        assert_eq!(best["language"], "sv");
        let rate = best["rate"].as_f64().expect("a rate");
        assert!(rate > 0.0 && rate <= 1.0, "rate: {rate}");

        // an explicit language does not detect, so there is nothing to report
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
        assert_eq!(value["extendedSentenceRanges"].as_array().unwrap().len(), 0);
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
