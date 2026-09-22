//! Port of `org.languagetool.rules.uk.TokenAgreementNumrNounRule`
//! (`UK_NUMR_NOUN_INFLECTION_AGREEMENT`) and
//! `TokenAgreementNumrNounExceptionHelper`.

use std::collections::HashSet;
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::{AnalyzedToken, AnalyzedTokenReadings, Match, Suggestion, TextRange};
use lt_tagger::uk_helpers;
use lt_tagger::UkrainianSynthesizer;

use crate::uk::inflection::{self, Inflection};

pub const RULE_ID: &str = "UK_NUMR_NOUN_INFLECTION_AGREEMENT";
const DESCRIPTION: &str = "Узгодження відмінків, роду і числа числівника та іменника";
const SHORT: &str = "Узгодження числівника та іменника";
const CATEGORY_ID: &str = "MISC";
const CATEGORY_NAME: &str = "Різне";
const USED_U_INSTEAD_OF_A_MSG: &str = ". Можливо, вжито невнормований родовий відмінок ч.р. з закінченням -у/-ю замість -а/-я (така тенденція є в сучасній мові)?";

const H: &str = " \\t\\u{00A0}\\u{1680}\\u{180E}\\u{2000}-\\u{200A}\\u{202F}\\u{205F}\\u{3000}";

fn jre(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|e| panic!("numr pattern {pattern}: {e}"))
}

static NOUN_IGNORE: LazyLock<Regex> = LazyLock::new(|| jre(r"^.*(prop|noun.*pron|v_oru).*$"));
static NUMR_PATTERN: LazyLock<Regex> = LazyLock::new(|| jre(r"^numr(?!.*abbr).*$"));
static NOUN_NUMR_ALL: LazyLock<Regex> = LazyLock::new(|| {
    jre(r"^(?:noun:inanim:([mf]:v_naz|p:v_(naz|rod)).*:numr.*|numr.*abbr.*|number)$")
});
static DVA_3_4: LazyLock<Regex> =
    LazyLock::new(|| jre(r"^(?:оби(два|дві)|(.+-)?((два|дві)|три|чотири))$"));
static DVA: LazyLock<Regex> = LazyLock::new(|| jre(r"(?i)^(?:(оби)?два|.+-два)$"));
static DVI: LazyLock<Regex> = LazyLock::new(|| jre(r"(?i)^(?:(оби)?дві|.+-дві)$"));
static ONE_5: LazyLock<Regex> = LazyLock::new(|| jre(r"^([0-9]+[–-])?1,5$"));
static TWO_5: LazyLock<Regex> = LazyLock::new(|| jre(r"^.*(?<!1)[234],5$"));
static FIVE_5: LazyLock<Regex> = LazyLock::new(|| {
    jre(&format!(
        r"^([0-9]+[–-])?([0-9{H}]*[05-9]|[0-9{H}]*1[1-4]),5$"
    ))
});
static FRACT: LazyLock<Regex> = LazyLock::new(|| jre(r"^.*,[1-9]+$"));
static TWO_TO_FOUR: LazyLock<Regex> = LazyLock::new(|| jre(r"^([0-9]+[–-])?[^,]*(?<!1)[234]$"));
static FIVE_TO_NINE: LazyLock<Regex> =
    LazyLock::new(|| jre(&format!(r"^[0-9{H}]*([5-90]|1[2-4])$")));
static FIVE_TO_NINE_ALPHA: LazyLock<Regex> = LazyLock::new(|| {
    jre(
        r"^(.+-)?(п.ять|шість|сім|вісім|(три)?дев.?ять|.*дцять|сорок|.*десять?|дев.яносто|сто|двісті|триста|чотириста|півтораста|.+сот)|(де)?кілька|кількох|аніскільки$",
    )
});
static NOUN_FORCE: LazyLock<Regex> = LazyLock::new(|| {
    jre(
        r"^(?:чоловік|солдат|тон|(нано|мікро|мілі|дека|кіло|мега|гіга|тера|пета)?(герц|байт|біт|бар|бер|ват|вольт|децибел|рентген|моль|мікрон|грам|аршин|лат|карат))$",
    )
});
static ADJ_P_ROD_NAZ: LazyLock<Regex> = LazyLock::new(|| jre(r"^adj:p:v_(rod|naz).*$"));
static ADJ_P_ROD: LazyLock<Regex> = LazyLock::new(|| jre(r"^adj:p:v_rod.*$"));
static M_V_ROD_TOKEN: LazyLock<Regex> = LazyLock::new(|| jre(r"^.*:m:v_rod.*$"));
static TOKEN_AA_YA: LazyLock<Regex> = LazyLock::new(|| jre(r"^.*[ая]$"));
static NOUN_M_V_DAV: LazyLock<Regex> = LazyLock::new(|| jre(r"^noun.*?:m:v_dav.*$"));
static NUMR_ADJ_V_MIS: LazyLock<Regex> = LazyLock::new(|| jre(r"^adj.*?v_mis.*$"));
static NOUN_V_MIS: LazyLock<Regex> = LazyLock::new(|| jre(r"^noun.*?v_mis.*$"));

#[derive(Default)]
struct State {
    number: bool,
    numr_pos: usize,
    noun_pos: usize,
    numr_readings: Vec<AnalyzedToken>,
    numr_idx: Option<usize>,
}

impl State {
    fn reset(&mut self) {
        self.number = false;
        self.numr_readings.clear();
        self.numr_idx = None;
    }
    fn is_empty(&self) -> bool {
        self.numr_readings.is_empty()
    }
}

fn has_tag_re(tr: &AnalyzedTokenReadings, re: &Regex) -> bool {
    tr.readings.iter().any(|r| {
        r.pos_tag
            .as_deref()
            .is_some_and(|t| re.is_match(t).unwrap_or(false))
    })
}

fn has_tag_readings(readings: &[AnalyzedToken], re: &Regex) -> bool {
    readings.iter().any(|r| {
        r.pos_tag
            .as_deref()
            .is_some_and(|t| re.is_match(t).unwrap_or(false))
    })
}

fn has_tag_start(tr: &AnalyzedTokenReadings, prefix: &str) -> bool {
    tr.readings
        .iter()
        .any(|r| r.pos_tag.as_deref().is_some_and(|t| t.starts_with(prefix)))
}

fn has_lemma(tr: &AnalyzedTokenReadings, lemmas: &[&str]) -> bool {
    uk_helpers::has_lemma(&tr.readings, lemmas)
}

fn has_lemma_re(tr: &AnalyzedTokenReadings, re: &Regex) -> bool {
    uk_helpers::has_lemma_regex(&tr.readings, re)
}

fn has_lemma_with(tr: &AnalyzedTokenReadings, lemmas: &[&str], pos: &Regex) -> bool {
    uk_helpers::has_lemma_with_pattern(&tr.readings, lemmas, pos)
}

pub struct TokenAgreementNumrNounRule {
    synth: std::sync::Arc<UkrainianSynthesizer>,
}

impl TokenAgreementNumrNounRule {
    pub fn new(synth: std::sync::Arc<UkrainianSynthesizer>) -> Self {
        Self { synth }
    }

    pub fn rule_id(&self) -> &str {
        RULE_ID
    }

    pub fn check_sentence(
        &self,
        tokens: &[AnalyzedTokenReadings],
        sentence_offset: usize,
    ) -> Vec<Match> {
        let view: Vec<&AnalyzedTokenReadings> =
            tokens.iter().filter(|t| !t.is_whitespace).collect();
        let n = view.len();
        let mut out = Vec::new();
        let mut state = State::default();
        let mut i = 1usize;
        while i < n {
            let tr = view[i];
            let pos0 = tr.readings.first().and_then(|r| r.pos_tag.clone());
            let clean = tr.surface().to_string();
            if pos0.is_none() || clean.is_empty() {
                state.reset();
                i += 1;
                continue;
            }
            if i == n - 1 && state.is_empty() {
                i += 1;
                continue;
            }
            let clean_lower = clean.to_lowercase();

            if has_tag_re(tr, &NOUN_NUMR_ALL) {
                if i < n - 1
                    && NOUN_FORCE
                        .is_match(&view[i + 1].surface().to_lowercase())
                        .unwrap_or(false)
                {
                    state.reset();
                    state.numr_pos = i;
                    state
                        .numr_readings
                        .push(tr.readings.first().cloned().unwrap());
                    state.numr_idx = Some(i);
                    state.number = has_tag_start(tr, "number");
                    i += 1;
                    continue;
                }
                if i < n - 2
                    && has_tag_re(view[i + 1], &ADJ_P_ROD)
                    && NOUN_FORCE
                        .is_match(&view[i + 2].surface().to_lowercase())
                        .unwrap_or(false)
                {
                    state.reset();
                    state.numr_pos = i;
                    state
                        .numr_readings
                        .push(tr.readings.first().cloned().unwrap());
                    state.numr_idx = Some(i);
                    state.number = has_tag_start(tr, "number");
                    i += 2;
                    continue;
                }
            }

            if has_tag_re(tr, &NUMR_PATTERN) {
                state.reset();
                if Regex::new(r"^.*[0-9]-[а-яіїєґ].*$")
                    .unwrap()
                    .is_match(&clean)
                    .unwrap_or(false)
                {
                    i += 1;
                    continue;
                }
                if has_lemma_with(tr, &["мати"], &Regex::new(r"^verb.*$").unwrap()) {
                    i += 1;
                    continue;
                }
                if has_lemma(tr, &["один"]) {
                    i += 1;
                    continue;
                }
                for at in &tr.readings {
                    if let Some(tag) = at.pos_tag.as_deref() {
                        if tag.starts_with("numr") || NOUN_NUMR_ALL.is_match(tag).unwrap_or(false) {
                            state.numr_pos = i;
                            state.numr_readings.push(at.clone());
                            state.numr_idx = Some(i);
                        }
                    }
                }
                i += 1;
                continue;
            } else if has_tag_start(tr, "number") {
                state.numr_pos = i;
                state.numr_readings.extend(tr.readings.iter().cloned());
                state.numr_idx = Some(i);
                state.number = true;
                i += 1;
                continue;
            }

            if state.is_empty() {
                i += 1;
                continue;
            }

            if i < n - 2
                && ["з", "із", "зі"].contains(&clean_lower.as_str())
                && ["половиною", "третиною", "чвертю", "гаком"]
                    .contains(&view[i + 1].surface().to_lowercase().as_str())
            {
                i += 2;
                continue;
            }

            let numr_at = view[state.numr_idx.unwrap()];
            let numr_lower = numr_at.surface().to_lowercase();

            if i < n - 1
                && (TWO_TO_FOUR.is_match(&numr_lower).unwrap_or(false)
                    || DVA_3_4.is_match(&numr_lower).unwrap_or(false))
                && has_tag_re(tr, &ADJ_P_ROD_NAZ)
                && uk_helpers::has_pos_tag_and_token(view[i + 1], &M_V_ROD_TOKEN, &TOKEN_AA_YA)
            {
                i += 1;
                continue;
            }

            let numr_clean = numr_at.surface().to_string();
            let numr_token = numr_clean.to_lowercase();

            if (Regex::new(r"^(?:один-|одне-)?півтора$")
                .unwrap()
                .is_match(&numr_token)
                .unwrap_or(false)
                || FRACT.is_match(&numr_token).unwrap_or(false))
                && ["раз", "рази", "разу", "разів"].contains(&clean_lower.as_str())
            {
                out.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        "Після десяткового дробу або «півтора» треба вживати «раза»",
                        Some(SHORT.to_string()),
                        TextRange::new(
                            sentence_offset + numr_at.start_pos,
                            sentence_offset + tr.end_pos(),
                        ),
                        vec![Suggestion {
                            value: format!("{} раза", numr_at.surface()),
                            short_description: None,
                        }],
                        CATEGORY_ID,
                        CATEGORY_NAME,
                    )
                    .with_metadata(DESCRIPTION, "misspelling", 0),
                );
                state.reset();
                i += 1;
                continue;
            }

            if clean_lower == "тон" {
                out.push(
                    Match::new(
                        RULE_ID,
                        Option::<String>::None,
                        "Ви мали на увазі: «тонн»?",
                        Some(SHORT.to_string()),
                        TextRange::new(
                            sentence_offset + tr.start_pos,
                            sentence_offset + tr.end_pos(),
                        ),
                        vec![Suggestion {
                            value: "тонн".to_string(),
                            short_description: None,
                        }],
                        CATEGORY_ID,
                        CATEGORY_NAME,
                    )
                    .with_metadata(DESCRIPTION, "misspelling", 0),
                );
                state.reset();
                i += 1;
                continue;
            }

            let mut noun_readings: Vec<AnalyzedToken> = Vec::new();
            for at in &tr.readings {
                let Some(tag) = at.pos_tag.as_deref() else {
                    continue;
                };
                if tag.ends_with("_END") {
                    continue;
                }
                if NOUN_IGNORE.is_match(tag).unwrap_or(false) {
                    noun_readings.clear();
                    break;
                }
                if tag.starts_with("noun") || tag.starts_with("adj") {
                    noun_readings.push(at.clone());
                } else if !uk_helpers::is_predict_or_insert(at) {
                    noun_readings.clear();
                    break;
                }
            }

            if numr_lower.ends_with("багато")
                && !(uk_helpers::has_male_ua(tr)
                    || NOUN_FORCE.is_match(&clean_lower).unwrap_or(false))
            {
                state.reset();
                i += 1;
                continue;
            }

            if noun_readings.is_empty() {
                state.reset();
                i += 1;
                continue;
            }
            state.noun_pos = i;

            if let Some(m) = self.check_match(
                &view,
                &state,
                &numr_clean,
                &numr_token,
                &noun_readings,
                tr,
                sentence_offset,
                n,
                i,
            ) {
                out.push(m);
            }
            state.reset();
            i += 1;
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn check_match(
        &self,
        view: &[&AnalyzedTokenReadings],
        state: &State,
        numr_clean: &str,
        numr_token: &str,
        noun_readings: &[AnalyzedToken],
        noun_at: &AnalyzedTokenReadings,
        sentence_offset: usize,
        n: usize,
        i: usize,
    ) -> Option<Match> {
        let numr_at = view[state.numr_idx?];
        let mut master: Vec<Inflection> = Vec::new();
        let mut gender_of_plural_not_found: Option<&'static str> = None;

        if state.numr_pos + 2 == i
            && ["десятих", "сотих", "тисячних", "третіх", "четвертих"]
                .contains(&view[i - 1].surface().to_lowercase().as_str())
        {
            for g in ["m", "f", "n"] {
                master.push(Inflection {
                    gender: g.into(),
                    case_: "v_rod".into(),
                    anim_tag: None,
                });
            }
        } else if state.number {
            if FIVE_5.is_match(numr_clean).unwrap_or(false) {
                for g in ["p", "m", "f", "n"] {
                    master.push(Inflection {
                        gender: g.into(),
                        case_: "v_rod".into(),
                        anim_tag: None,
                    });
                }
            } else if TWO_5.is_match(numr_clean).unwrap_or(false) {
                master.push(Inflection {
                    gender: "p".into(),
                    case_: "v_naz".into(),
                    anim_tag: None,
                });
                master.push(Inflection {
                    gender: "p".into(),
                    case_: "v_zna".into(),
                    anim_tag: Some("inanim".into()),
                });
                for g in ["m", "f", "n"] {
                    master.push(Inflection {
                        gender: g.into(),
                        case_: "v_rod".into(),
                        anim_tag: None,
                    });
                }
            } else if ONE_5.is_match(numr_clean).unwrap_or(false)
                || FRACT.is_match(numr_clean).unwrap_or(false)
            {
                for g in ["m", "f", "n"] {
                    master.push(Inflection {
                        gender: g.into(),
                        case_: "v_rod".into(),
                        anim_tag: None,
                    });
                }
            } else if TWO_TO_FOUR.is_match(numr_clean).unwrap_or(false)
                && uk_helpers::has_pos_tag_and_token(noun_at, &M_V_ROD_TOKEN, &TOKEN_AA_YA)
            {
                if is_nyn_case(noun_at) {
                    master.push(Inflection {
                        gender: "m".into(),
                        case_: "v_rod".into(),
                        anim_tag: None,
                    });
                } else {
                    master.push(Inflection {
                        gender: "p".into(),
                        case_: "v_naz".into(),
                        anim_tag: None,
                    });
                    master.push(Inflection {
                        gender: "p".into(),
                        case_: "v_zna".into(),
                        anim_tag: None,
                    });
                }
            } else if TWO_TO_FOUR.is_match(numr_clean).unwrap_or(false) {
                if is_nyn_case(noun_at) {
                    master.push(Inflection {
                        gender: "m".into(),
                        case_: "v_rod".into(),
                        anim_tag: None,
                    });
                } else {
                    return None;
                }
            } else if FIVE_TO_NINE.is_match(numr_clean).unwrap_or(false)
                && NOUN_FORCE
                    .is_match(&noun_at.surface().to_lowercase())
                    .unwrap_or(false)
            {
                master.push(Inflection {
                    gender: "p".into(),
                    case_: "v_rod".into(),
                    anim_tag: None,
                });
            } else {
                return None;
            }
        } else {
            master = if has_tag_readings(&state.numr_readings, &NUMR_PATTERN) {
                inflection::get_adj_inflections_start(&state.numr_readings, "numr")
            } else {
                vec![Inflection {
                    gender: "p".into(),
                    case_: "v_rod".into(),
                    anim_tag: None,
                }]
            };
            let p_vnaz_zna: Vec<Inflection> = master
                .iter()
                .filter(|inf| inf.gender == "p" && (inf.case_ == "v_naz" || inf.case_ == "v_zna"))
                .cloned()
                .collect();
            if !p_vnaz_zna.is_empty() {
                if FIVE_TO_NINE_ALPHA.is_match(numr_token).unwrap_or(false) {
                    master.retain(|m| !p_vnaz_zna.contains(m));
                    master.push(Inflection {
                        gender: "p".into(),
                        case_: "v_rod".into(),
                        anim_tag: None,
                    });
                } else if Regex::new(r"^((.+-)?(двоє|двох|троє|.+еро|.+ьох))|обидвоє|обидвох|обоє|обох|двійко$")
                    .unwrap()
                    .is_match(numr_token)
                    .unwrap_or(false)
                {
                    master.retain(|m| !p_vnaz_zna.contains(m));
                    master.push(Inflection {
                        gender: "p".into(),
                        case_: "v_rod".into(),
                        anim_tag: None,
                    });
                } else if Regex::new(r"^(?:не)?багато|багато-багато|(?:не|чи)?мало|с[тк]ільки(?:-то|сь)?|.+-скільки|кілько$")
                    .unwrap()
                    .is_match(numr_token)
                    .unwrap_or(false)
                {
                    master.retain(|m| !p_vnaz_zna.contains(m));
                    for g in ["p", "m", "n", "f"] {
                        master.push(Inflection {
                            gender: g.into(),
                            case_: "v_rod".into(),
                            anim_tag: None,
                        });
                    }
                } else if numr_token == "пів" {
                    master.clear();
                    for g in ["m", "f", "n"] {
                        master.push(Inflection {
                            gender: g.into(),
                            case_: "v_rod".into(),
                            anim_tag: None,
                        });
                    }
                } else if DVA_3_4.is_match(numr_token).unwrap_or(false) {
                    master.retain(|m| !p_vnaz_zna.contains(m));
                    if has_lemma(noun_at, &["друг"]) {
                        master = vec![Inflection {
                            gender: "m".into(),
                            case_: "v_rod".into(),
                            anim_tag: None,
                        }];
                    } else if is_nyn_case(noun_at) {
                        master.push(Inflection {
                            gender: "m".into(),
                            case_: "v_rod".into(),
                            anim_tag: None,
                        });
                        if numr_token == "обидва"
                            && (i == n - 1
                                || Regex::new(r"^[.,;!?)—–-]$")
                                    .unwrap()
                                    .is_match(view[i + 1].surface())
                                    .unwrap_or(false)
                                || has_tag_re(
                                    view[i + 1],
                                    &Regex::new(r"^noun:inanim:.:v_rod:prop:geo.*$").unwrap(),
                                ))
                        {
                            master.push(Inflection {
                                gender: "p".into(),
                                case_: "v_naz".into(),
                                anim_tag: None,
                            });
                        }
                        if numr_token == "обидві" {
                            master.push(Inflection {
                                gender: "p".into(),
                                case_: "v_naz".into(),
                                anim_tag: None,
                            });
                        }
                    } else {
                        master.push(Inflection {
                            gender: "p".into(),
                            case_: "v_naz".into(),
                            anim_tag: None,
                        });
                        if has_tag_re(noun_at, &Regex::new(r"^noun:inanim:p:v_zna.*$").unwrap())
                            || (has_tag_re(noun_at, &Regex::new(r"^adj:p:v_zna.*$").unwrap())
                                && (i == n - 1
                                    || !has_tag_re(
                                        view[i + 1],
                                        &Regex::new(r"^noun:.*p:v_rod.*$").unwrap(),
                                    )))
                        {
                            master.push(Inflection {
                                gender: "p".into(),
                                case_: "v_zna".into(),
                                anim_tag: None,
                            });
                        }
                    }

                    if DVI.is_match(numr_token).unwrap_or(false) {
                        gender_of_plural_not_found =
                            self.find_plural_gender(noun_readings, &master, DviGender::F);
                    } else if DVA.is_match(numr_token).unwrap_or(false) {
                        gender_of_plural_not_found =
                            self.find_plural_gender(noun_readings, &master, DviGender::MN);
                    }
                }
            } else if Regex::new(r"^(?:один-|одне-)?півтора$")
                .unwrap()
                .is_match(numr_token)
                .unwrap_or(false)
            {
                master.clear();
                master.push(Inflection {
                    gender: "m".into(),
                    case_: "v_rod".into(),
                    anim_tag: None,
                });
                master.push(Inflection {
                    gender: "n".into(),
                    case_: "v_rod".into(),
                    anim_tag: None,
                });
            } else if Regex::new(r"^(?:одн.+-)?півтори$")
                .unwrap()
                .is_match(numr_token)
                .unwrap_or(false)
            {
                master.clear();
                master.push(Inflection {
                    gender: "f".into(),
                    case_: "v_rod".into(),
                    anim_tag: None,
                });
            }
        }

        let mut noun_inflections = inflection::get_noun_inflections(noun_readings, None);
        noun_inflections.extend(inflection::get_adj_inflections(noun_readings));
        let mut deduped: Vec<Inflection> = Vec::new();
        for inf in noun_inflections {
            if !deduped.contains(&inf) {
                deduped.push(inf);
            }
        }
        let mut noun_inflections = deduped;

        let disjoint = master.iter().all(|m| !noun_inflections.contains(m));
        if gender_of_plural_not_found.is_none() && !disjoint {
            return None;
        }

        if is_exception(
            view,
            state,
            &master,
            &noun_inflections,
            noun_readings,
            numr_token,
            i,
            n,
        ) {
            return None;
        }

        let mut msg = format!(
            "Потенційна помилка: числівник не узгоджений з іменником: \"{}\" вимагає: [{}], а далі йде \"{}\": [{}]",
            state.numr_readings.first().map(|r| r.token.clone()).unwrap_or_default(),
            inflection::format_inflections(&mut master, true),
            noun_readings.first().map(|r| r.token.clone()).unwrap_or_default(),
            inflection::format_inflections(&mut noun_inflections, false),
        );

        if ONE_5.is_match(numr_clean).unwrap_or(false) {
            msg = "Після «1,5» треба вживати родовий відмінок однини".to_string();
        } else if TWO_5.is_match(numr_clean).unwrap_or(false) {
            msg = "Після числівника, що закінчується на 2-4 і потім «,5», іменник має стояти в називному відмінку множини (якщо вимовляємо «з половиною»), або в родовом відмінку однини (якщо вимовляємо «і п'ять десятих»)".to_string();
        } else if numr_clean.ends_with(",5") {
            msg = "Після числівника, що закінчується на 5-9 і потім «,5», іменник має стояти в родовому відмінку множини (якщо вимовляємо «з половиною»), або в родовом відмінку однини (якщо вимовляємо «і п'ять десятих»)".to_string();
        } else if numr_clean.eq_ignore_ascii_case("півтора") {
            msg = "Існує правило, що після «півтора» треба вживати родовий відмінок ч. або с.р., однак у текстах в багатьох випадках вживають і форму множини, надто коли перед іменником іде прикметник".to_string();
        } else if numr_clean.eq_ignore_ascii_case("півтори") {
            msg = "Існує правило, що після «півтора» треба вживати родовий відмінок ж.р., однак у текстах в багатьох випадках вживають і форму множини, надто коли перед іменником іде прикметник".to_string();
        } else if master.contains(&Inflection {
            gender: "m".into(),
            case_: "v_rod".into(),
            anim_tag: None,
        }) && Regex::new(r"^.*[ую]$")
            .unwrap()
            .is_match(noun_at.surface())
            .unwrap_or(false)
            && has_tag_re(noun_at, &NOUN_M_V_DAV)
        {
            msg.push_str(USED_U_INSTEAD_OF_A_MSG);
        } else if !has_tag_readings(&state.numr_readings, &NUMR_ADJ_V_MIS)
            && has_tag_re(noun_at, &NOUN_V_MIS)
        {
            msg.push_str(". Можливо, пропущено прийменник на/в/у...?");
        }

        if disjoint && gender_of_plural_not_found.is_some() {
            msg.push_str(". Можливо, не збігається рід однини для множинної форми?");
        }

        let mut suggestions: Vec<String> = Vec::new();
        if let Some(g) = gender_of_plural_not_found.filter(|_| !disjoint) {
            let sugg1 = if g == "f" {
                numr_clean.replacen('і', "а", 1)
            } else {
                numr_clean.replacen('а', "і", 1)
            };
            suggestions.push(format!("{sugg1} {}", view[state.noun_pos].surface()));
        } else {
            for numr_inf in &master {
                let gender_tag = format!(":{}:", numr_inf.gender);
                for noun_token in noun_readings {
                    let Some(noun_pos) = noun_token.pos_tag.as_deref() else {
                        continue;
                    };
                    if numr_inf.anim_matters() {
                        let anim = if noun_pos.starts_with("noun") {
                            format!(":{}", numr_inf.anim_tag.as_deref().unwrap_or(""))
                        } else {
                            format!(":r{}", numr_inf.anim_tag.as_deref().unwrap_or(""))
                        };
                        if !noun_pos.contains(&anim) {
                            continue;
                        }
                    }
                    let re = Regex::new(r":.:v_...").unwrap();
                    let new_tag = re.replace(noun_pos, &format!("{gender_tag}{}", numr_inf.case_));
                    for s in self.synth.synthesize(noun_token, &new_tag, false) {
                        if numr_clean.eq_ignore_ascii_case("півтора")
                            && noun_token.stem.as_deref() == Some("раз")
                            && s != "раза"
                        {
                            continue;
                        }
                        let mut suggestion = numr_at.surface().to_string();
                        for mid in &view[state.numr_pos + 1..state.noun_pos] {
                            suggestion.push(' ');
                            suggestion.push_str(mid.surface());
                        }
                        suggestion.push(' ');
                        suggestion.push_str(&s);
                        if !suggestions.contains(&suggestion) {
                            suggestions.push(suggestion);
                        }
                    }
                }
            }
        }

        Some(
            Match::new(
                RULE_ID,
                Option::<String>::None,
                msg,
                Some(SHORT.to_string()),
                TextRange::new(
                    sentence_offset + numr_at.start_pos,
                    sentence_offset + noun_at.end_pos(),
                ),
                suggestions
                    .into_iter()
                    .map(|value| Suggestion {
                        value,
                        short_description: None,
                    })
                    .collect(),
                CATEGORY_ID,
                CATEGORY_NAME,
            )
            .with_metadata(DESCRIPTION, "misspelling", 0),
        )
    }

    fn find_plural_gender(
        &self,
        noun_readings: &[AnalyzedToken],
        master: &[Inflection],
        which: DviGender,
    ) -> Option<&'static str> {
        let vidm = if master.len() == 2 {
            "(naz|zna)"
        } else {
            "naz"
        };
        let pattern = if master.len() == 2 {
            format!("^noun.*:p:v_{vidm}(?!:ns).*$")
        } else {
            format!("^noun.*:p:v_{vidm}.*$")
        };
        let re = Regex::new(&pattern).unwrap();
        if !has_tag_readings(noun_readings, &re) {
            return None;
        }
        if has_tag_readings(
            noun_readings,
            &Regex::new(&format!("^adj:p:v_{vidm}.*$")).unwrap(),
        ) {
            return None;
        }
        let look_for = match which {
            DviGender::F => ":f:",
            DviGender::MN => ":[mn]:",
        };
        let found = find_singulars(&self.synth, noun_readings, &re, look_for)?;
        if found.is_empty() {
            Some(match which {
                DviGender::F => "f",
                DviGender::MN => "mn",
            })
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
enum DviGender {
    F,
    MN,
}

fn is_nyn_case(noun_at: &AnalyzedTokenReadings) -> bool {
    let m_v_rod = Regex::new(r"^noun:anim:m:v_rod.*$").unwrap();
    let nin = Regex::new(r"^.*нин[ая]$").unwrap();
    let m_vrod_filtered = uk_helpers::filter_token(noun_at, &m_v_rod, &nin);
    if !m_vrod_filtered.is_empty() {
        return m_vrod_filtered.iter().any(|r| {
            let token = r.token.to_lowercase();
            let re = Regex::new(r"[ая]$").unwrap();
            let lemma = re.replace(&token, "").into_owned();
            r.stem.as_deref() == Some(lemma.as_str())
        });
    }
    let p_v_naz = Regex::new(r"^noun:anim:p:v_naz.*$").unwrap();
    let ny = Regex::new(r"^.*ни$").unwrap();
    let p_vnaz_filtered = uk_helpers::filter_token(noun_at, &p_v_naz, &ny);
    p_vnaz_filtered.iter().any(|r| {
        let token = r.token.to_lowercase();
        let re = Regex::new(r"ни$").unwrap();
        let lemma = format!("{}нин", re.replace(&token, ""));
        r.stem.as_deref() == Some(lemma.as_str())
    })
}

fn find_singulars(
    synth: &UkrainianSynthesizer,
    noun_readings: &[AnalyzedToken],
    pattern: &Regex,
    look_for: &str,
) -> Option<HashSet<String>> {
    let mut found = HashSet::new();
    for tr in noun_readings {
        let Some(pos) = tr.pos_tag.as_deref() else {
            continue;
        };
        if !pattern.is_match(pos).unwrap_or(false) {
            continue;
        }
        if synth.synthesize(tr, pos, false).is_empty() {
            return None;
        }
        if !found.contains(tr.stem.as_deref().unwrap_or("")) {
            let mut singular_tag = pos.replacen(":p:", look_for, 1);
            singular_tag = Regex::new(r":(var|bad|arch)")
                .unwrap()
                .replace_all(&singular_tag, ".*")
                .into_owned();
            for s in synth.synthesize(tr, &singular_tag, true) {
                found.insert(s);
            }
        }
    }
    Some(found)
}

#[allow(clippy::too_many_arguments)]
fn is_exception(
    view: &[&AnalyzedTokenReadings],
    state: &State,
    numr: &[Inflection],
    noun_inflections: &[Inflection],
    noun_readings: &[AnalyzedToken],
    _numr_token: &str,
    _i: usize,
    n: usize,
) -> bool {
    let numr_at = view[state.numr_idx.unwrap()];
    let noun_at = view[state.noun_pos];
    let numr_lower = numr_at.surface().to_lowercase();
    let noun_lower = noun_at.surface().to_lowercase();

    let disjoint = |a: &[Inflection], b: &[Inflection]| a.iter().all(|x| !b.contains(x));

    // багатьох, обох, двох...
    if Regex::new(
        r"^(?:багать(ох|ом|ма)|обо(х|м|ма)|(дв|трь|чотир)о[хм]|скільки(сь)?(-небудь)?|стільки)$",
    )
    .unwrap()
    .is_match(&numr_lower)
    .unwrap_or(false)
    {
        return true;
    }
    // плюс, мінус, ранку...
    if Regex::new(r"^(?:плюс|мінус|ранку|вечора|ночі|тепла|морозу|родом|зростом|дивом|станом|вагою|слід|типу|формату|вартістю|році|населення)$")
        .unwrap()
        .is_match(&noun_lower)
        .unwrap_or(false)
    {
        return true;
    }
    // весь, який, свій...
    if has_lemma_re(
        noun_at,
        &Regex::new(r"^(?:у?весь|який(сь)?|свій|сам|цей|решта|кількість|вартий|кожний|жодний|менший|більший|вищий|нижчий)$").unwrap(),
    ) {
        return true;
    }
    // хвилин п'ять люди
    if state.numr_pos > 1
        && has_lemma_with(
            view[state.numr_pos - 1],
            uk_helpers::TIME_PLUS_LEMMAS,
            &Regex::new(r"^noun.*?.:v_(naz|rod).*$").unwrap(),
        )
    {
        let ni = inflection::get_noun_inflections(&view[state.numr_pos - 1].readings, None);
        if !disjoint(numr, &ni) {
            return true;
        }
    }
    // півтора довгих роки
    if state.noun_pos < n - 1
        && Regex::new(r"^(?:один-|одне-)?півтора|(?:одна-)?півтори$")
            .unwrap()
            .is_match(&numr_at.surface().to_lowercase())
            .unwrap_or(false)
        && has_tag_re(view[state.noun_pos], &ADJ_P_ROD_NAZ)
        && has_tag_re(
            view[state.noun_pos + 1],
            &Regex::new(r"^noun.*?:p:v_naz.*$").unwrap(),
        )
    {
        return true;
    }
    // хвилин зо п'ять люди
    if state.numr_pos > 2
        && has_tag_start(view[state.numr_pos - 1], "prep")
        && has_lemma_with(
            view[state.numr_pos - 2],
            uk_helpers::TIME_PLUS_LEMMAS,
            &Regex::new(r"^noun.*?p:v_(naz|rod).*$").unwrap(),
        )
    {
        let ni = inflection::get_noun_inflections(&view[state.numr_pos - 2].readings, None);
        if !disjoint(numr, &ni) {
            return true;
        }
    }
    // У свої вісімдесят пан Василь
    if state.numr_pos > 2
        && has_tag_start(view[state.numr_pos - 2], "prep")
        && view[state.numr_pos - 1].surface().to_lowercase() == "свої"
        && has_tag_re(
            view[state.numr_pos],
            &Regex::new(r"^numr:p:v_zna.*$").unwrap(),
        )
        && has_tag_re(noun_at, &Regex::new(r"^noun:anim:.:v_naz.*$").unwrap())
    {
        return true;
    }
    // два провінційного вигляду персонажі
    if state.noun_pos + 2 < n
        && has_tag_re(
            view[state.noun_pos],
            &Regex::new(r"^adj:.:v_rod.*$").unwrap(),
        )
        && has_tag_re(
            view[state.noun_pos + 1],
            &Regex::new(r"^noun:inanim:.:v_rod(?!.*pron).*$").unwrap(),
        )
        && has_tag_re(
            view[state.noun_pos + 2],
            &Regex::new(r"^noun(?!.*pron).*$").unwrap(),
        )
    {
        let adj1 = uk_helpers::get_genders_token(
            view[state.noun_pos],
            &Regex::new(r"^adj:.:v_rod.*$").unwrap(),
        );
        let noun1 = uk_helpers::get_genders_token(
            view[state.noun_pos + 1],
            &Regex::new(r"^noun:inanim:.:v_rod(?!.*pron).*$").unwrap(),
        );
        if !noun1.is_empty()
            && Regex::new(&format!("^.*[{noun1}].*$"))
                .unwrap()
                .is_match(&adj1)
                .unwrap_or(false)
        {
            let ni = inflection::get_noun_inflections(&view[state.noun_pos + 2].readings, None);
            if !disjoint(numr, &ni) {
                return true;
            }
        }
    }
    // handled by another rule
    if numr_lower.ends_with(",5")
        && (Regex::new(r"^(?:тон|тис|коп)$")
            .unwrap()
            .is_match(&noun_lower)
            .unwrap_or(false)
            || (state.numr_pos > 1
                && Regex::new(r"^(?:від|до|протягом|[ув]продовж|близько|після|для|більше|менше)$")
                    .unwrap()
                    .is_match(&view[state.numr_pos - 1].surface().to_lowercase())
                    .unwrap_or(false)))
    {
        return true;
    }
    // обоє горбаті
    if Regex::new(r"^(?:обоє|двоє|троє|.+еро)$")
        .unwrap()
        .is_match(&numr_lower)
        .unwrap_or(false)
        && uk_helpers::has_pos_tag_and_token(
            noun_at,
            &Regex::new(r"^adj:p:v_naz.*$").unwrap(),
            &Regex::new(r"^.+і$").unwrap(),
        )
    {
        return true;
    }
    // обоє режисери
    if Regex::new(r"^(?:обоє|обидвоє|троє)$")
        .unwrap()
        .is_match(&numr_lower)
        .unwrap_or(false)
        && has_tag_re(noun_at, &Regex::new(r"^noun:anim:p:v_naz.*$").unwrap())
    {
        return true;
    }
    // 22 червня
    if state.number
        && has_lemma_with(
            noun_at,
            uk_helpers::MONTH_LEMMAS,
            &Regex::new(r"^:m:v_rod$").unwrap(),
        )
    {
        return true;
    }
    // 3 / 4 понеділка
    if state.numr_pos > 2 && view[state.numr_pos - 1].surface() == "/" {
        return true;
    }
    // ч., ст., №
    if state.numr_pos > 1
        && (has_lemma(
            view[state.numr_pos - 1],
            &[
                "ч.",
                "ст.",
                "п.",
                "частина",
                "стаття",
                "пункт",
                "підпункт",
                "абзац",
                "№",
                "номер",
            ],
        ) || view[state.numr_pos - 1].surface() == "№")
    {
        return true;
    }
    // двадцять перший; дві соті
    if has_tag_re(noun_at, &Regex::new(r"^adj.*numr.*$").unwrap()) {
        return true;
    }
    if DVA_3_4.is_match(&numr_lower).unwrap_or(false) || state.number {
        if has_tag_re(
            noun_at,
            &Regex::new(r"^adj(?!.*numr).*:p:v_rod.*$").unwrap(),
        ) && (state.noun_pos == n - 1
            || has_tag_re(
                view[state.noun_pos + 1],
                &Regex::new(r"^(?:adj(?!.*numr).*:p:v_rod.*|noun.*:p:v_naz.*|prep)$").unwrap(),
            )
            || !has_tag_re(
                view[state.noun_pos + 1],
                &Regex::new(r"^(?:adj|noun).*$").unwrap(),
            )
            || Regex::new(r"^[.,:;()«»—–-]|і|й|та$")
                .unwrap()
                .is_match(view[state.noun_pos + 1].surface())
                .unwrap_or(false))
        {
            return true;
        }
        if view[state.noun_pos]
            .surface()
            .to_lowercase()
            .ends_with("их")
            && has_tag_re(
                view[state.noun_pos],
                &Regex::new(r"^noun.*:p:v_rod.*$").unwrap(),
            )
        {
            return true;
        }
    }
    // сьома вода
    if Regex::new(r"^(?:сьома|дев.яноста)$")
        .unwrap()
        .is_match(&numr_lower)
        .unwrap_or(false)
        && has_tag_re(
            noun_at,
            &Regex::new(r"^(?:(?:noun:.*?|adj):[fp]:v_naz.*)$").unwrap(),
        )
    {
        return true;
    }
    let _ = noun_readings;
    let _ = noun_inflections;
    false
}
