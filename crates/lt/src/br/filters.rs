//! Breton rule filters referenced by `rules/br/grammar.xml`:
//! `org.languagetool.rules.br.DateCheckFilter` (rulegroup `DEIZ_DEIZIAD`),
//! the Breton localization of `AbstractDateCheckFilter`.

use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::dates::{self, trim_special_characters, Ymd};

/// `DateCheckFilter.getDayOfWeek(String)` (Sunday = 1, Java `Calendar`).
/// Unknown input makes Java throw, which rejects the match.
fn get_day_of_week(day_str: &str) -> Option<u32> {
    let day = day_str.to_lowercase();
    if day.ends_with("sul") {
        return Some(1);
    }
    if day.ends_with("lun") {
        return Some(2);
    }
    if day.ends_with("meurzh") {
        return Some(3);
    }
    if day.ends_with("merc’her") {
        return Some(4);
    }
    if day == "yaou" || day == "diriaou" {
        return Some(5);
    }
    if day.ends_with("gwener") {
        return Some(6);
    }
    if day.ends_with("sadorn") {
        return Some(7);
    }
    None
}

/// `DateCheckFilter.getDayOfWeek(Calendar)` (`Locale.UK` English day names
/// mapped to the Breton names).
fn day_name(dow: u32) -> &'static str {
    match dow {
        1 => "Sul",
        2 => "Lun",
        3 => "Meurzh",
        4 => "Merc’her",
        5 => "Yaou",
        6 => "Gwener",
        _ => "Sadorn",
    }
}

/// `DateCheckFilter.getMonth` (exact Breton month names, 1-based). Unknown
/// input makes Java throw, which rejects the match.
fn get_month(month_str: &str) -> Option<u32> {
    let mon = month_str.to_lowercase();
    match mon.as_str() {
        "genver" => Some(1),
        "c’hwevrer" => Some(2),
        "meurzh" => Some(3),
        "ebrel" => Some(4),
        "mae" => Some(5),
        "mezheven" | "even" => Some(6),
        "gouere" | "gouhere" => Some(7),
        "eost" => Some(8),
        "gwengolo" => Some(9),
        "here" => Some(10),
        "du" => Some(11),
        "kerzu" => Some(12),
        _ => None,
    }
}

/// `DateCheckFilter.getDayOfMonth` (Breton ordinal words); unknown input
/// yields 0, which the strict `Calendar` then rejects.
fn get_day_of_month(day_str: &str) -> u32 {
    let mut day = day_str.to_lowercase();
    let mut chars = day.chars();
    match chars.next() {
        Some('t') => {
            let rest: String = chars.collect();
            day = format!("d{rest}");
        }
        Some('p') => {
            let rest: String = chars.collect();
            day = format!("b{rest}");
        }
        _ => {}
    }
    if day.ends_with("vet") {
        day.truncate(day.len() - 3);
    }
    match day.as_str() {
        "c’hentañ" | "unan" => 1,
        "daou" | "eil" => 2,
        "dri" | "drede" | "deir" => 3,
        "bevar" => 4,
        "bemp" | "bem" => 5,
        "c’hwerc’h" => 6,
        "seizh" => 7,
        "eizh" => 8,
        "nav" | "na" => 9,
        "dek" => 10,
        "unnek" => 11,
        "daouzek" => 12,
        "drizek" => 13,
        "bevarzek" => 14,
        "bemzek" => 15,
        "c’hwezek" => 16,
        "seitek" => 17,
        "driwec’h" => 18,
        "naontek" => 19,
        "ugent" => 20,
        "dregont" => 30,
        _ => 0,
    }
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

/// `org.languagetool.rules.br.DateCheckFilter`.
pub struct DateCheckFilter {
    pub today: Ymd,
}

impl RuleFilter for DateCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(week_day_str) = ctx.args.get("weekDay") else {
            return FilterOutcome::reject();
        };
        let Some(dow_from_string) = get_day_of_week(&week_day_str.replace('\u{00AD}', "")) else {
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

/// `org.languagetool.rules.br.DateCheckFilter` registry (single class).
pub fn breton_filter_registry(today: Ymd) -> lt_pattern::FilterRegistry {
    lt_pattern::FilterRegistry::builder()
        .register(
            "org.languagetool.rules.br.DateCheckFilter",
            std::sync::Arc::new(DateCheckFilter { today }),
        )
        .build()
}
