//! Engine-level tests for the German pipeline. Expected values were probed
//! against the pinned Java build in Docker (individual-rule probes, see
//! `scripts/oracle/de/probe-rule.sh`); no German corpus statistics are run.

use std::sync::{Arc, Mutex, OnceLock};

use lt::{Engine, Lang};

/// Engines are built per enabled-rule list (text-level and overlapping rules
/// are sensitive to which rules run together, so tests must not share one
/// engine). A single-entry cache is enough to keep repeated keys cheap while
/// capping the process memory: each engine costs ~1 GB and ~4 s to build
/// (D-033), and the previous engine is dropped before the next one is built.
/// German engine builds are memory-heavy (~1 GB each, D-033); this guard
/// serializes the tests so at most one engine is alive at a time. Poisoning
/// is ignored so a failing test does not cascade.
fn engine_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn de_engine(enabled: &[&str]) -> Option<Arc<Engine>> {
    type Cache = Mutex<Option<(String, Arc<Engine>)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = enabled.join(",");
    let mut guard = CACHE.get_or_init(|| Mutex::new(None)).lock().unwrap();
    if let Some((cached_key, engine)) = guard.as_ref() {
        if *cached_key == key {
            return Some(Arc::clone(engine));
        }
    }
    // release the previous engine before allocating the next one
    *guard = None;
    let build = || {
        let Ok(builder) = Engine::builder(Lang::De) else {
            eprintln!("skipping: no vendored data found");
            return None;
        };
        if enabled.is_empty() {
            return builder.build().ok().map(Arc::new);
        }
        let options = lt::EngineOptions {
            enabled_rules: enabled.iter().map(|s| s.to_string()).collect(),
            enabled_only: true,
            picky: true,
            ..Default::default()
        };
        builder.options(options).build().ok().map(Arc::new)
    };
    let engine = build()?;
    *guard = Some((key, Arc::clone(&engine)));
    Some(engine)
}

#[test]
fn wieder_vs_wider_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_WIEDER_VS_WIDER"]) else {
        return;
    };
    let text = "Das spiegelt die Situation in Deutschland wieder. \
                Bitte spiegeln Sie das wieder.";
    let result = engine.check(text).unwrap();
    let matches: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_WIEDER_VS_WIDER")
        .collect();
    // Java probe: 42..48 and 73..79 (ASCII text, UTF-16 == UTF-8)
    assert_eq!(matches.len(), 2, "{matches:?}");
    assert_eq!(matches[0].range.start, 42);
    assert_eq!(matches[0].range.end, 48);
    assert_eq!(matches[0].suggestions[0].value, "wider");
    assert_eq!(matches[1].range.start, 73);
    assert_eq!(matches[1].range.end, 79);
    assert_eq!(
        matches[0].message,
        "'wider' in 'widerspiegeln' wird mit 'i' statt mit 'ie' geschrieben, \
         z.B. 'Das spiegelt die Situation gut wider.'"
    );
}

#[test]
fn du_upper_lower_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_DU_UPPER_LOWER"]) else {
        return;
    };
    let text = "Wie geht es Dir? Bist du wieder gesund? Kannst Du mir helfen? \
                Bitte sag du es ihm.";
    let result = engine.check(text).unwrap();
    let matches: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_DU_UPPER_LOWER")
        .collect();
    // Java probe: 22..24 and 72..74, both suggesting "Du"
    assert_eq!(matches.len(), 2, "{matches:?}");
    assert_eq!((matches[0].range.start, matches[0].range.end), (22, 24));
    assert_eq!((matches[1].range.start, matches[1].range.end), (72, 74));
    assert_eq!(matches[0].suggestions[0].value, "Du");
    assert_eq!(
        matches[0].message,
        "Vorher wurde bereits 'Dir' großgeschrieben. Aus Gründen der \
         Einheitlichkeit 'Du' hier auch großschreiben?"
    );
}

#[test]
fn similar_names_is_default_off_and_fires_when_enabled() {
    let _guard = engine_guard();
    let text = "Angela Müller ist CEO. Miller wurde in Hamburg geboren.";
    let Some(default_engine) = de_engine(&[]) else {
        return;
    };
    assert!(
        default_engine
            .check(text)
            .unwrap()
            .matches
            .iter()
            .all(|m| m.rule_id != "DE_SIMILAR_NAMES"),
        "DE_SIMILAR_NAMES must be default off"
    );
    let Some(engine) = de_engine(&["DE_SIMILAR_NAMES"]) else {
        return;
    };
    let result = engine.check(text).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DE_SIMILAR_NAMES")
        .expect("similar name match");
    // Java probe: 23..29 in UTF-16 code units
    assert_eq!(lt::to_utf16_range(text, m.range), (23, 29));
    assert_eq!(m.suggestions[0].value, "Müller");
    assert_eq!(
        m.message,
        "'Miller' ähnelt dem vorher benutzten 'Müller', handelt es sich evtl. \
         um einen Tippfehler?"
    );
}

#[test]
fn german_speller_matches_java_decisions() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["GERMAN_SPELLER_RULE"]) else {
        return;
    };
    // Java probe (`ProbeSpeller`): isMisspelled decisions
    let result = engine.check("Das ist ein Hajs.").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "GERMAN_SPELLER_RULE")
        .expect("misspelling");
    assert_eq!((m.range.start, m.range.end), (12, 16));
    assert_eq!(m.message, "Möglicher Tippfehler gefunden.");
    assert!(m.suggestions.iter().any(|s| s.value == "Haus"));
    for correct in ["Das Haus ist groß.", "Der Sonnenuntergang war schön."] {
        let result = engine.check(correct).unwrap();
        assert!(
            result
                .matches
                .iter()
                .all(|m| m.rule_id != "GERMAN_SPELLER_RULE"),
            "{correct}: {:?}",
            result.matches
        );
    }
}

#[test]
fn german_speller_tokenization_and_splits_match_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["GERMAN_SPELLER_RULE"]) else {
        return;
    };
    // Java probe (ProbeRule): the German `nonWordPattern` splits a hyphen
    // followed by a digit, so `Wenera-7` covers only `Wenera` (22..28).
    let result = engine
        .check("Die sowjetische Sonde Wenera-7 („Venus 7“) war die erste, welche auf der Venus landete.")
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "GERMAN_SPELLER_RULE")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (22, 28));
    let values: Vec<&str> = m[0].suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec![
            "Genera", "General", "Wiener", "Jener", "Weder", "Werner", "Seneca", "Weber",
            "Ventral", "Weser", "Cetera", "Väter", "Menora", "Verena", "Tenero", "Webers", "Werra",
            "Venen", "Wienere"
        ]
    );
    // Java probe: `COVID` is accepted by the speller and `COVID-19-Pandemie`
    // produces no match at all.
    let result = engine
        .check("Im Zuge der COVID-19-Pandemie wurde der US-Starttermin allerdings zunächst auf den 1. April 2032 verschoben.")
        .unwrap();
    assert!(result
        .matches
        .iter()
        .all(|m| m.rule_id != "GERMAN_SPELLER_RULE"));
    // Java probe: the `thanky ou` split branch requires
    // `acceptSuggestion("al a")`, which the German filter rejects — the
    // regular `la` suggestion list is used instead.
    let result = engine.check("Paso a la Inmortalidad del General.").unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "GERMAN_SPELLER_RULE" && m.range.start == 7)
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (7, 9));
    let values: Vec<&str> = m[0].suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec![
            "LA", "als", "das", "war", "hat", "man", "da", "gab", "kam", "was", "Mai", "Bau",
            "dar", "lag", "sah", "van", "Bad", "San", "Tag", "alt"
        ]
    );
    // Java probe: `StringUtils.removeEnd` strips only one infix `s`; the
    // `Zweifeldass` candidate list must match exactly.
    let result = engine
        .check("Es besteht kein Zweifeldass das geht.")
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "GERMAN_SPELLER_RULE")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (16, 27));
    let values: Vec<&str> = m[0].suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec![
            "Zweifel dass",
            "Zweifeldakts",
            "Zweifeltest",
            "Zweifeldmaßes",
            "Zweiverlass",
            "Zweideltas",
            "Zweigoldass",
            "Zweibeatass"
        ]
    );
}

#[test]
fn german_potential_compound_filter_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["US_ARMEE", "ZUM_FEM_NOMEN", "DEN_DEM"]) else {
        return;
    };
    // Java probes (CheckDump): the filter's joined word is checked with the
    // spelling rule's full `match` (incl. `ignorePotentiallyMisspelledWord`),
    // so `Kryptomarktplatzes` et al. are accepted compounds.
    type Case<'a> = (&'a str, &'a str, (usize, usize), &'a [&'a str], &'a str);
    let cases: &[Case] = &[
        (
            "wenn die jetzigen Stärken des Krypto Marktplatzes.",
            "US_ARMEE",
            (30, 49),
            &["Kryptomarktplatzes"],
            "Dieses Nomen wird in der Regel zusammengeschrieben.",
        ),
        (
            "Wir fahren zum Rohkost Frühstück.",
            "ZUM_FEM_NOMEN",
            (15, 32),
            &["Rohkostfrühstück"],
            "Dieses Nomen wird in der Regel zusammengeschrieben.",
        ),
        (
            "Am 11. November 1885 lief die Germania abends bei dichtem Nebel bei den Poller Köpfen oberhalb von Köln-Deutz auf Grund und schlug leck.",
            "DEN_DEM",
            (72, 85),
            &["Pollerköpfen"],
            "Dieses Wort wird in der Regel zusammengeschrieben.",
        ),
        (
            "Suchen Sie einfach bei den großen Preisvergleich Portalen gleichzeitig!",
            "DEN_DEM",
            (34, 57),
            &["Preisvergleich-Portalen", "Preisvergleichportalen"],
            "Dieses Wort wird in der Regel zusammengeschrieben.",
        ),
    ];
    for (sentence, rule_id, (from, to), suggestions, message) in cases {
        let result = engine.check(sentence).unwrap();
        let m: Vec<_> = result
            .matches
            .iter()
            .filter(|m| m.rule_id == *rule_id)
            .collect();
        assert_eq!(m.len(), 1, "{sentence}: {m:?}");
        assert_eq!(
            (m[0].range.start, m[0].range.end),
            utf16_to_utf8_range(sentence, (*from, *to)),
            "{sentence}"
        );
        let values: Vec<&str> = m[0].suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{sentence}");
        assert_eq!(m[0].message, *message, "{sentence}");
    }
}

#[test]
fn de_comma_whitespace_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["COMMA_PARENTHESIS_WHITESPACE"]) else {
        return;
    };
    let text = "Die Partei , die die letzte Wahl gewann.";
    let result = engine.check(text).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "COMMA_PARENTHESIS_WHITESPACE")
        .expect("comma whitespace match");
    // Java probe: 10..12
    assert_eq!((m.range.start, m.range.end), (10, 12));
    assert_eq!(
        m.message,
        "Nur hinter einem Komma steht ein Leerzeichen, aber nicht davor."
    );
    assert_eq!(m.suggestions[0].value, ",");

    // Java probe: the synthetic SENT_START entry counts as whitespace
    // (`StringTools.isWhitespace("")`), so a sentence-initial comma takes the
    // "space after comma" branch.
    let result = engine.check(",Nicht").unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "COMMA_PARENTHESIS_WHITESPACE")
        .expect("sentence-initial comma match");
    assert_eq!((m.range.start, m.range.end), (0, 1));
    assert_eq!(
        m.message,
        "Nur hinter einem Komma steht ein Leerzeichen, aber nicht davor."
    );
    assert_eq!(m.suggestions[0].value, ", ");
}

#[test]
fn german_goal_specific_rules_inactive() {
    let _guard = engine_guard();
    // Java `isRuleActiveForLevelAndToneTags` with the default tone-tag set:
    // a rule that carries tone tags and is goal-specific stays inactive even
    // when explicitly enabled (ProbeRule with TSCHULDIGUNG returns nothing).
    let Some(engine) = de_engine(&["TSCHULDIGUNG"]) else {
        return;
    };
    let result = engine.check("Tschuldigung!").unwrap();
    assert!(result.matches.iter().all(|m| m.rule_id != "TSCHULDIGUNG"));
}

#[test]
fn german_case_rule_emoji_antipattern_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_CASE"]) else {
        return;
    };
    // Java probes: the surrogate-pair ranges U+1F000-U+1F7FF and
    // U+2600-U+27FF immunize the capitalized word; U+1F900 does not.
    for emoji in ["🙂", "😀", "🚀", "🟀", "🀀", "☀", "➿"] {
        let text = format!("Hallo {emoji} Viel Erfolg");
        assert!(
            engine.check(&text).unwrap().matches.is_empty(),
            "DE_CASE should be immunized: {text}"
        );
    }
    let text = "Hallo 🤀 Viel Erfolg";
    let matches: Vec<_> = engine
        .check(text)
        .unwrap()
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_CASE")
        .map(|m| (m.range.start, m.range.end))
        .collect();
    assert_eq!(matches, vec![(11, 15)], "{text}");
}

#[test]
fn de_double_punctuation_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_DOUBLE_PUNCTUATION"]) else {
        return;
    };
    let text = "Sein Vater ist Regierungsrat a. D..";
    let result = engine.check(text).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DE_DOUBLE_PUNCTUATION")
        .expect("double punctuation match");
    // Java probe: 33..35
    assert_eq!((m.range.start, m.range.end), (33, 35));
    assert_eq!(
        m.message,
        "Zwei aufeinander folgende Punkte. Auch wenn ein Satz mit einer \
         Abkürzung endet, endet er nur mit einem Punkt (§103 Regelwerk)."
    );
    let suggestions: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(suggestions, vec![".", "…"]);
}

#[test]
fn de_sentence_whitespace_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_SENTENCE_WHITESPACE"]) else {
        return;
    };
    let text = "Hier steht ein Satz.Das ist ein weiterer Satz.";
    let result = engine.check(text).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "DE_SENTENCE_WHITESPACE")
        .expect("sentence whitespace match");
    // Java probe: 20..23, suggestion " Das"
    assert_eq!((m.range.start, m.range.end), (20, 23));
    assert_eq!(m.message, "Fügen Sie zwischen Sätzen ein Leerzeichen ein.");
    assert_eq!(m.suggestions[0].value, " Das");
}

#[test]
fn missing_verb_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["MISSING_VERB"]) else {
        return;
    };
    let text = "In diesem Satz kein Wort. Vielen Dank für alles.";
    let result = engine.check(text).unwrap();
    let matches: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "MISSING_VERB")
        .collect();
    // Java probe: one match 0..25 for the first sentence; "Vielen Dank" is
    // the rule's internal special case.
    assert_eq!(matches.len(), 1, "{matches:?}");
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 25));
    assert_eq!(
        matches[0].message,
        "Dieser Satz scheint kein Verb zu enthalten"
    );
}

/// `GermanChunker` chunk tags on the Java-probed sentence set
/// (`scripts/oracle/de/probe-chunk.sh`; 1,617 token lines, 0 diffs).
#[test]
fn german_chunker_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[]) else {
        return;
    };
    // (sentence, expected `surface:chunk|chunk` pairs for non-O tokens,
    // verbatim from the Java probe)
    let cases: &[(&str, &[&str])] = &[
        ("Ein Haus", &["Ein:B-NP|NPS", "Haus:I-NP|NPS"]),
        (
            "Ein Hund und eine Katze stehen dort",
            &[
                "Ein:B-NP|NPP",
                "Hund:I-NP|NPP",
                "und:NPP",
                "eine:B-NP|NPP",
                "Katze:I-NP|NPP",
            ],
        ),
        (
            "Es war die größte und erfolgreichste Erfindung",
            &[
                "die:NPS",
                "größte:B-NP|NPS",
                "und:I-NP|NPS",
                "erfolgreichste:B-NP|I-NP|NPS",
                "Erfindung:I-NP|NPS",
            ],
        ),
        (
            "Geräte, deren Bestimmung und Funktion unklar sind.",
            &[
                "Geräte:B-NP",
                "deren:B-NP|NPS",
                "Bestimmung:I-NP|B-NP|NPS",
                "und:I-NP|NPS",
                "Funktion:B-NP|I-NP|NPS",
            ],
        ),
        (
            "Julia und Karsten sind alt",
            &["Julia:NPP", "und:NPP", "Karsten:NPP"],
        ),
        (
            "Das sind 37 Prozent",
            &["37:NPS|NPP", "Prozent:B-NP|NPS|NPP"],
        ),
        (
            "Laut den meisten Quellen ist er 35 Jahre alt.",
            &[
                "Laut:B-NP|PP",
                "den:B-NP|PP",
                "meisten:I-NP|PP",
                "Quellen:I-NP|PP",
                "Jahre:B-NP",
            ],
        ),
        (
            "Autor der ersten beiden Bücher ist Stephen King",
            &[
                "Autor:B-NP|NPS",
                "der:B-NP|NPS",
                "ersten:NPS",
                "beiden:B-NP|NPS",
                "Bücher:I-NP|NPS",
                "King:B-NP|NPS",
            ],
        ),
    ];
    for (sentence, expected) in cases {
        let analyzed = engine.analyze(sentence);
        assert_eq!(analyzed.len(), 1, "{sentence}");
        let mut got: Vec<String> = Vec::new();
        for token in &analyzed[0].tokens {
            if token.chunk_tags.is_empty() || token.chunk_tags == ["O"] {
                continue;
            }
            got.push(format!(
                "{}:{}",
                token.surface(),
                token.chunk_tags.join("|")
            ));
        }
        let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
        assert_eq!(got, expected, "{sentence}");
    }
}

#[test]
fn german_unify_disambiguation_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[]) else {
        return;
    };
    // Java probe (`ProbeChunk`, de-DE): after UNIFY_DET_ADJ_SUB the shared
    // number/case/gender features must prune the Tag readings to nominative;
    // Rust kept an extra `SUB:AKK:SIN:NEU` before the unifier map-sharing fix.
    let sentence = &engine.analyze("Das ist ein schöner Tag.")[0];
    let readings = |surface: &str| -> Vec<String> {
        sentence
            .tokens
            .iter()
            .find(|t| t.readings.first().map(|r| r.token.as_str()) == Some(surface))
            .map(|t| {
                t.readings
                    .iter()
                    .map(|r| r.pos_tag.clone().unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(
        readings("ein"),
        ["ART:IND:NOM:SIN:MAS", "ART:IND:NOM:SIN:NEU"]
    );
    assert_eq!(
        readings("schöner"),
        ["ADJ:NOM:SIN:MAS:GRU:IND", "ADJ:NOM:SIN:MAS:GRU:SOL"]
    );
    assert_eq!(readings("Tag"), ["SUB:NOM:SIN:MAS", "SUB:NOM:SIN:NEU"]);
}

#[test]
fn german_word_repeat_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[]) else {
        return;
    };
    // Java probe: "In diesem Satz ist ist ein Wort doppelt." 15..22,
    // suggestion "ist"; "Gestern traf sie sie." 13..20; the antipattern and
    // ignore cases produce no match.
    let result = engine
        .check("In diesem Satz ist ist ein Wort doppelt.")
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "GERMAN_WORD_REPEAT_RULE")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (15, 22));
    assert_eq!(
        m[0].message,
        "Möglicher Tippfehler: ein Wort wird wiederholt"
    );
    assert_eq!(m[0].short_message.as_deref(), Some("Wortwiederholung"));
    assert_eq!(m[0].suggestions[0].value, "ist");
    assert_eq!(m[0].category_id, "REDUNDANCY");

    for text in [
        "Warum fragen Sie sie nicht selbst?",
        "Wahrscheinlich ist das das Problem.",
        "Alle die die Leute kamen.",
        "Dann warfen sie sie weg.",
        "Er gab ihr ihr Buch zurück.",
        "Falls das das Problem ist, dann gut.",
        "Die markierten Stellen stellen die Aufnahmepunkte dar.",
        "Dann konnte sie sie sehen.",
    ] {
        let result = engine.check(text).unwrap();
        assert!(
            !result
                .matches
                .iter()
                .any(|m| m.rule_id == "GERMAN_WORD_REPEAT_RULE"),
            "{text}"
        );
    }
}

#[test]
fn german_word_repeat_beginning_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[]) else {
        return;
    };
    // Java probe: adverb variant 18..22; word variant 45..49 / 30..33
    let text = "Auch ist es kalt. Auch ist es nass.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "GERMAN_WORD_REPEAT_BEGINNING_RULE")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (18, 22));
    assert_eq!(
        m[0].message,
        "Zwei aufeinanderfolgende Sätze beginnen mit dem gleichen Adverb. \
         Evtl. können Sie den Satz umformulieren, zum Beispiel, indem Sie ein Synonym nutzen."
    );
    assert_eq!(
        m[0].short_message.as_deref(),
        Some("Zwei aufeinanderfolgende Sätze beginnen mit dem gleichen Adverb.")
    );
    assert!(m[0].suggestions.is_empty());

    let text = "Dann hatten wir Freizeit. Dann gab es Essen. Dann gingen wir schlafen.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "GERMAN_WORD_REPEAT_BEGINNING_RULE")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (45, 49));
    assert_eq!(
        m[0].short_message.as_deref(),
        Some("Drei aufeinanderfolgende Sätze beginnen mit dem gleichen Wort.")
    );
    assert_eq!(m[0].category_name, "Wiederholungen (Stil)");
}

#[test]
fn german_repeated_words_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[]) else {
        return;
    };
    // Java probe: "Das ist leider so. Das ist leider schade." 27..33 with the
    // `leider` synonyms; no match for words without synonyms.txt entries.
    let result = engine
        .check("Das ist leider so. Das ist leider schade.")
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_REPEATEDWORDS")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (27, 33));
    let suggestions: Vec<&str> = m[0].suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        suggestions,
        vec![
            "bedauerlicherweise",
            "ungünstigerweise",
            "enttäuschenderweise"
        ]
    );
    assert_eq!(
        m[0].short_message.as_deref(),
        Some("Stil: Wortwiederholung")
    );
    for text in [
        "Das ist gut. Das ist gut.",
        "Er läuft schnell. Sie rennt schnell.",
    ] {
        let result = engine.check(text).unwrap();
        assert!(
            !result
                .matches
                .iter()
                .any(|m| m.rule_id == "DE_REPEATEDWORDS"),
            "{text}"
        );
    }
}

#[test]
fn german_paragraph_repeat_beginning_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["GERMAN_PARAGRAPH_REPEAT_BEGINNING_RULE"]) else {
        return;
    };
    let text = "Die Katze ist da.\n\nDie Katze ist weg.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "GERMAN_PARAGRAPH_REPEAT_BEGINNING_RULE")
        .collect();
    // Java probe: 0..9 and 19..28
    assert_eq!(m.len(), 2, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (0, 9));
    assert_eq!((m[1].range.start, m[1].range.end), (19, 28));
    assert_eq!(
        m[0].message,
        "Der Absatz beginnt so wie der vorhergehende Absatz"
    );
    // default off: not enabled without the explicit rule id
    let engine_default = de_engine(&[]).unwrap();
    let result = engine_default.check(text).unwrap();
    assert!(!result
        .matches
        .iter()
        .any(|m| m.rule_id == "GERMAN_PARAGRAPH_REPEAT_BEGINNING_RULE"));
}

#[test]
fn german_wrong_word_in_context_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[]) else {
        return;
    };
    // Java probe: GERMAN_WRONG_WORD_IN_CONTEXT_MIENE_MINE 4..9
    let result = engine
        .check("Die Miene vom Kugelschreiber ist leer.")
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id.starts_with("GERMAN_WRONG_WORD_IN_CONTEXT"))
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (4, 9));
    assert_eq!(
        m[0].message,
        "Mögliche Wortverwechslung: Meinten Sie <suggestion>Mine</suggestion> \
         (= unterirdischer Gang, Sprengkörper, Kugelschreibermine) anstatt 'Miene' (= Gesichtsausdruck)?"
    );
    assert_eq!(m[0].suggestions[0].value, "Mine");
}

#[test]
fn german_dash_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[]) else {
        return;
    };
    // Java probe: DE_DASH 21..37 with both suggestions
    let text = "Bundestag beschließt Diäten- Erhöhung";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_DASH")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    // Java probe (UTF-16) 21..37
    assert_eq!(lt::to_utf16_range(text, m[0].range), (21, 37));
    let suggestions: Vec<&str> = m[0].suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(suggestions, vec!["Diäten-Erhöhung", "Diäten-, Erhöhung"]);
    // "NORD- UND SÜDKOREA" is ignored
    let result = engine.check("Das ist NORD- UND SÜDKOREA").unwrap();
    assert!(!result.matches.iter().any(|m| m.rule_id == "DE_DASH"));
}

#[test]
fn german_word_coherency_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[]) else {
        return;
    };
    // Java probe: DE_WORD_COHERENCY 38..46
    let text = "Die Delfine gehören zu den Zahnwalen. \
                Delphine sind in allen Meeren verbreitet.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_WORD_COHERENCY")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    // Java probe (UTF-16) 38..46
    assert_eq!(lt::to_utf16_range(text, m[0].range), (38, 46));
    assert_eq!(
        m[0].message,
        "'Delphin' und 'Delfin' sollten nicht gleichzeitig benutzt werden."
    );
    assert_eq!(m[0].suggestions[0].value, "Delfine");
}

#[test]
fn german_compound_coherency_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[]) else {
        return;
    };
    // Java probe: DE_COMPOUND_COHERENCY 67..77, no suggestion (inflected)
    let text = "Ein Helpdesk gliedert sich in verschiedene Level. \
                Die Qualität des Help-Desks ist wichtig.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_COMPOUND_COHERENCY")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    // Java probe (UTF-16) 67..77
    assert_eq!(lt::to_utf16_range(text, m[0].range), (67, 77));
    assert_eq!(
        m[0].message,
        "Uneinheitliche Verwendung von Bindestrichen. \
         Der Text enthält sowohl 'Help-Desks' als auch 'Helpdesk'."
    );
    assert!(m[0].suggestions.is_empty());
}

#[test]
fn german_long_sentence_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["TOO_LONG_SENTENCE_DE"]) else {
        return;
    };
    let text = "Dies ist ein Bandwurmsatz, der immer weiter geht, obwohl das kein guter Stil ist, \
                den man eigentlich berücksichtigen sollte, obwohl es auch andere Meinungen gibt, \
                die aber in der Minderzahl sind, weil die meisten Autoren sich doch an die \
                Stilvorgaben halten, wenn auch nicht alle, was aber letztendlich wiederum eine \
                Sache des Geschmacks ist.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "TOO_LONG_SENTENCE_DE")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    // Java probe (UTF-16) 0..342; the Rust range is UTF-8 bytes
    assert_eq!(lt::to_utf16_range(text, m[0].range), (0, 342));
    assert_eq!(
        m[0].message,
        "Dieser Satz hat mehr als 40 Wörter. Kürzen Sie den Satz oder teilen Sie ihn, \
         um die Lesbarkeit zu verbessern."
    );
}

#[test]
fn german_paragraph_rules_match_java() {
    let _guard = engine_guard();
    // Java probes (`--picky` + the requested rule only): WHITESPACE_RULE,
    // WHITESPACE_PARAGRAPH{,_BEGIN}, EMPTY_LINE, TOO_LONG_PARAGRAPH,
    // PUNCTUATION_PARAGRAPH_END.
    let Some(engine) = de_engine(&["WHITESPACE_RULE"]) else {
        return;
    };
    let text = "Das  ist ein Satz.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "WHITESPACE_RULE")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (3, 5));
    assert_eq!(
        m[0].message,
        "Möglicher Tippfehler: mehr als ein Leerzeichen hintereinander"
    );

    // whitespace-only segments are merged into the next sentence (Java
    // `SrxTextIterator`), so the rule sees the paragraph-start spaces
    let Some(engine) = de_engine(&["WHITESPACE_PARAGRAPH_BEGIN"]) else {
        return;
    };
    let text = "Ein Absatz.\n\n  Leerzeichen am Anfang.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "WHITESPACE_PARAGRAPH_BEGIN")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!(lt::to_utf16_range(text, m[0].range), (13, 26));
    assert_eq!(
        m[0].message,
        "Bitte löschen Sie das Leerzeichen am Anfang des Absatzes."
    );

    let Some(engine) = de_engine(&["WHITESPACE_PARAGRAPH"]) else {
        return;
    };
    let text = "Ein Absatz.  \n\nNächster Absatz.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "WHITESPACE_PARAGRAPH")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!(lt::to_utf16_range(text, m[0].range), (10, 13));
    assert_eq!(
        m[0].message,
        "Bitte löschen Sie das Leerzeichen am Ende des Absatzes."
    );

    let Some(engine) = de_engine(&["PUNCTUATION_PARAGRAPH_END"]) else {
        return;
    };
    let text = "Erster Satz. Zweiter ohne Punkt\n\nDritter Absatz.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "PUNCTUATION_PARAGRAPH_END")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!(lt::to_utf16_range(text, m[0].range), (26, 31));
    assert_eq!(
        m[0].message,
        "Bitte ein Satzzeichen am Ende des Absatzes einfügen."
    );
    let suggestions: Vec<&str> = m[0].suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        suggestions,
        vec!["Punkt.", "Punkt!", "Punkt?", "Punkt:", "Punkt,", "Punkt;"]
    );
}

#[test]
fn german_uppercase_sentence_start_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["UPPERCASE_SENTENCE_START"]) else {
        return;
    };
    let text = "Das Haus ist alt. es wurde 1950 gebaut.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "UPPERCASE_SENTENCE_START")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (18, 20));
    assert_eq!(
        m[0].message,
        "Dieser Satz fängt nicht mit einem großgeschriebenen Wort an."
    );
    assert_eq!(m[0].suggestions[0].value, "Es");
    assert_eq!(m[0].category_name, "Groß-/Kleinschreibung");
}

#[test]
fn german_simple_replace_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_SIMPLE_REPLACE"]) else {
        return;
    };
    let text = "Nicholas Cage ist ein Schauspieler.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id.starts_with("DE_SIMPLE_REPLACE"))
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (0, 13));
    assert_eq!(
        m[0].message,
        "Meinten Sie vielleicht <suggestion>Nicolas Cage</suggestion>?"
    );
    assert_eq!(m[0].suggestions[0].value, "Nicolas Cage");
    assert_eq!(m[0].short_message.as_deref(), Some("Falsches Wort"));
}

#[test]
fn german_unpaired_rules_match_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["UNPAIRED_BRACKETS"]) else {
        return;
    };
    let text = "Dem Präsidenten des Deutschen Bauernverbands (DBV zufolge habe die Dürre einen Schaden verursacht.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "UNPAIRED_BRACKETS")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    // Java probe (UTF-16) 45..46
    assert_eq!(lt::to_utf16_range(text, m[0].range), (45, 46));
    assert_eq!(
        m[0].message,
        "Zeichen ohne sein Gegenstück: ')' scheint zu fehlen"
    );

    let Some(engine) = de_engine(&["DE_UNPAIRED_QUOTES"]) else {
        return;
    };
    let text = "»Hallo Hans ist das dein ›neues Auto?«, fragte er.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_UNPAIRED_QUOTES")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!(lt::to_utf16_range(text, m[0].range), (25, 26));
    assert_eq!(
        m[0].message,
        "Zeichen ohne sein Gegenstück: '‹' scheint zu fehlen"
    );
}

#[test]
fn german_style_too_often_matches_java() {
    let _guard = engine_guard();
    // Java probe: 100 matches each, first match at 3..7 (UTF-16) for
    // `Er geht.` x60 + `Sie läuft.` x40; only matches when enabled by id.
    let text = "Er geht. ".repeat(60) + &"Sie läuft. ".repeat(40);
    let Some(engine) = de_engine(&["TOO_OFTEN_USED_VERB_DE"]) else {
        return;
    };
    let result = engine.check(&text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "TOO_OFTEN_USED_VERB_DE")
        .collect();
    assert_eq!(m.len(), 100, "{:?}", &m[..1]);
    assert_eq!(lt::to_utf16_range(&text, m[0].range), (3, 7));
    assert_eq!(
        m[0].message,
        "Das Verb wird häufiger verwendet als 5% aller Verben. \
         Möglicherweise ist es besser es durch ein Synonym zu ersetzen."
    );
    assert_eq!(m[0].category_name, "Stiltipps für kreatives Schreiben");

    let Some(engine) = de_engine(&["TOO_OFTEN_USED_NOUN_DE"]) else {
        return;
    };
    let text = "Der Hund läuft. ".repeat(60) + &"Der Baum steht. ".repeat(40);
    let result = engine.check(&text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "TOO_OFTEN_USED_NOUN_DE")
        .collect();
    assert_eq!(m.len(), 100);
    assert_eq!(lt::to_utf16_range(&text, m[0].range), (4, 8));

    let Some(engine) = de_engine(&["TOO_OFTEN_USED_ADJECTIVE_DE"]) else {
        return;
    };
    let text = "Der schöne Hund läuft. ".repeat(60) + &"Der große Baum steht. ".repeat(40);
    let result = engine.check(&text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "TOO_OFTEN_USED_ADJECTIVE_DE")
        .collect();
    assert_eq!(m.len(), 100);
}

#[test]
fn german_statistic_sentence_rules_match_java() {
    let _guard = engine_guard();
    // Java probe: two matches each (the two matching sentences), messages
    // with the computed share; direct speech is excluded.
    let Some(engine) = de_engine(&["PASSIVE_SENTENCE_DE"]) else {
        return;
    };
    let text = "Das Haus wurde gebaut. Der Brief wurde geschrieben. Die Katze schläft.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "PASSIVE_SENTENCE_DE")
        .collect();
    assert_eq!(m.len(), 2, "{m:?}");
    // Java probe: 9..14 (the `wurde` token)
    assert_eq!((m[0].range.start, m[0].range.end), (9, 14));
    assert_eq!(
        m[0].message,
        "Mehr als 8% Passivsätze {67%} gefunden. Aktiv formulierte Sätze sprechen im Regelfall den Leser stärker an."
    );
    assert_eq!(m[0].category_name, "Stiltipps für kreatives Schreiben");

    let Some(engine) = de_engine(&["SENTENCE_WITH_MODAL_VERB_DE"]) else {
        return;
    };
    let text = "Er muss gehen. Sie kann kommen. Das Haus steht.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "SENTENCE_WITH_MODAL_VERB_DE")
        .collect();
    assert_eq!(m.len(), 2, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (3, 7));
    assert_eq!(
        m[0].message,
        "Mehr als 18% Sätze mit Modalverben {67%} gefunden. Modalverben blähen den Text häufig auf und sollten vermieden werden."
    );

    let Some(engine) = de_engine(&["SENTENCE_WITH_MAN_DE"]) else {
        return;
    };
    let text = "Man kann das machen. Man sollte das tun. Es ist schön.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "SENTENCE_WITH_MAN_DE")
        .collect();
    assert_eq!(m.len(), 2, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (0, 3));
    assert_eq!(
        m[0].message,
        "Mehr als 15‰ Sätze mit der indirekten Leseransprache 'man' {667‰} gefunden. \
         Lässt sich das Wort vermeiden?"
    );

    let Some(engine) = de_engine(&["SENTENCE_BEGINNING_WITH_CONJUNCTION_DE"]) else {
        return;
    };
    let text = "Aber das ist gut. Aber das ist schlecht. Es ist schön.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "SENTENCE_BEGINNING_WITH_CONJUNCTION_DE")
        .collect();
    assert_eq!(m.len(), 2, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (0, 4));
    // the legacy "Auch wenn" quirk: it fires unless the sentence is a question
    let engine_q = de_engine(&["SENTENCE_BEGINNING_WITH_CONJUNCTION_DE"]).unwrap();
    let result = engine_q
        .check("Auch wenn das passiert? Es ist gut.")
        .unwrap();
    assert!(!result
        .matches
        .iter()
        .any(|m| m.rule_id == "SENTENCE_BEGINNING_WITH_CONJUNCTION_DE"));
}

#[test]
fn german_statistic_word_rules_match_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["FILLER_WORDS_DE"]) else {
        return;
    };
    let text = "Das ist eben so. Das ist eben gut. Das ist schön.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "FILLER_WORDS_DE")
        .collect();
    assert_eq!(m.len(), 3, "{m:?}");
    assert_eq!(m[0].message, "Mehr als 8% Füllwörter {27%} gefunden. Möglicherweise ist es besser dieses potentielle Füllwort zu löschen.");

    let Some(engine) = de_engine(&["NON_SIGNIFICANT_VERB_DE"]) else {
        return;
    };
    let text = "Er macht einen Kuchen. Er macht das. Er tut das.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "NON_SIGNIFICANT_VERB_DE")
        .collect();
    assert_eq!(m.len(), 3, "{m:?}");
    // Java probe (UTF-16): first match 3..8 ("macht")
    assert_eq!(lt::to_utf16_range(text, m[0].range), (3, 8));

    let Some(engine) = de_engine(&["UNNECESSARY_PHRASES_DE"]) else {
        return;
    };
    let text =
        "Das ist allem Anschein nach eine Phrase. Das ist im Allgemeinen gut. Das ist schön.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "UNNECESSARY_PHRASES_DE")
        .collect();
    assert_eq!(m.len(), 2, "{m:?}");
    // multi-token phrase: "allem Anschein nach" (Java probe 8..27)
    assert_eq!(lt::to_utf16_range(text, m[0].range), (8, 27));
}

#[test]
fn german_style_repeated_rules_match_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["STYLE_REPEATED_SHORT_SENTENCES"]) else {
        return;
    };
    let text = "Das Auto kam näher. Der Hund schlief. Die Reifen quietschten. \
                Und dann kam der Regen.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "STYLE_REPEATED_SHORT_SENTENCES")
        .collect();
    assert_eq!(m.len(), 3, "{m:?}");
    assert_eq!(m[0].message, "Stakkato-Sätze");
    // direct speech resets the streak
    let engine_ds = de_engine(&["STYLE_REPEATED_SHORT_SENTENCES"]).unwrap();
    let result = engine_ds
        .check("Das Auto kam näher. »Der Hund schlief.« Die Reifen quietschten. Und dann kam der Regen.")
        .unwrap();
    assert!(!result
        .matches
        .iter()
        .any(|m| m.rule_id == "STYLE_REPEATED_SHORT_SENTENCES"));

    let Some(engine) = de_engine(&["STYLE_REPEATED_SENTENCE_BEGINNING"]) else {
        return;
    };
    let text = "Das Auto kam näher. Der Hund lief langsam. Die Reifen quietschten. \
                Dann kam der Regen.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "STYLE_REPEATED_SENTENCE_BEGINNING")
        .collect();
    assert_eq!(m.len(), 3, "{m:?}");
    // the match covers article + subject of each sentence
    assert_eq!((m[0].range.start, m[0].range.end), (0, 8));
    assert_eq!(m[0].message, "Subjekt als wiederholter Satzanfang");
}

#[test]
fn german_style_repeated_word_rule_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["STYLE_REPEATED_WORD_RULE_DE"]) else {
        return;
    };
    // Java probe (`ProbeRule`, de-DE, `STYLE_REPEATED_WORD_RULE_DE` enabled):
    //   4..8 same sentence, 32..36 same sentence,
    //   56..61 "nachfolgender Satz", 74..79 "vorhergehender Satz"
    let text = "Ich gehe zum Supermarkt, danach gehe ich nach Hause. \
                Er kauft ein. Danach kauft er Brot.";
    let result = engine.check(text).unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "STYLE_REPEATED_WORD_RULE_DE")
        .collect();
    assert_eq!(m.len(), 4, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (4, 8));
    assert_eq!((m[1].range.start, m[1].range.end), (32, 36));
    assert_eq!((m[2].range.start, m[2].range.end), (56, 61));
    assert_eq!((m[3].range.start, m[3].range.end), (74, 79));
    assert_eq!(
        m[0].message,
        "Mögliches Stilproblem: Das Wort wird noch einmal im selben Satz verwendet."
    );
    assert_eq!(
        m[2].message,
        "Mögliches Stilproblem: Das Wort wird auch in einem nachfolgenden Satz verwendet."
    );
    assert_eq!(
        m[3].message,
        "Mögliches Stilproblem: Das Wort wird bereits in einem vorhergehenden Satz verwendet."
    );

    // Java probe: question/answer pairs one sentence apart are no repetition
    let qa = engine.check("Kommst du mit? Ja, ich komme mit.").unwrap();
    assert!(!qa
        .matches
        .iter()
        .any(|m| m.rule_id == "STYLE_REPEATED_WORD_RULE_DE"));

    // Java probe: 3..9 after, 19..24 before
    let both = engine.check("Du kommst mit. Ich komme mit.").unwrap();
    let m: Vec<_> = both
        .matches
        .iter()
        .filter(|m| m.rule_id == "STYLE_REPEATED_WORD_RULE_DE")
        .collect();
    assert_eq!(m.len(), 2, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (3, 9));
    assert_eq!((m[1].range.start, m[1].range.end), (19, 24));

    // Java probe: a listing in the first sentence suppresses its own matches
    // (only 25..29 "Hund" in the second sentence is reported)
    let listing = engine
        .check("Der Hund - ein Tier. Der Hund bellt.")
        .unwrap();
    let m: Vec<_> = listing
        .matches
        .iter()
        .filter(|m| m.rule_id == "STYLE_REPEATED_WORD_RULE_DE")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (25, 29));

    // Java probe: direct speech is excluded (no matches)
    let ds = engine
        .check("Er sagt: „Ich gehe heim.“ Danach gehe ich schlafen.")
        .unwrap();
    assert!(!ds
        .matches
        .iter()
        .any(|m| m.rule_id == "STYLE_REPEATED_WORD_RULE_DE"));
}

#[test]
fn compound_infinitiv_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["COMPOUND_INFINITIV_RULE"]) else {
        return;
    };
    // Java probe (`ProbeRule`, de-DE): 45..60 UTF-16; the text has one
    // two-byte `ü` before the match, so the UTF-8 byte range is 47..62.
    let result = engine
        .check("Er überprüfte die Rechnungen noch einmal, um sicher zu gehen.")
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "COMPOUND_INFINITIV_RULE")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (47, 62));
    assert_eq!(
        m[0].message,
        "Wenn der erweiterte Infinitiv von dem Verb 'sichergehen' abgeleitet ist, \
         sollte er zusammengeschrieben werden."
    );
    assert_eq!(m[0].suggestions[0].value, "sicherzugehen");

    // Java probes: 34..50 'saubermachen', 26..42 'vorbeikommen', 37..53 'vorbeilassen'
    for (text, range, suggestion) in [
        (
            "Ich brachte ihn dazu, mein Zimmer sauber zu machen.",
            (34, 50),
            "sauberzumachen",
        ),
        (
            "Du brauchst nicht bei mir vorbei zu kommen.",
            (26, 42),
            "vorbeizukommen",
        ),
        (
            "Ich ging zur Seite, um die alte Dame vorbei zu lassen.",
            (37, 53),
            "vorbeizulassen",
        ),
    ] {
        let result = engine.check(text).unwrap();
        let m: Vec<_> = result
            .matches
            .iter()
            .filter(|m| m.rule_id == "COMPOUND_INFINITIV_RULE")
            .collect();
        assert_eq!(m.len(), 1, "{text}: {m:?}");
        assert_eq!((m[0].range.start, m[0].range.end), range, "{text}");
        assert_eq!(m[0].suggestions[0].value, suggestion, "{text}");
    }

    // Negative cases from `CompoundInfinitivRuleTest` (rule.match length 0)
    for text in [
        "Seine Frau gab vor zu schlafen.",
        "Mein Herz hörte auf zu schlagen.",
        "Den Sonnenaufgang von einem Berggipfel aus zu sehen, ist eine Wonne.",
        "Hör auf zu schreien",
        "Sie riss sich zusammen und fing wieder an zu reden.",
        "Fang an zu zählen.",
        "Er hatte nichts weiter zu sagen",
        "Sie strengte sich an zu schwimmen.",
        "Er erhob sich von dem Stuhl, auf dem er gesessen hatte, und begann im Zimmer auf und ab zu schreiten.",
        "Tom stand auf und fing an, auf und ab zu gehen.",
        "Aber um auf Nummer sicher zu gehen, schrieb er es auf.",
        "Sie scheint aus Spanien heraus zu kaufen.",
        "Hab keine Lust, mir Gedanken darüber zu machen.",
        "Fang an zu lesen, wo du aufgehört hast.",
        "Fang dort an zu lesen, wo du aufgehört hast.",
        "Der diensthabende Kollege hatte ganz schön zu tun.",
        "Das ist schwer zu sagen.",
        "Wir versuchen, das Problem zu lösen.",
        "Ich habe viel zu tun.",
    ] {
        let result = engine.check(text).unwrap();
        assert!(
            !result
                .matches
                .iter()
                .any(|m| m.rule_id == "COMPOUND_INFINITIV_RULE"),
            "{text}: {:?}",
            result.matches
        );
    }
}

#[test]
fn missing_comma_relative_clause_matches_java() {
    let _guard = engine_guard();
    // Front instance (`COMMA_IN_FRONT_RELATIVE_CLAUSE`) — expected values from
    // the pinned `MissingCommaRelativeClauseRuleTest`.
    let Some(front) = de_engine(&["COMMA_IN_FRONT_RELATIVE_CLAUSE"]) else {
        return;
    };
    let rule = "COMMA_IN_FRONT_RELATIVE_CLAUSE";
    for (text, range) in [
        (
            "Das Auto das am Straßenrand steht parkt im Halteverbot.",
            (4, 12),
        ),
        (
            "Das Auto das am Straßenrand steht, parkt im Halteverbot.",
            (4, 12),
        ),
        (
            "Das Auto in dem der Mann sitzt, parkt im Halteverbot.",
            (4, 15),
        ),
        (
            "Das Auto in dem der Mann sitzt parkt im Halteverbot.",
            (4, 15),
        ),
        (
            "Die Frau die vor dem Auto steht hat schwarze Haare.",
            (4, 12),
        ),
        (
            "Die Frau die vor dem Auto steht, hat schwarze Haare.",
            (4, 12),
        ),
        ("Alles was ich habe, ist ein Buch.", (0, 9)),
    ] {
        let matches = engine_matches(&front, text, rule);
        assert_eq!(matches.len(), 1, "{text}: {matches:?}");
        assert_eq!(
            (matches[0].0, matches[0].1),
            utf16_to_utf8_range(text, range),
            "{text}"
        );
    }
    for text in [
        "Computer machen die Leute dumm.",
        "Die Unstimmigkeit zwischen den Geschichten der zwei Unfallbeteiligten war groß.",
        "Ebenso darf keine schwerere Strafe als die zum Zeitpunkt der Begehung der strafbaren Handlung angedrohte Strafe verhängt werden.",
        "Als dritte Gruppe lassen sich Aminosäuren fassen, die der Organismus anstelle dieser in Proteine einbaut.",
        "Selbst wenn das alles perfekt verlustfrei wäre, hätte ich nichts gewonnen.",
        "Die Studenten, deren Urteil am stärksten von dem der Profis abwich, waren sich sicher, einen guten von einem schlechten unterscheiden zu können.",
        "Die Studenten, deren Urteil am stärksten durch das der Profis beeinflusst wurde, waren sich sicher, einen guten von einem schlechten unterscheiden zu können.",
        // Java probe: the token after the participle `übertragen` is the
        // sentence-final `.`, whose tagger-constructed `isPosTagUnknown`
        // snapshot is true, so `verbPos` drops `übertragen`,
        // `hasPotentialSubclause` returns -1 and the rule stays silent.
        "Wir werden, das was wir mit Google gemacht haben mal auf Bing übertragen.",
    ] {
        assert!(
            engine_matches(&front, text, rule).is_empty(),
            "front should not match: {text}"
        );
    }
    drop(front);

    // Behind instance (`COMMA_BEHIND_RELATIVE_CLAUSE`)
    let Some(behind) = de_engine(&["COMMA_BEHIND_RELATIVE_CLAUSE"]) else {
        return;
    };
    let rule = "COMMA_BEHIND_RELATIVE_CLAUSE";
    for (text, range) in [
        (
            "Das Auto, das am Straßenrand steht parkt im Halteverbot.",
            (29, 40),
        ),
        (
            "Das Auto, in dem der Mann sitzt parkt im Halteverbot.",
            (26, 37),
        ),
        (
            "Die Frau, die vor dem Auto steht hat schwarze Haare.",
            (27, 36),
        ),
        ("Alles, was ich habe ist ein Buch.", (15, 23)),
        (
            "In diesem Prozess sind aber Entwicklungsschritte ja integriert, die wir Psychiater glaube ich auch gut kennen.",
            (72, 93),
        ),
        (
            "Alles, was du für die Spieße brauchst sind Tortellini, getrocknete Tomaten, Cherrytomaten und Mozzarellakugeln.",
            (29, 42),
        ),
    ] {
        let matches = engine_matches(&behind, text, rule);
        assert_eq!(matches.len(), 1, "{text}: {matches:?}");
        assert_eq!(
            (matches[0].0, matches[0].1),
            utf16_to_utf8_range(text, range),
            "{text}"
        );
    }
    for text in [
        ".... das war eher so das, was alle im Browser deaktiviert hatten, weil man dachte, über Javascript wird irgendwie Schadsoftware eingeschleust.",
        "Ich habe einige Fehler begangen, die ich vermeiden hätte können sollen.",
        "Doch die Rolle, die Mohammed bin Salman in Riad spielt, ist zwiespältig.",
        "Laut Josef Peter Burg gehörte das Haus, aus dem er samt Familie geworfen wurde, Werner und Sigrid Bahlke, während er darin wohnte und darauf aufpasste.",
        "Darüber hinaus stellt die klassische Metaphysik eine Grundfrage, die sich etwa wie folgt formulieren lässt: Warum ist überhaupt Seiendes und nicht vielmehr Nichts?",
        "Wenn du alles, was du meinst nicht zu können, von anderen erledigen lässt, wirst du es niemals selbst lernen.",
        "Er hat einen Zeitraum durchlebt, in dem seine Gedanken verträumt auf den weiten Feldern der Mysterien umherirrten.",
        "Es ist die Wiederkehr der Panikmache, die der neue Nationalismus mit dem der Sprachreiniger verbindet und die Geschichte der Sprachreinigung zu einem Lehrstück macht.",
        "Gesuche können von Institutionen, Organisationen, Vereinen und Gruppierungen gestellt werden, die im Kanton Luzern domiziliert sind.",
        "Die Klausel kann zudem nur Gleichrang mit Verbindlichkeiten des Schuldners herstellen, die vom Gesetz in der Insolvenz nicht privilegiert sind, so dass die Klausel nichts an der gesetzlich vorgesehenen Rangfolge im Insolvenzverfahren ändert.",
        "Plan von Maßnahmen, mit denen das Ansteckungsrisiko während des Aufenthalts an einem Ort verringert werden soll",
        "Aus diesem Grund sind die Wörter nicht direkt übersetzt, stattdessen wird der Zustand oder die Situation beschrieben in der die Wörter benutzt werden.",
        "Kryptographisch sichere Verfahren sind dann solche, für die es keine bessere Methode zum Brechen der Sicherheit als das Faktorisieren einer großen Zahl gibt, insbesondere kann der private nicht aus dem öffentlichen Schlüssel errechnet werden.",
        "Wayne Jancik begrenzt seine One-Hit-Wonders in den USA auf die Billboard-Top-40-Pop-Hitparade mit einer „Ruhe-Periode“ von 5 Jahren, innerhalb derer kein weiterer Hit desselben Interpreten in die Top 40 gelangen darf.",
        "Nach dem Abzug russischer Truppen aus der Region Kiew sind in den ehemals besetzten und umkämpften ukrainischen Gebieten inzwischen Hunderte Leichen von Bewohnern gefunden worden.",
        "Bei der Installation werden Ihnen Sticker und Aufkleber zur Verfügung gestellt, die Sie anstelle der traditionellen Emojis verwenden können.",
        "»Alles, was wir dank dieses Projektes sehen werden, wird für uns neu sein«, so der renommierte Bienenforscher.",
    ] {
        assert!(
            engine_matches(&behind, text, rule).is_empty(),
            "behind should not match: {text}"
        );
    }
}

/// Matches of `rule` in `text` as UTF-8 byte ranges.
fn engine_matches(engine: &Engine, text: &str, rule: &str) -> Vec<(usize, usize)> {
    engine
        .check(text)
        .unwrap()
        .matches
        .iter()
        .filter(|m| m.rule_id == rule)
        .map(|m| (m.range.start, m.range.end))
        .collect()
}

/// Java probe/test offsets are UTF-16 code units; the engine reports UTF-8
/// bytes. This converts a UTF-16 range to the byte range for the same text.
fn utf16_to_utf8_range(text: &str, range: (usize, usize)) -> (usize, usize) {
    let (start, end) = range;
    let mut utf16 = 0usize;
    let mut byte_start = None;
    let mut byte_end = None;
    for (idx, ch) in text.char_indices() {
        if byte_start.is_none() && utf16 >= start {
            byte_start = Some(idx);
        }
        if byte_end.is_none() && utf16 >= end {
            byte_end = Some(idx);
        }
        utf16 += ch.len_utf16();
        if byte_start.is_some() && byte_end.is_some() {
            break;
        }
    }
    (
        byte_start.unwrap_or(text.len()),
        byte_end.unwrap_or(text.len()),
    )
}

#[test]
fn verb_agreement_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_VERBAGREEMENT"]) else {
        return;
    };
    let rule = "DE_VERBAGREEMENT";
    let matches = |text: &str| -> Vec<String> {
        engine
            .check(text)
            .unwrap()
            .matches
            .iter()
            .filter(|m| m.rule_id == rule)
            .map(|m| {
                format!(
                    "{}..{}|{}|{}",
                    m.range.start,
                    m.range.end,
                    m.message,
                    m.suggestions
                        .iter()
                        .map(|s| s.value.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect()
    };

    // Exact suggestion lists from `VerbAgreementRuleTest.testSuggestionSorting`
    // (Java `HashSet` order) and `testWrongVerbSubject`
    let sorting = matches("Wir nenne ihn mal „wild“.");
    assert_eq!(sorting.len(), 1, "{sorting:?}");
    assert!(
        sorting[0].ends_with(
            "Wir nennen, Wir nennten, Er nenne, Sie nenne, Wir nannten, Ich nenne, Es nenne"
        ),
        "{sorting:?}"
    );
    let m = matches("Du können heute leider nicht kommen.");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].ends_with("Du könnest, Du kannst, Du könntest, Wir können, Sie können, Du konntest"),
        "{m:?}"
    );
    let m = matches("Ich leben.");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].ends_with("Ich lebe, Ich lebte, Wir leben, Sie leben"),
        "{m:?}"
    );
    let m = matches("Lebe du?");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].ends_with("Lebest du, Lebst du, Lebe er, Lebe es, Lebtest du, Lebe ich, Lebe sie"),
        "{m:?}"
    );
    // Java engine probe (`ProbeRule`): the two raw rule matches collapse to
    // one (5..13) in the engine post-filters; suggestions from the probe.
    let m = matches("Nett bist ich nicht.");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].ends_with("bin ich, bist du, sei ich, wäre ich, war ich"),
        "{m:?}"
    );
    let m = matches("Ich bist nett.");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].starts_with("0..8|"), "{m:?}");
    assert!(
        m[0].ends_with("Ich bin, Du bist, Ich sei, Ich wäre, Ich war"),
        "{m:?}"
    );
    let m = matches("Nett sind er.");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].ends_with("sind wir, sei er, ist er, sind sie, wäre er, war er"),
        "{m:?}"
    );
    let m = matches("Wir lebst noch.");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].ends_with("Wir leben, Wir lebten, Du lebst"), "{m:?}");
    let m = matches("Er sagte düster: „Ich brauchen mich nicht schuldig fühlen.“");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].ends_with("Ich brauche, Ich brauchte, Ich bräuchte, Wir brauchen, Sie brauchen"),
        "{m:?}"
    );

    // `testPositions` (UTF-16 offsets; the ASCII texts are identical in UTF-8)
    let m = matches("Du erreichst ich unter 12345");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].starts_with("3..16|"), "{m:?}");
    let m = matches("Hallo Karl. Du erreichst ich unter 12345");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].starts_with("15..28|"), "{m:?}");
    let m = matches(
        "Ihr könnt das Training abbrechen, weil es nichts bringen wird. \
         Er geht los und sagt dabei: Werde ich machen.",
    );
    assert_eq!(m.len(), 1, "{m:?}");
    // Java probe: 97..107 (UTF-16); the `ö` of "könnt" makes the UTF-8 byte
    // range 98..108.
    assert!(m[0].starts_with("98..108|"), "{m:?}");
    assert!(matches("Mir ist bewusst, dass viele Menschen wie du empfinden.").is_empty());

    // `testWrongVerb` correct sentences
    for text in [
        "*runterguck* das ist aber tief",
        "Weder Peter noch ich wollen das.",
        "Du bist in dem Moment angekommen, als ich gegangen bin.",
        "Kümmere du dich mal nicht darum!",
        "Ich weiß, was ich tun werde, falls etwas geschehen sollte.",
        "...die dreißig Jahre jünger als ich ist.",
        "Ein Mann wie ich braucht einen Hut.",
        "Egal, was er sagen wird, ich habe meine Entscheidung getroffen.",
        "Du Beharrst darauf, dein Wörterbuch hätte recht, hast aber von den Feinheiten des Japanischen keine Ahnung!",
        "Bin gleich wieder da.",
        "Wobei ich äußerst vorsichtig bin.",
        "Es ist klar, dass ich äußerst vorsichtig mit den Informationen umgehe",
        "Es ist klar, dass ich äußerst vorsichtig bin.",
        "Wobei er äußerst selten darüber spricht.",
        "Wobei er äußerst selten über seine erste Frau spricht.",
        "Das Wort „schreibst“ ist schön.",
        "Die Jagd nach bin Laden.",
        "Die Unterlagen solltet ihr gründlich durcharbeiten.",
        "Er reagierte äußerst negativ.",
        "Max und ich sollten das machen.",
        "Osama bin Laden stammt aus Saudi-Arabien.",
        "Solltet ihr das machen?",
        "Dann beende du den Auftrag und bring sie ihrem Vater.",
        "- Wirst du ausflippen?",
        "Ein Geschenk, das er einst von Aphrodite erhalten hatte.",
        "Wenn ich sterben sollte, wer würde sich dann um die Katze kümmern?",
        "Wenn er sterben sollte, wer würde sich dann um die Katze kümmern?",
        "Wenn sie sterben sollte, wer würde sich dann um die Katze kümmern?",
        "Wenn es sterben sollte, wer würde sich dann um die Katze kümmern?",
        "Wenn ihr sterben solltet, wer würde sich dann um die Katze kümmern?",
        "Wenn wir sterben sollten, wer würde sich dann um die Katze kümmern?",
        "Dafür erhielten er sowie der Hofgoldschmied Theodor Heiden einen Preis.",
        "Probst wurde deshalb in den Medien gefeiert.",
        "/usr/bin/firefox",
        "Das sind Leute, die viel mehr als ich wissen.",
        "Das ist mir nicht klar, kannst ja mal beim Kunden nachfragen.",
        "So tes\u{00AD}test Du das mit dem soft hyphen.",
        "Viele Brunnen in Italiens Hauptstadt sind bereits abgeschaltet.",
        "„Werde ich tun!“",
        "Könntest dir mal eine Scheibe davon abschneiden!",
        "Müsstest dir das mal genauer anschauen.",
        "Kannst ein neues Release machen.",
        "Sie fragte: „Muss ich aussagen?“",
        "„Können wir bitte das Thema wechseln, denn ich möchte ungern darüber reden?“",
        "Er sagt: „Willst du behaupten, dass mein Sohn euch liebt?“",
        "Kannst mich gerne anrufen.",
        "Kannst ihn gerne anrufen.",
        "Kannst sie gerne anrufen.",
        "Aber wie ich sehe, benötigt ihr Nachschub.",
        "Wie ich sehe, benötigt ihr Nachschub.",
        "Einer wie du kennt doch bestimmt viele Studenten.",
        "Für Sie mache ich eine Ausnahme.",
        "Ohne sie hätte ich das nicht geschafft.",
        "Ohne Sie hätte ich das nicht geschafft.",
        "Ich hoffe du auch.",
        "Ich hoffe ihr auch.",
        "Wird hoffen du auch.",
        "Hab einen schönen Tag!",
        "Tom traue ich mehr als Maria.",
        "Tom kenne ich nicht besonders gut, dafür aber seine Frau.",
        "Tom habe ich heute noch nicht gesehen.",
        "Tom bezahle ich gut.",
        "Tom werde ich nicht noch mal um Hilfe bitten.",
        "Tom konnte ich überzeugen, nicht aber Maria.",
        "Mach du mal!",
        "Das bekomme ich nicht hin.",
        "Dies betreffe insbesondere Nietzsches Aussagen zu Kant und der Evolutionslehre.",
        "❌Du fühlst Dich unsicher?",
        "Bringst nicht einmal so etwas Einfaches zustande!",
        "Bekommst sogar eine Sicherheitszulage",
        "Dallun sagte nur, dass er gleich kommen wird und legte wieder auf.",
        "Tinne, Elvis und auch ich werden gerne wiederkommen!",
        "Du bist Lehrer und weißt diese Dinge nicht?",
        "Die Frage lautet: Bist du bereit zu helfen?",
        "Ich will nicht so wie er enden.",
        "Das heißt, wir geben einander oft nach als gute Freunde, ob wir gleich nicht einer Meinung sind.",
        "Wir seh'n uns in Berlin.",
        "Bist du bereit, darüber zu sprechen?",
        "Bist du schnell eingeschlafen?",
        "Im Gegenzug bin ich bereit, beim Türkischlernen zu helfen.",
        "Das habe ich lange gesucht.",
        "Dann solltest du schnell eine Nummer der sexy Omas wählen.",
        "Vielleicht würdest du bereit sein, ehrenamtlich zu helfen.",
        "Werde nicht alt, egal wie lange du lebst.",
        "Du bist hingefallen und hast dir das Bein gebrochen.",
        "Mögest du lange leben!",
        "Planst du lange hier zu bleiben?",
        "Du bist zwischen 11 und 12 Jahren alt und spielst gern Fußball bzw. möchtest damit anfangen?",
        "Ein großer Hadithwissenschaftler, Scheich Şemseddin Mehmed bin Muhammed-ül Cezri, kam in der Zeit von Mirza Uluğ Bey nach Semerkant.",
        "Die Prüfbescheinigung bekommst du gleich nach der bestandenen Prüfung vom Prüfer.",
        "Du bist sehr schön und brauchst überhaupt gar keine Schminke zu verwenden.",
        "Ist das so schnell, wie du gehen kannst?",
        "Egal wie lange du versuchst, die Leute davon zu überzeugen",
        "Du bist verheiratet und hast zwei Kinder.",
        "Du bist aus Berlin und wohnst in Bonn.",
        "Sie befestigen die Regalbretter vermittelst dreier Schrauben.",
        "Meine Familie & ich haben uns ein neues Auto gekauft.",
        "Der Bescheid lasse im übrigen die Abwägungen vermissen, wie die Betriebsprüfung zu den Sachverhaltsbeurteilungen gelange, die den von ihr bekämpften Bescheiden zugrundegelegt worden seien.",
        "Die Bildung des Samens erfolgte laut Alkmaion im Gehirn, von wo aus er durch die Adern in den Hoden gelange.",
        "Michael Redmond (geb. 1963, USA).",
        "Würd mich sehr freuen drüber.",
        "Es würd' ein jeder Doktor sein, wenn's Wissen einging wie der Wein.",
        "Bald merkte er, dass er dank seines Talents nichts mehr in der österreichischen Jazzszene lernen konnte.",
        "»Alles, was wir dank dieses Projektes sehen werden, wird für uns neu sein«, so der renommierte Bienenforscher.",
        "Und da wir äußerst Laissez-faire sind, kann man das auch machen.",
        "Duzen, jemanden mit Du anreden, eine Sitte, die bei allen alten Völkern üblich war.",
        "Schreibtischtäter wie Du sind doch eher selten.",
        "Nee, geh du!",
    ] {
        assert!(matches(text).is_empty(), "should be good: {text}: {:?}", matches(text));
    }

    // `testWrongVerb` incorrect sentences
    for text in [
        "Als Borcarbid weißt es eine hohe Härte auf.",
        "Das greift auf Vorläuferinstitutionen bist auf die Zeit von 1234 zurück.",
        "Die Eisenbahn dienst überwiegend dem Güterverkehr.",
        "Die Unterlagen solltest ihr gründlich durcharbeiten.",
        "Peter bin nett.",
        "Weiter befindest sich im Osten die Gemeinde Dorf.",
        "Ich geht jetzt nach Hause, weil ich schon zu spät bin.",
        "„Du muss gehen.“",
        "Du weiß es doch.",
        "Sie sagte zu mir: „Du muss gehen.“",
        "„Ich müsst alles machen.“",
        "„Ich könnt mich sowieso nicht verstehen.“",
        "Er sagte düster: Ich brauchen mich nicht böse angucken.",
        "David sagte düster: Ich brauchen mich nicht böse angucken.",
        "Ich setzet mich auf den weichen Teppich und kreuzte die Unterschenkel wie ein Japaner.",
        "Ich brauchen einen Karren mit zwei Ochsen.",
        "Ich haben meinen Ohrring fallen lassen.",
        "Ich stehen Ihnen gerne für Rückfragen zur Verfügung.",
    ] {
        assert_eq!(
            matches(text).len(),
            1,
            "should be bad: {text}: {:?}",
            matches(text)
        );
    }
    // Java engine probes (`ProbeRule`): raw rule matches that overlap with the
    // same rule id collapse in the post-filters, so the engine reports one.
    for (text, range) in [
        ("Du bin nett.", (0, 6)),
        ("Er bin nett.", (0, 6)),
        ("Er gelangst zu ihr.", (0, 11)),
        ("Er lebst.", (0, 8)),
        ("Ich kannst heute leider nicht kommen.", (0, 10)),
        ("Nett warst wir.", (5, 10)),
        ("Wir bin nett.", (0, 7)),
        ("Wir gelangst zu ihr.", (0, 12)),
        ("Wir lebst noch.", (0, 9)),
    ] {
        let m = matches(text);
        assert_eq!(m.len(), 1, "should be bad: {text}: {m:?}");
        assert!(
            m[0].starts_with(&format!("{}..{}|", range.0, range.1)),
            "{m:?}"
        );
    }

    // `testWrongVerbSubject` correct sentences
    for text in [
        "Auch morgen lebe ich.",
        "Auch morgen leben wir noch.",
        "Auch morgen lebst du.",
        "Auch morgen lebt er.",
        "Auch wenn du leben möchtest.",
        "auf der er sieben Jahre blieb.",
        "Das absolute Ich ist nicht mit dem individuellen Geist zu verwechseln.",
        "Das Ich ist keine Einbildung",
        "Das lyrische Ich ist verzweifelt.",
        "Den Park, von dem er äußerst genaue Karten zeichnete.",
        "Der auffälligste Ring ist der erster Ring, obwohl er verglichen mit den anderen Ringen sehr schwach erscheint.",
        "Der Fehler, falls er bestehen sollte, ist schwerwiegend.",
        "Der Vorfall, bei dem er einen Teil seines Vermögens verloren hat, ist lange vorbei.",
        "Diese Lösung wurde in der 64'er beschrieben, kam jedoch nie.",
        "Die Theorie, mit der ich arbeiten konnte.",
        "Du bist nett.",
        "Du kannst heute leider nicht kommen.",
        "Du lebst.",
        "Du wünschst dir so viel.",
        "Er geht zu ihr.",
        "Er ist nett.",
        "Er kann heute leider nicht kommen.",
        "Er lebt.",
        "Er wisse nicht, ob er lachen oder weinen solle.",
        "Er und du leben.",
        "Er und ich leben.",
        "Falls er bestehen sollte, gehen sie weg.",
        "Heere, des Gottes der Schlachtreihen Israels, den du verhöhnt hast.",
        "Ich bin",
        "Ich bin Frankreich!",
        "Ich bin froh, dass ich arbeiten kann.",
        "Ich bin nett.",
        "‚ich bin tot‘",
        "Ich kann heute leider nicht kommen.",
        "Ich lebe.",
        "Lebst du?",
        "Morgen kommen du und ich.",
        "Morgen kommen er, den ich sehr mag, und ich.",
        "Morgen kommen er und ich.",
        "Morgen kommen ich und sie.",
        "Morgen kommen wir und sie.",
        "nachdem er erfahren hatte",
        "Nett bin ich.",
        "Nett bist du.",
        "Nett ist er.",
        "Nett sind wir.",
        "Niemand ahnte, dass er gewinnen könne.",
        "Sie lebt und wir leben.",
        "Sie und er leben.",
        "Sind ich und Peter nicht nette Kinder?",
        "Sodass ich sagen möchte, dass unsere schönen Erinnerungen gut sind.",
        "Wann ich meinen letzten Film drehen werde, ist unbekannt.",
        "Was ich tun muss.",
        "Welche Aufgaben er dabei tatsächlich übernehmen könnte",
        "wie er beschaffen war",
        "Wir gelangen zu dir.",
        "Wir können heute leider nicht kommen.",
        "Wir leben noch.",
        "Wir sind nett.",
        "Wobei wir benutzt haben, dass der Satz gilt.",
        "Wünschst du dir mehr Zeit?",
        "Wyrjtjbst du?",
        "Wenn ich du wäre, würde ich das nicht machen.",
        "Er sagte: „Darf ich bitten, mir zu folgen?“",
        "Ja sind ab morgen dabei.",
        "Oh bin überfragt.",
        "Angenommen, du wärst ich.",
        "Ich denke, dass das Haus, in das er gehen will, heute Morgen gestrichen worden ist.",
        "Ich hab mein Leben, leb du deines!",
        "Da freut er sich, wenn er schlafen geht und was findet.",
        "John nimmt weiter an einem Abendkurs über Journalismus teil.",
        "Viele nahmen an der Aktion teil und am Ende des rAAd-Events war die Tafel zwar bunt, aber leider überwogen die roten Kärtchen sehr deutlich.",
        "Musst also nichts machen.",
        "Eine Situation, wo der Stadtrat gleich mal zum Du übergeht.",
        "Machen wir, sobald wir frische und neue Akkus haben.",
        "Darfst nicht so reden, Franz!",
        "Finde du den Jungen.",
        "Finde Du den Jungen.",
        "Kümmerst dich ja gar nicht um sie.",
        "Könntest was erfinden, wie dein Papa.",
        "Siehst aus wie ein Wachhund.",
        "Solltest es mal in seinem Büro versuchen.",
        "Stehst einfach nicht zu mir.",
        "Stellst für deinen Dad etwas zu Essen bereit.",
        "Springst weit, oder?",
        "Wirst groß, was?",
    ] {
        assert!(matches(text).is_empty(), "should be good: {text}: {:?}", matches(text));
    }

    // `testWrongVerbSubject` incorrect sentences
    for (text, n) in [
        ("Auch morgen leben du.", 1),
        ("Du weiß noch, dass du das gestern gesagt hast.", 1),
        ("Auch morgen leben du", 1),
        ("Auch morgen leben er.", 1),
        ("Auch morgen leben ich.", 1),
        ("Auch morgen lebte wir noch.", 1),
        ("Du können heute leider nicht kommen.", 1),
        ("Du leben.", 1),
        ("Du wünscht dir so viel.", 1),
        ("Er können heute leider nicht kommen.", 1),
        ("Ich leben.", 1),
        ("Lebe du?", 1),
        ("Leben du?", 1),
        ("Nett sind du.", 1),
        ("Nett sind er.", 1),
        ("Wir könnt heute leider nicht kommen.", 1),
        ("Wünscht du dir mehr Zeit?", 1),
        (
            "Er sagte düster: „Ich brauchen mich nicht schuldig fühlen.“",
            1,
        ),
        ("Er sagte: „Ich brauchen mich nicht schuldig fühlen.“", 1),
    ] {
        assert_eq!(
            matches(text).len(),
            n,
            "should be {n} bad: {text}: {:?}",
            matches(text)
        );
    }
}

#[test]
fn subject_verb_agreement_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_SUBJECT_VERB_AGREEMENT"]) else {
        return;
    };
    let rule = "DE_SUBJECT_VERB_AGREEMENT";
    let count = |text: &str| -> usize {
        engine
            .check(text)
            .unwrap()
            .matches
            .iter()
            .filter(|m| m.rule_id == rule)
            .count()
    };
    // `testRuleWithIncorrectSingularVerb` (bad)
    for text in [
        "Die Autos ist schnell.",
        "Der Hund und die Katze ist draußen.",
        "Ein Hund und eine Katze ist schön.",
        "Der Hund und die Katze ist schön.",
        "Der große Hund und die Katze ist schön.",
        "Der Hund und die graue Katze ist schön.",
        "Der große Hund und die graue Katze ist schön.",
        "Die Kenntnisse ist je nach Bildungsgrad verschieden.",
        "Die Kenntnisse der Sprachen ist je nach Bildungsgrad verschieden.",
        "Die Kenntnisse der Sprache ist je nach Bildungsgrad verschieden.",
        "Die Kenntnisse der europäischen Sprachen ist je nach Bildungsgrad verschieden.",
        "Die Kenntnisse der neuen europäischen Sprachen ist je nach Bildungsgrad verschieden.",
        "Die Kenntnisse der deutschen Sprache ist je nach Bildungsgrad verschieden.",
        "Die Kenntnisse der aktuellen deutschen Sprache ist je nach Bildungsgrad verschieden.",
        "Drei Katzen ist im Haus.",
        "Drei kleine Katzen ist im Haus.",
        "Viele Katzen ist schön.",
        "Drei Viertel der Erdoberfläche ist Wasser.",
        "Die ältesten und bekanntesten Maßnahmen ist die Einrichtung von Schutzgebieten.",
        "Ein Gramm Pfeffer waren früher wertvoll.",
        "Isolation und ihre Überwindung ist ein häufiges Thema in der Literatur.",
    ] {
        assert!(count(text) > 0, "should match: {text}");
    }
    // `testRuleWithCorrectSingularVerb` (good)
    for text in [
        "Abschluss und Höhepunkt ist der Festumzug.",
        "Abschluss und Höhepunkt ist der Festumzug, der im Oktober stattfindet.",
        "All diesen Stadtteilen ist die Nähe zum Hamburger Hafen und zu den Industrie- und Gewerbegebieten gemein.",
        "All diesen Bereichen ist gemeinsam, dass sie unterfinanziert sind.",
        "Nicht entmutigen lassen, nur weil Sie kein Genie sind.",
        "Denken Sie daran, dass Sie hier zu Gast sind und sich entsprechend verhalten sollten.",
        "Ist es wahr, dass Sie ein guter Mensch sind?",
        "Die Katze ist schön.",
        "Die eine Katze ist schön.",
        "Eine Katze ist schön.",
        "Für Tunesiens Tourismusindustrie mit seinen Stränden, Oasen und antiken Kulturschätzen ist das Attentat ein verheerender Rückschlag.",
        "Man darf jedoch auch nicht glauben, weil die Brustvergrößerungscremes ein Allheilmittel sind, jedoch sind Sie optimal um Ihr Selbstvertrauen zu erhöhen",
        "Die Rechte der Kinder sind universell.",
        "Beiden Filmen war kein Erfolg beschieden.",
        "In einigen Fällen ist der vermeintliche Beschützer schwach.",
        "Was Wasser für die Fische ist.",
        "In den letzten Jahrzehnten ist die Zusammenarbeit der Astronomie verbessert worden.",
        "Für Oberleitungen bei elektrischen Bahnen ist es dagegen anders.",
        "... deren Thema die Liebe zwischen männlichen Charakteren ist.",
        "Mehr als das in westlichen Produktionen der Fall ist.",
        "Da das ein fast aussichtsloses Unterfangen ist.",
        "Was sehr verbreitet bei der Synthese organischer Verbindungen ist.",
        "In chemischen Komplexverbindungen ist das Kation wichtig.",
        "In chemischen Komplexverbindungen ist das As5+-Kation wichtig.",
        "Die selbstständige Behandlung psychischer Störungen ist jedoch ineffektiv.",
        "Die selbstständige Behandlung eigener psychischer Störungen ist jedoch ineffektiv.",
        "Im Gegensatz zu anderen akademischen Berufen ist es in der Medizin durchaus üblich ...",
        "Im Unterschied zu anderen Branchen ist Ärzten anpreisende Werbung verboten.",
        "Aus den verfügbaren Quellen ist es ersichtlich.",
        "Das Mädchen mit den langen Haaren ist Judy.",
        "Der Durchschnitt offener Mengen ist nicht notwendig offen.",
        "Der Durchschnitt vieler offener Mengen ist nicht notwendig offen.",
        "Der Durchschnitt unendlich vieler offener Mengen ist nicht notwendig offen.",
        "Der Ausgangspunkt für die heute gebräuchlichen Alphabete ist ...",
        "Nach sieben männlichen Amtsvorgängern ist Merkel ...",
        "Für einen japanischen Hamburger ist er günstig.",
        "Derzeitiger Bürgermeister ist seit 2008 der ehemalige Minister Müller.",
        "Derzeitiger Bürgermeister der Stadt ist seit 2008 der ehemalige Minister Müller.",
        "Die Eingabe mehrerer assoziativer Verknüpfungen ist beliebig.",
        "Die inhalative Anwendung anderer Adrenalinpräparate zur Akutbehandlung asthmatischer Beschwerden ist somit außerhalb der arzneimittelrechtlichen Zulassung.",
        "Die Kategorisierung anhand morphologischer Merkmale ist nicht objektivierbar.",
        "Die Kategorisierung mit morphologischen Merkmalen ist nicht objektivierbar.",
        "Ute, deren Hauptproblem ihr Mangel an Problemen ist, geht baden.",
        "Ute, deren Hauptproblem ihr Mangel an realen Problemen ist, geht baden.",
        "In zwei Wochen ist Weihnachten.",
        "In nur zwei Wochen ist Weihnachten.",
        "Mit chemischen Methoden ist es möglich, das zu erreichen.",
        "Für die Stadtteile ist auf kommunalpolitischer Ebene jeweils ein Beirat zuständig.",
        "Für die Stadtteile und selbständigen Ortsteile ist auf kommunalpolitischer Ebene jeweils ein Beirat zuständig.",
        "Die Qualität der Straßen ist unterschiedlich.",
        "In deutschen Installationen ist seit Version 3.3 ein neues Feature vorhanden.",
        "In deren Installationen ist seit Version 3.3 ein neues Feature vorhanden.",
        "In deren deutschen Installationen ist seit Version 3.3 ein neues Feature vorhanden.",
        "Die Führung des Wortes in Unternehmensnamen ist nur mit Genehmigung zulässig.",
        "Die Führung des Wortes in Unternehmensnamen und Institutionen ist nur mit Genehmigung zulässig.",
        "Die Hintereinanderreihung mehrerer Einheitenvorsatznamen oder Einheitenvorsatzzeichen ist nicht zulässig.",
        "Eines ihrer drei Autos ist blau und die anderen sind weiß.",
        "Eines von ihren drei Autos ist blau und die anderen sind weiß.",
        "Bei fünf Filmen war Robert F. Boyle für das Production Design verantwortlich.",
        "Insbesondere das Wasserstoffatom als das einfachste aller Atome war dabei wichtig.",
        "In den darauf folgenden Wochen war die Partei führungslos",
        "Gegen die wegen ihrer Schönheit bewunderte Phryne ist ein Asebie-Prozess überliefert.",
        "Dieses für Ärzte und Ärztinnen festgestellte Risikoprofil ist berufsunabhängig.",
        "Das ist problematisch, da kDa eine Masseeinheit und keine Gewichtseinheit ist.",
        "Nach sachlichen oder militärischen Kriterien war das nicht nötig.",
        "Die Pyramide des Friedens und der Eintracht ist ein Bauwerk.",
        "Ohne Architektur der Griechen ist die westliche Kultur der Neuzeit nicht denkbar.",
        "Ohne Architektur der Griechen und Römer ist die westliche Kultur der Neuzeit nicht denkbar.",
        "Ohne Architektur und Kunst der Griechen und Römer ist die westliche Kultur der Neuzeit nicht denkbar.",
        "In denen jeweils für eine bestimmte Anzahl Elektronen Platz ist.",
        "Mit über 1000 Handschriften ist Aristoteles ein Vielschreiber.",
        "Mit über neun Handschriften ist Aristoteles ein Vielschreiber.",
        "Die Klammerung assoziativer Verknüpfungen ist beliebig.",
        "Die Klammerung mehrerer assoziativer Verknüpfungen ist beliebig.",
        "Einen Sonderfall bildete jedoch Ägypten, dessen neue Hauptstadt Alexandria eine Gründung Alexanders und der Ort seines Grabes war.",
        "Jeder Junge und jedes Mädchen war erfreut.",
        "Jedes Mädchen und jeder Junge war erfreut.",
        "Jede Frau und jeder Junge war erfreut.",
        "Als Wissenschaft vom Erleben des Menschen einschließlich der biologischen Grundlagen ist die Psychologie interdisziplinär.",
        "Als Wissenschaft vom Erleben des Menschen einschließlich der biologischen und sozialen Grundlagen ist die Psychologie interdisziplinär.",
        "Als Wissenschaft vom Erleben des Menschen einschließlich der biologischen und neurowissenschaftlichen Grundlagen ist die Psychologie interdisziplinär.",
        "Als Wissenschaft vom Erleben und Verhalten des Menschen einschließlich der biologischen bzw. sozialen Grundlagen ist die Psychologie interdisziplinär.",
        "Alle vier Jahre ist dem Volksfest das Landwirtschaftliche Hauptfest angeschlossen.",
        "Aller Anfang ist schwer.",
        "Alle Dichtung ist zudem Darstellung von Handlungen.",
        "Allen drei Varianten ist gemeinsam, dass meistens nicht unter bürgerlichem...",
        "Er sagte, dass es neun Uhr war.",
        "Auch den Mädchen war es untersagt, eine Schule zu besuchen.",
        "Das dazugehörende Modell der Zeichen-Wahrscheinlichkeiten ist unter Entropiekodierung beschrieben.",
        "Ein über längere Zeit entladener Akku ist zerstört.",
        "Der Fluss mit seinen Oberläufen Río Paraná und Río Uruguay ist der wichtigste Wasserweg.",
        "In den alten Mythen und Sagen war die Eiche ein heiliger Baum.",
        "In den alten Religionen, Mythen und Sagen war die Eiche ein heiliger Baum.",
        "Zehn Jahre ist es her, seit ich mit achtzehn nach Tokio kam.",
        "Bei den niedrigen Oberflächentemperaturen ist Wassereis hart wie Gestein.",
        "Bei den sehr niedrigen Oberflächentemperaturen ist Wassereis hart wie Gestein.",
        "Die älteste und bekannteste Maßnahme ist die Einrichtung von Schutzgebieten.",
        "Die größte Dortmunder Grünanlage ist der Friedhof.",
        "Die größte Berliner Grünanlage ist der Friedhof.",
        "Die größte Bielefelder Grünanlage ist der Friedhof.",
        "Die Pariser Linie ist hier mit 2,2558 mm gerechnet.",
        "Die Frankfurter Innenstadt ist 7 km entfernt.",
        "Die Dortmunder Konzernzentrale ist ein markantes Gebäude an der Bundesstraße 1.",
        "Die Düsseldorfer Brückenfamilie war ursprünglich ein Sammelbegriff.",
        "Die Düssel ist ein rund 40 Kilometer langer Fluss.",
        "Die Berliner Mauer war während der Teilung Deutschlands die Grenze.",
        "Für amtliche Dokumente und Formulare ist das anders.",
        "Wie viele Kilometer ist ihre Stadt von unserer entfernt?",
        "Über laufende Sanierungsmaßnahmen ist bislang nichts bekannt.",
        "In den letzten zwei Monate war ich fleißig wie eine Biene.",
        "Durch Einsatz größerer Maschinen und bessere Kapazitätsplanung ist die Zahl der Flüge gestiegen.",
        "Die hohe Zahl dieser relativ kleinen Verwaltungseinheiten ist immer wieder Gegenstand von Diskussionen.",
        "Teil der ausgestellten Bestände ist auch die Bierdeckel-Sammlung.",
        "Teil der umfangreichen dort ausgestellten Bestände ist auch die Bierdeckel-Sammlung.",
        "Teil der dort ausgestellten Bestände ist auch die Bierdeckel-Sammlung.",
        "Der zweite Teil dieses Buches ist in England angesiedelt.",
        "Eine der am meisten verbreiteten Krankheiten ist die Diagnose",
        "Eine der verbreitetsten Krankheiten ist hier.",
        "Die Krankheit unserer heutigen Städte und Siedlungen ist folgendes.",
        "Die darauffolgenden Jahre war er ...",
        "Die letzten zwei Monate war ich fleißig wie eine Biene.",
        "Bei sehr guten Beobachtungsbedingungen ist zu erkennen, dass ...",
        "Die beste Rache für Undank und schlechte Manieren ist Höflichkeit.",
        "Ein Gramm Pfeffer war früher wertvoll.",
        "Die größte Stuttgarter Grünanlage ist der Friedhof.",
        "Mancher will Meister sein und ist kein Lehrjunge gewesen.",
        "Ellen war vom Schock ganz bleich.",
        "Nun gut, die Nacht ist sehr lang, oder?",
        "Der Morgen ist angebrochen, die lange Nacht ist vorüber.",
        "Die stabilste und häufigste Oxidationsstufe ist dabei −1.",
        "Man kann nicht eindeutig zuordnen, wer Täter und wer Opfer war.",
        "Ich schätze, die Batterie ist leer.",
        "Der größte und schönste Tempel eines Menschen ist in ihm selbst.",
        "Begehe keine Dummheit zweimal, die Auswahl ist doch groß genug!",
        "Seine größte und erfolgreichste Erfindung war die Säule.",
        "Egal was du sagst, die Antwort ist Nein.",
        "... in der Geschichte des Museums, die Sammlung ist seit 2011 zugänglich.",
        "Deren Bestimmung und Funktion ist allerdings nicht so klar.",
        "Sie hat eine Tochter, die Pianistin ist.",
        "Ja, die Milch ist sehr gut.",
        "Der als Befestigung gedachte östliche Teil der Burg ist weitgehend verfallen.",
        "Das Kopieren und Einfügen ist sehr nützlich.",
        "Der letzte der vier großen Flüsse ist die Kolyma.",
        "In christlichen, islamischen und jüdischen Traditionen ist das höchste Ziel der meditativen Praxis.",
        "Der Autor der beiden Spielbücher war Markus Heitz selbst.",
        "Der Autor der ersten beiden Spielbücher war Markus Heitz selbst.",
        "Das Ziel der elf neuen Vorstandmitglieder ist klar definiert.",
        "Laut den meisten Quellen ist das Seitenverhältnis der Nationalflagge...",
        "Seine Novelle, die eigentlich eine Glosse ist, war toll.",
        "Für in Österreich lebende Afrikaner und Afrikanerinnen ist dies nicht üblich.",
        "Von ursprünglich drei Almhütten ist noch eine erhalten.",
        "Einer seiner bedeutendsten Kämpfe war gegen den späteren Weltmeister.",
        "Aufgrund stark schwankender Absatzmärkte war die GEFA-Flug Mitte der 90er Jahre gezwungen, ...",
        "Der Abzug der Besatzungssoldaten und deren mittlerweile ansässigen Angehörigen der Besatzungsmächte war vereinbart.",
        "Das Bündnis zwischen der Sowjetunion und Kuba war für beide vorteilhaft.",
        "Knapp acht Monate ist die Niederlage nun her.",
        "Vier Monate ist die Niederlage nun her.",
        "Sie liebt Kunst und Kunst war auch kein Problem, denn er würde das Geld zurückkriegen.",
        "Bei komplexen und andauernden Störungen ist der Stress-Stoffwechsel des Hundes entgleist.",
        "Eltern ist der bisherige Kita-Öffnungsplan zu unkonkret",
        "Einer der bedeutendsten Māori-Autoren der Gegenwart ist Witi Ihimaera.",
        "Start und Ziel ist Innsbruck",
        "Heute ist sie lieb.",
        "Anfänger wie auch Fortgeschrittene sind herzlich willkommen!",
        "Die Aussichten für Japans Zukunft sind düster.",
        "Das Angeln an Mallorcas Felsküsten ist überaus Erfolg versprechend.",
        "Das bedeutendste Bauwerk und Wahrzeichen der Stadt ist die ehemalige Klosterkirche des Klosters Hofen.",
        "Das saisonale Obst und Gemüse ist köstlich und oft deutlich günstiger als in der Stadt.",
        "Gründer und Leiter des Zentrums ist der Rabbiner Marvin Hier, sein Stellvertreter ist Rabbi Abraham Cooper.",
        "Dank unserer Kunden, Freunde, Partner und unserer Mitarbeiter ist Alpenwahnsinn zur Heimatadresse für schöne Trachtenmode geworden.",
        "Laut seiner Recherche und mehreren Berichten der Welt ist die Zahl zu niedrig gegriffen.",
    ] {
        assert_eq!(count(text), 0, "should be good: {text}");
    }
    // `testRuleWithIncorrectPluralVerb` (bad)
    for text in [
        "Die Katze sind schön.",
        "Die Katze waren schön.",
        "Der Text sind gut.",
        "Das Auto sind schnell.",
        "Herr Schröder sind alt.",
        "Julia und Karsten ist alt.",
        "Julia, Heike und Karsten ist alt.",
        "Herr Karsten Schröder sind alt.",
    ] {
        assert!(count(text) > 0, "should match: {text}");
    }
    // `testRuleWithCorrectPluralVerb` (good)
    for text in [
        "Wenn Sie kein Teil der Lösung sind, sind Sie ein Teil des Problems.",
        "Er bemerkte, dass Experimente nicht gerade sein Ding sind",
        "Die meisten der Spieler sind nicht vermögend",
        "Wie viel Prozent der Menschen sind total bescheuert?",
        "Glaubt wirklich jemand, dass gute Fotos keine Arbeit sind?",
        "Zwei Schülern war aufgefallen, dass man im Fernsehen dazu nichts mehr sieht.",
        "Auch die Reste eines sehr großen Insektenfressers sind unter den Fossilien.",
        "Eine Persönlichkeit sind Sie selbst.",
        "Die Katzen sind schön.",
        "Frau Meier und Herr Müller sind alt.",
        "Frau Julia Meier und Herr Karsten Müller sind alt.",
        "Julia und Karsten sind alt.",
        "Julia, Heike und Karsten sind alt.",
        "Frau und Herr Müller sind alt.",
        "Herr und Frau Schröder sind alt.",
        "Herr Meier und Frau Schröder sind alt.",
        "Die restlichen 86 Prozent sind in der Flasche.",
        "Die restlichen sechsundachtzig Prozent sind in der Flasche.",
        "Die restlichen 86 oder 87 Prozent sind in der Flasche.",
        "Die restlichen 86 % sind in der Flasche.",
        "Durch den schnellen Zerfall des Actiniums waren stets nur geringe Mengen verfügbar.",
        "Soda und Anilin waren die ersten Produkte des Unternehmens.",
        "Bob und Tom sind Brüder.",
        "Letztes Jahr sind wir nach London gegangen.",
        "Trotz des Regens sind die Kinder in die Schule gegangen.",
        "Die Zielgruppe sind Männer.",
        "Männer sind die Zielgruppe.",
        "Die Zielgruppe sind meist junge Erwachsene.",
        "Die USA sind ein repräsentativer demokratischer Staat.",
        "Wesentliche Eigenschaften der Hülle sind oben beschrieben.",
        "Wesentliche Eigenschaften der Hülle sind oben unter Quantenmechanische Atommodelle und Erklärung grundlegender Atomeigenschaften dargestellt.",
        "Er und seine Schwester sind eingeladen.",
        "Er und seine Schwester sind zur Party eingeladen.",
        "Sowohl er als auch seine Schwester sind zur Party eingeladen.",
        "Rekonstruktionen oder der Wiederaufbau sind wissenschaftlich sehr umstritten.",
        "Form und Materie eines Einzeldings sind aber nicht zwei verschiedene Objekte.",
        "Dieses Jahr sind die Birnen groß.",
        "Es so umzugestalten, dass sie wie ein Spiel sind.",
        "Die Zielgruppe sind meist junge Erwachsene.",
        "Die Ursache eines Hauses sind so Ziegel und Holz.",
        "Vertreter dieses Ansatzes sind unter anderem Roth und Meyer.",
        "Sowohl sein Vater als auch seine Mutter sind tot.",
        "Einige der Inhaltsstoffe sind schädlich.",
        "Diese Woche sind wir schon einen großen Schritt weiter.",
        "Diese Woche sind sie hier.",
        "Vorsitzende des Vereins waren:",
        "Weder Gerechtigkeit noch Freiheit sind möglich, wenn nur das Geld regiert.",
        "Ein typisches Beispiel sind Birkenpollenallergene.",
        "Eine weitere Variante sind die Miniatur-Wohnlandschaften.",
        "Eine Menge englischer Wörter sind aus dem Lateinischen abgeleitet.",
        "Völkerrechtlich umstrittenes Territorium sind die Falklandinseln.",
        "Einige dieser älteren Synthesen sind wegen geringer Ausbeuten ...",
        "Einzelne Atome sind klein.",
        "Die Haare dieses Jungens sind schwarz.",
        "Die wichtigsten Mechanismen des Aminosäurenabbaus sind:",
        "Wasserlösliche Bariumverbindungen sind giftig.",
        "Die Schweizer Trinkweise ist dabei die am wenigsten etablierte.",
        "Die Anordnung der vier Achsen ist damit identisch.",
        "Die Nauheimer Musiktage, die immer wieder ein kultureller Höhepunkt sind.",
        "Räumliche und zeitliche Abstände sowie die Trägheit sind vom Bewegungszustand abhängig.",
        "Solche Gewerbe sowie der Karosseriebau sind traditionell stark vertreten.",
        "Hundert Dollar sind doch gar nichts!",
        "Sowohl Tom als auch Maria waren überrascht.",
        "Robben, die die hauptsächliche Beute der Eisbären sind.",
        "Die Albatrosse sind eine Gruppe von Seevögeln",
        "Die Albatrosse sind eine Gruppe von großen Seevögeln",
        "Die Albatrosse sind eine Gruppe von großen bis sehr großen Seevögeln",
        "Vier Elemente, welche der Urstoff aller Körper sind.",
        "Die Beziehungen zwischen Kanada und dem Iran sind seitdem abgebrochen.",
        "Die diplomatischen Beziehungen zwischen Kanada und dem Iran sind seitdem abgebrochen.",
        "Die letzten zehn Jahre seines Lebens war er erblindet.",
        "Die letzten zehn Jahre war er erblindet.",
        "... so dass Knochenbrüche und Platzwunden die Regel sind.",
        "Die Eigentumsverhältnisse an der Gesellschaft sind unverändert geblieben.",
        "Gegenstand der Definition sind für ihn die Urbilder.",
        "Mindestens zwanzig Häuser sind abgebrannt.",
        "Sie hielten geheim, dass sie Geliebte waren.",
        "Einige waren verspätet.",
        "Kommentare, Korrekturen und Kritik sind verboten.",
        "Kommentare, Korrekturen, Kritik sind verboten.",
        "Letztere sind wichtig, um die Datensicherheit zu garantieren.",
        "Jüngere sind oft davon überzeugt, im Recht zu sein.",
        "Verwandte sind selten mehr als Bekannte.",
        "Ursache waren die hohe Arbeitslosigkeit und die Wohnungsnot.",
        "Ursache waren unter anderem die hohe Arbeitslosigkeit und die Wohnungsnot.",
        "Er ahnt nicht, dass sie und sein Sohn ein Paar sind.",
        "Die Ursachen der vorliegenden Durchblutungsstörung sind noch unbekannt.",
        "Der See und das Marschland sind ein Naturschutzgebiet",
        "Details, Dialoge, wie auch die Typologie der Charaktere sind frei erfunden.",
        "Die internen Ermittler und auch die Staatsanwaltschaft sind nun am Zug.",
        "Sie sind so erfolgreich, weil sie eine Einheit sind.",
        "Wie viele Erwerbstätige sind im Gesundheitswesen beschäftigt?",
        "Auch Polizisten zu Fuß sind unterwegs.",
        "Julia sagte, dass Vater und Mutter zu Hause sind.",
        "Damit müssen sie zurechtkommen, wenn Kinder zu Hause sind.",
        "Auch Studien zu Zink sind vielversprechend.",
        "Die Züge vor Ort sind nicht klimatisiert.",
        "Ich verspreche dir, dass wir ein tolles Team sind.",
    ] {
        assert_eq!(count(text), 0, "should be good: {text}");
    }
    // `testRuleWithCorrectSingularAndPluralVerb` (both are fine)
    for text in [
        "Solchen Personen ist der Zugriff auf diese Daten verboten.",
        "Personen ist der Zugriff auf diese Daten verboten.",
        "So mancher Mitarbeiter und manche Führungskraft ist im Urlaub.",
        "So mancher Mitarbeiter und manche Führungskraft sind im Urlaub.",
        "Jeder Schüler und jede Schülerin ist mal schlecht gelaunt.",
        "Jeder Schüler und jede Schülerin sind mal schlecht gelaunt.",
        "Kaum mehr als vier Prozent der Fläche ist für landwirtschaftliche Nutzung geeignet.",
        "Kaum mehr als vier Prozent der Fläche sind für landwirtschaftliche Nutzung geeignet.",
        "Kaum mehr als vier Millionen Euro des Haushalts ist verplant.",
        "Kaum mehr als vier Millionen Euro des Haushalts sind verplant.",
        "80 Cent ist nicht genug.",
        "80 Cent sind nicht genug.",
        "1,5 Pfund ist nicht genug.",
        "1,5 Pfund sind nicht genug.",
        "Hier ist sowohl Anhalten wie Parken verboten.",
        "Hier sind sowohl Anhalten wie Parken verboten.",
        "Der Eifer der Männer und Frauen ist enorm.",
    ] {
        assert_eq!(count(text), 0, "should be good: {text}");
    }
    // `testArrayOutOfBoundsBug`: must not panic
    let _ = engine
        .check("Die nicht Teil des Näherungsmodells sind")
        .unwrap();

    // Java probes: message with `<suggestion>` markup, offsets and suggestion
    let m = engine.check("Die Autos ist schnell.").unwrap();
    let m: Vec<_> = m.matches.iter().filter(|m| m.rule_id == rule).collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (10, 13));
    assert_eq!(
        m[0].message,
        "Bitte prüfen, ob hier <suggestion>sind</suggestion> stehen sollte."
    );
    assert_eq!(m[0].suggestions[0].value, "sind");
    let m = engine.check("Das Auto sind schnell.").unwrap();
    let m: Vec<_> = m.matches.iter().filter(|m| m.rule_id == rule).collect();
    assert_eq!((m[0].range.start, m[0].range.end), (9, 13));
    assert_eq!(m[0].suggestions[0].value, "ist");
}

#[test]
fn agreement_smoke_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_AGREEMENT", "DE_AGREEMENT2"]) else {
        return;
    };
    let report = |text: &str| -> Vec<String> {
        engine
            .check(text)
            .unwrap()
            .matches
            .iter()
            .filter(|m| m.rule_id.starts_with("DE_AGREEMENT"))
            .map(|m| {
                format!(
                    "{} {}..{}|{}|{}",
                    m.rule_id,
                    m.range.start,
                    m.range.end,
                    m.message,
                    m.suggestions
                        .iter()
                        .map(|s| s.value.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect()
    };
    let m = report("Das ist die Original Mail");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].contains("die Originalmail, die Original-Mail"),
        "{m:?}"
    );
    let m = report("Der Haus wurde letztes Jahr gebaut.");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("Dem Haus, Das Haus, Der Häuser"), "{m:?}");
    assert!(report("Die Katze ist schön.").is_empty());
    let m = report("Wirtschaftlicher Wachstum");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("Wirtschaftliches Wachstum"), "{m:?}");
    assert_eq!(m[0].split(' ').next(), Some("DE_AGREEMENT2"));
    let m = report("Kleiner Haus am Waldrand");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("Kleines Haus"), "{m:?}");
}

#[test]
fn agreement_rule_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_AGREEMENT"]) else {
        return;
    };
    let rule = "DE_AGREEMENT";
    let matches = |text: &str| -> Vec<(usize, usize, Vec<String>)> {
        engine
            .check(text)
            .unwrap()
            .matches
            .iter()
            .filter(|m| m.rule_id == rule)
            .map(|m| {
                (
                    m.range.start,
                    m.range.end,
                    m.suggestions
                        .iter()
                        .map(|s| s.value.clone())
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    };
    // assertGood
    for text in [
        "Es gibt ein Sprichwort, dem zufolge der tägliche Genuss einer Mandel dem Gedächtnis förderlich sei.",
        "War das Eifersucht?",
        "Sie gehörte einst zu den besten Afrikas.",
        "Dieses Bild stammt von einem lange Zeit unbekannten Maler.",
        "Das Staatsoberhaupt ist der Verfassung zufolge der König.",
        "Der Ende der 1960er Jahre umgestaltete Garten ist schön.",
        "Der Ende der achtziger Jahre umgestaltete Garten hat unter anderem ungefähr 70 verschiedene Sorten von Rosen und Volieren für exotische Vögel.",
        "Als Vorboten des Discounthandels sind die Ende der 50er Jahre in der Bundesrepublik Deutschland wiederauflebenden Erscheinungsformen des Beziehungs-, Betriebs- und Belegschaftshandels anzusehen.",
        "Die Anfang des letzten Monats umgestaltete Veranda ist schön.",
        "Der Mitte 2001 umgestaltete Garten ist schön.",
        "Bis zur Anfang Juni geplanten Eröffnung gebe es noch einiges zu tun.",
        "Der fließend Französisch sprechende Präsident dankt stilvoll ab.",
        "Inwiefern soll denn das romantische Hoffnungen begründen?",
        "Spricht der fließend Französisch?",
        "Spricht dieser fließend Französisch, muss er viel Geld verdienen.",
        "Der letzte Woche beschlossene Etat ist unwirksam.",
        "Die Einen sagen dies, die Anderen das.",
        "So ist es in den USA.",
        "Das ist der Tisch.",
        "Das ist das Haus.",
        "Das ist die Frau.",
        "Das ist das Auto der Frau.",
        "Das gehört dem Mann.",
        "Das Auto des Mannes.",
        "Das interessiert den Mann.",
        "Das interessiert die Männer.",
        "Das Auto von einem Mann.",
        "Das Auto eines Mannes.",
        "Des großen Mannes.",
        "Und nach der Nummerierung kommt die Überschrift.",
        "Sie wiesen dieselben Verzierungen auf.",
        "Die erwähnte Konferenz ist am Samstag.",
        "Sie erreichten 5 Prozent.",
        "Sie erreichten mehrere Prozent Zustimmung.",
        "Die Bestandteile, aus denen Schwefel besteht.",
        "Ich tat für ihn, was kein anderer Autor für ihn tat.",
        "Ich tat für ihn, was keine andere Autorin für ihn tat.",
        "Ich tat für ihn, was kein anderes Kind für ihn tat.",
        "Ich tat für ihn, was dieser andere Autor für ihn tat.",
        "Ich tat für ihn, was diese andere Autorin für ihn tat.",
        "Ich tat für ihn, was dieses andere Kind für ihn tat.",
        "Ich tat für ihn, was jener andere Autor für ihn tat.",
        "Ich tat für ihn, was jeder andere Autor für ihn tat.",
        "Ich tat für ihn, was jede andere Autorin für ihn tat.",
        "Ich tat für ihn, was jedes andere Kind für ihn tat.",
        "Klebe ein Preisschild auf jedes einzelne Produkt.",
        "Eine Stadt, in der zurzeit eine rege Bautätigkeit herrscht.",
        "... wo es zu einer regen Bautätigkeit kam.",
        "Mancher ausscheidende Politiker hinterlässt eine Lücke.",
        "Kern einer jeden Tragödie ist es, ..",
        "Das wenige Sekunden alte Baby schrie laut.",
        "Meistens sind das Frauen, die damit besser umgehen können.",
        "Er fragte, ob das Spaß macht.",
        "Das viele Geld wird ihr helfen.",
        "Er verspricht jedem hohe Gewinne.",
        "Er versprach allen Renditen jenseits von 15 Prozent.",
        "Sind das Eier aus Bodenhaltung?",
        "Sie sind sehr gute Kameraden, auf die Verlass ist.",
        "Dir macht doch irgendwas Sorgen.",
        "Sie fragte, ob das wirklich Kunst sei.",
        "Für ihn ist das Alltag.",
        "Für die Religiösen ist das Blasphemie und führt zu Aufständen.",
        "Das Orange ist schön.",
        "Dieses rötliche Orange gefällt mir am besten.",
        "Das ist ein super Tipp.",
        "Er nahm allen Mut zusammen und ging los.",
        "Sie kann einem Angst einjagen.",
        "Damit sollten zum einen neue Energien gefördert werden, zum anderen der Sozialbereich.",
        "Nichts ist mit dieser einen Nacht zu vergleichen.",
        "dann muss Schule dem Rechnung tragen.",
        "Das Dach von meinem Auto.",
        "Das Dach von meinen Autos.",
        "Da stellt sich die Frage: Ist das Science-Fiction oder moderne Mobilität?",
        "Er hat einen Post veröffentlicht.",
        "Eine lückenlose Aufklärung sämtlicher physiologischer Gehirnprozesse",
        "Sie fragte verwirrt: „Ist das Zucker?“",
        "Er versuchte sich vorzustellen, was sein Klient für ein Mensch sei.",
        "Sie legen ein Teilstück jenes Weges zurück, den die Tausenden Juden 1945 auf sich nehmen mussten.",
        "Aber das ignorierte Herr Grey bewusst.",
        "Aber das ignorierte Herr Müller bewusst.",
        "Ich werde mich zurücknehmen und mich frischen Ideen zuwenden.",
        "Das, plus ein eigener Firmenwagen.",
        "Dieses leise Summen stört nicht.",
        "Die Tiroler Küche",
        "Was ist denn das für ein ungewöhnlicher Name?",
        "Besonders reizen mich Fahrräder.",
        "Und nur, weil mich psychische Erkrankungen aus der Bahn werfen",
        "Das kostet dich Zinsen.",
        "Sie hatten keine Chance gegen das kleinere Preußen.",
        "Den 2019er Wert hatten sie geschätzt.",
        "Andere formale Systeme, deren Semantiken jeweils...",
        "Gesetz zur Änderung des Kündigungsrechts und anderer arbeitsrechtlicher Vorschriften",
        "Die dauerhafte Abgrenzung des später Niedersachsen genannten Gebietes von Westfalen begann im 12. Jahrhundert.",
        "Lieber jemanden, der einem Tipps gibt.",
        "Jainas ist sogar der Genuss jeglicher tierischer Nahrungsmittel strengstens untersagt.",
        "Es sind jegliche tierische Nahrungsmittel untersagt.",
        "Das reicht bis weit ins heutige Hessen.",
        "Die Customer Journey.",
        "Für dich gehört Radfahren zum perfekten Urlaub dazu?",
        ":D:D Leute, bitte!",
        "Es genügt, wenn ein Mann sein eigenes Geschäft versteht und sich nicht in das anderer Leute einmischt.",
        "Ich habe das einige Male versucht.",
        "Und keine Märchen erzählst, die dem anderen Hoffnungen machen können.",
        "Um diese Körpergrößen zu erreichen, war das Wachstum der Vertreter der Gattung Dinornis offenbar gegenüber dem anderer Moa-Gattungen beschleunigt",
        "Der Schädel entspricht in den Proportionen dem anderer Vulpes-Arten, besitzt aber sehr große Paukenhöhlen, ein typisches Merkmal von Wüstenbewohnern.",
        "Deuterium lässt sich aufgrund des großen Massenunterschieds leichter anreichern als die Isotope der anderer Elemente wie z. B. Uran.",
        "Unklar ist, ob er zwischen der Atemseele des Menschen und der anderer Lebewesen unterschied.",
        "Die Liechtensteiner Grenze ist im Verhältnis zu der anderer Länder kurz, da Liechtenstein ein eher kleines Land ist.",
        "Picassos Kunstwerke werden häufiger gestohlen als die anderer Künstler.",
        "Schreibe einen Artikel über deine Erfahrungen im Ausland oder die anderer Leute in deinem Land.",
        "Die Bevölkerungen Chinas und Indiens lassen die anderer Staaten als Zwerge erscheinen.",
        "Der eine mag Obst, ein anderer Gemüse, wieder ein anderer mag Fisch; allen kann man es nicht recht machen.",
        "Mittels eines Bootloaders und zugehöriger Software kann nach jedem Anstecken des Adapters eine andere Firmware-Varianten geladen werden",
        "Wenn sie eine andere Größe benötigen, teilen uns ihre speziellen Wünsche mit und wir unterbreiten ihnen ein Angebot über Preis und Lieferung.",
        "Dabei wird in einer Vakuumkammer eine einige Mikrometer dicke CVD-Diamantschicht auf den Substraten abgeschieden.",
        "1916 versuchte Gilbert Newton Lewis, die chemische Bindung durch Wechselwirkung der Elektronen eines Atoms mit einem anderen Atomen zu erklären.",
        "Vom einen Ende der Straße zum anderen.",
        "Er war müde vom vielen Laufen.",
        "Sind das echte Diamanten?",
        "Es wurde eine Verordnung erlassen, der zufolge jeder Haushalt Energie einsparen muss.",
        "Im Jahr 1922 verlieh ihm König George V. den erblichen Titel eines Baronet. ",
        "... der zu dieser Zeit aber ohnehin schon allen Einfluss verloren hatte.",
        "Ein Geschenk, das Maßstäbe setzt",
        "Einwohnerzahl stieg um das Zweieinhalbfache",
        "Die Müllers aus Hamburg.",
        "Es ist noch unklar, wann und für wen Impfungen vorgenommen werden könnten.",
        "Macht dir das Hoffnung?",
        "Mich fasziniert Macht.",
        "Der solchen Einsätzen gegenüber kritische Hitler wurde nicht im Voraus informiert.",
        "Gregor wählte die Gestalt des wenige Jahrzehnte zuvor verstorbenen Klostergründers.",
        "Wir machen das Januar.",
        "Wir teilen das Morgen mit.",
        "Wir präsentierten das vorletzten Sonnabend.",
        "Ich release das Vormittags.",
        "Sie aktualisieren das Montags.",
        "Kannst du das Mittags machen?",
        "Können Sie das nächsten Monat erledigen?",
        "Können Sie das auch nächsten Monat erledigen?",
        "War das Absicht?",
        "Alles Große und Edle ist einfacher Art.",
        "Dieser vereint Sprachprüfung, Thesaurus und Umformuliertool in einem.",
        "Das Dach meines Autos.",
        "Das Dach meiner Autos.",
        "Das Dach meines großen Autos.",
        "Das Dach meiner großen Autos.",
        "Dann schlug er so kräftig wie er konnte mit den Schwingen.",
        "Also wenn wir Glück haben, ...",
        "Wenn wir Pech haben, ...",
        "Ledorn öffnete eines der an ihr vorhandenen Fächer.",
        "Auf der einen Seite endlose Dünen",
        "In seinem Maul hielt er einen blutigen Fleischklumpen.",
        "Gleichzeitig dachte er intensiv an Nebelschwaden, aus denen Wolken ja bestanden.",
        "Warum stellte der bloß immer wieder dieselben Fragen?",
        "Bei der Hinreise.",
        "Schließlich tauchten in einem Waldstück unter ihnen Schienen auf.",
        "Das Wahlrecht, das Frauen damals zugesprochen bekamen.",
        "Es war Karl, dessen Leiche Donnerstag gefunden wurde.",
        "Erst recht ich Arbeiter.",
        "Erst recht wir Arbeiter.",
        "Erst recht wir fleißigen Arbeiter.",
        "Dann lud er Freunde ein.",
        "Dann lud sie Freunde ein.",
        "Aller Kommunikation liegt dies zugrunde.",
        "Pragmatisch wählt man solche Formeln als Axiome.",
        "Der eine Polizist rief dem anderen zu...",
        "Der eine große Polizist rief dem anderen zu...",
        "Das eine Kind rief dem anderen zu...",
        "Er wollte seine Interessen wahrnehmen.",
        "Es birgt für mich ein zu hohes juristisches Risiko.",
        "... wo Krieg den Unschuldigen Leid und Tod bringt.",
        "Der Abschuss eines Papageien.",
        "Die Beibehaltung des Art. 1 ist geplant.",
        "Die Verschiebung des bisherigen Art. 1 ist geplant.",
        "In diesem Fall hatte das Vorteile.",
        "So hat das Konsequenzen.",
        "Ein für viele wichtiges Anliegen.",
        "Das weckte bei vielen ungute Erinnerungen.",
        "Etwas, das einem Angst macht.",
        "Einem geschenkten Gaul schaut man nicht ins Maul.",
        "Das erfordert Können.",
        "Ist das Kunst?",
        "Ist das Kunst oder Abfall?",
        "Die Zeitdauer, während der Wissen nützlich bleibt, wird kürzer.",
        "Es sollte nicht viele solcher Bilder geben",
        "In den 80er Jahren.",
        "Hast du etwas das Carina machen kann?",
        "Ein Artikel in den Ruhr Nachrichten.",
        "Ich wollte nur allen Hallo sagen.",
        "Ich habe deshalb allen Freund*innen Bescheid gegeben.",
        "Ich habe deshalb allen Freund_innen Bescheid gegeben.",
        "Ich habe deshalb allen Freund:innen Bescheid gegeben.",
        "Das betrifft auch eure Werkstudent:innen-Zielgruppe.",
        "Das betrifft auch eure Werkstudent:innenzielgruppe.",
        "Das betrifft auch eure Jurist:innenausbildung.",
        "Sein*e Mitarbeiter*in ist davon auch betroffen.",
        "Jede*r Mitarbeiter*in ist davon betroffen.",
        "Alle Professor*innen",
        "Gleichzeitig wünscht sich Ihr frostresistenter Mitbewohner einige Grad weniger im eigenen Zimmer?",
        "Ein Trainer, der zum einen Fußballspiele sehr gut lesen und analysieren kann",
        "Eine Massengrenze, bis zu der Lithium nachgewiesen werden kann.",
        "Bei uns im Krankenhaus betrifft das Operationssäle.",
        "Macht dir das Freude?",
        "Das macht jedem Angst.",
        "Dann macht das Sinn.",
        "Das sind beides Lichtschalter.",
        "Spielst du vielleicht auf das Bordell neben unserm Hotel an?",
        "Spielst du vielleicht auf das Bordell neben unsrem Hotel an?",
        "Dieses ungeahnt prophetische Wort",
        "Das bestätigte Regierungssprecher Steffen Hebestreit am Freitag",
        "Es kann gut sein, dass bei sowas Probleme erkannt werden.",
        "Das Recht, das Frauen eingeräumt wird.",
        "Der Mann, in dem quadratische Fische schwammen.",
        "Der Mann, durch den quadratische Fische schwammen.",
        "Gutenberg, der quadratische Mann.",
        "Die größte Stuttgarter Grünanlage ist der Friedhof.",
        "Die meisten Lebensmittel enthalten das.",
        "Die wärmsten Monate sind August und September, die kältesten Januar und Februar.",
        "Das Münchener Fest.",
        "Das Münchner Fest.",
        "Die Planung des Münchener Festes.",
        "Das Berliner Wetter.",
        "Den Berliner Arbeitern ist das egal.",
        "Das Haus des Berliner Arbeiters.",
        "Es gehört dem Berliner Arbeiter.",
        "Das Stuttgarter Auto.",
        "Das Bielefelder Radio.",
        "Das Gütersloher Radio.",
        "Das wirklich Wichtige kommt jetzt erst.",
        "Besonders wenn wir Wermut oder Absinth trinken.",
        "Ich wünsche dir alles Gute.",
        "Es ist nicht bekannt, mit welchem Alter Kinder diese Fähigkeit erlernen.",
        "Dieser ist nun in den Ortungsbereich des einen Roboters gefahren.",
        "Wenn dies großen Erfolg hat, werden wir es weiter fördern.",
        "Alles Gute!",
        "Das bedeutet nichts Gutes.",
        "Die Ereignisse dieses einen Jahres waren sehr schlimm.",
        "Er musste einen Hochwasser führenden Fluss nach dem anderen überqueren.",
        "Darf ich Ihren Füller für ein paar Minuten ausleihen?",
        "Bringen Sie diesen Gepäckaufkleber an Ihrem Gepäck an.",
        "Extras, die den Wert Ihres Autos erhöhen.",
        "Er hat einen 34-jährigen Sohn.",
        "Die Polizei erwischte die Diebin, weil diese Ausweis und Visitenkarte hinterließ.",
        "Dieses Versäumnis soll vertuscht worden sein - es wurde Anzeige erstattet.",
        "Die Firmen - nicht nur die ausländischen, auch die katalanischen - treibt diese Frage um.",
        "Stell dich dem Leben lächelnd!",
        "Die Messe wird auf das vor der Stadt liegende Ausstellungsgelände verlegt.",
        "Sie sind ein den Frieden liebendes Volk.",
        "Zum Teil sind das Krebsvorstufen.",
        "Er sagt, dass das Rache bedeutet.",
        "Wenn das Kühe sind, bin ich ein Elefant.",
        "Karl sagte, dass sie niemandem Bescheid gegeben habe.",
        "Es blieb nur dieser eine Satz.",
        "Oder ist das Mathematikern vorbehalten?",
        "Wenn hier einer Fragen stellt, dann ich.",
        "Wenn einer Katzen mag, dann meine Schwester.",
        "Ergibt das Sinn?",
        "Sie ist über die Maßen schön.",
        "Ich vertraue ganz auf die Meinen.",
        "Was nützt einem Gesundheit, wenn man sonst ein Idiot ist?",
        "Auch das hatte sein Gutes.",
        "Auch wenn es sein Gutes hatte, war es doch traurig.",
        "Er wollte doch nur jemandem Gutes tun.",
        "und das erst Jahrhunderte spätere Auftauchen der Legende",
        "Texas und New Mexico, beides spanische Kolonien, sind...",
        "Texas und New Mexico - beides spanische Kolonien - sind...",
        "Texas und New Mexico – beides spanische Kolonien – sind...",
        "Weitere Brunnen sind insbesondere der Wittelsbacher und der Vater-Rhein-Brunnen auf der Museumsinsel, beides Werke von Adolf von Hildebrand.",
        "Für manche ist das Anlass genug, darüber nicht weiter zu diskutieren.",
        "Vielleicht schreckt das Frauen ab.",
        "Unser Hund vergräbt seine Knochen im Garten.",
        "Ob das Mehrwert bringt?",
        "Warum das Sinn macht?",
        "Das hängt davon ab, ob die Deutsch sprechen",
        "Die meisten Coaches wissen nichts.",
        "Die Präsent AG.",
        "In New York war er der Titelheld in Richard III. und spielte den Mark Anton in Julius Cäsar.",
        "Vielen Dank fürs Bescheid geben.",
        "Welche Display Ads?",
        "Das letzte Mal war das Anfang der 90er Jahre des vergangenen Jahrhunderts",
        "Der vom Rat der Justizminister gefasste Beschluss zur Aufnahme von Vertriebenen...",
        "Der letzte Woche vom Rat der Justizminister gefasste Beschluss zur Aufnahme von Vertriebenen...",
        "Was war sie nur für eine dumme Person!",
        "Was war ich für ein Idiot!",
        "Was für ein Idiot!",
        "Was für eine blöde Kuh!",
        "Was ist sie nur für eine blöde Kuh!",
        "Wie viele Paar Stiefel brauche ich eigentlich?",
        "Dieses versuchten Mathematiker 400 Jahre lang vergeblich zu beweisen.",
        "Gemälde informieren uns über das Leben von den vergangenen Jahrhunderten…",
        "Die Partei, die bei den vorangegangenen Wahlen noch seine Politik unterstützt hatte.",
        "Bei Zunahme der aufgelösten Mineralstoffe, bei denen...",
        "Je mehr Muskelspindeln in einem Muskel vorhanden sind, desto feiner können die mit diesem verbundenen Bewegungen abgestimmt werden.",
        "Diese datentechnischen Operationen werden durch Computerprogramme ausgelöst, d. h. über entsprechende, in diesen enthaltene Befehle (als Teil eines implementierten Algorithmus') vorgegeben.",
        "Aus diesen resultierten Konflikte wie der Bauernkrieg und der Pfälzische Erbfolgekrieg.",
        "Die Staatshandlungen einer Mikronation und von dieser herausgegebene Ausweise, Urkunden und Dokumente gelten im Rechtsverkehr als unwirksam",
        "Auf der Hohen See und auf den mit dieser verbundenen Gewässern gelten die internationalen Kollisionsverhütungsregeln.",
        "Art. 11 Abs. 2 GGV setzt dem bestimmte Arten der außergemeinschaftlichen Zugänglichmachung gleich",
        "Grundsätzlich sind die Heilungschancen von Männern mit Brustkrebs nicht schlechter als die betroffener Frauen.",
        "In diesem Viertel bin ich aufgewachsen.",
        "Im November wurde auf dem Gelände der Wettbewerb ausgetragen.",
        "Er ist Eigentümer des gleichnamigen Schemas und stellt dieses interessierten Domänen zur Verfügung.",
        "Dort finden sie viele Informationen rund um die Themen Schwangerschaft, Geburt, Stillen, Babys und Kinder.",
        "Die Galerie zu den Bildern findet sich hier.",
        "Ganz im Gegensatz zu den Blättern des Brombeerstrauches.",
        "Er erzählte von den Leuten und den Dingen, die er auf seiner Reise gesehen hatte.",
        "Diese Partnerschaft wurde 1989 nach dem Massaker auf dem Platz des Himmlischen Friedens eingefroren.",
        "Die Feuergefahr hingegen war für für die Londoner Teil des Alltags.",
        "Was ist, wenn ein Projekt bei den Berliner Type Awards mit einem Diplom ausgezeichnet wird?",
        "Was ist mit dem Liechtensteiner Kulturleben los?",
        "Das ist der Mann den Präsident Xi Jinping verurteilte.",
        "Wie viele Kolleg/-innen haben sie?",
        "Die Ideen der neuen Kolleg/-innen sind gut!",
        "Das erlaubt Forschern, neue Versuche durchzuführen.",
        "Dies ermöglicht Forschern, neue Versuche durchzuführen.",
        "Je länger zugewartet wird, desto schwieriger dürfte es werden, die Jungtiere von den Elterntieren zu unterscheiden.",
        "Er schrieb ein von 1237 bis 1358 reichendes Geschichtswerk, dessen Schwerpunkt auf den Ereignissen in der Lombardei liegt.",
        "Private Veranstaltungen waren, darauf hat die Strandhaus Norderstedt GmbH im Rahmen rechtlicher Klärungen selbst bestanden, nicht Bestandteil dieser Verträge.",
        "Die Klientel der Partei.",
        "Wenn die Gott zugeschriebenen Eigenschaften stimmen, dann...",
        "Dieses Grünkern genannte Getreide ist aber nicht backbar.",
        "Außerdem unterstützt mich Herr Müller beim abheften",
        "Außerdem unterstützt mich Frau Müller beim abheften",
        "Ich gebe dir ein kleines Kaninchen.",
        "Ich gebe dir das kleine Kaninchen.",
        "Die Top 3 der Umfrage",
        "Dein Vorschlag befindet sich unter meinen Top 5.",
        "Unter diesen rief das großen Unmut hervor.",
        "Bei mir löste das Panik aus.",
        "Sie können das machen in dem sie die CAD.pdf öffnen.",
        "Ich mache eine Ausbildung zur Junior Digital Marketing Managerin.",
        "Dann wird das Konsequenzen haben.",
        "Dann hat das Konsequenzen.",
        "Sollte das Konsequenzen nach sich ziehen?",
        "Der Echo Show von Amazon",
        "Die BVG kommen immer zu spät.",
        "In der Frühe der Nacht.",
        "Der TV Steinfurt.",
        "Ein ID 3 von Volkswagen.",
        "Der ID.3 von Volkswagen.",
        "Der ID3 von Volkswagen.",
        "Das bedeutet Krieg!",
        "Im Tun zu sein verhindert Prokrastination.",
        "Das ist doch lächerlich, was ist denn das für eine Klinik?",
        "Was ist denn das für ein Typ?",
        "Hier geht's zur Customer Journey.",
        "Das führt zur Verbesserung der gesamten Customer Journey.",
        "Meint er das wirklich Ernst?",
        "Meinen Sie das Ernst?",
        "Die können sich in unserer Hall of Fame verewigen.",
        "Die können sich in unserer neuen Hall of Fame verewigen.",
        "Auch, wenn das weite Teile der Bevölkerung betrifft.",
        "Hat das Einfluss auf Ihr Trinkverhalten?",
        "Ihr wisst aber schon, dass das Blödsinn ist.",
        "Aber mein Wissen über die Antike ist ausbaufähig.",
        "Sie werden merken, dass das echte Nutzer sind.",
        "Dieses neue Mac OS trug den Codenamen Rhapsody.",
        "Das Mac OS is besser als Windows.",
        "Damit steht das Porsche Museum wie kaum ein anderes Museum für Lebendigkeit und Abwechslung.",
        "Weitere Krankenhäuser sind dass Eastern Shore Memorial Hospital, IWK Health Centre, Nova Scotia Hospital und das Queen Elizabeth II Health Sciences Centre.",
        "Ich bin von Natur aus ein sehr neugieriger Mensch.",
        "Ich bin auf der Suche nach einer Junior Developerin.",
        "War das Eifersucht?",
        "Waren das schwierige Entscheidungen?",
        "Soll das Demokratie sein?",
        "Hat das Spaß gemacht?",
        "Soll das Sinn stiften?",
        "Soll das Freude machen?",
        "Die Trial ist ausgelaufen.",
        "Ein geworbener Neukunde interagiert zusätzlich mit dem Unternehmen.",
        "Ich weiß, dass jeder LanguageTool benutzen sollte.",
        "1992 übernahm die damalige Ernst Klett Schulbuchverlag GmbH, Stuttgart, den reprivatisierten Verlag Haack Gotha",
        "Überlegst du dir einen ID.3 zu leasen?",
        "Der Deutsch Langhaar ist ein mittelgroßer Jagdhund",
        "Eine Lösung die Spaß macht",
        "Mir machte das Spaß.",
        "Wir möchten nicht, dass irgendjemand Fragen stellt.",
        "Die Multiple Sklerose hat 1000 Gesichter.",
        "Na ja, einige nennen das Freundschaft plus, aber das machen wir besser nicht.",
        "Vogue, eigentlich als B-Seite der letzten Like A Prayer-Auskopplung Keep It Together gedacht, wurde kurzfristig als eigenständige Single herausgebracht",
        "..., die laufend Gewaltsituationen ausgeliefert sind",
        "Dann folgte die Festnahme der dringend Tatverdächtigen.",
        "Von der ersten Spielminute an machten die Münsteraner Druck und ...",
        "Wenn diese Prognose bestätigt wird, wird empfohlen, dass Unternehmen die gefährliche Güter benötigen, die Transporte am Montag und Dienstag machen.",
        "Ich habe meine Projektidee (die riesiges finanzielles Potenzial hat) an einen Unternehmenspräsidenten geschickt.",
        "Als weitere Rechtsquelle gelten gelegentlich noch immer der Londoner Court of Appeal und das britische House of Lords.",
        "Die Evangelische Kirche befindet sich in der Bad Sodener Altstadt direkt neben dem Quellenpark.",
        "Der volle Windows 10 Treibersupport",
        "Zugleich stärkt es die renommierte Berliner Biodiversitätsforschung.",
        "Der Windows 10 Treibersupport",
        "Kennt irgendwer Tipps wie Kopfhörer länger halten?",
        "George Lucas 1999 über seine sechsteilige Star Wars Saga.",
        "… und von denen mehrere Gegenstand staatsanwaltlicher Ermittlungen waren.",
        "Natürlich ist das Quatsch!",
        "Die Xi Jinping Ära ist …",
        "Die letzte unter Windows 98 lauffähige Version ist 5.1.",
        "Das veranlasste Bürgermeister Adam, selbst tätig zu werden, denn er wollte es nicht zulassen, dass in seiner Stadt Notleidende ohne Hilfe dastehen.",
        "Die südlichste Düsseldorfer Rheinbrücke ist die Fleher Brücke, eine Schrägseilbrücke mit dem höchsten Brückenpylon in Deutschland und einer Vielzahl von fächerförmig angeordneten Seilen.",
        "Ein zeitweise wahres Stakkato an einschlägigen Patenten, das Benz & Cie.",
        "Wem Rugby nicht sehr geläufig ist, dem wird auch das Six Nations nicht viel sagen.",
        "Eine Boeing 767 der Air China stürzt beim Landeanflug in ein Waldgebiet.",
        "Wir sind immer offen für Mitarbeiter die Teil eines der traditionellsten Malerbetriebe auf dem Platz Zürich werden möchten.",
        "Gelingt das mit Erregern rechtzeitig, könnte das Infektionen sogar oft verhindern.",
        "In der aktuellen Niedrigzinsphase bedeutet das sehr geringe Zinsen, die aber deutlich ansteigen können.",
        "Es gibt viele Stock Screener.",
        "Wir gehen zur Learning Academy.",
        "Es ist ein stiller Bank Run.",
        "Whirlpool Badewanne der europäische Marke SPAtec Modell Infinity.",
        "1944 eroberte diese weite Teile von Südosteuropa.",
        "Auch die Monopolstellung des staatlichen All India Radio, das in 24 Sprachen sendet",
        "Das schwedischen Entwicklerstudio MachineGames hat uns vor drei Jahren mit Wolfenstein: The New Order positiv überrascht.",
        "In einem normalen Joint habe es etwa ein halbes Gramm Hanf.",
        "den leidenschaftlichen Lobpreis der texanischen Gateway Church aus",
        "die gegnerischen Shooting Guards",
        "Bald läppert sich das zu richtigem Geld zusammen.",
        "Die Weimarer Parks laden ja förmlich ein zu Fotos im öffentlichen Raum.",
        "Es is schwierig für mich, diese zu Sätzen zu verbinden.",
        "Es kam zum einen zu technischen Problemen, zum anderen wurde es unübersichtlich.",
        "Das Spiel wird durch den zu neuer Größe gewachsenen Torwart dominiert.",
        "Dort findet sich schlicht und einfach alles & das zu sagenhafter Hafenkulisse.",
        "Man darf gespannt sein, wen Müller für diese Aufgabe gewinnt.",
        "Das Vereinslokal in welchem Zusammenkünfte stattfinden.",
        "Er lässt niemanden zu Wort kommen.",
        "Es war eine alles in allem spannende Geschichte.",
        "Eine mehrere hundert Meter lange Startbahn.",
        "Wir müssen jetzt um ein vielfaches höhere Preise zahlen.",
        "Und eine von manchem geforderte Übergewinnsteuer.",
        "Sie hat niemandem wirkliches Leid zugefügt.",
        "Die Organe eines gerade Verstorbenen",
        "Da wusste keiner Bescheid bezüglich dieser Sache.",
        "Es braucht keiner Bescheid wissen.",
        "Das sind auch beides staatliche Organe.",
        "Ein Haus für die weniger Glücklichen.",
        "Wir können sowas Mittwoch machen.",
        "Den schlechter Verdienenden geht es schlecht.",
        "Mit der weit weniger bekannten Horrorkomödie begann ihre Karriere.",
        "Die Adelmanns wohnen in Herford.",
        "Die Idee des Werbekaufmanns kam gut an.",
        "Solch harte Worte!",
        "Ich habe es an unseren amerikanischen Commercial Lawyer geschickt.",
        "Dieser eine Schritt hat gedauert.",
        "Es besteht durchaus die Gefahr, dass die Telekom eine solch starke monopolistische Stellung auf dem Markt hat, dass sich kaum Wettbewerb entfalten kann.",
        "Wenn ein Tiger einen Menschen tötet, ist das Grausamkeit.",
        "Kombinieren Sie diese zu ganzen Bewegungsprogrammen",
        "Erst später wurde Kritik hauptsächlich an den Plänen zu einem Patriot Act II laut.",
        "Laut Charlie XCX selbst sind das Personen, die vielleicht eine ...",
        "Solch frivolen Gedanken wollen wir gar nicht erst nachgehen.",
        "Er erwartete solch aggressives Verhalten.",
        "Eine solch schöne Frau.",
        "Einer solch schönen Frau.",
        "Ein solch schöner Tisch.",
        "Ein solch schöner neuer Tisch.",
        "Eine solch begnadete Fotografin mit dabei zu haben und Tipps für die Fotosession zu bekommen, wäre schon toll.",
        "Wie können wir als globale Gemeinschaft solch brennende Themen wie Klimawandel und Rezession in entwickelten Märkten in Angriff nehmen?",
        "Wir waren überrascht, von ihm solch beißende Bemerkungen über seinen besten Freund zu hören.",
        "Worten, wohl gewählt, wohnt solch große Macht inne.",
        "Warum gelingt es den Stuten die Hengste in solch großen Rennen zu schlagen.",
        "Eine solch gute Beratung kann natürlich nicht kostenlos sein, daher lassen Sie mich bitte wissen welche Kosten für die PKV und BU-Beratung auf mich zukommen werden.",
        "Ein solch großer Ausbruch außerhalb des Nahen Ostens ist eine neue Entwicklung, heißt es weiter.",
        "Natürlich dürfen unsere Sporteinheiten nicht fehlen, vor allem nicht bei solch gutem Essen.",
        "Wie konnten Sie solch harte Songs gegen Ihre Familie schreiben?",
        "Zur Krönung auf dem schönsten Aussichtsberg in der Ferienregion Tirol West gelegen, bietet die Venet Gipfelhütte alle Annehmlichkeiten, die man sich auf solch hohem Niveau (2.212 m) wünschen kann.",
        "Vor allem nicht einen, der sich ein solch hohen Ballbesitz organisierte und die Berliner nicht zur Entfaltung kommen ließ.",
        "Du solltest im Winter keinen solch hohen Berg besteigen.",
        "Man darf an dieser Stelle fragen, wie ein solch hoher Verlust zustande kommt, wo doch der Stuttgarter Weg aus Sparen bestand.",
        "Kann der Patient eine solch lange Operation überstehen?",
        "Er kennt sich aus mit solch monumentalen Projekten.",
        "Eine solch schöne hübsche Frau.",
        "Bisher hat Gül einen solch offenen Affront gegen die ErdoganRegierung vermieden.",
        "Umso dankbarer bin ich für Brüder, die klare Kante in theologischer Hinsicht zeigen und nachvollziehbar die Bibel auch in solch schwierigen unpopulären Themen auslegen.",
        "Hier geht's zur Sonne.",
        "Hier geht's zum Schrank.",
        "Niereninsuffizienz führt zu Störungen des Wasserhaushalts.",
        "Das hat der fließend Englisch sprechende Mitarbeiter veranlasst.",
        "Zusammenschluss mehrerer dörflicher Siedlungen an einer Furt",
        "Für einige markante Szenen",
        "Für einige markante Szenen baute Hitchcock ein Schloss.",
        "Haben Sie viele glückliche Erfahrungen in Ihrer Kindheit gemacht?",
        "Es gibt viele gute Sachen auf der Welt.",
        "Viele englische Wörter haben lateinischen Ursprung",
        "Ein Bericht über Fruchtsaft, einige ähnliche Erzeugnisse und Fruchtnektar",
        "Der Typ, der seit einiger Zeit immer wieder hierher kommt.",
        "Jede Schnittmenge abzählbar vieler offener Mengen",
        "Es kam zur Fusion der genannten und noch einiger weiterer Unternehmen.",
        "Zu dieser Fragestellung gibt es viele unterschiedliche Meinungen.",
        "Wir zeigen die Gründe auf, wieso noch nicht jeder solche Anschlüsse hat.",
        "Die Übernahme der früher selbständigen Gesellschaft",
        "Das ist, weil man oft bei anderen schreckliches Essen vorgesetzt bekommt.",
        "Das ist der riesige Tisch.",
        "Der riesige Tisch ist groß.",
        "Die Kanten der der riesigen Tische.",
        "Den riesigen Tisch mag er.",
        "Es mag den riesigen Tisch.",
        "Die Kante des riesigen Tisches.",
        "Dem riesigen Tisch fehlt was.",
        "Die riesigen Tische sind groß.",
        "Der riesigen Tische wegen.",
        "An der roten Ampel.",
        "Dann hat das natürlich Nachteile.",
        "Ihre erste Nr. 1",
        "Wir bedanken uns bei allen Teams.",
        "Als Heinrich versuchte, seinen Kandidaten für den Mailänder Bischofssitz durchzusetzen, reagierte der Papst sofort.",
        "Den neuen Finanzierungsweg wollen sie daher Hand in Hand mit dem Leser gehen.",
        "Lieber den Spatz in der Hand...",
        "Wir wollen sein ein einzig Volk von Brüdern",
        "Eine Zeitreise durch die 68er Revolte",
        "Ich besitze ein Modell aus der 300er Reihe.",
        "Aber ansonsten ist das erste Sahne",
        "...damit diese ausreichend Sauerstoff geben.",
        "...als auch die jedem zukommende Freiheit.",
        "...als auch die daraus jedem zukommende Freiheit.",
        "Damit zeigen wir, wie bedeutungsreich manche deutsche Begriffe sein können.",
        "Damit zeigen wir, wie bedeutungsreich manche deutschen Begriffe sein können.",
        "2009 gab es im Rathaus daher Bestrebungen ein leichter handhabbares Logo einzuführen.",
        "Das ist eine leichter handhabbare Situation.",
        "Es gibt viele verschiedene Stock Screener.",
        "Die Ware umräumen, um einer anderen genügend Platz zu schaffen.",
        "Ich widerrufe den mit Ihnen geschlossenen Vertrag.",
        "Er klagte auch gegen den ohne ihn verkündeten Sachbeschluss.",
        "Dieser relativ gesehen starke Mann.",
        "Diese relativ gesehen starke Frau.",
        "Dieses relativ gesehen starke Auto.",
        "Es kann gut sein, dass bei sowas echte Probleme erkannt werden.",
        "Das verlangt reifliche Überlegung.",
        "Das bedeutet private Versicherungssummen ab 100€.",
        "Das erfordert einigen Mut.",
        "Die abnehmend aufwendige Gestaltung der Portale...",
        "Die strahlend roten Blumen.",
        "Der weiter vorhandene Widerstand konnte sich nicht durchsetzen.",
        "Das jetzige gemeinsame Ergebnis...",
        "Das früher übliche Abdecken mit elementarem Schwefel...",
        "Das einzig wirklich Schöne...",
        "Andere weniger bekannte Vorschläge waren „Konsistenter Empirismus“ oder...",
        "Werden mehrere solcher physikalischen Elemente zu einer Einheit zusammengesetzt...",
        "Aufgrund ihrer weniger guten Bonitätslage.",
        "Mit ihren teilweise eigenwilligen Außenformen...",
        "Die deutsche Kommasetzung bedarf einiger technischer Ausarbeitung.",
        "Die deutsche Kommasetzung bedarf einiger guter technischer Ausarbeitung.",
        "Wieso verstehst du nicht, dass das komplett verschiedene Dinge sind?",
        "Ich frage mich sehr, ob die wirklich zusätzliche Gebühren abdrücken wollen",
        "Das passiert nur, wenn der zu Pflegende bereit ist.",
        "Peter, iss nicht meine",
    ] {
        assert!(matches(text).is_empty(), "should be good: {text}: {:?}", matches(text));
    }
    // assertBad (one match; no suggestion check)
    for text in [
        "Denn die einzelnen sehen sich einer sehr verschieden starken Macht des...",
        "Es birgt für mich ein zu hohes juristische Risiko.",
        "Das betrifft auch eure Werkstudent:innen-Xihfrisfgds.",
        "Das betrifft auch eure Jurist:innenxyzdfsdf.",
        "Gutenberg, die Genie.",
        "Ein Buch mit einem ganz ähnlichem Titel.",
        "Meiner Chef raucht.",
        "Er hat eine 34-jährigen Sohn.",
        "Die Galerie zu den Bilder findet sich hier.",
        "Ganz im Gegensatz zu den Blätter des Brombeerstrauches.",
        "Die erwähnt Konferenz ist am Samstag.",
        "Die erwähntes Konferenz ist am Samstag.",
        "Die erwähnten Konferenz ist am Samstag.",
        "Die erwähnter Konferenz ist am Samstag.",
        "Die erwähntem Konferenz ist am Samstag.",
        "Die gemessen Werte werden in die länderspezifische Höhe über dem Meeresspiegel umgerechnet.",
        "Darüber hinaus haben wir das berechtigte Interessen, diese Daten zu verarbeiten.",
        "Eine Amnestie kann den Hingerichteten nicht das Leben und dem heimgesuchten Familien nicht das Glück zurückgeben.",
        "Z. B. therapeutisches Klonen, um aus den gewonnen Zellen in vitro Ersatzorgane für den Patienten zu erzeugen",
        "Die Partei, die bei den vorangegangen Wahlen noch seine Politik unterstützt hatte.",
        "Bei Zunahme der aufgelösten Mineralstoffen, bei denen...",
        "Durch die große Vielfalt der verschiedene Linien ist für jeden Anspruch die richtige Brille im Portfolio.",
        "In diesen Viertel bin ich aufgewachsen.",
        "Im November wurde auf den Gelände der Wettbewerb ausgetragen.",
        "Dort finden sie Testberichte und viele Informationen rund um das Themen Schwangerschaft, Geburt, Stillen, Babys und Kinder.",
        "Je länger zugewartet wird, desto schwieriger dürfte es werden, die Jungtiere von den Elterntiere zu unterscheiden.",
        "Er schrieb ein von 1237 bis 1358 reichendes Geschichtswerk, dessen Schwerpunkt auf den Ereignisse in der Lombardei liegt.",
        "Des großer Mannes.",
        "Er erzählte von den Leute und den Dingen, die er gesehen hatte.",
        "Diese Partnerschaft wurde 1989 nach den Massaker auf dem Platz des Himmlischen Friedens eingefroren.",
        "Das Dach meinem großen Autos.",
        "Das Dach mein großen Autos.",
        "Der Zustand meiner Gehirns.",
        "Lebensmittel sind da, um den menschliche Körper zu ernähren.",
        "Geld ist da, um den menschliche Überleben sicherzustellen.",
        "Sie hatte das kleinen Kaninchen.",
        "Frau Müller hat das wichtigen Dokument gefunden.",
        "Ich gebe dir ein kleine Kaninchen.",
        "Ich gebe dir ein kleinen Kaninchen.",
        "Ich gebe dir ein kleinem Kaninchen.",
        "Ich gebe dir ein kleiner Kaninchen.",
        "Ich gebe dir das kleinen Kaninchen.",
        "Ich gebe dir das kleinem Kaninchen.",
        "Ich gebe dir das kleiner Kaninchen.",
        "Hier steht Ihre Text.",
        "Hier steht ihre Text.",
        "Antje Last, Inhaberin des Berliner Kult Hotels Auberge, freute sich ebenfalls über die Gastronomenfamilie aus Bayern.",
        "Das ist doch lächerlich, was ist denn das für ein Klinik?",
        "Ich weiß nicht mehr, was unser langweiligen Thema war.",
        "Er ging ins Küche.",
        "Er ging ans Luft.",
        "Eine Niereninsuffizienz führt zur Störungen des Wasserhaushalts.",
        "Er stieg durchs Fensters.",
        "Ich habe heute ein Krankenwagen gesehen.",
        "Funktioniert das Software auch mit Windows?",
        "So soll er etwa Texte des linken Literaturwissenschaftler Helmut Lethen mit besonderem Interesse gelesen haben.",
        "Auf dieser Website werden allerdings keine solche Daten weiterverxxx.",
        "Bei größeren Gruppen und/oder mehrere Tagen gibts einen nennenswerten Nachlass.",
        "Die Idee des Werbekaufmann kam gut an.",
        "Ich habe keine Zeit für solche kleinlichen Belangen.",
        "Einen Dämonen wird er nicht aufhalten.",
        "Das versetzte den Kronprinz in Schrecken.",
        "Solche kleinen Anbietern nutzen dann eines der drei großen Mobilfunknetze Deutschlands.",
        "..., das heißt solche natürliche Personen, welche unsere Leistungen in Anspruch nehmen, ...",
        "Der Erwerb solcher kultureller Güter ist natürlich stark an das ökonomische Kapital gebunden.",
        "Wir haben das Abo beendet und des Betrag erstattet.",
        "Es sind die riesigen Tisch.",
        "Als die riesigen Tischs kamen.",
        "Als die riesigen Tisches kamen.",
        "Der riesigen Tisch und so.",
        "An der roter Ampel.",
        "An der rote Ampel.",
        "An der rotes Ampel.",
        "An der rotem Ampel.",
        "Er hatte eine sehr schweren Infektion.",
        "Ein fast 5 Meter hohem Haus.",
        "Ein fünf Meter hohem Haus.",
        "Es wurden Karavellen eingesetzt, da diese für die flachen Gewässern geeignet waren.",
        "Wir bedanken uns bei allem Teams.",
        "Dabei geht es um das altbekannte Frage der Dynamiken der Eigenbildung..",
        "Den neue Finanzierungsweg wollen sie daher Hand in Hand mit dem Leser gehen.",
        "Den neuen Finanzierungsweg wollen sie daher Hand in Hand mit dem Lesern gehen.",
        "Die deutsche Kommasetzung bedarf einiger technisches Ausarbeitung.",
        "Die deutsche Kommasetzung bedarf einiger guter technische Ausarbeitung.",
        "Die Höhe kommt oft darauf an, ob die richtigen Leuten gut mit einen können oder nicht.",
    ] {
        assert_eq!(matches(text).len(), 1, "should be bad: {text}: {:?}", matches(text));
    }
    // assertBad with exact expected suggestions
    let cases: &[(&str, Vec<&str>)] = &[
        ("Das ist die Original Mail", vec!["die Originalmail", "die Original-Mail"]),
        ("Das ist die neue Original Mail", vec!["die neue Originalmail", "die neue Original-Mail"]),
        ("Das ist die ganz neue Original Mail", vec!["die ganz neue Originalmail", "die ganz neue Original-Mail"]),
        ("Doch dieser kleine Magnesium Anteil ist entscheidend.", vec!["dieser kleine Magnesiumanteil", "dieser kleine Magnesium-Anteil"]),
        ("Doch dieser sehr kleine Magnesium Anteil ist entscheidend.", vec!["dieser sehr kleine Magnesiumanteil", "dieser sehr kleine Magnesium-Anteil"]),
        ("Die Standard Priorität ist 5.", vec!["Die Standardpriorität", "Die Standard-Priorität"]),
        ("Die derzeitige Standard Priorität ist 5.", vec!["Die derzeitige Standardpriorität", "Die derzeitige Standard-Priorität"]),
        ("Ein neuer LanguageTool Account", vec!["Ein neuer LanguageTool-Account"]),
        ("Danke für deine Account Daten", vec!["deine Accountdaten", "deine Account-Daten"]),
        ("Mit seinem Konkurrent Alistair Müller", vec!["seinem Konkurrenten"]),
        ("Wir gehen ins Fitness Studio", vec!["ins Fitnessstudio", "ins Fitness-Studio"]),
        ("Wir gehen durchs Fitness Studio", vec!["durchs Fitnessstudio", "durchs Fitness-Studio"]),
        ("Was für eine schöner Sonnenuntergang!", vec!["ein schöner Sonnenuntergang"]),
        ("Es ist ein sehr interessantes kostenloses Slot Spiel.", vec!["ein sehr interessantes kostenloses Slotspiel", "ein sehr interessantes kostenloses Slot-Spiel"]),
        ("Wahrlich ein äußerst kritische Jury.", vec!["eine äußerst kritische Jury"]),
        ("Das ist ein enorm großer Auto.", vec!["ein enorm großes Auto"]),
        ("Es sind die Tisch.", vec!["die Tische", "der Tisch", "den Tisch", "dem Tisch"]),
        ("Es sind das Tisch.", vec!["der Tisch", "den Tisch", "dem Tisch"]),
        ("Es sind die Haus.", vec!["das Haus", "dem Haus", "die Häuser"]),
        ("Es sind der Haus.", vec!["dem Haus", "das Haus", "der Häuser"]),
        ("Es sind das Frau.", vec!["die Frau", "der Frau"]),
        ("Das Auto des Mann.", vec!["der Mann", "den Mann", "dem Mann", "des Manns", "des Mannes"]),
        ("Das interessiert das Mann.", vec!["der Mann", "den Mann", "dem Mann"]),
        ("Das interessiert die Mann.", vec!["der Mann", "den Mann", "dem Mann", "die Männer"]),
        ("Das Auto ein Mannes.", vec!["ein Mann", "eines Mannes"]),
        ("Das Auto einem Mannes.", vec!["eines Mannes", "einem Mann"]),
        ("Das Auto einer Mannes.", vec!["eines Mannes"]),
        ("Das Auto einen Mannes.", vec!["eines Mannes", "einen Mann"]),
        ("Das Dach von meine Auto.", vec!["meinem Auto"]),
        ("Das Dach von meinen Auto.", vec!["meinem Auto", "meinen Autos"]),
        ("Das Dach mein Autos.", vec!["mein Auto", "meine Autos", "meines Autos", "meinen Autos", "meiner Autos"]),
        ("Das Dach meinem Autos.", vec!["meinem Auto", "meines Autos", "meine Autos", "meinen Autos", "meiner Autos"]),
        ("Das Klientel der Partei.", vec!["Die Klientel", "Der Klientel"]),
        ("Der Haus ist groß", vec!["Dem Haus", "Das Haus", "Der Häuser"]),
        ("Aber der Haus ist groß", vec!["dem Haus", "das Haus", "der Häuser"]),
        ("Ich habe einen Feder gefunden.", vec!["eine Feder", "einer Feder"]),
        ("Geprägt ist der Platz durch einen 142 Meter hoher Obelisken", vec!["einen 142 Meter hohen Obelisken"]),
        ("Es birgt für mich ein überraschend hohes juristische Risiko.", vec!["ein überraschend hohes juristisches Risiko"]),
        ("Es birgt für mich ein zu hohes juristische Risiko.", vec!["ein zu hohes juristisches Risiko"]),
        ("Mit der weit weniger bekannte Horrorkomödie begann ihre Karriere.", vec!["der weit weniger bekannten Horrorkomödie"]),
        ("Hier geht's zur Schrank.", vec!["zum Schrank"]),
        ("Hier geht's zur Schränken.", vec!["zum Schränken", "zu Schränken"]),
        ("Hier geht's zur Männern.", vec!["zu Männern"]),
        ("Hier geht's zur Portal.", vec!["zum Portal"]),
        ("Hier geht's zur Portalen.", vec!["zu Portalen"]),
        ("Sie gehen zur Frauen.", vec!["zu Frauen", "zur Frau"]),
        ("Niereninsuffizienz führt zur Störungen des Wasserhaushalts.", vec!["zu Störungen", "zur Störung"]),
        ("Das Motiv wird in der Klassik auch zur Darstellungen übernommen.", vec!["zu Darstellungen", "zur Darstellung"]),
        ("Er hatte ein anstrengenden Tag", vec!["ein anstrengender Tag", "ein anstrengendes Tag", "einen anstrengenden Tag", "einem anstrengenden Tag"]),
        ("Er hatte ihn aus dem 1,4 Meter tiefem Wasser gezogen.", vec!["dem 1,4 Meter tiefen Wasser"]),
        ("Das ist ein sehr schönes Tisch.", vec!["ein sehr schöner Tisch"]),
        ("Ich widerrufe den mit Ihnen geschlossene Vertrag.", vec!["der mit Ihnen geschlossene Vertrag", "den mit Ihnen geschlossenen Vertrag"]),
        ("Er klagte auch gegen den ohne ihn verkündete Sachbeschluss.", vec!["den ohne ihn verkündeten Sachbeschluss"]),
        ("Das ist eine solides strategisches Fundament", vec!["ein solides strategisches Fundament"]),
        ("Das ist eine solide strategisches Fundament", vec!["ein solides strategisches Fundament"]),
        ("Das ist ein solide strategisches Fundament", vec!["ein solides strategisches Fundament"]),
        ("Das ist ein solides strategische Fundament", vec!["ein solides strategisches Fundament"]),
        ("Das ist ein solides strategisches Fundamente", vec!["ein solides strategisches Fundament"]),
        ("Das ist ein solides strategisches Fundaments", vec!["ein solides strategisches Fundament"]),
        ("Dies wurde durchgeführt um das moderne Charakter zu betonen.", vec!["den modernen Charakter"]),
        ("Nur bei Topfpflanzung ist eine regelmäßige Düngung wichtig, da die normalen Bodenbildungsprozessen nicht stattfinden.", vec!["die normalen Bodenbildungsprozesse", "den normalen Bodenbildungsprozessen"]),
        ("Denn die einzelnen sehen sich einer sehr verschieden starken Macht des...", vec!["einer sehr verschiedenen starken Macht"]),
    ];
    for (text, expected) in cases.iter() {
        let m = matches(text);
        assert_eq!(m.len(), 1, "should be bad: {text}: {m:?}");
        assert_eq!(&m[0].2, expected, "suggestions for {text}");
    }
    // assertBad1 (suggestion among the first 4)
    let cases: &[(&str, &str)] = &[(
        "Das ist eine solide strategische Fundament",
        "ein solides strategisches Fundament",
    )];
    for (text, expected) in cases.iter() {
        let m = matches(text);
        assert_eq!(m.len(), 1, "should be bad: {text}: {m:?}");
        assert!(
            m[0].2.iter().take(4).any(|s| s == expected),
            "expected {expected:?} in {text}: {m:?}"
        );
    }
}

#[test]
fn agreement2_rule_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_AGREEMENT2"]) else {
        return;
    };
    let rule = "DE_AGREEMENT2";
    let matches = |text: &str| -> Vec<(usize, usize, Vec<String>)> {
        engine
            .check(text)
            .unwrap()
            .matches
            .iter()
            .filter(|m| m.rule_id == rule)
            .map(|m| {
                (
                    m.range.start,
                    m.range.end,
                    m.suggestions
                        .iter()
                        .map(|s| s.value.clone())
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    };
    // assertGood
    for text in [
        "Kleines Haus am Waldesrand",
        "\"Kleines Haus am Waldesrand\"",
        "Wirtschaftliches Wachstum kommt ins Stocken",
        "Unter Berücksichtigung des Übergangs",
        "Wirklich Frieden herrscht aber noch nicht",
        "Deutscher Taschenbuch Verlag expandiert",
        "Wohl Anfang 1725 begegnete Bach dem Dichter.",
        "Weniger Personal wird im ganzen Land gebraucht.",
        "National Board of Review",
        "International Management",
        "Gemeinsam Sportler anfeuern.",
        "Viel Spaß beim Arbeiten",
        "Ganz Europa stand vor einer Neuordnung.",
        "Gesetzlich Versicherte sind davon ausgenommen.",
        "Ausreichend Bananen essen.",
        "Nachhaltig Yoga praktizieren",
        "Überraschend Besuch bekommt er dann von ihr.",
        "Ruhig Schlafen & Zentral Wohnen",
        "Voller Mitleid",
        "Voll Mitleid",
        "Einzig Fernschüsse brachten Erfolgsaussichten.",
        "Gelangweilt Dinge sortieren hilft als Ablenkung.",
        "Ganzjährig Garten pflegen",
        "Herzlich Willkommen bei unseren günstigen Rezepten!",
        "10-tägiges Rückgaberecht",
        "Angeblich Schüsse vor Explosionen gefallen",
        "Dickes Danke auch an Elena",
        "Dickes Dankeschön auch an Elena",
        "Echt Scheiße",
        "Entsprechende Automaten werden heute nicht mehr gebaut",
        "Existenziell Bedrohte kriegen einen Taschenrechner",
        "Flächendeckend Tempo 30",
        "Frei Klavier spielen lernen",
        "Ganz Eilige können es schaffen",
        "Gering Gebildete laufen Gefahr ...",
        "Ganz Ohr ist man hier",
        "Gleichzeitig Muskeln aufbauen und Fett verlieren",
        "Klar Schiff, Erster Offizier!",
        "Kostenlos Bewegung schnuppern",
        "Prinzipiell Anrecht auf eine Vertretung",
        "Regelrecht Modell gestanden haben Michel",
        "Weitgehend Konsens, auch über ...",
        "Alarmierte Polizeibeamte nahmen den Mann fest.",
        "Anderen Brot und Arbeit ermöglichen - das ist ihr Ziel",
        "Diverse Unwesen, mit denen sich Hellboy beschäftigen muss, ...",
        "Gut Qualifizierte bekommen Angebote",
        "Liebe Mai, wie geht es dir?",
        "Willkommen Simpsons-Fan!",
        "Kleinem Haus am Waldesrand ...",
        "Junger Frau geht das Geld aus",
        "Junge Frau gewinnt im Lotto",
    ] {
        assert!(
            matches(text).is_empty(),
            "should be good: {text}: {:?}",
            matches(text)
        );
    }
    // assertBad (one match; no suggestion check)
    for text in [
        "Kleiner Haus am Waldesrand",
        "\"Kleiner Haus am Waldesrand\"",
        "Wirtschaftlich Wachstum kommt ins Stocken",
        "Deutscher Taschenbuch",
    ] {
        assert_eq!(
            matches(text).len(),
            1,
            "should be bad: {text}: {:?}",
            matches(text)
        );
    }
    // assertBad with exact expected suggestions
    let cases: &[(&str, Vec<&str>)] = &[
        ("Kleiner Haus am Waldesrand", vec!["Kleines Haus"]),
        ("Kleines Häuser am Waldesrand", vec!["Kleine Häuser"]),
        ("Kleinem Häuser am Waldesrand", vec!["Kleine Häuser"]),
        ("Kleines Tisch reicht auch", vec!["Kleiner Tisch"]),
        ("Junges Frau gewinnt im Lotto", vec!["Junge Frau"]),
        ("Jungem Frau gewinnt im Lotto", vec!["Junge Frau"]),
        ("Jung Frau gewinnt im Lotto", vec!["Junge Frau"]),
        (
            "Wirtschaftlich Wachstum kommt ins Stocken",
            vec!["Wirtschaftliches Wachstum"],
        ),
        (
            "Wirtschaftlicher Wachstum kommt ins Stocken",
            vec!["Wirtschaftliches Wachstum"],
        ),
    ];
    for (text, expected) in cases.iter() {
        let m = matches(text);
        assert_eq!(m.len(), 1, "should be bad: {text}: {m:?}");
        assert_eq!(&m[0].2, expected, "suggestions for {text}");
    }
    // assertBad1 (suggestion among the first 4)
    let cases: &[(&str, &str)] = &[];
    for (text, expected) in cases.iter() {
        let m = matches(text);
        assert_eq!(m.len(), 1, "should be bad: {text}: {m:?}");
        assert!(
            m[0].2.iter().take(4).any(|s| s == expected),
            "expected {expected:?} in {text}: {m:?}"
        );
    }
}

#[test]
fn case_rule_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_CASE"]) else {
        return;
    };
    let rule = "DE_CASE";
    let matches = |text: &str| -> Vec<(usize, usize, Vec<String>)> {
        engine
            .check(text)
            .unwrap()
            .matches
            .iter()
            .filter(|m| m.rule_id == rule)
            .map(|m| {
                (
                    m.range.start,
                    m.range.end,
                    m.suggestions
                        .iter()
                        .map(|s| s.value.clone())
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    };
    // assertGood
    for text in [
        "(Dauer, Raum, Anwesende)",
        "Es gibt wenige Befragte.",
        "Es gibt weniger Befragte, die das machen würden.",
        "Es gibt mehr Befragte, die das machen würden.",
        "Das ist eine Abkehr von Gottes Geboten.",
        "Dem Hund Futter geben",
        "Heute spricht Frau Stieg.",
        "So könnte es auch den Handwerksbetrieben gehen, die ausbilden und deren Ausbildung dann Industriebetrieben zugutekäme.",
        "Die Firma Drosch hat nicht pünktlich geliefert.",
        "3.1 Technische Dokumentation",
        "Ein einfacher Satz zum Testen.",
        "Das Laufen fällt mir leicht.",
        "Das Winseln stört.",
        "Das schlägt nicht so zu Buche.",
        "Dirk Hetzel ist ein Name.",
        "Aber sie tat es, sodass unsere Klasse das sehen und fotografieren konnte.",
        "Sein Verhalten war okay.",
        "Hier ein Satz. \"Ein Zitat.\"",
        "Hier ein Satz. 'Ein Zitat.'",
        "Hier ein Satz. «Ein Zitat.»",
        "Hier ein Satz. »Ein Zitat.«",
        "Hier ein Satz. (Noch einer.)",
        "Hier geht es nach Tel Aviv.",
        "Unser Jüngster ist da.",
        "Alles Erfundene ist wahr.",
        "Sie hat immer ihr Bestes getan.",
        "Er wird etwas Verrücktes träumen.",
        "Er wird etwas schön Verrücktes träumen.",
        "Er wird etwas ganz schön Verrücktes träumen.",
        "Mit aufgewühltem Innerem.",
        "Mit völlig aufgewühltem Innerem.",
        "Er wird etwas so Verrücktes träumen.",
        "Tom ist etwas über dreißig.",
        "Diese Angriffe bleiben im Verborgenen.",
        "Ihr sollt mich das wissen lassen.",
        "Wenn er mich das rechtzeitig wissen lässt, gerne.",
        "Und sein völlig aufgewühltes Inneres erzählte von den Geschehnissen.",
        "Aber sein aufgewühltes Inneres erzählte von den Geschehnissen.",
        "Sein aufgewühltes Inneres erzählte von den Geschehnissen.",
        "Aber sein Inneres erzählte von den Geschehnissen.",
        "Ein Kaninchen, das zaubern kann.",
        "Keine Ahnung, wie ich das prüfen sollte.",
        "Und dann noch Strafrechtsdogmatikerinnen.",
        "Er kann ihr das bieten, was sie verdient.",
        "Das fragen sich mittlerweile viele.",
        "Ich habe gehofft, dass du das sagen würdest.",
        "Eigentlich hätte ich das wissen müssen.",
        "Mir tut es wirklich leid, Ihnen das sagen zu müssen.",
        "Der Wettkampf endete im Unentschieden.",
        "Er versuchte, Neues zu tun.",
        "Du musst das wissen, damit du die Prüfung bestehst",
        "Er kann ihr das bieten, was sie verdient.",
        "Er fragte, ob das gelingen wird.",
        "Er mag Obst, wie zum Beispel Apfelsinen.",
        "Er will die Ausgaben für Umweltschutz und Soziales kürzen.",
        "Die Musicalverfilmung „Die Schöne und das Biest“ bricht mehrere Rekorde.",
        "Joachim Sauer lobte Johannes Rau.",
        "Im Falle des Menschen ist dessen wirkendes Wollen gegeben.",
        "Szenario: 1) Zwei Galaxien verschmelzen.",
        "Existieren Außerirdische im Universum?",
        "Tom vollbringt Außerordentliches.",
        "Er führt Böses im Schilde.",
        "Es gab Überlebende.",
        "'Wir werden das stoppen.'",
        "Wahre Liebe muss das aushalten.",
        "Du kannst das machen.",
        "Vor dem Aus stehen.",
        "Ich Armer!",
        "Hallo Malte,",
        "Parks Vertraute Choi Soon Sil ist zu drei Jahren Haft verurteilt worden.",
        "Bei einer Veranstaltung Rechtsextremer passierte es.",
        "Eine Gruppe Betrunkener singt.",
        "Bei Betreten des Hauses.",
        "Das Aus für Italien ist bitter.",
        "Das Aus kam unerwartet.",
        "Anmeldung bis Fr. 1.12.",
        "Gibt es die Schuhe auch in Gr. 43?",
        "Weil er Unmündige sexuell missbraucht haben soll, wurde ein Lehrer verhaftet.",
        "Tausende Gläubige kamen.",
        "Es kamen Tausende Gläubige.",
        "Das schließen Forscher aus den gefundenen Spuren.",
        "Wieder Verletzter bei Unfall",
        "Eine Gruppe Aufständischer verwüstete die Bar.",
        "‚Dieser Satz.‘ Hier kommt der nächste Satz.",
        "Dabei werden im Wesentlichen zwei Prinzipien verwendet:",
        "Er fragte, ob das gelingen oder scheitern wird.",
        "Einen Tag nach Bekanntwerden des Skandals",
        "Das machen eher die Erwachsenen.",
        "Das ist ihr Zuhause.",
        "Das ist Sandras Zuhause.",
        "Das machen eher wohlhabende Leute.",
        "Als Erstes würde ich sofort die Struktur ändern.",
        "Er sagte: Als Erstes würde ich sofort die Struktur ändern.",
        "Das schaffen moderne E-Autos locker.",
        "Das schaffen moderne E-Autos schneller",
        "Das schaffen moderne und effizientere E-Autos schneller.",
        "Das verwalten User.",
        "Man kann das generalisieren",
        "Aber wie wir das machen und sicher gestalten, darauf konzentriert sich unsere Arbeit.",
        "Vielleicht kann man das erweitern",
        "Vielleicht soll er das generalisieren",
        "Wahrscheinlich müssten sie das überarbeiten",
        "Assistenzsysteme warnen rechtzeitig vor Gefahren.",
        "Jeremy Schulte rannte um sein Leben.",
        "Er arbeitet im Bereich Präsidiales.",
        "Er spricht Sunnitisch & Schiitisch.",
        "Er sagte, Geradliniges und Krummliniges sei unvergleichbar.",
        "Dort erfahren sie Kurioses und Erstaunliches zum Zusammenspiel von Mensch und Natur.",
        "Dabei unterscheidet die Shareware zwischen Privatem und Dienstlichem bei Fahrten ebenso wie bei Autos.",
        "Besucher erwartet Handegefertigtes, Leckeres und Informatives rund um den Hund.",
        "Der Unterschied zwischen Vorstellbarem und Machbarem war niemals geringer.",
        "Das war Fiete Lang.",
        "Wenn du an das glaubst, was du tust, kannst du Großes erreichen.",
        "Dann hat er Großes erreicht.",
        "Dann hat er Großes geleistet.",
        "Das Thema Datenaustauschverfahren ist mir wichtig.",
        "Ist das eine Frage ? Müsste das nicht anders sein?",
        "Das ist ein Satz !!! Das auch.",
        "Der russische Erdölmagnat Emanuel Nobel, der Erbauer des ersten Dieselmotorschiffes.",
        "Zur Versöhnung: Jüdische Gläubige sollen beten.",
        "Fast im Stundentakt wurden neue Infizierte gemeldet.",
        "Bert Van Den Brink",
        "“In den meisten Bundesländern werden solche Studien per se nicht durchgeführt.”",
        "Aber “in den meisten Bundesländern werden solche Studien per se nicht durchgeführt.”",
        "A) Das Haus",
        "Rabi und Polykarp Kusch an der Columbia-Universität",
        "Man geht davon aus, dass es sich dabei nicht um Reinigungsverhalten handelt.",
        "Wenn dort oft Gefahren lauern.",
        "3b) Den Bereich absichern",
        "@booba Da der Holger keine Zeit hat ...",
        "Es gibt infizierte Ärzt*innen.",
        "WUrzeln",
        "🙂 Übrigens finde ich dein neues Ordnungssystem richtig genial!",
        "Ein 10,4 Ah Lithium-Akku",
        "14:15 Uhr SpVgg Westheim",
        "Unser Wärmestrom-Tarif WärmeKompakt im Detail",
        "Autohaus Dornig GmbH",
        "Hans Pries GmbH",
        "Der Kund*innenservice war auch sehr kulant und persönlich.",
        ":D Auf dieses Frl.",
        "@b_fischer Der Bonussemester-Antrag oder der Widerspruch?",
        "Das Gedicht “Der Panther”.",
        "Klar, dass wir das brauchen.",
        "Das wird Scholz' engster Vertrauter Wolfgang Schmidt übernehmen.",
        "Bei der Fülle an Vorgaben kann das schnell vergessen werden.",
        "Majid ergänzte: ”Vorläufigen Analysen der Terrakottaröhren aus Ardais liegen ...",
        "Ist das eine Frage ? Müsste das nicht anders sein?",
        "Das ist ein Satz !!! Das auch.",
        "Liebe Kund:in",
        "Wir sollten das mal labeln.",
        "Teil 1: Der unaufhaltsame Aufstieg Bonapartes",
        "Der Absatz bestimmt, in welchem Maße diese Daten Dritten zugänglich gemacht werden.",
        "Der TN spricht Russisch - Muttersprache",
        "Ich musste das Video mehrmals stoppen, um mir über das Gesagte Gedanken zu machen.",
        "Während Besagtes Probleme verursachte.",
        "Während der Befragte Geschichten erzählte.",
        "Während ein Befragter Geschichten erzählte.",
        "... für welche ein Befragter Geld ausgegeben hat.",
        "Während die Befragte Geld verdiente.",
        "Während die Besagte Geschichten erzählte.",
        "Sind dem Zahlungspflichtigen Kosten entstanden?",
        "Jetzt, wo Protestierende und Politiker sich streiten",
        "Während die Besagte Geld verdiente.",
        "Die Nacht, die Liebe, dazu der Wein — zu nichts Gutem Ratgeber sein.",
        "Warum tun die Menschen Böses?",
        "Und das Vergangene Revue passieren lassen",
        "Seither ist das Französische Amtssprache in Frankreich.",
        "Für die Betreute Kontoauszüge holen.",
        "Das verstehen Deutsche halt nicht.",
        "12:00 - 13:00 Gemeinsames Mittagessen",
        "12:00 Gemeinsames Mittagessen",
        "Meld dich, wenn du Großes vorhast.",
        "Muss nicht der Einzelne Einschränkungen der Freiheit hinnehmen, wenn die Sicherheit der Menschen und des Staates mehr gefährdet sind?",
        "Wie reißt ein Einzelner Millionen aus ihren Sitzen?",
        "Der Aphorismus will nicht Dumme gescheit, sondern Gescheite nachdenklich machen.",
        "Während des Hochwassers den Eingeschlossenen Wasser und Nahrung bringen",
        "Aus dem Stein der Weisen macht ein Dummer Schotter.",
        "Auf dem Weg zu ihnen begegnet der Halbwüchsige Revolverhelden und Indianern.",
        "▶︎ Dies ist ein Test",
        "▶ Dies ist ein Test",
        "* Dies ist ein Test",
        "- Dies ist ein Test",
        "• Dies ist ein Test",
        ":-) Dies ist ein Test",
        ";-) Dies ist ein Test",
        ":) Dies ist ein Test",
        ";) Dies ist ein Test",
        "..., die ins Nichts griff.",
        "Er fragte, was sie über das denken und zwinkerte ihnen zu.",
        "dem Ägyptischen, Berberischen, Semitischen, Kuschitischen, Omotischen und dem Tschadischen",
        "mit S-Bahn-ähnlichen Verkehrsmitteln",
        "mit U-Bahn-ähnlichen und günstigen Verkehrsmitteln",
        "mit Ü-Ei-großen, schweren Hagelkörnern",
        "mit E-Musik-artigen, komplizierten Harmonien",
        "eBay International AG",
        "Harald & Schön",
        "Nicholas and Stark",
        "Die Schweizerische Bewachungsgesellschaft",
        "Stets suchte er das Extreme.",
        "Ich möchte zwei Kilo Zwiebeln.",
        "Ein Menschenfreund.",
        "Der Nachfahre.",
        "Hier ein Satz, \"Ein Zitat.\"",
        "Hier ein Satz, \"ein Zitat.\"",
        "Schon Le Monde schrieb das.",
        "In Blubberdorf macht man das so.",
        "Der Thriller spielt zur Zeit des Zweiten Weltkriegs",
        "Anders als physikalische Konstanten werden mathematische Konstanten unabhängig von jedem physikalischen Maß definiert.",
        "Eine besonders einfache Klasse bilden die polylogarithmischen Konstanten.",
        "Das südlich von Berlin gelegene Dörfchen.",
        "Weil er das kommen sah, traf er Vorkehrungen.",
        "Sie werden im Allgemeinen gefasst.",
        "Sie werden im allgemeinen Fall gefasst.",
        "Das sind Euroscheine.",
        "John Stallman isst.",
        "Das ist die neue Gesellschafterin hier.",
        "Das ist die neue Dienerin hier.",
        "Das ist die neue Geigerin hier.",
        "Die ersten Gespanne erreichen Köln.",
        "Er beschrieb den Angeklagten wie einen Schuldigen",
        "Er beschrieb den Angeklagten wie einen Schuldigen.",
        "Es dauerte bis ins neunzehnte Jahrhundert",
        "Das ist das Dümmste, was ich je gesagt habe.",
        "Wacht auf, Verdammte dieser Welt!",
        "Er sagt, dass Geistliche davon betroffen sind.",
        "Man sagt, Liebe mache blind.",
        "Die Deutschen sind sehr listig.",
        "Der Lesestoff bestimmt die Leseweise.",
        "Ich habe nicht viel von einem Reisenden.",
        "Die Vereinigten Staaten",
        "Der Satz vom ausgeschlossenen Dritten.",
        "Die Ausgewählten werden gut betreut.",
        "Die ausgewählten Leute werden gut betreut.",
        "Die Schlinge zieht sich zu.",
        "Die Schlingen ziehen sich zu.",
        "Sie fällt auf durch ihre hilfsbereite Art. Zudem zeigt sie soziale Kompetenz.",
        "Die Lieferadresse ist Obere Brandstr. 4-7",
        "Das ist es: kein Satz.",
        "Werner Dahlheim: Die Antike.",
        "1993: Der talentierte Mr. Ripley",
        "Ian Kershaw: Der Hitler-Mythos: Führerkult und Volksmeinung.",
        "Ich frage mich: Warum?",
        "Ich frage mich: Wieso?",
        "Ich frage mich: Weshalb?",
        "Ich frage mich: Und warum?",
        "Ich frage mich: Oder wieso?",
        "Ich frage mich: Aber warum?",
        "Das wirklich Wichtige ist dies:",
        "Das wirklich wichtige Verfahren ist dies:",
        "Im Norwegischen klingt das schöner.",
        "Übersetzt aus dem Norwegischen von Ingenieur Frederik Dingsbums.",
        "Dem norwegischen Ingenieur gelingt das gut.",
        "Peter Peterson, dessen Namen auf Griechisch Stein bedeutet.",
        "Peter Peterson, dessen Namen auf Griechisch gut klingt.",
        "Das dabei Erlernte und Erlebte ist sehr nützlich.",
        "Ein Kapitän verlässt als Letzter das sinkende Schiff.",
        "Es hilft, die Harmonie zwischen Führer und Geführten zu stützen.",
        "Das Gebäude des Auswärtigen Amts.",
        "Das Gebäude des Auswärtigen Amtes.",
        "   Im Folgenden beschreibe ich das Haus.",
        "\"Im Folgenden beschreibe ich das Haus.\"",
        "Gestern habe ich 10 Spieße gegessen.",
        "Die Verurteilten wurden mit dem Fallbeil enthauptet.",
        "Den Begnadigten kam ihre Reue zugute.",
        "Die Zahl Vier ist gerade.",
        "Ich glaube, dass das geschehen wird.",
        "Ich glaube, dass das geschehen könnte.",
        "Ich glaube, dass mir das gefallen wird.",
        "Ich glaube, dass mir das gefallen könnte.",
        "Alldem wohnte etwas faszinierend Rätselhaftes inne.",
        "Schau mich an, Kleine!",
        "Schau mich an, Süßer!",
        "Weißt du, in welchem Jahr das geschehen ist?",
        "Das wissen viele nicht.",
        "Die zum Tode Verurteilten wurden in den Hof geführt.",
        "Wenn Sie das schaffen, retten Sie mein Leben!",
        "Etwas Grünes, Schleimiges klebte an dem Stein.",
        "Er befürchtet Schlimmeres.",
        "#4 Aktuelle Situation",
        "Er trinkt ein kühles Blondes.",
        "* [ ] Ein GitHub Markdown Listenpunkt",
        "Tom ist ein engagierter, gutaussehender Vierzigjähriger, der...",
        "a.) Im Zusammenhang mit ...",
        "✔︎ Weckt Aufmerksamkeit.",
        "Hallo Eckhart,",
        "Er kann Polnisch und Urdu.",
        "---> Der USB 3.0 Stecker",
        "Black Lives Matter",
        "== Schrittweise Erklärung",
        "Audi A5 Sportback 2.0 TDI",
        "§ 1 Allgemeine Bedingungen",
        "§1 Allgemeine Bedingungen",
        "[H3] Was ist Daytrading?",
        " Das ist das Aus des Airbus A380.",
        "Wir sollten ihr irgendwas Erotisches schenken.",
        "Er trank ein paar Halbe.",
        "Sie/Er hat Schuld.",
        "Das war irgendein Irrer.",
        "Wir wagen Neues.",
        "Grundsätzlich gilt aber: Essen Sie, was die Einheimischen Essen.",
        "Vielleicht reden wir später mit ein paar Einheimischen.",
        "Das denken zwar viele, ist aber total falsch.",
        "Ich habe nix Besseres gefunden.",
        "Ich habe nichts Besseres gefunden.",
        "Ich habe noch Dringendes mitzuteilen.",
        "Er isst UV-bestrahltes Obst.",
        "Er isst Na-haltiges Obst.",
        "Er vertraut auf CO2-arme Wasserkraft",
        "Das Entweder-oder ist kein Problem.",
        "Er liebt ihre Makeup-freie Haut.",
        "Das ist eine Schreibweise.",
        "Das ist ein Mann.",
        "Du Ärmste!",
        "Ich habe nur Schlechtes über den Laden gehört.",
        "Du Ärmster, leg dich besser ins Bett.",
        "Er wohnt Am Hohen Hain 6a",
        "Das Bauvorhaben Am Wiesenhang 9",
        "... und das Zwischenmenschliche Hand in Hand.",
        "Der Platz auf dem die Ahnungslosen Kopf an Kopf stehen.",
        "4.)   Bei Beschäftigung von Hilfskräften: Schadenfälle durch Hilfskräfte",
        "Es besteht aus Schülern, Arbeitstätigen und Studenten.",
        "Sie starrt ständig ins Nichts.",
        "Sowas aber auch.\\u2063Das Haus ist schön.",
        "\\u2063Das Haus ist schön.",
        "\\u2063\\u2063Das Haus ist schön.",
        "Die Mannschaft ist eine gelungene Mischung aus alten Haudegen und jungen Wilden.",
        "Alleine durch die bloße Einwohnerzahl des Landes leben im Land zahlreiche Kulturschaffende, nach einer Schätzung etwa 30.000 Künstler.",
        "Ich hatte das offenbar vergessen oder nicht ganz verstanden.",
        "Ich hatte das vergessen oder nicht ganz verstanden.",
        "Das ist ein zwingendes Muss.",
        "Er hält eine Handbreit Abstand.",
        "Das ist das Debakel und Aus für Podolski.",
        "Ein Highlight für Klein und Groß!",
        "Der schwedische Psychologe Dan Katz, Autor von 'Angst kocht auch nur mit Wasser', sieht in der Corona-Krise dennoch nicht nur Negatives.",
        "Das fahrende Auto.",
        "Das können wir so machen.",
        "Denn das Fahren ist einfach.",
        "Das Fahren ist einfach.",
        "Das Gehen fällt mir leicht.",
        "Das Ernten der Kartoffeln ist mühsam.",
        "Entschuldige das späte Weiterleiten.",
        "Ich liebe das Lesen.",
        "Das Betreten des Rasens ist verboten.",
        "Das haben wir aus eigenem Antrieb getan.",
        "Das haben wir.",
        "Das haben wir schon.",
        "Das lesen sie doch sicher in einer Minute durch.",
        "Das lesen Sie doch sicher in einer Minute durch!",
        "Formationswasser, das oxidiert war.",
        "Um das herauszubekommen diskutieren zwei Experten.",
        "Ich würde ihn dann mal nach München schicken, damit die beiden das planen/entwickeln können.",
        "Das Lesen fällt mir schwer.",
        "Sie hörten ein starkes Klopfen.",
        "Wer erledigt das Fensterputzen?",
        "Viele waren am Zustandekommen des Vertrages beteiligt.",
        "Die Sache kam ins Stocken.",
        "Das ist zum Lachen.",
        "Euer Fernbleiben fiel uns auf.",
        "Uns half nur noch lautes Rufen.",
        "Die Mitbewohner begnügten sich mit Wegsehen und Schweigen.",
        "Sie wollte auf Biegen und Brechen gewinnen.",
        "Er klopfte mit Zittern und Zagen an.",
        "Ich nehme die Tabletten auf Anraten meiner Ärztin.",
        "Sie hat ihr Soll erfüllt.",
        "Dies ist ein absolutes Muss.",
        "Das Lesen fällt mir schwer.",
        "Das gilt ohne Wenn und Aber.",
        "Ohne Wenn und Aber",
        "Das gilt ohne Wenn und Aber bla blubb.",
        "Das gilt ohne wenn",
        "Das gilt ohne wenn und",
        "wenn und aber",
        "und aber",
        "aber",
    ] {
        assert!(matches(text).is_empty(), "should be good: {text}: {:?}", matches(text));
    }
    // assertBad
    for text in [
        "Während des Hochwassers den Eingeschlossenen Menschen Nahrung bringen",
        "Während Gefragte Menschen antworteten.",
        "Ich brauche eine Gratis App die Ohne WLAN.",
        "Alle Kommunikationsmedien die Meinem Widersacher dienen werden.",
        "Ich wünsche dir Alles Liebe.",
        "Das Auto Meines Vaters wird in Italien produziert.",
        "Nach Böhm-Bawerk steht die Allgemeine Profitrate und die Theorie der Produktionspreise im Widerspruch zum Wertgesetz des ersten Bandes.",
        "Ich sehe da keine Absolute Schranke.",
        "Manns und Fontanes Gesammelten Werken.",
        "Und das Neue Haus.",
        "Das sind die Die Lehrer.",
        "An der flachen Decke zeigt ein Großes Bildnis die Geburt Christi und die ewige Anbetung der Hirten.",
        "Und das Gesagte Wort.",
        "Und die Gesagten Wörter.",
        "Und meine Erzählte Geschichte.",
        "Und diese Erzählten Geschichten.",
        "Und eine Neue Zeit.",
        "Das machen der Töne ist schwierig.",
        "Sie Vertraute niemandem.",
        "Beten Lernt man in Nöten.",
        "Ich habe Heute keine Zeit.",
        "Er sagte, Geradliniges und krummliniges sei unvergleichbar.",
        "Er sagte, ein Geradliniges und Krummliniges Konzept ist nicht tragbar.",
        "Ä Was?",
        "… die preiswerte Variante unserer Topseller im Bereich Alternativ Mehle.",
        "…  jahrzehntelangen Mitstreitern und vielen Freunden aus Nah und Fern.",
        "Hi und Herzlich willkommen auf meiner Seite.",
        "Er ist Groß.",
        "Die Zahl ging auf Über 1.000 zurück.",
        "Er sammelt Große und kleine Tassen.",
        "Er sammelt Große, mittlere und kleine Tassen.",
        "Dann will sie mit London Über das Referendum verhandeln.",
        "Sie kann sich täglich Über vieles freuen.",
        "Der Vater (51) Fuhr nach Rom.",
        "Er müsse Überlegen, wie er das Problem löst.",
        "Er sagte, dass er Über einen Stein stolperte.",
        "Tom ist etwas über Dreißig.",
        "Unser warten wird sich lohnen.",
        "Tom kann mit fast Allem umgehen.",
        "Dabei Übersah er sie.",
        "Der Brief wird am Mittwoch in Brüssel Übergeben.",
        "Damit sollen sie die Versorgung in der Region Übernehmen.",
        "Die Unfallursache scheint geklärt, ein Lichtsignal wurde Überfahren.",
        "Der Lenker hatte die Höchstgeschwindigkeit um 76 km/h Überschritten.",
        "Das sind 10 Millionen Euro, Gleichzeitig und zusätzlich.",
        "Sie werden im Allgemeinen Fall gefasst.",
        "Das ist das Dümmste Kind.",
        "Er sagt, dass Geistliche Würdenträger davon betroffen sind.",
        "Er sagt, dass Geistliche und weltliche Würdenträger davon betroffen sind.",
        "Er ist begeistert Von der Fülle.",
        "Er wohnt Über einer Garage.",
        "Die Anderen 90 Prozent waren krank.",
        "Die Ausgewählten Leute werden gut betreut.",
        "Er war dort Im März 2000.",
        "Er war dort Im Jahr 96.",
        "Das ist es: Kein Satz.",
        "Wen magst du lieber: Die Giants oder die Dragons?",
        "Ich frage mich: Warum Das so ist.",
        "Das wirklich Wichtige Verfahren ist dies:",
        "Die Schöne Tür",
        "Das Blaue Auto.",
        "Ein Einfacher Satz zum Testen.",
        "Eine Einfache Frage zum Testen?",
        "Er kam Früher als sonst.",
        "Er rennt Schneller als ich.",
        "Das Winseln Stört.",
        "Sein verhalten war okay.",
        "Dem Norwegischen Ingenieur gelingt das gut.",
        "Das dabei erlernte und Erlebte Wissen ist sehr nützlich.",
        "Diese Regelung wurde als Überholt bezeichnet.",
        "Die Dolmetscherin und Der Vorleser gehen spazieren.",
        "Das sagen haben hier viele.",
        "Bis Bald!",
        "Das existiert im Jazz zunehmend nicht mehr Bei der weiteren Entwicklung des Jazz zeigt sich das.",
        "Das ist Eine Schreibweise.",
        "Das ist Ein Mann.",
        "Sie erhalten bald unsere Neuesten Insights.",
        "Auf eine Carvingschiene sollte die Kette schon im Kalten Zustand weit durchhängen.",
        "Das fahren ist einfach.",
        "Denn das fahren ist einfach.",
        "Denn das laufen ist einfach.",
        "Denn das essen ist einfach.",
        "Denn das gehen ist einfach.",
        "Das Große Auto wurde gewaschen.",
        "Ich habe ein Neues Fahrrad.",
    ] {
        assert_eq!(matches(text).len(), 1, "should be bad: {text}: {:?}", matches(text));
    }
}

#[test]
fn german_date_filters_match_java() {
    let _guard = engine_guard();
    // Java probes on 2026-09-17; the engine pins the same date.
    let build = |enabled: &[&str]| -> Option<Arc<Engine>> {
        let Ok(builder) = Engine::builder(Lang::De) else {
            eprintln!("skipping: no vendored data found");
            return None;
        };
        builder
            .today(2026, 9, 17)
            .options(lt::EngineOptions {
                enabled_rules: enabled.iter().map(|s| s.to_string()).collect(),
                enabled_only: true,
                picky: true,
                ..Default::default()
            })
            .build()
            .ok()
            .map(Arc::new)
    };
    let Some(engine) = build(&["DE_DATE_WEEKDAY"]) else {
        return;
    };
    // Java probe: 18..32, "Das Datum 15. September 2026 fällt nicht auf
    // einen Montag, sondern auf einen Dienstag.",
    // suggestions ["Dienstag, den 15", "Montag, den 14"]
    let result = engine
        .check("Der Termin ist am Montag, den 15. September 2026.")
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_DATE_WEEKDAY")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (18, 32));
    assert_eq!(
        m[0].message,
        "Das Datum 15. September 2026 fällt nicht auf einen Montag, sondern auf einen Dienstag."
    );
    assert_eq!(
        m[0].suggestions
            .iter()
            .map(|s| s.value.clone())
            .collect::<Vec<_>>(),
        vec!["Dienstag, den 15", "Montag, den 14"]
    );
    assert!(engine
        .check("Der Termin ist am Dienstag, den 15. September 2026.")
        .unwrap()
        .matches
        .iter()
        .all(|m| m.rule_id != "DE_DATE_WEEKDAY"));
    drop(engine);

    let Some(engine) = build(&["DE_DATE_WEEKDAY_CURRENTYEAR"]) else {
        return;
    };
    // Java probe: 18..32, "Bezieht sich dieses Datum auf das aktuelle Jahr?
    // Der 15. September 2026 fällt nicht auf einen Montag, sondern auf
    // einen Dienstag."
    let result = engine
        .check("Der Termin ist am Montag, den 15. September.")
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_DATE_WEEKDAY_CURRENTYEAR")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (18, 32));
    assert_eq!(
        m[0].message,
        "Bezieht sich dieses Datum auf das aktuelle Jahr? Der 15. September 2026 fällt nicht auf einen Montag, sondern auf einen Dienstag."
    );
    assert_eq!(
        m[0].suggestions
            .iter()
            .map(|s| s.value.clone())
            .collect::<Vec<_>>(),
        vec!["Dienstag, den 15", "Montag, den 14"]
    );
}

#[test]
fn german_multitoken_speller_two_three_match_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_MULTITOKEN_SPELLING_TWO", "DE_MULTITOKEN_SPELLING_THREE"])
    else {
        return;
    };
    // Java probes (CheckDump / ProbeRule on the pinned build): match range and
    // suggestion list, including the Java `HashMap` iteration order of the
    // `Stephen Hawkings|Stephen Hawking` case.
    type Case<'a> = (&'a str, &'a str, (usize, usize), &'a [&'a str]);
    let cases: &[Case] = &[
        (
            "Osama Bin Laden.",
            "DE_MULTITOKEN_SPELLING_THREE",
            (0, 15),
            &["Osama bin Laden"],
        ),
        (
            "Leonardo Da Vinci.",
            "DE_MULTITOKEN_SPELLING_THREE",
            (0, 17),
            &["Leonardo da Vinci"],
        ),
        (
            "Rio De Janeiro.",
            "DE_MULTITOKEN_SPELLING_THREE",
            (0, 14),
            &["Rio de Janeiro"],
        ),
        (
            "Louis Van Gaal.",
            "DE_MULTITOKEN_SPELLING_THREE",
            (0, 14),
            &["Louis van Gaal"],
        ),
        (
            "Brian de Palma.",
            "DE_MULTITOKEN_SPELLING_THREE",
            (0, 14),
            &["Brian De Palma"],
        ),
        (
            "Blu-Ray Disc.",
            "DE_MULTITOKEN_SPELLING_TWO",
            (0, 12),
            &["Blu-ray Disc"],
        ),
        (
            "IPAD mini.",
            "DE_MULTITOKEN_SPELLING_TWO",
            (0, 9),
            &["iPad mini", "iPad Mini"],
        ),
        (
            "Infratest Dimap.",
            "DE_MULTITOKEN_SPELLING_TWO",
            (0, 15),
            &["Infratest dimap"],
        ),
        (
            "Hannibal ante Portas.",
            "DE_MULTITOKEN_SPELLING_TWO",
            (9, 20),
            &["ante portas"],
        ),
        (
            "Im spiegel online.",
            "DE_MULTITOKEN_SPELLING_TWO",
            (3, 17),
            &["Spiegel Online"],
        ),
        (
            "Tyrannosaurus Rex.",
            "DE_MULTITOKEN_SPELLING_TWO",
            (0, 17),
            &["Tyrannosaurus rex"],
        ),
        (
            "Stephen Hawkins",
            "DE_MULTITOKEN_SPELLING_TWO",
            (0, 15),
            &["Stephen Hawkings", "Stephen Hawking"],
        ),
    ];
    for (sentence, rule_id, (from, to), suggestions) in cases {
        let result = engine.check(sentence).unwrap();
        let m: Vec<_> = result
            .matches
            .iter()
            .filter(|m| m.rule_id == *rule_id)
            .collect();
        assert_eq!(m.len(), 1, "{sentence}: {m:?}");
        assert_eq!(
            (m[0].range.start, m[0].range.end),
            (*from, *to),
            "{sentence}"
        );
        let values: Vec<&str> = m[0].suggestions.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(values, *suggestions, "{sentence}");
    }
}

#[test]
fn german_multitoken_speller_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_MULTITOKEN_SPELLING_FOUR"]) else {
        return;
    };
    // Java probes: 0..21 / 0..29 with the corrected proper names
    let result = engine.check("The Derk Knight Rises.").unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_MULTITOKEN_SPELLING_FOUR")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (0, 21));
    assert_eq!(m[0].suggestions[0].value, "The Dark Knight Rises");
    let result = engine.check("Rocky Mountains National Parc.").unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_MULTITOKEN_SPELLING_FOUR")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (0, 29));
    assert_eq!(m[0].suggestions[0].value, "Rocky Mountain National Park");
    // Java probe: "Rocky Mountains National Park." still matches (the
    // commented-out legacy example is aspirational)
    let result = engine.check("Rocky Mountains National Park.").unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_MULTITOKEN_SPELLING_FOUR")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!(m[0].suggestions[0].value, "Rocky Mountain National Park");
}

#[test]
fn german_compound_rule_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["DE_COMPOUNDS"]) else {
        return;
    };
    // Java probes: 40..48 "HNO-Arzt", 12..27 "CD-ROM-Laufwerk"
    let result = engine
        .check("Wenn es schlimmer wird, solltest Du zum HNO Arzt gehen.")
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_COMPOUNDS")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (40, 48));
    assert_eq!(
        m[0].message,
        "Dieses Wort wird mit Bindestrich geschrieben."
    );
    assert_eq!(m[0].suggestions[0].value, "HNO-Arzt");
    let result = engine.check("Das ist ein CD ROM Laufwerk.").unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "DE_COMPOUNDS")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (12, 27));
    assert_eq!(m[0].suggestions[0].value, "CD-ROM-Laufwerk");
    // the antipattern "an die 900 Meter Kabel" suppresses the match
    assert!(engine
        .check("Die Bürger konnten an die 900 Meter Kabel in Eigenregie verlegen.")
        .unwrap()
        .matches
        .iter()
        .all(|m| m.rule_id != "DE_COMPOUNDS"));
}

#[test]
fn german_compile_failures_report() {
    let _guard = engine_guard();
    let Ok(builder) = Engine::builder(Lang::De) else {
        return;
    };
    let engine = builder.build().unwrap();
    let failures = engine.compile_failures();
    if !failures.is_empty() {
        for (id, msg) in failures.iter().take(60) {
            eprintln!("FAIL {id}: {msg}");
        }
        panic!("{} compile failures", failures.len());
    }
}

#[test]
fn german_readability_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[
        "READABILITY_RULE_DIFFICULT_DE",
        "READABILITY_RULE_SIMPLE_DE",
    ]) else {
        return;
    };
    // Java probe (`ProbeRule`, de-DE):
    let simple = engine
        .check(
            "Das ist ein Haus. Der Hund ist da. Das Kind spielt gern.\n\n\
             Die Sonne scheint hell. Wir gehen nach Haus. Das ist fein.",
        )
        .unwrap();
    let m: Vec<_> = simple
        .matches
        .iter()
        .filter(|m| m.rule_id == "READABILITY_RULE_SIMPLE_DE")
        .collect();
    assert_eq!(m.len(), 2, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (0, 11));
    assert_eq!((m[1].range.start, m[1].range.end), (58, 75));
    assert_eq!(
        m[0].message,
        "Lesbarkeit: Der Text dieses Absatzes ist zu einfach {Grad 6: Sehr leicht}. \
         Zu wenige Wörter pro Satz und zu wenige Silben pro Wort."
    );
    assert_eq!(m[0].category_id, "CREATIVE_WRITING");
    let difficult = engine
        .check(
            "Die Verantwortlichkeiten der Geschäftsführung umfassen die strategische \
             Ausrichtung, die finanzielle Steuerung sowie die organisatorische \
             Weiterentwicklung des Unternehmens. Die Implementierung nachhaltiger \
             Entwicklungsstrategien erfordert umfangreiche Koordination.",
        )
        .unwrap();
    let m: Vec<_> = difficult
        .matches
        .iter()
        .filter(|m| m.rule_id == "READABILITY_RULE_DIFFICULT_DE")
        .collect();
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (0, 28));
    assert_eq!(
        m[0].message,
        "Lesbarkeit: Der Text dieses Absatzes ist zu schwierig {Grad 0: Sehr schwer}. \
         Zu viele Wörter pro Satz und zu viele Silben pro Wort."
    );
}

#[test]
fn german_unit_conversion_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["EINHEITEN_METRISCH"]) else {
        return;
    };
    // Java probe (`ProbeRule`, de-DE; Java offsets are UTF-16, engine ranges
    // are UTF-8 bytes — `ß` makes them differ after the first umlaut).
    let result = engine
        .check(
            "Ich bin 6 Fuß groß. Der Weg ist 3 Meilen lang. Es wiegt 5 Pfund. \
             Die Temperatur beträgt 100 Grad Fahrenheit.",
        )
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "EINHEITEN_METRISCH")
        .collect();
    assert_eq!(m.len(), 4, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (8, 14));
    assert_eq!(
        m[0].message,
        "Wollen Sie eine Umwandlung ins metrische System automatisch hinzufügen?"
    );
    assert_eq!(m[0].suggestions.first().unwrap().value, "6 Fuß (1,83 m)");
    assert_eq!(m[1].suggestions[3].value, "3 Meilen (4,83 Kilometer)");
    assert_eq!(m[1].suggestions[5].value, "3 Meilen (4.828,03 m)");
    assert_eq!(m[2].suggestions[2].value, "5 Pfund (ca. 2.268 g)");
    assert_eq!(m[3].suggestions[1].value, "100 Grad Fahrenheit (37,78 °C)");
    assert_eq!(m[0].category_id, "STYLE");

    let result = engine
        .check("Ich bin 6 Fuß (1,90 m) groß. Es sind 3 Meilen (3 Foo) weit.")
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "EINHEITEN_METRISCH")
        .collect();
    assert_eq!(m.len(), 2, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (8, 23));
    assert_eq!(
        m[0].message,
        "Diese Umrechnung scheint falsch zu sein. Wollen Sie sie automatisch korrigieren lassen?"
    );
    assert_eq!((m[1].range.start, m[1].range.end), (49, 54));
    assert_eq!(
        m[1].message,
        "Die in dieser Umrechnung verwendete Einheit wurde nicht erkannt."
    );
    assert_eq!(m[1].suggestions[0].value, "ca. 5 km");
}

#[test]
fn redundant_modal_verb_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["REDUNDANT_MODAL_VERB"]) else {
        return;
    };
    // `RedundantModalOrAuxiliaryVerbTest`: exactly one match per positive,
    // none per negative sentence (Java `lt.check` counts).
    let positives = [
        "Erst werde ich die Preise vergleichen und erst dann werde ich entscheiden, ob ich die Kamera kaufe.",
        "Sie hat das Foto von mir als kleinem Jungen angeschaut und hat gelacht.",
        "Ich bin gern in eurer Mitte und ich bin gern zu Gast bei euch.",
        "Da ich nun einmal bin, wer ich bin und was ich bin, kann ich es nicht übers Herz bringen.",
        "Wann ist jemand kühn und wann ist jemand tollkühn?",
        "Tom ist um halb drei angekommen und Mary ist kurze Zeit später angekommen.",
        "Das Essen ist gut und der Service hier ist gut.",
        "Wir müssen wissen, was wir tun sollen und wie wir es tun sollen.",
        "Ich muss unbedingt wissen, was zu tun ist, wie es zu tun ist und wann es zu tun ist.",
        "Er erzählte ihr, eine böse Hexe habe ihn in einen Frosch verwandelt und nur sie allein habe ihn retten können und morgen würden sie gemeinsam in sein Reich fahren.",
    ];
    for text in positives {
        let matches: Vec<_> = engine
            .check(text)
            .unwrap()
            .matches
            .into_iter()
            .filter(|m| m.rule_id == "REDUNDANT_MODAL_VERB")
            .collect();
        assert_eq!(matches.len(), 1, "{text:?}: {matches:?}");
    }
    let negatives = [
        "„nicht dürfen“ und „nicht müssen“ dürfen nicht miteinander verwechselt werden.",
        "Ich mag Physik und Mathematik mag ich noch mehr.",
        "Aus Angst vor den Zeitungen sind Politiker langweilig und am Ende sind sie selbst für die Zeitungen zu langweilig.",
        "Ändert der Akkusativ wirklich die Bedeutung eines Satzes: Sie hat Aids oder Aids hat sie?",
        "Tom hat eine Tüte Äpfel gekauft und er hat an einem Tag ein Drittel davon gegessen.",
        "Mein Vaterland ist mein Stadtviertel und nun ist es beinahe nicht wiedererkennbar.",
        "Johnny hat Alice vorgeschlagen und sie hat akzeptiert.",
        "Unsere Zeit hier ist begrenzt und sie ist wertvoll.",
        "Manchmal ist die Wahrheit nützlich und manchmal ist sie unnütz.",
        "Treue ist irgendwo absolut oder sie ist gar nichts.",
        "Sie mag niemanden und niemand mag sie.",
        "Eines Tages wird die USA eine Frau als Präsidenten wählen und das wird kein schönes Schauspiel sein.",
        "Mein Name ist Mary und das ist Tom.",
        "Er hat seine Augen langsam geöffnet und dann hat sie ihn geküsst.",
        "Die Flaneure haben es prinzipiell nicht eilig und sind immer für ein Schwätzchen zu haben.",
        "Es sind Tendenzen erkennbar und diese Tendenzen sind leider nicht ermutigend.",
        "Ja ist ja und nein ist nein, das nennt man Offenheit.",
        "Wie werden einmal unsere Namen hinter den Erfindern des Fliegens und dergleichen vergessen werden?",
        "In den Chroniken ist über die Flut von 1342 zu lesen, dass im Mainzer Dom einem Mann das Wasser bis zur Brust gestanden habe und dass man in Köln mit Booten über die Stadtmauern habe fahren können.",
        "Sie stellt sicher, dass die gerissenen Bänder nicht belastet werden können und dass das Gelenk dennoch bewegt werden kann.",
    ];
    for text in negatives {
        let matches: Vec<_> = engine
            .check(text)
            .unwrap()
            .matches
            .into_iter()
            .filter(|m| m.rule_id == "REDUNDANT_MODAL_VERB")
            .collect();
        assert!(matches.is_empty(), "{text:?}: {matches:?}");
    }
    // Java probe (UTF-16 offsets; these examples are ASCII after the match):
    let checks: &[(&str, usize, usize, &str)] = &[
        (
            "Erst werde ich die Preise vergleichen und erst dann werde ich entscheiden, ob ich die Kamera kaufe.",
            51,
            61,
            "Der Satzteil scheint redundant zu sein. Prüfen Sie, ob es gelöscht oder der Satz umformuliert werden kann.",
        ),
        (
            "Sie hat das Foto von mir als kleinem Jungen angeschaut und hat gelacht.",
            58,
            62,
            "Das Hilfsverb scheint redundant zu sein. Prüfen Sie, ob es gelöscht oder der Satz umformuliert werden kann.",
        ),
        (
            "Das Essen ist gut und der Service hier ist gut.",
            38,
            46,
            "Der Satzteil scheint redundant zu sein. Prüfen Sie, ob es gelöscht oder der Satz umformuliert werden kann.",
        ),
    ];
    for (text, from, to, message) in checks {
        let matches = engine.check(text).unwrap().matches;
        let m = matches
            .iter()
            .find(|m| m.rule_id == "REDUNDANT_MODAL_VERB")
            .unwrap();
        assert_eq!((m.range.start, m.range.end), (*from, *to), "{text:?}");
        assert_eq!(&m.message, message);
        assert_eq!(m.suggestions[0].value, "");
        assert_eq!(m.category_id, "STYLE");
    }
}

#[test]
fn old_spelling_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&["OLD_SPELLING_RULE"]) else {
        return;
    };
    // Java probe (`ProbeRule`, de-DE): 4..10 and 39..43 (UTF-16); engine
    // ranges are UTF-8 bytes (`ß`/`ä` widen the offsets).
    let result = engine
        .check(
            "Der Abfluß ist schon wieder verstopft. Läßt du das bitte. \
                Ein Schloß Holte gibt es. Das Foto zeigt Photonen.",
        )
        .unwrap();
    let m: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "OLD_SPELLING_RULE")
        .collect();
    assert_eq!(m.len(), 2, "{m:?}");
    assert_eq!((m[0].range.start, m[0].range.end), (4, 11));
    assert_eq!(m[0].suggestions[0].value, "Abfluss");
    assert_eq!(
        m[0].message,
        "Diese Schreibweise war nur in der alten Rechtschreibung korrekt. \
         Das Wort wird mit 'ss' geschrieben, wenn davor eine kurz gesprochene Silbe steht."
    );
    assert_eq!((m[1].range.start, m[1].range.end), (40, 46));
    assert_eq!(m[1].suggestions[0].value, "Lässt");
    assert_eq!(m[0].category_id, "TYPOS");
}

/// The in-tree hunspell checking engine (`lt_spell::hunspell`) must match
/// the native libhunspell results `GermanSpellerRule` sees. Every expected
/// value below was produced by `scripts/oracle/de/probe-hunspell.sh` on the
/// pinned Docker build (raw `DumontsHunspellDictionary.spell()`), covering
/// affixes, umlaut plurals, compounds, hyphen/break handling, CHECKSHARPS,
/// hidden capitalizations and forbidden dictionary entries.
#[test]
fn hunspell_engine_matches_java_probe() {
    let _guard = engine_guard();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/de/hunspell");
    if !dir.exists() {
        eprintln!("skipping: no vendored data found");
        return;
    }
    let checker =
        lt_spell::hunspell::HunspellChecker::load(&dir.join("de_DE.aff"), &dir.join("de_DE.dic"))
            .unwrap();
    let cases: &[(&str, bool)] = &[
        ("Haus", true),
        ("haus", false),
        ("Häuser", true),
        ("Bäume", true),
        ("Bett", true),
        ("bett", false),
        ("Straße", true),
        ("Straßenbahn", true),
        ("Arbeitszimmer", true),
        ("Arbeitsamt", true),
        ("Donaudampfschifffahrt", true),
        ("Dampfschifffahrt", true),
        ("Add-on", true),
        ("Add-on-Einstellungen", true),
        ("Gaus", true),
        ("Eugen-Gaus-Straße", true),
        ("Karl-Marx-Straße", true),
        ("OK", true),
        ("ok", false),
        ("jo", false),
        ("HAUS", true),
        ("WIFI", false),
        ("AutoBahnn", false),
        ("Busgeld", false),
        ("Treuehand", false),
        ("bedarfs", false),
        ("mus", false),
        ("planet", false),
        ("EU-DSGVO", false),
        ("GT-SUITE", false),
        ("SD-WAN", false),
        ("HUK-COBURG", false),
        ("Bundesverfassungsgericht", true),
        ("verfassungsrechtlich", true),
        ("schön", true),
        ("schönste", true),
        ("Krankenhaus", true),
        ("Krankenhäuser", true),
        ("Fußball", true),
        ("Fussball", false),
        ("Kuß", false),
        ("daß", false),
        ("E-Mail", true),
        ("e-mail", false),
        ("Spielzug", true),
        ("Spielzüge", true),
        ("mitarbeitenden", true),
        ("Prozeß", false),
        ("Prozess", true),
        ("Standart", false),
        ("Standarte", true),
        ("Tisch", true),
        ("Tische", true),
        ("Fuß", true),
        ("Füße", true),
        ("Maus", true),
        ("Mäuse", true),
        ("Auto", true),
        ("Autos", true),
        ("gehen", true),
        ("gegangen", true),
        ("schreiben", true),
        ("geschrieben", true),
        ("fahren", true),
        ("gefahren", true),
    ];
    for (word, expected) in cases {
        assert_eq!(
            checker.spell(word),
            *expected,
            "spell({word:?}) mismatch (Java probe says {expected})"
        );
    }
}

/// Same engine, Austrian and Swiss raw dictionaries: the probe values come
/// from `probe-hunspell.sh <words> de_AT`/`de_CH`.
#[test]
fn hunspell_engine_matches_java_probe_variants() {
    let _guard = engine_guard();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/de/hunspell");
    if !dir.exists() {
        eprintln!("skipping: no vendored data found");
        return;
    }
    let cases: &[(&str, &[(&str, bool)])] = &[
        (
            "de_AT",
            &[
                ("Haus", true),
                ("haus", false),
                ("Häuser", true),
                ("WIFI", false),
                ("ok", false),
                ("HAUS", true),
                ("Stiege", true),
                ("Jause", true),
                ("Erdäpfel", true),
                ("Erdapfel", true),
                ("Paradeiser", true),
                ("Tomate", true),
            ],
        ),
        (
            "de_CH",
            &[
                ("Haus", true),
                ("haus", false),
                ("Häuser", true),
                ("WIFI", false),
                ("ok", false),
                ("HAUS", true),
                ("Grüezi", true),
                ("Velo", true),
                ("Fahrrad", true),
                ("Straße", false),
                ("Strasse", true),
                ("ss", false),
            ],
        ),
    ];
    for (variant, words) in cases {
        let checker = lt_spell::hunspell::HunspellChecker::load(
            &dir.join(format!("{variant}.aff")),
            &dir.join(format!("{variant}.dic")),
        )
        .unwrap();
        for (word, expected) in *words {
            assert_eq!(checker.spell(word), *expected, "{variant} spell({word:?})");
        }
    }
}

/// Rule-level `GermanSpellerRule.isMisspelled` parity: the pinned values come
/// from `scripts/oracle/de/probe-speller.sh` (`ProbeSpeller`, 4,054-word
/// probe) and `probe-spell-debug.sh`, pinned Docker build. Covers the
/// hunspell backend, the expanded ignore lists (`LineExpander`), the
/// prohibit patterns and the German heuristics.
#[test]
fn german_speller_is_misspelled_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = de_engine(&[]) else {
        return;
    };
    let _ = engine; // the probe goes through the speller directly
    let Ok(data) = lt::DataDir::discover() else {
        eprintln!("skipping: no vendored data found");
        return;
    };
    let rule = lt::spelling_de_probe(&data, "de-DE");
    let cases: &[(&str, bool)] = &[
        ("Haus", false),
        ("haus", true),
        ("Häuser", false),
        ("Bett", false),
        ("bett", true),
        ("Straße", false),
        ("Arbeitszimmer", false),
        ("Add-on", false),
        ("Gaus", false),
        ("ok", false),
        ("jo", false),
        ("HAUS", false),
        ("WIFI", true),
        ("AutoBahnn", true),
        ("Busgeld", true),
        ("bedarfs", true),
        ("mus", true),
        ("planet", true),
        ("EU-DSGVO", false),
        ("Bundesverfassungsgericht", false),
        ("schön", false),
        ("Fußball", false),
        ("Fussball", true),
        ("Kuss", false),
        ("daß", true),
        ("dass", false),
        ("E-Mail", false),
        ("e-mail", true),
        ("Spielzug", false),
        ("Standart", true),
        ("Standarte", false),
        ("mitarbeitenden", false),
        ("Prozess", false),
        ("Prozeß", true),
        ("J.-Weissenecker-Gasse", false),
        ("(sic!)", false),
        ("Sozialpfl.", false),
        ("El.-Techn.", false),
        ("Pie'", false),
        ("Prof.-Schipperges-Straße", false),
        ("Dr.Lustkandl-Gasse", false),
        ("Kälberstall", false),
        ("Seeölbachweg", false),
        ("abdruck", false),
        ("rückwärtslaufen", false),
        ("hinzu_tun", true),
        ("weg_ducken", true),
        ("herum_schreiben", true),
        ("Afghan_in", true),
        ("magermilchpulver", true),
        ("bambusrohr", true),
        ("rauswollen", false),
        ("runterwillst", false),
        ("Stammtischexpert_in", true),
        ("flankenschrift", true),
        ("Pedibus-Linie", false),
        ("Chordophon", false),
        ("Docusign", false),
        ("Layoff", false),
        ("Vanauken", false),
        ("brennnessel", true),
        ("Brennnessel", false),
        ("Impfpflicht", false),
        ("Impflicht", true),
        ("Fussballplatz", true),
        ("Fußballplatz", false),
        ("wiederspiegeln", true),
        ("widerspiegeln", false),
    ];
    for (word, expected) in cases {
        assert_eq!(
            rule.is_misspelled(word),
            *expected,
            "isMisspelled({word:?}) mismatch (Java probe says {expected})"
        );
    }
}
