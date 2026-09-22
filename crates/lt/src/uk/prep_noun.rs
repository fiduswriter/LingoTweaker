//! Port of `org.languagetool.rules.uk.TokenAgreementPrepNounRule`
//! (`UK_PREP_NOUN_INFLECTION_AGREEMENT`) and
//! `TokenAgreementPrepNounExceptionHelper`.
#![allow(clippy::if_same_then_else, clippy::collapsible_if)]

use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Suggestion, TextRange};
use lt_tagger::uk_helpers::{self, Dir};
use lt_tagger::{UkrainianSynthesizer, UkrainianTagger};

use crate::uk::gov::CaseGovernment;

pub const RULE_ID: &str = "UK_PREP_NOUN_INFLECTION_AGREEMENT";
const DESCRIPTION: &str = "Узгодження прийменника та іменника у реченні";
const SHORT: &str = "Узгодження прийменника та іменника";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Різне";
const USED_U_INSTEAD_OF_A_MSG: &str = ". Можливо, вжито невнормований родовий відмінок ч.р. з закінченням -у/-ю замість -а/-я (така тенденція є в сучасній мові)?";
const QUOTES: &[&str] = &["«", "\"", "„", "“"];
const Z_ZI_IZ: &[&str] = &["з", "зі", "із"];
const Z_ZI_IZ_ZO: &[&str] = &["з", "зі", "із", "зо"];
const REQ_ANIM_INANIM: &str = ":r(?:in)?anim";

static REQ_ANIM_INANIM_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(REQ_ANIM_INANIM).unwrap());
static VIDMINOK_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":(v_[a-z]+)").unwrap());
static APPROX_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:noun.*v_oru.*|adv.*|part.*)$").unwrap());
static APPROX_LEMMAS: &[&str] = &[
    "розмір",
    "величина",
    "товщина",
    "вартість",
    "ріст",
    "зріст",
    "висота",
    "глибина",
    "діаметр",
    "вага",
    "обсяг",
    "площа",
    "приблизно",
    "десь",
    "завбільшки",
    "завширшки",
    "завдовжки",
    "завтовшки",
    "заввишки",
    "завглибшки",
    "накладом",
];

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

pub struct State {
    prep_pos: usize,
    prep_idx: usize,
    zi_zna_removed: bool,
    pos_tags_to_find: Vec<String>,
}

pub struct TokenAgreementPrepNounRule {
    tagger: std::sync::Arc<UkrainianTagger>,
    synth: std::sync::Arc<UkrainianSynthesizer>,
    gov: std::sync::Arc<CaseGovernment>,
}

impl TokenAgreementPrepNounRule {
    pub fn new(
        tagger: std::sync::Arc<UkrainianTagger>,
        synth: std::sync::Arc<UkrainianSynthesizer>,
        gov: std::sync::Arc<CaseGovernment>,
    ) -> Self {
        Self { tagger, synth, gov }
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
        let mut state_opt: Option<State> = None;
        let mut i = 1usize;
        while i < n {
            let tr = view[i];
            let pos_tag = tr.readings.first().and_then(|r| r.pos_tag.clone());
            let this_token = tr.surface().to_string();

            if QUOTES.contains(&this_token.as_str()) {
                i += 1;
                continue;
            }
            if pos_tag.is_none() || tr.is_immunized {
                state_opt = None;
                i += 1;
                continue;
            }
            if i > 1
                && this_token.chars().count() == 1
                && this_token.chars().next().is_some_and(|c| c.is_uppercase())
                && tr.whitespace_before
                && Regex::new(r"^.*[а-яіїєґ0-9]$")
                    .unwrap()
                    .is_match(view[i - 1].surface())
                    .unwrap_or(false)
            {
                state_opt = None;
                i += 1;
                continue;
            }

            if let Some(mw) = get_multiword_token(tr) {
                if Z_ZI_IZ.contains(&this_token.to_lowercase().as_str())
                    && mw.stem.as_deref().is_some_and(|l| l.starts_with("згідно "))
                {
                    state_opt = Some(State {
                        prep_pos: i,
                        prep_idx: i,
                        zi_zna_removed: false,
                        pos_tags_to_find: Vec::new(),
                    });
                    i += 1;
                    continue;
                } else {
                    if pos_tag.as_deref().is_some_and(|t| t.starts_with("prep")) {
                        state_opt = None;
                        i += 1;
                        continue;
                    }
                    let mw_tag = mw.pos_tag.as_deref().unwrap_or("");
                    if !mw_tag.contains("adv") && !mw_tag.contains("insert") {
                        state_opt = None;
                    }
                    i += 1;
                    continue;
                }
            }

            if pos_tag.as_deref().is_some_and(|t| t.starts_with("prep")) {
                let prep = this_token.to_lowercase();
                if prep == "понад" {
                    i += 1;
                    continue;
                }
                if Regex::new(r"^(?:шляхом|од|поруч|ради)$")
                    .unwrap()
                    .is_match(&prep)
                    .unwrap_or(false)
                {
                    state_opt = None;
                    i += 1;
                    continue;
                }
                state_opt = Some(State {
                    prep_pos: i,
                    prep_idx: i,
                    zi_zna_removed: false,
                    pos_tags_to_find: Vec::new(),
                });
                i += 1;
                continue;
            }

            let Some(mut state) = state_opt.take() else {
                i += 1;
                continue;
            };

            if this_token.to_lowercase() == "ван" {
                state_opt = Some(state);
                i += 1;
                continue;
            }
            if this_token == "Фон" {
                state_opt = Some(state);
                i += 1;
                continue;
            }
            if this_token.to_lowercase() == "та" {
                i += 1;
                continue;
            }

            let mut pos_tags_to_find: Vec<String> = Vec::new();
            let prep_lemma = view[state.prep_idx]
                .readings
                .first()
                .and_then(|r| r.stem.clone())
                .unwrap_or_default();

            if prep_lemma == "замість" {
                pos_tags_to_find.push("v_naz".to_string());
            } else if prep_lemma == "за"
                && i > 1
                && view[state.prep_pos - 1]
                    .surface()
                    .eq_ignore_ascii_case("що")
            {
                pos_tags_to_find.push("v_naz".to_string());
            }

            if QUOTES.contains(&view[i - 1].surface()) {
                if lt_tagger::uk_helpers::is_capitalized(view[i].surface())
                    || view[state.prep_idx]
                        .surface()
                        .eq_ignore_ascii_case("замість")
                {
                    i += 1;
                    continue;
                }
                pos_tags_to_find.push("v_naz".to_string());
            }

            let mut expected_cases = self.gov.get_case_governments_opt(
                &view[state.prep_idx].readings,
                Some("prep"),
                None,
            );
            if Z_ZI_IZ_ZO.contains(&prep_lemma.as_str()) {
                if view[i].surface().eq_ignore_ascii_case("нізвідки") {
                    i += 1;
                    continue;
                }
                if Z_ZI_IZ.contains(&prep_lemma.as_str())
                    && i >= 3
                    && view[i - 2].surface().eq_ignore_ascii_case("згідно")
                {
                    expected_cases = vec!["v_oru".to_string()];
                } else if !self.is_likely_approx_with_zi(&view, i, &state) {
                    expected_cases.retain(|c| c != "v_zna");
                    state.zi_zna_removed = true;
                }
            }
            expected_cases.retain(|c| c != "v_inf");
            for c in expected_cases {
                if !pos_tags_to_find.contains(&c) {
                    pos_tags_to_find.push(c);
                }
            }
            state.pos_tags_to_find = pos_tags_to_find;

            let ex = get_exception_strong(&view, i, state.prep_idx, &self.gov, &self.tagger);
            match ex.r#type {
                Type::Exception => {
                    i += 2;
                    continue;
                }
                Type::Skip => {
                    i += ex.skip + 1;
                    continue;
                }
                Type::None => {}
            }

            if uk_helpers::has_reading_pos_tag_part(&tr.readings, ":v_") {
                let pron_pos_noun: Vec<AnalyzedToken> = tr
                    .readings
                    .iter()
                    .filter(|r| {
                        uk_helpers::has_reading_pos_tag(
                            std::slice::from_ref(r),
                            &Regex::new(r"^noun:unanim:.:v_rod.*pron.*$").unwrap(),
                        ) && r
                            .stem
                            .as_deref()
                            .is_some_and(|l| ["вони", "він", "вона", "воно"].contains(&l))
                    })
                    .cloned()
                    .collect();
                if !pron_pos_noun.is_empty()
                    && !Regex::new(r"^(?:них|нього|неї)(?:-[а-я]+)?$")
                        .unwrap()
                        .is_match(&this_token.to_lowercase())
                        .unwrap_or(false)
                {
                    if i < n - 1
                        && (uk_helpers::has_reading_pos_tag(
                            &view[i + 1].readings,
                            &Regex::new(r"^(?:noun|adj|adv|part|num|conj:coord|noninfl).*$")
                                .unwrap(),
                        ) || Regex::new(r#"^["«„“/$€…]|[a-zA-Z'-]+$"#)
                            .unwrap()
                            .is_match(view[i + 1].surface())
                            .unwrap_or(false))
                    {
                        state_opt = Some(state);
                        i += 1;
                        continue;
                    } else {
                        let insert_end = find_insert_end(&view, i + 1, true);
                        if insert_end > 0 {
                            i = insert_end as usize + 1;
                            continue;
                        }
                        out.push(self.create_match(&view, &state, i, sentence_offset));
                        i += 1;
                        continue;
                    }
                }

                let pron_pos_adj: Vec<AnalyzedToken> = tr
                    .readings
                    .iter()
                    .filter(|r| {
                        uk_helpers::has_reading_pos_tag(
                            std::slice::from_ref(r),
                            &Regex::new(r"^adj.*pron:pos(?!:bad).*$").unwrap(),
                        ) && r
                            .stem
                            .as_deref()
                            .is_some_and(|l| ["їх", "його", "її"].contains(&l))
                    })
                    .cloned()
                    .collect();
                if !pron_pos_adj.is_empty() {
                    if !uk_helpers::has_vidm_pos_tag(&state.pos_tags_to_find, &pron_pos_adj) {
                        out.push(self.create_match(&view, &state, i, sentence_offset));
                        i += 1;
                        continue;
                    }
                    if i < n - 1 {
                        state_opt = Some(state);
                        i += 1;
                        continue;
                    }
                } else if this_token == "їх" {
                    out.push(self.create_match(&view, &state, i, sentence_offset));
                    i += 1;
                    continue;
                }

                if uk_helpers::has_vidm_pos_tag_token(&state.pos_tags_to_find, tr) {
                    i += 1;
                    continue;
                }

                let ex = get_exception_non_infl(&view, i, &self.gov);
                match ex.r#type {
                    Type::Exception => {
                        i += 2;
                        continue;
                    }
                    Type::Skip => {
                        i += ex.skip + 1;
                        continue;
                    }
                    Type::None => {}
                }
                let ex = get_exception_infl(&view, i, &state, &self.gov);
                match ex.r#type {
                    Type::Exception => {
                        i += 2;
                        continue;
                    }
                    Type::Skip => {
                        i += ex.skip + 1;
                        continue;
                    }
                    Type::None => {}
                }
                out.push(self.create_match(&view, &state, i, sentence_offset));
            } else {
                let ex = get_exception_non_infl(&view, i, &self.gov);
                match ex.r#type {
                    Type::Exception => {
                        i += 2;
                        continue;
                    }
                    Type::Skip => {
                        i += ex.skip + 1;
                        continue;
                    }
                    Type::None => {}
                }
            }
            i += 1;
        }
        out
    }

    fn is_likely_approx_with_zi(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        i: usize,
        state: &State,
    ) -> bool {
        if Regex::new(r"^.*поверх(?:ов|ів).*$")
            .unwrap()
            .is_match(tokens[i].surface())
            .unwrap_or(false)
        {
            return true;
        }
        let lemmas: Vec<&str> = uk_helpers::TIME_PLUS_LEMMAS
            .iter()
            .copied()
            .chain(["ложка", "ложечка"])
            .collect();
        uk_helpers::has_reading_pos_tag(
            &tokens[i].readings,
            &Regex::new(r"^(?:noun:inanim:[fnm]:v_zna.*num.*|num.*)$").unwrap(),
        ) || uk_helpers::has_lemma_with_pattern(
            &tokens[i].readings,
            &lemmas,
            &Regex::new(r"^noun:inanim:[mnf]:v_zna.*$").unwrap(),
        ) || (i < tokens.len() - 1
            && uk_helpers::has_reading_pos_tag(
                &tokens[i].readings,
                &Regex::new(r"^adj:[mnf]:v_zna.*$").unwrap(),
            )
            && uk_helpers::has_lemma_with_pattern(
                &tokens[i + 1].readings,
                &lemmas,
                &Regex::new(r"^noun:inanim:[mnf]:v_zna.*$").unwrap(),
            ))
            || uk_helpers::has_lemma_with_pattern(
                &tokens[state.prep_pos - 1].readings,
                APPROX_LEMMAS,
                &APPROX_TAG,
            )
            || (i < tokens.len() - 1
                && uk_helpers::has_lemma_with_pattern(
                    &tokens[i + 1].readings,
                    APPROX_LEMMAS,
                    &APPROX_TAG,
                ))
    }

    fn create_match(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        state: &State,
        i: usize,
        sentence_offset: usize,
    ) -> Match {
        let tr = tokens[i];
        let token_string = tr.surface().to_lowercase();
        let mut suggestions: Vec<String> = Vec::new();
        let required = format!(":({})", state.pos_tags_to_find.join("|"));
        for at in &tr.readings {
            let Some(old) = at.pos_tag.as_deref() else {
                continue;
            };
            let mut apply = required.clone();
            let m = REQ_ANIM_INANIM_PATTERN.find(old).ok().flatten();
            if let Some(m) = m {
                apply.push_str(m.as_str());
            } else {
                apply.push_str(&format!("(?:{REQ_ANIM_INANIM})?"));
            }
            let pos_tag = Regex::new(r":v_[a-z]+")
                .unwrap()
                .replace(old, apply.as_str())
                .into_owned();
            for s in self.synth.synthesize(at, &pos_tag, true) {
                if !suggestions.contains(&s) {
                    suggestions.push(s);
                }
            }
        }

        let req_names: Vec<String> = state
            .pos_tags_to_find
            .iter()
            .map(|v| uk_helpers::case_name(v))
            .collect();

        let mut found_names: Vec<String> = Vec::new();
        for at in &tr.readings {
            let Some(pos) = at.pos_tag.as_deref() else {
                continue;
            };
            if pos.contains(":v_") {
                let vidm = VIDMINOK_REGEX
                    .captures(pos)
                    .ok()
                    .flatten()
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
                    .unwrap_or_default();
                let mut name = uk_helpers::case_name(&vidm);
                if found_names.contains(&name) {
                    if pos.contains(":p:") {
                        name.push_str(" (мн.)");
                        found_names.push(name);
                    }
                } else {
                    found_names.push(name);
                }
            }
        }

        let mut msg = format!(
            "Прийменник «{}» вимагає іншого відмінка: {}, а знайдено: {}",
            tokens[state.prep_idx].surface(),
            req_names.join(", "),
            found_names.join(", ")
        );
        if state.zi_zna_removed {
            msg.push_str(". Але з.в. вимагається у випадках порівнянн предметів.");
        }
        if state.pos_tags_to_find.iter().any(|c| c == "v_rod")
            && Regex::new(r"^.*[ую]$")
                .unwrap()
                .is_match(tr.surface())
                .unwrap_or(false)
            && uk_helpers::has_reading_pos_tag(
                &tr.readings,
                &Regex::new(r"^noun.*?:m:v_dav.*$").unwrap(),
            )
        {
            msg.push_str(USED_U_INSTEAD_OF_A_MSG);
        } else if token_string == "їх" {
            msg.push_str(
                ". Можливо, тут потрібно присвійний займенник «їхній» або нормативна форма р.в. «них»?",
            );
        } else if token_string == "його" || token_string == "її" {
            let repl = if token_string == "його" {
                "нього"
            } else {
                "неї"
            };
            msg.push_str(&format!(
                ". Можливо, тут потрібно присвійний займенник «{repl}»?"
            ));
        } else if tokens[state.prep_idx].surface().eq_ignore_ascii_case("о") {
            for at in &tr.readings {
                if uk_helpers::has_reading_pos_tag(
                    std::slice::from_ref(at),
                    &Regex::new(r"^noun:anim:.:v_naz.*$").unwrap(),
                ) {
                    msg.push_str(". Можливо, тут «о» — це вигук і потрібно кличний відмінок?");
                    if let Some(pos) = at.pos_tag.as_deref() {
                        let new_pos = pos.replace("v_naz", "v_kly");
                        for s in self.synth.synthesize(at, &new_pos, false) {
                            if s != at.token && !suggestions.contains(&s) {
                                suggestions.push(s);
                            }
                        }
                    }
                    break;
                }
            }
        } else if uk_helpers::has_reading_pos_tag_start(&tokens[i - 1].readings, "adv") {
            let merged = format!(
                "{}{}",
                tokens[state.prep_idx].surface(),
                tokens[i - 1].surface()
            );
            let tagged = self.tagger.tag(&[merged]);
            if tagged
                .first()
                .is_some_and(|t| uk_helpers::has_reading_pos_tag_start(&t.readings, "adv"))
            {
                msg.push_str(". Можливо, прийменник і прислівник мають бути одним словом?");
            }
        }

        Match::new(
            RULE_ID,
            Option::<String>::None,
            msg,
            Some(SHORT.to_string()),
            TextRange::new(
                sentence_offset + tr.start_pos,
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
        .with_metadata(DESCRIPTION, "misspelling", 0)
    }
}

fn get_multiword_token(tr: &AnalyzedTokenReadings) -> Option<AnalyzedToken> {
    tr.readings
        .iter()
        .find(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with('<')))
        .cloned()
}

fn find_insert_end(tokens: &[&AnalyzedTokenReadings], i: usize, _look_for_part: bool) -> i64 {
    let n = tokens.len();
    if i >= n - 2 {
        return -1;
    }
    let mut next_pos = i;
    if Regex::new(r"^же?$")
        .unwrap()
        .is_match(tokens[i].surface())
        .unwrap_or(false)
    {
        next_pos = i + 1;
    }
    if next_pos > n - 3 {
        return if next_pos == i {
            -1
        } else {
            next_pos as i64 - 1
        };
    }
    if tokens[i].is_pos_tag_unknown
        && Regex::new(r"^[,(]$")
            .unwrap()
            .is_match(tokens[i].surface())
            .unwrap_or(false)
    {
        let comma_pos = uk_helpers::token_search(
            tokens,
            i as i64 + 1,
            None,
            Some(&Regex::new(r"^[,)]$").unwrap()),
            None,
            Dir::Forward,
        );
        if comma_pos > i as i64 + 1
            && comma_pos < i as i64 + 6
            && comma_pos < n as i64 - 1
            && tokens[comma_pos as usize + 1].surface() != "що"
            && tokens[i].surface().replace('(', ")") == tokens[comma_pos as usize].surface()
        {
            return comma_pos;
        }
    }
    if next_pos == i {
        -1
    } else {
        next_pos as i64 - 1
    }
}

#[allow(clippy::too_many_arguments)]
fn get_exception_infl(
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
    state: &State,
    gov: &CaseGovernment,
) -> RuleException {
    let tr = tokens[i];
    let token = tr.surface().to_string();
    let token_lower = token.to_lowercase();
    let prep = tokens[state.prep_idx].surface().to_lowercase();
    let n = tokens.len();

    if token == "дивом" {
        return RuleException::skip(0);
    }
    if i < n - 1
        && token == "тисяча"
        && (uk_helpers::has_reading_pos_tag_part(&tokens[i + 1].readings, "numr")
            || uk_helpers::has_lemma(&tokens[i + 1].readings, &["якийсь"]))
    {
        return RuleException::skip(0);
    }
    if i < n - 1
        && uk_helpers::has_reading_pos_tag_part(&tr.readings, "numr")
        && uk_helpers::has_reading_pos_tag_part(&tr.readings, "v_naz")
        && uk_helpers::has_reading_pos_tag_part(&tokens[i + 1].readings, "numr")
        && uk_helpers::has_reading_pos_tag(
            &tr.readings,
            &Regex::new(r"^.*v_(?:rod|dav|zna|oru|mis).*$").unwrap(),
        )
    {
        return RuleException::skip(1);
    }
    if prep == "на" {
        if lt_tagger::uk_helpers::is_capitalized(&token)
            && uk_helpers::has_pos_tag_str2(tr, "noun.*?:.:v_rod.*")
        {
            return RuleException::exception();
        }
        if uk_helpers::has_pos_tag_str2(tr, "noun:anim:.:v_naz:prop:[fl]name.*")
            && ((i > 1
                && ["ім'я", "прізвище"].contains(
                    &tokens[i - 2]
                        .readings
                        .first()
                        .map(|r| r.token.clone())
                        .unwrap_or_default()
                        .as_str(),
                ))
                || (i > 2
                    && ["ім'я", "прізвище"].contains(
                        &tokens[i - 3]
                            .readings
                            .first()
                            .and_then(|r| r.stem.clone())
                            .unwrap_or_default()
                            .as_str(),
                    )))
        {
            return RuleException::exception();
        }
        if Regex::new(r"^(?:ти|ви)$")
            .unwrap()
            .is_match(&token_lower)
            .unwrap_or(false)
        {
            return RuleException::exception();
        }
        if i < n - 1 && token == "Піп" && tokens[i + 1].surface() == "Іван" {
            return RuleException::exception();
        }
        if token_lower == "манер" {
            return RuleException::exception();
        }
    }
    if state.prep_pos > 1 && prep == "заради" {
        if Regex::new(r"(?i)^(?:справедливості|об.єктивності)$")
            .unwrap()
            .is_match(tokens[state.prep_pos - 1].surface())
            .unwrap_or(false)
        {
            return RuleException::exception();
        }
    }
    if prep == "при" && token == "їх" {
        return RuleException::skip(0);
    } else if prep == "з" && token == "рана" {
        return RuleException::exception();
    } else if prep == "від"
        && (token.eq_ignore_ascii_case("а")
            || token == "рана"
            || token == "корки"
            || token == "мала")
    {
        return RuleException::exception();
    } else if prep == "до"
        && (token.eq_ignore_ascii_case("я") || token == "корки" || token == "велика")
    {
        return RuleException::exception();
    }
    if n > i + 1 {
        if (uk_helpers::has_reading_pos_tag_start(&tokens[i + 1].readings, "num")
            || tokens[i + 1].surface() == "$")
            && uk_helpers::PLUS_MINUS.contains(&token_lower.as_str())
        {
            return RuleException::exception();
        }
        if uk_helpers::has_pos_tag_str2(tr, "noun.*?:v_oru.*")
            && uk_helpers::has_reading_pos_tag_part(&tokens[i + 1].readings, "adjp:pasv")
        {
            return RuleException::skip(1);
        }
        if token == "святая" && tokens[i + 1].surface() == "святих" {
            return RuleException::exception();
        }
        if (prep == "через" || prep == "на")
            && uk_helpers::has_lemma_with_pattern(
                &tr.readings,
                uk_helpers::TIME_PLUS_LEMMAS,
                &Regex::new(r"^noun:inanim:p:v_(?:rod|zna).*$").unwrap(),
            )
            && (uk_helpers::has_reading_pos_tag_part(&tokens[i + 1].readings, "num")
                || (i < n - 2
                    && uk_helpers::has_lemma(&tokens[i + 1].readings, &["зо", "з", "із"])
                    && uk_helpers::has_reading_pos_tag_part(&tokens[i + 2].readings, "num")))
        {
            return RuleException::exception();
        }
        if uk_helpers::has_pos_tag_str2(tr, "noun.*v_dav.*:pron:(refl|pers).*")
            && tokens[i + 1].surface().starts_with("подібн")
        {
            return RuleException::skip(0);
        }
        if (token == "усім" || token == "всім") && tokens[i + 1].surface().starts_with("відом")
        {
            return RuleException::skip(0);
        }
        if prep.eq_ignore_ascii_case("до") && token == "схід" && tokens[i + 1].surface() == "сонця"
        {
            return RuleException::exception();
        }
        if n > i + 2 {
            if uk_helpers::has_pos_tag_str2(tr, "adj:[mfn]:v_rod.*") {
                let genders =
                    uk_helpers::get_genders_token(tr, &Regex::new(r"^adj:[mfn]:v_rod.*$").unwrap());
                if !genders.is_empty()
                    && uk_helpers::has_pos_tag_str2(
                        tokens[i + 1],
                        &format!("noun.*?:[{genders}]:v_rod.*"),
                    )
                {
                    return RuleException::skip(1);
                }
            }
            if uk_helpers::has_pos_tag_str2(tr, "noun.*v_(?:dav|oru).*:pron:neg.*")
                && tokens[i + 1].surface() == "не"
            {
                return RuleException::skip(0);
            }
        }
    }
    let _ = gov;
    RuleException::none()
}

fn get_exception_strong(
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
    prep_idx: usize,
    gov: &CaseGovernment,
    _tagger: &UkrainianTagger,
) -> RuleException {
    let tr = tokens[i];
    let token = tr.surface().to_string();
    let token_lower = token.to_lowercase();
    let prep = tokens[prep_idx].surface().to_lowercase();
    let n = tokens.len();

    if prep == "до" || prep == "по" {
        if Regex::new(r"^(?:сьогодні|[ву]чора|позавчора|(?:після)?завтра|тепер|зараз|нині|опівдня|опівночі|досі|навпаки)$")
            .unwrap()
            .is_match(&token_lower)
            .unwrap_or(false)
        {
            return RuleException::exception();
        }
    }
    if prep == "на" || prep == "від" || prep == "про" {
        if Regex::new(r"^(?:сьогодні|[ву]чора|позавчора|(?:після)?завтра|тепер|зараз|нині|тоді|потім|щодень|повсякдень)$")
            .unwrap()
            .is_match(&token_lower)
            .unwrap_or(false)
        {
            return RuleException::exception();
        }
    }
    if Regex::new(r"^(?:за|зі?|із)$")
        .unwrap()
        .is_match(&prep)
        .unwrap_or(false)
        && Regex::new(r"^(?:сьогодні|[ву]чора|позавчора|(?:після)?завтра)$")
            .unwrap()
            .is_match(&token_lower)
            .unwrap_or(false)
    {
        return RuleException::exception();
    }
    if (prep == "в" || prep == "у") && token_lower == "нікуди" {
        return RuleException::exception();
    }
    if i < n - 1
        && token == "не"
        && uk_helpers::has_reading_pos_tag_start(&tokens[i + 1].readings, "ad")
    {
        return RuleException::skip(0);
    }
    if i < n - 1
        && uk_helpers::has_lemma_regex_with_pattern(
            &tr.readings,
            uk_helpers::adv_quant_pattern(),
            &Regex::new(r"^adv.*$").unwrap(),
        )
    {
        return RuleException::exception();
    }
    if i < n - 1
        && Z_ZI_IZ.contains(&prep.as_str())
        && uk_helpers::has_lemma(&tokens[i].readings, uk_helpers::PSEUDO_NUM_LEMMAS)
    {
        return RuleException::exception();
    }
    if uk_helpers::has_pos_tag_all(&tr.readings, &Regex::new(r"^adv(?!p).*$").unwrap()) {
        if i < n - 1 && tokens[i + 1].surface() == "собі" {
            return RuleException::skip(1);
        }
        return RuleException::skip(0);
    }
    if prep == "замість"
        && lt_tagger::uk_helpers::has_pos_tag_re(tr, &Regex::new(r"^verb.*:inf.*$").unwrap())
    {
        return RuleException::exception();
    }
    let _ = gov;
    RuleException::none()
}

fn get_exception_non_infl(
    tokens: &[&AnalyzedTokenReadings],
    i: usize,
    gov: &CaseGovernment,
) -> RuleException {
    let tr = tokens[i];
    let token = tr.surface().to_string();
    let n = tokens.len();
    if uk_helpers::has_reading_pos_tag_start(&tr.readings, "part")
        && uk_helpers::part_insert_pattern()
            .is_match(&token.to_lowercase())
            .unwrap_or(false)
    {
        return RuleException::skip(0);
    }
    if Regex::new(r"^лиш(?:е(?:нь)?)?$")
        .unwrap()
        .is_match(&token)
        .unwrap_or(false)
    {
        return RuleException::skip(0);
    }
    if token == "наприклад" {
        return RuleException::skip(0);
    }
    if uk_helpers::has_reading_pos_tag(&tr.readings, &Regex::new(r"^adv(?!p).*$").unwrap()) {
        if i < n - 1
            && uk_helpers::has_reading_pos_tag_start(&tokens[i + 1].readings, "adj")
            && uk_helpers::has_pos_tag_part_all(&tr.readings, "adv")
        {
            return RuleException::skip(0);
        }
        return RuleException::exception();
    }
    if n > i + 1
        && uk_helpers::has_reading_pos_tag(
            &tr.readings,
            &Regex::new(r"^noun:(?:un)?anim:.:v_dav.*:pron.*$").unwrap(),
        )
    {
        if uk_helpers::has_reading_pos_tag_start(&tokens[i + 1].readings, "adj")
            && gov
                .get_case_governments_opt(&tokens[i + 1].readings, None, None)
                .iter()
                .any(|c| c == "v_dav")
        {
            return RuleException::skip(1);
        }
        if n > i + 2
            && uk_helpers::has_reading_pos_tag_start(&tokens[i + 1].readings, "adv")
            && uk_helpers::has_reading_pos_tag_start(&tokens[i + 2].readings, "adj")
            && gov
                .get_case_governments_opt(&tokens[i + 2].readings, None, None)
                .iter()
                .any(|c| c == "v_dav")
        {
            return RuleException::skip(2);
        }
    }
    if n > i + 2
        && token == "нічого"
        && tokens[i + 1].surface() == "не"
        && uk_helpers::has_reading_pos_tag_start(&tokens[i + 2].readings, "adj")
    {
        return RuleException::skip(1);
    }
    RuleException::none()
}
