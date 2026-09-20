import org.languagetool.synthesis.FrenchSynthesizer;

/**
 * Prints the Java `FrenchSynthesizer.getSpelledNumber` result (Soros over
 * fr/fr.sor) for each argument: input TAB output.
 *
 * Usage: java SorosProbe <numeral> ...
 */
public class SorosProbe {
  public static void main(String[] args) throws Exception {
    FrenchSynthesizer s = FrenchSynthesizer.INSTANCE;
    for (String a : args) {
      System.out.println(a + "\t" + s.getSpelledNumber(a));
    }
  }
}
