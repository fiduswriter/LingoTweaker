// Developer tool: print MorfologikSpeller suggestions for words, matching the
// Java-side probe output. Usage:
//   cargo run -p lt-spell --example spell -- <dict> <info> <max-edit> <words...>
use std::path::Path;

use lt_spell::MorfologikSpeller;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dict = Path::new(&args[1]);
    let info = Path::new(&args[2]);
    let max_edit: i32 = args[3].parse().unwrap();
    let speller = MorfologikSpeller::from_dict_file(dict, info, max_edit).unwrap();
    for word in &args[4..] {
        let suggestions: Vec<String> = speller
            .get_suggestions(word)
            .into_iter()
            .map(|s| format!("{}/{}", s.word, s.weight))
            .collect();
        println!("{word}\t{}", suggestions.join("|"));
    }
}
