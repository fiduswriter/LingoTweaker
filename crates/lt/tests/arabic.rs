//! Arabic engine tests: stage-1 XML wiring state plus the XML/disambiguation
//! foundations.
//!
//! The pinned `ar` module is a maintained module; the Java oracle is wired in
//! a later stage. The expectations below come from the shared, Java-probed
//! engine foundations plus the `MessagesBundle_ar` strings, and UTF-16
//! offsets are asserted with the `common::assert_utf16` helper.

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
    Engine::builder(Lang::Ar).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Ar)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(ar) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    ar.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

/// Stage state: the `grammar.xml` rule units compile (`compile_failures()` = 0)
/// and the inventory-reported active count.
#[test]
fn arabic_engine_state() {
    let _guard = engine_guard();
    let Some(ar) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert!(
        ar.compile_failures().is_empty(),
        "{:?}",
        ar.compile_failures()
    );
    // `grammar.xml` parses to 450 rule units; one rule (`gender_0103`) is
    // `default="off"`, so 449 are active by default. `lt-cli inventory` reports
    // the parsed 450.
    assert_eq!(ar.grammar().expect("grammar").rules.len(), 450);
    assert_eq!(ar.active_rule_count(), 449);
    assert_eq!(ar.skipped_counts().filters, 0);
    assert_eq!(ar.skipped_counts().off_by_default, 1);
}

/// The engine resolves `ar` / `ar-SA` to the same language.
#[test]
fn arabic_language_metadata() {
    assert_eq!(Lang::from_long_code("ar"), Some(Lang::Ar));
    assert_eq!(Lang::from_long_code("ar-SA"), Some(Lang::Ar));
    assert_eq!(Lang::from_long_code("ar-EG"), Some(Lang::Ar));
    assert_eq!(Lang::Ar.base_code(), "ar");
    assert_eq!(Lang::Ar.info().name, "Arabic");
}

/// Uppercase `lt-cli check --lines` UTF-16 conversion on Arabic script: a pure
/// literal-pattern XML rule (`collo_0077_a3la_raghmi_min`, `على الرغم من`).
#[test]
fn arabic_literal_pattern_rule() {
    let _guard = engine_guard();
    let text = "على الرغم من أنّ المسألة صعبة";
    let matches = one(text, "collo_0077_a3la_raghmi_min");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 12));
    assert_eq!(matches[0].suggestions[0].value, "مع أنّ");
}

/// Multi-token literal rule (`collo_0049_kma_anna`: `كما أن` -> `ثم أن`).
#[test]
fn arabic_multi_token_rule() {
    let _guard = engine_guard();
    let text = "كان للجهاز أثر كبير ...، كما أن استخدامه سيزداد ...";
    let matches = one(text, "collo_0049_kma_anna");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (25, 31));
    assert_eq!(matches[0].suggestions[0].value, "ثم أن");
}

/// `ArabicDateCheckFilter` (`DATE_IN_WORD_DAY`): 2022-03-25 is a Friday, so a
/// "الخميس" (Thursday) date is flagged with `{realDay}` rendered.
#[test]
fn arabic_date_check_filter_word_day() {
    let _guard = engine_guard();
    let text = "الخميس 25 مارس 2022";
    let matches = one(text, "DATE_IN_WORD_DAY");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 19));
    assert!(
        matches[0].message.contains("الجمعة"),
        "{}",
        matches[0].message
    );
    // The matching weekday is not flagged.
    assert!(one("الجمعة 25 مارس 2022", "DATE_IN_WORD_DAY").is_empty());
}

/// `ArabicDateCheckFilter` (`DATE_IN_DIGIT_DAY`).
#[test]
fn arabic_date_check_filter_digit_day() {
    let _guard = engine_guard();
    let text = "الخميس 25/03/2022";
    let matches = one(text, "DATE_IN_DIGIT_DAY");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 17));
    assert!(one("الجمعة 25/03/2022", "DATE_IN_DIGIT_DAY").is_empty());
}

/// `ArabicHunspellSpellerRule` (`HUNSPELL_RULE_AR`): a nonsense word is flagged
/// and real words (incl. the affix-generated definite forms) are accepted.
/// Java probe (Docker, pinned 6.9): the same four verdicts and the same
/// suggestions for `خقخق`/`بتبتب`/`كتاااب`.
#[test]
fn arabic_hunspell_speller() {
    let _guard = engine_guard();
    let text = "خقخق";
    let matches = one(text, "HUNSPELL_RULE_AR");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 4));
    assert_eq!(matches[0].message, "وُجد خطأ إملائي محتمل");
    assert_eq!(matches[0].short_message.as_deref(), Some("خطأ إملائي"));
    assert_eq!(
        matches[0]
            .suggestions
            .iter()
            .map(|s| s.value.as_str())
            .collect::<Vec<_>>(),
        vec!["خفخف", "حفحف"]
    );
}

/// Real words are accepted, including the definite article generated through
/// the `AF` alias continuation classes (`الكتاب`, `المدرسة`).
#[test]
fn arabic_hunspell_speller_accepts_real_words() {
    let _guard = engine_guard();
    for text in ["كتاب", "الكتاب", "مدرسة", "المدرسة"] {
        assert!(
            one(text, "HUNSPELL_RULE_AR").is_empty(),
            "{text} must be accepted"
        );
    }
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// `ArabicVerbToMafoulMutlaqFilter` (`collo_0081_shkl_3am_Test`): the verb
/// lemma drives the `arabic_verb_masdar.txt` masdar list and the
/// `inflectMafoulMutlq`/`inflectAdjectiveTanwinNasb` helpers. Java probe:
/// suggestions `يعمل إعمالًا عامًا|يعمل عملةً عامةً|يعمل عملًا عامًا`.
#[test]
fn arabic_verb_to_mafoul_mutlaq_filter() {
    let _guard = engine_guard();
    let text = "الأمر مستقر يعمل بأسلوب عام في البلاد.";
    let matches = one(text, "collo_0081_shkl_3am_Test");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (12, 27));
    assert_eq!(
        suggestions(&matches[0]),
        vec!["يعمل إعمالًا عامًا", "يعمل عملةً عامةً", "يعمل عملًا عامًا"]
    );
}

/// `ArabicMasdarToVerbFilter` (`syntax_0000_Qam_bi_test`): `قمت بالعمل` ->
/// `عملت` (the auxiliary `قَامَ` inflection is reused via
/// `ArabicSynthesizer.inflectLemmaLike`).
#[test]
fn arabic_masdar_to_verb_filter() {
    let _guard = engine_guard();
    let text = "قمت بالعمل في الامتحان";
    let matches = one(text, "syntax_0000_Qam_bi_test");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 10));
    assert_eq!(suggestions(&matches[0]), vec!["عملت"]);
}

/// The future/conjunction auxiliaries chain through the tag manager.
#[test]
fn arabic_masdar_to_verb_filter_conjunction() {
    let _guard = engine_guard();
    let text = "وسيقومون بالأكل في الامتحان";
    let matches = one(text, "syntax_0000_Qam_bi_test");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 15));
    assert_eq!(suggestions(&matches[0]), vec!["وسيأكلون"]);
}

/// `ArabicCommaWhitespaceRule` (`ARABIC_COMMA_PARENTHESIS_WHITESPACE`): the
/// Arabic comma character `،` preceded by whitespace.
#[test]
fn arabic_comma_whitespace() {
    let _guard = engine_guard();
    let text = "وأخيرا وليس آخرا ، أختم كلامي بكذا";
    let matches = one(text, "ARABIC_COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (16, 18));
    assert_eq!(matches[0].message, "ضع فراغا بعد الفاصلة، وليس قبلها.");
    assert_eq!(suggestions(&matches[0]), vec!["،"]);
}

/// `ArabicDiacriticsRule` (`AR_DIACRITICS_REPLACE`): the `diacritics.txt`
/// replacement list.
#[test]
fn arabic_diacritics_replace() {
    let _guard = engine_guard();
    let text = "هو عالم فذ ولكن تنقصه التجارب";
    let matches = one(text, "AR_DIACRITICS_REPLACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (22, 29));
    assert_eq!(suggestions(&matches[0]), vec!["التجارِب"]);
}

/// `ArabicDoublePunctuationRule` (`ARABIC_DOUBLE_PUNCTUATION`, comma `،`).
#[test]
fn arabic_double_punctuation() {
    let _guard = engine_guard();
    let text = "نعم،، لقد نجحنا";
    let matches = one(text, "ARABIC_DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1);
    assert_eq!(suggestions(&matches[0]), vec!["،"]);
}
