// Print `SRXSentenceTokenizer.tokenize` segments of the pinned German LT.
//
// Usage: java -cp ... ProbeSentences <sentences-file>
import org.languagetool.Languages;
import org.languagetool.tokenizers.SentenceTokenizer;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;

public class ProbeSentences {
  public static void main(String[] args) throws Exception {
    SentenceTokenizer tokenizer =
        Languages.getLanguageForShortCode("de-DE").createDefaultSentenceTokenizer();
    String raw = new String(Files.readAllBytes(Paths.get(args[0])),
        java.nio.charset.StandardCharsets.UTF_8);
    for (String text : raw.split("%%%")) {
      if (text.isEmpty()) {
        continue;
      }
      System.out.println("== " + q(text));
      int pos = 0;
      for (String sentence : tokenizer.tokenize(text)) {
        System.out.println(pos + "\t" + (pos + sentence.length()) + "\t" + q(sentence));
        pos += sentence.length();
      }
    }
  }

  private static String q(String s) {
    return s.replace("\t", "\\t").replace("\n", "\\n");
  }
}
