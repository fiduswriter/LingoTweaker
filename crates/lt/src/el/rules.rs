//! Greek Java-coded built-in rules (`Greek.getRelevantRules`): the generic
//! `WordRepeatRule` (9) with the `MessagesBundle_el` strings. The Greek-only
//! classes (word-repeat-beginning, homonyms, specific case, numeral stress and
//! redundancy) are wired in stage 3.

/// `WordRepeatRule` (`WORD_REPEAT_RULE`) with the Greek strings.
pub fn word_repeat_rule() -> crate::word_repeat::WordRepeatRule {
    crate::word_repeat::WordRepeatRule::new(crate::word_repeat::WordRepeatConfig {
        description: "Επανάληψη λέξης (π.χ. 'και και')",
        message: "Πιθανό λάθος: επαναλάβατε μία λέξη",
        short_message: "Επανάληψη λέξης",
        category_name: "Διάφορα",
    })
}
