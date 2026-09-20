//! `org.languagetool.rules.ca.ConvertToGenderAndNumberFilter` (82 XML refs)
//! and its `ApostophationHelper` dependency (preposition+determiner
//! contraction and apostrophation).

use std::collections::HashMap;
use std::sync::OnceLock;

use lt_core::{AnalyzedToken, Suggestion, TextRange};
use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::ca::adapt::preserve_case;
use crate::ca::filters::Env;
use crate::ca::helpers::reading_with_tag_regex;
use crate::ca::verb_filters::{pos_word_start, tokens_without_whitespace};

/// `ConvertToGenderAndNumberFilter.splitGenderNumber`.
fn split_gender_number() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new("^(?:(N.|A..|V.P..|D..|PX.)(.)(.)(.*))$").unwrap())
}

fn split_gender_number_no_noun() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new("^(?:(A..|V.P..|D..|PX.)(.)(.)(.*))$").unwrap())
}

fn split_gender_number_adjective() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new("^(?:(A..|V.P..|PX.)(.)(.)(.*))$").unwrap())
}

fn postag_exceptions() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new("^(?:NP.*|AQ0CN0|SPS00|[CP].*)$").unwrap())
}

const FORMS_TO_IGNORE: [&str; 2] = ["mes", "las"];

#[derive(Debug, Clone, Default)]
pub(crate) struct GenderAndNumberSplit {
    pub(crate) prefix: String,
    pub(crate) suffix: String,
    pub(crate) gender: String,
    pub(crate) number: String,
}

/// `ConvertToGenderAndNumberFilter.splitGenderAndNumber`.
pub(crate) fn split_gender_and_number(atr: Option<&AnalyzedToken>) -> Option<GenderAndNumberSplit> {
    let tag = atr?.pos_tag.as_deref()?;
    let caps = split_gender_number().captures(tag)?;
    let mut results = GenderAndNumberSplit {
        prefix: caps[1].to_string(),
        suffix: caps[4].to_string(),
        ..Default::default()
    };
    let g2 = caps[2].to_string();
    let g3 = caps[3].to_string();
    if results.prefix.starts_with('V') {
        results.gender = g3;
        results.number = g2;
    } else {
        results.gender = g2;
        results.number = g3;
    }
    Some(results)
}

/// `ConvertToGenderAndNumberFilter.synthesizeWithGenderAndNumber`.
fn synthesize_with_gender_and_number(
    env: &Env,
    atr: &AnalyzedToken,
    split_postag: &GenderAndNumberSplit,
    gender: &str,
    number: &str,
) -> String {
    let (word, remainder) = match atr.token.split_once(' ') {
        Some((w, r)) => (w, r),
        None => (atr.token.as_str(), ""),
    };
    let _ = word;
    let mut gender = gender.to_string();
    let mut number = number.to_string();
    if split_postag.prefix.starts_with('V') {
        std::mem::swap(&mut gender, &mut number);
    }
    let add_gender = if split_postag.prefix.starts_with("DA") {
        ""
    } else {
        "C"
    };
    let pattern = format!(
        "{}[{gender}{add_gender}][{number}N]{}",
        split_postag.prefix, split_postag.suffix
    );
    let synthesized = env.synth.inner().synthesize(atr, &pattern, true);
    if let Some(first) = synthesized.first() {
        if !remainder.is_empty() {
            return format!("{first} {remainder}");
        }
        return first.clone();
    }
    String::new()
}

// ---------------------------------------------------------------------------
// ApostophationHelper
// ---------------------------------------------------------------------------

fn prep_det() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("MS", "el "),
            ("FS", "la "),
            ("MP", "els "),
            ("FP", "les "),
            ("MSapos", "l'"),
            ("FSapos", "l'"),
            ("aMS", "al "),
            ("aFS", "a la "),
            ("aMP", "als "),
            ("aFP", "a les "),
            ("aMSapos", "a l'"),
            ("aFSapos", "a l'"),
            ("dMS", "del "),
            ("dFS", "de la "),
            ("dMP", "dels "),
            ("dFP", "de les "),
            ("dMSapos", "de l'"),
            ("dFSapos", "de l'"),
            ("pMS", "pel "),
            ("pFS", "per la "),
            ("pMP", "pels "),
            ("pFP", "per les "),
            ("pMSapos", "per l'"),
            ("pFSapos", "per l'"),
        ])
    })
}

fn p_masc_yes() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new("(?i)^(?:h?[aeiouàèéíòóú].*)$").unwrap())
}

fn p_masc_no() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new("(?i)^(?:h?[ui][aeioàèéóò].+)$").unwrap())
}

fn p_fem_yes() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new("(?i)^(?:h?[aeoàèéíòóú].*|h?[ui][^aeiouàèéíòóúüï]+[aeiou][ns]?|urbs)$")
            .unwrap()
    })
}

fn p_fem_no() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new("(?i)^(?:host|ira|inxa)$").unwrap())
}

/// `ApostophationHelper.pHacAspirada` (from `mots_HAC_ASPIRADA2` and
/// `mots_HAC_ASPIRADA` in `entities.ent`).
fn p_hac_aspirada() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(concat!(
            "(?i)^(?:",
            r"Higgs|high|Hildesheim|Hill|hijabs?|Hillary|Himmler|hip-hop|hippies|hippy|hipsters?|Hirado|His|hits?|Hubei|Hudson|Hunter|Husserl|Huygens|husky|Utah|hides?|honey.*|Hartle.*|happy|happi.*|Hulk|Heart.*|Haakon|Halberstadt|Harley|Huck.*|Hanna|haka|hakes|Hama|Hornbostel|Heidi|Hayao|Hansi|Haas|Hindemith|user|users|one|head|history|Human|Hampshire|Hovedstaden|Handmade|Helm|Hahnem.*|hikimor.*|Houdini|Hugging|Heritage|hardcore|hancock|hender.*|h[ei][zs]b[ou]l.*|harira|Hawth.*|Henk.*|Humphry|Hohle|Höhle|Hooke|hajj.*|Hochschule|Hoch.*|Hutt|Hansel|Henley|hook|Handstand|Hull|Hatshepsut|Hatchepsut|Hana|Hamri|Hanley|Halis|Huxley|Hess|Hatteras|Herzberg|Hanlon|Harriet|hawl.*|hard|hip|herderi.*|Hangouts|Hayes|hostings?|Hal|hajj|Hermann|Hannah|Hertzsprung|Hotmail|Homrani|Harris|Harvey|Hunspell|Hassan|Haddock|Haarle[mn].*|Hainan|haendel.*|händel.*|habermas.*|hadits?|Hanuk?kà|hack.*|Harlem|Harper|Hartford|Haifa|haikus?|haima|haimes|Haikou|halal|halar|Halifax|Halmstad|halls?|Halle|Halley|Hallstatt|Hallstein|Halloweens?|Hals|herr|Herut|Hamadan|Hamas|Hamàs|hamilton.*|Hamlet.*|hammams?|Hammond|Hampton|hàmsters?|h[aà]ndicaps?|Hangzhou|Hannover|Hanoi|Hans|Hansa|hanseàti[cq].*|happenings?|Harbin|hardware|Haneke|harolds?|Hatay|Hamleigh |Harrisburg|Harrison|harrods?|harry|Hartley|Hartmann?|Hartree|Haruki|Har[td]?vard|Harz|hash.*|Hastings|Havel|Havilland|hawai.*|hawk.*|Hayek|Haydn.*|Hayworth|Heard|hearst|Heathrow|heav.*|hegel.*|Hebei|Hedmark|Heerenveen|Hedw.*|Heerlen|Hefei|Heidelberg|Heide[gn].*|Heilbronn|Heilongjiang|Heilig.*|hei[nk].*|Heisen.*|Heitz|Helmand|Helmholtz|Helen|Helsingborg|Hèlsinki|Heming.*|Henan|henna|hennes|Henry|Hepburn|herbert.*|Herder|Hereford|Herford|Herning|Hertfordshire|Herzog|Hesse|Hessen.*|Hewlett.*|H[ie]zbol·?l.+|high.*|hilbert.*|Hilda|Hillingdon|hinden.*|Hilton|hinterlands?|Hirsch.*|Hitch.*|hitler.*|Hilversum|Hobart|Hockenheim|Hodeida|Hohhot|Hokkaido|hobbes.*|hobby|Hogw.*|hobbies|Hodgkin|Hohen.*|Hölderlin|h[òo]ldings?|holy.*|hollywood.*|Holmes.*|Holstein|Hong|Hong-Kong|hongk.+|Honolu.+|Honsh[uū]|h[òo]bbits?|hooligan.*|hoover.*|hopkins|Hork.*|Horowitz|horst|H[ou]f.*|Houla|house|Houston|Howard|Hoyerswerda|Hunan|Huddersfield|Hunedoara|huskys?|huskies|hubs?|Hubble|humbold.*|Hume|hunting.*|Hussein|husseinit.+|Unity|university|united.*|European|OneDrive",
            ")$"
        ))
        .unwrap()
    })
}

/// `ApostophationHelper.getPrepositionAndDeterminer`.
pub(crate) fn get_preposition_and_determiner(
    new_form: &str,
    gender_number: &str,
    preposition: &str,
) -> String {
    let mut apos = "";
    let mut gn = gender_number.to_string();
    if gn.starts_with('C') {
        gn = format!("M{}", gn.chars().nth(1).unwrap_or('?'));
    }
    if gn.ends_with('N') {
        gn = format!("{}S", gn.chars().next().unwrap_or('?'));
    }
    let mut pre = String::new();
    if !preposition.is_empty() {
        pre = preposition
            .chars()
            .next()
            .map(|c| c.to_lowercase().to_string())
            .unwrap_or_default();
    }
    if !p_hac_aspirada().is_match(new_form) {
        if gn == "MS" {
            if p_masc_yes().is_match(new_form) && !p_masc_no().is_match(new_form) {
                apos = "apos";
            }
        } else if gn == "FS" && p_fem_yes().is_match(new_form) && !p_fem_no().is_match(new_form) {
            apos = "apos";
        }
    }
    prep_det()
        .get(format!("{pre}{gn}{apos}").as_str())
        .copied()
        .unwrap_or("")
        .to_string()
}

/// `StringTools.preserveCaseWordByWord`.
fn preserve_case_word_by_word(input_string: &str, model_string: &str) -> String {
    let input_words: Vec<&str> = input_string.split(' ').collect();
    let model_words: Vec<&str> = model_string.split(' ').collect();
    if input_words.len() != model_words.len() {
        return preserve_case(input_string, model_string);
    }
    let mut result = String::new();
    for (i, word) in input_words.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        result.push_str(&preserve_case(word, model_words[i]));
    }
    result
}

// ---------------------------------------------------------------------------
// ConvertToGenderAndNumberFilter
// ---------------------------------------------------------------------------

pub struct ConvertToGenderAndNumberFilter {
    pub(crate) env: Env,
}

impl RuleFilter for ConvertToGenderAndNumberFilter {
    #[allow(clippy::too_many_lines)]
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let tokens = tokens_without_whitespace(ctx);
        let src = ctx.sentence_text;
        let pos_word = pos_word_start(&tokens, ctx.match_range.start);
        let desired_gender_orig_str = ctx.args.get("gender").cloned().unwrap_or_default();
        let desired_number_orig_str = ctx.args.get("number").cloned().unwrap_or_default();
        let Some(lemma_select) = ctx.args.get("lemmaSelect") else {
            return FilterOutcome::reject();
        };
        let new_lemma = ctx.args.get("newLemma").cloned().unwrap_or_default();
        let keep_original = ctx
            .args
            .get("keepOriginal")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let atr_noun_orig = reading_with_tag_regex(tokens[pos_word], lemma_select);
        let mut atr_noun_list: Vec<AnalyzedToken> = Vec::new();
        let mut suggestions: Vec<String> = Vec::new();
        if let Some(orig) = atr_noun_orig.filter(|_| !new_lemma.is_empty()) {
            atr_noun_list.push(AnalyzedToken::new(
                orig.token.clone(),
                Some(new_lemma.clone()),
                orig.pos_tag.clone(),
            ));
        } else if !ctx.suggestions.is_empty() {
            let split_noun_orig_postag = split_gender_and_number(atr_noun_orig);
            for suggestion in &ctx.suggestions {
                let (word, remainder) = match suggestion.value.split_once(' ') {
                    Some((w, r)) => (w.to_string(), r.to_string()),
                    None => (suggestion.value.clone(), String::new()),
                };
                let atrs = self.env.tagger.tag(std::slice::from_ref(&word));
                let Some(atr_readings) = atrs.into_iter().next() else {
                    suggestions.extend(ctx.suggestions.iter().map(|s| s.value.clone()));
                    atr_noun_list.clear();
                    break;
                };
                let at = reading_with_tag_regex(&atr_readings, &split_gender_number().to_string());
                if at.is_none()
                    || at.and_then(|a| a.pos_tag.as_ref()).is_none()
                    || atr_readings.has_pos_tag_matching(postag_exceptions())
                {
                    // if there is any suggestion without gender and number,
                    // use the list of suggestions with no change
                    suggestions.extend(ctx.suggestions.iter().map(|s| s.value.clone()));
                    atr_noun_list.clear();
                    break;
                }
                let at = at.unwrap();
                let split_postag = split_gender_and_number(Some(at)).unwrap_or_default();
                let number = if !desired_number_orig_str.is_empty() {
                    desired_number_orig_str.clone()
                } else if let Some(split) = &split_noun_orig_postag {
                    if split.number != "N" {
                        split.number.clone()
                    } else {
                        split_postag.number.clone()
                    }
                } else {
                    split_postag.number.clone()
                };
                let gender = if !desired_gender_orig_str.is_empty() {
                    desired_gender_orig_str.clone()
                } else {
                    split_postag.gender.clone()
                };
                let mut new_postag = split_postag.prefix.clone();
                if split_postag.prefix.starts_with('V') {
                    new_postag.push_str(&number);
                    new_postag.push_str(&gender);
                } else {
                    new_postag.push_str(&gender);
                    new_postag.push_str(&number);
                }
                new_postag.push_str(&split_postag.suffix);
                let complete_form = if remainder.is_empty() {
                    at.lemma().to_string()
                } else {
                    format!("{word} {remainder}")
                };
                atr_noun_list.push(AnalyzedToken::new(
                    complete_form,
                    Some(at.lemma().to_string()),
                    Some(new_postag),
                ));
            }
        } else if let Some(orig) = atr_noun_orig {
            atr_noun_list.push(orig.clone());
        }

        let mut start_pos = pos_word;
        let mut end_pos = pos_word;
        for atr_noun in &atr_noun_list {
            start_pos = pos_word;
            end_pos = pos_word;
            let split_postag = split_gender_and_number(Some(atr_noun));
            let mut desired_gender_str = if !desired_gender_orig_str.is_empty() {
                desired_gender_orig_str.clone()
            } else {
                split_postag
                    .as_ref()
                    .map(|s| s.gender.clone())
                    .unwrap_or_default()
            };
            let mut desired_number_str = if !desired_number_orig_str.is_empty() {
                desired_number_orig_str.clone()
            } else {
                split_postag
                    .as_ref()
                    .map(|s| s.number.clone())
                    .unwrap_or_default()
            };
            // if gender = C, look into the words before and after
            if desired_gender_str == "C" && pos_word > 1 {
                if let Some(split2) = split_gender_and_number(reading_with_tag_regex(
                    tokens[pos_word - 1],
                    &split_gender_number().to_string(),
                )) {
                    if split2.gender == "F" || split2.gender == "M" {
                        desired_gender_str = split2.gender;
                    }
                }
            }
            if desired_gender_str == "C" && pos_word + 1 < tokens.len() {
                if let Some(split2) = split_gender_and_number(reading_with_tag_regex(
                    tokens[pos_word + 1],
                    &split_gender_number().to_string(),
                )) {
                    if split2.gender == "F" || split2.gender == "M" {
                        desired_gender_str = split2.gender;
                    }
                }
            }
            // if number = N, look into the words before and after
            if desired_number_str == "N" && pos_word > 1 {
                if let Some(split2) = split_gender_and_number(reading_with_tag_regex(
                    tokens[pos_word - 1],
                    &split_gender_number().to_string(),
                )) {
                    if split2.number == "S" || split2.number == "P" {
                        desired_number_str = split2.number;
                    }
                }
            }
            if desired_number_str == "N" && pos_word + 1 < tokens.len() {
                if let Some(split2) = split_gender_and_number(reading_with_tag_regex(
                    tokens[pos_word + 1],
                    &split_gender_number().to_string(),
                )) {
                    if split2.number == "S" || split2.number == "P" {
                        desired_number_str = split2.number;
                    }
                }
            }
            // Prioritize gender and number in the original
            if let Some(split) = &split_postag {
                if desired_gender_str.contains(&split.gender) {
                    desired_gender_str = format!(
                        "{}{}",
                        split.gender,
                        desired_gender_str.replace(&split.gender, "")
                    );
                }
                if desired_number_str.contains(&split.number) {
                    desired_number_str = format!(
                        "{}{}",
                        split.number,
                        desired_number_str.replace(&split.number, "")
                    );
                }
            }
            for gender_ch in desired_gender_str.chars() {
                for number_ch in desired_number_str.chars() {
                    let desired_gender = gender_ch.to_string();
                    let desired_number = number_ch.to_string();
                    let mut suggestion_builder = String::new();
                    let mut ignore_this_suggestion = false;
                    if !keep_original {
                        let s = synthesize_with_gender_and_number(
                            &self.env,
                            atr_noun,
                            split_postag
                                .as_ref()
                                .unwrap_or(&GenderAndNumberSplit::default()),
                            &desired_gender,
                            &desired_number,
                        );
                        if s.is_empty() {
                            ignore_this_suggestion = true;
                        }
                        suggestion_builder.push_str(&s);
                    } else {
                        suggestion_builder.push_str(&atr_noun.token);
                    }
                    // backwards
                    let mut stop = false;
                    let mut i = pos_word;
                    let mut preposition_to_add = String::new();
                    let mut add_determiner = false;
                    let mut added_demonstrative = false;
                    let mut conditional_added_string = String::new();
                    let mut add_tot = String::new();
                    while !stop && i > 1 {
                        i -= 1;
                        let token = tokens[i];
                        let mut atr = reading_with_tag_regex(
                            token,
                            &split_gender_number_no_noun().to_string(),
                        );
                        if (!token.has_pos_tag_starting_with("D")
                            && (token.has_pos_tag("_perfet")
                                || token.has_pos_tag("_GV_")
                                || token.chunk_tags.iter().any(|c| c == "GV")))
                            || FORMS_TO_IGNORE.contains(&token.surface().to_lowercase().as_str())
                        {
                            atr = None;
                        }
                        let valid_atr = atr.filter(|a| a.pos_tag.is_some() && a.stem.is_some());
                        if let Some(atr) = valid_atr {
                            let pos_tag = atr.pos_tag.as_deref().unwrap_or("");
                            if pos_tag.starts_with("DA") {
                                suggestion_builder.insert_str(0, &conditional_added_string);
                                conditional_added_string.clear();
                                add_determiner = true;
                                start_pos = i;
                            } else if !add_determiner && !added_demonstrative {
                                let mut s = synthesize_with_gender_and_number(
                                    &self.env,
                                    atr,
                                    &split_gender_and_number(Some(atr)).unwrap_or_default(),
                                    &desired_gender,
                                    &desired_number,
                                );
                                if s.is_empty() {
                                    ignore_this_suggestion = true;
                                }
                                if s == "bo" {
                                    s = "bon".to_string();
                                }
                                suggestion_builder.insert_str(0, &conditional_added_string);
                                conditional_added_string.clear();
                                if tokens[i + 1].whitespace_before {
                                    suggestion_builder.insert(0, ' ');
                                }
                                suggestion_builder.insert_str(0, &s);
                                start_pos = i;
                                if pos_tag.starts_with("DD") {
                                    added_demonstrative = true;
                                }
                                if pos_tag.starts_with('D')
                                    && !pos_tag.starts_with("DN")
                                    && !added_demonstrative
                                    && !atr.lemma().eq_ignore_ascii_case("quant")
                                {
                                    stop = true;
                                }
                            } else {
                                // only before "el/aquest/aquell...": tota
                                // l'estona, tota aquella estona
                                if atr.lemma() == "tot" {
                                    let s = synthesize_with_gender_and_number(
                                        &self.env,
                                        atr,
                                        &split_gender_and_number(Some(atr)).unwrap_or_default(),
                                        &desired_gender,
                                        &desired_number,
                                    );
                                    if !s.is_empty() {
                                        add_tot = format!("{s} ");
                                        start_pos = i;
                                    }
                                }
                                stop = true;
                            }
                        } else if token.has_pos_tag("SPS00") || token.has_pos_tag("LOC_PREP") {
                            if add_determiner {
                                let mut preposition = token.surface().to_lowercase();
                                if preposition == "pe" {
                                    preposition = "per".to_string();
                                }
                                if preposition == "d'" {
                                    preposition = "de".to_string();
                                }
                                if preposition == "a" || preposition == "de" || preposition == "per"
                                {
                                    preposition_to_add = preposition;
                                    start_pos = i;
                                }
                            }
                            stop = true;
                        } else if token.has_pos_tag("_PUNCT_CONT") || token.has_pos_tag("CC") {
                            if pos_word - i == 1
                                || (i > 1
                                    && token.surface().eq_ignore_ascii_case("i")
                                    && tokens[i - 1].surface().eq_ignore_ascii_case("tot"))
                            {
                                stop = true;
                            } else {
                                conditional_added_string
                                    .insert_str(0, &format!("{} ", token.surface()));
                            }
                        } else if token.has_pos_tag_starting_with("RG")
                            && (i <= 1
                                || reading_with_tag_regex(
                                    tokens[i - 1],
                                    &split_gender_number().to_string(),
                                )
                                .is_some())
                            && (i >= tokens.len() - 1
                                || reading_with_tag_regex(
                                    tokens[i + 1],
                                    &split_gender_number().to_string(),
                                )
                                .is_some())
                        {
                            conditional_added_string
                                .insert_str(0, &format!("{} ", token.surface()));
                        } else {
                            stop = true;
                        }
                    }
                    // forwards
                    stop = false;
                    i = pos_word;
                    conditional_added_string.clear();
                    let mut is_there_conjunction = false;
                    while !stop && i < tokens.len() - 1 {
                        i += 1;
                        if FORMS_TO_IGNORE.contains(&tokens[i].surface().to_lowercase().as_str()) {
                            break;
                        }
                        let token = tokens[i];
                        let mut atr = reading_with_tag_regex(
                            token,
                            &split_gender_number_adjective().to_string(),
                        );
                        if is_there_conjunction && token.has_pos_tag_starting_with("NC") {
                            atr = None;
                        }
                        if let Some(atr) = atr {
                            let s = synthesize_with_gender_and_number(
                                &self.env,
                                atr,
                                &split_gender_and_number(Some(atr)).unwrap_or_default(),
                                &desired_gender,
                                &desired_number,
                            );
                            if s.is_empty() {
                                ignore_this_suggestion = true;
                            }
                            suggestion_builder.push_str(&conditional_added_string);
                            conditional_added_string.clear();
                            suggestion_builder.push(' ');
                            suggestion_builder.push_str(&s);
                            end_pos = i;
                        } else if token.has_pos_tag_starting_with("RG") {
                            conditional_added_string.push_str(&format!(" {}", token.surface()));
                        } else if token.has_pos_tag("CC") {
                            is_there_conjunction = true;
                            conditional_added_string.push_str(&format!(" {}", token.surface()));
                        } else if token.has_pos_tag("_PUNCT_CONT") {
                            conditional_added_string.push_str(token.surface());
                        } else {
                            stop = true;
                        }
                    }
                    if add_determiner {
                        let prefix = get_preposition_and_determiner(
                            &suggestion_builder,
                            &format!("{desired_gender}{desired_number}"),
                            &preposition_to_add,
                        );
                        suggestion_builder.insert_str(0, &prefix);
                    } else if !preposition_to_add.is_empty() {
                        suggestion_builder.insert_str(0, &format!("{preposition_to_add} "));
                    }
                    suggestion_builder.insert_str(0, &add_tot);
                    let original_span =
                        &src[tokens[start_pos].start_pos..tokens[end_pos].end_pos()];
                    let suggestion = preserve_case_word_by_word(&suggestion_builder, original_span);
                    if end_pos == pos_word
                        && start_pos == pos_word
                        && tokens[pos_word].surface() == suggestion
                    {
                        continue;
                    }
                    if !ignore_this_suggestion {
                        suggestions.push(suggestion);
                    }
                }
            }
        }

        if suggestions.is_empty() {
            return FilterOutcome::reject();
        }
        let range = TextRange::new(tokens[start_pos].start_pos, tokens[end_pos].end_pos());
        let original_str = &src[tokens[start_pos].start_pos..tokens[end_pos].end_pos()];
        if suggestions.iter().any(|s| s == original_str) {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: Some(range),
            message: None,
            suggestions: Some(
                suggestions
                    .into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect(),
            ),
        }
    }
}
