// Prints which Spanish disambiguation rules change the readings of a sentence,
// applied one at a time in the same order as XmlRuleDisambiguator.
//
// Usage: java DebugDisambig <sentence>
import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.tagging.disambiguation.rules.DisambiguationPatternRule;
import org.languagetool.tagging.disambiguation.rules.DisambiguationRuleLoader;
import org.languagetool.rules.patterns.RuleSet;

import java.io.InputStream;
import java.util.List;

public class DebugDisambig {
  public static void main(String[] args) throws Exception {
    String text = args[0];
    Language lang = Languages.getLanguageForShortCode("es");
    JLanguageTool lt = new JLanguageTool(lang);
    InputStream is = JLanguageTool.getDataBroker().getFromResourceDirAsStream("es/disambiguation.xml");
    DisambiguationRuleLoader loader = new DisambiguationRuleLoader();
    List<DisambiguationPatternRule> ruleList = loader.getRules(is, lang, "es/disambiguation.xml");
    RuleSet rs = RuleSet.textHinted(ruleList);
    AnalyzedSentence s = lt.getRawAnalyzedSentence(text);
    System.out.println("RAW " + dump(s));
    for (Object o : rs.rulesForSentence(s)) {
      DisambiguationPatternRule r = (DisambiguationPatternRule) o;
      AnalyzedSentence s2 = r.replace(s);
      String a = dump(s);
      String b = dump(s2);
      if (!a.equals(b)) {
        System.out.println("RULE " + r.getId() + " / " + r.getDescription() + " | " + r.getSubId());
        System.out.println("  BEFORE " + a);
        System.out.println("  AFTER  " + b);
        s = s2;
      }
    }
    System.out.println("FINAL " + dump(s));
  }

  private static String dump(AnalyzedSentence s) {
    StringBuilder sb = new StringBuilder();
    for (AnalyzedTokenReadings t : s.getTokens()) {
      if (t.isWhitespace()) {
        continue;
      }
      sb.append('[').append(t.getToken()).append(':');
      for (int i = 0; i < t.getReadings().size(); i++) {
        AnalyzedToken a = t.getReadings().get(i);
        if (i > 0) {
          sb.append(',');
        }
        sb.append(a.getPOSTag());
      }
      sb.append("] ");
    }
    return sb.toString().trim();
  }
}
