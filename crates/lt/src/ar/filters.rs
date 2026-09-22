//! Arabic XML-referenced filter classes.
//!
//! `ArabicDateCheckFilter` is ported faithfully (date tables per
//! `ArabicDateFilterHelper`, mirroring `uk/filters.rs`). The three
//! tagger/synthesizer-dependent filters are explicit stage-3 placeholders:
//! they reject, so the rule stays inert until the custom `ArabicTagger`
//! affix taxonomy / `ArabicSynthesizer` / `ArabicNumbersWords` land. They
//! never emit a wrong match.
//!
//! `ArabicAdjectiveToExclamationFilter` is commented out in the XML and is
//! not registered (it is listed as a documented exception in the checklist).

use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::dates::{self, trim_special_characters, Ymd};

fn required<'a>(ctx: &'a FilterContext, key: &str) -> Option<&'a String> {
    ctx.args.get(key)
}

/// `ArabicDateFilterHelper.getDayOfWeek(String)` (exact names; throws on an
/// unknown string, which rejects the match).
fn date_check_day_of_week(day_str: &str) -> Option<u32> {
    match day_str {
        "السبت" => Some(7),             // SATURDAY
        "الأحد" => Some(1),              // SUNDAY
        "الإثنين" | "الاثنين" => Some(2), // MONDAY
        "الثلاثاء" => Some(3),           // TUESDAY
        "الأربعاء" => Some(4),           // WEDNESDAY
        "الخميس" => Some(5),            // THURSDAY
        "الجمعة" => Some(6),            // FRIDAY
        _ => None,
    }
}

/// `ArabicDateFilterHelper.getDayOfWeekName(int)` (the name used to render the
/// `{day}`/`{realDay}` placeholders).
fn ar_day_name(dow: u32) -> &'static str {
    match dow {
        1 => "الأحد",
        2 => "الإثنين",
        3 => "الثلاثاء",
        4 => "الأربعاء",
        5 => "الخميس",
        6 => "الجمعة",
        7 => "السبت",
        _ => "غير محدد",
    }
}

/// `ArabicDateFilterHelper.getMonth` (exact names after
/// `StringTools.trimSpecialCharacters`; Aden/Syriac/English/French month
/// names, in Java order).
fn date_check_month(month_str: &str) -> Option<u32> {
    match month_str {
        "كانون الثاني" | "كانون ثاني" | "يناير" | "جانفي" | "جانفييه" => {
            Some(1)
        }
        "شباط" | "فبراير" | "فيفري" => Some(2),
        "آذار" | "مارس" => Some(3),
        "نيسان" | "أبريل" | "أفريل" => Some(4),
        "أيار" | "مايو" | "ماي" => Some(5),
        "حزيران" | "يونيو" | "جوان" => Some(6),
        "تموز" | "يوليو" | "جويلية" => Some(7),
        "آب" | "أغسطس" | "أوت" => Some(8),
        "أيلول" | "سبتمبر" => Some(9),
        "تشرين الأول" | "أكتوبر" => Some(10),
        "تشرين الثاني" | "تشرين ثاني" | "نوفمبر" => Some(11),
        "كانون الأول" | "كانون أول" | "ديسمبر" => Some(12),
        _ => None,
    }
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
    if !month_str.is_empty() && month_str.chars().all(|c| c.is_ascii_digit()) {
        month_str.parse::<u32>().ok().map(|m| m.saturating_sub(1))
    } else {
        date_check_month(&trim_special_characters(month_str)).map(|m| m - 1)
    }
}

/// `org.languagetool.rules.ar.filters.ArabicDateCheckFilter`.
pub struct ArabicDateCheckFilter {
    pub today: Ymd,
}

impl RuleFilter for ArabicDateCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(week_day_str) = required(ctx, "weekDay") else {
            return FilterOutcome::reject();
        };
        let Some(dow_from_string) = date_check_day_of_week(week_day_str) else {
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
            .replace("{realDay}", ar_day_name(dow_from_date))
            .replace("{day}", ar_day_name(dow_from_string))
            .replace("{currentYear}", &self.today.year.to_string());
        FilterOutcome {
            accepted: true,
            range: None,
            message: Some(message),
            suggestions: None,
        }
    }
}

/// Stage-3 placeholder for a tagger/synthesizer-dependent Arabic filter: it
/// rejects, so the owning rule never fires until the stage-3 machinery is
/// ported.
struct Stage3PendingFilter {
    /// Java class name, for diagnostics.
    #[allow(dead_code)]
    class: &'static str,
}

impl RuleFilter for Stage3PendingFilter {
    fn accept(&self, _ctx: &FilterContext) -> FilterOutcome {
        FilterOutcome::reject()
    }
}

/// The Arabic XML-referenced filter registry.
pub fn arabic_filter_registry(today: Ymd) -> FilterRegistry {
    let mut builder = FilterRegistry::builder();
    builder = builder.register(
        "org.languagetool.rules.ar.filters.ArabicDateCheckFilter",
        std::sync::Arc::new(ArabicDateCheckFilter { today }),
    );
    for class in [
        "org.languagetool.rules.ar.filters.ArabicVerbToMafoulMutlaqFilter",
        "org.languagetool.rules.ar.filters.ArabicMasdarToVerbFilter",
        "org.languagetool.rules.ar.filters.ArabicNumberPhraseFilter",
    ] {
        builder = builder.register(class, std::sync::Arc::new(Stage3PendingFilter { class }));
    }
    builder.build()
}
