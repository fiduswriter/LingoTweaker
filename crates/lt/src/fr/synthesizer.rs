//! French synthesizer adapter: exposes `lt_tagger::FrenchSynthesizer`
//! through the pattern engine's `Synthesizer` trait (`<match postag="...">`
//! rendering).

use lt_core::AnalyzedToken;
use lt_pattern::Synthesizer;

/// Adapter exposing the French synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct FrenchSynthesizerAdapter {
    pub synth: std::sync::Arc<lt_tagger::FrenchSynthesizer>,
    pub tagger: std::sync::Arc<lt_tagger::FrenchTagger>,
}

impl FrenchSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::FrenchSynthesizer {
        &self.synth
    }
}

impl Synthesizer for FrenchSynthesizerAdapter {
    fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        self.synth.synthesize(token, pos_tag, pos_tag_regexp)
    }

    fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        // `BaseSynthesizer` has no comparator for French: the last matching
        // tag wins.
        pos_tags
            .last()
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }

    fn is_known_word(&self, word: &str) -> bool {
        // `MatchState.toFinalString`: `lemma == null && hasNoTag()`
        !self
            .tagger
            .tag_word(word)
            .into_iter()
            .next()
            .is_some_and(|r| r.stem.is_none() && r.pos_tag.is_none())
    }
}
