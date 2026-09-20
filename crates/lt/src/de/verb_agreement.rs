//! `VerbAgreementRule` (`DE_VERBAGREEMENT`, checklist item 23, text level,
//! default on): subject/verb agreement for 1st/2nd person and personal
//! pronouns ("ich bist" / "Max bin da").
//!
//! The 96 antipatterns are applied per *partial* sentence: Java splits each
//! sentence at `", " + conjunction` boundaries (`weil`, `obwohl`, `dass`,
//! `indem`, `sodass`) and runs the matcher on each slice. Suggestions come
//! from the German synthesizer (`language.getSynthesizer().synthesize(...)`)
//! and are sorted by Levenshtein similarity to the marked text; the verb
//! suggestion set reproduces Java's `HashSet` iteration order
//! (`java_hash_set_order`).

use std::sync::{Arc, LazyLock};

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};
use lt_pattern::matcher as pm;
use lt_pattern::PatternToken;

use super::util;

pub const RULE_ID: &str = "DE_VERBAGREEMENT";
const DESCRIPTION: &str =
    "Kongruenz von Subjekt und Prädikat (nur 1. u. 2. Person oder m. Personalpronomen), z.B. 'Er bist (ist)'";
const CATEGORY_ID: &str = "GRAMMAR";
const CATEGORY_NAME: &str = "Grammatik";

static ANTI_PATTERNS: LazyLock<Vec<Arc<pm::CompiledPattern>>> = LazyLock::new(compile_antipatterns);

/// `VerbAgreementRule.BIN_IGNORE`.
static BIN_IGNORE: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "Suleiman", "Mohamed", "Muhammad", "Muhammed", "Mohammed", "Mohammad", "Mansour", "Qaboos",
        "Qabus", "Tamim", "Majid", "Salman", "Ghazi", "Mahathir", "Madschid", "Maktum", "al-Aziz",
        "Asis", "Numan", "Hussein", "Abdul", "Abdulla", "Abdullah", "Isa", "Osama", "Said",
        "Zayid", "Zayed", "Hamad", "Chalifa", "Raschid", "Turki", "/",
    ]
});

/// `VerbAgreementRule.CONJUNCTIONS`.
static CONJUNCTIONS: [&str; 5] = ["weil", "obwohl", "dass", "indem", "sodass"];
/// `VerbAgreementRule.QUOTATION_MARKS`.
static QUOTATION_MARKS: [&str; 2] = ["\"", "„"];

pub struct VerbAgreementRule {
    synth: Arc<lt_tagger::GermanSynthesizer>,
}

impl VerbAgreementRule {
    pub fn new(synth: Arc<lt_tagger::GermanSynthesizer>) -> Self {
        Self { synth }
    }

    /// `VerbAgreementRule.match(List<AnalyzedSentence>)`.
    pub fn check(&self, sentences: &[AnalyzedSentence]) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        for sentence in sentences {
            let tokens = &sentence.tokens;
            let mut idx = 0usize;
            for i in 2..tokens.len() {
                if tokens[i - 2].surface() == "," && CONJUNCTIONS.contains(&tokens[i].surface()) {
                    rule_matches.extend(self.match_partial(&tokens[idx..i], sentence));
                    idx = i;
                }
            }
            rule_matches.extend(self.match_partial(&tokens[idx..], sentence));
        }
        rule_matches
    }

    /// The inner `match(partialSentence, pos, wholeSentence)`.
    fn match_partial(
        &self,
        partial: &[AnalyzedTokenReadings],
        whole: &AnalyzedSentence,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let immunized = immunize_tokens(partial, &ANTI_PATTERNS);
        let tokens = non_blank(&immunized);
        if tokens.len() < 4 {
            return rule_matches;
        }
        let mut pos_ich: isize = -1;
        let mut pos_du: isize = -1;
        let mut pos_er: isize = -1;
        let mut pos_wir: isize = -1;
        let mut pos_ver1_sin: isize = -1;
        let mut pos_ver2_sin: isize = -1;
        let mut pos_ver1_plu: isize = -1;
        let mut pos_possible_ver1_sin: isize = -1;
        let mut pos_possible_ver2_sin: isize = -1;
        let mut pos_possible_ver3_sin: isize = -1;
        let mut pos_possible_ver1_plu: isize = -1;

        for i in 1..tokens.len() {
            let str_token = tokens[i].surface().to_lowercase().replace('‚', "");
            match str_token.as_str() {
                "ich" => pos_ich = i as isize,
                "du" => pos_du = i as isize,
                "er" => pos_er = i as isize,
                "wir" => pos_wir = i as isize,
                _ => {}
            }
            let first_char_lower = tokens[i]
                .surface()
                .chars()
                .next()
                .is_some_and(char::is_lowercase);
            if tokens[i]
                .readings
                .iter()
                .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains("VER")))
                && (first_char_lower || i == 1 || is_quotation_mark(tokens[i - 1]))
            {
                if has_unambiguously_person_and_number(tokens[i], "1", "SIN")
                    && !(str_token == "bin"
                        && (BIN_IGNORE.contains(&tokens[i - 1].surface())
                            || (tokens.len() != i + 1
                                && tokens[i + 1].surface().starts_with("Laden"))))
                {
                    pos_ver1_sin = i as isize;
                } else if has_unambiguously_person_and_number(tokens[i], "2", "SIN")
                    && tokens[i].surface() != "Probst"
                {
                    pos_ver2_sin = i as isize;
                } else if has_unambiguously_person_and_number(tokens[i], "1", "PLU") {
                    pos_ver1_plu = i as isize;
                }
                if util::has_partial_pos_tag(tokens[i], ":1:SIN") {
                    pos_possible_ver1_sin = i as isize;
                }
                if util::has_partial_pos_tag(tokens[i], ":2:SIN") {
                    pos_possible_ver2_sin = i as isize;
                }
                if util::has_partial_pos_tag(tokens[i], ":3:SIN") {
                    pos_possible_ver3_sin = i as isize;
                }
                if util::has_partial_pos_tag(tokens[i], ":1:PLU") {
                    pos_possible_ver1_plu = i as isize;
                }
            }
        }

        if pos_ver1_sin != -1
            && pos_ich == -1
            && !is_quotation_mark(tokens[pos_ver1_sin as usize - 1])
        {
            if !tokens[pos_ver1_sin as usize].is_immunized {
                rule_matches.push(self.rule_match_wrong_verb(tokens[pos_ver1_sin as usize], whole));
            }
        } else if let Some(pos_ich) = non_negative(pos_ich) {
            // Java compares the token start position in UTF-16 code units;
            // the engine stores UTF-8 bytes, so convert for this check.
            let ich_start_utf16 = java_pos(&whole.text, tokens[pos_ich].start_pos);
            if !is_near(pos_possible_ver1_sin, pos_ich)
                && (tokens[pos_ich].surface() == "ich"
                    || ich_start_utf16 <= 1
                    || (tokens[pos_ich].surface() == "Ich"
                        && pos_ich >= 2
                        && tokens[pos_ich - 2].surface() == ":")
                    || (tokens[pos_ich].surface() == "Ich"
                        && pos_ich >= 1
                        && tokens[pos_ich - 1].surface() == ":"))
                && (!is_quotation_mark(tokens[pos_ich - 1])
                    || pos_ich < 3
                    || (pos_ich > 1 && tokens[pos_ich - 2].surface() == ":"))
            {
                let plus1 = usize::from(pos_ich + 1 != tokens.len());
                let check = verb_does_match_person_and_number(
                    tokens[pos_ich - 1],
                    tokens[pos_ich + plus1],
                    "1",
                    "SIN",
                );
                if !check.0
                    && !next_but_one_is_modal(&tokens, pos_ich)
                    && check.1.map(|v| v.surface()) != Some("äußerst")
                    && !tokens[pos_ich].is_immunized
                {
                    rule_matches.push(self.rule_match_wrong_verb_subject(
                        tokens[pos_ich],
                        check.1.expect("finite verb when no match"),
                        "1:SIN",
                        whole,
                    ));
                }
            }
        }

        if pos_ver2_sin != -1
            && pos_du == -1
            && !is_quotation_mark(tokens[pos_ver2_sin as usize - 1])
        {
            if !tokens[pos_ver2_sin as usize].is_immunized {
                rule_matches.push(self.rule_match_wrong_verb(tokens[pos_ver2_sin as usize], whole));
            }
        } else if let Some(pos_du) = non_negative(pos_du) {
            if !is_near(pos_possible_ver2_sin, pos_du)
                && (!is_quotation_mark(tokens[pos_du - 1])
                    || pos_du < 3
                    || (pos_du > 1 && tokens[pos_du - 2].surface() == ":"))
            {
                let plus1 = usize::from(pos_du + 1 != tokens.len());
                let check = verb_does_match_person_and_number(
                    tokens[pos_du - 1],
                    tokens[pos_du + plus1],
                    "2",
                    "SIN",
                );
                if !check.0
                    && !tokens[pos_du + plus1].has_pos_tag_starting_with("VER:1:SIN:KJ2")
                    && (!tokens[pos_du + plus1].has_pos_tag_starting_with("ADJ:")
                        || tokens[pos_du + plus1].has_pos_tag("ADJ:PRD:GRU"))
                    && !tokens[pos_du - 1].has_pos_tag_starting_with("VER:1:SIN:KJ2")
                    && !next_but_one_is_modal(&tokens, pos_du)
                    && !tokens[pos_du].is_immunized
                {
                    rule_matches.push(self.rule_match_wrong_verb_subject(
                        tokens[pos_du],
                        check.1.expect("finite verb when no match"),
                        "2:SIN",
                        whole,
                    ));
                }
            }
        }

        if let Some(pos_er) = non_negative(pos_er) {
            if !is_near(pos_possible_ver3_sin, pos_er)
                && (!is_quotation_mark(tokens[pos_er - 1])
                    || pos_er < 3
                    || (pos_er > 1 && tokens[pos_er - 2].surface() == ":"))
            {
                let plus1 = usize::from(pos_er + 1 != tokens.len());
                let check = verb_does_match_person_and_number(
                    tokens[pos_er - 1],
                    tokens[pos_er + plus1],
                    "3",
                    "SIN",
                );
                if !check.0
                    && !next_but_one_is_modal(&tokens, pos_er)
                    && check.1.map(|v| v.surface()) != Some("äußerst")
                    && check.1.map(|v| v.surface()) != Some("regen")
                    && !tokens[pos_er].is_immunized
                {
                    rule_matches.push(self.rule_match_wrong_verb_subject(
                        tokens[pos_er],
                        check.1.expect("finite verb when no match"),
                        "3:SIN",
                        whole,
                    ));
                }
            }
        }

        if pos_ver1_plu != -1
            && pos_wir == -1
            && !is_quotation_mark(tokens[pos_ver1_plu as usize - 1])
        {
            if !tokens[pos_ver1_plu as usize].is_immunized {
                rule_matches.push(self.rule_match_wrong_verb(tokens[pos_ver1_plu as usize], whole));
            }
        } else if let Some(pos_wir) = non_negative(pos_wir) {
            if !is_near(pos_possible_ver1_plu, pos_wir) && !is_quotation_mark(tokens[pos_wir - 1]) {
                let plus1 = usize::from(pos_wir + 1 != tokens.len());
                let check = verb_does_match_person_and_number(
                    tokens[pos_wir - 1],
                    tokens[pos_wir + plus1],
                    "1",
                    "PLU",
                );
                if !check.0
                    && !next_but_one_is_modal(&tokens, pos_wir)
                    && !tokens[pos_wir].is_immunized
                    && check.1.map(|v| v.surface()) != Some("äußerst")
                {
                    rule_matches.push(self.rule_match_wrong_verb_subject(
                        tokens[pos_wir],
                        check.1.expect("finite verb when no match"),
                        "1:PLU",
                        whole,
                    ));
                }
            }
        }

        rule_matches
    }
}

/// `nextButOneIsModal`.
fn next_but_one_is_modal(tokens: &[&AnalyzedTokenReadings], pos: usize) -> bool {
    pos < tokens.len().saturating_sub(2) && util::has_partial_pos_tag(tokens[pos + 2], ":MOD:")
}

/// `true if |a - b| < 5, and a != -1`.
fn is_near(a: isize, b: usize) -> bool {
    a != -1 && (a - b as isize).unsigned_abs() < 5
}

/// Java `AnalyzedTokenReadings.getStartPos()` (UTF-16 code units) for a
/// token whose engine byte offset is `byte_pos`.
fn java_pos(text: &str, byte_pos: usize) -> usize {
    text.get(..byte_pos)
        .map(|prefix| prefix.encode_utf16().count())
        .unwrap_or(usize::MAX)
}

fn non_negative(value: isize) -> Option<usize> {
    if value > 0 {
        Some(value as usize)
    } else {
        None
    }
}

/// `isQuotationMark`.
fn is_quotation_mark(token: &AnalyzedTokenReadings) -> bool {
    QUOTATION_MARKS.contains(&token.surface())
}

/// `hasUnambiguouslyPersonAndNumber`.
fn has_unambiguously_person_and_number(
    token: &AnalyzedTokenReadings,
    person: &str,
    number: &str,
) -> bool {
    let surface = token.surface();
    if surface.is_empty()
        || (surface.chars().next().is_some_and(char::is_uppercase) && token.start_pos != 0)
        || !token.has_pos_tag_starting_with("VER")
    {
        return false;
    }
    for reading in &token.readings {
        let Some(postag) = reading.pos_tag.as_deref() else {
            continue;
        };
        if postag.ends_with("_END") {
            continue;
        }
        if !postag.contains(&format!(":{person}:{number}")) {
            return false;
        }
    }
    true
}

/// `isFiniteVerb`.
fn is_finite_verb(token: &AnalyzedTokenReadings) -> bool {
    let surface = token.surface();
    if surface.is_empty()
        || (surface.chars().next().is_some_and(char::is_uppercase) && token.start_pos != 0)
        || !token.has_pos_tag_starting_with("VER")
        || token.readings.iter().any(|r| {
            r.pos_tag
                .as_deref()
                .is_some_and(|t| t.contains("PA2") || t.contains("PRO:") || t.contains("ZAL"))
        })
        || surface == "einst"
    {
        return false;
    }
    token.readings.iter().any(|r| {
        r.pos_tag
            .as_deref()
            .is_some_and(|t| t.contains(":1:") || t.contains(":2:") || t.contains(":3:"))
    })
}

/// `verbDoesMatchPersonAndNumber`: (matches, finite verb found). The finite
/// verb is `Some` whenever the first return value is `false`.
fn verb_does_match_person_and_number<'a>(
    token1: &'a AnalyzedTokenReadings,
    token2: &'a AnalyzedTokenReadings,
    person: &str,
    number: &str,
) -> (bool, Option<&'a AnalyzedTokenReadings>) {
    if matches!(token1.surface(), "," | "und" | "sowie" | "&")
        || matches!(token2.surface(), "," | "und" | "sowie" | "&")
    {
        return (true, None);
    }
    let mut found_finite_verb = false;
    let mut finite_verb = None;
    if is_finite_verb(token1) {
        found_finite_verb = true;
        finite_verb = Some(token1);
        if util::has_partial_pos_tag(token1, &format!(":{person}:{number}")) {
            return (true, finite_verb);
        }
    }
    if is_finite_verb(token2) {
        found_finite_verb = true;
        finite_verb = Some(token2);
        if util::has_partial_pos_tag(token2, &format!(":{person}:{number}")) {
            return (true, finite_verb);
        }
    }
    (!found_finite_verb, finite_verb)
}

impl VerbAgreementRule {
    /// `getVerbSuggestions`.
    fn get_verb_suggestions(
        &self,
        verb: &AnalyzedTokenReadings,
        expected_verb_pos: &str,
        to_uppercase: bool,
    ) -> Vec<String> {
        let Some(verb_token) = verb
            .readings
            .iter()
            .find(|t| t.pos_tag.as_deref().is_some_and(|p| p.starts_with("VER:")))
        else {
            return Vec::new();
        };
        let synthesized =
            self.synth
                .synthesize(verb_token, &format!("VER.*:{expected_verb_pos}.*"), true);
        let mut suggestions = util::java_hash_set_order(&synthesized);
        if to_uppercase {
            for suggestion in &mut suggestions {
                *suggestion = lt_tagger::uppercase_first_char(suggestion);
            }
        }
        suggestions
    }

    /// `getPronounSuggestions`.
    fn get_pronoun_suggestions(verb: &AnalyzedTokenReadings, to_uppercase: bool) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        if util::has_partial_pos_tag(verb, ":1:SIN") {
            result.push("ich".to_string());
        }
        if util::has_partial_pos_tag(verb, ":2:SIN") {
            result.push("du".to_string());
        }
        if util::has_partial_pos_tag(verb, ":3:SIN") {
            result.push("er".to_string());
            result.push("sie".to_string());
            result.push("es".to_string());
        }
        if util::has_partial_pos_tag(verb, ":1:PLU") {
            result.push("wir".to_string());
        }
        if util::has_partial_pos_tag(verb, ":2:PLU") {
            result.push("ihr".to_string());
        }
        if util::has_partial_pos_tag(verb, ":3:PLU") && !result.contains(&"sie".to_string()) {
            result.push("sie".to_string());
        }
        if to_uppercase {
            for suggestion in &mut result {
                *suggestion = lt_tagger::uppercase_first_char(suggestion);
            }
        }
        result
    }

    /// `ruleMatchWrongVerb`.
    fn rule_match_wrong_verb(
        &self,
        token: &AnalyzedTokenReadings,
        whole: &AnalyzedSentence,
    ) -> Match {
        let msg = format!(
            "Möglicherweise fehlende grammatische Übereinstimmung zwischen Subjekt und Prädikat ({}) bezüglich Person oder Numerus (Einzahl, Mehrzahl - Beispiel: 'Max bist' statt 'Max ist').",
            token.surface()
        );
        Match::new(
            RULE_ID,
            Option::<String>::None,
            msg,
            Option::<String>::None,
            TextRange::new(
                whole.offset + token.start_pos,
                whole.offset + token.end_pos(),
            ),
            Vec::new(),
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(DESCRIPTION, "uncategorized", 0)
    }

    /// `ruleMatchWrongVerbSubject`.
    fn rule_match_wrong_verb_subject(
        &self,
        subject: &AnalyzedTokenReadings,
        verb: &AnalyzedTokenReadings,
        expected_verb_pos: &str,
        whole: &AnalyzedSentence,
    ) -> Match {
        let msg = format!(
            "Möglicherweise fehlende grammatische Übereinstimmung zwischen Subjekt ({}) und Prädikat ({}) bezüglich Person oder Numerus (Einzahl, Mehrzahl - Beispiel: 'ich sind' statt 'ich bin').",
            subject.surface(),
            verb.surface()
        );
        let mut suggestions: Vec<String> = Vec::new();
        let (range, marked_text) = if subject.start_pos < verb.start_pos {
            let marked = whole
                .text
                .get(subject.start_pos..verb.end_pos())
                .unwrap_or("")
                .to_string();
            let verb_suggestions = self.get_verb_suggestions(verb, expected_verb_pos, false);
            for verb_suggestion in verb_suggestions {
                suggestions.push(format!("{} {}", subject.surface(), verb_suggestion));
            }
            let pronoun_suggestions = Self::get_pronoun_suggestions(
                verb,
                subject
                    .surface()
                    .chars()
                    .next()
                    .is_some_and(char::is_uppercase),
            );
            for pronoun_suggestion in pronoun_suggestions {
                suggestions.push(format!("{} {}", pronoun_suggestion, verb.surface()));
            }
            (
                TextRange::new(
                    whole.offset + subject.start_pos,
                    whole.offset + verb.end_pos(),
                ),
                marked,
            )
        } else {
            let marked = whole
                .text
                .get(verb.start_pos..subject.end_pos())
                .unwrap_or("")
                .to_string();
            let verb_suggestions = self.get_verb_suggestions(
                verb,
                expected_verb_pos,
                verb.surface()
                    .chars()
                    .next()
                    .is_some_and(char::is_uppercase),
            );
            for verb_suggestion in verb_suggestions {
                suggestions.push(format!("{} {}", verb_suggestion, subject.surface()));
            }
            let pronoun_suggestions = Self::get_pronoun_suggestions(verb, false);
            for pronoun_suggestion in pronoun_suggestions {
                suggestions.push(format!("{} {}", verb.surface(), pronoun_suggestion));
            }
            (
                TextRange::new(
                    whole.offset + verb.start_pos,
                    whole.offset + subject.end_pos(),
                ),
                marked,
            )
        };
        util::sort_by_similarity(&mut suggestions, &marked_text);
        Match::new(
            RULE_ID,
            Option::<String>::None,
            msg,
            Option::<String>::None,
            range,
            suggestions
                .into_iter()
                .map(|value| Suggestion {
                    value,
                    short_description: None,
                })
                .collect(),
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(DESCRIPTION, "uncategorized", 0)
    }
}

/// Non-blank view of a token slice (Java `AnalyzedSentence` constructor +
/// `getTokensWithoutWhitespace`).
fn non_blank(tokens: &[AnalyzedTokenReadings]) -> Vec<&AnalyzedTokenReadings> {
    tokens
        .iter()
        .filter(|t| {
            !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
        })
        .collect()
}

/// `getSentenceWithImmunization` on a token slice: clone, apply the rule's
/// antipatterns over the non-blank view, return the marked clone.
fn immunize_tokens(
    tokens: &[AnalyzedTokenReadings],
    antipatterns: &[Arc<pm::CompiledPattern>],
) -> Vec<AnalyzedTokenReadings> {
    let mut immunized = tokens.to_vec();
    let view = non_blank(tokens);
    let mut immune = vec![false; view.len()];
    for ap in antipatterns {
        if view.is_empty() {
            break;
        }
        for m in pm::find_matches(ap, &[], &view) {
            let last = m.end_tok().min(immune.len() - 1);
            for flag in immune.iter_mut().take(last + 1).skip(m.start_tok()) {
                *flag = true;
            }
        }
    }
    let mut view_index = 0usize;
    for token in &mut immunized {
        if !token.is_whitespace
            || token.is_sentence_start
            || token.is_sentence_end
            || token.is_paragraph_end
        {
            if immune.get(view_index).copied().unwrap_or(false) {
                token.is_immunized = true;
            }
            view_index += 1;
        }
    }
    immunized
}

fn token(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        in_marker: true,
        ..Default::default()
    }
}

fn cs_token(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        case_sensitive: true,
        in_marker: true,
        ..Default::default()
    }
}

fn token_regex(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        regexp: true,
        in_marker: true,
        ..Default::default()
    }
}

fn pos(postag: &str) -> PatternToken {
    PatternToken {
        postag: Some(postag.to_string()),
        in_marker: true,
        ..Default::default()
    }
}

fn pos_regex(postag: &str) -> PatternToken {
    PatternToken {
        postag: Some(postag.to_string()),
        postag_regexp: true,
        in_marker: true,
        ..Default::default()
    }
}

fn inflected(mut t: PatternToken) -> PatternToken {
    t.inflected = true;
    t
}

fn skip(mut t: PatternToken, value: i32) -> PatternToken {
    t.skip = Some(value);
    t
}

fn negate(mut t: PatternToken) -> PatternToken {
    t.negate = true;
    t
}

/// `VerbAgreementRule.ANTI_PATTERNS` (96 patterns, order matters).
fn antipattern_defs() -> Vec<Vec<PatternToken>> {
    vec![
        vec![token("du"), token("wärst"), token("ich")],
        vec![token("*"), pos_regex("VER:1:SIN:PRÄ:.*"), token("*")],
        vec![skip(token("weder"), 8), token("noch"), token("ich")],
        vec![
            pos("SENT_START"),
            pos_regex("VER:2:SIN:PRÄ:.*"),
            pos_regex("PRO:.*"),
        ],
        vec![
            pos("SENT_START"),
            pos_regex("VER:2:SIN:PRÄ:.*"),
            token_regex("einfach|denn|schon"),
        ],
        vec![
            pos("SENT_START"),
            pos_regex("VER:2:SIN:PRÄ:.*"),
            pos_regex("PRP:.*"),
        ],
        vec![
            pos("SENT_START"),
            pos_regex("VER:2:SIN:PRÄ:.*"),
            pos_regex("ADJ:PRD:.*"),
        ],
        vec![pos("SENT_START"), pos_regex("VER:2:SIN:PRÄ:.*"), pos("ZUS")],
        vec![
            pos("SENT_START"),
            pos("VER:MOD:2:SIN:KJ2"),
            pos_regex("PRO:.*"),
        ],
        vec![
            pos("SENT_START"),
            pos("VER:MOD:2:SIN:PRÄ"),
            negate(token_regex("ich|er|sie|es|wir|ihr|sie")),
        ],
        vec![token("wir"), token("frische")],
        vec![
            token("zum"),
            cs_token("Du"),
            inflected(token_regex("wechseln|übergehen|schwenken")),
        ],
        vec![token("ich"), token("schlafen"), token("gehe")],
        vec![token("ich"), token_regex("bin|war"), token("du")],
        vec![
            token_regex("darum|deswegen|dann|bitte|so|,|-"),
            pos_regex("VER:IMP:SIN.*"),
            token("du"),
        ],
        vec![
            token_regex("[-–]"),
            pos_regex("VER:.*(AUX|MOD).*"),
            token("du"),
            pos_regex("VER:INF.*"),
        ],
        vec![token_regex("-(du|ich|er|sie|wir|ihr)"), pos_regex("VER.*")],
        vec![token_regex("bin|war|wär"), token_regex("i|icke?")],
        vec![token_regex("i|icke?"), token_regex("bin|war|wär")],
        vec![token("du"), token("schlafen"), token("gehst")],
        vec![
            token("per"),
            token("du"),
            token_regex("sind|waren|sein|wären|war|ist|gewesen"),
        ],
        vec![token("schnellst"), token("möglich")],
        vec![token("er"), token("schlafen"), token("geht")],
        vec![token("vermittelst")],
        vec![token("du"), token("denkst"), token("ich")],
        vec![token("na"), token("komm")],
        vec![
            token_regex("muß|mußten?|müßt?en?"),
            token_regex("ich|wir|sie|er|es"),
        ],
        vec![
            token_regex("ich|wir|sie|er|es"),
            pos_regex("VER.*INF.*"),
            token_regex("muß|mußten?|müßt?en?"),
        ],
        vec![token_regex("mußt|müßtest|mußtest"), token("du")],
        vec![
            token("du"),
            pos_regex("VER.*INF.*"),
            token_regex("mußt|müßtest|mußtest"),
        ],
        vec![
            token("ich"),
            token_regex("würd|könnt|werd|wollt|sollt|müsst|fürcht"),
            token_regex("['’`´‘]"),
        ],
        vec![
            token_regex("wir|sie|zu"),
            token_regex("seh|steh|geh"),
            token_regex("['’`´‘]"),
            token("n"),
        ],
        vec![token("ick"), token_regex("bin|war|wär|hab|hatte")],
        vec![token("#"), pos_regex("VER.*")],
        vec![
            token("wie"),
            token_regex("du|ihr|er|es|sie"),
            pos_regex("VER.*"),
        ],
        vec![token_regex("[-:]"), pos_regex("VER.*(MOD|AUX).*")],
        vec![
            pos_regex("UNKNOWN|EIG.*"),
            token("bin"),
            pos_regex("UNKNOWN|EIG.*"),
        ],
        vec![
            token_regex("du|sie"),
            token_regex("schei(ß|ss)"),
            pos_regex("SUB.*|UNKNOWN"),
        ],
        vec![token("Du"), token_regex("bist|warst|wärst")],
        vec![
            token("als"),
            token("auch"),
            token_regex("er|sie|wir|du|ich|ihr"),
        ],
        vec![
            token_regex("so|wie|zu"),
            token("lange"),
            token_regex("er|sie|wir|du|ich|ihr"),
        ],
        vec![
            skip(inflected(token_regex("so|genauso|ähnlich")), 2),
            token("wie"),
            token_regex("er|sie|du|ihr|ich"),
            pos_regex("VER.*"),
        ],
        vec![
            pos_regex("VER.*(MOD|AUX).*"),
            token("wie"),
            token_regex("er|sie|du|ihr|ich"),
            pos_regex("VER.*INF.*"),
        ],
        vec![
            token("wie"),
            pos_regex("ADJ:PRD:GRU.*"),
            token_regex("er|sie|du|ihr|ich"),
            pos_regex("VER.*"),
        ],
        vec![
            pos("SENT_START"),
            pos_regex("VER:2:SIN:.*"),
            pos_regex("ART.*|ADV.*|PRO:POS.*"),
        ],
        vec![
            token(","),
            pos_regex("EIG:.*|UNKNOWN"),
            token_regex("und|oder"),
            token("auch"),
            token("ich"),
        ],
        vec![
            pos_regex("VER.*"),
            token_regex("er|sie|ich|wir|du|es|ihr"),
            token_regex("gleich|bereit|lange|schnelle?|halt|bitte|dank"),
        ],
        vec![
            pos_regex("ADV.*|KON.*"),
            token_regex("er|sie|ich|wir|du|es|ihr"),
            token_regex("gleich|bereit|lange|schnelle?|halt|bitte|dank"),
        ],
        vec![
            pos_regex("ADV.*|KON.*"),
            token_regex("er|sie|ich|wir|du|es|ihr"),
            token_regex("verlegen"),
            pos_regex("VER.*"),
        ],
        vec![pos("SENT_START"), pos_regex("VER:2:SIN:.*"), token("nicht")],
        vec![skip(inflected(token("machen")), -1), token("halt")],
        vec![pos_regex("VER:.*"), token_regex("du|ihr"), token("auch")],
        vec![
            token("für"),
            token("Sie"),
            pos("VER:3:SIN:KJ1:SFT"),
            token("ich"),
        ],
        vec![
            token_regex("(irgend)?einer?|(irgend)?jemand"),
            token("wie"),
            token("du"),
            pos_regex("VER:3:.*"),
        ],
        vec![pos("VER:MOD:2:SIN:PRÄ"), pos_regex("PRO:PER:.*")],
        vec![
            token_regex("die|welche"),
            token_regex(".*"),
            token_regex("mehr|weniger"),
            token("als"),
            token_regex("ich|du|e[rs]|sie"),
        ],
        vec![token("wenn"), token("du"), token("anstelle")],
        vec![
            token_regex("ok(ay)?|ja|nein|vielleicht|oh"),
            token_regex("bin|sind"),
        ],
        vec![token("das"), cs_token("Du"), inflected(token("anbieten"))],
        vec![cs_token("Du"), inflected(token_regex("anreden|ansprechen"))],
        vec![
            pos_regex("SUB:.*PLU.*"),
            token("wie"),
            token("Du"),
            pos_regex("VER:3:PLU.*"),
        ],
        vec![token("würd"), token_regex("[nm]ich|man|ichs|'")],
        vec![token(","), pos_regex("VER:MOD:2:.*")],
        vec![cs_token("Soll"), token("ich")],
        vec![cs_token("Solltest"), token("du")],
        vec![cs_token("Müsstest"), token("dir")],
        vec![cs_token("Könntest"), token("dir")],
        vec![cs_token("Sollte"), token_regex("er|sie")],
        vec![pos("SENT_START"), token_regex("Bin|Kannst|Musst")],
        vec![token(","), token_regex("bin|hast|kannst|musst")],
        vec![token("er"), pos_regex("VER:.*"), token("wird")],
        vec![token_regex("wie|als"), token("ich"), pos_regex("VER:.*")],
        vec![
            token_regex("glaube?|denke?|hoffe?|vermute?|behaupte?|wette?"),
            token("ich"),
            pos_regex("ADV.*|SUB.*|UNKNOWN|ADJ.*|PA[12].*|ART.*|PRP.*|PRO.*"),
        ],
        vec![token_regex("ich"), pos("VER:INF:NON"), token("werde")],
        vec![
            pos_regex("VER:IMP:SIN:.*"),
            token("du"),
            token_regex("dich|dein|deine[srnm]?|mal"),
        ],
        vec![pos_regex("VER:IMP:SIN:.*"), token("du"), token("!")],
        vec![token("sei"), token("du"), token("selbst")],
        vec![token("bin"), token_regex("dran|dabei")],
        vec![
            token("als"),
            token("ich"),
            pos_regex("PA2:.*"),
            token("bin"),
        ],
        vec![
            token("als"),
            token_regex("du|e[rs]|sie|ich"),
            inflected(token("sein")),
            token_regex("[\\.,]"),
        ],
        vec![
            token_regex("D[au]rf.*|Muss.*"),
            pos_regex("PRO:PER:NOM:.+"),
            pos_regex("VER:INF:.+"),
            pos("PKT"),
            token_regex("(?!die).+"),
        ],
        vec![cs_token("("), pos_regex("VER:2:SIN:.+"), cs_token(")")],
        vec![
            pos_regex("VER:MOD:1:PLU:.+"),
            cs_token("wir"),
            cs_token("bitte"),
        ],
        vec![token("ohne"), token("sie"), token("hätte"), token("ich")],
        vec![
            pos("SENT_START"),
            pos_regex("VER:IMP:SIN.+"),
            token("du"),
            negate(cs_token("?")),
        ],
        vec![token_regex("[^a-zäöüß]+du"), pos("VER:2:SIN:PRÄ:SFT")],
        vec![
            pos_regex("PRO:IND.*"),
            pos_regex("SUB:.+:PLU.*"),
            token_regex("wie|als"),
            pos_regex("PRO:PER.+"),
            pos_regex("VER:[1-3]:PLU.*"),
        ],
        vec![
            pos("ADV:MOD"),
            token_regex("als"),
            pos_regex("PRO:PER.+"),
            pos_regex("VER:3:SIN.*"),
            pos_regex("PRO:IND:NOM:SIN.*"),
        ],
        vec![
            pos_regex("PRO:IND.*"),
            token_regex("wie"),
            pos_regex("PRO:PER.+"),
            pos_regex("VER:.*:SIN.*"),
            pos_regex("PRO:PER:NOM:SIN.*"),
        ],
        vec![
            token_regex("kein|keine"),
            token_regex("anderer|andere"),
            token("als"),
            token_regex("ich|du|er|sie|es"),
            pos_regex("VER:MOD.*:PRÄ"),
        ],
        vec![
            pos_regex("ART:DEF.*"),
            token_regex("gleich(e|en)|selb(e|en)"),
            pos_regex("SUB:.+"),
            token("wie"),
            pos_regex("PRO:PER:NOM:SIN.*"),
            pos_regex("VER:INF.*"),
        ],
        vec![
            token("wenn"),
            pos_regex("PRO:PER:NOM:SIN.+"),
            pos_regex("PRO:PER:NOM.+"),
            pos_regex("VER:AUX:[1-3]:SIN:KJ2"),
        ],
        vec![
            token("wenn"),
            pos_regex("PRO:PER:NOM:PLU.+"),
            pos_regex("PRO:PER:NOM.+"),
            pos_regex("VER:AUX:[1-3]:PLU:KJ2"),
        ],
        vec![
            token("wenn"),
            token("du"),
            token("gehen"),
            token("willst"),
            token(","),
            token("dann"),
            token("geh"),
        ],
        vec![
            token("ob"),
            token("ich"),
            pos_regex("VER:1.+"),
            token("oder"),
            pos_regex("VER:1.+"),
            inflected(token("gehen")),
            pos_regex("VER:MOD:1.*"),
        ],
        vec![token("mal"), token("seh"), token_regex("’|'"), token("n")],
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
        assert_eq!(antipattern_defs().len(), 96);
    }

    #[test]
    fn near_semantics() {
        assert!(is_near(4, 8));
        assert!(!is_near(4, 9));
        assert!(!is_near(-1, 3));
    }

    #[test]
    fn java_hash_set_order_matches_hashmap_buckets() {
        // "ist", "bist", "bin" -> Java HashSet iteration order, verified
        // with a JDK 21 snippet (16 buckets)
        let items = vec!["ist".to_string(), "bist".to_string(), "bin".to_string()];
        assert_eq!(
            util::java_hash_set_order(&items),
            vec!["bist".to_string(), "bin".to_string(), "ist".to_string()]
        );
    }
}
