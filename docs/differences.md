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

This file records every case where the Rust engine does **not** exactly match
the Java engine. Each entry has a reproduction and states which side is more
correct and why (see the policy above). Entries that reach exact parity are
deleted rather than kept — the decision log preserves the history. Everything
not listed here matches Java; verify with `scripts/oracle/gate.sh` (2k sample)
and the per-language full-corpus oracle (`scripts/ci/parity.sh <lang>`).

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

## 11. Lithuanian (`lt`): vendored third-party dictionary, no Java baseline

**Verdict: intentional** — the legacy module is broken (it throws on every
check), so there is no Java reference to match; the Rust engine deliberately
vendors a third-party dictionary under the unchanged rule id.

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
## 12. Japanese (`ja`): segmentation-engine substitution (resolved)

Japanese was ported on the `lindera-cjk-spike` branch with a **different
segmenter** than Java: Java uses `net.java.sen` (Sen Viterbi) over the
lucene-gosen IPADIC 2.6.1 artifact, Rust uses Lindera 6 over a compiled
meCab-IPADIC 2.7.0 dictionary. The tagger is the same in both: the segmenter's
`surface/POS/basicForm` triple is split into an `AnalyzedToken`. There is no
disambiguator, chunker, synthesizer or speller, and `Japanese.getRelevantRules`
contributes only `DoublePunctuationRule` and `MultipleWhitespaceRule`.

### 12a. Example coverage — 737/737 (resolved)

The rules were authored against net.java.sen's token boundaries, so an initial
run fired on 690 of the 735 grammar.xml examples. All misses were fixed by
**realigning the affected pattern tokens to Lindera's boundaries** (language-local
edits, no engine change): merged tokens are matched as one (`曖`+`味` →
`曖味`, `忘`+`備` → `忘備`, `さ`+`せる` → `させる`, `来`+`れる` → `来れる`,
`ストリート`+`キング` → `ストリートキング`, …) and split tokens are matched as
several (`三寸` → `三`+`寸`, `未解決` → `未`+`解決`, `だらけ` → `だら`+`け`,
`キリスト教` → `キリスト`+`教`, …). The suggestion strings and rule ids are
unchanged.

Three rules have a second spelling that Lindera tokenizes into a **different
number of tokens**, so one pattern cannot cover both and they were split into
alternatives (`<or>` where both spellings have the same token count, a
`<rulegroup>` with one sub-rule per spelling where they differ):

| rule | variant A | variant B | form |
|---|---|---|---|
| `MATIDOUSII` | 待ちどう**しい** (しᐧい) | 待ちどう**しく** (しく) | `<rulegroup>` (2 sub-rules) |
| `ZURAI` | 読み**ずらい** (ずらᐧい) | 読み**ずらく** (ずᐧらく) | `<or>` at both positions |
| `KURERU` | **来れる** (one token) | **これる** (こᐧれる) | `<rulegroup>` (2 sub-rules) |

This adds one sub-rule + one example for each of the two rulegroups, so the
grammar now loads **737 rule definitions / 737 examples** (the rule *ids*
`MATIDOUSII`/`KURERU` are unchanged; the added variants carry sub-ids `2`).
Probes on all 737 examples fire 737/737 with 0 false positives, and the
corrected forms of every variant do not fire:

```sh
cargo test -p lt --test japanese -- --nocapture   # "error 737 (hit 737, miss 0)"
```

### 12b. Whitespace tokens and localized built-in strings

net.java.sen (and Lindera) drop whitespace from the token stream. The Rust
analyzer re-inserts whitespace **per character** (locating each surface in the
source text) so byte offsets stay exact; Java loses the spaces. Consequently
`MultipleWhitespaceRule` can fire in Rust on Japanese text with repeated spaces
where Java has no whitespace tokens to match:

```
テスト  です。
Rust: WHITESPACE_RULE
Java: (none)
```

Verdict: **intentional: Rust more correct** (the offsets are exact and the rule
sees the real token stream). The localized Japanese built-in strings
(`whitespace_repetition`) are not wired, so the base English messages are used
for `WHITESPACE_RULE` and `DOUBLE_PUNCTUATION`; this affects wording only, not
match sets.

## 13. Chinese (`zh`): segmentation-engine substitution (triage)

Chinese was ported on the `lindera-cjk-spike` branch with a **different
segmenter/tagger** than Java: Java uses HanLP's portable (mini) dictionary via
`ChineseWordTokenizer`/`ChineseTagger`; Rust uses Lindera 6's jieba dictionary
(`data/zh/dictionary`). The tagger reads the POS from the jieba detail field and
leaves the lemma null, like `ChineseTagger` (`new AnalyzedToken(word, pos,
null)`). There is no disambiguator, chunker, synthesizer or speller, and
`Chinese.getRelevantRules` contributes only `DoublePunctuationRule` and
`MultipleWhitespaceRule` (localized with the `MessagesBundle_zh` strings).

Sentence splitting does **not** use SRX: Java's `ChineseSentenceTokenizer`
wraps HanLP's `SentencesUtil.toSentenceList(text)` (shortest units, so it also
breaks at `，,;；` and spaces). The Rust analyzer ports that scan
(`crates/lt/src/zh.rs::split_sentences`).

### 13a. Per-rule triage (2026-09-24) — 1781/1789 error examples (99.6%), held-out 2

Because the Java `zh` checker is itself poor and the segmenter substitution makes
many HanLP-authored patterns unusable, the rules were **triaged one by one**
rather than realigned wholesale. The triage set was the 277 sub-rules where Java
fires but Rust does not (the `zh_diag.json` MISS rows), plus the top-level
`SHI_ADHECTIVE_ERROR#1` and the 18 sub-rules already parked as disabled.

| | before triage | after triage |
|---|---|---|
| error examples firing | 1533 / 1825 (84.0%) | **1781 / 1789 (99.6%)** |
| miss | 292 | 8 |
| false positives on `correct` examples | 18 | 18 |
| held-out Wikipedia prose matches (4,616 sentences) | 5 | **2** |

**Decisions.** Triage set of 291 sub-rules: **MODIFY 254, REMOVE 37** (18 already
parked + 19 newly found), **NEW 0, NO_CHANGE 0**. Eight further upstream-straggler
rules found during the pass were also handled (4 MODIFY, 4 REMOVE); totals over
all touched rules are **MODIFY 258 / REMOVE 41**. Unreliable rules are **deleted
from `grammar.xml`** rather than left disabled (no historical dead weight): the 41
removed rules were 40 rulegroup sub-rules + 1 top-level rule, and the 3
rulegroups left empty were removed too, so the grammar is now **1,822 rule
definitions** (was 1,863). The full per-rule table is in the local (untracked)
attic note `notes/chinese-port-findings.md`.

**MODIFY** is a re-alignment to jieba's **word tokens**, not a semantic rewrite:
the erroneous form is matched as the whole jieba token window that covers it,
including the minimum adjacent context tokens, e.g.

- `[雄][材]` → `<token>雄材大略</token>` (the wrong idiom is one jieba token);
- `再接再 [利|历|励|力]` → `<token>再接再励</token>`;
- `[应][接][不][遐|瑕]` → `<token>应接</token><token>不</token><token regexp="yes">遐|瑕</token>` (`应接` is one token);
- `[大][放][獗][词]` → `<token>大放</token><token>獗</token><token>词</token>`.

Whole-word misspellings are rare in correct prose, so this keeps precision; the
suggestion message is rewritten to the corrected whole span.

**REMOVE** covers the classes that cannot be made correct without HanLP (deleted from the grammar):

| class | examples | why |
|---|---|---|
| error form embedded in a valid word | `CHAN1_SHEN2#1` (`渗水` is a real word), `FAN3_FAN4#5` (`收入` is tagged `v`, so the noun guard breaks) | the pre/post context is chopped by jieba and matching the whole token would flag legitimate text |
| POS/syntax-dependent rules | `wb4#1`–`wb4#4` (`的/地/得`), `wa2#4`, `wa3#1`, `wb2#1/#2/#4/#6/#8/#9`, `BU#1`, `JI_YI#1`, `YU7_YU8#11`, `LING_JIN#3` | jieba has no comparable POS/syntactic confidence; these reliably false-alarm |
| classifier (量词) rules | `wa5#1/#4/#5/#6/#8/#9/#13/#14/#17/#18/#19/#20/#23/#24/#34` | jieba gives no noun-class information; several had measurable false positives in the oracle |
| broken upstream / wrong direction | `SHI_ADHECTIVE_ERROR#1` (3 held-out FPs), `s32#5` (`防止余震` is correct), `s32#15`, `LUO1_LUO2#2` (upstream prefers the rarer `啰哩啰唆` over the standard `啰里啰唆`) | Java's own rule is wrong; re-enabling would hurt precision |

The 8 residual misses are **duplicate rules** whose error example is already
caught under a different rule id (`BEI1_BEI2#2` vs `BINGXINGBUBEI#1`,
`LI3_LI4#3` vs `LI13_LI14#1`, `SHEN5_SHEN6#3` vs `SHENG3_SHENG4#1`,
`CHENG5_CHENG6#1` vs `XIANGFUXIANGCHENG#1`, `LIN5_LIN6#1` vs
`FENGMAOLINJIAO#1`, `MIAO3_MIAO4#2` vs `PIAO3_PIAO4#1`,
`TA1_TA2_TA3#5` vs `SIXINTADI#1`, `XIUYANSHENGYI#1` vs `XIU3_XIU4#1`), so they
are not coverage gaps. The 18 remaining `correct`-example false positives are
pre-existing, active non-typo rules (`wb1`, `wa5#21/#25/#26/#40`, `wa2#1`,
`wb2#5/#12`, `s2#2`, `s5#1/#3`, `ZH3#1`) that are out of this triage set and do
not fire on the held-out corpus.

The two remaining held-out matches are both real: `MAN5_MAN6#1` (`漫骂` →
`谩骂`) and `wb2#10` (`来自于` redundancy, `来自`). Both are deliberately kept.

Reproduce:

```sh
cargo test -p lt --test chinese -- --nocapture   # "error 1789 (hit 1781, miss 8)"
./target/debug/lt-cli check --lang zh --data-dir data --file <heldout> --lines | grep -c '^M'  # 2
```


### 13b. `ChineseConfusionProbabilityRule` not ported

Upstream's `zh` module also ships `ChineseConfusionProbabilityRule`
(`getRelevantLanguageModelRules`), which needs an n-gram language model
(`resource/zh/common_words.txt`, `confusion_sets.txt`). It is only active when
a language model is configured, is not shipped with the data packs and is not
ported.
