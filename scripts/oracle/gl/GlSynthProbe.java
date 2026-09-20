import org.languagetool.AnalyzedToken;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.synthesis.Synthesizer;

/**
 * Calls the Galician synthesizer directly, to tell which lemma/tag pairs the
 * legacy engine can synthesize (used to debug `<match postag_replace>`).
 *
 * Usage: java GlSynthProbe
 */
public class GlSynthProbe {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("gl");
    Synthesizer synth = lang.getSynthesizer();
    String[][] cases = {
        {"un", "Z0MS0", "Z0FS0"},
        {"Un", "Z0MS0", "Z0FS0"},
        {"un", "DI0MS0", "DI0FS0"},
        {"vaca", "NCFS000", "NCMS000"},
        {"necesario", "AQ0MS0", "AQ0FS0"},
    };
    for (String[] c : cases) {
      AnalyzedToken t = new AnalyzedToken("", c[1], c[0]);
      String[] plain = synth.synthesize(t, c[2]);
      String[] regexp = synth.synthesize(t, c[2], true);
      System.out.println("synth " + c[0] + "|" + c[1] + " -> " + c[2]
          + " plain=" + String.join("|", plain)
          + " regexp=" + String.join("|", regexp));
    }
  }
}