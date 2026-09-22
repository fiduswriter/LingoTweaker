//! Port of `org.languagetool.tokenizers.uk.UkrainianWordTokenizer`.
//!
//! Specific to Ukrainian: `'`/`U+2019` stay inside the word, so the tokenizer
//! first protects the non-breaking constructs with private-use placeholders
//! (decimals, dashes between numbers, dates, initials, abbreviations, `с/г`,
//! URLs, brackets in words `ВКПБ(о)`, superscripts) and then splits on the
//! `SPLIT_CHARS` regex, restoring the placeholders afterwards.
//!
//! Java regex constructs the Rust regex crate does not understand are
//! translated: `\h`/`\v` become explicit classes and variable-length
//! lookbehinds are expanded ([`lt_core::regex_util::expand_lookbehinds_bounded`]).

use std::sync::OnceLock;

use fancy_regex::Regex;

const SPLIT_CHARS: &str = r#"(!{2,3}|\?{2,3}|\.{3}|[!?][!?.]{1,2}|[\u{0020}\u{00A0}\n\r\t,.;!?\u{2014}\u{2015}:()\[\]{}<>/|\\…°$€₴=№§¿¡~×]|%(?![-\u{2013}][а-яіїєґ])|(?<!\u{E109})["«»„”“]|(?<=[а-яіїєґА-ЯІЇЄҐ])[\u{00B9}\u{00B2}\u{00B3}\u{2070}-\u{2079}]|(?<![а-яіїєґА-ЯІЇЄҐa-zA-Z])[_*]+|[_*]+(?![а-яіїєґА-ЯІЇЄҐa-zA-Z0-9])|[\u{2000}-\u{200F}\u{201A}\u{2020}-\u{202F}\u{2030}-\u{206F}\u{2400}-\u{27FF}\u{21B5}\u{1F000}-\u{1FFFF}\u{F000}-\u{FFFF}\u{E110}])(?!\u{E120})"#;

const DECIMAL_COMMA_SUBST: char = '\u{E001}';
const NON_BREAKING_SPACE_SUBST: char = '\u{E002}';
const NON_BREAKING_DOT_SUBST: char = '\u{E003}';
const NON_BREAKING_COLON_SUBST: char = '\u{E004}';
const LEFT_BRACE_SUBST: char = '\u{E005}';
const RIGHT_BRACE_SUBST: char = '\u{E006}';
const NON_BREAKING_SLASH_SUBST: char = '\u{E007}';
const LEFT_ANGLE_SUBST: char = '\u{E008}';
const RIGHT_ANGLE_SUBST: char = '\u{E009}';
const SLASH_SUBST: char = '\u{E010}';
const NON_BREAKING_PLACEHOLDER: char = '\u{E109}';
const BREAKING_PLACEHOLDER: char = '\u{E110}';
const NON_BREAKING_PLACEHOLDER2: char = '\u{E120}';
const SOFT_HYPHEN_WRAP: &str = "\u{00AD}\n";
const SOFT_HYPHEN_WRAP_SUBST: char = '\u{E103}';
const URL_START_REPLACE_CHAR: u32 = 0xE300;

/// Translate the Java regex constructs the Rust regex crate rejects and
/// optionally prefix a case-insensitive flag.
fn jregex(pattern: &str, case_insensitive: bool) -> Regex {
    const H: &str = r" \t\u{00A0}\u{1680}\u{180E}\u{2000}-\u{200A}\u{202F}\u{205F}\u{3000}";
    const V: &str = r"\n\u{000B}\f\r\u{0085}\u{2028}\u{2029}";
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len());
    let mut in_class = false;
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            if let Some(&n) = chars.get(i + 1) {
                match n {
                    'h' | 'v' => {
                        let content = if n == 'h' { H } else { V };
                        if in_class {
                            out.push_str(content);
                        } else {
                            out.push('[');
                            out.push_str(content);
                            out.push(']');
                        }
                        i += 2;
                        continue;
                    }
                    _ => {
                        out.push('\\');
                        out.push(n);
                        i += 2;
                        continue;
                    }
                }
            }
            out.push('\\');
            i += 1;
            continue;
        }
        match c {
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            _ => {}
        }
        out.push(c);
        i += 1;
    }
    let expanded = lt_core::regex_util::expand_lookbehinds_bounded(&out, 8);
    let final_pattern = if case_insensitive {
        format!("(?i){expanded}")
    } else {
        expanded
    };
    Regex::new(&final_pattern).unwrap_or_else(|e| panic!("uk tokenizer regex {final_pattern}: {e}"))
}

struct Patterns {
    split_chars: Regex,
    weird_apostroph: Regex,
    leading_dash: Regex,
    leading_dash2: Regex,
    decimal_comma: Regex,
    url: Regex,
    em_dash_space: Regex,
    dash_numbers: Regex,
    n_dash_space: Regex,
    n_dash_space2: Regex,
    year_with_r: Regex,
    percent: Regex,
    compound_quotes1: Regex,
    compound_quotes2: Regex,
    words_with_brackets: Regex,
    decimal_space: Regex,
    dotted3: Regex,
    dotted: Regex,
    abbr_nar1: Regex,
    abbr_nar2: Regex,
    abbr_2_small: Regex,
    abbr_vo1: Regex,
    abbr_vo2: Regex,
    abbr_vo3: Regex,
    abbr_art: Regex,
    abbr_man: Regex,
    abbr_tys1: Regex,
    abbr_tys2: Regex,
    abbr_vyd: Regex,
    abbr_lat: Regex,
    abbr_prof: Regex,
    abbr_gub: Regex,
    abbr_dash: Regex,
    initials_sp2: Regex,
    initials_sp1: Regex,
    initials_rsp2: Regex,
    initials_rsp1: Regex,
    abbr_kub_sm: Regex,
    abbr_s_g: Regex,
    abbr_chl_kor: Regex,
    abbr_nauk: Regex,
    abbr_pn_zah: Regex,
    abbr_i_t_p: Regex,
    abbr_i_t_ch: Regex,
    abbr_t_zv: Regex,
    abbr_red_avt: Regex,
    abbr_non_ending: Regex,
    abbr_non_ending2: Regex,
    invalid_mln: Regex,
    web_entities: Regex,
    web_entities2: Regex,
    abbr_ending: Regex,
    colon_numbers: Regex,
    brace_in_word: Regex,
    xml_tag: Regex,
    hyphen1: Regex,
    hyphen2: Regex,
    soft_hyphen_wrap: Regex,
    apostrophe_begin: Regex,
    apostrophe_end: Regex,
    plus: Regex,
    leading_num_dash: Regex,
    number_missing_space: Regex,
    abbr_at_end: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        split_chars: jregex(SPLIT_CHARS, false),
        weird_apostroph: jregex(
            r#"([бвджзклмнпрстфхш])(["\u{201D}\u{201F}`´])([єїюя])"#,
            true,
        ),
        leading_dash: jregex(r"^([\u{2014}\u{2013}])([а-яіїєґА-ЯІЇЄҐA-Z])", false),
        leading_dash2: jregex(r"^(-)([А-ЯІЇЄҐA-Z])", false),
        decimal_comma: jregex(r"([\d]),([\d])", true),
        url: jregex(
            r#"((https?|ftp)://|www\.)[^\h\v/$.?#),]+\.[^\h\v),">]*|(mailto:)?[\p{L}\d._-]+@[\p{L}\d_-]+(\.[\p{L}\d_-]+)+"#,
            true,
        ),
        em_dash_space: jregex(r"\u{2014}([\h\v])", false),
        dash_numbers: jregex(r"([IVXІХ]+)([\u{2013}-])([IVXІХ]+)", false),
        n_dash_space: jregex(
            r"([а-яіїєґa-z0-9])(\u{2013}\h)(?!(та|чи|і|й)[\h\v])",
            true,
        ),
        n_dash_space2: jregex(r"([\h.,;!?]\u{2013})([а-яіїєґa-z])", true),
        year_with_r: jregex(
            r"((?:[12][0-9]{3}[\u{2014}\u{2013}-])?[12][0-9]{3})(рр?\.)",
            false,
        ),
        percent: jregex(r"%([^-])", false),
        compound_quotes1: jregex(r#"([а-яіїє]-)([«"„])([а-яіїєґ'-]+)([»"“])"#, true),
        compound_quotes2: jregex(r#"([«"„])([а-яіїєґ0-9'-]+)([»\"“])(-[а-яіїє])"#, true),
        words_with_brackets: jregex(r"([а-яіїєґ])\[([а-яіїєґ]+)\]", true),
        // Java `(?<=^|[\h\v(])` has a variable-length lookbehind; rewrite it
        // as a non-capturing prefix group.
        decimal_space: jregex(
            r"(?:^|(?<=[\h\v(]))\d{1,3}([\h][\d]{3})+(?=[\h\v(]|$)",
            true,
        ),
        dotted3: jregex(r"([\d])\.([\d]+)\.([\d])", false),
        dotted: jregex(r"([\d])\.([\d])", false),
        abbr_nar1: jregex(r"(([0-9]|рік|[рp]\.|[\u{2013}\u{2014}-])[\h\v]+нар)\.", false),
        abbr_nar2: jregex(r"\b(нар)\.([\h\v]+[0-9а-яіїєґ])", false),
        abbr_2_small: jregex(
            r"([^а-яіїєґА-ЯІЇЄҐ'\u{301}-][векнпрстцч]{1,2})\.(\h*(?![смкд]?м\.)[екмнпрстч]{1,2})\.",
            false,
        ),
        abbr_vo1: jregex(r"\b([вВу])\.([\h\v]*о)\.", false),
        abbr_vo2: jregex(r"\b(к)\.([\h\v]*с)\.", false),
        abbr_vo3: jregex(r"\b(ч|ст)\.([\h\v]*л)\.", false),
        abbr_art: jregex(
            r"([Аа]рт|[Мм]ал|[Рр]ис|[Сс]пр)\.([\h]*(№[\h]*)?[0-9])",
            false,
        ),
        abbr_man: jregex(r"(Ман)\.([\h]*(Сіті|[Юю]н))", false),
        abbr_tys1: jregex(r"([0-9IІ][\h\v]+)(тис|арт)\.", false),
        abbr_tys2: jregex(r"(тис|арт)\.([\h\v]+[а-яіїєґ0-9])", false),
        abbr_vyd: jregex(r"([0-9IІVXХ](-[а-яєі]+)?[\h\v]+)(вид)\.", false),
        abbr_lat: jregex(r"([^а-яіїєґА-ЯІЇЄҐ'\u{301}-]лат)\.([\h\v]+[a-zA-Z])", false),
        abbr_prof: jregex(
            r"(?<![а-яіїєґА-ЯІЇЄҐ'\u{301}-])([Аа]кад|[Пп]роф|[Дд]оц|[Аа]сист|[Аа]рх|ап|тов|вул|бул|бульв|о|р|ім|упорядн?|др|[Пп]реп|Ів|Дж|Ол|[сС]вт|Авг)\.([\h\v]+[А-ЯІЇЄҐа-яіїєґ])",
            false,
        ),
        abbr_gub: jregex(r"(.[А-ЯІЇЄҐ][а-яіїєґ'-]+[\h\v]+губ)\.", false),
        abbr_dash: jregex(
            r"\b([А-ЯІЇЄҐ]ж?)\.([-\u{2013}]([А-ЯІЇЄҐ][а-яіїєґ']{2}|[А-ЯІЇЄҐ]\.))",
            false,
        ),
        initials_sp2: jregex(
            r"([А-ЯІЇЄҐ])\.([\h\v]{0,5}[А-ЯІЇЄҐ])\.([\h\v]{0,5}[А-ЯІЇЄҐ][а-яіїєґ']+)",
            false,
        ),
        initials_sp1: jregex(
            r"([А-ЯІЇЄҐ])\.([\h\v]{0,5}[А-ЯІЇЄҐ][а-яіїєґ']+)",
            false,
        ),
        initials_rsp2: jregex(
            r"([А-ЯІЇЄҐ][а-яіїєґ']+)([\h\v]?[А-ЯІЇЄҐ])\.([\h\v]?[А-ЯІЇЄҐ])\.",
            false,
        ),
        initials_rsp1: jregex(r"([А-ЯІЇЄҐ][а-яіїєґ']+)([\h\v]?[А-ЯІЇЄҐ])\.", false),
        abbr_kub_sm: jregex(r"(кв|куб)\.([\h\v]*(?:[смкд]|мк)?м)", false),
        abbr_s_g: jregex(r"\b(с)\.(-г)\.", false),
        abbr_chl_kor: jregex(r"(чл)\.(-кор)\.", false),
        abbr_nauk: jregex(r"(наук)\.(-[а-яіїєґ]+)\.", false),
        abbr_pn_zah: jregex(r"(пн|пд)\.(-(зах|сх))\.", false),
        abbr_i_t_p: jregex(r"([ій][\h\v]+т\.)([\h\v]*(д|п|ін)\.)", false),
        abbr_i_t_ch: jregex(r"([ву][\h\v]+т\.)([\h\v]*ч\.)", false),
        abbr_t_zv: jregex(r"([\h\v(]+т\.)([\h\v]*зв\.)", false),
        abbr_red_avt: jregex(r"([\h\v]+(?:[Рр]ед|[Аа]вт))\.(\h*[)\]а-яіїєґ])", false),
        abbr_non_ending: jregex(
            r"(?<![а-яіїєґА-ЯІЇЄҐ'\u{301}-])(абз|австрал|ам|амер|англ|акад(ем)?|арк|ауд|біол|бл(?:изьк)?|болг|буд|в(?!\.+)|вип|вірм|грец(?:ьк)?|держ|див|дир|діал|дод|дол|досл|доц|доп|екон|ел|жін|зав|заст|зах|зб|зв|зневажл?|зовн|іл|ім|івр|інж|ісп|іст|італ|к|каб|каф|канд|кв|[1-9]-кімн|кімн|кін|кл|кн|коеф|крим|латин|мал|моб|н|[Нн]апр|нач|нім|нац|нпр|образн|оз|оп|оф|п|пен|перекл|перен|пл|пол|пом|пор|порівн|[Пп]оч|пп|прибл|прикм|прим|присл|пров|пром|просп|[Рр]ед|[Рр]еж|розд|розм|рос|рт|рум|с|санскр|[Сс]вв?|скор|соц|співавт|[сС]т|стор|суч|сх|табл|тт|[тТ]ел|техн|укр|філол|фр|франц|худ|[цЦ]ит|ч|чайн|част|ц|яп|япон)\.(?!\u{E120}|\.+[\h\v]*$)",
            false,
        ),
        abbr_non_ending2: jregex(r"([^а-яіїєґА-ЯІЇЄҐ'-]м\.)([\h\v]*[А-ЯІЇЄҐ])", false),
        invalid_mln: jregex(r"(млн|млрд)\.( [а-яіїєґ])", false),
        web_entities: jregex(
            r"([а-яіїєґ])\.(НЕТ|net|Інфо|Info|City|Life|UA|юа|лі|media|com|фм|ru|ру|орг)\b",
            true,
        ),
        web_entities2: jregex(r"\.([a-z_-]+)\.(ua)", true),
        abbr_ending: jregex(
            r"([^а-яіїєґА-ЯІЇЄҐ'\u{301}-]((та|й|і) (інш?|под)|атм|відс|гр|коп|дес|дол|обл|пов|р|рр|РР|руб|ст|стст|стол|стор|чол|шт))\.(?!\u{E120})",
            false,
        ),
        colon_numbers: jregex(r"([\d]):([\d])", false),
        brace_in_word: jregex(r"([а-яіїєґ])\(([а-яіїєґ']+)\)", true),
        xml_tag: jregex(r"<(/?[a-z_]+/?)>", true),
        hyphen1: jregex(r#"([а-яіїєґА-ЯІЇЄҐ])([»"-]+-)"#, false),
        hyphen2: jregex(r#"([»"-]+-)([а-яіїєґА-ЯІЇЄҐ])"#, false),
        soft_hyphen_wrap: jregex(r"(?<!\s)\u{00AD}\n", false),
        apostrophe_begin: jregex(r#"(^|[\h\v(„«"'])'(?!дно)(\p{L})"#, false),
        apostrophe_end: jregex(
            r"(\p{L})(?<!\b(?:мо|тре|тра|чо|нічо|бо|зара|пра))'([^\p{L}-]|$)",
            true,
        ),
        plus: jregex(r"\+(?=[а-яіїєґА-ЯІЇЄҐ0-9])", false),
        // Java `(?<=(^|[\h\v]))`: rewrite the variable-length lookbehind.
        leading_num_dash: jregex(r"(?:^|(?<=[\h\v]))([-\u{2013}])(?=[0-9])", false),
number_missing_space: jregex(
            r#"((?:[\h\v\u{E110}]|^)[а-яїієґА-ЯІЇЄҐ'-]*[а-яїієґ']?[а-яїієґ])([0-9]+(?![а-яїієґА-ЯІЇЄҐa-zA-Z»"“]))"#,
            false,
        ),
        abbr_at_end: jregex(
            r"(?<![а-яіїєґА-ЯІЇЄҐ'\u{301}])(тис|губ|[А-ЯІЇЄҐ])\.[\h\v]*$",
            false,
        ),
    })
}

fn split_with_delimiters(text: &str, delim: &Regex) -> Vec<String> {
    let mut parts = Vec::new();
    let mut last_end = 0usize;
    for m in delim.find_iter(text).flatten() {
        if last_end != m.start() {
            parts.push(text[last_end..m.start()].to_string());
        }
        parts.push(m.as_str().to_string());
        last_end = m.end();
    }
    if last_end != text.len() {
        parts.push(text[last_end..].to_string());
    }
    parts
}

/// Map the tokenizer's output tokens back to byte spans in the **original**
/// text. `cleanup` is a 1:1 character map (typographic apostrophes/quotes to
/// ASCII, `U+2011` to `-`) and every placeholder is removed again, so the
/// tokens concatenate to the original text with those characters normalized;
/// walking both in parallel recovers the original byte length even when a
/// normalized character is 3 bytes in the original and 1 in the token.
pub fn original_spans(original: &str, tokens: &[String]) -> Vec<(usize, usize)> {
    fn normalize(c: char) -> char {
        match c {
            '\u{2019}' | '\u{02BC}' | '\u{2018}' => '\'',
            '\u{201A}' => ',',
            '\u{2011}' => '-',
            other => other,
        }
    }
    let chars: Vec<(char, usize, usize)> = original
        .char_indices()
        .map(|(i, c)| (normalize(c), i, i + c.len_utf8()))
        .collect();
    let mut oi = 0usize;
    let mut spans = Vec::with_capacity(tokens.len());
    for token in tokens {
        let start = chars.get(oi).map_or(original.len(), |c| c.1);
        for c in token.chars() {
            if oi < chars.len() && chars[oi].0 == c {
                oi += 1;
            } else {
                break;
            }
        }
        let end = chars.get(oi).map_or(original.len(), |c| c.1);
        spans.push((start, end.max(start)));
    }
    spans
}

fn cleanup(text: &str) -> String {
    let replaced: String = text
        .chars()
        .map(|c| match c {
            '\u{2019}' | '\u{02BC}' | '\u{2018}' => '\'',
            '\u{201A}' => ',',
            '\u{2011}' => '-',
            other => other,
        })
        .collect();
    patterns()
        .weird_apostroph
        .replace_all(
            &replaced,
            format!("$1{NON_BREAKING_PLACEHOLDER2}$2{NON_BREAKING_PLACEHOLDER2}$3"),
        )
        .into_owned()
}

fn adjust_text_for_tokenizing(text: &str) -> (String, Vec<(String, String)>) {
    let p = patterns();
    let mut text = cleanup(text);
    let mut urls: Vec<(String, String)> = Vec::new();

    if let Some(first) = text.chars().next() {
        if "\u{2014}\u{2013}-".contains(first) {
            if p.leading_dash.is_match(&text).unwrap_or(false) {
                text = p
                    .leading_dash
                    .replace(&text, format!("$1{BREAKING_PLACEHOLDER}$2"))
                    .into_owned();
            } else if p.leading_dash2.is_match(&text).unwrap_or(false) {
                text = p
                    .leading_dash2
                    .replace(&text, format!("$1{BREAKING_PLACEHOLDER}$2"))
                    .into_owned();
            }
        }
    }

    if text.contains(',') {
        text = p
            .decimal_comma
            .replace_all(&text, format!("$1{DECIMAL_COMMA_SUBST}$2"))
            .into_owned();
    }

    if text.contains("http") || text.contains("www") || text.contains("@") || text.contains("ftp") {
        // Java replaces the first match with a fresh private-use char and
        // re-matches; the char cannot introduce a new match, so assigning the
        // chars to the original matches and replacing right-to-left is
        // equivalent.
        let mut replacements: Vec<(usize, usize, char)> = Vec::new();
        for (i, m) in p.url.find_iter(&text).flatten().enumerate() {
            let replace_char = char::from_u32(URL_START_REPLACE_CHAR + i as u32).unwrap();
            urls.push((replace_char.to_string(), m.as_str().to_string()));
            replacements.push((m.start(), m.end(), replace_char));
        }
        for (start, end, replace_char) in replacements.into_iter().rev() {
            text.replace_range(start..end, &replace_char.to_string());
        }
    }

    if text.contains('\u{2014}') {
        text = p
            .em_dash_space
            .replace_all(&text, format!("{BREAKING_PLACEHOLDER}\u{2014}$1"))
            .into_owned();
    }

    let n_dash_present = text.contains('\u{2013}');
    if text.contains('-') || n_dash_present {
        text = p
            .dash_numbers
            .replace_all(
                &text,
                format!("$1{BREAKING_PLACEHOLDER}$2{BREAKING_PLACEHOLDER}$3"),
            )
            .into_owned();
        if n_dash_present {
            text = p
                .n_dash_space
                .replace_all(&text, format!("$1{BREAKING_PLACEHOLDER}$2"))
                .into_owned();
            text = p
                .n_dash_space2
                .replace_all(&text, format!("$1{BREAKING_PLACEHOLDER}$2"))
                .into_owned();
        }
    }

    if text.contains("с/г") {
        text = text.replace("с/г", &format!("с{NON_BREAKING_SLASH_SUBST}г"));
    }
    if text.contains("Л/ДНР") {
        text = text.replace("Л/ДНР", &format!("Л{NON_BREAKING_SLASH_SUBST}ДНР"));
    }

    if text.contains("р.") && p.year_with_r.is_match(&text).unwrap_or(false) {
        text = p
            .year_with_r
            .replace_all(&text, format!("$1{BREAKING_PLACEHOLDER}$2"))
            .into_owned();
    }

    text = text.replace('#', &format!("{BREAKING_PLACEHOLDER}#"));

    if text.contains('%') {
        text = p
            .percent
            .replace_all(&text, format!("%{BREAKING_PLACEHOLDER}$1"))
            .into_owned();
    }

    text = p
        .compound_quotes1
        .replace_all(
            &text,
            format!("$1$2{NON_BREAKING_PLACEHOLDER2}$3{NON_BREAKING_PLACEHOLDER2}$4{NON_BREAKING_PLACEHOLDER2}"),
        )
        .into_owned();
    text = p
        .compound_quotes2
        .replace_all(
            &text,
            format!("$1{NON_BREAKING_PLACEHOLDER2}$2{NON_BREAKING_PLACEHOLDER2}$3{NON_BREAKING_PLACEHOLDER2}$4"),
        )
        .into_owned();
    if text.contains('[') {
        text = p
            .words_with_brackets
            .replace_all(
                &text,
                format!("$1\\[{NON_BREAKING_PLACEHOLDER2}$2\\]{NON_BREAKING_PLACEHOLDER2}"),
            )
            .into_owned();
    }

    let dot_index = text.find('.');
    let rtrimmed_len = text
        .trim_end_matches(|c: char| c.is_whitespace())
        .chars()
        .count();
    let dot_inside_sentence = dot_index
        .map(|byte_idx| text[..byte_idx].chars().count() < rtrimmed_len.saturating_sub(1))
        .unwrap_or(false);
    let abbr_at_end = p.abbr_at_end.find(&text).ok().flatten().is_some();

    if dot_inside_sentence || (dot_index.is_some() && abbr_at_end) {
        text = p
            .dotted3
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}$2.{NON_BREAKING_PLACEHOLDER2}$3"),
            )
            .into_owned();
        text = p
            .dotted
            .replace_all(&text, format!("$1.{NON_BREAKING_PLACEHOLDER2}$2"))
            .into_owned();

        text = p
            .abbr_nar1
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        text = p
            .abbr_nar2
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}$2"),
            )
            .into_owned();
        text = p
            .abbr_2_small
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}$2.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        for re in [&p.abbr_vo1, &p.abbr_vo2, &p.abbr_vo3] {
            text = re
                .replace_all(
                    &text,
                    format!("$1{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}$2{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}"),
                )
                .into_owned();
        }
        for re in [&p.abbr_art, &p.abbr_man] {
            text = re
                .replace_all(
                    &text,
                    format!("$1{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}$2"),
                )
                .into_owned();
        }
        text = p
            .abbr_tys1
            .replace_all(
                &text,
                format!("$1$2{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        text = p
            .abbr_tys2
            .replace_all(
                &text,
                format!("$1{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}$2"),
            )
            .into_owned();
        text = p
            .abbr_vyd
            .replace_all(
                &text,
                format!("$1$3{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        text = p
            .abbr_lat
            .replace_all(
                &text,
                format!("$1{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}$2"),
            )
            .into_owned();
        text = p
            .abbr_prof
            .replace_all(
                &text,
                format!("$1{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}$2"),
            )
            .into_owned();
        text = p
            .abbr_gub
            .replace_all(
                &text,
                format!("$1{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        text = p
            .abbr_dash
            .replace_all(&text, format!("$1{NON_BREAKING_DOT_SUBST}$2"))
            .into_owned();

        text = p
            .initials_sp2
            .replace_all(
                &text,
                format!("$1{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}$2{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}$3"),
            )
            .into_owned();
        text = p
            .initials_sp1
            .replace_all(
                &text,
                format!("$1{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}$2"),
            )
            .into_owned();
        text = p
            .initials_rsp2
            .replace_all(
                &text,
                format!("$1{BREAKING_PLACEHOLDER}$2{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}$3{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        text = p
            .initials_rsp1
            .replace_all(
                &text,
                format!("$1{BREAKING_PLACEHOLDER}$2{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();

        text = p
            .abbr_kub_sm
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}$2"),
            )
            .into_owned();
        text = p
            .abbr_s_g
            .replace_all(
                &text,
                format!(
                    "$1{NON_BREAKING_DOT_SUBST}$2{NON_BREAKING_DOT_SUBST}{BREAKING_PLACEHOLDER}"
                ),
            )
            .into_owned();
        text = p
            .abbr_chl_kor
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}$2.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        text = p
            .abbr_nauk
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}$2.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        text = p
            .abbr_pn_zah
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}$2.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        for re in [&p.abbr_i_t_p, &p.abbr_i_t_ch, &p.abbr_t_zv] {
            text = re
                .replace_all(
                    &text,
                    format!("$1{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}$2{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}"),
                )
                .into_owned();
        }
        text = p
            .abbr_red_avt
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}$2"),
            )
            .into_owned();
        text = p
            .abbr_non_ending
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        text = p
            .abbr_non_ending2
            .replace_all(
                &text,
                format!("$1{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}$2"),
            )
            .into_owned();
        text = p
            .invalid_mln
            .replace_all(
                &text,
                format!("$1.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}$2"),
            )
            .into_owned();
    }

    if dot_inside_sentence {
        text = p
            .web_entities
            .replace_all(&text, format!("$1.{NON_BREAKING_PLACEHOLDER2}$2"))
            .into_owned();
        text = p
            .web_entities2
            .replace_all(
                &text,
                format!(".{NON_BREAKING_PLACEHOLDER2}$1.{NON_BREAKING_PLACEHOLDER2}$2"),
            )
            .into_owned();
    }

    text = p
        .abbr_ending
        .replace_all(
            &text,
            format!("$1.{NON_BREAKING_PLACEHOLDER2}{BREAKING_PLACEHOLDER}"),
        )
        .into_owned();

    if p.decimal_space.is_match(&text).unwrap_or(false) {
        let mut out = String::with_capacity(text.len());
        let mut last = 0usize;
        for m in p.decimal_space.find_iter(&text).flatten() {
            out.push_str(&text[last..m.start()]);
            let adjusted = m
                .as_str()
                .replace(' ', &NON_BREAKING_SPACE_SUBST.to_string());
            let adjusted = adjusted
                .replace('\u{00A0}', &NON_BREAKING_SPACE_SUBST.to_string())
                .replace('\u{202F}', &NON_BREAKING_SPACE_SUBST.to_string());
            out.push_str(&adjusted);
            last = m.end();
        }
        out.push_str(&text[last..]);
        text = out;
    }

    if text.contains(':') {
        text = p
            .colon_numbers
            .replace_all(&text, format!("$1{NON_BREAKING_COLON_SUBST}$2"))
            .into_owned();
    }

    if text.contains('(') {
        text = p
            .brace_in_word
            .replace_all(&text, format!("$1{LEFT_BRACE_SUBST}$2{RIGHT_BRACE_SUBST}"))
            .into_owned();
    }

    if text.contains('<') {
        text = p
            .xml_tag
            .replace_all(
                &text,
                format!("{BREAKING_PLACEHOLDER}{LEFT_ANGLE_SUBST}$1{RIGHT_ANGLE_SUBST}{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
        text = text.replace(
            &format!("{LEFT_ANGLE_SUBST}/"),
            &format!("{LEFT_ANGLE_SUBST}{SLASH_SUBST}"),
        );
        text = text.replace(
            &format!("/{RIGHT_ANGLE_SUBST}"),
            &format!("{SLASH_SUBST}{RIGHT_ANGLE_SUBST}"),
        );
    }

    if text.contains('-') {
        text = p
            .hyphen1
            .replace_all(&text, format!("$1{BREAKING_PLACEHOLDER}$2"))
            .into_owned();
        text = p
            .hyphen2
            .replace_all(&text, format!("$1{BREAKING_PLACEHOLDER}$2"))
            .into_owned();
    }

    if text.contains(SOFT_HYPHEN_WRAP) {
        text = p
            .soft_hyphen_wrap
            .replace_all(&text, SOFT_HYPHEN_WRAP_SUBST.to_string())
            .into_owned();
    }

    if text.contains('\'') {
        text = p
            .apostrophe_begin
            .replace_all(&text, format!("$1'{BREAKING_PLACEHOLDER}$2"))
            .into_owned();
        text = p
            .apostrophe_end
            .replace_all(&text, format!("$1{BREAKING_PLACEHOLDER}'$2"))
            .into_owned();
    }

    if text.contains('+') {
        text = p
            .plus
            .replace_all(
                &text,
                format!("{BREAKING_PLACEHOLDER}+{BREAKING_PLACEHOLDER}"),
            )
            .into_owned();
    }

    if text.chars().count() > 1 && (text.contains('-') || text.contains('\u{2013}')) {
        text = p
            .leading_num_dash
            .replace_all(&text, format!("$1{BREAKING_PLACEHOLDER}"))
            .into_owned();
    }

    text = p
        .number_missing_space
        .replace_all(&text, format!("$1{BREAKING_PLACEHOLDER}$2"))
        .into_owned();

    (text, urls)
}

/// Port of `UkrainianWordTokenizer.tokenize`.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut text = text.to_string();
    let mut urls: Vec<(String, String)> = Vec::new();
    if !text.trim().is_empty() {
        let (adjusted, found) = adjust_text_for_tokenizing(&text);
        text = adjusted;
        urls = found;
    }

    let mut token_list = Vec::new();
    for token in split_with_delimiters(&text, &patterns().split_chars) {
        if token == BREAKING_PLACEHOLDER.to_string() {
            continue;
        }
        let mut token = token
            .replace(DECIMAL_COMMA_SUBST, ",")
            .replace(NON_BREAKING_SLASH_SUBST, "/")
            .replace(NON_BREAKING_COLON_SUBST, ":")
            .replace(NON_BREAKING_SPACE_SUBST, " ")
            .replace(LEFT_BRACE_SUBST, "(")
            .replace(RIGHT_BRACE_SUBST, ")")
            .replace(LEFT_ANGLE_SUBST, "<")
            .replace(RIGHT_ANGLE_SUBST, ">")
            .replace(SLASH_SUBST, "/")
            .replace(NON_BREAKING_DOT_SUBST, ".")
            .replace(SOFT_HYPHEN_WRAP_SUBST, SOFT_HYPHEN_WRAP)
            .replace(
                &[NON_BREAKING_PLACEHOLDER, NON_BREAKING_PLACEHOLDER2][..],
                "",
            );
        for (key, url) in &urls {
            token = token.replace(key.as_str(), url);
        }
        token_list.push(token);
    }

    token_list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(text: &str) -> Vec<String> {
        tokenize(text)
    }

    /// Java-probed (`scripts/oracle/uk/probe-tokenizer.sh`): decimals, `с/г`,
    /// time, `%`, number-with-dash.
    #[test]
    fn uk_tokenizer_java_probed_samples() {
        assert_eq!(
            tokens("Це тест. Він має 5%-й результат, с/г, 12:25."),
            [
                "Це",
                " ",
                "тест",
                ".",
                " ",
                "Він",
                " ",
                "має",
                " ",
                "5%-й",
                " ",
                "результат",
                ",",
                " ",
                "с/г",
                ",",
                " ",
                "12:25",
                "."
            ]
        );
        assert_eq!(
            tokens("28.05.2019, 2 000 000 грн, 15:34, №5."),
            [
                "28.05.2019",
                ",",
                " ",
                "2 000 000",
                " ",
                "грн",
                ",",
                " ",
                "15:34",
                ",",
                " ",
                "№",
                "5",
                "."
            ]
        );
        assert_eq!(
            tokens("І. Франко, Т.Г. Шевченко, 2014 р., с. 5."),
            [
                "І.",
                " ",
                "Франко",
                ",",
                " ",
                "Т.",
                "Г.",
                " ",
                "Шевченко",
                ",",
                " ",
                "2014",
                " ",
                "р.",
                ",",
                " ",
                "с.",
                " ",
                "5",
                "."
            ]
        );
    }

    /// Apostrophes (`'`/`U+2019`) stay inside the word; the typographic forms
    /// are normalized to `'`.
    #[test]
    fn uk_tokenizer_apostrophes_and_entities() {
        assert_eq!(
            tokens("Харків’янин, з’їзд, п’ять, м’яч, Нью-Йорк."),
            [
                "Харків'янин",
                ",",
                " ",
                "з'їзд",
                ",",
                " ",
                "п'ять",
                ",",
                " ",
                "м'яч",
                ",",
                " ",
                "Нью-Йорк",
                "."
            ]
        );
        assert_eq!(
            tokens("<b>Жирний</b> текст"),
            ["<b>", "Жирний", "</b>", " ", "текст"]
        );
        assert_eq!(
            tokens("ВКПБ(о), 5², 10³, слово¹."),
            [
                "ВКПБ(о)",
                ",",
                " ",
                "5²",
                ",",
                " ",
                "10³",
                ",",
                " ",
                "слово",
                "¹",
                "."
            ]
        );
    }
}
