//! Port of `EnglishSynthesizer` / `BaseSynthesizer` (Morfologik synthesis):
//! inflected forms for a lemma and POS tag, used by `<match postag="...">`
//! suggestions and messages (`MatchState.toFinalString`).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use lt_core::{AnalyzedToken, Result};
use lt_pattern::Synthesizer;
use lt_tagger::{DictionaryInfo, SynthDictionary};

use crate::en::avs_an::AvsAnRule;

/// `EnglishSynthesizer.exceptions`.
const EXCEPTIONS: [&str; 7] = [
    "ne'er",
    "e'er",
    "o'er",
    "ol'",
    "ma'am",
    "n't",
    "informations",
];

/// `ManualSynthesizer`: manual lemma+tag forms (`added.txt`, `removed.txt`,
/// `do-not-synthesize.txt`), with the SUFFIX `+` marker encoding.
#[derive(Default)]
struct ManualSynthesizer {
    map: HashMap<(String, String), Vec<String>>,
    possible_tags: HashSet<String>,
}

impl ManualSynthesizer {
    fn load(path: &Path) -> Option<Self> {
        let text = lt_data::fs::read_to_string(path).ok()?;
        let mut map: HashMap<(String, String), Vec<String>> = HashMap::new();
        let mut possible_tags = HashSet::new();
        let mut separator = "\t".to_string();
        for raw in text.lines() {
            let line = raw.trim();
            if let Some(rest) = line.strip_prefix("#separatorRegExp=") {
                separator = rest.to_string();
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(separator.as_str()).collect();
            if parts.len() != 3 {
                continue;
            }
            let form = parts[0].to_string();
            let lemma = parts[1].to_string();
            let pos_tag = parts[2].to_string();
            possible_tags.insert(pos_tag.clone());
            map.entry((lemma, pos_tag)).or_default().push(form);
        }
        Some(Self { map, possible_tags })
    }

    /// `ManualSynthesizer.lookup`: decode `+`-suffix forms.
    fn lookup(&self, lemma: &str, pos_tag: &str) -> Option<Vec<String>> {
        let forms = self.map.get(&(lemma.to_string(), pos_tag.to_string()))?;
        Some(forms.iter().map(|form| decode_form(lemma, form)).collect())
    }
}

fn decode_form(lemma: &str, word: &str) -> String {
    if let Some(rest) = word.strip_prefix("++") {
        // Java `ManualSynthesizer.decodeForm`: `lemma[..len-1] + rest`
        let mut stem = lemma.to_string();
        stem.pop();
        return format!("{stem}{rest}");
    }
    if let Some(rest) = word.strip_prefix('+') {
        return format!("{lemma}{rest}");
    }
    word.to_string()
}

pub struct EnglishSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
    manual: Option<ManualSynthesizer>,
    removed: Option<ManualSynthesizer>,
    do_not_synthesize: Option<ManualSynthesizer>,
    a_vs_an: AvsAnRule,
    /// The language tagger for the `checksSpelling` check
    /// (`MatchState.toFinalString`); `None` in contexts without one.
    tagger: Option<std::sync::Arc<lt_tagger::EnglishTagger>>,
}

impl EnglishSynthesizer {
    pub fn from_data(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("en/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("english_synth.info"))?;
        let dict = SynthDictionary::load(&dict_dir.join("english_synth.dict"), &info)?;
        let tag_text = lt_data::fs::read_to_string(dict_dir.join("english_tags.txt"))
            .map_err(|e| lt_core::CoreError::Data(format!("cannot read english_tags.txt: {e}")))?;
        let mut possible_tags: Vec<String> = tag_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let words_dir = data_dir.join("en/words");
        let manual = ManualSynthesizer::load(&words_dir.join("added.txt"));
        let removed = ManualSynthesizer::load(&words_dir.join("removed.txt"));
        let do_not_synthesize = ManualSynthesizer::load(&words_dir.join("do-not-synthesize.txt"));
        if let Some(manual) = &manual {
            for tag in &manual.possible_tags {
                if !possible_tags.contains(tag) {
                    possible_tags.push(tag.clone());
                }
            }
        }
        Ok(Self {
            dict,
            possible_tags,
            manual,
            removed,
            do_not_synthesize,
            a_vs_an: AvsAnRule::from_data(data_dir)?,
            tagger: None,
        })
    }

    /// Attach the language tagger for the `checksSpelling` check.
    pub fn set_tagger(&mut self, tagger: std::sync::Arc<lt_tagger::EnglishTagger>) {
        self.tagger = Some(tagger);
    }

    /// `BaseSynthesizer.lookup`: dictionary forms plus manual forms minus the
    /// removal lists.
    fn lookup(&self, lemma: &str, pos_tag: &str) -> Vec<String> {
        let key = format!("{lemma}|{pos_tag}");
        let mut results = self.dict.lookup(&key);
        if let Some(manual) = &self.manual {
            if let Some(forms) = manual.lookup(lemma, pos_tag) {
                results.extend(forms);
            }
        }
        if let Some(removed) = &self.removed {
            if let Some(forms) = removed.lookup(lemma, pos_tag) {
                results.retain(|r| !forms.contains(r));
            }
        }
        if let Some(removed) = &self.do_not_synthesize {
            if let Some(forms) = removed.lookup(lemma, pos_tag) {
                results.retain(|r| !forms.contains(r));
            }
        }
        results
    }

    /// `EnglishSynthesizer.synthesize(AnalyzedToken, String, boolean)` incl.
    /// the `+DT`/`+INDT` article tags.
    fn synthesize_tag(&self, token: &AnalyzedToken, pos_tag: &str, regexp: bool) -> Vec<String> {
        if pos_tag.starts_with("_spell_number_") {
            // number spelling (Soros) is not ported
            return Vec::new();
        }
        if regexp {
            let mut my_pos_tag = pos_tag.to_string();
            let mut determiner = String::new();
            // `posTag.endsWith("+INDT")`, stripping the preceding backslash
            if let Some(pos) = pos_tag.rfind("+INDT") {
                let cut = if pos > 0 && pos_tag.as_bytes()[pos - 1] == b'\\' {
                    pos - 1
                } else {
                    pos
                };
                my_pos_tag = pos_tag[..cut].to_string();
                let lemma = token.stem.as_deref().unwrap_or(&token.token);
                let article = self.a_vs_an.suggest_a_or_an(lemma);
                if let Some(idx) = article.find(' ') {
                    determiner = article[..=idx].to_string();
                }
            } else if let Some(pos) = pos_tag.rfind("+DT") {
                let cut = if pos > 0 && pos_tag.as_bytes()[pos - 1] == b'\\' {
                    pos - 1
                } else {
                    pos
                };
                my_pos_tag = pos_tag[..cut].to_string();
                determiner = "the ".to_string();
            }
            let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{my_pos_tag})$")) else {
                return Vec::new();
            };
            let mut results = Vec::new();
            for tag in &self.possible_tags {
                if re.is_match(tag).unwrap_or(false) {
                    if let Some(lemma) = token.stem.as_deref() {
                        for form in self.lookup(lemma, tag) {
                            results.push(format!("{determiner}{form}"));
                        }
                    }
                }
            }
            return remove_exceptions(results);
        }
        // non-regexp `EnglishSynthesizer.synthesize` article tags
        if pos_tag == "+DT" {
            let article = self.a_vs_an.suggest_a_or_an(&token.token);
            return vec![
                article,
                format!(
                    "the {}",
                    crate::en::avs_an::lowercase_first_char_if_capitalized(&token.token)
                ),
            ];
        }
        if pos_tag == "+INDT" {
            return vec![self.a_vs_an.suggest_a_or_an(&token.token)];
        }
        let lemma = token.stem.clone().unwrap_or_else(|| token.token.clone());
        remove_exceptions(self.lookup(&lemma, pos_tag))
    }

    /// `AvsAnRule.suggestAorAn` for the `+DT`/`+INDT` synthesis tags.
    pub fn suggest_a_or_an(&self, word: &str) -> String {
        self.a_vs_an.suggest_a_or_an(word)
    }
}

/// `BaseSynthesizer.removeExceptions` + `EnglishSynthesizer.isException`.
fn remove_exceptions(forms: Vec<String>) -> Vec<String> {
    forms
        .into_iter()
        .filter(|f| !f.starts_with('\'') && !EXCEPTIONS.contains(&f.as_str()))
        .collect()
}

impl Synthesizer for EnglishSynthesizer {
    fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        self.synthesize_tag(token, pos_tag, pos_tag_regexp)
    }

    fn is_known_word(&self, word: &str) -> bool {
        // `MatchState.toFinalString`: `lemma == null && hasNoTag()`
        self.tagger.as_ref().is_none_or(|tagger| {
            !tagger
                .tag_word(word)
                .into_iter()
                .next()
                .is_some_and(|r| r.stem.is_none() && r.pos_tag.is_none())
        })
    }
}
