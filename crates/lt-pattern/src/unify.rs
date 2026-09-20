//! Port of `org.languagetool.rules.patterns.Unifier` and
//! `UnifierConfiguration`: feature unification over a matched token sequence.
//!
//! Equivalence types come from the top-level `<unification feature="...">`
//! definitions of the loaded rule file (`<equivalence type="..."><token/>`).
//! A `<unify>` block in a pattern marks its elements with the features to
//! test (an empty feature list means "all types of the feature").

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use lt_core::AnalyzedToken;

use crate::matcher::CompiledToken;
use crate::EquivalenceDef;

/// Java `Unifier.UNIFY_IGNORE`: neutral sequence elements (`<unify-ignore>`).
pub const UNIFY_IGNORE: &str = "unify-ignore";

/// `UnifierConfiguration`: equivalence types per (feature, type) and the list
/// of types per feature.
pub struct EquivalenceConfig {
    pub(crate) types: HashMap<(String, String), CompiledToken>,
    pub(crate) features: BTreeMap<String, Vec<String>>,
}

impl EquivalenceConfig {
    pub fn from_defs(defs: &[EquivalenceDef]) -> Result<Self, String> {
        let mut types: HashMap<(String, String), CompiledToken> = HashMap::new();
        let mut features: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for def in defs {
            let key = (def.feature.clone(), def.type_name.clone());
            if types.contains_key(&key) {
                continue; // Java `setEquivalence`: first definition wins
            }
            types.insert(key, crate::matcher::compile_token(&def.token)?);
            features
                .entry(def.feature.clone())
                .or_default()
                .push(def.type_name.clone());
        }
        Ok(Self { types, features })
    }

    pub fn create_unifier(&self) -> Unifier<'_> {
        Unifier::new(self)
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

type EquivMap = BTreeMap<String, BTreeSet<String>>;

/// Java's `equivalencesMatched` maps are stored *by reference* into
/// `tokSequenceEquivalences` (`add(equivalencesMatched.get(tokCnt))`), so
/// `startNextToken`'s pruning mutates the maps `checkNext` later reads.
/// The `Rc<RefCell<..>>` keeps that aliasing observable.
type SharedEquivMap = Rc<RefCell<EquivMap>>;

/// Port of `org.languagetool.rules.patterns.Unifier`.
pub struct Unifier<'a> {
    config: &'a EquivalenceConfig,
    /// unified readings per sequence position
    tok_sequence: Vec<Vec<AnalyzedToken>>,
    /// equivalences per sequence position and reading, parallel to `tok_sequence`
    tok_sequence_equivalences: Vec<Vec<SharedEquivMap>>,
    equivalences_matched: Vec<SharedEquivMap>,
    all_feats_in: bool,
    tok_cnt: usize,
    readings_counter: usize,
    features_found: Vec<bool>,
    tmp_features_found: Vec<bool>,
    equivalences_to_be_kept: EquivMap,
    unification_feats: BTreeMap<String, Vec<String>>,
    in_unification: bool,
    uni_matched: bool,
    uni_all_matched: bool,
}

impl<'a> Unifier<'a> {
    pub fn new(config: &'a EquivalenceConfig) -> Self {
        Self {
            config,
            tok_sequence: Vec::new(),
            tok_sequence_equivalences: Vec::new(),
            equivalences_matched: Vec::new(),
            all_feats_in: false,
            tok_cnt: 0,
            readings_counter: 1,
            features_found: Vec::new(),
            tmp_features_found: Vec::new(),
            equivalences_to_be_kept: BTreeMap::new(),
            unification_feats: BTreeMap::new(),
            in_unification: false,
            uni_matched: false,
            uni_all_matched: false,
        }
    }

    fn types_for(&self, feat: &str, types: &[String]) -> Vec<String> {
        if !types.is_empty() {
            types.to_vec()
        } else {
            self.config.features.get(feat).cloned().unwrap_or_default()
        }
    }

    /// `Unifier.isSatisfied`: collect equivalences for `a_token` at the
    /// current sequence position.
    fn is_satisfied(
        &mut self,
        a_token: &AnalyzedToken,
        u_features: &BTreeMap<String, Vec<String>>,
    ) -> bool {
        if self.all_feats_in && self.equivalences_matched.is_empty() {
            return false;
        }
        self.unification_feats = u_features.clone();

        if self.all_feats_in {
            return self.check_next(a_token, u_features);
        }
        while self.equivalences_matched.len() <= self.tok_cnt {
            self.equivalences_matched
                .push(Rc::new(RefCell::new(BTreeMap::new())));
        }
        let mut unified = true;
        for (feat, types) in u_features {
            let types = self.types_for(feat, types);
            if types.is_empty() && !self.config.features.contains_key(feat) {
                return false;
            }
            for type_name in &types {
                let Some(test_elem) = self.config.types.get(&(feat.clone(), type_name.clone()))
                else {
                    return false;
                };
                if crate::matcher::reading_matches(test_elem, a_token, false) {
                    self.equivalences_matched[self.tok_cnt]
                        .borrow_mut()
                        .entry(feat.clone())
                        .or_default()
                        .insert(type_name.clone());
                }
            }
            unified = self.equivalences_matched[self.tok_cnt]
                .borrow()
                .contains_key(feat);
            if !unified {
                self.equivalences_matched.remove(self.tok_cnt);
                break;
            }
        }
        if unified {
            // Java stores the map object itself in both `equivalencesMatched`
            // and `tokSequenceEquivalences` (no copy), so sharing is required.
            let shared = Rc::clone(&self.equivalences_matched[self.tok_cnt]);
            if self.tok_cnt == 0 || self.tok_sequence.is_empty() {
                self.tok_sequence.push(vec![a_token.clone()]);
                self.tok_sequence_equivalences.push(vec![shared]);
            } else {
                self.tok_sequence[0].push(a_token.clone());
                self.tok_sequence_equivalences[0].push(shared);
            }
            self.tok_cnt += 1;
        }
        unified
    }

    fn check_next(
        &mut self,
        a_token: &AnalyzedToken,
        u_features: &BTreeMap<String, Vec<String>>,
    ) -> bool {
        let mut any_feat_unified = false;
        let mut token_features_found = self.tmp_features_found.clone();
        let mut equivalences_matched_here: EquivMap = BTreeMap::new();
        if self.all_feats_in {
            for i in 0..self.tok_cnt {
                let mut all_feats_unified = true;
                for (feat, types) in u_features {
                    let types = self.types_for(feat, types);
                    if types.is_empty() {
                        all_feats_unified = false;
                        continue;
                    }
                    let mut feat_unified = false;
                    for type_name in &types {
                        let matched = self.equivalences_matched.get(i).is_some_and(|m| {
                            m.borrow()
                                .get(feat)
                                .is_some_and(|set| set.contains(type_name))
                        }) && {
                            let test_elem =
                                self.config.types.get(&(feat.clone(), type_name.clone()));
                            test_elem
                                .is_some_and(|t| crate::matcher::reading_matches(t, a_token, false))
                        };
                        feat_unified |= matched;
                        if matched {
                            self.equivalences_to_be_kept
                                .entry(feat.clone())
                                .or_default()
                                .insert(type_name.clone());
                            equivalences_matched_here
                                .entry(feat.clone())
                                .or_default()
                                .insert(type_name.clone());
                        }
                    }
                    all_feats_unified &= feat_unified;
                }
                if let Some(slot) = token_features_found.get_mut(i) {
                    *slot = *slot || all_feats_unified;
                }
                any_feat_unified |= all_feats_unified;
            }
            if any_feat_unified {
                let shared = Rc::new(RefCell::new(equivalences_matched_here));
                if self.tok_sequence.len() == self.readings_counter {
                    self.tok_sequence.push(vec![a_token.clone()]);
                    self.tok_sequence_equivalences.push(vec![shared]);
                } else if self.readings_counter < self.tok_sequence.len() {
                    self.tok_sequence[self.readings_counter].push(a_token.clone());
                    self.tok_sequence_equivalences[self.readings_counter].push(shared);
                } else {
                    any_feat_unified = false;
                }
                self.tmp_features_found = token_features_found;
            }
        }
        any_feat_unified
    }

    /// `Unifier.startNextToken`.
    pub fn start_next_token(&mut self) {
        self.features_found = self.tmp_features_found.clone();
        self.readings_counter += 1;
        for j in 0..self.tok_sequence.len() {
            for i in 0..self.tok_sequence_equivalences[j].len() {
                let map = self.tok_sequence_equivalences[j][i].clone();
                let mut map = map.borrow_mut();
                for feat in self.config.features.keys() {
                    if feat == UNIFY_IGNORE {
                        continue;
                    }
                    if map.contains_key(feat) {
                        if let Some(keep) = self.equivalences_to_be_kept.get(feat) {
                            if let Some(set) = map.get_mut(feat) {
                                set.retain(|t| keep.contains(t));
                            }
                        } else {
                            map.remove(feat);
                        }
                    } else {
                        map.remove(feat);
                    }
                }
            }
        }
        self.equivalences_to_be_kept.clear();
    }

    /// `Unifier.startUnify`.
    pub fn start_unify(&mut self) {
        self.all_feats_in = true;
        for _ in 0..self.tok_cnt {
            self.features_found.push(false);
        }
        self.tmp_features_found = self.features_found.clone();
    }

    /// `Unifier.getFinalUnificationValue`.
    pub fn get_final_unification_value(&self, u_features: &BTreeMap<String, Vec<String>>) -> bool {
        let mut tok_unified = 0usize;
        for j in 0..self.tok_sequence.len() {
            let mut unified_tokens_found = false;
            for i in 0..self.tok_sequence_equivalences[j].len() {
                let map = self.tok_sequence_equivalences[j][i].borrow();
                if map.contains_key(UNIFY_IGNORE) {
                    if i == 0 {
                        tok_unified += 1;
                    }
                    unified_tokens_found = true;
                    continue;
                }
                let mut feat_unified = 0usize;
                for feat in u_features.keys() {
                    let set = map.get(feat);
                    match set {
                        Some(s) if s.is_empty() => feat_unified = 0,
                        _ => feat_unified += 1,
                    }
                    if feat_unified == self.unification_feats.len() && tok_unified <= j {
                        tok_unified += 1;
                        unified_tokens_found = true;
                        break;
                    }
                }
            }
            if !unified_tokens_found {
                return false;
            }
        }
        tok_unified == self.tok_sequence.len()
    }

    /// `Unifier.reset`.
    pub fn reset(&mut self) {
        self.equivalences_matched.clear();
        self.all_feats_in = false;
        self.tok_cnt = 0;
        self.features_found.clear();
        self.tmp_features_found.clear();
        self.tok_sequence.clear();
        self.tok_sequence_equivalences.clear();
        self.readings_counter = 1;
        self.uni_matched = false;
        self.uni_all_matched = false;
        self.in_unification = false;
    }

    /// `Unifier.isUnified` with `isMatched = true` (the caller only passes
    /// readings that matched the pattern element).
    pub fn is_unified(
        &mut self,
        match_token: &AnalyzedToken,
        u_features: &BTreeMap<String, Vec<String>>,
        last_reading: bool,
    ) -> bool {
        if self.in_unification {
            self.uni_matched |= self.is_satisfied(match_token, u_features);
            self.uni_all_matched = self.uni_matched;
            if last_reading {
                self.start_next_token();
                self.uni_matched = false;
            }
            return self.uni_all_matched && self.get_final_unification_value(u_features);
        }
        self.is_satisfied(match_token, u_features);
        if last_reading {
            self.in_unification = true;
            self.uni_matched = false;
            self.start_unify();
        }
        true
    }

    /// `Unifier.addNeutralElement`.
    pub fn add_neutral_element(&mut self, readings: &[AnalyzedToken]) {
        self.tok_sequence.push(readings.to_vec());
        let mut map = EquivMap::new();
        map.insert(UNIFY_IGNORE.to_string(), BTreeSet::new());
        // Java adds one shared map object for every reading.
        let shared = Rc::new(RefCell::new(map));
        self.tok_sequence_equivalences
            .push(vec![shared; readings.len()]);
        self.readings_counter += 1;
    }

    /// `Unifier.getUnifiedTokens`: one reading list per sequence position
    /// (readings that satisfy all unified features), or `None` when no
    /// unified reading exists for some position.
    pub fn get_unified_tokens(&self) -> Option<Vec<Vec<AnalyzedToken>>> {
        if self.tok_sequence.is_empty() {
            return None;
        }
        let mut out: Vec<Vec<AnalyzedToken>> = Vec::new();
        for j in 0..self.tok_sequence.len() {
            let mut unified_tokens_found = false;
            for i in 0..self.tok_sequence_equivalences[j].len() {
                let map = self.tok_sequence_equivalences[j][i].borrow();
                if map.contains_key(UNIFY_IGNORE) {
                    add_token(&mut out, &self.tok_sequence[j][i], j);
                    unified_tokens_found = true;
                } else {
                    let mut feat_unified = 0usize;
                    for feat in self.unification_feats.keys() {
                        let set = map.get(feat);
                        match set {
                            Some(s) if s.is_empty() => feat_unified = 0,
                            _ => feat_unified += 1,
                        }
                        if feat_unified == self.unification_feats.len() {
                            add_token(&mut out, &self.tok_sequence[j][i], j);
                            unified_tokens_found = true;
                        }
                    }
                }
            }
            if !unified_tokens_found {
                return None;
            }
        }
        Some(out)
    }
}

fn add_token(out: &mut Vec<Vec<AnalyzedToken>>, token: &AnalyzedToken, pos: usize) {
    if out.len() <= pos || out.is_empty() {
        out.push(vec![token.clone()]);
    } else {
        out[pos].push(token.clone());
    }
}
