//! Port of `org.languagetool.synthesis.Soros` (the Numbertext interpreter
//! used by `BaseSynthesizer.getSpelledNumber`, French `fr/fr.sor`).

use std::sync::OnceLock;

use regex::Regex;

/// Java `Soros.slash` (`\uE000`).
const SLASH: char = '\u{E000}';
const QUOTE_MARK: char = '\u{E001}';
const SEMI_MARK: char = '\u{E002}';
const PIPE_MARK: char = '\u{E003}';
const ESCAPED_MARKS: &str = "\u{E004}\u{E005}\u{E006}\u{E007}\u{E008}\u{E009}";
/// Java `Soros` separator (`\uE00A`).
const SEPARATOR: char = '\u{E00A}';

pub struct Soros {
    patterns: Vec<Regex>,
    values: Vec<String>,
    begins: Vec<bool>,
    ends: Vec<bool>,
}

impl Soros {
    /// `Soros(String source, String lang)`.
    pub fn new(source: &str, lang: &str) -> Self {
        let mut source = translate(source, "\\\";#", "\u{E000}\u{E001}\u{E002}\u{E003}", "\\");
        // switch off all country-dependent lines, and switch on the requested ones
        let off = Regex::new(r"(^|[\n;])([^\n;#]*#[^\n]*\[:[^\n:\]]*:][^\n]*)")
            .expect("soros country-off pattern");
        source = off.replace_all(&source, "${1}#${2}").into_owned();
        let lang = lang.replace('_', "-");
        let on = Regex::new(&format!(
            r"(^|[\n;])#([^\n;#]*#[^\n]*\[:{}:][^\n]*)",
            regex::escape(&lang)
        ))
        .expect("soros country-on pattern");
        source = on.replace_all(&source, "${1}${2}").into_owned();
        source = Regex::new(r"(#[^\n]*)?(\n|$)")
            .expect("soros comment pattern")
            .replace_all(&source, ";")
            .into_owned();
        if !source.contains("__numbertext__") {
            source = format!("__numbertext__;{source}");
        }
        let numbertext = format!(
            "\"([a-z][-a-z]* )?0+(0|[1-9]\\d*)\" $(\\1\\2);\"{SEPARATOR}(.*){SEPARATOR}(.+){SEPARATOR}(.*)\" \\1\\2\\3;\"{SEPARATOR}.*{SEPARATOR}{SEPARATOR}.*\""
        );
        source = source.replace("__numbertext__", &numbertext);

        // Java `\s` is the ASCII class
        let ws = "[ \t\n\x0B\x0C\r]";
        let p = Regex::new(&format!(
            r#"^{ws}*("[^"]*"|[^{ws}]*){ws}*(.*[^{ws}])?{ws}*$"#
        ))
        .expect("soros line pattern");
        let macro_re = Regex::new(r"^== *(.*[^ ]?) ==$").expect("soros macro pattern");

        let mut patterns = Vec::new();
        let mut values = Vec::new();
        let mut begins = Vec::new();
        let mut ends = Vec::new();
        let mut prefix = String::new();
        let sep_start = Regex::new(r"^\[[$](\d\d?|\([^)]+\))").expect("soros sep-start pattern");
        let sep_mid =
            Regex::new(r"\[([^$\[\]\\]*)[$](\d\d?|\([^)]+\))").expect("soros sep-mid pattern");
        let sep_close = Regex::new(r"(\$\d|\))\|\$").expect("soros sep-close pattern");
        let group_ref = Regex::new(&format!("{SLASH}([0-9])")).expect("soros $n pattern");
        let backslash_ref = Regex::new(r"\\([0-9])").expect("soros \\n pattern");
        for segment in source.split(';') {
            if let Some(caps) = macro_re.captures(segment) {
                prefix = caps[1].to_string();
                continue;
            }
            if segment.is_empty() {
                continue;
            }
            let rebuilt_owned: String;
            let Some(mut caps) = p.captures(segment) else {
                continue;
            };
            if !prefix.is_empty() {
                let group1 = strip_quotes(&caps[1]);
                let group2 = caps.get(2).map(|m| m.as_str()).unwrap_or("null");
                rebuilt_owned = format!(
                    "\"{}{}{}{}\" {}",
                    if group1.starts_with('^') { "^" } else { "" },
                    prefix,
                    if group1.is_empty() { "" } else { " " },
                    group1.strip_prefix('^').unwrap_or(group1),
                    group2
                );
                let Some(rebuilt_caps) = p.captures(&rebuilt_owned) else {
                    continue;
                };
                caps = rebuilt_caps;
            }

            let mut pattern = translate(
                strip_quotes(&caps[1]),
                "\u{E001}\u{E002}\u{E003}",
                "\";#",
                "",
            );
            pattern = pattern.replace(SLASH, "\\\\");
            let mut value = translate(
                strip_quotes(caps.get(2).map(|m| m.as_str()).unwrap_or("")),
                "$()|[]",
                ESCAPED_MARKS,
                "\\",
            );
            // [ ... $1 ... ] -> $( <sep> ... <sep> $1 <sep> ... )
            value = sep_start
                .replace_all(&value, |c: &regex::Captures| {
                    format!("$({sep}{sep}|${g1}{sep}", sep = SEPARATOR, g1 = &c[1])
                })
                .into_owned();
            value = sep_mid
                .replace_all(&value, |c: &regex::Captures| {
                    format!(
                        "$({sep}{g1}{sep}${g2}{sep}",
                        sep = SEPARATOR,
                        g1 = &c[1],
                        g2 = &c[2]
                    )
                })
                .into_owned();
            value = value
                .replace(&format!("{SEPARATOR}]$"), &format!("|{SEPARATOR})"))
                .replace(']', ")");
            value = sep_close
                .replace_all(&value, |c: &regex::Captures| format!("{}||$", &c[1]))
                .into_owned();
            value = translate(&value, "\u{E000}\u{E001}\u{E002}\u{E003}", "\\\";#", "");
            value = translate(&value, "$()|", "\u{E000}\u{E001}\u{E002}\u{E003}", "");
            value = translate(&value, ESCAPED_MARKS, "$()|[]", "");
            value = value.replace('$', "\\$");
            value = group_ref
                .replace_all(&value, |c: &regex::Captures| {
                    format!("{SLASH}{QUOTE_MARK}${}{SEMI_MARK}", &c[1])
                })
                .into_owned();
            value = backslash_ref
                .replace_all(&value, |c: &regex::Captures| format!("${}", &c[1]))
                .into_owned();
            value = value.replace("\\n", "\n");

            let stripped = pattern.strip_prefix('^').unwrap_or(&pattern);
            let stripped = stripped.strip_suffix('$').unwrap_or(stripped);
            let re = Regex::new(&format!("^{stripped}$"))
                .unwrap_or_else(|e| panic!("fr.sor pattern does not compile: {e}: {stripped}"));
            patterns.push(re);
            begins.push(pattern.starts_with('^'));
            ends.push(pattern.ends_with('$'));
            values.push(value);
        }
        Self {
            patterns,
            values,
            begins,
            ends,
        }
    }

    /// `Soros.run(String)`.
    pub fn run(&self, input: &str) -> String {
        self.run_with(input, true, true)
    }

    fn run_with(&self, input: &str, begin: bool, end: bool) -> String {
        for i in 0..self.patterns.len() {
            if (!begin && self.begins[i]) || (!end && self.ends[i]) {
                continue;
            }
            let Some(caps) = self.patterns[i].captures(input) else {
                continue;
            };
            let mut s = expand_replacement(&self.values[i], &caps);
            while let Some(call) = func_regex().captures(&s) {
                let whole = call.get(0).expect("group 0");
                let group1 = call.get(1).expect("group 1");
                let group2 = call.get(2).expect("group 2");
                let mut use_begin = false;
                let mut use_end = false;
                if group1.as_str().starts_with(PIPE_MARK) || whole.as_str().starts_with(PIPE_MARK) {
                    use_begin = true;
                } else if whole.start() == 0 {
                    use_begin = begin;
                }
                if group1.as_str().ends_with(PIPE_MARK) || whole.as_str().ends_with(PIPE_MARK) {
                    use_end = true;
                } else if whole.end() == s.len() {
                    use_end = end;
                }
                let (start, stop) = (group1.start(), group1.end());
                let replaced = self.run_with(group2.as_str(), use_begin, use_end);
                s = format!("{}{replaced}{}", &s[..start], &s[stop..]);
            }
            return s;
        }
        String::new()
    }
}

/// Java `Soros.translate`.
fn translate(s: &str, chars: &str, chars2: &str, delim: &str) -> String {
    let mut out = s.to_string();
    for (a, b) in chars.chars().zip(chars2.chars()) {
        out = out.replace(&format!("{delim}{a}"), &b.to_string());
    }
    out
}

fn strip_quotes(s: &str) -> &str {
    let s = s.strip_prefix('"').unwrap_or(s);
    s.strip_suffix('"').unwrap_or(s)
}

/// Java `Matcher.replaceAll` replacement expansion: `\x` is `x`, `$N` is the
/// group (empty for a group that did not participate).
fn expand_replacement(replacement: &str, caps: &regex::Captures) -> String {
    let mut out = String::new();
    let mut chars = replacement.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            }
            '$' => {
                let mut group: Option<usize> = None;
                while let Some(&next) = chars.peek() {
                    let Some(digit) = next.to_digit(10) else {
                        break;
                    };
                    let candidate = group.map_or(digit as usize, |g| g * 10 + digit as usize);
                    if candidate >= caps.len() {
                        break;
                    }
                    group = Some(candidate);
                    chars.next();
                }
                match group {
                    Some(idx) => {
                        if let Some(m) = caps.get(idx) {
                            out.push_str(m.as_str());
                        }
                    }
                    None => out.push('$'),
                }
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Java `Soros.func`: recognizes `$(...)` calls (with optional `|` guards).
fn func_regex() -> &'static Regex {
    static FUNC: OnceLock<Regex> = OnceLock::new();
    FUNC.get_or_init(|| {
        let source = translate(
            r"(?:\|?(?:\$\(+))?(\|?\$\(([^\(\)]*)\)\|?)(?:\)+\|?)?",
            "$()|",
            "\u{E000}\u{E001}\u{E002}\u{E003}",
            "\\",
        );
        Regex::new(&source).expect("soros func pattern")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_data::PathExt as _;

    fn soros() -> Option<Soros> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/fr/fr.sor");
        if !path.lt_exists() {
            return None;
        }
        Some(Soros::new(
            &lt_data::fs::read_to_string(path).unwrap(),
            "fr",
        ))
    }

    /// Pinned with `scripts/oracle/fr/probe-soros.sh` (Java Docker).
    #[test]
    fn spells_out_numbers_like_java() {
        let Some(soros) = soros() else {
            eprintln!("skipping: no vendored data");
            return;
        };
        let cases: &[(&str, &str)] = &[
            ("0", "zéro"),
            ("1", "un"),
            ("2", "deux"),
            ("3", "trois"),
            ("4", "quatre"),
            ("5", "cinq"),
            ("6", "six"),
            ("7", "sept"),
            ("8", "huit"),
            ("9", "neuf"),
            ("10", "dix"),
            ("11", "onze"),
            ("16", "seize"),
            ("20", "vingt"),
            ("21", "vingt et un"),
            ("30", "trente"),
            ("34", "trente-quatre"),
            ("40", "quarante"),
            ("50", "cinquante"),
            ("60", "soixante"),
            ("70", "soixante-dix"),
            ("71", "soixante et onze"),
            ("72", "soixante-douze"),
            ("80", "quatre-vingts"),
            ("81", "quatre-vingt-un"),
            ("90", "quatre-vingt-dix"),
            ("91", "quatre-vingt-onze"),
            ("99", "quatre-vingt-dix-neuf"),
            ("100", "cent"),
            ("101", "cent un"),
            ("110", "cent dix"),
            ("111", "cent onze"),
            ("121", "cent vingt et un"),
            ("200", "deux cents"),
            ("201", "deux cent un"),
            ("300", "trois cents"),
            ("999", "neuf cent quatre-vingt-dix-neuf"),
            ("1000", "mille"),
            ("1001", "mille un"),
            ("1100", "onze cents"),
            ("1234", "mille deux cent trente-quatre"),
            ("2000", "deux mille"),
            ("10000", "dix mille"),
            ("21000", "vingt et un mille"),
            ("100000", "cent mille"),
            ("123456", "cent vingt-trois mille quatre cent cinquante-six"),
            ("1000000", "un million"),
            ("2000000", "deux millions"),
            ("1000000000", "un milliard"),
            ("3.14", "trois virgule quatorze"),
            ("3,14", "trois virgule quatorze"),
            ("-5", "moins cinq"),
            ("0.5", "zéro virgule cinq"),
            ("07", "sept"),
            ("007", "sept"),
            ("feminine 1", ""),
            ("feminine 5", ""),
            ("feminine 21", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(soros.run(input), *expected, "input {input:?}");
        }
    }
}
