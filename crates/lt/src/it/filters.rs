//! Italian rule filters: the `DateCheckFilter` reference of the Italian
//! grammar (`org.languagetool.rules.it.DateCheckFilter`, rulegroup
//! `DATE_WEEKDAY`) — a direct port of `AbstractDateCheckFilter` with the
//! Italian day/month names.
//!
//! Unlike the Spanish/English `DateCheckFilter`, the Italian one extends the
//! plain `AbstractDateCheckFilter`: it rewrites the message placeholders
//! (`{day}`/`{realDay}`/`{currentYear}`) and keeps the rule match range and
//! suggestions; it has no day/year suggestions.

use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::dates::{self, trim_special_characters, Ymd};

/// `DateCheckFilter.getDayOfWeek(String)`: Sunday = 1 (Java `Calendar`).
/// Unknown input makes Java throw, which rejects the match.
fn get_day_of_week(day_str: &str) -> Option<u32> {
    let day = day_str.replace('\u{00AD}', "").to_lowercase();
    if day.starts_with("do") || day == "domenica" {
        return Some(1);
    }
    if day.starts_with("lu") || day == "lunedì" {
        return Some(2);
    }
    if day.starts_with("ma") || day == "martedì" {
        return Some(3);
    }
    if day.starts_with("me") || day == "mercoledì" {
        return Some(4);
    }
    if day.starts_with("gi") || day == "giovedì" {
        return Some(5);
    }
    if day.starts_with("ve") || day == "venerdì" {
        return Some(6);
    }
    if day.starts_with("sa") || day == "sabato" {
        return Some(7);
    }
    None
}

/// `DateCheckFilter.getDayOfWeek(Calendar)` (Italian names).
fn day_name(dow: u32) -> &'static str {
    match dow {
        1 => "domenica",
        2 => "lunedì",
        3 => "martedì",
        4 => "mercoledì",
        5 => "giovedì",
        6 => "venerdì",
        _ => "sabato",
    }
}

/// `DateCheckFilter.getMonth` (Italian prefixes, 1-based).
fn get_month(month_str: &str) -> Option<u32> {
    let mon = month_str.to_lowercase();
    if mon.starts_with("gen") {
        return Some(1);
    }
    if mon.starts_with("feb") {
        return Some(2);
    }
    if mon.starts_with("mar") {
        return Some(3);
    }
    if mon.starts_with("apr") {
        return Some(4);
    }
    if mon.starts_with("mag") {
        return Some(5);
    }
    if mon.starts_with("giu") {
        return Some(6);
    }
    if mon.starts_with("lug") {
        return Some(7);
    }
    if mon.starts_with("ago") {
        return Some(8);
    }
    if mon.starts_with("set") {
        return Some(9);
    }
    if mon.starts_with("ott") {
        return Some(10);
    }
    if mon.starts_with("nov") {
        return Some(11);
    }
    if mon.starts_with("dic") {
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
/// (`(\d+).*`, else `getDayOfMonth` — 0 for Italian).
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

/// `org.languagetool.rules.it.DateCheckFilter`.
pub struct DateCheckFilter {
    pub today: Ymd,
}

impl RuleFilter for DateCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        // `getRequired("weekDay")` + `getDayOfWeek`, soft hyphen stripped.
        let Some(week_day_str) = ctx.args.get("weekDay") else {
            return FilterOutcome::reject();
        };
        let Some(dow_from_string) = get_day_of_week(week_day_str) else {
            return FilterOutcome::reject();
        };
        // `getDate(args)`; without a `year` argument the current year is used.
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

/// The Italian filter registry (only `DateCheckFilter` is referenced).
pub fn italian_filter_registry(today: Ymd) -> FilterRegistry {
    FilterRegistry::builder()
        .register(
            "org.languagetool.rules.it.DateCheckFilter",
            std::sync::Arc::new(DateCheckFilter { today }),
        )
        .build()
}
