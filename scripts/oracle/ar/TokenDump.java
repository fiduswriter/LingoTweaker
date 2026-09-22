import java.util.List;

import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.tagging.Tagger;
import org.languagetool.tokenizers.Tokenizer;

/**
 * Prints the Arabic tokenizer's tokens for a sentence, and the first reading
 * token of each (to debug the tashkeel handling in pattern rules).
 *
 * Usage: java TokenDump <sentence>
 */
public class TokenDump {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("ar");
    Tokenizer tokenizer = lang.getWordTokenizer();
    Tagger tagger = lang.getTagger();
    for (String text : args) {
      List<AnalyzedTokenReadings> readings = tagger.tag(tokenizer.tokenize(text));
      for (AnalyzedTokenReadings atr : readings) {
        AnalyzedToken r0 = atr.getReadings().get(0);
        System.out.println("T\t" + atr.getToken() + "\t" + r0.getToken() + "\t"
          + (r0.getLemma() == null ? "" : r0.getLemma()));
      }
    }
  }
}