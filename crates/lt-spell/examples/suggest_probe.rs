// Temporary differential probe: prints `word<TAB>suggestions` for argv words
// against a given .aff/.dic pair, so it can be diffed with the local hunspell
// 1.7.2 build (`hsprobe`).
use lt_spell::hunspell::HunspellChecker;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let aff = &args[1];
    let dic = &args[2];
    let checker = HunspellChecker::load(Path::new(aff), Path::new(dic)).unwrap();
    for w in &args[3..] {
        let s = checker.suggest(w);
        println!("S\t{}\t{}\t{}", w, checker.spell(w), s.join("|"));
    }
}
