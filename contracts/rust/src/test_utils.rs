use crate::cape::{BurnNote, DOM_SEP_CAPE_BURN};
use crate::deploy::EthMiddleware;
use crate::helpers::compare_merkle_root_from_contract_and_jf_tree;
use crate::ledger::CapeLedger;
use crate::types::TestRecordsMerkleTree;
use crate::types::{field_to_u256, SimpleToken, TestCAPE};
use ethers::prelude::TransactionReceipt;
use ethers::prelude::{Address, H160, U256};
use jf_cap::keys::{UserKeyPair, UserPubKey};
use jf_cap::structs::{
    AssetDefinition, FeeInput, FreezeFlag, RecordCommitment, RecordOpening, TxnFeeInfo,
};
use jf_cap::testing_apis::universal_setup_for_test;
use jf_cap::transfer::{TransferNote, TransferNoteInput};
use jf_cap::{AccMemberWitness, MerkleTree};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use reef::Ledger;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct ContractsInfo {
    pub cape_contract: TestCAPE<EthMiddleware>,
    pub erc20_token_contract: SimpleToken<EthMiddleware>,
    pub cape_contract_for_erc20_owner: TestCAPE<EthMiddleware>,
    pub erc20_token_address: H160,
    pub owner_of_erc20_tokens_client: EthMiddleware,
    pub owner_of_erc20_tokens_client_address: H160,
}

// TODO try to parametrize the struct with the trait M:Middleware
impl ContractsInfo {
    pub async fn new(
        cape_contract_ref: &TestCAPE<EthMiddleware>,
        erc20_token_contract_ref: &SimpleToken<EthMiddleware>,
    ) -> Self {
        let cape_contract = cape_contract_ref.clone();
        let erc20_token_contract = erc20_token_contract_ref.clone();

        let erc20_token_address = erc20_token_contract.address();
        let owner_of_erc20_tokens_client = erc20_token_contract.client().clone();
        let owner_of_erc20_tokens_client_address = owner_of_erc20_tokens_client.address();

        let cape_contract_for_erc20_owner = TestCAPE::new(
            cape_contract_ref.address(),
            Arc::from(owner_of_erc20_tokens_client.clone()),
        );

        Self {
            cape_contract,
            erc20_token_contract,
            cape_contract_for_erc20_owner,
            erc20_token_address,
            owner_of_erc20_tokens_client,
            owner_of_erc20_tokens_client_address,
        }
    }
}

/// Generates a user key pair that controls the faucet and call the contract for inserting a record commitment inside the merkle tree containing
/// some native fee asset records.
pub async fn create_faucet(contract: &TestCAPE<EthMiddleware>) -> (UserKeyPair, RecordOpening) {
    let mut rng = ChaChaRng::from_seed([42; 32]);
    let faucet_key_pair = UserKeyPair::generate(&mut rng);
    let faucet_rec = RecordOpening::new(
        &mut rng,
        u64::MAX / 2,
        AssetDefinition::native(),
        faucet_key_pair.pub_key(),
        FreezeFlag::Unfrozen,
    );
    let faucet_comm = RecordCommitment::from(&faucet_rec);
    contract
        .set_initial_record_commitments(vec![field_to_u256(faucet_comm.to_field_element())])
        .send()
        .await
        .unwrap()
        .await
        .unwrap();
    assert_eq!(contract.get_num_leaves().call().await.unwrap(), 1.into());

    (faucet_key_pair, faucet_rec)
}

pub fn contract_abi_path(contract_name: &str) -> PathBuf {
    [
        &PathBuf::from(env!("CONTRACTS_DIR")),
        Path::new("abi/contracts"),
        Path::new(&contract_name),
    ]
    .iter()
    .collect::<PathBuf>()
}

pub async fn check_erc20_token_balance(
    erc20_token_contract: &SimpleToken<EthMiddleware>,
    user_eth_address: Address,
    expected_amount: U256,
) {
    let balance = erc20_token_contract
        .balance_of(user_eth_address)
        .call()
        .await
        .unwrap();
    assert_eq!(balance, expected_amount);
}

pub fn compute_extra_proof_bound_data_for_burn_tx(recipient_address: Address) -> Vec<u8> {
    [
        DOM_SEP_CAPE_BURN.to_vec(),
        recipient_address.to_fixed_bytes().to_vec(),
    ]
    .concat()
}

pub fn generate_burn_tx(
    faucet_key_pair: &UserKeyPair,
    faucet_ro: RecordOpening,
    wrapped_ro: RecordOpening,
    mt: &MerkleTree,
    pos_fee_comm: u64,
    pos_wrapped_asset_comm: u64,
    ethereum_recipient_address: Address,
) -> BurnNote {
    let mut rng = ChaChaRng::from_seed([42; 32]);

    let srs = universal_setup_for_test(2usize.pow(16), &mut rng).unwrap();

    // 2 inputs: fee input record and wrapped asset record
    // 2 outputs: changed fee asset record, burn output record
    let xfr_prove_key =
        jf_cap::proof::transfer::preprocess(&srs, 2, 2, CapeLedger::merkle_height())
            .unwrap()
            .0;
    let valid_until = 2u64.pow(jf_cap::constants::MAX_TIMESTAMP_LEN as u32) - 1;

    let fee_input = FeeInput {
        ro: faucet_ro,
        acc_member_witness: AccMemberWitness::lookup_from_tree(mt, pos_fee_comm)
            .expect_ok()
            .unwrap()
            .1,
        owner_keypair: faucet_key_pair,
    };

    let txn_fee_info = TxnFeeInfo::new(&mut rng, fee_input, 10).unwrap().0;

    let inputs = vec![TransferNoteInput {
        ro: wrapped_ro.clone(),
        acc_member_witness: AccMemberWitness::lookup_from_tree(mt, pos_wrapped_asset_comm)
            .expect_ok()
            .unwrap()
            .1,
        owner_keypair: faucet_key_pair,
        cred: None,
    }];

    let burn_pk = UserPubKey::default();
    let burn_ro = RecordOpening::new(
        &mut rng,
        wrapped_ro.amount,
        wrapped_ro.asset_def,
        burn_pk,
        FreezeFlag::Unfrozen,
    );

    let outputs = vec![burn_ro.clone()];

    // Set the correct extra_proof_bound_data
    // The wrapped asset is sent back to the depositor address
    let extra_proof_bound_data =
        compute_extra_proof_bound_data_for_burn_tx(ethereum_recipient_address);

    let note = TransferNote::generate_non_native(
        &mut rng,
        inputs,
        &outputs,
        txn_fee_info,
        valid_until,
        &xfr_prove_key,
        extra_proof_bound_data,
    )
    .unwrap()
    .0;

    BurnNote::generate(note, burn_ro).unwrap()
}

pub async fn compare_roots_records_merkle_tree_contract(
    mt: &MerkleTree,
    contract: &TestRecordsMerkleTree<EthMiddleware>,
    should_be_equal: bool,
) {
    let root_fr254 = mt.commitment().root_value;
    let root_value_u256 = contract.get_root_value().call().await.unwrap();

    assert_eq!(
        should_be_equal,
        compare_merkle_root_from_contract_and_jf_tree(root_value_u256, root_fr254)
    );
}

pub async fn compare_roots_records_test_cape_contract(
    mt: &MerkleTree,
    contract: &TestCAPE<EthMiddleware>,
    should_be_equal: bool,
) {
    let root_fr254 = mt.commitment().root_value;
    let root_value_u256 = contract.get_root_value().call().await.unwrap();

    assert_eq!(
        should_be_equal,
        compare_merkle_root_from_contract_and_jf_tree(root_value_u256, root_fr254)
    );
}

pub trait PrintGas {
    fn print_gas(self, prefix: &str) -> Self;
}

impl PrintGas for Option<TransactionReceipt> {
    fn print_gas(self, prefix: &str) -> Self {
        println!(
            "{} gas used: {}",
            prefix,
            self.as_ref().unwrap().gas_used.unwrap()
        );
        self
    }
}
