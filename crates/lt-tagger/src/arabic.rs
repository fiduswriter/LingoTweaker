//! Arabic tagger: port of `org.languagetool.tagging.ar.ArabicTagger`, a
//! `BaseTagger` over the Arramooz-derived `arabic.dict` Morfologik dictionary,
//! plus its `ArabicTagManager` flag algebra.
//!
//! The tagger strips tashkeel before the lookup (`ArabicStringTools
//! .removeTashkeel`), then adds the affix-derived readings: every
//! prefix-index × suffix-index pair strips the affixes, looks the stem up in
//! the dictionary and rewrites the POS tag through [`ArabicTagManager`].
//!
//! The dictionary uses `fsa.dict.separator=+`, `fsa.dict.encoding=utf-8`,
//! `fsa.dict.encoder=SUFFIX` (CFSA2, no frequency data).

use std::path::Path;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Result};

use crate::{Dictionary, DictionaryInfo, ManualTagger};

/// `ArabicStringTools.removeTashkeel`: the 12 tashkeel characters plus
/// tatweel (`ArabicStringTools.TASHKEEL_CHARS`).
const TASHKEEL_CHARS: [char; 13] = [
    '\u{064B}', '\u{064C}', '\u{064D}', '\u{064E}', '\u{064F}', '\u{0650}', '\u{0651}', '\u{0652}',
    '\u{0653}', '\u{0654}', '\u{0655}', '\u{0656}', '\u{0640}',
];

/// `ArabicStringTools.removeTashkeel`.
pub fn remove_tashkeel(text: &str) -> String {
    text.chars()
        .filter(|c| !TASHKEEL_CHARS.contains(c))
        .collect()
}

/// `org.languagetool.tagging.ar.ArabicTagManager`: the positional flag algebra
/// over the fixed-length POS tag strings (`NA-;-3--;---`, `V31;M1H-pa-;---`,
/// `PRD;---;---`, …).
#[derive(Debug, Default)]
pub struct ArabicTagManager;

impl ArabicTagManager {
    /// `ArabicTagManager.isNoun`.
    pub fn is_noun(postag: &str) -> bool {
        postag.starts_with('N')
    }

    /// `ArabicTagManager.isVerb`.
    pub fn is_verb(postag: &str) -> bool {
        postag.starts_with('V')
    }

    /// `ArabicTagManager.isStopWord`.
    pub fn is_stop_word(postag: &str) -> bool {
        postag.starts_with('P')
    }

    /// `ArabicTagManager.isAdj`.
    pub fn is_adj(postag: &str) -> bool {
        postag.starts_with("NA")
    }

    /// `ArabicTagManager.isMasdar`.
    pub fn is_masdar(postag: &str) -> bool {
        postag.starts_with("NM")
    }

    /// `ArabicTagManager.getFlagPos2`: `NOUN_*` / `VERB_*` / `PARTICLE_*`
    /// position table.
    fn flag_pos(postag: &str, flag_type: &str) -> usize {
        let key = if Self::is_noun(postag) {
            format!("NOUN_{flag_type}")
        } else if Self::is_verb(postag) {
            format!("VERB_{flag_type}")
        } else if Self::is_stop_word(postag) {
            format!("PARTICLE_{flag_type}")
        } else {
            return 0;
        };
        match key.as_str() {
            "NOUN_TAG_LENGTH" => 12,
            "NOUN_WORDTYPE" => 0,
            "NOUN_CATEGORY" => 1,
            "NOUN_GENDER" => 4,
            "NOUN_NUMBER" => 5,
            "NOUN_CASE" => 6,
            "NOUN_INFLECT_MARK" => 7,
            "NOUN_CONJ" => 9,
            "NOUN_JAR" => 10,
            "NOUN_PRONOUN" => 11,
            "VERB_TAG_LENGTH" => 15,
            "VERB_WORDTYPE" => 0,
            "VERB_CATEGORY" => 1,
            "VERB_TRANS" => 2,
            "VERB_GENDER" => 4,
            "VERB_NUMBER" => 5,
            "VERB_PERSON" => 6,
            "VERB_INFLECT_MARK" => 7,
            "VERB_TENSE" => 8,
            "VERB_VOICE" => 9,
            "VERB_CASE" => 10,
            "VERB_CONJ" => 12,
            "VERB_ISTIQBAL" => 13,
            "VERB_PRONOUN" => 14,
            "PARTICLE_TAG_LENGTH" => 11,
            "PARTICLE_WORDTYPE" => 0,
            "PARTICLE_CATEGORY" => 1,
            "PARTICLE_OPTION" => 2,
            "PARTICLE_CONJ" => 8,
            "PARTICLE_JAR" => 9,
            "PARTICLE_PRONOUN" => 10,
            _ => 0,
        }
    }

    /// `ArabicTagManager.getFlag`: `'-'` when the tag is too short.
    pub fn get_flag(postag: &str, flag_type: &str) -> char {
        let pos = Self::flag_pos(postag, flag_type);
        postag
            .as_bytes()
            .get(pos)
            .map(|&b| b as char)
            .unwrap_or('-')
    }

    /// `ArabicTagManager.setFlag`: out-of-range positions leave the tag
    /// unchanged (Java catches `StringIndexOutOfBoundsException`).
    pub fn set_flag(postag: &str, flag_type: &str, flag: char) -> String {
        let pos = Self::flag_pos(postag, flag_type);
        let mut bytes = postag.as_bytes().to_vec();
        if pos < bytes.len() {
            bytes[pos] = flag as u8;
            String::from_utf8(bytes).unwrap_or_else(|_| postag.to_string())
        } else {
            postag.to_string()
        }
    }

    /// `ArabicTagManager.isMajrour`.
    pub fn is_majrour(postag: &str) -> bool {
        let flag = Self::get_flag(postag, "CASE");
        flag == 'I' || flag == '-'
    }

    /// `ArabicTagManager.isUnAttachedNoun`.
    pub fn is_unattached_noun(postag: &str) -> bool {
        Self::is_noun(postag) && Self::get_flag(postag, "PRONOUN") != 'H' && !postag.ends_with('X')
    }

    /// `ArabicTagManager.isAttached`.
    pub fn is_attached(postag: &str) -> bool {
        (Self::is_noun(postag) || Self::is_verb(postag)) && Self::get_flag(postag, "PRONOUN") == 'H'
    }

    /// `ArabicTagManager.isDefinite`.
    pub fn is_definite(postag: &str) -> bool {
        Self::is_noun(postag) && Self::get_flag(postag, "PRONOUN") == 'L'
    }

    /// `ArabicTagManager.isFeminin`.
    pub fn is_feminin(postag: &str) -> bool {
        Self::is_noun(postag) && Self::get_flag(postag, "GENDER") == 'F'
    }

    /// `ArabicTagManager.isDual`.
    pub fn is_dual(postag: &str) -> bool {
        Self::get_flag(postag, "NUMBER") == '2'
    }

    /// `ArabicTagManager.isFutureTense`.
    pub fn is_future_tense(postag: &str) -> bool {
        Self::is_verb(postag) && Self::get_flag(postag, "TENSE") == 'f'
    }

    /// `ArabicTagManager.hasJar`.
    pub fn has_jar(postag: &str) -> bool {
        Self::is_noun(postag) && Self::get_flag(postag, "JAR") != '-'
    }

    /// `ArabicTagManager.hasPronoun`.
    pub fn has_pronoun(postag: &str) -> bool {
        Self::get_flag(postag, "PRONOUN") == 'H'
    }

    /// `ArabicTagManager.hasConjunction`.
    pub fn has_conjunction(postag: &str) -> bool {
        let flag = Self::get_flag(postag, "CONJ");
        (Self::is_noun(postag) && flag != '-')
            || (Self::is_verb(postag) && flag != '-')
            || (Self::is_stop_word(postag) && flag != 'W')
    }

    /// `ArabicTagManager.addTag(postag, flagType, flag)`.
    pub fn add_tag(postag: &str, flag_type: &str, flag: &str) -> Option<String> {
        let mut postag = postag.to_string();
        match flag {
            "W" => postag = Self::set_flag(&postag, "CONJ", 'W'),
            "K" => {
                if Self::is_noun(&postag) {
                    if Self::is_majrour(&postag) {
                        postag = Self::set_flag(&postag, "JAR", 'K');
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            "B" => {
                if Self::is_noun(&postag) {
                    if Self::is_majrour(&postag) {
                        postag = Self::set_flag(&postag, "JAR", 'B');
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            "L" => {
                if Self::is_noun(&postag) {
                    if Self::is_majrour(&postag) {
                        postag = Self::set_flag(&postag, "JAR", 'L');
                    } else {
                        return None;
                    }
                } else {
                    postag = Self::set_flag(&postag, "ISTIQBAL", 'L');
                }
            }
            "D" => {
                if Self::is_unattached_noun(&postag) {
                    postag = Self::set_flag(&postag, "PRONOUN", 'L');
                } else {
                    return None;
                }
            }
            "S" => {
                if Self::is_future_tense(&postag) {
                    postag = Self::set_flag(&postag, "ISTIQBAL", 'S');
                } else {
                    return None;
                }
            }
            _ => {}
        }
        if flag_type == "PRONOUN" && !flag.is_empty() && flag != "D" {
            postag = Self::set_flag(&postag, flag_type, flag.chars().next().unwrap());
        }
        Some(postag)
    }

    /// `ArabicTagManager.addTag(postag, "TYPE;FLAG")`.
    pub fn add_tag_string(postag: &str, flag_string: &str) -> Option<String> {
        let mut parts = flag_string.split(';');
        let first = parts.next().unwrap_or("");
        match parts.next() {
            None => Self::add_tag(postag, "", first),
            Some(flag) => Self::add_tag(postag, first, flag),
        }
    }

    /// `ArabicTagManager.modifyPosTag`: applies every tag in order, aborting on
    /// the first incompatible one.
    pub fn modify_pos_tag(postag: &str, tags: &[String]) -> Option<String> {
        let mut postag = postag.to_string();
        for tag in tags {
            postag = Self::add_tag_string(&postag, tag)?;
        }
        Some(postag)
    }

    /// `ArabicTagManager.setJar`.
    pub fn set_jar(postag: &str, jar: &str) -> String {
        if !Self::is_majrour(postag) {
            return postag.to_string();
        }
        let flag = match jar {
            "ب" | "B" => Some('B'),
            "ل" | "L" => Some('L'),
            "ك" | "K" => Some('K'),
            "-" | "" => Some('-'),
            _ => None,
        };
        match flag {
            Some(f) => Self::set_flag(postag, "JAR", f),
            None => postag.to_string(),
        }
    }

    /// `ArabicTagManager.setDefinite`.
    pub fn set_definite(postag: &str, flag: &str) -> String {
        if !(Self::is_noun(postag) && Self::is_unattached_noun(postag)) {
            return postag.to_string();
        }
        let myflag = match flag {
            "ال" | "L" | "لل" | "D" => Some('L'),
            "-" | "" => Some('-'),
            _ => None,
        };
        match myflag {
            Some(f) => Self::set_flag(postag, "PRONOUN", f),
            None => postag.to_string(),
        }
    }

    /// `ArabicTagManager.unifyPronounTag`.
    pub fn unify_pronoun_tag(postag: &str) -> String {
        if Self::is_attached(postag) {
            Self::set_flag(postag, "PRONOUN", 'H')
        } else {
            postag.to_string()
        }
    }

    /// `ArabicTagManager.setConjunction`.
    pub fn set_conjunction(postag: &str, flag: &str) -> String {
        let myflag = match flag {
            "و" | "W" | "ف" | "F" => Some('W'),
            "-" | "" => Some('-'),
            _ => None,
        };
        match myflag {
            Some(f) if Self::is_noun(postag) || Self::is_verb(postag) => {
                Self::set_flag(postag, "CONJ", f)
            }
            _ => postag.to_string(),
        }
    }

    /// `ArabicTagManager.setPronoun`.
    pub fn set_pronoun(postag: &str, flag: &str) -> String {
        if (flag == "ه" || flag == "H") && (Self::is_noun(postag) || Self::is_verb(postag)) {
            Self::set_flag(postag, "PRONOUN", 'H')
        } else {
            postag.to_string()
        }
    }

    /// `ArabicTagManager.getDefinitePrefix`.
    pub fn get_definite_prefix(postag: &str) -> String {
        if postag.is_empty() {
            return String::new();
        }
        if Self::is_noun(postag) && Self::get_flag(postag, "PRONOUN") == 'L' {
            if Self::has_jar(postag) && Self::get_jar_prefix(postag) == "ل" {
                "ل".to_string()
            } else {
                "ال".to_string()
            }
        } else {
            String::new()
        }
    }

    /// `ArabicTagManager.getJarPrefix`.
    pub fn get_jar_prefix(postag: &str) -> String {
        if postag.is_empty() {
            return String::new();
        }
        if Self::is_noun(postag) {
            match Self::get_flag(postag, "JAR") {
                'L' => return "ل".to_string(),
                'K' => return "ك".to_string(),
                'B' => return "ب".to_string(),
                _ => {}
            }
        }
        String::new()
    }

    /// `ArabicTagManager.getConjunctionPrefix`.
    pub fn get_conjunction_prefix(postag: &str) -> String {
        match Self::get_flag(postag, "CONJ") {
            'F' => "ف".to_string(),
            'W' => "و".to_string(),
            _ => String::new(),
        }
    }

    /// `ArabicTagManager.getPronounSuffix`.
    pub fn get_pronoun_suffix(postag: &str) -> String {
        if postag.is_empty() {
            return String::new();
        }
        match Self::get_flag(postag, "PRONOUN") {
            'b' => "ني",
            'c' => "نا",
            'd' => "ك",
            'e' => "كما",
            'f' => "كم",
            'g' => "كن",
            'H' => "ه",
            'i' => "ها",
            'j' => "هما",
            'k' => "هم",
            'n' => "هن",
            _ => "",
        }
        .to_string()
    }

    /// `ArabicTagManager.setProcleticFlags`.
    pub fn set_procletic_flags(postag: &str) -> String {
        if postag.is_empty() {
            return String::new();
        }
        if Self::is_verb(postag) {
            let p = Self::set_flag(postag, "CONJ", '-');
            Self::set_flag(&p, "ISTIQBAL", '-')
        } else if Self::is_noun(postag) {
            let mut p = Self::set_flag(postag, "CONJ", '-');
            p = Self::set_flag(&p, "JAR", '-');
            if Self::is_definite(postag) {
                p = Self::set_flag(&p, "PRONOUN", '-');
            }
            p
        } else if Self::is_stop_word(postag) {
            let p = Self::set_flag(postag, "CONJ", '-');
            Self::set_flag(&p, "JAR", '-')
        } else {
            postag.to_string()
        }
    }

    /// `ArabicTagManager.setMajrour`.
    pub fn set_majrour(postag: &str) -> String {
        if Self::is_noun(postag) {
            if Self::is_dual(postag) {
                Self::set_flag(postag, "CASE", 'A')
            } else {
                Self::set_flag(postag, "CASE", 'I')
            }
        } else {
            postag.to_string()
        }
    }

    /// `ArabicTagManager.setMarfou3`.
    pub fn set_marfou3(postag: &str) -> String {
        Self::set_flag(postag, "CASE", 'U')
    }

    /// `ArabicTagManager.setMansoub`.
    pub fn set_mansoub(postag: &str) -> String {
        Self::set_flag(postag, "CASE", 'A')
    }

    /// `ArabicTagManager.setSingle`.
    pub fn set_single(postag: &str) -> String {
        Self::set_flag(postag, "NUMBER", '1')
    }

    /// `ArabicTagManager.setDual`.
    pub fn set_dual(postag: &str) -> String {
        Self::set_flag(postag, "NUMBER", '2')
    }

    /// `ArabicTagManager.setPlural`.
    pub fn set_plural(postag: &str) -> String {
        Self::set_flag(postag, "NUMBER", '3')
    }

    /// `ArabicTagManager.setTanwin`.
    pub fn set_tanwin(postag: &str) -> String {
        Self::set_flag(postag, "PRONOUN", 'n')
    }

    /// `ArabicTagManager.mergePosTag`.
    pub fn merge_pos_tag(source: &str, target: &str) -> String {
        if source.is_empty() {
            return target.to_string();
        }
        if target.is_empty() {
            return source.to_string();
        }
        if Self::is_noun(source) && Self::is_noun(target) {
            if source.len() != target.len() {
                return source.to_string();
            }
            return Self::set_flag(source, "CATEGORY", Self::get_flag(target, "CATEGORY"));
        }
        if Self::is_verb(source) && Self::is_verb(target) {
            if source.len() != target.len() {
                return source.to_string();
            }
            let tmp = Self::set_flag(source, "CATEGORY", Self::get_flag(target, "CATEGORY"));
            return Self::set_flag(&tmp, "TRANS", Self::get_flag(target, "TRANS"));
        }
        if Self::is_stop_word(source) && Self::is_stop_word(target) {
            if source.len() != target.len() {
                return source.to_string();
            }
            let tmp = Self::set_flag(source, "CATEGORY", Self::get_flag(target, "CATEGORY"));
            return Self::set_flag(&tmp, "OPTION", Self::get_flag(target, "OPTION"));
        }
        if (Self::is_stop_word(source) && (Self::is_verb(target) || Self::is_noun(target)))
            || ((Self::is_verb(source) || Self::is_noun(source)) && Self::is_stop_word(target))
        {
            let mut tmp = target.to_string();
            if Self::has_pronoun(source) {
                tmp = Self::set_flag(&tmp, "PRONOUN", Self::get_flag(source, "PRONOUN"));
            }
            return tmp;
        }
        if (Self::is_verb(source) && Self::is_noun(target))
            || (Self::is_noun(source) && Self::is_verb(target))
        {
            let mut tmp = target.to_string();
            if Self::has_pronoun(source) {
                tmp = Self::set_flag(&tmp, "PRONOUN", Self::get_flag(source, "PRONOUN"));
            }
            tmp = Self::set_flag(&tmp, "CONJ", Self::get_flag(source, "CONJ"));
            return tmp;
        }
        target.to_string()
    }
}

/// `java.lang.String.substring` with char (UTF-16 code unit) indices.
fn substring(word: &str, start: usize, end: usize) -> String {
    word.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

/// `ArabicTagger` over `arabic.dict` (a `BaseTagger`, so the word tagger is a
/// `CombiningTagger` over the manual `added`/`added_custom` and
/// `removed`/`removed_custom` lists).
#[derive(Debug)]
pub struct ArabicTagger {
    dict: Dictionary,
    tagmanager: ArabicTagManager,
    /// `CombiningTagger` second tagger (`ar/added.txt` + `added_custom.txt`).
    manual: ManualTagger,
    /// `CombiningTagger` removal tagger (`removed.txt` + `removed_custom.txt`).
    removals: ManualTagger,
}

impl ArabicTagger {
    /// Load `ar/dictionaries/arabic.{dict,info}` plus the manual word lists.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("ar/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("arabic.info"))?;
        let dict = Dictionary::load(&dict_dir.join("arabic.dict"), &info)?;
        let words_dir = data_dir.join("ar/words");
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
            tagmanager: ArabicTagManager,
            manual,
            removals,
        })
    }

    pub fn dict(&self) -> &Dictionary {
        &self.dict
    }

    pub fn tagmanager(&self) -> &ArabicTagManager {
        &self.tagmanager
    }

    /// `CombiningTagger.tag`: manual readings first, then the Morfologik
    /// dictionary (`overwriteWithManualTagger` is false), then the removals.
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

    /// `getWordTagger().tag(stripped)` + `asAnalyzedTokenListForTaggedWords`:
    /// the surface keeps the original word, lemma/tag come from the lookup.
    fn base_readings(&self, word: &str, lookup: &str) -> Vec<AnalyzedToken> {
        self.word_lookup(lookup)
            .into_iter()
            .map(|(lemma, tag)| AnalyzedToken::new(word, Some(lemma), Some(tag)))
            .collect()
    }

    fn is_stop_word_readings(readings: &[AnalyzedToken]) -> bool {
        readings.iter().any(|r| {
            r.pos_tag
                .as_deref()
                .is_some_and(ArabicTagManager::is_stop_word)
        })
    }

    /// `ArabicTagger.getSuffixIndexList`.
    fn suffix_index_list(word: &str) -> Vec<usize> {
        let len = word.chars().count();
        let mut indexes = vec![len];
        for suffix in ["ك", "ها", "هما", "كما", "هم", "هن", "كم", "كن", "نا"] {
            if word.ends_with(suffix) {
                let pos = if suffix == "ك" {
                    len - 1
                } else if suffix == "هما" || suffix == "كما" {
                    len - 3
                } else {
                    len - 2
                };
                indexes.push(pos);
                break;
            }
        }
        indexes
    }

    /// `ArabicTagger.getPrefixIndexList` (order: 0, four-, three-, two-,
    /// one-letter prefixes).
    fn prefix_index_list(word: &str) -> Vec<usize> {
        let mut indexes = vec![0];
        if ["وكال", "وبال", "فكال", "فبال"]
            .iter()
            .any(|p| word.starts_with(p))
        {
            indexes.push(4);
        }
        if ["ولل", "فلل", "فال", "وال", "بال", "كال"]
            .iter()
            .any(|p| word.starts_with(p))
        {
            indexes.push(3);
        }
        if [
            "لل", "وك", "ول", "وب", "فك", "فل", "فب", "ال", "فسأ", "فسن", "فسي", "فست", "وسأ",
            "وسن", "وسي", "وست",
        ]
        .iter()
        .any(|p| word.starts_with(p))
        {
            indexes.push(2);
        }
        if ["ك", "ل", "ب", "و", "ف", "سأ", "سن", "سي", "ست"]
            .iter()
            .any(|p| word.starts_with(p))
        {
            indexes.push(1);
        }
        indexes
    }

    /// `ArabicTagger.getStem`.
    fn get_stem(word: &str, pos_start: usize, pos_end: usize) -> Vec<String> {
        let len = word.chars().count();
        let mut stem = substring(word, pos_start, len);
        if pos_end != len {
            for suffix in ["كما", "هما", "هن", "هم", "كن", "كم", "نا", "ها", "ك", "ي"]
            {
                if stem.ends_with(suffix) {
                    let kept = stem[..stem.len() - suffix.len()].to_string();
                    stem = format!("{kept}ه");
                    break;
                }
            }
        }
        let prefix = substring(word, 0, pos_start);
        let mut stems = Vec::new();
        if prefix.ends_with("لل") {
            stems.push(format!("ل{stem}"));
        }
        stems.push(stem);
        stems
    }

    /// `ArabicTagger.getTags`.
    fn get_tags(word: &str, pos_start: usize, pos_end: usize) -> Vec<String> {
        let mut tags = Vec::new();
        let len = word.chars().count();
        let mut prefix = substring(word, 0, pos_start);
        let suffix = substring(word, pos_end, len);
        if prefix.starts_with('و') || prefix.starts_with('ف') {
            tags.push("CONJ;W".to_string());
            prefix = prefix.chars().skip(1).collect();
        }
        if prefix.starts_with('ك') {
            tags.push("JAR;K".to_string());
        } else if prefix.starts_with('ل') {
            tags.push("JAR;L".to_string());
        } else if prefix.starts_with('ب') {
            tags.push("JAR;B".to_string());
        } else if prefix.starts_with('س') {
            tags.push("ISTIQBAL;S".to_string());
        }
        if prefix.ends_with("ال") || prefix.ends_with("لل") {
            tags.push("PRONOUN;D".to_string());
        }
        if [
            "ني", "نا", "ك", "كما", "كم", "كن", "ه", "ها", "هما", "هم", "هن",
        ]
        .contains(&suffix.as_str())
        {
            tags.push("PRONOUN;H".to_string());
        }
        tags
    }

    /// `ArabicTagger.additionalTags`: the affix-derived readings.
    fn additional_tags(&self, word: &str, stripped: &str) -> Vec<AnalyzedToken> {
        let mut out = Vec::new();
        let len = stripped.chars().count();
        for i in Self::prefix_index_list(stripped) {
            for j in Self::suffix_index_list(stripped) {
                if i == 0 && j == len {
                    continue;
                }
                let stems = Self::get_stem(stripped, i, j);
                let tags = Self::get_tags(stripped, i, j);
                for stem in stems {
                    for (lemma, tag) in self.dict.lookup(&stem) {
                        if let Some(pos_tag) = ArabicTagManager::modify_pos_tag(&tag, &tags) {
                            out.push(AnalyzedToken::new(word, Some(lemma), Some(pos_tag)));
                        }
                    }
                }
            }
        }
        out
    }

    /// `ArabicTagger.tag(List<String>)` for one word.
    pub fn tag_word(&self, word: &str) -> Vec<AnalyzedToken> {
        let stripped = remove_tashkeel(word);
        let base = self.base_readings(word, &stripped);
        let mut l = base.clone();
        if !Self::is_stop_word_readings(&base) {
            l.extend(self.additional_tags(word, &stripped));
        }
        if l.is_empty() {
            l.push(AnalyzedToken::new(word, None, None));
        }
        l
    }

    /// Tokenizer helper: an Arabic word is "tagged" when any reading has a POS
    /// tag.
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

    /// `ArabicTagger.getProclitic`.
    pub fn get_proclitic(&self, token: &AnalyzedToken) -> String {
        let postag = token.pos_tag.as_deref().unwrap_or("");
        let word = &token.token;
        if postag.is_empty() {
            return String::new();
        }
        if ArabicTagManager::is_verb(postag) {
            let mut prefix_length = 0;
            if ArabicTagManager::get_flag(postag, "CONJ") == 'W' {
                prefix_length += 1;
            }
            if ArabicTagManager::get_flag(postag, "ISTIQBAL") == 'S' {
                prefix_length += 1;
            }
            substring(word, 0, prefix_length)
        } else if ArabicTagManager::is_noun(postag) {
            let conj = ArabicTagManager::get_flag(postag, "CONJ");
            let jar = ArabicTagManager::get_flag(postag, "JAR");
            let mut prefix_length = 0;
            if conj != '-' {
                prefix_length += 1;
            }
            if jar != '-' {
                prefix_length += 1;
            }
            if ArabicTagManager::is_definite(postag) {
                if jar == 'L' {
                    prefix_length += 1;
                } else {
                    prefix_length += 2;
                }
            }
            substring(word, 0, prefix_length)
        } else {
            String::new()
        }
    }

    /// `ArabicTagger.getEnclitic`.
    pub fn get_enclitic(&self, token: &AnalyzedToken) -> String {
        let postag = token.pos_tag.as_deref().unwrap_or("");
        let word = &token.token;
        if postag.is_empty() {
            return String::new();
        }
        let flag = ArabicTagManager::get_flag(postag, "PRONOUN");
        if flag == '-' {
            return ArabicTagManager::get_pronoun_suffix(postag);
        }
        for suffix in [
            "ه", "ها", "هما", "هم", "هن", "ك", "كما", "كم", "كن", "ني", "نا",
        ] {
            if word.ends_with(suffix) {
                return suffix.to_string();
            }
        }
        if (word == "عني" || word == "مني") && word.ends_with("ني") {
            return "ني".to_string();
        }
        if (word == "عنا" || word == "منا") && word.ends_with("نا") {
            return "نا".to_string();
        }
        String::new()
    }

    /// `ArabicTagger.getJarProclitic`.
    pub fn get_jar_proclitic(&self, token: &AnalyzedToken) -> String {
        let postag = token.pos_tag.as_deref().unwrap_or("");
        let word = &token.token;
        if postag.is_empty() {
            return String::new();
        }
        if ArabicTagManager::is_noun(postag) {
            let mut prefix_length = 0;
            if ArabicTagManager::get_flag(postag, "CONJ") != '-' {
                prefix_length += 1;
            }
            if ArabicTagManager::get_flag(postag, "JAR") != '-' {
                prefix_length += 1;
            }
            if prefix_length > 0 {
                return substring(word, prefix_length - 1, prefix_length);
            }
        }
        String::new()
    }

    /// `ArabicTagger.getLemmas(tokenReadings, type)`.
    pub fn get_lemmas(readings: &AnalyzedTokenReadings, kind: &str) -> Vec<String> {
        let mut lemmas = Vec::new();
        for tok in &readings.readings {
            let postag = tok.pos_tag.as_deref().unwrap_or("");
            let matches = (ArabicTagManager::is_verb(postag) && kind == "verb")
                || (ArabicTagManager::is_adj(postag) && kind == "adj")
                || (ArabicTagManager::is_masdar(postag) && kind == "masdar");
            if matches {
                if let Some(lemma) = &tok.stem {
                    if !lemmas.contains(lemma) {
                        lemmas.push(lemma.clone());
                    }
                }
            }
        }
        lemmas
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_manager_flag_ops() {
        // `كتاب`'s noun reading: definite article via PRONOUN='L'.
        let noun = "NA-;-3--;---";
        assert!(ArabicTagManager::is_noun(noun));
        assert!(!ArabicTagManager::is_verb(noun));
        assert_eq!(ArabicTagManager::get_flag(noun, "NUMBER"), '3');
        let def = ArabicTagManager::add_tag_string(noun, "PRONOUN;D").unwrap();
        assert_eq!(def, "NA-;-3--;--L");
        assert!(ArabicTagManager::is_definite(&def));
        assert_eq!(ArabicTagManager::get_flag(&def, "PRONOUN"), 'L');
    }

    #[test]
    fn tag_manager_majrour_jar() {
        // A majrour noun (CASE 'I') accepts the B jar prefix.
        let noun = "NJ-;M1I-;---";
        let majrour = ArabicTagManager::add_tag_string(noun, "JAR;B").unwrap();
        assert_eq!(majrour, "NJ-;M1I-;-B-");
        // A marfou3 noun (CASE 'U') rejects it.
        assert!(ArabicTagManager::add_tag_string("NJ-;M1U-;---", "JAR;B").is_none());
    }

    /// Tag the loaded dictionary exactly like the Java probe
    /// (`scripts/oracle/ar/probe-tagger.sh`): the readings below are the
    /// pinned Java output for the same words. Skips when `data/` is absent.
    #[test]
    fn tagger_matches_java_probe() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !dir.is_dir() {
            return;
        }
        let t = ArabicTagger::load(&dir).unwrap();
        let readings = |w: &str| -> Vec<(String, String)> {
            t.tag_word(w)
                .into_iter()
                .map(|r| (r.stem.unwrap_or_default(), r.pos_tag.unwrap_or_default()))
                .collect()
        };
        // `كتاب`: adjective (`NA`) + noun (`NJ`) + masdar (`NM`) readings.
        assert_eq!(
            readings("كتاب"),
            vec![
                ("كتاب".to_string(), "NA-;-3--;---".to_string()),
                ("كتاب".to_string(), "NA-;-3A-;---".to_string()),
                ("كتاب".to_string(), "NA-;-3I-;---".to_string()),
                ("كتاب".to_string(), "NA-;-3U-;---".to_string()),
                ("كتاب".to_string(), "NJ-;M1--;---".to_string()),
                ("كتاب".to_string(), "NJ-;M1A-;---".to_string()),
                ("كتاب".to_string(), "NJ-;M1I-;---".to_string()),
                ("كتاب".to_string(), "NJ-;M1U-;---".to_string()),
                ("كتاب".to_string(), "NM-;M1--;---".to_string()),
                ("كتاب".to_string(), "NM-;M1A-;---".to_string()),
                ("كتاب".to_string(), "NM-;M1I-;---".to_string()),
                ("كتاب".to_string(), "NM-;M1U-;---".to_string()),
            ]
        );
        // Definite prefix `ال` sets PRONOUN='L' on all 12 readings.
        let def = readings("الكتاب");
        assert_eq!(def.len(), 12);
        assert!(def.iter().all(|(l, t)| l == "كتاب" && t.ends_with("--L")));
        // `بالمدرسة`: the `بال` jar prefix, only majrour readings survive.
        assert_eq!(
            readings("بالمدرسة"),
            vec![
                ("مدرسة".to_string(), "NJ-;F1--;-BL".to_string()),
                ("مدرسة".to_string(), "NJ-;F1I-;-BL".to_string()),
                ("مدرس".to_string(), "NA-;F1--;-BL".to_string()),
                ("مدرس".to_string(), "NA-;F1I-;-BL".to_string()),
            ]
        );
        // `وكتابهم`: conjunction W + attached pronoun H.
        let w = readings("وكتابهم");
        assert_eq!(w.len(), 12);
        assert!(w.iter().all(|(l, t)| l == "كتاب" && t.ends_with("W-H")));
        // `سيكتبون`: future prefix `س` on the verbs.
        let fut = readings("سيكتبون");
        assert_eq!(fut.len(), 6);
        assert!(fut.iter().all(|(_, t)| t.ends_with("-S-")));
        // A word absent from the dictionary and the manual lists falls back to
        // `AnalyzedToken(word, null, null)`.
        let untagged = t.tag_word("هو");
        assert_eq!(untagged.len(), 1);
        assert!(untagged[0].stem.is_none() && untagged[0].pos_tag.is_none());
    }

    #[test]
    fn remove_tashkeel_strips_marks() {
        assert_eq!(remove_tashkeel("الْكِتَاب"), "الكتاب");
        assert_eq!(remove_tashkeel("كتــاب"), "كتاب");
    }
}
