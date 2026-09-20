//! SRX (Segmentation Rules eXchange) sentence splitting, mirroring the
//! algorithm of `net.loomchild.segment`'s `SrxTextIterator` as used by
//! The legacy `SRXSentenceTokenizer`.
//!
//! Break rules are found by searching `beforebreak`, then anchoring
//! `afterbreak` at the match end. No-break (`break="no"`) rules act as
//! exception guards at each break position, emulating Java's lookbehind by
//! precomputing where `beforebreak` matches end.

use std::collections::HashMap;

use fancy_regex::Regex as FancyRegex;
use lt_core::Result;
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Clone)]
pub struct SrxRule {
    pub is_break: bool,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Default)]
pub struct SrxDocument {
    /// named rule groups
    pub language_rules: HashMap<String, Vec<SrxRule>>,
    /// language pattern regex → rule group name, in document order
    pub language_map: Vec<(String, String)>,
    pub cascade: bool,
}

impl SrxDocument {
    pub fn load_file(path: &std::path::Path) -> Result<Self> {
        parse_srx(path)
    }

    /// Rule groups selected for `language_code` (cascade semantics: every
    /// matching languagemap contributes its rules, in document order).
    ///
    /// Codes with no language-specific mapping — the hand-authored Guaraní,
    /// Norwegian and Nordum modules pass `gn_two`/`no_two`/`nrd_two` — only
    /// match the `.*` maps, whose `Default` group has no plain ". " break
    /// rule (the legacy engine maps such languages to `Generic`). Prepend the generic
    /// break rules so sentences split like in the ported languages.
    pub fn rules_for(&self, language_code: &str) -> Vec<SrxRule> {
        // groups that every language gets from the `.*`/`_one`/`_two` maps
        const GENERIC_GROUPS: [&str; 4] = [
            "Default",
            "GeneralImportant",
            "ByLineBreak",
            "ByTwoLineBreaks",
        ];
        let mut rules = Vec::new();
        let mut specific_match = false;
        for (pattern, name) in &self.language_map {
            let matches = FancyRegex::new(pattern)
                .ok()
                .and_then(|r| r.is_match(language_code).ok())
                .unwrap_or(false);
            if matches {
                if !GENERIC_GROUPS.contains(&name.as_str()) {
                    specific_match = true;
                }
                if let Some(group) = self.language_rules.get(name) {
                    rules.extend(group.iter().cloned());
                }
                if !self.cascade {
                    break;
                }
            }
        }
        if !specific_match {
            if let Some(generic) = self.language_rules.get("Generic") {
                let mut with_generic = generic.clone();
                with_generic.extend(rules);
                return with_generic;
            }
        }
        rules
    }
}

fn parse_srx(path: &std::path::Path) -> Result<SrxDocument> {
    let file = lt_data::fs::open(path)
        .map_err(|e| lt_core::CoreError::Data(format!("cannot open {}: {e}", path.display())))?;
    let mut reader = Reader::from_reader(std::io::BufReader::new(file));
    // Significant leading/trailing whitespace in before/afterbreak patterns
    // (e.g. `[\[\(]*\.\.\.[\]\)]* `) must survive, matching the StAX
    // parser's `getElementText()`.
    reader.config_mut().trim_text(false);

    let mut doc = SrxDocument::default();
    let mut capture: Option<CaptureKind> = None;
    let mut buf = String::new();
    let mut current_rule: Option<SrxRule> = None;
    let mut current_group: Option<String> = None;

    enum CaptureKind {
        Before,
        After,
    }

    loop {
        let mut raw = Vec::new();
        match reader.read_event_into(&mut raw) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match name.as_str() {
                    "header" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"cascade" {
                                doc.cascade = attr_value(&attr) == "yes";
                            }
                        }
                    }
                    "languagerule" => {
                        current_group = None;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"languagerulename" {
                                current_group = Some(attr_value(&attr));
                            }
                        }
                    }
                    "languagemap" => push_language_map(&e, &mut doc),
                    "rule" => {
                        let mut is_break = false;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"break" {
                                is_break = attr_value(&attr) == "yes";
                            }
                        }
                        current_rule = Some(SrxRule {
                            is_break,
                            before: String::new(),
                            after: String::new(),
                        });
                    }
                    "beforebreak" => capture = Some(CaptureKind::Before),
                    "afterbreak" => capture = Some(CaptureKind::After),
                    _ => {}
                }
                buf.clear();
            }
            Ok(Event::Text(t)) if capture.is_some() => {
                buf.push_str(&t.unescape().unwrap_or_default());
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match name.as_str() {
                    "beforebreak" => {
                        if let Some(CaptureKind::Before) = capture {
                            if let Some(rule) = current_rule.as_mut() {
                                rule.before = normalize_java_regex(&buf);
                            }
                            capture = None;
                            buf.clear();
                        }
                    }
                    "afterbreak" => {
                        if let Some(CaptureKind::After) = capture {
                            if let Some(rule) = current_rule.as_mut() {
                                rule.after = normalize_java_regex(&buf);
                            }
                            capture = None;
                            buf.clear();
                        }
                    }
                    "rule" => {
                        if let (Some(rule), Some(group)) = (current_rule.take(), &current_group) {
                            doc.language_rules
                                .entry(group.clone())
                                .or_default()
                                .push(rule);
                        }
                    }
                    "languagerule" => current_group = None,
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if name == "languagemap" {
                    push_language_map(&e, &mut doc);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(lt_core::CoreError::Parse(
                    path.display().to_string(),
                    e.to_string(),
                ))
            }
            _ => {}
        }
    }
    Ok(doc)
}

fn attr_value(attr: &quick_xml::events::attributes::Attribute<'_>) -> String {
    attr.unescape_value()
        .map(|s| s.into_owned())
        .unwrap_or_default()
}

fn push_language_map(e: &quick_xml::events::BytesStart<'_>, doc: &mut SrxDocument) {
    let mut pattern = String::new();
    let mut rule_name = String::new();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"languagepattern" => pattern = attr_value(&attr),
            b"languagerulename" => rule_name = attr_value(&attr),
            _ => {}
        }
    }
    doc.language_map.push((pattern, rule_name));
}

/// Convert Java-regex-only constructs used by segment.srx into
/// fancy-regex/regex-crate syntax (`\uXXXX` → `\u{XXXX}`).
pub fn normalize_java_regex(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len() + 8);
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push_str(&format!("\\u{{{}}}", hex));
                }
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

struct Cursor {
    search_from: usize,
    break_pos: Option<usize>,
}

/// Compiled, cascaded rule set for one language.
#[derive(Debug)]
pub struct SrxTokenizer {
    break_rules: Vec<CompiledBreakRule>,
    /// compiled no-break rules (before, after) guarding break rules
    no_break_compiled: Vec<(FancyRegex, FancyRegex)>,
}

#[derive(Debug)]
struct CompiledBreakRule {
    before: FancyRegex,
    after: FancyRegex,
    /// indices into `no_break_compiled` that guard this break rule
    exceptions: Vec<usize>,
}

impl SrxTokenizer {
    pub fn new(doc: &SrxDocument, language_code: &str) -> Result<Self> {
        let mut pending_no_break: Vec<(String, String)> = Vec::new();
        let mut break_rules = Vec::new();
        for rule in doc.rules_for(language_code) {
            if rule.is_break {
                let exceptions: Vec<usize> = (0..pending_no_break.len()).collect();
                break_rules.push(CompiledBreakRule {
                    before: FancyRegex::new(&rule.before).map_err(|e| {
                        lt_core::CoreError::Parse("srx-before".into(), e.to_string())
                    })?,
                    after: FancyRegex::new(&rule.after).map_err(|e| {
                        lt_core::CoreError::Parse("srx-after".into(), e.to_string())
                    })?,
                    exceptions,
                });
            } else {
                pending_no_break.push((rule.before.clone(), rule.after.clone()));
            }
        }
        let mut no_break_compiled = Vec::new();
        for (before, after) in pending_no_break {
            let re_before = FancyRegex::new(&before)
                .map_err(|e| lt_core::CoreError::Parse("srx-before".into(), e.to_string()))?;
            let re_after = FancyRegex::new(&after)
                .map_err(|e| lt_core::CoreError::Parse("srx-after".into(), e.to_string()))?;
            no_break_compiled.push((re_before, re_after));
        }
        // the segment library applies `break="no"` rules as global exceptions
        // at a break position, regardless of which cascaded group the break
        // rule comes from (e.g. the English abbreviation rules must suppress
        // breaks from the GeneralImportant group)
        let all_exceptions: Vec<usize> = (0..no_break_compiled.len()).collect();
        for rule in &mut break_rules {
            rule.exceptions = all_exceptions.clone();
        }
        Ok(Self {
            break_rules,
            no_break_compiled,
        })
    }

    /// Split `text` into sentence spans (byte offsets, non-empty).
    pub fn split(&self, text: &str) -> Vec<(usize, usize)> {
        // precompute where each no-break `before` matches ends (Java
        // lookbehind emulation)
        let no_break_ends: Vec<std::collections::HashSet<usize>> = self
            .no_break_compiled
            .iter()
            .map(|(before, _)| {
                // lookbehind semantics: a `beforebreak` may match at *any*
                // start ending at the break position, including overlapping
                // matches (`find_iter` would skip "O.P." after "C.O.P.")
                let mut ends = std::collections::HashSet::new();
                let mut from = 0usize;
                while let Ok(Some(m)) = before.find_from_pos(text, from) {
                    ends.insert(m.end());
                    from = m.start() + 1;
                }
                ends
            })
            .collect();
        let mut segments = Vec::new();
        let mut cursors: Vec<Cursor> = self
            .break_rules
            .iter()
            .map(|_| Cursor {
                search_from: 0,
                break_pos: None,
            })
            .collect();
        for (i, rule) in self.break_rules.iter().enumerate() {
            advance(rule, text, &mut cursors[i]);
        }

        let mut start = 0usize;
        loop {
            let mut min_pos = usize::MAX;
            let mut min_idx: Option<usize> = None;
            for (i, cursor) in cursors.iter().enumerate() {
                if let Some(pos) = cursor.break_pos {
                    if pos < min_pos {
                        min_pos = pos;
                        min_idx = Some(i);
                    }
                }
            }
            let end = match min_idx {
                None => text.len(),
                Some(i) => {
                    let break_pos = cursors[i].break_pos.unwrap();
                    let mut accept = break_pos > start;
                    if accept
                        && break_pos < text.len()
                        && self.is_exception(i, break_pos, text, &no_break_ends)
                    {
                        accept = false;
                    }
                    // the segment library never emits a blank segment without
                    // a line break ("A. \n\n  B."): the spaces belong to the
                    // next sentence; verified against `SrxTextIterator`
                    if accept && is_blank_without_linebreak(&text[start..break_pos]) {
                        accept = false;
                    }
                    if accept {
                        for (j, cursor) in cursors.iter_mut().enumerate() {
                            if cursor.break_pos.is_some_and(|p| p <= break_pos) {
                                advance(&self.break_rules[j], text, cursor);
                            }
                        }
                        break_pos
                    } else {
                        advance(&self.break_rules[i], text, &mut cursors[i]);
                        continue;
                    }
                }
            };
            if start < end {
                segments.push((start, end));
            }
            start = end;
            if start >= text.len() {
                break;
            }
        }
        segments
    }

    fn is_exception(
        &self,
        break_rule_idx: usize,
        pos: usize,
        text: &str,
        no_break_ends: &[std::collections::HashSet<usize>],
    ) -> bool {
        for &idx in &self.break_rules[break_rule_idx].exceptions {
            let ends = &no_break_ends[idx];
            let (_, after) = &self.no_break_compiled[idx];
            if ends.contains(&pos)
                && after
                    .find_from_pos(text, pos)
                    .ok()
                    .flatten()
                    .is_some_and(|m| m.start() == pos)
            {
                return true;
            }
        }
        false
    }
}

/// True for a non-empty segment that is all whitespace and contains no line
/// break (`SrxTextIterator` merges those into the following segment).
fn is_blank_without_linebreak(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.contains('\n')
        && !segment.contains('\r')
        && segment.chars().all(char::is_whitespace)
}

/// Find the next break candidate for `rule`, resuming from the cursor's
/// search position (Java `Matcher.find()` progress semantics: zero-width
/// matches advance by one char).
fn advance(rule: &CompiledBreakRule, text: &str, cursor: &mut Cursor) {
    let mut search_from = cursor.search_from;
    loop {
        let Ok(Some(m)) = rule.before.find_from_pos(text, search_from) else {
            cursor.break_pos = None;
            return;
        };
        let next_from = std::cmp::max(m.end(), m.start() + 1);
        let after_here = rule
            .after
            .find_from_pos(text, m.end())
            .ok()
            .flatten()
            .is_some_and(|am| am.start() == m.end());
        if after_here {
            cursor.search_from = next_from;
            cursor.break_pos = Some(m.end());
            return;
        }
        search_from = next_from;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;
    use std::path::Path;

    fn en_tokenizer() -> Option<SrxTokenizer> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/core/segment.srx");
        if !dir.lt_exists() {
            return None;
        }
        let doc = SrxDocument::load_file(&dir).unwrap();
        Some(SrxTokenizer::new(&doc, "en_two").unwrap())
    }

    #[test]
    fn de_merges_blank_segments_without_linebreak_into_next_sentence() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/core/segment.srx");
        if !path.lt_exists() {
            eprintln!("skipping: no vendored segment.srx");
            return;
        }
        let doc = SrxDocument::load_file(&path).unwrap();
        let tok = SrxTokenizer::new(&doc, "de_two").unwrap();
        // verified against the segment library / pinned German LT
        // (`ProbeSentences.java`)
        let text = "Ein Absatz.\n\n  Leerzeichen am Anfang.\n";
        let segs: Vec<&str> = tok.split(text).iter().map(|(s, e)| &text[*s..*e]).collect();
        assert_eq!(segs, vec!["Ein Absatz.\n\n", "  Leerzeichen am Anfang.\n"]);
        // a blank segment *with* linebreaks stays with the previous sentence
        let text = "One.\n\n  \n\nTwo.\n";
        let segs: Vec<&str> = tok.split(text).iter().map(|(s, e)| &text[*s..*e]).collect();
        assert_eq!(segs, vec!["One.\n\n  \n\n", "Two.\n"]);
        let _ = tok;
    }

    #[test]
    fn unmapped_language_gets_generic_sentence_break() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/core/segment.srx");
        if !path.lt_exists() {
            eprintln!("skipping: no vendored segment.srx");
            return;
        }
        let doc = SrxDocument::load_file(&path).unwrap();
        // `nrd_two` has no language-specific languagemap (only line breaks)
        let tok = SrxTokenizer::new(&doc, "nrd_two").unwrap();
        let text = "Det er bra. Norsk og Spania er bra.";
        assert_eq!(tok.split(text).len(), 2, "{:?}", tok.split(text));
    }

    #[test]
    fn srx_splits_basic_sentences() {
        let Some(tok) = en_tokenizer() else {
            eprintln!("skipping: no vendored segment.srx");
            return;
        };
        let text = "This is a test. And another one! Right?";
        let spans = tok.split(text);
        let segs: Vec<&str> = spans.iter().map(|(s, e)| &text[*s..*e]).collect();
        assert_eq!(
            segs,
            vec!["This is a test. ", "And another one! ", "Right?"]
        );
    }

    #[test]
    fn srx_respects_abbreviations() {
        let Some(tok) = en_tokenizer() else {
            eprintln!("skipping: no vendored segment.srx");
            return;
        };
        let text = "Dr. Smith arrived. He was e.g. late.";
        let spans = tok.split(text);
        let segs: Vec<&str> = spans.iter().map(|(s, e)| &text[*s..*e]).collect();
        assert_eq!(segs.len(), 2, "segments: {segs:?}");
    }

    #[test]
    fn srx_handles_newline_paragraphs_for_en_two() {
        let Some(tok) = en_tokenizer() else {
            eprintln!("skipping: no vendored segment.srx");
            return;
        };
        let text = "First line.\nSecond line.";
        let spans = tok.split(text);
        assert!(spans.len() >= 2, "spans: {spans:?}");
    }
}

#[cfg(test)]
mod debug_tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn debug_rules() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/core/segment.srx");
        let doc = SrxDocument::load_file(&dir).unwrap();
        eprintln!(
            "groups: {:?}",
            doc.language_rules.keys().collect::<Vec<_>>()
        );
        eprintln!("maps: {:?}", doc.language_map);
        let rules = doc.rules_for("en_two");
        eprintln!(
            "en_two rules: {} (break={})",
            rules.len(),
            rules.iter().filter(|r| r.is_break).count()
        );
        for r in rules.iter().take(3) {
            eprintln!(
                "  break={} before={:?} after={:?}",
                r.is_break, r.before, r.after
            );
        }
    }
}
