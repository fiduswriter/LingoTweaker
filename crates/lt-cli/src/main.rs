//! `lt-cli` — batch checking, example-corpus extraction, inventory, and the
//! parity-harness workhorse (`check --json`).

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

use lt::Lang;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LangArg {
    En,
    De,
    Es,
    Fr,
    It,
    Pt,
    Nl,
    Ca,
    Gl,
    Ro,
    Pl,
    Sk,
    Sl,
    El,
    Da,
    Sv,
    Is,
    Eo,
    Ast,
    Br,
    Tl,
    Lt,
    Crh,
    Be,
    Ru,
    Uk,
    Sr,
    Ar,
    Fa,
    Km,
    Ml,
    Ta,
    No,
    Nrd,
    Nn,
    Gn,
}

impl From<LangArg> for Lang {
    fn from(l: LangArg) -> Self {
        match l {
            LangArg::En => Lang::En,
            LangArg::De => Lang::De,
            LangArg::Es => Lang::Es,
            LangArg::Fr => Lang::Fr,
            LangArg::It => Lang::It,
            LangArg::Pt => Lang::Pt,
            LangArg::Nl => Lang::Nl,
            LangArg::Ca => Lang::Ca,
            LangArg::Gl => Lang::Gl,
            LangArg::Ro => Lang::Ro,
            LangArg::Pl => Lang::Pl,
            LangArg::Sk => Lang::Sk,
            LangArg::Sl => Lang::Sl,
            LangArg::El => Lang::El,
            LangArg::Da => Lang::Da,
            LangArg::Sv => Lang::Sv,
            LangArg::Is => Lang::Is,
            LangArg::Eo => Lang::Eo,
            LangArg::Ast => Lang::Ast,
            LangArg::Br => Lang::Br,
            LangArg::Tl => Lang::Tl,
            LangArg::Lt => Lang::Lt,
            LangArg::Crh => Lang::Crh,
            LangArg::Be => Lang::Be,
            LangArg::Ru => Lang::Ru,
            LangArg::Uk => Lang::Uk,
            LangArg::Sr => Lang::Sr,
            LangArg::Ar => Lang::Ar,
            LangArg::Fa => Lang::Fa,
            LangArg::Km => Lang::Km,
            LangArg::Ml => Lang::Ml,
            LangArg::Ta => Lang::Ta,
            LangArg::No => Lang::No,
            LangArg::Nrd => Lang::Nrd,
            LangArg::Nn => Lang::Nn,
            LangArg::Gn => Lang::Gn,
        }
    }
}

#[derive(Parser)]
#[command(name = "lt-cli", about = "LingoTweaker command line interface")]
struct Cli {
    #[arg(long, env = "LT_DATA_DIR", global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check text with the engine and emit JSON.
    Check {
        /// Language code (en-US, en-GB, de-DE, es, fr, …)
        #[arg(short, long)]
        lang: String,
        /// Text to check, or read from file/stdin when omitted
        text: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        json: bool,
        /// Check each input line separately and emit oracle TSV
        #[arg(long)]
        lines: bool,
        /// With `--lines`: check the whole input as one text (one `L 1` TSV
        /// block) for text-level/paragraph rules; a single trailing line
        /// terminator is stripped like `CheckDumpText.java`
        #[arg(long)]
        whole: bool,
        /// include `tags="picky"` rules (Java `Level.PICKY`)
        #[arg(long)]
        picky: bool,
        /// enable a specific rule id (repeatable; Java `enableRule`)
        #[arg(long = "enable-rule")]
        enable_rules: Vec<String>,
        /// run only the enabled rules (Java `enabledOnly`)
        #[arg(long)]
        enable_only: bool,
        /// worker threads for `--lines` (results are emitted in input order)
        #[arg(long, default_value_t = 1)]
        jobs: usize,
        /// pin "today" for the date filters, like LT's JUnit harness
        /// (format: 2014-01-01); default = system date
        #[arg(long)]
        today: Option<String>,
    },
    /// Extract the <example> corpus for a language to JSONL.
    Examples {
        /// Language code (e.g. `en`, `de`, `de-DE-x-simple-language`)
        #[arg(short, long)]
        lang: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Print rule inventory statistics for a language.
    Inventory {
        /// Language code (e.g. `en`, `de`, `de-DE-x-simple-language`)
        #[arg(short, long)]
        lang: String,
        /// print every rule id (with sub id) instead of the summary
        #[arg(long)]
        rules: bool,
        /// print the rule metadata as JSON (for the web UI)
        #[arg(long)]
        json: bool,
    },
    /// Dump per-token readings as TSV (Java-oracle diff helper).
    Analyze {
        #[arg(short, long)]
        lang: LangArg,
        /// tag only, skip disambiguation
        #[arg(long)]
        raw: bool,
        /// Text to analyze, or read from file/stdin when omitted
        text: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Serve the LT v2-compatible HTTP API.
    Serve {
        #[arg(long, default_value = "0.0.0.0:8081")]
        addr: String,
    },
    /// Run the example corpus against the engine and report per-rule hits
    /// (primary parity gate, plan §9.1).
    Smoke {
        #[arg(short, long)]
        lang: LangArg,
        #[arg(long)]
        corpus: PathBuf,
        #[arg(long, default_value = "2000")]
        limit: usize,
        /// print per-rule hit rates
        #[arg(long)]
        detail: bool,
        /// pin "today" for the date filters, like LT's JUnit harness
        /// (format: 2014-01-01); default = system date
        #[arg(long)]
        today: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.cmd {
        Command::Check {
            lang,
            text,
            file,
            json,
            lines,
            whole,
            picky,
            enable_rules,
            enable_only,
            jobs,
            today,
        } => cmd_check(
            &cli,
            lang,
            text.clone(),
            file.clone(),
            *json,
            *lines,
            *whole,
            *picky,
            enable_rules.clone(),
            *enable_only,
            *jobs,
            today.clone(),
        ),
        Command::Analyze {
            lang,
            raw,
            text,
            file,
        } => cmd_analyze(&cli, *lang, *raw, text.clone(), file.clone()),
        Command::Examples { lang, out } => cmd_examples(&cli, lang, out.clone()),
        Command::Inventory { lang, rules, json } => cmd_inventory(&cli, lang, *rules, *json),
        Command::Serve { addr } => cmd_serve(addr.clone()),
        Command::Smoke {
            lang,
            corpus,
            limit,
            detail,
            today,
        } => cmd_smoke(&cli, *lang, corpus, *limit, *detail, today.clone()),
    }
}

fn data_dir(cli: &Cli) -> Result<lt_data::DataDir> {
    if let Some(dir) = &cli.data_dir {
        Ok(lt_data::DataDir::new(dir))
    } else {
        lt_data::DataDir::discover().context("set LT_DATA_DIR or run from the repository root")
    }
}

fn parse_today(today: &str) -> Result<(i32, u32, u32)> {
    let parts: Vec<u32> = today
        .split('-')
        .map(|p| p.parse())
        .collect::<std::result::Result<_, _>>()
        .with_context(|| format!("bad --today format, expected yyyy-mm-dd: {today}"))?;
    anyhow::ensure!(parts.len() == 3, "bad --today format, expected yyyy-mm-dd");
    Ok((parts[0] as i32, parts[1], parts[2]))
}

#[allow(clippy::too_many_arguments)]
fn cmd_check(
    cli: &Cli,
    lang: &str,
    text: Option<String>,
    file: Option<PathBuf>,
    json: bool,
    lines: bool,
    whole: bool,
    picky: bool,
    enable_rules: Vec<String>,
    enable_only: bool,
    jobs: usize,
    today: Option<String>,
) -> Result<()> {
    let lang_enum = parse_lang(lang)?;
    let data = data_dir(cli)?;
    let options = lt::EngineOptions {
        picky,
        enabled_rules: enable_rules,
        enabled_only: enable_only,
        ..lt::EngineOptions::default()
    };
    let mut builder = lt::Engine::builder(lang_enum)?
        .data_dir(data)
        .options(options);
    if let Some(variant) = variant_of(lang) {
        builder = builder.variant(variant);
    }
    if let Some(today) = &today {
        let (year, month, day) = parse_today(today)?;
        builder = builder.today(year, month, day);
    }
    let engine = builder.build()?;
    let input = match (text, file) {
        (Some(t), _) => t,
        (None, Some(f)) => std::fs::read_to_string(f)?,
        (None, None) => std::io::read_to_string(std::io::stdin())?,
    };
    if lines {
        // one input line = one check; oracle TSV matching scripts/oracle/CheckDump.java
        let q = |s: &str| s.replace('\t', "\\t").replace('\n', "\\n");
        let render = |lineno: usize, line: &str| -> Result<String> {
            let mut out = String::new();
            out.push_str(&format!("L\t{}\t{}\n", lineno + 1, q(line)));
            let result = engine.check(line)?;
            for m in &result.matches {
                let suggestions = m
                    .suggestions
                    .iter()
                    .map(|s| s.value.as_str())
                    .collect::<Vec<_>>()
                    .join("|");
                // Java's CheckDump writes UTF-16 offsets (Java String indices)
                let (from, to) = lt::to_utf16_range(line, m.range);
                out.push_str(&format!(
                    "M\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    lineno + 1,
                    m.rule_id,
                    m.sub_id.as_deref().unwrap_or(""),
                    from,
                    to,
                    m.match_type,
                    q(&m.message),
                    q(&suggestions)
                ));
            }
            Ok(out)
        };
        if whole {
            // the whole input is one text (multi-paragraph); one trailing
            // line terminator is stripped like scripts/oracle/CheckDumpText.java
            let text = input
                .strip_suffix('\n')
                .map(|t| t.strip_suffix('\r').unwrap_or(t))
                .unwrap_or(&input);
            print!("{}", render(0, text)?);
            return Ok(());
        }
        let items: Vec<(usize, &str)> = input
            .lines()
            .enumerate()
            .map(|(i, l)| (i, l.trim_end_matches('\r')))
            .filter(|(_, l)| !l.is_empty())
            .collect();
        if jobs > 1 && items.len() > 1 {
            let results = std::sync::Mutex::new(Vec::with_capacity(items.len()));
            let next = std::sync::atomic::AtomicUsize::new(0);
            let workers = jobs.min(items.len());
            std::thread::scope(|scope| {
                for _ in 0..workers {
                    scope.spawn(|| {
                        let mut local = Vec::new();
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if i >= items.len() {
                                break;
                            }
                            let (lineno, line) = items[i];
                            if let Ok(s) = render(lineno, line) {
                                local.push((i, s));
                            }
                        }
                        results.lock().unwrap().extend(local);
                    });
                }
            });
            let mut all = results.into_inner().unwrap();
            all.sort_by_key(|(i, _)| *i);
            for (_, s) in all {
                print!("{s}");
            }
        } else {
            for (lineno, line) in items {
                print!("{}", render(lineno, line)?);
            }
        }
        return Ok(());
    }
    let result = engine.check(&input)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "{} sentences, {} tokens, {} matches",
            result.sentences.len(),
            lt_tokenize::tokenize(&input).len(),
            result.matches.len()
        );
    }
    Ok(())
}

/// TSV per-token dump matching `/tmp/kilo/ltoracle/Dump.java` for oracle diffs:
/// `S<TAB>idx<TAB>sentence`, then `R|D<TAB>tok idx<TAB>surface<TAB>start
/// <TAB>FLAGS<TAB>readings<TAB>chunks`. Flags: WSB (whitespace before),
/// SS, SE, IMM, IGN, TYP.
fn cmd_analyze(
    cli: &Cli,
    lang: LangArg,
    raw: bool,
    text: Option<String>,
    file: Option<PathBuf>,
) -> Result<()> {
    let lang: Lang = lang.into();
    if !matches!(
        lang,
        Lang::En
            | Lang::De
            | Lang::Es
            | Lang::It
            | Lang::Pt
            | Lang::Nl
            | Lang::Ca
            | Lang::Gl
            | Lang::Ro
            | Lang::Pl
            | Lang::Sk
            | Lang::Sl
            | Lang::El
            | Lang::Is
            | Lang::Eo
            | Lang::Ast
            | Lang::Br
            | Lang::Tl
            | Lang::Lt
            | Lang::Crh
            | Lang::Be
            | Lang::Uk
            | Lang::Ar
            | Lang::Fa
            | Lang::Km
            | Lang::Ml
            | Lang::Ta
    ) {
        bail!(
            "analyze currently supports en/de/es/it/pt/nl/ca/gl/ro/pl/sk/sl/el/is/eo/ast/br/tl/lt/crh/be/uk/ar/fa/km/ml/ta only"
        );
    }
    let data = data_dir(cli)?;
    let engine = lt::Engine::builder(lang)?.data_dir(data).build()?;
    let input = match (text, file) {
        (Some(t), _) => t,
        (None, Some(f)) => std::fs::read_to_string(f)?,
        (None, None) => std::io::read_to_string(std::io::stdin())?,
    };
    let q = |s: &str| s.replace('\t', "\\t").replace('\n', "\\n");
    // one input line = one sentence, like the Java oracle helper
    let mut sentences = Vec::new();
    for line in input.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if raw {
            sentences.extend(engine.analyze_raw(line));
        } else {
            sentences.extend(engine.analyze(line));
        }
    }
    for (si, sentence) in sentences.iter().enumerate() {
        println!("S\t{}\t{}", si + 1, q(&sentence.text));
        let kind = if raw { "R" } else { "D" };
        for (ti, token) in sentence.tokens.iter().enumerate() {
            let mut flags = String::new();
            if token.whitespace_before {
                flags.push_str(" WSB");
            }
            if token.is_sentence_start {
                flags.push_str(" SS");
            }
            if token.is_sentence_end {
                flags.push_str(" SE");
            }
            if token.is_immunized {
                flags.push_str(" IMM");
            }
            if token.is_ignore_spelling {
                flags.push_str(" IGN");
            }
            if token.has_typographic_apostrophe {
                flags.push_str(" TYP");
            }
            let readings = token
                .readings
                .iter()
                .map(|r| {
                    format!(
                        "|{}:{}",
                        q(r.stem.as_deref().unwrap_or("")),
                        q(r.pos_tag.as_deref().unwrap_or(""))
                    )
                })
                .collect::<String>();
            let mut line = format!(
                "{kind}\t{ti}\t{}\t{}\t{}",
                q(token.surface()),
                token.start_pos,
                flags.trim()
            );
            line.push_str(&format!("\treadings{readings}"));
            if !token.chunk_tags.is_empty() {
                let chunks = token
                    .chunk_tags
                    .iter()
                    .map(|c| format!("|{c}"))
                    .collect::<String>();
                line.push_str(&format!("\tchunks{chunks}"));
            }
            println!("{line}");
        }
    }
    Ok(())
}

fn cmd_examples(cli: &Cli, lang_code: &str, out: Option<PathBuf>) -> Result<()> {
    let lang: Lang = parse_lang(lang_code)?;
    let variant = variant_of(lang_code);
    let data = data_dir(cli)?;
    let mut count = 0usize;
    let stdout = std::io::stdout();
    let wrote_to_file = out.is_some();
    let mut w: Box<dyn std::io::Write> = match out {
        Some(path) => Box::new(std::fs::File::create(&path)?),
        None => Box::new(stdout.lock()),
    };
    for path in rule_files(&data, lang, variant.as_deref())? {
        let grammar = lt_pattern::Grammar::load_file(&path)
            .with_context(|| format!("loading {}", path.display()))?;
        for rule in &grammar.rules {
            for (i, ex) in rule.examples.iter().enumerate() {
                let line = json!({
                    "rule_id": rule.id,
                    "sub_id": rule.sub_id,
                    "category_id": rule.category_id,
                    "kind": "pattern",
                    "correct": ex.correct,
                    "triggers_error": ex.triggers_error,
                    "text": ex.text,
                    "corrections": ex.corrections,
                    "source_file": path.strip_prefix(data.path()).map(|p| p.display().to_string()).unwrap_or_default(),
                    "example_index": i,
                });
                use std::io::Write;
                serde_json::to_writer(&mut w, &line)?;
                writeln!(&mut w)?;
                count += 1;
            }
        }
    }
    let disambig_path = data.disambiguation_path(lang);
    // The Simple German variant shares the German `de/disambiguation.xml`
    // (already part of the German corpus), so it contributes no disambiguation
    // examples of its own (D-309).
    if variant.as_deref() != Some("de-DE-x-simple-language") && disambig_path.exists() {
        let file = lt_pattern::Grammar::load_file(&disambig_path)
            .with_context(|| format!("loading {}", disambig_path.display()))?;
        for rule in &file.rules {
            for (i, ex) in rule.examples.iter().enumerate() {
                let line = json!({
                    "rule_id": rule.id,
                    "kind": "disambiguation",
                    "correct": null,
                    "text": ex.text,
                    "inputform": ex.inputform,
                    "outputform": ex.outputform,
                    "source_file": disambig_path.strip_prefix(data.path()).map(|p| p.display().to_string()).unwrap_or_default(),
                    "example_index": i,
                });
                use std::io::Write;
                serde_json::to_writer(&mut w, &line)?;
                writeln!(&mut w)?;
                count += 1;
            }
        }
    }
    w.flush()?;
    if wrote_to_file {
        eprintln!("wrote {count} examples");
    }
    Ok(())
}

fn cmd_inventory(cli: &Cli, lang_code: &str, list_rules: bool, as_json: bool) -> Result<()> {
    let lang: Lang = parse_lang(lang_code)?;
    let variant = variant_of(lang_code);
    let data = data_dir(cli)?;
    let mut total_rules = 0usize;
    let mut total_examples = 0usize;
    let mut with_filter = 0usize;
    let mut regexp_rules = 0usize;
    let mut categories = std::collections::BTreeMap::new();
    let mut files = 0usize;
    let mut json_rules: Vec<serde_json::Value> = Vec::new();
    for path in rule_files(&data, lang, variant.as_deref())? {
        files += 1;
        let grammar = lt_pattern::Grammar::load_file(&path)
            .with_context(|| format!("loading {}", path.display()))?;
        if as_json {
            for rule in &grammar.rules {
                json_rules.push(json!({
                    "id": rule.id,
                    "subId": rule.sub_id,
                    "name": rule.name,
                    "categoryId": rule.category_id,
                    "categoryName": rule.category_name,
                    "issueType": rule.issue_type,
                    "defaultOn": rule.default_on,
                    "categoryDefaultOn": rule.category_default_on,
                    "picky": rule.tags.iter().any(|t| t == "picky"),
                    "tags": rule.tags,
                    "priority": rule.prio,
                    "complex": rule.complex_pattern,
                }));
            }
        }
        total_rules += grammar.rules.len();
        total_examples += grammar.example_count();
        with_filter += grammar.rules.iter().filter(|r| r.has_filter).count();
        regexp_rules += grammar.rules.iter().filter(|r| r.is_regexp_rule).count();
        for cat in &grammar.categories {
            *categories
                .entry(format!("{} ({})", cat.name, cat.id))
                .or_insert(0usize) += grammar
                .rules
                .iter()
                .filter(|r| r.category_id.as_deref() == Some(cat.id.as_str()))
                .count();
        }
        if list_rules {
            for rule in &grammar.rules {
                match &rule.sub_id {
                    Some(sub) => println!("{}\t{sub}", rule.id),
                    None => println!("{}", rule.id),
                }
            }
        }
    }
    if as_json {
        println!("{}", serde_json::to_string(&json_rules)?);
        return Ok(());
    }
    if list_rules {
        return Ok(());
    }
    println!(
        "language: {}",
        variant.as_deref().unwrap_or(lang.info().long_code)
    );
    println!("files: {files}");
    println!("rules: {total_rules}");
    println!("examples: {total_examples}");
    println!("rules with <filter>: {with_filter}");
    println!("regexp rules: {regexp_rules}");
    println!("categories: {}", categories.len());
    for (name, count) in &categories {
        println!("  {count:6}  {name}");
    }
    Ok(())
}

fn cmd_serve(addr: String) -> Result<()> {
    let state = match lt_http::AppState::new(env!("CARGO_PKG_VERSION"), "unknown") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: {e}; serving validation-only endpoints");
            lt_http::AppState::without_engines(env!("CARGO_PKG_VERSION"), "unknown")
        }
    };
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        println!("listening on http://{addr}");
        lt_http::serve(&addr, state).await
    })?;
    Ok(())
}

fn parse_lang(code: &str) -> Result<Lang> {
    Lang::from_long_code(code)
        .with_context(|| format!("unsupported language {code} (v1: en, de, es, fr, it, pt)"))
}

/// Language variant part of a long code (`de-AT` -> `de-AT`), used to select
/// the variant spelling dictionary/rule files. The Simple German private-use
/// tag is kept whole (`de-DE-x-simple-language`), since it selects a whole
/// variant rule file (D-309).
fn variant_of(code: &str) -> Option<String> {
    if code.eq_ignore_ascii_case("de-DE-x-simple-language") {
        return Some("de-DE-x-simple-language".to_string());
    }
    let parts: Vec<&str> = code.split(['-', '_']).collect();
    if parts.len() >= 2 && !parts[1].is_empty() {
        Some(format!(
            "{}-{}",
            parts[0].to_lowercase(),
            parts[1].to_uppercase()
        ))
    } else {
        None
    }
}

fn rule_files(data: &lt_data::DataDir, lang: Lang, variant: Option<&str>) -> Result<Vec<PathBuf>> {
    // Simple German is a variant of `de` whose grammar lives in a
    // subdirectory of the German rules dir (`de/rules/de-DE-x-simple-language/`).
    // For that variant only the subdirectory is the rule set; for plain `de`
    // the subdirectory is excluded (it is not part of `German.getRuleFileNames`).
    const SIMPLE: &str = "de-DE-x-simple-language";
    let dir = data.path().join(lang.base_code()).join("rules");
    if !dir.is_dir() {
        bail!("no vendored rules at {}", dir.display());
    }
    if variant == Some(SIMPLE) {
        let sub = dir.join(SIMPLE);
        if !sub.is_dir() {
            bail!("no vendored rules at {}", sub.display());
        }
        let mut files = walk_xml(&sub);
        files.sort();
        return Ok(files);
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            // `bitext.xml` (Galician) is a parallel-corpus file, not a rule
            // file loaded by the engine (`Language.getRuleFileNames` returns
            // grammar.xml/style.xml); its examples must not enter the corpus.
            if p.file_name().is_some_and(|n| n == "bitext.xml") {
                return false;
            }
            // `grammar-nezaradene.xml` (Slovak) is not referenced by
            // `Slovak.getRuleFileNames` (only grammar.xml and the
            // `RULE_FILES` extra `grammar-typography.xml` are loaded).
            if p.file_name().is_some_and(|n| n == "grammar-nezaradene.xml") {
                return false;
            }
            // The Simple German variant's own grammar (D-309).
            if p.file_name().is_some_and(|n| n == SIMPLE) {
                return false;
            }
            if p.is_file() {
                return p.extension().is_some_and(|e| e == "xml");
            }
            if p.is_dir() {
                return true;
            }
            false
        })
        .flat_map(|p| if p.is_dir() { walk_xml(&p) } else { vec![p] })
        .collect();
    files.sort();
    Ok(files)
}

fn walk_xml(dir: &std::path::Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "xml"))
                .collect()
        })
        .unwrap_or_default()
}

fn cmd_smoke(
    _cli: &Cli,
    lang: LangArg,
    corpus: &PathBuf,
    limit: usize,
    detail: bool,
    today: Option<String>,
) -> Result<()> {
    let lang: Lang = lang.into();
    if !matches!(lang, Lang::En) {
        bail!("smoke currently supports en only");
    }
    let mut builder = lt::Engine::builder(lang)?;
    if let Some(today) = &today {
        let (year, month, day) = parse_today(today)?;
        builder = builder.today(year, month, day);
    }
    let engine = builder.build()?;
    let file = std::fs::File::open(corpus)?;
    let reader = std::io::BufReader::new(file);

    #[derive(serde::Deserialize)]
    struct Ex {
        rule_id: String,
        kind: String,
        correct: Option<bool>,
        triggers_error: Option<bool>,
        text: String,
    }

    let mut checked = 0usize;
    let mut hit = 0usize;
    let mut wrong_rule = 0usize;
    let mut miss = 0usize;
    let mut per_rule: std::collections::BTreeMap<String, (usize, usize)> = Default::default();

    for line in std::io::BufRead::lines(reader) {
        if checked >= limit {
            break;
        }
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let ex: Ex = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if ex.kind != "pattern" || ex.correct != Some(false) || ex.triggers_error == Some(true) {
            continue;
        }
        checked += 1;
        let result = engine.check(&ex.text)?;
        let rule_hit = result.matches.iter().any(|m| m.rule_id == ex.rule_id);
        let entry = per_rule.entry(ex.rule_id.clone()).or_insert((0, 0));
        entry.0 += 1;
        if rule_hit {
            hit += 1;
            entry.1 += 1;
        } else if !result.matches.is_empty() {
            wrong_rule += 1;
        } else {
            miss += 1;
        }
    }

    println!("examples checked: {checked}");
    println!(
        "rule fired correctly: {hit} ({:.1}%)",
        if checked > 0 {
            hit as f64 * 100.0 / checked as f64
        } else {
            0.0
        }
    );
    println!("match but wrong rule: {wrong_rule}");
    println!("no match: {miss}");
    if detail {
        println!("\nper-rule (examples, hits):");
        for (rule, (total, hits)) in per_rule {
            println!("  {hits:4}/{total:<4} {rule}");
        }
    }
    Ok(())
}
