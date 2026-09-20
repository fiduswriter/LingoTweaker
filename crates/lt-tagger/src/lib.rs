//! Morfologik dictionary tagger.
//!
//! Reads Morfologik `.dict` automata (CFSA2 format, version byte 0xc6 — the
//! format all current LT tagger/speller dictionaries use) and exposes
//! surface-form lookups with LT tag strings.
//!
//! Decision D-001 (internal development note): direct automaton reading, with
//! the automaton expanded into an in-memory map at load time (fast, simple);
//! revisit streaming FSA queries in the P2.5 performance pass.

pub mod catalan;
pub mod catalan_synth;
pub mod charset;
pub mod dutch;
pub mod dutch_synth;
pub mod english;
pub mod french;
pub mod french_synth;
pub mod fsa5;
pub mod german;
pub mod german_synth;
pub mod italian;
pub mod italian_synth;
pub mod manual_synth;
pub mod portuguese;
pub mod portuguese_synth;
pub mod soros;
pub mod spanish;
pub mod spanish_synth;

pub use catalan::CatalanTagger;
pub use catalan_synth::CatalanSynthesizer;
pub use charset::Charset;
pub use dutch::{CompoundPartsProvider, DutchTagger};
pub use dutch_synth::DutchSynthesizer;
pub use english::{
    is_all_uppercase, is_capitalized_word, is_mixed_case, is_not_all_lowercase,
    lowercase_first_char, uppercase_first_char, EnglishTagger,
};
pub use french::FrenchTagger;
pub use french_synth::FrenchSynthesizer;
pub use fsa5::Fsa5;
pub use german::{GermanTagger, SwissGermanTagger};
pub use german_synth::GermanSynthesizer;
pub use italian::ItalianTagger;
pub use italian_synth::ItalianSynthesizer;
pub use manual_synth::ManualSynthesizer;
pub use portuguese::PortugueseTagger;
pub use portuguese_synth::PortugueseSynthesizer;
pub use soros::Soros;
pub use spanish::{is_emoji, SpanishTagger};
pub use spanish_synth::SpanishSynthesizer;

use std::collections::HashMap;
use std::path::Path;

use lt_core::Result;

/// CFSA2 automaton read directly from a `.dict` file.
#[derive(Debug, Clone)]
pub struct Cfsa2 {
    arcs: Vec<u8>,
    label_mapping: Vec<u8>,
    root: usize,
}

const BIT_FINAL_ARC: u8 = 1 << 5;
const BIT_LAST_ARC: u8 = 1 << 6;
const BIT_TARGET_NEXT: u8 = 1 << 7;
const LABEL_INDEX_MASK: u8 = (1 << 5) - 1;
const VERSION_CFSA2: u8 = 0xc6;

impl Cfsa2 {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 8 {
            return Err(lt_core::CoreError::Parse(
                "dict".into(),
                "file too short".into(),
            ));
        }
        let magic = &bytes[..4];
        if magic != b"\\fsa" {
            return Err(lt_core::CoreError::Parse(
                "dict".into(),
                format!("bad magic {magic:02x?}, expected \\\\fsa (CFSA)"),
            ));
        }
        let version = bytes[4];
        if version != VERSION_CFSA2 {
            return Err(lt_core::CoreError::Parse(
                "dict".into(),
                format!("unsupported FSA version {version:#04x} (only CFSA2 supported)"),
            ));
        }
        // flags: 2 bytes, we only care whether NUMBERS (1 << 8) is set
        let flags = u16::from_be_bytes([bytes[5], bytes[6]]);
        let has_numbers = flags & (1 << 8) != 0;
        let mut pos = 7;
        let label_mapping_size = bytes[pos] as usize;
        pos += 1;
        let label_mapping = bytes[pos..pos + label_mapping_size].to_vec();
        pos += label_mapping_size;
        let arcs = bytes[pos..].to_vec();

        let dict = Self {
            arcs,
            label_mapping,
            root: 0,
        };
        let root = dict.destination_node_offset(dict.first_arc(0));
        let mut dict = dict;
        dict.root = root;
        let _ = has_numbers; // NUMBERS not set in any LT dictionary
        Ok(dict)
    }

    pub fn root_node(&self) -> usize {
        self.root
    }

    pub fn first_arc(&self, node: usize) -> usize {
        node
    }

    pub fn next_arc(&self, arc: usize) -> usize {
        if self.is_arc_last(arc) {
            0
        } else {
            self.skip_arc(arc)
        }
    }

    pub fn arc_label(&self, arc: usize) -> u8 {
        let index = (self.arcs[arc] & LABEL_INDEX_MASK) as usize;
        if index > 0 {
            self.label_mapping[index]
        } else {
            self.arcs[arc + 1]
        }
    }

    pub fn is_arc_final(&self, arc: usize) -> bool {
        self.arcs[arc] & BIT_FINAL_ARC != 0
    }

    /// `CFSA2.isArcTerminal`: the arc has no destination node.
    pub fn is_arc_terminal(&self, arc: usize) -> bool {
        self.destination_node_offset(arc) == 0
    }

    /// `CFSA2.getEndNode`; only valid for non-terminal arcs.
    pub fn end_node(&self, arc: usize) -> usize {
        self.destination_node_offset(arc)
    }

    /// `CFSA2.getArc(node, label)`; 0 when there is no such arc.
    pub fn arc_by_label(&self, node: usize, label: u8) -> usize {
        let mut arc = self.first_arc(node);
        while arc != 0 {
            if self.arc_label(arc) == label {
                return arc;
            }
            arc = self.next_arc(arc);
        }
        0
    }

    fn is_arc_last(&self, arc: usize) -> bool {
        self.arcs[arc] & BIT_LAST_ARC != 0
    }

    fn is_next_set(&self, arc: usize) -> bool {
        self.arcs[arc] & BIT_TARGET_NEXT != 0
    }

    fn destination_node_offset(&self, arc: usize) -> usize {
        if self.is_next_set(arc) {
            let mut arc = arc;
            while !self.is_arc_last(arc) {
                arc = self.next_arc(arc);
            }
            self.skip_arc(arc)
        } else {
            let start = arc
                + if self.arcs[arc] & LABEL_INDEX_MASK == 0 {
                    2
                } else {
                    1
                };
            Self::read_vint(&self.arcs, start)
        }
    }

    fn skip_arc(&self, offset: usize) -> usize {
        let flag = self.arcs[offset];
        let mut offset = offset + 1;
        if flag & LABEL_INDEX_MASK == 0 {
            offset += 1;
        }
        if flag & BIT_TARGET_NEXT == 0 {
            offset = Self::skip_vint_arcs(&self.arcs, offset);
        }
        offset
    }

    fn read_vint(array: &[u8], mut offset: usize) -> usize {
        let mut b = array[offset];
        let mut value = (b & 0x7f) as usize;
        let mut shift = 7;
        while b >= 0x80 {
            offset += 1;
            b = array[offset];
            value |= ((b & 0x7f) as usize) << shift;
            shift += 7;
        }
        value
    }

    fn skip_vint_arcs(array: &[u8], mut offset: usize) -> usize {
        while array[offset] >= 0x80 {
            offset += 1;
        }
        offset + 1
    }

    /// Walk `key` from `node`; returns the node reached (None if the prefix is
    /// not in the automaton).
    pub fn walk(&self, node: usize, key: &[u8]) -> Option<usize> {
        self.walk_final(node, key).map(|(node, _)| node)
    }

    /// Like [`Self::walk`], but also reports whether the last arc of the path
    /// is final (i.e. the exact key is an entry of the automaton).
    pub fn walk_final(&self, node: usize, key: &[u8]) -> Option<(usize, bool)> {
        let mut node = node;
        let mut last_final = false;
        for &label in key {
            let mut found = None;
            let mut arc = self.first_arc(node);
            while arc != 0 {
                if self.arc_label(arc) == label {
                    last_final = self.is_arc_final(arc);
                    found = Some(self.destination_node_offset(arc));
                    break;
                }
                arc = self.next_arc(arc);
            }
            node = found?;
        }
        Some((node, last_final))
    }

    /// Depth-first traversal of all sequences starting at `node` that end on
    /// final arcs, calling `f` with the accumulated suffix bytes.
    pub fn visit_sequences<F: FnMut(&[u8])>(&self, node: usize, f: &mut F) {
        let mut buf = Vec::new();
        self.visit(node, &mut buf, f);
    }

    /// Like [`Self::visit_sequences`], but only descends into arcs labeled
    /// `first` at `node`. Dictionary entries encode `word + sep + annotation`,
    /// so this visits the annotations of the word ending at `node` without
    /// walking the (potentially huge) subtree of longer words sharing the
    /// prefix. Arc order, and therefore sequence order, is preserved.
    pub fn visit_sequences_with_first<F: FnMut(&[u8])>(&self, node: usize, first: u8, f: &mut F) {
        let mut buf = vec![first];
        let mut arc = self.first_arc(node);
        while arc != 0 {
            if self.arc_label(arc) == first {
                if self.is_arc_final(arc) {
                    f(&buf);
                }
                let dest = self.destination_node_offset(arc);
                if dest != 0 {
                    self.visit(dest, &mut buf, f);
                }
            }
            arc = self.next_arc(arc);
        }
    }

    fn visit<F: FnMut(&[u8])>(&self, node: usize, buf: &mut Vec<u8>, f: &mut F) {
        let mut arc = self.first_arc(node);
        while arc != 0 {
            let label = self.arc_label(arc);
            buf.push(label);
            if self.is_arc_final(arc) {
                f(buf);
            }
            let dest = self.destination_node_offset(arc);
            if dest != 0 {
                self.visit(dest, buf, f);
            }
            buf.pop();
            arc = self.next_arc(arc);
        }
    }
}

/// A Morfologik automaton: current CFSA2 or the older FSA5 format
/// (`it/italian.dict`). The primitives mirror `morfologik.fsa.FSA`.
#[derive(Debug, Clone)]
pub enum Automaton {
    Cfsa2(Cfsa2),
    Fsa5(Fsa5),
}

impl Automaton {
    /// Parse either format from the same `\fsa`-prefixed bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match bytes.get(4) {
            Some(&VERSION_CFSA2) => Ok(Automaton::Cfsa2(Cfsa2::parse(bytes)?)),
            Some(&fsa5::VERSION_FSA5) => Ok(Automaton::Fsa5(Fsa5::parse(bytes)?)),
            Some(&other) => Err(lt_core::CoreError::Parse(
                "dict".into(),
                format!("unsupported FSA version {other:#04x} (CFSA2/FSA5 supported)"),
            )),
            None => Err(lt_core::CoreError::Parse(
                "dict".into(),
                "file too short".into(),
            )),
        }
    }

    pub fn root_node(&self) -> usize {
        match self {
            Automaton::Cfsa2(a) => a.root_node(),
            Automaton::Fsa5(a) => a.root_node(),
        }
    }

    pub fn first_arc(&self, node: usize) -> usize {
        match self {
            Automaton::Cfsa2(a) => a.first_arc(node),
            Automaton::Fsa5(a) => a.first_arc(node),
        }
    }

    pub fn next_arc(&self, arc: usize) -> usize {
        match self {
            Automaton::Cfsa2(a) => a.next_arc(arc),
            Automaton::Fsa5(a) => a.next_arc(arc),
        }
    }

    pub fn arc_label(&self, arc: usize) -> u8 {
        match self {
            Automaton::Cfsa2(a) => a.arc_label(arc),
            Automaton::Fsa5(a) => a.arc_label(arc),
        }
    }

    pub fn is_arc_final(&self, arc: usize) -> bool {
        match self {
            Automaton::Cfsa2(a) => a.is_arc_final(arc),
            Automaton::Fsa5(a) => a.is_arc_final(arc),
        }
    }

    pub fn is_arc_terminal(&self, arc: usize) -> bool {
        match self {
            Automaton::Cfsa2(a) => a.is_arc_terminal(arc),
            Automaton::Fsa5(a) => a.is_arc_terminal(arc),
        }
    }

    pub fn end_node(&self, arc: usize) -> usize {
        match self {
            Automaton::Cfsa2(a) => a.end_node(arc),
            Automaton::Fsa5(a) => a.end_node(arc),
        }
    }

    pub fn arc_by_label(&self, node: usize, label: u8) -> usize {
        match self {
            Automaton::Cfsa2(a) => a.arc_by_label(node, label),
            Automaton::Fsa5(a) => a.arc_by_label(node, label),
        }
    }

    fn destination_node_offset(&self, arc: usize) -> usize {
        match self {
            Automaton::Cfsa2(a) => a.end_node(arc),
            Automaton::Fsa5(a) => a.end_node(arc),
        }
    }

    /// Walk `key` from `node`; returns the node reached (None if the prefix is
    /// not in the automaton).
    pub fn walk(&self, node: usize, key: &[u8]) -> Option<usize> {
        self.walk_final(node, key).map(|(node, _)| node)
    }

    /// Like [`Self::walk`], but also reports whether the last arc of the path
    /// is final (i.e. the exact key is an entry of the automaton).
    pub fn walk_final(&self, node: usize, key: &[u8]) -> Option<(usize, bool)> {
        let mut node = node;
        let mut last_final = false;
        for &label in key {
            let mut found = None;
            let mut arc = self.first_arc(node);
            while arc != 0 {
                if self.arc_label(arc) == label {
                    last_final = self.is_arc_final(arc);
                    found = Some(self.destination_node_offset(arc));
                    break;
                }
                arc = self.next_arc(arc);
            }
            node = found?;
        }
        Some((node, last_final))
    }

    /// Depth-first traversal of all sequences starting at `node` that end on
    /// final arcs, calling `f` with the accumulated suffix bytes.
    pub fn visit_sequences<F: FnMut(&[u8])>(&self, node: usize, f: &mut F) {
        let mut buf = Vec::new();
        self.visit(node, &mut buf, f);
    }

    /// Like [`Self::visit_sequences`], but only descends into arcs labeled
    /// `first` at `node`. Dictionary entries encode `word + sep + annotation`,
    /// so this visits the annotations of the word ending at `node` without
    /// walking the (potentially huge) subtree of longer words sharing the
    /// prefix. Arc order, and therefore sequence order, is preserved.
    pub fn visit_sequences_with_first<F: FnMut(&[u8])>(&self, node: usize, first: u8, f: &mut F) {
        let mut buf = vec![first];
        let mut arc = self.first_arc(node);
        while arc != 0 {
            if self.arc_label(arc) == first {
                if self.is_arc_final(arc) {
                    f(&buf);
                }
                let dest = self.destination_node_offset(arc);
                if dest != 0 {
                    self.visit(dest, &mut buf, f);
                }
            }
            arc = self.next_arc(arc);
        }
    }

    fn visit<F: FnMut(&[u8])>(&self, node: usize, buf: &mut Vec<u8>, f: &mut F) {
        let mut arc = self.first_arc(node);
        while arc != 0 {
            let label = self.arc_label(arc);
            buf.push(label);
            if self.is_arc_final(arc) {
                f(buf);
            }
            let dest = self.destination_node_offset(arc);
            if dest != 0 {
                self.visit(dest, buf, f);
            }
            buf.pop();
            arc = self.next_arc(arc);
        }
    }
}

/// Plain-text manual tagger (`org.languagetool.tagging.ManualTagger`):
/// tab-separated `fullform<TAB>baseform<TAB>postags`, `#` comments, optional
/// `#separatorRegExp=`. Used for `added.txt`/`removed.txt` and their custom
/// complements.
#[derive(Debug, Clone, Default)]
pub struct ManualTagger {
    map: HashMap<String, Vec<(String, String)>>,
}

impl ManualTagger {
    /// Load and concatenate the given files (missing files are skipped, like
    /// the empty resource streams in `BaseTagger.initWordTagger`).
    pub fn load(paths: &[&Path]) -> Result<Self> {
        let mut map: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut separator = "\t".to_string();
        for path in paths {
            let Ok(text) = lt_data::fs::read_to_string(path) else {
                continue;
            };
            for line in text.lines() {
                let line = line.trim();
                if line.starts_with("#separatorRegExp=") {
                    separator = line.replace("#separatorRegExp=", "");
                }
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let line = line.split('#').next().unwrap_or("").trim();
                if line.is_empty() {
                    continue;
                }
                let mut parts: Vec<&str> = line.split(separator.as_str()).collect();
                // Java `String.split` drops trailing empty strings (French
                // files end every entry with the separator).
                while parts.last() == Some(&"") {
                    parts.pop();
                }
                if parts.len() != 3 {
                    return Err(lt_core::CoreError::Data(format!(
                        "manual tagger: expected three fields in '{}' ({})",
                        line,
                        path.display()
                    )));
                }
                let form = parts[0].to_string();
                let lemma = parts[1].to_string();
                let tag = parts[2].trim().to_string();
                map.entry(form).or_default().push((lemma, tag));
            }
        }
        Ok(Self { map })
    }

    pub fn lookup(&self, word: &str) -> &[(String, String)] {
        self.map.get(word).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// A Morfologik dictionary: surface form → decoded readings (stem, tag),
/// decoded on demand from the automaton. The stored annotation is
/// suffix-encoded per the `.info` encoder (LT dictionaries use `SUFFIX`):
/// first byte is a trim code, then the stem remainder, separator, and tag.
///
/// The automaton is walked per lookup instead of being expanded into a
/// `HashMap` up front: the German `german.dict` expansion alone used ~745 MB
/// and several seconds of load time (D-033).
#[derive(Debug, Clone)]
pub struct Dictionary {
    automaton: Automaton,
    separator: u8,
    /// `.info` `fsa.dict.encoding` (ISO-8859-15 for Italian).
    charset: Charset,
    /// `.info` `fsa.dict.frequency-included`: the tag's last byte is a
    /// frequency marker (Java `MorfologikTagger` strips it).
    frequency_included: bool,
}

const REMOVE_EVERYTHING: i32 = 255;

impl Dictionary {
    /// Load `.dict` + `.info` pair.
    pub fn load(dict_path: &Path, info: &DictionaryInfo) -> Result<Self> {
        let bytes = lt_data::fs::read(dict_path).map_err(|e| {
            lt_core::CoreError::Data(format!("cannot read {}: {e}", dict_path.display()))
        })?;
        let automaton = Automaton::parse(&bytes)?;
        let separator = info
            .fields
            .get("fsa.dict.separator")
            .and_then(|s| s.chars().next())
            .unwrap_or('+')
            .to_string()
            .into_bytes()
            .pop()
            .unwrap_or(b'+');
        let frequency_included = info
            .fields
            .get("fsa.dict.frequency-included")
            .is_some_and(|v| v.eq_ignore_ascii_case("true"));
        let charset = match info.encoding() {
            Some(name) => Charset::from_info_name(name).ok_or_else(|| {
                lt_core::CoreError::Data(format!("unsupported dictionary encoding: {name}"))
            })?,
            None => Charset::Utf8,
        };
        Ok(Self {
            automaton,
            separator,
            charset,
            frequency_included,
        })
    }

    /// All (stem, tag) readings for `word` (surface match only), in automaton
    /// (arc) order like morfologik's `DictionaryLookup`.
    pub fn lookup(&self, word: &str) -> Vec<(String, String)> {
        // `DictionaryLookup.lookup`: a word containing the separator can never
        // be a dictionary entry; an unmappable char means no possible hit.
        if word.contains(self.separator as char) {
            return Vec::new();
        }
        let Some(word_bytes) = self.charset.encode(word) else {
            return Vec::new();
        };
        let Some(node) = self.automaton.walk(self.automaton.root_node(), &word_bytes) else {
            return Vec::new();
        };
        let separator = self.separator;
        let charset = self.charset;
        let mut out = Vec::new();
        // Only the separator-labeled arcs at the word node carry this word's
        // annotations; descending into letter arcs would walk the subtree of
        // all longer words sharing the prefix (the pre-D-046 code did that and
        // dominated the German tagger).
        self.automaton
            .visit_sequences_with_first(node, separator, &mut |seq: &[u8]| {
                decode_annotation(&word_bytes, &seq[1..], separator, charset, &mut out);
            });
        if self.frequency_included {
            for (_, tag) in &mut out {
                if tag.len() > 1 {
                    tag.pop();
                }
            }
        }
        out
    }
}

/// Decode one `[trimCode][stem remainder][sep][tag]` annotation: the trim
/// counts *bytes* and the stem is the byte concatenation of the trimmed
/// surface and the encoded suffix (`TrimSuffixEncoder.decode`), so the
/// concatenation must happen before UTF-8 decoding (German umlauts split
/// across the boundary).
fn decode_annotation(
    surface_bytes: &[u8],
    ann: &[u8],
    separator: u8,
    charset: Charset,
    out: &mut Vec<(String, String)>,
) {
    if ann.is_empty() {
        return;
    }
    let Some(tag_sep) = ann[1..].iter().position(|&b| b == separator).map(|p| p + 1) else {
        return;
    };
    let trim_raw = ((ann[0] as i32) - (b'A' as i32)) & 0xFF;
    let trim = if trim_raw == REMOVE_EVERYTHING {
        surface_bytes.len()
    } else {
        trim_raw as usize
    };
    if trim > surface_bytes.len() {
        return;
    }
    let mut stem_bytes = surface_bytes[..surface_bytes.len() - trim].to_vec();
    stem_bytes.extend_from_slice(&ann[1..tag_sep]);
    out.push((
        charset.decode(&stem_bytes).into_owned(),
        charset.decode(&ann[tag_sep + 1..]).into_owned(),
    ));
}

/// Morfologik synthesizer dictionary (`BaseSynthesizer`/`DictionaryLookup`):
/// the automaton spells `<lemma>|<tag>+<trimCode><suffix>` (SUFFIX encoder).
/// `lookup("lemma|tag")` decodes the inflected forms.
#[derive(Debug, Clone)]
pub struct SynthDictionary {
    automaton: Automaton,
    separator: u8,
    charset: Charset,
}

const REMOVE_EVERYTHING_CODE: i32 = 255;

impl SynthDictionary {
    pub fn load(dict_path: &Path, info: &DictionaryInfo) -> Result<Self> {
        let bytes = lt_data::fs::read(dict_path).map_err(|e| {
            lt_core::CoreError::Data(format!("cannot read {}: {e}", dict_path.display()))
        })?;
        let automaton = Automaton::parse(&bytes)?;
        let separator = info
            .fields
            .get("fsa.dict.separator")
            .and_then(|s| s.chars().next())
            .unwrap_or('+') as u8;
        let charset = match info.encoding() {
            Some(name) => Charset::from_info_name(name).ok_or_else(|| {
                lt_core::CoreError::Data(format!("unsupported dictionary encoding: {name}"))
            })?,
            None => Charset::Utf8,
        };
        Ok(Self {
            automaton,
            separator,
            charset,
        })
    }

    /// All inflected forms for the exact `lemma|tag` key, in automaton
    /// (arc) order like morfologik's `DictionaryLookup`.
    pub fn lookup(&self, key: &str) -> Vec<String> {
        if key.contains(self.separator as char) {
            return Vec::new();
        }
        let Some(key_bytes) = self.charset.encode(key) else {
            return Vec::new();
        };
        let Some(node) = self.automaton.walk(self.automaton.root_node(), &key_bytes) else {
            return Vec::new();
        };
        let separator = self.separator;
        let charset = self.charset;
        let mut out = Vec::new();
        self.automaton
            .visit_sequences_with_first(node, separator, &mut |seq: &[u8]| {
                if seq.len() < 2 {
                    return;
                }
                let trim_raw = (seq[1] as i32) - (b'A' as i32);
                let trim = if trim_raw == REMOVE_EVERYTHING_CODE {
                    key_bytes.len()
                } else if trim_raw < 0 {
                    return;
                } else {
                    trim_raw as usize
                };
                if trim > key_bytes.len() {
                    return;
                }
                // byte concatenation first (`TrimSuffixEncoder.decode`), decode after
                let mut stem_bytes = key_bytes[..key_bytes.len() - trim].to_vec();
                stem_bytes.extend_from_slice(&seq[2..]);
                out.push(charset.decode(&stem_bytes).into_owned());
            });
        out
    }
}

/// Parsed `.info` metadata of a Morfologik dictionary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DictionaryInfo {
    pub fields: HashMap<String, String>,
}

impl DictionaryInfo {
    pub fn parse(text: &str) -> Result<Self> {
        let mut fields = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                fields.insert(k.trim().to_string(), unescape_properties_value(v.trim()));
            }
        }
        Ok(Self { fields })
    }

    pub fn encoding(&self) -> Option<&str> {
        self.fields.get("fsa.dict.encoding").map(|s| s.as_str())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = lt_data::fs::read_to_string(path).map_err(|e| {
            lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display()))
        })?;
        Self::parse(&text)
    }
}

/// `java.util.Properties`-style value unescaping: morfologik's
/// `DictionaryMetadata` loads the `.info` file with `Properties`, so
/// `\uXXXX` escapes (Catalan's `equivalent-chars=s \u00E7` and
/// `replacement-pairs`) arrive decoded.
fn unescape_properties_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(decoded) => out.push(decoded),
                    None => {
                        out.push('\\');
                        out.push('u');
                        out.push_str(&hex);
                    }
                }
            }
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\u{c}'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    fn en_dict() -> Option<Dictionary> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/dictionaries");
        if !dir.lt_exists() {
            return None;
        }
        let info = DictionaryInfo::load(&dir.join("english.info")).unwrap();
        Some(Dictionary::load(&dir.join("english.dict"), &info).unwrap())
    }

    #[test]
    fn parses_cfsa2_header() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/dictionaries");
        if !dir.lt_exists() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let bytes = lt_data::fs::read(dir.join("english_synth.dict")).unwrap();
        let automaton = Cfsa2::parse(&bytes).unwrap();
        assert!(automaton.root_node() > 0);
    }

    #[test]
    fn english_dictionary_expands_and_tags() {
        let Some(dict) = en_dict() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        assert!(!dict.lookup("walk").is_empty(), "english.dict missing walk");
        // Common word must exist with noun/verb readings
        let walk = dict.lookup("walk");
        let tags: Vec<&str> = walk.iter().map(|(_, t)| t.as_str()).collect();
        assert!(tags.contains(&"VB"), "walk tags: {tags:?}");
        assert!(tags.contains(&"NN"), "walk tags: {tags:?}");
        assert!(dict.lookup("runs").iter().any(|(_, t)| t == "VBZ"));
        // suffix-encoded stems decode to the lemma
        assert!(dict
            .lookup("wailers")
            .iter()
            .any(|(s, t)| s == "wailer" && t == "NNS"));
        assert!(dict
            .lookup("wariest")
            .iter()
            .any(|(s, t)| s == "wary" && t == "JJS"));
        assert!(dict.lookup("notawordxyz").is_empty());
    }

    #[test]
    fn parses_morfologik_info() {
        let info =
            DictionaryInfo::parse("fsa.dict.utf8=cfsgh\nfsa.dict.encoding=UTF-8\n\n# comment\n")
                .unwrap();
        assert_eq!(info.encoding(), Some("UTF-8"));
    }
}

#[cfg(test)]
mod probe_resources {
    use super::*;
    use lt_data::PathExt as _;
    #[test]
    fn probe() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/dictionaries");
        if !dir.lt_exists() {
            return;
        }
        let info = DictionaryInfo::load(&dir.join("english.info")).unwrap();
        let dict = Dictionary::load(&dir.join("english.dict"), &info).unwrap();
        println!("resources -> {:?}", dict.lookup("resources"));
        println!("resource -> {:?}", dict.lookup("resource"));
        // raw sequences behind the surface prefix
        let bytes = lt_data::fs::read(dir.join("english_synth.dict")).unwrap();
        let aut = Cfsa2::parse(&bytes).unwrap();
        for w in ["resources", "resource", "walk", "runs"] {
            if let Some(node) = aut.walk(aut.root_node(), w.as_bytes()) {
                let mut seqs = Vec::new();
                aut.visit_sequences(node, &mut |seq| {
                    seqs.push(String::from_utf8_lossy(seq).into_owned());
                });
                println!("{w}: node={node} seqs={seqs:?}");
            } else {
                println!("{w}: no node");
            }
        }
    }
}
