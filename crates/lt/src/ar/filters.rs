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

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use lt_core::Suggestion;
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

/// `SimpleReplaceDataLoader.loadWords`: `word=replacement1|replacement2`
/// lines (comments with `#`).
fn load_replace_map(path: &Path) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return map;
    };
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((left, right)) = line.split_once('=') else {
            continue;
        };
        if right.trim().is_empty() {
            continue;
        }
        let replacements: Vec<String> = right.split('|').map(str::to_string).collect();
        for wrong_form in left.split('|') {
            map.insert(wrong_form.to_string(), replacements.clone());
        }
    }
    map
}

/// `org.languagetool.rules.ar.filters.ArabicVerbToMafoulMutlaqFilter`
/// (rule `collo_0081_shkl_3am_Test`).
pub struct ArabicVerbToMafoulMutlaqFilter {
    verb2masdar: HashMap<String, Vec<String>>,
}

impl RuleFilter for ArabicVerbToMafoulMutlaqFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(verb) = required(ctx, "verb") else {
            return FilterOutcome::reject();
        };
        let Some(adj) = required(ctx, "adj") else {
            return FilterOutcome::reject();
        };
        let Some(verb_readings) = ctx.pattern_tokens.first() else {
            return FilterOutcome::reject();
        };
        let verb_lemmas = lt_tagger::ArabicTagger::get_lemmas(verb_readings, "verb");
        let inflected_adj_masculine =
            lt_tagger::arabic_synth::inflect_adjective_tanwin_nasb(adj, false);
        let inflected_adj_feminine =
            lt_tagger::arabic_synth::inflect_adjective_tanwin_nasb(adj, true);
        let mut inflected_masdar_list: Vec<String> = Vec::new();
        let mut inflected_adj_list: Vec<String> = Vec::new();
        for lemma in verb_lemmas {
            let Some(msdr_list) = self.verb2masdar.get(&lemma) else {
                continue;
            };
            for msdr in msdr_list {
                let inflected_masdar = lt_tagger::arabic_synth::inflect_mafoul_mutlq(msdr);
                let inflected_adj = if msdr.ends_with('\u{0629}') {
                    inflected_adj_feminine.clone()
                } else {
                    inflected_adj_masculine.clone()
                };
                inflected_masdar_list.push(inflected_masdar);
                inflected_adj_list.push(inflected_adj);
            }
        }
        let mut suggestions: Vec<Suggestion> = Vec::new();
        for (i, msdr) in inflected_masdar_list.iter().enumerate() {
            let phrase = format!("{verb} {msdr} {}", inflected_adj_list[i]);
            if !suggestions.iter().any(|s| s.value == phrase) {
                suggestions.push(Suggestion {
                    value: phrase,
                    short_description: None,
                });
            }
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(suggestions),
        }
    }
}

/// `org.languagetool.rules.ar.filters.ArabicMasdarToVerbFilter`
/// (rule `syntax_0000_Qam_bi_test`).
pub struct ArabicMasdarToVerbFilter {
    synthesizer: Arc<lt_tagger::ArabicSynthesizer>,
    masdar2verb: HashMap<String, Vec<String>>,
}

impl ArabicMasdarToVerbFilter {
    /// `filterLemmas`: keep the authorized lemmas in their own order.
    fn filter_lemmas(lemmas: &[String]) -> Vec<String> {
        let authorize = ["قَامَ"];
        authorize
            .iter()
            .filter(|l| lemmas.iter().any(|x| x == *l))
            .map(|l| l.to_string())
            .collect()
    }
}

impl RuleFilter for ArabicMasdarToVerbFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let (Some(aux_readings), Some(masdar_readings)) =
            (ctx.pattern_tokens.first(), ctx.pattern_tokens.get(1))
        else {
            return FilterOutcome::reject();
        };
        let aux_lemmas =
            Self::filter_lemmas(&lt_tagger::ArabicTagger::get_lemmas(aux_readings, "verb"));
        let masdar_lemmas = lt_tagger::ArabicTagger::get_lemmas(masdar_readings, "masdar");
        let mut verb_list: Vec<String> = Vec::new();
        for aux_token in &aux_readings.readings {
            let Some(lemma) = aux_token.stem.as_deref() else {
                continue;
            };
            if !aux_lemmas.iter().any(|l| l == lemma) {
                continue;
            }
            for masdar_lemma in &masdar_lemmas {
                let Some(verb_lemmas) = self.masdar2verb.get(masdar_lemma) else {
                    continue;
                };
                for verb_lemma in verb_lemmas {
                    for form in self.synthesizer.inflect_lemma_like(verb_lemma, aux_token) {
                        if !verb_list.contains(&form) {
                            verb_list.push(form);
                        }
                    }
                }
            }
        }
        let suggestions = verb_list
            .into_iter()
            .map(|value| Suggestion {
                value,
                short_description: None,
            })
            .collect();
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(suggestions),
        }
    }
}

/// Stage-3 placeholder for the `ArabicNumberPhraseFilter` (its
/// `ArabicNumbersWords` number-to-words engine is not ported yet): it rejects,
/// so `syntax_numeric_0003` stays inert rather than emitting wrong matches.
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
pub fn arabic_filter_registry(
    data_dir: &Path,
    today: Ymd,
    synthesizer: Arc<lt_tagger::ArabicSynthesizer>,
) -> FilterRegistry {
    let mut builder = FilterRegistry::builder();
    builder = builder.register(
        "org.languagetool.rules.ar.filters.ArabicDateCheckFilter",
        Arc::new(ArabicDateCheckFilter { today }),
    );
    builder = builder.register(
        "org.languagetool.rules.ar.filters.ArabicVerbToMafoulMutlaqFilter",
        Arc::new(ArabicVerbToMafoulMutlaqFilter {
            verb2masdar: load_replace_map(&data_dir.join("ar/rules/arabic_verb_masdar.txt")),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.ar.filters.ArabicMasdarToVerbFilter",
        Arc::new(ArabicMasdarToVerbFilter {
            synthesizer,
            masdar2verb: load_replace_map(&data_dir.join("ar/rules/arabic_masdar_verb.txt")),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.ar.filters.ArabicNumberPhraseFilter",
        Arc::new(Stage3PendingFilter {
            class: "org.languagetool.rules.ar.filters.ArabicNumberPhraseFilter",
        }),
    );
    builder.build()
}
