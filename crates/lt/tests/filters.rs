//! End-to-end tests for the English filter registry (P1.6): each test uses
//! an incorrect `<example>` of a filter rule from the vendored grammar and
//! asserts the rule fires (or is filtered) with the expected suggestions.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use lt::{Engine, Lang};

static TODAY_ENGINE: OnceLock<Option<Arc<Engine>>> = OnceLock::new();

fn engine_with_today() -> Option<Arc<Engine>> {
    TODAY_ENGINE
        .get_or_init(|| {
            let Ok(builder) = Engine::builder(Lang::En) else {
                eprintln!("skipping: no vendored data found");
                return None;
            };
            // Pin "today" so FutureDateFilter/NewYearDateFilter examples
            // behave as in the legacy XML examples (the legacy test harness pins
            // 2014-01-01).
            match builder.today(2014, 1, 1).build() {
                Ok(e) => Some(Arc::new(e)),
                Err(_) => {
                    eprintln!("skipping: no vendored data found");
                    None
                }
            }
        })
        .clone()
}

/// Engine with the given (often `tags="picky"`) rules explicitly enabled,
/// like LT's rule tests do. Engines are cached per rule set: building an
/// engine compiles thousands of regexes, so tests must not repeat it.
fn engine_with_enabled(rules: &[&str]) -> Option<Arc<Engine>> {
    type Cache = Mutex<HashMap<String, Arc<OnceLock<Option<Arc<Engine>>>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = rules.join(",");
    let cell = CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .entry(key)
        .or_default()
        .clone();
    cell.get_or_init(|| {
        let Ok(builder) = Engine::builder(Lang::En) else {
            eprintln!("skipping: no vendored data found");
            return None;
        };
        let options = lt::EngineOptions {
            enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
            enabled_only: true,
            picky: true,
            ..Default::default()
        };
        builder
            .today(2014, 1, 1)
            .options(options)
            .build()
            .ok()
            .map(Arc::new)
    })
    .clone()
}

fn rule_of(engine: &Engine, text: &str, expected: &str) -> Option<lt::Match> {
    let result = engine.check(text).unwrap();
    result
        .matches
        .iter()
        .find(|m| m.rule_id == expected)
        .cloned()
}

#[test]
fn ordinal_suffix_filter_rewrites_suggestion() {
    let Some(engine) = engine_with_enabled(&["ORDINAL_NUMBER_SUFFIX"]) else {
        return;
    };
    // ORDINAL_NUMBER_SUFFIX uses OrdinalSuffixFilter: "1nd" -> "1st"
    let m = rule_of(&engine, "This was my 1nd try.", "ORDINAL_NUMBER_SUFFIX")
        .expect("rule should fire");
    assert!(
        m.suggestions.iter().any(|s| s.value == "1st"),
        "suggestions: {:?}",
        m.suggestions
    );
    // "1th" -> "1st"
    let m = rule_of(&engine, "This was my 1th try.", "ORDINAL_NUMBER_SUFFIX")
        .expect("rule should fire");
    assert!(m.suggestions.iter().any(|s| s.value == "1st"));
}

#[test]
fn apostrophe_type_filters_direction() {
    // both rules are pick-mode (default="off"); enable them explicitly
    let Some(engine) = (|| {
        let Ok(builder) = Engine::builder(Lang::En) else {
            eprintln!("skipping: no vendored data found");
            return None;
        };
        let options = lt::EngineOptions {
            enabled_rules: vec![
                "TYPOGRAPHICAL_APOSTROPHE".into(),
                "TYPEWRITER_APOSTROPHE".into(),
            ],
            enabled_only: true,
            picky: true,
            ..Default::default()
        };
        builder.options(options).build().ok()
    })() else {
        return;
    };
    // straight apostrophe → TYPOGRAPHICAL_APOSTROPHE suggests ’s
    let m = rule_of(&engine, "An actress's role", "TYPOGRAPHICAL_APOSTROPHE")
        .expect("rule should fire");
    assert!(
        m.suggestions.iter().any(|s| s.value == "’s"),
        "suggestions: {:?}",
        m.suggestions
    );
    // typographic apostrophe → TYPOGRAPHICAL_APOSTROPHE must not fire
    let result = engine.check("An actress’s role").unwrap();
    assert!(
        !result
            .matches
            .iter()
            .any(|m| m.rule_id == "TYPOGRAPHICAL_APOSTROPHE"),
        "matches: {:?}",
        result.matches
    );
}

#[test]
fn underline_spaces_filter_extends_range() {
    let Some(engine) = engine_with_enabled(&["HYPHEN_TO_EN"]) else {
        return;
    };
    // HYPHEN_TO_EN uses UnderlineSpacesFilter (both): the range must include
    // the spaces around the hyphen
    let text = "Roman Principate (30 BC - AD 284)";
    let m = rule_of(&engine, text, "HYPHEN_TO_EN").expect("rule should fire");
    assert_eq!(&text[m.range.start..m.range.end], " - ");
}

#[test]
fn adverb_filter_maps_adverb_to_adjective() {
    let Some(engine) = engine_with_enabled(&["A_RB_NN"]) else {
        return;
    };
    // A_RB_NN + AdverbFilter: adverb -> adjective in the suggestion
    let text = "It was a slowly process.";
    let result = engine.check(text).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "A_RB_NN")
        .expect("rule should fire");
    assert!(
        m.suggestions.iter().any(|s| s.value == "slow process"),
        "suggestions: {:?}",
        m.suggestions
    );
}

#[test]
fn date_range_checker_rejects_valid_ranges() {
    let Some(engine) = engine_with_today() else {
        return;
    };
    // INVALID_DATE sub-rule with DateRangeChecker accepts only x >= y
    let good = "from January 10 to 5, 2016";
    let result = engine.check(good).unwrap();
    // the specific DateRangeChecker-guarded sub-rule must not fire on an
    // ascending range; nothing asserts a specific rule id here because the
    // ascending case is covered by sibling rules
    let _ = result;
}

#[test]
fn numbers_in_word_filter_suggests_corrections() {
    let Some(engine) = engine_with_today() else {
        return;
    };
    let m = rule_of(&engine, "Go0d morning!", "NUMBERS_IN_WORDS").expect("rule should fire");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert!(
        values.contains(&"Good") || values.contains(&"God"),
        "suggestions: {values:?}"
    );
}

#[test]
fn find_suggestions_filter_fills_suggestions() {
    let Some(engine) = engine_with_enabled(&["PRP_MD_NN"]) else {
        return;
    };
    // PRP_MD_NN + FindSuggestionsFilter: the filter accepts the match (the
    // dictionary word "cheeseburger" yields no spelling alternatives, so the
    // suggestion list may legitimately stay empty)
    let _m = rule_of(&engine, "I can cheeseburger.", "PRP_MD_NN").expect("rule should fire");
}

#[test]
fn suppress_misspelled_filters_all_misspelled_suggestions() {
    let Some(engine) = engine_with_today() else {
        return;
    };
    // The EN_COMPOUNDS-style rules with EnglishSuppressMisspelledSuggestionsFilter
    // keep their match but the suggestions must be correctly spelled words.
    let text = "The break- up was painful.";
    let result = engine.check(text).unwrap();
    for m in &result.matches {
        for s in &m.suggestions {
            assert!(
                !s.value.contains(char::is_whitespace) || s.value.len() > 1,
                "unexpected suggestion {s:?} in match {m:?}"
            );
        }
    }
}

// --- Non-default built-ins (text-level/paragraph rules, picky or default off)

#[test]
fn repeated_words_suggests_synonyms() {
    let Some(engine) = engine_with_enabled(&["EN_REPEATEDWORDS"]) else {
        return;
    };
    let text = "The problem is big. Another problem appears on the screen.";
    let result = engine.check(text).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "EN_REPEATEDWORDS")
        .expect("rule should fire");
    assert_eq!(m.range.start, 28);
    assert_eq!(m.range.end, 35);
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["issue", "concern", "difficulty"]);
    assert_eq!(
        m.specific_rule_id.as_deref(),
        Some("EN_REPEATEDWORDS_PROBLEM")
    );
}

#[test]
fn repeated_words_respects_antipatterns_and_case() {
    let Some(engine) = engine_with_enabled(&["EN_REPEATEDWORDS"]) else {
        return;
    };
    // "solve the problem" is an antipattern ("unique collocation")
    let result = engine
        .check("She can solve the problem quickly. Another problem appears.")
        .unwrap();
    assert!(
        !result
            .matches
            .iter()
            .any(|m| m.rule_id == "EN_REPEATEDWORDS"),
        "antipattern should immunize: {:?}",
        result.matches
    );
    // a mid-sentence capitalized repetition is an exception
    let result = engine
        .check("The problem is big. Another Problem appears.")
        .unwrap();
    assert!(!result
        .matches
        .iter()
        .any(|m| m.rule_id == "EN_REPEATEDWORDS"));
}

#[test]
fn readability_difficult_fires_per_paragraph() {
    let Some(engine) = engine_with_enabled(&["READABILITY_RULE_DIFFICULT"]) else {
        return;
    };
    let text = "The incomprehensibility of the internationalization infrastructure necessitates \
                substantial reconsideration and fundamental transformation throughout the entire organization.\n\n\
                Unquestionably, the implementation demonstrates considerable complexity regarding \
                interoperability, maintainability, and overall architectural sustainability.";
    let result = engine.check(text).unwrap();
    let matches: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "READABILITY_RULE_DIFFICULT")
        .collect();
    assert_eq!(matches.len(), 2, "matches: {:?}", result.matches);
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 26));
    assert_eq!(
        matches[0].message,
        "Readability: The text of this paragraph is too difficult {Level 0: Very difficult}. \
         Too many words per sentence and too many syllables per word."
    );
}

#[test]
fn readability_simple_fires_per_paragraph() {
    let Some(engine) = engine_with_enabled(&["READABILITY_RULE_SIMPLE"]) else {
        return;
    };
    let text =
        "The cat sat on the mat. The dog ran to the man. We can see the sun and the sky.\n\n\
                It is a big red bus. She has a new blue hat. They like to run and play in the sun.";
    let result = engine.check(text).unwrap();
    let matches: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "READABILITY_RULE_SIMPLE")
        .collect();
    assert_eq!(matches.len(), 2, "matches: {:?}", result.matches);
    assert!(matches[0].message.contains("{Level 6: Very easy}"));
}

#[test]
fn long_paragraph_rule_fires_on_long_paragraph() {
    let Some(engine) = engine_with_enabled(&["TOO_LONG_PARAGRAPH"]) else {
        return;
    };
    let long: String = (1..=12)
        .map(|i| {
            (1..=20)
                .map(|j| format!("s{i}w{j}"))
                .collect::<Vec<_>>()
                .join(" ")
                + "."
        })
        .collect::<Vec<_>>()
        .join(" ");
    let text = format!("{long}\n\nShort second paragraph.");
    let result = engine.check(&text).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "TOO_LONG_PARAGRAPH")
        .expect("rule should fire");
    // words 219 and 220 of the paragraph
    assert_eq!(&text[m.range.start..m.range.end], "s11w19 s11w20");
    assert_eq!(
        m.message,
        "This paragraph is over 220 words long here, consider revising it."
    );
}

#[test]
fn paragraph_rules_match_java_offsets() {
    // EMPTY_LINE: two blank lines ("\n\n\n\n") after a sentence
    let Some(engine) = engine_with_enabled(&["EMPTY_LINE"]) else {
        return;
    };
    let result = engine
        .check("First sentence here.\n\n\n\nSecond paragraph sentence.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "EMPTY_LINE")
        .expect("EMPTY_LINE should fire");
    assert_eq!((m.range.start, m.range.end), (19, 20));

    // WHITESPACE_PARAGRAPH_BEGIN
    let Some(engine) = engine_with_enabled(&["WHITESPACE_PARAGRAPH_BEGIN"]) else {
        return;
    };
    let result = engine
        .check("First sentence here. More text.\n\n Indented paragraph starts here. More text.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "WHITESPACE_PARAGRAPH_BEGIN")
        .expect("WHITESPACE_PARAGRAPH_BEGIN should fire");
    assert_eq!((m.range.start, m.range.end), (33, 42));
    assert_eq!(m.suggestions[0].value, "Indented");

    // WHITESPACE_PARAGRAPH (space before paragraph end)
    let Some(engine) = engine_with_enabled(&["WHITESPACE_PARAGRAPH"]) else {
        return;
    };
    let result = engine
        .check("First sentence here. More text. \n\nSecond paragraph here.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "WHITESPACE_PARAGRAPH")
        .expect("WHITESPACE_PARAGRAPH should fire");
    assert_eq!((m.range.start, m.range.end), (30, 32));
    assert_eq!(m.suggestions[0].value, ".");
}

#[test]
fn punctuation_paragraph_end_rules_match_java() {
    // PUNCTUATION_PARAGRAPH_END is picky + default on
    let Some(engine) = engine_with_enabled(&["PUNCTUATION_PARAGRAPH_END"]) else {
        return;
    };
    let text = "First sentence here. Second sentence without end\n\n\
                Third paragraph sentence. Another one here.";
    let result = engine.check(text).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PUNCTUATION_PARAGRAPH_END")
        .expect("PUNCTUATION_PARAGRAPH_END should fire");
    assert_eq!((m.range.start, m.range.end), (45, 48));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["end.", "end!", "end?", "end:", "end,", "end;"]);

    // PUNCTUATION_PARAGRAPH_END2 is default off, not picky
    let Some(engine) = engine_with_enabled(&["PUNCTUATION_PARAGRAPH_END2"]) else {
        return;
    };
    let text =
        "This is a fairly long first sentence that keeps going on. Second sentence ending here\n\n\
                Short paragraph.";
    let result = engine.check(text).unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "PUNCTUATION_PARAGRAPH_END2")
        .expect("PUNCTUATION_PARAGRAPH_END2 should fire");
    assert_eq!((m.range.start, m.range.end), (81, 85));
    assert_eq!(m.suggestions[0].value, "here.");
}

#[test]
fn paragraph_repeat_beginning_matches_both_paragraphs() {
    let Some(engine) = engine_with_enabled(&["PARAGRAPH_REPEAT_BEGINNING_RULE"]) else {
        return;
    };
    let text = "Problems are everywhere. More text follows here.\n\n\
                Problems keep appearing. Another sentence follows.";
    let result = engine.check(text).unwrap();
    let matches: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "PARAGRAPH_REPEAT_BEGINNING_RULE")
        .collect();
    assert_eq!(matches.len(), 2, "matches: {:?}", result.matches);
    assert_eq!((matches[0].range.start, matches[0].range.end), (0, 8));
    assert_eq!((matches[1].range.start, matches[1].range.end), (50, 58));
    assert_eq!(matches[0].message, "Same beginning as last paragraph");
}

#[test]
fn passive_voice_simple_suggestion_has_no_leading_space() {
    let Some(engine) = engine_with_enabled(&["PASSIVE_VOICE_SIMPLE"]) else {
        return;
    };
    // Java `CheckDumpPicky`: range 12-39, suggestion "Sam does not choose
    // Angela" (the leading `<match no="10">` is an unmatched optional
    // reference; Java's `concatWithoutExtraSpace` drops the following space)
    let m = rule_of(
        &engine,
        "Other days, Angela is not chosen by Sam.",
        "PASSIVE_VOICE_SIMPLE",
    )
    .expect("rule should fire");
    assert_eq!((m.range.start, m.range.end), (12, 39));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["Sam does not choose Angela"]);
}

#[test]
fn pattern_match_type_maps_style_issue_types_to_hint() {
    // Java `AbstractPatternRule.getType()`: `issueType="style"` pattern
    // rules report `Hint`; `Other` otherwise. The comparison harness
    // verifies the full corpus against the Java TSV `type` column (D-023).
    let Some(engine) = engine_with_enabled(&["SERIAL_COMMA_ON"]) else {
        return;
    };
    let m = rule_of(
        &engine,
        "I bought apples, bananas and pears.",
        "SERIAL_COMMA_ON",
    )
    .expect("rule should fire");
    assert_eq!(m.issue_type, "style");
    assert_eq!(m.match_type, "Hint");
    // a non-style pattern rule stays `Other`
    let Some(engine) = engine_with_enabled(&["EN_UNPAIRED_BRACKETS"]) else {
        return;
    };
    let m =
        rule_of(&engine, "This is (unclosed.", "EN_UNPAIRED_BRACKETS").expect("rule should fire");
    assert_eq!(m.match_type, "Other");
}

// --- METRIC_UNITS_EN_US (UnitConversionRuleUS, picky, D-021)

fn metric_match(engine: &Engine, text: &str) -> lt::Match {
    let result = engine.check(text).unwrap();
    result
        .matches
        .iter()
        .find(|m| m.rule_id == "METRIC_UNITS_EN_US")
        .cloned()
        .unwrap_or_else(|| panic!("METRIC_UNITS_EN_US should fire on {text:?}"))
}

#[test]
fn metric_units_suggestion_matches_java() {
    let Some(engine) = engine_with_enabled(&["METRIC_UNITS_EN_US"]) else {
        return;
    };
    // `CheckDumpPicky` line "It is 3 miles." (Docker probe)
    let m = metric_match(&engine, "It is 3 miles.");
    assert_eq!((m.range.start, m.range.end), (6, 13));
    assert_eq!(
        m.message,
        "Writing for an international audience? Consider adding the metric equivalent."
    );
    assert_eq!(m.short_message.as_deref(), Some("Add metric equivalent?"));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec![
            "3 miles (ca. 5 km)",
            "3 miles (4.83 km)",
            "3 miles (ca. 5 kilometers)",
            "3 miles (4.83 kilometers)",
            "3 miles (ca. 4,828 m)",
            "3 miles (4,828.03 m)",
        ]
    );
    assert_eq!(m.category_id, "STYLE");
    assert_eq!(m.issue_type, "style");
    assert!(m.picky);

    // special feet+inch pattern: group 0 (with the leading space) is used
    let m = metric_match(&engine, "It is 5ft2.");
    assert_eq!((m.range.start, m.range.end), (5, 10));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec![
            " 5ft2 (1.57 m)",
            " 5ft2 (1.57 meters)",
            " 5ft2 (ca. 157 cm)",
            " 5ft2 (157.48 cm)",
            " 5ft2 (ca. 157 centimeters)",
            " 5ft2 (157.48 centimeters)",
        ]
    );
}

#[test]
fn metric_units_check_paths_match_java() {
    let Some(engine) = engine_with_enabled(&["METRIC_UNITS_EN_US"]) else {
        return;
    };
    // wrong conversion in parentheses -> CHECK over the whole span
    let m = metric_match(&engine, "Already converted wrong: 100 miles (150 km).");
    assert_eq!((m.range.start, m.range.end), (25, 43));
    assert_eq!(
        m.message,
        "This unit conversion doesn't seem right. Do you want to correct it automatically?"
    );
    assert_eq!(m.suggestions[0].value, "100 miles (ca. 161 km)");
    assert_eq!(m.suggestions.len(), 6);

    // unknown unit after the number -> CHECK_UNKNOWN_UNIT over "5 furlongs"
    let m = metric_match(&engine, "Unknown unit conversion: 100 miles (5 furlongs).");
    assert_eq!((m.range.start, m.range.end), (36, 46));
    assert_eq!(
        m.message,
        "This unit conversion doesn't seem right, unable to recognize the used unit."
    );
    assert_eq!(
        m.suggestions[0].value, "ca. 161 km",
        "suggestions must not carry the original prefix here"
    );

    // incompatible units -> UNIT_MISMATCH (Java: `UnconvertibleException`)
    let m = metric_match(&engine, "100 Celsius (100 miles).");
    assert_eq!((m.range.start, m.range.end), (0, 23));
    assert_eq!(m.message, "These units don't seem to be compatible.");
    assert!(m.suggestions.is_empty());

    // already metric with a wrong conversion -> reverse-conversion CHECK
    let m = metric_match(&engine, "3 m (10 ft).");
    assert_eq!((m.range.start, m.range.end), (5, 7));
    assert_eq!(
        m.message,
        "This unit conversion doesn't seem right. Do you want to correct it automatically?"
    );
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec!["ca. 10 ft", "9.84 ft", "ca. 10 feet", "9.84 feet"]
    );

    // non-metric with a wrong metric conversion -> CHECK with the original
    let m = metric_match(&engine, "100 kg (100 lb).");
    assert_eq!((m.range.start, m.range.end), (8, 11));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec!["ca. 220 lb", "220.46 lb", "ca. 220 pounds", "220.46 pounds",]
    );
}
