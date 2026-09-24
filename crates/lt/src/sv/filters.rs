//! Swedish rule filters referenced by `rules/sv/grammar.xml`:
//! `org.languagetool.rules.sv.DateCheckFilter` (rulegroup
//! `VECKODAG_DATUM`), a hand-written localization of
//! `AbstractDateCheckFilter` (upstream `sv` has no filter classes yet; the
//! class name keeps the XML upstream-compatible, like the Esperanto
//! `DateCheckFilter`).

use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::dates::{self, trim_special_characters, Ymd};

/// `DateCheckFilter.getDayOfWeek(String)` (Sunday = 1, Java `Calendar`).
/// Unknown input makes Java throw, which rejects the match. The prefixes
/// match the genitive/definite forms of the `weekdays` entity as well
/// (`måndagen` …, `måndags` …).
fn get_day_of_week(day_str: &str) -> Option<u32> {
    let day = day_str.replace('\u{00AD}', "").to_lowercase();
    if day.starts_with("mån") || day.starts_with("man") {
        return Some(2);
    }
    if day.starts_with("tis") {
        return Some(3);
    }
    if day.starts_with("ons") {
        return Some(4);
    }
    if day.starts_with("tors") {
        return Some(5);
    }
    if day.starts_with("fre") {
        return Some(6);
    }
    if day.starts_with("lör") || day.starts_with("lor") {
        return Some(7);
    }
    if day.starts_with("sön") || day.starts_with("son") {
        return Some(1);
    }
    None
}

/// `DateCheckFilter.getDayOfWeek(Calendar)` (`Locale.UK` day names mapped to
/// the Swedish names).
fn day_name(dow: u32) -> &'static str {
    match dow {
        1 => "söndag",
        2 => "måndag",
        3 => "tisdag",
        4 => "onsdag",
        5 => "torsdag",
        6 => "fredag",
        _ => "lördag",
    }
}

/// `DateCheckFilter.getMonth` (Swedish prefixes, 1-based). The abbreviations
/// of the `months` entity are prefixes of the full names except `sept` /
/// `mars` / `majs`.
fn get_month(month_str: &str) -> Option<u32> {
    let mon = month_str.to_lowercase();
    if mon.starts_with("jan") {
        return Some(1);
    }
    if mon.starts_with("feb") {
        return Some(2);
    }
    if mon.starts_with("mars") || mon == "mar" || mon.starts_with("mar'") {
        return Some(3);
    }
    if mon.starts_with("apr") {
        return Some(4);
    }
    if mon.starts_with("maj") {
        return Some(5);
    }
    if mon.starts_with("jun") {
        return Some(6);
    }
    if mon.starts_with("jul") {
        return Some(7);
    }
    if mon.starts_with("aug") {
        return Some(8);
    }
    if mon.starts_with("sep") {
        return Some(9);
    }
    if mon.starts_with("okt") {
        return Some(10);
    }
    if mon.starts_with("nov") {
        return Some(11);
    }
    if mon.starts_with("dec") {
        return Some(12);
    }
    None
}

/// `AbstractDateCheckFilter.getDayOfMonthFromArguments` (`(\d+).*`, else
/// `getDayOfMonth`; Swedish has no localized day-number words → 0, which
/// makes the date invalid and rejects the match).
fn day_of_month_from_args(day_str: &str) -> Option<u32> {
    // The rule passes the day-of-month element, which may carry the written
    // article ("den 5"/"det 5"); the article part is not a date number.
    let day_str = day_str
        .trim()
        .strip_prefix("den ")
        .or_else(|| day_str.trim().strip_prefix("det "))
        .unwrap_or(day_str.trim());
    let digits: String = day_str.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        if day_str.chars().all(|c| !c.is_ascii_digit()) {
            return Some(0);
        }
        return None;
    }
    digits.parse().ok()
}

fn month_from_args(month_str: &str) -> Option<u32> {
    let ms = trim_special_characters(month_str);
    if !ms.is_empty() && ms.chars().all(|c| c.is_ascii_digit()) {
        ms.parse::<u32>().ok().map(|m| m.saturating_sub(1))
    } else {
        get_month(&ms).map(|m| m - 1)
    }
}

/// `org.languagetool.rules.sv.DateCheckFilter`.
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

/// `org.languagetool.rules.sv.DateCheckFilter` registry (single class).
pub fn swedish_filter_registry(today: Ymd) -> lt_pattern::FilterRegistry {
    lt_pattern::FilterRegistry::builder()
        .register(
            "org.languagetool.rules.sv.DateCheckFilter",
            std::sync::Arc::new(DateCheckFilter { today }),
        )
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swedish_weekday_prefixes_match_java_calendar_order() {
        // 1 = Sunday … 7 = Saturday
        assert_eq!(get_day_of_week("måndag"), Some(2));
        assert_eq!(get_day_of_week("Måndagen"), Some(2));
        assert_eq!(get_day_of_week("måndags"), Some(2));
        assert_eq!(get_day_of_week("mån",), Some(2));
        assert_eq!(get_day_of_week("tisdag"), Some(3));
        assert_eq!(get_day_of_week("onsdag"), Some(4));
        assert_eq!(get_day_of_week("torsdag"), Some(5));
        assert_eq!(get_day_of_week("fredag"), Some(6));
        assert_eq!(get_day_of_week("lördag"), Some(7));
        assert_eq!(get_day_of_week("söndag"), Some(1));
        assert_eq!(get_day_of_week("xyz"), None);
    }

    #[test]
    fn swedish_month_prefixes() {
        assert_eq!(get_month("januari"), Some(1));
        assert_eq!(get_month("jan"), Some(1));
        assert_eq!(get_month("mars"), Some(3));
        assert_eq!(get_month("Mar"), Some(3));
        assert_eq!(get_month("maj"), Some(5));
        assert_eq!(get_month("sept"), Some(9));
        assert_eq!(get_month("september"), Some(9));
        assert_eq!(get_month("december"), Some(12));
        assert_eq!(get_month("xyz"), None);
    }
}
