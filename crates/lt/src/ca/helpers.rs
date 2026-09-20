//! Catalan stage-3 helpers shared by the Java rule classes:
//! `PronomsFeblesHelper`, `VerbSynthesizer`, `VerbClassifier` and
//! `NounToVerbHelper` (internal development notes).
//!
//! Ported ahead of their XML consumers (the remaining stage-3 filters and
//! `QueIniciFilter`).
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings};

use crate::ca::adapt::adapt_suggestion;

// ---------------------------------------------------------------------------
// PronomsFeblesHelper
// ---------------------------------------------------------------------------

/// `PronomsFeblesHelper.PronounPosition` (ordinal order matters: the flat
/// table is indexed `7 * row + position`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PronounPosition {
    Davant,
    DavantApos,
    Darrere,
    DarrereApos,
    DarrereNoguionetNoapos,
    DarreAposNoguionetNoapos,
    Normalized,
}

impl PronounPosition {
    fn index(self) -> usize {
        match self {
            PronounPosition::Davant => 0,
            PronounPosition::DavantApos => 1,
            PronounPosition::Darrere => 2,
            PronounPosition::DarrereApos => 3,
            PronounPosition::DarrereNoguionetNoapos => 4,
            PronounPosition::DarreAposNoguionetNoapos => 5,
            PronounPosition::Normalized => 6,
        }
    }
}

/// `PronomsFeblesHelper.pronomsFebles` (rows of seven positions).
#[rustfmt::skip]
const PRONOMS_FEBLES: [&str; 630] = [
    "el", "l'", "-lo", "'l", "lo", "l", "el",
    "els el", "els l'", "-los-el", "'ls-el", "losel", "lsel", "els el",
    "els els", "els els", "-los-els", "'ls-els", "losels", "lsels", "els els",
    "els en", "els n'", "-los-en", "'ls-en", "losen", "lsen", "els en",
    "els hi", "els hi", "-los-hi", "'ls-hi", "loshi", "lshi", "els hi",
    "els ho", "els ho", "-los-ho", "'ls-ho", "losho", "lsho", "els ho",
    "els la", "els l'", "-los-la", "'ls-la", "losla", "lsla", "els la",
    "els les", "els les", "-los-les", "'ls-les", "losles", "lsles", "els les",
    "els", "els", "-los", "'ls", "los", "ls", "els",
    "em", "m'", "-me", "'m", "me", "m", "em",
    "en", "n'", "-ne", "'n", "ne", "n", "en",
    "ens el", "ens l'", "-nos-el", "'ns-el", "nosel", "nsel", "ens el",
    "ens els", "ens els", "-nos-els", "'ns-els", "nosels", "nsels", "ens els",
    "ens en", "ens n'", "-nos-en", "'ns-en", "nosen", "nsen", "ens en",
    "ens hi", "ens hi", "-nos-hi", "'ns-hi", "noshi", "nshi", "ens hi",
    "ens ho", "ens ho", "-nos-ho", "'ns-ho", "nosho", "nsho", "ens ho",
    "ens la", "ens l'", "-nos-la", "'ns-la", "nosla", "nsla", "ens la",
    "ens les", "ens les", "-nos-les", "'ns-les", "nosles", "nsles", "ens les",
    "ens li", "ens li", "-nos-li", "'ns-li", "nosli", "nsli", "ens li",
    "ens", "ens", "-nos", "'ns", "nos", "ns", "ens",
    "es", "s'", "-se", "'s", "se", "s", "es",
    "et", "t'", "-te", "'t", "te", "t", "et",
    "hi", "hi", "-hi", "-hi", "hi", "hi", "hi",
    "ho", "ho", "-ho", "-ho", "ho", "ho", "ho",
    "l'en", "el n'", "-l'en", "-l'en", "len", "len", "el en",
    "l'hi", "l'hi", "-l'hi", "-l'hi", "lhi", "lhi", "el hi",
    "la hi", "la hi", "-la-hi", "-la-hi", "lahi", "lahi", "la hi",
    "la", "l'", "-la", "-la", "la", "la", "la",
    "la'n", "la n'", "-la'n", "-la'n", "lan", "lan", "la en",
    "les en", "les n'", "-les-en", "-les-en", "lesen", "lesen", "les en",
    "les hi", "les hi", "-les-hi", "-les-hi", "leshi", "leshi", "les hi",
    "les", "les", "-les", "-les", "les", "les", "les",
    "li hi", "li hi", "-li-hi", "-li-hi", "lihi", "lihi", "li hi",
    "li ho", "li ho", "-li-ho", "-li-ho", "liho", "liho", "li ho",
    "li la", "li l'", "-li-la", "-li-la", "lila", "lila", "li la",
    "li les", "li les", "-li-les", "-li-les", "liles", "liles", "li les",
    "li", "li", "-li", "-li", "li", "li", "li",
    "li'l", "li l'", "-li'l", "-li'l", "lil", "lil", "li el",
    "li'ls", "li'ls", "-li'ls", "-li'ls", "lils", "lils", "li els",
    "li'n", "li n'", "-li'n", "-li'n", "lin", "lin", "li en",
    "m'hi", "m'hi", "-m'hi", "-m'hi", "mhi", "mhi", "em hi",
    "m'ho", "m'ho", "-m'ho", "-m'ho", "mho", "mho", "em ho",
    "me la", "me l'", "-me-la", "-me-la", "mela", "mela", "em la",
    "me les", "me les", "-me-les", "-me-les", "meles", "meles", "em les",
    "me li", "me li", "-me-li", "-me-li", "meli", "meli", "em li",
    "me'l", "me l'", "-me'l", "-me'l", "mel", "mel", "em el",
    "me'ls", "me'ls", "-me'ls", "-me'ls", "mels", "mels", "em els",
    "me'n", "me n'", "-me'n", "-me'n", "men", "men", "em en",
    "n'hi", "n'hi", "-n'hi", "-n'hi", "nhi", "nhi", "en hi",
    "s'hi", "s'hi", "-s'hi", "-s'hi", "shi", "shi", "es hi",
    "s'ho", "s'ho", "-s'ho", "-s'ho", "sho", "sho", "es ho",
    "se la", "se l'", "-se-la", "-se-la", "sela", "sela", "es la",
    "se les", "se les", "-se-les", "-se-les", "seles", "seles", "es les",
    "se li", "se li", "-se-li", "-se-li", "seli", "seli", "es li",
    "se us", "se us", "-se-us", "-se-us", "seus", "seus", "es us",
    "se vos", "se vos", "-se-vos", "-se-vos", "sevos", "sevos", "es vos",
    "se'l", "se l'", "-se'l", "-se'l", "sel", "sel", "es el",
    "se'ls", "se'ls", "-se'ls", "-se'ls", "sels", "sels", "es els",
    "se'm", "se m'", "-se'm", "-se'm", "sem", "sem", "es em",
    "se'n", "se n'", "-se'n", "-se'n", "sen", "sen", "es en",
    "se'ns", "se'ns", "-se'ns", "-se'ns", "sens", "sens", "es ens",
    "se't", "se t'", "-se't", "-se't", "set", "set", "es et",
    "t'hi", "t'hi", "-t'hi", "-t'hi", "thi", "thi", "et hi",
    "t'ho", "t'ho", "-t'ho", "-t'ho", "tho", "tho", "et ho",
    "te la", "te l'", "-te-la", "-te-la", "tela", "tela", "et la",
    "te les", "te les", "-te-les", "-te-les", "teles", "teles", "et les",
    "te li", "te li", "-te-li", "-te-li", "teli", "teli", "et li",
    "te'l", "te l'", "-te'l", "-te'l", "tel", "tel", "et el",
    "te'ls", "te'ls", "-te'ls", "-te'ls", "tels", "tels", "et els",
    "te'm", "te m'", "-te'm", "-te'm", "tem", "tem", "et em",
    "te'n", "te n'", "-te'n", "-te'n", "ten", "ten", "et en",
    "te'ns", "te'ns", "-te'ns", "-te'ns", "tens", "tens", "et ens",
    "us el", "us l'", "-vos-el", "-us-el", "vosel", "usel", "us el",
    "us els", "us els", "-vos-els", "-us-els", "vosels", "usels", "us els",
    "us em", "us m'", "-vos-em", "-us-em", "vosem", "usem", "us em",
    "us en", "us n'", "-vos-en", "-us-en", "vosen", "usen", "us en",
    "us ens", "us ens", "-vos-ens", "-us-ens", "vosens", "usens", "us ens",
    "us hi", "us hi", "-vos-hi", "-us-hi", "voshi", "ushi", "us hi",
    "us ho", "us ho", "-vos-ho", "-us-ho", "vosho", "usho", "us ho",
    "us la", "us l'", "-vos-la", "-us-la", "vosla", "usla", "us la",
    "us les", "us les", "-vos-les", "-us-les", "vosles", "usles", "us les",
    "us li", "us li", "-vos-li", "-us-li", "vosli", "usli", "us li",
    "us", "us", "-vos", "-us", "vos", "us", "us",
    "se me'n", "se me n'", "-se-me'n", "-se-me'n", "semen", "semen", "es em en",
    "se te'n", "se te n'", "-se-te'n", "-se-te'n", "seten", "seten", "es et en",
    "se li'n", "se li n'", "-se-li'n", "-se-li'n", "selin", "selin", "es li en",
    "se'ns en", "se'ns n'", "-se'ns-en", "-se'ns-en", "sensen", "sensen", "es ens en",
    "se us en", "se us n'", "-se-us-en", "-se-us-en", "seusen", "seusen", "es us en",
    "se vos en", "se vos n'", "-se-vos-en", "-se-vos-en", "sevosen", "sevosen", "es vos en",
    "se'ls en", "se'ls n'", "-se'ls-en", "-se'ls-en", "selsen", "selsen", "es els en",
];

/// `PronomsFeblesHelper.incorrectOrders`.
fn incorrect_orders() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("me se", "se'm"),
            ("me s'", "se m'"),
            ("te se", "se't"),
            ("te s'", "se t'"),
            ("li se", "se li"),
            ("li s'", "se li"),
            ("mi", "m'hi"),
            ("si", "s'hi"),
            ("nosi", "-nos-hi"),
            ("losi", "-los-hi"),
            ("lis", "els"),
            ("m'en", "me'n"),
            ("t'en", "te'n"),
            ("s'en", "se'n"),
        ])
    })
}

fn reflexive_pronoun() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("1S", "em"),
            ("2S", "et"),
            ("3S", "es"),
            ("1P", "ens"),
            ("2P", "us"),
            ("3P", "es"),
        ])
    })
}

fn dative_pronoun() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("1S", "em"),
            ("2S", "et"),
            ("3S", "li"),
            ("3C", "li"),
            ("1P", "ens"),
            ("2P", "us"),
            ("3P", "els"),
        ])
    })
}

pub fn get_reflexive_pronoun(key: &str) -> String {
    reflexive_pronoun()
        .get(key)
        .copied()
        .unwrap_or("")
        .to_string()
}

pub fn get_dative_pronoun(key: &str) -> String {
    dative_pronoun().get(key).copied().unwrap_or("").to_string()
}

/// `PronomsFeblesHelper.pPronomFeble`: reading POS-tag regex.
pub fn is_pronom_feble(token: &AnalyzedTokenReadings) -> bool {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"^(?:P0.{6}|PP3CN000|PP3NN000|PP3..A00|PP[123]CP000|PP3CSD00)$").unwrap()
    });
    token.has_pos_tag_matching(re)
}

/// `PronomsFeblesHelper.transform`.
pub fn transform(input_pronom: &str, pronoun_pos: PronounPosition) -> String {
    let mut input = input_pronom.to_lowercase();
    input = input.trim().to_string();
    if let Some(fixed) = incorrect_orders().get(input.as_str()) {
        input = (*fixed).to_string();
    }
    let mut i = 0usize;
    while i < PRONOMS_FEBLES.len() && !input.eq_ignore_ascii_case(PRONOMS_FEBLES[i]) {
        i += 1;
    }
    let positions = 7usize;
    let pf_pos = positions * (i / positions) + pronoun_pos.index();
    if pf_pos > PRONOMS_FEBLES.len() - 1 {
        // pronom inexistent, p.ex. -t
        return String::new();
    }
    let mut pronom = PRONOMS_FEBLES[pf_pos].to_string();
    if pronoun_pos == PronounPosition::Davant
        || (pronoun_pos == PronounPosition::DavantApos && !pronom.ends_with('\''))
    {
        pronom.push(' ');
    }
    pronom
}

/// `PronomsFeblesHelper.pApostropheNeeded`.
pub fn apostrophe_needed() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new("(?i)^h?[aeiouàèéíòóú].*$").unwrap())
}

/// `PronomsFeblesHelper.pApostropheNeededEnd`.
pub fn apostrophe_needed_end() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new("(?i)^.*[aei]$").unwrap())
}

/// `PronomsFeblesHelper.transformDavant`.
pub fn transform_davant(input_pronom: &str, next_word: &str) -> String {
    let next_word = next_word.to_lowercase();
    if apostrophe_needed().is_match(&next_word) {
        return transform(input_pronom, PronounPosition::DavantApos);
    }
    let pronom = transform(input_pronom, PronounPosition::Davant);
    // se senten, se cenyeix...
    if pronom == "es "
        && (next_word.starts_with('s')
            || next_word.starts_with("ce")
            || next_word.starts_with("ci"))
    {
        return "se ".to_string();
    }
    pronom
}

/// `PronomsFeblesHelper.transformDarrere`.
pub fn transform_darrere(input_pronom: &str, previous_word: &str) -> String {
    if apostrophe_needed_end().is_match(previous_word) {
        transform(input_pronom, PronounPosition::DarrereApos)
    } else {
        transform(input_pronom, PronounPosition::Darrere)
    }
}

/// `PronomsFeblesHelper.doAddPronounEn`.
pub fn do_add_pronoun_en(pronouns_str: &str, verb_str: &str, pronouns_after: bool) -> String {
    let mut pronoun_normalized = transform(pronouns_str, PronounPosition::Normalized);
    if pronoun_normalized.ends_with("hi") {
        pronoun_normalized = pronoun_normalized.replace("hi", "en hi");
    } else {
        pronoun_normalized += " en";
    }
    if pronouns_after {
        transform_darrere(&pronoun_normalized, verb_str)
    } else {
        transform_davant(&pronoun_normalized, verb_str)
    }
}

/// `PronomsFeblesHelper.doRemovePronounReflexive`.
pub fn do_remove_pronoun_reflexive(
    pronouns_str: &str,
    verb_str: &str,
    pronouns_after: bool,
) -> String {
    static REFLEXIVE_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = REFLEXIVE_RE.get_or_init(|| regex::Regex::new("(?i)(em|et|es|ens|us|vos)").unwrap());
    let pronouns_replacement = re
        .replace(
            &transform(&pronouns_str.to_lowercase(), PronounPosition::Normalized),
            "",
        )
        .trim()
        .to_string();
    if pronouns_after {
        let mut replacement = verb_str.to_string();
        let pronouns_replacement = transform_darrere(&pronouns_replacement, verb_str);
        if !pronouns_replacement.is_empty() {
            replacement = format!("{verb_str}{pronouns_replacement}");
        }
        return replacement;
    }
    let mut replacement = verb_str.to_string();
    let pronouns_replacement = transform_davant(&pronouns_replacement, verb_str);
    if !pronouns_replacement.is_empty() {
        replacement = format!("{pronouns_replacement}{verb_str}");
    }
    replacement
}

/// `PronomsFeblesHelper.pContainsReflexivePronoun` (contains semantics).
fn contains_reflexive_pronoun_pattern(pronouns_str: &str) -> bool {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new("(?i).*([mts][e']|[e'][mts]|vos|us|ens|-nos|-vos).*").unwrap()
    });
    re.is_match(pronouns_str)
}

/// `PronomsFeblesHelper.lReflexivePronouns`.
const L_REFLEXIVE_PRONOUNS: [&str; 6] = ["em", "et", "es", "ens", "us", "vos"];

fn contains_any_reflexive_pronoun(pronouns_str: &str) -> bool {
    let normalized = transform(pronouns_str, PronounPosition::Normalized);
    normalized
        .split(' ')
        .any(|p| L_REFLEXIVE_PRONOUNS.contains(&p))
}

/// `PronomsFeblesHelper.doAddPronounReflexive`.
pub fn do_add_pronoun_reflexive(
    pronouns_str: &str,
    verb_str: &str,
    first_verb_persona_number: &str,
    pronouns_after: bool,
) -> String {
    if pronouns_after {
        if contains_reflexive_pronoun_pattern(&pronouns_str.to_lowercase()) {
            return format!("{verb_str}{}", transform_darrere(pronouns_str, verb_str));
        }
        if verb_str.ends_with('r') || verb_str.ends_with("re") {
            return format!("{verb_str}{}", transform_darrere("-se", verb_str));
        }
        return verb_str.to_string();
    }
    let mut pronoun_to_add = transform(pronouns_str, PronounPosition::Normalized);
    if !contains_any_reflexive_pronoun(&pronouns_str.to_lowercase()) {
        pronoun_to_add = format!(
            "{} {pronoun_to_add}",
            get_reflexive_pronoun(first_verb_persona_number)
        );
    }
    format!("{}{verb_str}", transform_davant(&pronoun_to_add, verb_str))
}

/// `PronomsFeblesHelper.doAddPronounReflexiveEn`.
pub fn do_add_pronoun_reflexive_en(
    pronouns_str: &str,
    verb_str: &str,
    first_verb_persona_number: &str,
    pronouns_after: bool,
) -> String {
    if pronouns_after {
        if contains_reflexive_pronoun_pattern(&pronouns_str.to_lowercase()) {
            return format!(
                "{verb_str}{}",
                transform_darrere(&format!("{pronouns_str}'n"), verb_str)
            );
        }
        return format!("{verb_str}{}", transform_darrere("-se'n", verb_str));
    }
    if pronouns_str.is_empty() {
        let pronoun_to_add = transform_davant(
            &format!("{} en", get_reflexive_pronoun(first_verb_persona_number)),
            verb_str,
        );
        return format!("{pronoun_to_add}{verb_str}");
    }
    let pronoun_to_add = transform_davant(
        &format!(
            "es {} en",
            transform(pronouns_str, PronounPosition::Normalized)
        ),
        verb_str,
    );
    if !pronoun_to_add.is_empty() {
        format!("{pronoun_to_add}{verb_str}")
    } else {
        format!("{}{verb_str}", transform_davant(pronouns_str, verb_str))
    }
}

/// `PronomsFeblesHelper.doAddPronounReflexiveImperative`.
pub fn do_add_pronoun_reflexive_imperative(
    pronouns_str: &str,
    verb_str: &str,
    first_verb_persona_number: &str,
) -> String {
    if pronouns_str.is_empty() {
        let pronoun_to_add =
            transform_darrere(&get_reflexive_pronoun(first_verb_persona_number), verb_str);
        if !pronoun_to_add.is_empty() {
            return format!("{verb_str}{pronoun_to_add}");
        }
    }
    String::new()
}

/// `PronomsFeblesHelper.doReplaceEmEn`.
pub fn do_replace_em_en(pronouns_str: &str, verb_str: &str, _pronouns_after: bool) -> String {
    if pronouns_str.eq_ignore_ascii_case("em") {
        return format!("en {verb_str}");
    }
    if pronouns_str.eq_ignore_ascii_case("m'") {
        return format!("n'{verb_str}");
    }
    if pronouns_str.eq_ignore_ascii_case("m'hi") {
        return format!("n'hi {verb_str}");
    }
    String::new()
}

/// `PronomsFeblesHelper.doReplaceHiEn` (identical to `doReplaceEmEn`
/// upstream).
pub fn do_replace_hi_en(pronouns_str: &str, verb_str: &str, _pronouns_after: bool) -> String {
    if pronouns_str.eq_ignore_ascii_case("hi") {
        return format!("en {verb_str}");
    }
    if pronouns_str.eq_ignore_ascii_case("m'") {
        return format!("n'{verb_str}");
    }
    if pronouns_str.eq_ignore_ascii_case("m'hi") {
        return format!("n'hi {verb_str}");
    }
    String::new()
}

/// `PronomsFeblesHelper.convertPronounsForIntransitiveVerb`.
pub fn convert_pronouns_for_intransitive_verb(s: &str) -> String {
    s.replace("-se'l", "-se-li")
        .replace("se'l ", "se li ")
        .replace("l'", "li ")
        .replace("-lo", "-li")
        .replace("-la", "-li")
        .replace("la ", "li ")
        .replace("el ", "li ")
        .replace("ho", "hi")
}

/// `PronomsFeblesHelper.fixApostrophes`.
pub fn fix_apostrophes(s: &str) -> String {
    static DE_WRONG: OnceLock<regex::Regex> = OnceLock::new();
    static MISSING: OnceLock<regex::Regex> = OnceLock::new();
    static WRONG: OnceLock<regex::Regex> = OnceLock::new();
    static WRONG_HYPHEN: OnceLock<regex::Regex> = OnceLock::new();
    let de_wrong = DE_WRONG.get_or_init(|| regex::Regex::new("(?i)^(?:.*d'[^aeiouh].*)$").unwrap());
    let missing = MISSING
        .get_or_init(|| regex::Regex::new("(?i)^(?:(.*)\\be([stm]) (h?[aeiouh].*))$").unwrap());
    let wrong = WRONG.get_or_init(|| regex::Regex::new("(?i)^(?:([mts])'([^aeiouh].*))$").unwrap());
    let wrong_hyphen =
        WRONG_HYPHEN.get_or_init(|| regex::Regex::new("(?i)^(?:(.*)(-[stm])e-(h[oi]))$").unwrap());
    let mut s = s.to_string();
    if de_wrong.is_match(&s) {
        s = s.replace("d'", "de ");
    }
    if let Some(caps) = missing.captures(&s) {
        s = format!("{}{}'{}", &caps[1], &caps[2], &caps[3]);
    }
    if let Some(caps) = wrong.captures(&s) {
        s = format!("e{} {}", &caps[1], &caps[2]);
    }
    if let Some(caps) = wrong_hyphen.captures(&s) {
        s = format!("{}{}'{}", &caps[1], &caps[2], &caps[3]);
    }
    s
}

// ---------------------------------------------------------------------------
// VerbSynthesizer
// ---------------------------------------------------------------------------

/// Reading whose POS tag fully matches `regexp` (LT
/// `AnalyzedTokenReadings.readingWithTagRegex`). `None` tags never match.
pub fn reading_with_tag_regex<'a>(
    token: &'a AnalyzedTokenReadings,
    regexp: &str,
) -> Option<&'a AnalyzedToken> {
    let re = regex::Regex::new(&format!("^(?:{regexp})$")).ok()?;
    token
        .readings
        .iter()
        .find(|r| r.pos_tag.as_deref().is_some_and(|t| re.is_match(t)))
}

/// `AnalyzedTokenReadings.hasPartialPosTag` (substring).
pub fn has_partial_pos_tag(token: &AnalyzedTokenReadings, pos_tag: &str) -> bool {
    token
        .readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.contains(pos_tag)))
}

/// `org.languagetool.synthesis.ca.VerbSynthesizer`: synthesize a new verb
/// (single- or multi-word) from a lemma and a POS tag taken from the tokens.
pub struct VerbSynthesizer<'a> {
    /// LT `getTokensWithoutWhitespace()` (index 0 is SENT_START).
    tokens: &'a [&'a AnalyzedTokenReadings],
    i_first_verb: isize,
    i_last_verb: isize,
    new_lemma: Option<String>,
    new_postag: Option<String>,
    num_pronouns_before: isize,
    num_pronouns_after: isize,
    search_backward: bool,
    synth: &'a lt_tagger::CatalanSynthesizer,
}

impl<'a> VerbSynthesizer<'a> {
    pub fn new(
        tokens: &'a [&'a AnalyzedTokenReadings],
        start_pos: usize,
        synth: &'a lt_tagger::CatalanSynthesizer,
        search_backward: bool,
    ) -> Self {
        let mut s = Self {
            tokens,
            i_first_verb: -1,
            i_last_verb: -1,
            new_lemma: None,
            new_postag: None,
            num_pronouns_before: -1,
            num_pronouns_after: -1,
            search_backward,
            synth,
        };
        s.set_indexes(start_pos as isize);
        s
    }

    pub fn set_lemma_and_postag(&mut self, lemma: &str, postag: &str) {
        self.new_lemma = Some(lemma.to_string());
        self.new_postag = Some(postag.to_string());
    }

    /// `setPostag`: lemma from the last verb token, the given POS tag.
    pub fn set_postag(&mut self, postag: &str) {
        self.new_lemma = self
            .reading_at(self.i_last_verb, "V.*")
            .map(|r| r.lemma().to_string());
        self.new_postag = Some(postag.to_string());
    }

    /// `setLemma`: lemma as given, POS tag from the first verb token.
    pub fn set_lemma(&mut self, lemma: &str) {
        self.new_lemma = Some(lemma.to_string());
        self.new_postag = self
            .reading_at(self.i_first_verb, "V.*")
            .and_then(|r| r.pos_tag.clone());
    }

    fn reading_at(&self, i: isize, regexp: &str) -> Option<&'a AnalyzedToken> {
        if i < 0 || i as usize >= self.tokens.len() {
            return None;
        }
        reading_with_tag_regex(self.tokens[i as usize], regexp)
    }

    fn set_indexes(&mut self, start_pos: isize) {
        if start_pos < 0 || start_pos >= self.tokens.len() as isize {
            return;
        }
        let mut j = start_pos;
        // single participle
        if reading_with_tag_regex(self.tokens[j as usize], "V.P.*").is_some()
            && !self.tokens[j as usize].has_pos_tag("_GV_")
            && !self.tokens[j as usize].chunk_tags.iter().any(|c| c == "GV")
        {
            self.i_first_verb = j;
            self.i_last_verb = j;
            self.num_pronouns_before = 0;
            self.num_pronouns_after = 0;
            return;
        }
        // If it is not a verb, find the first one
        if self.search_backward {
            while j > 0 && !self.is_verb(j) {
                j -= 1;
            }
            let mut found_some_verb = false;
            while j > 0 && self.is_verb(j) {
                found_some_verb = true;
                j -= 1;
            }
            if found_some_verb {
                j += 1;
            }
        } else {
            while (j as usize) < self.tokens.len() && !self.is_verb(j) {
                j += 1;
            }
        }

        if self.is_verb(j) {
            self.i_first_verb = j;
            self.i_last_verb = j;
            // enrere
            let mut i = j - 1;
            while self.is_multitoken_verb(i) && !self.is_first_verb_is() {
                self.i_first_verb = i;
                i -= 1;
            }
            // avant
            i = j + 1;
            while self.is_multitoken_verb(i) && !(self.is_first_verb_is() && self.is_verb_is(i)) {
                self.i_last_verb = i;
                i += 1;
            }
        } else {
            return;
        }

        let mut i = 1isize;
        let mut pronouns_after = 0isize;
        while self.i_last_verb + i < self.tokens.len() as isize
            && !self.tokens[(self.i_last_verb + i) as usize].whitespace_before
            && is_pronom_feble(self.tokens[(self.i_last_verb + i) as usize])
        {
            pronouns_after += 1;
            i += 1;
        }
        self.num_pronouns_after = pronouns_after;

        i = -1;
        let mut pronouns_before_no_space_before = 0isize;
        let mut pronouns_before = 0isize;
        while self.i_first_verb + i > 0
            && is_pronom_feble(self.tokens[(self.i_first_verb + i) as usize])
        {
            let idx = self.i_first_verb + i;
            if self.tokens[idx as usize].whitespace_before
                || idx == 1
                || self.tokens[(idx - 1) as usize].has_pos_tag_starting_with("_QM")
            {
                pronouns_before += pronouns_before_no_space_before + 1;
                pronouns_before_no_space_before = 0;
            } else {
                pronouns_before_no_space_before += 1;
            }
            i -= 1;
        }
        self.num_pronouns_before = pronouns_before;
    }

    fn is_verb(&self, i: isize) -> bool {
        if i < 0 || i > self.tokens.len() as isize - 1 {
            return false; // out of bounds
        }
        let token = self.tokens[i as usize];
        token.chunk_tags.iter().any(|c| c == "GV")
            || reading_with_tag_regex(token, "V.[^P].*").is_some()
            || (reading_with_tag_regex(token, "V.P.*").is_some() && token.has_pos_tag("_GV_"))
    }

    fn is_multitoken_verb(&self, i: isize) -> bool {
        if i < 0 || i > self.tokens.len() as isize - 1 {
            return false; // out of bounds
        }
        let token = self.tokens[i as usize];
        token.chunk_tags.iter().any(|c| c == "GV") || token.has_pos_tag("_GV_")
    }

    /// `VerbSynthesizer.synthesize`.
    pub fn synthesize(&self) -> String {
        let mut result = String::new();
        let first_verb = self.reading_at(self.i_first_verb, "V.*");
        if self.i_first_verb == self.i_last_verb {
            let (Some(new_lemma), Some(new_postag)) = (&self.new_lemma, &self.new_postag) else {
                return result;
            };
            let postag = adjust_postag_to_lemma(new_lemma, new_postag);
            let token = AnalyzedToken::new("", Some(new_lemma.clone()), Some(new_postag.clone()));
            let synthesized = self.synth.synthesize(&token, &postag, false);
            if let Some(first) = synthesized.first() {
                result.push_str(first);
            }
        } else {
            for i in self.i_first_verb..=self.i_last_verb {
                let token = self.tokens[i as usize];
                if i == self.i_first_verb {
                    let Some(first_verb) = first_verb else {
                        continue;
                    };
                    let Some(new_postag) = &self.new_postag else {
                        continue;
                    };
                    let postag = adjust_postag_to_lemma(
                        first_verb.stem.as_deref().unwrap_or(&first_verb.token),
                        new_postag,
                    );
                    let synthesized = self.synth.synthesize(first_verb, &postag, false);
                    if let Some(first) = synthesized.first() {
                        result.push_str(first);
                    }
                } else if i == self.i_last_verb {
                    if token.whitespace_before {
                        result.push(' ');
                    }
                    let postag = self
                        .reading_at(self.i_last_verb, "V.*")
                        .and_then(|r| r.pos_tag.clone());
                    let (Some(new_lemma), Some(postag)) = (&self.new_lemma, postag) else {
                        continue;
                    };
                    let token =
                        AnalyzedToken::new("", Some(new_lemma.clone()), Some(postag.clone()));
                    let synthesized = self.synth.synthesize(
                        &token,
                        &adjust_postag_to_lemma(new_lemma, &postag),
                        false,
                    );
                    if let Some(first) = synthesized.first() {
                        result.push_str(first);
                    }
                } else {
                    if token.whitespace_before {
                        result.push(' ');
                    }
                    result.push_str(token.surface());
                }
            }
        }
        adapt_suggestion(&result, "")
    }

    pub fn get_string_from_to(&self, start: isize, end: isize) -> String {
        let mut sb = String::new();
        for i in start..=end {
            if i > start && self.tokens[i as usize].whitespace_before {
                sb.push(' ');
            }
            sb.push_str(self.tokens[i as usize].surface());
        }
        sb
    }

    pub fn get_pronouns_str_before(&self) -> String {
        self.get_string_from_to(
            self.i_first_verb - self.num_pronouns_before,
            self.i_first_verb - 1,
        )
    }

    pub fn get_pronouns_str_after(&self) -> String {
        self.get_string_from_to(
            self.i_last_verb + 1,
            self.i_last_verb + self.num_pronouns_after,
        )
    }

    pub fn get_whole_original_str(&self) -> String {
        self.get_string_from_to(
            self.i_first_verb - self.num_pronouns_before,
            self.i_last_verb + self.num_pronouns_after,
        )
    }

    pub fn get_verb_str(&self) -> String {
        self.get_string_from_to(self.i_first_verb, self.i_last_verb)
    }

    pub fn get_first_verb_index(&self) -> isize {
        self.i_first_verb
    }

    pub fn get_last_verb_index(&self) -> isize {
        self.i_last_verb
    }

    pub fn get_last_index(&self) -> isize {
        self.i_last_verb + self.num_pronouns_after
    }

    pub fn get_num_pronouns_after(&self) -> isize {
        self.num_pronouns_after
    }

    pub fn get_num_pronouns_before(&self) -> isize {
        self.num_pronouns_before
    }

    pub fn get_first_verb_persona_number(&self) -> String {
        self.reading_at(self.i_first_verb, "V.[SIM].*")
            .and_then(|r| r.pos_tag.as_deref())
            .and_then(|t| t.get(4..6))
            .unwrap_or("")
            .to_string()
    }

    pub fn get_first_verb_persona_number_imperative(&self) -> String {
        self.reading_at(self.i_first_verb, "V.M.*")
            .and_then(|r| r.pos_tag.as_deref())
            .and_then(|t| t.get(4..6))
            .unwrap_or("")
            .to_string()
    }

    pub fn is_first_verb_is(&self) -> bool {
        if self.i_first_verb == -1 {
            return false;
        }
        self.reading_at(self.i_first_verb, "V.[IS].*").is_some()
    }

    pub fn get_first_verb_is_postag(&self) -> Option<String> {
        if self.i_first_verb == -1 {
            return None;
        }
        self.reading_at(self.i_first_verb, "V.[IS].*")
            .and_then(|r| r.pos_tag.clone())
    }

    fn is_verb_is(&self, i: isize) -> bool {
        if i < 0 || i >= self.tokens.len() as isize {
            return false;
        }
        reading_with_tag_regex(self.tokens[i as usize], "V.[IS].*").is_some()
    }

    pub fn get_casing_model(&self) -> String {
        self.get_string_from_to(
            self.i_first_verb - self.num_pronouns_before,
            self.i_first_verb,
        )
    }

    pub fn is_undefined(&self) -> bool {
        self.i_first_verb == -1
            || self.i_last_verb == -1
            || self.num_pronouns_after == -1
            || self.num_pronouns_before == -1
    }

    pub fn is_passat_perifrastic(&self) -> bool {
        if self.i_first_verb < 1 || self.i_first_verb + 1 > self.tokens.len() as isize - 1 {
            return false;
        }
        let first = self.tokens[self.i_first_verb as usize];
        let second = self.tokens[(self.i_first_verb + 1) as usize];
        first.has_pos_tag_starting_with("VA")
            && first.has_lemma("anar")
            && (has_partial_pos_tag(second, "VMN")
                || has_partial_pos_tag(second, "VSN")
                || has_partial_pos_tag(second, "VAN"))
    }

    pub fn is_perfet(&self) -> bool {
        if self.i_first_verb < 1 || self.i_first_verb + 1 > self.tokens.len() as isize - 1 {
            return false;
        }
        let first = self.tokens[self.i_first_verb as usize];
        let second = self.tokens[(self.i_first_verb + 1) as usize];
        first.has_pos_tag_starting_with("VA")
            && first.has_lemma("haver")
            && (has_partial_pos_tag(second, "VMP")
                || has_partial_pos_tag(second, "VSP")
                || has_partial_pos_tag(second, "VAP"))
    }
}

/// `VerbSynthesizer.adjustPostagTolemma`.
fn adjust_postag_to_lemma(lemma: &str, postag: &str) -> String {
    if lemma == "haver" {
        return format!("VA{}", postag.get(2..).unwrap_or(""));
    }
    if lemma == "ser" {
        return format!("VS{}", postag.get(2..).unwrap_or(""));
    }
    postag.to_string()
}

// ---------------------------------------------------------------------------
// VerbClassifier
// ---------------------------------------------------------------------------

pub const VTR_CODE: u8 = 0b0000_0001;
pub const INTR_CODE: u8 = 0b0000_0010;
pub const PRON_CODE: u8 = 0b0000_0100;

/// `org.languagetool.tagging.ca.VerbClassifier` over
/// `ca/words/verbs_classification.txt`.
pub struct VerbClassifier {
    map: HashMap<String, u8>,
}

impl VerbClassifier {
    pub fn load(data_dir: &Path) -> Self {
        let mut map = HashMap::new();
        if let Ok(text) =
            lt_data::fs::read_to_string(data_dir.join("ca/words/verbs_classification.txt"))
        {
            for line in text.lines() {
                let line = line.split('#').next().unwrap_or("").trim();
                if line.is_empty() {
                    continue;
                }
                let Some((lemma, tags)) = line.split_once('=') else {
                    continue;
                };
                let mut code = 0u8;
                if tags.contains("vtr") {
                    code |= VTR_CODE;
                }
                if tags.contains("intr") || tags.contains("abs") {
                    code |= INTR_CODE;
                }
                if tags.contains("pron") {
                    code |= PRON_CODE;
                }
                map.insert(lemma.to_string(), code);
            }
        }
        Self { map }
    }

    pub fn is_verb_code(&self, lemma: &str, code_byte: u8) -> bool {
        self.map
            .get(lemma)
            .is_some_and(|code| (code & code_byte) != 0)
    }

    pub fn is_intransitive(&self, lemma: &str) -> bool {
        self.is_verb_code(lemma, INTR_CODE) && !self.is_verb_code(lemma, VTR_CODE)
    }
}

// ---------------------------------------------------------------------------
// NounToVerbHelper
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ca.NounToVerbHelper.NounToVerb`.
pub fn get_verb_from_noun(noun: &str) -> Option<&'static str> {
    const NOUN_TO_VERB: [(&str, &str); 167] = [
        ("acceptació", "acceptar"),
        ("acumulació", "acumular"),
        ("adaptació", "adaptar"),
        ("adhesió", "adherir"),
        ("admiració", "admirar"),
        ("adopció", "adoptar"),
        ("afectació", "afectar"),
        ("agregació", "agregar"),
        ("agressió", "agredir"),
        ("ajuda", "ajudar"),
        ("ampliació", "ampliar"),
        ("anàlisi", "analitzar"),
        ("aplicació", "aplicar"),
        ("aprovació", "aprovar"),
        ("arribada", "arribar"),
        ("ascensió", "ascendir"),
        ("assignació", "assignar"),
        ("associació", "associar"),
        ("atracció", "atreure"),
        ("augment", "augmentar"),
        ("avaluació", "avaluar"),
        ("cessió", "cedir"),
        ("circulació", "circular"),
        ("citació", "citar"),
        ("col·laboració", "col·laborar"),
        ("col·locació", "col·locar"),
        ("comparació", "comparar"),
        ("compensació", "compensar"),
        ("competició", "competir"),
        ("composició", "compondre"),
        ("compra", "comprar"),
        ("comprensió", "comprendre"),
        ("comunicació", "comunicar"),
        ("concreció", "concretar"),
        ("conducció", "conduir"),
        ("confecció", "confeccionar"),
        ("confirmació", "confirmar"),
        ("connexió", "connectar"),
        ("consideració", "considerar"),
        ("construcció", "construir"),
        ("consulta", "consultar"),
        ("contaminació", "contaminar"),
        ("contractació", "contractar"),
        ("contribució", "contribuir"),
        ("conversió", "convertir"),
        ("convocatòria", "convocar"),
        ("creació", "crear"),
        ("càlcul", "calcular"),
        ("declaració", "declarar"),
        ("dedicació", "dedicar"),
        ("definició", "definir"),
        ("demostració", "demostrar"),
        ("dependència", "dependre"),
        ("deposició", "depondre"),
        ("descripció", "descriure"),
        ("designació", "designar"),
        ("desocupació", "desocupar"),
        ("detecció", "detectar"),
        ("determinació", "determinar"),
        ("devolució", "tornar"),
        ("difusió", "difondre"),
        ("direcció", "dirigir"),
        ("distribució", "distribuir"),
        ("documentació", "documentar"),
        ("donació", "donar"),
        ("dubte", "dubtar"),
        ("educació", "educar"),
        ("elaboració", "elaborar"),
        ("elecció", "elegir"),
        ("eliminació", "eliminar"),
        ("emissió", "emetre"),
        ("ensenyament", "ensenyar"),
        ("entrada", "entrar"),
        ("enumeració", "enumerar"),
        ("enviament", "enviar"),
        ("evacuació", "evacuar"),
        ("exploració", "explorar"),
        ("exposició", "exposar"),
        ("extinció", "extingir"),
        ("fabricació", "fabricar"),
        ("formulació", "formular"),
        ("gestió", "gestionar"),
        ("identificació", "identificar"),
        ("il·lustració", "il·lustrar"),
        ("implantació", "implantar"),
        ("implementació", "implementar"),
        ("importació", "importar"),
        ("impressió", "imprimir"),
        ("inauguració", "inaugurar"),
        ("indicació", "indicar"),
        ("informació", "informar"),
        ("inspecció", "inspeccionar"),
        ("instal·lació", "instal·lar"),
        ("instrucció", "instruir"),
        ("integració", "integrar"),
        ("interpretació", "interpretar"),
        ("interrupció", "interrompre"),
        ("intervenció", "intervenir"),
        ("introducció", "introduir"),
        ("investigació", "investigar"),
        ("invitació", "invitar"),
        ("justificació", "justificar"),
        ("limitació", "limitar"),
        ("liquidació", "liquidar"),
        ("llista", "llistar"),
        ("localització", "localitzar"),
        ("manifestació", "manifestar"),
        ("manipulació", "manipular"),
        ("modificació", "modificar"),
        ("motivació", "motivar"),
        ("neteja", "netejar"),
        ("notificació", "notificar"),
        ("observació", "observar"),
        ("ocupació", "ocupar"),
        ("organització", "organitzar"),
        ("participació", "participar"),
        ("percepció", "percebre"),
        ("perdó", "perdonar"),
        ("pertorbació", "pertorbar"),
        ("planificació", "planificar"),
        ("preparació", "preparar"),
        ("prevenció", "prevenir"),
        ("producció", "produir"),
        ("programació", "programar"),
        ("prohibició", "prohibir"),
        ("protecció", "protegir"),
        ("publicació", "publicar"),
        ("qualificació", "qualificar"),
        ("quantificació", "quantificar"),
        ("recaptació", "recaptar"),
        ("recepció", "rebre"),
        ("reclamació", "reclamar"),
        ("recomanació", "recomanar"),
        ("reconstrucció", "reconstruir"),
        ("recuperació", "recuperar"),
        ("redacció", "redactar"),
        ("reducció", "reduir"),
        ("reflexió", "reflexionar"),
        ("reforma", "reformar"),
        ("regulació", "regular"),
        ("relació", "relacionar"),
        ("reparació", "reparar"),
        ("representació", "representar"),
        ("reproducció", "reproduir"),
        ("resolució", "resoldre"),
        ("resposta", "respondre"),
        ("restricció", "restringir"),
        ("revisió", "revisar"),
        ("salutació", "saludar"),
        ("selecció", "seleccionar"),
        ("separació", "separar"),
        ("significació", "significar"),
        ("sol·licitud", "sol·licitar"),
        ("substitució", "substituir"),
        ("suspensió", "suspendre"),
        ("temptació", "temptar"),
        ("traducció", "traduir"),
        ("tramitació", "tramitar"),
        ("transformació", "transformar"),
        ("transmissió", "transmetre"),
        ("transport", "transportar"),
        ("utilització", "usar"),
        ("valoració", "valorar"),
        ("venda", "vendre"),
        ("verificació", "verificar"),
        ("visita", "visitar"),
        ("votació", "votar"),
    ];
    let noun = noun.to_lowercase();
    NOUN_TO_VERB
        .iter()
        .find(|(n, _)| *n == noun.as_str())
        .map(|(_, v)| *v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_positions() {
        assert_eq!(transform("el", PronounPosition::Davant), "el ");
        assert_eq!(transform("el", PronounPosition::DavantApos), "l'");
        assert_eq!(transform("el", PronounPosition::Darrere), "-lo");
        assert_eq!(transform("el", PronounPosition::DarrereApos), "'l");
        assert_eq!(transform("el", PronounPosition::Normalized), "el");
        // incorrect order fix (pinned Java probe, 2026-09-20:
        // `PronomsFeblesHelper.transform("mi", NORMALIZED)` = "em hi")
        assert_eq!(transform("mi", PronounPosition::Normalized), "em hi");
        assert_eq!(transform("-t", PronounPosition::Normalized), "");
    }

    #[test]
    fn fix_apostrophes_cases() {
        // pinned Java probe (2026-09-20): fixApostrophes only rewrites the
        // hyphenated enclitic forms, not `d'home`/`me ha`
        assert_eq!(fix_apostrophes("d'home"), "d'home");
        assert_eq!(fix_apostrophes("m'ha"), "m'ha");
        assert_eq!(fix_apostrophes("me ha"), "me ha");
        assert_eq!(fix_apostrophes("menja-te-ho"), "menja-t'ho");
    }

    #[test]
    fn noun_to_verb_lookup() {
        assert_eq!(get_verb_from_noun("anàlisi"), Some("analitzar"));
        assert_eq!(get_verb_from_noun("Venda"), Some("vendre"));
        assert_eq!(get_verb_from_noun("inexistent"), None);
    }
}
