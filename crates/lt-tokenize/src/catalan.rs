//! `org.languagetool.tokenizers.ca.CatalanWordTokenizer`: the base
//! `WordTokenizer` with the Catalan apostrophe/hyphen handling, ela-geminada
//! placeholder protection, decimal point/comma and digit-space re-joins, the
//! `pronoms febles` patterns and the dictionary-aware hyphen splitting.

use std::sync::OnceLock;

use crate::english::IsTagged;
use crate::wordtokenizer::join_emails_and_urls;

const APOS_RECTE_SUBST: &str = "xxCA_APOS_RECTExx";
const APOS_RODO_SUBST: &str = "xxCA_APOS_RODOxx";
const HYPHEN_SUBST: &str = "xxCA_HYPHENxx";
const DECIMALPOINT_SUBST: &str = "xxCA_DECIMALPOINTxx";
const DECIMALCOMMA_SUBST: &str = "xxCA_DECIMALCOMMAxx";
const SPACE_SUBST: &str = "xxCA_SPACExx";
const ELA_SUBST: &str = "xxELA_GEMINADAxx";
const ELA_UPPER_SUBST: &str = "xxELA_GEMINADAUPPERCASExx";

/// `PF` (all possible forms of "pronoms febles" after a verb).
const PF: &str = "(['’]en|['’]hi|['’]ho|['’]l|['’]ls|['’]m|['’]n|['’]ns|['’]s|['’]t|-el|-els|-em|-en|-ens|-hi|-ho|-l|-la|-les|-li|-lo|-los|-m|-me|-n|-ne|-nos|-s|-se|-t|-te|-us|-vos)";

fn ela_geminada() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new("([aeiouàéèíóòúïüAEIOUÀÈÉÍÒÓÚÏÜ])l[\\.\u{2022}\u{22C5}\u{2219}\u{F0D7}]l([aeiouàéèíóòúïü])").unwrap()
    })
}

fn ela_geminada_uppercase() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            "([AEIOUÀÈÉÍÒÓÚÏÜ])L[\\.\u{2022}\u{22C5}\u{2219}\u{F0D7}]L([AEIOUÀÈÉÍÒÓÚÏÜ])",
        )
        .unwrap()
    })
}

fn apostrof_recte() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r#"(?i)(\p{L})'(\p{L}|"|‘|“|«)"#).unwrap())
}

fn apostrof_recte_1() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)([dl])'(\d[\d\s.,]?)").unwrap())
}

fn apostrof_rodo() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r#"(?i)(\p{L})’(\p{L}|"|‘|“|«)"#).unwrap())
}

fn apostrof_rodo_1() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)([dl])’(\d[\d\s.,]?)").unwrap())
}

fn decimal_point() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)([\d])\.([\d])").unwrap())
}

fn decimal_comma() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)([\d]),([\d])").unwrap())
}

fn space_digits0() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)([\d]{4}) ").unwrap())
}

fn space_digits() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)([\d]) ([\d][\d][\d])").unwrap())
}

fn space_digits2() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)([\d]) ([\d][\d][\d]) ([\d][\d][\d])").unwrap())
}

/// `wordCharacters` + the one-or-more/one-char alternation.
fn tokenizer_pattern() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let word_characters = "§©@€£\\$_\\p{L}\\d·\\-\u{0300}-\u{036F}\u{00A8}\u{2070}-\u{209F}°%‰‱&\u{FFFD}\u{00AD}\u{00AC}";
        regex::Regex::new(&format!("[{word_characters}]+|[^{word_characters}]")).unwrap()
    })
}

fn pattern_1() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)^([lnmtsd]['’])([^'’\-]*)$").unwrap())
}

fn pattern_2() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new("(?i)^(qui-sap-lo|qui-sap-la|qui-sap-los|qui-sap-les)|(Castella)(-)(la)$")
            .unwrap()
    })
}

fn pattern_3() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(&format!("(?i)^([lnmtsd]['’])(.{{2,}}){PF}{PF}{PF}$")).unwrap()
    })
}

fn pattern_4() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(&format!("(?i)^(.{{2,}}){PF}{PF}{PF}$")).unwrap())
}

fn pattern_5() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(&format!("(?i)^([lnmtsd]['’])(.{{2,}}){PF}{PF}$")).unwrap())
}

fn pattern_6() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(&format!("(?i)^(.{{2,}}){PF}{PF}$")).unwrap())
}

fn pattern_7() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(&format!("(?i)^([lnmtsd]['’])(.{{2,}}){PF}$")).unwrap())
}

fn pattern_8() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(&format!("(?i)^(.+[^wo]){PF}$")).unwrap())
}

fn pattern_9() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)^([lnmtsd]['’])(.*)$").unwrap())
}

fn pattern_10() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)^(a|de|pe)(ls?)$").unwrap())
}

fn pattern_11() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)^(ca)(n)$").unwrap())
}

fn hyphen_l() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)^(\p{L}+)(-)([Ll]['’])(\p{L}+)$").unwrap())
}

fn patterns() -> &'static [&'static regex::Regex] {
    static PATTERNS: OnceLock<Vec<&'static regex::Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            pattern_1(),
            pattern_2(),
            pattern_3(),
            pattern_4(),
            pattern_5(),
            pattern_6(),
            pattern_7(),
            pattern_8(),
            pattern_9(),
            pattern_10(),
            pattern_11(),
        ]
    })
}

/// `CatalanWordTokenizer`: the tokenizer keeps apostrophes inside words
/// (`l'home` → `l'`, `home`) and splits unknown hyphenated words.
pub struct CatalanWordTokenizer<'a> {
    is_tagged: IsTagged<'a>,
}

impl<'a> CatalanWordTokenizer<'a> {
    pub fn new(is_tagged: IsTagged<'a>) -> Self {
        Self { is_tagged }
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        // replace hyphen, non-break hyphen -> hyphen-minus; MODIFIER LETTER
        // APOSTROPHE (U+02BC) -> typographical apostrophe
        let mut aux_text = text
            .replace(['\u{2010}', '\u{2011}'], "-")
            .replace('\u{02BC}', "’");
        aux_text = ela_geminada()
            .replace_all(&aux_text, format!("${{1}}{ELA_SUBST}${{2}}").as_str())
            .into_owned();
        aux_text = ela_geminada_uppercase()
            .replace_all(&aux_text, format!("${{1}}{ELA_UPPER_SUBST}${{2}}").as_str())
            .into_owned();
        aux_text = apostrof_recte()
            .replace_all(
                &aux_text,
                format!("${{1}}{APOS_RECTE_SUBST}${{2}}").as_str(),
            )
            .into_owned();
        aux_text = apostrof_recte_1()
            .replace_all(
                &aux_text,
                format!("${{1}}{APOS_RECTE_SUBST}${{2}}").as_str(),
            )
            .into_owned();
        aux_text = apostrof_rodo()
            .replace_all(&aux_text, format!("${{1}}{APOS_RODO_SUBST}${{2}}").as_str())
            .into_owned();
        aux_text = apostrof_rodo_1()
            .replace_all(&aux_text, format!("${{1}}{APOS_RODO_SUBST}${{2}}").as_str())
            .into_owned();
        aux_text = decimal_point()
            .replace_all(
                &aux_text,
                format!("${{1}}{DECIMALPOINT_SUBST}${{2}}").as_str(),
            )
            .into_owned();
        aux_text = decimal_comma()
            .replace_all(
                &aux_text,
                format!("${{1}}{DECIMALCOMMA_SUBST}${{2}}").as_str(),
            )
            .into_owned();
        aux_text = space_digits0()
            .replace_all(&aux_text, "${1}xxCA_SPACE0xx")
            .into_owned();
        aux_text = space_digits2()
            .replace_all(
                &aux_text,
                format!("${{1}}{SPACE_SUBST}${{2}}{SPACE_SUBST}${{3}}").as_str(),
            )
            .into_owned();
        aux_text = space_digits()
            .replace_all(&aux_text, format!("${{1}}{SPACE_SUBST}${{2}}").as_str())
            .into_owned();
        aux_text = aux_text.replace("xxCA_SPACE0xx", " ");

        let mut l: Vec<String> = Vec::new();
        for m in tokenizer_pattern().find_iter(&aux_text) {
            let mut s = m.as_str().to_string();
            if let Some(last) = l.last_mut() {
                if s.chars().count() == 1 {
                    let c = s.chars().next().unwrap();
                    if ('\u{FE00}'..='\u{FE0F}').contains(&c) {
                        last.push_str(&s);
                        continue;
                    }
                }
            }
            s = s.replace(APOS_RECTE_SUBST, "'");
            s = s.replace(APOS_RODO_SUBST, "’");
            s = s.replace(HYPHEN_SUBST, "-");
            s = s.replace(DECIMALPOINT_SUBST, ".");
            s = s.replace(DECIMALCOMMA_SUBST, ",");
            s = s.replace(SPACE_SUBST, " ");
            s = s.replace(ELA_SUBST, "l.l");
            s = s.replace(ELA_UPPER_SUBST, "L.L");
            // leading hyphens
            while s.chars().count() > 1 && s.starts_with('-') {
                l.push("-".to_string());
                s = s[1..].to_string();
            }
            let mut hyphens_at_end = 0usize;
            while s.chars().count() > 1 && s.ends_with('-') {
                s.pop();
                hyphens_at_end += 1;
            }
            let mut match_found = false;
            for pattern in patterns() {
                if pattern.is_match(&s) {
                    match_found = true;
                    if let Some(caps) = pattern.captures(&s) {
                        for i in 1..caps.len() {
                            if let Some(group) = caps.get(i) {
                                l.extend(self.words_to_add(group.as_str()));
                            }
                        }
                    }
                    break;
                }
            }
            if !match_found {
                l.extend(self.words_to_add(&s));
            }
            for _ in 0..hyphens_at_end {
                l.push("-".to_string());
            }
        }
        join_emails_and_urls(l)
    }

    /// `CatalanWordTokenizer.wordsToAdd`.
    fn words_to_add(&self, s: &str) -> Vec<String> {
        let mut l: Vec<String> = Vec::new();
        if s.is_empty() {
            return l;
        }
        if !s.contains('-') && !s.ends_with('\'') && !s.ends_with('’') {
            l.push(s.to_string());
            return l;
        }
        // words containing hyphen (-) are looked up in the dictionary; some
        // camel-case words are accepted explicitly; the ela-geminada typo
        // retry replaces "l-l" with "l·l"
        let tagged = (self.is_tagged)(&s.replace('\u{00AD}', "").replace('’', "'"))
            || [
                "mers-cov",
                "mcgraw-hill",
                "sars-cov-2",
                "sars-cov",
                "ph-metre",
                "ph-metres",
            ]
            .iter()
            .any(|w| s.eq_ignore_ascii_case(w))
            || (self.is_tagged)(&s.replace('\u{00AD}', "").replace("l-l", "l·l"));
        if tagged {
            l.push(s.to_string());
        } else if (s.ends_with('\'') || s.ends_with('’')) && s.chars().count() > 1 {
            let head: String = s.chars().take(s.chars().count() - 1).collect();
            l.extend(self.words_to_add(&head));
            l.push(s.chars().last().unwrap().to_string());
        } else if let Some(caps) = hyphen_l().captures(s) {
            for i in 1..caps.len() {
                if let Some(group) = caps.get(i) {
                    l.extend(self.words_to_add(group.as_str()));
                }
            }
        } else {
            // StringTokenizer(s, "-", true)
            let mut current = String::new();
            for c in s.chars() {
                if c == '-' {
                    if !current.is_empty() {
                        l.push(std::mem::take(&mut current));
                    }
                    l.push("-".to_string());
                } else {
                    current.push(c);
                }
            }
            if !current.is_empty() {
                l.push(current);
            }
        }
        l
    }
}
