use ark_serialize::CanonicalSerialize;
use cap_rust_sandbox::cap_jf::create_anon_xfr_2in_3out;
use std::fs;

fn main() {
    println!("Making note");
    let mut prng = ark_std::test_rng();
    let note = create_anon_xfr_2in_3out(&mut prng, 1)[0].clone();
    let mut ser_bytes = Vec::new();
    note.serialize(&mut ser_bytes).unwrap();

    fs::write("my_note.bin", ser_bytes).expect("Unable to write to file");
    println!("Saved!");
}
