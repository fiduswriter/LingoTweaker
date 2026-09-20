// Whole-text variant of CheckDump/CheckDumpPicky: the file content is checked
// as ONE text (multi-paragraph, for text-level/paragraph rules), not line by
// line. A single trailing line terminator is stripped so the checked text
// matches the file content the Rust side sees.
//
// Usage: CheckDumpText <text-file> [picky] [enableRule ...]
// Output: same TSV as CheckDump, with line number 1.
import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.Languages;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.patterns.AbstractPatternRule;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Comparator;
import java.util.List;

public class CheckDumpText {
  public static void main(String[] args) throws Exception {
    byte[] raw = Files.readAllBytes(Paths.get(args[0]));
    String text = new String(raw, "UTF-8");
    if (text.endsWith("\n")) {
      text = text.substring(0, text.length() - 1);
    }
    if (text.endsWith("\r")) {
      text = text.substring(0, text.length() - 1);
    }
    Language en = Languages.getLanguageForShortCode("en-US");
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
    out.append("L\t1\t").append(q(text)).append('\n');
    List<RuleMatch> matches = lt.check(text, level);
    matches.sort(Comparator
        .comparingInt(RuleMatch::getFromPos)
        .thenComparingInt(RuleMatch::getToPos)
        .thenComparing(m -> m.getRule().getId())
        .thenComparing(m -> m.getMessage()));
    for (RuleMatch m : matches) {
      String subId = m.getRule() instanceof AbstractPatternRule
          ? ((AbstractPatternRule) m.getRule()).getSubId()
          : "";
      out.append("M\t1\t")
          .append(m.getRule().getId()).append('\t')
          .append(subId == null ? "" : subId).append('\t')
          .append(m.getFromPos()).append('\t')
          .append(m.getToPos()).append('\t')
          .append(m.getType().name()).append('\t')
          .append(q(m.getMessage())).append('\t')
          .append(q(String.join("|", m.getSuggestedReplacements())))
          .append('\n');
    }
    System.out.print(out);
  }

  private static String q(String s) {
    return s == null ? "" : s.replace("\t", "\\t").replace("\n", "\\n");
  }
}
