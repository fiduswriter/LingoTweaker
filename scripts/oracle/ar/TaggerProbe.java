import java.util.List;

import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.tagging.Tagger;

/**
 * Prints the Arabic tagger's readings for each argument, one line per word:
 * lemma:postag|lemma:postag (empty when untagged). The separator is `|`, not
 * `;`, because the POS tags themselves contain `;`.
 *
 * Usage: java TaggerProbe <word1> <word2> ...
 */
public class TaggerProbe {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("ar");
    Tagger tagger = lang.getTagger();
    for (String word : args) {
      List<AnalyzedTokenReadings> readings = tagger.tag(List.of(word));
      StringBuilder sb = new StringBuilder();
      for (AnalyzedToken at : readings.get(0).getReadings()) {
        if (sb.length() > 0) sb.append('|');
        sb.append(at.getLemma() == null ? "" : at.getLemma()).append(':')
          .append(at.getPOSTag() == null ? "" : at.getPOSTag());
      }
      System.out.println(word + "\t" + sb);
    }
  }
}