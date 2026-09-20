//! `AbstractStyleRepeatedWordRule` + `GermanStyleRepeatedWordRule`
//! (`STYLE_REPEATED_WORD_RULE_DE`, text level, default off, D-034).
//!
//! The base class is a core rule but has only the German subclass (verified
//! with `grep -rl AbstractStyleRepeatedWordRule` on the pinned checkout), so
//! the port lives in the German module.
//!
//! Scope: engine defaults (`UserConfig` stance). `TEST_COMPOUND_WORDS` is
//! `false` by default, so `isPartOfWord`/`isCorrectSpell` (the compound
//! speller path) are unreachable without a user configuration and are not
//! ported. `RuleMatch.setUrl` (openThesaurus link) is a tools-layer field the
//! engine's `Match` does not carry.

use std::sync::LazyLock;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, TextRange};

use super::util;

pub const RULE_ID: &str = "STYLE_REPEATED_WORD_RULE_DE";
const DESCRIPTION: &str = "Wiederholte Worte in aufeinanderfolgenden Sätzen";
const MESSAGE_SAME_SENTENCE: &str =
    "Mögliches Stilproblem: Das Wort wird noch einmal im selben Satz verwendet.";
const MESSAGE_SENTENCE_BEFORE: &str =
    "Mögliches Stilproblem: Das Wort wird bereits in einem vorhergehenden Satz verwendet.";
const MESSAGE_SENTENCE_AFTER: &str =
    "Mögliches Stilproblem: Das Wort wird auch in einem nachfolgenden Satz verwendet.";
const CATEGORY_ID: &str = "STYLE";
const CATEGORY_NAME: &str = "Stil";

/// `AbstractStyleRepeatedWordRule.MAX_DISTANCE_OF_SENTENCES`
/// (`GermanStyleRepeatedWordRule` default, user-configurable).
const MAX_DISTANCE_OF_SENTENCES: isize = 1;
/// `AbstractStyleRepeatedWordRule.EXCLUDE_DIRECT_SPEECH`
/// (`GermanStyleRepeatedWordRule` default, user-configurable).
const EXCLUDE_DIRECT_SPEECH: bool = true;
/// `AbstractStyleRepeatedWordRule.MAX_TOKEN_TO_CHECK`
const MAX_TOKEN_TO_CHECK: usize = 5;

static OPENING_QUOTES: [&str; 5] = ["\"", "“", "„", "»", "«"];
static ENDING_QUOTES: [&str; 5] = ["\"", "“", "”", "»", "«"];
static SINGLE_QUOTES: [&str; 6] = ["'", "‚", "‘", "’", "'", "›"];

/// `(SUB|EIG|VER|ADJ):.*` (full match per reading POS tag).
static POS_TO_CHECK: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(SUB|EIG|VER|ADJ):.*$").unwrap());
/// `(PRO|A(RT|DV)|VER:(AUX|MOD)):.*` (full match per reading POS tag).
static POS_NOT_TO_CHECK: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(PRO|A(RT|DV)|VER:(AUX|MOD)):.*$").unwrap());
/// `GermanStyleRepeatedWordRule.LETTERS`
static LETTERS: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[A-Za-zÄÖÜäöüß]+$").unwrap());

/// `GermanStyleRepeatedWordRule.isUnknownWord`.
fn is_unknown_word(token: &AnalyzedTokenReadings) -> bool {
    util::is_pos_tag_unknown(token)
        && token.surface().chars().count() > 2
        && LETTERS.is_match(token.surface())
}

/// `GermanStyleRepeatedWordRule.isTokenToCheck`.
fn is_token_to_check(tokens: &[&AnalyzedTokenReadings], n: usize) -> bool {
    if n > 0
        && n < tokens.len() - 1
        && (tokens[n + 1].has_pos_tag_starting_with("EIG") || is_unknown_word(tokens[n + 1]))
        && matches!(
            tokens[n].surface(),
            "Frau" | "Fräulein" | "Herr" | "Herrn" | "Lady" | "Mister"
        )
    {
        return false;
    }
    let token = tokens[n];
    (util::matches_pos_tag_regex(token, &POS_TO_CHECK)
        && !util::matches_pos_tag_regex(token, &POS_NOT_TO_CHECK)
        || is_unknown_word(token))
        && !matches!(
            token.surface(),
            "sicher" | "weit" | "Sie" | "Ich" | "Euch" | "Eure" | "Der" | "all"
        )
}

/// `GermanStyleRepeatedWordRule.isTokenPair`.
fn is_token_pair(tokens: &[&AnalyzedTokenReadings], n: usize, before: bool) -> bool {
    if before {
        n > 2
            && n < tokens.len()
            && ((tokens[n - 2].has_pos_tag_starting_with("SUB")
                && tokens[n - 1].has_pos_tag_starting_with("PRP")
                && tokens[n].has_pos_tag_starting_with("SUB"))
                || (tokens[n - 2].surface() == "hart"
                    && tokens[n - 1].surface() == "auf"
                    && tokens[n].surface() == "hart")
                || (tokens[n - 2].surface() == "dicht"
                    && tokens[n - 1].surface() == "an"
                    && tokens[n].surface() == "dicht")
                || (tokens[n - 2].surface() == "fressen"
                    && tokens[n - 1].surface() == "und"
                    && tokens[n].surface() == "gefressen"))
    } else {
        n > 0
            && n < tokens.len() - 2
            && ((tokens[n].has_pos_tag_starting_with("SUB")
                && tokens[n + 1].has_pos_tag_starting_with("PRP")
                && tokens[n + 2].has_pos_tag_starting_with("SUB"))
                || (tokens[n].surface() == "hart"
                    && tokens[n + 1].surface() == "auf"
                    && tokens[n + 2].surface() == "hart")
                || (tokens[n].surface() == "dicht"
                    && tokens[n + 1].surface() == "an"
                    && tokens[n + 2].surface() == "dicht")
                || (tokens[n].surface() == "fressen"
                    && tokens[n + 1].surface() == "und"
                    && tokens[n + 2].surface() == "gefressen"))
    }
}

/// `GermanStyleRepeatedWordRule.isExceptionPair`.
fn is_exception_pair(token1: &AnalyzedTokenReadings, token2: &AnalyzedTokenReadings) -> bool {
    (token1.has_lemma("nah") && token1.has_lemma("nächst") && !token2.has_lemma("nächst"))
        || (token2.has_lemma("nah") && token2.has_lemma("nächst") && !token1.has_lemma("nächst"))
        || (token1.has_lemma("gut")
            && ((token1.surface().starts_with("gut") && !token2.surface().starts_with("gut"))
                || (token2.surface().starts_with("gut") && !token1.surface().starts_with("gut"))))
}

/// `AbstractStyleRepeatedWordRule.hasBreakToken` (listings are excluded).
fn has_break_token(tokens: &[&AnalyzedTokenReadings]) -> bool {
    tokens
        .iter()
        .take(MAX_TOKEN_TO_CHECK)
        .any(|t| matches!(t.surface(), "-" | "—" | "–"))
}

fn is_in_quotes(tokens: &[&AnalyzedTokenReadings], i: usize) -> bool {
    i > 0
        && (OPENING_QUOTES.contains(&tokens[i - 1].surface())
            || SINGLE_QUOTES.contains(&tokens[i - 1].surface()))
        && i < tokens.len() - 1
        && (ENDING_QUOTES.contains(&tokens[i + 1].surface())
            || SINGLE_QUOTES.contains(&tokens[i + 1].surface()))
}

/// `AbstractStyleRepeatedWordRule.isQuestionResponse` (question/answer pairs
/// one sentence apart are not repetitions).
fn is_question_response(
    n_act: usize,
    n_test: usize,
    token_list: &[Vec<&AnalyzedTokenReadings>],
) -> bool {
    let dist = n_act as isize - n_test as isize;
    if dist != 1 && dist != -1 {
        return false;
    }
    let act_tokens = &token_list[n_act];
    let test_tokens = &token_list[n_test];
    if act_tokens.len() < 2 || test_tokens.len() < 2 {
        return false;
    }
    let act_token = if ENDING_QUOTES.contains(&act_tokens[act_tokens.len() - 1].surface()) {
        act_tokens[act_tokens.len() - 2].surface()
    } else {
        act_tokens[act_tokens.len() - 1].surface()
    };
    let test_token = if ENDING_QUOTES.contains(&test_tokens[test_tokens.len() - 1].surface()) {
        test_tokens[test_tokens.len() - 2].surface()
    } else {
        test_tokens[test_tokens.len() - 1].surface()
    };
    (act_token == "?" && test_token != "?") || (test_token == "?" && act_token != "?")
}

/// `AbstractStyleRepeatedWordRule.isTokenInSentence` (inner variant with the
/// `notCheck` position of the token under test).
fn is_token_in_sentence(
    test_token: &AnalyzedTokenReadings,
    tokens: &[&AnalyzedTokenReadings],
    not_check: isize,
    is_direct_speech: bool,
) -> bool {
    let lemmas: Vec<&str> = test_token
        .readings
        .iter()
        .filter_map(|r| r.stem.as_deref())
        .collect();
    let mut is_direct_speech = is_direct_speech;
    for i in 0..tokens.len() {
        if EXCLUDE_DIRECT_SPEECH
            && !is_direct_speech
            && OPENING_QUOTES.contains(&tokens[i].surface())
            && i < tokens.len() - 1
            && !tokens[i + 1].whitespace_before
        {
            is_direct_speech = true;
        } else if EXCLUDE_DIRECT_SPEECH
            && is_direct_speech
            && ENDING_QUOTES.contains(&tokens[i].surface())
            && i > 1
            && !tokens[i].whitespace_before
        {
            is_direct_speech = false;
        } else if i as isize != not_check
            && !is_direct_speech
            && !is_in_quotes(tokens, i)
            && is_token_to_check(tokens, i)
        {
            let any_lemma = !lemmas.is_empty()
                && tokens[i]
                    .readings
                    .iter()
                    .any(|r| r.stem.as_deref().is_some_and(|l| lemmas.contains(&l)));
            if (any_lemma && !is_exception_pair(test_token, tokens[i])) || is_part_of_word() {
                if not_check >= 0 {
                    if not_check == i as isize - 2 {
                        return !is_token_pair(tokens, i, true);
                    } else if not_check == i as isize + 2 {
                        return !is_token_pair(tokens, i, false);
                    } else if (not_check == i as isize + 1 || not_check == i as isize - 1)
                        && test_token.surface() == tokens[i].surface()
                    {
                        return false;
                    }
                }
                return true;
            }
        }
    }
    false
}

/// `AbstractStyleRepeatedWordRule.isPartOfWord` base implementation (the
/// German compound variant needs the speller and is user-config-only).
fn is_part_of_word() -> bool {
    false
}

/// `AbstractStyleRepeatedWordRule.getStartsWithDirectSpeech`.
fn get_starts_with_direct_speech(
    n: usize,
    sentences: &[AnalyzedSentence],
    is_direct_speech: bool,
) -> bool {
    if !EXCLUDE_DIRECT_SPEECH || n == 0 {
        return false;
    }
    let mut is_direct_speech = is_direct_speech;
    let sentence = sentences[n - 1].tokens_without_whitespace();
    for i in 0..sentence.len() {
        let token = sentence[i];
        if !is_direct_speech
            && OPENING_QUOTES.contains(&token.surface())
            && i < sentence.len() - 1
            && !sentence[i + 1].whitespace_before
        {
            is_direct_speech = true;
        } else if is_direct_speech
            && ENDING_QUOTES.contains(&token.surface())
            && i > 1
            && !token.whitespace_before
        {
            is_direct_speech = false;
        }
    }
    is_direct_speech
}

/// `GermanStyleRepeatedWordRule` (`STYLE_REPEATED_WORD_RULE_DE`) text-level
/// check over all sentences, in Java's order.
pub fn style_repeated_word_rule_de(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let max_distance = MAX_DISTANCE_OF_SENTENCES;
    let mut token_list: Vec<Vec<&AnalyzedTokenReadings>> = Vec::new();
    let mut is_ds_list: Vec<bool> = Vec::new();
    let mut pos = 0usize;
    let mut starts_with_direct_speech = false;
    for n in 0..max_distance.max(0) as usize {
        if n >= sentences.len() {
            break;
        }
        token_list.push(sentences[n].tokens_without_whitespace());
        starts_with_direct_speech =
            get_starts_with_direct_speech(n, sentences, starts_with_direct_speech);
        is_ds_list.push(starts_with_direct_speech);
    }
    let mut is_direct_speech = false;
    for (n, sentence) in sentences.iter().enumerate() {
        if (n as isize) + max_distance < sentences.len() as isize {
            let idx = (n as isize + max_distance) as usize;
            token_list.push(sentences[idx].tokens_without_whitespace());
            starts_with_direct_speech =
                get_starts_with_direct_speech(idx, sentences, starts_with_direct_speech);
            is_ds_list.push(starts_with_direct_speech);
        }
        if token_list.len() > (2 * max_distance + 1) as usize {
            token_list.remove(0);
            is_ds_list.remove(0);
        }
        let mut n_tok = max_distance;
        if (n as isize) < max_distance {
            n_tok = n as isize;
        } else if n as isize >= sentences.len() as isize - max_distance {
            n_tok = token_list.len() as isize - (sentences.len() - n) as isize;
        }
        if n_tok < 0 {
            continue;
        }
        let n_tok = n_tok as usize;
        if has_break_token(&token_list[n_tok]) {
            pos += sentence.text.len();
            continue;
        }
        let tokens = &token_list[n_tok];
        for i in 0..tokens.len() {
            let token = tokens[i];
            if EXCLUDE_DIRECT_SPEECH
                && !is_direct_speech
                && OPENING_QUOTES.contains(&token.surface())
                && i < tokens.len() - 1
                && !tokens[i + 1].whitespace_before
            {
                is_direct_speech = true;
            } else if EXCLUDE_DIRECT_SPEECH
                && is_direct_speech
                && ENDING_QUOTES.contains(&token.surface())
                && i > 1
                && !token.whitespace_before
            {
                is_direct_speech = false;
            } else if !is_direct_speech && !is_in_quotes(tokens, i) && is_token_to_check(tokens, i)
            {
                let mut is_repeated = 0;
                if is_token_in_sentence(token, tokens, i as isize, is_ds_list[n_tok]) {
                    is_repeated = 1;
                }
                let mut j = n_tok as isize - 1;
                while is_repeated == 0 && j >= 0 && j >= n_tok as isize - max_distance {
                    if !is_question_response(n_tok, j as usize, &token_list)
                        && is_token_in_sentence(
                            token,
                            &token_list[j as usize],
                            -1,
                            is_ds_list[j as usize],
                        )
                    {
                        is_repeated = 2;
                    }
                    j -= 1;
                }
                let mut j = n_tok as isize + 1;
                while is_repeated == 0
                    && j < token_list.len() as isize
                    && j <= n_tok as isize + max_distance
                {
                    if !is_question_response(n_tok, j as usize, &token_list)
                        && is_token_in_sentence(
                            token,
                            &token_list[j as usize],
                            -1,
                            is_ds_list[j as usize],
                        )
                    {
                        is_repeated = 3;
                    }
                    j += 1;
                }
                if is_repeated != 0 {
                    let msg = match is_repeated {
                        1 => MESSAGE_SAME_SENTENCE,
                        2 => MESSAGE_SENTENCE_BEFORE,
                        _ => MESSAGE_SENTENCE_AFTER,
                    };
                    rule_matches.push(
                        Match::new(
                            RULE_ID,
                            Option::<String>::None,
                            msg,
                            Option::<String>::None,
                            TextRange::new(pos + token.start_pos, pos + token.end_pos()),
                            Vec::new(),
                            CATEGORY_ID,
                            CATEGORY_NAME,
                        )
                        .with_metadata(DESCRIPTION, "style", 0),
                    );
                }
            }
        }
        pos += sentence.text.len();
    }
    rule_matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::AnalyzedToken;

    fn token_at(
        surface: &str,
        stem: Option<&str>,
        pos: Option<&str>,
        start: usize,
    ) -> AnalyzedTokenReadings {
        let mut token = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            surface,
            stem.map(str::to_string),
            pos.map(str::to_string),
        )]);
        token.start_pos = start;
        token.raw_byte_len = surface.len();
        token.is_whitespace = surface.chars().all(char::is_whitespace);
        token
    }

    /// Tokens of `text` are whitespace-separated (positions computed from the
    /// text), each token's stem/POS comes from `readings`.
    fn sentence(text: &str, readings: &[(&str, Option<&str>, Option<&str>)]) -> AnalyzedSentence {
        let mut tokens = Vec::new();
        let mut pos = 0usize;
        for (surface, stem, pos_tag) in readings {
            let start = text[pos..].find(surface).expect("token in text") + pos;
            let mut token = token_at(surface, *stem, *pos_tag, start);
            token.whitespace_before = start > 0 && text[..start].ends_with(' ');
            if start > pos {
                let mut ws = token_at(&text[pos..start], None, None, pos);
                ws.whitespace_before = pos > 0 && text[..pos].ends_with(' ');
                tokens.push(ws);
            }
            tokens.push(token);
            pos = start + surface.len();
        }
        AnalyzedSentence {
            text: text.to_string(),
            offset: 0,
            tokens,
            pre_disambig_tokens: Vec::new(),
            pre_disambig_detached: Vec::new(),
        }
    }

    #[test]
    fn detects_same_sentence_repetition() {
        let text = "Ich gehe danach gehe.";
        let s = sentence(
            text,
            &[
                ("Ich", Some("ich"), Some("PRO:PER:NOM:SIN:1P")),
                ("gehe", Some("gehen"), Some("VER:FIN:1P:SIN:PRS:IND")),
                ("danach", Some("danach"), Some("ADV")),
                ("gehe", Some("gehen"), Some("VER:FIN:1P:SIN:PRS:IND")),
                (".", None, Some("PCT")),
            ],
        );
        let matches = style_repeated_word_rule_de(&[s]);
        assert_eq!(matches.len(), 2, "{matches:?}");
        assert_eq!(matches[0].message, MESSAGE_SAME_SENTENCE);
        assert_eq!((matches[0].range.start, matches[0].range.end), (4, 8));
        assert_eq!((matches[1].range.start, matches[1].range.end), (16, 20));
    }

    #[test]
    fn excludes_direct_speech() {
        // Both occurrences inside the quotes: no match. Without quotes: two.
        let quoted = sentence(
            "„gehe los“ und gehe",
            &[
                ("„", None, Some("PCT")),
                ("gehe", Some("gehen"), Some("VER:FIN:1P:SIN:PRS:IND")),
                ("los", Some("los"), Some("ADV")),
                ("“", None, Some("PCT")),
                ("und", Some("und"), Some("KON")),
                ("gehe", Some("gehen"), Some("VER:FIN:1P:SIN:PRS:IND")),
            ],
        );
        assert!(style_repeated_word_rule_de(&[quoted]).is_empty());
        let plain = sentence(
            "gehe los und gehe",
            &[
                ("gehe", Some("gehen"), Some("VER:FIN:1P:SIN:PRS:IND")),
                ("los", Some("los"), Some("ADV")),
                ("und", Some("und"), Some("KON")),
                ("gehe", Some("gehen"), Some("VER:FIN:1P:SIN:PRS:IND")),
            ],
        );
        let matches = style_repeated_word_rule_de(&[plain]);
        assert_eq!(matches.len(), 2, "{matches:?}");
    }
}
