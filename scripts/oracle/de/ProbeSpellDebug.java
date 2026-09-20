import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.de.GermanSpellerRule;
import org.languagetool.rules.spelling.hunspell.HunspellRule;

/**
 * Prints `word<TAB>hunspell.spell()<TAB>rule.isMisspelled()` for debugging
 * acceptance differences between the raw dictionary and the rule.
 *
 * Usage: java ProbeSpellDebug <word>...
 */
public class ProbeSpellDebug {
  public static void main(String[] args) throws Exception {
    Language lang = Languages.getLanguageForShortCode("de-DE");
    GermanSpellerRule rule = (GermanSpellerRule) lang.getDefaultSpellingRule();
    rule.isMisspelled("x"); // force init
    java.lang.reflect.Field f = HunspellRule.class.getDeclaredField("hunspell");
    f.setAccessible(true);
    Object dict = f.get(rule);
    java.lang.reflect.Method spell = dict.getClass().getMethod("spell", String.class);
    java.lang.reflect.Method ign = org.languagetool.rules.spelling.SpellingCheckRule.class
        .getDeclaredMethod("ignoreWord", String.class);
    ign.setAccessible(true);
    java.lang.reflect.Method inSet = org.languagetool.rules.spelling.SpellingCheckRule.class
        .getDeclaredMethod("isInIgnoredSet", String.class);
    inSet.setAccessible(true);
    java.lang.reflect.Method noCase = org.languagetool.rules.spelling.SpellingCheckRule.class
        .getDeclaredMethod("isIgnoredNoCase", String.class);
    noCase.setAccessible(true);
    java.lang.reflect.Field wsb = org.languagetool.rules.spelling.SpellingCheckRule.class
        .getDeclaredField("wordsToBeIgnored");
    wsb.setAccessible(true);
    @SuppressWarnings("unchecked")
    java.util.Set<String> ignoreSet = (java.util.Set<String>) wsb.get(rule);
    for (String probe : new String[] {"ok", "jo", "abdruck", "rückwärtslaufen"}) {
      java.util.List<String> hits = ignoreSet.stream()
          .filter(k -> k.equals(probe) || k.equals(probe.toUpperCase()))
          .collect(java.util.stream.Collectors.toList());
      System.out.println("SET[" + probe + "]=" + hits);
    }
    for (String w : args) {
      System.out.println(w + "\t" + spell.invoke(dict, w) + "\t" + rule.isMisspelled(w)
          + "\tignoreWord=" + ign.invoke(rule, w)
          + "\tisInSet=" + inSet.invoke(rule, w)
          + "\tisIgnoredNoCase=" + noCase.invoke(rule, w));
    }
  }
}
