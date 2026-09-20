//! `CatalanRemoteRewriteFilter` and its two dependencies:
//! `CatalanRemoteRewriteHelper` (the offline `cachedResponses` for the
//! corpus examples and the remote POST path) and `DiffsAsMatches` (the
//! java-diff-utils `MeyersDiff` over the `DiffRowGenerator` word splitter,
//! converted into `PseudoMatch` objects).
//!
//! The pinned environment has no `CA_REMOTE_REWRITE_SERVER`, so Java's
//! `sendPostRequest` returns the cached response for the rule's own example
//! sentences and the filter turns the diff into suggestions; with no cached
//! response the filter returns the match unchanged (`suppressMatch=false`).

use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::wordutil::is_punctuation_mark;

// ---------------------------------------------------------------------------
// DiffRowGenerator.SPLITTER_BY_WORD + DEFAULT_EQUALIZER
// ---------------------------------------------------------------------------

/// `DiffRowGenerator.SPLIT_BY_WORD_PATTERN`.
static SPLIT_BY_WORD: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"\s+|[,.\[\](){}/\\*+\-#]").unwrap());

/// `DiffRowGenerator.SPLITTER_BY_WORD.apply(text)`
/// (`splitStringPreserveDelimiter`): text chunks and delimiter chunks.
fn split_words(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut last = 0usize;
    for m in SPLIT_BY_WORD.find_iter(text) {
        if last < m.start() {
            result.push(text[last..m.start()].to_string());
        }
        result.push(m.as_str().to_string());
        last = m.end();
    }
    if last < text.len() {
        result.push(text[last..].to_string());
    }
    result
}

/// `DiffRowGenerator.DEFAULT_EQUALIZER`: `adjustWhitespace(a).equals(adjustWhitespace(b))`
/// with `adjustWhitespace(s) = s.trim().replaceAll("\\s+", "")`.
fn diff_equal(a: &str, b: &str) -> bool {
    let strip = |s: &str| -> String { s.chars().filter(|c| !c.is_whitespace()).collect() };
    strip(a) == strip(b)
}

// ---------------------------------------------------------------------------
// java-diff-utils `MeyersDiff` (4.12, the `DiffUtils` default algorithm)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeltaType {
    Insert,
    Delete,
    Change,
}

#[derive(Debug, Clone)]
struct Delta {
    type_: DeltaType,
    src_pos: usize,
    src_end: usize,
    tgt_pos: usize,
    tgt_end: usize,
}

#[derive(Debug, Clone)]
struct PathNode {
    i: i64,
    j: i64,
    prev: Option<usize>,
    snake: bool,
    bootstrap: bool,
}

/// `PathNode` arena; the algorithm builds a backwards linked list.
struct Path {
    nodes: Vec<PathNode>,
}

impl Path {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// `PathNode.previousSnake()`: the nearest snake ancestor; a non-snake
    /// node without a `prev` is returned itself, the bootstrap node yields
    /// `None`.
    fn previous_snake(&self, id: usize) -> Option<usize> {
        let node = &self.nodes[id];
        if node.bootstrap {
            None
        } else if node.snake {
            Some(id)
        } else {
            match node.prev {
                Some(p) => self.previous_snake(p),
                None => Some(id),
            }
        }
    }

    /// `new PathNode(i, j, snake, bootstrap, prev)`: a non-snake node stores
    /// `prev.previousSnake()` as its `prev`, a snake node keeps `prev`.
    fn push(&mut self, i: i64, j: i64, prev: Option<usize>, snake: bool, bootstrap: bool) -> usize {
        let prev = if snake {
            prev
        } else {
            prev.and_then(|p| self.previous_snake(p))
        };
        self.nodes.push(PathNode {
            i,
            j,
            prev,
            snake,
            bootstrap,
        });
        self.nodes.len() - 1
    }
}

/// `MeyersDiff.diff` = `buildPath` + `buildRevision` + `Patch.getDeltas`
/// (sorted by source position, stable).
fn myers_diff(orig: &[String], rev: &[String]) -> Vec<Delta> {
    let n = orig.len() as i64;
    let m = rev.len() as i64;
    let max = n + m + 1;
    let size = 1 + 2 * max;
    let middle = size / 2;
    let mut diagonal: Vec<Option<usize>> = vec![None; size as usize];
    let mut path = Path::new();

    // `diagonal[middle + 1] = new PathNode(0, -1, false /*snake*/, true /*bootstrap*/, null)`
    diagonal[(middle + 1) as usize] = Some(path.push(0, -1, None, false, true));
    let mut final_node: Option<usize> = None;
    'outer: for d in 0..max {
        let mut k = -d;
        while k <= d {
            let kmiddle = middle + k;
            let kplus = kmiddle + 1;
            let kminus = kmiddle - 1;
            let use_plus = if k == -d {
                true
            } else if k == d {
                false
            } else {
                let minus = diagonal[kminus as usize].expect("kminus node");
                let plus = diagonal[kplus as usize].expect("kplus node");
                path.nodes[minus].i < path.nodes[plus].i
            };
            let (mut i, prev) = if use_plus {
                let plus = diagonal[kplus as usize].expect("kplus node");
                (path.nodes[plus].i, plus)
            } else {
                let minus = diagonal[kminus as usize].expect("kminus node");
                (path.nodes[minus].i + 1, minus)
            };
            diagonal[kminus as usize] = None;
            let mut j = i - k;
            let node = path.push(i, j, Some(prev), false, false);
            while i < n && j < m && diff_equal(&orig[i as usize], &rev[j as usize]) {
                i += 1;
                j += 1;
            }
            let node = if i > path.nodes[node].i {
                path.push(i, j, Some(node), true, false)
            } else {
                node
            };
            diagonal[kmiddle as usize] = Some(node);
            if i >= n && j >= m {
                final_node = Some(node);
                break 'outer;
            }
            k += 2;
        }
        diagonal[(middle + d - 1) as usize] = None;
    }

    // `buildRevision` (walks the path backwards, so the changes come out in
    // reverse document order; `Patch.getDeltas` then sorts by source
    // position).
    let mut changes: Vec<Delta> = Vec::new();
    let mut node = final_node.expect("diff path");
    if path.nodes[node].snake {
        node = path.nodes[node].prev.expect("snake prev");
    }
    while let Some(prev) = path.nodes[node].prev {
        if path.nodes[prev].j < 0 {
            break;
        }
        debug_assert!(!path.nodes[node].snake);
        let i = path.nodes[node].i;
        let j = path.nodes[node].j;
        node = prev;
        let ianchor = path.nodes[node].i;
        let janchor = path.nodes[node].j;
        let type_ = if ianchor == i {
            DeltaType::Insert
        } else if janchor == j {
            DeltaType::Delete
        } else {
            DeltaType::Change
        };
        changes.push(Delta {
            type_,
            src_pos: ianchor.max(0) as usize,
            src_end: i.max(0) as usize,
            tgt_pos: janchor.max(0) as usize,
            tgt_end: j.max(0) as usize,
        });
        if path.nodes[node].snake {
            node = path.nodes[node].prev.expect("snake prev");
        }
    }
    changes.sort_by_key(|c| c.src_pos);
    changes
}

// ---------------------------------------------------------------------------
// DiffsAsMatches
// ---------------------------------------------------------------------------

/// `PseudoMatch` (LT `org.languagetool.tools.PseudoMatch`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct PseudoMatch {
    replacement: String,
    from_pos: usize,
    to_pos: usize,
}

const MAX_CONTIGUOUS_DISTANCE: usize = 3;
const INSERT_LOOKBACK_MAX: usize = 2;

/// `DiffsAsMatches.getPseudoMatches` (char-index based, like Java's UTF-16
/// code-unit positions for the BMP text of the cached responses).
fn get_pseudo_matches(original: &str, revised: &str) -> Vec<PseudoMatch> {
    let original_chars: Vec<char> = original.chars().collect();
    let original_tokens = split_words(original);
    let revised_tokens = split_words(revised);
    let deltas = myers_diff(&original_tokens, &revised_tokens);
    let mut matches =
        process_deltas_into_matches(&deltas, &original_tokens, &revised_tokens, &original_chars);
    matches = filter_out_apostrophe_diffs(matches, &original_chars);
    join_contiguous_matches(matches, &original_chars)
}

fn token_char_len(token: &str) -> usize {
    token.chars().count()
}

fn position_from_token_index(tokens: &[String], token_index: usize) -> usize {
    tokens
        .iter()
        .take(token_index.min(tokens.len()))
        .map(|t| token_char_len(t))
        .sum()
}

fn is_whitespace_token(token: &str) -> bool {
    !token.is_empty() && token.chars().all(char::is_whitespace)
}

fn is_whitespace_char(c: char) -> bool {
    c.is_whitespace()
}

fn process_deltas_into_matches(
    deltas: &[Delta],
    original_tokens: &[String],
    revised_tokens: &[String],
    original: &[char],
) -> Vec<PseudoMatch> {
    let mut matches: Vec<PseudoMatch> = Vec::new();
    let mut last_match: Option<PseudoMatch> = None;
    let mut last_delta: Option<&Delta> = None;

    for delta in deltas {
        let replacement_joined: String = (delta.tgt_pos..delta.tgt_end)
            .filter_map(|i| revised_tokens.get(i))
            .cloned()
            .collect();

        let error_index = delta.src_pos;
        let index_correction = if delta.type_ == DeltaType::Insert {
            INSERT_LOOKBACK_MAX.min(error_index)
        } else {
            0
        };
        let mut from_pos =
            position_from_token_index(original_tokens, error_index - index_correction);

        let was_prev_ws = error_index >= 1
            && original_tokens
                .get(error_index - 1)
                .is_some_and(|t| is_whitespace_token(t));
        let last_punct = if error_index >= 1 {
            original_tokens
                .get(error_index - 1)
                .filter(|t| is_punctuation_mark(t))
                .cloned()
                .unwrap_or_default()
        } else {
            String::new()
        };

        let underlined_error: String = (delta.src_pos..delta.src_end)
            .filter_map(|i| original_tokens.get(i))
            .cloned()
            .collect();
        let mut to_pos = from_pos + token_char_len(&underlined_error);

        let prefix: String = (error_index - index_correction..error_index)
            .filter_map(|i| original_tokens.get(i))
            .cloned()
            .collect();
        to_pos += token_char_len(&prefix);
        let mut replacement = format!("{prefix}{replacement_joined}");

        let mut underlined: String = original
            [from_pos.min(original.len())..to_pos.min(original.len())]
            .iter()
            .collect();

        // Remove leading white spaces
        while !underlined.is_empty()
            && !replacement.is_empty()
            && is_whitespace_char(underlined.chars().next().unwrap())
            && is_whitespace_char(replacement.chars().next().unwrap())
        {
            from_pos += 1;
            underlined = underlined.chars().skip(1).collect();
            replacement = replacement.chars().skip(1).collect();
        }

        // Special case: INSERT at the sentence start
        if from_pos == 0 && to_pos == 0 {
            to_pos = original_tokens
                .first()
                .map(|t| token_char_len(t))
                .unwrap_or(0);
            replacement = format!(
                "{replacement}{}",
                original_tokens.first().cloned().unwrap_or_default()
            );
        }

        // Remove trailing whitespace in INSERTs
        while !underlined.is_empty()
            && !replacement.is_empty()
            && is_whitespace_char(underlined.chars().last().unwrap())
            && is_whitespace_char(replacement.chars().last().unwrap())
        {
            to_pos -= 1;
            underlined.pop();
            replacement.pop();
        }

        // Merge with the previous delta? (`shouldMergeChangeWithInsert` /
        // `shouldMergeWithDelete`)
        let merge_insert = match (last_match.as_ref(), last_delta) {
            (Some(pm), Some(pd))
                if pd.type_ == DeltaType::Change
                    && delta.type_ == DeltaType::Insert
                    && (was_prev_ws || !last_punct.is_empty())
                    && delta.src_pos == pd.src_end + 1 =>
            {
                Some(pm.clone())
            }
            _ => None,
        };
        let merge_delete = match (last_match.as_ref(), delta.type_) {
            (Some(pm), DeltaType::Delete) if was_prev_ws && pm.to_pos + 1 == from_pos => {
                Some(pm.clone())
            }
            _ => None,
        };
        let match_ = if let Some(pm) = merge_insert {
            let tail: String = replacement.chars().skip(to_pos - from_pos).collect();
            let merged = PseudoMatch {
                replacement: format!("{}{}{}", pm.replacement, last_punct, tail),
                from_pos: pm.from_pos,
                to_pos,
            };
            matches.pop();
            merged
        } else if let Some(pm) = merge_delete {
            let merged = PseudoMatch {
                replacement: pm.replacement.clone(),
                from_pos: pm.from_pos,
                to_pos: to_pos - 1,
            };
            matches.pop();
            merged
        } else {
            PseudoMatch {
                replacement,
                from_pos,
                to_pos,
            }
        };
        matches.push(match_.clone());
        last_match = Some(match_);
        last_delta = Some(delta);
    }
    matches
}

fn filter_out_apostrophe_diffs(matches: Vec<PseudoMatch>, original: &[char]) -> Vec<PseudoMatch> {
    matches
        .into_iter()
        .filter(|m| {
            let original_part: String = original
                [m.from_pos.min(original.len())..m.to_pos.min(original.len())]
                .iter()
                .collect();
            let normalize = |s: &str| s.replace('’', "'");
            normalize(&original_part) != normalize(&m.replacement)
        })
        .collect()
}

fn join_contiguous_matches(mut matches: Vec<PseudoMatch>, original: &[char]) -> Vec<PseudoMatch> {
    let mut results: Vec<PseudoMatch> = Vec::new();
    let mut previous_end: Option<usize> = None;
    for m in matches.drain(..) {
        if let Some(prev_end) = previous_end {
            if m.from_pos.saturating_sub(prev_end) < MAX_CONTIGUOUS_DISTANCE {
                let previous = results.last().unwrap().clone();
                let between: String = if previous.to_pos < m.from_pos {
                    original[previous.to_pos.min(original.len())..m.from_pos.min(original.len())]
                        .iter()
                        .collect()
                } else {
                    String::new()
                };
                let joined = PseudoMatch {
                    replacement: format!("{}{}{}", previous.replacement, between, m.replacement),
                    from_pos: previous.from_pos,
                    to_pos: m.to_pos,
                };
                results.pop();
                results.push(joined);
                previous_end = Some(m.to_pos);
                continue;
            }
        }
        previous_end = Some(m.to_pos);
        results.push(m);
    }
    results
}

/// `DiffsAsMatches.getJoinedMatch`.
fn get_joined_match(
    pseudo_matches: &[PseudoMatch],
    original: &[char],
    pattern_from: i64,
    pattern_to: i64,
) -> Option<PseudoMatch> {
    if pseudo_matches.is_empty() {
        return None;
    }
    let mut min_from: i64 = -1;
    let mut max_to: i64 = -1;
    let mut previous_to: i64 = -1;
    let mut suggestion = String::new();
    for m in pseudo_matches {
        let in_range = (m.from_pos as i64 >= pattern_from && m.from_pos as i64 <= pattern_to)
            || (m.to_pos as i64 >= pattern_from && m.to_pos as i64 <= pattern_to);
        if (min_from != -1 && max_to == -1) || in_range {
            if min_from == -1 {
                min_from = m.from_pos as i64;
            } else if m.from_pos as i64 > previous_to {
                suggestion.extend(
                    original[previous_to as usize..m.from_pos.min(original.len())]
                        .iter()
                        .copied(),
                );
            }
            suggestion.push_str(&m.replacement);
            max_to = m.to_pos as i64;
        }
        previous_to = m.to_pos as i64;
    }
    if min_from > -1 && max_to > 0 {
        Some(PseudoMatch {
            replacement: suggestion,
            from_pos: min_from as usize,
            to_pos: max_to as usize,
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// CatalanRemoteRewriteHelper
// ---------------------------------------------------------------------------

/// `CatalanRemoteRewriteHelper.cachedResponses` (the offline test cache; the
/// pinned environment has no `CA_REMOTE_REWRITE_SERVER`).
fn cached_response(rule_id: &str, sentence: &str) -> Option<&'static str> {
    const GERUNDI: &[(&str, &str)] = &[
        (
            "El lladre va atracar dues joieries, escapant-se més tard amb un cotxe robat.",
            ">El lladre va atracar dues joieries i després va escapar-se amb un cotxe robat.",
        ),
        (
            "El lladre va atracar dues joieries, fugint després amb un cotxe robat.",
            ". El lladre va atracar dues joieries i després va fugir amb un cotxe robat.",
        ),
        (
            "El lladre va atracar dues joieries, essent assassinat pocs dies després.",
            "El lladre va atracar dues joieries i va ser assassinat pocs dies després.",
        ),
        (
            "El lladre va atracar dues joieries, fugint al cap de poc amb un cotxe robat.",
            "El lladre va atracar dues joieries i al cap de poc va fugir amb un cotxe robat.",
        ),
        (
            "El lladre va atracar dues joieries, escapant-se al cap d'una estona amb un cotxe robat.",
            "El lladre va atracar dues joieries i després es va escapar amb un cotxe robat.",
        ),
        (
            "Es van presentar els estatuts, aprovant-se l'endemà mateix.",
            "Es van presentar els estatuts i es van aprovar l'endemà mateix.",
        ),
        (
            "Es van presentar els estatuts, sent aprovats l'endemà mateix.",
            "Es van presentar els estatuts i van ser aprovats l'endemà mateix.",
        ),
        (
            "Es van presentar els estatuts, sent aprovats al cap de poc.",
            "Es van presentar els estatuts i al cap de poc van ser aprovats.",
        ),
        (
            "Es van presentar diverses esmenes, sent aprovades per una àmplia majoria.",
            "Es van presentar diverses esmenes, i van ser aprovades per una àmplia majoria.",
        ),
        (
            "Es presentaren diverses esmenes, sent aprovades per una àmplia majoria.",
            "Es presentaren diverses esmenes, i foren aprovades per una àmplia majoria.",
        ),
        (
            "Hi hagué un accident greu a l'autopista entre un camió i un turisme, morint al cap de poc els passatgers del turisme.",
            "Hi hagué un accident greu a l'autopista entre un camió i un turisme, i els passatgers del turisme van morir al cap de poc.",
        ),
        (
            "Hi hagué un accident greu a l'autopista entre un camió i un turisme, morint els passatgers del turisme al cap de poc.",
            "Hi hagué un accident greu a l'autopista entre un camió i un turisme i els passatgers del turisme van morir al cap de poc.",
        ),
        (
            "Va arribar tard a l'examen, perdent així després tota oportunitat d'aprovar l'assignatura.",
            "Va arribar tard a l'examen i, per això, va perdre tota oportunitat d'aprovar l'assignatura.",
        ),
        (
            "Van anul·lar la reunió, convocant-la per a la setmana següent.",
            "Van anul·lar la reunió i la van convocar per a la setmana següent.",
        ),
        (
            "Van anul·lar la reunió, convocant-la per a la propera setmana.",
            "Van anul·lar la reunió i la van convocar per a la propera setmana.",
        ),
    ];
    const EN_NO_INFINITIU: &[(&str, &str)] = &[
        (
            "En no poder venir, vam decidir deixar-ho córrer.",
            "Com que no podíem venir, vam decidir deixar-ho córrer.",
        ),
        (
            "Al no poder venir, vam decidir deixar-ho córrer.",
            "Com que no podíem venir, vam decidir deixar-ho córrer.",
        ),
        (
            "En no tenir efectes pràctics, vam decidir deixar-ho córrer.",
            "Com que no tenia efectes pràctics, vam decidir deixar-ho córrer.",
        ),
        (
            "Al no venir fonamentades, tothom s'estranya.",
            "Com que no venen fonamentades, tothom s'estranya.",
        ),
        (
            "Ho vaig deixar córrer al no disposar d'informació.",
            "Ho vaig deixar córrer perquè no disposava d'informació.",
        ),
        (
            "En no haver descobert el resultat, vam quedar decebuts.",
            "Com que no havíem descobert el resultat, vam quedar decebuts.",
        ),
    ];
    const ESCOLTAR_SENTIR: &[(&str, &str)] = &[
        (
            "Vaig escoltar que deien coses inversemblants.",
            "Vaig sentir que deien coses inversemblants.",
        ),
        (
            "Vaig escoltar atentament les seves explicacions.",
            "Vaig escoltar atentament les seves explicacions.",
        ),
    ];
    let table = match rule_id {
        "GERUNDI_POSTERIORITAT" => GERUNDI,
        "EN_NO_INFINITIU_CAUSAL_REMOTE" => EN_NO_INFINITIU,
        "CA_REMOTE_ESCOLTAR_SENTIR" => ESCOLTAR_SENTIR,
        _ => return None,
    };
    table
        .iter()
        .find(|(key, _)| *key == sentence)
        .map(|(_, value)| *value)
}

/// Test/probe helper: the raw `MeyersDiff` deltas for a pair
/// (`type, src_pos, src_end, tgt_pos, tgt_end`).
#[doc(hidden)]
pub fn debug_deltas(original: &str, revised: &str) -> Vec<(char, usize, usize, usize, usize)> {
    let orig_tokens = split_words(original);
    let rev_tokens = split_words(revised);
    myers_diff(&orig_tokens, &rev_tokens)
        .into_iter()
        .map(|d| {
            let t = match d.type_ {
                DeltaType::Insert => 'I',
                DeltaType::Delete => 'D',
                DeltaType::Change => 'C',
            };
            (t, d.src_pos, d.src_end, d.tgt_pos, d.tgt_end)
        })
        .collect()
}

/// Test/probe helper: `DiffsAsMatches.getPseudoMatches` output for a pair.
#[doc(hidden)]
pub fn debug_pseudo_matches(original: &str, revised: &str) -> Vec<(String, usize, usize)> {
    get_pseudo_matches(original, revised)
        .into_iter()
        .map(|m| (m.replacement, m.from_pos, m.to_pos))
        .collect()
}

/// `CatalanRemoteRewriteHelper.sendPostRequest` with no remote service
/// configured (`CA_REMOTE_REWRITE_SERVER` unset).
fn send_post_request(rule_id: &str, sentence: &str) -> String {
    let trimmed = sentence.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    cached_response(rule_id, trimmed)
        .map(str::to_string)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// CatalanRemoteRewriteFilter
// ---------------------------------------------------------------------------

/// `org.languagetool.rules.ca.CatalanRemoteRewriteFilter`.
pub struct CatalanRemoteRewriteFilter;

impl RuleFilter for CatalanRemoteRewriteFilter {
    fn rebuilds_match_type(&self) -> Option<&'static str> {
        // `new RuleMatch(rule, sentence, …)` resets the type to Other.
        Some("Other")
    }

    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let original_sentence = ctx.sentence_text.trim();
        let corrected = send_post_request(ctx.rule_id, original_sentence);
        let suppress_match = ctx
            .args
            .get("suppressMatch")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if corrected.is_empty() {
            return if suppress_match {
                FilterOutcome::reject()
            } else {
                FilterOutcome::accept()
            };
        }
        let original_chars: Vec<char> = original_sentence.chars().collect();
        let pseudo_matches = get_pseudo_matches(original_sentence, &corrected);
        // The sentence-relative match range (UTF-8 bytes) as char indices.
        let from_char = original_sentence
            .get(..ctx.match_range.start.min(original_sentence.len()))
            .map(|s| s.chars().count())
            .unwrap_or(0) as i64;
        let to_char = original_sentence
            .get(..ctx.match_range.end.min(original_sentence.len()))
            .map(|s| s.chars().count())
            .unwrap_or(0) as i64;
        let Some(pseudo) = get_joined_match(
            &pseudo_matches,
            &original_chars,
            from_char - 2,
            to_char + 60,
        ) else {
            return if suppress_match {
                FilterOutcome::reject()
            } else {
                FilterOutcome::accept()
            };
        };
        let suggestion = pseudo.replacement.clone();
        let underlined: String = original_chars
            [pseudo.from_pos.min(original_chars.len())..pseudo.to_pos.min(original_chars.len())]
            .iter()
            .collect();
        if ((pseudo.to_pos == original_chars.len() || pseudo.from_pos == 0)
            && underlined.trim().is_empty())
            || suggestion.trim() == underlined.trim()
            || pseudo.to_pos <= pseudo.from_pos
        {
            return if suppress_match {
                FilterOutcome::reject()
            } else {
                FilterOutcome::accept()
            };
        }
        // Char indices -> UTF-8 byte offsets of the (trimmed) sentence.
        let byte_pos = |char_idx: usize| -> usize {
            original_sentence
                .char_indices()
                .nth(char_idx)
                .map(|(i, _)| i)
                .unwrap_or(original_sentence.len())
        };
        FilterOutcome {
            accepted: true,
            range: Some(lt_core::TextRange::new(
                byte_pos(pseudo.from_pos),
                byte_pos(pseudo.to_pos),
            )),
            message: None,
            suggestions: Some(vec![lt_core::Suggestion {
                value: suggestion,
                short_description: None,
            }]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DiffsAsMatches` pinned against `scripts/oracle/ca/DiffProbe.java`
    /// (pinned Java build, 2026-09-20): the java-diff-utils `MeyersDiff`
    /// path (`PathNode.previousSnake`) and the merge/join post-processing.
    #[test]
    #[allow(clippy::type_complexity)]
    fn diffs_as_matches_matches_java() {
        let cases: &[(&str, &str, &[(&str, usize, usize)])] = &[
            (
                "El lladre va atracar dues joieries, escapant-se més tard amb un cotxe robat.",
                ">El lladre va atracar dues joieries i després va escapar-se amb un cotxe robat.",
                &[(">El", 0, 2), (" i després va escapar-se", 34, 56)],
            ),
            (
                "En no poder venir, vam decidir deixar-ho córrer.",
                "Com que no podíem venir, vam decidir deixar-ho córrer.",
                &[("Com que", 0, 2), ("podíem", 6, 11)],
            ),
            (
                "El lladre va atracar dues joieries, fugint després amb un cotxe robat.",
                ". El lladre va atracar dues joieries i després va fugir amb un cotxe robat.",
                &[(". El", 0, 2), (" i després va fugir", 34, 50)],
            ),
            (
                "Es van presentar els estatuts, sent aprovats al cap de poc.",
                "Es van presentar els estatuts i al cap de poc van ser aprovats.",
                &[(" i", 29, 44), ("poc van ser aprovats", 55, 58)],
            ),
        ];
        for (original, revised, expected) in cases {
            let got: Vec<(String, usize, usize)> = get_pseudo_matches(original, revised)
                .into_iter()
                .map(|m| (m.replacement, m.from_pos, m.to_pos))
                .collect();
            let want: Vec<(String, usize, usize)> = expected
                .iter()
                .map(|(r, f, t)| ((*r).to_string(), *f, *t))
                .collect();
            assert_eq!(got, want, "pair {original:?} -> {revised:?}");
        }
    }

    #[test]
    fn cached_responses_are_trimmed_lookups() {
        assert_eq!(
            send_post_request(
                "GERUNDI_POSTERIORITAT",
                "  El lladre va atracar dues joieries, essent assassinat pocs dies després. "
            ),
            "El lladre va atracar dues joieries i va ser assassinat pocs dies després."
        );
        assert_eq!(send_post_request("GERUNDI_POSTERIORITAT", "Cap frase."), "");
    }
}
