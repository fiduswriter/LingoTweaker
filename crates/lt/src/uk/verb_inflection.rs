//! Port of `org.languagetool.rules.uk.VerbInflectionHelper` (the
//! `TokenAgreementVerbNounRule`/`TokenAgreementNounVerbRule` inflection
//! matching).
#![allow(dead_code)]

use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::AnalyzedToken;

static VERB_INFLECTION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":([mfnps])(?::([123])?|$)").unwrap());
static NOUN_INFLECTION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?::((?:[iu]n)?anim))?:([mfnps]):(v_naz)").unwrap());
static ADJ_INFLECTION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:adj|numr):([mfnps]):(v_naz)").unwrap());
static NOUN_PERSON_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":([123])").unwrap());

#[derive(Debug, Clone)]
pub struct VerbInflection {
    pub gender: Option<String>,
    pub plural: String,
    pub person: Option<String>,
}

impl VerbInflection {
    pub fn new(gender: &str, person: Option<&str>) -> Self {
        let (gender, plural) = if gender == "s" || gender == "p" {
            (None, gender.to_string())
        } else if gender == "i" {
            (Some(gender.to_string()), gender.to_string())
        } else {
            (Some(gender.to_string()), "s".to_string())
        };
        Self {
            gender,
            plural,
            person: person.map(|p| p.to_string()),
        }
    }
}

impl PartialEq for VerbInflection {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(a), Some(b)) = (&self.person, &other.person) {
            if a != b {
                return false;
            }
        }
        if let (Some(a), Some(b)) = (&self.gender, &other.gender) {
            if a != b {
                return false;
            }
        }
        self.plural == other.plural
    }
}

fn find_group(re: &Regex, s: &str, group: usize) -> Option<String> {
    re.captures(s)
        .ok()
        .flatten()
        .and_then(|c| c.get(group).map(|m| m.as_str().to_string()))
}

fn find_group_opt(re: &Regex, s: &str, group: usize) -> Option<String> {
    // `find()` + optional group (Java `find()` does not require a full match)
    re.captures(s)
        .ok()
        .flatten()
        .and_then(|c| c.get(group).map(|m| m.as_str().to_string()))
}

/// `VerbInflectionHelper.getVerbInflections`.
pub fn get_verb_inflections(readings: &[AnalyzedToken]) -> Vec<VerbInflection> {
    let mut out = Vec::new();
    for token in readings {
        let Some(pos_tag) = token.pos_tag.as_deref() else {
            continue;
        };
        if !pos_tag.starts_with("verb") {
            continue;
        }
        if pos_tag.contains(":inf") {
            out.push(VerbInflection::new("i", None));
            continue;
        }
        if pos_tag.contains(":impers") {
            out.push(VerbInflection::new("o", None));
            continue;
        }
        if let Ok(Some(caps)) = VERB_INFLECTION_PATTERN.captures(pos_tag) {
            let gen = caps
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let person = caps.get(2).map(|m| m.as_str().to_string());
            out.push(VerbInflection::new(&gen, person.as_deref()));
        }
    }
    out
}

/// `VerbInflectionHelper.getNounInflections`.
pub fn get_noun_inflections_v(readings: &[AnalyzedToken]) -> Vec<VerbInflection> {
    let mut out = Vec::new();
    for token in readings {
        let Some(pos_tag) = token.pos_tag.as_deref() else {
            continue;
        };
        let Some(gen) = find_group(&NOUN_INFLECTION_PATTERN, pos_tag, 2) else {
            continue;
        };
        let person = find_group_opt(&NOUN_PERSON_PATTERN, pos_tag, 1);
        out.push(VerbInflection::new(&gen, person.as_deref()));
    }
    out
}

/// `VerbInflectionHelper.getAdjInflections`.
pub fn get_adj_inflections_v(readings: &[AnalyzedToken]) -> Vec<VerbInflection> {
    let mut out = Vec::new();
    for token in readings {
        let Some(pos_tag) = token.pos_tag.as_deref() else {
            continue;
        };
        let Some(gen) = find_group(&ADJ_INFLECTION_PATTERN, pos_tag, 1) else {
            continue;
        };
        let person = find_group_opt(&NOUN_PERSON_PATTERN, pos_tag, 1);
        out.push(VerbInflection::new(&gen, person.as_deref()));
    }
    out
}

/// `VerbInflectionHelper.inflectionsOverlap`.
pub fn inflections_overlap(verb: &[AnalyzedToken], noun: &[AnalyzedToken]) -> bool {
    let verbs = get_verb_inflections(verb);
    let nouns = get_noun_inflections_v(noun);
    verbs.iter().any(|v| nouns.contains(v))
}
