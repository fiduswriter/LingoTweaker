//! Port of `org.languagetool.rules.uk.TokenAgreementNounVerbRule`
//! (`UK_NOUN_VERB_INFLECTION_AGREEMENT`) and
//! `TokenAgreementNounVerbExceptionHelper`.

use std::collections::HashSet;
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Suggestion, TextRange};
use lt_tagger::uk_helpers::{self, Dir};

use crate::uk::gov::CaseGovernment;
use crate::uk::inflection;
use crate::uk::search_helper::{Condition, Match as SearchMatch};
use crate::uk::verb_inflection::{self, VerbInflection};

pub const RULE_ID: &str = "UK_NOUN_VERB_INFLECTION_AGREEMENT";
const DESCRIPTION: &str = "Узгодження іменника та дієслова за родом, числом та особою";
const SHORT: &str = "Узгодження іменника з дієсловом";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Різне";

static INF_ARGREEMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:не)?(?:здатний|змушений|з?г[іо]дний|зобов'язаний|повинний|готовий|достойний|покликаний|спроможний|радий|налаштований|зацікавлений|повинно|змога|стан|можна)$").unwrap()
});
static GEO_QUALIFIERS: &[&str] = &[
    "село",
    "селище",
    "місто",
    "містечко",
    "хутір",
    "республіка",
    "держава",
    "гора",
    "планета",
    "мікрорайон",
    "райцентр",
    "заповідник",
    "мис",
    "острів",
    "м.",
    "с.",
    "п.",
    "штат",
    "округ",
    "графство",
    "вірус",
    "ураган",
];

pub struct State {
    noun_pos: usize,
    noun_readings: Vec<AnalyzedToken>,
    noun_idx: Option<usize>,
    adj_readings: Vec<AnalyzedToken>,
}

impl State {
    fn new() -> Self {
        Self {
            noun_pos: 0,
            noun_readings: Vec::new(),
            noun_idx: None,
            adj_readings: Vec::new(),
        }
    }
}

pub struct TokenAgreementNounVerbRule {
    gov: std::sync::Arc<CaseGovernment>,
    masc_fem: HashSet<String>,
}

impl TokenAgreementNounVerbRule {
    pub fn new(gov: std::sync::Arc<CaseGovernment>, masc_fem: HashSet<String>) -> Self {
        Self { gov, masc_fem }
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
            let clean = tr.surface().to_string();
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
                &Regex::new(r"^noun.*:v_naz.*$").unwrap(),
            ) || clean == "яка"
            {
                let mut st = State::new();
                let mut restart = false;
                for at in &tr.readings {
                    let Some(tag) = at.pos_tag.as_deref() else {
                        continue;
                    };
                    if at.stem.as_deref() == Some("який") && tag.contains(":f:v_naz") {
                        st.noun_pos = i;
                        st.noun_readings.push(at.clone());
                        st.noun_idx = Some(i);
                    } else if (clean.eq_ignore_ascii_case("хто")
                        && uk_helpers::token_search_re(
                            &view,
                            i as i64 + 1,
                            None,
                            Some(&Regex::new(r"^verb.*:[fp]\b.*$").unwrap()),
                            Some(&Regex::new(r"^part$").unwrap()),
                            Dir::Forward,
                        ) > 0)
                        || (i < n - 1
                            && clean.eq_ignore_ascii_case("хто")
                            && uk_helpers::has_pos_tag_all(
                                &view[i + 1].readings,
                                &Regex::new(r"^adv.*$").unwrap(),
                            )
                            && uk_helpers::token_search_re(
                                &view,
                                i as i64 + 2,
                                None,
                                Some(&Regex::new(r"^verb.*:[fp]\b.*$").unwrap()),
                                Some(&Regex::new(r"^part$").unwrap()),
                                Dir::Forward,
                            ) > 0)
                    {
                        restart = true;
                        break;
                    } else if tag.starts_with("noun") && tag.contains("v_naz") {
                        st.noun_pos = i;
                        st.noun_readings.push(at.clone());
                        st.noun_idx = Some(i);
                    } else if (tag.starts_with("noun") && tag.contains("v_kly"))
                        || uk_helpers::is_predict_or_insert(at)
                    {
                        // ignore
                    } else if (Regex::new(r"^adj:.:(?:v_naz|v_kly).*$")
                        .unwrap()
                        .is_match(tag)
                        .unwrap_or(false)
                        || (tag.starts_with("adj:m:v_zna:rinanim")
                            && !uk_helpers::has_reading_pos_tag_start(
                                &view[i - 1].readings,
                                "prep",
                            )))
                        && !["кожен", "інший", "старий", "черговий"]
                            .contains(&at.token.to_lowercase().as_str())
                    {
                        st.adj_readings.push(at.clone());
                    } else {
                        restart = true;
                        break;
                    }
                }
                if restart {
                    state = None;
                    i += 1;
                    continue;
                }
                state = Some(st);
                i += 1;
                continue;
            }

            let Some(st) = state.take() else {
                i += 1;
                continue;
            };

            if ["не", "б", "би", "бодай"].contains(&tr.surface()) {
                state = Some(st);
                i += 1;
                continue;
            }
            if uk_helpers::has_pos_tag_part_all(&tr.readings, "adv") {
                state = Some(st);
                i += 1;
                continue;
            }

            let mut verb_readings: Vec<AnalyzedToken> = Vec::new();
            let mut restart = false;
            for at in &tr.readings {
                let Some(tag) = at.pos_tag.as_deref() else {
                    continue;
                };
                if tag == "SENT_END" || tag == "PARA_END" {
                    continue;
                }
                if tag.starts_with('<') {
                    restart = true;
                    break;
                }
                if tag.starts_with("verb") {
                    verb_readings.push(at.clone());
                } else if uk_helpers::is_predict_or_insert(at) {
                    // ignore
                } else {
                    restart = true;
                    break;
                }
            }
            if restart || verb_readings.is_empty() {
                i += 1;
                continue;
            }

            let master = verb_inflection::get_noun_inflections_v(&st.noun_readings);
            let slave = verb_inflection::get_verb_inflections(&verb_readings);
            if master.iter().all(|m| !slave.contains(m)) {
                if is_exception(
                    &view,
                    st.noun_pos,
                    i,
                    &master,
                    &slave,
                    &st.noun_readings,
                    &verb_readings,
                    &self.gov,
                    &self.masc_fem,
                ) {
                    break;
                }
                let noun_at = view[st.noun_idx.unwrap()];
                let msg = format!(
                    "Не узгоджено {} з дієсловом: \"{}\" ({}) і \"{}\" ({})",
                    if uk_helpers::has_lemma(&st.noun_readings, &["який"]) {
                        "займенник"
                    } else {
                        "іменник"
                    },
                    st.noun_readings
                        .first()
                        .map(|r| r.token.clone())
                        .unwrap_or_default(),
                    format_inflections(&master, true),
                    verb_readings
                        .first()
                        .map(|r| r.token.clone())
                        .unwrap_or_default(),
                    format_inflections(&slave, false),
                );
                out.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        msg,
                        Some(SHORT.to_string()),
                        TextRange::new(
                            sentence_offset + noun_at.start_pos,
                            sentence_offset + tr.end_pos(),
                        ),
                        Vec::<Suggestion>::new(),
                        CATEGORY_ID,
                        CATEGORY_NAME,
                    )
                    .with_metadata(DESCRIPTION, "misspelling", 0),
                );
            }
            i += 1;
        }
        out
    }
}

fn format_inflections(inflections: &[VerbInflection], _noun: bool) -> String {
    let mut sorted = inflections.to_vec();
    sorted.sort_by_key(|inf| lt_tagger::uk_helpers::gen_order(inf.gender.as_deref().unwrap_or("")));
    let mut list: Vec<String> = Vec::new();
    for inf in sorted {
        let mut s = String::new();
        if let Some(g) = inf.gender.as_deref() {
            s = lt_tagger::uk_helpers::gender_name(g);
        } else {
            if let Some(p) = inf.person.as_deref() {
                s = lt_tagger::uk_helpers::person_name(p);
            }
            if !inf.plural.is_empty() {
                if !s.is_empty() {
                    s.push(' ');
                }
                s += &lt_tagger::uk_helpers::gender_name(&inf.plural);
            }
        }
        list.push(s);
    }
    let mut unique: Vec<String> = Vec::new();
    for s in list {
        if !unique.contains(&s) {
            unique.push(s);
        }
    }
    unique.join(", ")
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
fn has_start(tr: &AnalyzedTokenReadings, p: &str) -> bool {
    uk_helpers::has_reading_pos_tag_start(&tr.readings, p)
}
fn lemma(tr: &AnalyzedTokenReadings, l: &[&str]) -> bool {
    uk_helpers::has_lemma(&tr.readings, l)
}
fn lemma_with(tr: &AnalyzedTokenReadings, l: &[&str], p: &str) -> bool {
    uk_helpers::has_lemma_with_pattern(&tr.readings, l, &Regex::new(p).unwrap())
}
fn disjoint_inf(a: &[VerbInflection], b: &[VerbInflection]) -> bool {
    a.iter().all(|x| !b.contains(x))
}

fn disjoint_inf2(a: &[inflection::Inflection], b: &[inflection::Inflection]) -> bool {
    a.iter().all(|x| !b.contains(x))
}

fn is_non_plural_a(tokens: &[&AnalyzedTokenReadings], pos: usize) -> bool {
    (tokens[pos].surface() == "а" || tokens[pos].surface() == "a")
        && !uk_helpers::has_lemma(&tokens[pos + 1].readings, &["також", "потім", "пізніше"])
}

#[allow(clippy::too_many_arguments)]
fn is_exception(
    tokens: &[&AnalyzedTokenReadings],
    noun_pos: usize,
    verb_pos: usize,
    _noun_inflections: &[VerbInflection],
    verb_inflections: &[VerbInflection],
    noun_readings: &[AnalyzedToken],
    verb_readings: &[AnalyzedToken],
    gov: &CaseGovernment,
    masc_fem: &HashSet<String>,
) -> bool {
    let n = tokens.len();
    let verb_at = tokens[verb_pos];
    let noun_at = tokens[noun_pos];

    let m_before_gt0 = |line: &str, pos: usize| {
        SearchMatch::new()
            .token_line(line)
            .m_before(tokens, pos)
            .is_some_and(|x| x > 0)
    };
    let m_after_ge1 = |line: &str, pos: i64| {
        SearchMatch::new()
            .token_line(line)
            .m_after(tokens, pos as usize)
            .is_some_and(|x| x >= 1)
    };

    // Любителі фотографувати їжу
    if has_re(verb_at, uk_helpers::verb_inf_pattern()) {
        if gov
            .get_case_governments_opt(&noun_at.readings, None, None)
            .iter()
            .any(|c| c == "v_inf")
        {
            return true;
        }
        if m_before_gt0("не сила", noun_pos) || m_before_gt0("не проти", noun_pos) {
            return true;
        }
        if ["хтось", "дехто"].contains(&noun_at.surface().to_lowercase().as_str()) {
            return true;
        }
        if verb_pos > 0 && tokens[verb_pos - 1].surface().to_lowercase() == "намагаючись"
        {
            return true;
        }
    }
    if has_part(noun_at, "predic") && ["було", "буде"].contains(&verb_at.surface()) {
        return true;
    }
    if ["правда"].contains(&noun_at.surface().to_lowercase().as_str()) {
        return true;
    }
    if m_before_gt0("під три чорти", noun_pos)
        || m_before_gt0("не штука", noun_pos)
        || m_before_gt0("бісики", noun_pos)
    {
        return true;
    }
    if m_after_ge1("будь якого", verb_pos as i64) {
        return true;
    }
    if verb_pos > 0 && m_after_ge1("не сказати б", verb_pos as i64 - 1) {
        return true;
    }
    if verb_pos > 0 && m_before_gt0("не проти", verb_pos - 1) {
        return true;
    }
    if lemma(noun_at, &["воно", "решта"]) && has_part(verb_at, ":impers") {
        return true;
    }
    if verb_pos > 0 && lemma(tokens[verb_pos - 1], &["Газа"]) {
        return true;
    }
    if noun_pos > 1
        && has(noun_at, "noun:.*:p:v_naz(?!.*pron).*")
        && lemma_with(
            tokens[noun_pos - 1],
            &["два", "три", "чотири"],
            "numr:p:v_zna",
        )
    {
        return true;
    }
    let v_prezydenty_prep = ["в", "у", "між", "межи", "поміж", "на"];
    if noun_pos > 1
        && has_start(noun_at, "noun:anim:p:v_naz")
        && lemma(tokens[noun_pos - 1], &v_prezydenty_prep)
    {
        return true;
    }
    if noun_pos > 2
        && has_start(noun_at, "noun:anim:p:v_naz")
        && has_start(tokens[noun_pos - 1], "adj:p:v_zna:rinanim")
        && lemma(tokens[noun_pos - 2], &v_prezydenty_prep)
    {
        return true;
    }
    if lt_tagger::uk_helpers::is_capitalized(verb_at.surface())
        && lt_tagger::uk_helpers::is_capitalized(noun_at.surface())
    {
        return true;
    }
    if noun_pos > 1
        && has(noun_at, "noun:anim:.:v_naz:prop:[fl]name.*")
        && ["ім'я", "прізвище", "прізвисько"]
            .contains(&tokens[noun_pos - 1].surface().to_lowercase().as_str())
    {
        return true;
    }
    if noun_pos > 2
        && has(noun_at, "noun.*:v_naz.*prop.*")
        && Regex::new(r"^[-\u{2013}\u{2014}]$")
            .unwrap()
            .is_match(tokens[noun_pos - 1].surface())
            .unwrap_or(false)
        && has(tokens[noun_pos - 2], "noun.*:v_naz.*prop.*")
    {
        return true;
    }
    if verb_pos > 2
        && lt_tagger::uk_helpers::is_capitalized(noun_at.surface())
        && lt_tagger::uk_helpers::is_capitalized(tokens[noun_pos - 1].surface())
        && !disjoint_inf(
            &verb_inflection::get_noun_inflections_v(&tokens[noun_pos - 1].readings),
            verb_inflections,
        )
    {
        return true;
    }
    if lt_tagger::uk_helpers::is_all_upper(verb_at.surface()) {
        return true;
    }
    if noun_pos > 1 && noun_at.surface() == "Я" {
        return true;
    }
    if verb_pos > 2 && verb_pos < n - 1 && verb_at.surface() == "давай" {
        return true;
    }
    if verb_pos > 1
        && verb_pos < n - 1
        && verb_at.surface() == "може"
        && tokens[verb_pos - 1].surface() != "не"
        && !has_re(tokens[verb_pos + 1], uk_helpers::verb_inf_pattern())
    {
        return true;
    }
    if noun_pos > 1 && has_re(verb_at, uk_helpers::verb_inf_pattern()) {
        let found = uk_helpers::reverse_search_idx(
            tokens,
            noun_pos as i64 - 1,
            6,
            Some(&INF_ARGREEMENT),
            None,
        );
        if found >= 0 {
            let f = tokens[found as usize];
            if !has_start(f, "adj")
                || !disjoint_inf2(
                    &inflection::get_noun_inflections(&noun_at.readings, None),
                    &inflection::get_adj_inflections(&f.readings),
                )
            {
                return true;
            }
        }
    }
    if verb_pos < n - 1 && has_re(verb_at, uk_helpers::verb_inf_pattern()) {
        let found = uk_helpers::forward_lemma_search_idx(
            tokens,
            verb_pos as i64 + 1,
            7,
            Some(&INF_ARGREEMENT),
            None,
        );
        if found >= 0 {
            let f = tokens[found as usize];
            if !has_start(f, "adj")
                || !disjoint_inf2(
                    &inflection::get_noun_inflections(noun_readings, None),
                    &inflection::get_adj_inflections(&f.readings),
                )
            {
                return true;
            }
        }
    }
    if noun_pos > 1 && has_re(verb_at, uk_helpers::verb_inf_pattern()) {
        let prev = uk_helpers::reverse_search_idx(
            tokens,
            noun_pos as i64 - 1,
            7,
            None,
            Some(&Regex::new(r"^verb.*$").unwrap()),
        );
        if prev >= 0
            && !disjoint_inf(
                &verb_inflection::get_verb_inflections(&tokens[prev as usize].readings),
                &verb_inflection::get_noun_inflections_v(&noun_at.readings),
            )
        {
            return true;
        }
    }
    if verb_pos < n - 1 && has_re(verb_at, uk_helpers::verb_inf_pattern()) {
        let next = SearchMatch::new()
            .ignore_inserts()
            .limit(8)
            .target(vec![Condition::postag("verb.*")])
            .m_after(tokens, verb_pos + 1);
        if let Some(next) = next {
            if !disjoint_inf(
                &verb_inflection::get_verb_inflections(&tokens[next].readings),
                &verb_inflection::get_noun_inflections_v(&noun_at.readings),
            ) {
                return true;
            }
        }
    }
    // — це були невільники
    if noun_pos > 1
        && verb_pos < n - 1
        && noun_at.surface() == "це"
        && Regex::new(r"^[-\u{2010}-\u{2015}]$")
            .unwrap()
            .is_match(tokens[noun_pos - 1].surface())
            .unwrap_or(false)
    {
        return true;
    }
    if noun_at.surface() == "це" && has_part_readings(verb_readings, "impers") {
        return true;
    }
    if noun_pos > 1
        && has_re(noun_at, &Regex::new(r"^noun.*:p:v_naz.*$").unwrap())
        && has_re(verb_at, &Regex::new(r"^verb.*?past:n.*$").unwrap())
        && (Regex::new(r"^\d+[234]$")
            .unwrap()
            .is_match(tokens[noun_pos - 1].surface())
            .unwrap_or(false)
            || ["два", "три", "чотири"].contains(&tokens[noun_pos - 1].surface()))
    {
        return true;
    }
    if has_re(verb_at, &Regex::new(r"^verb.*:[fp](?::.*|$)$").unwrap())
        && SearchMatch::new()
            .target(vec![Condition::token("пара")])
            .skip(vec![Condition::token_pattern(
                "(?:і|а|й|та|чи|або|ані|також|то|a|i)",
            )
            .negate()])
            .m_before(tokens, noun_pos - 1)
            .is_some_and(|x| x > 0)
    {
        return true;
    }
    if has_re(verb_at, &Regex::new(r"^verb.*:p(?::.*|$)$").unwrap()) {
        if noun_pos > 2
            && (tokens[noun_pos - 1].surface() == "/" || tokens[noun_pos - 2].surface() == "/")
        {
            return true;
        }
        if noun_pos > 2
            && uk_helpers::CONJ_FOR_PLURAL_WITH_COMMA.contains(&tokens[noun_pos - 1].surface())
            && has_re(tokens[noun_pos - 2], uk_helpers::noun_v_naz_pattern())
        {
            return true;
        }
        if noun_pos > 3
            && uk_helpers::CONJ_FOR_PLURAL_WITH_COMMA.contains(&tokens[noun_pos - 2].surface())
            && has_re(tokens[noun_pos - 3], uk_helpers::noun_v_naz_pattern())
            && !disjoint_inf2(
                &inflection::get_adj_inflections(&tokens[noun_pos - 1].readings),
                &inflection::get_noun_inflections(&noun_at.readings, None),
            )
        {
            return true;
        }
        let pos0left_opt = SearchMatch::new()
            .ignore_inserts()
            .limit(7)
            .target(vec![Condition::token_pattern(
                "(?:і|а|й|та|чи|або|ані|також|то|a|i)",
            )])
            .skip(vec![Condition::postag(
                "(?:noun.*?v_naz|(?:adj|numr):.:v_naz|adv|part).*",
            )])
            .m_before(tokens, noun_pos - 1);
        let mut pos0left: i64 = pos0left_opt.map(|x| x as i64).unwrap_or(-1);
        let pos0right = pos0left;
        if pos0left > 0 && is_non_plural_a(tokens, pos0left as usize) {
            pos0left = -1;
        }
        if pos0left > 1 && tokens[pos0left as usize - 1].surface() == "," {
            pos0left -= 1;
        }
        if pos0left > 1 {
            if pos0right > 2 {
                if pos0right as usize + 1 < n
                    && lemma(tokens[pos0right as usize + 1], &["інший"])
                    && lemma(tokens[pos0left as usize - 1], &["той"])
                {
                    return true;
                }
                if has_part(tokens[pos0left as usize - 1], "conj") {
                    pos0left -= 1;
                }
                let osobysto = ["особисто", "зокрема", "загалом"];
                if osobysto.contains(&tokens[pos0left as usize - 1].surface()) {
                    pos0left -= 1;
                }
                if osobysto.contains(&tokens[verb_pos - 1].surface()) {
                    return true;
                }
                if tokens[pos0left as usize - 1].surface() == ")" {
                    return true;
                }
                if has_re(
                    tokens[pos0left as usize - 1],
                    uk_helpers::noun_v_naz_pattern(),
                ) {
                    return true;
                }
                if verb_pos > 6
                    && has_part(tokens[pos0left as usize - 1], "adv")
                    && has_part(tokens[pos0left as usize - 2], "conj")
                {
                    pos0left -= 2;
                }
                while pos0left > 2
                    && Regex::new(r#"^[,»“”"]$"#)
                        .unwrap()
                        .is_match(tokens[pos0left as usize - 1].surface())
                        .unwrap_or(false)
                {
                    pos0left -= 1;
                }
            }
            let prev = tokens[pos0left as usize - 1];
            if has_start(prev, "noun")
                || has_start(prev, "number:latin")
                || lt_tagger::uk_helpers::is_possibly_proper_noun(prev)
            {
                return true;
            }
            if has_re(prev, uk_helpers::adj_v_naz_pattern()) {
                return true;
            }
        }
        let pos3 = uk_helpers::token_search(
            tokens,
            verb_pos as i64 - 2,
            None,
            Some(&Regex::new(r"^також$").unwrap()),
            Some(&Regex::new(r"^(?:noun|adj:.:v_naz|adv|part).*$").unwrap()),
            Dir::Reverse,
        );
        if pos3 > 1 {
            return true;
        }
        if noun_pos > 5 {
            let lower = tokens[noun_pos - 1].surface().to_lowercase();
            if ["що", "не"].contains(&lower.as_str())
                && uk_helpers::token_search(
                    tokens,
                    noun_pos as i64 - 3,
                    None,
                    Some(&Regex::new(&format!("(?i)^{lower}$")).unwrap()),
                    Some(&Regex::new(r"^(?:noun|adj).*$").unwrap()),
                    Dir::Reverse,
                ) > noun_pos as i64 - 7
            {
                return true;
            }
        }
        let pos1 = uk_helpers::token_search(
            tokens,
            noun_pos as i64 - 1,
            None,
            Some(&Regex::new(r"^,$").unwrap()),
            Some(&Regex::new(r"^adj.*$").unwrap()),
            Dir::Reverse,
        );
        if (pos1 > 1
            && pos1 as usize - 1 < n
            && has_re(tokens[pos1 as usize - 1], uk_helpers::noun_v_naz_pattern()))
            || (pos1 > 2
                && has(tokens[pos1 as usize - 1], "noun.*:v_rod.*")
                && has_re(tokens[pos1 as usize - 2], uk_helpers::noun_v_naz_pattern()))
        {
            return true;
        }
        if noun_pos > 4
            && lt_tagger::uk_helpers::is_capitalized(noun_at.surface())
            && (has_start(tokens[noun_pos - 1], "noun:anim")
                || lt_tagger::uk_helpers::is_initial(tokens[noun_pos - 1]))
            && uk_helpers::CONJ_FOR_PLURAL_WITH_COMMA.contains(&tokens[noun_pos - 2].surface())
            && lt_tagger::uk_helpers::is_capitalized(tokens[noun_pos - 3].surface())
            && (has_start(tokens[noun_pos - 4], "noun:anim")
                || lt_tagger::uk_helpers::is_initial(tokens[noun_pos - 1]))
        {
            return true;
        }
        let mut idx: i64 = SearchMatch::new()
            .target(vec![Condition::token_pattern(
                "(?:і|а|й|та|чи|або|ані|також|то|a|i)",
            )])
            .ignore_inserts()
            .skip(vec![
                Condition::postag("(?:noun|adj).*?v_(?:naz|rod).*"),
                Condition::token_pattern("і?з|зі|від|на|навіть|також|потім|згодом"),
            ])
            .m_before(tokens, noun_pos - 1)
            .map(|x| x as i64)
            .unwrap_or(-1);
        if idx > 0 && is_non_plural_a(tokens, idx as usize) {
            idx = -1;
        }
        if idx > 1
            && idx as usize + 1 < n
            && (has_re(tokens[idx as usize - 1], uk_helpers::noun_v_naz_pattern())
                || lt_tagger::uk_helpers::is_capitalized(tokens[idx as usize - 1].surface())
                || lemma(
                    tokens[idx as usize + 1],
                    &["навіть", "також", "потім", "згодом"],
                )
                || lemma(tokens[idx as usize - 1], &["потім", "згодом"]))
        {
            return true;
        }
        if (has_part(noun_at, "numr") && !lemma(noun_at, &["один"]))
            || lemma(noun_at, &["сотня", "тисяча", "десяток"])
        {
            return true;
        }
        if noun_pos > 1
            && has_part(tokens[noun_pos - 1], "number")
            && (!tokens[noun_pos - 1].surface().ends_with('1')
                || tokens[noun_pos - 1].surface().ends_with("11"))
        {
            return true;
        }
        if has_part(tokens[noun_pos - 1], "num")
            && noun_at.surface() == "чоловік"
            && uk_helpers::token_search(
                tokens,
                1,
                Some("noun:anim:f:"),
                Some(&Regex::new(r"^жінк[аи]$").unwrap()),
                Some(&Regex::new(r"^.*$").unwrap()),
                Dir::Forward,
            ) == -1
        {
            return true;
        }
        if noun_pos > 1
            && (tokens[noun_pos - 1].surface().ends_with("+1")
                || uk_helpers::token_search(
                    tokens,
                    verb_pos as i64 - 2,
                    None,
                    Some(&Regex::new(r"^плюс$").unwrap()),
                    Some(&Regex::new(r"^(?:numr|adj).*.:v_naz.*$").unwrap()),
                    Dir::Reverse,
                ) > 0)
        {
            return true;
        }
        if noun_pos > 2
            && lemma(tokens[noun_pos - 2], &["решта"])
            && Regex::new(r"^.+1$")
                .unwrap()
                .is_match(tokens[noun_pos - 1].surface())
                .unwrap_or(false)
        {
            return true;
        }
        if noun_pos > 2
            && lemma(noun_at, &["кожний"])
            && has_re(verb_at, &Regex::new(r"^verb.*(?:past:p|:p:3).*$").unwrap())
        {
            return true;
        }
        if noun_pos > 2
            && Regex::new(r"^(?:а?ні|жодн.*|навіть)$")
                .unwrap()
                .is_match(tokens[noun_pos - 1].surface())
                .unwrap_or(false)
        {
            return true;
        }
        if noun_pos > 2
            && tokens[verb_pos - 1].surface() == "не"
            && uk_helpers::reverse_search(
                tokens,
                noun_pos as i64 - 1,
                5,
                Some(&Regex::new(r"^а?ні$").unwrap()),
                None,
            )
        {
            return true;
        }
        if noun_pos > 3
            && tokens[verb_pos - 1].surface() == "не"
            && Regex::new(r"^а?ні$")
                .unwrap()
                .is_match(tokens[noun_pos - 2].surface())
                .unwrap_or(false)
            && !disjoint_inf2(
                &inflection::get_adj_inflections(&tokens[noun_pos - 1].readings),
                &inflection::get_noun_inflections(&noun_at.readings, None),
            )
        {
            return true;
        }
    }
    // Сейм Республіки Польща проігнорував
    if noun_pos > 3
        && has_part(noun_at, ":prop")
        && has(tokens[noun_pos - 1], "noun.*:v_rod.*")
        && !disjoint_inf(
            &verb_inflection::get_noun_inflections_v(&tokens[noun_pos - 2].readings),
            verb_inflections,
        )
    {
        return true;
    }
    if noun_pos > 1 {
        if has(noun_at, "noun:inanim:[mnf]:v_naz:prop:geo.*")
            && has(
                tokens[noun_pos - 1],
                "noun:inanim:[mnf]:v_(?!naz)(?!.*pron).*",
            )
        {
            return true;
        }
        if lt_tagger::uk_helpers::is_possibly_proper_noun(noun_at)
            && lemma(tokens[noun_pos - 1], GEO_QUALIFIERS)
        {
            return true;
        }
        if noun_pos > 2
            && lt_tagger::uk_helpers::is_possibly_proper_noun(noun_at)
            && lt_tagger::uk_helpers::is_possibly_proper_noun(tokens[noun_pos - 1])
            && lemma(tokens[noun_pos - 2], GEO_QUALIFIERS)
        {
            return true;
        }
    }
    if noun_pos > 3
        && has_part(noun_at, "v_naz:prop")
        && has(tokens[noun_pos - 1], "adj:.:v_naz.*")
        && has(tokens[noun_pos - 2], "noun.*:v_(rod|zna|mis).*")
    {
        return true;
    }
    if noun_pos > 1
        && has_part(noun_at, ":prop")
        && has_re(verb_at, &Regex::new(r"^verb.*:impers.*$").unwrap())
    {
        return true;
    }
    if noun_pos > 3
        && has(noun_at, "noun:inanim:.:v_naz:prop.*")
        && has(tokens[noun_pos - 1], "noun:inanim:.*")
        && has(tokens[noun_pos - 2], "adj:.*")
        && has_part(tokens[noun_pos - 3], "prep")
    {
        let cases =
            gov.get_case_governments_opt(&tokens[noun_pos - 3].readings, Some("prep"), None);
        if uk_helpers::has_vidm_pos_tag_token(&cases, tokens[noun_pos - 1])
            && uk_helpers::has_vidm_pos_tag_token(&cases, tokens[noun_pos - 2])
        {
            return true;
        }
    }
    if verb_pos < n - 1 && verb_at.surface() == "було" {
        let pos = uk_helpers::token_search(
            tokens,
            verb_pos as i64 + 1,
            Some("verb:"),
            None,
            Some(&Regex::new(r"^adv.*$").unwrap()),
            Dir::Forward,
        );
        if pos >= 0
            && !disjoint_inf(
                &verb_inflection::get_noun_inflections_v(&noun_at.readings),
                &verb_inflection::get_verb_inflections(&tokens[pos as usize].readings),
            )
        {
            return true;
        }
    }
    if verb_pos < n - 1
        && has_part(noun_at, ":prop")
        && verb_at.surface() == "було"
        && has_re(
            tokens[verb_pos + 1],
            &Regex::new(r"^verb.*:impers.*$").unwrap(),
        )
    {
        return true;
    }
    if noun_pos > 1
        && has(tokens[noun_pos - 1], "noun:inanim:.:v_naz.*")
        && !has_part(tokens[noun_pos - 1], ":pron")
        && !has(noun_at, "noun.*pron.*")
        && !disjoint_inf(
            &verb_inflection::get_noun_inflections_v(&tokens[noun_pos - 1].readings),
            verb_inflections,
        )
    {
        return true;
    }
    if has_part_readings(noun_readings, "noun:anim:m:v_naz")
        && has_re(verb_at, &Regex::new(r"^verb.*:f(?::.*|$)$").unwrap())
        && uk_helpers::has_masc_fem_lemma(noun_readings, masc_fem)
    {
        return true;
    }
    if ["пора"].contains(&noun_at.surface().to_lowercase().as_str())
        && ["було"].contains(&verb_at.surface())
    {
        return true;
    }
    let pseudo_plural = [
        "решта",
        "частина",
        "частка",
        "половина",
        "третина",
        "чверть",
    ];
    if pseudo_plural.contains(&noun_at.surface().to_lowercase().as_str())
        && has_re(verb_at, &Regex::new(r"^.*:[pn](?::.*|$)$").unwrap())
    {
        return true;
    }
    if noun_pos + 1 < n
        && tokens[noun_pos + 1].surface().eq_ignore_ascii_case("разом")
        && has_re(verb_at, &Regex::new(r"^.*:p(?::.*|$)$").unwrap())
    {
        return true;
    }
    if noun_pos > 2 && lemma(tokens[noun_pos - 1], &["ніж"]) {
        return true;
    }
    if noun_pos > 1
        && noun_at.surface().eq_ignore_ascii_case("ти")
        && has(verb_at, "noun.*?v_kly.*")
    {
        return true;
    }
    if verb_pos < n - 2
        && verb_at.surface() == "візьми"
        && lemma(tokens[verb_pos + 1], &["і", "й", "та"])
    {
        return true;
    }
    let mut v_pos = verb_pos;
    if verb_pos > 3 && has(tokens[verb_pos - 1], "noun:inanim:.:v_naz:prop.*") {
        let token = tokens[noun_pos - 1].surface();
        if lt_tagger::uk_helpers::is_capitalized(token) && has_start(tokens[noun_pos - 1], "adj") {
            v_pos -= 1;
        }
        if has_start(tokens[v_pos - 2], "noun:inanim") && has_part(tokens[v_pos - 3], "prep") {
            let cases =
                gov.get_case_governments_opt(&tokens[v_pos - 3].readings, Some("prep"), None);
            if uk_helpers::has_vidm_pos_tag_token(&cases, tokens[v_pos - 2]) {
                return true;
            }
        }
    }
    if noun_pos > 1
        && has_part(tokens[noun_pos - 1], "adj")
        && has_re(verb_at, uk_helpers::verb_inf_pattern())
        && gov
            .get_case_governments_opt(&tokens[noun_pos - 1].readings, None, None)
            .iter()
            .any(|c| c == "v_inf")
        && !disjoint_inf2(
            &inflection::get_adj_inflections(&tokens[noun_pos - 1].readings),
            &inflection::get_noun_inflections(noun_readings, None),
        )
    {
        return true;
    }
    if noun_pos > 1
        && Regex::new(r"^(?i:як)$")
            .unwrap()
            .is_match(tokens[noun_pos - 1].surface())
            .unwrap_or(false)
    {
        if Regex::new(r"^(?:ніхто|усі)$")
            .unwrap()
            .is_match(noun_at.surface())
            .unwrap_or(false)
        {
            return true;
        }
        if noun_at.surface() == "воно" && has_re(verb_at, uk_helpers::verb_inf_pattern()) {
            return true;
        }
        if noun_pos > 2 && lemma(tokens[noun_pos - 2], &["такий", "само"]) {
            return true;
        }
    }
    if has_re(noun_at, uk_helpers::noun_non_pron_v_naz_pattern())
        && uk_helpers::token_search_re(
            tokens,
            noun_pos as i64 - 1,
            Some(uk_helpers::adj_v_naz_pattern()),
            Some(&Regex::new(r"^(?i:як)$").unwrap()),
            None,
            Dir::Reverse,
        ) != -1
    {
        return true;
    }
    false
}
