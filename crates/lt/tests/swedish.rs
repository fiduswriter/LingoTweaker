//! Swedish engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule and rule-class values.
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes
//! (`scripts/oracle/sv/probe-rule.sh`, `scripts/oracle/sv/check-diff-sv.sh`)
//! print UTF-16 code units. Every probe sentence below uses real Swedish
//! orthography (ä/ö/å), so the two formats differ and both are stated per
//! case. The speller suggestions are the full Java `HunspellRule`
//! suggestion list (the native suggestion engine is ported, D-…); they must
//! preserve diacritics.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

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
    Engine::builder(Lang::Sv).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Sv)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(sv) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    sv.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: active XML rule count, no XML-referenced filters and
/// `compile_failures()` = 0.
#[test]
fn swedish_engine_state() {
    let _guard = engine_guard();
    let Some(sv) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(sv.active_rule_count(), 29);
    assert_eq!(sv.skipped_counts().filters, 0);
    assert!(
        sv.compile_failures().is_empty(),
        "{:?}",
        sv.compile_failures()
    );
}

/// `sv.CompoundRule` (`SV_COMPOUNDS`), Java probe: `Detta är ett e mail.`
/// UTF-16 13..19, UTF-8 14..20 (`ä` is one extra byte) -> `e-mail`.
#[test]
fn swedish_compound_rule() {
    let _guard = engine_guard();
    let matches = one("Detta är ett e mail.", "SV_COMPOUNDS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 14);
    assert_eq!(matches[0].range.end, 20);
    assert_eq!(
        matches[0].message,
        "Dessa ord skrivs samman med bindestreck."
    );
    assert_eq!(suggestions(&matches[0]), vec!["e-mail"]);
}

/// `sv.WordCoherencyRule` (`SV_WORD_COHERENCY`), Java probe:
/// `Vi använder facett och fasett om varandra.` UTF-16 23..29, UTF-8 24..30
/// (`ä` shifts the byte offset) -> `facett`.
#[test]
fn swedish_word_coherency() {
    let _guard = engine_guard();
    let matches = one(
        "Vi använder facett och fasett om varandra.",
        "SV_WORD_COHERENCY",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 24);
    assert_eq!(matches[0].range.end, 30);
    assert_eq!(
        matches[0].message,
        "Använd endast en av stavningsvarianterna 'fasett' och 'facett' i en och samma text."
    );
    assert_eq!(suggestions(&matches[0]), vec!["facett"]);
}

/// `CommaWhitespaceRule` (1), Java probe: `Det är en mening , här.`
/// UTF-16 16..18, UTF-8 17..19 (`ä` adds one byte).
#[test]
fn swedish_comma_whitespace() {
    let _guard = engine_guard();
    let matches = one("Det är en mening , här.", "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 17);
    assert_eq!(matches[0].range.end, 19);
    assert_eq!(
        matches[0].message,
        "Lägg till ett blanksteg efter kommatecknet, men inte före."
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DoublePunctuationRule` (2), Java probe: `Det är en mening..`
/// UTF-16 16..18, UTF-8 17..19.
#[test]
fn swedish_double_punctuation() {
    let _guard = engine_guard();
    let matches = one("Det är en mening..", "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 17);
    assert_eq!(matches[0].range.end, 19);
    assert_eq!(matches[0].message, "Dubbla punkter");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `GenericUnpairedBracketsRule` (3), Java probe: `(Det är en mening.` 0..1
/// (the match is on the opening bracket, before any non-ASCII byte).
#[test]
fn swedish_unpaired_brackets() {
    let _guard = engine_guard();
    let matches = one("(Det är en mening.", "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 1);
    assert_eq!(
        matches[0].message,
        "Grupperingssymboler: ')' ser ut att saknas"
    );
}

/// `UppercaseSentenceStartRule` (6), Java probe: `det är en mening.` 0..3.
#[test]
fn swedish_uppercase_sentence_start() {
    let _guard = engine_guard();
    let matches = one("det är en mening.", "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 3);
    assert_eq!(matches[0].message, "Meningen börjar inte med stor bokstav");
    assert_eq!(suggestions(&matches[0]), vec!["Det"]);
}

/// `MultipleWhitespaceRule` (10), Java probe: `Det  är en mening.` 3..5.
#[test]
fn swedish_multiple_whitespace() {
    let _guard = engine_guard();
    let matches = one("Det  är en mening.", "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 3);
    assert_eq!(matches[0].range.end, 5);
    assert_eq!(
        matches[0].message,
        "Möjligt korrekturfel: du upprepade ett blanktecken"
    );
}

/// `SentenceWhitespaceRule` (11), Java probe:
/// `Det är en mening.Det är en till.` UTF-16 17..20, UTF-8 18..21 -> ` Det`.
#[test]
fn swedish_sentence_whitespace() {
    let _guard = engine_guard();
    let matches = one("Det är en mening.Det är en till.", "SENTENCE_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 18);
    assert_eq!(matches[0].range.end, 21);
    assert_eq!(
        matches[0].message,
        "Lägg till ett blanksteg mellan meningarna."
    );
    assert_eq!(suggestions(&matches[0]), vec![" Det"]);
}

/// `HunspellRule` (4) with real Swedish misspellings. Java probe
/// (`check-diff-sv.sh`): full `getSuggestedReplacements` lists, including
/// diacritic-preserving candidates; UTF-16/UTF-8 offsets are stated per case.
#[test]
fn swedish_speller() {
    let _guard = engine_guard();

    let matches = one("Vi tar tesst fem myror.", "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 7);
    assert_eq!(matches[0].range.end, 12);
    assert_eq!(matches[0].message, "Hittat ett möjligt stavfel.");
    assert_eq!(matches[0].match_type, "UnknownWord");
    assert_eq!(
        suggestions(&matches[0]),
        vec!["tests", "test", "estet", "restes", "stress", "tes"]
    );

    let matches = one("Det var myket bra.", "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 8);
    assert_eq!(matches[0].range.end, 13);
    assert_eq!(
        suggestions(&matches[0]),
        vec!["mycket", "dyket", "tyket", "myset", "byket"]
    );

    // `härr`: Java UTF-16 7..11, UTF-8 8..13 (`ä` adds one byte); the
    // suggestions preserve `ä`/`ö` (`här`, `härar`, `märr`, `kärr`, ...).
    let matches = one("Han är härr i staden.", "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 8);
    assert_eq!(matches[0].range.end, 13);
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "herr", "här", "ärr", "härar", "härur", "härd", "häri", "härs", "märr", "härk", "kärr",
            "här-"
        ]
    );
}

/// XML rule `efter-hand` (`grammar.xml`), Java probe: with real orthography
/// `Det visade sig efterhand att orden borde särskrivas.` the match is
/// UTF-16 15..24 == UTF-8 15..24 (all-ASCII span) -> `efter hand`.
#[test]
fn swedish_xml_efter_hand() {
    let _guard = engine_guard();
    let matches = one(
        "Det visade sig efterhand att orden borde särskrivas.",
        "efter-hand",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 15);
    assert_eq!(matches[0].range.end, 24);
    assert_eq!(suggestions(&matches[0]), vec!["efter hand"]);
}

/// Long paragraph with real Swedish orthography. Java probe
/// (`check-diff-sv.sh`) yields exactly five matches, with the UTF-8 byte
/// offsets below (UTF-16 in parentheses where they differ).
#[test]
fn swedish_long_paragraph() {
    let _guard = engine_guard();
    let Some(sv) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let text = "Det var en gång en liten flicka som bodde i en stuga vid skogen . \
        Hon hade en katt som hette Maja, och tillsammans gick de ofta ut för att \
        plocka bär. En dag såg de en älg som stod alldeles stilla.. Flickan blev \
        myket rädd, men katten tesst fram och nosade på djuret. Efterhand vande \
        sig alla vid varandra och de levde lyckliga i många år.";
    let matches: Vec<(String, usize, usize)> = sv
        .check(text)
        .expect("check")
        .matches
        .into_iter()
        .map(|m| (m.rule_id, m.range.start, m.range.end))
        .collect();
    assert_eq!(
        matches,
        vec![
            ("COMMA_PARENTHESIS_WHITESPACE".into(), 64, 66),
            ("DOUBLE_PUNCTUATION".into(), 201, 203),
            ("HUNSPELL_RULE".into(), 217, 222),
            ("HUNSPELL_RULE".into(), 241, 246),
            ("efter-hand".into(), 275, 284),
        ],
        "{matches:?}"
    );
}

/// The `BaseTagger`-derived `SwedishTagger` tags known words.
#[test]
fn swedish_tagger() {
    let _guard = engine_guard();
    let Some(sv) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let sentences = sv.analyze("Vi tar med oss Maria och åker till Jönköping.");
    let readings: Vec<&lt::AnalyzedToken> = sentences
        .iter()
        .flat_map(|s| s.tokens.iter())
        .flat_map(|t| t.readings.iter())
        .filter(|r| !matches!(r.pos_tag.as_deref(), Some("SENT_START" | "SENT_END")))
        .collect();
    let tagged = readings.iter().filter(|r| r.pos_tag.is_some()).count();
    assert!(tagged > 0, "no tagged readings: {readings:?}");
}
