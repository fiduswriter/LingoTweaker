//! Generated data tables from LT's `GermanSpellerRule`
//! (pinned commit 01d07e1f6165). Regenerate with
//! `tools/gen-de-speller-data.py [--upstream <lt-checkout>]`.

/// One suggestion expression of `GermanSpellerRule`'s curated
/// `ADDITIONAL_SUGGESTIONS` map (Java lambdas over the word).
pub(crate) enum SuggestExpr {
    /// A literal replacement.
    Lit(&'static str),
    /// `w.replaceFirst(pattern, replacement)` (Java regex, `$1` groups).
    ReplaceFirst {
        pattern: &'static str,
        replacement: &'static str,
    },
    /// `StringUtils.replaceOnce(w, from, to)` (literal substring).
    ReplaceOnce {
        from: &'static str,
        to: &'static str,
    },
    /// `uppercaseFirstChar(<one inner expression>)`.
    UpperFirst(&'static [SuggestExpr]),
}

/// `GermanSpellerRule.ADDITIONAL_SUGGESTIONS` in insertion order
/// (the first matching pattern wins; Java `StringMatcher.regexp` =
/// full match, case-sensitive).
pub(crate) static ADDITIONAL_SUGGESTIONS: &[(&str, &[SuggestExpr])] = &[
    (
        "lieder",
        &[SuggestExpr::Lit("leider"), SuggestExpr::Lit("Lieder")],
    ),
    (
        "vorbreiten",
        &[
            SuggestExpr::Lit("vorbereiten"),
            SuggestExpr::Lit("verbreiten"),
        ],
    ),
    (
        "Hungen",
        &[SuggestExpr::Lit("Hunger"), SuggestExpr::Lit("Hungern")],
    ),
    ("Topfen", &[SuggestExpr::Lit("Tropfen")]),
    (
        "gepart",
        &[SuggestExpr::Lit("gespart"), SuggestExpr::Lit("gepaart")],
    ),
    ("frägst", &[SuggestExpr::Lit("fragst")]),
    ("totkrank", &[SuggestExpr::Lit("todkrank")]),
    ("todtkrank", &[SuggestExpr::Lit("todkrank")]),
    ("sähte", &[SuggestExpr::Lit("säte")]),
    ("säht", &[SuggestExpr::Lit("sät")]),
    ("sähtest", &[SuggestExpr::Lit("sätest")]),
    ("sähten", &[SuggestExpr::Lit("säten")]),
    ("sähtet", &[SuggestExpr::Lit("sätet")]),
    ("gesäht", &[SuggestExpr::Lit("gesät")]),
    ("sähend", &[SuggestExpr::Lit("säend")]),
    ("Impflicht", &[SuggestExpr::Lit("Impfpflicht")]),
    ("Wandererin", &[SuggestExpr::Lit("Wanderin")]),
    ("daß", &[SuggestExpr::Lit("dass")]),
    ("eien", &[SuggestExpr::Lit("eine")]),
    ("wiederrum", &[SuggestExpr::Lit("wiederum")]),
    (
        "ne",
        &[
            SuggestExpr::Lit("'ne"),
            SuggestExpr::Lit("eine"),
            SuggestExpr::Lit("nein"),
            SuggestExpr::Lit("oder"),
        ],
    ),
    ("ner", &[SuggestExpr::Lit("einer")]),
    (
        "isses",
        &[SuggestExpr::Lit("ist es"), SuggestExpr::Lit("Risses")],
    ),
    ("isser", &[SuggestExpr::Lit("ist er")]),
    ("Vieleicht", &[SuggestExpr::Lit("Vielleicht")]),
    ("inbetracht", &[SuggestExpr::Lit("in Betracht")]),
    ("überwhatsapp", &[SuggestExpr::Lit("über WhatsApp")]),
    ("überzoom", &[SuggestExpr::Lit("über Zoom")]),
    ("überweißt", &[SuggestExpr::Lit("überweist")]),
    ("übergoogle", &[SuggestExpr::Lit("über Google")]),
    ("einlogen", &[SuggestExpr::Lit("einloggen")]),
    ("Kruks", &[SuggestExpr::Lit("Krux")]),
    ("Filterbubble", &[SuggestExpr::Lit("Filterblase")]),
    ("Filterbubbles", &[SuggestExpr::Lit("Filterblasen")]),
    ("Telefones", &[SuggestExpr::Lit("Telefons")]),
    (
        ".+telefones",
        &[SuggestExpr::ReplaceFirst {
            pattern: "telefones",
            replacement: "telefons",
        }],
    ),
    (
        ".+tips",
        &[SuggestExpr::ReplaceFirst {
            pattern: "tip",
            replacement: "tipp",
        }],
    ),
    (
        "Hifi-.+",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Hifi",
            replacement: "HiFi",
        }],
    ),
    (
        "Analgen.*",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Analgen",
            replacement: "Anlagen",
        }],
    ),
    (
        "wiedersteh(en|st|t)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "wiederstan(d|den|dest)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "wiedersprech(e|t|en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "wiedersprich(st|t)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "wiedersprach(st|t|en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "wiederruf(e|st|t|en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "wiederrief(st|t|en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "wiederleg(e|st|t|en|te|ten)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "wiederhall(e|st|t|en|te|ten)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "wiedersetz(e|t|en|te|ten)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "wiederstreb(e|st|t|en|te|ten)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wieder",
            replacement: "wider",
        }],
    ),
    (
        "zurück(ge|zu)?koppe?l(e|n|t(e(st|n)?)?|nd(e(r|s|m|n)?)?|st|)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "zurück",
            replacement: "rück",
        }],
    ),
    ("bekomms", &[SuggestExpr::Lit("bekomm es")]),
    ("Latin", &[SuggestExpr::Lit("Latein")]),
    ("liegts", &[SuggestExpr::Lit("liegt es")]),
    ("gesynct", &[SuggestExpr::Lit("synchronisiert")]),
    ("gesynced", &[SuggestExpr::Lit("synchronisiert")]),
    ("gesyncht", &[SuggestExpr::Lit("synchronisiert")]),
    ("gesyngt", &[SuggestExpr::Lit("synchronisiert")]),
    ("synce", &[SuggestExpr::Lit("synchronisiere")]),
    ("synche", &[SuggestExpr::Lit("synchronisiere")]),
    ("syncen", &[SuggestExpr::Lit("synchronisieren")]),
    ("synchen", &[SuggestExpr::Lit("synchronisieren")]),
    ("wiederspiegelten", &[SuggestExpr::Lit("widerspiegelten")]),
    ("wiedererwarten", &[SuggestExpr::Lit("wider Erwarten")]),
    ("widerholen", &[SuggestExpr::Lit("wiederholen")]),
    ("wiederhohlen", &[SuggestExpr::Lit("wiederholen")]),
    ("herrunterladen", &[SuggestExpr::Lit("herunterladen")]),
    ("dastellen", &[SuggestExpr::Lit("darstellen")]),
    ("zuviel", &[SuggestExpr::Lit("zu viel")]),
    ("abgekatertes", &[SuggestExpr::Lit("abgekartetes")]),
    ("wiederspiegelt", &[SuggestExpr::Lit("widerspiegelt")]),
    ("Komplexheit", &[SuggestExpr::Lit("Komplexität")]),
    ("unterschiedet", &[SuggestExpr::Lit("unterscheidet")]),
    ("einzigst", &[SuggestExpr::Lit("einzig")]),
    ("Einzigst", &[SuggestExpr::Lit("Einzig")]),
    ("geschumpfen", &[SuggestExpr::Lit("geschimpft")]),
    ("Geschumpfen", &[SuggestExpr::Lit("Geschimpft")]),
    ("Oke", &[SuggestExpr::Lit("Okay")]),
    ("Mü", &[SuggestExpr::Lit("My")]),
    ("packs", &[SuggestExpr::Lit("pack es")]),
    ("abschiednehmen", &[SuggestExpr::Lit("Abschied nehmen")]),
    (
        "wars",
        &[SuggestExpr::Lit("war es"), SuggestExpr::Lit("warst")],
    ),
    (
        "[aA]wa",
        &[
            SuggestExpr::Lit("AWA"),
            SuggestExpr::Lit("ach was"),
            SuggestExpr::Lit("aber"),
        ],
    ),
    (
        "[aA]lsallerersten?s",
        &[
            SuggestExpr::ReplaceFirst {
                pattern: "lsallerersten?s",
                replacement: "ls allererstes",
            },
            SuggestExpr::ReplaceFirst {
                pattern: "lsallerersten?s",
                replacement: "ls Allererstes",
            },
        ],
    ),
    (
        "(an|auf|ein|zu)gehangen(e[mnrs]?)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "hangen",
            replacement: "hängt",
        }],
    ),
    (
        "[oO]key",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ey$",
            replacement: "ay",
        }],
    ),
    ("packet", &[SuggestExpr::Lit("Paket")]),
    ("Thanks", &[SuggestExpr::Lit("Danke")]),
    ("Ghanesen?", &[SuggestExpr::Lit("Ghanaer")]),
    ("Thumberg", &[SuggestExpr::Lit("Thunberg")]),
    ("Allalei", &[SuggestExpr::Lit("Allerlei")]),
    ("geupdate[dt]$", &[SuggestExpr::Lit("upgedatet")]),
    (
        "[pP]roblemhaft(e[nmrs]?)?",
        &[
            SuggestExpr::ReplaceOnce {
                from: "haft",
                to: "behaftet",
            },
            SuggestExpr::ReplaceOnce {
                from: "haft",
                to: "atisch",
            },
        ],
    ),
    (
        "rosane[mnrs]?$",
        &[
            SuggestExpr::Lit("rosa"),
            SuggestExpr::ReplaceFirst {
                pattern: "^rosan",
                replacement: "rosafarben",
            },
        ],
    ),
    (
        "Erbung",
        &[SuggestExpr::Lit("Vererbung"), SuggestExpr::Lit("Erbschaft")],
    ),
    (
        "Energiesparung",
        &[
            SuggestExpr::Lit("Energieeinsparung"),
            SuggestExpr::Lit("Energieersparnis"),
        ],
    ),
    ("Abbrechung", &[SuggestExpr::Lit("Abbruch")]),
    (
        "Abbrechungen",
        &[SuggestExpr::Lit("Abbrüche"), SuggestExpr::Lit("Abbrüchen")],
    ),
    (
        "Urteilung",
        &[SuggestExpr::Lit("Urteil"), SuggestExpr::Lit("Verurteilung")],
    ),
    (
        "allmöglichen?",
        &[
            SuggestExpr::Lit("alle möglichen"),
            SuggestExpr::Lit("alle mögliche"),
        ],
    ),
    (
        "Krankenhausen",
        &[
            SuggestExpr::Lit("Krankenhäusern"),
            SuggestExpr::Lit("Krankenhäuser"),
        ],
    ),
    (
        "vorr?auss?etzlich",
        &[
            SuggestExpr::Lit("voraussichtlich"),
            SuggestExpr::Lit("vorausgesetzt"),
        ],
    ),
    (
        "nichtmals",
        &[
            SuggestExpr::Lit("nicht mal"),
            SuggestExpr::Lit("nicht einmal"),
        ],
    ),
    ("eingepeilt", &[SuggestExpr::Lit("angepeilt")]),
    ("gekukt", &[SuggestExpr::Lit("geguckt")]),
    (
        "nem",
        &[SuggestExpr::Lit("'nem"), SuggestExpr::Lit("einem")],
    ),
    (
        "nen",
        &[SuggestExpr::Lit("'nen"), SuggestExpr::Lit("einen")],
    ),
    ("geb", &[SuggestExpr::Lit("gebe")]),
    ("überhaut", &[SuggestExpr::Lit("überhaupt")]),
    ("nacher", &[SuggestExpr::Lit("nachher")]),
    ("jeztz", &[SuggestExpr::Lit("jetzt")]),
    ("les", &[SuggestExpr::Lit("lese")]),
    ("wr", &[SuggestExpr::Lit("wir")]),
    ("bezweifel", &[SuggestExpr::Lit("bezweifle")]),
    ("verzweifel", &[SuggestExpr::Lit("verzweifle")]),
    ("zweifel", &[SuggestExpr::Lit("zweifle")]),
    (
        "[wW]ah?rscheindlichkeit",
        &[SuggestExpr::Lit("Wahrscheinlichkeit")],
    ),
    ("Hijab", &[SuggestExpr::Lit("Hidschāb")]),
    ("[lL]eerequiment", &[SuggestExpr::Lit("Leerequipment")]),
    (
        "unauslässlich",
        &[
            SuggestExpr::Lit("unerlässlich"),
            SuggestExpr::Lit("unablässig"),
            SuggestExpr::Lit("unauslöschlich"),
        ],
    ),
    (
        "klappts",
        &[
            SuggestExpr::Lit("klappt’s"),
            SuggestExpr::Lit("klappt es"),
            SuggestExpr::Lit("klappst"),
        ],
    ),
    (
        "Klappts",
        &[
            SuggestExpr::Lit("Klappt’s"),
            SuggestExpr::Lit("Klappt es"),
            SuggestExpr::Lit("Klappst"),
        ],
    ),
    (
        "schicks",
        &[
            SuggestExpr::Lit("schick’s"),
            SuggestExpr::Lit("schick es"),
            SuggestExpr::Lit("schickst"),
        ],
    ),
    (
        "Schicks",
        &[
            SuggestExpr::Lit("Schick’s"),
            SuggestExpr::Lit("Schick es"),
            SuggestExpr::Lit("Schickst"),
        ],
    ),
    ("Registration", &[SuggestExpr::Lit("Registrierung")]),
    ("Registrationen", &[SuggestExpr::Lit("Registrierungen")]),
    ("Spinnenweben", &[SuggestExpr::Lit("Spinnweben")]),
    ("[Tt]uneup", &[SuggestExpr::Lit("TuneUp")]),
    (
        "[Ww]ar ne",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ne",
            replacement: "eine",
        }],
    ),
    (
        "[Ää]nliche[rnms]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "nlich",
            replacement: "hnlich",
        }],
    ),
    (
        "[Gg]arnix",
        &[SuggestExpr::ReplaceFirst {
            pattern: "nix",
            replacement: "nichts",
        }],
    ),
    (
        "[Ww]i",
        &[SuggestExpr::ReplaceFirst {
            pattern: "i",
            replacement: "ie",
        }],
    ),
    (
        "[uU]nauslässlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "aus",
            replacement: "er",
        }],
    ),
    (
        "[vV]erewiglicht(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "",
        }],
    ),
    (
        "[zZ]eritifiert(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "eritifiert",
            replacement: "ertifiziert",
        }],
    ),
    (
        "gerähten?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "geräht",
            replacement: "Gerät",
        }],
    ),
    (
        "leptops?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lep",
            replacement: "Lap",
        }],
    ),
    (
        "[pP]ie?rsings?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[pP]ie?rsing",
            replacement: "Piercing",
        }],
    ),
    (
        "for?melar(en?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "for?me",
            replacement: "Formu",
        }],
    ),
    (
        "näste[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^näs",
            replacement: "nächs",
        }],
    ),
    (
        "Erdogans?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^Erdogan",
            replacement: "Erdoğan",
        }],
    ),
    ("Germanistiker[ns]", &[SuggestExpr::Lit("Germanisten")]),
    ("Sodastream", &[SuggestExpr::Lit("SodaStream")]),
    ("Soda-Stream", &[SuggestExpr::Lit("SodaStream")]),
    ("Sodastreams", &[SuggestExpr::Lit("SodaStreams")]),
    ("Soda-Streams", &[SuggestExpr::Lit("SodaStreams")]),
    ("Motor-Flugzeug", &[SuggestExpr::Lit("Motorflugzeug")]),
    ("Motor-Flugzeugs", &[SuggestExpr::Lit("Motorflugzeugs")]),
    ("Motor-Flugzeuge", &[SuggestExpr::Lit("Motorflugzeuge")]),
    ("Motor-Flugzeugen", &[SuggestExpr::Lit("Motorflugzeugen")]),
    (
        "Germanistikerin(nen)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Germanistiker",
            replacement: "Germanist",
        }],
    ),
    (
        "[iI]ns?z[ie]nie?rung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[iI]ns?z[ie]nie?",
            replacement: "Inszenie",
        }],
    ),
    (
        "[eE]rhöherung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[eE]rhöherung",
            replacement: "Erhöhung",
        }],
    ),
    (
        "[vV]erspäterung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "später",
            replacement: "spät",
        }],
    ),
    (
        "[vV]orallendingen",
        &[SuggestExpr::ReplaceFirst {
            pattern: "orallendingen",
            replacement: "or allen Dingen",
        }],
    ),
    (
        "[aA]ufjede[nm]fall",
        &[SuggestExpr::ReplaceFirst {
            pattern: "jede[nm]fall$",
            replacement: " jeden Fall",
        }],
    ),
    (
        "[aA]us[vf]ersehen[dt]lich",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[vf]ersehen[dt]lich",
            replacement: " Versehen",
        }],
    ),
    (
        "^funk?z[ou]nier.+",
        &[SuggestExpr::ReplaceFirst {
            pattern: "funk?z[ou]nier",
            replacement: "funktionier",
        }],
    ),
    (
        "[wW]öruber",
        &[SuggestExpr::ReplaceFirst {
            pattern: "öru",
            replacement: "orü",
        }],
    ),
    (
        "[lL]einensamens?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[lL]einen",
            replacement: "Lein",
        }],
    ),
    (
        "Feinleiner[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Feinlei",
            replacement: "Fineli",
        }],
    ),
    (
        "[hH]eilei[td]s?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[hH]eilei[td]",
            replacement: "Highlight",
        }],
    ),
    (
        "Oldheimer[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "he",
            replacement: "t",
        }],
    ),
    (
        "[tT]räner[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[tT]rä",
            replacement: "Trai",
        }],
    ),
    (
        "[tT]eimings?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[tT]e",
            replacement: "T",
        }],
    ),
    (
        "unternehmensl[uü]stig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "mensl[uü]st",
            replacement: "mungslust",
        }],
    ),
    (
        "proff?ess?ional(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ff?ess?ional",
            replacement: "fessionell",
        }],
    ),
    (
        "zuverlässlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "ig",
        }],
    ),
    (
        "fluoreszenzierend(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "zen",
            replacement: "",
        }],
    ),
    (
        "revalierend(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^reval",
            replacement: "rivalis",
        }],
    ),
    (
        "verhäuft(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^ver",
            replacement: "ge",
        }],
    ),
    (
        "stürmig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "mig",
            replacement: "misch",
        }],
    ),
    (
        "größeste[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ßes",
            replacement: "ß",
        }],
    ),
    (
        "n[aä]heste[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "n[aä]he",
            replacement: "näch",
        }],
    ),
    (
        "gesundlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "heitlich",
        }],
    ),
    (
        "eckel(e|t(en?)?|st)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^eck",
            replacement: "ek",
        }],
    ),
    (
        "unhervorgesehen(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "hervor",
            replacement: "vorher",
        }],
    ),
    (
        "entt?euscht(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "entt?eusch",
            replacement: "enttäusch",
        }],
    ),
    (
        "Phählen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^Ph",
            replacement: "Pf",
        }],
    ),
    (
        "Kattermesser[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Ka",
            replacement: "Cu",
        }],
    ),
    (
        "gehe?rr?t(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "he?rr?",
            replacement: "ehr",
        }],
    ),
    (
        "gehrter?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^ge",
            replacement: "gee",
        }],
    ),
    (
        "[nN]amenhaft(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "amen",
            replacement: "am",
        }],
    ),
    (
        "hom(o?e|ö)ophatisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "hom(o?e|ö)ophat",
            replacement: "homöopath",
        }],
    ),
    (
        "Geschwindlichkeit(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "ig",
        }],
    ),
    (
        "Jänners?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Jänner",
            replacement: "Januar",
        }],
    ),
    (
        "[äÄ]hlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "nlich",
        }],
    ),
    (
        "entf[ai]ngen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ent",
            replacement: "emp",
        }],
    ),
    (
        "entf[äi]ngs?t",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ent",
            replacement: "emp",
        }],
    ),
    (
        "[Bb]ehilfreich(e[rnms]?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "reich",
            replacement: "lich",
        }],
    ),
    (
        "[Bb]zgl",
        &[SuggestExpr::ReplaceFirst {
            pattern: "zgl",
            replacement: "zgl.",
        }],
    ),
    (
        "kaltnass(e[rnms]?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "kaltnass",
            replacement: "nasskalt",
        }],
    ),
    (
        "Kaltnass(e[rnms]?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Kaltnass",
            replacement: "Nasskalt",
        }],
    ),
    ("check", &[SuggestExpr::Lit("checke")]),
    ("Rückrad", &[SuggestExpr::Lit("Rückgrat")]),
    ("ala", &[SuggestExpr::Lit("à la")]),
    ("Ala", &[SuggestExpr::Lit("À la")]),
    ("Reinfolge", &[SuggestExpr::Lit("Reihenfolge")]),
    ("Schloß", &[SuggestExpr::Lit("Schloss")]),
    ("Investion", &[SuggestExpr::Lit("Investition")]),
    ("Beleidung", &[SuggestExpr::Lit("Beleidigung")]),
    ("Bole", &[SuggestExpr::Lit("Bowle")]),
    ("letzens", &[SuggestExpr::Lit("letztens")]),
    (
        "Pakur",
        &[SuggestExpr::Lit("Parcours"), SuggestExpr::Lit("Parkuhr")],
    ),
    ("Dez", &[SuggestExpr::Lit("Dez."), SuggestExpr::Lit("Der")]),
    ("Jun", &[SuggestExpr::Lit("Jun.")]),
    ("Sept", &[SuggestExpr::Lit("Sept.")]),
    ("Aug", &[SuggestExpr::Lit("Aug."), SuggestExpr::Lit("Auge")]),
    (
        "Erstsemesterin",
        &[
            SuggestExpr::Lit("Erstsemester"),
            SuggestExpr::Lit("Erstsemesters"),
            SuggestExpr::Lit("Erstsemesterstudentin"),
        ],
    ),
    (
        "Erstsemesterinnen",
        &[
            SuggestExpr::Lit("Erstsemesterstudentinnen"),
            SuggestExpr::Lit("Erstsemester"),
            SuggestExpr::Lit("Erstsemestern"),
        ],
    ),
    (
        "kreativlos(e[nmrs]?)?",
        &[
            SuggestExpr::ReplaceOnce {
                from: "kreativ",
                to: "fantasie",
            },
            SuggestExpr::ReplaceOnce {
                from: "kreativ",
                to: "einfalls",
            },
            SuggestExpr::ReplaceOnce {
                from: "kreativlos",
                to: "unkreativ",
            },
            SuggestExpr::ReplaceOnce {
                from: "kreativlos",
                to: "uninspiriert",
            },
        ],
    ),
    ("Kreativlosigkeit", &[SuggestExpr::Lit("Unkreativität")]),
    ("hinund?her", &[SuggestExpr::Lit("hin und her")]),
    ("[lL]ymph?trie?nasche", &[SuggestExpr::Lit("Lymphdrainage")]),
    ("Interdeterminismus", &[SuggestExpr::Lit("Indeterminismus")]),
    ("elektrität", &[SuggestExpr::Lit("Elektrizität")]),
    ("ausgeboten", &[SuggestExpr::Lit("ausgebootet")]),
    ("nocheinmall", &[SuggestExpr::Lit("noch einmal")]),
    ("aüßerst", &[SuggestExpr::Lit("äußerst")]),
    ("Grrösse", &[SuggestExpr::Lit("Größe")]),
    ("misverständniss", &[SuggestExpr::Lit("Missverständnis")]),
    ("warheit", &[SuggestExpr::Lit("Wahrheit")]),
    ("[pP]okemon", &[SuggestExpr::Lit("Pokémon")]),
    ("kreigt", &[SuggestExpr::Lit("kriegt")]),
    ("Fritöse", &[SuggestExpr::Lit("Fritteuse")]),
    ("unerkennlich", &[SuggestExpr::Lit("unkenntlich")]),
    ("rückg[äe]nglich", &[SuggestExpr::Lit("rückgängig")]),
    ("em?men[sz]", &[SuggestExpr::Lit("immens")]),
    ("verhing", &[SuggestExpr::Lit("verhängte")]),
    ("verhingen", &[SuggestExpr::Lit("verhängten")]),
    ("fangte", &[SuggestExpr::Lit("fing")]),
    ("fangten", &[SuggestExpr::Lit("fingen")]),
    ("schlie[sß]te", &[SuggestExpr::Lit("schloss")]),
    ("schlie[sß]ten", &[SuggestExpr::Lit("schlossen")]),
    ("past", &[SuggestExpr::Lit("passt")]),
    ("eingetragt", &[SuggestExpr::Lit("eingetragen")]),
    ("getrunkt", &[SuggestExpr::Lit("getrunken")]),
    ("veräht", &[SuggestExpr::Lit("verrät")]),
    ("helfte", &[SuggestExpr::Lit("half")]),
    ("helften", &[SuggestExpr::Lit("halfen")]),
    ("lad", &[SuggestExpr::Lit("lade")]),
    ("befehlte", &[SuggestExpr::Lit("befahl")]),
    ("befehlten", &[SuggestExpr::Lit("befahlen")]),
    ("angelügt", &[SuggestExpr::Lit("angelogen")]),
    ("Bitet", &[SuggestExpr::Lit("Bittet")]),
    ("dagen", &[SuggestExpr::Lit("sagen")]),
    ("ändenr", &[SuggestExpr::Lit("ändern")]),
    ("übetragen", &[SuggestExpr::Lit("übertragen")]),
    ("Ihrn", &[SuggestExpr::Lit("Ihren")]),
    ("Emal", &[SuggestExpr::Lit("E-Mail")]),
    ("Emai", &[SuggestExpr::Lit("E-Mail")]),
    ("schuen", &[SuggestExpr::Lit("schauen")]),
    ("Hasue", &[SuggestExpr::Lit("Haus")]),
    ("leier", &[SuggestExpr::Lit("leider")]),
    ("Meschen", &[SuggestExpr::Lit("Menschen")]),
    ("unsen", &[SuggestExpr::Lit("unseren")]),
    ("biiten", &[SuggestExpr::Lit("bitten")]),
    ("geläscht", &[SuggestExpr::Lit("gelöscht")]),
    ("Kundein", &[SuggestExpr::Lit("Kundin")]),
    ("amch", &[SuggestExpr::Lit("mach")]),
    ("amche", &[SuggestExpr::Lit("mache")]),
    ("forfahren", &[SuggestExpr::Lit("fortfahren")]),
    ("verate", &[SuggestExpr::Lit("verrate")]),
    ("interen", &[SuggestExpr::Lit("interne")]),
    ("Budge", &[SuggestExpr::Lit("Budget")]),
    ("weiso", &[SuggestExpr::Lit("wieso")]),
    ("Parter", &[SuggestExpr::Lit("Partner")]),
    ("wiet", &[SuggestExpr::Lit("weit"), SuggestExpr::Lit("wie")]),
    (
        "beid",
        &[
            SuggestExpr::Lit("beide"),
            SuggestExpr::Lit("seid"),
            SuggestExpr::Lit("beim"),
            SuggestExpr::Lit("bei"),
        ],
    ),
    (
        "Theam",
        &[SuggestExpr::Lit("Thema"), SuggestExpr::Lit("Team")],
    ),
    (
        "ind",
        &[
            SuggestExpr::Lit("und"),
            SuggestExpr::Lit("ins"),
            SuggestExpr::Lit("in"),
            SuggestExpr::Lit("sind"),
        ],
    ),
    ("us", &[SuggestExpr::Lit("US"), SuggestExpr::Lit("aus")]),
    (
        "soch",
        &[
            SuggestExpr::Lit("doch"),
            SuggestExpr::Lit("sich"),
            SuggestExpr::Lit("noch"),
        ],
    ),
    (
        "Abe",
        &[
            SuggestExpr::Lit("Aber"),
            SuggestExpr::Lit("Ab"),
            SuggestExpr::Lit("ABE"),
            SuggestExpr::Lit("Aue"),
        ],
    ),
    ("lügte", &[SuggestExpr::Lit("log")]),
    ("lügten", &[SuggestExpr::Lit("logen")]),
    ("bratete", &[SuggestExpr::Lit("briet")]),
    ("brateten", &[SuggestExpr::Lit("brieten")]),
    ("gefahl", &[SuggestExpr::Lit("gefiel")]),
    ("Komplexibilität", &[SuggestExpr::Lit("Komplexität")]),
    ("abbonement", &[SuggestExpr::Lit("Abonnement")]),
    ("zugegebenerweise", &[SuggestExpr::Lit("zugegebenermaßen")]),
    ("perse", &[SuggestExpr::Lit("per se")]),
    ("Schwitch", &[SuggestExpr::Lit("Switch")]),
    (
        "[aA]nwesenzeiten",
        &[SuggestExpr::Lit("Anwesenheitszeiten")],
    ),
    ("[gG]eizigkeit", &[SuggestExpr::Lit("Geiz")]),
    ("[fF]leißigkeit", &[SuggestExpr::Lit("Fleiß")]),
    ("[bB]equemheit", &[SuggestExpr::Lit("Bequemlichkeit")]),
    (
        "[mM]issionarie?sie?rung",
        &[SuggestExpr::Lit("Missionierung")],
    ),
    ("[sS]chee?selonge?", &[SuggestExpr::Lit("Chaiselongue")]),
    ("Re[kc]amiere", &[SuggestExpr::Lit("Récamière")]),
    ("Singel", &[SuggestExpr::Lit("Single")]),
    ("legen[td]lich", &[SuggestExpr::Lit("lediglich")]),
    ("ein[ua]ndhalb", &[SuggestExpr::Lit("eineinhalb")]),
    (
        "[mM]illion(en)?mal",
        &[SuggestExpr::UpperFirst(&[SuggestExpr::ReplaceOnce {
            from: "mal",
            to: " Mal",
        }])],
    ),
    ("Mysql", &[SuggestExpr::Lit("MySQL")]),
    ("MWST", &[SuggestExpr::Lit("MwSt")]),
    ("Opelarena", &[SuggestExpr::Lit("Opel Arena")]),
    ("Toll-Collect", &[SuggestExpr::Lit("Toll Collect")]),
    ("[pP][qQ]-Formel", &[SuggestExpr::Lit("p-q-Formel")]),
    ("desweitere?m", &[SuggestExpr::Lit("des Weiteren")]),
    ("handzuhaben", &[SuggestExpr::Lit("zu handhaben")]),
    ("nachvollzuziehe?n", &[SuggestExpr::Lit("nachzuvollziehen")]),
    ("Porto?folien", &[SuggestExpr::Lit("Portfolios")]),
    (
        "[sS]chwie?ri?chkeiten",
        &[SuggestExpr::Lit("Schwierigkeiten")],
    ),
    (
        "[üÜ]bergrifflichkeiten",
        &[SuggestExpr::Lit("Übergriffigkeiten")],
    ),
    ("[aA]r?th?rie?th?is", &[SuggestExpr::Lit("Arthritis")]),
    ("zugesand", &[SuggestExpr::Lit("zugesandt")]),
    ("weibt", &[SuggestExpr::Lit("weißt")]),
    ("fress", &[SuggestExpr::Lit("friss")]),
    ("Mamma", &[SuggestExpr::Lit("Mama")]),
    ("Präse", &[SuggestExpr::Lit("Präsentation")]),
    ("Präsen", &[SuggestExpr::Lit("Präsentationen")]),
    ("Orga", &[SuggestExpr::Lit("Organisation")]),
    ("Orgas", &[SuggestExpr::Lit("Organisationen")]),
    ("Reorga", &[SuggestExpr::Lit("Reorganisation")]),
    ("Reorgas", &[SuggestExpr::Lit("Reorganisationen")]),
    (
        "instande?zusetzen",
        &[SuggestExpr::Lit("instand zu setzen")],
    ),
    ("Lia(si|is)onen", &[SuggestExpr::Lit("Liaisons")]),
    (
        "[cC]asemana?ge?ment",
        &[SuggestExpr::Lit("Case Management")],
    ),
    ("[aA]nn?[ou]ll?ie?rung", &[SuggestExpr::Lit("Annullierung")]),
    ("[sS]charm", &[SuggestExpr::Lit("Charme")]),
    (
        "[zZ]auberlich(e[mnrs]?)?",
        &[
            SuggestExpr::ReplaceOnce {
                from: "lich",
                to: "isch",
            },
            SuggestExpr::ReplaceOnce {
                from: "lich",
                to: "haft",
            },
        ],
    ),
    ("[eE]rledung", &[SuggestExpr::Lit("Erledigung")]),
    ("erledigigung", &[SuggestExpr::Lit("Erledigung")]),
    ("woltest", &[SuggestExpr::Lit("wolltest")]),
    ("[iI]ntranzparentheit", &[SuggestExpr::Lit("Intransparenz")]),
    ("dunkellilane[mnrs]?", &[SuggestExpr::Lit("dunkellila")]),
    ("helllilane[mnrs]?", &[SuggestExpr::Lit("helllila")]),
    ("Behauptungsthese", &[SuggestExpr::Lit("Behauptung")]),
    ("genzut", &[SuggestExpr::Lit("genutzt")]),
    ("[eEäÄ]klerung", &[SuggestExpr::Lit("Erklärung")]),
    ("[wW]eh?wechen", &[SuggestExpr::Lit("Wehwehchen")]),
    ("nocheinmals", &[SuggestExpr::Lit("noch einmal")]),
    (
        "unverantwortungs?los(e[mnrs]?)?",
        &[
            SuggestExpr::ReplaceFirst {
                pattern: "unverantwortungs?",
                replacement: "verantwortungs",
            },
            SuggestExpr::ReplaceFirst {
                pattern: "ungs?los",
                replacement: "lich",
            },
        ],
    ),
    (
        "[eE]rhaltbar(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "haltbar",
            replacement: "hältlich",
        }],
    ),
    (
        "[aA]ufkeinenfall?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "keinenfall?",
            replacement: " keinen Fall",
        }],
    ),
    (
        "[Dd]rumrum",
        &[SuggestExpr::ReplaceFirst {
            pattern: "rum$",
            replacement: "herum",
        }],
    ),
    (
        "([uU]n)?proff?esionn?ell?(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "proff?esionn?ell?",
            replacement: "professionell",
        }],
    ),
    (
        "[kK]inderlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "inder",
            replacement: "ind",
        }],
    ),
    (
        "[wW]iedersprichs?t",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ieder",
            replacement: "ider",
        }],
    ),
    (
        "[wW]hite-?[Ll]abels",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[wW]hite-?[Ll]abel",
            replacement: "White Label",
        }],
    ),
    (
        "[wW]iederstand",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ieder",
            replacement: "ider",
        }],
    ),
    (
        "[kK]önntes",
        &[SuggestExpr::ReplaceFirst {
            pattern: "es$",
            replacement: "est",
        }],
    ),
    (
        "[aA]ssess?oare?s?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[aA]ssess?oare?",
            replacement: "Accessoire",
        }],
    ),
    (
        "indifiziert(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ind",
            replacement: "ident",
        }],
    ),
    (
        "dreite[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "dreit",
            replacement: "dritt",
        }],
    ),
    (
        "verblüte[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "blü",
            replacement: "blüh",
        }],
    ),
    (
        "Einzigste[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "zigst",
            replacement: "zig",
        }],
    ),
    (
        "Invests?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Invest",
            replacement: "Investment",
        }],
    ),
    (
        "(aller)?einzie?gste[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "(aller)?einzie?gst",
            replacement: "einzig",
        }],
    ),
    (
        "[iI]nterkurell(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ku",
            replacement: "kultu",
        }],
    ),
    (
        "[iI]ntersannt(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "sannt",
            replacement: "essant",
        }],
    ),
    (
        "ubera(g|sch)end(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "uber",
            replacement: "überr",
        }],
    ),
    (
        "[Hh]ello",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ello",
            replacement: "allo",
        }],
    ),
    (
        "[Gg]etagged",
        &[SuggestExpr::ReplaceFirst {
            pattern: "gged",
            replacement: "ggt",
        }],
    ),
    (
        "[wW]olt$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lt",
            replacement: "llt",
        }],
    ),
    (
        "[zZ]uende",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ue",
            replacement: "u E",
        }],
    ),
    (
        "[iI]nbälde",
        &[SuggestExpr::ReplaceFirst {
            pattern: "nb",
            replacement: "n B",
        }],
    ),
    (
        "[lL]etztenendes",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ene",
            replacement: "en E",
        }],
    ),
    (
        "[nN]achwievor",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wievor",
            replacement: " wie vor",
        }],
    ),
    (
        "[zZ]umbeispiel",
        &[SuggestExpr::ReplaceFirst {
            pattern: "beispiel",
            replacement: " Beispiel",
        }],
    ),
    (
        "[gG]ottseidank",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[gG]ottseidank",
            replacement: "Gott sei Dank",
        }],
    ),
    (
        "[gG]rundauf",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[gG]rundauf",
            replacement: "Grund auf",
        }],
    ),
    (
        "[aA]nsichtnach",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[aA]nsicht",
            replacement: "Ansicht ",
        }],
    ),
    (
        "[uU]n[sz]war",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[sz]war",
            replacement: "d zwar",
        }],
    ),
    (
        "[wW]aschte(s?t)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "aschte",
            replacement: "usch",
        }],
    ),
    (
        "[wW]aschten",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ascht",
            replacement: "usch",
        }],
    ),
    (
        "Probiren?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ir",
            replacement: "ier",
        }],
    ),
    (
        "[gG]esetztreu(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "tz",
            replacement: "tzes",
        }],
    ),
    (
        "[wW]ikich(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "k",
            replacement: "rkl",
        }],
    ),
    (
        "[uU]naufbesichtigt(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "aufbe",
            replacement: "beauf",
        }],
    ),
    (
        "[nN]utzvoll(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "utzvoll",
            replacement: "ützlich",
        }],
    ),
    (
        "Lezte[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Lez",
            replacement: "Letz",
        }],
    ),
    (
        "Letze[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Letz",
            replacement: "Letzt",
        }],
    ),
    (
        "[nN]i[vw]os?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[nN]i[vw]o",
            replacement: "Niveau",
        }],
    ),
    (
        "[dD]illetant(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[dD]ille",
            replacement: "Dilet",
        }],
    ),
    (
        "Frauenhofer-(Institut|Gesellschaft)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Frauen",
            replacement: "Fraun",
        }],
    ),
    (
        "Add-?Ons?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Add-?On",
            replacement: "Add-on",
        }],
    ),
    (
        "Addons?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "on",
            replacement: "-on",
        }],
    ),
    (
        "Internetkaffees?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "kaffee",
            replacement: "café",
        }],
    ),
    (
        "[gG]ehorsamkeitsverweigerung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[gG]ehorsamkeit",
            replacement: "Gehorsam",
        }],
    ),
    (
        "[wW]ochende[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[wW]ochend",
            replacement: "Wochenend",
        }],
    ),
    (
        "[kK]ongratulier(en?|t(en?)?|st)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[kK]on",
            replacement: "",
        }],
    ),
    (
        "[wWkKdD]an$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "n$",
            replacement: "nn",
        }],
    ),
    (
        "geh?neh?m[ie]gung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "geh?neh?m[ie]gung",
            replacement: "Genehmigung",
        }],
    ),
    (
        "Korrigierung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "igierung",
            replacement: "ektur",
        }],
    ),
    (
        "[kK]orregierung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[kK]orregierung",
            replacement: "Korrektur",
        }],
    ),
    (
        "[kK]orrie?girung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[kK]orrie?girung",
            replacement: "Korrektur",
        }],
    ),
    (
        "[nN]ocheimal",
        &[SuggestExpr::ReplaceFirst {
            pattern: "eimal",
            replacement: " einmal",
        }],
    ),
    (
        "[aA]benzu",
        &[SuggestExpr::ReplaceFirst {
            pattern: "enzu",
            replacement: " und zu",
        }],
    ),
    (
        "[kK]onflikation(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[kK]onfli",
            replacement: "Kompli",
        }],
    ),
    (
        "[mM]itanader",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ana",
            replacement: "einan",
        }],
    ),
    (
        "[mM]itenand",
        &[SuggestExpr::ReplaceFirst {
            pattern: "enand",
            replacement: "einander",
        }],
    ),
    (
        "Gelangenheitsbestätigung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "heit",
            replacement: "",
        }],
    ),
    (
        "[jJ]edwillige[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "willig",
            replacement: "wed",
        }],
    ),
    (
        "[qQ]ualitäts?bewußt(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ts?bewußt",
            replacement: "tsbewusst",
        }],
    ),
    (
        "[vV]oraussichtig(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "sichtig",
            replacement: "sichtlich",
        }],
    ),
    (
        "[gG]leichrechtig(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "rechtig",
            replacement: "berechtigt",
        }],
    ),
    (
        "[uU]nnützlich(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "nützlich",
            replacement: "nütz",
        }],
    ),
    (
        "[uU]nzerbrechbar(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "bar",
            replacement: "lich",
        }],
    ),
    (
        "kolegen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ko",
            replacement: "Kol",
        }],
    ),
    (
        "tableten?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "tablet",
            replacement: "Tablett",
        }],
    ),
    (
        "verswinde(n|s?t)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^vers",
            replacement: "versch",
        }],
    ),
    (
        "unverantwortungsvoll(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "unverantwortungsvoll",
            replacement: "verantwortungslos",
        }],
    ),
    (
        "[gG]erechtlichkeit",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[gG]erechtlich",
            replacement: "Gerechtig",
        }],
    ),
    (
        "[zZ]uverlässlichkeit",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "ig",
        }],
    ),
    (
        "[uU]nverzeilig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "zeilig",
            replacement: "zeihlich",
        }],
    ),
    (
        "[zZ]uk(ue?|ü)nftlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "uk(ue?|ü)nftlich",
            replacement: "ukünftig",
        }],
    ),
    (
        "[rR]eligiösisch(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "isch",
            replacement: "",
        }],
    ),
    (
        "[fF]olklorisch(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "isch",
            replacement: "istisch",
        }],
    ),
    (
        "[eE]infühlsvoll(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "voll",
            replacement: "am",
        }],
    ),
    (
        "Unstimmlichkeit(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "ig",
        }],
    ),
    (
        "Strebergartens?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Stre",
            replacement: "Schre",
        }],
    ),
    (
        "[hH]ähern(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ähern",
            replacement: "ären",
        }],
    ),
    (
        "todesbedroh(end|lich)(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "todes",
            replacement: "lebens",
        }],
    ),
    (
        "^[uU]nabsichtig(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ig",
            replacement: "lich",
        }],
    ),
    (
        "[aA]ntisemitistisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "tist",
            replacement: "t",
        }],
    ),
    (
        "[uU]nvorsehbar(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "vor",
            replacement: "vorher",
        }],
    ),
    (
        "([eE]r|[bB]e|unter|[aA]uf)?hälst",
        &[SuggestExpr::ReplaceFirst {
            pattern: "hälst",
            replacement: "hältst",
        }],
    ),
    (
        "[wW]ohlfühlseins?",
        &[
            SuggestExpr::Lit("Wellness"),
            SuggestExpr::ReplaceFirst {
                pattern: "[wW]ohlfühlsein",
                replacement: "Wohlbefinden",
            },
            SuggestExpr::ReplaceFirst {
                pattern: "[wW]ohlfühlsein",
                replacement: "Wohlfühlen",
            },
        ],
    ),
    (
        "[sS]chmett?e?rling(s|en?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[sS]chmett?e?rling",
            replacement: "Schmetterling",
        }],
    ),
    (
        "^[eE]inlamie?nie?r(st|en?|(t(e[nmrs]?)?))?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^einlamie?nie?r",
            replacement: "laminier",
        }],
    ),
    (
        "[bB]ravurös(e[nrms]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "vur",
            replacement: "vour",
        }],
    ),
    (
        "[aA]ss?ecoires?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[aA]ss?ec",
            replacement: "Access",
        }],
    ),
    (
        "[aA]ufwechse?lungsreich(er|st)?(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ufwechse?lung",
            replacement: "bwechslung",
        }],
    ),
    (
        "[iI]nordnung",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ordnung",
            replacement: " Ordnung",
        }],
    ),
    (
        "[iI]mmoment",
        &[SuggestExpr::ReplaceFirst {
            pattern: "moment",
            replacement: " Moment",
        }],
    ),
    (
        "[hH]euteabend",
        &[SuggestExpr::ReplaceFirst {
            pattern: "abend",
            replacement: " Abend",
        }],
    ),
    (
        "[wW]ienerschnitzel[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[wW]ieners",
            replacement: "Wiener S",
        }],
    ),
    (
        "[sS]chwarzwälderkirschtorten?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[sS]chwarzwälderk",
            replacement: "Schwarzwälder K",
        }],
    ),
    (
        "[kK]oxial(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "x",
            replacement: "ax",
        }],
    ),
    (
        "([üÜ]ber|[uU]unter)?[dD]urs?chnitt?lich(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "s?chnitt?",
            replacement: "chschnitt",
        }],
    ),
    (
        "[dD]urs?chnitts?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "s?chnitt",
            replacement: "chschnitt",
        }],
    ),
    (
        "[sS]triktlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "",
        }],
    ),
    (
        "[hH]öchstwahrlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wahr",
            replacement: "wahrschein",
        }],
    ),
    (
        "[oO]rganisativ(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "tiv",
            replacement: "torisch",
        }],
    ),
    (
        "[kK]ontaktfreundlich(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ndlich",
            replacement: "dig",
        }],
    ),
    (
        "Helfer?s-Helfer[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Helfer?s-H",
            replacement: "Helfersh",
        }],
    ),
    (
        "[iI]ntell?igentsbestien?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[iI]ntell?igents",
            replacement: "Intelligenz",
        }],
    ),
    (
        "[aA]vantgardisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "gard",
            replacement: "gardist",
        }],
    ),
    (
        "[gG]ewohnheitsbedürftig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wohnheit",
            replacement: "wöhnung",
        }],
    ),
    (
        "[eE]infühlungsvoll(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "fühlungsvoll",
            replacement: "fühlsam",
        }],
    ),
    (
        "[vV]erwant(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "want",
            replacement: "wandt",
        }],
    ),
    (
        "[bB]eanstandigung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ig",
            replacement: "",
        }],
    ),
    (
        "[eE]inba(hn|nd)frei(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ba(hn|nd)",
            replacement: "wand",
        }],
    ),
    (
        "[äÄaAeE]rtzten?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[äÄaAeE]rt",
            replacement: "Är",
        }],
    ),
    (
        "pdf-Datei(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "pdf",
            replacement: "PDF",
        }],
    ),
    (
        "rumänern?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "rumäner",
            replacement: "Rumäne",
        }],
    ),
    (
        "[cCKk]o?usengs?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[cCKk]o?useng",
            replacement: "Cousin",
        }],
    ),
    (
        "Influenzer(in(nen)?|[ns])?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "zer",
            replacement: "cer",
        }],
    ),
    (
        "[vV]ersantdienstleister[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[vV]ersant",
            replacement: "Versand",
        }],
    ),
    (
        "[pP]atrolier(s?t|t?en?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "atrolier",
            replacement: "atrouillier",
        }],
    ),
    (
        "[pP]ropagandiert(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "and",
            replacement: "",
        }],
    ),
    (
        "[pP]ropagandier(en|st)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "and",
            replacement: "",
        }],
    ),
    (
        "[kK]app?erzität(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^[kK]app?er",
            replacement: "Kapa",
        }],
    ),
    (
        "känzel(n|s?t)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "känzel",
            replacement: "cancel",
        }],
    ),
    ("gekänzelt", &[SuggestExpr::Lit("gecancelt")]),
    (
        "[üÜ]berstreitung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[üÜ]berst",
            replacement: "Übersch",
        }],
    ),
    (
        "anschliess?lich(e(mnrs)?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "anschliess?lich",
            replacement: "anschließend",
        }],
    ),
    (
        "[rR]ethorisch(e(mnrs)?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "eth",
            replacement: "het",
        }],
    ),
    (
        "änlich(e(mnrs)?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "än",
            replacement: "ähn",
        }],
    ),
    (
        "spätmöglichste(mnrs)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "spätmöglichst",
            replacement: "spätestmöglich",
        }],
    ),
    ("mogen", &[SuggestExpr::Lit("morgen")]),
    ("[fF]uss?ill?ien", &[SuggestExpr::Lit("Fossilien")]),
    ("übrings", &[SuggestExpr::Lit("übrigens")]),
    ("[rR]evü", &[SuggestExpr::Lit("Revue")]),
    ("eingänglich", &[SuggestExpr::Lit("eingangs")]),
    ("geerthe", &[SuggestExpr::Lit("geehrte")]),
    ("interrese", &[SuggestExpr::Lit("Interesse")]),
    ("[rR]eschärschen", &[SuggestExpr::Lit("Recherchen")]),
    ("[rR]eschärsche", &[SuggestExpr::Lit("Recherche")]),
    ("ic", &[SuggestExpr::Lit("ich")]),
    ("w[eä]hret", &[SuggestExpr::Lit("wäret")]),
    ("mahte", &[SuggestExpr::Lit("Mathe")]),
    ("letzdenendes", &[SuggestExpr::Lit("letzten Endes")]),
    ("aufgesteht", &[SuggestExpr::Lit("aufgestanden")]),
    ("ganichts", &[SuggestExpr::Lit("gar nichts")]),
    ("gesich", &[SuggestExpr::Lit("Gesicht")]),
    ("glass", &[SuggestExpr::Lit("Glas")]),
    ("muter", &[SuggestExpr::Lit("Mutter")]),
    ("[pP]appa", &[SuggestExpr::Lit("Papa")]),
    ("dier", &[SuggestExpr::Lit("dir")]),
    ("Referenz-Nr", &[SuggestExpr::Lit("Referenz-Nr.")]),
    ("Matrikelnr.", &[SuggestExpr::Lit("Matrikel-Nr.")]),
    (
        "Rekrutings?prozess",
        &[SuggestExpr::Lit("Recruitingprozess")],
    ),
    ("sumarum", &[SuggestExpr::Lit("summarum")]),
    ("schein", &[SuggestExpr::Lit("scheine")]),
    (
        "Innzahlung",
        &[
            SuggestExpr::Lit("In Zahlung"),
            SuggestExpr::Lit("in Zahlung"),
        ],
    ),
    (
        "änderen",
        &[SuggestExpr::Lit("ändern"), SuggestExpr::Lit("anderen")],
    ),
    (
        "wanderen",
        &[SuggestExpr::Lit("wandern"), SuggestExpr::Lit("Wanderern")],
    ),
    (
        "Dutzen",
        &[SuggestExpr::Lit("Duzen"), SuggestExpr::Lit("Dutzend")],
    ),
    (
        "patien",
        &[SuggestExpr::Lit("Partien"), SuggestExpr::Lit("Patient")],
    ),
    (
        "Teammitgliederinnen",
        &[
            SuggestExpr::Lit("Teammitgliedern"),
            SuggestExpr::Lit("Teammitglieder"),
        ],
    ),
    (
        "beidige[mnrs]?",
        &[
            SuggestExpr::ReplaceOnce { from: "ig", to: "" },
            SuggestExpr::ReplaceOnce {
                from: "beid",
                to: "beiderseit",
            },
            SuggestExpr::Lit("beeidigen"),
        ],
    ),
    (
        "Wissbegierigkeit",
        &[
            SuggestExpr::Lit("Wissbegier"),
            SuggestExpr::Lit("Wissbegierde"),
        ],
    ),
    ("Nabend", &[SuggestExpr::Lit("'n Abend")]),
    ("gie?bts", &[SuggestExpr::Lit("gibt's")]),
    ("vs", &[SuggestExpr::Lit("vs.")]),
    ("[kK]affeeteria", &[SuggestExpr::Lit("Cafeteria")]),
    ("[kK]affeeterien", &[SuggestExpr::Lit("Cafeterien")]),
    ("berücksicht", &[SuggestExpr::Lit("berücksichtigt")]),
    ("must", &[SuggestExpr::Lit("musst")]),
    ("kaffe", &[SuggestExpr::Lit("Kaffee")]),
    ("zetel", &[SuggestExpr::Lit("Zettel")]),
    ("wie?daholung", &[SuggestExpr::Lit("Wiederholung")]),
    ("vie?d(er|a)sehen", &[SuggestExpr::Lit("wiedersehen")]),
    ("pr[eä]ventiert", &[SuggestExpr::Lit("verhindert")]),
    ("pr[eä]ventieren", &[SuggestExpr::Lit("verhindern")]),
    ("zur?verfügung", &[SuggestExpr::Lit("zur Verfügung")]),
    ("Verwahrlosigkeit", &[SuggestExpr::Lit("Verwahrlosung")]),
    ("[oO]r?ganisazion", &[SuggestExpr::Lit("Organisation")]),
    ("[oO]rganisative", &[SuggestExpr::Lit("Organisation")]),
    ("Emall?iearbeit", &[SuggestExpr::Lit("Emaillearbeit")]),
    ("[aA]petitt", &[SuggestExpr::Lit("Appetit")]),
    ("bezuggenommen", &[SuggestExpr::Lit("Bezug genommen")]),
    ("mägt", &[SuggestExpr::Lit("mögt")]),
    ("frug", &[SuggestExpr::Lit("fragte")]),
    ("gesäht", &[SuggestExpr::Lit("gesät")]),
    ("verennt", &[SuggestExpr::Lit("verrennt")]),
    ("überrant", &[SuggestExpr::Lit("überrannt")]),
    ("Gallop", &[SuggestExpr::Lit("Galopp")]),
    ("Stop", &[SuggestExpr::Lit("Stopp")]),
    ("Schertz", &[SuggestExpr::Lit("Scherz")]),
    ("geschied", &[SuggestExpr::Lit("geschieht")]),
    ("Aku", &[SuggestExpr::Lit("Akku")]),
    ("Migrationspackt", &[SuggestExpr::Lit("Migrationspakt")]),
    ("[zZ]ulaufror", &[SuggestExpr::Lit("Zulaufrohr")]),
    (
        "[gG]ebrauchss?puhren",
        &[SuggestExpr::Lit("Gebrauchsspuren")],
    ),
    ("[pP]reisnachlassung", &[SuggestExpr::Lit("Preisnachlass")]),
    ("[mM]edikamentation", &[SuggestExpr::Lit("Medikation")]),
    ("[nN][ei]gliche", &[SuggestExpr::Lit("Negligé")]),
    (
        "palletten?",
        &[
            SuggestExpr::ReplaceOnce {
                from: "pall",
                to: "Pal",
            },
            SuggestExpr::ReplaceOnce {
                from: "pa",
                to: "Pai",
            },
        ],
    ),
    ("[pP]allete", &[SuggestExpr::Lit("Palette")]),
    (
        "Geräuch",
        &[SuggestExpr::Lit("Geräusch"), SuggestExpr::Lit("Gesträuch")],
    ),
    (
        "Eon",
        &[
            SuggestExpr::Lit("Ein"),
            SuggestExpr::Lit("E.ON"),
            SuggestExpr::Lit("Von"),
        ],
    ),
    ("[sS]chull?igung", &[SuggestExpr::Lit("Entschuldigung")]),
    ("Geerte", &[SuggestExpr::Lit("geehrte")]),
    ("versichen", &[SuggestExpr::Lit("versichern")]),
    ("hobb?ies", &[SuggestExpr::Lit("Hobbys")]),
    ("Begierigkeiten", &[SuggestExpr::Lit("Begehrlichkeiten")]),
    ("selblosigkeit", &[SuggestExpr::Lit("Selbstlosigkeit")]),
    ("gestyled", &[SuggestExpr::Lit("gestylt")]),
    ("umstimigkeiten", &[SuggestExpr::Lit("Unstimmigkeiten")]),
    (
        "unann?äh?ml?ichkeiten",
        &[SuggestExpr::Lit("Unannehmlichkeiten")],
    ),
    (
        "unn?ann?ehmichkeiten",
        &[SuggestExpr::Lit("Unannehmlichkeiten")],
    ),
    ("übertr[äa]gte", &[SuggestExpr::Lit("übertrug")]),
    ("übertr[äa]gten", &[SuggestExpr::Lit("übertrugen")]),
    ("NodeJS", &[SuggestExpr::Lit("Node.js")]),
    ("Express", &[SuggestExpr::Lit("Express.js")]),
    ("erlas", &[SuggestExpr::Lit("Erlass")]),
    ("schlagte", &[SuggestExpr::Lit("schlug")]),
    ("schlagten", &[SuggestExpr::Lit("schlugen")]),
    ("überwissen", &[SuggestExpr::Lit("überwiesen")]),
    ("einpar", &[SuggestExpr::Lit("ein paar")]),
    ("sreiben", &[SuggestExpr::Lit("schreiben")]),
    ("routiene", &[SuggestExpr::Lit("Routine")]),
    ("ect", &[SuggestExpr::Lit("etc")]),
    ("giept", &[SuggestExpr::Lit("gibt")]),
    ("Pann?acott?a", &[SuggestExpr::Lit("Panna cotta")]),
    (
        "Fußgängerunterwegs?",
        &[SuggestExpr::Lit("Fußgängerunterführung")],
    ),
    ("angeschriehen", &[SuggestExpr::Lit("angeschrien")]),
    ("vieviel", &[SuggestExpr::Lit("wie viel")]),
    ("entäscht", &[SuggestExpr::Lit("enttäuscht")]),
    ("Rämchen", &[SuggestExpr::Lit("Rähmchen")]),
    ("Seminarbeit", &[SuggestExpr::Lit("Seminararbeit")]),
    ("Seminarbeiten", &[SuggestExpr::Lit("Seminararbeiten")]),
    ("[eE]ngangment", &[SuggestExpr::Lit("Engagement")]),
    ("[lL]eichtah?tleh?t", &[SuggestExpr::Lit("Leichtathlet")]),
    ("[pP]fane", &[SuggestExpr::Lit("Pfanne")]),
    ("[iI]ngini?eue?r", &[SuggestExpr::Lit("Ingenieur")]),
    ("[aA]nligen", &[SuggestExpr::Lit("Anliegen")]),
    (
        "Tankungen",
        &[
            SuggestExpr::Lit("Betankungen"),
            SuggestExpr::Lit("Tankvorgänge"),
        ],
    ),
    (
        "Ärcker",
        &[SuggestExpr::Lit("Erker"), SuggestExpr::Lit("Ärger")],
    ),
    (
        "überlasstet",
        &[
            SuggestExpr::Lit("überlastet"),
            SuggestExpr::Lit("überließt"),
        ],
    ),
    (
        "zeren",
        &[SuggestExpr::Lit("zerren"), SuggestExpr::Lit("zehren")],
    ),
    (
        "Hänchen",
        &[SuggestExpr::Lit("Hähnchen"), SuggestExpr::Lit("Hänschen")],
    ),
    ("[sS]itwazion", &[SuggestExpr::Lit("Situation")]),
    ("geschriehen", &[SuggestExpr::Lit("geschrien")]),
    ("beratete", &[SuggestExpr::Lit("beriet")]),
    ("Hälst", &[SuggestExpr::Lit("Hältst")]),
    ("[kK]aos", &[SuggestExpr::Lit("Chaos")]),
    ("[pP]upatät", &[SuggestExpr::Lit("Pubertät")]),
    ("überwendet", &[SuggestExpr::Lit("überwindet")]),
    ("[bB]esichtung", &[SuggestExpr::Lit("Besichtigung")]),
    ("[hH]ell?owi[eh]?n", &[SuggestExpr::Lit("Halloween")]),
    ("geschmelt?zt", &[SuggestExpr::Lit("geschmolzen")]),
    ("gewunschen", &[SuggestExpr::Lit("gewünscht")]),
    ("bittete", &[SuggestExpr::Lit("bat")]),
    ("nehm", &[SuggestExpr::Lit("nimm")]),
    ("möchst", &[SuggestExpr::Lit("möchtest")]),
    ("Win", &[SuggestExpr::Lit("Windows")]),
    ("anschein[dt]", &[SuggestExpr::Lit("anscheinend")]),
    ("Subvestitionen", &[SuggestExpr::Lit("Subventionen")]),
    ("angeschaffen", &[SuggestExpr::Lit("angeschafft")]),
    ("Rechtspruch", &[SuggestExpr::Lit("Rechtsspruch")]),
    ("Second-Hand", &[SuggestExpr::Lit("Secondhand")]),
    ("[jJ]ahundert", &[SuggestExpr::Lit("Jahrhundert")]),
    ("Gesochse", &[SuggestExpr::Lit("Gesocks")]),
    ("Vorraus", &[SuggestExpr::Lit("Voraus")]),
    ("[vV]orgensweise", &[SuggestExpr::Lit("Vorgehensweise")]),
    ("[kK]autsch", &[SuggestExpr::Lit("Couch")]),
    ("guterletzt", &[SuggestExpr::Lit("guter Letzt")]),
    ("Seminares", &[SuggestExpr::Lit("Seminars")]),
    ("Mousepad", &[SuggestExpr::Lit("Mauspad")]),
    ("Mousepads", &[SuggestExpr::Lit("Mauspads")]),
    ("Wi[Ff]i-Router", &[SuggestExpr::Lit("Wi-Fi-Router")]),
    (
        "[Ll]ilane[srm]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ilane[srm]?",
            replacement: "ila",
        }],
    ),
    (
        "[zZ]uguterletzt",
        &[SuggestExpr::ReplaceFirst {
            pattern: "guterletzt",
            replacement: " guter Letzt",
        }],
    ),
    (
        "Nootbooks?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Noot",
            replacement: "Note",
        }],
    ),
    (
        "[vV]ersendlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "send",
            replacement: "sehent",
        }],
    ),
    (
        "[uU]nfäh?r(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "fäh?r",
            replacement: "fair",
        }],
    ),
    (
        "[mM]edikatös(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ka",
            replacement: "kamen",
        }],
    ),
    (
        "(ein|zwei|drei|vier|fünf|sechs|sieben|acht|neun|zehn|elf|zwölf)undhalb",
        &[SuggestExpr::ReplaceFirst {
            pattern: "und",
            replacement: "ein",
        }],
    ),
    (
        "[gG]roßzüge[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "züg",
            replacement: "zügig",
        }],
    ),
    (
        "[äÄ]rtlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "rt",
            replacement: "rzt",
        }],
    ),
    (
        "[sS]chnelligkeitsfehler[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[sS]chnell",
            replacement: "Flücht",
        }],
    ),
    (
        "[sS]chweinerosane[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "weinerosane[mnrs]?",
            replacement: "weinchenrosa",
        }],
    ),
    (
        "[aA]nstecklich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "end",
        }],
    ),
    (
        "[gG]eflechtet(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "flechtet",
            replacement: "flochten",
        }],
    ),
    (
        "[gG]enrealistisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "re",
            replacement: "er",
        }],
    ),
    (
        "überträgt(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^überträgt",
            replacement: "übertragen",
        }],
    ),
    (
        "[iI]nterresent(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "rresent",
            replacement: "ressant",
        }],
    ),
    (
        "Simkartenleser[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^Simkartenl",
            replacement: "SIM-Karten-L",
        }],
    ),
    (
        "Hilfstmittel[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^Hilfst",
            replacement: "Hilfs",
        }],
    ),
    (
        "trationell(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^tra",
            replacement: "tradi",
        }],
    ),
    (
        "[bB]erreichs?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^[bB]er",
            replacement: "Be",
        }],
    ),
    (
        "[fF]uscher[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^[fF]u",
            replacement: "Pfu",
        }],
    ),
    (
        "[uU]nausweichbar(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "bar",
            replacement: "lich",
        }],
    ),
    (
        "[uU]nabdinglich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "bar",
        }],
    ),
    (
        "[eE]ingänglich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "ig",
        }],
    ),
    (
        "ausgewöh?nlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^ausgewöh?n",
            replacement: "außergewöhn",
        }],
    ),
    (
        "achsial(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^achs",
            replacement: "ax",
        }],
    ),
    (
        "famielen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^famiel",
            replacement: "Famili",
        }],
    ),
    (
        "miter[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^mi",
            replacement: "Mie",
        }],
    ),
    (
        "besig(t(e[mnrs]?)?|en?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "sig",
            replacement: "sieg",
        }],
    ),
    (
        "[vV]erziehr(t(e[mnrs]?)?|en?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ieh",
            replacement: "ie",
        }],
    ),
    (
        "^[pP]iek(s?t|en?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "iek",
            replacement: "ik",
        }],
    ),
    (
        "[mM]atschscheiben?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[mM]atschsch",
            replacement: "Mattsch",
        }],
    ),
    (
        "schafen?",
        &[
            SuggestExpr::ReplaceOnce {
                from: "sch",
                to: "schl",
            },
            SuggestExpr::ReplaceOnce {
                from: "af",
                to: "arf",
            },
            SuggestExpr::ReplaceOnce {
                from: "af",
                to: "aff",
            },
        ],
    ),
    ("zuschafen", &[SuggestExpr::Lit("zu schaffen")]),
    (
        "[hH]ofen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "of",
            replacement: "off",
        }],
    ),
    (
        "[sS]ommerverien?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[sS]ommerverien?",
            replacement: "Sommerferien",
        }],
    ),
    (
        "[rR]ecourcen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[rR]ec",
            replacement: "Ress",
        }],
    ),
    (
        "[fF]amm?ill?i?[aä]risch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "amm?ill?i?[aä]risch",
            replacement: "amiliär",
        }],
    ),
    (
        "Sim-Karten?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^Sim",
            replacement: "SIM",
        }],
    ),
    (
        "Spax-Schrauben?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^Spax",
            replacement: "SPAX",
        }],
    ),
    (
        "[aA]leine",
        &[SuggestExpr::ReplaceFirst {
            pattern: "l",
            replacement: "ll",
        }],
    ),
    (
        "Kaput",
        &[SuggestExpr::ReplaceFirst {
            pattern: "t",
            replacement: "tt",
        }],
    ),
    (
        "[fF]estell(s?t|en?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "est",
            replacement: "estst",
        }],
    ),
    (
        "[Ee]igtl",
        &[SuggestExpr::ReplaceFirst {
            pattern: "igtl",
            replacement: "igtl.",
        }],
    ),
    (
        "(Baden-)?Würtenbergs?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Würten",
            replacement: "Württem",
        }],
    ),
    (
        "Betriebsratzimmer[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "rat",
            replacement: "rats",
        }],
    ),
    (
        "Rechts?schreibungsfehler[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Rechts?schreibungs",
            replacement: "Rechtschreib",
        }],
    ),
    (
        "Open[aA]ir-Konzert(en?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Open[aA]ir",
            replacement: "Open-Air",
        }],
    ),
    (
        "Jugenschuhen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Jug",
            replacement: "Jung",
        }],
    ),
    (
        "TODO-Listen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "TODO",
            replacement: "To-do",
        }],
    ),
    (
        "ausiehs?t",
        &[SuggestExpr::ReplaceFirst {
            pattern: "aus",
            replacement: "auss",
        }],
    ),
    (
        "unterbemittel(nd|t)(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "unterbemittel(nd|t)",
            replacement: "minderbemittelt",
        }],
    ),
    (
        "[xX]te[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "te",
            replacement: "-te",
        }],
    ),
    (
        "verheielt(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "heiel",
            replacement: "heil",
        }],
    ),
    (
        "[rR]evolutionie?sier(s?t|en?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ie?s",
            replacement: "",
        }],
    ),
    (
        "Kohleaustiegs?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "aus",
            replacement: "auss",
        }],
    ),
    (
        "[jJ]urististisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "istist",
            replacement: "ist",
        }],
    ),
    (
        "gehäckelt(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ck",
            replacement: "k",
        }],
    ),
    (
        "deutsprachig(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "deut",
            replacement: "deutsch",
        }],
    ),
    (
        "angesehend(st)?e[nmrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "end",
            replacement: "en",
        }],
    ),
    (
        "[iI]slamophobisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "isch",
            replacement: "",
        }],
    ),
    (
        "[vV]erharkt(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ar",
            replacement: "a",
        }],
    ),
    (
        "[dD]esöfterer?[nm]",
        &[SuggestExpr::ReplaceFirst {
            pattern: "öfterer?[nm]",
            replacement: " Öfteren",
        }],
    ),
    (
        "[dD]eswei[dt]ere?[mn]",
        &[SuggestExpr::ReplaceFirst {
            pattern: "wei[dt]ere?[mn]",
            replacement: " Weiteren",
        }],
    ),
    (
        "Einkaufstachen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ch",
            replacement: "sch",
        }],
    ),
    (
        "Bortmesser[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Bor",
            replacement: "Bro",
        }],
    ),
    (
        "Makeupstylist(in(nen)?|en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Makeups",
            replacement: "Make-up-S",
        }],
    ),
    (
        "Fee?dbäcks?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Fee?dbäck",
            replacement: "Feedback",
        }],
    ),
    (
        "weirete[nmrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ret",
            replacement: "ter",
        }],
    ),
    (
        "Ni[vw]oschalter[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Ni[vw]o",
            replacement: "Niveau",
        }],
    ),
    (
        "[eE]xhibitionisch(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "isch",
            replacement: "istisch",
        }],
    ),
    (
        "(ein|aus)?[gG]eschalten(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ten",
            replacement: "tet",
        }],
    ),
    (
        "[uU]nterschiebene[nmrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "sch",
            replacement: "schr",
        }],
    ),
    (
        "[uU]nbequemlich(st)?e[nmrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "",
        }],
    ),
    (
        "[uU][nm]bekweh?m(e[nmrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[nm]bekweh?m",
            replacement: "nbequem",
        }],
    ),
    (
        "[dD]esatör(s|en?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "satör",
            replacement: "serteur",
        }],
    ),
    (
        "Panelen?",
        &[
            SuggestExpr::ReplaceOnce {
                from: "Panel",
                to: "Paneel",
            },
            SuggestExpr::Lit("Panels"),
        ],
    ),
    ("D[eèé]ja-?[vV]o?ue?", &[SuggestExpr::Lit("Déjà-vu")]),
    (
        "Cr[eèé]me-?fra[iî]che",
        &[SuggestExpr::Lit("Crème fraîche")],
    ),
    ("[aA]rr?an?gemont", &[SuggestExpr::Lit("Arrangement")]),
    ("[aA]ngagemon", &[SuggestExpr::Lit("Engagement")]),
    ("Phyrr?ussieg", &[SuggestExpr::Lit("Pyrrhussieg")]),
    ("Mio", &[SuggestExpr::Lit("Mio.")]),
    ("Datein", &[SuggestExpr::Lit("Dateien")]),
    ("[pP]u(zz|ss)el", &[SuggestExpr::Lit("Puzzle")]),
    ("Smilies", &[SuggestExpr::Lit("Smileys")]),
    ("[dD]iseing?", &[SuggestExpr::Lit("Design")]),
    ("[lL]ieradd?ress?e", &[SuggestExpr::Lit("Lieferadresse")]),
    ("[bB]o[yi]kutierung", &[SuggestExpr::Lit("Boykottierung")]),
    ("Mouseclick", &[SuggestExpr::Lit("Mausklick")]),
    (
        "[aA]ktuelli?esie?rung",
        &[SuggestExpr::Lit("Aktualisierung")],
    ),
    ("Händy", &[SuggestExpr::Lit("Handy")]),
    ("gewertschätzt", &[SuggestExpr::Lit("wertgeschätzt")]),
    ("tieger", &[SuggestExpr::Lit("Tiger")]),
    (
        "Rollade",
        &[SuggestExpr::Lit("Rollladen"), SuggestExpr::Lit("Roulade")],
    ),
    ("garnichtmehr", &[SuggestExpr::Lit("gar nicht mehr")]),
    ("vileich", &[SuggestExpr::Lit("vielleicht")]),
    ("vll?t", &[SuggestExpr::Lit("vielleicht")]),
    ("aufgewägt", &[SuggestExpr::Lit("aufgewogen")]),
    ("[rR]eflektion", &[SuggestExpr::Lit("Reflexion")]),
    ("momentmal", &[SuggestExpr::Lit("Moment mal")]),
    ("satzt", &[SuggestExpr::Lit("Satz")]),
    (
        "Büff?(ee|é)",
        &[SuggestExpr::Lit("Buffet"), SuggestExpr::Lit("Büfett")],
    ),
    (
        "[fF]rühstücksb[uü]ff?(é|ee)",
        &[SuggestExpr::Lit("Frühstücksbuffet")],
    ),
    ("[aA]lterego", &[SuggestExpr::Lit("Alter Ego")]),
    ("Copyride", &[SuggestExpr::Lit("Copyright")]),
    ("Analysierung", &[SuggestExpr::Lit("Analyse")]),
    ("Exel", &[SuggestExpr::Lit("Excel")]),
    ("Glücklichkeit", &[SuggestExpr::Lit("Glück")]),
    ("Begierigkeit", &[SuggestExpr::Lit("Begierde")]),
    ("voralem", &[SuggestExpr::Lit("vor allem")]),
    (
        "Unorganisation",
        &[
            SuggestExpr::Lit("Desorganisation"),
            SuggestExpr::Lit("Unorganisiertheit"),
        ],
    ),
    (
        "Cand(el|le)lightdinner",
        &[SuggestExpr::Lit("Candle-Light-Dinner")],
    ),
    ("wertgelegt", &[SuggestExpr::Lit("Wert gelegt")]),
    ("antuhen", &[SuggestExpr::Lit("antun")]),
    ("komen", &[SuggestExpr::Lit("kommen")]),
    ("genißen", &[SuggestExpr::Lit("genießen")]),
    (
        "Stationskrankenpflegerin",
        &[SuggestExpr::Lit("Stationsschwester")],
    ),
    (
        "[iIüÜuU]b[ea]w[ae]isung",
        &[SuggestExpr::Lit("Überweisung")],
    ),
    ("[bB]oxhorn", &[SuggestExpr::Lit("Bockshorn")]),
    ("[zZ]oolophie", &[SuggestExpr::Lit("Zoophilie")]),
    ("Makieren", &[SuggestExpr::Lit("Markieren")]),
    ("Altersheimer", &[SuggestExpr::Lit("Alzheimer")]),
    ("gesen", &[SuggestExpr::Lit("gesehen")]),
    (
        "Neugierigkeit",
        &[SuggestExpr::Lit("Neugier"), SuggestExpr::Lit("Neugierde")],
    ),
    ("[kK]onn?ekt?schen", &[SuggestExpr::Lit("Connection")]),
    ("E-Maul", &[SuggestExpr::Lit("E-Mail")]),
    ("E-Mauls", &[SuggestExpr::Lit("E-Mails")]),
    ("E-Mal", &[SuggestExpr::Lit("E-Mail")]),
    ("E-Mals", &[SuggestExpr::Lit("E-Mails")]),
    ("[nN]ah?richt", &[SuggestExpr::Lit("Nachricht")]),
    ("[nN]ah?richten", &[SuggestExpr::Lit("Nachrichten")]),
    ("Getrixe", &[SuggestExpr::Lit("Getrickse")]),
    ("Ausage", &[SuggestExpr::Lit("Aussage")]),
    ("gelessen", &[SuggestExpr::Lit("gelesen")]),
    ("Kanst", &[SuggestExpr::Lit("Kannst")]),
    ("Unwohlbefinden", &[SuggestExpr::Lit("Unwohlsein")]),
    ("leiwagen", &[SuggestExpr::Lit("Leihwagen")]),
    ("krahn", &[SuggestExpr::Lit("Kran")]),
    ("[hH]ifi", &[SuggestExpr::Lit("Hi-Fi")]),
    ("chouch", &[SuggestExpr::Lit("Couch")]),
    ("eh?rgeit?z", &[SuggestExpr::Lit("Ehrgeiz")]),
    ("solltes", &[SuggestExpr::Lit("solltest")]),
    ("geklabt", &[SuggestExpr::Lit("geklappt")]),
    ("angefangt", &[SuggestExpr::Lit("angefangen")]),
    ("beinhält", &[SuggestExpr::Lit("beinhaltet")]),
    ("beinhielt", &[SuggestExpr::Lit("beinhaltete")]),
    ("beinhielten", &[SuggestExpr::Lit("beinhalteten")]),
    ("einhaltest", &[SuggestExpr::Lit("einhältst")]),
    ("angeruft", &[SuggestExpr::Lit("angerufen")]),
    ("erhaltete", &[SuggestExpr::Lit("erhielt")]),
    ("übersäht", &[SuggestExpr::Lit("übersät")]),
    (
        "staats?angehoe?rigkeit",
        &[SuggestExpr::Lit("Staatsangehörigkeit")],
    ),
    (
        "[uU]nangeneh?mheiten",
        &[SuggestExpr::Lit("Unannehmlichkeiten")],
    ),
    ("Humuspaste", &[SuggestExpr::Lit("Hummuspaste")]),
    ("afarung", &[SuggestExpr::Lit("Erfahrung")]),
    ("bescheid?t", &[SuggestExpr::Lit("Bescheid")]),
    ("[mM]iteillung", &[SuggestExpr::Lit("Mitteilung")]),
    ("Revisionierung", &[SuggestExpr::Lit("Revision")]),
    (
        "[eE]infühlvermögen",
        &[SuggestExpr::Lit("Einfühlungsvermögen")],
    ),
    (
        "[sS]peziellisierung",
        &[SuggestExpr::Lit("Spezialisierung")],
    ),
    ("[cC]hangse", &[SuggestExpr::Lit("Chance")]),
    ("untergangen", &[SuggestExpr::Lit("untergegangen")]),
    ("geliegt", &[SuggestExpr::Lit("gelegen")]),
    ("BluRay", &[SuggestExpr::Lit("Blu-ray")]),
    ("Freiwilligerin", &[SuggestExpr::Lit("Freiwillige")]),
    (
        "Mitgliederinnen",
        &[
            SuggestExpr::Lit("Mitglieder"),
            SuggestExpr::Lit("Mitgliedern"),
        ],
    ),
    ("Hautreinheiten", &[SuggestExpr::Lit("Hautunreinheiten")]),
    ("Durfüh?rung", &[SuggestExpr::Lit("Durchführung")]),
    ("tuhen", &[SuggestExpr::Lit("tun")]),
    ("tuhe", &[SuggestExpr::Lit("tue")]),
    ("tip", &[SuggestExpr::Lit("Tipp")]),
    ("ccm", &[SuggestExpr::Lit("cm³")]),
    ("Kilimand?jaro", &[SuggestExpr::Lit("Kilimandscharo")]),
    ("[hH]erausfor?dung", &[SuggestExpr::Lit("Herausforderung")]),
    ("[bB]erücksichtung", &[SuggestExpr::Lit("Berücksichtigung")]),
    ("artzt?", &[SuggestExpr::Lit("Arzt")]),
    ("[tT]h?elepath?ie", &[SuggestExpr::Lit("Telepathie")]),
    ("Wi-?Fi-Dire[ck]t", &[SuggestExpr::Lit("Wi-Fi Direct")]),
    ("gans", &[SuggestExpr::Lit("ganz")]),
    ("Pearl-Harbou?r", &[SuggestExpr::Lit("Pearl Harbor")]),
    ("[aA]utonomität", &[SuggestExpr::Lit("Autonomie")]),
    ("[fF]r[uü]h?st[uü]c?k", &[SuggestExpr::Lit("Frühstück")]),
    (
        "(ge)?fr[uü]h?st[uü](c?k|g)t",
        &[SuggestExpr::ReplaceFirst {
            pattern: "fr[uü]h?st[uü](c?k|g)t",
            replacement: "frühstückt",
        }],
    ),
    ("zucc?h?inis?", &[SuggestExpr::Lit("Zucchini")]),
    ("[mM]itag", &[SuggestExpr::Lit("Mittag")]),
    ("Lexion", &[SuggestExpr::Lit("Lexikon")]),
    ("[mM]otorisation", &[SuggestExpr::Lit("Motorisierung")]),
    ("[fF]ormalisation", &[SuggestExpr::Lit("Formalisierung")]),
    ("ausprache", &[SuggestExpr::Lit("Aussprache")]),
    ("[mM]enegment", &[SuggestExpr::Lit("Management")]),
    ("[gG]ebrauspuren", &[SuggestExpr::Lit("Gebrauchsspuren")]),
    ("viedeo", &[SuggestExpr::Lit("Video")]),
    ("[hH]erstammung", &[SuggestExpr::Lit("Abstammung")]),
    ("[iI]nstall?atör", &[SuggestExpr::Lit("Installateur")]),
    ("maletriert", &[SuggestExpr::Lit("malträtiert")]),
    ("abgeschaffen", &[SuggestExpr::Lit("abgeschafft")]),
    ("Verschiden", &[SuggestExpr::Lit("Verschieden")]),
    ("Anschovis", &[SuggestExpr::Lit("Anchovis")]),
    ("Bravur", &[SuggestExpr::Lit("Bravour")]),
    ("Grisli", &[SuggestExpr::Lit("Grizzly")]),
    ("Grislibär", &[SuggestExpr::Lit("Grizzlybär")]),
    ("Grislibären", &[SuggestExpr::Lit("Grizzlybären")]),
    ("Frotté", &[SuggestExpr::Lit("Frottee")]),
    ("Joga", &[SuggestExpr::Lit("Yoga")]),
    ("Kalvinismus", &[SuggestExpr::Lit("Calvinismus")]),
    ("Kollier", &[SuggestExpr::Lit("Collier")]),
    ("Kolliers", &[SuggestExpr::Lit("Colliers")]),
    ("Ketschup", &[SuggestExpr::Lit("Ketchup")]),
    ("Kommunikee", &[SuggestExpr::Lit("Kommuniqué")]),
    ("Negligee", &[SuggestExpr::Lit("Negligé")]),
    ("Nessessär", &[SuggestExpr::Lit("Necessaire")]),
    ("passee", &[SuggestExpr::Lit("passé")]),
    ("Varietee", &[SuggestExpr::Lit("Varieté")]),
    ("Varietees", &[SuggestExpr::Lit("Varietés")]),
    ("Wandalismus", &[SuggestExpr::Lit("Vandalismus")]),
    ("Campagne", &[SuggestExpr::Lit("Kampagne")]),
    ("Campagnen", &[SuggestExpr::Lit("Kampagnen")]),
    ("Jockei", &[SuggestExpr::Lit("Jockey")]),
    ("Roulett", &[SuggestExpr::Lit("Roulette")]),
    ("Bestellungsdaten", &[SuggestExpr::Lit("Bestelldaten")]),
    ("Package", &[SuggestExpr::Lit("Paket")]),
    ("E-mail", &[SuggestExpr::Lit("E-Mail")]),
    ("geleased", &[SuggestExpr::Lit("geleast")]),
    ("released", &[SuggestExpr::Lit("releast")]),
    (
        "Ballets?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "llet",
            replacement: "llett",
        }],
    ),
    (
        "Saudiarabiens?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Saudiarabien",
            replacement: "Saudi-Arabien",
        }],
    ),
    (
        "eMail-Adressen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "eMail-",
            replacement: "E-Mail-",
        }],
    ),
    (
        "[Ww]ieviele?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ieviel",
            replacement: "ie viel",
        }],
    ),
    (
        "[Aa]dhoc",
        &[SuggestExpr::ReplaceFirst {
            pattern: "dhoc",
            replacement: "d hoc",
        }],
    ),
    ("As", &[SuggestExpr::Lit("Ass")]),
    ("[bB]i[sß](s?[ij]|ch)en", &[SuggestExpr::Lit("bisschen")]),
    (
        "Todos?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Todo",
            replacement: "To-do",
        }],
    ),
    ("Kovult", &[SuggestExpr::Lit("Konvolut")]),
    (
        "blog(t?en?|t(es?t)?)$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "g",
            replacement: "gg",
        }],
    ),
    ("Zombiefizierungen", &[SuggestExpr::Lit("Zombifizierungen")]),
    (
        "Tret",
        &[
            SuggestExpr::Lit("Tritt"),
            SuggestExpr::Lit("Trete"),
            SuggestExpr::Lit("Trat"),
        ],
    ),
    (
        "Hühne",
        &[
            SuggestExpr::Lit("Bühne"),
            SuggestExpr::Lit("Hüne"),
            SuggestExpr::Lit("Hühner"),
        ],
    ),
    (
        "Hühnen",
        &[
            SuggestExpr::Lit("Bühnen"),
            SuggestExpr::Lit("Hünen"),
            SuggestExpr::Lit("Hühnern"),
        ],
    ),
    ("tiptop", &[SuggestExpr::Lit("tiptopp")]),
    ("Briese", &[SuggestExpr::Lit("Brise")]),
    (
        "Rechtsschreibreformen",
        &[SuggestExpr::Lit("Rechtschreibreformen")],
    ),
    (
        "gewertschätzte(([mnrs]|re[mnrs]?)?)$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "gewertschätzt",
            replacement: "wertgeschätzt",
        }],
    ),
    (
        "knapps(t?en?|t(es?t)?)$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "pp",
            replacement: "p",
        }],
    ),
    ("geknappst", &[SuggestExpr::Lit("geknapst")]),
    (
        "gepiekste[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ie",
            replacement: "i",
        }],
    ),
    (
        "Yings?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ng",
            replacement: "n",
        }],
    ),
    ("Wiederstandes", &[SuggestExpr::Lit("Widerstandes")]),
    (
        "veganisch(e?[mnrs]?)$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "isch",
            replacement: "",
        }],
    ),
    (
        "totlangweiligste[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "tot",
            replacement: "tod",
        }],
    ),
    (
        "tottraurigste[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "tot",
            replacement: "tod",
        }],
    ),
    (
        "kreir(n|e?nd)(e[mnrs]?)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ire?n",
            replacement: "ieren",
        }],
    ),
    (
        "Pepps?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "pp",
            replacement: "p",
        }],
    ),
    (
        "Pariahs?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "h",
            replacement: "",
        }],
    ),
    (
        "Oeuvres?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Oe",
            replacement: "Œ",
        }],
    ),
    ("Margarite", &[SuggestExpr::Lit("Margerite")]),
    (
        "Kücken",
        &[SuggestExpr::Lit("Rücken"), SuggestExpr::Lit("Küken")],
    ),
    (
        "Kompanten",
        &[SuggestExpr::Lit("Kompasse"), SuggestExpr::Lit("Kompassen")],
    ),
    ("Kandarren", &[SuggestExpr::Lit("Kandaren")]),
    ("kniehen", &[SuggestExpr::Lit("knien")]),
    (
        "infisziertes?t$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "fisz",
            replacement: "fiz",
        }],
    ),
    (
        "Imbusse(n|s)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "m",
            replacement: "n",
        }],
    ),
    ("Hollundern", &[SuggestExpr::Lit("Holundern")]),
    (
        "handgehabt(e?[mnrs]?)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "handgehabt",
            replacement: "gehandhabt",
        }],
    ),
    ("Funieres", &[SuggestExpr::Lit("Furniers")]),
    ("Frohndiensts", &[SuggestExpr::Lit("Frondiensts")]),
    ("fithälst", &[SuggestExpr::Lit("fit hältst")]),
    (
        "fitzuhalten(de?[mnrs]?)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "fitzuhalten",
            replacement: "fit zu halten",
        }],
    ),
    (
        "(essen|schlafen|schwimmen|spazieren)zugehen$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "zugehen",
            replacement: " zu gehen",
        }],
    ),
    (
        "dilettant",
        &[
            SuggestExpr::Lit("Dilettant"),
            SuggestExpr::Lit("dilettantisch"),
        ],
    ),
    (
        "dilettante[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "te",
            replacement: "tische",
        }],
    ),
    ("Disastern", &[SuggestExpr::Lit("Desastern")]),
    (
        "Brandwein(en?|s)$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "d",
            replacement: "nt",
        }],
    ),
    (
        "Böhen?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "h",
            replacement: "",
        }],
    ),
    (
        "Aufständige[mnr]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ig",
            replacement: "isch",
        }],
    ),
    (
        "aufständig(e[mnrs]?)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ig",
            replacement: "isch",
        }],
    ),
    (
        "duzend(e[mnrs]?)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "uzend",
            replacement: "utzend",
        }],
    ),
    (
        "unrelevant(e[mnrs]?)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "un",
            replacement: "ir",
        }],
    ),
    (
        "Unrelevant(e[mnrs]?)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Un",
            replacement: "Ir",
        }],
    ),
    ("aufgrundedessen", &[SuggestExpr::Lit("aufgrund dessen")]),
    ("Amalgane", &[SuggestExpr::Lit("Amalgame")]),
    (
        "Kafe",
        &[SuggestExpr::Lit("Kaffee"), SuggestExpr::Lit("Café")],
    ),
    (
        "Dammbock",
        &[SuggestExpr::Lit("Dambock"), SuggestExpr::Lit("Rammbock")],
    ),
    ("Dammhirsch", &[SuggestExpr::Lit("Damhirsch")]),
    ("Fairnis", &[SuggestExpr::Lit("Fairness")]),
    (
        "auschluss",
        &[
            SuggestExpr::Lit("Ausschluss"),
            SuggestExpr::Lit("Ausschuss"),
        ],
    ),
    (
        "derikter",
        &[SuggestExpr::Lit("direkter"), SuggestExpr::Lit("Direktor")],
    ),
    ("[iI]dentifierung", &[SuggestExpr::Lit("Identifikation")]),
    ("[eE]mphatie", &[SuggestExpr::Lit("Empathie")]),
    ("[eE]iskrem", &[SuggestExpr::Lit("Eiscreme")]),
    ("[fF]lüchtung", &[SuggestExpr::Lit("Flucht")]),
    ("einamen", &[SuggestExpr::Lit("Einnahmen")]),
    ("[eE]inbu(ss|ß)ung", &[SuggestExpr::Lit("Einbuße")]),
    ("[eE]inbu(ss|ß)ungen", &[SuggestExpr::Lit("Einbußen")]),
    ("nachichten", &[SuggestExpr::Lit("Nachrichten")]),
    ("gegehen", &[SuggestExpr::Lit("gegangen")]),
    ("Ethnocid", &[SuggestExpr::Lit("Ethnozid")]),
    ("Exikose", &[SuggestExpr::Lit("Exsikkose")]),
    (
        "Schonvermögengrenze",
        &[SuggestExpr::Lit("Schonvermögensgrenze")],
    ),
    ("kontest", &[SuggestExpr::Lit("konntest")]),
    ("pitza", &[SuggestExpr::Lit("Pizza")]),
    ("Tütü", &[SuggestExpr::Lit("Tutu")]),
    ("gebittet", &[SuggestExpr::Lit("gebeten")]),
    ("gekricht", &[SuggestExpr::Lit("gekriegt")]),
    ("Krankenheit", &[SuggestExpr::Lit("Krankheit")]),
    ("Krankenheiten", &[SuggestExpr::Lit("Krankheiten")]),
    ("[hH]udd[yi]", &[SuggestExpr::Lit("Hoodie")]),
    ("Treibel", &[SuggestExpr::Lit("Tribal")]),
    ("vorort", &[SuggestExpr::Lit("vor Ort")]),
    ("Brotwürfelcro[uû]tons", &[SuggestExpr::Lit("Croûtons")]),
    ("bess?tetigung", &[SuggestExpr::Lit("Bestätigung")]),
    ("[mM]ayonaisse", &[SuggestExpr::Lit("Mayonnaise")]),
    ("misverstaendnis", &[SuggestExpr::Lit("Missverständnis")]),
    ("[vV]erlu(ss|ß)t", &[SuggestExpr::Lit("Verlust")]),
    ("glückigerweise", &[SuggestExpr::Lit("glücklicherweise")]),
    ("[sS]tandtart", &[SuggestExpr::Lit("Standard")]),
    ("Mainzerstrasse", &[SuggestExpr::Lit("Mainzer Straße")]),
    (
        "Genehmigerablauf",
        &[SuggestExpr::Lit("Genehmigungsablauf")],
    ),
    (
        "Bestellerurkunde",
        &[SuggestExpr::Lit("Bestellungsurkunde")],
    ),
    ("Selbstmitleidigkeit", &[SuggestExpr::Lit("Selbstmitleid")]),
    ("[iI]ntuion", &[SuggestExpr::Lit("Intuition")]),
    ("[cCkK]ontener", &[SuggestExpr::Lit("Container")]),
    ("Barcadi", &[SuggestExpr::Lit("Bacardi")]),
    ("Unnanehmigkeit", &[SuggestExpr::Lit("Unannehmlichkeit")]),
    ("[wW]ischmöppen?", &[SuggestExpr::Lit("Wischmopps")]),
    (
        "[oO]rdnungswiedrichkeit(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[oO]rdnungswiedrich",
            replacement: "Ordnungswidrig",
        }],
    ),
    (
        "Mauntenbiker[ns]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "^Maunten",
            replacement: "Mountain",
        }],
    ),
    (
        "Mauntenbikes?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Maunten",
            replacement: "Mountain",
        }],
    ),
    (
        "[nN]euhichkeit(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[nN]euhich",
            replacement: "Neuig",
        }],
    ),
    (
        "Prokopfverbrauchs?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Prokopfv",
            replacement: "Pro-Kopf-V",
        }],
    ),
    (
        "[Gg]ilst",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ilst",
            replacement: "iltst",
        }],
    ),
    (
        "[vV]ollrichtung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[vV]oll",
            replacement: "Ver",
        }],
    ),
    (
        "[vV]ollrichtest",
        &[SuggestExpr::ReplaceFirst {
            pattern: "oll",
            replacement: "er",
        }],
    ),
    (
        "[vV]ollrichten?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "oll",
            replacement: "er",
        }],
    ),
    (
        "[vV]ollrichtet(e([mnrs])?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "oll",
            replacement: "er",
        }],
    ),
    (
        "[bB]edingslos(e([mnrs])?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ding",
            replacement: "dingung",
        }],
    ),
    (
        "[eE]insichtbar(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "sicht",
            replacement: "seh",
        }],
    ),
    (
        "asymetrisch(ere|ste)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ym",
            replacement: "ymm",
        }],
    ),
    (
        "alterwürdig(ere|ste)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lter",
            replacement: "ltehr",
        }],
    ),
    (
        "aufständig(ere|ste)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ig",
            replacement: "isch",
        }],
    ),
    (
        "blutdurstig(ere|ste)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ur",
            replacement: "ür",
        }],
    ),
    (
        "dilettant(ere|este)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "nt",
            replacement: "ntisch",
        }],
    ),
    (
        "eliptisch(ere|ste)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "l",
            replacement: "ll",
        }],
    ),
    (
        "angegröhlt(e([mnrs])?)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "öh",
            replacement: "ö",
        }],
    ),
    (
        "gothisch(ere|ste)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "th",
            replacement: "t",
        }],
    ),
    (
        "kollossal(ere|ste)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ll",
            replacement: "l",
        }],
    ),
    (
        "paralel(lere|lste)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "paralel",
            replacement: "paralle",
        }],
    ),
    (
        "symetrischste[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ym",
            replacement: "ymm",
        }],
    ),
    (
        "rethorisch(ere|ste)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "rethor",
            replacement: "rhetor",
        }],
    ),
    (
        "repetativ(ere|ste)[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "repetat",
            replacement: "repetit",
        }],
    ),
    (
        "voluptös(e|ere|este)?[mnrs]?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "tös",
            replacement: "tuös",
        }],
    ),
    (
        "[pP]flanzig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ig",
            replacement: "lich",
        }],
    ),
    (
        "geblogt(e[mnrs]?)?$",
        &[SuggestExpr::ReplaceFirst {
            pattern: "gt",
            replacement: "ggt",
        }],
    ),
    (
        "herraus.*",
        &[SuggestExpr::ReplaceFirst {
            pattern: "herraus",
            replacement: "heraus",
        }],
    ),
    (
        "[aA]bbonier(en?|s?t|te[mnrst]?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "bbo",
            replacement: "bon",
        }],
    ),
    (
        "[aA]pelier(en?|s?t|te[nt]?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "pel",
            replacement: "ppell",
        }],
    ),
    (
        "[vV]oltie?schier(en?|s?t|te[nt]?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ie?sch",
            replacement: "ig",
        }],
    ),
    (
        "[mM]eistverkaufteste[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "teste",
            replacement: "te",
        }],
    ),
    (
        "[uU]nleshaft(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "haft",
            replacement: "erlich",
        }],
    ),
    (
        "[gG]laubenswürdig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ens",
            replacement: "",
        }],
    ),
    (
        "[nN]i[vw]ovoll(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[vw]ovoll",
            replacement: "veauvoll",
        }],
    ),
    (
        "[nN]otgezwungend?(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "zwungend?",
            replacement: "drungen",
        }],
    ),
    (
        "[mM]isstraurig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "rig",
            replacement: "isch",
        }],
    ),
    (
        "[iI]nflagrantie?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "flagrantie?",
            replacement: " flagranti",
        }],
    ),
    (
        "Aux-Anschl(uss(es)?|üssen?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Aux",
            replacement: "AUX",
        }],
    ),
    (
        "desinfektiert(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "fekt",
            replacement: "fiz",
        }],
    ),
    (
        "desinfektierend(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "fekt",
            replacement: "fiz",
        }],
    ),
    (
        "desinfektieren?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "fekt",
            replacement: "fiz",
        }],
    ),
    (
        "[dD]esinfektionier(en?|t(e[mnrs]?)?|st)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "fektionier",
            replacement: "fizier",
        }],
    ),
    (
        "[dD]esinfektionierend(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "fektionier",
            replacement: "fizier",
        }],
    ),
    (
        "[kK]ompensionier(en?|t(e[mnrs]?)?|st)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ion",
            replacement: "",
        }],
    ),
    (
        "neuliche[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "neu",
            replacement: "neuer",
        }],
    ),
    (
        "ausbüchsen?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "chs",
            replacement: "x",
        }],
    ),
    (
        "aus(ge)?büchst(en?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "chs",
            replacement: "x",
        }],
    ),
    (
        "innoff?iziell?(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "innoff?iziell?",
            replacement: "inoffiziell",
        }],
    ),
    (
        "[gG]roesste[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "oess",
            replacement: "öß",
        }],
    ),
    (
        "[tT]efonisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "efon",
            replacement: "elefon",
        }],
    ),
    (
        "[oO]ptimalisiert",
        &[SuggestExpr::ReplaceFirst {
            pattern: "alis",
            replacement: "",
        }],
    ),
    (
        "[iI]ntrovertisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "isch",
            replacement: "iert",
        }],
    ),
    (
        "[aA]miert(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "mi",
            replacement: "rmi",
        }],
    ),
    (
        "[vV]ersiehrt(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "h",
            replacement: "",
        }],
    ),
    (
        "[dD]urchsichtbar(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "bar",
            replacement: "ig",
        }],
    ),
    (
        "[oO]ffensichtig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ig",
            replacement: "lich",
        }],
    ),
    (
        "[zZ]urverfühgung",
        &[SuggestExpr::ReplaceFirst {
            pattern: "verfühgung",
            replacement: " Verfügung",
        }],
    ),
    (
        "[sS]pendeangebot(e[ns]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[sS]pende",
            replacement: "Spenden",
        }],
    ),
    (
        "gahrnichts?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "gahr",
            replacement: "gar ",
        }],
    ),
    (
        "[aA]ugensichtlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "sicht",
            replacement: "schein",
        }],
    ),
    (
        "[lL]eidensvoll(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ens",
            replacement: "",
        }],
    ),
    (
        "[bB]ewusstlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "",
        }],
    ),
    (
        "[vV]erschmerzlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "bar",
        }],
    ),
    (
        "Krankenbruders?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "bruder",
            replacement: "pfleger",
        }],
    ),
    (
        "Krankenbrüdern?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "brüder",
            replacement: "pfleger",
        }],
    ),
    (
        "Lan-(Kabel[ns]?|Verbindung)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Lan",
            replacement: "LAN",
        }],
    ),
    (
        "[sS]epalastschriftmandat(s|en?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[sS]epal",
            replacement: "SEPA-L",
        }],
    ),
    (
        "Pinn?eingaben?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Pinn?e",
            replacement: "PIN-E",
        }],
    ),
    (
        "[sS]imkarten?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[sS]imk",
            replacement: "SIM-K",
        }],
    ),
    (
        "[vV]orsich(geht|gehen|ging(en)?|gegangen)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "sich",
            replacement: " sich ",
        }],
    ),
    (
        "mitsich(bringt|bringen|brachten?|gebracht)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "sich",
            replacement: " sich ",
        }],
    ),
    (
        "[ck]arnivorisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[ck]arnivorisch",
            replacement: "karnivor",
        }],
    ),
    (
        "[pP]erfektest(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "est",
            replacement: "",
        }],
    ),
    (
        "[gG]leichtig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "tig",
            replacement: "zeitig",
        }],
    ),
    (
        "[uU]n(her)?vorgesehen(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "(her)?vor",
            replacement: "vorher",
        }],
    ),
    (
        "([cC]orona|[gG]rippe)viruss?es",
        &[SuggestExpr::ReplaceFirst {
            pattern: "viruss?es",
            replacement: "virus",
        }],
    ),
    (
        "Zaubererin(nen)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "er",
            replacement: "",
        }],
    ),
    (
        "Second-Hand-L[äa]dens?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Second-Hand-L",
            replacement: "Secondhandl",
        }],
    ),
    (
        "Second-Hand-Shops?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Second-Hand-S",
            replacement: "Secondhands",
        }],
    ),
    (
        "[mM]editerranisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "isch",
            replacement: "",
        }],
    ),
    (
        "interplementier(s?t|en?)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "inter",
            replacement: "im",
        }],
    ),
    (
        "[hH]ochalterlich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "alter",
            replacement: "mittelalter",
        }],
    ),
    (
        "posiniert(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "si",
            replacement: "sitio",
        }],
    ),
    (
        "[rR]ussophobisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "isch",
            replacement: "",
        }],
    ),
    (
        "[uU]nsachmä(ß|ss?)ig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "mä(ß|ss?)ig",
            replacement: "gemäß",
        }],
    ),
    (
        "[mM]odernisch(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "isch",
            replacement: "",
        }],
    ),
    (
        "intapretation(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "inta",
            replacement: "Inter",
        }],
    ),
    (
        "[rR]ethorikkurs(e[ns]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "eth",
            replacement: "het",
        }],
    ),
    (
        "[uU]nterschreibungsfähig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "schreibung",
            replacement: "schrift",
        }],
    ),
    (
        "[eE]rrorier(en?|t(e[mnrs]?)?|st)",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ror",
            replacement: "u",
        }],
    ),
    (
        "malediert(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "malediert",
            replacement: "malträtiert",
        }],
    ),
    (
        "maletriert(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "maletriert",
            replacement: "malträtiert",
        }],
    ),
    (
        "Ausbildereignerprüfung(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "eigner",
            replacement: "eignungs",
        }],
    ),
    (
        "abtrakt(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ab",
            replacement: "abs",
        }],
    ),
    (
        "unerfolgreich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "unerfolgreich",
            replacement: "erfolglos",
        }],
    ),
    (
        "[bB]attalion(en?|s)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[bB]attalion",
            replacement: "Bataillon",
        }],
    ),
    (
        "[bB]esuchungsverbot(e[ns]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ung",
            replacement: "",
        }],
    ),
    (
        "spätrig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "rig",
            replacement: "er",
        }],
    ),
    (
        "angehangene[mnrs]?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "hangen",
            replacement: "hängt",
        }],
    ),
    (
        "[ck]amel[ie]onhaft(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "[ck]am[ie]lion",
            replacement: "chamäleon",
        }],
    ),
    (
        "[wW]idersprüchig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ig",
            replacement: "lich",
        }],
    ),
    (
        "[fF]austig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "austig",
            replacement: "austdick",
        }],
    ),
    (
        "Belastungsekgs?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ekg",
            replacement: "-EKG",
        }],
    ),
    (
        "gehardcode[dt](e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "gehardcode",
            replacement: "hartkodier",
        }],
    ),
    (
        "hardgecode[dt](e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "gehardcode",
            replacement: "hartkodier",
        }],
    ),
    (
        "Flektion(en)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Flektion",
            replacement: "Flexion",
        }],
    ),
    (
        "Off-[Ss]hore-[A-Z].+",
        &[SuggestExpr::ReplaceFirst {
            pattern: "Off-[Ss]hore-",
            replacement: "Offshore",
        }],
    ),
    (
        "[Vv]ermißt.*",
        &[SuggestExpr::ReplaceFirst {
            pattern: "ermißt",
            replacement: "ermisst",
        }],
    ),
    (
        "EUfeindlich.*",
        &[SuggestExpr::ReplaceFirst {
            pattern: "EUfeindlich",
            replacement: "EU-feindlich",
        }],
    ),
    ("Deis", &[SuggestExpr::Lit("Dies")]),
    ("fr", &[SuggestExpr::Lit("für")]),
    (
        "abe",
        &[
            SuggestExpr::Lit("habe"),
            SuggestExpr::Lit("aber"),
            SuggestExpr::Lit("ab"),
        ],
    ),
    (
        "Oster",
        &[SuggestExpr::Lit("Ostern"), SuggestExpr::Lit("Osten")],
    ),
    (
        "richen",
        &[
            SuggestExpr::Lit("riechen"),
            SuggestExpr::Lit("reichen"),
            SuggestExpr::Lit("richten"),
        ],
    ),
    (
        "deien",
        &[SuggestExpr::Lit("deine"), SuggestExpr::Lit("dein")],
    ),
    (
        "meien",
        &[
            SuggestExpr::Lit("meine"),
            SuggestExpr::Lit("mein"),
            SuggestExpr::Lit("meinen"),
        ],
    ),
    (
        "berüht",
        &[
            SuggestExpr::Lit("berühmt"),
            SuggestExpr::Lit("berührt"),
            SuggestExpr::Lit("bemüht"),
        ],
    ),
    (
        "herlich",
        &[SuggestExpr::Lit("ehrlich"), SuggestExpr::Lit("herrlich")],
    ),
    (
        "erzeiht",
        &[SuggestExpr::Lit("erzieht"), SuggestExpr::Lit("verzeiht")],
    ),
    (
        "schalfen",
        &[
            SuggestExpr::Lit("schlafen"),
            SuggestExpr::Lit("schaffen"),
            SuggestExpr::Lit("scharfen"),
        ],
    ),
    (
        "Anfage",
        &[SuggestExpr::Lit("Anfrage"), SuggestExpr::Lit("Anlage")],
    ),
    (
        "gehör",
        &[
            SuggestExpr::Lit("gehört"),
            SuggestExpr::Lit("Gehör"),
            SuggestExpr::Lit("gehöre"),
        ],
    ),
    (
        "Sep",
        &[
            SuggestExpr::Lit("Sept."),
            SuggestExpr::Lit("Sepp"),
            SuggestExpr::Lit("September"),
            SuggestExpr::Lit("Separator"),
            SuggestExpr::Lit("Sei"),
        ],
    ),
    ("Formulares", &[SuggestExpr::Lit("Formulars")]),
    ("Danl", &[SuggestExpr::Lit("Dank")]),
    ("umbennen", &[SuggestExpr::Lit("umbenennen")]),
    ("bevorzugs", &[SuggestExpr::Lit("bevorzugst")]),
    ("einhergend", &[SuggestExpr::Lit("einhergehend")]),
    (
        "dos",
        &[
            SuggestExpr::Lit("das"),
            SuggestExpr::Lit("des"),
            SuggestExpr::Lit("DOS"),
            SuggestExpr::Lit("DoS"),
        ],
    ),
    (
        "mch",
        &[
            SuggestExpr::Lit("mich"),
            SuggestExpr::Lit("ich"),
            SuggestExpr::Lit("ach"),
        ],
    ),
    (
        "Ihc",
        &[
            SuggestExpr::Lit("Ich"),
            SuggestExpr::Lit("Ihr"),
            SuggestExpr::Lit("Ihm"),
        ],
    ),
    (
        "ihc",
        &[
            SuggestExpr::Lit("ich"),
            SuggestExpr::Lit("ihr"),
            SuggestExpr::Lit("ihm"),
        ],
    ),
    ("ioch", &[SuggestExpr::Lit("ich")]),
    ("of", &[SuggestExpr::Lit("oft")]),
    (
        "mi",
        &[
            SuggestExpr::Lit("im"),
            SuggestExpr::Lit("mit"),
            SuggestExpr::Lit("mir"),
        ],
    ),
    (
        "wier",
        &[
            SuggestExpr::Lit("wie"),
            SuggestExpr::Lit("wir"),
            SuggestExpr::Lit("vier"),
            SuggestExpr::Lit("hier"),
            SuggestExpr::Lit("wer"),
        ],
    ),
    (
        "ander",
        &[
            SuggestExpr::Lit("an der"),
            SuggestExpr::Lit("andere"),
            SuggestExpr::Lit("änder"),
            SuggestExpr::Lit("anders"),
        ],
    ),
    ("ech", &[SuggestExpr::Lit("euch"), SuggestExpr::Lit("ich")]),
    (
        "letzt",
        &[SuggestExpr::Lit("letzte"), SuggestExpr::Lit("jetzt")],
    ),
    (
        "beu",
        &[
            SuggestExpr::Lit("bei"),
            SuggestExpr::Lit("peu"),
            SuggestExpr::Lit("neu"),
        ],
    ),
    (
        "darn",
        &[
            SuggestExpr::Lit("daran"),
            SuggestExpr::Lit("darin"),
            SuggestExpr::Lit("dann"),
            SuggestExpr::Lit("dar"),
        ],
    ),
    (
        "zwie",
        &[
            SuggestExpr::Lit("zwei"),
            SuggestExpr::Lit("wie"),
            SuggestExpr::Lit("sie"),
            SuggestExpr::Lit("sowie"),
        ],
    ),
    (
        "gebten",
        &[
            SuggestExpr::Lit("gebeten"),
            SuggestExpr::Lit("gaben"),
            SuggestExpr::Lit("geboten"),
            SuggestExpr::Lit("gelten"),
        ],
    ),
    (
        "dea",
        &[
            SuggestExpr::Lit("der"),
            SuggestExpr::Lit("den"),
            SuggestExpr::Lit("des"),
            SuggestExpr::Lit("dem"),
        ],
    ),
    (
        "neune",
        &[
            SuggestExpr::Lit("neuen"),
            SuggestExpr::Lit("neue"),
            SuggestExpr::Lit("Neune"),
        ],
    ),
    (
        "geren",
        &[
            SuggestExpr::Lit("gegen"),
            SuggestExpr::Lit("gerne"),
            SuggestExpr::Lit("gären"),
        ],
    ),
    (
        "wuerden",
        &[SuggestExpr::Lit("würden"), SuggestExpr::Lit("wurden")],
    ),
    (
        "wuerde",
        &[SuggestExpr::Lit("würde"), SuggestExpr::Lit("wurde")],
    ),
    (
        "git",
        &[
            SuggestExpr::Lit("gut"),
            SuggestExpr::Lit("gibt"),
            SuggestExpr::Lit("gilt"),
            SuggestExpr::Lit("mit"),
        ],
    ),
    (
        "voher",
        &[
            SuggestExpr::Lit("vorher"),
            SuggestExpr::Lit("woher"),
            SuggestExpr::Lit("hoher"),
        ],
    ),
    (
        "hst",
        &[
            SuggestExpr::Lit("hast"),
            SuggestExpr::Lit("ist"),
            SuggestExpr::Lit("hat"),
        ],
    ),
    (
        "Hst",
        &[
            SuggestExpr::Lit("Hast"),
            SuggestExpr::Lit("Ist"),
            SuggestExpr::Lit("Hat"),
        ],
    ),
    (
        "herlichen",
        &[
            SuggestExpr::Lit("herzlichen"),
            SuggestExpr::Lit("ehrlichen"),
            SuggestExpr::Lit("herrlichen"),
        ],
    ),
    (
        "Herlichen",
        &[
            SuggestExpr::Lit("Herzlichen"),
            SuggestExpr::Lit("Ehrlichen"),
            SuggestExpr::Lit("Herrlichen"),
        ],
    ),
    (
        "herliche",
        &[
            SuggestExpr::Lit("herzliche"),
            SuggestExpr::Lit("ehrliche"),
            SuggestExpr::Lit("herrliche"),
        ],
    ),
    (
        "Herliche",
        &[
            SuggestExpr::Lit("Herzliche"),
            SuggestExpr::Lit("Ehrliche"),
            SuggestExpr::Lit("Herrliche"),
        ],
    ),
    (
        "it",
        &[
            SuggestExpr::Lit("ist"),
            SuggestExpr::Lit("IT"),
            SuggestExpr::Lit("in"),
            SuggestExpr::Lit("im"),
        ],
    ),
    (
        "ads",
        &[
            SuggestExpr::Lit("das"),
            SuggestExpr::Lit("ADS"),
            SuggestExpr::Lit("Ads"),
            SuggestExpr::Lit("als"),
            SuggestExpr::Lit("aus"),
        ],
    ),
    (
        "hats",
        &[
            SuggestExpr::Lit("hat es"),
            SuggestExpr::Lit("hast"),
            SuggestExpr::Lit("hat"),
        ],
    ),
    (
        "Hats",
        &[
            SuggestExpr::Lit("Hat es"),
            SuggestExpr::Lit("Hast"),
            SuggestExpr::Lit("Hat"),
        ],
    ),
    (
        "och",
        &[
            SuggestExpr::Lit("ich"),
            SuggestExpr::Lit("noch"),
            SuggestExpr::Lit("doch"),
        ],
    ),
    (
        "bein",
        &[
            SuggestExpr::Lit("Bein"),
            SuggestExpr::Lit("beim"),
            SuggestExpr::Lit("ein"),
            SuggestExpr::Lit("bei"),
        ],
    ),
    (
        "ser",
        &[
            SuggestExpr::Lit("der"),
            SuggestExpr::Lit("sehr"),
            SuggestExpr::Lit("er"),
            SuggestExpr::Lit("sei"),
        ],
    ),
    (
        "Monatg",
        &[
            SuggestExpr::Lit("Montag"),
            SuggestExpr::Lit("Monate"),
            SuggestExpr::Lit("Monats"),
        ],
    ),
    (
        "leiben",
        &[
            SuggestExpr::Lit("lieben"),
            SuggestExpr::Lit("bleiben"),
            SuggestExpr::Lit("leben"),
        ],
    ),
    (
        "grad",
        &[
            SuggestExpr::Lit("grade"),
            SuggestExpr::Lit("Grad"),
            SuggestExpr::Lit("gerade"),
        ],
    ),
    (
        "dnn",
        &[
            SuggestExpr::Lit("dann"),
            SuggestExpr::Lit("denn"),
            SuggestExpr::Lit("den"),
        ],
    ),
    (
        "vn",
        &[
            SuggestExpr::Lit("von"),
            SuggestExpr::Lit("an"),
            SuggestExpr::Lit("in"),
        ],
    ),
    (
        "sin",
        &[
            SuggestExpr::Lit("ein"),
            SuggestExpr::Lit("sind"),
            SuggestExpr::Lit("sie"),
            SuggestExpr::Lit("in"),
        ],
    ),
    (
        "schein",
        &[
            SuggestExpr::Lit("scheine"),
            SuggestExpr::Lit("Schein"),
            SuggestExpr::Lit("scheint"),
            SuggestExpr::Lit("schien"),
        ],
    ),
    (
        "wil",
        &[
            SuggestExpr::Lit("will"),
            SuggestExpr::Lit("wie"),
            SuggestExpr::Lit("weil"),
            SuggestExpr::Lit("wir"),
        ],
    ),
    (
        "Ihen",
        &[
            SuggestExpr::Lit("Ihren"),
            SuggestExpr::Lit("Ihnen"),
            SuggestExpr::Lit("Ihn"),
            SuggestExpr::Lit("Iren"),
        ],
    ),
    ("Iher", &[SuggestExpr::Lit("Ihre"), SuggestExpr::Lit("Ihr")]),
    (
        "neunen",
        &[SuggestExpr::Lit("neuen"), SuggestExpr::Lit("neunten")],
    ),
    (
        "tole",
        &[SuggestExpr::Lit("tolle"), SuggestExpr::Lit("tote")],
    ),
    (
        "tolen",
        &[SuggestExpr::Lit("tollen"), SuggestExpr::Lit("toten")],
    ),
    (
        "wiel",
        &[
            SuggestExpr::Lit("weil"),
            SuggestExpr::Lit("wie"),
            SuggestExpr::Lit("viel"),
        ],
    ),
    (
        "brauchts",
        &[
            SuggestExpr::Lit("braucht es"),
            SuggestExpr::Lit("brauchst"),
            SuggestExpr::Lit("braucht"),
        ],
    ),
    (
        "schöen",
        &[SuggestExpr::Lit("schönen"), SuggestExpr::Lit("schön")],
    ),
    (
        "ihne",
        &[SuggestExpr::Lit("ihn"), SuggestExpr::Lit("ihnen")],
    ),
    (
        "af",
        &[
            SuggestExpr::Lit("auf"),
            SuggestExpr::Lit("an"),
            SuggestExpr::Lit("an"),
            SuggestExpr::Lit("als"),
        ],
    ),
    (
        "mächte",
        &[SuggestExpr::Lit("möchte"), SuggestExpr::Lit("Mächte")],
    ),
    (
        "öffen",
        &[SuggestExpr::Lit("öffnen"), SuggestExpr::Lit("offen")],
    ),
    (
        "fernsehgucken",
        &[
            SuggestExpr::Lit("fernsehen"),
            SuggestExpr::Lit("Fernsehen gucken"),
        ],
    ),
    (
        "Mien",
        &[
            SuggestExpr::Lit("Mein"),
            SuggestExpr::Lit("Wien"),
            SuggestExpr::Lit("Miene"),
        ],
    ),
    (
        "abgeharkt",
        &[SuggestExpr::Lit("abgehakt"), SuggestExpr::Lit("abgehackt")],
    ),
    (
        "beiten",
        &[SuggestExpr::Lit("beiden"), SuggestExpr::Lit("bieten")],
    ),
    (
        "ber",
        &[
            SuggestExpr::Lit("über"),
            SuggestExpr::Lit("per"),
            SuggestExpr::Lit("der"),
            SuggestExpr::Lit("BER"),
        ],
    ),
    (
        "ehr",
        &[
            SuggestExpr::Lit("eher"),
            SuggestExpr::Lit("mehr"),
            SuggestExpr::Lit("sehr"),
            SuggestExpr::Lit("er"),
        ],
    ),
    (
        "Meien",
        &[
            SuggestExpr::Lit("Meine"),
            SuggestExpr::Lit("Meinen"),
            SuggestExpr::Lit("Mein"),
            SuggestExpr::Lit("Medien"),
        ],
    ),
    (
        "neus",
        &[
            SuggestExpr::Lit("neues"),
            SuggestExpr::Lit("neue"),
            SuggestExpr::Lit("neu"),
        ],
    ),
    (
        "Sunden",
        &[
            SuggestExpr::Lit("Sünden"),
            SuggestExpr::Lit("Stunden"),
            SuggestExpr::Lit("Kunden"),
        ],
    ),
    (
        "Bitt",
        &[
            SuggestExpr::Lit("Bitte"),
            SuggestExpr::Lit("Bett"),
            SuggestExpr::Lit("Bist"),
        ],
    ),
    ("bst", &[SuggestExpr::Lit("bist"), SuggestExpr::Lit("ist")]),
    (
        "ds",
        &[
            SuggestExpr::Lit("des"),
            SuggestExpr::Lit("das"),
            SuggestExpr::Lit("es"),
        ],
    ),
    (
        "mn",
        &[
            SuggestExpr::Lit("man"),
            SuggestExpr::Lit("in"),
            SuggestExpr::Lit("an"),
        ],
    ),
    (
        "hilt",
        &[
            SuggestExpr::Lit("gilt"),
            SuggestExpr::Lit("hilft"),
            SuggestExpr::Lit("hielt"),
            SuggestExpr::Lit("hält"),
        ],
    ),
    (
        "nei",
        &[
            SuggestExpr::Lit("bei"),
            SuggestExpr::Lit("nie"),
            SuggestExpr::Lit("ein"),
            SuggestExpr::Lit("neu"),
        ],
    ),
    (
        "riesen",
        &[
            SuggestExpr::Lit("riesigen"),
            SuggestExpr::Lit("diesen"),
            SuggestExpr::Lit("Riesen"),
            SuggestExpr::Lit("reisen"),
        ],
    ),
    (
        "geduld",
        &[SuggestExpr::Lit("Geduld"), SuggestExpr::Lit("gedulde")],
    ),
    (
        "bits",
        &[
            SuggestExpr::Lit("bist"),
            SuggestExpr::Lit("bis"),
            SuggestExpr::Lit("Bits"),
        ],
    ),
    (
        "aheb",
        &[SuggestExpr::Lit("habe"), SuggestExpr::Lit("aber")],
    ),
    (
        "versand",
        &[SuggestExpr::Lit("versandt"), SuggestExpr::Lit("Versand")],
    ),
    (
        "os",
        &[
            SuggestExpr::Lit("so"),
            SuggestExpr::Lit("es"),
            SuggestExpr::Lit("OS"),
        ],
    ),
    (
        "Kriese",
        &[SuggestExpr::Lit("Krise"), SuggestExpr::Lit("Kreise")],
    ),
    (
        "Kriesen",
        &[SuggestExpr::Lit("Krisen"), SuggestExpr::Lit("Kreisen")],
    ),
    (
        "aufteil",
        &[
            SuggestExpr::Lit("aufteile"),
            SuggestExpr::Lit("aufteilt"),
            SuggestExpr::Lit("auf Teil"),
        ],
    ),
    (
        "fürn",
        &[
            SuggestExpr::Lit("für ein"),
            SuggestExpr::Lit("für den"),
            SuggestExpr::Lit("für"),
            SuggestExpr::Lit("fürs"),
            SuggestExpr::Lit("fern"),
        ],
    ),
    (
        "Aliegen",
        &[SuggestExpr::Lit("Anliegen"), SuggestExpr::Lit("Fliegen")],
    ),
    ("gaz", &[SuggestExpr::Lit("ganz"), SuggestExpr::Lit("gab")]),
    (
        "vllt",
        &[SuggestExpr::Lit("vielleicht"), SuggestExpr::Lit("vllt.")],
    ),
    (
        "rauch",
        &[SuggestExpr::Lit("Rauch"), SuggestExpr::Lit("rauche")],
    ),
    (
        "liebs",
        &[
            SuggestExpr::Lit("liebe es"),
            SuggestExpr::Lit("liebes"),
            SuggestExpr::Lit("liebe"),
        ],
    ),
    (
        "as",
        &[
            SuggestExpr::Lit("aß"),
            SuggestExpr::Lit("das"),
            SuggestExpr::Lit("als"),
        ],
    ),
    (
        "bekommste",
        &[
            SuggestExpr::Lit("bekommst du"),
            SuggestExpr::Lit("bekommst"),
        ],
    ),
    (
        "under",
        &[SuggestExpr::Lit("unser"), SuggestExpr::Lit("unter")],
    ),
    ("dis", &[SuggestExpr::Lit("die"), SuggestExpr::Lit("dies")]),
    (
        "veil",
        &[
            SuggestExpr::Lit("viel"),
            SuggestExpr::Lit("weil"),
            SuggestExpr::Lit("teil"),
        ],
    ),
    (
        "mak",
        &[
            SuggestExpr::Lit("mag"),
            SuggestExpr::Lit("mak"),
            SuggestExpr::Lit("lag"),
        ],
    ),
    (
        "daum",
        &[
            SuggestExpr::Lit("da um"),
            SuggestExpr::Lit("darum"),
            SuggestExpr::Lit("kaum"),
            SuggestExpr::Lit("Raum"),
        ],
    ),
    (
        "gechickt",
        &[SuggestExpr::Lit("geschickt"), SuggestExpr::Lit("gecheckt")],
    ),
    (
        "gibs",
        &[SuggestExpr::Lit("gib es"), SuggestExpr::Lit("gibst")],
    ),
    (
        "Gibs",
        &[
            SuggestExpr::Lit("Gib es"),
            SuggestExpr::Lit("Gibst"),
            SuggestExpr::Lit("Gips"),
        ],
    ),
    (
        "Gutan",
        &[
            SuggestExpr::Lit("Gut an"),
            SuggestExpr::Lit("Guten"),
            SuggestExpr::Lit("Sudan"),
        ],
    ),
    (
        "vol",
        &[
            SuggestExpr::Lit("von"),
            SuggestExpr::Lit("vom"),
            SuggestExpr::Lit("voll"),
            SuggestExpr::Lit("vor"),
        ],
    ),
    (
        "einzulogen",
        &[
            SuggestExpr::Lit("einzuloggen"),
            SuggestExpr::Lit("einzulegen"),
        ],
    ),
    (
        "Liben",
        &[
            SuggestExpr::Lit("Lieben"),
            SuggestExpr::Lit("Leben"),
            SuggestExpr::Lit("Libyen"),
            SuggestExpr::Lit("Ligen"),
        ],
    ),
    (
        "bruchen",
        &[
            SuggestExpr::Lit("brauchen"),
            SuggestExpr::Lit("brachen"),
            SuggestExpr::Lit("brechen"),
        ],
    ),
    (
        "gerner",
        &[
            SuggestExpr::Lit("gern"),
            SuggestExpr::Lit("gern er"),
            SuggestExpr::Lit("ferner"),
        ],
    ),
    (
        "krige",
        &[SuggestExpr::Lit("kriege"), SuggestExpr::Lit("krieg")],
    ),
    (
        "Geschnek",
        &[SuggestExpr::Lit("Geschenk"), SuggestExpr::Lit("Geschmack")],
    ),
    (
        "meinste",
        &[
            SuggestExpr::Lit("meiste"),
            SuggestExpr::Lit("feinste"),
            SuggestExpr::Lit("meinte"),
            SuggestExpr::Lit("meinst du"),
        ],
    ),
    (
        "Meinste",
        &[
            SuggestExpr::Lit("Meiste"),
            SuggestExpr::Lit("Feinste"),
            SuggestExpr::Lit("Meinte"),
            SuggestExpr::Lit("Meinst du"),
        ],
    ),
    (
        "Telefones",
        &[SuggestExpr::Lit("Telefons"), SuggestExpr::Lit("Telefone")],
    ),
    (
        "wusten",
        &[SuggestExpr::Lit("wussten"), SuggestExpr::Lit("wüsten")],
    ),
    (
        "geschlaffen",
        &[
            SuggestExpr::Lit("geschlafen"),
            SuggestExpr::Lit("geschaffen"),
            SuggestExpr::Lit("geschliffen"),
        ],
    ),
    (
        "Feb",
        &[
            SuggestExpr::Lit("Feb."),
            SuggestExpr::Lit("Web"),
            SuggestExpr::Lit("Pep"),
            SuggestExpr::Lit("Geb"),
            SuggestExpr::Lit("Gäb"),
        ],
    ),
    (
        "Mogen",
        &[
            SuggestExpr::Lit("Mögen"),
            SuggestExpr::Lit("Morgen"),
            SuggestExpr::Lit("Zogen"),
        ],
    ),
    (
        "Dak",
        &[
            SuggestExpr::Lit("Dank"),
            SuggestExpr::Lit("Das"),
            SuggestExpr::Lit("Dock"),
        ],
    ),
    ("Dake", &[SuggestExpr::Lit("Danke")]),
    ("dake", &[SuggestExpr::Lit("danke")]),
    (
        "Laola",
        &[
            SuggestExpr::Lit("La-Ola"),
            SuggestExpr::Lit("Paola"),
            SuggestExpr::Lit("Layla"),
            SuggestExpr::Lit("Lala"),
        ],
    ),
    (
        "Laolas",
        &[
            SuggestExpr::Lit("La-Olas"),
            SuggestExpr::Lit("Paolas"),
            SuggestExpr::Lit("Laylas"),
        ],
    ),
    (
        "übernohmen",
        &[
            SuggestExpr::Lit("übernehmen"),
            SuggestExpr::Lit("übernommen"),
        ],
    ),
    (
        "augeschlossen",
        &[
            SuggestExpr::Lit("ausgeschlossen"),
            SuggestExpr::Lit("angeschlossen"),
        ],
    ),
    ("Akteures", &[SuggestExpr::Lit("Akteurs")]),
    ("popup", &[SuggestExpr::Lit("Pop-up")]),
    ("Gedaken", &[SuggestExpr::Lit("Gedanken")]),
    ("Wiso", &[SuggestExpr::Lit("Wieso")]),
    ("gebs", &[SuggestExpr::Lit("gebe es")]),
    ("angefordet", &[SuggestExpr::Lit("angefordert")]),
    ("onlein", &[SuggestExpr::Lit("online")]),
    ("Studen", &[SuggestExpr::Lit("Stunden")]),
    ("weils", &[SuggestExpr::Lit("weil es")]),
    ("unterscheid", &[SuggestExpr::Lit("Unterschied")]),
    ("mags", &[SuggestExpr::Lit("mag es")]),
    ("abzügl", &[SuggestExpr::Lit("abzgl")]),
    ("as", &[SuggestExpr::Lit("das")]),
    ("gefielts", &[SuggestExpr::Lit("gefielt es")]),
    ("gefiels", &[SuggestExpr::Lit("gefielt es")]),
    ("gefällts", &[SuggestExpr::Lit("gefällt es")]),
    ("nummer", &[SuggestExpr::Lit("Nummer")]),
    ("mitgetielt", &[SuggestExpr::Lit("mitgeteilt")]),
    ("Artal", &[SuggestExpr::Lit("Ahrtal")]),
    ("wuste", &[SuggestExpr::Lit("wusste")]),
    ("Kuden", &[SuggestExpr::Lit("Kunden")]),
    ("austehenden", &[SuggestExpr::Lit("ausstehenden")]),
    ("eingelogt", &[SuggestExpr::Lit("eingeloggt")]),
    ("kapput", &[SuggestExpr::Lit("kaputt")]),
    ("geeehrte", &[SuggestExpr::Lit("geehrte")]),
    ("geeehrter", &[SuggestExpr::Lit("geehrter")]),
    ("startup", &[SuggestExpr::Lit("Start-up")]),
    ("startups", &[SuggestExpr::Lit("Start-ups")]),
    ("Biite", &[SuggestExpr::Lit("Bitte")]),
    ("Gutn", &[SuggestExpr::Lit("Guten")]),
    ("gutn", &[SuggestExpr::Lit("guten")]),
    ("Ettiket", &[SuggestExpr::Lit("Etikett")]),
    ("iht", &[SuggestExpr::Lit("ihr")]),
    ("ligt", &[SuggestExpr::Lit("liegt")]),
    ("gester", &[SuggestExpr::Lit("gestern")]),
    ("veraten", &[SuggestExpr::Lit("verraten")]),
    ("dienem", &[SuggestExpr::Lit("deinem")]),
    ("Bite", &[SuggestExpr::Lit("Bitte")]),
    ("Serh", &[SuggestExpr::Lit("Sehr")]),
    ("serh", &[SuggestExpr::Lit("sehr")]),
    ("fargen", &[SuggestExpr::Lit("fragen")]),
    ("abrechen", &[SuggestExpr::Lit("abbrechen")]),
    ("aufzeichen", &[SuggestExpr::Lit("aufzeichnen")]),
    ("Geraet", &[SuggestExpr::Lit("Gerät")]),
    ("Geraets", &[SuggestExpr::Lit("Geräts")]),
    ("Geraete", &[SuggestExpr::Lit("Geräte")]),
    ("Geraeten", &[SuggestExpr::Lit("Geräten")]),
    ("Fals", &[SuggestExpr::Lit("Falls")]),
    ("soche", &[SuggestExpr::Lit("solche")]),
    ("verückt", &[SuggestExpr::Lit("verrückt")]),
    ("austellen", &[SuggestExpr::Lit("ausstellen")]),
    (
        "klapt",
        &[SuggestExpr::Lit("klappt"), SuggestExpr::Lit("klagt")],
    ),
    (
        "denks",
        &[
            SuggestExpr::Lit("denkst"),
            SuggestExpr::Lit("denkt"),
            SuggestExpr::Lit("denke"),
            SuggestExpr::Lit("denk"),
        ],
    ),
    ("geerhte", &[SuggestExpr::Lit("geehrte")]),
    ("geerte", &[SuggestExpr::Lit("geehrte")]),
    ("gehn", &[SuggestExpr::Lit("gehen")]),
    ("Spß", &[SuggestExpr::Lit("Spaß")]),
    ("kanst", &[SuggestExpr::Lit("kannst")]),
    ("fregen", &[SuggestExpr::Lit("fragen")]),
    ("Bingerloch", &[SuggestExpr::Lit("Binger Loch")]),
    (
        "[nN]or[dt]rh?einwest(f|ph)alen",
        &[SuggestExpr::Lit("Nordrhein-Westfalen")],
    ),
    ("abzusolvieren", &[SuggestExpr::Lit("zu absolvieren")]),
    ("Schutzfließ", &[SuggestExpr::Lit("Schutzvlies")]),
    ("Simlock", &[SuggestExpr::Lit("SIM-Lock")]),
    ("fäschungen", &[SuggestExpr::Lit("Fälschungen")]),
    ("Weinverköstigung", &[SuggestExpr::Lit("Weinverkostung")]),
    ("vertag", &[SuggestExpr::Lit("Vertrag")]),
    ("geauessert", &[SuggestExpr::Lit("geäußert")]),
    ("gefäh?ten", &[SuggestExpr::Lit("Gefährten")]),
    ("gefäh?te", &[SuggestExpr::Lit("Gefährte")]),
    ("immenoch", &[SuggestExpr::Lit("immer noch")]),
    ("sevice", &[SuggestExpr::Lit("Service")]),
    ("verhälst", &[SuggestExpr::Lit("verhältst")]),
    ("[sS]äusche", &[SuggestExpr::Lit("Seuche")]),
    ("Schalottenburg", &[SuggestExpr::Lit("Charlottenburg")]),
    ("senora", &[SuggestExpr::Lit("Señora")]),
    ("widerrum", &[SuggestExpr::Lit("wiederum")]),
    ("[dD]epp?risonen", &[SuggestExpr::Lit("Depressionen")]),
    ("Defribilator", &[SuggestExpr::Lit("Defibrillator")]),
    ("Defribilatoren", &[SuggestExpr::Lit("Defibrillatoren")]),
    ("SwatchGroup", &[SuggestExpr::Lit("Swatch Group")]),
    ("achtungslo[ßs]", &[SuggestExpr::Lit("achtlos")]),
    ("Boomerang", &[SuggestExpr::Lit("Bumerang")]),
    ("Boomerangs", &[SuggestExpr::Lit("Bumerangs")]),
    (
        "Lg",
        &[SuggestExpr::Lit("LG"), SuggestExpr::Lit("Liebe Grüße")],
    ),
    ("gildet", &[SuggestExpr::Lit("gilt")]),
    ("gleitete", &[SuggestExpr::Lit("glitt")]),
    ("gleiteten", &[SuggestExpr::Lit("glitten")]),
    ("Standbay", &[SuggestExpr::Lit("Stand-by")]),
    ("[vV]ollkommnung", &[SuggestExpr::Lit("Vervollkommnung")]),
    ("femist", &[SuggestExpr::Lit("vermisst")]),
    ("stantepede", &[SuggestExpr::Lit("stante pede")]),
    ("[kK]ostarika", &[SuggestExpr::Lit("Costa Rica")]),
    ("[kK]ostarikas", &[SuggestExpr::Lit("Costa Ricas")]),
    ("[aA]uthenzität", &[SuggestExpr::Lit("Authentizität")]),
    ("anlässig", &[SuggestExpr::Lit("anlässlich")]),
    ("[sS]tieft", &[SuggestExpr::Lit("Stift")]),
    ("[Ii]nspruchnahme", &[SuggestExpr::Lit("Inanspruchnahme")]),
    (
        "höstwah?rsch[ea]inlich",
        &[SuggestExpr::Lit("höchstwahrscheinlich")],
    ),
    (
        "[aA]lterschbeschränkung",
        &[SuggestExpr::Lit("Altersbeschränkung")],
    ),
    ("[kK]unstoff", &[SuggestExpr::Lit("Kunststoff")]),
    ("[iI]nstergramm?", &[SuggestExpr::Lit("Instagram")]),
    ("fleicht", &[SuggestExpr::Lit("vielleicht")]),
    ("[eE]rartens", &[SuggestExpr::Lit("Erachtens")]),
    ("laufte", &[SuggestExpr::Lit("lief")]),
    ("lauften", &[SuggestExpr::Lit("liefen")]),
    ("malzeit", &[SuggestExpr::Lit("Mahlzeit")]),
    ("[wW]ahts?app", &[SuggestExpr::Lit("WhatsApp")]),
    (
        "[wW]elan",
        &[SuggestExpr::Lit("WLAN"), SuggestExpr::Lit("W-LAN")],
    ),
    ("Pinn", &[SuggestExpr::Lit("Pin"), SuggestExpr::Lit("PIN")]),
    (
        "Geldmachung",
        &[
            SuggestExpr::Lit("Geltendmachung"),
            SuggestExpr::Lit("Geldmacherei"),
        ],
    ),
    (
        "[uU]nstimm?ichkeiten",
        &[SuggestExpr::Lit("Unstimmigkeiten")],
    ),
    ("Teilnehmung", &[SuggestExpr::Lit("Teilnahme")]),
    ("Teilnehmungen", &[SuggestExpr::Lit("Teilnahmen")]),
    ("waser", &[SuggestExpr::Lit("Wasser")]),
    ("Bekennung", &[SuggestExpr::Lit("Bekenntnis")]),
    ("[hH]irar?chie", &[SuggestExpr::Lit("Hierarchie")]),
    ("Chr", &[SuggestExpr::Lit("Chr.")]),
    ("Tiefbaumt", &[SuggestExpr::Lit("Tiefbauamt")]),
    ("getäucht", &[SuggestExpr::Lit("getäuscht")]),
    ("[hH]ähme", &[SuggestExpr::Lit("Häme")]),
    (
        "Wochendruhezeiten",
        &[SuggestExpr::Lit("Wochenendruhezeiten")],
    ),
    ("Studiumplatzt?", &[SuggestExpr::Lit("Studienplatz")]),
    (
        "Permanent-Make-Up",
        &[SuggestExpr::Lit("Permanent-Make-up")],
    ),
    ("woltet", &[SuggestExpr::Lit("wolltet")]),
    ("Bäckei", &[SuggestExpr::Lit("Bäckerei")]),
    ("Bäckeien", &[SuggestExpr::Lit("Bäckereien")]),
    ("warmweis", &[SuggestExpr::Lit("warmweiß")]),
    ("kaltweis", &[SuggestExpr::Lit("kaltweiß")]),
    ("jez", &[SuggestExpr::Lit("jetzt")]),
    ("hendis", &[SuggestExpr::Lit("Handys")]),
    ("wie?derwarten", &[SuggestExpr::Lit("wider Erwarten")]),
    ("[eE]ntercott?e", &[SuggestExpr::Lit("Entrecôte")]),
    ("[eE]rwachtung", &[SuggestExpr::Lit("Erwartung")]),
    ("[aA]nung", &[SuggestExpr::Lit("Ahnung")]),
    (
        "[uU]nreimlichkeiten",
        &[SuggestExpr::Lit("Ungereimtheiten")],
    ),
    (
        "[uU]nangeneh?mlichkeiten",
        &[SuggestExpr::Lit("Unannehmlichkeiten")],
    ),
    ("Messy", &[SuggestExpr::Lit("Messie")]),
    ("Polover", &[SuggestExpr::Lit("Pullover")]),
    ("heilwegs", &[SuggestExpr::Lit("halbwegs")]),
    ("undsoweiter", &[SuggestExpr::Lit("und so weiter")]),
    (
        "Gladbeckerstrasse",
        &[SuggestExpr::Lit("Gladbecker Straße")],
    ),
    ("Bonnerstra(ß|ss)e", &[SuggestExpr::Lit("Bonner Straße")]),
    ("[bB]range", &[SuggestExpr::Lit("Branche")]),
    ("Gewebtrauma", &[SuggestExpr::Lit("Gewebetrauma")]),
    (
        "Ehrenamtpauschale",
        &[SuggestExpr::Lit("Ehrenamtspauschale")],
    ),
    ("Essenzubereitung", &[SuggestExpr::Lit("Essenszubereitung")]),
    ("[gG]eborgsamkeit", &[SuggestExpr::Lit("Geborgenheit")]),
    ("gekommt", &[SuggestExpr::Lit("gekommen")]),
    ("hinweißen", &[SuggestExpr::Lit("hinweisen")]),
    ("Importation", &[SuggestExpr::Lit("Import")]),
    ("lädest", &[SuggestExpr::Lit("lädst")]),
    ("Themabereich", &[SuggestExpr::Lit("Themenbereich")]),
    ("Werksresett", &[SuggestExpr::Lit("Werksreset")]),
    ("wiederfahren", &[SuggestExpr::Lit("widerfahren")]),
    ("wiederspiegelten", &[SuggestExpr::Lit("widerspiegelten")]),
    ("weicheinlich", &[SuggestExpr::Lit("wahrscheinlich")]),
    ("schnäpchen", &[SuggestExpr::Lit("Schnäppchen")]),
    ("Hinduist", &[SuggestExpr::Lit("Hindu")]),
    ("Hinduisten", &[SuggestExpr::Lit("Hindus")]),
    ("Konzeptierung", &[SuggestExpr::Lit("Konzipierung")]),
    ("Phyton", &[SuggestExpr::Lit("Python")]),
    ("nochnichtmals?", &[SuggestExpr::Lit("noch nicht einmal")]),
    ("Refelektion", &[SuggestExpr::Lit("Reflexion")]),
    ("Refelektionen", &[SuggestExpr::Lit("Reflexionen")]),
    ("[sS]chanse", &[SuggestExpr::Lit("Chance")]),
    (
        "nich",
        &[SuggestExpr::Lit("nicht"), SuggestExpr::Lit("noch")],
    ),
    (
        "Nich",
        &[SuggestExpr::Lit("Nicht"), SuggestExpr::Lit("Noch")],
    ),
    ("wat", &[SuggestExpr::Lit("was")]),
    ("[Ee][Ss]ports", &[SuggestExpr::Lit("E-Sports")]),
    ("gerelaunch(ed|t)", &[SuggestExpr::Lit("relauncht")]),
    ("Gerelaunch(ed|t)", &[SuggestExpr::Lit("Relauncht")]),
    ("Bowl", &[SuggestExpr::Lit("Bowle")]),
    ("Dark[Ww]eb", &[SuggestExpr::Lit("Darknet")]),
    ("Sachs?en-Anhal?t", &[SuggestExpr::Lit("Sachsen-Anhalt")]),
    ("[Ss]chalgen", &[SuggestExpr::Lit("schlagen")]),
    ("[Ss]chalge", &[SuggestExpr::Lit("schlage")]),
    (
        "[dD]eutsche?sprache",
        &[SuggestExpr::Lit("deutsche Sprache")],
    ),
    ("eigl", &[SuggestExpr::Lit("eigtl")]),
    ("ma", &[SuggestExpr::Lit("mal")]),
    ("leidete", &[SuggestExpr::Lit("litt")]),
    ("leidetest", &[SuggestExpr::Lit("littest")]),
    ("leideten", &[SuggestExpr::Lit("litten")]),
    ("Hoody", &[SuggestExpr::Lit("Hoodie")]),
    ("Hoodys", &[SuggestExpr::Lit("Hoodies")]),
    ("Staatsexam", &[SuggestExpr::Lit("Staatsexamen")]),
    ("Staatsexams", &[SuggestExpr::Lit("Staatsexamens")]),
    ("Exam", &[SuggestExpr::Lit("Examen")]),
    ("Exams", &[SuggestExpr::Lit("Examens")]),
    ("[Rr]eviewing", &[SuggestExpr::Lit("Review")]),
    ("[Bb]aldmöglich", &[SuggestExpr::Lit("baldmöglichst")]),
    ("[Bb]rudi", &[SuggestExpr::Lit("Bruder")]),
    (
        "ih",
        &[
            SuggestExpr::Lit("ich"),
            SuggestExpr::Lit("in"),
            SuggestExpr::Lit("im"),
            SuggestExpr::Lit("ah"),
        ],
    ),
    (
        "Ih",
        &[
            SuggestExpr::Lit("Ich"),
            SuggestExpr::Lit("In"),
            SuggestExpr::Lit("Im"),
            SuggestExpr::Lit("Ah"),
        ],
    ),
    ("[qQ]uicky", &[SuggestExpr::Lit("Quickie")]),
    ("[qQ]uickys", &[SuggestExpr::Lit("Quickies")]),
    (
        "bissl",
        &[SuggestExpr::Lit("bissel"), SuggestExpr::Lit("bisserl")],
    ),
    (
        "Keywort",
        &[SuggestExpr::Lit("Keyword"), SuggestExpr::Lit("Stichwort")],
    ),
    (
        "Keyworts",
        &[SuggestExpr::Lit("Keywords"), SuggestExpr::Lit("Stichworts")],
    ),
    (
        "Keywörter",
        &[
            SuggestExpr::Lit("Keywords"),
            SuggestExpr::Lit("Stichwörter"),
        ],
    ),
    (
        "strang",
        &[SuggestExpr::Lit("Strang"), SuggestExpr::Lit("strengte")],
    ),
    (
        "Gym",
        &[
            SuggestExpr::Lit("Fitnessstudio"),
            SuggestExpr::Lit("Gymnasium"),
        ],
    ),
    (
        "Wur",
        &[
            SuggestExpr::Lit("Wir"),
            SuggestExpr::Lit("Zur"),
            SuggestExpr::Lit("War"),
            SuggestExpr::Lit("Nur"),
        ],
    ),
    (
        "wur",
        &[
            SuggestExpr::Lit("wir"),
            SuggestExpr::Lit("zur"),
            SuggestExpr::Lit("war"),
            SuggestExpr::Lit("nur"),
        ],
    ),
    (
        "Gyms",
        &[
            SuggestExpr::Lit("Fitnessstudios"),
            SuggestExpr::Lit("Gymnasiums"),
        ],
    ),
    (
        "gäng",
        &[SuggestExpr::Lit("ging"), SuggestExpr::Lit("gang")],
    ),
    (
        "di",
        &[
            SuggestExpr::Lit("du"),
            SuggestExpr::Lit("die"),
            SuggestExpr::Lit("Di."),
            SuggestExpr::Lit("der"),
            SuggestExpr::Lit("den"),
        ],
    ),
    (
        "Di",
        &[
            SuggestExpr::Lit("Du"),
            SuggestExpr::Lit("Die"),
            SuggestExpr::Lit("Di."),
            SuggestExpr::Lit("Der"),
            SuggestExpr::Lit("Den"),
        ],
    ),
    (
        "Aufn",
        &[
            SuggestExpr::Lit("Auf den"),
            SuggestExpr::Lit("Auf einen"),
            SuggestExpr::Lit("Auf"),
        ],
    ),
    (
        "aufn",
        &[
            SuggestExpr::Lit("auf den"),
            SuggestExpr::Lit("auf einen"),
            SuggestExpr::Lit("auf"),
        ],
    ),
    (
        "Aufm",
        &[
            SuggestExpr::Lit("Auf dem"),
            SuggestExpr::Lit("Auf einem"),
            SuggestExpr::Lit("Auf"),
        ],
    ),
    (
        "aufm",
        &[
            SuggestExpr::Lit("auf dem"),
            SuggestExpr::Lit("auf einem"),
            SuggestExpr::Lit("auf"),
        ],
    ),
    (
        "Ausm",
        &[
            SuggestExpr::Lit("Aus dem"),
            SuggestExpr::Lit("Aus einem"),
            SuggestExpr::Lit("Aus"),
        ],
    ),
    (
        "ausm",
        &[
            SuggestExpr::Lit("aus dem"),
            SuggestExpr::Lit("aus einem"),
            SuggestExpr::Lit("aus"),
        ],
    ),
    (
        "best",
        &[
            SuggestExpr::Lit("beste"),
            SuggestExpr::Lit("bester"),
            SuggestExpr::Lit("Best"),
        ],
    ),
    (
        "Bitet",
        &[
            SuggestExpr::Lit("Bitte"),
            SuggestExpr::Lit("Bittet"),
            SuggestExpr::Lit("Bidet"),
            SuggestExpr::Lit("Bietet"),
        ],
    ),
    (
        "lage",
        &[
            SuggestExpr::Lit("lange"),
            SuggestExpr::Lit("Lage"),
            SuggestExpr::Lit("läge"),
            SuggestExpr::Lit("lache"),
        ],
    ),
    (
        "mur",
        &[
            SuggestExpr::Lit("mir"),
            SuggestExpr::Lit("zur"),
            SuggestExpr::Lit("nur"),
            SuggestExpr::Lit("für"),
        ],
    ),
    (
        "ass",
        &[
            SuggestExpr::Lit("Ass"),
            SuggestExpr::Lit("aß"),
            SuggestExpr::Lit("aus"),
            SuggestExpr::Lit("dass"),
        ],
    ),
    (
        "Blat",
        &[
            SuggestExpr::Lit("Blatt"),
            SuggestExpr::Lit("Blut"),
            SuggestExpr::Lit("Bald"),
            SuggestExpr::Lit("Bat"),
        ],
    ),
    (
        "much",
        &[
            SuggestExpr::Lit("mich"),
            SuggestExpr::Lit("auch"),
            SuggestExpr::Lit("Buch"),
        ],
    ),
    (
        "scheibe",
        &[SuggestExpr::Lit("Scheibe"), SuggestExpr::Lit("schreibe")],
    ),
    (
        "vielmal",
        &[
            SuggestExpr::Lit("Vielmal"),
            SuggestExpr::Lit("vielmals"),
            SuggestExpr::Lit("viermal"),
            SuggestExpr::Lit("viel mal"),
        ],
    ),
    (
        "bachten",
        &[
            SuggestExpr::Lit("brachten"),
            SuggestExpr::Lit("beachten"),
            SuggestExpr::Lit("machten"),
            SuggestExpr::Lit("pachten"),
        ],
    ),
    (
        "brache",
        &[
            SuggestExpr::Lit("brauche"),
            SuggestExpr::Lit("brachte"),
            SuggestExpr::Lit("brach"),
            SuggestExpr::Lit("bräche"),
        ],
    ),
    (
        "beliebn",
        &[
            SuggestExpr::Lit("beliebt"),
            SuggestExpr::Lit("bleiben"),
            SuggestExpr::Lit("belieben"),
        ],
    ),
    (
        "Kono",
        &[
            SuggestExpr::Lit("Kino"),
            SuggestExpr::Lit("Kongo"),
            SuggestExpr::Lit("Konto"),
        ],
    ),
    (
        "aich",
        &[
            SuggestExpr::Lit("ich"),
            SuggestExpr::Lit("auch"),
            SuggestExpr::Lit("sich"),
            SuggestExpr::Lit("eich"),
        ],
    ),
    (
        "anahme",
        &[
            SuggestExpr::Lit("Annahme"),
            SuggestExpr::Lit("nahmen"),
            SuggestExpr::Lit("nahe"),
            SuggestExpr::Lit("nahmen"),
        ],
    ),
    (
        "anleigen",
        &[
            SuggestExpr::Lit("anlegen"),
            SuggestExpr::Lit("Anliegen"),
            SuggestExpr::Lit("anliegen"),
            SuggestExpr::Lit("anzeigen"),
        ],
    ),
    (
        "besproch",
        &[
            SuggestExpr::Lit("besprach"),
            SuggestExpr::Lit("besprich"),
            SuggestExpr::Lit("bespreche"),
            SuggestExpr::Lit("besprochen"),
        ],
    ),
    (
        "dan",
        &[
            SuggestExpr::Lit("dann"),
            SuggestExpr::Lit("den"),
            SuggestExpr::Lit("das"),
            SuggestExpr::Lit("an"),
        ],
    ),
    (
        "lase",
        &[
            SuggestExpr::Lit("las"),
            SuggestExpr::Lit("lasse"),
            SuggestExpr::Lit("Nase"),
        ],
    ),
    (
        "Shr",
        &[
            SuggestExpr::Lit("Sehr"),
            SuggestExpr::Lit("Ihr"),
            SuggestExpr::Lit("Uhr"),
            SuggestExpr::Lit("Sir"),
        ],
    ),
    (
        "start",
        &[
            SuggestExpr::Lit("Start"),
            SuggestExpr::Lit("stark"),
            SuggestExpr::Lit("statt"),
            SuggestExpr::Lit("stand"),
        ],
    ),
    (
        "neuse",
        &[SuggestExpr::Lit("neues"), SuggestExpr::Lit("neue")],
    ),
    (
        "Standart",
        &[SuggestExpr::Lit("Standard"), SuggestExpr::Lit("Standort")],
    ),
    (
        "wiessen",
        &[
            SuggestExpr::Lit("wissen"),
            SuggestExpr::Lit("weisen"),
            SuggestExpr::Lit("wiesen"),
        ],
    ),
    (
        "schnells",
        &[SuggestExpr::Lit("schnell"), SuggestExpr::Lit("schnellst")],
    ),
    ("sn", &[SuggestExpr::Lit("an"), SuggestExpr::Lit("in")]),
    (
        "eie",
        &[
            SuggestExpr::Lit("die"),
            SuggestExpr::Lit("wie"),
            SuggestExpr::Lit("eine"),
            SuggestExpr::Lit("sie"),
        ],
    ),
    (
        "Mei",
        &[
            SuggestExpr::Lit("Mai"),
            SuggestExpr::Lit("Bei"),
            SuggestExpr::Lit("Sei"),
            SuggestExpr::Lit("Mein"),
        ],
    ),
    (
        "bim",
        &[
            SuggestExpr::Lit("bin"),
            SuggestExpr::Lit("im"),
            SuggestExpr::Lit("bis"),
            SuggestExpr::Lit("beim"),
        ],
    ),
    (
        "lehr",
        &[
            SuggestExpr::Lit("mehr"),
            SuggestExpr::Lit("lehrt"),
            SuggestExpr::Lit("sehr"),
            SuggestExpr::Lit("leer"),
        ],
    ),
    (
        "sm",
        &[
            SuggestExpr::Lit("am"),
            SuggestExpr::Lit("im"),
            SuggestExpr::Lit("am"),
            SuggestExpr::Lit("SM"),
        ],
    ),
    (
        "tuh",
        &[
            SuggestExpr::Lit("tun"),
            SuggestExpr::Lit("tut"),
            SuggestExpr::Lit("tue"),
            SuggestExpr::Lit("Kuh"),
        ],
    ),
    (
        "wuden",
        &[SuggestExpr::Lit("wurden"), SuggestExpr::Lit("würden")],
    ),
    (
        "Arzte",
        &[SuggestExpr::Lit("Ärzte"), SuggestExpr::Lit("Arzt")],
    ),
    ("Arzten", &[SuggestExpr::Lit("Ärzten")]),
    ("Alternatief", &[SuggestExpr::Lit("Alternativ")]),
    (
        "Pkt",
        &[
            SuggestExpr::Lit("Pkt."),
            SuggestExpr::Lit("Pakt"),
            SuggestExpr::Lit("Punkt"),
            SuggestExpr::Lit("Akt"),
        ],
    ),
    (
        "intere",
        &[
            SuggestExpr::Lit("interne"),
            SuggestExpr::Lit("innere"),
            SuggestExpr::Lit("hintere"),
            SuggestExpr::Lit("untere"),
        ],
    ),
    ("Eon", &[SuggestExpr::Lit("Ein"), SuggestExpr::Lit("E.ON")]),
    (
        "unterschiede",
        &[
            SuggestExpr::Lit("Unterschiede"),
            SuggestExpr::Lit("unterscheide"),
            SuggestExpr::Lit("unterschiebe"),
            SuggestExpr::Lit("unterschieden"),
        ],
    ),
    ("bi", &[SuggestExpr::Lit("bei")]),
    ("Aendert", &[SuggestExpr::Lit("Ändert")]),
    ("aendert", &[SuggestExpr::Lit("ändert")]),
    ("bizte", &[SuggestExpr::Lit("bitte")]),
    ("korekkt", &[SuggestExpr::Lit("korrekt")]),
    ("Erhlich", &[SuggestExpr::Lit("Ehrlich")]),
    ("gestrest", &[SuggestExpr::Lit("gestresst")]),
    ("rauschicken", &[SuggestExpr::Lit("rausschicken")]),
    ("stoniren", &[SuggestExpr::Lit("stornieren")]),
    ("drinen", &[SuggestExpr::Lit("drinnen")]),
    ("gestigen", &[SuggestExpr::Lit("gestiegen")]),
    ("prozes", &[SuggestExpr::Lit("Prozess")]),
    ("Auschluss", &[SuggestExpr::Lit("Ausschluss")]),
    ("Anbeot", &[SuggestExpr::Lit("Angebot")]),
    ("Paleten", &[SuggestExpr::Lit("Paletten")]),
    ("mächten", &[SuggestExpr::Lit("möchten")]),
    ("auschreibung", &[SuggestExpr::Lit("Ausschreibung")]),
    ("worter", &[SuggestExpr::Lit("Wörter")]),
    ("Ihrerer", &[SuggestExpr::Lit("Ihrer")]),
    ("Modelles", &[SuggestExpr::Lit("Modells")]),
    ("entchuldigen", &[SuggestExpr::Lit("entschuldigen")]),
    ("kundne", &[SuggestExpr::Lit("Kunden")]),
    ("bestellun", &[SuggestExpr::Lit("Bestellung")]),
    ("[Nn]umber", &[SuggestExpr::Lit("Nummer")]),
    ("mirgen", &[SuggestExpr::Lit("morgen")]),
    ("korekkt", &[SuggestExpr::Lit("korrekt")]),
    ("Bs", &[SuggestExpr::Lit("Bis")]),
    ("Biß", &[SuggestExpr::Lit("Biss")]),
    ("bs", &[SuggestExpr::Lit("bis")]),
    ("sehn", &[SuggestExpr::Lit("sehen")]),
    ("zutun", &[SuggestExpr::Lit("zu tun")]),
    ("Müllhalte", &[SuggestExpr::Lit("Müllhalde")]),
    ("Entäuschung", &[SuggestExpr::Lit("Enttäuschung")]),
    ("Entäuschungen", &[SuggestExpr::Lit("Enttäuschungen")]),
    (
        "kanns",
        &[SuggestExpr::Lit("kann es"), SuggestExpr::Lit("kannst")],
    ),
    (
        "verklinken",
        &[
            SuggestExpr::Lit("verklinkern"),
            SuggestExpr::Lit("verlinken"),
            SuggestExpr::Lit("verklingen"),
        ],
    ),
    ("funktionierts", &[SuggestExpr::Lit("funktioniert es")]),
    ("hbat", &[SuggestExpr::Lit("habt")]),
    ("ichs", &[SuggestExpr::Lit("ich es")]),
    ("folgendermassen", &[SuggestExpr::Lit("folgendermaßen")]),
    ("Adon", &[SuggestExpr::Lit("Add-on")]),
    ("Adons", &[SuggestExpr::Lit("Add-ons")]),
    ("ud", &[SuggestExpr::Lit("und")]),
    (
        "vertaggt",
        &[SuggestExpr::Lit("vertagt"), SuggestExpr::Lit("getaggt")],
    ),
    (
        "keinsten",
        &[SuggestExpr::Lit("keinen"), SuggestExpr::Lit("kleinsten")],
    ),
    ("Angehensweise", &[SuggestExpr::Lit("Vorgehensweise")]),
    ("Angehensweisen", &[SuggestExpr::Lit("Vorgehensweisen")]),
    ("Neudefinierung", &[SuggestExpr::Lit("Neudefinition")]),
    ("Definierung", &[SuggestExpr::Lit("Definition")]),
    ("Definierungen", &[SuggestExpr::Lit("Definitionen")]),
    (
        "[Üü]bergrifflich(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "lich",
            replacement: "ig",
        }],
    ),
    (
        "löchen",
        &[
            SuggestExpr::Lit("löschen"),
            SuggestExpr::Lit("löchern"),
            SuggestExpr::Lit("Köchen"),
        ],
    ),
    (
        "wergen",
        &[
            SuggestExpr::Lit("werfen"),
            SuggestExpr::Lit("werben"),
            SuggestExpr::Lit("werten"),
        ],
    ),
    (
        "Wasn",
        &[
            SuggestExpr::Lit("Was denn"),
            SuggestExpr::Lit("Was ein"),
            SuggestExpr::Lit("Was"),
        ],
    ),
    (
        "schammig(e[mnrs]?)?",
        &[SuggestExpr::ReplaceFirst {
            pattern: "schamm",
            replacement: "schwamm",
        }],
    ),
];

/// `GermanSpellerRule.getOnlySuggestions` switch cases (replace all
/// other suggestions; descriptions dropped, they do not affect the
/// suggestion values).
pub(crate) static ONLY_SUGGESTIONS: &[(&str, &[&str])] = &[
    ("do", &["so"]),
    ("Akkupressur", &["Akupressur"]),
    ("EmailleOfen", &["Emailleofen"]),
    ("Mazeltov", &["Masel tov"]),
    ("Mazel tov", &["Masel tov"]),
    ("Weisglut", &["Weißglut"]),
    ("SuperGAU", &["Super-GAU"]),
    ("SuperGau", &["Super-Gau"]),
    ("Besenstil", &["Besenstiel"]),
    ("Vorbescheidverfahren", &["Vorbescheidsverfahren"]),
    ("Türahmen", &["Türrahmen"]),
    ("Unglückzahl", &["Unglückszahl"]),
    ("Unglückzahlen", &["Unglückszahlen"]),
    ("Anstossbreite", &["Anstoßbreite"]),
    ("Elfmeterschiessen", &["Elfmeterschießen"]),
    ("Feldschiessen", &["Feldschießen"]),
    ("Flössgräben", &["Flößgräben"]),
    ("Schiessen", &["Schießen"]),
    ("Gasterra", &["GasTerra"]),
    ("Gasterras", &["GasTerras"]),
    ("Bestwater", &["BestWater"]),
    ("Bestwaters", &["BestWaters"]),
    ("Außländer", &["Ausländer"]),
    ("Außländern", &["Ausländern"]),
    ("Außländers", &["Ausländers"]),
    ("Außländerin", &["Ausländerin"]),
    ("Außländerinnen", &["Ausländerinnen"]),
    ("Wehrwolf", &["Werwolf"]),
    ("Wehrwolfs", &["Werwolfs"]),
    ("Wehrwölfe", &["Werwölfe"]),
    ("Wehrwölfen", &["Werwölfen"]),
    ("Art-Nr", &["Art.-Nr."]),
    ("werrden", &["werden"]),
    ("erkäre", &["erkläre"]),
    ("Blackweek", &["Black Week"]),
    ("Blackfriday", &["Black Friday"]),
    ("Blackweeks", &["Black Weeks"]),
    ("Blackfridays", &["Black Fridays"]),
    ("Au-Pair", &["Au-pair"]),
    ("Au-Pairs", &["Au-pairs"]),
    ("deluxe", &["de luxe"]),
    ("Deluxe", &["de luxe"]),
    ("Design-Op", &["Design-OP"]),
    ("AppStore", &["App Store"]),
    ("AppStores", &["App Stores"]),
    ("AppleStore", &["Apple Store"]),
    ("AppleStores", &["Apple Stores"]),
    ("AirFrance", &["Air France"]),
    ("Wiederspruch", &["Widerspruch"]),
    ("Wiederspruchs", &["Widerspruchs"]),
    ("Wiedersprüche", &["Widersprüche"]),
    ("Wiedersprüchen", &["Widersprüchen"]),
    ("Vorraussetzung", &["Voraussetzung"]),
    ("Vorraussetzungen", &["Voraussetzungen"]),
    ("Schalosie", &["Jalousie"]),
    ("Schalosien", &["Jalousien"]),
    ("offensichtlicherweise", &["offensichtlich"]),
    ("Offensichtlicherweise", &["Offensichtlich"]),
    ("wohlwissend", &["wohl wissend"]),
    ("Visas", &["Visa"]),
    ("Interresse", &["Interesse"]),
    ("Interressen", &["Interessen"]),
    ("Terasse", &["Terrasse"]),
    ("Terassen", &["Terrassen"]),
    ("Reisverschluss", &["Reißverschluss"]),
    ("Reisverschlusses", &["Reißverschlusses"]),
    ("Reiszwecke", &["Reißzwecke"]),
    ("Reiszwecken", &["Reißzwecken"]),
    ("up-to-date", &["up to date"]),
    ("falscherweise", &["fälschlicherweise"]),
    ("schliesslich", &["schließlich"]),
    ("Schliesslich", &["Schließlich"]),
    ("daß", &["dass"]),
    ("Daß", &["Dass"]),
    ("mußt", &["musst"]),
    ("Mußt", &["Musst"]),
    ("müßt", &["müsst"]),
    ("Müßt", &["Müsst"]),
    ("heisst", &["heißt"]),
    ("Heisst", &["Heißt"]),
    ("heisse", &["heiße"]),
    ("heissen", &["heißen"]),
    ("beisst", &["beißt"]),
    ("beissen", &["beißen"]),
    ("mußten", &["mussten"]),
    ("mußte", &["musste"]),
    ("mußtest", &["musstest"]),
    ("müßtest", &["müsstest"]),
    ("müßen", &["müssen"]),
    ("müßten", &["müssten"]),
    ("müßte", &["müsste"]),
    ("wußte", &["wusste"]),
    ("wußten", &["wussten"]),
    ("wüßte", &["wüsste"]),
    ("wüßten", &["wüssten"]),
    ("bescheid", &["Bescheid"]),
    ("Facetime", &["FaceTime"]),
    ("Facetimes", &["FaceTimes"]),
    ("ausversehen", &["aus Versehen"]),
    ("Stückweit", &["Stück weit"]),
    ("Uranium", &["Uran"]),
    ("Uraniums", &["Urans"]),
    ("Luxenburg", &["Luxemburg"]),
    ("Luxenburgs", &["Luxemburgs"]),
    ("Lichtenstein", &["Liechtenstein"]),
    ("Lichtensteins", &["Liechtensteins"]),
    ("immernoch", &["immer noch"]),
    ("Rechtshcreibfehler", &["Rechtschreibfehler"]),
    ("markirt", &["markiert"]),
    ("Johannesbeere", &["Johannisbeere"]),
    ("Johannesbeeren", &["Johannisbeeren"]),
    ("Endgeld", &["Entgeld"]),
    ("Entäuschung", &["Enttäuschung"]),
    ("Entäuschungen", &["Enttäuschungen"]),
    ("Triologie", &["Trilogie"]),
    ("ausserdem", &["außerdem"]),
    ("Ausserdem", &["Außerdem"]),
    ("ausser", &["außer"]),
    ("Ausser", &["Außer"]),
    ("bischen", &["bisschen"]),
    ("bißchen", &["bisschen"]),
    ("meißt", &["meist"]),
    ("meißten", &["meisten"]),
    ("meißtens", &["meistens"]),
    ("Babyphone", &["Babyfon"]),
    ("Baby-Phone", &["Babyfon"]),
    ("gescheint", &["geschienen"]),
    ("staubgesaugt", &["gestaubsaugt"]),
    ("geupdated", &["upgedatet"]),
    ("geupdatet", &["upgedatet"]),
    ("gedownloaded", &["downgeloadet"]),
    ("gedownloadet", &["downgeloadet"]),
    ("gedownloadete", &["downgeloadete"]),
    ("gedownloadeter", &["downgeloadeter"]),
    ("gedownloadetes", &["downgeloadetes"]),
    ("gedownloadeten", &["downgeloadeten"]),
    ("gedownloadetem", &["downgeloadetem"]),
    ("geuploaded", &["upgeloadet"]),
    ("geuploadet", &["upgeloadet"]),
    ("geuploadete", &["upgeloadete"]),
    ("geuploadeter", &["upgeloadeter"]),
    ("geuploadetes", &["upgeloadetes"]),
    ("geuploadeten", &["upgeloadeten"]),
    ("geuploadetem", &["upgeloadetem"]),
    ("Frauenhofer", &["Fraunhofer"]),
    ("hörensagen", &["Hörensagen"]),
    ("Mwst", &["MwSt"]),
    ("MwSt", &["MwSt."]),
    ("MWST", &["MwSt."]),
    ("exkl", &["exkl."]),
    ("inkl", &["inkl."]),
    ("hälst", &["hältst"]),
    ("Rythmus", &["Rhythmus"]),
    ("Rhytmus", &["Rhythmus"]),
    ("Rhytmen", &["Rhythmen"]),
    ("Hobbies", &["Hobbys"]),
    ("Stehgreif", &["Stegreif"]),
    ("brilliant", &["brillant"]),
    ("brilliante", &["brillante"]),
    ("brilliantes", &["brillantes"]),
    ("brillianter", &["brillanter"]),
    ("brillianten", &["brillanten"]),
    ("brilliantem", &["brillantem"]),
    ("Billiard", &["Billard"]),
    ("garnicht", &["gar nicht"]),
    ("garnich", &["gar nicht"]),
    ("garnichts", &["gar nichts"]),
    ("assozial", &["asozial"]),
    ("assoziale", &["asoziale"]),
    ("assoziales", &["asoziales"]),
    ("assozialer", &["asozialer"]),
    ("assozialen", &["asozialen"]),
    ("assozialem", &["asozialem"]),
    ("Verwandschaft", &["Verwandtschaft"]),
    ("vorraus", &["voraus"]),
    ("Vorraus", &["Voraus"]),
    ("Reperatur", &["Reparatur"]),
    ("Reperaturen", &["Reparaturen"]),
    ("Bzgl", &["Bzgl."]),
    ("bzgl", &["bzgl."]),
    ("Eigtl", &["Eigtl."]),
    ("eigtl", &["eigtl."]),
    ("Mo-Di", &["Mo.–Di."]),
    ("Mo-Mi", &["Mo.–Mi."]),
    ("Mo-Do", &["Mo.–Do."]),
    ("Mo-Fr", &["Mo.–Fr."]),
    ("Mo-Sa", &["Mo.–Sa."]),
    ("Mo-So", &["Mo.–So."]),
    ("Di-Mi", &["Di.–Mi."]),
    ("Di-Do", &["Di.–Do."]),
    ("Di-Fr", &["Di.–Fr."]),
    ("Di-Sa", &["Di.–Sa."]),
    ("Di-So", &["Di.–So."]),
    ("Mi-Do", &["Mi.–Do."]),
    ("Mi-Fr", &["Mi.–Fr."]),
    ("Mi-Sa", &["Mi.–Sa."]),
    ("Mi-So", &["Mi.–So."]),
    ("Do-Fr", &["Do.–Fr."]),
    ("Do-Sa", &["Do.–Sa."]),
    ("Do-So", &["Do.–So."]),
    ("Fr-Sa", &["Fr.–Sa."]),
    ("Fr-So", &["Fr.–So."]),
    ("Sa-So", &["Sa.–So."]),
    ("Achso", &["Ach so"]),
    ("achso", &["ach so"]),
    ("Huskies", &["Huskys"]),
    ("Jedesmal", &["Jedes Mal"]),
    ("jedesmal", &["jedes Mal"]),
    ("Lybien", &["Libyen"]),
    ("Lybiens", &["Libyens"]),
    ("Youtube", &["YouTube"]),
    ("Youtuber", &["YouTuber"]),
    ("Youtuberin", &["YouTuberin"]),
    ("Youtuberinnen", &["YouTuberinnen"]),
    ("Youtubers", &["YouTubers"]),
    ("Reflektion", &["Reflexion"]),
    ("Reflektionen", &["Reflexionen"]),
    ("unrelevant", &["irrelevant"]),
    ("inflagranti", &["in flagranti"]),
    ("Storie", &["Story"]),
    ("Stories", &["Storys"]),
    ("Ladies", &["Ladys"]),
    ("Parties", &["Partys"]),
    ("Lobbies", &["Lobbys"]),
    ("Nestle", &["Nestlé"]),
    ("Nestles", &["Nestlés"]),
    ("vollzeit", &["Vollzeit"]),
    ("teilzeit", &["Teilzeit"]),
    ("Dnake", &["Danke"]),
    ("Muehe", &["Mühe"]),
    ("Muehen", &["Mühen"]),
    ("Kuhdamm", &["Ku’damm"]),
    ("Torschusspanik", &["Torschlusspanik"]),
    ("ggf", &["ggf."]),
    ("Ggf", &["Ggf."]),
    ("zzgl", &["zzgl."]),
    ("Zzgl", &["Zzgl."]),
    ("aufgehangen", &["aufgehängt"]),
    ("Annektion", &["Annexion"]),
    ("Annektionen", &["Annexionen"]),
    ("unkonsistent", &["inkonsistent"]),
    ("Weißheitszahn", &["Weisheitszahn"]),
    ("Weissheitszahn", &["Weisheitszahn"]),
    ("Weißheitszahns", &["Weisheitszahns"]),
    ("Weissheitszahns", &["Weisheitszahns"]),
    ("Weißheitszähne", &["Weisheitszähne"]),
    ("Weissheitszähne", &["Weisheitszähne"]),
    ("Weißheitszähnen", &["Weisheitszähnen"]),
    ("Weissheitszähnen", &["Weisheitszähnen"]),
    ("raufschauen", &["draufschauen"]),
    ("raufzuschauen", &["draufzuschauen"]),
    ("raufgeschaut", &["draufgeschaut"]),
    ("raufschaue", &["draufschaue"]),
    ("raufschaust", &["draufschaust"]),
    ("raufschaut", &["draufschaut"]),
    ("raufschaute", &["draufschaute"]),
    ("raufschauten", &["draufschauten"]),
    ("raufgucken", &["draufgucken"]),
    ("raufzugucken", &["draufzugucken"]),
    ("raufgeguckt", &["draufgeguckt"]),
    ("raufgucke", &["draufgucke"]),
    ("raufguckst", &["draufguckst"]),
    ("raufguckt", &["draufguckt"]),
    ("raufguckte", &["draufguckte"]),
    ("raufguckten", &["draufguckten"]),
    ("raufhauen", &["draufhauen"]),
    ("raufzuhauen", &["draufzuhauen"]),
    ("raufgehaut", &["draufgehaut"]),
    ("raufhaue", &["draufhaue"]),
    ("raufhaust", &["draufhaust"]),
    ("raufhaut", &["draufhaut"]),
    ("raufhaute", &["draufhaute"]),
    ("raufhauten", &["draufhauten"]),
    ("wohlmöglich", &["womöglich"]),
    ("geschalten", &["geschaltet"]),
    ("angeschalten", &["angeschaltet"]),
    ("abgeschalten", &["abgeschaltet"]),
    ("hiess", &["hieß"]),
    ("Click", &["Klick"]),
    ("Clicks", &["Klicks"]),
    ("jenachdem", &["je nachdem"]),
    ("bsp", &["bspw"]),
    ("vorallem", &["vor allem"]),
    ("draussen", &["draußen"]),
    ("ürbigens", &["übrigens"]),
    ("Whatsapp", &["WhatsApp"]),
    ("kucken", &["gucken"]),
    ("kuckten", &["guckten"]),
    ("kucke", &["gucke"]),
    ("aelter", &["älter"]),
    ("äussern", &["äußern"]),
    ("äusserst", &["äußerst"]),
    ("geäussert", &["geäußert"]),
    ("Äusserst", &["Äußerst"]),
    ("Dnk", &["Dank"]),
    ("schleswig-holstein", &["Schleswig-Holstein"]),
    ("Stahlkraft", &["Strahlkraft"]),
    ("trümmern", &["Trümmern"]),
    ("gradeaus", &["geradeaus"]),
    ("Anschliessend", &["Anschließend"]),
    ("anschliessend", &["anschließend"]),
    ("Abschliessend", &["Abschließend"]),
    ("abschliessend", &["abschließend"]),
    ("Ruckmeldung", &["Rückmeldung"]),
    ("Gepaeck", &["Gepäck"]),
    ("Grüsse", &["Grüße"]),
    ("Grüssen", &["Grüßen"]),
    ("entgültig", &["endgültig"]),
    ("entgültige", &["endgültige"]),
    ("entgültiges", &["endgültiges"]),
    ("entgültiger", &["endgültiger"]),
    ("entgültigen", &["endgültigen"]),
    ("desöfteren", &["des Öfteren"]),
    ("desweiteren", &["des Weiteren"]),
    ("weitesgehend", &["weitestgehend"]),
    ("Tschibo", &["Tchibo"]),
    ("Tschibos", &["Tchibos"]),
    ("Tiktok", &["TikTok"]),
    ("Tiktoks", &["TikToks"]),
    ("sodaß", &["sodass"]),
    ("regelmässig", &["regelmäßig"]),
    ("Carplay", &["CarPlay"]),
    ("Tiktoker", &["TikToker"]),
    ("Tiktokerin", &["TikTokerin"]),
    ("Tiktokerinnen", &["TikTokerinnen"]),
    ("Tiktokers", &["TikTokers"]),
    ("Tiktokern", &["TikTokern"]),
    ("languagetool", &["LanguageTool"]),
    ("languagetools", &["LanguageTools"]),
    ("Languagetool", &["LanguageTool"]),
    ("Languagetools", &["LanguageTools"]),
    ("liket", &["likt"]),
    ("nagut", &["na gut"]),
    ("Nagut", &["Na gut"]),
    ("HAllo", &["Hallo"]),
    ("HEy", &["Hey"]),
    ("SEhr", &["Sehr"]),
    ("abhol", &["abhole"]),
    ("amazon", &["Amazon"]),
    ("irgendeins", &["irgendeines"]),
    ("Communities", &["Communitys"]),
    ("ansich", &["an sich"]),
    ("Spass", &["Spaß"]),
    ("garkein", &["gar kein"]),
    ("garkeine", &["gar keine"]),
    ("garkeinen", &["gar keinen"]),
    ("wieviel", &["wie viel"]),
    ("Wieviel", &["Wie viel"]),
    ("gets", &["gehts"]),
    ("Quillbot", &["QuillBot"]),
    ("ebensowenig", &["ebenso wenig"]),
    ("Wiedersehn", &["Wiedersehen"]),
    ("wiedersehn", &["wiedersehen"]),
    ("Ohje", &["Oje"]),
    ("ohje", &["oje"]),
    ("schwupps", &["schwups"]),
    ("Schwupps", &["Schwups"]),
    ("Massnahme", &["Maßnahme"]),
    ("Massnahmen", &["Maßnahmen"]),
    ("Linkedin", &["LinkedIn"]),
    ("Wordpress", &["WordPress"]),
    ("gleichzeit", &["gleichzeitig"]),
    ("DAnke", &["Danke"]),
    ("Interior", &["Interieur"]),
    ("Interiors", &["Interieurs"]),
    ("trifftigen", &["triftigen"]),
    ("trifftigem", &["triftigem"]),
    ("trifftige", &["triftige"]),
    ("trifftiges", &["triftiges"]),
    ("trifftiger", &["triftiger"]),
    ("gehhrte", &["geehrte"]),
    ("gehhrten", &["geehrten"]),
    ("gehhrtes", &["geehrtes"]),
    ("Iphone", &["iPhone"]),
    ("Iphones", &["iPhones"]),
    ("iphone", &["iPhone"]),
    ("iphones", &["iPhones"]),
    ("Ipad", &["iPad"]),
    ("Ipads", &["iPads"]),
    ("ipad", &["iPad"]),
    ("ipads", &["iPads"]),
    ("letzlich", &["letztlich"]),
    ("Letzlich", &["Letztlich"]),
    ("gefühlsdusselig", &["gefühlsduselig"]),
    ("gefühlsdusselige", &["gefühlsduselige"]),
    ("gefühlsdusseliger", &["gefühlsduseliger"]),
    ("gefühlsdusseliges", &["gefühlsduseliges"]),
    ("gefühlsdusseligen", &["gefühlsduseligen"]),
    ("gegebenfalls", &["gegebenenfalls"]),
    ("Gegebenfalls", &["Gegebenenfalls"]),
    ("zugebenermaßen", &["zugegebenermaßen"]),
    ("beispielweise", &["beispielsweise"]),
    ("pdf", &["PDF"]),
    ("Pdf", &["PDF"]),
    ("pdfs", &["PDFs"]),
    ("Pdfs", &["PDFs"]),
    ("gekriecht", &["gekrochen"]),
    ("einzigst", &["einzig"]),
    ("Einzigst", &["Einzig"]),
    ("Eifelturm", &["Eiffelturm"]),
    ("Eifelturms", &["Eiffelturms"]),
    ("Eifelturmes", &["Eiffelturmes"]),
    ("Jojo-Effekt", &["Jo-Jo-Effekt"]),
    ("Jojo-Effekts", &["Jo-Jo-Effekts"]),
    ("Enschuldigen", &["Entschuldigen"]),
    ("Anschrifft", &["Anschrift"]),
    ("vertrauenserweckend", &["vertrauenerweckend"]),
    ("homepage", &["Homepage"]),
    ("interesse", &["Interesse"]),
    ("moglich", &["möglich"]),
    ("zusammenfässt", &["zusammenfasst"]),
    ("grosse", &["große"]),
    ("grossen", &["großen"]),
    ("grosser", &["großer"]),
    ("grosses", &["großes"]),
    ("geniesse", &["genieße"]),
    ("geniessen", &["genießen"]),
    ("grossartig", &["großartig"]),
    ("grosszügig", &["großzügig"]),
    ("moeglich", &["möglich"]),
    ("naturlich", &["natürlich"]),
    ("natuerlich", &["natürlich"]),
    ("unregelmässig", &["unregelmäßig"]),
    ("unregelmässige", &["unregelmäßige"]),
    ("unaktiv", &["inaktiv"]),
    ("unaktive", &["inaktive"]),
    ("unaktiver", &["inaktiver"]),
    ("unaktives", &["inaktives"]),
    ("unaktiven", &["inaktiven"]),
    ("uneffektiv", &["ineffektiv"]),
    ("uneffezient", &["ineffizient"]),
    ("rechtstaatlich", &["rechtsstaatlich"]),
    ("verhältnismässig", &["verhältnismäßig"]),
    ("unverhältnismässig", &["unverhältnismäßig"]),
    ("Hauptstrasse", &["Hauptstraße"]),
    ("Gespraech", &["Gespräch"]),
    ("Gespraechs", &["Gesprächs"]),
    ("Aussenbereich", &["Außenbereich"]),
    ("Aussenbereichs", &["Außenbereichs"]),
    ("Portrait", &["Porträt"]),
    ("Portraits", &["Porträts"]),
    ("weinachten", &["Weihnachten"]),
    ("Weinachten", &["Weihnachten"]),
    ("unterstüzt", &["unterstützt"]),
    ("untersützt", &["unterstützt"]),
    ("sontag", &["Sonntag"]),
    ("nichtsagend", &["nichtssagend"]),
    ("nichtsagende", &["nichtssagende"]),
    ("nichtsagender", &["nichtssagender"]),
    ("nichtsagendes", &["nichtssagendes"]),
    ("nichtsagenden", &["nichtssagenden"]),
    ("nichtsagendem", &["nichtssagendem"]),
    ("nirgendswo", &["nirgendwo"]),
    ("durchfuehren", &["durchführen"]),
    ("durchgefuehrt", &["durchgeführt"]),
    ("erhälst", &["erhältst"]),
    ("erhählst", &["erhältst"]),
    ("Nirgendswo", &["Nirgendwo"]),
    ("Typescript", &["TypeScript"]),
    ("mitinbegriffen", &["mit inbegriffen"]),
    ("miteinbegriffen", &["mit einbegriffen"]),
    ("unterjährlich", &["unterjährig"]),
    ("mehrjährlich", &["mehrjährig"]),
    ("mehrjährliche", &["mehrjährige"]),
    ("mehrjährlichen", &["mehrjährigen"]),
    ("mehrjährlicher", &["mehrjähriger"]),
    ("mehrjährliches", &["mehrjähriges"]),
    ("genausogut", &["genauso gut"]),
    ("Sylvester", &["Silvester"]),
    ("Außerden", &["Außerdem"]),
    ("Irish-Pub", &["Irish Pub"]),
    ("Irish-Pubs", &["Irish Pubs"]),
    ("ausserhalb", &["außerhalb"]),
    ("Ausserhalb", &["Außerhalb"]),
    ("Add-On", &["Add-on"]),
    ("Add-Ons", &["Add-ons"]),
    ("zweitenmal", &["zweiten Mal"]),
    ("Zweitenmal", &["Zweiten Mal"]),
    ("Nächstesmal", &["Nächstes Mal"]),
    ("Walldorfschule", &["Waldorfschule"]),
    ("Walldorfschulen", &["Waldorfschulen"]),
    ("ertragsreich", &["ertragreich"]),
    ("ertragsreiche", &["ertragreiche"]),
    ("ertragsreiches", &["ertragreiches"]),
    ("ertragsreichen", &["ertragreichen"]),
    ("einzigste", &["einzige"]),
    ("einzigstes", &["einziges"]),
    ("einzigster", &["einziger"]),
    ("einzigsten", &["einzigen"]),
    ("einzigstem", &["einzigem"]),
    ("Youngstar", &["Youngster"]),
    ("Youngstars", &["Youngsters"]),
    ("aussergewöhnlichen", &["außergewöhnlichen"]),
    ("aussergewöhnliche", &["außergewöhnliche"]),
    ("aussergewöhnlicher", &["außergewöhnlicher"]),
    ("aussergewöhnliches", &["außergewöhnliches"]),
    ("aussergewöhnlich", &["außergewöhnlich"]),
    ("Gluckwunsch", &["Glückwunsch"]),
    ("Gluckwunsche", &["Glückwünsche"]),
    ("Glückwunsche", &["Glückwünsche"]),
    ("außerden", &["außerdem"]),
    ("gleichermassen", &["gleichermaßen"]),
    ("massgeblich", &["maßgeblich"]),
    ("tschuldige", &["entschuldige"]),
    ("Tschuldigung", &["Entschuldigung"]),
    ("Schuldigung", &["Entschuldigung"]),
    ("Anteilname", &["Anteilnahme"]),
    ("Mahnungswesen", &["Mahnwesen"]),
    ("Mahnungswesens", &["Mahnwesens"]),
    ("Geruchsinn", &["Geruchssinn"]),
    ("Geruchsinns", &["Geruchssinns"]),
    ("Optin", &["Opt-in"]),
    ("Stk", &["Stk."]),
    ("T-shirt", &["T-Shirt"]),
    ("t-shirt", &["T-Shirt"]),
    ("T-shirts", &["T-Shirts"]),
    ("t-shirts", &["T-Shirts"]),
    ("umgangsprachlich", &["umgangssprachlich"]),
    ("E-Mai", &["E-Mail"]),
    ("E-Mais", &["E-Mails"]),
    ("Gelantine", &["Gelatine"]),
    ("angehangenen", &["angehängten"]),
    ("ausmahlen", &["ausmalen"]),
    ("ausgemahlt", &["ausgemalt"]),
    ("weisst", &["weißt"]),
    ("Weisst", &["Weißt"]),
    ("Rehgipsplatte", &["Rigipsplatte"]),
    ("Rehgipsplatten", &["Rigipsplatten"]),
    ("Rehgips-Platte", &["Rigips-Platte"]),
    ("Rehgips-Platten", &["Rigips-Platten"]),
    ("Rehgips", &["Rigips"]),
    ("rundumerneuert", &["runderneuert"]),
    ("rundumerneuerte", &["runderneuerte"]),
    ("rundumerneuertes", &["runderneuertes"]),
    ("rundumerneuerter", &["runderneuerter"]),
    ("rundumerneuerten", &["runderneuerten"]),
    ("rundumerneuertem", &["runderneuertem"]),
    ("Davidswache", &["Davidwache"]),
    ("Pinwand", &["Pinnwand"]),
    ("Kreisaal", &["Kreißsaal"]),
    ("Kreisaals", &["Kreißsaals"]),
    ("Kreissaal", &["Kreißsaal"]),
    ("Kreissäle", &["Kreißsäle"]),
    ("Kreissälen", &["Kreißsälen"]),
    ("Kreissaals", &["Kreißsaals"]),
    ("Laola-Welle", &["La-Ola-Welle"]),
    ("Laola-Wellen", &["La-Ola-Wellen"]),
    ("BayArea", &["Bay Area"]),
    ("kontaktfreundlich", &["kontaktfreudig"]),
    ("kontaktfreundliche", &["kontaktfreudige"]),
    ("kontaktfreundlicher", &["kontaktfreudiger"]),
    ("kontaktfreundliches", &["kontaktfreudiges"]),
    ("kontaktfreundlichen", &["kontaktfreudigen"]),
    ("kontaktfreundlichem", &["kontaktfreudigem"]),
    ("Wirtschaftsingenieurswesen", &["Wirtschaftsingenieurwesen"]),
    (
        "Wirtschaftsingenieurswesens",
        &["Wirtschaftsingenieurwesens"],
    ),
    ("wiederspiegeln", &["widerspiegeln"]),
    ("wiederspiegelt", &["widerspiegelt"]),
    ("wiederspiegelst", &["widerspiegelst"]),
    ("wiedergespiegelt", &["widergespiegelt"]),
    ("Wiederhall", &["Widerhall"]),
    ("Wiederhalls", &["Widerhalls"]),
    ("Ebensowenig", &["Ebenso wenig"]),
    ("Ebensooft", &["Ebenso oft"]),
    ("ebensooft", &["ebenso oft"]),
    ("Ebensogut", &["Ebenso gut"]),
    ("ebensogut", &["ebenso gut"]),
    ("Ebensoleicht", &["Ebenso leicht"]),
    ("ebensoleicht", &["ebenso leicht"]),
    ("eigendlich", &["eigentlich"]),
    ("eigendliche", &["eigentliche"]),
    ("eigendlicher", &["eigentlicher"]),
    ("eigendliches", &["eigentliches"]),
    ("eigendlichen", &["eigentlichen"]),
    ("eigendlichem", &["eigentlichem"]),
    ("rüberstülpen", &["überstülpen"]),
    ("rüberstülpe", &["überstülpe"]),
    ("rübergestülpt", &["übergestülpt"]),
    ("Websiten", &["Webseiten"]),
    ("freiverfügbar", &["frei verfügbar"]),
    ("freiverfügbare", &["frei verfügbare"]),
    ("freiverfügbares", &["frei verfügbares"]),
    ("freiverfügbarer", &["frei verfügbarer"]),
    ("freiverfügbaren", &["frei verfügbaren"]),
    ("freiverfügbarem", &["frei verfügbarem"]),
    ("freiverkäuflich", &["frei verkäuflich"]),
    ("freiverkäufliche", &["frei verkäufliche"]),
    ("freiverkäufliches", &["frei verkäufliches"]),
    ("freiverkäuflicher", &["frei verkäuflicher"]),
    ("freiverkäuflichen", &["frei verkäuflichen"]),
    ("freiverkäuflichem", &["frei verkäuflichem"]),
    ("Mfg", &["MfG"]),
    ("Gefahrenstoffe", &["Gefahrstoffe"]),
    ("Gefahrenstoffen", &["Gefahrstoffen"]),
    ("Resource", &["Ressource"]),
    ("Resourcen", &["Ressourcen"]),
    ("Resources", &["Ressourcen"]),
    ("Tzatziki", &["Zaziki"]),
    ("Selenski", &["Selenskyj"]),
    ("armzurechnen", &["arm zu rechnen"]),
    ("armrechne", &["arm rechne"]),
    ("armrechnest", &["arm rechnest"]),
    ("armrechnet", &["arm rechnet"]),
    ("armrechnen", &["arm rechnen"]),
    ("armgerechnet", &["arm gerechnet"]),
    ("ernstnimmst", &["ernst nimmst"]),
    ("ernstnimmt", &["ernst nimmt"]),
    ("ernstnehme", &["ernst nehme"]),
    ("ernstnehmen", &["ernst nehmen"]),
    ("ernstzunehmen", &["ernst zu nehmen"]),
    ("ernstgenommen", &["ernst genommen"]),
    ("ernstmeinst", &["ernst meinst"]),
    ("ernstmeine", &["ernst meine"]),
    ("ernstmeinte", &["ernst meinte"]),
    ("ernstmeinen", &["ernst meinen"]),
    ("ernstzumeinen", &["ernst zu meinen"]),
    ("ernstgemeinet", &["ernst gemeint"]),
    ("fertigschreiben", &["fertig schreiben"]),
    ("fertigzuschreiben", &["fertig zu schreiben"]),
    ("fertiggeschrieben", &["fertig geschrieben"]),
    ("fertigschreibt", &["fertig schreibt"]),
    ("freigedacht", &["frei gedacht"]),
    ("freidenken", &["frei denken"]),
    ("freizudenken", &["frei zu denken"]),
    ("freiliegen", &["frei liegen"]),
    ("freischreiben", &["frei schreiben"]),
    ("freizuschreiben", &["frei zu schreiben"]),
    ("freigeschrieben", &["frei geschrieben"]),
    ("freischlagen", &["frei schlagen"]),
    ("freizuschlagen", &["frei zu schlagen"]),
    ("freigeschlagen", &["frei geschlagen"]),
    ("geheimhalten", &["geheim halten"]),
    ("geheimhaltet", &["geheim haltet"]),
    ("geheimzuhalten", &["geheim zu halten"]),
    ("geheimgehalten", &["geheim gehalten"]),
    ("geheimhältst", &["geheim hältst"]),
    ("gleichlauten", &["gleich lauten"]),
    ("gutdünken", &["Gutdünken"]),
    ("langfahren", &["entlangfahren"]),
    ("langfuhren", &["entlangfuhren"]),
    ("langzufahren", &["entlangzufahren"]),
    ("langfahre", &["entlangfahre"]),
    ("langfährst", &["entlangfährst"]),
    ("langgefahren", &["entlanggefahren"]),
    ("langlaufen", &["entlanglaufen"]),
    ("langliefen", &["entlangliefen"]),
    ("langzulaufen", &["entlangzulaufen"]),
    ("langlaufe", &["entlanglaufe"]),
    ("langläufst", &["entlangläufst"]),
    ("langgelaufen", &["entlanggelaufen"]),
    ("langgehen", &["entlanggehen"]),
    ("langgingen", &["entlanggingen"]),
    ("langzugehen", &["entlangzugehen"]),
    ("langgehe", &["entlanggehe"]),
    ("langging", &["entlangging"]),
    ("Macbook", &["MacBook"]),
    ("Macbooks", &["MacBooks"]),
    ("langgegangen", &["entlanggegangen"]),
    ("lustigmachen", &["lustig machen"]),
    ("lustigmache", &["lustig mache"]),
    ("lustigmachst", &["lustig machst"]),
    ("lustigmachten", &["lustig machten"]),
    ("lustigzumachen", &["lustig zu machen"]),
    ("lustiggemacht", &["lustig gemacht"]),
    ("niederschlagreich", &["niederschlagsreich"]),
    ("niederschlagreiche", &["niederschlagsreiche"]),
    ("niederschlagreicher", &["niederschlagsreicher"]),
    ("niederschlagreiches", &["niederschlagsreiches"]),
    ("niederschlagreichem", &["niederschlagsreichem"]),
    ("niederschlagreichen", &["niederschlagsreichen"]),
    ("rechtgeben", &["recht geben"]),
    ("rechtzugeben", &["recht zu geben"]),
    ("rechtgegeben", &["recht gegeben"]),
    ("rechtgibst", &["recht gibst"]),
    ("rechtgibt", &["recht gibt"]),
    ("rechtgab", &["recht gab"]),
    ("rechthaben", &["recht haben"]),
    ("rechthabe", &["recht habe"]),
    ("rechtzuhaben", &["recht zu haben"]),
    ("rechtgehabt", &["recht gehabt"]),
    ("rechthatte", &["recht hatte"]),
    ("rechthast", &["recht hast"]),
    ("rechthabt", &["recht habt"]),
    ("rechtmachen", &["recht machen"]),
    ("rechtzumachen", &["recht zu machen"]),
    ("rechtgemacht", &["recht gemacht"]),
    ("rechtmacht", &["recht macht"]),
    ("rechtmache", &["recht mache"]),
    ("rechtmachte", &["recht machte"]),
    ("rechtmachten", &["recht machten"]),
    ("rechtmachst", &["recht machst"]),
    ("taubstellen", &["taub stellen"]),
    ("taubzustellen", &["taub zu stellen"]),
    ("taubgestellt", &["taub gestellt"]),
    ("taubstelle", &["taub stelle"]),
    ("taubstellt", &["taub stellt"]),
    ("taubstellst", &["taub stellst"]),
    ("wachgeblieben", &["wach geblieben"]),
    ("wachbleiben", &["wach bleiben"]),
    ("wachbleibe", &["wach bleibe"]),
    ("wachzubleiben", &["wach zu bleiben"]),
    ("wachbleibst", &["wach bleibst"]),
    ("wachblieb", &["wach blieb"]),
    ("wachblieben", &["wach blieben"]),
    ("ewiggleich", &["ewig gleich"]),
    ("ewiggleiche", &["ewig gleiche"]),
    ("ewiggleicher", &["ewig gleicher"]),
    ("ewiggleiches", &["ewig gleiches"]),
    ("ewiggleichem", &["ewig gleichem"]),
    ("ewiggleichen", &["ewig gleichen"]),
    ("sattessen", &["satt essen"]),
    ("gemäss", &["gemäß"]),
    ("upgedated", &["upgedatet"]),
    ("E.On", &["E.ON"]),
    ("E.on", &["E.ON"]),
    ("Juergen", &["Jürgen"]),
    ("deligieren", &["delegieren"]),
    ("deligiert", &["delegiert"]),
    ("telephonisch", &["telefonisch"]),
    ("telephonische", &["telefonische"]),
    ("telephonischen", &["telefonischen"]),
    ("telephonischem", &["telefonischem"]),
    ("telephonischer", &["telefonischer"]),
    ("telephonisches", &["telefonisches"]),
    ("beindruckend", &["beeindruckend"]),
    ("beindruckende", &["beeindruckende"]),
    ("beindruckender", &["beeindruckender"]),
    ("beindruckendes", &["beeindruckendes"]),
    ("beindruckenden", &["beeindruckenden"]),
    ("beindruckendem", &["beeindruckendem"]),
    ("beindruckt", &["beeindruckt"]),
    ("beindruckte", &["beeindruckte"]),
    ("heilsbringend", &["heilbringend"]),
    ("heilsbringende", &["heilbringende"]),
    ("heilsbringenden", &["heilbringenden"]),
    ("heilsbringendem", &["heilbringendem"]),
    ("heilsbringender", &["heilbringender"]),
    ("heilsbringendes", &["heilbringendes"]),
    ("vielfaltig", &["vielfältig"]),
    ("vielfaltige", &["vielfältige"]),
    ("vielfaltiger", &["vielfältiger"]),
    ("vielfaltiges", &["vielfältiges"]),
    ("vielfaltigen", &["vielfältigen"]),
    ("vielfaltigem", &["vielfältigem"]),
    ("barfuss", &["barfuß"]),
    ("nord-südlich", &["nordsüdlich"]),
    ("nord-südliche", &["nordsüdliche"]),
    ("nord-südlicher", &["nordsüdlicher"]),
    ("nord-südliches", &["nordsüdliches"]),
    ("nord-südlichen", &["nordsüdlichen"]),
    ("nord-südlichem", &["nordsüdlichem"]),
    ("nord-östlich", &["nordöstlich"]),
    ("nord-östliche", &["nordöstliche"]),
    ("nord-östlicher", &["nordöstlicher"]),
    ("nord-östliches", &["nordöstliches"]),
    ("nord-östlichen", &["nordöstlichen"]),
    ("nord-östlichem", &["nordöstlichem"]),
    ("nord-westlich", &["nordwestlich"]),
    ("nord-westliche", &["nordwestliche"]),
    ("nord-westlicher", &["nordwestlicher"]),
    ("nord-westliches", &["nordwestliches"]),
    ("nord-westlichen", &["nordwestlichen"]),
    ("nord-westlichem", &["nordwestlichem"]),
    ("süd-westlich", &["südwestlich"]),
    ("süd-westliche", &["südwestliche"]),
    ("süd-westlicher", &["südwestlicher"]),
    ("süd-westliches", &["südwestliches"]),
    ("süd-westlichen", &["südwestlichen"]),
    ("süd-westlichem", &["südwestlichem"]),
    ("süd-östlich", &["südöstlich"]),
    ("süd-östliche", &["südöstliche"]),
    ("süd-östlicher", &["südöstlicher"]),
    ("süd-östliches", &["südöstliches"]),
    ("süd-östlichen", &["südöstlichen"]),
    ("süd-östlichem", &["südöstlichem"]),
    ("ost-westlich", &["ostwestlich"]),
    ("ost-westliche", &["ostwestliche"]),
    ("ost-westlicher", &["ostwestlicher"]),
    ("ost-westliches", &["ostwestliches"]),
    ("ost-westlichen", &["ostwestlichen"]),
    ("ost-westlichem", &["ostwestlichem"]),
    ("afro-amerikanisch", &["afroamerikanisch"]),
    ("afro-amerikanische", &["afroamerikanische"]),
    ("afro-amerikanischer", &["afroamerikanischer"]),
    ("afro-amerikanisches", &["afroamerikanisches"]),
    ("afro-amerikanischen", &["afroamerikanischen"]),
    ("afro-amerikanischem", &["afroamerikanischem"]),
    ("tatsachlich", &["tatsächlich"]),
    ("tatsachliche", &["tatsächliche"]),
    ("tatsachlicher", &["tatsächlicher"]),
    ("tatsachliches", &["tatsächliches"]),
    ("tatsachlichen", &["tatsächlichen"]),
    ("tatsachlichem", &["tatsächlichem"]),
    ("ungelungen", &["misslungen"]),
    ("ungelungene", &["misslungene"]),
    ("ungelungener", &["misslungener"]),
    ("ungelungenes", &["misslungenes"]),
    ("ungelungenen", &["misslungenen"]),
    ("ungelungenem", &["misslungenem"]),
    ("totkrank", &["todkrank"]),
    ("totkranke", &["todkranke"]),
    ("totkranker", &["todkranker"]),
    ("totkrankes", &["todkrankes"]),
    ("totkranken", &["todkranken"]),
    ("totkrankem", &["todkrankem"]),
    ("SnapChat", &["Snapchat"]),
    ("SnapChats", &["Snapchats"]),
    ("jmd", &["jmd."]),
    ("Sparringpartner", &["Sparringspartner"]),
    ("Sparringpartners", &["Sparringspartners"]),
    ("Sparringpartnern", &["Sparringspartnern"]),
    ("ausserordentlich", &["außerordentlich"]),
    ("ausserordentliche", &["außerordentliche"]),
    ("ausserordentlichen", &["außerordentlichen"]),
    ("ausserordentlichem", &["außerordentlichem"]),
    ("ausserordentlicher", &["außerordentlicher"]),
    ("ausserordentliches", &["außerordentliches"]),
    ("unzippen", &["entzippen"]),
    ("unzippt", &["entzippt"]),
    ("unzippst", &["entzippst"]),
    ("unzippe", &["entzippe"]),
    ("Moet", &["Moët"]),
    ("Prigozhin", &["Prigoschin"]),
    ("Prigozhins", &["Prigoschins"]),
    ("unhilfreich", &["unbehilflich"]),
    ("gestriffen", &["gestreift"]),
    ("dererseits", &["ihrerseits"]),
    ("Regattas", &["Regatten"]),
    ("Segelregattas", &["Segelregatten"]),
    ("Brics-Staat", &["BRICS-Staat"]),
    ("Brics-Staats", &["BRICS-Staats"]),
    ("Brics-Staaten", &["BRICS-Staaten"]),
    ("Rene", &["René"]),
    ("Renes", &["Renés"]),
    ("einigermassen", &["einigermaßen"]),
    ("Eurocup", &["EuroCup"]),
    ("Eurocups", &["EuroCups"]),
    ("etc", &["etc."]),
    ("Ressorthotel", &["Resorthotel"]),
    ("Ressorthotels", &["Resorthotels"]),
    ("Kleidungstück", &["Kleidungsstück"]),
    ("Kleidungstücks", &["Kleidungsstücks"]),
    ("Kleidungstückes", &["Kleidungsstückes"]),
    ("Kleidungstücke", &["Kleidungsstücke"]),
    ("Kleidungstücken", &["Kleidungsstücken"]),
    ("unrentierlich", &["unrentabel"]),
    ("unrentierliche", &["unrentable"]),
    ("unrentierlicher", &["unrentabler"]),
    ("unrentierliches", &["unrentables"]),
    ("unrentierlichen", &["unrentablen"]),
    ("unrentierlichem", &["unrentablem"]),
    ("Hinterweltlerin", &["Hinterwäldlerin"]),
    ("Hinterweltlerinnen", &["Hinterwäldlerinnen"]),
    ("Hinterweltler", &["Hinterwäldler"]),
    ("Hinterweltlers", &["Hinterwäldlers"]),
    ("Hinterweltlern", &["Hinterwäldlern"]),
    ("erstrecht", &["erst recht"]),
    ("klangheimlich", &["klammheimlich"]),
    ("klangheimliche", &["klammheimliche"]),
    ("klangheimlicher", &["klammheimlicher"]),
    ("klangheimliches", &["klammheimliches"]),
    ("klangheimlichen", &["klammheimlichen"]),
    ("klangheimlichem", &["klammheimlichem"]),
    ("raufklicken", &["draufklicken"]),
    ("raufzuklicken", &["draufzuklicken"]),
    ("raufgeklickt", &["draufgeklickt"]),
    ("raufklicke", &["draufklicke"]),
    ("raufklickst", &["draufklickst"]),
    ("Aquaplanning", &["Aquaplaning"]),
    ("Aquaplannings", &["Aquaplanings"]),
    ("Kibbutz", &["Kibbuz"]),
    ("Prozentteil", &["Prozentanteil"]),
    ("Strebergarten", &["Schrebergarten"]),
    ("Strebergartens", &["Schrebergartens"]),
    ("Strebergärten", &["Schrebergärten"]),
    ("gunsten", &["Gunsten"]),
    ("ungunsten", &["Ungunsten"]),
    ("Situp", &["Sit-up"]),
    ("Situps", &["Sit-ups"]),
    ("Feb", &["Feb."]),
    ("Apr", &["Apr."]),
    ("Okt", &["Okt."]),
    ("Nov", &["Nov."]),
    ("aussen", &["außen"]),
    ("bestmöglichst", &["bestmöglich"]),
    ("nächstmöglichst", &["nächstmöglich"]),
    ("markaber", &["makaber"]),
    ("nachgeharkt", &["nachgehakt"]),
    ("nachgeharckt", &["nachgehackt"]),
    ("nachharken", &["nachhaken"]),
    ("nachhacken", &["nachhaken"]),
    ("Babies", &["Babys"]),
    ("Gummies", &["Gummis"]),
    ("Grüzi", &["Grüezi"]),
    ("fuer", &["für"]),
    ("Fuer", &["Für"]),
    ("Gruss", &["Gruß"]),
    ("nciht", &["nicht"]),
    ("heutejournal", &["heute journal"]),
    ("wikipedia", &["Wikipedia"]),
    ("Einfallspinsel", &["Einfaltspinsel"]),
    ("Einfallspinseln", &["Einfaltspinseln"]),
    ("Parcour", &["Parcours"]),
    ("Sommeliere", &["Sommelière"]),
    ("Sommeliére", &["Sommelière"]),
    ("Kosten-Nutzenanalyse", &["Kosten-Nutzen-Analyse"]),
    ("Kosten-Nutzenanalysen", &["Kosten-Nutzen-Analysen"]),
    ("Kosten-Nutzenverhältnis", &["Kosten-Nutzen-Verhältnis"]),
    ("Kosten-Nutzenverhältnisse", &["Kosten-Nutzen-Verhältnisse"]),
    (
        "Kosten-Nutzenverhältnisses",
        &["Kosten-Nutzen-Verhältnisses"],
    ),
    ("Kosten-Nutzenrechnung", &["Kosten-Nutzen-Rechnung"]),
    ("Kosten-Nutzenrechnungen", &["Kosten-Nutzen-Rechnungen"]),
    ("Brandwein", &["Branntwein"]),
    ("Brandweins", &["Branntweins"]),
    ("Brandweines", &["Branntweines"]),
    ("Brandweine", &["Branntweine"]),
    ("Brandweinen", &["Branntweinen"]),
    ("IfoInstitut", &["ifo Institut"]),
    ("IfoInstituts", &["ifo Instituts"]),
    ("ifoInstitut", &["ifo Institut"]),
    ("ifoInstituts", &["ifo Instituts"]),
    ("Ifo-Institut", &["ifo Institut"]),
    ("Ifo-Instituts", &["ifo Instituts"]),
    ("ifo-Institut", &["ifo Institut"]),
    ("ifo-Instituts", &["ifo Instituts"]),
    ("MacMini", &["Mac mini"]),
    ("MacMinis", &["Mac minis"]),
    ("Neisse-Verlag", &["Neisse Verlag"]),
    ("Neiße-Verlag", &["Neisse Verlag"]),
    ("Neisse-Verlags", &["Neisse Verlags"]),
    ("Neiße-Verlags", &["Neisse Verlags"]),
    ("weihnachten", &["Weihnachten"]),
    ("Carlsen-Verlag", &["Carlsen Verlag"]),
    ("Carlsen-Verlags", &["Carlsen Verlags"]),
    ("Sinnflut", &["Sintflut"]),
    ("Orginal", &["Original"]),
    ("Orginals", &["Originals"]),
    ("orginal", &["original"]),
    ("orginale", &["originale"]),
    ("orginalen", &["originalen"]),
    ("orginales", &["originales"]),
    ("Rundumsorglospaket", &["Rundum-sorglos-Paket"]),
    ("Rundumsorglospakets", &["Rundum-sorglos-Pakets"]),
    ("Fidji", &["Fidschi"]),
    ("tschüß", &["tschüss"]),
    ("Bautenzug", &["Bowdenzug"]),
    ("Bautenzugs", &["Bowdenzugs"]),
    ("Bautenzuges", &["Bowdenzuges"]),
    ("Bautenzüge", &["Bowdenzüge"]),
    ("Bautenzügen", &["Bowdenzügen"]),
    ("quillbot", &["QuillBot"]),
    ("quilbot", &["QuillBot"]),
    ("Quilbot", &["QuillBot"]),
    ("HeyLogin", &["heylogin"]),
    ("steinfarbenden", &["steinfarbenen"]),
    ("steinfarbende", &["steinfarbene"]),
    ("steinfarbender", &["steinfarbener"]),
    ("steinfarbendes", &["steinfarbenes"]),
    ("steinfarbendem", &["steinfarbenem"]),
    ("schwarzfarbenden", &["schwarzfarbenen"]),
    ("schwarzfarbende", &["schwarzfarbene"]),
    ("schwarzfarbender", &["schwarzfarbener"]),
    ("schwarzfarbendes", &["schwarzfarbenes"]),
    ("schwarzfarbendem", &["schwarzfarbenem"]),
    ("kupferfarbenden", &["kupferfarbenen"]),
    ("kupferfarbende", &["kupferfarbene"]),
    ("kupferfarbender", &["kupferfarbener"]),
    ("kupferfarbendes", &["kupferfarbenes"]),
    ("kupferfarbendem", &["kupferfarbenem"]),
    ("Außenriss", &["Außenrist"]),
    ("Innenriss", &["Innenrist"]),
    ("Lolly", &["Lolli"]),
    ("Lollys", &["Lollis"]),
    ("Lollies", &["Lollis"]),
];

/// `GermanSpellerRule.PREVENT_SUGGESTION_PATTERNS` (Java regex,
/// full-match semantics).
pub(crate) static PREVENT_SUGGESTION_PATTERNS: &[&str] = &[
    ".*(Majonäse|Bravur|Anschovis|Belkanto|Campagne|Frotté|Grisli|Jockei|Joga|Kalvinismus|Kanossa|Kargo|Ketschup|Kollier|Kommunikee|Masurka|Negligee|Nessessär|Poulard|Varietee|Wandalismus|kalvinist|[Ff]ick).*",
    ".+[*_:]in",
    ".+[*_:]innen",
    ".+\\szigste[srnm]?",
    "[\\wöäüÖÄÜß]+ [a-zöäüß]-[\\wöäüÖÄÜß]+",
    "[\\wöäüÖÄÜß]+- [\\wöäüÖÄÜß]+",
    "[A-ZÄÖÜ][a-zäöüß]+-[a-zäöüß]+-[a-zäöüß]+",
    "[A-ZÄÖÜ][a-zäöüß]+- [a-zäöüßA-ZÄÖÜ\\-]+",
    "[A-ZÄÖÜa-zäöüß\\-]+ [a-zäöüßA-ZÄÖÜ]-[a-zäöüßA-ZÄÖÜ\\-]+",
    "[A-ZÄÖÜa-zäöüß\\-]+ [a-zäöüß\\-]+-[A-ZÄÖÜ][a-zäöüß\\-]+",
    "[\\wöäüÖÄÜß]+ -[\\wöäüÖÄÜß]+",
    "[A-ZÄÖÜa-zäöüß\\-]+\\.[A-ZÄÖÜa-zäöüß][A-ZÄÖÜa-zäöüß\\-]+",
    "[A-ZÄÖÜa-zäöüß\\-]+\\.\\-[a-zäöüß\\-]+",
    "[a-zöäüß]{3,20} [A-ZÄÖÜ][a-zäöüß]{2,20}liche[rnsm]",
    "[A-ZÄÖÜ][a-zäöüß]{2,20}-[a-zäöüß]{2,20}-",
    "[a-zäöüß]{3,20}-[A-ZÄÖÜ][a-zäöüß\\-]{2,20}",
    "[a-zäöüß]{3,20}-[A-ZÄÖÜ\\-]{2,20}",
    "([skdm]?ein|viel|sitz|sing|web|hör|woh[nl]|kehr|adel|elektiv|wert|wein|wund|wurm|wand|weg|wett|gen|hei[lm]|kenn|vo[rnm]|fein|zu[rm]?|fehl|bei|peil|eckt?|mit|die|das|ehe|für|nur|eure[rn]?|unse?re?|e[sr]|fahr|bar|fern|warn|filz|oft|fort|bot|vote|käse|we[rnm]|was|gie(ss|ß)|haut|band|heiz|merk|mehr|z[äa]hl|knie|zie[lr]|braut|brat|park|reiz|wa[rs]|wo|ma(ß|ss)|kleb|gabel|brat|rast|rang|lesen?|arm|de[rnms]|sämig|sucht?|sägen?|steh|bahn|off|uff|auf|aß|also|anno|dank|back(en?)?|bl[oi]ck|fang|klär|macht?|haken?|[lw]agen?|messe?|bad(en?)?|pack|km|ecken?|bis|tauche?|tr?age?|segeln?|stei[lg]|stahl|da(nn)?|häng(en?)?[bt]oten?|plus|tat|lade?|tasten?|druck|fach|fragen?|lern|mag|facto|magre|bald|bau(en?)?|ich|sei[dtln]|gang|angeln?|[wl]ach|bist|[ge]ilt|warten?|turn|härten?|hold|[hg]alt|holt|angle|angab|ankam|anale?)-[A-ZÄÖÜa-zäöüß\\-]+",
    ".+-(gen|tu[etn]|l?ehrt?(en?)?|[fv]iele?n?|gärt?en?|igeln?|nein|ja|d?rum|erb(en?)?|vo[rnm]|vors|hat|gab(en)?|gabs?|gibt|km|geb(en?)?|nu[nr]|gay|kalt(e[snr]?)?|la[gd](en?)?|man|rängen?|nässen?|angle|angeln?|angst|stur(en?)?|oft|wo|wann|was|wer|mengen?|spie(ß|ss)en?|adeln?|näht?en?|ob|beide[rn]?|gärten|zweiten?|hütt?en?|kehrt?en?|h?orten?|messen?|tr[ea]u|trüb|trüben?|senden?|gr[uo]b|feinden?|wie|käsen?|ih[rmn](e[srnm]?)?|grau|trug(en?)?|weil|dass|sein?|zucken?|kanten?|s?ich|getan|hält|bald|ärgern?|fächern?|wart?(en?)?|leid|weit(e[snr]?)?|weiden?|ruf(en?)?|min|im|bin|zicken?|jo|siegeln?|[ao]ha|ganz|zäh|jäh|gehen?|ga[br]|kam|sah|[sr]itzen|kann|mit|ohne|ist|so|war|da[rh]in|über|unter|doof|bis|sie|er|aalen?|[lb]aden?|raten?|die|mit|bis|d[ea]s|eifern?|acker[tn]?|z[iu]cken?|j[oe]|jäh|haha|gerät|[wrbfk]etten?|tja|je|kau|nach|haben?|hab|gaga|kicken?|kick|heil|heilen?|altern?|wänden?|wert(e[rsnm]?)?|werben?|zoom|genug|gehen?|ums?|und|oder|[sn]ah|ha|de[mnsr]|sü(ß|ss)|ringen?|dingen?|seil|au[fs]|gurten?|munden?|eigen|wenden?|regen?|b?rechen?|legen?|fächern?|leger|g[ia]lt|heim|heimen?|[mksdw]?ein|[mksdw]?einen?|erden?|ändern?|ernten?|bänden?|ästen?|arten?|kanten?|eichen?|unken?|wunden?|kunden?|runden?|regeln?|kegeln?|krähen?|zechen?|mähen?|ehren?|ehen?|enden?|eng(e[srn]?)?|gut(e[srn]?)?|zielt?(en?)?|spielt?(en?)?|ätzt?(en?)?|riegeln?|segeln?|engt?|engen?|angeln?|kochen?|[lk]ehren?|festen?|essen?|steuern?|ekeln?|irren?|cum|de|da|du|raus|rein|dort|knien?|hin|zu[rm]?|ritten?|riss|rissen?|[tr]ast(en?)?|rasseln?|hieb|wässern?|putz|hängen?|zinken?|a[bnm]|bisher|schöne?|solo|haken?|dr[üu]ck(en?|tot)?|huren?|pries|hupen?|hüllen?|lang|joa|sei[dt]|weist|üben?|ufern?|iss|steck(en?)?|fort|mal|aal|darf|halt(en?)?|eifern?|van|guck(en?|t)?|ganze?|acht(en?)?|auch|solo|[zs]og|lagern?|baggern?|au|haut?|als|uns|bei[m]?|[dm]ir|dich|uni|ergo|eich(en?)?|spick(en?)?|e[rs]|spielt?|we[hg]|wart|wi[rl]d|neue[rns]?|mithin|tags?|eine[snmr]?|wiesen?|rei[sz]en?|wei[sh]en?|siegen?|sag(en?)?|sitzen?|tagen?|all(en?)?|zahlen?|rügen?|ruhen?|bar|hüben?|hick|arm|armen?|plan(en?)?|[fpl]assen?|per|reg|rinnen?|bringen?|öl(en?)?|alt(en?)?|elf(en?)?|kp|ward|apart|wer[dkt](en?)?|weis(en?)?|sind|mm|wand|wir|licht(en)?|lügen?|loch(en?)?|übel|peu|[wtm]isch(en?)?|fein(e[rns]?)?|a(ß|ss)|mol|neu(en?)?|[dm]ich|rang|obe[nr]|übe[nl]?|maxi?|hart(en?)?|hexen?|ab|zück(en?)?|zurück|köpf(en?)?|band(en?)?|schafft?en?|schalt?en?|giften?|sieben?|seil(en?)?|wehen?|sehen?|s[it]?eht?|stocken?|red|rät|ma(ß|ss)|schämen?|innen?|karren?|wer[tf]en?|werft|loch(en?)?|logen?|gossen?|steil(en?)?|fr?isch(en?)?|d[ea]nn|zelt(en?)?|luv|kauf(en?)?|lasch(en?)?|bei(ß|ss)(en?)?|leihen?|leid(en?)?|[drsl]icht(en?)?|opfern?|[wz]äh[mln]en?|wär(en?)?|À|à|fugen?|la[xs]|zahl(en?)?|[rf]all(en?)?|wichs(en?)?|sog(en?)?|alias|glich(en?)?|würd(en?)?|wärm(en?)?|[rhg]eiz(en?)?|stieren?|teils?|trotz|fahr(en?)?|b[oa]u?[dt](en?)?|kl[öo]n(en?)?|paar|park(en?)?|last|landen?|alle[rnms]?|ad|l[äa]u[ft](en?)?|[ws]äg(en?)?|pasch(en?)?|kehl(en?)?|wohl(en?)?|flucht?(en?)?|zeit|rasa|selben?|mehr(en?)?|gabeln?|ordern?|[cw]ach(en?)?|arg(en?)?|brauch(en?)?|hauch(en?)?|[ms]a(ß|ss)(en?)?|mm?h|zart(e[snmr]?)?|ehrt?(en?)?|de[rn]en|ähm?|hui|hmm?|al|für|[bl]au(en?)?|[lr]ahm(en?)?|[bs]uch(en?)?|[wv]ag(en?)?|[tl]os(en?)?|les(en?)?|str?ahl(en?)?|zäh[mn]t?(en?)?|fest(e[rsnm]?)?|folgt?(en?)?|f[aä]llt?(en?)?|[tr]oll(en?)?|[mf]üllt?(en?)?|[rl]eit(en?)?|ras(en?)?|hall(en?)?|well(en?)?|fra(ß|ss)(en)?|tat(en)?|pah|buh(en?)?|bäh|hör(en?)?|holz(en?)?|reif(e[rsmn]?)?|litt|fort(an)?|härten?|welche[rnsm]?|wegen|fach(en?)?|bog(en?)?|foul(en?)?|löst?(en?)?|lots(en?)?|falls|[bwh][ua]ldige[rsn]?|(st)?reift?(en?)?|t?rei[bh](en?)?|[rb]ück(en?)?|wett(en?)?|t[oü]t(en?)?|[ft]est(en?)?|h[aä]ut(en?)?|knall(en?)?|[dk]ämpft?(en?)?|hört?(en?)?|patt(en?)?|[tw]ollt?en?|[km]g|[bkps]ack(en?)?|[lf]an?d(en?)?|seifen?|tabu|heft(en?)?|forma?|knall(en?)?|[lm]?acht?(en)?|boot(en?)?|lach(en?)?|[hb]i?eb(en?)?|tut(en?)?|tr?öt(e[tn]?)?|[sp]ackt?(en?)?|[klnrd]?eckt?(en?)?|beut(en?)?|top|st?att(en?)?|dien(en?)?|[hl]ieb(en?)?|sät|satt(en?)?|droh(en?)?|[sr]äum(en?)?|zeugt?(en?)?|reu(en?)?|nies(en?)?|[gzf]eigt?(en?)?|gie(ß|ss)(en?)?|sichern?|zog(en?)?|schert?(en?)?|s[tp]r?ickt?(en?)?|seicht(e[srn]?)?|(be)?sorgt?(en?)?|ehelich(en?)?|link(en?)?|wein(en?)?|r?echt|orangen?|blick(en?)?|kling(en?)?|übrig(en?)?|klick(en?)?)",
    "[A-ZÖÄÜa-zöäüß] .+",
    ".+ [a-zöäüßA-ZÖÄÜ]",
];
