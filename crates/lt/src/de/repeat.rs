//! German repetition rules: `GermanWordRepeatRule`,
//! `GermanWordRepeatBeginningRule`, `GermanParagraphRepeatBeginningRule`
//! (via [`crate::paragraph`]) and `GermanRepeatedWordsRule`
//! (via [`crate::repeated_words`]).

use std::sync::LazyLock;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};

use crate::wordutil::{eq_ignore_case, is_word};

/// `GermanWordRepeatRule` (`Rule`, category REDUNDANCY, default on).
pub const WORD_REPEAT_ID: &str = "GERMAN_WORD_REPEAT_RULE";
const WORD_REPEAT_DESCRIPTION: &str = "Wortwiederholung (z. B. 'als als')";
const WORD_REPEAT_MESSAGE: &str = "Möglicher Tippfehler: ein Wort wird wiederholt";
const WORD_REPEAT_SHORT: &str = "Wortwiederholung";

/// `GermanWordRepeatBeginningRule` (`TextLevelRule`, REPETITIONS_STYLE).
pub const WORD_REPEAT_BEGINNING_ID: &str = "GERMAN_WORD_REPEAT_BEGINNING_RULE";
const BEGINNING_DESCRIPTION: &str = "Aufeinanderfolgende Sätze beginnen mit dem gleichen Wort";
const BEGINNING_ADV: &str = "Zwei aufeinanderfolgende Sätze beginnen mit dem gleichen Adverb.";
const BEGINNING_WORD: &str = "Drei aufeinanderfolgende Sätze beginnen mit dem gleichen Wort.";
const BEGINNING_THESAURUS: &str =
    "Evtl. können Sie den Satz umformulieren, zum Beispiel, indem Sie ein Synonym nutzen.";

/// `GermanWordRepeatBeginningRule.ADVERBS` ("Konjunktionaladverbien").
const ADVERBS: [&str; 11] = [
    "Auch",
    "Anschließend",
    "Außerdem",
    "Danach",
    "Ferner",
    "Nebenher",
    "Nebenbei",
    "Überdies",
    "Weiterführend",
    "Zudem",
    "Zusätzlich",
];

fn single_char_re() -> &'static regex::Regex {
    static RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)^[a-z]$").unwrap());
    &RE
}

// ---------------------------------------------------------------------------
// `GermanWordRepeatRule` antipatterns (Java `PatternToken` lists)
// ---------------------------------------------------------------------------

/// One programmatic antipattern element. `token`/`csToken` are literal
/// surfaces, `tokenRegex` a case-insensitive full match, `posRegex` a full
/// match over any POS tag, `matchInflectedForms` compares lemmas.
enum Pat {
    Token(&'static str, bool),
    Regex(regex::Regex),
    PosRegex(regex::Regex),
    Lemma(&'static str, bool),
    /// `tokenRegex(...).matchInflectedForms()`: case-insensitive regex over lemmas
    LemmaRegex(regex::Regex),
}

/// `tokenRegex(...)`: `StringMatcher.create(pattern, regexp=true,
/// caseSensitive=false)` — a case-insensitive full match.
fn pat_regex(pattern: &'static str) -> Pat {
    Pat::Regex(regex::Regex::new(&format!("(?i)^(?:{pattern})$")).unwrap())
}

/// `posRegex(...)`: also case-insensitive (`StringMatcher.create(..., false)`).
fn pat_pos_regex(pattern: &'static str) -> Pat {
    Pat::PosRegex(regex::Regex::new(&format!("(?i)^(?:{pattern})$")).unwrap())
}

fn pat_lemma(value: &'static str, case_sensitive: bool) -> Pat {
    Pat::Lemma(value, case_sensitive)
}

fn pat_lemma_regex(pattern: &'static str) -> Pat {
    Pat::LemmaRegex(regex::Regex::new(&format!("(?i)^(?:{pattern})$")).unwrap())
}

/// `GermanWordRepeatRule.ANTI_PATTERNS` in order.
fn anti_patterns() -> &'static [Vec<Pat>] {
    static PATTERNS: LazyLock<Vec<Vec<Pat>>> = LazyLock::new(|| {
        use Pat::Token;
        macro_rules! p {
            ($($pat:expr),* $(,)?) => { vec![$($pat),*] };
        }
        vec![
            p!(
                Token("please", true),
                Token("please", true),
                Token("please", true)
            ),
            p!(
                Token("Late", true),
                Token("Late", true),
                Token("Show", true)
            ),
            p!(
                Token("Wenn", true),
                Token("hinter", true),
                Token("Robben", true),
                Token("Robben", true),
                Token("robben", true),
                Token(",", true),
                Token("robben", true),
                Token("Robben", true),
                Token("Robben", true),
                Token("hinterher", true)
            ),
            p!(
                pat_regex("tägliche(n|m|s)?"),
                Token("klein", true),
                Token("klein", true)
            ),
            p!(Token("Bora", true), Token("Bora", true)),
            p!(Token("Tuk", true), Token("Tuk", true)),
            p!(Token("Miu", true), Token("Miu", true)),
            p!(Token("Moin", false), Token("Moin", false)),
            p!(Token("Na", false), Token("na", false)),
            p!(Token("la", false), Token("la", false)),
            p!(Token("Fragen", true), Token("fragen", true)),
            p!(Token("ha", false), Token("ha", false)),
            p!(Token("teils", false), Token("teils", false)),
            p!(Token("Marsch", false), Token("Marsch", false)),
            p!(
                Token("hip", false),
                Token("hip", false),
                Token("hurra", false)
            ),
            p!(Token("möp", false), Token("möp", false)),
            p!(Token("gout", false), Token("gout", false)),
            p!(Token("piep", false), Token("piep", false)),
            p!(Token("bla", false), Token("bla", false)),
            p!(Token("blah", false), Token("blah", false)),
            p!(Token("oh", false), Token("oh", false)),
            p!(Token("klopf", false), Token("klopf", false)),
            p!(Token("ne", false), Token("ne", false)),
            p!(
                Token("Fakten", false),
                Token("Fakten", false),
                Token("Fakten", false)
            ),
            p!(
                Token("Top", false),
                Token("Top", false),
                Token("Top", false)
            ),
            p!(
                Token("Toi", false),
                Token("Toi", false),
                Token("Toi", false)
            ),
            p!(
                Token("und", false),
                Token("und", false),
                Token("und", false)
            ),
            p!(
                Token("man", false),
                Token("man", false),
                Token("man", false)
            ),
            p!(
                pat_regex("wenn|falls"),
                Token("das", false),
                Token("das", false),
                Token("nächste", false),
                Token("mal", false)
            ),
            p!(
                Token("Arbeit", false),
                Token("Arbeit", false),
                Token("Arbeit", false)
            ),
            // Art Direktor*in in der ...
            p!(
                pat_regex(r"\*|:|\/"),
                Token("in", false),
                Token("in", false)
            ),
            p!(
                Token("Üben", false),
                Token("Üben", false),
                Token("Üben", false)
            ),
            p!(Token("cha", false), Token("cha", false)),
            p!(Token("zack", false), Token("zack", false)),
            p!(Token("sapiens", false), Token("sapiens", false)),
            p!(Token("peng", false), Token("peng", false)),
            p!(Token("bye", false), Token("bye", false)),
            // Man kann nicht nicht kommunizieren
            p!(
                Token("nicht", false),
                Token("nicht", false),
                Token("kommunizieren", false)
            ),
            // Dee Dee Ramone
            p!(Token("Dee", false), pat_regex("Dees?")),
            // Phi Phi Islands
            p!(Token("Phi", false), Token("Phi", false)),
            // Ich weiß, wer wer ist!
            p!(
                pat_regex(",|wei(ß|ss)|nicht"),
                Token("wer", false),
                Token("wer", false),
                pat_regex("war|ist|sein")
            ),
            // Wahrscheinlich ist das das Problem.
            p!(
                pat_regex("ist|war|wäre?|für|dass"),
                Token("das", false),
                Token("das", false),
                pat_pos_regex(".*SUB:.*NEU.*")
            ),
            p!(
                pat_regex("ist|war|wäre?|für|dass"),
                Token("das", false),
                Token("das", false),
                pat_pos_regex("ADJ:.*"),
                pat_pos_regex(".*SUB:.*NEU.*")
            ),
            p!(
                pat_regex("ist|war|wäre?|für|dass"),
                Token("das", false),
                Token("das", false),
                pat_pos_regex("ADJ:.*NEU.*"),
                pat_pos_regex("UNKNOWN")
            ),
            // Als wir das das erste Mal
            p!(
                pat_regex("als|wenn"),
                pat_pos_regex("(PRO|EIG):.*"),
                Token("das", false),
                Token("das", false),
                pat_pos_regex("ADJ:.*NEU.*"),
                pat_pos_regex(".*SUB:.*NEU.*")
            ),
            // Werden sie sie töten?
            p!(
                pat_regex("werden|würden|sollt?en|müsst?en|könnt?en"),
                Token("sie", false),
                Token("sie", false),
                pat_pos_regex("VER:1:PLU:.*")
            ),
            // Falls das das Problem ist, …
            p!(
                pat_regex("wenn|falls|ob"),
                Token("das", false),
                Token("das", false),
                pat_pos_regex("SUB:NOM:SIN:NEU.*"),
                pat_lemma_regex("sein|haben")
            ),
            // Falls das das neue Problem ist, …
            p!(
                pat_regex("wenn|falls|ob"),
                Token("das", false),
                Token("das", false),
                pat_pos_regex("(ADJ|PA[12]).*NEU.*"),
                pat_pos_regex("SUB:NOM:SIN:NEU.*"),
                pat_lemma_regex("sein|haben")
            ),
            // "wie Honda und Samsung, die die Bezahlung ihrer Firmenchefs..."
            p!(
                Token(",", true),
                pat_lemma("der", true),
                pat_lemma("der", true)
            ),
            // "Alle die die"
            p!(
                pat_regex("alle|nur|obwohl|lediglich|für|zwar|aber"),
                Token("die", true),
                Token("die", true)
            ),
            // "Haben die die Elemente ..."
            p!(
                pat_pos_regex("PKT|SENT_START|KON:NEB"),
                pat_regex("haben|hatten"),
                Token("die", true),
                Token("die", true),
                pat_pos_regex(".*SUB.*PLU.*|UNKNOWN")
            ),
            // "und ob die die Währungen ..."
            p!(
                pat_pos_regex("PKT|SENT_START|KON:NEB"),
                pat_regex("ob|falls"),
                Token("die", true),
                Token("die", true),
                pat_pos_regex(".*SUB.*PLU.*|UNKNOWN")
            ),
            // "Das Haus, in das das Kind läuft."
            p!(
                Token(",", true),
                pat_pos_regex("PRP:.+"),
                pat_lemma("der", true),
                pat_lemma("der", true)
            ),
            // "Er will sein Leben leben"
            p!(Token("Leben", true), Token("leben", true)),
            // "Die markierten Stellen stellen die Aufnahmepunkte dar."
            p!(Token("Stellen", true), Token("stellen", true)),
            // "Wir reisen in die ferne Ferne."
            p!(
                Token("die", false),
                Token("ferne", true),
                Token("Ferne", true)
            ),
            // "Er muss sein Essen essen"
            p!(Token("Essen", true), Token("essen", true)),
            p!(pat_regex("^[_]+$"), pat_regex("^[_]+$")),
            // "Er gab ihr ihr Buch zurück."
            p!(
                pat_pos_regex("VER:.*[123]:.+|PKT|ADV:INR"),
                Token("ihr", true),
                Token("ihr", true),
                pat_pos_regex("SUB.+")
            ),
        ]
    });
    &PATTERNS
}

fn pat_matches(pat: &Pat, token: &AnalyzedTokenReadings) -> bool {
    match pat {
        Pat::Token(value, case_sensitive) => {
            if *case_sensitive {
                token.surface() == *value
            } else {
                eq_ignore_case(token.surface(), value)
            }
        }
        Pat::Regex(re) => re.is_match(token.surface()),
        Pat::PosRegex(re) => token
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref().is_some_and(|t| re.is_match(t))),
        Pat::Lemma(value, case_sensitive) => token.readings.iter().any(|r| {
            let text = r.stem.as_deref().unwrap_or(r.token.as_str());
            if *case_sensitive {
                text == *value
            } else {
                eq_ignore_case(text, value)
            }
        }),
        Pat::LemmaRegex(re) => token.readings.iter().any(|r| {
            let text = r.stem.as_deref().unwrap_or(r.token.as_str());
            re.is_match(text)
        }),
    }
}

/// `Rule.getSentenceWithImmunization`: IMMUNIZE antimatches over a copy of
/// the non-blank token view; returns one flag per view token.
fn immunization_flags(view: &[&AnalyzedTokenReadings]) -> Vec<bool> {
    let mut immune = vec![false; view.len()];
    for pattern in anti_patterns() {
        if pattern.len() > view.len() {
            continue;
        }
        for start in 0..=view.len() - pattern.len() {
            if pattern
                .iter()
                .zip(&view[start..])
                .all(|(p, t)| pat_matches(p, t))
            {
                for flag in immune.iter_mut().skip(start).take(pattern.len()) {
                    *flag = true;
                }
            }
        }
    }
    immune
}

fn has_pos_tag_starting_with(token: &AnalyzedTokenReadings, prefix: &str) -> bool {
    token
        .readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with(prefix)))
}

fn has_pos_tag(token: &AnalyzedTokenReadings, tag: &str) -> bool {
    token
        .readings
        .iter()
        .any(|r| r.pos_tag.as_deref() == Some(tag))
}

/// `GermanWordRepeatRule.ignore`.
fn german_ignore(tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    let prev = tokens[position - 1].surface();
    let current = tokens[position].surface();
    if (position != 2 && prev == "Sie" && current == "sie") || (prev == "sie" && current == "Sie") {
        return true;
    }
    if (position != 2 && prev == "Waren" && current == "waren")
        || (prev == "waren" && current == "Waren")
    {
        return true;
    }
    if position > 2 && prev == "sie" && current == "sie" {
        if has_pos_tag(tokens[position - 2], "KON:UNT") {
            // "Sie tut das, damit sie sie nicht fortschickt"
            return true;
        }
        if tokens.len() - 1 > position
            && ((has_pos_tag_starting_with(tokens[position - 2], "VER:3:")
                && has_pos_tag(tokens[position + 1], "ZUS"))
                || (has_pos_tag_starting_with(tokens[position - 2], "VER:MOD:3")
                    && has_pos_tag(tokens[position + 1], "VER:INF:NON")))
        {
            // "Dann warfen sie sie weg." / "Dann konnte sie sie sehen."
            return true;
        }
    }
    if single_char_re().is_match(current)
        && position > 1
        && single_char_re().is_match(tokens[position - 2].surface())
        && position + 1 < tokens.len()
        && single_char_re().is_match(tokens[position + 1].surface())
    {
        // spelling with spaces in between: "A B B A"
        return true;
    }
    // `WordRepeatRule.ignore`: names that may repeat
    for name in [
        "Phi", "Li", "Xiao", "Duran", "Wagga", "Abdullah", "Nwe", "Pago", "Cao",
    ] {
        if position > 0 && tokens[position - 1].surface() == name && current == name {
            return true;
        }
    }
    false
}

/// `GermanWordRepeatRule.match` over one sentence.
pub fn word_repeat_sentence(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
) -> Vec<Match> {
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let immune = immunization_flags(&view);
    let mut rule_matches = Vec::new();
    let mut prev_token = String::new();
    for i in 1..view.len() {
        let token = view[i].surface().to_string();
        if view[i].is_immunized || immune[i] {
            prev_token.clear();
            continue;
        }
        if is_word(&token) && eq_ignore_case(&prev_token, &token) && !german_ignore(&view, i) {
            let prev_pos = view[i - 1].start_pos;
            let pos = view[i].start_pos;
            rule_matches.push(
                Match::new(
                    WORD_REPEAT_ID,
                    Option::<String>::None,
                    WORD_REPEAT_MESSAGE,
                    Some(WORD_REPEAT_SHORT.to_string()),
                    TextRange::new(
                        sentence_offset + prev_pos,
                        sentence_offset + pos + prev_token.len(),
                    ),
                    vec![Suggestion {
                        value: prev_token.clone(),
                        short_description: None,
                    }],
                    "REDUNDANCY",
                    "Redundanz",
                )
                .with_metadata(WORD_REPEAT_DESCRIPTION, "duplication", 1),
            );
        }
        prev_token = token;
    }
    rule_matches
}

// ---------------------------------------------------------------------------
// `GermanWordRepeatBeginningRule`
// ---------------------------------------------------------------------------

fn is_exception(token: &str) -> bool {
    matches!(token, ":" | "–" | "-" | "✔️" | "➡️" | "—" | "⭐️" | "⚠️")
}

fn trimmed_ends_like_sentence(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.len() > 1
        && trimmed
            .chars()
            .last()
            .is_some_and(|c| matches!(c, '.' | '?' | '!'))
}

/// `WordRepeatBeginningRule.match` with the German adverb set and messages
/// (no suggestions: the German subclass does not override `getSuggestions`).
pub fn word_repeat_beginning(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    let mut last_token = String::new();
    let mut before_last_token = String::new();
    let mut prev_sentence: Option<&AnalyzedSentence> = None;
    for sentence in sentences {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut token = String::new();
        if tokens.len() > 1 {
            token = tokens[1].surface().to_string();
            if tokens.len() > 3 {
                let is_word = token.chars().count() != 1
                    || token.chars().next().is_some_and(char::is_alphabetic);
                if is_word
                    && last_token == token
                    && !is_exception(&token)
                    && !is_exception(tokens[2].surface())
                    && !is_exception(tokens[3].surface())
                    && prev_sentence.is_some_and(|p| trimmed_ends_like_sentence(&p.text))
                {
                    let short_msg = if ADVERBS.contains(&token.as_str()) {
                        Some(BEGINNING_ADV)
                    } else if before_last_token == token {
                        Some(BEGINNING_WORD)
                    } else {
                        None
                    };
                    if let Some(short_msg) = short_msg {
                        let msg = format!("{short_msg} {BEGINNING_THESAURUS}");
                        let start_pos = tokens[1].start_pos;
                        let end_pos = start_pos + token.len();
                        rule_matches.push(
                            Match::new(
                                WORD_REPEAT_BEGINNING_ID,
                                Option::<String>::None,
                                msg,
                                Some(short_msg.to_string()),
                                TextRange::new(
                                    sentence.offset + start_pos,
                                    sentence.offset + end_pos,
                                ),
                                Vec::new(),
                                "REPETITIONS_STYLE",
                                "Wiederholungen (Stil)",
                            )
                            .with_metadata(
                                BEGINNING_DESCRIPTION,
                                "style",
                                0,
                            ),
                        );
                    }
                }
            }
        }
        before_last_token = last_token;
        last_token = token;
        prev_sentence = Some(sentence);
    }
    rule_matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::AnalyzedToken;

    fn token(surface: &str, stem: Option<&str>, pos: Option<&str>) -> AnalyzedTokenReadings {
        AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            surface,
            stem.map(str::to_string),
            pos.map(str::to_string),
        )])
    }

    #[test]
    fn antipattern_immunizes_falls_das_das_problem_ist() {
        let tokens = vec![
            token("", None, Some("SENT_START")),
            token("Falls", Some("falls"), Some("KON:UNT")),
            token("das", Some("der"), Some("ART:DEF:NOM:SIN:NEU")),
            token("das", Some("der"), Some("ART:DEF:NOM:SIN:NEU")),
            token("Problem", Some("Problem"), Some("SUB:NOM:SIN:NEU")),
            token("ist", Some("sein"), Some("VER:3:SIN:PRÄ:NON")),
            token(",", Some(","), Some("PKT")),
            token("dann", Some("dann"), Some("ADV")),
            token("gut", Some("gut"), Some("ADJ:NOM:SIN:NEU")),
            token(".", Some("."), Some("PKT")),
        ];
        let view: Vec<&AnalyzedTokenReadings> = tokens.iter().collect();
        let flags = immunization_flags(&view);
        assert!(flags[2] && flags[3], "das das must be immunized: {flags:?}");
    }

    #[test]
    fn antipattern_immunizes_wahrscheinlich_ist_das_das_problem() {
        let tokens = [
            token("", None, Some("SENT_START")),
            token(
                "Wahrscheinlich",
                Some("wahrscheinlich"),
                Some("ADJ:PRD:GRU"),
            ),
            token("ist", Some("sein"), Some("VER:3:SIN:PRÄ:NON")),
            token("das", Some("der"), Some("ART:DEF:NOM:SIN:NEU")),
            token("das", Some("der"), Some("ART:DEF:NOM:SIN:NEU")),
            token("Problem", Some("Problem"), Some("SUB:NOM:SIN:NEU")),
            token(".", Some("."), Some("PKT")),
        ];
        let view: Vec<&AnalyzedTokenReadings> = tokens.iter().collect();
        let flags = immunization_flags(&view);
        assert!(flags[3] && flags[4], "das das must be immunized: {flags:?}");
    }
}
