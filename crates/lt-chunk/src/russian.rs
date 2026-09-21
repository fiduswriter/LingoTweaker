//! `org.languagetool.chunking.RussianChunker` — the rule-based Russian
//! chunker used as `Russian`'s post-disambiguation chunker.
//!
//! Structurally the German chunker's sibling (both use OpenRegex); the port
//! reuses [`crate::openregex`]. Behavioural notes:
//!
//! * the working list contains only non-whitespace tokens that do **not**
//!   carry the `MayMissingYO` chunk tag (set by `RussianTagger`);
//! * `overwrite` regexes drop existing `PP`/`NPP`/`NPS`/`MayMissingYO`/`VP`/
//!   `SBAR`/`ADJP`/`DPT` tags before adding their own.

use fancy_regex::Regex as FancyRegex;
use lt_core::{AnalyzedTokenReadings, CoreError, Result};

use crate::openregex::{compile_logic, ChunkTok, OpenRegex, TokenPredicate, TokenView};

/// `RussianChunker.FILTER_TAGS`.
const FILTER_TAGS: [&str; 8] = [
    "PP",
    "NPP",
    "NPS",
    "MayMissingYO",
    "VP",
    "SBAR",
    "ADJP",
    "DPT",
];

/// `RussianChunker.SYNTAX_EXPANSION`.
const SYNTAX_EXPANSION: [(&str, &str); 4] = [
    ("<NP>", "<chunk=B-NP> <chunk=I-NP>*"),
    ("<VP>", "<chunk=B-VP> <chunk=I-VP>*"),
    ("<ADJP>", "<chunk=B-ADJP> <chunk=I-ADJP>*"),
    ("<DPT>", "<chunk=B-DPT> <chunk=I-DPT>*"),
];

/// `RussianChunker.PhraseType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PhraseType {
    Np,
    Nps,
    Npp,
    Pp,
    MayMissingYo,
    Vp,
    Sbar,
    Adjp,
    Dpt,
}

impl PhraseType {
    fn name(self) -> &'static str {
        match self {
            Self::Np => "NP",
            Self::Nps => "NPS",
            Self::Npp => "NPP",
            Self::Pp => "PP",
            Self::MayMissingYo => "MayMissingYO",
            Self::Vp => "VP",
            Self::Sbar => "SBAR",
            Self::Adjp => "ADJP",
            Self::Dpt => "DPT",
        }
    }
}

/// Java `String.equalsIgnoreCase` (per-char upper/lower comparison).
fn java_equals_ignore_case(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    if ac.len() != bc.len() {
        return false;
    }
    ac.iter().zip(bc.iter()).all(|(&x, &y)| {
        x == y || x.to_uppercase().eq(y.to_uppercase()) || x.to_lowercase().eq(y.to_lowercase())
    })
}

/// `StringMatcher.create(value, isRegExp, caseSensitive)` for the full-match
/// `matches()` case; `(?i)` stands in for Java's `CASE_INSENSITIVE |
/// UNICODE_CASE`.
fn java_full_match(pattern: &str, case_sensitive: bool) -> Result<FancyRegex> {
    let full = if case_sensitive {
        format!("^(?:{pattern})$")
    } else {
        format!("(?i)^(?:{pattern})$")
    };
    FancyRegex::new(&full)
        .map_err(|e| CoreError::Parse("RussianChunker regex".into(), format!("{pattern}: {e}")))
}

/// `TokenPredicate`/`TokenExpressionFactory` compiled for one `<...>` token.
struct ChunkPredicate {
    kind: PredKind,
}

enum PredKind {
    /// `string` — `StringMatcher.create(value, false, caseSensitive=false)`
    Str(String),
    /// `regex` — regex, case-insensitive
    Regex(FancyRegex),
    /// `regexCS` — regex, case-sensitive
    RegexCs(FancyRegex),
    /// `chunk` — `StringMatcher.regexp`, case-sensitive
    Chunk(FancyRegex),
    /// `pos` — substring of any POSTag
    Pos(String),
    /// `posre`/`posregex` — regex over the POSTags, case-sensitive
    PosRegex(FancyRegex),
}

impl TokenPredicate for ChunkPredicate {
    fn apply(&self, token: &TokenView<'_>) -> bool {
        let matches_readings = |f: &dyn Fn(&str) -> bool| {
            token.readings.is_some_and(|readings| {
                readings
                    .readings
                    .iter()
                    .any(|t| t.pos_tag.as_deref().is_some_and(f))
            })
        };
        match &self.kind {
            PredKind::Str(value) => java_equals_ignore_case(token.token, value),
            PredKind::Regex(re) => re.is_match(token.token).unwrap_or(false),
            PredKind::RegexCs(re) => re.is_match(token.token).unwrap_or(false),
            PredKind::Chunk(re) => token
                .chunk_tags
                .iter()
                .any(|tag| re.is_match(tag).unwrap_or(false)),
            PredKind::Pos(value) => matches_readings(&|pos| pos.contains(value.as_str())),
            PredKind::PosRegex(re) => matches_readings(&|pos| re.is_match(pos).unwrap_or(false)),
        }
    }
}

fn unquote(s: &str) -> &str {
    s.strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .unwrap_or(s)
}

/// Java `String.split("=")` with limit 0.
fn java_split_eq(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return vec![""];
    }
    let mut parts: Vec<&str> = s.split('=').collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

fn compile_chunk_expression(description: &str) -> Result<Box<dyn TokenPredicate>> {
    Ok(Box::new(compile_logic(description, &mut |arg| {
        compile_token_predicate(arg)
    })?))
}

/// `TokenPredicate.compile(description)`: one `type=value` argument.
fn compile_token_predicate(description: &str) -> Result<Box<dyn TokenPredicate>> {
    let parts = java_split_eq(description);
    let (expr_type, expr_value) = match parts.len() {
        1 => ("string", unquote(parts[0])),
        2 => (parts[0], unquote(parts[1])),
        _ => {
            return Err(CoreError::Parse(
                "RussianChunker".into(),
                format!("could not parse expression: {description}"),
            ))
        }
    };
    let kind = match expr_type {
        "string" => PredKind::Str(expr_value.to_string()),
        "regex" => PredKind::Regex(java_full_match(expr_value, false)?),
        "regexCS" => PredKind::RegexCs(java_full_match(expr_value, true)?),
        "chunk" => PredKind::Chunk(java_full_match(expr_value, true)?),
        "pos" => PredKind::Pos(expr_value.to_string()),
        "posre" | "posregex" => PredKind::PosRegex(java_full_match(expr_value, true)?),
        other => {
            return Err(CoreError::Parse(
                "RussianChunker".into(),
                format!("expression type not supported: '{other}'"),
            ))
        }
    };
    Ok(Box::new(ChunkPredicate { kind }))
}

struct Spec {
    expr: String,
    phrase_type: PhraseType,
    overwrite: bool,
}

/// `RussianChunker.build(expr, type, overwrite)`.
fn build(expr: &str, phrase_type: PhraseType, overwrite: bool) -> Spec {
    let mut expanded = expr.to_string();
    for (from, to) in SYNTAX_EXPANSION {
        expanded = expanded.replace(from, to);
    }
    Spec {
        expr: expanded,
        phrase_type,
        overwrite,
    }
}

fn regexes1() -> Vec<Spec> {
    use PhraseType::*;
    vec![
        // Иванов Иван Иванович
        build(
            "<posre='NN:(Name|Fam|Patr):.*'> <posre='NN:(Name|Fam|Patr):.*'>+ ",
            Np,
            true,
        ),
        // Иванов И.И.
        build(
            "<posre='NN:Fam:.*'> <regexCS=[А-ЯЁ]> <.> <regexCS=[А-ЯЁ]> <.> ",
            Np,
            true,
        ),
        // И.И. Иванов
        build(
            "<regexCS=[А-ЯЁ]> <.> <regexCS=[А-ЯЁ]> <.> <posre='NN:Fam:.*'> ",
            Np,
            true,
        ),
        // verb+verb
        build("<posre='VB:.*:.*' & !posre='NN:.*'>* ", Vp, false),
        build("<если>", Sbar, false),
        build("<поэтому>", Sbar, false),
        // noun phrase
        build(
            "<posre='ADJ:Posit:.*:.*'> <posre='NN:(Anim|Inanim):.*' & !posre='NN:(Anim|Inanim):.*:(R|D|T|P)'> ",
            Np,
            true,
        ),
        build(
            "<posre='ADJ:Posit:.*:.*'> <posre='NN:(Anim|Inanim):.*' & !posre='NN:(Anim|Inanim):.*:(R|D|T|P)'> <posre='NN:(Anim|Inanim):.*'> ",
            Np,
            true,
        ),
        // adj -> participle phrase
        build(
            "<posre='ADJ:Posit:.*:.*'> <posre='NN:(Anim|Inanim):.*' & !posre='NN:(Anim|Inanim):.*:(Nom|V)'> <posre='NN:(Anim|Inanim):.*:(Nom|V)' & !posre='NN:(Anim|Inanim):.*:(R|D|T|P)'> ",
            Adjp,
            true,
        ),
        // adverbial participle
        build("<posre='DPT:.*:.*' & !pos='PREP'> ", Dpt, false),
        build(
            "<posre='DPT:.*:.*' & !pos='PREP'> <posre='NN:.*:.*:(R|D|T|P)' > ",
            Dpt,
            true,
        ),
        build(
            "<posre='DPT:.*:.*' & !pos='PREP'> <posre='PREP'> <posre='NN:.*:.*:(R|D|T|P)' > ",
            Dpt,
            true,
        ),
        // participle
        build("<posre='PT:.*:.*'> ", Adjp, false),
        build("<posre='PT:.*:.*'> <pos='ADV' > ", Adjp, true),
        build(
            "<posre='PT:.*:.*'> <posre='NN:.*:.*:(R|D|T|P)' > ",
            Adjp,
            true,
        ),
        build(
            "<posre='PT:.*:.*'> <posre='PREP'> <posre='NN:.*:.*:(R|D|T|P|V)' > ",
            Adjp,
            true,
        ),
        build(
            "<posre='PT:.*:.*'> <posre='PREP'> <posre='ADJ:.*:.*:(R|D|T|P|V)' > <posre='NN:.*:.*:(R|D|T|P|V)' > ",
            Adjp,
            true,
        ),
        build(
            "<posre='PT:.*:.*'> <posre='NN:(Anim|Inanim):.*' & !posre='NN:(Anim|Inanim):.*:(Nom|V)'> <posre='NN:(Anim|Inanim):.*:(Nom|V)' & !posre='NN:(Anim|Inanim):.*:(R|D|T|P)'> ",
            Adjp,
            true,
        ),
        build(
            "<posre='PT:.*:.*'> <posre='PNN:.*' & !posre='PNN:.*:Nom:.*'> <posre='NN:(Anim|Inanim):.*:(Nom|V)' & !posre='NN:(Anim|Inanim):.*:(R|D|T|P)'> ",
            Adjp,
            true,
        ),
        build("<posre='PT:.*:.*'> <posre='ADJ:.*:.*' > ", Adjp, false),
        build("<тов>", Np, false),
    ]
}

fn regexes2() -> Vec<Spec> {
    use PhraseType::*;
    vec![
        // ===== plural and singular noun phrases =====
        // "Маша и Миша":
        build("<posre=NN:Name:.*> <и> <posre=NN:Name:.*>", Npp, true),
        build("<posre=NN:Name:.*> <или> <posre=NN:Name:.*>", Npp, true),
        // не + VB
        build("<не> <posre='VB:.*:.*' & !posre='NN:.*'>* ", Vp, false),
    ]
}

struct CompiledChunkRegex {
    regex: OpenRegex,
    phrase_type: PhraseType,
    overwrite: bool,
}

impl CompiledChunkRegex {
    fn compile(spec: &Spec) -> Result<Self> {
        let regex = OpenRegex::compile(&spec.expr, &mut |desc| compile_chunk_expression(desc))?;
        Ok(Self {
            regex,
            phrase_type: spec.phrase_type,
            overwrite: spec.overwrite,
        })
    }
}

/// `org.languagetool.chunking.RussianChunker`.
pub struct RussianChunker {
    regexes1: Vec<CompiledChunkRegex>,
    regexes2: Vec<CompiledChunkRegex>,
}

impl RussianChunker {
    pub fn new() -> Result<Self> {
        let regexes1 = regexes1()
            .iter()
            .map(CompiledChunkRegex::compile)
            .collect::<Result<Vec<_>>>()?;
        let regexes2 = regexes2()
            .iter()
            .map(CompiledChunkRegex::compile)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { regexes1, regexes2 })
    }

    /// `RussianChunker.addChunkTags`.
    pub fn add_chunk_tags(&self, tokens: &mut [AnalyzedTokenReadings]) {
        let mut chunk_tokens: Vec<ChunkTok> = Vec::new();
        let mut token_indices: Vec<usize> = Vec::new();
        for (i, readings) in tokens.iter().enumerate() {
            let surface = readings.surface();
            if readings.is_whitespace
                || surface.is_empty()
                || readings.is_sentence_start
                || readings.chunk_tags.iter().any(|t| t == "MayMissingYO")
            {
                continue;
            }
            chunk_tokens.push(ChunkTok {
                token: surface.to_string(),
                chunk_tags: vec!["O".to_string()],
                readings: Some(i),
            });
            token_indices.push(i);
        }
        for regex in &self.regexes1 {
            self.apply(regex, &mut chunk_tokens, tokens);
        }
        for regex in &self.regexes2 {
            self.apply(regex, &mut chunk_tokens, tokens);
        }
        for (chunk_index, &token_index) in token_indices.iter().enumerate() {
            tokens[token_index].chunk_tags = chunk_tokens[chunk_index].chunk_tags.clone();
        }
    }

    fn apply(
        &self,
        regex: &CompiledChunkRegex,
        chunk_tokens: &mut [ChunkTok],
        tokens: &[AnalyzedTokenReadings],
    ) {
        let matches = regex.regex.find_all(chunk_tokens, tokens);
        for (start, end) in matches {
            for (offset, token) in chunk_tokens[start..end].iter_mut().enumerate() {
                let i = start + offset;
                let mut new_tags = token.chunk_tags.clone();
                if regex.overwrite {
                    new_tags.retain(|tag| !FILTER_TAGS.contains(&tag.as_str()));
                }
                let new_tag = match regex.phrase_type {
                    PhraseType::Np => {
                        if i == start {
                            "B-NP"
                        } else {
                            "I-NP"
                        }
                    }
                    PhraseType::Npp => {
                        if i == start {
                            "B-NP-plural"
                        } else {
                            "I-NP-plural"
                        }
                    }
                    PhraseType::Vp => {
                        if i == start {
                            "B-VP"
                        } else {
                            "I-VP"
                        }
                    }
                    PhraseType::Adjp => {
                        if i == start {
                            "B-ADJP"
                        } else {
                            "I-ADJP"
                        }
                    }
                    PhraseType::Dpt => {
                        if i == start {
                            "B-DPT"
                        } else {
                            "I-DPT"
                        }
                    }
                    other => other.name(),
                };
                if !new_tags.iter().any(|tag| tag == new_tag) {
                    new_tags.push(new_tag.to_string());
                    new_tags.retain(|tag| tag != "O");
                }
                *token = ChunkTok {
                    token: token.token.clone(),
                    chunk_tags: new_tags,
                    readings: token.readings,
                };
            }
        }
    }
}
