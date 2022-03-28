use cap_rust_sandbox::types::EdOnBN254Point;
use ethers::{abi::AbiEncode, prelude::U256};
use seahorse::hd::{KeyTree, Mnemonic};
use structopt::StructOpt;

pub fn u256_to_hex(n: U256) -> String {
    hex::encode(AbiEncode::encode(n))
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "CAPE Faucet utility",
    about = "Create address and encryption key from mnemonic to pass to contract for testing"
)]
pub struct Options {
    /// mnemonic for the faucet wallet
    #[structopt(long, env = "CAPE_FAUCET_MANAGER_MNEMONIC")]
    pub mnemonic: String,
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    let opt = Options::from_args();
    let mnemonic = Mnemonic::from_phrase(opt.mnemonic.replace('-', " ")).unwrap();

    // We don't actually want to create a wallet, just generate a key, so we will directly generate
    // the key stream that the faucet wallet will use.
    let pub_key = KeyTree::from_mnemonic(&mnemonic)
        // This should really, be a public Seahorse API, like `KeyTree::wallet_sending_key_stream`.
        .derive_sub_tree("wallet".as_bytes())
        .derive_sub_tree("user".as_bytes())
        .derive_user_key_pair(&0u64.to_le_bytes())
        .pub_key();

    eprintln!("Faucet manager encryption key: {}", pub_key);
    eprintln!(
        "Faucet manager address: {}",
        net::UserAddress(pub_key.address())
    );

    let enc_key_bytes: [u8; 32] = pub_key.enc_key().into();
    let address: EdOnBN254Point = pub_key.address().into();

    println!("CAPE_FAUCET_MANAGER_MNEMONIC=\"{}\"", opt.mnemonic);
    println!(
        "CAPE_FAUCET_MANAGER_ENC_KEY=0x{}",
        hex::encode(enc_key_bytes)
    );
    println!("CAPE_FAUCET_MANAGER_ADDRESS_X=0x{}", u256_to_hex(address.x));
    println!("CAPE_FAUCET_MANAGER_ADDRESS_Y=0x{}", u256_to_hex(address.y));
    Ok(())
}
