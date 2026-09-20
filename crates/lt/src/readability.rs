//! Port of `ReadabilityRule` (`READABILITY_RULE_DIFFICULT` /
//! `READABILITY_RULE_SIMPLE`, both default off): Flesch reading ease per
//! paragraph, for English and German (`GermanReadabilityRule` overrides the
//! constants, formula, syllable counter and messages).

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, TextRange};

const RULE_ID_DIFFICULT: &str = "READABILITY_RULE_DIFFICULT";
const RULE_ID_SIMPLE: &str = "READABILITY_RULE_SIMPLE";
const RULE_ID_DIFFICULT_DE: &str = "READABILITY_RULE_DIFFICULT_DE";
const RULE_ID_SIMPLE_DE: &str = "READABILITY_RULE_SIMPLE_DE";
const RULE_ID_DIFFICULT_PT: &str = "READABILITY_RULE_DIFFICULT_PT";
const RULE_ID_SIMPLE_PT: &str = "READABILITY_RULE_SIMPLE_PT";

/// Language-specific parts of `ReadabilityRule` (`English` base vs
/// `GermanReadabilityRule`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReadabilityLanguage {
    En,
    De,
    Pt,
}
const MARK_WORDS: usize = 3;
const MIN_WORDS: usize = 10;
/// `ReadabilityRule.level` default (no `UserConfig`)
const DEFAULT_LEVEL: i32 = 3;

/// `AnalyzedTokenReadings.NON_WORD_REGEX` (a single non-word character).
fn non_word_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"^[.?!…:;,~’'"„“”»«‚‘›‹()\[\]\-–—*×∗·+÷/=]$"#).unwrap()
    });
    &RE
}

fn is_non_word(token: &AnalyzedTokenReadings) -> bool {
    non_word_re().is_match(token.surface())
}

fn is_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'A' | 'E' | 'I' | 'O' | 'U' | 'Y'
    )
}

/// `GermanTools.isVowel` (used by the German syllable counter).
fn is_german_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e'
            | 'i'
            | 'o'
            | 'u'
            | 'y'
            | 'A'
            | 'E'
            | 'I'
            | 'O'
            | 'U'
            | 'Y'
            | 'ä'
            | 'ö'
            | 'ü'
            | 'Ä'
            | 'Ö'
            | 'Ü'
    )
}

/// `GermanReadabilityRule.simpleSyllablesCount`.
fn simple_syllables_count_de(word: &str) -> i32 {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return 0;
    }
    let mut n_syllables = 0i32;
    if is_german_vowel(chars[0]) {
        n_syllables += 1;
    }
    let mut last_double = false;
    for i in 1..chars.len() {
        let c = chars[i];
        if is_german_vowel(c) {
            let cl = chars[i - 1];
            if last_double {
                n_syllables += 1;
                last_double = false;
            } else if ((c == 'i' || c == 'y') && (cl == 'a' || cl == 'e' || cl == 'A' || cl == 'E'))
                || (c == 'u'
                    && (cl == 'a' || cl == 'e' || cl == 'o' || cl == 'A' || cl == 'E' || cl == 'O'))
                || (c == 'e' && (cl == 'e' || cl == 'i' || cl == 'E' || cl == 'I'))
                || (c == 'a' && (cl == 'a' || cl == 'A'))
            {
                last_double = true;
            } else {
                n_syllables += 1;
                last_double = false;
            }
        } else {
            last_double = false;
        }
    }
    if n_syllables == 0 {
        1
    } else {
        n_syllables
    }
}

/// `PortugueseReadabilityRule.isVowel`.
fn is_portuguese_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e'
            | 'i'
            | 'o'
            | 'u'
            | 'y'
            | 'A'
            | 'E'
            | 'I'
            | 'O'
            | 'U'
            | 'Y'
            | 'á'
            | 'é'
            | 'í'
            | 'ó'
            | 'ú'
            | 'à'
            | 'Á'
            | 'É'
            | 'Í'
            | 'Ó'
            | 'Ú'
            | 'À'
            | 'â'
            | 'ê'
            | 'ô'
            | 'Â'
            | 'Ê'
            | 'Ô'
            | 'Ü'
    )
}

/// `PortugueseReadabilityRule.simpleSyllablesCount`.
fn simple_syllables_count_pt(word: &str) -> i32 {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return 0;
    }
    let mut n_syllables = 0i32;
    if is_portuguese_vowel(chars[0]) {
        n_syllables += 1;
    }
    let mut last_double = false;
    for i in 1..chars.len() {
        let c = chars[i];
        if is_portuguese_vowel(c) {
            let cl = chars[i - 1];
            if last_double {
                n_syllables += 1;
                last_double = false;
            } else if ((c == 'ã' || c == 'õ') && (cl == 'e' || cl == 'o'))
                || (c == 'a' && matches!(cl, 'e' | 'i' | 'í' | 'o' | 'u' | 'ú'))
                || (c == 'e' && matches!(cl, 'e' | 'i' | 'í' | 'o' | 'a' | 'u'))
                || (c == 'i' && matches!(cl, 'a' | 'e' | 'o' | 'u' | 'á' | 'é'))
                || (c == 'í' && (cl == 'a' || cl == 'e'))
                || (c == 'o' && matches!(cl, 'a' | 'á' | 'e' | 'é' | 'i' | 'í' | 'u'))
                || (c == 'u' && matches!(cl, 'a' | 'á' | 'e' | 'é' | 'i' | 'o'))
                || (c == 'ú' && (cl == 'a' || cl == 'e' || cl == 'o'))
            {
                last_double = true;
            } else {
                n_syllables += 1;
                last_double = false;
            }
        } else {
            last_double = false;
        }
    }
    if n_syllables == 0 {
        1
    } else {
        n_syllables
    }
}

/// `ReadabilityRule.simpleSyllablesCount` (char-based, Java `charAt`).
fn simple_syllables_count(word: &str) -> i32 {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return 0;
    }
    if chars.len() == 1 {
        return 1;
    }
    let mut n_syllables = 0i32;
    let mut last_double = false;
    for i in 0..chars.len() - 1 {
        let c = chars[i];
        if is_vowel(c) {
            let cn = chars[i + 1];
            if last_double {
                n_syllables += 1;
                last_double = false;
            } else if ((c == 'e' || c == 'E')
                && (cn == 'a' || cn == 'o' || cn == 'e' || cn == 'i' || cn == 'y'))
                || ((c == 'a' || c == 'A') && (cn == 'e' || cn == 'i' || cn == 'u'))
                || ((c == 'o' || c == 'O') && (cn == 'o' || cn == 'i' || cn == 'u' || cn == 'a'))
                || ((c == 'u' || c == 'U') && (cn == 'i' || cn == 'a'))
                || ((c == 'i' || c == 'I') && (cn == 'e' || cn == 'o'))
            {
                last_double = true;
            } else {
                n_syllables += 1;
                last_double = false;
            }
        } else {
            last_double = false;
        }
    }
    let c = chars[chars.len() - 1];
    let cl = chars[chars.len() - 2];
    if (cl == 'e' && (c == 's' || c == 'd')) || (cl == 'u' && c == 'e') {
        n_syllables -= 1;
    } else if is_vowel(c) && c != 'e' {
        n_syllables += 1;
    }
    if n_syllables <= 0 {
        1
    } else {
        n_syllables
    }
}

fn readability_level(fre: f64) -> i32 {
    if fre < 30.0 {
        0
    } else if fre < 50.0 {
        1
    } else if fre < 60.0 {
        2
    } else if fre < 70.0 {
        3
    } else if fre < 80.0 {
        4
    } else if fre < 90.0 {
        5
    } else {
        6
    }
}

fn flesch_reading_ease(lang: ReadabilityLanguage, asl: f64, asw: f64) -> f64 {
    match lang {
        ReadabilityLanguage::En => 206.835 - (1.015 * asl) - (84.6 * asw),
        ReadabilityLanguage::De => 180.0 - asl - (58.5 * asw),
        // PortugueseReadabilityRule.getFleschReadingEase
        ReadabilityLanguage::Pt => 206.84 - (1.02 * asl) - (60.0 * asw),
    }
}

fn print_message_level(lang: ReadabilityLanguage, level: i32) -> String {
    let name = match (lang, level) {
        (ReadabilityLanguage::En, 0) => Some("Very difficult"),
        (ReadabilityLanguage::En, 1) => Some("Difficult"),
        (ReadabilityLanguage::En, 2) => Some("Fairly difficult"),
        (ReadabilityLanguage::En, 3) => Some("Medium"),
        (ReadabilityLanguage::En, 4) => Some("Fairly easy"),
        (ReadabilityLanguage::En, 5) => Some("Easy"),
        (ReadabilityLanguage::En, 6) => Some("Very easy"),
        (ReadabilityLanguage::De, 0) => Some("Sehr schwer"),
        (ReadabilityLanguage::De, 1) => Some("Schwer"),
        (ReadabilityLanguage::De, 2) => Some("Mittelschwer"),
        (ReadabilityLanguage::De, 3) => Some("Mittel"),
        (ReadabilityLanguage::De, 4) => Some("Mittelleicht"),
        (ReadabilityLanguage::De, 5) => Some("Leicht"),
        (ReadabilityLanguage::De, 6) => Some("Sehr leicht"),
        (ReadabilityLanguage::Pt, 0) => Some("Muito complexo"),
        (ReadabilityLanguage::Pt, 1) => Some("Complexo"),
        (ReadabilityLanguage::Pt, 2) => Some("Moderadamente complexo"),
        (ReadabilityLanguage::Pt, 3) => Some("Meio-termo"),
        (ReadabilityLanguage::Pt, 4) => Some("Moderadamente simples"),
        (ReadabilityLanguage::Pt, 5) => Some("Simples"),
        (ReadabilityLanguage::Pt, 6) => Some("Muito simples"),
        _ => None,
    };
    match name {
        Some(name) => match lang {
            ReadabilityLanguage::En => format!(" {{Level {level}: {name}}}"),
            ReadabilityLanguage::De => format!(" {{Grad {level}: {name}}}"),
            ReadabilityLanguage::Pt => format!(" {{Nível {level}: {name}}}"),
        },
        None => String::new(),
    }
}

fn message(
    lang: ReadabilityLanguage,
    too_easy_test: bool,
    level: i32,
    fre: i32,
    asl: i32,
    asw: i32,
) -> String {
    match lang {
        ReadabilityLanguage::En => {
            let (simple, few) = if too_easy_test {
                ("simple", "few")
            } else {
                ("difficult", "many")
            };
            format!(
                "Readability: The text of this paragraph is too {simple}{}. Too {few} words per sentence and too {few} syllables per word.",
                print_message_level(lang, level)
            )
        }
        ReadabilityLanguage::De => {
            let (simple, few) = if too_easy_test {
                ("einfach", "wenige")
            } else {
                ("schwierig", "viele")
            };
            format!(
                "Lesbarkeit: Der Text dieses Absatzes ist zu {simple}{}. Zu {few} Wörter pro Satz und zu {few} Silben pro Wort.",
                print_message_level(lang, level)
            )
        }
        ReadabilityLanguage::Pt => {
            let (simple, few) = if too_easy_test {
                ("fácil", "poucas")
            } else {
                ("difícil", "muitas")
            };
            format!(
                "Legibilidade {{FRE: {fre}, ASL: {asl}, ASW: {asw}}}: O texto deste parágrafo é {simple}{}. Tem {few} palavras por frase e {few} sílabas por palavra.",
                print_message_level(lang, level)
            )
        }
    }
}

/// One `ReadabilityRule` instance (`tooEasyTest` selects the variant).
pub struct ReadabilityRule {
    pub too_easy_test: bool,
    pub level: i32,
    pub lang: ReadabilityLanguage,
}

impl ReadabilityRule {
    pub fn difficult() -> Self {
        Self {
            too_easy_test: false,
            level: DEFAULT_LEVEL,
            lang: ReadabilityLanguage::En,
        }
    }

    pub fn simple() -> Self {
        Self {
            too_easy_test: true,
            level: DEFAULT_LEVEL,
            lang: ReadabilityLanguage::En,
        }
    }

    /// `new GermanReadabilityRule(messages, lang, userConfig, false)`
    pub fn german_difficult() -> Self {
        Self {
            too_easy_test: false,
            level: DEFAULT_LEVEL,
            lang: ReadabilityLanguage::De,
        }
    }

    /// `new GermanReadabilityRule(messages, lang, userConfig, true)`
    pub fn german_simple() -> Self {
        Self {
            too_easy_test: true,
            level: DEFAULT_LEVEL,
            lang: ReadabilityLanguage::De,
        }
    }

    /// `new PortugueseReadabilityRule(messages, lang, userConfig, false)`
    pub fn portuguese_difficult() -> Self {
        Self {
            too_easy_test: false,
            level: DEFAULT_LEVEL,
            lang: ReadabilityLanguage::Pt,
        }
    }

    /// `new PortugueseReadabilityRule(messages, lang, userConfig, true)`
    pub fn portuguese_simple() -> Self {
        Self {
            too_easy_test: true,
            level: DEFAULT_LEVEL,
            lang: ReadabilityLanguage::Pt,
        }
    }

    pub fn id(&self) -> &'static str {
        match (self.lang, self.too_easy_test) {
            (ReadabilityLanguage::En, true) => RULE_ID_SIMPLE,
            (ReadabilityLanguage::En, false) => RULE_ID_DIFFICULT,
            (ReadabilityLanguage::De, true) => RULE_ID_SIMPLE_DE,
            (ReadabilityLanguage::De, false) => RULE_ID_DIFFICULT_DE,
            (ReadabilityLanguage::Pt, true) => RULE_ID_SIMPLE_PT,
            (ReadabilityLanguage::Pt, false) => RULE_ID_DIFFICULT_PT,
        }
    }

    pub fn description(&self) -> &'static str {
        match (self.lang, self.too_easy_test) {
            (ReadabilityLanguage::En, true) => "Readability: Too easy text",
            (ReadabilityLanguage::En, false) => "Readability: Too difficult text",
            (ReadabilityLanguage::De, true) => "Lesbarkeit: Zu einfacher Text",
            (ReadabilityLanguage::De, false) => "Lesbarkeit: Zu schwieriger Text",
            (ReadabilityLanguage::Pt, true) => "Legibilidade: texto demasiado simples",
            (ReadabilityLanguage::Pt, false) => "Legibilidade: texto demasiado complexo",
        }
    }

    fn is_too(&self, level: i32) -> bool {
        if self.too_easy_test {
            level > self.level
        } else {
            level < self.level
        }
    }

    /// `ReadabilityRule.match(List<AnalyzedSentence>)`.
    pub fn check(&self, sentences: &[AnalyzedSentence]) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let mut n_paragraph = 0i32;
        let mut n_all_sentences = 0i32;
        let mut n_all_words = 0i32;
        let mut n_all_syllables = 0i32;
        let mut n_sentences = 0i32;
        let mut n_words = 0i32;
        let mut n_syllables = 0i32;
        let mut pos = 0usize;
        let mut start_pos: i64 = -1;
        let mut end_pos: i64 = -1;
        for (n, sentence) in sentences.iter().enumerate() {
            let tokens = sentence.tokens_without_whitespace();
            if start_pos < 0 && tokens.len() > 1 {
                start_pos = (pos + tokens[1].start_pos) as i64;
            }
            if end_pos < 0 && tokens.len() > MARK_WORDS {
                end_pos = (pos + tokens[MARK_WORDS].end_pos()) as i64;
            }
            n_sentences += 1;
            for token in &tokens {
                let s_token = token.surface();
                // Java's `AnalyzedTokenReadings.isWhitespace` is true for the
                // empty SENT_START token (empty string is "whitespace")
                if !token.is_whitespace && !token.is_sentence_start && !is_non_word(token) {
                    n_words += 1;
                    n_syllables += match self.lang {
                        ReadabilityLanguage::En => simple_syllables_count(s_token),
                        ReadabilityLanguage::De => simple_syllables_count_de(s_token),
                        ReadabilityLanguage::Pt => simple_syllables_count_pt(s_token),
                    };
                }
            }
            if crate::paragraph::is_paragraph_end(sentences, n) {
                if n_words >= MIN_WORDS as i32 {
                    let asl = f64::from(n_words) / f64::from(n_sentences);
                    let asw = f64::from(n_syllables) / f64::from(n_words);
                    let fre = flesch_reading_ease(self.lang, asl, asw);
                    let r_level = readability_level(fre);
                    if self.is_too(r_level) {
                        let msg = message(
                            self.lang,
                            self.too_easy_test,
                            r_level,
                            fre as i32,
                            asl as i32,
                            asw as i32,
                        );
                        if start_pos >= 0 && end_pos > start_pos {
                            rule_matches.push(self.match_abs(
                                start_pos as usize,
                                end_pos as usize,
                                msg,
                            ));
                        }
                    }
                }
                n_all_sentences += n_sentences;
                n_all_words += n_words;
                n_all_syllables += n_syllables;
                n_sentences = 0;
                n_words = 0;
                n_syllables = 0;
                start_pos = -1;
                end_pos = -1;
                n_paragraph += 1;
            }
            pos += sentence.text.len();
        }
        let asl = if n_all_sentences == 0 {
            0.0
        } else {
            f64::from(n_all_words) / f64::from(n_all_sentences)
        };
        let asw = if n_all_words == 0 {
            0.0
        } else {
            f64::from(n_all_syllables) / f64::from(n_all_words)
        };
        let fre = flesch_reading_ease(self.lang, asl, asw);
        let r_level = readability_level(fre);
        // Java precedence: `nParagraph > 1 && tooEasy && rLevel > level
        // || !tooEasy && rLevel < level`
        let keep = (n_paragraph > 1 && self.too_easy_test && r_level > self.level)
            || (!self.too_easy_test && r_level < self.level);
        if keep {
            rule_matches
        } else {
            Vec::new()
        }
    }

    fn match_abs(&self, start: usize, end: usize, message: String) -> Match {
        let (category_id, category_name) = match self.lang {
            ReadabilityLanguage::En => ("TEXT_ANALYSIS", "Text Analysis"),
            ReadabilityLanguage::De => {
                ("CREATIVE_WRITING", "Stiltipps f\u{fc}r kreatives Schreiben")
            }
            ReadabilityLanguage::Pt => ("TEXT_ANALYSIS", "Análise de Texto"),
        };
        Match::new(
            self.id(),
            Option::<String>::None,
            message,
            Option::<String>::None,
            TextRange::new(start, end),
            Vec::new(),
            category_id,
            category_name,
        )
        .with_metadata(self.description(), "style", -1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_syllables_like_java() {
        assert_eq!(simple_syllables_count(""), 0);
        assert_eq!(simple_syllables_count("a"), 1);
        assert_eq!(simple_syllables_count("cat"), 1);
        assert_eq!(simple_syllables_count("mat"), 1);
        assert_eq!(simple_syllables_count("hello"), 2);
        // Java `ReadabilityRule.simpleSyllablesCount` values (Docker probe)
        assert_eq!(simple_syllables_count("incomprehensibility"), 8);
        assert_eq!(simple_syllables_count("internationalization"), 8);
    }

    #[test]
    fn maps_readability_levels() {
        assert_eq!(readability_level(20.0), 0);
        assert_eq!(readability_level(45.0), 1);
        assert_eq!(readability_level(55.0), 2);
        assert_eq!(readability_level(65.0), 3);
        assert_eq!(readability_level(75.0), 4);
        assert_eq!(readability_level(85.0), 5);
        assert_eq!(readability_level(95.0), 6);
    }
}
