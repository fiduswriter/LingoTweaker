//! Esperanto rule filters referenced by `rules/eo/grammar.xml`:
//! `org.languagetool.rules.eo.DateCheckFilter` (rulegroup `DATO_TAGO`), the
//! Esperanto localization of `AbstractDateCheckFilter`.

use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::dates::{self, trim_special_characters, Ymd};

/// `DateCheckFilter.getDayOfWeek(String)` (Sunday = 1, Java `Calendar`).
/// Unknown input makes Java throw, which rejects the match.
fn get_day_of_week(day_str: &str) -> Option<u32> {
    let day = day_str.replace('\u{00AD}', "").to_lowercase();
    if day.starts_with("dim") {
        return Some(1);
    }
    if day.starts_with("lun") {
        return Some(2);
    }
    if day.starts_with("mar") {
        return Some(3);
    }
    if day.starts_with("mer") {
        return Some(4);
    }
    if day.starts_with("ĵaŭ")
        || day.starts_with("jau")
        || day.starts_with("jhau")
        || day.starts_with("jxau")
    {
        return Some(5);
    }
    if day.starts_with("ven") {
        return Some(6);
    }
    if day.starts_with("sab") {
        return Some(7);
    }
    None
}

/// `DateCheckFilter.getDayOfWeek(Calendar)` (Locale.UK English day names
/// mapped to the Esperanto names; note `jaŭdo`, not `ĵaŭdo`).
fn day_name(dow: u32) -> &'static str {
    match dow {
        1 => "dimanĉo",
        2 => "lundo",
        3 => "mardo",
        4 => "merkredo",
        5 => "jaŭdo",
        6 => "vendredo",
        _ => "sabato",
    }
}

/// `DateCheckFilter.getMonth` (Esperanto prefixes, 1-based).
fn get_month(month_str: &str) -> Option<u32> {
    let mon = month_str.to_lowercase();
    if mon.starts_with("jan") {
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
    if mon.starts_with("maj") {
        return Some(5);
    }
    if mon.starts_with("jun") {
        return Some(6);
    }
    if mon.starts_with("jul") {
        return Some(7);
    }
    if mon.starts_with("aŭg") {
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

/// `DateCheckFilter.getDayOfMonth` (Esperanto ordinal words).
fn get_day_of_month(day_str: &str) -> u32 {
    let mut day = day_str.to_lowercase();
    if day.ends_with('n') {
        day.pop();
    }
    let mut n = 0u32;
    if day.starts_with("dek") {
        n = 10;
        day = day[3..].to_string();
    } else if day.starts_with("dudek") {
        n = 20;
        day = day[5..].to_string();
    } else if day.starts_with("tridek") {
        n = 30;
        day = day[6..].to_string();
    }
    if n > 0 && day.starts_with('-') {
        day = day[1..].to_string();
    }
    n += match day.as_str() {
        "unua" => 1,
        "dua" => 2,
        "tria" => 3,
        "kvara" => 4,
        "kvina" => 5,
        "sesa" => 6,
        "sepa" => 7,
        "oka" => 8,
        "naŭa" | "nauxa" | "naua" => 9,
        _ => 0,
    };
    n
}

/// `AbstractDateCheckFilter.getDayOfMonthFromArguments` (`(\d+).*`, else
/// `getDayOfMonth`).
fn day_of_month_from_args(day_str: &str) -> Option<u32> {
    let digits: String = day_str.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        if day_str.chars().all(|c| !c.is_ascii_digit()) {
            return Some(get_day_of_month(day_str));
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

/// `org.languagetool.rules.eo.DateCheckFilter`.
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

/// `org.languagetool.rules.eo.DateCheckFilter` registry (single class).
pub fn esperanto_filter_registry(today: Ymd) -> lt_pattern::FilterRegistry {
    lt_pattern::FilterRegistry::builder()
        .register(
            "org.languagetool.rules.eo.DateCheckFilter",
            std::sync::Arc::new(DateCheckFilter { today }),
        )
        .build()
}
