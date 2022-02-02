use async_std::task;
use cap_rust_sandbox::{cape::CapeBlock, state::CapeTransaction, types::CAPE};
use coins_bip39::English;
use ethers::{core::k256::ecdsa::SigningKey, prelude::*};
use jf_aap::keys::UserPubKey;
use net::server::{add_error_body, request_body, response};
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use std::sync::Arc;
use structopt::StructOpt;
use tide::StatusCode;

type Middleware = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;

#[derive(Clone, Debug, Snafu, Serialize, Deserialize)]
pub enum RelayerError {
    #[snafu(display("failed to deserialize request body: {}", msg))]
    Deserialize { msg: String },

    #[snafu(display("submitted transaction does not form a valid block: {}", msg))]
    BadBlock { msg: String },

    #[snafu(display("error during transaction submission: {}", msg))]
    Submission { msg: String },

    #[snafu(display("transaction was not accepted by Ethereum miners"))]
    Rejected,

    #[snafu(display("internal server error: {}", msg))]
    Internal { msg: String },
}

impl net::Error for RelayerError {
    fn catch_all(msg: String) -> Self {
        Self::Internal { msg }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Deserialize { .. } | Self::BadBlock { .. } => StatusCode::BadRequest,
            Self::Submission { .. } | Self::Rejected | Self::Internal { .. } => {
                StatusCode::InternalServerError
            }
        }
    }
}

fn server_error<E: Into<RelayerError>>(err: E) -> tide::Error {
    net::server_error(err)
}

#[derive(Clone)]
pub struct WebState {
    contract: CAPE<Middleware>,
}

async fn submit_endpoint(mut req: tide::Request<WebState>) -> Result<tide::Response, tide::Error> {
    let tx = request_body(&mut req).await.map_err(|err| {
        server_error(RelayerError::Deserialize {
            msg: err.to_string(),
        })
    })?;
    let ret = relay(&req.state().contract, tx)
        .await
        .map_err(server_error)?;
    response(&req, ret)
}

const DEFAULT_RELAYER_PORT: u16 = 50077u16;

fn init_web_server(
    contract: CAPE<Middleware>,
    port: String,
) -> task::JoinHandle<Result<(), std::io::Error>> {
    let mut web_server = tide::with_state(WebState { contract });
    web_server
        .with(add_error_body::<_, RelayerError>)
        .at("/submit")
        .post(submit_endpoint);
    let addr = format!("0.0.0.0:{}", port);
    async_std::task::spawn(web_server.listen(addr))
}

async fn relay(
    contract: &CAPE<Middleware>,
    transaction: CapeTransaction,
) -> Result<TransactionReceipt, RelayerError> {
    let miner = UserPubKey::default();
    let burned_ros = match &transaction {
        CapeTransaction::Burn { ro, .. } => vec![(**ro).clone().into()],
        _ => vec![],
    };
    let cape_block = CapeBlock::from_cape_transactions(vec![transaction], miner.address())
        .map_err(|err| RelayerError::BadBlock {
            msg: err.to_string(),
        })?;
    contract
        .submit_cape_block(cape_block.into(), burned_ros)
        .send()
        .await
        .map_err(|err| RelayerError::Submission { msg: err.to_string() })?
        .await
        .map_err(|err| RelayerError::Submission { msg: err.to_string() })?
        // If we are successful but get None instead of Some(TransactionReceipt), it means the
        // transaction was finalized but not accepted; i.e. it was rejected or expired.
        .ok_or(RelayerError::Rejected)
}

#[derive(Debug, StructOpt)]
#[structopt(name = "Minimal CAPE Relayer", about = "DO NOT USE IN PRODUCTION")]
struct MinimalRelayerOptions {
    /// URL for Ethers provider
    #[structopt(short = "u", long = "rpc-url", default_value = "http://localhost:8545")]
    rpc_url: String,

    /// Address for CAPE submit
    cape_address: Address,

    /// Mnemonic phrase for ETH wallet, for paying submission gas fees.
    mnemonic: String,
}

#[async_std::main]
async fn main() -> std::io::Result<()> {
    let opt = MinimalRelayerOptions::from_args();

    // Set up a client to submit ETH transactions.
    let wallet = MnemonicBuilder::<English>::default()
        .phrase(opt.mnemonic.as_str())
        .build()
        .expect("could not open relayer wallet");
    let provider = Provider::<Http>::try_from(opt.rpc_url.clone())
        .expect("could not instantiate HTTP Provider");
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    // Connect to CAPE smart contract.
    let contract = CAPE::new(opt.cape_address, client);

    // Start serving CAPE transaction submissions.
    let port = std::env::var("PORT").unwrap_or_else(|_| DEFAULT_RELAYER_PORT.to_string());
    init_web_server(contract, port).await
}

#[cfg(test)]
mod test {
    use super::*;
    use async_std::sync::Mutex;
    use cap_rust_sandbox::deploy::deploy_cape_test;
    use cap_rust_sandbox::test_utils::{contract_abi_path, create_faucet};
    use cap_rust_sandbox::{
        cape::CAPEConstructorArgs,
        ethereum::{deploy, get_funded_client},
        ledger::CapeLedger,
        state::CapeTransaction,
        types::{GenericInto, TestCAPE, CAPE},
    };
    use jf_aap::{
        keys::UserKeyPair,
        structs::{AssetDefinition, FreezeFlag, RecordCommitment, RecordOpening},
        testing_apis::universal_setup_for_test,
        transfer::{TransferNote, TransferNoteInput},
        AccMemberWitness, MerkleTree, TransactionNote,
    };
    use lazy_static::lazy_static;
    use net::{
        client::{parse_error_body, response_body},
        Error,
    };
    use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
    use reef::traits::Ledger;
    use surf::Url;

    lazy_static! {
        static ref PORT: Arc<Mutex<u64>> = {
            let port_offset =
                std::env::var("PORT").unwrap_or_else(|_| DEFAULT_RELAYER_PORT.to_string());
            Arc::new(Mutex::new(port_offset.parse().unwrap()))
        };
    }

    async fn get_port() -> u64 {
        let mut counter = PORT.lock().await;
        let port = *counter;
        *counter += 1;
        port
    }

    async fn deploy_test_contract_with_faucet(
    ) -> (TestCAPE<Middleware>, UserKeyPair, RecordOpening, MerkleTree) {
        let cape_contract = deploy_cape_test().await;
        let (faucet_key_pair, faucet_record_opening) = create_faucet(&cape_contract).await;

        let mut records = MerkleTree::new(CapeLedger::merkle_height()).unwrap();
        let faucet_comm = RecordCommitment::from(&faucet_record_opening);
        records.push(faucet_comm.to_field_element());

        (
            cape_contract,
            faucet_key_pair,
            faucet_record_opening,
            records,
        )
    }

    fn upcast_test_cape_to_cape(test_cape: TestCAPE<Middleware>) -> CAPE<Middleware> {
        CAPE::new(test_cape.address(), Arc::new(test_cape.client().clone()))
    }

    fn generate_transfer(
        rng: &mut ChaChaRng,
        faucet: &UserKeyPair,
        faucet_rec: RecordOpening,
        receiver: UserPubKey,
        records: &MerkleTree,
    ) -> CapeTransaction {
        let srs = universal_setup_for_test(2usize.pow(16), rng).unwrap();
        let xfr_prove_key =
            jf_aap::proof::transfer::preprocess(&srs, 1, 2, CapeLedger::merkle_height())
                .unwrap()
                .0;
        let valid_until = 2u64.pow(jf_aap::constants::MAX_TIMESTAMP_LEN as u32) - 1;
        let inputs = vec![TransferNoteInput {
            ro: faucet_rec.clone(),
            acc_member_witness: AccMemberWitness::lookup_from_tree(&records, 0)
                .expect_ok()
                .unwrap()
                .1,
            owner_keypair: faucet,
            cred: None,
        }];
        let outputs = [RecordOpening::new(
            rng,
            1,
            AssetDefinition::native(),
            receiver,
            FreezeFlag::Unfrozen,
        )];
        let note =
            TransferNote::generate_native(rng, inputs, &outputs, 1, valid_until, &xfr_prove_key)
                .unwrap()
                .0;
        CapeTransaction::AAP(TransactionNote::Transfer(Box::new(note)))
    }

    #[async_std::test]
    async fn test_relay() {
        let mut rng = ChaChaRng::from_seed([42; 32]);
        let user = UserKeyPair::generate(&mut rng);

        let (contract, faucet, faucet_rec, records) = deploy_test_contract_with_faucet().await;
        let transaction =
            generate_transfer(&mut rng, &faucet, faucet_rec, user.pub_key(), &records);

        // Submit a transaction and verify that the 2 output commitments get added to the contract's
        // records Merkle tree.
        relay(
            &upcast_test_cape_to_cape(contract.clone()),
            transaction.clone(),
        )
        .await
        .unwrap();
        assert_eq!(contract.get_num_leaves().call().await.unwrap(), 3.into());

        // Submit an invalid transaction (e.g.the same one again) and check that the contract's
        // records Merkle tree is not modified.
        match relay(&upcast_test_cape_to_cape(contract.clone()), transaction).await {
            Err(RelayerError::Submission { .. }) => {}
            res => panic!("expected submission error, got {:?}", res),
        }
        assert_eq!(contract.get_num_leaves().call().await.unwrap(), 3.into());
    }

    fn get_client(port: u64) -> surf::Client {
        let client: surf::Client = surf::Config::new()
            .set_base_url(Url::parse(&format!("http://localhost:{}", port)).unwrap())
            .try_into()
            .unwrap();
        client.with(parse_error_body::<RelayerError>)
    }

    #[async_std::test]
    async fn test_submit() {
        let mut rng = ChaChaRng::from_seed([42; 32]);
        let user = UserKeyPair::generate(&mut rng);

        let (contract, faucet, faucet_rec, records) = deploy_test_contract_with_faucet().await;
        let transaction =
            generate_transfer(&mut rng, &faucet, faucet_rec, user.pub_key(), &records);

        let port = get_port().await;
        init_web_server(upcast_test_cape_to_cape(contract.clone()), port.to_string());
        let client = get_client(port);
        let mut res = client
            .post("/submit")
            .body_json(&transaction)
            .unwrap()
            .send()
            .await
            .unwrap();
        let receipt = response_body::<TransactionReceipt>(&mut res).await.unwrap();
        assert_eq!(receipt.transaction_index, 0.into());
        assert_eq!(contract.get_num_leaves().call().await.unwrap(), 3.into());

        // Test with the non-mock CAPE contract. We can't generate any valid transactions for this
        // contract, since there's no faucet yet and it doesn't have the
        // `set_initial_record_commitments` method, but we can at least check that our transaction
        // is submitted correctly.
        let contract = {
            let deployer = get_funded_client().await.unwrap();

            let verifier = deploy(
                deployer.clone(),
                &contract_abi_path("verifier/PlonkVerifier.sol/PlonkVerifier"),
                (),
            )
            .await
            .unwrap();
            let address = deploy(
                deployer.clone(),
                &contract_abi_path("CAPE.sol/CAPE"),
                CAPEConstructorArgs::new(
                    CapeLedger::merkle_height(),
                    CapeLedger::record_root_history() as u64,
                    verifier.address(),
                )
                .generic_into::<(u8, u64, Address)>(),
            )
            .await
            .unwrap()
            .address();
            CAPE::new(address, deployer)
        };
        let port = get_port().await;
        init_web_server(contract, port.to_string());
        let client = get_client(port);
        match RelayerError::from_client_error(
            client
                .post("/submit")
                .body_json(&transaction)
                .unwrap()
                .send()
                .await
                .expect_err("expected submission of invalid transaction to fail"),
        ) {
            RelayerError::Submission { .. } => {}
            err => panic!("expected submission error, got {:?}", err),
        }
    }
}
