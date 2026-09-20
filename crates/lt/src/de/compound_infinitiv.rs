//! `CompoundInfinitivRule` (`COMPOUND_INFINITIV_RULE`, checklist item 44).
//!
//! Checks separable-verb-style extended infinitives (`sicher zu gehen` →
//! `sicherzugehen`). The rule owns its own morfologik `Speller` over
//! `de/hunspell/de_DE.dict` like Java
//! (`MorfologikSpeller.getDictionaryWithCaching("/de/hunspell/de_DE.dict")`),
//! independent of the variant and of the spelling rule's hunspell backend.
//!
//! Antipatterns (`cacheAntiPatterns(lang, ANTI_PATTERNS)`) are compiled as
//! `lt-pattern` patterns and applied as token-view immunization like
//! `Rule.getSentenceWithImmunization`.
//!
//! `LinguServices` (LO/OO dictionary) is `null` in the harness; `linguServices
//! == null` takes Java's `Speller` branch.

use std::path::Path;
use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};
use lt_pattern::matcher as pm;
use lt_pattern::PatternToken;

pub const RULE_ID: &str = "COMPOUND_INFINITIV_RULE";
const DESCRIPTION: &str = "Erweiterter Infinitiv mit zu (Zusammenschreibung)";
const CATEGORY_ID: &str = "COMPOUNDING";
const CATEGORY_NAME: &str = "Getrennt- und Zusammenschreibung";

const ADJ_EXCEPTION: [&str; 12] = [
    "schwer", "klar", "verloren", "bekannt", "rot", "blau", "gelb", "grün", "schwarz", "weiß",
    "fertig", "neu",
];

pub struct CompoundInfinitivRule {
    /// morfologik dictionary (Java `new Speller(...)` in `match`)
    speller: lt_spell::SpellChecker,
    antipatterns: Vec<Arc<pm::CompiledPattern>>,
}

impl CompoundInfinitivRule {
    /// Load the de_DE morfologik dictionary and compile `ANTI_PATTERNS`.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dir = data_dir.join("de/hunspell");
        let speller =
            lt_spell::SpellChecker::new(&dir.join("de_DE.dict"), &dir.join("de_DE.info"))?;
        Ok(Self {
            speller,
            antipatterns: compile_antipatterns(),
        })
    }

    /// `CompoundInfinitivRule.match` over one sentence.
    pub fn match_sentence(
        &self,
        sentence: &AnalyzedSentence,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let immunized = super::util::immunize_sentence(sentence, &self.antipatterns);
        let view = immunized.tokens_without_whitespace();
        for i in 2..view.len().saturating_sub(1) {
            if view[i].surface() == "zu"
                && is_infinitiv(view[i + 1])
                && is_relevant(view[i - 1])
                && !view[i].is_immunized
                && !self.is_exception(&view, i)
                && !self.is_misspelled(&format!(
                    "{}{}",
                    view[i - 1].surface(),
                    view[i + 1].surface()
                ))
            {
                let msg = format!(
                    "Wenn der erweiterte Infinitiv von dem Verb '{}{}' abgeleitet ist, sollte er zusammengeschrieben werden.",
                    view[i - 1].surface(),
                    view[i + 1].surface()
                );
                let suggestion = format!(
                    "{}{}{}",
                    view[i - 1].surface(),
                    view[i].surface(),
                    view[i + 1].surface()
                );
                rule_matches.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        msg,
                        Option::<String>::None,
                        TextRange::new(
                            sentence_offset + view[i - 1].start_pos,
                            sentence_offset + view[i + 1].end_pos(),
                        ),
                        vec![Suggestion {
                            value: suggestion,
                            short_description: None,
                        }],
                        CATEGORY_ID,
                        CATEGORY_NAME,
                    )
                    .with_metadata(DESCRIPTION, "misspelling", 0),
                );
            }
        }
        rule_matches
    }

    /// `CompoundInfinitivRule.isMisspelled`.
    fn is_misspelled(&self, word: &str) -> bool {
        let word = lt_tagger::lowercase_first_char(word);
        !self.speller.is_correct(&word)
    }

    /// `CompoundInfinitivRule.isException`.
    fn is_exception(&self, tokens: &[&AnalyzedTokenReadings], n: usize) -> bool {
        if tokens[n - 2].has_pos_tag_starting_with("VER") {
            return true;
        }
        for word in ADJ_EXCEPTION {
            if tokens[n - 1].surface() == word {
                return true;
            }
        }
        if tokens[n + 1].surface() == "sagen"
            && matches!(tokens[n - 1].surface(), "weiter" | "dazu")
        {
            return true;
        }
        if matches!(tokens[n + 1].surface(), "tragen" | "machen")
            && tokens[n - 1].surface() == "davon"
        {
            return true;
        }
        if tokens[n + 1].surface() == "geben" && tokens[n - 1].surface() == "daran" {
            return true;
        }
        if tokens[n + 1].surface() == "gehen" && tokens[n - 1].surface() == "ab" {
            return true;
        }
        if tokens[n + 1].surface() == "errichten" && tokens[n - 1].surface() == "wieder" {
            return true;
        }
        let mut verb: Option<String> = None;
        let mut i = n as isize - 2;
        while i > 0 && !is_punctuation(tokens[i as usize].surface()) {
            let token = tokens[i as usize];
            if token.has_pos_tag_starting_with("VER:IMP") {
                verb = get_lemma(token).map(|l| l.to_lowercase());
            } else if token.has_pos_tag_starting_with("VER") {
                verb = Some(token.surface().to_lowercase());
            } else if token.surface() == "Fang" {
                verb = Some("fangen".to_string());
            }
            if let Some(verb) = &verb {
                if !self.is_misspelled(&format!("{}{}", tokens[n - 1].surface(), verb)) {
                    return true;
                }
                break;
            }
            i -= 1;
        }
        if matches!(tokens[n - 1].surface(), "aus" | "an") {
            let mut i = n as isize - 2;
            while i > 0 && !is_punctuation(tokens[i as usize].surface()) {
                if matches!(tokens[i as usize].surface(), "von" | "vom") {
                    return true;
                }
                i -= 1;
            }
        }
        if tokens[n - 1].surface() == "her" {
            let mut i = n as isize - 2;
            while i > 0 && !is_punctuation(tokens[i as usize].surface()) {
                if tokens[i as usize].surface() == "vor" {
                    return true;
                }
                i -= 1;
            }
        }
        false
    }
}

/// `CompoundInfinitivRule.isInfinitiv`.
fn is_infinitiv(token: &AnalyzedTokenReadings) -> bool {
    token.has_pos_tag_starting_with("VER:INF")
}

/// `CompoundInfinitivRule.isRelevant`.
fn is_relevant(token: &AnalyzedTokenReadings) -> bool {
    token.has_pos_tag("ZUS") && !token.surface().eq_ignore_ascii_case("um")
}

/// `CompoundInfinitivRule.getLemma`: first non-null lemma.
fn get_lemma(token: &AnalyzedTokenReadings) -> Option<String> {
    token.readings.iter().find_map(|r| r.stem.clone())
}

/// `CompoundInfinitivRule.isPunctuation`.
fn is_punctuation(word: &str) -> bool {
    word.chars().count() == 1
        && matches!(
            word,
            "." | "?" | "!" | "…" | ":" | ";" | "," | "(" | ")" | "[" | "]"
        )
}

fn token(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        ..Default::default()
    }
}

fn token_regex(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        regexp: true,
        ..Default::default()
    }
}

fn pos_regex(postag: &str) -> PatternToken {
    PatternToken {
        postag: Some(postag.to_string()),
        postag_regexp: true,
        ..Default::default()
    }
}

fn min0(mut t: PatternToken) -> PatternToken {
    t.min = Some(0);
    t
}

fn skip(mut t: PatternToken, value: i32) -> PatternToken {
    t.skip = Some(value);
    t
}

/// `CompoundInfinitivRule.ANTI_PATTERNS` (order matters for immunization).
fn antipattern_defs() -> Vec<Vec<PatternToken>> {
    vec![
        // NOTE: antipatterns only work when they cover "zu":
        vec![
            token("auf"),
            token("Nummer"),
            token("sicher"),
            token("zu"),
        ],
        vec![
            token("ab"),
            token_regex("und|&"),
            token("an"),
            token("zu"),
        ],
        vec![token("ganz"), token("schön"), token("zu")],
        vec![
            token_regex("fang|fängst|fängt|fangt|fangen|fing|fingen"),
            pos_regex("ADV.*"),
            token("an"),
            token("zu"),
        ],
        vec![token_regex("dazu|darüber"), token("zu"), token("machen")],
        vec![token("schön"), token("zu"), token("machen")],
        vec![token("kurz"), token("davor"), token("zu")],
        vec![token_regex("Jahr|Monat|Zeit"), token("über"), token("zu")],
        vec![token("endlich"), token("wieder"), token("zu")],
        vec![token("bis"), token("hin"), token("zu")],
        vec![
            token("von"),
            token_regex(".*[a-z].*"),
            token("her"),
            token("zu"),
        ],
        vec![
            token_regex("sehr|ganz|äu(ss|ß)erst|zu|nicht|absolut|total|wirklich|möglichst"),
            pos_regex("ADJ.*"),
            token("zu"),
        ],
        vec![token("Schritt"), token("weiter"), token("zu")],
        vec![token("und"), token("so"), token("weiter")],
        vec![
            token("darauf"),
            token("zu"),
            pos_regex("VER.*"),
            token("dass"),
        ],
        vec![
            token("darauf"),
            token("zu"),
            pos_regex("VER.*"),
            token(","),
        ],
        vec![
            token_regex("Spiel|Tag|Nacht|Morgen|Nachmittag|Abend|Zeit|.+zeit|Jahr(zehnt)?|Monat|.+tag|Mittwoch|Januar|Februar|März|April|Mai|Juni|Juli|August|September|Oktober|November|Dezember"),
            token("über"),
            token("zu"),
        ],
        vec![token("kurz"), token("zu"), token("machen")],
        vec![token("dazu"), token("zu"), token("haben")],
        vec![
            token_regex("deutlich|viel|Stück|nichts|nix|noch"),
            token("weiter"),
            token("zu"),
        ],
        vec![
            token("auf"),
            token_regex("und|&|oder|\\/"),
            min0(pos_regex("ADV.*")),
            token("ab"),
            token("zu"),
        ],
        vec![token("zu"), pos_regex("ADJ.*"), token("zu")],
        vec![
            token("hin"),
            token_regex("und|&|oder|\\/"),
            min0(pos_regex("ADV.*")),
            token("her"),
            token("zu"),
        ],
        vec![
            token_regex("rauf|hoch"),
            token_regex("und|&|oder|\\/"),
            min0(pos_regex("ADV.*")),
            token("runter"),
            token("zu"),
        ],
        vec![
            skip(token("aus"), 3),
            token("heraus"),
            token("zu"),
        ],
    ]
}

fn compile_antipatterns() -> Vec<Arc<pm::CompiledPattern>> {
    antipattern_defs()
        .iter()
        .flat_map(|tokens| pm::compile_patterns(tokens, None, None).unwrap_or_default())
        .map(Arc::new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_all_antipatterns() {
        assert_eq!(compile_antipatterns().len(), antipattern_defs().len());
    }

    #[test]
    fn recognizes_punctuation() {
        assert!(is_punctuation(","));
        assert!(is_punctuation("…"));
        assert!(!is_punctuation(".."));
        assert!(!is_punctuation("zu"));
    }
}
