//! `LanguageNames` (`rules/de`): the language-name list used by `CaseRule`
//! (`isLanguage`) and `CaseRuleAntiPatterns` (`getAsRegex`). The Java set is
//! a `HashSet`; `getAsRegex` joins it in iteration order, but the regex is
//! used for a full match of a single alternative, so the order is not
//! observable — the source order is kept here.

/// The `languages` set (source order).
pub(crate) static LANGUAGE_NAMES: [&str; 84] = [
    "Angelsächsisch",
    "Afrikanisch",
    "Albanisch",
    "Altarabisch",
    "Altchinesisch",
    "Altgriechisch",
    "Althochdeutsch",
    "Altpersisch",
    "Amerikanisch",
    "Arabisch",
    "Armenisch",
    "Bairisch",
    "Baskisch",
    "Bengalisch",
    "Bulgarisch",
    "Chinesisch",
    "Dänisch",
    "Deutsch",
    "Englisch",
    "Estnisch",
    "Finnisch",
    "Französisch",
    "Frühneuhochdeutsch",
    "Germanisch",
    "Georgisch",
    "Griechisch",
    "Hebräisch",
    "Hocharabisch",
    "Hochchinesisch",
    "Hochdeutsch",
    "Holländisch",
    "Indonesisch",
    "Irisch",
    "Isländisch",
    "Italienisch",
    "Japanisch",
    "Jiddisch",
    "Jugoslawisch",
    "Kantonesisch",
    "Katalanisch",
    "Klingonisch",
    "Koreanisch",
    "Kroatisch",
    "Kurdisch",
    "Lateinisch",
    "Lettisch",
    "Litauisch",
    "Luxemburgisch",
    "Mittelhochdeutsch",
    "Mongolisch",
    "Neuhochdeutsch",
    "Niederländisch",
    "Norwegisch",
    "Persisch",
    "Plattdeutsch",
    "Polnisch",
    "Portugiesisch",
    "Rätoromanisch",
    "Rumänisch",
    "Russisch",
    "Sächsisch",
    "Schwäbisch",
    "Schwedisch",
    "Schweizerisch",
    "Serbisch",
    "Serbokroatisch",
    "Slawisch",
    "Slowakisch",
    "Slowenisch",
    "Spanisch",
    "Syrisch",
    "Tamilisch",
    "Tibetisch",
    "Tschechisch",
    "Tschetschenisch",
    "Türkisch",
    "Turkmenisch",
    "Uigurisch",
    "Ukrainisch",
    "Ungarisch",
    "Usbekisch",
    "Vietnamesisch",
    "Walisisch",
    "Weißrussisch",
];

/// `LanguageNames.get()`.
pub(crate) fn get() -> &'static [&'static str] {
    &LANGUAGE_NAMES
}

/// `LanguageNames.getAsRegex()` (the antipattern list inlines the value).
#[allow(dead_code)]
pub(crate) fn get_as_regex() -> String {
    LANGUAGE_NAMES.join("|")
}
