//! German synthesizer adapter: exposes `lt_tagger`'s `GermanSynthesizer`
//! through the pattern engine's `Synthesizer` trait (`<match postag="...">`
//! rendering).

use lt_core::AnalyzedToken;
use lt_pattern::Synthesizer;

use crate::de::pipeline::GermanTaggerKind;

/// Adapter exposing the German synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct GermanSynthesizerAdapter {
    pub synth: std::sync::Arc<lt_tagger::GermanSynthesizer>,
    pub tagger: GermanTaggerKind,
}

impl GermanSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::GermanSynthesizer {
        &self.synth
    }
}

impl Synthesizer for GermanSynthesizerAdapter {
    fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        self.synth.synthesize(token, pos_tag, pos_tag_regexp)
    }

    fn is_known_word(&self, word: &str) -> bool {
        // `MatchState.toFinalString`: `lemma == null && hasNoTag()`
        !self
            .tagger
            .tag(std::slice::from_ref(&word.to_string()), true)
            .into_iter()
            .next()
            .and_then(|r| r.readings.into_iter().next())
            .is_some_and(|r| r.stem.is_none() && r.pos_tag.is_none())
    }
}
