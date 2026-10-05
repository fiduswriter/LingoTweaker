//! The v2 HTTP contract, against the pinned Java oracle.
//!
//! `docs/parity/api-fixtures/requests.json` is the request set, captured from the
//! pinned LanguageTool commit in `upstream.json` by
//! `scripts/oracle/capture-fixtures.sh` and normalised by
//! `scripts/oracle/normalize-fixtures.py`. This replays exactly those requests
//! against `lt-http` and compares.
//!
//! The point is the *error* paths and the wire shape, not grammar quality: the
//! 2,000-example match-set gate (`scripts/oracle/gate.sh`) covers rule output,
//! and this covers what a drop-in client actually depends on — status lines,
//! offsets in UTF-16 code units, the shape of `detectedLanguage`, and the exact
//! wording of the rejections.
//!
//! Divergences are not silently allowed. Each one is either normalised (with the
//! reason in `normalize`), or listed in `ALLOWED` below with the verdict the
//! project's divergence policy requires: fix it when Java is more correct, keep
//! Rust and record it as intentional when Rust is. An empty `ALLOWED` means this
//! side is byte-identical to the oracle on every request in the set.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt;

const FIXTURES: &str = "../../docs/parity/api-fixtures";

/// Divergences from the pinned oracle, each as a *pattern for a reported diff
/// line* plus the verdict the project's divergence policy (D-310) requires.
///
/// - starting with a fixture name (`"check_es."`) matches that fixture's lines
///   only — for divergences that belong to one fixture;
/// - starting with a dot (`".software.name"`) matches that *path on any
///   fixture*, anchored at the `:` before the values, so `.software.name`
///   cannot swallow `software.nameX` or a value that happens to contain the
///   words;
/// - ending with a colon (`"check_legacy_param:"`) matches the error-text
///   lines, which are printed as `<fixture>: error text differs`.
///
/// Either way a diff line is accepted only when some entry matches it, so a
/// *new* difference — including inside a fixture that already has allowed
/// divergences — fails. Entries are written the way the failure output prints
/// them, so adding one is copy-paste.
const ALLOWED: &[(&str, &str)] = &[
    // `/v2/configinfo`: Java returns `software`, `parameter.maxTextLength` and
    // the full rule catalogue; ours returns a different top level and no rules.
    // Residue to fix, and the catalogue is what makes it non-trivial.
    (
        "configinfo.",
        "residue: /v2/configinfo has the wrong shape, no rule list",
    ),
    // The server's identity. Deliberate: this is LingoTweaker, and LT's own
    // clients read the field only for display. Intentional: Rust more correct.
    (".software.name", "intentional: identifies as LingoTweaker"),
    // On the `data` path Java's detector runs even for an explicit language and
    // answers `nl` for the fixture's two-character `Hi`, checked as `en-US`.
    // Ours does not detect for an explicit language at all (owner decision,
    // 2026-10-05: re-guessing a stated language is work whose only output is a
    // field that contradicts the caller), so `detectedLanguage` reports the
    // checked language at 1.0. Intentional: deliberate divergence.
    (
        "check_data.language.detectedLanguage.",
        "intentional: detection does not run for an explicit language; Java always detects",
    ),
    // `/v2/languages`: we list the 43 codes we can actually check; Java lists
    // every module its classpath carries (60, including variants we have no
    // data for). A client asks this endpoint to learn what it can *use*.
    // Intentional: Rust more correct.
    (
        "languages:",
        "intentional: lists the servable languages, not the compiled modules",
    ),
    // `/v2/maxtextlength`: Java advertises Integer.MAX_VALUE and does not
    // enforce it; we advertise 100,000 and do enforce it, so a caller reading
    // the endpoint learns the number that will be honoured. Intentional: Rust
    // more correct.
    ("maxtextlength:", "intentional: states the enforced limit"),
    // `/v2/words` is not implemented; Java's is (premium-backed). Ours names the
    // gap in plain text on v2, Java's wording differs.
    (
        "words_not_implemented:",
        "intentional: /v2/words is not implemented",
    ),
    // The unknown-language error follows Java's sentence — `'xx' is not a
    // language code known to … Supported language codes are: …` — but names our
    // languages (43 servable codes) and points at `/v2/languages` rather than at
    // Java's classpath mechanism. The codes differ for the same reason
    // `/v2/languages` does. Intentional: Rust more correct.
    (
        "check_unknown_language:",
        "intentional: lists our own languages, points at /v2/languages",
    ),
];

/// Fields that describe the build rather than the contract, and `detectedLanguage`
///'s free-form `source`, which LanguageTool composes from several detectors and
/// no client reads (`RemoteLanguageTool` takes only `code` and `name`).
///
/// Kept deliberately identical to `SOFTWARE_FIELDS` and `SOURCE_FIELDS` in
/// `scripts/oracle/normalize-fixtures.py`; the two lists have to agree because one
/// normalises the Java side and this normalises ours. A field added to one and not
/// the other fails the test loudly rather than comparing something meaningless.
const SOFTWARE_FIELDS: &[&str] = &[
    "version",
    "buildDate",
    "commit",
    "premium",
    "premiumHint",
    "status",
];

fn state() -> std::sync::Arc<lt_http::AppState> {
    std::sync::Arc::new(
        lt_http::AppState::new("oracle-fixture", "unknown", Some("*".into()))
            .expect("engines build from the vendored data"),
    )
}

/// Strip what `normalize-fixtures.py` strips, so both sides of the comparison
/// have had the same things removed.
fn normalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, item) in map {
                if key == "software" && item.is_object() {
                    let software: serde_json::Map<String, Value> = item
                        .as_object()
                        .expect("object")
                        .iter()
                        .filter(|(field, _)| !SOFTWARE_FIELDS.contains(&field.as_str()))
                        .map(|(field, value)| (field.clone(), value.clone()))
                        .collect();
                    out.insert(key.clone(), Value::Object(software));
                    continue;
                }
                if key == "source" && map.contains_key("code") && map.contains_key("name") {
                    out.insert(key.clone(), json!("<implementation-defined>"));
                    continue;
                }
                // The detector's own probability — Java's n-gram detector and
                // our fastText models disagree by construction. Kept in step
                // with `CONFIDENCE_FIELDS` in normalize-fixtures.py.
                if key == "confidence" && map.contains_key("code") && map.contains_key("name") {
                    out.insert(key.clone(), json!("<implementation-defined>"));
                    continue;
                }
                out.insert(key.clone(), normalize(item));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(normalize).collect()),
        other => other.clone(),
    }
}

/// Sort every match's `replacements`, so suggestion order is not compared.
fn sort_replacements(response: &mut Value) {
    let Some(matches) = response.get_mut("matches").and_then(Value::as_array_mut) else {
        return;
    };
    for entry in matches {
        let Some(replacements) = entry.get_mut("replacements").and_then(Value::as_array_mut) else {
            continue;
        };
        replacements.sort_by_key(|item| item.to_string());
    }
}

async fn send(router: Router, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
    use axum::body::Body;
    use axum::http::Request;

    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(body.unwrap_or_default().to_string()))
        .expect("request builds");
    let response = router.oneshot(request).await.expect("response");
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024 * 1024)
        .await
        .expect("body");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES)
}

struct Case {
    name: String,
    method: String,
    path: String,
    body: Option<String>,
}

fn cases() -> Vec<Case> {
    let spec: Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures_dir().join("requests.json")).expect("requests.json"),
    )
    .expect("requests.json parses");
    spec["requests"]
        .as_array()
        .expect("requests array")
        .iter()
        .map(|entry| Case {
            name: entry["name"].as_str().expect("name").to_string(),
            method: entry
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("GET")
                .to_string(),
            path: entry["path"].as_str().expect("path").to_string(),
            body: entry
                .get("body")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
        .collect()
}

/// Every path at which two normalised values differ, as `path: ours != java`.
///
/// Reported as a flat list rather than a diff so a failure names every
/// disagreement at once — the interesting cases differ in two or three places,
/// and one at a time is three rebuilds.
fn differences(name: &str, ours: &Value, java: &Value) -> Vec<String> {
    let mut out = Vec::new();
    collect(name, ours, java, &mut out);
    out
}

fn collect(path: &str, ours: &Value, java: &Value, out: &mut Vec<String>) {
    match (ours, java) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: BTreeMap<&str, ()> = BTreeMap::new();
            for key in a.keys().chain(b.keys()) {
                keys.insert(key.as_str(), ());
            }
            for key in keys.into_keys() {
                match (a.get(key), b.get(key)) {
                    (Some(ours), Some(java)) => {
                        collect(&format!("{path}.{key}"), ours, java, out);
                    }
                    (Some(ours), None) => {
                        out.push(format!("{path}.{key}: ours {ours}, absent in Java"))
                    }
                    (None, Some(java)) => {
                        out.push(format!("{path}.{key}: absent in ours, Java {java}"))
                    }
                    (None, None) => unreachable!(),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                out.push(format!("{path}: {} entries, Java {}", a.len(), b.len()));
                return;
            }
            for (index, (ours, java)) in a.iter().zip(b.iter()).enumerate() {
                collect(&format!("{path}[{index}]"), ours, java, out);
            }
        }
        _ => {
            if ours != java {
                out.push(format!("{path}: ours {ours}, Java {java}"));
            }
        }
    }
}

/// Print an allowed divergence so the classification is visible in test output
/// rather than buried in a constant.
fn note(line: &str) {
    println!("{line}");
}

/// Whether an allowlist entry accepts a reported diff line; see `ALLOWED`.
fn allowed(prefix: &str, line: &str) -> bool {
    if let Some(path) = prefix.strip_prefix('.') {
        // A path on any fixture: `<fixture>.<path>: ours … != Java …`
        return line.contains(&format!(".{path}:"));
    }
    if prefix.ends_with(':') {
        // An error-text line: `<fixture>: error text differs`.
        return line.starts_with(prefix);
    }
    // A whole fixture: every line it produces.
    line.starts_with(prefix)
}

#[tokio::test(flavor = "multi_thread")]
async fn lt_http_matches_the_pinned_java_oracle() {
    let dir = fixtures_dir();
    let router = lt_http::router(state());
    let mut checked = 0usize;
    let mut problems: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for case in cases() {
        let (status, body) = send(
            router.clone(),
            &case.method,
            &case.path,
            case.body.as_deref(),
        )
        .await;

        let json_path = dir.join("expected").join(format!("{}.json", case.name));
        let text_path = dir.join("expected").join(format!("{}.txt", case.name));
        if json_path.is_file() {
            let java: Value =
                serde_json::from_str(&std::fs::read_to_string(&json_path).expect("fixture reads"))
                    .expect("fixture parses");
            let mut ours: Value = serde_json::from_str(&body).unwrap_or_else(|error| {
                problems.push(format!("{}: not JSON ({error}): {body}", case.name));
                Value::Null
            });
            sort_replacements(&mut ours);
            let mut java = java;
            sort_replacements(&mut java);
            let diff = differences(&case.name, &normalize(&ours), &normalize(&java));
            for line in diff {
                match ALLOWED.iter().find(|(prefix, _)| allowed(prefix, &line)) {
                    Some((_, verdict)) => note(&format!("  allowed [{verdict}] {line}")),
                    None => problems.push(line),
                }
            }
        } else if text_path.is_file() {
            // Plain-text error bodies: Java's wording is part of the drop-in
            // contract, so it is compared as-is.
            let java = std::fs::read_to_string(&text_path).expect("fixture reads");
            let ours = body.trim_end();
            let java = java.trim_end();
            if ours != java {
                let line = format!(
                    "{}: error text differs (ours: {ours} | Java: {java})",
                    case.name
                );
                match ALLOWED.iter().find(|(prefix, _)| allowed(prefix, &line)) {
                    Some((_, verdict)) => note(&format!(
                        "  allowed [{verdict}] {}: error text differs",
                        case.name
                    )),
                    None => problems.push(line),
                }
            }
        } else {
            skipped.push(case.name.clone());
            continue;
        }
        checked += 1;
        assert!(
            !(status == 200 && case.name.starts_with("check") && body.is_empty()),
            "{}: 200 with an empty body",
            case.name
        );
    }

    assert!(
        skipped.is_empty(),
        "no fixture for {} — re-run scripts/oracle/capture-fixtures.sh",
        skipped.join(", ")
    );
    assert!(
        problems.is_empty(),
        "{} divergence(s) from the pinned Java oracle not covered by the allowlist:\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
    assert!(checked >= 19, "only {checked} fixtures compared");
}

/// The oracle's own `/v2/info` names the commit it was built from, which is how a
/// capture is tied to the pin in `upstream.json`. This test cannot reach the
/// oracle, so it asserts the recorded fixture instead: if the pin is ever moved,
/// the captured responses are stale and this fails until they are recaptured.
#[test]
fn the_captured_fixtures_came_from_the_pinned_commit() {
    let pin: Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../upstream.json"))
            .expect("upstream.json"),
    )
    .expect("upstream.json parses");
    let commit = pin["baseline_commit"].as_str().expect("baseline_commit");
    let response = std::fs::read_to_string(fixtures_dir().join("responses/info.json"))
        .expect("the raw capture is committed too");
    let info: Value = serde_json::from_str(&response).expect("info.json parses");
    let reported = info["software"]["commit"].as_str().expect("commit");
    assert!(
        commit.starts_with(reported),
        "fixtures were captured from commit {reported}, the pin is {commit}"
    );
}
