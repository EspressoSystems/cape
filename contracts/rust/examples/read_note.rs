use ark_serialize::CanonicalDeserialize;
use jf_txn;
use std::fs;

fn main() {
    let ser_bytes = fs::read("my_note.bin").expect("Can't read file");
    let note =
        jf_txn::transfer::TransferNote::deserialize(&ser_bytes[..]).expect("Failed to deserialize");
    println!("{:?}", note);
}
