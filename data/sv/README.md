# Swedish (sv) data

The Swedish tagger/synthesizer dictionaries are built from **SALDO**
(Språkbanken, Göteborgs universitet), replacing the DSSO-derived lexicon the
upstream LanguageTool `sv` module shipped (LGPL-2.1-or-later). Owner-approved
lexicon swap (plan item B1); the divergence from the Java `sv` module's
tagger data is intentional and accepted (the Java golden parity gate only
pins the 45-line corpus, which the rebuilt dictionaries keep at 0/0/0).

## Source

| | |
|---|---|
| Resource | SALDO (Språkbanken), morphological lexicon export `saldom.xml` |
| URL | <https://svn.spraakbanken.gu.se/sb-arkiv/pub/lexikon/saldom/saldom.xml> (development export, 165 MB; `$Id$` rev of 2017, 128,036 LexicalEntry) |
| Official distribution page | <https://sprakbanken.se/en/resources/saldo> (download table lists the SALDO lexicon under **CC-BY-4.0**; the lemma lexicon `saldo.xml` has 131,020 entries) |
| License | **CC BY 4.0** (Språkbanken SALDO; earlier distributions were CC BY 3.0 / LGPL dual-licensed — `saldo_2.3`'s `saldo20v03.txt` header) |
| Attribution | Borin, Lars, Markus Forsberg and Lennart Lönngren 2013. SALDO: a touch of yin to WordNet's yang. *Language Resources and Evaluation* 47(4): 1191–1211. |

The tagger dictionary triples are regenerated in-tree (no vendored
intermediate):

```
python3 tools/sv-dict/build-sv-tagger.py \
    --saldom /tmp/saldom.xml --old-dict /tmp/sv-old-dict.txt \
    --out /tmp/sv-triples.txt --report /tmp/sv-build-report.txt
python3 tools/morfologik/lt_morfologik.py pos \
    -i /tmp/sv-triples.txt --info data/sv/dictionaries/swedish.info \
    -o data/sv/dictionaries/swedish.dict
python3 tools/morfologik/lt_morfologik.py synth \
    -i /tmp/sv-triples.txt --info data/sv/dictionaries/swedish_synth.info \
    -o data/sv/dictionaries/swedish_synth.dict
```

`swedish.info` and `swedish_synth.info` are **unchanged upstream files** (the
tagger `swedish.info` carries the speller replacement-pairs configuration and
must not be regenerated).

## Tag mapping (SALDO -> SUC-style tagset)

The existing SUC-style tagset (`sv/words/tagset.txt`) is kept unchanged: the
32 XML rules in `sv/rules/grammar.xml`, the disambiguator
(`sv/disambiguation.xml`, `<unification>` over number/case/gender) and the
MultiWordChunker all key on it. SALDO's Swedish-terminology inflection
parameters map per feature:

| SALDO (`pos` + `param`) | tagset tag |
|---|---|
| `nn`/`nnm`/`nna`/`nnh`: `sg indef nom/gen` | `NN:OF:SIN:NOM/GEN:<g>` |
| `nn`: `sg def nom/gen` | `NN:BF:SIN:NOM/GEN:<g>` |
| `nn`: `pl indef nom/gen` | `NN:OF:PLU:NOM/GEN:<g>` |
| `nn`: `pl def nom/gen` | `NN:BF:PLU:NOM/GEN:<g>` |
| gender `<inhs>` `u` / `n` / `v`,`p` | `<g>` = `UTR` / `NEU` / `NON` |
| `nl` (numerals): `nom num u/n`, `gen num u/n`, `nom/gen ord *` | `NN:OF:SIN:NOM/GEN:UTR/NEU` |
| `av`: `pos indef sg u` | `JJ:PU` |
| `av`: `pos indef sg n` | `JJ:PN` |
| `av`: `pos indef pl` | `JJ:P` |
| `av`: `pos def sg no_masc`, `pos def pl` | `JJ:BF` |
| `av`: `pos def sg masc` | `JJ:M` (the `-e` definite-masculine form, e.g. `store`) |
| `av`: `komp` | `JJ:K` |
| `av`: `super indef` | `JJ:S` |
| `av`: `super def masc` / `super def no_masc` | `JJ:S:BF:M` / `JJ:S:BF:NM` |
| `av`: `invar` | full `JJ:PU`+`JJ:PN`+`JJ:P`+`JJ:BF` set (DSSO convention for invariant adjectives, e.g. `bra`) |
| `vb`: `inf aktiv` / `inf s-form` | `VB:INF` / `VB:INF:PF` |
| `vb`: `pres ind aktiv` / `pres ind s-form` | `VB:PRS` / `VB:PRS:PF` |
| `vb`: `pret ind aktiv` / `pret ind s-form` | `VB:PRT` / `VB:PRT:PF` |
| `vb`: `sup aktiv` / `sup s-form` | `VB:SUP` / `VB:SUP:PF` |
| `vb`: `imper` | `VB:IMP` |
| `vb`: `pres/pret konj aktiv/s-form` | `VB:KON` |
| `vb`: `pres_part nom` | `VB:PREPC` |
| `vb`: `pret_part indef sg u/n` | `VB:PPC:UTR` / `VB:PPC:NEU` |
| `vb`: `pret_part pl/def` | `VB:PPC:PLU` |
| `pm`: `nom` / `gen` | `PM:NOM` / `PM:GEN` |
| `pn`/`pnm`/`al` (en/den/ett/det/de/dom) | `PN` |
| `pp`/`ppm` | `PP`, `kn`/`knm` -> `KN`, `in/inm` -> `IN`, `sn/snm` -> `KN` |
| `ab`: `invar/komp/super/pos` | `AB` |

Dropped (no rule-relevant readings): verb/adjective genitive slots (the DSSO
dict has no participle or adjective genitive forms either, and SALDO's extra
gen readings would surface as extra `<match>` synth suggestions); the
compound-member slots (`c`, `ci`, `cm`, `sms`) whose forms end in `-`;
per-part indexed forms of multi-word entries (phrase fragments covered by the
standalone lemmas); entries whose grundform is a multi-word expression —
except their **fused single-part variants** (`2:1-1`-style, e.g. the verb
`ladda ur` fusing its particle into `urladdad`/`urladdat`), which are words
in their own right and are emitted under the family's citation form as lemma
so the synthesizer inflects them (`urladdad` + `VB:PPC:NEU` -> `urladdat`).

The seven punctuation entries of the old dict (readings whose tag is the
punctuation character itself) are carried over verbatim so the tagger
behavior for sentence punctuation is unchanged.

## Compound words

SALDO lemmas are non-compound. Unlike the speller (the hunspell `sv_SE.dic`,
which is a separate vendored component and is not affected), the tagger
dictionary previously carried some compound nouns as full lexicon entries
(DSSO was corpus-derived); SALDO carries only lexicalized compounds as
lemmas. The 45-line parity corpus is unaffected (0/0/0 gate); raw text with
unlisted compounds will tag whole compound tokens as unknown words in the
tagger — grammar-rule impact is limited to postag-keyed patterns on compound
head words, and no synthetic compound generation was added.

## Word lists

`sv/words/*` (added/removed/multiwords/common_words/compounds) and
`sv/sv.sor` are unchanged and still apply on top of the rebuilt dictionary
(`added.txt`/`removed.txt` are empty of entries; the manual tagger lists in
`data/sv/words/` load on top of the binary dict as before).