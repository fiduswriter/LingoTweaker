//! XML disambiguation interpreter: port of `XmlRuleDisambiguator` +
//! `DisambiguationPatternRuleReplacer` action semantics (en-focused; unification
//! deferred to the French phase per plan §7).
//!
//! Rules are parsed with the shared `lt-pattern` loader (disambiguation.xml
//! uses the same `<rule>/<pattern>/<token>` structure) and applied to
//! analyzed sentences in rule order, mirroring LT.
use lt_data::PathExt as _;

pub mod multiword;

pub use multiword::MultiWordChunker;

use std::path::Path;
use std::sync::Arc;

use fancy_regex::Regex as FancyRegex;
use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, CoreError, Result};

/// Interned compiled regexes for XML disambiguation attributes: the same
/// handful of `postag`/`regexp` patterns recur across rule applications per
/// sentence, and a fresh `FancyRegex::new` per call showed up in the
/// steady-state profile (the `lt-pattern` matcher intern cache is the model).
fn cached_regex_inner(
    cache: &std::sync::Mutex<std::collections::HashMap<String, Option<Arc<FancyRegex>>>>,
    compile: impl FnOnce() -> Option<FancyRegex>,
    pattern: &str,
) -> Option<Arc<FancyRegex>> {
    let mut cache = cache.lock().unwrap();
    if let Some(found) = cache.get(pattern) {
        return found.clone();
    }
    let compiled = compile().map(Arc::new);
    cache.insert(pattern.to_string(), compiled.clone());
    compiled
}

/// Full-match regex for `postag` attributes (`String.matches` semantics).
fn cached_postag_regex(pattern: &str) -> Option<Arc<FancyRegex>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Option<Arc<FancyRegex>>>>,
    > = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    cached_regex_inner(
        cache,
        || FancyRegex::new(&format!("^(?:{pattern})$")).ok(),
        pattern,
    )
}

/// Plain (search, not anchored) regex for `regexp_match` filters.
fn cached_plain_regex(pattern: &str) -> Option<Arc<FancyRegex>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Option<Arc<FancyRegex>>>>,
    > = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    cached_regex_inner(cache, || FancyRegex::new(pattern).ok(), pattern)
}

/// (compiled pattern, view-relative match positions, unified readings)
type Application = (
    Arc<CompiledPattern>,
    Vec<Option<usize>>,
    Option<Vec<Vec<AnalyzedToken>>>,
);
use lt_pattern::matcher as pm;
use lt_pattern::{CompiledPattern, DisambigAction, DisambigMatchFilter, Grammar};

struct CompiledDisambigRule {
    /// rule id (the filter context exposes it as LT's `match.getRule().getId()`)
    id: String,
    /// one compiled pattern per `<or>` expansion
    compiled: Vec<Arc<CompiledPattern>>,
    disambig: DisambigAction,
    antipatterns: Vec<Arc<CompiledPattern>>,
    /// `<filter class="…" args="…">` of the rule (Java `RuleFilterEvaluator`
    /// runs in `PatternRuleMatcher.createRuleMatch` for disambiguation rules
    /// as well)
    filter: Option<lt_pattern::FilterSpec>,
    /// `AbstractTokenBasedRule.canBeIgnoredFor` fast path (the same hints the
    /// pattern-rule path uses; a rule whose mandatory literal cannot occur
    /// cannot match, so skipping it is safe)
    min_token_count: usize,
    hints: Vec<pm::CompiledHint>,
}

pub struct XmlDisambiguator {
    /// filter registry for `<filter class="…">` rules (unmapped classes keep
    /// the pre-filter behavior of applying the rule)
    filters: Option<lt_pattern::FilterRegistry>,
    /// language synthesizer (`DisambiguationPatternRuleReplacer` resolves
    /// `<match>` references with it)
    synth: Option<std::sync::Arc<dyn pm::Synthesizer>>,
    rules: Vec<CompiledDisambigRule>,
    /// `<unification>` equivalence definitions of disambiguation.xml
    unify_config: lt_pattern::EquivalenceConfig,
    /// rules not applied by this interpreter (uncompilable)
    pub skipped: usize,
}

impl XmlDisambiguator {
    /// A disambiguator with no rules (placeholder for pipelines whose real
    /// disambiguation lives elsewhere, e.g. the German pipeline).
    pub fn empty() -> Result<Self> {
        Ok(Self {
            filters: None,
            synth: None,
            rules: Vec::new(),
            unify_config: lt_pattern::EquivalenceConfig::from_defs(&[])
                .map_err(|e| lt_core::CoreError::Parse("unification".into(), e))?,
            skipped: 0,
        })
    }

    pub fn load(path: &Path) -> Result<Self> {
        Self::load_with_extra(path, None)
    }

    /// Load the language's disambiguation file plus (like Java
    /// `XmlRuleDisambiguator(lang, true)`) the global rule file appended at
    /// the end.
    pub fn load_with_extra(path: &Path, global: Option<&Path>) -> Result<Self> {
        let mut grammar = Grammar::load_file(path)?;
        if let Some(global) = global {
            if global.lt_exists() {
                let g = Grammar::load_file(global)?;
                grammar.rules.extend(g.rules);
                grammar.equivalence_defs.extend(g.equivalence_defs);
            }
        }
        let unify_config = lt_pattern::EquivalenceConfig::from_defs(&grammar.equivalence_defs)
            .map_err(|e| CoreError::Parse(path.display().to_string(), e))?;
        let mut rules = Vec::new();
        let mut skipped = 0usize;
        for rule in &grammar.rules {
            if rule.disambigs.is_empty() {
                continue;
            }
            if rule.pattern.tokens.is_empty() || rule.complex_pattern {
                skipped += rule.disambigs.len();
                continue;
            }
            let compiled: Vec<Arc<CompiledPattern>> = match lt_pattern::compile_patterns(
                &rule.pattern.tokens,
                rule.pattern.marker_start,
                rule.pattern.marker_end,
            ) {
                Ok(c) => c.into_iter().map(Arc::new).collect(),
                Err(_) => {
                    skipped += rule.disambigs.len();
                    continue;
                }
            };
            let mut antipatterns = Vec::new();
            for anti in &rule.antipatterns {
                if let Ok(cs) =
                    lt_pattern::compile_patterns(&anti.tokens, anti.marker_start, anti.marker_end)
                {
                    antipatterns.extend(cs.into_iter().map(Arc::new));
                }
            }
            let (min_token_count, hints, _anchor) = pm::pattern_hints(&rule.pattern.tokens);
            for disambig in &rule.disambigs {
                rules.push(CompiledDisambigRule {
                    id: rule.id.clone(),
                    compiled: compiled.clone(),
                    disambig: disambig.clone(),
                    antipatterns: antipatterns.clone(),
                    filter: rule.filter.clone(),
                    min_token_count,
                    hints: hints.clone(),
                });
            }
        }
        Ok(Self {
            filters: None,
            synth: None,
            rules,
            unify_config,
            skipped,
        })
    }

    pub fn rules_len(&self) -> usize {
        self.rules.len()
    }

    /// Rule id by index in application order (debugging/tests).
    pub fn rule_id(&self, index: usize) -> Option<&str> {
        self.rules.get(index).map(|r| r.id.as_str())
    }

    /// Apply one rule by index (debugging/tests).
    pub fn apply_rule_index(&self, index: usize, sentence: &mut AnalyzedSentence) {
        let Some(rule) = self.rules.get(index) else {
            return;
        };
        let view: Vec<usize> = sentence
            .tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
            })
            .map(|(i, _)| i)
            .collect();
        let view_refs: Vec<&lt_core::AnalyzedTokenReadings> =
            view.iter().map(|&i| &sentence.tokens[i]).collect();
        let mut applications: Vec<Application> = Vec::new();
        for compiled in &rule.compiled {
            let matches = pm::find_matches_with_unify(
                compiled,
                &[],
                &view_refs,
                None,
                Some(&self.unify_config),
            );
            applications.extend(
                matches
                    .into_iter()
                    .filter(|m| {
                        !rule.antipatterns.iter().any(|ap| {
                            antipattern_overlaps(
                                ap,
                                &view_refs,
                                m.start_tok(),
                                m.end_tok(),
                                self.synth.as_deref(),
                            )
                        })
                    })
                    .filter(|m| {
                        filter_accepts(rule, self.filters.as_ref(), &view_refs, m, sentence)
                    })
                    .map(|m| (Arc::clone(compiled), m.positions, m.unified)),
            );
        }
        drop(view_refs);
        for (compiled, positions, unified) in applications {
            apply_action(
                &rule.disambig,
                &compiled,
                sentence,
                &view,
                &positions,
                unified.as_deref(),
            );
        }
    }

    /// Set the language synthesizer used to resolve `<match>` references.
    pub fn set_synthesizer(&mut self, synth: std::sync::Arc<dyn pm::Synthesizer>) {
        self.synth = Some(synth);
    }

    /// Set the filter registry used by `<filter class="…">` disambiguation
    /// rules (`RuleFilterEvaluator`).
    pub fn set_filter_registry(&mut self, filters: lt_pattern::FilterRegistry) {
        self.filters = Some(filters);
    }

    /// Apply all disambiguation rules to the sentence, in order.
    pub fn apply(&self, sentence: &mut AnalyzedSentence) {
        // LT matches on tokensWithoutWhitespace: non-whitespace entries plus
        // sentence-boundary whitespace entries
        let view: Vec<usize> = sentence
            .tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
            })
            .map(|(i, _)| i)
            .collect();
        // Java `AnalyzedSentence.tokenOffsets`/`lemmaOffsets` (lowercased).
        // They are recomputed whenever a previous rule changed the sentence,
        // because the anchor hints read the *current* readings (e.g. the
        // `buen` → `bueno` replace rule makes `amigo_n`'s lemma anchor match).
        let build_maps = |sentence: &AnalyzedSentence,
                          view: &[usize]|
         -> (
            lt_pattern::matcher::LowerIndexMap,
            lt_pattern::matcher::LowerIndexMap,
        ) {
            let mut token_lower: lt_pattern::matcher::LowerIndexMap =
                lt_pattern::lower_index_map(view.len());
            let mut lemma_lower: lt_pattern::matcher::LowerIndexMap =
                lt_pattern::lower_index_map(view.len() * 2);
            for (vi, &i) in view.iter().enumerate() {
                let t = &sentence.tokens[i];
                token_lower
                    .entry(t.surface().to_lowercase())
                    .or_default()
                    .push(vi);
                for r in &t.readings {
                    let lemma = r.stem.as_deref().unwrap_or(&r.token).to_lowercase();
                    let list = lemma_lower.entry(lemma).or_default();
                    if list.last() != Some(&vi) {
                        list.push(vi);
                    }
                }
            }
            (token_lower, lemma_lower)
        };
        let mut maps_dirty = false;
        let (mut token_lower, mut lemma_lower) = build_maps(sentence, &view);

        for rule in &self.rules {
            if maps_dirty {
                let (t, l) = build_maps(sentence, &view);
                token_lower = t;
                lemma_lower = l;
                maps_dirty = false;
            }
            if view.len() < rule.min_token_count {
                continue;
            }
            // Java `RuleSet.textHinted`: a rule is classified by the first
            // *non-inflected* token hint; inflected hints are ignored, and a
            // rule without any non-inflected hint is always considered.
            if let Some(hint) = rule.hints.iter().find(|h| !h.inflected) {
                if hint
                    .values_lower
                    .iter()
                    .all(|v| !token_lower.contains_key(v))
                {
                    continue;
                }
            }
            // Java `DisambiguationPatternRuleReplacer.replace`: `doMatch`
            // scans start positions in order and applies each match's action
            // immediately; the action mutates the shared token readings in
            // place, so a later start sees the change. This matters for `add`
            // (readings added by an earlier match enable a later one,
            // `propaga_marca_reflexiu` chains han→pogut→tornar) and for
            // `remove` (a removal lets the pattern match a later start, e.g.
            // uk `non_v_kly_2` cascades the vocative removal through
            // `він старший сестри`). For a single-pattern rule, re-scan after
            // each application, only for starts after the applied one,
            // mirroring Java's forward scan.
            if rule.compiled.len() == 1 && matches!(rule.disambig.action.as_str(), "add" | "remove")
            {
                let compiled = &rule.compiled[0];
                let starts = compiled
                    .anchor
                    .as_ref()
                    .map(|a| pm::anchor_starts(a, &token_lower, &lemma_lower));
                if starts.as_ref().is_some_and(|s| s.is_empty()) {
                    continue;
                }
                let mut min_start = 0usize;
                let mut applied: Vec<(usize, usize)> = Vec::new();
                loop {
                    let view_refs: Vec<&lt_core::AnalyzedTokenReadings> =
                        view.iter().map(|&i| &sentence.tokens[i]).collect();
                    let mut best: Option<Application> = None;
                    let mut best_start = usize::MAX;
                    let mut best_end = 0usize;
                    let matches = pm::find_matches_with_unify(
                        compiled,
                        &[],
                        &view_refs,
                        starts.as_deref(),
                        Some(&self.unify_config),
                    );
                    for m in matches {
                        let s = m.start_tok();
                        let e = m.end_tok();
                        if s < min_start || applied.contains(&(s, e)) {
                            continue;
                        }
                        if rule.antipatterns.iter().any(|ap| {
                            antipattern_overlaps(ap, &view_refs, s, e, self.synth.as_deref())
                        }) {
                            continue;
                        }
                        if !filter_accepts(rule, self.filters.as_ref(), &view_refs, &m, sentence) {
                            continue;
                        }
                        if s < best_start {
                            best_start = s;
                            best_end = e;
                            best = Some((Arc::clone(compiled), m.positions, m.unified));
                        }
                    }
                    drop(view_refs);
                    let Some((compiled, positions, unified)) = best else {
                        break;
                    };
                    apply_action(
                        &rule.disambig,
                        &compiled,
                        sentence,
                        &view,
                        &positions,
                        unified.as_deref(),
                    );
                    applied.push((best_start, best_end));
                    min_start = best_start + 1;
                    maps_dirty = true;
                    if applied.len() > view.len() + 8 {
                        break;
                    }
                }
                continue;
            }
            // phase 1 (immutable): find matches and map to real token indices
            let applications: Vec<Application> = {
                let view_refs: Vec<&lt_core::AnalyzedTokenReadings> =
                    view.iter().map(|&i| &sentence.tokens[i]).collect();
                let mut applications = Vec::new();
                for compiled in &rule.compiled {
                    // Java `AbstractPatternRulePerformer.doMatch`: the anchor
                    // hint restricts the candidate start positions
                    let starts = compiled
                        .anchor
                        .as_ref()
                        .map(|a| pm::anchor_starts(a, &token_lower, &lemma_lower));
                    if starts.as_ref().is_some_and(|s| s.is_empty()) {
                        continue;
                    }
                    let matches = pm::find_matches_with_unify(
                        compiled,
                        &[],
                        &view_refs,
                        starts.as_deref(),
                        Some(&self.unify_config),
                    );
                    applications.extend(
                        matches
                            .into_iter()
                            .filter(|m| {
                                !rule.antipatterns.iter().any(|ap| {
                                    antipattern_overlaps(
                                        ap,
                                        &view_refs,
                                        m.start_tok(),
                                        m.end_tok(),
                                        self.synth.as_deref(),
                                    )
                                })
                            })
                            .filter(|m| {
                                filter_accepts(rule, self.filters.as_ref(), &view_refs, m, sentence)
                            })
                            .map(|m| (Arc::clone(compiled), m.positions, m.unified)),
                    );
                }
                applications
            };
            // phase 2 (mutable): apply actions
            if !applications.is_empty() {
                maps_dirty = true;
            }
            for (compiled, positions, unified) in applications {
                apply_action(
                    &rule.disambig,
                    &compiled,
                    sentence,
                    &view,
                    &positions,
                    unified.as_deref(),
                );
            }
        }
    }
}

/// `PatternRuleMatcher` filter evaluation for disambiguation rules: an
/// unmapped class (or no registry) keeps the rule applying, like before the
/// filter support was added.
fn filter_accepts(
    rule: &CompiledDisambigRule,
    filters: Option<&lt_pattern::FilterRegistry>,
    view_refs: &[&AnalyzedTokenReadings],
    m: &pm::PatternMatch,
    sentence: &AnalyzedSentence,
) -> bool {
    let Some(spec) = &rule.filter else {
        return true;
    };
    let Some(registry) = filters else {
        return true;
    };
    let Some(filter) = registry.get(&spec.class) else {
        return true;
    };
    let first = m.start_tok();
    let last = m.end_tok().min(view_refs.len().saturating_sub(1));
    let pattern_tokens: Vec<&AnalyzedTokenReadings> = view_refs[first..=last].to_vec();
    let mut token_positions = Vec::with_capacity(m.positions.len());
    let mut prev: Option<usize> = None;
    for pos in &m.positions {
        match pos {
            None => token_positions.push(0),
            Some(p) => {
                let consumed = match prev {
                    None => 1,
                    Some(prev_p) => p - prev_p,
                };
                token_positions.push(consumed);
                prev = Some(*p);
            }
        }
    }
    let args = match lt_pattern::resolve_args(&spec.args, &pattern_tokens, &token_positions) {
        Ok(a) => a,
        Err(_) => return true,
    };
    let range = if first < view_refs.len() && last < view_refs.len() {
        lt_core::TextRange::new(view_refs[first].start_pos, view_refs[last].end_pos())
    } else {
        lt_core::TextRange::new(0, 0)
    };
    let ctx = lt_pattern::FilterContext {
        rule_id: &rule.id,
        args,
        pattern_tokens: &pattern_tokens,
        sentence_tokens: view_refs,
        token_positions: &token_positions,
        pattern_token_pos: first,
        match_range: range,
        sentence_text: &sentence.text,
        message: String::new(),
        short_message: None,
        suggestions: Vec::new(),
    };
    filter.accept(&ctx).accepted
}

fn antipattern_overlaps(
    anti: &CompiledPattern,
    tokens: &[&lt_core::AnalyzedTokenReadings],
    first: usize,
    last: usize,
    synth: Option<&dyn pm::Synthesizer>,
) -> bool {
    for m in pm::find_matches_with_synth(anti, &[], &[], tokens, None, None, synth) {
        let a_first = m.start_tok();
        let a_last = m.end_tok();
        if a_first <= last && a_last >= first {
            return true;
        }
    }
    false
}

/// Java `AnalyzedTokenReadings` copy constructor: keeps the SENT_END and
/// PARA_END readings when the token was the sentence/paragraph end and the
/// action rewrote its readings.
fn restore_sent_end(tr: &mut AnalyzedTokenReadings) {
    if tr.is_sentence_end && !tr.has_pos_tag("SENT_END") {
        let surface = tr
            .readings
            .first()
            .map(|r| r.token.clone())
            .unwrap_or_default();
        let lemma = tr.readings.first().and_then(|r| r.stem.clone());
        tr.add_reading(AnalyzedToken::new(
            surface,
            lemma,
            Some("SENT_END".to_string()),
        ));
    }
    if tr.is_paragraph_end {
        tr.set_paragraph_end();
    }
    // Java's isTagged() is dynamic; every action that rewrites readings must
    // recompute the rust-side snapshot (the multiword chunker adds readings
    // through this helper's callers' code paths too).
    tr.refresh_is_tagged();
}

/// Java `executeAction` position arithmetic: derive the corrected start
/// (view) index and the number of targeted tokens from the match.
struct ActionTargets {
    from_view: usize,
    count: usize,
    token_positions: Vec<usize>,
    start_correction: i64,
    end_correction: i64,
}

fn action_targets(pattern: &CompiledPattern, positions: &[Option<usize>]) -> Option<ActionTargets> {
    let mut token_positions = vec![0usize; positions.len()];
    let mut prev: Option<usize> = None;
    for (i, p) in positions.iter().enumerate() {
        if let Some(idx) = p {
            token_positions[i] = match prev {
                None => 1,
                Some(pv) => idx - pv,
            };
            prev = Some(*idx);
        }
    }
    let first = positions.iter().flatten().copied().next()?;
    // Java `DisambiguationPatternRuleReplacer.replace` passes
    // `lastMarkerMatchToken` (not the last matched token) into
    // `executeAction`; with `-1` (no marker) the position correction is
    // skipped entirely.
    let mut last_marker: Option<usize> = None;
    for (i, p) in positions.iter().enumerate() {
        if p.is_some() && pattern.tokens.get(i).is_some_and(|t| t.in_marker) {
            last_marker = *p;
        }
    }
    let matching_tokens = token_positions.iter().filter(|&&c| c != 0).count() as i64;
    let mut start_correction = pattern.marker_start.map(|s| s as i64).unwrap_or(0);
    let mut end_correction = pattern
        .marker_end
        .map(|e| e as i64 - positions.len() as i64)
        .unwrap_or(0);
    let mut corrected_st = 0i64;
    if start_correction > 0 {
        corrected_st = -1;
        for l in 0..=start_correction as usize {
            if l < token_positions.len() {
                corrected_st += token_positions[l] as i64;
            }
        }
        for j in 0..=start_correction as usize {
            if j < token_positions.len() && token_positions[j] == 0 {
                start_correction -= 1;
            }
        }
    }
    if end_correction < 0 {
        let from = start_correction.max(0) as usize;
        for consumed in token_positions.iter().skip(from) {
            if *consumed == 0 {
                end_correction += 1;
            }
        }
    }
    let mut count = matching_tokens;
    if let Some(last) = last_marker {
        let max_pos_correction =
            ((last as i64 + 1 - (first as i64 + corrected_st)) - matching_tokens).max(0);
        count += max_pos_correction;
    }
    count = count - start_correction + end_correction;
    if count <= 0 {
        return None;
    }
    Some(ActionTargets {
        from_view: (first as i64 + corrected_st) as usize,
        count: count as usize,
        token_positions,
        start_correction,
        end_correction,
    })
}

/// Java `executeAction` MARKER/IMMUNIZE targeting: the view index and token
/// count covered by the `<marker>` span of a match, with the same position
/// corrections the other disambiguation actions use. `None` when no `<marker>`
/// exists (Java then treats every pattern token as inside the marker).
pub fn marker_targets(
    pattern: &CompiledPattern,
    positions: &[Option<usize>],
) -> Option<(usize, usize)> {
    let targets = action_targets(pattern, positions)?;
    Some((targets.from_view, targets.count))
}

/// Java's `DisambiguationPatternRuleReplacer` mutates some tokens in place
/// (`REMOVE`, `ADD`, `ADDCHUNK`, `IMMUNIZE`, `IGNORE_SPELLING`); because
/// `JLanguageTool.getAnalyzedSentence` stores the *same* token objects as
/// the pre-disambiguation view, `raw_pos="yes"` rules observe those
/// changes. Rust clones the pre view, so the in-place mutations are
/// replayed onto it. Wrapper-replacing actions — `REPLACE`, `UNIFY`,
/// `FILTER`, `FILTERALL` — are not mirrored; they call
/// `AnalyzedSentence::detach_pre_disambig`, which freezes the pre view for
/// that slot so later in-place mutations stop propagating (Java assigns a
/// new `AnalyzedTokenReadings` to the live slot, breaking the alias).
fn mirror_pre(sentence: &mut AnalyzedSentence, idx: usize, f: impl Fn(&mut AnalyzedTokenReadings)) {
    if sentence.pre_disambig_tokens.len() == sentence.tokens.len()
        && sentence.pre_disambig_is_aliased(idx)
    {
        f(&mut sentence.pre_disambig_tokens[idx]);
    }
}

fn apply_action(
    action: &DisambigAction,
    pattern: &CompiledPattern,
    sentence: &mut AnalyzedSentence,
    view: &[usize],
    positions: &[Option<usize>],
    unified: Option<&[Vec<AnalyzedToken>]>,
) {
    let Some(t) = action_targets(pattern, positions) else {
        return;
    };
    // map a view index to the real token index
    let at = |vi: usize| -> Option<usize> { view.get(vi).copied() };
    let readings = &action.new_readings;
    if action.action == "unify" {
        // LT `executeAction` UNIFY: replace the marker readings with the
        // unified sequence (one reading list per targeted token)
        if let Some(unified) = unified {
            if unified.len() == t.count {
                for (i, readings) in unified.iter().enumerate() {
                    let Some(vi) = t.from_view.checked_add(i) else {
                        break;
                    };
                    let Some(idx) = at(vi) else { break };
                    let tr = &mut sentence.tokens[idx];
                    tr.readings = readings.clone();
                    restore_sent_end(tr);
                    sentence.detach_pre_disambig(idx);
                }
            }
        }
        return;
    }
    // `<disambig><match .../></disambig>` (REPLACE with a match element):
    // keep only the readings selected by the match
    // (`DisambiguationPatternRuleReplacer` line 347).
    if action.action == "replace" && readings.is_empty() {
        if let Some(filter) = &action.filter_match {
            if let Some(idx) = at(t.from_view) {
                apply_match_filter(&mut sentence.tokens[idx], filter);
                // the copy constructor keeps SENT_END/PARA_END (e.g.
                // RP-ETRE_ADJ_AMBIG drops the `N`/SENT_END readings of the
                // final `grands` but Java restores SENT_END)
                restore_sent_end(&mut sentence.tokens[idx]);
                sentence.detach_pre_disambig(idx);
            }
            return;
        }
    }
    match action.action.as_str() {
        "remove" => {
            if !readings.is_empty() && readings.len() == t.count {
                for (i, wd) in readings.iter().enumerate() {
                    let Some(idx) = t.from_view.checked_add(i).and_then(at) else {
                        break;
                    };
                    let token = &mut sentence.tokens[idx];
                    let surface = token
                        .readings
                        .first()
                        .map(|r| r.token.clone())
                        .unwrap_or_default();
                    // LT `AnalyzedToken.matches` ANDs all non-null fields
                    token.readings.retain(|r| {
                        let mut found = true;
                        if !wd.token.is_empty() {
                            found &= r.token == wd.token;
                        }
                        if let Some(p) = &wd.postag {
                            found &= r.pos_tag.as_deref() == Some(p.as_str());
                        }
                        if let Some(l) = &wd.lemma {
                            found &= r.stem.as_deref() == Some(l.as_str());
                        }
                        !found
                    });
                    if token.readings.is_empty() {
                        token
                            .readings
                            .push(AnalyzedToken::new(surface.clone(), None, None));
                    }
                    restore_sent_end(token);
                    let surface = surface.clone();
                    let wd = wd.clone();
                    mirror_pre(sentence, idx, |pre| {
                        pre.readings.retain(|r| {
                            let mut found = true;
                            if !wd.token.is_empty() {
                                found &= r.token == wd.token;
                            }
                            if let Some(p) = &wd.postag {
                                found &= r.pos_tag.as_deref() == Some(p.as_str());
                            }
                            if let Some(l) = &wd.lemma {
                                found &= r.stem.as_deref() == Some(l.as_str());
                            }
                            !found
                        });
                        if pre.readings.is_empty() {
                            pre.readings
                                .push(AnalyzedToken::new(surface.clone(), None, None));
                        }
                        restore_sent_end(pre);
                    });
                }
            } else if let Some(pos) = &action.postag {
                if let Some(re) = cached_postag_regex(pos) {
                    if let Some(idx) = at(t.from_view) {
                        let first = &mut sentence.tokens[idx];
                        let surface = first
                            .readings
                            .first()
                            .map(|r| r.token.clone())
                            .unwrap_or_default();
                        first.readings.retain(|r| {
                            r.pos_tag
                                .as_deref()
                                .map(|tag| !re.is_match(tag).unwrap_or(false))
                                .unwrap_or(true)
                        });
                        if first.readings.is_empty() {
                            first
                                .readings
                                .push(AnalyzedToken::new(surface.clone(), None, None));
                        }
                        restore_sent_end(first);
                        let re = re.clone();
                        mirror_pre(sentence, idx, |pre| {
                            pre.readings.retain(|r| {
                                r.pos_tag
                                    .as_deref()
                                    .map(|tag| !re.is_match(tag).unwrap_or(false))
                                    .unwrap_or(true)
                            });
                            if pre.readings.is_empty() {
                                pre.readings
                                    .push(AnalyzedToken::new(surface.clone(), None, None));
                            }
                            restore_sent_end(pre);
                        });
                    }
                }
            }
        }
        "add" if readings.len() == t.count => {
            for (i, wd) in readings.iter().enumerate() {
                let Some(idx) = t.from_view.checked_add(i).and_then(at) else {
                    break;
                };
                let surface = sentence.tokens[idx].surface().to_string();
                let token = if wd.token.is_empty() {
                    surface
                } else {
                    wd.token.clone()
                };
                let lemma = wd.lemma.clone().unwrap_or_else(|| token.clone());
                let new_tok = AnalyzedToken::new(token, Some(lemma), wd.postag.clone());
                let tr = &mut sentence.tokens[idx];
                let exists = tr.readings.iter().any(|r| {
                    r.token == new_tok.token
                        && r.pos_tag == new_tok.pos_tag
                        && r.stem == new_tok.stem
                });
                if !exists {
                    tr.add_reading(new_tok.clone());
                    tr.is_tagged = tr.is_tagged || wd.postag.is_some();
                }
                mirror_pre(sentence, idx, |pre| {
                    let exists = pre.readings.iter().any(|r| {
                        r.token == new_tok.token
                            && r.pos_tag == new_tok.pos_tag
                            && r.stem == new_tok.stem
                    });
                    if !exists {
                        pre.add_reading(new_tok.clone());
                        pre.is_tagged = pre.is_tagged || new_tok.pos_tag.is_some();
                    }
                });
            }
        }
        "addchunk" if readings.len() == t.count => {
            for (i, wd) in readings.iter().enumerate() {
                if let Some(tag) = &wd.postag {
                    let Some(idx) = t.from_view.checked_add(i).and_then(at) else {
                        break;
                    };
                    let tr = &mut sentence.tokens[idx];
                    if !tr.chunk_tags.contains(tag) {
                        tr.chunk_tags.push(tag.clone());
                    }
                    let tag = tag.clone();
                    mirror_pre(sentence, idx, |pre| {
                        if !pre.chunk_tags.contains(&tag) {
                            pre.chunk_tags.push(tag.clone());
                        }
                    });
                }
            }
        }
        "filter" => {
            if let Some(pos) = &action.postag {
                if let Some(re) = cached_postag_regex(pos) {
                    if let Some(idx) = at(t.from_view) {
                        let first = &mut sentence.tokens[idx];
                        let any_match = first.readings.iter().any(|r| {
                            r.pos_tag
                                .as_deref()
                                .map(|tag| re.is_match(tag).unwrap_or(false))
                                .unwrap_or(false)
                        });
                        if any_match {
                            first.readings.retain(|r| {
                                r.pos_tag
                                    .as_deref()
                                    .map(|tag| re.is_match(tag).unwrap_or(false))
                                    .unwrap_or(true)
                            });
                            restore_sent_end(first);
                            sentence.detach_pre_disambig(idx);
                        }
                    }
                }
            }
        }
        "filterall" => {
            for i in 0..t.count {
                let pos_idx = i + t.start_correction.max(0) as usize;
                let pt = if t.token_positions.get(pos_idx).copied().unwrap_or(0) > 0 {
                    pattern.tokens.get(pos_idx)
                } else {
                    let mut k = 1usize;
                    while pos_idx + k < pattern.tokens.len() + t.end_correction.max(0) as usize
                        && t.token_positions.get(pos_idx + k).copied().unwrap_or(0) == 0
                    {
                        k += 1;
                    }
                    pattern.tokens.get(pos_idx + k)
                };
                let source = pt.and_then(|p| p.postag_source.clone());
                let Some(re) = pt.and_then(|p| p.postag.as_ref()) else {
                    continue;
                };
                let Some(idx) = t.from_view.checked_add(i).and_then(at) else {
                    break;
                };
                let tr = &mut sentence.tokens[idx];
                let original = tr.readings.clone();
                let mut kept: Vec<AnalyzedToken> = original
                    .iter()
                    .filter(|r| {
                        r.pos_tag
                            .as_deref()
                            .map(|tag| re.is_match(tag))
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect();
                if kept.is_empty() {
                    // Java `MatchState.filterReadings` → `getNewToken`: with
                    // no matching reading, one reading per original with a
                    // non-null tag carrying the *literal* selector POSTag
                    // (keeps the surface; `nul` in `de nul part` becomes
                    // `nul:(P\+)?D.*|K`)
                    if let Some(source) = source {
                        kept = original
                            .iter()
                            .filter(|r| r.pos_tag.is_some())
                            .map(|r| {
                                AnalyzedToken::new(
                                    r.token.clone(),
                                    r.stem.clone(),
                                    Some(source.clone()),
                                )
                            })
                            .collect();
                    }
                    if kept.is_empty() {
                        kept = original;
                    }
                }
                tr.readings = kept;
                tr.refresh_is_tagged();
                restore_sent_end(tr);
                sentence.detach_pre_disambig(idx);
            }
        }
        "replace" if !readings.is_empty() && readings.len() == t.count => {
            for (i, wd) in readings.iter().enumerate() {
                let Some(idx) = t.from_view.checked_add(i).and_then(at) else {
                    break;
                };
                let surface = sentence.tokens[idx].surface().to_string();
                let token = if wd.token.is_empty() {
                    surface
                } else {
                    wd.token.clone()
                };
                let lemma = wd.lemma.clone().unwrap_or_else(|| token.clone());
                let new_tok = AnalyzedToken::new(token, Some(lemma), wd.postag.clone());
                let tr = &mut sentence.tokens[idx];
                tr.readings = vec![new_tok];
                tr.is_tagged = wd.postag.is_some();
                restore_sent_end(tr);
                sentence.detach_pre_disambig(idx);
            }
        }
        "replace" if readings.is_empty() => {
            if let Some(pos) = &action.postag {
                let Some(idx) = at(t.from_view) else { return };
                let (surface, lemma) = {
                    let tr = &sentence.tokens[idx];
                    let mut lemma = None;
                    for r in &tr.readings {
                        if r.pos_tag.as_deref() == Some(pos.as_str()) && r.stem.is_some() {
                            lemma = r.stem.clone();
                        }
                    }
                    let lemma = lemma.or_else(|| tr.readings.first().and_then(|r| r.stem.clone()));
                    (tr.surface().to_string(), lemma)
                };
                let tr = &mut sentence.tokens[idx];
                tr.readings = vec![AnalyzedToken::new(surface, lemma, Some(pos.clone()))];
                tr.is_tagged = true;
                restore_sent_end(tr);
                sentence.detach_pre_disambig(idx);
            }
        }
        "immunize" => {
            for i in 0..t.count {
                if let Some(idx) = t.from_view.checked_add(i).and_then(at) {
                    sentence.tokens[idx].is_immunized = true;
                    mirror_pre(sentence, idx, |pre| pre.is_immunized = true);
                }
            }
        }
        "ignore_spelling" => {
            for i in 0..t.count {
                if let Some(idx) = t.from_view.checked_add(i).and_then(at) {
                    sentence.tokens[idx].is_ignore_spelling = true;
                    mirror_pre(sentence, idx, |pre| pre.is_ignore_spelling = true);
                }
            }
        }
        _ => {}
    }
}

/// Java `MatchState.filterReadings` for a `<disambig><match .../></disambig>`
/// action: keep the readings whose POS tag matches the selector (with
/// `postag_replace` applied when present); the token text goes through
/// `regexp_match`/`regexp_replace`. With no matching reading Java's
/// `getNewToken` falls back to one reading per original with the literal
/// selector POSTag.
fn apply_match_filter(tr: &mut AnalyzedTokenReadings, filter: &DisambigMatchFilter) {
    let surface = tr
        .readings
        .first()
        .map(|r| r.token.clone())
        .unwrap_or_default();
    let mut token = surface;
    if let (Some(pattern), Some(replace)) = (&filter.regexp_match, &filter.regexp_replace) {
        if let Some(re) = cached_plain_regex(pattern) {
            token = re.replace_all(&token, replace.as_str()).into_owned();
        }
    }
    let postag_re = filter
        .postag
        .as_deref()
        .filter(|_| filter.postag_regexp)
        .and_then(cached_postag_regex);
    let mut kept: Vec<AnalyzedToken> = Vec::new();
    for r in &tr.readings {
        let tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
        let matches = match &filter.postag {
            None => true,
            Some(p) if filter.postag_regexp => postag_re
                .as_ref()
                .is_some_and(|re| re.is_match(tag).unwrap_or(false)),
            Some(p) => tag == p,
        };
        if !matches {
            continue;
        }
        let new_tag = match (&filter.postag_replace, &filter.postag, &postag_re) {
            (Some(replace), Some(_), Some(re)) => {
                re.replace_all(tag, replace.as_str()).into_owned()
            }
            _ => tag.to_string(),
        };
        kept.push(AnalyzedToken::new(
            token.clone(),
            r.stem.clone(),
            Some(new_tag),
        ));
    }
    if kept.is_empty() {
        kept = tr
            .readings
            .iter()
            .map(|r| AnalyzedToken::new(token.clone(), r.stem.clone(), filter.postag.clone()))
            .collect();
    }
    if !kept.is_empty() {
        tr.readings = kept;
        tr.is_tagged = tr.readings.iter().any(|r| r.pos_tag.is_some());
    }
}

pub fn load_disambiguator(path: &Path) -> Result<XmlDisambiguator> {
    if !path.lt_exists() {
        return Err(CoreError::Data(format!("missing {}", path.display())));
    }
    XmlDisambiguator::load(path)
}

#[cfg(test)]
mod tests_helpers {
    use super::*;
    use lt_core::AnalyzedTokenReadings;

    pub fn load_disambiguator() -> Option<XmlDisambiguator> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/en/disambiguation.xml");
        if !path.lt_exists() {
            return None;
        }
        Some(XmlDisambiguator::load(&path).unwrap())
    }

    pub fn tr(word: &str, tags: &[&str]) -> AnalyzedTokenReadings {
        AnalyzedTokenReadings {
            readings: tags
                .iter()
                .map(|t| {
                    AnalyzedToken::new(
                        word.to_string(),
                        Some(word.to_lowercase()),
                        Some(t.to_string()),
                    )
                })
                .collect(),
            chunk_tags: Vec::new(),
            whitespace_before: true,
            start_pos: 0,
            raw_byte_len: 0,
            is_whitespace: false,
            is_sentence_start: false,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged: true,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe: false,
            is_pos_tag_unknown: false,
        }
    }

    pub fn sentence(words: &[(&str, &[&str])]) -> AnalyzedSentence {
        let mut tokens = vec![AnalyzedTokenReadings {
            readings: vec![AnalyzedToken::new("", None, Some("SENT_START".into()))],
            chunk_tags: Vec::new(),
            whitespace_before: false,
            start_pos: 0,
            raw_byte_len: 0,
            is_whitespace: false,
            is_sentence_start: true,
            is_sentence_end: false,
            is_paragraph_end: false,
            is_tagged: false,
            is_immunized: false,
            is_ignore_spelling: false,
            has_typographic_apostrophe: false,
            is_pos_tag_unknown: false,
        }];
        for (w, tags) in words {
            tokens.push(tr(w, tags));
        }
        if let Some(last) = tokens.last_mut() {
            let surface = last.surface().to_string();
            last.readings
                .push(AnalyzedToken::new(surface, None, Some("SENT_END".into())));
            last.is_sentence_end = true;
        }
        AnalyzedSentence {
            text: String::new(),
            offset: 0,
            tokens,
            pre_disambig_tokens: Vec::new(),
            pre_disambig_detached: Vec::new(),
        }
    }

    pub fn tags(tr: &AnalyzedTokenReadings) -> Vec<String> {
        tr.readings
            .iter()
            .map(|r| r.pos_tag.clone().unwrap_or_default())
            .collect()
    }

    #[test]
    fn sent_start_prp_vb_nn_applies() {
        let Some(d) = tests_helpers::load_disambiguator() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let idx = d
            .rules
            .iter()
            .position(|r| r.id == "SENT_START_PRP_VB_NN")
            .expect("rule loaded");
        let mut s = tests_helpers::sentence(&[
            ("I", &["PRP", "PRP_S1S"]),
            ("hope", &["NN:UN", "VB", "VBP"]),
            ("he", &["PRP", "PRP_S3SM"]),
            ("go", &["NN", "VB", "VBP"]),
            ("away", &["RB"]),
            (".", &[".", "PCT"]),
        ]);
        d.apply_rule_index(idx, &mut s);
        assert_eq!(tests_helpers::tags(&s.tokens[2]), vec!["VB", "VBP"]);
    }

    #[test]
    fn street_rule_does_not_match_he_walk() {
        let Some(d) = tests_helpers::load_disambiguator() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let Some(idx) = d.rules.iter().position(|r| r.id == "Street") else {
            eprintln!("Street rule absent");
            return;
        };
        let mut s = tests_helpers::sentence(&[
            ("He", &["PRP", "PRP_S3SM", "NNP"]),
            ("walk", &["NN", "VB", "VBP"]),
            ("to", &["IN", "TO"]),
            ("the", &["DT"]),
            ("building", &["JJ", "NN:UN", "VBG"]),
            (".", &[".", "PCT"]),
        ]);
        let before: Vec<Vec<String>> = s.tokens.iter().map(tags).collect();
        d.apply_rule_index(idx, &mut s);
        let after: Vec<Vec<String>> = s.tokens.iter().map(tags).collect();
        assert_eq!(before, after, "Street rule must not modify this sentence");
    }
}

#[cfg(test)]
mod cd_nn_test {
    use super::tests_helpers::{load_disambiguator, sentence, tags};
    #[test]
    fn cd_nn_and_group_applies() {
        let Some(d) = load_disambiguator() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let Some(idx) = d.rules.iter().position(|r| r.id == "CD_NN") else {
            panic!("CD_NN not loaded");
        };
        let mut s = sentence(&[
            ("Now", &["RB"]),
            (",", &[",", "PCT"]),
            ("a", &["DT"]),
            ("hundred", &["CD", "JJ", "NN"]),
            ("thousand", &["CD", "JJ", "NN"]),
            ("people", &["NN:U", "NNS"]),
            ("use", &["NN", "VB", "VBP"]),
            ("LanguageTool", &["NNP"]),
            (".", &[".", "PCT"]),
        ]);
        d.apply_rule_index(idx, &mut s);
        assert_eq!(tags(&s.tokens[5]), vec!["CD"], "thousand must become CD");
    }
}

#[cfg(test)]
mod fr_filterall_test {
    use super::tests_helpers::{sentence, tags};
    use super::XmlDisambiguator;
    use lt_data::PathExt as _;
    use std::path::Path;

    /// Java `MatchState.filterReadings` → `getNewToken` fallback: when a
    /// `filterall` selector matches none of the readings, the token keeps its
    /// surface with one literal-selector-tag reading per original reading.
    /// `RP-D_N_AMBIG` fires twice on `débarqués de nul part` (de+nul, then
    /// nul+part); the second match filters `nul` with the determiner
    /// selector, producing Java's `nul:(P\+)?D.*|K` twice instead of an
    /// empty reading list.
    /// Java's `AnalyzedTokenReadings(AnalyzedTokenReadings, List, String)`
    /// copy constructor re-adds SENT_END after a
    /// `<disambig><match .../></disambig>` REPLACE rewrote the readings.
    /// `RP-ETRE_ADJ_AMBIG` on `Ils sont grands` (no final period) drops the
    /// `N` and SENT_END readings but restores SENT_END; POINTS_2's `<and>`
    /// marker needs that reading to fire.
    #[test]
    fn match_element_replace_restores_sent_end() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/fr/disambiguation.xml");
        if !path.lt_exists() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let d = XmlDisambiguator::load(&path).unwrap();
        let idx = d
            .rules
            .iter()
            .position(|r| r.id == "RP-ETRE_ADJ_AMBIG")
            .expect("RP-ETRE_ADJ_AMBIG loaded");
        let mut s = sentence(&[
            ("Ils", &["R pers suj 3 m p"]),
            ("sont", &["V etre ind pres 3 p"]),
            ("grands", &["J m p", "N m p"]),
        ]);
        d.apply_rule_index(idx, &mut s);
        assert_eq!(tags(&s.tokens[3]), vec!["J m p", "SENT_END"]);
    }

    #[test]
    fn filterall_literal_fallback_keeps_surface() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/fr/disambiguation.xml");
        if !path.lt_exists() {
            eprintln!("skipping: no vendored data");
            return;
        }
        let d = XmlDisambiguator::load(&path).unwrap();
        assert!(d.rules.iter().any(|r| r.id == "RP-D_N_AMBIG"));
        let mut s = sentence(&[
            ("débarqués", &["J m p", "V ppa m p"]),
            ("de", &["D e sp", "P"]),
            ("nul", &["D m s", "J m s", "N m s", "R m s"]),
            ("part", &["N e s"]),
        ]);
        d.apply(&mut s);
        assert_eq!(s.tokens[3].surface(), "nul");
        assert_eq!(tags(&s.tokens[3]), vec!["(P\\+)?D.*|K", "(P\\+)?D.*|K"]);
    }
}
