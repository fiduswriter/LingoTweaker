//! Serbian engine tests: stage-1 XML wiring state plus the generic built-in
//! rule values.
//!
//! The pinned `sr` module is excluded from the LanguageTool reactor
//! (`pom.xml`: "re-add when a maintainer is found") and does not compile
//! against the pinned core (4.5→6.9 API drift in `AbstractSimpleReplaceRule`,
//! `BaseTagger`, `BaseSynthesizer`, `MultiWordChunker`), so there is no
//! Java probe for Serbian yet (see `attic/docs/parity/sr-rule-port.md`). The
//! expectations below come from the shared, Java-probed engine foundations
//! (`BaseTagger`, `XmlRuleDisambiguator`, `GenericUnpairedBracketsRule`, the
//! generic built-ins) plus the `MessagesBundle_sr` strings, and the UTF-16
//! offsets are asserted with the `common::assert_utf16` helper.
//!
//! `Serbian` has no custom word tokenizer (`Serbian.getSentenceTokenizer` is
//! `SRXSentenceTokenizer` → `sr_two`), no `tag()`/`additionalTags` tagger
//! override, and the hybrid disambiguator's `multiwords.txt` is empty.

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
    Engine::builder(Lang::Sr).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Sr)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(tl) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    tl.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 12 active XML rules, no XML-referenced filters and
/// `compile_failures()` = 0.
#[test]
fn serbian_engine_state() {
    let _guard = engine_guard();
    let Some(tl) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(tl.active_rule_count(), 12);
    assert_eq!(tl.skipped_counts().filters, 0);
    assert!(
        tl.compile_failures().is_empty(),
        "{:?}",
        tl.compile_failures()
    );
}

/// `INVALID_DATE` sub-rule 1 (`31. новембра`, 30-day month).
#[test]
fn serbian_invalid_date_named_month() {
    let _guard = engine_guard();
    let text = "То се догодило 31. новембра 2014.";
    let matches = one(text, "INVALID_DATE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_utf16(text, &matches[0], (15, 27));
    assert_eq!(matches[0].message, "У овом месецу има само 30 дана.");
    assert!(suggestions(&matches[0]).is_empty());
}

/// `INVALID_DATE` sub-rule 2 (`31.4.`, numeric month).
#[test]
fn serbian_invalid_date_numeric_month() {
    let _guard = engine_guard();
    let text = "То је било 31.4.2014.";
    let matches = one(text, "INVALID_DATE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].sub_id.as_deref(), Some("2"));
    assert_utf16(text, &matches[0], (11, 16));
    assert_eq!(matches[0].message, "Овај месец има само 30 дана.");
}

/// `YEAR_20001` (`маја 20014` -> `маја 2014`).
#[test]
fn serbian_year_20001() {
    let _guard = engine_guard();
    let text = "Планирао сам распуст маја 20014.";
    let matches = one(text, "YEAR_20001");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (21, 31));
    assert_eq!(
        matches[0].message,
        "Заиста 20014? Или је <suggestion>маја 2014</suggestion>?"
    );
    assert_eq!(suggestions(&matches[0]), vec!["маја 2014"]);
}

/// `ZAPETA_TLD` (`example,com` -> `example.com`).
#[test]
fn serbian_zapeta_tld() {
    let _guard = engine_guard();
    let text = "Провери то на example,com .";
    let matches = one(text, "ZAPETA_TLD");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (14, 25));
    assert_eq!(
        matches[0].message,
        "Могућа грешка писања: Хтедосте ли написати <suggestion>example.com</suggestion>?"
    );
    assert_eq!(suggestions(&matches[0]), vec!["example.com"]);
}

/// `SR_ADJ_LOWER_LETTER_CASE` (`Новосадска` after a `PR:` token before an
/// `IM:` token).
#[test]
fn serbian_adj_lower_letter_case() {
    let _guard = engine_guard();
    let text = "Штранд је Новосадска плажа.";
    let matches = one(text, "SR_ADJ_LOWER_LETTER_CASE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (10, 20));
    assert_eq!(
        matches[0].message,
        "Придев <suggestion>новосадска</suggestion> пише се малим почетним словом."
    );
    assert_eq!(suggestions(&matches[0]), vec!["новосадска"]);
}

/// `COMMA_PARENTHESIS_WHITESPACE` (`space_after_comma`).
#[test]
fn serbian_comma_whitespace() {
    let _guard = engine_guard();
    let text = "Није шија , него врат.";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (9, 11));
    assert_eq!(
        matches[0].message,
        "Ставите размак после запете, никако пре"
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `UPPERCASE_SENTENCE_START` (`incorrect_case`/`category_case`).
#[test]
fn serbian_uppercase_start() {
    let _guard = engine_guard();
    let text = "Почела је школа. ђаци су поново сели у клупе.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (17, 21));
    assert_eq!(matches[0].message, "Ова реченица не почиње великим словом");
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("Велико/мало почетно слово")
    );
    assert_eq!(suggestions(&matches[0]), vec!["Ђаци"]);
}

/// `MultipleWhitespaceRule` (`WHITESPACE_RULE`, `whitespace_repetition`).
#[test]
fn serbian_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Он  иде.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (2, 4));
    assert_eq!(matches[0].message, "Могућа грешка: поновили сте белину");
}

/// `DoublePunctuationRule` (`two_dots`).
#[test]
fn serbian_double_punctuation() {
    let _guard = engine_guard();
    let text = "Он иде..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (6, 8));
    assert_eq!(matches[0].message, "Две узастопне тачке");
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("Две узастопне тачке")
    );
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `WordRepeatRule` (`repetition`/`desc_repetition_short`).
#[test]
fn serbian_word_repeat() {
    let _guard = engine_guard();
    let text = "Он он иде.";
    let matches = one(text, "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 5));
    assert_eq!(matches[0].message, "Могућа грешка: поновили сте реч");
    assert_eq!(matches[0].short_message.as_deref(), Some("Понављање речи"));
    assert_eq!(suggestions(&matches[0]), vec!["Он"]);
}

/// `GenericUnpairedBracketsRule` with the Serbian symbol lists and
/// `MessagesBundle_sr` strings (`MessageFormat` renders the doubled quotes).
#[test]
fn serbian_unpaired_brackets() {
    let _guard = engine_guard();
    let text = "(Он иде.";
    let matches = one(text, "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 1));
    assert_eq!(
        matches[0].message,
        "Распарени симбол: изгледа да ')' недостаје"
    );
}

/// `MorfologikEkavianSpellerRule` (`MORFOLOGIK_RULE_SR_EKAVIAN`): the
/// frequency-included ekavian dictionary. Latin-script words are checked.
#[test]
fn serbian_speller_flags_latin_word() {
    let _guard = engine_guard();
    let text = "kafana";
    let matches = one(text, "MORFOLOGIK_RULE_SR_EKAVIAN");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 6));
    assert_eq!(matches[0].message, "Пронађена вероватна грешка спеловања");
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("Грешка спеловања")
    );
    assert_eq!(suggestions(&matches[0]), vec!["Asana", "Grafana", "Kamala"]);
}

/// Upstream `MorfologikEkavianSpellerRule` does not override
/// `isLatinScript()`, so the base `true` makes `ignoreWord` drop every
/// pure-Cyrillic token (`^[^\p{script=latin}]+$`): the Serbian speller is
/// effectively inert for its own script. The port reproduces this exactly
/// (no Java probe is possible, see the checklist).
#[test]
fn serbian_speller_ignores_cyrillic_upstream_bug() {
    let _guard = engine_guard();
    for text in ["бткие", "теест", "Тамо је леп цвет"] {
        assert!(
            one(text, "MORFOLOGIK_RULE_SR_EKAVIAN").is_empty(),
            "Cyrillic token {text:?} must be ignored like Java"
        );
    }
}

/// `sr/disambiguation.xml` `RIMSKI_BROJEVI` (`action="ignore_spelling"`) marks
/// Roman numerals as not-to-be-spell-checked.
#[test]
fn serbian_disambiguation_roman_numerals() {
    let _guard = engine_guard();
    assert!(one("III", "MORFOLOGIK_RULE_SR_EKAVIAN").is_empty());
}
