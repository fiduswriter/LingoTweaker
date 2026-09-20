import java.util.List;

import org.languagetool.JLanguageTool;
import org.languagetool.Languages;
import org.languagetool.rules.RuleMatch;

/**
 * Checks one text with the pinned Italian LT and prints one line per match:
 * ruleId \t from \t to \t message \t suggestion1|suggestion2...
 *
 * Usage: java ProbeRule "<text>" [ruleId ...]
 *        java ProbeRule --picky "<text>" [ruleId ...]
 */
public class ProbeRule {
  public static void main(String[] args) throws Exception {
    boolean picky = args[0].equals("--picky");
    int base = picky ? 1 : 0;
    String text = args[base];
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode("it"));
    if (args.length > base + 1) {
      // enable only the requested rules (also the default-off ones)
      for (org.languagetool.rules.Rule rule : lt.getAllRules()) {
        if (java.util.Arrays.asList(args).subList(base + 1, args.length).contains(rule.getId())) {
          lt.enableRule(rule.getId());
          continue;
        }
        lt.disableRule(rule.getId());
      }
    }
    List<RuleMatch> matches = picky ? lt.check(text, JLanguageTool.Level.PICKY) : lt.check(text);
    for (RuleMatch m : matches) {
      StringBuilder sugg = new StringBuilder();
      for (String s : m.getSuggestedReplacements()) {
        if (sugg.length() > 0) sugg.append('|');
        sugg.append(s);
      }
      System.out.println(m.getRule().getId() + "\t" + m.getFromPos() + "\t" + m.getToPos()
          + "\t" + m.getMessage().replace("\t", " ") + "\t" + sugg);
    }
  }
}
