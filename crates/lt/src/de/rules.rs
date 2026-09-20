//! Java-coded German rules (ported one by one from
//! `languagetool-language-modules/de/.../rules/de/`). Each function follows
//! the Java class's metadata (id, category, default on/off, messages).

use lt_core::{AnalyzedSentence, Match, Suggestion, TextRange};

/// `DuUpperLowerCaseRule.lowerWords`.
const DU_LOWER_WORDS: &[&str] = &[
    "du", "dir", "dich", "dein", "deine", "deines", "deins", "deiner", "deinen", "deinem", "euch",
    "euer", "eure", "euere", "euren", "eueren", "euern", "eurer", "euerer", "eurem", "euerem",
    "eures", "eueres",
];

/// Tokens after which a lowercase `du` is not reported (Java's
/// `StringUtils.equalsAny(tokens[i-1].getToken(), ...)` list plus the
/// sentence-start check).
const DU_PREV_TOKENS: &[&str] = &[
    "\"", "„", "‚", ":", "»", "«", "“", "-", "–", "*", "•", "\u{2063}", "\u{25E6}", "\u{00B7}",
];

/// `org.languagetool.rules.de.DuUpperLowerCaseRule` (TextLevelRule,
/// category CASING).
pub fn du_upper_lower(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut first_use: Option<String> = None;
    let mut matches = Vec::new();
    for sentence in sentences {
        let tokens = sentence.tokens_without_whitespace();
        for (i, token) in tokens.iter().enumerate() {
            if i > 0 {
                let prev = tokens[i - 1];
                if prev.is_sentence_start || DU_PREV_TOKENS.contains(&prev.surface()) {
                    continue;
                }
            }
            let word = token.surface().to_string();
            let lc_word = word.to_lowercase();
            if !DU_LOWER_WORDS.contains(&lc_word.as_str()) {
                continue;
            }
            match &first_use {
                None => first_use = Some(word),
                Some(first) => {
                    let first_upper = first.chars().next().is_some_and(char::is_uppercase);
                    let word_upper = word.chars().next().is_some_and(char::is_uppercase);
                    let all_upper = lt_tagger::is_all_uppercase(&word);
                    let (msg, replacement) = if first_upper && !word_upper {
                        let replacement = lt_tagger::uppercase_first_char(&word);
                        (
                            format!(
                                "Vorher wurde bereits '{first}' großgeschrieben. \
                                 Aus Gründen der Einheitlichkeit '{replacement}' hier auch großschreiben?"
                            ),
                            replacement,
                        )
                    } else if !first_upper && word_upper && !all_upper {
                        let replacement = lt_tagger::lowercase_first_char(&word);
                        (
                            format!(
                                "Vorher wurde bereits '{first}' kleingeschrieben. \
                                 Aus Gründen der Einheitlichkeit '{replacement}' hier auch kleinschreiben?"
                            ),
                            replacement,
                        )
                    } else {
                        continue;
                    };
                    matches.push(
                        Match::new(
                            "DE_DU_UPPER_LOWER",
                            Option::<String>::None,
                            msg,
                            None,
                            TextRange::new(
                                sentence.offset + token.start_pos,
                                sentence.offset + token.end_pos(),
                            ),
                            vec![Suggestion {
                                value: replacement,
                                short_description: None,
                            }],
                            "CASING",
                            "Groß-/Kleinschreibung",
                        )
                        .with_metadata(
                            "Einheitliche Verwendung von Du/du, Dir/dir etc.",
                            "style",
                            0,
                        )
                        .with_match_type("Other"),
                    );
                }
            }
        }
    }
    matches
}

/// `org.languagetool.rules.de.SimilarNameRule` (TextLevelRule, category
/// TYPOS, default off).
pub fn similar_names(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const MIN_LENGTH: usize = 4;
    const MAX_DIFF: usize = 1;
    let excluded = [
        "Dein", "Deine", "Deinen", "Deiner", "Deines", "Deinem", "Ihr", "Ihre", "Ihren", "Ihrer",
        "Ihres", "Ihrem",
    ];
    let mut names_so_far: Vec<String> = Vec::new();
    let mut matches = Vec::new();
    for sentence in sentences {
        for token in sentence.tokens_without_whitespace() {
            let word = token.surface().to_string();
            let is_maybe_name = word.chars().count() >= MIN_LENGTH
                && ((token
                    .readings
                    .iter()
                    .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains("EIG:")))
                    && !token
                        .readings
                        .iter()
                        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains(":COU"))))
                    || !token.is_tagged)
                && !excluded.contains(&word.as_str());
            if !(is_maybe_name && word.chars().next().is_some_and(char::is_uppercase)) {
                continue;
            }
            let mut similar: Option<String> = None;
            for name in &names_so_far {
                if name == &word {
                    continue;
                }
                let len_diff = name.chars().count().abs_diff(word.chars().count());
                let name_s = name.ends_with('s') && !word.ends_with('s');
                let other_s = !name.ends_with('s') && word.ends_with('s');
                let name_n = name.ends_with('n') && !word.ends_with('n');
                let other_n = !name.ends_with('n') && word.ends_with('n');
                if name_s || other_s || name_n || other_n {
                    continue;
                }
                if len_diff <= MAX_DIFF && super::util::levenshtein(name, &word) <= MAX_DIFF {
                    similar = Some(name.clone());
                    break;
                }
            }
            if let Some(similar) = similar {
                let msg = format!(
                    "'{word}' ähnelt dem vorher benutzten '{similar}', handelt es sich evtl. um einen Tippfehler?"
                );
                matches.push(
                    Match::new(
                        "DE_SIMILAR_NAMES",
                        Option::<String>::None,
                        msg,
                        None,
                        TextRange::new(
                            sentence.offset + token.start_pos,
                            sentence.offset + token.end_pos(),
                        ),
                        vec![Suggestion {
                            value: similar,
                            short_description: None,
                        }],
                        "TYPOS",
                        "Mögliche Tippfehler",
                    )
                    .with_metadata(
                        "Mögliche Tippfehler in Namen finden",
                        "misspelling",
                        0,
                    ),
                );
            }
            names_so_far.push(word);
        }
    }
    matches
}

/// `org.languagetool.rules.de.WiederVsWiderRule` (Rule, category TYPOS).
pub fn wieder_vs_wider(sentence: &AnalyzedSentence, start: usize) -> Vec<Match> {
    let tokens = sentence.tokens_without_whitespace();
    let mut matches = Vec::new();
    let mut found_spiegelt = false;
    let mut found_wieder = false;
    let mut found_wider = false;
    for (i, token) in tokens.iter().enumerate() {
        let text = token.surface().to_string();
        if token.has_lemma("spiegeln") {
            found_spiegelt = true;
        } else if text.eq_ignore_ascii_case("wieder") && found_spiegelt {
            found_wieder = true;
        } else if text.eq_ignore_ascii_case("wider") && found_spiegelt {
            found_wider = true;
        }
        let later_wider = tokens.get(i + 1).map(|t| t.surface()) == Some("wider")
            || tokens.get(i + 2).map(|t| t.surface()) == Some("wider");
        if found_spiegelt && found_wieder && !found_wider && !later_wider {
            let msg = "'wider' in 'widerspiegeln' wird mit 'i' statt mit 'ie' \
                       geschrieben, z.B. 'Das spiegelt die Situation gut wider.'";
            let short_msg = "'wider' in 'widerspiegeln' wird mit 'i' geschrieben";
            let pos = token.start_pos;
            matches.push(
                Match::new(
                    "DE_WIEDER_VS_WIDER",
                    Option::<String>::None,
                    msg,
                    Some(short_msg.to_string()),
                    TextRange::new(start + pos, start + pos + text.len()),
                    vec![Suggestion {
                        value: "wider".to_string(),
                        short_description: None,
                    }],
                    "TYPOS",
                    "Mögliche Tippfehler",
                )
                .with_metadata(
                    "Möglicher Tippfehler 'spiegeln ... wieder (wider)'",
                    "misspelling",
                    0,
                ),
            );
            found_spiegelt = false;
            found_wieder = false;
            found_wider = false;
        }
    }
    matches
}

// ---------------------------------------------------------------------------
// `SentenceWhitespaceRule` (German) — TextLevelRule, category MISC,
// issue type Whitespace. Java's constructor uses `maxSpacesBetweenSentences
// = 1` (the English rule passes 2).
// ---------------------------------------------------------------------------

const DE_SENTENCE_WHITESPACE_ID: &str = "DE_SENTENCE_WHITESPACE";
const DE_SW_MAX_SPACES: usize = 1;
const DE_SW_REPEATED: &str = "Möglicher Tippfehler: mehr als ein Leerzeichen hintereinander";

fn is_only_spaces(token: &str) -> bool {
    token.chars().all(|c| c == ' ')
}

fn is_space_token(token: &lt_core::AnalyzedTokenReadings) -> bool {
    is_only_spaces(token.surface())
}

fn is_sentence_end_token(token: &str) -> bool {
    matches!(token, "." | "!" | "?")
}

fn is_line_break_token(token: &lt_core::AnalyzedTokenReadings) -> bool {
    let s = token.surface();
    matches!(s, "\n" | "\r\n" | "\r" | "\n\r") || s.contains('\n') || s.contains('\r')
}

fn follows_sentence_end(tokens: &[lt_core::AnalyzedTokenReadings], whitespace_pos: usize) -> bool {
    let mut i = whitespace_pos;
    while i > 1 {
        i -= 1;
        if !tokens[i].is_whitespace {
            return is_sentence_end_token(tokens[i].surface());
        }
    }
    false
}

fn get_whitespace_length(
    tokens: &[lt_core::AnalyzedTokenReadings],
    from: usize,
    to: usize,
) -> usize {
    (from..=to).map(|i| tokens[i].surface().len()).sum()
}

fn get_leading_spaces_length(tokens: &[lt_core::AnalyzedTokenReadings]) -> usize {
    let mut length = 0;
    let mut i = 1;
    while i < tokens.len() && is_space_token(&tokens[i]) {
        length += tokens[i].surface().len();
        i += 1;
    }
    length
}

fn has_text_after_leading_spaces(
    tokens: &[lt_core::AnalyzedTokenReadings],
    leading_spaces_length: usize,
) -> bool {
    let mut pos = 1;
    let mut spaces_length = 0;
    while pos < tokens.len() && spaces_length < leading_spaces_length {
        spaces_length += tokens[pos].surface().len();
        pos += 1;
    }
    pos < tokens.len() && !tokens[pos].is_whitespace && !is_line_break_token(&tokens[pos])
}

fn starts_with_line_break(tokens: &[lt_core::AnalyzedTokenReadings]) -> bool {
    tokens.len() > 1 && is_line_break_token(&tokens[1])
}

/// `org.languagetool.rules.de.SentenceWhitespaceRule` (TextLevelRule).
pub fn sentence_whitespace(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut is_first_sentence = true;
    let mut prev_sentence_ending_whitespace = String::new();
    let mut prev_sentence_ends_with_line_break = false;
    let mut prev_sentence_ends_with_number = false;
    for sentence in sentences {
        let tokens = &sentence.tokens;
        // `addRepeatedWhitespaceMatches`
        let mut i = 1usize;
        while i < tokens.len() {
            if is_space_token(&tokens[i]) && follows_sentence_end(tokens, i) {
                let first_whitespace = i;
                let mut last_whitespace = i;
                i += 1;
                while i < tokens.len() && is_space_token(&tokens[i]) {
                    last_whitespace = i;
                    i += 1;
                }
                if get_whitespace_length(tokens, first_whitespace, last_whitespace)
                    > DE_SW_MAX_SPACES
                    && last_whitespace + 1 < tokens.len()
                    && !tokens[last_whitespace + 1].is_whitespace
                    && !is_line_break_token(&tokens[last_whitespace + 1])
                {
                    rule_matches.push(sw_match(
                        TextRange::new(
                            sentence.offset + tokens[first_whitespace].start_pos,
                            sentence.offset + tokens[last_whitespace].end_pos(),
                        ),
                        DE_SW_REPEATED,
                    ));
                }
            } else {
                i += 1;
            }
        }
        if is_first_sentence {
            is_first_sentence = false;
        } else if !prev_sentence_ends_with_line_break && !starts_with_line_break(tokens) {
            let leading_spaces_length = get_leading_spaces_length(tokens);
            if is_only_spaces(&prev_sentence_ending_whitespace)
                && prev_sentence_ending_whitespace.len() + leading_spaces_length > DE_SW_MAX_SPACES
                && (!prev_sentence_ending_whitespace.is_empty() || leading_spaces_length > 0)
                && has_text_after_leading_spaces(tokens, leading_spaces_length)
            {
                let start = sentence
                    .offset
                    .saturating_sub(prev_sentence_ending_whitespace.len());
                rule_matches.push(sw_match(
                    TextRange::new(start, sentence.offset + leading_spaces_length),
                    DE_SW_REPEATED,
                ));
            } else if prev_sentence_ending_whitespace.is_empty() && tokens.len() > 1 {
                let first_token = tokens[1].surface().to_string();
                let msg = if prev_sentence_ends_with_number {
                    "Fügen Sie nach Ordnungszahlen (1., 2. usw.) ein Leerzeichen ein."
                } else {
                    "Fügen Sie zwischen Sätzen ein Leerzeichen ein."
                };
                let mut m = sw_match(
                    TextRange::new(sentence.offset, sentence.offset + first_token.len()),
                    msg,
                );
                m.suggestions = vec![Suggestion {
                    value: format!(" {first_token}"),
                    short_description: None,
                }];
                rule_matches.push(m);
            }
        }
        if !tokens.is_empty() {
            let last_token = tokens[tokens.len() - 1].surface();
            prev_sentence_ending_whitespace =
                if last_token.replace('\u{00A0}', " ").trim().is_empty() {
                    last_token.to_string()
                } else {
                    String::new()
                };
            prev_sentence_ends_with_line_break = is_line_break_token(&tokens[tokens.len() - 1]);
        }
        if tokens.len() > 1 {
            let second_last = tokens[tokens.len() - 2].surface();
            prev_sentence_ends_with_number =
                !second_last.is_empty() && second_last.chars().all(|c| c.is_numeric());
        }
    }
    rule_matches
}

fn sw_match(range: TextRange, message: &str) -> Match {
    Match::new(
        DE_SENTENCE_WHITESPACE_ID,
        Option::<String>::None,
        message,
        Option::<String>::None,
        range,
        vec![Suggestion {
            value: " ".to_string(),
            short_description: None,
        }],
        "MISC",
        "Sonstiges",
    )
    .with_metadata(
        "Fehlendes Leerzeichen zwischen Sätzen oder nach Ordnungszahlen",
        "whitespace",
        0,
    )
}

// ---------------------------------------------------------------------------
// `MissingVerbRule` — Rule, category GRAMMAR, default off.
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.de.MissingVerbRule` (default off). Needs the
/// German tagger for the `verbAtSentenceStart` workaround.
pub fn missing_verb(
    tagger: &lt_tagger::GermanTagger,
    sentence: &AnalyzedSentence,
    _start: usize,
) -> Vec<Match> {
    const MIN_TOKENS_FOR_ERROR: usize = 5;
    let tokens = sentence.tokens_without_whitespace();
    // `isRealSentence`: must end with . ? !
    let is_real_sentence = tokens
        .last()
        .is_some_and(|t| t.has_pos_tag("PKT") && matches!(t.surface(), "." | "?" | "!"));
    if !is_real_sentence {
        return Vec::new();
    }
    // `isSpecialCase`: "Vielen Dank" / "Herzlichen Glückwunsch"
    let surfaces: Vec<&str> = tokens.iter().map(|t| t.surface()).collect();
    for window in surfaces.windows(2) {
        if window == ["Vielen", "Dank"] || window == ["Herzlichen", "Glückwunsch"] {
            return Vec::new();
        }
    }
    let mut verb_found = false;
    let mut last_token: Option<&lt_core::AnalyzedTokenReadings> = None;
    for (i, readings) in tokens.iter().enumerate() {
        let is_verb = readings.has_pos_tag_starting_with("VER")
            || (!readings.is_tagged && !lt_tagger::is_capitalized_word(readings.surface()))
            || (i == 1 && verb_at_sentence_start(tagger, readings.surface()));
        if is_verb {
            verb_found = true;
            break;
        }
        last_token = Some(readings);
    }
    if !verb_found && tokens.len() >= MIN_TOKENS_FOR_ERROR {
        if let Some(last) = last_token {
            return vec![Match::new(
                "MISSING_VERB",
                Option::<String>::None,
                "Dieser Satz scheint kein Verb zu enthalten",
                None,
                TextRange::new(sentence.offset, sentence.offset + last.end_pos()),
                Vec::<Suggestion>::new(),
                "GRAMMAR",
                "Grammatik",
            )
            .with_metadata("Satz ohne Verb", "grammar", 0)];
        }
    }
    Vec::new()
}

fn verb_at_sentence_start(tagger: &lt_tagger::GermanTagger, word: &str) -> bool {
    let lowercased = lt_tagger::lowercase_first_char(word);
    let readings = tagger.tag(&[lowercased], true);
    readings
        .first()
        .is_some_and(|r| r.has_pos_tag_starting_with("VER"))
}

/// `org.languagetool.rules.de.RedundantModalOrAuxiliaryVerb` (`Rule`,
/// category STYLE, default off).
pub const REDUNDANT_MODAL_VERB_ID: &str = "REDUNDANT_MODAL_VERB";
const REDUNDANT_VERB_TEXT: &str = " scheint redundant zu sein. Prüfen Sie, ob es gelöscht oder der Satz umformuliert werden kann.";
const REDUNDANT_SUB_TEXT: &str = "Der Satzteil scheint redundant zu sein. Prüfen Sie, ob es gelöscht oder der Satz umformuliert werden kann.";

fn is_break_token(s: &str) -> bool {
    matches!(s, "und" | "oder" | "sowie")
        || (s.chars().count() == 1
            && matches!(
                s.chars().next().unwrap(),
                ',' | ';'
                    | '.'
                    | ':'
                    | '?'
                    | '!'
                    | '-'
                    | '–'
                    | '—'
                    | '’'
                    | '\''
                    | '"'
                    | '„'
                    | '“'
                    | '”'
                    | '»'
                    | '«'
                    | '‚'
                    | '‘'
                    | '›'
                    | '‹'
                    | '('
                    | ')'
                    | '['
                    | ']'
            ))
}

/// `RedundantModalOrAuxiliaryVerb.hasParticipleAt`.
fn has_participle_at(
    n_conjunction: usize,
    n_start: usize,
    tokens: &[&lt_core::AnalyzedTokenReadings],
) -> Option<usize> {
    if !tokens[n_conjunction - 1].has_pos_tag_starting_with("PA2") {
        return None;
    }
    let s_participle = tokens[n_conjunction - 1].surface().to_string();
    for i in n_start..tokens.len() {
        let s_token = tokens[i].surface().to_string();
        if is_break_token(&s_token) {
            return None;
        }
        if s_token == s_participle {
            if i == tokens.len() - 1 || is_break_token(tokens[i + 1].surface()) {
                return Some(i);
            }
            return None;
        }
    }
    None
}

fn redundant_match(
    tokens: &[&lt_core::AnalyzedTokenReadings],
    start: usize,
    from: usize,
    to: usize,
    message: String,
    suggestion: Option<String>,
) -> Match {
    Match::new(
        REDUNDANT_MODAL_VERB_ID,
        Option::<String>::None,
        message,
        Option::<String>::None,
        TextRange::new(start + tokens[from].end_pos(), start + tokens[to].end_pos()),
        vec![Suggestion {
            value: suggestion.unwrap_or_default(),
            short_description: None,
        }],
        "STYLE",
        "Stil",
    )
    .with_metadata("Redundantes Modal- oder Hilfsverb", "style", 0)
}

/// `RedundantModalOrAuxiliaryVerb.match(AnalyzedSentence)`.
pub fn redundant_modal_verb(sentence: &AnalyzedSentence, start: usize) -> Vec<Match> {
    let tokens = sentence.tokens_without_whitespace();
    let mut matches = Vec::new();
    let mut nt = 2usize;
    while nt < tokens.len() {
        let is_mod_verb = tokens[nt].has_pos_tag_starting_with("VER:MOD");
        let is_aux = tokens[nt].has_pos_tag_starting_with("VER:AUX");
        if !(is_mod_verb || is_aux)
            || nt + 1 >= tokens.len()
            || tokens[nt - 1].surface() == tokens[nt + 1].surface()
        {
            nt += 1;
            continue;
        }
        let s_verb = tokens[nt].surface().to_string();
        let n_verb = nt;
        nt += 1;
        'token_scan: while nt < tokens.len() {
            let s_token = tokens[nt].surface().to_string();
            if s_token.chars().count() == 1 && is_break_token(&s_token) {
                break;
            }
            if is_break_token(&s_token) {
                let n_conjunction = nt;
                nt += 1;
                while nt < tokens.len() {
                    let s_token = tokens[nt].surface().to_string();
                    if is_break_token(&s_token) {
                        break;
                    }
                    if s_token == s_verb {
                        let rule_match: Option<Match>;
                        let mut suggestion: Option<String> = None;
                        if nt - 1 == n_conjunction {
                            if n_verb == n_conjunction - 1 {
                                break;
                            }
                            let mut n = 1usize;
                            while nt + n < tokens.len()
                                && tokens[nt + n]
                                    .surface()
                                    .eq_ignore_ascii_case(tokens[n_verb + n].surface())
                            {
                                n += 1;
                            }
                            if n > 1 {
                                if n_verb + n == n_conjunction {
                                    break;
                                }
                                rule_match = Some(redundant_match(
                                    &tokens,
                                    start,
                                    nt - 1,
                                    nt + n - 1,
                                    REDUNDANT_SUB_TEXT.to_string(),
                                    None,
                                ));
                            } else {
                                let msg = format!(
                                    "Das {}{}",
                                    if is_mod_verb {
                                        "Modalverb"
                                    } else {
                                        "Hilfsverb"
                                    },
                                    REDUNDANT_VERB_TEXT
                                );
                                rule_match =
                                    Some(redundant_match(&tokens, start, nt - 1, nt, msg, None));
                            }
                        } else if tokens[nt - 1]
                            .surface()
                            .eq_ignore_ascii_case(tokens[n_verb - 1].surface())
                            && tokens[nt - 1].has_pos_tag_starting_with("PRO:PER")
                            && !tokens[nt - 1].has_pos_tag_starting_with("ART")
                        {
                            let msg = REDUNDANT_SUB_TEXT.to_string();
                            if n_verb == n_conjunction - 1 {
                                rule_match = Some(redundant_match(
                                    &tokens,
                                    start,
                                    n_verb - 2,
                                    n_verb,
                                    msg,
                                    None,
                                ));
                            } else {
                                rule_match =
                                    Some(redundant_match(&tokens, start, nt - 2, nt, msg, None));
                            }
                        } else if nt + 1 < tokens.len()
                            && tokens[nt + 1]
                                .surface()
                                .eq_ignore_ascii_case(tokens[n_verb + 1].surface())
                            && (tokens[nt + 1].has_pos_tag_starting_with("PRO:IND")
                                || (tokens[nt + 1].has_pos_tag_starting_with("PRO:PER")
                                    && tokens[nt + 1].surface() != "Sie"
                                    && !tokens[nt + 1].has_pos_tag_starting_with("ART")))
                        {
                            if tokens[nt + 1].has_pos_tag_starting_with("PRO:PER:AKK")
                                && tokens[nt].has_pos_tag_matching(&VER_KJ1_PATTERN)
                            {
                                let msg = format!(
                                    "Das {}{}",
                                    if is_mod_verb {
                                        "Modalverb"
                                    } else {
                                        "Hilfsverb"
                                    },
                                    REDUNDANT_VERB_TEXT
                                );
                                rule_match =
                                    Some(redundant_match(&tokens, start, nt - 1, nt, msg, None));
                            } else {
                                rule_match = Some(redundant_match(
                                    &tokens,
                                    start,
                                    nt - 1,
                                    nt + 1,
                                    REDUNDANT_SUB_TEXT.to_string(),
                                    None,
                                ));
                            }
                        } else {
                            if tokens[nt - 1].has_pos_tag_starting_with("PRO:PER")
                                || tokens[nt - 1].surface() == "da"
                                || tokens[nt - 1].surface() == "zu"
                                || tokens[n_verb + 1].surface() == tokens[nt - 1].surface()
                                || (nt + 1 < tokens.len()
                                    && (tokens[nt + 1].has_pos_tag_starting_with("PRO:PER")
                                        || tokens[nt - 1].surface() == tokens[nt + 1].surface()
                                        || tokens[nt - 1].surface() == tokens[nt + 1].surface()
                                        || tokens[n_verb - 1].surface()
                                            == tokens[nt + 1].surface()
                                        || (tokens[n_verb + 1]
                                            .has_pos_tag_starting_with("VER:MOD")
                                            && tokens[nt + 1]
                                                .has_pos_tag_starting_with("VER:MOD"))
                                        || (n_verb == n_conjunction - 1
                                            && !is_break_token(tokens[nt + 1].surface()))))
                                || (n_verb < n_conjunction - 1
                                    && (nt + 1 == tokens.len()
                                        || is_break_token(tokens[nt + 1].surface())))
                            {
                                break;
                            }
                            if n_verb == n_conjunction - 1 {
                                let mut n = 1usize;
                                while n_verb > n
                                    && nt - n > n_conjunction
                                    && tokens[n_verb - n].surface() == tokens[nt - n].surface()
                                {
                                    n += 1;
                                }
                                if n > 1 {
                                    rule_match = Some(redundant_match(
                                        &tokens,
                                        start,
                                        n_verb - n,
                                        n_verb,
                                        REDUNDANT_SUB_TEXT.to_string(),
                                        None,
                                    ));
                                } else {
                                    let msg = format!(
                                        "Das {}{}",
                                        if is_mod_verb {
                                            "Modalverb"
                                        } else {
                                            "Hilfsverb"
                                        },
                                        REDUNDANT_VERB_TEXT
                                    );
                                    rule_match = Some(redundant_match(
                                        &tokens,
                                        start,
                                        n_verb - 1,
                                        n_verb,
                                        msg,
                                        None,
                                    ));
                                }
                            } else {
                                let pa_at = has_participle_at(n_conjunction, nt + 1, &tokens);
                                if let Some(pa_at) = pa_at {
                                    let mut sugg = String::new();
                                    for t in tokens.iter().take(pa_at).skip(nt + 1) {
                                        sugg.push(' ');
                                        sugg.push_str(t.surface());
                                    }
                                    rule_match = Some(redundant_match(
                                        &tokens,
                                        start,
                                        nt - 1,
                                        pa_at,
                                        REDUNDANT_SUB_TEXT.to_string(),
                                        Some(sugg),
                                    ));
                                } else {
                                    let mut n = 1usize;
                                    while n + n_verb < n_conjunction
                                        && n + nt < tokens.len()
                                        && tokens[n_verb + n].surface() == tokens[nt + n].surface()
                                    {
                                        n += 1;
                                    }
                                    if n + n_verb == n_conjunction {
                                        rule_match = Some(redundant_match(
                                            &tokens,
                                            start,
                                            nt - 1,
                                            nt + n - 1,
                                            REDUNDANT_SUB_TEXT.to_string(),
                                            None,
                                        ));
                                    } else {
                                        let msg = format!(
                                            "Das {}{}",
                                            if is_mod_verb {
                                                "Modalverb"
                                            } else {
                                                "Hilfsverb"
                                            },
                                            REDUNDANT_VERB_TEXT
                                        );
                                        rule_match = Some(redundant_match(
                                            &tokens,
                                            start,
                                            nt - 1,
                                            nt,
                                            msg,
                                            None,
                                        ));
                                    }
                                }
                            }
                        }
                        if let Some(rule_match) = rule_match {
                            matches.push(rule_match);
                        }
                        let _ = suggestion.take();
                        break 'token_scan;
                    }
                    nt += 1;
                }
                break 'token_scan;
            }
            nt += 1;
        }
        nt += 1;
    }
    matches
}

static VER_KJ1_PATTERN: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^VER:(AUX|MOD):.*KJ1$").unwrap());
