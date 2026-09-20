import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.spelling.morfologik.MorfologikSpellerRule;

/**
 * Prints `MorfologikDutchSpellerRule.getSpellingSuggestions` per
 * argument (one word per line, suggestions joined with `|`), so the output
 * can be diffed against the Rust `spell_probe_nl` example.
 *
 * Usage: java SpellerProbe <nl-NL|nl-BE> word1 word2 ...
 */
public class SpellerProbe {
  public static void main(String[] args) throws Exception {
    String code = args[0];
    Language lang = Languages.getLanguageForShortCode(code);
    MorfologikSpellerRule rule =
        (MorfologikSpellerRule) lang.getDefaultSpellingRule();
    for (int i = 1; i < args.length; i++) {
      System.out.println(args[i] + "\t" + String.join("|", rule.getSpellingSuggestions(args[i])));
    }
  }
}
