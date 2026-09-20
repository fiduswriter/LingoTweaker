//! `org.languagetool.tagging.disambiguation.ca.CatalanMultitokenDisambiguator`
//! (D-147): after the XML disambiguation, unknown tokens that form a
//! multiword proper name recognized by the `ca-ES_spelling_multitoken.dict`
//! speller get an `NPCNM00` reading with the whole phrase as lemma.

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_spell::morfologik::MorfologikSpeller;

const WINDOW_FORWARD: usize = 10;
const WINDOW_BACKWARD: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchType {
    None,
    ShrinkFromEnd,
    ShrinkFromStart,
}

/// `CatalanMultitokenDisambiguator.disambiguate`.
pub fn disambiguate_multitoken(sentence: &mut AnalyzedSentence, speller: &MorfologikSpeller) {
    let tokens = &mut sentence.tokens;
    let len = tokens.len();
    for i in 1..len {
        if tokens[i].is_whitespace || tokens[i].is_tagged || tokens[i].is_ignore_spelling {
            continue;
        }
        let (from_index, to_index) = to_and_for_indexes(tokens, i);
        let mut found =
            search_in_dict_and_tag(tokens, from_index, to_index, SearchType::None, speller);
        // Forward
        if !found
            && tokens[i]
                .surface()
                .chars()
                .next()
                .is_some_and(char::is_uppercase)
        {
            let from_fwd = i;
            let to_fwd = (i + WINDOW_FORWARD).min(len - 1);
            found = search_in_dict_and_tag(
                tokens,
                from_fwd,
                to_fwd,
                SearchType::ShrinkFromEnd,
                speller,
            );
        }
        // Backward
        if !found {
            let from_bwd = i.saturating_sub(WINDOW_BACKWARD).max(1);
            let to_bwd = i;
            search_in_dict_and_tag(
                tokens,
                from_bwd,
                to_bwd,
                SearchType::ShrinkFromStart,
                speller,
            );
        }
    }
}

fn search_in_dict_and_tag(
    tokens: &mut [AnalyzedTokenReadings],
    from: usize,
    to: usize,
    shrink_from: SearchType,
    speller: &MorfologikSpeller,
) -> bool {
    let mut current_from = from;
    let mut current_to = to;
    while current_to > current_from {
        let text_to_check = get_text_from_to(tokens, current_from, current_to);
        if ["Santa María", "San Agustin"].contains(&text_to_check.as_str()) {
            return false;
        }
        if !text_to_check.ends_with(' ')
            && !text_to_check.starts_with(' ')
            && !text_to_check.is_empty()
            && !speller.is_misspelled(&text_to_check)
        {
            for token in tokens.iter_mut().take(current_to + 1).skip(current_from) {
                if !token.is_whitespace {
                    token.add_reading(AnalyzedToken::new(
                        token.surface().to_string(),
                        Some(text_to_check.clone()),
                        Some("NPCNM00".to_string()),
                    ));
                }
            }
            return true;
        }
        if shrink_from == SearchType::ShrinkFromEnd {
            current_to -= 1;
        } else if shrink_from == SearchType::ShrinkFromStart {
            current_from += 1;
        } else {
            return false;
        }
    }
    false
}

/// `CatalanMultitokenDisambiguator.getToAndForIndexes` (phrases in Title
/// Case, except prepositions/short words).
fn to_and_for_indexes(tokens: &[AnalyzedTokenReadings], start_index: usize) -> (usize, usize) {
    let first_upper = |i: usize| {
        tokens[i]
            .surface()
            .chars()
            .next()
            .is_some_and(char::is_uppercase)
    };
    let short = |i: usize| tokens[i].surface().chars().count() < 3;
    let mut from_index = start_index;
    while from_index > 1
        && (first_upper(from_index - 1)
            || tokens[from_index - 1].is_whitespace
            || short(from_index - 1))
    {
        from_index -= 1;
    }
    while !first_upper(from_index) && from_index < start_index {
        from_index += 1;
    }
    let mut to_index = start_index;
    while to_index < tokens.len() - 1
        && (first_upper(to_index + 1) || tokens[to_index + 1].is_whitespace || short(to_index + 1))
    {
        to_index += 1;
    }
    while !first_upper(to_index) && to_index > start_index {
        to_index -= 1;
    }
    (from_index, to_index)
}

fn get_text_from_to(
    tokens: &[AnalyzedTokenReadings],
    index_from: usize,
    index_to: usize,
) -> String {
    let mut sb = String::new();
    for (i, token) in tokens
        .iter()
        .enumerate()
        .take(index_to + 1)
        .skip(index_from)
    {
        if i > tokens.len() - 1 {
            return String::new();
        }
        sb.push_str(token.surface());
    }
    sb
}
