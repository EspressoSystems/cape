use ark_std::str::FromStr;
use jf_cap::keys::UserPubKey;

fn main() {
    let result = "USERPUBKEY~muN7VKxj1GbJ4D6rU6gANdvwD05oPKy_XmhkBxSByq0gAAAAAAAAAIRN-Rik8czFiToI8Ft5fsIf9HAEtWHDsOHh-ZBJZl1KxQ";
    let pk = UserPubKey::from_str(result).unwrap_or_default();
    ark_std::eprintln!(
        "x: {}, y: {}",
        pk.address().internal().x,
        pk.address().internal().y
    );
}
