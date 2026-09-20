//! `org.languagetool.rules.ca.QueIniciFilter` (D-153): shared filter of the
//! Catalan sentence-initial `que`/`què` interrogative rules. It predicts
//! whether the interrogative should carry an accent (`què` = "quina cosa",
//! subject/direct object of the main verb) or not (`que`, yes/no question)
//! and then confirms or rejects the match according to the rule direction
//! (`AMBACCENT` = text "que" → suggest "què"; `SENSEACCENT` = text "què" →
//! suggest "que").
//!
//! Ported line by line from the pinned Java source
//! (`languagetool` 01d07e1f6165,
//! `languagetool-language-modules/ca/src/main/java/org/languagetool/rules/ca/QueIniciFilter.java`).

use std::sync::LazyLock;

use lt_core::AnalyzedTokenReadings;
use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::ca::filters::Env;
use crate::ca::helpers::{reading_with_tag_regex, transform, PronounPosition, VerbSynthesizer};
use crate::ca::verb_filters::tokens_without_whitespace;

macro_rules! anchored_re {
    ($name:ident, $pat:expr) => {
        #[allow(dead_code)]
        static $name: LazyLock<regex::Regex> =
            LazyLock::new(|| regex::Regex::new(concat!("^(?:", $pat, ")$")).unwrap());
    };
}

// `QueIniciFilter` patterns (Java `Pattern` constants and inline patterns).
anchored_re!(
    PRONOM_RE,
    "P0.{6}|PP3CN000|PP3NN000|PP3..A00|PP[123]CP000|PP3CSD00"
);
anchored_re!(FINITE_VERB_RE, "V.[SI].*");
anchored_re!(ANY_VERB_RE, "V.*");
anchored_re!(DET_RE, "D.*");
// Java constant, unused there (kept for the inventory).
anchored_re!(DET_SINGULAR_RE, "D...S.");
// Java constant, unused there (kept for the inventory).
anchored_re!(DET_PLURAL_RE, "D...P.");
// Java constant, unused there (kept for the inventory).
anchored_re!(
    VERB_OR_PRONOM_RE,
    "V.[SI].*|P0.{6}|PP3CN000|PP3NN000|PP3..A00|PP[123]CP000|PP3CSD00"
);
anchored_re!(TRAILING_DISLOCATION_RE, "D.*|N.*|A.*|PP3.*|PX.*|PD.*");
anchored_re!(VERB_3S_RE, "V.[SI].3S.*");
anchored_re!(VERB_3P_RE, "V.[SI].3P.*");
anchored_re!(VERB_2P_RE, "V.[SI].2P.*");
anchored_re!(ADVERB_RE, "R.|RG|RN");
anchored_re!(ADVERB_OR_I_RE, "R.|I");
anchored_re!(PROPER_NOUN_RE, "NP.*");
anchored_re!(COMMON_NOUN_RE, "NC.*");
anchored_re!(NOUN_SINGULAR_RE, "NC.S.*");
anchored_re!(NOUN_PLURAL_RE, "NC.P.*|NCCN000");
anchored_re!(STRONG_SUBJECT_RE, "PP[123].*|NP.*");
anchored_re!(
    COPULAR_ATTRIBUTE_RE,
    "A.*|V.P.*|N.*|D.*|PI.*|PX.*|Z.*|SPS.*"
);
anchored_re!(COMPLETIVE_CONTENT_RE, "D.*|N.*|PI.*|PX.*|PD.*|Z.*");
anchored_re!(SUBJECT_AFTER_FER_MAL_RE, "D.*|N.*|NP.*|PI.*");
anchored_re!(INFINITIVE_RE, "V.N.*");

/// `QueIniciFilter.PTIME_CHUNK`.
const PTIME_CHUNK: &str = "PTime";

/// `QueIniciFilter.SINO_LAST_WORDS`.
const SINO_LAST_WORDS: [&str; 2] = ["potser", "oi"];

/// `QueIniciFilter.COPULAR_VERBS`.
const COPULAR_VERBS: [&str; 5] = ["ser", "ésser", "estar", "semblar", "parèixer"];

/// `QueIniciFilter.ANIMATE_SUBJECT_NOUNS`.
const ANIMATE_SUBJECT_NOUNS: [&str; 69] = [
    "persona",
    "gent",
    "home",
    "dona",
    "noi",
    "noia",
    "nen",
    "nena",
    "nan",
    "senyor",
    "senyora",
    "senyoreta",
    "pare",
    "mare",
    "fill",
    "filla",
    "germà",
    "germana",
    "amic",
    "amiga",
    "professor",
    "professora",
    "mestre",
    "mestra",
    "metge",
    "metgessa",
    "doctor",
    "doctora",
    "alcalde",
    "rei",
    "reina",
    "policia",
    "nadó",
    "avi",
    "àvia",
    "tia",
    "tio",
    "oncle",
    "cosí",
    "cosina",
    "veí",
    "veïna",
    "client",
    "clienta",
    "jutge",
    "gos",
    "gossa",
    "gat",
    "gata",
    "nena",
    "criatura",
    "company",
    "companya",
    "merdós",
    "cabró",
    "imbècil",
    "idiota",
    "desgraciat",
    "malparit",
    "beneit",
    "tio",
    "individu",
    "paio",
    "tipus",
    "subjecte",
    "element",
    "marit",
    "muller",
    "xiquet",
];

/// `QueIniciFilter.NEVER_SUBJECT_NOUNS`.
const NEVER_SUBJECT_NOUNS: [&str; 24] = [
    "cop",
    "vegada",
    "volta",
    "mica",
    "miqueta",
    "moment",
    "instant",
    "dilluns",
    "dimarts",
    "dimecres",
    "dijous",
    "divendres",
    "dissabte",
    "diumenge",
    "dia",
    "nit",
    "matí",
    "tarda",
    "vespre",
    "hora",
    "temps",
    "setmana",
    "mes",
    "any",
];

/// `QueIniciFilter.TEMPORAL_ADJUNCT_NOUNS`.
const TEMPORAL_ADJUNCT_NOUNS: [&str; 23] = [
    "cop",
    "vegada",
    "volta",
    "moment",
    "instant",
    "dilluns",
    "dimarts",
    "dimecres",
    "dijous",
    "divendres",
    "dissabte",
    "diumenge",
    "dia",
    "nit",
    "nit_1",
    "matí",
    "tarda",
    "vespre",
    "hora",
    "temps",
    "setmana",
    "mes",
    "any",
];

/// `QueIniciFilter.WEEKDAY_NOUNS`.
const WEEKDAY_NOUNS: [&str; 7] = [
    "dilluns",
    "dimarts",
    "dimecres",
    "dijous",
    "divendres",
    "dissabte",
    "diumenge",
];

/// `QueIniciFilter.ACCUSATIVE_PRONOUNS`.
const ACCUSATIVE_PRONOUNS: [&str; 8] = ["em", "et", "el", "la", "ens", "us", "les", "els"];

/// `QueIniciFilter.DATIVE_PRONOUNS`.
const DATIVE_PRONOUNS: [&str; 6] = ["em", "et", "li", "ens", "us", "els"];

/// `QueIniciFilter.INTERROGATIVE_WORDS`.
const INTERROGATIVE_WORDS: [&str; 12] = [
    "que", "què", "quin", "quina", "quins", "quines", "qui", "quant", "quants", "quanta",
    "quantes", "res",
];

/// `QueIniciFilter.COMPLETIVE_GAP_VERBS`.
const COMPLETIVE_GAP_VERBS: [&str; 16] = [
    "creure",
    "pensar",
    "dir",
    "opinar",
    "afirmar",
    "suposar",
    "imaginar",
    "saber",
    "considerar",
    "sospitar",
    "témer",
    "voler",
    "desitjar",
    "necessitar",
    "esperar",
    "preferir",
];

/// `QueIniciFilter.NEUTER_ATTRIBUTE_WORDS`.
const NEUTER_ATTRIBUTE_WORDS: [&str; 4] = ["això", "allò", "açò", "ho"];

/// `QueIniciFilter.CONFIRMATION_TAGS`.
const CONFIRMATION_TAGS: [&str; 6] = ["veritat", "oi", "eh", "no", "cert", "potser"];

/// `QueIniciFilter.EXPLETIVES`.
#[allow(dead_code)]
const EXPLETIVES: [&str; 25] = [
    "collons",
    "coi",
    "cony",
    "dimonis",
    "dimoni",
    "carai",
    "diantre",
    "caram",
    "carall",
    "punyeta",
    "punyetes",
    "redimonis",
    "diables",
    "diable",
    "hòstia",
    "dimontri",
    "dimontris",
    "redéu",
    "redeu",
    "punyetera",
    "leche",
    "putes",
    "fotons",
    "llamps",
    "trons",
];

/// `AnalyzedTokenReadings.matchesPosTagRegex` (full match).
fn matches_re(token: &AnalyzedTokenReadings, re: &regex::Regex) -> bool {
    token.has_pos_tag_matching(re)
}

fn at<'a>(tokens: &[&'a AnalyzedTokenReadings], i: isize) -> Option<&'a AnalyzedTokenReadings> {
    if i < 0 {
        None
    } else {
        tokens.get(i as usize).copied()
    }
}

/// `QueIniciFilter.findQue`.
fn find_que(tokens: &[&AnalyzedTokenReadings]) -> isize {
    for (i, token) in tokens.iter().enumerate() {
        let form = token.surface();
        if form.eq_ignore_ascii_case("que") || form.eq_ignore_ascii_case("què") {
            return i as isize;
        }
    }
    -1
}

/// `QueIniciFilter.findQuestionMark`.
fn find_question_mark(tokens: &[&AnalyzedTokenReadings], from_idx: isize) -> isize {
    for i in (from_idx + 1)..(tokens.len() as isize) {
        if tokens[i as usize].surface() == "?" {
            return i;
        }
    }
    -1
}

/// `QueIniciFilter.stripConfirmationTag`.
fn strip_confirmation_tag(
    tokens: &[&AnalyzedTokenReadings],
    que_idx: isize,
    question_idx: isize,
) -> isize {
    let last_core = question_idx - 1;
    let comma_idx = last_core - 1;
    if comma_idx > que_idx
        && tokens[comma_idx as usize].surface() == ","
        && CONFIRMATION_TAGS.contains(&tokens[last_core as usize].surface().to_lowercase().as_str())
    {
        return comma_idx;
    }
    question_idx
}

/// `QueIniciFilter.findFirst(tokens, start, form)` (case-insensitive match).
fn find_first_form(tokens: &[&AnalyzedTokenReadings], start: isize, form: &str) -> isize {
    for i in start..(tokens.len() as isize) {
        if tokens[i as usize].surface().eq_ignore_ascii_case(form) {
            return i;
        }
    }
    -1
}

/// Java `findFirst(tokens, start, posTagPattern)` overload, unused there
/// (kept for the inventory).
#[allow(dead_code)]
fn find_first_pattern(tokens: &[&AnalyzedTokenReadings], start: isize, re: &regex::Regex) -> isize {
    for i in start..(tokens.len() as isize) {
        if matches_re(tokens[i as usize], re) {
            return i;
        }
    }
    -1
}

/// `QueIniciFilter.VerbGroupInfo`.
struct VerbGroupInfo {
    first_verb_index: isize,
    last_verb_index: isize,
    last_index: isize,
    num_pronouns_before: isize,
}

/// `QueIniciFilter.verbGroupAfterQue` (the `Language` argument of the Java
/// signature is the filter's variant synthesizer in Rust).
fn verb_group_after_que(
    tokens: &[&AnalyzedTokenReadings],
    que_idx: isize,
    core_end: isize,
    env: &Env,
) -> Option<VerbGroupInfo> {
    let verb_synthesizer = VerbSynthesizer::new(
        tokens,
        (que_idx + 1).max(0) as usize,
        env.synth.inner(),
        false,
    );
    if verb_synthesizer.is_undefined() || verb_synthesizer.get_first_verb_index() >= core_end {
        return None;
    }
    let first_verb = verb_synthesizer.get_first_verb_index();
    let next_que = find_first_form(tokens, que_idx + 1, "que");
    if next_que > que_idx && next_que < first_verb {
        return None;
    }
    let boundary = if next_que > first_verb && next_que < core_end {
        next_que
    } else {
        core_end
    };
    let last_index = verb_synthesizer.get_last_index().min(boundary - 1);
    let mut last_verb = verb_synthesizer.get_last_verb_index().min(last_index);
    while last_verb >= first_verb
        && reading_with_tag_regex(tokens[last_verb as usize], "V.*").is_none()
    {
        last_verb -= 1;
    }
    if last_verb < first_verb {
        return None;
    }
    Some(VerbGroupInfo {
        first_verb_index: first_verb,
        last_verb_index: last_verb,
        last_index,
        num_pronouns_before: verb_synthesizer.get_num_pronouns_before(),
    })
}

/// `QueIniciFilter.verbIndexAfterQue` (Java-test helper, unused here).
#[allow(dead_code)]
pub fn verb_index_after_que(
    tokens: &[&AnalyzedTokenReadings],
    que_idx: isize,
    core_end: isize,
    env: &Env,
) -> isize {
    match verb_group_after_que(tokens, que_idx, core_end, env) {
        Some(verb_group) => verb_group.last_verb_index,
        None => -1,
    }
}

/// `QueIniciFilter.hasVerbStructure` (used by the Java corpus test).
#[allow(dead_code)]
pub fn has_verb_structure(tokens: &[&AnalyzedTokenReadings], env: &Env) -> bool {
    let que_idx = find_que(tokens);
    if que_idx < 0 {
        return false;
    }
    let question_idx = find_question_mark(tokens, que_idx);
    if question_idx < 0 {
        return false;
    }
    let core_end = strip_confirmation_tag(tokens, que_idx, question_idx);
    verb_index_after_que(tokens, que_idx, core_end, env) >= 0
}

/// `QueIniciFilter.addPronounToken`.
fn add_pronoun_token(tokens: &[&AnalyzedTokenReadings], index: isize, pronoms: &mut Vec<String>) {
    if index < 0 || index as usize >= tokens.len() {
        return;
    }
    if let Some(pronoun) = reading_with_tag_regex(tokens[index as usize], PRONOM_RE_STR) {
        pronoms.push(transform(&pronoun.token, PronounPosition::Normalized));
    }
}

/// The `PRONOM` pattern as a string (`addPronounToken`/`readingWithTagRegex`
/// path).
const PRONOM_RE_STR: &str = "P0.{6}|PP3CN000|PP3NN000|PP3..A00|PP[123]CP000|PP3CSD00";

/// `QueIniciFilter.firstArgumentTokenAfter`.
fn first_argument_token_after(
    tokens: &[&AnalyzedTokenReadings],
    start: isize,
    end: isize,
) -> isize {
    let mut i = start;
    while i < end && i < tokens.len() as isize {
        let form = tokens[i as usize].surface();
        if form == "?" || form == "," {
            return -1;
        }
        if matches_re(tokens[i as usize], &ADVERB_RE) {
            i += 1;
            continue;
        }
        return i;
    }
    -1
}

/// `QueIniciFilter.postverbalCanBeSubject`.
fn postverbal_can_be_subject(
    tokens: &[&AnalyzedTokenReadings],
    start: isize,
    verb3s: bool,
    verb3p: bool,
    verb2p: bool,
) -> bool {
    let verb_p = verb3p || verb2p;
    let mut i = start;
    while i < tokens.len() as isize && i <= start + 4 {
        let form = tokens[i as usize].surface();
        if form == "?" || form == "," {
            break;
        }
        if reading_with_tag_regex(tokens[i as usize], "NP.*").is_some() {
            // nom propi: normalment animat/subjecte (el nombre sovint no hi és marcat)
            return true;
        }
        if let Some(common_noun) = reading_with_tag_regex(tokens[i as usize], "NC.*") {
            if NEVER_SUBJECT_NOUNS.contains(&common_noun.lemma()) {
                return false;
            }
            if !(ANIMATE_SUBJECT_NOUNS.contains(&common_noun.lemma())
                || tokens[i as usize].has_pos_tag("NCCN000"))
            {
                // numeral
                return false;
            }
            let noun_singular = matches_re(tokens[i as usize], &NOUN_SINGULAR_RE);
            let noun_plural = matches_re(tokens[i as usize], &NOUN_PLURAL_RE);
            return (verb3s && noun_singular)
                || (verb_p && noun_plural)
                || (!noun_singular && !noun_plural);
        }
        i += 1;
    }
    false
}

/// `QueIniciFilter.isFerMal`.
fn is_fer_mal(tokens: &[&AnalyzedTokenReadings], start: isize) -> bool {
    if start <= 0 || start >= tokens.len() as isize {
        return false;
    }
    let first = tokens[start as usize].surface().to_lowercase();
    if first == "cap" || first == "algun" || first == "alguna" || first == "un" || first == "una" {
        return false;
    }
    tokens[start as usize].has_lemma("mal")
        || (tokens[start as usize].has_lemma("por") && !next_token_is(tokens, start, "que"))
}

/// `QueIniciFilter.isFerPorMalPolar`.
fn is_fer_por_mal_polar(
    tokens: &[&AnalyzedTokenReadings],
    main_verb_pos: isize,
    start: isize,
    end: isize,
    has_dative_pronoun: bool,
) -> bool {
    if main_verb_pos <= 0
        || start <= 0
        || start >= tokens.len() as isize
        || end <= start
        || !tokens[main_verb_pos as usize].has_lemma("fer")
    {
        return false;
    }
    let noun_pos = first_lexical_token(tokens, start, end);
    if noun_pos < 0
        || !(tokens[noun_pos as usize].has_lemma("por")
            || tokens[noun_pos as usize].has_lemma("mal"))
    {
        return false;
    }
    if tokens[noun_pos as usize].has_lemma("por") && next_token_is(tokens, noun_pos, "que") {
        return false;
    }
    has_dative_pronoun
        || has_infinitive_after_noun(tokens, noun_pos, end)
        || has_subject_after_fer_mal(tokens, noun_pos, end)
        || has_auxiliary_before_fer(tokens, main_verb_pos)
}

/// `QueIniciFilter.firstLexicalToken`.
fn first_lexical_token(tokens: &[&AnalyzedTokenReadings], start: isize, end: isize) -> isize {
    let mut i = start;
    while i < end && i < tokens.len() as isize {
        let token = tokens[i as usize].surface();
        if token == "," || token == "?" {
            break;
        }
        if token.eq_ignore_ascii_case("pas")
            || token.eq_ignore_ascii_case("gaire")
            || token.eq_ignore_ascii_case("més")
        {
            i += 1;
            continue;
        }
        return i;
    }
    -1
}

/// `QueIniciFilter.hasSubjectAfterFerMal`.
fn has_subject_after_fer_mal(
    tokens: &[&AnalyzedTokenReadings],
    noun_pos: isize,
    end: isize,
) -> bool {
    let mut i = noun_pos + 1;
    while i < end && i < tokens.len() as isize && i <= noun_pos + 4 {
        if tokens[i as usize].surface() == "," || tokens[i as usize].surface() == "?" {
            break;
        }
        if matches_re(tokens[i as usize], &SUBJECT_AFTER_FER_MAL_RE) {
            return true;
        }
        i += 1;
    }
    false
}

/// `QueIniciFilter.hasInfinitiveAfterNoun`.
fn has_infinitive_after_noun(
    tokens: &[&AnalyzedTokenReadings],
    noun_pos: isize,
    end: isize,
) -> bool {
    let mut i = noun_pos + 1;
    while i < end && i < tokens.len() as isize && i <= noun_pos + 3 {
        if tokens[i as usize].surface() == "," || tokens[i as usize].surface() == "?" {
            break;
        }
        if matches_re(tokens[i as usize], &INFINITIVE_RE) {
            return true;
        }
        i += 1;
    }
    false
}

/// `QueIniciFilter.hasAuxiliaryBeforeFer`.
fn has_auxiliary_before_fer(tokens: &[&AnalyzedTokenReadings], verb_pos: isize) -> bool {
    let mut i = verb_pos - 1;
    while i > 0 && i >= verb_pos - 4 {
        if tokens[i as usize].surface() == "," || tokens[i as usize].surface() == "?" {
            break;
        }
        if let Some(verb) = reading_with_tag_regex(tokens[i as usize], "V.*") {
            let lemma = verb.lemma();
            if lemma == "voler" || lemma == "poder" || lemma == "deure" || lemma == "haver" {
                return true;
            }
        }
        i -= 1;
    }
    false
}

/// `QueIniciFilter.isFerPorQue`.
fn is_fer_por_que(tokens: &[&AnalyzedTokenReadings], start: isize) -> bool {
    start > 0
        && start < tokens.len() as isize
        && tokens[start as usize].has_lemma("por")
        && next_token_is(tokens, start, "que")
}

/// `QueIniciFilter.nextTokenIs`.
fn next_token_is(tokens: &[&AnalyzedTokenReadings], start: isize, token: &str) -> bool {
    start + 1 < tokens.len() as isize
        && tokens[(start + 1) as usize]
            .surface()
            .eq_ignore_ascii_case(token)
}

/// `QueIniciFilter.postverbalStartsTemporalAdjunct`.
fn postverbal_starts_temporal_adjunct(
    tokens: &[&AnalyzedTokenReadings],
    start: isize,
    end: isize,
) -> bool {
    if start <= 0 || start >= tokens.len() as isize || end <= start {
        return false;
    }
    let mut has_determiner = false;
    let mut i = start;
    while i < end && i <= start + 3 {
        if matches_re(tokens[i as usize], &DET_RE) {
            has_determiner = true;
            i += 1;
            continue;
        }
        let Some(noun) = reading_with_tag_regex(tokens[i as usize], "NC.*") else {
            return false;
        };
        return TEMPORAL_ADJUNCT_NOUNS.contains(&noun.lemma())
            && (has_determiner || WEEKDAY_NOUNS.contains(&noun.lemma()));
    }
    false
}

/// `QueIniciFilter.isAnimateSubjectNoun` (used by the Java corpus test).
#[allow(dead_code)]
pub fn is_animate_subject_noun(lemma: &str) -> bool {
    ANIMATE_SUBJECT_NOUNS.contains(&lemma)
}

/// `QueIniciFilter.isNeverSubjectNoun` (used by the Java corpus test).
#[allow(dead_code)]
pub fn is_never_subject_noun(lemma: &str) -> bool {
    NEVER_SUBJECT_NOUNS.contains(&lemma)
}

/// `QueIniciFilter.isSkippableBeforeVerb` (present in Java, unused there).
#[allow(dead_code)]
fn is_skippable_before_verb(token: &AnalyzedTokenReadings) -> bool {
    let form = token.surface().to_lowercase();
    if form == "no" || EXPLETIVES.contains(&form.as_str()) {
        return true;
    }
    if is_weak_pronoun(token) {
        return true;
    }
    matches_re(token, &ADVERB_OR_I_RE)
}

/// `QueIniciFilter.isWeakPronoun` (present in Java, unused there).
#[allow(dead_code)]
fn is_weak_pronoun(token: &AnalyzedTokenReadings) -> bool {
    matches_re(token, &PRONOM_RE)
}

/// `QueIniciFilter.isVerb` (present in Java, unused there).
#[allow(dead_code)]
fn is_verb(token: &AnalyzedTokenReadings) -> bool {
    matches_re(token, &ANY_VERB_RE)
}

/// `QueIniciFilter.verbLemma` (present in Java, unused there).
#[allow(dead_code)]
pub fn verb_lemma(token: &AnalyzedTokenReadings) -> Option<&str> {
    token
        .readings
        .iter()
        .find(|reading| {
            reading
                .pos_tag
                .as_deref()
                .is_some_and(|pos_tag| pos_tag.starts_with('V'))
        })
        .map(|reading| reading.lemma())
}

/// `QueIniciFilter.completiveHasGap`.
fn completive_has_gap(
    tokens: &[&AnalyzedTokenReadings],
    main_verb_pos: isize,
    core_end: isize,
    env: &Env,
) -> Option<bool> {
    let que_pos = main_verb_pos + 1;
    let emb = verb_group_after_que(tokens, que_pos, core_end, env)?;
    let emb_verb_reading = reading_with_tag_regex(tokens[emb.last_verb_index as usize], "V.*")?;
    emb_verb_reading.stem.as_ref()?;
    let emb_lemma = emb_verb_reading.lemma().to_string();

    // Clítics de la subordinada (davant del primer verb i enclítics darrere l'últim).
    let mut emb_pron: Vec<String> = Vec::new();
    let mut k = emb.first_verb_index - emb.num_pronouns_before;
    while k < emb.first_verb_index {
        add_pronoun_token(tokens, k, &mut emb_pron);
        k += 1;
    }
    let mut k = emb.last_verb_index + 1;
    while k <= emb.last_index {
        add_pronoun_token(tokens, k, &mut emb_pron);
        k += 1;
    }
    for p in &emb_pron {
        // Un clític acusatiu o neutre/partitiu ("ho", "en") ja ocupa el CD -> saturada.
        if (ACCUSATIVE_PRONOUNS.contains(&p.as_str()) && !DATIVE_PRONOUNS.contains(&p.as_str()))
            || p == "ho"
            || p == "en"
        {
            return Some(false);
        }
    }

    // Subjecte overt preverbal (pronom fort o nom propi entre "que" i el verb).
    let mut overt_subject = false;
    let mut k = que_pos + 1;
    while k < emb.first_verb_index && k < tokens.len() as isize {
        if matches_re(tokens[k as usize], &STRONG_SUBJECT_RE) {
            overt_subject = true;
        }
        k += 1;
    }

    let copular = COPULAR_VERBS.contains(&emb_lemma.as_str());
    let intransitive = env.verb_classifier.is_intransitive(&emb_lemma) && !copular;
    let content_pos = first_argument_token_after(tokens, emb.last_index + 1, core_end);

    if copular {
        if content_pos < 0 {
            // atribut buit: "que ha estat?", "que és?"
            return Some(true);
        }
        let c_form = tokens[content_pos as usize].surface().to_lowercase();
        if NEUTER_ATTRIBUTE_WORDS.contains(&c_form.as_str())
            || INTERROGATIVE_WORDS.contains(&c_form.as_str())
        {
            // "que és això?", "que és qui?"
            return Some(true);
        }
        // Atribut ple (adjectiu, participi, SN o complement preposicional) -> saturada.
        if matches_re(tokens[content_pos as usize], &COPULAR_ATTRIBUTE_RE) {
            return Some(false);
        }
        return Some(true);
    }

    if content_pos < 0 {
        // Res darrere el verb. Amb subjecte overt (i sense CD) la subordinada és completa;
        // si no, el "què" pot ser el subjecte ("que ha passat?") o el CD ("que diria?").
        return Some(!(overt_subject && intransitive));
    }

    let c_form = tokens[content_pos as usize].surface().to_lowercase();
    if matches_re(tokens[content_pos as usize], &COMPLETIVE_CONTENT_RE)
        && !INTERROGATIVE_WORDS.contains(&c_form.as_str())
    {
        let v3s = matches_re(tokens[emb.first_verb_index as usize], &VERB_3S_RE);
        let v3p = matches_re(tokens[emb.first_verb_index as usize], &VERB_3P_RE);
        let v2p = matches_re(tokens[emb.first_verb_index as usize], &VERB_2P_RE);
        // SN posposat animat/propi -> és el subjecte, el CD queda buit ("que va dir en Joan?").
        // SN inanimat -> és el CD i la subordinada queda saturada ("que té raó?").
        return Some(postverbal_can_be_subject(
            tokens,
            content_pos,
            v3s,
            v3p,
            v2p,
        ));
    }

    // Darrere el verb només hi ha un complement no nominal (preposició, adverbi, infinitiu...).
    // Amb verb transitiu el CD continua buit ("que va dir a algú?"); amb intransitiu, no.
    Some(!intransitive)
}

/// `QueIniciFilter.predictsAccent`.
fn predicts_accent(
    tokens: &[&AnalyzedTokenReadings],
    env: &Env,
    completive_only: bool,
) -> Option<bool> {
    let len = tokens.len() as isize;
    let que_pos = find_que(tokens);
    if que_pos < 0 {
        return None;
    }
    let starts_with_accent = tokens[que_pos as usize]
        .surface()
        .eq_ignore_ascii_case("què");
    let question_pos = find_question_mark(tokens, que_pos);
    let core_end = if question_pos >= 0 {
        strip_confirmation_tag(tokens, que_pos, question_pos)
    } else {
        len
    };
    let last_word = if len > 2 {
        tokens[(len - 2) as usize].surface().to_string()
    } else {
        String::new()
    };
    let is_si_no_question = len > 2
        && (SINO_LAST_WORDS.contains(&last_word.to_lowercase().as_str())
            || (last_word == "què" && tokens[(len - 1) as usize].surface() == "o")
            || (last_word == "no" && tokens[(len - 1) as usize].surface() == ","));
    let is_another_subject = last_word == "això";

    // Subjecte dislocat al final (", el professor?"): si el subjecte ja hi és, el SN
    // postverbal és el CD -> no s'ha de comptar com a possible subjecte.
    let mut j = len - 2;
    while j > que_pos && matches_re(tokens[j as usize], &TRAILING_DISLOCATION_RE) {
        j -= 1;
    }
    let trailing_dislocation = j < len - 2 && j > que_pos && tokens[j as usize].surface() == ",";

    let verb_group = verb_group_after_que(tokens, que_pos, core_end, env)?;
    let first_verb_pos = verb_group.first_verb_index;
    let main_verb_pos = verb_group.last_verb_index;
    let verb3s = matches_re(tokens[first_verb_pos as usize], &VERB_3S_RE);
    let verb3p = matches_re(tokens[first_verb_pos as usize], &VERB_3P_RE);
    let verb2p = matches_re(tokens[first_verb_pos as usize], &VERB_2P_RE);
    let main_verb_reading = reading_with_tag_regex(tokens[main_verb_pos as usize], "V.*")?;
    let main_verb_lemma = main_verb_reading.lemma().to_string();

    let first_token_after_verb_pos = verb_group.last_index + 1;
    let first_postag_after_verb = if first_token_after_verb_pos < len {
        at(tokens, first_token_after_verb_pos)
            .and_then(|t| reading_with_tag_regex(t, ".*"))
            .and_then(|r| r.pos_tag.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string())
    } else {
        String::new()
    };

    if completive_only
        && !(main_verb_pos >= 0
            && main_verb_pos + 1 < len
            && tokens[(main_verb_pos + 1) as usize].surface() == "que"
            && COMPLETIVE_GAP_VERBS.contains(&main_verb_lemma.as_str()))
    {
        return None;
    }

    let mut pronoms: Vec<String> = Vec::new();
    let mut i = first_verb_pos - verb_group.num_pronouns_before;
    while i < first_verb_pos {
        add_pronoun_token(tokens, i, &mut pronoms);
        i += 1;
    }
    let mut i = main_verb_pos + 1;
    while i <= verb_group.last_index {
        add_pronoun_token(tokens, i, &mut pronoms);
        i += 1;
    }
    let has_ho = pronoms.iter().any(|p| p == "ho");
    let mut has_accusative_pronoun = false;
    let mut has_dative_pronoun = false;
    let mut has_accusative_not_dative_pronoun = false;
    for p in &pronoms {
        let accusative = ACCUSATIVE_PRONOUNS.contains(&p.as_str());
        let dative = DATIVE_PRONOUNS.contains(&p.as_str());
        has_accusative_pronoun |= accusative;
        has_dative_pronoun |= dative;
        has_accusative_not_dative_pronoun |= accusative && !dative;
    }
    let pronom_str = if pronoms.is_empty() {
        String::new()
    } else {
        transform(&pronoms.join(" "), PronounPosition::Davant)
    };

    // El complement posposat pot ser subjecte? Esbiaixem cap a "objecte": només si el nucli
    // del SN és un nom propi o un nom comú animat (i concorda en nombre amb el verb). Els SN
    // de nom comú inanimat es tracten com a CD (-> "que").
    let mut complement_can_be_subject = (verb3s || verb3p || verb2p)
        && first_token_after_verb_pos > 0
        && first_token_after_verb_pos < len
        && postverbal_can_be_subject(tokens, first_token_after_verb_pos, verb3s, verb3p, verb2p);

    // El complement posposat pot ser complement directe?
    let mut complement_can_be_object = has_accusative_not_dative_pronoun;
    if first_postag_after_verb.starts_with('N')
        || (first_token_after_verb_pos > 0
            && first_token_after_verb_pos + 1 < len
            && (matches_re(tokens[first_token_after_verb_pos as usize], &DET_RE)
                || INTERROGATIVE_WORDS.contains(
                    &tokens[first_token_after_verb_pos as usize]
                        .surface()
                        .to_lowercase()
                        .as_str(),
                )))
    {
        complement_can_be_object = true;
    }

    // veure't (i potser altres verbs)
    if main_verb_lemma == "veure" && has_accusative_pronoun {
        complement_can_be_object = true;
    }

    // Excepcions: complements de temps o oracions de relatiu, "tot"...
    let mut is_exception_object = false;
    let mut is_exception_subject = false;
    if first_token_after_verb_pos > 0 && first_token_after_verb_pos + 1 < len {
        let readings_after_verb = tokens[first_token_after_verb_pos as usize];
        let readings_after_verb2 = tokens[(first_token_after_verb_pos + 1) as usize];
        let is_common_exception = readings_after_verb2
            .chunk_tags
            .iter()
            .any(|c| c == PTIME_CHUNK)
            || readings_after_verb2.has_pos_tag("_loc_unavegada")
            || readings_after_verb2.has_pos_tag("_data_concreta")
            || readings_after_verb.has_lemma("tot");
        is_exception_object = !has_accusative_not_dative_pronoun && is_common_exception;
        is_exception_subject = is_common_exception;
    }
    complement_can_be_object = complement_can_be_object && !is_exception_object;
    if main_verb_lemma == "fer"
        && postverbal_starts_temporal_adjunct(tokens, first_token_after_verb_pos, core_end)
    {
        complement_can_be_object = false;
    }
    complement_can_be_subject = complement_can_be_subject
        && !is_exception_subject
        && !is_another_subject
        && !trailing_dislocation;

    // Transitivitat i rol de "què"
    let mut is_que_subject = false;
    if verb3s && main_verb_pos > 1 {
        let que_found_pos = find_first_form(tokens, main_verb_pos + 1, "que");
        let nearby_que = main_verb_pos < que_found_pos && que_found_pos < main_verb_pos + 5;
        let fer_has_subject_que = nearby_que || has_accusative_not_dative_pronoun;
        let previous_verb = reading_with_tag_regex(tokens[(main_verb_pos - 1) as usize], "V.*");
        is_que_subject = fer_has_subject_que
            && (main_verb_lemma == "fer"
                || previous_verb.is_some_and(|verb| verb.lemma() == "fer"));
    }
    let mut is_que_object = false;
    // Subordinada saturada ("que" de sí/no): força la predicció a no-accent.
    let mut saturated_completive = false;
    let main_verb_finite =
        reading_with_tag_regex(tokens[main_verb_pos as usize], FINITE_VERB_RE_STR);
    if let Some(main_verb_finite) = main_verb_finite {
        if main_verb_pos + 1 < len
            && tokens[(main_verb_pos + 1) as usize].surface() == "que"
            && main_verb_finite.lemma() != "veure"
        {
            if COMPLETIVE_GAP_VERBS.contains(&main_verb_lemma.as_str()) {
                // "[verb epistèmic/volitiu] que [subordinada]": decidim segons si hi ha buit.
                match completive_has_gap(tokens, main_verb_pos, core_end, env) {
                    Some(false) => saturated_completive = true,
                    // buit detectat o no decidible: "què" lliga un argument de la subordinada.
                    _ => is_que_object = true,
                }
            } else {
                is_que_object = true;
            }
        }
    }
    let is_fer = main_verb_lemma == "fer";
    let fer_por_mal_polar = !starts_with_accent
        && is_fer_por_mal_polar(
            tokens,
            main_verb_pos,
            first_token_after_verb_pos,
            core_end,
            has_dative_pronoun,
        );
    if fer_por_mal_polar || (is_fer && is_fer_por_que(tokens, first_token_after_verb_pos)) {
        is_que_subject = false;
        complement_can_be_object = true;
    } else if is_fer && is_fer_mal(tokens, first_token_after_verb_pos) {
        is_que_subject = true;
        complement_can_be_object = false;
    }

    let mut is_intransitive = (env.verb_classifier.is_intransitive(&main_verb_lemma)
        && !COPULAR_VERBS.contains(&main_verb_lemma.as_str()))
        || (main_verb_lemma == "ser" && pronom_str == "hi");
    if main_verb_lemma == "passar" || main_verb_lemma == "agradar" {
        is_intransitive = true;
        is_que_subject = !complement_can_be_object;
    } else if is_intransitive {
        is_que_subject = false;
    }
    is_que_subject = is_que_subject && !is_another_subject;

    if starts_with_accent
        && main_verb_lemma == "fer"
        && is_fer_mal(tokens, first_token_after_verb_pos)
    {
        // duplicate code, except for isTransitive
        is_intransitive = false;
        is_que_subject = true;
        complement_can_be_object = false;
    }

    let mut predicts_accent = is_que_subject
        || is_que_object
        || (!is_intransitive
            && ((!complement_can_be_object && !has_ho) || complement_can_be_subject));

    // Subordinada saturada d'un verb epistèmic/volitiu ("Que creus que té raó?"): sí/no,
    // tret que "què" quedi clarament com a subjecte de l'oració principal.
    if saturated_completive && !is_que_subject {
        predicts_accent = false;
    }
    // Una cua "..., potser?"/"..., oi?" indica sí/no, tret que "què" sigui clarament
    // subjecte o CD (interrogativa retòrica: "Què passa, ..., potser?").
    if is_si_no_question && !is_que_subject && !is_que_object {
        predicts_accent = false;
    }
    Some(predicts_accent)
}

/// The `FINITE_VERB` pattern as a string (`readingWithTagRegex` path).
const FINITE_VERB_RE_STR: &str = "V.[SI].*";

/// `org.languagetool.rules.ca.QueIniciFilter`.
pub struct QueIniciFilter {
    pub(crate) env: Env,
}

impl RuleFilter for QueIniciFilter {
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let id = ctx.rule_id;
        let is_amb = id.contains("AMBACCENT") || id.contains("AMB_ACCENT");
        let is_sense = id.contains("SENSEACCENT") || id.contains("SENSE_ACCENT");
        if !is_amb && !is_sense {
            return FilterOutcome::accept();
        }
        let completive_only = ctx.args.get("mode").is_some_and(|m| m == "completive");
        let tokens = tokens_without_whitespace(ctx);
        let predicts_accent = predicts_accent(&tokens, &self.env, completive_only);
        if completive_only && predicts_accent.is_none() {
            return FilterOutcome::accept();
        }
        let Some(predicts_accent) = predicts_accent else {
            return FilterOutcome::accept();
        };
        if is_amb {
            // Text "que", suggerim "què": confirmem només si la predicció és "què".
            return if predicts_accent {
                FilterOutcome::accept()
            } else {
                FilterOutcome::reject()
            };
        }
        // SENSE: text "què", suggerim "que": confirmem només si la predicció és "que".
        if predicts_accent {
            FilterOutcome::reject()
        } else {
            FilterOutcome::accept()
        }
    }
}
