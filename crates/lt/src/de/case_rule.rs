//! `CaseRule` (`DE_CASE`, checklist item 21) + `CaseRuleAntiPatterns` /
//! `CaseRuleExceptions` / `LanguageNames`: uppercase/lowercase checks for
//! adjectives and nominalized verbs ("Das laufen fällt mir leicht." →
//! "Das Laufen ...").
//!
//! `CaseRuleExceptions` loads `de/words/eigennamen_gross.txt` +
//! `de/words/case_rule_exceptions.txt`; each phrase part is a case-sensitive
//! full regexp (`StringMatcher.regexp`). `LanguageNames` lives in
//! `de::language_names`.

use std::path::Path;
use std::sync::{Arc, LazyLock};

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Result, Suggestion, TextRange};
use lt_pattern::matcher as pm;
use lt_pattern::PatternToken;

use super::case_rule_antipatterns;
use super::german_helper::{self, PosType};
use super::language_names;
use super::util;

pub const RULE_ID: &str = "DE_CASE";
const DESCRIPTION: &str = "Großschreibung von Nomen und substantivierten Verben";
const CATEGORY_ID: &str = "CASING";
const CATEGORY_NAME: &str = "Groß-/Kleinschreibung";
const UPPERCASE_MESSAGE: &str =
    "Außer am Satzanfang werden nur Nomen und Eigennamen großgeschrieben.";
const LOWERCASE_MESSAGE: &str =
    "Falls es sich um ein substantiviertes Verb handelt, wird es großgeschrieben.";
const COLON_MESSAGE: &str = "Folgt dem Doppelpunkt weder ein Substantiv noch eine wörtliche Rede oder ein vollständiger Hauptsatz, schreibt man klein weiter.";

static ANTI_PATTERNS: LazyLock<Vec<Arc<pm::CompiledPattern>>> =
    LazyLock::new(|| compile(case_rule_antipatterns::antipattern_defs()));

fn compile(defs: Vec<Vec<PatternToken>>) -> Vec<Arc<pm::CompiledPattern>> {
    defs.iter()
        .flat_map(|tokens| pm::compile_patterns(tokens, None, None).unwrap_or_default())
        .map(Arc::new)
        .collect()
}

/// `CaseRule.nounIndicators`.
const NOUN_INDICATORS: [&str; 6] = ["das", "sein", "mein", "dein", "euer", "unser"];
/// `CaseRule.SENTENCE_START_EXCEPTIONS`.
const SENTENCE_START_EXCEPTIONS: [&str; 11] =
    ["(", "\"", "'", "‘", "„", "«", "»", "‚", ".", "!", "?"];
/// `CaseRule.UNDEFINED_QUANTIFIERS`.
const UNDEFINED_QUANTIFIERS: [&str; 5] = ["viel", "nichts", "nix", "wenig", "allerlei"];
/// `CaseRule.INTERROGATIVE_PARTICLES`.
const INTERROGATIVE_PARTICLES: [&str; 9] = [
    "was", "wodurch", "wofür", "womit", "woran", "worauf", "woraus", "wovon", "wie",
];
/// `CaseRule.POSSESSIVE_INDICATORS`.
const POSSESSIVE_INDICATORS: [&str; 6] = ["einer", "eines", "der", "des", "dieser", "dieses"];
/// `CaseRule.DAS_VERB_EXCEPTIONS`.
const DAS_VERB_EXCEPTIONS: [&str; 7] = ["nur", "sogar", "auch", "die", "alle", "viele", "zu"];
/// `CaseRule.COLON_QUESTION_WORDS`.
const COLON_QUESTION_WORDS: [&str; 9] = [
    "warum", "wieso", "weshalb", "wer", "was", "wann", "wo", "wie", "wozu",
];
/// `CaseRule.COLON_QUESTION_CONJUNCTIONS`.
const COLON_QUESTION_CONJUNCTIONS: [&str; 4] = ["und", "oder", "aber", "denn"];
/// `CaseRule.substVerbenExceptions`.
const SUBST_VERBEN_EXCEPTIONS: [&str; 37] = [
    "hinziehen",
    "helfen",
    "lassen",
    "passieren",
    "haben",
    "passiert",
    "beschränkt",
    "wiederholt",
    "scheinen",
    "klar",
    "heißen",
    "einen",
    "gehören",
    "bedeutet",
    "ermöglicht",
    "funktioniert",
    "sollen",
    "werden",
    "dürfen",
    "müssen",
    "so",
    "ist",
    "können",
    "mein",
    "sein",
    "muss",
    "muß",
    "wollen",
    "habe",
    "ein",
    "tun",
    "bestätigt",
    "bestätigte",
    "bestätigten",
    "bekommen",
    "sauer",
    "bedeuten",
];

static NUMERALS_EN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"^(?:[a-z]|[0-9]+|(?:m{0,4}(c[md]|d?c{0,3})(x[cl]|l?x{0,3})(i[xv]|v?i{0,3})))$",
    )
    .unwrap()
});
static TWO_UPPERCASE_CHARS: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[A-ZÖÄÜ][A-ZÖÄÜ][a-zöäüß-]+$").unwrap());
static VERHALTEN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^.+verhalten$").unwrap());
static IRGEND_ETC: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(?:irgendwelche|irgendwas|irgendein|weniger?|einiger?|mehr|aufs)$")
        .unwrap()
});
static VER_MOD_AUX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^VER:(MOD|AUX):[1-3]:.*$").unwrap());
static ALLE_NM: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^Alle[nm]$").unwrap());

/// `CaseRule.exceptions` (words Morphy only knows as non-nouns).
const EXCEPTIONS: [&str; 647] = [
    "Bedienstete",
    "Bediensteter",
    "Feierwütiger",
    "Feierwütige",
    "Feierwütigen",
    "Dritten",
    "Berufstätige",
    "Berufstätigen",
    "Erwerbstätige",
    "Erwerbstätigen",
    "Tatverdächtige",
    "Tatverdächtigen",
    "Konsumierende",
    "Konsumierenden",
    "Verliebter",
    "Verliebte",
    "Beängstigendes",
    "Oppositioneller",
    "Oppositionelle",
    "Verantwortlicher",
    "Verantwortliche",
    "Verantwortlichen",
    "Beschuldigte",
    "Beschuldigten",
    "Beklagte",
    "Beklagten",
    "Befragte",
    "Befragten",
    "Hingerichtete",
    "Lehrende",
    "Lehrender",
    "Vertrauter",
    "Out",
    "Packet",
    "Adult",
    "Apart",
    "Universal",
    "Multinational",
    "Additional",
    "Smart",
    "Adverse",
    "Different",
    "Light",
    "Legal",
    "Computational",
    "Holder",
    "Just",
    "Lost",
    "Fundamental",
    "Quick",
    "Infernal",
    "Fit",
    "Fair",
    "Viral",
    "Tough",
    "Indoor",
    "Superb",
    "Resilient",
    "Hexagonal",
    "Responsive",
    "Anno",
    "Mo",
    "Di",
    "Mi",
    "Do",
    "Fr",
    "Sa",
    "Gr",
    "Mag",
    "Nov",
    "Diss",
    "Invalide",
    "Invalider",
    "Invaliden",
    "Sehende",
    "Sehender",
    "Sehenden",
    "Schutzheilige",
    "Schutzheiliger",
    "Schutzheiligen",
    "Lila",
    "Langzeitarbeitslose",
    "Langzeitarbeitslosen",
    "Langzeitarbeitsloser",
    "Linksintellektuelle",
    "Linksintellektueller",
    "Linksintellektuellen",
    "Beschuldigte",
    "Beschuldigten",
    "Drogenabhängige",
    "Drogenabhängiger",
    "Drogenabhängiger",
    "Drogenabhängigen",
    "Asylsuchender",
    "Asylsuchende",
    "Asylsuchenden",
    "Landtagsabgeordnete",
    "Landtagsabgeordneter",
    "Landtagsabgeordneten",
    "Stadtverordnete",
    "Stadtverordneter",
    "Stadtverordneten",
    "Veränderliche",
    "Veränderlicher",
    "Veränderlichen",
    "Werbetreibende",
    "Werbetreibender",
    "Werbetreibenden",
    "Verletzter",
    "Verletzten",
    "Werktätige",
    "Werktätiger",
    "Werktätigen",
    "Getestete",
    "Getesteten",
    "Genesene",
    "Genesenen",
    "Geimpfte",
    "Geboosterte",
    "Ungeimpfte",
    "Geimpften",
    "Geboosterten",
    "Ungeimpften",
    "Geflüchtete",
    "Geflüchteten",
    "Projektbeteiligte",
    "Projektbeteiligten",
    "Heranwachsende",
    "Heranwachsenden",
    "Interessierte",
    "Interessierten",
    "Infizierte",
    "Infizierten",
    "Gehörlose",
    "Gehörlosen",
    "Drücke",
    "Klecks",
    "Quatsch",
    "Speis",
    "Flash",
    "Suhl",
    "Müh",
    "Bims",
    "Wisch",
    "Außenputz",
    "Rinderhack",
    "Hack",
    "Schlitz",
    "Frevler",
    "Zementputz",
    "Hurst",
    "Bombardier",
    "Kraus",
    "Strunz",
    "Bell",
    "Melk",
    "Klopp",
    "Walz",
    "Schiel",
    "Dusch",
    "Penn",
    "Dörr",
    "Kies",
    "Koks",
    "Dell",
    "Wall",
    "Beige",
    "Zoom",
    "Perl",
    "Parallele",
    "Parallelen",
    "Rutsch",
    "Spar",
    "Merz",
    "Gefahren",
    "Minderjährige",
    "Minderjähriger",
    "Minderjährigen",
    "Scheinselbstständige",
    "Bundestagsabgeordneter",
    "Bundestagsabgeordneten",
    "Bundestagsabgeordnete",
    "Reichstagsabgeordneter",
    "Reichstagsabgeordneten",
    "Reichstagsabgeordnete",
    "Medienschaffende",
    "Medienschaffenden",
    "Medienschaffender",
    "Lehrende",
    "Lehrenden",
    "Vertretene",
    "Vertretenen",
    "Vorstandsvorsitzender",
    "Vorstandsvorsitzenden",
    "Vorstandsvorsitzende",
    "Demonstrierende",
    "Demonstrierenden",
    "Marketingtreibende",
    "Marketingtreibender",
    "Marketingtreibenden",
    "Strafgefangenen",
    "Strafgefangener",
    "Strafgefangene",
    "Pädophile",
    "Pädophiler",
    "Pädophilen",
    "Lehrbeauftragte",
    "Lehrbeauftragter",
    "Lehrbeauftragten",
    "Erkrankte",
    "Erkrankter",
    "Erkrankten",
    "Eigner",
    "Polizeibeamten",
    "Polizeibeamter",
    "Polizeibeamte",
    "Kriegsversehrte",
    "Kriegsversehrter",
    "Kriegsversehrten",
    "Demenzkranke",
    "Demenzkranker",
    "Demenzkranken",
    "Parteivorsitzende",
    "Parteivorsitzender",
    "Parteivorsitzenden",
    "Kriegsgefangene",
    "Kriegsgefangener",
    "Kriegsgefangenen",
    "Ehrenvorsitzende",
    "Ehrenvorsitzender",
    "Ehrenvorsitzenden",
    "Oberkommandierende",
    "Oberkommandierender",
    "Oberkommandierenden",
    "Werbungtreibende",
    "Werbungtreibenden",
    "Mitangeklagte",
    "Schuhfilz",
    "Mix",
    "Rahm",
    "Flansch",
    "WhatsApp",
    "Verschleiß",
    "Schutzsuchende",
    "Schutzsuchenden",
    "Versicherte",
    "Versicherten",
    "Cyberkriminelle",
    "Cyberkriminellen",
    "Kriminelle",
    "Kriminellen",
    "Auszubildenden",
    "Auszubildende",
    "Auszubildender",
    "Lernende",
    "Lernender",
    "Lernenden",
    "Teilnehmende",
    "Teilnehmenden",
    "Radfahrende",
    "Radfahrenden",
    "Autofahrende",
    "Autofahrenden",
    "Auszubildene",
    "Auszubildenen",
    "Absolvierende",
    "Absolvierenden",
    "Einheimische",
    "Einheimischen",
    "Einheimischer",
    "Wehrbeauftragter",
    "Wehrbeauftragte",
    "Wehrbeauftragten",
    "Wehrbeauftragtem",
    "Prozessbevollmächtigter",
    "Prozessbevollmächtigte",
    "Prozessbevollmächtigten",
    "Prozessbevollmächtigtem",
    "Bundesbeamte",
    "Bundesbeamter",
    "Bundesbeamten",
    "Bundesbeamtem",
    "Datenschutzbeauftragter",
    "Datenschutzbeauftragte",
    "Datenschutzbeauftragten",
    "Datenschutzbeauftragtem",
    "Steuerbevollmächtigte",
    "Steuerbevollmächtigter",
    "Steuerbevollmächtigten",
    "Steuerbevollmächtigtem",
    "Suchtkranken",
    "Suchtkranke",
    "Suchtkranker",
    "Filmschaffende",
    "Filmschaffender",
    "Filmschaffenden",
    "Filmschaffendem",
    "Arbeitssuchende",
    "Arbeitssuchender",
    "Arbeitssuchenden",
    "Arbeitssuchendem",
    "Bausachverständige",
    "Bausachverständiger",
    "Bausachverständigen",
    "Bausachverständigem",
    "Heurige",
    "Ratsuchende",
    "Ratsuchender",
    "Ratsuchenden",
    "Verwundete",
    "Verwundeter",
    "Verwundeten",
    "Vollzugsbeamte",
    "Vollzugsbeamter",
    "Vollzugsbeamten",
    "Schutzbefohlene",
    "Schutzbefohlener",
    "Schutzbefohlenen",
    "Verfahrensbeteiligte",
    "Verfahrensbeteiligter",
    "Verfahrensbeteiligten",
    "Kolonialbeamte",
    "Kolonialbeamter",
    "Kolonialbeamten",
    "Verwaltungsbeamte",
    "Verwaltungsbeamter",
    "Verwaltungsbeamten",
    "Verdächtige",
    "Verdächtiger",
    "Verdächtigen",
    "Leichtverletzte",
    "Leichtverletzten",
    "Leichtverletzte",
    "Dozierende",
    "Dozierenden",
    "Studierende",
    "Studierender",
    "Studierenden",
    "Suchbegriffen",
    "Plattdeutsch",
    "Wallet",
    "Str",
    "Priest",
    "Simple",
    "Legend",
    "Golden",
    "Forward",
    "Unverzagt",
    "Auszubildende",
    "Auszubildender",
    "Gelehrte",
    "Gelehrter",
    "Gelehrten",
    "Vorstehende",
    "Vorstehender",
    "Mitwirkende",
    "Mitwirkender",
    "Mitwirkenden",
    "Tabellenletzte",
    "Tabellenletzter",
    "Familienangehörige",
    "Familienangehöriger",
    "Zeitreisende",
    "Zeitreisender",
    "Zeitreisenden",
    "Erwerbstätige",
    "Erwerbstätigen",
    "Erwerbstätiger",
    "Selbstständige",
    "Selbstständigen",
    "Selbstständiger",
    "Selbständige",
    "Selbständigen",
    "Selbständiger",
    "Genaueres",
    "Äußersten",
    "Dienstreisender",
    "Verletzte",
    "Vermisste",
    "Äußeres",
    "Abseits",
    "Unschuldige",
    "Unschuldiger",
    "Unschuldigen",
    "Mitarbeitende",
    "Mitarbeitender",
    "Mitarbeitenden",
    "Beschäftigter",
    "Beschäftigte",
    "Beschäftigten",
    "Beschäftigtem",
    "Bekannter",
    "Bekannte",
    "Bevollmächtigte",
    "Bevollmächtigter",
    "Bevollmächtigten",
    "Brecht",
    "Tel",
    "Unschuldiger",
    "Vorgesetzter",
    "Abs",
    "Klappe",
    "Vorfahre",
    "Mittler",
    "Hr",
    "Schwarz",
    "Genese",
    "Rosa",
    "Auftrieb",
    "Zuschnitt",
    "Geschossen",
    "Vortrieb",
    "Abtrieb",
    "Gesandter",
    "Durchfahrt",
    "Durchgriff",
    "Überfahrt",
    "Zeche",
    "Sparte",
    "Sparten",
    "Heiliger",
    "Reisender",
    "Pest",
    "Schwinge",
    "Verlies",
    "Nachfolge",
    "Stift",
    "Belange",
    "Geistlicher",
    "Google",
    "Hu",
    "Jenseits",
    "Abends",
    "Stimmberechtigte",
    "Stimmberechtigten",
    "Stimmberechtigter",
    "Alleinerziehende",
    "Alleinerziehenden",
    "Alleinerziehender",
    "Abgeordneter",
    "Abgeordnete",
    "Abgeordneten",
    "Angestellter",
    "Angestellte",
    "Angestellten",
    "Armeeangehörige",
    "Armeeangehörigen",
    "Armeeangehöriger",
    "Liberaler",
    "Abriss",
    "Ahne",
    "Ähnlichem",
    "Ähnliches",
    "Allerlei",
    "Anklang",
    "Verlobter",
    "Anstrich",
    "Armes",
    "Ausdrücke",
    "Auswüchsen",
    "Bände",
    "Bänden",
    "Beauftragter",
    "Belange",
    "Biss",
    "De",
    "Diesseits",
    "Dr",
    "Durcheinander",
    "Eindrücke",
    "Erwachsener",
    "Familienangehörige",
    "Flöße",
    "Folgendes",
    "Fort",
    "Fraß",
    "Fristende",
    "Frevel",
    "Genüge",
    "Gefallen",
    "Gläubige",
    "Gläubiger",
    "Gläubigen",
    "Hechte",
    "Herzöge",
    "Herzögen",
    "Hinfahrt",
    "Hilfsstoff",
    "Hilfsstoffe",
    "Hundert",
    "Zehntausend",
    "Hunderttausend",
    "Hyperwallet",
    "Ihnen",
    "Ihrerseits",
    "Ihr",
    "Ihre",
    "Ihrem",
    "Ihren",
    "Ihrer",
    "Ihres",
    "Infrarot",
    "Jenseits",
    "Jugendliche",
    "Jugendlichen",
    "Jugendlicher",
    "Jünger",
    "Kant",
    "Klaue",
    "Konditional",
    "Krähe",
    "Kurzem",
    "Landwirtschaft",
    "Langem",
    "Längerem",
    "Lausitz",
    "Le",
    "Lehrlingsunterweisung",
    "Letzt",
    "Letzt",
    "Letztere",
    "Letzterer",
    "Letzteres",
    "Link",
    "Links",
    "Löhne",
    "Luden",
    "Milk",
    "Mitfahrt",
    "Mr",
    "Mrd",
    "Mrs",
    "Nachfrage",
    "Nachts",
    "Nachspann",
    "Nähte",
    "Nähten",
    "Narkoseverfahren",
    "Neuem",
    "Neugeborene",
    "Neugeborenen",
    "Neugeborenes",
    "Nr",
    "Nutze",
    "Obdachlose",
    "Obdachloser",
    "Obdachlosen",
    "Oder",
    "Ohrfeige",
    "Patsche",
    "Pfiffe",
    "Pfiffen",
    "Press",
    "Prof",
    "Puste",
    "Sachverständiger",
    "Sankt",
    "Schaulustige",
    "Schaulustigen",
    "Schaulustiger",
    "Scheine",
    "Scheiße",
    "Schuft",
    "Schufte",
    "Schuld",
    "Schwangere",
    "Schwangeren",
    "Schwärme",
    "Schwarzes",
    "Sie",
    "Skype",
    "Spitz",
    "Spott",
    "St",
    "Stereotyp",
    "Störe",
    "Tausend",
    "Tischende",
    "Toter",
    "Übrigen",
    "Unentschieden",
    "Unvorhergesehenes",
    "Verantwortlicher",
    "Verlass",
    "Verwandter",
    "Verstorbenen",
    "Verstorbene",
    "Vielfache",
    "Vielfaches",
    "Vorsitzender",
    "Fraktionsvorsitzender",
    "Verletzte",
    "Verletzten",
    "Walt",
    "Weitem",
    "Weiteres",
    "Wohlen",
    "Wicht",
    "Wichtiges",
    "Wider",
    "Wild",
    "Zeche",
    "Zusage",
    "Zwinge",
    "Zirkusrund",
    "Tertiär",
    "Erster",
    "Zweiter",
    "Dritter",
    "Vierter",
    "Fünfter",
    "Sechster",
    "Siebter",
    "Achter",
    "Neunter",
    "Erste",
    "Zweite",
    "Dritte",
    "Vierte",
    "Fünfte",
    "Sechste",
    "Siebte",
    "Achte",
    "Neunte",
    "Dein",
    "Deine",
    "Deinem",
    "Deinen",
    "Deiner",
    "Deines",
    "Deinerseits",
    "Dich",
    "Dir",
    "Du",
    "Euch",
    "Euer",
    "Eure",
    "Euern",
    "Eurem",
    "Euren",
    "Eures",
    "Eueren",
    "Euerem",
    "Eueres",
    "Euerer",
    "Eurerseits",
    "Euerseits",
];

pub struct CaseRule {
    tagger: Arc<lt_tagger::GermanTagger>,
    spelling: Arc<crate::de::spelling::GermanSpellingRule>,
    /// `CaseRuleExceptions.getExceptionPatterns()`
    exception_patterns: Vec<Vec<regex::Regex>>,
}

impl CaseRule {
    /// Load the exception phrase lists (rules-dir resources).
    pub fn load(
        data_dir: &Path,
        tagger: Arc<lt_tagger::GermanTagger>,
        spelling: Arc<crate::de::spelling::GermanSpellingRule>,
    ) -> Result<Self> {
        let mut phrases: Vec<String> = Vec::new();
        for name in ["eigennamen_gross.txt", "case_rule_exceptions.txt"] {
            let path = data_dir.join("de/words").join(name);
            let Ok(text) = lt_data::fs::read_to_string(&path) else {
                continue;
            };
            for line in text.lines() {
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                phrases.push(line.to_string());
            }
        }
        let mut exception_patterns = Vec::with_capacity(phrases.len());
        for phrase in phrases {
            let parts: Vec<&str> = phrase.split(' ').filter(|p| !p.is_empty()).collect();
            let mut compiled = Vec::with_capacity(parts.len());
            let mut ok = true;
            for part in parts {
                match regex::Regex::new(&format!("^(?:{part})$")) {
                    Ok(re) => compiled.push(re),
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && !compiled.is_empty() {
                exception_patterns.push(compiled);
            }
        }
        Ok(Self {
            tagger,
            spelling,
            exception_patterns,
        })
    }

    /// `estimateContextForSureMatch`.
    pub fn estimate_context_for_sure_match() -> i32 {
        case_rule_antipatterns::antipattern_defs()
            .iter()
            .map(|p| p.len() as i32)
            .max()
            .unwrap_or(0)
    }

    fn lookup(&self, word: &str) -> Option<AnalyzedTokenReadings> {
        self.tagger.lookup(word)
    }

    /// `CaseRule.match`.
    pub fn match_sentence(
        &self,
        sentence: &AnalyzedSentence,
        sentence_offset: usize,
    ) -> Vec<Match> {
        let mut rule_matches = Vec::new();
        let immunized = util::immunize_sentence(sentence, &ANTI_PATTERNS);
        let tokens = immunized.tokens_without_whitespace();
        let mut prev_token_is_das = false;
        let mut is_preceded_by_modal_or_auxiliary = false;
        for i in 0..tokens.len() {
            let pos_token = tokens[i].readings.first().and_then(|r| r.pos_tag.clone());
            if pos_token.as_deref() == Some("SENT_START") {
                continue;
            }
            if i == 1 {
                prev_token_is_das =
                    NOUN_INDICATORS.contains(&tokens[1].surface().to_lowercase().as_str());
                continue;
            }
            if i > 0
                && (is_salutation(tokens[i - 1].surface()) || is_company(tokens[i - 1].surface()))
            {
                continue;
            }
            // 1.1 Technische Dokumentation
            if i > 2
                && NUMERALS_EN.is_match(tokens[i - 1].surface())
                && is_dot(tokens[i - 2].surface())
                && NUMERALS_EN.is_match(tokens[i - 3].surface())
            {
                continue;
            }
            let analyzed_token = tokens[i];
            let token = analyzed_token.surface().to_string();
            // Java `AnalyzedTokenReadings.hasLemma`: the lemma must be
            // non-null and equal (the `lemma()` fallback used elsewhere would
            // treat the unknown reading of "Die" as a base form)
            let is_baseform = analyzed_token
                .readings
                .iter()
                .any(|r| r.stem.as_deref() == Some(token.as_str()));
            if (analyzed_token
                .readings
                .first()
                .and_then(|r| r.pos_tag.as_ref())
                .is_none()
                || german_helper::has_reading_of_type(analyzed_token, PosType::Verb))
                && is_baseform
            {
                let mut next_token_is_personal_or_reflexive_pronoun = false;
                if i < tokens.len() - 1 {
                    let next_token = tokens[i + 1];
                    next_token_is_personal_or_reflexive_pronoun =
                        util::has_partial_pos_tag(next_token, "PRO:PER")
                            || matches!(next_token.surface(), "sich" | "Sie");
                    if next_token.has_pos_tag("PKT") {
                        continue;
                    }
                    if (prev_token_is_das
                        && (DAS_VERB_EXCEPTIONS.contains(&next_token.surface())
                            || is_followed_by_relative_or_subordinate_clause(i, &tokens)))
                        || (i > 1 && partial_tag_any(tokens[i - 2], &["VER:AUX", "VER:MOD"]))
                    {
                        continue;
                    }
                }
                if is_prev_probably_relative_pronoun(&tokens, i)
                    || (prev_token_is_das && count_pos_tag_starting_with(&tokens, "VER") == 1)
                {
                    continue;
                }
                self.potentially_add_lowercase_match(
                    &mut rule_matches,
                    sentence_offset,
                    analyzed_token,
                    prev_token_is_das,
                    &token,
                    next_token_is_personal_or_reflexive_pronoun,
                );
            }
            prev_token_is_das =
                NOUN_INDICATORS.contains(&tokens[i].surface().to_lowercase().as_str());
            if analyzed_token.has_pos_tag_matching(&VER_MOD_AUX) {
                is_preceded_by_modal_or_auxiliary = true;
            }
            let lowercase_readings = self.lookup(&token.to_lowercase());
            if self.has_noun_reading(Some(analyzed_token)) {
                if !self.is_potential_upper_case_error(
                    i,
                    &tokens,
                    lowercase_readings.as_ref(),
                    is_preceded_by_modal_or_auxiliary,
                ) {
                    continue;
                }
            } else if analyzed_token.has_pos_tag_starting_with("SUB:")
                && i < tokens.len() - 1
                && tokens[i + 1]
                    .surface()
                    .chars()
                    .next()
                    .is_some_and(char::is_lowercase)
                && tokens[i + 1].has_pos_tag_matching(&util::anchored(r"(VER:[123]:|PA2).+"))
            {
                // "Viele Minderjährige sind" but not "Das wirklich Wichtige Verfahren ist"
                continue;
            }
            if analyzed_token
                .readings
                .first()
                .and_then(|r| r.pos_tag.as_ref())
                .is_none()
                && lowercase_readings.is_none()
            {
                continue;
            }
            if analyzed_token
                .readings
                .first()
                .and_then(|r| r.pos_tag.as_ref())
                .is_none()
                && lowercase_readings.is_some()
                && (lowercase_readings
                    .as_ref()
                    .and_then(|r| r.readings.first())
                    .and_then(|r| r.pos_tag.as_ref())
                    .is_none()
                    || token.ends_with("innen"))
            {
                continue; // unknown word, probably a name etc.
            }
            self.potentially_add_uppercase_match(
                &mut rule_matches,
                sentence_offset,
                &tokens,
                i,
                analyzed_token,
                &token,
                lowercase_readings.as_ref(),
            );
        }
        rule_matches
    }

    fn is_potential_upper_case_error(
        &self,
        pos: usize,
        tokens: &[&AnalyzedTokenReadings],
        lowercase_readings: Option<&AnalyzedTokenReadings>,
        is_preceded_by_modal_or_auxiliary: bool,
    ) -> bool {
        if pos <= 1 {
            return false;
        }
        // "Das ist zu Prüfen." but not "Das geht zu Herzen."
        if tokens[pos - 1].surface() == "zu"
            && !tokens[pos].has_pos_tag_matching(&util::anchored(".*(NEU|MAS|FEM)$"))
            && lowercase_readings.is_some_and(|r| r.has_pos_tag_starting_with("VER:INF"))
        {
            return true;
        }
        if VERHALTEN.is_match(tokens[pos].surface()) {
            return false;
        }
        // find error in: "Man müsse Überlegen, wie man das Problem löst."
        let mut is_potential_error = pos < tokens.len().saturating_sub(3)
            && tokens[pos + 1].surface() == ","
            && INTERROGATIVE_PARTICLES.contains(&tokens[pos + 2].surface())
            && tokens[pos - 1].has_pos_tag_starting_with("VER:MOD")
            && !tokens[pos - 1].has_lemma("mögen")
            && tokens[pos + 3].surface() != "zum";
        if !is_potential_error
            && partial_tag_any(tokens[pos], &["SUB:NOM:SIN:NEU:INF", "SUB:DAT:PLU:"])
            && (tokens[pos - 1].surface() == "zu"
                || partial_tag_any(
                    tokens[pos - 1],
                    &["SUB", "EIG", "VER:AUX:3:", "ADV:TMP", "ABK"],
                ))
        {
            if let Some(lr) = lowercase_readings {
                // find error in: "Der Brief wird morgen Übergeben." / "Die Ausgaben haben eine Mrd. Euro Überschritten."
                is_potential_error |= lr.has_pos_tag("PA2:PRD:GRU:VER")
                    && !tokens[pos - 1].has_pos_tag_starting_with("VER:AUX:3")
                    && !lr.has_pos_tag("VER:3:PLU:PRT:NON");
                // find error in: "Er lässt das Arktisbohrverbot Überprüfen." / "Sie bat ihn, es zu Überprüfen." / "Das Geld wird Überwiesen."
                is_potential_error |= (pos >= tokens.len() - 2 || tokens[pos + 1].surface() == ",")
                    && (tokens[pos - 1].surface() == "zu" || is_preceded_by_modal_or_auxiliary)
                    && tokens[pos].surface().starts_with("Über")
                    && partial_tag_any(lr, &["VER:INF:", "PA2:PRD:GRU:VER"]);
            }
        }
        is_potential_error
    }

    /// `hasNounReading`.
    fn has_noun_reading(&self, readings: Option<&AnalyzedTokenReadings>) -> bool {
        let Some(readings) = readings else {
            return false;
        };
        if readings.has_pos_tag_starting_with("ABK") && util::has_partial_pos_tag(readings, "SUB") {
            return true;
        }
        // "Die Schöne Tür": "Schöne" also has a noun reading but like
        // "SUB:AKK:SIN:FEM:ADJ", ignore that:
        let word = readings.surface().replace('\u{00AD}', "");
        if let Some(all) = self.lookup(&word) {
            for reading in &all.readings {
                if let Some(tag) = reading.pos_tag.as_deref() {
                    if tag.contains("SUB:") && !tag.contains(":ADJ") {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// `potentiallyAddLowercaseMatch`.
    #[allow(clippy::too_many_arguments)]
    fn potentially_add_lowercase_match(
        &self,
        rule_matches: &mut Vec<Match>,
        sentence_offset: usize,
        token_readings: &AnalyzedTokenReadings,
        prev_token_is_das: bool,
        token: &str,
        next_token_is_personal_or_reflexive_pronoun: bool,
    ) {
        // e.g. essen -> Essen
        if prev_token_is_das
            && !next_token_is_personal_or_reflexive_pronoun
            && token.chars().next().is_some_and(char::is_lowercase)
            && !SUBST_VERBEN_EXCEPTIONS.contains(&token)
            && token_readings.has_pos_tag_starting_with("VER:INF")
            && !token_readings.is_ignore_spelling
            && !token_readings.is_immunized
        {
            add_rule_match(
                rule_matches,
                sentence_offset,
                LOWERCASE_MESSAGE,
                token_readings,
                &lt_tagger::uppercase_first_char(token_readings.surface()),
            );
        }
    }

    /// `potentiallyAddUppercaseMatch`.
    #[allow(clippy::too_many_arguments)]
    fn potentially_add_uppercase_match(
        &self,
        rule_matches: &mut Vec<Match>,
        sentence_offset: usize,
        tokens: &[&AnalyzedTokenReadings],
        i: usize,
        analyzed_token: &AnalyzedTokenReadings,
        token: &str,
        lowercase_readings: Option<&AnalyzedTokenReadings>,
    ) {
        let is_upper_first = token.chars().next().is_some_and(char::is_uppercase);
        let lc_word = lt_tagger::lowercase_first_char(tokens[i].surface());
        if is_upper_first
            && token.chars().count() > 1 // length limit = ignore abbreviations
            && !tokens[i].is_ignore_spelling
            && !tokens[i].is_immunized
            && !SENTENCE_START_EXCEPTIONS.contains(&tokens[i - 1].surface())
            && !EXCEPTIONS.contains(&token)
            && !lt_tagger::is_all_uppercase(token)
            && !self.is_language(i, tokens, token)
            && !is_probably_city(i, tokens, token)
            && !german_helper::has_reading_of_type(analyzed_token, PosType::ProperNoun)
            && !analyzed_token.is_sentence_end
            && !is_ellipsis(i, tokens)
            && !is_numbering(i, tokens)
            && !self.is_nominalization(i, tokens, token, lowercase_readings)
            && !self.is_adverb_and_nominalization(i, tokens)
            && !self.is_special_case(i, tokens)
            && !self.is_adjective_as_noun(i, tokens, lowercase_readings)
            && !is_singular_imperative(lowercase_readings, tokens[i])
            && !self.is_exception_phrase(i, tokens)
            && !(i == 2 && tokens[i - 1].surface() == "“")
            && !is_case_typo(token)
            && !followed_by_gender_gap(tokens, i)
            && !is_noun_with_verb_reading(tokens, i)
            && !is_invisible_separator(i as isize - 1, tokens)
            && !self.spelling.is_misspelled(&lc_word)
        {
            if tokens[i - 1].surface() == ":" {
                // allow short question sentences like "Warum? Und warum?" after colon
                if is_question_equivalent_after_colon(i, tokens) {
                    return;
                }
                let verb_following = is_verb_following(i, tokens, lowercase_readings);
                if verb_following || count_pos_tag_starting_with(&tokens[..i], "VER") == 0 {
                } else {
                    add_rule_match(
                        rule_matches,
                        sentence_offset,
                        COLON_MESSAGE,
                        tokens[i],
                        &lc_word,
                    );
                }
                return;
            }
            add_rule_match(
                rule_matches,
                sentence_offset,
                UPPERCASE_MESSAGE,
                tokens[i],
                &lc_word,
            );
        }
    }

    /// `isLanguage`.
    fn is_language(&self, i: usize, tokens: &[&AnalyzedTokenReadings], token: &str) -> bool {
        let names = language_names::get();
        let maybe_language = (token.ends_with("sch") && names.contains(&token))
            || names.contains(&remove_end(&remove_end(token, "n"), "e").as_str());
        let prev_token = tokens.get(i.wrapping_sub(1)).copied();
        let next_readings = tokens.get(i + 1).copied();
        maybe_language
            && (!self.has_noun_reading(next_readings)
                || prev_token.is_some_and(|t| t.surface() == "auf"))
    }

    /// `isNominalization`.
    fn is_nominalization(
        &self,
        i: usize,
        tokens: &[&AnalyzedTokenReadings],
        token: &str,
        lowercase_readings: Option<&AnalyzedTokenReadings>,
    ) -> bool {
        let next_readings = tokens.get(i + 1).copied();
        if !util::starts_with_uppercase(token)
            || self.is_number(token)
            || self.has_noun_reading(next_readings)
            || next_readings.is_some_and(|t| is_numeric(t.surface()))
            || ALLE_NM.is_match(token)
        {
            return false;
        }
        if lowercase_readings.is_some_and(|r| r.has_pos_tag("PRP:LOK+TMP+CAU:DAT+AKK")) {
            return false;
        }
        // Ignore "das Dümmste, was je..." but not "das Dümmste Kind"
        let prev_token = i.checked_sub(1).and_then(|p| tokens.get(p)).copied();
        let prev_prev_token = i.checked_sub(2).and_then(|p| tokens.get(p)).copied();
        let prev_prev_prev_token = i.checked_sub(3).and_then(|p| tokens.get(p)).copied();
        let prev_token_str = prev_token.map(|t| t.surface()).unwrap_or("");
        if ["und", "oder", "beziehungsweise"].contains(&prev_token_str)
            && prev_prev_token.is_some()
            && ((util::has_partial_pos_tag(tokens[i], "SUB")
                && util::has_partial_pos_tag(tokens[i], ":ADJ"))
                || (prev_prev_token.is_some_and(|t| util::has_partial_pos_tag(t, "SUB"))
                    && !self.has_noun_reading(next_readings)
                    && lowercase_readings.is_some_and(|r| util::has_partial_pos_tag(r, "ADJ"))
                    && prev_token_str != ","))
        {
            return true;
        }
        if lowercase_readings.is_some_and(|r| r.has_pos_tag("PA1:PRD:GRU:VER")) {
            // "aus sechs Überwiegend muslimischen Ländern"
            return false;
        }
        (prev_token.is_some()
            && IRGEND_ETC.is_match(prev_token_str)
            && util::has_partial_pos_tag(tokens[i], "SUB"))
            || self.is_number(prev_token_str)
            || (partial_tag_any_opt(prev_token, &["ART", "PRO:"])
                && !(((i < 4 && tokens.len() > 4)
                    || prev_token.is_some_and(|t| t.readings.len() == 1)
                    || prev_prev_token.is_some_and(|t| t.has_lemma("sein")))
                    && prev_token.is_some_and(|t| t.has_pos_tag_starting_with("PRO:PER:NOM:")))
                && !prev_token.is_some_and(|t| util::has_partial_pos_tag(t, ":STD")))
            || (partial_tag_any_opt(prev_prev_prev_token, &["ART"])
                && partial_tag_any_opt(prev_prev_token, &["PRP"])
                && partial_tag_any_opt(prev_token, &["SUB"]))
            || (partial_tag_any_opt(prev_prev_token, &["PRO:", "PRP"])
                && partial_tag_any_opt(prev_token, &["ADJ", "ADV", "PA2", "PA1"]))
            || (partial_tag_any_opt(prev_prev_prev_token, &["PRO:", "PRP"])
                && partial_tag_any_opt(prev_prev_token, &["ADJ", "ADV"])
                && partial_tag_any_opt(prev_token, &["ADJ", "ADV", "PA2"]))
            || (tokens[i].has_pos_tag_starting_with("SUB:")
                && partial_tag_any_opt(prev_token, &["GEN"])
                && !partial_tag_any_opt(next_readings, &["PKT"]))
    }

    /// `isNumber`.
    fn is_number(&self, token: &str) -> bool {
        if is_numeric(token) {
            return true;
        }
        let lookup = self.lookup(&lt_tagger::lowercase_first_char(token));
        lookup.is_some_and(|l| l.has_pos_tag("ZAL"))
    }

    /// `isAdjectiveAsNoun`.
    fn is_adjective_as_noun(
        &self,
        i: usize,
        tokens: &[&AnalyzedTokenReadings],
        lowercase_readings: Option<&AnalyzedTokenReadings>,
    ) -> bool {
        let prev_token = i.checked_sub(1).and_then(|p| tokens.get(p)).copied();
        let next_readings = tokens.get(i + 1).copied();
        let mut prev_lowercase_readings: Option<AnalyzedTokenReadings> = None;
        if i > 1 && SENTENCE_START_EXCEPTIONS.contains(&tokens[i - 2].surface()) {
            prev_lowercase_readings =
                prev_token.and_then(|t| self.lookup(&t.surface().to_lowercase()));
        }
        // ignore "Der Versuch, Neues zu lernen / Gutes zu tun / Spannendes auszuprobieren"
        let is_possibly_followed_by_infinitive = next_readings.is_some_and(|t| t.surface() == "zu");
        let is_followed_by_infinitive = next_readings.is_some_and(|t| {
            !is_possibly_followed_by_infinitive && util::has_partial_pos_tag(t, "EIZ")
        });
        let is_followed_by_possessive_indicator =
            next_readings.is_some_and(|t| POSSESSIVE_INDICATORS.contains(&t.surface()));
        let is_undef_quantifier = prev_token
            .is_some_and(|t| UNDEFINED_QUANTIFIERS.contains(&t.surface().to_lowercase().as_str()));
        let is_prev_determiner = prev_token.is_some()
            && (partial_tag_any_opt(prev_token, &["ART", "PRP", "ZAL"])
                || partial_tag_any_opt(prev_lowercase_readings.as_ref(), &["ART", "PRP", "ZAL"]))
            && !prev_token.is_some_and(|t| util::has_partial_pos_tag(t, ":STD"));
        let is_preceded_by_verb = prev_token.is_some_and(|t| {
            t.has_pos_tag_matching(&util::anchored("VER:(MOD:|AUX:)?[1-3]:.*"))
                && !t.has_lemma("sein")
        });
        let no_determiner_context = !(is_prev_determiner
            || is_undef_quantifier
            || is_possibly_followed_by_infinitive
            || is_followed_by_infinitive);
        if no_determiner_context
            && !(is_preceded_by_verb
                && lowercase_readings
                    .is_some_and(|r| partial_tag_any_opt(Some(r), &["ADJ:", "PA"]))
                && next_readings.is_some_and(|t| !matches!(t.surface(), "und" | "oder" | ",")))
            && !(is_followed_by_possessive_indicator
                && lowercase_readings
                    .is_some_and(|r| partial_tag_any_opt(Some(r), &["ADJ", "VER"])))
            && !(prev_token.is_some_and(|t| t.has_pos_tag("KON:UNT"))
                && !self.has_noun_reading(next_readings)
                && next_readings.is_some_and(|t| !t.has_pos_tag("KON:NEB")))
        {
            let prev_prev_token =
                if i > 1 && prev_token.is_some_and(|t| util::has_partial_pos_tag(t, "ADJ")) {
                    i.checked_sub(2).and_then(|p| tokens.get(p)).copied()
                } else {
                    None
                };
            // Another check to avoid false alarms for "eine Gruppe Aufständischer starb"
            if !is_preceded_by_verb && lowercase_readings.is_some() {
                if let Some(prev_token) = prev_token {
                    if util::has_partial_pos_tag(prev_token, "SUB:")
                        && lowercase_readings.is_some_and(|r| {
                            r.has_pos_tag_matching(&util::anchored(
                                "(ADJ|PA2):GEN:PLU:MAS:GRU:SOL.*",
                            ))
                        })
                    {
                        return next_readings
                            .is_some_and(|t| !util::has_partial_pos_tag(t, "SUB:"));
                    } else if next_readings.is_some_and(|t| {
                        t.readings.len() == 1
                            && prev_token.has_pos_tag_starting_with("PRO:PER:NOM:")
                            && t.has_pos_tag("ADJ:PRD:GRU")
                    }) {
                        // avoid false alarm "Weil er Unmündige sexuell missbraucht haben soll,..."
                        return true;
                    }
                }
            }
            // Another check to avoid false alarms for "ein politischer Revolutionär"
            if !partial_tag_any_opt(prev_prev_token, &["ART", "PRP", "ZAL"]) {
                return false;
            }
        }
        // ignore "die Ausgewählten" but not "die Ausgewählten Leute":
        for reading in &tokens[i].readings {
            let pos_tag = reading.pos_tag.as_deref();
            if (pos_tag.is_none() || pos_tag.is_some_and(|t| t.contains("ADJ")))
                && !self.has_noun_reading(next_readings)
                && !is_numeric(next_readings.map(|t| t.surface()).unwrap_or(""))
            {
                if pos_tag.is_none()
                    && lowercase_readings.is_some_and(|r| {
                        partial_tag_any_opt(
                            Some(r),
                            &[
                                "PRP:LOK",
                                "PA2:PRD:GRU:VER",
                                "PA1:PRD:GRU:VER",
                                "ADJ:PRD:KOM",
                                "ADV:TMP",
                            ],
                        )
                    })
                {
                    // skip to avoid a false true for, e.g. "Die Zahl ging auf Über 1.000 zurück."
                } else {
                    return true;
                }
            }
        }
        false
    }

    /// `isAdverbAndNominalization`.
    fn is_adverb_and_nominalization(&self, i: usize, tokens: &[&AnalyzedTokenReadings]) -> bool {
        let prev_prev_token = if i > 1 { tokens[i - 2].surface() } else { "" };
        let prev_token = if i > 0 { Some(tokens[i - 1]) } else { None };
        let token = tokens[i].surface();
        let next_readings = tokens.get(i + 1).copied();
        // ignore "das wirklich Wichtige":
        prev_prev_token.eq_ignore_ascii_case("das")
            && partial_tag_any_opt(prev_token, &["ADV"])
            && util::starts_with_uppercase(token)
            && !self.has_noun_reading(next_readings)
    }

    /// `isSpecialCase`.
    fn is_special_case(&self, i: usize, tokens: &[&AnalyzedTokenReadings]) -> bool {
        let prev_token = if i > 1 { tokens[i - 1].surface() } else { "" };
        let token = tokens[i].surface();
        let next_readings = tokens.get(i + 1).copied();
        // ignore "im Allgemeinen gilt" but not "im Allgemeinen Fall":
        prev_token.eq_ignore_ascii_case("im")
            && token == "Allgemeinen"
            && !self.has_noun_reading(next_readings)
    }

    /// `isExceptionPhrase`.
    fn is_exception_phrase(&self, i: usize, tokens: &[&AnalyzedTokenReadings]) -> bool {
        for patterns in &self.exception_patterns {
            for (j, pattern) in patterns.iter().enumerate() {
                if pattern.is_match(tokens[i].surface()) {
                    let start = i as isize - j as isize;
                    if compare_lists(tokens, start, start + patterns.len() as isize - 1, patterns) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// `CaseRule.compareLists` (`@VisibleForTesting` upstream).
pub(crate) fn compare_lists(
    tokens: &[&AnalyzedTokenReadings],
    start_index: isize,
    end_index: isize,
    patterns: &[regex::Regex],
) -> bool {
    if start_index < 0 {
        return false;
    }
    let mut pattern_index = 0usize;
    let mut j = start_index;
    while j <= end_index {
        if pattern_index >= patterns.len()
            || j as usize >= tokens.len()
            || !patterns[pattern_index].is_match(tokens[j as usize].surface())
        {
            return false;
        }
        pattern_index += 1;
        j += 1;
    }
    true
}

/// `CaseRule.isQuestionEquivalentAfterColon` (`@VisibleForTesting` upstream).
pub(crate) fn is_question_equivalent_after_colon(
    i: usize,
    tokens: &[&AnalyzedTokenReadings],
) -> bool {
    if i < tokens.len() - 1 {
        let word = tokens[i].surface();
        let next = tokens[i + 1].surface();
        // "Warum?"
        if is_colon_question_word(word) && next == "?" {
            return true;
        }
        // "Und warum?"
        if COLON_QUESTION_CONJUNCTIONS
            .iter()
            .any(|w| word.eq_ignore_ascii_case(w))
            && i < tokens.len() - 2
            && is_colon_question_word(tokens[i + 1].surface())
            && tokens[i + 2].surface() == "?"
        {
            return true;
        }
    }
    false
}

fn is_colon_question_word(word: &str) -> bool {
    COLON_QUESTION_WORDS
        .iter()
        .any(|w| word.eq_ignore_ascii_case(w))
}

/// `StringUtils.removeEnd`.
fn remove_end(text: &str, suffix: &str) -> String {
    text.strip_suffix(suffix).unwrap_or(text).to_string()
}

/// `StringUtils.isNumeric`.
fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(char::is_numeric)
}

/// `CaseRule.getTokensWithPosTagStartingWithCount`.
fn count_pos_tag_starting_with(tokens: &[&AnalyzedTokenReadings], partial: &str) -> usize {
    tokens
        .iter()
        .filter(|t| t.has_pos_tag_starting_with(partial))
        .count()
}

/// `hasPartialTag` for a possibly absent token.
fn partial_tag_any_opt(token: Option<&AnalyzedTokenReadings>, tags: &[&str]) -> bool {
    token.is_some_and(|t| tags.iter().any(|tag| util::has_partial_pos_tag(t, tag)))
}

fn partial_tag_any(token: &AnalyzedTokenReadings, tags: &[&str]) -> bool {
    tags.iter().any(|tag| util::has_partial_pos_tag(token, tag))
}

fn is_salutation(token: &str) -> bool {
    ["Herr", "Hr", "Herrn", "Frau", "Fr", "Fräulein"].contains(&token)
}

fn is_company(token: &str) -> bool {
    [
        "Firma",
        "Familie",
        "Unternehmen",
        "Firmen",
        "Bäckerei",
        "Metzgerei",
        "Fa",
    ]
    .contains(&token)
}

fn is_dot(token: &str) -> bool {
    token == "."
}

fn followed_by_gender_gap(tokens: &[&AnalyzedTokenReadings], i: usize) -> bool {
    i + 2 < tokens.len()
        && tokens[i + 1].surface() == ":"
        && matches!(tokens[i + 2].surface(), "in" | "innen")
}

fn is_case_typo(token: &str) -> bool {
    TWO_UPPERCASE_CHARS.is_match(token)
}

fn is_singular_imperative(
    lowercase_readings: Option<&AnalyzedTokenReadings>,
    token: &AnalyzedTokenReadings,
) -> bool {
    lowercase_readings.is_some_and(|r| r.has_pos_tag_starting_with("VER:IMP:SIN"))
        && !matches!(token.surface(), "Ein" | "Eine")
}

fn is_noun_with_verb_reading(tokens: &[&AnalyzedTokenReadings], i: usize) -> bool {
    tokens[i].has_pos_tag_starting_with("SUB") && tokens[i].has_pos_tag_starting_with("VER:INF")
}

fn is_invisible_separator(i: isize, tokens: &[&AnalyzedTokenReadings]) -> bool {
    i >= 0
        && (i as usize) < tokens.len()
        && tokens[i as usize]
            .surface()
            .chars()
            .next()
            .is_some_and(|c| c == '\u{2063}')
}

fn is_verb_following(
    i: usize,
    tokens: &[&AnalyzedTokenReadings],
    lowercase_readings: Option<&AnalyzedTokenReadings>,
) -> bool {
    let mut subarray: Vec<&AnalyzedTokenReadings> = tokens[i..].to_vec();
    if let Some(lr) = lowercase_readings {
        subarray[0] = lr;
    }
    count_pos_tag_starting_with(&subarray, "VER:") != 0
}

fn is_numbering(i: usize, tokens: &[&AnalyzedTokenReadings]) -> bool {
    i >= 2
        && matches!(tokens[i - 1].surface(), ")" | "]")
        && NUMERALS_EN.is_match(tokens[i - 2].surface())
        && !(i > 3
            && tokens[i - 3].surface() == "("
            && tokens[i - 4].has_pos_tag_starting_with("SUB:"))
}

fn is_ellipsis(i: usize, tokens: &[&AnalyzedTokenReadings]) -> bool {
    matches!(tokens[i - 1].surface(), "]" | ")")
        && ((i == 4 && tokens[i - 2].surface() == "…")
            || (i == 6 && tokens[i - 2].surface() == "."))
}

fn is_followed_by_relative_or_subordinate_clause(
    i: usize,
    tokens: &[&AnalyzedTokenReadings],
) -> bool {
    if i < tokens.len().saturating_sub(4) {
        return tokens[i + 1].surface() == ","
            && (INTERROGATIVE_PARTICLES.contains(&tokens[i + 2].surface())
                || tokens[i + 2].has_pos_tag("KON:UNT"));
    }
    false
}

fn is_prev_probably_relative_pronoun(tokens: &[&AnalyzedTokenReadings], i: usize) -> bool {
    i >= 3
        && tokens[i - 1].surface() == "das"
        && tokens[i - 2].surface() == ","
        && tokens[i - 3].has_pos_tag_matching(&util::anchored("SUB:...:SIN:NEU"))
}

fn is_probably_city(i: usize, tokens: &[&AnalyzedTokenReadings], token: &str) -> bool {
    let has_city_prefix = matches!(token, "Klein" | "Groß" | "Neu");
    if has_city_prefix {
        let next_readings = tokens.get(i + 1).copied();
        return next_readings.is_some_and(|t| !t.is_tagged || t.has_pos_tag_starting_with("EIG"));
    }
    false
}

fn add_rule_match(
    rule_matches: &mut Vec<Match>,
    sentence_offset: usize,
    msg: &str,
    token_readings: &AnalyzedTokenReadings,
    fixed_word: &str,
) {
    rule_matches.push(
        Match::new(
            RULE_ID,
            Option::<String>::None,
            msg,
            Option::<String>::None,
            TextRange::new(
                sentence_offset + token_readings.start_pos,
                sentence_offset + token_readings.end_pos(),
            ),
            vec![Suggestion {
                value: fixed_word.to_string(),
                short_description: None,
            }],
            CATEGORY_ID,
            CATEGORY_NAME,
        )
        .with_metadata(
            DESCRIPTION,
            "uncategorized",
            CaseRule::estimate_context_for_sure_match(),
        ),
    );
}
