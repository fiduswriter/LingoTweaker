//! Ukrainian XML-referenced filter classes: `DateCheckFilter`
//! (`org.languagetool.rules.uk.DateCheckFilter`, rule `DATE_WEEKDAY1`).

use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::dates::{self, trim_special_characters, Ymd};

fn required<'a>(ctx: &'a FilterContext, key: &str) -> Option<&'a String> {
    ctx.args.get(key)
}

/// `DateCheckFilter.getDayOfWeek(String)` (Java `Calendar`: Sunday = 1).
/// Unknown input makes Java throw, which rejects the match.
fn date_check_day_of_week(day_str: &str) -> Option<u32> {
    let day = day_str.to_lowercase();
    if day.starts_with("по") || day == "пн" {
        return Some(2); // MONDAY
    }
    if day.starts_with("ві") || day == "вт" {
        return Some(3);
    }
    if day.starts_with("се") || day == "ср" {
        return Some(4);
    }
    if day.starts_with("че") || day == "чт" {
        return Some(5);
    }
    if day.starts_with("п'") || day.starts_with("п’") || day == "пт" {
        return Some(6);
    }
    if day.starts_with("су") || day == "сб" {
        return Some(7);
    }
    if day.starts_with("не") || day == "нд" {
        return Some(1);
    }
    None
}

/// `Calendar.getDisplayName(DAY_OF_WEEK, LONG, Locale.forLanguageTag("uk"))`.
fn uk_day_name(dow: u32) -> &'static str {
    match dow {
        1 => "неділя",
        2 => "понеділок",
        3 => "вівторок",
        4 => "середа",
        5 => "четвер",
        6 => "п'ятниця",
        _ => "субота",
    }
}

/// `DateCheckFilter.getMonth` (`startsWith`, in Java order: `ли` maps to July
/// before November).
fn date_check_month(month_str: &str) -> Option<u32> {
    let mon = month_str.to_lowercase();
    for (prefix, month) in [
        ("сі", 1u32),
        ("лю", 2),
        ("бе", 3),
        ("кв", 4),
        ("тр", 5),
        ("че", 6),
        ("ли", 7),
        ("се", 8),
        ("ве", 9),
        ("жо", 10),
        ("гр", 12),
    ] {
        if mon.starts_with(prefix) {
            return Some(month);
        }
    }
    None
}

/// `AbstractDateCheckFilter.getDayOfMonthFromArguments` with the base
/// `getDayOfMonth` (leading digits).
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

/// `org.languagetool.rules.uk.DateCheckFilter` (`DATE_WEEKDAY1`).
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
            .replace("{realDay}", uk_day_name(dow_from_date))
            .replace("{day}", uk_day_name(dow_from_string))
            .replace("{currentYear}", &self.today.year.to_string());
        FilterOutcome {
            accepted: true,
            range: None,
            message: Some(message),
            suggestions: None,
        }
    }
}

/// The Ukrainian XML-referenced filter registry.
pub fn ukrainian_filter_registry(today: Ymd) -> FilterRegistry {
    let mut builder = FilterRegistry::builder();
    builder = builder.register(
        "org.languagetool.rules.uk.DateCheckFilter",
        std::sync::Arc::new(DateCheckFilter { today }),
    );
    builder.build()
}
