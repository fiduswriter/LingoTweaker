import java.util.List;

import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.Language;
import org.languagetool.Languages;

/**
 * Prints the Ukrainian disambiguated analysis for each argument: one token per
 * line, `token \t lemma:postag;lemma:postag` (empty readings printed as `:`).
 *
 * Usage: java DisambigProbe "<text1>" "<text2>" ...
 */
public class DisambigProbe {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("uk");
    for (String text : args) {
      List<String> tokens = lang.getWordTokenizer().tokenize(text);
      List<AnalyzedTokenReadings> tagged = lang.getTagger().tag(tokens);
      AnalyzedSentence sent = new AnalyzedSentence(tagged.toArray(new AnalyzedTokenReadings[0]));
      AnalyzedSentence dis = lang.getDisambiguator().disambiguate(sent);
      System.out.println("==" + text);
      for (AnalyzedTokenReadings atr : dis.getTokensWithoutWhitespace()) {
        StringBuilder sb = new StringBuilder();
        for (AnalyzedToken at : atr.getReadings()) {
          if (sb.length() > 0) sb.append(';');
          sb.append(at.getLemma() == null ? "" : at.getLemma()).append(':')
            .append(at.getPOSTag() == null ? "" : at.getPOSTag());
        }
        System.out.println(atr.getToken() + "\t" + sb);
      }
    }
  }
}