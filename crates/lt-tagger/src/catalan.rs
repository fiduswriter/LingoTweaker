//! Catalan tagger (`CatalanTagger`): the `BaseTagger` lookup (`ca-ES.dict` +
//! `added*`/`removed*` manual taggers) plus the Catalan `tag()` overrides:
//! typographic-apostrophe normalization/flag, NFC normalization, the
//! ALLUPPERCASE exceptions, prefix/adverb/adjective-compound tagging, emoji,
//! the Valencian POS-tag filter, incorrect-verb tagging (from
//! `ca/rules/replace_verbs.txt`) and the Valencian `-iste` adjectives.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

use crate::english::{is_all_uppercase, is_mixed_case, uppercase_first_char};
use crate::spanish::is_emoji;
use crate::{Dictionary, DictionaryInfo, ManualTagger};

static ALLUPPERCASE_EXCEPTIONS: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["ARNAU", "CRISTIAN", "TOMÀS"]);

static ADJ_PART_FS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:VMP00SF.|A[QO].[FC]S.)$").unwrap());
static VERB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:V.+)$").unwrap());
static PREFIXES_FOR_VERBS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(auto)(.*[aeiouàéèíòóïü].+[aeiouàéèíòóïü].*)$").unwrap());
static ADJECTIU_COMPOST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.*)o-(.*.*)$").unwrap());
static TRES_ADJECTIUS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.*)o-(.*)o-(.*.*)$").unwrap());

static ALTRES_PREFIXOS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "greco", "sino", "italo", "franco", "gal·lo", "luso", "germano", "hispano", "anglo",
        "àrabo", "austro", "belgo",
    ]
});
static NO_ALTRES_PREFIXOS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "grego",
        "xineso",
        "italiano",
        "franceso",
        "portugueso",
        "angleso",
        "espanyolo",
        "alemanyo",
        "arabo",
        "austríaco",
        "bèlgico",
    ]
});

/// The `endings` alternation of `CatalanTagger` (verb desinences used by
/// `additionalTagsForIncorrectVerbs`), verbatim from the Java source.
static ENDINGS: &str = "a|ada|ades|am|ant|ar|ara|aran|arem|aren|ares|areu|aria|arien|aries|arà|aràs|aré|aríem|aríeu|assen|asses|assin|assis|at|ats|au|ava|aven|aves|e|ec|ega|eguda|egudes|eguem|eguen|eguera|egueren|egueres|egues|eguessen|eguesses|eguessin|eguessis|egueu|egui|eguin|eguis|egut|eguts|egué|eguérem|eguéreu|egués|eguéssem|eguésseu|eguéssim|eguéssiu|eguí|eix|eixem|eixen|eixent|eixeran|eixerem|eixeren|eixeres|eixereu|eixeria|eixerien|eixeries|eixerà|eixeràs|eixeré|eixeríem|eixeríeu|eixes|eixessen|eixesses|eixessin|eixessis|eixeu|eixi|eixia|eixien|eixies|eixin|eixis|eixo|eixé|eixérem|eixéreu|eixés|eixéssem|eixésseu|eixéssim|eixéssiu|eixí|eixíem|eixíeu|em|en|es|esc|esca|escuda|escudes|escut|escuts|esquem|esquen|esquera|esqueren|esqueres|esques|esquessen|esquesses|esquessin|esquessis|esqueu|esqui|esquin|esquis|esqué|esquérem|esquéreu|esqués|esquéssem|esquésseu|esquéssim|esquéssiu|esquí|essen|esses|essin|essis|eu|i|ia|ida|ides|ien|ies|iguem|igueu|im|in|int|ir|ira|iran|irem|iren|ires|ireu|iria|irien|iries|irà|iràs|iré|iríem|iríeu|is|isc|isca|isquen|isques|issen|isses|issin|issis|it|its|iu|ix|ixen|ixes|o|à|àrem|àreu|às|àssem|àsseu|àssim|àssiu|àvem|àveu|èixer|éixer|és|éssem|ésseu|éssim|éssiu|í|íem|íeu|írem|íreu|ís|íssem|ísseu|íssim|íssiu|ïs";

static DESINENCIES_1CONJ_0: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("(?i)^(.+?)({ENDINGS})$")).unwrap());
static DESINENCIES_1CONJ_1: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("(?i)^(.+)({ENDINGS})$")).unwrap());

/// Plain `BaseTagger` over the loaded Morfologik `ca-ES.dict` plus the
/// Catalan `tag()` overrides.
pub struct CatalanTagger {
    dict: Dictionary,
    /// `added.txt` + `added_custom.txt` (`CombiningTagger` second tagger)
    manual: ManualTagger,
    /// `removed.txt` + `removed_custom.txt` (`CombiningTagger` removal tagger)
    removals: ManualTagger,
    is_valencian: bool,
    /// `CatalanTagger.wrongVerbs` (`/ca/replace_verbs.txt`); only the wrong
    /// forms (keys) are used by `tryTag`.
    wrong_verbs: HashMap<String, Vec<String>>,
}

impl std::fmt::Debug for CatalanTagger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CatalanTagger").finish_non_exhaustive()
    }
}

impl CatalanTagger {
    /// Load `ca/dictionaries/ca-ES.{dict,info}` plus the manual word lists
    /// and the incorrect-verb data. `is_valencian` selects the Valencian
    /// POS-tag filter and the `-iste` adjective handling.
    pub fn load(data_dir: &Path, is_valencian: bool) -> Result<Self> {
        let dict_dir = data_dir.join("ca/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("ca-ES.info"))?;
        let dict = Dictionary::load(&dict_dir.join("ca-ES.dict"), &info)?;
        let words_dir = data_dir.join("ca/words");
        let manual = ManualTagger::load(&[
            &words_dir.join("added.txt"),
            &words_dir.join("added_custom.txt"),
        ])?;
        let removals = ManualTagger::load(&[
            &words_dir.join("removed.txt"),
            &words_dir.join("removed_custom.txt"),
        ])?;
        let wrong_verbs = load_simple_replace_data(&data_dir.join("ca/rules/replace_verbs.txt"));
        Ok(Self {
            dict,
            manual,
            removals,
            is_valencian,
            wrong_verbs,
        })
    }

    /// The plain `CatalanTagger` (`INSTANCE_CAT`).
    pub fn load_default(data_dir: &Path) -> Result<Self> {
        Self::load(data_dir, false)
    }

    /// `CombiningTagger.tag`: manual readings first, dictionary readings
    /// appended, removal tagger applied last (`overwriteWithManualTagger`
    /// is false).
    fn word_lookup(&self, word: &str) -> Vec<(String, String)> {
        let mut result: Vec<(String, String)> = self.manual.lookup(word).to_vec();
        result.extend(self.dict.lookup(word));
        if !self.removals.is_empty() {
            let removals = self.removals.lookup(word);
            if !removals.is_empty() {
                result.retain(|tw| !removals.contains(tw));
            }
        }
        result
    }

    fn add_from_word_tagger(&self, surface: &str, lookup: &str, out: &mut Vec<AnalyzedToken>) {
        for (stem, tag) in self.word_lookup(lookup) {
            out.push(AnalyzedToken::new(
                surface,
                Some(stem.clone()),
                Some(tag.clone()),
            ));
        }
    }

    /// `CatalanTagger.wordformHasPostag`.
    fn wordform_has_postag(&self, wordform: &str, postag: &str) -> bool {
        self.word_lookup(wordform)
            .iter()
            .any(|(_, tag)| tag == postag)
    }

    /// `CatalanTagger.isValidAdjectiveForm`.
    fn is_valid_adjective_form(&self, word_stem: &str) -> bool {
        let o_form = format!("{word_stem}o");
        !NO_ALTRES_PREFIXOS.contains(&o_form.as_str())
            && (self.wordform_has_postag(&format!("{word_stem}a"), "AQ0FS0")
                || ALTRES_PREFIXOS.contains(&o_form.as_str()))
    }

    /// `CatalanTagger.additionalTags` (null → empty list here).
    fn additional_tags(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut additional: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase().nfc().collect::<String>();

        // Any well-formed adverb with suffix -ment is tagged as an adverb (RG)
        if lower_word.ends_with("ment") {
            let count = lower_word.chars().count();
            // Java `replaceAll("^(.+)ment$", "$1")` returns the input
            // unchanged when the `.+` cannot match (word == "ment")
            let possible_adj: String = if count > 4 {
                lower_word.chars().take(count - 4).collect()
            } else {
                lower_word.clone()
            };
            for (_, pos_tag) in self.word_lookup(&possible_adj) {
                if ADJ_PART_FS.is_match(&pos_tag) {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(lower_word.clone()),
                        Some("RG".to_string()),
                    ));
                    return additional;
                }
            }
        }
        // Any well-formed verb with prefixes is tagged as a verb copying the
        // original tags
        if let Some(caps) = PREFIXES_FOR_VERBS.captures(word) {
            let prefix = caps
                .get(1)
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_default();
            let possible_verb = caps
                .get(2)
                .map(|m| m.as_str().to_lowercase().nfc().collect::<String>())
                .unwrap_or_default();
            for (lemma, pos_tag) in self.word_lookup(&possible_verb) {
                if lemma != "nòmer" && VERB.is_match(&pos_tag) {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(format!("{prefix}{lemma}")),
                        Some(pos_tag.clone()),
                    ));
                }
            }
            return additional;
        }
        // folklòrico-popular
        if let Some(caps) = ADJECTIU_COMPOST.captures(word) {
            let adj1 = caps
                .get(1)
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_default();
            if self.is_valid_adjective_form(&adj1) {
                let adj2 = caps
                    .get(2)
                    .map(|m| m.as_str().to_lowercase())
                    .unwrap_or_default();
                for (lemma, pos_tag) in self.word_lookup(&adj2) {
                    if pos_tag.starts_with('A') {
                        additional.push(AnalyzedToken::new(
                            word,
                            Some(format!("{adj1}o-{lemma}")),
                            Some(pos_tag.clone()),
                        ));
                        return additional;
                    }
                }
            }
        }
        // franco-americano-alemany
        if let Some(caps) = TRES_ADJECTIUS.captures(word) {
            let adj1 = caps
                .get(1)
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_default();
            let adj2 = caps
                .get(2)
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_default();
            if self.is_valid_adjective_form(&adj1) && self.is_valid_adjective_form(&adj2) {
                let adj3 = caps
                    .get(3)
                    .map(|m| m.as_str().to_lowercase())
                    .unwrap_or_default();
                for (lemma, pos_tag) in self.word_lookup(&adj3) {
                    if pos_tag.starts_with('A') {
                        additional.push(AnalyzedToken::new(
                            word,
                            Some(format!("{adj1}o-{adj2}o-{lemma}")),
                            Some(pos_tag.clone()),
                        ));
                        return additional;
                    }
                }
            }
        }
        // Interpret deprecated characters of "ela geminada"
        // (U+013F LATIN CAPITAL LETTER L WITH MIDDLE DOT,
        // U+0140 LATIN SMALL LETTER L WITH MIDDLE DOT)
        if word.contains('\u{0140}') || word.contains('\u{013F}') {
            let possible_word = lower_word.replace('\u{0140}', "l·");
            self.add_from_word_tagger(word, &possible_word, &mut additional);
            return additional;
        }
        // adjectives -iste in Valencian variant
        if self.is_valencian && lower_word.ends_with("iste") && lower_word.chars().count() > 4 {
            let count = lower_word.chars().count();
            let possible_adj_noun: String =
                lower_word.chars().take(count - 4).collect::<String>() + "ista";
            for (_, pos_tag) in self.word_lookup(&possible_adj_noun) {
                if pos_tag == "NCCS000" {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(possible_adj_noun.clone()),
                        Some("NCMS000".to_string()),
                    ));
                }
                if pos_tag == "AQ0CS0" {
                    additional.push(AnalyzedToken::new(
                        word,
                        Some(possible_adj_noun.clone()),
                        Some("AQ0MS0".to_string()),
                    ));
                }
                if !additional.is_empty() {
                    return additional;
                }
            }
        }
        additional
    }

    /// `CatalanTagger.filterAnalyzedTokensInPlace`.
    fn filter_analyzed_tokens_in_place(&self, tokens: &mut Vec<AnalyzedToken>) {
        if tokens.is_empty() {
            return;
        }
        if self.is_valencian {
            for token in tokens.iter_mut() {
                if let Some(pos_tag) = &token.pos_tag {
                    if let Some(stripped) = pos_tag.strip_prefix('0') {
                        token.pos_tag = Some(stripped.to_string());
                    }
                }
            }
        } else {
            tokens.retain(|token| {
                !token
                    .pos_tag
                    .as_deref()
                    .is_some_and(|tag| tag.starts_with('0'))
            });
        }
    }

    /// `CatalanTagger.additionalTagsForIncorrectVerbs`.
    fn additional_tags_for_incorrect_verbs(
        &self,
        original_word: &str,
        lower_word: &str,
    ) -> Vec<AnalyzedToken> {
        // enriure, enfotre...
        if let Some(stripped) = lower_word.strip_prefix("en") {
            let mut selected: Vec<(String, String)> = Vec::new();
            let mut lemma = String::new();
            for (l, tag) in self.word_lookup(stripped) {
                if tag.starts_with('V') {
                    lemma = format!("en{l}");
                    selected.push((lemma.clone(), tag));
                }
            }
            if !selected.is_empty() && (lemma == "enfotre" || lemma == "enriure") {
                return selected
                    .into_iter()
                    .filter(|(_, tag)| tag.starts_with('V'))
                    .map(|(l, tag)| AnalyzedToken::new(original_word, Some(l), Some(tag)))
                    .collect();
            }
        }
        for pattern in [&*DESINENCIES_1CONJ_0, &*DESINENCIES_1CONJ_1] {
            let Some(caps) = pattern.captures(lower_word) else {
                continue;
            };
            let base_lexeme = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let mut desinence = caps
                .get(2)
                .map(|m| m.as_str())
                .unwrap_or_default()
                .to_string();
            let mut adjusted_lexeme = base_lexeme.to_string();
            let mut lexemes: Vec<String> = vec![base_lexeme.to_string()];
            if matches!(desinence.chars().next(), Some('e' | 'é' | 'i' | 'ï')) {
                adjusted_lexeme = adjust_lexeme_for_soft_vowel(base_lexeme);
                if !lexemes.contains(&adjusted_lexeme) {
                    lexemes.push(adjusted_lexeme.clone());
                }
            }
            if desinence.starts_with('ï') {
                desinence = format!("i{}", &desinence[2..]);
            }
            let mut tokens = self.try_tag(
                original_word,
                &format!("{adjusted_lexeme}ar"),
                &format!("cant{desinence}"),
            );
            for lex in &lexemes {
                if tokens.is_empty() {
                    tokens = self.try_tag(
                        original_word,
                        &format!("{lex}ir"),
                        &format!("serv{desinence}"),
                    );
                }
                if tokens.is_empty() && lex.ends_with('g') {
                    tokens = self.try_tag(
                        original_word,
                        &format!("{lex}uir"),
                        &format!("serv{desinence}"),
                    );
                }
            }
            if tokens.is_empty() {
                let eixer = format!("{base_lexeme}èixer");
                tokens = self.try_tag(original_word, &eixer, &format!("con{desinence}"));
                if tokens.is_empty() {
                    tokens = self.try_tag(original_word, &eixer, &format!("desmer{desinence}"));
                }
            }
            if !tokens.is_empty() {
                return tokens;
            }
        }
        Vec::new()
    }

    /// `CatalanTagger.tryTag`: only verbs, with the infinitive as lemma.
    fn try_tag(
        &self,
        original_word: &str,
        infinitive: &str,
        conjugated: &str,
    ) -> Vec<AnalyzedToken> {
        if !self.wrong_verbs.contains_key(infinitive) {
            return Vec::new();
        }
        self.word_lookup(conjugated)
            .into_iter()
            .filter(|(_, tag)| tag.starts_with('V'))
            .map(|(_, tag)| {
                AnalyzedToken::new(original_word, Some(infinitive.to_string()), Some(tag))
            })
            .collect()
    }

    /// `BaseTagger.getAnalyzedTokens`-style single-word lookup (Catalan
    /// stage-1 helper used by tests/probes).
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut l: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase();
        let is_lowercase = word == lower_word;
        let is_mixed = is_mixed_case(word);
        self.add_from_word_tagger(word, word, &mut l);
        if !is_lowercase && !is_mixed {
            self.add_from_word_tagger(word, &lower_word, &mut l);
        }
        if l.is_empty() {
            l.push(AnalyzedToken::new(word, None, None));
        }
        l
    }

    /// Tokenizer helper: `CatalanTagger.INSTANCE.tag([word]).get(0).isTagged()`
    /// (the full `tag()` path, matching Java's `wordsToAdd` check).
    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag(std::slice::from_ref(&word.to_string()))
            .first()
            .is_some_and(|atr| atr.is_tagged)
    }

    /// `CatalanTagger.tag(List<String>)`: the `BaseTagger` lookup plus the
    /// Catalan overrides. One entry per input token with the cumulative Java
    /// UTF-16 position (the pipeline overrides `start_pos` with byte
    /// offsets).
    pub fn tag(&self, sentence_tokens: &[String]) -> Vec<AnalyzedTokenReadings> {
        let mut out = Vec::with_capacity(sentence_tokens.len());
        let mut pos = 0usize;
        for raw in sentence_tokens {
            let mut original_word = raw.clone();
            // "This hack allows all rules and dictionary entries to work with
            // typewriter apostrophe"
            let mut contains_typographic_apostrophe = false;
            if original_word.chars().count() > 1 && original_word.contains('\u{2019}') {
                contains_typographic_apostrophe = true;
                original_word = original_word.replace('\u{2019}', "'");
            }
            let normalized_word: String = original_word.nfc().collect();
            let lower_word = normalized_word.to_lowercase();
            let is_lowercase = normalized_word == lower_word;
            let is_mixed = is_mixed_case(&normalized_word);
            let is_all_upper = is_all_uppercase(&normalized_word);

            let mut l: Vec<AnalyzedToken> = Vec::new();
            self.add_from_word_tagger(&original_word, &normalized_word, &mut l);
            if !is_lowercase && !is_mixed {
                self.add_from_word_tagger(&original_word, &lower_word, &mut l);
            }
            // tag all-uppercase proper nouns (ex. FRANÇA)
            if (l.is_empty() || ALLUPPERCASE_EXCEPTIONS.contains(&normalized_word.as_str()))
                && is_all_upper
            {
                let first_upper = uppercase_first_char(&lower_word);
                self.add_from_word_tagger(&original_word, &first_upper, &mut l);
            }
            // additional tagging with prefixes
            if l.is_empty() && !is_mixed {
                let additional = self.additional_tags(&original_word);
                l.extend(additional);
            }
            // emoji
            if l.is_empty() && is_emoji(&original_word) {
                l.push(AnalyzedToken::new(
                    &original_word,
                    Some("_emoji_".to_string()),
                    Some("_emoji_".to_string()),
                ));
            }
            // filter for Valencian POS tags
            self.filter_analyzed_tokens_in_place(&mut l);
            // incorrect verbs
            let mut is_incorrect_verb = false;
            if l.is_empty() {
                let tags_for_incorrect_verbs =
                    self.additional_tags_for_incorrect_verbs(&original_word, &lower_word);
                if !tags_for_incorrect_verbs.is_empty() {
                    l.extend(tags_for_incorrect_verbs);
                    is_incorrect_verb = true;
                }
            }
            // if empty, add an analyzed token with no lemma and no postag
            if l.is_empty() {
                l.push(AnalyzedToken::new(&original_word, None, None));
            }
            let mut atr = AnalyzedTokenReadings::new(l);
            if contains_typographic_apostrophe {
                atr.has_typographic_apostrophe = true;
            }
            if is_incorrect_verb {
                atr.chunk_tags = vec!["_incorrect_verb_".to_string()];
            }
            atr.start_pos = pos;
            atr.raw_byte_len = raw.len();
            atr.is_tagged = atr.readings.iter().any(|r| r.pos_tag.is_some());
            out.push(atr);
            pos += raw.encode_utf16().count();
        }
        out
    }
}

/// `CatalanTagger.adjustLexemeForSoftVowel`.
fn adjust_lexeme_for_soft_vowel(lexeme: &str) -> String {
    if let Some(stem) = lexeme.strip_suffix('c') {
        return format!("{stem}ç");
    }
    if let Some(stem) = lexeme.strip_suffix("qu") {
        return format!("{stem}c");
    }
    if let Some(stem) = lexeme.strip_suffix('g') {
        return format!("{stem}j");
    }
    if let Some(stem) = lexeme.strip_suffix("gü") {
        return format!("{stem}gu");
    }
    if let Some(stem) = lexeme.strip_suffix("gu") {
        return format!("{stem}g");
    }
    lexeme.to_string()
}

/// `SimpleReplaceDataLoader` (`wrong|wrong2=repl|repl2` lines, `#` comments).
fn load_simple_replace_data(path: &Path) -> HashMap<String, Vec<String>> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return HashMap::new();
    };
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() != 2 {
            continue;
        }
        let wrong_forms: Vec<String> = parts[0].split('|').map(str::to_string).collect();
        let replacements: Vec<String> = parts[1].split('|').map(str::to_string).collect();
        for wrong in wrong_forms {
            map.insert(wrong, replacements.clone());
        }
    }
    map
}
