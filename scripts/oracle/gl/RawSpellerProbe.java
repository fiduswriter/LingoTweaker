import java.lang.reflect.Field;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.Rule;
import org.languagetool.rules.spelling.hunspell.HunspellRule;
import org.languagetool.rules.spelling.hunspell.HunspellDictionary;

/**
 * Calls the raw HunspellDictionary (bypassing the SpellingCheckRule ignore
 * lists) for each argument, to tell an acceptance difference from an
 * ignore-list difference.
 *
 * Usage: java RawSpellerProbe <word1> <word2> ...
 */
public class RawSpellerProbe {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("gl");
    Rule rule = lang.getDefaultSpellingRule();
    // trigger HunspellRule.init()
    ((HunspellRule) rule).isMisspelled("\u0000init");
    Field f = HunspellRule.class.getDeclaredField("hunspell");
    f.setAccessible(true);
    Object dict = f.get(rule);
    for (String word : args) {
      HunspellDictionary d = (HunspellDictionary) dict;
      System.out.println("R\t" + word + "\t" + d.spell(word));
    }
  }
}
