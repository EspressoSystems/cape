use cap_rust_sandbox::types::EdOnBN254Point;
use faucet::faucet_wallet_test_setup::u256_to_hex;
use jf_cap::keys::UserPubKey;
use std::str::FromStr;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "Faucet setup utility")]
struct Options {
    #[structopt(long, env = "FAUCET_PUB_KEY")]
    pub_key: String,
}

fn main() {
    let opt = Options::from_args();

    // output the typescript code for deployment script
    let pub_key = UserPubKey::from_str(&opt.pub_key).unwrap_or_default();
    let enc_key_bytes: [u8; 32] = pub_key.enc_key().into();
    let address: EdOnBN254Point = pub_key.address().into();

    println!(
        r#"
// Derived from {}
let faucetManagerEncKey = "0x{}";
let faucetManagerAddress = {{
  x: BigNumber.from("0x{}"),
  y: BigNumber.from("0x{}"),
}};
"#,
        pub_key,
        hex::encode(enc_key_bytes),
        u256_to_hex(address.x),
        u256_to_hex(address.y),
    );
}
