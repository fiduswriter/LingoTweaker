//! Arabic punctuation/whitespace Java rules: `ArabicCommaWhitespaceRule`,
//! `ArabicQuestionMarkWhitespaceRule`, `ArabicSemiColonWhitespaceRule`
//! (all `CommaWhitespaceRule` with an Arabic comma character) and
//! `ArabicDoublePunctuationRule` (`DoublePunctuationRule`).

use lt_core::{AnalyzedTokenReadings, Match};

use crate::comma_whitespace;
use crate::double_punctuation;

/// `MessagesBundle_ar` strings for the `CommaWhitespaceRule` messages.
fn translate_comma_message(msg: &str) -> String {
    match msg {
        "Don't put a space after the opening parenthesis." => {
            "لا تضع فراغا بعد القوس المفتوح.".to_string()
        }
        "Don't put a space before the closing parenthesis." => {
            "لا تضع فراغا قبل القوس المغلق.".to_string()
        }
        "Don't put a space on both sides of a quote symbol." => {
            "لا تضع مسافة على كلا جانبي رمز التنصيص.".to_string()
        }
        "Put a space after the comma." => "ضع فراغا بعد الفاصلة.".to_string(),
        "Put a space after the comma, but not before the comma." => {
            "ضع فراغا بعد الفاصلة، وليس قبلها.".to_string()
        }
        "Don't put a space before the full stop." => "لا تضع فراغا قبل النقطة.".to_string(),
        _ => msg.to_string(),
    }
}

/// `CommaWhitespaceRule` subclass with the given comma character and rule id
/// (`ARABIC_COMMA_PARENTHESIS_WHITESPACE` `،`, `ARABIC_QM_WHITESPACE` `؟`,
/// `ARABIC_SC_WHITESPACE` `؛`; the generic `COMMA_PARENTHESIS_WHITESPACE`
/// passes `,`).
pub fn comma_whitespace(
    tokens: &[AnalyzedTokenReadings],
    sentence_text: &str,
    sentence_offset: usize,
    comma: &str,
    rule_id: &str,
) -> Vec<Match> {
    let mut matches = comma_whitespace::check_sentence_with_comma(
        tokens,
        sentence_text,
        sentence_offset,
        true,
        comma,
        rule_id,
        None,
    );
    for m in &mut matches {
        m.category_name = "علامات الترقيم".to_string();
        m.description = "استعمال فراغات قبل الفاصلة أو قبل الأقواس أو بعدها".to_string();
        m.message = translate_comma_message(&m.message);
    }
    matches
}

/// `ArabicDoublePunctuationRule` (`ARABIC_DOUBLE_PUNCTUATION`, comma `،`).
pub fn double_punctuation(tokens: &[AnalyzedTokenReadings], sentence_offset: usize) -> Vec<Match> {
    let mut matches = double_punctuation::check_sentence_with_comma(
        tokens,
        sentence_offset,
        "،",
        "ARABIC_DOUBLE_PUNCTUATION",
    );
    for m in &mut matches {
        m.category_name = "علامات الترقيم".to_string();
        m.description = "استعمال نقطتين أو فاصلتين متتابعتين".to_string();
        m.message = match m.message.as_str() {
            "Two consecutive dots" => "نقطتان متتابعتان".to_string(),
            "Two consecutive commas" => "فاصلتان متتابعتان".to_string(),
            _ => m.message.clone(),
        };
        if let Some(short) = &m.short_message {
            m.short_message = Some(match short.as_str() {
                "Two consecutive dots" => "نقطتان متتابعتان".to_string(),
                "Two consecutive commas" => "فاصلتان متتابعتان".to_string(),
                _ => short.clone(),
            });
        }
    }
    matches
}
