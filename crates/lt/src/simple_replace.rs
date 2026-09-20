//! Port of `AbstractSimpleReplaceRule2` and its English subclasses:
//! `SimpleReplaceRule` (`EN_SIMPLE_REPLACE`), `AmericanReplaceRule`
//! (`EN_US_SIMPLE_REPLACE`), `BritishReplaceRule` (`EN_GB_SIMPLE_REPLACE`),
//! `EnglishDiacriticsRule` (`EN_DIACRITICS_REPLACE_ORTHOGRAPHY`),
//! `EnglishRedundancyRule` (default off) and `SimpleReplaceProfanityRule`
//! (`PROFANITY`, picky).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::wordutil::is_punctuation_mark;

/// `AbstractSimpleReplaceRule2.CaseSensitivy` (only CI is used by English).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseSensitivity {
    Ci,
    Cs,
}

pub struct SimpleReplaceConfig {
    pub rule_id: &'static str,
    pub description: &'static str,
    pub short: &'static str,
    pub message: &'static str,
    pub suggestions_separator: &'static str,
    pub sub_rule_specific_ids: bool,
    pub case_sensitivity: CaseSensitivity,
    pub category_id: &'static str,
    pub category_name: &'static str,
    pub issue_type: &'static str,
    pub default_off: bool,
    pub picky: bool,
    pub has_suggestions: bool,
    /// `AbstractSimpleReplaceRule2.isCheckingCase()` (`AbstractCheckCaseRule`):
    /// the file's left side is the correct form and the loader rewrites
    /// `FORM[=message]` to `form=FORM\tmessage`; casing is never adjusted
    /// beyond sentence start.
    pub checking_case: bool,
    /// `AbstractCheckCaseRule` sets `setIgnoreShortUppercaseWords(false)`;
    /// all other rules keep the Java default `true`.
    pub ignore_short_uppercase_words: bool,
    /// `isTokenException` override (pt barbarisms / Portugal-Brazilian
    /// replace: proper nouns, English-ignore and immunized tokens).
    pub is_token_exception: TokenException,
}

/// `AbstractSimpleReplaceRule2.isTokenException` variants used by the
/// Portuguese rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenException {
    None,
    /// `atr.hasPosTagStartingWith("NP") || atr.isImmunized()` (pt-PT/pt-BR
    /// replace rules).
    ProperNounOrImmunized,
    /// `atr.isImmunized() || atr.hasPosTagStartingWith("NP") ||
    /// atr.hasPosTagStartingWith("_english_ignore_")` (barbarisms).
    ProperNounImmunizedOrEnglishIgnore,
    /// Catalan `SimpleReplaceAnglicism.isTokenException`: proper nouns are
    /// only exceptions when longer than one character, plus immunized and
    /// speller-ignorable tokens.
    CatalanAnglicism,
}

impl TokenException {
    fn matches(self, atr: &AnalyzedTokenReadings) -> bool {
        match self {
            TokenException::None => false,
            TokenException::ProperNounOrImmunized => {
                atr.has_pos_tag_starting_with("NP") || atr.is_immunized
            }
            TokenException::ProperNounImmunizedOrEnglishIgnore => {
                atr.is_immunized
                    || atr.has_pos_tag_starting_with("NP")
                    || atr.has_pos_tag_starting_with("_english_ignore_")
            }
            TokenException::CatalanAnglicism => {
                (atr.has_pos_tag_starting_with("NP") && atr.surface().chars().count() > 1)
                    || atr.is_immunized
                    || atr.is_ignore_spelling
            }
        }
    }
}

#[derive(Clone)]
struct SuggestionWithMessage {
    suggestion: String,
    message: Option<String>,
}

pub struct SimpleReplaceRule {
    config: SimpleReplaceConfig,
    /// first char/word -> maximum token count of matching entries
    start_space: HashMap<String, usize>,
    start_no_space: HashMap<String, usize>,
    full_space: HashMap<String, SuggestionWithMessage>,
    full_no_space: HashMap<String, SuggestionWithMessage>,
}

/// Classic constructor for the default-on English instances. Order follows
/// `English.getRelevantRules`: diacritics (26), plain English (27), redundancy
/// (28), simple replace (29), profanity (30); the variant-specific
/// AmericanReplaceRule/BritishReplaceRule is appended by the variant language
/// class after the common list.
#[allow(clippy::vec_init_then_push)]
pub fn english_instances(data_dir: &Path, variant: Option<&str>) -> Result<Vec<SimpleReplaceRule>> {
    let rules_dir = data_dir.join("en/rules");
    let mut instances = Vec::new();
    instances.push(SimpleReplaceRule::from_files(
        &[rules_dir.join("diacritics.txt")],
        SimpleReplaceConfig {
            rule_id: "EN_DIACRITICS_REPLACE_ORTHOGRAPHY",
            description: "Suggest diacritics for '$match'",
            short: "The original word has a diacritic",
            message: "'$match' is an imported foreign name or expression, which originally has a diacritic.",
            suggestions_separator: " or ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "TYPOS",
            category_name: "Possible Typo",
            issue_type: "misspelling",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    instances.push(SimpleReplaceRule::from_files(
        &[rules_dir.join("wordiness.txt")],
        SimpleReplaceConfig {
            rule_id: "EN_PLAIN_ENGLISH_REPLACE",
            description: "1. Wordiness (General)",
            short: "Wordiness",
            message: "'$match' is a wordy or complex expression. In some cases, it might be preferable to use $suggestions.",
            suggestions_separator: " or ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "PLAIN_ENGLISH",
            category_name: "Plain English",
            issue_type: "style",
            default_off: true,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    instances.push(SimpleReplaceRule::from_files(
        &[rules_dir.join("redundancies.txt")],
        SimpleReplaceConfig {
            rule_id: "EN_REDUNDANCY_REPLACE",
            description: "1. Redundancy (General)",
            short: "Redundancy",
            message: "'$match' is a redundancy. In some cases, it might be preferable to use $suggestions",
            suggestions_separator: " or ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "REDUNDANCY",
            category_name: "Redundant Phrases",
            issue_type: "style",
            default_off: true,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    instances.push(SimpleReplaceRule::from_files(
        &[
            rules_dir.join("replace.txt"),
            rules_dir.join("replace_custom.txt"),
        ],
        SimpleReplaceConfig {
            rule_id: "EN_SIMPLE_REPLACE",
            description: "Check for wrong words/phrases: $match",
            short: "Wrong word",
            message: "Did you mean $suggestions?",
            suggestions_separator: ", ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "MISC",
            category_name: "Miscellaneous",
            issue_type: "misspelling",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    instances.push(SimpleReplaceRule::from_files(
        &[rules_dir.join("replace_profanity.txt")],
        SimpleReplaceConfig {
            rule_id: "PROFANITY",
            description: "Profanity",
            short: "Profanity",
            message: "This expression can be considered offensive.",
            suggestions_separator: ", ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "STYLE",
            category_name: "Style",
            issue_type: "style",
            default_off: false,
            picky: true,
            has_suggestions: false,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // variant language class appends its replace rule after the common list
    match variant {
        Some("en-GB") => instances.push(SimpleReplaceRule::from_files(
            &[rules_dir.join("en-GB/replace.txt")],
            SimpleReplaceConfig {
                rule_id: "EN_GB_SIMPLE_REPLACE",
                description: "American words easily confused in British English: $match",
                short: "American word",
                message: "'$match' is a common American expression. Consider using expressions more common to British English.",
                suggestions_separator: ", ",
                sub_rule_specific_ids: true,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "STYLE",
                category_name: "Style",
                issue_type: "locale-violation",
                default_off: false,
                picky: false,
                has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
            },
        )?),
        _ => instances.push(SimpleReplaceRule::from_files(
            &[rules_dir.join("en-US/replace.txt")],
            SimpleReplaceConfig {
                rule_id: "EN_US_SIMPLE_REPLACE",
                description: "British words easily confused in American English: $match",
                short: "British word",
                message: "'$match' is a common British expression. Consider using expressions more common to American English.",
                suggestions_separator: ", ",
                sub_rule_specific_ids: true,
                case_sensitivity: CaseSensitivity::Ci,
                category_id: "STYLE",
                category_name: "Style",
                issue_type: "locale-violation",
                default_off: false,
                picky: false,
                has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
            },
        )?),
    }
    Ok(instances)
}

/// `es.SpanishWikipediaRule` (`ES_WIKIPEDIA_COMMON_ERRORS`, default off):
/// `es/rules/wikipedia.txt` via the `AbstractSimpleReplaceRule2` machinery.
pub fn spanish_wikipedia_instance(data_dir: &Path) -> Result<SimpleReplaceRule> {
    SimpleReplaceRule::from_files(
        &[data_dir.join("es/rules/wikipedia.txt")],
        SimpleReplaceConfig {
            rule_id: "ES_WIKIPEDIA_COMMON_ERRORS",
            description: "Errores frecuentes en los artículos de la Wikipedia",
            short: "Error gramatical u ortográfico",
            message: "'$match' es una expresión errónea. Pruebe a utilizar $suggestions",
            suggestions_separator: " o ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "WIKIPEDIA",
            category_name: "Reglas específicas para Wikipedia",
            issue_type: "grammar",
            default_off: true,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )
}

/// `de.SimpleReplaceRule` (`DE_SIMPLE_REPLACE`): `de/rules/replace.txt` +
/// `de/rules/replace_custom.txt`.
pub fn german_instance(data_dir: &Path) -> Result<SimpleReplaceRule> {
    let rules_dir = data_dir.join("de/rules");
    SimpleReplaceRule::from_files(
        &[
            rules_dir.join("replace.txt"),
            rules_dir.join("replace_custom.txt"),
        ],
        SimpleReplaceConfig {
            rule_id: "DE_SIMPLE_REPLACE",
            description: "Prüft auf bestimmte falsche Wörter/Phrasen: $match",
            short: "Falsches Wort",
            message: "Meinten Sie vielleicht $suggestions?",
            suggestions_separator: ", ",
            sub_rule_specific_ids: true,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "MISC",
            category_name: "Sonstiges",
            issue_type: "uncategorized",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )
}

impl SimpleReplaceRule {
    pub fn rule_id(&self) -> &str {
        self.config.rule_id
    }

    pub fn default_off(&self) -> bool {
        self.config.default_off
    }

    pub fn picky(&self) -> bool {
        self.config.picky
    }

    pub fn category_id(&self) -> &str {
        self.config.category_id
    }

    pub fn from_files(files: &[PathBuf], config: SimpleReplaceConfig) -> Result<Self> {
        let mut rule = Self {
            config,
            start_space: HashMap::new(),
            start_no_space: HashMap::new(),
            full_space: HashMap::new(),
            full_no_space: HashMap::new(),
        };
        for file in files {
            rule.load_file(file);
        }
        Ok(rule)
    }

    /// `fillMaps`.
    fn load_file(&mut self, path: &Path) {
        let Ok(text) = lt_data::fs::read_to_string(path) else {
            return;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            // `isCheckingCase`: `FORM[=message]` becomes
            // `form=FORM[\tmessage]`
            let normalized: String;
            let line = if self.config.checking_case {
                let equal_parts: Vec<&str> = line.split('=').collect();
                normalized = match equal_parts.len() {
                    2 => format!(
                        "{}={}\t{}",
                        equal_parts[0].to_lowercase().trim(),
                        equal_parts[0].trim(),
                        equal_parts[1].trim()
                    ),
                    _ => format!(
                        "{}={}",
                        equal_parts[0].to_lowercase().trim(),
                        equal_parts[0].trim()
                    ),
                };
                normalized.as_str()
            } else {
                line
            };
            let parts: Vec<&str> = line.split('\t').collect();
            let conf_pair = parts[0];
            let message = parts.get(1).map(|s| s.to_string());
            let suggestion = if self.config.has_suggestions {
                match conf_pair.split_once('=') {
                    Some((_, right)) => right.to_string(),
                    None => continue,
                }
            } else {
                String::new()
            };
            let wrong_forms: Vec<&str> = conf_pair
                .split('=')
                .next()
                .unwrap_or("")
                .split('|')
                .collect();
            for wrong_form in wrong_forms {
                let search_key = if self.config.case_sensitivity == CaseSensitivity::Ci {
                    wrong_form.to_lowercase()
                } else {
                    wrong_form.to_string()
                };
                // Java `fillMaps`: equal sides are an error *except* for
                // `isCheckingCase` rules, where `form=form` marks a correct
                // form (the entry only feeds the covered-up-to logic).
                if self.config.has_suggestions
                    && !self.config.checking_case
                    && search_key == suggestion
                {
                    continue;
                }
                let value = SuggestionWithMessage {
                    suggestion: suggestion.clone(),
                    message: message.clone(),
                };
                let contains_space = wrong_form.find(' ').is_some_and(|i| i > 0);
                if !contains_space {
                    let first_char = search_key
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .unwrap_or_default();
                    let entry = self.start_no_space.entry(first_char).or_insert(0);
                    *entry = (*entry).max(search_key.chars().count());
                    self.full_no_space.insert(search_key, value);
                } else {
                    let tokens: Vec<&str> = search_key.split(' ').collect();
                    let first_token = tokens[0].to_string();
                    let entry = self.start_space.entry(first_token).or_insert(0);
                    *entry = (*entry).max(tokens.len());
                    self.full_space.insert(search_key, value);
                }
            }
        }
    }

    /// `AbstractSimpleReplaceRule2.match` over one sentence.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches: Vec<Match> = Vec::new();
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut sent_start = 1usize;
        while sent_start < non_blank.len() && is_punctuation_start(non_blank[sent_start].surface())
        {
            sent_start += 1;
        }
        // `checkCaseCoveredUpto` (`AbstractCheckCaseRule`)
        let mut check_case_covered_upto = 0usize;
        for start_index in sent_start..non_blank.len() {
            if self
                .config
                .is_token_exception
                .matches(non_blank[start_index])
            {
                continue;
            }
            let tok = non_blank[start_index].surface();
            if tok.is_empty() {
                continue;
            }
            let mut tok = tok.to_string();
            let mut k = start_index + 1;
            while k < non_blank.len() && !non_blank[k].whitespace_before {
                tok.push_str(non_blank[k].surface());
                k += 1;
            }
            if self.config.case_sensitivity == CaseSensitivity::Ci {
                tok = tok.to_lowercase();
            }
            if let Some(&max_token_len) = self.start_space.get(&tok) {
                let mut key = String::new();
                let mut end_index = start_index;
                while end_index < non_blank.len()
                    && end_index - start_index < MAX_TOKENS_IN_MULTIWORD
                {
                    if end_index > start_index && non_blank[end_index].whitespace_before {
                        key.push(' ');
                    }
                    key.push_str(non_blank[end_index].surface());
                    let original_str = key.clone();
                    let number_of_spaces = original_str.matches(' ').count();
                    if number_of_spaces + 1 > max_token_len {
                        break;
                    }
                    if number_of_spaces > 0 {
                        let key_str = if self.config.case_sensitivity == CaseSensitivity::Ci {
                            original_str.to_lowercase()
                        } else {
                            original_str.clone()
                        };
                        self.create_match(
                            &mut rule_matches,
                            self.full_space.get(&key_str),
                            start_index,
                            end_index,
                            &original_str,
                            &non_blank,
                            sent_start,
                            sentence_offset,
                            &mut check_case_covered_upto,
                        );
                    }
                    end_index += 1;
                }
            }
            let first_char = tok
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_default();
            if self.start_no_space.contains_key(&first_char) {
                let mut end_index = start_index;
                let mut key = String::new();
                while end_index < non_blank.len()
                    && end_index - start_index < MAX_TOKENS_IN_MULTIWORD
                {
                    if end_index > start_index && non_blank[end_index].whitespace_before {
                        break;
                    }
                    key.push_str(non_blank[end_index].surface());
                    let key_str = if self.config.case_sensitivity == CaseSensitivity::Ci {
                        key.to_lowercase()
                    } else {
                        key.clone()
                    };
                    self.create_match(
                        &mut rule_matches,
                        self.full_no_space.get(&key_str),
                        start_index,
                        end_index,
                        &key.clone(),
                        &non_blank,
                        sent_start,
                        sentence_offset,
                        &mut check_case_covered_upto,
                    );
                    end_index += 1;
                }
            }
        }
        rule_matches
    }

    #[allow(clippy::too_many_arguments)]
    fn create_match(
        &self,
        rule_matches: &mut Vec<Match>,
        suggestion_with_message: Option<&SuggestionWithMessage>,
        start_index: usize,
        end_index: usize,
        original_str: &str,
        tokens: &[&AnalyzedTokenReadings],
        sent_start: usize,
        sentence_offset: usize,
        check_case_covered_upto: &mut usize,
    ) {
        let Some(swm) = suggestion_with_message else {
            return;
        };
        let replacements: Vec<String> = if self.config.has_suggestions {
            swm.suggestion.split('|').map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        };
        let from_pos = sentence_offset + tokens[start_index].start_pos;
        let to_pos = sentence_offset + tokens[end_index].end_pos();
        // keep only the longest match
        if let Some(last) = rule_matches.last() {
            if last.range.start <= from_pos && last.range.end >= to_pos {
                return;
            }
        }
        let first_word_in_sugg_is_camel = replacements
            .iter()
            .any(|r| r.split(' ').next().is_some_and(is_camel_case));
        let is_all_upper = lt_spell::morfologik::is_all_uppercase(original_str);
        let is_capitalized =
            lt_spell::morfologik::is_capitalized_word(original_str.split(' ').next().unwrap_or(""));
        // `AbstractCheckCaseRule` exceptions
        if self.config.checking_case {
            if end_index <= *check_case_covered_upto {
                return;
            }
            if replacements.is_empty() {
                return;
            }
            let replacement_check_case = replacements[0].clone();
            let upper_replacement = uppercase_first(&replacement_check_case);
            if (sent_start == start_index && original_str == upper_replacement)
                || original_str == replacement_check_case
            {
                if let Some(last) = rule_matches.last() {
                    if last.range.end > from_pos {
                        rule_matches.pop();
                    }
                }
                *check_case_covered_upto = end_index;
                return;
            }
            // Allow all-upper case, except CamelCase and short words
            if !first_word_in_sugg_is_camel
                && original_str == original_str.to_uppercase()
                && (self.config.ignore_short_uppercase_words || original_str.chars().count() > 4)
            {
                *check_case_covered_upto = end_index;
                return;
            }
        }
        let mut final_replacements: Vec<String> = Vec::new();
        for repl in &replacements {
            let mut final_repl = repl.clone();
            if !first_word_in_sugg_is_camel
                && (sent_start == start_index || (is_capitalized && !self.config.checking_case))
            {
                final_repl = uppercase_first(repl);
            }
            if !self.config.checking_case && is_all_upper {
                final_repl = repl.to_uppercase();
            }
            if final_repl == original_str {
                final_replacements.clear();
                break;
            }
            if repl != original_str && !final_replacements.contains(&final_repl) {
                final_replacements.push(final_repl);
            }
        }
        if self.config.has_suggestions && final_replacements.is_empty() {
            return;
        }
        let msg = match &swm.message {
            Some(msg) => msg.clone(),
            None => {
                let mut msg_suggestions = String::new();
                for (k, repl) in replacements.iter().enumerate() {
                    if k > 0 {
                        msg_suggestions.push_str(if k == replacements.len() - 1 {
                            self.config.suggestions_separator
                        } else {
                            ", "
                        });
                    }
                    msg_suggestions.push_str("<suggestion>");
                    msg_suggestions.push_str(repl);
                    msg_suggestions.push_str("</suggestion>");
                }
                self.config
                    .message
                    .replacen("$match", original_str, 1)
                    .replacen("$suggestions", &msg_suggestions, 1)
            }
        };
        let rule_id = if self.config.sub_rule_specific_ids {
            let id = format!("{}_{}", self.config.rule_id, original_str);
            to_id(&id)
        } else {
            self.config.rule_id.to_string()
        };
        let description = self.config.description.replace("$match", original_str);
        let suggestions: Vec<Suggestion> = if self.config.has_suggestions {
            final_replacements
                .into_iter()
                .map(|value| Suggestion {
                    value,
                    short_description: None,
                })
                .collect()
        } else {
            Vec::new()
        };
        let m = Match::new(
            &rule_id,
            Option::<String>::None,
            msg,
            Some(self.config.short.to_string()),
            TextRange::new(from_pos, to_pos),
            suggestions,
            self.config.category_id,
            self.config.category_name,
        )
        .with_metadata(&description, self.config.issue_type, 0)
        .with_match_type("Other")
        .with_picky(self.config.picky);
        // remove the previous match if it is contained in this one
        if let Some(last) = rule_matches.last() {
            if last.range.start >= from_pos && last.range.end <= to_pos {
                rule_matches.pop();
            }
        }
        rule_matches.push(m);
    }
}

const MAX_TOKENS_IN_MULTIWORD: usize = 20;

fn is_punctuation_start(word: &str) -> bool {
    word.chars().any(|c| c.is_ascii_digit())
        || word.chars().all(|c| !c.is_alphabetic())
        || is_punctuation_mark(word)
}

/// `StringTools.toId` (English locale; also used by the Portuguese legacy
/// `AbstractSimpleReplaceRule` sub-rule ids).
pub(crate) fn to_id(input: &str) -> String {
    input
        .trim()
        .to_uppercase()
        .replace(' ', "_")
        .replace('\'', "_Q_")
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

/// `StringTools.isCamelCase`: `[a-z]+[A-Z][A-Za-z]+` (full match).
fn is_camel_case(s: &str) -> bool {
    static CAMEL: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^[a-z]+[A-Z][A-Za-z]+$").unwrap());
    CAMEL.is_match(s)
}

/// `StringTools.uppercaseFirstChar`: skips leading non-letter-or-digit
/// characters, so a suggestion starting with a quote becomes `'S …`.
fn uppercase_first(s: &str) -> String {
    lt_spell::morfologik::uppercase_first_char(s)
}
