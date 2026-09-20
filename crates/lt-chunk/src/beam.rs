//! Maxent beam search (port of `opennlp.tools.ml.BeamSearch` + `Sequence`).
//!
//! The beam keeps the `size` best-scoring partial sequences; ties at the
//! cutoff are all kept (Java behavior). Heap tie-breaking order may differ
//! from Java's `PriorityQueue` for exactly-equal scores (documented
//! deviation, D-008).
//!
//! Outcomes are carried as model-internal indices (never strings); predicate
//! ids are resolved by the context generators through [`FeatureSink`], so the
//! inner loop performs no per-candidate string allocation.

use crate::model::{FeatureSink, GenericModel};

#[derive(Clone, Default)]
pub struct Sequence {
    pub outcomes: Vec<u32>,
    score: f64,
}

impl Sequence {
    fn extend(&self, outcome: u32, log_p: f64) -> Self {
        let mut outcomes = self.outcomes.clone();
        outcomes.push(outcome);
        Self {
            outcomes,
            score: self.score + log_p,
        }
    }
}

struct HeapItem(Sequence);

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.cmp_score() == other.cmp_score()
    }
}
impl Eq for HeapItem {}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cmp_score().total_cmp(&other.cmp_score())
    }
}
impl HeapItem {
    // Java Sequence.compareTo: Double.compare(other.score, this.score) — a
    // higher score is "smaller" in Java's min-heap, so it pops first; model
    // that with a max-heap keyed on the score.
    fn cmp_score(&self) -> f64 {
        self.0.score
    }
}

/// BinaryHeap popping the smallest negated score = the highest score.
type Beam = std::collections::BinaryHeap<HeapItem>;

/// Port of `BeamSearch.bestSequences` with `minSequenceScore = zeroLog`
/// (`-100000.0`). The context closure fills `sink` with the predicate ids of
/// the current position; the validation closure decides whether an outcome
/// index may follow the current outcome sequence.
pub type ContextFn<'a, T> = &'a (dyn Fn(usize, &[T], &[u32], &mut FeatureSink<'_>) + 'a);
pub type ValidFn<'a, T> = &'a (dyn Fn(usize, &[T], &[u32], u32) -> bool + 'a);

pub fn best_sequences<T>(
    num_sequences: usize,
    beam_size: usize,
    sequence: &[T],
    model: &GenericModel,
    context: ContextFn<'_, T>,
    valid: ValidFn<'_, T>,
) -> Vec<Sequence> {
    const MIN_SEQUENCE_SCORE: f64 = -100_000.0;
    let mut prev: Beam = Beam::new();
    let mut next: Beam = Beam::new();
    let mut sink = FeatureSink::new(model);
    let mut sums = vec![0.0f64; model.num_outcomes()];
    let mut sorted: Vec<f64> = vec![0.0f64; model.num_outcomes()];
    prev.push(HeapItem(Sequence::default()));

    for i in 0..sequence.len() {
        let sz = beam_size.min(prev.len());
        for _ in 0..sz {
            let top = match prev.pop() {
                Some(t) => t.0,
                None => break,
            };
            context(i, sequence, &top.outcomes, &mut sink);
            model.eval_ids(&sink.ids, &mut sums);

            sorted.copy_from_slice(&sums);
            sorted.sort_by(|a, b| a.total_cmp(b));
            let min = sorted[sums.len().saturating_sub(beam_size)];

            for (p, &score) in sums.iter().enumerate() {
                if score >= min {
                    let out = p as u32;
                    if valid(i, sequence, &top.outcomes, out) {
                        let ns = top.extend(out, score.ln());
                        if ns.score > MIN_SEQUENCE_SCORE {
                            next.push(HeapItem(ns));
                        }
                    }
                }
            }
            if next.is_empty() {
                for (p, &score) in sums.iter().enumerate() {
                    let out = p as u32;
                    if valid(i, sequence, &top.outcomes, out) {
                        let ns = top.extend(out, score.ln());
                        if ns.score > MIN_SEQUENCE_SCORE {
                            next.push(HeapItem(ns));
                        }
                    }
                }
            }
        }
        std::mem::swap(&mut prev, &mut next);
        next.clear();
    }

    let num_seq = num_sequences.min(prev.len());
    let mut out = Vec::with_capacity(num_seq);
    for _ in 0..num_seq {
        if let Some(item) = prev.pop() {
            out.push(item.0);
        }
    }
    out
}
