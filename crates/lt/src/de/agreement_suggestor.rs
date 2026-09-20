//! `AgreementSuggestor2`: suggestions for German noun-phrase agreement
//! errors, using the German synthesizer and `PrepositionToCases`.

use std::sync::Arc;

use lt_core::AnalyzedToken;
use lt_core::AnalyzedTokenReadings;

use super::preposition_to_cases;

const DET_TEMPLATE: &str = "ART:IND/DEF:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU";
const PRO_POS_TEMPLATES: [&str; 2] = [
    "PRO:POS:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:BEG",
    "PRO:POS:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:B/S",
];
const PRO_DEM_TEMPLATES: [&str; 2] = [
    "PRO:DEM:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:BEG",
    "PRO:DEM:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:B/S",
];
const PRO_IND_TEMPLATES: [&str; 2] = [
    "PRO:IND:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:BEG",
    "PRO:IND:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:B/S",
];
const ADJ_TEMPLATE: &str = "ADJ:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:GRU:IND/DEF";
const PA1_TEMPLATE: &str = "PA1:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:GRU:IND/DEF:VER";
const PA2_TEMPLATE: &str = "PA2:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:GRU:IND/DEF:VER";
const NOUN_TEMPLATES: [&str; 2] = [
    "SUB:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU",
    "SUB:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:INF",
];
const NUMBER: [&str; 2] = ["SIN", "PLU"];
const GENDER: [&str; 4] = ["MAS", "FEM", "NEU", "NOG"];
const CASES: [&str; 4] = ["NOM", "AKK", "DAT", "GEN"];
const NOUN_CASES: [&str; 4] = ["NOM", "AKK", "DAT", "GEN"];
const SKIP_SUGGESTIONS: [&str; 7] = [
    "unsren", "unsrem", "unsres", "unsre", "unsern", "unserm", "unsrer",
];

/// `AgreementRule.ReplacementType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplacementType {
    Ins,
    Zur,
}

/// `AgreementSuggestor2.Suggestion`: `compareTo` sorts by (token-level edits,
/// char-level corrections); `equals` compares (phrase, token-level edits),
/// like the Java class.
#[derive(Debug, Clone)]
struct Suggestion {
    phrase: String,
    token_level_edits: i32,
    char_level_corrections: i32,
}

impl PartialEq for Suggestion {
    fn eq(&self, other: &Self) -> bool {
        self.token_level_edits == other.token_level_edits && self.phrase == other.phrase
    }
}

impl Eq for Suggestion {}

pub(crate) struct AgreementSuggestor2<'a> {
    synth: &'a lt_tagger::GermanSynthesizer,
    determiner_token: &'a AnalyzedTokenReadings,
    adj_token1: Option<&'a AnalyzedTokenReadings>,
    adj_token2: Option<&'a AnalyzedTokenReadings>,
    noun_token: &'a AnalyzedTokenReadings,
    replacement_type: Option<ReplacementType>,
    orig_phrase: String,
    preposition_token: Option<AnalyzedToken>,
    skipped_str: Option<String>,
}

impl<'a> AgreementSuggestor2<'a> {
    pub(crate) fn new(
        synth: &'a Arc<lt_tagger::GermanSynthesizer>,
        determiner_token: &'a AnalyzedTokenReadings,
        adj_token1: Option<&'a AnalyzedTokenReadings>,
        adj_token2: Option<&'a AnalyzedTokenReadings>,
        noun_token: &'a AnalyzedTokenReadings,
        replacement_type: Option<ReplacementType>,
    ) -> Self {
        let mut orig_phrase = determiner_token.surface().to_string();
        if let Some(adj1) = adj_token1 {
            orig_phrase.push(' ');
            orig_phrase.push_str(adj1.surface());
        }
        if let Some(adj2) = adj_token2 {
            orig_phrase.push(' ');
            orig_phrase.push_str(adj2.surface());
        }
        orig_phrase.push(' ');
        orig_phrase.push_str(noun_token.surface());
        Self {
            synth,
            determiner_token,
            adj_token1,
            adj_token2,
            noun_token,
            replacement_type,
            orig_phrase,
            preposition_token: None,
            skipped_str: None,
        }
    }

    pub(crate) fn set_preposition(&mut self, prep: Option<&AnalyzedTokenReadings>) {
        self.preposition_token = prep.map(|p| AnalyzedToken::new(p.surface(), None, None));
    }

    pub(crate) fn set_skipped(&mut self, skipped: Option<String>) {
        self.skipped_str = skipped;
    }

    /// `AgreementSuggestor2.getSuggestions(boolean)`.
    pub(crate) fn get_suggestions(&mut self, filter: bool) -> Vec<String> {
        if self.replacement_type == Some(ReplacementType::Zur) {
            self.preposition_token = Some(AnalyzedToken::new("zu", Some("zu".to_string()), None));
        } else if self.replacement_type == Some(ReplacementType::Ins) {
            self.preposition_token = Some(AnalyzedToken::new("in", Some("zu".to_string()), None));
        }
        let mut suggestions = self.get_suggestions_internal();
        // Java `Collections.sort` is stable; compareTo = (edits, chars)
        suggestions.sort_by_key(|s| (s.token_level_edits, s.char_level_corrections));
        self.add_contraction(&mut suggestions);
        if filter {
            let mut filtered: Vec<Suggestion> = Vec::new();
            let prev_corrections = suggestions
                .first()
                .map(|s| s.token_level_edits)
                .unwrap_or(0);
            let mut had_real_suggestions = false;
            for suggestion in suggestions {
                if had_real_suggestions && suggestion.token_level_edits > prev_corrections {
                    break;
                }
                had_real_suggestions = suggestion.token_level_edits > 0;
                filtered.push(suggestion);
            }
            filtered.into_iter().map(|s| s.phrase).collect()
        } else {
            suggestions.into_iter().map(|s| s.phrase).collect()
        }
    }

    /// `AgreementSuggestor2.addContraction`.
    fn add_contraction(&self, suggestions: &mut Vec<Suggestion>) {
        if self.replacement_type == Some(ReplacementType::Zur) {
            for suggestion in suggestions {
                if suggestion.phrase.starts_with("der") {
                    suggestion.phrase = replace_first(&suggestion.phrase, "der", "zur");
                } else if suggestion.phrase.starts_with("den") {
                    suggestion.phrase = replace_first(&suggestion.phrase, "den", "zu");
                } else if suggestion.phrase.starts_with("dem") {
                    suggestion.phrase = replace_first(&suggestion.phrase, "dem", "zum");
                }
            }
        } else if self.replacement_type == Some(ReplacementType::Ins) {
            suggestions.retain_mut(|suggestion| {
                if suggestion.phrase.starts_with("das") {
                    suggestion.phrase = replace_first(&suggestion.phrase, "das", "ins");
                    true
                } else if suggestion.phrase.starts_with("dem") {
                    suggestion.phrase = replace_first(&suggestion.phrase, "dem", "im");
                    true
                } else if suggestion.phrase.starts_with("den") {
                    suggestion.phrase = replace_first(&suggestion.phrase, "den", "in den");
                    true
                } else if suggestion.phrase.starts_with("die") {
                    suggestion.phrase = replace_first(&suggestion.phrase, "die", "in die");
                    true
                } else {
                    false
                }
            });
        }
    }

    fn get_suggestions_internal(&self) -> Vec<Suggestion> {
        let noun_cases = self.get_noun_cases();
        let mut result: Vec<Suggestion> = Vec::new();
        for num in NUMBER {
            for gen in GENDER {
                for a_case in CASES {
                    if !noun_cases.contains(&a_case) {
                        continue;
                    }
                    for det_reading in &self.determiner_token.readings {
                        if gen == "NOG" {
                            let det_synth =
                                self.get_det_or_pronoun_synth(num, "MAS", a_case, det_reading);
                            let adj1_synth = self.get_adj_synth(
                                num,
                                "MAS",
                                a_case,
                                self.adj_token1,
                                det_reading,
                            );
                            let adj2_synth = self.get_adj_synth(
                                num,
                                "MAS",
                                a_case,
                                self.adj_token2,
                                det_reading,
                            );
                            let noun_synth = self.get_noun_synth(num, "NOG", a_case);
                            self.combine_synth(
                                &mut result,
                                &det_synth,
                                &adj1_synth,
                                &adj2_synth,
                                &noun_synth,
                            );
                        } else {
                            let det_synth =
                                self.get_det_or_pronoun_synth(num, gen, a_case, det_reading);
                            let adj1_synth =
                                self.get_adj_synth(num, gen, a_case, self.adj_token1, det_reading);
                            let adj2_synth =
                                self.get_adj_synth(num, gen, a_case, self.adj_token2, det_reading);
                            let noun_synth = self.get_noun_synth(num, gen, a_case);
                            self.combine_synth(
                                &mut result,
                                &det_synth,
                                &adj1_synth,
                                &adj2_synth,
                                &noun_synth,
                            );
                        }
                    }
                }
            }
        }
        result
    }

    /// `AgreementSuggestor2.getNounCases`.
    fn get_noun_cases(&self) -> Vec<&'static str> {
        if let Some(preposition) = &self.preposition_token {
            let cases = preposition_to_cases::get_cases_for(&preposition.token);
            if !cases.is_empty() {
                return cases;
            }
        }
        NOUN_CASES.to_vec()
    }

    /// `getDetOrPronounSynth`.
    fn get_det_or_pronoun_synth(
        &self,
        num: &str,
        gen: &str,
        a_case: &str,
        det_reading: &AnalyzedToken,
    ) -> Vec<String> {
        let Some(det_pos) = det_reading.pos_tag.as_deref() else {
            return Vec::new();
        };
        let mut is_def = det_pos.contains(":DEF:");
        let mut det_reading = det_reading.clone();
        let mut templates: Vec<&str> = Vec::new();
        if SAME_TEMPLATE.is_match(&det_reading.token) {
            templates.push("PRO:DEM:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU");
        } else if WELCHE_TEMPLATE.is_match(&det_reading.token) {
            templates.push("PRO:RIN:NOM/AKK/DAT/GEN:SIN/PLU:MAS/FEM/NEU:B/S");
        } else if det_pos.contains("ART:") {
            templates.push(DET_TEMPLATE);
        } else if det_pos.contains("PRO:POS:") {
            templates.extend(PRO_POS_TEMPLATES);
        } else if det_pos.contains("PRO:DEM:") {
            templates.extend(PRO_DEM_TEMPLATES);
        } else if det_pos.contains("PRO:IND:") {
            templates.extend(PRO_IND_TEMPLATES);
        } else if det_reading.token == "zur" {
            templates.push(DET_TEMPLATE);
            det_reading = AnalyzedToken::new("der", Some("der".to_string()), None);
            is_def = true;
        } else if det_reading.token == "ins" {
            templates.push(DET_TEMPLATE);
            det_reading = AnalyzedToken::new("das", Some("der".to_string()), None);
            is_def = true;
        } else {
            return Vec::new();
        }
        let mut synthesized: Vec<String> = Vec::new();
        for template in templates {
            let template = replace_first(template, "IND/DEF", if is_def { "DEF" } else { "IND" });
            let pos = replace_vars(&template, num, gen, a_case);
            let tmp = self.synth.synthesize(&det_reading, &pos, false);
            let orig_first_char = det_reading.token.chars().next().unwrap_or(' ');
            let first_lower = orig_first_char.to_lowercase().next().unwrap_or(' ');
            for k in tmp {
                if k.chars()
                    .next()
                    .map(|c| c.to_lowercase().next().unwrap_or(' '))
                    .is_none_or(|c| c != first_lower)
                {
                    continue;
                }
                if SKIP_SUGGESTIONS.contains(&k.to_lowercase().as_str()) {
                    continue;
                }
                synthesized.push(if orig_first_char.is_uppercase() {
                    lt_tagger::uppercase_first_char(&k)
                } else {
                    k
                });
            }
        }
        synthesized
    }

    /// `getAdjSynth`.
    fn get_adj_synth(
        &self,
        num: &str,
        gen: &str,
        a_case: &str,
        adj_token: Option<&AnalyzedTokenReadings>,
        det_reading: &AnalyzedToken,
    ) -> Vec<String> {
        let mut adj_synthesized: Vec<String> = Vec::new();
        let Some(adj_token) = adj_token else {
            adj_synthesized.push(String::new());
            return adj_synthesized;
        };
        for adj_reading in &adj_token.readings {
            if adj_reading.pos_tag.is_none() || det_reading.pos_tag.is_none() {
                continue;
            }
            if adj_reading.token == "meisten" && num == "SIN" {
                continue;
            }
            let Some(adj_pos) = adj_reading.pos_tag.as_deref() else {
                continue;
            };
            if adj_pos.starts_with("ADV:") {
                adj_synthesized.push(adj_reading.token.clone());
                continue;
            }
            let det_is_def = det_reading
                .pos_tag
                .as_deref()
                .is_some_and(|p| p.contains(":DEF:"))
                || det_reading.token == "ins";
            let mut template = if adj_pos.starts_with("PA1") {
                PA1_TEMPLATE
            } else if adj_pos.starts_with("PA2") {
                PA2_TEMPLATE
            } else {
                ADJ_TEMPLATE
            }
            .to_string();
            if adj_pos.contains(":KOM:") {
                template = replace_first(&template, ":GRU:", ":KOM:");
            } else if adj_pos.contains(":SUP:") {
                template = replace_first(&template, ":GRU:", ":SUP:");
            }
            let template =
                replace_first(&template, "IND/DEF", if det_is_def { "DEF" } else { "IND" });
            let adj_pos = replace_vars(&template, num, gen, a_case);
            for synth_noun in self.synth.synthesize(adj_reading, &adj_pos, false) {
                if !adj_synthesized.contains(&synth_noun) {
                    adj_synthesized.push(synth_noun);
                }
            }
        }
        adj_synthesized
    }

    /// `getNounSynth`.
    fn get_noun_synth(&self, num: &str, gen: &str, a_case: &str) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        for noun_reading in &self.noun_token.readings {
            for noun_template in NOUN_TEMPLATES {
                let noun_pos = replace_vars(noun_template, num, gen, a_case);
                let noun_synthesized = self.synth.synthesize(noun_reading, &noun_pos, false);
                if noun_synthesized.is_empty() && noun_reading.token.contains('-') {
                    let first_part =
                        &noun_reading.token[..=noun_reading.token.rfind('-').unwrap_or(0)];
                    let last_token_part = noun_reading
                        .token
                        .rsplit_once('-')
                        .map(|(_, last)| last.to_string())
                        .unwrap_or_default();
                    let last_lemma_part = noun_reading.stem.as_deref().map(|l| {
                        l.rsplit_once('-')
                            .map(|(_, last)| last)
                            .unwrap_or(l)
                            .to_string()
                    });
                    let fake = AnalyzedToken::new(
                        last_token_part,
                        last_lemma_part,
                        Some("fake_value".to_string()),
                    );
                    for last_part_inflected in self.synth.synthesize(&fake, &noun_pos, false) {
                        result.push(format!("{first_part}{last_part_inflected}"));
                    }
                } else {
                    result.extend(noun_synthesized);
                }
            }
        }
        // remove old-spelling forms like "Blutfluß" when "Blutfluss" exists
        let old_spelling: Vec<String> = result
            .iter()
            .filter(|k| k.contains("ss"))
            .map(|k| k.replace("ss", "ß"))
            .collect();
        result
            .into_iter()
            .filter(|k| !old_spelling.contains(k))
            .collect()
    }

    /// `combineSynth`.
    #[allow(clippy::too_many_arguments)]
    fn combine_synth(
        &self,
        result: &mut Vec<Suggestion>,
        det_synthesized: &[String],
        adj1_synthesized: &[String],
        adj2_synthesized: &[String],
        noun_synthesized: &[String],
    ) {
        for det in det_synthesized {
            for adj1 in adj1_synthesized {
                for adj2 in adj2_synthesized {
                    for noun in noun_synthesized {
                        let mut elem = det.clone();
                        if let Some(skipped) = &self.skipped_str {
                            elem.push(' ');
                            elem.push_str(skipped);
                        }
                        if !adj1.is_empty() {
                            elem.push(' ');
                            elem.push_str(adj1);
                        }
                        if !adj2.is_empty() {
                            elem.push(' ');
                            elem.push_str(adj2);
                        }
                        elem.push(' ');
                        elem.push_str(noun);
                        let edits = i32::from(det != self.determiner_token.surface())
                            + i32::from(self.adj_token1.is_some_and(|t| adj1 != t.surface()))
                            + i32::from(self.adj_token2.is_some_and(|t| adj2 != t.surface()))
                            + i32::from(noun != self.noun_token.surface());
                        if edits == 0 {
                            continue;
                        }
                        let char_level_edits =
                            super::util::levenshtein(&elem, &self.orig_phrase) as i32;
                        let suggestion = Suggestion {
                            phrase: elem,
                            token_level_edits: edits,
                            char_level_corrections: char_level_edits,
                        };
                        if !result.contains(&suggestion) {
                            result.push(suggestion);
                        }
                    }
                }
            }
        }
    }
}

/// `StringUtils.replaceOnce`: replaces the first occurrence of `search`.
fn replace_first(text: &str, search: &str, replacement: &str) -> String {
    match text.find(search) {
        Some(idx) => {
            let mut out = String::with_capacity(text.len() + replacement.len());
            out.push_str(&text[..idx]);
            out.push_str(replacement);
            out.push_str(&text[idx + search.len()..]);
            out
        }
        None => text.to_string(),
    }
}

/// `replaceVars`.
fn replace_vars(template: &str, num: &str, gen: &str, a_case: &str) -> String {
    let template = replace_first(template, "SIN/PLU", num);
    let template = replace_first(&template, "MAS/FEM/NEU", gen);
    replace_first(&template, "NOM/AKK/DAT/GEN", a_case)
}

static SAME_TEMPLATE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"^(?:([Dd]as|[Dd]er|[Dd]ie|[Dd]em|[Dd]es)selben?)$").unwrap()
});
static WELCHE_TEMPLATE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^(?:[Ww]elche[nmsr]?)$").unwrap());
