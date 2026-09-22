//! Partial port of `org.languagetool.tagging.uk.CompoundTagger`: the entity and
//! numeric-compound paths (`generateEntities`, `matchDigitCompound`,
//! `matchNumberedProperNoun`) plus the early `doGuessCompoundTag` branches
//! (one-letter prefix, digit/`XLIV` compounds, numbered proper nouns, invalid
//! dash prefixes, `пів-`). The deeper dash-compound agreement logic
//! (`tagMatch`/`tryOWithAdj`/`getNvPrefixNounMatch`/`guessOtherTags` …) is not
//! ported yet and returns `None`.

use std::path::Path;
use std::sync::LazyLock;

use fancy_regex::Regex;
use lt_core::AnalyzedToken;

use crate::uk_helpers::{self, full_match};
use crate::ukrainian::UkrainianTagger;

static YEAR_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[12][0-9]{3}$").unwrap());
static NOUN_PREFIX_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]+$").unwrap());
static NOUN_WITH_INTERVAL_PREFIX_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]+[-–])?[0-9]+$").unwrap());
static NOUN_SUFFIX_NUMBER_LETTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9][0-9А-ЯІЇЄҐ-]*$").unwrap());
static ADJ_PREFIX_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:[0-9]+(,[0-9]+)?([-–—][0-9]+(,[0-9]+)?)?%?|(XC|XL|L?X{0,3})(IX|IV|V?I{0,3})|І{2,3})$",
    )
    .unwrap()
});
static REQ_NUM_DVA_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(місн|томник|поверхів).{0,4}").unwrap());
static REQ_NUM_DESYAT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(класни[кц]|бальни[кц]|раундов|томн|томов|хвилин|десятиріч|кілометрів|річ).{0,4}")
        .unwrap()
});
static REQ_NUM_STO_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(річч|літт|метрів|грамов|тисячник).{0,3}").unwrap());
static DASH_PREFIX_LAT_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[a-zA-Z]{3,}|[α-ωΑ-Ω]").unwrap());
static XLIV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[XLIV]+-.*").unwrap());

/// `dash_prefixes.txt` (key -> value) and `dash_prefixes_invalid.txt`.
#[derive(Debug, Default)]
pub struct CompoundTagger {
    dash_prefixes: std::collections::HashMap<String, String>,
    dash_prefixes_invalid: std::collections::HashSet<String>,
    numbered_entities: Vec<(String, Vec<String>)>,
}

impl CompoundTagger {
    pub fn load(data_dir: &Path) -> Self {
        let words = data_dir.join("uk/words");
        let mut dash_prefixes = std::collections::HashMap::new();
        for line in read_lines(&words.join("dash_prefixes.txt")) {
            let mut parts = line.split(' ');
            if let Some(key) = parts.next() {
                let value = parts.next().unwrap_or("");
                dash_prefixes.insert(key.to_string(), value.to_string());
            }
        }
        let dash_prefixes_invalid = read_lines(&words.join("dash_prefixes_invalid.txt"))
            .into_iter()
            .collect();
        let numbered_entities = read_lines(&words.join("entities.txt"))
            .into_iter()
            .filter_map(|line| {
                let line = line.split('#').next().unwrap_or("").trim().to_string();
                if line.is_empty() {
                    return None;
                }
                let mut parts = line.split([' ', '|']);
                let key = parts.next()?.to_string();
                let list: Vec<String> = parts.map(|s| s.to_string()).collect();
                Some((key, list))
            })
            .collect();
        Self {
            dash_prefixes,
            dash_prefixes_invalid,
            numbered_entities,
        }
    }

    fn is_dash_prefix_match(&self, left_word: &str, left_lower: &str) -> bool {
        self.dash_prefixes.contains_key(left_word)
            || self.dash_prefixes.contains_key(left_lower)
            || full_match(&DASH_PREFIX_LAT_PATTERN, left_word)
    }

    /// `CompoundTagger.generateEntities`.
    pub fn generate_entities(&self, word: &str) -> Vec<AnalyzedToken> {
        let mut out: Vec<AnalyzedToken> = Vec::new();
        for (pattern, tags) in &self.numbered_entities {
            if !full_match(&Regex::new(pattern).unwrap(), word) {
                continue;
            }
            for tag in tags {
                if tag.contains(":nv") {
                    let mut tag_parts = tag.split(':');
                    let _ = tag_parts.next();
                    let gender = tag_parts.next().unwrap_or("");
                    let extra_tags = tag
                        .split_once(":nv")
                        .map(|(_, rest)| rest.replace(":np", ""))
                        .unwrap_or_default();
                    let new_tokens =
                        uk_helpers::generate_tokens_for_nv(word, gender, Some(&extra_tags));
                    push_unique(&mut out, new_tokens);
                    if !tag.contains(":np") && !tag.contains(":p") {
                        let new_tokens =
                            uk_helpers::generate_tokens_for_nv(word, "p", Some(&extra_tags));
                        push_unique(&mut out, new_tokens);
                    }
                } else {
                    let token = AnalyzedToken::new(word, Some(word.to_string()), Some(tag.clone()));
                    if !out.contains(&token) {
                        out.push(token);
                    }
                }
            }
        }
        out
    }

    /// `CompoundTagger.guessCompoundTag` (partial: the branches implemented so
    /// far; `None` for the rest).
    pub fn guess_compound_tag(
        &self,
        tagger: &UkrainianTagger,
        word: &str,
    ) -> Option<Vec<AnalyzedToken>> {
        let dash_idx = word.rfind('-')?;
        if dash_idx == word.len() - 1 {
            return None;
        }
        let first_dash_idx = word.find('-')?;
        if first_dash_idx == 0 {
            return None;
        }
        let starts_with_digit = word.chars().next().is_some_and(|c| c.is_ascii_digit());

        if !starts_with_digit && dash_idx != first_dash_idx {
            // multi-hyphen paths (`doGuessMultiHyphens`/`doGuessTwoHyphens`) not
            // ported yet
            return None;
        }

        let left_word = &word[..dash_idx];
        let right_word = &word[dash_idx + 1..];
        let left_lower = left_word.to_lowercase();

        // з-зателефоную
        if left_word.chars().count() == 1
            && right_word.chars().count() > 3
            && right_word.starts_with(&left_lower)
        {
            let mut tagged = tagger.word_lookup(right_word);
            tagged = adjust(&tagged, None, None, ":alt");
            return Some(tagger.tagged_to_at(word, &tagged));
        }

        let dash_prefix_match = self.is_dash_prefix_match(left_word, &left_lower);

        if !dash_prefix_match && (starts_with_digit || full_match(&XLIV, word)) {
            return self.match_digit_compound(tagger, word, left_word, right_word);
        }

        if right_word
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
        {
            return self.match_numbered_proper_noun(tagger, word, left_word, right_word);
        }

        // авіа..., авто... пишуться разом
        if self.dash_prefixes_invalid.contains(&left_lower) {
            let mut right = tagger.tag_either_case(right_word);
            right.retain(|(_, tag)| {
                Regex::new(r"^(?:noun|adj)(?!.*pron).*")
                    .unwrap()
                    .is_match(tag)
                    .unwrap_or(false)
            });
            if right.is_empty() {
                return None;
            }
            let extra_tag = if right_word.chars().next().is_some_and(|c| c.is_uppercase()) {
                ""
            } else {
                ":bad"
            };
            let right = adjust(&right, Some(&format!("{left_word}-")), None, extra_tag);
            return Some(tagger.tagged_to_at(word, &right));
        }

        // additional branches not ported yet
        None
    }

    /// `CompoundTagger.matchDigitCompound`.
    fn match_digit_compound(
        &self,
        tagger: &UkrainianTagger,
        word: &str,
        left_word: &str,
        right_word: &str,
    ) -> Option<Vec<AnalyzedToken>> {
        if !full_match(&ADJ_PREFIX_NUMBER, left_word) {
            return None;
        }
        let mut out: Vec<AnalyzedToken> = Vec::new();
        if let Some(tags) = uk_helpers::find_tags_adj(left_word, right_word) {
            for tag in tags {
                let lemma = format!("{left_word}-й");
                let tag = if tag.contains(":bad") {
                    tag.replace(":bad", ":numr:bad")
                } else {
                    format!("{tag}:numr")
                };
                out.push(AnalyzedToken::new(
                    word,
                    Some(lemma),
                    Some(format!("adj{tag}")),
                ));
            }
        } else if full_match(&NOUN_WITH_INTERVAL_PREFIX_NUMBER, left_word) {
            if let Some(tags) = uk_helpers::find_tags_noun(left_word, right_word) {
                for tag in tags {
                    out.push(AnalyzedToken::new(
                        word,
                        Some(left_word.to_string()),
                        Some(format!("numr{tag}")),
                    ));
                }
                return Some(out);
            }
            if right_word == "мм" {
                for gender in uk_helpers::BASE_GENDERS {
                    for vidm in uk_helpers::VIDMINKY {
                        if *vidm == "v_kly" {
                            continue;
                        }
                        out.push(AnalyzedToken::new(
                            word,
                            Some(word.to_string()),
                            Some(format!("adj:{gender}:{vidm}")),
                        ));
                    }
                }
                return Some(out);
            }
            if let Some(try_prefix) = get_try_prefix(&right_word.to_lowercase()) {
                let right =
                    tagger.word_lookup(&format!("{try_prefix}{}", right_word.to_lowercase()));
                for at in tagger.tagged_to_at(right_word, &right) {
                    let lemma = at.stem.unwrap_or_default();
                    let lemma = lemma
                        .chars()
                        .skip(try_prefix.chars().count())
                        .collect::<String>();
                    out.push(AnalyzedToken::new(
                        word,
                        Some(format!("{left_word}-{lemma}")),
                        at.pos_tag,
                    ));
                }
                return Some(out);
            }
            let right = tagger.word_lookup(right_word);
            if right.is_empty() || right.iter().any(|(_, tag)| tag.contains("pron")) {
                return None;
            }
            for at in tagger.tagged_to_at(right_word, &right) {
                let pos_tag = at.pos_tag.clone().unwrap_or_default();
                let lemma = at.stem.clone().unwrap_or_default();
                if pos_tag.starts_with("adj") || lemma == "відсотково" {
                    out.push(AnalyzedToken::new(
                        word,
                        Some(format!("{left_word}-{lemma}")),
                        Some(pos_tag),
                    ));
                }
            }
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    /// `CompoundTagger.matchNumberedProperNoun`.
    fn match_numbered_proper_noun(
        &self,
        tagger: &UkrainianTagger,
        word: &str,
        left_word: &str,
        right_word: &str,
    ) -> Option<Vec<AnalyzedToken>> {
        // Ан-140
        if full_match(&NOUN_SUFFIX_NUMBER_LETTER, right_word) {
            let entities = self.generate_entities(word);
            if !entities.is_empty() {
                return Some(entities);
            }
        }
        // Вибори-2014
        if full_match(&YEAR_NUMBER, right_word) {
            let left = tagger.tag_as_is_and_with_lowercase(left_word);
            if !left.is_empty() {
                let mut out = Vec::new();
                let is_uppercase = left_word.chars().next().is_some_and(|c| c.is_uppercase());
                for at in tagger.tagged_to_at(left_word, &left) {
                    let pos_tag = at.pos_tag.clone().unwrap_or_default();
                    let lemma = at.stem.clone().unwrap_or_default();
                    if !pos_tag.contains(":prop") && !WORDS_WITH_YEAR.contains(&lemma.as_str()) {
                        continue;
                    }
                    if !pos_tag.starts_with("noun:inanim") || pos_tag.contains("v_kly") {
                        continue;
                    }
                    if pos_tag.contains(":p:")
                        && !["гра", "бюджет"].contains(&lemma.as_str())
                        && !pos_tag.contains(":ns")
                    {
                        continue;
                    }
                    let mut pos_tag = pos_tag.replace(":geo", "");
                    let mut lemma = lemma;
                    if !pos_tag.contains(":prop") && is_uppercase {
                        pos_tag.push_str(":prop");
                        lemma = crate::english::uppercase_first_char(&lemma);
                    }
                    out.push(AnalyzedToken::new(
                        word,
                        Some(format!("{lemma}-{right_word}")),
                        Some(pos_tag),
                    ));
                }
                if !out.is_empty() {
                    return Some(out);
                }
            }
        }
        // Формула-1, Карпати-2, омега-3
        if full_match(&NOUN_PREFIX_NUMBER, right_word) {
            let left = tagger.tag_as_is_and_with_lowercase(left_word);
            if !left.is_empty() {
                let mut out = Vec::new();
                for at in tagger.tagged_to_at(left_word, &left) {
                    let pos_tag = at.pos_tag.clone().unwrap_or_default();
                    let lemma = at.stem.clone().unwrap_or_default();
                    if !pos_tag.starts_with("noun:inanim") || pos_tag.contains("v_kly") {
                        continue;
                    }
                    let mut pos_tag = pos_tag;
                    let mut lemma = lemma;
                    if !pos_tag.contains(":prop") && !WORDS_WITH_NUM.contains(&lemma.as_str()) {
                        pos_tag.push_str(":prop");
                        lemma = crate::english::uppercase_first_char(&lemma);
                    }
                    if !WORDS_WITH_NUM.contains(&lemma.as_str()) {
                        continue;
                    }
                    out.push(AnalyzedToken::new(
                        word,
                        Some(format!("{lemma}-{right_word}")),
                        Some(pos_tag),
                    ));
                }
                if !out.is_empty() {
                    return Some(out);
                }
            }
        }
        None
    }
}

const WORDS_WITH_YEAR: &[&str] = &[
    "бюджет",
    "вибори",
    "гра",
    "держбюджет",
    "кошторис",
    "кампанія",
    "єврокубок",
    "єврокваліфікація",
    "євровідбір",
    "єврофорум",
    "конкурс",
    "кінофестиваль",
    "кубок",
    "мундіаль",
    "м'яч",
    "олімпіада",
    "оцінювання",
    "оскар",
    "пектораль",
    "перегони",
    "першість",
    "політреформа",
    "премія",
    "рейтинг",
    "реформа",
    "сезон",
    "турнір",
    "універсіада",
    "фестиваль",
    "форум",
    "чемпіонат",
    "чемпіон",
    "чемпіонка",
    "ярмарок",
    "ЧУ",
    "ЧЄ",
];
const WORDS_WITH_NUM: &[&str] = &[
    "Формула",
    "Карпати",
    "Динамо",
    "Шахтар",
    "Фукусіма",
    "Квартал",
    "Золоте",
    "Мінськ",
    "Нюренберг",
    "омега",
    "плутоній",
    "полоній",
    "стронцій",
    "уран",
    "потік",
];

fn get_try_prefix(right_word: &str) -> Option<&'static str> {
    if REQ_NUM_STO_PATTERN.is_match(right_word).unwrap_or(false) {
        return Some("сто");
    }
    if REQ_NUM_DESYAT_PATTERN.is_match(right_word).unwrap_or(false) {
        return Some("десяти");
    }
    if REQ_NUM_DVA_PATTERN.is_match(right_word).unwrap_or(false) {
        return Some("дво");
    }
    None
}

/// `PosTagHelper.adjust` (shared with `UkRainianTagger`).
pub(crate) fn adjust(
    tagged: &[(String, String)],
    lemma_prefix: Option<&str>,
    lemma_suffix: Option<&str>,
    add_tag: &str,
) -> Vec<(String, String)> {
    let cleanup = Regex::new(r":(comp.|adjp:.*?(:(im)?perf)+)").unwrap();
    tagged
        .iter()
        .map(|(lemma, tag)| {
            let mut lemma = lemma.clone();
            if let Some(p) = lemma_prefix {
                lemma = format!("{p}{lemma}");
            }
            if let Some(s) = lemma_suffix {
                lemma = format!("{lemma}{s}");
            }
            let tag = cleanup.replace_all(tag, "").into_owned();
            let tag = if !add_tag.is_empty() && !tag.contains(add_tag) {
                format!("{tag}{add_tag}")
            } else {
                tag
            };
            (lemma, tag)
        })
        .collect()
}

fn push_unique(out: &mut Vec<AnalyzedToken>, tokens: Vec<AnalyzedToken>) {
    for t in tokens {
        if !out.contains(&t) {
            out.push(t);
        }
    }
}

fn read_lines(path: &Path) -> Vec<String> {
    let Ok(text) = lt_data::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}
