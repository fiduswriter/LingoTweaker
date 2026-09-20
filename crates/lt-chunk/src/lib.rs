//! English chunking (P1.7/D-002 option (a)): a clean-room port of the
//! OpenNLP 1.9.5 components the legacy engine uses — `TokenizerME`,
//! `POSTaggerME` (1.5-era classic feature set) and `ChunkerME` — reading
//! LT's vendored `.bin` models directly, plus the `EnglishChunker` /
//! `EnglishChunkFilter` mapping into `AnalyzedTokenReadings.chunk_tags`.
//!
//! Deviations from the legacy engine: beam-search heap tie-breaking differs from
//! Java's `PriorityQueue` for exactly-equal sequence scores; feature caches
//! are not ported (results are identical, performance only).

mod beam;
mod chunk;
pub mod german;
pub mod model;
pub mod openregex;
mod pos;

use std::path::Path;

use lt_core::{AnalyzedTokenReadings, CoreError, Result};

use chunk::{tokenize_pos, TokenTag};
use model::{GenericModel, ModelFile};

/// `org.languagetool.chunking.EnglishChunker` + `EnglishChunkFilter`.
pub struct EnglishChunker {
    token_model: GenericModel,
    pos_model: GenericModel,
    chunk_model: GenericModel,
    tagdict: pos::PosDictionary,
}

struct ChunkTaggedToken {
    token: String,
    chunk_tags: Vec<String>,
    readings: Option<usize>,
    // readings of the mapped LT token, needed by the plural filter
    has_nns: bool,
}

impl EnglishChunker {
    /// Load the three models from a directory holding `en-token.bin`,
    /// `en-pos-maxent.bin`, `en-chunker.bin`.
    pub fn load(models_dir: &Path) -> Result<Self> {
        let token = ModelFile::load(&models_dir.join("en-token.bin"))?;
        let pos = ModelFile::load(&models_dir.join("en-pos-maxent.bin"))?;
        let chk = ModelFile::load(&models_dir.join("en-chunker.bin"))?;
        let token_model =
            GenericModel::read(token.entries.get("token.model").ok_or_else(|| {
                CoreError::Data("en-token.bin: missing token.model entry".into())
            })?)?;
        let pos_model = GenericModel::read(pos.entries.get("pos.model").ok_or_else(|| {
            CoreError::Data("en-pos-maxent.bin: missing pos.model entry".into())
        })?)?;
        let chunk_model =
            GenericModel::read(chk.entries.get("chunker.model").ok_or_else(|| {
                CoreError::Data("en-chunker.bin: missing chunker.model entry".into())
            })?)?;
        let tagdict_bytes = pos.entries.get("tags.tagdict").ok_or_else(|| {
            CoreError::Data("en-pos-maxent.bin: missing tags.tagdict entry".into())
        })?;
        let tagdict = pos::PosDictionary::read(tagdict_bytes)?;
        Ok(Self {
            token_model,
            pos_model,
            chunk_model,
            tagdict,
        })
    }

    /// `EnglishChunker.addChunkTags`: runs OpenNLP tokenize → POS tag →
    /// chunk over the sentence and assigns chunk tags to the LT tokens
    /// (replacing any previous chunk tags, like `setChunkTags`).
    pub fn add_chunk_tags(&self, tokens: &mut [AnalyzedTokenReadings]) {
        let mut sentence = String::with_capacity(128);
        for t in tokens.iter() {
            sentence.push_str(t.surface());
        }
        let opennlp_tokens = self.tokenize(&sentence);
        let tags = self.pos_tag(&opennlp_tokens);
        let chunks = self.chunk_tokens(&opennlp_tokens, &tags);

        // map OpenNLP tokens to LT token indices by exact whitespace-free
        // cumulative positions (EnglishChunker.getTokensWithTokenReadings)
        let mut tagged: Vec<ChunkTaggedToken> = Vec::with_capacity(chunks.len());
        let mut oi = 0usize;
        let mut pos = 0usize;
        for chunk_tag in &chunks {
            let Some(o_tok) = opennlp_tokens.get(oi) else {
                break;
            };
            let start = pos;
            let end = start + o_tok.chars().count();
            let mut readings: Option<usize> = None;
            let mut has_nns = false;
            // walk LT tokens (skipping whitespace/empty) to the same span
            let mut lt_pos = 0usize;
            for (ti, tr) in tokens.iter().enumerate() {
                let surface = tr.surface();
                if surface.trim().is_empty() {
                    continue;
                }
                let lt_start = lt_pos;
                let lt_end = lt_pos + surface.chars().count();
                if lt_start == start && lt_end == end {
                    readings = Some(ti);
                    has_nns = tr
                        .readings
                        .iter()
                        .any(|r| r.pos_tag.as_deref() == Some("NNS"));
                    break;
                }
                lt_pos = lt_end;
            }
            tagged.push(ChunkTaggedToken {
                token: o_tok.clone(),
                chunk_tags: vec![chunk_tag.clone()],
                readings,
                has_nns,
            });
            pos = end;
            oi += 1;
        }

        // EnglishChunkFilter: singular/plural refinement of NP chunks
        let filtered = self.chunk_filter(tagged);

        // assignChunksToReadings: setChunkTags replaces the chunk tags
        for t in &filtered {
            if let Some(ti) = t.readings {
                tokens[ti].chunk_tags = t.chunk_tags.clone();
            }
        }
    }

    /// Raw OpenNLP pipeline output for one sentence: (token, POS, chunk)
    /// triples, as `EnglishChunker.getChunkTagsForReadings` computes them
    /// (debug/oracle helper for `lt-cli`-side comparisons).
    pub fn opennlp_tag_sentence(&self, sentence: &str) -> Vec<(String, String, String)> {
        let tokens = self.tokenize(sentence);
        let tags = self.pos_tag(&tokens);
        let chunks = self.chunk_tokens(&tokens, &tags);
        tokens
            .into_iter()
            .zip(tags)
            .zip(chunks)
            .map(|((t, p), c)| (t, p, c))
            .collect()
    }

    /// `EnglishChunker.tokenize`: TokenizerME over the sentence with the
    /// typographic apostrophe replaced (OpenNLP expects `'`).
    fn tokenize(&self, sentence: &str) -> Vec<String> {
        let cleaned = sentence.replace('’', "'");
        tokenize_pos(&self.token_model, &cleaned)
            .into_iter()
            .map(|s| cleaned[s.start..s.end].to_string())
            .collect()
    }

    fn pos_tag(&self, tokens: &[String]) -> Vec<String> {
        pos::pos_tag(&self.pos_model, Some(&self.tagdict), tokens)
    }

    fn chunk_tokens(&self, tokens: &[String], tags: &[String]) -> Vec<String> {
        let tuples: Vec<TokenTag> = tokens
            .iter()
            .cloned()
            .zip(tags.iter().cloned())
            .map(|(token, tag)| TokenTag { token, tag })
            .collect();
        chunk::chunk(&self.chunk_model, &tuples)
    }

    /// `EnglishChunkFilter.filter`: the chunker tags noun phrases `B-NP`/
    /// `I-NP`; LT refines them into `B-NP-singular`/`B-NP-plural` with
    /// `I-`/`E-` continuation markers. Non-NP chunk tags are kept as-is.
    fn chunk_filter(&self, tokens: Vec<ChunkTaggedToken>) -> Vec<ChunkTaggedToken> {
        let n = tokens.len();
        let is_begin_np = |t: &ChunkTaggedToken| t.chunk_tags.iter().any(|c| c == "B-NP");
        let is_in_np = |t: &ChunkTaggedToken| t.chunk_tags.iter().any(|c| c == "I-NP");
        let is_end_np = |i: usize| i >= n.saturating_sub(1) || !is_in_np(&tokens[i + 1]);

        // chunk type of the phrase starting at chunk_start_pos
        let chunk_type = |chunk_start_pos: usize| -> bool {
            let mut is_plural = false;
            for t in &tokens[chunk_start_pos..] {
                if !is_begin_np(t) && !is_in_np(t) {
                    break;
                }
                if t.has_nns {
                    is_plural = true;
                }
            }
            is_plural
        };
        let end_of_np_is_singular = |i: usize| -> bool {
            for j in i..n {
                if is_end_np(j) {
                    return !chunk_type(j);
                }
            }
            false
        };

        let mut result: Vec<ChunkTaggedToken> = Vec::with_capacity(n);
        let mut new_chunk_tag: Option<String> = None;
        for (i, t) in tokens.iter().enumerate() {
            let mut chunk_tags: Vec<String> = Vec::new();
            if is_begin_np(t) {
                if !chunk_type(i) || end_of_np_is_singular(i) {
                    chunk_tags.push("B-NP-singular".to_string());
                    new_chunk_tag = Some("NP-singular".to_string());
                } else {
                    chunk_tags.push("B-NP-plural".to_string());
                    new_chunk_tag = Some("NP-plural".to_string());
                }
            }
            if let Some(nct) = &new_chunk_tag {
                if is_end_np(i) {
                    chunk_tags.push(format!("E-{nct}"));
                    new_chunk_tag = None;
                }
            }
            if let Some(nct) = &new_chunk_tag {
                if is_in_np(t) {
                    chunk_tags.push(format!("I-{nct}"));
                }
            }
            if chunk_tags.is_empty() {
                result.push(ChunkTaggedToken {
                    token: t.token.clone(),
                    chunk_tags: t.chunk_tags.clone(),
                    readings: t.readings,
                    has_nns: t.has_nns,
                });
            } else {
                result.push(ChunkTaggedToken {
                    token: t.token.clone(),
                    chunk_tags,
                    readings: t.readings,
                    has_nns: t.has_nns,
                });
            }
        }
        result
    }
}
