//! Portuguese built-in `Rule` classes that are not pattern/filter based:
//! `PortugueseWordRepeatRule`, `PortugueseWordRepeatBeginningRule` and
//! `PortugueseFillerWordsRule` (plus the shared per-variant message strings
//! of the still-to-port paragraph/long-sentence built-ins).

use std::sync::LazyLock;

use lt_core::{AnalyzedSentence, AnalyzedTokenReadings, Match, Suggestion, TextRange};

use crate::wordutil::{eq_ignore_case, is_word};

pub const WORD_REPEAT_ID: &str = "PORTUGUESE_WORD_REPEAT_RULE";
pub const WORD_REPEAT_BEGINNING_ID: &str = "PORTUGUESE_WORD_REPEAT_BEGINNING_RULE";
pub const FILLER_WORDS_ID: &str = "FILLER_WORDS_PT";

/// Per-variant `MessagesBundle_pt_{PT,BR}` strings; plain `pt`, `pt-AO` and
/// `pt-MZ` fall back to pt-PT (Java's `ResourceBundleTools.getMessageBundle`
/// resolves their missing language-level bundle through
/// `getDefaultLanguageVariant() == pt-PT`; probed 2026-09-19).
pub struct PtStrings {
    pub repetition_desc: &'static str,
    pub repetition_msg: &'static str,
    pub repetition_short: &'static str,
    pub repetition_beginning_desc: &'static str,
    pub repetition_beginning_word: &'static str,
    pub repetition_beginning_adv: &'static str,
    pub repetition_beginning_thesaurus: &'static str,
    pub category_repetitions: &'static str,
    pub category_repetitions_style: &'static str,
    pub filler_desc: &'static str,
    pub filler_msg: &'static str,
    pub category_creative_writing: &'static str,
}

const PT_PT: PtStrings = PtStrings {
    repetition_desc: "Repetição de palavras (por exemplo, 'de de')",
    repetition_msg: "Possível erro de digitação. Repetiu uma palavra.",
    repetition_short: "Repetição de palavras",
    repetition_beginning_desc: "Frases seguidas começadas com a mesma palavra",
    repetition_beginning_word: "Três frases seguidas começadas com a mesma palavra",
    repetition_beginning_adv: "Duas frases seguidas começadas com o mesmo advérbio",
    repetition_beginning_thesaurus: "Considere reescrever a frase, ou procurar sinónimos",
    category_repetitions: "Repetições",
    category_repetitions_style: "Repetitions (Style)",
    filler_desc: "Palavras de enchimento",
    filler_msg: "Esta palavra está classificada como de enchimento. Elimine-a se possível.",
    category_creative_writing: "Dicas de estilo para escrita creativa",
};

const PT_BR: PtStrings = PtStrings {
    repetition_desc: "Repetição de palavra (ex: vai vai)",
    repetition_msg: "Possível erro de escrita: você repetiu uma palavra",
    repetition_short: "Repetição de palavra",
    repetition_beginning_desc: "Frases seguidas começando com a mesma palavra",
    repetition_beginning_word: "Três frases seguidas começam com a mesma palavra.",
    repetition_beginning_adv: "Duas frases seguidas começam com o mesmo advérbio.",
    repetition_beginning_thesaurus:
        "Considere reformular a frase ou use um dicionário para encontrar um sinônimo.",
    category_repetitions: "Repetições",
    category_repetitions_style: "Repetitions (Style)",
    filler_desc: "Palavras de complementação",
    filler_msg:
        "A palavra é classificada como uma palavra de complementação. Delete-a se for possível",
    category_creative_writing: "Dicas de estilo para uma escrita criativa",
};

pub fn strings(variant: &str) -> &'static PtStrings {
    match variant {
        "pt-BR" => &PT_BR,
        _ => &PT_PT,
    }
}

// ---------------------------------------------------------------------------
// `PortugueseWordRepeatRule`
// ---------------------------------------------------------------------------

fn tautonyms_genus() -> &'static regex::Regex {
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^(?:A(?:aptos|canthogyrus|chatina|gagus|gama|lburnus|lces|lle|losa|mandava|mazilia|meiva|nableps|nguilla|nhinga|nostomus|nser|nthias|pus|rcinella|riadne|spredo|stacus|vicularia|xis)|B(?:adis|agarius|agre|alanus|anjos|arbatula|arbus|asiliscus|atasio|elobranchus|elone|elonimorphis|idyanus|ison|ombina|oops|rama|rosme|ubo|ucayana|ufo|uteo|utis)|C(?:alamus|alappa|aleta|allichthys|alotes|apoeta|apreolus|aracal|arassius|ardinalis|arduelis|aretta|asuarius|atla|atostomus|ephea|erastes|haca|halcides|handramara|hanos|haos|hinchilla|hiropotes|hitala|hromis|iconia|idaris|inclus|itellus|lelia|occothraustes|ochlearius|oeligena|olius|olumella|oncholepas|onger|onta|onvoluta|ordylus|oscoroba|ossus|otinga|oturnix|rangon|ressida|rex|ricetus|rocuta|rossoptilon|uraeus|yanicterus|ygnus|ymbium|ynoglossus)|D(?:ama|ario|entex|evario|iuca|ives|olabrifera)|E(?:nhydris|nsifera|nsis|rythrinus|xtra)|F(?:alcipennis|eroculus|icus|ragum|rancolinus|urcula)|G(?:agata|albula|allinago|allus|azella|emma|enetta|erbillus|ibberulus|iraffa|lis|lycimeris|lyphis|obio|oliathus|onorynchus|orilla|rapsus|rus|ryllotalpa|uira|ulo)|H(?:ara|arpa|austellum|emilepidotus|eterophyes|imantopus|ippocampus|ippoglossus|ippopus|istrio|istrionicus|oolock|ucho|uso|yaena|ypnale)|I(?:chthyaetus|cterus|dea|guana|ndicator|ndri)|J(?:acana|aculus|anthina)|K(?:achuga|oilofera)|L(?:actarius|agocephalus|agopus|agurus|ambis|emmus|epadogaster|erwa|euciscus|ima|imanda|imosa|iparis|ithognathus|ithophaga|oa|ota|uscinia|utjanus|utra|utraria|ynx)|M(?:acrophyllum|anacus|argaritifera|armota|artes|ascarinus|ashuna|egacephala|elanodera|eles|elo|elolontha|elongena|enidia|ephitis|ercenaria|eretrix|erluccius|eza|icrostoma|ilvus|itella|itra|itu|odiolus|odulus|ola|olossus|olva|onachus|oniliformis|ops|ustelus|yaka|yospalax|yotis)|N(?:aja|aja|angra|asua|atrix|eita|iviventer|otopterus|ycticorax)|O(?:enanthe|gasawarana|liva|phioscincus|plopomus|reotragus|riolus)|P(?:agrus|angasius|apio|auxi|erdix|eriphylla|erna|etaurista|etronia|hocoena|hoenicurus|hoxinus|hycis|ica|ipa|ipile|ipistrellus|ipra|ithecia|lanorbis|lica|oliocephalus|ollachius|ollicipes|orites|orphyrio|orphyrolaema|orpita|orzana|ristis|seudobagarius|udu|uffinus|ungitius|yrrhocorax|yrrhula)|Q(?:uadrula|uelea)|R(?:ama|anina|apa|asbora|attus|edunca|egulus|emora|etropinna|hinobatos|iparia|ita|upicapra|upicola|utilus)|S(?:accolaimus|alamandra|arda|calpellum|cincus|colytus|ephanoides|erinus|odreana|olea|phyraena|pinachia|pirorbis|pirula|prattus|quatina|taphylaea|uiriri|ula|uta|ynodus)|T(?:adorna|andanus|chagra|elescopium|emnurus|erebellum|etradactylus|etrax|herezopolis|hymallus|ibicen|inca|odus|orpedo|rachurus|rachycorystes|rachyrinchus|ricornis|roglodytes|ropheops|ubifex|yrannus)|U(?:mbraculum|ncia)|V(?:anellus|elella|elutina|icugna|illosa|imba|iviparus|olva|ulpes)|X(?:anthocephalus|anthostigma|enopirostris)|Ypiranga|Z(?:ebrus|era|ingel|ingha|oma|onia|ungaro|ygoneura)|Se)$").unwrap()
    });
    &RE
}

fn tautonyms_species() -> &'static regex::Regex {
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^(?:a(?:aptos|canthogyrus|chatina|gagus|gama|lburnus|lces|lle|losa|mandava|mazilia|meiva|nableps|nguilla|nhinga|nostomus|nser|nthias|pus|rcinella|riadne|spredo|stacus|vicularia|xis)|b(?:adis|agarius|agre|alanus|anjos|arbatula|arbus|asiliscus|atasio|elobranchus|elone|elonimorphis|idyanus|ison|ombina|oops|rama|rosme|ubo|ucayana|ufo|uteo|utis)|c(?:alamus|alappa|aleta|allichthys|alotes|apoeta|apreolus|aracal|arassius|ardinalis|arduelis|aretta|asuarius|atla|atostomus|ephea|erastes|haca|halcides|handramara|hanos|haos|hinchilla|hiropotes|hitala|hromis|iconia|idaris|inclus|itellus|lelia|occothraustes|ochlearius|oeligena|olius|olumella|oncholepas|onger|onta|onvoluta|ordylus|oscoroba|ossus|otinga|oturnix|rangon|ressida|rex|ricetus|rocuta|rossoptilon|uraeus|yanicterus|ygnus|ymbium|ynoglossus)|d(?:ama|ario|entex|evario|iuca|ives|olabrifera)|e(?:nhydris|nsifera|nsis|rythrinus|xtra)|f(?:alcipennis|eroculus|icus|ragum|rancolinus|urcula)|g(?:agata|albula|allinago|allus|azella|emma|enetta|erbillus|ibberulus|iraffa|lis|lycimeris|lyphis|obio|oliathus|onorynchus|orilla|rapsus|rus|ryllotalpa|uira|ulo)|h(?:ara|arpa|austellum|emilepidotus|eterophyes|imantopus|ippocampus|ippoglossus|ippopus|istrio|istrionicus|oolock|ucho|uso|yaena|ypnale)|i(?:chthyaetus|cterus|dea|guana|ndicator|ndri)|j(?:acana|aculus|anthina)|k(?:achuga|oilofera)|l(?:actarius|agocephalus|agopus|agurus|ambis|emmus|epadogaster|erwa|euciscus|ima|imanda|imosa|iparis|ithognathus|ithophaga|oa|ota|uscinia|utjanus|utra|utraria|ynx)|m(?:acrophyllum|anacus|argaritifera|armota|artes|ascarinus|ashuna|egacephala|elanodera|eles|elo|elolontha|elongena|enidia|ephitis|ercenaria|eretrix|erluccius|eza|icrostoma|ilvus|itella|itra|itu|odiolus|odulus|ola|olossus|olva|onachus|oniliformis|ops|ustelus|yaka|yospalax|yotis)|n(?:aja|aja|angra|asua|atrix|eita|iviventer|otopterus|ycticorax)|o(?:enanthe|gasawarana|liva|phioscincus|plopomus|reotragus|riolus)|p(?:agrus|angasius|apio|auxi|erdix|eriphylla|erna|etaurista|etronia|hocoena|hoenicurus|hoxinus|hycis|ica|ipa|ipile|ipistrellus|ipra|ithecia|lanorbis|lica|oliocephalus|ollachius|ollicipes|orites|orphyrio|orphyrolaema|orpita|orzana|ristis|seudobagarius|udu|uffinus|ungitius|yrrhocorax|yrrhula)|q(?:uadrula|uelea)|r(?:ama|anina|apa|asbora|attus|edunca|egulus|emora|etropinna|hinobatos|iparia|ita|upicapra|upicola|utilus)|s(?:accolaimus|alamandra|arda|calpellum|cincus|colytus|ephanoides|erinus|odreana|olea|phyraena|pinachia|pirorbis|pirula|prattus|quatina|taphylaea|uiriri|ula|uta|ynodus)|t(?:adorna|andanus|chagra|elescopium|emnurus|erebellum|etradactylus|etrax|herezopolis|hymallus|ibicen|inca|odus|orpedo|rachurus|rachycorystes|rachyrinchus|ricornis|roglodytes|ropheops|ubifex|yrannus)|u(?:mbraculum|ncia)|v(?:anellus|elella|elutina|icugna|illosa|imba|iviparus|olva|ulpes)|x(?:anthocephalus|anthostigma|enopirostris)|ypiranga|z(?:ebrus|era|ingel|ingha|oma|onia|ungaro|ygoneura)|se)$").unwrap()
    });
    &RE
}

fn pronouns() -> &'static regex::Regex {
    static RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^(?:mas|n?[ao]s?|se)$").unwrap());
    &RE
}

fn reduplicated_adverbs() -> &'static regex::Regex {
    static RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)^(?:já|logo|fácil)$").unwrap());
    &RE
}

/// `WordRepeatRule.wordRepetitionOf` (case-sensitive).
fn repetition_of(word: &str, tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    position > 0 && tokens[position - 1].surface() == word && tokens[position].surface() == word
}

/// `PortugueseWordRepeatRule.ignore` (including the `WordRepeatRule` base
/// exceptions).
fn portuguese_ignore(tokens: &[&AnalyzedTokenReadings], position: usize) -> bool {
    if position == 0 {
        return false;
    }
    for word in ["blá", "se", "sapiens", "tuk"] {
        if repetition_of(word, tokens, position) {
            return true;
        }
    }
    if tautonyms_genus().is_match(tokens[position - 1].surface())
        && tautonyms_species().is_match(tokens[position].surface())
    {
        return true; // e.g. Vulpes vulpes
    }
    if position >= 2
        && tokens[position - 2].surface() == "-"
        && !tokens[position - 1].whitespace_before
        && pronouns().is_match(tokens[position].surface())
    {
        return true; // e.g. "Coloquem-na na sala."
    }
    if reduplicated_adverbs().is_match(tokens[position].surface()) {
        return true; // e.g. "Logo logo"
    }
    for name in [
        "Phi", "Li", "Xiao", "Duran", "Wagga", "Abdullah", "Nwe", "Pago", "Cao",
    ] {
        if repetition_of(name, tokens, position) {
            return true;
        }
    }
    false
}

/// `PortugueseWordRepeatRule.match` over one sentence.
pub fn word_repeat_sentence(
    tokens: &[AnalyzedTokenReadings],
    sentence_offset: usize,
    variant: &str,
) -> Vec<Match> {
    let s = strings(variant);
    let view: Vec<&AnalyzedTokenReadings> = tokens
        .iter()
        .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
        .collect();
    let mut rule_matches = Vec::new();
    let mut prev_token = String::new();
    for i in 1..view.len() {
        let token = view[i].surface().to_string();
        if view[i].is_immunized {
            prev_token.clear();
            continue;
        }
        if is_word(&token) && eq_ignore_case(&prev_token, &token) && !portuguese_ignore(&view, i) {
            let prev_pos = view[i - 1].start_pos;
            let pos = view[i].start_pos;
            rule_matches.push(
                Match::new(
                    WORD_REPEAT_ID,
                    Option::<String>::None,
                    s.repetition_msg,
                    Some(s.repetition_short.to_string()),
                    TextRange::new(
                        sentence_offset + prev_pos,
                        sentence_offset + pos + prev_token.len(),
                    ),
                    vec![Suggestion {
                        value: prev_token.clone(),
                        short_description: None,
                    }],
                    "REPETITIONS",
                    s.category_repetitions,
                )
                .with_metadata(s.repetition_desc, "duplication", 1),
            );
        }
        prev_token = token;
    }
    rule_matches
}

// ---------------------------------------------------------------------------
// `PortugueseWordRepeatBeginningRule`
// ---------------------------------------------------------------------------

/// `PortugueseWordRepeatBeginningRule.ADVERBS`.
const BEGINNING_ADVERBS: [&str; 129] = [
    "Abaixo",
    "Acaso",
    "Acima",
    "Acolá",
    "Ademais",
    "Adentro",
    "Adiante",
    "Adicionalmente",
    "Afinal",
    "Afora",
    "Agora",
    "Aí",
    "Ainda",
    "Além",
    "Algures",
    "Ali",
    "Aliás",
    "Amanhã",
    "Amiúde",
    "Antigamente",
    "Aonde",
    "Apenas",
    "Apesar",
    "Aquém",
    "Aqui",
    "Assaz",
    "Assim",
    "Até",
    "Atrás",
    "Bastante",
    "Bem",
    "Bondosamente",
    "Breve",
    "Cá",
    "Casualmente",
    "Cedo",
    "Certamente",
    "Certo",
    "Constantemente",
    "Cuidadosamente",
    "Dantes",
    "Debaixo",
    "Debalde",
    "Decerto",
    "Defronte",
    "Demais",
    "Demasiado",
    "Dentro",
    "Depois",
    "Depressa",
    "Detrás",
    "Devagar",
    "Doravante",
    "E",
    "Efetivamente",
    "Embaixo",
    "Embora",
    "Enfim",
    "Então",
    "Entrementes",
    "Exclusivamente",
    "Externamente",
    "Fora",
    "Frequentemente",
    "Generosamente",
    "Hoje",
    "Imediatamente",
    "Inclusivamente",
    "Inda",
    "Já",
    "Jamais",
    "Lá",
    "Logo",
    "Longe",
    "Mais",
    "Mal",
    "Mas",
    "Melhor",
    "Menos",
    "Mesmo",
    "Muito",
    "Não",
    "Nem",
    "Nenhures",
    "Nunca",
    "Onde",
    "Ontem",
    "Ora",
    "Ou",
    "Outra",
    "Outro",
    "Outrora",
    "Outrossim",
    "Perto",
    "Pior",
    "Porventura",
    "Possivelmente",
    "Pouco",
    "Primeiramente",
    "Primeiro",
    "Principalmente",
    "Provavelmente",
    "Provisoriamente",
    "Quanto",
    "Quão",
    "Quase",
    "Quiçá",
    "Realmente",
    "Salvo",
    "Seguidamente",
    "Sempre",
    "Senão",
    "Será",
    "Sim",
    "Simplesmente",
    "Só",
    "Sobremaneira",
    "Sobremodo",
    "Sobretudo",
    "Somente",
    "Sucessivamente",
    "Talvez",
    "Também",
    "Tampouco",
    "Tanto",
    "Tão",
    "Tarde",
    "Ultimamente",
    "Unicamente",
];

fn is_exception(token: &str) -> bool {
    matches!(token, ":" | "–" | "-" | "✔️" | "➡️" | "—" | "⭐️" | "⚠️")
}

fn trimmed_ends_like_sentence(text: &str) -> bool {
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"^.+[.?!]$").unwrap());
    RE.is_match(text.trim())
}

/// `WordRepeatBeginningRule.match` with the Portuguese adverb set. Text-level
/// (`minToCheckParagraph` 2, no suggestions).
pub fn word_repeat_beginning(sentences: &[AnalyzedSentence], variant: &str) -> Vec<Match> {
    let s = strings(variant);
    let mut rule_matches = Vec::new();
    let mut last_token = String::new();
    let mut before_last_token = String::new();
    let mut prev_sentence: Option<&AnalyzedSentence> = None;
    for sentence in sentences {
        let tokens: Vec<&AnalyzedTokenReadings> = sentence
            .tokens
            .iter()
            .filter(|t| !t.is_whitespace || t.is_sentence_start || t.is_sentence_end)
            .collect();
        let mut token = String::new();
        if tokens.len() > 1 {
            token = tokens[1].surface().to_string();
            if tokens.len() > 3 {
                let is_word = token.chars().count() != 1
                    || token.chars().next().is_some_and(char::is_alphabetic);
                if is_word
                    && last_token == token
                    && !is_exception(&token)
                    && !is_exception(tokens[2].surface())
                    && !is_exception(tokens[3].surface())
                    && prev_sentence.is_some_and(|p| trimmed_ends_like_sentence(&p.text))
                {
                    let short_msg = if BEGINNING_ADVERBS.contains(&token.as_str()) {
                        Some(s.repetition_beginning_adv)
                    } else if before_last_token == token {
                        Some(s.repetition_beginning_word)
                    } else {
                        None
                    };
                    if let Some(short_msg) = short_msg {
                        let msg = format!("{short_msg} {}", s.repetition_beginning_thesaurus);
                        let start_pos = tokens[1].start_pos;
                        let end_pos = start_pos + token.len();
                        rule_matches.push(
                            Match::new(
                                WORD_REPEAT_BEGINNING_ID,
                                Option::<String>::None,
                                msg,
                                Some(short_msg.to_string()),
                                TextRange::new(
                                    sentence.offset + start_pos,
                                    sentence.offset + end_pos,
                                ),
                                Vec::new(),
                                "REPETITIONS_STYLE",
                                s.category_repetitions_style,
                            )
                            .with_metadata(
                                s.repetition_beginning_desc,
                                "style",
                                0,
                            ),
                        );
                    }
                }
            }
        }
        before_last_token = last_token;
        last_token = token;
        prev_sentence = Some(sentence);
    }
    rule_matches
}

// ---------------------------------------------------------------------------
// `PortugueseFillerWordsRule` (via `AbstractStatisticStyleRule`)
// ---------------------------------------------------------------------------

/// `PortugueseFillerWordsRule.fillerWords`.
const FILLER_WORDS: [&str; 172] = [
    "abundante",
    "acrescentou",
    "acrescidamente",
    "adição",
    "agora",
    "ainda",
    "além",
    "algo",
    "algum",
    "alguma",
    "algumas",
    "alguns",
    "aparecer",
    "aparentemente",
    "apenas",
    "apesar",
    "aproximadamente",
    "assim",
    "atrás",
    "atualmente",
    "automaticamente",
    "bem",
    "bonito",
    "certamente",
    "certo",
    "claramente",
    "claro",
    "completam",
    "completamente",
    "completo",
    "comumente",
    "consequentemente",
    "consistentemente",
    "continuamente",
    "contra",
    "contraste",
    "contudo",
    "cuidado",
    "curto",
    "dependendo",
    "depois",
    "desigual",
    "determinado",
    "deve",
    "dever",
    "difícil",
    "direito",
    "dúvida",
    "embora",
    "enquanto",
    "entanto",
    "ergo",
    "especial",
    "estranhamente",
    "eventualmente",
    "evidentemente",
    "expressar",
    "extremamente",
    "fácil",
    "famoso",
    "feio",
    "felizmente",
    "francamente",
    "frequência",
    "frequentemente",
    "geralmente",
    "graças",
    "impressionante",
    "impronunciável",
    "incomum",
    "indizível",
    "infelizmente",
    "irrelevante",
    "irrelevantes",
    "já",
    "justo",
    "lento",
    "longo",
    "lugares",
    "maior",
    "mais",
    "mas",
    "melhor",
    "mesmo",
    "muita",
    "muitas",
    "muito",
    "muitos",
    "múltipla",
    "nada",
    "não",
    "natural",
    "naturalmente",
    "natureza",
    "nehumas",
    "nenhum",
    "nenhuma",
    "nenhuns",
    "nomeadamente",
    "normalmente",
    "novo",
    "número",
    "nunca",
    "óbvio",
    "ocasionalmente",
    "outra",
    "outros",
    "para",
    "parente",
    "particularmente",
    "pessoa",
    "pode",
    "poderia",
    "pois",
    "porém",
    "porque",
    "portanto",
    "possível",
    "possivelmente",
    "pouca",
    "poucas",
    "pouco",
    "poucos",
    "prático",
    "precisas",
    "principalmente",
    "provável",
    "provavelmente",
    "quaisquer",
    "qualquer",
    "quase",
    "rápido",
    "raramente",
    "razoavelmente",
    "realmente",
    "recentemente",
    "relativamente",
    "repente",
    "sempre",
    "senão",
    "sentida",
    "sentidas",
    "sentido",
    "sentidos",
    "siga",
    "significativo",
    "sim",
    "simples",
    "simplesmente",
    "sobre",
    "sozinho",
    "suave",
    "suavemente",
    "substancialmente",
    "suficientemente",
    "tipo",
    "tornar",
    "tornaram",
    "tornou",
    "total",
    "totalmente",
    "toda",
    "todas",
    "todo",
    "todos",
    "tudo",
    "ultrajante",
    "velho",
    "verdade",
    "vez",
    "vezes",
    "volta",
];

fn filler_word_set() -> &'static std::collections::HashSet<&'static str> {
    static SET: LazyLock<std::collections::HashSet<&'static str>> =
        LazyLock::new(|| FILLER_WORDS.iter().copied().collect());
    &SET
}

/// `PortugueseFillerWordsRule.isException`.
fn filler_is_exception(tokens: &[&AnalyzedTokenReadings], num: usize) -> bool {
    if num >= 1 && tokens[num - 1].surface() == "," && tokens[num].surface() == "mas" {
        return true;
    }
    false
}

fn is_opening_quote(token: &str) -> bool {
    let mut chars = token.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if matches!(c, '"' | '“' | '„' | '»' | '«'))
}

fn is_ending_quote(token: &str) -> bool {
    let mut chars = token.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if matches!(c, '"' | '“' | '”' | '»' | '«'))
}

/// `FILLER_WORDS_PT` (`AbstractFillerWordsRule`, limit 8 %, direct speech
/// excluded). Text-level.
pub fn filler_words(sentences: &[AnalyzedSentence], variant: &str) -> Vec<Match> {
    let s = strings(variant);
    const MIN_PERCENT: u32 = 8;
    let mut hints: Vec<(usize, usize, usize)> = Vec::new();
    let mut word_count = 0u32;
    let mut is_direct_speech = false;
    for (sentence_index, sentence) in sentences.iter().enumerate() {
        let tokens = sentence.tokens_without_whitespace();
        for n in 1..tokens.len() {
            let token = tokens[n];
            let surface = token.surface();
            if !is_direct_speech
                && is_opening_quote(surface)
                && n < tokens.len() - 1
                && !tokens[n + 1].whitespace_before
            {
                is_direct_speech = true;
            } else if is_direct_speech
                && is_ending_quote(surface)
                && n > 1
                && !token.whitespace_before
            {
                is_direct_speech = false;
            } else if !is_direct_speech
                && !token.is_whitespace
                && !crate::style_too_often::is_non_word(surface)
            {
                word_count += 1;
                if filler_word_set().contains(surface) && !filler_is_exception(&tokens, n) {
                    hints.push((sentence_index, token.start_pos, tokens[n].end_pos()));
                }
            }
        }
    }
    let num_matches = hints.len() as f64;
    let percent = if word_count > 0 {
        num_matches * 100.0 / word_count as f64
    } else {
        0.0
    };
    if percent <= MIN_PERCENT as f64 {
        return Vec::new();
    }
    hints
        .into_iter()
        .map(|(sentence_index, start, end)| {
            let sentence = &sentences[sentence_index];
            Match::new(
                FILLER_WORDS_ID,
                Option::<String>::None,
                s.filler_msg,
                Option::<String>::None,
                TextRange::new(sentence.offset + start, sentence.offset + end),
                Vec::new(),
                "CREATIVE_WRITING",
                s.category_creative_writing,
            )
            .with_metadata(s.filler_desc, "style", 0)
        })
        .collect()
}
