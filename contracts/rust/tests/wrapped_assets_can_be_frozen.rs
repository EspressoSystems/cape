// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use anyhow::Result;
use cap_rust_sandbox::assertion::EnsureMined;
use cap_rust_sandbox::cape::CapeBlock;
use cap_rust_sandbox::deploy::{deploy_cape_test, deploy_erc20_token};
use cap_rust_sandbox::ledger::CapeLedger;
use cap_rust_sandbox::model::{erc20_asset_description, Erc20Code, EthereumAddr};
use cap_rust_sandbox::test_utils::{
    compare_roots_records_test_cape_contract, create_faucet, ContractsInfo, PrintGas,
};
use cap_rust_sandbox::types as sol;
use cap_rust_sandbox::types::GenericInto;
use ethers::prelude::U256;
use jf_cap::freeze::{FreezeNote, FreezeNoteInput};
use jf_cap::keys::{CredIssuerPubKey, FreezerKeyPair, UserKeyPair, UserPubKey};
use jf_cap::proof::freeze::preprocess;
use jf_cap::proof::universal_setup_for_staging;
use jf_cap::structs::{
    AssetCode, AssetDefinition, AssetPolicy, FeeInput, FreezeFlag, RecordCommitment, RecordOpening,
    TxnFeeInfo,
};
use jf_cap::{AccMemberWitness, MerkleTree, TransactionNote};
use rand::SeedableRng;
use rand_chacha::ChaChaRng;
use reef::Ledger;

#[tokio::test]
async fn integration_test_wrapped_assets_can_be_frozen() -> Result<()> {
    // Deploy the contracts

    const FAUCET_RECORD_POS: u64 = 0;
    const WRAPPED_RECORD_POS: u64 = 1;

    let cape_contract = deploy_cape_test().await;

    let mut mt = MerkleTree::new(CapeLedger::merkle_height()).unwrap();

    // Deploy ERC20 token contract. The client deploying the erc20 token contract receives 1000 * 10**18 tokens
    let erc20_token_contract = deploy_erc20_token().await;
    let contracts_info = ContractsInfo::new(&cape_contract, &erc20_token_contract).await;

    // Create some fee asset record
    let (faucet_key_pair, faucet_record_opening) = create_faucet(&cape_contract, None).await;

    let faucet_record_comm = RecordCommitment::from(&faucet_record_opening).to_field_element();
    mt.push(faucet_record_comm);

    // Sponsor CAPE asset
    let rng = &mut ark_std::test_rng();
    let erc20_code = Erc20Code(EthereumAddr(
        contracts_info.erc20_token_address.to_fixed_bytes(),
    ));

    let sponsor = contracts_info.owner_of_erc20_tokens_client_address;
    let description = erc20_asset_description(&erc20_code, &EthereumAddr(sponsor.to_fixed_bytes()));
    let asset_code = AssetCode::new_foreign(&description);

    // We use an asset policy that does not track the user's credential but can handle freezing.
    let freeze_keypair = FreezerKeyPair::generate(rng);
    let asset_policy = AssetPolicy::rand_for_test(rng)
        .set_cred_issuer_pub_key(CredIssuerPubKey::default())
        .set_freezer_pub_key(freeze_keypair.pub_key());

    let asset_def = AssetDefinition::new(asset_code, asset_policy).unwrap();
    let asset_def_sol = asset_def.clone().generic_into::<sol::AssetDefinition>();

    contracts_info
        .cape_contract_for_erc20_owner
        .sponsor_cape_asset(contracts_info.erc20_token_address, asset_def_sol)
        .send()
        .await?
        .await?;

    let deposited_amount = 1000u64;

    let cape_contract_address = contracts_info.cape_contract.address();

    // Deposit ERC20 tokens
    let amount_u256 = U256::from(deposited_amount);
    contracts_info
        .erc20_token_contract
        .approve(cape_contract_address, amount_u256)
        .send()
        .await?
        .await?;

    let user_key_pair = UserKeyPair::generate(rng);

    let wrapped_ro = RecordOpening::new(
        rng,
        deposited_amount,
        asset_def,
        user_key_pair.pub_key(),
        FreezeFlag::Unfrozen,
    );

    let wrapped_ro_commitment = RecordCommitment::from(&wrapped_ro).to_field_element();
    mt.push(wrapped_ro_commitment);

    // We call the CAPE contract from the address that owns the ERC20 tokens
    contracts_info
        .cape_contract_for_erc20_owner
        .deposit_erc_20(
            wrapped_ro.clone().generic_into::<sol::RecordOpening>(),
            contracts_info.erc20_token_address,
        )
        .send()
        .await?
        .await?;

    // Submit empty block to trigger the inclusion of the pending deposit record commitment into the merkle tree
    let miner = UserPubKey::default();
    let empty_block = CapeBlock::generate(vec![], vec![], miner.address())?;

    cape_contract
        .submit_cape_block(empty_block.clone().into())
        .send()
        .await?
        .await?
        .ensure_mined()
        .print_gas("Credit deposit");

    compare_roots_records_test_cape_contract(&mt, &cape_contract, true).await;

    // We now create a transaction to freeze the asset record inserted into the merkle tree
    let mut prng = ChaChaRng::from_seed([0x8au8; 32]);
    let max_degree = 65538;
    let srs = universal_setup_for_staging(max_degree, &mut prng)?;
    let (proving_key, _, _) = preprocess(&srs, 2, CapeLedger::merkle_height())?;

    let fee = 0;

    let freeze_note_input_wrapped_asset_record = FreezeNoteInput {
        ro: wrapped_ro,
        acc_member_witness: AccMemberWitness::lookup_from_tree(&mt, WRAPPED_RECORD_POS)
            .expect_ok()
            .unwrap()
            .1,
        keypair: &freeze_keypair,
    };

    let fee_input = FeeInput {
        ro: faucet_record_opening.clone(),
        acc_member_witness: AccMemberWitness::lookup_from_tree(&mt, FAUCET_RECORD_POS)
            .expect_ok()
            .unwrap()
            .1,
        owner_keypair: &faucet_key_pair,
    };

    let inputs = vec![freeze_note_input_wrapped_asset_record];

    let (txn_fee_info, _fee_chg_ro) = TxnFeeInfo::new(rng, fee_input.clone(), fee)?;
    let (note, _keypair, outputs) =
        FreezeNote::generate(rng, inputs.clone(), txn_fee_info, &proving_key)?;

    // Update the local merkle tree with the new record commitments
    assert_eq!(note.output_commitments.len(), 2);
    let frozen_record = outputs[0].clone();
    assert_eq!(frozen_record.freeze_flag, FreezeFlag::Frozen);
    let frozen_record_commitment = RecordCommitment::from(&frozen_record).to_field_element();
    assert_eq!(
        frozen_record_commitment,
        note.output_commitments[1].to_field_element()
    );
    mt.push(note.output_commitments[0].to_field_element());
    mt.push(note.output_commitments[1].to_field_element());

    // Submit the block with the freeze note
    let block_with_freeze_note = CapeBlock::generate(
        vec![TransactionNote::Freeze(Box::new(note.clone()))],
        vec![],
        miner.address(),
    )?;

    // Submit the block with the freeze note
    cape_contract
        .submit_cape_block(block_with_freeze_note.clone().into())
        .send()
        .await?
        .await?
        .ensure_mined()
        .print_gas("Submit freeze note");

    compare_roots_records_test_cape_contract(&mt, &cape_contract, true).await;

    Ok(())
}
