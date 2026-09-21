# Intentional differences from the Java engine

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
FRENCH_WORD_REPEAT_RULE`, Docker, commit `01d07e1f6165`):

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

## 7. `HUNSPELL_RULE` (Galician) suggestion ranking not ported (83 corpus field diffs)

Unlike the other entries this is a **known limitation, not a correctness
judgement**: the Galician match set is identical to the legacy engine
(0 only-Java / 0 only-Rust over the 717-example corpus), but the
`HUNSPELL_RULE` suggestions differ because the in-tree checker produces them
with a bounded edit-distance search over the `gl_ES.dic` words, while the
legacy engine calls native `hunspell.suggest` (`suggestmgr.cxx` is not
ported). The differences are candidate sets and ordering/casing, e.g.:

- `data/parity/golden/gl-full.txt` line 7, token `Hal`:
  legacy `Cal|Mal|Sal|Tal|Val|Ha|Hala|Hale|Halo|Chal|Haa|Hai|Hao|Han`,
  Rust `Halo`.
- line 7, token `Frank`: legacy `Franxa`, Rust `""` (no candidate within the
  bounded distance).

Reproduce (pinned Java build):

```sh
scripts/oracle/gl/probe-speller.sh Hal Frank VEDRAS
```

This is the only remaining Galician corpus difference and is pinned exactly
as `--expect-field-diffs=HUNSPELL_RULE=83` in `scripts/ci/parity.sh`. Porting
`suggestmgr` (or vendoring a compatible suggestion engine) would remove the
allowance.

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

Pinned Java (`scripts/oracle/es/probe-rule.sh`, Docker, commit `01d07e1f6165`)
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
