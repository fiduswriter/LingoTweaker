import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.Languages;

/**
 * Prints the disambiguated readings of a sentence's tokens.
 *
 * Usage: java GlReadingsProbe "sentence"
 */
public class GlReadingsProbe {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("gl");
    JLanguageTool lt = new JLanguageTool(lang);
    String sentence = args.length > 0 ? args[0] : "Un vaca está no prado.";
    AnalyzedSentence as = lt.getAnalyzedSentence(sentence);
    for (AnalyzedTokenReadings tr : as.getTokens()) {
      StringBuilder sb = new StringBuilder();
      for (AnalyzedToken at : tr) {
        if (sb.length() > 0) sb.append('|');
        sb.append(at.getToken()).append(':').append(at.getLemma()).append(':').append(at.getPOSTag());
      }
      System.out.println("T\t" + tr.getToken() + "\t" + tr.getStartPos() + "\t" + sb);
    }
  }
}