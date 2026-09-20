//! `Catalan.filterRuleMatches` (`adjustCatalanMatch`) and
//! `Catalan.filterRuleMatchesAfterOverlapping` (`RuleMatch.trimMatchEnds`),
//! wired into the engine's `JLanguageTool.filterMatches` post-processing
//! order (D-146).

use std::collections::HashSet;

use lt_core::{AnalyzedSentence, Match, Sentence, Suggestion};

fn full_id(m: &Match) -> String {
    match &m.sub_id {
        Some(sub) => format!("{}[{}]", m.rule_id, sub),
        None => m.rule_id.clone(),
    }
}

/// The `getFromPosSentence`/`getToPosSentence` pair Java sets in
/// `adjustRuleMatchPos` (sentence-relative offsets, `-1` when unknown).
fn sentence_positions(m: &Match, sentences: &[Sentence]) -> (isize, isize) {
    for s in sentences {
        if m.range.start >= s.range.start && m.range.start < s.range.end {
            return (
                (m.range.start - s.range.start) as isize,
                (m.range.end - s.range.start) as isize,
            );
        }
    }
    (-1, -1)
}

fn from_pos_sentence(m: &Match, sentences: &[Sentence]) -> isize {
    sentence_positions(m, sentences).0
}

fn to_pos_sentence(m: &Match, sentences: &[Sentence]) -> isize {
    sentence_positions(m, sentences).1
}

/// The analyzed sentence matching a raw `Sentence` (`offset` is absolute) or
/// its tokens for `hasTypographicApostrophe`.
fn analyzed_for<'a>(
    s: &Sentence,
    analyzed_sentences: &'a [AnalyzedSentence],
) -> Option<&'a AnalyzedSentence> {
    analyzed_sentences
        .iter()
        .find(|a| a.offset == s.range.start)
}

/// `Catalanish CA_OLD_DIACRITICS` full-match check.
fn ca_old_diacritics_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)^.*\b(sóc|dóna|dónes|vénen|véns|fóra|adéu|féu|desféu|vés|contrapèl)\b.*$",
        )
        .unwrap()
    })
}

/// `Catalan.removeOldDiacritics`.
fn remove_old_diacritics(s: &str) -> String {
    s.replace("contrapèl", "contrapel")
        .replace("Contrapèl", "Contrapel")
        .replace("vés", "ves")
        .replace("féu", "feu")
        .replace("desféu", "desfeu")
        .replace("adéu", "adeu")
        .replace("dóna", "dona")
        .replace("dónes", "dones")
        .replace("sóc", "soc")
        .replace("vénen", "venen")
        .replace("fóra", "fora")
        .replace("Vés", "Ves")
        .replace("Féu", "Feu")
        .replace("Desféu", "Desfeu")
        .replace("Adéu", "Adeu")
        .replace("Dóna", "Dona")
        .replace("Dónes", "Dones")
        .replace("Sóc", "Soc")
        .replace("Vénen", "Venen")
        .replace("Véns", "Vens")
        .replace("Fóra", "Fora")
}

fn possessius_v_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\b([mtsMTS]e)v(a|es)\b").unwrap())
}

fn possessius_v_upper_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\b([MTS]E)V(A|ES)\b").unwrap())
}

/// `Catalan.adjustCatalanMatch`.
fn adjust_catalan_match(
    m: &Match,
    sentences: &[Sentence],
    analyzed_sentences: &[AnalyzedSentence],
    enabled_rules: &HashSet<&str>,
) -> Match {
    // `RuleMatch.getOriginalErrorStr()`: filled by the pattern/regexp rule
    // constructors from the underlined range (Java falls back to the offset
    // positions while the sentence positions are unset); filter-rebuilt
    // matches keep it empty.
    let error_str: &str = m.original_error_str.as_deref().unwrap_or("");

    // Avoid two white spaces after removing a word
    if m.suggestions.len() == 1 && m.suggestions[0].value.is_empty() {
        if let Some(sentence) = sentences
            .iter()
            .find(|s| m.range.start >= s.range.start && m.range.start < s.range.end)
        {
            let sentence_text = &sentence.text;
            let from_sent = (m.range.start - sentence.range.start) as isize;
            let to_sent = (m.range.end - sentence.range.start) as isize;
            let bytes = sentence_text.as_bytes();
            if from_sent >= 0
                && to_sent >= 0
                && (to_sent as usize) < bytes.len()
                && (from_sent == 0 || bytes[(from_sent - 1) as usize] == b' ')
                && bytes[to_sent as usize] == b' '
            {
                let mut new_match = m.clone();
                new_match.range = lt_core::TextRange::new(m.range.start, m.range.end + 1);
                new_match.suggestions = vec![Suggestion {
                    value: String::new(),
                    short_description: None,
                }];
                // Java builds this match with the plain `RuleMatch(rule,
                // sentence, …)` constructor, which resets the match type to
                // `Other` (the rule-level `Hint` is lost).
                new_match.match_type = "Other".to_string();
                return new_match;
            }
        }
    }

    let has_typographic_apostrophe = sentences
        .iter()
        .find(|s| m.range.start >= s.range.start && m.range.start < s.range.end)
        .and_then(|s| analyzed_for(s, analyzed_sentences))
        .is_some_and(|a| a.tokens.iter().any(|t| t.has_typographic_apostrophe));

    let suggested_replacements: Vec<String> =
        m.suggestions.iter().map(|s| s.value.clone()).collect();
    let mut new_replacements: Vec<Suggestion> = Vec::new();
    for (idx, suggested_replacement) in suggested_replacements.iter().enumerate() {
        let mut new_repl_str = suggested_replacement.clone();
        if error_str.encode_utf16().count() > 2
            && error_str.ends_with('\'')
            && !new_repl_str.ends_with('\'')
            && !new_repl_str.ends_with('’')
        {
            new_repl_str.push(' ');
        }
        if !new_repl_str.eq_ignore_ascii_case("després")
            && enabled_rules.contains("EXIGEIX_ACCENTUACIO_GENERAL")
        {
            if new_repl_str.contains('é')
                && suggested_replacements.contains(&new_repl_str.replace('é', "è"))
            {
                continue;
            }
            if new_repl_str.contains('É')
                && suggested_replacements.contains(&new_repl_str.replace('É', "È"))
            {
                continue;
            }
        } else if enabled_rules.contains("EXIGEIX_ACCENTUACIO_VALENCIANA") {
            if new_repl_str.contains('è')
                && suggested_replacements.contains(&new_repl_str.replace('è', "é"))
            {
                continue;
            }
            if new_repl_str.contains('È')
                && suggested_replacements.contains(&new_repl_str.replace('È', "É"))
            {
                continue;
            }
        }
        if (enabled_rules.contains("APOSTROF_TIPOGRAFIC") || has_typographic_apostrophe)
            && new_repl_str.chars().count() > 1
            && !enabled_rules.contains("APOSTROF_RECTE")
        {
            new_repl_str = new_repl_str.replace('\'', "’");
        }
        if enabled_rules.contains("EXIGEIX_POSSESSIUS_U") && new_repl_str.chars().count() > 3 {
            new_repl_str = possessius_v_re()
                .replace_all(&new_repl_str, "${1}u${2}")
                .into_owned();
            new_repl_str = possessius_v_upper_re()
                .replace_all(&new_repl_str, "${1}U${2}")
                .into_owned();
            new_repl_str = new_repl_str
                .replace("feina", "faena")
                .replace("feiner", "faener")
                .replace("feinera", "faenera");
        }
        let changed = !enabled_rules.contains("DIACRITICS_TRADITIONAL_RULES")
            && ca_old_diacritics_re().is_match(&new_repl_str);
        let value = if changed {
            remove_old_diacritics(&new_repl_str)
        } else {
            new_repl_str
        };
        if !new_replacements.iter().any(|s| s.value == value) {
            new_replacements.push(Suggestion {
                value,
                short_description: m.suggestions[idx].short_description.clone(),
            });
        }
    }
    let mut adjusted = m.clone();
    adjusted.suggestions = new_replacements;
    adjusted
}

/// `RuleMatch.trimMatchEnds`.
fn trim_match_ends(
    m: &Match,
    sentences: &[Sentence],
    analyzed_sentences: &[AnalyzedSentence],
) -> Match {
    let mut replacements: Vec<String> = m.suggestions.iter().map(|s| s.value.clone()).collect();
    if replacements.is_empty() {
        return m.clone();
    }
    let sentence = sentences
        .iter()
        .find(|s| m.range.start >= s.range.start && m.range.start < s.range.end);
    let error_str: String = match sentence {
        Some(s) => {
            let text = analyzed_for(s, analyzed_sentences)
                .map(|a| a.text.as_str())
                .unwrap_or(s.text.as_str());
            let from_sent = m.range.start - s.range.start;
            let to_sent = m.range.end - s.range.start;
            if to_sent <= text.len() && from_sent < to_sent {
                text[from_sent..to_sent].to_string()
            } else {
                String::new()
            }
        }
        None => String::new(),
    };
    if error_str.is_empty() || lt_core::is_whitespace(&error_str) {
        return m.clone();
    }
    let mut from_pos = m.range.start;
    let mut to_pos = m.range.end;
    let mut error_str = error_str;
    let mut changed = true;
    while changed {
        changed = false;
        // Try trimming from the end
        if let Some(last_space_idx) = error_str.rfind(' ') {
            if last_space_idx > 0 {
                let last_token = &error_str[last_space_idx + 1..];
                let end_suffix = format!(" {last_token}");
                let all_end_with = replacements.iter().all(|r| r.ends_with(&end_suffix));
                if all_end_with {
                    let error_trim_len = error_str.len() - last_space_idx;
                    replacements = replacements
                        .iter()
                        .map(|r| r[..r.len() - end_suffix.len()].to_string())
                        .collect();
                    to_pos -= error_trim_len;
                    error_str = error_str[..last_space_idx].to_string();
                    changed = true;
                }
            }
        }
        // Try trimming from the beginning
        if let Some(first_space_idx) = error_str.find(' ') {
            if first_space_idx > 0 {
                let first_token = &error_str[..first_space_idx];
                let start_prefix = format!("{first_token} ");
                let all_start_with = replacements.iter().all(|r| r.starts_with(&start_prefix));
                if all_start_with {
                    let error_trim_len = first_space_idx + 1;
                    replacements = replacements
                        .iter()
                        .map(|r| r[start_prefix.len()..].to_string())
                        .collect();
                    from_pos += error_trim_len;
                    error_str = error_str[error_trim_len..].to_string();
                    changed = true;
                }
            }
        }
    }
    if from_pos == m.range.start && to_pos == m.range.end {
        return m.clone();
    }
    let mut trimmed = m.clone();
    trimmed.range = lt_core::TextRange::new(from_pos, to_pos);
    trimmed.suggestions = replacements
        .into_iter()
        .map(|value| Suggestion {
            value,
            short_description: None,
        })
        .collect();
    trimmed
}

/// `Catalan.filterRuleMatches`.
pub fn catalan_filter_rule_matches(
    matches: Vec<Match>,
    sentences: &[Sentence],
    analyzed_sentences: &[AnalyzedSentence],
    enabled_rules: &HashSet<&str>,
) -> Vec<Match> {
    let mut results: Vec<Match> = Vec::new();
    let mut ignore_match_in_pos: Option<usize> = None;
    let mut previous: Option<&Match> = None;
    for (i, m) in matches.iter().enumerate() {
        // remove rules IGNORE_PROPER_NOUNS and MORFOLOGIK_RULE_CA_ES if they
        // are in the same position
        if m.rule_id == "IGNORE_PROPER_NOUNS" {
            if let Some(prev) = previous {
                if prev.rule_id == "MORFOLOGIK_RULE_CA_ES" && m.range.start == prev.range.start {
                    results.pop();
                    ignore_match_in_pos = None;
                    continue;
                }
            }
            ignore_match_in_pos = Some(m.range.start);
            continue;
        }
        if m.rule_id == "MORFOLOGIK_RULE_CA_ES" && ignore_match_in_pos == Some(m.range.start) {
            ignore_match_in_pos = None;
            continue;
        }
        let id = full_id(m);
        if (id == "FALTA_ELEMENT_ENTRE_VERBS[3]" || id == "FALTA_ELEMENT_ENTRE_VERBS[4]")
            && i + 1 < matches.len()
        {
            let next = &matches[i + 1];
            let next_from = from_pos_sentence(next, sentences);
            if next_from > -1
                && full_id(next) != "FALTA_ELEMENT_ENTRE_VERBS[5]"
                && next_from - to_pos_sentence(m, sentences) < 20
            {
                continue;
            }
        }
        if i > 0
            && full_id(m) == "FALTA_ELEMENT_ENTRE_VERBS[5]"
            && matches[i - 1].rule_id == "FALTA_ELEMENT_ENTRE_VERBS"
        {
            continue;
        }
        results.push(adjust_catalan_match(
            m,
            sentences,
            analyzed_sentences,
            enabled_rules,
        ));
        previous = Some(&matches[i]);
    }
    // `Collections.sort`: stable, by fromPos only.
    results.sort_by_key(|m| m.range.start);
    results
}

/// `Catalan.filterRuleMatchesAfterOverlapping`.
pub fn catalan_filter_rule_matches_after_overlapping(
    matches: Vec<Match>,
    sentences: &[Sentence],
    analyzed_sentences: &[AnalyzedSentence],
) -> Vec<Match> {
    let mut results: Vec<Match> = matches
        .iter()
        .map(|m| trim_match_ends(m, sentences, analyzed_sentences))
        .collect();
    results.sort_by_key(|m| m.range.start);
    results
}
