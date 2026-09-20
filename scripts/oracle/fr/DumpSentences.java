import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;

import org.languagetool.language.French;
import org.languagetool.tokenizers.SentenceTokenizer;

/**
 * Dumps the French SRX sentence split of a whole text (one `S<TAB>sentence`
 * per split part), mirroring the sentence list of
 * `lt-cli check -l fr --json --file <text>`.
 *
 * Usage: java DumpSentences <text.txt>
 */
public class DumpSentences {
  public static void main(String[] args) throws Exception {
    French lang = new French();
    SentenceTokenizer tokenizer = lang.createDefaultSentenceTokenizer();
    String text = new String(Files.readAllBytes(Paths.get(args[0])), "UTF-8");
    List<String> sentences = tokenizer.tokenize(text);
    for (String s : sentences) {
      System.out.println("S\t" + s.replace("\n", "\\n"));
    }
  }
}
