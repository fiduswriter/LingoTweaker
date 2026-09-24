//! Japanese (`ja`) engine tests.
//!
//! The engine is `JapaneseWordTokenizer` (Lindera) + `JapaneseTagger` +
//! grammar.xml + the generic `DoublePunctuation`/`MultipleWhitespace` rules.
//! The example-coverage test is the differential probe: for every
//! `<example>` it checks whether the engine fires that rule.

use lt::{DataDir, Engine, Grammar, Lang};

fn data_dir() -> Option<DataDir> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    if !path.is_dir() {
        eprintln!("skipping: no vendored data directory found");
        return None;
    }
    // The compiled Lindera dictionary is a generated artifact; skip when absent.
    if !path.join("ja/dictionary").is_dir() {
        eprintln!("skipping: data/ja/dictionary not generated");
        return None;
    }
    Some(DataDir::new(path))
}

fn engine() -> Option<Engine> {
    let data = data_dir()?;
    Some(
        Engine::builder(Lang::Ja)
            .ok()?
            .data_dir(data)
            .build()
            .expect("build ja engine"),
    )
}

/// Rule ids that fire on `text`.
fn match_ids(engine: &Engine, text: &str) -> Vec<String> {
    engine
        .check(text)
        .map(|r| r.matches.iter().map(|m| m.rule_id.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn japanese_engine_loads_and_fires_a_rule() {
    let Some(engine) = engine() else { return };
    assert!(
        engine.active_rule_count() > 700,
        "expected the 735 ja rules"
    );
    assert!(
        engine.compile_failures().is_empty(),
        "compile failures: {:?}",
        engine.compile_failures()
    );

    // `O` (お/を) example from grammar.xml.
    let result = engine.check("名詞お見る").expect("check");
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "O")
        .expect("rule O should fire");
    assert!(
        m.suggestions.iter().any(|s| s.value.contains("を")),
        "expected a を suggestion, got {:?}",
        m.suggestions
    );

    // Correct text: same rule must not fire.
    let ok = engine.check("名詞を見る").expect("check");
    assert!(
        !ok.matches.iter().any(|m| m.rule_id == "O"),
        "rule O should not fire on the corrected text"
    );
}

#[test]
fn japanese_multiple_whitespace_builtin_fires() {
    let Some(engine) = engine() else { return };
    let result = engine.check("テスト  です。").expect("check");
    assert!(
        result
            .matches
            .iter()
            .any(|m| m.rule_id == "WHITESPACE_RULE"),
        "MultipleWhitespace should fire on a repeated space, got {:?}",
        result
            .matches
            .iter()
            .map(|m| m.rule_id.clone())
            .collect::<Vec<_>>()
    );
}

/// The three rules with a second spelling that tokenizes to a different token
/// count are split into `<or>`/`<rulegroup>` variants; both must fire and the
/// corrected forms must not.
#[test]
fn japanese_variant_spellings_covered() {
    let Some(engine) = engine() else { return };
    for (text, rule) in [
        ("デートが待ちどうしい。", "MATIDOUSII"),
        ("デートが待ちどうしく。", "MATIDOUSII"),
        ("読みずらい字", "ZURAI"),
        ("読みずらく字", "ZURAI"),
        ("来れる。", "KURERU"),
        ("これる。", "KURERU"),
    ] {
        let ids = match_ids(&engine, text);
        assert!(
            ids.iter().any(|id| id == rule),
            "{rule} should fire on {text:?}: {ids:?}"
        );
    }
    for (text, rule) in [
        ("待ちどおしい。", "MATIDOUSII"),
        ("待ちどおしく。", "MATIDOUSII"),
        ("づらい字", "ZURAI"),
        ("来られる。", "KURERU"),
    ] {
        let ids = match_ids(&engine, text);
        assert!(
            !ids.iter().any(|id| id == rule),
            "{rule} must not fire on the corrected form {text:?}: {ids:?}"
        );
    }
}

/// Coverage probe: how many of the 735 example sentences actually fire their
/// rule. Not an assertion of parity (there is no Java golden yet) — it prints
/// the number so the rules needing architecture-related changes are visible.
#[test]
fn japanese_example_coverage() {
    let Some(engine) = engine() else { return };
    let data = data_dir().unwrap();
    let grammar = Grammar::load_file(data.grammar_path(Lang::Ja)).expect("load grammar");

    let mut err = 0usize;
    let mut hit = 0usize;
    let mut miss = 0usize;
    let mut correct = 0usize;
    let mut fp = 0usize;
    let mut misses: Vec<(String, String)> = Vec::new();
    let mut fps: Vec<(String, String)> = Vec::new();
    for rule in &grammar.rules {
        for ex in &rule.examples {
            let ids: Vec<String> = engine
                .check(&ex.text)
                .map(|r| r.matches.iter().map(|m| m.rule_id.clone()).collect())
                .unwrap_or_default();
            let fired = ids.iter().any(|id| id == &rule.id);
            if ex.correct {
                correct += 1;
                if fired {
                    fp += 1;
                    if fps.len() < 30 {
                        fps.push((rule.id.clone(), ex.text.clone()));
                    }
                }
            } else {
                err += 1;
                if fired {
                    hit += 1;
                } else {
                    miss += 1;
                    if misses.len() < 100 {
                        misses.push((rule.id.clone(), ex.text.clone()));
                    }
                }
            }
        }
    }
    eprintln!(
        "ja examples: error {err} (hit {hit}, miss {miss}, {:.1}% fired); correct {correct} (false positive {fp})",
        if err > 0 { 100.0 * hit as f64 / err as f64 } else { 0.0 }
    );
    eprintln!("--- first misses ---");
    for (id, text) in &misses {
        eprintln!("MISS {id}: {text:?}");
    }
    eprintln!("--- first false positives ---");
    for (id, text) in &fps {
        eprintln!("FP {id}: {text:?}");
    }
    assert!(err > 700, "expected the 735 error examples");
    // All 45 initial misses were fixed by realigning the affected patterns to
    // Lindera's token boundaries (see docs/differences.md §13).
    assert_eq!(hit, err, "example coverage regressed: {hit}/{err}");
}
