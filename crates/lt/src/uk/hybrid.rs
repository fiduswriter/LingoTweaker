//! Port of the `UkrainianHybridDisambiguator.preDisambiguate` hand-coded passes
//! that do not need `CaseGovernmentHelper`/`InflectionHelper`. The
//! helper-dependent passes (`removeYih`, `removeVmis`, `removeVerbImpr`,
//! `disambiguatePronPos`) are not ported yet.

use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};

static INITIAL_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[А-ЯІЇЄҐ]\.$").unwrap());
static INANIM_VKLY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noun:inanim:.:v_kly(?!.*:geo).*$").unwrap());
static ADJ_V_KLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^adj:.:v_kly.*$").unwrap());
static PUNCT_AFTER_KLY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(?:[!?,»"“”…]|[.!?]{2,3})$"#).unwrap());
static PLURAL_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noun:anim:p:.*:fname.*$").unwrap());
static PLURAL_LNAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noun:anim:p:.*:[lp]name.*$").unwrap());
static PATTERN_1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[а-яіїєґa-z0-9].*$").unwrap());
static PATTERN_2: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9]+(?:[.,–—-][0-9]+)?$").unwrap());
static PATTERN_3: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:два|дві|три|чотири)$").unwrap());
static PATTERN_4: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":(alt|nv|up\d{2}|xp\d)").unwrap());
static PATTERN_5: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":[mfn]:v_rod").unwrap());
static LATIN_DIGITS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[XIVХІ]+(?:[–—-][XIVХІ]+)?$").unwrap());
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]+(?:[–—-][0-9]+)?$").unwrap());
static STATION_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:метро|[А-Я][а-яіїєґ'-]+)$").unwrap());
static CAPITALIZED_LEMMA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[А-ЯІЇЄҐ][а-яіїєґ'-].*$").unwrap());
static PROP_POS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^.*?:prop$").unwrap());
static ST_ABBR: &str = "ст.";

fn nw_indices(sentence: &AnalyzedSentence) -> Vec<usize> {
    sentence
        .tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.is_whitespace)
        .map(|(i, _)| i)
        .collect()
}

fn is_all_upper(s: &str) -> bool {
    s.chars().all(|c| !c.is_lowercase()) && s.chars().any(|c| c.is_uppercase())
}

fn has_tag_str(token: &AnalyzedTokenReadings, pattern: &str) -> bool {
    has_tag(&token.readings, pattern)
}

fn has_tag(readings: &[AnalyzedToken], pattern: &str) -> bool {
    let re = Regex::new(&format!("^(?:{pattern})$")).unwrap();
    readings.iter().any(|r| {
        r.pos_tag
            .as_deref()
            .is_some_and(|t| re.is_match(t).unwrap_or(false))
    })
}

fn has_tag_re(readings: &[AnalyzedToken], re: &Regex) -> bool {
    readings.iter().any(|r| reading_matches(r, re))
}

fn has_tag_part(token: &AnalyzedTokenReadings, part: &str) -> bool {
    token
        .readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains(part)))
}

fn add_reading(token: &mut AnalyzedTokenReadings, reading: AnalyzedToken) {
    if !token.readings.contains(&reading) {
        token.readings.push(reading);
    }
    token.is_tagged = token.readings.iter().any(|r| r.pos_tag.is_some());
}

fn set_readings(token: &mut AnalyzedTokenReadings, readings: Vec<AnalyzedToken>) {
    token.readings = readings;
    token.is_tagged = token.readings.iter().any(|r| r.pos_tag.is_some());
}

/// `UkrainianHybridDisambiguator.preDisambiguate` (implemented subset).
pub fn pre_disambiguate(sentence: &mut AnalyzedSentence, gov: &crate::uk::gov::CaseGovernment) {
    remove_yih(sentence, gov);
    remove_vmis(sentence, gov);
    retag_fem_names(sentence);
    retag_initials(sentence);
    retag_unknown_initials(sentence);
    remove_inanim_v_kly(sentence);
    remove_plural_for_names(sentence);
    remove_lower_case_homonyms_for_abbreviations(sentence);
    remove_lower_case_bad_for_upper_case_good(sentence);
    disambiguate_st(sentence);
    disambiguate_pron_pos(sentence);
    retag_plural_prop(sentence);
    remove_verb_impr(sentence);
}

/// `UkrainianHybridDisambiguator.retagFemNames`.
fn retag_fem_names(sentence: &mut AnalyzedSentence) {
    let idxs = nw_indices(sentence);
    let n = idxs.len();
    let mut i = 1;
    while i + 2 < n {
        for gen in ["f", "m"] {
            let prefix: &[&str] = if gen == "f" {
                &[
                    "пані",
                    "місіс",
                    "місис",
                    "міс",
                    "леді",
                    "княгиня",
                    "німкеня",
                ]
            } else {
                &["пан", "містер", "м-р", "сер", "князь", "німець", "поляк"]
            };
            let anim_prop_tag_prefix = format!("noun:anim:{gen}:v_naz:prop");
            let verb_past = format!(r"^verb.*:past:{gen}$");
            let cur = &sentence.tokens[idxs[i]];
            let has_prefix = prefix.iter().any(|p| {
                cur.readings.iter().any(|r| {
                    r.stem.as_deref() == Some(p)
                        && r.pos_tag
                            .as_deref()
                            .is_some_and(|t| t.starts_with(&format!("noun:anim:{gen}:v_naz")))
                })
            }) || cur.readings.iter().any(|r| {
                r.pos_tag
                    .as_deref()
                    .is_some_and(|t| t.starts_with(&format!("{anim_prop_tag_prefix}:fname")))
            });
            if has_prefix
                && sentence.tokens[idxs[i + 2]].readings.iter().any(|r| {
                    r.pos_tag.as_deref().is_some_and(|t| {
                        Regex::new(&verb_past).unwrap().is_match(t).unwrap_or(false)
                    })
                })
            {
                let name_i = idxs[i + 1];
                let name = &sentence.tokens[name_i];
                let name_readings = name.readings.clone();
                let name_surface = name.surface().to_string();
                if name_readings.iter().any(|r| {
                    r.pos_tag
                        .as_deref()
                        .is_some_and(|t| t.starts_with(&anim_prop_tag_prefix))
                }) {
                    let kept: Vec<AnalyzedToken> = name_readings
                        .iter()
                        .filter(|r| {
                            r.pos_tag
                                .as_deref()
                                .is_some_and(|t| t.starts_with(&anim_prop_tag_prefix))
                        })
                        .cloned()
                        .collect();
                    set_readings(&mut sentence.tokens[name_i], kept);
                } else if gen == "f"
                    && name_readings.iter().any(|r| {
                        r.pos_tag
                            .as_deref()
                            .is_some_and(|t| t.starts_with("noun:anim:m:v_naz:prop"))
                    })
                {
                    set_readings(
                        &mut sentence.tokens[name_i],
                        vec![AnalyzedToken::new(
                            &name_surface,
                            Some(name_surface.clone()),
                            Some("noun:anim:f:v_naz:prop:lname".to_string()),
                        )],
                    );
                } else if lt_tagger::uk_helpers::is_capitalized(&name_surface)
                    && !has_tag_part(name, ":prop")
                    && cur.readings.iter().any(|r| {
                        r.pos_tag.as_deref().is_some_and(|t| {
                            t.starts_with(&format!("{anim_prop_tag_prefix}:fname"))
                        })
                    })
                {
                    set_readings(
                        &mut sentence.tokens[name_i],
                        vec![AnalyzedToken::new(
                            &name_surface,
                            Some(name_surface.clone()),
                            Some(format!("{anim_prop_tag_prefix}:lname")),
                        )],
                    );
                }
                i += 1;
            }
        }
        i += 1;
    }
}

/// `UkrainianHybridDisambiguator.retagUnknownInitials`.
fn retag_unknown_initials(sentence: &mut AnalyzedSentence) {
    for token in sentence.tokens.iter_mut() {
        if token.is_whitespace {
            continue;
        }
        let surface = token.surface().to_string();
        if surface.ends_with('.') && INITIAL_REGEX.is_match(&surface).unwrap_or(false) {
            if has_tag_part(token, "name") {
                continue;
            }
            for reading in token.readings.clone() {
                token.readings.retain(|r| r != &reading);
            }
            add_reading(
                token,
                AnalyzedToken::new(&surface, None, Some("noninfl:abbr".to_string())),
            );
        }
    }
}

/// `UkrainianHybridDisambiguator.retagInitials` (+ `checkForInitialRetag`,
/// `getInitialReadings`, `isInitial`).
fn retag_initials(sentence: &mut AnalyzedSentence) {
    let mut initials_idxs: Vec<usize> = Vec::new();
    let mut last_name: Option<usize> = None;
    for i in 1..sentence.tokens.len() {
        if sentence.tokens[i].is_whitespace {
            continue;
        }
        if has_tag_part(&sentence.tokens[i], ":prop:lname") {
            last_name = Some(i);
            if !initials_idxs.is_empty() {
                check_for_initial_retag(sentence, last_name, &initials_idxs);
                last_name = None;
                initials_idxs.clear();
            }
            continue;
        }
        if is_initial(sentence, i) {
            initials_idxs.push(i);
            continue;
        }
        check_for_initial_retag(sentence, last_name, &initials_idxs);
        last_name = None;
        initials_idxs.clear();
    }
    check_for_initial_retag(sentence, last_name, &initials_idxs);
}

fn is_initial(sentence: &AnalyzedSentence, pos: usize) -> bool {
    let surface = sentence.tokens[pos].surface();
    surface.ends_with('.') && INITIAL_REGEX.is_match(surface).unwrap_or(false)
}

fn check_for_initial_retag(
    sentence: &mut AnalyzedSentence,
    last_name: Option<usize>,
    initials_idxs: &[usize],
) {
    if last_name.is_none() || (initials_idxs.len() != 1 && initials_idxs.len() != 2) {
        return;
    }
    let lname = last_name.unwrap();
    let fname_pos = initials_idxs[0];
    let new = get_initial_readings(
        &sentence.tokens[fname_pos],
        &sentence.tokens[lname],
        "fname",
    );
    replace_readings(&mut sentence.tokens[fname_pos], new);
    if initials_idxs.len() == 2 {
        let pname_pos = initials_idxs[1];
        let new = get_initial_readings(
            &sentence.tokens[pname_pos],
            &sentence.tokens[lname],
            "pname",
        );
        replace_readings(&mut sentence.tokens[pname_pos], new);
    }
}

fn replace_readings(token: &mut AnalyzedTokenReadings, new: AnalyzedTokenReadings) {
    token.readings = new.readings;
    token.start_pos = new.start_pos;
    token.is_tagged = token.readings.iter().any(|r| r.pos_tag.is_some());
}

fn get_initial_readings(
    initials: &AnalyzedTokenReadings,
    lname: &AnalyzedTokenReadings,
    initial_type: &str,
) -> AnalyzedTokenReadings {
    let initials_token = initials.surface().to_string();
    let mut new_tokens = Vec::new();
    for lname_token in &lname.readings {
        let Some(lname_pos) = lname_token.pos_tag.as_deref() else {
            continue;
        };
        if !lname_pos.contains(":prop:lname") {
            continue;
        }
        let cleaned = PATTERN_4.replace_all(lname_pos, "").into_owned();
        let pos = cleaned.replace(":prop:lname", &format!(":nv:abbr:prop:{initial_type}"));
        new_tokens.push(AnalyzedToken::new(
            &initials_token,
            Some(initials_token.clone()),
            Some(pos),
        ));
    }
    let mut out = AnalyzedTokenReadings::new(new_tokens);
    out.start_pos = initials.start_pos;
    out
}

/// `UkrainianHybridDisambiguator.removeInanimVKly`.
fn remove_inanim_v_kly(sentence: &mut AnalyzedSentence) {
    let idxs = nw_indices(sentence);
    for (k, &i) in idxs.iter().enumerate().skip(1) {
        if !has_tag(
            &sentence.tokens[i].readings,
            "noun:inanim:.:v_kly(?!.*:geo).*",
        ) || likely_vkly_context(sentence, &idxs, k)
        {
            continue;
        }
        let readings = sentence.tokens[i].readings.clone();
        let mut inanim_vkly = Vec::new();
        let mut other_found = false;
        for r in &readings {
            let Some(pos) = r.pos_tag.as_deref() else {
                break;
            };
            if pos == "SENT_END" {
                continue;
            }
            if INANIM_VKLY.is_match(pos).unwrap_or(false) {
                inanim_vkly.push(r.clone());
            } else {
                other_found = true;
            }
        }
        if !inanim_vkly.is_empty() && other_found {
            for r in inanim_vkly {
                if r.stem.as_deref() == Some("зоря") {
                    continue;
                }
                sentence.tokens[i].readings.retain(|x| x != &r);
            }
            sentence.tokens[i].is_tagged = sentence.tokens[i]
                .readings
                .iter()
                .any(|r| r.pos_tag.is_some());
        }
    }
}

fn likely_vkly_context(sentence: &AnalyzedSentence, idxs: &[usize], k: usize) -> bool {
    const LIKELY_V_KLY: &[&str] = &["суде", "роде", "заходе", "місяченьку", "редакціє"];
    let i = idxs[k];
    let surface = sentence.tokens[i].surface().to_lowercase();
    if LIKELY_V_KLY.contains(&surface.as_str()) {
        return true;
    }
    if k + 1 >= idxs.len() {
        return false;
    }
    let prev = &sentence.tokens[idxs[k - 1]];
    let next_surface = sentence.tokens[idxs[k + 1]].surface();
    let prev_is_o = prev.surface().eq_ignore_ascii_case("о");
    let prev_not_prep = !prev
        .readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with("prep")));
    (prev_is_o || prev_not_prep)
        && PUNCT_AFTER_KLY.is_match(next_surface).unwrap_or(false)
        && (prev.readings.iter().any(|r| {
            r.pos_tag
                .as_deref()
                .is_some_and(|t| ADJ_V_KLY.is_match(t).unwrap_or(false))
        }) || prev_is_o)
}

/// `UkrainianHybridDisambiguator.removePluralForNames`.
fn remove_plural_for_names(sentence: &mut AnalyzedSentence) {
    let idxs = nw_indices(sentence);
    for (k, &i) in idxs.iter().enumerate().skip(1) {
        if k > 1 {
            let prev = &sentence.tokens[idxs[k - 1]];
            let skip = prev.readings.iter().any(|r| {
                r.pos_tag
                    .as_deref()
                    .is_some_and(|t| t.starts_with("adj:p") || t.contains("num"))
            }) || prev.readings.iter().any(|r| {
                r.stem
                    .as_deref()
                    .is_some_and(|l| ["багато", "мало", "півсотня", "сотня"].contains(&l))
            });
            if skip {
                continue;
            }
        }
        if k + 1 < idxs.len()
            && sentence.tokens[idxs[k + 1]].readings.iter().any(|r| {
                r.pos_tag
                    .as_deref()
                    .is_some_and(|t| PLURAL_LNAME.is_match(t).unwrap_or(false))
            })
        {
            continue;
        }
        if k + 3 < idxs.len()
            && has_tag_part(&sentence.tokens[idxs[k + 1]], ":lname")
            && has_tag_part(&sentence.tokens[idxs[k + 3]], ":lname")
        {
            continue;
        }
        let readings = sentence.tokens[i].readings.clone();
        let mut plural_names = Vec::new();
        let mut other_found = false;
        for r in &readings {
            let Some(pos) = r.pos_tag.as_deref() else {
                break;
            };
            if pos == "SENT_END" {
                continue;
            }
            if PLURAL_NAME.is_match(pos).unwrap_or(false) {
                plural_names.push(r.clone());
            } else {
                other_found = true;
            }
        }
        if plural_names.is_empty() || !other_found {
            continue;
        }
        let prev_is_z = sentence.tokens[idxs[k - 1]].readings.iter().any(|r| {
            r.stem
                .as_deref()
                .is_some_and(|l| ["з", "із", "зі"].contains(&l))
                && r.pos_tag.as_deref().is_some_and(|t| t.contains("prep"))
        });
        if !prev_is_z {
            for r in plural_names {
                sentence.tokens[i].readings.retain(|x| x != &r);
            }
            sentence.tokens[i].is_tagged = sentence.tokens[i]
                .readings
                .iter()
                .any(|r| r.pos_tag.is_some());
        }
    }
}

/// `UkrainianHybridDisambiguator.removeLowerCaseHomonymsForAbbreviations`.
fn remove_lower_case_homonyms_for_abbreviations(sentence: &mut AnalyzedSentence) {
    for token in sentence.tokens.iter_mut() {
        if token.is_whitespace {
            continue;
        }
        let surface = token.surface();
        if is_all_upper(surface) && has_tag_part(token, ":abbr") {
            for r in token.readings.clone() {
                let pos = r.pos_tag.as_deref().unwrap_or("");
                if !pos.contains(":abbr") && pos != "SENT_END" && pos != "PARA_END" {
                    token.readings.retain(|x| x != &r);
                }
            }
            token.is_tagged = token.readings.iter().any(|r| r.pos_tag.is_some());
        }
    }
}

/// `UkrainianHybridDisambiguator.removeLowerCaseBadForUpperCaseGood`.
fn remove_lower_case_bad_for_upper_case_good(sentence: &mut AnalyzedSentence) {
    for token in sentence.tokens.iter_mut() {
        if token.is_whitespace || token.readings.len() <= 1 {
            continue;
        }
        let surface = token.surface().to_string();
        if !lt_tagger::uk_helpers::is_capitalized(&surface) {
            continue;
        }
        let has_prop_lemma = token.readings.iter().any(|r| {
            r.stem
                .as_deref()
                .is_some_and(|l| CAPITALIZED_LEMMA.is_match(l).unwrap_or(false))
                && r.pos_tag
                    .as_deref()
                    .is_some_and(|t| PROP_POS.is_match(t).unwrap_or(false))
        });
        if !has_prop_lemma {
            continue;
        }
        let lower_lemma = token
            .readings
            .first()
            .and_then(|r| r.stem.clone())
            .unwrap_or_default()
            .to_lowercase();
        for r in token.readings.clone() {
            if r.pos_tag.as_deref().is_some_and(|t| t.contains(":bad"))
                && r.stem.as_deref().is_some_and(|l| l == lower_lemma)
            {
                token.readings.retain(|x| x != &r);
            }
        }
        token.is_tagged = token.readings.iter().any(|r| r.pos_tag.is_some());
    }
}

/// `UkrainianHybridDisambiguator.removeTokensWithout`.
fn remove_tokens_without(token: &mut AnalyzedTokenReadings, pattern: &str) {
    let re = Regex::new(&format!("^(?:{pattern})$")).unwrap();
    for r in token.readings.clone() {
        let pos = r.pos_tag.as_deref().unwrap_or("");
        if pos != "SENT_END" && !re.is_match(pos).unwrap_or(false) {
            token.readings.retain(|x| x != &r);
        }
    }
    token.is_tagged = token.readings.iter().any(|r| r.pos_tag.is_some());
}

fn nw_at<'a>(
    sentence: &'a AnalyzedSentence,
    idxs: &[usize],
    k: usize,
) -> Option<&'a AnalyzedTokenReadings> {
    idxs.get(k).map(|&i| &sentence.tokens[i])
}

/// `UkrainianHybridDisambiguator.disambiguateSt`.
fn disambiguate_st(sentence: &mut AnalyzedSentence) {
    let idxs = nw_indices(sentence);
    let n = idxs.len();
    let mut k = 1;
    while k < n {
        if sentence.tokens[idxs[k]].surface() != ST_ABBR {
            k += 1;
            continue;
        }
        // 10 мм рт. ст.
        if k > 1 {
            if nw_at(sentence, &idxs, k - 1).map(|t| t.surface()) == Some("рт.") {
                remove_tokens_without(&mut sentence.tokens[idxs[k]], "noun.*:xp3.*");
                k += 1;
                continue;
            } else {
                remove_tokens_without(&mut sentence.tokens[idxs[k]], "(?!.*:xp3).*");
            }
        }
        // стаття/сторінка
        if k + 1 < n
            && PATTERN_2
                .is_match(sentence.tokens[idxs[k + 1]].surface())
                .unwrap_or(false)
        {
            if k > 2 && nw_at(sentence, &idxs, k - 1).map(|t| t.surface()) == Some(ST_ABBR) {
                remove_tokens_without(&mut sentence.tokens[idxs[k - 1]], "noun:inanim:p:.*");
            }
            remove_tokens_without(&mut sentence.tokens[idxs[k]], "noun:inanim:f:.*");
            k += 1;
            continue;
        }
        if k + 1 < n {
            let next_i = idxs[k + 1];
            let next = &sentence.tokens[next_i];
            // столова
            if uk_has_lemma(next, &["ложка"]) || next.surface() == "л." {
                remove_tokens_without(&mut sentence.tokens[idxs[k]], "adj:[fp]:.*");
                k += 2;
                continue;
            }
            // старший
            if uk_has_lemma(
                next,
                &["лейтенант", "сержант", "солдат", "науковий", "медсестра"],
            ) {
                remove_tokens_without(&mut sentence.tokens[idxs[k]], "adj:m:.*");
                k += 2;
                continue;
            }
            // станція
            if STATION_NAME.is_match(next.surface()).unwrap_or(false) {
                remove_tokens_without(&mut sentence.tokens[idxs[k]], "noun:inanim:f:.*");
                k += 2;
                continue;
            }
        }
        // століття
        if k > 1 {
            let prev = nw_at(sentence, &idxs, k - 1).map(|t| t.surface().to_string());
            if prev
                .as_deref()
                .is_some_and(|p| LATIN_DIGITS.is_match(p).unwrap_or(false))
            {
                if k + 1 < n && nw_at(sentence, &idxs, k + 1).map(|t| t.surface()) == Some(ST_ABBR)
                {
                    remove_tokens_without(&mut sentence.tokens[idxs[k + 1]], "noun:inanim:p:.*");
                }
                remove_tokens_without(&mut sentence.tokens[idxs[k]], "noun:inanim:n:.*");
                k += 2;
                continue;
            } else if prev
                .as_deref()
                .is_some_and(|p| DIGITS.is_match(p).unwrap_or(false))
            {
                if k + 1 < n && nw_at(sentence, &idxs, k + 1).map(|t| t.surface()) == Some(ST_ABBR)
                {
                    remove_tokens_without(&mut sentence.tokens[idxs[k + 1]], "noun:inanim:p:.*");
                }
                remove_tokens_without(&mut sentence.tokens[idxs[k]], "noun:inanim:[nf]:.*");
                k += 2;
                continue;
            }
        }
        k += 1;
    }
}

fn uk_has_lemma(token: &AnalyzedTokenReadings, lemmas: &[&str]) -> bool {
    token
        .readings
        .iter()
        .any(|r| r.stem.as_deref().is_some_and(|l| lemmas.contains(&l)))
}

/// `UkrainianHybridDisambiguator.retagPulralProp`.
fn retag_plural_prop(sentence: &mut AnalyzedSentence) {
    let idxs = nw_indices(sentence);
    let n = idxs.len();
    let mut k = 2;
    while k < n {
        let prop_i = idxs[k];
        let prev_surface = sentence.tokens[idxs[k - 1]].surface().to_lowercase();
        let has_naz_prop = has_tag_str(&sentence.tokens[prop_i], r"noun.*:p:v_naz.*:prop.*")
            || has_tag_str(&sentence.tokens[prop_i], r"noun.*:[mfn]:v_naz.*:prop.*");
        if PATTERN_3.is_match(&prev_surface).unwrap_or(false) && !has_naz_prop {
            let prop_only: Vec<AnalyzedToken> = sentence.tokens[prop_i]
                .readings
                .iter()
                .filter(|r| {
                    has_tag(std::slice::from_ref(r), r"noun:.*:[fmn]:v_rod.*prop.*")
                        && (r.pos_tag.as_deref().is_some_and(|t| !t.contains(":m:"))
                            || r.stem
                                .as_deref()
                                .is_some_and(|l| l.ends_with('а') || l.ends_with('о')))
                })
                .cloned()
                .collect();
            if let Some(first) = prop_only.first() {
                let postag = PATTERN_5
                    .replace(first.pos_tag.as_deref().unwrap_or(""), ":p:v_naz")
                    .into_owned();
                let lemma = first.stem.clone().unwrap_or_default();
                let surface = sentence.tokens[prop_i].surface().to_string();
                set_readings(
                    &mut sentence.tokens[prop_i],
                    vec![AnalyzedToken::new(&surface, Some(lemma), Some(postag))],
                );
                k += 1;
            }
        }
        k += 1;
    }
}
static ADJ_PRON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^adj.*pron.*$").unwrap());
static VERB_ANY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^verb.*$").unwrap());
static VERB_ADVP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:verb|advp).*$").unwrap());
static ADJ_OR_NOUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:adj|noun).*$").unwrap());
static ADV_OR_PREP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:adv|prep).*$").unwrap());
static PRON_PERS_OR_PREP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.*pron:pers.*|prep.*)$").unwrap());
static VERB_INF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^verb.*:inf.*$").unwrap());
static PUNCT_SINGLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[,.;\u{2013}\u{2014}-]$").unwrap());
static NONINFL_PREDIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noninfl:predic.*$").unwrap());
static YIH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:їх|його|її)$").unwrap());
static OBOH_NIHTO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:обох|ніхто|ніщо)$").unwrap());
static NE_NI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^н[еі]$").unwrap());

fn reading_matches(r: &AnalyzedToken, re: &Regex) -> bool {
    r.pos_tag
        .as_deref()
        .is_some_and(|t| re.is_match(t).unwrap_or(false))
}

fn remove_readings_by_regex(token: &mut AnalyzedTokenReadings, re: &Regex) {
    for r in token.readings.clone() {
        if reading_matches(&r, re) {
            token.readings.retain(|x| x != &r);
        }
    }
    token.is_tagged = token.readings.iter().any(|r| r.pos_tag.is_some());
}

/// `UkrainianHybridDisambiguator.removeYih`.
fn remove_yih(sentence: &mut AnalyzedSentence, gov: &crate::uk::gov::CaseGovernment) {
    let idxs = nw_indices(sentence);
    let n = idxs.len();
    let noun_lemmas = [
        "кількість",
        "розгляд",
        "обговорення",
        "використання",
        "реалізація",
        "виконання",
        "звільнення",
        "виробництво",
        "застосування",
        "проведення",
        "утримання",
        "вирішення",
        "загибель",
        "аналоги",
        "однолітки",
        "перелік",
        "затримання",
        "створення",
        "розміщення",
        "лікування",
        "втілення",
        "арешт",
        "формування",
        "наявність",
        "збереження",
    ];
    for k in 1..n {
        let i = idxs[k];
        let surface = sentence.tokens[i].surface().to_string();
        if surface.is_empty() {
            continue;
        }
        let lower = surface.to_lowercase();
        if !YIH.is_match(&lower).unwrap_or(false) {
            continue;
        }
        if k + 1 < n {
            let next_i = idxs[k + 1];
            let next = &sentence.tokens[next_i];
            let next_lower = sentence.tokens[next_i].surface().to_lowercase();
            let next_has_verb = next
                .readings
                .iter()
                .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with("verb")));
            if uk_has_lemma(next, &noun_lemmas)
                || has_tag_re(&next.readings, &NONINFL_PREDIC)
                || (next_has_verb && !has_tag_re(&next.readings, &ADJ_OR_NOUN))
            {
                remove_readings_by_regex(&mut sentence.tokens[i], &ADJ_PRON);
                continue;
            }
            if OBOH_NIHTO.is_match(&next_lower).unwrap_or(false)
                || (has_tag_re(&next.readings, &PRON_PERS_OR_PREP)
                    && !Regex::new(r"^і?з$")
                        .unwrap()
                        .is_match(&next_lower)
                        .unwrap_or(false))
                || (k + 2 < n
                    && NE_NI.is_match(&next_lower).unwrap_or(false)
                    && sentence.tokens[idxs[k + 2]]
                        .readings
                        .iter()
                        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with("verb"))))
            {
                remove_readings_by_regex(&mut sentence.tokens[i], &ADJ_PRON);
                continue;
            }
            if !has_tag_re(&next.readings, &ADJ_OR_NOUN) {
                let cg = gov.get_case_governments(&next.readings, &VERB_ANY);
                if cg.contains("v_rod") || cg.contains("v_zna") {
                    remove_readings_by_regex(&mut sentence.tokens[i], &ADJ_PRON);
                    continue;
                }
            }
        }
        if k > 1 {
            let prev = &sentence.tokens[idxs[k - 1]];
            let cg = gov.get_case_governments(&prev.readings, &VERB_ADVP);
            if cg.contains("v_rod") || cg.contains("v_zna") {
                let next_surface = if k + 1 < n {
                    sentence.tokens[idxs[k + 1]].surface().to_string()
                } else {
                    String::new()
                };
                if k + 1 >= n
                    || has_tag_re(&sentence.tokens[idxs[k + 1]].readings, &ADV_OR_PREP)
                    || PUNCT_SINGLE.is_match(&next_surface).unwrap_or(false)
                {
                    remove_readings_by_regex(&mut sentence.tokens[i], &ADJ_PRON);
                    continue;
                }
                if k + 1 < n
                    && has_tag_re(&prev.readings, &VERB_ADVP)
                    && has_tag_re(&sentence.tokens[idxs[k + 1]].readings, &VERB_INF)
                    && cg.contains("v_inf")
                {
                    remove_readings_by_regex(&mut sentence.tokens[i], &ADJ_PRON);
                    continue;
                }
            }
        }
    }
}

/// `UkrainianHybridDisambiguator.removeVmis`.
fn remove_vmis(sentence: &mut AnalyzedSentence, gov: &crate::uk::gov::CaseGovernment) {
    let idxs = nw_indices(sentence);
    let mut start_check = false;
    for &i in idxs.iter().skip(1) {
        let surface = sentence.tokens[i].surface().to_string();
        if surface.is_empty() {
            continue;
        }
        let lower = surface.to_lowercase();
        let has_prep = has_tag_part(&sentence.tokens[i], "prep");
        if !start_check {
            if has_prep {
                start_check = true;
            } else if PATTERN_1.is_match(&lower).unwrap_or(false) {
                if surface.chars().all(|c| !c.is_uppercase()) {
                    continue;
                }
                start_check = true;
            }
        }
        if has_prep && gov.v_mis_preps.contains(&lower) {
            return;
        }
        let readings = &sentence.tokens[i].readings;
        let mut found_vmis = false;
        let mut found_other = false;
        for t in readings {
            if t.pos_tag.as_deref().is_some_and(|x| x.contains("v_mis")) {
                found_vmis = true;
            } else if t.pos_tag.as_deref().is_some_and(|x| !x.ends_with("_END")) {
                found_other = true;
            }
            if found_vmis && found_other {
                break;
            }
        }
        if !(found_vmis && found_other) {
            continue;
        }
        for r in readings.clone() {
            if r.pos_tag.as_deref().is_some_and(|t| t.contains("v_mis")) {
                sentence.tokens[i].readings.retain(|x| x != &r);
            }
        }
        sentence.tokens[i].is_tagged = sentence.tokens[i]
            .readings
            .iter()
            .any(|r| r.pos_tag.is_some());
    }
}

static IGNORE_IN_PRON_POS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"pron|noun:anim:p:v_zna.*:rare.*").unwrap());
static ADJ_PRON_POS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^adj.*pron:pos.*$").unwrap());
static V_ZNA_VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new("v_zna:var").unwrap());
static VERB_IMPR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^verb.*impr.*$").unwrap());

/// `UkrainianHybridDisambiguator.disambiguatePronPos`.
fn disambiguate_pron_pos(sentence: &mut AnalyzedSentence) {
    use crate::uk::inflection;
    let idxs = nw_indices(sentence);
    let n = idxs.len();
    for k in 1..n {
        let i = idxs[k];
        let lower = sentence.tokens[i].surface().to_lowercase();
        if !matches!(lower.as_str(), "його" | "її" | "їх") {
            continue;
        }
        if !has_tag_re(&sentence.tokens[i].readings, &ADJ_PRON_POS) {
            continue;
        }
        let mut noun_inflections = Vec::new();
        if k > 1 {
            noun_inflections.extend(inflection::get_noun_inflections(
                &sentence.tokens[idxs[k - 1]].readings,
                Some(&IGNORE_IN_PRON_POS),
            ));
        }
        if k + 1 < n {
            noun_inflections.extend(inflection::get_noun_inflections(
                &sentence.tokens[idxs[k + 1]].readings,
                Some(&IGNORE_IN_PRON_POS),
            ));
        }
        if noun_inflections.is_empty() {
            continue;
        }
        for r in sentence.tokens[i].readings.clone() {
            if r.pos_tag.as_deref().is_some_and(|t| t.starts_with("adj")) {
                let adj_inflections = inflection::get_adj_inflections(std::slice::from_ref(&r));
                if adj_inflections
                    .iter()
                    .all(|a| !noun_inflections.contains(a))
                {
                    sentence.tokens[i].readings.retain(|x| x != &r);
                }
            }
        }
        sentence.tokens[i].is_tagged = sentence.tokens[i]
            .readings
            .iter()
            .any(|r| r.pos_tag.is_some());
    }
}

/// `UkrainianHybridDisambiguator.removeVerbImpr`.
fn remove_verb_impr(sentence: &mut AnalyzedSentence) {
    use crate::uk::inflection;
    let idxs = nw_indices(sentence);
    let n = idxs.len();
    for k in 2..n {
        let i = idxs[k];
        if has_tag_re(&sentence.tokens[i].readings, &VERB_IMPR)
            && has_tag_str(&sentence.tokens[i], "noun.*")
            && has_tag_str(&sentence.tokens[idxs[k - 1]], "adj.*")
        {
            let master = inflection::get_adj_inflections(&sentence.tokens[idxs[k - 1]].readings);
            let slave =
                inflection::get_noun_inflections(&sentence.tokens[i].readings, Some(&V_ZNA_VAR));
            if master.iter().any(|m| slave.contains(m)) {
                for r in sentence.tokens[i].readings.clone() {
                    if has_tag(std::slice::from_ref(&r), "verb.*impr.*") {
                        sentence.tokens[i].readings.retain(|x| x != &r);
                    }
                }
                sentence.tokens[i].is_tagged = sentence.tokens[i]
                    .readings
                    .iter()
                    .any(|r| r.pos_tag.is_some());
            }
        }
    }
}
