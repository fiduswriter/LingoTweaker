package org.languagetool.rules.nl;

/**
 * Prints `acceptCompound` / `getParts` per argument, mirroring
 * `CompoundAcceptor.main` so the Rust port can be diffed.
 *
 * Usage: java org.languagetool.rules.nl.CompoundProbe word1 word2 ...
 */
public class CompoundProbe {
  public static void main(String[] args) throws Exception {
    CompoundAcceptor acceptor = new CompoundAcceptor();
    for (String word : args) {
      boolean accepted = acceptor.acceptCompound(word);
      java.util.List<String> parts = acceptor.getParts(word);
      System.out.println(word + "\t" + accepted + "\t" + String.join("|", parts));
    }
  }
}
