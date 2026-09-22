//! Arabic synthesizer: port of `org.languagetool.synthesis.ar.ArabicSynthesizer`
//! (+ the `BaseSynthesizer` parts it uses). Inflected forms for a lemma and
//! POS tag, with the Arabic `correctTag`/`correctStem` adjustments and the
//! proclitic/enclitic helpers used by the XML synthesis filters.

use std::path::Path;
use std::sync::Arc;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, CoreError, Result};

use crate::arabic::{ArabicTagManager, ArabicTagger};
use crate::manual_synth::ManualSynthesizer;
use crate::soros::Soros;
use crate::{DictionaryInfo, SynthDictionary};

pub struct ArabicSynthesizer {
    dict: SynthDictionary,
    possible_tags: Vec<String>,
    manual: Option<ManualSynthesizer>,
    removed: Option<ManualSynthesizer>,
    do_not_synthesize: Option<ManualSynthesizer>,
    roman_numberer: Option<Soros>,
    tagger: Arc<ArabicTagger>,
}

impl ArabicSynthesizer {
    /// Load `ar/dictionaries/arabic_synth.{dict,info}` + `ar/words` tag lists.
    pub fn from_data(data_dir: &Path, tagger: Arc<ArabicTagger>) -> Result<Self> {
        let dict_dir = data_dir.join("ar/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("arabic_synth.info"))?;
        let dict = SynthDictionary::load(&dict_dir.join("arabic_synth.dict"), &info)?;
        let words_dir = data_dir.join("ar/words");
        let tag_text = lt_data::fs::read_to_string(words_dir.join("arabic_tags.txt"))
            .map_err(|e| CoreError::Data(format!("cannot read arabic_tags.txt: {e}")))?;
        let mut possible_tags: Vec<String> = tag_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let manual = ManualSynthesizer::load(&words_dir.join("added.txt"));
        let removed = ManualSynthesizer::load(&words_dir.join("removed.txt"));
        let do_not_synthesize = ManualSynthesizer::load(&words_dir.join("do-not-synthesize.txt"));
        if let Some(manual) = &manual {
            for tag in &manual.possible_tags {
                if !possible_tags.contains(tag) {
                    possible_tags.push(tag.clone());
                }
            }
        }
        let roman_numberer = lt_data::fs::read_to_string(data_dir.join("core/Roman.sor"))
            .ok()
            .map(|source| Soros::new(&source, "Roman"));
        Ok(Self {
            dict,
            possible_tags,
            manual,
            removed,
            do_not_synthesize,
            roman_numberer,
            tagger,
        })
    }

    /// `BaseSynthesizer.lookup`: dictionary + manual additions - removals.
    fn lookup(&self, lemma: &str, pos_tag: &str) -> Vec<String> {
        let key = format!("{lemma}|{pos_tag}");
        let mut results = self.dict.lookup(&key);
        if let Some(manual) = &self.manual {
            if let Some(forms) = manual.lookup(lemma, pos_tag) {
                results.extend(forms);
            }
        }
        if let Some(removed) = &self.removed {
            if let Some(forms) = removed.lookup(lemma, pos_tag) {
                results.retain(|r| !forms.contains(r));
            }
        }
        if let Some(removed) = &self.do_not_synthesize {
            if let Some(forms) = removed.lookup(lemma, pos_tag) {
                results.retain(|r| !forms.contains(r));
            }
        }
        results
    }

    /// `BaseSynthesizer.getSpelledNumber`: a null number speller returns the
    /// numeral unchanged.
    pub fn get_spelled_number(&self, arabic_numeral: &str) -> String {
        arabic_numeral.to_string()
    }

    /// `BaseSynthesizer.getRomanNumber` (`core/Roman.sor`).
    pub fn get_roman_number(&self, arabic_numeral: &str) -> String {
        match &self.roman_numberer {
            Some(numberer) => numberer.run(arabic_numeral),
            None => arabic_numeral.to_string(),
        }
    }

    /// `ArabicSynthesizer.correctTag`.
    pub fn correct_tag(&self, postag: &str) -> String {
        let p = ArabicTagManager::set_conjunction(postag, "-");
        let p = ArabicTagManager::set_definite(&p, "-");
        ArabicTagManager::unify_pronoun_tag(&p)
    }

    /// `ArabicSynthesizer.correctStem`.
    pub fn correct_stem(&self, stem: &str, postag: &str) -> String {
        let mut stem = stem.to_string();
        if postag.is_empty() {
            return stem;
        }
        if ArabicTagManager::is_attached(postag) {
            if let Some(stripped) = stem.strip_suffix('ه') {
                stem = stripped.to_string();
            }
        }
        if ArabicTagManager::is_definite(postag) {
            let prefix = ArabicTagManager::get_definite_prefix(postag);
            stem = format!("{prefix}{stem}");
        }
        if ArabicTagManager::has_jar(postag) {
            let prefix = ArabicTagManager::get_jar_prefix(postag);
            stem = format!("{prefix}{stem}");
        }
        if ArabicTagManager::has_conjunction(postag) {
            let prefix = ArabicTagManager::get_conjunction_prefix(postag);
            stem = format!("{prefix}{stem}");
        }
        stem
    }

    /// `BaseSynthesizer.getPosTagCorrection` (= `correctTag`).
    pub fn pos_tag_correction(&self, pos_tag: &str) -> String {
        self.correct_tag(pos_tag)
    }

    /// `ArabicSynthesizer.synthesize(AnalyzedToken, String)`.
    pub fn synthesize(&self, token: &AnalyzedToken, pos_tag: &str) -> Vec<String> {
        let lemma = token.stem.clone().unwrap_or_else(|| token.token.clone());
        self.dict
            .lookup(&format!("{lemma}|{pos_tag}"))
            .into_iter()
            .map(|stem| self.correct_stem(&stem, pos_tag))
            .collect()
    }

    /// `ArabicSynthesizer.synthesize(AnalyzedToken, String, boolean)`.
    pub fn synthesize_regexp(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        if !pos_tag.is_empty() && pos_tag_regexp {
            let corrected = self.correct_tag(pos_tag);
            let Ok(re) = fancy_regex::Regex::new(&format!("^(?:{corrected})$")) else {
                return Vec::new();
            };
            let Some(lemma) = token.stem.as_deref() else {
                return Vec::new();
            };
            let mut results = Vec::new();
            for tag in &self.possible_tags {
                if re.is_match(tag).unwrap_or(false) {
                    for stem in self.lookup(lemma, tag) {
                        results.push(self.correct_stem(&stem, pos_tag));
                    }
                }
            }
            return results;
        }
        self.synthesize(token, pos_tag)
    }

    pub fn synthesize_plain(&self, token: &AnalyzedToken, pos_tag: &str) -> Vec<String> {
        self.synthesize(token, pos_tag)
    }

    /// `Synthesizer.getTargetPosTag` (base class: the last tag).
    pub fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        pos_tags
            .last()
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }

    /// `ArabicSynthesizer.setEncliticMultiple`.
    pub fn set_enclitic_multiple(&self, token: &AnalyzedToken, suffix: &str) -> Vec<String> {
        let postag = token.pos_tag.as_deref().unwrap_or("");
        let word = &token.token;
        let default = vec![format!("({word})")];
        if postag.is_empty() {
            return default;
        }
        let flag = if suffix.is_empty() { '-' } else { 'H' };
        let procletic = self.tagger.get_proclitic(token);
        let newpos_tag = ArabicTagManager::set_flag(postag, "PRONOUN", flag);
        let newpos_tag = ArabicTagManager::set_procletic_flags(&newpos_tag);
        let lemma = token.stem.clone().unwrap_or_default();
        let new_token =
            AnalyzedToken::new(lemma.clone(), Some(lemma.clone()), Some(newpos_tag.clone()));
        let new_word_list = self.synthesize(&new_token, &newpos_tag);
        let mut wordlist: Vec<String> = Vec::new();
        if !new_word_list.is_empty() {
            for mut stem in new_word_list {
                let new_word = if ArabicTagManager::has_pronoun(&newpos_tag) && flag == 'H' {
                    if stem.ends_with('ي') {
                        // if the stem ends with the 1st-person pronoun Yeh:
                        // ignore the suffix if it is Yeh, else ignore the stem
                        if suffix == "ي" {
                            format!("{procletic}{stem}")
                        } else {
                            String::new()
                        }
                    } else if let Some(stripped) = stem.strip_suffix('ه') {
                        stem = stripped.to_string();
                        format!("{procletic}{stem}{suffix}")
                    } else {
                        format!("{procletic}{stem}{suffix}")
                    }
                } else {
                    format!("{procletic}{stem}")
                };
                if !new_word.is_empty() {
                    wordlist.push(new_word);
                }
            }
        } else {
            wordlist.push(format!("({word})"));
        }
        if wordlist.is_empty() {
            return default;
        }
        wordlist
    }

    /// `ArabicSynthesizer.setJarProcletic`.
    pub fn set_jar_procletic(&self, token: &AnalyzedToken, prefix: &str) -> String {
        let postag = token.pos_tag.as_deref().unwrap_or("");
        if postag.is_empty() {
            return token.token.clone();
        }
        let mut prefix = prefix.to_string();
        if ArabicTagManager::is_definite(postag) {
            if prefix == "ل" {
                prefix.push('ل');
            } else {
                prefix.push_str("ال");
            }
        }
        self.set_procletic(token, &prefix)
    }

    /// `ArabicSynthesizer.setProcletic`.
    pub fn set_procletic(&self, token: &AnalyzedToken, prefix: &str) -> String {
        let postag = token.pos_tag.as_deref().unwrap_or("");
        if postag.is_empty() {
            return token.token.clone();
        }
        let enclitic = self.tagger.get_enclitic(token);
        let newpos_tag = ArabicTagManager::set_procletic_flags(postag);
        let lemma = token.stem.clone().unwrap_or_default();
        let new_token = AnalyzedToken::new(lemma.clone(), Some(lemma), Some(newpos_tag.clone()));
        let new_word_list = self.synthesize(&new_token, &newpos_tag);
        let stem = if !new_word_list.is_empty() {
            let mut stem = new_word_list[0].clone();
            if ArabicTagManager::has_pronoun(&newpos_tag) {
                if let Some(stripped) = stem.strip_suffix('ه') {
                    stem = stripped.to_string();
                }
            }
            stem
        } else {
            format!("({})", token.token)
        };
        format!("{prefix}{stem}{enclitic}")
    }

    /// `ArabicSynthesizer.inflectLemmaLike`.
    pub fn inflect_lemma_like(
        &self,
        target_lemma: &str,
        source_token: &AnalyzedToken,
    ) -> Vec<String> {
        let readings = AnalyzedTokenReadings::new(self.tagger.tag_word(target_lemma));
        if !readings.has_lemma(target_lemma) {
            return vec![format!("[{target_lemma}]")];
        }
        let source_postag = source_token.pos_tag.as_deref().unwrap_or("");
        let prefix = self.tagger.get_proclitic(source_token);
        let suffix = self.tagger.get_enclitic(source_token);
        let mut wordlist: Vec<String> = Vec::new();
        for current in &readings.readings {
            if current.stem.as_deref() != Some(target_lemma) {
                continue;
            }
            let postag_lemma = current.pos_tag.as_deref().unwrap_or("");
            let merged = ArabicTagManager::merge_pos_tag(source_postag, postag_lemma);
            let word = format!("{prefix}{target_lemma}");
            let token = AnalyzedToken::new(word, Some(target_lemma.to_string()), Some(merged));
            for form in self.set_enclitic_multiple(&token, &suffix) {
                if !wordlist.contains(&form) {
                    wordlist.push(form);
                }
            }
        }
        wordlist
    }
}

/// `ArabicSynthesizer.inflectMafoulMutlq`.
pub fn inflect_mafoul_mutlq(word: &str) -> String {
    const TEH_MARBUTA: char = '\u{0629}';
    const FATHATAN: char = '\u{064B}';
    const ALEF: char = '\u{0627}';
    if word.ends_with(TEH_MARBUTA) {
        format!("{word}{FATHATAN}")
    } else {
        format!("{word}{FATHATAN}{ALEF}")
    }
}

/// `ArabicSynthesizer.inflectAdjectiveTanwinNasb`.
pub fn inflect_adjective_tanwin_nasb(word: &str, feminin: bool) -> String {
    const TEH_MARBUTA: char = '\u{0629}';
    const FATHATAN: char = '\u{064B}';
    const ALEF: char = '\u{0627}';
    if feminin {
        if word.ends_with(TEH_MARBUTA) {
            format!("{word}{FATHATAN}")
        } else {
            format!("{word}{TEH_MARBUTA}{FATHATAN}")
        }
    } else if word.ends_with(TEH_MARBUTA) {
        // if masculine, remove the teh marbuta
        word.replace(TEH_MARBUTA, "")
    } else {
        format!("{word}{FATHATAN}{ALEF}")
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// The pinned Java `SynthProbe` output for the same (word, lemma, tag)
    /// triples. Skips when `data/` is absent.
    #[test]
    fn synthesizer_matches_java_probe() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !dir.is_dir() {
            return;
        }
        let tagger = std::sync::Arc::new(ArabicTagger::load(&dir).unwrap());
        let synth = ArabicSynthesizer::from_data(&dir, tagger).unwrap();
        let forms = |lemma: &str, tag: &str| -> Vec<String> {
            synth.synthesize(
                &AnalyzedToken::new("", Some(lemma.to_string()), Some(tag.to_string())),
                tag,
            )
        };
        assert_eq!(
            forms("كتاب", "NA-;-3--;---"),
            vec!["كتاب", "كتابون", "كتابي", "كتابين"]
        );
        assert_eq!(forms("كتاب", "NJ-;M1U-;---"), vec!["كتاب"]);
        // A definite tag has no synth entry (Java returns empty).
        assert!(forms("كتاب", "NA-;-3--;--L").is_empty());
        assert_eq!(
            forms("مدرسة", "NJ-;F1--;---"),
            vec!["مدرسة", "مدرستة", "مدرسي"]
        );
        assert_eq!(forms("عَامَلَ", "V41;M1Y-i--;---"), vec!["عامل", "عاملن"]);
    }

    #[test]
    fn static_inflection_helpers() {
        // `inflectMafoulMutlq`: teh marbuta gets a fathatan, else fathatan+alef.
        assert_eq!(inflect_mafoul_mutlq("عمل"), "عمل\u{064B}\u{0627}");
        assert_eq!(inflect_mafoul_mutlq("مدرسة"), "مدرسة\u{064B}");
        // `inflectAdjectiveTanwinNasb`: feminine adds teh marbuta+fathatan,
        // masculine removes a teh marbuta (else adds fathatan+alef).
        assert_eq!(
            inflect_adjective_tanwin_nasb("طويل", false),
            "طويل\u{064B}\u{0627}"
        );
        assert_eq!(
            inflect_adjective_tanwin_nasb("طويل", true),
            "طويل\u{0629}\u{064B}"
        );
        assert_eq!(inflect_adjective_tanwin_nasb("جميلة", false), "جميل");
        assert_eq!(
            inflect_adjective_tanwin_nasb("جميلة", true),
            "جميلة\u{064B}"
        );
    }
}
