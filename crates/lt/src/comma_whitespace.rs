//! Port of `CommaWhitespaceRule` (`COMMA_PARENTHESIS_WHITESPACE`): periods,
//! commas and closing parentheses preceded by whitespace, opening parentheses
//! followed by whitespace. Works on the raw token view (whitespace tokens
//! included), like Java's `sentence.getTokens()`.

use lt_core::{AnalyzedTokenReadings, Match, Suggestion, TextRange};

const NO_SPACE_AFTER: &str = "Don't put a space after the opening parenthesis.";
const NO_SPACE_BEFORE: &str = "Don't put a space before the closing parenthesis.";
const NO_SPACE_AROUND_QUOTES: &str = "Don't put a space on both sides of a quote symbol.";
const MISSING_SPACE_AFTER_COMMA: &str = "Put a space after the comma.";
const SPACE_AFTER_COMMA: &str = "Put a space after the comma, but not before the comma.";
const NO_SPACE_BEFORE_DOT: &str = "Don't put a space before the full stop.";

fn file_extension_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^([a-z]{3,4}|[A-Z]{3,4}|ai|mp[34]|MP[34])(-.+)?$").unwrap()
    });
    &RE
}

fn domain_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)^(com|org|net|int|edu|gov|mil|[a-z]{2})$").unwrap()
    });
    &RE
}

fn is_whitespace_token(token: &AnalyzedTokenReadings) -> bool {
    // Java `AnalyzedTokenReadings.isWhitespace` is computed from the surface
    // (`StringTools.isWhitespace("")` is true), so the synthetic SENT_START
    // entry counts as whitespace here.
    (token.is_whitespace || token.is_sentence_start || token.surface() == "\u{00A0}")
        && token.surface() != "\u{200B}"
}

fn is_quote(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some('\'' | '"' | '’' | '”' | '“' | '«' | '»'), None)
    )
}

fn is_hyphen_or_comma(s: &str) -> bool {
    let mut chars = s.chars();
    matches!((chars.next(), chars.next()), (Some('-' | ','), None))
}

fn is_digit_or_dot(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c == '.' || c.is_numeric())
}

fn is_left_bracket(s: &str) -> bool {
    s.starts_with('(')
}

fn is_right_bracket(s: &str) -> bool {
    s.starts_with(')')
}

fn contains_digit(s: &str) -> bool {
    s.chars().any(|c| c.is_numeric())
}

fn is_proper_noun(token: &AnalyzedTokenReadings) -> bool {
    token.has_pos_tag("NNP")
        || token.has_pos_tag("NNPS")
        || token
            .readings
            .iter()
            .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with("NP")))
}

/// `CommaWhitespaceRule.match` over one sentence
/// (`quotesWhitespaceCheck = true`).
pub fn check_sentence(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    check_sentence_with_quotes(tokens, sentence_text, sentence_offset, true)
}

/// `CommaWhitespaceRule.match` over one sentence.
pub fn check_sentence_with_quotes(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
    quotes_whitespace: bool,
) -> Vec<Match> {
    let mut rule_matches: Vec<Match> = Vec::new();
    let mut prev_token = String::new();
    let mut prev_prev_token = String::new();
    let mut prev_white = false;
    for i in 0..tokens.len() {
        let token = tokens[i].surface().to_string();
        let is_whitespace = is_whitespace_token(&tokens[i]);
        let mut two_suggestions = false;

        let mut msg: Option<&str> = None;
        let mut suggestion_text: Option<String> = None;
        if is_whitespace && is_left_bracket(&prev_token) {
            let is_exception = i + 1 < tokens.len()
                && prev_token == "["
                && token == " "
                && tokens[i + 1].surface() == "]";
            if !is_exception {
                msg = Some(NO_SPACE_AFTER);
                suggestion_text = Some(prev_token.clone());
            }
        } else if quotes_whitespace
            && is_whitespace
            && is_quote(&prev_token)
            && prev_prev_token == " "
        {
            msg = Some(NO_SPACE_AROUND_QUOTES);
            suggestion_text = Some(prev_token.clone());
            two_suggestions = true;
        } else if !is_whitespace
            && prev_token == ","
            && !is_quote(&token)
            && !is_hyphen_or_comma(&token)
            && !contains_digit(&prev_prev_token)
            && !contains_digit(&token)
            && prev_prev_token != ","
        {
            msg = Some(MISSING_SPACE_AFTER_COMMA);
            suggestion_text = Some(format!(", {}", tokens[i].surface()));
        } else if prev_white {
            if is_right_bracket(&token) {
                let is_exception = token == "]" && prev_token == " " && prev_prev_token == "[";
                if !is_exception {
                    msg = Some(NO_SPACE_BEFORE);
                    suggestion_text = Some(token.clone());
                }
            } else if token == "," {
                msg = Some(SPACE_AFTER_COMMA);
                suggestion_text = Some(",".to_string());
                // exception for duplicated comma (we already have another rule)
                if i + 1 < tokens.len() && tokens[i + 1].surface() == "," {
                    msg = None;
                }
                if i + 1 < tokens.len() && !tokens[i + 1].is_whitespace {
                    suggestion_text = Some(", ".to_string());
                }
            } else if token == "."
                && !is_domain(tokens, i + 1)
                && !is_file_extension(tokens, i + 1)
                && !is_proper_noun(&tokens[i])
            {
                msg = Some(NO_SPACE_BEFORE_DOT);
                suggestion_text = Some(".".to_string());
                // exception for figures such as ".5" and ellipsis
                if i + 1 < tokens.len() && is_digit_or_dot(tokens[i + 1].surface()) {
                    msg = None;
                } else if i + 2 < tokens.len()
                    && tokens[i + 1].surface() == "/"
                    && tokens[i + 2]
                        .surface()
                        .chars()
                        .all(|c| c.is_ascii_alphabetic())
                    && !tokens[i + 2].surface().is_empty()
                {
                    // commands like "./validate.sh"
                    msg = None;
                }
            }
        }
        if let Some(msg) = msg {
            if !tokens[i].is_immunized {
                let mut from_pos = tokens[i - 1].start_pos;
                if two_suggestions {
                    from_pos = tokens[i - 2].start_pos;
                }
                let to_pos = tokens[i].end_pos();
                let suggestion_text = suggestion_text.unwrap_or_default();
                if to_pos < sentence_text.len() {
                    let marked = &sentence_text[from_pos..to_pos];
                    if marked == suggestion_text.as_str() && !two_suggestions {
                        prev_prev_token = prev_token;
                        prev_token = token;
                        prev_white = is_whitespace;
                        continue;
                    }
                }
                let suggestions = if two_suggestions {
                    vec![
                        Suggestion {
                            value: format!("{suggestion_text} "),
                            short_description: None,
                        },
                        Suggestion {
                            value: format!(" {suggestion_text}"),
                            short_description: None,
                        },
                    ]
                } else {
                    vec![Suggestion {
                        value: suggestion_text,
                        short_description: None,
                    }]
                };
                rule_matches.push(
                    Match::new(
                        "COMMA_PARENTHESIS_WHITESPACE",
                        Option::<String>::None,
                        msg,
                        Option::<String>::None,
                        TextRange::new(sentence_offset + from_pos, sentence_offset + to_pos),
                        suggestions,
                        "TYPOGRAPHY",
                        "Typography",
                    )
                    .with_metadata(
                        "Use of whitespace before comma and before/after parentheses",
                        "whitespace",
                        0,
                    ),
                );
            }
        }
        prev_prev_token = prev_token;
        prev_token = token;
        prev_white = is_whitespace;
    }
    rule_matches
}

fn is_domain(tokens: &[AnalyzedTokenReadings], i: usize) -> bool {
    i < tokens.len() && domain_re().is_match(tokens[i].surface())
}

fn is_file_extension(tokens: &[AnalyzedTokenReadings], i: usize) -> bool {
    i < tokens.len() && file_extension_re().is_match(tokens[i].surface())
}

/// `GermanCommaWhitespaceRule.match` (rule id stays
/// `COMMA_PARENTHESIS_WHITESPACE`, German messages and the
/// `<name>.<tld>-Domains` exception of `isException`).
pub fn check_sentence_de(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    const NO_SPACE_BEFORE_DOT_EN: &str = "Don't put a space before the full stop.";
    const NO_SPACE_BEFORE_DOT_DE: &str = "Vor dem Punkt sollte kein Leerzeichen stehen.";
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    // `isException`: a dot directly followed by "xyz-Domains?" is not an error
    matches.retain(|m| {
        let end = m.range.end - sentence_offset;
        let is_dot_match = tokens
            .iter()
            .position(|t| t.end_pos() == end && t.surface() == ".")
            .is_some();
        if !is_dot_match {
            return true;
        }
        let idx = tokens.iter().position(|t| t.end_pos() == end);
        match idx.and_then(|i| tokens.get(i + 1)) {
            Some(next) => !regex_domain().is_match(next.surface()),
            None => true,
        }
    });
    for m in &mut matches {
        m.category_name = "Zeichensetzung".to_string();
        m.description = "Leerzeichen vor/hinter Kommas und Klammern".to_string();
        if m.message == NO_SPACE_BEFORE_DOT_EN {
            m.message = NO_SPACE_BEFORE_DOT_DE.to_string();
        } else {
            m.message = translate_comma_message(&m.message);
        }
    }
    matches
}

/// `CommaWhitespaceRule(messages, false)` with the French
/// `MessagesBundle_fr` strings (French disables the quote-whitespace check).
pub fn check_sentence_fr(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence_with_quotes(tokens, sentence_text, sentence_offset, false);
    for m in &mut matches {
        m.category_name = "Typographie".to_string();
        m.description = "Espace devant « , », « ) » ou après « ( »".to_string();
        m.message = translate_comma_message_fr(&m.message);
    }
    matches
}

fn translate_comma_message_fr(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Ne placez pas d'espace après une parenthèse ouvrante.".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Ne placez pas d'espace avant une parenthèse fermante.".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Ne placez pas des espaces autour des guillemets.".to_string()
        }
        "Put a space after the comma." => "Insérez une espace après la virgule.".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Insérez une espace après la virgule et non avant.".to_string()
        }
        "Don't put a space before the full stop." => {
            "Ne placez pas d'espace avant le point.".to_string()
        }
        other => other.to_string(),
    }
}

/// `CommaWhitespaceRule` with the Spanish `MessagesBundle_es` strings
/// (Spanish uses the core class, no subclass override).
pub fn check_sentence_es(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Puntuación".to_string();
        m.description =
            "Espacios en blanco antes de coma y antes/después de paréntesis".to_string();
        m.message = translate_comma_message_es(&m.message);
    }
    matches
}

fn translate_comma_message_es(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "No se deja un espacio después de un paréntesis izquierdo.".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "No se deja un espacio antes de un paréntesis derecho.".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "No se deja un espacio a ambos lados de las comillas.".to_string()
        }
        "Put a space after the comma." => "Deje un espacio después de coma.".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Se deja un espacio después de coma y nunca antes del signo ortográfico.".to_string()
        }
        "Don't put a space before the full stop." => {
            "No se deja un espacio antes del punto.".to_string()
        }
        other => other.to_string(),
    }
}

/// `CommaWhitespaceRule` with the Italian `MessagesBundle_it` strings
/// (Italian uses the core class, no subclass override).
pub fn check_sentence_it(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Tipografia".to_string();
        m.description =
            "Utilizzo dello spazio prima della virgola e prima/dopo le parentesi".to_string();
        m.message = translate_comma_message_it(&m.message);
    }
    matches
}

fn translate_comma_message_it(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Non inserire lo spazio dopo l'apertura di parentesi".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Non inserire lo spazio dopo la chiusura di parentesi".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Don't put a space on both sides of a quote symbol".to_string()
        }
        "Put a space after the comma." => "Inserire uno spazio dopo la virgola".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Inserire lo spazio dopo la virgola e non prima".to_string()
        }
        "Don't put a space before the full stop." => {
            "Non inserire lo spazio dopo il punto a capo".to_string()
        }
        other => other.to_string(),
    }
}

/// `CommaWhitespaceRule` with the Portuguese strings
/// (`MessagesBundle_pt_PT`, or `MessagesBundle_pt_BR` for `pt-BR`).
pub fn check_sentence_pt(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
    variant: &str,
) -> Vec<Match> {
    let br = variant.starts_with("pt-BR");
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Tipografia".to_string();
        m.description = if br {
            "Uso de espaços em branco antes da vírgula e antes/depois de parênteses"
        } else {
            "Espaços junto a vírgulas e parênteses"
        }
        .to_string();
        m.message = translate_comma_message_pt(&m.message, br);
    }
    matches
}

/// Java `messages.getString` with the pt_PT / pt_BR `MessagesBundle`.
fn translate_comma_message_pt(msg: &str, br: bool) -> String {
    match (msg, br) {
        ("Don't put a space after the opening parenthesis.", false) => {
            "Não coloque um espaço depois de abrir parênteses".to_string()
        }
        ("Don't put a space after the opening parenthesis.", true) => {
            "Não insira um espaço depois de abrir parênteses".to_string()
        }
        ("Don't put a space before the closing parenthesis.", false) => {
            "Não coloque um espaço antes de fechar parênteses".to_string()
        }
        ("Don't put a space before the closing parenthesis.", true) => {
            "Não insira um espaço antes de fechar parênteses".to_string()
        }
        // pt_PT ships the English string untranslated
        ("Don't put a space on both sides of a quote symbol.", false) => {
            "Don't put a space on both sides of a quote symbol".to_string()
        }
        ("Don't put a space on both sides of a quote symbol.", true) => {
            "Não coloque um espaço em ambos os lados de um símbolo de citação".to_string()
        }
        ("Put a space after the comma.", false) => "Coloque um espaço após a vírgula".to_string(),
        ("Put a space after the comma.", true) => "Insira um espaço depois da vírgula".to_string(),
        ("Put a space after the comma, but not before the comma.", false) => {
            "Coloque um espaço após a vírgula, e não antes".to_string()
        }
        ("Put a space after the comma, but not before the comma.", true) => {
            "Insira um espaço depois da vírgula, mas não antes dela".to_string()
        }
        ("Don't put a space before the full stop.", false) => {
            "Não coloque um espaço antes do ponto final".to_string()
        }
        ("Don't put a space before the full stop.", true) => {
            "Não insira um espaço antes do ponto final.".to_string()
        }
        _ => msg.to_string(),
    }
}

/// `CommaWhitespaceRule` with the Dutch `MessagesBundle_nl` strings.
pub fn check_sentence_nl(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Interpunctie".to_string();
        m.description = "Spatie voor of achter haakje".to_string();
        m.message = translate_comma_message_nl(&m.message);
        m.short_message = Some(m.message.clone());
    }
    matches
}

/// Java `messages.getString` with the `MessagesBundle_ro` bundle.
fn translate_comma_message_ro(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Nu puneți spațiu după deschiderea parantezei".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Nu puneți spațiu după închiderea parantezei".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Don't put a space on both sides of a quote symbol".to_string()
        }
        "Put a space after the comma." => "Puneți un spațiu după virgulă".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Pune un spațiu după virgulă, dar nu înainte de virgulă".to_string()
        }
        "Don't put a space before the full stop." => {
            "Nu puneți spațiu înainte de punct".to_string()
        }
        _ => msg.to_string(),
    }
}

/// `CommaWhitespaceRule` with the Romanian `MessagesBundle_ro` strings.
pub fn check_sentence_ro(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Typography".to_string();
        m.description = "Spații puse înainte de virgulă sau înainte/după paranteze".to_string();
        m.message = translate_comma_message_ro(&m.message);
    }
    matches
}

/// `CommaWhitespaceRule` with the Catalan `MessagesBundle_ca` strings.
pub fn check_sentence_ca(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Tipografia".to_string();
        m.description = "Espais abans de coma i abans i després dels parèntesis".to_string();
        m.message = translate_comma_message_ca(&m.message);
        m.short_message = Some(m.message.clone());
    }
    matches
}

/// `CommaWhitespaceRule` with the Galician `MessagesBundle_gl` strings.
pub fn check_sentence_gl(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Tipografía".to_string();
        m.description =
            "Uso de espazos en branco diante dunha coma ou antes/despois de paréntese".to_string();
        m.message = translate_comma_message_gl(&m.message);
    }
    matches
}

/// `CommaWhitespaceRule` with the Polish `MessagesBundle_pl` strings.
pub fn check_sentence_pl(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Błędy typograficzne".to_string();
        m.description = "Odstępy przed przecinkami oraz przed nawiasami i po nawiasach".to_string();
        m.message = translate_comma_message_pl(&m.message);
    }
    matches
}

/// Java `messages.getString` with the `MessagesBundle_pl` bundle.
fn translate_comma_message_pl(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Nie wstawiamy spacji po nawiasie otwierającym".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Nie wstawiamy spacji przed nawiasem zamykającym".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Nie wstawiaj spacji po obu stronach cudzysłowu.".to_string()
        }
        "Put a space after the comma." => "Po przecinku wstawiamy spację".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Spację wstawiamy po przecinku, nie przed przecinkiem".to_string()
        }
        "Don't put a space before the full stop." => {
            "Nie wstawiamy spacji przed kropką".to_string()
        }
        _ => msg.to_string(),
    }
}

/// `CommaWhitespaceRule` with the Slovak `MessagesBundle_sk` strings.
pub fn check_sentence_sk(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Interpunkcia".to_string();
        m.description = "Použitie medzery pred čiarkou a pred/za zátvorkami".to_string();
        m.message = translate_comma_message_sk(&m.message);
    }
    matches
}

/// `CommaWhitespaceRule` with the Slovenian `MessagesBundle_sl` strings.
pub fn check_sentence_sl(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Postavitev ločil".to_string();
        m.description =
            "Uporaba presledka, tabulatorja ali preloma vrstice pred vejico in pred/po oklepaju"
                .to_string();
        m.message = translate_comma_message_sl(&m.message);
    }
    matches
}

/// `CommaWhitespaceRule` with the Greek `MessagesBundle_el` strings.
pub fn check_sentence_el(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Στίξη".to_string();
        m.description = "Χρήση κενού πριν από κόμμα και πρίν/μετά από παρένθεση".to_string();
        m.message = translate_comma_message_el(&m.message);
    }
    matches
}

/// `CommaWhitespaceRule` with the Danish `MessagesBundle_da` strings.
pub fn check_sentence_da(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Typografi".to_string();
        m.description = "Mellemrum før komma og før/efter parenteser".to_string();
        m.message = translate_comma_message_da(&m.message);
    }
    matches
}

/// Java `messages.getString` with the `MessagesBundle_da` bundle.
fn translate_comma_message_da(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Indsæt ikke et mellemrum efter parentesbegynd".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Indsæt ikke et mellemrum før parentesslut".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Don't put a space on both sides of a quote symbol".to_string()
        }
        "Put a space after the comma." => "Indsæt et mellemrum efter kommaet".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Indsæt ikke et mellemrum før komma, men efter det.".to_string()
        }
        "Don't put a space before the full stop." => {
            "Indsæt ikke et mellemrum før punktum".to_string()
        }
        _ => msg.to_string(),
    }
}

/// `CommaWhitespaceRule` with the Swedish `MessagesBundle_sv` strings.
pub fn check_sentence_sv(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Typografi".to_string();
        m.description = "Blanktecken före kommatecken samt före/efter parentes".to_string();
        m.message = translate_comma_message_sv(&m.message);
    }
    matches
}

/// Java `messages.getString` with the `MessagesBundle_sv` bundle.
fn translate_comma_message_sv(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Använd inte blanksteg efter öppnande parentes.".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Använd inte blanksteg före avslutande parentes.".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Använd inte blanksteg på båda sidor om kvoteringssymboler.".to_string()
        }
        "Put a space after the comma." => "Lägg till ett blanksteg efter kommatecknet.".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Lägg till ett blanksteg efter kommatecknet, men inte före.".to_string()
        }
        "Don't put a space before the full stop." => {
            "Använd inte blanksteg före punkt.".to_string()
        }
        _ => msg.to_string(),
    }
}

/// `CommaWhitespaceRule` with the Icelandic `MessagesBundle_is` strings.
pub fn check_sentence_is(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
) -> Vec<Match> {
    let mut matches = check_sentence(tokens, sentence_text, sentence_offset);
    for m in &mut matches {
        m.category_name = "Punctuation".to_string();
        m.description = "Bil á undan kommu og á undan/eftir sviga".to_string();
        m.message = translate_comma_message_is(&m.message);
    }
    matches
}

/// Java `messages.getString` with the `MessagesBundle_is` bundle.
fn translate_comma_message_is(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Ekki setja bil eftir að svigi er opnaður".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Ekki setja bil áður en sviga er lokað".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Don't put a space on both sides of a quote symbol".to_string()
        }
        "Put a space after the comma." => "Bil vantar á eftir kommu".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Bil skal vera á eftir kommu, ekki á undan henni".to_string()
        }
        "Don't put a space before the full stop." => "Ekki setja bil á undan punkti".to_string(),
        _ => msg.to_string(),
    }
}

/// Java `messages.getString` with the `MessagesBundle_sk` bundle.
fn translate_comma_message_sk(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Nevložiť medzeru za otváraciu zátvorku".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Nedávajte medzeru pred ukončovaciu zátvorku".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Don't put a space on both sides of a quote symbol".to_string()
        }
        "Put a space after the comma." => "Vložte medzeru za čiarku".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Vložte medzeru za čiarku, ale nie pred čiarku".to_string()
        }
        "Don't put a space before the full stop." => "Nedávajte medzeru pred bodku".to_string(),
        _ => msg.to_string(),
    }
}

/// Java `messages.getString` with the `MessagesBundle_sl` bundle.
fn translate_comma_message_sl(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Ne postavljaj presledka za oklepaj".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Ne postavljaj presledka pred zaklepaj".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Ne postavljaj presledka na obe strani narekovaja".to_string()
        }
        "Put a space after the comma." => "Po vejici vstavi presledek".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Presledek vstavi po vejici, ne pa pred vejico".to_string()
        }
        "Don't put a space before the full stop." => "Ne postavljaj presledka po piki".to_string(),
        _ => msg.to_string(),
    }
}

/// Java `messages.getString` with the `MessagesBundle_el` bundle.
fn translate_comma_message_el(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Μην βάλετε κενό μετά το άνοιγμα παρένθεσης".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Μην βάλετε κενό πριν το κλείσιμο παρένθεσης".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Don't put a space on both sides of a quote symbol".to_string()
        }
        "Put a space after the comma." => "Προσθέστε ένα κενό μετά το κόμμα".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Προσθέστε ένα κενό μετά το κόμμα αλλά όχι πριν το κόμμα.".to_string()
        }
        "Don't put a space before the full stop." => "Μην βάλετε κενό πριν από τελεία".to_string(),
        _ => msg.to_string(),
    }
}

/// Java `messages.getString` with the `MessagesBundle_gl` bundle.
fn translate_comma_message_gl(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Non deixe espazos detrás da paréntese de apertura.".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Non deixe espazos antes da paréntese de peche.".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Non poña un espazo a ambos os lados dunhas aspas.".to_string()
        }
        "Put a space after the comma." => "Poña un espazo detrás da coma.".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Poña un espazo en branco despois da coma, pero nunca antes.".to_string()
        }
        "Don't put a space before the full stop." => {
            "Non poña espazos antes dun punto.".to_string()
        }
        _ => msg.to_string(),
    }
}

/// Java `messages.getString` with the `MessagesBundle_ca` bundle.
fn translate_comma_message_ca(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "No poseu cap espai després dels parèntesis d'obertura.".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "No poseu cap espai abans dels parèntesis de tancament.".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "No poseu un espai a banda i banda d'un caràcter de cometes.".to_string()
        }
        "Put a space after the comma." => "Deixeu un espai després de la coma.".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Deixeu un espai després de la coma però no abans.".to_string()
        }
        "Don't put a space before the full stop." => {
            "No deixeu cap espai abans del punt.".to_string()
        }
        _ => msg.to_string(),
    }
}
fn translate_comma_message_nl(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Zet geen spatie na een haakje openen".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Zet geen spatie voor een haakje sluiten".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Gebruik geen spatie aan beide zijden van een aanhalingsteken".to_string()
        }
        "Put a space after the comma." => "Zet een spatie na de komma".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "Zet een spatie na een komma, maar niet ervoor".to_string()
        }
        "Don't put a space before the full stop." => "Zet geen spatie voor een punt".to_string(),
        _ => msg.to_string(),
    }
}

fn translate_comma_message(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "Hinter einer öffnenden Klammer wird kein Leerzeichen eingefügt.".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "Vor einer schließenden Klammer wird kein Leerzeichen eingefügt.".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "Ein Leerzeichen sollte nicht auf beiden Seiten eines Anführungszeichens stehen."
                .to_string()
        }
        "Put a space after the comma." => {
            "Hinter einem Komma sollte ein Leerzeichen stehen.".to_string()
        }
        "Put a space after the comma, but not before the comma." => {
            "Nur hinter einem Komma steht ein Leerzeichen, aber nicht davor.".to_string()
        }
        other => other.to_string(),
    }
}

fn regex_domain() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^[a-z]{2,10}-Domains?$").unwrap());
    &RE
}
