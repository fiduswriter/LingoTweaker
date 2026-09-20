//! Port of `org.languagetool.tagging.de.GermanTagger` (+ `SwissGermanTagger`):
//! the CFSA2 dictionary tagger with manual added/removed lists, the
//! spelling.txt-driven expansion infos (prefixed verbs, nominalized verbs,
//! adjectives), compound decomposition and the case/imperative/substantivation
//! heuristics.
//!
//! The Java `tag(List, boolean)` method is ported branch by branch; deliberate
//! Java quirks (e.g. `List.contains(String)` always being false) are kept and
//! marked.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use lt_core::{AnalyzedToken, AnalyzedTokenReadings, CoreError, Result};
use lt_tokenize::GermanCompoundTokenizer;

use crate::german_synth::GermanSynthesizer;
use crate::{
    is_capitalized_word, lowercase_first_char, uppercase_first_char, Dictionary, DictionaryInfo,
    ManualTagger,
};

/// `GermanTagger.nounTagExpansionExceptions`.
const NOUN_TAG_EXPANSION_EXCEPTIONS: [&str; 1] = ["Wegstrecken"];

/// Ordered list of separable verb prefixes (`prefixesSeparableVerbs`).
const PREFIXES_SEPARABLE_VERBS: &[&str] = &[
    "gegeneinander",
    "durcheinander",
    "nebeneinander",
    "übereinander",
    "aufeinander",
    "auseinander",
    "beieinander",
    "aneinander",
    "ineinander",
    "zueinander",
    "gegenüber",
    "beisammen",
    "gegenüber",
    "hernieder",
    "rückwärts",
    "wiederauf",
    "wiederein",
    "wiederher",
    "zufrieden",
    "zwangsvor",
    "entgegen",
    "hinunter",
    "abhanden",
    "aufrecht",
    "aufwärts",
    "auswärts",
    "beiseite",
    "danieder",
    "drauflos",
    "einwärts",
    "herunter",
    "hindurch",
    "verrückt",
    "vorwärts",
    "zunichte",
    "zusammen",
    "zwangsum",
    "zwischen",
    "abseits",
    "abwärts",
    "entlang",
    "hinfort",
    "ähnlich",
    "daneben",
    "general",
    "herüber",
    "hierher",
    "hierhin",
    "hinüber",
    "schwarz",
    "trocken",
    "überein",
    "vorlieb",
    "vorüber",
    "wichtig",
    "zurecht",
    "zuwider",
    "hinweg",
    "allein",
    "besser",
    "daheim",
    "doppel",
    "feinst",
    "fertig",
    "herauf",
    "heraus",
    "herbei",
    "hinauf",
    "hinaus",
    "hinein",
    "kaputt",
    "kennen",
    "kürzer",
    "mittag",
    "nieder",
    "runter",
    "sicher",
    "sitzen",
    "voraus",
    "vorbei",
    "vorweg",
    "weiter",
    "wieder",
    "zugute",
    "zurück",
    "zwangs",
    "abend",
    "blank",
    "brust",
    "dahin",
    "davon",
    "drauf",
    "drein",
    "durch",
    "einig",
    "empor",
    "grund",
    "herum",
    "höher",
    "klein",
    "knapp",
    "krank",
    "krumm",
    "kugel",
    "näher",
    "neben",
    "offen",
    "preis",
    "rüber",
    "ruhig",
    "statt",
    "still",
    "übrig",
    "umher",
    "unter",
    "voran",
    "zweck",
    "acht",
    "drei",
    "fehl",
    "feil",
    "fort",
    "frei",
    "groß",
    "hand",
    "hart",
    "heim",
    "hier",
    "hoch",
    "klar",
    "lahm",
    "miss",
    "nach",
    "nahe",
    "quer",
    "rauf",
    "raus",
    "rein",
    "rück",
    "satt",
    "stoß",
    "teil",
    "über",
    "voll",
    "wach",
    "wahr",
    "warm",
    "wert",
    "wohl",
    "auf",
    "aus",
    "bei",
    "ehe",
    "ein",
    "eis",
    "end",
    "her",
    "hin",
    "los",
    "maß",
    "mit",
    "out",
    "ran",
    "rum",
    "tot",
    "vor",
    "weg",
    "weh",
    "ab",
    "an",
    "da",
    "um",
    "zu",
];

const PREFIXES_NON_SEPARABLE_VERBS: &[&str] = &[
    "be", "emp", "ent", "er", "hinter", "miss", "un", "ver", "zer",
];

const PREFIXES_NON_SEPARABLE_VERBS_REGEXP: &str = "^(be|emp|ent|er|hinter|miss|un|ver|zer)";

/// `GermanTagger.prefixesVerbs` (separables plus a few non-separable prefixes).
const PREFIXES_VERBS: &[&str] = &[
    "gegeneinander",
    "durcheinander",
    "nebeneinander",
    "übereinander",
    "aufeinander",
    "auseinander",
    "beieinander",
    "aneinander",
    "ineinander",
    "zueinander",
    "gegenüber",
    "beisammen",
    "gegenüber",
    "hernieder",
    "rückwärts",
    "wiederauf",
    "wiederein",
    "wiederher",
    "zufrieden",
    "zwangsvor",
    "entgegen",
    "hinunter",
    "abhanden",
    "aufrecht",
    "aufwärts",
    "auswärts",
    "beiseite",
    "danieder",
    "drauflos",
    "einwärts",
    "herunter",
    "hindurch",
    "verrückt",
    "vorwärts",
    "zunichte",
    "zusammen",
    "zwangsum",
    "zwischen",
    "abseits",
    "abwärts",
    "entlang",
    "hinfort",
    "ähnlich",
    "daneben",
    "general",
    "herüber",
    "hierher",
    "hierhin",
    "hinüber",
    "schwarz",
    "trocken",
    "überein",
    "vorlieb",
    "vorüber",
    "wichtig",
    "zurecht",
    "zuwider",
    "hinweg",
    "hinter",
    "allein",
    "besser",
    "daheim",
    "doppel",
    "feinst",
    "fertig",
    "herauf",
    "heraus",
    "herbei",
    "hinauf",
    "hinaus",
    "hinein",
    "kaputt",
    "kennen",
    "kürzer",
    "mittag",
    "nieder",
    "runter",
    "sicher",
    "sitzen",
    "voraus",
    "vorbei",
    "vorweg",
    "weiter",
    "wieder",
    "zugute",
    "zurück",
    "zwangs",
    "abend",
    "blank",
    "brust",
    "dahin",
    "davon",
    "drauf",
    "drein",
    "durch",
    "einig",
    "empor",
    "grund",
    "herum",
    "höher",
    "klein",
    "knapp",
    "krank",
    "krumm",
    "kugel",
    "näher",
    "neben",
    "offen",
    "preis",
    "rüber",
    "ruhig",
    "statt",
    "still",
    "übrig",
    "umher",
    "unter",
    "voran",
    "zweck",
    "miss",
    "acht",
    "drei",
    "fehl",
    "feil",
    "fort",
    "frei",
    "groß",
    "hand",
    "hart",
    "heim",
    "hier",
    "hoch",
    "klar",
    "lahm",
    "miss",
    "nach",
    "nahe",
    "quer",
    "rauf",
    "raus",
    "rein",
    "rück",
    "satt",
    "stoß",
    "teil",
    "über",
    "voll",
    "wach",
    "wahr",
    "warm",
    "wert",
    "wohl",
    "emp",
    "ent",
    "ver",
    "zer",
    "auf",
    "aus",
    "bei",
    "ehe",
    "ein",
    "eis",
    "end",
    "her",
    "hin",
    "los",
    "maß",
    "mit",
    "out",
    "ran",
    "rum",
    "tot",
    "vor",
    "weg",
    "weh",
    "be",
    "er",
    "un",
    "ab",
    "an",
    "da",
    "um",
    "zu",
];

const PREFIXES_VERBS_REGEXP: &str = "^(gegeneinander|durcheinander|nebeneinander|übereinander|aufeinander|auseinander|beieinander|aneinander|ineinander|zueinander|gegenüber|beisammen|gegenüber|hernieder|rückwärts|wiederauf|wiederein|wiederher|zufrieden|zwangsvor|entgegen|hinunter|abhanden|aufrecht|aufwärts|auswärts|beiseite|danieder|drauflos|einwärts|herunter|hindurch|verrückt|vorwärts|zunichte|zusammen|zwangsum|zwischen|abseits|abwärts|entlang|hinfort|ähnlich|daneben|general|herüber|hierher|hierhin|hinüber|schwarz|trocken|überein|vorlieb|vorüber|wichtig|zurecht|zuwider|hinweg|hinter|allein|besser|daheim|doppel|feinst|fertig|herauf|heraus|herbei|hinauf|hinaus|hinein|kaputt|kennen|kürzer|mittag|nieder|runter|sicher|sitzen|voraus|vorbei|vorweg|weiter|wieder|zugute|zurück|zwangs|abend|blank|brust|dahin|davon|drauf|drein|durch|einig|empor|grund|herum|höher|klein|knapp|krank|krumm|kugel|näher|neben|offen|preis|rüber|ruhig|statt|still|übrig|umher|unter|voran|zweck|miss|acht|drei|fehl|feil|fort|frei|groß|hand|hart|heim|hier|hoch|klar|lahm|miss|nach|nahe|quer|rauf|raus|rein|rück|satt|stoß|teil|über|voll|wach|wahr|warm|wert|wohl|emp|ent|ver|zer|auf|aus|bei|ehe|ein|eis|end|her|hin|los|maß|mit|not|out|ran|rum|tot|vor|weg|weh|be|er|un|ab|an|da|um|zu)";

const PARTIZIP2_CONTAINS_1_PLU_PRA: &[&str] = &[
    "blasen", "fahren", "fallen", "fangen", "fressen", "geben", "halten", "kommen", "laden",
    "lassen", "laufen", "lesen", "messen", "raten", "schlafen", "schlagen", "sehen", "tragen",
    "treten",
];

const PARTIZIP2_CONTAINS_1_PLU_PRT: &[&str] = &[
    "bieten",
    "bleiben",
    "fliegen",
    "fließen",
    "heben",
    "leiden",
    "meiden",
    "scheiden",
    "schließen",
    "schreiben",
    "stehen",
    "steigen",
    "streiten",
    "treiben",
    "weisen",
    "ziehen",
];

const POSTAGS_PARTIZIP_ENDING_E: &[&str] = &[
    "AKK:PLU:FEM:GRU:SOL:VER",
    "AKK:PLU:MAS:GRU:SOL:VER",
    "AKK:PLU:NEU:GRU:SOL:VER",
    "AKK:SIN:FEM:GRU:DEF:VER",
    "AKK:SIN:FEM:GRU:IND:VER",
    "AKK:SIN:FEM:GRU:SOL:VER",
    "AKK:SIN:NEU:GRU:DEF:VER",
    "NOM:PLU:FEM:GRU:SOL:VER",
    "NOM:PLU:MAS:GRU:SOL:VER",
    "NOM:PLU:NEU:GRU:SOL:VER",
    "NOM:SIN:FEM:GRU:DEF:VER",
    "NOM:SIN:FEM:GRU:IND:VER",
    "NOM:SIN:FEM:GRU:SOL:VER",
    "NOM:SIN:MAS:GRU:DEF:VER",
    "NOM:SIN:NEU:GRU:DEF:VER",
];

const POSTAGS_PARTIZIP_ENDING_EM: &[&str] = &["DAT:SIN:MAS:GRU:SOL:VER", "DAT:SIN:NEU:GRU:SOL:VER"];

const POSTAGS_PARTIZIP_ENDING_EN: &[&str] = &[
    "AKK:PLU:FEM:GRU:DEF:VER",
    "AKK:PLU:FEM:GRU:IND:VER",
    "AKK:PLU:MAS:GRU:DEF:VER",
    "AKK:PLU:MAS:GRU:IND:VER",
    "AKK:PLU:NEU:GRU:DEF:VER",
    "AKK:PLU:NEU:GRU:IND:VER",
    "AKK:SIN:MAS:GRU:DEF:VER",
    "AKK:SIN:MAS:GRU:IND:VER",
    "AKK:SIN:MAS:GRU:SOL:VER",
    "DAT:PLU:FEM:GRU:DEF:VER",
    "DAT:PLU:FEM:GRU:IND:VER",
    "DAT:PLU:FEM:GRU:SOL:VER",
    "DAT:PLU:MAS:GRU:DEF:VER",
    "DAT:PLU:MAS:GRU:IND:VER",
    "DAT:PLU:MAS:GRU:SOL:VER",
    "DAT:PLU:NEU:GRU:DEF:VER",
    "DAT:PLU:NEU:GRU:IND:VER",
    "DAT:PLU:NEU:GRU:SOL:VER",
    "DAT:SIN:FEM:GRU:DEF:VER",
    "DAT:SIN:FEM:GRU:IND:VER",
    "DAT:SIN:MAS:GRU:DEF:VER",
    "DAT:SIN:MAS:GRU:IND:VER",
    "DAT:SIN:NEU:GRU:DEF:VER",
    "DAT:SIN:NEU:GRU:IND:VER",
    "GEN:PLU:FEM:GRU:DEF:VER",
    "GEN:PLU:FEM:GRU:IND:VER",
    "GEN:PLU:MAS:GRU:DEF:VER",
    "GEN:PLU:MAS:GRU:IND:VER",
    "GEN:PLU:NEU:GRU:DEF:VER",
    "GEN:PLU:NEU:GRU:IND:VER",
    "GEN:SIN:FEM:GRU:DEF:VER",
    "GEN:SIN:FEM:GRU:IND:VER",
    "GEN:SIN:MAS:GRU:DEF:VER",
    "GEN:SIN:MAS:GRU:IND:VER",
    "GEN:SIN:MAS:GRU:SOL:VER",
    "GEN:SIN:NEU:GRU:DEF:VER",
    "GEN:SIN:NEU:GRU:IND:VER",
    "GEN:SIN:NEU:GRU:SOL:VER",
    "NOM:PLU:FEM:GRU:DEF:VER",
    "NOM:PLU:FEM:GRU:IND:VER",
    "NOM:PLU:MAS:GRU:DEF:VER",
    "NOM:PLU:MAS:GRU:IND:VER",
    "NOM:PLU:NEU:GRU:DEF:VER",
    "NOM:PLU:NEU:GRU:IND:VER",
];

const POSTAGS_PARTIZIP_ENDING_ER: &[&str] = &[
    "DAT:SIN:FEM:GRU:SOL:VER",
    "GEN:PLU:FEM:GRU:SOL:VER",
    "GEN:PLU:MAS:GRU:SOL:VER",
    "GEN:PLU:NEU:GRU:SOL:VER",
    "GEN:SIN:FEM:GRU:SOL:VER",
    "NOM:SIN:MAS:GRU:IND:VER",
    "NOM:SIN:MAS:GRU:SOL:VER",
];

const POSTAGS_PARTIZIP_ENDING_ES: &[&str] = &[
    "AKK:SIN:NEU:GRU:IND:VER",
    "AKK:SIN:NEU:GRU:SOL:VER",
    "NOM:SIN:NEU:GRU:IND:VER",
    "NOM:SIN:NEU:GRU:SOL:VER",
];

const NOT_A_VERB: &[&str] = &[
    "angebot",
    "anteil",
    "aufenthalt",
    "ausdruck",
    "auswärtsspiel",
    "beispiel",
    "bereich",
    "besondere",
    "daring",
    "einfach",
    "einfachst",
    "endkasten",
    "freibetrag",
    "grautöne",
    "grüntöne",
    "großherzöge",
    "großteil",
    "hochhaus",
    "klarerweise",
    "maßnahme",
    "mitglieder",
    "nachricht",
    "nebenfach",
    "niederlage",
    "nothing",
    "notscheid",
    "preisver",
    "reinweiß",
    "schwarzweiß",
    "schwarzgrau",
    "schwarzgrün",
    "schwarztöne",
    "unbesiegt",
    "unmenge",
    "unrat",
    "unver",
    "verrückterweise",
    "versonnen",
    "vorlieb",
    "vorteil",
    "warmweiß",
    "wohldefiniert",
    "wohlergehen",
    "wohlgemerkt",
    "zuende",
    "zuhause",
    "zumal",
    "zuver",
    "darauf",
    "einmal",
    "kleinkram",
    "hochsicher",
    "ehering",
    "freitag",
    "großmeister",
    "handwerk",
    "herpes",
    "nachfolger",
];

/// `GermanTagger.tagsForWeise`.
fn tags_for_weise() -> Vec<&'static str> {
    vec![
        "ADJ:AKK:PLU:FEM:GRU:SOL",
        "ADJ:AKK:PLU:MAS:GRU:SOL",
        "ADJ:AKK:PLU:NEU:GRU:SOL",
        "ADJ:AKK:SIN:FEM:GRU:DEF",
        "ADJ:AKK:SIN:FEM:GRU:IND",
        "ADJ:AKK:SIN:FEM:GRU:SOL",
        "ADJ:AKK:SIN:NEU:GRU:DEF",
        "ADJ:NOM:PLU:FEM:GRU:SOL",
        "ADJ:NOM:PLU:MAS:GRU:SOL",
        "ADJ:NOM:PLU:NEU:GRU:SOL",
        "ADJ:NOM:SIN:FEM:GRU:DEF",
        "ADJ:NOM:SIN:FEM:GRU:IND",
        "ADJ:NOM:SIN:FEM:GRU:SOL",
        "ADJ:NOM:SIN:MAS:GRU:DEF",
        "ADJ:NOM:SIN:NEU:GRU:DEF",
        "ADJ:PRD:GRU",
    ]
}

/// `AdjectiveTags.tagsForAdj*` (`toPA2` = `ADJ:`→`PA2:` plus `:VER`).
const ADJ_TAGS_ADJ: &[&str] = &["ADJ:PRD:GRU"];
const ADJ_TAGS_ADJ_E: &[&str] = &[
    "ADJ:AKK:PLU:FEM:GRU:SOL",
    "ADJ:AKK:PLU:MAS:GRU:SOL",
    "ADJ:AKK:PLU:NEU:GRU:SOL",
    "ADJ:AKK:SIN:FEM:GRU:DEF",
    "ADJ:AKK:SIN:FEM:GRU:IND",
    "ADJ:AKK:SIN:FEM:GRU:SOL",
    "ADJ:AKK:SIN:NEU:GRU:DEF",
    "ADJ:NOM:PLU:FEM:GRU:SOL",
    "ADJ:NOM:PLU:MAS:GRU:SOL",
    "ADJ:NOM:PLU:NEU:GRU:SOL",
    "ADJ:NOM:SIN:FEM:GRU:DEF",
    "ADJ:NOM:SIN:FEM:GRU:IND",
    "ADJ:NOM:SIN:FEM:GRU:SOL",
    "ADJ:NOM:SIN:MAS:GRU:DEF",
    "ADJ:NOM:SIN:NEU:GRU:DEF",
];
const ADJ_TAGS_ADJ_EN: &[&str] = &[
    "ADJ:AKK:PLU:FEM:GRU:DEF",
    "ADJ:AKK:PLU:FEM:GRU:IND",
    "ADJ:AKK:PLU:MAS:GRU:DEF",
    "ADJ:AKK:PLU:MAS:GRU:IND",
    "ADJ:AKK:PLU:NEU:GRU:DEF",
    "ADJ:AKK:PLU:NEU:GRU:IND",
    "ADJ:AKK:SIN:MAS:GRU:DEF",
    "ADJ:AKK:SIN:MAS:GRU:IND",
    "ADJ:AKK:SIN:MAS:GRU:SOL",
    "ADJ:DAT:PLU:FEM:GRU:DEF",
    "ADJ:DAT:PLU:FEM:GRU:IND",
    "ADJ:DAT:PLU:FEM:GRU:SOL",
    "ADJ:DAT:PLU:MAS:GRU:DEF",
    "ADJ:DAT:PLU:MAS:GRU:IND",
    "ADJ:DAT:PLU:MAS:GRU:SOL",
    "ADJ:DAT:PLU:NEU:GRU:DEF",
    "ADJ:DAT:PLU:NEU:GRU:IND",
    "ADJ:DAT:PLU:NEU:GRU:SOL",
    "ADJ:DAT:SIN:FEM:GRU:DEF",
    "ADJ:DAT:SIN:FEM:GRU:IND",
    "ADJ:DAT:SIN:MAS:GRU:DEF",
    "ADJ:DAT:SIN:MAS:GRU:IND",
    "ADJ:DAT:SIN:NEU:GRU:DEF",
    "ADJ:DAT:SIN:NEU:GRU:IND",
    "ADJ:GEN:PLU:FEM:GRU:DEF",
    "ADJ:GEN:PLU:FEM:GRU:IND",
    "ADJ:GEN:PLU:MAS:GRU:DEF",
    "ADJ:GEN:PLU:MAS:GRU:IND",
    "ADJ:GEN:PLU:NEU:GRU:DEF",
    "ADJ:GEN:PLU:NEU:GRU:IND",
    "ADJ:GEN:SIN:FEM:GRU:DEF",
    "ADJ:GEN:SIN:FEM:GRU:IND",
    "ADJ:GEN:SIN:MAS:GRU:DEF",
    "ADJ:GEN:SIN:MAS:GRU:IND",
    "ADJ:GEN:SIN:MAS:GRU:SOL",
    "ADJ:GEN:SIN:NEU:GRU:DEF",
    "ADJ:GEN:SIN:NEU:GRU:IND",
    "ADJ:GEN:SIN:NEU:GRU:SOL",
    "ADJ:NOM:PLU:FEM:GRU:DEF",
    "ADJ:NOM:PLU:FEM:GRU:IND",
    "ADJ:NOM:PLU:MAS:GRU:DEF",
    "ADJ:NOM:PLU:MAS:GRU:IND",
    "ADJ:NOM:PLU:NEU:GRU:DEF",
    "ADJ:NOM:PLU:NEU:GRU:IND",
];
const ADJ_TAGS_ADJ_ER: &[&str] = &[
    "ADJ:DAT:SIN:FEM:GRU:SOL",
    "ADJ:GEN:PLU:FEM:GRU:SOL",
    "ADJ:GEN:PLU:MAS:GRU:SOL",
    "ADJ:GEN:PLU:NEU:GRU:SOL",
    "ADJ:GEN:SIN:FEM:GRU:SOL",
    "ADJ:NOM:SIN:MAS:GRU:IND",
    "ADJ:NOM:SIN:MAS:GRU:SOL",
];
const ADJ_TAGS_ADJ_EM: &[&str] = &["ADJ:DAT:SIN:MAS:GRU:SOL", "ADJ:DAT:SIN:NEU:GRU:SOL"];
const ADJ_TAGS_ADJ_ES: &[&str] = &[
    "ADJ:AKK:SIN:NEU:GRU:IND",
    "ADJ:AKK:SIN:NEU:GRU:SOL",
    "ADJ:NOM:SIN:NEU:GRU:IND",
    "ADJ:NOM:SIN:NEU:GRU:SOL",
];

/// `allAdjGruTags`: the 4×2×3×3 ADJ tag combinations.
fn all_adj_gru_tags() -> Vec<String> {
    let mut tags = Vec::new();
    for nom_akk_gen_dat in ["NOM", "AKK", "GEN", "DAT"] {
        for plu_sin in ["PLU", "SIN"] {
            for mas_fem_neu in ["MAS", "FEM", "NEU"] {
                for def_ind_sol in ["DEF", "IND", "SOL"] {
                    tags.push(format!(
                        "ADJ:{nom_akk_gen_dat}:{plu_sin}:{mas_fem_neu}:GRU:{def_ind_sol}"
                    ));
                }
            }
        }
    }
    tags
}

const DDD_ER_PATTERN: &str = r"\d{4}+er";
const MITARBEITENDEN_PATTERN: &str = "[A-ZÖÄÜ][a-zöäüß]{2,25}mitarbeitenden?";
const GENDER_GAP_CHARS: &str = "[*:_/]";
const AFTER_ASTERISK: &str = "in(nen)?|r|e";
const INNEN_PATTERN_1: &str = "in(nen)-[A-ZÖÄÜ][a-zöäüß-]+";
const ANYTHING_DASH: &str = ".*-";
const INNEN_PATTERN_2: &str = "innen[a-zöäüß-]+";
const BITTER_PREFIX_REGEXP: &str =
    "^(bitter|dunkel|erz|extra|früh|gemein|grund|hyper|lau|mega|minder|stock|super|tod|ultra|u[nr]|voll)";
const DOMAIN_TLDS: &str = "com|net|org|de|at|ch|fr|uk|gov";
const I_MATCH: &str = r".*i.+";

struct PrefixInfixVerb {
    prefix: String,
    infix: String,
    verb_baseform: String,
}

struct NominalizedVerb {
    prefix: String,
    verb_baseform: String,
}

struct NominalizedGenitiveVerb {
    prefix: String,
    verb_baseform: String,
}

struct AdjInfo {
    baseform: String,
    full_form: String,
    tag: String,
}

#[derive(Default)]
struct ExpansionInfos {
    verb_infos: HashMap<String, PrefixInfixVerb>,
    nominalized_verb_infos: HashMap<String, NominalizedVerb>,
    nominalized_gen_verb_infos: HashMap<String, NominalizedGenitiveVerb>,
    adj_infos: HashMap<String, Vec<AdjInfo>>,
}

fn to_pa2(tags: &[&str]) -> Vec<String> {
    tags.iter()
        .map(|t| t.replace("ADJ:", "PA2:") + ":VER")
        .collect()
}

fn fill_adj_infos(
    word: &str,
    suffix: &str,
    tags_for_form: &[String],
    adj_infos: &mut HashMap<String, Vec<AdjInfo>>,
) {
    let full_form = format!("{word}{suffix}");
    let entries: Vec<AdjInfo> = tags_for_form
        .iter()
        .map(|tag| AdjInfo {
            baseform: word.to_string(),
            full_form: full_form.clone(),
            tag: tag.clone(),
        })
        .collect();
    adj_infos.insert(full_form, entries);
}

fn init_expansion_infos(data_dir: &Path, synth: &GermanSynthesizer) -> Result<ExpansionInfos> {
    let mut infos = ExpansionInfos::default();
    let path = data_dir.join("de/hunspell/spelling.txt");
    let text = lt_data::fs::read_to_string(&path)
        .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", path.display())))?;
    let mut spelling_words: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let trimmed = line.split('#').next().unwrap_or("").trim();
        if !trimmed.is_empty() {
            spelling_words.push(trimmed.to_string());
        }
    }
    let pa2_adj = to_pa2(ADJ_TAGS_ADJ);
    let pa2_adj_e = to_pa2(ADJ_TAGS_ADJ_E);
    let pa2_adj_en = to_pa2(ADJ_TAGS_ADJ_EN);
    let pa2_adj_er = to_pa2(ADJ_TAGS_ADJ_ER);
    let pa2_adj_em = to_pa2(ADJ_TAGS_ADJ_EM);
    let pa2_adj_es = to_pa2(ADJ_TAGS_ADJ_ES);
    for line in &spelling_words {
        if line.ends_with("/PA") || line.ends_with("/AP") {
            return Err(CoreError::Data(format!(
                "Use '/P' or '/A', but not both for a word in spelling.txt: {line}"
            )));
        }
        let is_adj_line = line.ends_with("/P")
            || (line.ends_with("/A") && !line.ends_with("ste/A") && !line.ends_with("er/A"));
        if is_adj_line {
            let word = line.split('/').next().unwrap_or("");
            fill_adj_infos(word, "", &pa2_adj, &mut infos.adj_infos);
            fill_adj_infos(word, "e", &pa2_adj_e, &mut infos.adj_infos);
            fill_adj_infos(word, "en", &pa2_adj_en, &mut infos.adj_infos);
            fill_adj_infos(word, "er", &pa2_adj_er, &mut infos.adj_infos);
            fill_adj_infos(word, "em", &pa2_adj_em, &mut infos.adj_infos);
            fill_adj_infos(word, "es", &pa2_adj_es, &mut infos.adj_infos);
        } else if line.contains('_') && !line.ends_with("_in") {
            let cleaned = line.split('#').next().unwrap_or("").trim();
            let parts: Vec<&str> = cleaned.split('_').collect();
            if parts.len() < 2 {
                continue;
            }
            let prefix = parts[0];
            let verb_baseform = parts[1];
            let forms =
                synth.synthesize_for_pos_tags(verb_baseform, &|tag: &str| tag.starts_with("VER:"));
            for form in forms {
                if !form.contains('ß') {
                    infos.verb_infos.insert(
                        format!("{prefix}{form}"),
                        PrefixInfixVerb {
                            prefix: prefix.to_string(),
                            infix: String::new(),
                            verb_baseform: verb_baseform.to_string(),
                        },
                    );
                }
            }
            infos.verb_infos.insert(
                format!("{prefix}zu{verb_baseform}"),
                PrefixInfixVerb {
                    prefix: prefix.to_string(),
                    infix: "zu".to_string(),
                    verb_baseform: verb_baseform.to_string(),
                },
            );
            infos.nominalized_verb_infos.insert(
                format!("{}{}", uppercase_first_char(prefix), verb_baseform),
                NominalizedVerb {
                    prefix: uppercase_first_char(prefix),
                    verb_baseform: verb_baseform.to_string(),
                },
            );
            infos.nominalized_gen_verb_infos.insert(
                format!("{}{}s", uppercase_first_char(prefix), verb_baseform),
                NominalizedGenitiveVerb {
                    prefix: uppercase_first_char(prefix),
                    verb_baseform: verb_baseform.to_string(),
                },
            );
        }
    }
    Ok(infos)
}

/// `StringUtils.startsWithAny`.
fn starts_with_any(s: &str, search: &[&str]) -> bool {
    search.iter().any(|p| s.starts_with(p))
}

/// `StringUtils.containsAny`.
fn contains_any(s: &str, search: &[&str]) -> bool {
    search.iter().any(|p| s.contains(p))
}

/// `StringUtils.equalsAny`.
fn equals_any(s: &str, search: &[&str]) -> bool {
    search.contains(&s)
}

/// `RegExUtils.removePattern` (replace all matches with "").
fn remove_pattern(s: &str, pattern: &str) -> String {
    match regex::Regex::new(pattern) {
        Ok(re) => re.replace_all(s, "").into_owned(),
        Err(_) => s.to_string(),
    }
}

fn full_match(s: &str, pattern: &str) -> bool {
    regex::Regex::new(&format!("^(?:{pattern})$"))
        .map(|re| re.is_match(s))
        .unwrap_or(false)
}

fn find_match(s: &str, pattern: &str) -> bool {
    regex::Regex::new(pattern)
        .map(|re| re.is_match(s))
        .unwrap_or(false)
}

/// Java `List<AnalyzedToken>.toString()` (`lemmaOrToken + '/' + posTag`).
fn readings_repr(readings: &[AnalyzedToken]) -> String {
    let inner: Vec<String> = readings
        .iter()
        .map(|r| format!("{}/{}", r.lemma(), r.pos_tag.as_deref().unwrap_or("null")))
        .collect();
    format!("[{}]", inner.join(", "))
}

pub struct GermanTagger {
    dict: Dictionary,
    manual: ManualTagger,
    removals: ManualTagger,
    compound: Arc<GermanCompoundTokenizer>,
    expansion: ExpansionInfos,
    all_adj_gru_tags: Vec<String>,
}

impl GermanTagger {
    /// Load the tagger from the vendored data directory.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let dict_dir = data_dir.join("de/dictionaries");
        let info = DictionaryInfo::load(&dict_dir.join("german.info"))?;
        let dict = Dictionary::load(&dict_dir.join("german.dict"), &info)?;
        let words_dir = data_dir.join("de/words");
        let manual = ManualTagger::load(&[
            &words_dir.join("added.txt"),
            &words_dir.join("added_custom.txt"),
        ])?;
        let removals = ManualTagger::load(&[
            &words_dir.join("removed.txt"),
            &words_dir.join("removed_custom.txt"),
        ])?;
        let compound_dir = data_dir.join("de/compound");
        let compound = Arc::new(GermanCompoundTokenizer::load(
            &compound_dir.join("wordsGerman.txt"),
            &compound_dir.join("exceptionsGerman.txt"),
            true,
        )?);
        let synth = GermanSynthesizer::from_data(data_dir)?;
        let expansion = init_expansion_infos(data_dir, &synth)?;
        Ok(Self {
            dict,
            manual,
            removals,
            compound,
            expansion,
            all_adj_gru_tags: all_adj_gru_tags(),
        })
    }

    /// `CombiningTagger.tag` (manual + dictionary − removals).
    /// Returns `(lemma, pos_tag)` pairs in Java's order.
    pub fn word_tagger_tag(&self, word: &str) -> Vec<(String, String)> {
        let mut result: Vec<(String, String)> = self.manual.lookup(word).to_vec();
        result.extend(self.dict.lookup(word));
        if !self.removals.is_empty() {
            let removals = self.removals.lookup(word);
            if !removals.is_empty() {
                result.retain(|tagged| !removals.contains(tagged));
            }
        }
        result
    }

    /// `GermanTagger.tag(String)`.
    pub fn tag_word(&self, word: &str) -> Vec<(String, String)> {
        self.word_tagger_tag(word)
    }

    /// `GermanTagger.lookup(String)`: first reading or `None` when untagged.
    pub fn lookup(&self, word: &str) -> Option<AnalyzedTokenReadings> {
        let readings = self.tag(&[word.to_string()], false);
        let atr = readings.into_iter().next()?;
        atr.readings.first().and_then(|r| r.pos_tag.as_ref())?;
        Some(atr)
    }

    fn matches_uppercase_adjective(&self, unknown_uppercase_token: &str) -> bool {
        let temp = self.word_tagger_tag(&lowercase_first_char(unknown_uppercase_token));
        temp.first().is_some_and(|(_, tag)| tag.starts_with("ADJ"))
    }

    fn is_weise_exception(&self, word: &str) -> bool {
        if let Some(stem) = word.strip_suffix("erweise") {
            return self
                .word_tagger_tag(stem)
                .iter()
                .any(|(_, tag)| tag.starts_with("ADJ"));
        }
        false
    }

    fn add_stem(&self, analyzed: Vec<(String, String)>, stem: &str) -> Vec<(String, String)> {
        analyzed
            .into_iter()
            .map(|(lemma, tag)| {
                let mut lemma = lemma;
                if !stem.is_empty() && !stem.ends_with('-') && tag.starts_with("SUB") {
                    lemma = lemma.to_lowercase();
                }
                (format!("{stem}{lemma}"), tag)
            })
            .collect()
    }

    /// `GermanTagger.sanitizeWord` (dash-linked words: keep the last part).
    fn sanitize_word(&self, word: &str) -> String {
        let mut result = word.to_string();
        if !word.ends_with('-') {
            let split_word: Vec<&str> = word.split('-').collect();
            let last_token =
                if split_word.len() > 1 && !split_word[split_word.len() - 1].trim().is_empty() {
                    split_word[split_word.len() - 1]
                } else {
                    word
                };
            let compounded = self.compound.tokenize(last_token);
            let last_part =
                if compounded.len() > 1 && word.chars().next().is_some_and(char::is_uppercase) {
                    uppercase_first_char(&compounded[compounded.len() - 1])
                } else {
                    compounded.last().cloned().unwrap_or_default()
                };
            let tagged = self.word_tagger_tag(&last_part);
            let is_noun_or_adj = tagged
                .first()
                .is_some_and(|(_, tag)| tag.starts_with("SUB") || tag.starts_with("ADJ"));
            if !tagged.is_empty()
                && (is_noun_or_adj || self.matches_uppercase_adjective(&last_part))
            {
                result = last_part;
            }
        }
        result
    }

    fn get_analyzed_tokens(
        &self,
        tagged_words: &[(String, String)],
        word: &str,
    ) -> Vec<AnalyzedToken> {
        tagged_words
            .iter()
            .map(|(lemma, tag)| {
                AnalyzedToken::new(word.to_string(), Some(lemma.clone()), Some(tag.clone()))
            })
            .collect()
    }

    fn get_analyzed_tokens_compound(
        &self,
        tagged_words: &[(String, String)],
        word: &str,
        compound_parts: &[String],
    ) -> Vec<AnalyzedToken> {
        let mut result = Vec::new();
        for (lemma, tag) in tagged_words {
            if tag.starts_with("VER:IMP") {
                // ignore imperative, as otherwise e.g. "zehnfach" will be
                // interpreted as a verb (zehn + fach)
                continue;
            }
            let mut lemma_str = String::new();
            for (i, part) in compound_parts[..compound_parts.len() - 1]
                .iter()
                .enumerate()
            {
                if i == 0 {
                    lemma_str.push_str(part);
                } else {
                    lemma_str.push_str(&lowercase_first_char(part));
                }
            }
            lemma_str.push_str(&lowercase_first_char(lemma));
            result.push(AnalyzedToken::new(
                word.to_string(),
                Some(lemma_str),
                Some(tag.clone()),
            ));
        }
        result
    }

    /// `GermanTagger.tag(List, boolean)`.
    pub fn tag(&self, sentence_tokens: &[String], ignore_case: bool) -> Vec<AnalyzedTokenReadings> {
        let mut first_word = true;
        let mut token_readings: Vec<AnalyzedTokenReadings> = Vec::new();
        let mut pos = 0usize;
        let mut prev_word: Option<&str> = None;

        for (idx_pos, word) in sentence_tokens.iter().enumerate() {
            let mut readings: Vec<AnalyzedToken> = Vec::new();
            let mut tagger_tokens: Option<Vec<(String, String)>> = None;

            // Gender star etc.
            if idx_pos + 2 < sentence_tokens.len()
                && full_match(&sentence_tokens[idx_pos + 1], GENDER_GAP_CHARS)
            {
                let next2 = &sentence_tokens[idx_pos + 2];
                if full_match(next2, AFTER_ASTERISK) {
                    // "jede*r", "sein*e"
                    let mut tokens = self.word_tagger_tag(word);
                    tokens.extend(self.word_tagger_tag(&format!("{word}{next2}")));
                    tagger_tokens = Some(tokens);
                } else if full_match(next2, INNEN_PATTERN_1) {
                    // e.g. Werkstudent:innen-Zielgruppe -> take tags of 'Zielgruppe'
                    let last_part = remove_pattern(next2, ANYTHING_DASH);
                    tagger_tokens = Some(self.word_tagger_tag(&last_part));
                } else if full_match(next2, INNEN_PATTERN_2) {
                    // e.g. Werkstudent:innenzielgruppe -> take tags of 'Zielgruppe'
                    if let Some(idx) = next2.rfind("innen") {
                        let last_part = uppercase_first_char(&next2[idx + "innen".len()..]);
                        tagger_tokens = Some(self.word_tagger_tag(&last_part));
                    }
                }
            }
            let mut tagger_tokens = tagger_tokens.unwrap_or_else(|| self.word_tagger_tag(word));

            // Only first iteration. Consider ":" as a potential sentence start marker
            if (first_word || prev_word == Some(":")) && tagger_tokens.is_empty() && ignore_case {
                tagger_tokens = self.word_tagger_tag(&word.to_lowercase());
                first_word = !is_alphanumeric(word);
            } else if pos == 0 && ignore_case {
                // "Haben", "Sollen", "Können", "Gerade" etc. at start of sentence
                tagger_tokens.extend(self.word_tagger_tag(&word.to_lowercase()));
            } else if pos > 1 && tagger_tokens.is_empty() && ignore_case {
                if let Some(idx) = sentence_tokens.iter().position(|t| t == word) {
                    // add lowercase token readings to words at start of direct speech
                    if idx > 2 && sentence_tokens[idx - 1] == "„" && sentence_tokens[idx - 3] == ":"
                    {
                        tagger_tokens.extend(self.word_tagger_tag(&word.to_lowercase()));
                    }
                }
            }

            if !tagger_tokens.is_empty() {
                // Word known, just add analyzed token to readings
                readings.extend(self.get_analyzed_tokens(&tagger_tokens, word));
                // Imperative/indicative form expansion for words without a
                // separable prefix
                let word_lc = word.to_lowercase();
                let first_upper_rest_lower = is_capitalized_word(word);
                let is_lower_or_capitalized = first_upper_rest_lower || word == &word_lc;
                if !starts_with_any(&word_lc, PREFIXES_SEPARABLE_VERBS)
                    && !starts_with_any(&word_lc, NOT_A_VERB)
                    && is_lower_or_capitalized
                {
                    let (lst_prt, frst_prt) =
                        if starts_with_any(&word_lc, PREFIXES_NON_SEPARABLE_VERBS) {
                            let lst = remove_pattern(&word_lc, PREFIXES_NON_SEPARABLE_VERBS_REGEXP);
                            let frst = word.strip_suffix(&lst).unwrap_or("").to_string();
                            (lst, frst)
                        } else {
                            (word.clone(), String::new())
                        };
                    let verbs = self.word_tagger_tag(&lst_prt);
                    for (lemma, tag) in &verbs {
                        let word_index_is_zero = sentence_tokens
                            .iter()
                            .position(|t| t == word)
                            .map(|i| i == 0)
                            .unwrap_or(false);
                        let lower_first_rest = word
                            .chars()
                            .next()
                            .map(|c| c.to_lowercase().collect::<String>() + &word[c.len_utf8()..])
                            .map(|w| word == &w)
                            .unwrap_or(false);
                        if (word_index_is_zero || lower_first_rest)
                            && !equals_any(&lst_prt, &["gar", "mal", "null", "trotz"])
                        {
                            let repr = readings_repr(&readings);
                            if tag.starts_with("VER:IMP:SIN:SFT")
                                && !repr.contains("VER:1:SIN:PRÄ:SFT")
                            {
                                readings.push(AnalyzedToken::new(
                                    word.clone(),
                                    Some(format!("{}{}", frst_prt.to_lowercase(), lemma)),
                                    Some("VER:1:SIN:PRÄ:SFT".to_string()),
                                ));
                            }
                            if tag.starts_with("VER:1:SIN:PRÄ:SFT")
                                && !repr.contains("VER:IMP:SIN:SFT")
                            {
                                readings.push(AnalyzedToken::new(
                                    word.clone(),
                                    Some(format!("{}{}", frst_prt.to_lowercase(), lemma)),
                                    Some("VER:IMP:SIN:SFT".to_string()),
                                ));
                            }
                        }
                    }
                }
            } else {
                // Word not known: expansion / compound decomposition
                self.expand_unknown_word(word, sentence_tokens, idx_pos, pos, &mut readings);
            }

            let mut article = AnalyzedTokenReadings::new(readings);
            article.start_pos = pos;
            token_readings.push(article);
            pos += word.len();
            prev_word = Some(word);
        }
        token_readings
    }

    /// The `else` branch of `GermanTagger.tag` for unknown words.
    fn expand_unknown_word(
        &self,
        word: &str,
        sentence_tokens: &[String],
        idx_pos: usize,
        pos: usize,
        readings: &mut Vec<AnalyzedToken>,
    ) {
        let word_orig = word.to_string();
        let verb_info = self.expansion.verb_infos.get(word);
        let nom_verb_info = self.expansion.nominalized_verb_infos.get(word);
        let nom_gen_verb_info = self.expansion.nominalized_gen_verb_infos.get(word);
        let adj_infos = self.expansion.adj_infos.get(word);
        let add_noun_tags = !NOUN_TAG_EXPANSION_EXCEPTIONS.contains(&word);

        if let Some(verb_info) = verb_info {
            if verb_info
                .prefix
                .chars()
                .next()
                .is_some_and(char::is_lowercase)
            {
                let no_prefix_form = &word[verb_info.prefix.len() + verb_info.infix.len()..];
                let tags = self.word_tagger_tag(no_prefix_form);
                let mut is_sft = false;
                for (lemma, tag) in &tags {
                    let is_verb_like = starts_with_any(tag, &["VER:", "PA1:", "PA2:"])
                        && !starts_with_any(tag, &["VER:MOD", "VER:AUX"]);
                    if is_verb_like {
                        let flektion = tag[tag.len() - 3..].to_string();
                        if starts_with_any(&verb_info.prefix, PREFIXES_SEPARABLE_VERBS)
                            && !contains_any(&word_orig, NOT_A_VERB)
                        {
                            let can_append = sentence_tokens
                                .iter()
                                .position(|t| t == word)
                                .map(|i| i == 0)
                                .unwrap_or(false)
                                || first_char_lower(word) == word;
                            if starts_with_any(tag, &["VER:1", "VER:2", "VER:3"]) && can_append {
                                readings.push(AnalyzedToken::new(
                                    word.to_string(),
                                    Some(format!("{}{}", verb_info.prefix, lemma)),
                                    Some(format!("{tag}:NEB")),
                                ));
                            } else if !starts_with_any(tag, &["VER:IMP"]) {
                                readings.push(AnalyzedToken::new(
                                    word.to_string(),
                                    Some(format!("{}{}", verb_info.prefix, lemma)),
                                    Some(tag.clone()),
                                ));
                            } else if starts_with_any(tag, &["VER:IMP:SIN"]) {
                                // Java checks `readings.contains("VER:1:SIN:PRÄ")`,
                                // which is always false for AnalyzedToken lists
                                if flektion == "SFT" || !find_match(word, I_MATCH) {
                                    readings.push(AnalyzedToken::new(
                                        word.to_string(),
                                        Some(format!("{}{}", verb_info.prefix, lemma)),
                                        Some(format!("VER:1:SIN:PRÄ:{flektion}:NEB")),
                                    ));
                                }
                            }
                        } else if starts_with_any(&verb_info.prefix, PREFIXES_NON_SEPARABLE_VERBS)
                            && !contains_any(&word_orig, NOT_A_VERB)
                        {
                            // `readings.contains(...)` is always false (see above)
                            if starts_with_any(tag, &["VER:IMP:SIN"])
                                || starts_with_any(tag, &["VER:1:SIN:PRÄ"])
                            {
                                if flektion == "SFT" || !find_match(word, I_MATCH) {
                                    readings.push(AnalyzedToken::new(
                                        word.to_string(),
                                        Some(format!("{}{}", verb_info.prefix, lemma)),
                                        Some(format!("VER:IMP:SIN{flektion}")),
                                    ));
                                    readings.push(AnalyzedToken::new(
                                        word.to_string(),
                                        Some(format!("{}{}", verb_info.prefix, lemma)),
                                        Some(format!("VER:1:SIN:PRÄ:{flektion}")),
                                    ));
                                }
                            } else {
                                readings.push(AnalyzedToken::new(
                                    word.to_string(),
                                    Some(format!("{}{}", verb_info.prefix, lemma)),
                                    Some(tag.clone()),
                                ));
                            }
                        }
                        if tag.contains(":SFT") {
                            is_sft = true;
                        }
                    }
                }
                if verb_info.infix == "zu" {
                    readings.clear();
                    readings.push(AnalyzedToken::new(
                        word.to_string(),
                        Some(format!("{}{}", verb_info.prefix, verb_info.verb_baseform)),
                        Some(format!("VER:EIZ:{}", if is_sft { "SFT" } else { "NON" })),
                    ));
                }
            }
        } else if let Some(nom) = nom_verb_info {
            if add_noun_tags {
                // e.g. "herum_geben" -> "(das) Herumgeben"
                for tag in [
                    "SUB:NOM:SIN:NEU:INF",
                    "SUB:AKK:SIN:NEU:INF",
                    "SUB:DAT:SIN:NEU:INF",
                ] {
                    readings.push(AnalyzedToken::new(
                        word.to_string(),
                        Some(format!("{}{}", nom.prefix, nom.verb_baseform)),
                        Some(tag.to_string()),
                    ));
                }
            }
        } else if let Some(nom_gen) = nom_gen_verb_info {
            if add_noun_tags {
                // e.g. "herum_geben" -> "(des) Herumgebens"
                readings.push(AnalyzedToken::new(
                    word.to_string(),
                    Some(format!("{}{}", nom_gen.prefix, nom_gen.verb_baseform)),
                    Some("SUB:GEN:SIN:NEU:INF".to_string()),
                ));
            }
        } else if let Some(infos) = adj_infos {
            for info in infos {
                readings.push(AnalyzedToken::new(
                    info.full_form.clone(),
                    Some(info.baseform.clone()),
                    Some(info.tag.clone()),
                ));
            }
        } else if self.is_weise_exception(word) {
            // "idealerweise" etc. but not "überweise", "eimerweise"
            for tag in tags_for_weise() {
                readings.push(AnalyzedToken::new(
                    word.to_string(),
                    Some(word.to_string()),
                    Some(tag.to_string()),
                ));
            }
        } else if !word.chars().all(|c| c.is_whitespace())
            && full_match(word, MITARBEITENDEN_PATTERN)
        {
            let idx = word.find("mitarbeitende").unwrap_or(0);
            let first_part = &word[..idx];
            let last_part = &word[idx..];
            let mitarbeitende_tags = self.word_tagger_tag(&uppercase_first_char(last_part));
            for (_, tag) in &mitarbeitende_tags {
                readings.push(AnalyzedToken::new(
                    word.to_string(),
                    Some(format!("{first_part}mitarbeitende")),
                    Some(tag.clone()),
                ));
            }
        } else if !word.chars().all(|c| c.is_whitespace()) {
            let compound_parts = self.compound.tokenize(word);
            if compound_parts.len() <= 1 {
                // Recognize alternative imperative forms (e.g. "Geh bitte!")
                let imperative = self.get_imperative_form(word, sentence_tokens, pos);
                let substantivated = self.get_substantivated_forms(word, sentence_tokens);
                if !imperative.is_empty() {
                    readings.extend(imperative);
                } else if !substantivated.is_empty() {
                    readings.extend(substantivated);
                } else {
                    if starts_with_any(
                        word,
                        &[
                            "bitter", "dunkel", "erz", "extra", "früh", "gemein", "hyper", "lau",
                            "mega", "minder", "stock", "super", "tod", "ultra", "un", "ur",
                        ],
                    ) {
                        let last_part = remove_pattern(word, BITTER_PREFIX_REGEXP);
                        if last_part.chars().count() > 3 {
                            let first_part =
                                word.strip_suffix(&last_part).unwrap_or("").to_string();
                            for (lemma, tag) in self.word_tagger_tag(&last_part) {
                                if !(first_part.chars().count() == 2 && tag.starts_with("VER")) {
                                    readings.push(AnalyzedToken::new(
                                        word.to_string(),
                                        Some(format!("{first_part}{lemma}")),
                                        Some(tag),
                                    ));
                                }
                            }
                        }
                    }
                    // Separate dash-linked words
                    // Only check single word tokens and skip words containing
                    // numbers because it's unpredictable
                    let word_chars: Vec<char> = word.chars().collect();
                    if word.split(' ').count() == 1
                        && !word_chars.first().is_some_and(|c| c.is_ascii_digit())
                    {
                        let sanitized = self.sanitize_word(word);
                        let word_stem = word_orig[..word_orig.len() - sanitized.len()].to_string();
                        let mut word = sanitized;
                        let compounded_word = self.compound.tokenize(&word);
                        if compounded_word.len() > 1 {
                            word =
                                uppercase_first_char(&compounded_word[compounded_word.len() - 1]);
                        } else {
                            word = compounded_word.last().cloned().unwrap_or_default();
                        }
                        let mut linked = self.add_stem(self.word_tagger_tag(&word), &word_stem);
                        if word_orig.contains('-') && linked.is_empty() {
                            let lower = lowercase_first_char(&word);
                            if self.matches_uppercase_adjective(&word) {
                                word = lower;
                                linked = self.word_tagger_tag(&word);
                            }
                        }
                        word = word_orig.clone();

                        let word_starts_uppercase =
                            word.chars().next().is_some_and(char::is_uppercase);
                        if linked.is_empty() {
                            // Verbs with certain prefixes (e.g. "ab", "ein",
                            // "zwischen") are always separable.
                            let word_lc = word.to_lowercase();
                            let first_upper = first_char_upper(&word) == word;
                            if starts_with_any(&word_lc, PREFIXES_VERBS)
                                && !contains_any(&word_lc, NOT_A_VERB)
                                && (first_upper || word == word_lc)
                            {
                                let last_part = remove_pattern(&word_lc, PREFIXES_VERBS_REGEXP);
                                if last_part.chars().count() > 2 {
                                    let first_part =
                                        word.strip_suffix(&last_part).unwrap_or("").to_string();
                                    let word_at_start = sentence_tokens
                                        .iter()
                                        .position(|t| t == &word)
                                        .map(|i| i == 0)
                                        .unwrap_or(false);
                                    let word_is_lower_or_start = word == word_lc || word_at_start;
                                    // Extended infinitive with zu
                                    if let Some(infinitiv) = last_part.strip_prefix("zu") {
                                        for (lemma, inf_tag) in self.word_tagger_tag(infinitiv) {
                                            if inf_tag.starts_with("VER:INF") {
                                                let pstg = inf_tag.replacen("INF", "EIZ", 1);
                                                readings.push(AnalyzedToken::new(
                                                    word.clone(),
                                                    Some(format!("{first_part}{lemma}")),
                                                    Some(pstg),
                                                ));
                                            }
                                        }
                                    }
                                    for (lemma, tag) in self.word_tagger_tag(&last_part) {
                                        if tag.starts_with("VER")
                                            && !tag.starts_with("VER:PA")
                                            && !tag.starts_with("VER:AUX")
                                            && !tag.starts_with("VER:MOD")
                                            && first_part != "un"
                                        {
                                            if tag.starts_with("VER:INF") {
                                                if first_upper {
                                                    for sub_tag in [
                                                        "SUB:NOM:SIN:NEU:INF",
                                                        "SUB:DAT:SIN:NEU:INF",
                                                        "SUB:AKK:SIN:NEU:INF",
                                                    ] {
                                                        readings.push(AnalyzedToken::new(
                                                            word.clone(),
                                                            Some(word.clone()),
                                                            Some(sub_tag.to_string()),
                                                        ));
                                                    }
                                                    if word_at_start {
                                                        readings.push(AnalyzedToken::new(
                                                            word.clone(),
                                                            Some(format!(
                                                                "{}{lemma}",
                                                                first_part.to_lowercase()
                                                            )),
                                                            Some(tag.clone()),
                                                        ));
                                                    }
                                                } else {
                                                    readings.push(AnalyzedToken::new(
                                                        word.clone(),
                                                        Some(format!(
                                                            "{}{lemma}",
                                                            first_part.to_lowercase()
                                                        )),
                                                        Some(tag.clone()),
                                                    ));
                                                }
                                            } else if tag.starts_with("VER:IMP") {
                                                let flekt = &tag[tag.len() - 3..];
                                                if word_is_lower_or_start {
                                                    if !equals_any(
                                                        &first_part.to_lowercase(),
                                                        PREFIXES_SEPARABLE_VERBS,
                                                    ) {
                                                        readings.push(AnalyzedToken::new(
                                                            word.clone(),
                                                            Some(format!(
                                                                "{}{lemma}",
                                                                first_part.to_lowercase()
                                                            )),
                                                            Some(tag.clone()),
                                                        ));
                                                        // always-true Java `contains` checks
                                                        if tag.starts_with("VER:IMP:SIN") {
                                                            readings.push(AnalyzedToken::new(
                                                                word.clone(),
                                                                Some(format!(
                                                                    "{first_part}{lemma}"
                                                                )),
                                                                Some(format!(
                                                                    "VER:1:SIN:PRÄ:{flekt}"
                                                                )),
                                                            ));
                                                        }
                                                    } else if flekt == "SFT"
                                                        || !find_match(&word, I_MATCH)
                                                    {
                                                        readings.push(AnalyzedToken::new(
                                                            word.clone(),
                                                            Some(format!("{first_part}{lemma}")),
                                                            Some(format!(
                                                                "VER:1:SIN:PRÄ:{flekt}:NEB"
                                                            )),
                                                        ));
                                                    }
                                                }
                                            } else if equals_any(
                                                &first_part.to_lowercase(),
                                                PREFIXES_SEPARABLE_VERBS,
                                            ) && word_is_lower_or_start
                                            {
                                                let new_tag = if tag.ends_with(":NEB") {
                                                    tag.clone()
                                                } else {
                                                    format!("{tag}:NEB")
                                                };
                                                readings.push(AnalyzedToken::new(
                                                    word.clone(),
                                                    Some(format!(
                                                        "{}{lemma}",
                                                        first_part.to_lowercase()
                                                    )),
                                                    Some(new_tag),
                                                ));
                                                if tag.starts_with("VER:3:SIN:PRÄ")
                                                    && (first_part == "durch" || first_part == "um")
                                                {
                                                    readings.push(AnalyzedToken::new(
                                                        word.clone(),
                                                        Some(format!("{first_part}{lemma}")),
                                                        Some("VER:PA2:SFT".to_string()),
                                                    ));
                                                    readings.push(AnalyzedToken::new(
                                                        word.clone(),
                                                        Some(word.clone()),
                                                        Some("PA2:PRD:GRU:VER".to_string()),
                                                    ));
                                                }
                                            } else if equals_any(
                                                &first_part.to_lowercase(),
                                                PREFIXES_NON_SEPARABLE_VERBS,
                                            ) && word_is_lower_or_start
                                            {
                                                readings.push(AnalyzedToken::new(
                                                    word.clone(),
                                                    Some(format!(
                                                        "{}{lemma}",
                                                        first_part.to_lowercase()
                                                    )),
                                                    Some(tag.clone()),
                                                ));
                                                if tag.starts_with("VER:3:SIN:PRÄ:SFT")
                                                    || (tag.starts_with("VER:1:PLU:PRÄ:NON")
                                                        && contains_any(
                                                            lemma.as_str(),
                                                            PARTIZIP2_CONTAINS_1_PLU_PRA,
                                                        ))
                                                    || (tag.starts_with("VER:1:PLU:PRT:NON")
                                                        && contains_any(
                                                            lemma.as_str(),
                                                            PARTIZIP2_CONTAINS_1_PLU_PRT,
                                                        ))
                                                {
                                                    if first_part != "un" {
                                                        let fl = &tag[tag.len() - 3..];
                                                        readings.push(AnalyzedToken::new(
                                                            word.clone(),
                                                            Some(format!("{first_part}{lemma}")),
                                                            Some(format!("VER:PA2:{fl}")),
                                                        ));
                                                    }
                                                    readings.push(AnalyzedToken::new(
                                                        word.clone(),
                                                        Some(word.clone()),
                                                        Some("PA2:PRD:GRU:VER".to_string()),
                                                    ));
                                                }
                                            }
                                        } else if (tag.starts_with("PA")
                                            || tag.starts_with("VER:PA"))
                                            && word_is_lower_or_start
                                            && !(first_part == "un" && tag.starts_with("VER:PA"))
                                        {
                                            readings.push(AnalyzedToken::new(
                                                word.clone(),
                                                Some(format!(
                                                    "{}{lemma}",
                                                    first_part.to_lowercase()
                                                )),
                                                Some(tag.clone()),
                                            ));
                                        }
                                    }
                                    // Participle suffix analysis
                                    let mut middle_part = String::new();
                                    let mut suffix = String::new();
                                    for sffx in ["e", "em", "en", "er", "es"] {
                                        if last_part.ends_with(sffx) {
                                            middle_part = last_part[..last_part.len() - sffx.len()]
                                                .to_string();
                                            suffix = sffx.to_string();
                                        }
                                    }
                                    for (middle_lemma, middle_tag) in
                                        self.word_tagger_tag(&middle_part)
                                    {
                                        let _ = middle_lemma;
                                        if middle_tag.starts_with("VER:3:SIN:PRÄ:SFT")
                                            && word_is_lower_or_start
                                        {
                                            let lemma =
                                                word[..word.len() - suffix.len()].to_string();
                                            let pos_ends: &[&str] = match suffix.as_str() {
                                                "e" => POSTAGS_PARTIZIP_ENDING_E,
                                                "em" => POSTAGS_PARTIZIP_ENDING_EM,
                                                "en" => POSTAGS_PARTIZIP_ENDING_EN,
                                                "er" => POSTAGS_PARTIZIP_ENDING_ER,
                                                "es" => POSTAGS_PARTIZIP_ENDING_ES,
                                                _ => &[],
                                            };
                                            for pos_end in pos_ends {
                                                readings.push(AnalyzedToken::new(
                                                    word.clone(),
                                                    Some(lemma.clone()),
                                                    Some(format!("PA2:{pos_end}")),
                                                ));
                                            }
                                        }
                                    }
                                }
                            } else {
                                readings.push(no_info_token(&word));
                            }
                        } else {
                            if word_starts_uppercase {
                                readings.extend(self.get_analyzed_tokens(&linked, &word));
                            } else {
                                readings.extend(self.get_analyzed_tokens_compound(
                                    &linked,
                                    &word,
                                    &compounded_word,
                                ));
                            }
                        }
                    } else {
                        readings.push(no_info_token(word));
                    }
                }
            } else if !(idx_pos + 2 < sentence_tokens.len()
                && sentence_tokens[idx_pos + 1] == "."
                && full_match(&sentence_tokens[idx_pos + 2], DOMAIN_TLDS))
            {
                let last_part_raw = compound_parts[compound_parts.len() - 1].clone();
                let mut last_part = last_part_raw.clone();
                if word.chars().next().is_some_and(char::is_uppercase)
                    && !contains_any(
                        &last_part,
                        &["freie", "freier", "freien", "freies", "freiem"],
                    )
                {
                    last_part = uppercase_first_char(&last_part);
                }
                let part_tagger_tokens = self.word_tagger_tag(&last_part);
                if part_tagger_tokens.is_empty() {
                    readings.push(no_info_token(word));
                } else {
                    let temp = self.get_analyzed_tokens_compound(
                        &part_tagger_tokens,
                        word,
                        &compound_parts,
                    );
                    let first_part = compound_parts[0].clone();
                    let word_at_start = sentence_tokens
                        .iter()
                        .position(|t| t == word)
                        .map(|i| i == 0)
                        .unwrap_or(false);
                    let word_is_lower_or_start = word == word.to_lowercase() || word_at_start;
                    if VERB_COMPOUND_FIRST_PARTS.contains(&first_part.as_str()) {
                        for (lemma, tag) in &part_tagger_tokens {
                            if starts_with_any(tag, &["VER:1", "VER:2", "VER:3"])
                                && word_is_lower_or_start
                            {
                                let new_tag = if tag.ends_with("NEB") {
                                    tag.clone()
                                } else {
                                    format!("{tag}:NEB")
                                };
                                readings.push(AnalyzedToken::new(
                                    word.to_string(),
                                    Some(format!("{first_part}{lemma}")),
                                    Some(new_tag),
                                ));
                            } else if !starts_with_any(tag, &["VER:IMP"]) {
                                readings.push(AnalyzedToken::new(
                                    word.to_string(),
                                    Some(format!("{first_part}{lemma}")),
                                    Some(tag.clone()),
                                ));
                            }
                        }
                    } else {
                        for token in temp {
                            if !token.pos_tag.as_deref().unwrap_or("").contains("VER") {
                                readings.push(token);
                            }
                        }
                    }
                }
            }
        }
        if readings.is_empty() {
            readings.push(no_info_token(word));
        }
    }

    /// `GermanTagger.getImperativeForm`.
    fn get_imperative_form(
        &self,
        word: &str,
        sentence_tokens: &[String],
        pos: usize,
    ) -> Vec<AnalyzedToken> {
        let mut previous_word = "";
        if let Some(mut i) = sentence_tokens.iter().position(|t| t == word) {
            while i > 0 {
                i -= 1;
                previous_word = &sentence_tokens[i];
                if !apache_is_whitespace(previous_word) {
                    break;
                }
            }
        }
        if (pos != 0 || sentence_tokens.len() <= 1)
            && ![
                "ich", "er", "es", "sie", "bitte", "aber", "nun", "jetzt", "„",
            ]
            .iter()
            .any(|w| w.eq_ignore_ascii_case(previous_word))
        {
            return Vec::new();
        }
        let w = if pos == 0 || previous_word == "„" {
            word.to_lowercase()
        } else {
            word.to_string()
        };
        let tagged_with_e = self.word_tagger_tag(&format!("{w}e"));
        for (lemma, tag) in tagged_with_e {
            if tag.starts_with("VER:IMP:SIN") {
                let removal_tags = self.removals.lookup(&w);
                if !removal_tags.contains(&(lemma.clone(), tag.clone())) {
                    return vec![AnalyzedToken::new(word.to_string(), Some(lemma), Some(tag))];
                }
                break;
            }
        }
        Vec::new()
    }

    /// `GermanTagger.getSubstantivatedForms`.
    fn get_substantivated_forms(
        &self,
        word: &str,
        sentence_tokens: &[String],
    ) -> Vec<AnalyzedToken> {
        if !word.ends_with("er") {
            return Vec::new();
        }
        if full_match(word, DDD_ER_PATTERN) {
            // e.g. "Den 2019er Wert hatten sie geschätzt"
            return self
                .all_adj_gru_tags
                .iter()
                .map(|tag| {
                    AnalyzedToken::new(word.to_string(), Some(word.to_string()), Some(tag.clone()))
                })
                .collect();
        }
        let lower_case_tags = self.word_tagger_tag(&word.to_lowercase());
        if lower_case_tags
            .iter()
            .any(|(_, tag)| tag.starts_with("ADV"))
        {
            return Vec::new();
        }
        let mut idx = sentence_tokens.iter().position(|t| t == word).unwrap_or(0);
        while idx + 1 < sentence_tokens.len() {
            idx += 1;
            let next_word = &sentence_tokens[idx];
            if apache_is_whitespace(next_word) {
                continue;
            }
            if next_word.chars().next().is_some_and(char::is_uppercase) || next_word == "als" {
                return Vec::new();
            }
            break;
        }
        let female_form = &word[..word.len() - 1];
        let tagged_female = self.word_tagger_tag(female_form);
        if tagged_female
            .iter()
            .any(|(_, tag)| tag == "SUB:NOM:SIN:FEM:ADJ")
        {
            return vec![
                AnalyzedToken::new(
                    word.to_string(),
                    Some(word.to_string()),
                    Some("SUB:NOM:SIN:MAS:ADJ".to_string()),
                ),
                AnalyzedToken::new(
                    word.to_string(),
                    Some(word.to_string()),
                    Some("SUB:GEN:PLU:MAS:ADJ".to_string()),
                ),
            ];
        }
        Vec::new()
    }
}

/// `SwissGermanTagger`: adds ß-readings for untagged `ss` tokens (de-CH).
pub struct SwissGermanTagger {
    inner: Arc<GermanTagger>,
}

impl SwissGermanTagger {
    pub fn load(data_dir: &Path) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(GermanTagger::load(data_dir)?),
        })
    }

    pub fn inner(&self) -> &GermanTagger {
        &self.inner
    }

    /// The wrapped German tagger (shared with filters).
    pub fn base_arc(&self) -> Arc<GermanTagger> {
        Arc::clone(&self.inner)
    }

    pub fn tag(&self, sentence_tokens: &[String], ignore_case: bool) -> Vec<AnalyzedTokenReadings> {
        let mut readings = self.inner.tag(sentence_tokens, ignore_case);
        for reading in &mut readings {
            let surface = reading.surface().to_string();
            if !surface.is_empty() && surface.contains("ss") && !reading.is_tagged {
                if let Some(replacement) = self.inner.lookup(&surface.replace("ss", "ß")) {
                    for at in replacement.readings {
                        reading.add_reading(AnalyzedToken::new(
                            surface.clone(),
                            at.stem,
                            at.pos_tag,
                        ));
                    }
                }
            }
        }
        readings
    }
}

/// `GermanTagger.getNoInfoToken`.
fn no_info_token(word: &str) -> AnalyzedToken {
    AnalyzedToken::new(word.to_string(), None, None)
}

/// `word.equals(word.substring(0,1).toUpperCase() + word.substring(1))`.
fn first_char_upper(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// `word.equals(word.substring(0,1).toLowerCase() + word.substring(1))`.
fn first_char_lower(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

/// Apache `StringUtils.isWhitespace`: non-empty, all chars Java whitespace.
fn apache_is_whitespace(s: &str) -> bool {
    !s.is_empty() && s.chars().all(lt_core::java_char_is_whitespace)
}

/// Apache `StringUtils.isAlphanumeric` (non-empty, all chars alphanumeric).
fn is_alphanumeric(s: &str) -> bool {
    !s.is_empty() && s.chars().all(char::is_alphanumeric)
}

/// The `prfxs` list of the compound-verb branch (Java `GermanTagger` line 655).
const VERB_COMPOUND_FIRST_PARTS: &[&str] = &[
    "ab",
    "abend",
    "abhanden",
    "acht",
    "ähnlich",
    "allein",
    "an",
    "auf",
    "aufeinander",
    "aufrecht",
    "aufwärts",
    "aus",
    "auseinander",
    "auswärts",
    "bei",
    "beieinander",
    "beisammen",
    "beiseite",
    "besser",
    "blank",
    "brust",
    "da",
    "daheim",
    "dahin",
    "daneben",
    "danieder",
    "davon",
    "doppel",
    "drauflos",
    "drei",
    "drein",
    "durch",
    "durcheinander",
    "ehe",
    "ein",
    "einig",
    "einwärts",
    "eis",
    "empor",
    "end",
    "fehl",
    "feil",
    "feinst",
    "fort",
    "frei",
    "gegenüber",
    "general",
    "groß",
    "grund",
    "hand",
    "hart",
    "heim",
    "her",
    "herauf",
    "heraus",
    "herbei",
    "hernieder",
    "herüber",
    "herum",
    "herunter",
    "hier",
    "hierher",
    "hierhin",
    "hin",
    "hinauf",
    "hinaus",
    "hindurch",
    "hinein",
    "hinüber",
    "hoch",
    "höher",
    "ineinander",
    "kaputt",
    "kennen",
    "klar",
    "klein",
    "knapp",
    "krank",
    "krumm",
    "kugel",
    "kürzer",
    "lahm",
    "los",
    "maß",
    "miss",
    "mit",
    "mittag",
    "nach",
    "nahe",
    "näher",
    "neben",
    "nebeneinander",
    "nieder",
    "offen",
    "out",
    "preis",
    "quer",
    "ran",
    "rauf",
    "raus",
    "rein",
    "rüber",
    "rück",
    "rückwärts",
    "ruhig",
    "rum",
    "runter",
    "satt",
    "schwarz",
    "sicher",
    "sitzen",
    "statt",
    "still",
    "stoß",
    "teil",
    "tot",
    "trocken",
    "über",
    "überein",
    "übereinander",
    "übrig",
    "um",
    "umher",
    "unter",
    "verrückt",
    "voll",
    "vor",
    "voran",
    "voraus",
    "vorbei",
    "vorlieb",
    "vorüber",
    "vorwärts",
    "vorweg",
    "wach",
    "wahr",
    "warm",
    "weg",
    "weh",
    "weiter",
    "wert",
    "wichtig",
    "wieder",
    "wiederauf",
    "wiederein",
    "wiederher",
    "wohl",
    "zu",
    "zueinander",
    "zufrieden",
    "zugute",
    "zunichte",
    "zurecht",
    "zurück",
    "zusammen",
    "zuwider",
    "zwangs",
    "zwangsum",
    "zwangsvor",
    "zweck",
    "zwischen",
];
