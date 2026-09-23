//! `org.languagetool.synthesis.ManualSynthesizer`: manual lemma+tag forms
//! (`added.txt`, `removed.txt`, `do-not-synthesize.txt`), with the SUFFIX
//! `+`/`++` marker encoding.

use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Default)]
pub struct ManualSynthesizer {
    map: HashMap<(String, String), Vec<String>, rustc_hash::FxBuildHasher>,
    pub possible_tags: HashSet<String, rustc_hash::FxBuildHasher>,
}

impl ManualSynthesizer {
    pub fn load(path: &Path) -> Option<Self> {
        let text = lt_data::fs::read_to_string(path).ok()?;
        let mut map: HashMap<(String, String), Vec<String>, rustc_hash::FxBuildHasher> =
            HashMap::with_hasher(rustc_hash::FxBuildHasher);
        let mut possible_tags: HashSet<String, rustc_hash::FxBuildHasher> =
            HashSet::with_hasher(rustc_hash::FxBuildHasher);
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
            let mut parts: Vec<&str> = line.split(separator.as_str()).collect();
            // Java `String.split` drops trailing empty strings (French files
            // end every entry with the separator).
            while parts.last() == Some(&"") {
                parts.pop();
            }
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
    pub fn lookup(&self, lemma: &str, pos_tag: &str) -> Option<Vec<String>> {
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
