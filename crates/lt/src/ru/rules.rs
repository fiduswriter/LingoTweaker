//! Russian `getRelevantRules` Java rule classes (stage 3).

use std::path::Path;
use std::sync::{Arc, LazyLock};

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};
use regex::Regex;

use crate::compound::CompoundRule;
use crate::dash::DashRule;
use crate::simple_replace::{
    CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule, TokenException,
};
use crate::specific_case::{SpecificCaseConfig, SpecificCaseRule};
use crate::word_coherency::WordCoherencyRule;

/// The Russian `getRelevantRules` Java rule classes, loaded from the vendored
/// data directory.
pub struct RussianRules {
    pub simple_replace: Option<Arc<SimpleReplaceRule>>,
    pub specific_case: Arc<SpecificCaseRule>,
    pub dash: Arc<DashRule>,
    pub compound: Arc<CompoundRule>,
    pub word_coherency: Arc<WordCoherencyRule>,
    pub word_root_repeat: Arc<WordCoherencyRule>,
    pub verb_conjugation: RussianVerbConjugationRule,
    pub word_repeat: RussianWordRepeatRule,
    pub simple_word_repeat: RussianSimpleWordRepeatRule,
}

impl RussianRules {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            simple_replace: match simple_replace_instance(data_dir) {
                Ok(rule) => Some(Arc::new(rule)),
                Err(err) => {
                    eprintln!("[ru] simple replace rule disabled: {err}");
                    None
                }
            },
            specific_case: Arc::new(specific_case_instance(data_dir)),
            dash: Arc::new(dash_instance(data_dir)?),
            compound: Arc::new(compound_instance(data_dir)?),
            word_coherency: Arc::new(word_coherency_instance(data_dir)),
            word_root_repeat: Arc::new(word_root_repeat_instance(data_dir)),
            verb_conjugation: RussianVerbConjugationRule,
            word_repeat: RussianWordRepeatRule,
            simple_word_repeat: RussianSimpleWordRepeatRule,
        })
    }
}

// ---------------------------------------------------------------------------
// Shared-family instances
// ---------------------------------------------------------------------------

/// `RussianSimpleReplaceRule` (`RU_SIMPLE_REPLACE`, `/ru/replace.txt`).
pub fn simple_replace_instance(data_dir: &Path) -> Result<SimpleReplaceRule> {
    SimpleReplaceRule::from_files(
        &[data_dir.join("ru/rules/replace.txt")],
        SimpleReplaceConfig {
            rule_id: "RU_SIMPLE_REPLACE",
            description: "Поиск просторечий и ошибочных фраз",
            short: "Ошибка?",
            message: "«$match» — просторечие, исправление: $suggestions",
            suggestions_separator: ", ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "MISC",
            category_name: "Общие правила",
            issue_type: "misspelling",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )
}

/// `RussianSpecificCaseRule` (`RU_SPECIFIC_CASE`, `/ru/specific_case.txt`).
pub fn specific_case_instance(data_dir: &Path) -> SpecificCaseRule {
    SpecificCaseRule::from_data_with(
        data_dir,
        SpecificCaseConfig {
            rule_id: "RU_SPECIFIC_CASE",
            description: "Написание специальных наименований в верхнем или нижнем регистре",
            short_message: "Специальное написание",
            category_id: "CASING",
            category_name: "Заглавные буквы",
            initial_capital_message:
                "Для специальных наименований используйте начальную заглавную букву.",
            other_capitalization_message:
                "Для специальных наименований используйте предложенное написание заглавных и строчных букв.",
            phrases_path: "ru/words/specific_case.txt",
        },
    )
}

/// `RussianDashRule` (`RU_DASH_RULE`, `/ru/compounds.txt`); the class clears
/// the base `picky` tag, and `isBoundary` is "not a Cyrillic letter".
pub fn dash_instance(data_dir: &Path) -> Result<DashRule> {
    DashRule::from_data(
        data_dir,
        "ru/words/compounds.txt",
        crate::dash::DashConfig {
            rule_id: "RU_DASH_RULE",
            description: "Тире вместо дефиса («из — за» вместо «из-за»).",
            message: "Использовано тире вместо дефиса.",
            category_id: "TYPOGRAPHY",
            category_name: "Типографика",
            picky: false,
            boundary: |c| !('\u{0400}'..='\u{04FF}').contains(&c),
        },
    )
}

/// `RussianCompoundRule` (`RU_COMPOUNDS`, `/ru/compounds.txt`).
pub fn compound_instance(data_dir: &Path) -> Result<CompoundRule> {
    CompoundRule::russian(data_dir)
}

/// `RussianWordCoherencyRule` (`RU_WORD_COHERENCY`, `/ru/coherency.txt`).
pub fn word_coherency_instance(data_dir: &Path) -> WordCoherencyRule {
    WordCoherencyRule::russian(data_dir)
}

/// `RussianWordRootRepeatRule` (`RU_WORD_ROOT_REPEAT`, `/ru/wordrootrep.txt`,
/// default off).
pub fn word_root_repeat_instance(data_dir: &Path) -> WordCoherencyRule {
    WordCoherencyRule::russian_word_root(data_dir)
}

// ---------------------------------------------------------------------------
// RussianVerbConjugationRule
// ---------------------------------------------------------------------------

/// `RussianVerbConjugationRule` (`RU_VERB_CONJUGATION`).
pub struct RussianVerbConjugationRule;

fn pronoun_re() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("PNN:(.*):Nom:(.*)").unwrap());
    &RE
}

fn fut_real_verb_re() -> &'static Regex {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new("VB:(Fut|Real):(.*):(.*):(.*):(.*)").unwrap());
    &RE
}

fn past_verb_re() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("VB:Past:(.*):(.*):(.*)").unwrap());
    &RE
}

fn present_or_future_wrong(pronoun: (&str, &str), verb: (&str, &str)) -> bool {
    if pronoun.1 != verb.1 {
        return true;
    }
    if matches!(pronoun.0, "Masc" | "Fem" | "Neut") {
        return verb.0 == "PL";
    }
    pronoun.0 != verb.0
}

fn past_wrong(pronoun: &str, verb: &str) -> bool {
    if pronoun == "Sin" {
        return verb == "PL" || verb == "Neut";
    }
    pronoun != verb
}

impl RussianVerbConjugationRule {
    pub fn rule_id(&self) -> &'static str {
        "RU_VERB_CONJUGATION"
    }

    pub fn description(&self) -> &'static str {
        "Согласование личных местоимений с глаголами"
    }

    /// `RussianVerbConjugationRule.match` over one sentence.
    #[allow(clippy::needless_range_loop)] // the Java loop indexes i-1..i+2
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let view = tokens_without_whitespace(tokens);
        let mut rule_matches = Vec::new();
        for i in 1..view.len().saturating_sub(1) {
            let previous = view[i - 1];
            let current = view[i];
            let next = view[i + 1];
            let Some(previous_tok) = previous.readings.first() else {
                continue;
            };
            let Some(current_tok) = current.readings.first() else {
                continue;
            };
            let previous_token = previous_tok.token.as_str();
            let current_token = current_tok.token.as_str();
            let current_pos = current_tok.pos_tag.as_deref().unwrap_or("");
            if current_token.is_empty() || current_pos.is_empty() {
                continue;
            }
            let Some(pronoun_caps) = pronoun_re().captures(current_pos) else {
                continue;
            };
            if previous_token == "и" {
                continue;
            }
            let pronoun = (pronoun_caps[1].to_string(), pronoun_caps[2].to_string());
            let Some(next_tok) = next.readings.first() else {
                continue;
            };
            let next2_token = if i < view.len() - 2 {
                view[i + 2]
                    .readings
                    .first()
                    .map(|t| t.token.as_str())
                    .unwrap_or("")
            } else {
                ""
            };
            let next_token = next_tok.token.as_str();
            let next_pos = next_tok.pos_tag.as_deref().unwrap_or("");
            if next_pos.is_empty()
                || (next2_token == "быть" && next_token == "может")
                || next_token == "целую"
            {
                continue;
            }
            if let Some(verb_caps) = fut_real_verb_re().captures(next_pos) {
                let verb = (verb_caps[4].to_string(), verb_caps[5].to_string());
                if present_or_future_wrong((&pronoun.0, &pronoun.1), (&verb.0, &verb.1)) {
                    rule_matches.push(verb_match(current, next, sentence_offset));
                }
            } else if let Some(verb_caps) = past_verb_re().captures(next_pos) {
                if past_wrong(&pronoun_caps[1], &verb_caps[3]) {
                    rule_matches.push(verb_match(current, next, sentence_offset));
                }
            }
        }
        rule_matches
    }
}

fn verb_match(
    current: &AnalyzedTokenReadings,
    next: &AnalyzedTokenReadings,
    sentence_offset: usize,
) -> Match {
    Match::new(
        "RU_VERB_CONJUGATION",
        Option::<String>::None,
        "Неверное спряжение глагола или неверное местоимение",
        Some("Неверное спряжение глагола".to_string()),
        TextRange::new(
            sentence_offset + current.start_pos,
            sentence_offset + next.end_pos(),
        ),
        Vec::<Suggestion>::new(),
        "GRAMMAR",
        "Грамматика",
    )
    .with_metadata("Согласование личных местоимений с глаголами", "grammar", 0)
}

// ---------------------------------------------------------------------------
// RussianWordRepeatRule (AdvancedWordRepeatRule, default off)
// ---------------------------------------------------------------------------

/// `RussianWordRepeatRule` (`RU_WORD_REPEAT`, default off).
pub struct RussianWordRepeatRule;

fn advanced_exc_words() -> &'static std::collections::HashSet<&'static str> {
    static SET: LazyLock<std::collections::HashSet<&'static str>> = LazyLock::new(|| {
        [
            "не",
            "ни",
            "а",
            "их",
            "на",
            "в",
            "по",
            "минута",
            "друг",
            "час",
            "секунда",
            "ПАО",
            "ООО",
            "табл",
            "рис",
        ]
        .into_iter()
        .collect()
    });
    &SET
}

fn advanced_exc_pos() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new("INTERJECTION|PRDC|PREP|CONJ|PARTICLE|ABR|NumC:.*|Num:.*").unwrap()
    });
    &RE
}

fn advanced_exc_nonwords() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            "^(?:&quot|&gt|&lt|&amp|[0-9].*|M*(?:D?C{0,3}|C[DM])(?:L?X{0,3}|X[LC])(?:V?I{0,3}|I[VX])$)$",
        )
        .unwrap()
    });
    &RE
}

impl RussianWordRepeatRule {
    pub fn rule_id(&self) -> &'static str {
        "RU_WORD_REPEAT"
    }

    pub fn description(&self) -> &'static str {
        "Повтор слов в предложении"
    }

    /// `AdvancedWordRepeatRule.match` over one sentence.
    #[allow(clippy::needless_range_loop)] // `cur_token` tracks the token index
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let view = tokens_without_whitespace(tokens);
        let mut rule_matches = Vec::new();
        let mut repetition = false;
        let mut inflected_words: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        let mut cur_token = 0usize;
        for i in 1..view.len() {
            let readings = view[i];
            let token = readings.surface();
            let mut is_word = token.chars().count() >= 2;
            let mut has_lemma = true;
            for analyzed in &readings.readings {
                if let Some(pos_tag) = analyzed.pos_tag.as_deref() {
                    if pos_tag.is_empty() {
                        is_word = false;
                        break;
                    }
                    let Some(lemma) = analyzed.stem.as_deref() else {
                        has_lemma = false;
                        break;
                    };
                    if advanced_exc_words().contains(lemma) {
                        is_word = false;
                        break;
                    }
                    if advanced_exc_pos().is_match(pos_tag) {
                        is_word = false;
                        break;
                    }
                } else {
                    has_lemma = false;
                }
            }
            if is_word && advanced_exc_nonwords().is_match(token) {
                is_word = false;
            }
            let mut prev_lemma = String::new();
            if is_word {
                let mut not_sent_end = false;
                for analyzed in &readings.readings {
                    let pos = analyzed.pos_tag.as_deref();
                    if pos.is_some_and(|p| p == "SENT_END") {
                        not_sent_end = true;
                    }
                    if has_lemma {
                        let cur_lemma = analyzed.stem.clone().unwrap_or_default();
                        if prev_lemma != cur_lemma && !not_sent_end {
                            if inflected_words.contains(&cur_lemma) && cur_token != i {
                                repetition = true;
                            } else {
                                inflected_words.insert(cur_lemma.clone());
                                cur_token = i;
                            }
                        }
                        prev_lemma = cur_lemma;
                    } else if inflected_words.contains(token) && !not_sent_end {
                        repetition = true;
                    } else {
                        inflected_words.insert(token.to_string());
                    }
                }
            }
            if repetition {
                let pos = readings.start_pos;
                rule_matches.push(
                    Match::new(
                        "RU_WORD_REPEAT",
                        Option::<String>::None,
                        "Повтор слов в предложении",
                        Some("Повтор слов в предложении".to_string()),
                        TextRange::new(sentence_offset + pos, sentence_offset + pos + token.len()),
                        Vec::<Suggestion>::new(),
                        "MISC",
                        "Общие правила",
                    )
                    .with_metadata("Повтор слов в предложении", "style", 0),
                );
                repetition = false;
            }
        }
        rule_matches
    }
}

// ---------------------------------------------------------------------------
// RussianSimpleWordRepeatRule (WordRepeatRule)
// ---------------------------------------------------------------------------

/// `RussianSimpleWordRepeatRule` (`WORD_REPEAT_RULE`).
pub struct RussianSimpleWordRepeatRule;

fn single_letter_re() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("[a-zA-Zа-яёА-ЯЁ]").unwrap());
    &RE
}

/// `WordRepeatRule.ignore` base list.
fn base_ignore(view: &[&AnalyzedTokenReadings], position: usize) -> bool {
    [
        "Phi", "Li", "Xiao", "Duran", "Wagga", "Abdullah", "Nwe", "Pago", "Cao",
    ]
    .iter()
    .any(|name| {
        position > 0 && view[position - 1].surface() == *name && view[position].surface() == *name
    })
}

/// `RussianSimpleWordRepeatRule.ignore`.
fn russian_ignore(view: &[&AnalyzedTokenReadings], position: usize) -> bool {
    let repetition_of = |word: &str| {
        position > 0 && view[position - 1].surface() == word && view[position].surface() == word
    };
    if repetition_of("-") || repetition_of("и") || repetition_of("по") || repetition_of("что")
    {
        return true;
    }
    if position > 0
        && ((view[position - 1].surface() == "ПО" && view[position].surface() == "по")
            || (view[position - 1].surface() == "по" && view[position].surface() == "ПО"))
    {
        return true;
    }
    if single_letter_re().is_match(view[position].surface())
        && position > 1
        && single_letter_re().is_match(view[position - 1].surface())
    {
        // spelling with spaces in between: "L L"
        return true;
    }
    base_ignore(view, position)
}

impl RussianSimpleWordRepeatRule {
    pub fn rule_id(&self) -> &'static str {
        "WORD_REPEAT_RULE"
    }

    /// `WordRepeatRule.match` with the Russian `ignore` override.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let view = tokens_without_whitespace(tokens);
        let mut rule_matches = Vec::new();
        let mut prev_token = String::new();
        for i in 1..view.len() {
            let token = view[i].surface().to_string();
            if view[i].is_immunized {
                prev_token.clear();
                continue;
            }
            if crate::wordutil::is_word(&token)
                && crate::wordutil::eq_ignore_case(&prev_token, &token)
                && !russian_ignore(&view, i)
            {
                let prev_pos = view[i - 1].start_pos;
                let pos = view[i].start_pos;
                rule_matches.push(
                    Match::new(
                        "WORD_REPEAT_RULE",
                        Option::<String>::None,
                        "Возможная опечатка: повтор слова",
                        Some("Повтор слова".to_string()),
                        TextRange::new(
                            sentence_offset + prev_pos,
                            sentence_offset + pos + prev_token.len(),
                        ),
                        vec![Suggestion {
                            value: prev_token.clone(),
                            short_description: None,
                        }],
                        "MISC",
                        "Общие правила",
                    )
                    .with_metadata(
                        "Повтор слов (например: «он он»)",
                        "duplication",
                        1,
                    ),
                );
            }
            prev_token = token;
        }
        rule_matches
    }
}

// ---------------------------------------------------------------------------
// RussianFillerWordsRule (AbstractFillerWordsRule, default off)
// ---------------------------------------------------------------------------

pub const FILLER_WORDS_ID: &str = "FILLER_WORDS_RU";

/// `RussianFillerWordsRule.fillerWords`.
const FILLER_WORDS: [&str; 15] = [
    "ах",
    "аа",
    "ааа",
    "аааа",
    "ау",
    "бу",
    "вау",
    "ох",
    "однако",
    "эээ",
    "э",
    "эй",
    "эх",
    "ух-ты",
    "ух",
];

fn filler_word_set() -> &'static std::collections::HashSet<&'static str> {
    static SET: LazyLock<std::collections::HashSet<&'static str>> =
        LazyLock::new(|| FILLER_WORDS.iter().copied().collect());
    &SET
}

fn is_opening_quote(token: &str) -> bool {
    let mut chars = token.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if matches!(c, '"' | '“' | '„' | '»' | '«'))
}

fn is_ending_quote(token: &str) -> bool {
    let mut chars = token.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if matches!(c, '"' | '“' | '”' | '»' | '«'))
}

/// `FILLER_WORDS_RU` (`AbstractFillerWordsRule`, limit 8 %, direct speech
/// excluded). Text-level.
pub fn filler_words(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    const MIN_PERCENT: u32 = 8;
    let mut hints: Vec<(usize, usize, usize)> = Vec::new();
    let mut word_count = 0u32;
    let mut is_direct_speech = false;
    for (sentence_index, sentence) in sentences.iter().enumerate() {
        let tokens = sentence.tokens_without_whitespace();
        for n in 1..tokens.len() {
            let token = tokens[n];
            let surface = token.surface();
            if !is_direct_speech
                && is_opening_quote(surface)
                && n < tokens.len() - 1
                && !tokens[n + 1].whitespace_before
            {
                is_direct_speech = true;
            } else if is_direct_speech
                && is_ending_quote(surface)
                && n > 1
                && !token.whitespace_before
            {
                is_direct_speech = false;
            } else if !is_direct_speech
                && !token.is_whitespace
                && !crate::style_too_often::is_non_word(surface)
            {
                word_count += 1;
                if filler_word_set().contains(surface) {
                    hints.push((sentence_index, token.start_pos, token.end_pos()));
                }
            }
        }
    }
    let num_matches = hints.len() as f64;
    let percent = if word_count > 0 {
        num_matches * 100.0 / word_count as f64
    } else {
        0.0
    };
    if percent <= MIN_PERCENT as f64 {
        return Vec::new();
    }
    hints
        .into_iter()
        .map(|(sentence_index, start, end)| {
            let sentence = &sentences[sentence_index];
            Match::new(
                FILLER_WORDS_ID,
                Option::<String>::None,
                "Это — слово-паразит. Удалите его, если это возможно.",
                Option::<String>::None,
                TextRange::new(sentence.offset + start, sentence.offset + end),
                Vec::new(),
                "CREATIVE_WRITING",
                "Стилистические подсказки для творческого письма",
            )
            .with_metadata("Слова-паразиты", "style", 0)
        })
        .collect()
}

fn tokens_without_whitespace(tokens: &[AnalyzedTokenReadings]) -> Vec<&AnalyzedTokenReadings> {
    tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect()
}
