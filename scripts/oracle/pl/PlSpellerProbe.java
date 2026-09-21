import org.languagetool.AnalyzedSentence;
import org.languagetool.JLanguageTool;
import org.languagetool.language.Polish;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.spelling.SpellingCheckRule;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;

/**
 * Prints `MorfologikPolishSpellerRule.match` matches per input line:
 * `line\tfrom\tto\tsuggestion1|suggestion2|...` with UTF-16 offsets.
 * Lines without a match are skipped. Used to pin the Rust
 * `PolishSpellingRule` (tokenizing pattern, `isNotCompound`,
 * `pruneSuggestions`) against the legacy engine.
 *
 * Usage: java PlSpellerProbe <sentences.txt>
 *        java PlSpellerProbe --misspelled word1 word2 ...
 */
public class PlSpellerProbe {
  public static void main(String[] args) throws Exception {
    Polish pl = new Polish();
    JLanguageTool lt = new JLanguageTool(pl);
    SpellingCheckRule rule = pl.getDefaultSpellingRule();
    if (args.length > 0 && args[0].equals("--misspelled")) {
      for (int i = 1; i < args.length; i++) {
        System.out.println(args[i] + "\t" + rule.isMisspelled(args[i]));
      }
      return;
    }
    List<String> lines = Files.readAllLines(Paths.get(args[0]));
    for (String line : lines) {
      if (line.trim().isEmpty()) {
        continue;
      }
      AnalyzedSentence as = lt.getAnalyzedSentence(line);
      RuleMatch[] matches = rule.match(as);
      for (RuleMatch m : matches) {
        System.out.println(line + "\t" + m.getFromPos() + "\t" + m.getToPos() + "\t"
            + String.join("|", m.getSuggestedReplacements()));
      }
    }
  }
}
