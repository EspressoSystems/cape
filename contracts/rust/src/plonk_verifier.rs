#![cfg(test)]
use crate::{
    ethereum::{deploy, get_funded_deployer},
    types::{field_to_u256, u256_to_field, TestPlonkVerifier},
};
use anyhow::Result;
use ark_bn254::Fr;
use ark_ff::Field;
use ark_std::{test_rng, UniformRand};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use std::path::Path;

async fn deploy_contract(
) -> Result<TestPlonkVerifier<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
    let client = get_funded_deployer().await?;
    let contract = deploy(
        client.clone(),
        Path::new("../abi/contracts/mocks/TestPlonkVerifier.sol/TestPlonkVerifier"),
        (),
    )
    .await?;
    Ok(TestPlonkVerifier::new(contract.address(), client))
}

#[tokio::test]
async fn test_compute_alpha_powers() -> Result<()> {
    let rng = &mut test_rng();
    let contract = deploy_contract().await?;

    for _ in 0..5 {
        let alpha = Fr::rand(rng);
        let alpha2 = alpha.square();
        let expected = vec![alpha2, alpha2 * alpha];

        assert_eq!(
            contract
                .compute_alpha_powers(field_to_u256(alpha))
                .call()
                .await?
                .iter()
                .map(|&u| u256_to_field(u))
                .collect::<Vec<Fr>>(),
            expected
        );
    }
    Ok(())
}
