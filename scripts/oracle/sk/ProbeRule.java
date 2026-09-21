import java.util.List;

import org.languagetool.JLanguageTool;
import org.languagetool.Languages;
import org.languagetool.rules.RuleMatch;

/**
 * Checks one Slovak text and prints one line per match:
 * ruleId \t from \t to \t message \t suggestion1|suggestion2...
 * Only the requested rule ids are enabled (including default-off/picky ones).
 *
 * Usage: java ProbeRule "<text>" [ruleId ...]
 */
public class ProbeRule {
  public static void main(String[] args) throws Exception {
    String text = args[0];
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode("sk"));
    if (args.length > 1 && args[1].equals("--readings")) {
      org.languagetool.AnalyzedSentence as = lt.getAnalyzedSentence(text);
      for (org.languagetool.AnalyzedTokenReadings tr : as.getTokens()) {
        StringBuilder sb = new StringBuilder();
        for (org.languagetool.AnalyzedToken at : tr.getReadings()) {
          if (sb.length() > 0) {
            sb.append('|');
          }
          sb.append(at.getLemma()).append(':').append(at.getPOSTag());
        }
        System.out.println(tr.getToken() + "\t" + tr.isIgnoredBySpeller() + "\t" + sb);
      }
      return;
    }
    if (args.length > 1) {
      for (org.languagetool.rules.Rule rule : lt.getAllRules()) {
        if (java.util.Arrays.asList(args).subList(1, args.length).contains(rule.getId())) {
          lt.enableRule(rule.getId());
          continue;
        }
        lt.disableRule(rule.getId());
      }
    }
    List<RuleMatch> matches = lt.check(text, JLanguageTool.Level.PICKY);
    for (RuleMatch m : matches) {
      StringBuilder sugg = new StringBuilder();
      for (String s : m.getSuggestedReplacements()) {
        if (sugg.length() > 0) {
          sugg.append('|');
        }
        sugg.append(s);
      }
      System.out.println(m.getRule().getId() + "\t" + m.getFromPos() + "\t" + m.getToPos()
          + "\t" + m.getMessage().replace("\t", " ") + "\t" + sugg);
    }
  }
}
