// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![doc = include_str!("../README.md")]

#[warn(unused_imports)]
use async_std::sync::{Arc, Mutex};
use async_std::task;
use cap_rust_sandbox::{
    cape::{submit_block::submit_cape_block_with_memos, BlockWithMemos, CapeBlock},
    deploy::EthMiddleware,
    model::CapeModelTxn,
    types::CAPE,
};
use ethers::prelude::{BlockNumber, H256, U256};
use jf_cap::{keys::UserPubKey, structs::ReceiverMemo, Signature};
use net::server::{add_error_body, request_body, response};
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use std::{str::FromStr, time::Duration};
use tide::{
    http::headers::HeaderValue,
    security::{CorsMiddleware, Origin},
    StatusCode,
};
use tracing::{event, Level};

pub const DEFAULT_RELAYER_PORT: &str = "50077";
pub const DEFAULT_RELAYER_GAS_LIMIT: &str = "10000000"; // 10M
pub const DEFAULT_RELAYER_RETRY_INTERVAL_MS: &str = "500";
pub const DEFAULT_RELAYER_MAX_RETRIES: &str = "2";

#[derive(Clone, Debug, Snafu, Serialize, Deserialize)]
pub enum Error {
    #[snafu(display("failed to deserialize request body: {}", msg))]
    Deserialize { msg: String },

    #[snafu(display("submitted transaction does not form a valid block: {}", msg))]
    BadBlock { msg: String },

    #[snafu(display("error during transaction submission: {}", msg))]
    Submission { msg: String },

    #[snafu(display("submitted root is outdated or invalid: {}", msg))]
    RootNotFound { msg: String },

    #[snafu(display("internal server error: {}", msg))]
    Internal { msg: String },

    #[snafu(display("error fetching info from the CAPE contract: {}", msg))]
    CallContract { msg: String },

    #[snafu(display("submission failed with nonce error: {}", msg))]
    Nonce { msg: String },
}

impl net::Error for Error {
    fn catch_all(msg: String) -> Self {
        Self::Internal { msg }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Deserialize { .. } | Self::BadBlock { .. } | Self::RootNotFound { .. } => {
                StatusCode::BadRequest
            }
            Self::Submission { .. }
            | Self::CallContract { .. }
            | Self::Internal { .. }
            | Self::Nonce { .. } => StatusCode::InternalServerError,
        }
    }
}

fn server_error<E: Into<Error>>(err: E) -> tide::Error {
    net::server_error(err)
}

#[derive(Clone)]
pub struct WebState {
    contract: CAPE<EthMiddleware>,
    nonce_count_rule: NonceCountRule,
    gas_limit: u64,
    max_retries: u64,
    retry_interval: Duration,
    block_submission_mutex: Arc<Mutex<()>>,
}

impl WebState {
    pub fn new(
        contract: CAPE<EthMiddleware>,
        nonce_count_rule: NonceCountRule,
        gas_limit: u64,
        max_retries: u64,
        retry_interval: Duration,
    ) -> Self {
        Self {
            contract,
            nonce_count_rule,
            gas_limit,
            max_retries,
            retry_interval,
            block_submission_mutex: Arc::new(Mutex::new(())),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitBody {
    pub transaction: CapeModelTxn,
    pub memos: Vec<ReceiverMemo>,
    pub signature: Signature,
}

/// Determines how transaction nonces should be calculated.
#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum NonceCountRule {
    /// Only count mined transaction when creating the nonce.
    Mined,
    /// Also include pending transactions when creating the nonce.
    Pending,
}

impl From<NonceCountRule> for BlockNumber {
    fn from(policy: NonceCountRule) -> Self {
        match policy {
            NonceCountRule::Mined => BlockNumber::Latest,
            NonceCountRule::Pending => BlockNumber::Pending,
        }
    }
}

type ParseNonceCountRuleError = &'static str;

impl FromStr for NonceCountRule {
    type Err = ParseNonceCountRuleError;

    fn from_str(input: &str) -> Result<NonceCountRule, Self::Err> {
        match input {
            "mined" => Ok(NonceCountRule::Mined),
            "pending" => Ok(NonceCountRule::Pending),
            _ => Err("Unable to parse, use \"mined\" or \"pending\""),
        }
    }
}

/// Return a JSON expression with status 200 indicating the server
/// is up and running. The JSON expression is simply,
///    {"status": "available"}
/// When the server is running but unable to process requests
/// normally, a response with status 503 and payload {"status":
/// "unavailable"} should be added.
async fn healthcheck(_req: tide::Request<WebState>) -> Result<tide::Response, tide::Error> {
    Ok(tide::Response::builder(200)
        .content_type(tide::http::mime::JSON)
        .body(tide::prelude::json!({"status": "available"}))
        .build())
}

async fn submit_endpoint(mut req: tide::Request<WebState>) -> Result<tide::Response, tide::Error> {
    let SubmitBody {
        transaction,
        memos,
        signature,
    } = request_body(&mut req).await.map_err(|err| {
        server_error(Error::Deserialize {
            msg: err.to_string(),
        })
    })?;
    let ret = relay(req.state(), transaction, memos, signature)
        .await
        .map_err(server_error)?;
    response(&req, ret)
}
/// This function implements the core logic of the relayer
///
/// * `contract` -  CAPE contract instance to submit the block information to
/// * `transaction` - CAPE transaction from a user
/// * `memos` - list of memos corresponding to the transaction
/// * `signature` - signature over the memos information
///
/// Waits for the transaction to be submitted and returns its hash. Does not wait for the
/// transaction to be mined.
async fn relay(
    web_state: &WebState,
    transaction: CapeModelTxn,
    memos: Vec<ReceiverMemo>,
    sig: Signature,
) -> Result<H256, Error> {
    let miner = UserPubKey::default();
    let block = BlockWithMemos {
        block: CapeBlock::from_cape_transactions(vec![transaction], miner.address()).map_err(
            |err| Error::BadBlock {
                msg: err.to_string(),
            },
        )?,
        memos: vec![(memos, sig)],
    };
    // These log statements show what's being submitted to Ethereum blockchain,
    // except for the memos.
    event!(Level::INFO, "Submitting CAPE block: {:?}", block);
    event!(
        Level::INFO,
        "Submitting CAPE block: {:?}",
        cap_rust_sandbox::types::CapeBlock::from(block.block.clone())
    );
    submit_block(web_state, block).await
}

async fn submit_empty_block(web_state: &WebState) -> Result<H256, Error> {
    let miner = UserPubKey::default();
    let block = BlockWithMemos {
        block: CapeBlock::from_cape_transactions(vec![], miner.address()).map_err(|err| {
            Error::BadBlock {
                msg: err.to_string(),
            }
        })?,
        memos: vec![],
    };
    submit_block(web_state, block).await
}

async fn submit_block(web_state: &WebState, block: BlockWithMemos) -> Result<H256, Error> {
    let _guard = web_state.block_submission_mutex.lock().await;

    let mut attempt = 0;
    let pending = loop {
        let result = submit_cape_block_with_memos(
            &web_state.contract,
            block.clone(),
            web_state.nonce_count_rule.into(),
            web_state.gas_limit,
        )
        .await
        .map_err(|err| {
            let msg = err.to_string();
            if msg.contains("replacement transaction underpriced") {
                Error::Nonce { msg }
            } else if msg.contains("Root not found") {
                Error::RootNotFound { msg }
            } else {
                Error::Submission { msg }
            }
        });
        if matches!(result, Err(Error::Nonce { .. })) && attempt < web_state.max_retries {
            tracing::warn!("Submission failed, retrying");
            async_std::task::sleep(web_state.retry_interval).await;
            attempt += 1;
        } else {
            break result;
        }
    }?;

    // The pending transaction itself doesn't serialize well, but all the relevant information is
    // contained in the transaction hash. The client can reconstruct the pending transaction from
    // the hash using a particular provider.
    event!(
        Level::INFO,
        "Submitted Ethereum transaction hash ETH H256: {:#x}",
        *pending
    );
    Ok(*pending)
}

pub async fn submit_empty_block_loop(
    web_state: WebState,
    empty_block_interval: Duration,
) -> Result<(), Error> {
    loop {
        async_std::task::sleep(empty_block_interval).await;

        // If the pending deposits queue is NOT empty, submit an empty block

        // The queue is empty if we cannot access the first element.
        let queue_is_empty = web_state
            .contract
            .pending_deposits(U256::from(0u64))
            .call()
            .await
            .is_err();

        if !queue_is_empty {
            match submit_empty_block(&web_state).await {
                Ok(_) => {
                    event!(Level::INFO, "Empty block submitted.");
                }
                Err(err) => event!(Level::ERROR, "Failed to submit empty block {}", err),
            };
        }
    }
}

/// This function starts the web server
pub fn init_web_server(
    web_state: WebState,
    port: u16,
) -> task::JoinHandle<Result<(), std::io::Error>> {
    let mut web_server = tide::with_state(web_state);
    web_server.with(
        CorsMiddleware::new()
            .allow_methods("GET, POST".parse::<HeaderValue>().unwrap())
            .allow_headers("*".parse::<HeaderValue>().unwrap())
            .allow_origin(Origin::from("*"))
            .allow_credentials(true),
    );
    web_server.at("/healthcheck").get(healthcheck);
    web_server
        .with(add_error_body::<_, Error>)
        .at("/submit")
        .post(submit_endpoint);
    let addr = format!("0.0.0.0:{}", port);
    async_std::task::spawn(web_server.listen(addr))
}

#[cfg(any(test, feature = "testing"))]
pub mod testing {
    use super::*;
    use async_std::task::sleep;
    use cap_rust_sandbox::test_utils::upcast_test_cape_to_cape;
    use cap_rust_sandbox::{
        deploy::deploy_test_cape, ledger::CapeLedger, test_utils::create_faucet, types::TestCAPE,
    };
    use jf_cap::{
        keys::UserKeyPair,
        structs::{RecordCommitment, RecordOpening},
        MerkleTree,
    };
    use reef::Ledger;
    use std::time::Duration;

    #[allow(dead_code)]
    impl WebState {
        pub fn for_test(contract: &TestCAPE<EthMiddleware>) -> Self {
            Self::new(
                upcast_test_cape_to_cape(contract.clone()),
                NonceCountRule::Pending,
                DEFAULT_RELAYER_GAS_LIMIT.parse().unwrap(),
                DEFAULT_RELAYER_MAX_RETRIES.parse().unwrap(),
                Duration::from_millis(DEFAULT_RELAYER_RETRY_INTERVAL_MS.parse().unwrap()),
            )
        }
    }

    /// `faucet_key_pair` - If not provided, a random faucet key pair will be generated.
    pub async fn deploy_cape_contract_with_faucet(
        faucet_key_pair: Option<UserKeyPair>,
    ) -> (
        TestCAPE<EthMiddleware>,
        UserKeyPair,
        RecordOpening,
        MerkleTree,
    ) {
        let cape_contract = deploy_test_cape().await;
        let (faucet_key_pair, faucet_record_opening) = create_faucet(
            &upcast_test_cape_to_cape(cape_contract.clone()),
            faucet_key_pair,
        )
        .await;
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

    const RELAYER_STARTUP_RETRIES: usize = 8;

    pub async fn wait_for_server(port: u16) {
        // Wait for the server to come up and start serving.
        let mut backoff = Duration::from_millis(100);
        for _ in 0..RELAYER_STARTUP_RETRIES {
            if surf::connect(format!("http://localhost:{}", port))
                .send()
                .await
                .is_ok()
            {
                return;
            }
            sleep(backoff).await;
            backoff *= 2;
        }
        panic!("Minimal relayer did not start in {:?}", backoff);
    }

    /// Start a relayer running a TestCAPE contract.
    ///    
    /// `faucet_key_pair` - If not provided, a random faucet key pair will be generated.
    pub async fn start_minimal_relayer_for_test(
        port: u16,
        faucet_key_pair: Option<UserKeyPair>,
    ) -> (
        TestCAPE<EthMiddleware>,
        UserKeyPair,
        RecordOpening,
        MerkleTree,
    ) {
        let (contract, faucet, faucet_rec, records) =
            deploy_cape_contract_with_faucet(faucet_key_pair).await;
        let web_state = WebState::for_test(&contract);
        init_web_server(web_state, port);
        wait_for_server(port).await;
        (contract, faucet, faucet_rec, records)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use async_std::sync::{Arc, Mutex};
    use cap_rust_sandbox::assertion::{EnsureMined, EnsureRejected};
    use cap_rust_sandbox::cape::RecordsMerkleTreeConstructorArgs;
    use cap_rust_sandbox::model::CAPE_MERKLE_HEIGHT;
    use cap_rust_sandbox::test_utils::upcast_test_cape_to_cape;
    use cap_rust_sandbox::{
        cape::CAPEConstructorArgs,
        ethereum::{deploy, get_funded_client},
        ledger::CapeLedger,
        model::CapeModelTxn,
        test_utils::contract_abi_path,
        types::CAPE,
        universal_param::UNIVERSAL_PARAM,
    };
    use ethers::{prelude::PendingTransaction, providers::Middleware};
    use jf_cap::{
        keys::UserKeyPair,
        sign_receiver_memos,
        structs::{AssetDefinition, FreezeFlag, RecordOpening},
        transfer::{TransferNote, TransferNoteInput},
        AccMemberWitness, MerkleTree, TransactionNote,
    };
    use lazy_static::lazy_static;
    use net::{
        client::{parse_error_body, response_body},
        Error as _,
    };
    use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
    use reef::traits::Ledger;
    use std::iter::once;
    use surf::Url;
    use testing::{
        deploy_cape_contract_with_faucet, start_minimal_relayer_for_test, wait_for_server,
    };

    lazy_static! {
        static ref PORT: Arc<Mutex<u16>> = {
            let port_offset =
                std::env::var("PORT").unwrap_or_else(|_| DEFAULT_RELAYER_PORT.to_string());
            Arc::new(Mutex::new(port_offset.parse().unwrap()))
        };
    }

    async fn get_port() -> u16 {
        let mut counter = PORT.lock().await;
        let port = *counter;
        *counter += 1;
        port
    }

    fn generate_transfer(
        rng: &mut ChaChaRng,
        faucet: &UserKeyPair,
        faucet_rec: RecordOpening,
        receiver: UserPubKey,
        records: &MerkleTree,
    ) -> (CapeModelTxn, Vec<ReceiverMemo>, Signature) {
        let srs = &*UNIVERSAL_PARAM;
        let xfr_prove_key =
            jf_cap::proof::transfer::preprocess(srs, 1, 2, CapeLedger::merkle_height())
                .unwrap()
                .0;
        let valid_until = 2u64.pow(jf_cap::constants::MAX_TIMESTAMP_LEN as u32) - 1;
        let inputs = vec![TransferNoteInput {
            ro: faucet_rec,
            acc_member_witness: AccMemberWitness::lookup_from_tree(records, 0)
                .expect_ok()
                .unwrap()
                .1,
            owner_keypair: faucet,
            cred: None,
        }];
        let outputs = vec![RecordOpening::new(
            rng,
            1u64.into(),
            AssetDefinition::native(),
            receiver,
            FreezeFlag::Unfrozen,
        )];
        let (note, sign_key, fee_output) = TransferNote::generate_native(
            rng,
            inputs,
            &outputs,
            1u64.into(),
            valid_until,
            &xfr_prove_key,
        )
        .unwrap();
        let txn = CapeModelTxn::CAP(TransactionNote::Transfer(Box::new(note)));
        let memos = once(fee_output)
            .chain(outputs)
            .map(|ro| ReceiverMemo::from_ro(rng, &ro, &[]).unwrap())
            .collect::<Vec<_>>();
        let sig = sign_receiver_memos(&sign_key, &memos).unwrap();
        (txn, memos, sig)
    }

    #[async_std::test]
    async fn test_relay_nonce_count_mined() {
        test_relay(NonceCountRule::Mined).await
    }

    #[async_std::test]
    async fn test_relay_nonce_count_pending() {
        test_relay(NonceCountRule::Pending).await
    }

    async fn test_relay(nonce_count_rule: NonceCountRule) {
        let mut rng = ChaChaRng::from_seed([42; 32]);
        let user = UserKeyPair::generate(&mut rng);

        let (contract, faucet, faucet_rec, records) = deploy_cape_contract_with_faucet(None).await;
        let (transaction, memos, sig) =
            generate_transfer(&mut rng, &faucet, faucet_rec, user.pub_key(), &records);
        let provider = contract.client().provider().clone();

        let web_state = WebState::new(
            upcast_test_cape_to_cape(contract.clone()),
            nonce_count_rule,
            DEFAULT_RELAYER_GAS_LIMIT.parse().unwrap(),
            DEFAULT_RELAYER_MAX_RETRIES.parse().unwrap(),
            Duration::from_millis(DEFAULT_RELAYER_RETRY_INTERVAL_MS.parse().unwrap()),
        );

        // Submit a transaction and verify that the 2 output commitments get added to the contract's
        // records Merkle tree.
        let hash = relay(&web_state, transaction.clone(), memos.clone(), sig.clone())
            .await
            .unwrap();
        let receipt = PendingTransaction::new(hash, &provider);
        receipt.await.unwrap().ensure_mined();
        assert_eq!(contract.get_num_leaves().call().await.unwrap(), 3.into());

        // Submit an invalid transaction (e.g.the same one again) and check that the contract's
        // records Merkle tree is not modified.
        match relay(&web_state, transaction, memos, sig).await {
            Err(Error::Submission { .. }) => {}
            res => panic!("expected submission error, got {:?}", res),
        }
        assert_eq!(contract.get_num_leaves().call().await.unwrap(), 3.into());
    }

    #[async_std::test]
    async fn test_gas_limit_setting_has_effect() {
        let mut rng = ChaChaRng::from_seed([42; 32]);
        let user = UserKeyPair::generate(&mut rng);

        let (contract, faucet, faucet_rec, records) = deploy_cape_contract_with_faucet(None).await;
        let (transaction, memos, sig) =
            generate_transfer(&mut rng, &faucet, faucet_rec, user.pub_key(), &records);
        let provider = contract.client().provider().clone();

        // Submit transaction with insufficient gas limit.
        let web_state = WebState::new(
            upcast_test_cape_to_cape(contract.clone()),
            NonceCountRule::Pending,
            1_000_000, // gas limit
            DEFAULT_RELAYER_MAX_RETRIES.parse().unwrap(),
            Duration::from_millis(DEFAULT_RELAYER_RETRY_INTERVAL_MS.parse().unwrap()),
        );
        let hash = relay(&web_state, transaction.clone(), memos.clone(), sig.clone())
            .await
            .unwrap();

        PendingTransaction::new(hash, &provider)
            .await
            .unwrap()
            .ensure_rejected();

        // Submit transaction with sufficient gas limit.
        let hash = relay(
            &WebState::for_test(&contract),
            transaction.clone(),
            memos.clone(),
            sig.clone(),
        )
        .await
        .unwrap();

        PendingTransaction::new(hash, &provider)
            .await
            .unwrap()
            .ensure_mined();
    }

    fn get_client(port: u16) -> surf::Client {
        let client: surf::Client = surf::Config::new()
            .set_base_url(Url::parse(&format!("http://localhost:{}", port)).unwrap())
            .try_into()
            .unwrap();
        client.with(parse_error_body::<Error>)
    }

    #[async_std::test]
    async fn test_submit() {
        let mut rng = ChaChaRng::from_seed([42; 32]);
        let user = UserKeyPair::generate(&mut rng);

        let port = get_port().await;
        let (contract, faucet, faucet_rec, records) =
            start_minimal_relayer_for_test(port, None).await;
        let client = get_client(port);
        let provider = contract.client().provider().clone();
        let (transaction, memos, signature) =
            generate_transfer(&mut rng, &faucet, faucet_rec, user.pub_key(), &records);
        let submit_body = SubmitBody {
            transaction,
            memos,
            signature,
        };

        let mut res = client
            .post("/submit")
            .body_json(&submit_body)
            .unwrap()
            .send()
            .await
            .unwrap();
        let hash = response_body::<H256>(&mut res).await.unwrap();
        let receipt = PendingTransaction::new(hash, &provider);
        receipt.await.unwrap().ensure_mined();
        assert_eq!(contract.get_num_leaves().call().await.unwrap(), 3u64.into());

        // Submit the same transaction again to trigger a Submission error.
        match Error::from_client_error(
            client
                .post("/submit")
                .body_json(&submit_body)
                .unwrap()
                .send()
                .await
                .expect_err("expected submission of invalid transaction to fail"),
        ) {
            Error::Submission { .. } => {}
            err => panic!("expected Submission error, got {:?}", err),
        };

        // Deploy a new contract (without faucet setup) and submit the same
        // transaction to trigger a "Root not found" error.
        let contract = {
            let deployer = get_funded_client().await.unwrap();
            let verifier = deploy(
                deployer.clone(),
                &contract_abi_path("verifier/PlonkVerifier.sol/PlonkVerifier"),
                (),
            )
            .await
            .unwrap();
            let records_merkle_tree = deploy(
                deployer.clone(),
                &contract_abi_path("RecordsMerkleTree.sol/RecordsMerkleTree"),
                RecordsMerkleTreeConstructorArgs::new(CAPE_MERKLE_HEIGHT).to_tuple(),
            )
            .await
            .unwrap();
            let address = deploy(
                deployer.clone(),
                &contract_abi_path("CAPE.sol/CAPE"),
                CAPEConstructorArgs::new(
                    CapeLedger::record_root_history() as u64,
                    verifier.address(),
                    records_merkle_tree.address(),
                )
                .to_tuple(),
            )
            .await
            .unwrap()
            .address();
            CAPE::new(address, deployer)
        };
        let port = get_port().await;
        let web_state = WebState::new(
            contract,
            NonceCountRule::Pending,
            DEFAULT_RELAYER_GAS_LIMIT.parse().unwrap(),
            DEFAULT_RELAYER_MAX_RETRIES.parse().unwrap(),
            Duration::from_millis(DEFAULT_RELAYER_RETRY_INTERVAL_MS.parse().unwrap()),
        );
        init_web_server(web_state, port);
        wait_for_server(port).await;
        let client = get_client(port);
        match Error::from_client_error(
            client
                .post("/submit")
                .body_json(&submit_body)
                .unwrap()
                .send()
                .await
                .expect_err("expected submission of invalid transaction to fail"),
        ) {
            Error::RootNotFound { .. } => {}
            err => panic!("expected 'Root not found' error, got {:?}", err),
        }
    }
}
