//! MultiWordChunker port (`org.languagetool.tagging.disambiguation.MultiWordChunker`):
//! multiword POS tags via `<TAG>`/`</TAG>` chunk-marker readings, optional
//! ignore-spelling flags, and the `removePreviousTags` hybrid-disambiguator
//! transformation.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

/// `MultiWordChunker.GermanLineExpander` (`^.*/[ESN]+$`).
static GERMAN_LINE_EXPANDER: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^.*/[ESN]+$").unwrap());

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, Result};

pub const TAG_FOR_NOT_ADDING_TAGS: &str = "_NONE_";
const MAX_TOKENS_IN_MULTIWORD: usize = 20;

#[derive(Debug, Clone)]
struct ChunkEntry {
    tag: String,
    original: String,
}

pub struct MultiWordChunker {
    /// first token (with spaces in the entry) → max token count
    space_start: HashMap<String, usize>,
    space_full: HashMap<String, ChunkEntry>,
    /// first character (no-space entries) → max concatenated length
    nospace_start: HashMap<String, usize>,
    nospace_full: HashMap<String, ChunkEntry>,
    add_ignore_spelling: bool,
    remove_previous_tags: bool,
    #[allow(dead_code)]
    default_tag: Option<String>,
}

impl MultiWordChunker {
    pub fn load(
        path: &Path,
        add_ignore_spelling: bool,
        allow_first_capitalized: bool,
        allow_all_uppercase: bool,
        default_tag: Option<String>,
        remove_previous_tags: bool,
    ) -> Result<Self> {
        let text = lt_data::fs::read_to_string(path).map_err(|e| {
            lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display()))
        })?;
        let mut space_start: HashMap<String, usize> = HashMap::new();
        let mut space_full: HashMap<String, ChunkEntry> = HashMap::new();
        let mut nospace_start: HashMap<String, usize> = HashMap::new();
        let mut nospace_full: HashMap<String, ChunkEntry> = HashMap::new();
        // Java `MultiWordChunker.loadWords`: a `#separatorRegExp=...` line
        // changes the separator regexp (Spanish/French use `[\t;]`); the
        // default is a tab.
        let mut separator: Option<regex::Regex> = None;

        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("#separatorRegExp=") {
                separator = regex::Regex::new(rest).ok();
                continue;
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.split('#').next().unwrap_or("").trim().to_string();
            if line.is_empty() {
                continue;
            }
            // Java `loadWords`: German `form/ESN` lines expand to the plain
            // form plus the E/S/N suffixed forms.
            let mut expanded: Vec<String> = Vec::new();
            if GERMAN_LINE_EXPANDER.is_match(&line) {
                let parts: Vec<&str> = line.split('/').collect();
                let base = parts[0].trim();
                expanded.push(base.to_string());
                if parts[1].contains('E') {
                    expanded.push(format!("{base}e"));
                }
                if parts[1].contains('S') {
                    expanded.push(format!("{base}s"));
                }
                if parts[1].contains('N') {
                    expanded.push(format!("{base}n"));
                }
            } else {
                expanded.push(line);
            }
            for line in expanded {
                // Java `fillMaps`: with a default tag (`tagForNotAddingTags`)
                // lines carry no separator; otherwise exactly two separator-
                // split fields are required.
                let parts: Vec<&str> = match &separator {
                    Some(re) => re.split(&line).collect(),
                    None => line.split('\t').collect(),
                };
                let (original, tag) = match &default_tag {
                    Some(default) => {
                        if parts.len() != 1 {
                            continue;
                        }
                        (parts[0].trim().to_string(), default.clone())
                    }
                    None => {
                        if parts.len() != 2 {
                            continue;
                        }
                        (parts[0].trim().to_string(), parts[1].trim().to_string())
                    }
                };
                let contains_space = original.contains(' ');
                let mut variants = vec![original.clone()];
                let existing = |variant: &str| -> bool {
                    if contains_space {
                        space_full.contains_key(variant)
                    } else {
                        nospace_full.contains_key(variant)
                    }
                };
                // Java `getTokenLettercaseVariants`: a variant is only added
                // when the map does not already contain it (so an earlier
                // tag wins for the *derived* casing variants; the original
                // spelling is overwritten by later lines).
                if allow_all_uppercase && !is_camel_case(&original) {
                    let upper = original.to_uppercase();
                    if upper != original && !existing(&upper) {
                        variants.push(upper);
                    }
                }
                if allow_first_capitalized {
                    let first_cap = uppercase_first_char(&original);
                    if first_cap != original && !existing(&first_cap) {
                        variants.push(first_cap);
                    }
                }
                for variant in variants {
                    if contains_space {
                        let count = variant.split(' ').count();
                        let first = variant.split(' ').next().unwrap_or("").to_string();
                        let slot = space_start.entry(first).or_insert(0);
                        if *slot < count {
                            *slot = count;
                        }
                        space_full.insert(
                            variant.clone(),
                            ChunkEntry {
                                tag: tag.clone(),
                                original: original.to_string(),
                            },
                        );
                    } else {
                        let first_char = variant.chars().next().unwrap_or('\0').to_string();
                        let len = variant.chars().count();
                        let slot = nospace_start.entry(first_char).or_insert(0);
                        if *slot < len {
                            *slot = len;
                        }
                        nospace_full.insert(
                            variant.clone(),
                            ChunkEntry {
                                tag: tag.clone(),
                                original: original.to_string(),
                            },
                        );
                    }
                }
            }
        }

        Ok(Self {
            space_start,
            space_full,
            nospace_start,
            nospace_full,
            add_ignore_spelling,
            remove_previous_tags,
            default_tag,
        })
    }

    /// A chunker with no entries (used when the data file is missing).
    pub fn load_empty(add_ignore_spelling: bool, remove_previous_tags: bool) -> Self {
        Self {
            space_start: HashMap::new(),
            space_full: HashMap::new(),
            nospace_start: HashMap::new(),
            nospace_full: HashMap::new(),
            add_ignore_spelling,
            remove_previous_tags,
            default_tag: None,
        }
    }

    pub fn apply(&self, sentence: &mut AnalyzedSentence) {
        let tokens: Vec<String> = sentence
            .tokens
            .iter()
            .map(|t| t.surface().to_string())
            .collect();
        let is_ws: Vec<bool> = sentence.tokens.iter().map(|t| t.is_whitespace).collect();

        for i in 0..tokens.len() {
            if tokens[i].is_empty() {
                continue;
            }
            // space-keyed matching: Java builds `tok` from the current token
            // plus every directly following non-whitespace token (`n'` +
            // `importe` = `n'importe`, the first space-delimited word of
            // `n'importe quoi`), then looks that up in `mStartSpace`.
            let mut tok = tokens[i].clone();
            let mut k = i + 1;
            while k < tokens.len() && !is_ws[k] {
                tok.push_str(&tokens[k]);
                k += 1;
            }
            if let Some(&max_len) = self.space_start.get(&tok) {
                let mut key = String::new();
                let mut len_counter = 0usize;
                let mut final_len = i;
                let mut j = i;
                while j < tokens.len() && j - i < MAX_TOKENS_IN_MULTIWORD {
                    if !is_ws[j] {
                        key.push_str(&tokens[j]);
                        if let Some(entry) = self.space_full.get(&key) {
                            if entry.tag != TAG_FOR_NOT_ADDING_TAGS {
                                if final_len == i {
                                    // Java `setAndAnnotate`: plain tag, no
                                    // `<TAG>`/`</TAG>` markers
                                    self.set_annotate(sentence, j, &entry.tag, &entry.original);
                                } else {
                                    self.add_reading(
                                        sentence,
                                        i,
                                        &entry.tag,
                                        &entry.original,
                                        false,
                                    );
                                    self.add_reading(
                                        sentence,
                                        final_len,
                                        &entry.tag,
                                        &entry.original,
                                        true,
                                    );
                                }
                            }
                            if self.add_ignore_spelling {
                                for m in i..=final_len {
                                    sentence.tokens[m].is_ignore_spelling = true;
                                }
                            }
                        }
                    } else if j > 1 && !is_ws[j - 1] {
                        key.push(' ');
                        len_counter += 1;
                        if len_counter == max_len {
                            break;
                        }
                    }
                    j += 1;
                    final_len = j;
                }
            }
            // no-space matching: concatenate adjacent non-whitespace tokens
            let first_char: String = tokens[i].chars().take(1).collect();
            if self.nospace_start.contains_key(&first_char) {
                let max_len = self.nospace_start[&first_char];
                let mut key = String::new();
                let mut j = i;
                while j < tokens.len()
                    && !is_ws[j]
                    && j - i < MAX_TOKENS_IN_MULTIWORD
                    && key.chars().count() < max_len
                {
                    key.push_str(&tokens[j]);
                    if let Some(entry) = self.nospace_full.get(&key) {
                        if entry.tag != TAG_FOR_NOT_ADDING_TAGS {
                            if i == j {
                                // Java `isLowPriorityTag`: the NPCN000 chunk
                                // tag is not added to a token that already
                                // has a real reading (`hasReading() &&
                                // !isPosTagUnknown()`)
                                let tr = &sentence.tokens[i];
                                if entry.tag != "NPCN000"
                                    || tr.readings.is_empty()
                                    || tr.is_pos_tag_unknown
                                {
                                    self.set_annotate(sentence, i, &entry.tag, &entry.original);
                                }
                            } else {
                                self.add_reading(sentence, i, &entry.tag, &entry.original, false);
                                self.add_reading(sentence, j, &entry.tag, &entry.original, true);
                            }
                        }
                        if self.add_ignore_spelling {
                            for m in i..=j {
                                sentence.tokens[m].is_ignore_spelling = true;
                            }
                        }
                    }
                    j += 1;
                }
            }
        }

        if self.remove_previous_tags {
            remove_previous_tags(sentence);
        }
    }

    /// Java `setAndAnnotate`: plain reading with the chunk tag.
    fn set_annotate(&self, sentence: &mut AnalyzedSentence, idx: usize, tag: &str, original: &str) {
        let surface = sentence.tokens[idx].surface().to_string();
        let reading =
            AnalyzedToken::new(surface, Some(original.to_string()), Some(tag.to_string()));
        let tr = &mut sentence.tokens[idx];
        if !tr
            .readings
            .iter()
            .any(|r| r.token == reading.token && r.pos_tag == reading.pos_tag)
        {
            // Java `AnalyzedTokenReadings.addReading` drops the trailing
            // untagged fallback reading before appending.
            tr.add_reading(reading);
        }
    }

    fn add_reading(
        &self,
        sentence: &mut AnalyzedSentence,
        idx: usize,
        tag: &str,
        original: &str,
        is_last: bool,
    ) {
        let surface = sentence.tokens[idx].surface().to_string();
        let postag = if is_last {
            format!("</{}>", tag)
        } else {
            format!("<{}>", tag)
        };
        let reading = AnalyzedToken::new(surface, Some(original.to_string()), Some(postag));
        let tr = &mut sentence.tokens[idx];
        if !tr
            .readings
            .iter()
            .any(|r| r.token == reading.token && r.pos_tag == reading.pos_tag)
        {
            tr.add_reading(reading);
        }
    }
}

fn is_camel_case(s: &str) -> bool {
    // Java `StringTools.isCamelCase`: `token.matches("[a-z]+[A-Z][A-Za-z]+")`
    // — ASCII-only and anchored, so e.g. `al-Àndalus` and `n'importe` are not
    // camel case (a non-ASCII or punctuation char breaks the match) and do
    // get their all-uppercase variants.
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_lowercase() {
        i += 1;
    }
    if i == 0 || i >= bytes.len() || !bytes[i].is_ascii_uppercase() {
        return false;
    }
    i += 1;
    if i >= bytes.len() {
        return false;
    }
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    i == bytes.len()
}

fn uppercase_first_char(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Port of `MultiWordChunker.removePreviousTags`: converts `<TAG>` … `</TAG>`
/// chunk-marker readings into plain tag readings (the individual original
/// tags of multiword parts are replaced by the chunk-level tag).
fn remove_previous_tags(sentence: &mut AnalyzedSentence) {
    let mut open_tag = String::new();
    let mut lemma = String::new();
    let mut next_postag = String::new();
    for i in 0..sentence.tokens.len() {
        if sentence.tokens[i].is_whitespace {
            continue;
        }
        if !next_postag.is_empty() {
            let token = sentence.tokens[i].surface().to_string();
            let has_close = sentence.tokens[i].readings.iter().any(|r| {
                r.pos_tag.as_deref() == Some(format!("</{}>", open_tag).as_str())
                    && r.stem.as_deref() == Some(lemma.as_str())
            });
            let reading = AnalyzedToken::new(token, Some(lemma.clone()), Some(next_postag.clone()));
            sentence.tokens[i].readings = vec![reading];
            sentence.tokens[i].refresh_is_tagged();
            if has_close {
                next_postag.clear();
                lemma.clear();
            }
        } else if let Some((tag, orig)) = get_multiword_marker(&sentence.tokens, i) {
            open_tag = tag.clone();
            lemma = orig.clone();
            let close = format!("</{}>", tag);
            let has_close = sentence.tokens[i]
                .readings
                .iter()
                .any(|r| r.pos_tag.as_deref() == Some(close.as_str()));
            if has_close {
                let token = sentence.tokens[i].surface().to_string();
                sentence.tokens[i].readings.retain(|r| {
                    r.pos_tag.as_deref() != Some(close.as_str())
                        && r.pos_tag.as_deref() != Some(format!("<{}>", tag).as_str())
                });
                sentence.tokens[i].readings.push(AnalyzedToken::new(
                    token,
                    Some(orig.clone()),
                    Some(tag.clone()),
                ));
                sentence.tokens[i].refresh_is_tagged();
                next_postag.clear();
                lemma.clear();
            } else {
                let token = sentence.tokens[i].surface().to_string();
                sentence.tokens[i].readings = vec![AnalyzedToken::new(
                    token,
                    Some(orig.clone()),
                    Some(tag.clone()),
                )];
                sentence.tokens[i].refresh_is_tagged();
                next_postag = get_next_pos_tag(&tag); // Romance/English rule
            }
        }
    }
}

/// Java `MultiWordChunker.getNextPosTag`: the continuation token of a
/// Spanish/Portuguese/Catalan noun chunk gets `AQ0<gen><num>0`, French
/// gets `J `, everything else keeps the tag.
fn get_next_pos_tag(postag: &str) -> String {
    if let Some(rest) = postag.strip_prefix("NC") {
        format!("AQ0{}0", &rest[..2])
    } else if let Some(rest) = postag.strip_prefix("N ") {
        format!("J {rest}")
    } else {
        postag.to_string()
    }
}

/// Java `MultiWordChunker.getMultiWordAnalyzedToken`: among several stacked
/// open `<TAG>` markers (e.g. spelling_global's `NPCN000` plus a multiwords
/// entry), pick the one whose closing tag is furthest away; a later marker
/// wins ties (Java's `isLowPriorityTag` compares a truncated tag and never
/// matches, so the tie-break is effectively "last wins").
fn get_multiword_marker(tokens: &[AnalyzedTokenReadings], i: usize) -> Option<(String, String)> {
    let mut selected: Option<(String, String)> = None;
    let mut max_distance = 0usize;
    for r in &tokens[i].readings {
        let Some(tag) = &r.pos_tag else { continue };
        if !(tag.starts_with('<') && tag.ends_with('>') && !tag.starts_with("</")) {
            continue;
        }
        let inner = &tag[1..tag.len() - 1];
        let close = format!("</{inner}>");
        let lemma = r.stem.clone().unwrap_or_default();
        let mut distance = 1usize;
        while i + distance < tokens.len() {
            let has_close = tokens[i + distance].readings.iter().any(|o| {
                o.pos_tag.as_deref() == Some(close.as_str())
                    && o.stem.as_deref() == Some(lemma.as_str())
            });
            if has_close {
                if distance >= max_distance {
                    max_distance = distance;
                    selected = Some((inner.to_string(), lemma.clone()));
                }
                break;
            }
            distance += 1;
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    #[test]
    fn parses_multiwords_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/words/multiwords.txt");
        if !path.lt_exists() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let chunker = MultiWordChunker::load(&path, true, true, false, None, true).unwrap();
        assert!(!chunker.space_full.is_empty(), "multiwords should load");
    }

    /// Java `StringTools.isCamelCase` is `token.matches("[a-z]+[A-Z][A-Za-z]+")`
    /// (ASCII, anchored): `iPhone`/`iPad` are camel case, but a hyphen or a
    /// non-ASCII letter breaks the match, so `al-Àndalus` is not.
    #[test]
    fn is_camel_case_matches_java_semantics() {
        assert!(is_camel_case("iPhone"));
        assert!(is_camel_case("iPad"));
        assert!(!is_camel_case("al-Àndalus"));
        assert!(!is_camel_case("Àndalus"));
        assert!(!is_camel_case("al"));
        assert!(!is_camel_case("AL-ÀNDALUS"));
    }

    fn token(surface: &str, is_ws: bool, lemma: &str, tag: &str) -> AnalyzedTokenReadings {
        AnalyzedTokenReadings {
            readings: vec![AnalyzedToken::new(
                surface.to_string(),
                Some(lemma.to_string()),
                Some(tag.to_string()),
            )],
            chunk_tags: Vec::new(),
            whitespace_before: is_ws,
            start_pos: 0,
            raw_byte_len: surface.len(),
            is_whitespace: is_ws,
            is_sentence_start: false,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged: true,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe: false,
            is_pos_tag_unknown: false,
        }
    }

    /// Java builds the `mStartSpace` lookup key from the current token plus
    /// every directly following non-whitespace token, so the elided first
    /// word `n'importe` (`n'` + `importe`) of `n'importe quoi;A` matches.
    #[test]
    fn space_entry_with_elided_first_word_matches() {
        let dir = std::env::temp_dir().join("lt-multiword-elision-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("multiwords.txt");
        std::fs::write(&path, "#separatorRegExp=[\\t;]\nn'importe quoi;A\n").unwrap();
        let chunker = MultiWordChunker::load(&path, false, true, true, None, true).unwrap();
        let mut sentence = AnalyzedSentence {
            text: String::new(),
            offset: 0,
            tokens: vec![
                token("n'", false, "ne", "A"),
                token("importe", false, "importer", "V"),
                token(" ", true, "", ""),
                token("quoi", false, "quoi", "R"),
            ],
            pre_disambig_tokens: Vec::new(),
        };
        chunker.apply(&mut sentence);
        for token in &sentence.tokens {
            if token.is_whitespace {
                continue;
            }
            assert_eq!(
                token.readings[0].stem.as_deref(),
                Some("n'importe quoi"),
                "token {:?}",
                token.surface()
            );
            assert_eq!(token.readings[0].pos_tag.as_deref(), Some("A"));
        }
    }
}
