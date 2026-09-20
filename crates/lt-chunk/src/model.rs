//! Reader for Java `DataInputStream` "modified UTF-8" strings and the
//! OpenNLP `GenericModel` binary format (`opennlp.tools.ml.model.*`, the
//! `pos.model` / `chunker.model` / `token.model` entries of LT's vendored
//! 1.5-era OpenNLP models).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use lt_core::{CoreError, Result};

/// Sequential reader over a model entry's bytes (Java `DataInputStream`
/// semantics: big-endian ints/doubles, `readUTF` modified UTF-8).
pub struct DataReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> DataReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    pub fn read_int(&mut self) -> Result<i32> {
        let b: [u8; 4] = self
            .bytes
            .get(self.pos..self.pos + 4)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| CoreError::Data("model entry truncated (int)".into()))?;
        self.pos += 4;
        Ok(i32::from_be_bytes(b))
    }

    pub fn read_double(&mut self) -> Result<f64> {
        let b: [u8; 8] = self
            .bytes
            .get(self.pos..self.pos + 8)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| CoreError::Data("model entry truncated (double)".into()))?;
        self.pos += 8;
        Ok(f64::from_be_bytes(b))
    }

    /// Java `DataInputStream.readUTF`: 2-byte unsigned length prefix, then
    /// modified UTF-8 (NUL as 0xC0 0x80, supplementary chars as surrogate
    /// pairs of 3-byte CESU-8 sequences).
    pub fn read_utf(&mut self) -> Result<String> {
        let b: [u8; 2] = self
            .bytes
            .get(self.pos..self.pos + 2)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| CoreError::Data("model entry truncated (utf len)".into()))?;
        self.pos += 2;
        let len = u16::from_be_bytes(b) as usize;
        let start = self.pos;
        let end = start
            .checked_add(len)
            .filter(|&e| e <= self.bytes.len())
            .ok_or_else(|| CoreError::Data("model entry truncated (utf)".into()))?;
        let raw = &self.bytes[start..end];
        self.pos = end;
        decode_modified_utf8(raw)
    }
}

fn decode_modified_utf8(raw: &[u8]) -> Result<String> {
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;
    while i < raw.len() {
        let b = raw[i];
        let ch = if b & 0x80 == 0 {
            i += 1;
            b as char
        } else if b & 0xE0 == 0xC0 {
            let b2 = *raw
                .get(i + 1)
                .ok_or_else(|| CoreError::Data("bad modified utf8".into()))?;
            i += 2;
            let cp = (((b & 0x1F) as u32) << 6) | ((b2 & 0x3F) as u32);
            char::from_u32(cp).unwrap_or('\u{FFFD}')
        } else if b & 0xF0 == 0xE0 {
            // 3-byte sequence; may be a surrogate half (CESU-8)
            let b2 = *raw
                .get(i + 1)
                .ok_or_else(|| CoreError::Data("bad modified utf8".into()))?;
            let b3 = *raw
                .get(i + 2)
                .ok_or_else(|| CoreError::Data("bad modified utf8".into()))?;
            i += 3;
            let cp =
                (((b & 0x0F) as u32) << 12) | (((b2 & 0x3F) as u32) << 6) | ((b3 & 0x3F) as u32);
            match char::from_u32(cp) {
                Some(c) => c,
                None => {
                    // surrogate pair: combine with the next 3-byte half
                    let b4 = *raw
                        .get(i)
                        .ok_or_else(|| CoreError::Data("bad surrogate pair".into()))?;
                    let b5 = *raw
                        .get(i + 1)
                        .ok_or_else(|| CoreError::Data("bad surrogate pair".into()))?;
                    let b6 = *raw
                        .get(i + 2)
                        .ok_or_else(|| CoreError::Data("bad surrogate pair".into()))?;
                    if (b4 & 0xF0) == 0xE0 {
                        i += 3;
                        let low = (((b4 & 0x0F) as u32) << 12)
                            | (((b5 & 0x3F) as u32) << 6)
                            | ((b6 & 0x3F) as u32);
                        let combined = 0x10000 + (((cp & 0x3FF) << 10) | (low & 0x3FF));
                        char::from_u32(combined).unwrap_or('\u{FFFD}')
                    } else {
                        '\u{FFFD}'
                    }
                }
            }
        } else {
            return Err(CoreError::Data("bad modified utf8 lead byte".into()));
        };
        out.push(ch);
    }
    Ok(out)
}

/// Reusable feature buffer resolving feature names to predicate ids: the
/// context generators build their feature strings in `buf` (one allocation,
/// reused) and never materialize a `Vec<String>` per beam candidate.
pub struct FeatureSink<'m> {
    model: &'m GenericModel,
    pub ids: Vec<u32>,
    buf: String,
}

impl<'m> FeatureSink<'m> {
    pub fn new(model: &'m GenericModel) -> Self {
        Self {
            model,
            ids: Vec::with_capacity(64),
            buf: String::with_capacity(256),
        }
    }

    /// Resolve a feature name and add it (duplicates are kept, like Java's
    /// `eval` adding a predicate's weights once per occurrence).
    pub fn push(&mut self, feature: &str) {
        if let Some(id) = self.model.pred_id(feature) {
            self.ids.push(id);
        }
    }

    pub fn push2(&mut self, a: &str, b: &str) {
        self.buf.clear();
        self.buf.push_str(a);
        self.buf.push_str(b);
        let id = self.model.pred_id(&self.buf);
        if let Some(id) = id {
            self.ids.push(id);
        }
    }

    pub fn push3(&mut self, a: &str, b: &str, c: &str) {
        self.buf.clear();
        self.buf.push_str(a);
        self.buf.push_str(b);
        self.buf.push_str(c);
        let id = self.model.pred_id(&self.buf);
        if let Some(id) = id {
            self.ids.push(id);
        }
    }

    pub fn push4(&mut self, a: &str, b: &str, c: &str, d: &str) {
        self.buf.clear();
        self.buf.push_str(a);
        self.buf.push_str(b);
        self.buf.push_str(c);
        self.buf.push_str(d);
        let id = self.model.pred_id(&self.buf);
        if let Some(id) = id {
            self.ids.push(id);
        }
    }

    /// Build `a + b + c` (used for the tokenizer's character predicates,
    /// where the pieces are single chars or `&str` names).
    pub fn buf_push3(&mut self, a: &str, b: &str, c: &str) {
        self.buf.clear();
        self.buf.push_str(a);
        self.buf.push_str(b);
        self.buf.push_str(c);
        let id = self.model.pred_id(&self.buf);
        if let Some(id) = id {
            self.ids.push(id);
        }
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.buf.clear();
    }
}

/// One predicate's parameters: the outcomes it votes for and their weights.
pub struct Context {
    pub outcomes: Vec<usize>,
    pub parameters: Vec<f64>,
}

/// `opennlp.tools.ml.maxent.GISModel` loaded from the binary format:
/// type tag "GIS", outcome labels, outcome patterns, predicate labels,
/// then the parameter matrix.
pub struct GenericModel {
    outcome_labels: Vec<String>,
    pred_index: HashMap<String, usize>,
    params: Vec<Context>,
}

impl GenericModel {
    pub fn read(bytes: &[u8]) -> Result<Self> {
        let mut r = DataReader::new(bytes);
        let model_type = r.read_utf()?;
        if model_type != "GIS" {
            return Err(CoreError::Data(format!(
                "unsupported model type {model_type} (only GIS maxent models are supported)"
            )));
        }
        // correction constant / correction parameter (not used anymore)
        let _correction_constant = r.read_int()?;
        let _correction_param = r.read_double()?;

        let num_outcomes = r.read_int()? as usize;
        let mut outcome_labels = Vec::with_capacity(num_outcomes);
        for _ in 0..num_outcomes {
            outcome_labels.push(r.read_utf()?);
        }

        let num_patterns = r.read_int()? as usize;
        let mut patterns: Vec<(usize, Vec<usize>)> = Vec::with_capacity(num_patterns);
        for _ in 0..num_patterns {
            let spec = r.read_utf()?;
            let mut ints = spec
                .split(' ')
                .map(|s| s.parse::<usize>())
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| CoreError::Data(format!("bad outcome pattern '{spec}': {e}")))?;
            if ints.is_empty() {
                return Err(CoreError::Data("empty outcome pattern".into()));
            }
            let count = ints.remove(0);
            patterns.push((count, ints));
        }

        let num_preds = r.read_int()? as usize;
        let mut pred_labels = Vec::with_capacity(num_preds);
        let mut pred_index = HashMap::with_capacity(num_preds);
        for i in 0..num_preds {
            let label = r.read_utf()?;
            pred_index.insert(label.clone(), i);
            pred_labels.push(label);
        }

        // parameters: for each pattern, `count` contexts with one weight per
        // outcome in the pattern
        let mut params = Vec::with_capacity(num_preds);
        for (count, outcome_pattern) in &patterns {
            for _ in 0..*count {
                let mut weights = Vec::with_capacity(outcome_pattern.len());
                for _ in 0..outcome_pattern.len() {
                    weights.push(r.read_double()?);
                }
                params.push(Context {
                    outcomes: outcome_pattern.clone(),
                    parameters: weights,
                });
            }
        }
        if params.len() != num_preds {
            return Err(CoreError::Data(format!(
                "parameter count {} != predicate count {num_preds}",
                params.len()
            )));
        }
        let _ = pred_labels;
        Ok(Self {
            outcome_labels,
            pred_index,
            params,
        })
    }

    pub fn num_outcomes(&self) -> usize {
        self.outcome_labels.len()
    }

    pub fn outcome(&self, index: u32) -> &str {
        &self.outcome_labels[index as usize]
    }

    /// Predicate index of a feature name (`null` when the model has none).
    pub fn pred_id(&self, feature: &str) -> Option<u32> {
        self.pred_index.get(feature).map(|&i| i as u32)
    }

    /// `GISModel.eval` with the uniform prior: log(1/numOutcomes) plus the
    /// weighted votes of all active predicates, then softmax. The prior
    /// constant cancels out but is kept for faithfulness. `sums` is filled
    /// with the normalized probabilities (len == num outcomes).
    pub fn eval_ids(&self, ids: &[u32], sums: &mut [f64]) {
        let n = self.outcome_labels.len();
        debug_assert_eq!(sums.len(), n);
        let prior = -(1.0 / n as f64).ln();
        for s in sums.iter_mut() {
            *s = prior;
        }
        for &pid in ids {
            let ctx = &self.params[pid as usize];
            for (i, &o) in ctx.outcomes.iter().enumerate() {
                sums[o] += ctx.parameters[i];
            }
        }
        let mut normal = 0.0;
        for s in sums.iter_mut() {
            *s = s.exp();
            normal += *s;
        }
        if normal != 0.0 {
            for s in sums.iter_mut() {
                *s /= normal;
            }
        }
    }

    /// `model.getBestOutcome`: first argmax.
    pub fn best_outcome(&self, probs: &[f64]) -> usize {
        let mut max = 0usize;
        for (i, &p) in probs.iter().enumerate().skip(1) {
            if p > probs[max] {
                max = i;
            }
        }
        max
    }
}

/// Read one named entry out of an OpenNLP model "zip" file.
pub struct ModelFile {
    pub entries: HashMap<String, Vec<u8>>,
}

impl ModelFile {
    pub fn load(path: &Path) -> Result<Self> {
        let file = lt_data::fs::open(path)
            .map_err(|e| CoreError::Data(format!("cannot open {}: {e}", path.display())))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| {
            CoreError::Data(format!("{}: not an OpenNLP model: {e}", path.display()))
        })?;
        let mut entries = HashMap::new();
        for i in 0..zip.len() {
            let mut entry = zip
                .by_index(i)
                .map_err(|e| CoreError::Data(format!("{}: bad zip entry: {e}", path.display())))?;
            let name = entry.name().to_string();
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut buf)
                .map_err(|e| CoreError::Data(format!("{name}: read error: {e}")))?;
            entries.insert(name, buf);
        }
        Ok(Self { entries })
    }
}
