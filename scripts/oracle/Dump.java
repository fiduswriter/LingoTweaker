// Dump per-token readings of the Java LanguageTool pipeline as TSV, matching
// `lt-cli analyze` for oracle diffs.
//
// Usage: java -cp ... Dump <sentences-file>
// One sentence per line; output:
//   S	<idx>	<sentence>
//   R|D	<tok idx>	<surface>	<startPos>	<FLAGS>	readings|<lemma>:<pos>...	chunks|...
// Flags: WSB (whitespace before), SS, SE, IMM, IGN, TYP.
import opennlp.tools.chunker.ChunkerME;
import opennlp.tools.chunker.ChunkerModel;
import opennlp.tools.postag.POSModel;
import opennlp.tools.postag.POSTaggerME;
import opennlp.tools.tokenize.TokenizerME;
import opennlp.tools.tokenize.TokenizerModel;
import org.languagetool.AnalyzedSentence;
import org.languagetool.AnalyzedToken;
import org.languagetool.AnalyzedTokenReadings;
import org.languagetool.JLanguageTool;
import org.languagetool.Languages;
import org.languagetool.chunking.ChunkTag;
import org.languagetool.chunking.EnglishChunker;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;

public class Dump {
  private static TokenizerModel tokenModel;
  private static POSModel posModel;
  private static ChunkerModel chunkerModel;

  private static final boolean HIST = System.getenv("DUMP_HIST") != null;

  public static void main(String[] args) throws Exception {
    if (HIST) {
      org.languagetool.GlobalConfig.setVerbose(true);
    }
    tokenModel = new TokenizerModel(EnglishChunker.class.getResourceAsStream("/en-token.bin"));
    posModel = new POSModel(EnglishChunker.class.getResourceAsStream("/en-pos-maxent.bin"));
    chunkerModel = new ChunkerModel(EnglishChunker.class.getResourceAsStream("/en-chunker.bin"));
    List<String> sentences = Files.readAllLines(Paths.get(args[0]));
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode("en-US"));
    StringBuilder out = new StringBuilder();
    int si = 0;
    for (String sentence : sentences) {
      si++;
      out.append("S\t").append(si).append('\t').append(q(sentence)).append('\n');
      dumpOpenNlp(sentence, out);
      dump("R", lt.getRawAnalyzedSentence(sentence).getTokens(), out);
      dump("D", lt.getAnalyzedSentence(sentence).getTokens(), out);
    }
    System.out.print(out);
  }

  /** C lines: raw OpenNLP tokenize/POS/chunk, matching `dump_chunks`. */
  private static void dumpOpenNlp(String sentence, StringBuilder out) {
    String clean = sentence.replace('\u2019', '\'');
    String[] tokens = new TokenizerME(tokenModel).tokenize(clean);
    String[] tags = new POSTaggerME(posModel).tag(tokens);
    String[] chunks = new ChunkerME(chunkerModel).chunk(tokens, tags);
    for (int i = 0; i < tokens.length; i++) {
      out.append("C\t").append(i).append('\t').append(q(tokens[i])).append('\t')
          .append(tags[i]).append('\t').append(chunks[i]).append('\n');
    }
  }

  private static void dump(String kind, AnalyzedTokenReadings[] tokens, StringBuilder out) {
    int i = 0;
    for (AnalyzedTokenReadings t : tokens) {
      out.append(kind).append('\t').append(i++).append('\t')
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
      if (HIST) {
        String hist = t.getHistoricalAnnotations();
        if (hist != null && !hist.isEmpty()) {
          out.append("\thist:").append(q(hist.replace('\n', ' ')));
        }
      }
      out.append('\n');
    }
  }

  private static String q(String s) {
    return s.replace("\t", "\\t").replace("\n", "\\n");
  }
}
