//! Port of `org.languagetool.rules.uk.TokenAgreementVerbNounRule`
//! (`UK_VERB_NOUN_INFLECTION_AGREEMENT`) and
//! `TokenAgreementVerbNounExceptionHelper`.
#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Suggestion, TextRange};
use lt_tagger::uk_helpers::{self, Dir};
use lt_tagger::UkrainianSynthesizer;

use crate::uk::gov::CaseGovernment;
use crate::uk::inflection;
use crate::uk::search_helper::{Condition, Match as SearchMatch};
use crate::uk::verb_inflection::{self, VerbInflection};

pub const RULE_ID: &str = "UK_VERB_NOUN_INFLECTION_AGREEMENT";
const DESCRIPTION: &str = "Узгодження дієслова з іменником";
const SHORT: &str = "Узгодження дієслова з іменником";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Різне";

#[derive(PartialEq, Clone, Copy)]
pub enum Type {
    None,
    Exception,
    Skip,
}

pub struct RuleException {
    pub r#type: Type,
    pub skip: usize,
}

impl RuleException {
    fn none() -> Self {
        Self {
            r#type: Type::None,
            skip: 0,
        }
    }
    fn exception() -> Self {
        Self {
            r#type: Type::Exception,
            skip: 0,
        }
    }
    fn skip(skip: usize) -> Self {
        Self {
            r#type: Type::Skip,
            skip,
        }
    }
}

static VCHYTY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*вч[аи]ти(ся)?$").unwrap());
static ADV_PREDICT_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:adv|noninfl:predic).*$").unwrap());
static MODALS_ADJ: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:змушений|вимушений|повинний|здатний|готовий|ладний|радий)$").unwrap()
});
static V_ROD_DRIVER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:не|(?:на)?с[кт]ільки|(?:най)?більше|(?:най)?менше|(?:не|за)?багато|(?:не|чи|за)?мало|трохи|годі|неможливо|а?ніж|вдосталь|купу)$").unwrap()
});
static PARTS_CANT_SKIP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:і|й|та|чи|або|але|як|де|куди|наче|ніби|хоч|навіщо|немов|вдвічі|дедалі|щойно|наскільки)$").unwrap()
});
static VERB_SN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^.*:[sn](?::.*|$)$").unwrap());
static VERB_F: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^verb.*:f(?::.*|$)$").unwrap());
static V_FUTR_PAST_S3_N: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^verb.*?(?:futr|past):(?:s:3.*|n(?:$|:.+))$").unwrap());
static V_IMPR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^verb.*impr.*$").unwrap());

pub struct State {
    verb_pos: usize,
    pub noun_pos: usize,
    verb_readings: Vec<AnalyzedToken>,
    verb_idx: Option<usize>,
    noun_adj_naz_inflections: Vec<VerbInflection>,
    cases: HashSet<String>,
    noun_adj_indir: Vec<AnalyzedToken>,
}

impl State {
    fn new() -> Self {
        Self {
            verb_pos: 0,
            noun_pos: 0,
            verb_readings: Vec::new(),
            verb_idx: None,
            noun_adj_naz_inflections: Vec::new(),
            cases: HashSet::new(),
            noun_adj_indir: Vec::new(),
        }
    }
}

pub struct TokenAgreementVerbNounRule {
    synth: std::sync::Arc<UkrainianSynthesizer>,
    gov: std::sync::Arc<CaseGovernment>,
    masc_fem: HashSet<String>,
}

impl TokenAgreementVerbNounRule {
    pub fn new(
        synth: std::sync::Arc<UkrainianSynthesizer>,
        gov: std::sync::Arc<CaseGovernment>,
        masc_fem: HashSet<String>,
    ) -> Self {
        Self {
            synth,
            gov,
            masc_fem,
        }
    }

    pub fn rule_id(&self) -> &str {
        RULE_ID
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let view: Vec<&AnalyzedTokenReadings> =
            tokens.iter().filter(|t| !t.is_whitespace).collect();
        let n = view.len();
        let mut out = Vec::new();
        let mut state: Option<State> = None;
        let mut i = 1usize;
        while i < n {
            let tr = view[i];
            let pos0 = tr.readings.first().and_then(|r| r.pos_tag.clone());
            if pos0.is_none() {
                state = None;
                i += 1;
                continue;
            }
            if state.is_none() && i == n - 1 {
                i += 1;
                continue;
            }
            if uk_helpers::has_reading_pos_tag(
                &tr.readings,
                &Regex::new(r"^(?:verb|advp).*$").unwrap(),
            ) {
                let ex = self.is_exception_verb(&view, i, &state);
                if ex.r#type != Type::None {
                    if ex.r#type == Type::Exception {
                        state = None;
                    }
                    i += ex.skip;
                    i += 1;
                    continue;
                }
                let new_state = self.get_verb_state(&view, i);
                if new_state.is_none() || new_state.as_ref().unwrap().verb_pos == i {
                    state = new_state;
                    i += 1;
                    continue;
                }
                state = new_state;
            }
            let Some(mut state) = state.take() else {
                i += 1;
                continue;
            };
            if i >= n - 1 {
                break;
            }

            let hard = self.is_exception_hard_adj_noun(&view, i, &state);
            if let Some(skip) = hard {
                i += skip;
                i += 1;
                continue;
            }
            if let Some(skip) = self.is_exception_skip(&view, i) {
                i += skip;
                i += 1;
                continue;
            }

            let mut noun_adj_naz: Vec<AnalyzedToken> = Vec::new();
            let mut restart = false;
            for at in &tr.readings {
                let Some(tag) = at.pos_tag.as_deref() else {
                    continue;
                };
                if tag.ends_with("_END") {
                    continue;
                }
                if tag.starts_with('<') {
                    restart = true;
                    break;
                }
                if tag.starts_with("noun") || tag.starts_with("adj") || tag.starts_with("numr") {
                    if tag.contains("v_naz") {
                        noun_adj_naz.push(at.clone());
                    } else {
                        state.noun_adj_indir.push(at.clone());
                    }
                    state.noun_pos = i;
                } else {
                    restart = true;
                    break;
                }
            }
            if restart || (noun_adj_naz.is_empty() && state.noun_adj_indir.is_empty()) {
                i += 1;
                continue;
            }

            let mut pass = false;
            let mut verb_inflections: Vec<VerbInflection> = Vec::new();
            if !noun_adj_naz.is_empty() {
                let mut naz = verb_inflection::get_noun_inflections_v(&noun_adj_naz);
                naz.extend(verb_inflection::get_adj_inflections_v(&noun_adj_naz));
                state.noun_adj_naz_inflections = naz;
                verb_inflections = verb_inflection::get_verb_inflections(&state.verb_readings);
                pass = verb_inflections
                    .iter()
                    .any(|v| state.noun_adj_naz_inflections.contains(v));
            }
            if !pass && !state.noun_adj_indir.is_empty() {
                let verb_at = view[state.verb_idx.unwrap()];
                let mut cases =
                    self.gov
                        .get_case_governments_opt(&verb_at.readings, Some("verb"), None);
                if cases.is_empty()
                    && verb_at.surface().contains('-')
                    && uk_helpers::has_lemma_regex(
                        &verb_at.readings,
                        &Regex::new(r"^.+ти(ся)?-.+ти(ся)?$").unwrap(),
                    )
                {
                    let nodash: Vec<AnalyzedToken> = verb_at
                        .readings
                        .iter()
                        .filter(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with("verb")))
                        .map(|r| {
                            let lemma = Regex::new(r"(ти(ся)?)-.*")
                                .unwrap()
                                .replace(r.stem.as_deref().unwrap_or(""), "$1")
                                .into_owned();
                            AnalyzedToken::new(&r.token, Some(lemma), r.pos_tag.clone())
                        })
                        .collect();
                    cases = self
                        .gov
                        .get_case_governments_opt(&nodash, Some("verb"), None);
                }
                if state.verb_pos > 0
                    && view[state.verb_pos - 1].surface().to_lowercase() == "було"
                    && uk_helpers::has_reading_pos_tag(
                        &view[state.verb_pos].readings,
                        &Regex::new(r"^verb.*impers.*$").unwrap(),
                    )
                {
                    cases.insert("v_rod".to_string());
                }
                state.cases = cases;
                let token_lower = tr.surface().to_lowercase();
                if state.cases.contains("v_zna")
                    && Regex::new(r"^(?:грошей|грошенят|дров|товарів|пісень)$")
                        .unwrap()
                        .is_match(&token_lower)
                        .unwrap_or(false)
                {
                    i += 1;
                    continue;
                }
                if !state.cases.is_empty()
                    && uk_helpers::has_vidm_pos_tag(&state.cases, &state.noun_adj_indir)
                {
                    pass = true;
                }
            }

            if !pass {
                if i < n - 1
                    && uk_helpers::has_lemma_with_pattern(
                        &tr.readings,
                        &["він", "вона", "вони"],
                        &Regex::new(r"^noun:.*v_rod.*$").unwrap(),
                    )
                    && uk_helpers::has_reading_pos_tag(
                        &view[i + 1].readings,
                        &Regex::new(r"^(?:noun|adj).*$").unwrap(),
                    )
                {
                    state.noun_pos = 0;
                    i += 1;
                    continue;
                }
                if self.is_exception(&view, &state, &verb_inflections, &noun_adj_naz) {
                    i += 1;
                    continue;
                }
                if !noun_adj_naz.is_empty() || !state.noun_adj_indir.is_empty() {
                    let verb_at = view[state.verb_idx.unwrap()];
                    let cases =
                        self.gov
                            .get_case_governments_opt(&verb_at.readings, Some("verb"), None);
                    if !uk_helpers::has_vidm_pos_tag(&cases, &state.noun_adj_indir) {
                        let mut noun_adj_inflections =
                            inflection::get_noun_inflections(&state.noun_adj_indir, None);
                        noun_adj_inflections
                            .extend(inflection::get_adj_inflections(&state.noun_adj_indir));
                        noun_adj_inflections.extend(inflection::get_adj_inflections_start(
                            &state.noun_adj_indir,
                            "numr",
                        ));
                        if !noun_adj_naz.is_empty() {
                            let mut naz = inflection::get_noun_inflections(&noun_adj_naz, None);
                            naz.extend(inflection::get_adj_inflections(&noun_adj_naz));
                            naz.extend(inflection::get_adj_inflections_start(
                                &noun_adj_naz,
                                "numr",
                            ));
                            noun_adj_inflections.extend(naz);
                        }
                        let mut msg = format!(
                            "Не узгоджено дієслово з іменником: \"{}\" ({}) і \"{}\" ({})",
                            state
                                .verb_readings
                                .first()
                                .map(|r| r.token.clone())
                                .unwrap_or_default(),
                            format_inflections(&cases),
                            state
                                .noun_adj_indir
                                .first()
                                .map(|r| r.token.clone())
                                .unwrap_or_default(),
                            inflection::format_inflections(&noun_adj_inflections, false),
                        );
                        let mut verb_replace: Option<&str> = None;
                        let verb_lemma = state
                            .verb_readings
                            .first()
                            .and_then(|r| r.stem.clone())
                            .unwrap_or_default();
                        if verb_lemma == "сипіти" {
                            msg += ". Можливо ви мали на увазі слово «си́пати», а не «сипі́ти»?";
                            verb_replace = Some("сипати");
                        } else if verb_lemma == "сиплячи" {
                            msg += ". Можливо ви мали на увазі «сиплючи»?";
                            verb_replace = Some("сиплючи");
                        }
                        let mut suggestions = self.get_suggestions(&state.cases, tr);
                        if tr.surface() == "піку" && suggestions.contains(&"піка".to_string())
                        {
                            suggestions = vec!["піка".to_string()];
                        }
                        let mut inside = String::new();
                        for mid in &view[state.verb_pos + 1..state.noun_pos] {
                            inside.push(' ');
                            inside.push_str(mid.surface());
                        }
                        let suggestions = if let Some(vr) = verb_replace {
                            vec![format!("{vr}{inside} {}", tr.surface())]
                        } else {
                            suggestions
                                .into_iter()
                                .map(|s| format!("{}{inside} {s}", verb_at.surface()))
                                .collect()
                        };
                        out.push(
                            Match::new(
                                RULE_ID,
                                Option::<String>::None,
                                msg,
                                Some(SHORT.to_string()),
                                TextRange::new(
                                    sentence_offset + verb_at.start_pos,
                                    sentence_offset + tr.end_pos(),
                                ),
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
                                DESCRIPTION,
                                "misspelling",
                                0,
                            ),
                        );
                    }
                }
            }
            i += 1;
        }
        out
    }

    fn get_verb_state(&self, view: &[&AnalyzedTokenReadings], i: usize) -> Option<State> {
        let tr = view[i];
        let clean_lower = tr.surface().to_lowercase();
        if uk_helpers::has_reading_pos_tag(
            &tr.readings,
            &Regex::new(r"^.*(?:arch|bad|slang|alt).*$").unwrap(),
        ) {
            return None;
        }
        if Regex::new(r"^(?:значить|читай|бува|здавалось|здається|здалося)$")
            .unwrap()
            .is_match(&clean_lower)
            .unwrap_or(false)
        {
            return None;
        }
        let mut state: Option<State> = None;
        for at in &tr.readings {
            let Some(tag) = at.pos_tag.as_deref() else {
                continue;
            };
            if !Regex::new(r"^(?:verb|advp).*$")
                .unwrap()
                .is_match(tag)
                .unwrap_or(false)
                || tag.contains("abbr")
            {
                return None;
            }
            if state.is_none() {
                let mut s = State::new();
                s.verb_pos = i;
                s.verb_idx = Some(i);
                state = Some(s);
            }
            state.as_mut().unwrap().verb_readings.push(at.clone());
        }
        state
    }

    fn get_suggestions(&self, cases: &HashSet<String>, tr: &AnalyzedTokenReadings) -> Vec<String> {
        if cases.is_empty() {
            return Vec::new();
        }
        let mut sorted: Vec<&String> = cases.iter().collect();
        sorted.sort();
        let required = format!(
            ":({})",
            sorted
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("|")
        );
        let mut suggestions: Vec<String> = Vec::new();
        for at in &tr.readings {
            let Some(old) = at.pos_tag.as_deref() else {
                continue;
            };
            if !old.contains(":v_") {
                continue;
            }
            let pos_tag = Regex::new(r":v_[a-z]+")
                .unwrap()
                .replace(old, required.as_str())
                .into_owned();
            for s in self.synth.synthesize(at, &pos_tag, true) {
                if !suggestions.contains(&s) {
                    suggestions.push(s);
                }
            }
        }
        suggestions
    }

    #[allow(clippy::too_many_arguments)]
    fn is_exception(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        state: &State,
        verb_inflections: &[VerbInflection],
        noun_adj_naz: &[AnalyzedToken],
    ) -> bool {
        // filled in by is_exception_inner
        let _ = verb_inflections;
        is_exception_inner(self, tokens, state, noun_adj_naz)
    }

    fn is_exception_hard_adj_noun(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        i: usize,
        state: &State,
    ) -> Option<usize> {
        let clean_lower = tokens[i].surface().to_lowercase();
        if Regex::new(r"^(?:[0-9]{4}-.+|нікому|нічому|нічого|нікого|нічим|решту|ніщо)$")
            .unwrap()
            .is_match(&clean_lower)
            .unwrap_or(false)
        {
            return Some(1);
        }
        if uk_helpers::has_lemma(&tokens[i].readings, &["сам", "самий", "себе", "один"])
        {
            return Some(1);
        }
        if i < tokens.len() - 1
            && usize::from(uk_helpers::has_pos_tag_str2(tokens[i], "adj:m:v_rod.*")) > 0
            && Regex::new(r"^(?:роду|разу|типу|штибу|розміру)$")
                .unwrap()
                .is_match(tokens[i + 1].surface())
                .unwrap_or(false)
        {
            return Some(1);
        }
        if i < tokens.len() - 1
            && uk_helpers::has_pos_tag_str2(tokens[i], "(adj|numr):[mp]:v_oru.*")
            && Regex::new(r"^(?:чином|способом|робом|ходом|шляхом|коштом)$")
                .unwrap()
                .is_match(tokens[i + 1].surface())
                .unwrap_or(false)
        {
            return Some(1);
        }
        if i < tokens.len() - 1
            && uk_helpers::has_reading_pos_tag_start(&state.verb_readings, "advp")
            && tokens[i].surface().eq_ignore_ascii_case("тим")
            && tokens[i + 1].surface().eq_ignore_ascii_case("самим")
        {
            return Some(1);
        }
        if i < tokens.len() - 1
            && uk_helpers::has_pos_tag_str2(tokens[i], "adj:f:v_oru.*")
            && Regex::new(r"^мірою$")
                .unwrap()
                .is_match(tokens[i + 1].surface())
                .unwrap_or(false)
        {
            return Some(1);
        }
        if i < tokens.len() - 1
            && uk_helpers::has_pos_tag_str2(tokens[i], "adj:f:v_rod.*")
            && Regex::new(r"^(?:якості|свіжості)$")
                .unwrap()
                .is_match(tokens[i + 1].surface())
                .unwrap_or(false)
        {
            return Some(1);
        }
        if i < tokens.len() - 1 && tokens[i + 1].surface().to_lowercase() == "темпами" {
            return Some(1);
        }
        let m = |line: &str| SearchMatch::new().token_line(line).m_now(tokens, i);
        if m("не те щоб") == Some(i + 2)
            || m("не те що") == Some(i + 2)
            || m("не останньою чергою") == Some(i + 2)
        {
            return Some(3);
        }
        if m("не те, що") == Some(i + 3) {
            return Some(4);
        }
        if m("світ за очі") == Some(i + 2)
            || m("ні світ ні") == Some(i + 2)
            || m("куди очі") == Some(i + 1)
            || m("станом на") == Some(i + 1)
            || m("страх як") == Some(i + 1)
            || m("жах як") == Some(i + 1)
        {
            return Some(3);
        }
        if tokens[i - 1].surface() == "не"
            && Regex::new(r"^(?:указ|варіант|рідкість)$")
                .unwrap()
                .is_match(tokens[i].surface())
                .unwrap_or(false)
        {
            return Some(0);
        }
        None
    }

    fn is_exception_skip(&self, tokens: &[&AnalyzedTokenReadings], i: usize) -> Option<usize> {
        let clean_lower = tokens[i].surface().to_lowercase();
        if uk_helpers::has_pos_tag_all(
            &tokens[i].readings,
            &Regex::new(r"^(?:part|adv).*$").unwrap(),
        ) && !uk_helpers::adv_quant_pattern()
            .is_match(&clean_lower)
            .unwrap_or(false)
            && !PARTS_CANT_SKIP.is_match(&clean_lower).unwrap_or(false)
        {
            return Some(0);
        }
        if uk_helpers::has_reading_pos_tag(&tokens[i].readings, &Regex::new(r"^part.*$").unwrap())
            && uk_helpers::has_pos_tag_all(
                &tokens[i].readings,
                &Regex::new(r"^(?:part|conj|adv).*$").unwrap(),
            )
            && !PARTS_CANT_SKIP.is_match(&clean_lower).unwrap_or(false)
        {
            return Some(0);
        }
        None
    }

    fn is_exception_verb(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        i: usize,
        state: &Option<State>,
    ) -> RuleException {
        if uk_helpers::has_lemma(&tokens[i].readings, &["мусити"]) {
            return RuleException::exception();
        }
        let clean_lower = tokens[i].surface().to_lowercase();
        if clean_lower == "може" {
            return RuleException::exception();
        }
        if i > 1
            && (clean_lower == "є" || uk_helpers::has_lemma(&tokens[i].readings, &["могти"]))
            && tokens[i - 1].surface().eq_ignore_ascii_case("як")
        {
            return RuleException::exception();
        }
        if i < tokens.len() - 2
            && clean_lower == "будь"
            && tokens[i + 1].surface().eq_ignore_ascii_case("то")
        {
            return RuleException::exception();
        }
        if i > 1
            && i < tokens.len() - 1
            && tokens[i].surface().to_lowercase() == "спати"
            && uk_helpers::has_lemma_regex(
                &tokens[i - 1].readings,
                &Regex::new(r"^(?:по|в)?кла(?:сти|вши)$").unwrap(),
            )
        {
            return RuleException::skip(0);
        }
        if i > 1 && state.is_some() {
            let prev = tokens[i - 1];
            if Regex::new(r"^(?:був|було)$")
                .unwrap()
                .is_match(&clean_lower)
                .unwrap_or(false)
                && uk_helpers::has_pos_tag_str2(prev, "verb.*:past:m.*")
            {
                return RuleException::skip(0);
            }
            if Regex::new(r"^(?:були|було)$")
                .unwrap()
                .is_match(&clean_lower)
                .unwrap_or(false)
                && uk_helpers::has_pos_tag_str2(prev, "verb.*:past:p.*")
            {
                return RuleException::skip(0);
            }
            if clean_lower == "було" && uk_helpers::has_pos_tag_str2(prev, "verb.*:past:n.*") {
                return RuleException::skip(0);
            }
            if Regex::new(r"^(?:була|було)$")
                .unwrap()
                .is_match(&clean_lower)
                .unwrap_or(false)
                && uk_helpers::has_pos_tag_str2(prev, "verb.*:past:f.*")
            {
                return RuleException::skip(0);
            }
        }
        if i > 1
            && Regex::new(r"^(?:було|буде)$")
                .unwrap()
                .is_match(&clean_lower)
                .unwrap_or(false)
            && state.is_some()
            && uk_helpers::has_pos_tag_str2(tokens[i - 1], "verb.*(?:impers|predic).*")
        {
            return RuleException::skip(0);
        }
        RuleException::none()
    }
}

fn format_inflections(cases: &HashSet<String>) -> String {
    if cases.is_empty() {
        return "неперех.".to_string();
    }
    let mut sorted: Vec<&String> = cases.iter().collect();
    sorted.sort();
    format!(
        "вимагає: {}",
        sorted
            .iter()
            .map(|c| uk_helpers::case_name(c))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn has(tr: &AnalyzedTokenReadings, p: &str) -> bool {
    uk_helpers::has_pos_tag_str2(tr, p)
}
fn has_re(tr: &AnalyzedTokenReadings, re: &Regex) -> bool {
    uk_helpers::has_pos_tag_re(tr, re)
}
fn has_part(tr: &AnalyzedTokenReadings, p: &str) -> bool {
    uk_helpers::has_reading_pos_tag_part(&tr.readings, p)
}
fn has_part_readings(r: &[AnalyzedToken], p: &str) -> bool {
    uk_helpers::has_reading_pos_tag_part(r, p)
}
fn lemma(tr: &AnalyzedTokenReadings, l: &[&str]) -> bool {
    uk_helpers::has_lemma(&tr.readings, l)
}
fn lemma_re(tr: &AnalyzedTokenReadings, p: &str) -> bool {
    uk_helpers::has_lemma_regex(&tr.readings, &Regex::new(p).unwrap())
}
fn lemma_with(tr: &AnalyzedTokenReadings, l: &[&str], p: &str) -> bool {
    uk_helpers::has_lemma_with_pattern(&tr.readings, l, &Regex::new(p).unwrap())
}
fn agrees(
    rule: &TokenAgreementVerbNounRule,
    verb: &AnalyzedTokenReadings,
    naz: &[VerbInflection],
    indir: &[AnalyzedToken],
) -> bool {
    if !naz.is_empty() {
        let vi = verb_inflection::get_verb_inflections(&verb.readings);
        if vi.iter().any(|v| naz.contains(v)) {
            return true;
        }
    }
    if !indir.is_empty() {
        let cases = rule.gov.get_case_governments_opt(
            &verb.readings,
            None,
            Some(uk_helpers::verb_advp_pattern()),
        );
        if !cases.is_empty() && uk_helpers::has_vidm_pos_tag(&cases, indir) {
            return true;
        }
    }
    false
}

fn is_exception_inner(
    rule: &TokenAgreementVerbNounRule,
    tokens: &[&AnalyzedTokenReadings],
    state: &State,
    _noun_adj_naz: &[AnalyzedToken],
) -> bool {
    let verb_pos = state.verb_pos;
    let noun_adj_pos = state.noun_pos;
    let n = tokens.len();
    let clean_lower = tokens[noun_adj_pos].surface().to_lowercase();
    let verb_at = tokens[verb_pos];

    let gov_verb = |pos: usize| {
        rule.gov
            .get_case_governments_opt(&tokens[pos].readings, Some("verb"), None)
    };

    // боротиметься кілька однопартійців / входило двоє студентів
    if has_re(
        tokens[noun_adj_pos],
        &Regex::new(r"^numr.*v_naz.*$").unwrap(),
    ) || uk_helpers::has_lemma_with_pattern(
        &tokens[noun_adj_pos].readings,
        &[],
        uk_helpers::adv_quant_pattern(),
    ) && (has(tokens[noun_adj_pos], "noun.*v_naz.*")
        || has(tokens[noun_adj_pos], "adv.*")
        || has(tokens[noun_adj_pos], "part.*"))
    {
        if has_re(verb_at, &VERB_SN) {
            return true;
        }
        if verb_pos > 1
            && has(verb_at, "verb.*inf.*")
            && uk_helpers::has_lemma_with_pattern(
                &tokens[verb_pos - 1].readings,
                &["бути", "мусити"],
                &Regex::new(r"^verb.*(?:past:n|:s:3).*$").unwrap(),
            )
        {
            return true;
        }
    }
    if noun_adj_pos < n - 1 && lemma(verb_at, &["бути"]) && clean_lower == "марки" {
        return true;
    }
    if verb_pos > 1
        && lemma(verb_at, &["бути"])
        && uk_helpers::has_lemma_with_pattern(
            &tokens[verb_pos - 1].readings,
            &[
                "змушений",
                "вимушений",
                "повинний",
                "здатний",
                "готовий",
                "ладний",
                "радий",
            ],
            &Regex::new(r"^adj:.:v_naz.*$").unwrap(),
        )
    {
        return true;
    }
    if lemma_re(verb_at, r"з?могти") && has(tokens[noun_adj_pos], ".*v_oru.*") {
        return true;
    }
    if noun_adj_pos < n - 2
        && tokens[noun_adj_pos].surface().eq_ignore_ascii_case("тим")
        && tokens[noun_adj_pos + 1]
            .surface()
            .eq_ignore_ascii_case("більше")
    {
        return true;
    }
    if verb_pos > 1
        && lemma_re(verb_at, r"з?могти")
        && tokens[verb_pos - 1].surface().to_lowercase() == "чим"
    {
        return true;
    }
    if verb_at.surface().eq_ignore_ascii_case("стало")
        && Regex::new(r"^(?:відомо|видно|зрозуміло)$")
            .unwrap()
            .is_match(&tokens[verb_pos + 1].surface().to_lowercase())
            .unwrap_or(false)
    {
        return true;
    }
    if state.noun_pos == verb_pos + 2
        && lemma_re(tokens[verb_pos + 1], r"(?:най)?(?:більше|менше)")
        && has(tokens[noun_adj_pos], ".*:v_rod.*")
        && rule
            .gov
            .get_case_governments_opt(&verb_at.readings, None, Some(uk_helpers::verb_pattern()))
            .contains("v_zna")
    {
        return true;
    }
    if verb_pos > 1
        && tokens[verb_pos - 1].surface().to_lowercase() == "я"
        && tokens[verb_pos].surface().to_lowercase() == "буду"
        && (has(tokens[noun_adj_pos], "noun:inanim:.:v_zna.*")
            || has(tokens[noun_adj_pos], "adj:.:v_zna(?!:ranim).*"))
    {
        return true;
    }
    if lemma(verb_at, &["хотіти"]) && has_part(tokens[noun_adj_pos], "v_oru") {
        return true;
    }
    if lemma_re(verb_at, r"мати|маючи|мавши") && has_part(tokens[noun_adj_pos], "v_oru")
    {
        return true;
    }
    if lemma(verb_at, &["бути"]) && has(tokens[noun_adj_pos], "(adj|numr).*v_rod.*") {
        return true;
    }
    if verb_pos > 1
        && tokens[verb_pos - 1].surface().to_lowercase() == "що"
        && uk_helpers::has_lemma_with_pattern(
            &verb_at.readings,
            &["бути"],
            &Regex::new(r"^verb.*(?::s:3|past:n).*$").unwrap(),
        )
        && has(tokens[noun_adj_pos], "(adj|noun).*v_rod.*")
    {
        return true;
    }
    if verb_pos > 1
        && tokens[verb_pos].surface().to_lowercase() == "було"
        && tokens[verb_pos - 1].surface().to_lowercase() == "навіщо"
    {
        return true;
    }
    if verb_pos > 1
        && tokens[verb_pos].surface().to_lowercase() == "було"
        && has(tokens[verb_pos - 1], "(adv:comp[cs].*|.*predic.*)")
    {
        return true;
    }
    if verb_pos > 2
        && tokens[verb_pos].surface().to_lowercase() == "було"
        && Regex::new(r"^би?$")
            .unwrap()
            .is_match(&tokens[verb_pos - 1].surface().to_lowercase())
            .unwrap_or(false)
        && has(tokens[verb_pos - 2], "(adv:comp[cs].*|.*predic.*)")
    {
        return true;
    }
    if verb_pos > 1
        && tokens[verb_pos].surface().to_lowercase() == "було"
        && has(tokens[noun_adj_pos], ".*v_naz.*")
        && has(tokens[verb_pos - 1], "adj:.:v_naz.*:adjp:.*:perf.*")
    {
        return true;
    }
    if Regex::new(r"^(?:зайве|резон)$")
        .unwrap()
        .is_match(&clean_lower)
        .unwrap_or(false)
    {
        return true;
    }
    if tokens[noun_adj_pos - 1].surface().to_lowercase() == "далі"
        && has(tokens[noun_adj_pos], ".*v_rod.*")
    {
        return true;
    }
    if Regex::new(r"^(?:було|буде)$")
        .unwrap()
        .is_match(&tokens[verb_pos].surface().to_lowercase())
        .unwrap_or(false)
        && lemma_with(tokens[noun_adj_pos], &["весь"], ".*v_zna.*")
    {
        return true;
    }
    if Regex::new(r"^(?:було|буде)$")
        .unwrap()
        .is_match(&tokens[verb_pos].surface().to_lowercase())
        .unwrap_or(false)
        && SearchMatch::new()
            .target(vec![Condition::postag_re(
                &Regex::new(r"^.*predic.*$").unwrap(),
            )])
            .limit((noun_adj_pos - verb_pos) as i64)
            .m_after(tokens, verb_pos + 1)
            .is_some()
    {
        return true;
    }
    if Regex::new(r"^(?:було|буде)$")
        .unwrap()
        .is_match(&tokens[verb_pos].surface().to_lowercase())
        .unwrap_or(false)
        && SearchMatch::new()
            .target(vec![Condition::lemma_re(
                &Regex::new(r"^треба|потрібно$").unwrap(),
            )])
            .m_now(tokens, verb_pos - 1)
            .is_some()
    {
        return true;
    }
    if tokens[verb_pos].surface().to_lowercase() == "був" {
        if Regex::new(r"^(?:людина|знаменитість)$")
            .unwrap()
            .is_match(&tokens[noun_adj_pos].surface().to_lowercase())
            .unwrap_or(false)
        {
            return true;
        }
        if noun_adj_pos < n - 1 && tokens[noun_adj_pos + 1].surface().to_lowercase() == "людина"
        {
            return true;
        }
    }
    if verb_pos > 1
        && tokens[verb_pos - 1].surface().to_lowercase() == "конкурс"
        && uk_helpers::has_lemma_with_pattern(
            &verb_at.readings,
            &["бути"],
            &Regex::new(r"^verb.*(?::s:3|past:m).*$").unwrap(),
        )
        && has(tokens[noun_adj_pos], "num.*")
    {
        return true;
    }
    if noun_adj_pos - verb_pos > 1 {
        let adv_req = rule.gov.get_case_governments_opt(
            &tokens[noun_adj_pos - 1].readings,
            None,
            Some(&Regex::new(r"^adv(?!p).*$").unwrap()),
        );
        if !adv_req.is_empty() {
            for mid in &tokens[verb_pos + 1..noun_adj_pos] {
                if uk_helpers::has_vidm_pos_tag_token(&adv_req, mid) {
                    return true;
                }
            }
        }
    }
    if uk_helpers::PLUS_MINUS.contains(&clean_lower.as_str()) {
        return true;
    }
    if Regex::new(r"^(?:[0-9]+-.+|дорогою|толком|дивом|чверть|третину|половину|святая)$")
        .unwrap()
        .is_match(&clean_lower)
        .unwrap_or(false)
    {
        return true;
    }
    if noun_adj_pos < n - 1
        && has(tokens[noun_adj_pos], "adj:[fn]:v_(zna|oru).*")
        && uk_helpers::has_lemma_with_pattern(
            &tokens[noun_adj_pos + 1].readings,
            &["дорога", "життя", "міра"],
            &Regex::new(r"^noun:inanim:[fn]:v_(zna|oru).*$").unwrap(),
        )
    {
        return true;
    }
    if has_part(verb_at, "impers") && has(tokens[noun_adj_pos], ".*v_oru.*") {
        return true;
    }
    if lemma_with(tokens[noun_adj_pos], &["кожний"], ".*v_naz.*") {
        return true;
    }
    if lemma_re(verb_at, r"звати|називати|зватися|називатися")
        && tokens[noun_adj_pos]
            .surface()
            .chars()
            .next()
            .is_some_and(|c| c.is_uppercase())
    {
        return true;
    }
    if lemma_re(verb_at, r"тривати|протривати|йти|іти|ходити|їхати")
        && has(tokens[noun_adj_pos], "(adj|numr|noun:inanim).*v_zna.*")
    {
        return true;
    }
    if verb_pos > 3
        && tokens[verb_pos].surface().eq_ignore_ascii_case("впало")
        && tokens[verb_pos - 1].surface() == "ні"
    {
        return true;
    }
    if verb_pos > 2
        && tokens[verb_pos].surface().eq_ignore_ascii_case("сказати")
        && tokens[verb_pos - 1].surface() == "не"
        && has_part(tokens[noun_adj_pos], "v_naz")
    {
        return true;
    }
    if state.cases.contains("v_rod")
        && has(tokens[noun_adj_pos], "numr.*?v_zna.*|noun.*v_zna.*numr.*")
    {
        return true;
    }
    if noun_adj_pos < n - 1
        && has(tokens[noun_adj_pos], "(noun|adj):.*:v_rod.*")
        && has(tokens[noun_adj_pos + 1], "num.*")
    {
        return true;
    }
    if noun_adj_pos < n - 2
        && has(tokens[noun_adj_pos], "(noun|adj):.*:v_rod.*")
        && uk_helpers::is_dash(tokens[noun_adj_pos + 1])
        && has(tokens[noun_adj_pos + 2], "num.*")
    {
        return true;
    }
    if noun_adj_pos < n - 2 && has(tokens[noun_adj_pos], "(noun:inanim|adj):.:v_rod.*") {
        let v2pos = uk_helpers::token_search(
            tokens,
            state.noun_pos as i64 + 1,
            None,
            Some(&Regex::new(r"^на$").unwrap()),
            Some(&Regex::new(r"^[a-z].*$").unwrap()),
            Dir::Forward,
        );
        if v2pos >= 0 && v2pos <= state.noun_pos as i64 + 5 && v2pos < n as i64 - 1 {
            return true;
        }
    }
    if noun_adj_pos < n - 2
        && has(tokens[noun_adj_pos], "noun.*v_(rod|zna).*")
        && Regex::new(r"^(?:на|з|із|зо|під)$")
            .unwrap()
            .is_match(tokens[noun_adj_pos + 1].surface())
            .unwrap_or(false)
        && has(tokens[noun_adj_pos + 2], "number|numr.*v_zna.*")
    {
        return true;
    }
    if has_part(tokens[noun_adj_pos], "v_dav") {
        if has_part(verb_at, ":inf") {
            if verb_pos > 1
                && lemma(
                    tokens[verb_pos - 1],
                    &["як", "куди", "де", "що", "чого", "чи"],
                )
            {
                return true;
            }
            if noun_adj_pos < n - 1
                && Regex::new(r"^(?:ніколи|нікуди|нічого|нічим|ніде|немає?|не)$")
                    .unwrap()
                    .is_match(&tokens[noun_adj_pos + 1].surface().to_lowercase())
                    .unwrap_or(false)
            {
                return true;
            }
            if lemma(verb_at, &["жити", "сидіти", "судити"]) {
                return true;
            }
            if verb_pos > 1
                && Regex::new(r"^(?:ніколи|нікуди|нічого|нічим|ніде|де|немає?|не)$")
                    .unwrap()
                    .is_match(&tokens[verb_pos - 1].surface().to_lowercase())
                    .unwrap_or(false)
            {
                return true;
            }
            if verb_pos > 1
                && noun_adj_pos < n - 1
                && Regex::new(r"^(?:не|а?ні)$")
                    .unwrap()
                    .is_match(&tokens[verb_pos - 1].surface().to_lowercase())
                    .unwrap_or(false)
                && has_part(tokens[noun_adj_pos + 1], "v_rod")
            {
                return true;
            }
            if verb_pos > 1
                && Regex::new(r"^(?:слід|снаги|силу)$")
                    .unwrap()
                    .is_match(&tokens[verb_pos - 1].surface().to_lowercase())
                    .unwrap_or(false)
            {
                return true;
            }
        }
        if noun_adj_pos < n - 2
            && Regex::new(
                r"^(?:в|у|на|від|під|по|до|і?з|з[іо]|над|з-під|перед|попід|поза|напереріз)$",
            )
            .unwrap()
            .is_match(&tokens[noun_adj_pos + 1].surface().to_lowercase())
            .unwrap_or(false)
            && has(tokens[noun_adj_pos + 2], "(noun|adj).*")
        {
            return true;
        }
        if noun_adj_pos < n - 1
            && has(tokens[noun_adj_pos], ".*v_dav.*")
            && Regex::new(r"^(?:назустріч|навперейми|навздогін|услід)$")
                .unwrap()
                .is_match(&tokens[noun_adj_pos + 1].surface().to_lowercase())
                .unwrap_or(false)
        {
            return true;
        }
        if noun_adj_pos < n - 2 && has(tokens[noun_adj_pos], "noun.*?v_dav.*:pron:(pers|refl).*") {
            return true;
        }
    }
    if has_part(verb_at, ":inf") && tokens[noun_adj_pos].surface().eq_ignore_ascii_case("гріх")
    {
        return true;
    }
    if SearchMatch::new()
        .skip(vec![Condition::postag_re(
            &Regex::new(r"^.*v_(rod|zna|oru).*|part.*|number$").unwrap(),
        )])
        .target(vec![Condition::lemma_re(
            uk_helpers::time_plus_lemmas_pattern(),
        )])
        .limit(4)
        .m_after(tokens, noun_adj_pos)
        .is_some_and(|x| x > 0)
    {
        return true;
    }
    if noun_adj_pos < n - 3
        && has(tokens[noun_adj_pos], "numr.*v_zna.*")
        && SearchMatch::new()
            .target(vec![Condition::lemma_re(
                uk_helpers::time_plus_lemmas_pattern(),
            )])
            .limit(4)
            .m_after(tokens, noun_adj_pos + 1)
            .is_some_and(|x| x > 0)
    {
        return true;
    }
    if SearchMatch::new()
        .skip(vec![Condition::postag_re(
            &Regex::new(r"^.*v_oru.*|part.*|adv.*$").unwrap(),
        )])
        .target(vec![Condition::lemma_postag(
            "мова",
            "noun:inanim:.:v_oru.*",
        )])
        .limit(4)
        .m_after(tokens, noun_adj_pos)
        .is_some_and(|x| x > 0)
    {
        return true;
    }
    if noun_adj_pos < n - 1
        && tokens[noun_adj_pos + 1].surface().to_lowercase() == "кольору"
        && has_start(tokens[noun_adj_pos], "adj:m:v_rod")
    {
        return true;
    }
    if has_part(tokens[noun_adj_pos], "v_kly") && has_part(verb_at, "impr") {
        return true;
    }
    if has_part_readings(_noun_adj_naz, "noun:anim:m:v_naz")
        && has_re(verb_at, &VERB_F)
        && uk_helpers::has_masc_fem_lemma(_noun_adj_naz, &rule.masc_fem)
    {
        return true;
    }
    if has_part(tokens[state.noun_pos], "v_rod") && has_re(verb_at, &V_FUTR_PAST_S3_N) {
        return true;
    }
    if uk_helpers::has_lemma_regex_with_pattern(
        &verb_at.readings,
        &Regex::new(r"^(?:по)?меншати|(?:по)?більшати|стати$").unwrap(),
        &Regex::new(r"^verb.*:[sn](?::.*|$)$").unwrap(),
    ) && has(tokens[state.noun_pos], "(noun|adj).*v_rod.*")
    {
        return true;
    }
    if noun_adj_pos < n - 1
        && has(tokens[noun_adj_pos], "noun:.*v_rod.*")
        && Regex::new(r"^(?:менше|більше|удвічі|утричі)$")
            .unwrap()
            .is_match(tokens[noun_adj_pos + 1].surface())
            .unwrap_or(false)
    {
        return true;
    }
    if state.verb_pos > 1 && has_part(tokens[state.noun_pos], "v_rod") {
        let xpos = uk_helpers::token_search(
            tokens,
            state.verb_pos as i64 - 1,
            None,
            Some(&V_ROD_DRIVER_PATTERN),
            Some(&Regex::new(r"^[a-z].*$").unwrap()),
            Dir::Reverse,
        );
        if xpos >= 0 && xpos >= state.verb_pos as i64 - 4 {
            return true;
        }
    }
    if noun_adj_pos < n - 1
        && rule
            .gov
            .get_case_governments_opt(
                &verb_at.readings,
                None,
                Some(uk_helpers::verb_advp_pattern()),
            )
            .contains("v_inf")
    {
        let v2pos = uk_helpers::token_search(
            tokens,
            state.noun_pos as i64 + 1,
            None,
            None,
            Some(&Regex::new(r"^[a-z].*$").unwrap()),
            Dir::Forward,
        );
        if v2pos >= 0
            && v2pos <= state.noun_pos as i64 + 5
            && agrees(
                rule,
                tokens[v2pos as usize],
                &state.noun_adj_naz_inflections,
                &state.noun_adj_indir,
            )
        {
            return true;
        }
    }
    if noun_adj_pos < n - 1 && has_part(verb_at, ":inf") {
        let v2pos = uk_helpers::token_search(
            tokens,
            state.noun_pos as i64 + 1,
            None,
            None,
            Some(&Regex::new(r"^[a-z].*$").unwrap()),
            Dir::Forward,
        );
        if v2pos >= 0 && v2pos <= state.noun_pos as i64 + 4 {
            let v2 = tokens[v2pos as usize];
            if rule
                .gov
                .get_case_governments_opt(&v2.readings, None, Some(uk_helpers::verb_pattern()))
                .contains("v_inf")
            {
                if agrees(
                    rule,
                    v2,
                    &state.noun_adj_naz_inflections,
                    &state.noun_adj_indir,
                ) {
                    return true;
                }
                if tokens[v2pos as usize - 1].surface() == "не" {
                    return true;
                }
            }
        }
    }
    if noun_adj_pos < n - 1 && has_start(verb_at, "advp") {
        let v2pos = uk_helpers::token_search(
            tokens,
            state.noun_pos as i64 + 1,
            None,
            None,
            Some(&Regex::new(r"^[a-z].*$").unwrap()),
            Dir::Forward,
        );
        if v2pos >= 0
            && v2pos <= state.noun_pos as i64 + 3
            && agrees(
                rule,
                tokens[v2pos as usize],
                &state.noun_adj_naz_inflections,
                &state.noun_adj_indir,
            )
        {
            return true;
        }
    }
    if verb_pos > 1
        && has_start(verb_at, "advp")
        && ["посміхаючись", "сміючись"].contains(&verb_at.surface())
        && has_start(tokens[verb_pos - 1], "verb")
        && agrees(
            rule,
            tokens[verb_pos - 1],
            &state.noun_adj_naz_inflections,
            &state.noun_adj_indir,
        )
    {
        return true;
    }
    if noun_adj_pos < n - 1
        && has_part(verb_at, ":inf")
        && !uk_helpers::has_lemma_regex(&verb_at.readings, &VCHYTY_PATTERN)
    {
        let mut v2pos = uk_helpers::token_search(
            tokens,
            state.noun_pos as i64 + 1,
            None,
            None,
            Some(&Regex::new(r"^[a-z].*$").unwrap()),
            Dir::Forward,
        );
        while v2pos >= 0 && v2pos <= state.noun_pos as i64 + 4 {
            let cases = rule.gov.get_case_governments_opt(
                &tokens[v2pos as usize].readings,
                None,
                Some(&ADV_PREDICT_PATTERN),
            );
            if uk_helpers::has_vidm_pos_tag(&cases, &state.noun_adj_indir) {
                return true;
            }
            v2pos = uk_helpers::token_search(
                tokens,
                v2pos + 1,
                None,
                None,
                Some(&Regex::new(r"^[a-z].*$").unwrap()),
                Dir::Forward,
            );
        }
    }
    if noun_adj_pos < n - 1 && has_part(verb_at, ":inf") {
        let v2pos = uk_helpers::token_search(
            tokens,
            state.noun_pos as i64 + 1,
            None,
            None,
            Some(&Regex::new(r"^[a-z].*$").unwrap()),
            Dir::Forward,
        );
        if v2pos >= 0
            && v2pos <= state.noun_pos as i64 + 3
            && rule
                .gov
                .get_case_governments_opt(
                    &tokens[v2pos as usize].readings,
                    None,
                    Some(uk_helpers::adj_v_naz_pattern()),
                )
                .contains("v_inf")
        {
            let g1 = uk_helpers::get_genders_token(
                tokens[noun_adj_pos],
                &Regex::new(r"^(?:noun|adj).*v_naz.*$").unwrap(),
            );
            let g2 = uk_helpers::get_genders_token(
                tokens[v2pos as usize],
                uk_helpers::adj_v_naz_pattern(),
            );
            if !g2.is_empty()
                && Regex::new(&format!("^.*[{g2}].*$"))
                    .unwrap()
                    .is_match(&g1)
                    .unwrap_or(false)
            {
                return true;
            }
        }
    }
    if has_part(verb_at, ":inf")
        && has_re(tokens[noun_adj_pos], uk_helpers::adj_v_naz_pattern())
        && rule
            .gov
            .get_case_governments_opt(
                &tokens[noun_adj_pos].readings,
                None,
                Some(uk_helpers::adj_v_naz_pattern()),
            )
            .contains("v_inf")
    {
        return true;
    }
    let _ = gov_verb;
    false
}

fn has_start(tr: &AnalyzedTokenReadings, p: &str) -> bool {
    uk_helpers::has_reading_pos_tag_start(&tr.readings, p)
}
