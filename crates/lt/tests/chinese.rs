//! Chinese (`zh`) engine tests.
//!
//! The engine is the `ChineseWordTokenizer` (Lindera jieba) +
//! `ChineseTagger` + grammar.xml + the generic
//! `DoublePunctuation`/`MultipleWhitespace` rules. Sentence splitting ports
//! HanLP `SentencesUtil` (no SRX). The example-coverage test is the
//! differential probe.

use lt::{DataDir, Engine, Grammar, Lang};

fn data_dir() -> Option<DataDir> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    if !path.is_dir() {
        eprintln!("skipping: no vendored data directory found");
        return None;
    }
    if !path.join("zh/dictionary").is_dir() {
        eprintln!("skipping: data/zh/dictionary not generated");
        return None;
    }
    Some(DataDir::new(path))
}

fn engine() -> Option<Engine> {
    let data = data_dir()?;
    Some(
        Engine::builder(Lang::Zh)
            .ok()?
            .data_dir(data)
            .build()
            .expect("build zh engine"),
    )
}

fn match_ids(engine: &Engine, text: &str) -> Vec<String> {
    engine
        .check(text)
        .map(|r| r.matches.iter().map(|m| m.rule_id.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn chinese_engine_loads() {
    let Some(engine) = engine() else { return };
    assert!(engine.active_rule_count() > 1800, "expected the zh rules");
    assert!(
        engine.compile_failures().is_empty(),
        "compile failures: {:?}",
        engine.compile_failures()
    );
}

/// Coverage probe: how many grammar examples fire their rule. Prints the
/// numbers so the rules needing boundary/POS-related changes are visible.
#[test]
fn chinese_example_coverage() {
    let Some(engine) = engine() else { return };
    let data = data_dir().unwrap();
    let grammar = Grammar::load_file(data.grammar_path(Lang::Zh)).expect("load grammar");

    let mut err = 0usize;
    let mut hit = 0usize;
    let mut miss = 0usize;
    let mut correct = 0usize;
    let mut fp = 0usize;
    let mut misses: Vec<(String, String)> = Vec::new();
    let mut fps: Vec<(String, String)> = Vec::new();
    for rule in &grammar.rules {
        for ex in &rule.examples {
            // An example demands a match only when it carries a correction
            // (`type="correct"` / no `correction` = must NOT fire).
            if ex.triggers_error {
                continue;
            }
            let expect_fire = !ex.correct && !ex.corrections.is_empty();
            let ids = match_ids(&engine, &ex.text);
            let fired = ids.iter().any(|id| id == &rule.id);
            if expect_fire {
                err += 1;
                if fired {
                    hit += 1;
                } else {
                    miss += 1;
                    if misses.len() < 5000 {
                        misses.push((
                            format!("{}#{}", rule.id, rule.sub_id.as_deref().unwrap_or("-")),
                            ex.text.clone(),
                        ));
                    }
                }
            } else {
                correct += 1;
                if fired {
                    fp += 1;
                    if fps.len() < 60 {
                        fps.push((
                            format!("{}#{}", rule.id, rule.sub_id.as_deref().unwrap_or("-")),
                            ex.text.clone(),
                        ));
                    }
                }
            }
        }
    }
    eprintln!(
        "zh examples: error {err} (hit {hit}, miss {miss}, {:.1}% fired); correct {correct} (false positive {fp})",
        if err > 0 { 100.0 * hit as f64 / err as f64 } else { 0.0 }
    );
    for (id, text) in &misses {
        eprintln!("MISS\t{id}\t{text}");
    }
    for (id, text) in &fps {
        eprintln!("FP\t{id}\t{text}");
    }
    assert!(err >= 1789, "expected the zh error examples");
    assert!(
        fp <= 18,
        "false positives on correct examples regressed: {fp}"
    );
    // Lindera's jieba segmentation/tagging differs from HanLP's. The 2026-09-24
    // per-rule triage (docs/differences.md §13) realigned the surviving typo
    // rules to whole jieba tokens and disabled the unreliable
    // segmentation/POS/classifier rules. The residue is 8 duplicate rules whose
    // error example is already caught under another rule id.
    assert!(hit >= 1781, "example coverage regressed: {hit}/{err}");
}
