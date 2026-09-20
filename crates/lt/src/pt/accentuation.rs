//! `PortugueseAccentuationCheckRule` (`ACCENTUATION_CHECK_PT`, default
//! off): a word without accent that is tagged as a verb but reads as an
//! accented noun/adjective in the given context. Port of the Java
//! `matchPostagRegexp` POS-pattern logic and the data loader.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use lt_core::{AnalyzedTokenReadings, Match, TextRange};

pub const RULE_ID: &str = "ACCENTUATION_CHECK_PT";
const DESCRIPTION: &str = "Confusão com acentos gráficos";
const MESSAGE: &str = "Se é um nome ou um adjectivo, tem acento.";
const SHORT_MESSAGE: &str = "Falta um acento";
const CATEGORY_ID: &str = "CONFUSED_WORDS";

struct AccentedWord {
    token: String,
    postag: String,
}

fn re(s: &str) -> &'static regex::Regex {
    // The Java patterns are small; cache them in a lazy registry.
    static REGISTRY: LazyLock<std::sync::Mutex<HashMap<String, &'static regex::Regex>>> =
        LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = REGISTRY.lock().expect("regex registry");
    if let Some(found) = guard.get(s) {
        return found;
    }
    let compiled: &'static regex::Regex =
        Box::leak(Box::new(regex::Regex::new(s).expect("valid pattern")));
    guard.insert(s.to_string(), compiled);
    compiled
}

/// Java `matcher.matches()`: full match.
fn full(pattern: &str, s: &str) -> bool {
    re(&format!("^(?:{pattern})$")).is_match(s)
}

fn match_postag(token: &AnalyzedTokenReadings, pattern: &str) -> bool {
    let full = re(&format!("^(?:{pattern})$"));
    token
        .readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|tag| full.is_match(tag)))
}

fn has_pos_tag(token: &AnalyzedTokenReadings, tag: &str) -> bool {
    token.has_pos_tag(tag)
}

const PREPOSICAO_DE: &str = "de|d[a|o]s?";
const ARTIGO_O_MS: &str = "o|O";
const ARTIGO_O_FS: &str = "a|A";
const ARTIGO_O_MP: &str = "as|As";
const ARTIGO_O_FP: &str = "os|Os";
const DETERMINANTE: &str = "D[^R].*";
const DETERMINANTE_MS: &str = "D[^R].[MC][SN].*";
const DETERMINANTE_FS: &str = "D[^R].[FC][SN].*";
const DETERMINANTE_MP: &str = "D[^R].[MC][PN].*";
const DETERMINANTE_FP: &str = "D[^R].[FC][PN].*";
const NOME_MS: &str = "NC[MC][SN].*";
const NOME_FS: &str = "NC[FC][SN].*";
const NOME_MP: &str = "NC[MC][PN].*";
const NOME_FP: &str = "NC[FC][PN].*";
const ADJETIVO_MS: &str = "A..[MC][SN].*|V.P..SM.?|PX.MS.*";
const ADJETIVO_FS: &str = "A..[FC][SN].*|V.P..SF.?|PX.FS.*";
const ADJETIVO_MP: &str = "A..[MC][PN].*|V.P..PM.?|PX.MP.*";
const ADJETIVO_FP: &str = "A..[FC][PN].*|V.P..PF.?|PX.FP.*";
const INFINITIVO: &str = "V.N.*";
const VERBO_CONJUGADO: &str = "V.[^NGP].*|_GV_";
const PARTICIPIO_MS: &str = "V.P.*SM.?";
const GRUPO_VERBAL: &str = "_GV_";
const VERBO_3S: &str = "V...3S..?";
const NOT_IN_PREV_TOKEN: &str = "V..*|PP.*|P0.*|V.P.*";
const BEFORE_ADJECTIVE_MS: &str = "SPS00|D[^R].[MC][SN].*|V.[^NGP].*|PX.*";
const BEFORE_ADJECTIVE_FS: &str = "SPS00|D[^R].[FC][SN].*|V.[^NGP].*|PX.*";
const BEFORE_ADJECTIVE_MP: &str = "SPS00|D[^R].[MC][PN].*|V.[^NGP].*|PX.*";
const BEFORE_ADJECTIVE_FP: &str = "SPS00|D[^R].[FC][PN].*|V.[^NGP].*|PX.*";
const GN: &str = ".*_GN_.*|<?/?N[CP].*";
const EXCEPCOES_ANTES_DE: &str = "(?i)forma|manera|por|costat";
const PRONOME_PESSOAL: &str = "P0.{6}|PP3CN000|PP3NN000|PP3CP000|PP3CSD00";

pub struct AccentuationCheckRule {
    relevant_words: HashMap<String, AccentedWord>,
    relevant_words2: HashMap<String, AccentedWord>,
}

impl AccentuationCheckRule {
    pub fn load(data_dir: &Path) -> Self {
        Self {
            relevant_words: load_words(
                &data_dir.join("pt/rules/verbos_sem_acento_nomes_com_acento.txt"),
            ),
            relevant_words2: load_words(
                &data_dir.join("pt/rules/verbos_sem_acento_adj_com_acento.txt"),
            ),
        }
    }

    /// `PortugueseAccentuationCheckRule.match` over the tokens of one
    /// sentence (SENT_START/whitespace filtered like Java's
    /// `getTokensWithoutWhitespace`).
    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let view: Vec<&AnalyzedTokenReadings> = tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut rule_matches = Vec::new();
        for i in 1..view.len() {
            let token = if i == 1 {
                view[i].surface().to_lowercase()
            } else {
                view[i].surface().to_string()
            };
            let prev_token = view[i - 1].surface();
            let prev_prev_token = if i > 2 { view[i - 2].surface() } else { "" };
            let next_token = if i < view.len() - 1 {
                view[i + 1].surface()
            } else {
                ""
            };
            let next_next_token = if i < view.len() - 2 {
                view[i + 2].surface()
            } else {
                ""
            };
            if token.is_empty() {
                continue;
            }
            let is_relevant_word = self.relevant_words.contains_key(&token);
            let is_relevant_word2 = self.relevant_words2.contains_key(&token);
            if !is_relevant_word && !is_relevant_word2 {
                continue;
            }
            // verbo precedido de pronome reflexo
            if match_postag(view[i - 1], PRONOME_PESSOAL) && !prev_token.starts_with('-') {
                continue;
            }

            let mut replacement: Option<String> = None;
            if is_relevant_word && !match_postag(view[i], GN) {
                replacement = self.noun_replacement(
                    &view,
                    i,
                    &self.relevant_words[&token],
                    next_token,
                    next_next_token,
                );
            }
            if is_relevant_word2 && !match_postag(view[i], GN) {
                replacement = self.adj_replacement(
                    &view,
                    i,
                    &self.relevant_words2[&token],
                    prev_token,
                    prev_prev_token,
                );
            }

            if let Some(replacement) = replacement {
                rule_matches.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        MESSAGE,
                        Some(SHORT_MESSAGE.to_string()),
                        TextRange::new(
                            sentence_offset + view[i].start_pos,
                            sentence_offset + view[i].end_pos(),
                        ),
                        vec![lt_core::Suggestion {
                            value: replacement,
                            short_description: None,
                        }],
                        CATEGORY_ID,
                        "Confusão de Palavras",
                    )
                    .with_metadata(DESCRIPTION, "misspelling", 0),
                );
            }
        }
        rule_matches
    }
    /// The `VERB WITHOUT ACCENT -> NOUN WITH ACCENT` else-if chain (Java
    /// evaluates the branches in order; the trailing "de positiva
    /// influencia" `if` can override the chain result).
    fn noun_replacement(
        &self,
        view: &[&AnalyzedTokenReadings],
        i: usize,
        rw: &AccentedWord,
        next_token: &str,
        next_next_token: &str,
    ) -> Option<String> {
        let prev = view[i - 1];
        let m_preposicao_de = full(PREPOSICAO_DE, next_token);
        let m_excepcoes_de = full(EXCEPCOES_ANTES_DE, next_next_token);
        let m_artigo_o_ms = full(ARTIGO_O_MS, prev.surface());
        let m_artigo_o_fs = full(ARTIGO_O_FS, prev.surface());
        let m_artigo_o_mp = full(ARTIGO_O_MP, prev.surface());
        let m_artigo_o_fp = full(ARTIGO_O_FP, prev.surface());

        let chain = has_pos_tag(prev, "SPS00")
            && !has_pos_tag(prev, "RG")
            && !match_postag(prev, DETERMINANTE)
            && !match_postag(view[i], INFINITIVO)
            || (match_postag(prev, DETERMINANTE_MS) && match_postag_word(rw, NOME_MS))
            || (match_postag(prev, DETERMINANTE_MP) && match_postag_word(rw, NOME_MP))
            || (match_postag(prev, DETERMINANTE_FS) && match_postag_word(rw, NOME_FS))
            || (match_postag(prev, DETERMINANTE_FP) && match_postag_word(rw, NOME_FP))
            || i > 2
                && match_postag(view[i - 2], VERBO_CONJUGADO)
                && ((match_postag(prev, DETERMINANTE_MS) && match_postag_word(rw, NOME_MS))
                    || (match_postag(prev, DETERMINANTE_MP) && match_postag_word(rw, NOME_MP))
                    || (match_postag(prev, DETERMINANTE_FS) && match_postag_word(rw, NOME_FS))
                    || (match_postag(prev, DETERMINANTE_FP) && match_postag_word(rw, NOME_FP)))
            || i > 2
                && match_postag(view[i - 2], VERBO_CONJUGADO)
                && ((m_artigo_o_ms && match_postag_word(rw, NOME_MS))
                    || (m_artigo_o_mp && match_postag_word(rw, NOME_MP))
                    || (m_artigo_o_fs && match_postag_word(rw, NOME_FS))
                    || (m_artigo_o_fp && match_postag_word(rw, NOME_FP)))
            || !match_postag(view[i], PARTICIPIO_MS)
                && !match_postag(prev, NOT_IN_PREV_TOKEN)
                && i < view.len() - 2
                && !match_postag(view[i + 2], INFINITIVO)
                && !m_excepcoes_de
                && !has_pos_tag(prev, "RG")
            || ((m_artigo_o_ms && match_postag_word(rw, NOME_MS))
                || (m_artigo_o_fs && match_postag_word(rw, NOME_FS))
                || (m_artigo_o_mp && match_postag_word(rw, NOME_MP))
                || (m_artigo_o_fp && match_postag_word(rw, NOME_FP)))
                && m_preposicao_de
            || i < view.len() - 1
                && ((match_postag_word(rw, NOME_MS) && match_postag(view[i + 1], ADJETIVO_MS))
                    || (match_postag_word(rw, NOME_FS) && match_postag(view[i + 1], ADJETIVO_FS))
                    || (match_postag_word(rw, NOME_MP) && match_postag(view[i + 1], ADJETIVO_MP))
                    || (match_postag_word(rw, NOME_FP) && match_postag(view[i + 1], ADJETIVO_FP)))
            || (match_postag_word(rw, NOME_MS)
                && match_postag(prev, ADJETIVO_MS)
                && !match_postag(view[i], VERBO_3S)
                && !match_postag(view[i], GRUPO_VERBAL))
            || (match_postag_word(rw, NOME_FS)
                && match_postag(prev, ADJETIVO_FS)
                && !match_postag(view[i], VERBO_3S))
            || (match_postag_word(rw, NOME_MP) && match_postag(prev, ADJETIVO_MP))
            || (match_postag_word(rw, NOME_FP) && match_postag(prev, ADJETIVO_FP))
            || next_token == "que"
                && i > 2
                && ((match_postag_word(rw, NOME_MS)
                    && match_postag(prev, ADJETIVO_MS)
                    && match_postag(view[i - 2], DETERMINANTE_MS))
                    || (match_postag_word(rw, NOME_FS)
                        && match_postag(prev, ADJETIVO_FS)
                        && match_postag(view[i - 2], DETERMINANTE_FS))
                    || (match_postag_word(rw, NOME_MP)
                        && match_postag(prev, ADJETIVO_MP)
                        && match_postag(view[i - 2], DETERMINANTE_MP))
                    || (match_postag_word(rw, NOME_FP)
                        && match_postag(prev, ADJETIVO_FP)
                        && match_postag(view[i - 2], DETERMINANTE_FP)))
            || next_token == "que"
                && ((m_artigo_o_ms && match_postag_word(rw, NOME_MS))
                    || (m_artigo_o_fs && match_postag_word(rw, NOME_FS))
                    || (m_artigo_o_mp && match_postag_word(rw, NOME_MP))
                    || (m_artigo_o_fp && match_postag_word(rw, NOME_FP)));
        // "de positiva influencia" (a separate `if` in Java)
        if i > 2
            && has_pos_tag(view[i - 2], "SPS00")
            && !has_pos_tag(view[i - 2], "RG")
            && ((match_postag_word(rw, NOME_MS) && match_postag(prev, ADJETIVO_MS))
                || (match_postag_word(rw, NOME_FS) && match_postag(prev, ADJETIVO_FS))
                || (match_postag_word(rw, NOME_MP) && match_postag(prev, ADJETIVO_MP))
                || (match_postag_word(rw, NOME_FP) && match_postag(prev, ADJETIVO_FP)))
        {
            return Some(rw.token.clone());
        }
        chain.then(|| rw.token.clone())
    }

    /// The `VERB WITHOUT ACCENT -> ADJECTIVE WITH ACCENT` else-if chain.
    fn adj_replacement(
        &self,
        view: &[&AnalyzedTokenReadings],
        i: usize,
        rw: &AccentedWord,
        prev_token: &str,
        prev_prev_token: &str,
    ) -> Option<String> {
        let prev = view[i - 1];
        let m_artigo_o_ms = full(ARTIGO_O_MS, prev_token);
        let m_artigo_o_fs = full(ARTIGO_O_FS, prev_token);
        let m_artigo_o_mp = full(ARTIGO_O_MP, prev_token);
        let m_artigo_o_fp = full(ARTIGO_O_FP, prev_token);
        let chain = (match_postag_word(rw, ADJETIVO_MS)
            && match_postag(prev, NOME_MS)
            && !has_pos_tag(prev, "_GN_FS")
            && match_postag(view[i], VERBO_CONJUGADO)
            && !match_postag(view[i], VERBO_3S))
            || (match_postag_word(rw, ADJETIVO_FS)
                && prev_prev_token.eq_ignore_ascii_case("de")
                && (prev_token == "maneira" || prev_token == "forma"))
            || (match_postag_word(rw, ADJETIVO_MP) && match_postag(prev, NOME_MP))
            || (match_postag_word(rw, ADJETIVO_FP) && match_postag(prev, NOME_FP))
            || i < view.len() - 1
                && prev_token != "que"
                && !match_postag(prev, NOT_IN_PREV_TOKEN)
                && ((match_postag_word(rw, ADJETIVO_MS)
                    && match_postag(view[i + 1], NOME_MS)
                    && match_postag(prev, BEFORE_ADJECTIVE_MS))
                    || (match_postag_word(rw, ADJETIVO_FS)
                        && match_postag(view[i + 1], NOME_FS)
                        && match_postag(prev, BEFORE_ADJECTIVE_FS))
                    || (match_postag_word(rw, ADJETIVO_MP)
                        && match_postag(view[i + 1], NOME_MP)
                        && match_postag(prev, BEFORE_ADJECTIVE_MP))
                    || (match_postag_word(rw, ADJETIVO_FP)
                        && match_postag(view[i + 1], NOME_FP)
                        && match_postag(prev, BEFORE_ADJECTIVE_FP)))
            || i < view.len() - 1
                && ((match_postag_word(rw, ADJETIVO_MS)
                    && match_postag(view[i + 1], NOME_MS)
                    && m_artigo_o_ms)
                    || (match_postag_word(rw, ADJETIVO_FS)
                        && match_postag(view[i + 1], NOME_FS)
                        && m_artigo_o_fs)
                    || (match_postag_word(rw, ADJETIVO_MP)
                        && match_postag(view[i + 1], NOME_MP)
                        && m_artigo_o_mp)
                    || (match_postag_word(rw, ADJETIVO_FP)
                        && match_postag(view[i + 1], NOME_FP)
                        && m_artigo_o_fp));
        chain.then(|| rw.token.clone())
    }
}

/// `matchPostagRegexp` over the data entry's stored POS tag.
fn match_postag_word(word: &AccentedWord, pattern: &str) -> bool {
    full(pattern, &word.postag)
}

fn load_words(path: &Path) -> HashMap<String, AccentedWord> {
    let mut map = HashMap::new();
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return map;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(';').collect();
        if parts.len() != 3 {
            continue;
        }
        map.insert(
            parts[0].to_string(),
            AccentedWord {
                token: parts[1].to_string(),
                postag: parts[2].to_string(),
            },
        );
    }
    map
}
