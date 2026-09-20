import org.languagetool.AnalyzedToken;
import org.languagetool.synthesis.FrenchSynthesizer;

/**
 * Prints `FrenchSynthesizer.synthesize` results for lemma|tag arguments
 * (`lemma|postag|regexp`), one per line, Java `Arrays.toString` style.
 *
 * Usage: java SynthProbe "exister|V ppa m s|false" ...
 */
public class SynthProbe {
  public static void main(String[] args) throws Exception {
    FrenchSynthesizer s = FrenchSynthesizer.INSTANCE;
    for (String a : args) {
      String[] parts = a.split("\\|", -1);
      AnalyzedToken token = new AnalyzedToken(parts[0], parts[1], parts[0]);
      String[] forms = s.synthesize(token, parts[1], Boolean.parseBoolean(parts[2]));
      System.out.println(a + "\t" + String.join(",", forms));
    }
  }
}
