//! Nordum (`nrd`) engine tests.
//!
//! Nordum is a constructed pan-Scandinavian written language
//! (<https://www.nordum.org>); there is no legacy Java module, so the tests
//! pin the staged rule wiring and the owner-approved examples from
//! `new-languages/nordum/proposed-rules.md`. Offset probes assert the
//! Java/HTTP-compatible UTF-16 code units with `common::assert_utf16`,
//! pinned from the engine's own output (no Java oracle exists).

use std::sync::{Mutex, MutexGuard, OnceLock};

use lt::{DataDir, Engine, Lang};

mod common;
use common::assert_utf16;

fn engine_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn data_dir() -> Option<DataDir> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    if !path.is_dir() {
        eprintln!("skipping: no vendored data directory found");
        return None;
    }
    Some(DataDir::new(path))
}

fn engine_with_rules(rules: &[&str]) -> Option<Engine> {
    let data = data_dir()?;
    let options = lt::EngineOptions {
        enabled_rules: rules.iter().map(|s| s.to_string()).collect(),
        enabled_only: true,
        ..Default::default()
    };
    Engine::builder(Lang::Nrd)
        .ok()?
        .data_dir(data)
        .options(options)
        .build()
        .ok()
}

fn engine() -> Option<Engine> {
    let data = data_dir()?;
    Engine::builder(Lang::Nrd).ok()?.data_dir(data).build().ok()
}

fn match_ids(engine: &Engine, text: &str) -> Vec<String> {
    engine
        .check(text)
        .map(|r| r.matches.iter().map(|m| m.rule_id.clone()).collect())
        .unwrap_or_default()
}

fn hits(engine: &Engine, text: &str, rule_id: &str) -> bool {
    match_ids(engine, text).iter().any(|id| id == rule_id)
}

/// Stage-3 wiring state: 27 active XML rules, no unmapped filter, no compile
/// failure.
#[test]
fn nordum_engine_state() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert_eq!(engine.active_rule_count(), 27);
    assert!(engine.compile_failures().is_empty());
    let skipped = engine.skipped_counts();
    assert_eq!(skipped.filters, 0);
    assert_eq!(skipped.uncompilable, 0);
}

/// Approved rule examples that must fire with default options.
#[test]
fn nordum_rules_fire() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let cases: &[(&str, &str)] = &[
        ("backa", "NDM_SOURCE_FORMS"),
        ("tack", "NDM_SOURCE_FORMS"),
        ("fiks", "NDM_SOURCE_FORMS"),
        ("hvad", "NDM_SOURCE_FORMS"),
        ("arbeidar", "NDM_MORPHOLOGY"),
        ("jenter", "NDM_MORPHOLOGY"),
        ("flikkan", "NDM_MORPHOLOGY"),
        ("forskjell", "NDM_SOURCE_FORMS"),
        ("halvfjerds", "NDM_NUMERALS"),
        ("mellom", "NDM_PREPOSITIONS"),
        ("datamaskin", "NDM_LOANWORDS"),
        ("Jei vet å hun arbeider.", "NDM_AA_ATT"),
        ("att lære språket", "NDM_ATT_AA"),
        ("Jei ikke arbeider.", "NDM_NEG_MAIN"),
        ("Jei vet att hun arbeider ikke.", "NDM_NEG_SUB"),
        ("Nå jei arbeider.", "NDM_V2"),
        // spec §6/§7 transcription rules added 2026-09-23
        ("I dag jei arbeider jemme.", "NDM_V2_DATE"),
        ("Hvis du kommer blir jei glad.", "NDM_COMMA_FRONTED_SUB"),
        ("Vi har den stor bilen.", "NDM_DEF_ADJ"),
        ("Jei har en bilen.", "NDM_ART_DEF"),
        ("Vi feirer 50årsdag.", "NDM_HYPHEN_NUMBERS_JOINED"),
        ("Vi har ett stor hus.", "NDM_ADJ_NEUTER"),
        ("på Mandag", "NDM_CAPITALIZATION"),
        ("Vi har stor bilar.", "NDM_ADJ_PLURAL"),
        ("Ham arbeider i dag.", "NDM_PRON_SUBJ_HAM"),
        ("Jei snakker med hun.", "NDM_PRON_OBJ_HENNE"),
        ("Jei vil og lære nordum.", "NDM_OG_AA"),
        ("Du arbeider i dag?", "NDM_YN_QUESTION"),
        ("Vad du gør?", "NDM_WH_QUESTION_SHORT"),
        ("Vad du gør i dag?", "NDM_WH_QUESTION"),
        ("Du arbeider?", "NDM_YN_QUESTION_SHORT"),
        ("Spanien", "NDM_ENDONYMS"),
        ("Spania", "NDM_ENDONYMS"),
        ("eg", "NDM_SOURCE_FORMS"),
        ("Jei jei arbeider.", "NDM_WORD_REPETITION"),
        ("barn hage", "NDM_COMPOUND"),
        ("barnehage", "NDM_COMPOUND"),
        ("min hus", "NDM_PRON_POSS"),
        ("mitt jente", "NDM_PRON_POSS"),
        ("vår hus", "NDM_PRON_POSS"),
    ];
    for (text, rule_id) in cases {
        assert!(
            hits(&engine, text, rule_id),
            "expected {rule_id} for {text:?}, got {:?}",
            match_ids(&engine, text)
        );
    }
}

/// UTF-16 offset probes for owner-approved examples. No Java oracle exists,
/// so the expectations pin the engine's own output in the Java/HTTP-compatible
/// UTF-16 format (`common::assert_utf16`).
#[test]
fn nordum_utf16_offsets() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    let cases: &[(&str, &str, (usize, usize))] = &[
        ("på Mandag", "NDM_CAPITALIZATION", (3, 9)),
        ("Jei vet å hun arbeider.", "NDM_AA_ATT", (8, 9)),
        ("Vi har ett stor hus.", "NDM_ADJ_NEUTER", (11, 15)),
    ];
    for (text, rule, expected) in cases {
        let m = engine
            .check(text)
            .unwrap()
            .matches
            .into_iter()
            .find(|m| m.rule_id == *rule)
            .unwrap_or_else(|| panic!("{rule} did not match {text:?}"));
        assert_utf16(text, &m, *expected);
    }
}

/// Correct Nordum sentences must stay clean.
#[test]
fn nordum_correct_sentences() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    for text in [
        "Jei vet att hun arbeider.",
        "Jei arbeider ikke.",
        "Jei vet att hun ikke arbeider.",
        "I dag arbeider jei jemme.",
        "I dag må du gå.",
        "Hvis du kommer, blier jei glad.",
        "Vi har den store bilen.",
        "den vesle bilen",
        "den blå bilen",
        "Jei har en bil.",
        "Jei har bilen.",
        "Det er en orden i saken.",
        "Jei har ett våpen.",
        "Vi feirer 50-årsdag.",
        "Vi har ett stort hus.",
        "Jei snakker norsk.",
        "forskell",
        "flikken",
        "Jei har ett stort hus.",
        "Vi har store bilar.",
        "Han arbeider i dag.",
        "Jei snakker med henne.",
        "Jei vil å lære nordum.",
        "Arbeider du i dag?",
        "Vad gør du?",
        "Vad gør du i dag?",
        "Arbeider du?",
        "España",
        "Det er en bra dag. Jei bakka bilen. Jei har en computer. \
         Mellem huset og skogen arbeider femti personar. Norsk og dansk er bra språk.",
        "nordum",
        "Ja ja, jei kommer.",
        "barnhage",
        "arbeiddag",
        "min bil",
        "mitt hus",
        "vårt hus",
        "din bil",
    ] {
        let ids = match_ids(&engine, text);
        assert!(ids.is_empty(), "unexpected matches for {text:?}: {ids:?}");
    }
}

/// Default-off lexicon-driven split-compound check.
#[test]
fn nordum_split_compound_lex() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    assert!(!hits(&engine, "barn hage", "NDM_COMPOUND_LEX"));
    let Some(enabled) = engine_with_rules(&["NDM_COMPOUND_LEX"]) else {
        return;
    };
    for text in ["barn hage", "skole gård"] {
        assert!(
            hits(&enabled, text, "NDM_COMPOUND_LEX"),
            "expected NDM_COMPOUND_LEX for {text:?}, got {:?}",
            match_ids(&enabled, text)
        );
    }
    for text in ["god morgen", "stor bil", "i dag", "jei har en bil"] {
        assert!(
            !hits(&enabled, text, "NDM_COMPOUND_LEX"),
            "unexpected NDM_COMPOUND_LEX for {text:?}"
        );
    }
}

/// The generated Hunspell dictionary: broad source coverage, authoritative
/// core and suggestions from the core list.
#[test]
fn nordum_speller() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    // `bakka` (the normalized form) is accepted
    assert!(!hits(&engine, "bakka", "NDM_SPELLER"));
    // a nonsense word is flagged, with core-list suggestions
    let result = engine.check("arbeidre").unwrap();
    let spelling: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.rule_id == "NDM_SPELLER")
        .collect();
    assert_eq!(spelling.len(), 1);
    assert!(spelling[0]
        .suggestions
        .iter()
        .any(|s| s.value == "arbeide" || s.value == "arbeider"));
    // the generated dictionary accepts transformed source words
    assert!(!hits(&engine, "computer", "NDM_SPELLER"));
}

/// Owner decision (2026-09-20): the `ks` → `x` rule applies to number words
/// (`seks` → `sex`), and `mykket`/`meget` are accepted forms of `mye`.
#[test]
fn nordum_number_words_and_mye_variants() {
    let _guard = engine_guard();
    let Some(engine) = engine() else {
        return;
    };
    for word in ["mye", "meget", "mykket"] {
        assert!(
            !hits(&engine, word, "NDM_SPELLER"),
            "{word} must be accepted by the speller"
        );
    }
    for word in ["sex", "sexten", "sextende"] {
        assert!(
            !hits(&engine, word, "NDM_SPELLER"),
            "{word} must be accepted by the speller"
        );
    }
    assert!(hits(&engine, "seks", "NDM_SOURCE_FORMS"));
    let result = engine.check("seks").unwrap();
    let source_form = result
        .matches
        .iter()
        .find(|m| m.rule_id == "NDM_SOURCE_FORMS")
        .expect("NDM_SOURCE_FORMS for seks");
    // Sentence-start suggestions are capitalized (`Sex`), so compare
    // case-insensitively.
    assert!(source_form
        .suggestions
        .iter()
        .any(|s| s.value.eq_ignore_ascii_case("sex")));
}

/// The generated Nordum tagger dictionary (`tools/nordum-dict/
/// build-nordum-tagger.py`) tags the core lexicon with the Nordum tagset:
/// nouns, verbs and adjectives get their part-of-speech readings, unknown
/// words stay untagged, and a sentence-final token gains `SENT_END`.
#[test]
fn nordum_tagger_tags_core_lexicon() {
    let _guard = engine_guard();
    if data_dir().is_none() {
        return;
    }
    let tagger = lt_tagger::NrdTagger::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data")
            .as_path(),
    )
    .expect("nrd tagger loads");

    // bakka: converted Danish/Norwegian adjective/verb core word.
    let bakka = tagger.tag_word("bakka");
    assert!(
        bakka.iter().any(|t| t.pos_tag.is_some()),
        "bakka should carry a POS reading, got {bakka:?}"
    );

    // An unknown word gets the untagged fallback reading.
    let unknown = tagger.tag_word("qwartzl");
    assert!(unknown.iter().all(|t| t.pos_tag.is_none()));

    // Sentence tokens are tagged through the same dictionary.
    let tagged = tagger.tag(&["Han".to_string(), "bakka".to_string()]);
    assert_eq!(tagged.len(), 2);
}
