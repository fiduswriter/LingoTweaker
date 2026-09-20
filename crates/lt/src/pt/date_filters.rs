//! Portuguese date filters (`DateFilterHelper`, `DateCheckFilter`,
//! `FutureDateFilter`, `NewYearDateFilter`, `YMDNewYearDateFilter`) — the
//! Portuguese subclasses of the core abstract filters, ported from the same
//! logic as the Spanish/Italian/French instances with the Portuguese
//! day/month names and the Portuguese wrong-year message.
//!
//! Like the Spanish `DateCheckFilter`, the Portuguese one extends
//! `AbstractDateCheckWithSuggestionsFilter` (day-of-week and day-of-month
//! suggestions). `DateFilterHelper` trims special characters before the
//! prefix match, exactly like the Spanish/Italian helpers.

use lt_core::{Suggestion, TextRange};
use lt_pattern::{skip_corrected_reference, FilterContext, FilterOutcome, RuleFilter};

use crate::dates::{self, trim_special_characters, Ymd};

/// `DateFilterHelper.getDayOfWeek(String)` (Portuguese; Java
/// `Calendar.SUNDAY == 1` … `SATURDAY == 7`).
pub(crate) fn get_day_of_week(day_str: &str) -> Option<u32> {
    let day = trim_special_characters(day_str).to_lowercase();
    if day.starts_with("dom") {
        return Some(1);
    }
    if day.starts_with("seg") {
        return Some(2);
    }
    if day.starts_with("ter") {
        return Some(3);
    }
    if day.starts_with("qua") {
        return Some(4);
    }
    if day.starts_with("qui") {
        return Some(5);
    }
    if day.starts_with("sex") {
        return Some(6);
    }
    if day.starts_with("sáb") {
        return Some(7);
    }
    None
}

/// `DateFilterHelper.getDayOfWeek(Calendar)` (Portuguese names).
pub(crate) fn day_name(dow: u32) -> &'static str {
    match dow {
        1 => "domingo",
        2 => "segunda-feira",
        3 => "terça-feira",
        4 => "quarta-feira",
        5 => "quinta-feira",
        6 => "sexta-feira",
        _ => "sábado",
    }
}

/// `DateFilterHelper.getMonth` (Portuguese prefixes, 1-based).
pub(crate) fn get_month(month_str: &str) -> Option<u32> {
    let mon = trim_special_characters(month_str).to_lowercase();
    if mon.starts_with("jan") {
        return Some(1);
    }
    if mon.starts_with("fev") {
        return Some(2);
    }
    if mon.starts_with("mar") {
        return Some(3);
    }
    if mon.starts_with("abr") {
        return Some(4);
    }
    if mon.starts_with("mai") {
        return Some(5);
    }
    if mon.starts_with("jun") {
        return Some(6);
    }
    if mon.starts_with("jul") {
        return Some(7);
    }
    if mon.starts_with("ago") {
        return Some(8);
    }
    if mon.starts_with("set") {
        return Some(9);
    }
    if mon.starts_with("out") {
        return Some(10);
    }
    if mon.starts_with("nov") {
        return Some(11);
    }
    if mon.starts_with("dez") {
        return Some(12);
    }
    None
}

fn required<'a>(ctx: &'a FilterContext, key: &str) -> Option<&'a str> {
    ctx.args.get(key).map(|s| s.as_str())
}

fn parse_month_arg(month_str: &str) -> Option<u32> {
    let trimmed = trim_special_characters(month_str);
    if !trimmed.is_empty() && trimmed.chars().all(char::is_numeric) {
        trimmed.parse().ok()
    } else {
        get_month(month_str)
    }
}

/// `AbstractFutureDateFilter`/`AbstractNewYearDateFilter` `getDayOfMonth`
/// (not localized for Portuguese: letters-only names yield 0).
fn parse_day_arg(day_str: &str) -> Option<u32> {
    let digits: String = day_str.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() && day_str.chars().all(|c| !c.is_ascii_digit()) {
        return Some(0);
    }
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

fn find_new_day_of_month(day: u32, month: u32, year: i32, target_dow: u32) -> Option<u32> {
    for diff in 1u32..7 {
        if day > diff && dates::weekday(year, month, day - diff) == Some(target_dow) {
            return Some(day - diff);
        }
        if day + diff < 32 && dates::weekday(year, month, day + diff) == Some(target_dow) {
            return Some(day + diff);
        }
    }
    None
}

/// `org.languagetool.rules.pt.DateCheckFilter`
/// (`AbstractDateCheckWithSuggestionsFilter`; no `adjustSuggestion`).
pub struct DateCheckFilter {
    pub today: Ymd,
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
        let day_of_week_str = ctx.pattern_tokens[day_of_week_pos]
            .surface()
            .replace('\u{00AD}', "");
        let full_date_pos = ctx
            .args
            .get("date")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(-1);

        let (day_pos, year_pos, day_str, month_str, year_str, is_full_date_token) =
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
                    Some(full_pos),
                    parts[2].to_string(),
                    parts[1].to_string(),
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
                    year_pos,
                    ctx.pattern_tokens[day_pos].surface().to_string(),
                    ctx.pattern_tokens[month_pos].surface().to_string(),
                    year_str,
                    false,
                )
            };

        let Some(day_of_week_from_string) = get_day_of_week(&day_of_week_str) else {
            return FilterOutcome::reject();
        };
        let day_digits: String = day_str.chars().take_while(|c| c.is_ascii_digit()).collect();
        let day: u32 = if day_digits.is_empty() {
            0
        } else {
            day_digits.parse().unwrap_or(0)
        };
        let Some(month) = parse_month_arg(&month_str) else {
            return FilterOutcome::reject();
        };
        let year: i32 = match &year_str {
            Some(y) => match y.parse() {
                Ok(v) => v,
                Err(_) => return FilterOutcome::reject(),
            },
            None => self.today.year,
        };
        let Some(day_of_week_from_date) = dates::weekday(year, month, day) else {
            return FilterOutcome::reject();
        };
        if day_of_week_from_string == day_of_week_from_date {
            return FilterOutcome::reject();
        }

        let today = self.today;
        // suggest changing the year (to the current year)
        if dates::weekday(today.year, month, day) == Some(day_of_week_from_string) {
            let message =
                "Esta data está incorreta. Você está se referindo ao ano \"{currentYear}\"?"
                    .replace("{currentYear}", &today.year.to_string());
            let Some(year_pos) = year_pos else {
                return FilterOutcome::reject();
            };
            let start = ctx.pattern_tokens[year_pos].start_pos;
            let end = ctx.pattern_tokens[year_pos].end_pos();
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
        let real_day_name = day_name(day_of_week_from_date);
        let claimed_day_name = day_name(day_of_week_from_string);
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
            ctx.pattern_tokens[end_index].end_pos(),
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
            } else {
                corrected_day.to_string()
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

/// `FutureDateFilter` (`AbstractFutureDateFilter`).
pub struct FutureDateFilter {
    pub today: Ymd,
}

impl RuleFilter for FutureDateFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Some(year), Some(month), Some(day)) = (
            required(ctx, "year").and_then(|s| s.parse::<i32>().ok()),
            required(ctx, "month").and_then(parse_month_arg),
            required(ctx, "day").and_then(parse_day_arg),
        ) else {
            return FilterOutcome::reject();
        };
        // invalid dates ('32.8.2014') are caught by a different rule
        if !dates::is_valid(year, month, day) {
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

/// `NewYearDateFilter` (`AbstractNewYearDateFilter`).
pub struct NewYearDateFilter {
    pub today: Ymd,
}

impl NewYearDateFilter {
    fn accept_parts(&self, year: i32, month: u32, message: &str) -> FilterOutcome {
        // isJanuary && not December && the text's year is last year
        if self.today.month == 1 && month != 12 && year + 1 == self.today.year {
            let message = message
                .replace("{year}", &year.to_string())
                .replace("{realYear}", &self.today.year.to_string());
            // Java builds a new `RuleMatch` from the rewritten message, so
            // the suggestions are the inline `<suggestion>` blocks again.
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
}

impl RuleFilter for NewYearDateFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Some(year), Some(month), Some(day)) = (
            required(ctx, "year").and_then(|s| s.parse::<i32>().ok()),
            required(ctx, "month").and_then(parse_month_arg),
            required(ctx, "day").and_then(parse_day_arg),
        ) else {
            return FilterOutcome::reject();
        };
        if !dates::is_valid(year, month, day) {
            return FilterOutcome::reject();
        }
        self.accept_parts(year, month, &ctx.message)
    }
}

/// `YMDNewYearDateFilter` (`NewYearDateFilter` with a `yyyy-mm-dd` `date`
/// arg; `YMDDateHelper.parseDate` + `correctDate` rewrite `{realDate}`).
pub struct YmdNewYearDateFilter {
    pub today: Ymd,
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
        if parts.len() != 3 {
            return FilterOutcome::reject();
        }
        let (Some(year), Some(month), Some(day)) = (
            parts[0].parse::<i32>().ok(),
            parts[1].parse::<u32>().ok(),
            parts[2].parse::<u32>().ok(),
        ) else {
            return FilterOutcome::reject();
        };
        if !dates::is_valid(year, month, day) {
            return FilterOutcome::reject();
        }
        let message = ctx.message.replace(
            "{realDate}",
            &format!("{}-{}-{}", year + 1, parts[1], parts[2]),
        );
        let inner = NewYearDateFilter { today: self.today };
        inner.accept_parts(year, month, &message)
    }
}

pub(crate) fn register(
    builder: lt_pattern::FilterRegistryBuilder,
    today: Ymd,
) -> lt_pattern::FilterRegistryBuilder {
    builder
        .register(
            "org.languagetool.rules.pt.DateCheckFilter",
            std::sync::Arc::new(DateCheckFilter { today }),
        )
        .register(
            "org.languagetool.rules.pt.FutureDateFilter",
            std::sync::Arc::new(FutureDateFilter { today }),
        )
        .register(
            "org.languagetool.rules.pt.NewYearDateFilter",
            std::sync::Arc::new(NewYearDateFilter { today }),
        )
        .register(
            "org.languagetool.rules.pt.YMDNewYearDateFilter",
            std::sync::Arc::new(YmdNewYearDateFilter { today }),
        )
}

/// `StringTools.preserveCase`.
fn preserve_case(input: &str, model: &str) -> String {
    if model.is_empty() {
        return input.to_string();
    }
    if lt_tagger::is_capitalized_word(model) {
        return lt_tagger::uppercase_first_char(&input.to_lowercase());
    }
    if lt_tagger::is_all_uppercase(model) {
        return input.to_uppercase();
    }
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_and_month_names() {
        assert_eq!(get_day_of_week("segunda-feira"), Some(2));
        assert_eq!(get_day_of_week("Sáb."), Some(7));
        assert_eq!(get_day_of_week("Terça"), Some(3));
        assert_eq!(get_day_of_week("xyz"), None);
        assert_eq!(get_month("fevereiro"), Some(2));
        assert_eq!(get_month("out."), Some(10));
        assert_eq!(get_month("dezembro"), Some(12));
        assert_eq!(get_month("xyz"), None);
        assert_eq!(day_name(3), "terça-feira");
    }
}
