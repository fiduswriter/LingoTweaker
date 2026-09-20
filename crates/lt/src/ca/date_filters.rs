//! Catalan date filters (`DateFilterHelper`, `DateCheckFilter`,
//! `NewYearDateFilter`) — `lt::ca::DateCheckFilter` /
//! `lt::ca::NewYearDateFilter`, the Catalan subclasses of the core abstract
//! filters with the Catalan day/month names and the Catalan wrong-year
//! message. Ported from the same logic as `crate::es::date_filters`.

use lt_core::{AnalyzedTokenReadings, Suggestion, TextRange};
use lt_pattern::{skip_corrected_reference, FilterContext, FilterOutcome, RuleFilter};

use crate::dates::{self, trim_special_characters, Ymd};

/// `DateFilterHelper.getDayOfWeek(String)` (Catalan full names and
/// abbreviations; Java `Calendar.SUNDAY == 1` … `SATURDAY == 7`).
pub(crate) fn get_day_of_week(day_str: &str) -> Option<u32> {
    let day = trim_special_characters(day_str).to_lowercase();
    match day.as_str() {
        "dg" | "diumenge" => Some(1),
        "dl" | "dilluns" => Some(2),
        "dt" | "dimarts" => Some(3),
        "dc" | "dimecres" => Some(4),
        "dj" | "dijous" => Some(5),
        "dv" | "divendres" => Some(6),
        "ds" | "dissabte" => Some(7),
        _ => None,
    }
}

/// `DateFilterHelper.getMonth` (Catalan prefixes, 1-based).
pub(crate) fn get_month(month_str: &str) -> Option<u32> {
    let mon = trim_special_characters(month_str).to_lowercase();
    if mon.starts_with("gen") {
        return Some(1);
    }
    if mon.starts_with("febr") {
        return Some(2);
    }
    if mon.starts_with("març") {
        return Some(3);
    }
    if mon.starts_with("abr") {
        return Some(4);
    }
    if mon.starts_with("maig") {
        return Some(5);
    }
    if mon.starts_with("juny") {
        return Some(6);
    }
    if mon.starts_with("jul") {
        return Some(7);
    }
    if mon.starts_with("ag") {
        return Some(8);
    }
    if mon.starts_with("set") {
        return Some(9);
    }
    if mon.starts_with("oct") {
        return Some(10);
    }
    if mon.starts_with("nov") {
        return Some(11);
    }
    if mon.starts_with("des") {
        return Some(12);
    }
    None
}

/// `DateFilterHelper.getDayOfWeek(Calendar)` (Catalan names).
pub(crate) fn day_name(dow: u32) -> &'static str {
    match dow {
        1 => "diumenge",
        2 => "dilluns",
        3 => "dimarts",
        4 => "dimecres",
        5 => "dijous",
        6 => "divendres",
        _ => "dissabte",
    }
}

/// `DateCheckFilter` does not override `adjustSuggestion` (identity).
fn adjust_suggestion(sugg: String) -> String {
    sugg
}

fn required<'a>(ctx: &'a FilterContext, key: &str) -> Option<&'a str> {
    ctx.args.get(key).map(|s| s.as_str())
}

fn token_end(tr: &AnalyzedTokenReadings) -> usize {
    tr.end_pos()
}

fn parse_year_arg(year_str: &str) -> Option<i32> {
    year_str.parse().ok()
}

/// `AbstractFutureDateFilter`/`AbstractNewYearDateFilter` `getDayOfMonth` is
/// not localized in German (returns 0 for letters-only names).
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

fn parse_month_arg(month_str: &str) -> Option<u32> {
    let trimmed = trim_special_characters(month_str);
    if !trimmed.is_empty() && trimmed.chars().all(char::is_numeric) {
        trimmed.parse().ok()
    } else {
        get_month(month_str)
    }
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

/// `DateCheckFilter` (`AbstractDateCheckWithSuggestionsFilter`).
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
        let day_of_week_str = trim_special_characters(
            &ctx.pattern_tokens[day_of_week_pos]
                .surface()
                .replace('\u{00AD}', ""),
        );
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
            let message = format!(
                "Aquesta data no és correcta. ¿Us referiu a l'any \"{}\"?",
                today.year
            );
            let Some(year_pos) = year_pos else {
                return FilterOutcome::reject();
            };
            let start = ctx.pattern_tokens[year_pos].start_pos;
            let end = token_end(ctx.pattern_tokens[year_pos]);
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
            token_end(ctx.pattern_tokens[end_index]),
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
                value: adjust_suggestion(sugg1),
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
                    value: adjust_suggestion(sugg2),
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
            required(ctx, "year").and_then(parse_year_arg),
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

/// `StringTools.preserveCase` for the date suggestions (the es/fr/pt
/// precedent's local copy).
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
