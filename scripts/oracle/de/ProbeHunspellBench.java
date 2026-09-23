import java.io.BufferedReader;
import java.io.FileReader;
import java.nio.file.Path;
import java.util.List;
import java.util.Locale;

import org.languagetool.rules.spelling.hunspell.DumontsHunspellDictionary;

/**
 * Times native libhunspell spell() + suggest() over a word list.
 *
 * Usage: java ProbeHunspellBench <hunspell-dir> <de_DE|de_AT|de_CH> <words.txt> [suggestN]
 */
public class ProbeHunspellBench {
  public static void main(String[] args) throws Exception {
    Path dir = Path.of(args[0]);
    String variant = args[1];
    int suggestN = args.length > 3 ? Integer.parseInt(args[3]) : 500;
    DumontsHunspellDictionary dict =
        new DumontsHunspellDictionary(dir.resolve(variant + ".dic"), dir.resolve(variant + ".aff"), false);
    List<String> words;
    try (BufferedReader br = new BufferedReader(new FileReader(args[2]))) {
      words = br.lines().filter(l -> !l.isEmpty()).toList();
    }
    long t0 = System.nanoTime();
    int ok = 0;
    for (String w : words) {
      if (dict.spell(w)) ok++;
    }
    long spellNanos = System.nanoTime() - t0;
    List<String> misspelled = words.stream().filter(w -> w.endsWith("x") || w.endsWith("z")).limit(suggestN).toList();
    long t1 = System.nanoTime();
    int sug = 0;
    for (String w : misspelled) {
      sug += dict.suggest(w).size();
    }
    long sugNanos = System.nanoTime() - t1;
    System.err.printf(Locale.ROOT,
        "hunspell-java-native: spell %d/%d words %.3f ms/word (%.0f/s) | suggest %d/%d words %.3f ms/word%n",
        ok, words.size(), spellNanos / 1e6 / words.size(), words.size() / (spellNanos / 1e9),
        misspelled.size(), misspelled.size(), sugNanos / 1e6 / Math.max(1, misspelled.size()));
  }
}
