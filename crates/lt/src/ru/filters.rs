//! Russian XML-referenced filter classes: `DateCheckFilter`,
//! `FutureDateFilter` (via `DateFilterHelper`), `INNNumberFilter`,
//! `AdvancedSynthesizerFilter`, `RussianSuppressMisspelledSuggestionsFilter`
//! and `RussianPartialPosTagFilter` / `NoDisambiguationRussianPartialPosTagFilter`.

use std::sync::{Arc, OnceLock};

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, Suggestion};
use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::dates::{self, trim_special_characters, Ymd};
use crate::ru::RussianPipeline;

/// Environment for the Russian filters.
pub struct RuFilterEnv {
    pub tagger: Arc<lt_tagger::RussianTagger>,
    pub synth: Arc<crate::ru::RussianSynthesizerAdapter>,
    pub spelling: Option<Arc<crate::ru::spelling::RussianSpellingRule>>,
    pub today: Ymd,
    /// Filled after the pipeline (and thus the XML disambiguator) exists;
    /// `RussianPartialPosTagFilter` calls the language's default disambiguator.
    pub pipeline: Arc<OnceLock<Arc<RussianPipeline>>>,
}

fn required<'a>(ctx: &'a FilterContext, key: &str) -> Option<&'a String> {
    ctx.args.get(key)
}

// ---------------------------------------------------------------------------
// DateCheckFilter
// ---------------------------------------------------------------------------

/// `DateCheckFilter.getDayOfWeek(String)` (Java `Calendar`: Sunday = 1).
/// Unknown input makes Java throw, which rejects the match.
fn date_check_day_of_week(day_str: &str) -> Option<u32> {
    let day = day_str.to_lowercase();
    if day.starts_with("пн") || day == "понедельник" {
        return Some(2);
    }
    if day.starts_with("вт") {
        return Some(3);
    }
    if day.starts_with("ср") {
        return Some(4);
    }
    if day.starts_with("чт") || day == "четверг" {
        return Some(5);
    }
    if day == "пт" || day.starts_with("пятниц") {
        return Some(6);
    }
    if day.starts_with("сб") || day.starts_with("суббот") {
        return Some(7);
    }
    if day.starts_with("вс") || day == "воскресенье" {
        return Some(1);
    }
    None
}

/// `Calendar.getDisplayName(DAY_OF_WEEK, LONG, Locale.forLanguageTag("ru"))`.
fn ru_day_name(dow: u32) -> &'static str {
    match dow {
        1 => "воскресенье",
        2 => "понедельник",
        3 => "вторник",
        4 => "среда",
        5 => "четверг",
        6 => "пятница",
        _ => "суббота",
    }
}

/// `DateCheckFilter.getMonth` (exact forms plus the Roman numerals I–XII).
fn date_check_month(month_str: &str) -> Option<u32> {
    let mon = month_str.to_lowercase();
    let exact = |name: &str| mon == name;
    if exact("январь") || month_str == "I" || exact("января") || exact("янв") {
        return Some(1);
    }
    if exact("февраль") || month_str == "II" || exact("февраля") || exact("фев") {
        return Some(2);
    }
    if exact("март") || month_str == "III" || exact("марта") || exact("мар") {
        return Some(3);
    }
    if exact("апрель") || month_str == "IV" || exact("апреля") || exact("апр") {
        return Some(4);
    }
    if exact("май") || month_str == "V" || exact("мая") {
        return Some(5);
    }
    if exact("июнь") || month_str == "VI" || exact("июня") || exact("ин") {
        return Some(6);
    }
    if exact("июль") || month_str == "VII" || exact("июля") || exact("ил") {
        return Some(7);
    }
    if exact("август") || month_str == "VIII" || exact("августа") || exact("авг") {
        return Some(8);
    }
    if exact("сентябрь") || month_str == "IX" || exact("сентября") || exact("сен")
    {
        return Some(9);
    }
    if exact("октябрь") || month_str == "X" || exact("октября") || exact("окт") {
        return Some(10);
    }
    if exact("ноябрь") || month_str == "XI" || exact("ноября") || exact("ноя") {
        return Some(11);
    }
    if exact("декабрь") || month_str == "XII" || exact("декабря") || exact("дек") {
        return Some(12);
    }
    None
}

/// `AbstractDateCheckFilter.getDayOfMonthFromArguments` with the base
/// `getDayOfMonth` (0 for non-numeric input).
fn day_of_month_from_args(day_str: &str) -> u32 {
    let digits: String = day_str.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        0
    } else {
        digits.parse().unwrap_or(0)
    }
}

/// `AbstractDateCheckFilter.getMonthFromArguments`.
fn date_check_month_from_args(month_str: &str) -> Option<u32> {
    if month_str.chars().all(|c| c.is_ascii_digit()) && !month_str.is_empty() {
        month_str.parse::<u32>().ok().map(|m| m.saturating_sub(1))
    } else {
        date_check_month(&trim_special_characters(month_str)).map(|m| m - 1)
    }
}

/// `org.languagetool.rules.ru.DateCheckFilter` (`DATE_WEEKDAY1`).
pub struct DateCheckFilter {
    pub today: Ymd,
}

impl RuleFilter for DateCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(week_day_str) = required(ctx, "weekDay") else {
            return FilterOutcome::reject();
        };
        let Some(dow_from_string) = date_check_day_of_week(&week_day_str.replace('\u{00AD}', ""))
        else {
            return FilterOutcome::reject();
        };
        let year: i32 = match required(ctx, "year") {
            Some(y) => match y.parse() {
                Ok(v) => v,
                Err(_) => return FilterOutcome::reject(),
            },
            None => self.today.year,
        };
        let Some(month) = required(ctx, "month").and_then(|m| date_check_month_from_args(m)) else {
            return FilterOutcome::reject();
        };
        let Some(day) = required(ctx, "day").map(|d| day_of_month_from_args(d)) else {
            return FilterOutcome::reject();
        };
        let Some(dow_from_date) = dates::weekday(year, month + 1, day) else {
            return FilterOutcome::reject();
        };
        if dow_from_string == dow_from_date {
            return FilterOutcome::reject();
        }
        let message = ctx
            .message
            .replace("{realDay}", ru_day_name(dow_from_date))
            .replace("{day}", ru_day_name(dow_from_string))
            .replace("{currentYear}", &self.today.year.to_string());
        FilterOutcome {
            accepted: true,
            range: None,
            message: Some(message),
            suggestions: None,
        }
    }
}

// ---------------------------------------------------------------------------
// FutureDateFilter (DateFilterHelper)
// ---------------------------------------------------------------------------

/// `DateFilterHelper.getMonth` (`trimSpecialCharacters` + lowercase
/// `startsWith`). Unknown input makes Java throw, which rejects the match.
fn date_filter_helper_month(month_str: &str) -> Option<u32> {
    let mon = trim_special_characters(month_str).to_lowercase();
    for (prefix, month) in [
        ("янв", 1u32),
        ("фев", 2),
        ("мар", 3),
        ("апр", 4),
        ("май", 5),
        ("мая", 5),
        ("июн", 6),
        ("июл", 7),
        ("авг", 8),
        ("сен", 9),
        ("окт", 10),
        ("ноя", 11),
        ("дек", 12),
    ] {
        if mon.starts_with(prefix) {
            return Some(month);
        }
    }
    None
}

/// `org.languagetool.rules.ru.FutureDateFilter` (`INVALID_TENSE_DATE`).
pub struct FutureDateFilter {
    pub today: Ymd,
}

impl RuleFilter for FutureDateFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(year) = required(ctx, "year").and_then(|y| y.parse::<i32>().ok()) else {
            return FilterOutcome::reject();
        };
        let Some(month) = required(ctx, "month").and_then(|m| {
            if !m.is_empty() && m.chars().all(|c| c.is_ascii_digit()) {
                m.parse::<u32>().ok().map(|v| v.saturating_sub(1))
            } else {
                date_filter_helper_month(m).map(|v| v - 1)
            }
        }) else {
            return FilterOutcome::reject();
        };
        let day = required(ctx, "day")
            .map(|d| day_of_month_from_args(d))
            .unwrap_or(0);
        // invalid dates ('32.8.2014') belong to a different rule
        if !dates::is_valid(year, month + 1, day) {
            return FilterOutcome::reject();
        }
        let today = self.today;
        if (year, month, day) > (today.year, today.month, today.day) {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

// ---------------------------------------------------------------------------
// INNNumberFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ru.INNNumberFilter` (`WRONG_INN`): keep the match
/// only when the 10/12-digit checksum does not validate.
pub struct INNNumberFilter;

impl RuleFilter for INNNumberFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(inn) = required(ctx, "inn") else {
            return FilterOutcome::reject();
        };
        // `(\d*)` with `matches()`: empty or all digits; non-digits (and
        // empty) are handled by a different rule.
        if inn.is_empty() || !inn.chars().all(|c| c.is_ascii_digit()) {
            return FilterOutcome::reject();
        }
        let digits: Vec<i32> = inn
            .chars()
            .map(|c| c.to_digit(10).unwrap() as i32)
            .collect();
        match digits.len() {
            10 => {
                let mut kz1 = (digits[0] * 2
                    + digits[1] * 4
                    + digits[2] * 10
                    + digits[3] * 3
                    + digits[4] * 5
                    + digits[5] * 9
                    + digits[6] * 4
                    + digits[7] * 6
                    + digits[8] * 8)
                    % 11;
                if kz1 > 9 {
                    kz1 -= 10;
                }
                if digits[9] == kz1 {
                    FilterOutcome::reject()
                } else {
                    FilterOutcome::accept()
                }
            }
            12 => {
                let mut kz1 = (digits[0] * 7
                    + digits[1] * 2
                    + digits[2] * 4
                    + digits[3] * 10
                    + digits[4] * 3
                    + digits[5] * 5
                    + digits[6] * 9
                    + digits[7] * 4
                    + digits[8] * 6
                    + digits[9] * 8)
                    % 11;
                let mut kz2 = (digits[0] * 3
                    + digits[1] * 7
                    + digits[2] * 2
                    + digits[3] * 4
                    + digits[4] * 10
                    + digits[5] * 3
                    + digits[6] * 5
                    + digits[7] * 9
                    + digits[8] * 4
                    + digits[9] * 6
                    + digits[10] * 8)
                    % 11;
                if kz1 > 9 {
                    kz1 -= 10;
                }
                if kz2 > 9 {
                    kz2 -= 10;
                }
                if digits[10] == kz1 && digits[11] == kz2 {
                    FilterOutcome::reject()
                } else {
                    FilterOutcome::accept()
                }
            }
            _ => FilterOutcome::reject(),
        }
    }
}

// ---------------------------------------------------------------------------
// AdvancedSynthesizerFilter (AbstractAdvancedSynthesizerFilter)
// ---------------------------------------------------------------------------

struct AdvancedSynthesizerFilter {
    env: Arc<RuFilterEnv>,
}

impl AdvancedSynthesizerFilter {
    fn analyzed_token<'a>(
        &self,
        token: &'a AnalyzedTokenReadings,
        regexp: &str,
    ) -> Option<&'a AnalyzedToken> {
        let re = regex::Regex::new(&format!("^(?:{regexp})$")).ok()?;
        token
            .readings
            .iter()
            .find(|r| {
                let pos_tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
                re.is_match(pos_tag)
            })
            .or_else(|| token.readings.first())
    }

    fn composite_postag(
        &self,
        lemma_select: &str,
        postag_select: &str,
        original_postag: &str,
        desired_postag: &str,
        postag_replace: &str,
    ) -> String {
        let a_pattern = regex::Regex::new(&format!("^(?:{lemma_select})$")).ok();
        let b_pattern = regex::Regex::new(&format!("^(?:{postag_select})$")).ok();
        let mut result = postag_replace.to_string();
        let (Some(a_pattern), Some(b_pattern)) = (a_pattern, b_pattern) else {
            return result;
        };
        let (Some(a_caps), Some(b_caps)) = (
            a_pattern.captures(original_postag),
            b_pattern.captures(desired_postag),
        ) else {
            return result;
        };
        for i in 1..a_caps.len() {
            if let Some(group) = a_caps.get(i) {
                result = result.replace(&format!("\\a{i}"), group.as_str());
            }
        }
        for i in 1..b_caps.len() {
            if let Some(group) = b_caps.get(i) {
                result = result.replace(&format!("\\b{i}"), group.as_str());
            }
        }
        result
    }
}

impl RuleFilter for AdvancedSynthesizerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(postag_select) = ctx.args.get("postagSelect") else {
            return FilterOutcome::reject();
        };
        let Some(lemma_select) = ctx.args.get("lemmaSelect") else {
            return FilterOutcome::reject();
        };
        let Some(postag_from_str) = ctx.args.get("postagFrom") else {
            return FilterOutcome::reject();
        };
        let Some(lemma_from_str) = ctx.args.get("lemmaFrom") else {
            return FilterOutcome::reject();
        };
        let new_lemma = ctx.args.get("newLemma").cloned().unwrap_or_default();
        let resolve = |s: &str| -> Option<usize> {
            if s.starts_with("marker") {
                let mut pos = 0usize;
                while pos < ctx.pattern_tokens.len()
                    && ctx.pattern_tokens[pos].start_pos < ctx.match_range.start
                {
                    pos += 1;
                }
                pos += 1;
                if s.len() > 6 {
                    pos += s.replace("marker", "").parse::<usize>().ok()?;
                }
                Some(pos)
            } else {
                s.parse::<usize>().ok()
            }
        };
        let Some(postag_from) = resolve(postag_from_str) else {
            return FilterOutcome::reject();
        };
        let Some(lemma_from) = resolve(lemma_from_str) else {
            return FilterOutcome::reject();
        };
        if postag_from < 1
            || postag_from > ctx.pattern_tokens.len()
            || lemma_from < 1
            || lemma_from > ctx.pattern_tokens.len()
        {
            return FilterOutcome::reject();
        }
        let postag_replace = ctx.args.get("postagReplace");
        let lemma_token = ctx.pattern_tokens[lemma_from - 1];
        let Some(mut desired_lemma) = self
            .analyzed_token(lemma_token, lemma_select)
            .and_then(|t| t.stem.clone())
        else {
            return FilterOutcome::reject();
        };
        let original_postag = self
            .analyzed_token(lemma_token, lemma_select)
            .and_then(|t| t.pos_tag.clone())
            .unwrap_or_default();
        let Some(desired_postag) = self
            .analyzed_token(ctx.pattern_tokens[postag_from - 1], postag_select)
            .and_then(|t| t.pos_tag.clone())
        else {
            return FilterOutcome::reject();
        };
        if !new_lemma.is_empty() && !new_lemma.starts_with('_') {
            desired_lemma = new_lemma;
        }
        let desired_postag = match postag_replace {
            Some(replace) => self.composite_postag(
                lemma_select,
                postag_select,
                &original_postag,
                &desired_postag,
                replace,
            ),
            None => desired_postag,
        };
        let lemma_surface = lemma_token.surface();
        let is_word_capitalized = lt_tagger::is_capitalized_word(lemma_surface);
        let is_word_allupper = lt_tagger::is_all_uppercase(lemma_surface);
        let token = AnalyzedToken::new("", Some(desired_lemma), None);
        let replacements = self
            .env
            .synth
            .inner()
            .synthesize(&token, &desired_postag, true);
        if replacements.is_empty() {
            return FilterOutcome::accept();
        }
        let mut replacements_list: Vec<String> = Vec::new();
        let mut suggestion_used = false;
        for r in &ctx.suggestions {
            for nr in &replacements {
                if r.value.contains("{suggestion}")
                    || r.value.contains("{Suggestion}")
                    || r.value.contains("{SUGGESTION}")
                {
                    suggestion_used = true;
                }
                let mut nr = nr.clone();
                if is_word_capitalized {
                    nr = lt_tagger::uppercase_first_char(&nr);
                }
                if is_word_allupper {
                    nr = nr.to_uppercase();
                }
                let complete = r
                    .value
                    .replace("{suggestion}", &nr)
                    .replace("{Suggestion}", &lt_tagger::uppercase_first_char(&nr))
                    .replace("{SUGGESTION}", &nr.to_uppercase());
                if !replacements_list.contains(&complete) {
                    replacements_list.push(complete);
                }
            }
        }
        if !suggestion_used {
            replacements_list.extend(replacements);
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(
                replacements_list
                    .into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect(),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// RussianSuppressMisspelledSuggestionsFilter
// ---------------------------------------------------------------------------

struct RussianSuppressMisspelledSuggestionsFilter {
    env: Arc<RuFilterEnv>,
}

impl RussianSuppressMisspelledSuggestionsFilter {
    fn is_misspelled_multiword(&self, word: &str) -> bool {
        let Some(spelling) = &self.env.spelling else {
            return false;
        };
        let tokens = lt_tokenize::russian::RussianWordTokenizer::new().tokenize(word);
        tokens.iter().any(|token| spelling.is_misspelled(token))
    }
}

impl RuleFilter for RussianSuppressMisspelledSuggestionsFilter {
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
            if self.is_misspelled_multiword(&replacement.value) {
                continue;
            }
            let mut add = true;
            if suppress_postag.is_some() || filter_postag.is_some() {
                let readings = self
                    .env
                    .tagger
                    .tag(std::slice::from_ref(&replacement.value));
                let Some(atr) = readings.into_iter().next() else {
                    continue;
                };
                if let Some(re) =
                    suppress_postag.and_then(|p| regex::Regex::new(&format!("^(?:{p})$")).ok())
                {
                    if atr
                        .readings
                        .iter()
                        .any(|t| t.pos_tag.as_deref().is_some_and(|p| re.is_match(p)))
                    {
                        add = false;
                    }
                }
                if add {
                    if let Some(re) =
                        filter_postag.and_then(|p| regex::Regex::new(&format!("^(?:{p})$")).ok())
                    {
                        if !atr
                            .readings
                            .iter()
                            .any(|t| t.pos_tag.as_deref().is_some_and(|p| re.is_match(p)))
                        {
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
// PartialPosTagFilter
// ---------------------------------------------------------------------------

/// `PartialPosTagFilter` with the Russian `tag()` implementations:
/// `RussianPartialPosTagFilter` runs the tagger + the default disambiguator,
/// `NoDisambiguationRussianPartialPosTagFilter` only the tagger.
struct PartialPosTagFilter {
    env: Arc<RuFilterEnv>,
    disambiguate: bool,
}

impl PartialPosTagFilter {
    fn tag(&self, token: &str) -> Vec<AnalyzedTokenReadings> {
        let mut tokens = self
            .env
            .tagger
            .tag(std::slice::from_ref(&token.to_string()));
        if !self.disambiguate {
            return tokens;
        }
        let Some(pipeline) = self.env.pipeline.get() else {
            return tokens;
        };
        let mut sentence = AnalyzedSentence {
            text: token.to_string(),
            offset: 0,
            tokens: std::mem::take(&mut tokens),
            pre_disambig_tokens: Vec::new(),
            pre_disambig_detached: Vec::new(),
        };
        pipeline.disambiguate_hybrid(&mut sentence);
        sentence.tokens
    }
}

fn partial_tag_has_required_tag(
    tags: &[AnalyzedTokenReadings],
    required: &str,
    negate_pos: bool,
) -> bool {
    let Ok(re) = regex::Regex::new(&format!("^(?:{required})$")) else {
        return false;
    };
    let mut postag_count = 0usize;
    for tag in tags {
        for analyzed in &tag.readings {
            if let Some(pos_tag) = analyzed.pos_tag.as_deref() {
                if negate_pos {
                    postag_count += 1;
                    if re.is_match(pos_tag) {
                        return false;
                    }
                } else if re.is_match(pos_tag) {
                    return true;
                }
            }
        }
    }
    if postag_count == 0 {
        false
    } else {
        negate_pos
    }
}

impl RuleFilter for PartialPosTagFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Some(no), Some(regexp), Some(required)) = (
            ctx.args.get("no"),
            ctx.args.get("regexp"),
            ctx.args.get("postag_regexp"),
        ) else {
            return FilterOutcome::reject();
        };
        let Ok(token_pos) = no.parse::<usize>() else {
            return FilterOutcome::reject();
        };
        let negate_pos = ctx.args.contains_key("negate_pos");
        let two_groups_regexp = ctx.args.contains_key("two_groups_regexp");
        let prefix = ctx.args.get("prefix").cloned().unwrap_or_default();
        let suffix = ctx.args.get("suffix").cloned().unwrap_or_default();
        if token_pos < 1 || token_pos > ctx.pattern_tokens.len() {
            return FilterOutcome::reject();
        }
        let token = format!(
            "{prefix}{}{suffix}",
            ctx.pattern_tokens[token_pos - 1].surface()
        );
        let Ok(re) = regex::Regex::new(&format!("^(?:{regexp})$")) else {
            return FilterOutcome::reject();
        };
        let Some(caps) = re.captures(&token) else {
            return FilterOutcome::reject();
        };
        let group_count = caps.len() - 1;
        if (group_count != 1) != two_groups_regexp {
            return FilterOutcome::reject();
        }
        let mut partial_token = caps
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        if group_count == 2 {
            if let Some(second) = caps.get(2) {
                partial_token.push_str(second.as_str());
            }
        }
        let tags = self.tag(&partial_token);
        if partial_tag_has_required_tag(&tags, required, negate_pos) {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// The filter classes referenced by the Russian grammar and disambiguation
/// XML data.
pub fn russian_filter_registry(env: Arc<RuFilterEnv>) -> FilterRegistry {
    let mut builder = FilterRegistry::builder();
    builder = builder.register(
        "org.languagetool.rules.ru.DateCheckFilter",
        Arc::new(DateCheckFilter { today: env.today }),
    );
    builder = builder.register(
        "org.languagetool.rules.ru.FutureDateFilter",
        Arc::new(FutureDateFilter { today: env.today }),
    );
    builder = builder.register(
        "org.languagetool.rules.ru.INNNumberFilter",
        Arc::new(INNNumberFilter),
    );
    builder = builder.register(
        "org.languagetool.rules.ru.AdvancedSynthesizerFilter",
        Arc::new(AdvancedSynthesizerFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.ru.RussianSuppressMisspelledSuggestionsFilter",
        Arc::new(RussianSuppressMisspelledSuggestionsFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.ru.RussianPartialPosTagFilter",
        Arc::new(PartialPosTagFilter {
            env: Arc::clone(&env),
            disambiguate: true,
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.ru.NoDisambiguationRussianPartialPosTagFilter",
        Arc::new(PartialPosTagFilter {
            env,
            disambiguate: false,
        }),
    );
    builder.build()
}
