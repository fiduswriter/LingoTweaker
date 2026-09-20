import java.util.List;

import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.rules.fr.FindSuggestionsFilter;

/**
 * Prints the real `FindSuggestionsFilter.getSpellingSuggestions` output for a
 * token (surface|postag args), so the Rust filter path can be compared
 * against the real Java one.
 *
 * Usage: java FilterProbe <surface> <postag>
 */
public class FilterProbe {
  static class Exposed extends FindSuggestionsFilter {
    Exposed() throws Exception {
      super();
    }

    List<String> spell(AnalyzedTokenReadings atr) throws Exception {
      return getSpellingSuggestions(atr);
    }
  }

  public static void main(String[] args) throws Exception {
    Exposed f = new Exposed();
    AnalyzedTokenReadings atr =
        new AnalyzedTokenReadings(new AnalyzedToken(args[0], args[1], args[0]));
    List<String> suggestions = f.spell(atr);
    System.out.println(String.join("|", suggestions));
  }
}
