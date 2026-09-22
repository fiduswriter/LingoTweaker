import java.util.List;

import org.languagetool.tokenizers.uk.UkrainianWordTokenizer;

/**
 * Prints the Ukrainian word tokenizer output for one text, one token per line
 * prefixed with its index, so it can be diffed against the Rust tokenizer.
 *
 * Usage: java TokenizerProbe "<text>"
 */
public class TokenizerProbe {
  public static void main(String[] args) throws Exception {
    for (String text : args) {
      System.out.println("==" + text);
      List<String> tokens = new UkrainianWordTokenizer().tokenize(text);
      for (int i = 0; i < tokens.size(); i++) {
        System.out.println(i + "\t" + tokens.get(i).replace("\t", "\\t").replace("\n", "\\n"));
      }
    }
  }
}