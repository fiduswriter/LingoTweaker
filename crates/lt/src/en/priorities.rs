//! English rule/category priorities (`English.getPriorityForId` and
//! `Language.getRulePriority`), used by the `CleanOverlappingFilter` port.

use regex::Regex;
use std::sync::LazyLock;

// Generated from English.java id2prio (do not edit by hand).
pub(crate) static EN_PRIORITIES: &[(&str, i32)] = &[
    ("A3FT", 1),
    ("ABBREVIATION_PUNCTUATION", 2),
    ("ACCESS_EXCESS", 1),
    ("ADVERB_OR_HYPHENATED_ADJECTIVE", -1),
    ("ADVERB_WORD_ORDER", -1),
    ("ADVERB_WORD_ORDER_10_TEMP", 1),
    ("AGREEMENT_SENT_START", -1),
    ("ALL_NN", 1),
    ("ALL_OF_SUDDEN", 1),
    ("ALL_UPPERCASE", -1000),
    ("ANYWAYS", -1),
    ("AN_AND", 1),
    ("APOSTROPHE_IN_DAYS", 1),
    ("APOSTROPHE_VS_QUOTE", 1),
    ("APPSTORE", 1),
    ("ATD_VERBS_TO_COLLOCATION", -1),
    ("A_BUT", 1),
    ("A_HEADS_UP", 1),
    ("A_HUNDREDS", 1),
    ("A_INFINITIVE", -3),
    ("A_LOT_OF_NN", -1),
    ("A_NNS_BEST_NN", 1),
    ("A_NUMBER_NNS", 1),
    ("A_OK", 1),
    ("A_RB_NN", -1),
    ("A_SCISSOR", 1),
    ("A_SNICKERS", 1),
    ("A_TO", -1),
    ("A_WINDOWS", 1),
    ("BACHELORS", 1),
    ("BEEN_PART_AGREEMENT", -13),
    ("BESTEST", 1),
    ("BE_I_BE_GERUND", -1),
    ("BE_MD", -20),
    ("BE_NN", -26),
    ("BE_NOT_BE_JJ", 1),
    ("BE_RB_BE", -1),
    ("BE_TO_VBG", -1),
    ("BE_VBG_BE", -1),
    ("BE_VBG_NN", -12),
    ("BE_VBP_IN", -12),
    ("BE_VB_OR_NN", -26),
    ("BE_WITH_WRONG_VERB_FORM", -14),
    ("BLACK_SEA", -1),
    ("BORN_IN", 1),
    ("BOUT_TO", 1),
    ("CANT_JJ", -2),
    ("CAN_MISSPELLING", 1),
    ("CAUSE_BECAUSE", 1),
    ("CAUSE_COURSE", 1),
    ("CC_PRP_ARTICLE", -15),
    ("CD_NN", -1),
    ("CD_NNU", -1),
    ("CHARACTER_APOSTROPHE_WORD", -1),
    ("CHILDISH_LANGUAGE", 8),
    ("COLLECTIVE_NOUN_VERB_AGREEMENT_VBP", -12),
    ("COMMA_CLOSING_PARENTHESIS", 1),
    ("COMMA_COMPOUND_SENTENCE", -1),
    ("COMMA_COMPOUND_SENTENCE_2", -1),
    ("COMMA_PERIOD", 1),
    ("CONFUSION_GONG_GOING", 1),
    ("CONFUSION_OF_A_JJ_NNP_NNS_PRP", -1),
    ("COULD_YOU_NOT_NEEDED", -49),
    ("COUPLE_OF_TIMES", 1),
    ("COVID_19", 1),
    ("CURIOS_CURIOUS", 1),
    ("DAT", 1),
    ("DID_FOUND_AMBIGUOUS", -1),
    ("DONTCHA", -4),
    ("DONT_T", 1),
    ("DON_T_AREN_T", 1),
    ("DOS_AND_DONTS", 3),
    ("DO_DT_NN_BE", -26),
    ("DO_HE_VERB", 1),
    ("DO_PRP_NOTVB", -3),
    ("DO_TO", 1),
    ("DRIVE_THROUGH_HYPHEN", 1),
    ("DROP_DEAD_HYPHEN", 1),
    ("DT_JJ_NO_NOUN", -1),
    ("DT_NN_ARE_AME", -12),
    ("DT_NN_VBG", -1),
    ("DT_PDT", -1),
    ("DT_RB_IN", -1),
    ("DUPLICATION_OF_IS_VBZ", -1),
    ("EG_NO_COMMA", -600),
    ("ELLIPSIS", 1),
    ("ENGLISH_WORD_REPEAT_RULE", -1),
    ("EN_A_VS_AN", -1),
    ("EN_DIACRITICS_REPLACE_ORTHOGRAPHY", -1),
    ("EN_PLAIN_ENGLISH_REPLACE", -511),
    ("EN_REDUNDANCY_REPLACE", -510),
    ("EN_UNPAIRED_BRACKETS", -1),
    ("ETC_PERIOD", -49),
    ("EVEN_HANDED_HYPHEN", 1),
    ("EVERY_NOW_AND_THEN", 0),
    ("FACE_TO_FACE_HYPHEN", 1),
    ("FASTLY", -1),
    ("FEDEX", 2),
    ("FINE_TUNE_COMPOUNDS", 1),
    ("FOLLOW_UP", 1),
    ("FOR_ANY_CLARIFICATIONS", -1),
    ("FOR_AWHILE", 1),
    ("FOR_NOUN_SAKE", 6),
    ("FOR_THE_MOST_PART2", 1),
    ("FOUR_NN", -599),
    ("GAVE_HAVE", 1),
    ("GET_TOGETHER_HYPHEN", 1),
    ("GIMME", -4),
    ("GOING_TO_VBD", -1),
    ("GONNA", -4),
    ("GONNA_TEMP", -3),
    ("GOOD_FLUCK", 2),
    ("GOTCHA", -4),
    ("GOT_HERE", 1),
    ("HANDS_ON_HYPHEN", 1),
    ("HAS_TO_APPROVED_BY", 1),
    ("HAVE_HAVE", 1),
    ("HAVE_PART_AGREEMENT", -13),
    ("HAVE_TO_NOTVB", -15),
    ("HAVE_VB_DT", -1),
    ("HEAR_HERE", 1),
    ("HERE_HEAR", 1),
    ("HER_S", 1),
    ("HE_VERB_AGR", -12),
    ("HYPHEN_TO_EN", 1),
    ("ID_CASING", -4),
    ("IE_NO_COMMA", -600),
    ("IF_DT_NN_VBZ", -12),
    ("IF_THEN_COMMA", -1),
    ("IF_VB_PCT", 1),
    ("IF_YOU_FURTHER_QUESTIONS", 3),
    ("INCORRECT_CONTRACTIONS", 1),
    ("INCORRECT_POSSESSIVE_APOSTROPHE", 1),
    ("INDIAN_ENGLISH", -3),
    ("IN_DT_IN", -15),
    ("IN_THIS_REGARDS", 1),
    ("IRREGARDLESS", 1),
    ("IS_LIKELY_TO_BE", -1),
    ("IT_IF", 1),
    ("IT_IS_2", -1),
    ("IT_IS_DEPENDING_ON", 1),
    ("IT_ITS", -1),
    ("IT_SEAMS", 1),
    ("IT_SOMETHING", 1),
    ("IT_VBZ", -12),
    ("I_A", 1),
    ("I_AM_VB", -2),
    ("I_A_M", 1),
    ("I_E", 10),
    ("I_IF", -1),
    ("I_THINK_FEEL", -60),
    ("KNOW_AWARE_REDO", -60),
    ("LEMME", -4),
    ("LIFE_COMPOUNDS", 1),
    ("LIGATURES", 1),
    ("LINKED_IN", 2),
    ("LOOK_FORWARD_TO", 1),
    ("LOOK_SLIKE", 1),
    ("LUV", 1),
    ("MAC_OS", 1),
    ("MAKE_OR_BREAK_HYPHEN", 2),
    ("MANY_NN", -1),
    ("MAY_MANY", 1),
    ("MD_APOSTROPHE_VB", 1),
    ("MD_BASEFORM", -12),
    ("MD_DT_JJ", -1),
    ("MD_JJ", -12),
    ("MD_NN", -60),
    ("MD_PRP", -1),
    ("MD_PRP_QUESTION_MARK", -11),
    ("MD_VBD", -1),
    ("MD_VB_AND_NOTVB", -1),
    ("METRIC_UNITS_EN_IMPERIAL", -1),
    ("MISSING_COMMA_BETWEEN_DAY_AND_YEAR", -1),
    ("MISSING_GENITIVE", -1),
    ("MISSING_HYPHEN", 5),
    ("MISSING_POSS_APOS", 1),
    ("MISSING_PREPOSITION", -1),
    ("MISSING_SUBJECT", -15),
    ("MISSING_TO_BETWEEN_BE_AND_VB", -15),
    ("MONEY_BACK_HYPHEN", 1),
    ("MORFOLOGIK_RULE_EN_AU", -10),
    ("MORFOLOGIK_RULE_EN_CA", -10),
    ("MORFOLOGIK_RULE_EN_GB", -10),
    ("MORFOLOGIK_RULE_EN_NZ", -10),
    ("MORFOLOGIK_RULE_EN_US", -10),
    ("MORFOLOGIK_RULE_EN_ZA", -10),
    ("NEEDS_FIXED", -1),
    ("NEITHER_NOR", 1),
    ("NNP_COMMA_QUESTION", -2),
    ("NNS_THAT_ARE_JJ", -1),
    ("NON3PRS_VERB", -1),
    ("NON_ENGLISH_CHARACTER_IN_A_WORD", 1),
    ("NON_STANDARD_COMMA", 1),
    ("NOUNPHRASE_VB_RB_DT", -1),
    ("NOUN_VERB_CONFUSION", -1),
    ("NOW_A_DAYS", 1),
    ("NO_KNOW", 1),
    ("NO_WHERE", 1),
    ("NP_TO_IS", -1),
    ("OFF_OF", 1),
    ("ONE_TO_MANY_HYPHEN", 1),
    ("ON_EXCEL", 1),
    ("ON_THE_LOOK_OUT", 1),
    ("ORDER_OF_WORDS_WITH_NOT", -1),
    ("ORDINAL_NUMBER_MISSING_ORDINAL_INDICATOR", -1),
    ("OTHER_WISE_COMPOUND", 1),
    ("OUTTA", -4),
    ("PASSIVE_VOICE", -600),
    ("PICTURE_PERFECT_HYPHEN", 1),
    ("PIECE_COMPOUNDS", 1),
    ("PLEASE_DO_NOT_THE_CAT", -15),
    ("PLEASE_LET_ME_KNOW", -1),
    ("PLURALITY_CONFUSION_OF_NNS_OF_NN", -1),
    ("PLURAL_VERB_AFTER_THIS", -1),
    ("POSSESSIVE_APOSTROPHE", -10),
    ("PREPOSITION_VERB", -1),
    ("PROFANITY", 1),
    ("PROFANITY_TYPOS", 2),
    ("PROFANITY_XML", 1),
    ("PROFITS_WARNINGS", 1),
    ("PRONOUN_NOUN", -26),
    ("PRP_ABLE_TO", 1),
    ("PRP_AREA", 1),
    ("PRP_JJ", -12),
    ("PRP_MD_NN", -12),
    ("PRP_NO_ADVERB_VERB", -15),
    ("PRP_NO_VB", 1),
    ("PRP_PRP", -1),
    ("PRP_RB_NO_VB", -12),
    ("PRP_THE", -12),
    ("PRP_VB", -25),
    ("PRP_VBG", -2),
    ("PRP_VB_NN", -25),
    ("PRP_VB_VB", -1),
    ("QUESTION_WITHOUT_VERB", -25),
    ("QUIET_QUITE", 1),
    ("RATHER_NOT_VB", 1),
    ("READ_ONLY_ACCESS_HYPHEN", 2),
    ("REASON_WHY", -600),
    ("REPEATED_VERBS", -1),
    ("REPETITIONS_STYLE", -51),
    ("REP_PASSIVE_VOICE", -599),
    ("RUDE_SARCASTIC", 6),
    ("RUN_ON", 1),
    ("SAFE_GUARD_COMPOUND", 1),
    ("SAVE_SAFE", 1),
    ("SEEMS_TO_BE", -51),
    ("SEEM_SEEN", 1),
    ("SEEN_SEEM", 1),
    ("SENTENCE_FRAGMENT", -51),
    ("SENT_START_NNP_COMMA", -1),
    ("SENT_START_NN_DT", -1),
    ("SENT_START_NN_NN_VB", -1),
    ("SENT_START_NUM", -600),
    ("SENT_START_PRPS_JJ_NN_VBP", -12),
    ("SHELL_COMPOUNDS", 1),
    ("SHOW_COMPOUNDS", 1),
    ("SINGLE_CHARACTER", -1),
    ("SINGULAR_AGREEMENT_SENT_START", -12),
    ("SINGULAR_NOUN_ADV_AGREEMENT", -12),
    ("SINGULAR_NOUN_VERB_AGREEMENT", -12),
    ("SPURIOUS_APOSTROPHE", 1),
    ("STEP_COMPOUNDS", 1),
    ("SUBJECTVERBAGREEMENT_2", -12),
    ("SUBJECT_VERB_AGREEMENT", -12),
    ("SUPERLATIVE_THAN", -1),
    ("SUPPOSE_TO", 1),
    ("THANK_YOUR", 1),
    ("THANK_YOU_MUCH", 1),
    ("THAN_THANK", 1),
    ("THERE_FORE", 1),
    ("THERE_THEIR", 1),
    ("THE_CC", -2),
    ("THE_FRENCH", 1),
    ("THE_IT", 1),
    ("THE_NNS_NN_IS", -12),
    ("THE_SENT_END", -12),
    ("THE_THEM", 1),
    ("THE_US", 1),
    ("THINK_BELIEVE_THAT", 1),
    ("THIS_MISSING_VERB", 1),
    ("THIS_YEARS_POSSESSIVE_APOSTROPHE", 1),
    ("THREE_NN", -600),
    ("TOO_LONG_PARAGRAPH", -998),
    ("TOO_LONG_SENTENCE", -997),
    ("TO_AFTER_MODAL_VERBS", -12),
    ("TO_DO_HYPHEN", 1),
    ("TO_NIGHT_TO_DAY", 1),
    ("TO_WORRIED_ABOUT", 1),
    ("TWO_CONNECTED_MODAL_VERBS", -15),
    ("T_HE", 1),
    ("ULTRA_HYPHEN", 1),
    ("UNITES_UNITED", 1),
    ("UNLIKELY_OPENING_PUNCTUATION", -1),
    ("UNNECESSARY_CAPITALIZATION", -1),
    ("UPPERCASE_SENTENCE_START", -11),
    ("VBP_VBP", -2),
    ("VBZ_VBD", -1),
    ("VB_A_JJ_NNS", -1),
    ("VB_TO_JJ", -15),
    ("VB_TO_NN_DT", -12),
    ("VERB_APOSTROPHE_S", -12),
    ("VERB_NOUN_CONFUSION", -1),
    ("WAKED_UP", -1),
    ("WANNA", 1),
    ("WANT_TO_NN", -25),
    ("WAN_T", 1),
    ("WEE_WE", 1),
    ("WERE_WEAR", 1),
    ("WE_BE", -1),
    ("WHATCHA", -4),
    ("WHATS_APP", 1),
    ("WHAT_IS_YOU", 1),
    ("WHERE_MD_VB", -12),
    ("WHO_NOUN", -1),
    ("WILL_BASED_ON", 1),
    ("WILL_BECOMING", 1),
    ("WONT_CONTRACTION", 1),
    ("WON_T_TO", 1),
    ("WORLDS_BEST", 1),
    ("WOULD_A", -2),
    ("WOULD_NEVER_VBN", 1),
    ("WRONG_APOSTROPHE", 5),
    ("YEAR_OLD_HYPHEN", 6),
    ("YOURE", 1),
    ("YOU_GOOD", 3),
    ("Y_ALL", -4),
];
// entry count: 328

static FALSE_FRIENDS_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^EN_FOR_[A-Z]+_SPEAKERS_FALSE_FRIENDS.*").unwrap());

/// `English.getPriorityForId`: explicit map, then id-prefix fallbacks, then
/// the base `Language` fallbacks.
pub fn priority_for_id(id: &str) -> i32 {
    if let Ok(index) = EN_PRIORITIES.binary_search_by(|(key, _)| (*key).cmp(id)) {
        return EN_PRIORITIES[index].1;
    }
    if id.starts_with("EN_COMPOUNDS_") {
        return 2;
    }
    if id == "PRP_VBZ" {
        return -2; // prefer other more specific rules (with suggestions)
    }
    if id.starts_with("CONFUSION_RULE_") {
        return -20;
    }
    if id == "EN_UPPER_CASE_NGRAM" {
        return -12;
    }
    if id.starts_with("AI_SPELLING_RULE") {
        return -9;
    }
    if id.starts_with("EN_MULTITOKEN_SPELLING_") {
        return -9; // higher than MORFOLOGIK_*
    }
    if id.starts_with("EN_GB_SIMPLE_REPLACE") || id.starts_with("EN_US_SIMPLE_REPLACE") {
        return -5; // higher than MORFOLOGIK_*
    }
    if id == "QB_EN_OXFORD" {
        return -51;
    }
    if id.starts_with("EN_SIMPLE_REPLACE") && (id.ends_with("GRAMME") || id.ends_with("GRAMMES")) {
        return -49; // Higher prio than Oxford rule
    }
    if id.starts_with("AI_HYDRA_LEO") {
        if id == "AI_HYDRA_LEO_MISSING_COMMA" {
            return -51;
        }
        if id.starts_with("AI_HYDRA_LEO_CP_YOU_YOUARE") {
            return -1;
        }
        if id.starts_with("AI_HYDRA_LEO_CP") {
            return 2;
        }
        if id.starts_with("AI_HYDRA_LEO_MISSING_TO") {
            return -14;
        }
        return -11;
    }
    if id.starts_with("AI_EN_LECTOR") {
        return -11;
    }
    if FALSE_FRIENDS_PATTERN.is_match(id) {
        return -21;
    }
    // base `Language.getPriorityForId`
    if id.eq_ignore_ascii_case("TOO_LONG_SENTENCE") {
        return -101;
    }
    if id == "REPETITIONS_STYLE" {
        return -55;
    }
    if id.contains("STYLE") {
        return -50;
    }
    0
}

/// `Language.getRulePriority` for English (`getDefaultRulePriorityForStyle`
/// is -50).
pub fn rule_priority(
    rule_id: &str,
    category_id: &str,
    issue_type: &str,
    rule_priority: i32,
) -> i32 {
    let rule_specific = priority_for_id(rule_id);
    if rule_specific != 0 {
        return rule_specific;
    }
    if rule_priority != 0 {
        return rule_priority;
    }
    let category = priority_for_id(category_id);
    if category != 0 {
        return category;
    }
    if issue_type.eq_ignore_ascii_case("style") {
        return -50;
    }
    0
}

/// `Rule.isIncludedInErrorsCorrectedAllAtOnce`.
pub fn included_in_all_at_once(rule_id: &str) -> bool {
    matches!(
        rule_id,
        "FALSCHES_ANFUEHRUNGSZEICHEN"
            | "TYPOGRAFISCHE_ANFUEHRUNGSZEICHEN"
            | "TYPOGRAFISCHE_ANFUEHRUNGSZEICHEN_CH"
            | "TYPOGRAFISCHE_ANFUEHRUNGSZEICHEN_2"
            | "TYPOGRAFISCHE_APOSTROPHE"
            | "EN_QUOTES"
            | "WRONG_APOSTROPHE"
            | "PARAGRAF_LEERZEICHEN"
            | "CONSISTENCYRULE_JE_F"
            | "CONSISTENCYRULE_JE_M"
            | "COMILLAS_TIPOGRAFICAS"
            | "COMETES_TIPOGRAFIQUES"
            | "TYPOGRAFISCHE_AANHALINGSTEKENS"
            | "PT_SMART_QUOTES"
            | "GL_SMART_QUOTES"
    )
}
