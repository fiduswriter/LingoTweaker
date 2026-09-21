//! Legacy v2-compatible HTTP API (`ApiV2.java` drop-in, P2.3).
//!
//! The response schema mirrors `RuleMatchesAsJsonSerializer` (plan
//! Appendix A): UTF-16 code-unit offsets, `ContextTools`-style context
//! windows (±40 characters), sentence, rule/category metadata, and the
//! `software`/`warnings`/`language` sections.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use serde_json::{json, Value};

use lt::Engine;
use lt_core::{Lang, Match};

pub const API_VERSION: i32 = 1;
pub const MAX_TEXT_LENGTH: u32 = 60_000;
/// LT `TextChecker.CONTEXT_SIZE`
pub const CONTEXT_SIZE: i32 = 40;
pub const SOFTWARE_NAME: &str = "LingoTweaker";

#[derive(Clone)]
pub struct AppState {
    pub version: String,
    pub build_date: String,
    /// one engine per resolved long code (variant selects the spelling
    /// dictionary); plain `en` is resolved to `en-US` like the LT server
    engines: HashMap<String, Arc<Engine>>,
}

impl AppState {
    /// Build engines for every available variant. English resolves like the
    /// LT server: `en` and `en-US` share the American spelling rule;
    /// `en-GB` uses the British one. German is served as `de-DE` (default),
    /// `de-AT` and `de-CH`.
    pub fn new(version: impl Into<String>, build_date: impl Into<String>) -> Result<Self, String> {
        let mut engines = HashMap::new();
        for (long_code, variant) in [
            ("en-US", Some("en-US")),
            ("en-GB", Some("en-GB")),
            ("de-DE", Some("de-DE")),
            ("de-AT", Some("de-AT")),
            ("de-CH", Some("de-CH")),
            ("pt-PT", Some("pt-PT")),
            ("pt-BR", Some("pt-BR")),
            ("no", None),
            ("nrd", None),
            ("gn", None),
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
                engines.insert(long_code.to_string(), Arc::new(engine));
            }
        }
        if engines.is_empty() {
            return Err("no language pipelines could be built (data directory missing?)".into());
        }
        Ok(Self {
            version: version.into(),
            build_date: build_date.into(),
            engines,
        })
    }

    /// State without engines (validation-only endpoints still work).
    pub fn without_engines(version: impl Into<String>, build_date: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            build_date: build_date.into(),
            engines: HashMap::new(),
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
        .with_state(state)
}

/// Run the server on `addr` (e.g. `0.0.0.0:8081`).
pub async fn serve(addr: &str, state: AppState) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(Arc::new(state))).await
}

async fn languages() -> Json<Value> {
    let langs: Vec<Value> = Lang::ALL
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

/// Build the flat text from the `data` parameter's annotation JSON (LT
/// `ApiV2.getAnnotatedTextFromJson`): `{"text": ...}` parts concatenate,
/// `{"markup": ..., "interpretAs": ...}` contributes the interpreted text.
fn text_from_annotations(json: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| format!("'data' is not valid JSON: {e}"))?;
    let Some(annotation) = value.get("annotation").and_then(|a| a.as_array()) else {
        return Err("Error parsing annotation JSON: Missing 'annotation' element".to_string());
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
        (None, Some(d)) => match text_from_annotations(d) {
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
    // LT server: a language without variant falls back to its default
    // variant (`en` → `en-US`)
    let long_code = if auto || language == "en" {
        "en-US".to_string()
    } else {
        language.to_string()
    };
    let Some(lang) = Lang::from_long_code(&long_code) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            &format!("{long_code} is not a language code known to LingoTweaker."),
        );
    };
    let Some(engine) = state.engines.get(&long_code) else {
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
    let detected = json!({
        "name": name,
        "code": long_code,
        "confidence": 1.0,
        "source": serde_json::Value::Null,
    });

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
            "detectedLanguage": detected,
        },
        "matches": matches,
        "sentenceRanges": [],
        "extendedSentenceRanges": [],
    });
    (StatusCode::OK, Json(response)).into_response()
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
            match AppState::new("0.1.0", "unknown") {
                Ok(s) => Arc::new(s),
                Err(_) => Arc::new(AppState::without_engines("0.1.0", "unknown")),
            }
        }))
    }

    fn has_engines(s: &AppState) -> bool {
        !s.engines.is_empty()
    }

    async fn send(router: Router, method: &str, uri: &str, body: &str) -> (StatusCode, Value) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, value)
    }

    #[tokio::test]
    async fn languages_endpoint_lists_v1_languages() {
        let router = router(state());
        let (status, value) = send(router, "GET", "/v2/languages", "").await;
        assert_eq!(status, StatusCode::OK);
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 16);
        assert_eq!(arr[0]["longCode"], "en-US");
        assert!(arr.iter().any(|l| l["longCode"] == "it"));
        assert!(arr.iter().any(|l| l["longCode"] == "pt"));
        assert!(arr.iter().any(|l| l["longCode"] == "nl"));
        assert!(arr.iter().any(|l| l["longCode"] == "ca"));
        assert!(arr.iter().any(|l| l["longCode"] == "gl"));
        assert!(arr.iter().any(|l| l["longCode"] == "ro"));
        assert!(arr.iter().any(|l| l["longCode"] == "pl"));
        assert!(arr.iter().any(|l| l["longCode"] == "sk"));
        assert!(arr.iter().any(|l| l["longCode"] == "sl"));
        assert!(arr.iter().any(|l| l["longCode"] == "no"));
        assert!(arr.iter().any(|l| l["longCode"] == "nrd"));
        assert!(arr.iter().any(|l| l["longCode"] == "gn"));
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
}
