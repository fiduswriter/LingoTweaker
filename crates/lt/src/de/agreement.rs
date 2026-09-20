//! `AgreementRule` (`DE_AGREEMENT`) and `AgreementRule2` (`DE_AGREEMENT2`):
//! agreement inside German noun phrases (determiner/adjective/noun) and at
//! sentence start (adjective/noun).
//!
//! `AgreementRule`'s compound-error branch runs Java's nested
//! `JLanguageTool` check (`lt.check(testPhrase)` with only DE_AGREEMENT and
//! GERMAN_SPELLER_RULE active). The port reproduces that with a local
//! tokenize+tag+agreement-core check (compound errors disabled to avoid
//! recursion) plus `GermanSpellingRule::check_sentence`.

use std::sync::Arc;

use lt_core::{
    AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, Match, Suggestion, TextRange,
};
use lt_pattern::matcher as pm;
use lt_pattern::PatternToken;

use super::agreement_antipatterns as ap;
use super::agreement_suggestor::{AgreementSuggestor2, ReplacementType};
use super::german_helper::{self, PosType};
use super::util;

pub const AGREEMENT_ID: &str = "DE_AGREEMENT";
const AGREEMENT_DESCRIPTION: &str =
    "Kongruenz von Nominalphrasen (unvollständig!), z.B. 'mein kleiner (kleines) Haus'";
pub const AGREEMENT2_ID: &str = "DE_AGREEMENT2";
const AGREEMENT2_DESCRIPTION: &str =
    "Kongruenz von Adjektiv und Nomen (unvollständig!), z.B. 'kleiner (kleines) Haus'";
const CATEGORY_ID: &str = "GRAMMAR";
const CATEGORY_NAME: &str = "Grammatik";

const MSG: &str =
    "Möglicherweise passen das Nomen und die Wörter, die das Nomen beschreiben, grammatisch nicht zusammen.";
const SHORT_MSG: &str = "Evtl. passen Wörter grammatisch nicht zusammen.";
const MSG2: &str =
    "Möglicherweise passen das Nomen und die Wörter, die das Nomen beschreiben, grammatisch nicht zusammen.";
const COMPOUND_MSG: &str =
    "Wenn es sich um ein zusammengesetztes Nomen handelt, wird es zusammengeschrieben.";

/// `AgreementRule.MODIFIERS`.
const MODIFIERS: [&str; 39] = [
    "zu",
    "überraschend",
    "ungeahnt",
    "absolut",
    "ausgesprochen",
    "außergewöhnlich",
    "außerordentlich",
    "äußerst",
    "besonders",
    "dringend",
    "echt",
    "einigermaßen",
    "enorm",
    "extrem",
    "fast",
    "ganz",
    "entschieden",
    "geradezu",
    "zeitweise",
    "halbwegs",
    "höchst",
    "komplett",
    "laufend",
    "recht",
    "relativ",
    "sehr",
    "total",
    "überaus",
    "ungewöhnlich",
    "unglaublich",
    "völlig",
    "weit",
    "wirklich",
    "gerade",
    "vereint",
    "überwiegend",
    "gewollt",
    "angestrengt",
    "ziemlich",
];

/// `AgreementRule.VIELE_WENIGE_LOWERCASE`.
const VIELE_WENIGE_LOWERCASE: [&str; 18] = [
    "manche",
    "jegliche",
    "jeglicher",
    "andere",
    "anderer",
    "anderen",
    "sämtliche",
    "sämtlicher",
    "etliche",
    "etlicher",
    "viele",
    "vieler",
    "wenige",
    "weniger",
    "einige",
    "einiger",
    "mehrerer",
    "mehrere",
];

/// `AgreementRule.PRONOUNS_TO_BE_IGNORED`.
const PRONOUNS_TO_BE_IGNORED: [&str; 49] = [
    "nichts",
    "alles",
    "dies",
    "ebendies",
    "ich",
    "dir",
    "dich",
    "du",
    "d",
    "er",
    "sie",
    "es",
    "wir",
    "mich",
    "mir",
    "uns",
    "ihnen",
    "euch",
    "ihm",
    "ihr",
    "ihn",
    "dessen",
    "deren",
    "denen",
    "sich",
    "aller",
    "allen",
    "man",
    "beide",
    "beiden",
    "beider",
    "wessen",
    "a",
    "alle",
    "etwas",
    "irgendetwas",
    "irgendwas",
    "irgendwer",
    "was",
    "wer",
    "wem",
    "jenen",
    "diejenigen",
    "irgendjemand",
    "irgendjemandes",
    "jemand",
    "jemandes",
    "niemand",
    "niemandes",
];

/// `AgreementRule.NOUNS_TO_BE_IGNORED`.
const NOUNS_TO_BE_IGNORED: [&str; 34] = [
    "A",
    "Prozent",
    "Wollen",
    "Gramm",
    "Kilogramm",
    "Flippers",
    "Standart",
    "Stellungsname",
    "Kündigungsscheiben",
    "Piepen",
    "Badlands",
    "Visual",
    "Special",
    "Multiple",
    "Chief",
    "Carina",
    "Wüstenrot",
    "Rückgrad",
    "Rückgrads",
    "Anteilname",
    "Aalen",
    "Meter",
    "Boots",
    "Taxameter",
    "Bild",
    "Emirates",
    "Uhr",
    "cm",
    "km",
    "Nr",
    "KSC",
    "ANC",
    "DJK",
    "RP",
];

static ALL_ANTIPATTERNS: std::sync::LazyLock<Vec<Arc<pm::CompiledPattern>>> =
    std::sync::LazyLock::new(|| compile(ap::antipattern_defs()));
static AGREEMENT2_ANTIPATTERNS: std::sync::LazyLock<Vec<Arc<pm::CompiledPattern>>> =
    std::sync::LazyLock::new(|| compile(ap::agreement2_antipattern_defs()));

fn compile(defs: Vec<Vec<PatternToken>>) -> Vec<Arc<pm::CompiledPattern>> {
    defs.iter()
        .flat_map(|tokens| pm::compile_patterns(tokens, None, None).unwrap_or_default())
        .map(Arc::new)
        .collect()
}

/// `AgreementRule` + the nested-check dependencies it needs
/// (tagger/speller for the compound branch).
pub struct AgreementRule {
    synth: Arc<lt_tagger::GermanSynthesizer>,
    tagger: Arc<lt_tagger::GermanTagger>,
    spelling: Arc<crate::de::spelling::GermanSpellingRule>,
}

impl AgreementRule {
    pub fn new(
        synth: Arc<lt_tagger::GermanSynthesizer>,
        tagger: Arc<lt_tagger::GermanTagger>,
        spelling: Arc<crate::de::spelling::GermanSpellingRule>,
    ) -> Self {
        Self {
            synth,
            tagger,
            spelling,
        }
    }

    /// `estimateContextForSureMatch`.
    pub fn estimate_context_for_sure_match() -> i32 {
        ap::antipattern_defs()
            .iter()
            .map(|p| p.len() as i32)
            .max()
            .unwrap_or(0)
    }

    /// `AgreementRule.match(AnalyzedSentence)`.
    pub fn match_sentence(
        &self,
        sentence: &AnalyzedSentence,
        sentence_offset: usize,
    ) -> Vec<Match> {
        self.match_sentence_impl(sentence, sentence_offset, true)
    }

    fn match_sentence_impl(
        &self,
        sentence: &AnalyzedSentence,
        sentence_offset: usize,
        with_compound_errors: bool,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let immunized = util::immunize_sentence(sentence, &ALL_ANTIPATTERNS);
        let orig: Vec<AnalyzedTokenReadings> = immunized
            .tokens_without_whitespace()
            .iter()
            .map(|t| (*t).clone())
            .collect();
        let mut tokens = orig.clone();
        let repl_map = replace_prepositions_by_article(&mut tokens);
        for i in 0..tokens.len() {
            let pos_token = tokens[i].readings.first().and_then(|r| r.pos_tag.clone());
            if pos_token.as_deref() == Some("SENT_START")
                || tokens[i].is_immunized
                || orig[i].is_immunized
            {
                continue;
            }
            if could_be_relative_or_dependent_clause(&tokens, i) {
                continue;
            }
            if i > 0 {
                let prev_token = tokens[i - 1].surface().to_lowercase();
                if ["der", "die", "das", "des", "dieses"].contains(&prev_token.as_str())
                    && matches!(tokens[i].surface(), "eine" | "einen")
                {
                    continue;
                }
            }
            let token_readings = &tokens[i];
            let det_abbrev = i + 2 < tokens.len()
                && tokens[i + 1].surface() == "Art"
                && tokens[i + 2].surface() == ".";
            let det_adj_abbrev = i + 3 < tokens.len()
                && tokens[i + 2].surface() == "Art"
                && tokens[i + 3].surface() == ".";
            let following_participle = i + 2 < tokens.len()
                && (util::has_partial_pos_tag(&tokens[i + 2], "PA1")
                    || ZUGESCHRIEBENEN_GENANNTEN.is_match(tokens[i + 2].surface()));
            if det_abbrev || det_adj_abbrev || following_participle {
                continue;
            }
            if german_helper::has_reading_of_type(token_readings, PosType::Determiner)
                || is_relevant_pronoun(&tokens, i)
            {
                let token_pos_after_modifier = get_pos_after_modifier(i + 1, &tokens);
                // Java: `sentence.getText().substring(...)` of the original
                // sentence (byte offsets in the engine).
                let skipped_str = if token_pos_after_modifier > i + 1 {
                    sentence
                        .text
                        .get(
                            tokens[i + 1].start_pos..tokens[token_pos_after_modifier - 1].end_pos(),
                        )
                        .map(str::to_string)
                } else {
                    None
                };
                let token_pos = token_pos_after_modifier;
                if token_pos >= tokens.len() {
                    break;
                }
                let next_token = &tokens[token_pos];
                let mut maybe_preposition: Option<&AnalyzedTokenReadings> =
                    if i >= 1 { Some(&tokens[i - 1]) } else { None };
                if i >= 2 && tokens[i - 2].surface().eq_ignore_ascii_case("was") {
                    maybe_preposition = None;
                }
                if is_non_predicative_adjective(next_token) || is_participle(next_token) {
                    let token_pos = token_pos_after_modifier + 1;
                    if token_pos >= tokens.len() {
                        break;
                    }
                    if german_helper::has_reading_of_type(&tokens[token_pos], PosType::Nomen) {
                        if i >= 2
                            && german_helper::has_reading_of_type(&tokens[i - 2], PosType::Adjektiv)
                            && tokens[i - 1].surface() == "als"
                            && tokens[i].surface() == "das"
                        {
                            continue;
                        }
                        let allow_suggestion = token_pos == i + 2;
                        let repl = if allow_suggestion {
                            Some(&repl_map)
                        } else {
                            None
                        };
                        if let Some(m) = self.check_det_adj_noun_agreement(
                            maybe_preposition,
                            &tokens[i],
                            next_token,
                            &tokens[token_pos],
                            &orig,
                            sentence_offset,
                            i,
                            repl,
                            skipped_str.as_deref(),
                            with_compound_errors,
                        ) {
                            rule_matches.push(m);
                        }
                    } else if token_pos + 1 < tokens.len()
                        && german_helper::has_reading_of_type(
                            &tokens[token_pos + 1],
                            PosType::Nomen,
                        )
                        && german_helper::has_reading_of_type(&tokens[token_pos], PosType::Adjektiv)
                    {
                        if let Some(m) = self.check_det_adj_adj_noun_agreement(
                            maybe_preposition,
                            &tokens[i],
                            next_token,
                            &tokens[token_pos],
                            &tokens[token_pos + 1],
                            &orig,
                            sentence_offset,
                            i,
                            &repl_map,
                            skipped_str.as_deref(),
                            with_compound_errors,
                        ) {
                            rule_matches.push(m);
                        }
                    }
                } else if german_helper::has_reading_of_type(next_token, PosType::Nomen)
                    && next_token.surface() != "Herr"
                {
                    if let Some(m) = self.check_det_noun_agreement(
                        maybe_preposition,
                        &tokens[i],
                        next_token,
                        &orig,
                        sentence_offset,
                        i,
                        &repl_map,
                        skipped_str.as_deref(),
                        with_compound_errors,
                    ) {
                        rule_matches.push(m);
                    }
                }
            }
        }
        rule_matches
    }

    #[allow(clippy::too_many_arguments)]
    fn check_det_noun_agreement(
        &self,
        maybe_preposition: Option<&AnalyzedTokenReadings>,
        token1: &AnalyzedTokenReadings,
        token2: &AnalyzedTokenReadings,
        orig_tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
        token_pos: usize,
        repl_map: &std::collections::HashMap<usize, ReplacementType>,
        skipped_str: Option<&str>,
        with_compound_errors: bool,
    ) -> Option<Match> {
        if token2.is_immunized
            || NOUNS_TO_BE_IGNORED.contains(&token2.surface())
            || token2.surface() == "-"
        {
            return None;
        }
        let mut set1: Vec<String> = if token1.readings.len() == 1
            && token1.readings[0]
                .pos_tag
                .as_deref()
                .is_some_and(|t| t.ends_with(":STV"))
        {
            Vec::new()
        } else {
            german_helper::get_agreement_categories(token1, &[], false)
        };
        let set2 = german_helper::get_agreement_categories(token2, &[], false);
        set1.retain(|c| set2.contains(c));
        if set1.is_empty() && !(token1.surface() == "allen" && token2.surface() == "Grund") {
            if with_compound_errors {
                if let Some(compound_match) = self.get_compound_error2(
                    token1,
                    token2,
                    sentence_offset,
                    token_pos,
                    orig_tokens,
                ) {
                    return Some(compound_match);
                }
            }
            let message = MSG;
            let mut suggestor = AgreementSuggestor2::new(
                &self.synth,
                token1,
                None,
                None,
                token2,
                repl_map.get(&token_pos).copied(),
            );
            suggestor.set_preposition(maybe_preposition);
            suggestor.set_skipped(skipped_str.map(str::to_string));
            let suggestions = suggestor.get_suggestions(true);
            return Some(match_from_parts(
                token1.start_pos,
                token2.end_pos(),
                sentence_offset,
                message,
                SHORT_MSG,
                suggestions,
            ));
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn check_det_adj_noun_agreement(
        &self,
        maybe_preposition: Option<&AnalyzedTokenReadings>,
        token1: &AnalyzedTokenReadings,
        token2: &AnalyzedTokenReadings,
        token3: &AnalyzedTokenReadings,
        orig_tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
        token_pos: usize,
        repl_map: Option<&std::collections::HashMap<usize, ReplacementType>>,
        skipped_str: Option<&str>,
        with_compound_errors: bool,
    ) -> Option<Match> {
        if token3.surface().chars().count() < 2 {
            return None;
        }
        let set = retain_common_categories3(token1, token2, token3);
        if !set.is_empty() {
            return None;
        }
        if HERR_FRAU.is_match(token3.surface()) && token_pos + 3 < orig_tokens.len() {
            let token4 = &orig_tokens[token_pos + 3];
            if !is_tagged(token4) || token4.has_pos_tag_starting_with("EIG:") {
                return None;
            }
        }
        if with_compound_errors && token_pos + 4 < orig_tokens.len() {
            if let Some(m) = self.get_compound_error4(
                &orig_tokens[token_pos],
                &orig_tokens[token_pos + 1],
                &orig_tokens[token_pos + 2],
                &orig_tokens[token_pos + 3],
                token_pos,
                orig_tokens,
                sentence_offset,
                None,
            ) {
                return Some(m);
            }
        }
        if with_compound_errors {
            if let Some(m) = self.get_compound_error3(
                token1,
                token2,
                token3,
                token_pos,
                orig_tokens,
                sentence_offset,
            ) {
                return Some(m);
            }
        }
        if token3.has_pos_tag_starting_with("ABK") {
            return None;
        }
        let mut suggestor = AgreementSuggestor2::new(
            &self.synth,
            token1,
            Some(token2),
            None,
            token3,
            repl_map.and_then(|m| m.get(&token_pos)).copied(),
        );
        suggestor.set_preposition(maybe_preposition);
        suggestor.set_skipped(skipped_str.map(str::to_string));
        let suggestions = suggestor.get_suggestions(true);
        Some(match_from_parts(
            token1.start_pos,
            token3.end_pos(),
            sentence_offset,
            MSG,
            SHORT_MSG,
            suggestions,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn check_det_adj_adj_noun_agreement(
        &self,
        maybe_preposition: Option<&AnalyzedTokenReadings>,
        token1: &AnalyzedTokenReadings,
        token2: &AnalyzedTokenReadings,
        token3: &AnalyzedTokenReadings,
        token4: &AnalyzedTokenReadings,
        orig_tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
        token_pos: usize,
        repl_map: &std::collections::HashMap<usize, ReplacementType>,
        skipped_str: Option<&str>,
        with_compound_errors: bool,
    ) -> Option<Match> {
        let set = retain_common_categories4(token1, token2, token3, token4);
        if !set.is_empty() {
            return None;
        }
        if with_compound_errors {
            if let Some(m) = self.get_compound_error4(
                token1,
                token2,
                token3,
                token4,
                token_pos,
                orig_tokens,
                sentence_offset,
                skipped_str,
            ) {
                return Some(m);
            }
        }
        if token4.has_pos_tag_starting_with("ABK") {
            return None;
        }
        let mut suggestor = AgreementSuggestor2::new(
            &self.synth,
            token1,
            Some(token2),
            Some(token3),
            token4,
            repl_map.get(&token_pos).copied(),
        );
        suggestor.set_preposition(maybe_preposition);
        suggestor.set_skipped(skipped_str.map(str::to_string));
        let suggestions = suggestor.get_suggestions(true);
        Some(match_from_parts(
            token1.start_pos,
            token4.end_pos(),
            sentence_offset,
            MSG2,
            SHORT_MSG,
            suggestions,
        ))
    }

    /// `getCompoundError(token1, token2, tokenPos, sentence)`.
    fn get_compound_error2(
        &self,
        token1: &AnalyzedTokenReadings,
        token2: &AnalyzedTokenReadings,
        sentence_offset: usize,
        token_pos: usize,
        orig_tokens: &[AnalyzedTokenReadings],
    ) -> Option<Match> {
        if token_pos + 2 < orig_tokens.len() {
            let next_token = &orig_tokens[token_pos + 2];
            if starts_with_uppercase(next_token.surface()) {
                if token2.start_pos == next_token.start_pos {
                    return None;
                }
                let potential_compound = format!(
                    "{}{}",
                    token2.surface(),
                    lt_tagger::lowercase_first_char(next_token.surface())
                );
                let orig_token1 = &orig_tokens[token_pos];
                let test_phrase = format!("{} {}", orig_token1.surface(), potential_compound);
                let hyphen_potential_compound =
                    format!("{}-{}", token2.surface(), next_token.surface());
                let hyphen_test_phrase =
                    format!("{} {}", orig_token1.surface(), hyphen_potential_compound);
                return self.get_rule_match(
                    token1,
                    next_token,
                    sentence_offset,
                    &test_phrase,
                    &hyphen_test_phrase,
                );
            }
        }
        None
    }

    /// `getCompoundError(token1, token2, token3, tokenPos, sentence)`.
    fn get_compound_error3(
        &self,
        token1: &AnalyzedTokenReadings,
        token2: &AnalyzedTokenReadings,
        token3: &AnalyzedTokenReadings,
        token_pos: usize,
        orig_tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Option<Match> {
        if token_pos + 3 < orig_tokens.len() {
            let next_token = &orig_tokens[token_pos + 3];
            if starts_with_uppercase(next_token.surface()) {
                if token3.start_pos == next_token.start_pos {
                    return None;
                }
                let potential_compound = format!(
                    "{}{}",
                    token3.surface(),
                    lt_tagger::lowercase_first_char(next_token.surface())
                );
                let orig_token1 = &orig_tokens[token_pos];
                let test_phrase = format!(
                    "{} {} {}",
                    orig_token1.surface(),
                    token2.surface(),
                    potential_compound
                );
                let hyphen_potential_compound =
                    format!("{}-{}", token3.surface(), next_token.surface());
                let hyphen_test_phrase = format!(
                    "{} {} {}",
                    orig_token1.surface(),
                    token2.surface(),
                    hyphen_potential_compound
                );
                return self.get_rule_match(
                    token1,
                    next_token,
                    sentence_offset,
                    &test_phrase,
                    &hyphen_test_phrase,
                );
            }
        }
        None
    }

    /// `getCompoundError(token1, token2, token3, token4, tokenPos, sentence, skippedStr)`.
    #[allow(clippy::too_many_arguments)]
    fn get_compound_error4(
        &self,
        token1: &AnalyzedTokenReadings,
        token2: &AnalyzedTokenReadings,
        token3: &AnalyzedTokenReadings,
        token4: &AnalyzedTokenReadings,
        token_pos: usize,
        orig_tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
        skipped_str: Option<&str>,
    ) -> Option<Match> {
        let idx = token_pos + 4 + usize::from(skipped_str.is_some());
        if idx < orig_tokens.len() {
            let next_token = &orig_tokens[idx];
            let potential_compound = format!(
                "{}{}",
                token4.surface(),
                lt_tagger::lowercase_first_char(next_token.surface())
            );
            if starts_with_uppercase(token4.surface())
                && starts_with_uppercase(next_token.surface())
            {
                if token4.start_pos == next_token.start_pos {
                    return None;
                }
                let orig_token1 = &orig_tokens[token_pos];
                let skipped_part = skipped_str.map(|s| format!(" {s} ")).unwrap_or(" ".into());
                let test_phrase = format!(
                    "{}{}{} {} {}",
                    orig_token1.surface(),
                    skipped_part,
                    token2.surface(),
                    token3.surface(),
                    potential_compound
                );
                let hyphen_potential_compound =
                    format!("{}-{}", token4.surface(), next_token.surface());
                let hyphen_test_phrase = format!(
                    "{}{}{} {} {}",
                    orig_token1.surface(),
                    skipped_part,
                    token2.surface(),
                    token3.surface(),
                    hyphen_potential_compound
                );
                return self.get_rule_match(
                    token1,
                    next_token,
                    sentence_offset,
                    &test_phrase,
                    &hyphen_test_phrase,
                );
            }
        }
        None
    }

    /// `getRuleMatch`: nested `lt.check(phrase)` (DE_AGREEMENT + speller).
    fn get_rule_match(
        &self,
        token: &AnalyzedTokenReadings,
        token2: &AnalyzedTokenReadings,
        sentence_offset: usize,
        test_phrase: &str,
        hyphen_test_phrase: &str,
    ) -> Option<Match> {
        if token2
            .readings
            .iter()
            .all(|k| k.pos_tag.as_deref().is_some_and(|t| !t.starts_with("SUB")))
        {
            return None;
        }
        let mut replacements = Vec::new();
        if !self.phrase_finds_match(test_phrase) && is_tagged(token2) {
            replacements.push(test_phrase.to_string());
        }
        if !self.phrase_finds_match(hyphen_test_phrase) && is_tagged(token2) {
            replacements.push(hyphen_test_phrase.to_string());
        }
        if replacements.is_empty() {
            return None;
        }
        Some(match_from_parts(
            token.start_pos,
            token2.end_pos(),
            sentence_offset,
            COMPOUND_MSG,
            "",
            replacements,
        ))
    }

    /// The nested check: does DE_AGREEMENT or the German speller fire on the
    /// phrase? (Java `lt.check(testPhrase).isEmpty()`.)
    fn phrase_finds_match(&self, phrase: &str) -> bool {
        let sentence = analyze_plain_sentence(&self.tagger, phrase);
        if !self.match_sentence_impl(&sentence, 0, false).is_empty() {
            return true;
        }
        !self.spelling.check_sentence(&sentence.tokens, 0).is_empty()
    }
}

/// `AgreementRule2` (`DE_AGREEMENT2`).
pub struct AgreementRule2 {
    synth: Arc<lt_tagger::GermanSynthesizer>,
}

impl AgreementRule2 {
    pub fn new(synth: Arc<lt_tagger::GermanSynthesizer>) -> Self {
        Self { synth }
    }

    /// `estimateContextForSureMatch`.
    pub fn estimate_context_for_sure_match() -> i32 {
        ap::agreement2_antipattern_defs()
            .iter()
            .map(|p| p.len() as i32)
            .max()
            .unwrap_or(0)
    }

    /// `AgreementRule2.match`.
    pub fn match_sentence(
        &self,
        sentence: &AnalyzedSentence,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let immunized = util::immunize_sentence(sentence, &AGREEMENT2_ANTIPATTERNS);
        let tokens = immunized.tokens_without_whitespace();
        for i in 0..tokens.len() {
            let token = tokens[i].surface();
            if !tokens[i].is_sentence_start && !matches!(token, "\"" | "„" | "»" | "«") {
                // skip quotes, as these are not relevant
                if i + 1 < tokens.len()
                    && tokens[i].has_pos_tag_starting_with("ADJ")
                    && tokens[i + 1].has_pos_tag_starting_with("SUB")
                    && !tokens[i + 1].has_pos_tag_starting_with("EIG")
                {
                    if tokens[i].is_immunized
                        || tokens[i + 1].is_immunized
                        || tokens[i].surface().eq_ignore_ascii_case("unter")
                    {
                        continue;
                    }
                    if i + 2 < tokens.len() && tokens[i + 2].has_pos_tag_starting_with("SUB") {
                        // no alarm for e.g. "Deutscher Taschenbuch Verlag"
                        break;
                    }
                    if let Some(m) =
                        self.check_adj_noun_agreement(tokens[i], tokens[i + 1], sentence_offset)
                    {
                        let suggestions = self.get_suggestions(&tokens, i);
                        rule_matches.push(with_suggestions(m, suggestions));
                        break;
                    }
                } else {
                    // rule only works at sentence start (minus quotes)
                    break;
                }
            }
        }
        rule_matches
    }

    /// `AgreementRule2.getSuggestions`.
    fn get_suggestions(&self, tokens: &[&AnalyzedTokenReadings], i: usize) -> Vec<String> {
        let mut suggestions = Vec::new();
        let Some(adj_token) = tokens[i].readings.first() else {
            return suggestions;
        };
        for noun_token in &tokens[i + 1].readings {
            let Some(pos_tag) = noun_token.pos_tag.as_deref() else {
                continue;
            };
            let Some(gender) = get_gender(pos_tag) else {
                continue;
            };
            let Some(number) = get_number(pos_tag) else {
                continue;
            };
            let forms = self.synth.synthesize(
                adj_token,
                &format!("ADJ:NOM:{number}:{gender}:GRU:SOL"),
                true,
            );
            for s in forms {
                let full_sugg = format!(
                    "{} {}",
                    lt_tagger::uppercase_first_char(&s),
                    noun_token.token
                );
                if !suggestions.contains(&full_sugg) {
                    suggestions.push(full_sugg);
                }
            }
        }
        suggestions
    }

    /// `AgreementRule2.checkAdjNounAgreement`.
    fn check_adj_noun_agreement(
        &self,
        token1: &AnalyzedTokenReadings,
        token2: &AnalyzedTokenReadings,
        sentence_offset: usize,
    ) -> Option<Match> {
        let mut set1 = german_helper::get_agreement_categories(token1, &[], false);
        let set2 = german_helper::get_agreement_categories(token2, &[], false);
        set1.retain(|c| set2.contains(c));
        if set1.is_empty() {
            let msg = "Möglicherweise fehlende grammatikalische Übereinstimmung zwischen Adjektiv und Nomen bezüglich Kasus, Numerus oder Genus. Beispiel: 'kleiner Haus' statt 'kleines Haus'";
            let short_msg =
                "Möglicherweise keine Übereinstimmung bezüglich Kasus, Numerus oder Genus";
            return Some(match_from_parts(
                token1.start_pos,
                token2.end_pos(),
                sentence_offset,
                msg,
                short_msg,
                Vec::new(),
            ));
        }
        None
    }
}

fn get_gender(pos_tag: &str) -> Option<&'static str> {
    if pos_tag.contains(":MAS") {
        Some("MAS")
    } else if pos_tag.contains(":FEM") {
        Some("FEM")
    } else if pos_tag.contains(":NEU") {
        Some("NEU")
    } else {
        None
    }
}

fn get_number(pos_tag: &str) -> Option<&'static str> {
    if pos_tag.contains(":SIN:") {
        Some("SIN")
    } else if pos_tag.contains(":PLU:") {
        Some("PLU")
    } else {
        None
    }
}

/// `AgreementRule.replacePrepositionsByArticle` (with the replacement map).
fn replace_prepositions_by_article(
    tokens: &mut [AnalyzedTokenReadings],
) -> std::collections::HashMap<usize, ReplacementType> {
    let mut map = std::collections::HashMap::new();
    for (i, token) in tokens.iter_mut().enumerate() {
        if [
            "ins", "ans", "aufs", "vors", "durchs", "hinters", "unters", "übers", "fürs", "ums",
        ]
        .contains(&token.surface())
        {
            *token = replacement_token(
                "das",
                "ART:DEF:AKK:SIN:NEU",
                "das",
                token.start_pos,
                token.raw_byte_len,
            );
            map.insert(i, ReplacementType::Ins);
        } else if token.surface() == "zur" {
            *token = replacement_token(
                "der",
                "ART:DEF:DAT:SIN:FEM",
                "der",
                token.start_pos,
                token.raw_byte_len,
            );
            map.insert(i, ReplacementType::Zur);
        }
    }
    map
}

fn replacement_token(
    surface: &str,
    postag: &str,
    lemma: &str,
    start_pos: usize,
    raw_byte_len: usize,
) -> AnalyzedTokenReadings {
    let mut token = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
        surface,
        Some(lemma.to_string()),
        Some(postag.to_string()),
    )]);
    token.start_pos = start_pos;
    token.raw_byte_len = raw_byte_len;
    token
}

/// `AgreementRule.getPosAfterModifier`.
fn get_pos_after_modifier(start_at: usize, tokens: &[AnalyzedTokenReadings]) -> usize {
    let n = tokens.len();
    let mut start_at = start_at;
    if start_at < n
        && tokens[start_at].surface() == "relativ"
        && start_at + 1 < n
        && tokens[start_at + 1].surface() == "gesehen"
    {
        start_at += 2;
    }
    if start_at < n
        && VIEL_WEIT.is_match(tokens[start_at].surface())
        && start_at + 1 < n
        && WENIGER_EHER.is_match(tokens[start_at + 1].surface())
    {
        start_at += 2;
    } else if start_at + 1 < n && MODIFIERS.contains(&tokens[start_at].surface()) {
        start_at += 1;
    }
    if start_at + 1 < n {
        let phrase = format!(
            "{} {}",
            tokens[start_at].surface(),
            tokens[start_at + 1].surface()
        )
        .to_lowercase();
        if MIT_MIR_ETC.is_match(&phrase) || OHNE_MICH_ETC.is_match(&phrase) {
            start_at += 2;
        }
    }
    if start_at + 1 < n
        && (is_numeric(tokens[start_at].surface()) || tokens[start_at].has_pos_tag("ZAL"))
    {
        let mut pos_after_modifier = start_at + 1;
        if start_at + 3 < n
            && tokens[start_at + 1].surface() == ","
            && is_numeric(tokens[start_at + 2].surface())
        {
            pos_after_modifier = start_at + 3;
        }
        let token = tokens[pos_after_modifier].surface();
        if ["gramm", "Gramm", "Meter", "meter"]
            .iter()
            .any(|suffix| token.ends_with(suffix))
        {
            return pos_after_modifier + 1;
        }
    }
    start_at
}

/// `StringUtils.isNumeric` (non-empty, all chars numeric).
fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(char::is_numeric)
}

/// `AgreementRule.isRelevantPronoun`.
fn is_relevant_pronoun(tokens: &[AnalyzedTokenReadings], pos: usize) -> bool {
    let mut relevant_pronoun = german_helper::has_reading_of_type(&tokens[pos], PosType::Pronomen);
    let token = tokens[pos].surface();
    if PRONOUNS_TO_BE_IGNORED.contains(&token.to_lowercase().as_str())
        || (pos > 0
            && tokens[pos - 1].surface().eq_ignore_ascii_case("vor")
            && token.eq_ignore_ascii_case("allem"))
    {
        relevant_pronoun = false;
    }
    relevant_pronoun
}

/// `AgreementRule.couldBeRelativeOrDependentClause`.
fn could_be_relative_or_dependent_clause(tokens: &[AnalyzedTokenReadings], pos: usize) -> bool {
    if pos >= 1 {
        let comma = tokens[pos - 1].surface() == ",";
        let rel_pronoun = comma && (tokens[pos].has_lemma("der") || tokens[pos].has_lemma("welch"));
        if rel_pronoun && pos + 3 < tokens.len() {
            return true;
        }
    }
    if pos >= 2 {
        let comma = tokens[pos - 2].surface() == ",";
        if comma {
            let prep = tokens[pos - 1].has_pos_tag_starting_with("PRP:");
            let rel_pronoun = tokens[pos].has_lemma("der") || tokens[pos].has_lemma("welch");
            return prep && rel_pronoun
                || (tokens[pos - 1].has_pos_tag("KON:UNT")
                    && (tokens[pos].has_lemma("jen")
                        || tokens[pos].has_lemma("dies")
                        || tokens[pos].has_lemma("ebendies")));
        }
    }
    false
}

/// `AgreementRule.retainCommonCategories` (3 tokens).
fn retain_common_categories3(
    token1: &AnalyzedTokenReadings,
    token2: &AnalyzedTokenReadings,
    token3: &AnalyzedTokenReadings,
) -> Vec<String> {
    let skip_sol = !VIELE_WENIGE_LOWERCASE.contains(&token1.surface().to_lowercase().as_str());
    let mut set1 = german_helper::get_agreement_categories(token1, &[], skip_sol);
    let set2 = german_helper::get_agreement_categories(token2, &[], skip_sol);
    let set3 = german_helper::get_agreement_categories(token3, &[], true);
    set1.retain(|c| set2.contains(c));
    set1.retain(|c| set3.contains(c));
    set1
}

/// `AgreementRule.retainCommonCategories` (4 tokens).
fn retain_common_categories4(
    token1: &AnalyzedTokenReadings,
    token2: &AnalyzedTokenReadings,
    token3: &AnalyzedTokenReadings,
    token4: &AnalyzedTokenReadings,
) -> Vec<String> {
    let skip_sol = !VIELE_WENIGE_LOWERCASE.contains(&token1.surface().to_lowercase().as_str());
    let mut set1 = german_helper::get_agreement_categories(token1, &[], skip_sol);
    let set2 = german_helper::get_agreement_categories(token2, &[], skip_sol);
    let set3 = german_helper::get_agreement_categories(token3, &[], skip_sol);
    let set4 = german_helper::get_agreement_categories(token4, &[], true);
    set1.retain(|c| set2.contains(c));
    set1.retain(|c| set3.contains(c));
    set1.retain(|c| set4.contains(c));
    set1
}

/// `AgreementRule.isNonPredicativeAdjective`.
fn is_non_predicative_adjective(token: &AnalyzedTokenReadings) -> bool {
    token.readings.iter().any(|r| {
        r.pos_tag
            .as_deref()
            .is_some_and(|t| t.starts_with("ADJ") && !t.contains("PRD"))
    })
}

/// `AgreementRule.isParticiple`.
fn is_participle(token: &AnalyzedTokenReadings) -> bool {
    util::has_partial_pos_tag(token, "PA1") || util::has_partial_pos_tag(token, "PA2")
}

/// `AnalyzedTokenReadings.isTagged` (SENT_END/PARA_END/untagged don't count).
fn is_tagged(token: &AnalyzedTokenReadings) -> bool {
    token.readings.iter().any(|r| {
        r.pos_tag
            .as_deref()
            .is_some_and(|t| t != "SENT_END" && t != "PARA_END")
    })
}

/// `StringTools.startsWithUppercase`.
fn starts_with_uppercase(text: &str) -> bool {
    text.chars().next().is_some_and(char::is_uppercase)
}

#[allow(clippy::too_many_arguments)]
fn match_from_parts(
    start: usize,
    end: usize,
    sentence_offset: usize,
    message: &str,
    short_message: &str,
    suggestions: Vec<String>,
) -> Match {
    Match::new(
        AGREEMENT_ID,
        Option::<String>::None,
        message,
        if short_message.is_empty() {
            None
        } else {
            Some(short_message.to_string())
        },
        TextRange::new(sentence_offset + start, sentence_offset + end),
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
    .with_metadata(
        AGREEMENT_DESCRIPTION,
        "uncategorized",
        AgreementRule::estimate_context_for_sure_match(),
    )
}

/// `AgreementRule2`'s matches carry its own id/description.
fn with_suggestions(mut m: Match, suggestions: Vec<String>) -> Match {
    m.rule_id = AGREEMENT2_ID.to_string();
    m.description = AGREEMENT2_DESCRIPTION.to_string();
    m.context_for_sure_match = AgreementRule2::estimate_context_for_sure_match();
    m.suggestions = suggestions
        .into_iter()
        .map(|value| Suggestion {
            value,
            short_description: None,
        })
        .collect();
    m
}

/// Minimal tokenize + tag (no disambiguation/chunker) for the nested
/// `lt.check(phrase)` in the compound branch.
fn analyze_plain_sentence(tagger: &lt_tagger::GermanTagger, text: &str) -> AnalyzedSentence {
    let tokenizer = lt_tokenize::GermanWordTokenizer::new();
    let raw_tokens = tokenizer.tokenize(text);
    let tagged = tagger.tag(&raw_tokens, true);
    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(tagged.len() + 1);
    tokens.push(AnalyzedTokenReadings {
        readings: vec![AnalyzedToken::new("", None, Some("SENT_START".to_string()))],
        chunk_tags: Vec::new(),
        whitespace_before: false,
        start_pos: 0,
        raw_byte_len: 0,
        is_whitespace: false,
        is_sentence_start: true,
        is_sentence_end: false,
        is_paragraph_end: false,
        is_tagged: true,
        is_immunized: false,
        is_ignore_spelling: false,
        has_typographic_apostrophe: false,
        is_pos_tag_unknown: false,
    });
    let mut byte_pos = 0usize;
    let mut prev_was_whitespace = false;
    let mut last_non_ws_idx: Option<usize> = None;
    for (raw, mut reading) in raw_tokens.iter().zip(tagged) {
        let is_whitespace = lt_core::is_whitespace(raw);
        reading.whitespace_before = prev_was_whitespace;
        reading.start_pos = byte_pos;
        reading.raw_byte_len = raw.len();
        reading.is_whitespace = is_whitespace;
        reading.is_tagged = reading.readings.iter().any(|r| r.pos_tag.is_some());
        tokens.push(reading);
        if !is_whitespace {
            last_non_ws_idx = Some(tokens.len() - 1);
        }
        byte_pos += raw.len();
        prev_was_whitespace = is_whitespace;
    }
    if let Some(idx) = last_non_ws_idx {
        let tr = &mut tokens[idx];
        if !tr
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref() == Some("SENT_END"))
        {
            let surface = tr
                .readings
                .first()
                .map(|r| r.token.clone())
                .unwrap_or_default();
            let lemma = tr.readings.first().and_then(|r| r.stem.clone());
            tr.add_reading(AnalyzedToken::new(
                surface,
                lemma,
                Some("SENT_END".to_string()),
            ));
        }
        tr.is_sentence_end = true;
    }
    AnalyzedSentence {
        text: text.to_string(),
        offset: 0,
        tokens,
        pre_disambig_tokens: Vec::new(),
        pre_disambig_detached: Vec::new(),
    }
}

static MIT_MIR_ETC: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"^(?:mit (mir|dir|ihm|ihr|ihnen|uns|euch))$").unwrap()
});
static OHNE_MICH_ETC: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"^(?:ohne (mich|dich|ihn|sie|uns|euch))$").unwrap()
});
static ZUGESCHRIEBENEN_GENANNTEN: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^(?:zugeschriebenen?|genannten?)$").unwrap());
static VIEL_WEIT: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^(?:viel|weit)$").unwrap());
static WENIGER_EHER: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^(?:weniger|eher)$").unwrap());
static HERR_FRAU: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^(?:Herr|Frau)$").unwrap());

/// `AgreementRule.getCategoriesCausingError` (exposed for tests).
#[cfg(test)]
pub(crate) fn categories_causing_error(
    token1: &AnalyzedTokenReadings,
    token2: &AnalyzedTokenReadings,
) -> Vec<String> {
    let mut categories = Vec::new();
    for category in [
        german_helper::GrammarCategory::Kasus,
        german_helper::GrammarCategory::Genus,
        german_helper::GrammarCategory::Numerus,
    ] {
        let omit = [category];
        let mut set1 = german_helper::get_agreement_categories(token1, &omit, true);
        let set2 = german_helper::get_agreement_categories(token2, &omit, true);
        set1.retain(|c| set2.contains(c));
        if !set1.is_empty() {
            categories.push(category.display_name().to_string());
        }
    }
    categories
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antipattern_lists_compile() {
        assert!(!ALL_ANTIPATTERNS.is_empty());
        assert!(!AGREEMENT2_ANTIPATTERNS.is_empty());
    }

    #[test]
    fn categories_causing_error_matches_java() {
        let det_mas_sin = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            "der",
            Some("der".into()),
            Some("ART:DEF:NOM:SIN:MAS".into()),
        )]);
        let _det_fem_sin = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            "die",
            Some("der".into()),
            Some("ART:DEF:NOM:SIN:FEM".into()),
        )]);
        let det_fem_plu = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            "die",
            Some("der".into()),
            Some("ART:DEF:NOM:PLU:FEM".into()),
        )]);
        let sub_neu_sin = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            "Haus",
            Some("Haus".into()),
            Some("SUB:NOM:SIN:NEU".into()),
        )]);
        let sub_gen_fem_plu = AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            "Frauen",
            Some("Frau".into()),
            Some("SUB:GEN:PLU:FEM".into()),
        )]);
        let res1 = categories_causing_error(&det_fem_plu, &sub_gen_fem_plu);
        assert_eq!(res1.len(), 1);
        assert!(res1[0].contains("Kasus"));
        let res2 = categories_causing_error(&det_mas_sin, &sub_neu_sin);
        assert_eq!(res2.len(), 1);
        assert!(res2[0].contains("Genus"));
    }
}
