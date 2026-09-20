//! `org.languagetool.rules.ca.DonarseliBeFilter` (+ `VerbsHelper`): the
//! `donar-se-li bé/malament` → `tenir traça` / `fer bé` / `sortir-se'n` /
//! `anar bé` / `eixir/sortir bé` suggestions (D-152).

use lt_core::{AnalyzedTokenReadings, TextRange};
use lt_pattern::{FilterContext, FilterOutcome, RuleFilter};

use crate::ca::adapt::preserve_case;
use crate::ca::filters::Env;
use crate::ca::helpers::{
    reading_with_tag_regex, transform, transform_darrere, transform_davant, PronounPosition,
    VerbSynthesizer,
};
use crate::ca::verb_filters::tokens_without_whitespace;

/// `DonarseliBeFilter.pPronomFeble`.
const P_PRONOM_FEBLE: &str = "P0.{6}|PP3CN000|PP3NN000|PP3..A00|PP[123]CP000|PP3CSD00";

/// `DonarseliBeFilter.adverbiFinal`.
const ADVERBI_FINAL: [&str; 6] = ["bé", "malament", "mal", "millor", "pitjor", "fatal"];

/// `DonarseliBeFilter.pronomsPersonals`.
const PRONOMS_PERSONALS: [&str; 8] = [
    "mi",
    "tu",
    "ell",
    "ella",
    "nosaltres",
    "vosaltres",
    "ells",
    "elles",
];

/// `DonarseliBeFilter.exceptionsQue`.
const EXCEPTIONS_QUE: [&str; 6] = ["ja", "ara", "per", "de", "a", "en"];

/// `VerbsHelper.lVerbsDicendi`.
const VERBS_DICENDI: [&str; 168] = [
    "amollar",
    "afluixar",
    "entaferrar",
    "espletar",
    "soltar",
    "engaltar",
    "acceptar",
    "aclarir",
    "aconsellar",
    "acusar",
    "adduir",
    "admetre",
    "advertir",
    "afegir",
    "afirmar",
    "agregar",
    "al·legar",
    "al·ludir",
    "amenaçar",
    "amonestar",
    "ampliar",
    "anunciar",
    "apuntar",
    "argumentar",
    "assegurar",
    "assentir",
    "assenyalar",
    "atorgar",
    "atribuir",
    "avançar",
    "avisar",
    "barbotejar",
    "bordar",
    "bramar",
    "calcular",
    "callar",
    "citar",
    "comentar",
    "concedir",
    "concloure",
    "concretar",
    "confessar",
    "confiar",
    "confirmar",
    "considerar",
    "contestar",
    "creure",
    "cridar",
    "culpar",
    "decidir",
    "declamar",
    "declarar",
    "decretar",
    "defensar",
    "definir",
    "delimitar",
    "demanar",
    "descobrir",
    "descriure",
    "desitjar",
    "desmentir",
    "destacar",
    "desvelar",
    "detallar",
    "determinar",
    "dir",
    "dogmatitzar",
    "dubtar",
    "elogiar",
    "emfasitzar",
    "emfatitzar",
    "engegar",
    "enumerar",
    "esclafir",
    "escopir",
    "escridassar",
    "esgrimir",
    "esmentar",
    "especificar",
    "establir",
    "etzibar",
    "exclamar",
    "exigir",
    "explicar",
    "exposar",
    "expressar",
    "formular",
    "garantir",
    "gemegar",
    "imaginar",
    "implorar",
    "imputar",
    "increpar",
    "indicar",
    "informar",
    "inquirir",
    "insinuar",
    "insistir",
    "insultar",
    "interrogar",
    "intervenir",
    "ironitzar",
    "jurar",
    "justificar",
    "lamentar",
    "lladrar",
    "lloar",
    "maleir",
    "manar",
    "manifestar",
    "matisar",
    "mentir",
    "mostrar",
    "murmurar",
    "negar",
    "observar",
    "oferir",
    "opinar",
    "ordenar",
    "pensar",
    "plantejar",
    "pontificar",
    "pregar",
    "preguntar",
    "presumir",
    "preveure",
    "prometre",
    "proposar",
    "protestar",
    "puntualitzar",
    "quequejar",
    "ratificar",
    "reafirmar",
    "rebutjar",
    "recalcar",
    "recitar",
    "reclamar",
    "recomanar",
    "reconèixer",
    "referir",
    "refermar",
    "reflexionar",
    "refusar",
    "refutar",
    "relatar",
    "remarcar",
    "rematar",
    "remugar",
    "renegar",
    "renyar",
    "repetir",
    "replicar",
    "reprendre",
    "resar",
    "respondre",
    "retreure",
    "revelar",
    "sol·licitar",
    "somicar",
    "sospirar",
    "sospitar",
    "sostenir",
    "subratllar",
    "suggerir",
    "suposar",
    "xisclar",
    "xiuxiuejar",
    "trobar",
];

/// `VerbsHelper.isVerbDicendiBefore`.
fn is_verb_dicendi_before(tokens: &[&AnalyzedTokenReadings], mut i: isize) -> bool {
    while i > 0 && (i as usize) < tokens.len() {
        let Some(reading) = reading_with_tag_regex(tokens[i as usize], "V.*|RG.*|LOC_ADV") else {
            return false;
        };
        if VERBS_DICENDI.contains(&reading.lemma().to_lowercase().as_str()) {
            return true;
        }
        i -= 1;
    }
    false
}

fn replace_first_ci(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let re = regex::Regex::new(&format!("(?i){}", regex::escape(needle))).unwrap();
    re.replace(haystack, replacement).into_owned()
}

fn replace_first_literal(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let re = regex::Regex::new(&regex::escape(needle)).unwrap();
    re.replace(haystack, replacement).into_owned()
}

fn surface_ci(tokens: &[&AnalyzedTokenReadings], i: isize, s: &str) -> bool {
    i > 0 && (i as usize) < tokens.len() && tokens[i as usize].surface().eq_ignore_ascii_case(s)
}

/// `DonarseliBeFilter.getAdverbsFor`.
fn get_adverbs_for(
    tokens: &[&AnalyzedTokenReadings],
    primer_adverbi: usize,
    darrer_adverbi: usize,
    target: &str,
) -> String {
    let mut result = String::new();
    for token in &tokens[primer_adverbi..darrer_adverbi] {
        if token.whitespace_before {
            result.push(' ');
        }
        result.push_str(token.surface());
    }
    if target == "traça" {
        if result.eq_ignore_ascii_case(" molt") {
            result = " molta".to_string();
        } else if result.eq_ignore_ascii_case(" gens") {
            result = " gens de".to_string();
        } else if result.eq_ignore_ascii_case(" tan") {
            result = " tanta".to_string();
        }
    }
    result
}

/// `org.languagetool.rules.ca.DonarseliBeFilter`.
pub struct DonarseliBeFilter {
    pub(crate) env: Env,
}

impl RuleFilter for DonarseliBeFilter {
    #[allow(clippy::too_many_lines)]
    fn accept(&self, ctx: &FilterContext) -> FilterOutcome {
        let tokens = tokens_without_whitespace(ctx);
        let pos_word0 = crate::ca::verb_filters::pos_word_start(&tokens, ctx.match_range.start);
        let mut verb_synth =
            VerbSynthesizer::new(&tokens, pos_word0, self.env.synth.inner(), false);
        if verb_synth.is_undefined()
            || tokens[verb_synth.get_last_verb_index() as usize].end_pos() > ctx.match_range.end
        {
            return FilterOutcome::reject();
        }
        let pos_donar = verb_synth.get_last_verb_index();
        let pos_primer_verb = verb_synth.get_first_verb_index();
        let pos_init_underline = pos_primer_verb - verb_synth.get_num_pronouns_before();
        let is_pronom_feble_davant = verb_synth.get_num_pronouns_before() > 0;
        let mut pos_pronom_feble_relevant: isize = -1;
        if verb_synth.get_num_pronouns_after() == 2 {
            pos_pronom_feble_relevant = pos_donar + 2;
        } else if verb_synth.get_num_pronouns_before() >= 2 {
            // Si n'hi ha tres, suposem que és un "hi" que ignorem
            pos_pronom_feble_relevant =
                pos_primer_verb - (verb_synth.get_num_pronouns_before() - 1);
        }
        if pos_pronom_feble_relevant < 1 {
            return FilterOutcome::reject();
        }
        let Some(pronom_feble_relevant) =
            reading_with_tag_regex(tokens[pos_pronom_feble_relevant as usize], P_PRONOM_FEBLE)
                .cloned()
        else {
            return FilterOutcome::reject();
        };

        // mira darrere: molt bé
        let mut pos_word =
            (verb_synth.get_last_verb_index() + verb_synth.get_num_pronouns_after() + 1) as usize;
        let primer_adverbi = pos_word;
        while pos_word < tokens.len()
            && !ADVERBI_FINAL.contains(&tokens[pos_word].surface().to_lowercase().as_str())
        {
            pos_word += 1;
        }
        if pos_word >= tokens.len() {
            return FilterOutcome::reject();
        }
        let darrer_adverbi = pos_word;
        let mut darrer_adverbi_str = tokens[darrer_adverbi].surface().to_string();
        if darrer_adverbi_str.eq_ignore_ascii_case("mal")
            || darrer_adverbi_str.eq_ignore_ascii_case("fatal")
        {
            darrer_adverbi_str = "malament".to_string();
        }
        let despres_darrer_adverbi = if darrer_adverbi + 1 < tokens.len() {
            reading_with_tag_regex(tokens[darrer_adverbi + 1], "V.N.*|D.*|PD.*").cloned()
        } else {
            None
        };
        let mut add_tokens_to_right = 0isize;
        let mut add_string_to_right = String::new();
        let mut add_tokens_to_left = 0isize;
        let mut add_string_to_left = String::new();

        // analitza paraules prèvies: que a mi mai no se'm dona malament
        let is_no = pos_init_underline - 1 > 0
            && (surface_ci(&tokens, pos_init_underline - 1, "no")
                || surface_ci(&tokens, pos_init_underline - 1, "mai"));
        let is_mai_no = is_no
            && pos_init_underline - 2 > 0
            && surface_ci(&tokens, pos_init_underline - 2, "mai");
        let mut is_malament = darrer_adverbi_str.eq_ignore_ascii_case("malament")
            || darrer_adverbi_str.eq_ignore_ascii_case("pitjor");
        // No ... malament
        let is_no_malament = is_no && is_malament;
        // ... malement
        is_malament = is_malament && !is_no_malament;
        if is_mai_no {
            add_tokens_to_left += 1;
        }
        if is_no {
            add_tokens_to_left += 1;
        }
        let mut a_mi_string = String::new();
        if pos_init_underline - add_tokens_to_left - 2 > 0
            && surface_ci(&tokens, pos_init_underline - add_tokens_to_left - 2, "a")
            && PRONOMS_PERSONALS.contains(
                &tokens[(pos_init_underline - add_tokens_to_left - 1) as usize]
                    .surface()
                    .to_lowercase()
                    .as_str(),
            )
        {
            a_mi_string = format!(
                "{} {} ",
                tokens[(pos_init_underline - add_tokens_to_left - 2) as usize].surface(),
                tokens[(pos_init_underline - add_tokens_to_left - 1) as usize].surface()
            );
            add_tokens_to_left += 2;
        }
        let mut is_que = pos_init_underline - add_tokens_to_left - 1 > 0
            && surface_ci(&tokens, pos_init_underline - add_tokens_to_left - 1, "que")
            && !is_verb_dicendi_before(&tokens, pos_init_underline - add_tokens_to_left - 2);
        let mut is_que_accent = pos_init_underline - add_tokens_to_left - 1 > 0
            && surface_ci(&tokens, pos_init_underline - add_tokens_to_left - 1, "què");
        if pos_init_underline - add_tokens_to_left - 2 > 0
            && EXCEPTIONS_QUE.contains(
                &tokens[(pos_init_underline - add_tokens_to_left - 2) as usize]
                    .surface()
                    .to_lowercase()
                    .as_str(),
            )
        {
            is_que_accent = false;
            is_que = false;
        }
        let mut is_el_que = false;
        let mut is_a_qui = false;
        if pos_init_underline - add_tokens_to_left - 1 > 0
            && surface_ci(&tokens, pos_init_underline - add_tokens_to_left - 1, "qui")
        {
            is_el_que = true;
            if pos_init_underline - add_tokens_to_left - 2 > 0
                && surface_ci(&tokens, pos_init_underline - add_tokens_to_left - 2, "a")
            {
                is_a_qui = true;
            }
        }
        if is_que
            && pos_init_underline - add_tokens_to_left - 2 > 0
            && (tokens[(pos_init_underline - add_tokens_to_left - 2) as usize]
                .has_pos_tag_starting_with("DA")
                || ["alumne", "persona", "estudiant", "professor"]
                    .iter()
                    .any(|l| {
                        tokens[(pos_init_underline - add_tokens_to_left - 2) as usize].has_lemma(l)
                    }))
        {
            is_el_que = true;
            is_que = false; // no subratllem "que"
        }
        if is_que {
            add_tokens_to_left += 1;
        }
        if is_que_accent {
            add_tokens_to_left += 1;
        }
        for j in (pos_init_underline - add_tokens_to_left)..pos_init_underline {
            add_string_to_left.push_str(tokens[j as usize].surface());
            add_string_to_left.push(' ');
        }

        // Crea suggeriments
        let persona = pronom_feble_relevant
            .pos_tag
            .as_deref()
            .and_then(|t| t.get(2..3))
            .unwrap_or("")
            .to_string();
        let nombre = pronom_feble_relevant
            .pos_tag
            .as_deref()
            .and_then(|t| t.get(4..5))
            .unwrap_or("")
            .to_string();
        let Some(primer_verb) =
            reading_with_tag_regex(tokens[pos_primer_verb as usize], "V.*").cloned()
        else {
            return FilterOutcome::reject();
        };
        let verb_postag = primer_verb.pos_tag.clone().unwrap_or_default();
        let new_verb_postag = format!(
            "{}{}{}{}",
            verb_postag.get(0..4).unwrap_or(""),
            persona,
            nombre,
            verb_postag.get(6..8).unwrap_or("")
        );
        let mut replacements: Vec<String> = Vec::new();
        let casing_model = tokens[(pos_init_underline - add_tokens_to_left) as usize].surface();

        // tinc traça (per a)
        let mut add_string_to_left_tinc_traca =
            replace_first_ci(&add_string_to_left, "què ", "en què ");
        add_string_to_left_tinc_traca =
            replace_first_ci(&add_string_to_left_tinc_traca, "que ", "en què ");
        add_string_to_left_tinc_traca =
            replace_first_literal(&add_string_to_left_tinc_traca, &a_mi_string, "");
        if is_no_malament {
            add_string_to_left_tinc_traca =
                replace_first_ci(&add_string_to_left_tinc_traca, "no ", "");
            add_string_to_left_tinc_traca =
                replace_first_ci(&add_string_to_left_tinc_traca, "mai ", "");
        }
        let mut suggestion = add_string_to_left_tinc_traca.clone();
        if is_malament {
            suggestion.push_str("no ");
        }
        if !add_string_to_left.to_lowercase().starts_with("qu") && despres_darrer_adverbi.is_none()
        {
            suggestion.push_str("hi ");
        }
        verb_synth.set_lemma_and_postag("tenir", &new_verb_postag);
        suggestion.push_str(&verb_synth.synthesize());
        if !is_no_malament && !is_malament {
            suggestion.push_str(&get_adverbs_for(
                &tokens,
                primer_adverbi,
                darrer_adverbi,
                "traça",
            ));
        }
        suggestion.push_str(" traça");
        if let Some(despres) = &despres_darrer_adverbi {
            if despres.token.eq_ignore_ascii_case("el") {
                suggestion.push_str(" per al");
                add_tokens_to_right = 1;
                add_string_to_right = " el".to_string();
            } else if despres.token.eq_ignore_ascii_case("els") {
                suggestion.push_str(" per als");
                add_tokens_to_right = 1;
                add_string_to_right = " els".to_string();
            } else {
                suggestion.push_str(" per a");
            }
        }
        if !is_el_que {
            replacements.push(preserve_case(&suggestion, casing_model));
        }

        // faig bé
        let mut suggestion = replace_first_literal(&add_string_to_left, &a_mi_string, "");
        if !add_string_to_left.to_lowercase().starts_with("qu")
            && despres_darrer_adverbi.is_none()
            && !is_el_que
        {
            suggestion.push_str("ho ");
        }
        verb_synth.set_lemma_and_postag("fer", &new_verb_postag);
        suggestion.push_str(&verb_synth.synthesize());
        suggestion.push_str(&get_adverbs_for(
            &tokens,
            primer_adverbi,
            darrer_adverbi,
            "bé",
        ));
        suggestion.push(' ');
        suggestion.push_str(&darrer_adverbi_str);
        suggestion.push_str(&add_string_to_right);
        if !is_a_qui {
            replacements.push(preserve_case(&suggestion, casing_model));
        }

        // me'n surto (en)
        let mut suggestion = add_string_to_left_tinc_traca.clone();
        if is_malament {
            suggestion.push_str("no ");
        }
        if is_pronom_feble_davant {
            let mut pronom = pronom_feble_relevant.token.clone();
            if pronom.eq_ignore_ascii_case("'ls") || pronom.eq_ignore_ascii_case("li") {
                pronom = "es".to_string();
            }
            let pronoms_normalitzats =
                format!("{} en", transform(&pronom, PronounPosition::Normalized));
            suggestion.push_str(&transform_davant(&pronoms_normalitzats, &primer_verb.token));
        }
        verb_synth.set_lemma_and_postag("sortir", &new_verb_postag);
        suggestion.push_str(&verb_synth.synthesize());
        if !is_pronom_feble_davant {
            let mut pronom = pronom_feble_relevant.token.clone();
            if pronom.eq_ignore_ascii_case("'ls") || pronom.eq_ignore_ascii_case("-li") {
                pronom = "es".to_string();
            }
            let pronoms_normalitzats =
                format!("{} en", transform(&pronom, PronounPosition::Normalized));
            suggestion.push_str(&transform_darrere(
                &pronoms_normalitzats,
                &primer_verb.token,
            ));
        }
        if let Some(despres) = &despres_darrer_adverbi {
            if despres.pos_tag.as_deref().unwrap_or("").starts_with('V') {
                suggestion.push_str(" a");
            } else {
                suggestion.push_str(" en");
            }
        }
        suggestion.push_str(&add_string_to_right);
        if !is_el_que {
            replacements.push(preserve_case(&suggestion, casing_model));
        }

        // em van bé
        let mut suggestion = add_string_to_left.clone();
        verb_synth.set_lemma_and_postag("anar", &verb_postag);
        let verb = verb_synth.synthesize();
        if is_pronom_feble_davant {
            suggestion.push_str(&transform_davant(&pronom_feble_relevant.token, &verb));
        }
        suggestion.push_str(&verb);
        if !is_pronom_feble_davant {
            suggestion.push_str(&transform_darrere(&pronom_feble_relevant.token, &verb));
        }
        suggestion.push_str(&get_adverbs_for(
            &tokens,
            primer_adverbi,
            darrer_adverbi,
            "bé",
        ));
        suggestion.push(' ');
        suggestion.push_str(&darrer_adverbi_str);
        suggestion.push_str(&add_string_to_right);
        replacements.push(preserve_case(&suggestion, casing_model));

        // m'ixen bé, em surten bé
        let mut suggestion = add_string_to_left.clone();
        let new_lemma_sortir = if self.env.variant == "ca-ES-valencia" {
            "eixir"
        } else {
            "sortir"
        };
        verb_synth.set_lemma_and_postag(new_lemma_sortir, &verb_postag);
        let verb = verb_synth.synthesize();
        if is_pronom_feble_davant {
            suggestion.push_str(&transform_davant(&pronom_feble_relevant.token, &verb));
        }
        suggestion.push_str(&verb);
        if !is_pronom_feble_davant {
            suggestion.push_str(&transform_darrere(&pronom_feble_relevant.token, &verb));
        }
        suggestion.push_str(&get_adverbs_for(
            &tokens,
            primer_adverbi,
            darrer_adverbi,
            "bé",
        ));
        suggestion.push(' ');
        suggestion.push_str(&darrer_adverbi_str);
        suggestion.push_str(&add_string_to_right);
        replacements.push(preserve_case(&suggestion, casing_model));

        if replacements.is_empty() {
            return FilterOutcome::reject();
        }
        FilterOutcome {
            accepted: true,
            range: Some(TextRange::new(
                tokens[(pos_init_underline - add_tokens_to_left) as usize].start_pos,
                tokens[(darrer_adverbi as isize + add_tokens_to_right) as usize].end_pos(),
            )),
            message: None,
            suggestions: Some(
                replacements
                    .into_iter()
                    .map(|value| lt_core::Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect(),
            ),
        }
    }
}
