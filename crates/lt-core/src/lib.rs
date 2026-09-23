//! Core types for LingoTweaker.
//!
//! Offsets: all ranges produced by the engine use UTF-8 byte offsets into the
//! original text. The HTTP layer converts these to UTF-16 code units to stay
//! Legacy-compatible (locked decision 11).

use serde::{Deserialize, Serialize};

pub mod regex_util;

/// A half-open range `[start, end)` in UTF-8 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Whether `offset` falls inside the range (`[start, end)`).
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }
}

/// The languages targeted for v1 parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Lang {
    En,
    De,
    Es,
    Fr,
    It,
    Pt,
    Nl,
    Ca,
    Gl,
    /// Romanian (`ro`).
    Ro,
    /// Polish (`pl`).
    Pl,
    /// Slovak (`sk`).
    Sk,
    /// Slovenian (`sl`).
    Sl,
    /// Greek (`el`).
    El,
    /// Danish (`da`).
    Da,
    /// Swedish (`sv`).
    Sv,
    /// Icelandic (`is`).
    Is,
    /// Esperanto (`eo`).
    Eo,
    /// Asturian (`ast`).
    Ast,
    /// Breton (`br`).
    Br,
    /// Tagalog (`tl`).
    Tl,
    /// Lithuanian (`lt`).
    Lt,
    /// Crimean Tatar (`crh`).
    Crh,
    /// Belarusian (`be`).
    Be,
    /// Russian (`ru`).
    Ru,
    /// Ukrainian (`uk`).
    Uk,
    /// Serbian (`sr`; default variant `sr-RS`, ekavian).
    Sr,
    /// Arabic (`ar`; default variant has no country, like `es`).
    Ar,
    /// Persian (`fa`; default variant `fa-IR`).
    Fa,
    /// Khmer (`km`; default variant `km-KH`).
    Km,
    /// Norwegian Bokmål (legacy dynamic language code `no`; `nb`
    /// accepted as an alias).
    No,
    /// Nordum, the constructed pan-Scandinavian written language
    /// (<https://www.nordum.org>; `nrd` is not an assigned ISO 639-3 code).
    Nrd,
    /// Paraguayan Guaraní (`gn`; `gug` accepted as an alias).
    Gn,
}

impl Lang {
    pub const ALL: [Lang; 33] = [
        Lang::En,
        Lang::De,
        Lang::Es,
        Lang::Fr,
        Lang::It,
        Lang::Pt,
        Lang::Nl,
        Lang::Ca,
        Lang::Gl,
        Lang::Ro,
        Lang::Pl,
        Lang::Sk,
        Lang::Sl,
        Lang::El,
        Lang::Da,
        Lang::Sv,
        Lang::Is,
        Lang::Eo,
        Lang::Ast,
        Lang::Br,
        Lang::Tl,
        Lang::Lt,
        Lang::Crh,
        Lang::Be,
        Lang::Ru,
        Lang::Uk,
        Lang::Sr,
        Lang::Ar,
        Lang::Fa,
        Lang::Km,
        Lang::No,
        Lang::Nrd,
        Lang::Gn,
    ];

    /// Parse a legacy long code such as `en-US`, `de-DE`, `es`, `fr`.
    pub fn from_long_code(code: &str) -> Option<Lang> {
        let lower = code.to_ascii_lowercase();
        let base = lower.split(['-', '_']).next().unwrap_or(&lower);
        match base {
            "en" => Some(Lang::En),
            "de" => Some(Lang::De),
            "es" => Some(Lang::Es),
            "fr" => Some(Lang::Fr),
            "it" => Some(Lang::It),
            "pt" => Some(Lang::Pt),
            "nl" => Some(Lang::Nl),
            "ca" => Some(Lang::Ca),
            "gl" => Some(Lang::Gl),
            "ro" => Some(Lang::Ro),
            "pl" => Some(Lang::Pl),
            "sk" => Some(Lang::Sk),
            "sl" => Some(Lang::Sl),
            "el" => Some(Lang::El),
            "da" => Some(Lang::Da),
            "sv" => Some(Lang::Sv),
            "is" => Some(Lang::Is),
            "eo" => Some(Lang::Eo),
            "ast" => Some(Lang::Ast),
            "br" => Some(Lang::Br),
            "tl" => Some(Lang::Tl),
            "lt" => Some(Lang::Lt),
            "crh" => Some(Lang::Crh),
            "be" => Some(Lang::Be),
            "ru" => Some(Lang::Ru),
            "uk" => Some(Lang::Uk),
            "sr" => Some(Lang::Sr),
            "ar" => Some(Lang::Ar),
            "fa" => Some(Lang::Fa),
            "km" => Some(Lang::Km),
            "no" | "nb" => Some(Lang::No),
            "nrd" => Some(Lang::Nrd),
            "gn" | "gug" => Some(Lang::Gn),
            _ => None,
        }
    }

    pub fn base_code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::It => "it",
            Lang::Pt => "pt",
            Lang::Nl => "nl",
            Lang::Ca => "ca",
            Lang::Gl => "gl",
            Lang::Ro => "ro",
            Lang::Pl => "pl",
            Lang::Sk => "sk",
            Lang::Sl => "sl",
            Lang::El => "el",
            Lang::Da => "da",
            Lang::Sv => "sv",
            Lang::Is => "is",
            Lang::Eo => "eo",
            Lang::Ast => "ast",
            Lang::Br => "br",
            Lang::Tl => "tl",
            Lang::Lt => "lt",
            Lang::Crh => "crh",
            Lang::Be => "be",
            Lang::Ru => "ru",
            Lang::Uk => "uk",
            Lang::Sr => "sr",
            Lang::Ar => "ar",
            Lang::Fa => "fa",
            Lang::Km => "km",
            Lang::No => "no",
            Lang::Nrd => "nrd",
            Lang::Gn => "gn",
        }
    }

    /// Legacy-style display name and long code.
    pub fn info(self) -> Language {
        match self {
            Lang::En => Language {
                code: "en",
                long_code: "en-US",
                name: "English (US)",
            },
            Lang::De => Language {
                code: "de",
                long_code: "de-DE",
                name: "German (Germany)",
            },
            Lang::Es => Language {
                code: "es",
                long_code: "es",
                name: "Spanish",
            },
            Lang::Fr => Language {
                code: "fr",
                long_code: "fr",
                name: "French",
            },
            Lang::It => Language {
                code: "it",
                long_code: "it",
                name: "Italian",
            },
            Lang::Pt => Language {
                code: "pt",
                long_code: "pt",
                name: "Portuguese",
            },
            Lang::Nl => Language {
                code: "nl",
                long_code: "nl",
                name: "Dutch",
            },
            Lang::Ca => Language {
                code: "ca",
                long_code: "ca",
                name: "Catalan",
            },
            Lang::Gl => Language {
                code: "gl",
                long_code: "gl",
                name: "Galician",
            },
            Lang::Ro => Language {
                code: "ro",
                long_code: "ro",
                name: "Romanian",
            },
            Lang::Pl => Language {
                code: "pl",
                long_code: "pl",
                name: "Polish",
            },
            Lang::Sk => Language {
                code: "sk",
                long_code: "sk",
                name: "Slovak",
            },
            Lang::Sl => Language {
                code: "sl",
                long_code: "sl",
                name: "Slovenian",
            },
            Lang::El => Language {
                code: "el",
                long_code: "el",
                name: "Greek",
            },
            Lang::Da => Language {
                code: "da",
                long_code: "da-DK",
                name: "Danish",
            },
            Lang::Sv => Language {
                code: "sv",
                long_code: "sv",
                name: "Swedish",
            },
            Lang::Is => Language {
                code: "is",
                long_code: "is-IS",
                name: "Icelandic",
            },
            Lang::Eo => Language {
                code: "eo",
                long_code: "eo",
                name: "Esperanto",
            },
            Lang::Ast => Language {
                code: "ast",
                long_code: "ast-ES",
                name: "Asturian",
            },
            Lang::Br => Language {
                code: "br",
                long_code: "br-FR",
                name: "Breton",
            },
            Lang::Tl => Language {
                code: "tl",
                long_code: "tl-PH",
                name: "Tagalog",
            },
            Lang::Lt => Language {
                code: "lt",
                long_code: "lt-LT",
                name: "Lithuanian",
            },
            Lang::Crh => Language {
                code: "crh",
                long_code: "crh-UA",
                name: "Crimean Tatar",
            },
            Lang::Be => Language {
                code: "be",
                long_code: "be-BY",
                name: "Belarusian",
            },
            Lang::Ru => Language {
                code: "ru",
                long_code: "ru-RU",
                name: "Russian",
            },
            Lang::Uk => Language {
                code: "uk",
                long_code: "uk-UA",
                name: "Ukrainian",
            },
            Lang::Sr => Language {
                code: "sr",
                long_code: "sr-RS",
                name: "Serbian",
            },
            Lang::Ar => Language {
                code: "ar",
                long_code: "ar",
                name: "Arabic",
            },
            Lang::Fa => Language {
                code: "fa",
                long_code: "fa-IR",
                name: "Persian",
            },
            Lang::Km => Language {
                code: "km",
                long_code: "km-KH",
                name: "Khmer",
            },
            Lang::No => Language {
                code: "no",
                long_code: "no",
                name: "Norwegian (Bokmål)",
            },
            Lang::Nrd => Language {
                code: "nrd",
                long_code: "nrd",
                name: "Nordum",
            },
            Lang::Gn => Language {
                code: "gn",
                long_code: "gn",
                name: "Guaraní",
            },
        }
    }
}

/// Static language metadata (legacy-compatible fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Language {
    pub code: &'static str,
    pub long_code: &'static str,
    pub name: &'static str,
}

/// A sentence produced by the splitter, with UTF-8 byte range into the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sentence {
    pub range: TextRange,
    pub text: String,
}

/// A replacement suggestion for a match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_description: Option<String>,
}

/// A rule match. Offsets are UTF-8 bytes; HTTP layers must convert to UTF-16.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Match {
    pub rule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_id: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_message: Option<String>,
    pub range: TextRange,
    pub suggestions: Vec<Suggestion>,
    pub category_id: String,
    pub category_name: String,
    /// rule description (LT rule name attribute)
    pub description: String,
    /// LT issue type (`issueType` attribute, default "Other")
    pub issue_type: String,
    /// LT `estimateContextForSureMatch` (approximation without
    /// antipattern terms)
    pub context_for_sure_match: i32,
    /// LT `RuleMatch.Type` name ("Other" for pattern rules,
    /// "UnknownWord" for spelling)
    pub match_type: String,
    /// LT `RuleMatch.getSpecificRuleId()`: overrides `rule_id` in the v2 JSON
    /// `rule.id` field (e.g. `EN_REPEATEDWORDS_PROBLEM`); `None` means the
    /// plain rule id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specific_rule_id: Option<String>,
    /// LT `Rule.hasTag(Tag.picky)`: `CleanOverlappingFilter` gives picky
    /// matches the `Integer.MIN_VALUE + 10000` penalty.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub picky: bool,
    /// LT `RuleMatch.getOriginalErrorStr()`: the underlined text captured by
    /// the rule's constructor (`null` = Java's `setOriginalErrorStr=false`
    /// constructors, e.g. filter-rebuilt matches). Catalan's
    /// `adjustCatalanMatch` appends a space to suggestions when this ends
    /// with an apostrophe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_error_str: Option<String>,
}

#[allow(clippy::too_many_arguments)]
impl Match {
    pub fn new(
        rule_id: impl Into<String>,
        sub_id: Option<String>,
        message: impl Into<String>,
        short_message: Option<String>,
        range: TextRange,
        suggestions: Vec<Suggestion>,
        category_id: impl Into<String>,
        category_name: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            sub_id,
            message: message.into(),
            short_message,
            range,
            suggestions,
            category_id: category_id.into(),
            category_name: category_name.into(),
            description: String::new(),
            issue_type: "Other".to_string(),
            context_for_sure_match: 0,
            match_type: "Other".to_string(),
            specific_rule_id: None,
            picky: false,
            original_error_str: None,
        }
    }

    /// Set the rule description (LT rule name) and issue type.
    pub fn with_metadata(
        mut self,
        description: impl Into<String>,
        issue_type: impl Into<String>,
        context_for_sure_match: i32,
    ) -> Self {
        self.description = description.into();
        self.issue_type = issue_type.into();
        self.context_for_sure_match = context_for_sure_match;
        self
    }

    /// Set the LT `RuleMatch.Type` name.
    pub fn with_match_type(mut self, match_type: impl Into<String>) -> Self {
        self.match_type = match_type.into();
        self
    }

    /// Set the LT `RuleMatch.getSpecificRuleId()` override.
    pub fn with_specific_rule_id(mut self, rule_id: impl Into<String>) -> Self {
        self.specific_rule_id = Some(rule_id.into());
        self
    }

    /// Mark the match as coming from a `tags="picky"` rule.
    pub fn with_picky(mut self, picky: bool) -> Self {
        self.picky = picky;
        self
    }

    /// Set LT `RuleMatch.getOriginalErrorStr()`.
    pub fn with_original_error_str(mut self, error: Option<String>) -> Self {
        self.original_error_str = error;
        self
    }
}

/// The result of checking a text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CheckResult {
    pub text: String,
    pub sentences: Vec<Sentence>,
    pub matches: Vec<Match>,
}

impl CheckResult {
    pub fn new(text: impl Into<String>, sentences: Vec<Sentence>, matches: Vec<Match>) -> Self {
        Self {
            text: text.into(),
            sentences,
            matches,
        }
    }
}

/// Java `Character.isWhitespace`: Unicode whitespace except the non-breaking
/// spaces, plus the file/group/record/unit separators.
pub fn java_char_is_whitespace(c: char) -> bool {
    match c {
        '\u{00A0}' | '\u{2007}' | '\u{202F}' => false,
        '\u{001C}'..='\u{001F}' => true,
        _ => c.is_whitespace(),
    }
}

/// Java `StringTools.isWhitespace`: empty strings and `\u{FEFF}` are
/// whitespace, `\u{200B}`/`\u{00A0}`/`\u{202F}` are whitespace, the OOo field
/// markers `\u{0001}`/`\u{0002}` are not, and everything else must trim to
/// empty or be a single `Character.isWhitespace` char.
pub fn is_whitespace(s: &str) -> bool {
    if s == "\u{0002}" || s == "\u{0001}" {
        return false;
    }
    if s == "\u{FEFF}" {
        return true;
    }
    // Java `String.trim` removes chars <= U+0020 only
    let trimmed = s.trim_matches(|c: char| (c as u32) <= 0x20);
    if trimmed.is_empty() {
        return true;
    }
    let mut chars = trimmed.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if s == "\u{200B}" || s == "\u{00A0}" || s == "\u{202F}" {
            return true;
        }
        return java_char_is_whitespace(c);
    }
    false
}

/// A single reading of a token: surface form, stem, and POS tag
/// (legacy `AnalyzedToken` equivalent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedToken {
    /// The token's surface form.
    pub token: String,
    /// The stem (lemma candidate); `None` when untagged.
    pub stem: Option<String>,
    /// The POS tag; `None` when the reading is a lemma-only/unknown reading.
    pub pos_tag: Option<String>,
}

impl AnalyzedToken {
    pub fn new(token: impl Into<String>, stem: Option<String>, pos_tag: Option<String>) -> Self {
        Self {
            token: token.into(),
            stem,
            pos_tag,
        }
    }

    /// Legacy semantics: lemma falls back to the token itself.
    pub fn lemma(&self) -> &str {
        self.stem.as_deref().unwrap_or(&self.token)
    }
}

/// All readings of one token, plus stream annotations (legacy
/// `AnalyzedTokenReadings` equivalent). Whitespace tokens are part of the
/// stream, mirroring LT's token arrays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedTokenReadings {
    pub readings: Vec<AnalyzedToken>,
    pub chunk_tags: Vec<String>,
    pub whitespace_before: bool,
    /// Byte offset of the token within the sentence text.
    pub start_pos: usize,
    /// Byte length of the original token text (the surface may be shorter,
    /// e.g. a typographic apostrophe is normalized to `'`).
    #[serde(default)]
    pub raw_byte_len: usize,
    pub is_whitespace: bool,
    pub is_sentence_start: bool,
    pub is_sentence_end: bool,
    pub is_paragraph_end: bool,
    /// True when a reading has a real POS tag (no fallback reading).
    pub is_tagged: bool,
    /// immunized by a disambiguation rule (spelling rules skip it)
    pub is_immunized: bool,
    /// marked as correctly-spelled multiword part by the chunker
    pub is_ignore_spelling: bool,
    /// surface contains a typographic apostrophe (’); set by the English
    /// tagger pipeline, consulted by `ApostropheTypeFilter`
    pub has_typographic_apostrophe: bool,
    /// LT `AnalyzedTokenReadings.isPosTagUnknown`: true when the token was
    /// constructed with exactly one reading that has no POS tag (the tagger
    /// fallback reading). The flag persists through disambiguation (Java's
    /// `addReading` keeps it), so it is stored, not derived.
    #[serde(default)]
    pub is_pos_tag_unknown: bool,
}

impl AnalyzedTokenReadings {
    /// A plain token entry with the given readings and default flags
    /// (LT `new AnalyzedTokenReadings(readings, startPos)`).
    pub fn new(readings: Vec<AnalyzedToken>) -> Self {
        let is_tagged = readings.iter().any(|r| r.pos_tag.is_some());
        let is_pos_tag_unknown = readings.len() == 1 && readings[0].pos_tag.is_none();
        Self {
            readings,
            chunk_tags: Vec::new(),
            whitespace_before: false,
            start_pos: 0,
            raw_byte_len: 0,
            is_whitespace: false,
            is_sentence_start: false,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe: false,
            is_pos_tag_unknown,
        }
    }

    /// Surface form (first reading's token; empty when all readings were
    /// removed by disambiguation).
    pub fn surface(&self) -> &str {
        self.readings
            .first()
            .map(|r| r.token.as_str())
            .unwrap_or("")
    }

    /// Byte offset just past the original token text.
    pub fn end_pos(&self) -> usize {
        self.start_pos + self.raw_byte_len
    }

    pub fn has_pos_tag(&self, tag: &str) -> bool {
        self.readings
            .iter()
            .any(|r| r.pos_tag.as_deref() == Some(tag))
    }

    /// LT `AnalyzedTokenReadings.hasPosTagStartingWith`.
    pub fn has_pos_tag_starting_with(&self, prefix: &str) -> bool {
        self.readings
            .iter()
            .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with(prefix)))
    }

    /// LT `AnalyzedTokenReadings.isLinebreak`: the token is `\n`, `\r`, `\n\r`
    /// or `\r\n`.
    pub fn is_linebreak(&self) -> bool {
        matches!(self.surface(), "\n" | "\r\n" | "\r" | "\n\r")
    }

    /// True if any reading's POS tag matches `pattern` (a regex).
    pub fn has_pos_tag_matching(&self, pattern: &regex::Regex) -> bool {
        self.readings.iter().any(|r| {
            r.pos_tag
                .as_deref()
                .map(|t| pattern.is_match(t))
                .unwrap_or(false)
        })
    }

    /// `fancy-regex` variant (Java-style patterns with lookaround).
    pub fn has_pos_tag_matching_fancy(&self, pattern: &fancy_regex::Regex) -> bool {
        self.readings.iter().any(|r| {
            r.pos_tag
                .as_deref()
                .map(|t| pattern.is_match(t).unwrap_or(false))
                .unwrap_or(false)
        })
    }

    pub fn has_lemma(&self, lemma: &str) -> bool {
        self.readings.iter().any(|r| r.lemma() == lemma)
    }

    /// LT `AnalyzedTokenReadings.addReading`: drops a trailing reading
    /// without a POS tag (the unknown-word fallback), then appends.
    pub fn add_reading(&mut self, token: AnalyzedToken) {
        if self.readings.last().is_some_and(|r| r.pos_tag.is_none()) {
            self.readings.pop();
        }
        self.readings.push(token);
        self.refresh_is_tagged();
    }

    /// LT `AnalyzedTokenReadings.setParagraphEnd`: appends a `PARA_END`
    /// reading (token surface, first reading's lemma) unless the token
    /// already carries one. Java adds this reading to the last token of the
    /// text (`JLanguageTool.markAsParagraphEnd`) and to a linebreak-only
    /// sentence (`getRawAnalyzedSentence`), so pattern elements like `P.*`
    /// match the paragraph end. The presence check reads the readings (like
    /// Java's `isParagraphEnd`), so a disambiguation action that rewrote the
    /// readings can restore it.
    pub fn set_paragraph_end(&mut self) {
        if self.has_pos_tag("PARA_END") {
            self.is_paragraph_end = true;
            return;
        }
        let lemma = self.readings.first().and_then(|r| r.stem.clone());
        let surface = self.surface().to_string();
        self.add_reading(AnalyzedToken::new(
            surface,
            lemma,
            Some("PARA_END".to_string()),
        ));
        self.is_paragraph_end = true;
    }

    /// LT `AnalyzedTokenReadings.isTagged()` is computed from the current
    /// readings, so every direct mutation has to refresh the stored flag.
    /// `hasNoTag` covers `null`, `SENT_END` and `PARA_END`.
    pub fn refresh_is_tagged(&mut self) {
        self.is_tagged = self.readings.iter().any(|r| {
            !matches!(
                r.pos_tag.as_deref(),
                None | Some("SENT_END") | Some("PARA_END")
            )
        });
    }
}

/// A fully analyzed sentence: token stream plus the sentence text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedSentence {
    pub text: String,
    /// Byte offset of this sentence within the full input text.
    pub offset: usize,
    pub tokens: Vec<AnalyzedTokenReadings>,
    /// Java `AnalyzedSentence.getPreDisambigTokensWithoutWhitespace`: the
    /// token stream captured after tokenizer + tagger (and the
    /// pre-disambiguation chunker), before any disambiguation step. Rules
    /// with `<pattern raw_pos="yes">` match against this view. Empty when the
    /// pipeline has no `raw_pos` rule (the snapshot is then skipped).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_disambig_tokens: Vec<AnalyzedTokenReadings>,
    /// Per-token detachment flags for `pre_disambig_tokens`. Java's
    /// `getPreDisambigTokens()` holds *references* to the same
    /// `AnalyzedTokenReadings` objects as `getTokens()`; an in-place
    /// disambiguation action (`REMOVE`/`ADD`/`ADDCHUNK`/`IMMUNIZE`/
    /// `IGNORE_SPELLING`) is therefore visible in the pre-disambiguation view,
    /// while a wrapper-replacing action (`REPLACE`/`UNIFY`/`FILTER`/
    /// `FILTERALL`, incl. the `<match>` filter) assigns a *new* object to the
    /// live slot and permanently detaches it: later in-place mutations are no
    /// longer seen by `raw_pos="yes"` rules. `true` marks a detached index.
    /// Empty means "nothing detached yet" (and is kept in sync with
    /// `pre_disambig_tokens` by the disambiguator).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_disambig_detached: Vec<bool>,
}

impl AnalyzedSentence {
    /// LT `AnalyzedSentence.getTokensWithoutWhitespace`: non-whitespace
    /// tokens plus the synthetic sentence/paragraph boundary tokens.
    pub fn tokens_without_whitespace(&self) -> Vec<&AnalyzedTokenReadings> {
        self.tokens
            .iter()
            .filter(|t| {
                !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
            })
            .collect()
    }

    /// Whether the pre-disambiguation reference at `idx` still aliases the
    /// live token (Java: no wrapper-replacing action has hit that slot yet).
    pub fn pre_disambig_is_aliased(&self, idx: usize) -> bool {
        self.pre_disambig_detached.get(idx).is_none_or(|d| !*d)
    }

    /// Java's wrapper-replacing actions detach the pre-disambiguation
    /// reference for `idx` (the frozen value stays what the live token held
    /// just before the replacement).
    pub fn detach_pre_disambig(&mut self, idx: usize) {
        if idx >= self.pre_disambig_tokens.len() {
            return;
        }
        if self.pre_disambig_detached.len() < self.pre_disambig_tokens.len() {
            self.pre_disambig_detached
                .resize(self.pre_disambig_tokens.len(), false);
        }
        self.pre_disambig_detached[idx] = true;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("data error: {0}")]
    Data(String),
    #[error("parse error in {0}: {1}")]
    Parse(String, String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_long_codes() {
        assert_eq!(Lang::from_long_code("en-GB"), Some(Lang::En));
        assert_eq!(
            Lang::from_long_code("de-DE-x-simple-language"),
            Some(Lang::De)
        );
        assert_eq!(Lang::from_long_code("fr"), Some(Lang::Fr));
        assert_eq!(Lang::from_long_code("pt-PT"), Some(Lang::Pt));
        assert_eq!(Lang::from_long_code("xx"), None);
    }

    #[test]
    fn language_info_matches_lt_names() {
        assert_eq!(Lang::En.info().long_code, "en-US");
        assert_eq!(Lang::De.info().name, "German (Germany)");
    }
}
