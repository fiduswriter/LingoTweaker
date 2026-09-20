//! Port of morfologik-speller 2.2.0 (`Speller`, `HMatrix`,
//! `DictionaryMetadata`) plus the upstream `MorfologikSpeller` /
//! `MorfologikMultiSpeller` suggestion pipeline.
//!
//! The error-tolerant FSA walk follows Oflazer's algorithm exactly as
//! implemented in morfologik: candidates are emitted during a byte-level
//! traversal of the dictionary automaton, the edit distance is maintained in
//! an [`HMatrix`] band, replacement-pair maps (`any-to-one`, `any-to-two`,
//! "the rest" via [`Speller::get_all_replacements`]) and
//! diacritic/case equivalence affect the comparison, and results are sorted
//! by `distance * 26 + 25 - frequency` (stable). LT's wrapper then applies
//! the case-pattern adjustment of `MorfologikSpeller.getSuggestions`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use lt_core::{CoreError, Result};
use lt_tagger::{Cfsa2, Charset, DictionaryInfo};
use unicode_normalization::UnicodeNormalization;

pub const LANGUAGETOOL: &str = "LanguageTool";
pub const LANGUAGETOOLER: &str = "LanguageTooler";

/// morfologik `Speller.MAX_WORD_LENGTH`.
pub const MAX_WORD_LENGTH: usize = 120;
/// morfologik `Speller.FREQ_RANGES`.
const FREQ_RANGES: i32 = 26;
const FIRST_RANGE_CODE: u8 = b'A';
const UPPER_SEARCH_LIMIT: i32 = 15;
const MIN_WORD_LENGTH: i32 = 4;
const MAX_RECURSION_LEVEL: i32 = 6;
/// `StringMatcher.MAX_MATCH_LENGTH` used by `MorfologikSpeller.getSuggestions`.
const MAX_MATCH_LENGTH: usize = 250;

/// `morfologik.stemming.DictionaryMetadata` subset needed by the speller.
#[derive(Debug, Clone)]
pub struct SpellerMetadata {
    pub separator: char,
    pub separator_byte: u8,
    pub convert_case: bool,
    pub ignore_punctuation: bool,
    pub ignore_numbers: bool,
    pub ignore_camel_case: bool,
    pub ignore_all_uppercase: bool,
    pub ignore_diacritics: bool,
    pub support_run_on_words: bool,
    pub frequency_included: bool,
    /// `fsa.dict.speller.replacement-pairs`, insertion order preserved.
    pub replacement_pairs: Vec<(String, Vec<String>)>,
    pub input_conversion: Vec<(String, String)>,
    pub output_conversion: Vec<(String, String)>,
    pub equivalent_chars: Vec<(char, Vec<char>)>,
    /// `fsa.dict.encoding` (ISO-8859-15 for Italian).
    pub charset: Charset,
}

impl SpellerMetadata {
    /// Parse a `.info` file like `DictionaryMetadata.read` (defaults: every
    /// boolean attribute is `true` except `frequency-included`).
    pub fn from_info(info: &DictionaryInfo) -> Result<Self> {
        let flag = |key: &str, default: bool| -> bool {
            info.fields
                .get(key)
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(default)
        };
        let separator = info
            .fields
            .get("fsa.dict.separator")
            .and_then(|s| s.chars().next())
            .unwrap_or('+');
        if !separator.is_ascii() {
            return Err(CoreError::Parse(
                "dict".into(),
                format!("separator must be a single byte, got {separator:?}"),
            ));
        }
        let replacement_pairs = parse_replacement_pairs(
            info.fields
                .get("fsa.dict.speller.replacement-pairs")
                .map(String::as_str)
                .unwrap_or(""),
        )?;
        let input_conversion = parse_conversion_pairs(
            info.fields
                .get("fsa.dict.input-conversion")
                .map(String::as_str)
                .unwrap_or(""),
            "fsa.dict.input-conversion",
        )?;
        let output_conversion = parse_conversion_pairs(
            info.fields
                .get("fsa.dict.output-conversion")
                .map(String::as_str)
                .unwrap_or(""),
            "fsa.dict.output-conversion",
        )?;
        let equivalent_chars = parse_equivalent_chars(
            info.fields
                .get("fsa.dict.speller.equivalent-chars")
                .map(String::as_str)
                .unwrap_or(""),
        )?;
        // morfologik `DictionaryLookup`: the query word is encoded with the
        // metadata charset; an unmappable char means "no hit".
        let charset = match info.encoding() {
            Some(name) => Charset::from_info_name(name).ok_or_else(|| {
                CoreError::Parse(
                    "dict".into(),
                    format!("unsupported dictionary encoding: {name}"),
                )
            })?,
            None => Charset::Utf8,
        };
        Ok(Self {
            separator,
            separator_byte: separator as u8,
            convert_case: flag("fsa.dict.speller.convert-case", true),
            ignore_punctuation: flag("fsa.dict.speller.ignore-punctuation", true),
            ignore_numbers: flag("fsa.dict.speller.ignore-numbers", true),
            ignore_camel_case: flag("fsa.dict.speller.ignore-camel-case", true),
            ignore_all_uppercase: flag("fsa.dict.speller.ignore-all-uppercase", true),
            ignore_diacritics: flag("fsa.dict.speller.ignore-diacritics", true),
            // morfologik's `DictionaryAttribute.SUPPORT_RUN_ON_WORDS` key is
            // `runon-words` (no hyphen); the Dutch `nl_NL.info` sets it to
            // false. Reading the wrong key left run-on candidates enabled and
            // added space-inserted suggestions (`ttets` -> `t tets`).
            support_run_on_words: flag("fsa.dict.speller.runon-words", true),
            frequency_included: flag("fsa.dict.frequency-included", false),
            replacement_pairs,
            input_conversion,
            output_conversion,
            equivalent_chars,
            charset,
        })
    }
}

/// `DictionaryAttribute.REPLACEMENT_PAIRS.fromString`: `","`-separated
/// `source target` pairs, `_` means space; duplicate sources accumulate.
fn parse_replacement_pairs(value: &str) -> Result<Vec<(String, Vec<String>)>> {
    let mut pairs: Vec<(String, Vec<String>)> = Vec::new();
    if value.is_empty() {
        return Ok(pairs);
    }
    for string_pair in value.split(',') {
        let string_pair = string_pair.trim();
        if string_pair.is_empty() {
            continue;
        }
        let two: Vec<&str> = string_pair.split(' ').collect();
        if two.len() != 2 {
            return Err(CoreError::Parse(
                "dict".into(),
                format!("replacement-pairs entry is not `source target`: {string_pair}"),
            ));
        }
        let key = two[0].replace('_', " ");
        let val = two[1].replace('_', " ");
        match pairs.iter_mut().find(|(k, _)| *k == key) {
            Some((_, vals)) => vals.push(val),
            None => pairs.push((key, vec![val])),
        }
    }
    Ok(pairs)
}

fn parse_conversion_pairs(value: &str, name: &str) -> Result<Vec<(String, String)>> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    if value.is_empty() {
        return Ok(pairs);
    }
    for string_pair in value.split(',') {
        let string_pair = string_pair.trim();
        if string_pair.is_empty() {
            continue;
        }
        let two: Vec<&str> = string_pair.split(' ').collect();
        if two.len() != 2 {
            return Err(CoreError::Parse(
                "dict".into(),
                format!("{name} entry is not `source target`: {string_pair}"),
            ));
        }
        if !pairs.iter().any(|(k, _)| k == two[0]) {
            pairs.push((two[0].to_string(), two[1].to_string()));
        } else {
            return Err(CoreError::Parse(
                "dict".into(),
                format!("{name} specifies different values for {}", two[0]),
            ));
        }
    }
    Ok(pairs)
}

fn parse_equivalent_chars(value: &str) -> Result<Vec<(char, Vec<char>)>> {
    let mut pairs: Vec<(char, Vec<char>)> = Vec::new();
    if value.is_empty() {
        return Ok(pairs);
    }
    for character_pair in value.split(',') {
        let character_pair = character_pair.trim();
        if character_pair.is_empty() {
            continue;
        }
        let two: Vec<&str> = character_pair.split(' ').collect();
        if two.len() != 2 || two[0].chars().count() != 1 || two[1].chars().count() != 1 {
            return Err(CoreError::Parse(
                "dict".into(),
                format!("equivalent-chars entry is not `a b`: {character_pair}"),
            ));
        }
        let from = two[0].chars().next().unwrap();
        let to = two[1].chars().next().unwrap();
        match pairs.iter_mut().find(|(c, _)| *c == from) {
            Some((_, list)) => list.push(to),
            None => pairs.push((from, vec![to])),
        }
    }
    Ok(pairs)
}

/// `DictionaryLookup.applyReplacements`: ordered string replacements.
pub fn apply_replacements(word: &str, replacements: &[(String, String)]) -> String {
    if replacements.is_empty() {
        return word.to_string();
    }
    let mut s = word.to_string();
    for (key, value) in replacements {
        if key.is_empty() {
            continue;
        }
        let mut search_from = 0;
        while let Some(idx) = s[search_from..].find(key.as_str()) {
            let idx = search_from + idx;
            s.replace_range(idx..idx + key.len(), value);
            search_from = idx + value.len();
        }
    }
    s
}

/// morfologik `Speller.CandidateData`: word plus the original edit distance
/// and the composite weight used for ordering (distance, then frequency).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateData {
    pub word: String,
    pub orig_distance: i32,
    /// `distance * FREQ_RANGES + FREQ_RANGES - frequency - 1`
    pub distance: i32,
}

/// morfologik `Speller.HMatrix`: edit-distance band.
#[derive(Debug, Clone)]
struct HMatrix {
    p: Vec<i32>,
    row_length: usize,
    edit_distance: i32,
}

impl HMatrix {
    fn new(distance: i32, max_length: usize) -> Self {
        let row_length = max_length + 2;
        let column_height = 2 * distance as usize + 3;
        let mut m = Self {
            p: vec![0; row_length * column_height],
            row_length,
            edit_distance: distance,
        };
        m.init();
        m
    }

    fn init(&mut self) {
        let size = self.p.len();
        let edit_distance = self.edit_distance;
        for i in 0..self.row_length - edit_distance as usize - 1 {
            self.p[i] = edit_distance + 1;
            self.p[size - i - 1] = edit_distance + 1;
        }
        for j in 0..(edit_distance + 2) as usize {
            self.p[j * self.row_length] = edit_distance + 1 - j as i32;
            self.p[(j + edit_distance as usize + 1) * self.row_length + j] = j as i32;
        }
    }

    fn get(&self, i: i32, j: i32) -> i32 {
        // Java index arithmetic stays in `int`: negative `j` wraps into the
        // row above (the matrix has one spare row for the `-1` lookups), so
        // the cast to `usize` must happen after the addition.
        let index = (j - i + self.edit_distance + 1)
            .wrapping_mul(self.row_length as i32)
            .wrapping_add(j);
        self.p[index as usize]
    }

    fn set(&mut self, i: i32, j: i32, val: i32) {
        let index = (j - i + self.edit_distance + 1)
            .wrapping_mul(self.row_length as i32)
            .wrapping_add(j);
        self.p[index as usize] = val;
    }
}

/// One node of the compact byte trie: `first_child`/`next_sibling` link
/// siblings in ascending label order, mirroring CFSA2 arc iteration.
#[derive(Debug, Clone, Copy, Default)]
struct TrieNode {
    label: u8,
    is_final: bool,
    has_children: bool,
    first_child: u32,
    next_sibling: u32,
}

/// How a queried byte sequence relates to a dictionary automaton
/// (`morfologik.fsa.MatchResult.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceMatch {
    /// No byte of the input matched from the root.
    NoMatch,
    /// The whole input matches a complete dictionary entry.
    Exact,
    /// The input is longer than the dictionary sequence it starts with.
    AutomatonHasPrefix,
    /// The whole input is a prefix of at least one dictionary entry.
    SequenceIsPrefix,
}

/// Immutable byte trie over full dictionary sequences (`word` + separator +
/// annotation). Node ids double as arcs (0 = "no arc"); the root is 0.
#[derive(Debug, Clone, Default)]
pub struct ByteTrie {
    nodes: Vec<TrieNode>,
}

#[derive(Default)]
struct BuildNode {
    label: u8,
    is_final: bool,
    children: Vec<u32>,
    child_labels: Vec<u8>,
}

impl ByteTrie {
    pub fn build(sequences: &[Vec<u8>]) -> Self {
        let mut build: Vec<BuildNode> = vec![BuildNode::default()];
        for seq in sequences {
            let mut node = 0usize;
            for &b in seq {
                let pos = build[node].child_labels.partition_point(|&l| l < b);
                let child =
                    if pos < build[node].child_labels.len() && build[node].child_labels[pos] == b {
                        build[node].children[pos]
                    } else {
                        let new_id = build.len() as u32;
                        build.push(BuildNode {
                            label: b,
                            is_final: false,
                            children: Vec::new(),
                            child_labels: Vec::new(),
                        });
                        build[node].child_labels.insert(pos, b);
                        build[node].children.insert(pos, new_id);
                        new_id
                    };
                node = child as usize;
            }
            build[node].is_final = true;
        }
        let mut trie = ByteTrie {
            nodes: Vec::with_capacity(build.len()),
        };
        trie.nodes.push(TrieNode::default());
        trie.flatten(&build, 0, 0);
        trie
    }

    fn flatten(&mut self, build: &[BuildNode], build_index: usize, trie_id: u32) {
        let children: Vec<(u8, bool, usize)> = build[build_index]
            .children
            .iter()
            .map(|&child| {
                let child = child as usize;
                (build[child].label, build[child].is_final, child)
            })
            .collect();
        let mut child_ids = Vec::with_capacity(children.len());
        for (label, is_final, child) in &children {
            let id = self.nodes.len() as u32;
            self.nodes.push(TrieNode {
                label: *label,
                is_final: *is_final,
                has_children: !build[*child].children.is_empty(),
                first_child: 0,
                next_sibling: 0,
            });
            child_ids.push(id);
        }
        if !child_ids.is_empty() {
            self.nodes[trie_id as usize].first_child = child_ids[0];
            for pair in child_ids.windows(2) {
                self.nodes[pair[0] as usize].next_sibling = pair[1];
            }
        }
        for (index, &(_, _, child)) in children.iter().enumerate() {
            self.flatten(build, child, child_ids[index]);
        }
    }
}

/// View over either a CFSA2 automaton or an in-memory byte trie; both number
/// nodes/arcs like morfologik's `FSA` (0 = "no arc/node").
///
/// A source can be shared by several [`Speller`] instances (the engine uses
/// one source per dictionary at edit distances 1/2/3).
#[derive(Debug, Clone)]
pub enum DictSource {
    Fsa(Cfsa2),
    Trie(ByteTrie),
}

impl DictSource {
    /// Load a binary Morfologik `.dict` speller dictionary (`speller.dict`).
    pub fn from_dict_file(dict_path: &Path, _info_path: &Path) -> Result<Self> {
        let dict_bytes = lt_data::fs::read(dict_path)
            .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", dict_path.display())))?;
        Ok(DictSource::Fsa(Cfsa2::parse(&dict_bytes)?))
    }

    /// Build a source from plain text dictionary lines.
    pub fn from_lines(lines: &[Vec<u8>]) -> Self {
        DictSource::Trie(ByteTrie::build(lines))
    }

    fn root(&self) -> usize {
        match self {
            DictSource::Fsa(fsa) => fsa.root_node(),
            DictSource::Trie(_) => 0,
        }
    }

    fn first_arc(&self, node: usize) -> usize {
        match self {
            DictSource::Fsa(fsa) => fsa.first_arc(node),
            DictSource::Trie(trie) => trie.nodes[node].first_child as usize,
        }
    }

    fn next_arc(&self, arc: usize) -> usize {
        match self {
            DictSource::Fsa(fsa) => fsa.next_arc(arc),
            DictSource::Trie(trie) => trie.nodes[arc].next_sibling as usize,
        }
    }

    fn arc_label(&self, arc: usize) -> u8 {
        match self {
            DictSource::Fsa(fsa) => fsa.arc_label(arc),
            DictSource::Trie(trie) => trie.nodes[arc].label,
        }
    }

    fn is_arc_final(&self, arc: usize) -> bool {
        match self {
            DictSource::Fsa(fsa) => fsa.is_arc_final(arc),
            DictSource::Trie(trie) => trie.nodes[arc].is_final,
        }
    }

    fn is_arc_terminal(&self, arc: usize) -> bool {
        match self {
            DictSource::Fsa(fsa) => fsa.is_arc_terminal(arc),
            DictSource::Trie(trie) => !trie.nodes[arc].has_children,
        }
    }

    fn end_node(&self, arc: usize) -> usize {
        match self {
            DictSource::Fsa(fsa) => fsa.end_node(arc),
            DictSource::Trie(_) => arc,
        }
    }

    fn arc_by_label(&self, node: usize, label: u8) -> usize {
        match self {
            // FSA node 0 is the "no node" sentinel; the trie root is node 0
            DictSource::Fsa(fsa) => {
                if node == 0 {
                    return 0;
                }
                fsa.arc_by_label(node, label)
            }
            DictSource::Trie(trie) => {
                let mut arc = trie.nodes[node].first_child as usize;
                while arc != 0 {
                    if trie.nodes[arc].label == label {
                        return arc;
                    }
                    arc = trie.nodes[arc].next_sibling as usize;
                }
                0
            }
        }
    }

    /// `morfologik.fsa.FSATraversal.match`: walk a full byte sequence and
    /// report how it relates to the dictionary, plus the node reached.
    ///
    /// The distinction matters: a query that extends a dictionary word past a
    /// terminal arc (`star` + `r`) must *not* keep re-reading the last arc
    /// from the parent node — Java stops at `AUTOMATON_HAS_PREFIX` and
    /// `isInDictionary` then reports "not found". An earlier port returned
    /// the repeated arc and accepted e.g. `STARR` because `star` is a
    /// dictionary word.
    fn match_sequence(&self, key: &[u8]) -> (SequenceMatch, usize) {
        let mut node = self.root();
        for (i, &label) in key.iter().enumerate() {
            let arc = self.arc_by_label(node, label);
            if arc == 0 {
                return (
                    if i > 0 {
                        SequenceMatch::AutomatonHasPrefix
                    } else {
                        SequenceMatch::NoMatch
                    },
                    node,
                );
            }
            if i + 1 == key.len() && self.is_arc_final(arc) {
                return (SequenceMatch::Exact, node);
            }
            if self.is_arc_terminal(arc) {
                return (SequenceMatch::AutomatonHasPrefix, node);
            }
            node = self.end_node(arc);
        }
        (SequenceMatch::SequenceIsPrefix, node)
    }

    /// First final sequence reachable from `node` (depth-first, arc order);
    /// returns the sequence's last label.
    fn first_final_last_label(&self, node: usize) -> Option<u8> {
        let mut arc = self.first_arc(node);
        while arc != 0 {
            if self.is_arc_final(arc) {
                return Some(self.arc_label(arc));
            }
            if !self.is_arc_terminal(arc) {
                if let Some(found) = self.first_final_last_label(self.end_node(arc)) {
                    return Some(found);
                }
            }
            arc = self.next_arc(arc);
        }
        None
    }
}

/// Replacement pattern with optional start/end anchor (`Speller.Pattern`).
#[derive(Debug, Clone)]
struct Pattern {
    chars: Vec<char>,
    start_anchor: bool,
    end_anchor: bool,
}

/// One loaded speller dictionary plus morfologik's lookup/suggestion API.
pub struct Speller {
    source: std::sync::Arc<DictSource>,
    meta: SpellerMetadata,
    edit_distance: i32,
    replacements_any_to_one: HashMap<char, Vec<Pattern>>,
    replacements_any_to_two: HashMap<String, Vec<Pattern>>,
    /// Keys keep their `^`/`$` anchors (`replacementsTheRest`).
    replacements_the_rest: Vec<(String, Vec<String>)>,
}

impl Speller {
    /// Load a binary Morfologik `.dict` speller dictionary.
    pub fn from_dict_file(dict_path: &Path, info_path: &Path, edit_distance: i32) -> Result<Self> {
        let info_text = lt_data::fs::read_to_string(info_path)
            .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", info_path.display())))?;
        let info = DictionaryInfo::parse(&info_text)?;
        let meta = SpellerMetadata::from_info(&info)?;
        Ok(Self::new(
            std::sync::Arc::new(DictSource::from_dict_file(dict_path, info_path)?),
            meta,
            edit_distance,
        ))
    }

    /// Build a speller from plain text dictionary lines (LT builds an FSA at
    /// runtime; a trie reproduces the same byte-level traversal order).
    pub fn from_lines(lines: &[Vec<u8>], meta: SpellerMetadata, edit_distance: i32) -> Self {
        Self::new(
            std::sync::Arc::new(DictSource::from_lines(lines)),
            meta,
            edit_distance,
        )
    }

    /// Build a speller over an existing (possibly shared) dictionary source.
    pub fn from_source(
        source: std::sync::Arc<DictSource>,
        meta: SpellerMetadata,
        edit_distance: i32,
    ) -> Self {
        Self::new(source, meta, edit_distance)
    }

    fn new(source: std::sync::Arc<DictSource>, meta: SpellerMetadata, edit_distance: i32) -> Self {
        let mut any_to_one: HashMap<char, Vec<Pattern>> = HashMap::new();
        let mut any_to_two: HashMap<String, Vec<Pattern>> = HashMap::new();
        let mut the_rest: Vec<(String, Vec<String>)> = Vec::new();
        for (raw_key, values) in &meta.replacement_pairs {
            let start_anchor = raw_key.starts_with('^');
            let end_anchor = raw_key.ends_with('$');
            let stripped = strip_anchors(raw_key);
            for s in values {
                let chars: Vec<char> = stripped.chars().collect();
                match s.chars().count() {
                    1 => {
                        let target = s.chars().next().unwrap();
                        any_to_one.entry(target).or_default().push(Pattern {
                            chars: chars.clone(),
                            start_anchor,
                            end_anchor,
                        });
                    }
                    2 => {
                        any_to_two.entry(s.clone()).or_default().push(Pattern {
                            chars: chars.clone(),
                            start_anchor,
                            end_anchor,
                        });
                    }
                    _ => match the_rest.iter_mut().find(|(k, _)| k == raw_key) {
                        Some((_, list)) => list.push(s.clone()),
                        None => the_rest.push((raw_key.clone(), vec![s.clone()])),
                    },
                }
            }
        }
        Self {
            source,
            meta,
            edit_distance,
            replacements_any_to_one: any_to_one,
            replacements_any_to_two: any_to_two,
            replacements_the_rest: the_rest,
        }
    }

    pub fn metadata(&self) -> &SpellerMetadata {
        &self.meta
    }

    /// morfologik `Speller.isInDictionary`. `containsSeparators` is treated
    /// as `true` like the original port (the vendored speller dictionaries
    /// store `word` + separator + annotation sequences, so a word part is
    /// accepted when a non-empty separator arc follows it).
    pub fn is_in_dictionary(&self, word: &str) -> bool {
        let Some(word_bytes) = self.meta.charset.encode(word) else {
            return false;
        };
        let (kind, node) = self.source.match_sequence(&word_bytes);
        if kind == SequenceMatch::Exact {
            // `containsSeparators` is recomputed for the queried word.
            return !word_bytes.contains(&self.meta.separator_byte);
        }
        kind == SequenceMatch::SequenceIsPrefix
            && !word_bytes.is_empty()
            && self.source.arc_by_label(node, self.meta.separator_byte) != 0
    }

    /// `Speller.getFrequency`: frequency code of the first stored annotation.
    pub fn get_frequency(&self, word: &str) -> i32 {
        if !self.meta.frequency_included {
            return 0;
        }
        let Some(word_bytes) = self.meta.charset.encode(word) else {
            return 0;
        };
        let (kind, node) = self.source.match_sequence(&word_bytes);
        if kind != SequenceMatch::SequenceIsPrefix {
            return 0;
        }
        let sep = self.source.arc_by_label(node, self.meta.separator_byte);
        if sep == 0 || self.source.is_arc_final(sep) {
            return 0;
        }
        match self
            .source
            .first_final_last_label(self.source.end_node(sep))
        {
            Some(last) => (last as i32) - (FIRST_RANGE_CODE as i32),
            None => 0,
        }
    }

    fn converts_case(&self) -> bool {
        self.meta.convert_case
    }

    /// morfologik `Speller.isMisspelled`.
    #[allow(clippy::nonminimal_bool)]
    pub fn is_misspelled_speller(&self, word: &str) -> bool {
        let word_to_check = apply_replacements(word, &self.meta.input_conversion);
        let is_alphabetic = word_to_check.chars().count() != 1
            || word_to_check
                .chars()
                .next()
                .is_some_and(char::is_alphabetic);
        !word_to_check.is_empty()
            && (!self.meta.ignore_punctuation || is_alphabetic)
            && (!self.meta.ignore_numbers || !word_to_check.chars().any(|c| c.is_ascii_digit()))
            && !(self.meta.ignore_camel_case && is_camel_case(&word_to_check))
            && !(self.meta.ignore_all_uppercase
                && is_alphabetic
                && is_all_uppercase(&word_to_check))
            && !self.is_in_dictionary(&word_to_check)
            && (!self.meta.convert_case
                || !(!is_mixed_case(&word_to_check)
                    && (self.is_in_dictionary(&word_to_check.to_lowercase())
                        || (is_all_uppercase(&word_to_check)
                            && self.is_in_dictionary(&initial_uppercase(&word_to_check))))))
    }

    fn are_equal(&self, x: char, y: char) -> bool {
        if x == y {
            return true;
        }
        if let Some((_, chars)) = self.meta.equivalent_chars.iter().find(|(c, _)| *c == x) {
            if chars.contains(&y) {
                return true;
            }
        }
        if self.meta.ignore_diacritics {
            let xn = strip_diacritics(x);
            let yn = strip_diacritics(y);
            if xn == yn {
                return true;
            }
            if self.meta.convert_case
                && xn.is_alphabetic()
                && xn.is_lowercase() != yn.is_lowercase()
            {
                return xn.to_lowercase().eq(yn.to_lowercase());
            }
        }
        false
    }

    /// morfologik `Speller.findReplacementCandidates`.
    pub fn find_replacement_candidates(&self, word: &str) -> Vec<CandidateData> {
        self.find_replacement_candidates_inner(word, false)
    }

    /// morfologik `Speller.findSimilarWordCandidates` (includes dictionary words).
    pub fn find_similar_word_candidates(&self, word: &str) -> Vec<CandidateData> {
        self.find_replacement_candidates_inner(word, true)
    }

    fn find_replacement_candidates_inner(
        &self,
        word: &str,
        even_if_word_in_dictionary: bool,
    ) -> Vec<CandidateData> {
        let word = apply_replacements(word, &self.meta.input_conversion);
        let mut candidates: Vec<CandidateData> = Vec::new();
        if !word.is_empty()
            && word.chars().count() < MAX_WORD_LENGTH
            && (!self.is_in_dictionary(&word) || even_if_word_in_dictionary)
        {
            let mut words_to_check: Vec<String> = Vec::new();
            if !self.replacements_the_rest.is_empty() && word.chars().count() > 1 {
                for word_checked in self.get_all_replacements(&word, 0, 0) {
                    if self.is_in_dictionary(&word_checked) {
                        candidates.push(self.candidate(word_checked.clone(), 0));
                    } else {
                        let lower = word_checked.to_lowercase();
                        let upper = word_checked.to_uppercase();
                        if self.is_in_dictionary(&lower) {
                            candidates.push(self.candidate(lower.clone(), 0));
                        }
                        if self.is_in_dictionary(&upper) {
                            candidates.push(self.candidate(upper, 0));
                        }
                        if lower.chars().count() > 1 {
                            let first_upper = initial_uppercase(&lower);
                            if self.is_in_dictionary(&first_upper) {
                                candidates.push(self.candidate(first_upper, 0));
                            }
                        }
                    }
                    words_to_check.push(word_checked);
                }
            } else {
                words_to_check.push(word.clone());
            }

            let mut i = 1;
            for word_checked in &words_to_check {
                i += 1;
                if i > UPPER_SEARCH_LIMIT {
                    break;
                }
                let word_chars: Vec<char> = word_checked.chars().collect();
                let word_len = word_chars.len() as i32;
                if word_len < MIN_WORD_LENGTH && i > 2 {
                    break;
                }
                let effect_edit_distance = if word_len <= self.edit_distance {
                    word_len - 1
                } else {
                    self.edit_distance
                };
                let mut search = Search {
                    speller: self,
                    h: HMatrix::new(self.edit_distance, MAX_WORD_LENGTH),
                    word: word_chars,
                    word_len,
                    effect_ed: effect_edit_distance,
                    candidate: vec!['\0'; MAX_WORD_LENGTH],
                    out: std::mem::take(&mut candidates),
                };
                search.find_repl(0, self.source.root(), &[], 0, 0, -1, None, '\0');
                candidates = search.out;
            }
        }

        candidates.sort_by_key(|candidate| candidate.distance);

        let mut seen: HashSet<String> = HashSet::new();
        let mut result = Vec::with_capacity(candidates.len());
        for cd in candidates {
            let replaced = apply_replacements(&cd.word, &self.meta.output_conversion);
            if seen.insert(replaced.clone()) && replaced != word {
                result.push(CandidateData {
                    word: replaced,
                    orig_distance: cd.orig_distance,
                    distance: cd.distance,
                });
            }
        }
        result
    }

    fn candidate(&self, word: String, distance: i32) -> CandidateData {
        let freq = self.get_frequency(&word).max(0);
        CandidateData {
            word,
            orig_distance: distance,
            distance: distance * FREQ_RANGES + FREQ_RANGES - freq - 1,
        }
    }

    /// morfologik `Speller.replaceRunOnWordCandidates`.
    pub fn replace_run_on_word_candidates(&self, original: &str) -> Vec<CandidateData> {
        let mut candidates = Vec::new();
        let word_to_check = apply_replacements(original, &self.meta.input_conversion);
        if !self.is_in_dictionary(&word_to_check) && self.meta.support_run_on_words {
            let chars: Vec<char> = word_to_check.chars().collect();
            for i in 1..chars.len() {
                let prefix: String = chars[..i].iter().collect();
                let suffix: String = chars[i..].iter().collect();
                if self.is_in_dictionary(&suffix)
                    || (!is_not_capitalized_word(&suffix)
                        && self.is_in_dictionary(&suffix.to_lowercase()))
                {
                    let prefix_ok = self.is_in_dictionary(&prefix)
                        || (prefix.chars().next().is_some_and(char::is_uppercase)
                            && self.is_in_dictionary(&prefix.to_lowercase()));
                    if prefix_ok {
                        self.add_replacement(&mut candidates, format!("{prefix} {suffix}"));
                    }
                }
            }
        }
        candidates
    }

    fn add_replacement(&self, candidates: &mut Vec<CandidateData>, replacement: String) {
        let replacement = apply_replacements(&replacement, &self.meta.output_conversion);
        candidates.push(self.candidate(replacement, 1));
    }

    /// morfologik `Speller.getAllReplacements`.
    fn get_all_replacements(&self, s: &str, from_index: usize, level: i32) -> Vec<String> {
        let mut replaced = Vec::new();
        if level > MAX_RECURSION_LEVEL {
            replaced.push(s.to_string());
            return replaced;
        }
        let sb: Vec<char> = s.chars().collect();
        let mut index = MAX_WORD_LENGTH;
        let mut key: Option<&(String, Vec<String>)> = None;
        let mut key_length = 0usize;
        let mut found = false;
        let mut stripped_key_for_selected = String::new();
        for entry in &self.replacements_the_rest {
            let aux_key = &entry.0;
            let start_anchor = aux_key.starts_with('^');
            let end_anchor = aux_key.ends_with('$');
            let stripped = strip_anchors(aux_key);
            let stripped_chars: Vec<char> = stripped.chars().collect();
            let aux_index: i64 = if start_anchor && from_index > 0 {
                continue;
            } else if start_anchor {
                if starts_with_chars(&sb, &stripped_chars) {
                    0
                } else {
                    -1
                }
            } else if end_anchor {
                let expected = sb.len() as i64 - stripped_chars.len() as i64;
                if expected >= from_index as i64
                    && index_of_chars(&sb, &stripped_chars, expected as usize)
                        == Some(expected as usize)
                {
                    expected
                } else {
                    -1
                }
            } else {
                match index_of_chars(&sb, &aux_key.chars().collect::<Vec<_>>(), from_index) {
                    Some(i) => i as i64,
                    None => -1,
                }
            };
            if aux_index > -1
                && (aux_index < index as i64
                    || (aux_index == index as i64 && stripped_chars.len() >= key_length))
            {
                index = aux_index as usize;
                key = Some(entry);
                key_length = stripped_chars.len();
                stripped_key_for_selected = stripped;
            }
        }
        if index < MAX_WORD_LENGTH {
            let entry = key.unwrap();
            let selected_chars: Vec<char> = stripped_key_for_selected.chars().collect();
            for rep in &entry.1 {
                if !found {
                    replaced.extend(self.get_all_replacements(
                        s,
                        index + selected_chars.len(),
                        level + 1,
                    ));
                    found = true;
                }
                let rep_chars: Vec<char> = rep.chars().collect();
                // avoid unnecessary replacements
                let rep_from = (from_index + 1).saturating_sub(rep_chars.len());
                let ind = index_of_chars(&sb, &rep_chars, rep_from);
                // Morfologik `Speller.getAllReplacements` (bytecode): the
                // `ind > -1` guard comes first, then
                // `ind == index || ind == index - rep.length() + 1` in Java
                // `int` arithmetic. A negative target can therefore never
                // match; compute it signed to avoid the debug overflow the
                // former `usize` expression had (release wrapped and matched
                // Java by accident).
                if rep_chars.len() > selected_chars.len() && ind.is_some() {
                    let target = index as i64 - rep_chars.len() as i64 + 1;
                    if ind == Some(index) || ind.map(|i| i as i64) == Some(target) {
                        continue;
                    }
                }
                let mut new_sb: Vec<char> = sb[..index].to_vec();
                new_sb.extend(&rep_chars);
                new_sb.extend(&sb[index + selected_chars.len()..]);
                replaced.extend(self.get_all_replacements(
                    &new_sb.iter().collect::<String>(),
                    index + rep_chars.len(),
                    level + 1,
                ));
            }
        }
        if !found {
            replaced.push(sb.iter().collect());
        }
        replaced
    }
}

fn strip_anchors(key: &str) -> String {
    let start = usize::from(key.starts_with('^'));
    let end = key.len() - usize::from(key.ends_with('$') && key.len() > start);
    key[start..end].to_string()
}

fn starts_with_chars(haystack: &[char], needle: &[char]) -> bool {
    haystack.len() >= needle.len() && &haystack[..needle.len()] == needle
}

fn index_of_chars(haystack: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from.min(haystack.len()));
    }
    if haystack.len() < needle.len() || from > haystack.len() - needle.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

/// Mutable state of one Oflazer search (`Speller.findRepl`).
struct Search<'a> {
    speller: &'a Speller,
    h: HMatrix,
    word: Vec<char>,
    word_len: i32,
    effect_ed: i32,
    candidate: Vec<char>,
    out: Vec<CandidateData>,
}

impl Search<'_> {
    fn set_candidate(&mut self, index: i32, ch: char) {
        let index = index as usize;
        if index >= self.candidate.len() {
            self.candidate.resize(index + 1, '\0');
        }
        self.candidate[index] = ch;
    }

    fn candidate_str(&self, cand_index: i32) -> String {
        self.candidate[..cand_index as usize + 1].iter().collect()
    }

    fn is_end_of_candidate(&self, arc: usize, word_index: i32) -> bool {
        (self.speller.source.is_arc_final(arc) || self.is_before_separator(arc))
            && (self.word_len - 1 - word_index).abs() <= self.effect_ed
    }

    fn is_before_separator(&self, arc: usize) -> bool {
        let child = self.speller.source.end_node(arc);
        let sep = self
            .speller
            .source
            .arc_by_label(child, self.speller.meta.separator_byte);
        sep != 0 && !self.speller.source.is_arc_terminal(sep)
    }

    fn is_arc_not_terminal(&self, arc: usize, cand_index: i32) -> bool {
        !self.speller.source.is_arc_terminal(arc)
            && self.candidate[cand_index as usize] != self.speller.meta.separator
    }

    fn ed(&mut self, i: i32, j: i32, word_index: i32, cand_index: i32) -> i32 {
        let result = if self.speller.are_equal(
            self.word[word_index as usize],
            self.candidate[cand_index as usize],
        ) {
            self.h.get(i, j)
        } else if word_index > 0
            && cand_index > 0
            && self.word[word_index as usize] == self.candidate[(cand_index - 1) as usize]
            && self.word[(word_index - 1) as usize] == self.candidate[cand_index as usize]
        {
            let a = self.h.get(i - 1, j - 1);
            let b = self.h.get(i + 1, j);
            let c = self.h.get(i, j + 1);
            1 + a.min(b).min(c)
        } else {
            let a = self.h.get(i, j);
            let b = self.h.get(i + 1, j);
            let c = self.h.get(i, j + 1);
            1 + a.min(b).min(c)
        };
        self.h.set(i + 1, j + 1, result);
        result
    }

    fn cuted(&mut self, depth: i32, word_index: i32, cand_index: i32) -> i32 {
        let l = 0.max(depth - self.effect_ed);
        let u = (self.word_len - 1 - (word_index - depth)).min(depth + self.effect_ed);
        let mut min_ed = self.effect_ed + 1;
        let mut wi = word_index + l - depth;
        let mut i = l;
        while i <= u {
            let d = self.ed(i, depth, wi, cand_index);
            if d < min_ed {
                min_ed = d;
            }
            i += 1;
            wi += 1;
        }
        min_ed
    }

    fn match_any_to_one(&self, word_index: i32, cand_index: i32) -> i32 {
        let ch = self.candidate[cand_index as usize];
        if let Some(patterns) = self.speller.replacements_any_to_one.get(&ch) {
            for p in patterns {
                if p.start_anchor && word_index != 0 {
                    continue;
                }
                let mut i = 0usize;
                while i < p.chars.len()
                    && (word_index + i as i32) < self.word_len
                    && p.chars[i] == self.word[(word_index + i as i32) as usize]
                {
                    i += 1;
                }
                if i == p.chars.len() {
                    if p.end_anchor && word_index + i as i32 != self.word_len {
                        continue;
                    }
                    return i as i32;
                }
            }
        }
        0
    }

    fn match_any_to_two(
        &self,
        word_index: i32,
        cand_index: i32,
        min_lookback_word_index: i32,
        last_any_to_one_source: Option<&str>,
        last_any_to_one_target: char,
    ) -> i32 {
        if cand_index > 0 && word_index > 0 {
            let mut key_buf = [0u8; 8];
            let a = self.candidate[(cand_index - 1) as usize];
            let b = self.candidate[cand_index as usize];
            let a_len = a.encode_utf8(&mut key_buf).len();
            let b_len = b.encode_utf8(&mut key_buf[a_len..]).len();
            let two_char = std::str::from_utf8(&key_buf[..a_len + b_len]).unwrap_or("");
            if let Some(patterns) = self.speller.replacements_any_to_two.get(two_char) {
                for p in patterns {
                    if p.start_anchor && word_index - 1 != 0 {
                        continue;
                    }
                    if p.chars.len() == 2
                        && word_index < self.word_len
                        && self.candidate[(cand_index - 1) as usize]
                            == self.word[(word_index - 1) as usize]
                        && self.candidate[cand_index as usize] == self.word[word_index as usize]
                    {
                        return 0;
                    }
                    let mut i = 0usize;
                    while i < p.chars.len()
                        && (word_index - 1 + i as i32) < self.word_len
                        && p.chars[i] == self.word[(word_index - 1 + i as i32) as usize]
                    {
                        i += 1;
                    }
                    if i == p.chars.len() {
                        if p.end_anchor && word_index - 1 + i as i32 != self.word_len {
                            continue;
                        }
                        if word_index - 1 < min_lookback_word_index
                            && last_any_to_one_source.is_some()
                            && p.chars.len() == 1
                            && p.chars[0] == last_any_to_one_target
                            && last_any_to_one_source == Some(two_char)
                        {
                            continue;
                        }
                        return i as i32;
                    }
                }
            }
        }
        0
    }

    #[allow(clippy::too_many_arguments)]
    fn find_repl(
        &mut self,
        depth: i32,
        node: usize,
        prev_bytes: &[u8],
        word_index: i32,
        cand_index: i32,
        min_lookback_word_index: i32,
        last_any_to_one_source: Option<&str>,
        last_any_to_one_target: char,
    ) {
        let mut arc = self.speller.source.first_arc(node);
        while arc != 0 {
            // the byte prefix is at most one UTF-8 char wide; a stack buffer
            // avoids a heap allocation per visited arc
            let mut byte_buf = [0u8; 8];
            let base = prev_bytes.len().min(byte_buf.len() - 1);
            byte_buf[..base].copy_from_slice(&prev_bytes[..base]);
            byte_buf[base] = self.speller.source.arc_label(arc);
            let bytes = &byte_buf[..base + 1];
            let decoded: Option<char> = self.speller.meta.charset.decode_one(bytes);
            if let Some(ch) = decoded {
                self.set_candidate(cand_index, ch);
                // replacement "any to two"
                let length_replacement = self.match_any_to_two(
                    word_index,
                    cand_index,
                    min_lookback_word_index,
                    last_any_to_one_source,
                    last_any_to_one_target,
                );
                if length_replacement > 0 {
                    if self.is_end_of_candidate(arc, word_index) {
                        let mut dist = self.h.get(depth - 1, depth - 1);
                        if dist <= self.effect_ed {
                            let extra =
                                (self.word_len - 1 - (word_index + length_replacement - 2)).abs();
                            if extra > 0 {
                                dist += extra;
                            }
                            if dist <= self.effect_ed {
                                let word = self.candidate_str(cand_index);
                                self.out.push(self.speller.candidate(word, dist));
                            }
                        }
                    }
                    if self.is_arc_not_terminal(arc, cand_index) {
                        let x = self.h.get(depth, depth);
                        self.h.set(depth, depth, self.h.get(depth - 1, depth - 1));
                        self.find_repl(
                            0.max(depth),
                            self.speller.source.end_node(arc),
                            &[],
                            word_index + length_replacement - 1,
                            cand_index + 1,
                            min_lookback_word_index,
                            last_any_to_one_source,
                            last_any_to_one_target,
                        );
                        self.h.set(depth, depth, x);
                    }
                }
                // replacement "any to one"
                let length_replacement = self.match_any_to_one(word_index, cand_index);
                if length_replacement > 0 {
                    if self.is_end_of_candidate(arc, word_index) {
                        let mut dist = self.h.get(depth, depth);
                        if dist <= self.effect_ed {
                            let extra =
                                (self.word_len - 1 - (word_index + length_replacement - 1)).abs();
                            if extra > 0 {
                                dist += extra;
                            }
                            if dist <= self.effect_ed {
                                let word = self.candidate_str(cand_index);
                                self.out.push(self.speller.candidate(word, dist));
                            }
                        }
                    }
                    if self.is_arc_not_terminal(arc, cand_index) {
                        let new_source: String = self.word
                            [word_index as usize..(word_index + length_replacement) as usize]
                            .iter()
                            .collect();
                        let target = self.candidate[cand_index as usize];
                        self.find_repl(
                            depth,
                            self.speller.source.end_node(arc),
                            &[],
                            word_index + length_replacement,
                            cand_index + 1,
                            word_index + length_replacement,
                            Some(&new_source),
                            target,
                        );
                    }
                }
                // general
                if self.cuted(depth, word_index, cand_index) <= self.effect_ed {
                    if self.is_end_of_candidate(arc, word_index) {
                        let dist = self.ed(
                            self.word_len - 1 - (word_index - depth),
                            depth,
                            self.word_len - 1,
                            cand_index,
                        );
                        if dist <= self.effect_ed {
                            let word = self.candidate_str(cand_index);
                            self.out.push(self.speller.candidate(word, dist));
                        }
                    }
                    if self.is_arc_not_terminal(arc, cand_index) {
                        self.find_repl(
                            depth + 1,
                            self.speller.source.end_node(arc),
                            &[],
                            word_index + 1,
                            cand_index + 1,
                            min_lookback_word_index,
                            last_any_to_one_source,
                            last_any_to_one_target,
                        );
                    }
                }
            } else if !self.speller.source.is_arc_terminal(arc) {
                let end = self.speller.source.end_node(arc);
                self.find_repl(
                    depth,
                    end,
                    bytes,
                    word_index,
                    cand_index,
                    min_lookback_word_index,
                    last_any_to_one_source,
                    last_any_to_one_target,
                );
            }
            arc = self.speller.source.next_arc(arc);
        }
    }
}

/// LT `WeightedSuggestion` (word + composite weight).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedSuggestion {
    pub word: String,
    pub weight: i32,
}

/// LT `MorfologikSpeller.getSuggestions` for one dictionary.
pub struct MorfologikSpeller {
    speller: Speller,
}

impl MorfologikSpeller {
    pub fn from_dict_file(
        dict_path: &Path,
        info_path: &Path,
        max_edit_distance: i32,
    ) -> Result<Self> {
        Ok(Self {
            speller: Speller::from_dict_file(dict_path, info_path, max_edit_distance)?,
        })
    }

    pub fn from_lines(lines: Vec<Vec<u8>>, meta: SpellerMetadata, max_edit_distance: i32) -> Self {
        Self {
            speller: Speller::from_lines(&lines, meta, max_edit_distance),
        }
    }

    /// Build a speller over a shared dictionary source.
    pub fn from_source(
        source: std::sync::Arc<DictSource>,
        meta: SpellerMetadata,
        max_edit_distance: i32,
    ) -> Self {
        Self {
            speller: Speller::from_source(source, meta, max_edit_distance),
        }
    }

    pub fn speller(&self) -> &Speller {
        &self.speller
    }

    pub fn is_misspelled(&self, word: &str) -> bool {
        if word.is_empty() || word == LANGUAGETOOL || word == LANGUAGETOOLER {
            return false;
        }
        self.speller.is_misspelled_speller(word)
    }

    pub fn get_frequency(&self, word: &str) -> i32 {
        let freq = self.speller.get_frequency(word);
        if freq == 0 && word != word.to_lowercase() {
            self.speller.get_frequency(&word.to_lowercase())
        } else {
            freq
        }
    }

    /// LT `MorfologikSpeller.getSuggestions`.
    pub fn get_suggestions(&self, word: &str) -> Vec<WeightedSuggestion> {
        let mut suggestions: Vec<WeightedSuggestion> = Vec::new();
        let word_chars = word.chars().count();
        if word_chars > MAX_MATCH_LENGTH {
            return suggestions;
        }
        if word_chars < 50 {
            for cd in self.speller.find_replacement_candidates(word) {
                suggestions.push(WeightedSuggestion {
                    word: cd.word,
                    weight: cd.distance,
                });
            }
        }
        for cd in self.speller.replace_run_on_word_candidates(word) {
            suggestions.push(WeightedSuggestion {
                word: cd.word,
                weight: cd.distance,
            });
        }
        let converts_case = self.speller.converts_case();
        if converts_case && is_all_uppercase(word) {
            let mut i = 0usize;
            while i < suggestions.len() {
                let sugg = suggestions[i].clone();
                let mut all_uppercase = sugg.word.to_uppercase();
                if all_uppercase == word || is_mixed_case(&sugg.word) {
                    all_uppercase = sugg.word.clone();
                }
                match get_suggestion_index(&suggestions, &all_uppercase) {
                    Some(aux) if aux > i => {
                        suggestions.remove(aux);
                    }
                    Some(aux) if aux < i => {
                        // Java `i--` + loop `i++` keeps the index: the next
                        // element shifts in and is processed in the same slot
                        suggestions.remove(i);
                        continue;
                    }
                    _ => {}
                }
                suggestions[i] = WeightedSuggestion {
                    word: all_uppercase,
                    weight: sugg.weight,
                };
                i += 1;
            }
        } else if converts_case && word.chars().next().is_some_and(char::is_uppercase) {
            let mut i = 0usize;
            while i < suggestions.len() {
                let sugg = suggestions[i].clone();
                let mut uppercase_first = uppercase_first_char(&sugg.word);
                if uppercase_first == word || is_mixed_case(&sugg.word) {
                    uppercase_first = sugg.word.clone();
                }
                match get_suggestion_index(&suggestions, &uppercase_first) {
                    Some(aux) if aux > i => {
                        suggestions.remove(aux);
                    }
                    Some(aux) if aux < i => {
                        // Java `i--` + loop `i++` keeps the index: the next
                        // element shifts in and is processed in the same slot
                        suggestions.remove(i);
                        continue;
                    }
                    _ => {}
                }
                suggestions[i] = WeightedSuggestion {
                    word: uppercase_first,
                    weight: sugg.weight,
                };
                i += 1;
            }
        }
        suggestions
    }
}

fn get_suggestion_index(suggestions: &[WeightedSuggestion], word: &str) -> Option<usize> {
    suggestions.iter().position(|s| s.word == word)
}

fn strip_diacritics(c: char) -> char {
    c.to_string().nfd().next().unwrap_or(c)
}

/// LT `MorfologikMultiSpeller`: merges a binary dictionary with the plain
/// text dictionaries.
pub struct MultiSpeller {
    spellers: Vec<MorfologikSpeller>,
    default_dict_spellers: Vec<usize>,
}

impl MultiSpeller {
    pub fn new(spellers: Vec<MorfologikSpeller>, default_dict_spellers: Vec<usize>) -> Self {
        Self {
            spellers,
            default_dict_spellers,
        }
    }

    pub fn is_misspelled(&self, word: &str) -> bool {
        for speller in &self.spellers {
            if !speller.is_misspelled(word) {
                return false;
            }
        }
        true
    }

    pub fn get_frequency(&self, word: &str) -> i32 {
        for speller in &self.spellers {
            let freq = speller.get_frequency(word);
            if freq > 0 {
                return freq;
            }
        }
        0
    }

    pub fn get_weighted_suggestions_from_default_dicts(
        &self,
        word: &str,
    ) -> Vec<WeightedSuggestion> {
        self.get_suggestions_from_spellers(word, &self.default_dict_spellers)
    }

    /// LT `MorfologikMultiSpeller.getSuggestions`: the merged suggestions of
    /// all spellers (binary + plain text, user dictionaries first).
    pub fn get_suggestions(&self, word: &str) -> Vec<WeightedSuggestion> {
        let all: Vec<usize> = (0..self.spellers.len()).collect();
        self.get_suggestions_from_spellers(word, &all)
    }

    /// Merged suggestions of all spellers, values only.
    pub fn get_suggestions_words(&self, word: &str) -> Vec<String> {
        self.get_suggestions(word)
            .into_iter()
            .map(|suggestion| suggestion.word)
            .collect()
    }

    fn get_suggestions_from_spellers(
        &self,
        word: &str,
        speller_indices: &[usize],
    ) -> Vec<WeightedSuggestion> {
        let mut result: Vec<WeightedSuggestion> = Vec::new();
        let mut seen_words: HashSet<String> = HashSet::new();
        for &index in speller_indices {
            for suggestion in self.spellers[index].get_suggestions(word) {
                if !seen_words.contains(&suggestion.word) && suggestion.word != word {
                    result.push(suggestion.clone());
                }
                seen_words.insert(suggestion.word);
            }
        }
        result.sort_by_key(|suggestion| suggestion.weight);
        result
    }
}

// ---------------------------------------------------------------------------
// Case/word-shape helpers (LT `StringTools` / morfologik `Speller`)
// ---------------------------------------------------------------------------

pub fn is_all_uppercase(s: &str) -> bool {
    !s.chars().any(|c| c.is_alphabetic() && c.is_lowercase())
}

pub fn is_not_all_lowercase(s: &str) -> bool {
    s.chars().any(|c| c.is_alphabetic() && !c.is_lowercase())
}

pub fn is_capitalized_word(s: &str) -> bool {
    match s.chars().next() {
        Some(first) if first.is_uppercase() => s
            .chars()
            .skip(1)
            .all(|c| !c.is_alphabetic() || c.is_lowercase()),
        _ => false,
    }
}

pub fn is_mixed_case(s: &str) -> bool {
    !is_all_uppercase(s) && !is_capitalized_word(s) && is_not_all_lowercase(s)
}

fn is_not_capitalized_word(s: &str) -> bool {
    if !s.is_empty() && s.chars().next().is_some_and(char::is_uppercase) {
        for c in s.chars().skip(1) {
            if c.is_alphabetic() && !c.is_lowercase() {
                return true;
            }
        }
        return false;
    }
    true
}

pub fn is_camel_case(s: &str) -> bool {
    !s.is_empty()
        && !is_all_uppercase(s)
        && is_not_capitalized_word(s)
        && s.chars().next().is_some_and(char::is_uppercase)
        && (s.chars().count() <= 1 || s.chars().nth(1).is_some_and(char::is_lowercase))
        && is_not_all_lowercase(s)
}

fn initial_uppercase(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => {
            let mut out: String = first.to_uppercase().collect();
            out.extend(chars.flat_map(|c| c.to_lowercase()));
            out
        }
        None => String::new(),
    }
}

/// LT `StringTools.uppercaseFirstChar`.
pub fn uppercase_first_char(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 {
        return chars[0].to_uppercase().collect();
    }
    // `StringTools.changeFirstCharCase`: skip leading non-letter-or-digit
    let last = chars.len() - 1;
    let mut pos = 0usize;
    while !chars[pos].is_alphanumeric() && last > pos {
        pos += 1;
    }
    let mut out: String = chars[..pos].iter().collect();
    out.extend(chars[pos].to_uppercase());
    out.extend(&chars[pos + 1..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_tagger::DictionaryInfo;

    /// `DictionaryAttribute.SUPPORT_RUN_ON_WORDS` is spelled
    /// `fsa.dict.speller.runon-words` (no hyphen between `run` and `on`);
    /// the Dutch `nl_NL.info` sets it false (regression: reading
    /// `run-on-words` left run-on candidates enabled and added
    /// space-inserted suggestions like `ttets` -> `t tets`).
    #[test]
    fn runon_words_metadata_uses_the_morfologik_key() {
        let info = DictionaryInfo::parse(
            "fsa.dict.separator=~\nfsa.dict.encoding=utf-8\nfsa.dict.speller.runon-words=false\n",
        )
        .unwrap();
        assert!(
            !SpellerMetadata::from_info(&info)
                .unwrap()
                .support_run_on_words
        );
        let info = DictionaryInfo::parse("fsa.dict.speller.runon-words=true\n").unwrap();
        assert!(
            SpellerMetadata::from_info(&info)
                .unwrap()
                .support_run_on_words
        );
        // the default stays enabled when the key is absent
        let info = DictionaryInfo::parse("fsa.dict.encoding=utf-8\n").unwrap();
        assert!(
            SpellerMetadata::from_info(&info)
                .unwrap()
                .support_run_on_words
        );
    }
}
