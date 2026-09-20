//! French engine tests: Java-probed rule values (pinned LT build, Docker).
//!
//! Offsets are UTF-8 bytes (the engine format); the Java probes print UTF-16
//! code units, so non-ASCII probes are converted in the comments. Every case
//! comes from `scripts/oracle/fr/probe-rule.sh` / `probe-disambig.sh`.

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{Engine, EngineOptions, Lang};

/// One engine at a time: the French engine is large (6.6k rules) and several
/// tests build engines with different settings.
fn engine_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn engine() -> Option<Engine> {
    let Ok(builder) = Engine::builder(Lang::Fr) else {
        eprintln!("skipping: no vendored data found");
        return None;
    };
    builder.build().ok()
}

fn rule_match<'a>(result: &'a lt::CheckResult, id: &str) -> &'a lt::Match {
    result
        .matches
        .iter()
        .find(|m| m.rule_id == id)
        .unwrap_or_else(|| panic!("rule {id} did not match; matches: {:#?}", result.matches))
}

#[test]
fn french_speller_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("La maizon est grande.").unwrap();
    let m = rule_match(&result, "FR_SPELLING_RULE");
    // Java: UTF-16 3..9
    assert_eq!((m.range.start, m.range.end), (3, 9));
    assert_eq!(m.message, "Faute de frappe possible trouvée.");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec!["maison", "méson", "Mazion", "Muizon", "Mézin", "Mézos"]
    );
}

#[test]
fn french_word_with_determiner_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("un belle maison").unwrap();
    let m = rule_match(&result, "D_J");
    // Java: UTF-16 0..8
    assert_eq!((m.range.start, m.range.end), (0, 8));
    assert_eq!(
        m.message,
        "\"Un\" et l'adjectif \"belle\" ne semblent pas bien accordés."
    );
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec![
            "une belle",
            "un beau",
            "un bel",
            "des beaux",
            "des bels",
            "des belles"
        ]
    );
}

#[test]
fn french_find_suggestions_vowel_plural_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java probe (`probe-rule.sh`, pinned build): `ai eu|avais eu|eus`.
    // `FindSuggestionsFilter` appends the plural probe `w + "s"` only when
    // `ENDS_IN_VOWEL.matcher(w).matches()` — a *full* match, so only a
    // single-vowel `w` qualifies. Rust used `is_match` (trailing vowel) and
    // added the `ëus` suggestions (`élus`, `bus`, `dus`, ...).
    let result = engine.check("je les eu sans les pattes de col").unwrap();
    let m = rule_match(&result, "PRONSUJ_NONVERBE");
    assert_eq!((m.range.start, m.range.end), (7, 9));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["ai eu", "avais eu", "eus"]);
}

#[test]
fn french_speller_dotless_i_ordering_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java probe (`probe-speller.sh entraı entraıs`): `entrai|entra|entrât|entras`
    // and the speller2-merged `entrais|...` list. Java's `equalsIgnoreCase`
    // maps `ı` and `i` to the same uppercase `I`, so the suggestion `entrai`
    // counts as the case-only variant of `entraı` (orderSuggestions moves it
    // first and `onlyCaseDiffers` enables the distance-2 speller).
    let result = engine
        .check("Les effets des entraı ˆnements phonologiques et multisensoriels favorise l'apprentissage.")
        .unwrap();
    let m = result
        .matches
        .iter()
        .find(|m| m.rule_id == "FR_SPELLING_RULE")
        .expect("FR_SPELLING_RULE did not match");
    // Java UTF-16 15..21; `entraı` is bytes 15..22 (dotless ı is two bytes).
    assert_eq!((m.range.start, m.range.end), (15, 22));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["entrai", "entra", "entrât", "entras"]);
}

#[test]
fn french_suggestion_case_from_pre_marker_token_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java probe (`probe-rule.sh`): `N'a|N'est`. PRONSUJ_NONVERBE matches
    // `Il pas` with the marker on `[ne] pas`; Java's `startPositionCorrection`
    // arithmetic samples the pre-marker `Il` (the optional `ne` is unmatched),
    // so the literal lowercase suggestions are capitalized.
    let result = engine.check("Il pas président.").unwrap();
    let m = rule_match(&result, "PRONSUJ_NONVERBE");
    assert_eq!((m.range.start, m.range.end), (3, 6));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["N'a", "N'est"]);
}

#[test]
fn french_raw_pos_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // `<pattern raw_pos="yes">` rules match the pre-disambiguation token
    // view (Java `getPreDisambigTokensWithoutWhitespace`), e.g. `passer` still
    // carries its noun reading before `_GN_MS` filtering. Java probes
    // (`probe-rule.sh`), UTF-16 offsets (ASCII: identical to UTF-8 bytes):
    //   CE_SE "Cela ce passe sous la table." 5..7 `se`
    //   CE_SE "Il faut tout ce dire."        13..15 `se`
    //   A_ACCENT "Il soumis a rude épreuve." 10..11 `à`
    //   DU_DU "C'est trop, tu n'aurais pas du savoir !" 28..30 `dû`
    let result = engine.check("Cela ce passe sous la table.").unwrap();
    let m = rule_match(&result, "CE_SE");
    assert_eq!((m.range.start, m.range.end), (5, 7));
    assert_eq!(
        m.message,
        "Le pronom réfléchi \"se\" est attendu devant le verbe pronominal \"ce\"."
    );
    assert_eq!(m.suggestions[0].value, "se");

    let result = engine.check("Il faut tout ce dire.").unwrap();
    let m = rule_match(&result, "CE_SE");
    assert_eq!((m.range.start, m.range.end), (13, 15));
    assert_eq!(m.suggestions[0].value, "se");

    let result = engine.check("Il soumis a rude épreuve.").unwrap();
    let m = rule_match(&result, "A_ACCENT");
    assert_eq!((m.range.start, m.range.end), (10, 11));
    assert_eq!(
        m.message,
        "La préposition <suggestion>à</suggestion> est attendue après \"soumis\"."
    );
    assert_eq!(m.suggestions[0].value, "à");
    // The raw_pos view keeps `soumis`'s tagger readings: the earlier
    // `NE_V` `<match>` action is a wrapper-replacing `REPLACE` in Java, so it
    // does not propagate to `getPreDisambigTokens()`. The later `PRONOM_VERB`
    // `remove` must therefore not be mirrored onto the pre view (D-191);
    // otherwise the `J.*`+`V ppa.*` `<and>` fails and `A_A_ACCENT` survives
    // the overlap filter instead.
    assert!(
        result.matches.iter().all(|m| m.rule_id != "A_A_ACCENT"),
        "A_A_ACCENT must not survive once the raw_pos A_ACCENT matches: {:#?}",
        result.matches
    );

    let result = engine
        .check("C'est trop, tu n'aurais pas du savoir !")
        .unwrap();
    let m = rule_match(&result, "DU_DU");
    assert_eq!((m.range.start, m.range.end), (28, 30));
    assert_eq!(
        m.message,
        "Le participe passé du verbe \"devoir\" comporte un accent circonflexe."
    );
    assert_eq!(m.suggestions[0].value, "dû");
}

#[test]
fn french_lettre_unique_empty_ref_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // LETTRE_UNIQUE `\2e \3` with the optional `ne` unmatched: Java's
    // `concatWithoutExtraSpace` pops the space before the `</suggestion>`
    // tag, so the first suggestion is `se` and the second (from `\3` alone
    // in the next `<suggestion>` block) is the empty string (Java probe:
    // 6..7 `se|` = `se`, ``).
    let result = engine.check("C'est s gorge.").unwrap();
    let m = rule_match(&result, "LETTRE_UNIQUE");
    assert_eq!((m.range.start, m.range.end), (6, 7));
    assert_eq!(m.message, "Cette lettre doit être supprimée.");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["se", ""]);
}

#[test]
fn french_rester_vppa_paragraph_end_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java probe (`probe-rester.sh "Elles restent contaminer." 1`):
    // RESTER_VPPA[1] (trailing `P.*`) matches although `.` only has
    // `M fin`/`SENT_END` readings, because `JLanguageTool.markAsParagraphEnd`
    // appends a `PARA_END` reading to the final token of the text. Rust kept
    // the paragraph end as a flag and reported subrule 2 (`SENT_END`
    // selector) instead.
    let result = engine.check("Elles restent contaminer.").unwrap();
    let m = rule_match(&result, "RESTER_VPPA");
    assert_eq!(m.sub_id.as_deref(), Some("1"));
    assert_eq!((m.range.start, m.range.end), (14, 24));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec!["contaminé", "contaminée", "contaminées", "contaminés"]
    );
}

#[test]
fn french_points_2_sentence_end_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java probe (`probe-rule.sh "Ils sont grands" POINTS_2`): subrule 3,
    // 9..15, `grands.|grands…`. The marker's `<and>` requires the final
    // token's SENT_END reading; RP-ETRE_ADJ_AMBIG rewrites `grands` to
    // `J m p` and Java's copy constructor re-adds SENT_END.
    let result = engine.check("Ils sont grands").unwrap();
    let m = rule_match(&result, "POINTS_2");
    assert_eq!(m.sub_id.as_deref(), Some("3"));
    assert_eq!((m.range.start, m.range.end), (9, 15));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["grands.", "grands…"]);
}

#[test]
fn french_pronsuj_empty_ref_message_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // `\1` references the empty SENT_START token; Java's `formatMatches`
    // routes a single empty match through `concatWithoutExtraSpace`, so
    // `« \1 »` renders with one space (corpus line 7,879).
    let result = engine.check("Elles repartir.").unwrap();
    let m = rule_match(&result, "PRONSUJ_NONVERBE");
    assert_eq!((m.range.start, m.range.end), (0, 14));
    assert_eq!(
        m.message,
        "Un verbe conjugué est généralement attendu après le pronom personnel sujet « »."
    );
    assert_eq!(m.suggestions[0].value, "Elles repartirent");
}

#[test]
fn french_verb_pronoun_original_error_suggestion_removed() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // `Aix les Milles-la Pioline` is an example of `VERB_PRONOUN[1]` but
    // subrule 14 matches; Java's `FindSuggestionsFilter` builds a new
    // RuleMatch and `definitiveReplacements.remove(originalErrorStr)`
    // removes the single rendered `Milles-la`, leaving no suggestions
    // (corpus line 10,356).
    let result = engine.check("Aix les Milles-la Pioline").unwrap();
    let m = rule_match(&result, "VERB_PRONOUN");
    assert_eq!(m.sub_id.as_deref(), Some("14"));
    assert!(m.suggestions.is_empty(), "suggestions: {:?}", m.suggestions);
}

#[test]
fn french_soft_hyphen_tokens_are_tagged_cleaned() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java `JLanguageTool.replaceSoftHyphens` strips the soft hyphen
    // (U+00AD) before tagging, so `massive`/`d'activité` are known words and
    // FR_SPELLING_RULE does not fire (Java probe + corpus lines 8,330/10,779).
    for text in [
        "Une attaque mas\u{00AD}sive.",
        "le revenu d'acti\u{00AD}vit\u{00E9}.",
    ] {
        let result = engine.check(text).unwrap();
        assert!(
            result
                .matches
                .iter()
                .all(|m| m.rule_id != "FR_SPELLING_RULE"),
            "{text}: {:#?}",
            result.matches
        );
    }
}

#[test]
fn french_multiword_elision_blocks_confusion_est_et() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // `n'importe quoi` is a `multiwords.txt` entry whose first space-delimited
    // word spans two tokens (`n'` + `importe`); Java's `MultiWordChunker`
    // concatenates the following non-whitespace tokens for the `mStartSpace`
    // lookup, so `n'` gets the `n'importe quoi:A` reading and
    // CONFUSION_EST_ET's `est ne …` subrule cannot fire. Java probe: only
    // ELISION 11..17 `d'est`.
    let result = engine.check("Le blocage de est n'importe quoi.").unwrap();
    let m = rule_match(&result, "ELISION");
    assert_eq!((m.range.start, m.range.end), (11, 17));
    assert_eq!(m.suggestions[0].value, "d'est");
    assert!(
        result
            .matches
            .iter()
            .all(|m| m.rule_id != "CONFUSION_EST_ET"),
        "matches: {:#?}",
        result.matches
    );
}

#[test]
fn french_postponed_adjective_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Les chats noir dorment.").unwrap();
    let m = rule_match(&result, "AGREEMENT_POSTPONED_ADJ");
    // Java: UTF-16 10..14 ("noir")
    assert_eq!((m.range.start, m.range.end), (10, 14));
    assert_eq!(
        m.message,
        "Vérifiez la concordance de «noir» avec les noms précédents."
    );
    assert_eq!(m.suggestions[0].value, "noirs");
}

#[test]
fn french_question_whitespace_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java (UTF-16): 11..19 and 20..22; the two guillemets are 2-byte UTF-8.
    let result = engine.check("Il a dit : «Bonjour !»").unwrap();
    let matches: Vec<&lt::Match> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "FRENCH_WHITESPACE")
        .collect();
    assert_eq!(matches.len(), 2, "matches: {:#?}", result.matches);
    assert_eq!((matches[0].range.start, matches[0].range.end), (11, 20));
    assert_eq!(
        matches[0].message,
        "Le guillemet ouvrant est suivi d'une espace insécable."
    );
    assert_eq!(matches[0].suggestions[0].value, "«\u{00a0}Bonjour");
    assert_eq!((matches[1].range.start, matches[1].range.end), (21, 24));
    assert_eq!(
        matches[1].message,
        "Le guillemet fermant est précédé d'une espace insécable."
    );
    assert_eq!(matches[1].suggestions[0].value, "!\u{00a0}»");
}

#[test]
fn french_question_whitespace_strict_matches_java() {
    let _guard = engine_guard();
    let Some(builder) = Engine::builder(Lang::Fr).ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let options = EngineOptions {
        enabled_rules: vec!["FRENCH_WHITESPACE_STRICT".to_string()],
        enabled_only: true,
        picky: true,
        ..Default::default()
    };
    let Some(engine) = builder.options(options).build().ok() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Il a dit : «Bonjour !»").unwrap();
    let matches: Vec<&lt::Match> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "FRENCH_WHITESPACE_STRICT")
        .collect();
    assert_eq!(matches.len(), 2);
    // Java (UTF-16): 8..10 and 19..21
    assert_eq!((matches[0].range.start, matches[0].range.end), (8, 10));
    assert_eq!(matches[0].suggestions[0].value, "\u{00a0}:");
    assert_eq!((matches[1].range.start, matches[1].range.end), (20, 22));
    assert_eq!(matches[1].suggestions[0].value, "\u{202f}!");
}

#[test]
fn french_compound_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Le Haut Rhin est beau.").unwrap();
    let m = rule_match(&result, "FR_COMPOUNDS_HAUT_RHIN");
    // Java: UTF-16 3..12
    assert_eq!((m.range.start, m.range.end), (3, 12));
    assert_eq!(m.message, "Écrivez avec un trait d’union.");
    assert_eq!(m.suggestions[0].value, "Haut-Rhin");
}

#[test]
fn french_simple_replace_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Il est parmis nous.").unwrap();
    let m = rule_match(&result, "FR_SIMPLE_REPLACE_SIMPLE_PARMIS");
    // Java: UTF-16 7..13
    assert_eq!((m.range.start, m.range.end), (7, 13));
    assert_eq!(m.message, "Vouliez-vous dire « parmi » ?");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["parmi", "partis", "permis", "Paris"]);
}

#[test]
fn french_duplicate_determiner_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("le le chat").unwrap();
    let m = rule_match(&result, "DUPLICATE_DETERMINER");
    // Java: UTF-16 0..10
    assert_eq!((m.range.start, m.range.end), (0, 10));
    assert_eq!(m.message, "Choisissez un déterminant.");
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec!["le chat", "la chatte", "les chats", "les chattes"]
    );
}

#[test]
fn french_comma_whitespace_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Bonjour , le monde .").unwrap();
    let matches: Vec<&lt::Match> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "COMMA_PARENTHESIS_WHITESPACE")
        .collect();
    assert_eq!(matches.len(), 2);
    assert_eq!((matches[0].range.start, matches[0].range.end), (7, 9));
    assert_eq!(
        matches[0].message,
        "Insérez une espace après la virgule et non avant."
    );
    assert_eq!(matches[0].suggestions[0].value, ",");
    assert_eq!((matches[1].range.start, matches[1].range.end), (18, 20));
    assert_eq!(matches[1].message, "Ne placez pas d'espace avant le point.");
    assert_eq!(matches[1].suggestions[0].value, ".");
}

#[test]
fn french_uppercase_and_unpaired_brackets_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("bonjour le monde.").unwrap();
    let m = rule_match(&result, "UPPERCASE_SENTENCE_START");
    assert_eq!((m.range.start, m.range.end), (0, 7));
    assert_eq!(m.message, "Cette phrase ne commence pas par une majuscule.");
    assert_eq!(m.suggestions[0].value, "Bonjour");

    let result = engine.check("(Bonjour le monde.").unwrap();
    let m = rule_match(&result, "UNPAIRED_BRACKETS");
    // Java: UTF-16 0..1
    assert_eq!((m.range.start, m.range.end), (0, 1));
    assert_eq!(
        m.message,
        "Pas de correspondance fermante ou ouvrante pour le caractère « ) »"
    );
}

#[test]
fn french_interrogative_verb_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine.check("Peut je venir ?").unwrap();
    let m = rule_match(&result, "ACCORD_V_QUESTION2");
    // Java: UTF-16 0..7
    assert_eq!((m.range.start, m.range.end), (0, 7));
    assert_eq!(
        m.message,
        "Vérifiez l’accord entre le verbe \"Peut\" le pronom \"je\"."
    );
    assert_eq!(m.suggestions[0].value, "Puis-je");
}

#[test]
fn french_repeated_words_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("Il est notamment grand. Elle est notamment belle.")
        .unwrap();
    let m = rule_match(&result, "FR_REPEATEDWORDS");
    // Java: UTF-16 33..42
    assert_eq!((m.range.start, m.range.end), (33, 42));
    assert_eq!(
        m.message,
        "Ce mot apparaît déjà dans l'une des phrases précédant immédiatement celle-ci. Utilisez un synonyme pour apporter plus de variété à votre texte, excepté si la répétition est intentionnelle."
    );
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(
        values,
        vec![
            "particulièrement",
            "spécialement",
            "singulièrement",
            "surtout",
            "spécifiquement"
        ]
    );
}

#[test]
fn french_text_level_rules_reject_clean_text() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let result = engine
        .check("Les fleurs sont belles et le chat dort tranquillement.")
        .unwrap();
    for forbidden in [
        "FR_SPELLING_RULE",
        "UNPAIRED_BRACKETS",
        "UPPERCASE_SENTENCE_START",
        "WHITESPACE_RULE",
        "SENTENCE_WHITESPACE",
    ] {
        assert!(
            !result.matches.iter().any(|m| m.rule_id == forbidden),
            "{forbidden} fired on clean text: {:#?}",
            result.matches
        );
    }
}

#[test]
fn french_pronom_personnel_case_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java probe (`probe-rule.sh`): `HP5. J'|HP5 j'|HP5, j'`. The third
    // suggestion's `<match case_conversion="alllower"/>` disables the
    // automatic all-uppercase adjustment for the whole out-of-message
    // suggestion list (`matchPreservesCase(suggestionMatchesOutMsg, ...)`
    // sees every suggestion's match).
    let result = engine
        .check("J'ai hâte de tester avec de l'APX 100, FP4 et HP5 J'aimais bien le LC29 mais il ")
        .unwrap();
    let m = rule_match(&result, "PRONOMS_PERSONNELS_MINUSCULE");
    assert_eq!(m.sub_id.as_deref(), Some("1"));
    // Java probe: UTF-16 46..52; the `â` in `hâte` makes the UTF-8 offset 47.
    assert_eq!((m.range.start, m.range.end), (47, 53));
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["HP5. J'", "HP5 j'", "HP5, j'"]);
}

#[test]
fn french_srx_two_line_breaks_match_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // Java probe (`scripts/oracle/fr/dump-sentences.sh`, pinned build):
    // single line breaks do not split, blank lines do, and the
    // `S.A.R.L.`/`M.` abbreviations stay attached. Java always tokenizes
    // with the `fr_two` cascade (SRXSentenceTokenizer disables
    // single-line-break paragraphs); the bare `fr` code dropped
    // `ByTwoLineBreaks` (same class of bug as the Italian `it_two` fix).
    let text = "La société S.A.R.L. Dupont a été créée en 1990 par M. Dupont.\n\
                Elle emploie 12 personnes.\n\n\
                Le siège est à Lyon.\n\
                Le chiffre d'affaires est en hausse depuis 2019.\n";
    let result = engine.check(text).unwrap();
    let sentences: Vec<&str> = result.sentences.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        sentences,
        vec![
            "La société S.A.R.L. Dupont a été créée en 1990 par M. Dupont.\n",
            "Elle emploie 12 personnes.\n\n",
            "Le siège est à Lyon.\n",
            "Le chiffre d'affaires est en hausse depuis 2019.\n",
        ]
    );
}

#[test]
fn french_ou_accent_on_where_matches_java() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    // The corpus line's "ou" means "where" and must be corrected to "où";
    // both engines report it (Java golden: OU[32] 14..16, suggestion "où").
    // The extra Java SUJET_AUXILIAIRE[1] match on this line is the documented
    // cross-rule mutation false positive (docs/differences.md #4) and is
    // intentionally absent.
    let result = engine
        .check("Je me demande ou je pourrais partir en vacances.")
        .unwrap();
    let m = rule_match(&result, "OU");
    assert_eq!(m.sub_id.as_deref(), Some("32"));
    assert_eq!((m.range.start, m.range.end), (14, 16));
    assert_eq!(
        m.message,
        "Le mot \"où\" indique un lieu, un temps ou une situation."
    );
    let values: Vec<&str> = m.suggestions.iter().map(|s| s.value.as_str()).collect();
    assert_eq!(values, vec!["où"]);
    assert!(
        !result
            .matches
            .iter()
            .any(|m| m.rule_id == "SUJET_AUXILIAIRE"),
        "SUJET_AUXILIAIRE is the documented Java mutation false positive (#4)"
    );
}
