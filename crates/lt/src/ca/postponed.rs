//! `org.languagetool.rules.ca.PostponedAdjectiveConcordanceFilter` (D-151):
//! checks a postponed adjective that agrees with none of the previous
//! nouns/determiners. Ported from the Spanish sibling in
//! `crate::es::filters` with the Catalan pattern constants, the
//! `updateJValue` coordination handling (`dos o més`) and the
//! `RG_anteposat`/`com`-conjunction differences.

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Suggestion, TextRange};
use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::ca::filters::Env;

const MAX_LEVELS: usize = 4;

/// `matchPostagRegexp`: null POS tags are tested as `UNKNOWN`.
fn postag_matches(tr: &AnalyzedTokenReadings, pattern: &regex::Regex) -> bool {
    tr.readings.iter().any(|r| {
        let pos_tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
        pattern.is_match(pos_tag)
    })
}

fn get_analyzed_token<'a>(
    tr: &'a AnalyzedTokenReadings,
    pattern: &regex::Regex,
) -> Option<&'a AnalyzedToken> {
    tr.readings.iter().find(|r| {
        let pos_tag = r.pos_tag.as_deref().unwrap_or("UNKNOWN");
        pattern.is_match(pos_tag)
    })
}

fn re(pattern: &str) -> regex::Regex {
    regex::Regex::new(&format!("^(?:{pattern})$")).unwrap()
}

struct PaPatterns {
    nom: regex::Regex,
    nom_ms: regex::Regex,
    nom_fs: regex::Regex,
    nom_mp: regex::Regex,
    nom_mn: regex::Regex,
    nom_fp: regex::Regex,
    nom_cs: regex::Regex,
    nom_cp: regex::Regex,
    nom_det: regex::Regex,
    gn_any: regex::Regex,
    gn_ms: regex::Regex,
    gn_fs: regex::Regex,
    gn_mp: regex::Regex,
    gn_fp: regex::Regex,
    det: regex::Regex,
    det_cs: regex::Regex,
    det_ms: regex::Regex,
    det_fs: regex::Regex,
    det_mp: regex::Regex,
    det_fp: regex::Regex,
    subst: [(regex::Regex, regex::Regex, regex::Regex); 6],
    adjectiu: regex::Regex,
    adjectiu_ms: regex::Regex,
    adjectiu_fs: regex::Regex,
    adjectiu_mp: regex::Regex,
    adjectiu_fp: regex::Regex,
    adjectiu_cp: regex::Regex,
    adjectiu_cs: regex::Regex,
    adjectiu_s: regex::Regex,
    adjectiu_p: regex::Regex,
    adverbi: regex::Regex,
    conjuncio: regex::Regex,
    punctuacio: regex::Regex,
    loc_adv: regex::Regex,
    adverbis_acceptats: regex::Regex,
    coordinacio_ioni: regex::Regex,
    keep_count: regex::Regex,
    keep_count2: regex::Regex,
    stop_count: regex::Regex,
    preposicions: regex::Regex,
    preposicio_canvi_nivell: regex::Regex,
    verb: regex::Regex,
    gv: regex::Regex,
}

impl PaPatterns {
    fn new() -> Self {
        Self {
            nom: re("N.*"),
            nom_ms: re("N.MS.*"),
            nom_fs: re("N.FS.*"),
            nom_mp: re("N.MP.*"),
            nom_mn: re("N.MN.*"),
            nom_fp: re("N.FP.*"),
            nom_cs: re("N.CS.*"),
            nom_cp: re("N.CP.*"),
            nom_det: re("N.*|D[NDA0I].*"),
            gn_any: re("_GN_.*"),
            gn_ms: re("_GN_MS"),
            gn_fs: re("_GN_FS"),
            gn_mp: re("_GN_MP"),
            gn_fp: re("_GN_FP"),
            det: re("D[NDA0IP].*"),
            det_cs: re("D[NDA0IP]0CS0"),
            det_ms: re("D[NDA0IP]0MS0"),
            det_fs: re("D[NDA0IP]0FS0"),
            det_mp: re("D[NDA0IP]0MP0"),
            det_fp: re("D[NDA0IP]0FP0"),
            subst: [
                (
                    re("N.[FMC][SN].*|A..[FMC][SN].*|D[NDA0I]0[FM]S0"),
                    re("_GN_[MF]S"),
                    re("A...[SN].*|V.P..S..?|PX..S.*"),
                ),
                (
                    re("N.[FMC][PN].*|A..[FMC][PN].*|D[NDA0I]0[FM]P0"),
                    re("_GN_[MF]P"),
                    re("A...[PN].*|V.P..P..?|PX..P.*"),
                ),
                (
                    re("N.[MC][SN].*|A..[MC][SN].*|V.P..SM.?|PX.MS.*|D[NDA0I]0MS0|PI0MS000"),
                    re("_GN_MS"),
                    re("A..[MC][SN].*|V.P..SM.?|PX.MS.*"),
                ),
                (
                    re("N.[FC][SN].*|A..[FC][SN].*|V.P..SF.?|PX.FS.*|D[NDA0I]0FS0|PI0FS000"),
                    re("_GN_FS"),
                    re("A..[FC][SN].*|V.P..SF.?|PX.FS.*"),
                ),
                (
                    re("N.[MC][PN].*|A..[MC][PN].*|V.P..PM.?|PX.MP.*|D[NDA0I]0MP0"),
                    re("_GN_MP"),
                    re("A..[MC][PN].*|V.P..PM.?|PX.MP.*"),
                ),
                (
                    re("N.[FC][PN].*|A..[FC][PN].*|V.P..PF.?|PX.FP.*|D[NDA0I]0FP0"),
                    re("_GN_FP"),
                    re("A..[FC][PN].*|V.P..PF.?|PX.FP.*"),
                ),
            ],
            adjectiu: re("AQ.*|V.P.*|PX.*|.*LOC_ADJ.*"),
            adjectiu_ms: re("A..[MC][SN].*|V.P..SM.?|PX.MS.*"),
            adjectiu_fs: re("A..[FC][SN].*|V.P..SF.?|PX.FS.*"),
            adjectiu_mp: re("A..[MC][PN].*|V.P..PM.?|PX.MP.*"),
            adjectiu_fp: re("A..[FC][PN].*|V.P..PF.?|PX.FP.*"),
            adjectiu_cp: re("A..C[PN].*"),
            adjectiu_cs: re("A..C[SN].*"),
            adjectiu_s: re("A...[SN].*|V.P..S..?|PX..S.*"),
            adjectiu_p: re("A...[PN].*|V.P..P..?|PX..P.*"),
            adverbi: re("R.|.*LOC_ADV.*"),
            conjuncio: re("C.|.*LOC_CONJ.*"),
            punctuacio: re("_PUNCT"),
            loc_adv: re(".*LOC_ADV.*"),
            adverbis_acceptats: re("RG_anteposat"),
            coordinacio_ioni: re("i|o|ni"),
            keep_count: re(
                "A.*|N.*|D[NAIDP].*|SPS.*|.*LOC_ADV.*|V.P.*|_PUNCT.*|.*LOC_ADJ.*|PX.*|PI0.S000|UNKNOWN",
            ),
            keep_count2: re(",|i|o|ni"),
            stop_count: re("[;:]"),
            preposicions: re("SPS.*"),
            preposicio_canvi_nivell: re("de|d'|en|sobre|a|entre|per|pe|amb|sense|contra|com|envers"),
            verb: re("V.[^P].*|_GV_"),
            gv: re("_GV_"),
        }
    }
}

struct PaApparitions {
    adverb: bool,
    conjunction: bool,
    punctuation: bool,
}

impl PostponedAdjectiveConcordanceFilter {
    /// `keepCounting`.
    fn keep_counting(p: &PaPatterns, a: &PaApparitions, tr: &AnalyzedTokenReadings) -> bool {
        if p.preposicio_canvi_nivell.is_match(tr.surface()) {
            return true;
        }
        if tr.surface() == "." {
            // it is not sentence end, but abbreviation
            return true;
        }
        if (a.adverb && a.conjunction)
            || (a.adverb && a.punctuation)
            || (a.conjunction && a.punctuation)
            || (a.punctuation && postag_matches(tr, &p.punctuacio))
        {
            return false;
        }
        (postag_matches(tr, &p.keep_count)
            || p.keep_count2.is_match(tr.surface())
            || postag_matches(tr, &p.adverbis_acceptats))
            && !p.stop_count.is_match(tr.surface())
            && (!postag_matches(tr, &p.gv) || postag_matches(tr, &p.gn_any))
    }

    /// `updateApparitions`.
    fn update_apparitions(p: &PaPatterns, a: &mut PaApparitions, tr: &AnalyzedTokenReadings) {
        a.conjunction |= postag_matches(tr, &p.conjuncio);
        if tr.surface() == "com" {
            return;
        }
        if postag_matches(tr, &p.nom) || postag_matches(tr, &p.adjectiu) {
            a.adverb = false;
            a.conjunction = false;
            a.punctuation = false;
            return;
        }
        a.adverb |= postag_matches(tr, &p.adverbi);
        a.punctuation |= postag_matches(tr, &p.punctuacio) || tr.surface() == ",";
    }

    /// `updateJValue` (the old coordination loop is commented out upstream;
    /// only the `dos o més` handling remains).
    fn update_j_value(
        p: &PaPatterns,
        tokens: &[&AnalyzedTokenReadings],
        i: usize,
        j: usize,
        _level: usize,
    ) -> usize {
        if p.coordinacio_ioni.is_match(tokens[i - j].surface())
            && i - j - 1 > 0
            && i - j + 1 < tokens.len()
            && postag_matches(tokens[i - j - 1], &p.det)
            && tokens[i - j + 1].surface() == "més"
        {
            return j + 1;
        }
        j
    }
}

impl RuleFilter for PostponedAdjectiveConcordanceFilter {
    #[allow(clippy::too_many_lines)]
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let p = PaPatterns::new();
        let add_comma = ctx
            .args
            .get("addComma")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let tokens: Vec<&AnalyzedTokenReadings> = ctx
            .sentence_tokens
            .iter()
            .copied()
            .filter(|t| {
                !t.is_whitespace || t.is_sentence_start || t.is_sentence_end || t.is_paragraph_end
            })
            .collect();
        let i = ctx.pattern_token_pos;
        if i >= tokens.len() {
            return FilterOutcome::reject();
        }
        let mut is_plural = true;
        let mut is_prev_noun = false;
        let mut can_be_ms = false;
        let mut can_be_fs = false;
        let mut can_be_mp = false;
        let mut can_be_fp = false;
        let mut can_be_p = false;
        let mut c_nt = [0i32; MAX_LEVELS];
        let mut c_nms = [0i32; MAX_LEVELS];
        let mut c_nfs = [0i32; MAX_LEVELS];
        let mut c_nmp = [0i32; MAX_LEVELS];
        let mut c_nmn = [0i32; MAX_LEVELS];
        let mut c_nfp = [0i32; MAX_LEVELS];
        let mut c_ncs = [0i32; MAX_LEVELS];
        let mut c_ncp = [0i32; MAX_LEVELS];
        let mut c_dms = [0i32; MAX_LEVELS];
        let mut c_dfs = [0i32; MAX_LEVELS];
        let mut c_dmp = [0i32; MAX_LEVELS];
        let mut c_dfp = [0i32; MAX_LEVELS];
        let mut level = 0usize;
        let mut j = 1usize;
        let mut app = PaApparitions {
            adverb: false,
            conjunction: false,
            punctuation: false,
        };
        while i > j && Self::keep_counting(&p, &app, tokens[i - j]) && level < MAX_LEVELS {
            if !is_prev_noun {
                if postag_matches(tokens[i - j], &p.nom)
                    || (i - j - 1 > 0
                        && !postag_matches(tokens[i - j], &p.nom)
                        && postag_matches(tokens[i - j], &p.adjectiu)
                        && postag_matches(tokens[i - j - 1], &p.det))
                {
                    if postag_matches(tokens[i - j], &p.gn_ms) {
                        c_nms[level] += 1;
                        can_be_ms = true;
                    }
                    if postag_matches(tokens[i - j], &p.gn_fs) {
                        c_nfs[level] += 1;
                        can_be_fs = true;
                    }
                    if postag_matches(tokens[i - j], &p.gn_mp) {
                        c_nmp[level] += 1;
                        can_be_mp = true;
                    }
                    if postag_matches(tokens[i - j], &p.gn_fp) {
                        c_nfp[level] += 1;
                        can_be_fp = true;
                    }
                }
                if !postag_matches(tokens[i - j], &p.gn_any) {
                    if postag_matches(tokens[i - j], &p.nom_ms) {
                        c_nms[level] += 1;
                        can_be_ms = true;
                    } else if postag_matches(tokens[i - j], &p.nom_fs) {
                        c_nfs[level] += 1;
                        can_be_fs = true;
                    } else if postag_matches(tokens[i - j], &p.nom_mp) {
                        c_nmp[level] += 1;
                        can_be_mp = true;
                    } else if postag_matches(tokens[i - j], &p.nom_mn) {
                        c_nmn[level] += 1;
                        can_be_ms = true;
                        can_be_mp = true;
                    } else if postag_matches(tokens[i - j], &p.nom_fp) {
                        c_nfp[level] += 1;
                        can_be_fp = true;
                    } else if postag_matches(tokens[i - j], &p.nom_cs) {
                        c_ncs[level] += 1;
                        can_be_ms = true;
                        can_be_fs = true;
                    } else if postag_matches(tokens[i - j], &p.nom_cp) {
                        c_ncp[level] += 1;
                        can_be_fp = true;
                        can_be_mp = true;
                    }
                }
            }
            // avoid two consecutive nouns
            if postag_matches(tokens[i - j], &p.nom) {
                c_nt[level] += 1;
                is_prev_noun = true;
            } else {
                is_prev_noun = false;
            }

            if postag_matches(tokens[i - j], &p.det_cs) {
                if postag_matches(tokens[i - j + 1], &p.nom_ms) {
                    c_dms[level] += 1;
                    can_be_ms = true;
                }
                if postag_matches(tokens[i - j + 1], &p.nom_fs) {
                    c_dfs[level] += 1;
                    can_be_fs = true;
                }
            }
            if !postag_matches(tokens[i - j], &p.adverbi) {
                // exception: tot el
                if !(tokens[i - j].has_lemma("tot") && tokens[i - j + 1].has_lemma("el")) {
                    if postag_matches(tokens[i - j], &p.det_ms) {
                        c_dms[level] += 1;
                        can_be_ms = true;
                    }
                    if postag_matches(tokens[i - j], &p.det_fs) {
                        c_dfs[level] += 1;
                        can_be_fs = true;
                    }
                    if postag_matches(tokens[i - j], &p.det_mp) {
                        c_dmp[level] += 1;
                        can_be_mp = true;
                    }
                    if postag_matches(tokens[i - j], &p.det_fp) {
                        c_dfp[level] += 1;
                        can_be_fp = true;
                    }
                }
            }
            if i - j - 1 > 0
                && p.preposicio_canvi_nivell.is_match(tokens[i - j].surface())
                && !postag_matches(tokens[i - j], &p.conjuncio) // "com" com a conjunció
                && !p.coordinacio_ioni.is_match(tokens[i - j - 1].surface())
                && !postag_matches(tokens[i - j + 1], &p.adverbi)
            {
                level += 1;
            }
            j = Self::update_j_value(&p, &tokens, i, j, level);
            Self::update_apparitions(&p, &mut app, tokens[i - j]);
            j += 1;
        }
        level += 1;
        if level > MAX_LEVELS {
            level = MAX_LEVELS;
        }
        let mut c_n = [0i32; MAX_LEVELS];
        let mut c_d = [0i32; MAX_LEVELS];
        let mut c_ntotal = 0i32;
        let mut c_dtotal = 0i32;
        let mut jj = 0usize;
        while jj < level {
            c_n[jj] =
                c_nms[jj] + c_nfs[jj] + c_nmp[jj] + c_nfp[jj] + c_ncs[jj] + c_ncp[jj] + c_nmn[jj];
            c_d[jj] = c_dms[jj] + c_dfs[jj] + c_dmp[jj] + c_dfp[jj];
            c_ntotal += c_n[jj];
            c_dtotal += c_d[jj];
            // exceptions: adjective is plural and there are several nouns before
            if postag_matches(tokens[i], &p.adjectiu_mp)
                && (c_n[jj] > 1 || c_d[jj] > 1)
                && (c_nms[jj]
                    + c_nmn[jj]
                    + c_nmp[jj]
                    + c_ncs[jj]
                    + c_ncp[jj]
                    + c_dms[jj]
                    + c_dmp[jj])
                    > 0
                && (c_nfs[jj] + c_nfp[jj] <= c_nt[jj])
            {
                return FilterOutcome::reject();
            }
            if postag_matches(tokens[i], &p.adjectiu_fp)
                && (c_n[jj] > 1 || c_d[jj] > 1)
                && ((c_nms[jj] + c_nmp[jj] + c_nmn[jj] + c_dms[jj] + c_dmp[jj]) == 0
                    || (c_nt[jj] > 0 && c_nfs[jj] + c_nfp[jj] >= c_nt[jj]))
            {
                return FilterOutcome::reject();
            }
            // Adjective can't be singular
            if c_n[jj] + c_d[jj] > 0 {
                is_plural = is_plural && c_d[jj] > 1;
                can_be_p = can_be_p || c_n[jj] > 1;
            }
            jj += 1;
        }
        // comma + plural noun
        is_plural = is_plural
            || (i - 2 > 0 && c_nmp[0] + c_nfp[0] + c_ncp[0] > 0 && tokens[i - 2].surface() == ",");

        // there is no noun, (no determinant --> && cDtotal==0)
        if c_ntotal == 0 && c_dtotal == 0 {
            return FilterOutcome::reject();
        }

        // patterns according to the analyzed adjective
        let mut subst_pattern: Option<&(regex::Regex, regex::Regex, regex::Regex)> = None;
        if postag_matches(tokens[i], &p.adjectiu_cs) {
            subst_pattern = Some(&p.subst[0]);
        } else if postag_matches(tokens[i], &p.adjectiu_cp) {
            subst_pattern = Some(&p.subst[1]);
        } else if postag_matches(tokens[i], &p.adjectiu_ms) {
            subst_pattern = Some(&p.subst[2]);
        } else if postag_matches(tokens[i], &p.adjectiu_fs) {
            subst_pattern = Some(&p.subst[3]);
        } else if postag_matches(tokens[i], &p.adjectiu_mp) {
            subst_pattern = Some(&p.subst[4]);
        } else if postag_matches(tokens[i], &p.adjectiu_fp) {
            subst_pattern = Some(&p.subst[5]);
        }
        let Some((subst, gn_pattern, adj_pattern)) = subst_pattern else {
            return FilterOutcome::reject();
        };
        let subst_pattern = subst.clone();
        let gn_pattern = gn_pattern.clone();
        let adj_pattern = adj_pattern.clone();

        // combinations Det/Nom + adv (1,2..) + adj.
        // If there is agreement, the rule doesn't match
        let mut j = 1usize;
        let mut keep_count = true;
        while i > j && keep_count {
            if (postag_matches(tokens[i - j], &p.nom_det)
                && postag_matches(tokens[i - j], &gn_pattern))
                || (!postag_matches(tokens[i - j], &p.gn_any)
                    && postag_matches(tokens[i - j], &subst_pattern))
            {
                return FilterOutcome::reject();
            }
            keep_count = !postag_matches(tokens[i - j], &p.nom_det);
            j += 1;
        }

        // Necessary condition: previous token is a non-agreeing noun
        // or it is adjective or adverb (not preceded by verb)
        let necessary = (postag_matches(tokens[i - 1], &p.nom)
            && !postag_matches(tokens[i - 1], &subst_pattern))
            || (postag_matches(tokens[i - 1], &p.adjectiu)
                && !postag_matches(tokens[i - 1], &gn_pattern))
            || (postag_matches(tokens[i - 1], &p.adjectiu)
                && !postag_matches(tokens[i - 1], &adj_pattern))
            || (i > 2
                && postag_matches(tokens[i - 1], &p.adverbis_acceptats)
                && !postag_matches(tokens[i - 2], &p.verb)
                && !postag_matches(tokens[i - 2], &p.preposicions))
            || (i > 3
                && postag_matches(tokens[i - 1], &p.loc_adv)
                && postag_matches(tokens[i - 2], &p.loc_adv)
                && !postag_matches(tokens[i - 3], &p.verb)
                && !postag_matches(tokens[i - 3], &p.preposicions));
        if !necessary {
            return FilterOutcome::reject();
        }

        // Adjective can't be singular. The rule matches
        if !(is_plural && postag_matches(tokens[i], &p.adjectiu_s)) {
            // look into previous words
            let mut j = 1usize;
            let mut app = PaApparitions {
                adverb: false,
                conjunction: false,
                punctuation: false,
            };
            while i > j && Self::keep_counting(&p, &app, tokens[i - j]) {
                if (!postag_matches(tokens[i - j], &p.gn_any)
                    && postag_matches(tokens[i - j], &p.nom_det)
                    && postag_matches(tokens[i - j], &subst_pattern))
                    || postag_matches(tokens[i - j], &gn_pattern)
                {
                    return FilterOutcome::reject();
                }
                j = Self::update_j_value(&p, &tokens, i, j, 0);
                Self::update_apparitions(&p, &mut app, tokens[i - j]);
                j += 1;
            }
        }

        // The rule matches. Synthesize suggestions.
        let synth = self.env.synth.inner();
        let mut suggestions: Vec<String> = Vec::new();
        if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu_cs) {
            suggestions.extend(synth.synthesize(at, "A..CP.", true));
        }
        if suggestions.is_empty() {
            if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu_cp) {
                suggestions.extend(synth.synthesize(at, "A..CS.", true));
            }
        }
        if suggestions.is_empty() && is_plural {
            if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu_p) {
                suggestions.extend(synth.synthesize(at, "A...P.|V.P..P..|PX..P.*", true));
            }
        }
        if let Some(at) = get_analyzed_token(tokens[i], &p.adjectiu) {
            if suggestions.is_empty() {
                if can_be_ms && !is_plural {
                    suggestions.extend(synth.synthesize(at, "A..MS.|V.P..SM.|PX.MS.*", true));
                }
                if can_be_fs && !is_plural {
                    suggestions.extend(synth.synthesize(at, "A..FS.|V.P..SF.|PX.FS.*", true));
                }
                if can_be_mp {
                    suggestions.extend(synth.synthesize(at, "A..MP.|V.P..PM.|PX.MP.*", true));
                }
                if can_be_fp {
                    suggestions.extend(synth.synthesize(at, "A..FP.|V.P..PF.|PX.FP.*", true));
                }
                if can_be_ms && (is_plural || can_be_p) {
                    suggestions.extend(synth.synthesize(at, "A..MP.|V.P..PM.|PX.MP.*", true));
                }
                if can_be_fs && !can_be_ms && (is_plural || can_be_p) {
                    suggestions.extend(synth.synthesize(at, "A..FP.|V.P..PF.|PX.FP.*", true));
                }
            }
        }
        // avoid the original token as suggestion
        let lower = tokens[i].surface().to_lowercase();
        suggestions.retain(|s| *s != lower);
        if suggestions.is_empty() {
            return FilterOutcome::reject();
        }
        let mut definitive: Vec<String> = Vec::new();
        let mut range = None;
        if add_comma {
            definitive.push(format!(", {}", tokens[i].surface()));
            for s in &suggestions {
                definitive.push(format!(" {s}"));
            }
            let start = tokens[i].start_pos.saturating_sub(1);
            range = Some(TextRange::new(start, tokens[i].end_pos()));
        } else {
            definitive.extend(suggestions);
        }
        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<Suggestion> = Vec::new();
        for value in definitive {
            if !seen.contains(&value) {
                seen.push(value.clone());
                out.push(Suggestion {
                    value,
                    short_description: None,
                });
            }
        }
        FilterOutcome {
            accepted: true,
            range,
            message: None,
            suggestions: Some(out),
        }
    }
}

/// `org.languagetool.rules.ca.PostponedAdjectiveConcordanceFilter`.
pub struct PostponedAdjectiveConcordanceFilter {
    pub(crate) env: Env,
}
