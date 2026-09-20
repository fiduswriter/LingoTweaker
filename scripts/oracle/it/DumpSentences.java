import java.io.BufferedReader;
import java.io.FileReader;
import java.util.List;

import org.languagetool.language.Italian;
import org.languagetool.tokenizers.SentenceTokenizer;

/**
 * Dumps the Italian SRX sentence split per input line (one `S<TAB>sentence`
 * per split part), mirroring the sentence ranges of `lt-cli check --json`.
 *
 * Usage: java DumpSentences <sentences.txt>
 */
public class DumpSentences {
  public static void main(String[] args) throws Exception {
    Italian lang = new Italian();
    SentenceTokenizer tokenizer = lang.createDefaultSentenceTokenizer();
    try (BufferedReader br = new BufferedReader(new FileReader(args[0]))) {
      String line;
      while ((line = br.readLine()) != null) {
        if (line.isEmpty()) {
          continue;
        }
        System.out.println("L\t" + line);
        List<String> sentences = tokenizer.tokenize(line);
        for (String s : sentences) {
          System.out.println("S\t" + s);
        }
      }
    }
  }
}
