use ark_std::str::FromStr;
use itertools::Itertools;
use jf_cap::keys::UserPubKey;

// Fp256 "(2DCA81140764685EBFAC3C684E0FF0DB3500A853AB3EE0C966D463AC547BE39A)"
// => 2DCA81140764685EBFAC3C684E0FF0DB3500A853AB3EE0C966D463AC547BE39A
fn disp_hex<T: ToString>(x: T) -> String {
    let vec = x.to_string().chars().collect_vec();
    vec[8..vec.len() - 2].iter().collect()
}

fn main() {
    let result = "USERPUBKEY~muN7VKxj1GbJ4D6rU6gANdvwD05oPKy_XmhkBxSByq0gAAAAAAAAAIRN-Rik8czFiToI8Ft5fsIf9HAEtWHDsOHh-ZBJZl1KxQ";
    let pub_key = UserPubKey::from_str(result).unwrap_or_default();

    println!(
        r#"
const pubKey = "{}";
const faucetManagerAddress = {{
  x: BigNumber.from("0x{}"),
  y: BigNumber.from("0x{}"),
}};
"#,
        result,
        disp_hex(pub_key.address().internal().x),
        disp_hex(pub_key.address().internal().y)
    );
}
