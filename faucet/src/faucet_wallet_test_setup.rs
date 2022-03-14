use async_std::sync::Mutex;
use cap_rust_sandbox::{
    ledger::CapeLedger, types::EdOnBN254Point, universal_param::UNIVERSAL_PARAM,
};
use cape_wallet::mocks::{MockCapeBackend, MockCapeNetwork};
use ethers::{abi::AbiEncode, prelude::U256};
use jf_cap::{MerkleTree, TransactionVerifyingKey};
use key_set::{KeySet, VerifierKeySet};
use seahorse::{loader::Loader, reef::Ledger, testing::MockLedger, Wallet};
use std::{path::PathBuf, sync::Arc};
use structopt::StructOpt;

pub fn u256_to_hex(n: U256) -> String {
    hex::encode(AbiEncode::encode(n))
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "CAPE Faucet utility",
    about = "Create wallet and encryption key from mnemonic to pass to contract for testing"
)]
pub struct Options {
    /// mnemonic for the faucet wallet
    #[structopt(long, env = "CAPE_FAUCET_WALLET_MNEMONIC")]
    pub mnemonic: String,

    /// path to the faucet wallet
    #[structopt(long = "wallet-path", env = "CAPE_FAUCET_WALLET_PATH")]
    pub faucet_wallet_path: PathBuf,

    /// password on the faucet account keyfile
    #[structopt(
        long = "wallet-password",
        env = "CAPE_FAUCET_WALLET_PASSWORD",
        default_value = ""
    )]
    pub faucet_password: String,
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    let opt = Options::from_args();

    if opt.faucet_wallet_path.exists() && opt.faucet_wallet_path.read_dir()?.next().is_some() {
        panic!(
            "Wallet path {:?} is not empty, use a clean directory",
            opt.faucet_wallet_path
        );
    }

    let mut loader = Loader::recovery(
        opt.mnemonic.clone().replace('-', " "),
        opt.faucet_password,
        opt.faucet_wallet_path.clone(),
    );

    let verif_crs = VerifierKeySet {
        mint: TransactionVerifyingKey::Mint(
            jf_cap::proof::mint::preprocess(&*UNIVERSAL_PARAM, CapeLedger::merkle_height())
                .unwrap()
                .1,
        ),
        xfr: KeySet::new(
            vec![
                TransactionVerifyingKey::Transfer(
                    jf_cap::proof::transfer::preprocess(
                        &*UNIVERSAL_PARAM,
                        2,
                        2,
                        CapeLedger::merkle_height(),
                    )
                    .unwrap()
                    .1,
                ),
                TransactionVerifyingKey::Transfer(
                    jf_cap::proof::transfer::preprocess(
                        &*UNIVERSAL_PARAM,
                        3,
                        3,
                        CapeLedger::merkle_height(),
                    )
                    .unwrap()
                    .1,
                ),
            ]
            .into_iter(),
        )
        .unwrap(),
        freeze: KeySet::new(
            vec![TransactionVerifyingKey::Freeze(
                jf_cap::proof::freeze::preprocess(
                    &*UNIVERSAL_PARAM,
                    2,
                    CapeLedger::merkle_height(),
                )
                .unwrap()
                .1,
            )]
            .into_iter(),
        )
        .unwrap(),
    };

    let records = MerkleTree::new(CapeLedger::merkle_height()).unwrap();

    let mut ledger = MockLedger::new(MockCapeNetwork::new(verif_crs, records, vec![]));
    ledger.set_block_size(1).unwrap();

    let backend = MockCapeBackend::new(Arc::new(Mutex::new(ledger)), &mut loader).unwrap();
    let mut wallet = Wallet::new(backend).await.unwrap();

    let pub_key = wallet
        .generate_user_key("i am key".to_string(), None)
        .await
        .unwrap();
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
