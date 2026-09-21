//! Slovak Java-coded built-in rules (`Slovak.getRelevantRules`): the generic
//! `WordRepeatRule` (5) with the `MessagesBundle_sk` strings, and
//! `CompoundRule` (`SK_COMPOUNDS`, 8).

/// `WordRepeatRule` (`WORD_REPEAT_RULE`) with the Slovak strings.
pub fn word_repeat_rule() -> crate::word_repeat::WordRepeatRule {
    crate::word_repeat::WordRepeatRule::new(crate::word_repeat::WordRepeatConfig {
        description: "Opakovanie slov (napr. 'bude bude')",
        message: "Možný preklep: zopakovali ste slovo",
        short_message: "Opakovanie slov",
        category_name: "Rôzne",
    })
}
