//! Dutch `CompoundAcceptor` probe mirroring
//! `scripts/oracle/nl/CompoundProbe.java`:
//! `word<TAB>acceptCompound<TAB>getParts|...` per argument.
//!
//! Usage: cargo run --release --example compound_probe_nl -- word1 word2 ...

fn main() {
    let data = lt::DataDir::discover().unwrap();
    let acceptor = lt::compound_acceptor_nl_probe(&data);
    for word in std::env::args().skip(1) {
        println!(
            "{word}\t{}\t{}",
            acceptor.accept_compound(&word),
            acceptor.get_parts(&word).join("|")
        );
    }
}
