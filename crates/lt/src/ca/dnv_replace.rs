//! Catalan `AbstractSimpleReplaceLemmasRule` instances:
//! `SimpleReplaceDNVRule` (26), `SimpleReplaceDNVColloquialRule` (27) and
//! `SimpleReplaceDNVSecondaryRule` (28). The rules look up the *lemma* of
//! each reading in the data file and synthesize the replacement with the
//! matched reading's POS tag (Java's `new AnalyzedToken(replacementLemma,
//! replacePOSTag, replacementLemma)` puts the tag in the lemma slot, so the
//! uninflected lemma is usually the effective suggestion).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock};

fn ms_re() -> &'static regex::Regex {
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new("[MFC]S").unwrap());
    &RE
}

fn mp_re() -> &'static regex::Regex {
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new("[MFC]P").unwrap());
    &RE
}

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, TextRange};

use crate::ca::legacy_simple_replace::load_words;
use crate::simple_replace::to_id;

pub const DNV_ID: &str = "CA_SIMPLE_REPLACE_DNV";
pub const DNV_COLLOQUIAL_ID: &str = "CA_SIMPLE_REPLACE_DNV_COLLOQUIAL";
pub const DNV_SECONDARY_ID: &str = "CA_SIMPLE_REPLACE_DNV_SECONDARY";

pub struct DnvReplaceRule {
    pub rule_id: &'static str,
    description: &'static str,
    short: &'static str,
    message: &'static str,
    category_id: &'static str,
    category_name: &'static str,
    wrong_lemmas: HashMap<String, Vec<String>>,
    synth: Arc<lt_tagger::CatalanSynthesizer>,
    /// `SimpleReplaceDNVSecondaryRule.isTokenException`: `_english_ignore_`.
    english_ignore_exception: bool,
}

impl DnvReplaceRule {
    pub fn default_off(&self) -> bool {
        false
    }

    pub fn category_id(&self) -> &'static str {
        self.category_id
    }

    /// `AbstractSimpleReplaceLemmasRule.match` over one sentence.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut out = Vec::new();
        for tr in non_blank.iter().skip(1) {
            if tr.is_sentence_start
                || tr.is_immunized
                || tr.is_ignore_spelling
                || (self.english_ignore_exception && tr.has_pos_tag("_english_ignore_"))
            {
                continue;
            }
            let mut replacement_lemmas: Option<&Vec<String>> = None;
            let mut replace_postag: Option<String> = None;
            let mut original_lemma = String::new();
            for at in &tr.readings {
                if let Some(replacements) = self.wrong_lemmas.get(at.lemma()) {
                    replacement_lemmas = Some(replacements);
                    replace_postag = at.pos_tag.clone();
                    original_lemma = at.lemma().to_string();
                    break;
                }
            }
            let (Some(replacement_lemmas), Some(replace_postag)) =
                (replacement_lemmas, replace_postag)
            else {
                continue;
            };
            let mut possible_replacements: Vec<String> = Vec::new();
            for replacement_lemma in replacement_lemmas {
                // Java: `new AnalyzedToken(replacementLemma, replacePOSTag,
                // replacementLemma)`: (token, posTag, lemma).
                let probe = AnalyzedToken::new(
                    replacement_lemma.clone(),
                    Some(replacement_lemma.clone()),
                    Some(replace_postag.clone()),
                );
                let mut synthesized = self.synth.synthesize(&probe, &replace_postag, true);
                if synthesized.is_empty() {
                    let postag2 = ms_re().replace_all(&replace_postag, ".S").into_owned();
                    let postag2 = mp_re().replace_all(&postag2, ".P").into_owned();
                    synthesized = self.synth.synthesize(&probe, &postag2, true);
                }
                if synthesized.is_empty() && replacement_lemma.chars().count() > 1 {
                    possible_replacements.push(replacement_lemma.clone());
                } else {
                    possible_replacements.extend(synthesized);
                }
            }
            let token_string = tr.surface();
            let mut replacements = possible_replacements;
            if lt_tagger::is_capitalized_word(token_string) {
                for replacement in &mut replacements {
                    *replacement = lt_tagger::uppercase_first_char(replacement);
                }
            }
            out.push(
                Match::new(
                    to_id(&format!("{}_{}", self.rule_id, original_lemma)),
                    Option::<String>::None,
                    self.message,
                    Some(self.short.to_string()),
                    TextRange::new(
                        sentence_offset + tr.start_pos,
                        sentence_offset + tr.start_pos + token_string.len(),
                    ),
                    replacements
                        .into_iter()
                        .map(|value| lt_core::Suggestion {
                            value,
                            short_description: None,
                        })
                        .collect(),
                    self.category_id,
                    self.category_name,
                )
                .with_metadata(self.description, "style", 0)
                .with_match_type("Other"),
            );
        }
        out
    }
}

/// The DNV trio in `Catalan.getRelevantRules` order (26–28).
pub fn catalan_dnv_instances(
    data_dir: &Path,
    synth: Arc<lt_tagger::CatalanSynthesizer>,
) -> Vec<DnvReplaceRule> {
    let make = |rule_id,
                description,
                short,
                message,
                category_id,
                category_name,
                file: &str,
                english_ignore_exception| DnvReplaceRule {
        rule_id,
        description,
        short,
        message,
        category_id,
        category_name,
        wrong_lemmas: load_words(&[data_dir.join(file)]),
        synth: Arc::clone(&synth),
        english_ignore_exception,
    };
    vec![
        make(
            DNV_ID,
            "Detecta paraules admeses només per l'AVL i proposa suggeriments de canvi",
            "Paraula admesa només pel DNV (AVL).",
            "Paraula admesa pel DNV (AVL), però no per altres diccionaris.",
            "REGIONALISMS",
            "Variants regionals",
            "ca/rules/replace_dnv.txt",
            false,
        ),
        make(
            DNV_COLLOQUIAL_ID,
            "Detecta paraules marcades com a col·loquials en el DNV.",
            "Paraula o expressió col·loquial.",
            "Paraula o expressió col·loquial.",
            "COLLOQUIALISMS",
            "Estil col·loquial",
            "ca/rules/replace_dnv_colloquial.txt",
            false,
        ),
        make(
            DNV_SECONDARY_ID,
            "Recomana paraules o formes preferents.",
            "Forma secundària",
            "Paraula o forma secundària.",
            "REGIONALISMS",
            "Variants regionals",
            "ca/rules/replace_dnv_secondary.txt",
            true,
        ),
    ]
}
