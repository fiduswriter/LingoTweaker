import java.io.BufferedReader;
import java.io.FileReader;
import java.util.List;
import java.util.stream.Collectors;

import org.languagetool.AnalyzedSentence;
import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.de.GermanSpellerRule;

/**
 * Dumps the German speller's isMisspelled state and the suggestions of the
 * rule matches (the final pipeline incl. curated/additional suggestions) for
 * each input word, so the Rust port can be diffed against pinned Java.
 *
 * Usage: java ProbeSpeller <words.txt> [de-DE|de-AT|de-CH]
 */
public class ProbeSpeller {
  public static void main(String[] args) throws Exception {
    String variant = args.length > 1 ? args[1] : "de-DE";
    Language lang = Languages.getLanguageForShortCode(variant);
    GermanSpellerRule rule = (GermanSpellerRule) lang.getDefaultSpellingRule();
    JLanguageTool lt = new JLanguageTool(lang);
    try (BufferedReader br = new BufferedReader(new FileReader(args[0]))) {
      String line;
      while ((line = br.readLine()) != null) {
        if (line.isEmpty()) continue;
        boolean misspelled = rule.isMisspelled(line);
        AnalyzedSentence sentence = lt.getRawAnalyzedSentence(line);
        RuleMatch[] matches = rule.match(sentence);
        String sugg = "";
        if (matches.length > 0) {
          sugg = matches[0].getSuggestedReplacements().stream().collect(Collectors.joining("|"));
        }
        String ranges = java.util.Arrays.stream(matches)
            .map(m -> m.getFromPos() + "-" + m.getToPos())
            .collect(Collectors.joining(","));
        System.out.println(line + "\t" + misspelled + "\t" + ranges + "\t" + sugg);
      }
    }
  }
}
