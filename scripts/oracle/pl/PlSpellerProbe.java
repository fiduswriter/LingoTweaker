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
 */
public class PlSpellerProbe {
  public static void main(String[] args) throws Exception {
    Polish pl = new Polish();
    JLanguageTool lt = new JLanguageTool(pl);
    SpellingCheckRule rule = pl.getDefaultSpellingRule();
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
