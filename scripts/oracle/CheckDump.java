// Dump Java LanguageTool check() matches as TSV, matching
// `lt-cli check --lines` for exact match-set parity diffs.
//
// Usage: java -cp ... CheckDump <sentences-file>
// One sentence per line; output:
//   L	<line no>	<sentence>
//   M	<line no>	<rule id>	<sub id>	<from>	<to>	<type>	<message>	<suggestions joined by |>
import org.languagetool.JLanguageTool;
import org.languagetool.Languages;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.patterns.AbstractPatternRule;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Comparator;
import java.util.List;

public class CheckDump {
  public static void main(String[] args) throws Exception {
    List<String> sentences = Files.readAllLines(Paths.get(args[0]));
    String langCode = System.getenv("LT_LANG") != null ? System.getenv("LT_LANG") : "en-US";
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode(langCode));
    StringBuilder out = new StringBuilder();
    int lineno = 0;
    for (String line : sentences) {
      lineno++;
      line = line.replace("\r", "");
      if (line.isEmpty()) {
        continue;
      }
      out.append("L\t").append(lineno).append('\t').append(q(line)).append('\n');
      List<RuleMatch> matches = lt.check(line);
      matches.sort(Comparator
          .comparingInt(RuleMatch::getFromPos)
          .thenComparingInt(RuleMatch::getToPos)
          .thenComparing(m -> m.getRule().getId())
          .thenComparing(m -> m.getMessage()));
      for (RuleMatch m : matches) {
        String subId = m.getRule() instanceof AbstractPatternRule
            ? ((AbstractPatternRule) m.getRule()).getSubId()
            : "";
        out.append("M\t").append(lineno).append('\t')
            .append(m.getRule().getId()).append('\t')
            .append(subId == null ? "" : subId).append('\t')
            .append(m.getFromPos()).append('\t')
            .append(m.getToPos()).append('\t')
            .append(m.getType().name()).append('\t')
            .append(q(m.getMessage())).append('\t')
            .append(q(String.join("|", m.getSuggestedReplacements())))
            .append('\n');
      }
    }
    System.out.print(out);
  }

  private static String q(String s) {
    return s == null ? "" : s.replace("\t", "\\t").replace("\n", "\\n");
  }
}
