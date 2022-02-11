use anyhow::Result;
use cap_rust_sandbox::assertion::{EnsureMined, Matcher};
use cap_rust_sandbox::cape::CapeBlock;
use cap_rust_sandbox::deploy::{deploy_cape_test, deploy_erc20_token};
use cap_rust_sandbox::ethereum::get_funded_client;
use cap_rust_sandbox::ledger::CapeLedger;
use cap_rust_sandbox::state::{erc20_asset_description, Erc20Code, EthereumAddr};
use cap_rust_sandbox::test_utils::{
    check_erc20_token_balance, compare_roots_records_test_cape_contract, create_faucet,
    generate_burn_tx, ContractsInfo, PrintGas,
};
use cap_rust_sandbox::types as sol;
use cap_rust_sandbox::types::GenericInto;
use ethers::prelude::U256;
use jf_cap::keys::{CredIssuerPubKey, UserPubKey};
use jf_cap::structs::{
    AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, RecordCommitment, RecordOpening,
};
use jf_cap::{MerkleTree, TransactionNote};
use reef::Ledger;

#[tokio::test]
async fn integration_test_unwrapping() -> Result<()> {
    // Deploy the contracts
    let cape_contract = deploy_cape_test().await;

    // Deploy ERC20 token contract. The client deploying the erc20 token contract receives 1000 * 10**18 tokens
    let erc20_token_contract = deploy_erc20_token().await;
    let contracts_info = ContractsInfo::new(&cape_contract, &erc20_token_contract).await;

    // Generate a new client that will receive the unwrapped assets
    let final_recipient_of_unwrapped_assets = get_funded_client().await?;

    let mut mt = MerkleTree::new(CapeLedger::merkle_height()).unwrap();

    // Create some fee asset record
    let (faucet_key_pair, faucet_record_opening) = create_faucet(&cape_contract).await;
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

    // We use an asset policy that does not track the user's credential.
    let asset_policy =
        AssetPolicy::rand_for_test(rng).set_cred_issuer_pub_key(CredIssuerPubKey::default());

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

    let wrapped_ro = RecordOpening::new(
        rng,
        deposited_amount,
        asset_def,
        faucet_key_pair.pub_key(),
        FreezeFlag::Unfrozen,
    );

    let wrapped_record_comm = RecordCommitment::from(&wrapped_ro).to_field_element();
    mt.push(wrapped_record_comm);

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

    // Create burn transaction and record opening based on the content of the records merkle tree
    let unwrapped_assets_recipient_eth_address = final_recipient_of_unwrapped_assets.address();

    const POS_FEE_COMM: u64 = 0;
    const POS_WRAPPED_ASSET_COMM: u64 = 1;

    let cape_burn_tx = generate_burn_tx(
        &faucet_key_pair,
        faucet_record_opening,
        wrapped_ro,
        &mt,
        POS_FEE_COMM,
        POS_WRAPPED_ASSET_COMM,
        unwrapped_assets_recipient_eth_address,
    );

    // Create and submit new block with the burn transaction
    let burn_transaction_note =
        TransactionNote::Transfer(Box::new(cape_burn_tx.clone().transfer_note));
    let mut cape_block = CapeBlock::generate(
        vec![burn_transaction_note],
        vec![cape_burn_tx.clone().burned_ro],
        miner.address(),
    )
    .unwrap();

    // Alter the burn record opening to trigger an error in the CAPE contract
    // when checking that the record opening and its commitment inside the burn transaction match.
    cape_block.burn_notes[0].burned_ro.amount = 2222;

    cape_contract
        .submit_cape_block(cape_block.clone().into())
        .call()
        .await
        .should_revert_with_message("Bad record commitment");

    // Use the right burned amount and check the transaction goes through
    cape_block.burn_notes[0].burned_ro.amount = deposited_amount;

    cape_contract
        .submit_cape_block(cape_block.clone().into())
        .send()
        .await?
        .await?
        .print_gas("Burn transaction");

    // The recipient has received the ERC20 tokens
    check_erc20_token_balance(
        &contracts_info.erc20_token_contract,
        unwrapped_assets_recipient_eth_address,
        U256::from(deposited_amount),
    )
    .await;

    // The ERC20 tokens have left the CAPE contract
    check_erc20_token_balance(
        &contracts_info.erc20_token_contract,
        cape_contract_address,
        U256::from(0),
    )
    .await;

    // Check that the records merkle tree is updated correctly, in particular that the burned record commitment is NOT inserted
    let burned_tx_fee_rc =
        cape_block.burn_notes[0].transfer_note.output_commitments[0].to_field_element();
    mt.push(burned_tx_fee_rc);

    compare_roots_records_test_cape_contract(&mt, &cape_contract, true).await;

    Ok(())
}
