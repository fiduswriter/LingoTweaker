//! `AbstractStatisticSentenceStyleRule` and its German subclasses
//! (`PASSIVE_SENTENCE_DE`, `SENTENCE_WITH_MODAL_VERB_DE`,
//! `SENTENCE_WITH_MAN_DE`, `SENTENCE_BEGINNING_WITH_CONJUNCTION_DE`).
//!
//! All four are default-off stylistic statistics over the whole text: they
//! only emit matches when the share of matching sentences exceeds the
//! per-rule limit (8% / 18% / 15 per mil / 10%).

use std::sync::LazyLock;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, TextRange};

const CATEGORY_ID: &str = "CREATIVE_WRITING";
const CATEGORY_NAME: &str = "Stiltipps für kreatives Schreiben";

static OPENING_QUOTES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["\"", "“", "„", "»", "«"]);
static ENDING_QUOTES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["\"", "“", "”", "»", "«"]);

/// `AbstractStatisticSentenceStyleRule.isMark` (`[,;.:?•!-–—]`).
fn is_mark(token: &AnalyzedTokenReadings) -> bool {
    let mut chars = token.surface().chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => matches!(c, ',' | ';' | '.' | ':' | '?' | '•' | '!' | '-' | '–' | '—'),
        _ => false,
    }
}

fn has_lemma(token: &AnalyzedTokenReadings, lemma: &str) -> bool {
    token
        .readings
        .iter()
        .any(|r| r.stem.as_deref() == Some(lemma))
}

pub struct SentenceStyleConfig {
    pub id: &'static str,
    pub description: &'static str,
    pub min_percent: u32,
    pub denominator: f64,
    /// returns the index (into the relevant sentence part) of the hint token
    pub condition: fn(&[&AnalyzedTokenReadings]) -> Option<usize>,
    pub limit_message: fn(u32, f64) -> String,
}

/// `AbstractStatisticSentenceStyleRule.match`.
fn check_with(sentences: &[AnalyzedSentence], config: &SentenceStyleConfig) -> Vec<Match> {
    let mut hints: Vec<(usize, usize, usize)> = Vec::new(); // (sentence idx, start, end)
    let mut sentence_count = 0u32;
    let mut is_direct_speech = false;
    for (sentence_index, sentence) in sentences.iter().enumerate() {
        let tokens = sentence.tokens_without_whitespace();
        let mut relevant: Vec<&AnalyzedTokenReadings> = Vec::new();
        let mut is_sentence_count = false;
        let mut found: Option<usize> = None;
        for n in 1..tokens.len() {
            let token = tokens[n];
            if !is_direct_speech
                && OPENING_QUOTES.contains(&token.surface())
                && n < tokens.len() - 1
                && !tokens[n + 1].whitespace_before
            {
                is_direct_speech = true;
                if !relevant.is_empty() {
                    is_sentence_count = true;
                    found = (config.condition)(&relevant);
                    if found.is_some() {
                        break;
                    }
                }
            } else if is_direct_speech
                && ENDING_QUOTES.contains(&token.surface())
                && n > 1
                && !token.whitespace_before
            {
                is_direct_speech = false;
                relevant.clear();
            } else if (!is_direct_speech || config.min_percent == 0) && !token.is_whitespace {
                relevant.push(token);
            }
            if n == tokens.len() - 1 && !relevant.is_empty() {
                is_sentence_count = true;
                found = (config.condition)(&relevant);
            }
        }
        if is_sentence_count {
            sentence_count += 1;
        }
        if let Some(index) = found {
            let token = relevant[index];
            hints.push((sentence_index, token.start_pos, token.end_pos()));
        }
    }
    let num_matches = hints.len() as f64;
    let percent = if sentence_count > 0 {
        num_matches * config.denominator / sentence_count as f64
    } else {
        0.0
    };
    if percent <= config.min_percent as f64 {
        return Vec::new();
    }
    let message = (config.limit_message)(config.min_percent, percent);
    hints
        .into_iter()
        .map(|(sentence_index, start, end)| {
            let sentence = &sentences[sentence_index];
            Match::new(
                config.id,
                Option::<String>::None,
                message.clone(),
                Option::<String>::None,
                TextRange::new(sentence.offset + start, sentence.offset + end),
                Vec::new(),
                CATEGORY_ID,
                CATEGORY_NAME,
            )
            .with_metadata(config.description, "style", 0)
        })
        .collect()
}

fn is_modal_verb(token: &AnalyzedTokenReadings) -> bool {
    token.has_pos_tag_starting_with("VER:MOD")
}

/// `PASSIVE_SENTENCE_DE` (limit 8%).
pub fn passive_sentence(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        &SentenceStyleConfig {
            id: "PASSIVE_SENTENCE_DE",
            description: "Statistische Stilanalyse: Passivsätze",
            min_percent: 8,
            denominator: 100.0,
            condition: |sentence| {
                let mut i = 0usize;
                while i < sentence.len() {
                    if has_lemma(sentence[i], "werden") {
                        i += 1;
                        while i < sentence.len() {
                            if sentence[i].has_pos_tag_starting_with("VER:PA2:") {
                                return Some(i - 1);
                            } else if is_mark(sentence[i]) {
                                return None;
                            }
                            i += 1;
                        }
                    } else if sentence[i].has_pos_tag_starting_with("VER:PA2:") {
                        i += 1;
                        while i < sentence.len() {
                            if has_lemma(sentence[i], "werden") {
                                return Some(i);
                            } else if is_mark(sentence[i]) {
                                return None;
                            }
                            i += 1;
                        }
                    }
                    i += 1;
                }
                None
            },
            limit_message: |limit, percent| {
                if limit == 0 {
                    "Passivsatz: Aktiv formulierte Sätze sprechen im Regelfall den Leser stärker an."
                        .to_string()
                } else {
                    format!(
                        "Mehr als {limit}% Passivsätze {{{}%}} gefunden. Aktiv formulierte Sätze sprechen im Regelfall den Leser stärker an.",
                        (percent + 0.5) as i64
                    )
                }
            },
        },
    )
}

/// `SENTENCE_WITH_MODAL_VERB_DE` (limit 18%).
pub fn sentence_with_modal_verb(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        &SentenceStyleConfig {
            id: "SENTENCE_WITH_MODAL_VERB_DE",
            description: "Statistische Stilanalyse: Sätze mit Modalverb",
            min_percent: 18,
            denominator: 100.0,
            condition: |sentence| {
                let mut i = 0usize;
                while i < sentence.len() {
                    if is_modal_verb(sentence[i]) {
                        i += 1;
                        while i < sentence.len() {
                            if sentence[i].has_pos_tag_starting_with("VER:INF") {
                                return Some(i - 1);
                            } else if is_mark(sentence[i]) {
                                return None;
                            }
                            i += 1;
                        }
                    } else if sentence[i].has_pos_tag_starting_with("VER:INF") {
                        i += 1;
                        while i < sentence.len() {
                            if is_modal_verb(sentence[i]) {
                                return Some(i);
                            } else if is_mark(sentence[i]) {
                                return None;
                            }
                            i += 1;
                        }
                    }
                    i += 1;
                }
                None
            },
            limit_message: |limit, percent| {
                if limit == 0 {
                    "Modalverb: Modalverben blähen den Text häufig auf und sollten vermieden werden."
                        .to_string()
                } else {
                    format!(
                        "Mehr als {limit}% Sätze mit Modalverben {{{}%}} gefunden. Modalverben blähen den Text häufig auf und sollten vermieden werden.",
                        (percent + 0.5) as i64
                    )
                }
            },
        },
    )
}

/// `SENTENCE_WITH_MAN_DE` (limit 15 per mil).
pub fn sentence_with_man(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        &SentenceStyleConfig {
            id: "SENTENCE_WITH_MAN_DE",
            description: "Statistische Stilanalyse: Sätze mit indirekter Leseransprache 'man'",
            min_percent: 15,
            denominator: 1000.0,
            condition: |sentence| sentence.iter().position(|t| has_lemma(t, "man")),
            limit_message: |limit, percent| {
                if limit == 0 {
                    "Sätze mit der indirekten Leseransprache 'man' sind stilistisch wenig elegant formuliert. Lässt sich das Wort vermeiden?".to_string()
                } else {
                    format!(
                        "Mehr als {limit}‰ Sätze mit der indirekten Leseransprache 'man' {{{}‰}} gefunden. Lässt sich das Wort vermeiden?",
                        (percent + 0.5) as i64
                    )
                }
            },
        },
    )
}

/// `SENTENCE_BEGINNING_WITH_CONJUNCTION_DE` (limit 10%).
pub fn conjunction_at_begin_of_sentence(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        &SentenceStyleConfig {
            id: "SENTENCE_BEGINNING_WITH_CONJUNCTION_DE",
            description: "Statistische Stilanalyse: Sätze beginnend mit Konjunktion",
            min_percent: 10,
            denominator: 100.0,
            condition: |sentence| {
                if sentence.len() < 3 {
                    return None;
                }
                let mut num = 0usize;
                if OPENING_QUOTES.contains(&sentence[0].surface()) {
                    num += 1;
                }
                let token = sentence.get(num)?;
                if !token.has_pos_tag_starting_with("KON") {
                    return None;
                }
                let text = token.surface();
                if text == "Wie" || text == "Seit" || text == "Allerdings" {
                    return None;
                }
                if text == "Aber" && sentence[num + 1].surface() == "auch" {
                    return None;
                }
                if text == "Um" {
                    for token in sentence.iter().skip(1) {
                        if token.surface() == "," || token.surface() == "herum" {
                            return None;
                        }
                    }
                    return Some(num);
                }
                // the third disjunct is the legacy engine's (odd) handling of
                // "Auch wenn": it still fires unless the sentence is a question
                if !token.has_pos_tag_starting_with("KON:UNT")
                    || text == "Sondern"
                    || (text == "Auch" && sentence[num + 1].surface() == "wenn")
                {
                    if text == "Entweder" {
                        for token in sentence.iter().skip(1) {
                            if token.surface() == "oder" {
                                return None;
                            }
                        }
                    } else if text == "Sowohl" {
                        for (i, token) in sentence.iter().enumerate().skip(1) {
                            if i + 1 < sentence.len()
                                && token.surface() == "als"
                                && sentence[i + 1].surface() == "auch"
                            {
                                return None;
                            }
                        }
                    } else if text == "Weder" {
                        for token in sentence.iter().skip(1) {
                            if token.surface() == "noch" {
                                return None;
                            }
                        }
                    } else {
                        if sentence[sentence.len() - 1].surface() == "?" {
                            return None;
                        }
                        return Some(num);
                    }
                }
                for token in sentence.iter().skip(2) {
                    if token.surface() == "," {
                        return None;
                    }
                }
                Some(num)
            },
            limit_message: |limit, percent| {
                if limit == 0 {
                    "Eine Konjunktion sollte nur in Ausnahmefällen am Satzanfang verwendet werden. Formulieren Sie den Satz um, falls möglich.".to_string()
                } else {
                    format!(
                        "Mehr als {limit}% Sätze beginnen mit einer Konjunktion {{{}%}} gefunden. Formulieren Sie den Satz um, falls möglich.",
                        (percent + 0.5) as i64
                    )
                }
            },
        },
    )
}

// ---------------------------------------------------------------------------
// `AbstractStatisticStyleRule` (word-based) and its German subclasses
// ---------------------------------------------------------------------------

use std::collections::HashSet;

/// `GermanFillerWordsRule.fillerWords` (219 entries).
const FILLER_WORDS: [&str; 219] = [
    "aber",
    "abermals",
    "allein",
    "allemal",
    "allenfalls",
    "allenthalben",
    "allerdings",
    "allesamt",
    "allzu",
    "also",
    "alt",
    "andauernd",
    "andererseits",
    "andernfalls",
    "anscheinend",
    "auch",
    "auffallend",
    "augenscheinlich",
    "ausdrücklich",
    "ausgerechnet",
    "ausnahmslos",
    "außerdem",
    "äußerst",
    "beinahe",
    "bekanntlich",
    "bereits",
    "besonders",
    "bestenfalls",
    "bestimmt",
    "bloß",
    "dabei",
    "dadurch",
    "dafür",
    "dagegen",
    "daher",
    "damals",
    "danach",
    "demgegenüber",
    "demgemäß",
    "demnach",
    "denkbar",
    "denn",
    "dennoch",
    "deshalb",
    "deswegen",
    "doch",
    "durchaus",
    "durchweg",
    "eben",
    "eigentlich",
    "einerseits",
    "einfach",
    "einige",
    "einigermaßen",
    "einmal",
    "ergo",
    "erheblich",
    "etliche",
    "etwa",
    "etwas",
    "fast",
    "folgendermaßen",
    "folglich",
    "förmlich",
    "fortwährend",
    "fraglos",
    "freilich",
    "ganz",
    "gänzlich",
    "gar",
    "gelegentlich",
    "gemeinhin",
    "genau",
    "geradezu",
    "gewiss",
    "gewissermaßen",
    "glatt",
    "gleichsam",
    "gleichwohl",
    "glücklicherweise",
    "gottseidank",
    "größtenteils",
    "häufig",
    "hingegen",
    "hinlänglich",
    "höchst",
    "höchstens",
    "immer",
    "immerhin",
    "immerzu",
    "indessen",
    "infolgedessen",
    "insbesondere",
    "inzwischen",
    "irgend",
    "irgendein",
    "irgendjemand",
    "irgendwann",
    "irgendwie",
    "irgendwo",
    "ja",
    "je",
    "jedenfalls",
    "jedoch",
    "jemals",
    "kaum",
    "keinesfalls",
    "keineswegs",
    "längst",
    "lediglich",
    "leider",
    "letztlich",
    "manchmal",
    "mehrfach",
    "meinetwegen",
    "meist",
    "meistens",
    "meistenteils",
    "mindestens",
    "mithin",
    "mitunter",
    "möglicherweise",
    "möglichst",
    "nämlich",
    "naturgemäß",
    "natürlich",
    "neuerdings",
    "neuerlich",
    "neulich",
    "nichtsdestoweniger",
    "nie",
    "niemals",
    "nun",
    "nur",
    "offenbar",
    "offenkundig",
    "offensichtlich",
    "oft",
    "ohnedies",
    "partout",
    "plötzlich",
    "praktisch",
    "quasi",
    "recht",
    "reichlich",
    "reiflich",
    "relativ",
    "restlos",
    "richtiggehend",
    "rundheraus",
    "rundum",
    "sattsam",
    "schlicht",
    "schlichtweg",
    "schließlich",
    "schlussendlich",
    "schon",
    "sehr",
    "selbst",
    "selbstredend",
    "selbstverständlich",
    "selten",
    "seltsamerweise",
    "sicher",
    "sicherlich",
    "so",
    "sogar",
    "sonst",
    "sowieso",
    "sozusagen",
    "stellenweise",
    "stets",
    "trotzdem",
    "überaus",
    "überdies",
    "überhaupt",
    "übrigens",
    "umständehalber",
    "unbedingt",
    "unerhört",
    "ungefähr",
    "ungemein",
    "ungewöhnlich",
    "ungleich",
    "unglücklicherweise",
    "unlängst",
    "unmaßgeblich",
    "unsagbar",
    "unsäglich",
    "unstreitig",
    "unzweifelhaft",
    "vergleichsweise",
    "vermutlich",
    "vielfach",
    "vielleicht",
    "voll",
    "vollends",
    "völlig",
    "vollkommen",
    "vollständig",
    "wahrscheinlich",
    "weidlich",
    "weitgehend",
    "wenigstens",
    "wieder",
    "wiederum",
    "wirklich",
    "wohl",
    "wohlgemerkt",
    "womöglich",
    "ziemlich",
    "zudem",
    "zugegeben",
    "zumeist",
    "zusehends",
    "zuweilen",
    "zweifellos",
    "zweifelsfrei",
    "zweifelsohne",
];

pub struct WordStyleConfig {
    pub id: &'static str,
    pub description: &'static str,
    pub min_percent: u32,
    pub denominator: f64,
    /// `conditionFulfilled(tokens, n)` -> end token index
    pub condition: fn(&[&AnalyzedTokenReadings], usize) -> Option<usize>,
    pub limit_message: fn(u32, f64) -> String,
}

/// `AbstractStatisticStyleRule.match` (no user config: the sentence-level
/// conditions of all three German rules are constant `false`).
fn check_words(sentences: &[AnalyzedSentence], config: &WordStyleConfig) -> Vec<Match> {
    let mut hints: Vec<(usize, usize, usize)> = Vec::new();
    let mut word_count = 0u32;
    let mut is_direct_speech = false;
    for (sentence_index, sentence) in sentences.iter().enumerate() {
        let tokens = sentence.tokens_without_whitespace();
        for n in 1..tokens.len() {
            let token = tokens[n];
            if !is_direct_speech
                && OPENING_QUOTES.contains(&token.surface())
                && n < tokens.len() - 1
                && !tokens[n + 1].whitespace_before
            {
                is_direct_speech = true;
            } else if is_direct_speech
                && ENDING_QUOTES.contains(&token.surface())
                && n > 1
                && !token.whitespace_before
            {
                is_direct_speech = false;
            } else if (!is_direct_speech || config.min_percent == 0)
                && !token.is_whitespace
                && !crate::style_too_often::is_non_word(token.surface())
            {
                word_count += 1;
                if let Some(n_end) = (config.condition)(&tokens, n) {
                    if n_end >= n {
                        hints.push((sentence_index, token.start_pos, tokens[n_end].end_pos()));
                    }
                }
            }
        }
    }
    let num_matches = hints.len() as f64;
    let percent = if word_count > 0 {
        num_matches * config.denominator / word_count as f64
    } else {
        0.0
    };
    if percent <= config.min_percent as f64 {
        return Vec::new();
    }
    let message = (config.limit_message)(config.min_percent, percent);
    hints
        .into_iter()
        .map(|(sentence_index, start, end)| {
            let sentence = &sentences[sentence_index];
            Match::new(
                config.id,
                Option::<String>::None,
                message.clone(),
                Option::<String>::None,
                TextRange::new(sentence.offset + start, sentence.offset + end),
                Vec::new(),
                CATEGORY_ID,
                CATEGORY_NAME,
            )
            .with_metadata(config.description, "style", 0)
        })
        .collect()
}

fn filler_word_set() -> &'static HashSet<&'static str> {
    static SET: LazyLock<HashSet<&'static str>> =
        LazyLock::new(|| FILLER_WORDS.iter().copied().collect());
    &SET
}

fn has_any_lemma(token: &AnalyzedTokenReadings, lemmas: &[&str]) -> bool {
    token
        .readings
        .iter()
        .any(|r| r.stem.as_deref().is_some_and(|l| lemmas.contains(&l)))
}

/// `AnalyzedTokenReadings.isPosTagUnknown` + the letter check.
fn is_unknown_word(token: &AnalyzedTokenReadings) -> bool {
    token.readings.len() == 1
        && token.readings[0].pos_tag.is_none()
        && token.surface().chars().count() > 2
        && token.surface().chars().all(|c| {
            c.is_ascii_alphabetic() || matches!(c, 'Ä' | 'Ö' | 'Ü' | 'ä' | 'ö' | 'ü' | 'ß')
        })
}

/// `GermanFillerWordsRule.isException`.
fn filler_is_exception(tokens: &[&AnalyzedTokenReadings], num: usize) -> bool {
    if num == 1 || tokens[num - 1].surface() == "," {
        return true;
    }
    let text = tokens[num].surface();
    if text == "allein" {
        return tokens.iter().any(|t| has_lemma(t, "sein"));
    }
    if text == "recht" {
        return tokens.iter().any(|t| has_any_lemma(t, &["haben", "geben"]));
    }
    if num < tokens.len() - 1
        && (text == "so" || text == "besonders")
        && tokens[num + 1].has_pos_tag_starting_with("ADJ")
    {
        return true;
    }
    if tokens[num].has_pos_tag_starting_with("ADJ") && tokens[num - 1].surface() == "so" {
        return true;
    }
    if text == "nur" && tokens[num - 1].surface() == "nicht" {
        for i in num + 1..tokens.len().saturating_sub(2) {
            if tokens[i].surface() == ","
                && (tokens[i + 1].surface() == "auch"
                    || (tokens[i + 1].surface() == "sondern" && tokens[i + 2].surface() == "auch"))
            {
                return true;
            }
        }
    }
    if num > 2
        && text == "auch"
        && tokens[num - 1].surface() == "sondern"
        && tokens[num - 2].surface() == ","
    {
        for i in 1..num.saturating_sub(2) {
            if tokens[i].surface() == "nicht" && tokens[i + 1].surface() == "nur" {
                return true;
            }
        }
    }
    false
}

fn is_two_word_exception(first: &str, second: &str) -> bool {
    matches!(
        (first, second),
        ("aber", "nur")
            | ("aber", "auch")
            | ("auch", "nur")
            | ("immer", "wieder")
            | ("genau", "so")
            | ("so", "etwas")
            | ("so", "viel")
            | ("so", "oft")
            | ("schon", "fast")
    )
}

/// `FILLER_WORDS_DE` (limit 8%).
pub fn filler_words(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_words(
        sentences,
        &WordStyleConfig {
            id: "FILLER_WORDS_DE",
            description: "Statistische Stilanalyse: Füllwörter",
            min_percent: 8,
            denominator: 100.0,
            condition: |tokens, n| {
                let word = tokens[n].surface();
                if filler_word_set().contains(word)
                    && !filler_is_exception(tokens, n)
                    && (n < 2 || !is_two_word_exception(tokens[n - 1].surface(), word))
                    && (n + 1 >= tokens.len()
                        || !is_two_word_exception(word, tokens[n + 1].surface()))
                {
                    Some(n)
                } else {
                    None
                }
            },
            limit_message: |limit, percent| {
                if limit == 0 {
                    "Dieses Wort könnte ein Füllwort sein. Möglicherweise ist es besser es zu löschen.".to_string()
                } else {
                    format!(
                        "Mehr als {limit}% Füllwörter {{{}%}} gefunden. Möglicherweise ist es besser dieses potentielle Füllwort zu löschen.",
                        (percent + 0.5) as i64
                    )
                }
            },
        },
    )
}

/// `NON_SIGNIFICANT_VERB_DE` (limit 8 per mil).
pub fn non_significant_verbs(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_words(
        sentences,
        &WordStyleConfig {
            id: "NON_SIGNIFICANT_VERB_DE",
            description: "Statistische Stilanalyse: Verben mit wenig Aussagekraft",
            min_percent: 8,
            denominator: 1000.0,
            condition: |tokens, n| {
                let token = tokens[n];
                if !has_any_lemma(token, &["haben", "sein", "machen", "tun"])
                    || non_significant_is_exception(tokens, n)
                {
                    return None;
                }
                Some(n)
            },
            limit_message: |limit, percent| {
                if limit == 0 {
                    "Dieses Verb hat wenig Aussagekraft. Verwenden Sie wenn möglich ein anderes oder formulieren Sie den Satz um.".to_string()
                } else {
                    format!(
                        "Mehr als {limit}‰ wenig aussagekräftige Verben {{{}‰}} gefunden. Verwenden Sie wenn möglich ein anderes Verb oder formulieren Sie den Satz um.",
                        (percent + 0.5) as i64
                    )
                }
            },
        },
    )
}

/// `NonSignificantVerbsRule.isException`.
fn non_significant_is_exception(tokens: &[&AnalyzedTokenReadings], num: usize) -> bool {
    let text = tokens[num].surface();
    if text.starts_with("sein") || text.starts_with("Sein") {
        return true;
    }
    if has_lemma(tokens[num], "machen") {
        return tokens.iter().any(|t| {
            matches!(
                t.surface(),
                "Angst" | "Weg" | "frisch" | "bemerkbar" | "aufmerksam"
            )
        });
    }
    let is_haben = has_lemma(tokens[num], "haben");
    if is_haben
        && tokens
            .iter()
            .any(|t| matches!(t.surface(), "Glück" | "Angst" | "Mühe" | "Recht" | "recht"))
    {
        return true;
    }
    if is_haben || has_lemma(tokens[num], "sein") {
        return tokens.iter().any(|t| {
            t.has_pos_tag_starting_with("PA2")
                || t.has_pos_tag_starting_with("VER:PA2")
                || t.surface() == "Flucht"
                || is_unknown_word(t)
        });
    }
    false
}

/// `UnnecessaryPhraseRule.unnecessaryPhrases`.
const UNNECESSARY_PHRASES: [&[&str]; 22] = [
    &["dann", "und", "wann"],
    &["des", "Ungeachtet"],
    &["ganz", "und", "gar"],
    &["hie", "und", "da"],
    &["im", "Allgemeinen"],
    &["in", "der", "Tat"],
    &["in", "diesem", "Zusammenhang"],
    &["mehr", "oder", "weniger"],
    &["meines", "Erachtens"],
    &["ohne", "weiteres"],
    &["ohne", "Zweifel"],
    &["samt", "und", "sonders"],
    &["sowohl", "als", "auch"],
    &["voll", "und", "ganz"],
    &["von", "Neuem"],
    &["allem", "Anschein", "nach"],
    &["aufs", "Neue"],
    &["ein", "bisschen"],
    &["ein", "wenig"],
    &["des", "Öfteren"],
    &["bei", "weitem"],
    &["an", "sich"],
];

/// `UNNECESSARY_PHRASES_DE` (limit 8 per 10,000).
pub fn unnecessary_phrases(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_words(
        sentences,
        &WordStyleConfig {
            id: "UNNECESSARY_PHRASES_DE",
            description: "Statistische Stilanalyse: Potenzielle Phrasen",
            min_percent: 8,
            denominator: 10000.0,
            condition: |tokens, n| {
                for phrase in UNNECESSARY_PHRASES {
                    let mut j = 0usize;
                    while j < phrase.len()
                        && n + j < tokens.len()
                        && phrase[j] == token_for_phrase(tokens, n + j).as_str()
                    {
                        j += 1;
                    }
                    if j == phrase.len() {
                        if phrase.len() == 2
                            && phrase[0] == "an"
                            && phrase[1] == "sich"
                            && tokens.iter().any(|t| has_lemma(t, "drücken"))
                        {
                            return None;
                        }
                        return Some(n + phrase.len() - 1);
                    }
                }
                None
            },
            limit_message: |limit, percent| {
                if limit == 0 {
                    "Der Ausdruck gilt als Phrase. Es wird empfohlen ihn zu löschen, falls möglich."
                        .to_string()
                } else {
                    format!(
                        "Mehr als {limit}‱ potenzielle Phrasen {{{}‱}} gefunden. Es wird empfohlen den Ausdruck zu löschen, falls möglich.",
                        (percent + 0.5) as i64
                    )
                }
            },
        },
    )
}

/// `UnnecessaryPhraseRule.firstCharToLower` (lowercases the first character
/// of the second token only, to catch sentence starts).
fn token_for_phrase(tokens: &[&AnalyzedTokenReadings], n: usize) -> String {
    let token = tokens[n].surface();
    if n != 1 || token.chars().count() < 2 {
        return token.to_string();
    }
    let mut chars = token.chars();
    let first = chars.next().unwrap();
    format!("{}{}", first.to_lowercase(), chars.as_str())
}

// ---------------------------------------------------------------------------
// `StyleRepeatedVeryShortSentences` and `StyleRepeatedSentenceBeginning`
// ---------------------------------------------------------------------------

const MIN_REPEATED: usize = 3;
const MIN_WORDS: usize = 4;

/// `STYLE_REPEATED_SHORT_SENTENCES` (default off).
pub fn style_repeated_very_short_sentences(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    if sentences.len() < MIN_REPEATED {
        return Vec::new();
    }
    let mut matches = Vec::new();
    let mut repeated: Vec<(usize, usize, usize)> = Vec::new(); // (sentence, start, end)
    let mut ends_with_direct_speech = false;
    let mut n_para: i64 = -1;
    for (n, sentence) in sentences.iter().enumerate() {
        n_para += 1;
        let tokens = sentence.tokens_without_whitespace();
        let mut begins_with_direct_speech = ends_with_direct_speech;
        for i in 0..tokens.len() {
            if !begins_with_direct_speech
                && OPENING_QUOTES.contains(&tokens[i].surface())
                && i < tokens.len() - 1
                && !tokens[i + 1].whitespace_before
            {
                begins_with_direct_speech = true;
                ends_with_direct_speech = true;
            } else if begins_with_direct_speech
                && ENDING_QUOTES.contains(&tokens[i].surface())
                && i > 1
                && !tokens[i].whitespace_before
            {
                ends_with_direct_speech = false;
            }
        }
        let paragraph_end = crate::paragraph::is_paragraph_end(sentences, n);
        if !begins_with_direct_speech
            && (!paragraph_end || n_para > 0)
            && tokens.len() > 3
            && tokens.len() <= MIN_WORDS + 2
        {
            repeated.push((
                n,
                tokens[tokens.len() - 2].start_pos,
                tokens[tokens.len() - 1].end_pos(),
            ));
        } else {
            if repeated.len() >= MIN_REPEATED {
                for (sentence_index, start, end) in repeated.drain(..) {
                    matches.push(short_sentence_match(&sentences[sentence_index], start, end));
                }
            }
            repeated.clear();
        }
        if paragraph_end {
            n_para = -1;
        }
    }
    if repeated.len() >= MIN_REPEATED {
        for (sentence_index, start, end) in repeated {
            matches.push(short_sentence_match(&sentences[sentence_index], start, end));
        }
    }
    matches
}

fn short_sentence_match(sentence: &AnalyzedSentence, start: usize, end: usize) -> Match {
    Match::new(
        "STYLE_REPEATED_SHORT_SENTENCES",
        Option::<String>::None,
        "Stakkato-Sätze",
        Option::<String>::None,
        TextRange::new(sentence.offset + start, sentence.offset + end),
        Vec::new(),
        CATEGORY_ID,
        CATEGORY_NAME,
    )
    .with_metadata("Stakkato-Sätze", "style", 0)
}

/// `STYLE_REPEATED_SENTENCE_BEGINNING` (default off, `MIN_REPEATED` = 3).
pub fn style_repeated_sentence_beginning(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    if sentences.len() < MIN_REPEATED {
        return Vec::new();
    }
    let mut matches = Vec::new();
    let mut repeated: Vec<(usize, usize, usize)> = Vec::new();
    for (n, sentence) in sentences.iter().enumerate() {
        let tokens = sentence.tokens_without_whitespace();
        let Some(first) = tokens.get(1) else {
            continue;
        };
        if first.has_pos_tag_starting_with("ART:DEF:NOM")
            || first.has_pos_tag_starting_with("ART:IND:NOM")
        {
            let mut end = first.end_pos();
            for token in tokens.iter().skip(2) {
                if token.has_pos_tag_starting_with("VER") {
                    break;
                }
                if token.has_pos_tag_starting_with("SUB") {
                    end = token.end_pos();
                    break;
                }
            }
            repeated.push((n, first.start_pos, end));
        } else if first.has_pos_tag_starting_with("PRO:PER:NOM") {
            repeated.push((n, first.start_pos, first.end_pos()));
        } else {
            if repeated.len() >= MIN_REPEATED {
                for (sentence_index, start, end) in repeated.drain(..) {
                    matches.push(sentence_beginning_match(
                        &sentences[sentence_index],
                        start,
                        end,
                    ));
                }
            }
            repeated.clear();
        }
    }
    if repeated.len() >= MIN_REPEATED {
        for (sentence_index, start, end) in repeated {
            matches.push(sentence_beginning_match(
                &sentences[sentence_index],
                start,
                end,
            ));
        }
    }
    matches
}

fn sentence_beginning_match(sentence: &AnalyzedSentence, start: usize, end: usize) -> Match {
    Match::new(
        "STYLE_REPEATED_SENTENCE_BEGINNING",
        Option::<String>::None,
        "Subjekt als wiederholter Satzanfang",
        Option::<String>::None,
        TextRange::new(sentence.offset + start, sentence.offset + end),
        Vec::new(),
        CATEGORY_ID,
        CATEGORY_NAME,
    )
    .with_metadata("Subjekt als wiederholter Satzanfang", "style", 0)
}
