//! `SubjectVerbAgreementRule` (`DE_SUBJECT_VERB_AGREEMENT`, checklist item 24,
//! default on): "ist/sind", "war/waren" agreement with the preceding noun
//! phrase using the German chunker's NPS/NPP/PP tags.
//!
//! Java's `containsOnlyInfinitivesToTheLeft` needs the German tagger's
//! `lookup`, so the rule carries an `Arc<GermanTagger>`.
//! `getUrl` (dict.leo.org) is a tools-layer field the engine `Match` does not
//! carry, like the other ported rules.

use std::sync::{Arc, LazyLock};

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};
use lt_pattern::matcher as pm;
use lt_pattern::PatternToken;

use super::util;

pub const RULE_ID: &str = "DE_SUBJECT_VERB_AGREEMENT";
const DESCRIPTION: &str = "Kongruenz von Subjekt und Prädikat (unvollständig)";
const CATEGORY_ID: &str = "GRAMMAR";
const CATEGORY_NAME: &str = "Grammatik";

static ANTI_PATTERNS: LazyLock<Vec<Arc<pm::CompiledPattern>>> = LazyLock::new(compile_antipatterns);

/// `SubjectVerbAgreementRule.PAIRS`.
const PAIRS: [(&str, &str); 2] = [("ist", "sind"), ("war", "waren")];
const CURRENCIES: [&str; 3] = ["Dollar", "Euro", "Yen"];
const QUESTION_PRONOUNS: [&str; 1] = ["wie"];

pub struct SubjectVerbAgreementRule {
    tagger: Arc<lt_tagger::GermanTagger>,
}

impl SubjectVerbAgreementRule {
    pub fn new(tagger: Arc<lt_tagger::GermanTagger>) -> Self {
        Self { tagger }
    }

    /// `estimateContextForSureMatch`.
    pub fn estimate_context_for_sure_match() -> i32 {
        antipattern_defs()
            .iter()
            .map(|p| p.len() as i32)
            .max()
            .unwrap_or(0)
    }

    /// `SubjectVerbAgreementRule.match`.
    pub fn match_sentence(
        &self,
        sentence: &AnalyzedSentence,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let immunized = util::immunize_sentence(sentence, &ANTI_PATTERNS);
        let tokens = immunized.tokens_without_whitespace();
        for i in 1..tokens.len() {
            if tokens[i].is_immunized {
                continue;
            }
            let token_str = tokens[i].surface();
            if let Some(m) = self.get_singular_match_or_null(&tokens, i, token_str) {
                rule_matches.push(self.to_match(m, sentence_offset, tokens[i]));
            }
            if let Some(m) = self.get_plural_match_or_null(&tokens, i, token_str) {
                rule_matches.push(self.to_match(m, sentence_offset, tokens[i]));
            }
        }
        rule_matches
    }

    fn to_match(
        &self,
        replacement: String,
        sentence_offset: usize,
        token: &AnalyzedTokenReadings,
    ) -> Match {
        let message =
            format!("Bitte prüfen, ob hier <suggestion>{replacement}</suggestion> stehen sollte.");
        Match::new(
            RULE_ID,
            Option::<String>::None,
            message,
            Option::<String>::None,
            TextRange::new(
                sentence_offset + token.start_pos,
                sentence_offset + token.end_pos(),
            ),
            vec![Suggestion {
                value: replacement,
                short_description: None,
            }],
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(
            DESCRIPTION,
            "uncategorized",
            Self::estimate_context_for_sure_match(),
        )
    }

    /// `getSingularMatchOrNull`.
    fn get_singular_match_or_null(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        i: usize,
        token_str: &str,
    ) -> Option<String> {
        let singular = PAIRS.iter().find(|(s, _)| *s == token_str)?;
        let prev_token = tokens[i - 1];
        let next_token = tokens.get(i + 1).copied();
        let prev_chunk_tags = &prev_token.chunk_tags;
        let matches = prev_chunk_tags.iter().any(|t| t == "NPP")
            && !prev_chunk_tags.iter().any(|t| t == "PP")
            && prev_token.surface() != "Uhr"
            && !is_currency(prev_token)
            && next_token.is_none_or(|t| t.surface() != "es")
            && self.prev_chunk_is_nominative(tokens, i - 1)
            && !has_unknown_token_to_the_left(tokens, i)
            && !has_question_pronoun_to_the_left(tokens, i - 1)
            && !has_verb_to_the_left(tokens, i - 1)
            && !contains_regex_to_the_left(&WER_ALLEN_JEDEN_MANCHEN, tokens, i - 1)
            && !self.contains_only_infinitives_to_the_left(tokens, i - 1);
        if matches {
            Some(singular.1.to_string())
        } else {
            None
        }
    }

    /// `getPluralMatchOrNull`.
    fn get_plural_match_or_null(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        i: usize,
        token_str: &str,
    ) -> Option<String> {
        let pair = PAIRS.iter().find(|(_, p)| *p == token_str)?;
        let prev_token = tokens[i - 1];
        let next_token = tokens.get(i + 1).copied();
        let prev_chunk_tags = &prev_token.chunk_tags;
        let matches = prev_chunk_tags.iter().any(|t| t == "NPS")
            && next_token.is_none_or(|t| t.surface() != "Sie")
            && !prev_chunk_tags.iter().any(|t| t == "NPP")
            && !prev_chunk_tags.iter().any(|t| t == "PP")
            && !is_currency(prev_token)
            && self.prev_chunk_is_nominative(tokens, i - 1)
            && !has_unknown_token_to_the_left(tokens, i)
            && !has_unknown_token_to_the_right(tokens, i + 1)
            && !tokens
                .get(1)
                .is_some_and(|t| matches!(t.surface(), "Alle" | "Viele"))
            && !is_followed_by_nominative_plural(tokens, i + 1);
        if matches {
            Some(pair.0.to_string())
        } else {
            None
        }
    }

    /// `prevChunkIsNominative`.
    fn prev_chunk_is_nominative(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        start_pos: usize,
    ) -> bool {
        let mut i = start_pos + 1;
        while i > 1 {
            i -= 1;
            let chunk_tags = &tokens[i].chunk_tags;
            if chunk_tags.iter().any(|t| t == "NPS" || t == "NPP") {
                if util::has_partial_pos_tag(tokens[i], "NOM") {
                    return true;
                }
            } else {
                return false;
            }
        }
        false
    }

    /// `containsOnlyInfinitivesToTheLeft`.
    fn contains_only_infinitives_to_the_left(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        start_pos: usize,
    ) -> bool {
        let mut infinitives = 0;
        let mut i = start_pos + 1;
        while i > 1 {
            i -= 1;
            let token = tokens[i].surface();
            if util::has_partial_pos_tag(tokens[i], "SUB:") {
                let lookup = self.tagger.lookup(&token.to_lowercase());
                if lookup.is_some_and(|l| l.has_pos_tag_starting_with("VER:INF")) {
                    infinitives += 1;
                } else {
                    return false;
                }
            }
        }
        infinitives >= 2
    }
}

static WER_ALLEN_JEDEN_MANCHEN: LazyLock<regex::Regex> = LazyLock::new(|| {
    // `wer|(?i)alle[nr]?|(?i)jede[rs]?|(?i)manche[nrs]?` with Java's
    // mid-pattern `(?i)` scoped to the alternatives.
    util::anchored(r"wer|(?i:alle[nr]?)|(?i:jede[rs]?)|(?i:manche[nrs]?)")
});

/// `isCurrency`.
fn is_currency(token: &AnalyzedTokenReadings) -> bool {
    CURRENCIES.contains(&token.surface())
}

/// `hasUnknownTokenToTheLeft` / `hasUnknownTokenToTheRight`.
fn has_unknown_token_at(
    tokens: &[&AnalyzedTokenReadings],
    start_pos: usize,
    end_pos: usize,
) -> bool {
    for token in tokens.iter().take(end_pos).skip(start_pos) {
        if token.readings.iter().any(|r| r.pos_tag.is_none()) {
            return true;
        }
    }
    false
}

fn has_unknown_token_to_the_left(tokens: &[&AnalyzedTokenReadings], start_pos: usize) -> bool {
    has_unknown_token_at(tokens, 0, start_pos)
}

fn has_unknown_token_to_the_right(tokens: &[&AnalyzedTokenReadings], start_pos: usize) -> bool {
    if tokens.is_empty() {
        return false;
    }
    has_unknown_token_at(tokens, start_pos, tokens.len() - 1)
}

/// `hasQuestionPronounToTheLeft`.
fn has_question_pronoun_to_the_left(tokens: &[&AnalyzedTokenReadings], start_pos: usize) -> bool {
    let mut i = start_pos + 1;
    while i > 1 {
        i -= 1;
        if QUESTION_PRONOUNS.contains(&tokens[i].surface().to_lowercase().as_str()) {
            return true;
        }
    }
    false
}

/// `hasVerbToTheLeft`.
fn has_verb_to_the_left(tokens: &[&AnalyzedTokenReadings], start_pos: usize) -> bool {
    let re = util::anchored("VER:[1-3]:.+");
    let mut i = start_pos + 1;
    while i > 1 {
        i -= 1;
        if tokens[i].has_pos_tag_matching(&re) {
            return true;
        }
    }
    false
}

/// `containsRegexToTheLeft`.
fn contains_regex_to_the_left(
    re: &regex::Regex,
    tokens: &[&AnalyzedTokenReadings],
    start_pos: usize,
) -> bool {
    let mut i = start_pos + 1;
    while i > 1 {
        i -= 1;
        if re.is_match(tokens[i].surface()) {
            return true;
        }
    }
    false
}

/// `isFollowedByNominativePlural`.
fn is_followed_by_nominative_plural(tokens: &[&AnalyzedTokenReadings], start_pos: usize) -> bool {
    for token in tokens.iter().skip(start_pos) {
        if (util::has_partial_pos_tag(token, "SUB") || util::has_partial_pos_tag(token, "PRO"))
            && (util::has_partial_pos_tag(token, "NOM:PLU")
                || token.chunk_tags.iter().any(|t| t == "NPP"))
        {
            return true;
        }
    }
    false
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

fn cs_regex(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        regexp: true,
        case_sensitive: true,
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

fn min0(mut t: PatternToken) -> PatternToken {
    t.min = Some(0);
    t
}

fn skip(mut t: PatternToken, value: i32) -> PatternToken {
    t.skip = Some(value);
    t
}

/// `SubjectVerbAgreementRule.ANTI_PATTERNS` (order matters).
fn antipattern_defs() -> Vec<Vec<PatternToken>> {
    vec![
        vec![
            pos("ZAL"),
            pos_regex("SUB:DAT:PLU:.*"),
            cs_regex("war|ist"),
            pos_regex("NEG|PA2:.+"),
        ],
        vec![
            token("Prozent"),
            token("der"),
            pos_regex("SUB:.*:PLU:.*"),
            cs_regex("sind|waren"),
        ],
        vec![
            token("meisten"),
            token("der"),
            pos_regex("SUB:.*:PLU:.*"),
            cs_regex("sind|waren"),
        ],
        vec![
            pos_regex("SUB:.*:PLU:.*"),
            skip(token("nicht"), 1),
            pos_regex("PRO:.*"),
            cs_regex("Ding"),
            cs_regex("sind|waren"),
        ],
        vec![
            token("Teil"),
            token("der"),
            token("Lösung"),
            cs_regex("sind|waren"),
        ],
        vec![
            pos_regex("SUB:NOM:PLU:.*"),
            token("zu"),
            pos_regex("SUB:.*"),
            token_regex("sind|waren"),
        ],
        vec![
            pos_regex("SUB:.*:PLU:.*"),
            token_regex("keine|wenig|kaum|viel"),
            pos_regex("SUB:.*:SIN:.*"),
            token("sind"),
        ],
        vec![token("Zehn"), token("Gebote"), token("sind")],
        vec![
            token("all"),
            token_regex("d(ies)?en"),
            pos_regex("SUB:.*PLU.*"),
            token("ist"),
            pos_regex("ART:.*"),
            pos_regex("SUB:.*SIN.*"),
        ],
        vec![
            pos("SENT_START"),
            min0(token("Solchen")),
            pos_regex("SUB:.*PLU.*"),
            token("ist"),
            pos_regex("ART:.*"),
            pos_regex("SUB:.*SIN.*"),
        ],
        vec![
            token_regex("Reste|Überreste"),
            token_regex("eines|des"),
            pos_regex("ADV:.*"),
            pos_regex("ADJ:.*"),
            pos_regex("SUB:.*SIN.*"),
            token_regex("sind"),
        ],
        vec![
            pos_regex("ADJ:.*"),
            token_regex("und|sowie"),
            pos_regex("ADV:.*"),
            pos_regex("PA2:.*"),
            pos_regex("SUB:.*PLU.*"),
            token_regex("sind"),
        ],
        vec![
            token_regex("Gründer(in)?|Gesellschafter(in)?|Leiter(in)?|Geschäftsführer(in)?|Chef(in)?"),
            token_regex("und|sowie|&"),
            skip(
                token_regex("Gründer(in)?|Gesellschafter(in)?|Leiter(in)?|Geschäftsführer(in)?|Chef(in)?"),
                4,
            ),
            token_regex("ist"),
        ],
        vec![token_regex("ist|war"), token("gemeinsam")],
        vec![
            pos("SENT_START"),
            pos("ZAL"),
            token_regex("Minuten|Stunden|Tage|Monate|Jahre|Jahrzehnte"),
            pos_regex("VER:3:SIN:.*"),
        ],
        vec![
            pos("SENT_START"),
            token_regex("einige|viele|wenige|mehrere"),
            token_regex("Minuten|Stunden|Tage|Monate|Jahre|Jahrzehnte"),
            pos_regex("VER:3:SIN:.*"),
        ],
        vec![
            pos("SENT_START"),
            pos_regex("ADV:MOD|ADJ:PRD:GRU"),
            pos("ZAL"),
            token_regex("Minuten|Stunden|Tage|Monate|Jahre|Jahrzehnte"),
            pos_regex("VER:3:SIN:.*"),
        ],
        vec![
            pos("SENT_START"),
            skip(pos("PRP:CAU:GEN"), 4),
            skip(cs_token("und"), 4),
            token_regex("ist|war"),
        ],
        vec![
            pos_regex("SENT_START|KON:UNT"),
            pos_regex("(EIG|SUB):.*"),
            skip(cs_token("und"), 3),
            token_regex("sind|waren"),
        ],
        vec![
            pos("KON:UNT"),
            skip(token("sie"), 3),
            token_regex("sind|waren"),
        ],
        vec![
            pos("SENT_START"),
            skip(pos_regex("PRP:.+"), 4),
            token_regex("ist|war"),
            token_regex("d(as|er)|eine?"),
        ],
        vec![
            pos_regex("SUB:NOM:PLU:.+"),
            cs_token("vor"),
            cs_token("Ort"),
            token_regex("sind|waren"),
        ],
        vec![
            token("zu"),
            cs_regex("Fuß|Hause|Bein|Besuch"),
            token_regex("sind|waren"),
        ],
        vec![
            pos("SENT_START"),
            pos("SUB:DAT:PLU:NOG"),
            token_regex("ist|war"),
            pos_regex(".+:NOM:.+"),
        ],
        vec![
            pos_regex("SUB:.+"),
            min0(pos("PKT")),
            token("sowie"),
            pos_regex("ART.*"),
            min0(pos_regex("(ADJ|PA[12]).*")),
            pos_regex("SUB:.+"),
            token_regex("sind|waren"),
        ],
        vec![
            token_regex("das"),
            pos_regex("(ADJ|PA[12]).*NEU.*"),
            pos_regex("SUB:.*NEU.*"),
            token_regex("und"),
            pos_regex("SUB:.*NEU.*"),
            token_regex("ist|war"),
        ],
        vec![
            token_regex("der"),
            pos_regex("(ADJ|PA[12]).*MAS.*"),
            pos_regex("SUB:.*MAS.*"),
            token_regex("und"),
            pos_regex("SUB:.*MAS.*"),
            token_regex("ist|war"),
        ],
        vec![
            token_regex("die"),
            pos_regex("(ADJ|PA[12]).*FEM.*"),
            pos_regex("SUB:.*FEM.*"),
            token_regex("und"),
            pos_regex("SUB:.*FEM.*"),
            token_regex("ist|war"),
        ],
        vec![
            token_regex("(irgend)?einer?|meisten|viele|einige|Betreiber|(Mit)?Gründer|Inhaber"),
            skip(token_regex("der|dieser"), 4),
            token_regex("ist|war"),
        ],
        vec![
            skip(token("dank"), -1),
            token_regex("ist|war"),
            pos_regex("EIG.*|SUB.*SIN.*"),
        ],
        vec![token("Start"), token("und"), token("Ziel"), token_regex("ist|war")],
        vec![
            pos_regex("SUB.*SIN.*"),
            token("und"),
            pos_regex("SUB.*SIN.*"),
            token_regex("ist|war"),
        ],
        vec![token("Obst"), token("und"), token("Gemüse")],
        vec![token("Sport"), token("und"), token("Spiel")],
        vec![
            token("das"),
            pos_regex("(ADJ|PA[12]).*NEU.*"),
            pos_regex("SUB.*NEU.*"),
            token("und"),
            pos_regex("SUB.*NEU.*"),
            token_regex("der|dieser"),
            pos_regex("SUB.*"),
            token_regex("ist|war"),
        ],
        vec![
            token("die"),
            pos_regex("(ADJ|PA[12]).*PLU.*"),
            pos_regex("SUB.*PLU.*"),
            token("und"),
            pos_regex("SUB.*PLU.*"),
            token_regex("der|dieser"),
            pos_regex("SUB.*"),
            token_regex("sind|waren"),
        ],
        vec![
            pos_regex("(ADJ|PA[12]|ART).*SIN.*"),
            pos_regex("SUB.*SIN.*"),
            pos_regex("PRP.*"),
            pos_regex("EIG.*GEN.*"),
            pos_regex("SUB.*"),
            token_regex("ist|war"),
        ],
        vec![
            pos_regex("(ADJ|PA[12]|ART).*PLU.*"),
            pos_regex("SUB.*PLU.*"),
            pos_regex("PRP.*"),
            pos_regex("EIG.*GEN.*"),
            pos_regex("SUB.*"),
            token_regex("sind|waren"),
        ],
        vec![
            pos_regex("SUB.*PLU.*"),
            token("wie"),
            token("auch"),
            token_regex(".+"),
            token_regex("sind|waren"),
        ],
        vec![
            token_regex("ist|war|wäre?"),
            pos_regex("EIG:NOM:SIN.*|PRO:PER:NOM:SIN.*"),
            pos_regex("ADJ:PRD:GRU"),
        ],
        vec![
            token_regex("bist|w[äa]rst"),
            token_regex("du"),
            pos_regex("ADJ:PRD:GRU"),
        ],
        vec![
            token_regex("sind|w[äa]ren|seid"),
            pos_regex("PRO:PER:NOM:PLU.*"),
            pos_regex("ADJ:PRD:GRU"),
        ],
        vec![
            pos("SUB:NOM:SIN:MAS"),
            pos_regex("ART:...:GEN:PLU:MAS"),
            pos_regex("SUB:GEN:PLU:.+"),
            pos("KON:NEB"),
            pos_regex("SUB:GEN:PLU:.+"),
            token_regex("ist|w[äa]r"),
        ],
        vec![
            skip(cs_token("Laut"), 4),
            skip(cs_token("und"), 5),
            token_regex("ist|war"),
            token_regex("d(er|ie|as)"),
            pos_regex("SUB:NOM:SIN:.+"),
        ],
        vec![
            token_regex("ist|war"),
            token_regex("d(er|ie|as)"),
            min0(pos_regex("(ADJ|PA[12]).*")),
            pos_regex("SUB:NOM:SIN:.+"),
            pos_regex("(ART|PRO:POS).*SIN.*"),
            min0(pos_regex("(ADJ|PA[12]).*")),
            pos_regex("SUB:NOM:SIN:.+"),
        ],
        vec![
            token_regex("die"),
            min0(pos_regex("(ADJ|PA[12]).*")),
            token_regex("Mehrheit"),
            token_regex("der|dieser|aller|unse?rer|[dsm]einer|euer|eurer|ihrer"),
            min0(pos_regex("(ADJ|PA[12]).*")),
            pos_regex("SUB.*NOM.*PLU.*"),
            token_regex("sind|w[äa]ren"),
        ],
        vec![
            token_regex("weil|da|denn|dass"),
            token_regex("die|diese|solche|alle|viele|beide|einige|[mkds]eine|eure|unse?re|ihre"),
            min0(pos_regex("(ADJ|PA[12]).*")),
            pos_regex("SUB.*NOM.*PLU.*"),
            token_regex("ein"),
            min0(pos_regex("(ADJ|PA[12]).*")),
            pos_regex("SUB.*NOM.*SIN.*"),
            token_regex("sind|w[äa]ren"),
        ],
        vec![
            token_regex("alle|die(se)?|einige|keine|viele|solche"),
            min0(pos_regex("(ADJ|PA[12]).*")),
            pos_regex("SUB.*NOM.*PLU.*"),
            token_regex("der|unse?rer|euer|eurer|[dsm]einer|dieser|solcher|aller|einiger|vieler|ihrer|beider"),
            min0(pos_regex("(ADJ|PA[12]).*")),
            pos_regex("SUB.*GEN.*PLU.*"),
            pos_regex("VER.*PLU.*"),
        ],
        vec![
            token_regex("wir|sie|die|alle|diese|einige|manche|viele|sonstige"),
            pos_regex("ART.*|PRO:(POS|DEM|IND).*"),
            min0(pos_regex("(ADJ|PA[12]).*")),
            pos_regex("SUB.*SIN.*"),
            pos_regex("VER.*PLU.*"),
        ],
        vec![
            token_regex("wir|sie|die|alle|diese|einige|manche|viele|sonstige"),
            pos_regex("ART.*|PRO:(POS|DEM|IND).*"),
            pos_regex("(ADJ|PA[12]).*|ADV.*"),
            pos_regex("(ADJ|PA[12]).*"),
            pos_regex("SUB.*SIN.*"),
            pos_regex("VER.*PLU.*"),
        ],
        vec![skip(token("sie"), -1), token_regex("sind|w[äa]ren")],
        vec![
            token_regex("weder"),
            token_regex("er|es|sie"),
            skip(token("noch"), -1),
            token_regex("sind|w[äa]ren"),
        ],
        vec![
            skip(pos_regex("SUB.*PLU.*"), 5),
            token_regex("sind|w[äa]ren"),
        ],
        vec![
            pos_regex("SUB.*INF|SUB.*PLU.*"),
            token_regex("sind|w[äa]ren"),
        ],
        vec![
            token_regex("Teile"),
            token_regex("de[rs]|diese[sr]|[msd]?eine[rs]"),
            skip(pos_regex("SUB.*|EIG.*|UNKNOWN"), -1),
            token_regex("sind|w[äa]ren"),
        ],
        vec![
            token_regex("viele|alle"),
            PatternToken {
                postag: Some("SUB.*ADJ".to_string()),
                postag_regexp: true,
                text: Some(".+e".to_string()),
                regexp: true,
                in_marker: true,
                ..Default::default()
            },
            token_regex("sind|w[äa]ren"),
        ],
        vec![
            token_regex("Sinn"),
            token_regex("und"),
            skip(token_regex("Zweck"), -1),
            token_regex("ist|war"),
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
        assert_eq!(antipattern_defs().len(), 57);
    }

    #[test]
    fn context_estimate() {
        assert_eq!(
            SubjectVerbAgreementRule::estimate_context_for_sure_match(),
            8
        );
    }
}
