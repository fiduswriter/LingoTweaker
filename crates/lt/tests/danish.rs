//! Danish engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes
//! (`scripts/oracle/da/check-diff-da.sh`, `docs/parity/golden/da-full.java.tsv`)
//! print UTF-16 code units. Every probe sentence below uses real Danish
//! orthography (æ/ø/å), so the two formats differ and both are stated per
//! case. The speller suggestions are the full Java `HunspellRule` list; they
//! preserve diacritics.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

/// One engine at a time: the Danish engines hold the speller dictionary.
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
    Engine::builder(Lang::Da).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Da)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Rule matches for `text` with one rule enabled.
fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(da) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    da.check(text)
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
/// `compile_failures()` = 0 (Danish has no language-specific Java rules).
#[test]
fn danish_engine_state() {
    let _guard = engine_guard();
    let Some(da) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(da.active_rule_count(), 69);
    assert_eq!(da.skipped_counts().filters, 0);
    assert!(
        da.compile_failures().is_empty(),
        "{:?}",
        da.compile_failures()
    );
}

/// `CommaWhitespaceRule` (1), Java probe: `Han spiste et æble , og gik.`
/// UTF-16 18..20, UTF-8 19..21 (`æ` adds one byte).
#[test]
fn danish_comma_whitespace() {
    let _guard = engine_guard();
    let matches = one(
        "Han spiste et æble , og gik.",
        "COMMA_PARENTHESIS_WHITESPACE",
    );
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 19);
    assert_eq!(matches[0].range.end, 21);
    assert_eq!(
        matches[0].message,
        "Indsæt ikke et mellemrum før komma, men efter det."
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DoublePunctuationRule` (2), Java probe: `Han spiste æbler..`
/// UTF-16 16..18, UTF-8 17..19.
#[test]
fn danish_double_punctuation() {
    let _guard = engine_guard();
    let matches = one("Han spiste æbler..", "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 17);
    assert_eq!(matches[0].range.end, 19);
    assert_eq!(matches[0].message, "To på hinanden følgende punktummer");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `GenericUnpairedBracketsRule` (3) with the explicit Danish bracket lists,
/// Java probe: `(Han spiste et æble.` 0..1 (the opening bracket, before any
/// non-ASCII byte).
#[test]
fn danish_unpaired_brackets() {
    let _guard = engine_guard();
    let matches = one("(Han spiste et æble.", "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 1);
    assert_eq!(
        matches[0].message,
        "Ikke parret symbol: \")\" ser ud til at mangle"
    );
}

/// `UppercaseSentenceStartRule` (5), Java probe: `det er en øl.` 0..3.
#[test]
fn danish_uppercase_sentence_start() {
    let _guard = engine_guard();
    let matches = one("det er en øl.", "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 0);
    assert_eq!(matches[0].range.end, 3);
    assert_eq!(
        matches[0].message,
        "Denne sætning starter ikke med et stort begyndelsesbogstav"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Det"]);
}

/// `MultipleWhitespaceRule` (6), Java probe: `Det  er en øl.` 3..5.
#[test]
fn danish_multiple_whitespace() {
    let _guard = engine_guard();
    let matches = one("Det  er en øl.", "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 3);
    assert_eq!(matches[0].range.end, 5);
    assert_eq!(
        matches[0].message,
        "Mulig slåfejl: du har gentaget et mellemrum"
    );
}

/// `HunspellRule` (4) with real Danish misspellings. Java probe
/// (`check-diff-da.sh`): full `getSuggestedReplacements` lists, which
/// preserve æ/ø/å.
#[test]
fn danish_speller() {
    let _guard = engine_guard();

    // `øll`: Java UTF-16 12..15, UTF-8 12..16 (`ø` adds one byte)
    let matches = one("Dette er en øll.", "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 12);
    assert_eq!(matches[0].range.end, 16);
    assert_eq!(matches[0].message, "Mulig stavefejl fundet");
    assert_eq!(matches[0].match_type, "UnknownWord");
    assert_eq!(
        suggestions(&matches[0]),
        vec!["øl", "øls", "ørl", "All", "Oll"]
    );

    let matches = one("Jeg har lavet en fejll.", "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 17);
    assert_eq!(matches[0].range.end, 22);
    assert_eq!(suggestions(&matches[0]), vec!["fejl", "fejle", "fejls"]);

    // `æple`: Java UTF-16 16..20, UTF-8 16..21 (`æ` adds one byte)
    let matches = one("Det er et stort æple.", "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 16);
    assert_eq!(matches[0].range.end, 21);
    assert_eq!(
        suggestions(&matches[0]),
        vec!["æble", "pæle", "ædle", "ækle", "ævle"]
    );
}

/// XML rule `grube` (`grammar.xml`), Java probe: `Der er mange faldgruper i
/// skoven.` 13..23 (all-ASCII span) -> `faldgruber`.
#[test]
fn danish_xml_grube() {
    let _guard = engine_guard();
    let matches = one("Der er mange faldgruper i skoven.", "grube");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 13);
    assert_eq!(matches[0].range.end, 23);
    assert_eq!(
        matches[0].message,
        "Mente du <suggestion>faldgruber</suggestion>, altså en mine eller et hul i jorden?"
    );
    assert_eq!(suggestions(&matches[0]), vec!["faldgruber"]);
}

/// XML rule `yndlings` (`grammar.xml`), Java probe: `Det var mit
/// ynglingshold fra Århus.` 12..24 (all-ASCII span) -> `yndlingshold`.
#[test]
fn danish_xml_yndlings() {
    let _guard = engine_guard();
    let matches = one("Det var mit ynglingshold fra Århus.", "yndlings");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 12);
    assert_eq!(matches[0].range.end, 24);
    assert_eq!(
        matches[0].message,
        "Udtrykket staves <suggestion>yndlingshold</suggestion> ."
    );
    assert_eq!(suggestions(&matches[0]), vec!["yndlingshold"]);
}

/// Dotted abbreviations (the recent legacy-token suppression fix): a known
/// abbreviation (`ca.`) is not spell-checked, while the unknown `f.kr` inside
/// `753f.kr.` is flagged as one run (not hidden by the ignored `f` token).
/// Java probe `check-diff-da.sh`; also matches `da-full.java.tsv` lines
/// 174-179.
#[test]
fn danish_dotted_abbreviations() {
    let _guard = engine_guard();

    // known abbreviation: no speller match
    assert!(
        one("Der er ca. en liter mælk.", "HUNSPELL_RULE").is_empty(),
        "ca. must not be flagged"
    );

    // `753f.kr.`: HUNSPELL_RULE on `f.kr`, Java UTF-16 8..12 / UTF-8 9..13
    let matches = one("I år 753f.kr. blev Rom grundlagt.", "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].range.start, 9);
    assert_eq!(matches[0].range.end, 13);
    assert_eq!(suggestions(&matches[0]), vec!["frk.", "f.Kr."]);

    // the XML rule `fkr.` flags the spaced form, Java UTF-16 9..13 / UTF-8 10..14
    let matches = one("I år 200 fkr. Kristendommen fandtes ikke.", "fkr.");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_eq!(matches[0].range.start, 10);
    assert_eq!(matches[0].range.end, 14);
    assert_eq!(
        matches[0].message,
        "Før Kristus forkortes <suggestion>f.kr.</suggestion>"
    );
    assert_eq!(suggestions(&matches[0]), vec!["f.kr."]);
}

/// Long paragraph with real Danish orthography. Java probe
/// (`check-diff-da.sh`) yields exactly two matches (UTF-8 offsets; UTF-16 in
/// parentheses).
#[test]
fn danish_long_paragraph() {
    let _guard = engine_guard();
    let Some(da) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let text = "Det var en kold vintermorgen, hvor sneen lå tungt over de gamle huse i landsbyen . En lille dreng ved navn Søren løb ud i haven for at lege med sin hund.. Han havde fået en ny slæde i julegave, og den ville han prøve straks. Moren råbte, at han skulle tage en varm trøje på, men han hørte hende ikke. Pludselig faldt han og slog sit knæ, og så måtte han alligevel ind i varmen igen.";
    let matches: Vec<(String, usize, usize)> = da
        .check(text)
        .expect("check")
        .matches
        .into_iter()
        .map(|m| (m.rule_id, m.range.start, m.range.end))
        .collect();
    assert_eq!(
        matches,
        vec![
            ("COMMA_PARENTHESIS_WHITESPACE".into(), 81, 83),
            ("DOUBLE_PUNCTUATION".into(), 155, 157),
        ],
        "{matches:?}"
    );
}

/// The `BaseTagger`-derived `DanishTagger` tags known words and leaves
/// unknown ones untagged.
#[test]
fn danish_tagger() {
    let _guard = engine_guard();
    let Some(da) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let sentences = da.analyze("Han blev født i 1920'erne.");
    let readings: Vec<&lt::AnalyzedToken> = sentences
        .iter()
        .flat_map(|s| s.tokens.iter())
        .flat_map(|t| t.readings.iter())
        .filter(|r| !matches!(r.pos_tag.as_deref(), Some("SENT_START" | "SENT_END")))
        .collect();
    let tagged = readings.iter().filter(|r| r.pos_tag.is_some()).count();
    assert!(tagged > 0, "no tagged readings: {readings:?}");
}
