import java.io.BufferedReader;
import java.io.FileReader;
import java.nio.file.Path;

import org.languagetool.rules.spelling.hunspell.DumontsHunspellDictionary;

/**
 * Dumps the raw native-hunspell `spell()` decision for each input word, using
 * the same `DumontsHunspellDictionary`/libhunspell that GermanSpellerRule uses.
 * No LT ignore lists or rule logic are applied.
 *
 * Usage: java ProbeHunspell <hunspell-dir> <de_DE|de_AT|de_CH> <words.txt>
 */
public class ProbeHunspell {
  public static void main(String[] args) throws Exception {
    Path dir = Path.of(args[0]);
    String variant = args[1];
    DumontsHunspellDictionary dict =
        new DumontsHunspellDictionary(dir.resolve(variant + ".dic"), dir.resolve(variant + ".aff"), false);
    try (BufferedReader br = new BufferedReader(new FileReader(args[2]))) {
      String line;
      while ((line = br.readLine()) != null) {
        if (line.isEmpty()) {
          continue;
        }
        System.out.println(line + "\t" + dict.spell(line));
      }
    }
  }
}
