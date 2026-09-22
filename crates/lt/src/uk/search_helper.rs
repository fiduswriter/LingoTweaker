//! Port of `org.languagetool.rules.uk.SearchHelper`: token-sequence search
//! helpers used by `TokenAgreementPrepNounRule`.
#![allow(dead_code)]

use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::AnalyzedTokenReadings;

use lt_tagger::uk_helpers;

/// `SearchHelper.Condition`.
pub struct Condition {
    postag: Option<Regex>,
    lemma: Option<Regex>,
    token_pattern: Option<Regex>,
    token_str: Option<String>,
    negate: bool,
}

impl Condition {
    pub fn new() -> Self {
        Self {
            postag: None,
            lemma: None,
            token_pattern: None,
            token_str: None,
            negate: false,
        }
    }

    pub fn postag(pattern: &str) -> Self {
        let mut c = Self::new();
        c.postag = Regex::new(&format!("^(?:{pattern})$")).ok();
        c
    }

    pub fn lemma(pattern: &str) -> Self {
        let mut c = Self::new();
        c.lemma = Regex::new(&format!("^(?:{pattern})$")).ok();
        c
    }

    pub fn token_pattern(pattern: &str) -> Self {
        let mut c = Self::new();
        c.token_pattern = Regex::new(&format!("^(?:{pattern})$")).ok();
        c
    }

    pub fn token(token: &str) -> Self {
        let mut c = Self::new();
        c.token_str = Some(token.to_string());
        c
    }

    pub fn negate(mut self) -> Self {
        self.negate = true;
        self
    }

    fn matches(&self, token: &AnalyzedTokenReadings) -> bool {
        let clean = token.surface();
        let inner = (self.postag.as_ref().is_none_or(|re| {
            token.readings.iter().any(|r| {
                r.pos_tag
                    .as_deref()
                    .is_some_and(|t| re.is_match(t).unwrap_or(false))
            })
        })) && (self.lemma.as_ref().is_none_or(|re| {
            token.readings.iter().any(|r| {
                r.stem
                    .as_deref()
                    .is_some_and(|l| re.is_match(l).unwrap_or(false))
            })
        })) && (self
            .token_pattern
            .as_ref()
            .is_none_or(|re| re.is_match(clean).unwrap_or(false)))
            && (self
                .token_str
                .as_ref()
                .is_none_or(|s| s.eq_ignore_ascii_case(clean)));
        self.negate ^ inner
    }
}

impl Default for Condition {
    fn default() -> Self {
        Self::new()
    }
}

/// `SearchHelper.Match`.
pub struct Match {
    targets: Vec<Condition>,
    limit: i64,
    ignore_quotes: bool,
    ignore_inserts: bool,
    skips: Vec<Condition>,
}

impl Default for Match {
    fn default() -> Self {
        Self::new()
    }
}

impl Match {
    pub fn new() -> Self {
        Self {
            targets: Vec::new(),
            limit: -1,
            ignore_quotes: true,
            ignore_inserts: false,
            skips: Vec::new(),
        }
    }

    pub fn token_line(mut self, token_line: &str) -> Self {
        let replaced = token_line.replace(',', " ,");
        self.targets = replaced.split_whitespace().map(Condition::token).collect();
        self
    }

    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }

    pub fn ignore_inserts(mut self) -> Self {
        self.ignore_inserts = true;
        self
    }

    pub fn skip(mut self, conditions: Vec<Condition>) -> Self {
        self.skips = conditions;
        self
    }

    pub fn target(mut self, conditions: Vec<Condition>) -> Self {
        self.targets = conditions;
        self
    }

    fn can_skip(&self, token: &AnalyzedTokenReadings) -> bool {
        self.skips.is_empty() || self.skips.iter().any(|s| s.matches(token))
    }

    fn ignore_ins(&self, tokens: &[&AnalyzedTokenReadings], pos: i64, dir: i64) -> bool {
        let in_bounds = if dir > 0 {
            pos + 3 < tokens.len() as i64
        } else {
            pos - 3 > 0
        };
        in_bounds
            && tokens[pos as usize].surface() == ","
            && tokens[(pos + 2 * dir) as usize].surface() == ","
            && (uk_helpers::has_reading_pos_tag_part(
                &tokens[(pos + dir) as usize].readings,
                "insert",
            ) || uk_helpers::has_lemma(
                &tokens[(pos + dir) as usize].readings,
                &["зокрема", "відповідно"],
            ))
    }

    /// `SearchHelper.Match.mBefore`.
    pub fn m_before(&self, tokens: &[&AnalyzedTokenReadings], pos: usize) -> Option<usize> {
        let mut found_first = false;
        let mut logical = 0i64;
        let mut pos = pos as i64;
        let mut i_cond = self.targets.len() as i64 - 1;
        while i_cond >= 0 {
            if pos - 1 < i_cond {
                return None;
            }
            if self.limit > 0 && logical > self.limit {
                return None;
            }
            logical += 1;
            let current = tokens[pos as usize];
            if self.ignore_quotes
                && uk_helpers::quotes_pattern()
                    .is_match(current.surface())
                    .unwrap_or(false)
            {
                pos -= 1;
                continue;
            }
            if self.ignore_inserts && self.ignore_ins(tokens, pos, -1) {
                pos -= 3;
                continue;
            }
            if self.ignore_inserts && current.surface() == ")" {
                let mut i = pos - 1;
                while i > 0 {
                    if tokens[i as usize].surface() == "(" {
                        pos = i;
                    }
                    i -= 1;
                }
                pos -= 1;
                continue;
            }
            if !self.targets[i_cond as usize].matches(current) {
                if found_first {
                    return None;
                }
                if !self.can_skip(current) {
                    return None;
                }
                pos -= 1;
                continue;
            }
            found_first = true;
            i_cond -= 1;
            pos -= 1;
        }
        Some((pos + 1) as usize)
    }

    /// `SearchHelper.Match.mAfter`.
    pub fn m_after(&self, tokens: &[&AnalyzedTokenReadings], pos: usize) -> Option<usize> {
        self.m_after_limit(tokens, pos, self.limit)
    }

    fn m_after_limit(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        pos: usize,
        limit: i64,
    ) -> Option<usize> {
        let mut found_first = false;
        let mut logical = 0i64;
        let mut pos = pos as i64;
        let mut i_cond = 0i64;
        while i_cond < self.targets.len() as i64 {
            if pos + self.targets.len() as i64 - i_cond > tokens.len() as i64 {
                return None;
            }
            if limit > 0 && logical > limit {
                return None;
            }
            logical += 1;
            let current = tokens[pos as usize];
            if self.ignore_quotes
                && uk_helpers::quotes_pattern()
                    .is_match(current.surface())
                    .unwrap_or(false)
            {
                pos += 1;
                continue;
            }
            if self.ignore_inserts && self.ignore_ins(tokens, pos, 1) {
                pos += 3;
                continue;
            }
            if self.ignore_inserts && current.surface() == "(" {
                let mut i = pos + 1;
                while i < tokens.len() as i64 {
                    if tokens[i as usize].surface() == ")" {
                        pos = i;
                    }
                    i += 1;
                }
                pos += 1;
                continue;
            }
            if !self.targets[i_cond as usize].matches(current) {
                if found_first {
                    return None;
                }
                if !self.can_skip(current) {
                    return None;
                }
                pos += 1;
                continue;
            }
            found_first = true;
            i_cond += 1;
            pos += 1;
        }
        Some((pos - 1) as usize)
    }

    /// `SearchHelper.Match.mNow`.
    pub fn m_now(&self, tokens: &[&AnalyzedTokenReadings], pos: usize) -> Option<usize> {
        self.m_after_limit(tokens, pos, 0)
    }
}

/// `SearchHelper.QUOTES_PATTERN` re-export for callers.
pub fn quotes_pattern() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\p{Pi}\p{Pf}]$").unwrap());
    &RE
}
