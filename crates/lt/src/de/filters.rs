//! German filter registry (`<filter class="...">` implementations used by
//! `de/rules/grammar.xml` and `style.xml`).
//!
//! Filters whose Java implementation depends on the German speller are
//! registered once the speller port lands; until then the engine reports
//! them as unmapped filter classes in `compile_failures` (never silently
//! dropped).

use std::sync::Arc;

use lt_pattern::{FilterContext, FilterOutcome, FilterRegistry, RuleFilter};

use crate::dates::Ymd;

/// Shared state for German filters (mirrors the Java singletons
/// `GermanyGerman.getInstance()` / `GermanTagger.INSTANCE`).
pub struct DeFilterEnv {
    pub tagger: Arc<lt_tagger::GermanTagger>,
    pub spelling: Arc<crate::de::spelling::GermanSpellingRule>,
    /// English tagger for `IsEnglishWordFilter`. `None` reproduces the
    /// pinned German build: its classpath has no `en-US` language
    /// (`Languages.getLanguageForShortCode("en-US")` is null in the
    /// language-de module and in the Docker oracle), so the Java filter
    /// constructor leaves `tagger == null` and every match is rejected.
    pub english_tagger: Option<Arc<lt_tagger::EnglishTagger>>,
    /// Used by the synthesizer-based filters (`AdvancedSynthesizerFilter`,
    /// `AdaptSuggestionFilter`) once ported.
    #[allow(dead_code)]
    pub synth: Arc<crate::de::synthesizer::GermanSynthesizerAdapter>,
    /// Used by the date filters once ported.
    #[allow(dead_code)]
    pub today: Ymd,
    /// `de/rules/addedCompound.txt` (rules-dir resource)
    pub compound_check_path: std::path::PathBuf,
    /// `GermanMultitokenSpeller` (`MultitokenSpellerFilter`)
    pub multitoken: crate::multitoken::MultitokenSpeller,
}

pub type Env = Arc<DeFilterEnv>;

/// `org.languagetool.rules.DateRangeChecker`.
struct DateRangeChecker;

impl RuleFilter for DateRangeChecker {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let x = ctx.args.get("x").and_then(|s| s.parse::<i64>().ok());
        let y = ctx.args.get("y").and_then(|s| s.parse::<i64>().ok());
        let (Some(x), Some(y)) = (x, y) else {
            return FilterOutcome::reject();
        };
        if x >= y {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

/// `org.languagetool.rules.de.UppercaseNounReadingFilter`.
struct UppercaseNounReadingFilter {
    env: Env,
}

impl RuleFilter for UppercaseNounReadingFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(token) = ctx.args.get("token") else {
            return FilterOutcome::reject();
        };
        let uppercase = lt_tagger::uppercase_first_char(token);
        let readings = self.env.tagger.tag(&[uppercase], true);
        let has_noun_reading = readings.iter().any(|r| {
            r.readings.iter().any(|at| {
                at.pos_tag
                    .as_deref()
                    .map(|t| t.contains("SUB:"))
                    .unwrap_or(false)
            }) && !r.readings.iter().any(|at| {
                at.pos_tag
                    .as_deref()
                    .map(|t| t.contains("ADJ"))
                    .unwrap_or(false)
            })
        });
        if has_noun_reading {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

/// `org.languagetool.rules.de.InsertCommaFilter`.
struct InsertCommaFilter {
    env: Env,
}

impl InsertCommaFilter {
    fn tag(&self, word: &str) -> Vec<lt_core::AnalyzedTokenReadings> {
        self.env.tagger.tag(&[word.to_string()], true)
    }

    fn has_tag(&self, tags: &[lt_core::AnalyzedTokenReadings], prefix: &str) -> bool {
        tags.iter().any(|t| t.has_pos_tag_starting_with(prefix))
    }
}

impl RuleFilter for InsertCommaFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let suggestions = insert_comma_suggestions(self, ctx);
        let mut out = FilterOutcome::accept();
        out.suggestions = Some(
            suggestions
                .into_iter()
                .map(|value| lt_core::Suggestion {
                    value,
                    short_description: None,
                })
                .collect(),
        );
        out
    }
}

// Java's `InsertCommaFilter` has several branches with the same suggestion;
// keep them in the original evaluation order (a token can carry both
// PRO:POS and PRO:PER readings, so the order is observable).
#[allow(clippy::if_same_then_else)]
fn insert_comma_suggestions(filter: &InsertCommaFilter, ctx: &FilterContext) -> Vec<String> {
    let mut suggestions = Vec::new();
    for suggestion in &ctx.suggestions {
        let parts: Vec<&str> = suggestion.value.split_whitespace().collect();
        match parts.len() {
            2 => suggestions.push(format!("{}, {}", parts[0], parts[1])),
            3 => {
                let tags1 = filter.tag(parts[0]);
                let tags2 = filter.tag(parts[1]);
                let tags3 = filter.tag(parts[2]);
                if filter.has_tag(&tags1, "VER:") && filter.has_tag(&tags2, "PRO:PER:") {
                    // "Ich hoffe(,) es geht Ihnen gut."
                    suggestions.push(format!("{}, {} {}", parts[0], parts[1], parts[2]));
                } else if regex_matches(parts[0], "[Ss]agt?")
                    && parts[1] == "mal"
                    && filter.has_tag(&tags3, "VER:")
                {
                    // "Sag mal(,) hast du"
                    suggestions.push(format!("{} {}, {}", parts[0], parts[1], parts[2]));
                } else if filter.has_tag(&tags1, "VER:")
                    && filter.has_tag(&tags2, "ADV:")
                    && filter.has_tag(&tags3, "VER:")
                {
                    // "Ich denke(,) hier kann aber auch ..."
                    suggestions.push(format!("{}, {} {}", parts[0], parts[1], parts[2]));
                }
            }
            4..=7 => {
                let tags1 = filter.tag(parts[0]);
                let tags2 = filter.tag(parts[1]);
                let tags3 = filter.tag(parts[2]);
                let tags4 = filter.tag(parts[3]);
                let rest1 = parts[1..].join(" ");
                let pattern_token_pos = ctx.pattern_token_pos;
                let cond = pattern_token_pos <= 2
                    || (pattern_token_pos == 3
                        && ctx
                            .sentence_tokens
                            .get(1)
                            .map(|t| t.has_pos_tag_starting_with("ADV:"))
                            .unwrap_or(false));
                if cond {
                    if parts.len() == 5
                        && filter.has_tag(&tags1, "VER:")
                        && filter.has_tag(&tags2, "ART:")
                        && filter.has_tag(&tags3, "SUB:")
                        && filter.has_tag(&filter.tag(parts[3]), "SUB:")
                        && filter.has_tag(&filter.tag(parts[4]), "VER:")
                    {
                        // "Ist der Kunde Verbraucher(,) gilt ..."
                        suggestions.push(format!(
                            "{} {} {} {},",
                            parts[0], parts[1], parts[2], parts[3]
                        ));
                    } else if parts.len() == 4
                        && ctx
                            .pattern_tokens
                            .first()
                            .map(|t| t.has_pos_tag_starting_with("VER:"))
                            .unwrap_or(false)
                        && regex_matches(
                            parts[1],
                            "der|die|das|seine|ihre|deine|unsere|meine|folgender|dieser",
                        )
                    {
                        // "Aristoteles meint(,) das Genussleben führe ..."
                        suggestions.push(format!("{}, {}", parts[0], rest1));
                    } else if filter.has_tag(&tags1, "VER:")
                        && filter.has_tag(&tags2, "PRO:POS:")
                        && filter.has_tag(&tags3, "SUB:")
                    {
                        suggestions.push(format!("{}, {}", parts[0], rest1));
                    } else if filter.has_tag(&tags1, "VER:")
                        && filter.has_tag(&tags2, "PRO:PER:")
                        && filter.has_tag(&tags3, "ADV:INR")
                    {
                        let rest2 = parts[2..].join(" ");
                        suggestions.push(format!("{} {}, {}", parts[0], parts[1], rest2));
                    } else if filter.has_tag(&tags1, "VER:")
                        && filter.has_tag(&tags2, "PRO:POS:")
                        && filter.has_tag(&tags3, "ADJ:")
                    {
                        suggestions.push(format!("{}, {}", parts[0], rest1));
                    } else if regex_matches(
                        parts[0],
                        "denke|dachte|glaube|schätze|vermute|behaupte",
                    ) && filter.has_tag(&tags2, "PRO:DEM:")
                        && filter.has_tag(&tags3, "SUB:")
                    {
                        suggestions.push(format!("{}, {}", parts[0], rest1));
                    } else if pattern_token_pos == 1
                        && regex_matches(parts[1], "bei|für|mit")
                        && regex_matches(parts[2], "[Di]ir|[Dd]ich|[Ee]uer|[Ee]uch")
                        && filter.has_tag(&tags4, "VER:")
                    {
                        suggestions.push(format!("{}, {}", parts[0], rest1));
                    }
                }
            }
            _ => {}
        }
    }
    suggestions
}

fn regex_matches(text: &str, pattern: &str) -> bool {
    regex::Regex::new(&format!("^(?:{pattern})$"))
        .map(|re| re.is_match(text))
        .unwrap_or(false)
}

/// `org.languagetool.rules.de.RecentYearFilter`.
///
/// Uses the engine's pinned "today" (like the other date filters) so wasm
/// builds without a clock behave deterministically.
struct RecentYearFilter {
    today: Ymd,
}

impl RuleFilter for RecentYearFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(max_years_back) = ctx
            .args
            .get("maxYearsBack")
            .and_then(|s| s.parse::<i32>().ok())
        else {
            return FilterOutcome::reject();
        };
        let Some(year) = ctx.args.get("year").and_then(|s| s.parse::<i32>().ok()) else {
            return FilterOutcome::reject();
        };
        let this_year = self.today.year;
        let max_year = this_year - max_years_back;
        if year < this_year && year >= max_year {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

/// `org.languagetool.rules.de.RemoveUnknownCompoundsFilter`.
struct RemoveUnknownCompoundsFilter {
    env: Env,
}

impl RuleFilter for RemoveUnknownCompoundsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let part1 = ctx.args.get("part1").cloned().unwrap_or_default();
        let part2 = ctx.args.get("part2").cloned().unwrap_or_default();
        let compound = format!("{part1}{}", part2.to_lowercase());
        if self.env.spelling.is_misspelled(&compound) {
            return FilterOutcome::reject();
        }
        FilterOutcome::accept()
    }
}

/// `org.languagetool.rules.de.ValidWordFilter`.
struct ValidWordFilter {
    env: Env,
}

impl RuleFilter for ValidWordFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let word1 = format!(
            "{}{}",
            ctx.args.get("word1").cloned().unwrap_or_default(),
            ctx.args.get("word2").cloned().unwrap_or_default()
        );
        let word2 = format!(
            "{}{}",
            ctx.args.get("word1").cloned().unwrap_or_default(),
            ctx.args
                .get("word2")
                .map(|s| s.to_lowercase())
                .unwrap_or_default()
        );
        if !self.env.spelling.is_misspelled(&word1) || !self.env.spelling.is_misspelled(&word2) {
            return FilterOutcome::reject();
        }
        FilterOutcome::accept()
    }
}

/// `org.languagetool.rules.de.CompoundCheckFilter` (`addedCompound.txt`
/// `part1;part2` pairs, lowercased).
struct CompoundCheckFilter {
    pairs: std::collections::HashMap<String, Vec<String>>,
}

impl CompoundCheckFilter {
    fn load(path: &std::path::Path) -> Self {
        let mut pairs: std::collections::HashMap<String, Vec<String>> = Default::default();
        if let Ok(text) = lt_data::fs::read_to_string(path) {
            for line in text.lines() {
                let line = line.split('#').next().unwrap_or("").trim();
                if line.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split(';').collect();
                if parts.len() != 2 {
                    continue;
                }
                pairs
                    .entry(parts[0].trim().to_lowercase())
                    .or_default()
                    .push(parts[1].trim().to_lowercase());
            }
        }
        Self { pairs }
    }
}

impl RuleFilter for CompoundCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let part1 = ctx.args.get("part1").cloned().unwrap_or_default();
        let part2 = ctx.args.get("part2").cloned().unwrap_or_default();
        match self.pairs.get(&part1.to_lowercase()) {
            Some(list) if list.contains(&part2.to_lowercase()) => FilterOutcome::accept(),
            _ => FilterOutcome::reject(),
        }
    }
}

/// `org.languagetool.rules.de.PotentialCompoundFilter`.
struct PotentialCompoundFilter {
    env: Env,
}

impl PotentialCompoundFilter {
    /// Java `PotentialCompoundFilter`: the joined word is tagged and the
    /// default spelling rule's full `match` runs on a synthetic
    /// `[SENT_START, word]` sentence — this also applies
    /// `ignorePotentiallyMisspelledWord` (e.g. `Kryptomarktplatzes` is
    /// `isMisspelled` but the rule accepts it as a compound).
    fn spelling_rejects(&self, joined_word: &str) -> bool {
        let mut tokens: Vec<lt_core::AnalyzedTokenReadings> = Vec::with_capacity(2);
        tokens.push(lt_core::AnalyzedTokenReadings {
            readings: vec![lt_core::AnalyzedToken::new(
                "",
                None,
                Some(crate::pipeline::sentence_start_tag().to_string()),
            )],
            chunk_tags: Vec::new(),
            whitespace_before: false,
            start_pos: 0,
            raw_byte_len: 0,
            is_whitespace: false,
            is_sentence_start: true,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged: false,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe: false,
            is_pos_tag_unknown: false,
        });
        tokens.extend(self.env.tagger.tag(&[joined_word.to_string()], true));
        !self.env.spelling.check_sentence(&tokens, 0).is_empty()
    }
}

impl RuleFilter for PotentialCompoundFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let part1 = ctx.args.get("part1").cloned().unwrap_or_default();
        let part2 = ctx.args.get("part2").cloned().unwrap_or_default();
        let plain = !lt_tagger::is_mixed_case(&part2) && !lt_tagger::is_all_uppercase(&part2);
        let part2lowercase = if plain {
            part2.to_lowercase()
        } else {
            part2.clone()
        };
        let part2capitalized = if plain {
            lt_tagger::uppercase_first_char(&part2.to_lowercase())
        } else {
            part2.clone()
        };
        let part1capitalized =
            if !lt_tagger::is_mixed_case(&part1) && !lt_tagger::is_all_uppercase(&part1) {
                lt_tagger::uppercase_first_char(&part1.to_lowercase())
            } else {
                part1.clone()
            };
        let joined_word = format!("{part1capitalized}{part2lowercase}");
        let hyphenated_word = format!("{part1capitalized}-{part2capitalized}");
        let mut replacements: Vec<String> = Vec::new();
        if !self.spelling_rejects(&joined_word) {
            if joined_word.chars().count() > 20 {
                replacements.push(hyphenated_word);
            }
            replacements.push(joined_word);
        } else {
            replacements.push(hyphenated_word);
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        let mut out = FilterOutcome::accept();
        out.suggestions = Some(
            replacements
                .into_iter()
                .map(|value| lt_core::Suggestion {
                    value,
                    short_description: None,
                })
                .collect(),
        );
        out
    }
}

/// `org.languagetool.rules.IsEnglishWordFilter`: accept when the form(s)
/// named by `formPositions` are English words (or match the `postags`
/// regexes).
struct IsEnglishWordFilter {
    /// `IsEnglishWordFilter` only reads the language's English tagger
    /// (`GermanyGerman.getTagger()` never provides one in the pinned build),
    /// so it needs no other German state. This lets the Simple German variant
    /// register it without building the German speller (D-309).
    english_tagger: Option<Arc<lt_tagger::EnglishTagger>>,
}

impl IsEnglishWordFilter {
    fn tagger(&self) -> Option<&lt_tagger::EnglishTagger> {
        self.english_tagger.as_deref()
    }

    fn is_tagged(&self, word: &str) -> bool {
        self.tagger()
            .map(|t| t.tag_word(word))
            .unwrap_or_default()
            .iter()
            .any(|r| {
                r.pos_tag
                    .as_deref()
                    .is_some_and(|t| t != "SENT_END" && t != "PARA_END")
            })
    }

    fn is_tagged_with(&self, word: &str, postag: &str) -> bool {
        let re = regex::Regex::new(&format!("^(?:{postag})$"));
        self.tagger()
            .map(|t| t.tag_word(word))
            .unwrap_or_default()
            .iter()
            .any(|r| {
                r.pos_tag
                    .as_deref()
                    .is_some_and(|t| re.as_ref().is_ok_and(|re| re.is_match(t)))
            })
    }
}

impl RuleFilter for IsEnglishWordFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        // Java `IsEnglishWordFilter.acceptRuleMatch`: `tagger == null` ->
        // return null (reject)
        if self.tagger().is_none() {
            return FilterOutcome::reject();
        }
        let Some(form_positions) = ctx.args.get("formPositions") else {
            return FilterOutcome::reject();
        };
        let mut forms: Vec<String> = Vec::new();
        for form_position in form_positions.split(',') {
            let Ok(position) = form_position.parse::<i64>() else {
                return FilterOutcome::reject();
            };
            let corrected = lt_pattern::skip_corrected_reference(ctx.token_positions, position);
            let Some(token) = usize::try_from(corrected)
                .ok()
                .and_then(|i| ctx.pattern_tokens.get(i))
            else {
                return FilterOutcome::reject();
            };
            forms.push(token.surface().to_string());
        }
        let is_english = match ctx.args.get("postags") {
            Some(postags_str) => {
                let postags: Vec<&str> = postags_str.split(',').collect();
                if postags.len() != forms.len() {
                    return FilterOutcome::reject();
                }
                forms
                    .iter()
                    .zip(postags)
                    .all(|(form, postag)| self.is_tagged_with(form, postag))
            }
            None => forms.iter().all(|form| self.is_tagged(form)),
        };
        if is_english {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

/// `org.languagetool.rules.WhitespaceCheckFilter`: accept unless the token
/// before `position` is preceded by exactly `whitespaceChar` (Java compares
/// `AnalyzedTokenReadings.getWhitespaceBefore()`, the previous *raw* token).
struct WhitespaceCheckFilter;

impl RuleFilter for WhitespaceCheckFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(ws_char) = ctx.args.get("whitespaceChar") else {
            return FilterOutcome::reject();
        };
        let Some(pos) = ctx
            .args
            .get("position")
            .and_then(|s| s.parse::<usize>().ok())
        else {
            return FilterOutcome::reject();
        };
        if pos < 1 || pos > ctx.pattern_tokens.len() {
            return FilterOutcome::reject();
        }
        let token = ctx.pattern_tokens[pos - 1];
        // reconstruct the previous raw token from the sentence text (the
        // engine does not keep `whitespaceBeforeChar`)
        let prefix = ctx.sentence_text.get(..token.start_pos).unwrap_or_default();
        let prev_token = match prefix.chars().last() {
            Some(last) if last.is_whitespace() => {
                let start = prefix
                    .char_indices()
                    .rev()
                    .find(|(_, c)| !c.is_whitespace())
                    .map(|(i, _)| i + prefix[i..].chars().next().map(char::len_utf8).unwrap_or(0))
                    .unwrap_or(0);
                &prefix[start..]
            }
            Some(_) => {
                let start = prefix
                    .char_indices()
                    .rev()
                    .find(|(_, c)| c.is_whitespace())
                    .map(|(i, _)| i + prefix[i..].chars().next().map(char::len_utf8).unwrap_or(0))
                    .unwrap_or(0);
                &prefix[start..]
            }
            None => "",
        };
        if prev_token != ws_char {
            FilterOutcome::accept()
        } else {
            FilterOutcome::reject()
        }
    }
}

/// `org.languagetool.rules.de.GermanNumberInWordFilter`
/// (`AbstractNumberInWordFilter` with the German speller).
struct GermanNumberInWordFilter {
    env: Env,
}

impl RuleFilter for GermanNumberInWordFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(word) = ctx.args.get("word") else {
            return FilterOutcome::reject();
        };
        let word_replacing_zero_o = word.replace('0', "o");
        let word_without_number: String = word.chars().filter(|c| !c.is_ascii_digit()).collect();
        let mut replacements: Vec<String> = Vec::new();
        if !self.env.spelling.is_misspelled(&word_replacing_zero_o)
            && *word != word_replacing_zero_o
        {
            replacements.push(word_replacing_zero_o);
        }
        if !self.env.spelling.is_misspelled(&word_without_number) {
            replacements.push(word_without_number.clone());
        }
        if replacements.is_empty() {
            replacements.extend(self.env.spelling.suggestions(&word_without_number));
        }
        if replacements.is_empty() {
            FilterOutcome::reject()
        } else {
            FilterOutcome {
                accepted: true,
                range: None,
                message: None,
                suggestions: Some(
                    replacements
                        .into_iter()
                        .map(|value| lt_core::Suggestion {
                            value,
                            short_description: None,
                        })
                        .collect(),
                ),
            }
        }
    }
}

/// `org.languagetool.rules.de.AdvancedSynthesizerFilter`
/// (`AbstractAdvancedSynthesizerFilter`).
struct AdvancedSynthesizerFilter {
    env: Env,
}

impl AdvancedSynthesizerFilter {
    /// `getAnalyzedToken`: the first reading whose POS tag fully matches
    /// `regexp` (null tags are tested as `UNKNOWN`).
    fn analyzed_token<'a>(
        &self,
        token: &'a lt_core::AnalyzedTokenReadings,
        regexp: &str,
    ) -> Option<&'a lt_core::AnalyzedToken> {
        let re = regex::Regex::new(&format!("^(?:{regexp})$")).ok()?;
        token
            .readings
            .iter()
            .find(|r| {
                let pos_tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
                re.is_match(pos_tag)
            })
            .or_else(|| token.readings.first())
    }

    /// `getCompositePostag`.
    fn composite_postag(
        &self,
        lemma_select: &str,
        postag_select: &str,
        original_postag: &str,
        desired_postag: &str,
        postag_replace: &str,
    ) -> String {
        let a_pattern = regex::Regex::new(&format!("^(?:{lemma_select})$")).ok();
        let b_pattern = regex::Regex::new(&format!("^(?:{postag_select})$")).ok();
        let mut result = postag_replace.to_string();
        let (Some(a_pattern), Some(b_pattern)) = (a_pattern, b_pattern) else {
            return result;
        };
        let (Some(a_caps), Some(b_caps)) = (
            a_pattern.captures(original_postag),
            b_pattern.captures(desired_postag),
        ) else {
            return result;
        };
        for i in 1..a_caps.len() {
            if let Some(group) = a_caps.get(i) {
                result = result.replace(&format!("\\a{i}"), group.as_str());
            }
        }
        for i in 1..b_caps.len() {
            if let Some(group) = b_caps.get(i) {
                result = result.replace(&format!("\\b{i}"), group.as_str());
            }
        }
        result
    }
}

impl RuleFilter for AdvancedSynthesizerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let Some(postag_select) = ctx.args.get("postagSelect") else {
            return FilterOutcome::reject();
        };
        let Some(lemma_select) = ctx.args.get("lemmaSelect") else {
            return FilterOutcome::reject();
        };
        let Some(postag_from_str) = ctx.args.get("postagFrom") else {
            return FilterOutcome::reject();
        };
        let Some(lemma_from_str) = ctx.args.get("lemmaFrom") else {
            return FilterOutcome::reject();
        };
        let new_lemma = ctx.args.get("newLemma").cloned().unwrap_or_default();
        let resolve = |s: &str| -> Option<usize> {
            if s.starts_with("marker") {
                let mut pos = 0usize;
                while pos < ctx.pattern_tokens.len()
                    && ctx.pattern_tokens[pos].start_pos < ctx.match_range.start
                {
                    pos += 1;
                }
                pos += 1;
                if s.len() > 6 {
                    pos += s.replace("marker", "").parse::<usize>().ok()?;
                }
                Some(pos)
            } else {
                s.parse::<usize>().ok()
            }
        };
        let Some(postag_from) = resolve(postag_from_str) else {
            return FilterOutcome::reject();
        };
        let Some(lemma_from) = resolve(lemma_from_str) else {
            return FilterOutcome::reject();
        };
        if postag_from < 1
            || postag_from > ctx.pattern_tokens.len()
            || lemma_from < 1
            || lemma_from > ctx.pattern_tokens.len()
        {
            return FilterOutcome::reject();
        }
        let postag_replace = ctx.args.get("postagReplace");
        let lemma_token = ctx.pattern_tokens[lemma_from - 1];
        let Some(mut desired_lemma) = self
            .analyzed_token(lemma_token, lemma_select)
            .and_then(|t| t.stem.clone())
        else {
            return FilterOutcome::reject();
        };
        let original_postag = self
            .analyzed_token(lemma_token, lemma_select)
            .and_then(|t| t.pos_tag.clone())
            .unwrap_or_default();
        let Some(desired_postag) = self
            .analyzed_token(ctx.pattern_tokens[postag_from - 1], postag_select)
            .and_then(|t| t.pos_tag.clone())
        else {
            return FilterOutcome::reject();
        };
        if !new_lemma.is_empty() && !new_lemma.starts_with('_') {
            desired_lemma = new_lemma;
        }
        let desired_postag = match postag_replace {
            Some(replace) => self.composite_postag(
                lemma_select,
                postag_select,
                &original_postag,
                &desired_postag,
                replace,
            ),
            None => desired_postag,
        };
        // take capitalization from the lemma (?)
        let lemma_surface = lemma_token.surface();
        let is_word_capitalized = lt_tagger::is_capitalized_word(lemma_surface);
        let is_word_allupper = lt_tagger::is_all_uppercase(lemma_surface);
        let token = lt_core::AnalyzedToken::new("", Some(desired_lemma), None);
        let replacements = self
            .env
            .synth
            .inner()
            .synthesize(&token, &desired_postag, true);
        if replacements.is_empty() {
            return FilterOutcome::accept();
        }
        let mut replacements_list: Vec<String> = Vec::new();
        let mut suggestion_used = false;
        for r in &ctx.suggestions {
            for nr in &replacements {
                if r.value.contains("{suggestion}")
                    || r.value.contains("{Suggestion}")
                    || r.value.contains("{SUGGESTION}")
                {
                    suggestion_used = true;
                }
                let mut nr = nr.clone();
                if is_word_capitalized {
                    nr = lt_tagger::uppercase_first_char(&nr);
                }
                if is_word_allupper {
                    nr = nr.to_uppercase();
                }
                let complete = r
                    .value
                    .replace("{suggestion}", &nr)
                    .replace("{Suggestion}", &lt_tagger::uppercase_first_char(&nr))
                    .replace("{SUGGESTION}", &nr.to_uppercase());
                if !replacements_list.contains(&complete) {
                    replacements_list.push(complete);
                }
            }
        }
        if !suggestion_used {
            replacements_list.extend(replacements);
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(
                replacements_list
                    .into_iter()
                    .map(|value| lt_core::Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect(),
            ),
        }
    }
}

/// `org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter` with
/// the German `MultitokenSpeller` (`GermanMultitokenSpeller.isException`).
struct GermanMultitokenSpellerFilter {
    env: Env,
}

impl GermanMultitokenSpellerFilter {
    /// `MultitokenSpellerFilter.isMisspelled(String, Language)`.
    fn is_misspelled(&self, text: &str) -> bool {
        for token in lt_tokenize::GermanWordTokenizer::new().tokenize(text) {
            if token.trim().is_empty() {
                continue;
            }
            if self.env.spelling.is_misspelled(&token) {
                return true;
            }
        }
        false
    }
}

impl RuleFilter for GermanMultitokenSpellerFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        if ctx.pattern_tokens.iter().all(|t| t.is_ignore_spelling) {
            return FilterOutcome::reject();
        }
        let underlined_error = &ctx.sentence_text[ctx.match_range.start..ctx.match_range.end];
        let are_tokens_accepted_by_speller = !self.is_misspelled(underlined_error);
        let mut replacements = self
            .env
            .multitoken
            .suggestions(underlined_error, are_tokens_accepted_by_speller);
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        if underlined_error.chars().count() > 4 && lt_tagger::is_all_uppercase(underlined_error) {
            let mut all_upper: Vec<String> = Vec::new();
            for replacement in replacements {
                let new_replacement = replacement.to_uppercase();
                if !all_upper.contains(&new_replacement) && new_replacement != underlined_error {
                    all_upper.push(new_replacement);
                }
            }
            replacements = all_upper;
        } else {
            // capitalize suggestions when the error starts the sentence
            let non_blank: Vec<&lt_core::AnalyzedTokenReadings> = ctx
                .sentence_tokens
                .iter()
                .copied()
                .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
                .collect();
            let mut words_start_pos = 1usize;
            while words_start_pos < non_blank.len()
                && (is_punctuation_mark(non_blank[words_start_pos].surface())
                    || is_not_word_string(non_blank[words_start_pos].surface()))
            {
                words_start_pos += 1;
            }
            if ctx.pattern_token_pos == words_start_pos {
                let mut capitalized: Vec<String> = Vec::new();
                for replacement in replacements {
                    let mut new_replacement = replacement.clone();
                    if replacement == replacement.to_lowercase() {
                        // do not capitalize iPad
                        new_replacement = lt_tagger::uppercase_first_char(&replacement);
                    }
                    if !capitalized.contains(&new_replacement)
                        && new_replacement != underlined_error
                    {
                        capitalized.push(new_replacement);
                    }
                }
                replacements = capitalized;
            }
        }
        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(
                replacements
                    .into_iter()
                    .map(|value| lt_core::Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect(),
            ),
        }
    }
}

fn is_punctuation_mark(s: &str) -> bool {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.is_ascii_punctuation() || c == '’' || is_unicode_punct(c),
        _ => false,
    }
}

fn is_unicode_punct(c: char) -> bool {
    // approximation of Java \p{IsPunctuation} (Pc, Pd, Ps, Pe, Pi, Pf, Po)
    matches!(
        c,
        '\u{00A1}'..='\u{00BF}' | '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{205E}'
    ) && !c.is_alphanumeric()
}

fn is_not_word_string(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| !c.is_alphabetic())
}

/// `org.languagetool.rules.de.GermanSuppressMisspelledSuggestionsFilter`
/// (`AbstractSuppressMisspelledSuggestionsFilter`); not referenced by the
/// pinned German rule data, ported for completeness.
struct GermanSuppressMisspelledSuggestionsFilter {
    env: Env,
}

impl GermanSuppressMisspelledSuggestionsFilter {
    fn is_misspelled_multiword(&self, word: &str) -> bool {
        for token in lt_tokenize::GermanWordTokenizer::new().tokenize(word) {
            if token.trim().is_empty() {
                continue;
            }
            if self.env.spelling.is_misspelled(&token) {
                return true;
            }
        }
        false
    }
}

impl RuleFilter for GermanSuppressMisspelledSuggestionsFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let suppress_match = ctx
            .args
            .get("suppressMatch")
            .map(|v| !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        let suppress_postag = ctx.args.get("SuppressPostag");
        let filter_postag = ctx.args.get("FilterPostag");
        let mut new_replacements: Vec<lt_core::Suggestion> = Vec::new();
        for replacement in &ctx.suggestions {
            if self.is_misspelled_multiword(&replacement.value) {
                continue;
            }
            let mut add = true;
            if suppress_postag.is_some() || filter_postag.is_some() {
                let readings = self
                    .env
                    .tagger
                    .tag(std::slice::from_ref(&replacement.value), true);
                let atr = readings.into_iter().next();
                let Some(atr) = atr else {
                    continue;
                };
                if let Some(re) =
                    suppress_postag.and_then(|p| regex::Regex::new(&format!("^(?:{p})$")).ok())
                {
                    if atr.has_pos_tag_matching(&re) {
                        add = false;
                    }
                }
                if add {
                    if let Some(re) =
                        filter_postag.and_then(|p| regex::Regex::new(&format!("^(?:{p})$")).ok())
                    {
                        if !atr.has_pos_tag_matching(&re) {
                            add = false;
                        }
                    }
                }
            }
            if add {
                new_replacements.push(replacement.clone());
            }
        }
        if new_replacements.is_empty() && suppress_match {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: None,
            message: None,
            suggestions: Some(new_replacements),
        }
    }
}

pub fn german_filter_registry(env: Env) -> FilterRegistry {
    let mut builder = FilterRegistry::builder();
    builder = builder.register(
        "org.languagetool.rules.DateRangeChecker",
        Arc::new(DateRangeChecker),
    );
    builder = builder.register(
        "org.languagetool.rules.de.RecentYearFilter",
        Arc::new(RecentYearFilter { today: env.today }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.UppercaseNounReadingFilter",
        Arc::new(UppercaseNounReadingFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.RemoveUnknownCompoundsFilter",
        Arc::new(RemoveUnknownCompoundsFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.ValidWordFilter",
        Arc::new(ValidWordFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.PotentialCompoundFilter",
        Arc::new(PotentialCompoundFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.CompoundCheckFilter",
        Arc::new(CompoundCheckFilter::load(&env.compound_check_path)),
    );
    builder = builder.register(
        "org.languagetool.rules.de.InsertCommaFilter",
        Arc::new(InsertCommaFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.IsEnglishWordFilter",
        Arc::new(IsEnglishWordFilter {
            english_tagger: env.english_tagger.clone(),
        }),
    );
    let today = env.today;
    builder = builder.register(
        "org.languagetool.rules.de.DateCheckFilter",
        Arc::new(crate::de::date_filters::DateCheckFilter { today }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.FutureDateFilter",
        Arc::new(crate::de::date_filters::FutureDateFilter { today }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.NewYearDateFilter",
        Arc::new(crate::de::date_filters::NewYearDateFilter { today }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.YMDNewYearDateFilter",
        Arc::new(crate::de::date_filters::YmdNewYearDateFilter { today }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.YMDDateCheckFilter",
        Arc::new(crate::de::date_filters::YmdDateCheckFilter { today }),
    );
    builder = builder.register(
        "org.languagetool.rules.WhitespaceCheckFilter",
        Arc::new(WhitespaceCheckFilter),
    );
    builder = builder.register(
        "org.languagetool.rules.de.GermanNumberInWordFilter",
        Arc::new(GermanNumberInWordFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.AdvancedSynthesizerFilter",
        Arc::new(AdvancedSynthesizerFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.spelling.multitoken.MultitokenSpellerFilter",
        Arc::new(GermanMultitokenSpellerFilter {
            env: Arc::clone(&env),
        }),
    );
    builder = builder.register(
        "org.languagetool.rules.de.GermanSuppressMisspelledSuggestionsFilter",
        Arc::new(GermanSuppressMisspelledSuggestionsFilter {
            env: Arc::clone(&env),
        }),
    );
    builder.build()
}

/// The minimal German filter registry the Simple German variant needs: only
/// `IsEnglishWordFilter`, which `de/disambiguation.xml` references 11 times
/// (reject-all in the pinned build, exactly like plain German, D-039). The
/// simple `grammar.xml` has no `<filter>` at all, so no speller-backed filter
/// is needed and the German speller is never built for the variant (D-309).
pub fn german_disambiguation_filter_registry() -> FilterRegistry {
    let mut builder = FilterRegistry::builder();
    builder = builder.register(
        "org.languagetool.rules.IsEnglishWordFilter",
        Arc::new(IsEnglishWordFilter {
            english_tagger: None,
        }),
    );
    builder.build()
}
