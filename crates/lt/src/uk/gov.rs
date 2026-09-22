//! Minimal port of `org.languagetool.rules.uk.CaseGovernmentHelper`: the
//! `case_government.txt` (+ `derivats.txt`) case-government map, the
//! `V_MIS_PREPS` sets and `getCaseGovernments` (used by the hybrid
//! disambiguator).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::AnalyzedToken;

static MATY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^verb:imperf:(?:futr|past|pres).*$").unwrap());
static BUTY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^verb:imperf:(?:futr|past:n|pres:s:3).*$").unwrap());
static IMPERSONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^verb.*(?:pres:s:3|futr:s:3|past:n).*$").unwrap());
static NALEZHYT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^verb:imperf:inf.*$").unwrap());
static BILSHATY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:по)?більшати|(?:по)?меншати$").unwrap());
static BILSHATY_POS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^verb.*(?:inf|pres:s:3|futr:s:3|past:n).*$").unwrap());

pub struct CaseGovernment {
    map: HashMap<String, HashSet<String>>,
    pub v_mis_preps: HashSet<String>,
    /// `CaseGovernmentHelper.DERIVATIVES_MAP` (`derivats.txt`:
    /// derivative -> verbs).
    pub derivatives: HashMap<String, HashSet<String>>,
}

impl CaseGovernment {
    pub fn load(words_dir: &Path) -> Self {
        let mut map = load_map(&words_dir.join("case_government.txt"));
        map.entry("згідно з".to_string())
            .or_default()
            .insert("v_oru".to_string());
        let derivatives = load_map(&words_dir.join("derivats.txt"));
        for (key, verbs) in &derivatives {
            let mut set = HashSet::new();
            for verb in verbs {
                if let Some(rvs) = map.get(verb) {
                    set.extend(rvs.iter().cloned());
                }
            }
            map.insert(key.clone(), set);
        }
        let mut v_mis_preps: HashSet<String> = map
            .iter()
            .filter(|(_, v)| v.contains("v_mis"))
            .map(|(k, _)| k.clone())
            .collect();
        // add Latin y/B - often used instead of the real prep
        v_mis_preps.insert("y".to_string());
        v_mis_preps.insert("B".to_string());
        Self {
            map,
            v_mis_preps,
            derivatives,
        }
    }

    /// `CaseGovernmentHelper.CASE_GOVERNMENT_MAP`.
    pub fn case_map(&self) -> &HashMap<String, HashSet<String>> {
        &self.map
    }

    /// `CaseGovernmentHelper.hasCaseGovernment(readings, startPosTag, rvCase)`.
    pub fn has_case_government(
        &self,
        readings: &[AnalyzedToken],
        start_pos_tag: Option<&str>,
        rv_case: &str,
    ) -> bool {
        self.get_case_governments_opt(readings, start_pos_tag, None)
            .contains(rv_case)
    }

    /// `CaseGovernmentHelper.getCaseGovernments(readings, Pattern)`.
    pub fn get_case_governments_opt(
        &self,
        readings: &[AnalyzedToken],
        start_pos_tag: Option<&str>,
        pos_tag_regex: Option<&Regex>,
    ) -> HashSet<String> {
        let mut list = get_custom_govs(readings);
        let mut start_pos_tag = start_pos_tag.map(|s| s.to_string());
        if start_pos_tag.as_deref() == Some("verb")
            && readings
                .first()
                .and_then(|r| r.pos_tag.as_deref())
                .is_some_and(|t| t.starts_with("advp"))
        {
            start_pos_tag = Some("advp".to_string());
        }
        for token in readings {
            let Some(pos_tag) = token.pos_tag.as_deref() else {
                continue;
            };
            let matches = match (&start_pos_tag, pos_tag_regex) {
                (_, Some(re)) => re.is_match(pos_tag).unwrap_or(false),
                (Some(start), None) => pos_tag.starts_with(start.as_str()),
                (None, None) => true,
            };
            if matches && self.map.contains_key(token.stem.as_deref().unwrap_or("")) {
                if let Some(rv_list) = self.map.get(token.stem.as_deref().unwrap_or("")) {
                    list.extend(rv_list.iter().cloned());
                }
                if pos_tag.contains("adjp:pasv") {
                    list.insert("v_oru".to_string());
                }
            }
        }
        list
    }

    /// `CaseGovernmentHelper.getCaseGovernments(readings, posTagRegex)`.
    pub fn get_case_governments(
        &self,
        readings: &[AnalyzedToken],
        pos_tag_regex: &Regex,
    ) -> HashSet<String> {
        let mut list = get_custom_govs(readings);
        for token in readings {
            if token.pos_tag.is_none() {
                continue;
            }
            if pos_tag_regex
                .is_match(token.pos_tag.as_deref().unwrap_or(""))
                .unwrap_or(false)
            {
                let mut v_lemma = token.stem.clone().unwrap_or_default();
                if !self.map.contains_key(&v_lemma)
                    && token
                        .pos_tag
                        .as_deref()
                        .is_some_and(|t| t.starts_with("advp"))
                {
                    v_lemma = get_advp_verb_lemma(token);
                }
                if let Some(rv_list) = self.map.get(&v_lemma) {
                    list.extend(rv_list.iter().cloned());
                }
            }
            if token
                .pos_tag
                .as_deref()
                .is_some_and(|t| t.contains("adjp:pasv"))
            {
                list.insert("v_oru".to_string());
            }
        }
        list
    }

    /// `CaseGovernmentHelper.getCaseGovernments(readings, startPosTag)`.
    pub fn get_case_governments_start(
        &self,
        readings: &[AnalyzedToken],
        start_pos_tag: &str,
    ) -> HashSet<String> {
        let mut start_pos_tag = start_pos_tag.to_string();
        if start_pos_tag == "verb"
            && readings
                .first()
                .and_then(|r| r.pos_tag.as_deref())
                .is_some_and(|t| t.starts_with("advp"))
        {
            start_pos_tag = "advp".to_string();
        }
        let mut list = get_custom_govs(readings);
        for token in readings {
            let Some(pos_tag) = token.pos_tag.as_deref() else {
                continue;
            };
            if (pos_tag.starts_with(&start_pos_tag)
                || (start_pos_tag == "prep" && pos_tag == "<prep>"))
                && self.map.contains_key(token.stem.as_deref().unwrap_or(""))
            {
                if let Some(rv_list) = self.map.get(token.stem.as_deref().unwrap_or("")) {
                    list.extend(rv_list.iter().cloned());
                }
                if pos_tag.contains("adjp:pasv") {
                    list.insert("v_oru".to_string());
                }
            }
        }
        list
    }
}

fn load_map(path: &Path) -> HashMap<String, HashSet<String>> {
    let mut result: HashMap<String, HashSet<String>> = HashMap::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return result;
    };
    for line in text.lines() {
        let mut parts = line.split(' ');
        let Some(key) = parts.next() else { continue };
        let Some(vidm) = parts.next() else { continue };
        let entry = result.entry(key.to_string()).or_default();
        for c in vidm.split(':') {
            entry.insert(c.to_string());
        }
    }
    result
}

/// `CaseGovernmentHelper.getCustomGovs`.
fn get_custom_govs(readings: &[AnalyzedToken]) -> HashSet<String> {
    let mut list = HashSet::new();
    let v_inf = lt_tagger::uk_helpers::has_lemma_with_pattern(readings, &["мати"], &MATY)
        || lt_tagger::uk_helpers::has_lemma_with_pattern(readings, &["бути"], &BUTY)
        || lt_tagger::uk_helpers::has_lemma_with_pattern(
            readings,
            &[
                "вимагатися",
                "випадати",
                "випасти",
                "личити",
                "належати",
                "тягнути",
                "щастити",
                "плануватися",
                "рекомендуватися",
                "пропонуватися",
                "сподобатися",
                "прийтися",
                "удатися",
                "годитися",
                "доводитися",
            ],
            &IMPERSONAL,
        )
        || lt_tagger::uk_helpers::has_lemma_with_pattern(readings, &["належить"], &NALEZHYT);
    if v_inf {
        list.insert("v_inf".to_string());
    } else if lt_tagger::uk_helpers::has_lemma_regex_with_pattern(
        readings,
        &BILSHATY,
        &BILSHATY_POS,
    ) {
        list.insert("v_rod".to_string());
    }
    list
}

fn get_advp_verb_lemma(token: &AnalyzedToken) -> String {
    let lemma = token.stem.clone().unwrap_or_default();
    if lemma == "даючи" {
        return "давати".to_string();
    }
    if lemma == "змушуючи" {
        return "змушувати".to_string();
    }
    let re1 = Regex::new(r"лячи(с[яь])?").unwrap();
    let re2 = Regex::new(r"(ючи|вши)(с[яь])?").unwrap();
    let step1 = re1.replace(&lemma, "ити$1").into_owned();
    re2.replace(&step1, "ти$2").into_owned()
}
