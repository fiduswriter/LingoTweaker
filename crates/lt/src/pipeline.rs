//! Engine pipeline: sentence split → tokenize → tag → match rules → filters.
//!
//! Mirrors LanguageTool's `JLanguageTool.analyzeSentences` token-stream
//! construction: a synthetic `SENT_START` entry, whitespace tokens as
//! separate entries with `whitespace_before` flags, and a `SENT_END` reading
//! added to the last non-whitespace token. Disambiguation (P1.3) is applied;
//! chunking (D-002) is not yet — chunk-dependent rules are excluded via the
//! allowlist mechanism (P1.7).
use lt_data::PathExt as _;

use std::collections::HashSet;
use std::sync::Arc;

use lt_core::{
    AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, CheckResult, CoreError, Lang, Match,
    Result, Sentence, Suggestion, TextRange,
};
use lt_pattern::matcher as pm;
use lt_pattern::{FilterContext, FilterOutcome, Grammar, SuggestionPart};

use crate::dates::Ymd;
use crate::de::pipeline::load_chunker;
use crate::multitoken::{IsMisspelled, MultitokenSpeller};
use crate::wordutil::split_compound;

pub struct CompiledRule {
    pub rule_id: String,
    pub sub_id: Option<String>,
    pub category_id: String,
    pub category_name: String,
    pub message: String,
    pub short_message: Option<String>,
    /// concrete patterns: one per `<or>` expansion (LT creates one rule per
    /// OR alternative; all share this rule id)
    pub compiled: Vec<pm::CompiledPattern>,
    pub suggestions: Vec<Vec<SuggestionPart>>,
    /// `<match no="N">` elements of the message (case/regexp transforms)
    pub message_match_refs: Vec<lt_pattern::MessageMatchRef>,
    /// post-matching filter (P1.6); `args` resolved per match
    pub filter: Option<(Arc<dyn lt_pattern::RuleFilter>, String)>,
    /// `<antipattern>` sub-patterns (matches overlapping these are dropped)
    pub antipatterns: Vec<Arc<pm::CompiledPattern>>,
    /// `<regexp>` rules match the sentence text directly
    /// (LT `RegexPatternRule`)
    pub regex: Option<(fancy_regex::Regex, usize)>,
    /// rule description (LT `name` attribute)
    pub description: String,
    /// LT `issueType` attribute
    pub issue_type: String,
    /// LT `estimateContextForSureMatch`
    pub context_for_sure_match: i32,
    /// text-level repetition rule (`RepeatedPatternRuleTransformer`)
    pub min_prev_matches: i32,
    pub distance_tokens: i32,
    /// enclosing category is default-off (cannot be enabled per rule)
    pub category_default_on: bool,
    /// rule tags (`tags="picky"` etc.)
    pub tags: Vec<String>,
    /// rule `tone_tags` (`Rule.getToneTags`; with default tone tags a
    /// goal-specific rule that carries tone tags is inactive)
    pub tone_tags: Vec<String>,
    /// `is_goal_specific="true"` (`Rule.isGoalSpecific`)
    pub goal_specific: bool,
    /// parallel to `suggestions`: element had suppress_misspelled=yes
    pub suggestion_suppress: Vec<bool>,
    /// Java `AbstractTokenBasedRule`: minimum non-whitespace token count
    pub min_token_count: usize,
    /// Java `AbstractTokenBasedRule` token hints (conservative subset)
    pub hints: Vec<pm::CompiledHint>,
    /// `<message suppress_misspelled="yes">` (Java `isRuleSuppressMisspelled`)
    pub message_suppress_misspelled: bool,
    /// `<pattern raw_pos="yes">` (Java
    /// `PatternRule.isInterpretPosTagsPreDisambiguation`): match against the
    /// pre-disambiguation token view
    pub raw_pos: bool,
}

/// One match of a text-level repetition rule before the
/// `RepeatedPatternRuleTransformer` filter is applied.
struct RepeatingMatch {
    sentence: usize,
    rule_id: String,
    min_prev_matches: i32,
    distance_tokens: i32,
    /// absolute token index (view tokens, like Java's offsetTokens)
    from_token: usize,
    match_data: Match,
}

pub struct Pipeline {
    pub lang: Lang,
    /// `<unification>` equivalence definitions of grammar.xml/style.xml
    pub unify_config: lt_pattern::EquivalenceConfig,
    pub srx: lt_tokenize::SrxTokenizer,
    /// English tagger (`None` for German engines)
    pub tagger: Option<Arc<lt_tagger::EnglishTagger>>,
    pub grammar: Grammar,
    pub compiled_rules: Vec<Arc<CompiledRule>>,
    pub skipped_counts: SkippedCounts,
    /// (rule id, error) for rules that failed to compile or whose filter
    /// class is not in the registry
    pub compile_failures: Vec<(String, String)>,
    /// disambiguation pipeline (EnglishHybridDisambiguator order:
    /// spelling_global chunker → multiwords chunker → XML rules)
    pub global_chunker: lt_disambig::MultiWordChunker,
    pub multiword_chunker: lt_disambig::MultiWordChunker,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// D-002 option (a): OpenNLP maxent chunker over the vendored models
    /// (runs post-disambiguation, like LT's `getPostDisambiguationChunker`)
    pub english_chunker: Option<lt_chunk::EnglishChunker>,
    /// spelling rule per requested variant (en-US default, en-GB optional)
    pub spelling: Option<Arc<crate::en::spelling::SpellingRule>>,
    /// `EN_A_VS_AN` (English built-in rule, runs before pattern rules)
    pub avs_an: Option<crate::en::avs_an::AvsAnRule>,
    /// `EN_COMPOUNDS` (`AbstractCompoundRule` + `compounds.txt`)
    pub compound: Option<crate::compound::CompoundRule>,
    /// `EN_CONTRACTION_SPELLING` (`AbstractSimpleReplaceRule`)
    pub contractions: Option<crate::en::contractions::ContractionSpellingRule>,
    /// `EnglishSynthesizer` for `<match postag="...">` rendering
    pub synthesizer: Option<Arc<crate::en::synthesizer::EnglishSynthesizer>>,
    /// `ENGLISH_WRONG_WORD_IN_CONTEXT` (default-on, after contractions)
    pub wrong_word_in_context: Option<crate::wrong_word_in_context::WrongWordInContextRule>,
    /// `EN_DASH_RULE` (`tags="picky"`, after WrongWordInContext)
    pub dash: Option<crate::dash::DashRule>,
    /// `AbstractSimpleReplaceRule2` family (EN_SIMPLE_REPLACE, diacritics, …)
    pub simple_replace: Vec<crate::simple_replace::SimpleReplaceRule>,
    /// `EN_WORD_COHERENCY` (text level)
    pub word_coherency: Option<crate::word_coherency::WordCoherencyRule>,
    /// `EN_SPECIFIC_CASE` (English built-in rule)
    pub specific_case: Option<crate::en::specific_case::SpecificCaseRule>,
    /// `READABILITY_RULE_DIFFICULT` + `READABILITY_RULE_SIMPLE` (text level,
    /// both default off)
    pub readability: Vec<crate::readability::ReadabilityRule>,
    /// `EN_REPEATEDWORDS` (`tags="picky"`, text level)
    pub repeated_words: Option<crate::repeated_words::RepeatedWordsRule>,
    /// German pipeline parts (`None` for English engines)
    pub german: Option<Box<crate::de::pipeline::GermanPipeline>>,
    /// Spanish pipeline parts (`None` for English/German engines)
    pub spanish: Option<Box<crate::es::pipeline::SpanishPipeline>>,
    /// French pipeline parts (`None` for English/German/Spanish engines)
    pub french: Option<Arc<crate::fr::pipeline::FrenchPipeline>>,
    /// Italian pipeline parts (`None` for the other languages)
    pub italian: Option<Arc<crate::it::ItalianPipeline>>,
    /// Portuguese pipeline parts (`None` for the other languages)
    pub portuguese: Option<Arc<crate::pt::PortuguesePipeline>>,
    /// Dutch pipeline parts (`None` for the other languages)
    pub dutch: Option<Arc<crate::nl::DutchPipeline>>,
    /// Catalan pipeline parts (`None` for the other languages)
    pub catalan: Option<Arc<crate::ca::CatalanPipeline>>,
    /// Galician pipeline parts (`None` for the other languages)
    pub galician: Option<Arc<crate::gl::GalicianPipeline>>,
    /// Romanian pipeline parts (`None` for the other languages)
    pub romanian: Option<Arc<crate::ro::RomanianPipeline>>,
    /// Polish pipeline parts (`None` for the other languages)
    pub polish: Option<Arc<crate::pl::PolishPipeline>>,
    /// Slovak pipeline parts (`None` for the other languages)
    pub slovak: Option<Arc<crate::sk::SlovakPipeline>>,
    /// Slovenian pipeline parts (`None` for the other languages)
    pub slovenian: Option<Arc<crate::sl::SlovenianPipeline>>,
    /// Icelandic pipeline parts (`None` for the other languages)
    pub icelandic: Option<Arc<crate::is::IcelandicPipeline>>,
    /// Esperanto pipeline parts (`None` for the other languages)
    pub esperanto: Option<Arc<crate::eo::EsperantoPipeline>>,
    /// Asturian pipeline parts (`None` for the other languages)
    pub asturian: Option<Arc<crate::ast::AsturianPipeline>>,
    /// Breton pipeline parts (`None` for the other languages)
    pub breton: Option<Arc<crate::br::BretonPipeline>>,
    /// Tagalog pipeline parts (`None` for the other languages)
    pub tagalog: Option<Arc<crate::tl::TagalogPipeline>>,
    /// Lithuanian pipeline parts (`None` for the other languages)
    pub lithuanian: Option<Arc<crate::lt::LithuanianPipeline>>,
    /// Crimean Tatar pipeline parts (`None` for the other languages)
    pub crimean_tatar: Option<Arc<crate::crh::CrimeanTatarPipeline>>,
    /// Greek pipeline parts (`None` for the other languages)
    pub greek: Option<Arc<crate::el::GreekPipeline>>,
    /// Danish pipeline parts (`None` for the other languages)
    pub da: Option<Arc<crate::da::DanishPipeline>>,
    /// Swedish pipeline parts (`None` for the other languages)
    pub sv: Option<Arc<crate::sv::SwedishPipeline>>,
    /// Norwegian Bokmål pipeline parts (`None` for the other languages)
    pub norwegian: Option<Arc<crate::no::NorwegianPipeline>>,
    /// Nordum pipeline parts (`None` for the other languages)
    pub nordum: Option<Arc<crate::nrd::NordumPipeline>>,
    /// Guaraní pipeline parts (`None` for the other languages)
    pub guarani: Option<Arc<crate::gn::GuaraniPipeline>>,
    /// Belarusian pipeline parts (`None` for the other languages)
    pub belarusian: Option<Arc<crate::be::BelarusianPipeline>>,
    /// Russian pipeline parts (`None` for the other languages)
    pub russian: Option<Arc<crate::ru::RussianPipeline>>,
    /// Java `JLanguageTool.cleanOverlappingMatches` (default true)
    pub clean_overlapping_matches: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SkippedCounts {
    /// rules whose `<filter>` class is not in the registry (unmapped filter
    /// = load error, plan §7)
    pub filters: usize,
    pub complex: usize,
    pub chunk_attrs: usize,
    pub regexp_rules: usize,
    pub off_by_default: usize,
    /// rules whose pattern failed to compile (Java/fancy-regex gaps)
    pub uncompilable: usize,
}

pub fn sentence_start_tag() -> &'static str {
    "SENT_START"
}

pub fn sentence_end_tag() -> &'static str {
    "SENT_END"
}

/// Build a surface-only [`AnalyzedSentence`] for a language without a tagger:
/// tokens carry a single reading with no stem/POS, like `BaseTagger`'s
/// unknown-word readings. Used by the hand-authored languages.
pub fn surface_sentence(text: &str) -> AnalyzedSentence {
    let raw_tokens = lt_tokenize::wordtokenizer::join_emails_and_urls(
        lt_tokenize::wordtokenizer::string_tokenize(
            text,
            &lt_tokenize::wordtokenizer::base_tokenizing_characters(),
        ),
    );
    surface_sentence_from_tokens(text, raw_tokens)
}

/// Like [`surface_sentence`] but with a language-specific tokenization
/// (e.g. Guaraní keeps the puso inside the word).
pub fn surface_sentence_from_tokens(text: &str, raw_tokens: Vec<String>) -> AnalyzedSentence {
    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(raw_tokens.len() + 1);
    // synthetic sentence-start entry (LT: AnalyzedToken("", "SENT_START", null))
    tokens.push(AnalyzedTokenReadings {
        readings: vec![AnalyzedToken::new(
            "",
            None,
            Some(sentence_start_tag().to_string()),
        )],
        chunk_tags: Vec::new(),
        whitespace_before: false,
        start_pos: 0,
        raw_byte_len: 0,
        is_whitespace: false,
        is_sentence_start: true,
        is_sentence_end: false,
        is_paragraph_end: false,
        // Java `AnalyzedTokenReadings.isTagged()` counts SENT_START as tagged
        is_tagged: true,
        is_immunized: false,
        is_ignore_spelling: false,
        has_typographic_apostrophe: false,
        is_pos_tag_unknown: false,
    });

    let mut byte_pos = 0usize;
    let mut prev_was_whitespace = false;
    let mut last_non_ws_idx: Option<usize> = None;
    for raw in raw_tokens {
        let is_whitespace = lt_core::is_whitespace(&raw);
        let has_typographic_apostrophe = raw.chars().count() > 1 && raw.contains('’');
        tokens.push(AnalyzedTokenReadings {
            readings: vec![AnalyzedToken::new(raw.clone(), None, None)],
            chunk_tags: Vec::new(),
            whitespace_before: prev_was_whitespace,
            start_pos: byte_pos,
            raw_byte_len: raw.len(),
            is_whitespace,
            is_sentence_start: false,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged: false,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe,
            is_pos_tag_unknown: !is_whitespace,
        });
        if !is_whitespace {
            last_non_ws_idx = Some(tokens.len() - 1);
        }
        byte_pos += raw.len();
        prev_was_whitespace = is_whitespace;
    }

    if let Some(idx) = last_non_ws_idx {
        let tr = &mut tokens[idx];
        let surface = tr
            .readings
            .first()
            .map(|r| r.token.clone())
            .unwrap_or_default();
        tr.add_reading(AnalyzedToken::new(
            surface,
            None,
            Some(sentence_end_tag().to_string()),
        ));
        tr.is_sentence_end = true;
    } else if let Some(tr) = tokens.last_mut() {
        tr.add_reading(AnalyzedToken::new(
            tr.surface().to_string(),
            None,
            Some(sentence_end_tag().to_string()),
        ));
        tr.is_sentence_end = true;
        if tr.is_linebreak() {
            tr.set_paragraph_end();
        }
    }

    AnalyzedSentence {
        text: text.to_string(),
        offset: 0,
        tokens,
        pre_disambig_tokens: Vec::new(),
        pre_disambig_detached: Vec::new(),
    }
}

/// Tokenize one sentence like LT's `EnglishWordTokenizer` and build the
/// analyzed token stream (SENT_START entry + whitespace entries + SENT_END).
pub fn analyze_sentence(tagger: &lt_tagger::EnglishTagger, text: &str) -> AnalyzedSentence {
    let callback = |w: &str| tagger.is_tagged(w);
    let tokenizer = lt_tokenize::EnglishWordTokenizer::new(&callback);
    let raw_tokens = tokenizer.tokenize(text);

    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(raw_tokens.len() + 1);

    // synthetic sentence-start entry
    // synthetic sentence-start entry (LT: AnalyzedToken("", "SENT_START", null))
    tokens.push(AnalyzedTokenReadings {
        readings: vec![AnalyzedToken::new(
            "",
            None,
            Some(sentence_start_tag().to_string()),
        )],
        chunk_tags: Vec::new(),
        whitespace_before: false,
        start_pos: 0,
        raw_byte_len: 0,
        is_whitespace: false,
        is_sentence_start: true,
        is_sentence_end: false,
        is_paragraph_end: false,
        is_tagged: false,
        is_immunized: false,
        is_ignore_spelling: false,
        has_typographic_apostrophe: false,
        is_pos_tag_unknown: false,
    });

    let mut byte_pos = 0usize;
    let mut prev_was_whitespace = false;
    let mut last_non_ws_idx: Option<usize> = None;
    for token in raw_tokens {
        // Java `AnalyzedTokenReadings.isWhitespace` (`StringTools.isWhitespace`)
        let is_whitespace = lt_core::is_whitespace(&token);
        let readings = if is_whitespace {
            vec![AnalyzedToken::new(token.clone(), None, None)]
        } else {
            tagger.tag_word(&token)
        };
        let is_tagged = readings.iter().any(|r| r.pos_tag.is_some());
        let is_pos_tag_unknown = readings.len() == 1 && readings[0].pos_tag.is_none();
        // EnglishTagger typographic-apostrophe flag (ApostropheTypeFilter)
        let has_typographic_apostrophe = token.chars().count() > 1 && token.contains('’');
        let raw_byte_len = token.len();
        tokens.push(AnalyzedTokenReadings {
            readings,
            chunk_tags: Vec::new(),
            whitespace_before: prev_was_whitespace,
            start_pos: byte_pos,
            raw_byte_len,
            is_whitespace,
            is_sentence_start: false,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe,
            is_pos_tag_unknown,
        });
        if !is_whitespace {
            last_non_ws_idx = Some(tokens.len() - 1);
        }
        byte_pos += token.len();
        prev_was_whitespace = is_whitespace;
    }

    // add SENT_END reading to the last non-whitespace token (LT
    // `AnalyzedTokenReadings.setSentEnd`: no duplicate when the dictionary
    // already produced a SENT_END reading; `addReading` drops the trailing
    // unknown-word fallback)
    if let Some(idx) = last_non_ws_idx {
        let tr = &mut tokens[idx];
        if !tr
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref() == Some("SENT_END"))
        {
            let surface = tr
                .readings
                .first()
                .map(|r| r.token.clone())
                .unwrap_or_default();
            let lemma = tr.readings.first().and_then(|r| r.stem.clone());
            tr.add_reading(AnalyzedToken::new(
                surface,
                lemma,
                Some(sentence_end_tag().to_string()),
            ));
        }
        tr.is_sentence_end = true;
    } else if let Some(tr) = tokens.last_mut() {
        // LT `getRawAnalyzedSentence`: a sentence that is all whitespace gets
        // SENT_END on its last token; if that token is a linebreak it is also
        // the paragraph end (PARA_END reading in Java).
        if !tr.is_sentence_end {
            tr.add_reading(AnalyzedToken::new(
                tr.surface().to_string(),
                tr.readings.first().and_then(|r| r.stem.clone()),
                Some(sentence_end_tag().to_string()),
            ));
            tr.is_sentence_end = true;
        }
        if tr.is_linebreak() {
            tr.set_paragraph_end();
        }
    }

    AnalyzedSentence {
        text: text.to_string(),
        offset: 0,
        tokens,
        pre_disambig_tokens: Vec::new(),
        pre_disambig_detached: Vec::new(),
    }
}

impl std::fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pipeline")
            .field("lang", &self.lang)
            .field("grammar_rules", &self.grammar.rules.len())
            .field("compiled_rules", &self.compiled_rules.len())
            .field("disambig_rules", &self.disambiguator.rules_len())
            .field("skipped_counts", &self.skipped_counts)
            .finish()
    }
}

impl std::fmt::Debug for CompiledRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledRule")
            .field("rule_id", &self.rule_id)
            .field("sub_id", &self.sub_id)
            .finish()
    }
}

/// LT `PatternRuleHandler.replaceSpacesInRegex` (smart regexp mode): spaces
/// outside character classes match optional Unicode whitespace runs.
fn replace_spaces_in_regex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_bracket = false;
    for c in s.chars() {
        match c {
            '[' => {
                in_bracket = true;
                out.push(c);
            }
            ']' => {
                in_bracket = false;
                out.push(c);
            }
            ' ' if !in_bracket => out.push_str("(?:[\\s\u{00A0}\u{202F}]+)"),
            _ => out.push(c),
        }
    }
    out
}

/// Resolve `<suggestion>` parts for `<regexp>` rules: `<match no>` refers to
/// capture groups, `regexp_match`/`regexp_replace` use replace-first, and
/// case conversion uses the original group as the case sample.
fn resolve_regexp_suggestions(
    suggestions: &[Vec<SuggestionPart>],
    caps: &fancy_regex::Captures<'_>,
) -> Vec<Suggestion> {
    let mut out = Vec::new();
    for parts in suggestions {
        let mut s = String::new();
        for part in parts {
            match part {
                SuggestionPart::Literal(text) => s.push_str(text),
                SuggestionPart::MatchRef(spec) => {
                    let Some(g) = caps.get(spec.no) else {
                        continue;
                    };
                    let model = g.as_str();
                    let mut value = model.to_string();
                    if let Some(rm) = &spec.regexp_match {
                        // Java accepts nested quantifiers like `X*{1,30}`
                        // (`(X*){1,30}` ≡ `X*`), which the Rust engines
                        // read as a literal brace (D-022)
                        let rm = pm::normalize_java_quantifiers(rm);
                        if let Ok(re) = fancy_regex::Regex::new(&rm) {
                            let replacement = pm::normalize_java_replacement(
                                spec.regexp_replace.as_deref().unwrap_or(""),
                                re.captures_len().saturating_sub(1),
                            );
                            let new = re.replace(value.as_str(), replacement.as_str());
                            value = new.into_owned();
                        }
                    }
                    if !spec.case_conversion.is_empty() {
                        value = pm::apply_case_conversion(&value, &spec.case_conversion, model);
                    }
                    s.push_str(&value);
                }
            }
        }
        // Java keeps empty replacements (`<suggestion/>` in a `<regexp>`
        // rule, e.g. LOOSE_ACCENTS' "remove the loose accent" alternative)
        let s = pm::expand_backrefs(&s, &|n| caps.get(n).map(|g| g.as_str().to_string()));
        out.push(Suggestion {
            value: s,
            short_description: None,
        });
    }
    out
}

/// Expand variable-length lookarounds into constant-size chains so that
/// `fancy-regex` (which, unlike Java, requires constant-size lookbehinds)
/// can compile them. `(?<!A-?[0-9.]{0,5})` becomes
/// `(?<!A)(?<!A-)(?<!A[0-9.])...` — each alternative matches a fixed length,
/// which is equivalent for negative lookbehind; positive lookbehind
/// alternatives are joined with `|`.
///
/// Only top-level `?` and bounded `{m,n}` quantifiers are expanded; patterns
/// with unbounded quantifiers inside lookbehinds (or excessive expansion)
/// pass through unchanged and fail compilation as before.
fn expand_lookbehinds(pattern: &str) -> String {
    const MAX_PATHS: usize = 64;
    let mut out = String::with_capacity(pattern.len());
    let mut rest = pattern;
    loop {
        let next = match ["(?<!", "(?<="].iter().filter_map(|m| rest.find(m)).min() {
            Some(pos) => pos,
            None => {
                out.push_str(rest);
                return out;
            }
        };
        let negative = rest[next..].starts_with("(?<!");
        let prefix_len = 4;
        out.push_str(&rest[..next]);
        let Some(close) = find_group_close(&rest[next..]) else {
            out.push_str(&rest[next..next + prefix_len]);
            rest = &rest[next + prefix_len..];
            continue;
        };
        let body = &rest[next + prefix_len..next + close];
        rest = &rest[next + close + 1..];
        // a negative lookbehind wrapped in a single group is redundant:
        // `(?<!(a|bb|ccc))` ≡ `(?<!a)(?<!bb)(?<!ccc)` — strip it so the
        // inner alternation branches can be expanded. The first expanded
        // path keeps a capturing group so group numbering of the rest of
        // the pattern is preserved (Java numbers lookbehind groups too;
        // they never capture when the negative assertion succeeds).
        let body_owned;
        let mut wrapped_first_path = false;
        let body = if negative
            && body.starts_with('(')
            && body.ends_with(')')
            && find_group_close(body) == Some(body.len() - 1)
        {
            body_owned = &body[1..body.len() - 1];
            wrapped_first_path = true;
            body_owned
        } else {
            body
        };
        match expand_body_paths(body) {
            Some(paths) if paths.len() <= MAX_PATHS && paths.iter().all(|p| !p.is_empty()) => {
                if negative {
                    for (i, p) in paths.iter().enumerate() {
                        if i == 0 && wrapped_first_path {
                            out.push_str("(?!(");
                            out.push_str(p);
                            out.push_str("))");
                        } else {
                            out.push_str("(?<!");
                            out.push_str(p);
                            out.push(')');
                        }
                    }
                } else {
                    out.push_str("(?<=");
                    out.push_str(&paths.join("|"));
                    out.push(')');
                }
            }
            _ => {
                // leave unchanged (either trivially empty or too complex)
                out.push_str(if negative { "(?<!" } else { "(?<=" });
                out.push_str(body);
                out.push(')');
            }
        }
    }
}

/// Find the index of the `)` closing the group that opens at position 0 of
/// `s` (`s` starts with the group opener), respecting escapes and classes.
fn find_group_close(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0usize;
    let mut i = 0usize;
    let mut in_class = false;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'[' if !in_class => in_class = true,
            b']' if in_class => in_class = false,
            b'(' if !in_class => depth += 1,
            b')' if !in_class => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Split a regex body into top-level units (verbatim strings, possibly with
/// a quantifier) and expand bounded quantifiers into all fixed-length paths.
/// Top-level `|` branches are expanded independently and the path sets are
/// unioned (for negative lookbehind, each path gets its own assertion).
fn expand_body_paths(body: &str) -> Option<Vec<String>> {
    let mut paths: Vec<String> = Vec::new();
    for branch in split_top_level_branches(body) {
        let branch_paths = expand_branch_paths(&branch)?;
        paths.extend(branch_paths);
    }
    Some(paths)
}

/// Split on top-level `|` (outside groups/classes/escapes).
fn split_top_level_branches(body: &str) -> Vec<String> {
    let mut branches = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut in_class = false;
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                current.push(c);
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '[' if !in_class => {
                in_class = true;
                current.push('[');
            }
            ']' if in_class => {
                in_class = false;
                current.push(']');
            }
            '(' if !in_class => {
                depth += 1;
                current.push('(');
            }
            ')' if !in_class => {
                depth = depth.saturating_sub(1);
                current.push(')');
            }
            '|' if !in_class && depth == 0 => {
                branches.push(current);
                current = String::new();
            }
            _ => current.push(c),
        }
    }
    branches.push(current);
    branches
}

/// Expand one alternation-free branch into fixed-length paths.
fn expand_branch_paths(branch: &str) -> Option<Vec<String>> {
    let body = branch;
    let chars: Vec<char> = body.chars().collect();
    let mut units: Vec<(String, Option<(u32, u32)>)> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let base_start = i;
        match chars[i] {
            '\\' => i += 2,
            '[' => {
                i += 1;
                while i < chars.len() && chars[i] != ']' {
                    if chars[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            '(' => {
                let tail: String = chars[i..].iter().collect();
                let close = find_group_close(&tail)?;
                i += close + 1;
            }
            ')' => return None,
            _ => i += 1,
        }
        if i > chars.len() {
            return None;
        }
        let base: String = chars[base_start..i].iter().collect();
        let quantifier = if i < chars.len() {
            match chars[i] {
                '?' => {
                    i += 1;
                    Some((0, 1))
                }
                '{' => {
                    let close = chars[i..].iter().position(|c| *c == '}')?;
                    let spec: String = chars[i + 1..i + close].iter().collect();
                    let (m, n) = match spec.split_once(',') {
                        Some((m, n)) => (m.parse::<u32>().ok()?, n.parse::<u32>().ok()?),
                        None => {
                            let m = spec.parse::<u32>().ok()?;
                            (m, m)
                        }
                    };
                    i += close + 1;
                    Some((m, n))
                }
                '*' | '+' => return None,
                _ => None,
            }
        } else {
            None
        };
        units.push((base, quantifier));
    }
    let mut paths: Vec<String> = vec![String::new()];
    for (base, quantifier) in units {
        match quantifier {
            None => {
                for p in &mut paths {
                    p.push_str(&base);
                }
            }
            Some((m, n)) => {
                if n > 10 {
                    return None;
                }
                let mut next = Vec::new();
                for p in &paths {
                    for k in m..=n {
                        let mut candidate = p.clone();
                        for _ in 0..k {
                            candidate.push_str(&base);
                        }
                        next.push(candidate);
                    }
                }
                paths = next;
            }
        }
    }
    Some(paths)
}

/// Compile the XML rules of `grammar` into pattern/regexp rules (Java
/// `PatternRuleLoader` + `PatternRuleHandler`), preserving document order,
/// the skip counters and the compile-failure list.
fn compile_rules(
    grammar: &Grammar,
    filters: &lt_pattern::FilterRegistry,
    enabled_rules: &[String],
) -> (Vec<Arc<CompiledRule>>, SkippedCounts, Vec<(String, String)>) {
    let mut compiled_rules = Vec::new();
    let mut skipped = SkippedCounts::default();
    let mut compile_failures: Vec<(String, String)> = Vec::new();
    let explicitly_enabled: HashSet<&str> = enabled_rules.iter().map(|s| s.as_str()).collect();
    // Cheap per-rule gating first (no regex compilation): keep the
    // original counters and order, then compile the expensive pattern
    // regexes in parallel (rule compilation dominates engine startup).
    #[derive(Default, Clone)]
    struct RuleSkip {
        off_by_default: bool,
        /// a failed `<regexp>` body (the rule may still compile as a
        /// pattern rule when it is not a regexp rule)
        regexp_failure: Option<String>,
        filter_unmapped: bool,
        complex: bool,
        /// rule produces no `CompiledRule` at all
        drop: bool,
    }
    enum Pending<'a> {
        Regexp {
            rule: &'a lt_pattern::RuleDef,
            regex: fancy_regex::Regex,
            mark: usize,
            filter: Option<(Arc<dyn lt_pattern::RuleFilter>, String)>,
        },
        Pattern {
            rule: &'a lt_pattern::RuleDef,
            filter: Option<(Arc<dyn lt_pattern::RuleFilter>, String)>,
        },
    }
    let mut skips: Vec<RuleSkip> = vec![RuleSkip::default(); grammar.rules.len()];
    let mut pending: Vec<Pending<'_>> = Vec::with_capacity(grammar.rules.len());
    for (index, rule) in grammar.rules.iter().enumerate() {
        if !rule.default_on && !explicitly_enabled.contains(rule.id.as_str()) {
            skips[index].off_by_default = true;
            continue;
        }
        // LT PatternRuleHandler: UNICODE_CHARACTER_CLASS + case flag;
        // smart mode replaces spaces outside character classes
        let mut regexp_mark = None;
        let mut regexp_failure = None;
        let regex = rule.regexp.as_ref().and_then(|spec| {
            let pattern = if spec.exact {
                spec.pattern.clone()
            } else {
                replace_spaces_in_regex(&spec.pattern)
            };
            let pattern = expand_lookbehinds(&pattern);
            let anchored = if spec.case_sensitive {
                pattern
            } else {
                format!("(?i){pattern}")
            };
            match fancy_regex::Regex::new(&anchored) {
                Ok(re) => {
                    regexp_mark = Some(spec.mark);
                    Some(re)
                }
                Err(e) => {
                    regexp_failure = Some(e.to_string());
                    None
                }
            }
        });
        skips[index].regexp_failure = regexp_failure;
        if rule.is_regexp_rule && regex.is_none() {
            skips[index].drop = true;
            continue;
        }
        let filter = match &rule.filter {
            Some(spec) => match filters.get(&spec.class) {
                Some(f) => Some((f, spec.args.clone())),
                None => {
                    skips[index].filter_unmapped = true;
                    skips[index].drop = true;
                    continue;
                }
            },
            None => None,
        };
        if let Some(re) = regex {
            pending.push(Pending::Regexp {
                rule,
                regex: re,
                mark: regexp_mark.unwrap_or(1),
                filter,
            });
            continue;
        }
        if rule.complex_pattern || rule.pattern.tokens.is_empty() {
            skips[index].complex = true;
            skips[index].drop = true;
            continue;
        }
        pending.push(Pending::Pattern { rule, filter });
    }

    enum PatternResult {
        Ok {
            compiled: Vec<pm::CompiledPattern>,
            antipatterns: Vec<Arc<pm::CompiledPattern>>,
            context_for_sure_match: i32,
        },
        Failed(String),
    }
    let compile_pattern_rule = |rule: &lt_pattern::RuleDef| -> PatternResult {
        let compiled = match lt_pattern::compile_patterns(
            &rule.pattern.tokens,
            rule.pattern.marker_start,
            rule.pattern.marker_end,
        ) {
            Ok(mut c) => {
                // `PatternRuleMatcher.testAllReadings` skips immunized
                // tokens (antipatterns do not)
                for pattern in &mut c {
                    pattern.skip_immunized = true;
                }
                c
            }
            Err(e) => return PatternResult::Failed(e),
        };
        let context_for_sure_match = compiled
            .iter()
            .map(pm::CompiledPattern::estimate_context_for_sure_match)
            .max()
            .unwrap_or(0);
        let antipatterns: Vec<Arc<pm::CompiledPattern>> = rule
            .antipatterns
            .iter()
            .filter_map(|ap| {
                lt_pattern::compile_patterns(&ap.tokens, ap.marker_start, ap.marker_end).ok()
            })
            .flatten()
            .map(Arc::new)
            .collect();
        PatternResult::Ok {
            compiled,
            antipatterns,
            context_for_sure_match,
        }
    };

    // Compile the pattern rules in parallel (by far the most expensive
    // part of engine construction: thousands of regexes).
    let pattern_positions: Vec<usize> = pending
        .iter()
        .enumerate()
        .filter_map(|(i, p)| matches!(p, Pending::Pattern { .. }).then_some(i))
        .collect();
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(8);
    let mut pattern_results: Vec<Option<PatternResult>> =
        (0..pattern_positions.len()).map(|_| None).collect();
    if threads > 1 && pattern_positions.len() > 64 {
        let chunk_size = pattern_positions.len().div_ceil(threads);
        let pending_ref = &pending;
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for (chunk_index, chunk) in pattern_positions.chunks(chunk_size).enumerate() {
                let compile_pattern_rule = &compile_pattern_rule;
                handles.push(scope.spawn(move || {
                    let results: Vec<PatternResult> = chunk
                        .iter()
                        .map(|&pos| match &pending_ref[pos] {
                            Pending::Pattern { rule, .. } => compile_pattern_rule(rule),
                            Pending::Regexp { .. } => unreachable!(),
                        })
                        .collect();
                    (chunk_index, results)
                }));
            }
            for handle in handles {
                let (chunk_index, results) = handle
                    .join()
                    .unwrap_or_else(|_| panic!("rule compilation thread panicked"));
                for (offset, result) in results.into_iter().enumerate() {
                    pattern_results[chunk_index * chunk_size + offset] = Some(result);
                }
            }
        });
    } else {
        for (slot, &pos) in pattern_positions.iter().enumerate() {
            if let Pending::Pattern { rule, .. } = &pending[pos] {
                pattern_results[slot] = Some(compile_pattern_rule(rule));
            }
        }
    }

    // Assemble in document order (counter/failure order stays identical).
    let mut pending_iter = pending.into_iter();
    let mut pattern_slot = 0usize;
    for (index, rule) in grammar.rules.iter().enumerate() {
        let sk = &skips[index];
        if sk.off_by_default {
            skipped.off_by_default += 1;
            continue;
        }
        if let Some(e) = &sk.regexp_failure {
            skipped.uncompilable += 1;
            compile_failures.push((rule.id.clone(), e.clone()));
        }
        if sk.filter_unmapped {
            skipped.filters += 1;
            let class = rule
                .filter
                .as_ref()
                .map(|f| f.class.clone())
                .unwrap_or_default();
            compile_failures.push((rule.id.clone(), format!("unmapped filter class {class}")));
            continue;
        }
        if sk.complex {
            skipped.complex += 1;
            continue;
        }
        if sk.drop {
            continue;
        }
        match pending_iter.next() {
            None => {}
            Some(Pending::Regexp {
                rule,
                regex,
                mark,
                filter,
            }) => {
                compiled_rules.push(Arc::new(CompiledRule {
                    rule_id: rule.id.clone(),
                    sub_id: rule.sub_id.clone(),
                    category_id: rule.category_id.clone().unwrap_or_default(),
                    category_name: rule.category_name.clone().unwrap_or_default(),
                    message: rule.message.clone().unwrap_or_default(),
                    short_message: rule.short.clone(),
                    compiled: Vec::new(),
                    suggestions: rule.suggestions.clone(),
                    suggestion_suppress: rule.suggestion_suppress.clone(),
                    message_match_refs: rule.message_match_refs.clone(),
                    filter,
                    antipatterns: Vec::new(),
                    regex: Some((regex, mark)),
                    description: rule.name.clone().unwrap_or_default(),
                    issue_type: rule.issue_type.clone(),
                    context_for_sure_match: 0,
                    min_prev_matches: rule.min_prev_matches,
                    distance_tokens: rule.distance_tokens,
                    category_default_on: rule.category_default_on,
                    tags: rule.tags.clone(),
                    tone_tags: rule.tone_tags.clone(),
                    goal_specific: rule.goal_specific,
                    min_token_count: 0,
                    hints: Vec::new(),
                    message_suppress_misspelled: false,
                    raw_pos: false,
                }));
            }
            Some(Pending::Pattern { rule, filter }) => {
                let result = pattern_results[pattern_slot].take();
                pattern_slot += 1;
                match result {
                    Some(PatternResult::Failed(e)) => {
                        skipped.uncompilable += 1;
                        compile_failures.push((rule.id.clone(), e));
                    }
                    Some(PatternResult::Ok {
                        compiled,
                        antipatterns,
                        context_for_sure_match,
                    }) => {
                        let hint_info = pm::pattern_hints(&rule.pattern.tokens);
                        compiled_rules.push(Arc::new(CompiledRule {
                            rule_id: rule.id.clone(),
                            sub_id: rule.sub_id.clone(),
                            category_id: rule.category_id.clone().unwrap_or_default(),
                            category_name: rule.category_name.clone().unwrap_or_default(),
                            message: rule.message.clone().unwrap_or_default(),
                            short_message: rule.short.clone(),
                            compiled,
                            suggestions: rule.suggestions.clone(),
                            suggestion_suppress: rule.suggestion_suppress.clone(),
                            message_match_refs: rule.message_match_refs.clone(),
                            filter,
                            antipatterns,
                            regex: None,
                            description: rule.name.clone().unwrap_or_default(),
                            issue_type: rule.issue_type.clone(),
                            context_for_sure_match,
                            min_prev_matches: rule.min_prev_matches,
                            distance_tokens: rule.distance_tokens,
                            category_default_on: rule.category_default_on,
                            tags: rule.tags.clone(),
                            tone_tags: rule.tone_tags.clone(),
                            goal_specific: rule.goal_specific,
                            min_token_count: hint_info.0,
                            hints: hint_info.1,
                            message_suppress_misspelled: rule.message_suppress_misspelled,
                            raw_pos: rule.pattern.raw_pos,
                        }));
                    }
                    None => {}
                }
            }
        }
    }

    (compiled_rules, skipped, compile_failures)
}

/// Foundations shared by the hand-authored languages (no legacy Java
/// module): SRX, grammar.xml (+ optional style.xml), compiled rules and the
/// XML disambiguator.
struct HandAuthoredFoundations {
    srx: lt_tokenize::SrxTokenizer,
    grammar: Grammar,
    unify_config: lt_pattern::EquivalenceConfig,
    compiled_rules: Vec<Arc<CompiledRule>>,
    skipped: SkippedCounts,
    compile_failures: Vec<(String, String)>,
    disambiguator: lt_disambig::XmlDisambiguator,
}

impl Pipeline {
    /// Build the English pipeline from the vendored data directory. `today`
    /// overrides the system date for the date filters (tests/parity);
    /// `enabled_rules` are compiled even when `default="off"`.
    pub fn new_english(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let timing = std::env::var("LT_TIMING").is_ok();
        // `Instant::now()` traps on wasm32-unknown-unknown; only arm the
        // timer when LT_TIMING requested it (native/WASI runs).
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "en_two")?;

        mark("srx");
        let tagger = Self::load_english_tagger(data_dir)?;

        mark("tagger dict+manual");
        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::En))?;
        if data_dir.style_path(Lang::En).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::En))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        // variant-specific rule files (`Language.getRuleFileNames` loads
        // en/<variant>/grammar.xml and style.xml after the common files)
        let variant_name = variant.unwrap_or("en-US");
        for name in ["grammar.xml", "style.xml"] {
            let path = data_dir
                .path()
                .join("en/rules")
                .join(variant_name)
                .join(name);
            if path.lt_exists() {
                let extra = Grammar::load_file(&path)?;
                grammar.rules.extend(extra.rules);
                grammar.categories.extend(extra.categories);
                grammar.equivalence_defs.extend(extra.equivalence_defs);
            }
        }
        mark("grammar+style+variant");
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // P1.6 filter environment: en_US speller (default spelling rule),
        // tagger-dictionary speller (FindSuggestionsFilter), multitoken
        // speller (multiwords.txt + spelling_global.txt)
        mark("unify config");
        let hunspell_dir = data_dir.path().join("en/hunspell");
        let us_speller = lt_spell::SpellChecker::new(
            &hunspell_dir.join("en_US.dict"),
            &hunspell_dir.join("en_US.info"),
        )?;
        let dict_speller = lt_spell::morfologik::MorfologikSpeller::from_dict_file(
            &data_dir.path().join("en/dictionaries/english.dict"),
            &data_dir.path().join("en/dictionaries/english.info"),
            1,
        )?;
        let tagger_for_misspelled = Arc::clone(&tagger);
        let speller_for_misspelled = us_speller.clone();
        let is_misspelled: IsMisspelled = Arc::new(move |word: &str| {
            if word.is_empty() {
                return false;
            }
            // the spelling rule consults the dictionary with straight
            // apostrophes (EnglishTagger's typewriter-apostrophe hack)
            let normalized = if word.chars().count() > 1 && word.contains('’') {
                word.replace('’', "'")
            } else {
                word.to_string()
            };
            let tagged = |w: &str| tagger_for_misspelled.is_tagged(w);
            let tokenizer = lt_tokenize::EnglishWordTokenizer::new(&tagged);
            tokenizer.tokenize(&normalized).iter().any(|t| {
                if speller_for_misspelled.is_correct(t) {
                    return false;
                }
                // `MorfologikSpellerRule.isMisspelled`: hyphen compounds are
                // spelled correctly when every part is
                if t.contains('-') {
                    return split_compound(t)
                        .iter()
                        .any(|part| !part.is_empty() && !speller_for_misspelled.is_correct(part));
                }
                true
            })
        });
        mark("us_speller(SpellChecker)+multitoken");
        let multitoken = MultitokenSpeller::load_english(
            &[
                data_dir.path().join("en/words/multiwords.txt"),
                data_dir.path().join("core/spelling_global.txt"),
            ],
            Arc::clone(&is_misspelled),
        );
        let env = Arc::new(crate::en::filters::EnFilterEnv {
            tagger: Arc::clone(&tagger),
            dict_speller,
            us_speller,
            multitoken,
            is_misspelled,
            today: today.unwrap_or_else(Ymd::today),
        });
        let filters = crate::en::filters::english_filter_registry(env);

        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("filters+rule compile");
        let global_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("core/spelling_global.txt"),
            // Java: MultiWordChunker.getInstance("/spelling_global.txt", true, true, false, _NONE_)
            true,
            true,
            true,
            Some("_NONE_".to_string()),
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));

        let multiword_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("en/words/multiwords.txt"),
            true,
            true,
            false,
            None,
            true,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));

        // Java `EnglishHybridDisambiguator` loads the language rules followed
        // by the global rules (`XmlRuleDisambiguator(lang, true)`)
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::En),
            Some(&global_disambig),
        )?;
        let models_dir = data_dir.path().join("en/models");
        mark("disambiguator");
        let english_chunker = if models_dir.join("en-chunker.bin").lt_exists() {
            Some(lt_chunk::EnglishChunker::load(&models_dir)?)
        } else {
            None
        };
        // LT server resolves plain `en` to its default variant (en-US); the
        // spelling rule follows the variant's dictionary
        mark("chunker");
        let synthesizer = Some(Arc::new({
            let mut synth = crate::en::synthesizer::EnglishSynthesizer::from_data(data_dir.path())?;
            synth.set_tagger(Arc::clone(&tagger));
            synth
        }));
        let mut disambiguator = disambiguator;
        if let Some(synth) = &synthesizer {
            disambiguator.set_synthesizer(synth.clone());
        }
        let mut spelling_rule = match variant {
            Some("en-GB") => {
                crate::en::spelling::SpellingRule::british(data_dir.path(), Arc::clone(&tagger))?
            }
            _ => crate::en::spelling::SpellingRule::american(data_dir.path(), Arc::clone(&tagger))?,
        };
        if let Some(synth) = &synthesizer {
            spelling_rule.set_synthesizer(Arc::clone(synth));
        }
        let spelling = Some(Arc::new(spelling_rule));
        let avs_an = Some(crate::en::avs_an::AvsAnRule::from_data(data_dir.path())?);
        let compound = match &spelling {
            Some(spelling) => Some(crate::compound::CompoundRule::english(
                data_dir.path(),
                Arc::clone(spelling),
            )?),
            None => None,
        };
        let contractions = Some(crate::en::contractions::ContractionSpellingRule::from_data(
            data_dir.path(),
        )?);
        let wrong_word_in_context =
            Some(crate::wrong_word_in_context::WrongWordInContextRule::english(data_dir.path())?);
        let dash = Some(crate::dash::english(data_dir.path())?);
        let simple_replace = crate::simple_replace::english_instances(data_dir.path(), variant)?;
        let word_coherency = Some(crate::word_coherency::WordCoherencyRule::from_data(
            data_dir.path(),
        ));
        let specific_case = Some(crate::en::specific_case::SpecificCaseRule::from_data(
            data_dir.path(),
        ));
        // `ReadabilityRule` (both variants) and `EnglishRepeatedWordsRule`
        // are text-level rules; both are default off / picky.
        let readability = vec![
            crate::readability::ReadabilityRule::difficult(),
            crate::readability::ReadabilityRule::simple(),
        ];
        let repeated_words = Some(crate::repeated_words::RepeatedWordsRule::english(
            data_dir.path(),
        )?);
        mark("spelling rule");
        if disambiguator.skipped > 0 {
            eprintln!(
                "disambiguation: {} actions skipped (complex/uncompilable patterns)",
                disambiguator.skipped
            );
        }

        Ok(Self {
            lang: Lang::En,
            unify_config,
            srx,
            tagger: Some(tagger),
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker,
            multiword_chunker,
            disambiguator,
            english_chunker,
            spelling,
            avs_an,
            compound,
            contractions,
            wrong_word_in_context,
            dash,
            synthesizer,
            simple_replace,
            word_coherency,
            specific_case,
            readability,
            repeated_words,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// The English tagger (`en/dictionaries/english.dict` + the plain-text
    /// manual taggers, `BaseTagger` semantics). Also used by the German
    /// disambiguator's `IsEnglishWordFilter`.
    pub(crate) fn load_english_tagger(
        data_dir: &lt_data::DataDir,
    ) -> Result<Arc<lt_tagger::EnglishTagger>> {
        let dict_dir = data_dir.path().join("en/dictionaries");
        let info = lt_tagger::DictionaryInfo::load(&dict_dir.join("english.info"))?;
        let dict = lt_tagger::Dictionary::load(&dict_dir.join("english.dict"), &info)?;
        // BaseTagger combines the binary dictionary with the plain-text manual
        // taggers (`added.txt`/`removed.txt` and their custom complements)
        let words_dir = data_dir.path().join("en/words");
        let manual = lt_tagger::ManualTagger::load(&[
            &words_dir.join("added.txt"),
            &words_dir.join("added_custom.txt"),
        ])?;
        let removals = lt_tagger::ManualTagger::load(&[
            &words_dir.join("removed.txt"),
            &words_dir.join("removed_custom.txt"),
        ])?;
        Ok(Arc::new(lt_tagger::EnglishTagger::with_manual(
            dict, manual, removals,
        )))
    }

    /// Build the German pipeline (`de-DE` default, `de-AT`, `de-CH`).
    pub fn new_german(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let timing = std::env::var("LT_TIMING").is_ok();
        // `Instant::now()` traps on wasm32-unknown-unknown; only arm the
        // timer when LT_TIMING requested it (native/WASI runs).
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] de {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let variant = variant.unwrap_or("de-DE").to_string();
        let srx = crate::de::pipeline::german_srx(data_dir)?;
        mark("srx");
        let synth = Arc::new(lt_tagger::GermanSynthesizer::from_data(data_dir.path())?);
        let tagger = if variant == "de-CH" {
            crate::de::pipeline::GermanTaggerKind::Swiss(Arc::new(
                lt_tagger::SwissGermanTagger::load(data_dir.path())?,
            ))
        } else {
            crate::de::pipeline::GermanTaggerKind::De(Arc::new(lt_tagger::GermanTagger::load(
                data_dir.path(),
            )?))
        };
        let synth_adapter = Arc::new(crate::de::synthesizer::GermanSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: tagger.clone(),
        });
        let base_tagger = match &tagger {
            crate::de::pipeline::GermanTaggerKind::De(t) => Arc::clone(t),
            crate::de::pipeline::GermanTaggerKind::Swiss(t) => t.base_arc(),
        };

        mark("tagger");
        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::De))?;
        if data_dir.style_path(Lang::De).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::De))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        // `Language.getRuleFileNames`: GermanyGerman/AustrianGerman append
        // `de/de-DE-AT/grammar.xml` (SwissGerman does not).
        if variant == "de-DE" || variant == "de-AT" {
            let path = data_dir.path().join("de/rules/de-DE-AT/grammar.xml");
            if path.lt_exists() {
                let extra = Grammar::load_file(&path)?;
                grammar.rules.extend(extra.rules);
                grammar.categories.extend(extra.categories);
                grammar.equivalence_defs.extend(extra.equivalence_defs);
            }
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // one speller instance: the filter environment and the German rule
        // set share it (each load costs ~350 MB and several seconds, D-033)
        mark("grammar");
        let spelling = Arc::new(crate::de::spelling::GermanSpellingRule::load(
            data_dir.path(),
            &variant,
            Arc::clone(&base_tagger),
            Arc::clone(&synth),
        )?);
        mark("speller");
        let multitoken = {
            let speller = Arc::clone(&spelling);
            crate::multitoken::MultitokenSpeller::load_german(
                &[
                    data_dir.path().join("de/words/multitoken-suggest.txt"),
                    data_dir.path().join("core/spelling_global.txt"),
                    data_dir.path().join("de/hunspell/spelling.txt"),
                ],
                Arc::new(move |word: &str| speller.is_misspelled(word)),
            )
        };
        let env = Arc::new(crate::de::filters::DeFilterEnv {
            tagger: Arc::clone(&base_tagger),
            spelling: Arc::clone(&spelling),
            // the pinned German build has no en-US tagger (see `DeFilterEnv`)
            english_tagger: None,
            synth: Arc::clone(&synth_adapter),
            today: today.unwrap_or_else(Ymd::today),
            compound_check_path: data_dir.path().join("de/rules/addedCompound.txt"),
            multitoken,
        });
        let filters = crate::de::filters::german_filter_registry(env);
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("filters+rules");
        // `GermanRuleDisambiguator`: multitoken-ignore → spelling_global →
        // multitoken-suggest → XML rules (+ global rules)
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::De),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters.clone());
        let global_chunker = load_chunker(&data_dir.path().join("core/spelling_global.txt"), false);
        let multitoken_chunker = load_chunker(
            &data_dir.path().join("de/words/multitoken-ignore.txt"),
            true,
        );
        let multitoken_suggest_chunker = load_chunker(
            &data_dir.path().join("de/words/multitoken-suggest.txt"),
            true,
        );

        mark("disambiguator+chunkers");
        let german = crate::de::pipeline::GermanPipeline {
            tagger,
            synthesizer: Arc::clone(&synth),
            synth_adapter,
            global_chunker,
            multitoken_chunker,
            multitoken_suggest_chunker,
            disambiguator,
            chunker: lt_chunk::german::GermanChunker::new()?,
            old_spelling: crate::de::old_spelling::OldSpellingRule::load(
                data_dir.path(),
                &variant,
                &synth,
            )?,
            compound_infinitiv: crate::de::compound_infinitiv::CompoundInfinitivRule::load(
                data_dir.path(),
            )?,
            agreement: crate::de::agreement::AgreementRule::new(
                Arc::clone(&synth),
                Arc::clone(&base_tagger),
                Arc::clone(&spelling),
            ),
            agreement2: crate::de::agreement::AgreementRule2::new(Arc::clone(&synth)),
            case_rule: crate::de::case_rule::CaseRule::load(
                data_dir.path(),
                Arc::clone(&base_tagger),
                Arc::clone(&spelling),
            )?,
            compound: crate::compound::CompoundRule::german(
                data_dir.path(),
                variant.as_str() == "de-CH",
                Arc::clone(&spelling),
            )?,
            readability: vec![
                crate::readability::ReadabilityRule::german_difficult(),
                crate::readability::ReadabilityRule::german_simple(),
            ],
            spelling,
            repeated_words: crate::repeated_words::RepeatedWordsRule::german(data_dir.path())?,
            verb_agreement: crate::de::verb_agreement::VerbAgreementRule::new(Arc::clone(&synth)),
            subject_verb_agreement:
                crate::de::subject_verb_agreement::SubjectVerbAgreementRule::new(Arc::clone(
                    &base_tagger,
                )),
            wrong_word_in_context: crate::wrong_word_in_context::WrongWordInContextRule::german(
                data_dir.path(),
            )?,
            word_coherency: crate::word_coherency::WordCoherencyRule::german(data_dir.path()),
            variant,
        };
        Ok(Self {
            lang: Lang::De,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: vec![crate::simple_replace::german_instance(data_dir.path())?],
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: Some(Box::new(german)),
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Build the Spanish pipeline (`es-ES`; see `Spanish.java`).
    pub fn new_spanish(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let timing = std::env::var("LT_TIMING").is_ok();
        // `Instant::now()` traps on wasm32-unknown-unknown; only arm the
        // timer when LT_TIMING requested it (native/WASI runs).
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] es {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let variant = variant.unwrap_or("es-ES").to_string();
        // `SRXSentenceTokenizer` always sets
        // `singleLineBreaksMarksParagraph(false)`, so the Spanish rule group
        // matches through the `es_two` language code.
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "es_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::SpanishTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::SpanishSynthesizer::from_data(data_dir.path())?);
        let synth_adapter = Arc::new(crate::es::synthesizer::SpanishSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Es))?;
        if data_dir.style_path(Lang::Es).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Es))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        // LingoTweaker hand-authored rules (not upstream). Kept in a separate
        // file so `lt-sync import` never overwrites them; see
        // docs/differences.md #8.
        let local_path = data_dir.path().join("es/rules/local.xml");
        if local_path.lt_exists() {
            let local = Grammar::load_file(&local_path)?;
            grammar.rules.extend(local.rules);
            grammar.categories.extend(local.categories);
            grammar.equivalence_defs.extend(local.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // one speller instance shared by the filter environment and the
        // spelling rule
        let spelling = Arc::new(crate::es::spelling::SpanishSpellingRule::load(
            data_dir.path(),
            Arc::clone(&tagger),
        )?);
        mark("speller");
        let multitoken = Arc::new({
            let speller = Arc::clone(&spelling);
            crate::multitoken::MultitokenSpeller::load_spanish(
                &[
                    data_dir.path().join("es/words/multiwords.txt"),
                    data_dir.path().join("core/spelling_global.txt"),
                    data_dir.path().join("es/words/hyphenated_words.txt"),
                ],
                Arc::new(move |word: &str| speller.is_misspelled(word)),
            )
        });
        mark("multitoken");
        let env = Arc::new(crate::es::filters::EsFilterEnv {
            tagger: Arc::clone(&tagger),
            synth: Arc::clone(&synth_adapter),
            spelling: Arc::clone(&spelling),
            multitoken: Arc::clone(&multitoken),
            today: today.unwrap_or_else(Ymd::today),
            data_dir: data_dir.path().to_path_buf(),
        });
        let filters = crate::es::filters::spanish_filter_registry(env);
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("filters+rules");

        // `SpanishHybridDisambiguator`: spelling_global → es/multiwords →
        // XML rules (+ global rules)
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::Es),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters.clone());
        mark("disambiguator");

        let global_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("core/spelling_global.txt"),
            false,
            false,
            true,
            Some("NPCN000".to_string()),
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));
        let multiwords_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("es/words/multiwords.txt"),
            false,
            true,
            true,
            None,
            true,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, true));
        mark("chunkers");

        let compound =
            crate::compound::CompoundRule::spanish(data_dir.path(), Arc::clone(&tagger))?;
        let simple_replace =
            crate::es::simple_replace::SpanishSimpleReplaceRule::load(data_dir.path())?;
        let simple_replace_verbs = crate::es::simple_replace::SpanishSimpleReplaceVerbsRule::load(
            data_dir.path(),
            Arc::clone(&synth),
            Arc::clone(&tagger),
        )?;
        let spanish = crate::es::pipeline::SpanishPipeline {
            tagger,
            synthesizer: synth,
            synth_adapter,
            global_chunker,
            multiwords_chunker,
            disambiguator,
            spelling,
            multitoken,
            simple_replace,
            simple_replace_verbs,
            wrong_word_in_context: crate::wrong_word_in_context::WrongWordInContextRule::spanish(
                data_dir.path(),
            )?,
            compound,
            repeated_words: crate::repeated_words::RepeatedWordsRule::spanish(data_dir.path())?,
        };
        let _ = variant;
        Ok(Self {
            lang: Lang::Es,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: vec![crate::simple_replace::spanish_wikipedia_instance(
                data_dir.path(),
            )?],
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: Some(Box::new(spanish)),
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    pub fn new_french(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let timing = std::env::var("LT_TIMING").is_ok();
        // `Instant::now()` traps on wasm32-unknown-unknown; only arm the
        // timer when LT_TIMING requested it (native/WASI runs).
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] fr {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let variant = variant.unwrap_or("fr").to_string();
        // `SRXSentenceTokenizer` always sets
        // `singleLineBreaksMarksParagraph(false)` -> Java tokenizes with
        // `fr_two`, which cascades the French group and `ByTwoLineBreaks`
        // (the bare `fr` code dropped the two-line-break handling; same
        // class of bug as the Italian `it_two` fix, D-103).
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "fr_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::FrenchTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::FrenchSynthesizer::from_data(data_dir.path())?);
        let synth_adapter = Arc::new(crate::fr::synthesizer::FrenchSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Fr))?;
        if data_dir.style_path(Lang::Fr).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Fr))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        // `WordWithDeterminerFilter.suggestionHasNoErrors` re-checks a
        // candidate against the same unification config.
        let unify_recheck = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        let spelling = Arc::new(crate::fr::spelling::FrenchSpellingRule::load(
            data_dir.path(),
            Arc::clone(&tagger),
        )?);
        mark("speller");
        let multitoken = Arc::new({
            let speller = Arc::clone(&spelling);
            crate::multitoken::MultitokenSpeller::load_french(
                &[
                    data_dir.path().join("fr/words/multiwords.txt"),
                    data_dir.path().join("core/spelling_global.txt"),
                    data_dir.path().join("fr/words/hyphenated_words.txt"),
                ],
                Arc::new(move |word: &str| speller.is_misspelled(word)),
            )
        });
        mark("multitoken");
        let env = Arc::new(crate::fr::filters::FrFilterEnv {
            tagger: Arc::clone(&tagger),
            synth: Arc::clone(&synth_adapter),
            spelling: Arc::clone(&spelling),
            multitoken: Arc::clone(&multitoken),
            today: today.unwrap_or_else(Ymd::today),
            suggestion_checker: Arc::new(std::sync::OnceLock::new()),
        });
        let filters = crate::fr::filters::french_filter_registry(Arc::clone(&env));
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("filters+rules");

        // `FrenchHybridDisambiguator`: spelling_global (tagForNotAddingTags,
        // ignoreSpelling) -> fr/multiwords (removePreviousTags) -> XML rules
        // (+ global rules)
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::Fr),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters.clone());
        mark("disambiguator");

        let global_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("core/spelling_global.txt"),
            true,
            false,
            true,
            Some("_NONE_".to_string()),
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));
        let multiwords_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("fr/words/multiwords.txt"),
            false,
            true,
            true,
            None,
            true,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, true));
        mark("chunkers");

        let compound = crate::compound::CompoundRule::french(data_dir.path(), Arc::clone(&tagger))?;
        let simple_replace =
            crate::fr::simple_replace::FrenchSimpleReplaceRule::load(data_dir.path())?;
        let french = Arc::new(crate::fr::pipeline::FrenchPipeline {
            tagger,
            synthesizer: synth,
            synth_adapter,
            global_chunker,
            multiwords_chunker,
            disambiguator,
            spelling,
            multitoken,
            simple_replace,
            compound,
            repeated_words: crate::repeated_words::RepeatedWordsRule::french(data_dir.path())?,
            question_whitespace: crate::fr::question_whitespace::FrenchQuestionWhitespaceRule::new(
                false,
            ),
            question_whitespace_strict:
                crate::fr::question_whitespace::FrenchQuestionWhitespaceRule::new(true),
        });
        // Install the `CAT_ELISION` re-check hook for `WordWithDeterminerFilter`.
        {
            let recheck_rules: Vec<Arc<CompiledRule>> = compiled_rules
                .iter()
                .filter(|rule| {
                    rule.category_id == "CAT_ELISION"
                        || crate::fr::pipeline::ELISION_RECHECK_RULE_IDS
                            .contains(&rule.rule_id.as_str())
                })
                .cloned()
                .collect();
            let french_hook = Arc::clone(&french);
            let _ = env.suggestion_checker.set(Arc::new(move |text: &str| {
                crate::fr::pipeline::suggestion_has_no_errors(
                    &french_hook,
                    &recheck_rules,
                    &unify_recheck,
                    text,
                )
            }));
        }
        let _ = variant;
        Ok(Self {
            lang: Lang::Fr,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: Some(french),
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Build the Italian pipeline (`it`; see `Italian.java`).
    pub fn new_italian(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let timing = std::env::var("LT_TIMING").is_ok();
        // `Instant::now()` traps on wasm32-unknown-unknown; only arm the
        // timer when LT_TIMING requested it (native/WASI runs).
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] it {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let variant = variant.unwrap_or("it").to_string();
        // `SRXSentenceTokenizer` always sets
        // `singleLineBreaksMarksParagraph(false)`, so the Italian rule group
        // matches through the `it_two` language code.
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "it_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::ItalianTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::ItalianSynthesizer::from_data(data_dir.path())?);
        let synth_adapter = Arc::new(crate::it::ItalianSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let spelling = Arc::new(crate::it::spelling::ItalianSpellingRule::load(
            data_dir.path(),
        )?);
        mark("speller");

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::It))?;
        if data_dir.style_path(Lang::It).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::It))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // Stage 3: the only XML-referenced filter class is the Italian
        // `DateCheckFilter` (rulegroup DATE_WEEKDAY).
        let filters = crate::it::filters::italian_filter_registry(today.unwrap_or_else(Ymd::today));
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("filters+rules");

        // `ItalianRuleDisambiguator`: XML rules (+ global rules)
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::It),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters);
        mark("disambiguator");

        let italian = Arc::new(crate::it::ItalianPipeline {
            tagger,
            synthesizer: synth,
            synth_adapter,
            disambiguator,
            spelling,
        });
        let _ = variant;
        Ok(Self {
            lang: Lang::It,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: Some(italian),
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Portuguese pipeline (stage 1: XML rules). `Portuguese.java` uses the
    /// `PortugueseHybridDisambiguator` order (global chunker →
    /// `pt/multiwords.txt` chunker → XML rules), the plain
    /// `BaseTagger`/`BaseSynthesizer` subset and the default variant is
    /// `pt-PT` (`Portuguese.getDefaultLanguageVariant`).
    pub fn new_portuguese(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let timing = std::env::var("LT_TIMING").is_ok();
        // `Instant::now()` traps on wasm32-unknown-unknown; only arm the
        // timer when LT_TIMING requested it (native/WASI runs).
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] pt {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let variant = variant.unwrap_or("pt-PT").to_string();
        // `SRXSentenceTokenizer` always sets
        // `singleLineBreaksMarksParagraph(false)`, so the Portuguese rule
        // group matches through the `pt_two` language code.
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "pt_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::PortugueseTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::PortugueseSynthesizer::from_data(
            data_dir.path(),
        )?);
        let synth_adapter = Arc::new(crate::pt::PortugueseSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Pt))?;
        if data_dir.style_path(Lang::Pt).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Pt))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        // variant-specific rule files (`Language.getRuleFileNames` loads
        // pt/<variant>/grammar.xml and style.xml after the common files;
        // pt-AO/pt-MZ ship no style.xml)
        for name in ["grammar.xml", "style.xml"] {
            let path = data_dir.path().join("pt/rules").join(&variant).join(name);
            if path.lt_exists() {
                let extra = Grammar::load_file(&path)?;
                grammar.rules.extend(extra.rules);
                grammar.categories.extend(extra.categories);
                grammar.equivalence_defs.extend(extra.equivalence_defs);
            }
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        mark("grammar+style+variant");

        // `MorfologikPortugueseSpellerRule` (3) + `PortugueseMultitokenSpeller`
        let spelling = Arc::new(crate::pt::spelling::PortugueseSpellingRule::load(
            data_dir.path(),
            &variant,
            Arc::clone(&tagger),
            Arc::clone(&synth),
        )?);
        mark("speller");
        let multitoken = Arc::new(crate::multitoken::MultitokenSpeller::load_portuguese(
            &[
                data_dir.path().join("pt/words/multiwords.txt"),
                data_dir.path().join("core/spelling_global.txt"),
                data_dir.path().join("pt/words/hyphenated_words.txt"),
            ],
            {
                let speller = Arc::clone(&spelling);
                Arc::new(move |word: &str| speller.is_misspelled(word))
            },
        ));
        mark("multitoken");

        let filters = crate::pt::filters::portuguese_filter_registry(
            today.unwrap_or_else(Ymd::today),
            Arc::clone(&tagger),
            Arc::clone(&synth_adapter),
            Arc::clone(&spelling),
            Arc::clone(&multitoken),
            data_dir.path(),
            &variant,
        );
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("filters+rules");

        // `PortugueseHybridDisambiguator`: XML rules (+ global rules)
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::Pt),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters);
        mark("disambiguator");

        let global_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("core/spelling_global.txt"),
            // Java: MultiWordChunker.getInstance("/spelling_global.txt", false,
            // true, true, "NPCN000") + setIgnoreSpelling(true)
            true,
            false,
            true,
            Some("NPCN000".to_string()),
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));
        let multiwords_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("pt/words/multiwords.txt"),
            // Java: MultiWordChunker.getInstance("/pt/multiwords.txt", true,
            // true, true) + setRemovePreviousTags(true) + setIgnoreSpelling(true)
            true,
            true,
            true,
            None,
            true,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, true));
        mark("chunkers");

        // `Portuguese.getRelevantRules` compound family: the common
        // `PT_COMPOUNDS_POST_REFORM` (16) + `PT_COLOUR_HYPHENATION` (17),
        // then the variant additions (pt-PT/pt-BR add another post-reform
        // instance + `PT_POSAO_DASH_RULE`; pt-AO/pt-MZ the pre-reform pair).
        let mut compounds = vec![
            crate::compound::CompoundRule::portuguese(data_dir.path(), false)?,
            crate::compound::CompoundRule::portuguese_colour(data_dir.path())?,
        ];
        let mut dashes = Vec::new();
        match variant.as_str() {
            "pt-AO" | "pt-MZ" => {
                compounds.push(crate::compound::CompoundRule::portuguese(
                    data_dir.path(),
                    true,
                )?);
                dashes.push(crate::dash::portuguese(data_dir.path(), true)?);
            }
            _ => {
                compounds.push(crate::compound::CompoundRule::portuguese(
                    data_dir.path(),
                    false,
                )?);
                dashes.push(crate::dash::portuguese(data_dir.path(), false)?);
            }
        }
        let wrong_word_in_context = Some(
            crate::wrong_word_in_context::WrongWordInContextRule::portuguese(data_dir.path())?,
        );
        let word_coherency = crate::word_coherency::WordCoherencyRule::portuguese(data_dir.path());
        let accentuation = crate::pt::accentuation::AccentuationCheckRule::load(data_dir.path());
        let readability = vec![
            crate::readability::ReadabilityRule::portuguese_difficult(),
            crate::readability::ReadabilityRule::portuguese_simple(),
        ];
        let replace_rule2 =
            crate::pt::simple_replace::pt_simple_replace_instances(data_dir.path(), &variant)?;
        let legacy_replace = crate::pt::legacy_simple_replace::pt_legacy_replace_instances(
            data_dir.path(),
            &variant,
            Arc::clone(&synth),
        );
        let replace_order = crate::pt::replace_order(&replace_rule2, &legacy_replace, &variant);
        let portuguese = Arc::new(crate::pt::PortuguesePipeline {
            variant,
            tagger,
            synthesizer: synth,
            synth_adapter,
            global_chunker,
            multiwords_chunker,
            disambiguator,
            spelling,
            multitoken,
            legacy_replace,
            replace_rule2,
            replace_order,
            compounds,
            dashes,
            wrong_word_in_context,
            word_coherency,
            accentuation,
            readability,
        });
        Ok(Self {
            lang: Lang::Pt,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: Some(portuguese),
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Dutch pipeline (stage 1: XML rules). `Dutch.java` uses the
    /// `DutchHybridDisambiguator` order (global chunker → `nl/multiwords.txt`
    /// chunker → XML rules), the plain `BaseTagger`/`BaseSynthesizer` subset
    /// and the `nl-NL` default variant; `BelgianDutch` loads `nl-BE`.
    pub fn new_dutch(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let timing = std::env::var("LT_TIMING").is_ok();
        // `Instant::now()` traps on wasm32-unknown-unknown; only arm the
        // timer when LT_TIMING requested it (native/WASI runs).
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] nl {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let variant = variant.unwrap_or("nl-NL").to_string();
        // `SRXSentenceTokenizer` always sets
        // `singleLineBreaksMarksParagraph(false)`, so the Dutch rule group
        // matches through the `nl_two` language code.
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "nl_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::DutchTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::DutchSynthesizer::from_data(data_dir.path())?);
        let synth_adapter = Arc::new(crate::nl::DutchSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Nl))?;
        if data_dir.style_path(Lang::Nl).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Nl))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        // variant-specific rule files (`Language.getRuleFileNames` loads
        // nl/<variant>/grammar.xml and style.xml after the common files;
        // nl-NL ships no style.xml)
        for name in ["grammar.xml", "style.xml"] {
            let path = data_dir.path().join("nl/rules").join(&variant).join(name);
            if path.lt_exists() {
                let extra = Grammar::load_file(&path)?;
                grammar.rules.extend(extra.rules);
                grammar.categories.extend(extra.categories);
                grammar.equivalence_defs.extend(extra.equivalence_defs);
            }
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        mark("grammar+style+variant");

        // `MorfologikDutchSpellerRule` (5) + `DutchMultitokenSpeller` and
        // `Dutch.getCompoundAcceptor()` (the speller's
        // `ignorePotentiallyMisspelledWord` and the stage-3 `CompoundFilter`)
        let spelling = Arc::new(crate::nl::spelling::DutchSpellingRule::load(
            data_dir.path(),
        )?);
        let compound_acceptor = Arc::new(crate::nl::compound_acceptor::CompoundAcceptor::load(
            data_dir.path(),
            Arc::clone(&tagger),
        ));
        compound_acceptor.set_speller(Arc::clone(&spelling));
        spelling.set_compound_acceptor(Arc::clone(&compound_acceptor));
        tagger.set_compound_acceptor(
            Arc::clone(&compound_acceptor) as Arc<dyn lt_tagger::CompoundPartsProvider>
        );
        mark("speller");
        let multitoken = Arc::new(crate::multitoken::MultitokenSpeller::load_dutch(
            &[
                data_dir.path().join("nl/words/multiwords.txt"),
                data_dir.path().join("core/spelling_global.txt"),
            ],
            {
                let speller = Arc::clone(&spelling);
                Arc::new(move |word: &str| speller.is_misspelled(word))
            },
        ));
        mark("multitoken");

        let filters = crate::nl::filters::dutch_filter_registry(
            today.unwrap_or_else(Ymd::today),
            Arc::clone(&tagger),
            Arc::clone(&spelling),
            Arc::clone(&multitoken),
        );
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("filters+rules");

        // `DutchHybridDisambiguator`: XML rules (+ global rules)
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::Nl),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters);
        mark("disambiguator");

        let global_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("core/spelling_global.txt"),
            // Java: MultiWordChunker.getInstance("/spelling_global.txt",
            // false, true, false, tagForNotAddingTags) + setIgnoreSpelling(true)
            true,
            false,
            true,
            Some(lt_disambig::multiword::TAG_FOR_NOT_ADDING_TAGS.to_string()),
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));
        let multiwords_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("nl/words/multiwords.txt"),
            // Java: MultiWordChunker.getInstance("/nl/multiwords.txt", true,
            // true, false, tagForNotAddingTags) + setIgnoreSpelling(true)
            true,
            true,
            true,
            Some(lt_disambig::multiword::TAG_FOR_NOT_ADDING_TAGS.to_string()),
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));
        mark("chunkers");

        let dutch = Arc::new(crate::nl::DutchPipeline {
            variant,
            tagger,
            synthesizer: synth,
            synth_adapter,
            global_chunker,
            multiwords_chunker,
            disambiguator,
            spelling,
            multitoken,
            compound_acceptor,
            compound: crate::compound::CompoundRule::dutch(data_dir.path())?,
            wrong_word_in_context: crate::wrong_word_in_context::WrongWordInContextRule::dutch(
                data_dir.path(),
            )?,
            word_coherency: crate::word_coherency::WordCoherencyRule::dutch(data_dir.path()),
            simple_replace: crate::nl::rules::simple_replace_instance(data_dir.path())?,
            check_case: crate::nl::rules::check_case_instance(data_dir.path())?,
            preferred_word: crate::nl::rules::PreferredWordRule::load(data_dir.path()),
            space_in_compound: crate::nl::rules::SpaceInCompoundRule::load(data_dir.path()),
        });
        Ok(Self {
            lang: Lang::Nl,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: Some(dutch),
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Catalan engine (`Catalan.getRelevantRules`, stage 1: XML rules, plain
    /// tagger/synthesizer and the hybrid chunker-disambiguator; the speller,
    /// tagger heuristics and Java rule classes follow in stages 2/3).
    /// `ValencianCatalan.getDefaultEnabledRulesForVariant` (2026-09-20).
    const VALENCIA_ENABLED_RULES: [&str; 8] = [
        "EXIGEIX_VERBS_VALENCIANS",
        "EXIGEIX_ACCENTUACIO_VALENCIANA",
        "EXIGEIX_POSSESSIUS_U",
        "EXIGEIX_VERBS_EIX",
        "EXIGEIX_VERBS_ISC",
        "PER_PER_A_INFINITIU",
        "FINS_EL_AVL",
        "LES_HA_FETES",
    ];

    /// `ValencianCatalan.getDefaultDisabledRulesForVariant`.
    const VALENCIA_DISABLED_RULES: [&str; 11] = [
        "EXIGEIX_VERBS_CENTRAL",
        "EXIGEIX_ACCENTUACIO_GENERAL",
        "EXIGEIX_POSSESSIUS_V",
        "EVITA_PRONOMS_VALENCIANS",
        "EVITA_DEMOSTRATIUS_EIXE",
        "VOCABULARI_VALENCIA",
        "EXIGEIX_US",
        "FINS_EL_GENERAL",
        "EVITA_INFINITIUS_INDRE",
        "EVITA_DEMOSTRATIUS_ESTE",
        "CASTIC_CASTIG",
    ];

    /// `BalearicCatalan.getDefaultEnabledRulesForVariant`.
    const BALEAR_ENABLED_RULES: [&str; 1] = ["EXIGEIX_VERBS_BALEARS"];

    /// `BalearicCatalan.getDefaultDisabledRulesForVariant`.
    const BALEAR_DISABLED_RULES: [&str; 2] =
        ["EXIGEIX_VERBS_CENTRAL", "CA_SIMPLE_REPLACE_BALEARIC"];

    pub fn new_catalan(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let timing = std::env::var("LT_TIMING").is_ok();
        // `Instant::now()` traps on wasm32-unknown-unknown; only arm the
        // timer when LT_TIMING requested it (native/WASI runs).
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] ca {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let variant = variant.unwrap_or("ca-ES").to_string();
        // `SRXSentenceTokenizer` always sets
        // `singleLineBreaksMarksParagraph(false)`, so the Catalan rule group
        // matches through the `ca_two` language code.
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "ca_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::CatalanTagger::load(
            data_dir.path(),
            variant.ends_with("valencia"),
        )?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::CatalanSynthesizer::from_data(
            data_dir.path(),
            &variant,
        )?);
        let synth_adapter = Arc::new(crate::ca::CatalanSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Ca))?;
        if data_dir.style_path(Lang::Ca).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Ca))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        // variant-specific rule files (`Language.getRuleFileNames` loads
        // ca/<variant>/grammar.xml after the common files; only
        // ca-ES-valencia ships one)
        for name in ["grammar.xml", "style.xml"] {
            let path = data_dir.path().join("ca/rules").join(&variant).join(name);
            if path.lt_exists() {
                let extra = Grammar::load_file(&path)?;
                grammar.rules.extend(extra.rules);
                grammar.categories.extend(extra.categories);
                grammar.equivalence_defs.extend(extra.equivalence_defs);
            }
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        mark("grammar+style+variant");

        // `MorfologikCatalanSpellerRule` (9) + `CatalanMultitokenSpeller`
        // (the `MultitokenSpellerFilter` over the same speller).
        let spelling = Arc::new(crate::ca::spelling::CatalanSpellingRule::load(
            data_dir.path(),
        )?);
        // `MorfologikCatalanSpellerRule`'s `INSTANCE_CAT`/`INSTANCE_VAL`
        // tagger for `orderSuggestions`/`getAdditionalTopSuggestions`.
        spelling.set_tagger(Arc::clone(&tagger));
        let multitoken = Arc::new(crate::multitoken::MultitokenSpeller::load_catalan(
            &[
                data_dir.path().join("ca/words/multiwords.txt"),
                data_dir.path().join("core/spelling_global.txt"),
                data_dir.path().join("ca/words/hyphenated_words.txt"),
            ],
            {
                let speller = Arc::clone(&spelling);
                Arc::new(move |word: &str| speller.is_misspelled(word))
            },
            {
                let mt = crate::ca::spelling::multitoken_speller(data_dir.path());
                Arc::new(move |word: &str| match &mt {
                    Some(speller) => speller.get_suggestions(word),
                    None => Vec::new(),
                })
            },
        ));
        mark("speller");

        let diacritics = Arc::new(crate::ca::diacritics::DiacriticsCheckFilter::load(
            data_dir.path(),
        ));
        let (filters, filter_env) = crate::ca::filters::catalan_filter_registry(
            today.unwrap_or_else(Ymd::today),
            &variant,
            Arc::clone(&tagger),
            Arc::new(crate::ca::helpers::VerbClassifier::load(data_dir.path())),
            Arc::clone(&synth_adapter),
            Arc::clone(&spelling),
            Arc::clone(&multitoken),
            diacritics,
        );
        // Legacy `AbstractSimpleReplaceRule` family (13–18, 22), built with
        // the shared filter environment for the post filters
        // (`ConvertToGenderAndNumberFilter`, `AdjustVerbSuggestionsFilter`).
        let legacy_replace = crate::ca::legacy_simple_replace::catalan_legacy_instances(
            data_dir.path(),
            Arc::clone(&filter_env),
        );
        let multiwords = crate::ca::simple_replace::catalan_multiwords_instance(data_dir.path())?;
        let anglicism = crate::ca::simple_replace::catalan_anglicism_instance(data_dir.path())?;
        let check_case = crate::ca::simple_replace::catalan_check_case_instance(data_dir.path())?;
        let dnv_replace =
            crate::ca::dnv_replace::catalan_dnv_instances(data_dir.path(), Arc::clone(&synth));
        // `ca.CompoundRule.isMisspelled` uses `CatalanTagger.INSTANCE_VAL`
        // (the valencia tagger) regardless of the requested variant.
        let compound = crate::compound::CompoundRule::catalan(
            data_dir.path(),
            Arc::new(lt_tagger::CatalanTagger::load(data_dir.path(), true)?),
        )?;
        // `ValencianCatalan`/`BalearicCatalan.getDefaultEnabledRulesForVariant`
        // + `getDefaultDisabledRulesForVariant` (D-154): the variant-enabled
        // XML rules (mostly `default="off"`) are compiled and their
        // `category_default_on` is patched, so the variant defaults apply
        // before the user's explicit enable/disable sets (which still win,
        // like Java's `RuleSet`). `CA_SIMPLE_REPLACE_BALEARIC` is a built-in
        // rule and is handled where the Catalan built-ins are wired.
        let (variant_enabled, variant_disabled): (&[&str], &[&str]) = match variant.as_str() {
            "ca-ES-valencia" => (
                &Self::VALENCIA_ENABLED_RULES,
                &Self::VALENCIA_DISABLED_RULES,
            ),
            "ca-ES-balear" => (&Self::BALEAR_ENABLED_RULES, &Self::BALEAR_DISABLED_RULES),
            _ => (&[], &[]),
        };
        let mut compile_enabled: Vec<String> = enabled_rules.to_vec();
        for id in variant_enabled {
            if !compile_enabled.iter().any(|e| e == id) {
                compile_enabled.push((*id).to_string());
            }
        }
        let (mut compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, &compile_enabled);
        for rule in &mut compiled_rules {
            let id = rule.rule_id.clone();
            let on = if variant_enabled.contains(&id.as_str()) {
                Some(true)
            } else if variant_disabled.contains(&id.as_str()) {
                Some(false)
            } else {
                None
            };
            if let Some(on) = on {
                if let Some(rule) = Arc::get_mut(rule) {
                    rule.category_default_on = on;
                }
            }
        }
        mark("filters+rules");
        // `CatalanHybridDisambiguator`: XML rules (+ global rules)
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::Ca),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters);
        mark("disambiguator");

        let global_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("core/spelling_global.txt"),
            // Java: MultiWordChunker.getInstance("/spelling_global.txt",
            // false, true, false, "NPCN000")
            false,
            false,
            true,
            Some("NPCN000".to_string()),
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));
        let multiwords_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("ca/words/multiwords.txt"),
            // Java: MultiWordChunker.getInstance("/ca/multiwords.txt", true,
            // true, false) + setRemovePreviousTags(true)
            false,
            true,
            true,
            None,
            true,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));
        mark("chunkers");

        let word_coherency =
            crate::word_coherency::WordCoherencyRule::catalan(data_dir.path(), Arc::clone(&synth));
        let word_coherency_valencia = if variant == "ca-ES-valencia" {
            Some(crate::word_coherency::WordCoherencyRule::valencian(
                data_dir.path(),
                Arc::clone(&synth),
            ))
        } else {
            None
        };
        let wrong_word_in_context =
            crate::wrong_word_in_context::WrongWordInContextRule::catalan(data_dir.path())?;
        let catalan = Arc::new(crate::ca::CatalanPipeline {
            variant,
            tagger,
            synthesizer: synth,
            synth_adapter,
            global_chunker,
            multiwords_chunker,
            disambiguator,
            spelling,
            multitoken,
            multitoken_dict_speller: crate::ca::spelling::multitoken_speller(data_dir.path())
                .map(Arc::new),
            word_coherency,
            word_coherency_valencia,
            wrong_word_in_context,
            legacy_replace,
            multiwords,
            anglicism,
            check_case,
            dnv_replace,
            compound,
            filter_env: Arc::clone(&filter_env),
        });
        // stage-3 filters that re-analyze suggestion strings
        // (`createDefaultJLanguageTool().analyzeText(...)`) need the assembled
        // pipeline; keep it weak so the Arc cycle is broken.
        let _ = filter_env.pipeline.set(Arc::downgrade(&catalan));
        Ok(Self {
            lang: Lang::Ca,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: Some(catalan),
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Galician engine (`Galician.getRelevantRules`, stage 1: XML rules, the
    /// `GalicianTagger`/`GalicianSynthesizer` and the
    /// `GalicianHybridDisambiguator`; the hunspell speller and the Java rule
    /// classes follow in stages 2/3).
    pub fn new_galician(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let _ = today;
        let _ = variant;
        let timing = std::env::var("LT_TIMING").is_ok();
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] gl {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "gl_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::GalicianTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::GalicianSynthesizer::from_data(data_dir.path())?);
        let synth_adapter = Arc::new(crate::gl::GalicianSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let grammar = Grammar::load_file(data_dir.grammar_path(Lang::Gl))?;
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        mark("grammar");

        // XML-referenced filter classes (`AdvancedSynthesizerFilter`).
        let filters = crate::gl::filters::galician_filter_registry(Arc::clone(&synth_adapter));
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("rules");

        // `GalicianHybridDisambiguator`: `gl/multiwords.txt` chunker → XML
        // rules (+ global rules).
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::Gl),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters);
        mark("disambiguator");

        let multiwords_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("gl/words/multiwords.txt"),
            // Java: MultiWordChunker.getInstance("/gl/multiwords.txt")
            false,
            false,
            false,
            None,
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));
        mark("chunker");

        // `HunspellRule` (4), default on. The vendored `gl_ES` dictionary uses
        // the `FLAG num` flag mode (now supported by `lt-spell`); a load
        // failure would only disable the speller, not the engine.
        let spelling = match crate::gl::spelling::GalicianSpellingRule::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[gl] spelling rule disabled: {err}");
                None
            }
        };
        mark("speller");

        // Stage-3 built-in replace family (15–20).
        let legacy_replace =
            crate::gl::rules::legacy_replace_instances(data_dir.path(), Arc::clone(&synth));
        let rule2 = crate::gl::rules::rule2_instances(data_dir.path())?;

        let galician = Arc::new(crate::gl::GalicianPipeline {
            tagger,
            synthesizer: synth,
            synth_adapter,
            multiwords_chunker,
            disambiguator,
            spelling,
            legacy_replace,
            rule2,
        });
        Ok(Self {
            lang: Lang::Gl,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: Some(galician),
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Romanian (`ro`) engine. Stage 1 wires the XML rules with the plain
    /// `RomanianTagger`/`RomanianSynthesizer` and the `XmlRuleDisambiguator`
    /// order; the Morfologik speller and the Java rule classes follow in
    /// stages 2/3.
    pub fn new_romanian(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let _ = today;
        let _ = variant;
        let timing = std::env::var("LT_TIMING").is_ok();
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] ro {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "ro_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::RomanianTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::RomanianSynthesizer::from_data(data_dir.path())?);
        let synth_adapter = Arc::new(crate::ro::RomanianSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let grammar = Grammar::load_file(data_dir.grammar_path(Lang::Ro))?;
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        mark("grammar");

        // Romanian references no `<filter>` classes from its rule XML.
        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("rules");

        // `Romanian.createDefaultDisambiguator` = plain `XmlRuleDisambiguator`
        // (+ global rules).
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::Ro),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters);
        mark("disambiguator");

        // `MorfologikRomanianSpellerRule` (7), default on.
        let spelling = match crate::ro::spelling::RomanianSpellingRule::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[ro] spelling rule disabled: {err}");
                None
            }
        };
        mark("speller");

        let word_repeat = crate::ro::rules::WordRepeatSentenceRule::new();
        let simple_replace = crate::ro::rules::simple_replace_instance(data_dir.path())?;
        let compound = crate::compound::CompoundRule::romanian(data_dir.path())?;
        mark("rules-java");

        let romanian = Arc::new(crate::ro::RomanianPipeline {
            tagger,
            synthesizer: synth,
            synth_adapter,
            disambiguator,
            spelling,
            word_repeat,
            simple_replace,
            compound,
        });
        Ok(Self {
            lang: Lang::Ro,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: Some(romanian),
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }
    /// Polish (`pl`): the PoliMorf `PolishTagger`/`PolishSynthesizer`, the
    /// `pl_two` SRX, the `PolishHybridDisambiguator` order (XML rules →
    /// `pl/multiwords.txt` chunker) and the four XML-referenced filters.
    /// Stage 1 wires the XML rules and the generic core built-ins; the
    /// morfologik speller and the Java rule classes follow in stages 2/3.
    pub fn new_polish(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let _ = variant;
        let timing = std::env::var("LT_TIMING").is_ok();
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] pl {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "pl_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::PolishTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::PolishSynthesizer::from_data(data_dir.path())?);
        let synth_adapter = Arc::new(crate::pl::PolishSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let grammar = Grammar::load_file(data_dir.grammar_path(Lang::Pl))?;
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        mark("grammar");

        // XML-referenced filter classes: `DateCheckFilter`,
        // `DecadeSpellingFilter`, `DateRangeChecker`,
        // `ShortenedYearRangeChecker`.
        let filters = crate::pl::filters::polish_filter_registry(today.unwrap_or_else(Ymd::today));
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("rules");

        // `PolishHybridDisambiguator`: XML rules (+ global rules), then the
        // `pl/multiwords.txt` chunker.
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::Pl),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters);
        mark("disambiguator");

        let multiwords_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("pl/words/multiwords.txt"),
            // Java: MultiWordChunker.getInstance("/pl/multiwords.txt")
            false,
            false,
            false,
            None,
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));
        mark("chunker");

        // `MorfologikPolishSpellerRule` (7), default on. A load failure would
        // only disable the speller, not the engine.
        let spelling = match crate::pl::spelling::PolishSpellingRule::load(
            data_dir.path(),
            Arc::clone(&tagger),
        ) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[pl] spelling rule disabled: {err}");
                None
            }
        };
        mark("speller");

        // Stage-3 Java rule classes (`Polish.getRelevantRules` 8–12).
        let polish_word_repeat = crate::pl::rules::PolishWordRepeatRule::new();
        let simple_replace = crate::pl::rules::PolishSimpleReplaceRule::load(data_dir.path())?;
        let compound = crate::compound::CompoundRule::polish(data_dir.path())?;
        let word_coherency = crate::word_coherency::WordCoherencyRule::polish(data_dir.path());
        let dash = crate::dash::polish(data_dir.path())?;
        mark("rules-java");

        let polish = Arc::new(crate::pl::PolishPipeline {
            tagger,
            synthesizer: synth,
            synth_adapter,
            multiwords_chunker,
            disambiguator,
            word_repeat: crate::pl::rules::WordRepeatSentenceRule::new(),
            spelling,
            polish_word_repeat,
            simple_replace,
            compound,
            word_coherency,
            dash,
        });
        Ok(Self {
            lang: Lang::Pl,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: Some(polish),
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }
    /// Slovak (`sk`) engine: the XML rules with the
    /// `SlovakTagger`/`SlovakSynthesizer` and the `grammar-typography.xml`
    /// extra rule file, plus the `MorfologikSlovakSpellerRule` and
    /// `CompoundRule`.
    pub fn new_slovak(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let _ = today;
        let _ = variant;
        let timing = std::env::var("LT_TIMING").is_ok();
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] sk {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "sk_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::SlovakTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::SlovakSynthesizer::from_data(data_dir.path())?);
        let synth_adapter = Arc::new(crate::sk::SlovakSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        // `Language.getRuleFileNames`: grammar.xml (+ style.xml) then the
        // `Slovak.RULE_FILES` extra file `grammar-typography.xml`.
        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Sk))?;
        if data_dir.style_path(Lang::Sk).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Sk))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let typography = data_dir.path().join("sk/rules/grammar-typography.xml");
        if typography.lt_exists() {
            let extra = Grammar::load_file(&typography)?;
            grammar.rules.extend(extra.rules);
            grammar.categories.extend(extra.categories);
            grammar.equivalence_defs.extend(extra.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        mark("grammar");

        // Slovak references no `<filter>` classes from its rule XML.
        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("rules");

        // `MorfologikSlovakSpellerRule` (8), default on. A load failure would
        // only disable the speller, not the engine.
        let spelling = match crate::sk::spelling::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[sk] spelling rule disabled: {err}");
                None
            }
        };
        mark("speller");

        let compound = crate::compound::CompoundRule::slovak(data_dir.path())?;
        let word_repeat = crate::sk::rules::word_repeat_rule();
        mark("rules-java");

        let slovak = Arc::new(crate::sk::SlovakPipeline {
            tagger,
            synthesizer: synth,
            synth_adapter,
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            spelling,
            word_repeat,
            compound,
        });
        Ok(Self {
            lang: Lang::Sk,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: Some(slovak),
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Slovenian (`sl`) engine: the XML rules over the surface tokenization
    /// (`Slovenian` has no tagger/synthesizer/disambiguator) plus the
    /// `MorfologikSlovenianSpellerRule` and the generic `WordRepeatRule`.
    pub fn new_slovenian(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let _ = today;
        let _ = variant;
        let timing = std::env::var("LT_TIMING").is_ok();
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] sl {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "sl_two")?;
        mark("srx");

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Sl))?;
        if data_dir.style_path(Lang::Sl).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Sl))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        mark("grammar");

        // Slovenian references no `<filter>` classes from its rule XML.
        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("rules");

        // `MorfologikSlovenianSpellerRule` (4), default on.
        let spelling = match crate::sl::spelling::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[sl] spelling rule disabled: {err}");
                None
            }
        };
        mark("speller");

        let slovenian = Arc::new(crate::sl::SlovenianPipeline {
            spelling,
            word_repeat: crate::sl::rules::word_repeat_rule(),
        });
        Ok(Self {
            lang: Lang::Sl,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: Some(slovenian),
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }
    /// Greek (`el`) engine: the `GreekWordTokenizer`/`GreekTagger` (with the
    /// `morphology-el` analyzer), the `GreekSynthesizer`, the
    /// `el/disambiguation.xml` XML disambiguator and the generic core rules.
    /// The Morfologik speller and the Greek-only rule classes follow in
    /// stages 2/3.
    pub fn new_greek(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let _ = today;
        let _ = variant;
        let timing = std::env::var("LT_TIMING").is_ok();
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] el {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "el_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::GreekTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::GreekSynthesizer::from_data(data_dir.path())?);
        let synth_adapter = Arc::new(crate::el::GreekSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::El))?;
        if data_dir.style_path(Lang::El).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::El))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        mark("grammar");

        // Greek references no `<filter>` classes from its rule XML.
        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("rules");

        // `Greek.createDefaultDisambiguator` = plain `XmlRuleDisambiguator`
        // (XML rules + `core/disambiguation-global.xml`).
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::El),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters);
        mark("disambiguator");

        // `MorfologikGreekSpellerRule` (5), default on. A load failure would
        // only disable the speller, not the engine.
        let spelling = match crate::el::spelling::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[el] spelling rule disabled: {err}");
                None
            }
        };
        mark("speller");

        // Stage-3 Greek rule classes (`Greek.getRelevantRules` 8, 10–13).
        let homonyms = crate::el::rules::homonyms_instance(data_dir.path())?;
        let specific_case = crate::el::rules::specific_case_instance(data_dir.path());
        let redundancy = crate::el::rules::redundancy_instance(data_dir.path())?;
        mark("rules-java");

        let greek = Arc::new(crate::el::GreekPipeline {
            tagger,
            synthesizer: synth,
            synth_adapter,
            disambiguator,
            spelling,
            word_repeat: crate::el::rules::word_repeat_rule(),
            homonyms,
            specific_case,
            redundancy,
        });
        Ok(Self {
            lang: Lang::El,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: Some(greek),
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }
    /// Danish (`da`) engine: the `DanishTagger` (`BaseTagger` over
    /// `danish.dict` + the manual word lists), the `da_two` SRX, the plain
    /// `XmlRuleDisambiguator` (`da/disambiguation.xml` + global rules) and the
    /// hunspell `HunspellRule` speller. `Danish.getRelevantRules` adds no Java
    /// rule classes beyond the generic core built-ins.
    pub fn new_danish(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let f = Self::hand_authored_foundations(data_dir, Lang::Da, "da_two", enabled_rules)?;
        let tagger = Arc::new(lt_tagger::DanishTagger::load(data_dir.path())?);
        let spelling = match crate::da::spelling::DanishSpellingRule::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[da] spelling rule disabled: {err}");
                None
            }
        };
        let danish = Arc::new(crate::da::DanishPipeline {
            tagger,
            disambiguator: f.disambiguator,
            spelling,
        });
        Ok(Self {
            lang: Lang::Da,
            unify_config: f.unify_config,
            srx: f.srx,
            tagger: None,
            grammar: f.grammar,
            compiled_rules: f.compiled_rules,
            skipped_counts: f.skipped,
            compile_failures: f.compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            da: Some(danish),
            sv: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            clean_overlapping_matches: true,
        })
    }

    /// Swedish (`sv`) engine: the `SwedishTagger`/`SwedishSynthesizer` (plain
    /// `BaseTagger`/`BaseSynthesizer` over the vendored dictionaries), the
    /// `SwedishHybridDisambiguator` order (XML rules → `sv/multiwords.txt`
    /// chunker) and the Swedish built-in rules (`WordRepeatRule`,
    /// `WordCoherencyRule`, `CompoundRule`, the hunspell speller and the generic
    /// text-level rules).
    pub fn new_swedish(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        variant: Option<&str>,
    ) -> Result<Self> {
        let _ = today;
        let _ = variant;
        let timing = std::env::var("LT_TIMING").is_ok();
        let mut last = timing.then(std::time::Instant::now);
        let mut mark = |name: &str| {
            if let Some(previous) = last {
                let now = std::time::Instant::now();
                eprintln!("[timing] sv {name}: {:?}", now - previous);
                last = Some(now);
            }
        };
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "sv_two")?;
        mark("srx");

        let tagger = Arc::new(lt_tagger::SwedishTagger::load(data_dir.path())?);
        mark("tagger");
        let synth = Arc::new(lt_tagger::SwedishSynthesizer::from_data(data_dir.path())?);
        let synth_adapter = Arc::new(crate::sv::SwedishSynthesizerAdapter {
            synth: Arc::clone(&synth),
            tagger: Arc::clone(&tagger),
        });
        mark("synth");

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Sv))?;
        if data_dir.style_path(Lang::Sv).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Sv))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;
        mark("grammar");

        // Swedish references no `<filter>` classes from its rule XML.
        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);
        mark("rules");

        // `SwedishHybridDisambiguator`: XML rules (+ global rules) first, then
        // the `sv/multiwords.txt` chunker.
        let global_disambig = data_dir.path().join("core/disambiguation-global.xml");
        let mut disambiguator = lt_disambig::XmlDisambiguator::load_with_extra(
            &data_dir.disambiguation_path(Lang::Sv),
            Some(&global_disambig),
        )?;
        disambiguator.set_synthesizer(Arc::clone(&synth_adapter) as Arc<dyn pm::Synthesizer>);
        disambiguator.set_filter_registry(filters);
        let multiwords_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("sv/words/multiwords.txt"),
            false,
            false,
            false,
            None,
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));
        mark("disambiguator");

        // `HunspellRule` (4), default on.
        let spelling = match crate::sv::spelling::SwedishSpellingRule::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[sv] spelling rule disabled: {err}");
                None
            }
        };
        mark("speller");

        let word_repeat =
            crate::word_repeat::WordRepeatRule::new(crate::word_repeat::WordRepeatConfig {
                description: "Upprepning av ord (exempelvis 'till till')",
                message: "Möjligt korrekturfel: du upprepade ett ord",
                short_message: "Upprepning av ord",
                category_name: "Upprepningar",
            });
        let word_coherency = crate::word_coherency::WordCoherencyRule::swedish(data_dir.path());
        let compound = crate::compound::CompoundRule::swedish(data_dir.path())?;
        mark("rules-java");

        let swedish = Arc::new(crate::sv::SwedishPipeline {
            tagger,
            synthesizer: synth,
            synth_adapter,
            multiwords_chunker,
            disambiguator,
            spelling,
            word_repeat,
            word_coherency,
            compound,
        });
        Ok(Self {
            lang: Lang::Sv,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            da: None,
            sv: Some(swedish),
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            clean_overlapping_matches: true,
        })
    }

    /// Icelandic (`is`) engine: the XML rules over the surface tokenization
    /// (`Icelandic` has no tagger/synthesizer/disambiguator) plus the
    /// `HunspellNoSuggestionRule` speller and the generic `WordRepeatRule`.
    pub fn new_icelandic(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let f = Self::hand_authored_foundations(data_dir, Lang::Is, "is_two", enabled_rules)?;
        let spelling = match crate::is::spelling::IcelandicSpellingRule::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[is] spelling rule disabled: {err}");
                None
            }
        };
        let icelandic = Arc::new(crate::is::IcelandicPipeline {
            spelling,
            word_repeat: crate::is::word_repeat_rule(),
        });
        Ok(Self {
            lang: Lang::Is,
            unify_config: f.unify_config,
            srx: f.srx,
            tagger: None,
            grammar: f.grammar,
            compiled_rules: f.compiled_rules,
            skipped_counts: f.skipped,
            compile_failures: f.compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: Some(icelandic),
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            da: None,
            sv: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            clean_overlapping_matches: true,
        })
    }

    /// Esperanto (`eo`) engine: the `EsperantoTagger` + `EsperantoWordTokenizer`,
    /// the plain `XmlRuleDisambiguator` (`eo/disambiguation.xml` + global
    /// rules), the hunspell speller and the generic built-ins (including
    /// `SentenceWhitespaceRule`). `Esperanto.getRelevantRules` adds no Java
    /// rule classes beyond the generic built-ins; the XML `DateCheckFilter` is
    /// registered for the `DATO_TAGO` rule.
    pub fn new_esperanto(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "eo_two")?;

        let tagger = Arc::new(lt_tagger::EsperantoTagger::load(data_dir.path())?);

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Eo))?;
        if data_dir.style_path(Lang::Eo).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Eo))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // XML-referenced filter classes: `DateCheckFilter` (`DATO_TAGO`).
        let filters =
            crate::eo::filters::esperanto_filter_registry(today.unwrap_or_else(Ymd::today));
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);

        // `Esperanto.createDefaultDisambiguator` is `new XmlRuleDisambiguator(new
        // Esperanto())`, which does NOT load `disambiguation-global.xml`
        // (`useGlobalDisambiguation = false`).
        let mut disambiguator =
            lt_disambig::XmlDisambiguator::load(&data_dir.disambiguation_path(Lang::Eo))?;
        disambiguator.set_filter_registry(filters);

        let spelling = match crate::eo::spelling::EsperantoSpellingRule::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[eo] spelling rule disabled: {err}");
                None
            }
        };

        let esperanto = Arc::new(crate::eo::EsperantoPipeline {
            tagger,
            disambiguator,
            spelling,
            word_repeat: crate::eo::word_repeat_rule(),
        });
        Ok(Self {
            lang: Lang::Eo,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: Some(esperanto),
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            da: None,
            sv: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            clean_overlapping_matches: true,
        })
    }

    /// Asturian (`ast`) engine: the `AsturianTagger` (`BaseTagger` over
    /// `ast/dictionaries/asturian.dict` + the manual word lists), the
    /// `ast_two` SRX, the base no-op disambiguator and the
    /// `MorfologikAsturianSpellerRule`. `Asturian.getRelevantRules` adds no
    /// Java rule classes beyond the generic built-ins.
    pub fn new_asturian(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "ast_two")?;

        let tagger = Arc::new(lt_tagger::AsturianTagger::load(data_dir.path())?);

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Ast))?;
        if data_dir.style_path(Lang::Ast).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Ast))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // Asturian references no `<filter>` classes from its rule XML.
        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);

        let spelling = match crate::ast::spelling::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[ast] spelling rule disabled: {err}");
                None
            }
        };

        let asturian = Arc::new(crate::ast::AsturianPipeline { tagger, spelling });
        Ok(Self {
            lang: Lang::Ast,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: Some(asturian),
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            da: None,
            sv: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            clean_overlapping_matches: true,
        })
    }

    /// Breton (`br`) engine: the `BretonTagger` (FSA5 `breton.dict` + the
    /// manual word lists), the `BretonWordTokenizer`, the plain
    /// `XmlRuleDisambiguator` (`br/disambiguation.xml`, no global rules), the
    /// `MorfologikBretonSpellerRule` and the `TopoReplaceRule` (`BR_TOPO`).
    /// `Breton.getRelevantRules` adds those two Java rule classes plus the
    /// generic built-ins; the XML-referenced `DateCheckFilter` is registered.
    pub fn new_breton(
        data_dir: &lt_data::DataDir,
        today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "br_two")?;

        let tagger = Arc::new(lt_tagger::BretonTagger::load(data_dir.path())?);

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Br))?;
        if data_dir.style_path(Lang::Br).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Br))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // XML-referenced filter classes: `DateCheckFilter` (DEIZ_DEIZIAD).
        let filters = crate::br::filters::breton_filter_registry(today.unwrap_or_else(Ymd::today));
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);

        // `Breton.createDefaultDisambiguator` is `new XmlRuleDisambiguator(new
        // Breton())`, which does NOT load `disambiguation-global.xml`.
        let mut disambiguator =
            lt_disambig::XmlDisambiguator::load(&data_dir.disambiguation_path(Lang::Br))?;
        disambiguator.set_filter_registry(filters);

        let spelling = match crate::br::spelling::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[br] spelling rule disabled: {err}");
                None
            }
        };

        let topo = crate::br::topo::TopoReplaceRule::load(data_dir.path())?;

        let breton = Arc::new(crate::br::BretonPipeline {
            tagger,
            disambiguator,
            spelling,
            topo,
        });
        Ok(Self {
            lang: Lang::Br,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: Some(breton),
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            da: None,
            sv: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            clean_overlapping_matches: true,
        })
    }

    /// Tagalog (`tl`) engine: the `TagalogTagger` (`BaseTagger` over
    /// `tl/dictionaries/tagalog.dict` + the manual word lists), the
    /// `TagalogWordTokenizer` (base characters + `-`), the base no-op
    /// disambiguator and the `MorfologikTagalogSpellerRule`.
    /// `Tagalog.getRelevantRules` adds the speller plus the generic built-ins
    /// (including `GenericUnpairedBracketsRule`); the rule XML references no
    /// `<filter>` class.
    pub fn new_tagalog(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "tl_two")?;

        let tagger = Arc::new(lt_tagger::TagalogTagger::load(data_dir.path())?);

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Tl))?;
        if data_dir.style_path(Lang::Tl).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Tl))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // Tagalog references no `<filter>` classes from its rule XML.
        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);

        let spelling = match crate::tl::spelling::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[tl] spelling rule disabled: {err}");
                None
            }
        };

        let tagalog = Arc::new(crate::tl::TagalogPipeline { tagger, spelling });
        Ok(Self {
            lang: Lang::Tl,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: Some(tagalog),
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            da: None,
            sv: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            clean_overlapping_matches: true,
        })
    }

    /// Lithuanian (`lt`) engine: the `DemoTagger` (every token untagged, i.e.
    /// `surface_sentence`), the base no-op disambiguator and the
    /// `MorfologikLithuanianSpellerRule` (`MORFOLOGIK_RULE_LT_LT`) over the
    /// vendored third-party ispell-lt dictionary. `Lithuanian.getRelevantRules`
    /// adds the speller plus the generic built-ins (including
    /// `GenericUnpairedBracketsRule`). Upstream ships no `lt_LT` dictionary, so
    /// the owner asked us to vendor one ourselves; `lt` therefore stays on the
    /// tests-only gate (see `crate::lt`, `docs/differences.md` #12).
    pub fn new_lithuanian(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "lt_two")?;

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Lt))?;
        if data_dir.style_path(Lang::Lt).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Lt))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // Lithuanian references no `<filter>` classes from its rule XML.
        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);

        let spelling = match crate::lt::spelling::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                // The vendored `lt/hunspell/lt_LT` dictionary could not be
                // read; the legacy engine has no dictionary at all.
                eprintln!("[lt] spelling rule disabled: {err}");
                None
            }
        };

        let lithuanian = Arc::new(crate::lt::LithuanianPipeline { spelling });
        Ok(Self {
            lang: Lang::Lt,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: Some(lithuanian),
            crimean_tatar: None,
            greek: None,
            da: None,
            sv: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            clean_overlapping_matches: true,
        })
    }

    /// Crimean Tatar (`crh`) engine: the `CrimeanTatarTagger` +
    /// `CrimeanTatarWordTokenizer`, the `CrimeanTatarSynthesizer`, the base
    /// no-op disambiguator and the `MorfologikCrimeanTatarSpellerRule`.
    /// `CrimeanTatar.getRelevantRules` adds the speller plus the generic
    /// built-ins (incl. the two paragraph-whitespace rules); the module has no
    /// `MessagesBundle_crh`, so the core English strings apply. The rule XML
    /// references no `<filter>` class.
    pub fn new_crimean_tatar(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "crh_two")?;

        let tagger = Arc::new(lt_tagger::CrimeanTatarTagger::load(data_dir.path())?);
        let synthesizer = Arc::new(lt_tagger::CrimeanTatarSynthesizer::from_data(
            data_dir.path(),
        )?);
        let synth_adapter = Arc::new(crate::crh::CrimeanTatarSynthesizerAdapter {
            synth: Arc::clone(&synthesizer),
            tagger: Arc::clone(&tagger),
        });

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Crh))?;
        if data_dir.style_path(Lang::Crh).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Crh))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);

        let spelling = match crate::crh::spelling::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[crh] spelling rule disabled: {err}");
                None
            }
        };

        let crimean_tatar = Arc::new(crate::crh::CrimeanTatarPipeline {
            tagger,
            synthesizer,
            synth_adapter,
            spelling,
        });
        Ok(Self {
            lang: Lang::Crh,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: Some(crimean_tatar),
            greek: None,
            da: None,
            sv: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            clean_overlapping_matches: true,
        })
    }

    /// Belarusian (`be`) engine: the `DemoTagger` (every token untagged, i.e.
    /// `surface_sentence`), the `BelarusianWordTokenizer` (apostrophes stay
    /// inside the word), the base no-op disambiguator, the
    /// `MorfologikBelarusianSpellerRule` (`MORFOLOGIK_RULE_BE_BY`) and the two
    /// language rule classes `BE_SIMPLE_REPLACE` and `BE_SPECIFIC_CASE`.
    /// `Belarusian.getRelevantRules` adds the generic built-ins (with the
    /// `MessagesBundle_be` strings) and the paragraph/long-sentence text-level
    /// rules; the module has **no** `GenericUnpairedBracketsRule`, and the rule
    /// XML references no `<filter>` class.
    pub fn new_belarusian(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "be_two")?;

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Be))?;
        if data_dir.style_path(Lang::Be).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Be))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // Belarusian references no `<filter>` classes from its rule XML.
        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);

        let spelling = match crate::be::spelling::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[be] spelling rule disabled: {err}");
                None
            }
        };

        let belarusian = Arc::new(crate::be::BelarusianPipeline { spelling });
        Ok(Self {
            lang: Lang::Be,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: vec![crate::be::rules::simple_replace_instance(data_dir.path())?],
            word_coherency: None,
            specific_case: Some(crate::be::rules::specific_case_instance(data_dir.path())),
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            da: None,
            sv: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: Some(belarusian),
            russian: None,
            clean_overlapping_matches: true,
        })
    }

    /// Russian (`ru`) engine. Stage 1 wires the XML rules with the
    /// `RussianTagger` + `RussianWordTokenizer`, the
    /// `RussianHybridDisambiguator` order (`ru/multiwords.txt` chunker → XML
    /// disambiguation) and the post-disambiguation `RussianChunker`; the
    /// speller and the language's Java rule classes follow in stages 2/3.
    pub fn new_russian(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, "ru_two")?;

        let tagger = Arc::new(lt_tagger::RussianTagger::load(data_dir.path())?);

        let mut grammar = Grammar::load_file(data_dir.grammar_path(Lang::Ru))?;
        if data_dir.style_path(Lang::Ru).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(Lang::Ru))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        // Russian references `<filter>` classes (DateCheckFilter,
        // FutureDateFilter, INNNumberFilter, AdvancedSynthesizerFilter,
        // RussianPartialPosTagFilter,
        // RussianSuppressMisspelledSuggestionsFilter) that are wired in
        // stage 3.
        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);

        let multiwords_chunker = lt_disambig::MultiWordChunker::load(
            &data_dir.path().join("ru/words/multiwords.txt"),
            // Java: MultiWordChunker.getInstance("/ru/multiwords.txt")
            false,
            false,
            false,
            None,
            false,
        )
        .unwrap_or_else(|_| lt_disambig::MultiWordChunker::load_empty(false, false));

        // `new XmlRuleDisambiguator(Russian.getInstance())` uses the default
        // `useGlobalDisambiguation = false`.
        let mut disambiguator =
            lt_disambig::XmlDisambiguator::load(&data_dir.disambiguation_path(Lang::Ru))?;
        disambiguator.set_filter_registry(filters);

        let post_chunker = lt_chunk::RussianChunker::new()?;

        let spelling = match crate::ru::spelling::load(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[ru] spelling rule disabled: {err}");
                None
            }
        };
        let spelling_yo = match crate::ru::spelling::load_yo(data_dir.path()) {
            Ok(rule) => Some(Arc::new(rule)),
            Err(err) => {
                eprintln!("[ru] yo spelling rule disabled: {err}");
                None
            }
        };

        let russian = Arc::new(crate::ru::RussianPipeline {
            tagger,
            multiwords_chunker,
            disambiguator,
            post_chunker,
            spelling,
            spelling_yo,
        });
        Ok(Self {
            lang: Lang::Ru,
            unify_config,
            srx,
            tagger: None,
            grammar,
            compiled_rules,
            skipped_counts: skipped,
            compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace: Vec::new(),
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            da: None,
            sv: None,
            norwegian: None,
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: Some(russian),
            clean_overlapping_matches: true,
        })
    }

    /// Shared loading for the hand-authored languages (Norwegian Bokmål,
    /// Nordum, Guaraní): SRX, grammar.xml (+ optional style.xml), compiled
    /// rules and the XML disambiguator (+ global rules when the language
    /// file exists).
    fn hand_authored_foundations(
        data_dir: &lt_data::DataDir,
        lang: Lang,
        srx_code: &str,
        enabled_rules: &[String],
    ) -> Result<HandAuthoredFoundations> {
        let srx_path = data_dir.path().join("core/segment.srx");
        if !srx_path.lt_exists() {
            return Err(CoreError::Data("missing core/segment.srx".into()));
        }
        let doc = lt_tokenize::SrxDocument::load_file(&srx_path)?;
        let srx = lt_tokenize::SrxTokenizer::new(&doc, srx_code)?;

        let mut grammar = Grammar::load_file(data_dir.grammar_path(lang))?;
        if data_dir.style_path(lang).lt_exists() {
            let style = Grammar::load_file(data_dir.style_path(lang))?;
            grammar.rules.extend(style.rules);
            grammar.categories.extend(style.categories);
            grammar.equivalence_defs.extend(style.equivalence_defs);
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?;

        let filters = lt_pattern::FilterRegistry::builder().build();
        let (compiled_rules, skipped, compile_failures) =
            compile_rules(&grammar, &filters, enabled_rules);

        let mut disambiguator = if data_dir.disambiguation_path(lang).lt_exists() {
            let global = data_dir.path().join("core/disambiguation-global.xml");
            lt_disambig::XmlDisambiguator::load_with_extra(
                &data_dir.disambiguation_path(lang),
                Some(&global),
            )?
        } else {
            lt_disambig::XmlDisambiguator::empty()?
        };
        disambiguator.set_filter_registry(filters);

        Ok(HandAuthoredFoundations {
            srx,
            grammar,
            unify_config,
            compiled_rules,
            skipped,
            compile_failures,
            disambiguator,
        })
    }

    /// Norwegian Bokmål (`no`) engine. Hand-authored language: XML rules,
    /// Hunspell speller and word-list rules; no legacy Java module
    /// exists (LT ships only a spell-check-only dynamic language).
    pub fn new_norwegian(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let f = Self::hand_authored_foundations(data_dir, Lang::No, "no_two", enabled_rules)?;
        let spelling = Arc::new(crate::no::spelling::NorwegianSpellingRule::load(
            data_dir.path(),
        )?);
        let simple_replace = crate::no::rules::norwegian_instances(data_dir.path())?;
        let repetition = crate::word_repetition::WordRepetitionRule::load(
            data_dir.path(),
            crate::word_repetition::WordRepetitionConfig {
                rule_id: "NB_WORD_REPETITION",
                description: "Ord gjentatt to ganger",
                message: "Mulig skrivefeil: ordet er gjentatt.",
                short_message: "Gjentakelse",
                category_id: "MISC",
                category_name: "Diverse",
                lang_dir: "no",
            },
        )?;
        let gender_overrides = crate::no::context::load_gender_overrides(data_dir.path())?;
        let norwegian = Arc::new(crate::no::NorwegianPipeline {
            disambiguator: f.disambiguator,
            spelling,
            repetition,
            gender_overrides,
        });
        Ok(Self {
            lang: Lang::No,
            unify_config: f.unify_config,
            srx: f.srx,
            tagger: None,
            grammar: f.grammar,
            compiled_rules: f.compiled_rules,
            skipped_counts: f.skipped,
            compile_failures: f.compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace,
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: Some(norwegian),
            nordum: None,
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Nordum (`nrd`) engine. Hand-authored constructed language
    /// (<https://www.nordum.org>): XML rules, a word-list Hunspell speller and
    /// word-list rules; no legacy Java module exists.
    pub fn new_nordum(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let f = Self::hand_authored_foundations(data_dir, Lang::Nrd, "nrd_two", enabled_rules)?;
        let spelling = Arc::new(crate::nrd::spelling::NordumSpellingRule::load(
            data_dir.path(),
        )?);
        let simple_replace = crate::nrd::rules::nordum_instances(data_dir.path())?;
        let repetition = crate::word_repetition::WordRepetitionRule::load(
            data_dir.path(),
            crate::word_repetition::WordRepetitionConfig {
                rule_id: "NDM_WORD_REPETITION",
                description: "Repeated word",
                message: "Possible typo: the same word is repeated.",
                short_message: "Repetition",
                category_id: "MISC",
                category_name: "Miscellaneous",
                lang_dir: "nrd",
            },
        )?;
        let nordum = Arc::new(crate::nrd::NordumPipeline {
            disambiguator: f.disambiguator,
            spelling,
            repetition,
        });
        Ok(Self {
            lang: Lang::Nrd,
            unify_config: f.unify_config,
            srx: f.srx,
            tagger: None,
            grammar: f.grammar,
            compiled_rules: f.compiled_rules,
            skipped_counts: f.skipped,
            compile_failures: f.compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace,
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: Some(nordum),
            guarani: None,
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Paraguayan Guaraní (`gn`/`gug`) engine. Hand-authored language: XML
    /// rules, the LibreOffice `gug` Hunspell speller (data only) and word-list
    /// rules; no legacy Java module exists.
    pub fn new_guarani(
        data_dir: &lt_data::DataDir,
        _today: Option<Ymd>,
        enabled_rules: &[String],
        _variant: Option<&str>,
    ) -> Result<Self> {
        let f = Self::hand_authored_foundations(data_dir, Lang::Gn, "gn_two", enabled_rules)?;
        let spelling = Arc::new(crate::gn::spelling::GuaraniSpellingRule::load(
            data_dir.path(),
        )?);
        let accents = crate::gn::accents::GuaraniAccentRule::load(data_dir.path())?;
        let harmony = crate::gn::context::GuaraniHarmonyRule::load(data_dir.path())?;
        let simple_replace = crate::gn::rules::guarani_instances(data_dir.path())?;
        let repetition = crate::word_repetition::WordRepetitionRule::load(
            data_dir.path(),
            crate::word_repetition::WordRepetitionConfig {
                rule_id: "GN_WORD_REPETITION",
                description: "Repeated word",
                message: "Possible typo: the same word is repeated.",
                short_message: "Repetition",
                category_id: "MISC",
                category_name: "Miscellaneous",
                lang_dir: "gn",
            },
        )?;
        let guarani = Arc::new(crate::gn::GuaraniPipeline {
            disambiguator: f.disambiguator,
            spelling,
            accents,
            harmony,
            repetition,
        });
        Ok(Self {
            lang: Lang::Gn,
            unify_config: f.unify_config,
            srx: f.srx,
            tagger: None,
            grammar: f.grammar,
            compiled_rules: f.compiled_rules,
            skipped_counts: f.skipped,
            compile_failures: f.compile_failures,
            global_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            multiword_chunker: lt_disambig::MultiWordChunker::load_empty(false, false),
            disambiguator: lt_disambig::XmlDisambiguator::empty()?,
            english_chunker: None,
            spelling: None,
            avs_an: None,
            compound: None,
            contractions: None,
            wrong_word_in_context: None,
            dash: None,
            synthesizer: None,
            simple_replace,
            word_coherency: None,
            specific_case: None,
            readability: Vec::new(),
            repeated_words: None,
            german: None,
            spanish: None,
            french: None,
            italian: None,
            portuguese: None,
            dutch: None,
            catalan: None,
            galician: None,
            romanian: None,
            polish: None,
            slovak: None,
            slovenian: None,
            icelandic: None,
            esperanto: None,
            asturian: None,
            breton: None,
            tagalog: None,
            lithuanian: None,
            crimean_tatar: None,
            greek: None,
            norwegian: None,
            nordum: None,
            guarani: Some(guarani),
            belarusian: None,
            russian: None,
            da: None,
            sv: None,
            clean_overlapping_matches: true,
        })
    }

    /// Analyze `text` into sentences with the token readings of the pipeline.
    /// `disambiguate` also applies the XML disambiguation and the chunker
    /// (the state pattern rules match on). Used by the Java oracle diff
    /// (`lt-cli analyze`).
    pub fn analyze(&self, text: &str, disambiguate: bool) -> Vec<AnalyzedSentence> {
        let mut out = Vec::new();
        for (start, end) in self.srx.split(text) {
            let sentence_text = &text[start..end];
            let mut analyzed = if self.norwegian.is_some() || self.nordum.is_some() {
                surface_sentence(sentence_text)
            } else if self.guarani.is_some() {
                crate::gn::analyze_guarani_sentence(sentence_text)
            } else if self.belarusian.is_some() {
                crate::be::analyze_belarusian_sentence(sentence_text)
            } else if let Some(russian) = &self.russian {
                crate::ru::analyze_russian_sentence(russian, sentence_text)
            } else {
                match &self.german {
                    Some(german) => {
                        crate::de::pipeline::analyze_german_sentence(german, sentence_text)
                    }
                    None => match &self.spanish {
                        Some(spanish) => {
                            crate::es::pipeline::analyze_spanish_sentence(spanish, sentence_text)
                        }
                        None => match &self.french {
                            Some(french) => {
                                crate::fr::pipeline::analyze_french_sentence(french, sentence_text)
                            }
                            None => match &self.italian {
                                Some(italian) => {
                                    crate::it::analyze_italian_sentence(italian, sentence_text)
                                }
                                None => match &self.portuguese {
                                    Some(portuguese) => crate::pt::analyze_portuguese_sentence(
                                        portuguese,
                                        sentence_text,
                                    ),
                                    None => match &self.dutch {
                                        Some(dutch) => {
                                            crate::nl::analyze_dutch_sentence(dutch, sentence_text)
                                        }
                                        None => match &self.catalan {
                                            Some(catalan) => crate::ca::analyze_catalan_sentence(
                                                catalan,
                                                sentence_text,
                                            ),
                                            None => match &self.romanian {
                                                Some(romanian) => {
                                                    crate::ro::analyze_romanian_sentence(
                                                        romanian,
                                                        sentence_text,
                                                    )
                                                }
                                                None => match &self.polish {
                                                    Some(polish) => {
                                                        crate::pl::analyze_polish_sentence(
                                                            polish,
                                                            sentence_text,
                                                        )
                                                    }
                                                    None => match &self.galician {
                                                        Some(galician) => {
                                                            crate::gl::analyze_galician_sentence(
                                                                galician,
                                                                sentence_text,
                                                            )
                                                        }
                                                        None => match &self.slovak {
                                                            Some(slovak) => {
                                                                crate::sk::analyze_slovak_sentence(
                                                                    slovak,
                                                                    sentence_text,
                                                                )
                                                            }
                                                            None => match &self.greek {
                                                                Some(greek) => {
                                                                    crate::el::analyze_greek_sentence(greek, sentence_text)
                                                                }
                                                                None => match &self.da {
                                                                    Some(danish) => {
                                                                        crate::da::analyze_danish_sentence(danish, sentence_text)
                                                                    }
                                                                    None => match &self.sv {
                                                                        Some(swedish) => {
                                                                            crate::sv::analyze_swedish_sentence(swedish, sentence_text)
                                                                        }
                                                                        None => match &self.slovenian {
                                                                            Some(_) => {
                                                                                surface_sentence(sentence_text)
                                                                            }
                                                                            None if self.lithuanian.is_some() => {
                                                                                surface_sentence(sentence_text)
                                                                            }
                                                                            None => match &self.icelandic {
                                                                                Some(_) => {
                                                                                    surface_sentence(sentence_text)
                                                                                }
                                                                                None => match &self.esperanto {
                                                                                    Some(esperanto) => crate::eo::analyze_esperanto_sentence(
                                                                                        esperanto,
                                                                                        sentence_text,
                                                                                    ),
                                                                                    None => match &self.asturian {
                                                                                        Some(asturian) => crate::ast::analyze_asturian_sentence(
                                                                                            asturian,
                                                                                            sentence_text,
                                                                                        ),
                                                                                        None => match &self.breton {
                                                                                            Some(breton) => crate::br::analyze_breton_sentence(
                                                                                                breton,
                                                                                                sentence_text,
                                                                                            ),
                                                                                            None => match &self.tagalog {
                                                                                                Some(tagalog) => crate::tl::analyze_tagalog_sentence(
                                                                                                    tagalog,
                                                                                                    sentence_text,
                                                                                                ),
                                                                                                None => match &self.crimean_tatar {
                                                                                                    Some(crh) => crate::crh::analyze_crimean_tatar_sentence(
                                                                                                        crh,
                                                                                                        sentence_text,
                                                                                                    ),
                                                                                                    None => analyze_sentence(
                                                                                                        self.tagger
                                                                                                            .as_deref()
                                                                                                            .expect("english tagger"),
                                                                                                        sentence_text,
                                                                                                    ),
                                                                                                },
                                                                                            },
                                                                                        },
                                                                                    },
                                                                                },
                                                                            },
                                                                        },
                                                                    },
                                                                },
                                                            },
                                                        },
                                                    },
                                                },
                                            },
                                        },
                                    },
                                },
                            },
                        },
                    },
                }
            };
            analyzed.offset = start;
            // LT `JLanguageTool.getRawAnalyzedSentence` runs the chunker on
            // the raw tokens; English has no post-disambiguation chunker, so
            // chunk tags are assigned before disambiguation. Running it after
            // would change the singular/plural NP decision, which reads the
            // token readings.
            if let Some(chunker) = &self.english_chunker {
                chunker.add_chunk_tags(&mut analyzed.tokens);
            }
            if disambiguate {
                self.apply_disambiguation(&mut analyzed, false);
                // German's post-disambiguation chunker (the raw path keeps
                // Java's `getChunker()` == null for German)
                if let Some(german) = &self.german {
                    german.chunker.add_chunk_tags(&mut analyzed.tokens);
                }
            }
            out.push(analyzed);
        }
        // Java `getAnalyzedSentence` (the oracle `Dump` path) carries no
        // paragraph-end marker; `analyzeText`/`check` add it to the last
        // sentence in `analyze_pipeline_sentence`.
        out
    }

    /// Run the full pipeline over `text` (UTF-8 byte offsets internally).
    /// The ids of the rules active for this check (`RuleSet.allRuleIds()` of
    /// `JLanguageTool.getActiveRulesForLevelAndToneTags`): mirrors the
    /// match-time gating in `check_range`/`match_compiled_rule`.
    #[allow(clippy::too_many_arguments)]
    fn active_rule_ids<'a>(
        compiled_rules: &'a [Arc<CompiledRule>],
        options: &crate::EngineOptions,
        enabled_rules: &HashSet<&str>,
        disabled_rules: &HashSet<&str>,
        disabled_categories: &HashSet<&str>,
        enabled_categories: &HashSet<&str>,
    ) -> HashSet<&'a str> {
        let mut ids: HashSet<&'a str> = HashSet::new();
        for rule in compiled_rules {
            let id = rule.rule_id.as_str();
            let explicitly_enabled = enabled_rules.contains(id);
            if !rule.category_default_on && !explicitly_enabled {
                continue;
            }
            if !options.picky && rule.tags.iter().any(|t| t == "picky") {
                continue;
            }
            if rule.goal_specific && !rule.tone_tags.is_empty() {
                continue;
            }
            if !enabled_rules.is_empty() && options.enabled_only {
                if !enabled_categories.is_empty() {
                    // With both an explicit rule list and a category list,
                    // `enabledOnly` keeps the union (Java `Tools.selectRules`,
                    // #12194/#aece4da), not the intersection.
                    let enabled_by_category =
                        enabled_categories.contains(rule.category_id.as_str());
                    if !explicitly_enabled && !enabled_by_category {
                        continue;
                    }
                } else if !explicitly_enabled {
                    continue;
                }
            } else {
                if disabled_rules.contains(id) {
                    continue;
                }
                if disabled_categories.contains(rule.category_id.as_str()) {
                    continue;
                }
                if options.enabled_only
                    && !enabled_categories.is_empty()
                    && !enabled_categories.contains(rule.category_id.as_str())
                {
                    continue;
                }
            }
            ids.insert(id);
        }
        ids
    }

    /// Run only the rules whose ids are in `rule_ids` on an already analyzed
    /// sentence and return their matches (`Rule.match(analyzedSentence)` for
    /// `SuppressIfAnyRuleMatchesFilter`). The level/picky gates are relaxed
    /// (Java's `r.match(...)` bypasses `isRuleActiveForLevelAndToneTags`);
    /// the returned ranges are relative to `sentence_text` plus `start`.
    pub fn match_rule_ids(
        &self,
        analyzed: &lt_core::AnalyzedSentence,
        sentence_text: &str,
        start: usize,
        rule_ids: &[String],
    ) -> Vec<lt_core::Match> {
        let enabled: HashSet<&str> = rule_ids.iter().map(|s| s.as_str()).collect();
        let empty: HashSet<&str> = HashSet::new();
        let options = crate::EngineOptions {
            enabled_rules: rule_ids.to_vec(),
            enabled_only: true,
            picky: true,
            ..Default::default()
        };
        self.check_analyzed_sentence(
            analyzed,
            sentence_text,
            start,
            0,
            0,
            &options,
            &enabled,
            &empty,
            &empty,
            &empty,
        )
        .0
    }

    pub fn check(&self, text: &str, options: &crate::EngineOptions) -> Result<CheckResult> {
        let disabled_rules: HashSet<&str> =
            options.disabled_rules.iter().map(|s| s.as_str()).collect();
        let enabled_rules: HashSet<&str> =
            options.enabled_rules.iter().map(|s| s.as_str()).collect();
        let disabled_categories: HashSet<&str> = options
            .disabled_categories
            .iter()
            .map(|s| s.as_str())
            .collect();
        let enabled_categories: HashSet<&str> = options
            .enabled_categories
            .iter()
            .map(|s| s.as_str())
            .collect();

        let spans = self.srx.split(text);
        // `seen`/`repeating` are applied in sentence order; the sequential
        // path is the single-pass loop (hot path for `--lines`), long texts
        // use the two-pass fan-out. Both produce identical results.
        let (sentences, analyzed_sentences, mut matches, repeating) =
            if spans.len() < PARALLEL_MIN_SENTENCES {
                self.check_sentences_sequential(
                    &spans,
                    text,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                )
            } else {
                self.check_sentences_parallel(
                    &spans,
                    text,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                )
            };

        // `TextLevelRule`s run before the sentence-level rules in Java
        // (`TextCheckCallable`, mode ALL). The ported text-level rules are
        // English-specific; German text-level rules are added in their port.
        let mut text_level_matches: Vec<Match> = Vec::new();
        if self.lang == crate::Lang::En {
            let synth: Option<&dyn pm::Synthesizer> = self.synthesizer();
            text_level_matches = crate::uppercase::check(&analyzed_sentences);
            text_level_matches.extend(crate::whitespace::check(&analyzed_sentences));
            text_level_matches.extend(crate::sentence_whitespace::check(&analyzed_sentences));
            // `WhiteSpaceBeforeParagraphEnd` (6), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::whitespace_before_paragraph_end(
                    &analyzed_sentences,
                ));
            }
            // `EmptyLineRule` (8), default off
            if builtin_active(
                crate::paragraph::EMPTY_LINE_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::empty_line(&analyzed_sentences));
            }
            // `TOO_LONG_SENTENCE` (9) is picky, `EN_CONSISTENT_APOS` (14) is
            // temp-off: both are skipped at the default level unless requested.
            if options.picky && !disabled_rules.contains("TOO_LONG_SENTENCE") {
                text_level_matches.extend(crate::long_sentence::check(&analyzed_sentences));
            }
            // `TOO_LONG_PARAGRAPH` (10), `tags="picky"` and default off
            if builtin_active(
                crate::paragraph::LONG_PARAGRAPH_ID,
                "STYLE",
                false,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::long_paragraph(&analyzed_sentences));
            }
            // `PARAGRAPH_REPEAT_BEGINNING_RULE` (11), default off
            if builtin_active(
                crate::paragraph::PARAGRAPH_REPEAT_BEGINNING_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::paragraph_repeat_beginning(
                    &analyzed_sentences,
                ));
            }
            // `PUNCTUATION_PARAGRAPH_END` (12) is picky, `…_END2` (13) default off
            if builtin_active(
                crate::paragraph::PUNCTUATION_PARAGRAPH_END_ID,
                "PUNCTUATION",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::punctuation_at_paragraph_end(
                    &analyzed_sentences,
                ));
            }
            if builtin_active(
                crate::paragraph::PUNCTUATION_PARAGRAPH_END2_ID,
                "PUNCTUATION",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::punctuation_at_paragraph_end2(
                    &analyzed_sentences,
                ));
            }
            if enabled_rules.contains("EN_CONSISTENT_APOS")
                && !disabled_rules.contains("EN_CONSISTENT_APOS")
            {
                text_level_matches.extend(crate::en::consistent_apostrophes::check(
                    &analyzed_sentences,
                ));
            }
            text_level_matches.extend(crate::unpaired_brackets::check(&analyzed_sentences));
            text_level_matches.extend(crate::unpaired_quotes::check(&analyzed_sentences));
            text_level_matches.extend(crate::en::word_repeat_beginning::check(&analyzed_sentences));
            if let Some(coherency) = &self.word_coherency {
                text_level_matches.extend(coherency.check(&analyzed_sentences));
            }
            // `READABILITY_RULE_DIFFICULT`/`_SIMPLE` (31/32), both default off
            for rule in &self.readability {
                if builtin_active(
                    rule.id(),
                    "TEXT_ANALYSIS",
                    false,
                    false,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(rule.check(&analyzed_sentences));
                }
            }
            // `EN_REPEATEDWORDS` (33), `tags="picky"`
            if let Some(repeated_words) = &self.repeated_words {
                if builtin_active(
                    "EN_REPEATEDWORDS",
                    "REPETITIONS_STYLE",
                    true,
                    true,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(repeated_words.check(&analyzed_sentences, synth));
                }
            }
            // `StyleTooOftenUsed*` (34-36; default off; enable by rule id)
            for kind in [
                crate::style_too_often::StyleKind::Verb,
                crate::style_too_often::StyleKind::Noun,
                crate::style_too_often::StyleKind::Adjective,
            ] {
                let id = kind.rule_id(crate::style_too_often::StyleLang::En);
                if enabled_rules.contains(id) && !disabled_rules.contains(id) {
                    text_level_matches
                        .extend(crate::style_too_often::check(&analyzed_sentences, kind));
                }
            }
        }
        if self.lang == crate::Lang::De {
            // German text-level Java rules in `German.getRelevantRules` order
            // (`DE_SENTENCE_WHITESPACE` (13), `DE_SIMILAR_NAMES` (26) before
            // `DE_DU_UPPER_LOWER` (39)).
            let para = crate::paragraph::strings_de();
            // `GermanUnpairedBracketsRule` (2) / `GermanUnpairedQuotesRule` (3),
            // both default on
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_de(&analyzed_sentences));
            }
            if builtin_active(
                "DE_UNPAIRED_QUOTES",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_quotes::check_de(&analyzed_sentences));
            }
            // `UPPERCASE_SENTENCE_START` (4), default on
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_de(&analyzed_sentences));
            }
            // `WHITESPACE_PARAGRAPH` (6), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::whitespace_before_paragraph_end_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            // `WHITESPACE_PARAGRAPH_BEGIN` (7), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_BEGIN_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                for sentence in &analyzed_sentences {
                    text_level_matches.extend(
                        crate::paragraph::whitespace_at_begin_of_paragraph_with(sentence, &para),
                    );
                }
            }
            // `EMPTY_LINE` (8), default off
            if builtin_active(
                crate::paragraph::EMPTY_LINE_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::empty_line_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            // `TOO_LONG_PARAGRAPH` (9), default off and picky
            if builtin_active(
                crate::paragraph::LONG_PARAGRAPH_ID,
                "STYLE",
                false,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::long_paragraph_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            // `PUNCTUATION_PARAGRAPH_END` (10), default on and picky
            if builtin_active(
                crate::paragraph::PUNCTUATION_PARAGRAPH_END_ID,
                "PUNCTUATION",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::punctuation_at_paragraph_end_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            if builtin_active(
                "DE_SENTENCE_WHITESPACE",
                "MISC",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::de::rules::sentence_whitespace(&analyzed_sentences));
            }
            // `GERMAN_WORD_REPEAT_BEGINNING_RULE` (17)
            if builtin_active(
                crate::de::repeat::WORD_REPEAT_BEGINNING_ID,
                "REPETITIONS_STYLE",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::de::repeat::word_repeat_beginning(
                    &analyzed_sentences,
                ));
            }
            // `DE_VERBAGREEMENT` (23), text level, default on
            if builtin_active(
                crate::de::verb_agreement::RULE_ID,
                "GRAMMAR",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                if let Some(german) = &self.german {
                    text_level_matches.extend(german.verb_agreement.check(&analyzed_sentences));
                }
            }
            // `DE_WORD_COHERENCY` (25), text level
            if builtin_active(
                "DE_WORD_COHERENCY",
                "MISC",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                if let Some(german) = &self.german {
                    text_level_matches.extend(german.word_coherency.check(&analyzed_sentences));
                }
            }
            if builtin_active(
                "DE_SIMILAR_NAMES",
                "TYPOS",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::de::rules::similar_names(&analyzed_sentences));
            }
            // `STYLE_REPEATED_WORD_RULE_DE` (28), default off
            if builtin_active(
                crate::de::style_repeated_word::RULE_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(
                    crate::de::style_repeated_word::style_repeated_word_rule_de(
                        &analyzed_sentences,
                    ),
                );
            }
            // `DE_COMPOUND_COHERENCY` (29), text level
            if builtin_active(
                "DE_COMPOUND_COHERENCY",
                "STYLE",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::de::style::compound_coherency(&analyzed_sentences));
            }
            // `TOO_LONG_SENTENCE_DE` (30), `tags="picky"`
            if builtin_active(
                "TOO_LONG_SENTENCE_DE",
                "STYLE",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::long_sentence::check_de(&analyzed_sentences));
            }
            // `PASSIVE_SENTENCE_DE` (32), `SENTENCE_WITH_MODAL_VERB_DE` (33),
            // `SENTENCE_WITH_MAN_DE` (34) and
            // `SENTENCE_BEGINNING_WITH_CONJUNCTION_DE` (35), default off
            for (id, check) in [
                (
                    "FILLER_WORDS_DE",
                    crate::de::statistic::filler_words
                        as fn(&[lt_core::AnalyzedSentence]) -> Vec<Match>,
                ),
                (
                    "NON_SIGNIFICANT_VERB_DE",
                    crate::de::statistic::non_significant_verbs,
                ),
                (
                    "UNNECESSARY_PHRASES_DE",
                    crate::de::statistic::unnecessary_phrases,
                ),
                (
                    "PASSIVE_SENTENCE_DE",
                    crate::de::statistic::passive_sentence
                        as fn(&[lt_core::AnalyzedSentence]) -> Vec<Match>,
                ),
                (
                    "SENTENCE_WITH_MODAL_VERB_DE",
                    crate::de::statistic::sentence_with_modal_verb,
                ),
                (
                    "SENTENCE_WITH_MAN_DE",
                    crate::de::statistic::sentence_with_man,
                ),
                (
                    "SENTENCE_BEGINNING_WITH_CONJUNCTION_DE",
                    crate::de::statistic::conjunction_at_begin_of_sentence,
                ),
            ] {
                if builtin_active(
                    id,
                    "CREATIVE_WRITING",
                    false,
                    false,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(check(&analyzed_sentences));
                }
            }
            // `GERMAN_PARAGRAPH_REPEAT_BEGINNING_RULE` (38), default off
            if builtin_active(
                "GERMAN_PARAGRAPH_REPEAT_BEGINNING_RULE",
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::paragraph_repeat_beginning_de(
                    &analyzed_sentences,
                ));
            }
            if builtin_active(
                "DE_DU_UPPER_LOWER",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::de::rules::du_upper_lower(&analyzed_sentences));
            }
            // `TOO_OFTEN_USED_{VERB,NOUN,ADJECTIVE}_DE` (48-50), default off
            for kind in [
                crate::style_too_often::StyleKind::Verb,
                crate::style_too_often::StyleKind::Noun,
                crate::style_too_often::StyleKind::Adjective,
            ] {
                let id = kind.rule_id(crate::style_too_often::StyleLang::De);
                if enabled_rules.contains(id) && !disabled_rules.contains(id) {
                    text_level_matches
                        .extend(crate::style_too_often::check_de(&analyzed_sentences, kind));
                }
            }
            // `STYLE_REPEATED_SHORT_SENTENCES` (45) and
            // `STYLE_REPEATED_SENTENCE_BEGINNING` (46), default off
            if builtin_active(
                "STYLE_REPEATED_SHORT_SENTENCES",
                "CREATIVE_WRITING",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(
                    crate::de::statistic::style_repeated_very_short_sentences(&analyzed_sentences),
                );
            }
            if builtin_active(
                "STYLE_REPEATED_SENTENCE_BEGINNING",
                "CREATIVE_WRITING",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::de::statistic::style_repeated_sentence_beginning(
                    &analyzed_sentences,
                ));
            }
            // `GermanReadabilityRule` (43/44), both default off
            if let Some(german) = &self.german {
                for rule in &german.readability {
                    if builtin_active(
                        rule.id(),
                        "CREATIVE_WRITING",
                        false,
                        false,
                        options,
                        &enabled_rules,
                        &disabled_rules,
                        &disabled_categories,
                        &enabled_categories,
                    ) {
                        text_level_matches.extend(rule.check(&analyzed_sentences));
                    }
                }
            }
            // `DE_REPEATEDWORDS` (47), default on (no `Tag.picky`)
            if let Some(german) = &self.german {
                if builtin_active(
                    "DE_REPEATEDWORDS",
                    "REPETITIONS_STYLE",
                    true,
                    false,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(german.repeated_words.check(
                        &analyzed_sentences,
                        Some(german.synth_adapter.as_ref() as &dyn pm::Synthesizer),
                    ));
                }
            }
        }
        if self.lang == crate::Lang::Es {
            let para = crate::paragraph::strings_es();
            // `SpanishUnpairedBracketsRule` (3), default on
            if builtin_active(
                "ES_UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_es(&analyzed_sentences));
            }
            // `ES_QUESTION_MARK` (4), text level, default on
            if builtin_active(
                crate::es::rules::QUESTION_MARK_RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::es::rules::question_mark(&analyzed_sentences));
            }
            // `UPPERCASE_SENTENCE_START` (6), default on
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_es(&analyzed_sentences));
            }
            // `WHITESPACE_RULE` (8), default on
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_es(&analyzed_sentences));
            }
            // `TOO_LONG_SENTENCE` (11), `tags="picky"` (60 words)
            if options.picky
                && !disabled_rules.contains("TOO_LONG_SENTENCE")
                && !disabled_categories.contains("STYLE")
            {
                text_level_matches.extend(crate::long_sentence::check_es(&analyzed_sentences));
            }
            // `ES_REPEATEDWORDS` (17), `tags="picky"`
            if let Some(spanish) = &self.spanish {
                if builtin_active(
                    "ES_REPEATEDWORDS",
                    "REPETITIONS_STYLE",
                    true,
                    true,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(spanish.repeated_words.check(
                        &analyzed_sentences,
                        Some(spanish.synth_adapter.as_ref() as &dyn pm::Synthesizer),
                    ));
                }
            }
            // `SPANISH_WORD_REPEAT_BEGINNING_RULE` (15), `tags="picky"`
            if options.picky
                && !disabled_rules.contains(crate::es::rules::WORD_REPEAT_BEGINNING_RULE_ID)
                && !disabled_categories.contains("REPETITIONS_STYLE")
            {
                text_level_matches
                    .extend(crate::es::rules::word_repeat_beginning(&analyzed_sentences));
            }
            // `TOO_LONG_PARAGRAPH` (12), default off and picky
            if builtin_active(
                crate::paragraph::LONG_PARAGRAPH_ID,
                "STYLE",
                false,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::long_paragraph_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
        }
        if self.lang == crate::Lang::Fr {
            let para = crate::paragraph::strings_fr();
            // `GenericUnpairedBracketsRule` (3), default on
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_fr(&analyzed_sentences));
            }
            // `UPPERCASE_SENTENCE_START` (5), default on
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_fr(&analyzed_sentences));
            }
            // `WHITESPACE_RULE` (6), default on
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_fr(&analyzed_sentences));
            }
            // `SENTENCE_WHITESPACE` (7), default on
            if builtin_active(
                "SENTENCE_WHITESPACE",
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::sentence_whitespace::check_fr(&analyzed_sentences));
            }
            // `TOO_LONG_SENTENCE` (8), `tags="picky"` (40 words)
            if options.picky
                && !disabled_rules.contains("TOO_LONG_SENTENCE")
                && !disabled_categories.contains("STYLE")
            {
                text_level_matches.extend(crate::long_sentence::check_fr(&analyzed_sentences));
            }
            // `TOO_LONG_PARAGRAPH` (9), default off and picky
            if builtin_active(
                crate::paragraph::LONG_PARAGRAPH_ID,
                "STYLE",
                false,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::long_paragraph_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            // `FR_REPEATEDWORDS` (14), default on
            if let Some(french) = &self.french {
                if builtin_active(
                    "FR_REPEATEDWORDS",
                    "REPETITIONS_STYLE",
                    true,
                    false,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(french.repeated_words.check(
                        &analyzed_sentences,
                        Some(french.synth_adapter.as_ref() as &dyn pm::Synthesizer),
                    ));
                }
            }
        }
        if self.lang == crate::Lang::It {
            // `GenericUnpairedBracketsRule` (4), default on (Italian symbols)
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_it(&analyzed_sentences));
            }
            // `UPPERCASE_SENTENCE_START` (6), default on
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_it(&analyzed_sentences));
            }
            // `WHITESPACE_RULE` (8), default on
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_it(&analyzed_sentences));
            }
        }
        if self.lang == crate::Lang::Pt {
            let pt_variant = self
                .portuguese
                .as_ref()
                .map(|p| p.variant.as_str())
                .unwrap_or("pt-PT");
            // `GenericUnpairedBracketsRule` (2), default on (Portuguese symbols)
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_pt(
                    &analyzed_sentences,
                    pt_variant,
                ));
            }
            // `LongSentenceRule` (4), `tags="picky"`, 50 words
            if builtin_active(
                "TOO_LONG_SENTENCE",
                "STYLE",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::long_sentence::check_pt(
                    &analyzed_sentences,
                    pt_variant,
                ));
            }
            // `LongParagraphRule` (5), picky and default off, 220 words
            if builtin_active(
                crate::paragraph::LONG_PARAGRAPH_ID,
                "STYLE",
                false,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::long_paragraph_with(
                    &analyzed_sentences,
                    &crate::paragraph::strings_pt(pt_variant),
                ));
            }
            // `UppercaseSentenceStartRule` (6), default on
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::uppercase::check_pt(&analyzed_sentences, pt_variant));
            }
            // `MultipleWhitespaceRule` (7), default on
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::whitespace::check_pt(&analyzed_sentences, pt_variant));
            }
            // `SentenceWhitespaceRule` (8), default on
            if builtin_active(
                "SENTENCE_WHITESPACE",
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::sentence_whitespace::check_pt(
                    &analyzed_sentences,
                    pt_variant,
                ));
            }
            let pt_para = crate::paragraph::strings_pt(pt_variant);
            // `WhiteSpaceBeforeParagraphEnd` (9), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::whitespace_before_paragraph_end_with(
                    &analyzed_sentences,
                    &pt_para,
                ));
            }
            // `WhiteSpaceAtBeginOfParagraph` (10), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_BEGIN_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                for sentence in &analyzed_sentences {
                    text_level_matches.extend(
                        crate::paragraph::whitespace_at_begin_of_paragraph_with(sentence, &pt_para),
                    );
                }
            }
            // `EmptyLineRule` (11), default off
            if builtin_active(
                crate::paragraph::EMPTY_LINE_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::empty_line_with(
                    &analyzed_sentences,
                    &pt_para,
                ));
            }
            // `ParagraphRepeatBeginningRule` (12), default off
            if builtin_active(
                crate::paragraph::PARAGRAPH_REPEAT_BEGINNING_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::paragraph_repeat_beginning_with(
                    &analyzed_sentences,
                    &pt_para,
                ));
            }
            // `PunctuationMarkAtParagraphEnd` (13), default on and picky
            if builtin_active(
                crate::paragraph::PUNCTUATION_PARAGRAPH_END_ID,
                "PUNCTUATION",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::punctuation_at_paragraph_end_with(
                    &analyzed_sentences,
                    &pt_para,
                ));
            }
            // `PortugueseWordRepeatBeginningRule` (29), text level, default on
            if builtin_active(
                crate::pt::rules::WORD_REPEAT_BEGINNING_ID,
                "REPETITIONS_STYLE",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::pt::rules::word_repeat_beginning(
                    &analyzed_sentences,
                    pt_variant,
                ));
            }
            // `PortugueseFillerWordsRule` (23), text level, default off
            if builtin_active(
                crate::pt::rules::FILLER_WORDS_ID,
                "CREATIVE_WRITING",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::pt::rules::filler_words(
                    &analyzed_sentences,
                    pt_variant,
                ));
            }
            // `PortugueseWordCoherencyRule` (33), text level, default on
            if let Some(pt) = &self.portuguese {
                if builtin_active(
                    "PT_WORD_COHERENCY",
                    "STYLE",
                    true,
                    false,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(pt.word_coherency.check(&analyzed_sentences));
                }
                // `READABILITY_RULE_DIFFICULT_PT`/`_SIMPLE_PT` (35/36), both
                // default off
                for rule in &pt.readability {
                    if builtin_active(
                        rule.id(),
                        "TEXT_ANALYSIS",
                        false,
                        false,
                        options,
                        &enabled_rules,
                        &disabled_rules,
                        &disabled_categories,
                        &enabled_categories,
                    ) {
                        text_level_matches.extend(rule.check(&analyzed_sentences));
                    }
                }
            }
        }
        // Dutch text-level rules (`Dutch.getRelevantRules`): GenericUnpairedBrackets (3),
        // UppercaseSentenceStart (4), MultipleWhitespace (6).
        if self.lang == crate::Lang::Nl {
            // `GenericUnpairedBracketsRule` (3), default on (Dutch symbols)
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_nl(&analyzed_sentences));
            }
            // `UPPERCASE_SENTENCE_START` (4), default on
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_nl(&analyzed_sentences));
            }
            // `WHITESPACE_RULE` (6), default on
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_nl(&analyzed_sentences));
            }
            // `WordCoherencyRule` (9), default on (text level)
            if let Some(dutch) = &self.dutch {
                if builtin_active(
                    "NL_WORD_COHERENCY",
                    "MISC",
                    true,
                    false,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(dutch.word_coherency.check(&analyzed_sentences));
                }
            }
            // `LongSentenceRule` (11), `tags="picky"`, 40 words
            if builtin_active(
                "TOO_LONG_SENTENCE",
                "STYLE",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::long_sentence::check_nl(&analyzed_sentences));
            }
            // `LongParagraphRule` (12), picky and default off
            if builtin_active(
                crate::paragraph::LONG_PARAGRAPH_ID,
                "STYLE",
                false,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::long_paragraph_with(
                    &analyzed_sentences,
                    &crate::paragraph::strings_nl(),
                ));
            }
            // `SentenceWhitespaceRule` (15), default on
            if builtin_active(
                "SENTENCE_WHITESPACE",
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::sentence_whitespace::check_nl(&analyzed_sentences));
            }
        }
        // Catalan text-level rules (`Catalan.getRelevantRules`):
        // CatalanUnpairedBrackets (3), UppercaseSentenceStart (4),
        // MultipleWhitespace (6).
        if self.lang == crate::Lang::Ca {
            // `CatalanUnpairedBracketsRule` (3), default on
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_ca(&analyzed_sentences));
            }
            // `UppercaseSentenceStartRule` (4), default on
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_ca(&analyzed_sentences));
            }
            // `WHITESPACE_RULE` (6), default on
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_ca(&analyzed_sentences));
            }
            // `SentenceWhitespaceRule` (6), default on
            if builtin_active(
                "SENTENCE_WHITESPACE",
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::sentence_whitespace::check_ca(&analyzed_sentences));
            }
            // `LongSentenceRule` (7), `tags="picky"`, 60 words
            if builtin_active(
                "TOO_LONG_SENTENCE",
                "STYLE",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::long_sentence::check_ca(&analyzed_sentences));
            }
            // `WordCoherencyRule` (29) + the valencia variant addition
            if let Some(catalan) = &self.catalan {
                if builtin_active(
                    catalan.word_coherency.rule_id(),
                    "STYLE",
                    true,
                    false,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(catalan.word_coherency.check(&analyzed_sentences));
                }
                if let Some(valencia) = &catalan.word_coherency_valencia {
                    if builtin_active(
                        valencia.rule_id(),
                        "STYLE",
                        true,
                        false,
                        options,
                        &enabled_rules,
                        &disabled_rules,
                        &disabled_categories,
                        &enabled_categories,
                    ) {
                        text_level_matches.extend(valencia.check(&analyzed_sentences));
                    }
                }
            }
            // `PunctuationMarkAtParagraphEnd` (30), `tags="picky"`, default on
            if builtin_active(
                crate::paragraph::PUNCTUATION_PARAGRAPH_END_ID,
                "PUNCTUATION",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                let ca_para = crate::paragraph::strings_ca();
                text_level_matches.extend(crate::paragraph::punctuation_at_paragraph_end_with(
                    &analyzed_sentences,
                    &ca_para,
                ));
            }
            // `IgnoreProperNouns` (33), default on
            if builtin_active(
                crate::ca::rules::IGNORE_PROPER_NOUNS_ID,
                "MISC",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::ca::rules::ignore_proper_nouns(&analyzed_sentences));
            }
            // `CatalanWordRepeatBeginningRule` (23), `tags="picky"`
            if builtin_active(
                crate::ca::rules::WORD_REPEAT_BEGINNING_ID,
                "REPETITIONS_STYLE",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::ca::rules::word_repeat_beginning(&analyzed_sentences));
            }
            // `CA_UNPAIRED_QUESTION` (10) / `CA_UNPAIRED_EXCLAMATION` (11),
            // both default off
            if builtin_active(
                "CA_UNPAIRED_QUESTION",
                "MISC",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::ca::rules::unpaired_marks(
                    &analyzed_sentences,
                    "CA_UNPAIRED_QUESTION",
                    "¿",
                    "?",
                ));
            }
            if builtin_active(
                "CA_UNPAIRED_EXCLAMATION",
                "MISC",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::ca::rules::unpaired_marks(
                    &analyzed_sentences,
                    "CA_UNPAIRED_EXCLAMATION",
                    "¡",
                    "!",
                ));
            }
        }
        // Galician text-level rules (`Galician.getRelevantRules`):
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (5),
        // MultipleWhitespace (6), SentenceWhitespace (9), LongSentence (7)
        // and the paragraph rules (8, 10–14).
        if self.lang == crate::Lang::Gl {
            // `GenericUnpairedBracketsRule` (3), default on
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_gl(&analyzed_sentences));
            }
            // `UppercaseSentenceStartRule` (5), default on
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_gl(&analyzed_sentences));
            }
            // `MultipleWhitespaceRule` (6), default on
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_gl(&analyzed_sentences));
            }
            // `SentenceWhitespaceRule` (9), default on
            if builtin_active(
                "SENTENCE_WHITESPACE",
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::sentence_whitespace::check_gl(&analyzed_sentences));
            }
            // `LongSentenceRule` (7), `tags="picky"`, 50 words
            if builtin_active(
                "TOO_LONG_SENTENCE",
                "STYLE",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::long_sentence::check_gl(&analyzed_sentences));
            }
            let gl_para = crate::paragraph::strings_gl();
            // `LongParagraphRule` (8), default off and picky
            if builtin_active(
                crate::paragraph::LONG_PARAGRAPH_ID,
                "STYLE",
                false,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::long_paragraph_with(
                    &analyzed_sentences,
                    &gl_para,
                ));
            }
            // `WhiteSpaceBeforeParagraphEnd` (10), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::whitespace_before_paragraph_end_with(
                    &analyzed_sentences,
                    &gl_para,
                ));
            }
            // `WhiteSpaceAtBeginOfParagraph` (11), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_BEGIN_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                for sentence in &analyzed_sentences {
                    text_level_matches.extend(
                        crate::paragraph::whitespace_at_begin_of_paragraph_with(sentence, &gl_para),
                    );
                }
            }
            // `EmptyLineRule` (12), default off
            if builtin_active(
                crate::paragraph::EMPTY_LINE_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::empty_line_with(
                    &analyzed_sentences,
                    &gl_para,
                ));
            }
            // `ParagraphRepeatBeginningRule` (13), default off
            if builtin_active(
                crate::paragraph::PARAGRAPH_REPEAT_BEGINNING_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::paragraph_repeat_beginning_with(
                    &analyzed_sentences,
                    &gl_para,
                ));
            }
            // `PunctuationMarkAtParagraphEnd` (14), default on and picky
            if builtin_active(
                crate::paragraph::PUNCTUATION_PARAGRAPH_END_ID,
                "PUNCTUATION",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::punctuation_at_paragraph_end_with(
                    &analyzed_sentences,
                    &gl_para,
                ));
            }
        }
        // Romanian text-level rules (`Romanian.getRelevantRules`):
        // UppercaseSentenceStart (3), GenericUnpairedBrackets (5) and
        // RomanianWordRepeatBeginning (8).
        if self.lang == crate::Lang::Ro {
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_ro(&analyzed_sentences));
            }
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_ro(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_ro(&analyzed_sentences));
            }
            if builtin_active(
                crate::ro::rules::WORD_REPEAT_BEGINNING_ID,
                "REPETITIONS_STYLE",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::ro::rules::word_repeat_beginning(&analyzed_sentences));
            }
        }
        // Polish text-level rules (`Polish.getRelevantRules`):
        // UppercaseSentenceStart (2), MultipleWhitespace (4),
        // SentenceWhitespace (5), PolishUnpairedBrackets (6) and
        // WordCoherencyRule (11).
        if self.lang == crate::Lang::Pl {
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_pl(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_pl(&analyzed_sentences));
            }
            if builtin_active(
                "SENTENCE_WHITESPACE",
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::sentence_whitespace::check_pl(&analyzed_sentences));
            }
            if builtin_active(
                "PL_UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_pl(&analyzed_sentences));
            }
            if let Some(polish) = &self.polish {
                if builtin_active(
                    polish.word_coherency.rule_id(),
                    "MISC",
                    true,
                    false,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(polish.word_coherency.check(&analyzed_sentences));
                }
            }
        }
        // Slovak text-level rules (`Slovak.getRelevantRules`):
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (4) and
        // MultipleWhitespace (6).
        if self.lang == crate::Lang::Sk {
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_sk(&analyzed_sentences));
            }
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_sk(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_sk(&analyzed_sentences));
            }
        }
        // Slovenian text-level rules (`Slovenian.getRelevantRules`):
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (5) and
        // MultipleWhitespace (7).
        if self.lang == crate::Lang::Sl {
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_sl(&analyzed_sentences));
            }
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_sl(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_sl(&analyzed_sentences));
            }
        }
        // Danish text-level rules (`Danish.getRelevantRules`):
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (5) and
        // MultipleWhitespace (6).
        if self.lang == crate::Lang::Da {
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_da(&analyzed_sentences));
            }
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_da(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_da(&analyzed_sentences));
            }
        }
        // Swedish text-level rules (`Swedish.getRelevantRules`):
        // GenericUnpairedBrackets (3), LongParagraph (5), UppercaseSentenceStart
        // (6), LongSentence (7, picky), WordCoherency (9), MultipleWhitespace
        // (10) and SentenceWhitespace (11).
        if self.lang == crate::Lang::Sv {
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_sv(&analyzed_sentences));
            }
            if builtin_active(
                crate::paragraph::LONG_PARAGRAPH_ID,
                "STYLE",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::long_paragraph_with_max(
                    &analyzed_sentences,
                    &crate::paragraph::strings_sv(),
                    150,
                ));
            }
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_sv(&analyzed_sentences));
            }
            if builtin_active(
                "TOO_LONG_SENTENCE",
                "STYLE",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::long_sentence::check_sv(&analyzed_sentences));
            }
            if let Some(swedish) = &self.sv {
                if builtin_active(
                    swedish.word_coherency.rule_id(),
                    "MISC",
                    true,
                    false,
                    options,
                    &enabled_rules,
                    &disabled_rules,
                    &disabled_categories,
                    &enabled_categories,
                ) {
                    text_level_matches.extend(swedish.word_coherency.check(&analyzed_sentences));
                }
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_sv(&analyzed_sentences));
            }
            if builtin_active(
                crate::sentence_whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::sentence_whitespace::check_sv(&analyzed_sentences));
            }
        }
        // Icelandic text-level rules (`Icelandic.getRelevantRules`):
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (5) and
        // MultipleWhitespace (7).
        if self.lang == crate::Lang::Is {
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_is(&analyzed_sentences));
            }
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_is(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_is(&analyzed_sentences));
            }
        }
        // Esperanto text-level rules (`Esperanto.getRelevantRules`):
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (5),
        // MultipleWhitespace (7) and SentenceWhitespace (8).
        if self.lang == crate::Lang::Eo {
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_eo(&analyzed_sentences));
            }
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_eo(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_eo(&analyzed_sentences));
            }
            if builtin_active(
                crate::sentence_whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::sentence_whitespace::check_eo(&analyzed_sentences));
            }
        }
        // Asturian text-level rules (`Asturian.getRelevantRules`):
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (5) and
        // MultipleWhitespace (6).
        if self.lang == crate::Lang::Ast {
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_ast(&analyzed_sentences));
            }
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_ast(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_ast(&analyzed_sentences));
            }
        }
        // Breton text-level rules (`Breton.getRelevantRules`):
        // UppercaseSentenceStart (4), MultipleWhitespace (5) and
        // SentenceWhitespace (6). Breton has no `GenericUnpairedBracketsRule`.
        if self.lang == crate::Lang::Br {
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_br(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_br(&analyzed_sentences));
            }
            if builtin_active(
                crate::sentence_whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::sentence_whitespace::check_br(&analyzed_sentences));
            }
        }
        // Tagalog text-level rules (`Tagalog.getRelevantRules`):
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (4) and
        // MultipleWhitespace (5).
        if self.lang == crate::Lang::Tl {
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_tl(&analyzed_sentences));
            }
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_tl(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_tl(&analyzed_sentences));
            }
        }
        // Lithuanian text-level rules (`Lithuanian.getRelevantRules`):
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (5) and
        // MultipleWhitespace (6).
        if self.lang == crate::Lang::Lt {
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_lt(&analyzed_sentences));
            }
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_lt(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_lt(&analyzed_sentences));
            }
        }
        // Belarusian text-level rules (`Belarusian.getRelevantRules`):
        // UppercaseSentenceStart (4), MultipleWhitespace (5),
        // SentenceWhitespace (6) and the default-off/picky paragraph rules
        // (7, 8, 10, 11, 12) plus LongSentence (9). The module has no
        // `GenericUnpairedBracketsRule`.
        if self.lang == crate::Lang::Be {
            let para = crate::paragraph::strings_be();
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_be(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_be(&analyzed_sentences));
            }
            if builtin_active(
                crate::sentence_whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::sentence_whitespace::check_be(&analyzed_sentences));
            }
            // `WHITESPACE_PARAGRAPH` (7), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::whitespace_before_paragraph_end_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            // `WHITESPACE_PARAGRAPH_BEGIN` (8), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_BEGIN_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                for sentence in &analyzed_sentences {
                    text_level_matches.extend(
                        crate::paragraph::whitespace_at_begin_of_paragraph_with(sentence, &para),
                    );
                }
            }
            // `TOO_LONG_SENTENCE` (9), `tags="picky"` (50 words)
            if options.picky && !disabled_rules.contains("TOO_LONG_SENTENCE") {
                text_level_matches.extend(crate::long_sentence::check_be(&analyzed_sentences));
            }
            // `TOO_LONG_PARAGRAPH` (10), default off and picky
            if builtin_active(
                crate::paragraph::LONG_PARAGRAPH_ID,
                "STYLE",
                false,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::long_paragraph_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            // `PARAGRAPH_REPEAT_BEGINNING_RULE` (11), default off
            if builtin_active(
                crate::paragraph::PARAGRAPH_REPEAT_BEGINNING_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::paragraph_repeat_beginning_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            // `PUNCTUATION_PARAGRAPH_END2` (12), default off
            if builtin_active(
                crate::paragraph::PUNCTUATION_PARAGRAPH_END2_ID,
                "PUNCTUATION",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::punctuation_at_paragraph_end2_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
        }
        // Russian text-level rules (`Russian.getRelevantRules`):
        // UppercaseSentenceStart (1), MultipleWhitespace (3),
        // SentenceWhitespace (4) and the default-off/picky paragraph rules
        // (5, 6, 8, 9, 11) plus LongSentence (7, 50 words). Russian has no
        // `GenericUnpairedBracketsRule`.
        if self.lang == crate::Lang::Ru {
            let para = crate::paragraph::strings_ru();
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_ru(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_ru(&analyzed_sentences));
            }
            if builtin_active(
                crate::sentence_whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::sentence_whitespace::check_ru(&analyzed_sentences));
            }
            // `WHITESPACE_PARAGRAPH` (5), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::whitespace_before_paragraph_end_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            // `WHITESPACE_PARAGRAPH_BEGIN` (6), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_BEGIN_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                for sentence in &analyzed_sentences {
                    text_level_matches.extend(
                        crate::paragraph::whitespace_at_begin_of_paragraph_with(sentence, &para),
                    );
                }
            }
            // `TOO_LONG_SENTENCE` (7), `tags="picky"` (50 words)
            if options.picky && !disabled_rules.contains("TOO_LONG_SENTENCE") {
                text_level_matches.extend(crate::long_sentence::check_ru(&analyzed_sentences));
            }
            // `TOO_LONG_PARAGRAPH` (8), default off and picky
            if builtin_active(
                crate::paragraph::LONG_PARAGRAPH_ID,
                "STYLE",
                false,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::long_paragraph_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            // `PARAGRAPH_REPEAT_BEGINNING_RULE` (9), default off
            if builtin_active(
                crate::paragraph::PARAGRAPH_REPEAT_BEGINNING_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::paragraph_repeat_beginning_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
            // `PUNCTUATION_PARAGRAPH_END2` (11), default off
            if builtin_active(
                crate::paragraph::PUNCTUATION_PARAGRAPH_END2_ID,
                "PUNCTUATION",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::punctuation_at_paragraph_end2_with(
                    &analyzed_sentences,
                    &para,
                ));
            }
        }
        // Crimean Tatar text-level rules (`CrimeanTatar.getRelevantRules`):
        // GenericUnpairedBrackets (2), UppercaseSentenceStart (3),
        // MultipleWhitespace (4), SentenceWhitespace (5) and the two
        // default-off paragraph rules (6, 7). The module has no
        // `MessagesBundle_crh`, so the core English strings apply.
        if self.lang == crate::Lang::Crh {
            if builtin_active(
                "UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_crh(&analyzed_sentences));
            }
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check(&analyzed_sentences));
            }
            if builtin_active(
                crate::sentence_whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::sentence_whitespace::check(&analyzed_sentences));
            }
            // `WhiteSpaceBeforeParagraphEnd` (6), default off
            if builtin_active(
                crate::paragraph::WHITESPACE_PARAGRAPH_ID,
                "STYLE",
                false,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::paragraph::whitespace_before_paragraph_end(
                    &analyzed_sentences,
                ));
            }
        }
        // Greek text-level rules (`Greek.getRelevantRules`):
        // GenericUnpairedBrackets (3), LongSentence (4, picky),
        // UppercaseSentenceStart (6) and MultipleWhitespace (7).
        if self.lang == crate::Lang::El {
            if builtin_active(
                "EL_UNPAIRED_BRACKETS",
                "PUNCTUATION",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::unpaired_brackets::check_el(&analyzed_sentences));
            }
            if builtin_active(
                "TOO_LONG_SENTENCE",
                "STYLE",
                true,
                true,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::long_sentence::check_el(&analyzed_sentences));
            }
            if builtin_active(
                "UPPERCASE_SENTENCE_START",
                "CASING",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::uppercase::check_el(&analyzed_sentences));
            }
            if builtin_active(
                crate::whitespace::RULE_ID,
                "TYPOGRAPHY",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches.extend(crate::whitespace::check_el(&analyzed_sentences));
            }
            // `GreekWordRepeatBeginningRule` (8), text level
            if builtin_active(
                "GREEK_WORD_REPEAT_BEGINNING_RULE",
                "REPETITIONS_STYLE",
                true,
                false,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            ) {
                text_level_matches
                    .extend(crate::el::rules::word_repeat_beginning(&analyzed_sentences));
            }
        }
        text_level_matches.append(&mut matches);
        matches = text_level_matches;
        // Text-level repetition rules (`RepeatedPatternRuleTransformer`):
        // per sentence, collapse overlapping matches of the same id
        // (`SameRuleGroupFilter`), then keep a match only when at least
        // `min_prev_matches` earlier matches are within the token distance.
        let mut repeating_ids: Vec<String> = Vec::new();
        for rm in &repeating {
            if !repeating_ids.contains(&rm.rule_id) {
                repeating_ids.push(rm.rule_id.clone());
            }
        }
        for id in repeating_ids {
            let mut group: Vec<&RepeatingMatch> =
                repeating.iter().filter(|r| r.rule_id == id).collect();
            group.sort_by_key(|r| (r.sentence, r.match_data.range.start));
            let mut candidates: Vec<&RepeatingMatch> = Vec::new();
            let mut i = 0usize;
            while i < group.len() {
                let cur = group[i];
                candidates.push(cur);
                let mut j = i + 1;
                while j < group.len()
                    && group[j].sentence == cur.sentence
                    && group[j].match_data.range.start <= cur.match_data.range.end
                    && group[j].match_data.range.end >= cur.match_data.range.start
                {
                    j += 1;
                }
                i = j;
            }
            let mut prev_from = 0usize;
            let mut distances: Vec<usize> = Vec::new();
            for (prev_matches, m) in candidates.into_iter().enumerate() {
                let max_distance = if m.distance_tokens < 1 {
                    60 * m.min_prev_matches.max(1) as usize
                } else {
                    m.distance_tokens as usize
                };
                distances.push(m.from_token.saturating_sub(prev_from));
                if prev_matches >= m.min_prev_matches.max(0) as usize {
                    let n = m.min_prev_matches.max(0) as usize;
                    let start = distances.len().saturating_sub(n);
                    if distances[start..].iter().sum::<usize>() < max_distance {
                        matches.push(m.match_data.clone());
                    }
                }
                prev_from = m.from_token;
            }
        }

        // `JLanguageTool.filterMatches` post-processing, in Java's order.
        // SameRuleGroupFilter does its own (stable) sort by start position;
        // the engine keeps Java's rule-execution order up to that point.
        matches = crate::matchfilters::same_rule_group_filter(matches);
        if self.lang == crate::Lang::En {
            matches = crate::matchfilters::english_filter_rule_matches(matches, text);
        }
        if self.lang == crate::Lang::Ca {
            // `LanguageDependentRuleMatchFilter` → `Catalan.filterRuleMatches`
            // (`adjustCatalanMatch`), before `CleanOverlappingFilter`.
            // `enabledRules` = `RuleSet.allRuleIds()` of the *active* rules.
            let enabled_ids = Self::active_rule_ids(
                &self.compiled_rules,
                options,
                &enabled_rules,
                &disabled_rules,
                &disabled_categories,
                &enabled_categories,
            );
            matches = crate::ca::match_filter::catalan_filter_rule_matches(
                matches,
                &sentences,
                &analyzed_sentences,
                &enabled_ids,
            );
        }
        if self.clean_overlapping_matches {
            let pt_variant = self.portuguese.as_ref().map(|p| p.variant.as_str());
            matches =
                crate::matchfilters::clean_overlapping_filter(matches, text, self.lang, pt_variant);
        }
        if self.lang == crate::Lang::Ca {
            // `Catalan.filterRuleMatchesAfterOverlapping` (`trimMatchEnds`).
            matches = crate::ca::match_filter::catalan_filter_rule_matches_after_overlapping(
                matches,
                &sentences,
                &analyzed_sentences,
            );
        }

        matches.sort_by_key(|m| (m.range.start, m.rule_id.clone()));
        Ok(CheckResult::new(text, sentences, matches))
    }

    /// Analyze one sentence (tokenize, chunk, disambiguate, paragraph-end
    /// marker) and return it with its `getTokensWithoutWhitespace` length.
    fn analyze_pipeline_sentence(
        &self,
        text: &str,
        start: usize,
        end: usize,
        is_last: bool,
    ) -> (lt_core::AnalyzedSentence, usize) {
        let mut analyzed = if self.norwegian.is_some() || self.nordum.is_some() {
            surface_sentence(&text[start..end])
        } else if self.guarani.is_some() {
            crate::gn::analyze_guarani_sentence(&text[start..end])
        } else if self.belarusian.is_some() {
            crate::be::analyze_belarusian_sentence(&text[start..end])
        } else if let Some(russian) = &self.russian {
            crate::ru::analyze_russian_sentence(russian, &text[start..end])
        } else {
            match &self.german {
                Some(german) => {
                    crate::de::pipeline::analyze_german_sentence(german, &text[start..end])
                }
                None => match &self.spanish {
                    Some(spanish) => {
                        crate::es::pipeline::analyze_spanish_sentence(spanish, &text[start..end])
                    }
                    None => match &self.french {
                        Some(french) => {
                            crate::fr::pipeline::analyze_french_sentence(french, &text[start..end])
                        }
                        None => match &self.italian {
                            Some(italian) => {
                                crate::it::analyze_italian_sentence(italian, &text[start..end])
                            }
                            None => match &self.portuguese {
                                Some(portuguese) => crate::pt::analyze_portuguese_sentence(
                                    portuguese,
                                    &text[start..end],
                                ),
                                None => match &self.dutch {
                                    Some(dutch) => {
                                        crate::nl::analyze_dutch_sentence(dutch, &text[start..end])
                                    }
                                    None => match &self.catalan {
                                        Some(catalan) => crate::ca::analyze_catalan_sentence(
                                            catalan,
                                            &text[start..end],
                                        ),
                                        None => match &self.romanian {
                                            Some(romanian) => crate::ro::analyze_romanian_sentence(
                                                romanian,
                                                &text[start..end],
                                            ),
                                            None => match &self.polish {
                                                Some(polish) => crate::pl::analyze_polish_sentence(
                                                    polish,
                                                    &text[start..end],
                                                ),
                                                None => match &self.galician {
                                                    Some(galician) => {
                                                        crate::gl::analyze_galician_sentence(
                                                            galician,
                                                            &text[start..end],
                                                        )
                                                    }
                                                    None => match &self.slovak {
                                                        Some(slovak) => {
                                                            crate::sk::analyze_slovak_sentence(
                                                                slovak,
                                                                &text[start..end],
                                                            )
                                                        }
                                                        None => match &self.greek {
                                                            Some(greek) => {
                                                                crate::el::analyze_greek_sentence(
                                                                    greek,
                                                                    &text[start..end],
                                                                )
                                                            }
                                                            None => match &self.da {
                                                                Some(danish) => {
                                                                    crate::da::analyze_danish_sentence(
                                                                        danish,
                                                                        &text[start..end],
                                                                    )
                                                                }
                                                                None => match &self.sv {
                                                                    Some(swedish) => {
                                                                        crate::sv::analyze_swedish_sentence(
                                                                            swedish,
                                                                            &text[start..end],
                                                                        )
                                                                    }
                                                                    None => match &self.slovenian {
                                                                        Some(_) => surface_sentence(
                                                                            &text[start..end],
                                                                        ),
                                                                        None if self.lithuanian.is_some() => surface_sentence(
                                                                            &text[start..end],
                                                                        ),
                                                                        None => match &self.icelandic {
                                                                            Some(_) => surface_sentence(
                                                                                &text[start..end],
                                                                            ),
                                                                            None => match &self.esperanto {
                                                                                Some(esperanto) => crate::eo::analyze_esperanto_sentence(
                                                                                    esperanto,
                                                                                    &text[start..end],
                                                                                ),
                                                                                None => match &self.asturian {
                                                                                    Some(asturian) => crate::ast::analyze_asturian_sentence(
                                                                                        asturian,
                                                                                        &text[start..end],
                                                                                    ),
                                                                                    None => match &self.breton {
                                                                                        Some(breton) => crate::br::analyze_breton_sentence(
                                                                                            breton,
                                                                                            &text[start..end],
                                                                                        ),
                                                                                        None => match &self.tagalog {
                                                                                            Some(tagalog) => crate::tl::analyze_tagalog_sentence(
                                                                                                tagalog,
                                                                                                &text[start..end],
                                                                                            ),
                                                                                            None => match &self.crimean_tatar {
                                                                                                Some(crh) => crate::crh::analyze_crimean_tatar_sentence(
                                                                                                    crh,
                                                                                                    &text[start..end],
                                                                                                ),
                                                                                                None => analyze_sentence(
                                                                                                    self.tagger
                                                                                                        .as_deref()
                                                                                                        .expect("english tagger"),
                                                                                                    &text[start..end],
                                                                                                ),
                                                                                            },
                                                                                        },
                                                                                    },
                                                                                },
                                                                            },
                                                                        },
                                                                    },
                                                                },
                                                            },
                                                        },
                                                    },
                                                },
                                            },
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
            }
        };
        analyzed.offset = start;
        // chunker runs on raw tokens (LT getRawAnalyzedSentence); the tags
        // survive disambiguation in place
        if let Some(chunker) = &self.english_chunker {
            chunker.add_chunk_tags(&mut analyzed.tokens);
        }
        // Java `JLanguageTool.getAnalyzedSentence`: the pre-disambiguation
        // snapshot is taken after tokenizer/tagger (+ the pre-disambiguation
        // chunker) and before the disambiguator; `raw_pos="yes"` rules match
        // against it. French takes it here (Java's view also reflects the
        // in-place chunker/`action="add"` mutations for Catalan, so the
        // Catalan pipeline snapshots after its chunkers instead — D-185).
        // Skipped when no compiled rule needs it.
        let needs_pre_disambig = self.compiled_rules.iter().any(|r| r.raw_pos);
        if self.french.is_some() && needs_pre_disambig {
            analyzed.pre_disambig_tokens = analyzed.tokens.clone();
            analyzed.pre_disambig_detached = Vec::new();
        }
        self.apply_disambiguation(&mut analyzed, needs_pre_disambig);
        // German's post-disambiguation chunker (`German.createDefaultPost
        // DisambiguationChunker`), after the XML disambiguation
        if let Some(german) = &self.german {
            german.chunker.add_chunk_tags(&mut analyzed.tokens);
        }
        // Java `analyzeSentences`: the last sentence of the text carries the
        // paragraph-end marker on its final token (`markAsParagraphEnd`).
        if is_last {
            if let Some(token) = analyzed.tokens.last_mut() {
                token.set_paragraph_end();
            }
            if let Some(token) = analyzed.pre_disambig_tokens.last_mut() {
                token.set_paragraph_end();
            }
        }
        let view_len = analyzed
            .tokens
            .iter()
            .filter(|t| {
                !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
            })
            .count();
        (analyzed, view_len)
    }

    /// Single-pass sentence loop (identical to the pre-parallelization code
    /// path; used for short texts).
    #[allow(clippy::too_many_arguments)]
    fn check_sentences_sequential(
        &self,
        spans: &[(usize, usize)],
        text: &str,
        options: &crate::EngineOptions,
        enabled_rules: &HashSet<&str>,
        disabled_rules: &HashSet<&str>,
        disabled_categories: &HashSet<&str>,
        enabled_categories: &HashSet<&str>,
    ) -> (
        Vec<Sentence>,
        Vec<lt_core::AnalyzedSentence>,
        Vec<Match>,
        Vec<RepeatingMatch>,
    ) {
        let mut sentences = Vec::with_capacity(spans.len());
        let mut analyzed_sentences = Vec::with_capacity(spans.len());
        let mut matches: Vec<Match> = Vec::new();
        let mut repeating: Vec<RepeatingMatch> = Vec::new();
        let mut token_offset = 0usize;
        for (span_index, (start, end)) in spans.iter().copied().enumerate() {
            let sentence_text = &text[start..end];
            let (analyzed, view_len) =
                self.analyze_pipeline_sentence(text, start, end, span_index + 1 == spans.len());
            let (sentence_matches, sentence_repeating) = self.check_analyzed_sentence(
                &analyzed,
                sentence_text,
                start,
                span_index,
                token_offset,
                options,
                enabled_rules,
                disabled_rules,
                disabled_categories,
                enabled_categories,
            );
            matches.extend(sentence_matches);
            repeating.extend(sentence_repeating);
            token_offset += view_len.saturating_sub(1);
            sentences.push(Sentence {
                range: TextRange::new(start, end),
                text: sentence_text.to_string(),
            });
            analyzed_sentences.push(analyzed);
        }
        (sentences, analyzed_sentences, matches, repeating)
    }

    /// Two-pass sentence loop: analysis and rules fan out to worker threads
    /// for long texts, results merged in sentence order.
    #[allow(clippy::too_many_arguments)]
    fn check_sentences_parallel(
        &self,
        spans: &[(usize, usize)],
        text: &str,
        options: &crate::EngineOptions,
        enabled_rules: &HashSet<&str>,
        disabled_rules: &HashSet<&str>,
        disabled_categories: &HashSet<&str>,
        enabled_categories: &HashSet<&str>,
    ) -> (
        Vec<Sentence>,
        Vec<lt_core::AnalyzedSentence>,
        Vec<Match>,
        Vec<RepeatingMatch>,
    ) {
        let analyses: Vec<(lt_core::AnalyzedSentence, usize)> =
            map_sentences(spans.len(), |span_index| {
                let (start, end) = spans[span_index];
                self.analyze_pipeline_sentence(text, start, end, span_index + 1 == spans.len())
            });
        let sentences: Vec<Sentence> = spans
            .iter()
            .map(|&(start, end)| Sentence {
                range: TextRange::new(start, end),
                text: text[start..end].to_string(),
            })
            .collect();
        // Java `RepeatedPatternRuleTransformer` offset: each preceding
        // sentence contributes `tokensWithoutWhitespace - 1` (no SENT_START)
        let mut token_offsets: Vec<usize> = Vec::with_capacity(analyses.len());
        let mut next_token_offset = 0usize;
        for (_, view_len) in &analyses {
            token_offsets.push(next_token_offset);
            next_token_offset += view_len.saturating_sub(1);
        }
        let per_sentence: Vec<(Vec<Match>, Vec<RepeatingMatch>)> =
            map_sentences(spans.len(), |sentence_index| {
                let (start, end) = spans[sentence_index];
                self.check_analyzed_sentence(
                    &analyses[sentence_index].0,
                    &text[start..end],
                    start,
                    sentence_index,
                    token_offsets[sentence_index],
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                )
            });
        // `seen` is local to each sentence pass and a match key cannot repeat
        // across sentences (their byte ranges are disjoint)
        let mut matches: Vec<Match> = Vec::new();
        let mut repeating: Vec<RepeatingMatch> = Vec::new();
        for (sentence_matches, sentence_repeating) in per_sentence {
            matches.extend(sentence_matches);
            repeating.extend(sentence_repeating);
        }
        let analyzed_sentences: Vec<lt_core::AnalyzedSentence> =
            analyses.into_iter().map(|(analyzed, _)| analyzed).collect();
        (sentences, analyzed_sentences, matches, repeating)
    }

    /// Per-sentence rule pass (Java `SentenceLevelRule`s) for one analyzed
    /// sentence, in Java's rule-execution order. Long texts run the pass on
    /// worker threads (`map_sentences`); results are merged in sentence
    /// order, so the match order fed to the post-filters is unchanged.
    #[allow(clippy::too_many_arguments)]
    fn check_analyzed_sentence(
        &self,
        analyzed: &lt_core::AnalyzedSentence,
        sentence_text: &str,
        start: usize,
        sentence_index: usize,
        token_offset: usize,
        options: &crate::EngineOptions,
        enabled_rules: &HashSet<&str>,
        disabled_rules: &HashSet<&str>,
        disabled_categories: &HashSet<&str>,
        enabled_categories: &HashSet<&str>,
    ) -> (Vec<Match>, Vec<RepeatingMatch>) {
        let mut matches: Vec<Match> = Vec::new();
        let mut seen: HashSet<(String, usize, usize)> = HashSet::new();
        let mut repeating: Vec<RepeatingMatch> = Vec::new();

        // pattern rules match on the non-blank token view (LT
        // `getTokensWithoutWhitespace`), like the disambiguator
        let view: Vec<usize> = analyzed
            .tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
            })
            .map(|(i, _)| i)
            .collect();
        let token_refs: Vec<&lt_core::AnalyzedTokenReadings> =
            view.iter().map(|&i| &analyzed.tokens[i]).collect();
        // Java `PatternRuleMatcher.match`: `raw_pos="yes"` rules match on
        // `getPreDisambigTokensWithoutWhitespace()`; the tokenization is
        // identical, only the readings differ. Empty when the snapshot was
        // skipped (no raw_pos rule compiled).
        let pre_view: Vec<usize> = analyzed
            .pre_disambig_tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
            })
            .map(|(i, _)| i)
            .collect();
        let pre_token_refs: Vec<&lt_core::AnalyzedTokenReadings> = pre_view
            .iter()
            .map(|&i| &analyzed.pre_disambig_tokens[i])
            .collect();
        // Java `AnalyzedSentence.tokenOffsets`/`lemmaOffsets` (lowercased)
        let mut token_lower: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::with_capacity(token_refs.len());
        let mut lemma_lower: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::with_capacity(token_refs.len() * 2);
        for (i, t) in token_refs.iter().enumerate() {
            token_lower
                .entry(t.surface().to_lowercase())
                .or_default()
                .push(i);
            for r in &t.readings {
                let lemma = r.stem.as_deref().unwrap_or(&r.token).to_lowercase();
                let list = lemma_lower.entry(lemma).or_default();
                if list.last() != Some(&i) {
                    list.push(i);
                }
            }
        }
        // Java `RepeatedPatternRuleTransformer` offset: each preceding
        // sentence contributes `tokensWithoutWhitespace - 1` (no SENT_START)
        // English built-in rules in `English.getRelevantRules` order:
        // `CommaWhitespaceRule` (1), `DoublePunctuationRule` (2),
        // `EnglishSpecificCaseRule` (15), `AvsAnRule` (19),
        // `CompoundRule` (21), `ContractionSpellingRule` (22),
        // simple-replace family (26-30). They run before the XML rules
        // and feed the post-filters in this order.
        if self.lang == crate::Lang::En {
            matches.extend(crate::comma_whitespace::check_sentence(
                &analyzed.tokens,
                sentence_text,
                start,
            ));
            matches.extend(crate::double_punctuation::check_sentence(
                &analyzed.tokens,
                start,
            ));
        }
        // `WhiteSpaceAtBeginOfParagraph` (7): sentence-level, default off
        if builtin_active(
            crate::paragraph::WHITESPACE_PARAGRAPH_BEGIN_ID,
            "STYLE",
            false,
            false,
            options,
            enabled_rules,
            disabled_rules,
            disabled_categories,
            enabled_categories,
        ) {
            for m in crate::paragraph::whitespace_at_begin_of_paragraph(analyzed) {
                let key = (m.rule_id.clone(), m.range.start, m.range.end);
                if seen.insert(key) {
                    matches.push(m);
                }
            }
        }
        if let Some(specific_case) = &self.specific_case {
            for m in specific_case.check_sentence(&analyzed.tokens, start) {
                let key = (m.rule_id.clone(), m.range.start, m.range.end);
                if seen.insert(key) {
                    matches.push(m);
                }
            }
        }
        // `EnglishWordRepeatRule` (18), before AvsAn (19). English-only: the
        // German built-in (`GERMAN_WORD_REPEAT_RULE`) runs in the German
        // section below.
        if self.lang == crate::Lang::En {
            for m in crate::en::word_repeat::check_sentence(&analyzed.tokens, start) {
                let key = (m.rule_id.clone(), m.range.start, m.range.end);
                if seen.insert(key) {
                    matches.push(m);
                }
            }
        }
        if let Some(avs_an) = &self.avs_an {
            for m in avs_an.check_sentence(&analyzed.tokens, start) {
                let key = (m.rule_id.clone(), m.range.start, m.range.end);
                if seen.insert(key) {
                    matches.push(m);
                }
            }
        }
        if let Some(compound) = &self.compound {
            for m in compound.check_sentence(&analyzed.tokens, sentence_text, start) {
                let key = (m.rule_id.clone(), m.range.start, m.range.end);
                if seen.insert(key) {
                    matches.push(m);
                }
            }
        }
        if let Some(contractions) = &self.contractions {
            for m in contractions.check_sentence(&analyzed.tokens, start) {
                let key = (m.rule_id.clone(), m.range.start, m.range.end);
                if seen.insert(key) {
                    matches.push(m);
                }
            }
        }
        if let Some(wrong_word) = &self.wrong_word_in_context {
            for m in wrong_word.check_sentence(analyzed, start) {
                let key = (m.rule_id.clone(), m.range.start, m.range.end);
                if seen.insert(key) {
                    matches.push(m);
                }
            }
        }
        if options.picky && !disabled_rules.contains("EN_DASH_RULE") {
            if let Some(dash) = &self.dash {
                for m in dash.check_sentence(analyzed) {
                    let key = (m.rule_id.clone(), m.range.start, m.range.end);
                    if seen.insert(key) {
                        matches.push(m);
                    }
                }
            }
        }
        // Portuguese sentence-level built-ins that Java runs before the
        // compound family: `CommaWhitespaceRule` (1) and
        // `MorfologikPortugueseSpellerRule` (3).
        if let Some(pt) = &self.portuguese {
            let pt_variant = pt.variant.as_str();
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "TYPOGRAPHY",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_pt(
                    &analyzed.tokens,
                    sentence_text,
                    start,
                    pt_variant,
                ),
                &mut seen,
            );
            let rule_id = pt.spelling.rule_id().to_string();
            append_active(
                &mut matches,
                builtin_active(
                    &rule_id,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                pt.spelling.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
        }
        // Portuguese compound family (`PT_COMPOUNDS_POST_REFORM` (16),
        // `PT_COLOUR_HYPHENATION` (17), the variant compound additions) and
        // the variant dash rules, in Java's rule-list order (before the
        // Rule2 replace rules 18+).
        if let Some(pt) = &self.portuguese {
            for rule in &pt.compounds {
                append_active(
                    &mut matches,
                    builtin_active(
                        rule.rule_id(),
                        "COMPOUNDING",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    rule.check_sentence(&analyzed.tokens, sentence_text, start),
                    &mut seen,
                );
            }
            for rule in &pt.dashes {
                append_active(
                    &mut matches,
                    builtin_active(
                        rule.rule_id(),
                        "TYPOGRAPHY",
                        true,
                        true,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    rule.check_sentence(analyzed),
                    &mut seen,
                );
            }
        }
        // Built-in `AbstractSimpleReplaceRule2` instances (Java rules run
        // before XML rules; same enable/picky semantics).
        for builtin in &self.simple_replace {
            let explicitly_enabled = enabled_rules.contains(builtin.rule_id());
            if builtin.default_off() && !explicitly_enabled {
                continue;
            }
            if !options.picky && builtin.picky() && !explicitly_enabled {
                continue;
            }
            if !enabled_rules.is_empty() && options.enabled_only {
                if !enabled_categories.is_empty() {
                    // Union of rule ids and categories (Java #12194/#aece4da).
                    let enabled_by_category = enabled_categories.contains(builtin.category_id());
                    if !enabled_rules.contains(builtin.rule_id()) && !enabled_by_category {
                        continue;
                    }
                } else if !enabled_rules.contains(builtin.rule_id()) {
                    continue;
                }
            } else {
                if disabled_rules.contains(builtin.rule_id()) {
                    continue;
                }
                if disabled_categories.contains(builtin.category_id()) {
                    continue;
                }
                if options.enabled_only
                    && !enabled_categories.is_empty()
                    && !enabled_categories.contains(builtin.category_id())
                {
                    continue;
                }
            }
            for m in builtin.check_sentence(&analyzed.tokens, start) {
                let key = (m.rule_id.clone(), m.range.start, m.range.end);
                if seen.insert(key) {
                    matches.push(m);
                }
            }
        }
        // Portuguese replace family in Java's rule-list order: the legacy
        // `AbstractSimpleReplaceRule` rules (orthography 18, replace 19,
        // pt-PT agreement 64) interleaved with the `AbstractSimpleReplaceRule2`
        // instances (common 20–31, variant 65–71), `DoublePunctuationRule`
        // (26) and the English contractions rule (174).
        if let Some(pt) = &self.portuguese {
            let pt_variant = pt.variant.as_str();
            for item in &pt.replace_order {
                match item {
                    crate::pt::ReplaceInstance::Rule2(i) => {
                        let builtin = &pt.replace_rule2[*i];
                        let explicitly_enabled = enabled_rules.contains(builtin.rule_id());
                        if builtin.default_off() && !explicitly_enabled {
                            continue;
                        }
                        if !options.picky && builtin.picky() && !explicitly_enabled {
                            continue;
                        }
                        if !enabled_rules.is_empty() && options.enabled_only {
                            if !enabled_categories.is_empty() {
                                // Union of rule ids and categories
                                // (Java #12194/#aece4da).
                                let enabled_by_category =
                                    enabled_categories.contains(builtin.category_id());
                                if !enabled_rules.contains(builtin.rule_id())
                                    && !enabled_by_category
                                {
                                    continue;
                                }
                            } else if !enabled_rules.contains(builtin.rule_id()) {
                                continue;
                            }
                        } else if disabled_rules.contains(builtin.rule_id())
                            || disabled_categories.contains(builtin.category_id())
                            || (options.enabled_only
                                && !enabled_categories.is_empty()
                                && !enabled_categories.contains(builtin.category_id()))
                        {
                            continue;
                        }
                        for m in builtin.check_sentence(&analyzed.tokens, start) {
                            let key = (m.rule_id.clone(), m.range.start, m.range.end);
                            if seen.insert(key) {
                                matches.push(m);
                            }
                        }
                    }
                    crate::pt::ReplaceInstance::Legacy(i) => {
                        let rule = &pt.legacy_replace[*i];
                        append_active(
                            &mut matches,
                            builtin_active(
                                rule.rule_id(),
                                rule.category_id(),
                                true,
                                false,
                                options,
                                enabled_rules,
                                disabled_rules,
                                disabled_categories,
                                enabled_categories,
                            ),
                            rule.check_sentence(&analyzed.tokens, start),
                            &mut seen,
                        );
                    }
                    crate::pt::ReplaceInstance::DoublePunctuation => {
                        append_active(
                            &mut matches,
                            builtin_active(
                                "DOUBLE_PUNCTUATION",
                                "PUNCTUATION",
                                true,
                                false,
                                options,
                                enabled_rules,
                                disabled_rules,
                                disabled_categories,
                                enabled_categories,
                            ),
                            crate::double_punctuation::check_sentence_pt(
                                &analyzed.tokens,
                                start,
                                pt_variant,
                            ),
                            &mut seen,
                        );
                    }
                }
            }
        }
        // `UnitConversionRuleUS` (`METRIC_UNITS_EN_US`, `tags="picky"`):
        // added by `AmericanEnglish.getRelevantRules` after the common
        // English built-ins, before the XML rules.
        if builtin_active(
            "METRIC_UNITS_EN_US",
            "STYLE",
            true,
            true,
            options,
            enabled_rules,
            disabled_rules,
            disabled_categories,
            enabled_categories,
        ) {
            for m in crate::unit_conversion::check_sentence(sentence_text, start) {
                let key = (m.rule_id.clone(), m.range.start, m.range.end);
                if seen.insert(key) {
                    matches.push(m);
                }
            }
        }
        // German sentence-level Java rules in `German.getRelevantRules`
        // order: GermanCommaWhitespace (1) … GermanDoublePunctuation (14),
        // MissingVerb (15), WiederVsWider (27) — before the XML rules.
        if self.lang == crate::Lang::De {
            // `OLD_SPELLING_RULE` (12), default on; Java runs it right after
            // the `SimpleReplaceRule` (11) built-ins
            if let Some(german) = &self.german {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::de::old_spelling::RULE_ID,
                        "TYPOS",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    german.old_spelling.check_sentence(analyzed, start),
                    &mut seen,
                );
            }
            // `WHITESPACE_RULE` (5), default on
            append_active(
                &mut matches,
                builtin_active(
                    crate::whitespace::RULE_ID,
                    "TYPOGRAPHY",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::whitespace::check_de(std::slice::from_ref(analyzed)),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_de(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DE_DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_de(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(german) = &self.german {
                append_active(
                    &mut matches,
                    builtin_active(
                        "MISSING_VERB",
                        "GRAMMAR",
                        false,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    crate::de::rules::missing_verb(german.tagger.base(), analyzed, start),
                    &mut seen,
                );
            }
            // `GERMAN_WORD_REPEAT_RULE` (16)
            append_active(
                &mut matches,
                builtin_active(
                    crate::de::repeat::WORD_REPEAT_ID,
                    "REDUNDANCY",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::de::repeat::word_repeat_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            // `GERMAN_WRONG_WORD_IN_CONTEXT` (18)
            if let Some(german) = &self.german {
                append_active(
                    &mut matches,
                    builtin_active(
                        "GERMAN_WRONG_WORD_IN_CONTEXT",
                        "CONFUSED_WORDS",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    german.wrong_word_in_context.check_sentence(analyzed, start),
                    &mut seen,
                );
            }
            // `DE_AGREEMENT` (19) / `DE_AGREEMENT2` (20), default on
            if let Some(german) = &self.german {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::de::agreement::AGREEMENT_ID,
                        "GRAMMAR",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    german.agreement.match_sentence(analyzed, start),
                    &mut seen,
                );
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::de::agreement::AGREEMENT2_ID,
                        "GRAMMAR",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    german.agreement2.match_sentence(analyzed, start),
                    &mut seen,
                );
            }
            // `DE_CASE` (21), default on
            if let Some(german) = &self.german {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::de::case_rule::RULE_ID,
                        "CASING",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    german.case_rule.match_sentence(analyzed, start),
                    &mut seen,
                );
            }
            // `DE_DASH` (22)
            append_active(
                &mut matches,
                builtin_active(
                    crate::de::style::DASH_ID,
                    "COMPOUNDING",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::de::style::dash(&analyzed.tokens, start),
                &mut seen,
            );
            // `DE_SUBJECT_VERB_AGREEMENT` (24)
            if let Some(german) = &self.german {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::de::subject_verb_agreement::RULE_ID,
                        "GRAMMAR",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    german
                        .subject_verb_agreement
                        .match_sentence(analyzed, start),
                    &mut seen,
                );
            }
            append_active(
                &mut matches,
                builtin_active(
                    "DE_WIEDER_VS_WIDER",
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::de::rules::wieder_vs_wider(analyzed, start),
                &mut seen,
            );
            // `EINHEITEN_METRISCH` (40), default on
            append_active(
                &mut matches,
                builtin_active(
                    "EINHEITEN_METRISCH",
                    "STYLE",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::unit_conversion::check_sentence_cfg(
                    &crate::unit_conversion::CONFIG_DE,
                    sentence_text,
                    start,
                ),
                &mut seen,
            );
            // `MissingCommaRelativeClauseRule` (41), two default-on
            // instances (`behind=false` front, `behind=true` behind)
            for behind in [false, true] {
                let rule = crate::de::missing_comma::MissingCommaRelativeClauseRule::new(behind);
                append_active(
                    &mut matches,
                    builtin_active(
                        rule.rule_id(),
                        "HILFESTELLUNG_KOMMASETZUNG",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    rule.match_sentence(analyzed, start),
                    &mut seen,
                );
            }
            // `REDUNDANT_MODAL_VERB` (42), default off
            append_active(
                &mut matches,
                builtin_active(
                    crate::de::rules::REDUNDANT_MODAL_VERB_ID,
                    "STYLE",
                    false,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::de::rules::redundant_modal_verb(analyzed, start),
                &mut seen,
            );
            // `COMPOUND_INFINITIV_RULE` (44), default on
            if let Some(german) = &self.german {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::de::compound_infinitiv::RULE_ID,
                        "COMPOUNDING",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    german.compound_infinitiv.match_sentence(analyzed, start),
                    &mut seen,
                );
            }
            // `DE_COMPOUNDS`/`DE_CH_COMPOUNDS` (GermanyGerman/SwissGerman
            // add them after the common built-ins)
            if let Some(german) = &self.german {
                append_active(
                    &mut matches,
                    builtin_active(
                        german.compound.rule_id(),
                        "COMPOUNDING",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    german
                        .compound
                        .check_sentence(&analyzed.tokens, sentence_text, start),
                    &mut seen,
                );
            }
        }
        // Spanish sentence-level Java rules in `Spanish.getRelevantRules`
        // order: `MorfologikSpanishSpellerRule` (5) and
        // `SpanishWordRepeatRule` (7) run before the XML rules.
        if let Some(spanish) = &self.spanish {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "TYPOGRAPHY",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_es(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_es(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::es::spelling::RULE_ID,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                spanish.spelling.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::es::rules::WORD_REPEAT_RULE_ID,
                    "MISC",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::es::rules::word_repeat_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::es::simple_replace::SIMPLE_REPLACE_RULE_ID,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                spanish
                    .simple_replace
                    .check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "SPANISH_WRONG_WORD_IN_CONTEXT",
                    "CONFUSED_WORDS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                spanish
                    .wrong_word_in_context
                    .check_sentence(analyzed, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    spanish.compound.rule_id(),
                    "MISC",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                spanish
                    .compound
                    .check_sentence(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::es::simple_replace::SIMPLE_REPLACE_VERBS_RULE_ID,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                spanish
                    .simple_replace_verbs
                    .check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            let _ = &spanish.multitoken;
        }

        // French sentence-level Java rules in `French.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2),
        // MorfologikFrenchSpellerRule (4), CompoundRule (10),
        // QuestionWhitespaceStrict (11), QuestionWhitespace (12),
        // SimpleReplace (13).
        if let Some(french) = &self.french {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "TYPOGRAPHY",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_fr(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_fr(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::fr::spelling::RULE_ID,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                french.spelling.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    french.compound.rule_id(),
                    "MISC",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                french
                    .compound
                    .check_sentence(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::fr::question_whitespace::STRICT_RULE_ID,
                    "MISC",
                    false,
                    true,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                french
                    .question_whitespace_strict
                    .check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::fr::question_whitespace::RULE_ID,
                    "MISC",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                french
                    .question_whitespace
                    .check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::fr::simple_replace::SIMPLE_REPLACE_RULE_ID,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                french
                    .simple_replace
                    .check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            let _ = &french.multitoken;
        }
        // Italian sentence-level Java rules in `Italian.getRelevantRules`
        // order: WhitespaceBeforePunctuation (1), CommaWhitespace (2),
        // DoublePunctuation (3). `MorfologikItalianSpellerRule` (5) lands in
        // stage 2, `ItalianWordRepeatRule` (7) in stage 3.
        if self.lang == crate::Lang::It {
            append_active(
                &mut matches,
                builtin_active(
                    crate::whitespace_before_punctuation::RULE_ID,
                    "TYPOGRAPHY",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::whitespace_before_punctuation::check_sentence_it(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "TYPOGRAPHY",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_it(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_it(&analyzed.tokens, start),
                &mut seen,
            );
            // `MorfologikItalianSpellerRule` (5), default on
            if let Some(italian) = &self.italian {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::it::spelling::RULE_ID,
                        "TYPOS",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    italian.spelling.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
            }
            // `ItalianWordRepeatRule` (7), default on
            append_active(
                &mut matches,
                builtin_active(
                    crate::it::rules::WORD_REPEAT_RULE_ID,
                    "MISC",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::it::rules::word_repeat_sentence(&analyzed.tokens, start),
                &mut seen,
            );
        }
        // Dutch sentence-level Java rules in `Dutch.getRelevantRules` order:
        // CommaWhitespace (1), DoublePunctuation (2).
        // `MorfologikDutchSpellerRule` (5) lands in stage 2, the compound
        // family/CheckCase/PreferredWord/SpaceInCompound in stage 3.
        if self.lang == crate::Lang::Nl {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_nl(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_nl(&analyzed.tokens, start),
                &mut seen,
            );
            // `MorfologikDutchSpellerRule` (5), default on
            if let Some(dutch) = &self.dutch {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::nl::spelling::RULE_ID,
                        "TYPOS",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    dutch.spelling.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `CompoundRule` (7), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        "NL_COMPOUNDS",
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    dutch
                        .compound
                        .check_sentence(&analyzed.tokens, sentence_text, start),
                    &mut seen,
                );
                // `DutchWrongWordInContextRule` (8), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        "DUTCH_WRONG_WORD_IN_CONTEXT",
                        "CONFUSED_WORDS",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    dutch.wrong_word_in_context.check_sentence(analyzed, start),
                    &mut seen,
                );
                // `SimpleReplaceRule` (10), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        "NL_SIMPLE_REPLACE",
                        "VERGISSINGEN",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    dutch.simple_replace.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `PreferredWordRule` (13), default on (inner rule id)
                append_active(
                    &mut matches,
                    builtin_active(
                        "NL_PREFERRED_WORD_RULE",
                        "STYLE",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    dutch.preferred_word.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `SpaceInCompoundRule` (14), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        "NL_SPACE_IN_COMPOUND",
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    dutch
                        .space_in_compound
                        .check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `CheckCaseRule` (16), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        "NL_CHECKCASE",
                        "CASING",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    dutch.check_case.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
            }
        }
        // Galician sentence-level Java rules in `Galician.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2), Hunspell (4) and
        // the replace family (15–20). The speller is stage 2 and the replace
        // family stage 3.
        if self.lang == crate::Lang::Gl {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_gl(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_gl(&analyzed.tokens, start),
                &mut seen,
            );
            // `HunspellRule` (4), default on
            if let Some(galician) = &self.galician {
                if let Some(spelling) = &galician.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::gl::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, sentence_text, start),
                        &mut seen,
                    );
                }
                // `SimpleReplaceRule` (15), `CastWordsRule` (16)
                for rule in &galician.legacy_replace {
                    append_active(
                        &mut matches,
                        builtin_active(
                            rule.rule_id(),
                            "MISC",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        rule.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
                // `AbstractSimpleReplaceRule2` family (17–20)
                let rule2_categories = ["REDUNDANCY", "REDUNDANCY", "STYLE", "WIKIPEDIA"];
                for (rule, category) in galician.rule2.iter().zip(rule2_categories) {
                    append_active(
                        &mut matches,
                        builtin_active(
                            rule.rule_id(),
                            category,
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        rule.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
            }
        }
        // Romanian sentence-level Java rules in `Romanian.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2), WordRepeatRule
        // (6), MorfologikRomanianSpellerRule (7), SimpleReplaceRule (9),
        // CompoundRule (10). UppercaseSentenceStart (3),
        // GenericUnpairedBrackets (5) and RomanianWordRepeatBeginning (8) are
        // text-level and run above.
        if self.lang == crate::Lang::Ro {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_ro(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_ro(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(romanian) = &self.romanian {
                // `WordRepeatRule` (6), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::ro::rules::WORD_REPEAT_RULE_ID,
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    romanian.word_repeat.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `MorfologikRomanianSpellerRule` (7), default on
                if let Some(spelling) = &romanian.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::ro::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
                // `SimpleReplaceRule` (9), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        romanian.simple_replace.rule_id(),
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    romanian
                        .simple_replace
                        .check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `CompoundRule` (10), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        romanian.compound.rule_id(),
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    romanian
                        .compound
                        .check_sentence(&analyzed.tokens, sentence_text, start),
                    &mut seen,
                );
            }
        }
        // Polish sentence-level Java rules in `Polish.getRelevantRules`
        // order: CommaWhitespace (1), WordRepeatRule (3). UppercaseSentenceStart
        // (2), MultipleWhitespace (4), SentenceWhitespace (5),
        // PolishUnpairedBrackets (6) and PolishWordRepeat (8) are text-level
        // and run above; the speller and the Polish replace/compound family
        // are stage 2/3.
        if self.lang == crate::Lang::Pl {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_pl(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            if let Some(polish) = &self.polish {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::pl::rules::WORD_REPEAT_RULE_ID,
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    polish.word_repeat.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `MorfologikPolishSpellerRule` (7), default on
                if let Some(spelling) = &polish.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::pl::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
                // `PolishWordRepeatRule` (8), default off
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::pl::rules::POLISH_WORD_REPEAT_ID,
                        "MISC",
                        false,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    polish
                        .polish_word_repeat
                        .check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `CompoundRule` (9), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        polish.compound.rule_id(),
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    polish
                        .compound
                        .check_sentence(&analyzed.tokens, sentence_text, start),
                    &mut seen,
                );
                // `SimpleReplaceRule` (10), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        polish.simple_replace.rule_id(),
                        "PRAWDOPODOBNE_LITEROWKI",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    polish
                        .simple_replace
                        .check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `DashRule` (12), default on and picky
                append_active(
                    &mut matches,
                    builtin_active(
                        polish.dash.rule_id(),
                        "TYPOGRAPHY",
                        true,
                        true,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    polish.dash.check_sentence(analyzed),
                    &mut seen,
                );
            }
        }
        // Slovak sentence-level Java rules in `Slovak.getRelevantRules` order:
        // CommaWhitespace (1), DoublePunctuation (2), WordRepeatRule (5),
        // MorphologikSpeller (8) and CompoundRule (7). UnpairedBrackets (3),
        // UppercaseSentenceStart (4) and MultipleWhitespace (6) are
        // text-level and run above.
        if self.lang == crate::Lang::Sk {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_sk(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_sk(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(slovak) = &self.slovak {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::word_repeat::RULE_ID,
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    slovak.word_repeat.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                if let Some(spelling) = &slovak.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::sk::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
                append_active(
                    &mut matches,
                    builtin_active(
                        slovak.compound.rule_id(),
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    slovak
                        .compound
                        .check_sentence(&analyzed.tokens, sentence_text, start),
                    &mut seen,
                );
            }
        }
        // Slovenian sentence-level Java rules in `Slovenian.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2), Speller (4),
        // WordRepeatRule (6). UnpairedBrackets (3), UppercaseSentenceStart (5)
        // and MultipleWhitespace (7) are text-level and run above.
        if self.lang == crate::Lang::Sl {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_sl(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_sl(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(slovenian) = &self.slovenian {
                if let Some(spelling) = &slovenian.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::sl::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::word_repeat::RULE_ID,
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    slovenian
                        .word_repeat
                        .check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
            }
        }
        // Greek sentence-level Java rules in `Greek.getRelevantRules` order:
        // CommaWhitespace (1), DoublePunctuation (2), Speller (5, stage 2),
        // WordRepeatRule (9). UnpairedBrackets (3), LongSentence (4),
        // UppercaseSentenceStart (6) and MultipleWhitespace (7) are text-level
        // and run above.
        if self.lang == crate::Lang::El {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_el(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_el(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(greek) = &self.greek {
                if let Some(spelling) = &greek.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::el::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::word_repeat::RULE_ID,
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    greek.word_repeat.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `ReplaceHomonymsRule` (10)
                append_active(
                    &mut matches,
                    builtin_active(
                        "GREEK_HOMONYMS_REPLACE",
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    greek.homonyms.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `GreekSpecificCaseRule` (11)
                append_active(
                    &mut matches,
                    builtin_active(
                        "EL_SPECIFIC_CASE",
                        "CASING",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    greek.specific_case.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `NumeralStressRule` (12)
                append_active(
                    &mut matches,
                    builtin_active(
                        "GREEK_ORTHOGRAPHY_NUMERAL_STRESS",
                        "ORTHOGRAPHY",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    crate::el::rules::numeral_stress(&analyzed.tokens, start),
                    &mut seen,
                );
                // `GreekRedundancyRule` (13)
                append_active(
                    &mut matches,
                    builtin_active(
                        "EL_REDUNDANCY_REPLACE",
                        "REDUNDANCY",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    greek.redundancy.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
            }
        }
        // Danish sentence-level Java rules in `Danish.getRelevantRules` order:
        // CommaWhitespace (1), DoublePunctuation (2), HunspellRule (4).
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (5) and
        // MultipleWhitespace (6) are text-level and run above.
        if self.lang == crate::Lang::Da {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_da(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_da(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(danish) = &self.da {
                if let Some(spelling) = &danish.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::da::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, sentence_text, start),
                        &mut seen,
                    );
                }
            }
        }
        // Swedish sentence-level Java rules in `Swedish.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2), HunspellRule (4),
        // WordRepeatRule (8) and CompoundRule (12).
        if self.lang == crate::Lang::Sv {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_sv(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_sv(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(swedish) = &self.sv {
                if let Some(spelling) = &swedish.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::sv::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, sentence_text, start),
                        &mut seen,
                    );
                }
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::word_repeat::RULE_ID,
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    swedish.word_repeat.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                append_active(
                    &mut matches,
                    builtin_active(
                        swedish.compound.rule_id(),
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    swedish
                        .compound
                        .check_sentence(&analyzed.tokens, sentence_text, start),
                    &mut seen,
                );
            }
        }
        // Icelandic sentence-level Java rules in `Icelandic.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2),
        // HunspellNoSuggestionRule (4) and WordRepeatRule (6).
        // GenericUnpairedBrackets (3), UppercaseSentenceStart (5) and
        // MultipleWhitespace (7) are text-level and run above.
        if self.lang == crate::Lang::Is {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_is(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_is(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(icelandic) = &self.icelandic {
                if let Some(spelling) = &icelandic.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::is::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, sentence_text, start),
                        &mut seen,
                    );
                }
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::word_repeat::RULE_ID,
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    icelandic
                        .word_repeat
                        .check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
            }
        }
        // Esperanto sentence-level Java rules in `Esperanto.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2), HunspellRule (4)
        // and WordRepeatRule (6).
        if self.lang == crate::Lang::Eo {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_eo(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_eo(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(esperanto) = &self.esperanto {
                if let Some(spelling) = &esperanto.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::eo::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, sentence_text, start),
                        &mut seen,
                    );
                }
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::word_repeat::RULE_ID,
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    esperanto
                        .word_repeat
                        .check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
            }
        }
        // Asturian sentence-level Java rules in `Asturian.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2) and
        // MorfologikAsturianSpellerRule (4). GenericUnpairedBrackets (3),
        // UppercaseSentenceStart (5) and MultipleWhitespace (6) are
        // text-level and run above.
        if self.lang == crate::Lang::Ast {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_ast(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_ast(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(asturian) = &self.asturian {
                if let Some(spelling) = &asturian.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::ast::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
            }
        }
        // Breton sentence-level Java rules in `Breton.getRelevantRules` order:
        // CommaWhitespace (1), DoublePunctuation (2),
        // MorfologikBretonSpellerRule (3) and TopoReplaceRule (7).
        // UppercaseSentenceStart (4), MultipleWhitespace (5) and
        // SentenceWhitespace (6) are text-level and run above.
        if self.lang == crate::Lang::Br {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_br(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_br(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(breton) = &self.breton {
                if let Some(spelling) = &breton.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::br::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::br::topo::RULE_ID,
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    breton.topo.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
            }
        }
        // Tagalog sentence-level Java rules in `Tagalog.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2) and
        // MorfologikTagalogSpellerRule (6). GenericUnpairedBrackets (3),
        // UppercaseSentenceStart (4) and MultipleWhitespace (5) are
        // text-level and run above.
        if self.lang == crate::Lang::Tl {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_tl(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_tl(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(tagalog) = &self.tagalog {
                if let Some(spelling) = &tagalog.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::tl::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
            }
        }
        // Lithuanian sentence-level Java rules in `Lithuanian.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2) and
        // MorfologikLithuanianSpellerRule (4). GenericUnpairedBrackets (3),
        // UppercaseSentenceStart (5) and MultipleWhitespace (6) are
        // text-level and run above. The speller runs over the vendored
        // third-party `lt_LT` dictionary.
        if self.lang == crate::Lang::Lt {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_lt(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_lt(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(lithuanian) = &self.lithuanian {
                if let Some(spelling) = &lithuanian.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::lt::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
            }
        }
        // Belarusian sentence-level Java rules in `Belarusian.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2) and
        // MorfologikBelarusianSpellerRule (3). UppercaseSentenceStart (4),
        // MultipleWhitespace (5), SentenceWhitespace (6) and the paragraph /
        // long-sentence rules run above as text-level rules;
        // `BE_SIMPLE_REPLACE` (13) and `BE_SPECIFIC_CASE` (14) run through the
        // generic `simple_replace`/`specific_case` slots.
        if self.lang == crate::Lang::Be {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_be(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_be(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(belarusian) = &self.belarusian {
                if let Some(spelling) = &belarusian.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::be::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
            }
        }
        // Russian sentence-level Java rules in `Russian.getRelevantRules`
        // order: CommaWhitespace (0), then the speller (2) and the
        // Russian-specific rules (stage 2/3).
        if self.lang == crate::Lang::Ru {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "TYPOGRAPHY",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_ru(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            if let Some(russian) = &self.russian {
                if let Some(spelling) = &russian.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::ru::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
                if let Some(spelling_yo) = &russian.spelling_yo {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::ru::spelling::YO_RULE_ID,
                            "TYPOS",
                            false,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling_yo.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
            }
        }
        // Crimean Tatar sentence-level Java rules in
        // `CrimeanTatar.getRelevantRules` order: CommaWhitespace (0),
        // DoublePunctuation (1) and MorfologikCrimeanTatarSpellerRule (8).
        // The generic text-level rules run above. The module has no
        // `MessagesBundle_crh`, so the core English strings apply.
        if self.lang == crate::Lang::Crh {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "TYPOGRAPHY",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            if let Some(crh) = &self.crimean_tatar {
                if let Some(spelling) = &crh.spelling {
                    append_active(
                        &mut matches,
                        builtin_active(
                            crate::crh::spelling::RULE_ID,
                            "TYPOS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        spelling.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
            }
            // `WhiteSpaceAtBeginOfParagraph` (7), default off
            append_active(
                &mut matches,
                builtin_active(
                    crate::paragraph::WHITESPACE_PARAGRAPH_BEGIN_ID,
                    "STYLE",
                    false,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::paragraph::whitespace_at_begin_of_paragraph(analyzed),
                &mut seen,
            );
        }
        // Catalan sentence-level Java rules in `Catalan.getRelevantRules`
        // order: CommaWhitespace (1), DoublePunctuation (2). The Catalan-only
        // built-ins and XML-referenced filters are stage 2/3.
        if self.lang == crate::Lang::Ca {
            append_active(
                &mut matches,
                builtin_active(
                    "COMMA_PARENTHESIS_WHITESPACE",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::comma_whitespace::check_sentence_ca(&analyzed.tokens, sentence_text, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    "DOUBLE_PUNCTUATION",
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::double_punctuation::check_sentence_ca(&analyzed.tokens, start),
                &mut seen,
            );
            // `MorfologikCatalanSpellerRule` (9), default on
            if let Some(catalan) = &self.catalan {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::ca::spelling::RULE_ID,
                        "TYPOS",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    catalan.spelling.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `CatalanWrongWordInContextRule` (12), default on
                append_active(
                    &mut matches,
                    builtin_active(
                        "CATALAN_WRONG_WORD_IN_CONTEXT",
                        "CONFUSED_WORDS",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    catalan
                        .wrong_word_in_context
                        .check_sentence(analyzed, start),
                    &mut seen,
                );
                // Legacy `AbstractSimpleReplaceRule` family in
                // `Catalan.getRelevantRules` order: verbs (13), balearic
                // (14, variant-disabled for ca-ES-balear), simple (15),
                // operation names (17), diacritics IEC (18), -ment adverbs
                // (22, default off + picky).
                for rule in &catalan.legacy_replace {
                    if catalan.variant == "ca-ES-balear"
                        && rule.rule_id() == crate::ca::legacy_simple_replace::BALEARIC_ID
                        && !enabled_rules.contains(rule.rule_id())
                    {
                        continue;
                    }
                    append_active(
                        &mut matches,
                        builtin_active(
                            rule.rule_id(),
                            rule.category_id(),
                            !rule.default_off(),
                            rule.picky(),
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        rule.check_sentence(&analyzed.tokens, analyzed, start),
                        &mut seen,
                    );
                }
                // `SimpleReplaceMultiwordsRule` (16)
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::ca::simple_replace::MULTIWORDS_ID,
                        "GRAMMAR",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    catalan.multiwords.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `SimpleReplaceAnglicism` (19) with the gender/number filter
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::ca::simple_replace::ANGLICISM_ID,
                        "STYLE",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    crate::ca::simple_replace::check_anglicism(
                        &catalan.anglicism,
                        &catalan.filter_env,
                        &analyzed.tokens,
                        analyzed,
                        start,
                    ),
                    &mut seen,
                );
                // `CheckCaseRule` (21)
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::ca::simple_replace::CHECK_CASE_ID,
                        "CASING",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    catalan.check_case.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `CatalanWordRepeatRule` (8)
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::ca::rules::WORD_REPEAT_ID,
                        "MISC",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    crate::ca::rules::word_repeat_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
                // `PronomFebleDuplicateRule` (20)
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::ca::rules::PRONOM_FEBLE_DUPLICATE_ID,
                        "PRONOMS_FEBLES",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    crate::ca::rules::pronom_feble_duplicate(&analyzed.tokens, start),
                    &mut seen,
                );
                // `CompoundRule` (24)
                append_active(
                    &mut matches,
                    builtin_active(
                        "CA_COMPOUNDS",
                        "COMPOUNDING",
                        true,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    catalan
                        .compound
                        .check_sentence(&analyzed.tokens, sentence_text, start),
                    &mut seen,
                );
                // DNV lemma rules (26–28)
                for rule in &catalan.dnv_replace {
                    append_active(
                        &mut matches,
                        builtin_active(
                            rule.rule_id,
                            rule.category_id(),
                            !rule.default_off(),
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        rule.check_sentence(&analyzed.tokens, start),
                        &mut seen,
                    );
                }
            }
        }
        // Hand-authored languages (Norwegian Bokmål, Nordum, Guaraní): the
        // Hunspell speller is the only language-level built-in here. The
        // word-list rules ride the shared `simple_replace` loop above.
        if let Some(norwegian) = &self.norwegian {
            append_active(
                &mut matches,
                builtin_active(
                    norwegian.repetition.rule_id(),
                    "MISC",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                norwegian.repetition.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::no::context::SPLIT_LEX_RULE_ID,
                    "TYPOS",
                    false,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::no::context::check_split_compound_lex(&analyzed.tokens, start, &|word| {
                    norwegian.spelling.is_known(word)
                }),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::no::context::SIN_HANS_RULE_ID,
                    "GRAMMAR",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::no::context::check_sin_hans(
                    &analyzed.tokens,
                    start,
                    &|word| norwegian.spelling.is_known(word),
                    &norwegian.gender_overrides,
                ),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::no::context::SEG_RULE_ID,
                    "GRAMMAR",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::no::context::check_seg_reflex(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::no::context::GENDER_RULE_ID,
                    "GRAMMAR",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::no::context::check_en_et_gender(
                    &analyzed.tokens,
                    start,
                    &|word| norwegian.spelling.is_known(word),
                    &norwegian.gender_overrides,
                ),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::no::context::COMMA_PP_RULE_ID,
                    "PUNCTUATION",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::no::context::check_comma_pp(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::no::spelling::RULE_ID,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                norwegian.spelling.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
        }
        if let Some(nordum) = &self.nordum {
            append_active(
                &mut matches,
                builtin_active(
                    crate::nrd::context::SPLIT_LEX_RULE_ID,
                    "TYPOS",
                    false,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::nrd::context::check_split_compound_lex(&analyzed.tokens, start, &|word| {
                    nordum.spelling.is_known(word)
                }),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::nrd::context::POSSESSIVE_RULE_ID,
                    "GRAMMAR",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::nrd::context::check_possessives(&analyzed.tokens, start, &|word| {
                    nordum.spelling.is_known(word)
                }),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    nordum.repetition.rule_id(),
                    "MISC",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                nordum.repetition.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::nrd::spelling::RULE_ID,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                nordum.spelling.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
        }
        if let Some(guarani) = &self.guarani {
            append_active(
                &mut matches,
                builtin_active(
                    guarani.repetition.rule_id(),
                    "MISC",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                guarani.repetition.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::gn::accents::RULE_ID,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                guarani.accents.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::gn::context::RULE_ID,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                guarani.harmony.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
            append_active(
                &mut matches,
                builtin_active(
                    crate::gn::spelling::RULE_ID,
                    "TYPOS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                guarani.spelling.check_sentence(&analyzed.tokens, start),
                &mut seen,
            );
        }
        // Portuguese sentence-level Java rules in `Portuguese.getRelevantRules`
        // order: CommaWhitespace (1), `MorfologikPortugueseSpellerRule` (3)
        // run early (before the compound family, see above);
        // DoublePunctuation (26) runs inside the merged replace-family loop.
        if self.lang == crate::Lang::Pt {
            let pt_variant = self
                .portuguese
                .as_ref()
                .map(|p| p.variant.as_str())
                .unwrap_or("pt-PT");
            // `PortugueseWordRepeatRule` (28), default on
            append_active(
                &mut matches,
                builtin_active(
                    crate::pt::rules::WORD_REPEAT_ID,
                    "REPETITIONS",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::pt::rules::word_repeat_sentence(&analyzed.tokens, start, pt_variant),
                &mut seen,
            );
            // `PortugueseAccentuationCheckRule` (30), default off
            if let Some(pt) = &self.portuguese {
                append_active(
                    &mut matches,
                    builtin_active(
                        crate::pt::accentuation::RULE_ID,
                        "CONFUSED_WORDS",
                        false,
                        false,
                        options,
                        enabled_rules,
                        disabled_rules,
                        disabled_categories,
                        enabled_categories,
                    ),
                    pt.accentuation.check_sentence(&analyzed.tokens, start),
                    &mut seen,
                );
            }
            // `PortugueseWrongWordInContextRule` (32), default on
            if let Some(pt) = &self.portuguese {
                if let Some(wwic) = &pt.wrong_word_in_context {
                    append_active(
                        &mut matches,
                        builtin_active(
                            "PORTUGUESE_WRONG_WORD_IN_CONTEXT",
                            "SEMANTICS",
                            true,
                            false,
                            options,
                            enabled_rules,
                            disabled_rules,
                            disabled_categories,
                            enabled_categories,
                        ),
                        wwic.check_sentence(analyzed, start),
                        &mut seen,
                    );
                }
            }
            // `PortugueseUnitConversionRule` (34), default on
            append_active(
                &mut matches,
                builtin_active(
                    "UNIDADES_METRICAS",
                    "STYLE",
                    true,
                    false,
                    options,
                    enabled_rules,
                    disabled_rules,
                    disabled_categories,
                    enabled_categories,
                ),
                crate::unit_conversion::check_sentence_cfg(
                    &crate::unit_conversion::CONFIG_PT,
                    sentence_text,
                    start,
                ),
                &mut seen,
            );
        }
        for rule in &self.compiled_rules {
            // Java `AbstractTokenBasedRule.canBeIgnoredFor`
            if token_refs.len() < rule.min_token_count {
                continue;
            }
            let hint_maps = (&token_lower, &lemma_lower);
            if !rule.hints.is_empty()
                && rule.hints.iter().any(|h| {
                    let map = if h.inflected {
                        hint_maps.1
                    } else {
                        hint_maps.0
                    };
                    h.values_lower.iter().all(|v| !map.contains_key(v))
                })
            {
                continue;
            }
            // Java `ignoreRule`: a default-off category stays disabled
            // unless the rule is explicitly enabled
            let explicitly_enabled = enabled_rules.contains(rule.rule_id.as_str());
            if !rule.category_default_on && !explicitly_enabled {
                continue;
            }
            // Java `isRuleActiveForLevelAndToneTags`: the default level
            // skips `tags="picky"` rules
            if !options.picky && rule.tags.iter().any(|t| t == "picky") {
                continue;
            }
            // Java `isRuleActiveForLevelAndToneTags` with the default
            // tone-tag set (this harness configures no goals): a rule that
            // carries tone tags and is goal-specific stays inactive at every
            // supported level, even when explicitly enabled.
            if rule.goal_specific && !rule.tone_tags.is_empty() {
                continue;
            }
            if !enabled_rules.is_empty() && options.enabled_only {
                if !enabled_categories.is_empty() {
                    // With both an explicit rule list and a category list,
                    // `enabledOnly` keeps the union (Java `Tools.selectRules`,
                    // #12194/#aece4da), not the intersection.
                    let enabled_by_category =
                        enabled_categories.contains(rule.category_id.as_str());
                    if !enabled_rules.contains(rule.rule_id.as_str()) && !enabled_by_category {
                        continue;
                    }
                } else if !enabled_rules.contains(rule.rule_id.as_str()) {
                    continue;
                }
            } else {
                if disabled_rules.contains(rule.rule_id.as_str()) {
                    continue;
                }
                if disabled_categories.contains(rule.category_id.as_str()) {
                    continue;
                }
                if options.enabled_only
                    && !enabled_categories.is_empty()
                    && !enabled_categories.contains(rule.category_id.as_str())
                {
                    continue;
                }
            }

            // `<regexp>` rules: match the sentence text directly
            // (LT RegexPatternRule); groups drive suggestions/messages
            if let Some((re, mark)) = &rule.regex {
                for caps in re.captures_iter(sentence_text) {
                    let Ok(caps) = caps else { continue };
                    let Some(whole) = caps.get(0) else { continue };
                    let marked = caps.get(*mark).unwrap_or(whole);
                    let mut range = TextRange::new(marked.start(), marked.end());
                    let mut message = pm::expand_message_backrefs(
                        &rule.message,
                        &rule.message_match_refs,
                        &|n| caps.get(n).map(|g| g.as_str().to_string()),
                    );
                    let mut suggestions = resolve_regexp_suggestions(&rule.suggestions, &caps);
                    if let Some((filter, filter_args)) = &rule.filter {
                        // RegexRuleFilterEvaluator passes args literally
                        // (no token back-references)
                        let mut args = std::collections::HashMap::new();
                        let mut args_ok = true;
                        for arg in filter_args.split_whitespace() {
                            match arg.split_once(':') {
                                Some((k, v)) => {
                                    args.insert(k.to_string(), v.to_string());
                                }
                                None => {
                                    args_ok = false;
                                    break;
                                }
                            }
                        }
                        if !args_ok {
                            continue;
                        }
                        let ctx = FilterContext {
                            rule_id: &rule.rule_id,
                            args,
                            pattern_tokens: &[],
                            sentence_tokens: &token_refs,
                            token_positions: &[],
                            pattern_token_pos: 0,
                            match_range: range,
                            sentence_text,
                            message: message.clone(),
                            short_message: rule.short_message.clone(),
                            suggestions: suggestions.clone(),
                        };
                        let captures: Vec<Option<String>> = (0..caps.len())
                            .map(|i| caps.get(i).map(|g| g.as_str().to_string()))
                            .collect();
                        let FilterOutcome {
                            accepted,
                            range: new_range,
                            message: new_message,
                            suggestions: new_suggestions,
                        } = filter.accept_regexp(&ctx, &captures);
                        if !accepted {
                            continue;
                        }
                        if let Some(r) = new_range {
                            range = r;
                        }
                        if let Some(msg) = new_message {
                            message = msg;
                        }
                        if let Some(suggs) = new_suggestions {
                            suggestions = suggs;
                        }
                    }
                    let abs_start = start + range.start;
                    let abs_end = start + range.end;
                    let m = Match::new(
                        rule.rule_id.clone(),
                        rule.sub_id.clone(),
                        message,
                        rule.short_message.clone(),
                        TextRange::new(abs_start, abs_end),
                        suggestions,
                        rule.category_id.clone(),
                        rule.category_name.clone(),
                    )
                    .with_metadata(
                        rule.description.clone(),
                        rule.issue_type.clone(),
                        rule.context_for_sure_match,
                    )
                    .with_picky(rule.tags.iter().any(|t| t == "picky"))
                    .with_original_error_str(
                        sentence_text
                            .get(range.start..range.end)
                            .map(str::to_string),
                    );
                    if rule.min_prev_matches > 0 {
                        // Java: first view token whose startPos >= fromPos
                        let mut from_tok = 0usize;
                        while from_tok < token_refs.len()
                            && token_refs[from_tok].start_pos < range.start
                        {
                            from_tok += 1;
                        }
                        repeating.push(RepeatingMatch {
                            sentence: sentence_index,
                            rule_id: rule.rule_id.clone(),
                            min_prev_matches: rule.min_prev_matches,
                            distance_tokens: rule.distance_tokens,
                            from_token: token_offset + from_tok,
                            match_data: m,
                        });
                        continue;
                    }
                    let key = (rule.rule_id.clone(), abs_start, abs_end);
                    if !seen.insert(key) {
                        continue;
                    }
                    matches.push(m);
                }
                continue;
            }

            // Java `PatternRule.checkForAntiPatterns`: when an antipattern
            // immunizes any token of the sentence, the rule is re-matched
            // on the immunized sentence (the matcher then skips immunized
            // tokens). Like Java, this only runs when the rule matched
            // something and the sentence has antipatterns; the immunized
            // view is computed at most once per rule.
            let mut immunized_owned: Option<Vec<lt_core::AnalyzedTokenReadings>> = None;
            // Java `PatternRuleHandler` case SUGGESTION: the rule-level
            // `suppress_misspelled` reaches every suggestion element.
            let effective_suppress: Vec<bool> = if rule.message_suppress_misspelled {
                vec![true; rule.suggestions.len()]
            } else {
                rule.suggestion_suppress.clone()
            };
            let mut immunized_computed = false;
            for pattern in &rule.compiled {
                let synth: Option<&dyn pm::Synthesizer> = self.synthesizer();
                // Java `PatternRuleMatcher.match`: raw_pos rules read the
                // pre-disambiguation view; `doMatch` also disables the anchor
                // optimization for them (`anchorIndices == null`), and the
                // hint maps (`canBeIgnoredFor`) keep using the disambiguated
                // sentence either way.
                let rule_refs: &[&lt_core::AnalyzedTokenReadings] =
                    if rule.raw_pos && !pre_token_refs.is_empty() {
                        &pre_token_refs
                    } else {
                        &token_refs
                    };
                let anchor_starts: Option<Vec<usize>> = if rule.raw_pos {
                    None
                } else {
                    pattern.anchor.as_ref().map(|a| {
                        let map = if a.inflected {
                            &lemma_lower
                        } else {
                            &token_lower
                        };
                        let mut starts: Vec<usize> = Vec::new();
                        for v in &a.values_lower {
                            if let Some(idxs) = map.get(v) {
                                for &i in idxs {
                                    if i >= a.token_index {
                                        starts.push(i - a.token_index);
                                    }
                                }
                            }
                        }
                        starts.sort_unstable();
                        starts.dedup();
                        starts
                    })
                };
                let found = pm::find_matches_with_synth(
                    pattern,
                    &rule.suggestions,
                    &effective_suppress,
                    rule_refs,
                    anchor_starts.as_deref(),
                    Some(&self.unify_config),
                    synth,
                );

                if found.is_empty() {
                    continue;
                }
                let found = if rule.antipatterns.is_empty() {
                    found
                } else {
                    if !immunized_computed {
                        immunized_computed = true;
                        let mut immune = vec![false; rule_refs.len()];
                        for ap in &rule.antipatterns {
                            // Java resolves `<match>` references in
                            // antipatterns with the language synthesizer;
                            // antipatterns always match the disambiguated
                            // view, even for raw_pos rules (`PatternRule
                            // .checkForAntiPatterns` uses a
                            // DisambiguationPatternRule matcher)
                            for m in pm::find_matches_with_synth(
                                ap,
                                &[],
                                &[],
                                &token_refs,
                                None,
                                Some(&self.unify_config),
                                synth,
                            ) {
                                // Java IMMUNIZE targets the `<marker>` span
                                // (position-corrected); without a marker every
                                // antipattern token is inside the marker.
                                let marker = if ap.marker_start.is_some() {
                                    lt_disambig::marker_targets(ap, &m.positions)
                                } else {
                                    None
                                };
                                match marker {
                                    Some((from, count)) => {
                                        for idx in from..(from + count).min(immune.len()) {
                                            immune[idx] = true;
                                        }
                                    }
                                    None => {
                                        for idx in m.start_tok()..=m.end_tok().min(immune.len() - 1)
                                        {
                                            immune[idx] = true;
                                        }
                                    }
                                }
                            }
                        }
                        if immune.iter().any(|x| *x) {
                            let mut owned: Vec<lt_core::AnalyzedTokenReadings> =
                                rule_refs.iter().map(|t| (*t).clone()).collect();
                            for (idx, flag) in immune.iter().enumerate() {
                                if *flag {
                                    owned[idx].is_immunized = true;
                                }
                            }
                            immunized_owned = Some(owned);
                        }
                    }
                    match &immunized_owned {
                        Some(owned) => {
                            let search_refs: Vec<&lt_core::AnalyzedTokenReadings> =
                                owned.iter().collect();
                            pm::find_matches_with_synth(
                                pattern,
                                &rule.suggestions,
                                &effective_suppress,
                                &search_refs,
                                anchor_starts.as_deref(),
                                Some(&self.unify_config),
                                synth,
                            )
                        }
                        None => found,
                    }
                };
                for m in found {
                    let mut range = pm::match_range(pattern, rule_refs, &m);
                    let synth: Option<&dyn pm::Synthesizer> = self.synthesizer();
                    let mut message = pm::expand_message_matches(
                        &rule.message,
                        &rule.message_match_refs,
                        rule_refs,
                        &m.positions,
                        synth,
                    );
                    let mut suggestions: Vec<Suggestion> = m
                        .suggestions
                        .iter()
                        .map(|s| Suggestion {
                            value: s.clone(),
                            short_description: None,
                        })
                        .collect();
                    // Java `createRuleMatch`: with `suppress_misspelled`
                    // (message or element) the unsynthesized `(...)`
                    // suggestions are removed; a match that then has no
                    // suggestions at all is dropped. The per-suggestion
                    // `<mistake/>` tagger check lives in `resolve_suggestions`.
                    let suppress_msg = rule.message_suppress_misspelled
                        || rule
                            .suggestion_suppress
                            .iter()
                            .take(rule.message.matches("<suggestion>").count())
                            .any(|x| *x);
                    if suppress_msg {
                        // Java marks both in-message and out-of-message
                        // suggestions with PLEASE_SPELL_ME when the rule
                        // (message) is suppress_misspelled
                        suggestions.retain(|s| !(s.value.contains('(') && s.value.contains(')')));
                        message = remove_suppressed_suggestions(&message, &suggestions);
                        if !message.contains("<suggestion>") && suggestions.is_empty() {
                            continue;
                        }
                    }
                    // Java `RuleMatch` constructor: suggestion case follows
                    // the matched text's case (unless the rule controls it
                    // with case-converting `<match>` elements).
                    let (starts_upper, all_upper) =
                        suggestion_case_flags(rule, pattern.marker_start, rule_refs, &m);
                    if starts_upper || all_upper {
                        let original = sentence_text.get(range.start..range.end).unwrap_or("");
                        for suggestion in &mut suggestions {
                            suggestion.value = adjust_suggestion_case(
                                &suggestion.value,
                                starts_upper,
                                all_upper,
                                original,
                            );
                        }
                        // Java `RuleMatch` collects the `<suggestion>` values in
                        // a `LinkedHashSet<SuggestedReplacement>` *after* the
                        // case conversion, so two forms that differ only in case
                        // collapse to one (Crimean Tatar `Terekke`/`terekke`).
                        let mut seen: Vec<(String, Option<String>)> = Vec::new();
                        suggestions.retain(|s| {
                            let key = (s.value.clone(), s.short_description.clone());
                            if seen.contains(&key) {
                                false
                            } else {
                                seen.push(key);
                                true
                            }
                        });
                    }
                    // `PatternRuleMatcher.createRuleMatch` FIXME quirk:
                    // when the formatted message or suggestion list
                    // contains `<suggestion>,`, the match starts at the end
                    // of the token before the first marker token.
                    let first_marker_tok = match (pattern.marker_start, pattern.marker_end) {
                        (Some(ms), Some(me)) if ms < m.positions.len() && me > ms => m.positions
                            [ms..me.min(m.positions.len())]
                            .iter()
                            .flatten()
                            .copied()
                            .next(),
                        _ => None,
                    }
                    .unwrap_or_else(|| m.start_tok());
                    let comma_suggestion = message.contains("<suggestion>,")
                        || suggestions.iter().any(|s| s.value.starts_with(','));
                    if first_marker_tok >= 1 && comma_suggestion {
                        range.start = rule_refs[first_marker_tok - 1].end_pos();
                    }
                    // Java `PatternRuleMatcher.createRuleMatch` only builds
                    // the match when `fromPos < toPos`: a marker that consumed
                    // only a zero-length token (e.g. the synthetic
                    // SENT_START) yields an empty range and is dropped.
                    if range.start >= range.end {
                        continue;
                    }
                    // LT `RuleMatch.getOriginalErrorStr()` is filled by the
                    // constructor (`setOriginalErrorStr=true` for pattern and
                    // regexp rules) from the underlined range; filters that
                    // rebuild the match with the plain constructor lose it.
                    let mut original_error_str = sentence_text
                        .get(range.start..range.end)
                        .map(str::to_string);
                    let mut filter_match_type: Option<&str> = None;
                    if let Some((filter, filter_args)) = &rule.filter {
                        let first = m.start_tok();
                        let last = m.end_tok();
                        let pattern_tokens: Vec<&lt_core::AnalyzedTokenReadings> =
                            rule_refs[first..=last].to_vec();
                        // consumed token counts per pattern element; unmatched
                        // optional elements contribute 0 (LT tokenPositions)
                        let mut token_positions = Vec::with_capacity(m.positions.len());
                        let mut prev: Option<usize> = None;
                        for pos in &m.positions {
                            match pos {
                                None => token_positions.push(0),
                                Some(p) => {
                                    let consumed = match prev {
                                        None => 1,
                                        Some(prev_p) => p - prev_p,
                                    };
                                    token_positions.push(consumed);
                                    prev = Some(*p);
                                }
                            }
                        }
                        let args = match lt_pattern::resolve_args(
                            filter_args,
                            &pattern_tokens,
                            &token_positions,
                        ) {
                            Ok(a) => a,
                            Err(e) => {
                                eprintln!("filter args error in rule {}: {e}", rule.rule_id);
                                continue;
                            }
                        };
                        let ctx = FilterContext {
                            rule_id: &rule.rule_id,
                            args,
                            pattern_tokens: &pattern_tokens,
                            sentence_tokens: rule_refs,
                            token_positions: &token_positions,
                            pattern_token_pos: first,
                            match_range: range,
                            sentence_text,
                            message: message.clone(),
                            short_message: rule.short_message.clone(),
                            suggestions: suggestions.clone(),
                        };
                        let FilterOutcome {
                            accepted,
                            range: new_range,
                            message: new_message,
                            suggestions: new_suggestions,
                        } = filter.accept(&ctx);
                        if !accepted {
                            continue;
                        }
                        // Java's remote filter only rebuilds (and resets the
                        // type) when it found a rewrite; otherwise the
                        // original match is returned unchanged.
                        let rebuilt_match = new_range.is_some();
                        if let Some(r) = new_range {
                            range = r;
                            // a rebuilt `RuleMatch` has no original error
                            // string (`setOriginalErrorStr=false`)
                            original_error_str = None;
                        }
                        if let Some(msg) = new_message {
                            message = msg;
                        }
                        if let Some(suggs) = new_suggestions {
                            suggestions = suggs;
                        }
                        // Java's `CatalanRemoteRewriteFilter` rebuilds the
                        // match with the plain `RuleMatch(rule, sentence, …)`
                        // constructor, which resets the type to `Other`.
                        if rebuilt_match {
                            if let Some(t) = filter.rebuilds_match_type() {
                                filter_match_type = Some(t);
                            }
                        }
                    }
                    let abs_start = start + range.start;
                    let abs_end = start + range.end;
                    let m = Match::new(
                        rule.rule_id.clone(),
                        rule.sub_id.clone(),
                        message,
                        rule.short_message.clone(),
                        TextRange::new(abs_start, abs_end),
                        suggestions,
                        rule.category_id.clone(),
                        rule.category_name.clone(),
                    )
                    .with_metadata(
                        rule.description.clone(),
                        rule.issue_type.clone(),
                        rule.context_for_sure_match,
                    )
                    .with_match_type(
                        filter_match_type.unwrap_or_else(|| pattern_match_type(&rule.issue_type)),
                    )
                    .with_original_error_str(original_error_str)
                    .with_picky(rule.tags.iter().any(|t| t == "picky"));
                    if rule.min_prev_matches > 0 {
                        // Java: first view token whose startPos >= fromPos
                        let mut from_tok = 0usize;
                        while from_tok < rule_refs.len()
                            && rule_refs[from_tok].start_pos < range.start
                        {
                            from_tok += 1;
                        }
                        repeating.push(RepeatingMatch {
                            sentence: sentence_index,
                            rule_id: rule.rule_id.clone(),
                            min_prev_matches: rule.min_prev_matches,
                            distance_tokens: rule.distance_tokens,
                            from_token: token_offset + from_tok,
                            match_data: m,
                        });
                        continue;
                    }
                    let key = (rule.rule_id.clone(), abs_start, abs_end);
                    if !seen.insert(key) {
                        continue;
                    }
                    matches.push(m);
                }
            }
        }

        // German speller (`GERMAN_SPELLER_RULE`, runs after the XML rules)
        if let Some(german) = &self.german {
            if builtin_active(
                german.spelling.rule_id(),
                "TYPOS",
                true,
                false,
                options,
                enabled_rules,
                disabled_rules,
                disabled_categories,
                enabled_categories,
            ) {
                for m in german.spelling.check_sentence(&analyzed.tokens, start) {
                    let key = (m.rule_id.clone(), m.range.start, m.range.end);
                    if !seen.insert(key) {
                        continue;
                    }
                    matches.push(m);
                }
            }
        }
        // spelling rule (D5, runs like a LT rule over each sentence)
        if let Some(spelling) = &self.spelling {
            if builtin_active(
                spelling.rule_id(),
                "TYPOS",
                true,
                false,
                options,
                enabled_rules,
                disabled_rules,
                disabled_categories,
                enabled_categories,
            ) {
                for m in spelling.check_sentence(&analyzed.tokens, start) {
                    let key = (m.rule_id.clone(), m.range.start, m.range.end);
                    if !seen.insert(key) {
                        continue;
                    }
                    matches.push(m);
                }
            }
        }
        (matches, repeating)
    }

    /// Language synthesizer for `<match postag="...">` rendering.
    fn synthesizer(&self) -> Option<&dyn pm::Synthesizer> {
        if let Some(german) = &self.german {
            return Some(german.synth_adapter.as_ref());
        }
        if let Some(spanish) = &self.spanish {
            return Some(spanish.synth_adapter.as_ref());
        }
        if let Some(french) = &self.french {
            return Some(french.synth_adapter.as_ref());
        }
        if let Some(italian) = &self.italian {
            return Some(italian.synth_adapter.as_ref());
        }
        if let Some(portuguese) = &self.portuguese {
            return Some(portuguese.synth_adapter.as_ref());
        }
        if let Some(dutch) = &self.dutch {
            return Some(dutch.synth_adapter.as_ref());
        }
        if let Some(catalan) = &self.catalan {
            return Some(catalan.synth_adapter.as_ref());
        }
        if let Some(galician) = &self.galician {
            return Some(galician.synth_adapter.as_ref());
        }
        if let Some(romanian) = &self.romanian {
            return Some(romanian.synth_adapter.as_ref());
        }
        if let Some(polish) = &self.polish {
            return Some(polish.synth_adapter.as_ref());
        }
        if let Some(slovak) = &self.slovak {
            return Some(slovak.synth_adapter.as_ref());
        }
        if let Some(greek) = &self.greek {
            return Some(greek.synth_adapter.as_ref());
        }
        if let Some(swedish) = &self.sv {
            return Some(swedish.synth_adapter.as_ref());
        }
        if let Some(crh) = &self.crimean_tatar {
            return Some(crh.synth_adapter.as_ref());
        }
        self.synthesizer
            .as_deref()
            .map(|s| s as &dyn pm::Synthesizer)
    }

    /// EnglishHybridDisambiguator order: global multiword chunker
    /// (spelling_global.txt, `_NONE_` tags), multiwords chunker, XML rules.
    fn apply_disambiguation(&self, sentence: &mut AnalyzedSentence, snapshot_catalan_pre: bool) {
        if let Some(german) = &self.german {
            // GermanRuleDisambiguator order: multitoken-ignore →
            // spelling_global → multitoken-suggest → XML rules
            german.disambiguate(sentence);
            return;
        }
        if let Some(spanish) = &self.spanish {
            // SpanishHybridDisambiguator order: spelling_global →
            // es/multiwords → XML rules
            spanish.disambiguate(sentence);
            return;
        }
        if let Some(french) = &self.french {
            // FrenchHybridDisambiguator order: spelling_global →
            // fr/multiwords → XML rules
            french.disambiguate(sentence);
            return;
        }
        if let Some(italian) = &self.italian {
            // ItalianRuleDisambiguator: XML rules (+ global rules)
            italian.disambiguate(sentence);
            return;
        }
        if let Some(portuguese) = &self.portuguese {
            // PortugueseHybridDisambiguator order: spelling_global →
            // pt/multiwords → XML rules
            portuguese.disambiguate(sentence);
            return;
        }
        if let Some(dutch) = &self.dutch {
            // DutchHybridDisambiguator order: spelling_global →
            // nl/multiwords → XML rules
            dutch.disambiguate(sentence);
            return;
        }
        if let Some(catalan) = &self.catalan {
            // CatalanHybridDisambiguator order: spelling_global ("NPCN000")
            // → ca/multiwords (removePreviousTags) → XML rules (+ global);
            // the `raw_pos` snapshot is taken after the chunkers (D-185).
            catalan.disambiguate_with_snapshot(sentence, snapshot_catalan_pre);
            return;
        }
        if let Some(galician) = &self.galician {
            // GalicianHybridDisambiguator order: gl/multiwords → XML rules
            // (+ global rules).
            galician.disambiguate(sentence);
            return;
        }
        if let Some(romanian) = &self.romanian {
            // `Romanian.createDefaultDisambiguator` is a plain
            // `XmlRuleDisambiguator`: XML rules (+ global rules).
            romanian.disambiguate(sentence);
            return;
        }
        if let Some(polish) = &self.polish {
            // PolishHybridDisambiguator order: XML rules (+ global rules) →
            // pl/multiwords chunker.
            polish.disambiguate(sentence);
            return;
        }
        if let Some(slovak) = &self.slovak {
            // `Slovak` does not override `createDefaultDisambiguator`, so the
            // base no-op `DemoDisambiguator` applies (no XML disambiguation).
            slovak.disambiguate(sentence);
            return;
        }
        if let Some(slovenian) = &self.slovenian {
            // `Slovenian` does not override `createDefaultDisambiguator`
            // either: the base no-op `DemoDisambiguator` applies.
            slovenian.disambiguate(sentence);
            return;
        }
        if let Some(icelandic) = &self.icelandic {
            // `Icelandic` does not override `createDefaultDisambiguator`: the
            // base no-op `DemoDisambiguator` applies.
            icelandic.disambiguate(sentence);
            return;
        }
        if let Some(esperanto) = &self.esperanto {
            // `Esperanto.createDefaultDisambiguator` is a plain
            // `XmlRuleDisambiguator`: XML rules (+ global rules).
            esperanto.disambiguate(sentence);
            return;
        }
        if let Some(asturian) = &self.asturian {
            // `Asturian` does not override `createDefaultDisambiguator`: the
            // base no-op `DemoDisambiguator` applies.
            asturian.disambiguate(sentence);
            return;
        }
        if let Some(breton) = &self.breton {
            // `Breton.createDefaultDisambiguator` is a plain
            // `XmlRuleDisambiguator`: XML rules only (no global rules).
            breton.disambiguate(sentence);
            return;
        }
        if let Some(tagalog) = &self.tagalog {
            // `Tagalog` does not override `createDefaultDisambiguator`: the
            // base no-op `DemoDisambiguator` applies.
            tagalog.disambiguate(sentence);
            return;
        }
        if let Some(lithuanian) = &self.lithuanian {
            // `Lithuanian` does not override `createDefaultDisambiguator`: the
            // base no-op `DemoDisambiguator` applies.
            lithuanian.disambiguate(sentence);
            return;
        }
        if let Some(crh) = &self.crimean_tatar {
            // `CrimeanTatar` does not override `createDefaultDisambiguator`:
            // the base no-op `DemoDisambiguator` applies.
            crh.disambiguate(sentence);
            return;
        }
        if let Some(greek) = &self.greek {
            // `Greek.createDefaultDisambiguator` is a plain
            // `XmlRuleDisambiguator`: XML rules (+ global rules).
            greek.disambiguate(sentence);
            return;
        }
        if let Some(danish) = &self.da {
            // `Danish.createDefaultDisambiguator` is a plain
            // `XmlRuleDisambiguator`: XML rules (+ global rules).
            danish.disambiguate(sentence);
            return;
        }
        if let Some(swedish) = &self.sv {
            // `SwedishHybridDisambiguator`: XML rules (+ global rules), then
            // the `sv/multiwords.txt` chunker.
            swedish.disambiguate(sentence);
            return;
        }
        if let Some(norwegian) = &self.norwegian {
            norwegian.disambiguate(sentence);
            return;
        }
        if let Some(nordum) = &self.nordum {
            nordum.disambiguate(sentence);
            return;
        }
        if let Some(guarani) = &self.guarani {
            guarani.disambiguate(sentence);
            return;
        }
        if let Some(russian) = &self.russian {
            // `RussianHybridDisambiguator`: `ru/multiwords.txt` chunker →
            // XML rules; the post-disambiguation `RussianChunker` runs after.
            russian.disambiguate(sentence);
            return;
        }
        self.global_chunker.apply(sentence);
        self.multiword_chunker.apply(sentence);
        self.disambiguator.apply(sentence);
    }
}

/// Minimum sentence count before the per-sentence passes fan out to worker
/// threads; short checks stay sequential (no thread overhead and exactly the
/// same execution as before).
const PARALLEL_MIN_SENTENCES: usize = 16;
/// Cap on the internal fan-out: callers may already run whole files in
/// parallel (`lt-cli check --lines --jobs N`).
const PARALLEL_MAX_THREADS: usize = 8;

/// Map `0..n` to results in parallel chunks for long inputs, preserving the
/// index order (`f` must be deterministic; the pipeline's post-filters see
/// the same sequence as the sequential loop).
fn map_sentences<R: Send>(n: usize, f: impl Fn(usize) -> R + Sync) -> Vec<R> {
    if n < PARALLEL_MIN_SENTENCES {
        return (0..n).map(&f).collect();
    }
    let threads = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1)
        .min(PARALLEL_MAX_THREADS)
        .min(n);
    if threads <= 1 {
        return (0..n).map(&f).collect();
    }
    let chunk = n.div_ceil(threads);
    let mut slots: Vec<Option<R>> = (0..n).map(|_| None).collect();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (chunk_index, slot) in slots.chunks_mut(chunk).enumerate() {
            let f = &f;
            handles.push(scope.spawn(move || {
                let base = chunk_index * chunk;
                for (offset, out) in slot.iter_mut().enumerate() {
                    *out = Some(f(base + offset));
                }
            }));
        }
        for handle in handles {
            handle.join().expect("sentence worker panicked");
        }
    });
    slots
        .into_iter()
        .map(|slot| slot.expect("sentence worker left a hole"))
        .collect()
}

/// `AbstractPatternRule.getType()`: XML pattern rules whose issue type is
/// `ITSIssueType.Style`, `LocaleViolation` or `Register` are reported as
/// `RuleMatch.Type.Hint`; everything else (and the regexp rules, which never
/// call `setType`) stays `Other` (D-024).
fn pattern_match_type(issue_type: &str) -> &'static str {
    match issue_type {
        "style" | "locale-violation" | "register" => "Hint",
        _ => "Other",
    }
}

/// Append rule matches that are active for the current options, deduping by
/// (rule id, range) like the per-sentence `seen` set.
fn append_active(
    matches: &mut Vec<Match>,
    active: bool,
    found: Vec<Match>,
    seen: &mut HashSet<(String, usize, usize)>,
) {
    if !active {
        return;
    }
    for m in found {
        let key = (m.rule_id.clone(), m.range.start, m.range.end);
        if seen.insert(key) {
            matches.push(m);
        }
    }
}

/// Rule activation for Java-coded built-ins, mirroring `JLanguageTool`'s
/// `ignoreRule`/`isRuleActiveForLevelAndToneTags`: default-off rules need an
/// explicit enable, `tags="picky"` rules need `Level.PICKY`.
#[allow(clippy::too_many_arguments)]
fn builtin_active(
    rule_id: &str,
    category_id: &str,
    default_on: bool,
    picky: bool,
    options: &crate::EngineOptions,
    enabled_rules: &HashSet<&str>,
    disabled_rules: &HashSet<&str>,
    disabled_categories: &HashSet<&str>,
    enabled_categories: &HashSet<&str>,
) -> bool {
    let explicitly_enabled = enabled_rules.contains(rule_id);
    if !default_on && !explicitly_enabled {
        return false;
    }
    if picky && !options.picky && !explicitly_enabled {
        return false;
    }
    if !enabled_rules.is_empty() && options.enabled_only {
        // With both an explicit rule list and a category list, `enabledOnly`
        // keeps the union (Java `Tools.selectRules`, #12194/#aece4da).
        return enabled_rules.contains(rule_id)
            || (!enabled_categories.is_empty() && enabled_categories.contains(category_id));
    }
    if disabled_rules.contains(rule_id) {
        return false;
    }
    if disabled_categories.contains(category_id) {
        return false;
    }
    if options.enabled_only
        && !enabled_categories.is_empty()
        && !enabled_categories.contains(category_id)
    {
        return false;
    }
    true
}

static NOT_WORD_STR: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[^\p{L}]+$").unwrap());
static PUNCTUATION_MARK: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[\p{P}']$").unwrap());

/// `StringTools.isAllUppercase(List<String>)`: all entries uppercase and at
/// least one entry that is not pure punctuation/symbols.
fn is_all_uppercase_tokens(tokens: &[&str]) -> bool {
    let mut all_upper = true;
    let mut all_not_letters = true;
    for s in tokens {
        all_upper = all_upper && lt_tagger::is_all_uppercase(s);
        all_not_letters =
            all_not_letters && (NOT_WORD_STR.is_match(s) || PUNCTUATION_MARK.is_match(s));
    }
    all_upper && !all_not_letters
}

/// `PatternRuleMatcher.matchPreservesCase`: a message-level case-converting
/// `<match>` plus a backreference at the start of the first `<suggestion>`
/// means the rule controls suggestion case itself. The Rust loader keeps
/// match references inside the suggestion parts, so this is checked there
/// (D-012 approximation: `isInMessageOnly` matches are not filtered out).
fn match_preserves_case(rule: &CompiledRule) -> bool {
    let has_converting = |parts: &Vec<SuggestionPart>| {
        parts.iter().any(|part| {
            matches!(part, SuggestionPart::MatchRef(spec) if !spec.case_conversion.is_empty())
        })
    };
    let starts_with_backref = |parts: &Vec<SuggestionPart>| match parts.first() {
        Some(SuggestionPart::MatchRef { .. }) => true,
        Some(SuggestionPart::Literal(text)) => {
            text.starts_with('\\') && text.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
        }
        _ => false,
    };
    let in_message_count = rule.message.matches("<suggestion>").count();
    // in-message suggestions: `matchPreservesCase(suggestionMatches, message)`
    if let Some(pos) = rule.message.find("<suggestion>") {
        if rule.message[pos + "<suggestion>".len()..].starts_with('\\')
            && rule
                .suggestions
                .iter()
                .take(in_message_count)
                .any(has_converting)
        {
            return false;
        }
    }
    // out-of-message suggestions: `matchPreservesCase(suggestionMatchesOutMsg,
    // suggestionsOutMsg)`. Java passes the flat list of all out-of-message
    // `<match>` elements, so a case-converting match in a *later* suggestion
    // also disables the automatic case adjustment.
    if let Some(parts) = rule.suggestions.get(in_message_count) {
        if starts_with_backref(parts)
            && rule
                .suggestions
                .iter()
                .skip(in_message_count)
                .any(has_converting)
        {
            return false;
        }
    }
    true
}

/// Java `PatternRuleMatcher.SUGGESTION_PATTERN_SUPPRESS`: unsynthesized
/// `(...)` replacements from `suppress_misspelled` suggestions are removed.
fn remove_suppressed_suggestions(message: &str, suggestions: &[Suggestion]) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(start) = rest.find("<suggestion>") {
        let Some(end) = rest[start..].find("</suggestion>") else {
            break;
        };
        let block = &rest[start..start + end + "</suggestion>".len()];
        let inner = &rest[start + "<suggestion>".len()..start + end];
        out.push_str(&rest[..start]);
        // drop a `(...)` placeholder and any suggestion that did not survive
        // the tagger-mistake suppression (`removeSuppressMisspelled`)
        let kept = suggestions.iter().any(|s| s.value == inner);
        if kept && !(inner.contains('(') && inner.contains(')')) {
            out.push_str(block);
        }
        rest = &rest[start + end + "</suggestion>".len()..];
    }
    out.push_str(rest);
    out
}

/// `PatternRuleMatcher.createRuleMatch`: `startsWithUppercase`/`isAllUppercase`
/// for the matched span (with the sentence-start workaround).
///
/// Java's `idx = firstMatchToken + correctedStPos` with
/// `correctedStPos = Σ tokenPositions[0..=startPositionCorrection] - 1`
/// (`startPositionCorrection` is the first `<marker>` pattern index, and a
/// span is 0 for an unmatched optional element). That is the last token of
/// the last matched pattern element up to and including the first marker
/// element — when that marker element is an unmatched optional token, `idx`
/// stays on the pre-marker token (e.g. PRONSUJ_NONVERBE's `il|elle [ne] pas`
/// samples `Il`, so Java capitalizes `n'a` to `N'a`). A marker at index 0 or
/// no marker keeps `firstMatchToken`.
fn suggestion_case_flags(
    rule: &CompiledRule,
    marker_start: Option<usize>,
    tokens: &[&lt_core::AnalyzedTokenReadings],
    m: &pm::PatternMatch,
) -> (bool, bool) {
    let last = m.end_tok();
    let first = m.start_tok();
    let idx = match marker_start {
        Some(ms) if ms > 0 && !m.positions.is_empty() => {
            let upto = ms.min(m.positions.len() - 1);
            m.positions[..=upto]
                .iter()
                .rev()
                .flatten()
                .next()
                .copied()
                .unwrap_or(first)
        }
        _ => first,
    };
    if tokens.is_empty() || last >= tokens.len() {
        return (false, false);
    }
    let idx = idx.min(tokens.len() - 1);
    let mut first_token = tokens[idx];
    let input_tokens: Vec<&str> = tokens[idx..=last].iter().map(|t| t.surface()).collect();
    let is_all_upper = is_all_uppercase_tokens(&input_tokens)
        && (first_token.surface().replace('\'', "").chars().count() > 1 || last > idx)
        && match_preserves_case(rule);
    let mut starts_upper = first_token
        .surface()
        .chars()
        .next()
        .is_some_and(char::is_uppercase)
        && match_preserves_case(rule);
    if first_token.is_sentence_start && tokens.len() > idx + 1 {
        // make uppercasing work also at sentence start
        first_token = tokens[idx + 1];
        starts_upper = first_token
            .surface()
            .chars()
            .next()
            .is_some_and(char::is_uppercase);
    }
    (starts_upper, is_all_upper)
}

/// `RuleMatch` constructor suggestion case handling (`isAllUppercase` first,
/// otherwise `startsWithUppercase`).
#[allow(clippy::nonminimal_bool)]
fn adjust_suggestion_case(
    replacement: &str,
    starts_with_uppercase: bool,
    is_all_uppercase: bool,
    original_error: &str,
) -> String {
    if is_all_uppercase && !(morfologik_is_mixed_case(replacement) && !replacement.contains(' ')) {
        // do not create a suggestion equal to the input string
        if original_error != replacement.to_uppercase() {
            replacement.to_uppercase()
        } else {
            replacement.to_string()
        }
    } else if starts_with_uppercase {
        lt_tagger::uppercase_first_char(replacement)
    } else {
        replacement.to_string()
    }
}

fn morfologik_is_mixed_case(s: &str) -> bool {
    !lt_tagger::is_all_uppercase(s)
        && !lt_tagger::is_capitalized_word(s)
        && lt_tagger::is_not_all_lowercase(s)
}

#[cfg(test)]
mod lookbehind_tests {
    use super::*;

    #[test]
    fn expands_variable_lookbehind() {
        let p = r"(?<![A-Z\$€£¥฿=]-?[0-9\.]{0,5})X";
        let expanded = expand_lookbehinds(p);
        println!("expanded: {expanded}");
        assert!(!expanded.contains("{0,5}"), "still variable: {expanded}");
        assert!(fancy_regex::Regex::new(&expanded).is_ok(), "must compile");
    }

    #[test]
    fn expands_alternation_lookbehind() {
        let p = r"(?<!([a-vyz]|[a-vyz]\d|[a-vyz]\d{2}))X";
        let expanded = expand_lookbehinds(p);
        println!("expanded: {expanded}");
        assert!(
            fancy_regex::Regex::new(&expanded).is_ok(),
            "must compile: {expanded}"
        );
    }
}
