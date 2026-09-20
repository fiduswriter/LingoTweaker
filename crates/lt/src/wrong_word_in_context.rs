//! Port of `WrongWordInContextRule` / `EnglishWrongWordInContextRule`
//! (`ENGLISH_WRONG_WORD_IN_CONTEXT`): context-dependent confusions from
//! `en/rules/wrongWordInContext.txt`.

use std::path::Path;

use lt_core::{AnalyzedSentence, Match, Result, Suggestion, TextRange};

const RULE_ID: &str = "ENGLISH_WRONG_WORD_IN_CONTEXT";
const DE_RULE_ID: &str = "GERMAN_WRONG_WORD_IN_CONTEXT";
const DE_DESCRIPTION: &str = "Mögliche Wortverwechslungen: $match";
const DE_SHORT_MESSAGE: &str = "Mögliche Wortverwechslung";
const DE_MESSAGE: &str =
    "Mögliche Wortverwechslung: Meinten Sie <suggestion>$SUGGESTION</suggestion> anstatt '$WRONGWORD'?";
const DE_LONG_MESSAGE: &str = "Mögliche Wortverwechslung: Meinten Sie <suggestion>$SUGGESTION</suggestion> (= $EXPLANATION_SUGGESTION) anstatt '$WRONGWORD' (= $EXPLANATION_WRONGWORD)?";
const DE_CATEGORY: &str = "Leicht zu verwechselnde Wörter";
const DESCRIPTION: &str = "commonly confused words: $match";
const SHORT_MESSAGE: &str = "Possibly confused word";
const MESSAGE: &str =
    "Possibly confused word: Did you mean <suggestion>$SUGGESTION</suggestion> instead of '$WRONGWORD'?";
const LONG_MESSAGE: &str = "Possibly confused word: Did you mean <suggestion>$SUGGESTION</suggestion> (= $EXPLANATION_SUGGESTION) instead of '$WRONGWORD' (= $EXPLANATION_WRONGWORD)?";

struct ContextWords {
    words: [regex::Regex; 2],
    contexts: [regex::Regex; 2],
    matches: [String; 2],
    explanations: [String; 2],
    word_prefixes: [String; 2],
}

fn add_boundaries(str: &str) -> String {
    let (ignore_case, rest) = match str.strip_prefix("(?i)") {
        Some(rest) => ("(?i)", rest),
        None => ("", str),
    };
    // Java `Pattern`'s `\b` is ASCII-only (no UNICODE_CHARACTER_CLASS), so a
    // context or word ending in a non-ASCII letter (e.g. `café`) never
    // matches in Java. Keep the Rust regex Unicode flag off for the boundary.
    format!("{ignore_case}(?-u:\\b)({rest})(?-u:\\b)")
}

fn set_word(word: &str) -> (regex::Regex, String) {
    let re = regex::Regex::new(&add_boundaries(word)).expect("wrong-word pattern");
    let pattern = word
        .strip_prefix("(?i)")
        .or_else(|| word.strip_prefix("(?-i)"))
        .unwrap_or(word);
    (re, extract_word_prefix(pattern))
}

fn set_context(context: &str) -> regex::Regex {
    regex::Regex::new(&add_boundaries(context)).expect("wrong-word context")
}

fn split_top_level_alternatives(pattern: &str) -> Vec<String> {
    let bytes = pattern.as_bytes();
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '\\' {
            i += 2;
            continue;
        }
        if c == '(' || c == '[' {
            depth += 1;
        } else if c == ')' || c == ']' {
            depth -= 1;
        } else if c == '|' && depth == 0 {
            parts.push(pattern[start..i].to_string());
            start = i + 1;
        }
        i += 1;
    }
    parts.push(pattern[start..].to_string());
    parts
}

fn extract_simple_prefix(alt: &str) -> String {
    let bytes = alt.as_bytes();
    let mut sb = String::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '\\' && i + 1 < bytes.len() {
            i += 1;
            let escaped = bytes[i] as char;
            if i + 1 < bytes.len() && (bytes[i + 1] as char == '?' || bytes[i + 1] as char == '*') {
                break;
            }
            sb.push(escaped);
            if i + 1 < bytes.len() && bytes[i + 1] as char == '+' {
                i += 1;
            }
        } else if "[(|.^$\\{".contains(c) {
            break;
        } else if c == '?' || c == '*' {
            if !sb.is_empty() {
                sb.pop();
            }
            break;
        } else if c == '+' {
            break;
        } else {
            if i + 1 < bytes.len() && (bytes[i + 1] as char == '?' || bytes[i + 1] as char == '*') {
                break;
            }
            sb.push(c);
        }
        i += 1;
    }
    sb
}

fn common_prefix(a: &str, b: &str) -> String {
    let mut out = String::new();
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca != cb {
            break;
        }
        out.push(ca);
    }
    out
}

/// `ContextWords.extractWordPrefix` (MIN_PREFIX_LENGTH = 3).
fn extract_word_prefix(pattern: &str) -> String {
    let alternatives = split_top_level_alternatives(pattern);
    let mut prefixes = Vec::new();
    for alt in &alternatives {
        let p = extract_simple_prefix(alt);
        if p.chars().count() < 3 {
            return String::new();
        }
        prefixes.push(p.to_lowercase());
    }
    let Some(first) = prefixes.first() else {
        return String::new();
    };
    let mut common = first.clone();
    for prefix in &prefixes[1..] {
        common = common_prefix(&common, prefix);
        if common.chars().count() < 3 {
            return String::new();
        }
    }
    common
}

impl ContextWords {
    fn could_match_sentence(&self, sentence_lower: &str) -> bool {
        let word0 =
            self.word_prefixes[0].is_empty() || sentence_lower.contains(&self.word_prefixes[0]);
        let word1 =
            self.word_prefixes[1].is_empty() || sentence_lower.contains(&self.word_prefixes[1]);
        word0 || word1
    }
}

fn load_context_words(path: &Path) -> Result<Vec<ContextWords>> {
    let text = lt_data::fs::read_to_string(path)
        .map_err(|e| lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display())))?;
    let mut set = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let column: Vec<&str> = line.split('\t').collect();
        if column.len() < 6 {
            continue;
        }
        let (words0, prefix0) = set_word(column[0]);
        let (words1, prefix1) = set_word(column[1]);
        let mut explanations = [String::new(), String::new()];
        if column.len() > 6 {
            explanations[0] = column[6].to_string();
            if column.len() > 7 {
                explanations[1] = column[7].to_string();
            }
        }
        set.push(ContextWords {
            words: [words0, words1],
            contexts: [set_context(column[4]), set_context(column[5])],
            matches: [column[2].to_string(), column[3].to_string()],
            explanations,
            word_prefixes: [prefix0, prefix1],
        });
    }
    Ok(set)
}

/// `StringTools.preserveCase`.
fn preserve_case(input: &str, model: &str) -> String {
    if model.is_empty() {
        return input.to_string();
    }
    if lt_tagger::is_capitalized_word(model) {
        return lt_tagger::uppercase_first_char(&input.to_lowercase());
    }
    if lt_spell::morfologik::is_all_uppercase(model) {
        return input.to_uppercase();
    }
    input.to_string()
}

/// `WrongWordInContextRule.getMessage`.
fn build_message(
    config: &WwicConfig,
    wrong_word: &str,
    suggestion: &str,
    explanation_suggestion: &str,
    explanation_wrong_word: &str,
) -> String {
    let template = if explanation_suggestion.is_empty() || explanation_wrong_word.is_empty() {
        config.message
    } else {
        config.long_message
    };
    template
        .replacen("$SUGGESTION", suggestion, 1)
        .replacen("$WRONGWORD", wrong_word, 1)
        .replacen("$EXPLANATION_SUGGESTION", explanation_suggestion, 1)
        .replacen("$EXPLANATION_WRONGWORD", explanation_wrong_word, 1)
}

/// `StringTools.toId` for the specific-id rule.
fn to_id(input: &str, german: bool) -> String {
    let mut normalized = input
        .trim()
        .to_uppercase()
        .replace(' ', "_")
        .replace('\'', "_Q_");
    if german {
        normalized = normalized
            .replace('Ä', "AE")
            .replace('Ü', "UE")
            .replace('Ö', "OE");
    }
    normalized
        .chars()
        .map(|c| {
            if c.is_ascii_uppercase()
                || ('\u{c0}'..='\u{d6}').contains(&c)
                || ('\u{d8}'..='\u{de}').contains(&c)
            {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Per-language strings of `WrongWordInContextRule`.
#[derive(Clone, Copy)]
struct WwicConfig {
    rule_id: &'static str,
    description: &'static str,
    short_message: &'static str,
    message: &'static str,
    long_message: &'static str,
    category_id: &'static str,
    category_name: &'static str,
    german: bool,
    /// Spanish `setMatchLemmmas()`
    match_lemmas: bool,
    issue_type: &'static str,
}

pub struct WrongWordInContextRule {
    context_words: Vec<ContextWords>,
    config: WwicConfig,
}

impl WrongWordInContextRule {
    pub fn english(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            context_words: load_context_words(&data_dir.join("en/rules/wrongWordInContext.txt"))?,
            config: WwicConfig {
                rule_id: RULE_ID,
                description: DESCRIPTION,
                short_message: SHORT_MESSAGE,
                message: MESSAGE,
                long_message: LONG_MESSAGE,
                category_id: "CONFUSED_WORDS",
                category_name: "Commonly Confused Words",
                german: false,
                match_lemmas: false,
                issue_type: "misspelling",
            },
        })
    }

    /// `GermanWrongWordInContextRule`: `de/rules/wrongWordInContext.txt`.
    pub fn german(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            context_words: load_context_words(&data_dir.join("de/rules/wrongWordInContext.txt"))?,
            config: WwicConfig {
                rule_id: DE_RULE_ID,
                description: DE_DESCRIPTION,
                short_message: DE_SHORT_MESSAGE,
                message: DE_MESSAGE,
                long_message: DE_LONG_MESSAGE,
                category_id: "CONFUSED_WORDS",
                category_name: DE_CATEGORY,
                german: true,
                match_lemmas: false,
                issue_type: "misspelling",
            },
        })
    }

    /// `SpanishWrongWordInContextRule` (`SPANISH_WRONG_WORD_IN_CONTEXT`).
    pub fn spanish(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            context_words: load_context_words(
                &data_dir.join("es/rules/wrongWordInContext.txt"),
            )?,
            config: WwicConfig {
                rule_id: "SPANISH_WRONG_WORD_IN_CONTEXT",
                description: "Confusión según el contexto: $match",
                short_message: "Posible confusión",
                message: "¿Quería decir <suggestion>$SUGGESTION</suggestion> en vez de \"$WRONGWORD\"?",
                long_message: "¿Quería decir <suggestion>$SUGGESTION</suggestion> ($EXPLANATION_SUGGESTION) en vez de \"$WRONGWORD\" ($EXPLANATION_WRONGWORD)?",
                category_id: "CONFUSED_WORDS",
                category_name: "Confusiones",
                german: false,
                match_lemmas: true,
                issue_type: "grammar",
            },
        })
    }

    /// `CatalanWrongWordInContextRule`: `ca/rules/wrongWordInContext.txt`,
    /// category "Z) Confusions", `setMatchLemmmas`, issue type grammar.
    pub fn catalan(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            context_words: load_context_words(
                &data_dir.join("ca/rules/wrongWordInContext.txt"),
            )?,
            config: WwicConfig {
                rule_id: "CATALAN_WRONG_WORD_IN_CONTEXT",
                description: "Confusió segons el context: $match",
                short_message: "Possible confusió",
                message: "¿Volíeu dir <suggestion>$SUGGESTION</suggestion> en lloc de \"$WRONGWORD\"?",
                long_message: "¿Volíeu dir <suggestion>$SUGGESTION</suggestion> ($EXPLANATION_SUGGESTION) en lloc de \"$WRONGWORD\" ($EXPLANATION_WRONGWORD)?",
                category_id: "CONFUSED_WORDS",
                category_name: "Z) Confusions",
                german: false,
                match_lemmas: true,
                issue_type: "grammar",
            },
        })
    }

    /// `PortugueseWrongWordInContextRule`: `pt/rules/wrongWordInContext.txt`,
    /// Portuguese messages, category SEMANTICS, issue type grammar.
    pub fn portuguese(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            context_words: load_context_words(
                &data_dir.join("pt/rules/wrongWordInContext.txt"),
            )?,
            config: WwicConfig {
                rule_id: "PORTUGUESE_WRONG_WORD_IN_CONTEXT",
                description: "Confusão de palavra dentro do contexto (p.ex. infligir/infringir, etc.)",
                short_message: "Possível confusão de termos. Verifique.",
                message: "Pretende dizer <suggestion>$SUGGESTION</suggestion> em vez de $WRONGWORD?",
                long_message: "Considere <suggestion>$SUGGESTION</suggestion>, i.e. $EXPLANATION_SUGGESTION, em vez de '$WRONGWORD', i.e. $EXPLANATION_WRONGWORD?",
                category_id: "SEMANTICS",
                category_name: "Semântica",
                german: false,
                match_lemmas: false,
                issue_type: "grammar",
            },
        })
    }

    /// `DutchWrongWordInContextRule`: `nl/rules/wrongWordInContext.txt`,
    /// category CONFUSED_WORDS ("Gemakkelijk te verwarren woorden"), issue
    /// type misspelling.
    pub fn dutch(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            context_words: load_context_words(
                &data_dir.join("nl/rules/wrongWordInContext.txt"),
            )?,
            config: WwicConfig {
                rule_id: "DUTCH_WRONG_WORD_IN_CONTEXT",
                description: "Woordverwarring: $match",
                short_message: "Mogelijk verwarring",
                message: "Mogelijk verwarring: Bedoelde u <suggestion>$SUGGESTION</suggestion> i.p.v. '$WRONGWORD'?",
                long_message: "Mogelijk verwarring: Bedoelde u <suggestion>$SUGGESTION</suggestion> (= $EXPLANATION_SUGGESTION) i.p.v. '$WRONGWORD' (= $EXPLANATION_WRONGWORD)?",
                category_id: "CONFUSED_WORDS",
                category_name: "Gemakkelijk te verwarren woorden",
                german: false,
                match_lemmas: false,
                issue_type: "misspelling",
            },
        })
    }

    /// `WrongWordInContextRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        sentence: &AnalyzedSentence,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let tokens: Vec<&lt_core::AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let sentence_lower = sentence.text.to_lowercase();
        for context_words in &self.context_words {
            if !context_words.could_match_sentence(&sentence_lower) {
                continue;
            }
            let mut matched_word = [false, false];
            let mut matched_pos = [0usize, 0usize];
            let mut token_text = [String::new(), String::new()];
            for (k, token) in tokens.iter().enumerate().skip(1) {
                if token.has_pos_tag("IS_URL") {
                    continue;
                }
                let t = token.surface();
                for idx in 0..2 {
                    if !matched_word[idx] && context_words.words[idx].is_match(t) {
                        matched_word[idx] = true;
                        matched_pos[idx] = k;
                        token_text[idx] = t.to_string();
                    }
                }
                if matched_word[0] && matched_word[1] {
                    break;
                }
            }
            let (found, not_found) = match (matched_word[0], matched_word[1]) {
                (true, false) => (0usize, 1usize),
                (false, true) => (1usize, 0usize),
                _ => continue,
            };
            let start_pos = tokens[matched_pos[found]].start_pos;
            let end_pos = start_pos + token_text[found].len();
            let matched_token = token_text[found].clone();

            let mut matched_context = [false, false];
            for token in tokens.iter().skip(1) {
                if self.config.match_lemmas {
                    // Java `matchLemmas`: the context patterns are tested
                    // against every reading's lemma
                    for reading in &token.readings {
                        let Some(lemma) = reading.stem.as_deref() else {
                            continue;
                        };
                        if lemma.is_empty() {
                            continue;
                        }
                        if !matched_context[found] && context_words.contexts[found].is_match(lemma)
                        {
                            matched_context[found] = true;
                        }
                        if !matched_context[not_found]
                            && context_words.contexts[not_found].is_match(lemma)
                        {
                            matched_context[not_found] = true;
                        }
                        if matched_context[found] && matched_context[not_found] {
                            break;
                        }
                    }
                } else {
                    if !matched_context[found]
                        && context_words.contexts[found].is_match(token.surface())
                    {
                        matched_context[found] = true;
                    }
                    if !matched_context[not_found]
                        && context_words.contexts[not_found].is_match(token.surface())
                    {
                        matched_context[not_found] = true;
                    }
                }
                if matched_context[found] && matched_context[not_found] {
                    break;
                }
            }
            if matched_context[not_found] && !matched_context[found] {
                let original = &context_words.matches[found];
                let replacement = &context_words.matches[not_found];
                let pattern = format!("(?i){original}");
                let repl = match regex::Regex::new(&pattern) {
                    Ok(re) => {
                        let replaced = re.replacen(&matched_token, 1, replacement.as_str());
                        preserve_case(&replaced, &matched_token)
                    }
                    Err(_) => matched_token.clone(),
                };
                let msg = build_message(
                    &self.config,
                    &matched_token,
                    &repl,
                    &context_words.explanations[not_found],
                    &context_words.explanations[found],
                );
                let id = to_id(
                    &format!("{}_{}_{}", self.config.rule_id, matched_token, repl),
                    self.config.german,
                );
                let description = self
                    .config
                    .description
                    .replace("$match", &format!("{matched_token}/{repl}"));
                rule_matches.push(
                    Match::new(
                        id,
                        Option::<String>::None,
                        msg,
                        Some(self.config.short_message.to_string()),
                        TextRange::new(sentence_offset + start_pos, sentence_offset + end_pos),
                        vec![Suggestion {
                            value: repl,
                            short_description: None,
                        }],
                        self.config.category_id,
                        self.config.category_name,
                    )
                    .with_metadata(&description, self.config.issue_type, 0),
                );
            }
        }
        rule_matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::{AnalyzedToken, AnalyzedTokenReadings};
    use lt_data::PathExt as _;

    fn token(surface: &str, pos: &str) -> AnalyzedTokenReadings {
        AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            surface,
            Some(surface.to_lowercase()),
            Some(pos.to_string()),
        )])
    }

    #[test]
    fn german_mine_miene_probe() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !data.join("de/rules/wrongWordInContext.txt").lt_exists() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let rule = WrongWordInContextRule::german(&data).unwrap();
        let text = "Die Miene vom Kugelschreiber ist leer.";
        let mut tokens = vec![token("", "SENT_START")];
        for (surface, pos) in [
            ("Die", "ART:DEF:NOM:SIN:FEM"),
            ("Miene", "SUB:AKK:SIN:FEM"),
            ("vom", "PRP:ART"),
            ("Kugelschreiber", "SUB:DAT:SIN:MAS"),
            ("ist", "VER:3:SIN:PRÄ"),
            ("leer", "ADJ:PRD:GRU"),
        ] {
            tokens.push(token(surface, pos));
        }
        tokens[0].is_sentence_start = true;
        let mut end = token(".", "PKT");
        end.is_sentence_end = true;
        tokens.push(end);
        let mut cursor = 0usize;
        for token in tokens.iter_mut() {
            let surface = token.surface().to_string();
            if !surface.is_empty() {
                let pos = text[cursor..].find(&surface).unwrap() + cursor;
                token.start_pos = pos;
                cursor = pos + surface.len();
            }
        }
        let sentence = AnalyzedSentence {
            text: text.to_string(),
            offset: 0,
            tokens,
            pre_disambig_tokens: Vec::new(),
        };
        let matches = rule.check_sentence(&sentence, 0);
        assert_eq!(matches.len(), 1, "{matches:?}");
        assert_eq!((matches[0].range.start, matches[0].range.end), (4, 9));
        assert_eq!(matches[0].suggestions[0].value, "Mine");
    }
}
