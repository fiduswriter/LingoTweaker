// Time Java LanguageTool per-sentence checking on a sentences file, matching
// the lt-cli bench workload (one sentence per line, single JLanguageTool).
//
// Usage: java -cp ... TimingDump <sentences-file>
//   LT_LANG   language short code (default en-US)
//   WARMUP    number of leading sentences timed separately and discarded
//             from steady-state stats (default 100)
//
// Output to stderr:
//   init <ms>          engine build + first check
//   warmup <n> <ms>    warm-up prefix
//   steady <n> <ms> <ms/sentence>
import org.languagetool.JLanguageTool;
import org.languagetool.Languages;
import org.languagetool.rules.RuleMatch;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;
import java.util.Locale;

public class TimingDump {
  public static void main(String[] args) throws Exception {
    List<String> sentences = Files.readAllLines(Paths.get(args[0]));
    String langCode = System.getenv().getOrDefault("LT_LANG", "en-US");
    int warmup = Integer.parseInt(System.getenv().getOrDefault("WARMUP", "100"));

    long t0 = System.nanoTime();
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode(langCode));
    long initStart = System.nanoTime();
    long nInit = 0;
    for (String line : sentences) {
      if (line.isEmpty()) continue;
      lt.check(line.replace("\r", ""));
      nInit++;
      break;
    }
    long initNanos = System.nanoTime() - initStart;
    long buildNanos = initStart - t0;
    System.err.printf(Locale.ROOT, "build %d ms%n", buildNanos / 1_000_000);
    System.err.printf(Locale.ROOT, "init_first_check %d ms (%d sentences)%n",
        initNanos / 1_000_000, nInit);

    int total = 0;
    for (String line : sentences) {
      if (!line.isEmpty()) total++;
    }

    long warmStart = System.nanoTime();
    int warmN = 0;
    for (int i = 0; i < sentences.size() && warmN < warmup; i++) {
      String line = sentences.get(i).replace("\r", "");
      if (line.isEmpty()) continue;
      lt.check(line);
      warmN++;
    }
    long warmNanos = System.nanoTime() - warmStart;
    System.err.printf(Locale.ROOT, "warmup %d %d ms%n", warmN, warmNanos / 1_000_000);

    long steadyStart = System.nanoTime();
    int n = 0;
    for (String line : sentences) {
      line = line.replace("\r", "");
      if (line.isEmpty()) continue;
      List<RuleMatch> matches = lt.check(line);
      n++;
      long sink = 0;
      for (RuleMatch m : matches) {
        sink += m.getFromPos();
      }
      if (sink == Long.MIN_VALUE) System.out.print(sink); // keep matches alive
    }
    long steadyNanos = System.nanoTime() - steadyStart;
    double msPer = n == 0 ? 0 : steadyNanos / 1_000_000.0 / n;
    System.err.printf(Locale.ROOT, "steady %d %d ms %.3f ms/sentence%n",
        n, steadyNanos / 1_000_000, msPer);
  }
}
