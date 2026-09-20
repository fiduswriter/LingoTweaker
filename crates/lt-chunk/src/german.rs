//! `org.languagetool.chunking.GermanChunker` — the rule-based German noun
//! phrase chunker used as `German`'s post-disambiguation chunker.
//!
//! The upstream class is 528 lines of OpenRegex expressions plus
//! `TokenExpressionFactory`/`TokenPredicate` (core chunking). Both are ported
//! here on top of [`crate::openregex`]. Behavioural notes:
//!
//! * the working list contains only non-whitespace tokens (Java
//!   `AnalyzedTokenReadings.isWhitespace()` is true for the empty SENT_START
//!   token as well);
//! * the `simpleFormRegexp` form hints skip a regex entirely when none of its
//!   enumerable possible values occurs in the sentence (case-insensitively);
//! * `overwrite` regexes drop existing `PP`/`NPP`/`NPS` tags before adding
//!   their own.

use fancy_regex::Regex as FancyRegex;
use lt_core::{AnalyzedTokenReadings, CoreError, Result};

use crate::openregex::{compile_logic, ChunkTok, OpenRegex, TokenPredicate, TokenView};

/// `GermanChunker.FILTER_TAGS`.
const FILTER_TAGS: [&str; 3] = ["PP", "NPP", "NPS"];

/// `GermanChunker.SYNTAX_EXPANSION`.
const SYNTAX_EXPANSION: [(&str, &str); 2] = [
    ("<NP>", "<chunk=B-NP> <chunk=I-NP>*"),
    ("&prozent;", "Prozent|Kilo|Kilogramm|Gramm|Euro|Pfund"),
];

/// `GermanChunker.PhraseType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PhraseType {
    /// assigned as `B-NP`/`I-NP` like the OpenNLP chunker
    Np,
    Nps,
    Npp,
    Pp,
}

impl PhraseType {
    fn name(self) -> &'static str {
        match self {
            Self::Np => "NP",
            Self::Nps => "NPS",
            Self::Npp => "NPP",
            Self::Pp => "PP",
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

/// `StringMatcher.create(value, isRegExp, caseSensitive)` for the
/// full-match `matches()` case (the possible-values fast path is only an
/// optimization for the same language).
///
/// `(?i)` stands in for Java's `CASE_INSENSITIVE | UNICODE_CASE`; `\d` is
/// rewritten because Java's `\d` is ASCII-only while the regex crate's is
/// Unicode-aware.
fn java_full_match(pattern: &str, case_sensitive: bool) -> Result<FancyRegex> {
    let translated = ascii_digit_class(pattern);
    let full = if case_sensitive {
        format!("^(?:{translated})$")
    } else {
        format!("(?i)^(?:{translated})$")
    };
    FancyRegex::new(&full)
        .map_err(|e| CoreError::Parse("GermanChunker regex".into(), format!("{pattern}: {e}")))
}

/// Rewrite Java's ASCII-only `\d` into an explicit `[0-9]` / `0-9`.
fn ascii_digit_class(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let chars: Vec<char> = pattern.chars().collect();
    let mut in_class = false;
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            if chars[i + 1] == 'd' {
                out.push_str(if in_class { "0-9" } else { "[0-9]" });
            } else {
                out.push('\\');
                out.push(chars[i + 1]);
            }
            i += 2;
            continue;
        }
        if in_class {
            if c == ']' {
                in_class = false;
            }
        } else if c == '[' {
            in_class = true;
        }
        out.push(c);
        i += 1;
    }
    out
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

/// Java `String.split("=")` with limit 0 (all trailing empty parts removed;
/// the empty string yields one empty part).
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

/// `TokenExpressionFactory.create`: the `<...>` content is a logic expression
/// over `TokenPredicate` arguments (`<und|oder>`, `<pos=ADJ & chunk=B-NP>`).
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
                "GermanChunker".into(),
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
                "GermanChunker".into(),
                format!("expression type not supported: '{other}'"),
            ))
        }
    };
    Ok(Box::new(ChunkPredicate { kind }))
}

// ---------------------------------------------------------------------------
// `simpleFormRegexp` form hints (`calcFormHints` + StringMatcher possible
// values)
// ---------------------------------------------------------------------------

/// The `simpleFormRegexp` char class: `[a-zäöüß|()\[\]?,]`.
fn is_simple_form_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            'ä' | 'ö' | 'ü' | 'ß' | 'Ä' | 'Ö' | 'Ü' | '|' | '(' | ')' | '[' | ']' | '?' | ','
        )
}

/// Java `StringMatcher.getPossibleRegexpValues`, ported including the
/// `TooComplexRegexp` bail-outs (returning `None`).
fn possible_regexp_values(regexp: &str) -> Option<Vec<String>> {
    let mut pattern = regexp;
    if let Some(rest) = pattern.strip_prefix("\\b") {
        pattern = rest;
    }
    if let Some(rest) = pattern.strip_prefix('^') {
        pattern = rest;
    }
    if pattern.ends_with("\\b") && !pattern.ends_with("\\\\b") {
        pattern = &pattern[..pattern.len() - 2];
    }
    if pattern.ends_with('$') && !pattern.ends_with("\\$") {
        pattern = &pattern[..pattern.len() - 1];
    }
    let mut parser = ValueParser {
        chars: pattern.chars().collect(),
        pos: 0,
    };
    parser.disjunction().ok().map(|values| {
        let mut seen = std::collections::HashSet::new();
        values
            .into_iter()
            .filter(|v| seen.insert(v.clone()))
            .collect()
    })
}

struct TooComplex;

fn char_literal(c: Option<char>) -> std::result::Result<String, TooComplex> {
    Ok(c.ok_or(TooComplex)?.to_string())
}

struct ValueParser {
    chars: Vec<char>,
    pos: usize,
}

impl ValueParser {
    fn disjunction(&mut self) -> std::result::Result<Vec<String>, TooComplex> {
        let mut components = vec![self.concatenation()?];
        loop {
            if self.pos >= self.chars.len() || self.chars[self.pos] != '|' {
                if components.len() == 1 {
                    return Ok(components.pop().unwrap());
                }
                let mut out = Vec::new();
                for c in components {
                    out.extend(c);
                }
                return Ok(out);
            }
            self.pos += 1;
            components.push(self.concatenation()?);
        }
    }

    fn concatenation(&mut self) -> std::result::Result<Vec<String>, TooComplex> {
        let mut result = self.postfix()?;
        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            if c == ')' || c == '|' {
                break;
            }
            if "?$^{}*+".contains(c) {
                return Err(TooComplex);
            }
            let right = self.postfix()?;
            let mut product = Vec::with_capacity(result.len() * right.len());
            for l in &result {
                for r in &right {
                    product.push(format!("{l}{r}"));
                }
            }
            result = product;
        }
        Ok(result)
    }

    fn postfix(&mut self) -> std::result::Result<Vec<String>, TooComplex> {
        let group = self.atom()?;
        if self.pos < self.chars.len() {
            let mut next = self.chars[self.pos];
            if next == '{' {
                let Some(offset) = self.chars[self.pos + 1..].iter().position(|&c| c == '}') else {
                    return Err(TooComplex);
                };
                self.pos += offset + 2;
                // upstream `unknown()` for `{n,m}` quantifiers
                return Err(TooComplex);
            }
            if "*+?".contains(next) {
                self.pos += 1;
                if next == '?' {
                    let mut values = group;
                    values.push(String::new());
                    return Ok(values);
                }
                return Err(TooComplex);
            }
            let _ = &mut next;
        }
        Ok(group)
    }

    fn atom(&mut self) -> std::result::Result<Vec<String>, TooComplex> {
        if self.pos >= self.chars.len() {
            return Ok(vec![String::new()]);
        }
        match self.chars[self.pos] {
            '(' => {
                self.pos += 1;
                if self.chars.get(self.pos) == Some(&'?') {
                    self.pos += 1;
                    if self.chars.get(self.pos) != Some(&':') {
                        return Err(TooComplex);
                    }
                    self.pos += 1;
                }
                let group = self.disjunction()?;
                if self.chars.get(self.pos) != Some(&')') {
                    return Err(TooComplex);
                }
                self.pos += 1;
                Ok(group)
            }
            '[' => self.square_bracket_group(),
            '\\' => {
                self.pos += 1;
                let c = self.escape()?;
                Ok(vec![char_literal(c)?])
            }
            '.' => {
                self.pos += 1;
                Err(TooComplex)
            }
            _ => {
                let literal_start = self.pos;
                while self.pos < self.chars.len()
                    && !")|?$^{}*+([\\.".contains(self.chars[self.pos])
                {
                    self.pos += 1;
                }
                if literal_start + 1 < self.pos
                    && self.pos < self.chars.len()
                    && self.chars[self.pos] == '?'
                {
                    self.pos -= 1;
                }
                Ok(vec![self.chars[literal_start..self.pos].iter().collect()])
            }
        }
    }

    fn square_bracket_group(&mut self) -> std::result::Result<Vec<String>, TooComplex> {
        self.pos += 1;
        let start = self.pos;
        let mut options: Option<Vec<Option<char>>> = Some(Vec::new());
        loop {
            let c1 = *self.chars.get(self.pos).ok_or(TooComplex)?;
            self.pos += 1;
            if c1 == ']' {
                break;
            }
            if c1 == '-' && self.pos != start + 1 && self.chars.get(self.pos) != Some(&']') {
                let last = options.as_ref().and_then(|v| v.last().copied().flatten());
                let next = *self.chars.get(self.pos).ok_or(TooComplex)?;
                self.pos += 1;
                if last.is_none() || next == '\\' || (next as i32 - last.unwrap() as i32) > 10 {
                    options = None;
                }
                if let (Some(values), Some(last)) = (options.as_mut(), last) {
                    let mut c = last as u32 + 1;
                    while c <= next as u32 {
                        values.push(char::from_u32(c));
                        c += 1;
                    }
                }
            } else if c1 == '^' {
                options = None;
            } else if c1 == '[' {
                return Err(TooComplex);
            } else {
                let simple = if c1 == '\\' { self.escape()? } else { Some(c1) };
                if let Some(values) = options.as_mut() {
                    values.push(simple);
                }
            }
        }
        let Some(values) = options else {
            return Err(TooComplex);
        };
        if values.is_empty() {
            return Err(TooComplex);
        }
        let mut literals = Vec::with_capacity(values.len());
        for c in values {
            literals.push(char_literal(c)?);
        }
        Ok(literals)
    }

    fn escape(&mut self) -> std::result::Result<Option<char>, TooComplex> {
        let next = *self.chars.get(self.pos).ok_or(TooComplex)?;
        self.pos += 1;
        if "0xucpP".contains(next) {
            return Err(TooComplex);
        }
        if next.is_alphanumeric() {
            return Ok(None);
        }
        Ok(Some(next))
    }
}

/// `GermanChunker.calcFormHints` over the expanded expression.
fn calc_form_hints(expanded: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = expanded.chars().collect();
    let mut hints = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        // `(^| )`: either the start of the string or one space
        if i != 0 && chars[i] != ' ' {
            i += 1;
            continue;
        }
        let mut k = i;
        if chars.get(k) == Some(&' ') {
            k += 1;
        }
        if chars.get(k) != Some(&'<') {
            i += 1;
            continue;
        }
        k += 1;
        let word_start = k;
        while k < chars.len() && is_simple_form_char(chars[k]) {
            k += 1;
        }
        if k == word_start {
            i += 1;
            continue;
        }
        let word: String = chars[word_start..k].iter().collect();
        if chars.get(k) != Some(&'>') {
            i += 1;
            continue;
        }
        k += 1;
        if chars.get(k) == Some(&'+') {
            k += 1;
        }
        let at_end = k == chars.len();
        let followed_by_space = chars.get(k) == Some(&' ');
        if !at_end && !followed_by_space {
            i += 1;
            continue;
        }
        if let Some(values) = possible_regexp_values(&word) {
            hints.push(values);
        }
        // non-overlapping `find()`: continue after the match
        i = if followed_by_space { k + 1 } else { k };
    }
    hints
}

/// Expanded expression + the hint sets computed from it.
struct Spec {
    expr: String,
    phrase_type: PhraseType,
    overwrite: bool,
    form_hints: Vec<Vec<String>>,
}

/// `GermanChunker.build(expr, type, overwrite)`: expand `SYNTAX_EXPANSION`
/// and compute the form hints from the expanded form.
fn build(expr: &str, phrase_type: PhraseType, overwrite: bool) -> Spec {
    let mut expanded = expr.to_string();
    for (from, to) in SYNTAX_EXPANSION {
        expanded = expanded.replace(from, to);
    }
    let form_hints = calc_form_hints(&expanded);
    Spec {
        expr: expanded,
        phrase_type,
        overwrite,
        form_hints,
    }
}

/// `GermanChunker.buildExpanded(expr, type, overwrite, hints)`.
fn build_expanded(
    expr: &str,
    phrase_type: PhraseType,
    overwrite: bool,
    form_hints: Vec<Vec<String>>,
) -> Spec {
    Spec {
        expr: expr.to_string(),
        phrase_type,
        overwrite,
        form_hints,
    }
}

fn und_oder_bzw() -> Vec<Vec<String>> {
    vec![vec!["und".into(), "oder".into(), "bzw".into()]]
}

fn regexes1() -> Vec<Spec> {
    use PhraseType::*;
    vec![
        // "das Auto", "das schöne Auto", "das sehr schöne Auto", "die Pariser Innenstadt":
        build(
            "(<posre=^ART.*>|<pos=PRO>)? <pos=ADV>* <pos=PA2>* <pos=ADJ>* <pos=SUB>+",
            Np,
            false,
        ),
        // "Mythen und Sagen":
        build_expanded(
            "<pos=SUB> (<und|oder>|(<bzw> <.>)) <pos=SUB>",
            Np,
            false,
            und_oder_bzw(),
        ),
        // "ältesten und bekanntesten Maßnahmen":
        build_expanded(
            "<pos=ADJ> (<und|oder>|(<bzw> <.>)) <pos=PA2> <pos=SUB>",
            Np,
            false,
            und_oder_bzw(),
        ),
        // "räumliche und zeitliche Abstände":
        build_expanded(
            "<pos=ADJ> (<und|oder>|(<bzw> <.>)) <pos=ADJ> <pos=SUB>",
            Np,
            false,
            und_oder_bzw(),
        ),
        // "eine leckere Lasagne":
        build(
            "<posre=^ART.*> <pos=ADV>* <pos=ADJ>* <regexCS=[A-ZÖÄÜ][a-zöäü]+>",
            Np,
            false,
        ),
        // "zwei Wochen", "[eines] ihrer drei Autos"
        build("<pos=PRO>? <pos=ZAL> <pos=SUB>", Np, false),
        build("<Herr|Herrn|Frau> <pos=EIG>+", Np, false),
        build("<Herr|Herrn|Frau> <regexCS=[A-ZÖÄÜ][a-zöäü-]+>+", Np, false),
        build("<der>", Np, false),
    ]
}

fn regexes2() -> Vec<Spec> {
    use PhraseType::*;
    let units = "Sekunden|Minuten|Stunden|Tage|Wochen|Monate|Jahre|Jahrzehnte|Jahrhunderte";
    vec![
        // ===== plural and singular noun phrases, based on OpenNLP chunker output =====
        // "In christlichen, islamischen und jüdischen Traditionen":
        build("<pos=ADJ> <,> <chunk=B-NP> <chunk=I-NP>* <und|sowie> <NP>", Npp, false),
        // "ein Hund und eine Katze":
        build(
            "<chunk=B-NP & !regex=jede[rs]?> <chunk=I-NP>* <und|sowie> <pos=ADV>? <NP>",
            Npp,
            false,
        ),
        // "größte und erfolgreichste Erfindung" (fixes mistagging introduced above):
        build(
            "<pos=ADJ> <und|sowie> <chunk=B-NP & !pos=PLU> <chunk=I-NP>*",
            Nps,
            true,
        ),
        // "deren Bestimmung und Funktion" (fixes mistagging introduced above):
        build(
            "<deren> <chunk=B-NP & !pos=PLU> <und|sowie> <chunk=B-NP>*",
            Nps,
            true,
        ),
        // "Julia und Karsten":
        build("<pos=EIG> <und> <pos=EIG>", Npp, false),
        // "die älteste und bekannteste Maßnahme" - OpenNLP won't detect that as one NP:
        build(
            "<pos=ART> <pos=ADJ> <und|sowie> (<pos=ADJ>|<pos=PA2>) <chunk=I-NP & !pos=PLU>+",
            Nps,
            true,
        ),
        // "eine Masseeinheit und keine Gewichtseinheit":
        build(
            "<chunk=B-NP & !pos=PLU> <chunk=I-NP>* <und|sowie> <keine> <chunk=I-NP>+",
            Nps,
            true,
        ),
        // "Der See und das anliegende Marschland":
        build(
            "<NP> <und|sowie> <pos=ART> <pos=PA1> <pos=SUB>",
            Npp,
            true,
        ),
        // "eins ihrer drei Autos":
        build("<eins|eines> <chunk=B-NP> <chunk=I-NP>+", Nps, false),
        // "er und seine Schwester":
        build(
            "<ich|du|er|sie|es|wir|ihr|sie> <und|oder|sowie> <NP>",
            Npp,
            false,
        ),
        // "sowohl sein Vater als auch seine Mutter":
        build("<sowohl> <NP> <als> <auch> <NP>", Npp, false),
        // "sowohl Tom als auch Maria":
        build("<sowohl> <pos=EIG> <als> <auch> <pos=EIG>", Npp, false),
        // "sowohl er als auch seine Schwester":
        build(
            "<sowohl> <ich|du|er|sie|es|wir|ihr|sie> <als> <auch> <NP>",
            Npp,
            false,
        ),
        // "Rekonstruktionen oder der Wiederaufbau", aber nicht "Isolation und ihre Überwindung":
        build(
            "<pos=SUB> <und|oder|sowie> <chunk=B-NP & !ihre> <chunk=I-NP>*",
            Npp,
            false,
        ),
        // "Weder Gerechtigkeit noch Freiheit":
        build("<weder> <pos=SUB> <noch> <pos=SUB>", Npp, false),
        // "drei Katzen" - needed as ZAL cannot be unified, as it has no features:
        build(
            "<zwei|drei|vier|fünf|sechs|sieben|acht|neun|zehn|elf|zwölf> <chunk=I-NP>",
            Npp,
            false,
        ),
        // "der von der Regierung geprüfte Hund ist grün":
        build(
            "<chunk=B-NP> <pos=PRP> <NP> <chunk=B-NP & pos=SIN> <chunk=I-NP>*",
            Nps,
            false,
        ),
        build(
            "<chunk=B-NP> <pos=PRP> <NP> <chunk=B-NP & pos=PLU> <chunk=I-NP>*",
            Npp,
            false,
        ),
        // "der von der Regierung geprüfte Hund":
        build(
            "<chunk=B-NP> <pos=PRP> <NP> <pos=PA2> <chunk=B-NP & !pos=PLU> <chunk=I-NP>*",
            Nps,
            false,
        ),
        build(
            "<chunk=B-NP> <pos=PRP> <NP> <pos=PA2> <chunk=B-NP & !pos=SIN> <chunk=I-NP>*",
            Npp,
            false,
        ),
        // "Herr und Frau Schröder":
        build("<Herr|Frau> <und> <Herr|Frau> <pos=EIG>*", Npp, false),
        // "ein Hund", aber nicht: "[kaum mehr als] vier Prozent":
        build(
            "<chunk=B-NP & !pos=ZAL & !pos=PLU & !chunk=NPP & !einige & !(regex=&prozent;)> <chunk=I-NP & !pos=PLU & !und>*",
            Nps,
            false,
        ),
        // "die Hunde":
        build(
            "<chunk=B-NP & !pos=SIN & !chunk=NPS & !Ellen> <chunk=I-NP & !pos=SIN>*",
            Npp,
            false,
        ),
        // "die hohe Zahl dieser relativ kleinen Verwaltungseinheiten":
        build("<chunk=NPS> <pos=PRO> <pos=ADJ> <pos=ADJ> <NP>", Nps, false),
        // "eine der am meisten verbreiteten Krankheiten":
        build("<regex=eine[rs]?> <der> <am> <pos=ADJ> <pos=PA2> <NP>", Nps, false),
        // "einer der beiden Höfe":
        build("<regex=eine[rs]?> <der> <beiden> <pos=ADJ>* <pos=SUB>", Nps, false),
        // "Einer seiner bedeutendsten Kämpfe":
        build("<regex=eine[rs]?> <seiner|ihrer> <pos=PA1> <pos=SUB>", Nps, false),
        // "xy Prozent" - beide Varianten okay (zumindest umgangssprachlich):
        build("<regex=[\\d,.]+> <&prozent;>", Nps, false),
        build("<regex=[\\d,.]+> <&prozent;>", Npp, false),
        // "[alle Arbeitsplätze so umzugestalten,] dass sie wie ein Spiel":
        build("<dass> <sie> <wie> <NP>", Npp, false),
        // "[so dass Knochenbrüche und] Platzwunden die Regel [sind]"
        build("<pos=PLU> <die> <Regel>", Npp, false),
        // "Veranstaltung, die immer wieder ein kultureller Höhepunkt", aber nicht "... in der Geschichte des Museums, die Sammlung ist seit 2011 zugänglich.":
        build(
            "<chunk=B-NP & pos=SIN> <chunk=I-NP & pos=SIN>* <,> <die> <pos=ADV>+ <chunk=NPS>+",
            Nps,
            false,
        ),
        // "Die Nauheimer Musiktage, die immer wieder ein kultureller Höhepunkt sind":
        build(
            "<chunk=B-NP & pos=PLU> <chunk=I-NP & pos=PLU>* <,> <die> <pos=ADV>+ <chunk=NPS>+",
            Npp,
            false,
        ),
        // ===== genitive phrases and similar ====================================================
        // "Das letzte der teilnehmenden Länder":
        build("<der|die|das> <pos=ADJ> <der> <pos=PA1> <pos=SUB>", Nps, false),
        // "Ursachen der vorliegenden Durchblutungsstörung":
        build("<pos=SUB & pos=PLU> <der> <pos=PA1> <pos=SUB>", Npp, false),
        // "die ältere der beiden Töchter":
        build("<der|die|das> <pos=ADJ> <der> <pos=PRO>? <pos=SUB>", Nps, false),
        // "Synthese organischer Verbindungen", "die Anordnung der vier Achsen", aber nicht "Einige der Inhaltsstoffe":
        build("<chunk=NPS & !einige> <chunk=NPP & (pos=GEN |pos=ZAL)>+", Nps, true),
        // "die Kenntnisse der Sprache":
        build("<chunk=NPP> <chunk=NPS & pos=GEN>+", Npp, true),
        // "die Pyramide des Friedens und der Eintracht":
        build(
            "<chunk=NPS>+ <und> <chunk=NP[SP] & (pos=GEN | pos=ADV)>+",
            Nps,
            true,
        ),
        // "Teil der dort ausgestellten Bestände":
        build(
            "<chunk=NPS>+ <der> <pos=ADV> <pos=PA2> <chunk=I-NP>",
            Nps,
            true,
        ),
        // "Autor der ersten beiden Bücher":
        build("<chunk=NPS>+ <der> (<pos=ADJ>|<pos=ZAL>) <NP>", Nps, true),
        // "Autor der beiden Bücher":
        build("<chunk=NPS>+ <der> <NP>", Nps, true),
        // "Teil der umfangreichen dort ausgestellten Bestände":
        build(
            "<chunk=NPS>+ <der> <pos=ADJ> <pos=ADV> <pos=PA2> <NP>",
            Nps,
            true,
        ),
        // "die Krankheit unserer heutigen Städte und Siedlungen":
        build("<chunk=NPS>+ <pos=PRO:POS> <pos=ADJ> <NP>", Nps, true),
        // "der letzte der vier großen Flüsse":
        build("<der|das> <pos=ADJ> <der> <pos=ZAL> <NP>", Nps, true),
        // "eine Menge englischer Wörter [sind aus dem Lateinischen abgeleitet]":
        build("<eine> <menge> <NP>+", Npp, true),
        // "dass [sie und sein Sohn ein Paar] sind":
        build("<er|sie|es> <und> <NP> <NP>", Npp, false),
        // ===== prepositional phrases ===========================================================
        // "laut den meisten Quellen":
        build("<laut> <regex=.*>{0,3} <Quellen>", Pp, true),
        // "bei den sehr niedrigen Oberflächentemperaturen" (OpenNLP doesn't find this)
        build("<pos=PRP> <pos=ART:> <pos=ADV>* <pos=ADJ> <NP>", Pp, true),
        // "in den alten Religionen, Mythen und Sagen":
        build("<pos=PRP> <chunk=NPP>+ <,> <NP>", Pp, true),
        // "für die Stadtteile und selbständigen Ortsteile":
        build("<pos=PRP> <chunk=NPP>+", Pp, true),
        // "Das Bündnis zwischen der Sowjetunion und Kuba":
        build("<pos=PRP> <der> <chunk=NPP>+", Pp, false),
        // "in chemischen Komplexverbindungen", "für die Fische":
        build("<pos=PRP> <NP>", Pp, false),
        // "einschließlich der biologischen und sozialen Grundlagen":
        build("<pos=PRP> <NP> <pos=ADJ> <und|oder|bzw.> <NP>", Pp, false),
        // "für Ärzte und Ärztinnen festgestellte Risikoprofil", "der als Befestigung gedachte östliche Teil der Burg":
        build("<pos=PRP> (<NP>)+", Pp, false),
        // "in den darauf folgenden Wochen":
        build("<pos=PRP> <chunk=B-NP> <pos=ADV> <NP>", Pp, false),
        // "in nur zwei Wochen":
        build("<pos=PRP> <pos=ADV> <pos=ZAL> <chunk=B-NP>", Pp, false),
        // "in deren deutschen Installationen":
        build("<pos=PRP> <pos=PRO> <NP>", Pp, false),
        // "nach sachlichen und militärischen Kriterien" - we need to help OpenNLP a bit with this one:
        build("<pos=PRP> <pos=ADJ> <und|oder|sowie> <NP>", Pp, false),
        // "mit über 1000 Handschriften":
        build("<pos=PRP> <pos=ADV> <regex=\\d+> <NP>", Pp, false),
        // "über laufende Sanierungsmaßnahmen":
        build("<pos=PRP> <pos=PA1> <NP>", Pp, false),
        // "Aufgrund stark schwankender Absatzmärkte war die GEFA-Flug..."
        build("<pos=PRP> <pos=ADJ> <pos=PA1> <NP>", Pp, false),
        // "durch Einsatz größerer Maschinen und bessere Kapazitätsplanung":
        build("<pos=PRP> <NP> <NP> <und|oder> <NP>", Pp, false),
        // "bei sehr guten Beobachtungsbedingungen":
        build("<pos=PRP> <pos=ADV> <pos=ADJ> <NP>", Pp, false),
        // "[Von ursprünglich drei Almhütten] ist noch eine erhalten":
        build("<pos=PRP> <pos=ADJ:PRD:GRU> <pos=ZAL> <NP>", Pp, false),
        // "die darauffolgenden Jahre" -> eigentlich "in den darauffolgenden Jahren":
        build(&format!("<die> <pos=ADJ> <{units}> (<NP>)?"), Pp, false),
        // "die letzten zwei Monate" -> eigentlich "in den letzten zwei Monaten":
        build(
            &format!("<die> <pos=ADJ> <pos=ZAL> <{units}> (<NP>)?"),
            Pp,
            false,
        ),
        // "letztes Jahr":
        build(
            "<regex=(vor)?letzte[sn]?> <Woche|Monat|Jahr|Jahrzehnt|Jahrhundert>",
            Pp,
            false,
        ),
        // "Für in Österreich lebende Afrikaner und Afrikanerinnen":
        build(
            "<für> <in> <pos=EIG> <pos=PA1> <pos=SUB> <und> <pos=SUB>",
            Pp,
            true,
        ),
        // "die Beziehungen zwischen Kanada und dem Iran":
        build("<chunk=NPP> <zwischen> <pos=EIG> <und|sowie> <NP>", Npp, false),
        // ", die die hauptsächliche Beute der Eisbären", ", welche der Urstoff aller Körper":
        build("<,> <die|welche> <NP> <chunk=NPS & pos=GEN>+", Npp, false),
        // "Kommentare, Korrekturen, Kritik":
        build("<NP> <,> <NP> <,> <NP>", Npp, false),
        // "Details, Dialoge, wie auch die Typologie der Charaktere":
        build("<NP> <,> <NP> <,> <wie> <auch> <chunk=NPS>+", Npp, false),
    ]
}

struct CompiledChunkRegex {
    regex: OpenRegex,
    phrase_type: PhraseType,
    overwrite: bool,
    form_hints: Vec<Vec<String>>,
}

impl CompiledChunkRegex {
    fn compile(spec: &Spec) -> Result<Self> {
        let regex = OpenRegex::compile(&spec.expr, &mut |desc| compile_chunk_expression(desc))?;
        Ok(Self {
            regex,
            phrase_type: spec.phrase_type,
            overwrite: spec.overwrite,
            form_hints: spec.form_hints.clone(),
        })
    }
}

/// `org.languagetool.chunking.GermanChunker`.
pub struct GermanChunker {
    regexes1: Vec<CompiledChunkRegex>,
    regexes2: Vec<CompiledChunkRegex>,
}

impl GermanChunker {
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

    /// `GermanChunker.addChunkTags`.
    pub fn add_chunk_tags(&self, tokens: &mut [AnalyzedTokenReadings]) {
        // `allForms`: case-insensitive set of all token surfaces (the Java
        // TreeSet is case-insensitive; whitespace/SENT_START are included)
        let all_forms: std::collections::HashSet<String> =
            tokens.iter().map(|t| t.surface().to_lowercase()).collect();
        let mut chunk_tokens: Vec<ChunkTok> = Vec::new();
        let mut token_indices: Vec<usize> = Vec::new();
        for (i, readings) in tokens.iter().enumerate() {
            let surface = readings.surface();
            // Java skips `isWhitespace()` tokens; `StringTools.isWhitespace("")`
            // is true, which covers the SENT_START entry
            if readings.is_whitespace || surface.is_empty() || readings.is_sentence_start {
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
            self.apply(regex, &mut chunk_tokens, tokens, &all_forms);
        }
        for regex in &self.regexes2 {
            self.apply(regex, &mut chunk_tokens, tokens, &all_forms);
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
        all_forms: &std::collections::HashSet<String>,
    ) {
        if !has_all_form_hints(regex, all_forms) {
            return;
        }
        let matches = regex.regex.find_all(chunk_tokens, tokens);
        for (start, end) in matches {
            for (offset, token) in chunk_tokens[start..end].iter_mut().enumerate() {
                let i = start + offset;
                let mut new_tags = token.chunk_tags.clone();
                if regex.overwrite {
                    new_tags.retain(|tag| !FILTER_TAGS.contains(&tag.as_str()));
                }
                let new_tag = if regex.phrase_type == PhraseType::Np {
                    if i == start {
                        "B-NP"
                    } else {
                        "I-NP"
                    }
                } else {
                    regex.phrase_type.name()
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

fn has_all_form_hints(
    regex: &CompiledChunkRegex,
    all_forms: &std::collections::HashSet<String>,
) -> bool {
    regex.form_hints.iter().all(|hints| {
        hints
            .iter()
            .any(|hint| all_forms.contains(&hint.to_lowercase()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn possible_values_matches_java_examples() {
        assert_eq!(
            possible_regexp_values("Herr|Herrn|Frau").map(sorted),
            Some(sorted(vec!["Herr".into(), "Herrn".into(), "Frau".into()]))
        );
        assert_eq!(
            possible_regexp_values("und|oder").map(sorted),
            Some(sorted(vec!["und".into(), "oder".into()]))
        );
        assert_eq!(
            possible_regexp_values(",").map(sorted),
            Some(vec![",".into()])
        );
        assert_eq!(
            possible_regexp_values("jede[rs]?").map(sorted),
            Some(sorted(vec!["jede".into(), "jeder".into(), "jedes".into()]))
        );
        assert_eq!(possible_regexp_values(".*"), None);
        assert_eq!(
            possible_regexp_values("eine[rs]?").map(sorted),
            Some(sorted(vec!["eine".into(), "einer".into(), "eines".into()]))
        );
        assert_eq!(
            possible_regexp_values("[abc]").map(sorted),
            Some(sorted(vec!["a".into(), "b".into(), "c".into()]))
        );
        assert_eq!(possible_regexp_values("[a-z]"), None);
        assert_eq!(
            possible_regexp_values("(vor)?letzte[sn]?").map(sorted),
            Some(sorted(vec![
                "letzte".into(),
                "letztes".into(),
                "letzten".into(),
                "vorletzte".into(),
                "vorletzten".into(),
                "vorletztes".into(),
            ]))
        );
    }

    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    #[test]
    fn form_hints_extraction() {
        let hints = calc_form_hints("<Herr|Herrn|Frau> <pos=EIG>+");
        assert_eq!(hints.len(), 1);
        assert!(hints[0].contains(&"Herr".to_string()));
        let hints = calc_form_hints("<Herr|Herrn|Frau> <regexCS=[A-ZÖÄÜ][a-zöäü-]+>+");
        assert_eq!(hints.len(), 1);
        let hints = calc_form_hints("<chunk=B-NP> <chunk=I-NP>*");
        assert!(hints.is_empty());
    }
}
