import java.util.Arrays;
import java.util.List;

import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.Rule;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.spelling.SpellingCheckRule;

/**
 * Prints the Russian Morfologik speller's acceptance and match() output per
 * argument, so it can be diffed against the Rust rule.
 *
 * Usage:
 *   java SpellerProbe <word1> <word2> ...            # default speller
 *   java SpellerProbe --rule <id> <word1> ...        # one rule (default-off ok)
 */
public class SpellerProbe {
  public static void main(String[] args) throws Exception {
    String ruleId = null;
    if (args.length >= 2 && args[0].equals("--rule")) {
      ruleId = args[1];
      args = Arrays.copyOfRange(args, 2, args.length);
    }
    Language lang = Languages.getLanguageForShortCode("ru");
    JLanguageTool lt = new JLanguageTool(lang);
    Rule rule = null;
    if (ruleId == null) {
      rule = lang.getDefaultSpellingRule();
    } else {
      for (Rule r : lt.getAllRules()) {
        if (r.getId().equals(ruleId)) {
          rule = r;
          lt.enableRule(r.getId());
        } else {
          lt.disableRule(r.getId());
        }
      }
    }
    SpellingCheckRule spell = (SpellingCheckRule) rule;
    for (String word : args) {
      System.out.println("S\t" + word + "\t" + spell.isMisspelled(word));
      if (ruleId == null) {
        AnalyzedToken start = new AnalyzedToken("", "SENT_START", null);
        AnalyzedToken at = new AnalyzedToken(word, null, null);
        AnalyzedSentence sent = new AnalyzedSentence(new AnalyzedTokenReadings[]{
            new AnalyzedTokenReadings(start, 0), new AnalyzedTokenReadings(at, 0)});
        printMatches(word, rule.match(sent));
      } else {
        printMatches(word, lt.check(word));
      }
    }
  }

  private static void printMatches(String word, List<RuleMatch> matches) {
    StringBuilder m = new StringBuilder();
    for (RuleMatch rm : matches) {
      if (m.length() > 0) m.append(';');
      m.append(rm.getFromPos()).append('-').append(rm.getToPos()).append(':')
       .append(rm.getMessage()).append(':')
       .append(String.join("|", rm.getSuggestedReplacements()));
    }
    System.out.println("M\t" + word + "\t" + m);
  }

  private static void printMatches(String word, RuleMatch[] matches) {
    printMatches(word, Arrays.asList(matches));
  }
}
