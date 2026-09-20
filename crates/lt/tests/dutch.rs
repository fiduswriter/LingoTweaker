//! Dutch engine tests: stage-1 XML wiring state plus Java-probed built-in-rule
//! values.
//!
//! Stage gates follow internal development notes: this file pins the
//! progress metric and gets updated by each stage. Offsets are UTF-8 bytes
//! (the engine format); the Java probes (`scripts/oracle/nl/probe-rule.sh`,
//! pinned LT build) print UTF-16 code units, converted in the comments.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

/// One engine at a time: the Dutch engines hold the tagger dictionary.
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

fn engine_variant(variant: &str) -> Option<Engine> {
    let data = data_dir()?;
    let builder = Engine::builder(Lang::Nl).ok()?;
    builder.variant(variant).data_dir(data).build().ok()
}

fn engine_with_rules(variant: &str, rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Nl)
        .ok()?
        .variant(variant)
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Stage-3b state: 3,393 active XML rules for nl-NL and 3,476 for nl-BE
/// (`BelgianDutch` adds `nl-BE/grammar.xml` + `style.xml`), 940
/// disambiguation rules, and `compile_failures()` at zero for both variants.
#[test]
fn dutch_engine_state_stage3() {
    let _guard = engine_guard();
    let Some(nl_nl) = engine_variant("nl-NL") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(nl_nl.active_rule_count(), 3393);
    assert_eq!(nl_nl.disambig_rule_count(), 940);
    assert_eq!(nl_nl.skipped_counts().filters, 0);
    assert_eq!(nl_nl.skipped_counts().off_by_default, 107);
    assert!(nl_nl.compile_failures().is_empty());

    let Some(nl_be) = engine_variant("nl-BE") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(nl_be.active_rule_count(), 3476);
    assert_eq!(nl_be.disambig_rule_count(), 940);
    assert!(nl_be.compile_failures().is_empty());
}

/// Stage-3b XML filters probed against the pinned Java build
/// (`scripts/oracle/nl/probe-rule.sh`, 2026-09-19): `DateCheckFilter`
/// (`NL_DATE_WEEKDAY`), `CompoundFilter` (`QUASI_LOS`, `PRIVE_AUTO`-style
/// glueing), `DutchNumberInWordFilter` (`CIJFERS_IN_WOORD`) and
/// `DutchSuppressMisspelledSuggestionsFilter` (`NL_AFGEBROKEN_WOORD`).
#[test]
fn dutch_stage3b_filters_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine_variant("nl-NL") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let check = |text: &str, rule: &str| -> lt::Match {
        engine
            .check(text)
            .unwrap()
            .matches
            .into_iter()
            .find(|m| m.rule_id == rule)
            .unwrap_or_else(|| panic!("{rule} did not match {text:?}"))
    };

    // DateCheckFilter: Java 0..9, "Dinsdag 7|Maandag 6"
    let m = check("Maandag 7 oktober 2014.", "NL_DATE_WEEKDAY");
    assert_eq!((m.range.start, m.range.end), (0, 9));
    assert_eq!(
        m.message,
        "7 oktober 2014 is geen maandag, maar een dinsdag."
    );
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["Dinsdag 7", "Maandag 6"]);

    // CompoundFilter: Java 14..30 "quasinauwkeurig" / 14..28 "quasi-irritant"
    let m = check("Deze regel is quasi nauwkeurig.", "QUASI_LOS");
    assert_eq!((m.range.start, m.range.end), (14, 30));
    assert_eq!(m.suggestions[0].value, "quasinauwkeurig");
    let m = check("Deze regel is quasi irritant.", "QUASI_LOS");
    assert_eq!((m.range.start, m.range.end), (14, 28));
    assert_eq!(m.suggestions[0].value, "quasi-irritant");

    // DutchNumberInWordFilter: Java 4..9 "goede"
    let m = check("Een g0ede morgen!", "CIJFERS_IN_WOORD");
    assert_eq!((m.range.start, m.range.end), (4, 9));
    assert_eq!(m.message, "Mogelijke tikfout.");
    assert_eq!(m.suggestions[0].value, "goede");

    // DutchSuppressMisspelledSuggestionsFilter: Java 3..12 / 10..27
    let m = check("De pia- nist speelt door.", "NL_AFGEBROKEN_WOORD");
    assert_eq!((m.range.start, m.range.end), (3, 12));
    assert_eq!(m.suggestions[0].value, "pianist");
    let m = check(
        "Het is de garan- tieperiode die niet klopt.",
        "NL_AFGEBROKEN_WOORD",
    );
    assert_eq!((m.range.start, m.range.end), (10, 27));
    assert_eq!(m.suggestions[0].value, "garantieperiode");
}

/// Stage-2 speller: `MorfologikDutchSpellerRule` +
/// `ignorePotentiallyMisspelledWord` (`CompoundAcceptor`), probed against
/// the pinned Java build (`scripts/oracle/nl/probe-speller.sh nl-NL`,
/// `getSpellingSuggestions`, 2026-09-19; the 61-word probe is
/// byte-identical).
#[test]
fn dutch_speller_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine_variant("nl-NL") else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let speller = |text: &str| -> Vec<lt::Match> {
        engine
            .check(text)
            .unwrap()
            .matches
            .into_iter()
            .filter(|m| m.rule_id == "MORFOLOGIK_RULE_NL_NL")
            .collect()
    };

    // ttets: the full Java suggestion list (case variants included)
    let result = engine.check("Dit is een ttets.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "MORFOLOGIK_RULE_NL_NL")
        .unwrap();
    assert_eq!((m.range.start, m.range.end), (11, 16));
    assert_eq!(m.message, "Er is een mogelijke spelfout gevonden.");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec!["toets", "Toets", "Tets", "Tiets", "tets", "Trets", "Stets", "Taets"]
    );

    // huiss: the full Java suggestion list
    let result = engine.check("De huiss staat leeg.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "MORFOLOGIK_RULE_NL_NL")
        .unwrap();
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec![
            "huis", "Huis", "huis-", "huist", "Huis'", "Huss", "Huijs", "Huise", "Huibs", "Huins",
            "Huits", "Huids", "Huigs", "Huiks"
        ]
    );

    // `ignorePotentiallyMisspelledWord` → CompoundAcceptor: accepted
    // compounds have no match, rejected ones keep the Java suggestions
    assert!(speller("De regenboog is mooi.").is_empty());
    assert!(speller("Een fietsenmaker repareert fietsen.").is_empty());
    assert!(speller("De hondenbelasting is afgeschaft.").is_empty());
    let matches = speller("De politieeenheid werkt hard.");
    assert_eq!(matches.len(), 1);
    assert_eq!((matches[0].range.start, matches[0].range.end), (3, 17));
    assert_eq!(matches[0].suggestions[0].value, "politie-eenheid");

    // English words keep the Java behavior (the `_english_ignore_` tag only
    // suppresses them where the disambiguation sets it)
    let matches = speller("Dit is a example of the government.");
    let ids: Vec<(usize, usize)> = matches
        .iter()
        .map(|m| (m.range.start, m.range.end))
        .collect();
    assert_eq!(ids, vec![(9, 16), (20, 23), (24, 34)]);
}

/// Stage-2 `CompoundAcceptor`, pinned from
/// `scripts/oracle/nl/probe-speller.sh compounds ...`
/// (`CompoundProbe`, 2026-09-19; the 24-word probe is byte-identical).
#[test]
fn dutch_compound_acceptor_matches_java() {
    let _guard = engine_guard();
    let Some(data) = data_dir() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let acceptor = lt::compound_acceptor_nl_probe(&data);
    let cases: &[(&str, bool, &[&str])] = &[
        ("regenboog", true, &["regen", "boog"]),
        ("hondenbelasting", true, &["honden", "belasting"]),
        ("politieeenheid", false, &[]),
        ("fietsenmaker", false, &[]),
        ("boekenkast", false, &[]),
        ("wetenschapsonderzoek", true, &["wetenschaps", "onderzoek"]),
        ("varkensvlees", true, &["varkens", "vlees"]),
        ("overheidsbeleid", true, &["overheids", "beleid"]),
        ("milieu-effectrapportage", false, &[]),
        ("IRA-akkoord", true, &["IRA-", "akkoord"]),
        ("MIDI-bestanden", true, &["MIDI-", "bestanden"]),
        ("WK-finalisten", true, &["WK-", "finalisten"]),
        ("Staten-Generaal", false, &[]),
        ("fiets's", false, &[]),
        ("fiets-", false, &[]),
        ("politieauto", true, &["politie", "auto"]),
        (
            "arbeidsongeschiktheidsverzekering",
            true,
            &["arbeids", "ongeschiktheidsverzekering"],
        ),
    ];
    for (word, accepted, parts) in cases {
        assert_eq!(
            acceptor.accept_compound(word),
            *accepted,
            "acceptCompound({word})"
        );
        assert_eq!(acceptor.get_parts(word), *parts, "getParts({word})");
    }
}

/// Stage-3c tokenizer/tagger: the whole
/// `scripts/oracle/nl/tagger-sentences.java.tsv` dump (185 lines, pinned
/// Java build 2026-09-19) must be reproduced byte-for-byte by
/// `lt_tokenize::DutchWordTokenizer` + the `DutchTagger` heuristics
/// (typewriter apostrophes, optional accents, hyphen parts,
/// unknown-compound tagging via `CompoundAcceptor.getParts`,
/// `ignoreSpelling`).
#[test]
fn dutch_tagger_heuristics_match_java() {
    let _guard = engine_guard();
    let Some(data) = data_dir() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture =
        match std::fs::read_to_string(root.join("scripts/oracle/nl/tagger-sentences.java.tsv")) {
            Ok(text) => text,
            Err(_) => {
                eprintln!("skipping: no tagger fixture");
                return;
            }
        };
    let sentences = std::fs::read_to_string(root.join("scripts/oracle/nl/tagger-sentences.txt"))
        .expect("sentences file");
    let tagger = lt::dutch_tagger_nl_probe(&data);
    let mut out = String::new();
    for line in sentences.lines() {
        if line.is_empty() {
            continue;
        }
        out.push_str(&format!("S\t{line}\n"));
        let tokens = lt_tokenize::DutchWordTokenizer.tokenize(line);
        let readings = tagger.tag(&tokens);
        for (token, atr) in tokens.iter().zip(readings.iter()) {
            let mut sb = String::from("T\t");
            sb.push_str(token);
            sb.push('\t');
            for (j, r) in atr.readings.iter().enumerate() {
                if j > 0 {
                    sb.push('|');
                }
                sb.push_str(r.stem.as_deref().unwrap_or("null"));
                sb.push(':');
                sb.push_str(r.pos_tag.as_deref().unwrap_or("null"));
            }
            out.push_str(&sb);
            out.push('\n');
        }
    }
    assert_eq!(out, fixture);
}

/// Stage-1 core built-ins with the Dutch `MessagesBundle_nl` strings, probed
/// against the pinned Java build (`scripts/oracle/nl/probe-rule.sh nl-NL`,
/// one rule enabled at a time, 2026-09-19). The offsets are ASCII in all
/// probes, so UTF-8 and Java UTF-16 agree.
#[test]
fn dutch_core_rules_match_java() {
    let _guard = engine_guard();

    // UPPERCASE_SENTENCE_START (4): Java 0..2, suggestion "De"
    let Some(engine) = engine_with_rules("nl-NL", &["UPPERCASE_SENTENCE_START"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("de kat zit op de mat.").unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "UPPERCASE_SENTENCE_START");
    assert_eq!((m.range.start, m.range.end), (0, 2));
    assert_eq!(m.message, "Deze zin begint niet met een hoofdletter");
    assert_eq!(m.suggestions[0].value, "De");

    // COMMA_PARENTHESIS_WHITESPACE (1): Java 15..17, suggestion "."
    let Some(engine) = engine_with_rules(
        "nl-NL",
        &["COMMA_PARENTHESIS_WHITESPACE", "DOUBLE_PUNCTUATION"],
    ) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Dit is een test .").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "COMMA_PARENTHESIS_WHITESPACE")
        .unwrap();
    assert_eq!((m.range.start, m.range.end), (15, 17));
    assert_eq!(m.message, "Zet geen spatie voor een punt");
    assert_eq!(m.suggestions[0].value, ".");

    // DOUBLE_PUNCTUATION (2): Java 6..8, suggestion ","
    let result = engine.check("Dit is,, een test.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DOUBLE_PUNCTUATION")
        .unwrap();
    assert_eq!((m.range.start, m.range.end), (6, 8));
    assert_eq!(m.message, "Twee opeenvolgende komma's");
    assert_eq!(m.suggestions[0].value, ",");

    // UNPAIRED_BRACKETS (3): Java 7..8, no suggestions
    let Some(engine) = engine_with_rules("nl-NL", &["UNPAIRED_BRACKETS"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Dit is (een test.").unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "UNPAIRED_BRACKETS");
    assert_eq!((m.range.start, m.range.end), (7, 8));
    assert_eq!(
        m.message,
        "Niet-gecombineerd symbool: \")\" lijkt te ontbreken"
    );
    assert!(m.suggestions.is_empty());

    // WHITESPACE_RULE (6, MultipleWhitespaceRule): Java 6..8, suggestion " "
    let Some(engine) = engine_with_rules("nl-NL", &["WHITESPACE_RULE"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Dit is  goed.").unwrap();
    let m = &result.matches[0];
    assert_eq!(m.rule_id, "WHITESPACE_RULE");
    assert_eq!((m.range.start, m.range.end), (6, 8));
    assert_eq!(m.message, "Te veel witruimte");
    assert_eq!(m.suggestions[0].value, " ");
}
