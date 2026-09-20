//! `org.languagetool.rules.ca.DiacriticsCheckFilter` (47 XML references):
//! checks a word against `ca/confusion_pairs.txt` (`form;replacement;postag`
//! lines), optionally against a gender/number desired by the
//! `gendernumberFrom` pattern token, and rewrites the `{suggestion}`-style
//! placeholders of the rule's suggestions.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedTokenReadings, Suggestion};
use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use unicode_normalization::UnicodeNormalization;

/// `StringTools.hasDiacritics`: `removeDiacritics` (NFD + strip the
/// U+0300–U+036F combining marks) changes the string.
fn has_diacritics(s: &str) -> bool {
    s.nfd().any(|c| matches!(c as u32, 0x0300..=0x036F))
}

static MS: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(?:NC[MC][SN]000|A..[MC][SN].|V.P..SM.)$").unwrap());
static FS: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(?:NC[FC][SN]000|A..[FC][SN].|V.P..SF.)$").unwrap());
static MP: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(?:NC[MC][PN]000|A..[MC][PN].|V.P..PM.)$").unwrap());
static FP: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(?:NC[FC][PN]000|A..[FC][PN].|V.P..PF.)$").unwrap());
static CP: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(?:NC[MFC][PN]000|A..[MFC][PN].|V.P..P..)$").unwrap());
static CS: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(?:NC[MFC][SN]000|A..[MFC][SN].|V.P..S..)$").unwrap());

/// `AnalyzedTokenReadings.matchesPosTagRegex` (full match, non-null tags).
fn matches_pos_tag_regex(atr: &AnalyzedTokenReadings, pattern: &str) -> bool {
    let Ok(re) = regex::Regex::new(&format!("^(?:{pattern})$")) else {
        return false;
    };
    atr.readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|tag| re.is_match(tag)))
}

pub struct DiacriticsCheckFilter {
    /// `ConfusionPairsDataLoader.loadWords("/ca/confusion_pairs.txt")`:
    /// lower-case form → (replacement token, POS tag) readings.
    words: HashMap<String, Vec<(String, String)>>,
}

impl DiacriticsCheckFilter {
    pub fn load(data_dir: &Path) -> Self {
        let mut words: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let Ok(text) = lt_data::fs::read_to_string(data_dir.join("ca/rules/confusion_pairs.txt"))
        else {
            return Self { words };
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split(';').collect();
            if parts.len() != 3 {
                continue;
            }
            words
                .entry(parts[0].to_string())
                .or_default()
                .push((parts[1].to_string(), parts[2].to_string()));
        }
        Self { words }
    }
}

impl RuleFilter for DiacriticsCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Some(postag), Some(original_form)) = (ctx.args.get("postag"), ctx.args.get("form"))
        else {
            return FilterOutcome::reject();
        };
        let is_all_uppercase = lt_tagger::is_all_uppercase(original_form);
        let is_capitalized = lt_tagger::is_capitalized_word(original_form);
        let form = original_form.to_lowercase();

        let gendernumber_from = ctx.args.get("gendernumberFrom");
        let mut desired_gender_number: Option<&regex::Regex> = None;
        if let Some(value) = gendernumber_from {
            let Ok(index) = value.parse::<usize>() else {
                return FilterOutcome::reject();
            };
            if index < 1 || index > ctx.pattern_tokens.len() {
                return FilterOutcome::reject();
            }
            let atr = ctx.pattern_tokens[index - 1];
            desired_gender_number = if matches_pos_tag_regex(atr, "[NAPD].+MS.*|V.P..SM.") {
                Some(&MS)
            } else if matches_pos_tag_regex(atr, "[NAPD].+MP.*|V.P..PM.") {
                Some(&MP)
            } else if matches_pos_tag_regex(atr, "[NAPD].+FS.*|V.P..SF.") {
                Some(&FS)
            } else if matches_pos_tag_regex(atr, "[NAPD].+FP.*|V.P..PF.") {
                Some(&FP)
            } else if matches_pos_tag_regex(atr, "[NAPD].+CP.*|V.P..P..") {
                Some(&CP)
            } else if matches_pos_tag_regex(atr, "[NAPD].+CS.*|V.P..S..") {
                Some(&CS)
            } else {
                None
            };
        }

        let mut replacement: Option<&str> = None;
        if let Some(readings) = self.words.get(&form) {
            if readings.iter().any(|(_, tag)| {
                regex::Regex::new(&format!("^(?:{postag})$"))
                    .map(|re| re.is_match(tag))
                    .unwrap_or(false)
            }) {
                if let Some(desired) = desired_gender_number {
                    if desired.is_match(&readings[0].1) {
                        replacement = Some(&readings[0].0);
                    }
                } else if gendernumber_from.is_none() {
                    replacement = Some(&readings[0].0);
                }
            }
        }
        let Some(replacement) = replacement else {
            return FilterOutcome::reject();
        };

        let mut message = ctx.message.clone();
        // Change the message if the replacement has no diacritic
        if !(has_diacritics(replacement) && !has_diacritics(&form)) {
            message = message.replace("s'escriu amb accent", "s'escriu d'una altra manera");
        }
        let mut replacement = replacement.to_string();
        if is_all_uppercase {
            replacement = replacement.to_uppercase();
        }
        if is_capitalized {
            replacement = lt_tagger::uppercase_first_char(&replacement);
        }
        let suggestions: Vec<Suggestion> = ctx
            .suggestions
            .iter()
            .map(|s| Suggestion {
                value: s
                    .value
                    .replace("{suggestion}", &replacement)
                    .replace(
                        "{Suggestion}",
                        &lt_tagger::uppercase_first_char(&replacement),
                    )
                    .replace("{SUGGESTION}", &replacement.to_uppercase()),
                short_description: s.short_description.clone(),
            })
            .collect();
        FilterOutcome {
            accepted: true,
            range: None,
            message: Some(message),
            suggestions: Some(suggestions),
        }
    }
}
