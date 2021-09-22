use aap_rust_sandbox::aap_jf::create_test_anon_xfr_2in_6out;
use ark_serialize::CanonicalSerialize;
use std::fs;

fn main() {
    println!("Making note");
    let note = create_test_anon_xfr_2in_6out();
    let mut ser_bytes = Vec::new();
    note.serialize(&mut ser_bytes).unwrap();

    fs::write("my_note.bin", ser_bytes).expect("Unable to write to file");
    println!("Saved!");
}
