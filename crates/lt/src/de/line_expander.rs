//! Port of `org.languagetool.rules.de.LineExpander`: expands the lines of the
//! German speller word lists (`spelling.txt`, `ignore.txt`, …) into the word
//! forms that are added to the rule's ignore set.
//!
//! Expansion rules (Java):
//! - `#…` tags are cut off, `\` escapes removed, the line trimmed;
//! - `prefix_verb` lines expand to the synthesizer's `VER:` forms of the
//!   verb prefixed with `prefix`, plus `prefixzuverb` and the genitive
//!   `Prefixverbs` form; lowercase `<verb>` forms are added, forms with `ß`
//!   are skipped;
//! - `X_in` expands to the common gender-gap spellings (`X_in`, `X*in`, …);
//! - `/S`, `/N`, `/E`, `/F`, `/T`, `/A`, `/P` flags expand to inflected
//!   forms (`/T` also produces the `Str.` abbreviation for `-straße`).
//!
//! The Java implementation uses a `HashSet` before adding the synthesizer
//! forms; the result is a set of words, so the iteration order does not
//! matter for the caller.

use std::sync::Arc;

/// Port of `org.languagetool.rules.de.LineExpander`.
pub struct LineExpander {
    synth: Arc<lt_tagger::GermanSynthesizer>,
    /// PROTOTYPE: the same verb lemma appears under many prefixes in
    /// `spelling.txt` (5,439 prefix lines, 1,460 unique lemmas).
    forms_cache: std::cell::RefCell<std::collections::HashMap<String, Vec<String>>>,
}

impl LineExpander {
    pub fn new(synth: Arc<lt_tagger::GermanSynthesizer>) -> Self {
        Self {
            synth,
            forms_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    fn verb_forms(&self, lemma: &str) -> Vec<String> {
        if let Some(cached) = self.forms_cache.borrow().get(lemma) {
            return cached.clone();
        }
        let forms = self
            .synth
            .synthesize_for_pos_tags(lemma, &|tag: &str| tag.starts_with("VER:"));
        self.forms_cache
            .borrow_mut()
            .insert(lemma.to_string(), forms.clone());
        forms
    }

    /// `LineExpander.expandLine`.
    pub fn expand_line(&self, line: &str) -> Vec<String> {
        if is_line_with_verb_prefix(line) {
            self.handle_line_with_prefix(line)
        } else if is_line_with_flag(line) {
            handle_line_with_flags(line)
        } else {
            vec![clean_tags_and_escape_chars(line)]
        }
    }

    /// `handleLineWithPrefix`.
    fn handle_line_with_prefix(&self, line: &str) -> Vec<String> {
        let cleaned = clean_tags_and_escape_chars(line);
        let parts: Vec<&str> = cleaned.split('_').collect();
        if parts.len() != 2 {
            panic!("Unexpected line format, expected at most one '_': {line}");
        }
        let mut result: Vec<String> = Vec::new();
        if parts[1] == "in" {
            for infix in ["_in", "_innen", "*in", "*innen", ":in", ":innen"] {
                result.push(format!("{}{infix}", parts[0]));
            }
            return result;
        }
        let mut forms = self.verb_forms(parts[1]);
        // Java iterates a HashSet here; deduplicate but keep a stable order.
        let mut seen = std::collections::HashSet::new();
        forms.retain(|f| seen.insert(f.clone()));
        for form in forms {
            if !form.contains('ß')
                && !form.is_empty()
                && form.chars().next().is_some_and(char::is_lowercase)
            {
                result.push(format!("{}{form}", parts[0]));
            }
        }
        result.push(format!("{}zu{}", parts[0], parts[1]));
        result.push(format!(
            "{}{}s",
            lt_tagger::uppercase_first_char(parts[0]),
            parts[1]
        ));
        result
    }
}

/// `isLineWithVerbPrefix`.
fn is_line_with_verb_prefix(line: &str) -> bool {
    let Some(idx) = line.find('_') else {
        return false;
    };
    !line.starts_with('#') && idx > 0 && line.as_bytes()[idx - 1] != b'\\'
}

/// `isLineWithFlag`.
fn is_line_with_flag(line: &str) -> bool {
    let Some(idx) = line.find('/') else {
        return false;
    };
    !line.starts_with('#') && idx > 0 && line.as_bytes()[idx - 1] != b'\\'
}

/// `handleLineWithFlags`.
fn handle_line_with_flags(line: &str) -> Vec<String> {
    let cleaned = clean_tags_and_escape_chars(line);
    let parts: Vec<&str> = cleaned.split('/').collect();
    if parts.len() != 2 {
        panic!("Unexpected line format, expected at most one slash: {line}");
    }
    let word = parts[0];
    let suffix = parts[1];
    let mut result: Vec<String> = Vec::new();
    let add = |result: &mut Vec<String>, w: String| {
        if !result.contains(&w) {
            result.push(w);
        }
    };
    for c in suffix.chars() {
        match c {
            'S' => {
                add(&mut result, word.to_string());
                add(&mut result, format!("{word}s"));
            }
            'N' => {
                add(&mut result, word.to_string());
                add(&mut result, format!("{word}n"));
            }
            'E' => {
                add(&mut result, word.to_string());
                add(&mut result, format!("{word}e"));
            }
            'F' => {
                add(&mut result, word.to_string());
                add(&mut result, format!("{word}in"));
            }
            'T' => {
                add(&mut result, word.to_string());
                if word.ends_with("straße") || word.ends_with("strasse") {
                    add(&mut result, replace_street_suffix(word, false));
                }
                if word.ends_with("Straße") || word.ends_with("Strasse") {
                    add(&mut result, replace_street_suffix(word, true));
                }
            }
            'A' | 'P' => {
                add(&mut result, word.to_string());
                if word.ends_with('e') {
                    for suffix in ["r", "s", "n", "m"] {
                        add(&mut result, format!("{word}{suffix}"));
                    }
                } else {
                    for suffix in ["e", "er", "es", "en", "em"] {
                        add(&mut result, format!("{word}{suffix}"));
                    }
                }
            }
            other => panic!("Unknown suffix: {other} in line: {line}"),
        }
    }
    result
}

/// Java `word.replaceAll("stra(ß|ss)e", "str.")` / `"Stra(ß|ss)e"` → `"Str."`.
fn replace_street_suffix(word: &str, upper: bool) -> String {
    if upper {
        word.replace("Straße", "Str.").replace("Strasse", "Str.")
    } else {
        word.replace("straße", "str.").replace("strasse", "str.")
    }
}

/// `cleanTagsAndEscapeChars`: cut at `#`, remove `\`, trim.
fn clean_tags_and_escape_chars(s: &str) -> String {
    let s = match s.find('#') {
        Some(idx) => &s[..idx],
        None => s,
    };
    s.replace('\\', "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_tags_and_escapes() {
        assert_eq!(clean_tags_and_escape_chars("ok #abk #ugs"), "ok");
        assert_eq!(clean_tags_and_escape_chars("  foo\\/bar  "), "foo/bar");
    }

    #[test]
    fn expands_flags() {
        assert_eq!(
            handle_line_with_flags("Abendessen/S"),
            vec!["Abendessen", "Abendessens"]
        );
        assert_eq!(handle_line_with_flags("Haus/NE").len(), 3);
        assert_eq!(
            handle_line_with_flags("Hauptstraße/T"),
            vec!["Hauptstraße", "Hauptstr."]
        );
    }
}
