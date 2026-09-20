import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Languages;

/**
 * Dumps the pre-disambiguation (`getPreDisambigTokensWithoutWhitespace`) and
 * final (`getTokensWithoutWhitespace`) readings of a French sentence, so the
 * Rust `raw_pos` implementation can be pinned against the Java reference.
 *
 * Usage: java ProbeRawPos <sentences.txt>
 */
public class ProbeRawPos {
  public static void main(String[] args) throws Exception {
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode("es"));
    java.io.BufferedReader br = new java.io.BufferedReader(new java.io.FileReader(args[0]));
    String line;
    while ((line = br.readLine()) != null) {
      if (line.isEmpty()) {
        continue;
      }
      System.out.println("S\t" + line);
      AnalyzedSentence s = lt.getAnalyzedSentence(line);
      AnalyzedTokenReadings[] pre = s.getPreDisambigTokensWithoutWhitespace();
      AnalyzedTokenReadings[] post = s.getTokensWithoutWhitespace();
      for (int i = 0; i < Math.max(pre.length, post.length); i++) {
        System.out.println("PRE\t" + i + "\t" + (i < pre.length ? dump(pre[i]) : "(none)"));
        System.out.println("POST\t" + i + "\t" + (i < post.length ? dump(post[i]) : "(none)"));
      }
    }
  }

  private static String dump(AnalyzedTokenReadings atr) {
    StringBuilder sb = new StringBuilder();
    sb.append('[').append(atr.getToken()).append(']');
    if (atr.isImmunized()) {
      sb.append("{IMMUNIZED}");
    }
    sb.append(' ');
    for (AnalyzedToken at : atr.getReadings()) {
      sb.append(at.getLemma()).append(':').append(at.getPOSTag()).append(' ');
    }
    if (atr.getChunkTags() != null && !atr.getChunkTags().isEmpty()) {
      sb.append("chunks=").append(atr.getChunkTags());
    }
    return sb.toString();
  }
}
