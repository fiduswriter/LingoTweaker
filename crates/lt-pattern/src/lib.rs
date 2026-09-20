//! XML pattern-rule engine (legacy-compatible).
//!
//! v0 scope (P0.2): load `grammar.xml`/`style.xml` rule files into a typed
//! model, extract per-rule metadata and `<example>` corpora. The full
//! matcher (tokens, exceptions, `match`, unification, filters) is delivered
//! with P1.4 on top of this model.
use lt_data::PathExt as _;

use std::path::{Path, PathBuf};

use lt_core::Result;
use quick_xml::events::{attributes::Attribute, Event};
use quick_xml::Reader;
use serde::Serialize;

pub mod filters;
pub mod matcher;
pub mod string_matcher;
pub mod unify;

pub use filters::{
    resolve_args, skip_corrected_reference, FilterContext, FilterOutcome, FilterRegistry,
    FilterRegistryBuilder, RuleFilter,
};

pub use matcher::{
    compile_pattern, compile_patterns, expand_message_matches, find_matches,
    find_matches_with_synth, find_matches_with_unify, match_range, render_match_ref,
    CompiledPattern, PatternMatch, Synthesizer,
};
pub use unify::{EquivalenceConfig, Unifier};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Example {
    pub text: String,
    /// true for `type="correct"` examples.
    pub correct: bool,
    /// `type="triggers_error"` examples still count as incorrect but must
    /// match only after corrections are applied.
    pub triggers_error: bool,
    pub corrections: Vec<String>,
    /// disambiguation-example annotations (`type="ambiguous"`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputform: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleDef {
    /// `id` attribute of the `<rule>` or `<rulegroup>` element.
    pub id: String,
    /// Present for `<rule>` elements nested inside a `<rulegroup>`.
    pub sub_id: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    /// rule description (LT `name` attribute)
    pub name: Option<String>,
    /// LT `issueType` attribute (default "Other")
    pub issue_type: String,
    pub message: Option<String>,
    pub short: Option<String>,
    pub url: Option<String>,
    pub default_on: bool,
    /// enclosing category is default-off (`Category.isDefaultOff()`)
    pub category_default_on: bool,
    pub prio: Option<i32>,
    pub examples: Vec<Example>,
    pub token_count: usize,
    pub has_filter: bool,
    /// `<filter class args>` of this rule (P1.6 filter registry)
    pub filter: Option<FilterSpec>,
    /// rule uses chunk attributes (needs the chunking decision D-002)
    pub has_chunk_attrs: bool,
    /// `tags` of the rule/rulegroup/category (space separated in XML)
    pub tags: Vec<String>,
    /// `tone_tags` of the rule/rulegroup (`Rule.getToneTags`; in Java the
    /// category tone tags reach only the non-pattern rule branches, which is
    /// mirrored here)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tone_tags: Vec<String>,
    /// `is_goal_specific="true"` (`Rule.isGoalSpecific`)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub goal_specific: bool,
    /// `<rulegroup min_prev_matches="...">` / `<rule min_prev_matches="...">`
    /// (text-level repetition rules, `RepeatedPatternRuleTransformer`)
    pub min_prev_matches: i32,
    /// `<... distance_tokens="...">`; `< 1` means the Java default
    /// `60 * min_prev_matches`
    pub distance_tokens: i32,
    /// pattern uses unification or phrases; the engine cannot match it yet
    /// (`<and>`/`<or>` are expanded by `compile_patterns`)
    pub complex_pattern: bool,
    /// the compiled pattern (empty for regexp rules)
    pub pattern: Pattern,
    /// `<suggestion>` contents from the message
    pub suggestions: Vec<Vec<SuggestionPart>>,
    /// parallel to `suggestions`: the element had `suppress_misspelled="yes"`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestion_suppress: Vec<bool>,
    /// `<message suppress_misspelled="yes">`
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub message_suppress_misspelled: bool,
    /// `<match no="N">` elements of the message (offsets into `message`)
    pub message_match_refs: Vec<MessageMatchRef>,
    /// `<disambig>` actions (disambiguation.xml rules; one rule may carry
    /// several, each becoming a separate LT disambiguation rule)
    pub disambigs: Vec<DisambigAction>,
    /// `<antipattern>` sub-patterns of this rule
    pub antipatterns: Vec<Pattern>,
    /// true for standalone `<regexp>` rules.
    pub is_regexp_rule: bool,
    /// `<regexp>` spec of this rule (`is_regexp_rule`)
    pub regexp: Option<RegexpSpec>,
    pub source_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CategoryDef {
    pub id: String,
    pub name: String,
    /// LT `Category.isDefaultOff()` (`default="off"`; cannot be overridden
    /// by rule-level `default="on"`)
    pub default_on: bool,
}

/// A `<regexp>` rule: the pattern is matched directly against the sentence
/// text (LT `RegexPatternRule`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegexpSpec {
    pub pattern: String,
    pub case_sensitive: bool,
    /// LT `type="exact"`; the default `smart` replaces spaces outside
    /// character classes with `(?:[\s\u00A0\u202F]+)`
    pub exact: bool,
    /// capture group whose range is reported (LT `mark`; 0 = whole match)
    pub mark: usize,
}

/// A `<filter class="..." args="..."/>` element of a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FilterSpec {
    /// fully qualified Java class name
    pub class: String,
    /// raw `args` attribute (back-references unresolved)
    pub args: String,
}

/// A single `<exception>` inside a pattern token.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ExceptionSpec {
    pub text: Option<String>,
    pub regexp: bool,
    pub case_sensitive: bool,
    /// true when `case_sensitive` was set explicitly on this element
    #[serde(skip)]
    pub case_sensitive_set: bool,
    pub postag: Option<String>,
    pub postag_regexp: bool,
    pub negate: bool,
    pub negate_pos: bool,
    pub inflected: bool,
    /// LT `spacebefore` (test yes/no; `None` = ignore)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacebefore: Option<bool>,
    /// current (default) | next | previous
    pub scope: String,
}

/// One `<token>` of a `<pattern>` (matched against AnalyzedTokenReadings).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PatternToken {
    pub text: Option<String>,
    pub regexp: bool,
    pub case_sensitive: bool,
    /// true when `case_sensitive` was set explicitly on this element
    #[serde(skip)]
    pub case_sensitive_set: bool,
    pub postag: Option<String>,
    pub postag_regexp: bool,
    pub negate: bool,
    pub negate_pos: bool,
    /// match the base form (lemma) instead of the surface
    pub inflected: bool,
    /// chunk tag of the token (LT `chunk` attribute; D-002)
    pub chunk: Option<String>,
    /// chunk tag regex (LT `chunk_re` attribute; D-002)
    pub chunk_re: Option<String>,
    pub skip: Option<i32>,
    pub min: Option<i32>,
    pub max: Option<i32>,
    pub exceptions: Vec<ExceptionSpec>,
    /// true when inside `<marker>`
    pub in_marker: bool,
    /// LT `spacebefore` (test yes/no; `None` = ignore)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacebefore: Option<bool>,
    /// `<and>` sub-elements: every one of them (and the element itself) must
    /// match the same token (LT `andGroupList`, a logical AND over readings)
    pub and_group: Vec<PatternToken>,
    /// `<or>` sub-elements: alternative matches for this element. LT expands
    /// them into separate rules at load time (`PatternRuleHandler.createRules`);
    /// `lt-pattern` does the same in `compile_patterns`.
    pub or_group: Vec<PatternToken>,
    /// `<unify>` features of this element (LT `unificationFeatures`); an
    /// absent map means the element does not take part in unification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unification: Option<std::collections::BTreeMap<String, Vec<String>>>,
    /// `<unify negate="yes">`: the last element must not unify
    pub uni_negated: bool,
    /// the last element of a `<unify>` block (LT `lastInUnification`)
    pub last_in_unification: bool,
    /// `<unify-ignore>`: the element is added to the unified sequence as-is
    pub unification_neutral: bool,
    /// `<match no="N">` reference (unification/suggestion matches excluded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_ref: Option<MatchSpec>,
}

/// `<match>` reference inside a pattern token (Java `Match`): the token is
/// matched against the surface of an earlier matched token of the same rule
/// attempt (`no` is 0-based, relative to the match start).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchSpec {
    pub no: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_conversion: Option<String>,
    pub setpos: bool,
}

/// One `<equivalence>` of a top-level `<unification feature="...">`
/// definition: `<equivalence type="..."><token .../></equivalence>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquivalenceDef {
    pub feature: String,
    pub type_name: String,
    pub token: PatternToken,
}

/// Configuration of one `<match/>` element (LT `rules.patterns.Match`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MatchRefSpec {
    /// 1-based pattern token reference (`no` attribute)
    pub no: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postag_replace: Option<String>,
    pub postag_regexp: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regexp_match: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regexp_replace: Option<String>,
    /// Java `Match.CaseConversion` name (`""` = NONE)
    pub case_conversion: String,
    pub setpos: bool,
    /// text content of the element (Java `setLemmaString`): synthesize this
    /// static lemma instead of using the referenced token's surface
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_lemma: Option<String>,
    /// Java `include_skipped`: none | following | all
    pub include_skipped: String,
    /// suppress suggestions that are not tagged by the speller
    pub suppress_misspelled: bool,
}

/// Part of a `<suggestion>`: literal text or a `<match no>` token reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SuggestionPart {
    Literal(String),
    MatchRef(MatchRefSpec),
}

/// A `<match no="N">` element inside a rule message: the `\N` placeholder
/// carries the element's transforms (`PatternRuleMatcher.formatMatches`
/// applies them to the message text as well).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MessageMatchRef {
    /// Byte offset of the `\` placeholder in `RuleDef.message`.
    pub offset: usize,
    pub spec: MatchRefSpec,
}

/// A `<wd lemma pos>` child of a `<disambig>` element.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct WdReading {
    pub token: String,
    pub lemma: Option<String>,
    pub postag: Option<String>,
}

/// A `<match no="..." postag="..." postag_regexp="yes"/>` child of a
/// `<disambig>` element: keeps only the readings selected by the match
/// (`DisambiguationPatternRuleReplacer` REPLACE with a match element).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DisambigMatchFilter {
    pub postag: Option<String>,
    pub postag_regexp: bool,
    pub postag_replace: Option<String>,
    pub regexp_match: Option<String>,
    pub regexp_replace: Option<String>,
}

/// A `<disambig action="...">` element of a disambiguation rule.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DisambigAction {
    /// add | addchunk | filter | remove | replace | unify | immunize |
    /// ignore_spelling | filterall (missing action attr means replace)
    pub action: String,
    /// the `postag` attribute (disambiguatedPOS in LT)
    pub postag: Option<String>,
    pub new_readings: Vec<WdReading>,
    /// `<match .../>` child (Java `posSelector`)
    pub filter_match: Option<DisambigMatchFilter>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Pattern {
    pub tokens: Vec<PatternToken>,
    /// 0-based index of the first token inside `<marker>`, if any
    pub marker_start: Option<usize>,
    pub marker_end: Option<usize>,
    /// pattern uses unification/phrases → not matchable by the v1 engine
    pub complex: bool,
    /// `<pattern raw_pos="yes">`: Java matches this rule against
    /// `AnalyzedSentence.getPreDisambigTokensWithoutWhitespace()`
    /// (`PatternRule.isInterpretPosTagsPreDisambiguation`)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub raw_pos: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Grammar {
    pub rules: Vec<RuleDef>,
    pub categories: Vec<CategoryDef>,
    pub antipatterns: usize,
    pub regexp_blocks: usize,
    pub unification_features: usize,
    /// top-level `<unification>` equivalence definitions
    pub equivalence_defs: Vec<EquivalenceDef>,
}

impl Grammar {
    pub fn load_file(path: impl AsRef<Path>) -> Result<Self> {
        Loader::run(path.as_ref())
    }

    pub fn examples(&self) -> impl Iterator<Item = (&RuleDef, &Example)> {
        self.rules
            .iter()
            .flat_map(|r| r.examples.iter().map(move |e| (r, e)))
    }

    pub fn example_count(&self) -> usize {
        self.rules.iter().map(|r| r.examples.len()).sum()
    }

    pub fn incorrect_example_count(&self) -> usize {
        self.rules
            .iter()
            .map(|r| r.examples.iter().filter(|e| !e.correct).count())
            .sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PendingKind {
    #[default]
    Rule,
    Rulegroup,
    Regexp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupKind {
    And,
    Or,
}

/// Currently open `<and>`/`<or>` element. `base` is the index the group's
/// first token will get in the element list (Java `tokenCounter`), `count`
/// the number of tokens seen inside the group.
#[derive(Debug, Clone, Copy)]
struct OpenGroup {
    kind: GroupKind,
    base: usize,
    count: usize,
}

#[derive(Debug, Clone, Default)]
struct Pending {
    kind: PendingKind,
    id: String,
    sub_id: Option<String>,
    default_on: bool,
    prio: Option<i32>,
    message: Option<String>,
    short: Option<String>,
    url: Option<String>,
    examples: Vec<Example>,
    token_count: usize,
    filter: Option<FilterSpec>,
    name: Option<String>,
    issue_type: Option<String>,
    regexp: Option<RegexpSpec>,
    regex_buf: String,
    pattern: Pattern,
    suggestions: Vec<Vec<SuggestionPart>>,
    /// parallel to `suggestions`; filled when the element closes
    suggestion_suppress: Vec<bool>,
    /// `<message suppress_misspelled="yes">` (Java `isRuleSuppressMisspelled`)
    message_suppress_misspelled: bool,
    /// current `<suggestion suppress_misspelled="yes">`
    suggestion_suppress_current: bool,
    message_match_refs: Vec<MessageMatchRef>,
    current_token: Option<PatternToken>,
    current_exception: Option<ExceptionSpec>,
    current_suggestion: Option<Vec<SuggestionPart>>,
    suggestion_text_buf: String,
    in_pattern: bool,
    marker_depth: usize,
    has_chunk_attrs: bool,
    current_disambig: Option<DisambigAction>,
    current_wd: Option<WdReading>,
    current_antipattern: Option<Pattern>,
    antipatterns: Vec<Pattern>,
    disambigs: Vec<DisambigAction>,
    /// open `<and>`/`<or>` groups (schema forbids nesting, stack for safety)
    groups: Vec<OpenGroup>,
    /// `<pattern case_sensitive="...">` default inherited by tokens
    pattern_case_sensitive: bool,
    /// inside a `<unify>` block
    in_unification: bool,
    uni_negation: bool,
    /// inside `<unify-ignore>`
    in_unification_neutral: bool,
    /// features of the current `<unify>` block (feature id -> type ids)
    equivalence_features: std::collections::BTreeMap<String, Vec<String>>,
    current_feature: Option<String>,
    current_types: Vec<String>,
    /// text-level repetition attributes
    min_prev_matches: i32,
    distance_tokens: i32,
    /// rule/rulegroup/category `tags`
    tags: Vec<String>,
    /// rule/rulegroup `tone_tags`
    tone_tags: Vec<String>,
    /// `is_goal_specific` (rule > rulegroup > category)
    goal_specific: bool,
}

impl Pending {
    fn has_content(&self) -> bool {
        self.message.is_some()
            || self.short.is_some()
            || self.url.is_some()
            || !self.examples.is_empty()
            || self.token_count > 0
            || self.filter.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Capture {
    Message,
    Short,
    Url,
}

/// capture tuple for `<example>` elements: text, correct, triggers_error,
/// corrections, inputform, outputform
type ExampleCapture = (
    String,
    bool,
    bool,
    Vec<String>,
    Option<String>,
    Option<String>,
);

impl Loader {
    fn on_feature_element(&mut self, e: &quick_xml::events::BytesStart<'_>) {
        let id = self.attr_opt(e, b"id");
        if let Some(p) = self.pending.as_mut() {
            if p.in_unification {
                p.current_feature = id;
            }
        }
    }

    fn on_type_element(&mut self, e: &quick_xml::events::BytesStart<'_>) {
        let id = self.attr_opt(e, b"id");
        if let Some(p) = self.pending.as_mut() {
            if p.in_unification && p.current_feature.is_some() {
                if let Some(id) = id {
                    p.current_types.push(id);
                }
            }
        }
    }

    fn parse_token_attrs(
        &self,
        e: &quick_xml::events::BytesStart<'_>,
        default_case_sensitive: bool,
    ) -> (PatternToken, bool) {
        let mut token = PatternToken {
            case_sensitive: default_case_sensitive,
            ..PatternToken::default()
        };
        let mut has_chunk = false;
        for attr in e.attributes().flatten() {
            let key = attr.key.as_ref();
            let value = attr
                .unescape_value()
                .map(|s| s.into_owned())
                .unwrap_or_default();
            match key {
                b"text" => token.text = Some(normalize_text_pattern(&value)),
                b"regexp" => token.regexp = value == "yes",
                b"case_sensitive" => {
                    token.case_sensitive = value == "yes";
                    token.case_sensitive_set = true;
                }
                b"postag" => token.postag = Some(value),
                b"postag_regexp" => token.postag_regexp = value == "yes",
                b"negate" => token.negate = value == "yes",
                b"negate_pos" => token.negate_pos = value == "yes",
                b"inflected" => token.inflected = value == "yes",
                b"chunk" => {
                    token.chunk = Some(value.clone());
                    has_chunk = true;
                }
                b"chunk_re" => {
                    token.chunk_re = Some(value.clone());
                    has_chunk = true;
                }
                b"skip" => token.skip = value.parse::<i32>().ok(),
                b"min" => token.min = value.parse::<i32>().ok(),
                b"max" => token.max = value.parse::<i32>().ok(),
                b"spacebefore" if value != "ignore" => token.spacebefore = Some(value == "yes"),
                _ => {}
            }
        }
        (token, has_chunk)
    }

    fn parse_exception_attrs(&self, e: &quick_xml::events::BytesStart<'_>) -> ExceptionSpec {
        let mut exc = ExceptionSpec::default();
        for attr in e.attributes().flatten() {
            let key = attr.key.as_ref();
            let value = attr
                .unescape_value()
                .map(|s| s.into_owned())
                .unwrap_or_default();
            match key {
                b"text" => exc.text = Some(normalize_text_pattern(&value)),
                b"regexp" => exc.regexp = value == "yes",
                b"case_sensitive" => {
                    exc.case_sensitive = value == "yes";
                    exc.case_sensitive_set = true;
                }
                b"postag" => exc.postag = Some(value),
                b"postag_regexp" => exc.postag_regexp = value == "yes",
                b"negate" => exc.negate = value == "yes",
                b"negate_pos" => exc.negate_pos = value == "yes",
                b"inflected" => exc.inflected = value == "yes",
                b"spacebefore" if value != "ignore" => exc.spacebefore = Some(value == "yes"),
                b"scope" => exc.scope = value,
                _ => {}
            }
        }
        exc
    }

    fn parse_filter_attrs(&self, e: &quick_xml::events::BytesStart<'_>) -> FilterSpec {
        let mut class = String::new();
        let mut args = String::new();
        for attr in e.attributes().flatten() {
            match attr.key.as_ref() {
                b"class" => class = self.attr_value(&attr),
                b"args" => args = self.attr_value(&attr),
                _ => {}
            }
        }
        FilterSpec { class, args }
    }

    fn attr_opt(&self, e: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
        e.attributes()
            .flatten()
            .find(|a| a.key.as_ref() == key)
            .map(|a| self.attr_value(&a))
    }

    fn attr_value(&self, attr: &Attribute<'_>) -> String {
        match attr.unescape_value() {
            Ok(s) => s.into_owned(),
            Err(_) => {
                // custom DTD entities defeat quick-xml's unescape; expand
                // them from the raw bytes, then unescape the standard ones
                let mut s = String::from_utf8_lossy(&attr.value).into_owned();
                for _ in 0..5 {
                    match expand_entities(&s, &self.entities) {
                        Some(next) => s = next,
                        None => break,
                    }
                }
                match quick_xml::escape::unescape(&s) {
                    Ok(u) => u.into_owned(),
                    Err(_) => s,
                }
            }
        }
    }
}

/// `PatternRuleHandler`'s `is_goal_specific` attribute handling: only
/// `"true"`/`"false"` set the flag, anything else leaves it unset.
fn goal_specific_value(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn open_char(s: &str, from: usize) -> char {
    s[from..].chars().next().unwrap_or('"')
}

/// `StringTools.trimWhitespace` / `PatternToken.normalizeTextPattern`: Java
/// normalizes every token and exception string when the `PatternToken` is
/// constructed. Whitespace runs (two or more whitespace characters) are
/// dropped entirely — not collapsed — and isolated `\n`/`\t`/`\r` are
/// removed; a single space between two non-space characters survives. The
/// Portuguese `&subjunctive_verbs;`/`&compounds_with_bem;` entities span
/// multiple lines, so without this the leading indentation stays part of the
/// regexp and the alternative never matches (D-115).
fn normalize_text_pattern(s: &str) -> String {
    let trimmed = s.trim_matches(|c: char| c <= ' ');
    if !trimmed.chars().any(|c| c <= ' ') {
        return trimmed.to_string();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let mut out = String::with_capacity(trimmed.len());
    let mut i = 0usize;
    while i < chars.len() {
        while i < chars.len()
            && chars[i] <= ' '
            && ((i + 1 < chars.len() && chars[i + 1] <= ' ') || (i > 1 && chars[i - 1] <= ' '))
        {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let c = chars[i];
        if c != '\n' && c != '\t' && c != '\r' {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// Replace `&name;` references with the given entity values (custom DTD
/// entities; standard XML escapes are left for the caller's unescape step).
/// Returns `None` when nothing changed.
fn expand_entities(
    text: &str,
    entities: &std::collections::HashMap<String, String>,
) -> Option<String> {
    if !text.contains('&') || entities.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut changed = false;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        if let Some(end) = tail.find(';') {
            let name = &tail[1..end];
            if name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            {
                if let Some(value) = entities.get(name) {
                    out.push_str(value);
                    changed = true;
                    rest = &tail[end + 1..];
                    continue;
                }
            }
        }
        // not a known reference: keep as-is
        let skip = rest[pos..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&rest[pos..pos + skip]);
        rest = &rest[pos + skip..];
    }
    out.push_str(rest);
    if changed {
        Some(out)
    } else {
        None
    }
}

/// Java `hasPosixCharacterClass`: `\p{Lu}`/`\p{Ll}` imply case sensitivity.
/// Expand the internal-DTD `<!ENTITY name "value">` declarations of a rules
/// file in the document text (excluding the DOCTYPE), so entity values that
/// contain markup are parsed as elements like Java's SAX parser does.
///
/// Also resolves external parameter entities (`<!ENTITY % name SYSTEM "file">`
/// plus `%name;`, used by the Spanish rules files). The referenced file is
/// read relative to the XML file and then from the vendored `<lang>/words/`
/// location; its declarations join the entity set like Java's SAX parser
/// loading the external DTD subset.
fn preprocess_entities(text: &str, base_dir: Option<&Path>) -> String {
    let Some(dtd_start) = text.find("<!DOCTYPE") else {
        return text.to_string();
    };
    let Some(open) = text[dtd_start..].find('[') else {
        return text.to_string();
    };
    let Some(close) = text[dtd_start..].find("]>") else {
        return text.to_string();
    };
    let dtd = &text[dtd_start + open..dtd_start + close];
    // comments may contain commented-out `<!ENTITY>` declarations (e.g.
    // `abbreviated_negated_verbs` in en/disambiguation.xml); Java's SAX
    // parser ignores them, so the active declaration must win
    let dtd = strip_xml_comments(dtd);
    let mut entities: Vec<(String, String)> = Vec::new();
    collect_dtd_entities(&dtd, &mut entities);
    // external parameter entities (`%entities;`): merge the declarations of
    // the referenced files
    if let Some(base) = base_dir {
        let mut seen = std::collections::HashSet::new();
        for uri in parameter_entity_systems(&dtd) {
            seen.insert(uri.clone());
            collect_external_entities(&uri, base, &mut entities, &mut seen, 0);
        }
    }
    entities.sort_by_key(|(n, _)| std::cmp::Reverse(n.len()));
    let mut doc = text.to_string();
    if let Some(close_pos) = doc.find("]>") {
        let dtd_end = close_pos + 2;
        let head = doc[..dtd_end].to_string();
        let mut body = doc[dtd_end..].to_string();
        // PROTOTYPE: single-pass entity expansion (was O(entities × size) per
        // pass via `contains` + `replace`).
        let lookup: std::collections::HashMap<&str, &str> = entities
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        for _ in 0..5 {
            let mut out = String::with_capacity(body.len());
            let mut rest = body.as_str();
            let mut changed = false;
            while let Some(amp) = rest.find('&') {
                out.push_str(&rest[..amp]);
                let tail = &rest[amp..];
                let expanded = tail
                    .find(';')
                    .and_then(|semi| lookup.get(&tail[1..semi]).map(|value| (semi, *value)));
                if let Some((semi, value)) = expanded {
                    out.push_str(value);
                    rest = &tail[semi + 1..];
                    changed = true;
                } else {
                    out.push('&');
                    rest = &tail[1..];
                }
            }
            out.push_str(rest);
            body = out;
            if !changed {
                break;
            }
        }
        doc = head + &body;
    }
    doc
}

/// Parse the `<!ENTITY name "value">` declarations of a DTD fragment (the
/// internal subset or an external entities file).
fn collect_dtd_entities(dtd: &str, entities: &mut Vec<(String, String)>) {
    let mut rest: &str = dtd;
    while let Some(pos) = rest.find("<!ENTITY") {
        rest = &rest[pos + "<!ENTITY".len()..];
        let trimmed = rest.trim_start();
        // parameter entity declaration (`<!ENTITY % name SYSTEM "file">`) —
        // handled by `parameter_entity_systems`
        if trimmed.starts_with('%') {
            continue;
        }
        let name: String = trimmed.chars().take_while(|c| !c.is_whitespace()).collect();
        if name.is_empty() {
            continue;
        }
        let after = trimmed[name.len()..].trim_start();
        let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let Some(end) = after[1..].find(quote) else {
            continue;
        };
        let value = after[1..1 + end].to_string();
        rest = &after[1 + end..];
        if !entities.iter().any(|(n, _)| n == &name) {
            entities.push((name, value));
        }
    }
}

/// The `SYSTEM "..."` URIs of the parameter entity declarations.
fn parameter_entity_systems(dtd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = dtd;
    while let Some(pos) = rest.find("<!ENTITY") {
        rest = &rest[pos + "<!ENTITY".len()..];
        let trimmed = rest.trim_start();
        let Some(after_pct) = trimmed.strip_prefix('%') else {
            continue;
        };
        let name_len = after_pct
            .trim_start()
            .chars()
            .take_while(|c| !c.is_whitespace())
            .count();
        let after_name = after_pct.trim_start()[name_len..].trim_start();
        if let Some(sys) = after_name.strip_prefix("SYSTEM") {
            let sys = sys.trim_start();
            if let Some(quote) = sys.chars().next().filter(|c| *c == '"' || *c == '\'') {
                if let Some(end) = sys[1..].find(quote) {
                    out.push(sys[1..1 + end].to_string());
                }
            }
        }
    }
    out
}

/// Resolve an external parameter entity file: relative to the XML file, then
/// the vendored `<lang>/words/` directory (lt-sync vendored the upstream
/// `resource/<lang>/*.ent` files there), then the same directory as the XML.
fn resolve_entity_path(uri: &str, base_dir: &Path) -> Option<std::path::PathBuf> {
    let rel = uri.trim_start_matches('/');
    let direct = base_dir.join(rel);
    if direct.lt_exists() {
        return Some(direct);
    }
    let file_name = Path::new(rel).file_name()?;
    // vendored entity files: `lt-sync` maps upstream `resource/<lang>/*.ent`
    // to `<lang>/words/*.ent` (es) and `resource/<lang>/entities/*.ent` to
    // `<lang>/entities/*.ent` (pt). The XML either sits in the language
    // directory (`es/disambiguation.xml`), one level below it
    // (`es/rules/grammar.xml`) or one more level down for variant rule files
    // (`pt/rules/pt-BR/grammar.xml`), so look up to three levels up.
    let mut dir = Some(base_dir);
    while let Some(d) = dir {
        for candidate in [
            d.join("entities").join(file_name),
            d.join("words").join(file_name),
        ] {
            if candidate.lt_exists() {
                return Some(candidate);
            }
        }
        dir = d.parent();
    }
    let sibling = base_dir.join(file_name);
    if sibling.lt_exists() {
        return Some(sibling);
    }
    None
}

fn collect_external_entities(
    uri: &str,
    base_dir: &Path,
    entities: &mut Vec<(String, String)>,
    seen: &mut std::collections::HashSet<String>,
    depth: usize,
) {
    if depth > 4 {
        return;
    }
    let Some(path) = resolve_entity_path(uri, base_dir) else {
        return;
    };
    let Ok(text) = lt_data::fs::read_to_string(&path) else {
        return;
    };
    collect_dtd_entities(&text, entities);
    for nested in parameter_entity_systems(&text) {
        if seen.insert(nested.clone()) {
            collect_external_entities(&nested, &path, entities, seen, depth + 1);
        }
    }
}

/// Remove `<!-- ... -->` comments (unterminated comments drop the rest).
fn strip_xml_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start + 4..].find("-->") {
            Some(end) => rest = &rest[start + 4 + end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn posix_case_sensitive(text: Option<&str>) -> bool {
    text.map(|t| t.contains("\\p{Lu}") || t.contains("\\p{Ll}"))
        .unwrap_or(false)
}

/// Java `finalizeExceptions`: an exception without its own `case_sensitive`
/// inherits the token's setting; POSIX classes force case sensitivity.
fn inherit_exception_case(exc: &mut ExceptionSpec, token: &PatternToken) {
    if !exc.case_sensitive_set {
        exc.case_sensitive = token.case_sensitive;
    }
    if posix_case_sensitive(exc.text.as_deref()) {
        exc.case_sensitive = true;
    }
}

/// Java `XMLRuleHandler.addLegacyMatches`: the message's match list follows
/// the `\N` placeholders in message order; placeholders without an explicit
/// `<match>` element get a default (no POS/lemma) spec.
fn finalize_message_matches(p: &mut Pending) {
    let Some(message) = p.message.as_deref() else {
        return;
    };
    let refs = std::mem::take(&mut p.message_match_refs);
    let mut ordered: Vec<MessageMatchRef> = Vec::new();
    let mut ref_idx = 0usize;
    let bytes = message.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let explicit = refs
                .get(ref_idx)
                .filter(|r| r.offset == i)
                .map(|r| r.spec.clone());
            match explicit {
                Some(spec) => {
                    ordered.push(MessageMatchRef { offset: i, spec });
                    ref_idx += 1;
                }
                None => ordered.push(MessageMatchRef {
                    offset: i,
                    spec: MatchRefSpec::default(),
                }),
            }
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    p.message_match_refs = ordered;
}

/// Append a finalized token to `list`, or attach it as an `<and>`/`<or>`
/// member of the previous element when a group is open (LT `finalizeTokens`:
/// the first token of a group is a regular element, the others are group
/// members of it).
fn push_or_attach(list: &mut Vec<PatternToken>, token: PatternToken, groups: &mut [OpenGroup]) {
    if let Some(group) = groups.last_mut() {
        let is_member = group.count > 0;
        group.count += 1;
        if is_member {
            if let Some(last) = list.last_mut() {
                match group.kind {
                    GroupKind::And => last.and_group.push(token),
                    GroupKind::Or => last.or_group.push(token),
                }
                return;
            }
        }
    }
    list.push(token);
}

struct Loader {
    grammar: Grammar,
    source: PathBuf,
    path: Vec<String>,
    category_id: Option<String>,
    category_name: Option<String>,
    group: Option<(String, bool, Option<i32>)>,
    pending: Option<Pending>,
    example: Option<ExampleCapture>,
    captures: Vec<Capture>,
    /// depth of `<antipattern>` nesting; contents never attach to rules
    antipattern_depth: usize,
    /// `issueType` of the enclosing rulegroup (inherited by nested rules)
    group_issue_type: Option<String>,
    /// `name` of the enclosing rulegroup (inherited by nested rules)
    group_name: Option<String>,
    /// custom DTD entities from the document's internal subset
    /// (`<!ENTITY name "value">`), expanded lazily at use sites
    entities: std::collections::HashMap<String, String>,
    /// per-element content buffers (token/wd/exception texts); a single
    /// shared buffer would be clobbered by nested `<exception>` elements
    content_stack: Vec<String>,
    /// inside a top-level `<unification feature="...">` definition
    in_unification_def: bool,
    unification_feature: Option<String>,
    equivalence_type: Option<String>,
    def_current_token: Option<PatternToken>,
    group_min_prev_matches: i32,
    group_distance_tokens: i32,
    /// enclosing `<category default="off">`
    category_default_on: bool,
    /// `<category type="...">` (Java `categoryIssueType` fallback for rules)
    category_issue_type: Option<String>,
    /// `<category tags="...">` (Java adds them to every contained rule)
    category_tags: Vec<String>,
    /// 1-based subrule counter inside a `<rulegroup>`
    group_sub_counter: usize,
    /// `<rulegroup tags="...">`
    group_tags: Vec<String>,
    /// `<category tone_tags="...">`
    category_tone_tags: Vec<String>,
    /// `<category is_goal_specific="...">`
    category_goal_specific: Option<bool>,
    /// `<rulegroup tone_tags="...">`
    group_tone_tags: Vec<String>,
    /// `<rulegroup is_goal_specific="...">`
    group_goal_specific: Option<bool>,
    /// `<antipattern>` elements declared directly under `<rulegroup>`
    /// (inherited by every rule of the group, LT `getAntiPatterns`)
    group_antipatterns: Vec<Pattern>,
    /// inside `<match>...</match>`: text becomes the static lemma
    /// (Java `XMLRuleHandler.inMatch` / `setLemmaString`)
    in_match: bool,
    match_buf: String,
    /// inside `<phrases>`
    in_phrases: bool,
    /// id of the `<phrase>` currently being defined
    phrase_id: Option<String>,
    /// Java `phraseMap`: phrase id -> alternative token lists
    phrase_map: std::collections::HashMap<String, Vec<Vec<PatternToken>>>,
    /// tokens collected for the current phrase definition (Java's global
    /// `patternTokens` while `inPhrases`)
    phrase_tokens: Vec<PatternToken>,
    /// open `<and>`/`<or>` groups while collecting phrase tokens
    phrase_groups: Vec<OpenGroup>,
    /// token being finalized inside a phrase definition
    phrase_current_token: Option<PatternToken>,
    /// exception being finalized inside a phrase definition
    phrase_current_exception: Option<ExceptionSpec>,
    /// Java `phrasePatternTokens`: alternatives produced by `<phraseref>`
    phrase_pattern_tokens: Vec<Vec<PatternToken>>,
    /// Java `lastPhrase`: a `<phraseref>` was the last element, so the next
    /// `<token>` clears the accumulated tokens (which already became the
    /// expansion's prefix)
    last_phrase: bool,
}

impl Loader {
    fn run(path: &Path) -> Result<Grammar> {
        // Entity values may contain markup (`&it_s;` expands to a
        // `<suggestion>` element in Java SAX); preprocess the file so
        // quick-xml sees the expanded markup (`preprocess_entities`).
        let raw = lt_data::fs::read_to_string(path).map_err(|e| {
            lt_core::CoreError::Data(format!("cannot open {}: {e}", path.display()))
        })?;
        let expanded = preprocess_entities(&raw, path.parent());
        let mut reader = Reader::from_str(&expanded);
        reader.config_mut().trim_text(false);
        let mut loader = Loader {
            grammar: Grammar::default(),
            source: path.to_path_buf(),
            path: Vec::new(),
            category_id: None,
            category_name: None,
            group: None,
            pending: None,
            example: None,
            captures: Vec::new(),
            group_issue_type: None,
            group_name: None,
            antipattern_depth: 0,
            entities: std::collections::HashMap::new(),
            content_stack: Vec::new(),
            in_unification_def: false,
            unification_feature: None,
            equivalence_type: None,
            def_current_token: None,
            group_min_prev_matches: 0,
            group_distance_tokens: 0,
            category_default_on: true,
            category_issue_type: None,
            category_tags: Vec::new(),
            group_sub_counter: 0,
            group_tags: Vec::new(),
            category_tone_tags: Vec::new(),
            category_goal_specific: None,
            group_tone_tags: Vec::new(),
            group_goal_specific: None,
            group_antipatterns: Vec::new(),
            in_match: false,
            match_buf: String::new(),
            in_phrases: false,
            phrase_id: None,
            phrase_map: std::collections::HashMap::new(),
            phrase_tokens: Vec::new(),
            phrase_groups: Vec::new(),
            phrase_current_token: None,
            phrase_current_exception: None,
            phrase_pattern_tokens: Vec::new(),
            last_phrase: false,
        };

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => loader.on_start(&e)?,
                Ok(Event::DocType(d)) => loader.on_doctype(&d),
                Ok(Event::Empty(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                    match name.as_str() {
                        "token" => loader.on_token_empty(&e),
                        "match" => {
                            if loader
                                .pending
                                .as_ref()
                                .is_some_and(|p| p.current_disambig.is_some())
                            {
                                loader.on_disambig_match_empty(&e);
                            } else {
                                loader.on_match_element(&e, true);
                            }
                        }
                        "phraseref" => {
                            if let Some(idref) = loader.attr_opt(&e, b"idref") {
                                loader.prepare_phrase(&idref);
                            }
                        }
                        "suggestion" => {
                            // self-closing `<suggestion/>` = empty replacement
                            let in_message = loader.captures.contains(&Capture::Message);
                            let suppress = e
                                .attributes()
                                .flatten()
                                .find(|a| a.key.as_ref() == b"suppress_misspelled")
                                .is_some_and(|a| loader.attr_value(&a) == "yes");
                            if let Some(p) = loader.pending.as_mut() {
                                p.suggestions.push(Vec::new());
                                p.suggestion_suppress.push(suppress);
                                if in_message {
                                    p.message
                                        .get_or_insert_with(String::new)
                                        .push_str("<suggestion></suggestion>");
                                }
                            }
                        }
                        "filter" if loader.antipattern_depth == 0 => {
                            let spec = loader.parse_filter_attrs(&e);
                            if let Some(p) = loader.pending.as_mut() {
                                p.filter = Some(spec);
                            }
                        }
                        "exception" => {
                            let mut exc = loader.parse_exception_attrs(&e);
                            if let Some(token) = loader.phrase_current_token.as_mut() {
                                inherit_exception_case(&mut exc, token);
                                token.exceptions.push(exc);
                            } else if let Some(p) = loader.pending.as_mut() {
                                if let Some(token) = p.current_token.as_mut() {
                                    inherit_exception_case(&mut exc, token);
                                    token.exceptions.push(exc);
                                }
                            }
                        }
                        "feature" => {
                            // `<feature id="x"/>` is self-closing: commit it
                            loader.on_feature_element(&e);
                            if let Some(p) = loader.pending.as_mut() {
                                if let Some(feature) = p.current_feature.take() {
                                    p.equivalence_features
                                        .insert(feature, std::mem::take(&mut p.current_types));
                                }
                            }
                        }
                        "type" => loader.on_type_element(&e),
                        "unify-ignore" => {
                            if let Some(p) = loader.pending.as_mut() {
                                if p.in_unification {
                                    p.in_unification_neutral = true;
                                }
                            }
                        }
                        "disambig" => {
                            let mut action = String::from("replace");
                            let mut postag = None;
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"action" => action = loader.attr_value(&attr).to_lowercase(),
                                    b"postag" => postag = Some(loader.attr_value(&attr)),
                                    _ => {}
                                }
                            }
                            if let Some(p) = loader.pending.as_mut() {
                                p.disambigs.push(DisambigAction {
                                    action,
                                    postag,
                                    new_readings: Vec::new(),
                                    filter_match: None,
                                });
                            }
                        }
                        "wd" => {
                            let mut wd = WdReading::default();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"lemma" => wd.lemma = Some(loader.attr_value(&attr)),
                                    b"pos" => wd.postag = Some(loader.attr_value(&attr)),
                                    _ => {}
                                }
                            }
                            if let Some(p) = loader.pending.as_mut() {
                                if let Some(d) = p.current_disambig.as_mut() {
                                    d.new_readings.push(wd);
                                }
                            }
                        }

                        _ => {}
                    }
                }
                Ok(Event::Text(t)) => {
                    let raw = String::from_utf8_lossy(t.as_ref()).into_owned();
                    loader.on_text(&loader.prepare_text(&raw))?;
                }
                Ok(Event::CData(t)) => {
                    loader.on_text(&loader.prepare_text(&String::from_utf8_lossy(&t)))?;
                }
                Ok(Event::End(e)) => loader.on_end(e.name().as_ref())?,
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(lt_core::CoreError::Parse(
                        path.display().to_string(),
                        e.to_string(),
                    ))
                }
                _ => {}
            }
            buf.clear();
        }
        loader.finish_pending();
        Ok(loader.grammar)
    }

    fn on_start(&mut self, e: &quick_xml::events::BytesStart<'_>) -> Result<()> {
        let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
        match name.as_str() {
            "category" => {
                let mut id = None;
                let mut cname = None;
                let mut default_off = false;
                let mut category_issue_type = None;
                let mut category_tags: Vec<String> = Vec::new();
                let mut category_tone_tags: Vec<String> = Vec::new();
                let mut category_goal_specific = None;
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"id" => id = Some(self.attr_value(&attr)),
                        b"name" => cname = Some(self.attr_value(&attr)),
                        b"default" => default_off = self.attr_value(&attr) == "off",
                        b"type" => category_issue_type = Some(self.attr_value(&attr)),
                        b"tags" => {
                            category_tags = self
                                .attr_value(&attr)
                                .split_whitespace()
                                .map(str::to_string)
                                .collect();
                        }
                        b"tone_tags" => {
                            category_tone_tags = self
                                .attr_value(&attr)
                                .split_whitespace()
                                .map(str::to_string)
                                .collect();
                        }
                        b"is_goal_specific" => {
                            category_goal_specific = goal_specific_value(&self.attr_value(&attr));
                        }
                        _ => {}
                    }
                }
                if let (Some(id), Some(cname)) = (id, cname) {
                    let def = CategoryDef {
                        id: id.clone(),
                        name: cname.clone(),
                        default_on: !default_off,
                    };
                    if !self.grammar.categories.contains(&def) {
                        self.grammar.categories.push(def);
                    }
                    self.category_id = Some(id);
                    self.category_name = Some(cname);
                    self.category_default_on = !default_off;
                    self.category_issue_type = category_issue_type;
                    self.category_tags = category_tags;
                    self.category_tone_tags = category_tone_tags;
                    self.category_goal_specific = category_goal_specific;
                }
            }
            "rulegroup" => {
                self.finish_pending();
                let id = self.attr_opt(e, b"id").unwrap_or_default();
                let mut default_on = true;
                let mut prio = None;
                let mut issue_type = None;
                let mut gname = None;
                self.group_tags = self.category_tags.clone();
                self.group_tone_tags.clear();
                self.group_goal_specific = None;
                self.group_antipatterns.clear();
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        // Java `PatternRuleHandler`: `default="off"` and
                        // `default="temp_off"` are both off by default
                        // (`setDefaultOff` / `setDefaultTempOff`).
                        b"default" => {
                            let value = self.attr_value(&attr);
                            default_on = value != "off" && value != "temp_off";
                        }
                        b"prio" => prio = self.attr_value(&attr).parse::<i32>().ok(),
                        b"type" => issue_type = Some(self.attr_value(&attr)),
                        b"name" => gname = Some(self.attr_value(&attr)),
                        b"tags" => {
                            self.group_tags = self
                                .attr_value(&attr)
                                .split_whitespace()
                                .map(str::to_string)
                                .collect();
                        }
                        b"tone_tags" => {
                            self.group_tone_tags = self
                                .attr_value(&attr)
                                .split_whitespace()
                                .map(str::to_string)
                                .collect();
                        }
                        b"is_goal_specific" => {
                            self.group_goal_specific = goal_specific_value(&self.attr_value(&attr));
                        }
                        b"min_prev_matches" => {
                            self.group_min_prev_matches =
                                self.attr_value(&attr).parse::<i32>().unwrap_or(0);
                        }
                        b"distance_tokens" => {
                            self.group_distance_tokens =
                                self.attr_value(&attr).parse::<i32>().unwrap_or(0);
                        }
                        _ => {}
                    }
                }
                self.group_issue_type = issue_type.clone();
                self.group_name = gname.clone();
                self.group = Some((id.clone(), default_on, prio));
                self.group_sub_counter = 0;
                if let Some(p) = self.pending.as_mut() {
                    p.tags = self.group_tags.clone();
                }
                self.pending = Some(Pending {
                    kind: PendingKind::Rulegroup,
                    id,
                    sub_id: None,
                    default_on,
                    prio,
                    name: gname,
                    issue_type,
                    min_prev_matches: self.group_min_prev_matches,
                    distance_tokens: self.group_distance_tokens,
                    ..Pending::default()
                });
            }
            "rule" => {
                if let Some(p) = self.pending.take() {
                    if p.kind != PendingKind::Rulegroup {
                        self.emit(p);
                    }
                }
                let id = self.attr_opt(e, b"id").unwrap_or_default();
                let mut default_on = self.group.as_ref().map(|g| g.1).unwrap_or(true);
                let mut prio = self.group.as_ref().and_then(|g| g.2);
                let mut issue_type = self
                    .group_issue_type
                    .clone()
                    .or_else(|| self.category_issue_type.clone());
                let mut name = self.group_name.clone();
                let mut min_prev_matches = self.group_min_prev_matches;
                let mut distance_tokens = self.group_distance_tokens;
                let mut tags = self.group_tags.clone();
                // Java `PatternRuleHandler`: the rule's `tone_tags` are added
                // to the rulegroup ones; `is_goal_specific` falls back from
                // rule to rulegroup to category.
                let mut tone_tags = self.group_tone_tags.clone();
                let mut goal_specific = self
                    .group_goal_specific
                    .or(self.category_goal_specific)
                    .unwrap_or(false);
                let antipatterns = if self.group.is_some() {
                    self.group_antipatterns.clone()
                } else {
                    Vec::new()
                };
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"default" => {
                            let value = self.attr_value(&attr);
                            default_on = value != "off" && value != "temp_off";
                        }
                        b"prio" => prio = self.attr_value(&attr).parse::<i32>().ok(),
                        b"type" => issue_type = Some(self.attr_value(&attr)),
                        b"name" => name = Some(self.attr_value(&attr)),
                        b"tags" => {
                            tags = self
                                .attr_value(&attr)
                                .split_whitespace()
                                .map(str::to_string)
                                .collect();
                        }
                        b"tone_tags" => {
                            for tag in self.attr_value(&attr).split_whitespace() {
                                let tag = tag.to_string();
                                if !tone_tags.contains(&tag) {
                                    tone_tags.push(tag);
                                }
                            }
                        }
                        b"is_goal_specific" => {
                            goal_specific =
                                goal_specific_value(&self.attr_value(&attr)).unwrap_or(false)
                        }
                        b"min_prev_matches" => {
                            min_prev_matches = self.attr_value(&attr).parse::<i32>().unwrap_or(0)
                        }
                        b"distance_tokens" => {
                            distance_tokens = self.attr_value(&attr).parse::<i32>().unwrap_or(0)
                        }
                        _ => {}
                    }
                }
                // Java `PatternRuleHandler`: a nested rule's own `id` becomes
                // the rule id (group id otherwise); the sub id is the 1-based
                // counter within the group ("1" for standalone rules)
                let (rule_id, sub_id) = match &self.group {
                    Some((gid, ..)) => {
                        self.group_sub_counter += 1;
                        let rid = if id.is_empty() { gid.clone() } else { id };
                        (rid, Some(self.group_sub_counter.to_string()))
                    }
                    None => (id, Some("1".to_string())),
                };
                self.pending = Some(Pending {
                    kind: PendingKind::Rule,
                    id: rule_id,
                    sub_id,
                    default_on,
                    prio,
                    name,
                    issue_type,
                    min_prev_matches,
                    distance_tokens,
                    tags: {
                        let mut t = tags;
                        for tag in &self.category_tags {
                            if !t.contains(tag) {
                                t.push(tag.clone());
                            }
                        }
                        t
                    },
                    // Java `PatternRuleHandler`:
                    // `rule.addToneTags(categoryToneTags)`; rules in
                    // tone-tag categories are goal-specific-inactive without
                    // a matching goal.
                    tone_tags: {
                        let mut t = tone_tags;
                        for tag in &self.category_tone_tags {
                            if !t.contains(tag) {
                                t.push(tag.clone());
                            }
                        }
                        t
                    },
                    goal_specific,
                    antipatterns,
                    ..Pending::default()
                });
            }
            "regexp" => {
                self.grammar.regexp_blocks += 1;
                let mut case_sensitive = false;
                let mut exact = false;
                let mut mark = 0usize;
                for attr in e.attributes().flatten() {
                    let value = self.attr_value(&attr);
                    match attr.key.as_ref() {
                        b"case_sensitive" => case_sensitive = value == "yes",
                        b"type" => exact = value == "exact",
                        b"mark" => mark = value.parse::<usize>().unwrap_or(0),
                        _ => {}
                    }
                }
                let spec = RegexpSpec {
                    pattern: String::new(),
                    case_sensitive,
                    exact,
                    mark,
                };
                // the `<regexp>` element lives inside `<rule>`: convert the
                // (still empty) rule pending in place so id/sub_id/examples
                // are kept
                match self.pending.as_mut() {
                    Some(p) if p.kind == PendingKind::Rule && !p.has_content() => {
                        p.kind = PendingKind::Regexp;
                        p.regexp = Some(spec);
                    }
                    _ => {
                        self.finish_pending();
                        self.pending = Some(Pending {
                            kind: PendingKind::Regexp,
                            regexp: Some(spec),
                            ..Pending::default()
                        });
                    }
                }
            }
            "example" => {
                if self.antipattern_depth > 0 {
                    return Ok(());
                }
                let mut correct = false;
                let mut triggers = false;
                let mut corrections = Vec::new();
                let mut inputform = None;
                let mut outputform = None;
                for attr in e.attributes().flatten() {
                    let key = attr.key.as_ref();
                    let value = self.attr_value(&attr);
                    match key {
                        b"type" => {
                            correct = value == "correct";
                            triggers = value == "triggers_error";
                        }
                        b"correction" | b"corrections" => {
                            corrections = value
                                .split('|')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        b"inputform" => inputform = Some(value),
                        b"outputform" => outputform = Some(value),
                        _ => {}
                    }
                }
                self.example = Some((
                    String::new(),
                    correct,
                    triggers,
                    corrections,
                    inputform,
                    outputform,
                ));
            }
            "token" => {
                let default_case = self
                    .pending
                    .as_ref()
                    .map(|p| p.pattern_case_sensitive)
                    .unwrap_or(false);
                self.drop_tokens_after_phrase();
                let (mut token, has_chunk) = self.parse_token_attrs(e, default_case);
                if self.in_unification_def {
                    self.content_stack.push(String::new());
                    self.def_current_token = Some(token);
                    self.path.push(name);
                    return Ok(());
                }
                if self.phrase_id.is_some() {
                    // token inside a `<phrase>` definition (Java collects
                    // these in the global `patternTokens` via `setToken`)
                    token.in_marker = false;
                    self.content_stack.push(String::new());
                    self.phrase_current_token = Some(token);
                    self.path.push(name);
                    return Ok(());
                }
                if let Some(p) = self.pending.as_mut() {
                    if self.antipattern_depth > 0 {
                        token.in_marker = p.marker_depth > 0;
                        self.content_stack.push(String::new());
                        p.current_token = Some(token);
                    } else if p.in_pattern {
                        token.in_marker = p.marker_depth > 0;
                        p.has_chunk_attrs |= has_chunk;
                        self.content_stack.push(String::new());
                        p.current_token = Some(token);
                    } else {
                        p.token_count += 1;
                    }
                }
            }
            "exception" => {
                let mut exc = self.parse_exception_attrs(e);
                self.content_stack.push(String::new());
                if let Some(token) = self.phrase_current_token.as_mut() {
                    inherit_exception_case(&mut exc, token);
                    token.exceptions.push(exc.clone());
                    self.phrase_current_exception = Some(exc);
                    self.path.push(name);
                    return Ok(());
                }
                if let Some(p) = self.pending.as_mut() {
                    if let Some(token) = p.current_token.as_mut() {
                        inherit_exception_case(&mut exc, token);
                        token.exceptions.push(exc.clone());
                    }
                    p.current_exception = Some(exc);
                }
            }
            "pattern" => {
                let case_sensitive = self.attr_opt(e, b"case_sensitive").as_deref() == Some("yes");
                let raw_pos = self.attr_opt(e, b"raw_pos").as_deref() == Some("yes");
                if let Some(p) = self.pending.as_mut() {
                    p.in_pattern = true;
                    p.pattern_case_sensitive = case_sensitive;
                    // Java PatternRuleHandler reads the flag at `<pattern>`
                    // start; antipatterns always match the disambiguated view
                    // (their matcher is a DisambiguationPatternRule, not a
                    // PatternRule)
                    p.pattern.raw_pos = raw_pos;
                }
            }
            "marker" => {
                if let Some(p) = self.pending.as_mut() {
                    if p.in_pattern || self.antipattern_depth > 0 {
                        p.marker_depth += 1;
                        if p.marker_depth == 1 {
                            // Java `tokenCounter`: an open group counts as the
                            // element its first token will become
                            let list_len = if self.antipattern_depth > 0 {
                                p.current_antipattern
                                    .as_ref()
                                    .map(|a| a.tokens.len())
                                    .unwrap_or(0)
                            } else {
                                p.pattern.tokens.len()
                            };
                            let idx = p.groups.last().map(|g| g.base).unwrap_or(list_len);
                            if self.antipattern_depth > 0 {
                                if let Some(anti) = p.current_antipattern.as_mut() {
                                    anti.marker_start = Some(idx);
                                }
                            } else {
                                p.pattern.marker_start = Some(idx);
                            }
                        }
                    }
                }
            }
            "and" | "or" => {
                if self.phrase_id.is_some() {
                    self.phrase_groups.push(OpenGroup {
                        kind: if name == "and" {
                            GroupKind::And
                        } else {
                            GroupKind::Or
                        },
                        base: self.phrase_tokens.len(),
                        count: 0,
                    });
                } else if let Some(p) = self.pending.as_mut() {
                    if p.in_pattern || self.antipattern_depth > 0 {
                        let base = if self.antipattern_depth > 0 {
                            p.current_antipattern
                                .as_ref()
                                .map(|a| a.tokens.len())
                                .unwrap_or(0)
                        } else {
                            p.pattern.tokens.len()
                        };
                        p.groups.push(OpenGroup {
                            kind: if name == "and" {
                                GroupKind::And
                            } else {
                                GroupKind::Or
                            },
                            base,
                            count: 0,
                        });
                    }
                }
            }
            "unify" => {
                let negate = self.attr_opt(e, b"negate").as_deref() == Some("yes");
                if let Some(p) = self.pending.as_mut() {
                    if p.in_pattern || self.antipattern_depth > 0 {
                        p.in_unification = true;
                        p.uni_negation = negate;
                        p.equivalence_features.clear();
                    }
                }
            }
            "unify-ignore" => {
                if let Some(p) = self.pending.as_mut() {
                    if p.in_unification {
                        p.in_unification_neutral = true;
                    }
                }
            }
            "feature" => self.on_feature_element(e),
            "type" => self.on_type_element(e),
            "phrase" | "phraseref" => {
                if name == "phrase" {
                    if self.in_phrases {
                        self.phrase_id = self.attr_opt(e, b"id");
                        self.phrase_tokens.clear();
                        self.phrase_groups.clear();
                        self.phrase_pattern_tokens.clear();
                    }
                } else if let Some(idref) = self.attr_opt(e, b"idref") {
                    self.prepare_phrase(&idref);
                }
            }
            "phrases" => {
                self.in_phrases = true;
            }
            "includephrases" => {}
            "suggestion" => {
                let in_message = self.captures.contains(&Capture::Message);
                let suppress = e
                    .attributes()
                    .flatten()
                    .find(|a| a.key.as_ref() == b"suppress_misspelled")
                    .is_some_and(|a| self.attr_value(&a) == "yes");
                if let Some(p) = self.pending.as_mut() {
                    if p.current_suggestion.is_none() {
                        p.current_suggestion = Some(Vec::new());
                        p.suggestion_text_buf.clear();
                        p.suggestion_suppress_current = suppress;
                    }
                    // Java keeps the `<suggestion>` markup in the message
                    if in_message {
                        p.message
                            .get_or_insert_with(String::new)
                            .push_str("<suggestion>");
                    }
                }
            }
            "match" => {
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.current_disambig.is_some())
                {
                    self.on_disambig_match_empty(e);
                } else {
                    self.on_match_element(e, false);
                }
            }
            "filter" if self.antipattern_depth == 0 => {
                let spec = self.parse_filter_attrs(e);
                if let Some(p) = self.pending.as_mut() {
                    p.filter = Some(spec);
                }
            }
            "antipattern" => {
                self.grammar.antipatterns += 1;
                self.antipattern_depth += 1;
                // Java `case ANTIPATTERN`: `case_sensitive` applies to the
                // antipattern's tokens like on `<pattern>`
                let case_sensitive = self.attr_opt(e, b"case_sensitive").as_deref() == Some("yes");
                if let Some(p) = self.pending.as_mut() {
                    p.current_antipattern = Some(Pattern::default());
                    p.pattern_case_sensitive = case_sensitive;
                }
            }
            "disambig" => {
                let mut action = String::from("replace");
                let mut postag = None;
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"action" => action = self.attr_value(&attr).to_lowercase(),
                        b"postag" => postag = Some(self.attr_value(&attr)),
                        _ => {}
                    }
                }
                if let Some(p) = self.pending.as_mut() {
                    p.current_disambig = Some(DisambigAction {
                        action,
                        postag,
                        new_readings: Vec::new(),
                        filter_match: None,
                    });
                }
            }
            "wd" => {
                let mut wd = WdReading::default();
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"lemma" => wd.lemma = Some(self.attr_value(&attr)),
                        b"pos" => wd.postag = Some(self.attr_value(&attr)),
                        _ => {}
                    }
                }
                if let Some(p) = self.pending.as_mut() {
                    self.content_stack.push(String::new());
                    p.current_wd = Some(wd);
                }
            }
            "unification" if self.path.first().map(|p| p == "rules").unwrap_or(false) => {
                self.grammar.unification_features += 1;
                self.in_unification_def = true;
                self.unification_feature = self.attr_opt(e, b"feature");
            }
            "equivalence" if self.in_unification_def => {
                self.equivalence_type = self.attr_opt(e, b"type");
            }
            "message" if self.antipattern_depth == 0 => {
                let suppress = e
                    .attributes()
                    .flatten()
                    .find(|a| a.key.as_ref() == b"suppress_misspelled")
                    .is_some_and(|a| self.attr_value(&a) == "yes");
                if let Some(p) = self.pending.as_mut() {
                    p.message_suppress_misspelled = suppress;
                }
                self.captures.push(Capture::Message)
            }
            "short" if self.antipattern_depth == 0 => self.captures.push(Capture::Short),
            "url" if self.antipattern_depth == 0 => self.captures.push(Capture::Url),
            _ => {}
        }
        self.path.push(name);
        Ok(())
    }

    fn on_doctype(&mut self, d: &quick_xml::events::BytesText<'_>) {
        let raw = String::from_utf8_lossy(d.as_ref()).into_owned();
        // parse `<!ENTITY name "value">` declarations of the internal subset
        for cap in raw.split("<!ENTITY").skip(1) {
            let Some(open) = cap.find(['"', '\'']) else {
                continue;
            };
            let quote = open_char(cap, open);
            let Some(end_rel) = cap[open + 1..].find(quote) else {
                continue;
            };
            let value = &cap[open + 1..open + 1 + end_rel];
            let name = cap[..open].trim().to_string();
            if name.is_empty() || name.contains('<') || name.contains('!') {
                continue;
            }
            self.entities.insert(name, value.to_string());
        }
        // expand entity references inside entity values (bounded rounds)
        for _ in 0..5 {
            let mut changed = false;
            let snapshot = self.entities.clone();
            for value in self.entities.values_mut() {
                if let Some(expanded) = expand_entities(value, &snapshot) {
                    *value = expanded;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Text content of one event: expand custom DTD entities, then unescape
    /// the standard XML escapes.
    fn prepare_text(&self, raw: &str) -> String {
        let mut s = raw.to_string();
        for _ in 0..5 {
            match expand_entities(&s, &self.entities) {
                Some(next) => s = next,
                None => break,
            }
        }
        match quick_xml::escape::unescape(&s) {
            Ok(u) => u.into_owned(),
            Err(_) => s,
        }
    }

    /// `<match no="..." postag="..." postag_regexp="yes"/>` inside a
    /// `<disambig>` element (Java `DisambiguationRuleHandler` `posSelector`):
    /// the action keeps only the readings selected by the match.
    fn on_disambig_match_empty(&mut self, e: &quick_xml::events::BytesStart<'_>) {
        let mut filter = DisambigMatchFilter::default();
        for attr in e.attributes().flatten() {
            match attr.key.as_ref() {
                b"postag" => filter.postag = Some(self.attr_value(&attr)),
                b"postag_regexp" => filter.postag_regexp = self.attr_value(&attr) == "yes",
                b"postag_replace" => filter.postag_replace = Some(self.attr_value(&attr)),
                b"regexp_match" => filter.regexp_match = Some(self.attr_value(&attr)),
                b"regexp_replace" => filter.regexp_replace = Some(self.attr_value(&attr)),
                _ => {}
            }
        }
        if let Some(p) = self.pending.as_mut() {
            if let Some(d) = p.current_disambig.as_mut() {
                d.filter_match = Some(filter);
            }
        }
    }

    /// `<match no="...">` inside `<suggestion>` or inside a pattern `<token>`
    /// (start and self-closing).
    fn on_match_element(&mut self, e: &quick_xml::events::BytesStart<'_>, self_closing: bool) {
        let mut spec = MatchRefSpec {
            include_skipped: "none".to_string(),
            ..MatchRefSpec::default()
        };
        for attr in e.attributes().flatten() {
            let key = attr.key.as_ref();
            let value = self.attr_value(&attr);
            match key {
                b"no" => spec.no = value.parse::<usize>().unwrap_or(0),
                b"postag" => spec.postag = Some(value),
                b"postag_replace" => spec.postag_replace = Some(value),
                b"postag_regexp" => spec.postag_regexp = value == "yes",
                b"case_conversion" => spec.case_conversion = value.to_lowercase().replace('_', ""),
                b"regexp_match" => spec.regexp_match = Some(value),
                b"regexp_replace" => spec.regexp_replace = Some(value),
                b"setpos" => spec.setpos = value == "yes",
                b"include_skipped" => spec.include_skipped = value.to_lowercase(),
                b"suppress_misspelled" => spec.suppress_misspelled = value == "yes",
                _ => {}
            }
        }
        // Java `checkRefNumber`: `no` is required for suggestion/message
        // references; without it the element is ignored
        if !e.attributes().flatten().any(|a| a.key.as_ref() == b"no") {
            return;
        }
        if !self_closing {
            self.in_match = true;
            self.match_buf.clear();
        }
        let Some(p) = self.pending.as_mut() else {
            return;
        };
        if let Some(parts) = p.current_suggestion.as_mut() {
            let lit = std::mem::take(&mut p.suggestion_text_buf);
            if !lit.is_empty() {
                parts.push(SuggestionPart::Literal(lit));
            }
            parts.push(SuggestionPart::MatchRef(spec.clone()));
            // the message keeps a `\N` placeholder like Java's
            // `\u0001\N` (both the suggestion value and the message render it)
            if self.captures.contains(&Capture::Message) {
                let no = spec.no;
                // the message may *start* with `<match/>`; the placeholder
                // still has to be created
                let message = p.message.get_or_insert_with(String::new);
                p.message_match_refs.push(MessageMatchRef {
                    offset: message.len(),
                    spec,
                });
                message.push('\\');
                message.push_str(&no.to_string());
            }
            return;
        }
        // message-level reference: Java `suggestionMatches` writes the
        // placeholder `\u0001\N`; our message pipeline expands `\N`
        if self.captures.contains(&Capture::Message) {
            let no = spec.no;
            // the message may *start* with `<match/>`; the placeholder
            // still has to be created
            let message = p.message.get_or_insert_with(String::new);
            p.message_match_refs.push(MessageMatchRef {
                offset: message.len(),
                spec,
            });
            message.push('\\');
            message.push_str(&no.to_string());
            return;
        }
        // token-level reference: the token text gets the `\N` placeholder
        // (XMLRuleHandler appends `\` + refNumber to the token buffer)
        if let Some(token) = p.current_token.as_mut() {
            token.match_ref = Some(MatchSpec {
                no: spec.no,
                postag: spec.postag.clone(),
                case_conversion: if spec.case_conversion.is_empty() {
                    None
                } else {
                    Some(spec.case_conversion.clone())
                },
                setpos: spec.setpos,
            });
            if let Some(buf) = self.content_stack.last_mut() {
                buf.push('\\');
                buf.push_str(&spec.no.to_string());
            }
        }
    }

    /// `XMLRuleHandler.endElement` for `<match>`: the buffered text is the
    /// static lemma of the last match reference (`setLemmaString`).
    fn finish_match_element(&mut self) {
        if !self.in_match {
            return;
        }
        self.in_match = false;
        let lemma = std::mem::take(&mut self.match_buf);
        if lemma.is_empty() {
            return;
        }
        let Some(p) = self.pending.as_mut() else {
            return;
        };
        if let Some(parts) = p.current_suggestion.as_mut() {
            if let Some(SuggestionPart::MatchRef(spec)) = parts.last_mut() {
                spec.static_lemma = Some(lemma.clone());
                spec.postag_regexp = true;
            }
            if self.captures.contains(&Capture::Message) {
                if let Some(mref) = p.message_match_refs.last_mut() {
                    mref.spec.static_lemma = Some(lemma);
                    mref.spec.postag_regexp = true;
                }
            }
        } else if let Some(mref) = p.message_match_refs.last_mut() {
            mref.spec.static_lemma = Some(lemma);
            mref.spec.postag_regexp = true;
        } else {
            // token-level `<match no="N">lemma</match>` (`setLemmaString` on
            // `tokenReference`) is only used by rule data outside the gate
            // sample; the token matcher ignores the static lemma for now.
            let _ = lemma;
        }
    }

    fn on_text(&mut self, text: &str) -> Result<()> {
        // Java checks `inMatch` first: text inside `<match>` is the static
        // lemma and never reaches the message/suggestion buffers.
        if self.in_match {
            self.match_buf.push_str(text);
            return Ok(());
        }
        // entity expansion + standard unescape happen in `prepare_text`
        let top = self.path.last().map(|s| s.as_str()).unwrap_or("");
        if self.antipattern_depth > 0 {
            // token texts inside `<antipattern>` matter (the patterns are
            // matched); examples inside antipatterns are ignored
            if matches!(top, "token" | "wd" | "exception") {
                if let Some(buf) = self.content_stack.last_mut() {
                    buf.push_str(text);
                }
            }
            return Ok(());
        }
        if let Some((buf, ..)) = self.example.as_mut() {
            buf.push_str(text);
        }
        match top {
            "token" | "wd" | "exception" => {
                if let Some(buf) = self.content_stack.last_mut() {
                    buf.push_str(text);
                }
            }
            "regexp" => {
                if let Some(p) = self.pending.as_mut() {
                    p.regex_buf.push_str(text);
                }
            }
            "suggestion" => {
                if let Some(p) = self.pending.as_mut() {
                    p.suggestion_text_buf.push_str(text);
                }
            }
            _ => {}
        }
        if self.captures.is_empty() {
            return Ok(());
        }
        let Some(p) = self.pending.as_mut() else {
            return Ok(());
        };
        for capture in self.captures.clone() {
            match capture {
                Capture::Message => p.message.get_or_insert_with(String::new).push_str(text),
                Capture::Short => p.short.get_or_insert_with(String::new).push_str(text),
                Capture::Url => p.url.get_or_insert_with(String::new).push_str(text),
            }
        }
        Ok(())
    }

    fn on_end(&mut self, name: &[u8]) -> Result<()> {
        let name = String::from_utf8_lossy(name).into_owned();
        self.path.pop();
        match name.as_str() {
            "match" => self.finish_match_element(),
            "example" => {
                if self.antipattern_depth > 0 {
                    return Ok(());
                }
                if let Some((text, correct, triggers, corrections, inputform, outputform)) =
                    self.example.take()
                {
                    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if let Some(p) = self.pending.as_mut() {
                        p.examples.push(Example {
                            text,
                            correct,
                            triggers_error: triggers,
                            corrections,
                            inputform,
                            outputform,
                        });
                    }
                }
            }
            "feature" => {
                if let Some(p) = self.pending.as_mut() {
                    if p.in_unification {
                        if let Some(feature) = p.current_feature.take() {
                            p.equivalence_features
                                .insert(feature, std::mem::take(&mut p.current_types));
                        }
                    }
                }
            }
            "unify-ignore" => {
                if let Some(p) = self.pending.as_mut() {
                    p.in_unification_neutral = false;
                }
            }
            "unify" => {
                if let Some(p) = self.pending.as_mut() {
                    if p.in_unification {
                        let last = if self.antipattern_depth > 0 {
                            p.current_antipattern
                                .as_mut()
                                .and_then(|a| a.tokens.last_mut())
                        } else {
                            p.pattern.tokens.last_mut()
                        };
                        if let Some(token) = last {
                            token.last_in_unification = true;
                            if p.uni_negation {
                                token.uni_negated = true;
                            }
                        }
                        p.in_unification = false;
                        p.in_unification_neutral = false;
                        p.uni_negation = false;
                        p.equivalence_features.clear();
                        p.current_feature = None;
                        p.current_types.clear();
                    }
                }
            }
            "unification" if self.in_unification_def => {
                self.in_unification_def = false;
                self.unification_feature = None;
                self.equivalence_type = None;
                self.def_current_token = None;
            }
            "equivalence" if self.in_unification_def => {
                if let (Some(feature), Some(type_name), Some(token)) = (
                    self.unification_feature.clone(),
                    self.equivalence_type.clone(),
                    self.def_current_token.take(),
                ) {
                    self.grammar.equivalence_defs.push(EquivalenceDef {
                        feature,
                        type_name,
                        token,
                    });
                }
            }
            "message" => {
                let was_message = self.captures.pop() == Some(Capture::Message);
                if was_message {
                    if let Some(p) = self.pending.as_mut() {
                        finalize_message_matches(p);
                    }
                }
            }

            "short" | "url" => {
                self.captures.pop();
            }
            "pattern" => {
                if let Some(p) = self.pending.as_mut() {
                    p.in_pattern = false;
                }
            }
            "marker" => {
                if let Some(p) = self.pending.as_mut() {
                    if p.marker_depth > 0 {
                        p.marker_depth -= 1;
                        if p.marker_depth == 0 {
                            let list_len = if self.antipattern_depth > 0 {
                                p.current_antipattern
                                    .as_ref()
                                    .map(|a| a.tokens.len())
                                    .unwrap_or(0)
                            } else {
                                p.pattern.tokens.len()
                            };
                            let idx = p.groups.last().map(|g| g.base + 1).unwrap_or(list_len);
                            if self.antipattern_depth > 0 {
                                if let Some(anti) = p.current_antipattern.as_mut() {
                                    anti.marker_end = Some(idx);
                                }
                            } else {
                                p.pattern.marker_end = Some(idx);
                            }
                        }
                    }
                }
            }
            "and" | "or" => {
                if self.phrase_id.is_some() {
                    self.phrase_groups.pop();
                } else if let Some(p) = self.pending.as_mut() {
                    p.groups.pop();
                }
            }
            "token" => {
                if self.in_unification_def {
                    if let Some(content) = self.content_stack.pop() {
                        if let Some(mut token) = self.def_current_token.take() {
                            let content = content.trim().to_string();
                            if token.text.is_none() && !content.is_empty() {
                                token.text = Some(normalize_text_pattern(&content));
                            }
                            if !token.case_sensitive_set
                                && posix_case_sensitive(token.text.as_deref())
                            {
                                token.case_sensitive = true;
                            }
                            self.def_current_token = Some(token);
                        }
                    }
                    return Ok(());
                }
                if self.phrase_current_token.is_some() {
                    self.commit_phrase_token();
                    return Ok(());
                }
                if let Some(p) = self.pending.as_mut() {
                    if let Some(content) = self.content_stack.pop() {
                        let content = content.trim().to_string();
                        if let Some(mut token) = p.current_token.take() {
                            if token.text.is_none() && !content.is_empty() {
                                token.text = Some(normalize_text_pattern(&content));
                            }
                            if !token.case_sensitive_set
                                && posix_case_sensitive(token.text.as_deref())
                            {
                                token.case_sensitive = true;
                            }
                            if p.in_unification {
                                token.unification = Some(p.equivalence_features.clone());
                            }
                            if p.in_unification_neutral {
                                token.unification_neutral = true;
                            }
                            // Java `XMLRuleHandler`: `min` > 1 duplicates the
                            // element (`setMinOccurrences` only accepts 0/1)
                            let copies = token.min.unwrap_or(1).max(1) as usize;
                            for _ in 0..copies {
                                let token = token.clone();
                                if self.antipattern_depth > 0 {
                                    if let Some(anti) = p.current_antipattern.as_mut() {
                                        push_or_attach(&mut anti.tokens, token, &mut p.groups);
                                    }
                                } else {
                                    push_or_attach(&mut p.pattern.tokens, token, &mut p.groups);
                                }
                            }
                        }
                    }
                }
            }
            "wd" => {
                if let Some(p) = self.pending.as_mut() {
                    if let Some(content) = self.content_stack.pop() {
                        if let Some(mut wd) = p.current_wd.take() {
                            wd.token = content.trim().to_string();
                            if let Some(d) = p.current_disambig.as_mut() {
                                d.new_readings.push(wd);
                            }
                        }
                    }
                }
            }
            "disambig" => {
                if let Some(p) = self.pending.as_mut() {
                    if let Some(d) = p.current_disambig.take() {
                        p.disambigs.push(d);
                    }
                }
            }
            "antipattern" => {
                self.antipattern_depth = self.antipattern_depth.saturating_sub(1);
                let mut group_level = None;
                if let Some(p) = self.pending.as_mut() {
                    if let Some(anti) = p.current_antipattern.take() {
                        if !anti.tokens.is_empty() {
                            p.antipatterns.push(anti.clone());
                            if p.kind == PendingKind::Rulegroup {
                                group_level = Some(anti);
                            }
                        }
                    }
                    p.pattern_case_sensitive = false;
                }
                if let Some(anti) = group_level {
                    self.group_antipatterns.push(anti);
                }
            }
            "exception" => {
                if let Some(mut exc) = self.phrase_current_exception.take() {
                    if let Some(content) = self.content_stack.pop() {
                        let content = content.trim().to_string();
                        if exc.text.is_none() && !content.is_empty() {
                            exc.text = Some(normalize_text_pattern(&content));
                        }
                        if posix_case_sensitive(exc.text.as_deref()) {
                            exc.case_sensitive = true;
                        }
                        if let Some(token) = self.phrase_current_token.as_mut() {
                            if let Some(last) = token.exceptions.last_mut() {
                                *last = exc;
                            }
                        }
                    }
                    return Ok(());
                }
                if let Some(p) = self.pending.as_mut() {
                    if let Some(content) = self.content_stack.pop() {
                        if let Some(mut exc) = p.current_exception.take() {
                            let content = content.trim().to_string();
                            if exc.text.is_none() && !content.is_empty() {
                                exc.text = Some(normalize_text_pattern(&content));
                            }
                            if posix_case_sensitive(exc.text.as_deref()) {
                                exc.case_sensitive = true;
                            }
                            if let Some(token) = p.current_token.as_mut() {
                                if let Some(last) = token.exceptions.last_mut() {
                                    *last = exc;
                                }
                            }
                        }
                    }
                }
            }
            "suggestion" => {
                let in_message = self.captures.contains(&Capture::Message);
                if let Some(p) = self.pending.as_mut() {
                    if let Some(mut parts) = p.current_suggestion.take() {
                        let lit = std::mem::take(&mut p.suggestion_text_buf);
                        if !lit.is_empty() {
                            parts.push(SuggestionPart::Literal(lit));
                        }
                        p.suggestions.push(parts);
                        p.suggestion_suppress
                            .push(std::mem::take(&mut p.suggestion_suppress_current));
                    }
                    if in_message {
                        p.message
                            .get_or_insert_with(String::new)
                            .push_str("</suggestion>");
                    }
                }
            }
            "rule" => {
                if let Some(p) = self.pending.take() {
                    if p.kind != PendingKind::Rulegroup {
                        // `<rule>` and `<regexp>` rules both emit here
                        self.emit_rule(p);
                    }
                }
            }
            "rulegroup" => {
                // reset rulegroup-level defaults so following standalone
                // rules do not inherit stale tags/issue type/name
                self.group_min_prev_matches = 0;
                self.group_distance_tokens = 0;
                self.group_tags.clear();
                self.group_tone_tags.clear();
                self.group_goal_specific = None;
                self.group_issue_type = None;
                self.group_name = None;
                if let Some(p) = self.pending.take() {
                    if p.has_content() {
                        self.emit(p);
                    }
                }
                self.group = None;
            }
            "regexp" => {
                // finalize the pattern; message/suggestion/example captures
                // continue on the same pending until `</rule>`
                if let Some(p) = self.pending.as_mut() {
                    if let Some(spec) = p.regexp.as_mut() {
                        spec.pattern = p.regex_buf.trim().to_string();
                    }
                    p.regex_buf.clear();
                }
            }
            "phrase" if self.in_phrases => {
                self.finalize_phrase();
            }
            "includephrases" => {
                // Java `endElement`: the prefix tokens collected in a phrase
                // definition are discarded; the phraseref expansions remain
                self.phrase_tokens.clear();
            }
            "phrases" => {
                self.in_phrases = false;
            }
            "category" => {
                self.finish_pending();
                self.category_id = None;
                self.category_name = None;
                self.category_default_on = true;
                self.category_issue_type = None;
                self.category_tags.clear();
                self.category_tone_tags.clear();
                self.category_goal_specific = None;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_token_empty(&mut self, e: &quick_xml::events::BytesStart<'_>) {
        let default_case = self
            .pending
            .as_ref()
            .map(|p| p.pattern_case_sensitive)
            .unwrap_or(false);
        self.drop_tokens_after_phrase();
        let (mut token, has_chunk) = self.parse_token_attrs(e, default_case);
        if self.in_unification_def {
            self.def_current_token = Some(token);
            return;
        }
        if self.phrase_id.is_some() {
            token.in_marker = false;
            let copies = token.min.unwrap_or(1).max(1) as usize;
            for _ in 0..copies {
                push_or_attach(
                    &mut self.phrase_tokens,
                    token.clone(),
                    &mut self.phrase_groups,
                );
            }
            return;
        }
        let Some(p) = self.pending.as_mut() else {
            return;
        };
        if p.in_unification {
            token.unification = Some(p.equivalence_features.clone());
        }
        if p.in_unification_neutral {
            token.unification_neutral = true;
        }
        // Java `XMLRuleHandler.setToken`: `min` >= 1 duplicates the element
        // (`min="0"` keeps one optional copy). Self-closing `<token/>`
        // elements take the same path as tokens with content.
        let copies = token.min.unwrap_or(1).max(1) as usize;
        if self.antipattern_depth > 0 {
            token.in_marker = p.marker_depth > 0;
            if let Some(anti) = p.current_antipattern.as_mut() {
                for _ in 0..copies {
                    push_or_attach(&mut anti.tokens, token.clone(), &mut p.groups);
                }
            }
        } else if p.in_pattern {
            token.in_marker = p.marker_depth > 0;
            p.has_chunk_attrs |= has_chunk;
            for _ in 0..copies {
                push_or_attach(&mut p.pattern.tokens, token.clone(), &mut p.groups);
            }
        } else {
            p.token_count += 1;
        }
    }

    fn finish_pending(&mut self) {
        if let Some(p) = self.pending.take() {
            if p.has_content() {
                self.emit(p);
            }
        }
    }

    /// Tokens collected so far in the current context (the rule pattern, an
    /// antipattern, or a `<phrase>` definition) — Java's global
    /// `patternTokens` when `preparePhrase` runs.
    fn current_pattern_tokens(&self) -> Vec<PatternToken> {
        if let Some(p) = self.pending.as_ref() {
            if self.antipattern_depth > 0 {
                return p
                    .current_antipattern
                    .as_ref()
                    .map(|a| a.tokens.clone())
                    .unwrap_or_default();
            }
            if p.in_pattern {
                return p.pattern.tokens.clone();
            }
        }
        self.phrase_tokens.clone()
    }

    /// Java `setToken`'s `lastPhrase` handling: the token after a
    /// `<phraseref>` starts a new token list (the old one is already part of
    /// the expansion prefix).
    fn drop_tokens_after_phrase(&mut self) {
        if !self.last_phrase {
            return;
        }
        self.last_phrase = false;
        if self.phrase_id.is_some() {
            self.phrase_tokens.clear();
            return;
        }
        if let Some(p) = self.pending.as_mut() {
            if self.antipattern_depth > 0 {
                if let Some(anti) = p.current_antipattern.as_mut() {
                    anti.tokens.clear();
                }
            } else if p.in_pattern {
                p.pattern.tokens.clear();
            }
        }
    }

    /// Java `XMLRuleHandler.preparePhrase`: expand `<phraseref>` into the
    /// referenced phrase's alternatives, prefixing the tokens collected so
    /// far. Tokens after the reference are appended when the rule ends.
    fn prepare_phrase(&mut self, idref: &str) {
        let Some(alts) = self.phrase_map.get(idref).cloned() else {
            return;
        };
        let in_marker = self
            .pending
            .as_ref()
            .map(|p| p.marker_depth > 0 && (p.in_pattern || self.antipattern_depth > 0))
            .unwrap_or(false);
        let prefix = self.current_pattern_tokens();
        for alt in alts {
            let mut copy = alt;
            for token in copy.iter_mut() {
                token.in_marker = in_marker;
            }
            if prefix.is_empty() {
                self.phrase_pattern_tokens.push(copy);
            } else {
                let mut prev = prefix.clone();
                prev.extend(copy);
                self.phrase_pattern_tokens.push(prev);
            }
        }
        self.last_phrase = true;
    }

    /// Java `XMLRuleHandler.finalizePhrase`: store the collected tokens as a
    /// (possibly cross-producted) phrase definition.
    fn finalize_phrase(&mut self) {
        for token in self.phrase_tokens.iter_mut() {
            token.in_marker = false;
        }
        if self.phrase_pattern_tokens.is_empty() {
            self.phrase_pattern_tokens.push(self.phrase_tokens.clone());
        } else {
            let tokens = self.phrase_tokens.clone();
            for ph in self.phrase_pattern_tokens.iter_mut() {
                ph.extend(tokens.iter().cloned());
            }
        }
        if let Some(id) = self.phrase_id.take() {
            self.phrase_map
                .insert(id, std::mem::take(&mut self.phrase_pattern_tokens));
        }
        self.phrase_tokens.clear();
        self.phrase_groups.clear();
    }

    /// Commit the token collected in a `<phrase>` definition (mirrors the
    /// `<token>` branch of `on_end`).
    fn commit_phrase_token(&mut self) {
        let Some(content) = self.content_stack.pop() else {
            return;
        };
        let Some(mut token) = self.phrase_current_token.take() else {
            return;
        };
        let content = content.trim().to_string();
        if token.text.is_none() && !content.is_empty() {
            token.text = Some(normalize_text_pattern(&content));
        }
        if !token.case_sensitive_set && posix_case_sensitive(token.text.as_deref()) {
            token.case_sensitive = true;
        }
        let copies = token.min.unwrap_or(1).max(1) as usize;
        for _ in 0..copies {
            push_or_attach(
                &mut self.phrase_tokens,
                token.clone(),
                &mut self.phrase_groups,
            );
        }
    }

    /// Emit a rule pending that holds a `<phraseref>`: Java `createRules`
    /// runs once per phrase alternative, with the trailing pattern tokens
    /// appended (a rulegroup keeps the same id/sub id for all of them).
    fn emit_rule(&mut self, p: Pending) {
        if self.phrase_pattern_tokens.is_empty() {
            self.emit(p);
            return;
        }
        let trailing = p.pattern.tokens.clone();
        for alt in std::mem::take(&mut self.phrase_pattern_tokens) {
            let mut tokens = alt;
            tokens.extend(trailing.iter().cloned());
            let marker_start = tokens.iter().position(|t| t.in_marker);
            let marker_end = tokens.iter().rposition(|t| t.in_marker).map(|i| i + 1);
            let mut p2 = p.clone();
            p2.pattern.tokens = tokens;
            p2.pattern.marker_start = marker_start;
            p2.pattern.marker_end = marker_end;
            self.emit(p2);
        }
    }

    fn emit(&mut self, p: Pending) {
        let pattern = p.pattern;
        let complex_pattern = pattern.complex;
        let rule = RuleDef {
            id: p.id,
            sub_id: p.sub_id,
            category_id: self.category_id.clone(),
            category_name: self.category_name.clone(),
            name: p.name,
            issue_type: p.issue_type.unwrap_or_else(|| "Other".to_string()),
            message: p.message,
            short: p.short,
            url: p.url,
            default_on: p.default_on,
            category_default_on: self.category_default_on,
            prio: p.prio,
            examples: p.examples,
            token_count: p.token_count,
            has_filter: p.filter.is_some(),
            filter: p.filter,
            has_chunk_attrs: p.has_chunk_attrs,
            tags: p.tags,
            tone_tags: p.tone_tags,
            goal_specific: p.goal_specific,
            min_prev_matches: p.min_prev_matches,
            distance_tokens: p.distance_tokens,
            complex_pattern,
            pattern,
            suggestions: p.suggestions,
            suggestion_suppress: p.suggestion_suppress,
            message_suppress_misspelled: p.message_suppress_misspelled,
            message_match_refs: p.message_match_refs,
            disambigs: p.disambigs,
            antipatterns: p.antipatterns,
            is_regexp_rule: p.kind == PendingKind::Regexp,
            regexp: if p.kind == PendingKind::Regexp {
                p.regexp
            } else {
                None
            },
            source_file: self.source.clone(),
        };
        self.grammar.rules.push(rule);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn data_dir() -> Option<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        dir.lt_is_dir().then_some(dir)
    }

    #[test]
    fn loads_english_grammar() {
        let Some(dir) = data_dir() else {
            eprintln!("skipping: vendored data not found");
            return;
        };
        let grammar = Grammar::load_file(dir.join("en/rules/grammar.xml")).unwrap();
        assert!(
            grammar.rules.len() > 5000,
            "en grammar rules: {}",
            grammar.rules.len()
        );
        assert!(
            grammar.example_count() > 20000,
            "en grammar examples: {}",
            grammar.example_count()
        );
        assert!(grammar.categories.len() > 10);
        assert!(grammar.rules.iter().any(|r| r.has_filter));
    }

    #[test]
    fn examples_are_extracted_with_marker_stripped() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rules lang="en">
  <category name="Test" id="TEST">
    <rule id="TEST_RULE" name="test">
      <pattern><token>a</token></pattern>
      <message>test message</message>
      <example type="incorrect">This is <marker>a</marker> bad day.</example>
      <example correction="an">This is an ok day.</example>
    </rule>
  </category>
</rules>"#;
        let dir = std::env::temp_dir().join("lt-pattern-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.xml");
        std::fs::write(&path, xml).unwrap();
        let grammar = Grammar::load_file(&path).unwrap();
        assert_eq!(grammar.rules.len(), 1, "rules: {grammar:?}");
        let rule = &grammar.rules[0];
        assert_eq!(rule.id, "TEST_RULE");
        assert_eq!(rule.category_id.as_deref(), Some("TEST"));
        assert_eq!(rule.examples.len(), 2);
        // type defaults to "incorrect" per the LT schema
        assert!(!rule.examples[0].correct);
        assert_eq!(rule.examples[0].text, "This is a bad day.");
        assert!(!rule.examples[1].correct);
        assert_eq!(rule.examples[1].corrections, vec!["an"]);
    }

    #[test]
    fn java_nested_quantifiers_are_normalized() {
        // Java `(X*){m,n}` ≡ `X*` (verified with `QuantProbe`, D-022)
        assert_eq!(
            matcher::normalize_java_quantifiers(r"[0-9,.]*{1,30}"),
            r"[0-9,.]*"
        );
        assert_eq!(matcher::normalize_java_quantifiers(r"a+{1,3}"), "a+");
        assert_eq!(matcher::normalize_java_quantifiers(r"a?{1,3}"), "a?");
        // literal braces in classes/escapes stay untouched
        assert_eq!(
            matcher::normalize_java_quantifiers(r"[*{1,3}]"),
            r"[*{1,3}]"
        );
        assert_eq!(matcher::normalize_java_quantifiers(r"\*{1,3}"), r"\*{1,3}");
        assert_eq!(
            matcher::normalize_java_quantifiers(r"[0-9]{1,30}"),
            r"[0-9]{1,30}"
        );
    }

    #[test]
    fn rulegroups_flush_one_rule_per_nested_rule() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rules lang="en">
  <category name="Test" id="TEST">
    <rulegroup id="TEST_GROUP" name="group" default="off">
      <rule id="SUB_A" name="a">
        <pattern><token>x</token></pattern>
        <message>msg a</message>
        <example type="incorrect"><marker>x</marker> y</example>
      </rule>
      <rule id="SUB_B" name="b">
        <pattern><token>y</token></pattern>
        <message>msg b</message>
        <example type="incorrect"><marker>y</marker> z</example>
      </rule>
    </rulegroup>
  </category>
</rules>"#;
        let dir = std::env::temp_dir().join("lt-pattern-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_group.xml");
        std::fs::write(&path, xml).unwrap();
        let grammar = Grammar::load_file(&path).unwrap();
        assert_eq!(grammar.rules.len(), 2, "rules: {grammar:?}");
        // Java: nested `<rule id>` becomes the rule id, sub id is the
        // 1-based counter within the group
        assert_eq!(grammar.rules[0].id, "SUB_A");
        assert_eq!(grammar.rules[0].sub_id.as_deref(), Some("1"));
        assert!(!grammar.rules[0].default_on);
        assert_eq!(grammar.rules[1].id, "SUB_B");
        assert_eq!(grammar.rules[1].sub_id.as_deref(), Some("2"));
    }

    #[test]
    fn raw_pos_attribute_is_parsed() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rules lang="en">
  <category name="Test" id="TEST">
    <rule id="RAW" name="raw">
      <pattern raw_pos="yes"><token>x</token><token>y</token></pattern>
      <message>msg</message>
    </rule>
    <rule id="PLAIN" name="plain">
      <pattern><token>x</token></pattern>
      <message>msg</message>
    </rule>
    <rule id="EXPLICIT_NO" name="no">
      <pattern raw_pos="no"><token>x</token></pattern>
      <message>msg</message>
    </rule>
  </category>
</rules>"#;
        let dir = std::env::temp_dir().join("lt-pattern-raw-pos-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("raw_pos.xml");
        std::fs::write(&path, xml).unwrap();
        let grammar = Grammar::load_file(&path).unwrap();
        assert!(grammar.rules[0].pattern.raw_pos);
        assert!(!grammar.rules[1].pattern.raw_pos);
        assert!(!grammar.rules[2].pattern.raw_pos);
    }

    /// The 11 French `raw_pos="yes"` patterns (D-083/D-084): the loader must
    /// keep the flag on exactly these rulegroups.
    #[test]
    fn french_raw_pos_patterns_are_parsed() {
        let Some(dir) = data_dir() else {
            eprintln!("skipping: vendored data not found");
            return;
        };
        let grammar = Grammar::load_file(dir.join("fr/rules/grammar.xml")).unwrap();
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for rule in grammar.rules.iter().filter(|r| r.pattern.raw_pos) {
            *counts.entry(rule.id.as_str()).or_default() += 1;
        }
        assert_eq!(
            counts,
            std::collections::BTreeMap::from([
                ("A_ACCENT", 1),
                ("A_INFINITIF", 1),
                ("CE_SE", 4),
                ("DU_DU", 3),
                ("PRONSUJ_NONVERBE", 1),
                ("SE_CE", 1),
            ])
        );
    }
}

#[cfg(test)]
mod rep_probe {
    use super::*;
    #[test]
    fn rep_passive_min_prev() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !dir.lt_is_dir() {
            return;
        }
        let g = Grammar::load_file(dir.join("en/rules/style.xml")).unwrap();
        let rs: Vec<_> = g
            .rules
            .iter()
            .filter(|r| r.id == "REP_PASSIVE_VOICE" || r.id == "REP_THANK_YOU_FOR")
            .map(|r| (r.id.clone(), r.min_prev_matches, r.distance_tokens))
            .collect();
        println!("{rs:?}");
        assert!(rs.iter().any(|(_, m, d)| *m == 4 && *d == 80));
    }
}

#[cfg(test)]
mod entity_tests {
    use super::normalize_text_pattern;
    use super::preprocess_entities;

    /// `StringTools.trimWhitespace`: a whitespace run is dropped entirely, so
    /// a multi-line entity value yields clean `|`-separated alternatives.
    #[test]
    fn trim_whitespace_drops_runs_like_java() {
        assert_eq!(
            normalize_text_pattern("esperar|imaginar|\n                            propor|querer"),
            "esperar|imaginar|propor|querer"
        );
        assert_eq!(normalize_text_pattern("a  b"), "ab");
        assert_eq!(normalize_text_pattern("a b"), "a b");
        assert_eq!(normalize_text_pattern("a\nb"), "ab");
        assert_eq!(normalize_text_pattern("  \t foo  "), "foo");
        assert_eq!(normalize_text_pattern(""), "");
    }

    /// Java's SAX parser ignores commented-out DTD declarations; the active
    /// `abbreviated_negated_verbs` definition in en/disambiguation.xml must
    /// not be shadowed by the commented one above it.
    #[test]
    fn commented_entity_does_not_shadow_active_declaration() {
        let doc = r#"<?xml version="1.0"?>
<!DOCTYPE rules [
  <!--<!ENTITY foo "old|stuff">-->
  <!ENTITY foo "wo|would">
]>
<rules><rule><pattern><token regexp="yes">&foo;</token></pattern></rule></rules>"#;
        let out = preprocess_entities(doc, None);
        let body = out.split("]>").nth(1).unwrap();
        assert!(body.contains("wo|would"), "{body}");
        assert!(!body.contains("old|stuff"), "{body}");
    }

    /// The Spanish rules files reference `entities.ent` as an external
    /// parameter entity (`%entities;`); the declarations must be merged into
    /// the document like Java's SAX parser does.
    #[test]
    fn external_parameter_entity_is_resolved() {
        let dir = std::env::temp_dir().join(format!("lt-entity-test-{}", std::process::id()));
        let rules = dir.join("rules");
        let words = dir.join("words");
        std::fs::create_dir_all(&rules).unwrap();
        std::fs::create_dir_all(&words).unwrap();
        std::fs::write(
            words.join("entities.ent"),
            "<!ENTITY unidades_tiempo \"segundo|minuto|año\">\n",
        )
        .unwrap();
        let doc = r#"<?xml version="1.0"?>
<!DOCTYPE rules [
    <!ENTITY % entities  SYSTEM "../../resource/es/entities.ent" >
    %entities;
]>
<rules><rule><pattern><token regexp="yes">&unidades_tiempo;|mes</token></pattern></rule></rules>"#;
        let out = preprocess_entities(doc, Some(&rules));
        let body = out.split("]>").nth(1).unwrap();
        assert!(body.contains("segundo|minuto|año"), "{body}");
        assert!(!body.contains("&unidades_tiempo;"), "{body}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
