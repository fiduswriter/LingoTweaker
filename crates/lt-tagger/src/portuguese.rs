//! Portuguese tagger: the `BaseTagger` lookup (`portuguese.dict` +
//! `added*`/`removed*` manual taggers) plus the `PortugueseTagger.tag()`
//! overrides: ordinal/percent/degree number expressions, `-mente` adverbs,
//! `soto-`-prefixed verbs, typewriter-apostrophe rewriting and the
//! hyphen-aware mixed-case rule.

use std::path::Path;
use std::sync::OnceLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};
use regex::Regex;

use crate::english::{is_mixed_case, uppercase_first_char};
use crate::{Dictionary, DictionaryInfo, ManualTagger};

/// `PortugueseTagger.ADJ_PART_FS`.
fn adj_part_fs() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"V.P..SF.|A[QO].[FC][SN].").unwrap())
}

/// `PortugueseTagger.VERB`.
fn verb_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"V.+").unwrap())
}

/// `PortugueseTagger.PREFIXES_FOR_VERBS` (`(soto-)(...+)`, case-insensitive).
fn prefixes_for_verbs() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(soto-)(...+)").unwrap())
}

/// `ORDINAL_SUFFIXES` = `[oºᵒaªᵃ][sˢ]?`.
fn ordinal_suffixes() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new("[\u{006F}\u{00BA}\u{1D52}\u{0061}\u{00AA}\u{1D43}][\u{0073}\u{02E2}]?").unwrap()
    })
}

fn ordinal_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new("^[0-9]+[0-9,.]*\\.?[\u{006F}\u{00BA}\u{1D52}\u{0061}\u{00AA}\u{1D43}][\u{0073}\u{02E2}]?$")
            .unwrap()
    })
}

fn ordinal_masc_sg() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("[\u{006F}\u{00BA}\u{1D52}]$").unwrap())
}

fn ordinal_fem_sg() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("[\u{0061}\u{00AA}\u{1D43}]$").unwrap())
}

fn ordinal_masc_pl() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("[\u{006F}\u{00BA}\u{1D52}][\u{0073}\u{02E2}]$").unwrap())
}

fn ordinal_fem_pl() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("[\u{0061}\u{00AA}\u{1D43}][\u{0073}\u{02E2}]$").unwrap())
}

/// `PortugueseTagger.PERCENT_PATTERN` (`−` is U+2212, `\d` is ASCII).
fn percent_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("^\u{2212}?[0-9]+[0-9,.]*%$").unwrap())
}

/// `PortugueseTagger.DEGREE_PATTERN`.
fn degree_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("^\u{2212}?[0-9]+[0-9,.]*\u{00B0}$").unwrap())
}

/// Plain `BaseTagger` over the loaded Morfologik `portuguese.dict`.
#[derive(Debug)]
pub struct PortugueseTagger {
    dict: Dictionary,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` second tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
}

impl PortugueseTagger {
    /// Load `pt/dictionaries/portuguese.{dict,info}` plus the manual word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("pt/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("portuguese.info"))?;
        let dict = Dictionary::load(&dict_dir.join("portuguese.dict"), &info)?;
        let words_dir = data_dir.join("pt/words");
        let manual = ManualTagger::load(&[
            &words_dir.join("added.txt"),
            &words_dir.join("added_custom.txt"),
        ])?;
        let removals = ManualTagger::load(&[
            &words_dir.join("removed.txt"),
            &words_dir.join("removed_custom.txt"),
        ])?;
        Ok(Self {
            dict,
            manual,
            removals,
        })
    }

    pub fn dict(&self) -> &Dictionary {
        &self.dict
    }

    /// `CombiningTagger.tag`: manual readings first, dictionary readings
    /// appended, removal tagger applied last (`overwriteWithManualTagger`
    /// is false).
    fn word_lookup(&self, word: &str) -> Vec<(String, String)> {
        let mut result: Vec<(String, String)> = self.manual.lookup(word).to_vec();
        result.extend(self.dict.lookup(word));
        if !self.removals.is_empty() {
            let removals = self.removals.lookup(word);
            if !removals.is_empty() {
                result.retain(|tw| !removals.contains(tw));
            }
        }
        result
    }

    fn add_from_word_tagger(&self, surface: &str, lookup: &str, out: &mut Vec<AnalyzedToken>) {
        for (stem, tag) in self.word_lookup(lookup) {
            out.push(AnalyzedToken::new(
                surface,
                Some(stem.clone()),
                Some(tag.clone()),
            ));
        }
    }

    /// `BaseTagger.tag(List<String>)` for one word (`BaseTagger.getAnalyzedTokens`).
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut l: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase();
        let is_lowercase = word == lower_word;
        let is_mixed = is_mixed_case(word);

        let before = l.len();
        self.add_from_word_tagger(word, word, &mut l);
        let tagger_tokens_empty = l.len() == before;
        if !is_lowercase && !is_mixed {
            self.add_from_word_tagger(word, &lower_word, &mut l);
        }
        // tag a lowercase word with startuppercase word tags
        if is_lowercase && tagger_tokens_empty && l.is_empty() {
            let upper = uppercase_first_char(word);
            self.add_from_word_tagger(word, &upper, &mut l);
        }
        if l.is_empty() {
            l.push(AnalyzedToken::new(word, None, None));
        }
        l
    }

    /// `PortugueseTagger.tagNumberExpressions` (ordinal/degree/percent).
    fn tag_number_expressions(&self, word: &str) -> Vec<AnalyzedToken> {
        if ordinal_pattern().is_match(word) {
            return self.build_ordinal_tokens(word);
        }
        if degree_pattern().is_match(word) || percent_pattern().is_match(word) {
            return vec![AnalyzedToken::new(
                word,
                Some(word.to_string()),
                Some("NCMP000".to_string()),
            )];
        }
        Vec::new()
    }

    /// `PortugueseTagger.buildOrdinalTokens`.
    fn build_ordinal_tokens(&self, word: &str) -> Vec<AnalyzedToken> {
        let lemma = ordinal_suffixes()
            .replace_all(word, "\u{00BA}")
            .into_owned();
        let mut number_gender_tags = "";
        if ordinal_masc_sg().find(word).is_some() {
            number_gender_tags = "MS";
        }
        if ordinal_fem_sg().find(word).is_some() {
            number_gender_tags = "FS";
        }
        if ordinal_masc_pl().find(word).is_some() {
            number_gender_tags = "MP";
        }
        if ordinal_fem_pl().find(word).is_some() {
            number_gender_tags = "FP";
        }
        vec![
            AnalyzedToken::new(
                word,
                Some(lemma.clone()),
                Some(format!("NC{number_gender_tags}000")),
            ),
            AnalyzedToken::new(word, Some(lemma), Some(format!("AO0{number_gender_tags}0"))),
        ]
    }

    /// `PortugueseTagger.tagMenteAdverbs`.
    fn tag_mente_adverbs(&self, word: &str, lower_word: &str) -> Vec<AnalyzedToken> {
        if !word.ends_with("mente") {
            return Vec::new();
        }
        let Ok(re) = Regex::new("^(.+)mente$") else {
            return Vec::new();
        };
        let possible_adj = re.replace(lower_word, "$1").into_owned();
        for (_, tag) in self.dict.lookup(&possible_adj) {
            if adj_part_fs().is_match(&tag) {
                return vec![AnalyzedToken::new(
                    word,
                    Some(lower_word.to_string()),
                    Some("RG".to_string()),
                )];
            }
        }
        Vec::new()
    }

    /// `PortugueseTagger.tagPrefixedVerbs` (`soto-` + verb).
    fn tag_prefixed_verbs(&self, word: &str) -> Vec<AnalyzedToken> {
        let Some(caps) = prefixes_for_verbs().captures(word) else {
            return Vec::new();
        };
        let prefix = caps
            .get(1)
            .map(|g| g.as_str().to_lowercase())
            .unwrap_or_default();
        let possible_verb = caps
            .get(2)
            .map(|g| g.as_str().to_lowercase())
            .unwrap_or_default();
        let mut out = Vec::new();
        for (stem, tag) in self.dict.lookup(&possible_verb) {
            if !verb_pattern().is_match(&tag) {
                continue;
            }
            let lemma = format!("{prefix}{stem}");
            if self.dict.lookup(&lemma).is_empty() {
                out.push(AnalyzedToken::new(word, Some(lemma), Some(tag.clone())));
            }
        }
        out
    }

    /// Tokenizer helper: `PortugueseTagger.tag([word]).get(0).isTagged()`
    /// (the full override, including the ordinal/`-mente`/`soto-` heuristics).
    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag(std::slice::from_ref(&word.to_string()))
            .first()
            .is_some_and(|atr| atr.is_tagged)
    }

    /// `PortugueseTagger.tag(List<String>)`: one entry per input token with
    /// the cumulative Java UTF-16 position (the pipeline overrides
    /// `start_pos` with byte offsets).
    pub fn tag(&self, sentence_tokens: &[String]) -> Vec<AnalyzedTokenReadings> {
        let mut out = Vec::with_capacity(sentence_tokens.len());
        let mut pos = 0usize;
        for raw in sentence_tokens {
            // This hack allows all rules and dictionary entries to work with
            // the typewriter apostrophe.
            let mut word = raw.clone();
            let mut contains_typewriter_apostrophe = false;
            if word.chars().count() > 1 {
                if word.contains('\'') {
                    contains_typewriter_apostrophe = true;
                }
                word = word.replace('\u{2019}', "'");
            }
            let lower_word = word.to_lowercase();
            let is_lowercase = word == lower_word;
            let is_mixed = if word.contains('-') {
                word.split('-').any(is_mixed_case)
            } else {
                is_mixed_case(&word)
            };

            let mut analyzed: Vec<AnalyzedToken> = Vec::new();
            self.add_from_word_tagger(&word, &word, &mut analyzed);
            if !is_lowercase && !is_mixed {
                self.add_from_word_tagger(&word, &lower_word, &mut analyzed);
            }
            if analyzed.is_empty() {
                analyzed.extend(self.tag_number_expressions(&word));
            }
            if analyzed.is_empty() && !is_mixed {
                analyzed.extend(self.tag_mente_adverbs(&word, &lower_word));
            }
            let mut ignore_spelling = false;
            if analyzed.is_empty() && !is_mixed {
                let prefixed = self.tag_prefixed_verbs(&word);
                if !prefixed.is_empty() {
                    ignore_spelling = true;
                    analyzed.extend(prefixed);
                }
            }
            if analyzed.is_empty() {
                analyzed.push(AnalyzedToken::new(word.clone(), None, None));
            }

            let mut atr = AnalyzedTokenReadings::new(analyzed);
            atr.start_pos = pos;
            atr.raw_byte_len = raw.len();
            if ignore_spelling {
                atr.is_ignore_spelling = true;
            }
            if contains_typewriter_apostrophe {
                atr.chunk_tags
                    .push("containsTypewriterApostrophe".to_string());
            }
            out.push(atr);
            pos += raw.encode_utf16().count();
        }
        out
    }
}
