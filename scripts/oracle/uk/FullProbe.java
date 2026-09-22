import java.util.List;

import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Languages;

/**
 * Prints the full JLanguageTool-analyzed readings (preDisambiguate +
 * disambiguate) for each argument, one token per line.
 *
 * Usage: java FullProbe "<text1>" "<text2>" ...
 */
public class FullProbe {
  public static void main(String[] args) throws Exception {
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode("uk"));
    for (String text : args) {
      System.out.println("==" + text);
      List<AnalyzedSentence> sentences = lt.analyzeText(text);
      for (AnalyzedSentence s : sentences) {
        for (AnalyzedTokenReadings atr : s.getTokensWithoutWhitespace()) {
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
}