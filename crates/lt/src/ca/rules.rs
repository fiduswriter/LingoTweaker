//! Catalan built-in rules that are not pattern/filter based
//! (`Catalan.getRelevantRules`): `IgnoreProperNouns` (33) and the ported
//! word-repeat family.

use lt_core::{AnalyzedSentence, Match, TextRange};

pub const IGNORE_PROPER_NOUNS_ID: &str = "IGNORE_PROPER_NOUNS";

/// `IgnoreProperNouns.match`: a token that is not tagged and whose exact
/// surface form was seen as a proper noun (`NP*`) earlier in the text is
/// reported as acceptable (message only, no suggestion). Text level.
pub fn ignore_proper_nouns(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut seen_proper_nouns: Vec<String> = Vec::new();
    for sentence in sentences {
        for token in &sentence.tokens {
            if token.has_pos_tag_starting_with("NP") {
                seen_proper_nouns.push(token.surface().to_string());
            }
            if !token.is_tagged && seen_proper_nouns.iter().any(|n| n == token.surface()) {
                rule_matches.push(Match::new(
                    IGNORE_PROPER_NOUNS_ID,
                    Option::<String>::None,
                    "Aquesta paraula ja aparegut abans i es pot donar per correcta.",
                    None::<String>,
                    TextRange::new(
                        sentence.offset + token.start_pos,
                        sentence.offset + token.end_pos(),
                    ),
                    Vec::new(),
                    "MISC",
                    "Miscel·lània",
                ));
            }
        }
    }
    rule_matches
}

// ---------------------------------------------------------------------------
// `CatalanWordRepeatRule` (8)
// ---------------------------------------------------------------------------

pub const WORD_REPEAT_ID: &str = "CATALAN_WORD_REPEAT_RULE";
pub const WORD_REPEAT_BEGINNING_ID: &str = "CATALAN_WORD_REPEAT_BEGINNING_RULE";

/// `WordRepeatRule.wordRepetitionOf` (case-sensitive).
fn repetition_of(word: &str, tokens: &[&lt_core::AnalyzedTokenReadings], position: usize) -> bool {
    position > 0 && tokens[position - 1].surface() == word && tokens[position].surface() == word
}

/// `WordRepeatRule.isWord`: no emoji, no numeric spaces, single chars must be
/// letters.
fn is_word_token(token: &str) -> bool {
    let mut chars = token.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.is_alphabetic(),
        (Some(_), Some(_)) => true,
        _ => false,
    }
}

/// `CatalanWordRepeatRule.ignore` (base name exceptions + `_allow_repeat`,
/// `LOC_ADV` and the two Catalan multiword lemmas).
fn catalan_word_repeat_ignore(tokens: &[&lt_core::AnalyzedTokenReadings], position: usize) -> bool {
    if position > 0
        && (tokens[position].has_pos_tag("_allow_repeat")
            || tokens[position - 1].has_pos_tag("_allow_repeat")
            || tokens[position].has_pos_tag("LOC_ADV")
            || tokens[position].has_lemma("Joan-Lluís Lluís")
            || tokens[position].has_lemma("Chitty Chitty Bang Bang"))
    {
        return true;
    }
    [
        "Phi", "Li", "Xiao", "Duran", "Wagga", "Abdullah", "Nwe", "Pago", "Cao",
    ]
    .iter()
    .any(|name| repetition_of(name, tokens, position))
}

/// `CatalanWordRepeatRule.match` over one sentence.
pub fn word_repeat_sentence(
    tokens: &[lt_core::AnalyzedTokenReadings],
    sentence_offset: usize,
) -> Vec<Match> {
    let view: Vec<&lt_core::AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut rule_matches = Vec::new();
    let mut prev_token = String::new();
    for i in 1..view.len() {
        let token = view[i].surface().to_string();
        if view[i].is_immunized {
            prev_token.clear();
            continue;
        }
        if is_word_token(&token)
            && prev_token.eq_ignore_ascii_case(&token)
            && !catalan_word_repeat_ignore(&view, i)
        {
            let prev_pos = view[i - 1].start_pos;
            let pos = view[i].start_pos;
            rule_matches.push(
                Match::new(
                    WORD_REPEAT_ID,
                    Option::<String>::None,
                    "Possible error: heu repetit una paraula",
                    Some("Repetició de paraules".to_string()),
                    TextRange::new(
                        sentence_offset + prev_pos,
                        sentence_offset + pos + prev_token.len(),
                    ),
                    vec![lt_core::Suggestion {
                        value: prev_token.clone(),
                        short_description: None,
                    }],
                    "MISC",
                    "Miscel·lània",
                )
                .with_metadata("Repetició de paraules", "duplication", 1),
            );
        }
        prev_token = token;
    }
    rule_matches
}

// ---------------------------------------------------------------------------
// `PhraseRepeatRule` (core) + `CatalanPhraseRepeatRule`
// ---------------------------------------------------------------------------

pub const PHRASE_REPEAT_ID: &str = "CATALAN_PHRASE_REPEAT_RULE";

/// Longest phrase length checked first, so a 3-word repetition isn't reported
/// twice as a shorter 2-word repetition (`PhraseRepeatRule` constants).
const MAX_PHRASE_LENGTH: usize = 3;
const MIN_PHRASE_LENGTH: usize = 2;

/// `CatalanPhraseRepeatRule.UNITATS_TEMPS` (from `ca/entities.ent`
/// `unitats_temps`).
const UNITATS_TEMPS: [&str; 23] = [
    "segon",
    "minut",
    "hora",
    "horeta",
    "dia",
    "jorn",
    "jornada",
    "setmana",
    "quinzena",
    "mes",
    "trimestre",
    "quadrimestre",
    "semestre",
    "any",
    "lustre",
    "dècada",
    "decenni",
    "segle",
    "mil·lenni",
    "mil·lenari",
    "minutet",
    "segonet",
    "anyet",
];

/// `CatalanPhraseRepeatRule.POSTAG_EXCEPTIONS`.
const POSTAG_EXCEPTIONS: [&str; 11] = [
    "_emoji_",
    "_PUNCT",
    "_PUNCT_CONT",
    "allow_repetition",
    "_allow_repeat",
    "LOC_ADJ",
    "LOC_ADV",
    "LOC_CONJ",
    "LOC_PREP",
    "SENT_START",
    "UNKNOWN",
];

/// `CatalanPhraseRepeatRule.CONJUNCIONS_I_O`.
const CONJUNCIONS_I_O: [&str; 2] = ["i", "o"];

/// Fixed 2-, 3- and 4-token antipatterns from the XML rule (as literal surface
/// sequences).
const LITERAL_ANTIPATTERNS: [&[&str]; 7] = [
    &["casa", "a", "casa", "a"],
    &["boca", "a", "boca", "a"],
    &["braç", "a", "braç", "a"],
    &["gen", "a", "gen", "a"],
    &["de", "tu", "a", "tu"],
    &["milions", "de"],
    &["res", "de", "res", "de"],
];

/// `PhraseRepeatRule.phraseRepeatedAt`: the `phraseLength` tokens starting at
/// `position` are immediately repeated (case-insensitively) and contain at
/// least one actual word.
fn phrase_repeated_at(
    tokens: &[&lt_core::AnalyzedTokenReadings],
    position: usize,
    phrase_length: usize,
) -> bool {
    let mut saw_word = false;
    for j in 0..phrase_length {
        let first = tokens[position + j];
        let second = tokens[position + phrase_length + j];
        if first.is_immunized || second.is_immunized {
            return false;
        }
        if !crate::wordutil::eq_ignore_case(first.surface(), second.surface()) {
            return false;
        }
        if crate::wordutil::is_word(first.surface()) {
            saw_word = true;
        }
    }
    saw_word
}

/// `PhraseRepeatRule.phraseToString`.
fn phrase_to_string(
    tokens: &[&lt_core::AnalyzedTokenReadings],
    position: usize,
    phrase_length: usize,
) -> String {
    let mut sb = String::new();
    for j in 0..phrase_length {
        if j > 0 && tokens[position + j].whitespace_before {
            sb.push(' ');
        }
        sb.push_str(tokens[position + j].surface());
    }
    sb
}

/// `CatalanPhraseRepeatRule.ignore` (antipatterns + pattern exceptions).
fn catalan_phrase_repeat_ignore(
    tokens: &[&lt_core::AnalyzedTokenReadings],
    position: usize,
    phrase_length: usize,
) -> bool {
    phrase_length == 2 && matches_antipattern(tokens, position, position + phrase_length)
        || violates_pattern_exceptions(tokens, position, phrase_length)
}

/// `CatalanPhraseRepeatRule.violatesPatternExceptions`: word1 must not be
/// punctuation/"i" nor SENT_START/allow_repetition/UNKNOWN/_PUNCT/_PUNCT_CONT/
/// _emoji_; word2 must additionally not be a LOC_ADV/LOC_ADJ/LOC_PREP/LOC_CONJ
/// locution; word3/word4 (the repeated pair) must not be such a locution
/// either.
fn violates_pattern_exceptions(
    tokens: &[&lt_core::AnalyzedTokenReadings],
    position: usize,
    phrase_length: usize,
) -> bool {
    for i in position..position + phrase_length {
        let first_token = tokens[i];
        let second_token = tokens[i + phrase_length];
        let token_str = first_token.surface().to_lowercase();
        if first_token.is_pos_tag_unknown {
            return true;
        }
        if CONJUNCIONS_I_O.contains(&token_str.as_str())
            || is_punct_symbol_or_apostrophe(&token_str)
        {
            return true;
        }
        for postag_exception in POSTAG_EXCEPTIONS {
            if first_token.has_pos_tag(postag_exception)
                || second_token.has_pos_tag(postag_exception)
            {
                return true;
            }
        }
    }
    false
}

/// `StringTools.isPunctuationOrSymbol` (`[\p{P}\p{S}']`, exactly one char).
fn is_punct_symbol_or_apostrophe(s: &str) -> bool {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^[\p{P}\p{S}']$").unwrap());
    RE.is_match(s)
}

/// `CatalanPhraseRepeatRule.matchesAntipattern`: fixed idioms ("boca a boca a",
/// "milions de", ...), the "ni massa poc(s)" and "de tu a tu" contexts, the
/// `&unitats_temps;` + "a" + repeat + "a" construction (e.g. "dia a dia a"),
/// and quotation marks immediately wrapping a single token. Antipatterns block
/// the rule wherever they overlap the current 2-word-phrase match.
fn matches_antipattern(
    tokens: &[&lt_core::AnalyzedTokenReadings],
    match_start: usize,
    match_end: usize,
) -> bool {
    let window_start = match_start.saturating_sub(3).max(1);
    let window_end = (tokens.len() - 1).min(match_end + 3);

    for antipattern in LITERAL_ANTIPATTERNS {
        let mut j = window_start;
        while j + antipattern.len() <= window_end + 1 {
            if sequence_matches(tokens, j, antipattern)
                && overlaps(j, j + antipattern.len() - 1, match_start, match_end)
            {
                return true;
            }
            j += 1;
        }
    }
    let mut j = window_start;
    while j + 3 <= window_end {
        // <token regexp="yes">&unitats_temps;</token> <token>a</token>
        // <match no="0"/> <token>a</token>
        if UNITATS_TEMPS.contains(&tokens[j].surface().to_lowercase().as_str())
            && crate::wordutil::eq_ignore_case(tokens[j + 1].surface(), "a")
            && crate::wordutil::eq_ignore_case(tokens[j].surface(), tokens[j + 2].surface())
            && crate::wordutil::eq_ignore_case(tokens[j + 3].surface(), "a")
            && overlaps(j, j + 3, match_start, match_end)
        {
            return true;
        }
        j += 1;
    }
    let mut j = window_start;
    while j + 2 <= window_end {
        // <token>ni</token> <token>massa</token> <token inflected="yes">poc</token>
        if crate::wordutil::eq_ignore_case(tokens[j].surface(), "ni")
            && crate::wordutil::eq_ignore_case(tokens[j + 1].surface(), "massa")
            && tokens[j + 2].has_lemma("poc")
            && overlaps(j, j + 2, match_start, match_end)
        {
            return true;
        }
        // <token postag="_QM_OPEN"/> <token spacebefore="no"/>
        // <token postag="_QM_CLOSE" spacebefore="no"/>
        if tokens[j].has_pos_tag("_QM_OPEN")
            && tokens[j + 2].has_pos_tag("_QM_CLOSE")
            && overlaps(j, j + 2, match_start, match_end)
        {
            return true;
        }
        j += 1;
    }
    false
}

/// `CatalanPhraseRepeatRule.sequenceMatches`.
fn sequence_matches(
    tokens: &[&lt_core::AnalyzedTokenReadings],
    start: usize,
    words: &[&str],
) -> bool {
    for (k, word) in words.iter().enumerate() {
        if !crate::wordutil::eq_ignore_case(tokens[start + k].surface(), word) {
            return false;
        }
    }
    true
}

/// `CatalanPhraseRepeatRule.overlaps`.
fn overlaps(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start <= b_end && b_start <= a_end
}

/// `CatalanPhraseRepeatRule.match` (the base `PhraseRepeatRule` scan over the
/// tokens without whitespace, starting at token 1 after SENT_START).
pub fn phrase_repeat_sentence(
    tokens: &[lt_core::AnalyzedTokenReadings],
    sentence_offset: usize,
) -> Vec<Match> {
    let view: Vec<&lt_core::AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut rule_matches = Vec::new();
    let mut i = 1;
    while i < view.len() {
        let mut matched_length = 0;
        // Check longer phrases first so we don't report a 3-word repetition as
        // a 2-word one
        for phrase_length in (MIN_PHRASE_LENGTH..=MAX_PHRASE_LENGTH).rev() {
            if i + 2 * phrase_length <= view.len()
                && phrase_repeated_at(&view, i, phrase_length)
                && !catalan_phrase_repeat_ignore(&view, i, phrase_length)
            {
                matched_length = phrase_length;
                break;
            }
        }
        if matched_length > 0 {
            let first_start = view[i].start_pos;
            let second_end = view[i + 2 * matched_length - 1].end_pos();
            let phrase = phrase_to_string(&view, i, matched_length);
            rule_matches.push(
                Match::new(
                    PHRASE_REPEAT_ID,
                    Option::<String>::None,
                    "Repetició de paraules (p. ex. «mà mà»)",
                    Some("Repetició de paraules".to_string()),
                    TextRange::new(sentence_offset + first_start, sentence_offset + second_end),
                    vec![lt_core::Suggestion {
                        value: phrase,
                        short_description: None,
                    }],
                    "MISC",
                    "Miscel·lània",
                )
                .with_metadata("Repetició de paraules", "duplication", 1),
            );
            // Skip past the whole repeated span to avoid overlapping matches
            i += 2 * matched_length;
        } else {
            i += 1;
        }
    }
    rule_matches
}

// ---------------------------------------------------------------------------
// `CatalanWordRepeatBeginningRule` (23)
// ---------------------------------------------------------------------------

/// `CatalanWordRepeatBeginningRule` expression sets.
// The four suggestion sets are Java `HashSet`s; the arrays keep the JVM
// bucket order (capacity 16) so the suggestion order matches Java.
const BEGINNING_ADD_ADVERBS: [&str; 3] = ["Igualment", "Addicionalment", "També"];
const BEGINNING_CONTRAST_CONJ: [&str; 3] = ["Però", "Mes", "Emperò"];
const BEGINNING_CAUSE_CONJ: [&str; 2] = ["Car", "Perquè"];
const BEGINNING_EMPHASIS_ADVERBS: [&str; 4] =
    ["Absolutament", "Clarament", "Òbviament", "Definitivament"];
const BEGINNING_EXPLAIN_ADVERBS: [&str; 4] = [
    "Particularment",
    "Precisament",
    "Específicament",
    "Concretament",
];
const BEGINNING_PERSONAL_PRONOUNS: [&str; 13] = [
    "jo",
    "tu",
    "ell",
    "ella",
    "nosaltres",
    "vosaltres",
    "ells",
    "elles",
    "vostè",
    "vostès",
    "vosté",
    "vostés",
    "vós",
];
const BEGINNING_ADD_EXPRESSIONS: [&str; 2] = ["Així mateix", "A més a més"];
const BEGINNING_CONTRAST_EXPRESSIONS: [&str; 3] = ["Així i tot", "D'altra banda", "Per altra part"];
const BEGINNING_CAUSE_EXPRESSIONS: [&str; 4] = ["Ja que", "Per tal com", "Pel fet que", "Puix que"];
const BEGINNING_EXCEPCIONS_START: [&str; 16] = [
    "l'", "el", "la", "els", "les", "punt", "article", "mòdul", "part", "sessió", "unitat", "tema",
    "a", "per", "en", "com",
];

fn beginning_is_adverb(token: &lt_core::AnalyzedTokenReadings) -> bool {
    if token.has_pos_tag("RG") || token.has_pos_tag("LOC_ADV") {
        return true;
    }
    let tok = token.surface();
    BEGINNING_ADD_ADVERBS.contains(&tok)
        || BEGINNING_CONTRAST_CONJ.contains(&tok)
        || BEGINNING_EMPHASIS_ADVERBS.contains(&tok)
        || BEGINNING_EXPLAIN_ADVERBS.contains(&tok)
        || BEGINNING_CAUSE_CONJ.contains(&tok)
}

fn beginning_is_exception(token: &str) -> bool {
    matches!(token, ":" | "–" | "-" | "✔️" | "➡️" | "—" | "⭐️" | "⚠️")
        || token.chars().next().is_some_and(|c| c.is_ascii_digit())
        || BEGINNING_EXCEPCIONS_START.contains(&token.to_lowercase().as_str())
}

fn different_adverbs_of_same_category(adverb: &str, category: &[&str]) -> Vec<String> {
    category
        .iter()
        .filter(|adv| **adv != adverb)
        .map(|s| (*s).to_string())
        .collect()
}

/// `CatalanWordRepeatBeginningRule.getSuggestions`.
fn beginning_suggestions(token: &lt_core::AnalyzedTokenReadings) -> Vec<String> {
    let tok = token.surface();
    let lower_tok = tok.to_lowercase();
    if BEGINNING_PERSONAL_PRONOUNS.contains(&lower_tok.as_str()) {
        return vec![
            format!("A més a més, {lower_tok}"),
            format!("Igualment, {lower_tok}"),
            format!("No sols aixó, sinó que {lower_tok}"),
        ];
    }
    if BEGINNING_ADD_ADVERBS.contains(&tok) {
        let mut suggestions = different_adverbs_of_same_category(tok, &BEGINNING_ADD_ADVERBS);
        suggestions.extend(BEGINNING_ADD_EXPRESSIONS.iter().map(|s| (*s).to_string()));
        return suggestions;
    }
    if BEGINNING_CONTRAST_CONJ.contains(&tok) {
        return BEGINNING_CONTRAST_EXPRESSIONS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
    }
    if BEGINNING_EMPHASIS_ADVERBS.contains(&tok) {
        return different_adverbs_of_same_category(tok, &BEGINNING_EMPHASIS_ADVERBS);
    }
    if BEGINNING_EXPLAIN_ADVERBS.contains(&tok) {
        return different_adverbs_of_same_category(tok, &BEGINNING_EXPLAIN_ADVERBS);
    }
    if BEGINNING_CAUSE_CONJ.contains(&tok) {
        let mut suggestions = different_adverbs_of_same_category(tok, &BEGINNING_CAUSE_CONJ);
        suggestions.extend(BEGINNING_CAUSE_EXPRESSIONS.iter().map(|s| (*s).to_string()));
        return suggestions;
    }
    Vec::new()
}

/// `CatalanWordRepeatBeginningRule.match` (text level, `tags="picky"`).
pub fn word_repeat_beginning(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut last_token = String::new();
    let mut before_last_token = String::new();
    let mut prev_sentence: Option<&AnalyzedSentence> = None;
    for sentence in sentences {
        let tokens: Vec<&lt_core::AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut token = String::new();
        if tokens.len() > 1 {
            let analyzed_token = tokens[1];
            token = analyzed_token.surface().to_string();
            if tokens.len() > 3 {
                let mut is_word = true;
                if token.chars().count() == 1 {
                    is_word = token.chars().next().is_some_and(char::is_alphabetic);
                }
                let trimmed_like_sentence =
                    prev_sentence.is_some_and(|p| regex_sentence_end().is_match(p.text.trim()));
                if is_word
                    && last_token == token
                    && !beginning_is_exception(&token)
                    && !beginning_is_exception(tokens[2].surface())
                    && !beginning_is_exception(tokens[3].surface())
                    && trimmed_like_sentence
                {
                    let short_msg = if beginning_is_adverb(analyzed_token) {
                        "Dues frases consecutives comencen amb el mateix element."
                    } else if before_last_token == token {
                        "Tres frases successives comencen amb la mateixa paraula."
                    } else {
                        ""
                    };
                    if !short_msg.is_empty() {
                        let msg = format!(
                            "{short_msg} Considereu reescriure la frase o usar un sinònim."
                        );
                        let start_pos = analyzed_token.start_pos;
                        let end_pos = start_pos + token.len();
                        let suggestions = beginning_suggestions(analyzed_token);
                        let mut m = Match::new(
                            WORD_REPEAT_BEGINNING_ID,
                            Option::<String>::None,
                            msg,
                            Some(short_msg.to_string()),
                            TextRange::new(sentence.offset + start_pos, sentence.offset + end_pos),
                            suggestions
                                .into_iter()
                                .map(|value| lt_core::Suggestion {
                                    value,
                                    short_description: None,
                                })
                                .collect(),
                            "REPETITIONS_STYLE",
                            "Repeticions (estil)",
                        )
                        .with_metadata(
                            "Frases successives que comencen amb la mateixa paraula",
                            "style",
                            0,
                        )
                        .with_match_type("Other");
                        m = m.with_picky(true);
                        rule_matches.push(m);
                    }
                }
            }
        }
        before_last_token = last_token;
        last_token = token;
        prev_sentence = Some(sentence);
    }
    rule_matches
}

fn regex_sentence_end() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^.+[.?!]$").unwrap());
    &RE
}

// ---------------------------------------------------------------------------
// `PronomFebleDuplicateRule` (20)
// ---------------------------------------------------------------------------

pub const PRONOM_FEBLE_DUPLICATE_ID: &str = "PRONOMS_FEBLES_DUPLICATS";

fn pronom_feble_lemma(token: &lt_core::AnalyzedTokenReadings) -> String {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^(?:P0.{6}|PP3CN000|PP3NN000|PP3..A00|PP[123]CP000|PP3CSD00)$").unwrap()
    });
    for reading in &token.readings {
        let tag = reading.pos_tag.as_deref().unwrap_or("UNKNOWN");
        if RE.is_match(tag) {
            return reading.lemma().to_string();
        }
    }
    String::new()
}

fn suggestion_from_to(
    tokens: &[&lt_core::AnalyzedTokenReadings],
    from: usize,
    to: usize,
) -> String {
    let mut sugg = String::new();
    for token in tokens.iter().take(to).skip(from) {
        if token.whitespace_before && !sugg.is_empty() {
            sugg.push(' ');
        }
        sugg.push_str(token.surface());
    }
    sugg
}

/// `PronomFebleDuplicateRule.isException`.
fn pronom_exception(tokens: &[&lt_core::AnalyzedTokenReadings], i: usize) -> bool {
    if (crate::ca::helpers::has_partial_pos_tag(tokens[i], "VMN0000")
        || crate::ca::helpers::has_partial_pos_tag(tokens[i], "VSN0000")
        || crate::ca::helpers::has_partial_pos_tag(tokens[i], "VAN0000"))
        && tokens[i - 1].has_lemma("fer")
    {
        // et fan adonar-te --> rule EL_FAN_AGENOLLAR
        return true;
    }
    if tokens[i].surface() == "poder" && tokens[i - 1].has_pos_tag_starting_with("V") {
        return true;
    }
    if i > 3
        && i + 3 < tokens.len()
        && (tokens[i].surface() == "ha" || tokens[i].surface() == "havia")
        && tokens[i - 2].surface() == "que"
        && tokens[i - 1].surface() == "hi"
        && tokens[i + 1].has_lemma("de")
        && !(tokens[i + 2].has_lemma("haver") && tokens[i + 3].has_lemma("hi"))
    {
        return true;
    }
    false
}

/// `PronomFebleDuplicateRule.isThereErrorInLemmas`; `corrected_pronouns` is
/// the mutable `correctedPronouns` field of the Java rule.
fn pronom_error_in_lemmas(
    lemes_pronoms_abans: &[String],
    lemes_pronoms_despres: &[String],
    lemes_preposicions: &[String],
    tokens: &[&lt_core::AnalyzedTokenReadings],
    last_verb_pos: isize,
    corrected_pronouns: &mut Option<String>,
) -> bool {
    *corrected_pronouns = None;
    if lemes_pronoms_abans.is_empty() || lemes_pronoms_despres.is_empty() {
        return false;
    }
    if lemes_pronoms_abans.len() == 1
        && lemes_pronoms_despres.len() == 1
        && lemes_pronoms_abans[0] == lemes_pronoms_despres[0]
    {
        return true;
    }
    if lemes_pronoms_abans.len() > 1 && lemes_pronoms_despres.len() > 1 {
        return true;
    }
    let last_verb = tokens[last_verb_pos as usize].surface();
    let contains = |list: &[String], value: &str| list.iter().any(|v| v == value);
    if (last_verb == "haver" || last_verb == "havent")
        && ["en", "hi"].contains(&lemes_pronoms_despres[0].as_str())
        && ["en", "hi"].contains(&lemes_pronoms_abans[0].as_str())
    {
        *corrected_pronouns = Some("n'hi".to_string());
        return true;
    }
    if contains(lemes_pronoms_abans, "en") && contains(lemes_pronoms_despres, "en") {
        return true;
    }
    if contains(lemes_pronoms_abans, "ell") && contains(lemes_pronoms_despres, "ell") {
        return true;
    }
    if contains(lemes_pronoms_abans, "ho")
        && (contains(lemes_pronoms_despres, "hi")
            || contains(lemes_pronoms_despres, "ell")
            || contains(lemes_pronoms_despres, "en"))
    {
        return true;
    }
    if contains(lemes_pronoms_abans, "hi")
        && !contains(lemes_preposicions, "a")
        && (contains(lemes_pronoms_despres, "ho")
            || contains(lemes_pronoms_despres, "ell")
            || contains(lemes_pronoms_despres, "en"))
    {
        return true;
    }
    false
}

/// `PronomFebleDuplicateRule.match` over one sentence.
pub fn pronom_feble_duplicate(
    tokens: &[lt_core::AnalyzedTokenReadings],
    sentence_offset: usize,
) -> Vec<Match> {
    static VERB_CONJUGAT: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^(?:V.[SI].*)$").unwrap());
    let non_blank: Vec<&lt_core::AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut rule_matches = Vec::new();
    let mut init_pos: isize = -1;
    let mut last_verb_pos: isize = -1;
    let mut lemes_pronoms_abans: Vec<String> = Vec::new();
    let mut lemes_pronoms_despres: Vec<String> = Vec::new();
    let mut lemes_preposicions: Vec<String> = Vec::new();
    let mut count_verb = 0usize;
    let mut in_verb_group = false;
    let mut i = 1usize;
    while i < non_blank.len() {
        let pf_lemma = pronom_feble_lemma(non_blank[i]);
        if !pf_lemma.is_empty() {
            if count_verb == 0
                && (!lemes_pronoms_abans.is_empty()
                    || non_blank[i].whitespace_before
                    || non_blank[i - 1].has_pos_tag("SENT_START"))
            {
                lemes_pronoms_abans.push(pf_lemma);
                if lemes_pronoms_abans.len() == 1 {
                    init_pos = i as isize;
                }
                in_verb_group = true;
            } else if !non_blank[i].whitespace_before {
                lemes_pronoms_despres.push(pf_lemma);
            } else {
                count_verb = 0;
                lemes_pronoms_abans.clear();
                lemes_pronoms_despres.clear();
                lemes_pronoms_abans.push(pf_lemma);
                init_pos = i as isize;
                in_verb_group = true;
            }
        } else if non_blank[i].chunk_tags.iter().any(|c| c == "GV")
            && !lemes_pronoms_abans.is_empty()
            && lemes_pronoms_despres.is_empty()
            && !pronom_exception(&non_blank, i)
        {
            let is_conjugated = non_blank[i].readings.iter().any(|r| {
                r.pos_tag
                    .as_deref()
                    .is_some_and(|t| VERB_CONJUGAT.is_match(t))
            });
            if is_conjugated && count_verb > 0 {
                in_verb_group = false;
            } else {
                count_verb += 1;
                in_verb_group = true;
                last_verb_pos = i as isize;
                if let Some(prep) =
                    crate::ca::helpers::reading_with_tag_regex(non_blank[i], "SPS00")
                {
                    lemes_preposicions.push(prep.lemma().to_string());
                }
            }
        } else {
            in_verb_group = false;
        }
        if !in_verb_group || i == non_blank.len() - 1 {
            let mut corrected_pronouns: Option<String> = None;
            if pronom_error_in_lemmas(
                &lemes_pronoms_abans,
                &lemes_pronoms_despres,
                &lemes_preposicions,
                &non_blank,
                last_verb_pos,
                &mut corrected_pronouns,
            ) {
                let end_index = if in_verb_group && i == non_blank.len() - 1 {
                    i + 1
                } else {
                    i
                };
                let count = count_verb;
                let mut replacements: Vec<String> = Vec::new();
                let casing_model = non_blank[init_pos as usize].surface();
                match &corrected_pronouns {
                    None => {
                        let first = suggestion_from_to(
                            &non_blank,
                            (init_pos as usize) + lemes_pronoms_abans.len(),
                            (init_pos as usize)
                                + lemes_pronoms_abans.len()
                                + count
                                + lemes_pronoms_despres.len(),
                        );
                        replacements.push(crate::ca::adapt::preserve_case(&first, casing_model));
                        let second = suggestion_from_to(
                            &non_blank,
                            init_pos as usize,
                            (init_pos as usize) + lemes_pronoms_abans.len() + count,
                        );
                        replacements.push(crate::ca::adapt::preserve_case(&second, casing_model));
                    }
                    Some(corrected) => {
                        let verbs = suggestion_from_to(
                            &non_blank,
                            (init_pos as usize) + lemes_pronoms_abans.len(),
                            (init_pos as usize) + lemes_pronoms_abans.len() + count,
                        );
                        let before = format!("{corrected} {verbs}");
                        replacements.push(crate::ca::adapt::preserve_case(&before, casing_model));
                        let pronoms_darrere =
                            crate::ca::helpers::transform_darrere(corrected, &verbs);
                        let after = format!("{verbs}{pronoms_darrere}");
                        replacements.push(crate::ca::adapt::preserve_case(&after, casing_model));
                    }
                }
                rule_matches.push(
                    Match::new(
                        PRONOM_FEBLE_DUPLICATE_ID,
                        Option::<String>::None,
                        "Combinació incorrecta de pronoms febles. Deixeu els de davant o els de darrere del verb.",
                        Some("Combinació incorrecta de pronoms febles.".to_string()),
                        TextRange::new(
                            sentence_offset + non_blank[init_pos as usize].start_pos,
                            sentence_offset + non_blank[end_index - 1].end_pos(),
                        ),
                        replacements
                            .into_iter()
                            .map(|value| lt_core::Suggestion {
                                value,
                                short_description: None,
                            })
                            .collect(),
                        "PRONOMS_FEBLES",
                        "Pronoms febles",
                    )
                    .with_metadata("Pronoms febles duplicats", "grammar", 0)
                    .with_match_type("Other"),
                );
            }
            count_verb = 0;
            lemes_pronoms_abans.clear();
            lemes_pronoms_despres.clear();
            lemes_preposicions.clear();
        }
        i += 1;
    }
    rule_matches
}

// ---------------------------------------------------------------------------
// `CatalanUnpairedQuestionMarksRule` (10) / `...ExclamationMarksRule` (11)
// ---------------------------------------------------------------------------

/// Both unpaired-mark rules (default off; text level).
pub fn unpaired_marks(
    sentences: &[AnalyzedSentence],
    rule_id: &str,
    start_symbol: &str,
    end_symbol: &str,
) -> Vec<Match> {
    let mut matches = Vec::new();
    for sentence in sentences {
        let tokens: Vec<&lt_core::AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut needs_inv_at: isize = -1;
        let mut j = tokens.len() as isize - 1;
        while j > 0 {
            if tokens[j as usize].surface() == end_symbol {
                let next = tokens.get(j as usize + 1);
                let joined_to_next = next.is_some_and(|t| {
                    !t.whitespace_before
                        && !crate::wordutil::is_punctuation_mark(t.surface())
                        && !t.is_whitespace
                });
                if !joined_to_next {
                    needs_inv_at = j;
                    break;
                }
            }
            j -= 1;
        }
        if needs_inv_at > 1 {
            let mut has_inv_mark = false;
            let mut first_token: Option<&lt_core::AnalyzedTokenReadings> = None;
            for (i, token) in tokens.iter().enumerate() {
                if first_token.is_none()
                    && !token.is_sentence_start
                    && !crate::wordutil::is_punctuation_mark(token.surface())
                {
                    first_token = Some(token);
                }
                if token.surface() == start_symbol && (i as isize) < needs_inv_at {
                    has_inv_mark = true;
                }
                if !token.is_sentence_end
                    && token.surface() == end_symbol
                    && (i as isize) < needs_inv_at
                {
                    first_token = None;
                }
                if i > 2 && i + 2 < tokens.len() {
                    let prev_is_comma = tokens[i - 1].surface() == ",";
                    if prev_is_comma
                        && tokens[i].has_pos_tag("CC")
                        && tokens[i + 1].has_pos_tag("SPS00")
                        && (tokens[i + 2].has_pos_tag_starting_with("PT")
                            || tokens[i + 2].has_pos_tag_starting_with("DT"))
                    {
                        first_token = Some(tokens[i]);
                    }
                    if prev_is_comma
                        && tokens[i].has_pos_tag("SPS00")
                        && (tokens[i + 1].has_pos_tag_starting_with("PT")
                            || tokens[i + 1].has_pos_tag_starting_with("DT"))
                    {
                        first_token = Some(tokens[i]);
                    }
                    if prev_is_comma
                        && tokens[i].has_pos_tag("CC")
                        && (tokens[i + 1].has_pos_tag_starting_with("PT")
                            || tokens[i + 1].has_pos_tag_starting_with("DT"))
                    {
                        first_token = Some(tokens[i]);
                    }
                    if prev_is_comma
                        && (tokens[i].has_pos_tag_starting_with("PT")
                            || tokens[i].has_pos_tag_starting_with("DT"))
                    {
                        first_token = Some(tokens[i]);
                    }
                    if prev_is_comma
                        && tokens[i].has_pos_tag("CC")
                        && (tokens[i + 1].surface() == "no" || tokens[i + 1].surface() == "sí")
                    {
                        first_token = Some(tokens[i]);
                    }
                }
                if i > 2
                    && tokens[i - 1].surface() == ","
                    && matches!(tokens[i].surface(), "no" | "sí" | "oi" | "eh")
                {
                    first_token = Some(tokens[i]);
                }
            }
            if let Some(first) = first_token {
                if !has_inv_mark {
                    matches.push(Match::new(
                        rule_id,
                        Option::<String>::None,
                        format!("Símbol sense parella: Sembla que falta un '{start_symbol}'"),
                        None::<String>,
                        TextRange::new(
                            sentence.offset + first.start_pos,
                            sentence.offset + first.end_pos(),
                        ),
                        vec![lt_core::Suggestion {
                            value: format!("{start_symbol}{}", first.surface()),
                            short_description: None,
                        }],
                        "MISC",
                        "Miscel·lània",
                    ));
                }
            }
        }
    }
    matches
}
