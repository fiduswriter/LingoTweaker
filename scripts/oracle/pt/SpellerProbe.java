import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.spelling.morfologik.MorfologikSpellerRule;

/**
 * Prints `MorfologikPortugueseSpellerRule.getSpellingSuggestions` per
 * argument (one word per line, suggestions joined with `|`), so the output
 * can be diffed against the Rust `spell_probe_pt` example.
 *
 * Usage: java SpellerProbe <pt-PT|pt-BR|pt-AO|pt-MZ> word1 word2 ...
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
