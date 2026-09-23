//! French pipeline parts: tokenizer/tagger, the `FrenchHybridDisambiguator`
//! chunker order and (incrementally) the Java-coded French built-in rules.

use std::sync::Arc;

use lt_core::{AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings};
use lt_pattern::matcher as pm;
use lt_pattern::Synthesizer;
use lt_tokenize::FrenchWordTokenizer;

/// `FrenchHybridDisambiguator`: `spelling_global.txt` chunker
/// (`tagForNotAddingTags`, `ignoreSpelling=true`) → `fr/multiwords.txt`
/// chunker (`removePreviousTags=true`) → XML rules (+ global rules).
pub struct FrenchPipeline {
    pub tagger: Arc<lt_tagger::FrenchTagger>,
    pub synthesizer: Arc<lt_tagger::FrenchSynthesizer>,
    /// The same synthesizer through the pattern engine's trait.
    pub synth_adapter: Arc<crate::fr::synthesizer::FrenchSynthesizerAdapter>,
    pub global_chunker: lt_disambig::MultiWordChunker,
    pub multiwords_chunker: lt_disambig::MultiWordChunker,
    pub disambiguator: lt_disambig::XmlDisambiguator,
    /// `MorfologikFrenchSpellerRule` (`FR_SPELLING_RULE`, rule 4)
    pub spelling: Arc<crate::fr::spelling::FrenchSpellingRule>,
    /// `FrenchMultitokenSpeller` (`MultitokenSpellerFilter`)
    pub multitoken: Arc<crate::multitoken::MultitokenSpeller>,
    /// `fr.SimpleReplaceRule` (`FR_SIMPLE_REPLACE_SIMPLE`, 13, default on)
    pub simple_replace: crate::fr::simple_replace::FrenchSimpleReplaceRule,
    /// `fr.CompoundRule` (`FR_COMPOUNDS`, 10, default on)
    pub compound: crate::compound::CompoundRule,
    /// `FrenchRepeatedWordsRule` (`FR_REPEATEDWORDS`, 14, default on)
    pub repeated_words: crate::repeated_words::RepeatedWordsRule,
    /// `QuestionWhitespaceRule` (`FRENCH_WHITESPACE`, 12)
    pub question_whitespace: crate::fr::question_whitespace::FrenchQuestionWhitespaceRule,
    /// `QuestionWhitespaceStrictRule` (`FRENCH_WHITESPACE_STRICT`, 11, picky,
    /// default temp off)
    pub question_whitespace_strict: crate::fr::question_whitespace::FrenchQuestionWhitespaceRule,
}

impl FrenchPipeline {
    pub fn disambiguate(&self, sentence: &mut AnalyzedSentence) {
        self.global_chunker.apply(sentence);
        self.multiwords_chunker.apply(sentence);
        self.disambiguator.apply(sentence);
    }
}

/// `WordWithDeterminerFilter.suggestionHasNoErrors`: the rule ids checked in
/// addition to the whole `CAT_ELISION` category.
pub const ELISION_RECHECK_RULE_IDS: [&str; 5] =
    ["CET_CE", "CE_CET", "MA_VOYELLE", "MON_NFS", "VIEUX"];

/// `WordWithDeterminerFilter.suggestionHasNoErrors`: analyze `text` with the
/// French pipeline and report whether any `CAT_ELISION` rule (or one of the
/// five named rules) matches. `true` = the suggestion is clean.
pub fn suggestion_has_no_errors(
    french: &FrenchPipeline,
    rules: &[Arc<crate::pipeline::CompiledRule>],
    unify: &lt_pattern::EquivalenceConfig,
    text: &str,
) -> bool {
    let mut analyzed = analyze_french_sentence(french, text);
    // `raw_pos="yes"` rules (Java `PatternRule.isInterpretPosTagsPreDisambiguation`)
    // need the view captured before the disambiguation steps.
    analyzed.pre_disambig_tokens = analyzed.tokens.clone();
    analyzed.pre_disambig_detached = Vec::new();
    french.disambiguate(&mut analyzed);
    // Same token view as the engine's rule loop: whitespace tokens are
    // dropped (Java's `PatternRule` matches on
    // `getTokensWithoutWhitespace()`); passing them here made every
    // multi-token elision pattern miss.
    let tokens: Vec<&AnalyzedTokenReadings> = analyzed
        .tokens
        .iter()
        .filter(|t| {
            !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
        })
        .collect();
    let pre_tokens: Vec<&AnalyzedTokenReadings> = analyzed
        .pre_disambig_tokens
        .iter()
        .filter(|t| {
            !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
        })
        .collect();
    let synth: &dyn Synthesizer = french.synth_adapter.as_ref();
    for rule in rules {
        let rule_refs: &[&AnalyzedTokenReadings] = if rule.raw_pos { &pre_tokens } else { &tokens };
        for pattern in &rule.compiled {
            let found = pm::find_matches_with_synth(
                pattern,
                &rule.suggestions,
                &rule.suggestion_suppress,
                rule_refs,
                None,
                Some(unify),
                Some(synth),
            );
            if found.is_empty() {
                continue;
            }
            // `PatternRule.checkForAntiPatterns`: re-match on the immunized
            // view when an antipattern hits (same logic as the engine's
            // rule loop).
            let found = if rule.antipatterns.is_empty() {
                found
            } else {
                let mut immune = vec![false; tokens.len()];
                for ap in &rule.antipatterns {
                    for m in pm::find_matches_with_synth(
                        ap,
                        &[],
                        &[],
                        &tokens,
                        None,
                        Some(unify),
                        Some(synth),
                    ) {
                        let marker = if ap.marker_start.is_some() {
                            lt_disambig::marker_targets(ap, &m.positions)
                        } else {
                            None
                        };
                        match marker {
                            Some((from, count)) => {
                                for idx in from..(from + count).min(immune.len()) {
                                    immune[idx] = true;
                                }
                            }
                            None => {
                                for idx in m.start_tok()..=m.end_tok().min(immune.len() - 1) {
                                    immune[idx] = true;
                                }
                            }
                        }
                    }
                }
                if !immune.iter().any(|x| *x) {
                    found
                } else {
                    let mut owned: Vec<AnalyzedTokenReadings> =
                        rule_refs.iter().map(|t| (*t).clone()).collect();
                    for (idx, flag) in immune.iter().enumerate() {
                        if *flag {
                            owned[idx].is_immunized = true;
                        }
                    }
                    let search_refs: Vec<&AnalyzedTokenReadings> = owned.iter().collect();
                    pm::find_matches_with_synth(
                        pattern,
                        &rule.suggestions,
                        &rule.suggestion_suppress,
                        &search_refs,
                        None,
                        Some(unify),
                        Some(synth),
                    )
                }
            };
            for m in found {
                let range = pm::match_range(pattern, rule_refs, &m);
                if let Some((filter, filter_args)) = &rule.filter {
                    let first = m.start_tok();
                    let last = m.end_tok();
                    let pattern_tokens: Vec<&AnalyzedTokenReadings> =
                        rule_refs[first..=last].to_vec();
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
                    let Ok(args) =
                        lt_pattern::resolve_args(filter_args, &pattern_tokens, &token_positions)
                    else {
                        continue;
                    };
                    let ctx = lt_pattern::FilterContext {
                        rule_id: &rule.rule_id,
                        args,
                        pattern_tokens: &pattern_tokens,
                        sentence_tokens: rule_refs,
                        token_positions: &token_positions,
                        pattern_token_pos: first,
                        match_range: range,
                        sentence_text: text,
                        message: rule.message.clone(),
                        short_message: rule.short_message.clone(),
                        suggestions: Vec::new(),
                    };
                    if !filter.accept(&ctx).accepted {
                        continue;
                    }
                }
                return false;
            }
        }
    }
    true
}

/// French sentence tokenization + tagger. The French tokenizer consults the
/// tagger for hyphenated words; the tagger needs the whole token list.
pub fn analyze_french_sentence(french: &FrenchPipeline, text: &str) -> AnalyzedSentence {
    let is_tagged = |w: &str| french.tagger.is_tagged_word(w);
    let tokenizer = FrenchWordTokenizer::new(&is_tagged);
    let raw_tokens = tokenizer.tokenize(text);
    // Java `JLanguageTool.replaceSoftHyphens` (`Language
    // .getIgnoredCharactersRegex` strips `[\u00AD]`): the tagger sees the
    // cleaned token; the original surface comes back as an appended untagged
    // reading, so `massive` stays a tagged `massif:J f s` word and the
    // speller does not flag it.
    let cleaned: Vec<String> = raw_tokens
        .iter()
        .map(|t| t.replace('\u{00AD}', ""))
        .collect();
    let tagged = french.tagger.tag(&cleaned);

    let mut tokens: Vec<AnalyzedTokenReadings> = Vec::with_capacity(tagged.len() + 1);
    // synthetic sentence-start entry (LT: AnalyzedToken("", "SENT_START", null))
    tokens.push(AnalyzedTokenReadings {
        readings: vec![AnalyzedToken::new(
            "",
            None,
            Some(crate::pipeline::sentence_start_tag().to_string()),
        )],
        chunk_tags: Vec::new(),
        whitespace_before: false,
        start_pos: 0,
        raw_byte_len: 0,
        is_whitespace: false,
        is_sentence_start: true,
        is_sentence_end: false,
        is_paragraph_end: false,
        // Java `AnalyzedTokenReadings.isTagged()` counts SENT_START as a real
        // POS tag (only SENT_END/PARA_END/null are "no real tag")
        is_tagged: true,
        is_immunized: false,
        is_ignore_spelling: false,
        has_typographic_apostrophe: false,
        is_pos_tag_unknown: false,
    });

    let mut byte_pos = 0usize;
    let mut prev_was_whitespace = false;
    let mut last_non_ws_idx: Option<usize> = None;
    for (raw, mut reading) in raw_tokens.iter().zip(tagged) {
        let is_whitespace = lt_core::is_whitespace(raw);
        if raw.contains('\u{00AD}') {
            // `AnalyzedTokenReadings.addReading` with
            // `tagger.createToken(origToken, null)` (drops a trailing
            // unknown-word fallback first).
            reading.add_reading(AnalyzedToken::new(raw.clone(), None, None));
        }
        reading.whitespace_before = prev_was_whitespace;
        reading.start_pos = byte_pos;
        reading.raw_byte_len = raw.len();
        reading.is_whitespace = is_whitespace;
        reading.is_tagged = reading.readings.iter().any(|r| r.pos_tag.is_some());
        let has_typographic_apostrophe = raw.chars().count() > 1 && raw.contains('’');
        reading.has_typographic_apostrophe = has_typographic_apostrophe;
        tokens.push(reading);
        if !is_whitespace {
            last_non_ws_idx = Some(tokens.len() - 1);
        }
        byte_pos += raw.len();
        prev_was_whitespace = is_whitespace;
    }

    if let Some(idx) = last_non_ws_idx {
        let tr = &mut tokens[idx];
        if !tr
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref() == Some("SENT_END"))
        {
            let surface = tr
                .readings
                .first()
                .map(|r| r.token.clone())
                .unwrap_or_default();
            let lemma = tr.readings.first().and_then(|r| r.stem.clone());
            tr.add_reading(AnalyzedToken::new(
                surface,
                lemma,
                Some(crate::pipeline::sentence_end_tag().to_string()),
            ));
        }
        tr.is_sentence_end = true;
    } else if let Some(tr) = tokens.last_mut() {
        if !tr.is_sentence_end {
            tr.add_reading(AnalyzedToken::new(
                tr.surface().to_string(),
                tr.readings.first().and_then(|r| r.stem.clone()),
                Some(crate::pipeline::sentence_end_tag().to_string()),
            ));
            tr.is_sentence_end = true;
        }
        if tr.is_linebreak() {
            tr.set_paragraph_end();
        }
    }

    AnalyzedSentence {
        text: text.to_string(),
        offset: 0,
        tokens,
        pre_disambig_tokens: Vec::new(),
        pre_disambig_detached: Vec::new(),
    }
}
