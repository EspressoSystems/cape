use crate::{
    ethereum::{deploy, get_funded_deployer},
    plonk_verifier::helpers::get_poly_evals,
    types as sol,
    types::{field_to_u256, TestVerifyingKeys},
};
use anyhow::Result;
use ark_std::{rand::Rng, test_rng};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use std::path::Path;

async fn deploy_contract(
) -> Result<TestVerifyingKeys<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
    let client = get_funded_deployer().await?;
    let contract = deploy(
        client.clone(),
        Path::new("../abi/contracts/mocks/TestVerifyingKeys.sol/TestVerifyingKeys"),
        (),
    )
    .await?;
    Ok(TestVerifyingKeys::new(contract.address(), client))
}

#[tokio::test]
async fn test_get_encoded_id() -> Result<()> {
    let contract = deploy_contract().await?;
    let rng = &mut test_rng();

    for _ in 0..5 {
        let note_type: u8 = rng.gen_range(0..=3);
        let num_input: u8 = rng.gen_range(0..=5);
        let num_output: u8 = rng.gen_range(0..=5);
        let tree_depth: u8 = rng.gen_range(20..=26);

        assert_eq!(
            contract
                .get_encoded_id(note_type, num_input, num_output, tree_depth)
                .call()
                .await?,
            (U256::from(note_type) << 24)
                + (U256::from(num_input) << 16)
                + (U256::from(num_output) << 8)
                + U256::from(tree_depth)
        );
    }

    Ok(())
}
