//! Port of `org.languagetool.tools.ArabicNumbersWords`,
//! `ArabicNumbersWordsConstants` and `ArabicUnitsHelper`: the number-to-words
//! engine behind `ArabicNumberPhraseFilter` (rule `syntax_numeric_0003`).
//!
//! `numberToArabicWords` works on `BigInteger`/`BigDecimal` upstream; the
//! inputs here are `textToNumber` results (`Integer`, i32 with Java's
//! wrapping arithmetic), so i64 covers the group arithmetic exactly.
//! `ArabicStringTools.removeTashkeel` is the shared `lt_tagger::arabic`
//! helper.

use std::collections::HashMap;

use lt_tagger::arabic::remove_tashkeel;

/* `ArabicNumbersWordsConstants` */

/// `arabicOnes`.
const ARABIC_ONES: [&str; 20] = [
    "",
    "واحد",
    "اثنان",
    "ثلاثة",
    "أربعة",
    "خمسة",
    "ستة",
    "سبعة",
    "ثمانية",
    "تسعة",
    "عشرة",
    "أحد عشر",
    "اثنا عشر",
    "ثلاثة عشر",
    "أربعة عشر",
    "خمسة عشر",
    "ستة عشر",
    "سبعة عشر",
    "ثمانية عشر",
    "تسعة عشر",
];

/// `arabicFeminineOnes`.
const ARABIC_FEMININE_ONES: [&str; 20] = [
    "",
    "إحدى",
    "اثنتان",
    "ثلاث",
    "أربع",
    "خمس",
    "ست",
    "سبع",
    "ثمان",
    "تسع",
    "عشر",
    "إحدى عشرة",
    "اثنتا عشرة",
    "ثلاث عشرة",
    "أربع عشرة",
    "خمس عشرة",
    "ست عشرة",
    "سبع عشرة",
    "ثماني عشرة",
    "تسع عشرة",
];

/// `arabicTens`.
const ARABIC_TENS: [&str; 8] = [
    "عشرون",
    "ثلاثون",
    "أربعون",
    "خمسون",
    "ستون",
    "سبعون",
    "ثمانون",
    "تسعون",
];

/// `arabicHundreds`.
const ARABIC_HUNDREDS: [&str; 10] = [
    "",
    "مائة",
    "مئتان",
    "ثلاثمئة",
    "أربعمئة",
    "خمسمئة",
    "ستمئة",
    "سبعمئة",
    "ثمانمئة",
    "تسعمئة",
];

/// `arabicTwos`.
const ARABIC_TWOS: [&str; 8] = [
    "مئتان",
    "ألفان",
    "مليونان",
    "ملياران",
    "تريليونان",
    "كوادريليونان",
    "كوينتليونان",
    "سكستيليونان",
];

/// `arabicAppendedTwos`.
const ARABIC_APPENDED_TWOS: [&str; 8] = [
    "مئتا",
    "ألفا",
    "مليونا",
    "مليارا",
    "تريليونا",
    "كوادريليونا",
    "كوينتليونا",
    "سكستيليونا",
];

/// `arabicGroup`.
const ARABIC_GROUP: [&str; 8] = [
    "مائة",
    "ألف",
    "مليون",
    "مليار",
    "تريليون",
    "كوادريليون",
    "كوينتليون",
    "سكستيليون",
];

/// `arabicAppendedGroup`.
const ARABIC_APPENDED_GROUP: [&str; 8] = [
    "",
    "ألفاً",
    "مليوناً",
    "ملياراً",
    "تريليوناً",
    "كوادريليوناً",
    "كوينتليوناً",
    "سكستيليوناً",
];

/// `arabicPluralGroups`.
const ARABIC_PLURAL_GROUPS: [&str; 8] = [
    "",
    "آلاف",
    "ملايين",
    "مليارات",
    "تريليونات",
    "كوادريليونات",
    "كوينتليونات",
    "سكستيليونات",
];

/// `arabicJarTens`.
const ARABIC_JAR_TENS: [&str; 8] = [
    "عشرين",
    "ثلاثين",
    "أربعين",
    "خمسين",
    "ستين",
    "سبعين",
    "ثمانين",
    "تسعين",
];

/// `arabicJarTwos`.
const ARABIC_JAR_TWOS: [&str; 8] = [
    "مئتين",
    "ألفين",
    "مليونين",
    "مليارين",
    "تريليونين",
    "كوادريليونين",
    "كوينتليونين",
    "سكستيليونين",
];

/// `arabicJarAppendedTwos`.
const ARABIC_JAR_APPENDED_TWOS: [&str; 8] = [
    "مئتي",
    "ألفي",
    "مليوني",
    "ملياري",
    "تريليوني",
    "كوادريليوني",
    "كوينتليوني",
    "سكستيليوني",
];

/// `arabicJarOnes`.
const ARABIC_JAR_ONES: [&str; 20] = [
    "",
    "واحد",
    "اثنين",
    "ثلاثة",
    "أربعة",
    "خمسة",
    "ستة",
    "سبعة",
    "ثمانية",
    "تسعة",
    "عشرة",
    "أحد عشر",
    "اثني عشر",
    "ثلاثة عشر",
    "أربعة عشر",
    "خمسة عشر",
    "ستة عشر",
    "سبعة عشر",
    "ثمانية عشر",
    "تسعة عشر",
];

/// `arabicJarFeminineOnes`.
const ARABIC_JAR_FEMININE_ONES: [&str; 20] = [
    "",
    "إحدى",
    "اثنتين",
    "ثلاث",
    "أربع",
    "خمس",
    "ست",
    "سبع",
    "ثمان",
    "تسع",
    "عشر",
    "إحدى عشرة",
    "اثنتي عشرة",
    "ثلاث عشرة",
    "أربع عشرة",
    "خمس عشرة",
    "ست عشرة",
    "سبع عشرة",
    "ثماني عشرة",
    "تسع عشرة",
];

/// `arabicJarHundreds`.
const ARABIC_JAR_HUNDREDS: [&str; 10] = [
    "",
    "مائة",
    "مئتين",
    "ثلاثمائة",
    "أربعمائة",
    "خمسمائة",
    "ستمائة",
    "سبعمائة",
    "ثمانمائة",
    "تسعمائة",
];

/// `NUMBER_WORDS` (the numeric word values; the `تسعمئة` duplicate of the
/// `مئة`-series hundreds is overwritten upstream, so the map keeps 900).
fn number_words() -> &'static HashMap<&'static str, i32> {
    static NUMBER_WORDS: std::sync::LazyLock<HashMap<&'static str, i32>> =
        std::sync::LazyLock::new(|| {
            let mut m = HashMap::new();
            m.insert("صفر", 0);
            m.insert("واحد", 1);
            m.insert("واحدة", 1);
            m.insert("اثنان", 2);
            m.insert("ثلاثة", 3);
            m.insert("أربعة", 4);
            m.insert("خمسة", 5);
            m.insert("ستة", 6);
            m.insert("سبعة", 7);
            m.insert("ثمانية", 8);
            m.insert("تسعة", 9);
            m.insert("عشرة", 10);
            m.insert("عشرون", 20);
            m.insert("ثلاثون", 30);
            m.insert("أربعون", 40);
            m.insert("خمسون", 50);
            m.insert("ستون", 60);
            m.insert("سبعون", 70);
            m.insert("ثمانون", 80);
            m.insert("تسعون", 90);
            m.insert("مئة", 100);
            m.insert("مئتان", 200);
            m.insert("ثلاثمئة", 300);
            m.insert("أربعمئة", 400);
            m.insert("خمسمئة", 500);
            m.insert("ستمئة", 600);
            m.insert("سبعمئة", 700);
            m.insert("ثمانمئة", 800);
            m.insert("تسعمئة", 900);
            m.insert("ثلاثمائة", 300);
            m.insert("أربعمائة", 400);
            m.insert("خمسمائة", 500);
            m.insert("ستمائة", 600);
            m.insert("سبعمائة", 700);
            m.insert("ثمانمائة", 800);
            m.insert("تسعمائة", 900);
            m.insert("ألف", 1000);
            m.insert("ألفا", 1000);
            m.insert("مليون", 1000000);
            m.insert("مليونا", 1000000);
            m.insert("مليار", 1000000000);
            m.insert("ألفان", 2000);
            m.insert("ألفين", 2000);
            m.insert("مليونان", 2000000);
            m.insert("مليونين", 2000000);
            m.insert("ملياران", 2000000000);
            m.insert("مليارين", 2000000000);
            m.insert("أحد", 1);
            m.insert("إحدى", 1);
            m.insert("اثنين", 2);
            m.insert("إثنين", 2);
            m.insert("إثنان", 2);
            m.insert("اثني", 2);
            m.insert("اثنتي", 2);
            m.insert("اثنا", 2);
            m.insert("إثني", 2);
            m.insert("إثنتي", 2);
            m.insert("إثنا", 2);
            m.insert("ثلاث", 3);
            m.insert("أربع", 4);
            m.insert("خمس", 5);
            m.insert("ست", 6);
            m.insert("سبع", 7);
            m.insert("ثمان", 8);
            m.insert("ثماني", 8);
            m.insert("تسع", 9);
            m.insert("عشر", 10);
            m.insert("ثلاثا", 3);
            m.insert("أربعا", 4);
            m.insert("خمسا", 5);
            m.insert("ستا", 6);
            m.insert("سبعا", 7);
            m.insert("تسعا", 9);
            m.insert("عشرا", 10);
            m.insert("عشرين", 20);
            m.insert("ثلاثين", 30);
            m.insert("أربعين", 40);
            m.insert("خمسين", 50);
            m.insert("ستين", 60);
            m.insert("سبعين", 70);
            m.insert("ثمانين", 80);
            m.insert("تسعين", 90);
            m.insert("مائة", 100);
            m.insert("مئتين", 200);
            m.insert("آلاف", 1000);
            m.insert("ملايين", 1000000);
            m.insert("مليارات", 1000000000);
            m
        });
    &NUMBER_WORDS
}

/// `ArabicNumbersWords.isNumericWord`.
fn is_numeric_word(word: &str) -> bool {
    number_words().contains_key(word)
}

/// `ArabicNumbersWords.getNumericWordValue`.
fn numeric_word_value(word: &str) -> i32 {
    number_words()[word]
}

/* `ArabicNumbersWords` */

/// `ArabicNumbersWords.textToNumber`.
pub fn text_to_number(text: &str) -> i32 {
    let text = remove_tashkeel(text);
    let words: Vec<&str> = text.split(' ').collect();
    text_to_number_words(&words)
}

/// `ArabicNumbersWords.textToNumber(List<String>)`.
fn text_to_number_words(words: &[&str]) -> i32 {
    let mut total: i32 = 0;
    let mut partial: i32 = 0;
    for word in words {
        let mut word = *word;
        if !word.is_empty()
            && word != "واحد"
            && ["و", "ف", "ب", "ك", "ل"]
                .iter()
                .any(|p| word.starts_with(p))
        {
            // strip first char
            word = &word[word.chars().next().unwrap().len_utf8()..];
        }
        if word != "واحد" && word.starts_with('و') {
            // strip first char
            word = &word[word.chars().next().unwrap().len_utf8()..];
        }
        if is_numeric_word(word) {
            let actual = numeric_word_value(word);
            if actual % 1000 == 0 {
                // the case of 1000 or 1 million
                if partial == 0 {
                    partial = 1;
                }
                total = total.wrapping_add(partial.wrapping_mul(actual));
                // re-initiate the partial total
                partial = 0;
            } else {
                partial = partial.wrapping_add(actual);
            }
        }
    }
    // add the final partial to total
    total.wrapping_add(partial)
}

/// `ArabicNumbersWords.numberToArabicWords(BigInteger, boolean)` (the
/// non-inflected default: not attached, no inflection case).
#[allow(dead_code)]
pub fn number_to_arabic_words(number: i64, is_feminine: bool) -> String {
    convert_to_arabic(number, is_feminine, false, "")
        .trim()
        .to_string()
}

/// `ArabicNumbersWords.numberToArabicWords(BigInteger, boolean, boolean,
/// String)`.
pub fn number_to_arabic_words_inflected(
    number: i64,
    is_feminine: bool,
    is_attached: bool,
    inflection_case: &str,
) -> String {
    convert_to_arabic(number, is_feminine, is_attached, inflection_case)
        .trim()
        .to_string()
}

/// `ArabicNumbersWords.convertToArabic`.
fn convert_to_arabic(
    mut number: i64,
    is_feminine: bool,
    is_attached: bool,
    inflection_case: &str,
) -> String {
    if number == 0 {
        return "صفر".to_string();
    } else if number == 1 {
        return "واحد".to_string();
    } else if number == 2 {
        return get_digit_inflected_status(2, 0, false, false, inflection_case).to_string();
    }
    let mut result = String::new();
    let mut group: i32 = 0;
    while number >= 1 {
        // separate number into groups
        let number_to_process = number % 1000;
        number /= 1000;
        // convert group into its text (`tempNumber.setScale(0, FLOOR)`)
        let temp_value = number as i32;
        let group_description = process_arabic_group(
            number_to_process as i32,
            group,
            temp_value,
            is_feminine,
            is_attached,
            inflection_case,
        );
        if !group_description.is_empty() {
            // here we add the new converted group to the previous concatenated text
            if group > 0 {
                if !result.is_empty() {
                    result.insert(0, 'و');
                }
                if number_to_process != 2 && number_to_process % 100 != 1 {
                    if (3..=10).contains(&number_to_process) {
                        // for numbers between 3 and 9 we use plural name
                        result.insert_str(0, &format!("{} ", ARABIC_PLURAL_GROUPS[group as usize]));
                    } else if !result.is_empty() {
                        // use appending case
                        result
                            .insert_str(0, &format!("{} ", ARABIC_APPENDED_GROUP[group as usize]));
                    } else {
                        // use normal case
                        result.insert_str(0, &format!("{} ", ARABIC_GROUP[group as usize]));
                    }
                }
            }
            result.insert_str(0, &format!("{} ", group_description));
        }
        group += 1;
    }
    result
}

/// `ArabicNumbersWords.processArabicGroup`.
fn process_arabic_group(
    group_number: i32,
    group_level: i32,
    remaining_number: i32,
    is_feminine: bool,
    is_attached: bool,
    inflection_case: &str,
) -> String {
    let tens = group_number % 100;
    let hundreds = group_number / 100;
    let mut result = String::new();
    if hundreds > 0 {
        if tens == 0 && hundreds == 2 {
            // حالة المضاف
            if group_level == 0 {
                result = get_digit_hundred_jar_status(hundreds, inflection_case).to_string();
            } else {
                result = get_digit_twos_jar_status(0, inflection_case, true).to_string();
            }
        } else {
            // الحالة العادية
            result = get_digit_hundred_jar_status(hundreds, inflection_case).to_string();
        }
    }
    if tens > 0 {
        if tens < 20 {
            // if we are processing under 20 numbers
            if tens == 2 && hundreds == 0 && group_level > 0 {
                // This is special case for number 2 when it comes alone in the group
                // في حالة الافراد
                result = get_digit_twos_jar_status(group_level, inflection_case, false).to_string();
            } else {
                // General case
                if !result.is_empty() {
                    result += " و";
                }
                if tens == 1 && group_level > 0 {
                    result += ARABIC_GROUP[group_level as usize];
                } else if (tens == 1 || tens == 2)
                    && (group_level == 0 || group_level == -1)
                    && hundreds == 0
                    && remaining_number == 0
                {
                    // Special case for 1 and 2 numbers like: ليرة سورية و ليرتان سوريتان
                } else {
                    // Get Feminine status for this digit
                    result += get_digit_inflected_status(
                        tens,
                        group_level,
                        is_feminine,
                        is_attached,
                        inflection_case,
                    );
                }
            }
        } else {
            let ones = tens % 10;
            let tens = (tens / 10) - 2; // 20's offset
            if ones > 0 {
                if !result.is_empty() {
                    result += " و";
                }
                // Get Feminine status for this digit
                result += get_digit_inflected_status(
                    ones,
                    group_level,
                    is_feminine,
                    is_attached,
                    inflection_case,
                );
            }
            if !result.is_empty() {
                result += " و";
            }
            // Get Tens text (get ten text for inflected case jar or nasb)
            result += get_digit_tens_jar_status(tens, inflection_case);
        }
    }
    result
}

/// `ArabicNumbersWords.getDigitInflectedStatus`.
fn get_digit_inflected_status(
    digit: i32,
    group_level: i32,
    is_feminine: bool,
    _is_attached: bool,
    inflection_case: &str,
) -> &'static str {
    if inflection_case == "jar" {
        if group_level == -1 || group_level == 0 {
            if !is_feminine {
                return ARABIC_JAR_ONES[digit as usize];
            }
            return ARABIC_JAR_FEMININE_ONES[digit as usize];
        }
    } else if group_level == -1 || group_level == 0 {
        if !is_feminine {
            return ARABIC_ONES[digit as usize];
        }
        return ARABIC_FEMININE_ONES[digit as usize];
    }
    ARABIC_ONES[digit as usize]
}

/// `ArabicNumbersWords.getDigitTensJarStatus` (the Java source compares
/// `inflectionCase` twice against "jar").
fn get_digit_tens_jar_status(digit: i32, inflection_case: &str) -> &'static str {
    if inflection_case == "jar" {
        return ARABIC_JAR_TENS[digit as usize];
    }
    ARABIC_TENS[digit as usize]
}

/// `ArabicNumbersWords.getDigitHundredJarStatus`.
fn get_digit_hundred_jar_status(digit: i32, inflection_case: &str) -> &'static str {
    if inflection_case == "jar" {
        return ARABIC_JAR_HUNDREDS[digit as usize];
    }
    ARABIC_HUNDREDS[digit as usize]
}

/// `ArabicNumbersWords.getDigitTwosJarStatus`.
fn get_digit_twos_jar_status(digit: i32, inflection_case: &str, is_appended: bool) -> &'static str {
    if !is_appended {
        if inflection_case == "jar" {
            return ARABIC_JAR_TWOS[digit as usize];
        }
        return ARABIC_TWOS[digit as usize];
    }
    if inflection_case == "jar" {
        return ARABIC_JAR_APPENDED_TWOS[digit as usize];
    }
    ARABIC_APPENDED_TWOS[digit as usize]
}

/// `ArabicNumbersWords.checkNumericPhrase`.
#[allow(dead_code)]
pub fn check_numeric_phrase(
    phrase_input: &str,
    feminin: bool,
    attached: bool,
    inflection: &str,
) -> bool {
    let phrase = remove_tashkeel(phrase_input);
    let x = text_to_number(&phrase);
    let auto_phrase = remove_tashkeel(&number_to_arabic_words_inflected(
        x as i64, feminin, attached, inflection,
    ));
    auto_phrase == phrase
}

/// `ArabicNumbersWords.getSuggestionsNumericPhrase`.
pub fn get_suggestions_numeric_phrase(
    phrase_input: &str,
    feminin: bool,
    attached: bool,
    inflection: &str,
) -> Vec<String> {
    let mut suggestions = Vec::new();
    let phrase = remove_tashkeel(phrase_input);
    let x = text_to_number(&phrase);
    let auto_phrase = remove_tashkeel(&number_to_arabic_words_inflected(
        x as i64, feminin, attached, inflection,
    ));
    if auto_phrase != phrase {
        // if inflection is null generate all cases
        if !inflection.is_empty() {
            suggestions.push(auto_phrase);
        } else {
            // Raf3
            suggestions.push(remove_tashkeel(&number_to_arabic_words_inflected(
                x as i64, feminin, attached, "raf3",
            )));
            // nasb+jar
            suggestions.push(remove_tashkeel(&number_to_arabic_words_inflected(
                x as i64, feminin, attached, "jar",
            )));
        }
    }
    suggestions
}

/* `ArabicUnitsHelper` */

/// One `unitsMap` entry: the `feminin` flag plus the nine forms
/// (`one|two|plural` × `raf3|nasb|jar`).
struct UnitEntry {
    feminin: bool,
    forms: [(&'static str, &'static str); 9],
}

impl UnitEntry {
    fn get_form(&self, category: &str, inflection: &str) -> Option<&'static str> {
        let key = format!("{category}_{inflection}");
        self.forms.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }
}

/// `ArabicUnitsHelper.unitsMap` (the four units, in Java order).
fn units_map(unit: &str) -> Option<UnitEntry> {
    let entry = match unit {
        "دينار" => UnitEntry {
            feminin: false,
            forms: [
                ("one_raf3", "دينار"),
                ("one_nasb", "دينارًا"),
                ("one_jar", "دينارٍ"),
                ("two_raf3", "ديناران"),
                ("two_nasb", "دينارين"),
                ("two_jar", "دينارين"),
                ("plural_raf3", "دنانيرُ"),
                ("plural_nasb", "دنانيرَ"),
                ("plural_jar", "دنانيرَ"),
            ],
        },
        "درهم" => UnitEntry {
            feminin: false,
            forms: [
                ("one_raf3", "درهم"),
                ("one_nasb", "درهمًا"),
                ("one_jar", "درهمٍ"),
                ("two_raf3", "درهمان"),
                ("two_nasb", "درهمين"),
                ("two_jar", "درهمين"),
                ("plural_raf3", "دراهمُ"),
                ("plural_nasb", "دراهمَ"),
                ("plural_jar", "دراهمَ"),
            ],
        },
        "دولار" => UnitEntry {
            feminin: false,
            forms: [
                ("one_raf3", "دولار"),
                ("one_nasb", "دولارًا"),
                ("one_jar", "دولارٍ"),
                ("two_raf3", "دولاران"),
                ("two_nasb", "دولارين"),
                ("two_jar", "دولارين"),
                ("plural_raf3", "دولاراتٌ"),
                ("plural_nasb", "دولاراتٍ"),
                ("plural_jar", "دولاراتٍ"),
            ],
        },
        "ليرة" => UnitEntry {
            feminin: true,
            forms: [
                ("one_raf3", "ليرة"),
                ("one_nasb", "ليرةً"),
                ("one_jar", "ليرةٍ"),
                ("two_raf3", "ليرتان"),
                ("two_nasb", "ليرتين"),
                ("two_jar", "ليرتين"),
                ("plural_raf3", "ليراتٌ"),
                ("plural_nasb", "ليراتٍ"),
                ("plural_jar", "ليراتٍ"),
            ],
        },
        _ => return None,
    };
    Some(entry)
}

/// `ArabicUnitsHelper.isFeminin`.
pub fn is_feminin(unit: &str) -> bool {
    units_map(unit).is_some_and(|e| e.feminin)
}

/// `ArabicUnitsHelper.getForm`.
pub fn get_form(unit: &str, category: &str, inflection: &str) -> String {
    let inflection = if inflection.is_empty() {
        "raf3"
    } else {
        inflection
    };
    if let Some(entry) = units_map(unit) {
        return entry
            .get_form(category, inflection)
            .map(str::to_string)
            .unwrap_or_else(|| format!("[{unit}]"));
    }
    format!("[[{unit}]]")
}

/// `ArabicNumbersWords.numberToWordsWithUnitsMap`: the phrase/unit agreement
/// map (the filter consumes `phrase`, `unitInflection` and `unitNumber`;
/// `all` and `unit` back the upstream `numberToWordsWithUnits`/`getUnitForm`
/// accessors).
#[allow(dead_code)]
pub struct NumberUnitsPhrase {
    pub all: String,
    pub phrase: String,
    pub unit: String,
    pub unit_number: String,
    pub unit_inflection: String,
}

/// `ArabicNumbersWords.numberToWordsWithUnitsMap`.
pub fn number_to_words_with_units_map(n: i32, unit: &str, inflection: &str) -> NumberUnitsPhrase {
    // get feminin from unit
    let feminin = is_feminin(unit);
    let mut unit_inflection = "";
    let mut unit_number = "";
    // generate phrase from number
    let number_phrase = number_to_arabic_words_inflected(n as i64, feminin, true, inflection);
    let mut phrase = String::new();
    let new_unit;
    if n == 0 {
        new_unit = get_form(unit, "plural", "nasb");
        unit_inflection = "nasb";
        unit_number = "plural";
        phrase.push_str("لا");
        phrase.push(' ');
        phrase.push_str(&new_unit);
    } else if n == 1 {
        // دينار واحد
        new_unit = get_form(unit, "one", inflection);
        unit_inflection = inflection;
        unit_number = "one";
        phrase.push_str(&new_unit);
        phrase.push(' ');
        phrase.push_str(&number_phrase);
    } else if n == 2 {
        // ديناران
        new_unit = get_form(unit, "two", inflection);
        unit_inflection = inflection;
        unit_number = "two";
        phrase.push_str(&new_unit);
    } else if n % 100 == 1 {
        // مئة دينار ودينار: regenerate the phrase number for n-1, then add the
        // unit for hundreds, then add one unit
        let number_phrase_hundred =
            number_to_arabic_words_inflected((n - 1) as i64, feminin, true, inflection);
        let new_unit_hundred = get_form(unit, "one", "jar");
        let new_unit_one = get_form(unit, "one", inflection);
        unit_inflection = inflection;
        unit_number = "one";
        new_unit = new_unit_one.clone();
        phrase.push_str(&number_phrase_hundred);
        phrase.push(' ');
        phrase.push_str(&new_unit_hundred);
        phrase.push(' ');
        phrase.push('و');
        phrase.push_str(&new_unit_one);
    } else if n % 100 == 2 {
        // مئة دينار ودينارين: regenerate the phrase number for n-2, then add
        // the unit for hundreds, then add two units
        let number_phrase_hundred =
            number_to_arabic_words_inflected((n - 2) as i64, feminin, true, inflection);
        let new_unit_hundred = get_form(unit, "one", "jar");
        let new_unit_two = get_form(unit, "two", inflection);
        unit_inflection = inflection;
        unit_number = "two";
        new_unit = new_unit_two.clone();
        phrase.push_str(&number_phrase_hundred);
        phrase.push(' ');
        phrase.push_str(&new_unit_hundred);
        phrase.push(' ');
        phrase.push('و');
        phrase.push_str(&new_unit_two);
    } else if (3..=10).contains(&(n % 100)) {
        // خمسة دنانير
        new_unit = get_form(unit, "plural", "jar");
        unit_inflection = "jar";
        unit_number = "plural";
        phrase.push_str(&number_phrase);
        phrase.push(' ');
        phrase.push_str(&new_unit);
    } else if n % 100 >= 11 {
        // أحد عشر رجلا
        new_unit = get_form(unit, "one", "nasb");
        unit_inflection = "nasb";
        unit_number = "one";
        phrase.push_str(&number_phrase);
        phrase.push(' ');
        phrase.push_str(&new_unit);
    } else if n % 100 == 0 {
        // مئة دينار
        new_unit = get_form(unit, "one", "jar");
        unit_inflection = "jar";
        unit_number = "one";
        phrase.push_str(&number_phrase);
        phrase.push(' ');
        phrase.push_str(&new_unit);
    } else {
        phrase.push_str(&number_phrase);
        phrase.push_str(" **");
        phrase.push_str(unit);
        phrase.push_str("**");
        new_unit = String::new();
    }
    NumberUnitsPhrase {
        all: phrase,
        phrase: number_phrase,
        unit: new_unit,
        unit_number: unit_number.to_string(),
        unit_inflection: unit_inflection.to_string(),
    }
}

/// `ArabicNumbersWords.getSuggestionsNumericPhraseWithUnits` (upstream takes
/// `feminin`/`attached` but never uses them).
pub fn get_suggestions_numeric_phrase_with_units(
    phrase_input: &str,
    unit: &str,
    _feminin: bool,
    _attached: bool,
    inflection: &str,
) -> Vec<NumberUnitsPhrase> {
    let mut suggestions = Vec::new();
    let phrase = remove_tashkeel(phrase_input);
    let x = text_to_number(&phrase);
    let auto_phrase_map = number_to_words_with_units_map(x, unit, inflection);
    let auto_phrase = remove_tashkeel(&auto_phrase_map.phrase);
    if auto_phrase != phrase {
        // if inflection is null generate all cases
        if !inflection.is_empty() {
            suggestions.push(auto_phrase_map);
        } else {
            // Raf3
            suggestions.push(number_to_words_with_units_map(x, unit, "raf3"));
            // nasb+jar
            suggestions.push(number_to_words_with_units_map(x, unit, "jar"));
        }
    }
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `ArabicNumberPhraseFilterTest.testFilter` phrases, with the
    /// expected jar suggestions re-derived from the engine (a phrase that is
    /// already correct gets no suggestion).
    #[test]
    fn number_engine_matches_java_test_cases() {
        let jar = |phrase: &str| get_suggestions_numeric_phrase(phrase, false, false, "jar");
        assert!(jar("صفر").is_empty());
        assert!(jar("واحد").is_empty());
        assert_eq!(jar("اثنان"), vec!["اثنين"]);
        assert!(jar("ثلاثة").is_empty());
        assert_eq!(jar("إحدى عشر"), vec!["أحد عشر"]);
        assert_eq!(jar("اثنتي عشر"), vec!["اثني عشر"]);
        assert!(jar("أربعة عشر").is_empty());
        assert_eq!(jar("أربعة وثلاثون"), vec!["أربعة وثلاثين"]);
        assert!(jar("مائة").is_empty());
        assert_eq!(jar("مائة وخمسة وعشرون"), vec!["مائة وخمسة وعشرين"]);
        assert!(jar("مائة وأربعة وثلاثين").is_empty());
        assert_eq!(
            jar("ألف وتسعمائة واثنان وعشرين"),
            vec!["ألف وتسعمائة واثنين وعشرين"]
        );
        assert_eq!(
            jar("مليون ومئتان وخمسة وأربعين ألفاً وسبعمائة وواحد"),
            vec!["مليون ومئتين وخمسة وأربعين ألفا وسبعمائة وواحد"]
        );
        assert!(jar("مائة واثنين").is_empty());
        assert_eq!(jar("عشرة وآلاف"), vec!["عشرة آلاف"]);
        // An empty inflection generates the raf3 and jar phrases when the
        // phrase is not already the raf3 form.
        assert!(get_suggestions_numeric_phrase("أربعة وثلاثون", false, false, "").is_empty());
        assert_eq!(
            get_suggestions_numeric_phrase("أربعة وثلاثين", false, false, ""),
            vec!["أربعة وثلاثون", "أربعة وثلاثين"]
        );
    }

    /// `numberToWordsWithUnitsMap`: phrase/unit agreement for the `دينار`
    /// unit (the filter renders `{previous} {phrase} {inflected unit}`).
    #[test]
    fn unit_engine_matches_java_forms() {
        let m = number_to_words_with_units_map(1, "دينار", "");
        assert_eq!((m.phrase.as_str(), m.unit.as_str()), ("واحد", "دينار"));
        assert_eq!(m.all, "دينار واحد");
        let m = number_to_words_with_units_map(2, "دينار", "");
        assert_eq!((m.phrase.as_str(), m.unit.as_str()), ("اثنان", "ديناران"));
        let m = number_to_words_with_units_map(3, "دينار", "");
        assert_eq!(
            (
                m.phrase.as_str(),
                m.unit.as_str(),
                m.unit_inflection.as_str(),
                m.unit_number.as_str(),
            ),
            ("ثلاثة", "دنانيرَ", "jar", "plural")
        );
        assert_eq!(m.all, "ثلاثة دنانيرَ");
        let m = number_to_words_with_units_map(11, "دينار", "");
        assert_eq!(
            (
                m.phrase.as_str(),
                m.unit.as_str(),
                m.unit_inflection.as_str(),
                m.unit_number.as_str(),
            ),
            ("أحد عشر", "دينارًا", "nasb", "one")
        );
        let m = number_to_words_with_units_map(100, "دينار", "");
        assert_eq!(
            (
                m.phrase.as_str(),
                m.unit.as_str(),
                m.unit_inflection.as_str(),
                m.unit_number.as_str(),
            ),
            ("مائة", "دينارٍ", "jar", "one")
        );
        let m = number_to_words_with_units_map(0, "دينار", "");
        assert_eq!(m.all, "لا دنانيرَ");
        // The feminine unit and the unknown-unit placeholder (`getForm`).
        assert!(is_feminin("ليرة"));
        assert!(!is_feminin("دينار"));
        assert_eq!(get_form("ليرة", "one", "nasb"), "ليرةً");
        assert_eq!(get_form("دينار", "two", "jar"), "دينارين");
        assert_eq!(get_form("فرس", "one", "raf3"), "[[فرس]]");
        assert_eq!(get_form("دينار", "one", ""), "دينار");
    }

    /// The pinned corpus example (`في مليونان ... صندوق.`): the phrase maps to
    /// 2245701 and the jar phrase is the golden suggestion (modulo the
    /// stripped tanween on ألفاً).
    #[test]
    fn corpus_number_phrase() {
        let phrase = "مليونان ومئتان وخمسة وأربعون ألفاً وسبعمائة وواحد";
        assert_eq!(text_to_number(phrase), 2245701);
        let sugs = get_suggestions_numeric_phrase(phrase, false, false, "jar");
        assert_eq!(
            sugs,
            vec!["مليونين ومئتين وخمسة وأربعين ألفا وسبعمائة وواحد"]
        );
    }
}
