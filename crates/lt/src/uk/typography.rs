//! Port of `org.languagetool.rules.uk.TypographyRule` (`DASH`): a short dash
//! inside a word, or a dash without spaces between words.

use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const RULE_ID: &str = "DASH";
const DESCRIPTION: &str = "Коротка риска замість дефісу";
const SHORT: &str = "Коротка риска";
const MSG: &str = "Риска всередині слова. Всередині слова вживайте дефіс, між словами виокремлюйте риску пробілами.";
const CATEGORY_ID: &str = "TYPOGRAPHY";
const CATEGORY_NAME: &str = "Можлива механічна помилка";

static CYRILLIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^.*[а-яїієґА-ЯІЇЄҐ].*$").unwrap());
static SHORT_DASH_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[а-яіїєґ']{2,}(?:[\u{2013}\u{2014}][а-яіїєґ']{2,})+$").unwrap()
});
static BAD_LATIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[ХІXIV]+[\u{2013}\u{2014}][ХІXIV]+$").unwrap());

fn is_number(tr: &AnalyzedTokenReadings) -> bool {
    tr.readings.iter().any(|r| {
        r.pos_tag
            .as_deref()
            .is_some_and(|t| t.starts_with("number"))
    })
}

fn short_dash_token(tr: &AnalyzedTokenReadings) -> Option<String> {
    let token = tr.readings.last().map(|r| r.token.clone())?;
    let contains = token
        .char_indices()
        .any(|(i, c)| i > 0 && (c == '\u{2013}' || c == '\u{2014}'));
    (contains
        && SHORT_DASH_WORD.is_match(&token).unwrap_or(false)
        && !BAD_LATIN.is_match(&token).unwrap_or(false))
    .then_some(token)
}

/// `TypographyRule.match` over one sentence.
pub fn check_sentence_uk(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens.iter().filter(|t| !t.is_whitespace).collect();
    let mut out = Vec::new();
    for i in 1..view.len() {
        let tr = view[i];
        if let Some(short_dash) = short_dash_token(tr) {
            let replacements = vec![
                short_dash.replace(['\u{2013}', '\u{2014}'], "-"),
                short_dash.replace(['\u{2013}', '\u{2014}'], " \u{2014} "),
            ];
            out.push(
                Match::new(
                    RULE_ID,
                    Option::<String>::None,
                    MSG,
                    Some(SHORT.to_string()),
                    TextRange::new(
                        sentence_offset + tr.start_pos,
                        sentence_offset + tr.start_pos + tr.surface().len(),
                    ),
                    replacements
                        .into_iter()
                        .map(|value| Suggestion {
                            value,
                            short_description: None,
                        })
                        .collect(),
                    CATEGORY_ID,
                    CATEGORY_NAME,
                )
                .with_metadata(DESCRIPTION, "typographical", 0),
            );
            continue;
        }
        let surface = tr.surface();
        if surface != "\u{2014}" && surface != "\u{2013}" {
            continue;
        }
        let no_space_left = i > 1
            && !tr.whitespace_before
            && view[i - 1].surface() != ","
            && view[i - 1].surface() != "«";
        let no_space_right =
            i < view.len() - 1 && !view[i + 1].whitespace_before && view[i + 1].surface() != ">";
        if !(no_space_left || no_space_right) {
            continue;
        }
        if i > 1 && is_number(view[i - 1]) && i < view.len() - 1 && is_number(view[i + 1]) {
            continue;
        }
        let mut replacements = Vec::new();
        if i > 1
            && i < view.len() - 1
            && CYRILLIC.is_match(view[i - 1].surface()).unwrap_or(false)
            && CYRILLIC.is_match(view[i + 1].surface()).unwrap_or(false)
        {
            replacements.push(format!(
                "{}-{}",
                view[i - 1].surface(),
                view[i + 1].surface()
            ));
        }
        let start_pos = if i > 1 {
            view[i - 1].start_pos
        } else {
            tr.start_pos
        };
        let end_pos = if i < view.len() - 1 {
            view[i + 1].start_pos
        } else {
            tr.end_pos()
        };
        let mut repl = if i > 1 {
            format!("{} ", view[i - 1].surface())
        } else {
            String::new()
        };
        repl.push('\u{2014}');
        if i < view.len() - 1 {
            repl.push(' ');
            repl.push_str(view[i + 1].surface());
        }
        replacements.push(repl);
        out.push(
            Match::new(
                RULE_ID,
                Option::<String>::None,
                MSG,
                Some(SHORT.to_string()),
                TextRange::new(sentence_offset + start_pos, sentence_offset + end_pos),
                replacements
                    .into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect(),
                CATEGORY_ID,
                CATEGORY_NAME,
            )
            .with_metadata(DESCRIPTION, "typographical", 0),
        );
    }
    out
}
