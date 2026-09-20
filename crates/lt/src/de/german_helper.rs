//! `GermanHelper` + `AnalyzedGermanToken` (`tagging.de`): parse the Morphy
//! POS tags into type/case/number/gender/determination and the agreement
//! category strings used by `AgreementTools`.

use lt_core::{AnalyzedToken, AnalyzedTokenReadings};

/// `GermanToken.POSType` (only the values the agreement code distinguishes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PosType {
    Nomen,
    Verb,
    Adjektiv,
    Determiner,
    Pronomen,
    Partizip,
    ProperNoun,
}

/// `GermanToken.Kasus`; `to_string` values match the Java enum names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kasus {
    Nominativ,
    Akkusativ,
    Dativ,
    Genitiv,
}

impl Kasus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Nominativ => "Nominativ",
            Self::Akkusativ => "Akkusativ",
            Self::Dativ => "Dativ",
            Self::Genitiv => "Genitiv",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Numerus {
    Singular,
    Plural,
}

impl Numerus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Singular => "Singular",
            Self::Plural => "Plural",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Genus {
    Neutrum,
    Maskulinum,
    Femininum,
    Allgemein,
}

impl Genus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Neutrum => "Neutrum",
            Self::Maskulinum => "Maskulinum",
            Self::Femininum => "Femininum",
            Self::Allgemein => "Allgemein",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Determination {
    Definite,
    Indefinite,
}

impl Determination {
    fn as_str(self) -> &'static str {
        match self {
            Self::Definite => "definit",
            Self::Indefinite => "indefinit",
        }
    }
}

/// `GermanToken.GrammarCategory` (`AgreementRule.GrammarCategory`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GrammarCategory {
    Kasus,
    Genus,
    Numerus,
}

impl GrammarCategory {
    /// The `displayName` used by `getCategoriesCausingError` (upstream only
    /// called from its test; kept for parity).
    #[allow(dead_code)]
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::Kasus => {
                "Kasus (Fall: Wer/Was, Wessen, Wem, Wen/Was - Beispiel: 'das Fahrrads' statt 'des Fahrrads')"
            }
            Self::Genus => {
                "Genus (männlich, weiblich, sächlich - Beispiel: 'der Fahrrad' statt 'das Fahrrad')"
            }
            Self::Numerus => {
                "Numerus (Einzahl, Mehrzahl - Beispiel: 'das Fahrräder' statt 'die Fahrräder')"
            }
        }
    }
}

/// `AnalyzedGermanToken`: the parsed properties of one reading.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AnalyzedGermanToken {
    pub(crate) pos_type: Option<PosType>,
    pub(crate) casus: Option<Kasus>,
    pub(crate) numerus: Option<Numerus>,
    pub(crate) genus: Option<Genus>,
    pub(crate) determination: Option<Determination>,
}

impl AnalyzedGermanToken {
    /// Java constructor: tags with fewer than three `:`-separated parts are
    /// all-null.
    pub(crate) fn parse(token: &AnalyzedToken) -> Self {
        let Some(pos_tag) = token.pos_tag.as_deref() else {
            return Self::default();
        };
        let parts: Vec<&str> = pos_tag.split(':').collect();
        if parts.len() < 3 {
            return Self::default();
        }
        let mut out = Self::default();
        for part in parts {
            match part {
                "EIG" => out.pos_type = Some(PosType::ProperNoun),
                "SUB" if out.pos_type.is_none() => out.pos_type = Some(PosType::Nomen),
                "PA1" | "PA2" => out.pos_type = Some(PosType::Partizip),
                "VER" if out.pos_type.is_none() => out.pos_type = Some(PosType::Verb),
                "ADJ" if out.pos_type.is_none() => out.pos_type = Some(PosType::Adjektiv),
                "PRO" if out.pos_type.is_none() => out.pos_type = Some(PosType::Pronomen),
                "ART" if out.pos_type.is_none() => out.pos_type = Some(PosType::Determiner),
                "AKK" => out.casus = Some(Kasus::Akkusativ),
                "GEN" => out.casus = Some(Kasus::Genitiv),
                "NOM" => out.casus = Some(Kasus::Nominativ),
                "DAT" => out.casus = Some(Kasus::Dativ),
                "PLU" => out.numerus = Some(Numerus::Plural),
                "SIN" => out.numerus = Some(Numerus::Singular),
                "MAS" => out.genus = Some(Genus::Maskulinum),
                "FEM" => out.genus = Some(Genus::Femininum),
                "NEU" => out.genus = Some(Genus::Neutrum),
                "NOG" => out.genus = Some(Genus::Femininum),
                "ALG" => out.genus = Some(Genus::Allgemein),
                "IND" => out.determination = Some(Determination::Indefinite),
                "DEF" => out.determination = Some(Determination::Definite),
                _ => {}
            }
        }
        out
    }
}

/// `GermanHelper.hasReadingOfType`.
pub(crate) fn has_reading_of_type(token: &AnalyzedTokenReadings, pos_type: PosType) -> bool {
    for reading in &token.readings {
        if let Some(tag) = reading.pos_tag.as_deref() {
            if tag == "SENT_END" || tag == "PARA_END" {
                return false;
            }
        }
        if AnalyzedGermanToken::parse(reading).pos_type == Some(pos_type) {
            return true;
        }
    }
    false
}

/// `AgreementTools.getAgreementCategories`.
pub(crate) fn get_agreement_categories(
    token: &AnalyzedTokenReadings,
    omit: &[GrammarCategory],
    skip_sol: bool,
) -> Vec<String> {
    let mut set: Vec<String> = Vec::new();
    for reading in &token.readings {
        if skip_sol
            && reading
                .pos_tag
                .as_deref()
                .is_some_and(|t| t.ends_with(":SOL"))
        {
            continue;
        }
        let parsed = AnalyzedGermanToken::parse(reading);
        if parsed.casus.is_none() && parsed.numerus.is_none() && parsed.genus.is_none() {
            continue;
        }
        if parsed.genus == Some(Genus::Allgemein)
            && reading
                .pos_tag
                .as_deref()
                .is_some_and(|t| !t.ends_with(":STV"))
            && !possessive_special_case(token, reading)
        {
            // expand ALG so that e.g. "Ich Arbeiter" is not flagged
            if parsed.determination.is_none() {
                for genus in [Genus::Maskulinum, Genus::Femininum, Genus::Neutrum] {
                    for det in [Determination::Definite, Determination::Indefinite] {
                        push_unique(
                            &mut set,
                            make_string(parsed.casus, parsed.numerus, Some(genus), Some(det), omit),
                        );
                    }
                }
            } else {
                for genus in [Genus::Maskulinum, Genus::Femininum, Genus::Neutrum] {
                    push_unique(
                        &mut set,
                        make_string(
                            parsed.casus,
                            parsed.numerus,
                            Some(genus),
                            parsed.determination,
                            omit,
                        ),
                    );
                }
            }
        } else if parsed.determination.is_none()
            || matches!(reading.stem.as_deref(), Some("jed") | Some("manch"))
        {
            for det in [Determination::Definite, Determination::Indefinite] {
                push_unique(
                    &mut set,
                    make_string(parsed.casus, parsed.numerus, parsed.genus, Some(det), omit),
                );
            }
        } else {
            push_unique(
                &mut set,
                make_string(
                    parsed.casus,
                    parsed.numerus,
                    parsed.genus,
                    parsed.determination,
                    omit,
                ),
            );
        }
    }
    set
}

/// `AgreementTools.getAgreementSOLCategories` (only referenced from the
/// commented-out branch of `AgreementRule2` upstream; kept for parity).
#[allow(dead_code)]
pub(crate) fn get_agreement_sol_categories(
    token: &AnalyzedTokenReadings,
    omit: &[GrammarCategory],
) -> Vec<String> {
    let mut set: Vec<String> = Vec::new();
    for reading in &token.readings {
        if !reading
            .pos_tag
            .as_deref()
            .is_some_and(|t| t.ends_with(":SOL"))
        {
            continue;
        }
        let parsed = AnalyzedGermanToken::parse(reading);
        if parsed.casus.is_none() && parsed.numerus.is_none() && parsed.genus.is_none() {
            continue;
        }
        for det in [Determination::Definite, Determination::Indefinite] {
            push_unique(
                &mut set,
                make_string(parsed.casus, parsed.numerus, parsed.genus, Some(det), omit),
            );
        }
    }
    set
}

fn possessive_special_case(token: &AnalyzedTokenReadings, reading: &AnalyzedToken) -> bool {
    token.has_pos_tag_starting_with("PRO:POS")
        && matches!(reading.stem.as_deref(), Some("ich") | Some("sich"))
}

fn make_string(
    casus: Option<Kasus>,
    numerus: Option<Numerus>,
    genus: Option<Genus>,
    determination: Option<Determination>,
    omit: &[GrammarCategory],
) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(casus) = casus {
        if !omit.contains(&GrammarCategory::Kasus) {
            parts.push(casus.as_str());
        }
    }
    if let Some(numerus) = numerus {
        if !omit.contains(&GrammarCategory::Numerus) {
            parts.push(numerus.as_str());
        }
    }
    if let Some(genus) = genus {
        if !omit.contains(&GrammarCategory::Genus) {
            parts.push(genus.as_str());
        }
    }
    if let Some(determination) = determination {
        parts.push(determination.as_str());
    }
    parts.join("/")
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !list.contains(&value) {
        list.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(pos: &str) -> AnalyzedToken {
        AnalyzedToken::new("x", Some("x".into()), Some(pos.into()))
    }

    #[test]
    fn parses_types_and_categories() {
        let parsed = AnalyzedGermanToken::parse(&tok("ART:DEF:NOM:SIN:MAS"));
        assert_eq!(parsed.pos_type, Some(PosType::Determiner));
        assert_eq!(parsed.casus, Some(Kasus::Nominativ));
        assert_eq!(parsed.genus, Some(Genus::Maskulinum));
        assert_eq!(parsed.determination, Some(Determination::Definite));
        let parsed = AnalyzedGermanToken::parse(&tok("PA2:PRD:GRU:VER"));
        assert_eq!(parsed.pos_type, Some(PosType::Partizip));
        let parsed = AnalyzedGermanToken::parse(&tok("PKT"));
        assert_eq!(parsed.pos_type, None);
    }

    #[test]
    fn categories_match_java_names() {
        let token = AnalyzedTokenReadings::new(vec![tok("SUB:NOM:SIN:NEU")]);
        let set = get_agreement_categories(&token, &[], false);
        assert_eq!(
            set,
            vec![
                "Nominativ/Singular/Neutrum/definit".to_string(),
                "Nominativ/Singular/Neutrum/indefinit".to_string()
            ]
        );
    }
}
