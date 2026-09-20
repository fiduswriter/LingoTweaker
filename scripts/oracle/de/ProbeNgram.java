import org.languagetool.JLanguageTool;
import org.languagetool.Language;
import org.languagetool.language.GermanyGerman;
import org.languagetool.rules.Rule;

/** Checks whether the ngram rules are active in the standard harness. */
public class ProbeNgram {
  public static void main(String[] args) throws Exception {
    Language de = GermanyGerman.getInstance();
    System.out.println("languageModel=" + ((org.languagetool.LanguageWithModel) de).getLanguageModel(null));
    JLanguageTool lt = new JLanguageTool(de);
    for (Rule r : lt.getAllRules()) {
      String id = r.getId();
      if (id.contains("NGRAM") || id.contains("CONFUSION") || id.contains("PROHIBITED_COMPOUND")) {
        System.out.println("rule=" + id + " class=" + r.getClass().getName());
      }
    }
    System.out.println("totalRules=" + lt.getAllRules().size());
  }
}
