//! `MissingCommaRelativeClauseRule` (`COMMA_IN_FRONT_RELATIVE_CLAUSE` /
//! `COMMA_BEHIND_RELATIVE_CLAUSE`, checklist item 41).
//!
//! Two instances of the same class (front/behind relative clause), both
//! default on (category `HILFESTELLUNG_KOMMASETZUNG` "Kommasetzung",
//! `Location.INTERNAL`, on by default). The 25 antipatterns are shared by
//! both instances (`static ANTI_PATTERNS`).

use std::sync::{Arc, LazyLock};

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};
use lt_pattern::matcher as pm;
use lt_pattern::PatternToken;

use super::util;

pub const FRONT_ID: &str = "COMMA_IN_FRONT_RELATIVE_CLAUSE";
pub const BEHIND_ID: &str = "COMMA_BEHIND_RELATIVE_CLAUSE";
const CATEGORY_ID: &str = "HILFESTELLUNG_KOMMASETZUNG";
const CATEGORY_NAME: &str = "Kommasetzung";

static MARKS_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| util::anchored("[,;.:?•!-–—’'\"„“”…»«‚‘›‹()\\/\\[\\]]"));
static PRONOUN: LazyLock<regex::Regex> =
    LazyLock::new(|| util::anchored("(d(e[mnr]|ie|as|e([nr]|ss)en)|welche[mrs]?|wessen|was)"));
static VERB_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| util::anchored("(VER:[1-3]:|VER:.*:[1-3]:).*"));
static ZAL_ETC_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| util::anchored("(ZAL|AD[JV]|ART|SUB|PRO:POS|PRP).*"));

/// The two rule instances share the compiled antipatterns.
static ANTI_PATTERNS: LazyLock<Vec<Arc<pm::CompiledPattern>>> = LazyLock::new(compile_antipatterns);

pub struct MissingCommaRelativeClauseRule {
    behind: bool,
}

impl MissingCommaRelativeClauseRule {
    pub fn new(behind: bool) -> Self {
        Self { behind }
    }

    pub fn rule_id(&self) -> &'static str {
        if self.behind {
            BEHIND_ID
        } else {
            FRONT_ID
        }
    }

    pub fn description(&self) -> &'static str {
        if self.behind {
            "Fehlendes Komma nach Relativsatz"
        } else {
            "Fehlendes Komma vor Relativsatz"
        }
    }

    /// `MissingCommaRelativeClauseRule.match`.
    pub fn match_sentence(
        &self,
        sentence: &AnalyzedSentence,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let immunized = util::immunize_sentence(sentence, &ANTI_PATTERNS);
        let tokens = immunized.tokens_without_whitespace();
        if tokens.len() <= 1 {
            return rule_matches;
        }
        let mut sub_start = 1usize;
        if is_separator(tokens[sub_start].surface()) {
            sub_start += 1;
        }
        if self.behind {
            let mut sub_in_front = sub_start;
            if sub_in_front >= tokens.len() {
                return rule_matches;
            }
            sub_start = next_separator(&tokens, sub_in_front) + 1;
            while sub_start < tokens.len() {
                let sub_end = next_separator(&tokens, sub_start);
                let last_verb = has_potential_subclause(&tokens, sub_start, sub_end);
                if last_verb > 0 {
                    let n_token = missed_comma_behind(&tokens, sub_in_front, sub_start, sub_end);
                    if n_token > 0 {
                        let n_token = n_token as usize;
                        if is_verb_pro_pair(&tokens, n_token) {
                            let m = Match::new(
                                BEHIND_ID,
                                Option::<String>::None,
                                "Sollten Sie hier ein Komma einfügen oder zwei?",
                                Option::<String>::None,
                                TextRange::new(
                                    sentence_offset + tokens[n_token - 1].start_pos,
                                    sentence_offset + tokens[n_token + 1].end_pos(),
                                ),
                                vec![
                                    Suggestion {
                                        value: format!(
                                            "{}, {} {},",
                                            tokens[n_token - 1].surface(),
                                            tokens[n_token].surface(),
                                            tokens[n_token + 1].surface()
                                        ),
                                        short_description: None,
                                    },
                                    Suggestion {
                                        value: format!(
                                            "{} {} {},",
                                            tokens[n_token - 1].surface(),
                                            tokens[n_token].surface(),
                                            tokens[n_token + 1].surface()
                                        ),
                                        short_description: None,
                                    },
                                    Suggestion {
                                        value: format!(
                                            "{} {}, {}",
                                            tokens[n_token - 1].surface(),
                                            tokens[n_token].surface(),
                                            tokens[n_token + 1].surface()
                                        ),
                                        short_description: None,
                                    },
                                ],
                                CATEGORY_ID,
                                CATEGORY_NAME,
                            )
                            .with_metadata(
                                self.description(),
                                "uncategorized",
                                0,
                            );
                            rule_matches.push(m);
                        } else {
                            let m = Match::new(
                                BEHIND_ID,
                                Option::<String>::None,
                                "Sollten Sie hier ein Komma einfügen?",
                                Option::<String>::None,
                                TextRange::new(
                                    sentence_offset + tokens[n_token].start_pos,
                                    sentence_offset + tokens[n_token + 1].end_pos(),
                                ),
                                vec![Suggestion {
                                    value: format!(
                                        "{}, {}",
                                        tokens[n_token].surface(),
                                        tokens[n_token + 1].surface()
                                    ),
                                    short_description: None,
                                }],
                                CATEGORY_ID,
                                CATEGORY_NAME,
                            )
                            .with_metadata(
                                self.description(),
                                "uncategorized",
                                0,
                            );
                            rule_matches.push(m);
                        }
                    }
                }
                sub_in_front = sub_start;
                sub_start = sub_end + 1;
            }
        } else {
            while sub_start < tokens.len() {
                let sub_end = next_separator(&tokens, sub_start);
                let last_verb = has_potential_subclause(&tokens, sub_start, sub_end);
                if last_verb > 0 {
                    let n_token = missed_comma_in_front(&tokens, sub_start, sub_end, last_verb);
                    if n_token > 0 {
                        let n_token = n_token as usize;
                        let start_token = n_token - if is_prp(tokens[n_token - 1]) { 2 } else { 1 };
                        let message = "Sowohl angehängte als auch eingeschobene Relativsätze werden durch Kommas vom Hauptsatz getrennt.";
                        let suggestion = if n_token - start_token > 1 {
                            format!(
                                "{}, {} {}",
                                tokens[start_token].surface(),
                                tokens[n_token - 1].surface(),
                                tokens[n_token].surface()
                            )
                        } else {
                            format!(
                                "{}, {}",
                                tokens[start_token].surface(),
                                tokens[n_token].surface()
                            )
                        };
                        let m = Match::new(
                            FRONT_ID,
                            Option::<String>::None,
                            message,
                            Option::<String>::None,
                            TextRange::new(
                                sentence_offset + tokens[start_token].start_pos,
                                sentence_offset + tokens[n_token].end_pos(),
                            ),
                            vec![Suggestion {
                                value: suggestion,
                                short_description: None,
                            }],
                            CATEGORY_ID,
                            CATEGORY_NAME,
                        )
                        .with_metadata(
                            self.description(),
                            "uncategorized",
                            0,
                        );
                        rule_matches.push(m);
                    }
                }
                sub_start = sub_end + 1;
            }
        }
        rule_matches
    }
}

/// `isSeparator`.
fn is_separator(token: &str) -> bool {
    MARKS_REGEX.is_match(token) || token == "und" || token == "oder"
}

/// `nextSeparator`.
fn next_separator(tokens: &[&AnalyzedTokenReadings], start: usize) -> usize {
    for (i, token) in tokens.iter().enumerate().skip(start) {
        if is_separator(token.surface()) {
            return i;
        }
    }
    tokens.len() - 1
}

/// `isPrp`.
fn is_prp(token: &AnalyzedTokenReadings) -> bool {
    token.has_pos_tag_starting_with("PRP:") && !token.is_immunized
}

/// `isVerb`.
fn is_verb(tokens: &[&AnalyzedTokenReadings], n: usize) -> bool {
    tokens[n].has_pos_tag_matching(&VERB_PATTERN)
        && !tokens[n].has_pos_tag_matching(&ZAL_ETC_PATTERN)
        && (!tokens[n].has_pos_tag_starting_with("VER:INF:") || tokens[n - 1].surface() != "zu")
        && !tokens[n].is_immunized
}

/// `isAnyVerb`.
fn is_any_verb(tokens: &[&AnalyzedTokenReadings], n: usize) -> bool {
    tokens[n].has_pos_tag_starting_with("VER:")
        || (n < tokens.len() - 1
            && ((tokens[n].surface() == "zu"
                && tokens[n + 1].has_pos_tag_starting_with("VER:INF:"))
                || (tokens[n].has_pos_tag("NEG")
                    && tokens[n + 1].has_pos_tag_starting_with("VER:"))))
}

/// `isVerbBehind` (used by `MissingCommaRelativeClauseRuleTest` upstream;
/// kept for API parity).
#[allow(dead_code)]
pub(crate) fn is_verb_behind(tokens: &[&AnalyzedTokenReadings], end: usize) -> bool {
    end < tokens.len() - 1
        && tokens[end].surface() == ","
        && tokens[end + 1].has_pos_tag_starting_with("VER:")
}

/// `verbPos`.
fn verb_pos(tokens: &[&AnalyzedTokenReadings], start: usize, end: usize) -> Vec<usize> {
    let mut verbs = Vec::new();
    for i in start..end {
        if is_verb(tokens, i) {
            if tokens[i].has_pos_tag_starting_with("PA") {
                let gender = get_gender(tokens[i]);
                let s_str = util::anchored(&format!("(ADJ|PA[12]):.*{gender}.*"));
                let mut j = i + 1;
                while j < end && tokens[j].has_pos_tag_matching(&s_str) {
                    j += 1;
                }
                if let Some(token) = tokens.get(j) {
                    let sub_str = util::anchored(&format!("(SUB|EIG):.*{gender}.*"));
                    if !token.has_pos_tag_matching(&sub_str) && !util::is_pos_tag_unknown(token) {
                        verbs.push(i);
                    }
                }
            } else {
                verbs.push(i);
            }
        }
    }
    verbs
}

/// `isKonUnt`.
fn is_kon_unt(token: &AnalyzedTokenReadings) -> bool {
    token.has_pos_tag("KON:UNT")
        || ["wer", "wo", "wohin"]
            .iter()
            .any(|w| token.surface().eq_ignore_ascii_case(w))
}

/// `hasPotentialSubclause`.
fn has_potential_subclause(tokens: &[&AnalyzedTokenReadings], start: usize, end: usize) -> isize {
    let verbs = verb_pos(tokens, start, end);
    if verbs.len() == 1 && end < tokens.len() - 2 && verbs[0] == end - 1 {
        let next_end = next_separator(tokens, end + 1);
        let next_verbs = verb_pos(tokens, end + 1, next_end);
        if is_kon_unt(tokens[start]) {
            if next_verbs.len() > 1 || (next_verbs.len() == 1 && next_verbs[0] == end - 1) {
                return verbs[0] as isize;
            }
        } else if !next_verbs.is_empty() {
            return verbs[0] as isize;
        }
        return -1;
    }
    if verbs.len() == 2 {
        if tokens[verbs[0]].has_pos_tag_matching(&util::anchored("VER:(MOD|AUX):.*"))
            && tokens[verbs[1]].has_pos_tag_starting_with("VER:INF:")
        {
            return verbs[0] as isize;
        }
        if tokens[verbs[0]].has_pos_tag_starting_with("VER:AUX:")
            && tokens[verbs[1]].has_pos_tag_starting_with("VER:PA2:")
        {
            return -1;
        }
        if end == tokens.len() - 1
            && verbs[0] == end - 2
            && tokens[verbs[0]].has_pos_tag_starting_with("VER:INF:")
            && tokens[verbs[1]].has_pos_tag_starting_with("VER:MOD:")
        {
            return -1;
        }
    }
    if verbs.len() == 3
        && tokens[verbs[0]].has_pos_tag_starting_with("VER:MOD:")
        && ((tokens[verbs[2] - 1].has_pos_tag_matching(&util::anchored("VER:(INF|PA2):.*"))
            && tokens[verbs[2]].has_pos_tag_starting_with("VER:INF:"))
            || (tokens[verbs[1] - 1].surface() == "weder"
                && tokens[verbs[1]].has_pos_tag_starting_with("VER:INF:")
                && tokens[verbs[2] - 1].surface() == "noch"
                && tokens[verbs[1]].has_pos_tag_starting_with("VER:INF:")))
    {
        return -1;
    }
    if verbs.len() > 1 {
        return verbs[verbs.len() - 1] as isize;
    }
    -1
}

/// `isPronoun`.
fn is_pronoun(tokens: &[&AnalyzedTokenReadings], n: usize) -> bool {
    PRONOUN.is_match(tokens[n].surface()) && tokens[n - 1].surface() != "sowie"
}

/// `getGender`.
fn get_gender(token: &AnalyzedTokenReadings) -> String {
    let mut ret = String::new();
    let mut n_matches = 0;
    for (pattern, value) in [
        (".*:SIN:FEM.*", "SIN:FEM"),
        (".*:SIN:MAS.*", "SIN:MAS"),
        (".*:SIN:NEU.*", "SIN:NEU"),
        (".*:PLU.*", "PLU"),
    ] {
        if token.has_pos_tag_matching(&util::anchored(pattern)) {
            if n_matches > 0 {
                ret.push('|');
            }
            ret.push_str(value);
            n_matches += 1;
        }
    }
    if n_matches > 1 {
        ret = format!("({ret})");
    }
    ret
}

/// `matchesGender`.
fn matches_gender(gender: &str, tokens: &[&AnalyzedTokenReadings], from: usize, to: usize) -> bool {
    let m_str = if gender.is_empty() {
        util::anchored("PRO:DEM:.*SIN:NEU.*")
    } else {
        util::anchored(&format!("(SUB|EIG):.*{gender}.*"))
    };
    let mut i = to;
    while i > from {
        i -= 1;
        if tokens[i].has_pos_tag_matching(&m_str)
            && (i != 1 || !tokens[i].has_pos_tag_starting_with("VER:"))
        {
            return true;
        }
    }
    false
}

/// `isArticleWithoutSub`.
fn is_article_without_sub(gender: &str, tokens: &[&AnalyzedTokenReadings], n: usize) -> bool {
    if gender.is_empty() || n == 0 {
        return false;
    }
    tokens[n].has_pos_tag_starting_with("VER:")
        && tokens[n - 1].has_pos_tag_matching(&util::anchored(&format!(
            "(ADJ|PA[12]|PRO:POS):.*{gender}.*"
        )))
}

/// `skipSub`.
fn skip_sub(tokens: &[&AnalyzedTokenReadings], n: usize, to: usize) -> isize {
    let gender = get_gender(tokens[n]);
    let pattern = util::anchored(&format!("(SUB|EIG):.*{gender}.*"));
    for (i, token) in tokens.iter().enumerate().take(to).skip(n + 1) {
        if token.has_pos_tag_matching(&pattern) {
            return i as isize;
        }
    }
    -1
}

/// `skipToSub`.
fn skip_to_sub(gender: &str, tokens: &[&AnalyzedTokenReadings], n: usize, to: usize) -> isize {
    if tokens
        .get(n + 1)
        .is_some_and(|t| t.has_pos_tag_matching(&util::anchored(&format!("PA[12]:.*{gender}.*"))))
    {
        return (n + 1) as isize;
    }
    let adj = util::anchored(&format!("(ADJ|PA[12]):.*{gender}.*"));
    let mut i = n + 1;
    while i < to {
        if tokens[i].has_pos_tag_matching(&adj) || util::is_pos_tag_unknown(tokens[i]) {
            return i as isize;
        }
        if tokens[i].has_pos_tag_starting_with("ART") {
            let next = skip_sub(tokens, i, to);
            if next < 0 {
                return next;
            }
            i = next as usize;
        }
        i += 1;
    }
    -1
}

/// `isArticle`.
fn is_article(gender: &str, tokens: &[&AnalyzedTokenReadings], from: usize, to: usize) -> bool {
    if gender.is_empty() {
        return false;
    }
    let s_sub = util::anchored(&format!("(SUB|EIG):.*{gender}.*"));
    let s_adj = util::anchored(&format!(
        "(ZAL|PRP:|KON:|ADV:|ADJ:PRD:|(ADJ|PA[12]|PRO:(POS|DEM|IND)):.*{gender}).*"
    ));
    let mut i = from + 1;
    while i < to {
        if tokens[i].has_pos_tag_matching(&s_sub) || util::is_pos_tag_unknown(tokens[i]) {
            return true;
        }
        if tokens[i].has_pos_tag_starting_with("ART") || !tokens[i].has_pos_tag_matching(&s_adj) {
            if is_article_without_sub(gender, tokens, i) {
                return true;
            }
            let skip_to = skip_to_sub(gender, tokens, i, to);
            if skip_to > 0 {
                i = skip_to as usize;
            } else {
                return false;
            }
        }
        i += 1;
    }
    to < tokens.len() && is_article_without_sub(gender, tokens, to)
}

/// `missedCommaInFront`.
fn missed_comma_in_front(
    tokens: &[&AnalyzedTokenReadings],
    start: usize,
    _end: usize,
    last_verb: isize,
) -> isize {
    let mut i = start;
    while i < last_verb as usize - 1 {
        if tokens[i].is_immunized {
            i += 1;
            continue;
        }
        if is_pronoun(tokens, i) {
            let gender = get_gender(tokens[i]);
            if !is_any_verb(tokens, i + 1)
                && matches_gender(&gender, tokens, start, i)
                && !is_article(&gender, tokens, i, last_verb as usize)
            {
                return i as isize;
            }
        }
        i += 1;
    }
    -1
}

/// `isTwoCombinedVerbs`.
fn is_two_combined_verbs(first: &AnalyzedTokenReadings, second: &AnalyzedTokenReadings) -> bool {
    first.has_pos_tag_matching(&util::anchored("(VER:.*INF|.*PA[12]:).*"))
        && second.has_pos_tag_starting_with("VER:")
}

/// `isThreeCombinedVerbs`.
fn is_three_combined_verbs(tokens: &[&AnalyzedTokenReadings], first: usize, last: usize) -> bool {
    tokens[first].has_pos_tag_matching(&util::anchored("VER:(AUX|INF|PA[12]).*"))
        && tokens[first + 1].has_pos_tag_matching(&util::anchored("VER:(.*INF|PA[12]).*"))
        && tokens[last].has_pos_tag_matching(&util::anchored("VER:(MOD|AUX).*"))
}

/// `isFourCombinedVerbs`.
fn is_four_combined_verbs(tokens: &[&AnalyzedTokenReadings], first: usize, last: usize) -> bool {
    util::has_partial_pos_tag(tokens[first], "KJ2")
        && util::has_partial_pos_tag(tokens[first + 1], "PA2")
        && tokens[first + 2].has_pos_tag_matching(&util::anchored("VER:(.*INF|PA[12]).*"))
        && tokens[last].has_pos_tag_matching(&util::anchored("VER:(MOD|AUX).*"))
}

/// `isPar`.
fn is_par(token: &AnalyzedTokenReadings) -> bool {
    token.has_pos_tag_starting_with("PA2:") || token.has_pos_tag_starting_with("VER:PA2")
}

/// `isInfinitivZu`.
fn is_infinitiv_zu(tokens: &[&AnalyzedTokenReadings], last: usize) -> bool {
    tokens[last - 1].surface() == "zu"
        && tokens[last].has_pos_tag_matching(&util::anchored("VER:.*INF.*"))
}

/// `isTwoPlusCombinedVerbs`.
fn is_two_plus_combined_verbs(
    tokens: &[&AnalyzedTokenReadings],
    first: usize,
    last: usize,
) -> bool {
    tokens[first].has_pos_tag_matching(&util::anchored(".*PA[12]:.*"))
        && tokens[last - 1].has_pos_tag_matching(&util::anchored("VER:.*INF.*"))
}

/// `isKonAfterVerb`.
fn is_kon_after_verb(tokens: &[&AnalyzedTokenReadings], start: usize, end: usize) -> bool {
    if tokens[start].has_pos_tag_matching(&util::anchored("VER:(MOD|AUX).*"))
        && tokens[start + 1].has_pos_tag_matching(&util::anchored("(KON|PRP).*"))
    {
        if start + 3 == end {
            return true;
        }
        for token in tokens.iter().take(end).skip(start + 2) {
            if token.has_pos_tag_matching(&util::anchored("(SUB|PRO:PER).*")) {
                return true;
            }
        }
    }
    false
}

/// `isSpecialPair`.
fn is_special_pair(tokens: &[&AnalyzedTokenReadings], first: usize, second: usize) -> bool {
    if first + 3 >= second
        && tokens[first].has_pos_tag_matching(&util::anchored("VER:.*INF.*"))
        && matches!(tokens[first + 1].surface(), "als" | "noch")
        && tokens[first + 2].has_pos_tag_matching(&util::anchored("VER:.*INF.*"))
    {
        if first + 2 == second {
            return true;
        }
        return is_two_combined_verbs(tokens[second - 1], tokens[second]);
    }
    false
}

/// `isPerfect` (two indices).
fn is_perfect(tokens: &[&AnalyzedTokenReadings], first: usize, second: usize) -> bool {
    tokens[first].has_pos_tag_starting_with("VER:AUX:")
        && tokens[second].has_pos_tag_matching(&util::anchored("VER:.*(INF|PA2).*"))
}

/// `isPerfect` (three indices).
fn is_perfect3(
    tokens: &[&AnalyzedTokenReadings],
    first: usize,
    second: usize,
    third: usize,
) -> bool {
    tokens[second].has_pos_tag_matching(&util::anchored("VER:.*INF.*"))
        && is_perfect(tokens, first, third)
}

/// `isSpecialInf`.
fn is_special_inf(
    tokens: &[&AnalyzedTokenReadings],
    first: usize,
    second: usize,
    start: usize,
) -> bool {
    if !tokens[first].has_pos_tag_starting_with("VER:INF") {
        return false;
    }
    let mut i = first;
    while i > start + 1 {
        i -= 1;
        if tokens[i].has_pos_tag_starting_with("ART") {
            let skip_to = skip_sub(tokens, i, second);
            return skip_to > 0;
        }
    }
    false
}

/// `isSeparatorOrInf`.
fn is_separator_or_inf(tokens: &[&AnalyzedTokenReadings], n: usize) -> bool {
    is_separator(tokens[n].surface())
        || tokens[n].has_pos_tag_starting_with("VER:INF")
        || (tokens.len() > n + 1
            && tokens[n].surface() == "zu"
            && tokens[n + 1].has_pos_tag_matching(&util::anchored("VER:.*INF.*")))
}

/// `getCommaBehind`.
fn get_comma_behind(
    tokens: &[&AnalyzedTokenReadings],
    verbs: &[usize],
    start: usize,
    end: usize,
) -> isize {
    if verbs.len() == 1 {
        if is_separator(tokens[verbs[0] + 1].surface()) {
            return -1;
        }
        return verbs[0] as isize;
    } else if verbs.len() == 2 {
        if is_special_pair(tokens, verbs[0], verbs[1]) {
            if is_separator_or_inf(tokens, verbs[1] + 1) {
                return -1;
            }
            return verbs[1] as isize;
        } else if verbs[0] + 1 == verbs[1] {
            if is_two_combined_verbs(tokens[verbs[0]], tokens[verbs[1]]) {
                if is_separator_or_inf(tokens, verbs[1] + 1)
                    || is_kon_after_verb(tokens, verbs[1], end)
                {
                    return -1;
                }
                return verbs[1] as isize;
            }
        } else if verbs[0] + 2 == verbs[1] && is_three_combined_verbs(tokens, verbs[0], verbs[1]) {
            if is_separator_or_inf(tokens, verbs[1] + 1) {
                return -1;
            }
            return verbs[1] as isize;
        }
        if is_par(tokens[verbs[0]])
            || is_perfect(tokens, verbs[0], verbs[1])
            || is_infinitiv_zu(tokens, verbs[1])
            || is_special_inf(tokens, verbs[0], verbs[1], start)
        {
            if is_separator_or_inf(tokens, verbs[1] + 1) {
                return -1;
            }
            return verbs[1] as isize;
        }
    } else if verbs.len() == 3 {
        if is_two_plus_combined_verbs(tokens, verbs[0], verbs[2]) {
            if is_separator_or_inf(tokens, verbs[2] + 1) {
                return -1;
            }
            return verbs[2] as isize;
        } else if verbs[0] + 2 == verbs[2] {
            if verbs[0] + 1 == verbs[1] && is_three_combined_verbs(tokens, verbs[0], verbs[2]) {
                if is_separator_or_inf(tokens, verbs[2] + 1) {
                    return -1;
                }
                return verbs[2] as isize;
            }
        } else if (verbs[0] + 3 == verbs[2] && is_four_combined_verbs(tokens, verbs[0], verbs[2]))
            || (tokens[verbs[2]].has_pos_tag_starting_with("VER:MOD:")
                && is_special_pair(tokens, verbs[0], verbs[1]))
        {
            if is_separator_or_inf(tokens, verbs[2] + 1) {
                return -1;
            }
            return verbs[2] as isize;
        }
        if is_perfect3(tokens, verbs[0], verbs[1], verbs[2]) {
            if is_separator_or_inf(tokens, verbs[2] + 1) {
                return -1;
            }
            return verbs[1] as isize;
        }
    }
    verbs[0] as isize
}

/// `missedCommaBehind`.
fn missed_comma_behind(
    tokens: &[&AnalyzedTokenReadings],
    in_front: usize,
    start: usize,
    end: usize,
) -> isize {
    for i in start..end {
        if is_pronoun(tokens, i) {
            let verbs = verb_pos(tokens, i, end);
            if !verbs.is_empty() {
                let gender = get_gender(tokens[i]);
                if !is_any_verb(tokens, i + 1)
                    && matches_gender(&gender, tokens, in_front, i.saturating_sub(1))
                    && !is_article(&gender, tokens, i, verbs[verbs.len() - 1])
                {
                    return get_comma_behind(tokens, &verbs, i, end);
                }
            }
        }
    }
    -1
}

/// `getSinOrPluOfPro`.
fn get_sin_or_plu_of_pro(token: &AnalyzedTokenReadings) -> Option<String> {
    if !util::has_partial_pos_tag(token, "PRO:PER:") && !token.has_pos_tag_starting_with("IND:") {
        return None;
    }
    let mut ret = String::new();
    let mut n_matches = 0;
    if token.has_pos_tag_matching(&util::anchored(".*:SIN.*")) {
        ret.push_str("SIN");
        n_matches += 1;
    }
    if token.has_pos_tag_matching(&util::anchored(".*:PLU.*")) {
        if !ret.is_empty() {
            ret.push('|');
        }
        ret.push_str("PLU");
        n_matches += 1;
    }
    if n_matches > 1 {
        ret = format!("({ret})");
    }
    Some(ret)
}

/// `isVerbProPair`.
fn is_verb_pro_pair(tokens: &[&AnalyzedTokenReadings], n: usize) -> bool {
    let Some(sin_or_plu) = get_sin_or_plu_of_pro(tokens[n + 1]) else {
        return false;
    };
    tokens[n].has_pos_tag_matching(&util::anchored(&format!("VER:.*{sin_or_plu}.*")))
}

fn token(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        in_marker: true,
        ..Default::default()
    }
}

fn cs_token(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        case_sensitive: true,
        in_marker: true,
        ..Default::default()
    }
}

fn regex(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        regexp: true,
        in_marker: true,
        ..Default::default()
    }
}

fn pos(postag: &str) -> PatternToken {
    PatternToken {
        postag: Some(postag.to_string()),
        in_marker: true,
        ..Default::default()
    }
}

fn pos_regex(postag: &str) -> PatternToken {
    PatternToken {
        postag: Some(postag.to_string()),
        postag_regexp: true,
        in_marker: true,
        ..Default::default()
    }
}

fn inflected_regex(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        regexp: true,
        inflected: true,
        in_marker: true,
        ..Default::default()
    }
}

fn pos_regex_end_inflected(text: &str) -> PatternToken {
    PatternToken {
        postag: Some("SENT_END".to_string()),
        postag_regexp: true,
        text: Some(text.to_string()),
        regexp: true,
        inflected: true,
        in_marker: true,
        ..Default::default()
    }
}

fn skip(mut t: PatternToken, value: i32) -> PatternToken {
    t.skip = Some(value);
    t
}

/// `MissingCommaRelativeClauseRule.ANTI_PATTERNS` (order matters).
fn antipattern_defs() -> Vec<Vec<PatternToken>> {
    vec![
        vec![regex("gerade|wenn"), token("das")],
        vec![
            token("anstelle"),
            regex(
                "diese[rsm]|de[rsm]|dessen|jene[rsm]|[dms]?eine[rsm]|ihre[rs]|eure[sr]|unse?re[sr]",
            ),
        ],
        vec![token("im"), token("Zuge"), token("dessen")],
        vec![
            cs_token("mit"),
            regex("de[mr]"),
            regex("de[mrs]"),
            pos_regex("SUB:.+"),
            cs_token("verbindet"),
        ],
        vec![regex("eine"), cs_token("menge"), pos_regex("SUB:.+")],
        vec![regex("wie"), cs_token("folgt"), pos_regex("VER:.+")],
        vec![regex("gut"), cs_token("überlegt"), cs_token("sein")],
        vec![cs_token("samt"), pos_regex("SUB:DAT.*")],
        vec![
            pos_regex("PA2:PRD:GRU:VER|VER:PA2.*"),
            cs_token("sind"),
            pos_regex("PKT"),
        ],
        vec![
            cs_token("am"),
            pos("ADJ:PRD:SUP"),
            pos_regex("PRP:.+"),
            regex("d(e[mnr]|ie|as|e([nr]|ss)en)"),
        ],
        vec![
            pos("SENT_START"),
            token("Aber"),
            regex("der|die|denen|das|jenen|einigen|anderen|vielen|manchen|allen"),
        ],
        vec![
            pos_regex("PA2.*|VER:PA2.*"),
            token("werden"),
            regex("[\\.\\!\\?…\\:;]+"),
        ],
        vec![
            token("werden"),
            pos_regex_end_inflected("sollen|können|müssen"),
        ],
        vec![
            pos_regex("PA2.*|VER:PA2.*"),
            pos_regex_end_inflected("haben|werden"),
        ],
        vec![
            pos_regex("VER:INF.*"),
            pos_regex_end_inflected("können|werden|sollen|dürfen|müssen|wollen|mögen"),
        ],
        vec![regex("ja|mal"), cs_token("was")],
        vec![
            pos_regex("SENT_START|PKT"),
            cs_token("aber"),
            regex("solange|wenn|wo|wie|was"),
            regex("du|er|sie|sich|man|euch|uns|die|der|das"),
        ],
        vec![
            cs_token("selbst"),
            cs_token("wenn"),
            regex("du|er|sie|sich|man|euch|uns|die|der|das"),
            regex("die|der|das"),
        ],
        vec![cs_token("wie"), regex("die|der|das")],
        vec![skip(inflected_regex("weder"), 12), token("noch")],
        vec![
            pos_regex("VER:.*1:SIN:KJ1:.+"),
            pos_regex("VER:MOD:[12]:.+"),
            pos_regex("PKT|KON:NEB"),
        ],
        vec![
            pos_regex("VER:.+"),
            cs_token("bzw"),
            cs_token("."),
            pos_regex("VER:.+"),
        ],
        vec![
            regex("w[eu]rden"),
            pos_regex("PA2:PRD:GRU:VER|VER:PA2.*"),
            pos("PKT"),
        ],
        vec![
            pos_regex("PA2:PRD:GRU:VER|VER:PA2.*"),
            regex("haben?|hatten?"),
            pos_regex("VER:EIZ.*"),
            pos("PKT"),
        ],
        vec![pos_regex("VER.*"), regex("\\u2063")],
    ]
}

fn compile_antipatterns() -> Vec<Arc<pm::CompiledPattern>> {
    antipattern_defs()
        .iter()
        .flat_map(|tokens| pm::compile_patterns(tokens, None, None).unwrap_or_default())
        .map(Arc::new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_all_antipatterns() {
        assert_eq!(compile_antipatterns().len(), antipattern_defs().len());
        assert_eq!(antipattern_defs().len(), 25);
    }

    #[test]
    fn gender_string_matches_java() {
        use lt_core::{AnalyzedToken, AnalyzedTokenReadings};
        let tok = AnalyzedTokenReadings::new(vec![
            AnalyzedToken::new("Tag", Some("Tag".into()), Some("SUB:NOM:SIN:MAS".into())),
            AnalyzedToken::new("Tag", Some("Tag".into()), Some("SUB:AKK:SIN:NEU".into())),
        ]);
        assert_eq!(get_gender(&tok), "(SIN:MAS|SIN:NEU)");
        let tok = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            "Tag",
            Some("Tag".into()),
            Some("SUB:NOM:SIN:MAS".into()),
        )]);
        assert_eq!(get_gender(&tok), "SIN:MAS");
    }
}
