// CheckDump variant with Java levels/rule enabling (picky/temp-off rules).
// Usage: CheckDumpPicky <sentences-file> [picky] [enableRule ...]
import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.patterns.AbstractPatternRule;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Comparator;
import java.util.List;

public class CheckDumpPicky {
  public static void main(String[] args) throws Exception {
    List<String> sentences = Files.readAllLines(Paths.get(args[0]));
    String langCode = System.getenv("LT_LANG") != null ? System.getenv("LT_LANG") : "en-US";
    Language en = Languages.getLanguageForShortCode(langCode);
    JLanguageTool lt = new JLanguageTool(en);
    JLanguageTool.Level level = JLanguageTool.Level.DEFAULT;
    for (int i = 1; i < args.length; i++) {
      if (args[i].equals("picky")) {
        level = JLanguageTool.Level.PICKY;
      } else {
        lt.enableRule(args[i]);
      }
    }
    StringBuilder out = new StringBuilder();
    int lineno = 0;
    for (String line : sentences) {
      lineno++;
      line = line.replace("\r", "");
      if (line.isEmpty()) {
        continue;
      }
      out.append("L\t").append(lineno).append('\t').append(q(line)).append('\n');
      List<RuleMatch> matches = lt.check(line, level);
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
