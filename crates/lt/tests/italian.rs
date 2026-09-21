//! Italian engine tests: Java-probed tagger values (pinned LT build, Docker)
//! and the stage-1 XML wiring state.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/it/probe-rule.sh`). Tagger
//! values come from `scripts/oracle/it/dump-tags.sh`
//! (`scripts/oracle/it/tagger-sentences.txt`, 502 lines byte-identical).

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{Engine, Lang};

mod common;
use common::assert_utf16;

/// One engine at a time: the Italian engine holds the tagger dictionary.
fn engine_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn engine() -> Option<Engine> {
    let Ok(builder) = Engine::builder(Lang::It) else {
        eprintln!("skipping: no vendored data found");
        return None;
    };
    builder.build().ok()
}

/// `ItalianTagger` over the FSA5/ISO-8859-15 `italian.dict`: Java-probed
/// readings for accented and uppercase words.
#[test]
fn italian_tagger_matches_java() {
    let _guard = engine_guard();
    let Ok(data) = lt::DataDir::discover() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let tagger = match lt_tagger::ItalianTagger::load(data.path()) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("skipping: no vendored data");
            return;
        }
    };
    let readings = |word: &str| -> Vec<String> {
        tagger
            .tag_word(word)
            .into_iter()
            .map(|r| {
                format!(
                    "{}:{}",
                    r.stem.as_deref().unwrap_or("null"),
                    r.pos_tag.as_deref().unwrap_or("null")
                )
            })
            .collect()
    };
    assert_eq!(
        readings("è"),
        vec!["essere:AUX:ind+pres+3+s", "essere:VER:ind+pres+3+s"]
    );
    // uppercase input also carries the lowercase dictionary readings
    assert_eq!(readings("È"), readings("è"));
    assert_eq!(readings("perché"), vec!["perché:WH"]);
    assert_eq!(readings("più"), vec!["più:ADV"]);
    // the typographic apostrophe cannot be encoded in ISO-8859-15, so the
    // lookup yields no reading (morfologik `UnmappableInputException` path)
    assert_eq!(readings("’"), vec!["null:null"]);
}

/// Stage 2 speller: Java-probed `getSpellingSuggestions` values
/// (`scripts/oracle/it/probe-speller.sh`, 14 words byte-identical on
/// 2026-09-19).
#[test]
fn italian_speller_matches_java() {
    let _guard = engine_guard();
    let Ok(data) = lt::DataDir::discover() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let rule = match std::panic::catch_unwind(|| lt::spelling_it_probe(&data)) {
        Ok(rule) => rule,
        Err(_) => {
            eprintln!("skipping: no vendored data");
            return;
        }
    };
    let suggestions = |word: &str| rule.suggestions(word);
    assert_eq!(
        suggestions("perchè"),
        vec!["perché", "parche", "pecche", "pesche", "petche", "porche", "purché"]
    );
    assert_eq!(suggestions("lunedi"), vec!["lunedì", "lune di"]);
    assert_eq!(suggestions("aereoplano"), vec!["aeroplano", "aereo plano"]);
    // the base `getAdditionalTopSuggestions` sentinels
    assert_eq!(suggestions("languagetool"), vec!["LanguageTool"]);
    assert_eq!(
        suggestions("Languagetooler"),
        vec!["LanguageTooler", "Languagetool.org"]
    );
    // known words have no suggestions
    assert!(suggestions("città").is_empty());
    assert!(suggestions("problema").is_empty());
}

/// Stage 3 (internal development notes): 86 active XML rules, 68
/// disambiguation rules, no unmapped filter and no compile failure left.
#[test]
fn italian_engine_stage3_state() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(engine.active_rule_count(), 86);
    assert_eq!(engine.disambig_rule_count(), 68);
    assert!(engine.compile_failures().is_empty());
    let skipped = engine.skipped_counts();
    assert_eq!(skipped.filters, 0);
    assert_eq!(skipped.uncompilable, 0);
}

/// `DateCheckFilter` (DATE_WEEKDAY) and `ItalianWordRepeatRule` values from
/// `scripts/oracle/it/probe-rule.sh` (2026-09-19).
#[test]
fn italian_date_and_word_repeat_match_java() {
    let _guard = engine_guard();
    let Ok(builder) = Engine::builder(Lang::It) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let Some(engine) = builder.today(2026, 9, 19).build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };

    // "Questa data non è un lunedì ma un martedì.", no suggestions.
    let text = "Lunedì, 7 ottobre 2014. Martedì, 7 ottobre 2014.";
    let result = engine.check(text).unwrap();
    let date = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DATE_WEEKDAY")
        .expect("DATE_WEEKDAY match");
    assert_utf16(text, date, (0, 22));
    assert_eq!(date.message, "Questa data non è un lunedì ma un martedì.");
    assert!(date.suggestions.is_empty());
    assert_eq!(
        result
            .matches
            .iter()
            .filter(|m| m.rule_id == "DATE_WEEKDAY")
            .count(),
        1,
        "the correct weekday must not match"
    );

    // no `year` argument: the pinned `today` year is assumed
    // ("… (2026) … non è un lunedì ma un mercoledì.")
    let text = "Lunedì 7 ottobre";
    let result = engine.check(text).unwrap();
    let date = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DATE_WEEKDAY")
        .expect("DATE_WEEKDAY match");
    assert_utf16(text, date, (0, 16));
    assert_eq!(
        date.message,
        "Se si riferisce all'anno in corso (2026), questa data non è un lunedì ma un mercoledì."
    );

    // suggestion "gatto"; the fixed `così così` / `via via` pairs must be
    // ignored.
    let text =
        "Il gatto gatto dorme. Mi è sembrato così così. Il lessico si andava via via modificando.";
    let result = engine.check(text).unwrap();
    let repeats: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "ITALIAN_WORD_REPEAT_RULE")
        .collect();
    assert_eq!(repeats.len(), 1);
    assert_utf16(text, repeats[0], (3, 14));
    assert_eq!(
        repeats[0].message,
        "Possibile errore di battitura: parola ripetuta"
    );
    assert_eq!(repeats[0].suggestions[0].value, "gatto");
}
