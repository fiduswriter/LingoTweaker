import org.languagetool.JLanguageTool;
import org.languagetool.Languages;
import org.languagetool.rules.Rule;

public class RuleOrder {
  public static void main(String[] args) throws Exception {
    JLanguageTool lt = new JLanguageTool(Languages.getLanguageForShortCode("fr"));
    java.util.List<Rule> rules = lt.getAllActiveRules();
    for (String id : args) {
      int idx = -1;
      for (int i = 0; i < rules.size(); i++) {
        if (rules.get(i).getId().equals(id)) { idx = i; break; }
      }
      System.out.println(id + "\t" + idx + "/" + rules.size());
    }
  }
}
