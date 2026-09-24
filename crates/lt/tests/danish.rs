//! Danish engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Probe offsets are the Java UTF-16 code units and are asserted with the
//! `common::assert_utf16` helper (`scripts/oracle/da/check-diff-da.sh`,
//! `docs/parity/golden/da-full.java.tsv`). The speller suggestions are the
//! full Java `HunspellRule` list; they preserve diacritics.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

mod common;
use common::{assert_utf16, assert_utf16_range};

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

/// Stage state: active XML rule count (93; 69 original + 23 rulegroups mined
/// from DanNet for the confusable-word categories, see
/// `danish_dannet_confusables`; the DANISH_TYPOS list rule rides
/// `simple_replace` and is not counted), no XML-referenced filters and
/// `compile_failures()` = 0 (Danish has no language-specific Java rules).
#[test]
fn danish_engine_state() {
    let _guard = engine_guard();
    let Some(da) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(da.active_rule_count(), 93);
    assert_eq!(da.skipped_counts().filters, 0);
    assert!(
        da.compile_failures().is_empty(),
        "{:?}",
        da.compile_failures()
    );
}

/// `CommaWhitespaceRule` (1), Java probe.
#[test]
fn danish_comma_whitespace() {
    let _guard = engine_guard();
    let text = "Han spiste et æble , og gik.";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (18, 20));
    assert_eq!(
        matches[0].message,
        "Indsæt ikke et mellemrum før komma, men efter det."
    );
    assert_eq!(suggestions(&matches[0]), vec![","]);
}

/// `DoublePunctuationRule` (2), Java probe.
#[test]
fn danish_double_punctuation() {
    let _guard = engine_guard();
    let text = "Han spiste æbler..";
    let matches = one(text, "DOUBLE_PUNCTUATION");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (16, 18));
    assert_eq!(matches[0].message, "To på hinanden følgende punktummer");
    assert_eq!(suggestions(&matches[0]), vec![".", "…"]);
}

/// `GenericUnpairedBracketsRule` (3) with the explicit Danish bracket lists,
/// Java probe (the opening bracket, before any non-ASCII byte).
#[test]
fn danish_unpaired_brackets() {
    let _guard = engine_guard();
    let text = "(Han spiste et æble.";
    let matches = one(text, "UNPAIRED_BRACKETS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (0, 1));
    assert_eq!(
        matches[0].message,
        "Ikke parret symbol: \")\" ser ud til at mangle"
    );
}

/// `UppercaseSentenceStartRule` (5), Java probe.
#[test]
fn danish_uppercase_sentence_start() {
    let _guard = engine_guard();
    let text = "det er en øl.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (0, 3));
    assert_eq!(
        matches[0].message,
        "Denne sætning starter ikke med et stort begyndelsesbogstav"
    );
    assert_eq!(suggestions(&matches[0]), vec!["Det"]);
}

/// `MultipleWhitespaceRule` (6), Java probe.
#[test]
fn danish_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Det  er en øl.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (3, 5));
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

    let text = "Dette er en øll.";
    let matches = one(text, "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (12, 15));
    assert_eq!(matches[0].message, "Mulig stavefejl fundet");
    assert_eq!(matches[0].match_type, "UnknownWord");
    assert_eq!(
        suggestions(&matches[0]),
        vec!["øl", "øls", "ørl", "All", "Oll"]
    );

    let text = "Jeg har lavet en fejll.";
    let matches = one(text, "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (17, 22));
    assert_eq!(suggestions(&matches[0]), vec!["fejl", "fejle", "fejls"]);

    let text = "Det er et stort æple.";
    let matches = one(text, "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (16, 20));
    assert_eq!(
        suggestions(&matches[0]),
        vec!["æble", "pæle", "ædle", "ækle", "ævle"]
    );
}

/// XML rule `grube` (`grammar.xml`), Java probe -> `faldgruber`.
#[test]
fn danish_xml_grube() {
    let _guard = engine_guard();
    let text = "Der er mange faldgruper i skoven.";
    let matches = one(text, "grube");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_utf16(text, &matches[0], (13, 23));
    assert_eq!(
        matches[0].message,
        "Mente du <suggestion>faldgruber</suggestion>, altså en mine eller et hul i jorden?"
    );
    assert_eq!(suggestions(&matches[0]), vec!["faldgruber"]);
}

/// XML rule `yndlings` (`grammar.xml`), Java probe -> `yndlingshold`.
#[test]
fn danish_xml_yndlings() {
    let _guard = engine_guard();
    let text = "Det var mit ynglingshold fra Århus.";
    let matches = one(text, "yndlings");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_utf16(text, &matches[0], (12, 24));
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

    // `753f.kr.`: HUNSPELL_RULE on `f.kr`
    let text = "I år 753f.kr. blev Rom grundlagt.";
    let matches = one(text, "HUNSPELL_RULE");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_utf16(text, &matches[0], (8, 12));
    assert_eq!(suggestions(&matches[0]), vec!["frk.", "f.Kr."]);

    // the XML rule `fkr.` flags the spaced form
    let text = "I år 200 fkr. Kristendommen fandtes ikke.";
    let matches = one(text, "fkr.");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), Some("1"));
    assert_utf16(text, &matches[0], (9, 13));
    assert_eq!(
        matches[0].message,
        "Før Kristus forkortes <suggestion>f.kr.</suggestion>"
    );
    assert_eq!(suggestions(&matches[0]), vec!["f.kr."]);
}

/// `DANISH_TYPOS` (owner-added, Rust-only; no Java rule classes exist for
/// Danish): a typo from `data/da/rules/typos_wikipedia.txt` flags with the
/// suggestion, and correct Danish stays clean.
#[test]
fn danish_typos_wikipedia() {
    let _guard = engine_guard();

    let text = "Det er et biblotek.";
    let matches = one(text, "DANISH_TYPOS");
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!(matches[0].sub_id.as_deref(), None);
    assert_utf16(text, &matches[0], (10, 18));
    assert_eq!(
        matches[0].message,
        "'biblotek' er en almindelig stavefejl. Mente du <suggestion>bibliotek</suggestion>?"
    );
    assert_eq!(suggestions(&matches[0]), vec!["bibliotek"]);

    // correct Danish: no DANISH_TYPOS match
    let text = "Jeg lånte bogen på biblioteket i går.";
    assert!(one(text, "DANISH_TYPOS").is_empty());
}

/// Confusable-word rulegroups mined from DanNet (the Danish wordnet, CC BY-SA
/// 4.0) and hand-curated into CAT2/CAT3 of `grammar.xml` (owner-added, no Java
/// equivalent): each wrong sentence fires exactly its rulegroup with the
/// expected suggestion(s), each corrected sentence stays clean.
#[test]
fn danish_dannet_confusables() {
    let _guard = engine_guard();
    let rules = [
        "ligge_lægge",
        "lede_lide",
        "hælde_hævde",
        "vælte_vælge",
        "byde_bøde",
        "held_helt",
        "låse_løse",
        "brygge_bygge",
        "folk_flok",
        "højde_højre",
        "leve_lave",
        "lade_lave",
        "sælge_vælge",
        "stille_spille",
        "række_trække",
        "strikke_strække",
    ];
    let Some(da) = engine_with_rules(&rules) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // (wrong sentence, rulegroup, sub_id, suggestions)
    let cases: &[(&str, &str, &str, &[&str])] = &[
        (
            "Lig mærke til, hvad han siger.",
            "ligge_lægge",
            "1",
            &["Læg"],
        ),
        (
            "Han brød sig ikke om at lede under det.",
            "lede_lide",
            "1",
            &["lide"],
        ),
        (
            "Politiet lide efter den forsvundne dreng.",
            "lede_lide",
            "2",
            &["lede"],
        ),
        (
            "Hun hælder at det ikke er hendes skyld.",
            "hælde_hævde",
            "1",
            &["hævde", "hævder"],
        ),
        (
            "Han hævder vand i glasset.",
            "hælde_hævde",
            "2",
            &["hælde", "hælder"],
        ),
        (
            "Du skal ikke vælte mellem dem.",
            "vælte_vælge",
            "1",
            &["vælge"],
        ),
        (
            "Staklen vælger omkuld.",
            "vælte_vælge",
            "2",
            &["vælte", "vælter"],
        ),
        (
            "Han fik en byde for at køre for stærkt.",
            "byde_bøde",
            "1",
            &["bøde"],
        ),
        (
            "Han bøder på den gamle vase.",
            "byde_bøde",
            "2",
            &["byde", "byder"],
        ),
        (
            "Jeg ønsker dig helt og lykke.",
            "held_helt",
            "1",
            &["held og lykke"],
        ),
        ("Han kunne ikke låse opgaven.", "låse_løse", "1", &["løse"]),
        (
            "Hun glemte at løse døren, da hun gik ud.",
            "låse_løse",
            "2",
            &["låse"],
        ),
        (
            "Han bygger kaffe hver morgen.",
            "brygge_bygge",
            "1",
            &["brygge", "brygger"],
        ),
        (
            "De brygger et nyt hus.",
            "brygge_bygge",
            "2",
            &["bygge", "bygger"],
        ),
        (
            "Der kom en folk løbende ad gaden.",
            "folk_flok",
            "1",
            &["flok"],
        ),
        ("Gå til højde ved krydset.", "højde_højre", "1", &["højre"]),
        (
            "Hun lever aftensmad til familien.",
            "leve_lave",
            "1",
            &["lave", "laver"],
        ),
        ("Lave være med at røre ved det!", "lade_lave", "1", &["Lad"]),
        (
            "Jeg laver være med at blande mig.",
            "lade_lave",
            "2",
            &["lader"],
        ),
        (
            "Hun kunne ikke sælge mellem de to hunde.",
            "sælge_vælge",
            "1",
            &["vælge"],
        ),
        (
            "Han stiller fodbold hver lørdag.",
            "stille_spille",
            "1",
            &["spille", "spiller"],
        ),
        (
            "Han spiller mange spørgsmål til politikerne.",
            "stille_spille",
            "2",
            &["stille", "stiller"],
        ),
        (
            "Tandlægen skal række tanden ud.",
            "række_trække",
            "1",
            &["trække"],
        ),
        (
            "Hun strækker en trøje til sin søn.",
            "strikke_strække",
            "1",
            &["strikke", "strikker"],
        ),
    ];
    for (text, rule, sub, sugg) in cases {
        let matches: Vec<lt::Match> = da
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
    // corrected sentences: none of the new rulegroups fire
    let corrections = [
        "Læg mærke til, hvad han siger.",
        "Han brød sig ikke om at lide under det.",
        "Politiet leder efter den forsvundne dreng.",
        "Hun hævder at det ikke er hendes skyld.",
        "Han hælder vand i glasset.",
        "Du skal vælge mellem dem.",
        "Staklen vælter omkuld.",
        "Han fik en bøde for at køre for stærkt.",
        "Han byder på den gamle vase.",
        "Jeg ønsker dig held og lykke.",
        "Han kunne ikke løse opgaven.",
        "Hun glemte at låse døren, da hun gik ud.",
        "Han brygger kaffe hver morgen.",
        "De bygger et nyt hus.",
        "Der kom en flok løbende ad gaden.",
        "Gå til højre ved krydset.",
        "Hun laver aftensmad til familien.",
        "Lad være med at røre ved det!",
        "Jeg lader være med at blande mig.",
        "Hun kunne ikke vælge mellem de to hunde.",
        "Han spiller fodbold hver lørdag.",
        "Han stiller mange spørgsmål til politikerne.",
        "Tandlægen skal trække tanden ud.",
        "Hun strikker en trøje til sin søn.",
    ];
    for text in corrections {
        let result = da.check(text).expect("check");
        let matches: Vec<&lt::Match> = result
            .matches
            .iter()
            .filter(|m| rules.contains(&m.rule_id.as_str()))
            .collect();
        assert!(matches.is_empty(), "{text}: {matches:?}");
    }
}

/// Long paragraph with real Danish orthography. Java probe
/// (`check-diff-da.sh`) yields exactly two matches.
#[test]
fn danish_long_paragraph() {
    let _guard = engine_guard();
    let Some(da) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let text = "Det var en kold vintermorgen, hvor sneen lå tungt over de gamle huse i landsbyen . En lille dreng ved navn Søren løb ud i haven for at lege med sin hund.. Han havde fået en ny slæde i julegave, og den ville han prøve straks. Moren råbte, at han skulle tage en varm trøje på, men han hørte hende ikke. Pludselig faldt han og slog sit knæ, og så måtte han alligevel ind i varmen igen.";
    let matches: Vec<(String, lt::TextRange)> = da
        .check(text)
        .expect("check")
        .matches
        .into_iter()
        .map(|m| (m.rule_id, m.range))
        .collect();
    let expected = [
        ("COMMA_PARENTHESIS_WHITESPACE", (80, 82)),
        ("DOUBLE_PUNCTUATION", (152, 154)),
    ];
    assert_eq!(matches.len(), expected.len(), "{matches:?}");
    for ((rule, range), (want_rule, want_range)) in matches.iter().zip(expected) {
        assert_eq!(rule, want_rule, "{matches:?}");
        assert_utf16_range(text, *range, want_range);
    }
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
