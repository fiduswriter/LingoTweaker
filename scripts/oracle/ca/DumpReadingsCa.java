// Dump the Java Catalan pipeline per-token readings as TSV, matching
// `lt-cli analyze` for oracle diffs (the format of scripts/oracle/Dump.java's
// `D` lines without the OpenNLP `C` lines).
//
// Usage: java DumpReadingsCa <ca-ES|ca-ES-balear|ca-ES-valencia> <sentences-file>
// Output per sentence: S <idx> <sentence>, then
//   D <tok idx> <surface> <startPos> <FLAGS> readings|... chunks|...
import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Languages;
import org.languagetool.chunking.ChunkTag;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;

public class DumpReadingsCa {
  public static void main(String[] args) throws Exception {
    String code = args[0];
    List<String> sentences = Files.readAllLines(Paths.get(args[1]));
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode(code));
    StringBuilder out = new StringBuilder();
    int si = 0;
    for (String sentence : sentences) {
      if (sentence.isEmpty()) {
        continue;
      }
      si++;
      out.append("S\t").append(si).append('\t').append(q(sentence)).append('\n');
      AnalyzedSentence analyzed = lt.getAnalyzedSentence(sentence);
      int i = 0;
      for (AnalyzedTokenReadings t : analyzed.getTokens()) {
        out.append("D\t").append(i++).append('\t')
            .append(q(t.getToken())).append('\t').append(t.getStartPos()).append('\t');
        StringBuilder flags = new StringBuilder();
        if (t.isWhitespaceBefore()) flags.append(" WSB");
        if (t.isSentenceStart()) flags.append(" SS");
        if (t.isSentenceEnd()) flags.append(" SE");
        if (t.isImmunized()) flags.append(" IMM");
        if (t.isIgnoredBySpeller()) flags.append(" IGN");
        if (t.hasTypographicApostrophe()) flags.append(" TYP");
        out.append(flags.toString().trim());
        out.append("\treadings");
        for (AnalyzedToken a : t) {
          out.append('|');
          if (a.getLemma() != null) {
            out.append(a.getLemma());
          }
          out.append(':').append(a.getPOSTag());
        }
        out.append("\tchunks");
        for (ChunkTag chunk : t.getChunkTags()) {
          out.append('|').append(chunk);
        }
        out.append('\n');
      }
    }
    System.out.print(out);
  }

  private static String q(String s) {
    return s.replace("\t", "\\t").replace("\n", "\\n");
  }
}
