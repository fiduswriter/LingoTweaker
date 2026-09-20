// Print, for one sentence, which Java disambiguation rules match and which
// tokens end up flagged (oracle debugging helper; not part of the parity
// harness). Usage: java -cp ... Probe "<sentence>"
import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.Rule;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.patterns.AbstractTokenBasedRule;
import org.languagetool.rules.patterns.PatternRuleMatcher;
import org.languagetool.rules.patterns.RuleSet;
import org.languagetool.tagging.disambiguation.rules.DisambiguationPatternRule;

import java.lang.reflect.Field;
import java.util.List;

public class Probe {
  public static void main(String[] args) throws Exception {
    String text = String.join(" ", args);
    Language lang = Languages.getLanguageForShortCode("en-US");
    JLanguageTool lt = new JLanguageTool(lang);
    AnalyzedSentence raw = lt.getRawAnalyzedSentence(text);

    Object hybrid = lang.getDisambiguator();
    Field inner = hybrid.getClass().getDeclaredField("disambiguator");
    inner.setAccessible(true);
    Object xml = inner.get(hybrid);
    Field rulesField = xml.getClass().getDeclaredField("disambiguationRules");
    rulesField.setAccessible(true);
    RuleSet ruleSet = (RuleSet) rulesField.get(xml);
    List<Rule> rules = ruleSet.rulesForSentence(raw);
    System.out.println("rules: " + rules.size());
    for (Rule rule : rules) {
      if (!(rule instanceof DisambiguationPatternRule)) {
        continue;
      }
      DisambiguationPatternRule dpr = (DisambiguationPatternRule) rule;
      RuleMatch[] matches =
          new PatternRuleMatcher((AbstractTokenBasedRule) rule, false).match(raw);
      if (matches.length > 0) {
        System.out.print("MATCH " + rule.getId() + " " + dpr.getAction() + " " + matches.length);
        for (RuleMatch m : matches) {
          System.out.print(" [" + m.getFromPos() + "," + m.getToPos() + "]");
        }
        System.out.println();
      }
    }
    AnalyzedSentence dis = lt.getAnalyzedSentence(text);
    int i = 0;
    for (AnalyzedTokenReadings t : dis.getTokensWithoutWhitespace()) {
      System.out.println(
          "TOK " + (i++) + " " + t.getToken() + " ign=" + t.isIgnoredBySpeller());
    }
  }
}
