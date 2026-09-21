//! Port of `MultipleWhitespaceRule` (`WHITESPACE_RULE`): duplicated
//! whitespace inside a sentence.

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};

pub const RULE_ID: &str = "WHITESPACE_RULE";
const DE_DESCRIPTION: &str = "Wiederholung von Leerzeichen";
const DE_MESSAGE: &str = "Möglicher Tippfehler: mehr als ein Leerzeichen hintereinander";
const DESCRIPTION: &str = "Whitespace repetition (bad formatting)";
const MESSAGE: &str = "Possible typo: you repeated a whitespace";

/// `AnalyzedTokenReadings.isLinebreak`.
fn is_linebreak(token: &str) -> bool {
    matches!(token, "\n" | "\r\n" | "\r" | "\n\r")
}

/// `MultipleWhitespaceRule.isFirstWhite`.
fn is_first_white(token: &AnalyzedTokenReadings) -> bool {
    (token.is_whitespace || token.surface() == "\u{00A0}")
        && !is_linebreak(token.surface())
        && !token.surface().contains('\u{200B}')
        && !token.surface().contains('\u{FEFF}')
        && !token.surface().contains('\u{2060}')
}

/// `MultipleWhitespaceRule.isRemovableWhite`.
fn is_removable_white(token: &AnalyzedTokenReadings) -> bool {
    (token.is_whitespace || token.surface() == "\u{00A0}")
        && !is_linebreak(token.surface())
        && token.surface() != "\t"
        && !token.surface().contains('\u{200B}')
        && !token.surface().contains('\u{FEFF}')
        && !token.surface().contains('\u{2060}')
}

/// `MultipleWhitespaceRule.match` over all sentences (German messages).
pub fn check_de(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        DE_DESCRIPTION,
        DE_MESSAGE,
        ("TYPOGRAPHY", "Typografie"),
    )
}

/// `MultipleWhitespaceRule` with the French `MessagesBundle_fr` strings.
pub fn check_fr(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Plusieurs espaces vides (mauvais format)",
        "Faute de frappe possible : une espace est répétée",
        ("TYPOGRAPHY", "Typographie"),
    )
}

/// `MultipleWhitespaceRule` with the Spanish `MessagesBundle_es` strings.
pub fn check_es(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Múltiples espacios en blanco (formato incorrecto)",
        "Posible error tipográfico: múltiples espacios en blanco",
        ("TYPOGRAPHY", "Tipografía"),
    )
}

/// `MultipleWhitespaceRule` with the Italian `MessagesBundle_it` strings.
pub fn check_it(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Ripetizione dello spazio (brutta formattazione)",
        "Probabile errore: ripetizione di spazio",
        ("TYPOGRAPHY", "Tipografia"),
    )
}

/// `MultipleWhitespaceRule` with the Portuguese strings
/// (`MessagesBundle_pt_PT`, or `MessagesBundle_pt_BR` for `pt-BR`).
pub fn check_pt(sentences: &[AnalyzedSentence], variant: &str) -> Vec<Match> {
    if variant.starts_with("pt-BR") {
        check_with(
            sentences,
            "Repetição de espaço em branco (formatação incorreta)",
            "Possível erro de escrita: você repetiu um espaço em branco",
            ("TYPOGRAPHY", "Tipografia"),
        )
    } else {
        check_with(
            sentences,
            "Espaços em branco múltiplos",
            "Possível erro: repetiu um espaço",
            ("TYPOGRAPHY", "Tipografia"),
        )
    }
}

/// `MultipleWhitespaceRule` with the Dutch `MessagesBundle_nl` strings.
pub fn check_nl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Te veel witruimte tussen woorden",
        "Te veel witruimte",
        ("TYPOGRAPHY", "Typografie"),
    )
}

/// `MultipleWhitespaceRule` with the Catalan `MessagesBundle_ca` strings.
pub fn check_ca(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Repetició d'espais en blanc (formatació incorrecta)",
        "Possible error: heu repetit un espai en blanc",
        ("TYPOGRAPHY", "Tipografia"),
    )
}

/// `MultipleWhitespaceRule` with the Romanian `MessagesBundle_ro` strings.
pub fn check_ro(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Spațiu repetat (formatare greșită)",
        "Posibilă greșeală: ați repetat un spațiu",
        ("TYPOGRAPHY", "Typography"),
    )
}

/// `MultipleWhitespaceRule` with the Slovak `MessagesBundle_sk` strings.
pub fn check_sk(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Opakovanie \"bielych znakov\" napr. medzier (zlé formátovanie)",
        "Možný preklep: zopakovali ste \"biely znak\" (whitespace)",
        ("TYPOGRAPHY", "Typografia"),
    )
}

/// `MultipleWhitespaceRule` with the Slovenian `MessagesBundle_sl` strings.
pub fn check_sl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Ponovljen presledek (nepravilno oblikovanje)",
        "Možna tipkarska napaka: ponovili ste presledek",
        ("TYPOGRAPHY", "Tipografija"),
    )
}

/// `MultipleWhitespaceRule` with the Galician `MessagesBundle_gl` strings.
pub fn check_gl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Repetición de espazos en branco (erro de formato)",
        "Posíbel erro tipográfico: repetiu un espazo en branco",
        ("TYPOGRAPHY", "Tipografía"),
    )
}

/// `MultipleWhitespaceRule` with the Greek `MessagesBundle_el` strings.
pub fn check_el(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Επανάληψη κενού",
        "Πιθανό λάθος: επανάληψη κενού",
        ("TYPOGRAPHY", "Τυπογραφικά"),
    )
}

/// `MultipleWhitespaceRule` with the Danish `MessagesBundle_da` strings.
pub fn check_da(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Gentagende mellemrum (dårlig formatering)",
        "Mulig slåfejl: du har gentaget et mellemrum",
        ("TYPOGRAPHY", "Typografi"),
    )
}

/// `MultipleWhitespaceRule` with the Swedish `MessagesBundle_sv` strings.
pub fn check_sv(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Upprepade blanktecken (dålig formatering)",
        "Möjligt korrekturfel: du upprepade ett blanktecken",
        ("TYPOGRAPHY", "Typografi"),
    )
}

/// `MultipleWhitespaceRule` with the Asturian `MessagesBundle_ast` strings.
pub fn check_ast(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Repetición d'espacios",
        "Posible error: repitisti un espaciu",
        ("TYPOGRAPHY", "Tipografía"),
    )
}

/// `MultipleWhitespaceRule` with the Breton `MessagesBundle_br` strings.
pub fn check_br(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Spas-gwenn doublet (furmad fall)",
        "Fazi bizskrivañ posupl: daou spas ho peus lakaet",
        ("TYPOGRAPHY", "Lizherennerezh"),
    )
}

/// `MultipleWhitespaceRule` with the Tagalog `MessagesBundle_tl` strings.
pub fn check_tl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Pag-uulit ng Whitespace (pangit na pag-format)",
        "Posibleng typo: naulit mo ang whitespace",
        ("TYPOGRAPHY", "Typography"),
    )
}

/// `MultipleWhitespaceRule` with the Lithuanian `MessagesBundle_lt` strings.
pub fn check_lt(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Tarpų pasikartojimas (blogas formatavimas)",
        "Galima rinkimo klaida: pakartojote tarpą",
        ("TYPOGRAPHY", "Typography"),
    )
}

/// `MultipleWhitespaceRule` with the Esperanto `MessagesBundle_eo` strings.
pub fn check_eo(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Ripetita spaceto (neĝusta formato)",
        "Ebla mistajpaĵo: vi ripetis spaceton",
        ("TYPOGRAPHY", "Tipografio"),
    )
}

/// `MultipleWhitespaceRule` with the Icelandic `MessagesBundle_is` strings.
pub fn check_is(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Tvöfalt bil (galli á uppsetningu)",
        "Hugsanleg ritvilla: endurtekið bil",
        ("TYPOGRAPHY", "Typography"),
    )
}

/// `MultipleWhitespaceRule` with the Polish `MessagesBundle_pl` strings.
pub fn check_pl(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Powtórzenie spacji (błędne formatowanie)",
        "Prawdopodobna literówka: wiele spacji z rzędu",
        ("TYPOGRAPHY", "Błędy typograficzne"),
    )
}

/// `MultipleWhitespaceRule.match` over all sentences.
pub fn check(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        DESCRIPTION,
        MESSAGE,
        ("TYPOGRAPHY", "Typography"),
    )
}

fn check_with(
    sentences: &[AnalyzedSentence],
    description: &str,
    message: &str,
    category: (&str, &str),
) -> Vec<Match> {
    let mut rule_matches = Vec::new();
    for sentence in sentences {
        let tokens = &sentence.tokens;
        let mut i = 1usize;
        while i < tokens.len() {
            if is_first_white(&tokens[i]) {
                let n_first = i;
                i += 1;
                while i < tokens.len() && is_removable_white(&tokens[i]) {
                    i += 1;
                }
                i -= 1;
                if i > n_first {
                    rule_matches.push(
                        Match::new(
                            RULE_ID,
                            Option::<String>::None,
                            message,
                            Option::<String>::None,
                            TextRange::new(
                                sentence.offset + tokens[n_first].start_pos,
                                sentence.offset + tokens[i].end_pos(),
                            ),
                            vec![Suggestion {
                                value: tokens[n_first].surface().to_string(),
                                short_description: None,
                            }],
                            category.0,
                            category.1,
                        )
                        .with_metadata(description, "whitespace", 0),
                    );
                }
            } else if is_linebreak(tokens[i].surface()) {
                i += 1;
                while i < tokens.len() && is_removable_white(&tokens[i]) {
                    i += 1;
                }
            }
            i += 1;
        }
    }
    rule_matches
}

/// `MultipleWhitespaceRule` with the Belarusian `MessagesBundle_be` strings.
pub fn check_be(sentences: &[AnalyzedSentence]) -> Vec<Match> {
    check_with(
        sentences,
        "Паўтарэнне прабелаў (дрэннае фарматаванне)",
        "Магчымая памылка друку: вы паўтарылі прабел",
        ("TYPOGRAPHY", "Тыпаграфіка"),
    )
}
