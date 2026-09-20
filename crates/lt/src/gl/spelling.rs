//! Galician spelling rule: `org.languagetool.rules.spelling.hunspell.HunspellRule`
//! over the vendored VOLGa-based `gl/hunspell/gl_ES.{aff,dic}`.
//!
//! `HunspellRule` is the plain `SpellingCheckRule` hunspell backend: the
//! `gl/hunspell/{ignore,spelling,spelling_custom}.txt` and
//! `core/spelling_global.txt` word lists, `gl/hunspell/prohibit*.txt`, the
//! `_english_ignore_` tag and the `desc_spelling`/`spelling`/
//! `desc_spelling_short` messages. Suggestions come from the shared bounded
//! search over the dictionary words (`HunspellSpellingRule`); upstream's
//! native `hunspell.suggest` ranking is not reproduced (internal notes).

use std::path::Path;

use lt_core::{AnalyzedTokenReadings, Match, Result};

use crate::hunspell_spelling::{HunspellSpellingConfig, HunspellSpellingRule as InnerRule};

pub const RULE_ID: &str = "HUNSPELL_RULE";

pub struct GalicianSpellingRule(InnerRule);

impl GalicianSpellingRule {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self(InnerRule::load(
            data_dir,
            HunspellSpellingConfig {
                rule_id: RULE_ID,
                description: "Posíbel erro ortográfico",
                message: "Atopouse un posíbel erro ortográfico",
                short_message: "Erro ortográfico",
                category_id: "TYPOS",
                category_name: "Posíbeis erros tipográficos",
                lang_dir: "gl",
                aff: "gl_ES.aff",
                dic: "gl_ES.dic",
                suggestion_file: None,
                morfologik_dict: None,
                max_suggestions: 5,
            },
        )?))
    }

    pub fn rule_id(&self) -> &str {
        self.0.rule_id()
    }

    /// Dictionary membership for the context rules.
    pub fn is_known(&self, word: &str) -> bool {
        self.0.is_known(word)
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        self.0.check_sentence(tokens, sentence_offset)
    }
}
