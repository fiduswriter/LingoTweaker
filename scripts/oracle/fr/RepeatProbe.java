import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.patterns.AbstractPatternRule;

/**
 * Prints FRENCH_WORD_REPEAT_RULE matches with the underlying pattern
 * positions and the sentence token view, to debug the `match no="0"`
 * reference resolution.
 *
 * Usage: java RepeatProbe "<text>"
 */
public class RepeatProbe {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("fr");
    JLanguageTool lt = new JLanguageTool(lang);
    String text = args[0];
    String ruleId = args.length > 1 ? args[1] : "FRENCH_WORD_REPEAT_RULE";
    for (AnalyzedTokenReadings tok : lt.getAnalyzedSentence(text).getTokensWithoutWhitespace()) {
      System.out.println(
          "tok " + tok.getStartPos() + "-" + tok.getEndPos() + " " + tok.getToken()
              + " sentStart=" + tok.isSentenceStart()
              + " tags=" + tok.getReadings());
    }
    for (org.languagetool.rules.Rule rule : lt.getAllActiveRules()) {
      if (!(rule instanceof AbstractPatternRule)) {
        continue;
      }
      AbstractPatternRule pr = (AbstractPatternRule) rule;
      if (!ruleId.equals(pr.getId())) {
        continue;
      }
      System.out.print(
          "RULE full=" + pr.getFullId() + " sub=" + pr.getSubId() + " pattern=[");
      for (org.languagetool.rules.patterns.PatternToken tok : pr.getPatternTokens()) {
        System.out.print("'" + tok.getString() + "'pt=" + tok.getPOStag() + " ");
      }
      System.out.println("]");
    }

    for (RuleMatch m : lt.check(text)) {
      if (!ruleId.equals(m.getRule().getId())) {
        continue;
      }
      StringBuilder pat = new StringBuilder();
      if (m.getRule() instanceof AbstractPatternRule) {
        AbstractPatternRule pr = (AbstractPatternRule) m.getRule();
        pat.append(" full=").append(pr.getFullId()).append(" sub=").append(pr.getSubId());
        pat.append(" pattern=[");
        for (org.languagetool.rules.patterns.PatternToken tok : pr.getPatternTokens()) {
          pat.append("'").append(tok.getString()).append("'pt=").append(tok.getPOStag()).append(" ");
        }
        pat.append("]");
      }
      System.out.println(
          "MATCH from=" + m.getFromPos() + " to=" + m.getToPos()
              + " patternFrom=" + m.getPatternFromPos() + " patternTo=" + m.getPatternToPos()
              + " sugg=" + m.getSuggestedReplacements() + pat);
    }
  }
}
