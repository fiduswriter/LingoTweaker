import java.io.BufferedReader;
import java.io.FileReader;
import java.util.List;

import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.language.Dutch;
import org.languagetool.tagging.Tagger;
import org.languagetool.tokenizers.Tokenizer;

/**
 * Dumps the raw Dutch tagger readings for each input line so the Rust
 * port can be diffed against the pinned Java build.
 *
 * Usage: java DumpTags <sentences.txt>
 */
public class DumpTags {
  public static void main(String[] args) throws Exception {
    Dutch lang = new Dutch();
    Tagger tagger = lang.createDefaultTagger();
    Tokenizer tokenizer = lang.getWordTokenizer();
    try (BufferedReader br = new BufferedReader(new FileReader(args[0]))) {
      String line;
      while ((line = br.readLine()) != null) {
        if (line.isEmpty()) {
          continue;
        }
        System.out.println("S\t" + line);
        List<String> tokens = tokenizer.tokenize(line);
        List<AnalyzedTokenReadings> readings = tagger.tag(tokens);
        int i = 0;
        for (AnalyzedTokenReadings atr : readings) {
          StringBuilder sb = new StringBuilder();
          sb.append("T\t").append(tokens.get(i)).append('\t');
          for (int j = 0; j < atr.getReadings().size(); j++) {
            AnalyzedToken at = atr.getReadings().get(j);
            if (j > 0) {
              sb.append('|');
            }
            sb.append(at.getLemma()).append(':').append(at.getPOSTag());
          }
          System.out.println(sb);
          i++;
        }
      }
    }
  }
}
