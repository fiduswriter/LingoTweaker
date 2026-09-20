//! Polish rule filters referenced by `rules/pl/grammar.xml`:
//! `org.languagetool.rules.pl.DateCheckFilter` (rulegroup `DATE_WEEKDAY`),
//! `org.languagetool.rules.pl.DecadeSpellingFilter` (`DATA_DEKADY`) and the
//! generic `org.languagetool.rules.DateRangeChecker` /
//! `ShortenedYearRangeChecker`.

use std::sync::Arc;

use lt_core::Suggestion;
use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::dates::{self, trim_special_characters, Ymd};

/// `DateCheckFilter.getDayOfWeek(String)`: Sunday = 1 (Java `Calendar`).
/// Unknown input makes Java throw, which rejects the match.
fn get_day_of_week(day_str: &str) -> Option<u32> {
    let day = day_str.replace('\u{00AD}', "").to_lowercase();
    if day.starts_with("pon") {
        return Some(2);
    }
    if day.starts_with("wt") {
        return Some(3);
    }
    if day.starts_with("śr") {
        return Some(4);
    }
    if day.starts_with("czw") {
        return Some(5);
    }
    if day == "pt" || day.starts_with("piątk") || day == "piątek" {
        return Some(6);
    }
    if day.starts_with("sob") {
        return Some(7);
    }
    if day.starts_with("niedz") {
        return Some(1);
    }
    None
}

/// `DateCheckFilter.getDayOfWeek(Calendar)` (Polish long day names, as
/// returned by `Calendar.getDisplayName(DAY_OF_WEEK, LONG, Locale "pl")`).
fn day_name(dow: u32) -> &'static str {
    match dow {
        1 => "niedziela",
        2 => "poniedziałek",
        3 => "wtorek",
        4 => "środa",
        5 => "czwartek",
        6 => "piątek",
        _ => "sobota",
    }
}

/// `DateCheckFilter.getMonth` (exact Polish month names or Roman numerals).
fn get_month(month_str: &str) -> Option<u32> {
    let mon = month_str.to_lowercase();
    if mon == "stycznia" || month_str == "I" {
        return Some(1);
    }
    if mon == "lutego" || month_str == "II" {
        return Some(2);
    }
    if mon == "marca" || month_str == "III" {
        return Some(3);
    }
    if mon == "kwietnia" || month_str == "IV" {
        return Some(4);
    }
    if mon == "maja" || month_str == "V" {
        return Some(5);
    }
    if mon == "czerwca" || month_str == "VI" {
        return Some(6);
    }
    if mon == "lipca" || month_str == "VII" {
        return Some(7);
    }
    if mon == "sierpnia" || month_str == "VIII" {
        return Some(8);
    }
    if mon == "września" || month_str == "IX" {
        return Some(9);
    }
    if mon == "października" || month_str == "X" {
        return Some(10);
    }
    if mon == "listopada" || month_str == "XI" {
        return Some(11);
    }
    if mon == "grudnia" || month_str == "XII" {
        return Some(12);
    }
    None
}

/// `AbstractDateCheckFilter.getMonthFromArguments` (0-based result).
fn month_from_args(month_str: &str) -> Option<u32> {
    let ms = trim_special_characters(month_str);
    if !ms.is_empty() && ms.chars().all(|c| c.is_ascii_digit()) {
        ms.parse::<u32>().ok().map(|m| m.saturating_sub(1))
    } else {
        get_month(&ms).map(|m| m - 1)
    }
}

/// `AbstractDateCheckFilter.getDayOfMonthFromArguments`
/// (`(\d+).*`, else `getDayOfMonth` — 0 for Polish).
fn day_of_month_from_args(day_str: &str) -> Option<u32> {
    let digits: String = day_str.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        if day_str.chars().all(|c| !c.is_ascii_digit()) {
            return Some(0);
        }
        return None;
    }
    digits.parse().ok()
}

/// `org.languagetool.rules.pl.DateCheckFilter`.
pub struct DateCheckFilter {
    pub today: Ymd,
}

impl RuleFilter for DateCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(week_day_str) = ctx.args.get("weekDay") else {
            return FilterOutcome::reject();
        };
        let Some(dow_from_string) = get_day_of_week(week_day_str) else {
            return FilterOutcome::reject();
        };
        let year: i32 = match ctx.args.get("year") {
            Some(y) => match y.parse() {
                Ok(v) => v,
                Err(_) => return FilterOutcome::reject(),
            },
            None => self.today.year,
        };
        let Some(month) = ctx.args.get("month").and_then(|m| month_from_args(m)) else {
            return FilterOutcome::reject();
        };
        let Some(day) = ctx.args.get("day").and_then(|d| day_of_month_from_args(d)) else {
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
            .replace("{realDay}", day_name(dow_from_date))
            .replace("{day}", day_name(dow_from_string))
            .replace("{currentYear}", &self.today.year.to_string());
        FilterOutcome {
            accepted: true,
            range: None,
            message: Some(message),
            suggestions: None,
        }
    }
}

/// `org.languagetool.rules.pl.DecadeSpellingFilter` (rulegroup `DATA_DEKADY`):
/// rewrites `{dekada}`/`{wiek}` from the `lata` argument.
pub struct DecadeSpellingFilter;

impl RuleFilter for DecadeSpellingFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(lata) = ctx.args.get("lata") else {
            return FilterOutcome::reject();
        };
        if lata.len() < 2 {
            // Java would throw StringIndexOutOfBoundsException here.
            return FilterOutcome::reject();
        }
        let decade = &lata[2..];
        let century = &lata[..2];
        let Ok(cent) = century.parse::<u32>() else {
            return FilterOutcome::reject();
        };
        let message = ctx
            .message
            .replace("{dekada}", decade)
            .replace("{wiek}", &roman_number(cent + 1));
        // `DecadeSpellingFilter.acceptRuleMatch` builds a new `RuleMatch` whose
        // constructor re-extracts the `<suggestion>` tags from the replaced
        // message (`startWithUppercase = match.getFromPos() == 0`).
        let suggestions = extract_message_suggestions(&message, ctx.match_range.start == 0);
        FilterOutcome {
            accepted: true,
            range: None,
            message: Some(message),
            suggestions: Some(suggestions),
        }
    }
}

/// `RuleMatch`'s `<suggestion>…</suggestion>` extraction from a message:
/// skip `<mistake/>` placeholders and apply `uppercaseFirstChar` when the
/// match starts the sentence.
fn extract_message_suggestions(message: &str, start_with_uppercase: bool) -> Vec<Suggestion> {
    let mut out = Vec::new();
    let mut rest = message;
    while let Some(start) = rest.find("<suggestion>") {
        let after = &rest[start + "<suggestion>".len()..];
        let Some(end) = after.find("</suggestion>") else {
            break;
        };
        let replacement = &after[..end];
        rest = &after[end + "</suggestion>".len()..];
        if replacement.contains("<mistake/>") {
            continue;
        }
        let value = if start_with_uppercase {
            uppercase_first_char(replacement)
        } else {
            replacement.to_string()
        };
        out.push(Suggestion {
            value,
            short_description: None,
        });
    }
    out
}

fn uppercase_first_char(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `DecadeSpellingFilter.getRomanNumber`.
fn roman_number(mut num: u32) -> String {
    const NUMBERS: [u32; 13] = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1];
    const LETTERS: [&str; 13] = [
        "M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I",
    ];
    let mut roman = String::new();
    for (i, value) in NUMBERS.iter().enumerate() {
        while num >= *value {
            roman.push_str(LETTERS[i]);
            num -= value;
        }
    }
    roman
}

/// The Polish filter registry (the four XML-referenced filter classes).
pub fn polish_filter_registry(today: Ymd) -> FilterRegistry {
    FilterRegistry::builder()
        .register(
            "org.languagetool.rules.pl.DateCheckFilter",
            Arc::new(DateCheckFilter { today }),
        )
        .register(
            "org.languagetool.rules.pl.DecadeSpellingFilter",
            Arc::new(DecadeSpellingFilter),
        )
        .register(
            "org.languagetool.rules.DateRangeChecker",
            Arc::new(crate::en::filters::DateRangeCheckerFilter),
        )
        .register(
            "org.languagetool.rules.ShortenedYearRangeChecker",
            Arc::new(crate::en::filters::ShortenedYearRangeCheckerFilter),
        )
        .build()
}
