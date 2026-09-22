//! Port of `org.languagetool.rules.uk.TokenAgreementAdjNounRule`
//! (`UK_ADJ_NOUN_INFLECTION_AGREEMENT`) and
//! `TokenAgreementAdjNounExceptionHelper`.

use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Suggestion, TextRange};
use lt_tagger::uk_helpers::{self, Dir};
use lt_tagger::UkrainianSynthesizer;

use crate::uk::gov::CaseGovernment;
use crate::uk::inflection::{self, Inflection};
use crate::uk::search_helper::Condition;
use crate::uk::search_helper::Match as SearchMatch;

pub const RULE_ID: &str = "UK_ADJ_NOUN_INFLECTION_AGREEMENT";
const DESCRIPTION: &str = "Узгодження відмінків, роду і числа прикметника та іменника";
const SHORT: &str = "Узгодження прикметника та іменника";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Різне";
const USED_U_INSTEAD_OF_A_MSG: &str = ". Можливо, вжито невнормований родовий відмінок ч.р. з закінченням -у/-ю замість -а/-я (така тенденція є в сучасній мові)?";

static FAKE_FEM_LIST: &[&str] = &[
    "ступінь",
    "степінь",
    "продаж",
    "собака",
    "дріб",
    "ярмарок",
    "нежить",
    "рукопис",
    "накип",
    "насип",
    "путь",
];
static CONJ_FOR_PLURAL_WITH_COMMA: &[&str] = &[
    "і",
    "а",
    "й",
    "та",
    "чи",
    "або",
    "ані",
    "також",
    "плюс",
    "то",
    "a",
    "i",
    ",",
];

static VERB_ADVP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:verb|advp).*$").unwrap());
static NUMBER_V_NAZ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:number|numr:p:v_naz|noun.*?:p:v_naz.*:numr.*)$").unwrap());
static DOVYE_TROYE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:.*[2-4]|.*[2-4][\u{2013}\u{2014}-].*[2-4]|два|обидва|двоє|двійко|три|троє|чотири|один[\u{2013}\u{2014}-]два|два[\u{2013}\u{2014}-]три|три[\u{2013}\u{2014}-]чотири|двоє[\u{2013}\u{2014}-]троє|троє[\u{2013}\u{2014}-]четверо|півтор[аи])$").unwrap()
});

#[derive(Default)]
struct State {
    adj_pos: usize,
    adj_readings: Vec<AnalyzedToken>,
    adj_idx: Option<usize>,
}

impl State {
    fn reset(&mut self) {
        self.adj_readings.clear();
        self.adj_idx = None;
    }
    fn is_empty(&self) -> bool {
        self.adj_readings.is_empty()
    }
}

fn has(tr: &AnalyzedTokenReadings, pattern: &str) -> bool {
    uk_helpers::has_pos_tag_str2(tr, pattern)
}
fn has_re(tr: &AnalyzedTokenReadings, re: &Regex) -> bool {
    uk_helpers::has_pos_tag_re(tr, re)
}
fn has_part(tr: &AnalyzedTokenReadings, part: &str) -> bool {
    uk_helpers::has_reading_pos_tag_part(&tr.readings, part)
}
fn has_part_readings(readings: &[AnalyzedToken], part: &str) -> bool {
    uk_helpers::has_reading_pos_tag_part(readings, part)
}
fn has_start(tr: &AnalyzedTokenReadings, prefix: &str) -> bool {
    uk_helpers::has_reading_pos_tag_start(&tr.readings, prefix)
}
fn lemma(tr: &AnalyzedTokenReadings, list: &[&str]) -> bool {
    uk_helpers::has_lemma(&tr.readings, list)
}
fn lemma_re(tr: &AnalyzedTokenReadings, pattern: &str) -> bool {
    uk_helpers::has_lemma_regex(&tr.readings, &Regex::new(pattern).unwrap())
}
fn lemma_with(tr: &AnalyzedTokenReadings, list: &[&str], pos: &str) -> bool {
    uk_helpers::has_lemma_with_pattern(&tr.readings, list, &Regex::new(pos).unwrap())
}
fn up(tr: &AnalyzedTokenReadings) -> bool {
    tr.surface()
        .chars()
        .next()
        .is_some_and(|c| c.is_uppercase())
}
fn tok(tr: &AnalyzedTokenReadings) -> String {
    tr.surface().to_string()
}
fn tok_lower(tr: &AnalyzedTokenReadings) -> String {
    tr.surface().to_lowercase()
}
fn disjoint(a: &[Inflection], b: &[Inflection]) -> bool {
    a.iter().all(|x| !b.contains(x))
}

pub struct TokenAgreementAdjNounRule {
    synth: std::sync::Arc<UkrainianSynthesizer>,
    gov: std::sync::Arc<CaseGovernment>,
}

impl TokenAgreementAdjNounRule {
    pub fn new(
        synth: std::sync::Arc<UkrainianSynthesizer>,
        gov: std::sync::Arc<CaseGovernment>,
    ) -> Self {
        Self { synth, gov }
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
        let mut state = State::default();
        let mut i = 1usize;
        while i < n {
            let tr = view[i];
            let pos0 = tr.readings.first().and_then(|r| r.pos_tag.clone());
            if pos0.is_none() {
                state.reset();
                i += 1;
                continue;
            }
            if state.is_empty() {
                if i == n - 1 {
                    i += 1;
                    continue;
                }
            } else {
                let adv_and_not_prep = (has_part_readings_all_adv(tr)
                    || ["дуже", "небагато", "багато"].contains(&tr.surface()))
                    && !(i < n - 1
                        && has_start(tr, "prep")
                        && uk_helpers::has_vidm_pos_tag_token(
                            &self
                                .gov
                                .get_case_governments_opt(&tr.readings, Some("prep"), None),
                            view[i + 1],
                        ))
                    && has_part_readings(&state.adj_readings, "adjp");
                if adv_and_not_prep {
                    i += 1;
                    continue;
                }
            }

            if has_start(tr, "adj") {
                state.reset();
                if has_part(tr, ":nv") || lemma(tr, &["який", "котрий", "сам"]) || has_part(tr, "<")
                {
                    i += 1;
                    continue;
                }
                if lemma_with(tr, &["подібний"], ":n:") {
                    state.reset();
                    break;
                }
                for at in &tr.readings {
                    let Some(tag) = at.pos_tag.as_deref() else {
                        continue;
                    };
                    if tag.starts_with("adj") {
                        state.adj_pos = i;
                        state.adj_readings.push(at.clone());
                        state.adj_idx = Some(i);
                    } else if !(lemma_with(tr, &["другий"], "adj:f:")
                        || (i + 1 < n && lemma_with(view[i + 1], FAKE_FEM_LIST, "noun:inanim:m:")))
                        && !uk_helpers::is_predict_or_insert(at)
                    {
                        state.reset();
                        break;
                    }
                }
                i += 1;
                continue;
            }

            if state.is_empty() {
                i += 1;
                continue;
            }

            if has_part(tr, ":nv") || has_part(tr, "pron") {
                state.reset();
                i += 1;
                continue;
            }

            let mut noun_readings: Vec<AnalyzedToken> = Vec::new();
            for at in &tr.readings {
                let Some(tag) = at.pos_tag.as_deref() else {
                    continue;
                };
                if tag.starts_with("noun") {
                    noun_readings.push(at.clone());
                } else if tag == "SENT_END" || tag == "PARA_END" {
                    continue;
                } else if !uk_helpers::is_predict_or_insert(at) {
                    noun_readings.clear();
                    break;
                }
            }

            if noun_readings.is_empty() {
                state.reset();
                i += 1;
                continue;
            }

            let mut master = inflection::get_adj_inflections(&state.adj_readings);
            let mut slave = inflection::get_noun_inflections(
                &noun_readings,
                Some(&Regex::new("v_zna:var").unwrap()),
            );

            if disjoint(&master, &slave) {
                if is_exception(
                    &view,
                    state.adj_idx.unwrap(),
                    i,
                    &master,
                    &slave,
                    &state.adj_readings,
                    &noun_readings,
                    &self.gov,
                ) {
                    state.reset();
                    i += 1;
                    continue;
                }

                let adj_at = view[state.adj_idx.unwrap()];
                let mut msg = format!(
                    "Потенційна помилка: прикметник не узгоджений з іменником: \"{}\": [{}] і \"{}\": [{}]",
                    state.adj_readings.first().map(|r| r.token.clone()).unwrap_or_default(),
                    inflection::format_inflections(&mut master, true),
                    noun_readings.first().map(|r| r.token.clone()).unwrap_or_default(),
                    inflection::format_inflections(&mut slave, false),
                );

                if has_part_readings(&state.adj_readings, ":m:v_rod")
                    && Regex::new(r"^.*[ую]$")
                        .unwrap()
                        .is_match(tr.surface())
                        .unwrap_or(false)
                    && uk_helpers::has_reading_pos_tag(
                        &noun_readings,
                        &Regex::new(r"^noun.*?:m:v_dav.*$").unwrap(),
                    )
                {
                    msg.push_str(USED_U_INSTEAD_OF_A_MSG);
                } else if adj_at.surface().contains('-')
                    && Regex::new(r"^.*(?:[23]-є|[02-9]-а|[0-9]-м[иа])$")
                        .unwrap()
                        .is_match(adj_at.surface())
                        .unwrap_or(false)
                {
                    msg.push_str(
                        ". Можливо, вжито зайве літерне нарощення після кількісного числівника?",
                    );
                } else if adj_at.surface().starts_with("не")
                    && uk_helpers::has_reading_pos_tag(
                        &noun_readings,
                        &Regex::new(r"^noun.*?:v_oru.*$").unwrap(),
                    )
                {
                    msg.push_str(". Можливо, тут «не» потрібно написати окремо?");
                } else if !uk_helpers::has_reading_pos_tag(
                    &state.adj_readings,
                    &Regex::new(r"^adj.*?v_mis.*$").unwrap(),
                ) && uk_helpers::has_reading_pos_tag(
                    &noun_readings,
                    &Regex::new(r"^noun.*?v_mis.*$").unwrap(),
                ) {
                    msg.push_str(". Можливо, пропущено прийменник на/в/у...?");
                }

                let mut suggestions: Vec<String> = Vec::new();
                for adj_inf in &master {
                    let gender_tag = format!(":{}:", adj_inf.gender);
                    let vidm_tag = &adj_inf.case_;
                    if vidm_tag != "v_kly"
                        && (adj_inf.gender == "p" || has_part_readings(&noun_readings, &gender_tag))
                    {
                        for noun_token in &noun_readings {
                            let Some(noun_pos) = noun_token.pos_tag.as_deref() else {
                                continue;
                            };
                            if adj_inf.anim_matters()
                                && !noun_pos.contains(&format!(
                                    ":{}",
                                    adj_inf.anim_tag.as_deref().unwrap_or("")
                                ))
                            {
                                continue;
                            }
                            let new_tag = Regex::new(r":.:v_...")
                                .unwrap()
                                .replace(noun_pos, &format!("{gender_tag}{vidm_tag}"))
                                .into_owned();
                            for s in self.synth.synthesize(noun_token, &new_tag, false) {
                                let suggestion = format!("{} {s}", adj_at.surface());
                                if !suggestions.contains(&suggestion) {
                                    suggestions.push(suggestion);
                                }
                            }
                        }
                    }
                }
                for noun_inf in &slave {
                    let gender_tag = format!(":{}:", noun_inf.gender);
                    let mut vidm_tag = noun_inf.case_.clone();
                    if noun_inf.anim_matters() {
                        vidm_tag
                            .push_str(&format!(":r{}", noun_inf.anim_tag.as_deref().unwrap_or("")));
                    }
                    for adj_token in &state.adj_readings {
                        let Some(adj_pos) = adj_token.pos_tag.as_deref() else {
                            continue;
                        };
                        let new_tag = Regex::new(r":.:v_...(:r(in)?anim)?")
                            .unwrap()
                            .replace(adj_pos, &format!("{gender_tag}{vidm_tag}"))
                            .into_owned();
                        for s in self.synth.synthesize(adj_token, &new_tag, false) {
                            let suggestion = format!("{s} {}", tr.surface());
                            if !suggestions.contains(&suggestion) {
                                suggestions.push(suggestion);
                            }
                        }
                    }
                }
                if msg.contains("кількісного числівника") {
                    let sugg_num = Regex::new(r"[-–]м[аи]$")
                        .unwrap()
                        .replace(adj_at.surface(), "")
                        .into_owned()
                        + " "
                        + tr.surface();
                    suggestions.push(sugg_num);
                }

                out.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        msg,
                        Some(SHORT.to_string()),
                        TextRange::new(
                            sentence_offset + adj_at.start_pos,
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
                    .with_metadata(DESCRIPTION, "misspelling", 0),
                );
            }
            state.reset();
            i += 1;
        }
        out
    }
}

fn has_part_readings_all_adv(tr: &AnalyzedTokenReadings) -> bool {
    uk_helpers::has_pos_tag_part_all(&tr.readings, "adv")
}

#[allow(clippy::too_many_arguments)]
fn is_exception(
    tokens: &[&AnalyzedTokenReadings],
    adj_pos: usize,
    noun_pos: usize,
    master: &[Inflection],
    slave: &[Inflection],
    adj_readings: &[AnalyzedToken],
    noun_readings: &[AnalyzedToken],
    gov: &CaseGovernment,
) -> bool {
    let adj_at = tokens[adj_pos];
    let noun_at = tokens[noun_pos];
    let n = tokens.len();

    let case_gov_adv = |pos: usize| -> Vec<String> {
        gov.get_case_governments_opt(&tokens[pos].readings, Some("adv"), None)
    };

    // схований всередині номера
    if noun_pos - adj_pos > 1 {
        let cases = case_gov_adv(adj_pos + 1);
        if !cases.is_empty() && uk_helpers::has_vidm_pos_tag_token(&cases, noun_at) {
            return true;
        }
    }
    if lemma_with(adj_at, &["інститутський"], "adj:f:.*")
        && Regex::new(r"^І.*$")
            .unwrap()
            .is_match(adj_at.surface())
            .unwrap_or(false)
    {
        return true;
    }
    if adj_pos > 1 && tokens[adj_pos - 1].surface() == "більшості" && adj_at.surface() == "своїй"
    {
        return true;
    }
    if adj_pos > 1
        && lt_tagger::uk_helpers::is_capitalized(adj_at.surface())
        && lt_tagger::uk_helpers::is_capitalized(tokens[adj_pos - 1].surface())
        && (lemma(adj_at, &["вітчизняний"]) || lemma(adj_at, &["житомирський"]))
        && lemma(tokens[adj_pos - 1], &["великий"])
        && !lemma(noun_at, &["війна"])
    {
        return true;
    }
    if adj_pos > 1
        && lemma(adj_at, &["національний"])
        && lemma(tokens[adj_pos - 1], &["перший"])
        && up(adj_at)
        && up(tokens[adj_pos - 1])
    {
        return true;
    }
    if adj_at.surface().eq_ignore_ascii_case("голому")
        && noun_at.surface().eq_ignore_ascii_case("сорочка")
    {
        return true;
    }
    if adj_pos > 1
        && adj_at.surface().eq_ignore_ascii_case("доброго")
        && Regex::new(r"^(?:ні)?чого$")
            .unwrap()
            .is_match(tokens[adj_pos - 1].surface())
            .unwrap_or(false)
    {
        return true;
    }
    if adj_pos > 1
        && Regex::new(r"(?i)^(?:середньому|цілому|основному|подальшому)$")
            .unwrap()
            .is_match(adj_at.surface())
            .unwrap_or(false)
        && Regex::new(r"(?i)^[ву]$")
            .unwrap()
            .is_match(tokens[adj_pos - 1].surface())
            .unwrap_or(false)
    {
        return true;
    }
    if lemma_with(adj_at, &["бережений"], "adj:m:v_rod.*")
        && lemma_with(noun_at, &["бог"], "noun:anim:m:v_naz.*")
    {
        return true;
    }
    if lemma_with(adj_at, &["кожний"], "adj:f:v_naz.*")
        && lemma_with(
            noun_at,
            &[
                "вага",
                "маса",
                "вартість",
                "потужність",
                "тривалість",
                "чисельність",
                "номінал",
                "наклад",
            ],
            "noun:inanim:.:v_oru.*",
        )
    {
        return true;
    }
    if lemma(adj_at, &["божий", "господній", "Христовий"]) && up(adj_at) {
        return true;
    }
    if adj_pos > 1
        && has_part(tokens[adj_pos - 1], "noun")
        && up(adj_at)
        && !disjoint(
            master,
            &inflection::get_noun_inflections(&tokens[adj_pos - 1].readings, None),
        )
    {
        return true;
    }
    if Regex::new(r"^[1-9]|1[0-2][\u{2018}-][а-д]$")
        .unwrap()
        .is_match(adj_at.surface())
        .unwrap_or(false)
        && lemma(noun_at, &["клас"])
    {
        return true;
    }
    if noun_pos > 1
        && lt_tagger::uk_helpers::has_lemma(&adj_at.readings, &["перший"])
        && !lemma_with(noun_at, FAKE_FEM_LIST, "noun:inanim:m:")
    {
        return true;
    }
    if adj_pos > 1
        && noun_pos < n - 1
        && (has_re(
            adj_at,
            &Regex::new(r"^adj:[mf]:.*numr.*|number.*$").unwrap(),
        ) || uk_helpers::has_reading_pos_tag(
            noun_readings,
            &Regex::new(r"^noun:inanim:.:v_rod.*$").unwrap(),
        ))
        && uk_helpers::has_reading_pos_tag(
            noun_readings,
            &Regex::new(r"^noun:inanim:.:v_rod.*$").unwrap(),
        )
        && lemma(
            tokens[adj_pos - 1],
            &["абзац", "розділ", "пункт", "підпункт", "частина", "стаття"],
        )
    {
        return true;
    }
    if adj_pos > 1
        && has_part(adj_at, "num")
        && lemma(tokens[adj_pos - 1], &["стаття"])
        && !disjoint(
            master,
            &inflection::get_noun_inflections(&tokens[adj_pos - 1].readings, None),
        )
    {
        return true;
    }
    if adj_pos > 1 && adj_at.surface() == "запасних" && lemma(tokens[adj_pos - 1], &["лава"])
    {
        return true;
    }
    if ["зміни", "групи"].contains(&noun_at.surface()) && lemma(adj_at, &["старший"])
    {
        return true;
    }
    if adj_pos > 1
        && adj_at.surface() == "повну"
        && tokens[adj_pos - 1].surface().eq_ignore_ascii_case("на")
    {
        return true;
    }
    if adj_pos > 1
        && lemma_with(adj_at, &["світовий"], ":f:")
        && lemma_with(tokens[adj_pos - 1], &["другий", "перший"], ":f:")
    {
        return true;
    }
    if noun_pos > 1
        && lemma(
            tokens[noun_pos - 1],
            &["увечері", "уранці", "ввечері", "вранці"],
        )
        && has(noun_at, "noun.*v_rod.*")
    {
        return true;
    }
    if has_part(adj_at, "pron") && noun_at.surface().eq_ignore_ascii_case("богом") {
        return true;
    }
    if lemma(adj_at, &["той"])
        && ["родом", "кулею", "розміром"].contains(&tok_lower(noun_at).as_str())
    {
        return true;
    }
    if ["таке", "такого"].contains(&tok_lower(adj_at).as_str())
        && has(noun_at, "noun.*:v_naz.*")
        && SearchMatch::new()
            .target(vec![Condition::postag("verb.*")])
            .limit(2)
            .skip(vec![Condition::postag("(part|adv).*")])
            .m_after(tokens, noun_pos + 1)
            .is_some_and(|x| x > 0)
    {
        return true;
    }
    if noun_pos < n - 1
        && tok_lower(adj_at) == "той"
        && has(noun_at, "noun.*:v_(zna|oru).*")
        && has_start(tokens[noun_pos + 1], "verb")
    {
        return true;
    }
    if adj_pos > 1
        && adj_at.surface() == "таке"
        && uk_helpers::rev_search(
            tokens,
            adj_pos as i64 - 1,
            Some(&Regex::new("що").unwrap()),
            None,
        )
    {
        return true;
    }
    if adj_at.surface().eq_ignore_ascii_case("таких")
        && (has_part(noun_at, ":p:v_naz")
            || ["меншість", "більшість"].contains(&tok_lower(noun_at).as_str()))
    {
        return true;
    }
    if adj_pos > 1
        && adj_at.surface() == "рівних"
        && tokens[adj_pos - 1].surface().eq_ignore_ascii_case("на")
    {
        return true;
    }
    if noun_pos < n - 1 && noun_at.surface() == "зразка" {
        return true;
    }
    if ["мінус", "плюс"].contains(&noun_at.surface()) {
        return true;
    }
    if noun_pos < n - 1
        && lemma(
            noun_at,
            &[
                "пара",
                "низка",
                "ряд",
                "купа",
                "більшість",
                "десятка",
                "сотня",
                "тисяча",
                "мільйон",
            ],
        )
        && (has(noun_at_plus(tokens, noun_pos, 1), "noun.*?:p:v_rod.*")
            || (noun_pos < n - 2
                && has(noun_at_plus(tokens, noun_pos, 1), "adj:p:v_rod.*")
                && has(noun_at_plus(tokens, noun_pos, 2), "noun.*?:p:v_rod.*")))
    {
        return true;
    }
    if noun_pos < n - 1
        && lemma_with(noun_at, &["раз"], ".*p:v_(naz|rod).*")
        && (has_re(noun_at_plus(tokens, noun_pos, 1), &NUMBER_V_NAZ)
            || has_part(noun_at_plus(tokens, noun_pos, 1), "prep"))
    {
        return true;
    }
    if noun_pos < n - 1
        && lemma_with(
            noun_at,
            uk_helpers::TIME_PLUS_LEMMAS,
            "noun.*?p:v_(naz|rod).*",
        )
        && (has_re(noun_at_plus(tokens, noun_pos, 1), &NUMBER_V_NAZ)
            || (noun_pos < n - 2
                && uk_helpers::has_lemma_with_pattern(
                    &noun_at_plus(tokens, noun_pos, 1).readings,
                    &["на", "за", "з", "із", "зо", "через", "під"],
                    &Regex::new("^prep$").unwrap(),
                )
                && has_re(noun_at_plus(tokens, noun_pos, 2), &NUMBER_V_NAZ)))
    {
        return true;
    }
    if noun_pos < n - 2
        && lemma_with(noun_at, &["особа"], "noun.*?p:v_(naz|rod).*")
        && lemma_with(
            noun_at_plus(tokens, noun_pos, 1),
            &["на", "з", "із", "зо", "під"],
            "prep",
        )
        && has_re(noun_at_plus(tokens, noun_pos, 2), &NUMBER_V_NAZ)
    {
        return true;
    }
    if adj_pos > 2
        && lemma(
            tokens[adj_pos - 2],
            &["секунда", "хвилина", "година", "рік"],
        )
        && has_start(tokens[adj_pos - 1], "prep")
        && has_part(adj_at, "num")
    {
        let cases = gov.get_case_governments_opt(&tokens[adj_pos - 1].readings, Some("prep"), None);
        if uk_helpers::has_vidm_pos_tag_token(&cases, tokens[adj_pos - 2])
            && uk_helpers::has_vidm_pos_tag_token(&cases, adj_at)
        {
            return true;
        }
    }
    if noun_pos < n - 1
        && uk_helpers::has_lemma(&noun_at.readings, uk_helpers::TIME_LEMMAS)
        && lemma(tokens[noun_pos + 1], &["тому"])
    {
        return true;
    }
    if noun_pos < n - 1
        && lemma_with(
            noun_at,
            uk_helpers::TIME_PLUS_LEMMAS,
            "noun:inanim:p:v_oru.*",
        )
    {
        return true;
    }
    if lemma(
        adj_at,
        &[
            "десятий",
            "сотий",
            "тисячний",
            "десятитисячний",
            "стотитисячний",
            "мільйонний",
            "мільярдний",
        ],
    ) && has(adj_at, ".*:[fp]:.*")
        && has(noun_at, "noun.*v_rod.*")
    {
        return true;
    }
    if adj_pos > 1
        && has(adj_at, ".*:p:v_(rod|naz).*")
        && uk_helpers::reverse_search(tokens, adj_pos as i64 - 1, 5, Some(&DOVYE_TROYE), None)
        && (has(noun_at, ".*(:p:v_naz|:n:v_rod).*")
            || ["імені", "ока"].contains(&noun_at.surface()))
    {
        return true;
    }
    if (Regex::new(r"^[0-9]+[\u{2014}\u{2013}-][0-9]+[\u{2013}-][а-яіїєґ]{1,3}$")
        .unwrap()
        .is_match(adj_at.surface())
        .unwrap_or(false)
        || (Regex::new(r"^.*[а-яїієґ][\u{2014}\u{2013}-].*$")
            .unwrap()
            .is_match(adj_at.surface())
            .unwrap_or(false)
            && has_part(adj_at, "numr")))
        && has_part_readings(noun_readings, ":p:")
        && has_overlap_ignore_gender(master, slave, None, None)
    {
        return true;
    }
    if noun_pos > 2
        && ["\u{2013}", "\u{2014}"].contains(&tokens[adj_pos - 1].surface())
        && has_part(adj_at, "num")
        && has_part(tokens[adj_pos - 2], "num")
        && has_part_readings(noun_readings, ":p:")
        && (has_start(tokens[adj_pos - 2], "number")
            || has_overlap_ignore_gender(
                &inflection::get_adj_inflections(&tokens[adj_pos - 2].readings),
                slave,
                None,
                None,
            ))
        && has_overlap_ignore_gender(master, slave, None, None)
    {
        return true;
    }
    if has(adj_at, "adj.*:p:.*")
        && Regex::new(r"^.*[\u{2014}\u{2013}-].*$")
            .unwrap()
            .is_match(noun_at.surface())
            .unwrap_or(false)
        && (noun_at
            .readings
            .first()
            .and_then(|r| r.stem.as_deref())
            .is_some_and(|l| {
                let first = l.split(['\u{2014}', '\u{2013}', '-']).next().unwrap_or("");
                uk_helpers::TIME_PLUS_LEMMAS.contains(&first)
            })
            || has_overlap_ignore_gender(master, slave, None, None))
    {
        return true;
    }
    if noun_pos < n - 1
        && lemma(noun_at, &["пара"])
        && has(adj_at, "adj.*:p:.*")
        && has(noun_at_plus(tokens, noun_pos, 1), ".*:p:v_rod.*")
    {
        return true;
    }
    if noun_pos > 1 && has_part(tokens[adj_pos - 1], "num") && has(adj_at, "adj.*num.*") {
        if has(tokens[adj_pos - 1], "(noun|numr).*") && has(adj_at, "adj:p:v_rod.*") {
            if lemma(adj_at, &["другий"]) && !lemma(tokens[adj_pos - 1], &["один"]) {
                return false;
            }
            return true;
        }
        if lemma_with(tokens[adj_pos - 1], &["один"], "numr:f:.*")
            && !disjoint(
                &inflection::get_adj_inflections_start(&tokens[adj_pos - 1].readings, "numr"),
                &inflection::get_adj_inflections(&adj_at.readings),
            )
        {
            return true;
        }
    }
    if noun_pos > 3
        && tokens[adj_pos - 1].surface() == "/"
        && has_part(tokens[adj_pos - 2], "numb")
        && has_overlap_ignore_gender(master, slave, None, None)
    {
        return true;
    }
    if has_part(adj_at, ":numr") {
        let adj_token = tok(adj_at);
        if Regex::new(r"^([12][0-9])?[0-9][0-9][\u{2014}\u{2013}-](?:й|го|м|му)$").unwrap().is_match(&adj_token).unwrap_or(false)
            || Regex::new(r"^([12][0-9])?[0-9]0[\u{2014}\u{2013}-](?:ті|тих|их|х)$").unwrap().is_match(&adj_token).unwrap_or(false)
            || Regex::new(r"^([12][0-9])?[0-9][0-9][\u{2014}\u{2013}-](?:[12][0-9])?[0-9][0-9][\u{2014}\u{2013}-](?:й|го|м|му|ті|тих|их|х)$").unwrap().is_match(&adj_token).unwrap_or(false)
        {
            return true;
        }
        if noun_pos > 1
            && has_part(adj_at, ":f:")
            && lemma(
                tokens[adj_pos - 1],
                &[
                    "на",
                    "в",
                    "у",
                    "за",
                    "о",
                    "до",
                    "після",
                    "близько",
                    "раніше",
                ],
            )
            && !lemma(noun_at, &["хвилина", "година"])
        {
            return true;
        }
        if has_part(adj_at, ":f:")
            && Regex::new(r"^(?:ранку|дня|вечора|ночі|пополудня)$")
                .unwrap()
                .is_match(noun_at.surface())
                .unwrap_or(false)
        {
            return true;
        }
        if has_part(adj_at, ":n:")
            && uk_helpers::has_lemma_with_pattern(
                &noun_at.readings,
                uk_helpers::MONTH_LEMMAS,
                &Regex::new(r"^v_rod$").unwrap(),
            )
        {
            return true;
        }
    }
    if has(adj_at, ".*?adjp:actv.*:bad.*") {
        return true;
    }
    if noun_pos > 2
        && noun_pos < n
        && lemma(tokens[adj_pos - 1], &["ніщо", "щось", "ніхто", "хтось"])
        && !disjoint(
            &inflection::get_noun_inflections(&tokens[adj_pos - 1].readings, None),
            master,
        )
    {
        return true;
    }
    if adj_pos > 1
        && uk_helpers::rev_search(
            tokens,
            adj_pos as i64 - 1,
            Some(&Regex::new(".*(ння|ття)").unwrap()),
            None,
        )
        && has(adj_at, "adj.*:v_oru.*")
        && has(noun_at, "noun:.*:v_rod.*")
        && gender_matches(master, slave, Some("v_oru"), Some("v_rod"))
    {
        return true;
    }
    let verb_pos = uk_helpers::rev_search_idx(
        tokens,
        adj_pos as i64 - 1,
        Some(&Regex::new("бути|ставати|стати|залишатися|залишитися").unwrap()),
        None,
    );
    if verb_pos != -1 {
        if has(adj_at, "adj.*v_naz.*adjp:pasv.*") {
            if gender_matches(master, slave, Some("v_naz"), Some("v_naz")) {
                return true;
            }
        } else if has(adj_at, "adj.*v_oru.*") {
            if uk_helpers::has_reading_pos_tag(
                noun_readings,
                &Regex::new(r"^noun.*v_naz.*$").unwrap(),
            ) {
                if gender_matches(master, slave, Some("v_oru"), Some("v_naz")) {
                    if has_part(tokens[verb_pos as usize], ":inf")
                        || crate::uk::verb_inflection::inflections_overlap(
                            &tokens[verb_pos as usize].readings,
                            &tokens[noun_pos].readings,
                        )
                    {
                        return true;
                    }
                } else if noun_pos < n - 1
                    && has_part(adj_at, "adj:p:")
                    && CONJ_FOR_PLURAL_WITH_COMMA
                        .contains(&tok_lower(tokens[noun_pos + 1]).as_str())
                {
                    return true;
                }
            } else if uk_helpers::has_reading_pos_tag(
                noun_readings,
                &Regex::new(r"^noun.*v_dav.*$").unwrap(),
            ) && gender_matches(master, slave, Some("v_oru"), Some("v_dav"))
            {
                return true;
            }
        }
    }
    let verb_pos2 = uk_helpers::rev_search_idx(tokens, adj_pos as i64 - 1, None, Some("verb.*"));
    if verb_pos2 != -1
        && has(adj_at, "adj.*v_oru.*")
        && uk_helpers::has_reading_pos_tag(noun_readings, &Regex::new(r"^noun.*v_naz.*$").unwrap())
        && crate::uk::verb_inflection::inflections_overlap(
            &tokens[verb_pos2 as usize].readings,
            &tokens[noun_pos].readings,
        )
    {
        return true;
    }
    if adj_pos > 2
        && [
            "біле",
            "чорне",
            "оранжеве",
            "червоне",
            "жовте",
            "синє",
            "зелене",
            "фіолетове",
        ]
        .contains(&adj_at.surface())
        && ["в", "у"].contains(&tokens[adj_pos - 1].surface())
        && has_part(tokens[adj_pos - 2], "adjp:pasv")
        && !disjoint(
            &inflection::get_adj_inflections(&tokens[adj_pos - 2].readings),
            slave,
        )
    {
        return true;
    }
    if adj_pos > 3
        && ["біле", "чорне"].contains(&adj_at.surface())
        && ["усе", "все"].contains(&tokens[adj_pos - 1].surface())
        && ["в", "у"].contains(&tokens[adj_pos - 2].surface())
        && has_part(tokens[adj_pos - 3], "adjp:pasv")
        && !disjoint(
            &inflection::get_adj_inflections(&tokens[adj_pos - 3].readings),
            slave,
        )
    {
        return true;
    }
    if noun_pos < n - 1
        && has_part(adj_at, "adjp:pasv")
        && ["тисячу", "сотню", "десятки"].contains(&noun_at.surface())
        && ["разів", "раз", "років"].contains(&tokens[noun_pos + 1].surface())
    {
        return true;
    }
    if noun_at.surface().eq_ignore_ascii_case("раз")
        && tokens[noun_pos - 1].surface().eq_ignore_ascii_case("ще")
    {
        return true;
    }
    if adj_pos > 2 {
        if has(adj_at, "adj.*v_oru.*")
            && lemma(tokens[adj_pos - 2], &["порівняно", "аналогічно"])
            && lemma_re(tokens[adj_pos - 1], "з|із|зі")
        {
            return true;
        }
        if has_part(tokens[adj_pos - 1], "prep")
            && has(tokens[adj_pos - 2], "(adj|verb|part|noun|adv).*")
        {
            let cases =
                gov.get_case_governments_opt(&tokens[adj_pos - 1].readings, Some("prep"), None);
            if uk_helpers::has_vidm_pos_tag_token(&cases, adj_at) {
                if ((has_start(tokens[adj_pos - 2], "verb")
                    || lemma(tokens[adj_pos - 2], &["би", "б"]))
                    || ["поряд", "відміну", "порівнянні"]
                        .contains(&tok_lower(tokens[adj_pos - 2]).as_str()))
                    && has(noun_at, "noun.*v_(naz|zna|oru).*")
                {
                    return true;
                }
                if !disjoint(
                    &inflection::get_adj_inflections(&tokens[adj_pos - 2].readings),
                    slave,
                ) {
                    return true;
                }
                if noun_pos < n - 1
                    && has_part(adj_at, "adj:p:")
                    && CONJ_FOR_PLURAL_WITH_COMMA
                        .contains(&tok_lower(tokens[noun_pos + 1]).as_str())
                    && has_part(tokens[adj_pos - 2], "adj:p:")
                    && has_overlap_ignore_gender(
                        &inflection::get_adj_inflections(&tokens[adj_pos - 2].readings),
                        slave,
                        Some("p"),
                        None,
                    )
                {
                    return true;
                }
            }
        }
    }
    if adj_pos > 1
        && has_part(tokens[adj_pos - 1], "adjp:pasv")
        && has(adj_at, "adj.*v_oru.*")
        && !disjoint(
            &inflection::get_adj_inflections(&tokens[adj_pos - 1].readings),
            slave,
        )
    {
        return true;
    }
    if has_part(adj_at, "adjp:pasv") && has_part_readings(noun_readings, "v_oru") {
        return true;
    }
    if !has(tokens[adj_pos - 1], ".*adjp:pasv.*|prep.*")
        && has(adj_at, "adj.*v_oru.*")
        && uk_helpers::has_reading_pos_tag(
            noun_readings,
            &Regex::new(r"^noun.*v_(zna|naz).*$").unwrap(),
        )
    {
        let v_pos = uk_helpers::token_search(
            tokens,
            noun_pos as i64 + 1,
            Some("verb"),
            None,
            None,
            Dir::Forward,
        );
        if v_pos > 0
            && v_pos <= noun_pos as i64 + 5
            && (uk_helpers::has_reading_pos_tag(
                noun_readings,
                &Regex::new(r"^noun.*v_naz.*$").unwrap(),
            ) || (gov.has_case_government(&tokens[v_pos as usize].readings, None, "v_zna")
                && gender_matches(master, slave, Some("v_oru"), Some("v_zna"))))
        {
            return true;
        }
    }
    if adj_pos > 1
        && has(adj_at, "adj:.:v_oru.*")
        && uk_helpers::has_reading_pos_tag(noun_readings, &Regex::new(r"^.*v_zna.*$").unwrap())
        && gender_matches(master, slave, Some("v_oru"), Some("v_zna"))
    {
        let v_pos = uk_helpers::token_search(
            tokens,
            adj_pos as i64 - 1,
            None,
            None,
            Some(&VERB_ADVP),
            Dir::Reverse,
        );
        if v_pos > 0
            && v_pos >= adj_pos as i64 - 3
            && gov.has_case_government(&tokens[v_pos as usize].readings, None, "v_oru")
            && gov.has_case_government(&tokens[v_pos as usize].readings, None, "v_zna")
        {
            return true;
        }
    }
    if adj_pos > 2
        && has(tokens[adj_pos - 1], "adv(?!p).*")
        && gov
            .get_case_governments_opt(&tokens[adj_pos - 2].readings, None, Some(&VERB_ADVP))
            .iter()
            .any(|c| c == "v_oru")
        && has_part(adj_at, "v_oru")
        && uk_helpers::has_reading_pos_tag(noun_readings, &Regex::new(r"^.*v_zna.*$").unwrap())
        && gender_matches(master, slave, Some("v_oru"), Some("v_zna"))
    {
        return true;
    }
    if case_government_matches(adj_readings, slave, gov) {
        if noun_pos < n - 1 && has_part(tokens[noun_pos + 1], "noun:") {
            if has(tokens[noun_pos + 1], "noun.*v_(rod|oru|naz|dav).*") {
                return true;
            }
            let slave2 = inflection::get_noun_inflections(&tokens[noun_pos + 1].readings, None);
            if !disjoint(master, &slave2) {
                return true;
            }
        } else {
            return true;
        }
    }
    if adj_pos > 1
        && has_part(tokens[adj_pos - 1], "adj")
        && case_government_matches(&tokens[adj_pos - 1].readings, master, gov)
    {
        let pre_adj = inflection::get_adj_inflections(&tokens[adj_pos - 1].readings);
        if !disjoint(&pre_adj, slave) {
            return true;
        }
    }
    false
}

fn noun_at_plus<'a>(
    tokens: &[&'a AnalyzedTokenReadings],
    pos: usize,
    offset: usize,
) -> &'a AnalyzedTokenReadings {
    tokens[pos + offset]
}

fn gender_matches(
    master: &[Inflection],
    slave: &[Inflection],
    master_case_filter: Option<&str>,
    slave_case_filter: Option<&str>,
) -> bool {
    for m in master {
        for s in slave {
            if master_case_filter.is_none_or(|c| m.case_ == c)
                && slave_case_filter.is_none_or(|c| s.case_ == c)
                && s.gender == m.gender
            {
                return true;
            }
        }
    }
    false
}

fn case_government_matches(
    adj_readings: &[AnalyzedToken],
    slave: &[Inflection],
    gov: &CaseGovernment,
) -> bool {
    let map = gov.case_map();
    let mut seen = Vec::new();
    for at in adj_readings {
        let Some(lemma) = at.stem.as_deref() else {
            continue;
        };
        if seen.contains(&lemma) {
            continue;
        }
        seen.push(lemma);
        if let Some(inflections) = map.get(lemma) {
            for inf in slave {
                if inflections.contains(&inf.case_) {
                    return true;
                }
            }
        }
    }
    false
}

fn has_overlap_ignore_gender(
    master: &[Inflection],
    slave: &[Inflection],
    master_gender_filter: Option<&str>,
    slave_gender_filter: Option<&str>,
) -> bool {
    for m in master {
        if master_gender_filter.is_some_and(|f| !m.gender.eq_ignore_ascii_case(f)) {
            continue;
        }
        for s in slave {
            if slave_gender_filter.is_some_and(|f| !s.gender.eq_ignore_ascii_case(f)) {
                continue;
            }
            if m.equals_ignore_gender(s) {
                return true;
            }
        }
    }
    false
}
