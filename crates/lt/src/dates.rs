//! Civil-date arithmetic for the date filters (port of the Java `Calendar`
//! usage in `AbstractDateCheckFilter`, `AbstractFutureDateFilter`,
//! `AbstractNewYearDateFilter` with `Locale.UK` / `DateFilterHelper`).
//!
//! Weekday convention matches `java.util.Calendar`: 1 = Sunday … 7 =
//! Saturday. Invalid dates (e.g. 31 February) are rejected like
//! `Calendar.setLenient(false)`.

#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ymd {
    pub year: i32,
    /// 1..=12
    pub month: u32,
    pub day: u32,
}

impl Ymd {
    /// Today (UTC) — the engine has no timezone context; LT uses the JVM
    /// default locale's calendar.
    ///
    /// On `wasm32-unknown-unknown` there is no clock (`SystemTime::now()`
    /// traps), so wasm callers pin the date through `EngineBuilder::today`;
    /// the `lt-wasm` wrapper passes the browser's date. The fallback keeps
    /// engine construction panic-free for wasm callers that do not care
    /// about date rules.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn today() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let days = secs.div_euclid(86_400);
        civil_from_days(days).unwrap_or(Ymd {
            year: 1970,
            month: 1,
            day: 1,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn today() -> Self {
        Ymd {
            year: 1970,
            month: 1,
            day: 1,
        }
    }
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// `None` for structurally invalid dates (month outside 1..=12, day outside
/// the month's length) — LT's lenient-calendar exception path.
pub fn is_valid(year: i32, month: u32, day: u32) -> bool {
    (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)
}

/// Days since 1970-01-01 (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(if m <= 2 { 1 } else { 0 });
    let era = y.div_euclid(400);
    let yoe = y - era * 400; // [0, 399]
    let mp = (i64::from(m) + 9) % 12; // Mar=0
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn civil_from_days(z: i64) -> Option<Ymd> {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let year = (y + i64::from(if m <= 2 { 1 } else { 0 })) as i32;
    is_valid(year, m, d).then_some(Ymd {
        year,
        month: m,
        day: d,
    })
}

/// Day of week, 1 = Sunday … 7 = Saturday (`java.util.Calendar`); `None` for
/// invalid dates.
pub fn weekday(year: i32, month: u32, day: u32) -> Option<u32> {
    if !is_valid(year, month, day) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    // 1970-01-01 was a Thursday (4 in the Sunday-first convention).
    Some(((((days % 7 + 7) % 7 + 4) % 7) + 1) as u32)
}

/// Localized weekday name for a `Calendar.DAY_OF_WEEK` value
/// (`Calendar.getDisplayName(LONG, Locale.UK)`).
pub fn weekday_name(calendar_dow: u32) -> &'static str {
    match calendar_dow {
        1 => "Sunday",
        2 => "Monday",
        3 => "Tuesday",
        4 => "Wednesday",
        5 => "Thursday",
        6 => "Friday",
        _ => "Saturday",
    }
}

/// `DateFilterHelper.getDayOfWeek`: prefix match on su/mo/tu/we/th/fr/sa →
/// `Calendar.SUNDAY(1)`…`SATURDAY(7)`.
pub fn weekday_from_name(day_str: &str) -> Option<u32> {
    let day = trim_special_characters(day_str).to_lowercase();
    for (prefix, dow) in [
        ("su", 1u32),
        ("mo", 2),
        ("tu", 3),
        ("we", 4),
        ("th", 5),
        ("fr", 6),
        ("sa", 7),
    ] {
        if day.starts_with(prefix) {
            return Some(dow);
        }
    }
    None
}

/// `DateFilterHelper.getMonth`: prefix match on jan..dec → 1..=12.
pub fn month_from_name(month_str: &str) -> Option<u32> {
    let mon = trim_special_characters(month_str).to_lowercase();
    for (i, prefix) in [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ]
    .iter()
    .enumerate()
    {
        if mon.starts_with(prefix) {
            return Some(i as u32 + 1);
        }
    }
    None
}

/// `StringTools.trimSpecialCharacters`: strips soft hyphens and other
/// zero-width/format characters LT treats as noise.
pub fn trim_special_characters(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !matches!(
                *c,
                '\u{00AD}' | '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{2060}'
            )
        })
        .collect()
}

/// `OrdinalSuffixFilter` / `DateCheckFilter.getDayStrLikeOriginal` ordinal
/// suffix for an English cardinal.
pub fn ordinal_suffix(number: u32) -> &'static str {
    let last_two = number % 100;
    if (11..=13).contains(&last_two) {
        "th"
    } else {
        match number % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekdays_match_java_calendar() {
        // Thursday
        assert_eq!(weekday(1970, 1, 1), Some(5));
        // Monday, 8 November 2004
        assert_eq!(weekday(2004, 11, 8), Some(2));
        assert_eq!(weekday(2003, 11, 8), Some(7)); // Saturday
        assert_eq!(weekday(2026, 2, 30), None);
        assert_eq!(weekday(2024, 2, 29), Some(5)); // leap year Thursday
    }

    #[test]
    fn parses_english_month_and_day_names() {
        assert_eq!(month_from_name("March"), Some(3));
        assert_eq!(month_from_name("Sept."), Some(9));
        assert_eq!(month_from_name("Xyz"), None);
        assert_eq!(weekday_from_name("Monday"), Some(2));
        assert_eq!(weekday_from_name("Sun."), Some(1));
    }

    #[test]
    fn ordinal_suffixes() {
        assert_eq!(ordinal_suffix(1), "st");
        assert_eq!(ordinal_suffix(2), "nd");
        assert_eq!(ordinal_suffix(3), "rd");
        assert_eq!(ordinal_suffix(4), "th");
        assert_eq!(ordinal_suffix(11), "th");
        assert_eq!(ordinal_suffix(21), "st");
        assert_eq!(ordinal_suffix(113), "th");
    }
}
