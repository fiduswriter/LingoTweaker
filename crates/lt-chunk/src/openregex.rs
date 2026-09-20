//! Clean-room Rust port of the OpenRegex subset used by the German chunker.
//!
//! Upstream: `edu.washington.cs.knowitall:openregex:1.1.1`
//! (<https://github.com/knowitall/openregex>, LGPL-2.1-or-later), classes
//! `edu.washington.cs.knowitall.regex.{RegularExpression, RegularExpressionParser,
//! Expression, FiniteAutomaton}` and `edu.washington.cs.knowitall.logic.
//! {LogicExpression, LogicExpressionParser, Expression}`.
//!
//! Ported: the token/group/alternation/quantifier/assertion grammar, the
//! Thompson-NFA `lookingAt`/`find`/`findAll` semantics (the longest match at
//! the earliest start position, non-overlapping results), `minMatchingLength`
//! including its quirks (`Plus` == 1, `MinMax` == min occurrences) and the
//! logic expression grammar (`&`, `|`, `!`, parentheses and quoted literals).
//!
//! Not ported: `Match` objects, `buildMatch`/group extraction and named groups
//! beyond parsing (`GermanChunker` only reads `Match.startIndex()`/`endIndex()`).

use lt_core::{AnalyzedTokenReadings, CoreError, Result};

/// What a compiled predicate sees of one element (`ChunkTaggedToken`).
pub struct TokenView<'a> {
    /// `ChunkTaggedToken.getToken()` — the surface of the underlying readings
    pub token: &'a str,
    pub chunk_tags: &'a [String],
    /// `ChunkTaggedToken.getReadings()`, `None` for unmapped tokens
    pub readings: Option<&'a AnalyzedTokenReadings>,
}

/// `org.languagetool.chunking.TokenPredicate` (its `predicate` field).
pub trait TokenPredicate: Send + Sync {
    fn apply(&self, token: &TokenView<'_>) -> bool;
}

/// The chunker's working token (`ChunkTaggedToken`); `readings` indexes the
/// sentence's token list.
pub struct ChunkTok {
    pub token: String,
    pub chunk_tags: Vec<String>,
    pub readings: Option<usize>,
}

/// A compiled logic expression (the inside of one `<...>` token).
pub enum LogicExpr {
    /// `LogicExpression` with an empty expression list is true for everything.
    AlwaysTrue,
    Not(Box<LogicExpr>),
    And(Box<LogicExpr>, Box<LogicExpr>),
    Or(Box<LogicExpr>, Box<LogicExpr>),
    Pred(Box<dyn TokenPredicate>),
}

impl TokenPredicate for LogicExpr {
    fn apply(&self, token: &TokenView<'_>) -> bool {
        LogicExpr::apply(self, token)
    }
}

impl LogicExpr {
    pub fn apply(&self, view: &TokenView<'_>) -> bool {
        match self {
            Self::AlwaysTrue => true,
            Self::Not(e) => !e.apply(view),
            Self::And(a, b) => a.apply(view) && b.apply(view),
            Self::Or(a, b) => a.apply(view) || b.apply(view),
            Self::Pred(p) => p.apply(view),
        }
    }
}

/// Compile `input` (`LogicExpression.compile`) with `factory` creating the
/// `Arg.Pred` leaves from each `type=value` description.
pub fn compile_logic(
    input: &str,
    factory: &mut dyn FnMut(&str) -> Result<Box<dyn TokenPredicate>>,
) -> Result<LogicExpr> {
    let tokens = logic_tokenize(input)?;
    if tokens.is_empty() {
        return Ok(LogicExpr::AlwaysTrue);
    }
    let mut parser = LogicParser {
        tokens: &tokens,
        pos: 0,
        factory,
    };
    let expr = parser.parse_or()?;
    if parser.pos != tokens.len() {
        return Err(logic_err("trailing tokens"));
    }
    Ok(expr)
}

enum LogicTok {
    LParen,
    RParen,
    Not,
    And,
    Or,
    Arg(String),
}

fn logic_err(msg: &str) -> CoreError {
    CoreError::Parse("OpenRegex logic expression".into(), msg.to_string())
}

/// Java `String.trim()`: strips all chars `<= ' '` from both ends.
fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

fn logic_tokenize(input: &str) -> Result<Vec<LogicTok>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < input.len() {
        let c = input[i..].chars().next().unwrap();
        match c {
            ' ' => {
                i += 1;
            }
            '(' => {
                out.push(LogicTok::LParen);
                i += 1;
            }
            ')' => {
                out.push(LogicTok::RParen);
                i += 1;
            }
            '!' => {
                out.push(LogicTok::Not);
                i += 1;
            }
            '&' => {
                out.push(LogicTok::And);
                i += 1;
            }
            '|' => {
                out.push(LogicTok::Or);
                i += 1;
            }
            _ => {
                let token = logic_read_token(&input[i..])?;
                out.push(LogicTok::Arg(token.to_string()));
                i += token.len();
            }
        }
    }
    Ok(out)
}

/// `LogicExpressionParser.readToken`: scan to the first unbalanced `)` or
/// `&`/`|`, skipping quoted literals, then trim.
fn logic_read_token(remainder: &str) -> Result<&str> {
    let chars: Vec<(usize, char)> = remainder.char_indices().collect();
    let mut parens = 0i32;
    let mut end = remainder.len();
    let mut k = 0usize;
    while k < chars.len() {
        let (byte_idx, c) = chars[k];
        if let Some(len) = quoted_literal_len(&remainder[byte_idx..]) {
            k += len;
            continue;
        }
        if c == '(' {
            parens += 1;
        } else if c == ')' {
            if parens == 0 {
                end = byte_idx;
                break;
            }
            parens -= 1;
        } else if c == '&' || c == '|' {
            end = byte_idx;
            break;
        }
        k += 1;
    }
    let token = java_trim(&remainder[..end]);
    if token.is_empty() {
        return Err(logic_err("zero-length token found"));
    }
    Ok(token)
}

/// Length in bytes of a quoted literal (`"..."`, `'...'`, `/.../`) starting
/// at `s`, or `None` when `s` does not start with one.
fn quoted_literal_len(s: &str) -> Option<usize> {
    let mut chars = s.char_indices();
    let (_, open) = chars.next()?;
    match open {
        '\'' => {
            let rest = &s[1..];
            rest.find('\'').map(|i| i + 2)
        }
        '"' => {
            let mut escaped = false;
            for (i, c) in s.char_indices().skip(1) {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    return Some(i + 1);
                }
            }
            None
        }
        '/' => {
            let mut escaped = false;
            for (i, c) in s.char_indices().skip(1) {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '/' {
                    return Some(i + 1);
                }
            }
            None
        }
        _ => None,
    }
}

struct LogicParser<'a, 'f> {
    tokens: &'a [LogicTok],
    pos: usize,
    factory: &'f mut dyn FnMut(&str) -> Result<Box<dyn TokenPredicate>>,
}

impl LogicParser<'_, '_> {
    /// `!` binds tighter than `&`, which binds tighter than `|` — the same
    /// result as the upstream shunting-yard (`precedence` 0/1/2, strict
    /// `preceeds`), including left associativity of `&`/`|`.
    fn parse_or(&mut self) -> Result<LogicExpr> {
        let mut left = self.parse_and()?;
        while matches!(self.tokens.get(self.pos), Some(LogicTok::Or)) {
            self.pos += 1;
            let right = self.parse_and()?;
            left = LogicExpr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<LogicExpr> {
        let mut left = self.parse_unary()?;
        while matches!(self.tokens.get(self.pos), Some(LogicTok::And)) {
            self.pos += 1;
            let right = self.parse_unary()?;
            left = LogicExpr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<LogicExpr> {
        if matches!(self.tokens.get(self.pos), Some(LogicTok::Not)) {
            self.pos += 1;
            let inner = self.parse_unary()?;
            return Ok(LogicExpr::Not(Box::new(inner)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<LogicExpr> {
        match self.tokens.get(self.pos) {
            Some(LogicTok::LParen) => {
                self.pos += 1;
                let inner = self.parse_or()?;
                match self.tokens.get(self.pos) {
                    Some(LogicTok::RParen) => {
                        self.pos += 1;
                        Ok(inner)
                    }
                    _ => Err(logic_err("unbalanced parentheses")),
                }
            }
            Some(LogicTok::Arg(desc)) => {
                let desc = desc.clone();
                self.pos += 1;
                Ok(LogicExpr::Pred((self.factory)(&desc)?))
            }
            _ => Err(logic_err("expected argument")),
        }
    }
}

/// A regular expression component (upstream `Expression`).
enum RExpr {
    Pred(usize),
    /// `MatchingGroup` of several sub-expressions
    Group(Vec<RExpr>),
    Or(Box<RExpr>, Box<RExpr>),
    Star(Box<RExpr>),
    Plus(Box<RExpr>),
    Option(Box<RExpr>),
    MinMax(Box<RExpr>, usize, usize),
    StartAssertion,
    EndAssertion,
}

/// `Expression.*.minMatchingLength()` — reproduces the upstream quirks
/// (`Plus` is 1, `MinMax` is the minimum count, not scaled by the sub-expr).
fn min_matching_length(expr: &RExpr) -> usize {
    match expr {
        RExpr::Pred(_) => 1,
        RExpr::Group(exprs) => exprs.iter().map(min_matching_length).sum(),
        RExpr::Or(a, b) => min_matching_length(a).min(min_matching_length(b)),
        RExpr::Star(_) => 0,
        RExpr::Plus(_) => 1,
        RExpr::Option(_) => 0,
        RExpr::MinMax(_, min, _) => *min,
        RExpr::StartAssertion | RExpr::EndAssertion => 0,
    }
}

#[derive(Default)]
struct Node {
    epsilons: Vec<usize>,
    /// (dest, predicate index) — `FiniteAutomaton.Edge`
    edges: Vec<(usize, usize)>,
    /// (dest, 0 = start assertion, 1 = end assertion)
    assertions: Vec<(usize, u8)>,
}

struct Builder {
    nodes: Vec<Node>,
}

impl Builder {
    fn new_node(&mut self) -> usize {
        self.nodes.push(Node::default());
        self.nodes.len() - 1
    }

    fn build(&mut self, expr: &RExpr) -> (usize, usize) {
        match expr {
            RExpr::Pred(idx) => {
                let start = self.new_node();
                let end = self.new_node();
                self.nodes[start].edges.push((end, *idx));
                (start, end)
            }
            RExpr::Group(exprs) => {
                let start = self.new_node();
                let end = self.new_node();
                let mut prev = start;
                for e in exprs {
                    let (s, t) = self.build(e);
                    self.nodes[prev].epsilons.push(s);
                    prev = t;
                }
                self.nodes[prev].epsilons.push(end);
                (start, end)
            }
            RExpr::Or(a, b) => {
                let start = self.new_node();
                let end = self.new_node();
                let (s1, e1) = self.build(a);
                let (s2, e2) = self.build(b);
                self.nodes[start].epsilons.push(s1);
                self.nodes[start].epsilons.push(s2);
                self.nodes[e1].epsilons.push(end);
                self.nodes[e2].epsilons.push(end);
                (start, end)
            }
            RExpr::Star(inner) => {
                let start = self.new_node();
                let end = self.new_node();
                let (s, e) = self.build(inner);
                self.nodes[e].epsilons.push(s);
                self.nodes[start].epsilons.push(s);
                self.nodes[e].epsilons.push(end);
                self.nodes[start].epsilons.push(end);
                (start, end)
            }
            RExpr::Plus(inner) => {
                let start = self.new_node();
                let end = self.new_node();
                let (s, e) = self.build(inner);
                self.nodes[e].epsilons.push(s);
                self.nodes[start].epsilons.push(s);
                self.nodes[e].epsilons.push(end);
                (start, end)
            }
            RExpr::Option(inner) => {
                let start = self.new_node();
                let end = self.new_node();
                let (s, e) = self.build(inner);
                self.nodes[start].epsilons.push(s);
                self.nodes[e].epsilons.push(end);
                self.nodes[start].epsilons.push(end);
                (start, end)
            }
            RExpr::MinMax(inner, min, max) => {
                let start = self.new_node();
                let end = self.new_node();
                let mut subs = Vec::with_capacity(*max);
                for _ in 0..*max {
                    subs.push(self.build(inner));
                }
                self.nodes[start].epsilons.push(subs[0].0);
                for (i, (_, e)) in subs.iter().enumerate() {
                    if i >= min.saturating_sub(1) {
                        self.nodes[*e].epsilons.push(end);
                    }
                    if i < subs.len() - 1 {
                        self.nodes[*e].epsilons.push(subs[i + 1].0);
                    }
                }
                if *min == 0 {
                    self.nodes[start].epsilons.push(end);
                }
                (start, end)
            }
            RExpr::StartAssertion => {
                let start = self.new_node();
                let end = self.new_node();
                self.nodes[start].assertions.push((end, 0));
                (start, end)
            }
            RExpr::EndAssertion => {
                let start = self.new_node();
                let end = self.new_node();
                self.nodes[start].assertions.push((end, 1));
                (start, end)
            }
        }
    }
}

/// `edu.washington.cs.knowitall.regex.RegularExpression`.
pub struct OpenRegex {
    preds: Vec<Box<dyn TokenPredicate>>,
    nodes: Vec<Node>,
    start: usize,
    end: usize,
    min_len: usize,
}

impl OpenRegex {
    /// `RegularExpression.compile(expression, factory)`.
    pub fn compile(
        expression: &str,
        factory: &mut dyn FnMut(&str) -> Result<Box<dyn TokenPredicate>>,
    ) -> Result<Self> {
        let mut parser = RegexParser {
            src: expression,
            pos: 0,
            preds: Vec::new(),
            factory,
            or_pending: false,
        };
        let exprs = parser.tokenize()?;
        if parser.or_pending {
            return Err(regex_err("OR remains on the stack"));
        }
        let root = RExpr::Group(exprs);
        let min_len = min_matching_length(&root);
        let mut builder = Builder { nodes: Vec::new() };
        let (start, end) = builder.build(&root);
        Ok(Self {
            preds: parser.preds,
            nodes: builder.nodes,
            start,
            end,
            min_len,
        })
    }

    pub fn min_matching_length(&self) -> usize {
        self.min_len
    }

    /// `RegularExpression.findAll` — non-overlapping matches in order.
    pub fn find_all(
        &self,
        tokens: &[ChunkTok],
        readings: &[AnalyzedTokenReadings],
    ) -> Vec<(usize, usize)> {
        let mut results = Vec::new();
        let mut start = 0usize;
        while let Some((s, e)) = self.find(tokens, readings, start) {
            start = e;
            results.push((s, e));
        }
        results
    }

    /// `RegularExpression.find(tokens, start)`: the first start index with a
    /// match, returned as `(startIndex, endIndex)`.
    fn find(
        &self,
        tokens: &[ChunkTok],
        readings: &[AnalyzedTokenReadings],
        start: usize,
    ) -> Option<(usize, usize)> {
        let mut i = start;
        while i + self.min_len <= tokens.len() {
            if let Some(end) = self.looking_at(tokens, readings, i) {
                return Some((i, end));
            }
            i += 1;
        }
        None
    }

    /// `FutureAutomaton.lookingAt(tokens, startIndex)` reduced to the match
    /// span: the longest match starting at `startIndex`, `None` if no match
    /// consumes at least one token.
    fn looking_at(
        &self,
        tokens: &[ChunkTok],
        readings: &[AnalyzedTokenReadings],
        start: usize,
    ) -> Option<usize> {
        if tokens.len() < start + self.min_len {
            return None;
        }
        let mut states: Vec<usize> = vec![self.start];
        let mut solution: Option<usize> = None;
        let mut pos = start;
        loop {
            // upstream alternates `expandEpsilons`/`expandAssertions` until
            // no new states appear (assertions can be followed by epsilons)
            loop {
                let before = states.len();
                self.close_epsilons(&mut states);
                self.close_assertions(&mut states, tokens.len(), pos, start);
                if states.len() == before {
                    break;
                }
            }
            if pos > start && states.contains(&self.end) {
                // later positions overwrite: the upstream `evaluate` keeps the
                // solution with the fewest remaining tokens (longest match)
                solution = Some(pos);
            }
            if pos >= tokens.len() {
                break;
            }
            let mut next: Vec<usize> = Vec::new();
            let view = self.view(&tokens[pos], readings);
            for &state in &states {
                for &(dest, pred) in &self.nodes[state].edges {
                    if self.preds[pred].apply(&view) && !next.contains(&dest) {
                        next.push(dest);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            states = next;
            pos += 1;
        }
        solution
    }

    fn view<'a>(&self, tok: &'a ChunkTok, readings: &'a [AnalyzedTokenReadings]) -> TokenView<'a> {
        TokenView {
            token: &tok.token,
            chunk_tags: &tok.chunk_tags,
            readings: tok.readings.and_then(|i| readings.get(i)),
        }
    }

    fn close_epsilons(&self, states: &mut Vec<usize>) {
        let mut i = 0usize;
        while i < states.len() {
            let state = states[i];
            for &dest in &self.nodes[state].epsilons {
                if !states.contains(&dest) {
                    states.push(dest);
                }
            }
            i += 1;
        }
    }

    /// `expandAssertions`: `^` holds only at the very start of the whole
    /// token list, `$` only when every token has been consumed.
    fn close_assertions(&self, states: &mut Vec<usize>, total: usize, pos: usize, start: usize) {
        let mut i = 0usize;
        while i < states.len() {
            let state = states[i];
            let mut fired = Vec::new();
            for &(dest, kind) in &self.nodes[state].assertions {
                let holds = if kind == 0 {
                    start == 0 && pos == start
                } else {
                    pos == total
                };
                if holds && !states.contains(&dest) {
                    fired.push(dest);
                }
            }
            states.extend(fired);
            i += 1;
        }
    }
}

fn regex_err(msg: &str) -> CoreError {
    CoreError::Parse("OpenRegex expression".into(), msg.to_string())
}

struct RegexParser<'a, 'f> {
    src: &'a str,
    pos: usize,
    preds: Vec<Box<dyn TokenPredicate>>,
    factory: &'f mut dyn FnMut(&str) -> Result<Box<dyn TokenPredicate>>,
    or_pending: bool,
}

impl RegexParser<'_, '_> {
    fn tokenize(&mut self) -> Result<Vec<RExpr>> {
        let mut exprs: Vec<RExpr> = Vec::new();
        while self.pos < self.src.len() {
            // skip `\s+` (Java's ASCII whitespace class)
            while self.pos < self.src.len() {
                let c = self.src[self.pos..].chars().next().unwrap();
                if matches!(c, ' ' | '\t' | '\n' | '\x0B' | '\x0C' | '\r') {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if self.pos >= self.src.len() {
                break;
            }
            let c = self.src[self.pos..].chars().next().unwrap();
            if matches!(c, '(' | '<' | '[' | '$' | '^') {
                if c == '(' {
                    let end = index_of_close(self.src, self.pos, '(', ')')?;
                    let group = &self.src[self.pos + 1..end];
                    self.pos = end + 1;
                    let inner = self.tokenize_group(group)?;
                    exprs.push(RExpr::Group(inner));
                } else if c == '<' || c == '[' {
                    let token = read_token(&self.src[self.pos..])?;
                    let inside = &token[1..token.len() - 1];
                    let pred = (self.factory)(inside)?;
                    self.preds.push(pred);
                    exprs.push(RExpr::Pred(self.preds.len() - 1));
                    self.pos += token.len();
                } else if c == '^' {
                    exprs.push(RExpr::StartAssertion);
                    self.pos += 1;
                } else {
                    exprs.push(RExpr::EndAssertion);
                    self.pos += 1;
                }
                if self.or_pending {
                    self.or_pending = false;
                    if exprs.len() < 2 {
                        return Err(regex_err("OR operator is applied to fewer than 2 elements"));
                    }
                    let right = exprs.pop().unwrap();
                    let left = exprs.pop().unwrap();
                    exprs.push(RExpr::Or(Box::new(left), Box::new(right)));
                }
            } else if matches!(c, '*' | '?' | '+') {
                let base = exprs
                    .pop()
                    .ok_or_else(|| regex_err("no expression for unary operator"))?;
                exprs.push(match c {
                    '?' => RExpr::Option(Box::new(base)),
                    '*' => RExpr::Star(Box::new(base)),
                    _ => RExpr::Plus(Box::new(base)),
                });
                self.pos += 1;
            } else if c == '{' {
                let rest = &self.src[self.pos..];
                let Some((min, max, len)) = parse_min_max(rest) else {
                    return Err(regex_err(&format!("unknown symbol: {rest}")));
                };
                let base = exprs
                    .pop()
                    .ok_or_else(|| regex_err("no expression for {n,m}"))?;
                if min > max {
                    return Err(regex_err("minOccurrences must be <= maxOccurrences"));
                }
                exprs.push(RExpr::MinMax(Box::new(base), min, max));
                self.pos += len;
            } else if c == '|' {
                self.or_pending = true;
                self.pos += 1;
            } else {
                return Err(regex_err(&format!(
                    "unknown symbol: {}",
                    &self.src[self.pos..]
                )));
            }
        }
        Ok(exprs)
    }

    fn tokenize_group(&mut self, group: &str) -> Result<Vec<RExpr>> {
        // `(<name>:...)` and `(?:...)` are parsed but share `MatchingGroup`
        // build semantics upstream; only the outer group is needed here.
        let inner = if let Some(rest) = group.strip_prefix('?') {
            if let Some(rest) = rest.strip_prefix(':') {
                rest
            } else {
                return Err(regex_err("unsupported group syntax"));
            }
        } else if group.starts_with('<') {
            match group.find(">:") {
                Some(close) => &group[close + 2..],
                None => group,
            }
        } else {
            group
        };
        let mut sub = RegexParser {
            src: inner,
            pos: 0,
            preds: std::mem::take(&mut self.preds),
            factory: &mut *self.factory,
            or_pending: false,
        };
        let exprs = sub.tokenize()?;
        if sub.or_pending {
            return Err(regex_err("OR remains on the stack"));
        }
        self.preds = sub.preds;
        Ok(exprs)
    }
}

fn index_of_close(src: &str, start: usize, open: char, close: char) -> Result<usize> {
    let mut depth = 0i32;
    for (i, c) in src[start..].char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Ok(start + i);
            }
        }
    }
    Err(regex_err("bad token. Non-matching brackets (<> or [])"))
}

fn read_token(remaining: &str) -> Result<String> {
    let c = remaining
        .chars()
        .next()
        .ok_or_else(|| regex_err("empty token"))?;
    let close = match c {
        '<' => '>',
        '[' => ']',
        _ => return Err(regex_err("token must start with '<' or '['")),
    };
    let end = index_of_close(remaining, 0, c, close)?;
    Ok(remaining[..end + 1].to_string())
}

/// `\{(\d+),(\d+)\}` at the start of `s`; returns (min, max, byte length).
fn parse_min_max(s: &str) -> Option<(usize, usize, usize)> {
    let rest = s.strip_prefix('{')?;
    let (min_s, rest) = rest.split_once(',')?;
    let (max_s, _) = rest.split_once('}')?;
    if min_s.is_empty() || max_s.is_empty() {
        return None;
    }
    if !min_s.bytes().all(|b| b.is_ascii_digit()) || !max_s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let min: usize = min_s.parse().ok()?;
    let max: usize = max_s.parse().ok()?;
    Some((min, max, 1 + min_s.len() + 1 + max_s.len() + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::{AnalyzedToken, AnalyzedTokenReadings};

    struct IsToken(String);
    impl TokenPredicate for IsToken {
        fn apply(&self, token: &TokenView<'_>) -> bool {
            token.token == self.0
        }
    }

    fn compile(expr: &str) -> OpenRegex {
        OpenRegex::compile(expr, &mut |desc| Ok(Box::new(IsToken(desc.to_string())))).unwrap()
    }

    fn toks(words: &[&str]) -> Vec<ChunkTok> {
        words
            .iter()
            .map(|w| ChunkTok {
                token: w.to_string(),
                chunk_tags: vec!["O".to_string()],
                readings: None,
            })
            .collect()
    }

    fn spans(expr: &str, words: &[&str]) -> Vec<(usize, usize)> {
        compile(expr).find_all(&toks(words), &[])
    }

    #[test]
    fn single_token_and_sequence() {
        assert_eq!(spans("<a> <b>", &["a", "b", "c"]), vec![(0, 2)]);
        assert_eq!(spans("<a> <b>", &["x", "a", "b"]), vec![(1, 3)]);
        assert_eq!(spans("<a>", &["x", "a"]), vec![(1, 2)]);
    }

    #[test]
    fn longest_match_wins() {
        // <a>+ from index 0 consumes all three
        assert_eq!(spans("<a>+", &["a", "a", "a"]), vec![(0, 3)]);
        // non-overlapping: after (0,3) nothing is left
        assert_eq!(spans("<a>+", &["a", "x", "a", "a"]), vec![(0, 1), (2, 4)]);
    }

    #[test]
    fn option_star_and_or() {
        assert_eq!(spans("<a>? <b>", &["b"]), vec![(0, 1)]);
        assert_eq!(spans("<a>* <b>", &["a", "a", "b"]), vec![(0, 3)]);
        // `|` inside `<...>` is a logic-OR token; the regex-level `|` needs a group
        assert_eq!(spans("(<a>|<b>)", &["x", "b"]), vec![(1, 2)]);
        assert_eq!(spans("(<a>|<b>) <c>", &["b", "c"]), vec![(0, 2)]);
    }

    #[test]
    fn min_max_quantifier() {
        assert_eq!(spans("<a>{0,3}", &["x"]), Vec::<(usize, usize)>::new());
        assert_eq!(spans("<a>{2,3}", &["a", "x"]), Vec::<(usize, usize)>::new());
        assert_eq!(spans("<a>{2,3}", &["a", "a", "x"]), vec![(0, 2)]);
        assert_eq!(spans("<a>{2,3}", &["a", "a", "a", "a"]), vec![(0, 3)]);
        assert_eq!(spans("<a>{2,3}", &["x", "a", "a", "a", "a"]), vec![(1, 4)]);
    }

    #[test]
    fn assertions() {
        assert_eq!(spans("^<a>", &["a", "a"]), vec![(0, 1)]);
        assert_eq!(spans("<a> ^<b>", &["b"]), Vec::<(usize, usize)>::new());
        assert_eq!(spans("<a>+ $", &["a", "a"]), vec![(0, 2)]);
    }

    #[test]
    fn min_matching_length_quirks() {
        assert_eq!(compile("<a>+").min_matching_length(), 1);
        assert_eq!(compile("<a>*").min_matching_length(), 0);
        assert_eq!(compile("<a>{2,3}").min_matching_length(), 2);
        // upstream quirk: MinMax.minMatchingLength is the minimum count only
        assert_eq!(compile("(<a> <b>){2,4}").min_matching_length(), 2);
        // `<a>|<b> <c>` is `(a|b) c`: the top-level group sums 1+1
        assert_eq!(compile("<a>|<b> <c>").min_matching_length(), 2);
    }

    struct Const(bool);
    impl TokenPredicate for Const {
        fn apply(&self, _token: &TokenView<'_>) -> bool {
            self.0
        }
    }

    #[test]
    fn logic_expressions() {
        let tags: Vec<String> = vec!["O".to_string()];
        let readings = [AnalyzedTokenReadings::new(vec![AnalyzedToken::new(
            "a",
            None,
            Some("SUB".into()),
        )])];
        // descriptions "T"/"'T'"/"F" evaluate to constants, so precedence and
        // parenthesization are observable independent of the token
        let check = |expr: &str| {
            let compiled = compile_logic(expr, &mut |d| {
                Ok(Box::new(Const(d.trim_matches('\'').starts_with('T'))))
            })
            .unwrap();
            compiled.apply(&TokenView {
                token: "a",
                chunk_tags: &tags,
                readings: Some(&readings[0]),
            })
        };
        assert!(check("T & T"));
        assert!(!check("T & F"));
        assert!(check("T | F"));
        assert!(check("!F & T"));
        assert!(!check("!T & F"));
        assert!(check("'T' & T"));
        // precedence: `T | F & F` is `T | (F & F)`, `F & T | T` is `(F & T) | T`
        assert!(check("T | F & F"));
        assert!(check("F & T | T"));
        assert!(!check("(T | F) & F"));
        // empty expressions are true for everything
        assert!(check(""));
    }
}
