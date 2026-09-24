//! Swedish engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule and rule-class values.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/sv/probe-rule.sh`,
//! `scripts/oracle/sv/check-diff-sv.sh`). The speller suggestions are the
//! full Java `HunspellRule` suggestion list (the native suggestion engine is
//! ported, D-…); they must preserve diacritics.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

mod common;
use common::{assert_utf16, assert_utf16_range};

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

/// Stage state: active XML rule count (35; 29 original + 6 rules mined from
/// SALDO for the "Ord som ofta förväxlas" category, see
/// `swedish_saldo_confusables`), no XML-referenced filters and
/// `compile_failures()` = 0.
#[test]
fn swedish_engine_state() {
    let _guard = engine_guard();
    let Some(sv) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(sv.active_rule_count(), 35);
    assert_eq!(sv.skipped_counts().filters, 0);
    assert!(
        sv.compile_failures().is_empty(),
        "{:?}",
        sv.compile_failures()
    );
}

/// `sv.CompoundRule` (`SV_COMPOUNDS`), Java probe -> `e-mail`.
#[test]
fn swedish_compound_rule() {
    let _guard = engine_guard();
    let text = "Detta är ett e mail.";
    let matches = one(text, "SV_COMPOUNDS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (13, 19));
    assert_eq!(
        matches[0].message,
        "Dessa ord skrivs samman med bindestreck."
    );
    assert_eq!(suggestions(&matches[0]), vec!["e-mail"]);
}

/// `sv.WordCoherencyRule` (`SV_WORD_COHERENCY`), Java probe -> `facett`.
#[test]
fn swedish_word_coherency() {
    let _guard = engine_guard();
    let text = "Vi använder facett och fasett om varandra.";
    let matches = one(text, "SV_WORD_COHERENCY");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (23, 29));
    assert_eq!(
        matches[0].message,
        "Använd endast en av stavningsvarianterna 'fasett' och 'facett' i en och samma text."
    );
    assert_eq!(suggestions(&matches[0]), vec!["facett"]);
}

/// New coherency pairs mined from SALDO (CC BY 4.0) spelling-variant sister
/// terms (owner-added entries in `coherency.txt`): mixing the two variants
/// flags the second occurrence, single-variant text stays clean.
#[test]
fn swedish_word_coherency_saldo() {
    let _guard = engine_guard();
    for (text, from, to, range) in [
        (
            "Vi tog spagetti och spaghetti till middag.",
            "spaghetti",
            "spagetti",
            (20, 29),
        ),
        (
            "Han låssas och låtsas alltid.",
            "låtsas",
            "låssas",
            (15, 21),
        ),
        ("Ett café och ett kafé öppnade.", "kafé", "café", (17, 21)),
        ("Jag prova och pröva på det.", "pröva", "prova", (14, 19)),
    ] {
        let matches = one(text, "SV_WORD_COHERENCY");
        assert_eq!(matches.len(), 1, "{text}");
        assert_eq!(
            matches[0].message,
            format!(
                "Använd endast en av stavningsvarianterna '{from}' och '{to}' i en och samma text."
            )
        );
        assert_eq!(suggestions(&matches[0]), vec![to], "{text}");
        assert_utf16(text, &matches[0], range);
    }
    // single-variant sentences: no coherency match
    for text in [
        "Vi tog spaghetti till middag.",
        "Han låtsas att han sover.",
        "Ett kafé öppnade.",
        "Jag pröva på det.",
    ] {
        assert!(one(text, "SV_WORD_COHERENCY").is_empty(), "{text}");
    }
}

/// Confusable-word rules mined from SALDO (CC BY 4.0) and hand-curated into
/// CAT4 "Ord som ofta förväxlas" (owner-added, no Java equivalent): each wrong
/// sentence fires exactly its rule with the expected suggestion(s), each
/// corrected sentence stays clean.
#[test]
fn swedish_saldo_confusables() {
    let _guard = engine_guard();
    let rules = [
        "sväraVSsvara",
        "svärarVSsvarar",
        "bryggaVSbygga_öl",
        "bryggaVSbygga_hus",
        "antaVSinta_att",
        "intaVSanta_mat",
    ];
    let Some(sv) = engine_with_rules(&rules) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // (wrong sentence, rule, sub_id, suggestions)
    let cases: &[(&str, &str, &str, &[&str])] = &[
        (
            "Han svärde nej tack till kaffet.",
            "sväraVSsvara",
            "1",
            &["svarade"],
        ),
        (
            "Han svärar alltid ärligt på prov.",
            "svärarVSsvarar",
            "1",
            &["svarar"],
        ),
        (
            "Han bygger kaffe varje morgon.",
            "bryggaVSbygga_öl",
            "1",
            &["brygger"],
        ),
        (
            "De brygger ett nytt hus.",
            "bryggaVSbygga_hus",
            "1",
            &["bygger"],
        ),
        (
            "Jag intar att han kommer i kväll.",
            "antaVSinta_att",
            "1",
            &["antar"],
        ),
        (
            "Jag intade att det var så.",
            "antaVSinta_att",
            "1",
            &["antar", "antade"],
        ),
        ("Patienten antar mat.", "intaVSanta_mat", "1", &["intar"]),
    ];
    for (text, rule, sub, sugg) in cases {
        let matches: Vec<lt::Match> = sv
            .check(text)
            .expect("check")
            .matches
            .into_iter()
            .filter(|m| m.rule_id == *rule)
            .collect();
        assert_eq!(matches.len(), 1, "{rule}: {text}");
        assert_eq!(matches[0].sub_id.as_deref(), Some(*sub), "{text}");
        assert_eq!(suggestions(&matches[0]), sugg.to_vec(), "{text}");
    }
    // corrected sentences: none of the new rules fire
    let corrections = [
        "Han svarade nej tack till kaffet.",
        "Han svarar alltid ärligt på prov.",
        "Han brygger kaffe varje morgon.",
        "De bygger ett nytt hus.",
        "Jag antar att han kommer i kväll.",
        "Jag antade att det var så.",
        "Patienten intar mat.",
    ];
    for text in corrections {
        let result = sv.check(text).expect("check");
        let matches: Vec<&lt::Match> = result
            .matches
            .iter()
            .filter(|m| rules.contains(&m.rule_id.as_str()))
            .collect();
        assert!(matches.is_empty(), "{text}: {matches:?}");
    }
}

/// `CommaWhitespaceRule` (1), Java probe.
#[test]
fn swedish_comma_whitespace() {
    let _guard = engine_guard();
    let text = "Det är en mening , här.";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (16, 18));
    assert_eq!(
        matches[0].message,
        "Lägg till ett blanksteg efter kommatecknet, men inte före."
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DoublePunctuationRule` (2), Java probe.
#[test]
fn swedish_double_punctuation() {
    let _guard = engine_guard();
    let text = "Det är en mening..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (16, 18));
    assert_eq!(matches[0].message, "Dubbla punkter");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `GenericUnpairedBracketsRule` (3), Java probe (the match is on the opening
/// bracket, before any non-ASCII byte).
#[test]
fn swedish_unpaired_brackets() {
    let _guard = engine_guard();
    let text = "(Det är en mening.";
    let matches = one(text, "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (0, 1));
    assert_eq!(
        matches[0].message,
        "Grupperingssymboler: ')' ser ut att saknas"
    );
}

/// `UppercaseSentenceStartRule` (6), Java probe.
#[test]
fn swedish_uppercase_sentence_start() {
    let _guard = engine_guard();
    let text = "det är en mening.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (0, 3));
    assert_eq!(matches[0].message, "Meningen börjar inte med stor bokstav");
    assert_eq!(suggestions(&matches[0]), vec!["Det"]);
}

/// `MultipleWhitespaceRule` (10), Java probe.
#[test]
fn swedish_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Det  är en mening.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (3, 5));
    assert_eq!(
        matches[0].message,
        "Möjligt korrekturfel: du upprepade ett blanktecken"
    );
}

/// `SentenceWhitespaceRule` (11), Java probe -> ` Det`.
#[test]
fn swedish_sentence_whitespace() {
    let _guard = engine_guard();
    let text = "Det är en mening.Det är en till.";
    let matches = one(text, "SENTENCE_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (17, 20));
    assert_eq!(
        matches[0].message,
        "Lägg till ett blanksteg mellan meningarna."
    );
    assert_eq!(suggestions(&matches[0]), vec![" Det"]);
}

/// `HunspellRule` (4) with real Swedish misspellings. Java probe
/// (`check-diff-sv.sh`): full `getSuggestedReplacements` lists, including
/// diacritic-preserving candidates.
#[test]
fn swedish_speller() {
    let _guard = engine_guard();

    let text = "Vi tar tesst fem myror.";
    let matches = one(text, "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (7, 12));
    assert_eq!(matches[0].message, "Hittat ett möjligt stavfel.");
    assert_eq!(matches[0].match_type, "UnknownWord");
    assert_eq!(
        suggestions(&matches[0]),
        vec!["tests", "test", "estet", "restes", "stress", "tes"]
    );

    let text = "Det var myket bra.";
    let matches = one(text, "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (8, 13));
    assert_eq!(
        suggestions(&matches[0]),
        vec!["mycket", "dyket", "tyket", "myset", "byket"]
    );

    // The suggestions preserve `ä`/`ö` (`här`, `härar`, `märr`, `kärr`, ...).
    let text = "Han är härr i staden.";
    let matches = one(text, "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (7, 11));
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "herr", "här", "ärr", "härar", "härur", "härd", "häri", "härs", "märr", "härk", "kärr",
            "här-"
        ]
    );
}

/// XML rule `efter-hand` (`grammar.xml`), Java probe -> `efter hand`.
#[test]
fn swedish_xml_efter_hand() {
    let _guard = engine_guard();
    let text = "Det visade sig efterhand att orden borde särskrivas.";
    let matches = one(text, "efter-hand");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_utf16(text, &matches[0], (15, 24));
    assert_eq!(suggestions(&matches[0]), vec!["efter hand"]);
}

/// Long paragraph with real Swedish orthography. Java probe
/// (`check-diff-sv.sh`) yields exactly five matches.
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
    let matches: Vec<(String, lt::TextRange)> = sv
        .check(text)
        .expect("check")
        .matches
        .into_iter()
        .map(|m| (m.rule_id, m.range))
        .collect();
    let expected = [
        ("COMMA_PARENTHESIS_WHITESPACE", (63, 65)),
        ("DOUBLE_PUNCTUATION", (196, 198)),
        ("HUNSPELL_RULE", (212, 217)),
        ("HUNSPELL_RULE", (235, 240)),
        ("efter-hand", (268, 277)),
    ];
    assert_eq!(matches.len(), expected.len(), "{matches:?}");
    for ((rule, range), (want_rule, want_range)) in matches.iter().zip(expected) {
        assert_eq!(rule, want_rule, "{matches:?}");
        assert_utf16_range(text, *range, want_range);
    }
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
