# Rust vs Java throughput, main languages (2026-09-22)

Single-sentence (per-line) checking throughput of the Rust engine
(`lt-cli`, release build) against the pinned Java LanguageTool 6.9-SNAPSHOT
(`JLanguageTool.check` on one sentence per line), for en-US, de-DE, es, fr and
pt-PT. All numbers are single-threaded (`--jobs 1` / one `lt` object), so the
ratios reflect the sequential per-sentence cost, which is what an
interactive session or an HTTP server worker sees.

Method, harness and raw logs:

- Sentences: deduped `text` fields of `docs/parity/corpora/<lang>-examples.jsonl`,
  capped at 5,000 per language (`scripts/bench/extract_corpus.py`).
- Java: `scripts/bench/TimingDump.java` (clone of the oracle `CheckDump`
  harness with timing), run in the `maven:3.9-eclipse-temurin-21` image
  against the prebuilt standalone distribution, `-Xmx3g`, 100-sentence warm-up
  discarded. Peak RSS polled as `VmHWM` inside the container.
- Rust: `target/release/lt-cli check --lines --jobs 1`, init split from a
  1-sentence run. Peak RSS = maxrss of the process.
- Machine: AMD Ryzen AI 9 HX 370 (24 threads), 91 GB RAM, Linux.
- Timing variance across repeated runs was ~±15% for Java (JIT) and ~±5% for
  Rust; both engines computed full match sets including suggestions
  (`lt-cli check --lines` and `TimingDump` are match-set identical per the
  parity gates).

## Throughput (steady state)

| Language | Java ms/sentence | Rust ms/sentence | Rust/Java | Java lines/s | Rust lines/s |
|---|---|---|---|---|---|
| en-US | 3.6–4.7 | 9.1  | ~2.4× slower | 210–280 | 110 |
| de-DE | 13.5–16.2 | 14.4 | ~1.1× slower (parity-ish) | 62–74 | 69 |
| es    | 2.5  | 8.4  | ~3.3× slower | 396 | 119 |
| pt-PT | 9.0  | 24.9 | ~2.8× slower | 111 | 40 |
| fr    | 12.2 | 11.5 | ~parity (Rust slightly faster) | 82 | 87 |

## Engine build (init) and memory

| Language | Java build+first check | Rust engine build | Java peak RSS | Rust maxrss |
|---|---|---|---|---|
| en-US | 1.6 s | 0.8 s | ~1.3 GB | 0.53 GB |
| de-DE | 14.8 s | 5.3 s | ~1.9 GB | 0.78 GB |
| es | 1.4 s | 0.5 s | — | 0.23 GB |
| pt-PT | 1.7 s | 2.2 s | — | 0.65 GB |
| fr | 2.0 s | 0.9 s | — | 0.59 GB |

Rust init is 2–3× faster than Java and peak memory is ~2.5× lower. Java's
steady-state throughput, however, is ahead in en, es and pt by 2.4–3.3× and
on par in de/fr.

## German hunspell comparison

Raw dictionary engines over 7,500 words (5,000 corpus words + 2,500 single-char
misspellings of them; `scripts/bench/results/de-words.txt`). "suggest" measured
on misspelled words only. Java native = the legacy engine's
`DumontsHunspellDictionary` (libhunspell binding) via
`scripts/oracle/de/ProbeHunspellBench.java`; Rust = `lt_spell::hunspell`
(the pure-Rust hunspell 1.7.2 port) and `lt_spell::MorfologikSpeller`
(the engine the Rust German speller rule actually uses).

| Engine | spell() | suggest() | dict load |
|---|---|---|---|
| Java native libhunspell (de_DE) | 0.010 ms/word (103k/s) | ~56 ms/word | — |
| Rust hunspell port (de_DE) | 0.012 ms/word (81k/s) | ~91 ms/word | 372 ms |
| Rust morfologik (german.dict) | 0.0005 ms/word (1.9M/s) | ~2.2 ms/word | <1 ms |

`spell()` is at parity with native libhunspell; `suggest()` is ~1.6× slower
than native. Both hunspell variants are orders of magnitude slower than the
morfologik FSA speller — which is exactly why the Rust port uses morfologik
for the big languages and keeps the hunspell port for the small languages
that only ship `.aff/.dic`. There are no raw hunspell sources for
en/es/fr/pt in this repository, so no further hunspell comparison was
possible.

## Stage split (Rust, 1,000 lines/language)

From `cargo run --release -p lt --example stage_bench -- <lang> <file>`:

| Language | analyze | full check | speller share | pattern-rule share |
|---|---|---|---|---|
| en | 2.9 ms | 10.8 ms | 2.2 ms | 3.3 ms |
| de | 4.6 ms | 16.6 ms | 3.2 ms | 1.3 ms |
| es | 3.3 ms | 10.3 ms | 1.4 ms | 5.4 ms |
| pt | 1.2 ms | 40.1 ms | 2.5 ms | 11.6 ms |
| fr | 1.3 ms | 11.2 ms | 0.4 ms | 6.7 ms |

("pattern-rule share" = no-speller minus spell-only; both runs still include
analysis, so the sum exceeds the full-check number.)

The analysis/tagging stage (2.9–4.6 ms/line for en/de, ~1.3 for fr/pt) is a
fixed floor that Java does not pay per sentence for its POS taggers in the
same form; even with all pattern rules off, Rust spends as much as Java's
full check in en.

## Where the Rust time goes (perf, de + en, single-threaded)

`perf record` on the steady-state runs (flat profiles, + code inspection):

1. **Regex evaluation in the pattern matcher, ~15–25%.**
   `BoundedBacktracker::search` (fancy-regex Thompson NFA walk) 11% (de),
   `fancy_regex::vm::run` 2.4%, lazy-DFA/hybrid search ~2%, plus matcher
   helpers (`try_from::<&AnalyzedTokenReadings>`, `reading_matches`,
   `text_literal_hit`, `token_matches`). Token-level POS/text regexes from
   rule data are evaluated through fancy-regex even when the pattern needs no
   lookaround.
2. **Regex construction inside the hot loop, ~5% (de) / ~20% (en).**
   The `regex-automata` constructor symbols (`meta::strategy::new`,
   `thompson::compiler`, `determinize*`, NFA map `from_elem`) appear in the
   steady-state profile — regexes are being recompiled per sentence/match.
   Identified uncached construction sites (all reachable per check):
   `crates/lt/src/repeated_words.rs:395` (`pos_tag_matches`) and `:402`
   (`chunk_regex_matches`) — a fresh `Regex::new` per token comparison;
   `crates/lt/src/pipeline.rs:512` — `fancy_regex::Regex::new` per suggestion
   with a `<regexp>` back-ref; `crates/lt-pattern/src/matcher.rs:2327` and
   `:2686` — per-token `FancyRegex::new` for postag_regexp synthesis/unify.
   The shared rule regexes themselves are correctly cached
   (`compile_regex_fast`, matcher.rs:196).
3. **Hashing churn, ~11–12%.** `RandomState::hash_one` + SipHash `write`
   dominate — per-sentence `HashMap`s (token/lemma lower maps, hint lookups,
   `(String,String)`-keyed maps) keyed with the default randomized SipHash
   hasher, plus `reserve_rehash` costs (fresh maps per sentence).
4. **Allocator, ~7%.** malloc/free/memmove — per-token/per-rule string
   materialization (`to_lowercase`, `from_utf8`, clones).
5. **Speller, 0.4–3.2 ms/line** by stage timings; profile shows
   `morfologik::Search::ed`/`find_repl` (suggestion edit-distance) 4.5% (de)
   and `strip_diacritics` 4.7% (en).
6. Disambiguation/chunker/tagger: ~3–6% total (`XmlDisambiguator::apply` 3%,
   `EnglishChunker::add_chunk_tags` 2.4%, tagger automaton ~2%).
   Engine build is dominated by the parallel pattern-rule regex compile
   (de 5.3 s); dictionaries parse in well under a second.


## Measured impact of the P1–P5 changes (2026-09-22, same day)

P1 (hot-path regex caching), P2 (Fx hasher for per-sentence index maps),
P4 (fast-path chunk regexes), P5 (`Engine::shared` cache) plus one extra
profile-driven fix — the Portuguese dash rule's per-compound `str::find`
scan replaced with a single Aho-Corasick pass over all compound variants
(`crates/lt/src/dash.rs`; that naive scan was 61% of the whole pt profile) —
were implemented after this report was written. `lt-cli check --lines`
throughput on the same 5,000-sentence corpora (best-of-3 wall time,
contention on this shared machine inflates single runs):

| Language | before | after | speedup | Java (steady) |
|---|---|---|---|---|
| en-US | 46.4 s | 18.2 s | 2.5× | 18.9 s |
| de-DE | 77.1 s | 54.7 s | 1.4× | 67.3 s |
| es | 42.6 s | 33.7 s | 1.3× | 12.6 s |
| pt-PT | 126.2 s | 22.8 s | 5.5× | 45.2 s |
| fr | 58.8 s | 35.8 s | 1.6× | 61.1 s |

The Rust engine is now at parity or faster than Java for en, de, pt and fr
(Java numbers are the steady-state rows from the table above, for reference,
measured before the changes). Spanish remains ~2.7× slower than Java — its
profile is split across the pattern-rule loop and analysis, and P3
(precomputed rule gating) plus P8 are the remaining levers.

All five main-language parity gates re-run clean after the changes
(0 only-Java / 0 only-Rust / 0 unexplained field diffs against the pinned
Java goldens).

# Proposal: performance improvements for all languages

Ordered by (expected gain × universality ÷ risk). Every item below is
language-independent: it touches shared engine code or shared data-structure
handling, not per-language rules or resources, so it composes with the
in-progress non-LTR work without touching it.

**P1. Cache all hot-path regex construction (est. 5–15%, trivial risk).**
Route `pos_tag_matches`/`chunk_regex_matches` (repeated_words.rs),
`pipeline.rs:512` and the per-token `FancyRegex::new` sites in matcher.rs
through the existing interned `compile_regex_fast` cache (or `LazyLock`
statics where the pattern is static). This removes the constructor-symbol
share of the profile outright and is the single cheapest win; do it first.

**P2. Kill SipHash churn in per-sentence maps (est. ~10%, low risk).**
Use `BuildHasherDefault`-style fast hashers (FxHash/ahash) for the
per-sentence lookup maps (token-lower/lemma-lower, hints, filter-arg maps)
and reuse them across sentences (keep per-sentence content, swap the hasher).
Standard-library correctness is unaffected; no match-set changes.

**P3. Precompute per-rule gating into an index (est. 5–15% on rule-heavy
languages, medium risk).** `check_analyzed_sentence` runs ~10 string
contains/HashSet lookups per rule per sentence (`enabled_rules`,
`disabled_rules/categories`, hints). Precompute at engine build, per
`EngineOptions` set: a filtered `compiled_rules` slice plus token-masked
literal prefilters (Aho-Corasick over `rule.hints` values), so the loop
iterates only candidate rules. Java effectively does this via
`Tools.selectRules`. Biggest payoff on pt/es/fr where the rule share is
5–12 ms/line.

**P4. Prefer the `regex` crate over fancy-regex for featureless token
patterns (est. 5–10%, medium risk).** At compile time, classify pattern
regexes: those without lookaround/backrefs compile as plain `regex::Regex`
(fast DFA path) and only the rest keep the fancy fallback. The 11%
`BoundedBacktracker` share is mostly simple POS patterns like `NN.*` paying
the NFA-walk cost.

**P5. Process-wide engine cache (init cost, zero steady-state risk).**
Wrap built engines in a `OnceLock<Arc<Engine>>` keyed by language+options (or
keep the HTTP server's engine permanently warm). Today every CLI invocation
and every `build()` reparses dictionaries and recompiles thousands of regexes
(de: 5.3 s). For the parity harness and repeated CLI use this is pure win.
Optionally persist the compiled-rule cache on disk later.

**P6. Suggestion-cost bounds in the morfologik suggester (est. 1–3 ms/line
on misspelled text, low risk).** Suggestion edit distance is the whole
speller share. Tiered early-exit (return as soon as the suggestion cap is
reached), cheaper diacritic folding (single-pass, avoid re-allocation — en
`strip_diacritics` is 4.7% alone), and reusing scratch buffers in
`Search::ed`.

**P7. Allocation trimming in the sentence loop (est. 3–7%, mechanical).**
The 7% allocator share comes from per-token strings (`to_lowercase`,
cloned `String`s in `try_from`, `HashMap<String,_>` rebuilds). Reuse
per-sentence buffers and avoid re-cloning match fields (`rule.rule_id.clone()`
per emitted match) where borrows suffice.

**P8. Throughput mode (free).** The engine already has
`check_sentences_parallel` (pipeline.rs:9457); `lt-cli --lines` defaults to
`--jobs 1`. Server/batch deployments should default to sentence-level
parallelism — on this 24-core machine that is the difference between 110 and
potentially >1,000 lines/s without any of the above.

Expected combined effect for interactive single-thread checking: roughly
2× on en/es/pt (bringing Rust to Java's level or better on de/fr) from
P1+P2+P3 alone; the bigger structural items are P3/P4, which also cap the
cost of adding future rule data to any language.

Caveat: estimates are from single profiling runs; re-run `stage_bench` +
`perf` after each change, and re-run the offline parity gates
(`scripts/ci/parity.sh <lang>`) since P1–P4 touch shared matcher code that
affects all gated languages.

Raw artifacts (measurement logs; internal consumption):
`attic/notes/benchmarks-2026-09/`. Harness:
`scripts/bench/{extract_corpus.py,TimingDump.java,run_java_bench.sh,run_rust_bench.sh,bench_in_docker.sh}`
(corpus/log files under `scripts/bench/results/` are regenerated by the
runners and gitignored); profiling helpers:
`crates/lt/examples/{stage_bench,spell_bench_de}.rs`.
