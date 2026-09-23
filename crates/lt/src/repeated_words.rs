//! Port of `AbstractRepeatedWordsRule` + `EnglishRepeatedWordsRule`
//! (`EN_REPEATEDWORDS`, `tags="picky"`): lemma repetitions across adjacent
//! sentences with synonym suggestions from `en/rules/synonyms.txt`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use lt_core::{
    AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, Match, Result, Suggestion, TextRange,
};
use lt_pattern::matcher as pm;
use lt_pattern::{PatternToken, Synthesizer};

const RULE_ID: &str = "EN_REPEATEDWORDS";
const DE_RULE_ID: &str = "DE_REPEATEDWORDS";
const DESCRIPTION: &str = "Suggest synonyms for repeated words.";
const MESSAGE: &str = "This word has been used in one of the immediately preceding sentences. Using a synonym could make your text more interesting to read, unless the repetition is intentional.";
const SHORT_MESSAGE: &str = "Style: repeated word";
const CATEGORY_ID: &str = "REPETITIONS_STYLE";
const CATEGORY_NAME: &str = "Repetitions (Style)";
const CATEGORY_NAME_DE: &str = "Wiederholungen (Stil)";
const DESCRIPTION_DE: &str = "Synonyme für wiederholte Wörter.";
const MESSAGE_DE: &str = "Dieses Wort kommt in einem nahe gelegenen vorherigen Satz bereits vor. Verwenden Sie ein Synonym, um Ihren Text abwechslungsreicher zu gestalten, außer die Wiederholung ist beabsichtigt.";
const SHORT_MESSAGE_DE: &str = "Stil: Wortwiederholung";
/// `AbstractRepeatedWordsRule.maxWordsDistance()`
const MAX_WORDS_DISTANCE: i64 = 150;

fn single_punct_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^\p{P}$").unwrap());
    &RE
}

/// One `word/postag/chunk=syn;syn` entry of `synonyms.txt`.
#[derive(Debug, Clone)]
struct SynonymsData {
    synonyms: Vec<String>,
    postag: Option<String>,
    chunk: Option<String>,
}

/// Per-language behaviour of `AbstractRepeatedWordsRule` subclasses.
#[derive(Clone, Copy)]
pub struct RepeatedWordsConfig {
    pub rule_id: &'static str,
    pub description: &'static str,
    pub message: &'static str,
    pub short_message: &'static str,
    pub category_id: &'static str,
    pub category_name: &'static str,
    /// `tags="picky"` (the English rule; the German one is default-on)
    pub picky: bool,
    /// German `isException` uses `EIG:` instead of `NNP` and `toId` maps umlauts
    pub german: bool,
    /// Spanish `isException` (`NP...`/`_english_ignore_`) and `adjustPostag`
    pub spanish: bool,
    /// French `isException` (`Z*`) and `adjustPostag`
    pub french: bool,
}

pub struct RepeatedWordsRule {
    words: HashMap<String, SynonymsData>,
    antipatterns: Vec<Arc<pm::CompiledPattern>>,
    config: RepeatedWordsConfig,
}

impl RepeatedWordsRule {
    /// Load `en/rules/synonyms.txt` and compile the English code-level
    /// antipatterns (`cacheAntiPatterns(ANTI_PATTERNS)`).
    pub fn english(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("en/rules/synonyms.txt");
        let words = load_words(&path)?;
        let antipatterns = compile_antipatterns();
        Ok(Self {
            words,
            antipatterns,
            config: RepeatedWordsConfig {
                rule_id: RULE_ID,
                description: DESCRIPTION,
                message: MESSAGE,
                short_message: SHORT_MESSAGE,
                category_id: CATEGORY_ID,
                category_name: CATEGORY_NAME,
                picky: true,
                german: false,
                spanish: false,
                french: false,
            },
        })
    }

    /// `GermanRepeatedWordsRule`: `de/rules/synonyms.txt`, no antipatterns,
    /// default-on (no `Tag.picky`).
    pub fn german(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("de/rules/synonyms.txt");
        let words = load_words(&path)?;
        Ok(Self {
            words,
            antipatterns: Vec::new(),
            config: RepeatedWordsConfig {
                rule_id: DE_RULE_ID,
                description: DESCRIPTION_DE,
                message: MESSAGE_DE,
                short_message: SHORT_MESSAGE_DE,
                category_id: CATEGORY_ID,
                category_name: CATEGORY_NAME_DE,
                picky: false,
                german: true,
                spanish: false,
                french: false,
            },
        })
    }

    /// `SpanishRepeatedWordsRule` (`ES_REPEATEDWORDS`, `Tag.picky`):
    /// `es/rules/synonyms.txt` + the five code-level antipatterns.
    pub fn spanish(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("es/rules/synonyms.txt");
        let words = load_words(&path)?;
        let antipatterns = spanish_antipatterns()
            .iter()
            .flat_map(|tokens| pm::compile_patterns(tokens, None, None).unwrap_or_default())
            .map(Arc::new)
            .collect();
        Ok(Self {
            words,
            antipatterns,
            config: RepeatedWordsConfig {
                rule_id: "ES_REPEATEDWORDS",
                description: "Sinónimos para palabras repetidas.",
                message: "Esta palabra ya ha aparecido en una de las frases inmediatamente anteriores. Puede usar un sinónimo para hacer más interesante el texto, excepto si la repetición es intencionada.",
                short_message: "Estilo: palabra repetida",
                category_id: CATEGORY_ID,
                category_name: "Repeticiones",
                picky: true,
                german: false,
                spanish: true,
                french: false,
            },
        })
    }

    /// `FrenchRepeatedWordsRule` (`FR_REPEATEDWORDS`, default-on):
    /// `fr/rules/synonyms.txt`, no code-level antipatterns, the French
    /// `adjustPostag`/`isException`.
    pub fn french(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("fr/rules/synonyms.txt");
        let words = load_words(&path)?;
        Ok(Self {
            words,
            antipatterns: Vec::new(),
            config: RepeatedWordsConfig {
                rule_id: "FR_REPEATEDWORDS",
                description: "Synonymes de mots répétés.",
                message: "Ce mot apparaît déjà dans l'une des phrases précédant immédiatement celle-ci. Utilisez un synonyme pour apporter plus de variété à votre texte, excepté si la répétition est intentionnelle.",
                short_message: "Style : Mot répété",
                category_id: CATEGORY_ID,
                category_name: "Répétitions (Style)",
                picky: false,
                german: false,
                spanish: false,
                french: true,
            },
        })
    }

    /// `AbstractRepeatedWordsRule.match(List<AnalyzedSentence>)`.
    pub fn check(
        &self,
        sentences: &[AnalyzedSentence],
        synth: Option<&dyn Synthesizer>,
    ) -> Vec<Match> {
        let mut matches = Vec::new();
        let mut word_number: i64 = 0;
        let mut words_last_seen: HashMap<String, i64> = HashMap::new();
        let mut pos: usize = 0;
        let mut prev_sentence_length: usize = 0;
        for sentence in sentences {
            let immunized = self.immunize(sentence, synth);
            let tokens = immunized.tokens_without_whitespace();
            pos += prev_sentence_length;
            prev_sentence_length = sentence.text.len();
            // ignore sentences not ending in a period (Java: . ! ?)
            let Some(last) = tokens.last() else {
                continue;
            };
            if !matches!(last.surface(), "." | "!" | "?") {
                continue;
            }
            let mut sent_start = true;
            let mut lemmas_in_sentence: Vec<String> = Vec::new();
            let mut i: isize = -1;
            for atrs in &tokens {
                if atrs.is_immunized {
                    continue;
                }
                let token = atrs.surface();
                if !token.is_empty() {
                    word_number += 1;
                }
                let is_capitalized = lt_tagger::is_capitalized_word(token);
                let is_all_uppercase = lt_tagger::is_all_uppercase(token);
                i += 1;
                let token_index = i as usize;
                let is_exception = token.is_empty() || {
                    let current = tokens.get(token_index).copied().unwrap_or(atrs);
                    // `EnglishRepeatedWordsRule.isException`
                    is_all_uppercase
                        || (is_capitalized && !sent_start)
                        || if self.config.spanish {
                            current.has_pos_tag_starting_with("NP")
                                || current.has_pos_tag("_english_ignore_")
                        } else if self.config.french {
                            current.has_pos_tag_starting_with("Z")
                        } else {
                            current.has_pos_tag_starting_with(if self.config.german {
                                "EIG:"
                            } else {
                                "NNP"
                            })
                        }
                };
                if sent_start && !token.is_empty() && !single_punct_re().is_match(token) {
                    sent_start = false;
                }
                if is_exception {
                    continue;
                }
                let mut lemmas: Vec<Option<String>> = Vec::new();
                for atr in &atrs.readings {
                    let lemma = atr.stem.clone();
                    lemmas.push(lemma.clone());
                    let seen_in_word_position = lemma
                        .as_deref()
                        .and_then(|l| words_last_seen.get(l))
                        .copied();
                    if let (Some(seen), Some(lemma)) = (seen_in_word_position, lemma.as_deref()) {
                        if !lemmas_in_sentence.iter().any(|l| l == lemma)
                            && (word_number - seen) <= MAX_WORDS_DISTANCE
                        {
                            let Some(data) = self.words.get(lemma) else {
                                continue;
                            };
                            let mut create_match = true;
                            if let Some(postag) = &data.postag {
                                match atr.pos_tag.as_deref() {
                                    Some(tag) if pos_tag_matches(postag, tag) => {}
                                    _ => create_match = false,
                                }
                            }
                            if let Some(chunk) = &data.chunk {
                                if !chunk_regex_matches(atrs, chunk) {
                                    create_match = false;
                                }
                            }
                            if create_match {
                                let mut suggestions = Vec::new();
                                for replacement_lemma in &data.synonyms {
                                    let analyzed = AnalyzedToken::new(
                                        token,
                                        Some(replacement_lemma.clone()),
                                        atr.pos_tag.clone(),
                                    );
                                    let pos_tag = atr.pos_tag.as_deref().unwrap_or("");
                                    let pos_tag = if self.config.spanish {
                                        adjust_postag_es(pos_tag)
                                    } else if self.config.french {
                                        adjust_postag_fr(pos_tag)
                                    } else {
                                        pos_tag.to_string()
                                    };
                                    let mut replacements = synth
                                        .map(|s| s.synthesize(&analyzed, &pos_tag, true))
                                        .unwrap_or_default();
                                    if replacements.is_empty() {
                                        replacements = vec![replacement_lemma.clone()];
                                    }
                                    for r in replacements {
                                        let value = if is_all_uppercase {
                                            r.to_uppercase()
                                        } else if is_capitalized {
                                            lt_tagger::uppercase_first_char(&r)
                                        } else {
                                            r
                                        };
                                        let suggestion = Suggestion {
                                            value,
                                            short_description: None,
                                        };
                                        if !suggestions.contains(&suggestion) {
                                            suggestions.push(suggestion);
                                        }
                                    }
                                }
                                let rule_match = Match::new(
                                    self.config.rule_id,
                                    Option::<String>::None,
                                    self.config.message,
                                    Some(self.config.short_message.to_string()),
                                    TextRange::new(pos + atrs.start_pos, pos + atrs.end_pos()),
                                    suggestions,
                                    self.config.category_id,
                                    self.config.category_name,
                                )
                                .with_specific_rule_id(format!(
                                    "{}_{}",
                                    self.config.rule_id,
                                    to_id(lemma, self.config.german)
                                ))
                                .with_metadata(self.config.description, "style", -1)
                                .with_picky(self.config.picky);
                                matches.push(rule_match);
                                break;
                            }
                        }
                    }
                }
                // count even if postag/chunk don't match
                for lemma in lemmas.into_iter().flatten() {
                    if self.words.contains_key(&lemma) {
                        words_last_seen.insert(lemma.clone(), word_number);
                        lemmas_in_sentence.push(lemma);
                    }
                }
            }
        }
        matches
    }

    /// `Rule.getSentenceWithImmunization`: apply the rule's antipatterns
    /// (IMMUNIZE) to a copy of the sentence, over the token view.
    fn immunize(
        &self,
        sentence: &AnalyzedSentence,
        synth: Option<&dyn Synthesizer>,
    ) -> AnalyzedSentence {
        let view = sentence.tokens_without_whitespace();
        let mut immune = vec![false; view.len()];
        for ap in &self.antipatterns {
            for m in pm::find_matches_with_synth(ap, &[], &[], &view, None, None, synth) {
                if immune.is_empty() {
                    break;
                }
                let last = m.end_tok().min(immune.len() - 1);
                for flag in immune.iter_mut().take(last + 1).skip(m.start_tok()) {
                    *flag = true;
                }
            }
        }
        let mut immunized = sentence.clone();
        let mut view_index = 0usize;
        for token in &mut immunized.tokens {
            if !token.is_whitespace
                || token.is_sentence_start
                || token.is_sentence_end
                || token.is_paragraph_end
            {
                if immune.get(view_index).copied().unwrap_or(false) {
                    token.is_immunized = true;
                }
                view_index += 1;
            }
        }
        immunized
    }
}

/// `StringTools.toId(lemma, language)`; the German short code replaces the
/// umlauts before the non-character mapping.
fn to_id(input: &str, german: bool) -> String {
    let mut normalized = input
        .to_uppercase()
        .trim()
        .replace(' ', "_")
        .replace('\'', "_Q_");
    if german {
        normalized = normalized
            .replace('Ä', "AE")
            .replace('Ü', "UE")
            .replace('Ö', "OE");
    }
    normalized
        .chars()
        .map(|c| match c {
            'A'..='Z' | '\u{00c0}'..='\u{00d6}' | '\u{00d8}'..='\u{00de}' => c,
            _ => '_',
        })
        .collect()
}

/// Interned full-match regex for `synonyms.txt` patterns: the same handful
/// of patterns is re-tested for every token, and a fresh `Regex::new` per
/// call showed up prominently in the steady-state profile. Compile failures
/// are cached as `None` (Java `String.matches` → no match).
fn cached_full_match_regex(pattern: &str) -> Option<std::sync::Arc<regex::Regex>> {
    static CACHE: std::sync::OnceLock<
        Mutex<HashMap<String, Option<std::sync::Arc<regex::Regex>>>>,
    > = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap();
    cache.get(pattern).cloned().unwrap_or_else(|| {
        let compiled = regex::Regex::new(&format!("^(?:{pattern})$"))
            .ok()
            .map(std::sync::Arc::new);
        cache.insert(pattern.to_string(), compiled.clone());
        compiled
    })
}

/// `PatternToken` pos matching is a full regex match; the `synonyms.txt`
/// tags are regexes even without `postag_regexp` (Java `String.matches`).
fn pos_tag_matches(pattern: &str, tag: &str) -> bool {
    cached_full_match_regex(pattern)
        .map(|re| re.is_match(tag))
        .unwrap_or(false)
}

/// `AnalyzedTokenReadings.matchesChunkRegex`: full match on any chunk tag.
fn chunk_regex_matches(token: &AnalyzedTokenReadings, chunk_regex: &str) -> bool {
    let Some(re) = cached_full_match_regex(chunk_regex) else {
        return false;
    };
    token.chunk_tags.iter().any(|c| re.is_match(c))
}

/// `AbstractRepeatedWordsRule.loadWords` (`synonyms.txt`).
fn load_words(path: &Path) -> Result<HashMap<String, SynonymsData>> {
    let text = lt_data::fs::read_to_string(path)
        .map_err(|e| lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display())))?;
    let mut map: HashMap<String, SynonymsData> = HashMap::new();
    for raw in text.lines() {
        // `HASH_PATTERN = #.*` with replaceFirst("")
        let line = raw.find('#').map(|i| &raw[..i]).unwrap_or(raw).trim();
        if line.is_empty() {
            continue;
        }
        let main_parts = java_split(line, '=');
        let (parts, word, postag, chunk) = if main_parts.len() == 2 {
            let parts = java_split(main_parts[1], ';');
            let word_pos_chunk = java_split(main_parts[0], '/');
            let word = word_pos_chunk[0].to_string();
            let postag = word_pos_chunk.get(1).map(|s| s.to_string());
            let chunk = word_pos_chunk.get(2).map(|s| s.to_string());
            (parts, word, postag, chunk)
        } else if main_parts.len() == 1 {
            (java_split(line, ';'), String::new(), None, None)
        } else {
            return Err(lt_core::CoreError::Data(format!(
                "Format error in file {}, line: {line}",
                path.display()
            )));
        };
        if (word.is_empty() && parts.len() < 2) || (!word.is_empty() && parts.is_empty()) {
            return Err(lt_core::CoreError::Data(format!(
                "Format error in file {}, line: {line}",
                path.display()
            )));
        }
        let mut insert = |key: String, synonyms: Vec<String>| -> Result<()> {
            if map.contains_key(&key) {
                return Err(lt_core::CoreError::Data(format!(
                    "Word found in more than one line. \"{key}\" in line: {line}"
                )));
            }
            map.insert(
                key,
                SynonymsData {
                    synonyms,
                    postag: postag.clone(),
                    chunk: chunk.clone(),
                },
            );
            Ok(())
        };
        if !word.is_empty() {
            insert(word, parts.iter().map(|s| s.to_string()).collect())?;
        } else {
            for key in &parts {
                let values: Vec<String> = parts
                    .iter()
                    .filter(|v| v != &key)
                    .map(|v| v.to_string())
                    .collect();
                insert((*key).to_string(), values)?;
            }
        }
    }
    Ok(map)
}

/// Java `String.split(regex)`: trailing empty strings are removed.
fn java_split(s: &str, sep: char) -> Vec<&str> {
    let mut parts: Vec<&str> = s.split(sep).collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// `PatternRuleBuilderHelper` equivalents for the English antipatterns.
fn token(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        in_marker: true,
        ..Default::default()
    }
}

fn cs_token(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        case_sensitive: true,
        in_marker: true,
        ..Default::default()
    }
}

fn inflected_cs_token(text: &str) -> PatternToken {
    PatternToken {
        inflected: true,
        ..cs_token(text)
    }
}

fn token_regex(text: &str) -> PatternToken {
    PatternToken {
        text: Some(text.to_string()),
        regexp: true,
        in_marker: true,
        ..Default::default()
    }
}

fn pos(pos_tag: &str) -> PatternToken {
    PatternToken {
        postag: Some(pos_tag.to_string()),
        in_marker: true,
        ..Default::default()
    }
}

fn pos_regex(pos_tag: &str) -> PatternToken {
    PatternToken {
        postag: Some(pos_tag.to_string()),
        postag_regexp: true,
        in_marker: true,
        ..Default::default()
    }
}

fn min(mut t: PatternToken, value: i32) -> PatternToken {
    t.min = Some(value);
    t
}

fn skip(mut t: PatternToken, value: i32) -> PatternToken {
    t.skip = Some(value);
    t
}

/// `EnglishRepeatedWordsRule.ANTI_PATTERNS` (order matters for
/// immunization; each list is a separate antipattern).
fn antipattern_defs() -> Vec<Vec<PatternToken>> {
    vec![
        // "I still need -> require to sign in"
        vec![inflected_cs_token("need"), token("to")],
        // "solve the problem" is a unique collocation
        vec![
            skip(token_regex("solve(s|d|ing)?"), 3),
            token_regex("problems?"),
        ],
        // "No problem, I'm not in a rush."
        vec![
            pos_regex("SENT_START|PCT"),
            token("no"),
            token("problem"),
            pos("PCT"),
        ],
        // "math/word problem"
        vec![token_regex("math|word"), token_regex("problems?")],
        // "doesn't apply to the group as a whole"
        vec![
            token_regex("as"),
            token_regex("a"),
            token_regex("whole"),
        ],
        vec![
            token("more"),
            token("often"),
            token("than"),
            token("not"),
        ],
        vec![token("often"), token("times")],
        vec![
            token_regex(
                "details?|facts?|it|journals?|questions?|research|results?|study|studies|this|these|those|which",
            ),
            min(pos("RB"), 0),
            inflected_cs_token("suggest"),
        ],
        // "form in the bloodstream"
        vec![
            inflected_cs_token("form"),
            pos_regex("IN|PCT|RP|TO|SENT_END"),
        ],
        vec![
            skip(
                token_regex("bonds?|crystals?|ions?|rocks?|.*valence"),
                10,
            ),
            inflected_cs_token("form"),
        ],
        vec![
            skip(token_regex("form(s|ed|ing)?"), 10),
            token_regex("bonds?|crystals?|ions?|rocks?|.*valence"),
        ],
        vec![token("interesting"), token_regex("facts?|things?")],
        vec![
            token("several"),
            token_regex("hundreds?|thousands?|millions?"),
        ],
        vec![token("must"), token("be"), token("nice")],
        vec![token("nice"), token("day")],
        vec![
            token("nice"),
            token("to"),
            min(token("meet"), 0),
            pos_regex("PRP_O.*"),
        ],
        // nice and plump
        vec![
            inflected_cs_token("be"),
            token("nice"),
            token("and"),
            pos("JJ"),
            pos_regex("PCT|SENT_END"),
        ],
        // the proposed agreement
        vec![
            pos_regex("P?DT|PRP$.*"),
            token("proposed"),
            pos_regex("N.*"),
        ],
        vec![
            inflected_cs_token("propose"),
            token_regex("to|marriage"),
        ],
        vec![token("too"), token("literally")],
        vec![
            token("literally"),
            token("and"),
            token("figuratively"),
        ],
        vec![token("literally"), token("everything")],
        vec![token("literally"), pos_regex("PCT|SENT_END")],
        // "Or maybe it's because I have eyes that see!"
        vec![pos_regex("CC"), token("maybe")],
    ]
}

/// `SpanishRepeatedWordsRule.adjustPostag` (`StringUtils.replaceOnce`).
/// `FrenchRepeatedWordsRule.adjustPostag` (`StringUtils.replaceOnce`).
fn adjust_postag_fr(postag: &str) -> String {
    for (suffix, replacement) in [
        ("e sp", ". .*"),
        ("m s", "[me] sp?"),
        ("f s", "[fe] sp?"),
        ("m p", "[me] s?p"),
        ("f p", "[fe] s?p"),
        ("e s", "[me] sp?"),
        ("e p", "[me] s?p"),
        ("m sp", "[me] s?p?"),
        ("f sp", "[fe] s?p?"),
    ] {
        if postag.ends_with(suffix) {
            return postag.replacen(suffix, replacement, 1);
        }
    }
    postag.to_string()
}

fn adjust_postag_es(postag: &str) -> String {
    for (from, to) in [
        ("CN", ".."),
        ("MS", "[MC][SN]"),
        ("FS", "[FC][SN]"),
        ("MP", "[MC][PN]"),
        ("FP", "[FC][PN]"),
        ("CS", "[MC][SN]"),
        ("CP", "[MC][PN]"),
        ("MN", "[MC][SPN]"),
        ("FN", "[FC][SPN]"),
    ] {
        if let Some(pos) = postag.find(from) {
            let mut out = postag.to_string();
            out.replace_range(pos..pos + from.len(), to);
            return out;
        }
    }
    postag.to_string()
}

fn cs_regex(text: &str) -> PatternToken {
    PatternToken {
        case_sensitive: true,
        regexp: true,
        ..token(text)
    }
}

/// `SpanishRepeatedWordsRule.ANTI_PATTERNS`.
fn spanish_antipatterns() -> Vec<Vec<PatternToken>> {
    vec![
        vec![token("también"), cs_regex(".+")],
        vec![cs_regex(".+"), token("también")],
        vec![cs_regex("[Aa]ntes|[Dd]espués"), cs_regex("de|del")],
        vec![cs_regex("[Tt]ema|TEMA"), cs_regex("\\d+|[IXVC]+")],
        vec![cs_regex("[Aa]sí"), token("que")],
    ]
}

fn compile_antipatterns() -> Vec<Arc<pm::CompiledPattern>> {
    antipattern_defs()
        .iter()
        .flat_map(|tokens| pm::compile_patterns(tokens, None, None).unwrap_or_default())
        .map(Arc::new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    #[test]
    fn parses_java_splits() {
        assert_eq!(java_split("a=b", '='), vec!["a", "b"]);
        assert_eq!(java_split("a=", '='), vec!["a"]);
        assert_eq!(java_split("a;b;", ';'), vec!["a", "b"]);
        assert_eq!(
            java_split("several/JJ/.*-NP.*", '/'),
            vec!["several", "JJ", ".*-NP.*"]
        );
    }

    #[test]
    fn loads_synonyms() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !dir.join("en/rules/synonyms.txt").lt_exists() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let rule = RepeatedWordsRule::english(&dir).unwrap();
        assert_eq!(
            rule.words["problem"].synonyms,
            vec!["issue", "concern", "difficulty"]
        );
        assert_eq!(rule.words["nice"].postag.as_deref(), Some("JJ"));
        assert_eq!(rule.words["nice"].chunk.as_deref(), Some(".-(ADJP|NP).*"));
        assert_eq!(rule.words["need"].postag.as_deref(), Some("VB.*"));
        assert!(rule.words["need"].synonyms == vec!["require"]);
    }

    #[test]
    fn compiles_antipatterns() {
        let antipatterns = compile_antipatterns();
        assert_eq!(antipatterns.len(), antipattern_defs().len());
        assert!(antipatterns.len() >= 24);
    }
}
