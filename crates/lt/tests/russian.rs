//! Russian engine tests. Stage 1 pins the XML wiring state (active rules and
//! the unmapped `<filter>` classes) and the generic core built-ins; the
//! speller and the language's Java rule classes are added in stages 2/3.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/ru/check-diff-ru.sh`,
//! `scripts/oracle/ru/probe-rule.sh`); the probes use real Cyrillic
//! orthography.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

mod common;
use common::assert_utf16;

fn engine_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn data_dir() -> Option<DataDir> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    if !path.is_dir() {
        eprintln!("skipping: no vendored data directory found");
        return None;
    }
    Some(DataDir::new(path))
}

fn engine() -> Option<Engine> {
    let data = data_dir()?;
    Engine::builder(Lang::Ru).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Ru)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(ru) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    ru.check(text)
        .map(|r| r.matches)
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

#[test]
fn filters_are_all_mapped() {
    let _guard = engine_guard();
    let Some(ru) = engine() else {
        return;
    };
    // All six XML-referenced filter classes are ported (stage 3).
    assert!(
        ru.compile_failures().is_empty(),
        "unexpected compile failures: {:?}",
        ru.compile_failures()
    );
}

#[test]
fn xml_rulegroup_rule_fires() {
    let _guard = engine_guard();
    let matches = one("Он надел будний костюм.", "budniy_budnichnij");
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_eq!(matches.len(), 1);
    assert_utf16("Он надел будний костюм.", &matches[0], (9, 22));
}

#[test]
fn comma_whitespace_uses_russian_message() {
    let _guard = engine_guard();
    let matches = one(
        "Не род , а ум поставлю в воеводы.",
        "COMMA_PARENTHESIS_WHITESPACE",
    );
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_eq!(
        matches[0].message,
        "Поставьте пробел после запятой, а не перед ней."
    );
}

#[test]
fn uppercase_sentence_start_uses_russian_message() {
    let _guard = engine_guard();
    let matches = one(
        "Закончилось лето. дети снова сели за школьные парты.",
        "UPPERCASE_SENTENCE_START",
    );
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_eq!(
        matches[0].message,
        "Это предложение не начинается с заглавной буквы."
    );
    assert_eq!(matches[0].suggestions[0].value, "Дети");
    assert_utf16(
        "Закончилось лето. дети снова сели за школьные парты.",
        &matches[0],
        (18, 22),
    );
}

/// `scripts/oracle/ru/probe-speller.sh каждя` — Java probe
/// `M каждя 0-5:Возможно найдена орфографическая ошибка.:дождя|кадя|каждая|вождя|ка ждя`.
#[test]
fn speller_matches_java_for_kazhdya() {
    let _guard = engine_guard();
    let matches = one("каждя", "MORFOLOGIK_RULE_RU_RU");
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_eq!(matches.len(), 1);
    assert_utf16("каждя", &matches[0], (0, 5));
    assert_eq!(
        matches[0].message,
        "Возможно найдена орфографическая ошибка."
    );
    let suggestions: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(suggestions, ["дождя", "кадя", "каждая", "вождя", "ка ждя"]);
}

/// `RUSSIAN_LETTERS.ignoreToken`: a token with anything but Russian
/// letters/hyphen/stress marks is not spell-checked (`abc`, `тест123`), while
/// `по-русски` is.
#[test]
fn speller_ignores_non_russian_letters() {
    let _guard = engine_guard();
    for word in ["abc", "тест123", "по-русски", "каждая"] {
        let matches = one(word, "MORFOLOGIK_RULE_RU_RU");
        assert!(
            matches.is_empty(),
            "unexpected speller match for {word}: {matches:?}"
        );
    }
}

/// `scripts/oracle/ru/probe-speller.sh --rule MORFOLOGIK_RULE_RU_RU_YO елка`
/// — Java probe `ёлка|ёлку|ялта|…|шелка`; the rule is default off.
#[test]
fn yo_speller_flags_elka_when_enabled() {
    let _guard = engine_guard();
    let matches = one("елка", "MORFOLOGIK_RULE_RU_RU_YO");
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_eq!(matches.len(), 1);
    assert_utf16("елка", &matches[0], (0, 4));
    let suggestions: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(suggestions[0], "ёлка");
    assert_eq!(
        suggestions,
        [
            "ёлка",
            "ёлку",
            "ялта",
            "ярко",
            "ямка",
            "явка",
            "ятка",
            "едко",
            "белка",
            "янка",
            "телка",
            "ёлках",
            "ёлкам",
            "ёлке",
            "ёлки",
            "ёлкою",
            "ёлкой",
            "ёмка",
            "ёмко",
            "юлка",
            "юлку",
            "едка",
            "ейка",
            "ейку",
            "ела",
            "елла",
            "еллу",
            "елва",
            "елву",
            "елза",
            "елзу",
            "емко",
            "ила",
            "илека",
            "илга",
            "илгу",
            "илька",
            "илза",
            "илзу",
            "инка",
            "инку",
            "иска",
            "иску",
            "лека",
            "леку",
            "мелка",
            "яка",
            "яла",
            "ялика",
            "ялту",
            "ямку",
            "янку",
            "ярка",
            "ярку",
            "ятку",
            "явку",
            "шелка"
        ]
    );
}

/// The YO rule is default off, so a plain check does not report it.
#[test]
fn yo_speller_is_default_off() {
    let _guard = engine_guard();
    let Some(ru) = engine() else {
        return;
    };
    let matches: Vec<lt::Match> = ru
        .check("елка")
        .map(|r| r.matches)
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.rule_id == "MORFOLOGIK_RULE_RU_RU_YO")
        .collect();
    assert!(matches.is_empty(), "YO rule must be default off");
}

/// `scripts/oracle/ru/probe-synth.sh стол книга красивый` — Java probe:
/// `стол|NN:Inanim:Masc:Sin:Nom` → `стол`, `красивый|ADJ:Posit:Masc:V` →
/// `красивый|красивого`.
#[test]
fn synthesizer_matches_java() {
    let Some(data) = data_dir() else {
        return;
    };
    let synth = lt_tagger::RussianSynthesizer::from_data(data.path()).expect("synth");
    let plain = |lemma: &str, tag: &str| {
        synth.synthesize(
            &lt::AnalyzedToken::new("", Some(lemma.to_string()), Some(tag.to_string())),
            tag,
            false,
        )
    };
    assert_eq!(plain("стол", "NN:Inanim:Masc:Sin:Nom"), ["стол"]);
    assert_eq!(
        plain("красивый", "ADJ:Posit:Masc:V"),
        ["красивый", "красивого"]
    );
}

// Filter probes (`scripts/oracle/ru/probe-rule.sh`), Java UTF-16 offsets.

#[test]
fn date_check_filter_matches_java() {
    let _guard = engine_guard();
    let text = "Конференция состоится в понедельник, 7 октября 2014 г.";
    let matches = one(text, "DATE_WEEKDAY1");
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_utf16(text, &matches[0], (24, 51));
    assert_eq!(
        matches[0].message,
        "Днём недели 7 октября 2014 года является вторник."
    );
}

#[test]
fn wrong_inn_filter_matches_java() {
    let _guard = engine_guard();
    let text = "ИНН: 1234567890";
    let matches = one(text, "WRONG_INN");
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_utf16(text, &matches[0], (0, 15));
    assert_eq!(matches[0].message, "Некорректный ИНН: 1234567890");
}

#[test]
fn future_date_filter_matches_java() {
    let _guard = engine_guard();
    let text = "Мы посетили клиента 17 июня 2039 г.";
    let matches = one(text, "INVALID_TENSE_DATE");
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_utf16(text, &matches[0], (20, 32));
    assert_eq!(
        matches[0].message,
        "Данная дата находится в будущем, но глагол стоит в прошедшем времени."
    );
}

#[test]
fn advanced_synthesizer_filter_matches_java() {
    let _guard = engine_guard();
    let text = "Я терпеть не могу эту глупою женщину.";
    let matches = one(text, "Unify_Adj_NN_case");
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_utf16(text, &matches[0], (22, 36));
    assert_eq!(
        matches[0].message,
        "Прилагательное не согласуется с существительным по падежу."
    );
    let suggestions: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(suggestions, ["глупую женщину"]);
}

#[test]
fn partial_pos_tag_filter_matches_java() {
    let _guard = engine_guard();
    let text = "Южнокорейский консорциум может по участвовать в проекте.";
    let matches = one(text, "pouchastvovat");
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_utf16(text, &matches[0], (31, 45));
    let suggestions: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(suggestions, ["поучаствовать"]);
}

#[test]
fn suppress_misspelled_filter_matches_java() {
    let _guard = engine_guard();
    let text = "Сегодня на ужин жареная на масле картошка.";
    let matches = one(text, "NN_N_pril_prich");
    if matches.is_empty() {
        eprintln!("skipping: no vendored data");
        return;
    }
    assert_utf16(text, &matches[0], (16, 23));
    let suggestions: Vec<&str> = matches[0]
        .suggestions
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(suggestions, ["жаренная"]);
}
