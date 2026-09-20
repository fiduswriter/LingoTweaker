//! Spanish synthesizer adapter: exposes `lt_tagger::SpanishSynthesizer`
//! through the pattern engine's `Synthesizer` trait (`<match postag="...">`
//! rendering).

use lt_core::AnalyzedToken;
use lt_pattern::Synthesizer;

/// Adapter exposing the Spanish synthesizer through the pattern engine's
/// [`Synthesizer`] trait, plus the tagger for the `checksSpelling` check.
pub struct SpanishSynthesizerAdapter {
    pub synth: std::sync::Arc<lt_tagger::SpanishSynthesizer>,
    pub tagger: std::sync::Arc<lt_tagger::SpanishTagger>,
}

impl SpanishSynthesizerAdapter {
    pub fn inner(&self) -> &lt_tagger::SpanishSynthesizer {
        &self.synth
    }
}

impl Synthesizer for SpanishSynthesizerAdapter {
    fn synthesize(
        &self,
        token: &AnalyzedToken,
        pos_tag: &str,
        pos_tag_regexp: bool,
    ) -> Vec<String> {
        self.synth.synthesize(token, pos_tag, pos_tag_regexp)
    }

    fn target_pos_tag(&self, pos_tags: &[String], fallback: &str) -> String {
        self.synth.target_pos_tag(pos_tags, fallback)
    }

    fn sort_target_pos_tags(&self, pos_tags: &mut Vec<String>) {
        self.synth.sort_pos_tags(pos_tags);
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
