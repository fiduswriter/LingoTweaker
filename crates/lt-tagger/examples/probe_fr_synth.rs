fn main() {
    let data = lt_data::DataDir::discover().unwrap();
    let synth = lt_tagger::FrenchSynthesizer::from_data(data.path()).unwrap();
    let le = lt_core::AnalyzedToken::new("le", Some("le".to_string()), Some("D e s".to_string()));
    println!("le D f s => {:?}", synth.synthesize(&le, "D f s", false));
    println!("le D f s re => {:?}", synth.synthesize(&le, "D f s", true));
    let le2 = lt_core::AnalyzedToken::new("le", Some("le".to_string()), Some("D m s".to_string()));
    println!(
        "le(D m s) D f s => {:?}",
        synth.synthesize(&le2, "D f s", false)
    );
}
