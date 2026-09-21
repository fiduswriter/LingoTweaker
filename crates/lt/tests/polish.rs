//! Polish engine tests: stage-1 XML wiring state plus Java-probed
//! built-in-rule values.
//!
//! Stage gates follow internal development notes: this file pins the progress
//! metric and gets updated by each stage. Offsets are UTF-8 bytes (the engine
//! format); the Java probes print UTF-16 code units, converted in the comments
//! where a test exercises diacritics.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

/// One engine at a time: the Polish engines hold the tagger dictionary.
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
    Engine::builder(Lang::Pl).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Pl)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

/// Rule suggestions as plain strings.
fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Convert an engine byte offset to the UTF-16 code-unit offset the Java
/// probes print.
fn utf16(text: &str, byte: usize) -> usize {
    text[..byte].encode_utf16().count()
}

/// `(from, to)` in UTF-16 code units, for comparison with the Java probes.
fn range16(text: &str, m: &lt::Match) -> (usize, usize) {
    (utf16(text, m.range.start), utf16(text, m.range.end))
}

/// Stage state: active XML rule count, disambiguation rules and
/// `compile_failures()` = 0. Updated by each stage.
#[test]
fn polish_engine_state() {
    let _guard = engine_guard();
    let Some(pl) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    eprintln!(
        "pl state: active={} disambig={} filters={} off={} failures={:?}",
        pl.active_rule_count(),
        pl.disambig_rule_count(),
        pl.skipped_counts().filters,
        pl.skipped_counts().off_by_default,
        pl.compile_failures()
    );
    assert_eq!(pl.active_rule_count(), 1773);
    assert_eq!(pl.disambig_rule_count(), 1347);
    assert_eq!(pl.skipped_counts().filters, 0);
    assert_eq!(pl.skipped_counts().off_by_default, 13);
    assert!(
        pl.compile_failures().is_empty(),
        "{:?}",
        pl.compile_failures()
    );
}

/// `MorfologikPolishSpellerRule` (`MORFOLOGIK_RULE_PL_PL`), Java-probed with
/// `scripts/oracle/pl/probe-speller.sh` (UTF-16 offsets; the test sentences
/// are ASCII up to the offsets, so byte == code unit here).
#[test]
fn polish_speller_matches_java_probe() {
    let _guard = engine_guard();
    let Some(pl) = engine_with_rules(&["MORFOLOGIK_RULE_PL_PL"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let matches = |text: &str| -> Vec<lt::Match> {
        pl.check(text)
            .expect("check")
            .matches
            .into_iter()
            .filter(|m| m.rule_id == "MORFOLOGIK_RULE_PL_PL")
            .collect()
    };

    // Two misspellings, with the full first suggestion list (Java probe).
    let ms = matches("To jest zdanie z bledem i slowem.");
    assert_eq!(ms.len(), 2);
    assert_eq!((ms[0].range.start, ms[0].range.end), (17, 23));
    assert_eq!((ms[1].range.start, ms[1].range.end), (26, 32));
    assert_eq!(
        suggestions(&ms[0]),
        vec![
            "błędem",
            "Bledem",
            "biedę",
            "błotem",
            "biletem",
            "blatem",
            "obłędem",
            "pledem",
            "błędom",
            "baletem",
            "blefem",
            "fletem",
            "Biedę",
            "Blidę",
            "betem",
            "Bredę",
            "Kletem",
            "bleedem",
            "bletek",
            "ględę",
            "łętem",
            "ble dem",
        ]
    );
    assert_eq!(suggestions(&ms[1])[0], "słowem");

    // `tokenizingPattern`: the `Niby-` prefix is dropped, the offset moves.
    let ms = matches("Niby-czlowiek przyszedl.");
    assert_eq!(
        ms.iter()
            .map(|m| (m.range.start, m.range.end))
            .collect::<Vec<_>>(),
        vec![(5, 13), (14, 23)]
    );
    assert_eq!(suggestions(&ms[0])[0], "Człowiek");
    assert_eq!(suggestions(&ms[1])[0], "przyszedł");

    // `Quasi-naukowy` is accepted (no match); `wyklad` is matched at its
    // offset inside the sentence.
    let ms = matches("quasi-naukowy wyklad.");
    assert_eq!(
        ms.iter()
            .map(|m| (m.range.start, m.range.end))
            .collect::<Vec<_>>(),
        vec![(14, 20)]
    );

    // `isNotCompound` accepts valid compounds/prefix words (Java: no match).
    for word in [
        "trzynastobitowy",
        "supernowoczesny",
        "pseudonaukowy",
        "arcytrudny",
        "wysokoprocentowy",
        "dwudziestoczterogodzinny",
    ] {
        assert!(
            matches(word).is_empty(),
            "{word} should be accepted by isNotCompound"
        );
    }

    // The sentence-start capitalization of suggestions.
    let ms = matches("Zrobilem to wczoraj.");
    assert_eq!(suggestions(&ms[0])[0], "Zrobiłem");
}

/// `SpellingCheckRule.addIgnoreWords`: multi-word entries in
/// `core/spelling_global.txt` (`Google Maps`, `Gro Harlem Brundtland`)
/// immunize their tokens, while the last word alone is still checked.
/// Java-probed with `scripts/oracle/pl/probe-speller.sh`.
#[test]
fn polish_speller_multiword_ignore_matches_java_probe() {
    let _guard = engine_guard();
    let Some(pl) = engine_with_rules(&["MORFOLOGIK_RULE_PL_PL"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let matches = |text: &str| -> Vec<lt::Match> {
        pl.check(text)
            .expect("check")
            .matches
            .into_iter()
            .filter(|m| m.rule_id == "MORFOLOGIK_RULE_PL_PL")
            .collect()
    };
    for text in [
        "Google Maps.",
        "z Google Maps.",
        "Gro Harlem Brundtland zjadł kanapkę.",
        "New York Islanders.",
    ] {
        assert!(matches(text).is_empty(), "phrase should be ignored: {text}");
    }
    // the word alone is still reported
    assert!(!matches("Maps.").is_empty());
}

/// `WLADCE` has `suppress_misspelled="yes"`; when the tagger does not know the
/// synthesized suggestion (`Surowcę`, `stojącę`), the match is dropped
/// (Java `removeSuppressMisspelled` + the `createRuleMatch` no-suggestion
/// guard). Java-probed with `scripts/oracle/pl/probe-rule.sh`.
#[test]
fn polish_suppress_misspelled_matches_java_probe() {
    let _guard = engine_guard();
    let Some(pl) = engine_with_rules(&["WLADCE"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let check = |text: &str| -> Vec<lt::Match> {
        pl.check(text)
            .expect("check")
            .matches
            .into_iter()
            .filter(|m| m.rule_id == "WLADCE")
            .collect()
    };
    // unknown synthesized form -> whole match dropped
    assert!(check("Surowce zawierające te alkaloidy").is_empty());
    assert!(check("Zarząd określił w tym czasie zadania stojące przede nim.").is_empty());
    // known form -> match kept
    let text = "Uwielbiam tego gruzińskiego twórce ludowego.";
    let ms = check(text);
    assert_eq!(ms.len(), 1);
    assert_eq!(range16(text, &ms[0]), (28, 34));
    assert_eq!(suggestions(&ms[0]), vec!["twórcę"]);
}

/// The stage-3 Java rule classes, Java-probed with
/// `scripts/oracle/pl/probe-rule.sh` (UTF-16 offsets; ASCII prefixes).
#[test]
fn polish_stage3_rules_match_java_probe() {
    let _guard = engine_guard();
    let check = |text: &str, rule: &str| -> Vec<lt::Match> {
        let Some(pl) = engine_with_rules(&[rule]) else {
            eprintln!("skipping: no vendored data");
            return Vec::new();
        };
        pl.check(text)
            .expect("check")
            .matches
            .into_iter()
            .filter(|m| m.rule_id == rule)
            .collect()
    };

    // `PL_SIMPLE_REPLACE` (`AbstractSimpleReplaceRule` v1).
    let ms = check("Uspokój sei.", "PL_SIMPLE_REPLACE");
    assert_eq!(ms.len(), 1);
    assert_eq!(range16("Uspokój sei.", &ms[0]), (8, 11));
    assert_eq!(
        ms[0].message,
        "Wyraz „sei” to najczęściej literówka; poprawnie pisze się: się."
    );
    assert_eq!(suggestions(&ms[0]), vec!["się"]);

    // `PL_COMPOUNDS`.
    let ms = check("Witamy w Rabce Zdroju.", "PL_COMPOUNDS");
    assert_eq!(ms.len(), 1);
    assert_eq!(range16("Witamy w Rabce Zdroju.", &ms[0]), (9, 21));
    assert_eq!(ms[0].message, "Ten wyraz pisze się z łącznikiem.");
    assert_eq!(suggestions(&ms[0]), vec!["Rabce-Zdroju"]);

    // `DASH_RULE` (picky).
    let ms = check("Busko — Zdrój to miasto.", "DASH_RULE");
    assert_eq!(ms.len(), 1);
    assert_eq!(range16("Busko — Zdrój to miasto.", &ms[0]), (0, 13));
    assert_eq!(suggestions(&ms[0]), vec!["Busko-Zdrój"]);

    // `PL_WORD_REPEAT` (default off).
    let ms = check(
        "Mówiła długo, bo lubiła robić wszystko długo.",
        "PL_WORD_REPEAT",
    );
    assert_eq!(ms.len(), 1);
    assert_eq!(
        range16("Mówiła długo, bo lubiła robić wszystko długo.", &ms[0]),
        (39, 44)
    );
    assert_eq!(ms[0].message, "Powtórzony wyraz w zdaniu");

    // `PL_WORD_COHERENCY` (text level).
    let ms = check(
        "Grapefruity są zdrowe. Grejpfrut smakuje najlepiej.",
        "PL_WORD_COHERENCY",
    );
    assert_eq!(ms.len(), 1);
    assert_eq!(
        range16(
            "Grapefruity są zdrowe. Grejpfrut smakuje najlepiej.",
            &ms[0]
        ),
        (23, 32)
    );
    assert_eq!(
        ms[0].message,
        "Formy „grejpfrut” i „grapefruit” zwykle nie powinny być używane jednocześnie."
    );
    assert_eq!(suggestions(&ms[0]), vec!["Grapefruit"]);

    // `PL_UNPAIRED_BRACKETS`.
    let ms = check("To jest zdanie z „cudzysłowem.", "PL_UNPAIRED_BRACKETS");
    assert_eq!(ms.len(), 1);
    assert_eq!(range16("To jest zdanie z „cudzysłowem.", &ms[0]), (17, 18));
    assert_eq!(ms[0].message, "Brak niesparowanego symbolu: „””");
}

/// `OPATRZYC_W`: a `<match no>` naming a `<phraseref>` renders the whole
/// phrase (Java `PatternRule.elementNo` / `PatternRuleMatcher.concatMatches`),
/// and a phrase token tagged SENT_END keeps its surface (the `postag_regexp`
/// `oneForm` branch of `MatchState.toFinalString`). Java-probed with
/// `scripts/oracle/pl/probe-rule.sh`.
#[test]
fn polish_phraseref_match_matches_java_probe() {
    let _guard = engine_guard();
    let Some(pl) = engine_with_rules(&["OPATRZYC_W"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let check = |text: &str| -> Vec<lt::Match> {
        pl.check(text)
            .expect("check")
            .matches
            .into_iter()
            .filter(|m| m.rule_id == "OPATRZYC_W")
            .collect()
    };

    let text = "Prace należy opatrzyć w osobiste godło.";
    let ms = check(text);
    assert_eq!(ms.len(), 1);
    assert_eq!(range16(text, &ms[0]), (13, 38));
    assert_eq!(
        suggestions(&ms[0]),
        vec!["zaopatrzyć w osobiste godło", "opatrzyć osobistym godłem"]
    );

    let text = "Pracę opatrzyłem w podpis.";
    let ms = check(text);
    assert_eq!(ms.len(), 1);
    assert_eq!(range16(text, &ms[0]), (6, 26));
    assert_eq!(
        suggestions(&ms[0]),
        vec!["zaopatrzyłem w podpis.", "opatrzyłem podpisem."]
    );
}

/// `DWA_LUB_WIECEJ`: a message `<match no>` that yields several synthesized
/// forms (and is followed by another `<match no>`) expands every form and
/// reuses the recorded match spec for the repeated reference (Java
/// `formatMatches` `numbersToMatches`). Java-probed with
/// `scripts/oracle/pl/probe-rule.sh`.
#[test]
fn polish_multi_form_message_matches_java_probe() {
    let _guard = engine_guard();
    let Some(pl) = engine_with_rules(&["DWA_LUB_WIECEJ"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let text = "Nudził mnie przez dwie lub więcej wieczornych godzin.";
    let ms: Vec<lt::Match> = pl
        .check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == "DWA_LUB_WIECEJ")
        .collect();
    assert_eq!(ms.len(), 1);
    assert_eq!(
        suggestions(&ms[0]),
        vec![
            "co najmniej dwie wieczorne godziny",
            "co najmniej dwie wieczornych godziny",
        ]
    );
    assert_eq!(
        ms[0].message,
        "Konstrukcji tego typu nie da się użyć tak, aby nie budziła zastrzeżeń. Lepiej: \
         <suggestion>co najmniej dwie wieczorne godziny</suggestion>, \
         <suggestion>co najmniej dwie wieczornych godziny</suggestion>."
    );
}
