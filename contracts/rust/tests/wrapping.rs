/// This file contains integration tests for the CAPE contract.
use anyhow::Result;
use cap_rust_sandbox::cape::CapeBlock;
use cap_rust_sandbox::deploy::{deploy_cape_test, deploy_erc20_token};
use cap_rust_sandbox::helpers::{
    compare_merkle_root_from_contract_and_jf_tree, eth_transaction_has_been_mined,
};
use cap_rust_sandbox::ledger::CapeLedger;
use cap_rust_sandbox::state::{erc20_asset_description, Erc20Code, EthereumAddr};
use cap_rust_sandbox::types as sol;
use cap_rust_sandbox::types::{
    GenericInto, MerkleRootSol, RecordOpening as RecordOpeningSol, SimpleToken, TestCAPE,
};
use ethers::abi::AbiDecode;
use ethers::prelude::{
    k256::ecdsa::SigningKey, Http, Provider, SignerMiddleware, Wallet, H160, U256,
};
use jf_aap::keys::UserPubKey;
use jf_aap::structs::{
    AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, RecordCommitment, RecordOpening,
};
use jf_aap::utils::TxnsParams;
use jf_aap::{MerkleTree, NodeValue};
use reef::Ledger;
use std::sync::Arc;

fn generate_cape_block_and_merkle_root(tree_height: u8) -> Result<(CapeBlock, NodeValue)> {
    let rng = &mut ark_std::test_rng();
    let num_transfer_txn = 1;
    let num_mint_txn = 1;
    let num_freeze_txn = 1;
    let params = TxnsParams::generate_txns(
        rng,
        num_transfer_txn,
        num_mint_txn,
        num_freeze_txn,
        tree_height,
    );
    let miner = UserPubKey::default();

    let root = params.txns[0].merkle_root();

    let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;
    Ok((cape_block, root))
}

#[derive(Clone)]
struct ContractsInfo {
    cape_contract: TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    erc20_token_contract: SimpleToken<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    cape_contract_for_erc20_owner: TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    erc20_token_address: H160,
    owner_of_erc20_tokens_client: SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
    owner_of_erc20_tokens_client_address: H160,
}

// TODO try to parametrize the struct with the trait M:Middleware
impl ContractsInfo {
    pub async fn new(
        cape_contract_ref: &TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
        erc20_token_contract_ref: &SimpleToken<
            SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
        >,
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

async fn check_pending_deposits_queue_at_index(
    queue_index: usize,
    log_id: usize,
    ro: RecordOpening,
    contracts_info: ContractsInfo,
) -> Result<()> {
    let block_id = 0;
    let logs = contracts_info
        .cape_contract_for_erc20_owner
        .erc_20_tokens_deposited_filter()
        .from_block(block_id)
        .query()
        .await?;
    let ro_bytes = logs[log_id].ro_bytes.clone();
    let ro_sol: RecordOpeningSol = AbiDecode::decode(ro_bytes).unwrap();
    let expected_ro = RecordOpening::from(ro_sol);
    assert_eq!(expected_ro, ro);
    assert_eq!(
        logs[log_id].erc_20_token_address,
        contracts_info.erc20_token_address
    );
    assert_eq!(
        logs[log_id].from,
        contracts_info.owner_of_erc20_tokens_client_address
    );

    let deposit_record_commitment_contract = contracts_info
        .cape_contract
        .get_pending_deposits_at_index(U256::from(queue_index))
        .call()
        .await?;
    let deposit_record_commitment = RecordCommitment::from(&ro);
    let expected_deposit_record_commitment_sol = deposit_record_commitment
        .clone()
        .generic_into::<sol::RecordCommitmentSol>()
        .0;
    assert_eq!(
        deposit_record_commitment_contract,
        expected_deposit_record_commitment_sol
    );

    Ok(())
}

async fn call_and_check_deposit_erc20(
    register_asset: bool,
    deposited_amount: u64,
    expected_owner_balance_before_call: &str,
    expected_owner_balance_after_call: &str,
    expected_contract_balance_after_call: u64,
    contracts_info: ContractsInfo,
) -> Result<RecordOpening> {
    let balance_caller = contracts_info
        .erc20_token_contract
        .balance_of(contracts_info.owner_of_erc20_tokens_client_address)
        .call()
        .await?;

    assert_eq!(
        balance_caller,
        U256::from_dec_str(expected_owner_balance_before_call)?
    );

    // Approve
    let contract_address = contracts_info.cape_contract.address();

    let amount_u256 = U256::from(deposited_amount);
    contracts_info
        .erc20_token_contract
        .approve(contract_address, amount_u256)
        .send()
        .await?
        .await?;

    // Call CAPE contract function

    // Sponsor asset type
    let rng = &mut ark_std::test_rng();
    let erc20_code = Erc20Code(EthereumAddr(
        contracts_info.erc20_token_address.to_fixed_bytes(),
    ));

    // TODO the sponsor should be different from the erc20 tokens owner
    let sponsor = contracts_info.owner_of_erc20_tokens_client_address;

    let description = erc20_asset_description(&erc20_code, &EthereumAddr(sponsor.to_fixed_bytes()));
    let asset_code = AssetCode::new_foreign(&description);
    let asset_def = AssetDefinition::new(asset_code, AssetPolicy::rand_for_test(rng)).unwrap();
    let asset_def_sol = asset_def.clone().generic_into::<sol::AssetDefinition>();

    if register_asset {
        contracts_info
            .cape_contract
            .sponsor_cape_asset(contracts_info.erc20_token_address, asset_def_sol)
            .send()
            .await?
            .await?;
    }

    // Build record opening
    let ro = RecordOpening::new(
        rng,
        deposited_amount,
        asset_def,
        UserPubKey::default(),
        FreezeFlag::Unfrozen,
    );

    // We call the CAPE contract from the address that owns the ERC20 tokens
    contracts_info
        .cape_contract_for_erc20_owner
        .deposit_erc_20(
            ro.clone().generic_into::<sol::RecordOpening>(),
            contracts_info.erc20_token_address,
        )
        .send()
        .await?
        .await?;

    // Check the money has been transferred
    let balance_owner_erc20 = contracts_info
        .erc20_token_contract
        .balance_of(contracts_info.owner_of_erc20_tokens_client_address)
        .call()
        .await?;
    assert_eq!(
        balance_owner_erc20,
        U256::from_dec_str(expected_owner_balance_after_call)?
    );

    let balance_contract_erc20 = contracts_info
        .erc20_token_contract
        .balance_of(contract_address)
        .call()
        .await?;
    assert_eq!(
        balance_contract_erc20,
        U256::from(expected_contract_balance_after_call)
    );

    Ok(ro)
}

#[tokio::test]
async fn integration_test_wrapping_erc20_tokens() -> Result<()> {
    // Create a block containing three transactions
    // Note: we build the block at the beginning of this function because it is time consuming and
    // triggers some timeout with the ethereum client if done after deploying the contracts.
    let (cape_block, root) =
        generate_cape_block_and_merkle_root(CapeLedger::merkle_height()).unwrap();

    // Deploy CAPE contract
    let cape_contract = deploy_cape_test().await;

    // Deploy ERC20 token contract. The caller of this method receives 1000 * 10**18 tokens
    let erc20_token_contract = deploy_erc20_token().await;

    let contracts_info = ContractsInfo::new(&cape_contract, &erc20_token_contract).await;

    let ro1 = call_and_check_deposit_erc20(
        true,
        1000u64,
        "1000000000000000000000",
        "999999999999999999000",
        1000u64,
        contracts_info.clone(),
    )
    .await?;

    let ro2 = call_and_check_deposit_erc20(
        false,
        2000u64,
        "999999999999999999000",
        "999999999999999997000",
        3000u64,
        contracts_info.clone(),
    )
    .await?;

    // Check that the pending deposits queue has been updated correctly
    check_pending_deposits_queue_at_index(0, 0, ro1.clone(), contracts_info.clone()).await?;

    check_pending_deposits_queue_at_index(1, 1, ro2.clone(), contracts_info).await?;

    assert!(
        !cape_contract
            .is_pending_deposits_queue_empty()
            .call()
            .await?
    );

    // Now we submit a new block to the contract and check that the records merkle tree is updated correctly
    cape_contract
        .add_root(root.generic_into::<MerkleRootSol>().0)
        .send()
        .await?
        .await?;

    // Submit to the contract
    let receipt = cape_contract
        .submit_cape_block(cape_block.clone().into(), vec![])
        .gas(U256::from(10_000_000))
        .send()
        .await?
        .await;

    assert!(eth_transaction_has_been_mined(&receipt.unwrap().unwrap()));

    // Check the block has been processed.
    assert_eq!(cape_contract.block_height().call().await?, 1u64);

    // Handle local version of the records merkle tree
    let mut mt = MerkleTree::new(CapeLedger::merkle_height()).unwrap();

    // Add the output commitments of the transactions
    let output_commitments = cape_block.get_list_of_input_record_commitments();
    for comm in output_commitments {
        mt.push(comm.to_field_element());
    }

    // Add the deposit record commitments
    let rc1 = RecordCommitment::from(&ro1);
    mt.push(rc1.to_field_element());
    let rc2 = RecordCommitment::from(&ro2);
    mt.push(rc2.to_field_element());

    // Check the merkle tree root has been updated correctly
    let cape_contract_root_value = cape_contract.get_root_value().call().await?;
    let mt_root_value = mt.commitment().root_value;

    assert!(compare_merkle_root_from_contract_and_jf_tree(
        cape_contract_root_value,
        mt_root_value
    ));

    // Check that the pending deposits queue is empty
    let is_queue_empty = cape_contract
        .is_pending_deposits_queue_empty()
        .call()
        .await?;

    assert!(is_queue_empty);

    Ok(())
}
