// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use anyhow::Result;
use cap_rust_sandbox::assertion::EnsureMined;
use cap_rust_sandbox::cape::CapeBlock;
use cap_rust_sandbox::deploy::{deploy_erc20_token, deploy_test_cape};
use cap_rust_sandbox::helpers::compare_merkle_root_from_contract_and_jf_tree;
use cap_rust_sandbox::ledger::CapeLedger;
use cap_rust_sandbox::model::{erc20_asset_description, Erc20Code, EthereumAddr};
use cap_rust_sandbox::test_utils::{
    check_erc20_token_balance, upcast_test_cape_to_cape, ContractsInfo,
};
use cap_rust_sandbox::types::{self as sol, RecordCommitmentSol};
use cap_rust_sandbox::types::{GenericInto, MerkleRootSol, RecordOpening as RecordOpeningSol};
use ethers::abi::AbiDecode;
use ethers::prelude::U256;
use itertools::Itertools;
use jf_cap::keys::UserPubKey;
use jf_cap::structs::{
    AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, RecordCommitment, RecordOpening,
};
use jf_cap::utils::TxnsParams;
use jf_cap::MerkleTree;
use reef::Ledger;

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
        .pending_deposits(U256::from(queue_index))
        .call()
        .await?;
    let deposit_record_commitment = RecordCommitment::from(&ro);
    let expected_deposit_record_commitment_sol = deposit_record_commitment
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
    check_erc20_token_balance(
        &contracts_info.erc20_token_contract,
        contracts_info.owner_of_erc20_tokens_client_address,
        U256::from_dec_str(expected_owner_balance_before_call)?,
    )
    .await;

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

    let sponsor = contracts_info.owner_of_erc20_tokens_client_address;

    let policy = AssetPolicy::rand_for_test(rng);
    let description = erc20_asset_description(
        &erc20_code,
        &EthereumAddr(sponsor.to_fixed_bytes()),
        policy.clone(),
    );
    let asset_code = AssetCode::new_foreign(&description);
    let asset_def = AssetDefinition::new(asset_code, policy).unwrap();
    let asset_def_sol = asset_def.clone().generic_into::<sol::AssetDefinition>();

    if register_asset {
        println!("Sponsoring asset");
        contracts_info
            .cape_contract_for_erc20_owner
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
    println!("Depositing tokens");
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
    check_erc20_token_balance(
        &contracts_info.erc20_token_contract,
        contracts_info.owner_of_erc20_tokens_client_address,
        U256::from_dec_str(expected_owner_balance_after_call)?,
    )
    .await;

    check_erc20_token_balance(
        &contracts_info.erc20_token_contract,
        contract_address,
        U256::from(expected_contract_balance_after_call),
    )
    .await;

    Ok(ro)
}

#[tokio::test]
async fn integration_test_wrapping_erc20_tokens() -> Result<()> {
    // Create a block containing three transactions
    // Note: we build the block at the beginning of this function because it is time consuming and
    // triggers some timeout with the ethereum client if done after deploying the contracts.
    let rng = &mut ark_std::test_rng();
    let num_transfer_txn = 1;
    let num_mint_txn = 1;
    let num_freeze_txn = 1;
    let params = TxnsParams::generate_txns(
        rng,
        num_transfer_txn,
        num_mint_txn,
        num_freeze_txn,
        CapeLedger::merkle_height(),
    );
    let miner = UserPubKey::default();
    let root = params.txns[0].merkle_root();

    let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

    // Deploy CAPE contract
    let cape_contract = deploy_test_cape().await;

    // Deploy ERC20 token contract. The client deploying the erc20 token contract receives 1000 * 10**18 tokens
    let erc20_token_contract = deploy_erc20_token().await;

    let contracts_info = ContractsInfo::new(
        &upcast_test_cape_to_cape(cape_contract.clone()),
        &erc20_token_contract,
    )
    .await;

    let ro1 = call_and_check_deposit_erc20(
        true,
        1000u64,
        "1000000000",
        "999999000",
        1000u64,
        contracts_info.clone(),
    )
    .await?;

    let ro2 = call_and_check_deposit_erc20(
        false,
        2000u64,
        "999999000",
        "999997000",
        3000u64,
        contracts_info.clone(),
    )
    .await?;

    // Check that the pending deposits queue has been updated correctly
    check_pending_deposits_queue_at_index(0, 0, ro1.clone(), contracts_info.clone()).await?;

    check_pending_deposits_queue_at_index(1, 1, ro2.clone(), contracts_info).await?;

    assert_ne!(
        cape_contract.pending_deposits_length().call().await?,
        U256::zero()
    );

    // Now we submit a new block to the contract and check that the records merkle tree is updated correctly
    cape_contract
        .add_root(root.generic_into::<MerkleRootSol>().0)
        .send()
        .await?
        .await?;

    println!("Submitting block");
    // Submit to the contract
    cape_contract
        .submit_cape_block(cape_block.clone().into())
        .gas(U256::from(10_000_000))
        .send()
        .await?
        .await?
        .ensure_mined();

    // Check the block has been processed.
    assert_eq!(cape_contract.block_height().call().await?, 1u64);

    // Handle local version of the records merkle tree
    let mut mt = MerkleTree::new(CapeLedger::merkle_height()).unwrap();

    // Add the output commitments of the transactions
    let output_commitments = cape_block.commitments();
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
    assert_eq!(
        cape_contract.pending_deposits_length().call().await?,
        U256::zero()
    );

    // Check the record commitments for the deposits were emitted
    let logs = cape_contract
        .block_committed_filter()
        .from_block(0u64)
        .query()
        .await?;

    assert_eq!(
        logs[0]
            .deposit_commitments
            .iter()
            .map(|&rc| rc
                .generic_into::<RecordCommitmentSol>()
                .generic_into::<RecordCommitment>())
            .collect_vec(),
        vec![rc1, rc2]
    );

    Ok(())
}
