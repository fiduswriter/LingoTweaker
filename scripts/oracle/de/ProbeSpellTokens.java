import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.List;

import org.languagetool.AnalyzedSentence;
import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.de.GermanSpellerRule;

/**
 * Debug probe: prints the hunspell-tokenized words of a sentence (the
 * `HunspellRule.match` token loop input) and, per token, the ignore /
 * misspelled / ignorePotentiallyMisspelledWord decisions and the resulting
 * speller matches.
 *
 * Usage: java ProbeSpellTokens <sentence>
 */
public class ProbeSpellTokens {
  public static void main(String[] args) throws Exception {
    String text = String.join(" ", args);
    Language lang = Languages.getLanguageForShortCode("de-DE");
    GermanSpellerRule rule = (GermanSpellerRule) lang.getDefaultSpellingRule();
    rule.isMisspelled("x"); // force init
    JLanguageTool lt = new JLanguageTool(lang);
    AnalyzedSentence sentence = lt.getRawAnalyzedSentence(text);

    Method gswi = org.languagetool.rules.Rule.class
        .getDeclaredMethod("getSentenceWithImmunization", AnalyzedSentence.class);
    gswi.setAccessible(true);
    AnalyzedSentence immunized = (AnalyzedSentence) gswi.invoke(rule, sentence);
    for (org.languagetool.AnalyzedTokenReadings atr : immunized.getTokens()) {
      System.out.println("IMM\t" + atr.getToken() + "\timmunized=" + atr.isImmunized()
          + "\tignoredBySpeller=" + atr.isIgnoredBySpeller()
          + "\tsource=" + atr.getImmunizationSourceLine());
    }

    Method without = org.languagetool.rules.spelling.hunspell.HunspellRule.class
        .getDeclaredMethod("getSentenceTextWithoutUrlsAndImmunizedTokens", AnalyzedSentence.class);
    without.setAccessible(true);
    String stripped = (String) without.invoke(rule, sentence);
    System.out.println("STRIPPED\t" + stripped);

    Method tokenize = org.languagetool.rules.spelling.hunspell.HunspellRule.class
        .getDeclaredMethod("tokenizeText", String.class);
    tokenize.setAccessible(true);
    String[] tokens = (String[]) tokenize.invoke(rule, stripped);
    System.out.println("TOKENS\t" + Arrays.toString(tokens));
    for (org.languagetool.AnalyzedTokenReadings atr : sentence.getTokens()) {
      StringBuilder tags = new StringBuilder();
      for (org.languagetool.AnalyzedToken t : atr.getReadings()) {
        tags.append(t.getPOSTag()).append("|");
      }
      System.out.println("ENGINE\t" + atr.getToken() + "\tignoredBySpeller=" + atr.isIgnoredBySpeller()
          + "\timmunized=" + atr.isImmunized()
          + "\tenglishIgnore=" + atr.hasPosTag("_english_ignore_")
          + "\ttags=" + tags);
    }

    Method ignoreList = org.languagetool.rules.spelling.SpellingCheckRule.class
        .getDeclaredMethod("ignoreWord", List.class, int.class);
    ignoreList.setAccessible(true);
    Method ignoreWord = org.languagetool.rules.spelling.SpellingCheckRule.class
        .getDeclaredMethod("ignoreWord", String.class);
    ignoreWord.setAccessible(true);
    Method potentially = org.languagetool.rules.spelling.SpellingCheckRule.class
        .getDeclaredMethod("ignorePotentiallyMisspelledWord", String.class);
    potentially.setAccessible(true);
    for (int i = 0; i < tokens.length; i++) {
      String w = tokens[i];
      boolean ign = (Boolean) ignoreList.invoke(rule, Arrays.asList(tokens), i)
          || (Boolean) ignoreWord.invoke(rule, w);
      boolean mis = rule.isMisspelled(w);
      boolean pot = mis && (Boolean) potentially.invoke(rule, w);
      System.out.println("TOK\t" + i + "\t" + w + "\tignore=" + ign + "\tmisspelled=" + mis
          + "\tpotentialIgnore=" + pot);
    }
    RuleMatch[] matches = rule.match(sentence);
    for (RuleMatch m : matches) {
      System.out.println("MATCH\t" + m.getFromPos() + "\t" + m.getToPos() + "\t"
          + String.join("|", m.getSuggestedReplacements()));
    }
  }
}
