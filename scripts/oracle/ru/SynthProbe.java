import java.util.List;

import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.synthesis.Synthesizer;
import org.languagetool.tagging.Tagger;

/**
 * Tags each argument with the Russian tagger and prints every reading, then
 * synthesizes each reading's lemma with its own POS tag and with a caller
 * supplied target tag. Used to debug `<match postag_replace>` and the
 * synthesizer.
 *
 * Usage: java SynthProbe <word> [<word> ...]
 */
public class SynthProbe {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("ru");
    Tagger tagger = lang.getTagger();
    Synthesizer synth = lang.getSynthesizer();
    for (String word : args) {
      List<AnalyzedTokenReadings> readings = tagger.tag(java.util.Collections.singletonList(word));
      for (AnalyzedTokenReadings atr : readings) {
        for (AnalyzedToken t : atr.getReadings()) {
          String lemma = t.getLemma();
          String tag = t.getPOSTag();
          System.out.println("R\t" + word + "\t" + lemma + "\t" + tag);
          if (lemma != null && tag != null) {
            String[] plain = synth.synthesize(new AnalyzedToken("", tag, lemma), tag);
            System.out.println("S\t" + lemma + "\t" + tag + "\t" + String.join("|", plain));
          }
        }
      }
    }
  }
}
