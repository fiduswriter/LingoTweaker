import org.languagetool.language.Italian;
import org.languagetool.rules.spelling.morfologik.MorfologikSpellerRule;

/**
 * Prints `MorfologikItalianSpellerRule.getSpellingSuggestions` per argument
 * (one word per line, suggestions joined with `|`), so the output can be
 * diffed against the Rust `spell_probe_it` example.
 *
 * Usage: java SpellerProbe word1 word2 ...
 */
public class SpellerProbe {
  public static void main(String[] args) throws Exception {
    MorfologikSpellerRule rule =
        (MorfologikSpellerRule) new Italian().getDefaultSpellingRule();
    for (String word : args) {
      System.out.println(word + "\t" + String.join("|", rule.getSpellingSuggestions(word)));
    }
  }
}
