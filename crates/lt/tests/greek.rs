//! Greek engine tests: stage-1/2 XML wiring state plus Java-probed built-in
//! values.
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes
//! (`scripts/oracle/el/probe-rule.sh`) print UTF-16 code units, converted
//! where a test exercises Greek letters. The Greek tagger combines the small
//! `greek.dict` with the `morphology-el` analyzer fallback.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

/// One engine at a time: the Greek engines hold the speller dictionary.
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
    Engine::builder(Lang::El).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::El)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Rule matches for `text` with one rule enabled.
fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(el) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    el.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: active XML rule count and `compile_failures()` = 0.
#[test]
fn greek_engine_state() {
    let _guard = engine_guard();
    let Some(el) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(el.active_rule_count(), 53);
    assert_eq!(el.disambig_rule_count(), 66);
    assert_eq!(el.skipped_counts().filters, 0);
    assert!(
        el.compile_failures().is_empty(),
        "{:?}",
        el.compile_failures()
    );
}

/// `CommaWhitespaceRule` (1), Java probe: `Το , je test.` 2..4.
#[test]
fn greek_comma_whitespace() {
    let _guard = engine_guard();
    let matches = one("Το , je test.", "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 4);
    assert_eq!(matches[0].range.end, 6);
    assert_eq!(
        matches[0].message,
        "Προσθέστε ένα κενό μετά το κόμμα αλλά όχι πριν το κόμμα."
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DoublePunctuationRule` (2), Java probe: `Το κείμενο..` 10..12.
#[test]
fn greek_double_punctuation() {
    let _guard = engine_guard();
    let matches = one("Το κείμενο..", "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 19);
    assert_eq!(matches[0].range.end, 21);
    assert_eq!(matches[0].message, "Δύο συνεχόμενες τελείες");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `UppercaseSentenceStartRule` (6), Java probe: the second sentence starts
/// lowercase (Java 12..14; UTF-8 bytes 14..16).
#[test]
fn greek_uppercase_sentence_start() {
    let _guard = engine_guard();
    let matches = one("Το κείμενο. το κείμενο.", "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 21);
    assert_eq!(matches[0].range.end, 25);
    assert_eq!(
        matches[0].message,
        "Η πρόταση δεν ξεκινάει με κεφαλαίο γράμμα"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Το"]);
}

/// `MultipleWhitespaceRule` (7), Java probe: `Το  κείμενο.` 2..4.
#[test]
fn greek_multiple_whitespace() {
    let _guard = engine_guard();
    let matches = one("Το  κείμενο.", "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 4);
    assert_eq!(matches[0].range.end, 6);
    assert_eq!(matches[0].message, "Πιθανό λάθος: επανάληψη κενού");
}

/// `WordRepeatRule` (9), Java probe: `Το το κείμενο.` 0..5.
#[test]
fn greek_word_repeat() {
    let _guard = engine_guard();
    let matches = one("Το το κείμενο.", "WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 9);
    assert_eq!(matches[0].message, "Πιθανό λάθος: επαναλάβατε μία λέξη");
    assert_eq!(suggestions(&matches[0]), vec!["Το"]);
}

/// `MorfologikGreekSpellerRule` (5), Java probe: `Αυτό ειναι λαθος.` with the
/// ISO-8859-7 dictionary. Java 5..10 / 11..16; UTF-8 bytes 9..19 / 20..30.
#[test]
fn greek_speller() {
    let _guard = engine_guard();
    let matches = one("Αυτό ειναι λαθος.", "MORFOLOGIK_RULE_EL_GR");
    assert_eq!(matches.len(), 2, "{matches:?}");
    assert_eq!(matches[0].range.start, 9);
    assert_eq!(matches[0].range.end, 19);
    assert_eq!(matches[0].message, "Βρέθηκε πιθανό ορθογραφικό λάθος");
    assert_eq!(matches[0].match_type, "UnknownWord");
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "Είναι",
            "είναι",
            "Εϊνάρ",
            "Σίναι",
            "είμαι",
            "είσαι",
            "ει ναι"
        ]
    );
    assert_eq!(matches[1].range.start, 20);
    assert_eq!(matches[1].range.end, 30);
    assert_eq!(
        suggestions(&matches[1]),
        vec![
            "λάθος",
            "Λάζος",
            "Λάιος",
            "Λάος",
            "Λάσος",
            "βάθος",
            "λάζος",
            "λάθους",
            "λάλος",
            "λάρος",
            "λάχος",
            "λίθος",
            "λαγός",
            "λαός",
            "μάθος",
            "μαθός",
            "πάθος",
            "παθός"
        ]
    );
}

/// XML rule `GREEK_AGREEMENT_1` (grammar.xml), Java probe:
/// `Οι μέθοδοι αυτοί είναι κατάλληλοι.` Java 11..16; UTF-8 bytes 20..30.
#[test]
fn greek_xml_agreement() {
    let _guard = engine_guard();
    let matches = one("Οι μέθοδοι αυτοί είναι κατάλληλοι.", "GREEK_AGREEMENT_1");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 20);
    assert_eq!(matches[0].range.end, 30);
    assert_eq!(suggestions(&matches[0]), vec!["αυτές"]);
}

/// XML rule `GREEK_WHERE-` (accented question word), Java probe:
/// `Που πας;` 0..3 -> `Πού`.
#[test]
fn greek_xml_accented_question() {
    let _guard = engine_guard();
    let matches = one("Που πας;", "GREEK_WHERE-");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 6);
    assert_eq!(suggestions(&matches[0]), vec!["Πού"]);
}

/// The `GreekTagger` combines the small `greek.dict` with the
/// `morphology-el` analyzer: `έδρα` is only in the analyzer.
#[test]
fn greek_tagger_analyzer_fallback() {
    let _guard = engine_guard();
    let Some(el) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let sentences = el.analyze("έδρα");
    let readings: Vec<String> = sentences
        .iter()
        .flat_map(|s| s.tokens.iter())
        .flat_map(|t| t.readings.iter())
        .filter(|r| r.token == "έδρα")
        .map(|r| {
            format!(
                "{}:{}",
                r.stem.clone().unwrap_or_default(),
                r.pos_tag.clone().unwrap_or_default()
            )
        })
        .collect();
    assert!(
        readings.contains(&"έδρα:N:FEM:SG:NOM".to_string()),
        "{readings:?}"
    );
    assert!(
        readings.contains(&"έδρα:N:FEM:SG:ACC".to_string()),
        "{readings:?}"
    );
}

/// `GreekRedundancyRule` (13), Java probe:
/// `Μου αρέσει να ανεβαίνω πάνω σε δέντρα.` Java 14..27; UTF-8 bytes 25..50.
#[test]
fn greek_redundancy() {
    let _guard = engine_guard();
    let matches = one(
        "Μου αρέσει να ανεβαίνω πάνω σε δέντρα.",
        "EL_REDUNDANCY_REPLACE",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 25);
    assert_eq!(matches[0].range.end, 50);
    assert_eq!(
        matches[0].message,
        "'ανεβαίνω πάνω' είναι πλεονασμός. Γενικά, είναι προτιμότερο το: <suggestion>ανεβαίνω</suggestion>"
    );
    assert_eq!(suggestions(&matches[0]), vec!["ανεβαίνω"]);
}

/// `ReplaceHomonymsRule` (10), Java probe: `πολικό κλήμα` 0..12 ->
/// `πολικό κλίμα` (suggestion uppercased at sentence start).
#[test]
fn greek_homonyms() {
    let _guard = engine_guard();
    let matches = one("πολικό κλήμα", "GREEK_HOMONYMS_REPLACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 23);
    assert_eq!(
        matches[0].message,
        "Μήπως εννοούσατε <suggestion>πολικό κλίμα</suggestion>?"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Πολικό κλίμα"]);
}

/// `GreekSpecificCaseRule` (11), Java probe:
/// `Κατοικώ στις Ηνωμένες πολιτείες.` Java 13..31; UTF-8 bytes 24..59.
#[test]
fn greek_specific_case() {
    let _guard = engine_guard();
    let matches = one("Κατοικώ στις Ηνωμένες πολιτείες.", "EL_SPECIFIC_CASE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 24);
    assert_eq!(matches[0].range.end, 59);
    assert_eq!(
        matches[0].message,
        "Οι λέξεις της συγκεκριμένης έκφρασης χρείαζεται να ξεκινούν με κεφαλαία γράμματα."
    );
    assert_eq!(suggestions(&matches[0]), vec!["Ηνωμένες Πολιτείες"]);
}

/// `NumeralStressRule` (12), Java probe:
/// `Ο 20ος αιώνας μαζί με τον 21ο αιώνα.` Java 2..6; UTF-8 bytes 3..9.
#[test]
fn greek_numeral_stress() {
    let _guard = engine_guard();
    let matches = one(
        "Ο 20ος αιώνας μαζί με τον 21ο αιώνα.",
        "GREEK_ORTHOGRAPHY_NUMERAL_STRESS",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 3);
    assert_eq!(matches[0].range.end, 9);
    assert_eq!(matches[0].message, "<suggestion>20ός</suggestion>");
    assert_eq!(suggestions(&matches[0]), vec!["20ός"]);
    // 10ος is correct (no stress)
    let none = one("Ο 10ος αιώνας.", "GREEK_ORTHOGRAPHY_NUMERAL_STRESS");
    assert!(none.is_empty(), "{none:?}");
}

/// `GreekWordRepeatBeginningRule` (8), Java probe:
/// `Επίσης, παίζω ποδόσφαιρο. Επίσης, παίζω μπάσκετ.` Java 26..32; bytes 47..59.
#[test]
fn greek_word_repeat_beginning() {
    let _guard = engine_guard();
    let matches = one(
        "Επίσης, παίζω ποδόσφαιρο. Επίσης, παίζω μπάσκετ.",
        "GREEK_WORD_REPEAT_BEGINNING_RULE",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 47);
    assert_eq!(matches[0].range.end, 59);
    assert_eq!(
        suggestions(&matches[0]),
        vec!["Επιπρόσθετα", "Επιπλέον", "Συμπληρωματικά", "Ακόμη"]
    );
}

/// The `el/disambiguation.xml` `HAVE_INF` rule: `πάει` keeps only the `INF`
/// reading after `έχει` (Java probe: `πάω:INF|πάω:SENT_END`).
#[test]
fn greek_disambiguation_have_inf() {
    let _guard = engine_guard();
    let Some(el) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let sentences = el.analyze("έδρα αγωγός έχει πάει");
    let paei: Vec<String> = sentences
        .iter()
        .flat_map(|s| s.tokens.iter())
        .filter(|t| t.surface() == "πάει")
        .flat_map(|t| t.readings.iter())
        .map(|r| {
            format!(
                "{}:{}",
                r.stem.clone().unwrap_or_default(),
                r.pos_tag.clone().unwrap_or_default()
            )
        })
        .collect();
    assert_eq!(paei, vec!["πάω:INF", "πάω:SENT_END"], "{paei:?}");
}

/// XML rule `HAVE_INF` (`grammar.xml`), Java probe: `Είχα πάω.` Java 5..8 ->
/// the `<match no="2" postag="INF"/>` synthesis `πάει` (the Greek synthesizer
/// must be wired for `<match postag>` rendering).
#[test]
fn greek_xml_have_inf_synthesis() {
    let _guard = engine_guard();
    let matches = one("Είχα πάω.", "HAVE_INF");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 9);
    assert_eq!(matches[0].range.end, 15);
    assert_eq!(
        matches[0].message,
        "Πιθανόν να χρειάζεται να χρησιμοποιήσετε τον απαρεμφατικό τύπο <suggestion>πάει</suggestion>"
    );
    assert_eq!(suggestions(&matches[0]), vec!["πάει"]);
}
