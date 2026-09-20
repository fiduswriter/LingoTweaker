import java.lang.reflect.Field;
import java.util.List;
import java.util.function.Supplier;
import java.util.stream.Collectors;

import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.de.GermanSpellerRule;
import org.languagetool.rules.spelling.hunspell.CompoundAwareHunspellRule;
import org.languagetool.rules.spelling.morfologik.MorfologikMultiSpeller;
import org.languagetool.rules.spelling.morfologik.WeightedSuggestion;

/**
 * Dumps the raw `MorfologikMultiSpeller.getSuggestions()` result (word/weight
 * pairs, Java order) of the German speller, plus `isMisspelled` for each
 * input word, so the Rust `lt-spell` morfologik port can be diffed against
 * pinned Java without the rule-level pipeline on top.
 *
 * With `stages <word>` it dumps the `CompoundAwareHunspellRule.getSuggestions`
 * stages (dictionary lists, candidates, de-duplication, filterForLanguage,
 * sortSuggestionByQuality) for debugging one word.
 *
 * Usage: java ProbeMorfo <words.txt> [de-DE|de-AT|de-CH]
 *        java ProbeMorfo x de-DE stages <word>
 */
public class ProbeMorfo {
  public static void main(String[] args) throws Exception {
    String variant = args.length > 1 ? args[1] : "de-DE";
    Language lang = Languages.getLanguageForShortCode(variant);
    GermanSpellerRule rule = (GermanSpellerRule) lang.getDefaultSpellingRule();
    rule.isMisspelled("x"); // force lazy init (hunspell + morfologik)

    Field f = CompoundAwareHunspellRule.class.getDeclaredField("morfoSpeller");
    f.setAccessible(true);
    @SuppressWarnings("unchecked")
    Supplier<MorfologikMultiSpeller> supplier = (Supplier<MorfologikMultiSpeller>) f.get(rule);
    MorfologikMultiSpeller speller = supplier.get();

    if (args.length > 3 && args[2].equals("stages")) {
      stages(rule, speller, lang, args[3]);
      return;
    }

    for (String line : java.nio.file.Files.readAllLines(java.nio.file.Paths.get(args[0]))) {
      if (line.isEmpty()) continue;
      boolean misspelled = rule.isMisspelled(line);
      List<WeightedSuggestion> weighted = speller.getWeightedSuggestionsFromDefaultDicts(line);
      List<String> suggestions = weighted.stream()
          .map(s -> s.getWord() + "/" + s.getWeight())
          .collect(Collectors.toList());
      System.out.println(line + "\t" + misspelled + "\t" + String.join("|", suggestions));
    }
  }

  private static void stages(GermanSpellerRule rule, MorfologikMultiSpeller speller,
      Language lang, String word) throws Exception {
    List<String> noSplit = speller.getWeightedSuggestionsFromDefaultDicts(word).stream()
        .map(WeightedSuggestion::getWord).distinct().collect(Collectors.toList());
    List<String> noSplitLc = org.languagetool.tools.StringTools.startsWithUppercase(word)
        && !org.languagetool.tools.StringTools.isAllUppercase(word)
            ? speller.getWeightedSuggestionsFromDefaultDicts(word.toLowerCase()).stream()
                .map(WeightedSuggestion::getWord).distinct().collect(Collectors.toList())
            : new java.util.ArrayList<>();
    java.lang.reflect.Method candM = GermanSpellerRule.class
        .getDeclaredMethod("getCandidates", String.class);
    candM.setAccessible(true);
    @SuppressWarnings("unchecked")
    List<String> cands = (List<String>) candM.invoke(rule, word);
    java.lang.reflect.Method correctM = CompoundAwareHunspellRule.class
        .getDeclaredMethod("getCorrectWords", List.class);
    correctM.setAccessible(true);
    @SuppressWarnings("unchecked")
    List<String> simple = (List<String>) correctM.invoke(rule, cands);
    java.lang.reflect.Method filterM = GermanSpellerRule.class
        .getDeclaredMethod("filterForLanguage", List.class);
    filterM.setAccessible(true);

    List<String> suggestions = new java.util.ArrayList<>();
    int max = Math.max(simple.size(), Math.max(noSplit.size(), noSplitLc.size()));
    for (int i = 0; i < max; i++) {
      if (i < noSplit.size()) suggestions.add(noSplit.get(i));
      if (i < noSplitLc.size()) {
        suggestions.add(org.languagetool.tools.StringTools.uppercaseFirstChar(noSplitLc.get(i)));
      }
      if (i < simple.size()) suggestions.add(simple.get(i));
    }
    System.out.println("NO_SPLIT\t" + String.join("|", noSplit));
    System.out.println("NO_SPLIT_LC\t" + String.join("|", noSplitLc));
    System.out.println("SIMPLE\t" + String.join("|", simple));
    System.out.println("MIXED\t" + String.join("|", suggestions));
    suggestions = new java.util.ArrayList<>(new java.util.LinkedHashSet<>(suggestions));
    System.out.println("DEDUPED\t" + String.join("|", suggestions));
    filterM.invoke(rule, suggestions);
    System.out.println("FILTERED_LANG\t" + String.join("|", suggestions));
    java.lang.reflect.Method sortM = GermanSpellerRule.class
        .getDeclaredMethod("sortSuggestionByQuality", String.class, List.class);
    sortM.setAccessible(true);
    @SuppressWarnings("unchecked")
    List<String> sorted = (List<String>) sortM.invoke(rule, word, suggestions);
    System.out.println("SORTED\t" + String.join("|", sorted));
    for (org.languagetool.AnalyzedTokenReadings r : lang.getTagger().tag(new java.util.ArrayList<>(sorted))) {
      StringBuilder lemmas = new StringBuilder();
      for (org.languagetool.AnalyzedToken t : r.getReadings()) {
        if (t.getLemma() != null) lemmas.append(t.getLemma()).append(":");
      }
      System.out.println("TAG\t" + r.getToken() + "\t"
          + (r.getAnalyzedToken(0).getLemma() == null ? "" : r.getAnalyzedToken(0).getLemma())
          + "\tLEMMAS=" + lemmas);
    }
  }
}
