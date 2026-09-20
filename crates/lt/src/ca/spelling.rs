//! Catalan spelling rule (`MorfologikCatalanSpellerRule`): port of the
//! morfologik speller path of `MorfologikSpellerRule` and
//! `SpellingCheckRule` plus the Catalan overrides.
//!
//! Catalan specifics: the binary dictionary is `ca/spelling/ca-ES_spelling.dict`
//! (CFSA2, UTF-8) with the `ca-ES_spelling.info` speller options, the
//! ignore/prohibit lists are `ca/hunspell/{ignore,prohibit}.txt`, the plain
//! spell list is `ca/spelling.txt` (+ `ca/multiwords.txt`,
//! `ca/spelling-special.txt`, `core/spelling_global.txt`), `tokenizeNewWords()`
//! is false (multi-word entries stay raw ignore words) and
//! `setIgnoreTaggedWords()` is on.
//!
//! Stage 2 wires the base speller; `MorfologikCatalanSpellerRule`'s
//! `orderSuggestions` (tagger-based reordering, `inalambric`, the
//! lemma-ignore lists) and `getAdditionalTopSuggestions` (camel-case /
//! digits-at-end / apostrophe-hyphen splits, `PronomsFeblesHelper`) are
//! stage 3 (internal development notes).
use lt_data::PathExt as _;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, OnceLock};

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};

use crate::wordutil::{is_email, is_punctuation_mark, is_url};
use lt_spell::morfologik::{
    self, DictSource, MorfologikSpeller, MultiSpeller, SpellerMetadata, WeightedSuggestion,
};
use lt_tagger::DictionaryInfo;
use regex::Regex;

/// One suggestion candidate with its morfologik weight (LT
/// `SuggestedReplacement.getWeight`). The weight only matters inside the
/// Catalan `orderSuggestions` weight-jump filter; the final `Match` drops it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cand {
    value: String,
    short_description: Option<String>,
    weight: Option<i32>,
}

impl Cand {
    fn plain(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            short_description: None,
            weight: None,
        }
    }
}

/// `MorfologikCatalanSpellerRule.ParticulaInicial`.
static PARTICULA_INICIAL: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "amb", "sota", "no", "en", "a", "el", "els", "al", "als", "pel", "pels", "del", "dels",
        "del", "de", "per", "un", "uns", "una", "unes", "la", "les", "teu", "meu", "seu", "teus",
        "meus", "seus",
    ]
});
/// `MorfologikCatalanSpellerRule.Preposicions`.
static PREPOSICIONS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "amb", "en", "sota", "a", "al", "als", "pel", "pels", "pel", "dels", "del", "de", "per",
    ]
});
/// `MorfologikCatalanSpellerRule.PrefixAmbEspai`.
static PREFIX_AMB_ESPAI: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "pod", "ultra", "eco", "tele", "anti", "re", "des", "sen", "sem", "s", "avant", "auto",
        "ex", "extra", "macro", "mega", "meta", "micro", "multi", "mono", "mini", "post", "retro",
        "semi", "super", "trans", "pro", "g", "l", "m", "e", "pos", "acost",
    ]
});
/// `MorfologikCatalanSpellerRule.EspaiAmbSufixNo`.
static ESPAI_AMB_SUFIX_NO: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["mi", "lis", "isme"]);
/// `MorfologikCatalanSpellerRule.EspaiAmbSufixSi`.
static ESPAI_AMB_SUFIX_SI: LazyLock<Vec<&'static str>> = LazyLock::new(|| vec!["a", "o", "i"]);
/// `MorfologikCatalanSpellerRule.PronomInicial`.
static PRONOM_INICIAL: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "em", "et", "es", "se", "ens", "us", "vos", "li", "hi", "ho", "el", "la", "els", "les",
    ]
});
/// `MorfologikCatalanSpellerRule.LemmasToIgnore`.
const LEMMAS_TO_IGNORE: [&str; 6] = [
    "enterar",
    "sentar",
    "conseguir",
    "alcançar",
    "entimar",
    "pisar",
];
/// `MorfologikCatalanSpellerRule.LemmasToAllow`.
const LEMMAS_TO_ALLOW: [&str; 2] = ["enter", "sentir"];
/// `MorfologikCatalanSpellerRule.inalambric`.
static INALAMBRIC: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "inalàmbric",
        "inalàmbrica",
        "inalàmbrics",
        "inalàmbriques",
        "inalàmbricament",
        "inalàmbricamente",
    ]
});
/// `MorfologikCatalanSpellerRule.SPLIT_DIGITS_AT_END`.
static SPLIT_DIGITS_AT_END: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["en", "de", "del", "al", "dels", "als", "a", "i", "o", "amb"]);
/// `MorfologikCatalanSpellerRule.PronomsDarrere` (order matters:
/// `endsWithPronoun` returns the first list entry the word ends with).
static PRONOMS_DARRERE: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "losels", "losles", "nosels", "nosles", "vosels", "vosens", "vosles", "lesen", "leshi",
        "liles", "losel", "losen", "loshi", "losho", "losla", "lsels", "lsles", "meles", "nosel",
        "nosen", "noshi", "nosho", "nosla", "nosli", "nsels", "nsles", "seles", "sevos", "teles",
        "usels", "usens", "usles", "vosel", "vosem", "vosen", "voshi", "vosho", "vosla", "vosli",
        "lahi", "lihi", "liho", "lila", "lils", "lsel", "lsen", "lshi", "lsho", "lsla", "mela",
        "meli", "mels", "nsel", "nsen", "nshi", "nsho", "nsla", "nsli", "sela", "seli", "sels",
        "sens", "seus", "tela", "teli", "tels", "tens", "usel", "usem", "usen", "ushi", "usho",
        "usla", "usli", "lan", "len", "les", "lhi", "lil", "lin", "los", "mel", "men", "mhi",
        "mho", "nhi", "nos", "sel", "sem", "sen", "set", "shi", "sho", "tel", "tem", "ten", "thi",
        "tho", "vos", "hi", "ho", "la", "li", "lo", "ls", "me", "ne", "ns", "se", "te", "us", "mi",
        "nosi", "losi", "si", "lis",
    ]
});

/// Java `Pattern` full-match semantics for POS tags (the patterns are
/// unanchored, `matches()` requires the whole tag).
fn matches_pos_tag_regexp(atr: &AnalyzedTokenReadings, pattern: &Regex) -> bool {
    atr.readings.iter().any(|r| {
        let tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
        match pattern.find(tag) {
            Some(m) => m.start() == 0 && m.end() == tag.len(),
            None => false,
        }
    })
}

/// Java `AnalyzedTokenReadings.hasAnyLemma`.
fn has_any_lemma(atr: &AnalyzedTokenReadings, lemmas: &[&str]) -> bool {
    atr.readings.iter().any(|r| lemmas.contains(&r.lemma()))
}

/// LT `StringTools.IsAllLowercase`.
fn is_all_lowercase(word: &str) -> bool {
    !word.chars().any(|c| c.is_alphabetic() && !c.is_lowercase())
}

/// LT `StringTools.equalsIgnoreCaseAndDiacritics`.
fn equals_ignore_case_and_diacritics(a: &str, b: &str) -> bool {
    crate::multitoken::remove_diacritics(a)
        .to_lowercase()
        .eq(&crate::multitoken::remove_diacritics(b).to_lowercase())
}

/// LT `StringTools.splitCamelCase`.
fn split_camel_case(input: &str) -> Vec<String> {
    if morfologik::is_all_uppercase(input) {
        return vec![input.to_string()];
    }
    let mut word = String::new();
    let mut result = String::new();
    let mut previous_is_uppercase = false;
    for current_char in input.chars() {
        if current_char.is_uppercase() {
            if !previous_is_uppercase {
                result.push_str(&word);
                result.push(' ');
                word.clear();
            }
            previous_is_uppercase = true;
        } else {
            previous_is_uppercase = false;
        }
        word.push(current_char);
    }
    result.push_str(&word);
    let trimmed = result.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed.split(' ').map(str::to_string).collect()
    }
}

/// LT `StringTools.splitDigitsAtEnd`.
fn split_digits_at_end(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut last_index = chars.len() as isize - 1;
    while last_index >= 0 && chars[last_index as usize].is_ascii_digit() {
        last_index -= 1;
    }
    let non_digit: String = chars[..(last_index + 1) as usize].iter().collect();
    let digits: String = chars[(last_index + 1) as usize..].iter().collect();
    if !non_digit.is_empty() && !digits.is_empty() {
        vec![non_digit, digits]
    } else {
        vec![input.to_string()]
    }
}

pub const RULE_ID: &str = "MORFOLOGIK_RULE_CA_ES";
const DESCRIPTION: &str = "Possible error ortogràfic";
const MESSAGE: &str = "Possible error ortogràfic.";
const SHORT_MESSAGE: &str = "Error ortogràfic";
const CATEGORY_ID: &str = "TYPOS";
const CATEGORY_NAME: &str = "Errors ortogràfics";
/// `MorfologikSpellerRule.MAX_FREQUENCY_FOR_SPLITTING`
const MAX_FREQUENCY_FOR_SPLITTING: i32 = 21;
/// `SpellingCheckRule.MAX_TOKEN_LENGTH`
const MAX_TOKEN_LENGTH: usize = 200;
/// `MorfologikCatalanSpellerRule.MAX_WEIGHT_DIFF`
const MAX_WEIGHT_DIFF: i32 = 10;

/// `MorfologikCatalanSpellerRule.QUOTE_OR_HYPHEN`.
static QUOTE_OR_HYPHEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"['-]").unwrap());
static APOSTROF_INICI_VERBS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([lnts])[90]?(h?[aeiouàéèíòóú].*)$").unwrap());
static APOSTROF_INICI_VERBS_M: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(m)[90]?(h?[aeiouàéèíòóú].*)$").unwrap());
static APOSTROF_INICI_NOM_SING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([ld])[90]?(h?[aeiouàéèíòóú]...+)$").unwrap());
static APOSTROF_INICI_NOM_PLURAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(d)[90]?(h?[aeiouàéèíòóú].+)$").unwrap());
static APOSTROF_FINAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(...+[aei])[90]?(l|ls|m|ns|n|t)$").unwrap());
static APOSTROF_FINAL_S: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.+e)[90]?(s)$").unwrap());
static GUIONET_FINAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^([\p{L}·]+)[’']?(hi|ho|la|les|li|lo|los|me|ne|nos|se|te|vos)$").unwrap()
});
static GUIONET_FINAL_GERUNDI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^([\p{L}·]+n)(hi|ho|la|les|li|lo|los|me|ne|nos|se|te|vos)$").unwrap()
});
static VERB_INDSUBJ: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"V.[SI].*").unwrap());
static VERB_INDSUBJ_M: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"V.[SI].[123]S.*|V.[SI].[23]P.*").unwrap());
static NOM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"V.[NG].*|V.P.*|N.*|A.*|PX.*|DD.*").unwrap());
static NOM_SING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"V.[NG].*|V.P..S..|N..[SN].*|A...[SN].|PX..S...|DD..S.").unwrap());
static NOM_PLURAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"V.P..P..|N..[PN].*|A...[PN].|PX..P...|DD..P.").unwrap());
static VERB_INFGERIMP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"V.[NGM].*").unwrap());
static VERB_INF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"V.N.*").unwrap());
static VERB_GER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"V.G.*").unwrap());
static VERB_BALEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"V......B").unwrap());
static HAS_NO_LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\p{Latin}]+$").unwrap());
static STARTS_WITH_NUMBERS_BULLETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d[.,\d]*|\P{L}+)(.*)$").unwrap());
static STARTS_WITH_NUMBERS_BULLETS_EXCEPTIONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([\p{C}\-\$%&]+)(.*)$").unwrap());

pub struct CatalanSpellingRule {
    /// the binary `ca-ES_spelling.dict` alone (`MorfologikSpeller` for
    /// `FindSuggestionsFilter`'s `findSimilarWords`)
    binary_speller: MorfologikSpeller,
    speller1: MultiSpeller,
    speller2: MultiSpeller,
    speller3: MultiSpeller,
    /// `SpellingCheckRule.wordsToBeIgnored` (case-sensitive, like Java);
    /// `tokenizeNewWords()` is false for Catalan, so multi-word entries stay
    /// raw and are treated as known words.
    ignore: HashSet<String>,
    /// `SpellingCheckRule.wordsToBeProhibited`
    prohibit: HashSet<String>,
    /// `MorfologikCatalanSpellerRule` calls `setIgnoreTaggedWords()`
    ignore_tagged_words: bool,
    /// `CatalanTagger.INSTANCE_CAT` / `INSTANCE_VAL` (the Catalan
    /// `orderSuggestions` and `getAdditionalTopSuggestions` overrides run the
    /// tagger). Set by the pipeline / probe after construction.
    tagger: OnceLock<Arc<lt_tagger::CatalanTagger>>,
    suggestion_cache:
        std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Vec<Cand>>>>,
}

impl CatalanSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let spelling_dir = data_dir.join("ca/spelling");
        let info = DictionaryInfo::load(&spelling_dir.join("ca-ES_spelling.info"))?;
        let meta = SpellerMetadata::from_info(&info)?;
        let binary_source = Arc::new(DictSource::from_dict_file(
            &spelling_dir.join("ca-ES_spelling.dict"),
            &spelling_dir.join("ca-ES_spelling.info"),
        )?);
        let binary = |distance: i32| -> MorfologikSpeller {
            MorfologikSpeller::from_source(Arc::clone(&binary_source), meta.clone(), distance)
        };
        let lines = load_plain_text_dict_lines(data_dir);
        let plain_source = Arc::new(DictSource::from_lines(&lines));
        let plain = |distance: i32| -> MorfologikSpeller {
            MorfologikSpeller::from_source(Arc::clone(&plain_source), meta.clone(), distance)
        };
        let binary_speller = binary(1);
        let speller1 = MultiSpeller::new(vec![binary(1), plain(1)], vec![0, 1]);
        let speller2 = MultiSpeller::new(vec![binary(2), plain(2)], vec![0, 1]);
        let speller3 = MultiSpeller::new(vec![binary(3), plain(3)], vec![0, 1]);

        let mut rule = Self {
            binary_speller,
            speller1,
            speller2,
            speller3,
            ignore: HashSet::new(),
            prohibit: HashSet::new(),
            ignore_tagged_words: true,
            tagger: OnceLock::new(),
            suggestion_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        };
        // `SpellingCheckRule.init`: ignore file, spelling file, additional
        // spelling files (custom, global, multiwords, spelling-special), then
        // the prohibit files.
        for path in [
            data_dir.join("ca/hunspell/ignore.txt"),
            data_dir.join("ca/words/spelling.txt"),
            data_dir.join("ca/hunspell/spelling_custom.txt"),
            data_dir.join("core/spelling_global.txt"),
            data_dir.join("ca/words/multiwords.txt"),
            data_dir.join("ca/words/spelling-special.txt"),
        ] {
            rule.load_ignore(&path);
        }
        for path in [
            data_dir.join("ca/hunspell/prohibit.txt"),
            data_dir.join("ca/hunspell/prohibit_custom.txt"),
        ] {
            rule.load_prohibit(&path);
        }
        Ok(rule)
    }

    pub fn rule_id(&self) -> &str {
        RULE_ID
    }

    /// `MorfologikCatalanSpellerRule`'s tagger (`INSTANCE_CAT`/`INSTANCE_VAL`
    /// depending on the variant). The pipeline/probe sets it once.
    pub fn set_tagger(&self, tagger: Arc<lt_tagger::CatalanTagger>) {
        let _ = self.tagger.set(tagger);
    }

    fn tagger(&self) -> Option<&Arc<lt_tagger::CatalanTagger>> {
        self.tagger.get()
    }

    /// The binary-dictionary speller (`FindSuggestionsFilter`).
    pub fn dict_speller(&self) -> &MorfologikSpeller {
        &self.binary_speller
    }

    /// `MorfologikSpellerRule.getSpellingSuggestions` (one synthetic token).
    pub fn suggestions(&self, word: &str) -> Vec<String> {
        let token = lt_core::AnalyzedTokenReadings::new(vec![lt_core::AnalyzedToken::new(
            word, None, None,
        )]);
        self.check_sentence(&[token], 0)
            .into_iter()
            .next()
            .map(|m| m.suggestions.into_iter().map(|s| s.value).collect())
            .unwrap_or_default()
    }

    fn load_ignore(&mut self, path: &Path) {
        // `addIgnoreWords` with `tokenizeNewWords() == false`: every line is
        // an accepted word as-is (no IGNORE_SPELLING anti-patterns).
        for word in cache_word_list(path) {
            self.ignore.insert(word);
        }
    }

    fn load_prohibit(&mut self, path: &Path) {
        for word in cache_word_list(path) {
            self.prohibit.insert(word);
        }
    }

    /// `MorfologikSpellerRule.isMisspelled(speller1, word)` with the
    /// `updateIgnoredWordDictionary` accepted words; `checkCompound` is false
    /// for Catalan.
    pub(crate) fn is_misspelled(&self, word: &str) -> bool {
        if self.ignore.contains(word) {
            return false;
        }
        self.speller1.is_misspelled(word)
    }

    pub(crate) fn is_prohibited(&self, word: &str) -> bool {
        self.prohibit.contains(word)
    }

    fn is_ignored_no_case(&self, word: &str) -> bool {
        let converts_case = true;
        self.ignore.contains(word)
            || (!morfologik::is_mixed_case(word)
                && converts_case
                && self.ignore.contains(&word.to_lowercase()))
    }

    /// `SpellingCheckRule.ignoreWord(String)`.
    pub(crate) fn ignore_word(&self, word: &str) -> bool {
        if word.chars().count() > MAX_TOKEN_LENGTH {
            return true;
        }
        if HAS_NO_LETTER.is_match(word) {
            return true;
        }
        if let Some(stripped) = word.strip_suffix('.') {
            if !self.ignore.contains(word) {
                return self.is_ignored_no_case(stripped);
            }
        }
        self.is_ignored_no_case(word)
    }

    /// `SpellingCheckRule.ignoreWord(String)` + `StringTools.isEmoji`.
    fn ignore_word_with_emoji(&self, word: &str) -> bool {
        self.ignore_word(word) || lt_tagger::is_emoji(word)
    }

    /// `MorfologikSpellerRule.canBeIgnored`.
    fn can_be_ignored(
        &self,
        tokens: &[&AnalyzedTokenReadings],
        idx: usize,
        token: &AnalyzedTokenReadings,
    ) -> bool {
        token.is_sentence_start
            || token.is_immunized
            || token.is_ignore_spelling
            || is_url(token.surface())
            || is_email(token.surface())
            || (self.ignore_tagged_words && token.is_tagged && !self.is_prohibited(token.surface()))
            || self.ignore_word_with_emoji(tokens[idx].surface())
    }

    /// `MorfologikSpellerRule.match` over one sentence's token stream
    /// (absolute byte offsets already applied by the caller).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let non_blank: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        // `tokenizeNewWords()` is false for Catalan, so `addIgnoreWords` adds
        // every line to `wordsToBeIgnored` as-is; no IGNORE_SPELLING
        // anti-patterns exist (unlike Dutch).
        let mut matches: Vec<Match> = Vec::new();
        let mut is_first_word = true;
        for (idx, token) in non_blank.iter().enumerate() {
            if self.can_be_ignored(&non_blank, idx, token) {
                if idx > 0 && is_first_word && !is_punctuation_mark(token.surface()) {
                    is_first_word = false;
                }
                continue;
            }
            let start_pos = token.start_pos;
            let word = token
                .readings
                .first()
                .map(|r| r.token.clone())
                .unwrap_or_else(|| token.surface().to_string());
            let new_matches =
                self.get_rule_matches(&word, start_pos, &mut matches, idx, &non_blank);
            matches.extend(new_matches);

            // Capitalize the (first) match's suggestions when the word is the
            // sentence's first word and not its last token.
            if is_first_word && !matches.is_empty() && idx < non_blank.len() - 1 {
                let values: Vec<String> = matches[0]
                    .suggestions
                    .iter()
                    .map(|s| s.value.clone())
                    .collect();
                let mut new_values: Vec<String> = Vec::new();
                for replacement in values {
                    if replacement == replacement.to_lowercase() {
                        let capitalized = morfologik::uppercase_first_char(&replacement);
                        if !new_values.contains(&capitalized) {
                            new_values.push(capitalized);
                        }
                    } else if !new_values.contains(&replacement) {
                        new_values.push(replacement);
                    }
                }
                matches[0].suggestions = new_values
                    .into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect();
            }
            if idx > 0 && is_first_word && !is_punctuation_mark(token.surface()) {
                is_first_word = false;
            }
        }
        let mut out = Vec::with_capacity(matches.len());
        for mut m in matches {
            m.range = TextRange::new(
                sentence_offset + m.range.start,
                sentence_offset + m.range.end,
            );
            out.push(m);
        }
        out
    }

    /// `MorfologikCatalanSpellerRule.getRuleMatches` (no overrides beyond
    /// `MorfologikSpellerRule`).
    fn get_rule_matches(
        &self,
        word: &str,
        start_pos: usize,
        rule_matches_so_far: &mut Vec<Match>,
        idx: usize,
        tokens: &[&AnalyzedTokenReadings],
    ) -> Vec<Match> {
        let mut rule_matches: Vec<Match> = Vec::new();
        let mut rule_match: Option<Match> = None;

        if !self.is_misspelled(word) && !self.is_prohibited(word) {
            return rule_matches;
        }
        if rule_matches_so_far
            .last()
            .is_some_and(|m| m.range.end > start_pos)
        {
            return rule_matches;
        }

        let mut before_suggestion_str = String::new();
        let mut after_suggestion_str = String::new();

        // Check for split word with previous word
        if idx > 0 && tokens[idx].whitespace_before {
            let prev_word = tokens[idx - 1].surface().to_string();
            if !prev_word.is_empty()
                && !prev_word.chars().any(|c| c.is_ascii_digit())
                && self.speller1.get_frequency(&prev_word) < MAX_FREQUENCY_FOR_SPLITTING
            {
                let prev_start_pos = tokens[idx - 1].start_pos;
                // "thanky ou" -> "thank you"
                if let Some((prev_head, prev_tail)) = split_last_char(&prev_word) {
                    let sugg1a = prev_head.to_string();
                    let sugg1b = format!("{prev_tail}{word}");
                    if sugg1a.chars().count() > 1
                        && sugg1b.chars().count() > 2
                        && !self.is_misspelled(&sugg1a)
                        && !self.is_misspelled(&sugg1b)
                        && self.speller1.get_frequency(&sugg1a)
                            + self.speller1.get_frequency(&sugg1b)
                            > self.speller1.get_frequency(&prev_word)
                    {
                        rule_match = Some(self.create_wrong_split_match(
                            rule_matches_so_far,
                            tokens[idx].end_pos(),
                            &sugg1a,
                            &sugg1b,
                            prev_start_pos,
                        ));
                        before_suggestion_str = format!("{prev_word} ");
                    }
                }
                // "than kyou" -> "thank you"
                {
                    let first_char: String = word.chars().take(1).collect();
                    let sugg2a = format!("{prev_word}{first_char}");
                    let sugg2b: String = word.chars().skip(1).collect();
                    if sugg2b.chars().count() > 2
                        && !self.is_misspelled(&sugg2a)
                        && !self.is_misspelled(&sugg2b)
                    {
                        if rule_match.is_none() {
                            if self.speller1.get_frequency(&sugg2a)
                                + self.speller1.get_frequency(&sugg2b)
                                > self.speller1.get_frequency(&prev_word)
                            {
                                rule_match = Some(self.create_wrong_split_match(
                                    rule_matches_so_far,
                                    tokens[idx].end_pos(),
                                    &sugg2a,
                                    &sugg2b,
                                    prev_start_pos,
                                ));
                                before_suggestion_str = format!("{prev_word} ");
                            }
                        } else if let Some(rm) = &mut rule_match {
                            rm.suggestions.push(Suggestion {
                                value: format!("{sugg2a} {sugg2b}").trim().to_string(),
                                short_description: None,
                            });
                        }
                    }
                }
                // "g oing" -> "going"
                let sugg = format!("{prev_word}{word}");
                if word == word.to_lowercase() && !self.is_misspelled(&sugg) {
                    if rule_match.is_none() {
                        if self.speller1.get_frequency(&sugg)
                            >= self.speller1.get_frequency(&prev_word)
                        {
                            let mut m = self.new_rule_match(
                                prev_start_pos,
                                tokens[idx].end_pos(),
                                MESSAGE,
                                SHORT_MESSAGE,
                            );
                            before_suggestion_str = format!("{prev_word} ");
                            m.suggestions.push(Suggestion {
                                value: sugg.clone(),
                                short_description: None,
                            });
                            rule_match = Some(m);
                        }
                    } else if let Some(rm) = &mut rule_match {
                        rm.suggestions.push(Suggestion {
                            value: sugg.clone(),
                            short_description: None,
                        });
                    }
                }
                if rule_match.is_some() && self.is_misspelled(&prev_word) {
                    rule_matches.push(rule_match.take().unwrap());
                    return rule_matches;
                }
            }
        }

        // Check for split word with next word
        if rule_match.is_none() && idx < tokens.len() - 1 && tokens[idx + 1].whitespace_before {
            let next_word = tokens[idx + 1].surface().to_string();
            if !next_word.is_empty()
                && !next_word.chars().any(|c| c.is_ascii_digit())
                && self.speller1.get_frequency(&next_word) < MAX_FREQUENCY_FOR_SPLITTING
            {
                if let Some((word_head, word_tail)) = split_last_char(word) {
                    let sugg1a = word_head.to_string();
                    let sugg1b = format!("{word_tail}{next_word}");
                    if sugg1a.chars().count() > 1
                        && sugg1b.chars().count() > 2
                        && !self.is_misspelled(&sugg1a)
                        && !self.is_misspelled(&sugg1b)
                        && self.speller1.get_frequency(&sugg1a)
                            + self.speller1.get_frequency(&sugg1b)
                            > self.speller1.get_frequency(&next_word)
                    {
                        rule_match = Some(self.create_wrong_split_match(
                            rule_matches_so_far,
                            tokens[idx + 1].end_pos(),
                            &sugg1a,
                            &sugg1b,
                            start_pos,
                        ));
                        after_suggestion_str = format!(" {next_word}");
                    }
                }
                {
                    let first_char: String = next_word.chars().take(1).collect();
                    let sugg2a = format!("{word}{first_char}");
                    let sugg2b: String = next_word.chars().skip(1).collect();
                    if sugg2b.chars().count() > 2
                        && !self.is_misspelled(&sugg2a)
                        && !self.is_misspelled(&sugg2b)
                    {
                        if rule_match.is_none() {
                            if self.speller1.get_frequency(&sugg2a)
                                + self.speller1.get_frequency(&sugg2b)
                                > self.speller1.get_frequency(&next_word)
                            {
                                rule_match = Some(self.create_wrong_split_match(
                                    rule_matches_so_far,
                                    tokens[idx + 1].end_pos(),
                                    &sugg2a,
                                    &sugg2b,
                                    start_pos,
                                ));
                                after_suggestion_str = format!(" {next_word}");
                            }
                        } else if let Some(rm) = &mut rule_match {
                            rm.suggestions.push(Suggestion {
                                value: format!("{sugg2a} {sugg2b}").trim().to_string(),
                                short_description: None,
                            });
                        }
                    }
                }
                let sugg = format!("{word}{next_word}");
                if next_word == next_word.to_lowercase() && !self.is_misspelled(&sugg) {
                    if rule_match.is_none() {
                        if self.speller1.get_frequency(&sugg)
                            >= self.speller1.get_frequency(&next_word)
                        {
                            let mut m = self.new_rule_match(
                                start_pos,
                                tokens[idx + 1].end_pos(),
                                MESSAGE,
                                SHORT_MESSAGE,
                            );
                            after_suggestion_str = format!(" {next_word}");
                            m.suggestions.push(Suggestion {
                                value: sugg.clone(),
                                short_description: None,
                            });
                            rule_match = Some(m);
                        }
                    } else if let Some(rm) = &mut rule_match {
                        rm.suggestions.push(Suggestion {
                            value: sugg.clone(),
                            short_description: None,
                        });
                    }
                }
                if rule_match.is_some() && self.is_misspelled(&next_word) {
                    rule_matches.push(rule_match.take().unwrap());
                    return rule_matches;
                }
            }
        }

        let mut prevent_further_suggestions = false;
        let mut clean_word = word.to_string();

        if rule_match.is_none() {
            rule_match =
                Some(self.new_rule_match(start_pos, tokens[idx].end_pos(), MESSAGE, SHORT_MESSAGE));
        }

        // word starting with numbers or bullets
        if let Some(caps) = STARTS_WITH_NUMBERS_BULLETS.captures(word) {
            if !STARTS_WITH_NUMBERS_BULLETS_EXCEPTIONS.is_match(word) {
                let first_part = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let second_part = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let second_part_tokens = lt_tokenize::wordtokenizer::join_emails_and_urls(
                    lt_tokenize::wordtokenizer::string_tokenize(
                        second_part,
                        &lt_tokenize::wordtokenizer::base_tokenizing_characters(),
                    ),
                );
                let multitoken_is_misspelled =
                    second_part_tokens.iter().any(|s| self.is_misspelled(s));
                if (!multitoken_is_misspelled || self.is_ignored_no_case(second_part))
                    && !self.is_prohibited(second_part)
                {
                    if let Some(rm) = &mut rule_match {
                        rm.suggestions.push(Suggestion {
                            value: format!("{first_part} {second_part}"),
                            short_description: None,
                        });
                    }
                    prevent_further_suggestions = true;
                } else {
                    before_suggestion_str = format!("{first_part} ");
                    clean_word = second_part.to_string();
                }
            }
        }

        let prev_suggestions = rule_match
            .as_ref()
            .map(|m| m.suggestions.clone())
            .unwrap_or_default();
        if !prevent_further_suggestions {
            let mut joined = Vec::new();
            let cached = self.calc_speller_suggestions_cached(&clean_word);
            for suggestion in cached.iter() {
                joined.push(Suggestion {
                    value: format!(
                        "{before_suggestion_str}{}{after_suggestion_str}",
                        suggestion.value
                    ),
                    short_description: suggestion.short_description.clone(),
                });
            }
            let mut all = prev_suggestions;
            all.extend(joined);
            if let Some(rm) = &mut rule_match {
                rm.suggestions = all;
            }
        }

        if let Some(rm) = rule_match {
            let mut rule_match = rm;
            rule_match.suggestions = dedupe(rule_match.suggestions);
            rule_matches.push(rule_match);
        }
        rule_matches
    }

    /// `SpellingCheckRule.createWrongSplitMatch`.
    fn create_wrong_split_match(
        &self,
        rule_matches_so_far: &mut Vec<Match>,
        end_pos: usize,
        suggestion1: &str,
        suggestion2: &str,
        prev_pos: usize,
    ) -> Match {
        if let Some(prev) = rule_matches_so_far.last() {
            if prev.range.start == prev_pos {
                rule_matches_so_far.pop();
            }
        }
        let mut m = self.new_rule_match(prev_pos, end_pos, MESSAGE, SHORT_MESSAGE);
        m.suggestions.push(Suggestion {
            value: format!("{suggestion1} {suggestion2}").trim().to_string(),
            short_description: None,
        });
        m
    }

    fn new_rule_match(&self, from: usize, to: usize, message: &str, short_message: &str) -> Match {
        Match::new(
            RULE_ID,
            Option::<String>::None,
            message,
            Some(short_message.to_string()),
            TextRange::new(from, to),
            Vec::new(),
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(DESCRIPTION, "misspelling", 0)
        .with_match_type("UnknownWord")
    }

    fn calc_speller_suggestions_cached(&self, word: &str) -> std::sync::Arc<Vec<Cand>> {
        if let Ok(cache) = self.suggestion_cache.lock() {
            if let Some(hit) = cache.get(word) {
                return std::sync::Arc::clone(hit);
            }
        }
        let computed = std::sync::Arc::new(self.calc_speller_suggestions(word));
        if let Ok(mut cache) = self.suggestion_cache.lock() {
            if cache.len() >= 50_000 {
                cache.clear();
            }
            cache.insert(word.to_string(), std::sync::Arc::clone(&computed));
        }
        computed
    }

    /// `MorfologikSpellerRule.calcSpellerSuggestions` with the Catalan
    /// `getAdditionalTopSuggestions`/`orderSuggestions` overrides and the
    /// morfologik weights (needed by the weight-jump filter).
    fn calc_speller_suggestions(&self, word: &str) -> Vec<Cand> {
        let mut default_suggestions: Vec<Cand> = self
            .speller1
            .get_weighted_suggestions_from_default_dicts(word)
            .into_iter()
            .map(weighted_to_cand)
            .collect();
        let user_suggestions: Vec<Cand> = Vec::new();
        let only_case_differs = default_suggestions
            .first()
            .is_some_and(|s| s.value.eq_ignore_ascii_case(word));
        let full_results = false;
        if word.chars().count() >= 3
            && (only_case_differs || full_results || default_suggestions.is_empty())
        {
            default_suggestions.extend(
                self.speller2
                    .get_weighted_suggestions_from_default_dicts(word)
                    .into_iter()
                    .map(weighted_to_cand),
            );
            if word.chars().count() >= 5 && (full_results || default_suggestions.is_empty()) {
                default_suggestions.extend(
                    self.speller3
                        .get_weighted_suggestions_from_default_dicts(word)
                        .into_iter()
                        .map(weighted_to_cand),
                );
            }
        }
        // `MorfologikCatalanSpellerRule.getAdditionalTopSuggestions`
        // (camel-case / digits-at-end / apostrophe-hyphen splits).
        let top_suggestions = self.additional_top_suggestions(word);
        default_suggestions.splice(0..0, top_suggestions);

        if default_suggestions.is_empty() && user_suggestions.is_empty() {
            return Vec::new();
        }
        let default_suggestions = self.filter_suggestions(default_suggestions);
        let user_suggestions = dedupe(user_suggestions);
        let default_suggestions = self.order_suggestions(default_suggestions, word);
        if word.chars().count() > 4 {
            user_suggestions
                .into_iter()
                .chain(default_suggestions)
                .collect()
        } else {
            default_suggestions
                .into_iter()
                .chain(user_suggestions)
                .collect()
        }
    }

    /// `MorfologikCatalanSpellerRule.getAdditionalTopSuggestionsString`
    /// (the Catalan override; the base sentinels are not used).
    fn additional_top_suggestions(&self, word: &str) -> Vec<Cand> {
        let parts = split_camel_case(word);
        if parts.len() > 1 && parts[0].chars().count() > 1 {
            let mut is_not_misspelled = true;
            for part in &parts {
                // Java `speller1.isMisspelled` (no ignore-list shortcut).
                is_not_misspelled &= !self.speller1.is_misspelled(part);
            }
            if is_not_misspelled {
                return vec![Cand::plain(parts.join(" "))];
            }
        }
        let parts = split_digits_at_end(word);
        if parts.len() > 1 {
            let tagged = self
                .tagger()
                .map(|t| t.tag(&[parts[0].clone()]))
                .and_then(|atrs| atrs.first().map(|atr| atr.is_tagged))
                .unwrap_or(false);
            let first = parts[0].to_lowercase();
            if tagged
                && (parts[0].chars().count() > 2 || SPLIT_DIGITS_AT_END.contains(&first.as_str()))
            {
                return vec![Cand::plain(parts.join(" "))];
            }
        }
        let mut suggestion = String::new();
        suggestion = self.find_suggestion(
            suggestion,
            word,
            &APOSTROF_INICI_VERBS,
            &VERB_INDSUBJ,
            2,
            "'",
            "",
        );
        suggestion = self.find_suggestion(
            suggestion,
            word,
            &APOSTROF_INICI_VERBS_M,
            &VERB_INDSUBJ_M,
            2,
            "'",
            "",
        );
        suggestion = self.find_suggestion(
            suggestion,
            word,
            &APOSTROF_INICI_NOM_SING,
            &NOM_SING,
            2,
            "'",
            "",
        );
        suggestion = self.find_suggestion(
            suggestion,
            word,
            &APOSTROF_INICI_NOM_PLURAL,
            &NOM_PLURAL,
            2,
            "'",
            "",
        );
        suggestion = self.find_suggestion(
            suggestion,
            word,
            &APOSTROF_FINAL,
            &VERB_INFGERIMP,
            1,
            "'",
            "",
        );
        suggestion =
            self.find_suggestion(suggestion, word, &APOSTROF_FINAL_S, &VERB_INF, 1, "'", "");
        suggestion = self.find_suggestion(
            suggestion,
            word,
            &GUIONET_FINAL_GERUNDI,
            &VERB_GER,
            1,
            "-",
            "t",
        );
        suggestion = self.find_suggestion(
            suggestion,
            word,
            &GUIONET_FINAL,
            &VERB_INFGERIMP,
            1,
            "-",
            "",
        );
        suggestion = self.find_suggestion_multiple_pronouns(suggestion, word);
        if !suggestion.is_empty() {
            return vec![Cand::plain(suggestion)];
        }
        Vec::new()
    }

    /// `MorfologikCatalanSpellerRule.findSuggestion`.
    #[allow(clippy::too_many_arguments)]
    fn find_suggestion(
        &self,
        suggestion: String,
        word: &str,
        word_pattern: &Regex,
        postag_pattern: &Regex,
        suggestion_position: usize,
        separator: &str,
        add_str: &str,
    ) -> String {
        if !suggestion.is_empty() {
            return suggestion;
        }
        let Some(matcher) = word_pattern.captures(word) else {
            return String::new();
        };
        let group = |i: usize| matcher.get(i).map(|m| m.as_str()).unwrap_or("").to_string();
        let new_suggestion = format!("{}{}", group(suggestion_position), add_str);
        let Some(tagger) = self.tagger() else {
            return String::new();
        };
        let atrs = tagger.tag(std::slice::from_ref(&new_suggestion));
        let Some(newatr) = atrs.first() else {
            return String::new();
        };
        if (!newatr.has_pos_tag("VMIP1S0B")
            || new_suggestion.eq_ignore_ascii_case("fer")
            || new_suggestion.eq_ignore_ascii_case("ajust")
            || new_suggestion.eq_ignore_ascii_case("gran"))
            && matches_pos_tag_regexp(newatr, postag_pattern)
        {
            return format!("{}{}{}{}", group(1), add_str, separator, group(2));
        }
        String::new()
    }

    /// `MorfologikCatalanSpellerRule.findSuggestionMultiplePronouns`
    /// (`anarsen` -> `anar-se'n`, `danarsen` -> `d'anar-se'n`).
    fn find_suggestion_multiple_pronouns(&self, suggestion: String, word: &str) -> String {
        if !suggestion.is_empty() {
            return suggestion;
        }
        let Some(tagger) = self.tagger() else {
            return String::new();
        };
        let lcword = word.to_lowercase();
        let pronouns = ends_with_pronoun(&lcword);
        let mut verb: String = lcword
            .chars()
            .take(word.chars().count() - pronouns.chars().count())
            .collect();
        let atrs = tagger.tag(&[verb.clone()]);
        let matches_infgerimp = atrs
            .first()
            .is_some_and(|atr| matches_pos_tag_regexp(atr, &VERB_INFGERIMP));
        if matches_infgerimp {
            return format!(
                "{}{}",
                verb,
                crate::ca::helpers::transform_darrere(&pronouns, &verb)
            );
        }
        if verb.chars().count() < 5 {
            return String::new();
        }
        if lcword.starts_with('d') || lcword.starts_with('l') {
            verb = verb.chars().skip(1).collect();
            let atrs = tagger.tag(&[verb.clone()]);
            let matches_inf = atrs
                .first()
                .is_some_and(|atr| matches_pos_tag_regexp(atr, &VERB_INF));
            if matches_inf {
                let first_char = lcword.chars().next().unwrap_or('\0');
                return format!(
                    "{first_char}'{verb}{}",
                    crate::ca::helpers::transform_darrere(&pronouns, &verb)
                );
            }
        }
        String::new()
    }

    /// `SpellingCheckRule.filterSuggestions` (ends with `filterDupes` =
    /// `Stream.distinct()`, which compares `SuggestedReplacement` equality —
    /// replacement + short description, *not* the weight: a curated top
    /// suggestion and the raw morfologik one with the same text collapse).
    fn filter_suggestions(&self, suggestions: Vec<Cand>) -> Vec<Cand> {
        let mut seen: Vec<(String, Option<String>)> = Vec::new();
        let mut out = Vec::with_capacity(suggestions.len());
        for s in suggestions {
            if self.is_prohibited(&s.value) {
                continue;
            }
            let key = (s.value.clone(), s.short_description.clone());
            if !seen.contains(&key) {
                seen.push(key);
                out.push(s);
            }
        }
        out
    }

    /// `MorfologikCatalanSpellerRule.orderSuggestions` (the run-on-word
    /// reordering, tagger filters and the weight-jump filter).
    fn order_suggestions(&self, suggestions: Vec<Cand>, word: &str) -> Vec<Cand> {
        let Some(tagger) = self.tagger() else {
            return suggestions;
        };
        let mut new_suggestions: Vec<Cand> = Vec::new();
        let word_without_diacritics = crate::multitoken::remove_diacritics(word);
        let replacements: Vec<String> = suggestions.iter().map(|s| s.value.clone()).collect();
        for (i, cand) in suggestions.iter().enumerate() {
            let replacement = cand.value.clone();
            // avoid duplicate capitalized replacement
            if word == word.to_lowercase()
                && morfologik::is_capitalized_word(&replacement)
                && replacements.contains(&replacement.to_lowercase())
            {
                continue;
            }
            // avoid capitalized suggestions if there are previous all lower
            // case suggestions
            if i > 0
                && !new_suggestions.is_empty()
                && is_all_lowercase(word)
                && morfologik::is_capitalized_word(&replacement)
                && is_all_lowercase(&new_suggestions[0].value)
                && !equals_ignore_case_and_diacritics(word, &replacement)
            {
                continue;
            }
            if INALAMBRIC.contains(&replacement.to_lowercase().as_str()) {
                return vec![
                    Cand::plain("sense fils"),
                    Cand::plain("sense fil"),
                    Cand::plain("sense cables"),
                    Cand::plain("autònom"),
                ];
            }
            // remove always
            if replacement.eq_ignore_ascii_case("como") {
                continue;
            }
            let words_to_analyze: Vec<String> = replacement
                .split([' ', '\'', '-'])
                .map(str::to_string)
                .collect();
            let newatrs = tagger.tag(&words_to_analyze);
            if newatrs.iter().any(|atr| {
                has_any_lemma(atr, &LEMMAS_TO_IGNORE) && !has_any_lemma(atr, &LEMMAS_TO_ALLOW)
            }) {
                continue;
            }
            let mut replacement = replacement;
            // l'_ : remove superfluous space
            if replacement.contains("' ") {
                replacement = replacement.replace("' ", "'");
            }
            let parts: Vec<String> = replacement.split(' ').map(str::to_string).collect();
            if parts.len() == 2 {
                if parts[1].eq_ignore_ascii_case("s") {
                    continue;
                }
                // remove wrong split prefixes
                if PREFIX_AMB_ESPAI.contains(&parts[0].to_lowercase().as_str()) {
                    continue;
                }
                // remove wrong split sufixes
                if ESPAI_AMB_SUFIX_NO.contains(&parts[1].to_lowercase().as_str()) {
                    continue;
                }
                // allow only one-letter second part " a", " o", " i"
                if parts[1].chars().count() == 1
                    && !ESPAI_AMB_SUFIX_SI.contains(&parts[1].to_lowercase().as_str())
                {
                    continue;
                }
                // remove preposition + inflected verb
                if parts[1].chars().count() > 1
                    && PREPOSICIONS.contains(&parts[0].to_lowercase().as_str())
                {
                    let atkn = tagger.tag(&[parts[1].clone()]);
                    if atkn.first().is_some_and(|atr| {
                        matches_pos_tag_regexp(atr, &VERB_INDSUBJ)
                            && !matches_pos_tag_regexp(atr, &NOM)
                    }) {
                        continue;
                    }
                }
            }

            // Don't change first suggestions if they match word without
            // diacritics
            let mut pos_new_sugg = 0usize;
            while new_suggestions.len() > pos_new_sugg
                && crate::multitoken::remove_diacritics(&new_suggestions[pos_new_sugg].value)
                    .to_lowercase()
                    == word_without_diacritics.to_lowercase()
            {
                pos_new_sugg += 1;
            }

            // move some split words to first place
            if parts.len() == 2 {
                let atkn0 = tagger.tag(&[parts[0].clone()]);
                let atkn1 = tagger.tag(&[parts[1].clone()]);
                let is_balear = |atrs: &[AnalyzedTokenReadings]| {
                    atrs.first().is_some_and(|atr| {
                        matches_pos_tag_regexp(atr, &VERB_BALEAR)
                            && !atr.has_pos_tag_starting_with("N")
                            && !atr.has_pos_tag_starting_with("S")
                    })
                };
                if is_balear(&atkn0) || is_balear(&atkn1) {
                    continue;
                }
                if parts[1].chars().count() > 1
                    && PARTICULA_INICIAL.contains(&parts[0].to_lowercase().as_str())
                {
                    new_suggestions.insert(pos_new_sugg, cand.clone());
                    continue;
                }
                if parts[1].chars().count() > 1
                    && PRONOM_INICIAL.contains(&parts[0].to_lowercase().as_str())
                    && atkn1
                        .first()
                        .is_some_and(|atr| matches_pos_tag_regexp(atr, &VERB_INDSUBJ))
                {
                    new_suggestions.insert(pos_new_sugg, cand.clone());
                    continue;
                }
            }

            let sugg_without_diacritics = crate::multitoken::remove_diacritics(&replacement);
            if crate::multitoken::remove_diacritics(word)
                .to_lowercase()
                .eq(&sugg_without_diacritics.to_lowercase())
            {
                new_suggestions.insert(pos_new_sugg, cand.clone());
                continue;
            }

            // move words with apostrophe or hyphen to second position
            let clean_suggestion = QUOTE_OR_HYPHEN.replace_all(&replacement, "");
            if i > 1 && suggestions.len() > 2 && clean_suggestion.eq_ignore_ascii_case(word) {
                if pos_new_sugg == 0 {
                    pos_new_sugg = 1;
                }
                new_suggestions.insert(pos_new_sugg, cand.clone());
                continue;
            }
            // move "queda'n" to second position, but not anar-se'n
            if i == 1 {
                let first = &suggestions[0].value;
                if !first.contains('-') && (first.ends_with("'n") || first.ends_with("'t")) {
                    new_suggestions.insert(0, cand.clone());
                    continue;
                }
            }
            new_suggestions.push(cand.clone());
        }

        // filter by weight jump
        let mut prev_weight: Option<i32> = None;
        let mut prev_replacement_str = String::new();
        let mut filtered_suggestions: Vec<Cand> = Vec::new();
        for sr in new_suggestions {
            if !equals_ignore_case_and_diacritics(&sr.value, &prev_replacement_str) {
                if let (Some(w), Some(prev)) = (sr.weight, prev_weight) {
                    if w - prev > MAX_WEIGHT_DIFF {
                        break;
                    }
                }
                if let Some(w) = sr.weight {
                    prev_weight = Some(w);
                }
            }
            filtered_suggestions.push(sr.clone());
            prev_replacement_str = sr.value;
        }
        filtered_suggestions
    }
}

/// `MorfologikCatalanSpellerRule.endsWithPronoun`.
fn ends_with_pronoun(s: &str) -> String {
    for pronoun in PRONOMS_DARRERE.iter() {
        if s.ends_with(pronoun) {
            return (*pronoun).to_string();
        }
    }
    String::new()
}

fn weighted_to_cand(s: WeightedSuggestion) -> Cand {
    Cand {
        value: s.word,
        short_description: None,
        weight: Some(s.weight),
    }
}

/// `List.distinct()` over suggestions (`equals` = value + short description).
fn dedupe<T: Clone + PartialEq>(suggestions: Vec<T>) -> Vec<T> {
    let mut seen = Vec::new();
    let mut out = Vec::with_capacity(suggestions.len());
    for s in suggestions {
        if !seen.contains(&s) {
            seen.push(s.clone());
            out.push(s);
        }
    }
    out
}

fn split_last_char(word: &str) -> Option<(&str, &str)> {
    let mut chars = word.char_indices();
    chars
        .next_back()
        .map(|(split, _)| (&word[..split], &word[split..]))
}

/// CachingWordListLoader: skip empty/`#` lines, cut at `#`, trim.
fn cache_word_list(path: &Path) -> Vec<String> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let trimmed = line.trim();
        let cut = trimmed.split('#').next().unwrap_or("").trim();
        if !cut.is_empty() {
            result.push(cut.to_string());
        }
    }
    result
}

/// Plain-text speller dictionary lines. Java passes
/// `getSpellingFileName()` (`/ca/spelling.txt`) plus
/// `getAdditionalSpellingFileNames()` (custom spelling, the global list,
/// `ca/multiwords.txt`, `ca/spelling-special.txt`) through
/// `MorfologikMultiSpeller` and applies `Catalan.prepareLineForSpeller` to
/// every line. Java adds the `LanguageTool` sentinel line only for
/// languages with a variant spelling file (null here).
fn load_plain_text_dict_lines(data_dir: &Path) -> Vec<Vec<u8>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for path in [
        data_dir.join("ca/words/spelling.txt"),
        data_dir.join("ca/hunspell/spelling_custom.txt"),
        data_dir.join("core/spelling_global.txt"),
        data_dir.join("ca/words/multiwords.txt"),
        data_dir.join("ca/words/spelling-special.txt"),
    ] {
        if path.lt_exists() {
            paths.push(path);
        }
    }
    let mut lines: Vec<Vec<u8>> = Vec::new();
    for path in &paths {
        lines.extend(read_speller_lines(path));
    }
    lines
}

/// `CatalanMorfologikMultitokenSpeller.getSpeller`: the
/// `ca-ES_spelling_multitoken.dict` speller (edit distance 1) whose weighted
/// suggestions feed `CatalanMultitokenSpeller.getAdditionalSuggestions`.
pub fn multitoken_speller(data_dir: &Path) -> Option<MorfologikSpeller> {
    let dict = data_dir.join("ca/spelling/ca-ES_spelling_multitoken.dict");
    if !dict.lt_exists() {
        return None;
    }
    MorfologikSpeller::from_dict_file(
        &dict,
        &data_dir.join("ca/spelling/ca-ES_spelling_multitoken.info"),
        1,
    )
    .ok()
}

/// `Catalan.prepareLineForSpeller` (the speller exceptions and the
/// `N*`/`_Latin_` tag filter).
pub fn prepare_line_for_speller(line: &str) -> Vec<String> {
    const SPELLER_EXCEPTIONS: [&str; 16] = [
        "San Juan",
        "Copa América",
        "Colección Jumex",
        "Banco Santander",
        "San Marcos",
        "Santa Ana",
        "San Joaquín",
        "Naguib Mahfouz",
        "Rosalía",
        "Aristide Maillol",
        "Alexia Putellas",
        "Mónica Randall",
        "Vicente Blasco Ibáñez",
        "Copa Sudamericana",
        "Série A",
        "Banco Sabadell",
    ];
    let parts: Vec<&str> = line.split('#').collect();
    if parts.is_empty() {
        return vec![line.to_string()];
    }
    let form_tag: Vec<&str> = parts[0].split(['\t', ';']).collect();
    let form = form_tag[0].trim();
    if SPELLER_EXCEPTIONS.contains(&form) {
        return vec![String::new()];
    }
    if form_tag.len() > 1 {
        let tag = form_tag[1].trim();
        if tag.starts_with('N') || tag == "_Latin_" {
            vec![form.to_string()]
        } else {
            vec![String::new()]
        }
    } else {
        vec![line.to_string()]
    }
}

/// LT `MorfologikMultiSpeller.getLines` (`prepareLineForSpeller` then the
/// comment/`#`-suffix handling).
fn read_speller_lines(path: &Path) -> Vec<Vec<u8>> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for original_line in text.lines() {
        for prepared in prepare_line_for_speller(original_line) {
            if prepared.starts_with('#') || prepared.is_empty() {
                continue;
            }
            let line = prepared.split('#').next().unwrap_or("").trim();
            if !line.is_empty() {
                out.push(line.as_bytes().to_vec());
            }
        }
    }
    out
}
