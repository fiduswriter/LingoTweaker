//! OpenNLP POS tagger (1.5-era classic features): `DefaultPOSContextGenerator`
//! + `DefaultPOSSequenceValidator` + the XML tag dictionary (`tags.tagdict`).

use std::collections::HashMap;

use lt_core::{CoreError, Result};

use crate::beam;
use crate::model::{FeatureSink, GenericModel};

/// `opennlp.tools.postag.POXDictionary`-equivalent: word → allowed tags,
/// loaded from the `<dictionary><entry tags="..."><token>...` XML.
pub struct PosDictionary {
    map: HashMap<String, Vec<String>>,
    case_sensitive: bool,
}

impl PosDictionary {
    pub fn read(bytes: &[u8]) -> Result<Self> {
        let mut map = HashMap::new();
        let mut reader = quick_xml::Reader::from_reader(bytes);
        use quick_xml::events::Event;
        let mut buf = Vec::new();
        let mut current_word: Option<String> = None;
        let mut current_tags: Option<Vec<String>> = None;
        // `DictionaryEntryPersistor` default: case-sensitive unless the
        // `caseSensitive` attribute explicitly says otherwise
        let mut case_sensitive = true;
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                    b"dictionary" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"caseSensitive" {
                                let v = attr
                                    .unescape_value()
                                    .map(|s| s.into_owned())
                                    .unwrap_or_default();
                                case_sensitive = v.eq_ignore_ascii_case("true");
                            }
                        }
                    }
                    b"entry" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"tags" {
                                let v = attr
                                    .unescape_value()
                                    .map(|s| s.into_owned())
                                    .unwrap_or_default();
                                current_tags = Some(v.split(' ').map(str::to_string).collect());
                            }
                        }
                    }
                    b"token" => {
                        current_word = None;
                    }
                    _ => {}
                },
                Ok(Event::Text(t)) => {
                    if let (None, Some(_)) = (&current_word, &current_tags) {
                        current_word = Some(
                            t.unescape()
                                .map(|s| s.into_owned())
                                .unwrap_or_default()
                                .trim()
                                .to_string(),
                        );
                    }
                }
                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"token" => {}
                    b"entry" => {
                        if let (Some(word), Some(tags)) = (current_word.take(), current_tags.take())
                        {
                            map.insert(word, tags);
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(CoreError::Data(format!("tagdict XML error: {e}")));
                }
                _ => {}
            }
            buf.clear();
        }
        if !case_sensitive {
            // Java `POSDictionary.create`: lower-case the keys at load time
            let lowered = map
                .iter()
                .map(|(k, v)| (k.to_lowercase(), v.clone()))
                .collect();
            map = lowered;
        }
        Ok(Self {
            map,
            case_sensitive,
        })
    }

    pub fn get_tags(&self, word: &str) -> Option<&Vec<String>> {
        if self.case_sensitive {
            self.map.get(word)
        } else {
            self.map.get(&word.to_lowercase())
        }
    }
}

const SE: &str = "*SE*";
const SB: &str = "*SB*";
const PREFIX_LENGTH: usize = 4;
const SUFFIX_LENGTH: usize = 4;

/// `DefaultPOSContextGenerator` with `dict == null` (the 1.5 POS model has
/// no `dictionary` artifact, so suffix/prefix features always fire).
///
/// The token-dependent features are built once per position (`PosPieces`);
/// the tag-history features (`p`/`t`/`pp`/`t2`) are added per beam candidate
/// from the outcome indices.
pub(crate) struct PosPieces {
    w: String,
    prevprev: String,
    n: String,
    nn: Option<String>,
    /// token-only feature strings (suffix/prefix/shape markers and `w`)
    static_features: Vec<String>,
}

pub(crate) fn pos_pieces(index: usize, tokens: &[String]) -> PosPieces {
    let lex = &tokens[index];
    // Java: nextnext = *SE* only when a next token exists but no nextnext
    // token; both stay null when the token itself is the last
    let (next, nextnext) = if tokens.len() > index + 1 {
        if tokens.len() > index + 2 {
            (tokens[index + 1].clone(), Some(tokens[index + 2].clone()))
        } else {
            (tokens[index + 1].clone(), Some(SE.to_string()))
        }
    } else {
        (SE.to_string(), None)
    };
    let mut static_features: Vec<String> = Vec::with_capacity(16);
    static_features.push("default".to_string());
    static_features.push(format!("w={lex}"));
    let char_count = lex.chars().count();
    for li in 0..SUFFIX_LENGTH {
        let sfx: String = lex
            .chars()
            .skip(char_count.saturating_sub(li + 1))
            .collect();
        static_features.push(format!("suf={sfx}"));
    }
    for li in 0..PREFIX_LENGTH {
        let pre: String = lex.chars().take(li + 1).collect();
        static_features.push(format!("pre={pre}"));
    }
    if lex.contains('-') {
        static_features.push("h".to_string());
    }
    if lex.chars().any(|c| c.is_ascii_uppercase()) {
        static_features.push("c".to_string());
    }
    if lex.chars().any(|c| c.is_ascii_digit()) {
        static_features.push("d".to_string());
    }
    PosPieces {
        w: format!("p={}", if index >= 1 { &tokens[index - 1] } else { SB }),
        prevprev: if index >= 2 {
            tokens[index - 2].clone()
        } else {
            SB.to_string()
        },
        n: format!("n={next}"),
        nn: nextnext.map(|nn| format!("nn={nn}")),
        static_features,
    }
}

pub(crate) fn pos_static(pieces: &PosPieces, sink: &mut FeatureSink<'_>) {
    for f in &pieces.static_features {
        sink.push(f);
    }
}

pub(crate) fn pos_dynamic(
    sink: &mut FeatureSink<'_>,
    model: &GenericModel,
    pieces: &PosPieces,
    preds: &[u32],
    index: usize,
) {
    sink.push(&pieces.w);
    if index >= 1 {
        if index >= 2 {
            sink.push2("pp=", &pieces.prevprev);
        } else {
            sink.push2("pp=", SB);
        }
    }
    sink.push(&pieces.n);
    if let Some(nn) = &pieces.nn {
        sink.push(nn);
    }
    if index >= 1 {
        sink.push2("t=", model.outcome(preds[index - 1]));
        if index >= 2 {
            sink.push4(
                "t2=",
                model.outcome(preds[index - 2]),
                ",",
                model.outcome(preds[index - 1]),
            );
        }
    }
}

/// `DefaultPOSSequenceValidator`: if the tag dictionary knows the word, the
/// outcome must be one of its tags.
pub fn pos_valid(
    model: &GenericModel,
    dict: Option<&PosDictionary>,
    i: usize,
    tokens: &[String],
    _outcomes: &[u32],
    outcome: u32,
) -> bool {
    match dict {
        None => true,
        Some(d) => match d.get_tags(&tokens[i]) {
            None => true,
            Some(tags) => tags.iter().any(|t| t == model.outcome(outcome)),
        },
    }
}

/// `POSTaggerME.tag` (beam size 3).
pub fn pos_tag(
    model: &GenericModel,
    dict: Option<&PosDictionary>,
    tokens: &[String],
) -> Vec<String> {
    let pieces: Vec<PosPieces> = (0..tokens.len()).map(|i| pos_pieces(i, tokens)).collect();
    let static_ids: Vec<Vec<u32>> = {
        let mut sink = FeatureSink::new(model);
        let mut ids = Vec::with_capacity(tokens.len());
        for p in &pieces {
            sink.clear();
            pos_static(p, &mut sink);
            ids.push(sink.ids.clone());
        }
        ids
    };
    let context = |i: usize, _toks: &[String], preds: &[u32], sink: &mut FeatureSink<'_>| {
        sink.clear();
        sink.ids.extend_from_slice(&static_ids[i]);
        pos_dynamic(sink, model, &pieces[i], preds, i);
    };
    let valid = |i: usize, toks: &[String], outcomes: &[u32], out: u32| {
        pos_valid(model, dict, i, toks, outcomes, out)
    };
    let sequences = beam::best_sequences(1, 3, tokens, model, &context, &valid);
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
