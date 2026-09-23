//! Arabic Java rule classes: the `AbstractSimpleReplaceRule2` instances
//! (`ArabicSimpleReplaceRule`, `ArabicDiacriticsRule`, `ArabicDarjaRule`,
//! `ArabicHomophonesRule`, `ArabicRedundancyRule`, `ArabicWordinessRule`)
//! plus the two custom subclasses (`ArabicTransVerbRule`,
//! `ArabicInflectedOneWordReplaceRule`).

use std::collections::HashMap;
use std::path::Path;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::simple_replace::{
    CaseSensitivity, SimpleReplaceConfig, SimpleReplaceRule, TokenException,
};

/// `AbstractSimpleReplaceRule2.fillMaps` for the single-word (no-space) map
/// the two custom `match` overrides consult (`getWrongWords().get(size - 1)`
/// = `mFullNoSpace`): `wrongForm[|wrongForm...]=suggestion[\tmessage]` lines,
/// keys lowercased (`CI`).
fn load_no_space_map(path: &Path) -> HashMap<String, (String, Option<String>)> {
    let mut map = HashMap::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return map;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.split('#').next().unwrap_or(line).trim();
        let mut parts = line.split('\t');
        let Some(conf_pair) = parts.next() else {
            continue;
        };
        let msg = parts.next().map(str::to_string);
        let mut conf_pair_parts = conf_pair.split('=');
        let Some(wrong_forms) = conf_pair_parts.next() else {
            continue;
        };
        let Some(suggestion) = conf_pair_parts.next() else {
            continue;
        };
        for wrong_form in wrong_forms.split('|') {
            // `wrongForm.indexOf(' ') > 0` entries go to `mFullSpace`, which
            // the two rules never read.
            if wrong_form.contains(' ') {
                continue;
            }
            map.insert(
                wrong_form.to_lowercase(),
                (suggestion.to_string(), msg.clone()),
            );
        }
    }
    map
}

/// The six `AbstractSimpleReplaceRule2` instances in `Arabic.getRelevantRules`
/// order (8, 12, 13, 14, 15, 17): simple replace, diacritics, darja,
/// homophones, redundancy, wordiness.
#[allow(clippy::vec_init_then_push)]
pub fn simple_replace_instances(data_dir: &Path) -> Result<Vec<SimpleReplaceRule>> {
    let rules = data_dir.join("ar/rules");
    let mut instances = Vec::new();
    // `ArabicSimpleReplaceRule` (8): `AR_SIMPLE_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("replaces.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_SIMPLE_REPLACE",
            description: "قاعدة تطابق الكلمات التي يجب تجنبها وتقترح تصويبا لها",
            short: "خطأ، يفضل أن  يقال:",
            message: "قل $suggestions",
            suggestions_separator: " أو  ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "CONFUSED_WORDS",
            category_name: "كلمات ملتبسة شائعة",
            issue_type: "uncategorized",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `ArabicDiacriticsRule` (12): `AR_DIACRITICS_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("diacritics.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_DIACRITICS_REPLACE",
            description: "كلمات مشكولة للتوضيح",
            short: "كلمات يستحسن أن تشكّل لتصحيح نطقها",
            message: "'$match' كلمة يشيع نطقها نطقا خاطئا لذا نقترح تشكيلها كالآتي: $suggestions",
            suggestions_separator: " أو ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "STYLE",
            category_name: "الأسلوب",
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `ArabicDarjaRule` (13): `AR_DARJA_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("darja.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_DARJA_REPLACE",
            description: "كلمات بديلة للكلمات العامية أو الأجنبية",
            short: "كلمات بديلة للكلمات العامية أو الأجنبية",
            message: "الكلمة عامية  أو أجنبية يفضل أن يقال $suggestions",
            suggestions_separator: " أو  ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "STYLE",
            category_name: "الأسلوب",
            issue_type: "locale-violation",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `ArabicHomophonesRule` (14): `AR_HOMOPHONES_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("homophones.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_HOMOPHONES_REPLACE",
            description: "كلمات متشابهة لفظا للتوضيح، يرجى التحقق منها مثل تشابه الظاء والضاد.",
            short: "كلمات متشابهة لفظا يرجى التحقق منها",
            message: "قل $suggestions",
            suggestions_separator: " أو  ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "CONFUSED_WORDS",
            category_name: "كلمات ملتبسة شائعة",
            issue_type: "uncategorized",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `ArabicRedundancyRule` (15): `AR_REDUNDANCY_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("redundancies.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_REDUNDANCY_REPLACE",
            description: "1. تكرار (عام)",
            short: "تكرار",
            message: "'$match' تعبير فيه تكرار.في بعض الحالات، يستحسن استعمال $suggestions",
            suggestions_separator: " أو ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "REDUNDANCY",
            category_name: "العبارات المسهَبة",
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    // `ArabicWordinessRule` (17): `AR_WORDINESS_REPLACE`.
    instances.push(SimpleReplaceRule::from_files(
        &[rules.join("wordiness.txt")],
        SimpleReplaceConfig {
            rule_id: "AR_WORDINESS_REPLACE",
            description: "2. حشو(تعبير فيه تكرار)",
            short: "حشو (تعبير فيه تكرار)",
            message: "'$match' تعبير فيه حشو يفضل أن يقال $suggestions",
            suggestions_separator: " أو ",
            sub_rule_specific_ids: false,
            case_sensitivity: CaseSensitivity::Ci,
            category_id: "REDUNDANCY",
            category_name: "العبارات المسهَبة",
            issue_type: "style",
            default_off: false,
            picky: false,
            has_suggestions: true,
            checking_case: false,
            ignore_short_uppercase_words: true,
            is_token_exception: TokenException::None,
        },
    )?);
    Ok(instances)
}

/// `Categories.MISC.getCategory(messages)` for `ar`
/// (`MessagesBundle_ar.properties`: `category_misc`).
const MISC_CATEGORY_NAME: &str = "متنوع";

/// `SuggestionWithMessage` (suggestion + optional per-entry message).
type SuggestionWithMessage = (String, Option<String>);

/// `org.languagetool.rules.ar.ArabicTransVerbRule`
/// (`AR_VERB_TRANSITIVE_IINDIRECT`): an indirect-transitive verb used
/// transitively is corrected to verb + preposition
/// (`أفاضَ الخيرَ` -> `أفاض في`).
///
/// The Java rule also instantiates a `newStylePronounTag` tagger in its
/// constructor, but the field is never read by `match`, so the tagger mode
/// has no effect on the rule's output.
pub struct ArabicTransVerbRule {
    wrong_words: HashMap<String, SuggestionWithMessage>,
}

impl ArabicTransVerbRule {
    /// `new ArabicTransVerbRule(messages)` over
    /// `ar/rules/verb_trans_to_untrans2.txt`.
    pub fn load(data_dir: &Path) -> Self {
        Self {
            wrong_words: load_no_space_map(&data_dir.join("ar/rules/verb_trans_to_untrans2.txt")),
        }
    }

    pub fn rule_id(&self) -> &'static str {
        "AR_VERB_TRANSITIVE_IINDIRECT"
    }

    /// `ArabicTransVerbRule.isAttachedTransitiveVerb`.
    fn is_attached_transitive_verb(&self, token: &AnalyzedTokenReadings) -> bool {
        for verb_tok in &token.readings {
            if verb_tok.pos_tag.is_some() {
                // lookup in WrongWords
                if let Some(lemma) = &verb_tok.stem {
                    if self.wrong_words.contains_key(lemma) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// `ArabicTransVerbRule.getProperPrepositionForTransitiveVerb`.
    fn proper_prepositions(&self, token: &AnalyzedTokenReadings) -> Vec<String> {
        for verb_tok in &token.readings {
            if verb_tok.pos_tag.is_some() {
                if let Some(lemma) = &verb_tok.stem {
                    if let Some((suggestion, _)) = self.wrong_words.get(lemma) {
                        return suggestion.split('|').map(str::to_string).collect();
                    }
                }
            }
        }
        Vec::new()
    }

    /// `ArabicTransVerbRule.isRightPreposition`.
    fn is_right_preposition(next_token: &AnalyzedTokenReadings, prepositions: &[String]) -> bool {
        next_token
            .readings
            .first()
            .and_then(|r| r.stem.as_deref())
            .is_some_and(|lemma| prepositions.contains(&lemma.to_string()))
    }

    /// `ArabicTransVerbRule.generateNewForm`.
    fn generate_new_form(
        &self,
        synthesizer: &lt_tagger::ArabicSynthesizer,
        word: &str,
        pos_tag: &str,
        flag: char,
    ) -> String {
        use lt_tagger::arabic::ArabicTagManager;
        let mut new_pos_tag = ArabicTagManager::set_flag(pos_tag, "PRONOUN", flag);
        // FIXME: remove the specific flag for option D (upstream comment)
        if flag != '-' {
            new_pos_tag = ArabicTagManager::set_flag(&new_pos_tag, "OPTION", 'D');
        }
        let prep_a_token = AnalyzedToken::new(
            word.to_string(),
            Some(word.to_string()),
            Some(new_pos_tag.clone()),
        );
        let new_word_list = synthesizer.synthesize(&prep_a_token, &new_pos_tag);
        new_word_list.first().cloned().unwrap_or_default()
    }

    /// `ArabicTransVerbRule.generateUnattachedNewForm` (`None` for Java's NPE
    /// on an untagged first reading).
    fn generate_unattached_new_form(
        &self,
        synthesizer: &lt_tagger::ArabicSynthesizer,
        token: &AnalyzedTokenReadings,
    ) -> Option<String> {
        let readings = token.readings.first()?;
        let lemma = readings.stem.as_deref()?;
        let postag = readings.pos_tag.as_deref()?;
        Some(self.generate_new_form(synthesizer, lemma, postag, '-'))
    }

    /// `ArabicTransVerbRule.generateAttachedNewForm`.
    fn generate_attached_new_form(
        &self,
        synthesizer: &lt_tagger::ArabicSynthesizer,
        preposition_lemma: &str,
        prev_token: &AnalyzedTokenReadings,
    ) -> Option<String> {
        // FIXME ; generate multiple cases (upstream comment)
        let postag = "PR-;---;---";
        let prev_postag = prev_token.readings.first()?.pos_tag.as_deref()?;
        let flag = lt_tagger::arabic::ArabicTagManager::get_flag(prev_postag, "PRONOUN");
        Some(self.generate_new_form(synthesizer, preposition_lemma, postag, flag))
    }

    /// `ArabicTransVerbRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        synthesizer: &lt_tagger::ArabicSynthesizer,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches: Vec<Match> = Vec::new();
        if self.wrong_words.is_empty() {
            return rule_matches;
        }
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut prev_token_index = 0usize;
        for i in 1..non_blank.len() {
            // ignoring token 0, i.e., SENT_START
            let token = non_blank[i];
            let prev_token = if prev_token_index > 0 {
                Some(non_blank[prev_token_index])
            } else {
                None
            };
            if let Some(prev) = prev_token {
                let prev_token_str = prev.surface();
                // test if the first token is a verb
                let is_attached_verb_transitive = self.is_attached_transitive_verb(prev);
                // test if the preposition token is suitable for verb token (previous)
                let prepositions = self.proper_prepositions(prev);
                let is_right_preposition = Self::is_right_preposition(token, &prepositions);
                // the verb is attached and the next token is not the suitable
                // preposition: we give the correct new form
                if is_attached_verb_transitive && !is_right_preposition {
                    if let Some(verb) = self.generate_unattached_new_form(synthesizer, prev) {
                        // FIXME: test all suggestions (upstream comment)
                        let new_preposition = prepositions[0].clone();
                        if let Some(preposition) =
                            self.generate_attached_new_form(synthesizer, &new_preposition, prev)
                        {
                            let replacement = format!("{verb} {preposition}");
                            let msg = format!(
                                "قل <suggestion>{replacement}</suggestion> بدلا من '{prev_token_str}' لأنّ الفعل  متعد بحرف  ."
                            );
                            let m = Match::new(
                                self.rule_id(),
                                Option::<String>::None,
                                msg,
                                Some("خطأ في الفعل المتعدي بحرف".to_string()),
                                TextRange::new(
                                    sentence_offset + prev.start_pos,
                                    sentence_offset + prev.end_pos(),
                                ),
                                vec![Suggestion {
                                    value: replacement,
                                    short_description: None,
                                }],
                                "MISC",
                                MISC_CATEGORY_NAME,
                            )
                            .with_metadata(
                                "َTransitive verbs corrected to indirect transitive",
                                "misspelling",
                                0,
                            )
                            .with_match_type("Other");
                            rule_matches.push(m);
                        }
                    }
                }
            }
            if self.is_attached_transitive_verb(token) {
                prev_token_index = i;
            } else {
                prev_token_index = 0;
            }
        }
        rule_matches
    }
}

/// `org.languagetool.rules.ar.ArabicInflectedOneWordReplaceRule`
/// (`AR_INFLECTED_ONE_WORD`): a wrong word whose lemma is listed in
/// `ar/rules/inflected_one_word.txt` is replaced with the suggestion lemma
/// inflected like the source token
/// (`أبحاثه` -> `بحوثهما|بحوثه|بحوث|بحوثها`).
///
/// The Java rule also instantiates a `newStylePronounTag` tagger in its
/// constructor, but the field is never read by `match`, so the tagger mode
/// has no effect on the rule's output.
pub struct ArabicInflectedOneWordReplaceRule {
    wrong_words: HashMap<String, SuggestionWithMessage>,
}

impl ArabicInflectedOneWordReplaceRule {
    /// `new ArabicInflectedOneWordReplaceRule(messages)` over
    /// `ar/rules/inflected_one_word.txt`.
    pub fn load(data_dir: &Path) -> Self {
        Self {
            wrong_words: load_no_space_map(&data_dir.join("ar/rules/inflected_one_word.txt")),
        }
    }

    pub fn rule_id(&self) -> &'static str {
        "AR_INFLECTED_ONE_WORD"
    }

    /// `ArabicInflectedOneWordReplaceRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        synthesizer: &lt_tagger::ArabicSynthesizer,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches: Vec<Match> = Vec::new();
        if self.wrong_words.is_empty() {
            return rule_matches;
        }
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        for token in non_blank.iter().skip(1) {
            // ignoring token 0, i.e., SENT_START: browse each word
            for word_tok in &token.readings {
                // test if the first token is a to replace word
                // (`wordPostag != null` gates the WrongWords lookup)
                if word_tok.pos_tag.is_none() {
                    continue;
                }
                let Some((suggestion, sug_msg)) = word_tok
                    .stem
                    .as_deref()
                    .and_then(|l| self.wrong_words.get(l))
                else {
                    continue;
                };
                // generate suggestion according to suggested word
                let propositions: Vec<&str> = suggestion.split('|').collect();
                let sug_msg = sug_msg.as_deref().unwrap_or("");
                let mut replacement = String::new();
                let mut suggestions: Vec<Suggestion> = Vec::new();
                for proposition in propositions {
                    for w in synthesizer.inflect_lemma_like(proposition, word_tok) {
                        replacement.push_str(&format!("<suggestion>{w}</suggestion>&nbsp;"));
                        // the RuleMatch message parser yields a LinkedHashSet
                        if !suggestions.iter().any(|s| s.value == w) {
                            suggestions.push(Suggestion {
                                value: w,
                                short_description: None,
                            });
                        }
                    }
                }
                let msg = format!(
                    "' الكلمة خاطئة {} ' ،{}. استعمل  {}",
                    token.surface(),
                    sug_msg,
                    replacement
                );
                let m = Match::new(
                    self.rule_id(),
                    Option::<String>::None,
                    msg,
                    Some(format!("خطأ في استعمال كلمة:{sug_msg}")),
                    TextRange::new(
                        sentence_offset + token.start_pos,
                        sentence_offset + token.end_pos(),
                    ),
                    suggestions,
                    "MISC",
                    MISC_CATEGORY_NAME,
                )
                .with_metadata(
                    "قاعدة تطابق الكلمات التي يجب تجنبها وتقترح تصويبا لها",
                    "inconsistency",
                    0,
                )
                .with_match_type("Other");
                rule_matches.push(m);
            }
        }
        rule_matches
    }
}
