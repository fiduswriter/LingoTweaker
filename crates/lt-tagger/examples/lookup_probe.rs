fn main() {
    let lang = std::env::args().nth(1).expect("lang");
    let word = std::env::args().nth(2).expect("word");
    let data = lt_data::DataDir::discover().unwrap();
    let dir = data.path().join(format!("{lang}/dictionaries"));
    let info = lt_tagger::DictionaryInfo::load(&dir.join(format!("{lang}.info"))).unwrap();
    let dict = lt_tagger::Dictionary::load(&dir.join(format!("{lang}.dict")), &info).unwrap();
    println!("{:?}", dict.lookup(&word));
}
