# Intentional differences from the Java engine

Policy (D-310, owner 2026-09-23): a divergence is **fixed** only when the Java
implementation is more correct; where the **Rust** implementation is more
correct, Rust is kept and the divergence is documented here as intentional.
Every entry must state which side is more correct and why. Entries are labelled
either **intentional: Rust more correct** (Java's output is wrong or an
artifact, so the Rust behaviour is deliberately kept) or **residue to fix**
(an engine-fidelity gap / unported feature where Java is the reference and
Rust should eventually be brought up to it). The same distinction is annotated
in the `scripts/ci/parity.sh` allowances.

This file records cases where the Rust engine deliberately does **not**
replicate the Java engine's behavior because Java's decision is (or appears
to be) wrong. Every entry has a reproduction and says which engine is more
correct and why. Everything not listed here matches Java; verify with
`scripts/oracle/gate.sh` (2k sample) and the full-corpus oracle
(D-016/D-017). As of D-032 this is the only remaining difference
(0 only-Java, 0 only-Rust, 1 field diff over 23,818 examples).

## 1. `ADVERB_VERB_ADVERB_REPETITION` suggestion (` do n't`) (line 7156)

- Java engine: `"n't do| do n't"` (leading space in the second suggestion).
- Java rule-level `PatternRule.match` probe: `[n't do, do n't]` (no space) —
  i.e. Java's engine output contradicts its own rule result.
- Rust: `"n't do|do n't"`.
- Cause: Java's `PatternRuleMatcher.formatMatches` mutates the suggestion
  buffer in place and the second suggestion inherits a separator space from
  the first replacement's trailing text; the rule itself defines the
  suggestions as `\1 \2` and `\2 \3`, which render without a leading space.
- Verdict: **Rust is more correct** (and agrees with Java's own rule-level
  output). Not fixed.
- Reproduce: `If it isn’t don’t say things that could be considered offensive.`

## 2. `ES_SIMPLE_REPLACE_VERBS` expands `$match` in the message

Reproduction (pinned Java build, `scripts/oracle/es/probe-rule.sh`):

```
$ scripts/oracle/es/probe-rule.sh "Voy a logear en el sistema." ES_SIMPLE_REPLACE_VERBS
ES_SIMPLE_REPLACE_VERBS_LOGEAR	6	12	Verbo incorrecto: $match	loguear|conectar|entrar|iniciar sesión
```

Java's `es.SimpleReplaceVerbsRule.getMessage(String tokenStr, List replacements)`
ignores `tokenStr` and returns the literal `"Verbo incorrecto: $match"`; the
legacy `AbstractSimpleReplaceRule.createRuleMatch` expands `$match` only in the
*description* (which Java does: `ES_SIMPLE_REPLACE_VERBS_LOGEAR`), unlike
`AbstractSimpleReplaceRule2`, which expands `$match`/`$suggestions` in both the
message and the description. The unexpanded `$match` in a user-visible message
is a Java bug. An audit of the legacy `AbstractSimpleReplaceRule` subclasses in
en/de/es/fr/ca found no other affected rule: Catalan's six legacy-base rules
(`SimpleReplaceRule`, `SimpleReplaceDiacriticsIEC`, `ReplaceOperationNamesRule`,
`SimpleReplaceAdverbsMent`, `SimpleReplaceBalearicRule`, `SimpleReplaceVerbsRule`)
and Spanish's other rules keep `$match` only in `getDescription()`, which the
base expands, and the no-arg `getMessage()` templates of
`AbstractSimpleReplaceRule2` rules are expanded in both engines.
The Catalan stage-3 audit (2026-09-19, pinned source) covered all nine
concrete legacy-base rules: the six listed above plus the three DNV rules
(`SimpleReplaceDNVRule`, `SimpleReplaceDNVColloquialRule`,
`SimpleReplaceDNVSecondaryRule`), which extend `AbstractSimpleReplaceRule`
through `AbstractSimpleReplaceLemmasRule` and use literal messages (no
`$match` anywhere). No Catalan message divergence exists.

**Rust is more correct**: the port expands the placeholder exactly like Java's
own description handling (`originalTokenStr`), so the message reads
`Verbo incorrecto: logear`. Consequence: where this rule fires, the message is
a documented field diff vs Java (the current 6,944-line Spanish corpus contains
no `ES_SIMPLE_REPLACE_VERBS` match, so the recorded Spanish totals are
unaffected); every other field is identical. No English or German message is
affected.

## 3. `FRENCH_WORD_REPEAT_RULE` duplicate-word false positives (7 corpus matches)

`data/fr/rules/style.xml:16078` ("Doublon")'s identical-word subrule is a
two-token pattern `(\p{L}+)` + `<token spacebefore="yes"><match no="0"/></token>`
whose intent (documented by its own examples, e.g.
`<example correction="Je"><marker>Je je</marker> suis français.</example>`)
is to flag a word repeated twice in a row. Rust implements the `<match
no="0"/>` reference as the first matched token, so it flags exactly those
repetitions.

Pinned Java (single-line probe, `scripts/oracle/fr/probe-rule.sh "<text>"
FRENCH_WORD_REPEAT_RULE`, Docker, commit `7bd1f99b849b`):

```
Parfois, on dit ou on écrit, avec insouciance.   -> 9..15 "on dit",   suggestion "dit"
Qu'ils restent ou ils sont.                      -> 3..14 "ils restent", suggestion "restent"
Je laisse mon numéro de téléphone au cas ou ...  -> 0..9  "Je laisse", suggestion "laisse"
```

None of these lines contains two consecutive identical words, and the
suggestion is always the *second* word of the pair — Java's reference
element resolves to a token that makes the rule match arbitrary consecutive
word pairs. All 7 Java-only `FRENCH_WORD_REPEAT_RULE` corpus diffs are of
this shape; every true repetition (e.g. `Je je suis français.`) is reported
by both engines.

**Rust is more correct** (it follows the subrule's own documented examples);
Java's matches are false positives. Not fixed — reproducing them would mean
emulating Java's reference-element resolution quirk. Reproduce with
`scripts/oracle/fr/probe-repeat.sh` (prints the token view, the match range
and the pattern for each match).

## 4. `SUJET_AUXILIAIRE[1]` corpus false positive (1 corpus match)

Corpus line: `Je me demande ou je pourrais partir en vacances.` — the line
does contain a real error (`ou` means "where" here and must be `où`) and
both engines report it (`OU[32]` 14..16, suggestion `où`; pinned by
`french_ou_accent_on_where_matches_java` in `crates/lt/tests/french.rs`).
The divergence is one *extra* Java match: `SUJET_AUXILIAIRE[1]` 3..5 (`me`,
suggestions `suis me|ai me|mue`), which Rust does not report.

Pinned Java build (`scripts/oracle/fr/probe-mutation.sh "<text>"
SUJET_AUXILIAIRE 1`): on a freshly analyzed sentence the isolated subrule
match is **0 matches and the sentence is unchanged**. In
`JLanguageTool.check` (same probe without the sub id) the rule `OU` subrule
7 (`data/fr/rules/grammar.xml`, the conjoined-clause `ou` rule) **mutates the
shared `AnalyzedSentence` while matching**: the second token `Je{je:R pers
suj 1 s}` becomes `demande{je:R pers suj 1 s, demander:V ind pres 3 s,
demander:V sub pres 1 s, demander:V sub pres 3 s}` — its surface widens to a
neighbouring token's and that token's readings are appended — even though
`OU[7]` itself returns zero matches. After that mutation,
`SUJET_AUXILIAIRE[1]` matches against the corrupted readings:

```
check MATCH sub=1 3-5 [suis me, ai me, mue]
MUTATED by OU sub=7 -> {null:SENT_START,} demande{je:R pers suj 1 s,demander:V ind pres 3 s,...} ...
inOrder sub=1 len=1
```

The mutation is reproducible by calling `OU[7].match(sentence)` alone
(zero matches returned, sentence still mutated), so it is a side effect of
Java's matcher on the shared token objects, not a rule-level decision. It is
order-dependent cross-rule state corruption; Rust's immutable per-sentence
analysis cannot and should not reproduce it.

**Rust is more correct.** Reproduce with
`scripts/oracle/fr/probe-mutation.sh "Je me demande ou je pourrais partir en
vacances." SUJET_AUXILIAIRE` (prints the `check` match, the mutating rule and
the in-order match lengths) and `... SUJET_AUXILIAIRE 1` (isolated subrule:
0 matches, no mutation). This is the only remaining Java-only French corpus
match besides #3; the field-diff and suggestion-order cases in
`fr-rule-port.md` are tracked separately.

## 5. `AGREEMENT_PARTICULAR` multi-form suggestion rendering (1 corpus match)

Corpus line: `La réunion suivant.` — Java reports
`AGREEMENT_PARTICULAR` 11..19 with suggestions
`suivant |suivante.|suivie(.)|suivant(.)`; Rust reports
`suivant |suivante.|suivie.|suivantes.|suivies.|suivants.|suivis.`.

Pinned Java (`scripts/oracle/fr/probe-rule.sh "La réunion suivant."
AGREEMENT_PARTICULAR`). The subrule's suggestions are `\2 `, a `$1 f s`
synthesis, `$1 f p` and `$1 [em] p`, each followed by the literal `\4`
(the punctuation token). `suivant` has the readings `J m s`, `N m s`, `P`
and `V ppr` (lemma `suivre`); `FrenchSynthesizer` has `suivre` entries for
the `J` tags, so the `$1 f s`/`$1 f p` matches synthesize **two** forms each
(`suivante`/`suivie`, `suivantes`/`suivies`). With one form per block (the
rule's own `encombrant` example) both engines agree exactly.

Java's `formatMultipleSynthesis` writes the extra forms as
`</suggestion>, <suggestion>` groups and continues scanning the same
buffer; the literal `\4` inside those groups is then expanded against the
flat `suggestionMatches` counter (see the `suggestionMatches.add(
suggestionMatches.get(numbersToMatches[j]))` workaround and the
`errorMessageProcessed` FIXME in `PatternRuleMatcher.formatMatches`), which
corrupts the pairing: the punctuation is rendered through a postag-bearing
match (`(.)` = `MatchState.toFinalString`'s no-synthesis fallback) and three
synthesized forms disappear. Rust renders every suggestion block
independently as a Cartesian product of its parts, which is the consistent
reading of the rule.

**Rust is more correct**; Java's list is an artifact of its buffer/counter
suggestion rendering. Reproduce:
`scripts/oracle/fr/probe-rule.sh "La réunion suivant." AGREEMENT_PARTICULAR`.
This is the only remaining French corpus field diff.

## 6. `PODER_SER_POSSIVEL` positional `<match>` expansion (1 corpus match)

Corpus line: `Vou estudar enquanto possa ser possível.` — Java reports
`PODER_SER_POSSIVEL` 21..30 with **1,481** suggestions; Rust reports the
same match with **495**.

The rule (`data/pt/rules/style.xml:1053`) defines
`<suggestion><match no='1' postag='VMI.+|VMN0000' postag_regexp='yes'>dever</match> \2</suggestion>`
and `<suggestion><match no='3' postag='NCM(.)000' postag_replace='VMIP3$10'>ser</match></suggestion>`.
With the short `pode` example (`A vitória pode ser possível.`) both engines
report exactly `deve ser|é`.

Pinned Java (`scripts/oracle/pt/probe-rule.sh pt-PT "Vou estudar enquanto
possa ser possível." PODER_SER_POSSIVEL`): three blocks — 494 `dever` forms
followed by ` ser`, 493 followed by ` seres`, and 494 bare forms (then the
intended `é`). The extra blocks come from Java's sequential
`PatternRuleMatcher.formatMatches` counter: the plain `\2` token
back-reference is expanded with `suggestionMatches.get(matchCounter)` (the
*second* `<match>` element, `no=3`, whose static lemma `ser` synthesizes
`ser`/`seres`), and the `\3` of the second suggestion then runs past the
match list and reuses the first `<match>` (all `dever` forms), duplicating
its block.

Rust resolves `\2` as the second pattern token's surface (`ser`), which is
the rule author's intent, so it emits the 494 `dever … ser` forms plus `é`.
**Rust is more correct**; the Java list is the same buffer/counter artifact
as differences #5. Not fixed — replicating it would mean threading Java's
positional `suggestionMatches` counter through the shared suggestion
renderer (`resolve_suggestions`), which would change every language.
Reproduce:
`scripts/oracle/pt/probe-rule.sh pt-PT "Vou estudar enquanto possa ser possível." PODER_SER_POSSIVEL`.
This is the only remaining Portuguese corpus field diff.

## 7. `HUNSPELL_RULE` (Galician) suggestion timer boundary (2 corpus field diffs)

Unlike the other entries this is a **known limitation, not a correctness
judgement**: the Galician match set is identical to the legacy engine
(0 only-Java / 0 only-Rust over the 717-example corpus). The native hunspell
suggestion engine (`suggestmgr.cxx`, hunspell 1.7.2) is ported into
`crates/lt-spell` (`HunspellChecker::suggest`): the capitalization, `REP`,
`MAP`, adjacent/long swap, add/remove/move char, double-two-chars and
two-word generators and their iteration order, the compound-aware candidate
`checkword`, the `TRY`/`KEY` tables (with hunspell's QWERTY `KEY` default),
the `Hunspell::suggest` wrapper (case restoration, `SUGSWITHDOTS`, keepcase
filtering, dedup, dash suggestions) **and the n-gram fallback
(`ngsuggest`: dictionary hash-walk order, `expand_rootword`, n-gram/LCS
scoring, `MAXNGRAMSUGS`/`ONLYMAXDIFF`/`MAXDIFF`)** all reproduce the legacy
engine.

Two field diffs remain, both at upstream's wall-clock
`TIMELIMIT_SUGGESTION`/`TIMELIMIT_GLOBAL` boundary: the legacy engine bails
out of the generator loop (and therefore skips the n-gram stage) when a
generator exceeds 100 ms / the whole suggestion exceeds 250 ms. That cutoff
is machine-timing-dependent upstream, so it cannot be reproduced
byte-for-byte; the in-tree port bounds the only exponentially branching
generator (`MAP`) with a deterministic node budget instead (D-224). A
deterministic stand-in (bail out whenever the `MAP` budget is exhausted) was
tried and made the corpus worse (6 vs 2 diffs), so it was reverted. The
residue:

- line 624, `percatamos`: the legacy run hit the generator limit and stopped
  at `percutamos|percutiramos|peraltamos|percorramos`; the port keeps
  `percútamos` and appends the n-gram `permutamos`.
- line 672, `monoméricas`: the legacy run hit the 100 ms generator limit and
  skipped n-gram; the port appends `cronométricas|monométricos|…`.

The former third diff (`Maria`, line 288) was a stale golden entry: the
pinned Java build now returns the same list as hunspell 1.7.2, and the
golden was regenerated with `scripts/ci/update-golden.sh gl` (one line).

Reproduce (pinned Java build):

```sh
scripts/oracle/gl/probe-speller.sh percatamos monoméricas
```

Pinned exactly as `--expect-field-diffs=HUNSPELL_RULE=2` in
`scripts/ci/parity.sh`. A deterministic emulation of the upstream wall-clock
cutoff would remove the allowance.

## 8. `AGREEMENT_DEMONSTRATIVE_VERB` (Spanish, hand-authored rule)

This is the first entry that is an **added rule**, not a different rendering
of an upstream one. `data/es/rules/local.xml` (manifest kind
`hand-authored`, so `lt-sync import` never overwrites it) adds
`AGREEMENT_DEMONSTRATIVE_VERB`, which reports a sentence-initial demonstrative
pronoun whose verb disagrees in number:

```
Estos es un problema.   -> "es" -> "son"
Esta son muy buena.     -> "son" -> "es"
Ese son un problema.    -> "son" -> "es"
Este están muy bueno.   -> "están" -> "está"
Este son un problema.   -> "son" -> "es"
```

Pinned Java (`scripts/oracle/es/probe-rule.sh`, Docker, commit `7bd1f99b849b`)
reports **none** of these. The upstream agreement rule
`AGREEMENT_SUBJECT_VERB_SG_PL` (`data/es/rules/grammar.xml:25242`) requires a
full singular noun phrase (`<phraseref idref="_GN_SINGULAR"/>`) before the
verb, so a bare demonstrative subject never qualifies; the broader
`AGREEMENT_SUBJECT_VERB` / `AGRREMENT_SUBJECT_PREDICATE` rules are
`default="off"` and do not match this shape even when enabled. Personal
pronouns (`Ella son profesora.`) are already handled by the upstream
`AGREEMENT_PRONOUNSUBJECT_VERB` in both engines.

**Rust is more correct**: each sentence above is a real agreement error.
Details and limits of the rule:

- Neuter `esto/eso/aquello` are excluded on purpose: `Esto son los motivos`
  is accepted by RAE, so flagging it would be a false positive.
- `Este son` is special: `son` is also a masculine noun, so the Spanish
  disambiguator reads `Este son` as the valid noun phrase "this tune" and
  drops the verb reading. The rule matches that noun reading but only when a
  further noun phrase follows (`Este son un problema.`); a verb directly after
  `son` is the valid `Este son es bonito.` and is left untouched.
- Only the sentence-initial position is covered.

Parity: the Spanish corpus (`docs/parity/golden/es-full.txt`, 7,056 lines)
was regenerated from the current rules and includes the rule's examples, so
the gate reports 0 only-Java / 11 only-Rust / 0 field diffs. The 11 are exactly
the rule's incorrect examples; `scripts/ci/parity.sh` pins them with
`--expect-only-rust=AGREEMENT_DEMONSTRATIVE_VERB=11` (exact-count validated)
and runs es with `PARITY_TODAY=2026-09-20` (the golden capture date). The
behavior is also covered by
`crates/lt/tests/spanish.rs::spanish_demonstrative_verb_agreement`.

Reproduce the Java side:

```sh
scripts/oracle/es/probe-rule.sh "Estos es un problema." AGREEMENT_SUBJECT_VERB_SG_PL
scripts/oracle/es/probe-rule.sh "Este son un problema." AGREEMENT_SUBJECT_VERB_SG_PL
```

Reproduce the Rust side:

```sh
cargo run -p lt-cli -- check -l es --json "Estos es un problema."
cargo run -p lt-cli -- check -l es --json "Este son un problema."
```

## 9. Polish known fidelity gaps (agreement unification, ZDANIA_ZLOZONE, PCON_VERB)

The Polish corpus (`docs/parity/golden/pl-full.txt`, 6,438 examples) is at
**4 only-Java / 5 only-Rust / 0 field diffs**. All remaining differences are
engine-fidelity gaps (not deliberate design choices) and are pinned exactly in
`scripts/ci/parity.sh`; the `pl` golden is captured at `PARITY_TODAY=2026-09-20`.

- **`<unify negate="yes">` agreement rules** (`ADJ_SUBST_ADJ_UNIFY`,
  `SUBST_ADJ_UNIFY`, `NIEZGODNO_PRZYPADKW_PRZYMIOTNIKA_I_RZECZOWNIKA_RODZAJU_ESKIEGO`,
  `NIEZGODNOSC_LICZBY_PODMIOTU_I_ORZECZENIA`): the Rust matcher does not yet
  reproduce Java's three-token negative-unification outcome, so these rules
  can miss (`ADJ_SUBST_ADJ_UNIFY` / `SUBST_ADJ_UNIFY`, 1 only-Java each) or
  over-fire (`ADJ_SUBST_ADJ_UNIFY`, `NIEZGODNO…`, `NIEZGODNOSC…`, 1–2
  only-Rust each). Reproduce:

  ```sh
  scripts/oracle/pl/probe-rule.sh "Beztlenowe bakterie magnetotaktyczna mają funkcję wykrywania tlenu." ADJ_SUBST_ADJ_UNIFY
  scripts/oracle/pl/probe-rule.sh "Szampon to RENE FURTERER OKARA przedłużający o 80% trwałość koloru włosów farbowanych." SUBST_ADJ_UNIFY
  ```

- **`ZDANIA_ZLOZONE` and the `comp:comma` disambiguation context** (1 only-Rust,
  1 only-Java): Java's disambiguator adds `comp:comma` to a conjunction such as
  `i`/`jak` in some clause contexts and drops it in others; the Rust tagger
  differs in a few sentences, so the rule can over-fire
  (`…gdyż jestem stary i nerwy moje są chore.`) or miss
  (`Czy słyszałeś jak mój syn gra na skrzypcach?`). Reproduce:

  ```sh
  scripts/oracle/pl/probe-rule.sh "Muszę tam czekać śmierci, gdyż jestem stary i nerwy moje są chore." ZDANIA_ZLOZONE
  scripts/oracle/pl/probe-rule.sh "Czy słyszałeś jak mój syn gra na skrzypcach?" ZDANIA_ZLOZONE
  ```

- **`PCON_VERB`** (1 only-Java): the participle-without-finite-verb rule does
  not match `Widząc to jedna szpetna starucha...` in the Rust engine. Reproduce:

  ```sh
  scripts/oracle/pl/probe-rule.sh "Widząc to jedna szpetna starucha..." PCON_VERB
  ```

## 10. Esperanto (`eo`) suggestion ranking (resolved)

The `HunspellRule` wrong-split check (`La ŭesta` -> `Laŭ esta`) is ported into
`crates/lt/src/eo/spelling.rs`, and the `SuggestMgr::twowords` UTF-8 buffer
indexing was corrected (it used a one-off 1-based view, dropping the first
character of the first split part), so the `eo.aff` `BREAK`-based
space/hyphen recombinations (`inflamiĝis` -> `inflami ĝis`, `inflami-ĝis`;
`semajnofinon` -> `semajno finon`, `semajno-finon`) now match the legacy
engine.

The Esperanto corpus (`docs/parity/golden/eo-full.txt`, 876 examples) is now
at **0 only-Java / 0 only-Rust / 0 field diffs**; the former
`--expect-only-java=HUNSPELL_RULE=1`, `--expect-only-rust=UESTO=1` and
`--expect-field-diffs=HUNSPELL_RULE=5` allowances are removed from
`scripts/ci/parity.sh`.

Reproduce (pinned Java build):

```sh
scripts/oracle/eo/probe-rule.sh "La ŭesta parto de la urbo." HUNSPELL_RULE
scripts/oracle/eo/probe-rule.sh "La digesta aparato inflamiĝis." HUNSPELL_RULE
```


## 11. Tagalog (`tl`) `MORFOLOGIK_RULE_TL` suggestion ordering (5 corpus field diffs)

The Tagalog speller dictionary (`tl/hunspell/tl_PH.dict`) is the only vendored
Morfologik dictionary with `fsa.dict.frequency-included=true`, so its
suggestion weights use the morfologik
`distance * FREQ_RANGES + FREQ_RANGES - frequency - 1` composite. The Rust
speller returns the **same suggestion set and the same match set** as the
legacy engine, but orders the frequency-weighted candidates differently for
the misspelling `nag` (5 corpus lines):

```
Java: nang|nga|pag|mag|wag|bag|Naga|Pag|nagi|ang|ng|na
Rust: ang|ng|na|nang|nga|pag|mag|wag|bag|Naga|Pag|nagi
```

The difference is the frequency code read for the high-frequency function
words `ang`/`ng`/`na` (the Rust `Speller::get_frequency` picks the first
stored annotation's last byte; the legacy engine's frequency lookup orders
them differently). The corpus gate pins this exactly with
`--expect-field-diffs=MORFOLOGIK_RULE_TL=5`; no only-Java/only-Rust matches.

Reproduce (pinned Java build):

```sh
scripts/oracle/tl/probe-rule.sh "Sa DLSU rin ako nag-aral." MORFOLOGIK_RULE_TL
```

## 12. Lithuanian (`lt`): vendored third-party dictionary, no Java baseline

The pinned upstream Lithuanian module (`Lithuanian.getRelevantRules`)
includes `MorfologikLithuanianSpellerRule` over `/lt/hunspell/lt_LT.dict`,
but that dictionary is **not shipped** in the LanguageTool checkout or in any
pinned Maven artifact (Lithuanian is deprecated upstream since 3.6; the
module's `src/main/resources/org/languagetool/resource/lt/` directory does
not exist). Consequently the legacy engine throws on **every** check:

```
java.lang.RuntimeException: Could not check sentence (language: Lithuanian)
```

The Rust engine does **not** replicate the broken legacy module. By owner
request it vendors a third-party dictionary instead: the ispell-lt 1.3.2
Hunspell dictionary (`data/lt/hunspell/lt_LT.aff`/`lt_LT.dic`, BSD-3-Clause,
from [LibreOffice/dictionaries `lt_LT`](https://github.com/LibreOffice/dictionaries/tree/master/lt_LT)),
runs the speller under the unchanged legacy id `MORFOLOGIK_RULE_LT_LT`
(`native_suggestions`, five-suggestion cap) and keeps the 4 XML rules plus the
generic built-ins. There is **no Java baseline for the speller** — the legacy
engine cannot check Lithuanian at all — so the speller's behaviour is pinned
by our own tests (`crates/lt/tests/lithuanian.rs`). The per-rule Java probes
still work because `ProbeRule` enables a single rule and never initializes the
speller. `lt` therefore stays on the **tests-only** gate in
`scripts/ci/parity.sh` (like `no`/`nrd`/`gn`): `cargo test -p lt --test
lithuanian` plus an `lt-cli inventory` sanity check, no corpus golden.

Reproduce (pinned Java build, per-rule probe):

```sh
scripts/oracle/lt/probe-rule.sh "Jaroslavas pajuto kad jo draugas yra Mantas." BRAK_PRZECINKA_ZE
```

## 13. Crimean Tatar (`crh`) Java `UNICODE_CASE` folding of `ı` (1 corpus match)

Java compiles pattern-token regexps with
`Pattern.CASE_INSENSITIVE | Pattern.UNICODE_CASE` (unless `case_sensitive`
is set). Under that flag `Character.toUpperCase('ı')` (U+0131, dotless i)
is `I`, so `[A-Za-z…]` matches `ı`:

```java
Pattern.compile("[A-Za-zñğüşöçâ][A-Za-zñğüşöçâ-]*",
    Pattern.CASE_INSENSITIVE | Pattern.UNICODE_CASE)
  .matcher("21fayız").matches()   // true
```

The Rust `regex` crate uses Unicode *simple* case folding, where U+0131
folds to itself, so `(?i)[A-Za-z…]` does **not** match `ı`. The corpus line
`21fayız.` therefore matches `COMPLEX_NUMBER_DEFIS_MISSING` in the legacy
engine but not in the Rust engine (1 only-Java match; no only-Rust matches,
0 field diffs). Pinned exactly with
`--expect-only-java=COMPLEX_NUMBER_DEFIS_MISSING=1` in
`scripts/ci/parity.sh`.

Reproduce (pinned Java build):

```sh
scripts/oracle/crh/probe-rule.sh "21fayız." COMPLEX_NUMBER_DEFIS_MISSING
```

## 14. Ukrainian (`uk`) remaining corpus residue (3 only-Java / 1 only-Rust / 2 field diffs)

The Ukrainian port is at **3 only-Java / 1 only-Rust / 2 field diffs** on the
4,437-line corpus (Java 4,613 vs Rust 4,611 matches). All four divergences are
pinned exactly in `scripts/ci/parity.sh` and are documented here with a
pinned-Java reproduction. They are all engine-fidelity gaps in the XML
disambiguation stage, not rule-data differences.

### 14a. XML disambiguation forward-scan cascade (`non_v_kly_2`) — 2 field diffs

Java's `DisambiguationPatternRuleReplacer.replace` runs `doMatch`, which scans
start positions in order and applies each match's action **immediately**; the
actions mutate the shared `AnalyzedTokenReadings` in place, so a later start
sees readings removed by an earlier match. The uk rule `non_v_kly_2`
("Не кличний після некличного") removes `v_kly` from `старший` (preceded by
`він`), and then the *same rule* removes `v_kly` from the following `сестри`,
because `старший` no longer carries `v_kly` when the scan reaches it:

```
він старший сестри на 3 роки
Java: сестри -> [ж.р.: родовий, мн.: називний]
Rust: сестри -> [ж.р.: родовий, мн.: називний, кличний]
```

The Rust `XmlDisambiguator::apply` collects all matches for a rule against a
single snapshot (`phase 1 (immutable)`) and then applies them, so the cascade
does not happen. This is the same limitation as the Polish
`<unify negate="yes">`/ZDANIA_ZLOZONE gaps (#9). It affects the two
`UK_ADJ_NOUN_INFLECTION_AGREEMENT` messages at corpus lines 2519/2520.

Reproduce (pinned Java build, full `preDisambiguate` + XML stage):

```sh
scripts/oracle/uk/probe-disambig.sh "він старший сестри на 3 роки"
# (the manual probe uses `disambiguate` only; use FullProbe.java for the
#  full `analyzeText` readings)
```

### 14b. `TokenAgreementPrepNounRule`: preposition + `не` + noun — 1 only-Java

Java flags `UK_PREP_NOUN_INFLECTION_AGREEMENT` when a `part` token (`не`)
stands between the preposition `незважаючи` and the nominative `це`; the Rust
rule aborts on the intervening particle:

```
і незважаючи не це.
Java: UK_PREP_NOUN_INFLECTION_AGREEMENT 16-18
Rust: (none)
```

Reproduce:

```sh
scripts/oracle/uk/probe-rule.sh "незважаючи не це" UK_PREP_NOUN_INFLECTION_AGREEMENT
```

### 14c. Abbreviation sentence segmentation — 1 only-Java

`т. 2 ч. 1` is one sentence in Java (the abbreviation dot does not end the
sentence), so `UkrainianUppercaseSentenceStartRule` reports the lowercase
start (0-1). The Rust engine splits sentences with the shared SRX before
tokenization and segments after `т.`, so no rule sees a lowercase sentence
start:

```
т. 2 ч. 1
Java: UPPERCASE_SENTENCE_START 0-1
Rust: (none)
```

Reproduce:

```sh
scripts/oracle/uk/probe-rule.sh "т. 2 ч. 1" UPPERCASE_SENTENCE_START
```

### 14d. Plural adjective + proper-name list — 1 only-Java / 1 only-Rust

For `молодші Олександр Ірванець, Оксана Луцишина` Java reports the lowercase
sentence start and does **not** report `UK_ADJ_NOUN_INFLECTION_AGREEMENT`; the
Rust engine reports the agreement rule (0-17) instead (the exception helper
branch Java uses here is not ported). Same span, so the overlap filter keeps a
different rule on each side:

```
молодші Олександр Ірванець, Оксана Луцишина
Java: UPPERCASE_SENTENCE_START 0-7
Rust: UK_ADJ_NOUN_INFLECTION_AGREEMENT 0-17
```

Reproduce:

```sh
scripts/oracle/uk/probe-rule.sh "молодші Олександр Ірванець, Оксана Луцишина" UK_ADJ_NOUN_INFLECTION_AGREEMENT UPPERCASE_SENTENCE_START
```

The gate allowance:

```sh
scripts/ci/parity.sh uk
# allowed field diffs: 2/2 UK_ADJ_NOUN_INFLECTION_AGREEMENT
# allowed only-Java: 1/1 UK_PREP_NOUN_INFLECTION_AGREEMENT
# allowed only-Java: 2/2 UPPERCASE_SENTENCE_START
# allowed only-Rust: 1/1 UK_ADJ_NOUN_INFLECTION_AGREEMENT
```

## 15. Arabic (`ar`) remaining corpus residue (14 only-Java / 8 only-Rust / 0 field diffs)

The `ar` corpus (`docs/parity/corpora/ar-examples.jsonl`, 1,045 sentences;
golden `ar-full.{txt,java.tsv}`) reaches **0 field diffs**; the residual match
set is:

| rule | kind | count | cause |
|---|---|---|---|
| `syntax_numeric_0003` | only-Java | 10 | `ArabicNumberPhraseFilter` / `ArabicNumbersWords` not ported (the filter rejects, so the rule is inert) |
| `AR_INFLECTED_ONE_WORD` | only-Java | 1 | rule class not ported |
| `AR_VERB_TRANSITIVE_IINDIRECT` | only-Java | 1 | rule class not ported |
| `HUNSPELL_RULE_AR` | only-Java | 2 | Hunspell range/wrong-split residue (see below) |
| `typo_000_tanwin_nasb` | only-Rust | 3 | secondary rule leaks through where Java's missing `syntax_numeric_0003` match would win the overlap |
| `AR_SIMPLE_REPLACE` | only-Rust | 1 | overlap tie-break where Java keeps the speller |
| `verb_287_yTAlhA_AlqAnwn` | only-Rust | 1 | overlap tie-break where Java keeps the speller |
| `grammar_0000_jar_dual` | only-Rust | 1 | secondary rule leaks through (missing number match) |
| `grammar_0000_jar_plural` | only-Rust | 1 | secondary rule leaks through (missing number match) |
| `number_21to99_majrour_separate_jar` | only-Rust | 1 | secondary rule leaks through (missing number match) |

Pinned Java reproductions (all from the committed corpus/Java golden):

- `syntax_numeric_0003` (Java) on `في مليونان ومئتان وخمسة وأربعون ألفاً
  وسبعمائة وواحد صندوق.` -> match `0-52` with the inflected suggestion
  `في مليونين ومئتين وخمسة وأربعين ألفا وسبعمائة وواحد`; the Rust engine emits
  no match because `ArabicNumberPhraseFilter` is a documented reject
  placeholder. Porting `ArabicNumbersWords` (515 lines) +
  `ArabicNumbersWordsConstants` (357) + `ArabicUnitsHelper` (138) is the
  remaining work; it also resolves the five only-Rust "secondary rule leaks"
  (the number match wins the overlap in Java) and the two speller/`AR_SIMPLE`
  overlap tie-breaks.
- `HUNSPELL_RULE_AR` on `قفل الباب، فهو مقفول`: Java reports the speller at
  `11-20` (`فهو مقفول`), Rust at `15-20` (`مقفول`) — the Hunspell rule's
  wrong-split/leading-token range logic is not reproduced by the Arabic
  sentence-text `LETTER_RUN` wrapper; the match *set* is otherwise identical.
- `AR_INFLECTED_ONE_WORD` (`أبحاث` -> `بحوث`) and
  `AR_VERB_TRANSITIVE_IINDIRECT` (`أفاض في` -> `أفاض إلى`) are the two
  remaining `getRelevantRules` classes (they use `ArabicSynthesizer
  .inflectLemmaLike` and the `enableNewStylePronounTag` tagger mode).

`scripts/ci/parity.sh ar` pins all ten counts exactly with
`--expect-only-java`/`--expect-only-rust`.

## 16. Persian (`fa`) token `\w` is ASCII in Java, Unicode in Rust — resolved

**Verdict: Java is more correct; fixed (D-310).** The former residue was
exactly **258 only-Rust `Bad_ZWNJ`** matches on the `ZWNJ_Connection` correct
examples (e.g. `می‌ایستادند`, `سرشناس‌تر`). Cause: Java compiles
`<token regexp="yes">` with `Pattern.CASE_INSENSITIVE | Pattern.UNICODE_CASE`
but **without** `Pattern.UNICODE_CHARACTER_CLASS` (`StringMatcher.create`), so
`\w` is ASCII `[a-zA-Z_0-9]`; the Rust `lt_pattern` engine compiled with the
`regex` crate's default Unicode mode, so `\w` also matched Persian letters. The
first `Bad_ZWNJ` rule's pattern
`([\.\w۰-۹إأةؤورزژاآدذ،؛,:«»\/@#$٪×*()ـ-]+)‌` therefore matched `می` before a
ZWNJ in Rust but not in Java.

The rule's own short text ("a ZWNJ after punctuation, numbers and **some**
Persian letters is not allowed") and its explicit non-joining letter list show
the `\w` was intended as ASCII: `می‌ایستادند` is **correct** Persian (the ZWNJ
joins the imperfective prefix `می` to the verb), so Rust's extra matches were
false positives, not ZWNJ errors Java misses. Java is more correct.

Fixed in the shared `lt_pattern` translation
(`normalize_java_ascii_classes`, `crates/lt-pattern/src/matcher.rs`): `\w`,
`\W`, `\d`, `\D`, `\s`, `\S` and `\b`/`\B` are rewritten to Java's ASCII
definitions (the `fancy-regex` path uses an ASCII look-around form because it
cannot disable Unicode mode). Real-orthography regression test:
`crates/lt/tests/persian.rs::persian_bad_zwnj_is_ascii_word_only`. The `fa`
gate is now **0 only-Java / 0 only-Rust / 0 field diffs** (no allowance); the
`\w`-using `de`/`en`/`fr`/`pt`/`gl`/`es`/`pl` gates and every other gated
language were re-run green.

Pinned Java reproductions (unchanged):

```sh
scripts/oracle/fa/probe-rule.sh "و‌ارد"          # Bad_ZWNJ 0-2  -> suggestion و
scripts/oracle/fa/probe-rule.sh "می‌ایستادند"    # (no match)
```

## 17. Khmer (`km`) hunspell single-character compounds in `testsug` (3 field diffs)

The `km` corpus (`docs/parity/corpora/km-examples.jsonl`, 66 examples; golden
`km-full.{txt,java.tsv}`) reaches **0 only-Java / 0 only-Rust**. The residual
difference is exactly **3 `HUNSPELL_RULE` suggestion field diffs** on two
words (`បញ` and `មណ`):

| Java suggestions | Rust suggestions |
|---|---|
| `ញប\|ប\|ញ\|បាញ\|…\|បម` | `ប\|ញ\|បាញ\|…\|បម\|បន` |
| `មន\|ម\|ណ\|…\|មៃ\|…\|វណ` | `មន\|ម\|ណ\|…\|មួ` |

Cause: `km_KH.aff` sets `COMPOUNDFLAG a` and `COMPOUNDMIN 1`, so Java's
hunspell accepts single-character compounds (`ញប`, `មៃ`) in
`SuggestMgr::testsug`; the Rust `lt-spell` `sm_checkword` does not, so the
`swapchar`/`extrachar` candidate is dropped and the 15-entry cap then admits
a different last entry (`បន`, `មួ`). The match **set** is identical — only the
suggestion lists differ.

Pinned Java reproductions:

```sh
scripts/oracle/km/probe-rule.sh "មានបញ្ញត្តិជាអារម្មណ៍" HUNSPELL_RULE
# 5-7 ញប|ប|ញ|…   and   23-25 …|មៃ|…
```

The gate allowance (validated exactly):

```sh
scripts/ci/parity.sh km
# only Java: 0; only Rust: 0; field diffs: 0; missing lines: 0
# allowed field diffs: 3/3 HUNSPELL_RULE
```

A future shared fix would make `lt-spell`'s compound acceptance honour
`COMPOUNDMIN 1` single-character compounds in `testsug`; that would need
re-running the `gl`/`da`/`sv`/`nl`/`de`/`en`/`no` speller gates.

