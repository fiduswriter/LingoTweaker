// Dump the post-disambiguation token stream of the pinned Spanish LT
// (the D lines of Dump.java, es).
//
// Usage: java -cp ... ProbeDisambig <sentences-file>
// Output per line: S <idx> <sentence> then
//   D <tok idx> <surface> <startPos> <FLAGS> readings|<lemma>:<pos>...
import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Languages;
import org.languagetool.chunking.ChunkTag;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;

public class ProbeDisambig {
  public static void main(String[] args) throws Exception {
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode("es"));
    List<String> sentences = Files.readAllLines(Paths.get(args[0]));
    StringBuilder out = new StringBuilder();
    int si = 0;
    for (String sentence : sentences) {
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
        for (AnalyzedToken a : t.getReadings()) {
          out.append('|').append(q(a.getLemma() == null ? "" : a.getLemma()))
              .append(':').append(q(a.getPOSTag() == null ? "" : a.getPOSTag()));
        }
        if (!t.getChunkTags().isEmpty()) {
          out.append("\tchunks");
          for (ChunkTag c : t.getChunkTags()) {
            out.append('|').append(q(c.getChunkTag()));
          }
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
