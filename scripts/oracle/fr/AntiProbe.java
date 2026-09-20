import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.patterns.AbstractPatternRule;

/**
 * Debug probe for the cross-rule sentence mutation behind
 * docs/differences.md #4 (SUJET_AUXILIAIRE false positive on
 * "Je me demande ou je pourrais partir en vacances.").
 *
 * Usage: java AntiProbe "<text>" <ruleId> [subId]
 *
 * - prints the `JLanguageTool.check` matches of `ruleId`
 * - prints, for every rule in `getAllActiveRules()` order, the sentence
 *   readings whenever a rule's `match` mutated the shared sentence, plus the
 *   `ruleId` match length at that point
 * - with `subId`, prints one specific subrule's match length on a fresh
 *   sentence (isolated) and the mutated original afterwards
 */
public class AntiProbe {
  public static void main(String[] args) throws Exception {
    String text = args[0];
    String ruleId = args[1];
    Language lang = Languages.getLanguageForShortCode("fr");
    JLanguageTool lt = new JLanguageTool(lang);
    AnalyzedSentence sentence = lt.getAnalyzedSentence(text);

    if (args.length > 2) {
      for (org.languagetool.rules.Rule r : lt.getAllActiveRules()) {
        if (r instanceof AbstractPatternRule && ruleId.equals(r.getId())
            && args[2].equals(((AbstractPatternRule) r).getSubId())) {
          System.out.println("before=" + dumpReadings(sentence));
          System.out.println("isolatedMatches=" + r.match(sentence.copy(sentence)).length);
          System.out.println("after =" + dumpReadings(sentence));
        }
      }
      return;
    }

    for (org.languagetool.rules.RuleMatch m : lt.check(text)) {
      if (ruleId.equals(m.getRule().getId())) {
        System.out.println("check MATCH sub="
            + ((AbstractPatternRule) m.getRule()).getSubId() + " "
            + m.getFromPos() + "-" + m.getToPos() + " " + m.getSuggestedReplacements());
      }
    }
    AnalyzedSentence analyzed = lt.analyzeText(text).get(0);
    String prev = dumpReadings(analyzed);
    for (org.languagetool.rules.Rule r : lt.getAllActiveRules()) {
      if (r instanceof org.languagetool.rules.TextLevelRule
          || r instanceof org.languagetool.rules.RemoteRule) {
        continue;
      }
      r.match(analyzed);
      String now = dumpReadings(analyzed);
      if (!now.equals(prev)) {
        System.out.println("MUTATED by " + r.getId() + " sub="
            + (r instanceof AbstractPatternRule ? ((AbstractPatternRule) r).getSubId() : "?")
            + " -> " + now);
        prev = now;
      }
      if (ruleId.equals(r.getId())) {
        System.out.println("inOrder sub=" + ((AbstractPatternRule) r).getSubId()
            + " len=" + r.match(analyzed).length);
      }
    }
  }

  private static String dumpReadings(AnalyzedSentence s) {
    StringBuilder sb = new StringBuilder();
    for (AnalyzedTokenReadings t : s.getTokensWithoutWhitespace()) {
      sb.append(t.getToken()).append(t.isImmunized() ? "!" : "").append('{');
      for (AnalyzedToken a : t.getReadings()) {
        sb.append(a.getLemma()).append(':').append(a.getPOSTag()).append(',');
      }
      sb.append("} ");
    }
    return sb.toString();
  }
}
