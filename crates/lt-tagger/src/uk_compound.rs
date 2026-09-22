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
    left_master_set: std::collections::HashSet<String>,
    follower_set: std::collections::HashSet<String>,
    no_dash_prefixes: std::collections::HashSet<String>,
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
        let dash_prefixes_invalid: std::collections::HashSet<String> =
            read_lines(&words.join("dash_prefixes_invalid.txt"))
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
        let left_master_set = read_lines(&words.join("dash_left_master.txt"))
            .into_iter()
            .collect();
        let follower_set = read_lines(&words.join("dash_follower.txt"))
            .into_iter()
            .collect();
        // `noDashPrefixes2019` = dash prefixes whose value contains "alt";
        // `noDashPrefixes` = invalid prefixes + those, minus мілі/поп/прес.
        let mut no_dash_prefixes: std::collections::HashSet<String> = dash_prefixes_invalid.clone();
        for (key, value) in &dash_prefixes {
            if value.contains("alt") {
                no_dash_prefixes.insert(key.clone());
            }
        }
        for drop in ["мілі", "поп", "прес"] {
            no_dash_prefixes.remove(drop);
        }
        Self {
            dash_prefixes,
            dash_prefixes_invalid,
            numbered_entities,
            left_master_set,
            follower_set,
            no_dash_prefixes,
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
            let dash_count = word.matches('-').count();
            if dash_count >= 2 && dash_idx > first_dash_idx + 1 {
                if let Some(tokens) =
                    self.do_guess_multi_hyphens(tagger, word, first_dash_idx, dash_idx)
                {
                    return Some(tokens);
                }
            }
            if dash_count == 2 && dash_idx > first_dash_idx + 1 {
                return self.do_guess_two_hyphens(tagger, word, first_dash_idx, dash_idx);
            }
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

        // wrong: пів-качана
        if left_lower == "пів" && right_word.chars().next().is_some_and(|c| c.is_lowercase()) {
            let right = tagger.tag_either_case(right_word);
            let right_at = tagger.tagged_to_at(right_word, &right);
            return Some(Self::add_plural_nv_tokens(word, &right_at, ":bad"));
        }

        let left_wd = tagger.tag_as_is_and_with_lowercase(left_word);

        // стривай-бо, чекай-но, прийшов-таки, такий-от, такий-то, ішов-єм
        let right_lower = right_word.to_lowercase();
        if let Some(left_tag_regex) = right_parts_with_left_tag(&right_lower) {
            if !uk_helpers::has_pos_tag_part2(&left_wd, "abbr") {
                if left_wd.is_empty() {
                    return None;
                }
                let left_at = tagger.tagged_to_at(left_word, &left_wd);
                if right_lower == "то" && uk_helpers::has_lemma(&left_at, &["хто", "що", "чи"])
                {
                    return None;
                }
                let mut new_tokens = Vec::new();
                for at in &left_at {
                    let pos_tag = at.pos_tag.as_deref().unwrap_or("");
                    if left_word.eq_ignore_ascii_case("як") && pos_tag.contains("noun") {
                        continue;
                    }
                    if !pos_tag.is_empty()
                        && ((left_lower == "дуже" && pos_tag.contains("adv"))
                            || left_tag_regex.is_match(pos_tag).unwrap_or(false))
                    {
                        let mut pos_tag = pos_tag.to_string();
                        if right_word == "єм" {
                            pos_tag = uk_helpers::add_if_not_contains(&pos_tag, ":arch");
                        }
                        new_tokens.push(AnalyzedToken::new(word, at.stem.clone(), Some(pos_tag)));
                    }
                }
                return if new_tokens.is_empty() {
                    None
                } else {
                    Some(new_tokens)
                };
            }
        }

        // по-болгарськи, по-болгарському
        let mut right_word_owned = right_word.to_string();
        if left_word.eq_ignore_ascii_case("по") && full_match(&SKY_PATTERN, &right_word_owned) {
            right_word_owned.push('й');
        }
        let right_word = right_word_owned.as_str();

        // Пенсильванія-авеню
        if left_word.chars().next().is_some_and(|c| c.is_uppercase())
            && uk_helpers::CITY_AVENU.contains(&right_lower.as_str())
        {
            let add_pos = if right_word == "штрассе" {
                ":alt"
            } else {
                ""
            };
            return Some(uk_helpers::generate_tokens_for_nv(
                word,
                "f",
                Some(&format!(":prop{add_pos}")),
            ));
        }

        // Fe-вмісний
        if right_lower.starts_with("вмісн") {
            let adjusted = format!("боро{right_word}");
            let mut right = tagger.tag_either_case(&adjusted);
            for (lemma, _) in right.iter_mut() {
                *lemma = "вмісний".to_string();
            }
            let right_at = tagger.tagged_to_at(right_word, &right);
            let comp = Regex::new(":comp.").unwrap();
            return Some(Self::generate_tokens_with_right_inflected(
                word,
                left_word,
                &right_at,
                "adj",
                None,
                Some(&comp),
            ));
        }

        let right_wd = tagger.tag_either_case(right_word);

        // напівпольської-напіванглійської
        if word.to_lowercase().starts_with("напів") {
            if let Ok(Some(caps)) = NAPIV.captures(word) {
                let g1 = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let g2 = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let left_napiv = adjust(
                    &tagger.tag_as_is_and_with_lowercase(g1),
                    Some("напів"),
                    None,
                    "",
                );
                let right_napiv = if !right_wd.is_empty() {
                    right_wd.clone()
                } else {
                    adjust(
                        &tagger.tag_as_is_and_with_lowercase(g2),
                        Some("напів"),
                        None,
                        "",
                    )
                };
                if !left_napiv.is_empty() && !right_napiv.is_empty() {
                    let left_at = tagger.tagged_to_at(g1, &left_napiv);
                    let right_at = tagger.tagged_to_at(g2, &right_napiv);
                    if let Some(tag_match) = self.tag_match(word, &left_at, &right_at) {
                        return Some(tag_match);
                    }
                }
            }
        }

        let left_at = tagger.tagged_to_at(left_word, &left_wd);

        // гірко-прегірко / гіркий-прегіркий
        if right_word.starts_with("пре") && left_lower == right_word[3..].to_lowercase() {
            if uk_helpers::has_pos_tag_start2(&left_wd, "adv") {
                return Some(
                    left_at
                        .iter()
                        .filter(|a| a.pos_tag.as_deref().is_some_and(|t| t.starts_with("adv")))
                        .map(|a| {
                            AnalyzedToken::new(
                                word,
                                Some(word.to_string()),
                                Some(
                                    TAGS_TO_REMOVE
                                        .replace_all(a.pos_tag.as_deref().unwrap_or(""), "")
                                        .into_owned(),
                                ),
                            )
                        })
                        .collect(),
                );
            } else if uk_helpers::has_pos_tag_start2(&left_wd, "adj") {
                return Some(
                    left_at
                        .iter()
                        .filter(|a| a.pos_tag.as_deref().is_some_and(|t| t.starts_with("adj")))
                        .map(|a| {
                            let lemma = a.stem.as_deref().unwrap_or("");
                            AnalyzedToken::new(
                                word,
                                Some(format!("{lemma}-пре{lemma}")),
                                Some(
                                    TAGS_TO_REMOVE
                                        .replace_all(a.pos_tag.as_deref().unwrap_or(""), "")
                                        .into_owned(),
                                ),
                            )
                        })
                        .collect(),
                );
            }
        }

        // Мустафа-ага
        if NAME_SUFFIX.contains(&right_word)
            && uk_helpers::has_reading_pos_tag_part(&left_at, "name")
        {
            let word_list = adjust(&left_wd, None, Some(&format!("-{right_word}")), "");
            return Some(tagger.tagged_to_at(word, &word_list));
        }

        if left_word == "аль" {
            let wd = format!("Аль-{right_word}");
            let mut wd_list = tagger.word_lookup(&wd);
            if !wd_list.is_empty() {
                wd_list = adjust(&wd_list, None, None, ":bad");
                return Some(tagger.tagged_to_at(&wd, &wd_list));
            }
        }

        if right_wd.is_empty() {
            return None;
        }
        let right_at = tagger.tagged_to_at(right_word, &right_wd);

        // півгодини-годину
        if word.starts_with("пів")
            && uk_helpers::has_reading_pos_tag(
                &left_at,
                &Regex::new(r"^noun:inanim:p:v_...:nv.*$").unwrap(),
            )
        {
            return Some(
                right_at
                    .iter()
                    .filter(|a| {
                        a.pos_tag
                            .as_deref()
                            .is_some_and(|t| t.starts_with("noun:inanim:"))
                    })
                    .map(|a| {
                        AnalyzedToken::new(
                            word,
                            Some(word.to_string()),
                            Some(
                                a.pos_tag
                                    .as_deref()
                                    .unwrap_or("")
                                    .replacen(":[mfn]:", ":p:", 1),
                            ),
                        )
                    })
                    .collect(),
            );
        }

        if left_word.eq_ignore_ascii_case("по") {
            if right_word.ends_with("ому") {
                return Self::po_adv_match(word, &right_at, ADJ_TAG_FOR_PO_ADV_MIS);
            } else if full_match(&SKYI_PATTERN, right_word) {
                return Self::po_adv_match(word, &right_at, ADJ_TAG_FOR_PO_ADV_NAZ);
            }
            return None;
        }

        // Київ-Прага / Хуана-Карлоса / …
        if left_word.chars().next().is_some_and(|c| c.is_uppercase())
            && right_word.chars().next().is_some_and(|c| c.is_uppercase())
        {
            if uk_helpers::has_reading_pos_tag(&left_at, &GEO_V_NAZ)
                && uk_helpers::has_reading_pos_tag(&right_at, &GEO_V_NAZ)
            {
                return Some(vec![AnalyzedToken::new(
                    word,
                    Some(word.to_string()),
                    Some("noninfl:prop:geo".to_string()),
                )]);
            }
            if uk_helpers::has_reading_pos_tag(&left_at, &FNAME)
                && uk_helpers::has_reading_pos_tag(&right_at, &FNAME)
            {
                let left_f = uk_helpers::filter_readings(&left_at, &FNAME);
                let right_f = uk_helpers::filter_readings(&right_at, &FNAME);
                return self.tag_match(word, &left_f, &right_f);
            }
            if uk_helpers::has_reading_pos_tag(&left_at, &LNAME_V_NAZ)
                && uk_helpers::has_reading_pos_tag(&right_at, &LNAME_V_NAZ)
            {
                return Some(vec![AnalyzedToken::new(
                    word,
                    Some(word.to_string()),
                    Some("noninfl:prop:lname".to_string()),
                )]);
            }
            if uk_helpers::has_reading_pos_tag(&left_at, &LNAME_V_ROD)
                && uk_helpers::has_reading_pos_tag(&right_at, &LNAME_V_ROD)
            {
                return Some(vec![AnalyzedToken::new(
                    word,
                    Some(word.to_string()),
                    Some("noninfl:prop:lname".to_string()),
                )]);
            }
            if uk_helpers::has_reading_pos_tag(&left_at, &NAME)
                && uk_helpers::has_reading_pos_tag(&right_at, &NAME)
            {
                return None;
            }
            if uk_helpers::has_reading_pos_tag(&left_at, &PROP_V_NAZ)
                && uk_helpers::has_reading_pos_tag(&right_at, &PROP_V_NAZ)
            {
                return Some(vec![AnalyzedToken::new(
                    word,
                    Some(word.to_string()),
                    Some("noninfl:prop".to_string()),
                )]);
            }
        }

        // був-би, but not м-б
        if left_word.chars().count() > 1 && BAD_SUFFIX.contains(&right_word) {
            // Java reassigns to `addIfNotContains(leftWdList, ":bad")` here.
            let word_list: Vec<(String, String)> = left_wd
                .iter()
                .map(|(lemma, tag)| (lemma.clone(), uk_helpers::add_if_not_contains(tag, ":bad")))
                .collect();
            return Some(tagger.tagged_to_at(word, &word_list));
        }

        if left_word.eq_ignore_ascii_case(right_word)
            && !left_at.is_empty()
            && uk_helpers::has_lemma_regex(&left_at, &ALL_VES)
        {
            if let Some(tag_match) = self.tag_match(word, &left_at, &right_at) {
                return Some(
                    tag_match
                        .into_iter()
                        .filter(|m| m.stem.as_deref().is_some_and(equal_parts))
                        .collect(),
                );
            }
        }

        if uk_helpers::has_reading_pos_tag_part(&left_at, "pron")
            && !uk_helpers::has_reading_pos_tag_part(&left_at, "numr")
        {
            return None;
        }

        if !left_word.eq_ignore_ascii_case(right_word)
            && uk_helpers::has_reading_pos_tag(&right_at, &PART_CONJ_PRON)
            && !(uk_helpers::has_reading_pos_tag_start(&left_at, "numr")
                && uk_helpers::has_reading_pos_tag_start(&right_at, "numr"))
        {
            return None;
        }

        let mut adj_compounds = Vec::new();
        if full_match(&SINGLE_LAT, left_word)
            && uk_helpers::has_reading_pos_tag(&right_at, &ADJ_NO_BAD)
        {
            let comp = Regex::new(":comp.").unwrap();
            adj_compounds = Self::generate_tokens_with_right_inflected(
                word,
                left_word,
                &right_at,
                "adj",
                None,
                Some(&comp),
            );
        }

        // майстер-класу
        if dash_prefix_match
            && !(left_word.eq_ignore_ascii_case("міді")
                && uk_helpers::has_lemma(&right_at, &["бронза"]))
        {
            let mut new_tokens = Vec::new();
            let mut extra_tag = String::new();
            let mut lower_cased = false;
            if let Some(v) = self.dash_prefixes.get(left_word) {
                extra_tag = v.clone();
            } else if let Some(v) = self.dash_prefixes.get(&left_lower) {
                extra_tag = v.clone();
                if full_match(&UKR_LOWER, &left_lower) {
                    lower_cased = true;
                }
            }
            let prefix_word = if lower_cased {
                left_lower.as_str()
            } else {
                left_word
            };
            if let Some(noun_tokens) =
                Self::get_nv_prefix_noun_match(tagger, word, &right_at, prefix_word, &extra_tag)
            {
                new_tokens.extend(noun_tokens);
            }
            if left_word.eq_ignore_ascii_case("топ")
                && uk_helpers::has_reading_pos_tag_part(&right_at, "numr:")
            {
                return Some(Self::generate_tokens_with_right_inflected(
                    word,
                    left_word,
                    &right_at,
                    "numr:",
                    Some(":bad"),
                    None,
                ));
            }
            if new_tokens.is_empty() {
                new_tokens.extend(adj_compounds);
            }
            return Some(new_tokens);
        }

        if !adj_compounds.is_empty() {
            return Some(adj_compounds);
        }

        // пів-України
        if right_word.chars().next().is_some_and(|c| c.is_uppercase()) {
            if word.starts_with("пів-") {
                return Some(Self::add_plural_nv_tokens(word, &right_at, ":alt"));
            }
            let capitalized = right_word.chars().next().is_some_and(|c| c.is_uppercase())
                && right_word.chars().skip(1).all(|c| c.is_lowercase());
            if capitalized
                || left_word.ends_with('о')
                || uk_helpers::has_reading_pos_tag(&right_at, &ADJ_ANY)
            {
                let lower_compound = tagger.tag_as_is_and_with_lowercase(&word.to_lowercase());
                if uk_helpers::has_pos_tag2(&lower_compound, &ADJ_BAD) {
                    return Some(tagger.tagged_to_at(word, &lower_compound));
                }
                let right_wd2 = tagger.tag_as_is_and_with_lowercase(right_word);
                let right_at2 = tagger.tagged_to_at(right_word, &right_wd2);
                if let Some(m) = self.try_o_with_adj(tagger, word, left_word, &right_at2) {
                    return Some(m);
                }
            }
            if !(uk_helpers::has_reading_pos_tag(&left_at, &NOUN_NOT_PROP)
                && uk_helpers::has_reading_pos_tag(&right_at, &NOUN_NOT_PROP))
            {
                return None;
            }
        }

        // don't allow: Донець-кий, зовнішньо-економічний, мас-штаби
        let has_intj = uk_helpers::has_reading_pos_tag_start(&left_at, "intj");
        let mut no_dash_at = Vec::new();
        if !has_intj {
            let no_dash_word = word.replace('-', "");
            let no_dash_list = tagger.tag_as_is_and_with_lowercase(&no_dash_word);
            no_dash_at = tagger.tagged_to_at(&no_dash_word, &no_dash_list);
        }

        // вгору-вниз, лікар-гомеопат, жило-було
        if no_dash_at.is_empty()
            && !left_wd.is_empty()
            && (left_word.chars().count() > 2 || has_intj)
        {
            if let Some(tag_match) = self.tag_match(word, &left_at, &right_at) {
                return Some(tag_match);
            }
        }

        if let Some(m) = self.try_o_with_adj(tagger, word, left_word, &right_at) {
            return Some(m);
        }

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

const TAG_ANIM: &str = ":anim";
const TAG_INANIM: &str = ":inanim";
const ADJ_TAG_FOR_PO_ADV_MIS: &str = "adj:m:v_mis";
const ADJ_TAG_FOR_PO_ADV_NAZ: &str = "adj:m:v_naz";

static EXTRA_TAGS_DROP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":(comp.|np|ns|slang|xp[1-9]|predic|insert)").unwrap());
static EXTRA_TAGS_DROP_NONINFL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":(comp.|np|ns|slang|xp[1-9]|insert)").unwrap());
static NOUN_SING_V_ROD_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noun.*?:[mfn]:v_rod.*$").unwrap());
static SING_REGEX_F: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":[mfn]:").unwrap());
static O_ADJ_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^.+?(о|[чшщ]е)$").unwrap());
static NUMR_ADJ_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.+?(одно|дво|ох|и)$").unwrap());
static INTJ_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^intj.*$").unwrap());
static NONINFL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noninfl.*(onomat|predic).*$").unwrap());
static UKR_LETTERS_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[А-ЯІЇЄҐа-яіїєґ'-]+$").unwrap());
static GEO_V_NAZ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noun:inanim:.:v_naz.*:geo.*$").unwrap());
static FNAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^noun:anim:[mf].*fname.*$").unwrap());
static LNAME_V_NAZ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noun:anim:[fm]:v_naz.*lname.*$").unwrap());
static LNAME_V_ROD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noun:anim:[fm]:v_rod.*lname.*$").unwrap());
static NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^noun:anim:.*name.*$").unwrap());
static PROP_V_NAZ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noun:inanim:.:v_naz.*prop.*$").unwrap());
static MNP_NAZ_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*?:[mnp]:v_naz.*$").unwrap());
static MNP_ZNA_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*?:[mnp]:v_zna.*$").unwrap());
static MNP_ROD_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*?:[mnp]:v_rod.*$").unwrap());
static STD_NOUN_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^noun:(?:in)?anim:(.):(v_...).*$").unwrap());
static PREFIX_NO_DASH_POSTAG_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:noun|adj|adv)(?!.*pron).*$").unwrap());
static SKY_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^.*[сзц]ьки$").unwrap());
static SKYI_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^.*[сзц]ький$").unwrap());
static ABBR_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^.*abbr.*$").unwrap());
static STRETCH_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([а-іяїєґА-ЯІЇЄҐ])\1*-\1+").unwrap());
static TAGS_TO_REMOVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":comp.|:predic|:insert").unwrap());
static NAPIV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^напів(.+?)-напів(.+)$").unwrap());
static SINGLE_LAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[А-ЯІЇЄҐa-zA-Zα-ωΑ-Ω]|[a-zA-Z-]+)$").unwrap());
static UKR_LOWER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[а-яіїєґ']+$").unwrap());
static PART_CONJ_PRON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:part|conj).*|.*?:pron.*$").unwrap());
static ADJ_NO_BAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^adj(?!.*(pron|bad|slang|arch)).*$").unwrap());
static NOUN_NOT_PROP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^noun(?!.prop).*$").unwrap());
static ADJ_ANY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^adj.*$").unwrap());
static ADJ_BAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^adj.*bad$").unwrap());
static GEO_LEMMA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?iu)^(?:ріка|гора|місто|град|поле|море|парк)$").unwrap());
static ALL_VES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[ув]?весь|[ву]с[еі])$").unwrap());
static NAME_SUFFIX: &[&str] = &[
    "ага",
    "ефенді",
    "бек",
    "заде",
    "огли",
    "сан",
    "кизи",
    "сенсей",
];
static BAD_SUFFIX: &[&str] = &["б", "би", "ж", "же"];
static LEFT_O_ADJ: &[&str] = &[
    "австро",
    "адиго",
    "американо",
    "англо",
    "афро",
    "еко",
    "індо",
    "іспано",
    "італо",
    "історико",
    "києво",
    "марокано",
    "угро",
    "японо",
    "румуно",
];
static LEFT_O_ADJ_INVALID: &[&str] = &[
    "багато",
    "мало",
    "високо",
    "низько",
    "старо",
    "важко",
    "зовнішньо",
    "внутрішньо",
    "ново",
    "середньо",
    "південно",
    "північно",
    "західно",
    "східно",
    "центрально",
    "ранньо",
    "пізньо",
];

static RIGHT_BO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:verb|.*?pron|noun|adv|intj|part).*$").unwrap());
static RIGHT_NO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(?:verb(?!.*bad).*?:(?:impr|futr|insert))|intj|adv|part|conj).*$").unwrap()
});
static RIGHT_OT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.*?pron|adv|part|verb).*$").unwrap());
static RIGHT_TO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.*?pron|verb|noun|adj|adv|conj).*$").unwrap());
static RIGHT_TAKY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:verb|adv|adj|.*?pron|part|noninfl:predic).*$").unwrap());
static RIGHT_YEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^verb.*$").unwrap());

fn right_parts_with_left_tag(re: &str) -> Option<&'static Regex> {
    Some(match re {
        "бо" => &RIGHT_BO,
        "но" => &RIGHT_NO,
        "от" => &RIGHT_OT,
        "то" => &RIGHT_TO,
        "таки" => &RIGHT_TAKY,
        "єм" => &RIGHT_YEM,
        _ => return None,
    })
}

/// `CompoundTagger.tagBothCases(leftWord, posTagMatcher)`.
fn tag_both_cases(
    tagger: &UkrainianTagger,
    left_word: &str,
    pos_tag_matcher: Option<&Regex>,
) -> Vec<(String, String)> {
    let mut out = tagger.word_lookup(left_word);
    let lower = left_word.to_lowercase();
    if left_word != lower {
        out.extend(tagger.word_lookup(&lower));
    } else {
        let upper = capitalize(left_word);
        if left_word != upper {
            out.extend(tagger.word_lookup(&upper));
        }
    }
    if let Some(matcher) = pos_tag_matcher {
        out.retain(|(_, tag)| matcher.is_match(tag).unwrap_or(false));
    }
    out
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn drop_extra(postags: &str) -> String {
    let re = if postags.starts_with("noninfl") {
        &EXTRA_TAGS_DROP_NONINFL
    } else {
        &EXTRA_TAGS_DROP
    };
    re.replace_all(postags, "").into_owned()
}

fn strip_perf_imperf(pos_tag: &str) -> String {
    let re = Regex::new(r":(im)?perf|:adjp:(actv|pasv)").unwrap();
    re.replace_all(pos_tag, "").into_owned()
}

fn is_same_anim_status(left: &str, right: &str) -> bool {
    left.contains(TAG_ANIM) == right.contains(TAG_ANIM)
}

fn is_plural(pos_tag: &str) -> bool {
    pos_tag.starts_with("noun:") && pos_tag.contains(":p:")
}

fn equal_parts(lemma: &str) -> bool {
    if !lemma.contains('-') {
        return false;
    }
    let mut parts = lemma.splitn(2, '-');
    let a = parts.next().unwrap_or("");
    let b = parts.next().unwrap_or("");
    a == b
}

impl CompoundTagger {
    fn get_num_agreed_pos_tag(&self, left: &str, right: &str) -> Option<String> {
        if (left.contains(":p:") && SING_REGEX_F.is_match(right).unwrap_or(false))
            || (SING_REGEX_F.is_match(left).unwrap_or(false) && right.contains(":p:"))
        {
            let left_conj = uk_helpers::get_conj(left);
            if left_conj.is_some() && left_conj == uk_helpers::get_conj(right) {
                return Some(left.to_string());
            }
        }
        None
    }

    fn get_agreed_pos_tag(
        &self,
        left: &str,
        right: &str,
        left_nv: bool,
        word: &str,
    ) -> Option<String> {
        if is_plural(left) != is_plural(right) {
            return None;
        }
        if !is_same_anim_status(left, right) {
            return None;
        }
        if let Ok(Some(l)) = STD_NOUN_TAG_REGEX.captures(left) {
            if let Ok(Some(r)) = STD_NOUN_TAG_REGEX.captures(right) {
                let g1 = l.get(2).map(|m| m.as_str()).unwrap_or("");
                let g2 = r.get(2).map(|m| m.as_str()).unwrap_or("");
                if g1 == g2 {
                    if l.get(1).map(|m| m.as_str()) != r.get(1).map(|m| m.as_str())
                        && word.chars().count() < 10
                    {
                        return None;
                    }
                    return Some(if left_nv {
                        right.to_string()
                    } else {
                        left.to_string()
                    });
                }
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn try_anim_inanim(
        &self,
        left_pos: &str,
        right_pos: &str,
        left_lemma: &str,
        right_lemma: &str,
        left_nv: bool,
        right_nv: bool,
        word: &str,
    ) -> Option<String> {
        let mut right_pos = right_pos.to_string();
        let mut left_pos = left_pos.to_string();
        if self.left_master_set.contains(left_lemma) {
            if left_pos.contains(TAG_ANIM) {
                right_pos = right_pos.replace(TAG_INANIM, TAG_ANIM);
            } else {
                right_pos = right_pos.replace(TAG_ANIM, TAG_INANIM);
            }
            let agreed = self.get_agreed_pos_tag(&left_pos, &right_pos, left_nv, word);
            if agreed.is_some() {
                return agreed;
            }
            if !left_pos.contains(TAG_ANIM) {
                if MNP_ZNA_REGEX.is_match(&left_pos).unwrap_or(false)
                    && MNP_NAZ_REGEX.is_match(&right_pos).unwrap_or(false)
                    && !left_nv
                    && !right_nv
                {
                    return Some(left_pos);
                }
            } else if MNP_ZNA_REGEX.is_match(&left_pos).unwrap_or(false)
                && MNP_ROD_REGEX.is_match(&right_pos).unwrap_or(false)
                && !left_nv
                && !right_nv
            {
                return Some(left_pos);
            }
        } else if self.follower_set.contains(right_lemma) {
            right_pos = right_pos.replace(TAG_ANIM, TAG_INANIM);
            let agreed = self.get_agreed_pos_tag(&left_pos, &right_pos, false, word);
            if agreed.is_some() {
                return agreed;
            }
            if left_pos.contains(TAG_INANIM)
                && MNP_ZNA_REGEX.is_match(&left_pos).unwrap_or(false)
                && MNP_NAZ_REGEX.is_match(&right_pos).unwrap_or(false)
                && uk_helpers::get_num(&left_pos) == uk_helpers::get_num(&right_pos)
                && !left_nv
                && !right_nv
            {
                return Some(left_pos);
            }
        } else if self.follower_set.contains(left_lemma) {
            left_pos = left_pos.replace(TAG_ANIM, TAG_INANIM);
            let agreed = self.get_agreed_pos_tag(&right_pos, &left_pos, false, word);
            if agreed.is_some() {
                return agreed;
            }
            if right_pos.contains(TAG_INANIM)
                && MNP_ZNA_REGEX.is_match(&right_pos).unwrap_or(false)
                && MNP_NAZ_REGEX.is_match(&left_pos).unwrap_or(false)
                && uk_helpers::get_num(&left_pos) == uk_helpers::get_num(&right_pos)
                && !left_nv
                && !right_nv
            {
                return Some(right_pos);
            }
        }
        None
    }

    fn is_junior_senior(left: &AnalyzedToken, right: &AnalyzedToken) -> bool {
        let re = Regex::new(r"^.*?:[flp]name.*$").unwrap();
        let re2 = Regex::new(r".*(молодший|старший)").unwrap();
        left.pos_tag
            .as_deref()
            .is_some_and(|t| re.is_match(t).unwrap_or(false))
            && right
                .stem
                .as_deref()
                .is_some_and(|l| re2.is_match(l).unwrap_or(false))
    }

    /// `CompoundTagger.tagMatch`.
    fn tag_match(
        &self,
        word: &str,
        left_tokens: &[AnalyzedToken],
        right_tokens: &[AnalyzedToken],
    ) -> Option<Vec<AnalyzedToken>> {
        let mut new_tokens: Vec<AnalyzedToken> = Vec::new();
        let mut anim_inanim: Vec<AnalyzedToken> = Vec::new();
        let mut anim_inanim_not_tagged = false;

        for left in left_tokens {
            let left_pos = match left.pos_tag.as_deref() {
                Some(t) if !t.contains("abbr") => t,
                _ => continue,
            };
            if left_pos.starts_with("noun:inanim") && left_pos.contains("v_kly") {
                continue;
            }
            let mut left_nv = false;
            let mut left_pos = left_pos.to_string();
            if left_pos.contains(":nv") {
                left_nv = true;
                left_pos = left_pos.replace(":nv", "");
            }
            left_pos = drop_extra(&left_pos);
            let mut left_extra = String::new();
            if left_pos.contains(":bad") {
                left_extra.push_str(":bad");
                left_pos = left_pos.replace(":bad", "");
            }
            for right in right_tokens {
                let right_pos = match right.pos_tag.as_deref() {
                    Some(t) if !t.contains("abbr") && !t.contains("v_zna:var") => t,
                    _ => continue,
                };
                let mut right_pos = right_pos.to_string();
                if right_pos.starts_with("noun:inanim") {
                    if right_pos.contains("v_kly") {
                        continue;
                    }
                    if left_pos.contains(":geo")
                        && !right_pos.contains(":geo")
                        && !right
                            .stem
                            .as_deref()
                            .is_some_and(|l| GEO_LEMMA.is_match(l).unwrap_or(false))
                    {
                        continue;
                    }
                }
                if right_pos.starts_with("noun:anim:p:v_zna:rare")
                    && left_pos.starts_with("noun:inanim")
                {
                    continue;
                }
                let mut extra_nv = String::new();
                let mut right_nv = false;
                if right_pos.contains(":nv") {
                    right_nv = true;
                    if left_nv {
                        extra_nv.push_str(":nv");
                    }
                }
                right_pos = drop_extra(&right_pos);
                if right_pos.contains(":bad") {
                    right_pos = right_pos.replace(":bad", "");
                }
                let same = strip_perf_imperf(&left_pos) == strip_perf_imperf(&right_pos);
                let starts_ok = left_pos.starts_with("numr")
                    || left_pos.starts_with("adv")
                    || left_pos.starts_with("adj")
                    || left_pos.starts_with("verb");
                let intj_ok = (left_pos.starts_with("intj") || left_pos.starts_with("noninfl"))
                    && left
                        .stem
                        .as_deref()
                        .zip(right.stem.as_deref())
                        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b));
                if same && (starts_ok || intj_ok) {
                    let mut new_pos = format!("{left_pos}{extra_nv}{left_extra}");
                    if left_pos.contains("adjp") != right_pos.contains("adjp") {
                        let re = Regex::new(r":adjp:(actv|pasv):(im)?perf").unwrap();
                        new_pos = re.replace(&new_pos, "").into_owned();
                    }
                    new_tokens.push(AnalyzedToken::new(
                        word,
                        Some(format!(
                            "{}-{}",
                            left.stem.as_deref().unwrap_or(""),
                            right.stem.as_deref().unwrap_or("")
                        )),
                        Some(new_pos),
                    ));
                } else if left_pos.starts_with("noun") && right_pos.starts_with("noun") {
                    let mut agreed = self.get_agreed_pos_tag(&left_pos, &right_pos, left_nv, word);
                    if agreed.is_none()
                        && right_pos.starts_with("noun:inanim:m:v_naz")
                        && right.stem.as_deref().is_some_and(is_min_max)
                    {
                        agreed = Some(left_pos.clone());
                    }
                    if agreed.is_none() && !is_same_anim_status(&left_pos, &right_pos) {
                        agreed = self.try_anim_inanim(
                            &left_pos,
                            &right_pos,
                            left.stem.as_deref().unwrap_or(""),
                            right.stem.as_deref().unwrap_or(""),
                            left_nv,
                            right_nv,
                            word,
                        );
                        match agreed {
                            Some(agreed) => {
                                anim_inanim.push(AnalyzedToken::new(
                                    word,
                                    Some(format!(
                                        "{}-{}",
                                        left.stem.as_deref().unwrap_or(""),
                                        right.stem.as_deref().unwrap_or("")
                                    )),
                                    Some(format!("{agreed}{extra_nv}{left_extra}")),
                                ));
                                continue;
                            }
                            None => anim_inanim_not_tagged = true,
                        }
                    }
                    if let Some(agreed) = agreed {
                        new_tokens.push(AnalyzedToken::new(
                            word,
                            Some(format!(
                                "{}-{}",
                                left.stem.as_deref().unwrap_or(""),
                                right.stem.as_deref().unwrap_or("")
                            )),
                            Some(format!("{agreed}{extra_nv}{left_extra}")),
                        ));
                    }
                } else if left_pos.starts_with("numr") && right_pos.starts_with("numr") {
                    if let Some(agreed) = self.get_num_agreed_pos_tag(&left_pos, &right_pos) {
                        let agreed = if right_pos.contains(":p:") && !agreed.contains(":p:") {
                            let re = Regex::new(":[mfn]:").unwrap();
                            re.replace(&agreed, ":p:").into_owned()
                        } else {
                            agreed
                        };
                        new_tokens.push(AnalyzedToken::new(
                            word,
                            Some(format!(
                                "{}-{}",
                                left.stem.as_deref().unwrap_or(""),
                                right.stem.as_deref().unwrap_or("")
                            )),
                            Some(format!("{agreed}{extra_nv}{left_extra}")),
                        ));
                    }
                } else if left_pos.starts_with("noun") && right_pos.starts_with("numr") {
                    if left.stem.as_deref() != Some("п'ята") {
                        let lgc = uk_helpers::get_gender_conj(&left_pos);
                        if lgc.is_some() && lgc == uk_helpers::get_gender_conj(&right_pos) {
                            let lemma = format!(
                                "{}-{}",
                                left.stem.as_deref().unwrap_or(""),
                                right.stem.as_deref().unwrap_or("")
                            );
                            new_tokens.push(AnalyzedToken::new(
                                word,
                                Some(lemma.clone()),
                                Some(format!("{left_pos}{extra_nv}{left_extra}")),
                            ));
                            if !left_pos.contains(":p:") {
                                new_tokens.push(AnalyzedToken::new(
                                    word,
                                    Some(lemma),
                                    Some(format!(
                                        "{}{extra_nv}{left_extra}",
                                        Regex::new(":[mfn]:").unwrap().replace(&left_pos, ":p:")
                                    )),
                                ));
                            }
                        } else if let Some(agreed) =
                            self.get_num_agreed_pos_tag(&left_pos, &right_pos)
                        {
                            let lemma = format!(
                                "{}-{}",
                                left.stem.as_deref().unwrap_or(""),
                                right.stem.as_deref().unwrap_or("")
                            );
                            new_tokens.push(AnalyzedToken::new(
                                word,
                                Some(lemma.clone()),
                                Some(format!("{agreed}{extra_nv}{left_extra}")),
                            ));
                            if !agreed.contains(":p:") {
                                new_tokens.push(AnalyzedToken::new(
                                    word,
                                    Some(lemma),
                                    Some(format!(
                                        "{}{extra_nv}{left_extra}",
                                        Regex::new(":[mfn]:").unwrap().replace(&agreed, ":p:")
                                    )),
                                ));
                            }
                        }
                    }
                } else if (left_pos.starts_with("noun") && right_pos.starts_with("numr"))
                    || (right_pos.starts_with("adj") && Self::is_junior_senior(left, right))
                {
                    let lgc = uk_helpers::get_gender_conj(&left_pos);
                    if lgc.is_some() && lgc == uk_helpers::get_gender_conj(&right_pos) {
                        new_tokens.push(AnalyzedToken::new(
                            word,
                            Some(format!(
                                "{}-{}",
                                left.stem.as_deref().unwrap_or(""),
                                right.stem.as_deref().unwrap_or("")
                            )),
                            Some(format!("{left_pos}{extra_nv}{left_extra}")),
                        ));
                    }
                } else if left_pos.starts_with("noun") && right.stem.as_deref() == Some("другий")
                {
                    let lgc = uk_helpers::get_gender_conj(&left_pos);
                    if let Some(lgc) =
                        lgc.filter(|l| Some(l) == uk_helpers::get_gender_conj(&right_pos).as_ref())
                    {
                        let right_lemma = if lgc.starts_with('m') {
                            "другий"
                        } else if lgc.starts_with('f') {
                            "друга"
                        } else {
                            "друге"
                        };
                        new_tokens.push(AnalyzedToken::new(
                            word,
                            Some(format!(
                                "{}-{right_lemma}",
                                left.stem.as_deref().unwrap_or("")
                            )),
                            Some(format!("{left_pos}{extra_nv}{left_extra}")),
                        ));
                    }
                }
            }
        }

        if !new_tokens.is_empty() && !uk_helpers::has_reading_pos_tag_part(&new_tokens, ":p:") {
            let days = uk_helpers::has_lemma(left_tokens, uk_helpers::DAYS_OF_WEEK)
                && uk_helpers::has_lemma(right_tokens, uk_helpers::DAYS_OF_WEEK);
            let months = uk_helpers::has_lemma(left_tokens, uk_helpers::MONTH_LEMMAS)
                && uk_helpers::has_lemma(right_tokens, uk_helpers::MONTH_LEMMAS);
            if days || months {
                let re = Regex::new(r":[mfn]:").unwrap();
                let first = new_tokens[0].clone();
                new_tokens.push(AnalyzedToken::new(
                    word,
                    first.stem.clone(),
                    Some(
                        re.replace_all(first.pos_tag.as_deref().unwrap_or(""), ":p:")
                            .into_owned(),
                    ),
                ));
            }
        }

        // remove duplicates (LinkedHashSet preserves order)
        let mut deduped: Vec<AnalyzedToken> = Vec::new();
        for t in new_tokens {
            if !deduped.contains(&t) {
                deduped.push(t);
            }
        }
        let mut result = deduped;
        if result.is_empty() {
            result = anim_inanim;
        }
        let _ = anim_inanim_not_tagged;
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    fn o_to_yj(left_word: &str) -> String {
        let chars: Vec<char> = left_word.chars().collect();
        if left_word.ends_with("ьо") {
            let base: String = chars[..chars.len() - 2].iter().collect();
            format!("{base}ій")
        } else {
            let base: String = chars[..chars.len() - 1].iter().collect();
            format!("{base}ий")
        }
    }

    /// `CompoundTagger.oAdjMatch`.
    fn o_adj_match(
        &self,
        tagger: &UkrainianTagger,
        word: &str,
        analyzed_tokens: &[AnalyzedToken],
        left_word: &str,
    ) -> Option<Vec<AnalyzedToken>> {
        let left_chars: Vec<char> = left_word.chars().collect();
        let left_base: String = left_chars[..left_chars.len() - 1].iter().collect();
        let mut extra_tag = String::new();
        if !LEFT_O_ADJ.contains(&left_word.to_lowercase().as_str()) {
            let adv_re = Regex::new(r"^(?:adv.*|.*?numr.*)$").unwrap();
            let mut tagged = tag_both_cases(tagger, left_word, Some(&adv_re));
            if tagged.is_empty() {
                let adj_re = ADJ_ANY.clone();
                tagged = tag_both_cases(tagger, &Self::o_to_yj(left_word), Some(&adj_re));
            }
            if tagged.is_empty() && left_word.chars().count() > 4 {
                let noun_re = Regex::new(r"^noun.*$").unwrap();
                tagged = tag_both_cases(tagger, &left_base, Some(&noun_re));
            }
            if tagged.is_empty() {
                let base_a = format!("{left_base}а");
                let re = Regex::new(r"^(?:noun:inanim:f:v_naz|numr).*$").unwrap();
                tagged = tag_both_cases(tagger, &base_a, Some(&re));
            }
            if tagged.is_empty() {
                return None;
            }
            let all_cap = tagger.analyze_all_capitalized_adj(word);
            let first_tag = tagged[0].1.clone();
            if (first_tag.starts_with("adv")
                && uk_helpers::has_reading_pos_tag_part(analyzed_tokens, "adjp"))
                || uk_helpers::has_pos_tag_part2(&tagged, ":bad")
            {
                extra_tag = ":bad".into();
            } else if LEFT_O_ADJ_INVALID.contains(&left_word.to_lowercase().as_str()) {
                if all_cap.is_empty() {
                    extra_tag = ":bad".into();
                }
            } else if all_cap.is_empty() {
                let no_dash = word.replace('-', "");
                let no_dash_tagged = tagger.tag_as_is_and_with_lowercase(&no_dash);
                if !no_dash_tagged.is_empty() {
                    extra_tag = ":bad".into();
                }
            }
        }
        let mut out = Vec::new();
        for at in analyzed_tokens {
            let pos_tag = at.pos_tag.as_deref().unwrap_or("");
            if pos_tag.starts_with("adj") {
                let mut pos_tag = if pos_tag.contains(":comp") {
                    uk_helpers::adj_comp_regex()
                        .replace(pos_tag, "")
                        .into_owned()
                } else {
                    pos_tag.to_string()
                };
                if extra_tag.contains(":bad") {
                    pos_tag = pos_tag.replace(":arch", "");
                }
                pos_tag = uk_helpers::add_if_not_contains(&pos_tag, &extra_tag);
                out.push(AnalyzedToken::new(
                    word,
                    Some(format!(
                        "{}-{}",
                        left_word.to_lowercase(),
                        at.stem.as_deref().unwrap_or("")
                    )),
                    Some(pos_tag),
                ));
            }
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    /// `CompoundTagger.numrAdjMatch`.
    fn numr_adj_match(
        &self,
        tagger: &UkrainianTagger,
        word: &str,
        analyzed_tokens: &[AnalyzedToken],
        left_word: &str,
    ) -> Option<Vec<AnalyzedToken>> {
        let tagged = tagger.word_lookup(left_word);
        if !uk_helpers::has_pos_tag_start2(&tagged, "numr") {
            return None;
        }
        let left_lower = left_word.to_lowercase();
        let mut extra_tag = "";
        if left_lower.ends_with("двох")
            || left_lower.ends_with("трьох")
            || left_lower.ends_with("чотирьох")
        {
            extra_tag = ":bad";
        } else if let Some(first) = analyzed_tokens.first() {
            let re = Regex::new(
                r"(?iu)^(?:дво|три|чотири|п'яти|шести|семи|вісьми|двох|трьох|чотирьох).+$",
            )
            .unwrap();
            if !first.token.is_empty() && !re.is_match(&first.token).unwrap_or(false) {
                extra_tag = ":bad";
            }
        }
        let mut out = Vec::new();
        for at in analyzed_tokens {
            let pos_tag = at.pos_tag.as_deref().unwrap_or("");
            if pos_tag.starts_with("adj") {
                let mut pos_tag = if pos_tag.contains(":comp") {
                    uk_helpers::adj_comp_regex()
                        .replace(pos_tag, "")
                        .into_owned()
                } else {
                    pos_tag.to_string()
                };
                if !pos_tag.contains(":bad") {
                    pos_tag.push_str(extra_tag);
                }
                out.push(AnalyzedToken::new(
                    word,
                    Some(format!("{left_lower}-{}", at.stem.as_deref().unwrap_or(""))),
                    Some(pos_tag),
                ));
            }
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    fn try_o_with_adj(
        &self,
        tagger: &UkrainianTagger,
        word: &str,
        left_word: &str,
        right_tokens: &[AnalyzedToken],
    ) -> Option<Vec<AnalyzedToken>> {
        if left_word.chars().count() < 3 {
            return None;
        }
        if full_match(&NUMR_ADJ_PATTERN, left_word) {
            return self.numr_adj_match(tagger, word, right_tokens, left_word);
        }
        if full_match(&O_ADJ_PATTERN, left_word) {
            return self.o_adj_match(tagger, word, right_tokens, left_word);
        }
        None
    }

    fn get_nv_prefix_noun_match(
        tagger: &UkrainianTagger,
        word: &str,
        analyzed_tokens: &[AnalyzedToken],
        left_word: &str,
        extra_tag: &str,
    ) -> Option<Vec<AnalyzedToken>> {
        let mut out = Vec::new();
        for at in analyzed_tokens {
            let pos_tag = at.pos_tag.as_deref().unwrap_or("");
            if pos_tag.starts_with("noun") && !pos_tag.contains("v_kly") {
                let mut pos_tag = pos_tag.to_string();
                let lemma = at.stem.as_deref().unwrap_or("");
                if (!extra_tag.eq(":alt")
                    || !lemma.chars().next().is_some_and(|c| c.is_uppercase()))
                    && !extra_tag.is_empty()
                {
                    pos_tag = uk_helpers::add_if_not_contains(&pos_tag, extra_tag);
                }
                out.push(AnalyzedToken::new(
                    word,
                    Some(format!("{left_word}-{lemma}")),
                    Some(pos_tag),
                ));
            }
        }
        let _ = tagger;
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    fn po_adv_match(
        word: &str,
        analyzed_tokens: &[AnalyzedToken],
        adj_tag: &str,
    ) -> Option<Vec<AnalyzedToken>> {
        for at in analyzed_tokens {
            if at
                .pos_tag
                .as_deref()
                .is_some_and(|t| t.starts_with(adj_tag))
            {
                return Some(vec![AnalyzedToken::new(
                    word,
                    Some(word.to_string()),
                    Some("adv".to_string()),
                )]);
            }
        }
        None
    }

    fn generate_tokens_with_right_inflected(
        word: &str,
        left_word: &str,
        right_tokens: &[AnalyzedToken],
        pos_tag_start: &str,
        add_tag: Option<&str>,
        drop_tag: Option<&Regex>,
    ) -> Vec<AnalyzedToken> {
        let mut out = Vec::new();
        for at in right_tokens {
            let pos_tag = at.pos_tag.as_deref().unwrap_or("");
            if pos_tag.starts_with(pos_tag_start) && !pos_tag.contains("v_kly") {
                let mut pos_tag = match drop_tag {
                    Some(re) => re.replace_all(pos_tag, "").into_owned(),
                    None => pos_tag.to_string(),
                };
                if let Some(add) = add_tag {
                    pos_tag = uk_helpers::add_if_not_contains(&pos_tag, add);
                }
                out.push(AnalyzedToken::new(
                    word,
                    Some(format!("{left_word}-{}", at.stem.as_deref().unwrap_or(""))),
                    Some(pos_tag),
                ));
            }
        }
        out
    }

    /// `CompoundTagger.addPluralNvTokens`.
    fn add_plural_nv_tokens(
        word: &str,
        right_tokens: &[AnalyzedToken],
        add_tag: &str,
    ) -> Vec<AnalyzedToken> {
        let mut out: Vec<AnalyzedToken> = Vec::new();
        for at in right_tokens {
            let right_pos = at.pos_tag.as_deref().unwrap_or("");
            if full_match(&NOUN_SING_V_ROD_REGEX, right_pos) {
                for vid in uk_helpers::VIDMINKY {
                    if *vid == "v_kly" {
                        continue;
                    }
                    let step1 = right_pos.replace("v_rod", vid);
                    let re = Regex::new(":[mfn]:v_").unwrap();
                    let pos_tag = re.replace(&step1, ":p:v_").into_owned() + ":nv" + add_tag;
                    let token = AnalyzedToken::new(word, Some(word.to_string()), Some(pos_tag));
                    if !out.contains(&token) {
                        out.push(token);
                    }
                }
            }
        }
        out
    }

    fn collapse_stretch(word: &str) -> String {
        let capitalized = word.chars().next().is_some_and(|c| c.is_uppercase());
        let mut merged = STRETCH_PATTERN
            .replace_all(&word.to_lowercase(), "$1")
            .into_owned();
        merged = STRETCH_PATTERN.replace_all(&merged, "$1").into_owned();
        merged = merged.replace('-', "");
        if capitalized {
            merged = capitalize(&merged);
        }
        merged
    }

    pub(crate) fn guess_other_tags(
        &self,
        tagger: &UkrainianTagger,
        word: &str,
    ) -> Option<Vec<AnalyzedToken>> {
        if word.chars().count() <= 7 || !full_match(&UKR_LETTERS_PATTERN, word) {
            return None;
        }
        if word.chars().next().is_some_and(|c| c.is_uppercase()) {
            if word.ends_with("штрассе") || word.ends_with("штрасе") {
                let add_pos = if word.ends_with("штрассе") {
                    ":alt"
                } else {
                    ""
                };
                return Some(uk_helpers::generate_tokens_for_nv(
                    word,
                    "f",
                    Some(&format!(":prop{add_pos}")),
                ));
            }
            if word.ends_with("дзе") || word.ends_with("швілі") || word.ends_with("іані")
            {
                return Some(uk_helpers::generate_tokens_for_nv(
                    word,
                    "mf",
                    Some(":prop:lname"),
                ));
            }
        }
        let lower = word.to_lowercase();
        let mut prefixes: Vec<&String> = self.no_dash_prefixes.iter().collect();
        prefixes.sort();
        for prefix in prefixes {
            if !lower.starts_with(prefix.as_str()) {
                continue;
            }
            let mut right = word
                .chars()
                .skip(prefix.chars().count())
                .collect::<String>();
            let mut apo = "";
            let mut add_tag: Vec<&str> = Vec::new();
            if right.starts_with('\'') {
                right = right[1..].to_string();
                apo = "'";
            }
            if right.chars().count() < 2 {
                continue;
            }
            let apo_needed = right.chars().next().is_some_and(|c| "єїюя".contains(c))
                && !prefix
                    .chars()
                    .last()
                    .is_some_and(|c| "аеєиіїоуюя".contains(c));
            if !apo_needed && !apo.is_empty() {
                break;
            }
            if apo_needed == apo.is_empty() {
                add_tag.push(":bad");
            }
            if right.chars().count() >= 4 && !right.chars().next().is_some_and(|c| c.is_uppercase())
            {
                let mut right_tagged = tagger.word_lookup(&right);
                right_tagged = uk_helpers::filter2(right_tagged, &PREFIX_NO_DASH_POSTAG_PATTERN);
                right_tagged
                    .retain(|(_, tag)| !(tag.starts_with("noun:inanim") && tag.contains("v_kly")));
                if !right_tagged.is_empty() {
                    let add = add_tag.join("");
                    let right_tagged =
                        adjust(&right_tagged, Some(&format!("{prefix}{apo}")), None, &add);
                    return Some(tagger.tagged_to_at(word, &right_tagged));
                }
            }
        }
        None
    }

    fn do_guess_multi_hyphens(
        &self,
        tagger: &UkrainianTagger,
        word: &str,
        _first_dash_idx: usize,
        _dash_idx: usize,
    ) -> Option<Vec<AnalyzedToken>> {
        let lower_word = word.to_lowercase();
        let parts: Vec<&str> = lower_word.split('-').collect();
        let mut set: Vec<String> = Vec::new();
        for p in &parts {
            if !set.iter().any(|s| s == p) {
                set.push(p.to_string());
            }
        }
        if set.len() == 2 {
            let left = tagger.tag_either_case(parts[0]);
            let right = tagger.tag_either_case(&set[1]);
            if (uk_helpers::has_pos_tag2(&left, &INTJ_PATTERN)
                && uk_helpers::has_pos_tag2(&right, &INTJ_PATTERN))
                || (uk_helpers::has_pos_tag2(&left, &NONINFL_PATTERN)
                    && uk_helpers::has_pos_tag2(&right, &NONINFL_PATTERN))
            {
                return Some(vec![AnalyzedToken::new(
                    word,
                    Some(lower_word.clone()),
                    Some(right[0].1.clone()),
                )]);
            }
        } else if set.len() == 1 {
            if lower_word == "ла" {
                return Some(vec![AnalyzedToken::new(
                    word,
                    Some(lower_word),
                    Some("intj".to_string()),
                )]);
            }
            let right = tagger.tag_either_case(parts[0]);
            if uk_helpers::has_pos_tag2(&right, &INTJ_PATTERN) {
                return Some(vec![AnalyzedToken::new(
                    word,
                    Some(lower_word),
                    Some(right[0].1.clone()),
                )]);
            }
        }
        if parts.len() == 3 {
            let tokens = self.generate_entities(word);
            if !tokens.is_empty() {
                return Some(tokens);
            }
        }
        if parts.len() >= 3
            && set.len() > 1
            && !self.dash_prefixes.contains_key(parts[0])
            && !self.dash_prefixes_invalid.contains(parts[0])
        {
            let merged = word.replace('-', "");
            let tagged = tag_both_cases(tagger, &merged, None);
            let tagged = uk_helpers::filter2_negative(tagged, &ABBR_PATTERN);
            if !tagged.is_empty() {
                let tagged: Vec<(String, String)> = tagged
                    .into_iter()
                    .map(|(l, t)| (l, uk_helpers::add_if_not_contains(&t, ":alt")))
                    .collect();
                return Some(tagger.tagged_to_at(word, &tagged));
            }
            let merged = Self::collapse_stretch(word);
            let tagged = tag_both_cases(tagger, &merged, None);
            let tagged = uk_helpers::filter2_negative(tagged, &ABBR_PATTERN);
            if !tagged.is_empty() {
                let tagged: Vec<(String, String)> = tagged
                    .into_iter()
                    .map(|(l, t)| (l, uk_helpers::add_if_not_contains(&t, ":alt")))
                    .collect();
                return Some(tagger.tagged_to_at(word, &tagged));
            }
        }
        None
    }

    fn do_guess_two_hyphens(
        &self,
        tagger: &UkrainianTagger,
        word: &str,
        _first_dash_idx: usize,
        _dash_idx: usize,
    ) -> Option<Vec<AnalyzedToken>> {
        let parts: Vec<&str> = word.split('-').collect();
        if parts.len() < 3 {
            return None;
        }
        let right_wd = tagger.tag_either_case(parts[2]);
        if right_wd.is_empty() {
            return None;
        }
        let right_at = tagger.tagged_to_at(parts[2], &right_wd);
        let first_and_second = format!("{}-{}", parts[0], parts[1]);
        let mut extra_tag = String::new();
        let mut two_dash = false;
        if let Some(v) = self.dash_prefixes.get(&first_and_second) {
            extra_tag = v.clone();
            two_dash = true;
        } else if let Some(v) = self.dash_prefixes.get(&first_and_second.to_lowercase()) {
            extra_tag = v.clone();
            two_dash = true;
        }
        if two_dash {
            return Self::get_nv_prefix_noun_match(
                tagger,
                word,
                &right_at,
                &first_and_second,
                &extra_tag,
            );
        }
        let second_wd = tagger.tag_either_case(parts[1]);
        if uk_helpers::has_pos_tag_start2(&second_wd, "adj") {
            let second_at = tagger.tagged_to_at(parts[1], &second_wd);
            if let Some(tag_match_second_third) = self.tag_match(word, &second_at, &right_at) {
                let left_wd = tagger.tag_either_case(parts[0]);
                if !left_wd.is_empty() {
                    let left_at = tagger.tagged_to_at(parts[0], &left_wd);
                    let _ = self.tag_match(word, &left_at, &tag_match_second_third);
                }
                return Some(tag_match_second_third);
            }
        }
        if let Some(second_and_third) = self.try_o_with_adj(tagger, word, parts[1], &right_at) {
            return self.try_o_with_adj(tagger, word, parts[0], &second_and_third);
        }
        None
    }
}

fn is_min_max(token: &str) -> bool {
    token == "максимум" || token == "мінімум"
}
