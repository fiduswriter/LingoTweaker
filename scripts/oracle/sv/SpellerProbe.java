import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.Rule;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.spelling.hunspell.HunspellRule;

/**
 * Prints the Galician HunspellRule's acceptance and match() output per
 * argument, so it can be diffed against the Rust rule.
 *
 * Usage: java SpellerProbe <word1> <word2> ...
 */
public class SpellerProbe {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("sv");
    Rule rule = lang.getDefaultSpellingRule();
    HunspellRule hun = (HunspellRule) rule;
    for (String word : args) {
      String sugg = String.join("|", hun.getSuggestions(word));
      System.out.println("S\t" + word + "\t" + hun.isMisspelled(word) + "\t" + sugg);
      AnalyzedToken start = new AnalyzedToken("", "SENT_START", null);
      AnalyzedToken at = new AnalyzedToken(word, null, null);
      AnalyzedSentence sent = new AnalyzedSentence(new AnalyzedTokenReadings[]{
          new AnalyzedTokenReadings(start, 0), new AnalyzedTokenReadings(at, 0)});
      RuleMatch[] matches = rule.match(sent);
      StringBuilder m = new StringBuilder();
      for (RuleMatch rm : matches) {
        if (m.length() > 0) m.append(';');
        m.append(rm.getFromPos()).append('-').append(rm.getToPos()).append(':')
         .append(rm.getMessage()).append(':')
         .append(String.join("|", rm.getSuggestedReplacements()));
      }
      System.out.println("M\t" + word + "\t" + m);
    }
  }
}