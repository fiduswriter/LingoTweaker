//! English `<filter>` implementations (P1.6), keyed by the upstream Java
//! class name in `FilterRegistry`. Ports of:
//!
//! - `org.languagetool.rules.DateRangeChecker`
//! - `org.languagetool.rules.UnderlineSpacesFilter`
//! - `org.languagetool.rules.en.OrdinalSuffixFilter`
//! - `org.languagetool.rules.en.FutureDateFilter` (AbstractFutureDateFilter)
//! - `org.languagetool.rules.en.NewYearDateFilter` / `YMDNewYearDateFilter`
//! - `org.languagetool.rules.en.DateCheckFilter`
//!   (AbstractDateCheckWithSuggestionsFilter)
//! - `org.languagetool.rules.patterns.ApostropheTypeFilter`
//! - `org.languagetool.rules.patterns.RegexAntiPatternFilter`
//! - `org.languagetool.rules.en.AdverbFilter`
//! - `org.languagetool.rules.en.EnglishSuppressMisspelledSuggestionsFilter`
//!   (AbstractSuppressMisspelledSuggestionsFilter)
//! - `org.languagetool.rules.en.EnglishNumberInWordFilter`
//! - `org.languagetool.rules.en.FindSuggestionsFilter`
//!   (AbstractFindSuggestionsFilter)
//! - `org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter`
//!
//! Deviations from upstream (recorded in the internal development notes D-007):
//! - the default spelling rule's ignore/prohibit word lists are not applied;
//!   `SpellChecker::is_correct` over `en_US.dict` decides misspellings;
//! - the English "speller" of `FindSuggestionsFilter` ranks by edit distance
//!   only (the tagger dictionary carries no frequencies);
//! - `DateCheckFilter` without a `year` argument uses today's year (LT uses
//!   the current year in production too; LT's JUnit test hack pins 2014).

use std::collections::HashMap;
use std::sync::Arc;

use fancy_regex::Regex as FancyRegex;

use lt_core::{AnalyzedTokenReadings, Suggestion, TextRange};
use lt_pattern::{
    skip_corrected_reference, FilterContext, FilterOutcome, FilterRegistry, RuleFilter,
};
use lt_tokenize::EnglishWordTokenizer;

use crate::dates::{
    self, month_from_name, ordinal_suffix, trim_special_characters, weekday, weekday_from_name,
    weekday_name, Ymd,
};
use crate::multitoken::{remove_diacritics, IsMisspelled, MultitokenSpeller};

/// Services shared by the English filters (tagger, spellers, multitoken
/// speller, injected "today" for deterministic tests).
pub struct EnFilterEnv {
    pub tagger: Arc<lt_tagger::EnglishTagger>,
    /// speller over the tagger dictionary (`FindSuggestionsFilter`:
    /// `new MorfologikSpeller(\"/en/english.dict\")`, max edit distance 1)
    pub dict_speller: lt_spell::morfologik::MorfologikSpeller,
    /// en_US speller (default spelling rule suggestions)
    pub us_speller: lt_spell::SpellChecker,
    pub multitoken: MultitokenSpeller,
    pub is_misspelled: IsMisspelled,
    pub today: Ymd,
}

type Env = Arc<EnFilterEnv>;

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

/// `RuleFilter.getPosition`: resolve a `wordFrom` argument to a 0-based index
/// into `pattern_tokens`.
fn get_position(
    from_str: &str,
    pattern_tokens: &[&AnalyzedTokenReadings],
    from_pos: usize,
) -> Option<usize> {
    let i = if let Some(rest) = from_str.strip_prefix("marker") {
        let mut i = 0usize;
        while i < pattern_tokens.len()
            && (pattern_tokens[i].start_pos < from_pos || pattern_tokens[i].is_sentence_start)
        {
            i += 1;
        }
        let mut i = i + 1;
        if !rest.is_empty() {
            i += rest.parse::<usize>().ok()?;
        }
        i
    } else {
        from_str.parse::<usize>().ok()?
    };
    if i < 1 || i > pattern_tokens.len() {
        return None;
    }
    Some(i - 1)
}

fn has_pos_tag_full_match(tr: &AnalyzedTokenReadings, regex: &FancyRegex) -> bool {
    tr.readings.iter().any(|r| {
        r.pos_tag
            .as_deref()
            .map(|t| regex.is_match(t).unwrap_or(false))
            .unwrap_or(false)
    })
}

/// Compile a Java-style POS-tag regex for full-match semantics
/// (`Matcher.matches()`).
fn pos_tag_regex(pattern: &str) -> Option<FancyRegex> {
    FancyRegex::new(&format!("^(?:{pattern})$")).ok()
}

/// `AbstractSuppressMisspelledSuggestionsFilter.isMisspelled`: tokenize and
/// consult the default spelling rule per token.
fn is_misspelled_multiword(word: &str, env: &EnFilterEnv) -> bool {
    let tagged = |w: &str| env.tagger.is_tagged(w);
    let tokenizer = EnglishWordTokenizer::new(&tagged);
    for token in tokenizer.tokenize(word) {
        if (env.is_misspelled)(&token) {
            return true;
        }
    }
    false
}

fn required<'a>(ctx: &'a FilterContext, key: &str) -> Option<&'a str> {
    ctx.args.get(key).map(|s| s.as_str())
}

fn suggestions_values(values: &[String]) -> Vec<Suggestion> {
    values
        .iter()
        .map(|v| Suggestion {
            value: v.clone(),
            short_description: None,
        })
        .collect()
}

fn is_all_uppercase(s: &str) -> bool {
    !s.chars().any(|c| c.is_alphabetic() && c.is_lowercase())
}

fn is_capitalized_word(s: &str) -> bool {
    lt_tagger::is_capitalized_word(s)
}

fn uppercase_first_char(s: &str) -> String {
    lt_tagger::uppercase_first_char(s)
}

/// `StringTools.preserveCase`.
fn preserve_case(input: &str, model: &str) -> String {
    if model.is_empty() {
        return input.to_string();
    }
    if is_capitalized_word(model) {
        return uppercase_first_char(&input.to_lowercase());
    }
    if is_all_uppercase(model) {
        return input.to_uppercase();
    }
    input.to_string()
}

// ---------------------------------------------------------------------------
// DateRangeChecker
// ---------------------------------------------------------------------------

pub struct DateRangeCheckerFilter;

impl RuleFilter for DateRangeCheckerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Ok(x), Ok(y)) = (
            required(ctx, "x").unwrap_or("").parse::<i64>(),
            required(ctx, "y").unwrap_or("").parse::<i64>(),
        ) else {
            // something's fishy with the number – not a date range
            return FilterOutcome::reject();
        };
        if x >= y {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

// ---------------------------------------------------------------------------
// ShortenedYearRangeChecker
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ShortenedYearRangeChecker`: accepts when a
/// shortened year range such as `1998-92` is valid, i.e. the starting year is
/// not before the end year (the end year is prefixed with the first two
/// digits of the start year).
pub struct ShortenedYearRangeCheckerFilter;

impl RuleFilter for ShortenedYearRangeCheckerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Some(x_str), Some(y_str)) = (required(ctx, "x"), required(ctx, "y")) else {
            return FilterOutcome::reject();
        };
        // `Integer.parseInt(x)` / `Integer.parseInt(prefix + y)`; any parse
        // failure means "not a date range".
        let Ok(x) = x_str.parse::<i64>() else {
            return FilterOutcome::reject();
        };
        let Some(prefix) = x_str.get(..2) else {
            return FilterOutcome::reject();
        };
        let Ok(y) = format!("{prefix}{y_str}").parse::<i64>() else {
            return FilterOutcome::reject();
        };
        if x >= y {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

// ---------------------------------------------------------------------------
// UnderlineSpacesFilter
// ---------------------------------------------------------------------------

pub struct UnderlineSpacesFilter;

impl RuleFilter for UnderlineSpacesFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(mode) = required(ctx, "underlineSpaces") else {
            return FilterOutcome::reject();
        };
        let sentence = ctx.sentence_text;
        let mut range = ctx.match_range;
        if mode == "before" || mode == "both" {
            if let Some(c) = sentence[..range.start].chars().last() {
                if c.is_whitespace() {
                    range.start -= c.len_utf8();
                }
            }
        }
        if mode == "after" || mode == "both" {
            if let Some(c) = sentence[range.end..].chars().next() {
                if c.is_whitespace() {
                    range.end += c.len_utf8();
                }
            }
        }
        FilterOutcome {
            accepted: true,
            range: Some(range),
            message: None,
            suggestions: None,
        }
    }
}

// ---------------------------------------------------------------------------
// OrdinalSuffixFilter
// ---------------------------------------------------------------------------

pub struct OrdinalSuffixFilter;

impl RuleFilter for OrdinalSuffixFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(first) = ctx.suggestions.first() else {
            return FilterOutcome::accept();
        };
        let digits: String = first.value.chars().filter(|c| c.is_ascii_digit()).collect();
        let number: u32 = digits.parse().unwrap_or(0);
        let suffix = ordinal_suffix(number);
        let mut suggestions = ctx.suggestions.clone();
        suggestions[0] = Suggestion {
            value: format!("{digits}{suffix}"),
            short_description: None,
        };
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(suggestions),
        }
    }
}

// ---------------------------------------------------------------------------
// FutureDateFilter (AbstractFutureDateFilter)
// ---------------------------------------------------------------------------

/// Parse the `day` argument: digits (possibly with ordinal suffix) or a
/// localized name (English has no `getDayOfMonth` override → 0).
fn parse_day_arg(day_str: &str) -> Option<u32> {
    let digits: String = day_str.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || day_str[digits.len()..].chars().any(|c| c.is_ascii_digit()) {
        // letters-only names are not localized in English
        if digits.is_empty() && day_str.chars().all(|c| !c.is_ascii_digit()) {
            return Some(0);
        }
        return None;
    }
    digits.parse().ok()
}

fn parse_month_arg(month_str: &str) -> Option<u32> {
    let trimmed = trim_special_characters(month_str);
    if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_numeric()) {
        trimmed.parse().ok()
    } else {
        month_from_name(month_str)
    }
}

fn parse_year_arg(year_str: &str) -> Option<i32> {
    year_str.parse().ok()
}

pub struct FutureDateFilter {
    env: Env,
}

impl RuleFilter for FutureDateFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Some(year), Some(month), Some(day)) = (
            required(ctx, "year").and_then(parse_year_arg),
            required(ctx, "month").and_then(parse_month_arg),
            required(ctx, "day").and_then(parse_day_arg),
        ) else {
            return FilterOutcome::reject();
        };
        // invalid dates ('32.8.2014') belong to a different rule
        if !dates::is_valid(year, month, day) {
            return FilterOutcome::reject();
        }
        let today = self.env.today;
        if (year, month, day) > (today.year, today.month, today.day) {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

// ---------------------------------------------------------------------------
// NewYearDateFilter / YMDNewYearDateFilter (AbstractNewYearDateFilter)
// ---------------------------------------------------------------------------

fn new_year_accept(today: &Ymd, year: i32, month: u32, message: &str) -> FilterOutcome {
    // isJanuary && not December && the text's year is last year
    if today.month == 1 && month != 12 && year + 1 == today.year {
        let message = message
            .replace("{year}", &year.to_string())
            .replace("{realYear}", &today.year.to_string());
        // Java builds a new `RuleMatch` from the rewritten message, so the
        // suggestions are the inline `<suggestion>` blocks again.
        let suggestions = crate::wordutil::suggestion_tags(&message)
            .into_iter()
            .map(|value| Suggestion {
                value,
                short_description: None,
            })
            .collect();
        FilterOutcome {
            accepted: true,
            range: None,
            message: Some(message),
            suggestions: Some(suggestions),
        }
    } else {
        FilterOutcome::reject()
    }
}

pub struct NewYearDateFilter {
    env: Env,
}

impl RuleFilter for NewYearDateFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Some(year), Some(month), Some(day)) = (
            required(ctx, "year").and_then(parse_year_arg),
            required(ctx, "month").and_then(parse_month_arg),
            required(ctx, "day").and_then(parse_day_arg),
        ) else {
            return FilterOutcome::reject();
        };
        if !dates::is_valid(year, month, day) {
            return FilterOutcome::reject();
        }
        new_year_accept(&self.env.today, year, month, &ctx.message)
    }
}

pub struct YmdNewYearDateFilter {
    env: Env,
}

impl RuleFilter for YmdNewYearDateFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        if ctx.args.contains_key("year")
            || ctx.args.contains_key("month")
            || ctx.args.contains_key("day")
        {
            return FilterOutcome::reject();
        }
        let Some(date) = required(ctx, "date") else {
            return FilterOutcome::reject();
        };
        let parts: Vec<&str> = date.split('-').collect();
        let (Some(year), Some(month), Some(day)) = (
            parts.first().and_then(|s| parse_year_arg(s)),
            parts.get(1).and_then(|s| s.parse::<u32>().ok()),
            parts.get(2).and_then(|s| s.parse::<u32>().ok()),
        ) else {
            return FilterOutcome::reject();
        };
        if !dates::is_valid(year, month, day) {
            return FilterOutcome::reject();
        }
        let message = ctx
            .message
            .replace("{realDate}", &format!("{}-{}-{}", year + 1, month, day));
        new_year_accept(&self.env.today, year, month, &message)
    }
}

// ---------------------------------------------------------------------------
// DateCheckFilter (AbstractDateCheckWithSuggestionsFilter)
// ---------------------------------------------------------------------------

pub struct DateCheckFilter {
    env: Env,
}

impl RuleFilter for DateCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(week_day) = required(ctx, "weekDay").and_then(|s| s.parse::<i64>().ok()) else {
            return FilterOutcome::reject();
        };
        let Some(day_of_week_pos) =
            usize::try_from(skip_corrected_reference(ctx.token_positions, week_day))
                .ok()
                .filter(|&p| p < ctx.pattern_tokens.len())
        else {
            return FilterOutcome::reject();
        };
        let day_of_week_str = trim_special_characters(
            &ctx.pattern_tokens[day_of_week_pos]
                .surface()
                .replace('\u{00AD}', ""),
        );
        let full_date_pos = ctx
            .args
            .get("date")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(-1);

        let (day_pos, _month_pos, year_pos, day_str, month_str, year_str, is_full_date_token) =
            if full_date_pos > -1 {
                let Some(full_pos) =
                    usize::try_from(skip_corrected_reference(ctx.token_positions, full_date_pos))
                        .ok()
                        .filter(|&p| p < ctx.pattern_tokens.len())
                else {
                    return FilterOutcome::reject();
                };
                let parts: Vec<&str> = ctx.pattern_tokens[full_pos].surface().split('-').collect();
                if parts.len() != 3 {
                    return FilterOutcome::reject();
                }
                (
                    full_pos,
                    full_pos,
                    Some(full_pos),
                    parts[2],
                    parts[1],
                    Some(parts[0].to_string()),
                    true,
                )
            } else {
                let resolve = |ref_no: i64| -> Option<usize> {
                    usize::try_from(skip_corrected_reference(ctx.token_positions, ref_no))
                        .ok()
                        .filter(|&p| p < ctx.pattern_tokens.len())
                };
                let Some(day_pos) = required(ctx, "day")
                    .and_then(|s| s.parse::<i64>().ok())
                    .and_then(resolve)
                else {
                    return FilterOutcome::reject();
                };
                let Some(month_pos) = required(ctx, "month")
                    .and_then(|s| s.parse::<i64>().ok())
                    .and_then(resolve)
                else {
                    return FilterOutcome::reject();
                };
                let year_pos = ctx
                    .args
                    .get("year")
                    .and_then(|s| s.parse::<i64>().ok())
                    .and_then(resolve);
                let year_str = year_pos.map(|p| ctx.pattern_tokens[p].surface().to_string());
                (
                    day_pos,
                    month_pos,
                    year_pos,
                    ctx.pattern_tokens[day_pos].surface(),
                    ctx.pattern_tokens[month_pos].surface(),
                    year_str,
                    false,
                )
            };

        let Some(day_of_week_from_string) = weekday_from_name(&day_of_week_str) else {
            return FilterOutcome::reject();
        };
        // day of month may carry a suffix ("22nd"); letters-only names are
        // not localized in English (getDayOfMonth → 0)
        let day_digits: String = day_str.chars().take_while(|c| c.is_ascii_digit()).collect();
        let day: u32 = if day_digits.is_empty() {
            0
        } else {
            day_digits.parse().unwrap_or(0)
        };
        let Some(month) = parse_month_arg(month_str) else {
            return FilterOutcome::reject();
        };
        let year: i32 = match &year_str {
            Some(y) => match y.parse() {
                Ok(v) => v,
                Err(_) => return FilterOutcome::reject(),
            },
            None => self.env.today.year,
        };
        let Some(day_of_week_from_date) = weekday(year, month, day) else {
            return FilterOutcome::reject();
        };
        if day_of_week_from_string == day_of_week_from_date {
            return FilterOutcome::reject();
        }

        let today = self.env.today;
        // suggest changing the year (to the current year)
        if weekday(today.year, month, day) == Some(day_of_week_from_string) {
            let message = format!("This date is wrong. Did you mean \"{}\"?", today.year);
            let Some(year_pos) = year_pos else {
                return FilterOutcome::reject();
            };
            let start = ctx.pattern_tokens[year_pos].start_pos;
            let end = token_end(ctx.pattern_tokens[year_pos]);
            let suggestion = if is_full_date_token {
                format!("{}-{}-{}", today.year, month_str, day_str)
            } else {
                today.year.to_string()
            };
            return FilterOutcome {
                accepted: true,
                range: Some(TextRange::new(start, end.max(start))),
                message: Some(message),
                suggestions: Some(vec![Suggestion {
                    value: suggestion,
                    short_description: None,
                }]),
            };
        }

        // suggest changing the day of week or the day of month
        let real_day_name = weekday_name(day_of_week_from_date);
        let claimed_day_name = weekday_name(day_of_week_from_string);
        let message = ctx
            .message
            .replace("{realDay}", real_day_name)
            .replace("{day}", claimed_day_name)
            .replace("{currentYear}", &today.year.to_string());
        let (start_index, end_index) = if day_of_week_pos < day_pos {
            (day_of_week_pos, day_pos)
        } else {
            (day_pos, day_of_week_pos)
        };
        let range = TextRange::new(
            ctx.pattern_tokens[start_index].start_pos,
            token_end(ctx.pattern_tokens[end_index]),
        );

        // suggestion 1: change the day of week
        let mut sugg1 = String::new();
        for (j, tok) in ctx.pattern_tokens[start_index..=end_index]
            .iter()
            .enumerate()
        {
            let j = start_index + j;
            if j > start_index && tok.whitespace_before {
                sugg1.push(' ');
            }
            if j == day_of_week_pos {
                sugg1.push_str(&preserve_case(real_day_name, &day_of_week_str));
            } else {
                sugg1.push_str(tok.surface());
            }
        }
        let mut suggestions: Vec<Suggestion> = Vec::new();
        if !sugg1.is_empty() {
            suggestions.push(Suggestion {
                value: sugg1,
                short_description: None,
            });
        }

        // suggestion 2: change the day of month
        if let Some(corrected_day) =
            find_new_day_of_month(day, month, year, day_of_week_from_string)
        {
            let corrected_day_str = if is_full_date_token {
                format!(
                    "{}-{}-{}",
                    year_str.as_deref().unwrap_or(""),
                    month_str,
                    corrected_day
                )
            } else if day_str.chars().all(|c| c.is_numeric()) {
                corrected_day.to_string()
            } else {
                format!("{}{}", corrected_day, ordinal_suffix(corrected_day))
            };
            let mut sugg2 = String::new();
            for (j, tok) in ctx.pattern_tokens[start_index..=end_index]
                .iter()
                .enumerate()
            {
                let j = start_index + j;
                if j > start_index && tok.whitespace_before {
                    sugg2.push(' ');
                }
                if j == day_pos {
                    sugg2.push_str(&corrected_day_str);
                } else {
                    sugg2.push_str(tok.surface());
                }
            }
            if !sugg2.is_empty() {
                suggestions.push(Suggestion {
                    value: sugg2,
                    short_description: None,
                });
            }
        }

        FilterOutcome {
            accepted: true,
            range: Some(range),
            message: Some(message),
            suggestions: Some(suggestions),
        }
    }
}

fn token_end(tr: &AnalyzedTokenReadings) -> usize {
    tr.end_pos()
}

fn find_new_day_of_month(day: u32, month: u32, year: i32, target_dow: u32) -> Option<u32> {
    for diff in 1u32..7 {
        if day > diff && weekday(year, month, day - diff) == Some(target_dow) {
            return Some(day - diff);
        }
        if day + diff < 32 && weekday(year, month, day + diff) == Some(target_dow) {
            return Some(day + diff);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// ApostropheTypeFilter
// ---------------------------------------------------------------------------

pub struct ApostropheTypeFilter;

impl RuleFilter for ApostropheTypeFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(word_from) = required(ctx, "wordFrom") else {
            return FilterOutcome::reject();
        };
        let has_typographic = required(ctx, "hasTypographicalApostrophe")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let pos_word = if word_from == "marker" {
            let mut pos_word = 0usize;
            while pos_word < ctx.pattern_tokens.len()
                && ctx.pattern_tokens[pos_word].start_pos < ctx.match_range.start
            {
                pos_word += 1;
            }
            pos_word + 1
        } else {
            match word_from.parse::<usize>() {
                Ok(v) => v,
                Err(_) => return FilterOutcome::reject(),
            }
        };
        if pos_word < 1 || pos_word > ctx.pattern_tokens.len() {
            return FilterOutcome::reject();
        }
        let atr_word = ctx.pattern_tokens[pos_word - 1];
        if has_typographic == atr_word.has_typographic_apostrophe {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

// ---------------------------------------------------------------------------
// RegexAntiPatternFilter
// ---------------------------------------------------------------------------

pub struct RegexAntiPatternFilter;

impl RuleFilter for RegexAntiPatternFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(anti_pattern_str) = required(ctx, "antipatterns") else {
            return FilterOutcome::reject();
        };
        let text = ctx.sentence_text;
        for anti_pattern in anti_pattern_str.split('|') {
            let Ok(re) = FancyRegex::new(anti_pattern) else {
                continue;
            };
            for m in re.find_iter(text).flatten() {
                let (start, end) = (m.start(), m.end());
                // partial overlap is enough to filter out a match
                if (start <= ctx.match_range.end && end >= ctx.match_range.end)
                    || (start <= ctx.match_range.start && end >= ctx.match_range.start)
                {
                    return FilterOutcome::reject();
                }
            }
        }
        FilterOutcome::accept()
    }
}

// ---------------------------------------------------------------------------
// AdverbFilter
// ---------------------------------------------------------------------------

pub struct AdverbFilter {
    adverb2adj: HashMap<&'static str, &'static str>,
}

impl AdverbFilter {
    fn map() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            // irregular ones:
            ("well", "good"),
            ("fast", "fast"),
            ("hard", "hard"),
            ("late", "late"),
            ("early", "early"),
            ("daily", "daily"),
            ("straight", "straight"),
            // regular ones (and the special cases at the end):
            ("simply", "simple"),
            ("cheaply", "cheap"),
            ("quickly", "quick"),
            ("slowly", "slow"),
            ("easily", "easy"),
            ("angrily", "angry"),
            ("happily", "happy"),
            ("luckily", "lucky"),
            ("terribly", "terrible"),
            ("tragically", "tragic"),
            ("economically", "economic"),
            ("greatly", "great"),
            ("highly", "high"),
            ("generally", "general"),
            ("differently", "different"),
            ("rightly", "right"),
            ("largely", "large"),
            ("really", "real"),
            ("philosophically", "philosophical"),
            ("directly", "direct"),
            ("clearly", "clear"),
            ("merely", "mere"),
            ("exactly", "exact"),
            ("recently", "recent"),
            ("rapidly", "rapid"),
            ("suddenly", "sudden"),
            ("extremely", "extreme"),
            ("properly", "proper"),
            ("politically", "political"),
            ("probably", "probable"),
            ("self-consciously", "self-conscious"),
            ("successfully", "successful"),
            ("unusually", "unusual"),
            ("obviously", "obvious"),
            ("currently", "current"),
            ("residentially", "residential"),
            ("fully", "full"),
            ("accidentally", "accidental"),
            ("medicinally", "medicinal"),
            ("automatically", "automatic"),
            ("completely", "complete"),
            ("chronologically", "chronological"),
            ("accurately", "accurate"),
            ("necessarily", "necessary"),
            ("temporarily", "temporary"),
            ("significantly", "significant"),
            ("hastily", "hasty"),
            ("immediately", "immediate"),
            ("rarely", "rare"),
            ("totally", "total"),
            ("literally", "literal"),
            ("gently", "gentle"),
            ("finally", "final"),
            ("increasingly", "increasing"),
            ("decreasingly", "decreasing"),
            ("considerably", "considerable"),
            ("effectively", "effective"),
            ("briefly", "brief"),
            ("exceedingly", "exceeding"),
            ("physically", "physical"),
            ("enthusiastically", "enthusiastic"),
            ("incredibly", "incredible"),
            ("permanently", "permanent"),
            ("entirely", "entire"),
            ("surely", "sure"),
            ("positively", "positive"),
            ("negatively", "negative"),
            ("devastatingly", "devastating"),
            ("relatively", "relative"),
            ("absolutely", "absolute"),
            ("socially", "social"),
            ("industriously", "industrious"),
            ("solely", "sole"),
            ("asynchronously", "asynchronous"),
            ("fortunately", "fortunate"),
            ("unfortunately", "unfortunate"),
            ("ideally", "ideal"),
            ("privately", "private"),
            ("unreasonably", "unreasonable"),
            ("personally", "personal"),
            ("basically", "basic"),
            ("definitely", "definite"),
            ("potentially", "potential"),
            ("manually", "manual"),
            ("continuously", "continuous"),
            ("sadly", "sad"),
            ("eventually", "eventual"),
            ("possibly", "possible"),
            ("visually", "visual"),
            ("predominantly", "predominant"),
            ("predominately", "predominant"),
            ("quietly", "quiet"),
            ("slightly", "slight"),
            ("cleverly", "clever"),
            ("roughly", "rough"),
            ("environmentally", "environmental"),
            ("geographically", "geographical"),
            ("usually", "usual"),
            ("normally", "normal"),
            ("deliciously", "delicious"),
            ("steadily", "steady"),
            ("actively", "active"),
            ("schematically", "schematic"),
            ("mindfully", "mindful"),
            ("statistically", "statistical"),
            ("culturally", "cultural"),
            ("vicariously", "vicarious"),
            ("vividly", "vivid"),
            ("partially", "partial"),
            ("seriously", "serious"),
            ("non-verbally", "non-verbal"),
            ("nonverbally", "nonverbal"),
            ("verbally", "verbal"),
            ("shortly", "short"),
            ("mildly", "mild"),
            ("secretly", "secret"),
            ("especially", "especial"),
            ("specially", "special"),
            ("previously", "previous"),
            ("whitely", "white"),
            ("traditionally", "traditional"),
            ("individually", "individual"),
            ("carefully", "careful"),
            ("essentially", "essential"),
            ("originally", "original"),
            ("alarmingly", "alarming"),
            ("newly", "new"),
            ("wrongfully", "wrongful"),
            ("structurally", "structural"),
            ("globally", "global"),
            ("pacifically", "pacific"),
            ("seemingly", "seeming"),
            ("seamlessly", "seamless"),
            ("sustainably", "sustainable"),
            ("momentarily", "momentary"),
            ("coldly", "cold"),
            ("densely", "dense"),
            ("grimly", "grim"),
            ("calmly", "calm"),
            ("racially", "racial"),
            ("widely", "wide"),
            ("heavily", "heavy"),
            ("authentically", "authentic"),
            ("honestly", "honest"),
            ("desperately", "desperate"),
            ("immensely", "immense"),
            ("apparently", "apparent"),
            ("straightforwardly", "straightforward"),
            ("anatomically", "anatomical"),
            ("uniquely", "unique"),
            ("systemically", "systemic"),
            ("jokily", "jokey"),
            ("critically", "critical"),
            ("equally", "equal"),
            ("strongly", "strong"),
            ("purposely", "intentional"),
            ("thoroughly", "thorough"),
            ("outwardly", "outward"),
            ("horizontally", "horizontal"),
            ("vertically", "vertical"),
            ("technically", "technical"),
            ("swiftly", "swift"),
            ("accessibly", "accessible"),
            ("occasionally", "occasional"),
            ("specifically", "specific"),
            ("subtly", "subtle"),
            ("actually", "actual"),
            ("particularly", "particular"),
            ("gloomily", "gloomy"),
            ("nicely", "nice"),
            ("progressively", "progressive"),
            ("genuinely", "genuine"),
            ("characteristically", "characteristic"),
            ("deeply", "deep"),
            ("spiritually", "spiritual"),
            ("purely", "pure"),
            ("satisfyingly", "satisfying"),
            ("indolently", "indolent"),
            ("obliquely", "oblique"),
            ("preferably", "preferable"),
            ("oddly", "odd"),
            ("professionally", "professional"),
            ("indispensably", "indispensable"),
            ("dispensably", "dispensable"),
            ("consistently", "consistent"),
            ("truly", "true"),
            ("commonly", "common"),
            ("safely", "safe"),
            ("evolutionarily", "evolutionary"),
            ("internally", "internal"),
            ("magically", "magical"),
            ("annually", "annual"),
            ("brightly", "bright"),
            ("officially", "official"),
            ("inofficially", "inofficial"),
            ("perfectly", "perfect"),
            ("overly", "over"),
            ("tropically", "tropical"),
            ("brilliantly", "brilliant"),
            ("exclusively", "exclusive"),
            ("commercially", "commercial"),
            ("mischievously", "mischievous"),
            ("weirdly", "weird"),
            ("routinely", "routine"),
            ("gruffly", "gruff"),
            ("naturally", "natural"),
            ("lightly", "light"),
            ("haphazardly", "haphazard"),
            ("lovingly", "loving"),
            ("sagely", "sage"),
            ("systematically", "systematical"),
            ("academically", "academical"),
            ("jokingly", "joking"),
            ("primarily", "primary"),
            ("secondarily", "secondary"),
            ("peacefully", "peaceful"),
            ("thankfully", "thankful"),
            ("reliably", "reliable"),
            ("unreliably", "unreliable"),
            ("infinitesimally", "infinitesimal"),
            ("hugely", "huge"),
            ("strictly", "strict"),
            ("morally", "moral"),
            ("involuntarily", "involuntary"),
            ("voluntarily", "voluntary"),
            ("vanishingly", "vanishing"),
            ("typically", "typical"),
            ("playfully", "playful"),
            ("wonderfully", "wonderful"),
            ("roguishly", "roguish"),
            ("emotionally", "emotional"),
            ("efficiently", "efficient"),
            ("unkindly", "unkind"),
            ("mentally", "mental"),
            ("credibly", "credible"),
            ("seductively", "seductive"),
            ("rashly", "rash"),
            ("outwards", "outward"),
            ("periodically", "periodical"),
            ("comparatively", "comparative"),
            ("confidentially", "confidential"),
            ("dominantly", "dominant"),
            ("forcibly", "forcible"),
            ("formerly", "former"),
            ("financially", "financial"),
            ("urgently", "urgent"),
            ("inherently", "inherent"),
            ("historically", "historical"),
            ("tightly", "tight"),
            ("greedily", "greedy"),
            ("fluently", "fluent"),
            ("ordinarily", "ordinary"),
            ("inevitably", "inevitable"),
            ("partly", "partial"),
            ("liquidly", "liquid"),
            ("supremely", "supreme"),
            ("initially", "initial"),
            ("unjustly", "unjust"),
            ("justly", "just"),
            ("plausibly", "plausible"),
            ("amiably", "amiable"),
            ("massively", "massive"),
            ("lowly", "low"),
            ("notoriously", "notorious"),
            ("meaningfully", "meaningful"),
            ("approximately", "approximate"),
            ("extraordinarily", "extraordinary"),
            ("warmly", "warm"),
            ("nearly", "near"),
            ("strategically", "strategical"),
            ("endlessly", "endless"),
            ("virtually", "virtual"),
            ("regularly", "regular"),
            ("deliberately", "deliberate"),
            ("reasonably", "reasonable"),
            ("similarly", "similar"),
            ("flexibly", "flexible"),
            ("softly", "soft"),
            ("responsibly", "responsible"),
            ("irresponsibly", "irresponsible"),
            ("sweetly", "sweet"),
            ("comfortably", "comfortable"),
            ("uncomfortably", "uncomfortable"),
            ("intricately", "intricate"),
            ("unnecessarily", "unnecessary"),
            ("obstinately", "obstinate"),
            ("reportedly", "reported"),
            ("loosely", "loose"),
            ("profusely", "profuse"),
            ("mortally", "mortal"),
            ("dynamically", "dynamical"),
            ("illegally", "illegal"),
            ("legally", "legal"),
            ("undoubtedly", "undoubted"),
            ("humanly", "human"),
            ("likewise", "similar"),
            ("intrinsically", "intrinsic"),
            ("substantially", "substantial"),
            ("suspiciously", "suspicious"),
            ("generationally", "generational"),
            ("loudly", "loud"),
            ("moderately", "moderate"),
            ("gravely", "grave"),
            ("temporally", "temporal"),
            ("digitally", "digital"),
            ("finely", "fine"),
            ("respectfully", "respectful"),
            ("questioningly", "questioning"),
            ("diagonally", "diagonal"),
            ("additionally", "additional"),
            ("sexually", "sexual"),
            ("remarkably", "remarkable"),
            ("acutely", "acute"),
            ("linearly", "linear"),
            ("perfunctorily", "perfunctory"),
            ("unbelievably", "unbelievable"),
            ("merrily", "merry"),
            ("beneath", "below"),
            ("lest", "least"),
            ("either", "other"),
            ("nasally", "nasal"),
            ("concretely", "concrete"),
            ("intuitively", "intuitive"),
            ("please", "pleasing"),
            ("intermediately", "intermediate"),
            ("powerfully", "powerful"),
            ("fairly", "fair"),
            ("wholly", "whole"),
            ("keenly", "keen"),
            ("unconsciously", "unconscious"),
            ("consciously", "conscious"),
            ("humanely", "humane"),
            ("honorably", "honorable"),
            ("rudely", "rude"),
            ("incorrectly", "incorrect"),
            ("correctly", "correct"),
            ("mistakenly", "mistaken"),
            ("wrongly", "wrong"),
            ("morosely", "morose"),
            ("worryingly", "worrying"),
            ("drastically", "drastical"),
            ("willingly", "willing"),
            ("additively", "additive"),
            ("drolly", "droll"),
            ("statically", "statical"),
            ("hopefully", "hopeful"),
            ("untruthfully", "untruthful"),
            ("truthfully", "truthful"),
            ("attractively", "attractive"),
            ("supposedly", "supposed"),
            ("overwhelmingly", "overwhelming"),
            ("imperfectly", "imperfect"),
            ("deftly", "deft"),
            ("wildly", "wild"),
            ("sheepishly", "sheepish"),
            ("hotly", "hot"),
            ("genetically", "genetic"),
            ("inexplicably", "inexplicable"),
            ("explicably", "explicable"),
            ("domestically", "domestical"),
            ("invisibly", "invisible"),
            ("visibly", "visible"),
            ("noteworthily", "noteworthy"),
            ("unexpectably", "unexpectable"),
            ("expectably", "expectable"),
            ("foreseeably", "foreseeable"),
            ("unforeseeably", "unforeseeable"),
            ("distinctly", "distinct"),
            ("unequivocally", "unequivocal"),
            ("signally", "signal"),
            ("medically", "medical"),
            ("certainly", "certain"),
            ("beautifully", "beautiful"),
            ("firmly", "firm"),
            ("electrically", "electrical"),
            ("gradually", "gradual"),
            ("grossly", "gross"),
            ("memorably", "memorable"),
            ("unmemorably", "unmemorable"),
            ("shelly", "shell"),
            ("strangely", "strange"),
            ("unhealthily", "unhealthy"),
            ("healthily", "healthy"),
            ("harshly", "harsh"),
            ("proudly", "proud"),
            ("lately", "late"),
            ("remotely", "remote"),
            ("longly", "long"),
            ("politely", "polite"),
            ("ethically", "ethical"),
            ("noticeably", "noticeable"),
            ("unnoticeably", "unnoticeable"),
            ("consequently", "consequent"),
            ("snugly", "snug"),
            ("mainly", "main"),
            ("popularly", "popular"),
            ("improperly", "improper"),
            ("deliverly", "delivery"),
            ("rushingly", "rushing"),
            ("gravitationally", "gravitational"),
            ("cruelly", "cruel"),
            ("optimally", "optimal"),
            ("fictionally", "fictional"),
            ("manageably", "manageable"),
            ("unmanageably", "unmanageable"),
            ("fashionably", "fashionable"),
            ("secondly", "second"),
            ("thirdly", "third"),
            ("curtly", "curt"),
            ("secretively", "secretive"),
            ("surprisingly", "surprising"),
            ("sociologically", "sociological"),
            ("severely", "severe"),
            ("ruffianly", "ruffian"),
            ("bigly", "big"),
            ("frequently", "frequent"),
            ("irrationally", "irrational"),
            ("rationally", "rational"),
            ("riotously", "riotous"),
            ("excruciatingly", "excruciating"),
            ("intensively", "intensive"),
            ("separately", "separate"),
            ("favorably", "favorable"),
            ("favourably", "favourable"),
            ("unfavorably", "unfavorable"),
            ("unfavourably", "unfavourable"),
            ("fittingly", "fitting"),
            ("orally", "oral"),
            ("jointly", "joint"),
            ("methodically", "methodical"),
            ("ecologically", "ecological"),
            ("irrepressibly", "irrepressible"),
            ("repressibly", "repressible"),
            ("heartily", "hearty"),
            ("smoothly", "smooth"),
            ("dreamily", "dreamy"),
            ("indirectly", "indirect"),
            ("fascinatingly", "fascinating"),
            ("scientifically", "scientific"),
            ("unhappily", "unhappy"),
            ("publicly", "public"),
            ("healthfully", "healthful"),
            ("uncharacteristically", "uncharacteristic"),
            ("genially", "genial"),
            ("ineludibly", "ineludible"),
            ("tenderly", "tender"),
            ("arguably", "arguable"),
            ("comparably", "comparable"),
            ("procedurally", "procedural"),
            ("interchangeably", "interchangeable"),
            ("conceivably", "conceivable"),
            ("jokily", "joking"),
            ("resignedly", "resigned"),
            ("vehemently", "vehement"),
            ("horribly", "horrible"),
            ("teasingly", "teasing"),
            ("figuratively", "figurative"),
            ("excitingly", "exciting"),
            ("haltingly", "halting"),
            ("phonetically", "phonetic"),
            ("proverbially", "proverbial"),
            ("informally", "informal"),
            ("cozily", "cozy"),
            ("cosily", "cosy"),
            ("constantly", "constant"),
            ("rightfully", "rightful"),
            ("reluctantly", "reluctant"),
            ("externally", "external"),
            ("intellectually", "intellectual"),
            ("dramatically", "dramatic"),
            ("freshly", "fresh"),
            ("casually", "casual"),
            ("unevenly", "uneven"),
            ("enormously", "enormous"),
            ("callously", "callous"),
            ("imperiously", "imperious"),
            ("messily", "messy"),
            ("alternatively", "alternative"),
            ("gladly", "glad"),
            ("adversely", "adverse"),
            ("petulantly", "petulant"),
            ("shakily", "shaky"),
            ("menacingly", "menacing"),
            ("consensually", "consensual"),
            ("bitterly", "bitter"),
            ("terminally", "terminal"),
            ("faintly", "faint"),
            ("brusquely", "brusque"),
            ("humbly", "humble"),
            ("promptly", "prompt"),
            ("identically", "identical"),
            ("militarily", "military"),
            ("neatly", "neat"),
            ("insanely", "insane"),
            ("analytically", "analytical"),
            ("firstly", "first"),
            ("twice", "second"),
        ])
    }
}

impl AdverbFilter {
    pub fn new() -> Self {
        Self {
            adverb2adj: Self::map(),
        }
    }
}

impl Default for AdverbFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleFilter for AdverbFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Some(adverb), Some(noun)) = (ctx.args.get("adverb"), ctx.args.get("noun")) else {
            return FilterOutcome::accept();
        };
        if let Some(adjective) = self.adverb2adj.get(adverb.as_str()) {
            if *adjective != adverb.as_str() {
                return FilterOutcome {
                    accepted: true,
                    range: None,
                    message: None,
                    suggestions: Some(vec![Suggestion {
                        value: format!("{adjective} {noun}"),
                        short_description: None,
                    }]),
                };
            }
        }
        FilterOutcome::accept()
    }
}

// ---------------------------------------------------------------------------
// EnglishSuppressMisspelledSuggestionsFilter
// ---------------------------------------------------------------------------

pub struct SuppressMisspelledSuggestionsFilter {
    env: Env,
}

impl RuleFilter for SuppressMisspelledSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let suppress_match = ctx
            .args
            .get("suppressMatch")
            .map(|v| !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        let suppress_postag = ctx.args.get("SuppressPostag");
        let filter_postag = ctx.args.get("FilterPostag");
        let mut new_replacements: Vec<Suggestion> = Vec::new();
        for replacement in &ctx.suggestions {
            if is_misspelled_multiword(&replacement.value, &self.env) {
                continue;
            }
            let mut add = true;
            if suppress_postag.is_some() || filter_postag.is_some() {
                // LT tags the replacement string itself
                let readings = self.env.tagger.tag_word(&replacement.value);
                let atr = AnalyzedTokenReadings {
                    readings,
                    chunk_tags: Vec::new(),
                    whitespace_before: false,
                    start_pos: 0,
                    raw_byte_len: replacement.value.len(),
                    is_whitespace: false,
                    is_sentence_start: false,
                    is_sentence_end: false,
                    is_paragraph_end: false,
                    is_tagged: true,
                    is_immunized: false,
                    is_ignore_spelling: false,
                    has_typographic_apostrophe: false,
                    is_pos_tag_unknown: false,
                };
                if let Some(re) = suppress_postag.and_then(|p| pos_tag_regex(p)) {
                    if has_pos_tag_full_match(&atr, &re) {
                        add = false;
                    }
                }
                if add {
                    if let Some(re) = filter_postag.and_then(|p| pos_tag_regex(p)) {
                        if !has_pos_tag_full_match(&atr, &re) {
                            add = false;
                        }
                    }
                }
            }
            if add {
                new_replacements.push(replacement.clone());
            }
        }
        if new_replacements.is_empty() && suppress_match {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(new_replacements),
        }
    }
}

// ---------------------------------------------------------------------------
// EnglishNumberInWordFilter (AbstractNumberInWordFilter)
// ---------------------------------------------------------------------------

pub struct NumberInWordFilter {
    env: Env,
}

impl RuleFilter for NumberInWordFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(word) = ctx.args.get("word") else {
            return FilterOutcome::reject();
        };
        let word_replacing_zero_o = word.replace('0', "o");
        let word_without_number: String = word.chars().filter(|c| !c.is_ascii_digit()).collect();
        let mut replacements: Vec<String> = Vec::new();
        if !(self.env.is_misspelled)(&word_replacing_zero_o) && *word != word_replacing_zero_o {
            replacements.push(word_replacing_zero_o);
        }
        if !(self.env.is_misspelled)(&word_without_number) {
            replacements.push(word_without_number.clone());
        }
        if replacements.is_empty() {
            replacements.extend(self.env.us_speller.suggest(&word_without_number, 10));
        }
        if replacements.is_empty() {
            FilterOutcome::reject()
        } else {
            FilterOutcome {
                accepted: true,
                range: None,
                message: None,
                suggestions: Some(suggestions_values(&replacements)),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FindSuggestionsFilter (AbstractFindSuggestionsFilter)
// ---------------------------------------------------------------------------

const MAX_SUGGESTIONS: usize = 10;

pub struct FindSuggestionsFilter {
    env: Env,
}

impl FindSuggestionsFilter {
    fn spelling_suggestions(&self, word: &str) -> Vec<String> {
        // `MorfologikSpeller.findSimilarWords`: morfologik's
        // `findSimilarWordCandidates` (edit distance 1, includes dictionary
        // words), in candidate order
        self.env
            .dict_speller
            .speller()
            .find_similar_word_candidates(word)
            .into_iter()
            .map(|c| c.word)
            .collect()
    }
}

impl RuleFilter for FindSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(word_from) = ctx.args.get("wordFrom") else {
            return FilterOutcome::reject();
        };
        let Some(desired_postag) = ctx.args.get("desiredPostag") else {
            return FilterOutcome::reject();
        };
        let priority_postag = ctx.args.get("priorityPostag");
        let remove_suggestions_regexp = ctx.args.get("removeSuggestionsRegexp");
        let suppress_match = ctx
            .args
            .get("suppressMatch")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let mode = ctx.args.get("Mode");
        let diacritics_mode = mode.map(|m| m == "diacritics").unwrap_or(false);
        let Some(desired_re) = pos_tag_regex(desired_postag) else {
            return FilterOutcome::reject();
        };
        let priority_re = priority_postag.and_then(|p| pos_tag_regex(p));
        let regexp_pattern = remove_suggestions_regexp.and_then(|r| {
            // Java Matcher.matches() = full match
            FancyRegex::new(&format!("^(?:{r})$")).ok()
        });

        let atr_word_token: String = if word_from == "inmarker" {
            ctx.sentence_text[ctx.match_range.start..ctx.match_range.end].replace(' ', "")
        } else {
            let Some(pos) = get_position(word_from, ctx.pattern_tokens, ctx.match_range.start)
            else {
                return FilterOutcome::reject();
            };
            ctx.pattern_tokens[pos].surface().to_string()
        };
        let is_word_capitalized = is_capitalized_word(&atr_word_token);
        let is_word_all_upper = is_all_uppercase(&atr_word_token);

        // if the original token already meets the requirements, nothing to do
        for atr in self.env.tagger.tag_word(&atr_word_token) {
            if let Some(tag) = &atr.pos_tag {
                if desired_re.is_match(tag).unwrap_or(false) && diacritics_mode {
                    return FilterOutcome::reject();
                }
            }
        }

        let mut replacements: Vec<String> = Vec::new();
        // (replacements2 stays empty: English has no synthesizer here)
        let mut used_priority_pos = 0usize;
        for suggestion in self.spelling_suggestions(&atr_word_token) {
            let analyzed = self.env.tagger.tag_word(&suggestion);
            for analyzed_suggestion in &analyzed {
                if replacements.len() >= 2 * MAX_SUGGESTIONS {
                    break;
                }
                let Some(tag) = &analyzed_suggestion.pos_tag else {
                    continue;
                };
                if suggestion != atr_word_token
                    && desired_re.is_match(tag).unwrap_or(false)
                    && !replacements.contains(&suggestion)
                    && !replacements.contains(&suggestion.to_lowercase())
                {
                    let diacritics_ok = !diacritics_mode
                        || remove_diacritics(&suggestion).to_lowercase()
                            == remove_diacritics(&atr_word_token).to_lowercase();
                    if diacritics_ok {
                        let regexp_ok = regexp_pattern
                            .as_ref()
                            .map(|re| !re.is_match(&suggestion).unwrap_or(false))
                            .unwrap_or(true);
                        if regexp_ok {
                            let mut replacement = suggestion.clone();
                            if is_word_all_upper {
                                replacement = replacement.to_uppercase();
                            }
                            if is_word_capitalized {
                                replacement = uppercase_first_char(&replacement);
                            }
                            let is_priority = priority_re
                                .as_ref()
                                .map(|re| re.is_match(tag).unwrap_or(false))
                                .unwrap_or(false);
                            if is_priority {
                                replacements
                                    .insert(used_priority_pos.min(replacements.len()), replacement);
                                used_priority_pos += 1;
                            } else {
                                replacements.push(replacement);
                            }
                        }
                    }
                }
            }
        }

        let match_contains_finished_suggestion = ctx
            .suggestions
            .iter()
            .any(|s| !s.value.contains("{suggestion}"));
        if diacritics_mode && replacements.is_empty() && !match_contains_finished_suggestion {
            return FilterOutcome::reject();
        }
        if replacements.is_empty() && suppress_match && !match_contains_finished_suggestion {
            return FilterOutcome::reject();
        }

        // expand {suggestion} placeholders or append the generated list
        let mut definitive: Vec<String> = Vec::new();
        let mut replacements_used = false;
        for s in &ctx.suggestions {
            if s.value.contains("{suggestion}")
                || s.value.contains("{Suggestion}")
                || s.value.contains("{SUGGESTION}")
            {
                replacements_used = true;
                for s2 in &replacements {
                    if definitive.len() >= MAX_SUGGESTIONS {
                        break;
                    }
                    if s.value.contains("{suggestion}") {
                        let v = s.value.replace("{suggestion}", s2);
                        if !definitive.contains(&v) {
                            definitive.push(v);
                        }
                    } else if s.value.contains("{Suggestion}") {
                        let v = s.value.replace("{Suggestion}", &uppercase_first_char(s2));
                        if !definitive.contains(&v) {
                            definitive.push(v);
                        }
                    } else {
                        let v = s.value.replace("{SUGGESTION}", &s2.to_uppercase());
                        if !definitive.contains(&v) {
                            definitive.push(v);
                        }
                    }
                }
            } else if !definitive.contains(&s.value) {
                definitive.push(s.value.clone());
            }
        }
        if !replacements_used {
            for replacement in &replacements {
                if definitive.len() >= MAX_SUGGESTIONS {
                    break;
                }
                if !definitive.contains(replacement) {
                    definitive.push(replacement.clone());
                }
            }
        }

        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(suggestions_values(&definitive)),
        }
    }
}

// ---------------------------------------------------------------------------
// MultitokenSpellerFilter
// ---------------------------------------------------------------------------

fn is_punctuation_mark(s: &str) -> bool {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.is_ascii_punctuation() || c == '’' || is_unicode_punct(c),
        _ => false,
    }
}

fn is_unicode_punct(c: char) -> bool {
    // approximation of Java \p{IsPunctuation} (Pc, Pd, Ps, Pe, Pi, Pf, Po)
    matches!(
        c,
        '\u{00A1}'..='\u{00BF}' | '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{205E}'
    ) && !c.is_alphanumeric()
}

fn is_not_word_string(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| !c.is_alphabetic())
}

pub struct MultitokenSpellerFilter {
    env: Env,
}

impl RuleFilter for MultitokenSpellerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        if ctx.pattern_tokens.iter().all(|t| t.is_ignore_spelling) {
            return FilterOutcome::reject();
        }
        let underlined_error = &ctx.sentence_text[ctx.match_range.start..ctx.match_range.end];
        let are_tokens_accepted_by_speller = !is_misspelled_multiword(underlined_error, &self.env);
        let mut replacements = self
            .env
            .multitoken
            .suggestions(underlined_error, are_tokens_accepted_by_speller);
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        if underlined_error.chars().count() > 4 && is_all_uppercase(underlined_error) {
            let mut all_upper: Vec<String> = Vec::new();
            for replacement in replacements {
                let new_replacement = replacement.to_uppercase();
                if !all_upper.contains(&new_replacement) && new_replacement != underlined_error {
                    all_upper.push(new_replacement);
                }
            }
            replacements = all_upper;
        } else {
            // capitalize suggestions when the error starts the sentence
            // (LT: skip leading punctuation marks in tokensWithoutWhitespace)
            let non_blank: Vec<&AnalyzedTokenReadings> = ctx
                .sentence_tokens
                .iter()
                .copied()
                .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
                .collect();
            let mut words_start_pos = 1usize;
            while words_start_pos < non_blank.len()
                && (is_punctuation_mark(non_blank[words_start_pos].surface())
                    || is_not_word_string(non_blank[words_start_pos].surface()))
            {
                words_start_pos += 1;
            }
            if ctx.pattern_token_pos == words_start_pos {
                let mut capitalized: Vec<String> = Vec::new();
                for replacement in replacements {
                    let mut new_replacement = replacement.clone();
                    if replacement == replacement.to_lowercase() {
                        // do not capitalize iPad
                        new_replacement = uppercase_first_char(&replacement);
                    }
                    if !capitalized.contains(&new_replacement)
                        && new_replacement != underlined_error
                    {
                        capitalized.push(new_replacement);
                    }
                }
                replacements = capitalized;
            }
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(suggestions_values(&replacements)),
        }
    }
}

// ---------------------------------------------------------------------------
// registry
// ---------------------------------------------------------------------------

pub const FUTURE_DATE: &str = "org.languagetool.rules.en.FutureDateFilter";
pub const MULTITOKEN_SPELLER: &str =
    "org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter";
pub const DATE_CHECK: &str = "org.languagetool.rules.en.DateCheckFilter";
pub const UNDERLINE_SPACES: &str = "org.languagetool.rules.UnderlineSpacesFilter";
pub const ORDINAL_SUFFIX: &str = "org.languagetool.rules.en.OrdinalSuffixFilter";
pub const NEW_YEAR_DATE: &str = "org.languagetool.rules.en.NewYearDateFilter";
pub const FIND_SUGGESTIONS: &str = "org.languagetool.rules.en.FindSuggestionsFilter";
pub const SUPPRESS_MISSPELLED: &str =
    "org.languagetool.rules.en.EnglishSuppressMisspelledSuggestionsFilter";
pub const REGEX_ANTI_PATTERN: &str = "org.languagetool.rules.patterns.RegexAntiPatternFilter";
pub const APOSTROPHE_TYPE: &str = "org.languagetool.rules.patterns.ApostropheTypeFilter";
pub const DATE_RANGE_CHECKER: &str = "org.languagetool.rules.DateRangeChecker";
pub const YMD_NEW_YEAR_DATE: &str = "org.languagetool.rules.en.YMDNewYearDateFilter";
pub const NUMBER_IN_WORD: &str = "org.languagetool.rules.en.EnglishNumberInWordFilter";
pub const ADVERB: &str = "org.languagetool.rules.en.AdverbFilter";

/// Build the English filter registry over the shared environment.
pub fn english_filter_registry(env: Env) -> FilterRegistry {
    FilterRegistry::builder()
        .register(DATE_RANGE_CHECKER, Arc::new(DateRangeCheckerFilter))
        .register(UNDERLINE_SPACES, Arc::new(UnderlineSpacesFilter))
        .register(ORDINAL_SUFFIX, Arc::new(OrdinalSuffixFilter))
        .register(FUTURE_DATE, Arc::new(FutureDateFilter { env: env.clone() }))
        .register(
            NEW_YEAR_DATE,
            Arc::new(NewYearDateFilter { env: env.clone() }),
        )
        .register(
            YMD_NEW_YEAR_DATE,
            Arc::new(YmdNewYearDateFilter { env: env.clone() }),
        )
        .register(DATE_CHECK, Arc::new(DateCheckFilter { env: env.clone() }))
        .register(APOSTROPHE_TYPE, Arc::new(ApostropheTypeFilter))
        .register(REGEX_ANTI_PATTERN, Arc::new(RegexAntiPatternFilter))
        .register(
            SUPPRESS_MISSPELLED,
            Arc::new(SuppressMisspelledSuggestionsFilter { env: env.clone() }),
        )
        .register(
            NUMBER_IN_WORD,
            Arc::new(NumberInWordFilter { env: env.clone() }),
        )
        .register(
            FIND_SUGGESTIONS,
            Arc::new(FindSuggestionsFilter { env: env.clone() }),
        )
        .register(
            MULTITOKEN_SPELLER,
            Arc::new(MultitokenSpellerFilter { env: env.clone() }),
        )
        .register(ADVERB, Arc::new(AdverbFilter::new()))
        .build()
}
