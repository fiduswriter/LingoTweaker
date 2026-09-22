import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.Rule;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.spelling.morfologik.MorfologikSpellerRule;

/**
 * Prints the Ukrainian Morfologik speller's acceptance and match() output per
 * argument, so it can be diffed against the Rust rule.
 *
 * Usage: java SpellerProbe <word1> <word2> ...
 */
public class SpellerProbe {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("uk");
    Rule rule = lang.getDefaultSpellingRule();
    MorfologikSpellerRule morfo = (MorfologikSpellerRule) rule;
    for (String word : args) {
      String sugg = String.join("|", morfo.getSpellingSuggestions(word));
      System.out.println("S\t" + word + "\t" + morfo.isMisspelled(word) + "\t" + sugg);
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