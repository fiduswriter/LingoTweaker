import java.util.List;

import org.languagetool.JLanguageTool;
import org.languagetool.Languages;
import org.languagetool.rules.RuleMatch;

/**
 * Checks one text with the pinned Icelandic LT and prints one line per
 * match: ruleId \t from \t to \t message \t suggestion1|suggestion2...
 *
 * Usage: java ProbeRule <sv> "<text>" [ruleId ...]
 */
public class ProbeRule {
  public static void main(String[] args) throws Exception {
    String code = args[0];
    String text = args[1];
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode(code));
    if (args.length > 2) {
      // enable only the requested rules (also the default-off ones)
      for (org.languagetool.rules.Rule rule : lt.getAllRules()) {
        if (java.util.Arrays.asList(args).subList(2, args.length).contains(rule.getId())) {
          lt.enableRule(rule.getId());
          continue;
        }
        lt.disableRule(rule.getId());
      }
    }
    // Level.PICKY runs picky-tagged rules too
    List<RuleMatch> matches = lt.check(text, JLanguageTool.Level.PICKY);
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
