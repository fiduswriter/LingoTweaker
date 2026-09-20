//! OpenNLP chunker (`ChunkerME` + `DefaultChunkerContextGenerator` +
//! `DefaultChunkerSequenceValidator`) and tokenizer (`TokenizerME`).

use crate::beam;
use crate::model::{FeatureSink, GenericModel};

/// `DefaultChunkerContextGenerator` — note the upstream quirk that `p_2`
/// has no `=` separator (`"p_2" + preds[i - 2]`); reproduced exactly since
/// the predicates must match training.
///
/// The feature strings are built once per token position (`ChunkPieces`),
/// resolved to model predicate ids for the outcome-independent part, and
/// combined with the beam's previous outcomes per candidate.
pub(crate) struct ChunkPieces {
    w_2: String,
    w_1: String,
    w0: String,
    w1: String,
    w2: String,
    t_2: String,
    t_1: String,
    t0: String,
    t1: String,
    t2: String,
}

pub(crate) fn chunk_pieces(i: usize, toks: &[TokenTag]) -> ChunkPieces {
    let n = toks.len();
    ChunkPieces {
        w_2: if i < 2 {
            "w_2=bos".to_string()
        } else {
            format!("w_2={}", toks[i - 2].token)
        },
        w_1: if i < 1 {
            "w_1=bos".to_string()
        } else {
            format!("w_1={}", toks[i - 1].token)
        },
        w0: format!("w0={}", toks[i].token),
        w1: if i + 1 >= n {
            "w1=eos".to_string()
        } else {
            format!("w1={}", toks[i + 1].token)
        },
        w2: if i + 2 >= n {
            "w2=eos".to_string()
        } else {
            format!("w2={}", toks[i + 2].token)
        },
        t_2: if i < 2 {
            "t_2=bos".to_string()
        } else {
            format!("t_2={}", toks[i - 2].tag)
        },
        t_1: if i < 1 {
            "t_1=bos".to_string()
        } else {
            format!("t_1={}", toks[i - 1].tag)
        },
        t0: format!("t0={}", toks[i].tag),
        t1: if i + 1 >= n {
            "t1=eos".to_string()
        } else {
            format!("t1={}", toks[i + 1].tag)
        },
        t2: if i + 2 >= n {
            "t2=eos".to_string()
        } else {
            format!("t2={}", toks[i + 2].tag)
        },
    }
}

/// Outcome-independent chunk features, resolved once per position.
pub(crate) fn chunk_static_ids(pieces: &ChunkPieces, sink: &mut FeatureSink<'_>) {
    for f in [&pieces.w_2, &pieces.w_1, &pieces.w0, &pieces.w1, &pieces.w2] {
        sink.push(f);
    }
    sink.push2(&pieces.w_1, &pieces.w0);
    sink.push2(&pieces.w0, &pieces.w1);
    for f in [&pieces.t_2, &pieces.t_1, &pieces.t0, &pieces.t1, &pieces.t2] {
        sink.push(f);
    }
    sink.push2(&pieces.t_2, &pieces.t_1);
    sink.push2(&pieces.t_1, &pieces.t0);
    sink.push2(&pieces.t0, &pieces.t1);
    sink.push2(&pieces.t1, &pieces.t2);
    sink.push3(&pieces.t_2, &pieces.t_1, &pieces.t0);
    sink.push3(&pieces.t_1, &pieces.t0, &pieces.t1);
    sink.push3(&pieces.t0, &pieces.t1, &pieces.t2);
}

/// Outcome-dependent chunk features (the `p_*` group); `preds` are the
/// outcome indices chosen so far (length == `i`).
pub(crate) fn chunk_dynamic(
    sink: &mut FeatureSink<'_>,
    model: &GenericModel,
    pieces: &ChunkPieces,
    preds: &[u32],
    i: usize,
) {
    let p_2 = if i < 2 {
        "p_2=bos".to_string()
    } else {
        format!("p_2{}", model.outcome(preds[i - 2])) // no '=' — upstream quirk
    };
    let p_1 = if i < 1 {
        "p_1=bos".to_string()
    } else {
        format!("p_1={}", model.outcome(preds[i - 1]))
    };
    sink.push(&p_2);
    sink.push(&p_1);
    sink.push2(&p_2, &p_1);
    sink.push2(&p_1, &pieces.t_2);
    sink.push2(&p_1, &pieces.t_1);
    sink.push2(&p_1, &pieces.t0);
    sink.push2(&p_1, &pieces.t1);
    sink.push2(&p_1, &pieces.t2);
    sink.push3(&p_1, &pieces.t_2, &pieces.t_1);
    sink.push3(&p_1, &pieces.t_1, &pieces.t0);
    sink.push3(&p_1, &pieces.t0, &pieces.t1);
    sink.push3(&p_1, &pieces.t1, &pieces.t2);
    sink.push4(&p_1, &pieces.t_2, &pieces.t_1, &pieces.t0);
    sink.push4(&p_1, &pieces.t_1, &pieces.t0, &pieces.t1);
    sink.push4(&p_1, &pieces.t0, &pieces.t1, &pieces.t2);
    sink.push2(&p_1, &pieces.w_2);
    sink.push2(&p_1, &pieces.w_1);
    sink.push2(&p_1, &pieces.w0);
    sink.push2(&p_1, &pieces.w1);
    sink.push2(&p_1, &pieces.w2);
    sink.push3(&p_1, &pieces.w_1, &pieces.w0);
    sink.push3(&p_1, &pieces.w0, &pieces.w1);
}

/// `DefaultChunkerSequenceValidator`: `I-` outcomes must continue the same
/// chunk type as the previous outcome (Java: `substring(2)` comparison).
fn chunk_valid(model: &GenericModel, outcomes: &[u32], outcome: u32) -> bool {
    let label = model.outcome(outcome);
    if label.starts_with("I-") {
        return match outcomes.last() {
            None => false,
            Some(&prev) => {
                let prev = model.outcome(prev);
                if prev == "O" {
                    false
                } else {
                    prev.chars().skip(2).eq(label.chars().skip(2))
                }
            }
        };
    }
    true
}

/// `ChunkerME.chunk` (beam size 10). The beam's partial outcome sequence
/// provides the previous predictions (`p_1`, `p_2` features).
pub fn chunk(model: &GenericModel, toks: &[TokenTag]) -> Vec<String> {
    let pieces: Vec<ChunkPieces> = (0..toks.len()).map(|i| chunk_pieces(i, toks)).collect();
    let static_ids: Vec<Vec<u32>> = {
        let mut sink = FeatureSink::new(model);
        let mut ids = Vec::with_capacity(toks.len());
        for p in &pieces {
            sink.clear();
            chunk_static_ids(p, &mut sink);
            ids.push(sink.ids.clone());
        }
        ids
    };
    let context = |i: usize, _toks: &[TokenTag], preds: &[u32], sink: &mut FeatureSink<'_>| {
        sink.clear();
        sink.ids.extend_from_slice(&static_ids[i]);
        chunk_dynamic(sink, model, &pieces[i], preds, i);
    };
    let valid =
        |_i: usize, _t: &[TokenTag], outcomes: &[u32], out: u32| chunk_valid(model, outcomes, out);
    let sequences = beam::best_sequences(1, 10, toks, model, &context, &valid);
    sequences
        .into_iter()
        .next()
        .map(|s| {
            s.outcomes
                .iter()
                .map(|&o| model.outcome(o).to_string())
                .collect()
        })
        .unwrap_or_default()
}

pub struct TokenTag {
    pub token: String,
    pub tag: String,
}

/// Tokenizer split decision label (`TokenizerME.SPLIT`; `NO_SPLIT` is "F").
pub const SPLIT: &str = "T";

/// `DefaultTokenContextGenerator` (no induced abbreviations). Features are
/// resolved to predicate ids through a reusable [`FeatureSink`].
pub fn token_context(sink: &mut FeatureSink<'_>, sentence: &str, index: usize) {
    let chars: Vec<char> = sentence.chars().collect();
    let prefix: String = chars[..index].iter().collect();
    let suffix: String = chars[index..].iter().collect();
    sink.push2("p=", &prefix);
    sink.push2("s=", &suffix);
    if index > 0 {
        add_char_preds(sink, "p1", chars[index - 1]);
        if index > 1 {
            add_char_preds(sink, "p2", chars[index - 2]);
            sink.buf_push3(
                "p21=",
                &chars[index - 2].to_string(),
                &chars[index - 1].to_string(),
            );
        } else {
            sink.push("p2=bok");
        }
        sink.buf_push3(
            "p1f1=",
            &chars[index - 1].to_string(),
            &chars[index].to_string(),
        );
    } else {
        sink.push("p1=bok");
    }
    add_char_preds(sink, "f1", chars[index]);
    if index + 1 < chars.len() {
        add_char_preds(sink, "f2", chars[index + 1]);
        sink.buf_push3(
            "f12=",
            &chars[index].to_string(),
            &chars[index + 1].to_string(),
        );
    } else {
        sink.push("f2=bok");
    }
    if chars.first() == Some(&'&') && chars.last() == Some(&';') {
        sink.push("cc");
    }
    // pabb requires induced abbreviations (absent in this model)
}

fn add_char_preds(sink: &mut FeatureSink<'_>, key: &str, c: char) {
    let cbuf = c.to_string();
    sink.buf_push3(key, "=", &cbuf);
    if c.is_alphabetic() {
        sink.buf_push3(key, "_", "alpha");
        if c.is_uppercase() {
            sink.buf_push3(key, "_", "caps");
        }
    } else if c.is_numeric() {
        sink.buf_push3(key, "_", "num");
    } else if c.is_whitespace() {
        sink.buf_push3(key, "_", "ws");
    } else {
        match c {
            '.' | '?' | '!' => sink.buf_push3(key, "_", "eos"),
            '`' | '"' | '\'' => sink.buf_push3(key, "_", "quote"),
            '[' | '{' | '(' => sink.buf_push3(key, "_", "lp"),
            ']' | '}' | ')' => sink.buf_push3(key, "_", "rp"),
            _ => {}
        }
    }
}

/// A tokenizer output span (char offsets, like `opennlp.tools.util.Span`).
#[derive(Debug)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// `TokenizerME.tokenizePos` (greedy per-character split decisions;
/// `useAlphaNumericOptimization=true` with `^[A-Za-z0-9]+$`).
pub fn tokenize_pos(model: &GenericModel, sentence: &str) -> Vec<Span> {
    let chars: Vec<(usize, char)> = sentence.char_indices().collect();
    let mut tokens: Vec<Span> = Vec::new();
    let mut probs: Vec<f64> = Vec::new();

    // WhitespaceTokenizer: spans of non-whitespace runs (char boundaries)
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    for (byte_pos, c) in &chars {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                spans.push((s, *byte_pos));
            }
        } else if start.is_none() {
            start = Some(*byte_pos);
        }
    }
    if let Some(s) = start {
        spans.push((s, sentence.len()));
    }

    for (s, e) in spans {
        let tok = &sentence[s..e];
        let char_len = tok.chars().count();
        if char_len < 2 {
            tokens.push(Span { start: s, end: e });
            probs.push(1.0);
            continue;
        }
        // alphanumeric optimization: ^[A-Za-z0-9]+$
        if !tok.is_empty() && tok.chars().all(|c| c.is_ascii_alphanumeric()) {
            tokens.push(Span { start: s, end: e });
            probs.push(1.0);
            continue;
        }
        let mut start = s;
        let mut token_prob = 1.0f64;
        let char_offsets: Vec<usize> = chars
            .iter()
            .skip_while(|(p, _)| *p < s)
            .take(char_len)
            .map(|(p, _)| *p)
            .collect();
        let mut sink = FeatureSink::new(model);
        let mut sums = vec![0.0f64; model.num_outcomes()];
        for (ci, &byte_pos) in char_offsets.iter().enumerate().skip(1) {
            sink.clear();
            token_context(&mut sink, tok, ci);
            model.eval_ids(&sink.ids, &mut sums);
            let best = model.best_outcome(&sums);
            token_prob *= sums[best];
            if model.outcome(best as u32) == SPLIT {
                tokens.push(Span {
                    start,
                    end: byte_pos,
                });
                probs.push(token_prob);
                start = byte_pos;
                token_prob = 1.0;
            }
        }
        tokens.push(Span { start, end: e });
        probs.push(token_prob);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GenericModel, ModelFile};

    fn token_model() -> Option<GenericModel> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/en/models");
        let mf = ModelFile::load(&dir.join("en-token.bin")).ok()?;
        GenericModel::read(mf.entries.get("token.model")?).ok()
    }

    /// `TokenizerME.SPLIT` is "T" (not "SPLIT"): punctuation after a word
    /// must produce its own token.
    #[test]
    fn splits_punctuation_from_words() {
        let Some(model) = token_model() else {
            eprintln!("skipping: no vendored models");
            return;
        };
        let spans = tokenize_pos(&model, "He walk to the building every day.");
        let toks: Vec<&str> = spans
            .iter()
            .map(|s| &"He walk to the building every day."[s.start..s.end])
            .collect();
        assert_eq!(
            toks,
            vec!["He", "walk", "to", "the", "building", "every", "day", "."],
            "spans: {spans:?}"
        );
    }
}
