//! Port of `AbstractCompoundRule` + `CompoundRuleData` and the English
//! `CompoundRule` (`EN_COMPOUNDS`): compounds listed in `compounds.txt` must
//! not be written as separate words.

use std::collections::{HashMap, HashSet};

use crate::en::spelling::SpellingRule;
use std::path::Path;
use std::sync::Arc;

use lt_core::{AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};
use lt_pattern::matcher as pm;
use lt_pattern::PatternToken;

/// `AbstractCompoundRule.MAX_TERMS`
const MAX_TERMS: usize = 5;

const WITH_HYPHEN_MESSAGE: &str = "This word is normally spelled with a hyphen.";
const WITHOUT_HYPHEN_MESSAGE: &str = "This word is normally spelled as one.";
const WITH_OR_WITHOUT_HYPHEN_MESSAGE: &str =
    "This expression is normally spelled as one or with a hyphen.";
const SHORT_MESSAGE: &str = "Compound";

fn whitespace_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"[\t\n\x0B\f\r ]+").unwrap());
    &RE
}

fn dashes_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"--+").unwrap());
    &RE
}

/// `CompoundRuleData`: the sets loaded from `compounds.txt`.
pub struct CompoundData {
    incorrect: HashSet<String>,
    joined: HashSet<String>,
    joined_lowercase: HashSet<String>,
    dash: HashSet<String>,
    has_digit_patterns: bool,
}

impl CompoundData {
    pub fn from_path(path: &Path) -> Result<Self> {
        Self::from_paths(&[path.to_path_buf()], &|line| vec![line.to_string()])
    }

    /// Load several compound files; `expand` is the language's
    /// `LineExpander` (the Swiss rule expands `ß` → `ss`).
    pub fn from_paths(
        paths: &[std::path::PathBuf],
        expand: &dyn Fn(&str) -> Vec<String>,
    ) -> Result<Self> {
        let mut data = Self {
            incorrect: HashSet::new(),
            joined: HashSet::new(),
            joined_lowercase: HashSet::new(),
            dash: HashSet::new(),
            has_digit_patterns: false,
        };
        for path in paths {
            let text = lt_data::fs::read_to_string(path).map_err(|e| {
                lt_core::CoreError::Data(format!("cannot read {}: {e}", path.display()))
            })?;
            for raw in text.lines() {
                if raw.is_empty() || raw.starts_with('#') {
                    continue;
                }
                let line = match raw.find('#') {
                    Some(idx) => raw[..idx].trim(),
                    None => raw.trim(),
                };
                for expanded in expand(line) {
                    let exp_line = expanded.replace('-', " ");
                    data.add_line(&exp_line)?;
                }
            }
        }
        Ok(data)
    }

    fn add_line(&mut self, exp_line: &str) -> Result<()> {
        let parts: Vec<&str> = exp_line.split(' ').collect();
        if parts.len() == 1 {
            return Err(lt_core::CoreError::Data(format!(
                "Not a compound: {exp_line}"
            )));
        }
        if parts.len() > MAX_TERMS {
            return Err(lt_core::CoreError::Data(format!(
                "Too many compound parts: {exp_line}, maximum allowed: {MAX_TERMS}"
            )));
        }
        if self.incorrect.contains(&exp_line.to_lowercase()) {
            return Err(lt_core::CoreError::Data(format!(
                "Duplicated word: {exp_line}"
            )));
        }
        let stem = match exp_line.chars().last() {
            Some('+') | Some('*') | Some('?') | Some('$') => &exp_line[..exp_line.len() - 1],
            _ => exp_line,
        };
        match exp_line.chars().last() {
            Some('+') => {
                self.joined.insert(stem.to_string());
            }
            Some('*') => {
                self.dash.insert(stem.to_string());
            }
            Some('?') => {
                self.joined.insert(stem.to_string());
                self.joined_lowercase.insert(stem.to_string());
            }
            Some('$') => {
                self.joined.insert(stem.to_string());
                self.dash.insert(stem.to_string());
                self.joined_lowercase.insert(stem.to_string());
            }
            _ => {
                self.joined.insert(stem.to_string());
                self.dash.insert(stem.to_string());
            }
        }
        if stem.contains("\\d") {
            self.has_digit_patterns = true;
        }
        self.incorrect.insert(stem.to_string());
        Ok(())
    }
}

/// One entry of the sliding window (`ArrayDeque<AnalyzedTokenReadings>`).
struct QueueToken {
    surface: String,
    whitespace_before: bool,
    is_sent_start: bool,
    is_immunized: bool,
    start_pos: usize,
    end_pos: usize,
}

/// `AbstractCompoundRule` messages/id/category (per-language subclasses).
struct CompoundConfig {
    rule_id: &'static str,
    description: String,
    with_hyphen_message: &'static str,
    without_hyphen_message: &'static str,
    with_or_without_hyphen_message: &'static str,
    short_message: Option<&'static str>,
    category_id: &'static str,
    category_name: &'static str,
    /// `useSubRuleSpecificIds()` (English; German does not)
    sub_rule_specific_ids: bool,
    /// `StringTools.toId(..., language)` German short-code behavior
    to_id_german: bool,
    /// `mergeCompound` override (Portuguese post-reform digraphs)
    merge: fn(&str, bool) -> String,
    /// `setLocQualityIssueType`
    issue_type: &'static str,
}

pub struct CompoundRule {
    data: CompoundData,
    is_misspelled: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    config: CompoundConfig,
    /// `CompoundRule.getAntiPatterns()` (IMMUNIZE disambiguation patterns)
    anti_patterns: Vec<Arc<pm::CompiledPattern>>,
}

/// `GermanCompoundRule.ANTI_PATTERNS`.
fn german_anti_patterns() -> Vec<Vec<PatternToken>> {
    let token = |s: &str| PatternToken {
        text: Some(s.to_string()),
        ..Default::default()
    };
    let token_re = |s: &str| PatternToken {
        text: Some(s.to_string()),
        regexp: true,
        ..Default::default()
    };
    let skip = |mut t: PatternToken, value: i32| {
        t.skip = Some(value);
        t
    };
    vec![
        vec![token_re("an|um"), token("die"), token_re("\\d+")],
        vec![skip(token_re("von|vom"), 5), token("aus"), token("gedacht")],
        vec![
            token_re("rund|etwa|zirka|cirka|ungefähr|annähernd|grob|wohl|gegen|schätzungsweise"),
            token_re("\\d+"),
        ],
        vec![token("ca"), token("."), token_re("\\d+")],
        vec![token("Kung"), token("Fu"), token_re("Panda|Fighting")],
        vec![token("Harlem"), token("Gospel"), token("Singers")],
        vec![
            token("Always"),
            token("on"),
            token_re("my|your|the|an?|their"),
        ],
        vec![
            token_re("sich|uns|ihm|ihr|mir|euch"),
            token("selbst"),
            token_re("gerecht.*"),
        ],
    ]
}

/// `CompoundRule.ANTI_PATTERNS` (PatternTokenBuilder terms). Text tokens are
/// case-insensitive, `tokenRegex` sets `regexp`, `pos`/`posRegex` set the
/// postag (regexp) test.
fn english_anti_patterns() -> Vec<Vec<PatternToken>> {
    let token = |s: &str| PatternToken {
        text: Some(s.to_string()),
        ..Default::default()
    };
    let token_re = |s: &str| PatternToken {
        text: Some(s.to_string()),
        regexp: true,
        ..Default::default()
    };
    let pos = |s: &str| PatternToken {
        postag: Some(s.to_string()),
        ..Default::default()
    };
    let pos_re = |s: &str| PatternToken {
        postag: Some(s.to_string()),
        postag_regexp: true,
        ..Default::default()
    };
    vec![
        vec![token_re("['’`´‘]"), token("re")],
        vec![
            pos_re("SENT_START|CC|PCT"),
            token_re("we|you|they|I|s?he|it"),
            token("well"),
            pos_re("VB.*"),
        ],
        vec![token("how"), token("well"), pos_re("VB.*")],
        vec![token_re("and|&"), token("co")],
        vec![token("power"), token("off"), token("key")],
        vec![token("see"), token("saw"), token("seen")],
        vec![token("forward"), token("looking"), pos_re("IN|TO")],
        vec![token("store"), token("front"), token_re("doors?")],
        vec![
            token("from"),
            token("surface"),
            token("to"),
            token("surface"),
        ],
        vec![token_re("senior|junior"), token("year"), token("end")],
        vec![token("under"), token("investment"), token("banking")],
        vec![
            token("spring"),
            token_re("cleans?|cleaned|cleaning"),
            token_re("up|the|my|our|his|her"),
        ],
        vec![token_re("series?"), token_re("a")],
        vec![token("hard"), token("time"), pos("VBG")],
        vec![token("first"), token_re("ever"), token_re("green")],
        vec![
            token_re(".+"),
            token("."),
            token_re("(com|io|de|nl|co|net|org|es)"),
        ],
    ]
}

impl CompoundRule {
    pub fn english(data_dir: &Path, spelling: Arc<SpellingRule>) -> Result<Self> {
        let data = CompoundData::from_path(&data_dir.join("en/words/compounds.txt"))?;
        let mut anti_patterns = Vec::new();
        for pattern in english_anti_patterns() {
            let compiled = pm::compile_pattern(&pattern, None, None)
                .map_err(|e| lt_core::CoreError::Data(format!("EN_COMPOUNDS antipattern: {e}")))?;
            anti_patterns.push(Arc::new(compiled));
        }
        Ok(Self {
            data,
            is_misspelled: {
                let spelling = Arc::clone(&spelling);
                Arc::new(move |word: &str| spelling.is_misspelled(word))
            },
            config: CompoundConfig {
                rule_id: "EN_COMPOUNDS",
                description: "Hyphenated words: $match".to_string(),
                with_hyphen_message: WITH_HYPHEN_MESSAGE,
                without_hyphen_message: WITHOUT_HYPHEN_MESSAGE,
                with_or_without_hyphen_message: WITH_OR_WITHOUT_HYPHEN_MESSAGE,
                short_message: Some(SHORT_MESSAGE),
                category_id: "MISC",
                category_name: "Miscellaneous",
                sub_rule_specific_ids: true,
                to_id_german: false,
                merge: merge_compound,
                issue_type: "misspelling",
            },
            anti_patterns,
        })
    }

    /// `GermanCompoundRule` / `SwissCompoundRule`: `de/compounds.txt` +
    /// `de/compound-cities.txt` (the Swiss variant expands `ß` → `ss`),
    /// German messages and no sub-rule specific ids.
    pub fn german(
        data_dir: &Path,
        swiss: bool,
        spelling: Arc<crate::de::spelling::GermanSpellingRule>,
    ) -> Result<Self> {
        let expand = move |line: &str| -> Vec<String> {
            if swiss && line.contains('ß') {
                vec![line.to_string(), line.replace('ß', "ss")]
            } else {
                vec![line.to_string()]
            }
        };
        let data = CompoundData::from_paths(
            &[
                data_dir.join("de/words/compounds.txt"),
                data_dir.join("de/words/compound-cities.txt"),
            ],
            &expand,
        )?;
        let mut anti_patterns = Vec::new();
        for pattern in german_anti_patterns() {
            let compiled = pm::compile_pattern(&pattern, None, None)
                .map_err(|e| lt_core::CoreError::Data(format!("DE_COMPOUNDS antipattern: {e}")))?;
            anti_patterns.push(Arc::new(compiled));
        }
        Ok(Self {
            data,
            is_misspelled: Arc::new(move |word: &str| spelling.is_misspelled(word)),
            config: CompoundConfig {
                rule_id: if swiss {
                    "DE_CH_COMPOUNDS"
                } else {
                    "DE_COMPOUNDS"
                },
                description: "Zusammenschreibung von Wörtern, z. B. 'CD-ROM' statt 'CD ROM'"
                    .to_string(),
                with_hyphen_message: "Dieses Wort wird mit Bindestrich geschrieben.",
                without_hyphen_message: "Dieses Wort wird zusammengeschrieben.",
                with_or_without_hyphen_message:
                    "Diese Wörter werden zusammengeschrieben oder mit Bindestrich getrennt.",
                short_message: None,
                category_id: "COMPOUNDING",
                category_name: "Getrennt- und Zusammenschreibung",
                sub_rule_specific_ids: false,
                to_id_german: true,
                merge: merge_compound,
                issue_type: "misspelling",
            },
            anti_patterns,
        })
    }

    /// `es.CompoundRule` (`ES_COMPOUNDS`): `es/compounds.txt`, tagger-based
    /// `isMisspelled`, sub-rule specific ids, Spanish messages.
    pub fn spanish(data_dir: &Path, tagger: Arc<lt_tagger::SpanishTagger>) -> Result<Self> {
        let data = CompoundData::from_path(&data_dir.join("es/words/compounds.txt"))?;
        Ok(Self {
            data,
            is_misspelled: Arc::new(move |word: &str| !tagger.is_tagged_word(word)),
            config: CompoundConfig {
                rule_id: "ES_COMPOUNDS",
                description: "Palabras compuestas con guion: $match".to_string(),
                with_hyphen_message: "Se escribe con un guion.",
                without_hyphen_message: "Se escribe junto sin espacio ni guion.",
                with_or_without_hyphen_message: "Se escribe junto o con un guion.",
                short_message: Some("Error de palabra compuesta"),
                category_id: "MISC",
                category_name: "Varios",
                sub_rule_specific_ids: true,
                to_id_german: false,
                merge: merge_compound,
                issue_type: "misspelling",
            },
            anti_patterns: Vec::new(),
        })
    }

    /// `ca.CompoundRule` (`CA_COMPOUNDS`): `ca/words/compounds.txt`,
    /// `isMisspelled` via `CatalanTagger.INSTANCE_VAL` (the valencia tagger,
    /// like Java), sub-rule specific ids and Catalan messages.
    pub fn catalan(data_dir: &Path, tagger: Arc<lt_tagger::CatalanTagger>) -> Result<Self> {
        let data = CompoundData::from_path(&data_dir.join("ca/words/compounds.txt"))?;
        Ok(Self {
            data,
            is_misspelled: Arc::new(move |word: &str| !tagger.is_tagged_word(word)),
            config: CompoundConfig {
                rule_id: "CA_COMPOUNDS",
                description: "Paraules compostes amb guionet: $match".to_string(),
                with_hyphen_message: "S'escriu amb un guionet.",
                without_hyphen_message: "S'escriu junt sense espai ni guionet.",
                with_or_without_hyphen_message: "S'escriu junt o amb guionet.",
                short_message: Some("Error de mot compost"),
                category_id: "COMPOUNDING",
                category_name: "Paraules compostes",
                sub_rule_specific_ids: true,
                to_id_german: false,
                merge: merge_compound,
                issue_type: "misspelling",
            },
            anti_patterns: Vec::new(),
        })
    }

    /// `fr.CompoundRule` (`FR_COMPOUNDS`): `fr/words/compounds.txt`,
    /// tagger-based `isMisspelled`, sub-rule specific ids, French messages.
    pub fn french(data_dir: &Path, tagger: Arc<lt_tagger::FrenchTagger>) -> Result<Self> {
        let data = CompoundData::from_path(&data_dir.join("fr/words/compounds.txt"))?;
        Ok(Self {
            data,
            is_misspelled: Arc::new(move |word: &str| !tagger.is_tagged_word(word)),
            config: CompoundConfig {
                rule_id: "FR_COMPOUNDS",
                description: "Mots avec trait d’union : $match".to_string(),
                with_hyphen_message: "Écrivez avec un trait d’union.",
                without_hyphen_message: "Écrivez avec un mot seul sans espace ni trait d’union.",
                with_or_without_hyphen_message: "Écrivez avec un mot seul ou avec trait d’union.",
                short_message: Some("Erreur de trait d'union"),
                category_id: "MISC",
                category_name: "Règles de base",
                sub_rule_specific_ids: true,
                to_id_german: false,
                merge: merge_compound,
                issue_type: "misspelling",
            },
            anti_patterns: Vec::new(),
        })
    }

    /// `PostReformPortugueseCompoundRule` (`PT_COMPOUNDS_POST_REFORM`) and
    /// `PreReformPortugueseCompoundRule` (`PT_COMPOUNDS_PRE_REFORM`):
    /// `AbstractCompoundRule` with the Portuguese messages; the post-reform
    /// variant overrides `mergeCompound` for the `ultra + som → ultrassom`
    /// digraph transformation. `isMisspelled` is the base-class `false`.
    pub fn portuguese(data_dir: &Path, pre_reform: bool) -> Result<Self> {
        let (data_file, rule_id, description) = if pre_reform {
            (
                "pt/words/pre-reform-compounds.txt",
                "PT_COMPOUNDS_PRE_REFORM",
                "Palavras compostas".to_string(),
            )
        } else {
            (
                "pt/words/post-reform-compounds.txt",
                "PT_COMPOUNDS_POST_REFORM",
                "Erro na formação da palavra composta \"$match\"".to_string(),
            )
        };
        let data = CompoundData::from_path(&data_dir.join(data_file))?;
        Ok(Self {
            data,
            is_misspelled: Arc::new(|_| false),
            config: CompoundConfig {
                rule_id,
                description,
                with_hyphen_message: "Esta palavra é hifenizada.",
                without_hyphen_message: "Esta palavra é composta por justaposição.",
                with_or_without_hyphen_message:
                    "Esta palavra pode ser composta por justaposição ou hifenizada.",
                short_message: Some("Este conjunto forma uma palavra composta."),
                category_id: "COMPOUNDING",
                category_name: "Compounding",
                sub_rule_specific_ids: true,
                to_id_german: false,
                merge: if pre_reform {
                    merge_compound
                } else {
                    portuguese_merge_compound
                },
                issue_type: "grammar",
            },
            anti_patterns: Vec::new(),
        })
    }

    /// `Dutch CompoundRule` (`NL_COMPOUNDS`): `nl/words/compounds.txt`,
    /// Dutch messages, base-class `isMisspelled` (false),
    /// `sentenceStartsWithUpperCase` (default true) and sub-rule specific ids.
    pub fn dutch(data_dir: &Path) -> Result<Self> {
        let data = CompoundData::from_path(&data_dir.join("nl/words/compounds.txt"))?;
        Ok(Self {
            data,
            is_misspelled: Arc::new(|_| false),
            config: CompoundConfig {
                rule_id: "NL_COMPOUNDS",
                description: "Woorden die aaneengeschreven horen, bijvoorbeeld 'zee-egel' i.p.v. 'zee egel': $match".to_string(),
                with_hyphen_message:
                    "Dit woord hoort waarschijnlijk aaneengeschreven met een koppelteken.",
                without_hyphen_message: "Dit woord hoort waarschijnlijk aaneengeschreven.",
                with_or_without_hyphen_message:
                    "Deze uitdrukking hoort mogelijk aan elkaar, eventueel met een koppelteken.",
                short_message: Some("Koppeltekenprobleem"),
                category_id: "MISC",
                category_name: "Diversen",
                sub_rule_specific_ids: true,
                to_id_german: false,
                merge: merge_compound,
                issue_type: "misspelling",
            },
            anti_patterns: Vec::new(),
        })
    }

    /// `PortugueseColourHyphenationRule` (`PT_COLOUR_HYPHENATION`):
    /// `pt/compound_colours.txt`, Portuguese colour messages, default
    /// `mergeCompound`.
    pub fn portuguese_colour(data_dir: &Path) -> Result<Self> {
        let data = CompoundData::from_path(&data_dir.join("pt/words/compound_colours.txt"))?;
        Ok(Self {
            data,
            is_misspelled: Arc::new(|_| false),
            config: CompoundConfig {
                rule_id: "PT_COLOUR_HYPHENATION",
                description: "Nomes de cores devem ser hifenizados: \"$match\"".to_string(),
                with_hyphen_message:
                    "Nomes de cores são palavras compostas e devem ser hifenizados.",
                without_hyphen_message: "Esta palavra é composta por justaposição.",
                with_or_without_hyphen_message:
                    "Esta palavra pode ser composta por justaposição ou hifenizada.",
                short_message: Some("Nomes de cores são palavras compostas."),
                category_id: "COMPOUNDING",
                category_name: "Compounding",
                sub_rule_specific_ids: true,
                to_id_german: false,
                merge: merge_compound,
                issue_type: "grammar",
            },
            anti_patterns: Vec::new(),
        })
    }

    /// `pl.CompoundRule` (`PL_COMPOUNDS`): `pl/words/compounds.txt`. The
    /// Polish class does not override `isMisspelled`, so every candidate
    /// replacement passes `filterReplacements` (base `isMisspelled` = false).
    pub fn polish(data_dir: &Path) -> Result<Self> {
        let data = CompoundData::from_path(&data_dir.join("pl/words/compounds.txt"))?;
        Ok(Self {
            data,
            is_misspelled: Arc::new(|_| false),
            config: CompoundConfig {
                rule_id: "PL_COMPOUNDS",
                description: "Sprawdza wyrazy z łącznikiem, np. „łapu capu” zamiast „łapu-capu”"
                    .to_string(),
                with_hyphen_message: "Ten wyraz pisze się z łącznikiem.",
                without_hyphen_message: "Ten wyraz pisze się razem (bez spacji ani łącznika).",
                with_or_without_hyphen_message: "Ten wyraz pisze się z łącznikiem lub bez niego.",
                short_message: Some("Brak łącznika lub zbędny łącznik"),
                category_id: "MISC",
                category_name: "Błędy różne",
                sub_rule_specific_ids: false,
                to_id_german: false,
                merge: merge_compound,
                issue_type: "misspelling",
            },
            anti_patterns: Vec::new(),
        })
    }

    /// `ro.CompoundRule` (`RO_COMPOUND`): `ro/words/compounds.txt`. The
    /// Romanian class does not override `isMisspelled`, so every candidate
    /// replacement passes `filterReplacements` (base `isMisspelled` = false).
    pub fn romanian(data_dir: &Path) -> Result<Self> {
        let data = CompoundData::from_path(&data_dir.join("ro/words/compounds.txt"))?;
        Ok(Self {
            data,
            is_misspelled: Arc::new(|_| false),
            config: CompoundConfig {
                rule_id: "RO_COMPOUND",
                description: "Greșeală de scriere (cuvinte scrise legat sau cu cratimă)"
                    .to_string(),
                with_hyphen_message: "Cuvântul se scrie cu cratimă.",
                without_hyphen_message: "Cuvântul se scrie legat.",
                with_or_without_hyphen_message: "Cuvântul se scrie legat sau cu cratimă.",
                short_message: Some("Problemă de scriere (cratimă, spațiu, etc.)"),
                category_id: "MISC",
                category_name: "Diverse",
                sub_rule_specific_ids: false,
                to_id_german: false,
                merge: merge_compound,
                issue_type: "misspelling",
            },
            anti_patterns: Vec::new(),
        })
    }

    /// `sk.CompoundRule` (`SK_COMPOUNDS`): `sk/words/compounds.txt`. The
    /// Slovak class does not override `isMisspelled`, so every candidate
    /// replacement passes `filterReplacements` (base `isMisspelled` = false).
    pub fn slovak(data_dir: &Path) -> Result<Self> {
        let data = CompoundData::from_path(&data_dir.join("sk/words/compounds.txt"))?;
        Ok(Self {
            data,
            is_misspelled: Arc::new(|_| false),
            config: CompoundConfig {
                rule_id: "SK_COMPOUNDS",
                description:
                    "Slová so spojovníkom napr. použite „česko-slovenský” namiesto „česko slovenský”"
                        .to_string(),
                with_hyphen_message: "Toto slovo sa zvyčajne píše so spojovníkom.",
                without_hyphen_message: "Toto slovo sa obvykle píše bez spojovníka.",
                with_or_without_hyphen_message:
                    "Tento výraz sa bežne píše s alebo bez spojovníka.",
                short_message: Some("Problém spájania slov"),
                category_id: "MISC",
                category_name: "Rôzne",
                sub_rule_specific_ids: false,
                to_id_german: false,
                merge: merge_compound,
                issue_type: "misspelling",
            },
            anti_patterns: Vec::new(),
        })
    }

    pub fn rule_id(&self) -> &str {
        self.config.rule_id
    }

    /// `AbstractCompoundRule.match` over one sentence. `sentence_text` is the
    /// sentence's text; `tokens` the full token stream (whitespace included).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_text: &str,
        sentence_offset: usize,
    ) -> Vec<Match> {
        // `getSentenceWithImmunization` + `getTokensWithoutWhitespace`
        let view: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut immunized = vec![false; view.len()];
        for ap in &self.anti_patterns {
            for m in pm::find_matches(ap, &[], &view) {
                for idx in m.start_tok()..=m.end_tok() {
                    if idx < immunized.len() {
                        immunized[idx] = true;
                    }
                }
            }
        }

        let mut rule_matches: Vec<Match> = Vec::new();
        let mut prev_match_from: Option<usize> = None;
        let mut prev_tokens: Vec<QueueToken> = Vec::with_capacity(MAX_TERMS);
        let n = view.len();
        for i in 0..n + MAX_TERMS {
            let token = if i >= n {
                let front = prev_tokens.first().expect("queue non-empty");
                QueueToken {
                    surface: String::new(),
                    whitespace_before: false,
                    is_sent_start: false,
                    is_immunized: false,
                    start_pos: front.start_pos,
                    end_pos: front.start_pos,
                }
            } else {
                let t = view[i];
                QueueToken {
                    surface: t.surface().to_string(),
                    whitespace_before: t.whitespace_before,
                    is_sent_start: t.is_sentence_start,
                    is_immunized: immunized[i] || t.is_immunized,
                    start_pos: t.start_pos,
                    end_pos: t.end_pos(),
                }
            };
            if i == 0 {
                add_to_queue(&mut prev_tokens, token);
                continue;
            }
            if token.is_immunized {
                continue;
            }

            let mut strings_to_check: Vec<String> = Vec::with_capacity(MAX_TERMS);
            let mut orig_strings_to_check: Vec<String> = Vec::with_capacity(MAX_TERMS);
            let mut string_to_token: HashMap<String, usize> = HashMap::with_capacity(MAX_TERMS * 2);
            get_string_to_token_map(
                &prev_tokens,
                &mut strings_to_check,
                &mut orig_strings_to_check,
                &mut string_to_token,
            );

            for k in (0..strings_to_check.len()).rev() {
                let string_to_check = strings_to_check[k].clone();
                let orig_string_to_check = orig_strings_to_check[k].clone();
                let contains_digits = self.data.has_digit_patterns
                    && string_to_check
                        .split(' ')
                        .any(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()));
                let digits_hit = contains_digits
                    && self
                        .data
                        .incorrect
                        .contains(&digit_regexp(&string_to_check));
                if !self.data.incorrect.contains(&string_to_check) && !digits_hit {
                    continue;
                }
                let atr = &prev_tokens[string_to_token[&string_to_check]];
                let mut msg: Option<&str> = None;
                let mut replacement: Vec<String> = Vec::new();
                if self.data.dash.contains(&string_to_check) && !orig_string_to_check.contains(' ')
                {
                    // It is already joined
                    break;
                }
                if self.data.dash.contains(&string_to_check) || digits_hit {
                    replacement.push(orig_string_to_check.replace(' ', "-"));
                    msg = Some(self.config.with_hyphen_message);
                }
                if is_not_all_uppercase(&orig_string_to_check)
                    && self.data.joined.contains(&string_to_check)
                {
                    let uncapitalize_mid_words = self
                        .data
                        .joined_lowercase
                        .iter()
                        .any(|s| string_to_check.contains(s));
                    replacement.push((self.config.merge)(
                        &orig_string_to_check,
                        uncapitalize_mid_words,
                    ));
                    msg = Some(self.config.without_hyphen_message);
                }
                let parts: Vec<&str> = string_to_check.split(' ').collect();
                if parts.first().is_some_and(|p| p.chars().count() == 1) {
                    replacement.clear();
                    replacement.push(orig_string_to_check.replace(' ', "-"));
                    msg = Some(self.config.with_hyphen_message);
                } else if replacement.is_empty() || replacement.len() == 2 {
                    msg = Some(self.config.with_or_without_hyphen_message);
                }
                let first_match_token = &prev_tokens[0];
                let original_span =
                    &sentence_text[first_match_token.start_pos.min(sentence_text.len())
                        ..atr.end_pos.min(sentence_text.len())];
                replacement =
                    filter_replacements(replacement, original_span, |w| !(self.is_misspelled)(w));
                if replacement.is_empty() {
                    break;
                }
                let Some(msg) = msg else { break };
                let from_pos = first_match_token.start_pos;
                let to_pos = atr.end_pos;
                let rule_id = if self.config.sub_rule_specific_ids {
                    to_id_with(
                        &format!("{}_{string_to_check}", self.config.rule_id),
                        self.config.to_id_german,
                    )
                } else {
                    self.config.rule_id.to_string()
                };
                let description = self
                    .config
                    .description
                    .replace("$match", &orig_string_to_check);
                let suggestions: Vec<Suggestion> = replacement
                    .into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect();
                let m = Match::new(
                    rule_id,
                    Option::<String>::None,
                    msg,
                    self.config.short_message.map(|s| s.to_string()),
                    TextRange::new(sentence_offset + from_pos, sentence_offset + to_pos),
                    suggestions,
                    self.config.category_id,
                    self.config.category_name,
                )
                .with_metadata(&description, self.config.issue_type, 1);
                // avoid duplicate matches:
                if prev_match_from == Some(from_pos) {
                    prev_match_from = Some(from_pos);
                    break;
                }
                prev_match_from = Some(from_pos);
                rule_matches.push(m);
                break;
            }
            add_to_queue(&mut prev_tokens, token);
        }
        rule_matches
    }
}

fn get_string_to_token_map(
    prev_tokens: &[QueueToken],
    strings_to_check: &mut Vec<String>,
    orig_strings_to_check: &mut Vec<String>,
    string_to_token: &mut HashMap<String, usize>,
) {
    let mut sb = String::new();
    let mut is_first_sent_start = false;
    for (j, atr) in prev_tokens.iter().enumerate() {
        if atr.whitespace_before {
            sb.push(' ');
        }
        sb.push_str(&atr.surface);
        if j == 0 {
            is_first_sent_start = atr.is_sent_start;
        }
        if j >= 1 || (j == 0 && !is_first_sent_start) {
            let mut string_to_check = normalize(&sb);
            if is_first_sent_start {
                string_to_check = uncapitalize(&string_to_check);
            }
            strings_to_check.push(string_to_check.clone());
            orig_strings_to_check.push(sb.trim().to_string());
            string_to_token.entry(string_to_check).or_insert(j);
        }
    }
}

fn add_to_queue(queue: &mut Vec<QueueToken>, token: QueueToken) {
    if queue.len() == MAX_TERMS {
        queue.remove(0);
    }
    queue.push(token);
}

/// `DIGIT.matcher(...).replaceAll("\\d+")`: only called when the string
/// contains numeric parts.
fn digit_regexp(s: &str) -> String {
    static DIGIT: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\d+").unwrap());
    DIGIT.replace_all(s, "\\d+").to_string()
}

/// `AbstractCompoundRule.normalize`.
fn normalize(s: &str) -> String {
    let t = s.trim().replace(" - ", " ").replace('-', " ");
    whitespace_re().replace_all(&t, " ").to_string()
}

/// `StringUtils.uncapitalize` (ASCII/locale-independent lowercasing of the
/// first character when it is uppercase).
fn uncapitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) if first.is_uppercase() => {
            first.to_lowercase().collect::<String>() + chars.as_str()
        }
        _ => s.to_string(),
    }
}

/// `AbstractCompoundRule.isNotAllUppercase` over the space-separated parts.
fn is_not_all_uppercase(s: &str) -> bool {
    for part in s.split(' ') {
        if part != "-" && lt_spell::morfologik::is_all_uppercase(part) {
            return false;
        }
    }
    true
}

/// `AbstractCompoundRule.mergeCompound`.
fn merge_compound(s: &str, uncapitalize_mid_words: bool) -> String {
    let replaced = s.replace('-', " ");
    let parts: Vec<&str> = replaced.split(' ').collect();
    let mut out = String::new();
    for (k, part) in parts.iter().enumerate() {
        if k == 0 {
            out.push_str(part);
        } else if uncapitalize_mid_words {
            out.push_str(&uncapitalize(part));
        } else {
            out.push_str(part);
        }
    }
    out
}

/// `PostReformPortugueseCompoundRule.mergeCompound`: ultra + som →
/// ultrassom (doubles `<r>`/`<s>` after a preceding vowel).
fn portuguese_merge_compound(s: &str, uncapitalize_mid_words: bool) -> String {
    static VOWEL: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)^.+[aeiou]$").unwrap());
    static RS: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)^[rs].+$").unwrap());
    let replaced = s.replace('-', " ");
    let mut parts: Vec<String> = replaced.split(' ').map(str::to_string).collect();
    let mut out = String::new();
    for k in 0..parts.len() {
        if k == 0 {
            out.push_str(&parts[0]);
            continue;
        }
        if VOWEL.is_match(&parts[k - 1]) && RS.is_match(&parts[k]) {
            let first = parts[k].chars().next().expect("non-empty part");
            parts[k] = format!("{first}{}", parts[k]);
        }
        if uncapitalize_mid_words {
            out.push_str(&uncapitalize(&parts[k]));
        } else {
            out.push_str(&parts[k]);
        }
    }
    out
}

/// `AbstractCompoundRule.filterReplacements` (`isCorrectSpell` = not
/// misspelled, since `linguServices` is null outside LO/OO).
fn filter_replacements(
    replacements: Vec<String>,
    original: &str,
    is_correct_spell: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut out = Vec::new();
    for replacement in replacements {
        let new_replacement = dashes_re().replace_all(&replacement, "-").to_string();
        if new_replacement != original && is_correct_spell(&new_replacement) {
            out.push(new_replacement);
        }
    }
    out
}

/// `StringTools.toId` (`NONCHAR = [^A-ZÀ-ÖØ-Þ]`); the German short code
/// replaces the umlauts first.
fn to_id_with(input: &str, german: bool) -> String {
    let mut normalized = input
        .trim()
        .to_uppercase()
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
        .map(|c| {
            if c.is_ascii_uppercase()
                || ('\u{c0}'..='\u{d6}').contains(&c)
                || ('\u{d8}'..='\u{de}').contains(&c)
            {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    #[test]
    fn normalizes_compounds() {
        assert_eq!(normalize(" part - time "), "part time");
        assert_eq!(normalize("part-time"), "part time");
    }

    #[test]
    fn merges_compounds() {
        assert_eq!(merge_compound("Dev-Ops", false), "DevOps");
        assert_eq!(merge_compound("Dev Ops", true), "Devops");
    }

    #[test]
    fn loads_data_sets() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !dir.join("en/words/compounds.txt").lt_exists() {
            eprintln!("skipping: data dir not found");
            return;
        }
        let data = CompoundData::from_path(&dir.join("en/words/compounds.txt")).unwrap();
        // `an-other+`: joined suggestion only, no hyphen suggestion
        assert!(data.incorrect.contains("an other"));
        assert!(data.joined.contains("an other"));
        assert!(!data.dash.contains("an other"));
        // `full-blown*`: hyphen suggestion only
        assert!(data.incorrect.contains("full blown"));
        assert!(data.dash.contains("full blown"));
        assert!(!data.joined.contains("full blown"));
    }
}
