//! # lt — the public facade of LingoTweaker
//!
//! ```no_run
//! use lt::{Engine, Lang};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let engine = Engine::builder(Lang::En)?.build()?;
//! let result = engine.check("I can heard you.")?;
//! println!("{} sentences, {} matches", result.sentences.len(), result.matches.len());
//! # Ok(())
//! # }
//! ```
//!
//! The engine is immutable after load, `Send + Sync`, and cheap to share via
//! `Arc`. Internal offsets are UTF-8 bytes; use `lt::to_utf16_offset` helpers
//! when exposing LT-compatible HTTP results.

pub use lt_core::{
    AnalyzedSentence, AnalyzedToken, AnalyzedTokenReadings, CheckResult, CoreError, Lang, Language,
    Match, Result, Sentence, Suggestion, TextRange,
};
pub use lt_data::DataDir;
pub use lt_pattern::{Example, Grammar, RuleDef};
pub use lt_tokenize::{EnglishWordTokenizer, SrxDocument, SrxTokenizer};

// Language-specific modules live in `en/` and `de/`; rule families that
// carry per-language entry points stay at the crate root (see `en.rs`/`de.rs`
// for the split rationale).
mod ca;
mod comma_whitespace;
mod compound;
mod da;
mod dash;
mod dates;
mod de;
mod double_punctuation;
mod el;
mod en;
mod es;
mod fr;
mod gl;
mod gn;
mod hunspell_spelling;
mod it;
mod long_sentence;
mod matchfilters;
mod morfologik_spelling;
mod multitoken;
mod nl;
mod no;
mod nrd;
mod paragraph;
mod pipeline;
mod pl;
mod pt;
mod readability;
mod repeated_words;
mod ro;
mod sentence_whitespace;
mod simple_replace;
mod sk;
mod sl;
mod specific_case;
mod style_too_often;
mod unit_conversion;
mod unpaired_brackets;
mod unpaired_quotes;
mod uppercase;
mod whitespace;
mod whitespace_before_punctuation;
mod word_coherency;
mod word_repeat;
mod word_repetition;
mod wordutil;
mod wrong_word_in_context;
pub use pipeline::{analyze_sentence, CompiledRule, Pipeline, SkippedCounts};

/// Test/probe helper exposing the German speller without the engine.
#[doc(hidden)]
pub fn spelling_de_probe(
    data_dir: &DataDir,
    variant: &str,
) -> crate::de::spelling::GermanSpellingRule {
    let tagger = std::sync::Arc::new(lt_tagger::GermanTagger::load(data_dir.path()).unwrap());
    let synth =
        std::sync::Arc::new(lt_tagger::GermanSynthesizer::from_data(data_dir.path()).unwrap());
    crate::de::spelling::GermanSpellingRule::load(data_dir.path(), variant, tagger, synth).unwrap()
}

/// Test/probe helper exposing the French speller without the engine.
#[doc(hidden)]
pub fn spelling_fr_probe(data_dir: &DataDir) -> crate::fr::spelling::FrenchSpellingRule {
    let tagger = std::sync::Arc::new(lt_tagger::FrenchTagger::load(data_dir.path()).unwrap());
    crate::fr::spelling::FrenchSpellingRule::load(data_dir.path(), tagger).unwrap()
}

/// Test/probe helper exposing the Italian speller without the engine.
#[doc(hidden)]
pub fn spelling_it_probe(data_dir: &DataDir) -> crate::it::spelling::ItalianSpellingRule {
    crate::it::spelling::ItalianSpellingRule::load(data_dir.path()).unwrap()
}

/// Test/probe helper exposing the Portuguese speller without the engine.
#[doc(hidden)]
pub fn spelling_pt_probe(
    data_dir: &DataDir,
    variant: &str,
) -> crate::pt::spelling::PortugueseSpellingRule {
    let tagger = std::sync::Arc::new(lt_tagger::PortugueseTagger::load(data_dir.path()).unwrap());
    let synth =
        std::sync::Arc::new(lt_tagger::PortugueseSynthesizer::from_data(data_dir.path()).unwrap());
    crate::pt::spelling::PortugueseSpellingRule::load(data_dir.path(), variant, tagger, synth)
        .unwrap()
}

/// Test/probe helper exposing the Dutch speller without the engine (with
/// the `CompoundAcceptor` wired like the pipeline does).
#[doc(hidden)]
pub fn spelling_nl_probe(
    data_dir: &DataDir,
) -> std::sync::Arc<crate::nl::spelling::DutchSpellingRule> {
    let tagger = std::sync::Arc::new(lt_tagger::DutchTagger::load(data_dir.path()).unwrap());
    let rule =
        std::sync::Arc::new(crate::nl::spelling::DutchSpellingRule::load(data_dir.path()).unwrap());
    let acceptor = std::sync::Arc::new(crate::nl::compound_acceptor::CompoundAcceptor::load(
        data_dir.path(),
        std::sync::Arc::clone(&tagger),
    ));
    acceptor.set_speller(std::sync::Arc::clone(&rule));
    rule.set_compound_acceptor(std::sync::Arc::clone(&acceptor));
    tagger.set_compound_acceptor(acceptor as std::sync::Arc<dyn lt_tagger::CompoundPartsProvider>);
    rule
}

/// Test/probe helper exposing the Dutch `CompoundAcceptor` with its speller
/// wired (`acceptCompound`/`getParts`).
#[doc(hidden)]
pub fn compound_acceptor_nl_probe(
    data_dir: &DataDir,
) -> std::sync::Arc<crate::nl::compound_acceptor::CompoundAcceptor> {
    let tagger = std::sync::Arc::new(lt_tagger::DutchTagger::load(data_dir.path()).unwrap());
    let speller =
        std::sync::Arc::new(crate::nl::spelling::DutchSpellingRule::load(data_dir.path()).unwrap());
    let acceptor = std::sync::Arc::new(crate::nl::compound_acceptor::CompoundAcceptor::load(
        data_dir.path(),
        std::sync::Arc::clone(&tagger),
    ));
    acceptor.set_speller(std::sync::Arc::clone(&speller));
    tagger
        .set_compound_acceptor(std::sync::Arc::clone(&acceptor)
            as std::sync::Arc<dyn lt_tagger::CompoundPartsProvider>);
    acceptor
}

/// Test/probe helper exposing the Dutch tagger with the compound acceptor
/// wired like the pipeline does.
#[doc(hidden)]
pub fn dutch_tagger_nl_probe(data_dir: &DataDir) -> std::sync::Arc<lt_tagger::DutchTagger> {
    let tagger = std::sync::Arc::new(lt_tagger::DutchTagger::load(data_dir.path()).unwrap());
    let speller =
        std::sync::Arc::new(crate::nl::spelling::DutchSpellingRule::load(data_dir.path()).unwrap());
    let acceptor = std::sync::Arc::new(crate::nl::compound_acceptor::CompoundAcceptor::load(
        data_dir.path(),
        std::sync::Arc::clone(&tagger),
    ));
    acceptor.set_speller(std::sync::Arc::clone(&speller));
    speller.set_compound_acceptor(std::sync::Arc::clone(&acceptor));
    tagger.set_compound_acceptor(acceptor as std::sync::Arc<dyn lt_tagger::CompoundPartsProvider>);
    tagger
}

/// Test/probe helper exposing the Catalan speller without the engine.
#[doc(hidden)]
pub fn spelling_ca_probe(
    data_dir: &DataDir,
) -> std::sync::Arc<crate::ca::spelling::CatalanSpellingRule> {
    let rule = std::sync::Arc::new(
        crate::ca::spelling::CatalanSpellingRule::load(data_dir.path()).unwrap(),
    );
    let tagger =
        std::sync::Arc::new(lt_tagger::CatalanTagger::load_default(data_dir.path()).unwrap());
    rule.set_tagger(tagger);
    rule
}

/// Test/probe helper exposing `DiffsAsMatches.getPseudoMatches` without the
/// engine (`scripts/oracle/ca/DiffProbe.java` output).
#[doc(hidden)]
pub fn ca_remote_deltas(original: &str, revised: &str) -> Vec<(char, usize, usize, usize, usize)> {
    crate::ca::remote::debug_deltas(original, revised)
}

#[doc(hidden)]
pub fn ca_remote_pseudo_matches(original: &str, revised: &str) -> Vec<(String, usize, usize)> {
    crate::ca::remote::debug_pseudo_matches(original, revised)
}

/// Test/probe helper exposing the Catalan tagger without the engine.
#[doc(hidden)]
pub fn catalan_tagger_ca_probe(data_dir: &DataDir) -> std::sync::Arc<lt_tagger::CatalanTagger> {
    std::sync::Arc::new(lt_tagger::CatalanTagger::load_default(data_dir.path()).unwrap())
}

/// Convert a UTF-8 byte offset into a UTF-16 code-unit offset (LanguageTool
/// HTTP API semantics). Offsets past the end or inside a multi-unit scalar
/// are clamped to the nearest char boundary (engine offsets are always
/// valid; this keeps the output surfaces robust against legacy bugs).
pub fn to_utf16_offset(text: &str, utf8_offset: usize) -> usize {
    let mut offset = utf8_offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    text[..offset].encode_utf16().count()
}

/// Options controlling which rules run.
#[derive(Debug, Clone, Default)]
pub struct EngineOptions {
    pub enabled_rules: Vec<String>,
    pub disabled_rules: Vec<String>,
    pub enabled_categories: Vec<String>,
    pub disabled_categories: Vec<String>,
    pub enabled_only: bool,
    /// include `tags="picky"` rules (Java `Level.PICKY`)
    pub picky: bool,
}

/// Builder for [`Engine`].
#[derive(Debug, Clone)]
pub struct EngineBuilder {
    lang: Lang,
    data_dir: Option<DataDir>,
    options: EngineOptions,
    /// pinned "today" for the date filters (parity tests)
    today: Option<crate::dates::Ymd>,
    /// language variant (e.g. "en-GB"); selects the spelling dictionary.
    /// Plain `en` behaves as en-US, matching the LT server's default-variant
    /// resolution.
    variant: Option<String>,
}

impl EngineBuilder {
    pub fn new(lang: Lang) -> Result<Self> {
        Ok(Self {
            lang,
            data_dir: None,
            options: EngineOptions::default(),
            today: None,
            variant: None,
        })
    }

    /// Language variant (e.g. `en-GB`) selecting the spelling dictionary.
    /// Defaults to the language's LT default variant (`en` → `en-US`).
    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.variant = Some(variant.into());
        self
    }

    /// Pin the date the date filters (`FutureDateFilter`,
    /// `NewYearDateFilter`, `DateCheckFilter`) treat as "today". Defaults to
    /// the system UTC date.
    pub fn today(mut self, year: i32, month: u32, day: u32) -> Self {
        self.today = Some(crate::dates::Ymd { year, month, day });
        self
    }

    /// Override the data directory (otherwise `LT_DATA_DIR` / `./data`).
    pub fn data_dir(mut self, dir: impl Into<DataDir>) -> Self {
        self.data_dir = Some(dir.into());
        self
    }

    pub fn options(mut self, options: EngineOptions) -> Self {
        self.options = options;
        self
    }

    pub fn build(self) -> Result<Engine> {
        // `Option::unwrap_or` would evaluate `discover()` even when a data
        // dir was supplied — fatal on wasm, where there is no file system.
        let data_dir = match self.data_dir {
            Some(dir) => dir,
            None => DataDir::discover()?,
        };
        data_dir.load_manifest()?;
        let pipeline = match self.lang {
            Lang::En => Pipeline::new_english(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::De => Pipeline::new_german(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Es => Pipeline::new_spanish(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Fr => Pipeline::new_french(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::It => Pipeline::new_italian(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Pt => Pipeline::new_portuguese(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Nl => Pipeline::new_dutch(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Ca => Pipeline::new_catalan(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Gl => Pipeline::new_galician(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Ro => Pipeline::new_romanian(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Pl => Pipeline::new_polish(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Sk => Pipeline::new_slovak(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Sl => Pipeline::new_slovenian(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::El => Pipeline::new_greek(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Da => Pipeline::new_danish(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::No => Pipeline::new_norwegian(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Nrd => Pipeline::new_nordum(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            Lang::Gn => Pipeline::new_guarani(
                &data_dir,
                self.today,
                &self.options.enabled_rules,
                self.variant.as_deref(),
            )?,
            other => {
                return Err(CoreError::UnsupportedLanguage(format!(
                    "{} pipeline is not ported yet",
                    other.base_code()
                )))
            }
        };
        let pipeline = std::sync::Arc::new(pipeline);
        // `SuppressIfAnyRuleMatchesFilter` needs the assembled pipeline to run
        // the listed rules on a candidate sentence (`JLanguageTool`).
        if let Some(catalan) = &pipeline.catalan {
            let _ = catalan
                .filter_env
                .outer_pipeline
                .set(std::sync::Arc::downgrade(&pipeline));
        }
        Ok(Engine {
            lang: self.lang,
            pipeline,
            options: self.options,
        })
    }
}

/// An immutable, `Send + Sync` checking engine.
#[derive(Debug)]
pub struct Engine {
    lang: Lang,
    pipeline: std::sync::Arc<Pipeline>,
    options: EngineOptions,
}

impl Engine {
    pub fn builder(lang: Lang) -> Result<EngineBuilder> {
        EngineBuilder::new(lang)
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    pub fn options(&self) -> &EngineOptions {
        &self.options
    }

    /// Number of rules actively matched by the v1 pattern engine.
    pub fn active_rule_count(&self) -> usize {
        self.pipeline.compiled_rules.len()
    }

    /// Number of loaded XML disambiguation rules.
    pub fn disambig_rule_count(&self) -> usize {
        if let Some(german) = &self.pipeline.german {
            return german.disambiguator.rules_len();
        }
        if let Some(spanish) = &self.pipeline.spanish {
            return spanish.disambiguator.rules_len();
        }
        if let Some(french) = &self.pipeline.french {
            return french.disambiguator.rules_len();
        }
        if let Some(italian) = &self.pipeline.italian {
            return italian.disambiguator.rules_len();
        }
        if let Some(portuguese) = &self.pipeline.portuguese {
            return portuguese.disambiguator.rules_len();
        }
        if let Some(dutch) = &self.pipeline.dutch {
            return dutch.disambiguator.rules_len();
        }
        if let Some(catalan) = &self.pipeline.catalan {
            return catalan.disambiguator.rules_len();
        }
        if let Some(galician) = &self.pipeline.galician {
            return galician.disambiguator.rules_len();
        }
        if let Some(romanian) = &self.pipeline.romanian {
            return romanian.disambiguator.rules_len();
        }
        if let Some(polish) = &self.pipeline.polish {
            return polish.disambiguator.rules_len();
        }
        if let Some(greek) = &self.pipeline.greek {
            return greek.disambiguator.rules_len();
        }
        if let Some(norwegian) = &self.pipeline.norwegian {
            return norwegian.disambiguator.rules_len();
        }
        if let Some(nordum) = &self.pipeline.nordum {
            return nordum.disambiguator.rules_len();
        }
        if let Some(guarani) = &self.pipeline.guarani {
            return guarani.disambiguator.rules_len();
        }
        self.pipeline.disambiguator.rules_len()
    }

    /// Why rules are not active yet (burn-down evidence, plan §9.3).
    pub fn skipped_counts(&self) -> SkippedCounts {
        self.pipeline.skipped_counts
    }

    /// (rule id, error) for rules that failed to compile or whose filter
    /// class is unmapped (burn-down evidence).
    pub fn compile_failures(&self) -> &[(String, String)] {
        &self.pipeline.compile_failures
    }

    /// Load the XML rule grammar for the engine's language.
    pub fn grammar(&self) -> Result<Grammar> {
        Ok(self.pipeline.grammar.clone())
    }

    /// Check `text` and return sentences plus matches (UTF-8 byte offsets).
    pub fn check(&self, text: &str) -> Result<CheckResult> {
        self.pipeline.check(text, &self.options)
    }

    /// Post-disambiguation token readings per sentence (oracle diff helper).
    pub fn analyze(&self, text: &str) -> Vec<AnalyzedSentence> {
        self.pipeline.analyze(text, true)
    }

    /// Raw token readings (tagged, before disambiguation).
    pub fn analyze_raw(&self, text: &str) -> Vec<AnalyzedSentence> {
        self.pipeline.analyze(text, false)
    }

    /// Like [`Engine::check`] but with per-call options (HTTP server).
    pub fn check_with_options(&self, text: &str, options: &EngineOptions) -> Result<CheckResult> {
        self.pipeline.check(text, options)
    }
}

/// Convert a [`TextRange`] from UTF-8 bytes to UTF-16 code units.
pub fn to_utf16_range(text: &str, range: TextRange) -> (usize, usize) {
    (
        to_utf16_offset(text, range.start),
        to_utf16_offset(text, range.end),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_offsets_match_char_units_for_ascii() {
        let text = "hello";
        assert_eq!(to_utf16_offset(text, 5), 5);
    }

    #[test]
    fn utf16_offsets_count_surrogate_pairs() {
        let text = "a😀b";
        // UTF-8: a(1) 😀(4) b(1); UTF-16: a(1) 😀(2) b(1)
        assert_eq!(to_utf16_offset(text, 5), 3);
        assert_eq!(to_utf16_offset(text, 6), 4);
    }

    #[test]
    fn engine_checks_with_sentence_offsets() {
        let Ok(builder) = Engine::builder(Lang::En) else {
            eprintln!("skipping: no vendored data found");
            return;
        };
        let engine = match builder.build() {
            Ok(e) => e,
            Err(_) => {
                eprintln!("skipping: no vendored data found");
                return;
            }
        };
        let result = engine.check("First sentence. Second one!").unwrap();
        assert_eq!(result.sentences.len(), 2);
        assert!(result.sentences[0].text.trim_end().ends_with('.'));
    }

    #[test]
    fn engine_loads_grammar() {
        let Ok(builder) = Engine::builder(Lang::En) else {
            eprintln!("skipping: no vendored data found");
            return;
        };
        let engine = match builder.build() {
            Ok(e) => e,
            Err(_) => {
                eprintln!("skipping: no vendored data found");
                return;
            }
        };
        let grammar = engine.grammar().unwrap();
        assert!(grammar.rules.len() > 5000, "rules: {}", grammar.rules.len());
    }
}
