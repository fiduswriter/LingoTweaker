//! Pattern matching v1 over analyzed token streams.
//!
//! Supports the common LT pattern vocabulary: text/regexp tokens, postag
//! (+regexp), negate/negate_pos, inflected, case_sensitive, skip/min/max
//! gaps, exceptions (scopes current/next/previous), markers, `<and>` groups
//! (compiled as `CompiledToken::and_group`), `<or>` groups (expanded into one
//! `CompiledPattern` per alternative, like LT's rule loader), and literal
//! `<suggestion>` outputs with `<match no>` references. Rules using
//! unification or phrases, `<filter>` classes, or chunk attributes are out of
//! scope for v1 (see `RuleDef::complex_pattern` / `has_filter` /
//! `has_chunk_attrs`).

use fancy_regex::Regex as FancyRegex;
use std::ops::Deref;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, TextRange};

use crate::string_matcher;

use crate::{MatchRefSpec, MessageMatchRef, PatternToken, SuggestionPart};

/// Upper bound for `skip="-1"` (unlimited) gaps.
const MAX_GAP: i32 = 30;

pub struct CompiledPattern {
    pub tokens: Vec<CompiledToken>,
    pub marker_start: Option<usize>,
    pub marker_end: Option<usize>,
    /// minimum number of tokens a match consumes
    /// (`AbstractPatternRulePerformer.getMinOccurrenceCorrection`): elements
    /// with `min="0"` consume none.
    pub min_len: usize,
    /// any element takes part in unification (LT `isTestUnification`)
    pub has_unification: bool,
    /// `PatternRuleMatcher.testAllReadings`: immunized tokens never match
    /// (set for rule patterns; antipatterns ignore immunity like Java's
    /// disambiguation matcher)
    pub skip_immunized: bool,
    /// Java `AbstractTokenBasedRule.anchorHint`: only these match starts are
    /// tried (set when every element before the hint has default min/max/skip)
    pub anchor: Option<CompiledHint>,
}

/// Compiled `<match no="N">` reference: the element's text is the template
/// with `\N` replaced by the referenced token's (case-converted) surface.
#[derive(Debug, Clone)]
struct CompiledMatchRef {
    no: usize,
    template: String,
    case_conversion: Option<String>,
    setpos: bool,
    postag: Option<String>,
}

#[derive(Clone)]
pub struct CompiledToken {
    pub text: Option<std::sync::Arc<TextRegex>>,
    /// the token has a text matcher at all (`hasStringThatMustMatch`)
    pub(crate) has_text: bool,
    /// fast path for non-regexp text matchers: literal + case_sensitive
    pub(crate) literal: Option<(String, bool)>,
    /// fast path for regexps whose complete value set is known
    /// (`StringMatcher.getPossibleRegexpValues`); values are lowercased when
    /// the matcher is case-insensitive
    pub(crate) value_set: Option<(std::collections::HashSet<String>, bool)>,
    negate: bool,
    pub postag: Option<std::sync::Arc<TextRegex>>,
    /// raw `postag` attribute (Java `PatternToken.getPOStag`; the
    /// disambiguation `filterall` fallback stores the literal selector tag)
    pub postag_source: Option<String>,
    /// `postag` matches the special `UNKNOWN` tag (precomputed)
    pos_unknown: bool,
    negate_pos: bool,
    inflected: bool,
    gap_min: i32,
    gap_max: i32,
    /// element itself may match zero tokens (LT min="0")
    optional: bool,
    /// LT `maxOccurrence` (1 = single token; the element is matched as a
    /// consecutive run of at most this many tokens)
    max_occurrence: i32,

    /// chunk tag requirement (`chunk` / `chunk_re`, D-002)
    chunk: Option<(String, bool)>,
    /// LT `isInsideMarker` (needed by `estimateContextForSureMatch`)
    pub in_marker: bool,
    /// LT `skip` attribute (-1 = unbounded)
    skip: i32,
    /// pattern postag is SENTENCE_END (needed by the context estimate)
    sentence_end_postag: bool,
    exceptions: Vec<CompiledException>,
    /// LT `spacebefore` test (`None` = ignore)
    spacebefore: Option<bool>,
    /// `<match no="N">` reference (resolved per match attempt)
    match_ref: Option<CompiledMatchRef>,
    /// the token's text matcher was built from a regexp (`regexp="yes"`)
    regexp: bool,
    case_sensitive: bool,
    /// LT `andGroupList`: all members must match the same token position
    /// (each may match a different reading)
    pub and_group: Vec<CompiledToken>,
    /// LT `unificationFeatures` (`<unify>`); `None` = no unification
    pub unification: Option<std::collections::BTreeMap<String, Vec<String>>>,
    pub uni_negated: bool,
    pub last_in_unification: bool,
    pub unification_neutral: bool,
}

#[derive(Clone)]
pub struct CompiledException {
    pub text: Option<std::sync::Arc<TextRegex>>,
    /// the exception has a text matcher at all
    pub(crate) has_text: bool,
    /// fast path for non-regexp text matchers: literal + case_sensitive
    pub(crate) literal: Option<(String, bool)>,
    /// fast path for regexps whose complete value set is known
    pub(crate) value_set: Option<(std::collections::HashSet<String>, bool)>,
    pub postag: Option<std::sync::Arc<TextRegex>>,
    /// `postag` matches the special `UNKNOWN` tag (precomputed)
    pos_unknown: bool,
    negate: bool,
    negate_pos: bool,
    inflected: bool,
    spacebefore: Option<bool>,
    pub scope: String,
}

/// Text/postag matcher: the `regex` crate when the pattern is supported
/// (fast, same leftmost-first semantics), else `fancy-regex` for lookaround
/// and backreferences.
pub enum TextRegex {
    Fast(regex::Regex),
    Fancy(FancyRegex),
}

impl TextRegex {
    pub fn is_match(&self, s: &str) -> bool {
        match self {
            TextRegex::Fast(re) => re.is_match(s),
            TextRegex::Fancy(re) => re.is_match(s).unwrap_or(false),
        }
    }
}

impl Clone for TextRegex {
    fn clone(&self) -> Self {
        match self {
            TextRegex::Fast(re) => TextRegex::Fast(re.clone()),
            TextRegex::Fancy(re) => TextRegex::Fancy(re.clone()),
        }
    }
}

impl std::fmt::Debug for TextRegex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextRegex::Fast(re) => f.debug_tuple("Fast").field(re).finish(),
            TextRegex::Fancy(re) => f.debug_tuple("Fancy").field(re).finish(),
        }
    }
}

/// Compiled regexes are interned by `(pattern, case_sensitive, anchored)`:
/// rule tokens repeat the same patterns (POS patterns like `NN.*` appear in
/// thousands of rules) and each `regex` program costs a few KB, so the
/// duplicate programs used to add up to hundreds of MB (D-033).
type RegexCache =
    std::sync::Mutex<std::collections::HashMap<(String, bool, bool), std::sync::Arc<TextRegex>>>;

fn regex_cache() -> &'static RegexCache {
    static CACHE: std::sync::OnceLock<RegexCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn compile_regex_fast(
    pattern: &str,
    case_sensitive: bool,
    anchored: bool,
) -> Result<std::sync::Arc<TextRegex>, String> {
    let key = (pattern.to_string(), case_sensitive, anchored);
    if let Ok(cache) = regex_cache().lock() {
        if let Some(found) = cache.get(&key) {
            return Ok(std::sync::Arc::clone(found));
        }
    }
    let compiled = std::sync::Arc::new(compile_regex_uncached(pattern, case_sensitive, anchored)?);
    if let Ok(mut cache) = regex_cache().lock() {
        cache.insert(key, std::sync::Arc::clone(&compiled));
    }
    Ok(compiled)
}

fn compile_regex_uncached(
    pattern: &str,
    case_sensitive: bool,
    anchored: bool,
) -> Result<TextRegex, String> {
    let pattern = normalize_java_surrogate_escapes(&drop_quantified_anchors(pattern));
    let pattern = normalize_java_octal_escapes(&pattern);
    // Java allows a literal `-` right after a class escape or nested class
    // inside `[...]` (`[\p{Punct}-…&&[^!\.]]`, Polish `interp`); the Rust
    // regex crate reads it as an invalid range start.
    let pattern = escape_class_hyphens(&pattern);
    // Java inline flag groups such as `(?-iu)` cannot be left as-is: the Rust
    // regex crate cannot disable Unicode mode (Polish `DNI_TYGODNIA`).
    let pattern = strip_java_unicode_flags(&pattern);
    // Java accepts `(?-)` as a flag reset with no flags (used by the French
    // rules as `(?-)[A-Z]`); the Rust engines reject the empty flag list.
    let pattern = pattern.replace("(?-)", "");
    let mut s = String::new();
    if !case_sensitive {
        s.push_str("(?i)");
    }
    if anchored {
        s.push_str("^(?:");
        s.push_str(&pattern);
        s.push_str(")$");
    } else {
        s.push_str(&pattern);
    }
    match regex::Regex::new(&s) {
        Ok(re) => Ok(TextRegex::Fast(re)),
        Err(_) => FancyRegex::new(&s)
            .map(TextRegex::Fancy)
            .map_err(|e| e.to_string()),
    }
}

/// Java regex patterns spell astral code points as two adjacent `\uXXXX`
/// surrogate escapes and the JDK `Pattern` merges them into one code point
/// (`[\ud83d\udc00-\ud83d\udfff]` is U+1F000-U+1F7FF; probed against the
/// pinned build). The Rust regex crate rejects lone surrogates, so merge the
/// pairs into `\u{...}` escapes here. `\\` (an escaped backslash) is left
/// untouched.
fn normalize_java_surrogate_escapes(pattern: &str) -> String {
    fn hex4(bytes: &[u8]) -> Option<u32> {
        if bytes.len() != 4 || !bytes.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        std::str::from_utf8(bytes)
            .ok()
            .and_then(|s| u32::from_str_radix(s, 16).ok())
    }
    let bytes = pattern.as_bytes();
    let mut out = String::with_capacity(pattern.len());
    let mut i = 0usize;
    while i < pattern.len() {
        if bytes[i] == b'\\' {
            if bytes.get(i + 1) == Some(&b'\\') {
                out.push('\\');
                out.push('\\');
                i += 2;
                continue;
            }
            if bytes.get(i + 1) == Some(&b'u') && i + 6 <= pattern.len() {
                if let Some(value) = hex4(&bytes[i + 2..i + 6]) {
                    if (0xD800..=0xDBFF).contains(&value)
                        && bytes.get(i + 6) == Some(&b'\\')
                        && bytes.get(i + 7) == Some(&b'u')
                        && i + 12 <= pattern.len()
                    {
                        if let Some(low) = hex4(&bytes[i + 8..i + 12]) {
                            if (0xDC00..=0xDFFF).contains(&low) {
                                let code_point =
                                    0x10000 + ((value - 0xD800) << 10) + (low - 0xDC00);
                                out.push_str(&format!("\\u{{{code_point:X}}}"));
                                i += 12;
                                continue;
                            }
                        }
                    }
                    out.push_str(&pattern[i..i + 6]);
                    i += 6;
                    continue;
                }
            }
        }
        let ch = pattern[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Java allows a literal `-` right after a character-class escape or a nested
/// class inside a character class (`[\d-–]`, `[\p{Lu}-–]`); the Rust regex
/// crate reads it as an invalid range start. Escape such hyphens. A `}` that
/// closes a `\u{…}`/`\x{…}` code point is *not* a class end (the hyphen there
/// forms a valid range, e.g. `[\u{1F000}-\u{1F3FF}]`).
fn escape_class_hyphens(pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len());
    let mut in_class = 0usize;
    let mut i = 0usize;
    // the previous consumed item was a character-class escape (`\p{..}`,
    // `\d`, `\w`, `\s`, …) or a nested class, so a following `-` is literal
    let mut last_is_class_end = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            match chars.get(i + 1).copied() {
                Some(kind @ ('p' | 'P')) => {
                    out.push('\\');
                    out.push(kind);
                    i += 2;
                    if i < chars.len() && chars[i] == '{' {
                        while i < chars.len() {
                            out.push(chars[i]);
                            if chars[i] == '}' {
                                i += 1;
                                break;
                            }
                            i += 1;
                        }
                    }
                    last_is_class_end = true;
                    continue;
                }
                Some(kind @ ('d' | 'D' | 'w' | 'W' | 's' | 'S')) => {
                    out.push('\\');
                    out.push(kind);
                    i += 2;
                    last_is_class_end = true;
                    continue;
                }
                Some(next) => {
                    out.push('\\');
                    out.push(next);
                    i += 2;
                    last_is_class_end = false;
                    continue;
                }
                None => {
                    out.push('\\');
                    i += 1;
                    continue;
                }
            }
        }
        if c == '[' {
            in_class += 1;
            out.push(c);
            i += 1;
            last_is_class_end = false;
            continue;
        }
        if c == ']' {
            in_class = in_class.saturating_sub(1);
            out.push(c);
            i += 1;
            last_is_class_end = in_class > 0;
            continue;
        }
        if c == '-' && in_class > 0 {
            if last_is_class_end {
                out.push('\\');
            }
            out.push('-');
            i += 1;
            last_is_class_end = false;
            continue;
        }
        out.push(c);
        i += 1;
        last_is_class_end = false;
    }
    out
}

/// Java octal escapes (`\02`, `\012`, `\0mnn`) have no Rust regex
/// equivalent; convert them to `\x{...}` (Polish `BRAK_KROPKI` uses `[\02]`).
fn normalize_java_octal_escapes(pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len());
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '\\' {
            if i + 2 < chars.len() && chars[i + 1] == '0' && chars[i + 2].is_digit(8) {
                let mut digits = String::new();
                let mut k = i + 2;
                while k < chars.len() && digits.len() < 3 && chars[k].is_digit(8) {
                    digits.push(chars[k]);
                    k += 1;
                }
                if let Ok(value) = u32::from_str_radix(&digits, 8) {
                    out.push_str(&format!("\\x{{{value:X}}}"));
                    i = k;
                    continue;
                }
            }
            out.push(chars[i]);
            if i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Java allows quantified anchors (e.g. `PRP$?` in postag patterns); a
/// quantified anchor matches the empty string, so dropping it yields the
/// same full-match semantics for the regex crate.
fn drop_quantified_anchors(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            out.push(c);
            if let Some(next) = chars.next() {
                out.push(next);
            }
            continue;
        }
        if (c == '$' || c == '^') && chars.peek() == Some(&'?') {
            chars.next();
            continue;
        }
        out.push(c);
    }
    out
}

/// One `AbstractTokenBasedRule.TokenHint` (conservative subset: Java also
/// derives hints from regexp patterns via `StringMatcher.getPossibleValues`).
#[derive(Debug, Clone)]
pub struct CompiledHint {
    pub inflected: bool,
    pub values_lower: Vec<String>,
    pub token_index: usize,
}

/// Java `AbstractTokenBasedRule` constructor: the minimum number of
/// non-whitespace tokens a sentence needs before the rule can match, and the
/// text/lemma hints that must occur for a match to be possible.
///
/// The port only derives hints from plain (non-regexp) mandatory tokens
/// without OR groups, a strict subset of Java's hints; skipping a rule based
/// on such a hint is therefore as safe as Java's own check.
pub fn pattern_hints(tokens: &[PatternToken]) -> (usize, Vec<CompiledHint>, Option<CompiledHint>) {
    let first_can_match_start = tokens.first().is_none_or(|t| {
        t.postag.as_deref() == Some("SENT_START") || t.negate || !has_string_that_must_match(t)
    });
    let mut min_token_count = usize::from(!first_can_match_start);
    let mut hints = Vec::new();
    let mut anchor: Option<CompiledHint> = None;
    let mut fixed_offset = true;
    for (i, t) in tokens.iter().enumerate() {
        if t.min.unwrap_or(1) > 0 {
            min_token_count += 1;
        }
        let hintable = !(t.negate
            || !t.or_group.is_empty()
            || t.match_ref.is_some()
            || t.min.unwrap_or(1) == 0
            || !has_string_that_must_match(t));
        if hintable {
            // plain text, or a regexp whose complete value set is known
            // (`StringMatcher.getPossibleValues`)
            let values: Option<Vec<String>> = match t.text.as_deref() {
                Some(text) if !text.is_empty() => {
                    if t.regexp {
                        string_matcher::possible_values(text)
                    } else {
                        Some(vec![text.to_string()])
                    }
                }
                _ => None,
            };
            if let Some(values) = values {
                let hint = CompiledHint {
                    inflected: t.inflected,
                    values_lower: values.iter().map(|v| v.to_lowercase()).collect(),
                    token_index: i,
                };
                if fixed_offset && anchor.is_none() {
                    anchor = Some(hint.clone());
                }
                hints.push(hint);
            }
        }
        // Java updates `fixedOffset` for every token, whether or not the
        // token produced a hint
        if t.min.unwrap_or(1) != 1 || t.skip.unwrap_or(0) != 0 || t.max.unwrap_or(1) != 1 {
            fixed_offset = false;
        }
    }
    hints.sort_by_key(|h| {
        (
            h.values_lower.len(),
            std::cmp::Reverse(h.values_lower.iter().map(|v| v.len()).min().unwrap_or(0)),
        )
    });
    (min_token_count, hints, anchor)
}

/// Fast-path matchers for a pattern element's text (`StringMatcher.create`):
/// a literal for plain text or single-value regexps, a value set for regexps
/// whose complete value set is known, else `None` (regex).
/// Build the text matcher of a pattern element or exception: a literal or
/// value-set fast path when possible (`StringMatcher.create`), else the
/// compiled regexp. The regexp is not compiled when a fast path replaces it.
#[allow(clippy::type_complexity)]
fn compile_text_matcher(
    text: Option<&str>,
    regexp: bool,
    case_sensitive: bool,
    is_plain: bool,
) -> Result<
    (
        Option<std::sync::Arc<TextRegex>>,
        Option<(String, bool)>,
        Option<(std::collections::HashSet<String>, bool)>,
    ),
    String,
> {
    let Some(text) = text else {
        return Ok((None, None, None));
    };
    if !is_plain {
        // `<match no>` reference elements resolve their matcher per attempt
        return Ok((None, None, None));
    }
    if !regexp {
        return Ok((None, Some((text.to_string(), case_sensitive)), None));
    }
    match string_matcher::possible_values(text) {
        Some(values) if values.len() == 1 => {
            Ok((None, Some((values[0].clone(), case_sensitive)), None))
        }
        Some(values) => {
            let set: std::collections::HashSet<String> = if case_sensitive {
                values.into_iter().collect()
            } else {
                values.iter().map(|v| v.to_lowercase()).collect()
            };
            Ok((None, None, Some((set, case_sensitive))))
        }
        None => Ok((
            Some(compile_regex_fast(text, case_sensitive, true)?),
            None,
            None,
        )),
    }
}

/// `PatternToken.hasStringThatMustMatch` (negation/omitted/reference/empty).
fn has_string_that_must_match(t: &PatternToken) -> bool {
    !t.negate
        && t.min.unwrap_or(1) != 0
        && t.match_ref.is_none()
        && !t.text.as_deref().unwrap_or("").is_empty()
}

pub(crate) fn compile_token(t: &PatternToken) -> Result<CompiledToken, String> {
    // reference elements are compiled per match attempt (`resolve_references`)
    let (text, literal, value_set) = compile_text_matcher(
        t.text.as_deref(),
        t.regexp,
        t.case_sensitive,
        t.match_ref.is_none(),
    )?;
    let postag = match (&t.postag, t.postag_regexp) {
        (Some(tag), true) => Some(compile_regex_fast(tag, true, true)?),
        (Some(tag), false) => Some(compile_regex_fast(&regex::escape(tag), true, true)?),
        (None, _) => None,
    };
    let pos_unknown = postag
        .as_ref()
        .map(|re: &std::sync::Arc<TextRegex>| re.is_match("UNKNOWN"))
        .unwrap_or(false);
    let exceptions = t
        .exceptions
        .iter()
        .map(|e| -> Result<CompiledException, String> {
            let (text, literal, value_set) =
                compile_text_matcher(e.text.as_deref(), e.regexp, e.case_sensitive, true)?;
            Ok(CompiledException {
                has_text: e.text.is_some(),
                text,
                literal,
                value_set,
                postag: match (&e.postag, e.postag_regexp) {
                    (Some(tag), true) => Some(compile_regex_fast(tag, true, true)?),
                    (Some(tag), false) => {
                        Some(compile_regex_fast(&regex::escape(tag), true, true)?)
                    }
                    (None, _) => None,
                },
                pos_unknown: e
                    .postag
                    .as_ref()
                    .map(|tag| {
                        compile_regex_fast(tag, true, true)
                            .map(|re| re.is_match("UNKNOWN"))
                            .unwrap_or(false)
                    })
                    .unwrap_or(false),
                negate: e.negate,
                negate_pos: e.negate_pos,
                inflected: e.inflected,
                spacebefore: e.spacebefore,
                scope: e.scope.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let and_group = t
        .and_group
        .iter()
        .map(compile_token)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CompiledToken {
        text,
        has_text: t.text.is_some(),
        literal,
        value_set,
        negate: t.negate,
        pos_unknown,
        sentence_end_postag: t
            .postag
            .as_deref()
            .map(|p| p == "SENT_END")
            .unwrap_or(false),
        postag,
        postag_source: t.postag.clone(),
        negate_pos: t.negate_pos,
        inflected: t.inflected,
        gap_min: 0,
        gap_max: 0,
        optional: t.min == Some(0),
        max_occurrence: t.max.unwrap_or(1),
        chunk: match (&t.chunk, &t.chunk_re) {
            (None, Some(re)) => Some((re.clone(), true)),
            (Some(c), None) => Some((c.clone(), false)),
            _ => None,
        },
        in_marker: t.in_marker,
        skip: t.skip.unwrap_or(0),
        spacebefore: t.spacebefore,
        match_ref: t.match_ref.as_ref().map(|m| CompiledMatchRef {
            no: m.no,
            template: t.text.clone().unwrap_or_default(),
            case_conversion: m.case_conversion.clone(),
            setpos: m.setpos,
            postag: m.postag.clone(),
        }),
        regexp: t.regexp,
        case_sensitive: t.case_sensitive,
        exceptions,
        and_group,
        unification: t.unification.clone(),
        uni_negated: t.uni_negated,
        last_in_unification: t.last_in_unification,
        unification_neutral: t.unification_neutral,
    })
}

/// Compile one concrete element sequence (no OR groups) into a pattern.
fn compile_sequence(
    pt: &[PatternToken],
    marker_start: Option<usize>,
    marker_end: Option<usize>,
) -> Result<CompiledPattern, String> {
    let mut tokens = Vec::new();
    for (i, t) in pt.iter().enumerate() {
        let mut token = compile_token(t)?;
        // LT: `skip` on the previous token controls the gap before this one
        // (`max` is a repetition count of the element itself, matched as a
        // consecutive run in `try_from`)
        let gap_max = if i == 0 {
            0
        } else {
            match pt[i - 1].skip {
                Some(-1) => MAX_GAP,
                Some(skip) => skip,
                None => 0,
            }
        };
        token.gap_min = 0;
        token.gap_max = gap_max;
        tokens.push(token);
    }
    let has_unification = tokens.iter().any(|t| t.unification.is_some());
    let min_len = tokens.iter().filter(|t| !t.optional).count();
    Ok(CompiledPattern {
        tokens,
        marker_start,
        marker_end,
        min_len,
        has_unification,
        skip_immunized: false,
        anchor: None,
    })
}

/// Expand `<or>` groups the way `PatternRuleHandler.createRules` does: one
/// concrete rule per OR alternative (nested alternatives included).
fn expand_or_groups(pt: &[PatternToken]) -> Vec<Vec<PatternToken>> {
    let mut results: Vec<Vec<PatternToken>> = vec![Vec::new()];
    for element in pt {
        let mut next = Vec::new();
        for variant in expand_element(element) {
            for prefix in &results {
                let mut seq = prefix.clone();
                seq.extend(variant.iter().cloned());
                next.push(seq);
            }
        }
        results = next;
    }
    results
}

/// All concrete expansions of one element: the element itself plus its OR
/// alternatives, each recursively expanded.
fn expand_element(element: &PatternToken) -> Vec<Vec<PatternToken>> {
    let mut alternatives = Vec::with_capacity(1 + element.or_group.len());
    alternatives.push(without_or(element));
    for alt in &element.or_group {
        alternatives.push(without_or(alt));
    }
    let mut out = Vec::new();
    for alt in alternatives {
        if alt.or_group.is_empty() {
            out.push(vec![alt]);
        } else {
            out.extend(expand_element(&alt));
        }
    }
    out
}

fn without_or(element: &PatternToken) -> PatternToken {
    let mut e = element.clone();
    e.or_group = Vec::new();
    e
}

/// Compile a pattern, expanding `<or>` groups into concrete alternative
/// sequences (one compiled pattern per alternative, like LT's rule loader).
pub fn compile_patterns(
    pt: &[PatternToken],
    marker_start: Option<usize>,
    marker_end: Option<usize>,
) -> Result<Vec<CompiledPattern>, String> {
    expand_or_groups(pt)
        .iter()
        .map(|seq| {
            let mut compiled = compile_sequence(seq, marker_start, marker_end)?;
            compiled.anchor = pattern_hints(seq).2;
            Ok(compiled)
        })
        .collect()
}

/// Compile a pattern into its first OR expansion (callers that need all
/// alternatives use `compile_patterns`).
pub fn compile_pattern(
    pt: &[PatternToken],
    marker_start: Option<usize>,
    marker_end: Option<usize>,
) -> Result<CompiledPattern, String> {
    compile_sequence(&expand_or_groups(pt).remove(0), marker_start, marker_end)
}

/// Expand `\N` back-references in rule messages and suggestions. `resolve`
/// maps a 1-based reference number to its replacement (`None`/missing →
/// empty, like LT's handling of optional unmatched tokens).
/// Wrap `\N` occurrences in literal suggestion text with `\u{1}N\u{2}` so
/// that digits from a following match result cannot extend the number.
fn shield_literal_refs(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            out.push('\u{1}');
            out.push_str(&s[i + 1..j]);
            out.push('\u{2}');
            i = j;
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// `RegexPatternRule.processMessage`: Java substitutes `""` for missing
/// groups without `concatWithoutExtraSpace`, so this keeps the plain
/// reference expansion (no space removal).
pub fn expand_backrefs(s: &str, resolve: &dyn Fn(usize) -> Option<String>) -> String {
    expand_backrefs_impl(s, resolve, false)
}

/// `PatternRuleMatcher.formatMatches` suggestion rendering: Java processes
/// the whole `<suggestion>…</suggestion>` block, so an empty reference
/// directly before the closing tag (`concatWithoutExtraSpace`'s
/// `rightSide.startsWith("</suggestion>")` branch) pops the preceding space.
/// In the per-suggestion rendering the closing tag is the string end.
fn expand_suggestion_backrefs(s: &str, resolve: &dyn Fn(usize) -> Option<String>) -> String {
    expand_backrefs_impl(s, resolve, true)
}

fn expand_backrefs_impl(
    s: &str,
    resolve: &dyn Fn(usize) -> Option<String>,
    at_suggestion_end: bool,
) -> String {
    if !s.contains('\\') && !s.contains('\u{1}') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < bytes.len() {
        // sentinel-wrapped reference (see `shield_literal_refs`)
        let sentinel = bytes[i] == 1
            && bytes[i + 1..]
                .iter()
                .position(|b| *b == 2)
                .is_some_and(|p| p > 0);
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() || sentinel {
            let num_start = i + 1;
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let n: usize = s[num_start..j].parse().unwrap_or(0);
            if sentinel && j < bytes.len() && bytes[j] == 2 {
                j += 1;
            }
            match resolve(n) {
                // Java `formatMatches`: a single *empty* match value (e.g.
                // `\1` referring to an unmatched optional element or the empty
                // SENT_START token) goes through `concatWithoutExtraSpace`
                // exactly like an unresolved reference, so a leading space is
                // dropped (`\1 <match/>` renders "O seu", not " O seu").
                Some(v) if !(at_suggestion_end && v.is_empty()) => out.push_str(&v),
                // Java `concatWithoutExtraSpace`: an empty (optional
                // unmatched) reference also removes the adjacent space
                _ => {
                    let mut k = j;
                    while k < bytes.len() && (bytes[k] as char).is_whitespace() {
                        k += 1;
                    }
                    let right_starts_ws = k > j && k < bytes.len();
                    // in the per-suggestion rendering the closing tag sits at
                    // the string end (Java: `rightSide.startsWith("</suggestion>")`)
                    let before_closing_tag = at_suggestion_end && j >= bytes.len();
                    if out.ends_with(' ')
                        && (right_starts_ws || punct_after(s, j) || before_closing_tag)
                    {
                        out.pop();
                    } else if (out.is_empty() || out.ends_with("suggestion>")) && right_starts_ws {
                        // Java `concatWithoutExtraSpace`: drop the space right
                        // after a suggestion tag (the string start stands in
                        // for the tag in per-suggestion rendering)
                        j = k;
                    } else if right_starts_ws {
                        j = k - 1; // leave one whitespace char to skip below
                    }
                }
            }
            i = j;
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// `PatternRuleMatcher.formatMatches`: expand `\N` placeholders, applying
/// the `<match>` element's case conversion/regexp replacement when the
/// placeholder came from one (`refs` records their byte offsets).
pub fn expand_message_backrefs(
    s: &str,
    refs: &[MessageMatchRef],
    resolve: &dyn Fn(usize) -> Option<String>,
) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let n: usize = s[i + 1..j].parse().unwrap_or(0);
            let spec = refs.iter().find(|r| r.offset == i && r.spec.no == n);
            match resolve(n) {
                Some(v) => {
                    let value = match spec {
                        Some(r) => apply_match_transforms(
                            &v,
                            &r.spec.case_conversion,
                            r.spec.regexp_match.as_deref(),
                            r.spec.regexp_replace.as_deref(),
                        ),
                        None => v,
                    };
                    out.push_str(&value);
                }
                None => {
                    let mut k = j;
                    while k < bytes.len() && (bytes[k] as char).is_whitespace() {
                        k += 1;
                    }
                    let right_starts_ws = k > j && k < bytes.len();
                    // Java `concatWithoutExtraSpace`: a reference directly
                    // before `</suggestion>` also drops the preceding space
                    let before_closing_tag = s[j..].starts_with("</suggestion>");
                    if out.ends_with(' ')
                        && (right_starts_ws || punct_after(s, j) || before_closing_tag)
                    {
                        out.pop();
                    } else if (out.is_empty() || out.ends_with("suggestion>")) && right_starts_ws {
                        // Java `concatWithoutExtraSpace`: drop the space right
                        // after a suggestion tag (the string start stands in
                        // for the tag in per-suggestion rendering)
                        j = k;
                    } else if right_starts_ws {
                        j = k - 1;
                    }
                }
            }
            i = j;
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// `PatternRuleMatcher.formatMatches` with a synthesizer: expands the `\N`
/// placeholders of a rule message, using the message's `<match>` elements in
/// occurrence order (Java `suggestionMatches` + `matchCounter`).
pub fn expand_message_matches<T: Deref<Target = AnalyzedTokenReadings>>(
    s: &str,
    refs: &[MessageMatchRef],
    tokens: &[T],
    positions: &[Option<usize>],
    synth: Option<&dyn Synthesizer>,
) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let _ = expand_message_matches_into(s, refs, tokens, positions, synth, 0, &mut out);
    out
}

#[allow(clippy::too_many_arguments)]
fn expand_message_matches_into<T: Deref<Target = AnalyzedTokenReadings>>(
    s: &str,
    refs: &[MessageMatchRef],
    tokens: &[T],
    positions: &[Option<usize>],
    synth: Option<&dyn Synthesizer>,
    mut match_counter: usize,
    out: &mut String,
) -> usize {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let n: usize = s[i + 1..j].parse().unwrap_or(0);
            let spec = refs.get(match_counter).map(|r| r.spec.clone());
            match spec {
                Some(spec) => {
                    let mut spec = spec;
                    spec.no = n;
                    match render_match_ref(&spec, tokens, positions, synth) {
                        // Java `formatMatches`: a single *empty* match (e.g.
                        // `\1` on the empty SENT_START token) goes through
                        // `concatWithoutExtraSpace` like an unmatched optional
                        // element, so `« \1 »` renders `« »` (one space).
                        Some(forms) if forms.len() == 1 && !forms[0].is_empty() => {
                            out.push_str(&forms[0]);
                            i = j;
                        }
                        Some(forms) if forms.len() > 1 => {
                            // Java concatenates the replacement and keeps
                            // scanning in place; here the already-processed
                            // prefix plus the replacement is committed and
                            // the untouched remainder is scanned recursively.
                            let right = &s[j..];
                            let left = out.clone();
                            let (joined, right_new) =
                                format_multiple_synthesis(&forms, &left, right);
                            // `joined` always ends with the whole `right`
                            // (when `suggestionRight` is empty, `rightNew` is
                            // `right`); commit only the replacement and
                            // re-scan the untouched remainder so placeholders
                            // inside the suggestion text still expand.
                            let replacement =
                                joined[left.len()..joined.len() - right_new.len()].to_string();
                            let next_counter = expand_message_matches_into(
                                &replacement,
                                refs,
                                tokens,
                                positions,
                                synth,
                                match_counter + 1,
                                out,
                            );
                            expand_message_matches_into(
                                &s[j + right.len() - right_new.len()..],
                                refs,
                                tokens,
                                positions,
                                synth,
                                next_counter,
                                out,
                            );
                            return next_counter;
                        }
                        _ => {
                            // unmatched optional element: collapse a space.
                            // Java's `WHITESPACE_OR_PUNCT` uses the ASCII
                            // `\s` class, so an NBSP does *not* trigger the
                            // space collapse (French messages embed NBSPs).
                            let mut k = j;
                            while k < bytes.len() && is_java_whitespace(bytes[k]) {
                                k += 1;
                            }
                            let right_starts_ws = k > j && k < bytes.len();
                            if out.ends_with(' ') && (right_starts_ws || punct_after(s, j)) {
                                out.pop();
                            } else if (out.is_empty() || out.ends_with("suggestion>"))
                                && right_starts_ws
                            {
                                // Java `concatWithoutExtraSpace`: drop the space
                                // right after a suggestion tag
                                j = k;
                            } else if right_starts_ws {
                                j = k - 1;
                            }
                            i = j;
                        }
                    }
                    match_counter += 1;
                }
                None => {
                    // no `<match>` element: Java's `!newWay` fallback replaces
                    // the placeholder with the referenced token's surface
                    if let Some(idx) = fallback_placeholder_token(n, positions) {
                        if let Some(t) = tokens.get(idx) {
                            out.push_str(t.surface());
                        }
                    }
                    i = j;
                }
            }
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    match_counter
}

/// Java `formatMatches` fallback without a `<match>` element: the placeholder
/// maps to the token matched by pattern element N. An unmatched optional
/// element renders empty; an out-of-range reference clamps to the last match.
fn fallback_placeholder_token(n: usize, positions: &[Option<usize>]) -> Option<usize> {
    if positions.is_empty() {
        return None;
    }
    if n <= positions.len() {
        return positions[n - 1];
    }
    positions.iter().flatten().next_back().copied()
}

/// `PatternRuleMatcher.formatMultipleSynthesis`: duplicate the enclosing
/// `<suggestion>` tags around each synthesized form.
fn format_multiple_synthesis(forms: &[String], left: &str, right: &str) -> (String, String) {
    let mut suggestion_left = "";
    let mut suggestion_right = "";
    let mut right_new = right.to_string();
    let s_pos = left.rfind("<suggestion>");
    if let Some(pos) = s_pos {
        suggestion_left = &left[pos + "<suggestion>".len()..];
    }
    let mut error_message = if suggestion_left.is_empty() {
        left.to_string()
    } else {
        left[..s_pos.unwrap()].to_string() + "<suggestion>"
    };
    let r_pos = right.find("</suggestion>");
    if let Some(pos) = r_pos {
        suggestion_right = &right[..pos];
    }
    if !suggestion_right.is_empty() {
        right_new = right[r_pos.unwrap()..].to_string();
    }
    // Java compares raw string indexes (`-1` when absent)
    let last_left_sug_end = left.rfind("</suggestion>").map_or(-1i64, |i| i as i64);
    let last_left_sug_start = left.rfind("<suggestion>").map_or(-1i64, |i| i as i64);
    for (z, form) in forms.iter().enumerate() {
        error_message.push_str(suggestion_left);
        error_message.push_str(form);
        error_message.push_str(suggestion_right);
        if z < forms.len() - 1 && last_left_sug_end < last_left_sug_start {
            error_message.push_str("</suggestion>, <suggestion>");
        }
    }
    error_message.push_str(&right_new);
    (error_message, right_new)
}

/// LT `MatchState`: case conversion then `regexp_match`/`regexp_replace`.
/// Java `$N` replacement back-references → regex-crate `${N}`.
pub fn normalize_java_replacement(replacement: &str, group_count: usize) -> String {
    // Java `Matcher.appendReplacement` parses `$`-references with the
    // "largest legal group number" rule: `$200` with two groups means group 2
    // followed by the literal `00` (the regex crate would treat `${200}` as
    // an empty group), so clamp the digits here.
    let mut out = String::with_capacity(replacement.len());
    let chars: Vec<char> = replacement.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        // Java treats a backslash as an escape: `\1` is the literal `1`
        // (e.g. the Portuguese `postag_replace='$1\1S$2'`), while the regex
        // crate would keep the backslash in the output.
        if chars[i] == '\\' && i + 1 < chars.len() {
            // A literal `$` must stay literal for the regex crate as well.
            if chars[i + 1] == '$' {
                out.push_str("$$");
            } else {
                out.push(chars[i + 1]);
            }
            i += 2;
            continue;
        }
        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            let mut ref_num = (chars[i + 1] as u8 - b'0') as usize;
            let mut j = i + 2;
            while j < chars.len() && chars[j].is_ascii_digit() {
                let next = ref_num * 10 + (chars[j] as u8 - b'0') as usize;
                if next > group_count {
                    break;
                }
                ref_num = next;
                j += 1;
            }
            out.push_str("${");
            out.push_str(&ref_num.to_string());
            out.push('}');
            i = j;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Java allows a quantifier to be applied to an already quantified atom
/// (`X*{1,30}` is `(X*){1,30}`, i.e. equivalent to `X*`); the regex crate
/// reads the `{` after a quantifier as a literal. Rewrite those nested
/// quantifiers away (verified against Java's `Pattern`; D-022).
pub fn normalize_java_quantifiers(pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len());
    let mut i = 0usize;
    let mut in_class = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            out.push(c);
            if i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if in_class {
            out.push(c);
            if c == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }
        if c == '[' {
            in_class = true;
            out.push(c);
            i += 1;
            continue;
        }
        if matches!(c, '*' | '+' | '?') && i + 1 < chars.len() && chars[i + 1] == '{' {
            let mut j = i + 2;
            let mut digits = 0usize;
            while j < chars.len() && chars[j].is_ascii_digit() {
                digits += 1;
                j += 1;
            }
            if digits > 0 && j < chars.len() && chars[j] == ',' {
                j += 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
            }
            if digits > 0 && j < chars.len() && chars[j] == '}' {
                out.push(c);
                i = j + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Java's regex syntax allows an empty inline-flag group `(?)` (no-op);
/// `fancy-regex` rejects it, so normalize that one case.
fn java_regex(pattern: &str) -> Option<FancyRegex> {
    let pattern = normalize_java_quantifiers(pattern);
    let pattern = normalize_java_octal_escapes(&pattern);
    let pattern = escape_class_hyphens(&pattern);
    let pattern = strip_java_unicode_flags(&pattern);
    if let Ok(re) = FancyRegex::new(&pattern) {
        return Some(re);
    }
    FancyRegex::new(&pattern.replace("(?)", "(?:)")).ok()
}

/// Drop `u`/`U` from Java inline flag groups: the Rust regex crate cannot
/// leave Unicode mode (`ParseError(NonUnicodeUnsupported)` for `(?-iu)`),
/// and Java's `UNICODE_CASE`/`UNICODE_CHARACTER_CLASS` have no equivalent
/// there. The remaining flags keep their meaning; a group left empty
/// becomes the no-op `(?:)`.
fn strip_java_unicode_flags(pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len());
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '\\' {
            out.push(chars[i]);
            if i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if chars[i] == '(' && i + 1 < chars.len() && chars[i + 1] == '?' {
            let mut j = i + 2;
            let mut flags = String::new();
            let mut is_flag_group = true;
            while j < chars.len() && chars[j] != ')' {
                if matches!(chars[j], 'i' | 'm' | 's' | 'x' | 'u' | 'U' | 'd' | '-') {
                    flags.push(chars[j]);
                    j += 1;
                } else {
                    is_flag_group = false;
                    break;
                }
            }
            if is_flag_group && !flags.is_empty() && j < chars.len() && chars[j] == ')' {
                let cleaned: String = flags.chars().filter(|c| !matches!(c, 'u' | 'U')).collect();
                let cleaned = cleaned.trim_end_matches('-');
                if cleaned.is_empty() {
                    out.push_str("(?:)");
                } else {
                    out.push_str("(?");
                    out.push_str(cleaned);
                    out.push(')');
                }
                i = j + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

pub fn apply_match_transforms(
    value: &str,
    case_conversion: &str,
    regexp_match: Option<&str>,
    regexp_replace: Option<&str>,
) -> String {
    let sample = value;
    // LT MatchState.toFinalString: regexp replacement first, then case
    let mut value = value.to_string();
    if let Some(rm) = regexp_match {
        if let Some(re) = java_regex(rm) {
            // Java `Matcher.replaceAll` uses `$N`; the regex crate reads
            // `$1re` as a group named "1re", so normalize to `${1}re`.
            let group_count = re.captures_len().saturating_sub(1);
            let replacement = regexp_replace.unwrap_or("").to_string();
            let replacement = normalize_java_replacement(&replacement, group_count);
            let new = re.replace_all(&value, replacement.as_str());
            value = new.into_owned();
        }
    }
    if case_conversion.is_empty() {
        value
    } else {
        apply_case_conversion(&value, case_conversion, sample)
    }
}

/// Java `WHITESPACE_OR_PUNCT`: `[\s,:;.!?].*`
fn punct_after(s: &str, at: usize) -> bool {
    s[at..]
        .chars()
        .next()
        .map(|c| is_java_whitespace_char(c) || matches!(c, ',' | ':' | ';' | '.' | '!' | '?'))
        .unwrap_or(false)
}

/// Java regex `\s` is the ASCII class (no NBSP).
fn is_java_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

fn is_java_whitespace_char(c: char) -> bool {
    c.is_ascii() && is_java_whitespace(c as u8)
}

/// Resolve a `<match no="N">` token to a concrete text matcher, mirroring
/// `PatternTokenMatcher.resolveReference` + `PatternToken.doCompile` (common
/// path: text reference with case conversion). Returns `None` when the
/// referenced token is not part of this attempt.
fn resolve_references<T: Deref<Target = AnalyzedTokenReadings>>(
    token: &CompiledToken,
    tokens: &[T],
    first_matched: Option<usize>,
    synth: Option<&dyn Synthesizer>,
) -> Option<CompiledToken> {
    let mut resolved = token.clone();
    resolved.and_group = token
        .and_group
        .iter()
        .map(|m| resolve_references(m, tokens, first_matched, synth))
        .collect::<Option<Vec<_>>>()?;
    if let Some(mref) = &token.match_ref {
        // Java `PatternToken.doCompile`: the reference is replaced by the
        // synthesized forms (`MatchState.toTokenString`, `|`-joined)
        let first = first_matched?;
        let atr = tokens.get(first + mref.no)?;
        let spec = crate::MatchRefSpec {
            no: mref.no,
            phrase_len: 1,
            postag: mref.postag.clone(),
            postag_replace: None,
            postag_regexp: false,
            regexp_match: None,
            regexp_replace: None,
            case_conversion: mref.case_conversion.clone().unwrap_or_default(),
            setpos: mref.setpos,
            static_lemma: None,
            include_skipped: "none".to_string(),
            suppress_misspelled: false,
        };
        let value = match render_match_ref_forms(&spec, atr, synth) {
            Some(forms) => forms.join("|"),
            None => String::new(),
        };
        let text = if mref.template.is_empty() {
            value
        } else {
            mref.template.replace(&format!("\\{}", mref.no), &value)
        };
        resolved.match_ref = None;
        resolved.literal = None;
        resolved.value_set = None;
        resolved.text = Some(if token.regexp {
            compile_regex_fast(&text, token.case_sensitive, true).ok()?
        } else {
            compile_regex_fast(&regex::escape(&text), token.case_sensitive, true).ok()?
        });
    }
    Some(resolved)
}

/// One reading against one compiled element, mirroring
/// `PatternToken.isMatched(AnalyzedToken)`: text and POS each XOR their
/// negation, combined with logical AND.
pub(crate) fn reading_matches(
    token: &CompiledToken,
    r: &AnalyzedToken,
    token_untagged: bool,
) -> bool {
    // literal and value-set matchers replace the compiled regexp entirely
    // (`StringMatcher.create`), so `text` is only consulted without them
    let text_ok = if !token.has_text && token.literal.is_none() && token.value_set.is_none() {
        !token.negate
    } else {
        let test = if token.inflected {
            r.stem.as_deref().unwrap_or(&r.token)
        } else {
            &r.token
        };
        let hit = match (&token.literal, &token.value_set, &token.text) {
            (Some(lit), _, _) => text_literal_hit(lit, test),
            (_, Some(set), _) => text_set_hit(set, test),
            (_, _, Some(re)) => re.is_match(test),
            _ => false,
        };
        hit ^ token.negate
    };
    let pos_ok = match &token.postag {
        Some(re) => {
            // LT PatternToken.isPosTokenMatched: the special UNKNOWN tag
            // matches readings without a real POS tag (null, SENT_END,
            // PARAGRAPH_END)
            let pos_unknown = token.pos_unknown;
            let hit = if pos_unknown && unknown_hits(r, token_untagged) {
                true
            } else {
                match &r.pos_tag {
                    None => pos_unknown,
                    Some(tag) => re.is_match(tag),
                }
            };
            hit ^ token.negate_pos
        }
        None => true,
    };
    text_ok && pos_ok
}

/// Java `AnalyzedToken.hasNoPOSTag`: null, SENT_END or PARA_END.
fn has_no_pos_tag(tag: Option<&str>) -> bool {
    matches!(tag, None | Some("SENT_END") | Some("PARA_END"))
}

/// Empirical Java matcher rule (see docs/differences.md): the special
/// `UNKNOWN` postag also matches a SENT_END/PARA_END reading, but only for
/// tokens that have no real tag at all (e.g. unknown words at sentence end).
fn unknown_hits(r: &AnalyzedToken, token_untagged: bool) -> bool {
    token_untagged && has_no_pos_tag(r.pos_tag.as_deref())
}

/// Java `String.equalsIgnoreCase`-equivalent, no allocation for the
/// common ASCII path.
pub(crate) fn eq_ignore_case(a: &str, b: &str) -> bool {
    if a.is_ascii() && b.is_ascii() {
        a.eq_ignore_ascii_case(b)
    } else {
        a.chars()
            .flat_map(char::to_lowercase)
            .eq(b.chars().flat_map(char::to_lowercase))
    }
}

/// `StringMatcher.matches` for the literal fast path, including the
/// `MAX_MATCH_LENGTH` limit.
fn text_literal_hit(lit: &(String, bool), test: &str) -> bool {
    if test.chars().count() > 250 {
        return false;
    }
    let (lit, case_sensitive) = lit;
    if *case_sensitive {
        test == lit
    } else {
        eq_ignore_case(test, lit)
    }
}

/// `StringMatcher.matches` for the extracted value-set fast path.
fn text_set_hit(set: &(std::collections::HashSet<String>, bool), test: &str) -> bool {
    if test.chars().count() > 250 {
        return false;
    }
    let (set, case_sensitive) = set;
    if *case_sensitive {
        set.contains(test)
    } else {
        set.contains(&test.to_lowercase())
    }
}

fn any_reading_matches(token: &CompiledToken, tr: &AnalyzedTokenReadings) -> bool {
    let untagged = tr
        .readings
        .iter()
        .all(|r| has_no_pos_tag(r.pos_tag.as_deref()));
    tr.readings
        .iter()
        .any(|r| reading_matches(token, r, untagged))
}

/// LT `PatternToken.isWhitespaceBefore`: the `spacebefore` attribute must
/// agree with the token's whitespace-before flag.
fn whitespace_ok(spacebefore: Option<bool>, tr: &AnalyzedTokenReadings) -> bool {
    spacebefore.is_none_or(|req| req == tr.whitespace_before)
}

/// All match conditions of one pattern element, including its AND group
/// (LT `PatternTokenMatcher.isMatched` + `addMemberAndGroup`; base and group
/// members may match different readings of the same token). Exceptions are
/// handled separately by the caller, like LT's `testAllReadings`.
fn token_matches(token: &CompiledToken, tr: &AnalyzedTokenReadings) -> bool {
    if !whitespace_ok(token.spacebefore, tr)
        || !any_reading_matches(token, tr)
        || !chunk_matches(token, tr)
    {
        return false;
    }
    token.and_group.iter().all(|member| {
        whitespace_ok(member.spacebefore, tr)
            && any_reading_matches(member, tr)
            && chunk_matches(member, tr)
    })
}

/// One reading against one exception (`PatternToken.isMatched` semantics).
fn exception_matches_reading(
    exc: &CompiledException,
    r: &AnalyzedToken,
    token_untagged: bool,
) -> bool {
    // Java `XMLRuleHandler` adds no exception for an attribute-less
    // `<exception/>` (e.g. the Catalan ECLIPSIS_ECLIPSI/ROTAR_GIRAR
    // patterns); the empty spec must not match every token.
    if !exc.has_text && exc.literal.is_none() && exc.value_set.is_none() && exc.postag.is_none() {
        return false;
    }
    {
        let text_ok = if !exc.has_text && exc.literal.is_none() && exc.value_set.is_none() {
            !exc.negate
        } else {
            let test = if exc.inflected {
                r.stem.as_deref().unwrap_or(&r.token)
            } else {
                &r.token
            };
            let hit = match (&exc.literal, &exc.value_set, &exc.text) {
                (Some(lit), _, _) => text_literal_hit(lit, test),
                (_, Some(set), _) => text_set_hit(set, test),
                (_, _, Some(re)) => re.is_match(test),
                _ => false,
            };
            hit ^ exc.negate
        };
        let pos_ok = match &exc.postag {
            Some(re) => {
                let pos_unknown = exc.pos_unknown;
                let hit = if pos_unknown && unknown_hits(r, token_untagged) {
                    true
                } else {
                    match &r.pos_tag {
                        None => pos_unknown,
                        Some(tag) => re.is_match(tag),
                    }
                };
                hit ^ exc.negate_pos
            }
            None => true,
        };
        text_ok && pos_ok
    }
}

/// LT `isExceptionMatched`: disjunction over readings of the exception's own
/// match condition (negations apply per reading).
fn exception_matches(exc: &CompiledException, tr: &AnalyzedTokenReadings) -> bool {
    if !whitespace_ok(exc.spacebefore, tr) {
        return false;
    }
    let untagged = tr
        .readings
        .iter()
        .all(|r| has_no_pos_tag(r.pos_tag.as_deref()));
    tr.readings
        .iter()
        .any(|r| exception_matches_reading(exc, r, untagged))
}

/// Exceptions of `token` with the given scope matching one reading (LT
/// `isMatchedByScopeNextException`, used for the performer's `prevMatched`
/// accumulation).
fn reading_scope_exception_matches(
    token: &CompiledToken,
    scope: &str,
    r: &AnalyzedToken,
    whitespace_before: bool,
    untagged: bool,
) -> bool {
    token.exceptions.iter().any(|exc| {
        let s = match exc.scope.as_str() {
            "next" => "next",
            "previous" => "previous",
            _ => "current",
        };
        s == scope
            && exc.spacebefore.is_none_or(|req| req == whitespace_before)
            && exception_matches_reading(exc, r, untagged)
    })
}

/// A successful match: token index matched per pattern token (`None` for an
/// unmatched optional `min="0"` element, like LT's position 0), plus
/// resolved suggestions.
pub struct PatternMatch {
    /// token index matched by each pattern token (last token for `max`
    /// repetition runs)
    pub positions: Vec<Option<usize>>,
    /// first token matched by each pattern token (differs from `positions`
    /// only for `maxOccurrence > 1` runs)
    pub starts: Vec<Option<usize>>,
    pub suggestions: Vec<String>,
    /// unified readings per sequence element for `action="unify"`
    /// (LT `getFinalUnified`)
    pub unified: Option<Vec<Vec<AnalyzedToken>>>,
}

impl PatternMatch {
    /// Index of the first matched token (`starts` keeps the run start for
    /// `maxOccurrence > 1` elements, like LT's `firstMatchToken`).
    pub fn start_tok(&self) -> usize {
        self.starts
            .iter()
            .flatten()
            .copied()
            .next()
            .or_else(|| self.positions.iter().flatten().copied().next())
            .unwrap_or(0)
    }

    /// Index of the last matched token.
    pub fn end_tok(&self) -> usize {
        self.positions
            .iter()
            .flatten()
            .copied()
            .next_back()
            .unwrap_or(0)
    }
}

/// LT `DisambiguationPatternRuleReplacer.keepByDisambig` / grammar-rule
/// antipatterns: does an antipattern match overlap the `[first, last]`
/// token range of a candidate match?
pub fn antipattern_overlaps<T: Deref<Target = AnalyzedTokenReadings>>(
    anti: &CompiledPattern,
    tokens: &[T],
    first: usize,
    last: usize,
    synth: Option<&dyn Synthesizer>,
) -> bool {
    find_matches_with_synth(anti, &[], &[], tokens, None, None, synth)
        .into_iter()
        .any(|m| m.start_tok() <= last && m.end_tok() >= first)
}

/// Subset of `org.languagetool.synthesis.Synthesizer` needed by
/// `MatchState`/`formatMatches` (implemented by `lt::synthesizer`).
pub trait Synthesizer: Send + Sync {
    /// `Synthesizer.synthesize(AnalyzedToken, String, boolean)`
    fn synthesize(&self, token: &AnalyzedToken, pos_tag: &str, pos_tag_regexp: bool)
        -> Vec<String>;

    /// `Synthesizer.synthesize(AnalyzedToken, String)` (the 2-arg form used
    /// by `MatchState.toFinalString` for `<match postag="…">` without
    /// `postag_regexp`). The base class treats the tag as an exact tag; the
    /// Catalan synthesizer overrides it to treat it as a regexp, so languages
    /// can override this.
    fn synthesize_plain(&self, token: &AnalyzedToken, pos_tag: &str) -> Vec<String> {
        self.synthesize(token, pos_tag, false)
    }

    /// `Synthesizer.getTargetPosTag(List<String>, String)` (base class:
    /// the last matching tag, else the fallback)
    fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        pos_tags
            .last()
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Java's `getTargetPosTag` implementations sort the caller's list in
    /// place; `MatchState.getTargetPosTag` relies on that for the
    /// `postag_replace` join order (e.g. the Catalan verb tags). Base class:
    /// no reordering.
    fn sort_target_pos_tags(&self, _pos_tags: &mut Vec<String>) {}

    /// `Synthesizer.getPosTagCorrection`
    fn pos_tag_correction(&self, pos_tag: &str) -> String {
        pos_tag.to_string()
    }

    /// `MatchState.toFinalString`'s tagger-based speller: `false` when the
    /// formatted word is unknown to the language tagger (Java marks it
    /// `<mistake/>` for `checksSpelling` matches). Languages without a
    /// tagger wrapper accept every word.
    fn is_known_word(&self, _word: &str) -> bool {
        true
    }
}

/// Find all non-overlapping matches, left to right.
pub fn find_matches<T: Deref<Target = AnalyzedTokenReadings>>(
    pattern: &CompiledPattern,
    suggestions: &[Vec<SuggestionPart>],
    tokens: &[T],
) -> Vec<PatternMatch> {
    find_matches_with_unify(pattern, suggestions, tokens, None, None)
}

/// Like `find_matches_with_unify`, rendering suggestions with a synthesizer
/// (`MatchState.toFinalString`).
pub fn find_matches_with_synth<T: Deref<Target = AnalyzedTokenReadings>>(
    pattern: &CompiledPattern,
    suggestions: &[Vec<SuggestionPart>],
    suppress_misspelled: &[bool],
    tokens: &[T],
    starts: Option<&[usize]>,
    unify: Option<&crate::unify::EquivalenceConfig>,
    synth: Option<&dyn Synthesizer>,
) -> Vec<PatternMatch> {
    let mut out = Vec::new();
    // Java `AbstractPatternRulePerformer.doMatch`: starts beyond
    // `len - patternSize + minOccurCorrection` cannot complete a match
    let last_start = tokens.len().saturating_sub(pattern.min_len) + 1;
    let range = 0..last_start.min(tokens.len());
    let iter: Box<dyn Iterator<Item = usize>> = match starts {
        Some(s) => Box::new(s.iter().copied()),
        None => Box::new(range),
    };
    for start in iter {
        if let Some(m) = try_match_at(
            pattern,
            suggestions,
            suppress_misspelled,
            tokens,
            start,
            unify,
            synth,
        ) {
            out.push(m);
        }
    }
    out
}

/// Like `find_matches`, with an equivalence config for `<unify>` features.
pub fn find_matches_with_unify<T: Deref<Target = AnalyzedTokenReadings>>(
    pattern: &CompiledPattern,
    suggestions: &[Vec<SuggestionPart>],
    tokens: &[T],
    starts: Option<&[usize]>,
    unify: Option<&crate::unify::EquivalenceConfig>,
) -> Vec<PatternMatch> {
    // LT `AbstractPatternRulePerformer.doMatch` tries every start position
    // (or the anchor hint positions when given); matches may overlap
    // (callers dedupe by rule id + range where needed)
    let mut out = Vec::new();
    let last_start = tokens.len().saturating_sub(pattern.min_len) + 1;
    let iter: Box<dyn Iterator<Item = usize>> = match starts {
        Some(s) => Box::new(s.iter().copied()),
        None => Box::new(0..last_start.min(tokens.len())),
    };
    for start in iter {
        if start >= tokens.len() {
            continue;
        }
        if let Some(m) = try_match_at(pattern, suggestions, &[], tokens, start, unify, None) {
            out.push(m);
        }
    }
    out
}

/// Java `TokenHint.getPossibleIndices`: candidate match starts derived from
/// an anchor hint (`AbstractPatternRulePerformer.doMatch`).
pub fn anchor_starts(
    anchor: &CompiledHint,
    token_lower: &std::collections::HashMap<String, Vec<usize>>,
    lemma_lower: &std::collections::HashMap<String, Vec<usize>>,
) -> Vec<usize> {
    let map = if anchor.inflected {
        lemma_lower
    } else {
        token_lower
    };
    let mut starts: Vec<usize> = Vec::new();
    for v in &anchor.values_lower {
        if let Some(idxs) = map.get(v) {
            for &i in idxs {
                if i >= anchor.token_index {
                    starts.push(i - anchor.token_index);
                }
            }
        }
    }
    starts.sort_unstable();
    starts.dedup();
    starts
}

/// Values collected for one pattern element that takes part in unification
/// (`Readings`: readings that matched, `Neutral`: the whole token).
#[derive(Clone)]
enum UnifyEntry {
    Readings(Vec<AnalyzedToken>),
    Neutral(usize),
}

#[allow(clippy::too_many_arguments)]
fn try_match_at<T: Deref<Target = AnalyzedTokenReadings>>(
    pattern: &CompiledPattern,
    suggestions: &[Vec<SuggestionPart>],
    suppress_misspelled: &[bool],
    tokens: &[T],
    start: usize,
    unify: Option<&crate::unify::EquivalenceConfig>,
    synth: Option<&dyn Synthesizer>,
) -> Option<PatternMatch> {
    try_from(
        pattern,
        suggestions,
        suppress_misspelled,
        tokens,
        0,
        start,
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        unify,
        synth,
    )
}

/// Match pattern element `i` onward, with `positions` accumulated so far.
///
/// `pos` is Java's `nextPos`: the first candidate token of this element's
/// window. `eff_skip` is Java's `prevSkipNext`: the `skip` of the last
/// *matched* element, which persists across skipped optional elements (each
/// skipped optional element cancels its own advance through Java's
/// `minOccurSkip`, so the window stays anchored at the last match).
#[allow(clippy::too_many_arguments)]
fn try_from<T: Deref<Target = AnalyzedTokenReadings>>(
    pattern: &CompiledPattern,
    suggestions: &[Vec<SuggestionPart>],
    suppress_misspelled: &[bool],
    tokens: &[T],
    i: usize,
    pos: usize,
    eff_skip: i32,
    positions: Vec<Option<usize>>,
    starts: Vec<Option<usize>>,
    entries: Vec<Option<UnifyEntry>>,
    unify: Option<&crate::unify::EquivalenceConfig>,
    synth: Option<&dyn Synthesizer>,
) -> Option<PatternMatch> {
    if i == pattern.tokens.len() {
        let resolved =
            resolve_suggestions(suggestions, suppress_misspelled, tokens, &positions, synth);
        // LT `testUnification`: when the config is missing the caller does not
        // test unification at all (tests); the pipeline always passes one.
        let unified = match (pattern.has_unification, unify) {
            (true, Some(cfg)) => run_unification(pattern, tokens, &entries, cfg)?,
            _ => None,
        };
        return Some(PatternMatch {
            positions,
            starts,
            suggestions: resolved,
            unified,
        });
    }
    let base_token = &pattern.tokens[i];
    // `<match no="N">` elements are compiled per attempt against the
    // referenced token (`PatternTokenMatcher.resolveReference`)
    let first_matched = positions.iter().flatten().copied().next();
    let resolved;
    let token = if base_token.match_ref.is_some()
        || base_token.and_group.iter().any(|m| m.match_ref.is_some())
    {
        resolved = resolve_references(base_token, tokens, first_matched, synth)?;
        &resolved
    } else {
        base_token
    };
    let unify_active = token.unification.is_some() || token.unification_neutral;

    // optional elements (min="0") may match zero tokens (LT position 0).
    // Java only skips an element when a following element (or a consecutive
    // chain of optional elements ending in a mandatory one) matches at the
    // *same* candidate token; that decision is made inside the candidate loop
    // below. A blanket pre-skip would wrongly prefer the zero-token match
    // even for trailing optional elements.
    // Java `matchFrom` line 107: an unbounded (or past-the-end) skip window
    // is clamped to the end of the sentence before matching, and the clamped
    // value decides whether the `scope="next"` exception branch applies.
    let prev_skip = if i == 0 {
        0
    } else if eff_skip < 0 || pos + eff_skip as usize >= tokens.len() {
        (tokens.len().saturating_sub(pos)).saturating_sub(1) as i32
    } else {
        eff_skip
    };
    // candidate positions (the compact pattern view has no whitespace tokens)
    let mut candidates: Vec<usize> = Vec::new();
    if i == 0 {
        candidates.push(pos);
    } else {
        let gap_max = prev_skip.max(0) as usize;
        for skipped in 0..=gap_max {
            let cand = pos + skipped;
            if cand >= tokens.len() {
                break;
            }
            candidates.push(cand);
        }
    }

    // LT `AbstractPatternRulePerformer.prevMatched` is a field that is only
    // reset per pattern element: once a `scope="next"` exception matched a
    // candidate, all later candidates for this element fail immediately.
    let mut prev_matched = false;
    for cand in candidates {
        let Some(tr) = tokens.get(cand) else { break };
        if prev_skip != 0 {
            if i > 0 {
                if let Some(prev) = pattern.tokens.get(i - 1) {
                    let untagged = tr
                        .readings
                        .iter()
                        .all(|r| has_no_pos_tag(r.pos_tag.as_deref()));
                    prev_matched |= tr.readings.iter().any(|r| {
                        reading_scope_exception_matches(
                            prev,
                            "next",
                            r,
                            tr.whitespace_before,
                            untagged,
                        )
                    });
                }
            }
        } else if let Some(next_tr) = tokens.get(cand + 1) {
            let untagged = next_tr
                .readings
                .iter()
                .all(|r| has_no_pos_tag(r.pos_tag.as_deref()));
            prev_matched |= next_tr.readings.first().is_some_and(|first| {
                reading_scope_exception_matches(
                    token,
                    "next",
                    first,
                    next_tr.whitespace_before,
                    untagged,
                )
            });
        }
        if prev_matched {
            continue;
        }
        let self_ok = !(tr.is_whitespace || (pattern.skip_immunized && tr.is_immunized))
            && token_matches(token, tr)
            && !element_blocked(pattern, i, tokens, cand, prev_skip);
        if !token.optional {
            if !self_ok {
                continue;
            }
        } else {
            // LT min="0": prefer skipping the element when the following
            // element matches the same token; otherwise consume it (and
            // commit like Java's `break`).
            let next_ok = next_element_matches_at(
                pattern,
                i,
                tokens,
                cand,
                prev_skip,
                first_matched,
                synth,
                &mut prev_matched,
            );
            if self_ok && next_ok {
                let mut positions = positions;
                positions.push(None);
                let mut starts = starts;
                starts.push(None);
                let mut entries = entries;
                entries.push(None);
                return try_from(
                    pattern,
                    suggestions,
                    suppress_misspelled,
                    tokens,
                    i + 1,
                    pos,
                    prev_skip,
                    positions,
                    starts,
                    entries,
                    unify,
                    synth,
                );
            }
            if !self_ok && !next_ok {
                continue;
            }
            if !self_ok {
                let mut positions = positions;
                positions.push(None);
                let mut starts = starts;
                starts.push(None);
                let mut entries = entries;
                entries.push(None);
                return try_from(
                    pattern,
                    suggestions,
                    suppress_misspelled,
                    tokens,
                    i + 1,
                    pos,
                    prev_skip,
                    positions,
                    starts,
                    entries,
                    unify,
                    synth,
                );
            }
        }
        let mut idx = cand;
        // LT `skipMaxTokens`: an element with `maxOccurrence > 1` matches a
        // consecutive run of matching tokens (greedy, longest run wins).
        // Java routes each extension through `testAllReadings`, so a
        // `prevMatched` flag set by a `scope="next"` exception stops the run.
        if token.max_occurrence != 1 {
            let limit = if token.max_occurrence == -1 {
                usize::MAX
            } else {
                token.max_occurrence.max(1) as usize
            };
            let remaining = pattern.tokens.len() - i - 1;
            let mut consumed = 1usize;
            while consumed < limit && !prev_matched {
                let next = idx + 1;
                if next >= tokens.len().saturating_sub(remaining) {
                    break;
                }
                let Some(tr) = tokens.get(next) else { break };
                if !(tr.is_whitespace || (pattern.skip_immunized && tr.is_immunized))
                    && token_matches(token, tr)
                    && !element_blocked(pattern, i, tokens, next, prev_skip)
                {
                    idx = next;
                    consumed += 1;
                } else {
                    break;
                }
            }
        }
        let mut positions = positions;
        positions.push(Some(idx));
        let mut starts = starts;
        starts.push(Some(cand));
        let mut entries = entries;
        entries.push(if unify_active {
            unify_entry(token, &tokens[idx], idx)
        } else {
            None
        });
        return try_from(
            pattern,
            suggestions,
            suppress_misspelled,
            tokens,
            i + 1,
            idx + 1,
            token.skip,
            positions,
            starts,
            entries,
            unify,
            synth,
        );
    }
    None
}

/// LT `matchFrom` min="0" lookahead: does the following element (or a
/// consecutive chain of optional elements) match at the same token?
#[allow(clippy::too_many_arguments)]
fn next_element_matches_at<T: Deref<Target = AnalyzedTokenReadings>>(
    pattern: &CompiledPattern,
    i: usize,
    tokens: &[T],
    cand: usize,
    prev_skip: i32,
    first_matched: Option<usize>,
    synth: Option<&dyn Synthesizer>,
    prev_matched: &mut bool,
) -> bool {
    let Some(tr) = tokens.get(cand) else {
        return false;
    };
    if tr.is_whitespace || (pattern.skip_immunized && tr.is_immunized) {
        return false;
    }
    for k2 in i + 1..pattern.tokens.len() {
        let next_token = &pattern.tokens[k2];
        let resolved;
        let next_token = if next_token.match_ref.is_some() {
            let Some(r) = resolve_references(next_token, tokens, first_matched, synth) else {
                continue;
            };
            resolved = r;
            &resolved
        } else {
            next_token
        };
        let skip = if k2 > 0 {
            pattern.tokens[k2 - 1].skip
        } else {
            0
        };
        // Java `testAllReadings` runs for each looked-ahead element and its
        // `prevMatched` flag is a performer field: when the element has a
        // `scope="next"` exception matching the token after `cand`, the flag
        // stays set for the rest of the attempt and blocks the
        // `skipMaxTokens` extension of the *current* element (this is what
        // keeps HO_FA_TOT matching `Deixa sempre tot …` in Java). Java's
        // workaround only applies when `prevSkipNext == 0`; with a pending
        // `skip` it is condition 1 (the previous element's next-scoped
        // exception) that can set the flag.
        if *prev_matched
            || (prev_skip == 0 && next_scope_next_exception_hits(next_token, tokens, cand))
        {
            *prev_matched = true;
            return false;
        }
        if token_matches(next_token, tr) && !element_blocked(pattern, k2, tokens, cand, skip) {
            return true;
        }
        if !next_token.optional {
            break;
        }
    }
    false
}

/// Java `testAllReadings`'s `scope="next"` workaround: does `token` have a
/// next-scoped exception matching the first reading of `tokens[cand + 1]`?
fn next_scope_next_exception_hits<T: Deref<Target = AnalyzedTokenReadings>>(
    token: &CompiledToken,
    tokens: &[T],
    cand: usize,
) -> bool {
    let Some(next_tr) = tokens.get(cand + 1) else {
        return false;
    };
    let untagged = next_tr
        .readings
        .iter()
        .all(|r| has_no_pos_tag(r.pos_tag.as_deref()));
    next_tr.readings.first().is_some_and(|first| {
        reading_scope_exception_matches(token, "next", first, next_tr.whitespace_before, untagged)
    })
}
/// Collect the unification payload of one matched element (LT `toUnify` /
/// `neutralReadings` population in `testAllReadings`).
fn unify_entry(
    token: &CompiledToken,
    tr: &AnalyzedTokenReadings,
    idx: usize,
) -> Option<UnifyEntry> {
    if token.unification_neutral {
        return Some(UnifyEntry::Neutral(idx));
    }
    let untagged = tr
        .readings
        .iter()
        .all(|r| has_no_pos_tag(r.pos_tag.as_deref()));
    let readings: Vec<AnalyzedToken> = tr
        .readings
        .iter()
        .filter(|r| reading_matches(token, r, untagged))
        .cloned()
        .collect();
    if readings.is_empty() {
        None
    } else {
        Some(UnifyEntry::Readings(readings))
    }
}

/// LT `AbstractPatternRulePerformer.testUnification`: feed the collected
/// readings to the unifier in pattern order and produce the unified tokens
/// (used by the disambiguation `unify` action).
fn run_unification<T: Deref<Target = AnalyzedTokenReadings>>(
    pattern: &CompiledPattern,
    tokens: &[T],
    entries: &[Option<UnifyEntry>],
    config: &crate::unify::EquivalenceConfig,
) -> Option<Option<Vec<Vec<AnalyzedToken>>>> {
    let mut unifier = config.create_unifier();
    let mut unified: Option<Vec<Vec<AnalyzedToken>>> = None;
    for (i, token) in pattern.tokens.iter().enumerate() {
        match entries.get(i).and_then(|e| e.as_ref()) {
            Some(UnifyEntry::Neutral(tok_idx)) => {
                unifier.add_neutral_element(&tokens[*tok_idx].readings);
                continue;
            }
            Some(UnifyEntry::Readings(readings)) => {
                let Some(features) = token.unification.as_ref() else {
                    continue;
                };
                let mut any_matched = false;
                for (ri, reading) in readings.iter().enumerate() {
                    any_matched |= unifier.is_unified(reading, features, ri + 1 == readings.len());
                }
                if token.uni_negated && any_matched {
                    return None;
                }
                if token.last_in_unification {
                    if !any_matched && !token.uni_negated {
                        return None;
                    }
                    unified = unifier.get_unified_tokens();
                    unifier.reset();
                }
            }
            None => {}
        }
    }
    Some(unified)
}

/// LT `AbstractPatternRulePerformer`: chunk tags are tested with
/// `contains`/full-match against the token's chunk tags, XOR the token's
/// `negate` flag.
fn chunk_matches(token: &CompiledToken, tr: &AnalyzedTokenReadings) -> bool {
    let Some((spec, is_re)) = &token.chunk else {
        return true;
    };
    let hit = if *is_re {
        FancyRegex::new(&format!("^(?:{spec})$"))
            .map(|re| {
                tr.chunk_tags
                    .iter()
                    .any(|t| re.is_match(t).unwrap_or(false))
            })
            .unwrap_or(false)
    } else {
        tr.chunk_tags.iter().any(|t| t == spec)
    };
    hit ^ token.negate
}

/// Exceptions of `token` (own and AND-group members', like LT's
/// `isAndExceptionGroupMatched`) with the given scope matching `tokens[at]`.
/// Scope `""` means "current" (Java: neither `next` nor `previous`).
fn exceptions_match_scope<T: Deref<Target = AnalyzedTokenReadings>>(
    token: &CompiledToken,
    scope: &str,
    tokens: &[T],
    at: usize,
) -> bool {
    let Some(tr) = tokens.get(at) else {
        return false;
    };
    token
        .exceptions
        .iter()
        .chain(token.and_group.iter().flat_map(|m| m.exceptions.iter()))
        .any(|exc| {
            let exc_scope = match exc.scope.as_str() {
                "next" => "next",
                "previous" => "previous",
                _ => "current",
            };
            exc_scope == scope && exception_matches(exc, tr)
        })
}

/// LT `testAllReadings` exception semantics for a candidate token:
/// current-scope exceptions (own + AND members) block directly; a
/// `scope="next"` exception blocks the token after the candidate when the
/// previous element did not skip; with a skip gap it is consulted by the
/// following element instead (`PrevElement.isMatchedByScopeNextException`).
fn element_blocked<T: Deref<Target = AnalyzedTokenReadings>>(
    pattern: &CompiledPattern,
    i: usize,
    tokens: &[T],
    cand: usize,
    _prev_skip: i32,
) -> bool {
    let token = &pattern.tokens[i];
    if exceptions_match_scope(token, "current", tokens, cand) {
        return true;
    }
    if cand > 0 && exceptions_match_scope(token, "previous", tokens, cand - 1) {
        return true;
    }
    false
}

/// LT `CaseConversionHelper.convertCase`: `sample` is the original string
/// used for case preservation.
pub fn apply_case_conversion(value: &str, conversion: &str, sample: &str) -> String {
    let uppercase_first = |s: &str| -> String {
        let mut chars = s.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        }
    };
    match conversion {
        "allupper" => value.to_uppercase(),
        "alllower" => value.to_lowercase(),
        "startupper" => uppercase_first(value),
        "startlower" => {
            let mut chars = value.chars();
            match chars.next() {
                Some(c) => c.to_lowercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        }
        "firstupper" => uppercase_first(&value.to_lowercase()),
        "preserve" => {
            let sample_upper = !sample.is_empty()
                && sample.chars().next().is_some_and(|c| c.is_uppercase())
                && !sample
                    .chars()
                    .any(|c| c.is_alphabetic() && c.is_lowercase());
            if sample.chars().next().is_some_and(|c| c.is_uppercase()) {
                if sample_upper {
                    value.to_uppercase()
                } else {
                    uppercase_first(value)
                }
            } else {
                value.to_string()
            }
        }
        _ => value.to_string(),
    }
}

fn resolve_suggestions<T: Deref<Target = AnalyzedTokenReadings>>(
    suggestions: &[Vec<SuggestionPart>],
    suppress_misspelled: &[bool],
    tokens: &[T],
    positions: &[Option<usize>],
    synth: Option<&dyn Synthesizer>,
) -> Vec<String> {
    let mut out = Vec::new();
    for (sug_idx, parts) in suggestions.iter().enumerate() {
        let mut alternatives: Vec<String> = vec![String::new()];
        let mut skip_next_space = false;

        for (part_idx, part) in parts.iter().enumerate() {
            match part {
                SuggestionPart::Literal(text) => {
                    let text = if skip_next_space {
                        skip_next_space = false;
                        text.trim_start_matches([' ', '\t'])
                    } else {
                        text.as_str()
                    };
                    // Shield literal `\N` references with sentinels until
                    // the whole suggestion is assembled: appending a match
                    // result that starts with a digit would glue `\2` + `3`
                    // into `\33` (Java expands in place, we cannot).
                    let text = shield_literal_refs(text);
                    for alt in &mut alternatives {
                        alt.push_str(&text);
                    }
                }
                SuggestionPart::MatchRef(spec) => {
                    // Java `formatMatches`: an unmatched optional element
                    // contributes an empty string (collapsing a space, see
                    // `concatWithoutExtraSpace`), an out-of-range reference
                    // clamps to the last matched token.
                    skip_next_space = false;
                    let forms = render_match_ref(spec, tokens, positions, synth)
                        .unwrap_or_else(|| vec![String::new()]);
                    // Java `formatMatches`: a single empty replacement goes
                    // through `concatWithoutExtraSpace`. It removes one space
                    // before the reference when the remainder starts with
                    // `</suggestion>`/whitespace/`,:;.!?`, and (D-023) drops
                    // the space after the reference when it is at the
                    // suggestion start.
                    if forms.len() == 1 && forms[0].is_empty() {
                        let remainder_starts_with_punct = match parts.get(part_idx + 1) {
                            None => true,
                            Some(SuggestionPart::Literal(t)) => t.starts_with(|c: char| {
                                c.is_whitespace() || matches!(c, ',' | ':' | ';' | '.' | '!' | '?')
                            }),
                            Some(SuggestionPart::MatchRef(_)) => false,
                        };
                        if remainder_starts_with_punct {
                            for alt in &mut alternatives {
                                if alt.ends_with(' ') {
                                    alt.pop();
                                }
                            }
                        }
                        let next_starts_space = matches!(
                            parts.get(part_idx + 1),
                            Some(SuggestionPart::Literal(t))
                                if t.starts_with([' ', '\t'])
                        );
                        if next_starts_space && alternatives.iter().all(|alt| alt.is_empty()) {
                            skip_next_space = true;
                        }
                    }
                    let mut next = Vec::with_capacity(alternatives.len() * forms.len().max(1));
                    for alt in &alternatives {
                        for form in &forms {
                            next.push(format!("{alt}{form}"));
                        }
                    }
                    alternatives = next;
                }
            }
        }
        let tagger_mistake = suppress_misspelled.get(sug_idx).copied().unwrap_or(false)
            && synth.is_some_and(|synth| {
                parts.iter().any(|part| match part {
                    SuggestionPart::MatchRef(spec) => {
                        render_match_ref(spec, tokens, positions, Some(synth))
                            .is_some_and(|forms| forms.iter().any(|f| !synth.is_known_word(f)))
                    }
                    SuggestionPart::Literal(_) => false,
                })
            });
        for mut s in alternatives {
            // Java `MatchState.toFinalString` + `removeSuppressMisspelled`: a
            // suppressed element whose rendered form is unknown to the
            // tagger is a `<mistake/>` and the whole suggestion disappears.
            if tagger_mistake {
                continue;
            }
            // Java expands `\N` placeholders inside literal suggestion text
            // through `formatMatches` as well (out-of-range references clamp
            // to the last matched token).
            s = expand_suggestion_backrefs(&s, &|n| {
                fallback_placeholder_token(n, positions)
                    .and_then(|i| tokens.get(i))
                    .map(|t| t.surface().to_string())
            });
            // Java `removeSuppressMisspelled`: suggestions from elements
            // with `suppress_misspelled="yes"` that were not synthesized
            // (`(...)`) are dropped
            if suppress_misspelled.get(sug_idx).copied().unwrap_or(false)
                && s.contains('(')
                && s.contains(')')
            {
                continue;
            }
            // Java keeps empty replacements (`<suggestion></suggestion>`,
            // e.g. REPEATED_PLEASE's "remove one" suggestion)
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

/// `StringTools.addSpace(word, language)` for the non-French branch: a single
/// punctuation character takes no preceding space, everything else takes one.
fn add_space_for(word: &str) -> &'static str {
    let mut chars = word.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if matches!(c, '.' | ',' | ';' | ':' | '?' | '!') {
            return "";
        }
    }
    " "
}

/// `MatchState.toFinalString`: render one `<match/>` reference into its
/// possible strings. `None` = Java's `{""}` case for an unmatched optional
/// element (the caller collapses the surrounding space).
pub fn render_match_ref<T: Deref<Target = AnalyzedTokenReadings>>(
    spec: &MatchRefSpec,
    tokens: &[T],
    positions: &[Option<usize>],
    synth: Option<&dyn Synthesizer>,
) -> Option<Vec<String>> {
    let no = spec.no;
    let tok_idx = match positions.get(no.saturating_sub(1)).and_then(|p| *p) {
        Some(idx) => idx,
        None if no > positions.len() => positions.iter().flatten().next_back().copied()?,
        None => return None,
    };
    // Java `PatternRuleMatcher.concatMatches` for a `<phraseref>` element
    // (`phraseLen(index) > 1`): render each phrase token and join the
    // alternatives with `StringTools.addSpace`.
    if spec.phrase_len > 1 && (spec.include_skipped.is_empty() || spec.include_skipped == "none") {
        let mut groups: Vec<Vec<String>> = Vec::new();
        for k in 0..spec.phrase_len {
            let t = tokens.get(tok_idx + k)?;
            groups.push(render_match_ref_forms(spec, t, synth)?);
        }
        let mut combined = vec![String::new()];
        for (index, forms) in groups.iter().enumerate() {
            let mut next = Vec::new();
            for prefix in &combined {
                for form in forms {
                    if index == 0 {
                        next.push(form.clone());
                    } else {
                        next.push(format!("{prefix}{}{form}", add_space_for(form)));
                    }
                }
            }
            combined = next;
        }
        return Some(combined);
    }
    let tr = tokens.get(tok_idx)?;
    // `MatchState.setToken(tokens, index, next)`: skipped tokens are the ones
    // consumed by the *following* pattern element when it spans >1 token.
    let mut skipped = String::new();
    if spec.include_skipped != "none" && !spec.include_skipped.is_empty() {
        if let Some(next_idx) = positions.get(no).and_then(|p| *p) {
            let count = next_idx.saturating_sub(tok_idx);
            if count > 1 {
                for k in tok_idx + 1..tok_idx + count {
                    if let Some(t) = tokens.get(k) {
                        if t.whitespace_before
                            && !(k == tok_idx + 1 && spec.include_skipped == "following")
                        {
                            skipped.push(' ');
                        }
                        skipped.push_str(t.surface());
                    }
                }
            }
        }
    }
    let mut forms = if spec.include_skipped == "following" {
        // `formattedToken = null` in Java: only the skipped tokens remain
        vec![String::new()]
    } else {
        render_match_ref_forms(spec, tr, synth)?
    };
    if spec.include_skipped != "none" && !skipped.is_empty() {
        for form in &mut forms {
            form.push_str(&skipped);
        }
    }
    Some(forms)
}

/// `MatchState.toFinalString` for the match element itself (without the
/// `include_skipped` suffix).
fn render_match_ref_forms(
    spec: &MatchRefSpec,
    tr: &AnalyzedTokenReadings,
    synth: Option<&dyn Synthesizer>,
) -> Option<Vec<String>> {
    let matched_surface = tr.surface().to_string();

    // static lemma (`<match ...>word</match>`): formatted token is
    // `AnalyzedToken(lemma, postag, lemma)`
    let mut form = spec
        .static_lemma
        .clone()
        .unwrap_or_else(|| matched_surface.clone());
    if let (Some(m), Some(r)) = (&spec.regexp_match, &spec.regexp_replace) {
        form = regex_replace(&form, m, r);
    }
    let sample = matched_surface.clone();
    let case = spec.case_conversion.as_str();
    let Some(pos_tag) = spec.postag.as_deref() else {
        return Some(vec![apply_case_conversion(&form, case, &sample)]);
    };
    let Some(synth) = synth else {
        return Some(vec![apply_case_conversion(&form, case, &sample)]);
    };

    // readings used to derive the target POS tag and to synthesize
    let target_readings: &[AnalyzedToken] = &tr.readings;
    let form_readings: Vec<AnalyzedToken> = if spec.static_lemma.is_some() {
        vec![AnalyzedToken::new(
            form.clone(),
            spec.static_lemma.clone(),
            Some(pos_tag.to_string()),
        )]
    } else {
        tr.readings
            .iter()
            .map(|r| AnalyzedToken::new(r.token.clone(), r.stem.clone(), r.pos_tag.clone()))
            .collect()
    };

    let mut forms: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    if spec.postag_regexp {
        let re = fancy_regex::Regex::new(&format!("^(?:{pos_tag})$")).ok()?;
        let mut pos_tags: Vec<String> = target_readings
            .iter()
            .filter_map(|r| r.pos_tag.as_deref())
            .filter(|t| re.is_match(t).unwrap_or(false))
            .map(str::to_string)
            .collect();
        synth.sort_target_pos_tags(&mut pos_tags);

        // `MatchState.getTargetPosTag`: `synthesizer.getTargetPosTag` sorts
        // the list in place (the language hook above) and returns the last
        // entry (the fallback when the list is empty); sorting again here
        // would reorder the list for inconsistent comparators.
        let last_tag = || {
            pos_tags
                .last()
                .cloned()
                .unwrap_or_else(|| pos_tag.to_string())
        };
        let target = if spec.static_lemma.is_some() {
            let target = last_tag();
            match &spec.postag_replace {
                Some(replace) if !pos_tags.is_empty() => regex_replace(&target, pos_tag, replace),
                _ => target,
            }
        } else {
            let target = last_tag();
            match &spec.postag_replace {
                Some(replace) => {
                    let mut tags = pos_tags.clone();
                    if tags.is_empty() {
                        tags.push(target.clone());
                    }
                    let joined = tags
                        .iter()
                        .map(|t| {
                            let mut t = regex_replace(t, pos_tag, replace);
                            if spec.setpos {
                                t = synth.pos_tag_correction(&t);
                            }
                            t
                        })
                        .collect::<Vec<_>>()
                        .join("|");
                    joined
                }
                None => target,
            }
        };
        // Java `MatchState.toFinalString` (postagRegexp branch): readings
        // without a lemma keep the surface form; a SENT_START/SENT_END/
        // PARAGRAPH_END tag (e.g. a phrase's trailing `.`) sets `oneForm`,
        // which skips synthesis entirely.
        let mut one_form = false;
        for reading in &tr.readings {
            if reading.stem.is_none() {
                match reading.pos_tag.as_deref() {
                    None => {
                        forms.insert(matched_surface.clone());
                        one_form = true;
                    }
                    Some("SENT_START" | "SENT_END" | "PARAGRAPH_END") => {
                        if !one_form {
                            forms.insert(matched_surface.clone());
                        }
                        one_form = true;
                    }
                    Some(_) => one_form = false,
                }
            }
        }
        if !one_form {
            for reading in &form_readings {
                for f in synth.synthesize(reading, &target, true) {
                    forms.insert(f);
                }
            }
        }
        if forms.is_empty() {
            forms.insert(format!("({form})"));
        }
    } else {
        for reading in &form_readings {
            // Java `MatchState.toFinalString` (non-regexp branch) calls the
            // 2-arg `synthesize`; for Catalan that is the regexp-capable
            // override (`<match postag="N..P.*"/>` synthesizes the plural).
            for f in synth.synthesize_plain(reading, pos_tag) {
                forms.insert(f);
            }
        }
        if forms.is_empty() {
            // Java returns an empty array; keep the plain form so the
            // suggestion does not vanish entirely.
            forms.insert(form.clone());
        }
    }
    Some(
        forms
            .into_iter()
            .map(|f| apply_case_conversion(&f, case, &sample))
            .collect(),
    )
}

/// `StringTools`-compatible `regexp_match`/`regexp_replace`: Java `$N`
/// replacement back-references become `${N}` and the no-op flag group `(?)`
/// becomes `(?:)` for the regex crate.
fn regex_replace(s: &str, pattern: &str, replacement: &str) -> String {
    match java_regex(pattern) {
        Some(re) => {
            let normalized =
                normalize_java_replacement(replacement, re.captures_len().saturating_sub(1));
            re.replace_all(s, normalized.as_str()).into_owned()
        }
        None => s.to_string(),
    }
}

/// Byte range of a match within the sentence text (marker-aware). Unmatched
/// optional elements inside the marker range are collapsed away.
pub fn match_range<T: Deref<Target = AnalyzedTokenReadings>>(
    pattern: &CompiledPattern,
    tokens: &[T],
    m: &PatternMatch,
) -> TextRange {
    let (from_tok, to_tok) = match (pattern.marker_start, pattern.marker_end) {
        (Some(ms), Some(me)) if ms < m.positions.len() && me > ms => {
            let first = m.starts[ms..me.min(m.starts.len())]
                .iter()
                .flatten()
                .copied()
                .next();
            let last = m.positions[ms..me.min(m.positions.len())]
                .iter()
                .flatten()
                .copied()
                .next_back();
            match (first, last) {
                (Some(f), Some(l)) => (f, l),
                _ => (m.start_tok(), m.end_tok()),
            }
        }
        _ => (m.start_tok(), m.end_tok()),
    };
    let from = tokens[from_tok].start_pos;
    let to = tokens[to_tok].end_pos();
    TextRange::new(from, to.max(from))
}

impl CompiledPattern {
    /// LT `PatternRule.estimateContextForSureMatch` (antipattern terms are
    /// not tracked by the Rust loader and treated as 0).
    pub fn estimate_context_for_sure_match(&self) -> i32 {
        let mut extend_after_marker: i32 = 0;
        let mut marker_seen = false;
        for t in &self.tokens {
            if marker_seen && !t.in_marker {
                extend_after_marker += 1;
            }
            if t.sentence_end_postag {
                extend_after_marker += 1;
            }
            if t.in_marker {
                marker_seen = true;
            }
            if t.skip == -1 {
                return -1;
            }
            extend_after_marker += t.skip;
        }
        extend_after_marker
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_replacement_backslash_escapes_are_literal() {
        // `postag_replace='$1\1S$2'` (pt `GENERAL_VERB_AGREEMENT_ERRORS`):
        // Java's `Matcher` escapes the backslash, the regex crate keeps it.
        assert_eq!(normalize_java_replacement(r"$1\1S$2", 2), "${1}1S${2}");
        assert_eq!(normalize_java_replacement(r"\$$1", 1), "$$${1}");
    }

    #[test]
    fn java_surrogate_escapes_merge_to_code_points() {
        assert_eq!(
            normalize_java_surrogate_escapes("[\\ud83d\\udc00-\\ud83d\\udfff]+"),
            "[\\u{1F400}-\\u{1F7FF}]+"
        );
        // escaped backslashes and lone BMP escapes are untouched
        assert_eq!(
            normalize_java_surrogate_escapes("\\\\ud83d\\u2600"),
            "\\\\ud83d\\u2600"
        );
    }

    #[test]
    fn java_unicode_case_flag_groups_compile() {
        // Java `(?-iu)` / `(?iu)`: `UNICODE_CASE` has no Rust regex
        // equivalent; dropping `u` keeps the case-sensitivity semantics.
        assert_eq!(regex_replace("renvoyer", "(?-iu)er$", "é"), "renvoyé");
        assert_eq!(regex_replace("superstar", "super(?-iu)", "sur"), "surstar");
        assert_eq!(regex_replace("SUPER", "(?iu)super", "sur"), "sur");
        assert_eq!(regex_replace("axa", "a(?u)x", "b"), "ba");
    }

    /// Java `MatchState.toFinalString` renders an `include_skipped="following"`
    /// reference with no skipped tokens as `""`; `formatMatches` then runs
    /// `concatWithoutExtraSpace`, which removes the space before the
    /// reference (`com en va ser de fàcil ` + `""` -> `com en va ser de
    /// fàcil`). Regression: LO_MALA_PERSONA kept a trailing space.
    #[test]
    fn empty_following_match_ref_collapses_the_preceding_space() {
        let token = AnalyzedTokenReadings::new(vec![AnalyzedToken::new("fàcil", None, None)]);
        let tokens: Vec<&AnalyzedTokenReadings> = vec![&token];
        let positions = vec![Some(0usize)];
        let spec = MatchRefSpec {
            no: 1,
            phrase_len: 1,
            postag: None,
            postag_replace: None,
            postag_regexp: false,
            regexp_match: None,
            regexp_replace: None,
            case_conversion: String::new(),
            setpos: false,
            static_lemma: None,
            include_skipped: "following".to_string(),
            suppress_misspelled: false,
        };
        let suggestions = vec![vec![
            SuggestionPart::Literal("com en va ser de fàcil ".to_string()),
            SuggestionPart::MatchRef(spec),
        ]];
        let out = resolve_suggestions(&suggestions, &[false], &tokens, &positions, None);
        assert_eq!(out, vec!["com en va ser de fàcil".to_string()]);
    }

    #[test]
    fn emoji_antipattern_regex_compiles() {
        let re = compile_regex_uncached(
            "[\\ud83c\\udc00-\\ud83c\\udfff]+|[\\ud83d\\udc00-\\ud83d\\udfff]+|[\\u2600-\\u27ff]+",
            false,
            true,
        )
        .expect("emoji regex");
        assert!(re.is_match("🙂"));
        assert!(!re.is_match("🤀"));
        assert!(re.is_match("☀"));
    }
}
