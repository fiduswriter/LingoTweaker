//! Ukrainian engine tests. Stage 1 pins the XML wiring state (active rules and
//! the single unmapped `<filter>` class) and the generic `MultipleWhitespace`
//! built-in; the custom tokenizer/tagger/disambiguator, the speller and the
//! language's Java rule classes are added in stages 2/3.
//!
//! `Ukrainian` loads `grammar.xml` plus `Ukrainian.RULE_FILES`
//! (`grammar-spelling`, `grammar-grammar`, `grammar-barbarism`,
//! `grammar-style`, `grammar-punctuation`); the only XML-referenced filter is
//! `org.languagetool.rules.uk.DateCheckFilter` (ported in stage 3).

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, EngineOptions, Lang};

mod common;
use common::assert_utf16;

fn engine_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn data_dir() -> Option<DataDir> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    if !path.is_dir() {
        eprintln!("skipping: no vendored data directory found");
        return None;
    }
    Some(DataDir::new(path))
}

fn engine() -> Option<Engine> {
    let data = data_dir()?;
    Engine::builder(Lang::Uk).ok()?.data_dir(data).build().ok()
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Uk)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn one(text: &str, rule: &str) -> Vec<lt::Match> {
    let Some(uk) = engine_with_rules(&[rule]) else {
        eprintln!("skipping: no vendored data");
        return Vec::new();
    };
    uk.check(text)
        .expect("check")
        .matches
        .into_iter()
        .filter(|m| m.rule_id == rule)
        .collect()
}

fn suggestions(m: &lt::Match) -> Vec<String> {
    m.suggestions.iter().map(|s| s.value.clone()).collect()
}

/// Stage state: 1,248 default-active XML rules, the XML-referenced
/// `uk.DateCheckFilter` compiled and `compile_failures()` = 0.
#[test]
fn ukrainian_engine_state() {
    let _guard = engine_guard();
    let Some(uk) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    assert_eq!(uk.active_rule_count(), 1248);
    assert_eq!(uk.skipped_counts().filters, 0);
    assert!(
        uk.compile_failures().is_empty(),
        "{:?}",
        uk.compile_failures()
    );
}

/// `DATE_WEEKDAY1` + `uk.DateCheckFilter` (Java-probed via
/// `scripts/oracle/uk/probe-rule.sh`): `понеділок, 7 жовтня 2014` was a
/// Tuesday.
#[test]
fn ukrainian_date_weekday_filter() {
    let _guard = engine_guard();
    let text = "Конференція відбудеться в понеділок, 7 жовтня 2014 р.";
    let matches = one(text, "DATE_WEEKDAY1");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (26, 50));
    assert_eq!(
        matches[0].message,
        "Днем тижня 7 жовтня 2014 є не понеділок, а вівторок."
    );
    assert_eq!(matches[0].category_id, "LOGICAL_ERRORS");
}

/// `MultipleWhitespaceRule` with the `MessagesBundle_uk` strings.
#[test]
fn ukrainian_multiple_whitespace() {
    let _guard = engine_guard();
    let text = "Це  тест.";
    let matches = one(text, "WHITESPACE_RULE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "Ймовірна помилка: повтор пробілу");
    assert_eq!(matches[0].description, "Повтор пробілу");
    assert_eq!(matches[0].category_id, "TYPOGRAPHY");
    assert_eq!(matches[0].category_name, "Оформлення");
    assert_utf16(text, &matches[0], (2, 4));
}

/// `MORFOLOGIK_RULE_UK_UA` over `uk/hunspell/uk_UA.dict`
/// (`scripts/oracle/uk/probe-speller.sh`): `кампутар` -> `Каптар`/`кампусам`/
/// `кампусах` (Java-probed list and UTF-16 range).
#[test]
fn ukrainian_speller_кампутар() {
    let _guard = engine_guard();
    let text = "кампутар";
    let matches = one(text, "MORFOLOGIK_RULE_UK_UA");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 8));
    assert_eq!(
        matches[0].message,
        "Знайдено потенційну орфографічну помилку."
    );
    assert_eq!(
        matches[0].short_message.as_deref(),
        Some("Орфографічна помилка")
    );
    assert_eq!(matches[0].description, "Ймовірна орфографічна помилка");
    assert_eq!(matches[0].category_id, "TYPOS");
    assert_eq!(matches[0].category_name, "Можлива механічна помилка");
    assert_eq!(
        suggestions(&matches[0]),
        vec!["Каптар", "кампусам", "кампусах"]
    );
}

/// The 2019 `dash_prefixes.txt` additional suggestions (Java-probed): the
/// dash-split form is appended after the speller suggestions.
#[test]
fn ukrainian_speller_dash_prefix_suggestions() {
    let _guard = engine_guard();
    for (text, expected_last) in [("блогерр", "блог-ерр"), ("бізнесменн", "бізнес-менн")]
    {
        let matches = one(text, "MORFOLOGIK_RULE_UK_UA");
        assert_eq!(matches.len(), 1, "{text}");
        let got = suggestions(&matches[0]);
        assert_eq!(
            got.last().map(String::as_str),
            Some(expected_last),
            "{text}: {got:?}"
        );
    }
}

/// A correctly spelled dictionary word is clean (the speller ignores tagged
/// tokens).
#[test]
fn ukrainian_speller_accepts_known_words() {
    let _guard = engine_guard();
    let Some(uk) = engine_with_rules(&["MORFOLOGIK_RULE_UK_UA"]) else {
        eprintln!("skipping: no vendored data");
        return;
    };
    for text in ["автомобіль", "тест", "кінопрокат", "їжак"] {
        let result = uk.check(text).expect("check");
        assert!(result.matches.is_empty(), "{text}: {:?}", result.matches);
    }
}

/// Java-probed (`scripts/oracle/uk/probe-disambig.sh`) disambiguation output,
/// restricted to content tokens (SENT_START/SENT_END excluded).
fn disambiguated(uk: &lt::Engine, text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for sentence in uk.analyze(text) {
        for tr in sentence.tokens_without_whitespace() {
            if tr.is_sentence_start {
                continue;
            }
            let readings: Vec<String> = tr
                .readings
                .iter()
                .filter(|a| !matches!(a.pos_tag.as_deref(), Some("SENT_END") | Some("PARA_END")))
                .map(|a| {
                    format!(
                        "{}:{}",
                        a.stem.as_deref().unwrap_or(""),
                        a.pos_tag.as_deref().unwrap_or("")
                    )
                })
                .collect();
            if readings.is_empty() {
                continue;
            }
            out.push((tr.surface().to_string(), readings.join(";")));
        }
    }
    out
}

/// `UkrainianHybridDisambiguator.preDisambiguate` passes, Java-probed.
#[test]
fn ukrainian_hybrid_disambiguation() {
    let _guard = engine_guard();
    let Some(uk) = engine() else {
        eprintln!("skipping: no vendored data");
        return;
    };
    let got = disambiguated(&uk, "Прийшов їх син.");
    assert_eq!(got.len(), 3, "{got:?}");
    assert_eq!(
        got[1].1,
        "їх:adj:m:v_naz:nv:pron:pos:bad;вони:noun:unanim:p:v_rod:pron:pers:3;вони:noun:unanim:p:v_zna:pron:pers:3",
        "{got:?}"
    );

    // removeVmis: only the `f:v_mis` reading is dropped
    let got = disambiguated(&uk, "Петро Іванович");
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(
        got[1].1,
        "Іванович:noun:anim:f:v_dav:nv:prop:lname;Іванович:noun:anim:f:v_naz:nv:prop:lname;Іванович:noun:anim:f:v_oru:nv:prop:lname;Іванович:noun:anim:f:v_rod:nv:prop:lname;Іванович:noun:anim:f:v_zna:nv:prop:lname;Іванович:noun:anim:m:v_naz:prop:lname;Іванович:noun:anim:m:v_naz:prop:pname"
    );

    let got = disambiguated(&uk, "5 кг");
    assert_eq!(got.len(), 2);
    assert_eq!(got[1].0, "кг");
    assert_eq!(got[1].1.matches("nv:abbr").count(), 12);
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`) text/sentence rules:
/// `UkrainianUppercaseSentenceStartRule` (`а) б) в)` exception),
/// `UkrainianCommaWhitespaceRule`, `UkrainianWordRepeatRule` and
/// `HiddenCharacterRule`.
#[test]
fn ukrainian_text_rules() {
    let _guard = engine_guard();
    // uppercase: only the first lowercase sentence start is flagged, the
    // `а)` list item is an exception
    let text = "це речення. а) наступне.";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 2));
    assert_eq!(
        matches[0].message,
        "Це речення не починається з великої літери."
    );
    assert_eq!(matches[0].category_name, "Великі літери");

    // comma whitespace (uk strings)
    let text = "Слово , слово.";
    let matches = one(text, "COMMA_PARENTHESIS_WHITESPACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (5, 7));
    assert_eq!(
        matches[0].message,
        "Поставте пробіл після коми, а не перед комою."
    );
    assert_eq!(matches[0].category_name, "Можлива механічна помилка");

    // word repeat + exceptions
    let text = "Він буде буде тут.";
    let matches = one(text, "UKRAINIAN_WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (4, 13));
    assert!(one("Тому що що?", "UKRAINIAN_WORD_REPEAT_RULE").is_empty());
    assert!(one("від добра добра не шукають.", "UKRAINIAN_WORD_REPEAT_RULE").is_empty());
    let text = "І і знову.";
    let matches = one(text, "UKRAINIAN_WORD_REPEAT_RULE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 3));
    assert_eq!(
        matches[0].message,
        "Можлива механічна помилка: повторення слова або, можливо, перша І має бути латинською."
    );
    assert_eq!(
        suggestions(&matches[0]),
        vec!["І".to_string(), "I і".to_string()]
    );

    // hidden soft hyphen
    let text = "текст\u{00AD}текст";
    let matches = one(text, "UK_HIDDEN_CHARS");
    assert_eq!(matches.len(), 1);
    assert_eq!(suggestions(&matches[0]), vec!["тексттекст".to_string()]);
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`): `MixedAlphabetsRule`.
#[test]
fn ukrainian_mixed_alphabets() {
    let _guard = engine_guard();
    assert!(one("Це test слово.", "UK_MIXED_ALPHABETS").is_empty());

    let text = "Слово cлово.";
    let matches = one(text, "UK_MIXED_ALPHABETS");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (6, 11));
    assert_eq!(
        matches[0].message,
        "Вжито кириличні й латинські літери в одному слові"
    );
    assert_eq!(suggestions(&matches[0]), vec!["слово".to_string()]);

    let text = "XІІ ст.";
    let matches = one(text, "UK_MIXED_ALPHABETS");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 3));
    assert_eq!(
        matches[0].message,
        "Вжито кириличні літери замість латинських"
    );
    assert_eq!(suggestions(&matches[0]), vec!["XII".to_string()]);

    assert!(one("І. Франко", "UK_MIXED_ALPHABETS").is_empty());
    assert!(one("5-й", "UK_MIXED_ALPHABETS").is_empty());
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`):
/// `SimpleReplaceRenamedRule` (`UK_SIMPLE_REPLACE_RENAMED`).
#[test]
fn ukrainian_simple_replace_renamed() {
    let _guard = engine_guard();
    let text = "Альошинське село";
    let matches = one(text, "UK_SIMPLE_REPLACE_RENAMED");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 11));
    assert_eq!(matches[0].message, "«Альошинське» було перейменовано");
    assert_eq!(suggestions(&matches[0]), vec!["Загороднє".to_string()]);
    // "Аврора" has a `fname` reading first, which aborts the renamed check
    assert!(one("Аврора місто", "UK_SIMPLE_REPLACE_RENAMED").is_empty());
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`): `TypographyRule` (`DASH`)
/// and `MissingHyphenRule` (`UK_MISSING_HYPHEN`).
#[test]
fn ukrainian_typography_and_missing_hyphen() {
    let _guard = engine_guard();
    let text = "Київ—Львів";
    let matches = one(text, "DASH");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 5));
    assert_eq!(
        matches[0].message,
        "Риска всередині слова. Всередині слова вживайте дефіс, між словами виокремлюйте риску пробілами."
    );
    assert_eq!(
        suggestions(&matches[0]),
        vec!["Київ-Львів".to_string(), "Київ — Львів".to_string()]
    );
    assert!(one("спорт-клуб", "DASH").is_empty());
    assert!(one("Київ — Львів", "DASH").is_empty());

    // the short-dash range spans the original token (the en dash is 3 bytes
    // but `surface()` holds the normalized `-`)
    let text = "при вступі до ВНЗ здавали два–три екзамени.";
    let matches = one(text, "DASH");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (26, 33));

    let text = "медіа центр";
    let matches = one(text, "UK_MISSING_HYPHEN");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 11));
    assert_eq!(matches[0].message, "Можливо, зайвий пробіл?");
    assert_eq!(suggestions(&matches[0]), vec!["медіацентр".to_string()]);

    let text = "прем'єр міністр";
    let matches = one(text, "UK_MISSING_HYPHEN");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 15));
    assert_eq!(matches[0].message, "Можливо, пропущено дефіс?");
    assert_eq!(
        suggestions(&matches[0]),
        vec!["прем'єр-міністр".to_string()]
    );
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`): `SimpleReplaceRule`
/// (`UK_SIMPLE_REPLACE`, barbarisms via the lemma) and
/// `SimpleReplaceSoftRule` (`UK_SIMPLE_REPLACE_SOFT`, incl. `ctx:` contexts).
#[test]
fn ukrainian_simple_replace() {
    let _guard = engine_guard();
    let text = "вживати міроприємства";
    let matches = one(text, "UK_SIMPLE_REPLACE");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (8, 21));
    assert_eq!(
        matches[0].message,
        "«міроприємства» - помилкове слово, виправлення: захід."
    );
    assert_eq!(suggestions(&matches[0]), vec!["захід".to_string()]);

    // `:bad` fallback: Java uses the edit-distance-1
    // `getSuggestionsFromDefaultDicts`, not the tiered
    // `getSpellingSuggestions` (speller2/3).
    let text = "дорожному";
    let matches = one(text, "UK_SIMPLE_REPLACE");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].message, "Неправильно написане слово.");
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "Дорожному".to_string(),
            "дорожньому".to_string(),
            "дорожчому".to_string()
        ]
    );

    let text = "азіат";
    let matches = one(text, "UK_SIMPLE_REPLACE_SOFT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 5));
    assert_eq!(
        matches[0].message,
        "«азіат» — нерекомендоване слово, кращий варіант: азієць."
    );
    assert_eq!(suggestions(&matches[0]), vec!["азієць".to_string()]);

    let text = "многогрішний";
    let matches = one(text, "UK_SIMPLE_REPLACE_SOFT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 12));
    assert_eq!(
        matches[0].message,
        "«многогрішний» вживається лише в таких контекстах: релігія, поезія, можливо, мали на увазі: великогрішний, великогрішник?"
    );
    assert_eq!(
        suggestions(&matches[0]),
        vec!["великогрішний".to_string(), "великогрішник".to_string()]
    );
}

/// Java-probed: `JLanguageTool.replaceSoftHyphens` strips the language's
/// `getIgnoredCharactersRegex` (uk: soft hyphen + combining acute) from tokens
/// before tagging, so a stressed word matches the unaccented dictionary entry.
#[test]
fn ukrainian_ignored_characters_are_stripped_for_tagging() {
    let _guard = engine_guard();

    let text = "ґра́нтовий";
    let matches = one(text, "ALT_SPELLING");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 10));
    assert!(one(text, "MORFOLOGIK_RULE_UK_UA").is_empty(), "{text}");

    let text = "Снігопади паралізували пів Євро́пи.";
    assert!(one(text, "MORFOLOGIK_RULE_UK_UA").is_empty(), "{text}");
}

/// The tokenizer normalizes typographic apostrophes/quotes to ASCII (1 byte
/// instead of 3), so match ranges must still use the original byte span.
#[test]
fn ukrainian_normalized_character_offsets() {
    let _guard = engine_guard();

    let text = "зв’язок з науковцями втрачено";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 7));

    let text = "До Кот д’Івуара";
    let matches = one(text, "MORFOLOGIK_RULE_UK_UA");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (7, 15));

    let text = "призначає нового прем’єра. щоправда, у Держдуми";
    let matches = one(text, "comma_insert_words");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (25, 35));
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`): XML multi-form `<match>`
/// synthesis (`PatternRuleMatcher.formatMatches`) must expand the full
/// cartesian product, re-scanning duplicated `<suggestion>` copies in place.
#[test]
fn ukrainian_multiple_synthesis() {
    let _guard = engine_guard();

    let text = "на повістці дня.";
    let matches = one(text, "povistka_dnya");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "порядкові денному".to_string(),
            "порядкові деннім".to_string(),
            "порядку денному".to_string(),
            "порядку деннім".to_string()
        ]
    );

    let text = "він рахуватися народним депутатом";
    let matches = one(text, "RAHUVATYSIA_SCHO");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "вважатись за народне депутата".to_string(),
            "вважатись за народний депутата".to_string(),
            "вважатись за народного депутата".to_string(),
            "вважатися за народне депутата".to_string(),
            "вважатися за народний депутата".to_string(),
            "вважатися за народного депутата".to_string()
        ]
    );
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`):
/// `TokenAgreementNumrNounRule` + its exception helper.
#[test]
fn ukrainian_numr_noun_agreement() {
    let _guard = engine_guard();
    let text = "два стола";
    let matches = one(text, "UK_NUMR_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 9));
    assert_eq!(
        matches[0].message,
        "Потенційна помилка: числівник не узгоджений з іменником: \"два\" вимагає: [мн.: називний], а далі йде \"стола\": [ч.р.: родовий, знахідний]"
    );
    assert_eq!(suggestions(&matches[0]), vec!["два столи".to_string()]);

    let text = "півтора роки";
    let matches = one(text, "UK_NUMR_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 12));
    assert_eq!(
        matches[0].message,
        "Існує правило, що після «півтора» треба вживати родовий відмінок ч. або с.р., однак у текстах в багатьох випадках вживають і форму множини, надто коли перед іменником іде прикметник"
    );
    assert_eq!(suggestions(&matches[0]), vec!["півтора року".to_string()]);

    for ok in [
        "два столи",
        "пять столів",
        "дві книги",
        "три столи",
        "1,5 метра",
        "багато людей",
        "два з половиною роки",
        // month lemma + `:m:v_rod` suppresses (`LemmaHelper` partPos match)
        "23 Липня",
        "23 Листопада.",
    ] {
        assert!(
            one(ok, "UK_NUMR_NOUN_INFLECTION_AGREEMENT").is_empty(),
            "{ok}"
        );
    }
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`):
/// `TokenAgreementAdjNounRule` + its exception helper.
#[test]
fn ukrainian_adj_noun_agreement() {
    let _guard = engine_guard();
    let text = "новий книга";
    let matches = one(text, "UK_ADJ_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 11));
    assert_eq!(
        matches[0].message,
        "Потенційна помилка: прикметник не узгоджений з іменником: \"новий\": [ч.р.: називний, знахідний (неіст.), кличний] і \"книга\": [ж.р.: називний]"
    );
    assert_eq!(suggestions(&matches[0]), vec!["нова книга".to_string()]);

    let text = "велика місто";
    let matches = one(text, "UK_ADJ_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 12));
    assert_eq!(suggestions(&matches[0]), vec!["велике місто".to_string()]);

    // Java sorts `master`/`slave` in place in `formatInflections` before the
    // suggestion loops, so the suggestions follow the gen/vidm order.
    let text = "в командну строку.";
    let matches = one(text, "UK_ADJ_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "командного строку".to_string(),
            "командному строку".to_string(),
            "команднім строку".to_string()
        ]
    );

    let text = "на добровільний дачі свідчень.";
    let matches = one(text, "UK_ADJ_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        suggestions(&matches[0]),
        vec![
            "добровільної дачі".to_string(),
            "добровільній дачі".to_string()
        ]
    );

    for ok in [
        "нова книга",
        "синя стіна",
        "зелений стіл",
        "добра людина",
        "старого будинку",
        "цікава книжка",
        // `reverseConjFind`/`reverseConjAdvFind` (plural noun after `та <adj>`)
        "середня та старша сестри",
        "права та ліва частини",
    ] {
        assert!(
            one(ok, "UK_ADJ_NOUN_INFLECTION_AGREEMENT").is_empty(),
            "{ok}"
        );
    }
}

/// Java-probed (`scripts/oracle/uk/probe-disambig.sh`): the XML
/// disambiguation forward-scan cascade. `non_v_kly_2` removes the vocative
/// reading from `старший` and then, because the scan continues on the mutated
/// readings, from the following `сестри`; `lt-disambig` re-scans single-pattern
/// `remove` rules forward like Java's `doMatch`.
#[test]
fn ukrainian_disambiguation_vocative_cascade() {
    let _guard = engine_guard();
    for text in [
        "він старший сестри на 3 роки",
        "вона старша сестри на 3 роки",
    ] {
        let matches = one(text, "UK_ADJ_NOUN_INFLECTION_AGREEMENT");
        assert_eq!(matches.len(), 1, "{text}");
        assert!(
            !matches[0].message.contains("кличний"),
            "spurious vocative reading in {text}: {}",
            matches[0].message
        );
    }
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`):
/// `TokenAgreementVerbNounRule` + its exception helper.
#[test]
fn ukrainian_verb_noun_agreement() {
    let _guard = engine_guard();
    let text = "Працює студенти";
    let matches = one(text, "UK_VERB_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 15));
    assert_eq!(
        matches[0].message,
        "Не узгоджено дієслово з іменником: \"Працює\" (вимагає: орудний) і \"студенти\" (мн.: називний, кличний)"
    );
    assert_eq!(
        suggestions(&matches[0]),
        vec!["Працює студентами".to_string()]
    );

    let text = "Було студенти";
    let matches = one(text, "UK_VERB_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 13));
    assert_eq!(
        matches[0].message,
        "Не узгоджено дієслово з іменником: \"Було\" (вимагає: інфінітив, орудний, давальний) і \"студенти\" (мн.: називний, кличний)"
    );
    assert_eq!(
        suggestions(&matches[0]),
        vec!["Було студентам".to_string(), "Було студентами".to_string()]
    );

    for ok in [
        "Прийшли студент",
        "Прийшла студент",
        "Прийшли студенти",
        "Прийшла студентка",
        "Працюють студенти",
        "Пішли студент",
    ] {
        assert!(
            one(ok, "UK_VERB_NOUN_INFLECTION_AGREEMENT").is_empty(),
            "{ok}"
        );
    }
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`):
/// `TokenAgreementNounVerbRule` + its exception helper.
#[test]
fn ukrainian_noun_verb_agreement() {
    let _guard = engine_guard();
    for (text, rng) in [
        ("Студент прийшли", (0, 15)),
        ("Студенти прийшов", (0, 16)),
        ("Діти прийшла", (0, 12)),
        ("Він прийшли", (0, 11)),
        ("Людина прийшли", (0, 14)),
    ] {
        let matches = one(text, "UK_NOUN_VERB_INFLECTION_AGREEMENT");
        assert_eq!(matches.len(), 1, "{text}");
        assert_utf16(text, &matches[0], rng);
        assert!(
            matches[0]
                .message
                .starts_with("Не узгоджено іменник з дієсловом:"),
            "{text}: {}",
            matches[0].message
        );
        assert!(matches[0].suggestions.is_empty());
    }
    assert_eq!(
        one("Студент прийшли", "UK_NOUN_VERB_INFLECTION_AGREEMENT")[0].message,
        "Не узгоджено іменник з дієсловом: \"Студент\" (ч.р.) і \"прийшли\" (мн.)"
    );
    assert!(one("Вона прийшла", "UK_NOUN_VERB_INFLECTION_AGREEMENT").is_empty());
    assert!(one("Місто розташувалися", "UK_NOUN_VERB_INFLECTION_AGREEMENT").is_empty());
}

/// Java-probed (`scripts/oracle/uk/probe-rule.sh`):
/// `TokenAgreementPrepNounRule` + its exception helper.
#[test]
fn ukrainian_prep_noun_agreement() {
    let _guard = engine_guard();
    let text = "згідно з документа";
    let matches = one(text, "UK_PREP_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (9, 18));
    assert_eq!(
        matches[0].message,
        "Прийменник «з» вимагає іншого відмінка: орудний, а знайдено: родовий, знахідний"
    );
    assert_eq!(suggestions(&matches[0]), vec!["документом".to_string()]);

    let text = "завдяки його зусиллі";
    let matches = one(text, "UK_PREP_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (13, 20));
    assert_eq!(suggestions(&matches[0]), vec!["зусиллю".to_string()]);

    let text = "до Київ";
    let matches = one(text, "UK_PREP_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (3, 7));
    assert_eq!(suggestions(&matches[0]), vec!["Києва".to_string()]);

    for ok in [
        "згідно з документом",
        "завдяки його зусиллям",
        "на вулиці",
        "до Києва",
        "при його ділянці",
    ] {
        assert!(
            one(ok, "UK_PREP_NOUN_INFLECTION_AGREEMENT").is_empty(),
            "{ok}"
        );
    }
}

/// `TokenAgreementPrepNounRule` keeps its prep state across skip-type
/// exception steps (as in the Java reference), so an intervening `part`
/// token such as the `не` in `незважаючи не це` does not abort the check.
#[test]
fn ukrainian_prep_noun_skips_intervening_particle() {
    let _guard = engine_guard();

    let text = "незважаючи не це";
    let matches = one(text, "UK_PREP_NOUN_INFLECTION_AGREEMENT");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (14, 16));
    assert_eq!(
        matches[0].message,
        "Прийменник «незважаючи» вимагає іншого відмінка: , а знайдено: називний, знахідний"
    );
    assert_eq!(suggestions(&matches[0]), vec!["це".to_string()]);
}

/// `UkrainianUppercaseSentenceStartRule` flags a lowercase sentence start
/// after an abbreviation dot: its list exception requires `)` after the
/// lowercase letter (like Java's override), while `.` only feeds the
/// `NUMERALS_EN` numeric-enumeration check, which does not match Cyrillic.
/// A letter followed by `)` is a genuine list marker and stays exempt.
#[test]
fn ukrainian_uppercase_start_after_abbreviation_dot() {
    let _guard = engine_guard();

    let text = "т. 2 ч. 1";
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 1));
    assert_eq!(suggestions(&matches[0]), vec!["Т".to_string()]);
    assert!(one("а) наступне.", "UPPERCASE_SENTENCE_START").is_empty());
}

/// The adj-noun agreement rule exempts a plural adjective followed by a
/// coordinated proper-name list (the `forwardConjFind` branch of Java's
/// exception helper), so only the lowercase sentence start is reported.
#[test]
fn ukrainian_adj_noun_coordinated_names_exception() {
    let _guard = engine_guard();

    let text = "молодші Олександр Ірванець, Оксана Луцишина";
    assert!(one(text, "UK_ADJ_NOUN_INFLECTION_AGREEMENT").is_empty());
    let matches = one(text, "UPPERCASE_SENTENCE_START");
    assert_eq!(matches.len(), 1);
    assert_utf16(text, &matches[0], (0, 7));
    assert_eq!(suggestions(&matches[0]), vec!["Молодші".to_string()]);
}
