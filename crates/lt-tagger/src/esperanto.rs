//! Esperanto tagger: port of `org.languagetool.tagging.eo.EsperantoTagger`.
//! A `ManualTagger` over the closed-word list (`eo/words/manual-tagger.txt`)
//! plus rule-based tagging of the open word classes (nouns, adjectives,
//! adverbs, verbs, `tabelvortoj` and participles) with the
//! transitive/intransitive verb lists (`eo/rules/verb-{tr,ntr}.txt`) and the
//! non-participle root list (`eo/rules/root-ant-at.txt`).

use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};
use regex::Regex;

use crate::ManualTagger;

fn pattern_verb() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(..+)(as|os|is|us|u|i)$").unwrap())
}

fn pattern_prefix() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(?:mal|mis|ek|re|fi|ne)(.*)").unwrap())
}

fn pattern_suffix() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(.*)(?:ad|aĉ|eg|et)i$").unwrap())
}

fn pattern_participle() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"((..+)([aio])(n?)t)([aoe])(j?)(n?)$").unwrap())
}

fn pattern_tabelvorto() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^(i|ti|ki|ĉi|neni)(?:(?:([uoae])(j?)(n?))|(am|al|es|el|om))$").unwrap()
    })
}

fn pattern_tabelvorto_adverb() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(?:ti|i|ĉi|neni)(?:am|om|el|e)$").unwrap())
}

/// `EsperantoTagger.xSystemToUnicode`: `jxauxdo` → `ĵaŭdo`.
fn x_system_to_unicode(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c1 = chars[i];
        let c2 = if i + 1 < chars.len() {
            chars[i + 1]
        } else {
            ' '
        };
        if c2 == 'x' {
            match c1 {
                'c' => {
                    result.push('ĉ');
                    i += 2;
                    continue;
                }
                'g' => {
                    result.push('ĝ');
                    i += 2;
                    continue;
                }
                'h' => {
                    result.push('ĥ');
                    i += 2;
                    continue;
                }
                'j' => {
                    result.push('ĵ');
                    i += 2;
                    continue;
                }
                's' => {
                    result.push('ŝ');
                    i += 2;
                    continue;
                }
                'u' => {
                    result.push('ŭ');
                    i += 2;
                    continue;
                }
                _ => {
                    result.push(c1);
                }
            }
        } else {
            result.push(c1);
        }
        i += 1;
    }
    result
}

fn load_word_set(path: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return set;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        set.insert(line.to_string());
    }
    set
}

#[derive(Debug)]
pub struct EsperantoTagger {
    manual: ManualTagger,
    transitive: HashSet<String>,
    intransitive: HashSet<String>,
    non_participle: HashSet<String>,
}

impl EsperantoTagger {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let words = data_dir.join("eo/words");
        let rules = data_dir.join("eo/rules");
        let manual = ManualTagger::load(&[&words.join("manual-tagger.txt")])?;
        let transitive = load_word_set(&rules.join("verb-tr.txt"));
        let intransitive = load_word_set(&rules.join("verb-ntr.txt"));
        let non_participle = load_word_set(&rules.join("root-ant-at.txt"));
        Ok(Self {
            manual,
            transitive,
            intransitive,
            non_participle,
        })
    }

    /// `findTransitivity`.
    fn find_transitivity(&self, verb: &str) -> &'static str {
        if verb.ends_with("iĝi") {
            return "nt";
        } else if verb.ends_with("igi") {
            return if verb == "memmortigi" { "nt" } else { "tr" };
        }
        let mut verb = verb.to_string();
        loop {
            let is_transitive = self.transitive.contains(&verb);
            let is_intransitive = self.intransitive.contains(&verb);
            if is_transitive {
                return if is_intransitive { "tn" } else { "tr" };
            } else if is_intransitive {
                return "nt";
            }
            if let Some(m) = pattern_prefix().captures(&verb) {
                verb = m.get(1).map(|g| g.as_str()).unwrap_or("").to_string();
                continue;
            }
            if let Some(m) = pattern_suffix().captures(&verb) {
                verb = format!("{}i", m.get(1).map(|g| g.as_str()).unwrap_or(""));
                continue;
            }
            break;
        }
        "xx"
    }

    /// `EsperantoTagger.tag` for one token.
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut l: Vec<AnalyzedToken> = Vec::new();
        if word.chars().count() > 50 {
            l.push(AnalyzedToken::new(word, None, None));
            return l;
        }
        if word.chars().count() <= 1 {
            l.push(AnalyzedToken::new(word, None, None));
            return l;
        }
        let l_word = x_system_to_unicode(&word.to_lowercase());
        let manual_tags = self.manual.lookup(&l_word);
        if !manual_tags.is_empty() {
            for (lemma, tag) in manual_tags {
                l.push(AnalyzedToken::new(
                    word,
                    Some(lemma.clone()),
                    Some(tag.clone()),
                ));
            }
            return l;
        }

        // Open word: look at the ending.
        if let Some(m) = pattern_tabelvorto().captures(&l_word) {
            let type1_group = m
                .get(1)
                .map(|g| {
                    g.as_str()
                        .chars()
                        .next()
                        .unwrap_or(' ')
                        .to_lowercase()
                        .to_string()
                })
                .unwrap_or_default();
            let type2_group = m.get(2).map(|g| g.as_str().to_string());
            let pl_group = m.get(3).map(|g| g.as_str().to_string());
            let acc_group = m.get(4).map(|g| g.as_str().to_string());
            let type3_group = m.get(5).map(|g| g.as_str().to_string());
            let accusative = match acc_group.as_deref() {
                None => "xxx",
                Some(a) if a.eq_ignore_ascii_case("n") => "akz",
                _ => "nak",
            };
            let plural = match pl_group.as_deref() {
                None => " pn ",
                Some(p) if p.eq_ignore_ascii_case("j") => " pl ",
                _ => " np ",
            };
            let type_str = type2_group
                .or(type3_group)
                .unwrap_or_default()
                .to_lowercase();
            l.push(AnalyzedToken::new(
                word,
                None,
                Some(format!("T {accusative}{plural}{type1_group} {type_str}")),
            ));
            if pattern_tabelvorto_adverb().is_match(&l_word) {
                l.push(AnalyzedToken::new(
                    word,
                    Some(l_word.clone()),
                    Some("E nak".to_string()),
                ));
            }
        } else if l_word.ends_with('o') {
            l.push(AnalyzedToken::new(
                word,
                Some(l_word.clone()),
                Some("O nak np".to_string()),
            ));
        } else if l_word.chars().count() >= 2 && l_word.ends_with('\'') {
            let mut lemma = l_word.clone();
            lemma.pop();
            lemma.push('o');
            l.push(AnalyzedToken::new(
                word,
                Some(lemma),
                Some("O nak np".to_string()),
            ));
        } else if l_word.ends_with("oj") {
            let lemma = l_word[..l_word.len() - 1].to_string();
            l.push(AnalyzedToken::new(
                word,
                Some(lemma),
                Some("O nak pl".to_string()),
            ));
        } else if l_word.ends_with("on") {
            let lemma = l_word[..l_word.len() - 1].to_string();
            l.push(AnalyzedToken::new(
                word,
                Some(lemma),
                Some("O akz np".to_string()),
            ));
        } else if l_word.ends_with("ojn") {
            let lemma = l_word[..l_word.len() - 2].to_string();
            l.push(AnalyzedToken::new(
                word,
                Some(lemma),
                Some("O akz pl".to_string()),
            ));
        } else if l_word.ends_with('a') {
            l.push(AnalyzedToken::new(
                word,
                Some(l_word.clone()),
                Some("A nak np".to_string()),
            ));
        } else if l_word.ends_with("aj") {
            let lemma = l_word[..l_word.len() - 1].to_string();
            l.push(AnalyzedToken::new(
                word,
                Some(lemma),
                Some("A nak pl".to_string()),
            ));
        } else if l_word.ends_with("an") {
            let lemma = l_word[..l_word.len() - 1].to_string();
            l.push(AnalyzedToken::new(
                word,
                Some(lemma),
                Some("A akz np".to_string()),
            ));
        } else if l_word.ends_with("ajn") {
            let lemma = l_word[..l_word.len() - 2].to_string();
            l.push(AnalyzedToken::new(
                word,
                Some(lemma),
                Some("A akz pl".to_string()),
            ));
        } else if l_word.ends_with('e') {
            l.push(AnalyzedToken::new(
                word,
                Some(l_word.clone()),
                Some("E nak".to_string()),
            ));
        } else if l_word.ends_with("en") {
            let lemma = l_word[..l_word.len() - 1].to_string();
            l.push(AnalyzedToken::new(
                word,
                Some(lemma),
                Some("E akz".to_string()),
            ));
        } else if let Some(m) = pattern_verb().captures(&l_word) {
            let verb = format!("{}i", m.get(1).map(|g| g.as_str()).unwrap_or(""));
            let tense = m.get(2).map(|g| g.as_str()).unwrap_or("");
            let transitive = self.find_transitivity(&verb);
            l.push(AnalyzedToken::new(
                word,
                Some(verb),
                Some(format!("V {transitive} {tense}")),
            ));
        } else {
            l.push(AnalyzedToken::new(word, None, None));
        }

        // Participle (can be combined with other tags).
        if let Some(m) = pattern_participle().captures(&l_word) {
            let group1 = m.get(1).map(|g| g.as_str()).unwrap_or("");
            if !self.non_participle.contains(group1) {
                let verb = format!("{}i", m.get(2).map(|g| g.as_str()).unwrap_or(""));
                let aio = m.get(3).map(|g| g.as_str()).unwrap_or("");
                let ant_at = if m.get(4).map(|g| g.as_str()) == Some("n") {
                    "n"
                } else {
                    "-"
                };
                let aoe = m.get(5).map(|g| g.as_str()).unwrap_or("");
                let plural = if m.get(6).map(|g| g.as_str()) == Some("j") {
                    "pl"
                } else {
                    "np"
                };
                let accusative = if m.get(7).map(|g| g.as_str()) == Some("n") {
                    "akz"
                } else {
                    "nak"
                };
                let transitive = self.find_transitivity(&verb);
                l.push(AnalyzedToken::new(
                    word,
                    Some(verb),
                    Some(format!(
                        "C {accusative} {plural} {transitive} {aio} {ant_at} {aoe}"
                    )),
                ));
            }
        }
        l
    }

    /// `EsperantoTagger.tag(List<String>)`: one entry per token; Java passes
    /// start position 0 for every reading (the pipeline overrides the byte
    /// offsets).
    pub fn tag(&self, sentence_tokens: &[String]) -> Vec<AnalyzedTokenReadings> {
        sentence_tokens
            .iter()
            .map(|raw| {
                let mut atr = AnalyzedTokenReadings::new(self.tag_word(raw));
                atr.raw_byte_len = raw.len();
                atr
            })
            .collect()
    }
}
