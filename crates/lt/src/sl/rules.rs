//! Slovenian Java-coded built-in rules (`Slovenian.getRelevantRules`): the
//! generic `WordRepeatRule` (5) with the `MessagesBundle_sl` strings.
//! Slovenian has no language-specific rule classes.

/// `WordRepeatRule` (`WORD_REPEAT_RULE`) with the Slovenian strings.
pub fn word_repeat_rule() -> crate::word_repeat::WordRepeatRule {
    crate::word_repeat::WordRepeatRule::new(crate::word_repeat::WordRepeatConfig {
        description: "Podvojena beseda (npr. 'bo bo')",
        message: "Možna tipkarska napaka: ponovili ste besedo",
        short_message: "Podvojena beseda",
        category_name: "Razno",
    })
}
