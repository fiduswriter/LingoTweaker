//! Ukrainian tagger: port of `org.languagetool.tagging.uk.UkrainianTagger`,
//! a `BaseTagger` over `uk/dictionaries/ukrainian.dict` with
//! `tagLowercaseWithUppercase = false`.
//!
//! Wired so far (stage 3c part 1): the plain `BaseTagger` lookup, the
//! `additionalTags` heuristics that do not need `CompoundTagger` (numerals,
//! Latin/Cyrillic numerals, time, date, hashtags, `З`-for-`С`, `ї`-for-`і`,
//! missing apostrophe/hyphen, `стодвадцятиріччя`-style numerals) and the
//! `getAnalyzedTokens` alternates (`ґ`→`г`, `ія`→`іа`, `тер`→`тр`, `льо`→`ло`,
//! `сьвя`/`сьві`, `ьск`→`ьськ`, repeated vowels, `[`/`]`, all-uppercase proper
//! names, capitalized compound adjectives).
//!
//! The `CompoundTagger` branches (`generateEntities`, `guessCompoundTag`,
//! `guessOtherTags`) are stubbed to empty until `CompoundTagger` is ported; such
//! words stay untagged, exactly as before this change.

use std::path::Path;
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};

use crate::english::is_mixed_case;
use crate::uk_compound::{adjust, CompoundTagger};
use crate::uk_helpers::{
    add_if_not_contains, capitalize_proper_name, full_match, is_all_uppercase_uk,
};
use crate::{Dictionary, DictionaryInfo, ManualTagger};

static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:[-+±]?[0-9]+(,[0-9]+)?([-–—][0-9]+(,[0-9]+)?)?|\d{1,3}([\s\u{00A0}\u{202F}]\d{3})+)$",
    )
    .unwrap()
});
static LATIN_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(?=[MDCLXVI])M*(C[MD]|D?C*)(X[CL]|L?X*)(I[XV]|V?I*))$").unwrap()
});
static PATTERN_MD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[MD]+$").unwrap());
static LATIN_NUMBER_CYR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[IXІХV]{2,4}(-[а-яі]{1,4})?|[IXІХV](-[а-яі]{1,4}))$").unwrap()
});
static TIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:([01]?[0-9]|2[0-3])[.:][0-5][0-9]|([01]?[0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9])$")
        .unwrap()
});
static DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\d]{1,2}\.[\d]{1,2}\.[\d]{4}$").unwrap());
static HASHTAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^#[а-яіїєґa-z_][а-яіїєґa-z0-9_]*$").unwrap());
static CAPS_INSIDE_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[а-яіїєґ'-]*[а-яіїєґ][А-ЯІЇЄҐ][а-яіїєґ][а-яіїєґ'-]*$").unwrap());
static Z_KPTF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?iu)^з[кптфх].+$").unwrap());
static YI_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([бвгґджзклмнпрстфхцчшщ])ї").unwrap());
static MISSING_APO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([бвгґдзкмнпрстфхш])([єїюя])").unwrap());
static MISSING_HYPHEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([а-яіїєґ']+)(небудь)$").unwrap());
static COMPOUND_WITH_QUOTES_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[-–][«"„]"#).unwrap());
static COMPOUND_WITH_QUOTES_REGEX2: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[»"“][-–]"#).unwrap());
static QUOTES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[«»"„“]"#).unwrap());
static IGNORED_CHARS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\u{00AD}\u{0301}]").unwrap());
static REPEATED_VOWEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([аеєиіїоуюя])\1{2,}").unwrap());
static WORDS_WITH_BRACKETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([а-яіїєґ])\[([а-яіїєґ]+)\]").unwrap());
static ADJ_POS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^adj.*$").unwrap());
static PROP_POS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:noun.*?:prop.*|noninfl.*)$").unwrap());
static NOT_BAD_POS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?!.*:(bad|arch|alt|abbr|slang|subst|short|long)).*$").unwrap());
static REPEATED_OK_POS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?!noun.*:prop|.*abbr).*$").unwrap());
static ALT_DASHES_IN_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[а-яіїєґ0-9a-z]–[а-яіїєґ]|[а-яіїєґ]–[0-9]").unwrap());
static LEFT_O_ADJ_INVALID_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(багато|мало|високо|низько|старо|важко|зовнішньо|внутрішньо|ново|середньо|південно|північно|західно|східно|центрально|ранньо|пізньо)(.+)",
    )
    .unwrap()
});
static RICCHA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(сто|[а-яіїєґ']+?(?:сот))?([а-яіїє']+?(?:ти|ка|то))?([а-яіїє']+?(?:ти|ри|ох|ми|во))?(річч[а-яі]{1,3})$",
    )
    .unwrap()
});
static OTYI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(сто|[а-яіїєґ']+?(?:сот))?([а-яіїє']+?(?:ти|ка|то))?([а-яіїє']+?(?:ти|ри|ох|ми|во|но))?((?:мільйон|тисяч|річн)[а-яії]+)$",
    )
    .unwrap()
});

/// Port of `UkrainianTagger`'s `BaseTagger` foundation + overrides.
#[derive(Debug)]
pub struct UkrainianTagger {
    dict: Dictionary,
    /// `/uk/added.txt` + `/uk/added_custom.txt`
    manual: ManualTagger,
    /// `/uk/removed.txt` + `/uk/removed_custom.txt`
    removals: ManualTagger,
    compound: CompoundTagger,
}

impl UkrainianTagger {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("uk/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("ukrainian.info"))?;
        let dict = Dictionary::load(&dict_dir.join("ukrainian.dict"), &info)?;
        let words_dir = data_dir.join("uk/words");
        let manual = ManualTagger::load(&[
            &words_dir.join("added.txt"),
            &words_dir.join("added_custom.txt"),
        ])?;
        let removals = ManualTagger::load(&[
            &words_dir.join("removed.txt"),
            &words_dir.join("removed_custom.txt"),
        ])?;
        Ok(Self {
            dict,
            manual,
            removals,
            compound: CompoundTagger::load(data_dir),
        })
    }

    pub fn dict(&self) -> &Dictionary {
        &self.dict
    }

    /// `CombiningTagger.tag`: manual readings first, dictionary readings
    /// appended, removal tagger applied last (`overwriteWithManualTagger` is
    /// false).
    pub(crate) fn word_lookup(&self, word: &str) -> Vec<(String, String)> {
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

    /// `BaseTagger.asAnalyzedTokenListForTaggedWords`.
    pub(crate) fn tagged_to_at(
        &self,
        surface: &str,
        tagged: &[(String, String)],
    ) -> Vec<AnalyzedToken> {
        tagged
            .iter()
            .map(|(lemma, tag)| AnalyzedToken::new(surface, Some(lemma.clone()), Some(tag.clone())))
            .collect()
    }

    fn add_from_word_tagger(&self, surface: &str, lookup: &str, out: &mut Vec<AnalyzedToken>) {
        let tagged = self.word_lookup(lookup);
        out.extend(self.tagged_to_at(surface, &tagged));
    }

    /// `BaseTagger.getAnalyzedTokens` (with virtual `additionalTags` and
    /// `tagLowercaseWithUppercase = false`).
    fn base_get_analyzed_tokens(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut result: Vec<AnalyzedToken> = Vec::new();
        let lower_word = word.to_lowercase();
        let is_lowercase = word == lower_word;
        let is_mixed = is_mixed_case(word);

        self.add_from_word_tagger(word, word, &mut result);
        if !is_lowercase && !is_mixed {
            self.add_from_word_tagger(word, &lower_word, &mut result);
        }
        if result.is_empty() {
            result = self.additional_tags(word);
        }
        if result.is_empty() {
            result.push(AnalyzedToken::new(word, None, None));
        }
        result
    }

    /// `UkrainianTagger.additionalTags`. The `CompoundTagger` branches
    /// (`generateEntities`, `guessCompoundTag`, `guessOtherTags`) are stubbed.
    fn additional_tags(&self, word: &str) -> Vec<AnalyzedToken> {
        if full_match(&NUMBER, word) {
            return vec![AnalyzedToken::new(
                word,
                Some(word.to_string()),
                Some("number".into()),
            )];
        }
        if full_match(&LATIN_NUMBER, word) && !full_match(&PATTERN_MD, word) {
            return vec![AnalyzedToken::new(
                word,
                Some(word.to_string()),
                Some("number:latin".into()),
            )];
        }
        if full_match(&LATIN_NUMBER_CYR, word) {
            let ordinal = match word.rfind('-') {
                Some(dash) if dash > 0 => {
                    crate::uk_helpers::is_possible_adj_adj_ending(&word[dash + 1..])
                }
                _ => false,
            };
            let dash_idx = word.rfind('-');
            if dash_idx.is_none() || ordinal {
                return vec![AnalyzedToken::new(
                    word,
                    Some(word.to_string()),
                    Some("number:latin:bad".into()),
                )];
            }
        }
        if full_match(&TIME, word) {
            return vec![AnalyzedToken::new(
                word,
                Some(word.to_string()),
                Some("time".into()),
            )];
        }
        if full_match(&DATE, word) {
            return vec![AnalyzedToken::new(
                word,
                Some(word.to_string()),
                Some("date".into()),
            )];
        }
        // compoundTagger.generateEntities(word)
        if word.find('(').is_some_and(|i| i > 0) || word.find('/').is_some_and(|i| i > 0) {
            let entities = self.compound.generate_entities(word);
            if !entities.is_empty() {
                return entities;
            }
        }
        if word.starts_with('#') && full_match(&HASHTAG, word) {
            return vec![AnalyzedToken::new(
                word,
                Some(word.to_string()),
                Some("hashtag".into()),
            )];
        }
        if word.chars().count() > 5 && full_match(&CAPS_INSIDE_WORD, word) {
            let tagged = self.word_lookup(&word.to_lowercase());
            if !tagged.is_empty() {
                let tagged = adjust(&tagged, None, None, ":alt");
                return self.tagged_to_at(word, &tagged);
            }
        }
        // «з» замість «с» перед губними
        if word.chars().count() > 5 && full_match(&Z_KPTF, word) {
            let new_word = word.replacen('з', "с", 1).replacen('З', "С", 1);
            let tagged = self.tag_both_cases(&new_word);
            if !tagged.is_empty() {
                let mapped: Vec<(String, String)> = tagged
                    .into_iter()
                    .map(|(lemma, tag)| {
                        let lemma = lemma.replacen('с', "з", 1).replacen('С', "З", 1);
                        (lemma, add_if_not_contains(&tag, ":alt"))
                    })
                    .collect();
                return self.tagged_to_at(word, &mapped);
            }
        }
        // дївчина
        if word.chars().count() > 3 && word.contains('ї') {
            let word2 = YI_PATTERN.replace_all(word, "${1}і").into_owned();
            let tagged = self.word_lookup(&word2);
            if !tagged.is_empty() {
                let tagged = adjust(&tagged, None, None, ":alt");
                return self.tagged_to_at(word, &tagged);
            }
        }
        if word.chars().count() > 4 && MISSING_APO.is_match(word).unwrap_or(false) {
            let adjusted = MISSING_APO.replace(word, "$1'$2").into_owned();
            let tagged: Vec<(String, String)> = self
                .word_lookup(&adjusted)
                .into_iter()
                .filter(|(_, tag)| full_match(&NOT_BAD_POS, tag))
                .collect();
            if !tagged.is_empty() {
                let tagged: Vec<(String, String)> = tagged
                    .into_iter()
                    .map(|(lemma, tag)| (lemma, add_if_not_contains(&tag, ":bad")))
                    .collect();
                return self.tagged_to_at(word, &tagged);
            }
        }
        if word.chars().count() > 5 {
            if let Some(caps) = full_captures(&MISSING_HYPHEN, word) {
                let head = caps[1].to_lowercase();
                let tagged = self.word_lookup(&head);
                if !tagged.is_empty() && tagged.iter().any(|(_, tag)| tag.contains("pron")) {
                    let suffix = format!("-{}", caps[2].to_lowercase());
                    let tagged = adjust(&tagged, None, Some(&suffix), ":bad");
                    return self.tagged_to_at(word, &tagged);
                }
            }
        }

        let word = IGNORED_CHARS.replace_all(word, "").into_owned();

        if word.chars().count() >= 3 && word.find('-').is_some_and(|i| i > 0) {
            // екс-«депутат», "заступницю"-колаборантку
            if word.chars().count() >= 6
                && (COMPOUND_WITH_QUOTES_REGEX.is_match(&word).unwrap_or(false)
                    || COMPOUND_WITH_QUOTES_REGEX2.is_match(&word).unwrap_or(false))
            {
                let adjusted_word = QUOTES.replace_all(&word, "").into_owned();
                return self.get_adjusted_analyzed_tokens(
                    &word,
                    &adjusted_word,
                    None,
                    None,
                    &|l| l.to_string(),
                );
            }
            // Java returns here regardless (`compoundTagger.guessCompoundTag`);
            // the deeper dash-compound branches are not ported yet.
            return self
                .compound
                .guess_compound_tag(self, &word)
                .unwrap_or_default();
        }

        // стодвадцятиріччя
        if word.chars().count() >= 10 {
            if let Some(caps) = full_captures(&RICCHA, &word) {
                let end_word = caps.get(4).map(|m| m.as_str()).unwrap_or("");
                let right = self.word_lookup(&format!("сто{end_word}"));
                if right.is_empty() {
                    return Vec::new();
                }
                if !self.is_all_num(&caps, 1, 3) {
                    return Vec::new();
                }
                let mut out = Vec::new();
                for (lemma, tag) in &right {
                    if tag.contains("v_kly") || tag.contains(":p:") {
                        continue;
                    }
                    let lemma =
                        concat_groups(&caps, 1, 3) + &lemma.chars().skip(3).collect::<String>();
                    out.push(AnalyzedToken::new(
                        word.clone(),
                        Some(lemma),
                        Some(tag.clone()),
                    ));
                }
                return out;
            } else if let Some(caps) = full_captures(&OTYI, &word) {
                let end_word = caps.get(4).map(|m| m.as_str()).unwrap_or("");
                let right = self.word_lookup(end_word);
                if right.is_empty() {
                    return Vec::new();
                }
                if !self.is_all_num(&caps, 1, 3) {
                    return Vec::new();
                }
                let mut out = Vec::new();
                for (lemma, tag) in &right {
                    if !tag.starts_with("adj") || tag.contains("v_kly") {
                        continue;
                    }
                    let lemma = concat_groups(&caps, 1, 3) + lemma;
                    out.push(AnalyzedToken::new(
                        word.clone(),
                        Some(lemma),
                        Some(tag.clone()),
                    ));
                }
                return out;
            }
        }

        // compoundTagger.guessOtherTags(word) — stage 3c part 2.
        Vec::new()
    }

    /// `CompoundTagger.tagBothCases(leftWord, null)`.
    fn tag_both_cases(&self, word: &str) -> Vec<(String, String)> {
        let mut out = self.word_lookup(word);
        let lower = word.to_lowercase();
        if word != lower {
            out.extend(self.word_lookup(&lower));
        } else {
            let upper = crate::english::uppercase_first_char(word);
            if word != upper {
                out.extend(self.word_lookup(&upper));
            }
        }
        out
    }

    /// `CompoundTagger.tagEitherCase`.
    pub(crate) fn tag_either_case(&self, word: &str) -> Vec<(String, String)> {
        if word.is_empty() {
            return Vec::new();
        }
        let mut out = self.word_lookup(word);
        if out.is_empty() && word.chars().next().is_some_and(|c| c.is_uppercase()) {
            out = self.word_lookup(&word.to_lowercase());
        }
        out
    }

    /// `CompoundTagger.tagAsIsAndWithLowerCase`.
    pub(crate) fn tag_as_is_and_with_lowercase(&self, word: &str) -> Vec<(String, String)> {
        let mut out = self.word_lookup(word);
        let lower = word.to_lowercase();
        if word != lower {
            out.extend(self.word_lookup(&lower));
        } else {
            let upper = crate::english::uppercase_first_char(word);
            if word != upper {
                out.extend(self.word_lookup(&upper));
            }
        }
        out
    }

    fn is_all_num(&self, caps: &fancy_regex::Captures, from: usize, to: usize) -> bool {
        for ii in from..=to {
            let group = caps.get(ii).map(|m| m.as_str()).unwrap_or("");
            if !group.is_empty() {
                let tagged = self.word_lookup(&group.to_lowercase());
                let has_num = tagged.iter().any(|(_, tag)| tag.contains("num"));
                let has_bad = tagged
                    .iter()
                    .any(|(_, tag)| tag.contains("bad") || tag.contains("subst"));
                if !has_num || has_bad {
                    return false;
                }
            }
        }
        true
    }

    /// `UkrainianTagger.getAnalyzedTokens`.
    fn get_analyzed_tokens(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut word = word.to_string();
        if word.find('`').is_some_and(|i| i > 0) {
            word = word.replace('`', "'");
        }
        let mut tokens = self.base_get_analyzed_tokens(&word);

        if word.chars().count() < 2 {
            return tokens;
        }
        if tokens[0].pos_tag.is_none() {
            let orig_word = word.clone();
            if word.chars().count() > 2 {
                if word.find('\u{2013}').is_some_and(|i| i > 0)
                    && ALT_DASHES_IN_WORD.is_match(&word).unwrap_or(false)
                {
                    word = orig_word.replace('\u{2013}', "-");
                    let new_tokens = self.base_get_analyzed_tokens(&word);
                    if !new_tokens.is_empty() && new_tokens[0].pos_tag.is_some() {
                        let mut new_tokens = new_tokens;
                        new_tokens.push(AnalyzedToken::new(orig_word.clone(), None, None));
                        tokens = new_tokens;
                    }
                } else if word.contains('ґ') || word.contains('Ґ') {
                    tokens = self.convert_tokens(&tokens, &word, "ґ", "г", ":alt");
                } else if word.contains("ія") {
                    tokens = self.convert_tokens(&tokens, &word, "ія", "іа", ":alt");
                } else if word.ends_with("тер") {
                    tokens = self.convert_tokens(&tokens, &word, "тер", "тр", ":alt");
                } else if word.contains("льо") {
                    tokens = self.convert_tokens(&tokens, &word, "льо", "ло", ":alt");
                } else if word.starts_with("сьвя") {
                    tokens = self.convert_tokens(&tokens, &word, "сьвя", "свя", ":arch");
                } else if word.starts_with("сьві") {
                    tokens = self.convert_tokens(&tokens, &word, "сьві", "сві", ":arch");
                } else if word.contains("ьск") && !word.ends_with("ская") && word != "Комсомольском"
                {
                    tokens = self.convert_tokens(&tokens, &word, "ьск", "ьськ", ":bad");
                }

                if tokens[0].pos_tag.is_none() && word.chars().count() >= 3 {
                    if word.chars().count() >= 9 {
                        if let Ok(Some(caps)) = LEFT_O_ADJ_INVALID_PATTERN.captures(&word) {
                            let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                            let adjusted_word = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                            let new_tokens = self.get_adjusted_analyzed_tokens(
                                &word,
                                adjusted_word,
                                Some(&ADJ_POS),
                                None,
                                &|lemma| format!("{prefix}{lemma}"),
                            );
                            if !new_tokens.is_empty() {
                                tokens = new_tokens;
                            }
                        }
                    }
                    // гааа
                    if tokens[0].pos_tag.is_none() && !word.eq_ignore_ascii_case("ііі") {
                        if let Ok(Some(caps)) = REPEATED_VOWEL.captures(&word) {
                            let adjusted_word =
                                REPEATED_VOWEL.replace_all(&word, "$1").into_owned();
                            let _ = caps;
                            let new_tokens = self.get_adjusted_analyzed_tokens(
                                &word,
                                &adjusted_word,
                                Some(&REPEATED_OK_POS),
                                Some(":alt"),
                                &|lemma| lemma.to_string(),
                            );
                            if !new_tokens.is_empty() {
                                tokens = new_tokens;
                            }
                        }
                    }
                    if tokens[0].pos_tag.is_none()
                        && word.contains('[')
                        && word.contains(']')
                        && WORDS_WITH_BRACKETS.is_match(&word).unwrap_or(false)
                    {
                        let adjusted_word = word.replace(['[', ']'], "");
                        let new_tokens = self.get_adjusted_analyzed_tokens(
                            &word,
                            &adjusted_word,
                            None,
                            Some(":alt"),
                            &|lemma| lemma.to_string(),
                        );
                        if !new_tokens.is_empty() {
                            tokens = new_tokens;
                        }
                    }
                }
            }
        }

        // try УКРАЇНА as Україна
        if word.chars().count() > 2 && is_all_uppercase_uk(&word) {
            let new_word = capitalize_proper_name(&word);
            let new_tokens = self.get_adjusted_analyzed_tokens(
                &word,
                &new_word,
                Some(&PROP_POS),
                None,
                &|lemma| lemma.to_string(),
            );
            if !new_tokens.is_empty() {
                if tokens[0].pos_tag.is_none() {
                    tokens = new_tokens;
                } else {
                    for t in new_tokens {
                        if !tokens.contains(&t) {
                            tokens.push(t);
                        }
                    }
                }
            }
        }

        let analyzed = self.analyze_all_capitalized_adj(&word);
        if !analyzed.is_empty() {
            if tokens[0].pos_tag.is_none() {
                tokens = analyzed;
            } else {
                for t in analyzed {
                    if !tokens.contains(&t) {
                        tokens.push(t);
                    }
                }
            }
        }

        tokens
    }

    fn analyze_all_capitalized_adj(&self, word: &str) -> Vec<AnalyzedToken> {
        if word.find('-').is_some_and(|i| i > 1) && !word.ends_with('-') {
            let parts: Vec<&str> = word.split('-').collect();
            if parts.iter().all(|p| crate::uk_helpers::is_capitalized(p)) {
                let lower = word.to_lowercase();
                let tagged = self.word_lookup(&lower);
                if tagged.iter().any(|(_, tag)| tag.contains("adj")) {
                    return self
                        .tagged_to_at(word, &tagged)
                        .into_iter()
                        .filter(|t| full_match(&ADJ_POS, t.pos_tag.as_deref().unwrap_or("")))
                        .collect();
                }
            }
        }
        Vec::new()
    }

    fn convert_tokens(
        &self,
        orig_tokens: &[AnalyzedToken],
        word: &str,
        s: &str,
        dict_s: &str,
        additional_tag: &str,
    ) -> Vec<AnalyzedToken> {
        let mut adjusted_word = word.replace(s, dict_s);
        if s.chars().count() == 1 {
            adjusted_word = adjusted_word.replace(&s.to_uppercase(), &dict_s.to_uppercase());
        }
        let new_tokens = self.get_adjusted_analyzed_tokens(
            word,
            &adjusted_word,
            None,
            Some(additional_tag),
            &|lemma| lemma.replace(dict_s, s),
        );
        if new_tokens.is_empty() {
            orig_tokens.to_vec()
        } else {
            new_tokens
        }
    }

    fn get_adjusted_analyzed_tokens(
        &self,
        word: &str,
        adjusted_word: &str,
        pos_tag_regex: Option<&Regex>,
        additional_tag: Option<&str>,
        lemma_function: &dyn Fn(&str) -> String,
    ) -> Vec<AnalyzedToken> {
        let new_tokens = self.base_get_analyzed_tokens(adjusted_word);
        if new_tokens.is_empty() || new_tokens[0].pos_tag.is_none() {
            return Vec::new();
        }
        let mut derived = Vec::new();
        for at in &new_tokens {
            let pos_tag = at.pos_tag.as_deref().unwrap_or("");
            if at.token == adjusted_word && pos_tag_regex.is_none_or(|re| full_match(re, pos_tag)) {
                let lemma = at.stem.as_deref().unwrap_or("");
                let lemma = lemma_function(lemma);
                let pos_tag = match additional_tag {
                    Some(tag) => add_if_not_contains(pos_tag, tag),
                    None => pos_tag.to_string(),
                };
                derived.push(AnalyzedToken::new(word, Some(lemma), Some(pos_tag)));
            }
        }
        derived
    }

    /// `UkrainianTagger.tag_word` (`getAnalyzedTokens` with the override).
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        self.get_analyzed_tokens(word)
    }

    /// `UkrainianTagger.INSTANCE.tag([word]).get(0).isTagged()`.
    pub fn is_tagged_word(&self, word: &str) -> bool {
        self.tag_word(word).iter().any(|t| t.pos_tag.is_some())
    }

    /// `BaseTagger.tag(List<String>)`: one entry per input token with the
    /// cumulative Java UTF-16 position (the pipeline overrides `start_pos`
    /// with byte offsets).
    pub fn tag(&self, sentence_tokens: &[String]) -> Vec<AnalyzedTokenReadings> {
        let mut out = Vec::with_capacity(sentence_tokens.len());
        let mut pos = 0usize;
        for raw in sentence_tokens {
            let readings = self.tag_word(raw);
            let mut atr = AnalyzedTokenReadings::new(readings);
            atr.start_pos = pos;
            atr.raw_byte_len = raw.len();
            out.push(atr);
            pos += raw.encode_utf16().count();
        }
        out
    }
}

/// Java `Matcher.matches()` with captures.
fn full_captures<'r>(re: &Regex, s: &'r str) -> Option<fancy_regex::Captures<'r>> {
    let caps = re.captures(s).ok().flatten()?;
    let m = caps.get(0)?;
    if m.start() == 0 && m.end() == s.len() {
        Some(caps)
    } else {
        None
    }
}

/// `UkrainainTagger.concatGroups`.
fn concat_groups(caps: &fancy_regex::Captures, i: usize, j: usize) -> String {
    let mut out = String::new();
    for ii in i..=j {
        if let Some(m) = caps.get(ii) {
            out.push_str(m.as_str());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tagger() -> Option<UkrainianTagger> {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !data.is_dir() {
            return None;
        }
        UkrainianTagger::load(&data).ok()
    }

    fn readings(t: &UkrainianTagger, word: &str) -> Vec<String> {
        t.tag_word(word)
            .iter()
            .filter(|a| a.pos_tag.is_some())
            .map(|a| {
                format!(
                    "{}:{}",
                    a.stem.as_deref().unwrap_or(""),
                    a.pos_tag.as_deref().unwrap_or("")
                )
            })
            .collect()
    }

    /// Java-probed (`scripts/oracle/uk/probe-tagger.sh`): the `additionalTags`
    /// patterns that do not depend on `CompoundTagger`.
    #[test]
    fn ukrainian_additional_tags() {
        let Some(t) = tagger() else { return };
        assert_eq!(readings(&t, "123"), ["123:number"]);
        assert_eq!(readings(&t, "5,5"), ["5,5:number"]);
        assert_eq!(readings(&t, "2 000 000"), ["2 000 000:number"]);
        assert_eq!(readings(&t, "12:25"), ["12:25:time"]);
        assert_eq!(readings(&t, "28.05.2019"), ["28.05.2019:date"]);
        assert_eq!(readings(&t, "#тег"), ["#тег:hashtag"]);
        // LATIN_NUMBER_CYR with an ordinal ending stays `number:latin:bad`
        assert_eq!(readings(&t, "X-й"), ["X-й:number:latin:bad"]);
    }

    /// Java-probed alternates: `З`-for-`С`, `ї`-for-`і`, and the
    /// `getAnalyzedTokens` all-uppercase / compound-adjective branches.
    #[test]
    fn ukrainian_tag_alternates() {
        let Some(t) = tagger() else { return };
        assert_eq!(readings(&t, "зкривити"), ["зкривити:verb:perf:inf:alt"]);
        assert_eq!(readings(&t, "дївчина"), ["дівчина:noun:anim:f:v_naz:alt"]);
        assert_eq!(
            readings(&t, "Івано-Франківська"),
            [
                "Івано-Франківськ:noun:inanim:m:v_rod:prop:geo",
                "івано-франківський:adj:f:v_kly",
                "івано-франківський:adj:f:v_naz"
            ]
        );
        assert_eq!(
            readings(&t, "стодвадцятиріччя"),
            [
                "стодвадцятиріччя:noun:inanim:n:v_naz",
                "стодвадцятиріччя:noun:inanim:n:v_rod",
                "стодвадцятиріччя:noun:inanim:n:v_zna"
            ]
        );
        // unknown words stay untagged (the CompoundTagger branches are not
        // ported yet)
        assert!(readings(&t, "ІванІван").is_empty());
        assert!(readings(&t, "пять").is_empty());
        assert!(readings(&t, "небудьщо").is_empty());
    }

    /// Java-probed (`scripts/oracle/uk/probe-tagger.sh`): the entity and
    /// numeric-compound `CompoundTagger` paths.
    #[test]
    fn ukrainian_compounds() {
        let Some(t) = tagger() else { return };
        assert_eq!(
            readings(&t, "5-й"),
            [
                "5-й:adj:m:v_naz:numr",
                "5-й:adj:m:v_zna:rinanim:numr",
                "5-й:adj:f:v_dav:numr",
                "5-й:adj:f:v_mis:numr"
            ]
        );
        assert_eq!(
            readings(&t, "100-річному"),
            [
                "100-річний:adj:m:v_dav",
                "100-річний:adj:m:v_mis",
                "100-річний:adj:n:v_dav",
                "100-річний:adj:n:v_mis"
            ]
        );
        assert_eq!(readings(&t, "Ан-140").len(), 12);
        assert_eq!(readings(&t, "А-4").len(), 13);
        assert_eq!(
            readings(&t, "Вибори-2014"),
            [
                "Вибори-2014:noun:inanim:p:v_naz:ns:prop",
                "Вибори-2014:noun:inanim:p:v_zna:ns:prop"
            ]
        );
        assert_eq!(
            readings(&t, "Формула-1"),
            ["Формула-1:noun:inanim:f:v_naz:prop"]
        );
        assert_eq!(
            readings(&t, "авто-пенсіонер"),
            ["авто-пенсіонер:noun:anim:m:v_naz:bad"]
        );
        assert_eq!(
            readings(&t, "з-зателефоную"),
            ["зателефонувати:verb:perf:futr:s:1:alt"]
        );
        assert_eq!(readings(&t, "ла-ла"), ["ла-ла:noninfl:onomat:predic"]);
    }
}
