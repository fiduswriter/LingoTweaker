//! Debug helper: print the `DiffsAsMatches` pseudo-matches for an
//! original/revised sentence pair (mirrors `scripts/oracle/ca/DiffProbe.java`).
//! Usage: cargo run --release -p lt --example ca_remote_probe -- ORIGINAL REVISED
fn main() {
    let mut args = std::env::args().skip(1);
    let original = args.next().expect("original");
    let revised = args.next().expect("revised");
    for (t, sp, se, tp, te) in lt::ca_remote_deltas(&original, &revised) {
        println!("DELTA {t} src={sp}..{se} tgt={tp}..{te}");
    }
    for (replacement, from, to) in lt::ca_remote_pseudo_matches(&original, &revised) {
        let underlined: String = original.chars().skip(from).take(to - from).collect();
        println!("MATCH {from}..{to} [{underlined}] -> {replacement}");
    }
}
