import java.util.List;

import org.languagetool.AnalyzedSentence;
import org.languagetool.JLanguageTool;
import org.languagetool.language.GermanyGerman;
import org.languagetool.rules.RuleMatch;
import org.languagetool.rules.de.AgreementRule;
import org.languagetool.rules.de.AgreementRule2;

/**
 * Rule-only probe for the agreement rules: `rule.match(getAnalyzedSentence(text))`
 * like the upstream unit tests (bypasses the engine post-filters).
 *
 * Usage: java ProbeAgreement "<text>" [DE_AGREEMENT|DE_AGREEMENT2]
 */
public class ProbeAgreement {
  public static void main(String[] args) throws Exception {
    String text = args[0];
    String which = args.length > 1 ? args[1] : "DE_AGREEMENT";
    org.languagetool.language.German lang = (org.languagetool.language.German) org.languagetool.Languages.getLanguageForShortCode("de-DE");
    JLanguageTool lt = new JLanguageTool(lang);
    AnalyzedSentence sentence = lt.getAnalyzedSentence(text);
    List<RuleMatch> matches;
    if (which.equals("DE_AGREEMENT2")) {
      matches = java.util.Arrays.asList(new AgreementRule2(org.languagetool.TestTools.getMessages("de"), lang).match(sentence));
    } else {
      matches = java.util.Arrays.asList(new AgreementRule(org.languagetool.TestTools.getMessages("de"), lang).match(sentence));
    }
    for (RuleMatch m : matches) {
      StringBuilder sugg = new StringBuilder();
      for (String s : m.getSuggestedReplacements()) {
        if (sugg.length() > 0) sugg.append('|');
        sugg.append(s);
      }
      System.out.println(which + "\t" + m.getFromPos() + "\t" + m.getToPos() + "\t"
          + m.getMessage().replace("\t", " ") + "\t" + sugg);
    }
  }
}
