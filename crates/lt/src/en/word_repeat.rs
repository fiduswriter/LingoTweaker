//! Port of `WordRepeatRule` and `EnglishWordRepeatRule`
//! (`ENGLISH_WORD_REPEAT_RULE`): repeated words like "the the", with the
//! English false-alarm exceptions.

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

use crate::wordutil::{eq_ignore_case, is_word};

const RULE_ID: &str = "ENGLISH_WORD_REPEAT_RULE";
const DESCRIPTION: &str = "Word repetition (e.g. 'will will')";
const MESSAGE: &str = "Possible typo: you repeated a word.";
const SHORT_MESSAGE: &str = "Word repetition";

fn single_char_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)^[a-z]$").unwrap());
    &RE
}

fn log_in_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"log(ged|s)?|sign(ed|s)?").unwrap());
    &RE
}

fn apostrophe_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"['’`´‘]").unwrap());
    &RE
}

fn repetition_of(word: &str, tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    position > 0
        && eq_ignore_case(tokens[position - 1].surface(), word)
        && eq_ignore_case(tokens[position].surface(), word)
}

/// Core `WordRepeatRule.wordRepetitionOf` (case-sensitive).
fn base_word_repetition_of(word: &str, tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    position > 0 && tokens[position - 1].surface() == word && tokens[position].surface() == word
}

fn pos_is_in(tokens: &[&AnalyzedTokenReadings], position: isize, pos_tags: &[&str]) -> bool {
    if position >= 0 && (position as usize) < tokens.len() {
        let token = tokens[position as usize];
        return pos_tags.iter().any(|tag| {
            token
                .readings
                .iter()
                .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with(tag)))
        });
    }
    false
}

fn next_is(tokens: &[&AnalyzedTokenReadings], position: usize, word: &str) -> bool {
    position + 1 < tokens.len() && eq_ignore_case(tokens[position + 1].surface(), word)
}

/// `EnglishWordRepeatRule.ignore`.
fn english_ignore(tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    if position == 0 {
        return false;
    }
    let word = tokens[position].surface().to_string();

    if (repetition_of("did", tokens, position)
        || repetition_of("do", tokens, position)
        || repetition_of("does", tokens, position))
        && next_is(tokens, position, "n't")
    {
        return true;
    }
    if repetition_of("her", tokens, position)
        && pos_is_in(
            tokens,
            position as isize - 2,
            &["VB", "VBP", "VBZ", "VBG", "VBD", "VBN"],
        )
        && pos_is_in(
            tokens,
            position as isize + 1,
            &["NN", "NNS", "NN:U", "NN:UN", "NNP"],
        )
    {
        return true; // "Please pass her her phone."
    }
    if repetition_of("had", tokens, position)
        && pos_is_in(tokens, position as isize - 2, &["PRP", "NN"])
    {
        return true;
    }
    if repetition_of("that", tokens, position)
        && pos_is_in(
            tokens,
            position as isize + 1,
            &["MD", "NN", "PRP$", "JJ", "VBZ", "VBD"],
        )
    {
        return true;
    }
    if repetition_of("can", tokens, position) && pos_is_in(tokens, position as isize - 1, &["NN"]) {
        return true;
    }
    if repetition_of("hip", tokens, position) && next_is(tokens, position, "hooray") {
        return true;
    }
    if repetition_of("bam", tokens, position) && next_is(tokens, position, "bigelow") {
        return true;
    }
    if repetition_of("wild", tokens, position) && next_is(tokens, position, "west") {
        return true;
    }
    if repetition_of("far", tokens, position) && next_is(tokens, position, "away") {
        return true;
    }
    if repetition_of("so", tokens, position) && next_is(tokens, position, "much") {
        return true;
    }
    if repetition_of("so", tokens, position) && next_is(tokens, position, "many") {
        return true;
    }
    if repetition_of("s", tokens, position)
        && position > 1
        && apostrophe_re().is_match(tokens[position - 2].surface())
    {
        return true; // It's S.T.E.A.M.
    }
    if repetition_of("in", tokens, position)
        && position > 2
        && log_in_re().is_match(tokens[position - 3].surface())
    {
        return true;
    }
    if repetition_of("in", tokens, position)
        && position > 1
        && log_in_re().is_match(tokens[position - 2].surface())
    {
        return true;
    }
    if repetition_of("a", tokens, position) && position > 1 && tokens[position - 2].surface() == "."
    {
        return true; // "a.k.a a"
    }
    if repetition_of("on", tokens, position)
        && position > 1
        && tokens[position - 2].surface() == "."
    {
        return true; // "You can contact E.ON on Instagram"
    }
    if eq_ignore_case(tokens[position - 1].surface(), &word)
        && ((position + 1 < tokens.len() && eq_ignore_case(tokens[position + 1].surface(), &word))
            || (position > 1 && eq_ignore_case(tokens[position - 2].surface(), &word)))
    {
        return true; // three times word repetition
    }
    if single_char_re().is_match(tokens[position].surface())
        && position > 1
        && single_char_re().is_match(tokens[position - 2].surface())
        && position + 1 < tokens.len()
        && single_char_re().is_match(tokens[position + 1].surface())
    {
        return true; // spelling with spaces in between
    }
    for filler in [
        "aye", "blah", "mau", "uh", "paw", "cha", "yum", "wop", "woop", "fnarr", "fnar", "ha",
        "omg", "boo", "tick", "twinkle", "ta", "la", "x", "hi", "ho", "heh", "jay", "walla", "sri",
        "hey", "hah", "oh", "ouh", "chop", "ring", "beep", "bleep", "yeah", "gout", "quack",
        "meow", "squawk", "whoa", "si", "honk", "brum", "chi", "santorio", "lapu", "chow", "shh",
        "yummy", "boom", "bye", "ah", "aah", "bang", "woof", "wink", "yes", "tsk", "hush", "ding",
        "choo", "miu", "tuk", "yadda", "doo", "sapiens", "tse", "no", "Bora",
    ] {
        if repetition_of(filler, tokens, position) {
            return true;
        }
    }
    if repetition_of("wait", tokens, position) && position == 2 {
        return true;
    }
    if tokens[position].surface().ends_with("ay") {
        if tokens[position - 1].surface() == "may" && tokens[position].surface() == "May" {
            return true;
        }
        if tokens[position - 1].surface() == "May" && tokens[position].surface() == "may" {
            return true;
        }
        if tokens.len() > 2 && tokens[1].surface() == "May" && tokens[2].surface() == "May" {
            return true; // "May May" SENT_START
        }
    } else if tokens[position].surface().ends_with("ill") {
        return (position > 0
            && tokens[position - 1].surface() == "will"
            && tokens[position].surface() == "Will")
            || (tokens[position - 1].surface() == "Will" && tokens[position].surface() == "will")
            || (tokens.len() > 2
                && tokens[1].surface() == "Will"
                && tokens[2].surface() == "Will");
    }
    // `WordRepeatRule.ignore`
    for name in [
        "Phi", "Li", "Xiao", "Duran", "Wagga", "Abdullah", "Nwe", "Pago", "Cao",
    ] {
        if base_word_repetition_of(name, tokens, position) {
            return true;
        }
    }
    false
}

/// `EnglishWordRepeatRule.match` over one sentence.
pub fn check_sentence(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut prev_token = String::new();
    for i in 1..view.len() {
        let token = view[i].surface().to_string();
        if view[i].is_immunized {
            prev_token.clear();
            continue;
        }
        if is_word(&token) && eq_ignore_case(&prev_token, &token) && !english_ignore(&view, i) {
            let prev_pos = view[i - 1].start_pos;
            let pos = view[i].start_pos;
            rule_matches.push(
                Match::new(
                    RULE_ID,
                    Option::<String>::None,
                    MESSAGE,
                    Some(SHORT_MESSAGE.to_string()),
                    TextRange::new(
                        sentence_offset + prev_pos,
                        sentence_offset + pos + prev_token.len(),
                    ),
                    vec![Suggestion {
                        value: prev_token.clone(),
                        short_description: None,
                    }],
                    "MISC",
                    "Miscellaneous",
                )
                .with_metadata(DESCRIPTION, "duplication", 1),
            );
        }
        prev_token = token;
    }
    rule_matches
}
