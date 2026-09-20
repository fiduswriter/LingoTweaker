//! German spelling rule: port of `GermanSpellerRule` /
//! `CompoundAwareHunspellRule` / `HunspellRule` over the vendored morfologik
//! dictionaries (`de_DE.dict`, `de_AT.dict`, `de_CH.dict`).
//!
//! Ported: the rule metadata and messages, the hunspell token loop with the
//! `.aff` `WORDCHARS` re-tokenization and wrong-split heuristics, the
//! ignore/prohibit word lists (incl. the `ExpandingReader` expansion), the
//! German `isMisspelled` overrides (Spielzug/Standart/schaf/…),
//! `ignoreWord(List, idx)` (hanging hyphens, `missingAdjPattern`, ignored
//! compounds, elatives), `ignorePotentiallyMisspelledWord` (the two-/three-
//! part compound heuristics), the curated `ADDITIONAL_SUGGESTIONS`/
//! `getOnlySuggestions` pipeline and the suggestion backend
//! (`MorfologikMultiSpeller` over the binary dictionary + the plain-text
//! speller lists).
//!
//! Remaining deviations (documented in internal development notes):
//! `GermanSpellerRule.getSuggestions`' native-hunspell `suggest` fallback in
//! the "Email…" branch (the hunspell suggestion generator is not ported; the
//! morfologik speller stands in), the language-model-based ranking
//! (`languageModel == null` in this harness), and the multi-word
//! `IGNORE_SPELLING` anti-pattern immunization (`ignored_phrases`).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};
use lt_spell::morfologik::{
    DictSource, MorfologikSpeller, MultiSpeller, SpellerMetadata, WeightedSuggestion,
};
use lt_tagger::DictionaryInfo;
use regex::Regex;

use crate::de::speller_data::{
    SuggestExpr, ADDITIONAL_SUGGESTIONS, ONLY_SUGGESTIONS, PREVENT_SUGGESTION_PATTERNS,
};
use crate::de::spelling_patterns::{
    adjSuffix, missingAdjPattern, ARBEIT_COMP, BACH_COMP, BAD_COMP, CAMEL_CASE, CITIES_EXCEPTIONS,
    COMPOUND_END_TYPOS, COMPOUND_TYPOS, CONFUSED_PREFIXES, DIRECTION,
    ENDS_WITH_IBELKEIT_IBLICHKEIT, FILE_UNDERLINE_PATTERN, GENDER_STAR_PATTERN, INFIX_S_SUFFIXES,
    INVALID_COMP_PART_1, INVALID_COMP_PART_2, LINKS_COMP, LINK_COMP, MENTION_UNDERLINE_PATTERN,
    NEEDS_TO_BE_PLURAL, PERSON_SUFFIXES, RECHTS_COMP, RECHT_COMP, SPECIAL_CASE,
    SPECIAL_CASE_WITH_S, SUBINF_SINGULAR_OBJECT, SUBNOMPLUFEM_EXCEPTIONS, VERBANDS_COMP,
    VERBAND_COMP, WECHSELINFIX, WECHSELNUMERUS, WELTEN_COMP, WIDER_COMP, WOCHENTAGE, WOCHENTAGE_S,
    WOCHENTAG_COMP, WOERTER_COMP,
};
use crate::wordutil::split_compound;
use crate::wordutil::{is_email, is_url};

/// `GermanSpellerRule.MAX_EDIT_DISTANCE`.
const MAX_EDIT_DISTANCE: i32 = 2;
/// `GermanSpellerRule.MIN_WORD_LENGTH` / `MAX_WORD_LENGTH`.
const MIN_WORD_LENGTH: usize = 5;
const MAX_WORD_LENGTH: usize = 40;

/// `CommonFileTypes.getSuffixPattern` (Java regex, full `matches()`).
static COMMON_FILE_TYPES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?:[A-Za-z0-9_áàâóòìíéèùúôîêûäöüß\-.()]*?",
        r".+\.(jpeg|jpg|gif|png|bmp|svg|ai|sketch|ico|ps|psd|tiff|tif|",
        r"mp3|wav|midi|mid|aif|mpa|ogg|wma|wpl|cda|7z|arj|deb|pkg|plist|rar|rpm|tar\.gz|tar|zip|",
        r"bin|dmg|iso|toast|vcd|csv|dat|db|log|mdb|sav|sql|xml|apk|bat|cgi|com|exe|gadget|jar|py|js|jsx|json|wsf|ts|tsx|",
        r"fnt|fon|otf|ttf|woff|woff2|rb|java|php|html|asp|aspx|cer|cfm|pl|css|scss|htm|jsp|part|rss|xhtml|",
        r"key|odp|pps|ppt|pptx|class|cpp|cs|h|sh|swift|vb|ods|odt|xlr|xls|xlsx|xlt|xltx|",
        r"bak|cab|cfg|cpl|cur|dll|dmp|msi|ini|tmp|3g2|3gp|avi|flv|h264|m4v|mkv|mov|mp4|mpg|mpeg|rm|swf|vob|wmv|",
        r"doc|docx|dot|dotx|pdf|rtf|srx|text|tex|wks|wps|wpd|txt|yaml|yml|csl|md|adm|webm|webp))$"
    ))
    .unwrap()
});

/// `GermanSpellerRule.GENDER2_STAR2` without the Java lookbehind: the pattern
/// full-matches iff the word contains `\wIn`, `[*:_]in` or `/-in` (Java `\w`
/// is ASCII-only).
static GENDER2_STAR2: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:[A-Za-z0-9_]In|[*:_]in|/-in)").unwrap());
/// `(?<=(\w))In` (ASCII word char before `In`).
static BINNEN_I_IN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9_]In").unwrap());
/// `GENDER_NEUTRAL_SPECIAL_CHRS_PLU` (full match).
static GENDER_NEUTRAL_PLU: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.*[*:_/]in.+)$").unwrap());
/// `GENDER_NEUTRAL_SLASH_HYPHEN` (full match).
static GENDER_NEUTRAL_SLASH_HYPHEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.*/-in.+)$").unwrap());

/// Java `GENDER_STAR.matcher(word).replaceFirst("in")` (the lookbehind is
/// zero-width: only the `In` is replaced, the preceding word char is kept).
fn gender_star_to_in(word: &str) -> String {
    BINNEN_I_IN
        .replace(word, |caps: &regex::Captures| {
            format!("{}in", &caps[0][..caps[0].len() - 2])
        })
        .into_owned()
}

pub const RULE_ID: &str = "GERMAN_SPELLER_RULE";
pub const AUSTRIAN_RULE_ID: &str = "AUSTRIAN_GERMAN_SPELLER_RULE";
pub const SWISS_RULE_ID: &str = "SWISS_GERMAN_SPELLER_RULE";

/// `MessagesBundle_de` `spelling`
const MESSAGE: &str = "Möglicher Tippfehler gefunden.";
/// `MessagesBundle_de` `desc_spelling` (rule description)
const DESCRIPTION: &str = "Möglicher Rechtschreibfehler";
/// `MessagesBundle_de` `desc_spelling_short`
const SHORT_MESSAGE: &str = "Rechtschreibfehler";
const CATEGORY_ID: &str = "TYPOS";
/// `MessagesBundle_de` `category_typo`
const CATEGORY_NAME: &str = "Mögliche Tippfehler";
/// `SpellingCheckRule.MAX_TOKEN_LENGTH`
const MAX_TOKEN_LENGTH: usize = 200;

/// `GermanSpellerRule.lcDoNotSuggestWords`.
const LC_DO_NOT_SUGGEST: &[&str] = &[
    "verjuden",
    "verjudet",
    "verjudeter",
    "verjudetes",
    "verjudeten",
    "verjudetem",
    "entjuden",
    "entjudet",
    "entjudete",
    "entjudetes",
    "entjudeter",
    "entjudeten",
    "entjudetem",
    "auschwitzmythos",
    "judensippe",
    "judensippen",
    "judensippschaft",
    "judensippschaften",
    "nigger",
    "niggern",
    "niggers",
    "neger",
    "negers",
    "negern",
    "rassejude",
    "rassejuden",
    "rassejüdin",
    "rassejüdinnen",
    "möse",
    "mösen",
    "fotze",
    "fotzen",
    "judenfrei",
    "judenfreie",
    "judenfreier",
    "judenfreies",
    "judenfreien",
    "judenfreiem",
    "judenrein",
    "judenreine",
    "judenreiner",
    "judenreines",
    "judenreinen",
    "judenreinem",
    "judenmord",
    "judenmorden",
    "judenmörder",
];

/// `SpellingCheckRule.pHasNoLetterLatin`: German uses `isLatinScript()` =
/// true, so words without a Latin letter are ignored.
static HAS_NO_LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\p{Latin}]+$").unwrap());
/// `GermanSpellerRule.SPECIAL_CASE_THIRD`
static SPECIAL_CASE_THIRD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[A-ZÖÄÜ][a-zöäüß]{2,}(ei|öl)$").unwrap());
/// `GermanSpellerRule.FIRST_UPPER_CASE`
static FIRST_UPPER_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[A-ZÖÄÜ][a-zöäüß-]+$").unwrap());
static SCHAF_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.{3,}schaf(s|en)?$").unwrap());
static START_WITH_SPIEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("^(Spielzugs?|Spielzugangs?|Spielzuganges|Spielzugbuchs?|Spielzugbüchern?|Spielzuges|Spielzugverluste?|Spielzugverluste[ns])$")
        .unwrap()
});
static END_WITH_SCHAFTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-ZÖÄÜ][a-zöäß-]+schafte$").unwrap());
static ELATIVE_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("^(bitter|dunkel|erz|extra|früh|gemein|grund|hyper|lau|mega|minder|stock|super|tod|ultra|ur|voll)")
        .unwrap()
});
// --- `GermanSpellerRule.getAdditionalTopSuggestionsString` regexes ---
static ALLMAHLLIG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[aA]llmähll?i(g|ch)(e[mnrs]?)?$").unwrap());
static CONTAINS_MAYONNAISE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*[mM]a[jy]onn?[äe]se.*$").unwrap());
static CONTAINS_RESERVIERUNG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*[rR]es(a|er)[vw]i[he]?rung(en)?$").unwrap());
static STARTS_WITH_RESCHASCHIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[rR]eschaschier.+$").unwrap());
static ENDS_WITH_LABORANTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*[lL]aborants$").unwrap());
static PROFESSIONELL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[pP]roff?ess?ion([äe])h?ll?(e[mnrs]?)?$").unwrap());
static VERSTANDNIS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[vV]erstehendniss?(es?)?$").unwrap());
static STARTS_WITH_DIAGNOSZIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^diagno[sz]ier.*$").unwrap());
static ZB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^z[bB]$").unwrap());
static STARTS_WITH_ZB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^z[bB].$").unwrap());
static STARTING_WITH_SINGLE_CHAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\p{L} \p{L}+$").unwrap());
/// Java `\p{Punct}` (ASCII POSIX punctuation).
static HYPHENED_UPPER_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-ZÖÄÜ][a-zöäüß]+-[\-\s]?[a-zöäüß]+$").unwrap());
static HYPHENED_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zöäüß]+-[\-\s][A-ZÖÄÜa-zöäüß]+$").unwrap());
static WORD_WITH_PUNCT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("^[0-9A-Za-z_][!\"#$%&'()*+,./:;<=>?@\\x5b\\x5c\\x5d^_`{|}~-]?$").unwrap()
});

// --- `GermanSpellerRule.getOnlySuggestions` regexes ---
static AUTENTISCH_WITH_CASES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[Aa]utentisch(e[nmsr]?|ste[nmsr]?|ere[nmsr]?)?$").unwrap());
static SYMPHATISCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^symphatisch(e[nmsr]?|ste[nmsr]?|ere[nmsr]?)?$").unwrap());
static BRILLIANT_WITH_CASES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^brilliant(e[nmsr]?|ere[nmsr]?|este[nmsr]?)?$").unwrap());
static RECHTMASIG_WITH_CASES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^rechtmässig(e[nmsr]?|ere[nmsr]?|ste[nmsr]?)?$").unwrap());
static CONTAINS_MASZNAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*[mM]a(ss|ß)namen?.*$").unwrap());
static HOLZ_SPIEGEL_PANEL_COMPOUND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(Holz|Spiegel)panel(s|en?)?$").unwrap());
static SBHAN_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("^SBahn(en|hofs?|zug(e?s)?|zügen?|höfen?|netz(e[ns]?)?|tunnel[sn]?|linien?)?$")
        .unwrap()
});
static UBAHN_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("^UBahn(en|hofs?|zug(e?s)?|zügen?|höfen?|netz(e[ns]?)?|tunnel[sn]?|linien?)?$")
        .unwrap()
});

// --- `GermanSpellerRule.filterNoSuggestWords` regexes ---
static START_WITH_NEGER: LazyLock<Regex> = LazyLock::new(|| Regex::new("^neger.*$").unwrap());
static CONTAINS_NEGER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^.+neger(s|n|in|innen)?.+$").unwrap());
static CONTAINS_NEGER_2: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^.+-neger(s|n|in|innen)?-.+$").unwrap());
static UNCOMMON_LOWERCASED_NOUN_AT_BEGINNING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("^(hunger|zeit|käse|zwiebel|kommoden?|lager|angst|freund|feind)\\s.+$").unwrap()
});
static UNCOMMON_LOWERCASED_NOUN_AT_END: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("^.+\\s(hunger|zeit|käse|zwiebel|kommoden?|lager|angst|freund|feind)$").unwrap()
});
static SUB_VER_INF: LazyLock<Regex> = LazyLock::new(|| Regex::new("^SUB:.*:INF$").unwrap());

/// Compiled `speller_data::ADDITIONAL_SUGGESTIONS` (Java compiles all
/// patterns at class init; the order is the insertion order).
static COMPILED_ADDITIONAL_SUGGESTIONS: LazyLock<
    Vec<(Regex, &'static [crate::de::speller_data::SuggestExpr])>,
> = LazyLock::new(|| {
    ADDITIONAL_SUGGESTIONS
        .iter()
        .map(|(pattern, exprs)| {
            let re = Regex::new(&format!("^(?:{pattern})$"))
                .unwrap_or_else(|e| panic!("invalid curated speller pattern {pattern:?}: {e}"));
            (re, *exprs)
        })
        .collect()
});

static COMPILED_PREVENT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    PREVENT_SUGGESTION_PATTERNS
        .iter()
        .map(|pattern| {
            Regex::new(&format!("^(?:{pattern})$"))
                .unwrap_or_else(|e| panic!("invalid prevent-suggestion pattern {pattern:?}: {e}"))
        })
        .collect()
});

/// Java regex replacement cache for the curated `replaceFirst` expressions.
static REPLACEMENT_CACHE: LazyLock<Mutex<std::collections::HashMap<&'static str, Regex>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Java `String.replaceFirst(pattern, replacement)` (first match, `$1`
/// group references).
fn re_first(word: &str, pattern: &'static str, replacement: &str) -> String {
    let re = {
        let mut cache = REPLACEMENT_CACHE.lock().unwrap();
        cache
            .entry(pattern)
            .or_insert_with(|| {
                Regex::new(pattern).unwrap_or_else(|e| {
                    panic!("invalid speller replacement pattern {pattern:?}: {e}")
                })
            })
            .clone()
    };
    re.replace(word, replacement).into_owned()
}

/// `HunspellRule.addIgnoreWords` (`Resources.readLines`): all lines except
/// `#` comments, not trimmed and with tags kept.
fn raw_word_list_lines(path: &Path) -> Vec<String> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// `CachingWordListLoader.loadWords`: skip empty/`#` lines, then
/// `StringUtils.substringBefore(line.trim(), "#").trim()`.
fn read_word_list(path: &Path) -> Vec<String> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let line = line.trim();
            match line.find('#') {
                Some(idx) => line[..idx].trim().to_string(),
                None => line.to_string(),
            }
        })
        .filter(|line| !line.is_empty())
        .collect()
}

/// `GermanSpellerRule.getSpeller` → `MorfologikMultiSpeller`'s plain-text
/// lines: the `ExpandingReader`-expanded `spelling.txt`,
/// `spelling_custom.txt`, `multitoken-suggest.txt`, `spelling_global.txt`
/// and `spelling_recommendation.txt` streams (plus the variant file for
/// de-AT/de-CH), each line filtered like `MorfologikMultiSpeller.getLines`
/// (skip `#`/empty, cut at the first `#`, trim). Java adds the
/// `LanguageTool` sentinel when a variant reader exists.
///
/// Vendored-path mapping: Java's `/de/multitoken-suggest.txt` lives in
/// `data/de/words/` and `spelling_global.txt` in `data/core/` (the lt-sync
/// layout used by every other German loader).
fn plain_text_speller_lines(
    data_dir: &Path,
    variant: &str,
    expander: &crate::de::line_expander::LineExpander,
) -> Result<Vec<Vec<u8>>> {
    let hunspell_dir = data_dir.join("de/hunspell");
    let mut paths = vec![
        hunspell_dir.join("spelling.txt"),
        hunspell_dir.join("spelling_custom.txt"),
        data_dir.join("de/words/multitoken-suggest.txt"),
        data_dir.join("core/spelling_global.txt"),
        hunspell_dir.join("spelling_recommendation.txt"),
    ];
    match variant {
        "de-AT" => paths.push(hunspell_dir.join("spelling-de-AT.txt")),
        "de-CH" => paths.push(hunspell_dir.join("spelling-de-CH.txt")),
        _ => {}
    }
    let mut lines: Vec<Vec<u8>> = Vec::new();
    for path in &paths {
        for raw in lt_data::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
        {
            for expanded in expander.expand_line(raw) {
                if expanded.starts_with('#') || expanded.is_empty() {
                    continue;
                }
                let line = match expanded.find('#') {
                    Some(idx) => expanded[..idx].trim(),
                    None => expanded.trim(),
                };
                if !line.is_empty() {
                    lines.push(line.as_bytes().to_vec());
                }
            }
        }
    }
    if variant != "de-DE" {
        lines.push(lt_spell::morfologik::LANGUAGETOOL.as_bytes().to_vec());
    }
    Ok(lines)
}

pub struct GermanSpellingRule {
    rule_id: String,
    /// native-hunspell-equivalent acceptance (`HunspellRule.isMisspelled`)
    hunspell: lt_spell::hunspell::HunspellChecker,
    /// `MorfologikMultiSpeller` over the binary `.dict` + the plain-text
    /// spellers (suggestion generation only; misspellings are decided by
    /// native hunspell, like Java)
    morfo: MultiSpeller,
    /// sorted + deduplicated (`startsWithIgnoredWord` binary-searches it)
    ignore: Vec<String>,
    prohibit: HashSet<String>,
    /// `GermanSpellerRule.wordStartsToBeProhibited` (`prohibit.txt` `X.*`)
    word_starts_to_be_prohibited: HashSet<String>,
    /// `GermanSpellerRule.wordEndingsToBeProhibited` (`prohibit.txt` `.*X`)
    word_endings_to_be_prohibited: HashSet<String>,
    /// `GermanSpellerRule.wordsToBeIgnoredInCompounds` (ignore file `X-*`)
    words_to_be_ignored_in_compounds: HashSet<String>,
    /// IGNORE_SPELLING anti-pattern phrases from multi-word ignore entries,
    /// indexed by their first token (`SpellingCheckRule.addIgnoreWords`).
    ignored_phrases: HashMap<String, Vec<Vec<String>>>,
    /// `GermanSpellerRule.wordsNeedingInfixS` (`words_infix_s.txt`)
    words_needing_infix_s: HashSet<String>,
    /// `GermanSpellerRule.verbStems` (`verb_stems.txt`)
    verb_stems: HashSet<String>,
    /// `GermanSpellerRule.verbPrefixes` (`verb_prefixes.txt`)
    verb_prefixes: HashSet<String>,
    /// `GermanSpellerRule.otherPrefixes` (`other_prefixes.txt`)
    other_prefixes: HashSet<String>,
    /// `GermanSpellerRule.oldSpelling` (`alt_neu.csv`)
    old_spelling: HashSet<String>,
    /// strict compound tokenizer (`language.getStrictCompoundTokenizer()`)
    compound: Arc<lt_tokenize::GermanCompoundTokenizer>,
    /// non-strict (jWordSplitter) compound tokenizer
    /// (`GermanCompoundTokenizer.getNonStrictInstance()`)
    non_strict_compound: Arc<lt_tokenize::GermanCompoundTokenizer>,
    /// `German`'s tagger (`getTagger()`, used by the suggestion quality sort
    /// and `getFilteredSuggestions`)
    tagger: Arc<lt_tagger::GermanTagger>,
    /// `GermanSynthesizer` (`GermanSpellerRule`'s suggestion helpers)
    synth: Arc<lt_tagger::GermanSynthesizer>,
    /// de-CH: suggestions get `ß` → `ss` (`filterForLanguage`)
    swiss: bool,
    /// `de-DE`/`de-AT`/`de-CH` (variant-dependent suggestions)
    variant: String,
    suggestion_cache: Mutex<std::collections::HashMap<String, Arc<Vec<Suggestion>>>>,
}

impl GermanSpellingRule {
    /// Load the speller for `variant` (`de-DE`/`de-AT`/`de-CH`). `synth` is
    /// the German synthesizer, needed by the `LineExpander` for the
    /// `prefix_verb` lines of the word lists.
    pub fn load(
        data_dir: &Path,
        variant: &str,
        tagger: Arc<lt_tagger::GermanTagger>,
        synth: Arc<lt_tagger::GermanSynthesizer>,
    ) -> Result<Self> {
        let (rule_id, dict_name) = match variant {
            "de-AT" => (AUSTRIAN_RULE_ID, "de_AT"),
            "de-CH" => (SWISS_RULE_ID, "de_CH"),
            _ => (RULE_ID, "de_DE"),
        };
        let hunspell_dir = data_dir.join("de/hunspell");
        let hunspell = lt_spell::hunspell::HunspellChecker::load(
            &hunspell_dir.join(format!("{dict_name}.aff")),
            &hunspell_dir.join(format!("{dict_name}.dic")),
        )?;
        let swiss = variant == "de-CH";
        // `GermanSpellerRule.getSpeller` → `MorfologikMultiSpeller` over the
        // binary `.dict` and the plain-text speller lists. The metadata of
        // the binary dictionary is shared by both spellers in Java (the
        // plain-text FSA is built with the same `.info` file).
        let info_path = hunspell_dir.join(format!("{dict_name}.info"));
        let info_text = lt_data::fs::read_to_string(&info_path).map_err(|e| {
            lt_core::CoreError::Data(format!("cannot read {}: {e}", info_path.display()))
        })?;
        let info = DictionaryInfo::parse(&info_text)?;
        let meta = SpellerMetadata::from_info(&info)?;
        let binary_source = Arc::new(DictSource::from_dict_file(
            &hunspell_dir.join(format!("{dict_name}.dict")),
            &info_path,
        )?);
        let binary = MorfologikSpeller::from_source(
            Arc::clone(&binary_source),
            meta.clone(),
            MAX_EDIT_DISTANCE,
        );
        // `SpellingCheckRule.init` + `GermanSpellerRule.addIgnoreWords` /
        // `expandLine` (`LineExpander`): ignore.txt + spelling.txt +
        // spelling_custom.txt + spelling_global.txt (+ the variant file).
        let expander = crate::de::line_expander::LineExpander::new(Arc::clone(&synth));
        let plain_lines = plain_text_speller_lines(data_dir, variant, &expander)?;
        let plain = MorfologikSpeller::from_lines(plain_lines, meta.clone(), MAX_EDIT_DISTANCE);
        let morfo = MultiSpeller::new(vec![binary, plain], vec![0, 1]);
        let mut ignore: Vec<String> = Vec::new();
        let mut words_to_be_ignored_in_compounds: HashSet<String> = HashSet::new();
        let mut ignored_phrases: Vec<Vec<String>> = Vec::new();
        let mut spelling_files = vec![
            hunspell_dir.join("ignore.txt"),
            hunspell_dir.join("spelling.txt"),
            hunspell_dir.join("spelling_custom.txt"),
            data_dir.join("core/spelling_global.txt"),
        ];
        match variant {
            "de-AT" => spelling_files.push(hunspell_dir.join("spelling-de-AT.txt")),
            "de-CH" => spelling_files.push(hunspell_dir.join("spelling-de-CH.txt")),
            _ => {}
        }
        for path in spelling_files {
            for raw in read_word_list(&path) {
                // hack: Swiss German doesn't use "ß", see
                // `GermanSpellerRule.addIgnoreWords`
                let line = if swiss { raw.replace('ß', "ss") } else { raw };
                if line.ends_with("-*") {
                    words_to_be_ignored_in_compounds.insert(line[..line.len() - 2].to_string());
                    continue;
                }
                for word in expander.expand_line(&line) {
                    // `tokenizeNewWords()` is true for German: multi-word
                    // entries become IGNORE_SPELLING anti-patterns instead of
                    // single ignore words.
                    let tokens: Vec<String> = lt_tokenize::GermanWordTokenizer::new()
                        .tokenize(&word)
                        .into_iter()
                        .filter(|t| !t.trim().is_empty())
                        .collect();
                    if tokens.len() > 1 {
                        ignored_phrases.push(tokens);
                    } else {
                        ignore.push(word);
                    }
                }
            }
        }
        // `HunspellRule.addIgnoreWords()`: the raw ignore.txt lines are added
        // on top of the expanded ones (only `#` comment lines are filtered).
        for line in raw_word_list_lines(&hunspell_dir.join("ignore.txt")) {
            ignore.push(line);
        }
        ignore.sort_unstable();
        ignore.dedup();
        ignore.shrink_to_fit();
        let mut prohibit: HashSet<String> = HashSet::new();
        let mut word_starts_to_be_prohibited: HashSet<String> = HashSet::new();
        let mut word_endings_to_be_prohibited: HashSet<String> = HashSet::new();
        for path in [
            hunspell_dir.join("prohibit.txt"),
            hunspell_dir.join("prohibit_custom.txt"),
        ] {
            for raw in read_word_list(&path) {
                let expanded = expander.expand_line(&raw);
                if expanded.len() == 1 && expanded[0].ends_with(".*") {
                    let w = &expanded[0];
                    word_starts_to_be_prohibited.insert(w[..w.len() - 2].to_string());
                } else if expanded.first().is_some_and(|w| w.starts_with(".*")) {
                    for w in &expanded {
                        word_endings_to_be_prohibited.insert(w[2..].to_string());
                    }
                } else {
                    prohibit.extend(expanded);
                }
            }
        }
        let words_dir = data_dir.join("de/words");
        let compound = Arc::new(lt_tokenize::GermanCompoundTokenizer::load(
            &data_dir.join("de/compound/wordsGerman.txt"),
            &data_dir.join("de/compound/exceptionsGerman.txt"),
            true,
        )?);
        let non_strict_compound = Arc::new(lt_tokenize::GermanCompoundTokenizer::load(
            &data_dir.join("de/compound/wordsGerman.txt"),
            &data_dir.join("de/compound/exceptionsGerman.txt"),
            false,
        )?);
        // `GermanSpellerRule` constructor: the `loadFile` word sets (first
        // `;`-separated field per line, `#` lines skipped).
        let load_set = |name: &str| -> HashSet<String> {
            let mut set = HashSet::new();
            if let Ok(text) = lt_data::fs::read_to_string(words_dir.join(name)) {
                for line in text.lines() {
                    if !line.starts_with('#') {
                        set.insert(line.trim().split(';').next().unwrap_or("").to_string());
                    }
                }
            }
            set
        };
        let words_needing_infix_s = load_set("words_infix_s.txt");
        let verb_stems = load_set("verb_stems.txt");
        let verb_prefixes = load_set("verb_prefixes.txt");
        let other_prefixes = load_set("other_prefixes.txt");
        let old_spelling = load_set("alt_neu.csv");
        Ok(Self {
            rule_id: rule_id.to_string(),
            hunspell,
            morfo,
            ignore,
            prohibit,
            word_starts_to_be_prohibited,
            word_endings_to_be_prohibited,
            words_to_be_ignored_in_compounds,
            ignored_phrases: {
                let mut by_first: HashMap<String, Vec<Vec<String>>> = HashMap::new();
                for phrase in ignored_phrases {
                    if let Some(first) = phrase.first() {
                        by_first.entry(first.clone()).or_default().push(phrase);
                    }
                }
                by_first
            },
            words_needing_infix_s,
            verb_stems,
            verb_prefixes,
            other_prefixes,
            old_spelling,
            compound,
            non_strict_compound,
            tagger,
            synth,
            swiss,
            variant: variant.to_string(),
            suggestion_cache: Mutex::new(std::collections::HashMap::new()),
        })
    }

    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    /// `GermanSpellerRule.isIgnoredNoCase` (case-sensitive, plus the
    /// uppercase-sentence-start form of lowercase entries, plus the
    /// `ignoreWordsWithLength = 1` single-character rule).
    fn ignore_contains(&self, word: &str) -> bool {
        self.ignore
            .binary_search_by(|p| p.as_str().cmp(word))
            .is_ok()
    }

    fn is_ignored_no_case(&self, word: &str) -> bool {
        self.ignore_contains(word)
            || (FIRST_UPPER_CASE.is_match(word) && self.ignore_contains(&word.to_lowercase()))
            || word.chars().count() <= 1
    }

    /// `GermanSpellerRule.isProhibited` incl. the `wordStartsToBeProhibited`
    /// / `wordEndingsToBeProhibited` patterns.
    fn is_prohibited(&self, word: &str) -> bool {
        self.prohibit.contains(word)
            || self
                .word_starts_to_be_prohibited
                .iter()
                .any(|w| word.starts_with(w))
            || self
                .word_endings_to_be_prohibited
                .iter()
                .any(|w| word.ends_with(w))
    }

    /// `GermanSpellerRule.ignoreElative`.
    fn ignore_elative(&self, word: &str) -> bool {
        if let Some(caps) = ELATIVE_PREFIX.find(word) {
            let last_part = &word[caps.end()..];
            return last_part.chars().count() >= 3 && !self.is_misspelled(last_part);
        }
        false
    }

    /// `SpellingCheckRule.startsWithIgnoredWord(word, true)`: length of the
    /// longest ignored word that is a prefix of `word` (0 = none), via the
    /// same binary-search/shrink loop as Java.
    fn starts_with_ignored_word(&self, word: &str) -> usize {
        if word.chars().count() < 4 {
            return 0;
        }
        let sorted = &self.ignore;
        let mut current = word.to_string();
        while !current.is_empty() {
            match sorted.binary_search(&current) {
                Ok(_) => break,
                Err(0) => return 0,
                Err(insert) => {
                    let prev = &sorted[insert - 1];
                    current = common_prefix(&current, prev);
                }
            }
        }
        current.chars().count()
    }

    /// `GermanSpellerRule.ignoreCompoundWithIgnoredWord`: a compound that
    /// contains an ignored word from `spelling.txt` is accepted when its
    /// other parts are spelled correctly.
    fn ignore_compound_with_ignored_word(&self, word: &str) -> bool {
        if !starts_with_uppercase(word) && !starts_with_any(word, &["nord", "west", "ost", "süd"])
        {
            // otherwise stuff like "rumfangreichen" gets accepted
            return false;
        }
        let words = java_split_hyphen(word);
        if words.len() < 2 {
            // non-hyphenated compound (e.g., "Feynmandiagramm"): only search
            // for compounds that start(!) with a word from spelling.txt
            let mut end = self.starts_with_ignored_word(word);
            if end < 3 {
                // support for geographical adjectives
                if word.starts_with("ost") || word.starts_with("süd") {
                    end = 3;
                } else if word.starts_with("west") || word.starts_with("nord") {
                    end = 4;
                } else {
                    return false;
                }
            }
            let ignored_word = word.chars().take(end).collect::<String>();
            let mut partial_word = word.chars().skip(end).collect::<String>();
            if partial_word.ends_with('.') {
                partial_word.pop();
            }
            let is_noun = self.is_noun(&partial_word);
            let mut is_uppercase_noun = false;
            if !is_noun && !starts_with_uppercase(&partial_word) {
                is_uppercase_noun = self.is_noun(&lt_tagger::uppercase_first_char(&partial_word));
            }
            let is_direction = DIRECTION.is_match(&ignored_word);
            let is_adjective = self.is_adjective(&ignored_word);
            let is_directional_adjective =
                is_direction && (is_adjective || PARTIAL_ADJ_ISCH.is_match(&partial_word));
            let is_candidate = (is_directional_adjective || is_noun || is_uppercase_noun)
                && !lt_tagger::is_all_uppercase(&ignored_word)
                && (is_all_lowercase(&partial_word) || ignored_word.ends_with('-'));
            let need_fugen_s = is_needing_fugen_s(&ignored_word);
            if is_candidate && !need_fugen_s && partial_word.chars().count() > 2 {
                return self.hunspell.spell(&partial_word)
                    || self
                        .hunspell
                        .spell(&lt_tagger::uppercase_first_char(&partial_word));
            } else if is_candidate && need_fugen_s && partial_word.chars().count() > 2 {
                if partial_word.starts_with('s') {
                    partial_word = partial_word[1..].to_string();
                }
                return self.hunspell.spell(&partial_word)
                    || self
                        .hunspell
                        .spell(&lt_tagger::uppercase_first_char(&partial_word));
            }
            return false;
        }
        // hyphenated compound (e.g., "Feynman-Diagramm")
        let mut has_ignored_word = false;
        let mut to_spell_check: Vec<String> = Vec::new();
        let first_len = words[0].chars().count();
        let last_len = words[words.len() - 1].chars().count();
        let strip_first: String = word.chars().skip(first_len + 1).collect();
        let strip_last: String = word
            .chars()
            .take(word.chars().count() - last_len - 1)
            .collect();
        if self.ignore_word_base(&strip_first)
            || self.words_to_be_ignored_in_compounds.contains(&strip_first)
        {
            // e.g., "Senioren-Au-pair"
            has_ignored_word = true;
            if !self.ignore_word_base(&words[0]) {
                to_spell_check.push(words[0].clone());
            }
        } else if self.ignore_word_base(&strip_last)
            || self.words_to_be_ignored_in_compounds.contains(&strip_last)
        {
            // e.g., "Au-pair-Agentur"
            has_ignored_word = true;
            if !self.ignore_word_base(&words[words.len() - 1]) {
                to_spell_check.push(words[words.len() - 1].clone());
            }
        } else {
            for word1 in &words {
                if self.ignore_word_base(word1)
                    || self.words_to_be_ignored_in_compounds.contains(word1)
                {
                    has_ignored_word = true;
                } else {
                    to_spell_check.push(word1.clone());
                }
            }
        }
        if has_ignored_word {
            for w in &to_spell_check {
                if !self.hunspell.spell(w) {
                    return false;
                }
            }
        }
        has_ignored_word
    }

    /// `SpellingCheckRule.ignoreWord(String)`: the base ignore logic that
    /// `HunspellRule.isMisspelled` uses (no German token-loop heuristics).
    fn ignore_word_base(&self, word: &str) -> bool {
        if word.chars().count() > MAX_TOKEN_LENGTH {
            return true;
        }
        if HAS_NO_LETTER.is_match(word) {
            return true;
        }
        if let Some(stripped) = word.strip_suffix('.') {
            if !self.ignore_contains(word) {
                self.is_ignored_no_case(stripped)
            } else {
                self.is_ignored_no_case(word)
            }
        } else {
            self.is_ignored_no_case(word)
        }
    }

    /// `GermanSpellerRule.ignoreWord(List<String>, int)` (the token-loop
    /// heuristics: uncapitalized sentence start, bullet lists, hanging
    /// hyphens, file suffixes, `mitarbeitende`, `.mp3`/`sat`, compounds with
    /// ignored parts, elative prefixes). The `missingAdjPattern` block is
    /// not ported yet (documented in D-033).
    fn german_ignore_word(&self, words: &[String], idx: usize) -> bool {
        let word = words[idx].as_str();
        if word.chars().count() > MAX_TOKEN_LENGTH {
            return true;
        }
        let ignore = self.ignore_word_base(word);
        let ignore_uncapitalized_word = !ignore
            && idx == 0
            && self.ignore_word_base(&lt_tagger::lowercase_first_char(&words[0]));
        let mut ignore_bullet_point_case = false;
        if !ignore_uncapitalized_word {
            ignore_bullet_point_case = !ignore
                && idx == 1
                && words[0].is_empty()
                && lt_tagger::is_capitalized_word(word)
                && self.is_misspelled(word)
                && !self.is_misspelled(&word.to_lowercase());
        }
        let mut ignore_by_hyphen = false;
        let mut ignore_hyphenated_compound = false;
        if !ignore && !ignore_uncapitalized_word {
            if word.contains('-') {
                if idx > 0
                    && words[idx - 1].is_empty()
                    && (word.starts_with("stel-") || word.starts_with("tel-"))
                {
                    // accept compounds such as '100stel-Millimeter'
                    let after = word.split_once('-').map(|(_, r)| r).unwrap_or("");
                    return !self.is_misspelled(after);
                }
                ignore_by_hyphen = word.ends_with('-') && self.ignore_by_hanging_hyphen(words, idx);
            }
            ignore_hyphenated_compound =
                !ignore_by_hyphen && self.ignore_compound_with_ignored_word(word);
        }
        if COMMON_FILE_TYPES.is_match(word) {
            return true;
        }
        if missingAdjPattern.is_match(word) {
            let first_part = lt_tagger::uppercase_first_char(&ADJ_SUFFIX_PATTERN.replace(word, ""));
            // We append "test" to see if the word plus "test" is accepted as
            // a compound. This way, the infix 's' is handled properly.
            if self.is_misspelled(word) {
                let plain_first_part = !self.is_misspelled(&first_part)
                    && !SPECIAL_CASE.is_match(&first_part)
                    && self.is_only_noun(&first_part);
                let infix_s_first_part = first_part.ends_with('s')
                    && !self.is_misspelled(&first_part[..first_part.len() - 1])
                    && SPECIAL_CASE_WITH_S.is_match(&first_part)
                    && self.is_only_noun(&first_part[..first_part.len() - 1]);
                if (plain_first_part || infix_s_first_part)
                    && !self.is_misspelled(&format!("{first_part}test"))
                {
                    return true;
                }
            }
        }
        if word.ends_with("mitarbeitende") || word.ends_with("mitarbeitenden") {
            let replaced = word.replace("mitarbeitenden", "mitarbeiter");
            let replaced = replaced.replace("mitarbeitende", "mitarbeiter");
            if self.hunspell.spell(&replaced) {
                return true;
            }
        }
        if (idx + 1 < words.len()
            && (word.ends_with(".mp") || word.ends_with(".woff"))
            && words[idx + 1].is_empty())
            || (idx > 0
                && words[idx - 1].is_empty()
                && ["sat", "stel", "tel", "stels", "tels"].contains(&word))
        {
            return true;
        }
        ignore
            || ignore_uncapitalized_word
            || ignore_bullet_point_case
            || ignore_by_hyphen
            || ignore_hyphenated_compound
            || self.ignore_elative(word)
    }

    /// `GermanSpellerRule.ignoreByHangingHyphen`.
    fn ignore_by_hanging_hyphen(&self, words: &[String], idx: usize) -> bool {
        let word = words[idx].as_str();
        let next = get_word_after_enumeration_or_null(words, idx + 1);
        let next = next.map(|w| w.strip_suffix('.').unwrap_or(w));
        let is_compound = next.is_some_and(|next| {
            self.compound.tokenize(next).len() > 1
                || next.find('-').is_some_and(|i| i > 0)
                || SPECIAL_CASE_THIRD.is_match(next)
        });
        if !is_compound {
            return false;
        }
        let stem = word.strip_suffix('-').unwrap_or(word);
        let mut is_misspelled = !self.hunspell.spell(stem);
        if is_misspelled
            && (self.ignore_word_base(stem) || self.words_to_be_ignored_in_compounds.contains(stem))
        {
            is_misspelled = false;
        } else if is_misspelled
            && stem.ends_with('s')
            && is_needing_fugen_s(&stem[..stem.len() - 1])
        {
            is_misspelled = !self.hunspell.spell(&stem[..stem.len() - 1]);
        }
        !is_misspelled
    }

    /// `GermanSpellerRule.ignorePotentiallyMisspelledWord`: cheap compound
    /// heuristics that suppress false-positive misspellings after the
    /// dictionary check but before a match is created. Returns true when the
    /// word should be ignored.
    fn ignore_potentially_misspelled_word(&self, word: &str) -> bool {
        let word_no_dot = word.strip_suffix('.').unwrap_or(word);
        if self.is_valid_word_length(word)
            || starts_with_lowercase(word)
            || self.is_prohibited(word)
            || self.is_prohibited(word_no_dot)
        {
            // exclude weird/irrelevant words and very long words
            return false;
        }
        if self.is_probably_typo(word) {
            return false;
        }
        // format gender-neutral forms here to make splitting easier, but
        // double-check the word later (camel case words can look similar)
        let word_no_dot_org = word_no_dot;
        let word_no_dot = gender_star_to_in(word_no_dot);
        if !self.is_valid_camel_case(&word_no_dot) {
            return false;
        }
        let mut parts = self.compound.tokenize(&word_no_dot);
        if parts.len() == 1 {
            parts = self.non_strict_compound.tokenize(&word_no_dot);
        }
        parts = avoid_infix_s_as_single_token(parts);
        if word_no_dot.contains('-') {
            let split_by_hyphen = java_split_hyphen(&word_no_dot);
            let last_part = &split_by_hyphen[split_by_hyphen.len() - 1];
            if !self.is_noun(last_part) && self.is_noun(&lt_tagger::uppercase_first_char(last_part))
            {
                // make sure that the last part is uppercase if it probably
                // is a noun, e.g. "Implementierungs-pflicht"
                return false;
            }
            for w in &split_by_hyphen {
                if self.is_misspelled(w) && self.is_misspelled(&remove_trailing_s_and_hyphen(w)) {
                    return false;
                }
            }
            // split tokenized parts that contain hyphens and restore the
            // hyphens removed by the tokenizer
            parts = split_parts_by_hyphen(&parts);
            parts = restore_removed_hyphens(&parts, &word_no_dot);
        }
        // short parts can also be typos
        if !is_valid_part_length(&parts) {
            return false;
        }
        if parts.len() >= 2 && is_old_spelling(&parts, &self.old_spelling) {
            return false;
        }
        if GENDER2_STAR2.is_match(word_no_dot_org)
            && !self.is_valid_gender_neutral_word(&parts, word_no_dot_org)
        {
            return false;
        }
        if parts.len() == 2 {
            self.process_two_part_compounds(&parts[0], &parts[1])
        } else if parts.len() == 3 {
            self.process_three_part_compound(&parts)
        } else {
            // more than three parts can be supported later
            false
        }
    }

    /// `GermanSpellerRule.isValidWordLength`.
    fn is_valid_word_length(&self, word: &str) -> bool {
        let len = word.chars().count();
        len <= MIN_WORD_LENGTH || len >= MAX_WORD_LENGTH
    }

    /// `GermanSpellerRule.isProbablyTypo`.
    fn is_probably_typo(&self, word: &str) -> bool {
        COMPOUND_TYPOS.is_match(word) || COMPOUND_END_TYPOS.is_match(word)
    }

    /// `GermanSpellerRule.isValidCamelCase`.
    fn is_valid_camel_case(&self, input: &str) -> bool {
        !CAMEL_CASE.is_match(input)
    }

    /// `GermanSpellerRule.isValidGenderNeutralWord`.
    fn is_valid_gender_neutral_word(&self, parts: &[String], word: &str) -> bool {
        let chars: Vec<char> = word.chars().collect();
        let mut start = 0usize;
        for part in parts {
            let mut end = start + part.chars().count();
            let mut to_check: String = chars[start..end.min(chars.len())].iter().collect();
            if to_check.starts_with('I') && start > 0 {
                // e.g. AktienIndex
                return false;
            }
            if BINNEN_I_IN.is_match(&to_check)
                && (self.is_misspelled(&gender_star_to_in(&to_check))
                    || (!to_check.ends_with("In") && !to_check.ends_with("Innen")))
            {
                return false;
            }
            if GENDER_NEUTRAL_SIN.is_match(&to_check) {
                if self.is_misspelled(&REPLACE_GENDER_IN.replace(&to_check, "in")) {
                    return false;
                }
                end += 1;
            }
            if GENDER_NEUTRAL_PLU.is_match(&to_check) {
                if end < chars.len() {
                    end += 1;
                    to_check = chars[start..end].iter().collect();
                }
                if self.is_misspelled(&REPLACE_GENDER_IN.replace(&to_check, "in"))
                    || (!to_check.ends_with("in") && !to_check.ends_with("innen"))
                {
                    return false;
                }
            }
            if GENDER_NEUTRAL_SLASH_HYPHEN.is_match(&to_check) {
                if end + 1 < chars.len() {
                    end += 2;
                    to_check = chars[start..end].iter().collect();
                }
                if self.is_misspelled(&to_check.replacen("/-in", "in", 1))
                    || (!to_check.ends_with("in") && !to_check.ends_with("innen"))
                {
                    return false;
                }
            }
            start = end;
        }
        true
    }

    /// `GermanSpellerRule.processTwoPartCompounds`.
    fn process_two_part_compounds(&self, part1: &str, part2: &str) -> bool {
        let part1_upcased = lt_tagger::uppercase_first_char(part1);
        let part2_upcased = lt_tagger::uppercase_first_char(part2);
        let part1_without_hyphen = remove_trailing_hyphen(part1);
        let part2_upcased_is_noun = self.is_noun(&part2_upcased);
        let part2_upcased_is_misspelled = self.is_misspelled(&part2_upcased);

        if INVALID_COMP_PART_1.is_match(&lt_tagger::lowercase_first_char(&part1_without_hyphen)) {
            return false;
        }
        if INVALID_COMP_PART_2.is_match(&lt_tagger::lowercase_first_char(part2)) {
            return false;
        }

        let mut part1_without_infix_s = part1_upcased.clone();
        // sometimes part1 requires singular or plural
        let mut part1_lemma = self.find_lemma_for_noun(&remove_trailing_hyphen(part1));
        if part1_lemma.is_empty() && remove_trailing_hyphen(part1).ends_with('s') {
            part1_lemma = self.find_lemma_for_noun(&remove_trailing_s_and_hyphen(part1));
            part1_without_infix_s = remove_trailing_s(&part1_upcased);
        }

        // allow part1 to be a plural noun if...
        if self.is_noun_nom_plu(&part1_without_infix_s)
            && !self.is_noun_nom_sin(&part1_without_infix_s)
            && !self.is_country_or_region_nom_sin(&part1_without_infix_s)
            && (!self.is_sub_ver_inf(&part2_upcased)
                || (self.is_sub_ver_inf(&part2_upcased)
                    && SUBINF_SINGULAR_OBJECT.is_match(&lt_tagger::lowercase_first_char(part2))))
            && !self.needs_to_be_plural(&lt_tagger::lowercase_first_char(&part1_lemma))
            && !WECHSELNUMERUS.is_match(&lt_tagger::lowercase_first_char(&part1_lemma))
            && !WECHSELINFIX.is_match(&lt_tagger::lowercase_first_char(&remove_trailing_s(
                &part1_lemma,
            )))
        {
            return false;
        }
        // ... part1 always needs to be plural or...
        if self.needs_to_be_plural(&lt_tagger::lowercase_first_char(&part1_lemma))
            && self.is_noun_nom_sin(&part1_without_infix_s)
        {
            return false;
        }
        // ... part1
        if WECHSELNUMERUS.is_match(&lt_tagger::lowercase_first_char(&part1_lemma))
            && !self.check_plural_for_part1_part2_combination(part1, part2)
        {
            return false;
        }

        // for some part1/part2 combinations an infix s is correct or not
        if WECHSELINFIX.is_match(&lt_tagger::lowercase_first_char(part1)) {
            return self.check_infix_s_for_part1_part2_combination(part1, part2);
        }

        // easily confused prefixes (e.g. wieder vs wider)
        if CONFUSED_PREFIXES.is_match(&lt_tagger::lowercase_first_char(part1))
            && !part2_upcased_is_misspelled
            && part2_upcased_is_noun
        {
            return self.check_confusion_for_part1_part2_combination(part1, part2);
        }

        if part2_upcased_is_noun
            && !part2_upcased_is_misspelled
            && part1_without_hyphen.ends_with('s')
            && (self.is_noun_nom(&part1_upcased) || self.is_verb_stem(part1))
            && !self.needs_infix_s(&remove_trailing_s(&part1_upcased))
        {
            return true;
        }
        if part2_upcased_is_noun
            && !part2_upcased_is_misspelled
            && part1_without_hyphen.ends_with('s')
            && self.is_noun_nom(&remove_trailing_s_and_hyphen(&part1_upcased))
            && self.needs_infix_s(&remove_trailing_s_and_hyphen(&part1_upcased))
        {
            return true;
        }
        if part2_upcased_is_noun
            && !part2_upcased_is_misspelled
            && !part1_without_hyphen.ends_with('s')
            && (self.is_noun_nom(&part1_upcased) || self.is_verb_stem(part1))
            && !self.needs_infix_s(&part1_upcased)
        {
            return true;
        }
        if part2_upcased_is_noun
            && !part2_upcased_is_misspelled
            && ((lt_tagger::is_all_uppercase(&remove_trailing_s_and_hyphen(part1))
                && !self.is_misspelled(&remove_trailing_s_and_hyphen(part1)))
                || self.is_other_prefix(part1))
        {
            return true;
        }
        if part2_upcased_is_noun
            && !part2_upcased_is_misspelled
            && self.is_country_or_region_nom_sin(part1)
            && !CITIES_EXCEPTIONS.is_match(&lt_tagger::lowercase_first_char(part2))
        {
            // e.g. Schwedenreise
            return true;
        }
        false
    }

    /// `GermanSpellerRule.processThreePartCompound`.
    fn process_three_part_compound(&self, parts: &[String]) -> bool {
        let part1 = &parts[0];
        let part2 = &parts[1];
        let part3 = &parts[2];
        let compound1 = format!("{part1}{part2}");
        let compound2 = format!("{}{part3}", lt_tagger::uppercase_first_char(part2));

        if self.is_noun(&compound1) && self.is_noun(&compound2) {
            // if part1part2 and part2part3 are compounds, so is part1part2part3
            return self.process_two_part_compounds(part1, &remove_trailing_hyphen(part2))
                && self.process_two_part_compounds(part2, part3);
        }
        if compound1.ends_with('s') || compound1.ends_with("s-") {
            let part2_no_infix = remove_trailing_s_and_hyphen(part2);
            // if part1part2NoInfixSNoHyphen and part2part3 are compounds, so
            // is part1part2part3
            return self.process_two_part_compounds(part1, &part2_no_infix)
                && self.process_two_part_compounds(part2, part3);
        }
        if self.is_verb_prefix(part1) && self.is_verb_stem(part2) && self.is_noun(&compound2) {
            // e.g. "Aus" + "leih" + "stelle"
            return true;
        }
        if self.is_noun_nom_sin(part1) && self.is_verb_stem(part2) && self.is_noun(&compound2) {
            // e.g. "Wein" + "kühl" + "schrank"
            return true;
        }
        if self.is_noun_nom(part1) && self.is_other_prefix(part2) && self.is_noun(&compound2) {
            // e.g. "Erwachsenen" + "intensiv" + "kurse"
            return true;
        }
        if self.is_other_prefix(part1) && self.is_verb_stem(part2) && self.is_noun(&compound2) {
            // e.g. "Horizontal" + "bohr" + "technik"
            return true;
        }
        false
    }

    /// `GermanSpellerRule.checkInfixSForPart1Part2Combination`.
    fn check_infix_s_for_part1_part2_combination(&self, part1: &str, part2: &str) -> bool {
        let mut part2_lemma = self.find_lemma_for_noun(&remove_trailing_hyphen(part2));
        if part2_lemma.is_empty() && remove_trailing_hyphen(part2).ends_with('s') {
            part2_lemma = self.find_lemma_for_noun(&remove_trailing_s_and_hyphen(part2));
        }
        let lemma_lc = lt_tagger::lowercase_first_char(&part2_lemma);

        if part1 == "Arbeit" && ARBEIT_COMP.is_match(part2) {
            return true; // e.g. "Arbeitplatz"
        }
        if part1 == "Arbeits" && !ARBEIT_COMP.is_match(part2) {
            return true; // e.g. "Arbeitsgeber"
        }
        if part1 == "Link" && LINK_COMP.is_match(&lemma_lc) {
            return true;
        }
        if part1 == "Links" && LINKS_COMP.is_match(&lemma_lc) {
            return true;
        }
        if part1 == "Recht" && RECHT_COMP.is_match(&lemma_lc) {
            return true; // e.g. "Rechtanwälte"
        }
        if part1 == "Rechts" && RECHTS_COMP.is_match(&lemma_lc) {
            return true; // e.g. "Rechtsfertigung"
        }
        if part1 == "Verband" && VERBAND_COMP.is_match(&lemma_lc) {
            return true; // e.g. "Verbandgemeinde"
        }
        if part1 == "Verbands" && VERBANDS_COMP.is_match(&lemma_lc) {
            return true; // e.g. "Verbandszeug"
        }
        if WOCHENTAGE.is_match(part1) && WOCHENTAG_COMP.is_match(&lemma_lc) {
            return true;
        }
        if WOCHENTAGE_S.is_match(part1) && !WOCHENTAG_COMP.is_match(&lemma_lc) {
            return true;
        }
        false
    }

    /// `GermanSpellerRule.checkConfusionForPart1Part2Combination`.
    fn check_confusion_for_part1_part2_combination(&self, part1: &str, part2: &str) -> bool {
        let part2_lemma = self.find_lemma_for_noun(&remove_trailing_hyphen(part2));
        let lemma_lc = lt_tagger::lowercase_first_char(&part2_lemma);
        if part1 == "Bad" && BACH_COMP.is_match(&lemma_lc) {
            return true;
        }
        if part1 == "Bad" && BAD_COMP.is_match(&lemma_lc) {
            return true;
        }
        if part1 == "Bade" && !BAD_COMP.is_match(&lemma_lc) {
            return true;
        }
        if part1 == "Wider" && WIDER_COMP.is_match(&lemma_lc) {
            return true;
        }
        if part1 == "Wieder" && !WIDER_COMP.is_match(&lemma_lc) {
            return true;
        }
        false
    }

    /// `GermanSpellerRule.checkPluralForPart1Part2Combination`.
    fn check_plural_for_part1_part2_combination(&self, part1: &str, part2: &str) -> bool {
        let mut part2_lemma = self.find_lemma_for_noun(&remove_trailing_hyphen(part2));
        if part2_lemma.is_empty() && remove_trailing_hyphen(part2).ends_with('s') {
            part2_lemma = self.find_lemma_for_noun(&remove_trailing_s_and_hyphen(part2));
        }
        if part1 == "Welt" && !WELTEN_COMP.is_match(&part2_lemma) {
            return true; // e.g. "Weltklima"
        }
        if part1 == "Welten" && WELTEN_COMP.is_match(&part2_lemma) {
            return true; // e.g. "Weltenbummler"
        }
        if part1 == "Wort" && !WOERTER_COMP.is_match(&part2_lemma) {
            return true; // e.g. "Wortgrenze"
        }
        if part1 == "Wörter" && WOERTER_COMP.is_match(&part2_lemma) {
            return true; // e.g. "Wörterbuch"
        }
        false
    }

    /// `GermanSpellerRule.findLemmaForNoun`.
    fn find_lemma_for_noun(&self, word: &str) -> String {
        let mut lemma = String::new();
        let readings = self.tag_one(&lt_tagger::uppercase_first_char(word));
        for reading in &readings {
            if reading.has_pos_tag_starting_with("SUB") {
                if let Some(first) = reading.readings.first() {
                    lemma = first.stem.clone().unwrap_or_else(|| first.token.clone());
                }
            }
        }
        lemma
    }

    /// `GermanSpellerRule.needsInfixS`.
    fn needs_infix_s(&self, word: &str) -> bool {
        if INFIX_S_SUFFIXES.is_match(word)
            && self
                .tag_one(word)
                .iter()
                .any(|r| r.has_pos_tag_starting_with("SUB:NOM:SIN:FEM"))
        {
            return true;
        }
        self.words_needing_infix_s.contains(word)
    }

    /// `GermanSpellerRule.needsToBePlural`.
    fn needs_to_be_plural(&self, lemma: &str) -> bool {
        if NEEDS_TO_BE_PLURAL.is_match(lemma) {
            return true;
        }
        if self
            .tag_one(&lt_tagger::uppercase_first_char(lemma))
            .iter()
            .any(|r| r.has_pos_tag_starting_with("SUB:NOM:SIN:FE"))
            && lemma.ends_with('e')
            && !SUBNOMPLUFEM_EXCEPTIONS.is_match(lemma)
        {
            return true;
        }
        if PERSON_SUFFIXES.is_match(lemma) {
            return true;
        }
        false
    }

    fn is_noun(&self, word: &str) -> bool {
        self.tag_one(word)
            .iter()
            .any(|r| r.has_pos_tag_starting_with("SUB:"))
    }

    fn is_adjective(&self, word: &str) -> bool {
        self.tag_one(word)
            .iter()
            .any(|r| r.has_pos_tag_starting_with("ADJ:"))
    }

    fn is_noun_nom(&self, word: &str) -> bool {
        self.tag_one(word)
            .iter()
            .any(|r| r.has_pos_tag_starting_with("SUB:NOM"))
    }

    fn is_noun_nom_sin(&self, word: &str) -> bool {
        self.tag_one(word)
            .iter()
            .any(|r| r.has_pos_tag_starting_with("SUB:NOM:SIN"))
    }

    fn is_noun_nom_plu(&self, word: &str) -> bool {
        self.tag_one(word)
            .iter()
            .any(|r| r.has_pos_tag_starting_with("SUB:NOM:PLU"))
    }

    fn is_country_or_region_nom_sin(&self, word: &str) -> bool {
        self.tag_one(word)
            .iter()
            .any(|r| r.has_pos_tag_matching(&COUNTRY_OR_REGION_TAG))
    }

    fn is_other_prefix(&self, word: &str) -> bool {
        self.other_prefixes
            .contains(&lt_tagger::lowercase_first_char(word))
    }

    fn is_verb_prefix(&self, word: &str) -> bool {
        self.verb_prefixes
            .contains(&lt_tagger::lowercase_first_char(word))
    }

    fn is_verb_stem(&self, word: &str) -> bool {
        self.verb_stems
            .contains(&lt_tagger::lowercase_first_char(word))
    }

    /// `GermanSpellerRule.isMisspelled`.
    pub fn is_misspelled(&self, word: &str) -> bool {
        if SCHAF_PATTERN.is_match(word) {
            let variant = word
                .trim_end_matches("schafen")
                .trim_end_matches("schafs")
                .trim_end_matches("schaf");
            let variant = format!("{variant}schaft");
            let variant = if word.ends_with("schafen") {
                format!("{variant}en")
            } else {
                variant
            };
            if !self.is_misspelled(&variant) {
                return true;
            }
        }
        if word.starts_with("Spielzug") && !START_WITH_SPIEL.is_match(word) {
            return true;
        }
        if word.starts_with("Standart")
            && !matches!(word, "Standarte" | "Standarten")
            && !word.starts_with("Standartenträger")
            && !word.starts_with("Standartenführer")
        {
            return true;
        }
        if word.ends_with("schafte") && END_WITH_SCHAFTE.is_match(word) {
            return true;
        }
        // `HunspellRule.isMisspelled`: single alphabetic chars are not
        // checked; the ignore lists win over the dictionary.
        let mut is_alphabetic = true;
        if word.chars().count() == 1 {
            is_alphabetic = word.chars().next().is_some_and(char::is_alphabetic);
        }
        (is_alphabetic
            && word != "--"
            && !self.hunspell.spell(word)
            && !self.ignore_word_base(word))
            || self.is_prohibited(&cut_off_dot(word))
    }

    /// `GermanSpellerRule.getSuggestions` (its override of
    /// `CompoundAwareHunspellRule.getSuggestions`): the dictionary edits plus
    /// the German `acceptSuggestion`/dot/single-char filters.
    pub fn suggestions(&self, word: &str) -> Vec<String> {
        let mut suggestions = self.compound_aware_suggestions(word);
        suggestions.retain(|s| self.accept_suggestion(s));
        if word.ends_with('.') {
            for s in &mut suggestions {
                if !s.ends_with('.') {
                    s.push('.');
                }
            }
        }
        suggestions.retain(|k| {
            !k.eq(word)
                && (!k.ends_with('-') || word.ends_with('-'))
                && !STARTING_WITH_SINGLE_CHAR.is_match(k)
        });
        // `GermanSynthesizer`-independent case preservation for the first
        // letter is handled by the caller (sentence-start capitalization).
        suggestions
    }

    /// `GermanSpellerRule.getCandidates`: all jWordSplitter splits, each
    /// repaired part-by-part with the dictionary suggestions.
    fn get_candidates(&self, word: &str) -> Vec<String> {
        let part_list = self.compound.get_all_splits(word);
        let mut candidates: Vec<String> = Vec::new();
        for parts in part_list {
            let mut tmp = self.candidates_for_parts(&parts);
            // avoid e.g. "Direkt-weg" and "Geheimnis-s-voll"
            tmp.retain(|k| !HYPHENED_UPPER_WORD.is_match(k) && !HYPHENED_WORD.is_match(k));
            tmp.retain(|k| !k.contains("-s-"));
            if !word.ends_with('-') {
                // avoid "xyz-" unless the input word ends in "-"
                tmp.retain(|k| !k.ends_with('-'));
            }
            candidates.extend(tmp);
            if parts.len() == 2 {
                // e.g. "inneremedizin" -> "innere Medizin", "gleichgroß" -> "gleich groß"
                candidates.push(format!("{} {}", parts[0], parts[1]));
                let up = lt_tagger::uppercase_first_char(&parts[1]);
                if self.is_noun_or_proper_noun(&up) {
                    candidates.push(format!("{} {}", parts[0], up));
                }
            }
            if parts.len() == 2 && !parts[0].ends_with('s') {
                // so we get e.g. Einzahlungschein -> Einzahlungsschein
                candidates.push(format!("{}s{}", parts[0], parts[1]));
            }
            if parts.len() == 2 && parts[1].starts_with('s') {
                // so we get e.g. Ordnungshütter -> Ordnungshüter
                let first_part = parts[0].clone();
                let second_part = parts[1][1..].to_string();
                candidates
                    .extend(self.candidates_for_parts(&[format!("{first_part}s"), second_part]));
            }
        }
        candidates
    }

    /// `MorfologikMultiSpeller.getSuggestions` (all spellers, merged and
    /// deduplicated by weight).
    fn morfo_suggestions(&self, word: &str) -> Vec<String> {
        self.morfo
            .get_suggestions(word)
            .into_iter()
            .map(|WeightedSuggestion { word, .. }| word)
            .collect()
    }

    /// `CompoundAwareHunspellRule.getCandidates(List<String>)`.
    fn candidates_for_parts(&self, parts: &[String]) -> Vec<String> {
        let mut candidates: Vec<String> = Vec::new();
        for (part_count, part) in parts.iter().enumerate() {
            if self.hunspell.spell(part) {
                continue;
            }
            // assume noun, so use uppercase:
            let do_upper = part_count > 0 && !part.chars().next().is_some_and(char::is_uppercase);
            let mut suggestions = if do_upper {
                self.morfo_suggestions(&lt_tagger::uppercase_first_char(part))
            } else {
                self.morfo_suggestions(part)
            };
            if suggestions.is_empty() {
                suggestions = if do_upper {
                    self.morfo_suggestions(&lt_tagger::lowercase_first_char(part))
                } else {
                    self.morfo_suggestions(part)
                };
            }
            let mut append_s = false;
            if do_upper && part.ends_with('s') {
                // maybe infix-s as in "Dampfschiffahrtskapitän"
                // (`StringUtils.removeEnd`: strip one trailing "s", not all)
                suggestions.extend(self.morfo_suggestions(part.strip_suffix('s').unwrap_or(part)));
                append_s = true;
            }
            for mut suggestion in suggestions {
                let mut parts_copy = parts.to_vec();
                if append_s {
                    suggestion.push('s');
                }
                if part_count > 0
                    && parts[part_count].starts_with('-')
                    && parts[part_count].chars().count() > 1
                {
                    let rest: String = suggestion.chars().skip(1).collect();
                    parts_copy[part_count] = format!("-{}", lt_tagger::uppercase_first_char(&rest));
                } else if part_count > 0 && !parts[part_count - 1].ends_with('-') {
                    parts_copy[part_count] = suggestion.to_lowercase();
                } else {
                    parts_copy[part_count] = suggestion.clone();
                }
                let candidate: String = parts_copy.concat();
                if !self.is_misspelled(&candidate) {
                    candidates.push(candidate);
                }
                // Arbeidszimmer -> Arbeitszimmer
                if part_count < parts.len() - 1 && part.ends_with('s') && suggestion.ends_with('-')
                {
                    parts_copy[part_count] = suggestion[..suggestion.len() - 1].to_string();
                    let infix_candidate: String = parts_copy.concat();
                    if !self.is_misspelled(&infix_candidate) {
                        candidates.push(infix_candidate);
                    }
                }
            }
        }
        candidates
    }

    /// `CompoundAwareHunspellRule.getCorrectWords`.
    fn get_correct_words(&self, candidates: &[String]) -> Vec<String> {
        candidates
            .iter()
            .filter(|candidate| {
                // the phrase is split with the hunspell tokenizer pattern
                candidate
                    .split(|c: char| !(c.is_alphabetic() || c == 'ß' || c == '-' || c == '.'))
                    .filter(|w| !w.is_empty())
                    .all(|word| self.hunspell.spell(word))
            })
            .cloned()
            .collect()
    }

    /// `CompoundAwareHunspellRule.getSuggestions`: no-split dictionary
    /// suggestions interleaved with the lowercase variants and the split
    /// `getCandidates`/`getCorrectWords` suggestions. The dictionary
    /// suggestions come from the `SpellChecker` generate-and-validate walk
    /// (documented deviation: Java uses `MorfologikMultiSpeller`/Oflazer).
    fn compound_aware_suggestions(&self, word: &str) -> Vec<String> {
        let mut no_split = self.morfo_suggestions(word);
        // `handleWordEndPunctuation`
        for punct in [".", "..."] {
            if let Some(stem) = word.strip_suffix(punct) {
                for s in self.morfo_suggestions(stem) {
                    no_split.push(format!("{s}{punct}"));
                }
            }
        }
        let mut no_split_lowercase: Vec<String> = Vec::new();
        if starts_with_uppercase(word) && !lt_tagger::is_all_uppercase(word) {
            no_split_lowercase = self.morfo_suggestions(&word.to_lowercase());
        }
        // Java applies the (German) `getFilteredSuggestions` to the split
        // candidates before they are mixed in.
        let simple_suggestions =
            self.get_filtered_suggestions(self.get_correct_words(&self.get_candidates(word)));
        let max = simple_suggestions
            .len()
            .max(no_split.len())
            .max(no_split_lowercase.len());
        let mut suggestions: Vec<String> = Vec::new();
        for i in 0..max {
            if i < no_split.len() {
                suggestions.push(no_split[i].clone());
            }
            if i < no_split_lowercase.len() {
                suggestions.push(lt_tagger::uppercase_first_char(&no_split_lowercase[i]));
            }
            if i < simple_suggestions.len() {
                suggestions.push(simple_suggestions[i].clone());
            }
        }
        let mut seen = HashSet::new();
        suggestions.retain(|s| seen.insert(s.clone()));
        self.filter_for_language(&mut suggestions);
        let suggestions = self.sort_suggestion_by_quality(word, &suggestions);
        suggestions.into_iter().take(20).collect()
    }

    /// `GermanSpellerRule.filterForLanguage` (de-CH `ß` → `ss`, part-with-
    /// punctuation removal, leading `-` removal).
    fn filter_for_language(&self, suggestions: &mut Vec<String>) {
        if self.swiss {
            for s in suggestions.iter_mut() {
                *s = s.replace('ß', "ss");
            }
        }
        suggestions.retain(|s| {
            !(s.split(' ').any(|k| WORD_WITH_PUNCT.is_match(k))
                || s.chars().count() > 1 && s.starts_with('-'))
        });
    }

    /// `GermanSpellerRule.acceptSuggestion`.
    fn accept_suggestion(&self, s: &str) -> bool {
        !COMPILED_PREVENT_PATTERNS.iter().any(|p| p.is_match(s))
            && !s.contains("--")
            && !s.ends_with("roulett")
            && !s.ends_with("-s")
            && !s.ends_with(" de")
            && !s.ends_with(" en")
            && !s.ends_with(" Artigen")
            && !s.ends_with(" Artige")
            && !s.ends_with(" artigen")
            && !s.ends_with(" artiges")
            && !s.ends_with(" artiger")
            && !s.ends_with(" artige")
            && !s.ends_with(" artig")
            && !s.ends_with(" gen")
            && !s.ends_with(" ehe")
            && !s.ends_with(" ende")
            && !s.ends_with(" enden")
            && !s.ends_with(" enge")
            && !s.ends_with(" förmig")
            && !s.ends_with(" förmige")
            && !s.ends_with(" förmigen")
            && !s.ends_with(" förmiger")
            && !s.ends_with(" förmiges")
            && !s.starts_with("Doppel ")
            && !s.starts_with("Kombi ")
    }

    fn tag_one(&self, word: &str) -> Vec<AnalyzedTokenReadings> {
        self.tagger.tag(&[word.to_string()], true)
    }

    /// `GermanSpellerRule.getOnlySuggestions`: suggestions that replace all
    /// others (the word is known to be spelled correctly).
    fn get_only_suggestions(&self, word: &str) -> Vec<String> {
        let one = |s: String| vec![s];
        if SYMPHATISCH.is_match(word) {
            return one(word.replacen("symphatisch", "sympathisch", 1));
        }
        if AUTENTISCH_WITH_CASES.is_match(word) {
            return one(word.replacen("utent", "uthent", 1));
        }
        if BRILLIANT_WITH_CASES.is_match(word) {
            return one(word.replacen("brilliant", "brillant", 1));
        }
        if RECHTMASIG_WITH_CASES.is_match(word) {
            return one(word.replacen("mässig", "mäßig", 1));
        }
        if CONTAINS_MASZNAME.is_match(word) {
            return one(re_first(word, "a(ss|ß)name", "aßnahme"));
        }
        if HOLZ_SPIEGEL_PANEL_COMPOUND.is_match(word) {
            return one(word.replacen("panel", "paneel", 1));
        }
        if SBHAN_PREFIX.is_match(word) {
            return one(word.replacen("SBahn", "S-Bahn", 1));
        }
        if UBAHN_PREFIX.is_match(word) {
            return one(word.replacen("UBahn", "U-Bahn", 1));
        }
        if matches!(
            word,
            "Büffet" | "Buffett" | "Bufett" | "Büffett" | "Bufet" | "Büfet"
        ) {
            return one(if self.swiss || self.variant == "de-AT" {
                "Buffet"
            } else {
                "Büfett"
            }
            .to_string());
        }
        for (w, values) in ONLY_SUGGESTIONS {
            if word == *w {
                return values.iter().map(|v| v.to_string()).collect();
            }
        }
        Vec::new()
    }

    /// `GermanSpellerRule.filterNoSuggestWords`.
    fn filter_no_suggest_words(&self, suggestions: Vec<String>) -> Vec<String> {
        suggestions
            .into_iter()
            .filter(|s| {
                let lc = s.to_lowercase();
                !LC_DO_NOT_SUGGEST.contains(&lc.as_str())
                    && !START_WITH_NEGER.is_match(&lc)
                    && !CONTAINS_NEGER.is_match(&lc)
                    && !CONTAINS_NEGER_2.is_match(&lc)
                    && !UNCOMMON_LOWERCASED_NOUN_AT_END.is_match(s)
                    && !UNCOMMON_LOWERCASED_NOUN_AT_BEGINNING.is_match(s)
            })
            .collect()
    }

    /// `CompoundAwareHunspellRule.sortSuggestionByQuality` with the German
    /// override: drop undesired inflections (unless the form matches the
    /// misspelling's ending), boost case-only and multi-word suggestions.
    fn sort_suggestion_by_quality(&self, misspelling: &str, suggestions: &[String]) -> Vec<String> {
        // Java tags the whole list in one call, so the sentence-start
        // lowercase expansion applies to the first suggestion only.
        let readings_list: Vec<AnalyzedTokenReadings> = self.tagger.tag(suggestions, true);
        let mut lemma_to_filter = String::new();
        let mut form_to_accept = String::new();
        let misspelling_tail: String = {
            let chars: Vec<char> = misspelling.chars().collect();
            let start = chars.len().saturating_sub(2);
            chars[start..].iter().collect()
        };
        for readings in &readings_list {
            let partial = readings.readings.iter().any(|t| {
                t.pos_tag.as_deref().is_some_and(|p| {
                    p.contains("ADJ")
                        || p.contains("SUB")
                        || p.contains("PA1:")
                        || p.contains("PA2:")
                })
            });
            if partial {
                let token = readings
                    .readings
                    .first()
                    .map(|t| t.token.clone())
                    .unwrap_or_default();
                if token.ends_with(&misspelling_tail) {
                    form_to_accept = token;
                    lemma_to_filter = readings
                        .readings
                        .first()
                        .and_then(|t| t.stem.clone())
                        .unwrap_or_default();
                    break;
                }
            }
        }
        let mut filtered: Vec<String> = Vec::new();
        if !lemma_to_filter.is_empty()
            && !form_to_accept.is_empty()
            && misspelling.chars().count() > 1
        {
            for (i, s) in suggestions.iter().enumerate() {
                let has_lemma = readings_list[i]
                    .readings
                    .iter()
                    .any(|t| t.stem.as_deref() == Some(lemma_to_filter.as_str()));
                let keep = s == &form_to_accept || !has_lemma;
                if keep && !filtered.contains(s) {
                    filtered.push(s.clone());
                }
            }
        } else {
            filtered.extend_from_slice(suggestions);
        }
        let mut result: Vec<String> = Vec::new();
        let mut top: Vec<String> = Vec::new();
        for s in filtered {
            if misspelling.eq_ignore_ascii_case(&s) {
                top.push(s);
            } else if s.contains(' ') {
                // no language model in this harness (D-026): every multi-word
                // suggestion is boosted, like Java's `languageModel == null`
                top.push(s);
            } else {
                result.push(s);
            }
        }
        result.splice(0..0, top);
        result
    }

    /// `GermanSpellerRule.getFilteredSuggestions`.
    fn get_filtered_suggestions(&self, words_or_phrases: Vec<String>) -> Vec<String> {
        let mut result = Vec::new();
        for word_or_phrase in words_or_phrases {
            // `HunspellRule.tokenizeText`: the `.aff` `WORDCHARS` are `ß-.`,
            // so hyphens/dots do not split suggestions like "Wi-Fi".
            let words: Vec<&str> = word_or_phrase
                .split(|c: char| !(c.is_alphabetic() || c == 'ß' || c == '-' || c == '.'))
                .filter(|w| !w.is_empty())
                .collect();
            let two_upper_phrases = words.len() >= 2
                && self.is_adj_or_noun_or_unknown(words[0])
                && self.is_noun_or_unknown(words[1])
                && lt_tagger::is_capitalized_word(words[0])
                && lt_tagger::is_capitalized_word(words[1]);
            let verb_phrase = words.len() == 2
                && self.is_adj_base_form(words[0])
                && !lt_tagger::is_capitalized_word(words[0])
                && self.is_sub_ver_inf(words[1]);
            if !two_upper_phrases && !verb_phrase {
                result.push(word_or_phrase);
            }
        }
        result
    }

    fn is_noun_or_unknown(&self, word: &str) -> bool {
        let readings = self.tag_one(word);
        readings
            .iter()
            .any(|r| r.has_pos_tag_starting_with("SUB") || is_pos_tag_unknown(r))
    }

    #[allow(dead_code)] // used by `getCandidates` once ported
    fn is_only_noun(&self, word: &str) -> bool {
        let readings = self.tag_one(word);
        !readings.is_empty()
            && readings.iter().all(|r| {
                r.readings
                    .iter()
                    .all(|t| t.pos_tag.as_deref().is_some_and(|p| p.starts_with("SUB:")))
            })
    }

    fn is_adj_or_noun_or_unknown(&self, word: &str) -> bool {
        let readings = self.tag_one(word);
        readings.iter().any(|r| {
            r.has_pos_tag_starting_with("ADJ")
                || r.has_pos_tag_starting_with("SUB")
                || is_pos_tag_unknown(r)
        })
    }

    #[allow(dead_code)] // used by `getCandidates` once ported
    fn is_noun_or_proper_noun(&self, word: &str) -> bool {
        let readings = self.tag_one(word);
        readings
            .iter()
            .any(|r| r.has_pos_tag_starting_with("SUB") || r.has_pos_tag_starting_with("EIG"))
    }

    fn is_sub_ver_inf(&self, word: &str) -> bool {
        let readings = self.tag_one(word);
        readings
            .iter()
            .any(|r| r.has_pos_tag_matching(&SUB_VER_INF))
    }

    fn is_adj_base_form(&self, word: &str) -> bool {
        let readings = self.tag_one(word);
        readings
            .iter()
            .any(|r| r.has_pos_tag_starting_with("ADJ:PRD:GRU"))
    }

    /// `GermanSpellerRule.getAdditionalTopSuggestionsString` (curated
    /// suggestions that are prepended to the dictionary suggestions).
    fn additional_top_suggestions_string(
        &self,
        suggestions: &[String],
        word: &str,
    ) -> Option<Vec<String>> {
        let one = |s: &str| Some(vec![s.to_string()]);
        if word.eq_ignore_ascii_case("WIFI") {
            return one("Wi-Fi");
        }
        if word.eq_ignore_ascii_case("W-Lan") {
            return one("WLAN");
        }
        let table: &[(&str, &[&str])] = &[
            ("Endstadion", &["Endstadium"]),
            ("Endstadions", &["Endstadiums"]),
            ("genomen", &["genommen"]),
            ("Preis-Leistungsverhältnis", &["Preis-Leistungs-Verhältnis"]),
            ("getz", &["jetzt", "geht's"]),
            ("Trons", &["Trance"]),
            ("ei", &["ein"]),
            ("jo", &["ja"]),
            ("jepp", &["ja"]),
            ("jopp", &["ja"]),
            ("Jo", &["Ja"]),
            ("Jepp", &["Ja"]),
            ("Jopp", &["Ja"]),
            ("Ne", &["Nein", "Eine"]),
            ("is", &["ist"]),
            ("Is", &["Ist"]),
            ("un", &["und"]),
            ("Un", &["Und"]),
            ("Std", &["Std."]),
            ("gin", &["ging"]),
            ("dh", &["d.\u{202f}h."]),
            ("dh.", &["d.\u{202f}h."]),
            ("ua", &["u.\u{202f}a."]),
            ("ua.", &["u.\u{202f}a."]),
            ("uvm", &["u.\u{202f}v.\u{202f}m."]),
            ("uvm.", &["u.\u{202f}v.\u{202f}m."]),
            ("udgl", &["u.\u{202f}dgl."]),
            ("udgl.", &["u.\u{202f}dgl."]),
            ("Ruhigkeit", &["Ruhe"]),
            ("angepreist", &["angepriesen"]),
            ("halo", &["hallo"]),
            ("ca", &["ca."]),
            ("Jezt", &["Jetzt"]),
            ("Wollst", &["Wolltest"]),
            ("wollst", &["wolltest"]),
            ("Rolladen", &["Rollladen"]),
            ("Maßname", &["Maßnahme"]),
            ("Maßnamen", &["Maßnahmen"]),
            ("nanten", &["nannten"]),
            ("diees", &["dieses", "dies"]),
            ("Diees", &["Dieses", "Dies"]),
            ("Lobbies", &["Lobbys"]),
            ("Parties", &["Partys"]),
            ("Babies", &["Babys"]),
            ("Hallochen", &["Hallöchen", "hallöchen"]),
            ("hallochen", &["hallöchen"]),
            ("ok", &["okay", "O.\u{202f}K."]),
            ("gesuchen", &["gesuchten", "gesucht"]),
            ("Germanistiker", &["Germanist", "Germanisten"]),
            ("Abschlepper", &["Abschleppdienst", "Abschleppwagen"]),
            ("par", &["paar"]),
            ("iwie", &["irgendwie"]),
            ("schwarzfarbenden", &["schwarzfarbenen"]),
            ("bzgl", &["bzgl."]),
            ("bau", &["baue"]),
            ("sry", &["sorry"]),
            ("Sry", &["Sorry"]),
            ("thx", &["danke"]),
            ("Thx", &["Danke"]),
            ("Zynik", &["Zynismus"]),
            ("pieksen", &["piksen"]),
            ("piekst", &["pikst"]),
            ("gepiekst", &["gepikst"]),
            ("wiederspiegeln", &["widerspiegeln"]),
            ("ch", &["ich"]),
        ];
        for (w, sugg) in table {
            if word == *w {
                return Some(sugg.iter().map(|s| s.to_string()).collect());
            }
        }
        if word.eq_ignore_ascii_case("zumindestens") {
            return one(&word.replace("ens", ""));
        }
        if word.eq_ignore_ascii_case("email") {
            return one("E-Mail");
        }
        if word.chars().count() > 9 && word.starts_with("Email") {
            let mut suffix: String = word.chars().skip(5).collect();
            if !self.hunspell.spell(&suffix) {
                let uc = lt_tagger::uppercase_first_char(&suffix);
                // Java uses native hunspell's `suggest` here; the hunspell
                // suggestion generator is not ported, so the morfologik
                // multi-speller stands in (documented deviation).
                let suffix_suggestions = self.morfo_suggestions(&uc);
                if let Some(first) = suffix_suggestions.first() {
                    suffix = first.clone();
                }
            }
            let mut chars = suffix.chars();
            let rest: String = chars.clone().skip(1).collect();
            return one(&format!(
                "E-Mail-{}{}",
                chars
                    .next()
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_default(),
                rest
            ));
        }
        // complex `endsWith`/`startsWith`/matcher branches, in Java's order
        let replaced = |pat: &'static str, repl: &str| re_first(word, pat, repl);
        let checked = |s: String| -> Option<Vec<String>> {
            if self.hunspell.spell(&s) {
                Some(vec![s])
            } else {
                None
            }
        };
        if ENDS_WITH_IBELKEIT_IBLICHKEIT.is_match(word) {
            if let Some(s) = checked(replaced("el[hk]eit$", "ilität")) {
                return Some(s);
            }
        }
        if word.ends_with("aquise") {
            if let Some(s) = checked(replaced("aquise$", "akquise")) {
                return Some(s);
            }
        }
        if word.ends_with("standart") {
            if let Some(s) = checked(replaced("standart$", "standard")) {
                return Some(s);
            }
        }
        if word.ends_with("standarts") {
            if let Some(s) = checked(replaced("standarts$", "standards")) {
                return Some(s);
            }
        }
        if word.ends_with("tips") {
            if let Some(s) = checked(replaced("tips$", "tipps")) {
                return Some(s);
            }
        }
        if word.ends_with("tip") {
            if let Some(s) = checked(format!("{word}p")) {
                return Some(s);
            }
        }
        if word.ends_with("entfehlung") {
            if let Some(s) = checked(word.replacen("ent", "emp", 1)) {
                return Some(s);
            }
        }
        if word.ends_with("oullie") {
            if let Some(s) = checked(replaced("oullie$", "ouille")) {
                return Some(s);
            }
        }
        if word.starts_with("Bundstift") {
            if let Some(s) = checked(replaced("^Bundstift", "Buntstift")) {
                return Some(s);
            }
        }
        if ALLMAHLLIG.is_match(word) {
            if let Some(s) = checked(replaced("llmähll?i(g|ch)", "llmählich")) {
                return Some(s);
            }
        }
        if CONTAINS_MAYONNAISE.is_match(word) {
            if let Some(s) = checked(replaced("a[jy]onn?[äe]se", "ayonnaise")) {
                return Some(s);
            }
        }
        if CONTAINS_RESERVIERUNG.is_match(word) {
            if let Some(s) = checked(replaced("es(a|er)[vw]i[he]?rung", "eservierung")) {
                return Some(s);
            }
        }
        if STARTS_WITH_RESCHASCHIER.is_match(word) {
            if let Some(s) = checked(word.replacen("schaschier", "cherchier", 1)) {
                return Some(s);
            }
        }
        if ENDS_WITH_LABORANTS.is_match(word) {
            if let Some(s) = checked(replaced("ts$", "ten")) {
                return Some(s);
            }
        }
        if PROFESSIONELL.is_match(word) {
            if let Some(s) = checked(replaced("roff?ess?ion([äe])h?l{1,2}", "rofessionell")) {
                return Some(s);
            }
        }
        if VERSTANDNIS.is_match(word) {
            if let Some(s) = checked(replaced("[vV]erstehendnis", "Verständnis")) {
                return Some(s);
            }
        }
        if word.starts_with("koregier") {
            if let Some(s) = checked(word.replace("reg", "rrig")) {
                return Some(s);
            }
        }
        if STARTS_WITH_DIAGNOSZIER.is_match(word) {
            if let Some(s) = checked(replaced("gno[sz]ier", "gnostizier")) {
                return Some(s);
            }
        }
        if word.contains("eiss") {
            if let Some(s) = checked(word.replace("eiss", "eiß")) {
                return Some(s);
            }
        }
        if word.contains("Akkupressur") {
            if let Some(s) = checked(word.replace("Akkupressur", "Akupressur")) {
                return Some(s);
            }
        }
        if word.contains("farbend") {
            if let Some(s) = checked(word.replace("farbend", "farben")) {
                return Some(s);
            }
        }
        if word.contains("uess") {
            if let Some(s) = checked(word.replace("uess", "üß")) {
                return Some(s);
            }
        }
        if ZB.is_match(word) || STARTS_WITH_ZB.is_match(word) {
            return one("z.\u{202f}B.");
        }
        if word.ends_with("ies") {
            if word.ends_with("derbies") {
                if let Some(s) = checked(replaced("derbies$", "derbys")) {
                    return Some(s);
                }
            } else if word.ends_with("stories") {
                if let Some(s) = checked(replaced("stories$", "storys")) {
                    return Some(s);
                }
            } else if word.ends_with("parties") {
                if let Some(s) = checked(replaced("parties$", "partys")) {
                    return Some(s);
                }
            }
        }
        for (pattern, exprs) in COMPILED_ADDITIONAL_SUGGESTIONS.iter() {
            if pattern.is_match(word) {
                return Some(
                    exprs
                        .iter()
                        .map(|e| self.apply_suggest_expr(word, e))
                        .collect(),
                );
            }
        }
        // `if (!startsWithUppercase(word))`: offer the capitalized word when
        // it is in the dictionary and not already suggested.
        if !lt_tagger::is_capitalized_word(word) {
            let uc = lt_tagger::uppercase_first_char(word);
            if !suggestions.contains(&uc) && self.hunspell.spell(&uc) && !uc.ends_with('.') {
                return Some(vec![uc]);
            }
        }
        if let Some(s) = self.past_tense_verb_suggestion(word) {
            return Some(vec![s]);
        }
        if let Some(s) = self.participle_suggestion(word) {
            return Some(vec![s]);
        }
        if let Some(s) = self.abbreviation_suggestion(word) {
            return Some(vec![s]);
        }
        // hyphenated compounds (e.g. "Netflix-Flm")
        if suggestions.is_empty() && word.contains('-') {
            let parts = java_split_hyphen(word);
            if parts.len() > 1 {
                let mut suggestion_lists: Vec<Vec<String>> = Vec::with_capacity(parts.len());
                let mut start_at = 0usize;
                let mut stop_at = parts.len();
                let mut partial_word = format!("{}-{}", parts[0], parts[1]);
                if self.ignore_word_base(&partial_word)
                    || self
                        .words_to_be_ignored_in_compounds
                        .contains(&partial_word)
                {
                    // "Au-pair-Agentr"
                    start_at = 2;
                    suggestion_lists.push(vec![format!("{}-{}", parts[0], parts[1])]);
                }
                partial_word = format!("{}-{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                if self.ignore_word_base(&partial_word)
                    || self
                        .words_to_be_ignored_in_compounds
                        .contains(&partial_word)
                {
                    // "Seniren-Au-pair"
                    stop_at = parts.len() - 2;
                }
                for part in &parts[start_at..stop_at] {
                    if !self.hunspell.spell(part) {
                        let list = self.compound_aware_suggestions(part);
                        suggestion_lists.push(self.sort_suggestion_by_quality(part, &list));
                    } else {
                        suggestion_lists.push(vec![part.clone()]);
                    }
                }
                if stop_at < parts.len() - 1 {
                    suggestion_lists.push(vec![partial_word]);
                }
                // avoid OutOfMemory on words like
                // "free-and-open-source-and-cross-platform"
                if suggestion_lists.len() <= 3 {
                    let mut additional_suggestions = suggestion_lists[0].clone();
                    for suggestion_list in suggestion_lists.iter().skip(1) {
                        let mut new_list = Vec::with_capacity(
                            additional_suggestions.len() * suggestion_list.len(),
                        );
                        for additional in &additional_suggestions {
                            for candidate in suggestion_list {
                                new_list.push(format!("{additional}-{candidate}"));
                            }
                        }
                        additional_suggestions = new_list;
                    }
                    // we just take the first results, although we don't know
                    // whether they are better
                    additional_suggestions.truncate(5);
                    return Some(additional_suggestions);
                }
            }
        }
        None
    }

    /// `GermanSpellerRule.getPastTenseVerbSuggestion`.
    fn past_tense_verb_suggestion(&self, word: &str) -> Option<String> {
        if !word.ends_with('e') {
            return None;
        }
        let word_stem = &word[..word.len() - 1];
        let lemma = self.base_for_third_person_singular_verb(word_stem)?;
        let token = AnalyzedToken::new(lemma.clone(), Some(lemma), None);
        self.synth
            .synthesize(&token, "VER:3:SIN:PRT:.*", true)
            .into_iter()
            .next()
    }

    /// `GermanSpellerRule.baseForThirdPersonSingularVerb`.
    fn base_for_third_person_singular_verb(&self, word: &str) -> Option<String> {
        let readings = self.tag_one(word);
        readings.iter().find_map(|reading| {
            if reading.has_pos_tag_starting_with("VER:3:SIN") {
                reading.readings.first().and_then(|t| t.stem.clone())
            } else {
                None
            }
        })
    }

    /// `GermanSpellerRule.getParticipleSuggestion`.
    fn participle_suggestion(&self, word: &str) -> Option<String> {
        if word.starts_with("ge") && word.ends_with('t') {
            let baseform = format!("{}en", &word[2..word.len() - 1]);
            return self.participle_for_baseform(&baseform);
        }
        None
    }

    /// `GermanSpellerRule.getParticipleForBaseform`.
    fn participle_for_baseform(&self, baseform: &str) -> Option<String> {
        let token = AnalyzedToken::new(baseform.to_string(), Some(baseform.to_string()), None);
        let forms = self.synth.synthesize(&token, "VER:PA2:.*", true);
        forms
            .first()
            .filter(|form| self.hunspell.spell(form))
            .cloned()
    }

    /// `GermanSpellerRule.getAbbreviationSuggestion`.
    fn abbreviation_suggestion(&self, word: &str) -> Option<String> {
        if word.chars().count() < 5 {
            let readings = self.tag_one(word);
            if readings
                .iter()
                .any(|reading| reading.has_pos_tag_starting_with("ABK"))
            {
                return Some(format!("{word}."));
            }
        }
        None
    }

    fn apply_suggest_expr(&self, word: &str, expr: &SuggestExpr) -> String {
        match expr {
            SuggestExpr::Lit(s) => (*s).to_string(),
            SuggestExpr::ReplaceFirst {
                pattern,
                replacement,
            } => re_first(word, pattern, replacement),
            SuggestExpr::ReplaceOnce { from, to } => word.replacen(from, to, 1),
            SuggestExpr::UpperFirst(inner) => {
                let value = self.apply_suggest_expr(word, &inner[0]);
                lt_tagger::uppercase_first_char(&value)
            }
        }
    }

    /// German `getMessage` for the `ss`/`ß` case; falls back to `spelling`.
    fn message_for(&self, word: &str, first_suggestion: Option<&str>) -> String {
        if let Some(first) = first_suggestion {
            if let Some(byte_pos) = word.find("ss") {
                // Java `indexOf`/`charAt` are UTF-16 units; for German BMP
                // text that is the char index.
                let pos = word[..byte_pos].chars().count();
                let replaced = word.replacen("ss", "ß", 1);
                if replaced == first && pos >= 2 {
                    let chars: Vec<char> = word.chars().collect();
                    if let (Some(&prev), Some(&prev_prev)) =
                        (chars.get(pos - 1), chars.get(pos - 2))
                    {
                        if is_vowel(prev_prev) && is_vowel(prev) {
                            return format!(
                                "Nach einer Silbe aus zwei Vokalen (hier: {prev_prev}{prev}) schreibt man 'ß' statt 'ss'."
                            );
                        }
                        return format!(
                            "Nach einer lang gesprochenen Silbe (hier: {prev}) schreibt man 'ß' statt 'ss'."
                        );
                    }
                }
            }
        }
        MESSAGE.to_string()
    }

    /// `HunspellRule.calcSuggestions` for the German rule.
    fn calc_suggestions(&self, word: &str, clean_word: &str) -> Arc<Vec<Suggestion>> {
        let cache_key = format!("{word}\u{0}{clean_word}");
        if let Ok(cache) = self.suggestion_cache.lock() {
            if let Some(hit) = cache.get(&cache_key) {
                return Arc::clone(hit);
            }
        }
        let only = self.get_only_suggestions(clean_word);
        let suggestions: Vec<String> = if !only.is_empty() {
            only
        } else {
            let mut suggestions = self.suggestions(clean_word);
            if word.ends_with('.') {
                let mut pos = 1usize;
                for suggestion in self.suggestions(word) {
                    if !suggestions.contains(&suggestion) {
                        let value = suggestion
                            .strip_suffix('.')
                            .unwrap_or(&suggestion)
                            .to_string();
                        let at = pos.min(suggestions.len());
                        suggestions.insert(at, value);
                        pos += 2;
                    }
                }
            }
            let top = self.additional_top_suggestions_string(&suggestions, clean_word);
            let top = match (top, word.ends_with('.')) {
                (Some(t), _) if !t.is_empty() => Some(t),
                (_, true) => self
                    .additional_top_suggestions_string(&suggestions, word)
                    .map(|list| {
                        list.into_iter()
                            .map(|s| if s.ends_with('.') { s } else { format!("{s}.") })
                            .collect()
                    }),
                (t, _) => t,
            };
            if let Some(mut top) = top {
                top.reverse();
                for s in top {
                    if clean_word != s {
                        suggestions.insert(0, s);
                    }
                }
            }
            suggestions.retain(|s| self.accept_suggestion(s));
            self.filter_suggestions(suggestions)
        };
        let suggestions = suggestions
            .into_iter()
            .map(|value| Suggestion {
                value,
                short_description: None,
            })
            .collect::<Vec<_>>();
        let computed = Arc::new(suggestions);
        if let Ok(mut cache) = self.suggestion_cache.lock() {
            if cache.len() >= 50_000 {
                cache.clear();
            }
            cache.insert(cache_key, Arc::clone(&computed));
        }
        computed
    }

    /// `SpellingCheckRule.filterSuggestions` (prohibited words, dedupe and
    /// the German `filterNoSuggestWords`).
    fn filter_suggestions(&self, suggestions: Vec<String>) -> Vec<String> {
        let filtered: Vec<String> = suggestions
            .into_iter()
            .filter(|s| !self.is_prohibited(s))
            .collect();
        let mut seen = HashSet::new();
        let deduped: Vec<String> = filtered
            .into_iter()
            .filter(|s| seen.insert(s.clone()))
            .collect();
        self.filter_no_suggest_words(deduped)
    }

    /// Test/probe helper: `calcSuggestions` for a single word (Java
    /// `check()` uses the same path through `match()`).
    #[doc(hidden)]
    pub fn probe_match_suggestions(&self, word: &str) -> Vec<String> {
        let clean = cut_off_dot(word);
        self.calc_suggestions(word, &clean)
            .iter()
            .map(|s| s.value.clone())
            .collect()
    }

    /// Test/probe helper: raw `MorfologikMultiSpeller.getSuggestions` as
    /// `(word, weight)` pairs (mirrors `ProbeMorfo.java`).
    #[doc(hidden)]
    pub fn probe_morfo_suggestions(&self, word: &str) -> Vec<(String, i32)> {
        self.morfo
            .get_suggestions(word)
            .into_iter()
            .map(|s| (s.word, s.weight))
            .collect()
    }

    fn new_rule_match(
        &self,
        from: usize,
        to: usize,
        message: &str,
        suggestions: Vec<Suggestion>,
    ) -> Match {
        Match::new(
            &self.rule_id,
            Option::<String>::None,
            message,
            Some(SHORT_MESSAGE.to_string()),
            TextRange::new(from, to),
            suggestions,
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(DESCRIPTION, "misspelling", 0)
        .with_match_type("UnknownWord")
    }

    /// `SpellingCheckRule.createWrongSplitMatch`: if the last match starts at
    /// the previous word, it is replaced by one covering both words.
    fn push_wrong_split_match(
        &self,
        matches: &mut Vec<Match>,
        suggestion1: &str,
        suggestion2: &str,
        from: usize,
        to: usize,
    ) {
        if let Some(prev) = matches.last() {
            if prev.range.start == from {
                matches.pop();
            }
        }
        let mut m = self.new_rule_match(
            from,
            to,
            MESSAGE,
            vec![Suggestion {
                value: format!("{suggestion1} {suggestion2}"),
                short_description: None,
            }],
        );
        m.suggestions[0].value = m.suggestions[0].value.trim().to_string();
        matches.push(m);
    }

    /// `HunspellRule.match` over one sentence's token stream (absolute byte
    /// offsets applied by the caller). The sentence is re-tokenized with the
    /// hunspell non-word pattern (`.aff` `WORDCHARS` = `\u{df}-.`), so
    /// abbreviations like "Fr." stay one token; `len` mirrors Java's
    /// cumulative UTF-16 offset arithmetic (including its one-separator-per
    /// token assumption) for byte-exact match ranges.
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let sentence_text: String = tokens.iter().map(|t| t.surface()).collect();
        let masked = self.sentence_text_for_speller(tokens);
        let words = hunspell_tokens(&masked);
        let mut len = if tokens.len() > 1 {
            crate::to_utf16_offset(&sentence_text, tokens[1].start_pos)
        } else {
            tokens
                .first()
                .map(|t| crate::to_utf16_offset(&sentence_text, t.start_pos))
                .unwrap_or(0)
        };
        let byte = |utf16_off: usize| -> usize {
            sentence_offset + utf16_to_utf8_offset(&sentence_text, utf16_off)
        };
        let mut matches: Vec<Match> = Vec::new();
        let mut prev_start_pos: Option<usize> = None;
        for (idx, word) in words.iter().enumerate() {
            let word_utf16 = word.encode_utf16().count();
            let mut dash_corr = 0usize;
            if (self.german_ignore_word(&words, idx) || self.ignore_word_base(word))
                && !self.is_prohibited(&cut_off_dot(word))
            {
                prev_start_pos = Some(len);
                len += word_utf16 + 1;
                continue;
            }
            if self.is_misspelled(word) {
                if self.ignore_potentially_misspelled_word(word) {
                    prev_start_pos = Some(len);
                    len += word_utf16 + 1;
                    continue;
                }
                let clean_word = word.strip_suffix('.').unwrap_or(word).to_string();
                if word.starts_with('-') {
                    if !self.is_misspelled(&clean_word[1..]) || clean_word.chars().all(|c| c == '-')
                    {
                        len += word_utf16 + 1;
                        continue;
                    } else {
                        dash_corr += 1;
                    }
                }
                // "thanky ou" -> "thank you" / "than kyou" -> "thank you"
                if idx > 0 {
                    if let Some(prev_pos) = prev_start_pos {
                        let prev_word = &words[idx - 1];
                        if !prev_word.is_empty() {
                            let prev_chars: Vec<char> = prev_word.chars().collect();
                            let sugg1a: String =
                                prev_chars[..prev_chars.len() - 1].iter().collect();
                            let mut sugg1b: String =
                                prev_chars[prev_chars.len() - 1..].iter().collect();
                            sugg1b.push_str(&clean_word);
                            let sugg1b = cut_off_dot(&sugg1b);
                            if !self.is_misspelled(&sugg1a)
                                && !self.is_misspelled(&sugg1b)
                                && self.accept_suggestion(&format!("{sugg1a} {sugg1b}"))
                            {
                                self.push_wrong_split_match(
                                    &mut matches,
                                    &sugg1a,
                                    &sugg1b,
                                    byte(prev_pos),
                                    byte(len + clean_word.encode_utf16().count()),
                                );
                            }
                            // Java has no length guard; a single-char word can
                            // never reach here (single letters are not checked).
                            let mut chars = clean_word.chars();
                            if let Some(first) = chars.next() {
                                let sugg2a = format!("{prev_word}{first}");
                                let sugg2b = cut_off_dot(chars.as_str());
                                if !self.is_misspelled(&sugg2a)
                                    && !self.is_misspelled(&sugg2b)
                                    && self.accept_suggestion(&format!("{sugg2a} {sugg2b}"))
                                {
                                    self.push_wrong_split_match(
                                        &mut matches,
                                        &sugg2a,
                                        &sugg2b,
                                        byte(prev_pos),
                                        byte(len + clean_word.encode_utf16().count()),
                                    );
                                }
                            }
                        }
                    }
                }
                let from = len + dash_corr;
                let to = len + clean_word.encode_utf16().count();
                let clean = clean_word[dash_corr..].to_string();
                let suggestions = self.calc_suggestions(word, &clean);
                if suggestions.iter().any(|s| s.value == clean) {
                    prev_start_pos = Some(len + dash_corr);
                    len += word_utf16 + 1;
                    continue;
                }
                let message =
                    self.message_for(&clean, suggestions.first().map(|s| s.value.as_str()));
                matches.push(self.new_rule_match(
                    byte(from),
                    byte(to),
                    &message,
                    suggestions.as_ref().clone(),
                ));
            }
            prev_start_pos = Some(len + dash_corr);
            len += word_utf16 + 1;
        }
        self.remove_gender_compound_matches(&sentence_text, sentence_offset, matches)
    }

    /// `GermanSpellerRule.removeGenderCompoundMatches`: drop matches covered
    /// by gender-gap forms (`Jurist:innenausbildung`), file names and
    /// `@`-mentions whose `*`/`:`/`_`-free form is spelled correctly.
    fn remove_gender_compound_matches(
        &self,
        sentence_text: &str,
        sentence_offset: usize,
        matches: Vec<Match>,
    ) -> Vec<Match> {
        let mut filtered = matches;
        let mut pos = 0usize;
        while let Some(m) = GENDER_STAR_PATTERN.find_at(sentence_text, pos) {
            let without_separator = m.as_str().replacen(['*', ':', '_'], "", 1);
            if !self.is_misspelled(&without_separator) {
                // UTF-16 vs UTF-8 positions are monotonic; comparing byte
                // ranges here is equivalent to Java's char-index checks.
                let (gen_from, gen_to) = (m.start(), m.end());
                filtered.retain(|k| {
                    let from = k.range.start.saturating_sub(sentence_offset);
                    let to = k.range.end.saturating_sub(sentence_offset);
                    !((gen_from < from && gen_to == to) || (gen_from == from && gen_to > to))
                });
            }
            pos = m.end();
        }
        for pattern in [&*FILE_UNDERLINE_PATTERN, &*MENTION_UNDERLINE_PATTERN] {
            let mut pos = 0usize;
            while let Some(m) = pattern.find_at(sentence_text, pos) {
                filtered.retain(|k| {
                    let from = k.range.start.saturating_sub(sentence_offset);
                    let to = k.range.end.saturating_sub(sentence_offset);
                    !(m.start() <= from && m.end() >= to)
                });
                pos = m.end();
            }
        }
        filtered
    }

    /// `HunspellRule.getSentenceTextWithoutUrlsAndImmunizedTokens`: the
    /// sentence text with URLs/e-mails/immunized/ignored-by-speller tokens
    /// blanked (same length) and the German `isQuotedCompound` override.
    fn sentence_text_for_speller(&self, tokens: &[AnalyzedTokenReadings]) -> String {
        // `Rule.getSentenceWithImmunization`: the rule's IGNORE_SPELLING
        // anti-patterns (multi-token ignore-list entries) mark every matched
        // token as ignored by the speller.
        let view: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .map(|(i, _)| i)
            .collect();
        let mut ignored = vec![false; tokens.len()];
        for (start, &token_idx) in view.iter().enumerate() {
            let surface = tokens[token_idx].surface();
            let Some(phrases) = self.ignored_phrases.get(surface) else {
                continue;
            };
            for phrase in phrases {
                let end = start + phrase.len();
                if end <= view.len()
                    && phrase
                        .iter()
                        .zip(&view[start..end])
                        .all(|(part, &idx)| tokens[idx].surface() == part)
                {
                    for &idx in &view[start..end] {
                        ignored[idx] = true;
                    }
                }
            }
        }
        let mut out = String::new();
        for (i, token) in tokens.iter().enumerate().skip(1) {
            let surface = token.surface();
            if ignored[i]
                || token.is_immunized
                || token.is_ignore_spelling
                || is_url(surface)
                || is_email(surface)
                || self.is_quoted_compound(tokens, i)
                || token.has_pos_tag("_english_ignore_")
            {
                if self.is_quoted_compound(tokens, i) {
                    out.push(' ');
                    out.push_str(
                        &surface[surface.chars().next().map(char::len_utf8).unwrap_or(0)..],
                    );
                } else {
                    for _ in 0..surface.encode_utf16().count() {
                        out.push(' ');
                    }
                }
            } else {
                out.push_str(&string_for_speller(surface));
            }
        }
        out
    }

    /// `GermanSpellerRule.isQuotedCompound` ("Spiegel"-Magazin).
    fn is_quoted_compound(&self, tokens: &[AnalyzedTokenReadings], idx: usize) -> bool {
        if idx > 3 && tokens[idx].surface().starts_with('-') {
            let prev = tokens.get(idx - 1).map(|t| t.surface()).unwrap_or("");
            let prev3 = tokens.get(idx - 3).map(|t| t.surface()).unwrap_or("");
            return matches!(prev, "\u{201c}" | "\"") && matches!(prev3, "\u{201e}" | "\"");
        }
        false
    }
}

/// `GermanSpellerRule.GENDER_NEUTRAL_SPECIAL_CHRS_SIN` (find semantics; the
/// Java pattern is `.*[\*:_/]in$` used with `matches()`).
static GENDER_NEUTRAL_SIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[*:_/]in$").unwrap());
/// `replaceFirst("[\\*:_/]in", "in")`.
static REPLACE_GENDER_IN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[*:_/]in").unwrap());
/// `GermanSpellerRule.isCountryOrRegionNomSin`'s `EIG:NOM:SIN.+(COU|GEB|STD|WAT)`
/// (Java `matchesPosTagRegex` is a full match).
static COUNTRY_OR_REGION_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:EIG:NOM:SIN.+(COU|GEB|STD|WAT))$").unwrap());

/// `StringTools.startsWithLowercase`.
fn starts_with_lowercase(word: &str) -> bool {
    word.chars().next().is_some_and(char::is_lowercase)
}

/// `StringTools.startsWithUppercase`.
fn starts_with_uppercase(word: &str) -> bool {
    word.chars().next().is_some_and(char::is_uppercase)
}

/// `GermanSpellerRule`'s `adjSuffix + "(er|es|en|em|e)?"` replacement pattern.
static ADJ_SUFFIX_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("{adjSuffix}(er|es|en|em|e)?")).unwrap());

/// `GermanSpellerRule`'s `partialWord.matches(".+ische?[mnrs]?")`.
static PARTIAL_ADJ_ISCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.+ische?[mnrs]?$").unwrap());

/// `StringUtils.startsWithAny`.
fn starts_with_any(word: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| word.starts_with(prefix))
}

/// `StringUtils.isAllLowerCase` (false for empty strings).
fn is_all_lowercase(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_lowercase())
}

/// Guava `Strings.commonPrefix`.
fn common_prefix(a: &str, b: &str) -> String {
    a.chars()
        .zip(b.chars())
        .take_while(|(x, y)| x == y)
        .map(|(x, _)| x)
        .collect()
}

/// `StringUtils.removeEnd(part, "s")`.
fn remove_trailing_s(part: &str) -> String {
    part.strip_suffix('s').unwrap_or(part).to_string()
}

/// `StringUtils.removeEnd(part, "-")`.
fn remove_trailing_hyphen(part: &str) -> String {
    part.strip_suffix('-').unwrap_or(part).to_string()
}

/// `removeTrailingS(removeTrailingHyphen(part))`.
fn remove_trailing_s_and_hyphen(part: &str) -> String {
    remove_trailing_s(&remove_trailing_hyphen(part))
}

/// `GermanSpellerRule.avoidInfixSAsSingleToken`: append standalone `s` parts
/// to their predecessor.
fn avoid_infix_s_as_single_token(mut parts: Vec<String>) -> Vec<String> {
    let mut indexes: Vec<usize> = parts
        .iter()
        .enumerate()
        .filter(|(_, part)| part.as_str() == "s")
        .map(|(i, _)| i)
        .collect();
    indexes.sort_unstable_by(|a, b| b.cmp(a));
    for index in indexes {
        if index > 0 && index < parts.len() {
            let to_append = parts.remove(index);
            parts[index - 1].push_str(&to_append);
        }
    }
    parts
}

/// `GermanSpellerRule.splitPartsByHyphen` (Java `split("-")` semantics).
fn split_parts_by_hyphen(original_parts: &[String]) -> Vec<String> {
    let mut parts = original_parts.to_vec();
    let mut i = 0usize;
    while i < parts.len() {
        let element = parts[i].clone();
        if element.contains('-') {
            let split_words = java_split_hyphen(&element);
            parts.remove(i);
            for word in split_words.iter().rev() {
                parts.insert(i, word.clone());
            }
            i += split_words.len() - 1;
        }
        i += 1;
    }
    parts
}

/// `GermanSpellerRule.restoreRemovedHyphens`: re-append the hyphens the
/// tokenizer dropped (positions computed over the original word).
fn restore_removed_hyphens(parts: &[String], word: &str) -> Vec<String> {
    let hyphen_positions: Vec<usize> = word
        .char_indices()
        .filter(|(_, c)| *c == '-')
        .map(|(i, _)| i)
        .collect();
    let mut result = Vec::with_capacity(parts.len());
    let mut current_pos = 0usize;
    for token in parts {
        let mut token = token.clone();
        for &hyphen_pos in &hyphen_positions {
            if hyphen_pos >= current_pos && hyphen_pos == current_pos + token.chars().count() {
                token.push('-');
                break;
            }
        }
        current_pos += token.chars().count();
        result.push(token);
    }
    result
}

/// `GermanSpellerRule.isValidPartLength`.
fn is_valid_part_length(parts: &[String]) -> bool {
    let lens: Vec<usize> = parts.iter().map(|p| p.chars().count()).collect();
    if parts.len() == 2 {
        return lens[0] >= 3 && lens[1] >= 4;
    }
    if parts.len() == 3 {
        return lens[0] >= 3 && lens[1] >= 4 && lens[2] >= 4;
    }
    false
}

/// `GermanSpellerRule.isOldSpelling` over the `alt_neu.csv` word set.
fn is_old_spelling(parts: &[String], old_spelling: &HashSet<String>) -> bool {
    for part in parts {
        let cleaned = remove_trailing_s_and_hyphen(part);
        if part.ends_with('s') {
            if old_spelling.contains(&lt_tagger::uppercase_first_char(part))
                || old_spelling.contains(&lt_tagger::uppercase_first_char(&cleaned))
                || old_spelling.contains(&lt_tagger::lowercase_first_char(part))
                || old_spelling.contains(&lt_tagger::lowercase_first_char(&cleaned))
            {
                return true;
            }
        } else if old_spelling.contains(&lt_tagger::uppercase_first_char(&cleaned))
            || old_spelling.contains(&lt_tagger::lowercase_first_char(&cleaned))
        {
            return true;
        }
    }
    false
}

/// `GermanSpellerRule.getWordAfterEnumerationOrNull`.
fn get_word_after_enumeration_or_null(words: &[String], idx: usize) -> Option<&str> {
    for w in words.iter().skip(idx) {
        let word = w.as_str();
        if !(word.ends_with('-')
            || [
                ",",
                "/",
                "&",
                "und",
                "oder",
                "bzw.",
                "beziehungsweise",
                "sowie",
                "statt",
            ]
            .contains(&word)
            || word.trim().is_empty())
        {
            return Some(word);
        }
    }
    None
}

/// Java `String.split("-")` (keeps leading empties, drops trailing ones).
fn java_split_hyphen(s: &str) -> Vec<String> {
    let mut parts: Vec<String> = s.split('-').map(str::to_string).collect();
    while parts.last().is_some_and(|part| part.is_empty()) {
        parts.pop();
    }
    parts
}

/// `GermanSpellerRule.isNeedingFugenS`.
fn is_needing_fugen_s(word: &str) -> bool {
    [
        "tum", "ling", "ion", "tät", "keit", "schaft", "sicht", "ung", "en",
    ]
    .iter()
    .any(|suffix| word.ends_with(suffix))
}

/// Java `AnalyzedTokenReadings.isPosTagUnknown`.
fn is_pos_tag_unknown(r: &AnalyzedTokenReadings) -> bool {
    r.is_pos_tag_unknown
}

fn is_vowel(c: char) -> bool {
    matches!(
        c.to_ascii_lowercase(),
        'a' | 'e' | 'i' | 'o' | 'u' | 'ä' | 'ö' | 'ü'
    )
}

/// Java `StringTools.stringForSpeller`: replace non-BMP characters (UTF-16
/// surrogate pairs) with spaces of the same UTF-16 length.
fn string_for_speller(s: &str) -> String {
    if s.chars().any(|c| c.len_utf16() > 1) {
        let mut out = String::new();
        for c in s.chars() {
            if c.len_utf16() > 1 {
                for _ in 0..c.len_utf16() {
                    out.push(' ');
                }
            } else {
                out.push(c);
            }
        }
        out
    } else {
        s.to_string()
    }
}

/// `HunspellRule.tokenizeText` with the `.aff` `WORDCHARS` (`\u{df}-.`):
/// split at every non-letter char that is not one of `-`/`.` (Java keeps
/// leading/middle empty tokens, drops trailing ones).
fn hunspell_tokens(text: &str) -> Vec<String> {
    // `HunspellRule.tokenizeText` over the `.aff` `WORDCHARS` (`ß-.`), plus
    // the German override in `GermanSpellerRule.init`:
    // `"(" + nonWordPattern + "|(?<=[\\d°])-|-(?=\\d+))"` — a hyphen preceded
    // by a digit/degree sign or followed by a digit splits words, so
    // `Wenera-7` is `Wenera` and `COVID-19-Pandemie` is `COVID`/`19`/
    // `Pandemie`.
    let chars: Vec<char> = text.chars().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    for (i, &c) in chars.iter().enumerate() {
        let split_hyphen = c == '-'
            && ((i > 0 && (chars[i - 1].is_ascii_digit() || chars[i - 1] == '°'))
                || chars.get(i + 1).is_some_and(char::is_ascii_digit));
        if c.is_alphabetic() || c == '.' || (c == '-' && !split_hyphen) {
            current.push(c);
        } else {
            tokens.push(std::mem::take(&mut current));
        }
    }
    tokens.push(current);
    while tokens.last().is_some_and(|t| t.is_empty()) {
        tokens.pop();
    }
    tokens
}

/// `HunspellRule.cutOffDot`: remove a single trailing dot.
fn cut_off_dot(s: &str) -> String {
    s.strip_suffix('.').unwrap_or(s).to_string()
}

/// UTF-16 code-unit offset → UTF-8 byte offset (clamped to the text end).
fn utf16_to_utf8_offset(text: &str, utf16_offset: usize) -> usize {
    let mut units = 0usize;
    for (byte, c) in text.char_indices() {
        if units >= utf16_offset {
            return byte;
        }
        units += c.len_utf16();
    }
    text.len()
}

/// The hyphen-compound check used by the German speller's suggestion
/// filtering (delegates to the shared split helper).
#[allow(dead_code)]
pub fn is_hyphen_compound(word: &str) -> bool {
    word.contains('-') && split_compound(word).len() > 1
}
