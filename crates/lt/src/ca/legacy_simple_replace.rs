//! Catalan `AbstractSimpleReplaceRule` (legacy, single-token) instances:
//! `SimpleReplaceVerbsRule` (13), `SimpleReplaceBalearicRule` (14),
//! `SimpleReplaceRule` (15), `ReplaceOperationNamesRule` (17),
//! `SimpleReplaceDiacriticsIEC` (18) and `SimpleReplaceAdverbsMent` (22).
//!
//! Java rewrites the `$match` placeholder of the rule description with the
//! matched token (the lemma for `SimpleReplaceVerbsRule`, the singularised
//! map key for `ReplaceOperationNamesRule`); the same `originalTokenStr` is
//! re-checked at runtime by the language-specific post filters
//! (`ConvertToGenderAndNumberFilter`, `AdjustVerbSuggestionsFilter`) before
//! the match is kept.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock};

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, Match, TextRange};
use lt_pattern::{FilterContext, RuleFilter};

use crate::ca::filters::Env;
use crate::ca::helpers::reading_with_tag_regex;
use crate::simple_replace::to_id;

pub const VERBS_ID: &str = "CA_SIMPLE_REPLACE_VERBS";
pub const BALEARIC_ID: &str = "CA_SIMPLE_REPLACE_BALEARIC";
pub const SIMPLE_ID: &str = "CA_SIMPLE_REPLACE_SIMPLE";
pub const OPERATION_NAMES_ID: &str = "NOMS_OPERACIONS";
pub const DIACRITICS_IEC_ID: &str = "CA_SIMPLE_REPLACE_DIACRITICS_IEC";
pub const ADVERBS_MENT_ID: &str = "ADVERBIS_MENT";

/// `SimpleReplaceDataLoader.loadWords`: `wrong1|wrong2=right1|right2` lines,
/// `#` comments, last entry wins.
pub(crate) fn load_words(paths: &[std::path::PathBuf]) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    for path in paths {
        let Ok(text) = lt_data::fs::read_to_string(path) else {
            continue;
        };
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((left, right)) = line.split_once('=') else {
                continue;
            };
            if right.trim().is_empty() {
                continue;
            }
            let replacements: Vec<String> = right.split('|').map(|s| s.to_string()).collect();
            for wrong_form in left.split('|') {
                map.insert(wrong_form.to_string(), replacements.clone());
            }
        }
    }
    map
}

/// The six legacy subclasses of the Catalan rule list.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LegacyKind {
    Verbs,
    Balearic,
    Simple,
    OperationNames,
    DiacriticsIec,
    AdverbsMent,
}

/// `isTokenException` overrides.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenException {
    None,
    /// `SimpleReplaceDiacriticsIEC`: proper nouns only.
    ProperNoun,
    /// `SimpleReplaceBalearicRule`: proper nouns tagged in multiwords plus
    /// speller-ignorable tokens.
    Balearic,
}

/// `createRuleMatch` post-processing (a Java filter run on the raw match).
#[derive(Clone, Copy, PartialEq, Eq)]
enum PostFilter {
    None,
    GenderNumber,
    VerbSuggestions,
}

pub struct LegacyConfig {
    kind: LegacyKind,
    pub rule_id: &'static str,
    description: &'static str,
    short: &'static str,
    category_id: &'static str,
    category_name: &'static str,
    issue_type: &'static str,
    case_sensitive: bool,
    check_lemmas: bool,
    ignore_tagged_words: bool,
    sub_rule_specific_ids: bool,
    default_off: bool,
    picky: bool,
    token_exception: TokenException,
    post_filter: PostFilter,
    files: &'static [&'static str],
}

pub struct LegacyReplaceRule {
    config: LegacyConfig,
    wrong_words: HashMap<String, Vec<String>>,
    #[allow(dead_code)]
    synth: Arc<lt_tagger::CatalanSynthesizer>,
    env: Env,
}

fn starts_with_uppercase(text: &str) -> bool {
    text.chars().next().is_some_and(char::is_uppercase)
}

fn is_capitalized_word(text: &str) -> bool {
    lt_tagger::is_capitalized_word(text)
}

fn matches_pos_regexp(tr: &AnalyzedTokenReadings, re: &regex::Regex) -> bool {
    tr.readings.iter().any(|r| {
        let tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
        re.is_match(tag)
    })
}

macro_rules! anchored_re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<regex::Regex> =
            LazyLock::new(|| regex::Regex::new(concat!("^(?:", $pat, ")$")).unwrap());
    };
}

// `ReplaceOperationNamesRule` patterns.
anchored_re!(PREV_POS_RE, "D[^R].*|PX.*|SPS00|SENT_START");
anchored_re!(
    PREV_EXCEP_RE,
    "RG_anteposat|N.*|CC|_PUNCT.*|_loc_unavegada|RN"
);
anchored_re!(NEXT_EXCEP_RE, "N.*");
anchored_re!(PUNTUACIO_RE, "PUNCT.*|SENT_START");
anchored_re!(DETERMINANT_RE, "D[^R].M.*");

/// `NonInteractiveRule.runFilter`-style direct filter call: run `filter`
/// over a raw match and fold the outcome back into the match.
pub(crate) fn apply_filter(
    filter: &dyn RuleFilter,
    m: Match,
    sentence: &AnalyzedSentence,
    tokens: &[AnalyzedTokenReadings],
    args: HashMap<String, String>,
) -> Option<Match> {
    let refs: Vec<&AnalyzedTokenReadings> = tokens.iter().collect();
    let matched: Vec<&AnalyzedTokenReadings> = refs
        .iter()
        .copied()
        .filter(|t| !t.is_whitespace && t.start_pos < m.range.end && t.end_pos() > m.range.start)
        .collect();
    let ctx = FilterContext {
        rule_id: &m.rule_id,
        args,
        pattern_tokens: &matched,
        sentence_tokens: &refs,
        token_positions: &[],
        pattern_token_pos: 0,
        match_range: m.range,
        sentence_text: &sentence.text,
        message: m.message.clone(),
        short_message: m.short_message.clone(),
        suggestions: m.suggestions.clone(),
    };
    let out = filter.accept(&ctx);
    if !out.accepted {
        return None;
    }
    let mut m = m;
    if let Some(range) = out.range {
        m.range = range;
    }
    if let Some(message) = out.message {
        m.message = message;
    }
    if let Some(suggestions) = out.suggestions {
        m.suggestions = suggestions;
    }
    Some(m)
}

impl LegacyReplaceRule {
    fn load(config: LegacyConfig, data_dir: &Path, env: Env) -> Self {
        let files: Vec<std::path::PathBuf> =
            config.files.iter().map(|f| data_dir.join(f)).collect();
        let synth = Arc::clone(&env.synth.synth);
        Self {
            wrong_words: load_words(&files),
            config,
            synth,
            env,
        }
    }

    pub fn rule_id(&self) -> &'static str {
        self.config.rule_id
    }

    pub fn category_id(&self) -> &'static str {
        self.config.category_id
    }

    pub fn default_off(&self) -> bool {
        self.config.default_off
    }

    pub fn picky(&self) -> bool {
        self.config.picky
    }

    fn cleanup(&self, word: &str) -> String {
        if self.config.case_sensitive {
            word.to_string()
        } else {
            word.to_lowercase()
        }
    }

    fn token_exception(&self, tr: &AnalyzedTokenReadings) -> bool {
        match self.config.token_exception {
            TokenException::None => false,
            TokenException::ProperNoun => tr.has_pos_tag_starting_with("NP"),
            TokenException::Balearic => {
                tr.has_pos_tag_starting_with("NP")
                    || tr.is_immunized
                    || tr.is_ignore_spelling
                    || tr.has_pos_tag("_english_ignore_")
                    || tr.has_pos_tag("_Latin_")
            }
        }
    }

    fn message(&self, replacements: &[String]) -> String {
        match self.config.kind {
            LegacyKind::Verbs => "Verb incorrecte.".to_string(),
            LegacyKind::Balearic => {
                "Possible error ortogràfic (forma verbal vàlida en la varietat balear).".to_string()
            }
            LegacyKind::Simple => {
                if !replacements.is_empty() {
                    format!("¿Volíeu dir «{}»?", replacements[0])
                } else {
                    self.config.short.to_string()
                }
            }
            LegacyKind::OperationNames => {
                "Si és el nom d'una operació tècnica, val més usar una altra forma.".to_string()
            }
            LegacyKind::DiacriticsIec => {
                "Hi sobra l'accent diacrític (segons les normes noves).".to_string()
            }
            LegacyKind::AdverbsMent => {
                "A vegades s'abusa dels adverbis acabats en -ment en detriment de formes més àgils."
                    .to_string()
            }
        }
    }

    /// `AbstractSimpleReplaceRule.createRuleMatch`.
    fn create_rule_match(
        &self,
        tr: &AnalyzedTokenReadings,
        replacements: Vec<String>,
        original_token_str: &str,
        sentence_offset: usize,
    ) -> Match {
        let token_string = tr.surface();
        let pos = tr.start_pos;
        // Java `createRuleMatch` builds the message before it uppercases the
        // replacements (so `¿Volíeu dir «això»?` keeps the lowercase form).
        let message = self.message(&replacements);
        let mut replacements = replacements;
        if !self.config.case_sensitive && starts_with_uppercase(token_string) {
            for replacement in &mut replacements {
                *replacement = lt_tagger::uppercase_first_char(replacement);
            }
        }
        let description = self
            .config
            .description
            .replace("$match", original_token_str);
        let rule_id = if self.config.sub_rule_specific_ids {
            to_id(&format!("{}_{}", self.config.rule_id, original_token_str))
        } else {
            self.config.rule_id.to_string()
        };
        Match::new(
            &rule_id,
            Option::<String>::None,
            message,
            Some(self.config.short.to_string()),
            TextRange::new(
                sentence_offset + pos,
                sentence_offset + pos + token_string.len(),
            ),
            replacements
                .into_iter()
                .map(|value| lt_core::Suggestion {
                    value,
                    short_description: None,
                })
                .collect(),
            self.config.category_id,
            self.config.category_name,
        )
        .with_metadata(&description, self.config.issue_type, 0)
        .with_match_type("Other")
    }

    fn apply_post_filter(
        &self,
        m: Match,
        sentence: &AnalyzedSentence,
        tokens: &[AnalyzedTokenReadings],
    ) -> Option<Match> {
        match self.config.post_filter {
            PostFilter::None => Some(m),
            PostFilter::GenderNumber => {
                let filter = crate::ca::gender_number::ConvertToGenderAndNumberFilter {
                    env: Arc::clone(&self.env),
                };
                let mut args = HashMap::new();
                args.insert("lemmaSelect".to_string(), "[NA].*".to_string());
                apply_filter(&filter, m, sentence, tokens, args)
            }
            PostFilter::VerbSuggestions => {
                let filter = crate::ca::verb_filters::AdjustVerbSuggestionsFilter {
                    env: Arc::clone(&self.env),
                };
                let mut args = HashMap::new();
                args.insert("actions".to_string(), "None".to_string());
                apply_filter(&filter, m, sentence, tokens, args)
            }
        }
    }

    /// `AbstractSimpleReplaceRule.match` over one sentence (all subclasses
    /// except the custom `match` overrides).
    fn base_matches(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence: &AnalyzedSentence,
        offset: usize,
    ) -> Vec<Match> {
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let exceptions: [&str; 3] = ["perquè", "què", "per què"];
        let mut out = Vec::new();
        for tr in non_blank {
            if tr.is_sentence_start
                || tr.is_immunized
                || tr.is_ignore_spelling
                || self.token_exception(tr)
                || (self.config.ignore_tagged_words && tr.is_tagged)
            {
                continue;
            }
            let original = tr.surface().to_string();
            let token_string = self.cleanup(&original);
            let mut possible = self
                .wrong_words
                .get(&original)
                .or_else(|| self.wrong_words.get(&token_string))
                .cloned();
            if possible.is_none() && self.config.check_lemmas {
                let mut lemmas: Vec<String> = Vec::new();
                for reading in &tr.readings {
                    if let Some(lemma) = &reading.stem {
                        if self.wrong_words.contains_key(lemma) && !lemmas.contains(lemma) {
                            lemmas.push(lemma.clone());
                        }
                    }
                }
                let mut synthesized: Vec<String> = Vec::new();
                for lemma in lemmas {
                    let Some(replacement_lemmas) = self.wrong_words.get(&lemma) else {
                        continue;
                    };
                    for replacement_lemma in replacement_lemmas {
                        for at in &tr.readings {
                            let token = AnalyzedToken::new(
                                at.stem.clone().unwrap_or_default(),
                                Some(replacement_lemma.clone()),
                                at.pos_tag.clone(),
                            );
                            let Some(tag) = at.pos_tag.as_deref() else {
                                continue;
                            };
                            for form in self.synth.synthesize(&token, tag, false) {
                                if !synthesized.contains(&form) {
                                    synthesized.push(form);
                                }
                            }
                        }
                    }
                }
                if !synthesized.is_empty() {
                    possible = Some(synthesized);
                }
            }
            let Some(possible) = possible else {
                continue;
            };
            if possible.is_empty() {
                continue;
            }
            let is_all_uppercase = lt_tagger::is_all_uppercase(&original);
            let mut replacements: Vec<String> = if is_all_uppercase {
                possible.iter().map(|s| s.to_uppercase()).collect()
            } else {
                possible
            };
            replacements.retain(|r| *r != original);
            if replacements.is_empty() {
                continue;
            }
            let m = self.create_rule_match(tr, replacements, &original, offset);
            if self.config.kind == LegacyKind::Simple
                && m.suggestions
                    .iter()
                    .any(|s| exceptions.contains(&s.value.to_lowercase().as_str()))
            {
                out.push(m);
                continue;
            }
            if let Some(m) = self.apply_post_filter(m, sentence, tokens) {
                out.push(m);
            }
        }
        out
    }

    /// `SimpleReplaceVerbsRule.match`: the `_incorrect_verb_` chunk tag plus
    /// `AdjustVerbSuggestionsFilter(actions:None)`.
    fn verb_matches(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence: &AnalyzedSentence,
        offset: usize,
    ) -> Vec<Match> {
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut out = Vec::new();
        for tr in non_blank.iter().skip(1) {
            if !tr.chunk_tags.iter().any(|c| c == "_incorrect_verb_")
                || tr.has_pos_tag_starting_with("N")
                || tr.has_pos_tag_starting_with("A")
            {
                continue;
            }
            let Some(at) = reading_with_tag_regex(tr, "V.*") else {
                continue;
            };
            let lemma = at.lemma().to_string();
            let Some(replacement_infinitives) = self.wrong_words.get(&lemma) else {
                continue;
            };
            let m = self.create_rule_match(tr, replacement_infinitives.clone(), &lemma, offset);
            if let Some(m) = self.apply_post_filter(m, sentence, tokens) {
                out.push(m);
            }
        }
        out
    }

    /// `ReplaceOperationNamesRule.match`.
    fn operation_name_matches(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence: &AnalyzedSentence,
        offset: usize,
    ) -> Vec<Match> {
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut out = Vec::new();
        for (i, tr) in non_blank.iter().enumerate().skip(1) {
            let mut token = tr.surface().to_lowercase();
            if token.len() > 3 && token.ends_with('s') {
                token.truncate(token.len() - 1);
            }
            let Some(replacement_lemmas) = self.wrong_words.get(&token) else {
                continue;
            };
            // exceptions
            if token == "duplicat" && non_blank[i - 1].surface().eq_ignore_ascii_case("per") {
                continue;
            }
            if i + 1 < non_blank.len()
                && token.eq_ignore_ascii_case("polit")
                && is_capitalized_word(non_blank[i + 1].surface())
            {
                continue;
            }
            if i + 1 < non_blank.len()
                && matches_pos_regexp(non_blank[i - 1], &PUNTUACIO_RE)
                && matches_pos_regexp(non_blank[i + 1], &DETERMINANT_RE)
            {
                continue;
            }
            if tr.has_pos_tag("_GV_") {
                continue;
            }
            if i + 1 < non_blank.len()
                && (non_blank[i + 1].has_lemma("per")
                    || non_blank[i + 1].has_lemma("com")
                    || non_blank[i + 1].has_lemma("des")
                    || non_blank[i + 1].has_lemma("amb")
                    || matches_pos_regexp(non_blank[i + 1], &NEXT_EXCEP_RE))
            {
                continue;
            }
            if !matches_pos_regexp(non_blank[i - 1], &PREV_POS_RE)
                || matches_pos_regexp(non_blank[i - 1], &PREV_EXCEP_RE)
            {
                continue;
            }
            let mut possible_replacements: Vec<String> = Vec::new();
            if !tr.surface().to_lowercase().ends_with('s') {
                possible_replacements.extend(replacement_lemmas.iter().cloned());
            } else {
                // synthesize plural: Java `new AnalyzedToken(replacementLemma,
                // "NCMS000", replacementLemma)` = (token, posTag, lemma)
                for replacement_lemma in replacement_lemmas {
                    let probe = AnalyzedToken::new(
                        replacement_lemma.clone(),
                        Some(replacement_lemma.clone()),
                        Some("NCMS000".to_string()),
                    );
                    possible_replacements.extend(self.synth.synthesize(&probe, "NC.P.*", true));
                }
            }
            if possible_replacements.is_empty() {
                continue;
            }
            let m = self.create_rule_match(tr, possible_replacements, &token, offset);
            if let Some(m) = self.apply_post_filter(m, sentence, tokens) {
                out.push(m);
            }
        }
        out
    }

    /// One sentence of the legacy subclass (`Rule.match`).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence: &AnalyzedSentence,
        sentence_offset: usize,
    ) -> Vec<Match> {
        match self.config.kind {
            LegacyKind::Verbs => self.verb_matches(tokens, sentence, sentence_offset),
            LegacyKind::OperationNames => {
                self.operation_name_matches(tokens, sentence, sentence_offset)
            }
            _ => self.base_matches(tokens, sentence, sentence_offset),
        }
    }
}

/// The six legacy rules in `Catalan.getRelevantRules` order.
pub fn catalan_legacy_instances(data_dir: &Path, env: Env) -> Vec<LegacyReplaceRule> {
    let make = |config: LegacyConfig| LegacyReplaceRule::load(config, data_dir, Arc::clone(&env));
    vec![
        // `SimpleReplaceVerbsRule` (13)
        make(LegacyConfig {
            kind: LegacyKind::Verbs,
            rule_id: VERBS_ID,
            description: "Verb incorrecte: $match",
            short: "Verb incorrecte",
            category_id: "TYPOS",
            category_name: "Errors ortogràfics",
            issue_type: "grammar",
            case_sensitive: true,
            check_lemmas: true,
            ignore_tagged_words: true,
            sub_rule_specific_ids: true,
            default_off: false,
            picky: false,
            token_exception: TokenException::None,
            post_filter: PostFilter::VerbSuggestions,
            files: &["ca/rules/replace_verbs.txt"],
        }),
        // `SimpleReplaceBalearicRule` (14)
        make(LegacyConfig {
            kind: LegacyKind::Balearic,
            rule_id: BALEARIC_ID,
            description: "Suggeriments per a formes balears: $match",
            short: "Possible error ortogràfic.",
            category_id: "TYPOS",
            category_name: "Errors ortogràfics",
            issue_type: "grammar",
            case_sensitive: false,
            check_lemmas: false,
            ignore_tagged_words: false,
            sub_rule_specific_ids: true,
            default_off: false,
            picky: false,
            token_exception: TokenException::Balearic,
            post_filter: PostFilter::None,
            files: &["ca/rules/replace_balearic.txt"],
        }),
        // `SimpleReplaceRule` (15)
        make(LegacyConfig {
            kind: LegacyKind::Simple,
            rule_id: SIMPLE_ID,
            description: "Paraula incorrecta: $match",
            short: "Paraula incorrecta",
            category_id: "TYPOS",
            category_name: "Errors ortogràfics",
            issue_type: "grammar",
            case_sensitive: false,
            check_lemmas: false,
            ignore_tagged_words: true,
            sub_rule_specific_ids: true,
            default_off: false,
            picky: false,
            token_exception: TokenException::None,
            post_filter: PostFilter::GenderNumber,
            files: &["ca/rules/replace.txt", "ca/rules/replace_custom.txt"],
        }),
        // `ReplaceOperationNamesRule` (17)
        make(LegacyConfig {
            kind: LegacyKind::OperationNames,
            rule_id: OPERATION_NAMES_ID,
            description: "S'ha d'evitar com a nom d'operació tècnica: $match",
            short: "Forma preferible",
            category_id: "FORMES_SECUNDARIES",
            category_name: "C8) Formes secundàries",
            issue_type: "style",
            case_sensitive: false,
            check_lemmas: true,
            ignore_tagged_words: false,
            sub_rule_specific_ids: true,
            default_off: false,
            picky: false,
            token_exception: TokenException::None,
            post_filter: PostFilter::GenderNumber,
            files: &["ca/rules/replace_operationnames.txt"],
        }),
        // `SimpleReplaceDiacriticsIEC` (18)
        make(LegacyConfig {
            kind: LegacyKind::DiacriticsIec,
            rule_id: DIACRITICS_IEC_ID,
            description: "Accents diacrítics segons les normes noves (2017): $match",
            short: "Hi sobra l'accent.",
            category_id: "DIACRITICS_IEC",
            category_name: "Z) Accents diacrítics segons l'IEC",
            issue_type: "grammar",
            case_sensitive: false,
            check_lemmas: false,
            ignore_tagged_words: false,
            sub_rule_specific_ids: true,
            default_off: false,
            picky: false,
            token_exception: TokenException::ProperNoun,
            post_filter: PostFilter::None,
            files: &["ca/rules/replace_diacritics_iec.txt"],
        }),
        // `SimpleReplaceAdverbsMent` (22), default off + picky
        make(LegacyConfig {
            kind: LegacyKind::AdverbsMent,
            rule_id: ADVERBS_MENT_ID,
            description: "Alternatives a adverbis acabats en -ment: $match",
            short: "Alternatives a adverbis acabats en -ment",
            category_id: "PICKY_STYLE",
            category_name: "regles d'estil, mode perfeccionaista",
            issue_type: "style",
            case_sensitive: false,
            check_lemmas: false,
            ignore_tagged_words: false,
            sub_rule_specific_ids: true,
            default_off: true,
            picky: true,
            token_exception: TokenException::None,
            post_filter: PostFilter::None,
            files: &["ca/rules/replace_adverbs_ment.txt"],
        }),
    ]
}
