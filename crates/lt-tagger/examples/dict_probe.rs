fn main() {
    let needle = std::env::args().nth(1).expect("needle");
    let data = lt_data::DataDir::discover().unwrap();
    let bytes = std::fs::read(data.path().join("de/dictionaries/german.dict")).unwrap();
    let aut = lt_tagger::Cfsa2::parse(&bytes).unwrap();
    aut.visit_sequences(aut.root_node(), &mut |seq: &[u8]| {
        let s = String::from_utf8_lossy(seq);
        if s.contains(&needle) {
            println!("{:?}", seq);
        }
    });
}
