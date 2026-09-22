//! Port of the parts of `org.languagetool.rules.uk.InflectionHelper` used by
//! the hybrid disambiguator (`getAdjInflections`/`getNounInflections`).

use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::AnalyzedToken;

static ADJ_INFLECTION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":([mfnp]):(v_...)(:r(in)?anim)?").unwrap());
static NOUN_INFLECTION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"((?:[iu]n)?anim):([mfnps]):(v_...)").unwrap());
static MFN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[mfn]$").unwrap());

#[derive(Debug, Clone)]
pub struct Inflection {
    pub gender: String,
    pub case_: String,
    pub anim_tag: Option<String>,
}

impl Inflection {
    fn gender_equals(a: &str, b: &str) -> bool {
        if a == b {
            return true;
        }
        (a == "s" && MFN.is_match(b).unwrap_or(false))
            || (b == "s" && MFN.is_match(a).unwrap_or(false))
    }

    pub fn anim_matters(&self) -> bool {
        self.anim_tag.is_some()
            && self.anim_tag.as_deref() != Some("unanim")
            && self.case_ == "v_zna"
            && self.is_animal_sensitive()
    }

    fn is_animal_sensitive(&self) -> bool {
        self.gender.contains('m') || self.gender.contains('p')
    }
}

impl Inflection {
    /// `InflectionHelper.Inflection.equalsIgnoreGender`.
    pub fn equals_ignore_gender(&self, other: &Inflection) -> bool {
        self.case_ == other.case_
            && (self.anim_tag.is_none()
                || other.anim_tag.is_none()
                || !self.anim_matters()
                || self.anim_tag == other.anim_tag)
    }
}

impl PartialEq for Inflection {
    fn eq(&self, other: &Self) -> bool {
        Self::gender_equals(&self.gender, &other.gender)
            && self.case_ == other.case_
            && (self.anim_tag.is_none()
                || other.anim_tag.is_none()
                || !self.anim_matters()
                || !other.is_animal_sensitive()
                || self.anim_tag == other.anim_tag)
    }
}

/// `InflectionHelper.getAdjInflections(readings)`.
pub fn get_adj_inflections(readings: &[AnalyzedToken]) -> Vec<Inflection> {
    get_adj_inflections_start(readings, "adj")
}

/// `InflectionHelper.getAdjInflections(readings, postagStart)`.
pub fn get_adj_inflections_start(readings: &[AnalyzedToken], start: &str) -> Vec<Inflection> {
    let mut out: Vec<Inflection> = Vec::new();
    for token in readings {
        let Some(pos_tag) = token.pos_tag.as_deref() else {
            continue;
        };
        if !pos_tag.starts_with(start) {
            continue;
        }
        let Ok(Some(caps)) = ADJ_INFLECTION_PATTERN.captures(pos_tag) else {
            continue;
        };
        let Some(g1) = caps.get(1) else { continue };
        let Some(g2) = caps.get(2) else { continue };
        let anim_tag = caps.get(3).map(|m| m.as_str()[2..].to_string());
        let inflection = Inflection {
            gender: g1.as_str().to_string(),
            case_: g2.as_str().to_string(),
            anim_tag,
        };
        if !out.contains(&inflection) {
            out.push(inflection);
        }
    }
    out
}

/// `InflectionHelper.getNounInflections(readings, ignoreTag)`.
pub fn get_noun_inflections(
    readings: &[AnalyzedToken],
    ignore_tag: Option<&Regex>,
) -> Vec<Inflection> {
    let mut out: Vec<Inflection> = Vec::new();
    for token in readings {
        let Some(pos_tag) = token.pos_tag.as_deref() else {
            continue;
        };
        if let Some(ignore) = ignore_tag {
            if ignore.find(pos_tag).ok().flatten().is_some() {
                continue;
            }
        }
        let Ok(Some(caps)) = NOUN_INFLECTION_PATTERN.captures(pos_tag) else {
            continue;
        };
        let (Some(g1), Some(g2), Some(g3)) = (caps.get(1), caps.get(2), caps.get(3)) else {
            continue;
        };
        let inflection = Inflection {
            gender: g2.as_str().to_string(),
            case_: g3.as_str().to_string(),
            anim_tag: Some(g1.as_str().to_string()),
        };
        if !out.contains(&inflection) {
            out.push(inflection);
        }
    }
    out
}

/// `TokenAgreementAdjNounRule.formatInflections`.
pub fn format_inflections(inflections: &[Inflection], adj: bool) -> String {
    let mut sorted = inflections.to_vec();
    sorted.sort_by(|a, b| {
        lt_tagger::uk_helpers::gen_order(&a.gender)
            .cmp(&lt_tagger::uk_helpers::gen_order(&b.gender))
            .then_with(|| {
                lt_tagger::uk_helpers::vidm_order(&a.case_)
                    .cmp(&lt_tagger::uk_helpers::vidm_order(&b.case_))
            })
    });
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for inf in &sorted {
        let mut case_str = lt_tagger::uk_helpers::case_name(&inf.case_);
        if adj {
            if let Some(anim) = inf.anim_tag.as_deref() {
                case_str += if anim == "anim" {
                    " (іст.)"
                } else {
                    " (неіст.)"
                };
            }
        }
        if let Some(group) = groups.iter_mut().find(|(g, _)| g == &inf.gender) {
            group.1.push(case_str);
        } else {
            groups.push((inf.gender.clone(), vec![case_str]));
        }
    }
    groups
        .iter()
        .map(|(g, cases)| {
            format!(
                "{}: {}",
                lt_tagger::uk_helpers::gender_name(g),
                cases.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
