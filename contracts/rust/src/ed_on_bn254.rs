#![cfg(test)]

use crate::{
    ethereum::{deploy, get_funded_deployer},
    types::{EdOnBN254Point, TestEdOnBN254},
};
use anyhow::Result;
use ark_ec::AffineCurve;
use ark_ed_on_bn254::EdwardsAffine;
use ark_serialize::CanonicalSerialize;
use ark_std::UniformRand;
use ark_std::Zero;
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use std::path::Path;

async fn deploy_contract(
) -> Result<TestEdOnBN254<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
    let client = get_funded_deployer().await?;

    let contract = deploy(
        client.clone(),
        Path::new("../abi/contracts/mocks/TestEdOnBN254.sol/TestEdOnBN254"),
        (),
    )
    .await?;

    Ok(TestEdOnBN254::new(contract.address(), client))
}

#[tokio::test]
async fn test_serialization() -> Result<()> {
    let rng = &mut ark_std::test_rng();

    // somehow deploying this contract returns an error
    let contract = deploy_contract().await?;
    let mut rust_ser = Vec::new();

    // infinity
    let point = EdwardsAffine::zero();
    point.serialize(&mut rust_ser)?;
    let sol_point: EdOnBN254Point = point.into();
    let sol_ser = contract.serialize(sol_point.into()).call().await?;

    assert_eq!(sol_ser.to_vec(), rust_ser);

    // generator
    rust_ser = Vec::new();
    let point = EdwardsAffine::prime_subgroup_generator();
    point.serialize(&mut rust_ser)?;
    let sol_point: EdOnBN254Point = point.into();
    let sol_ser = contract.serialize(sol_point.into()).call().await?;

    assert_eq!(sol_ser.to_vec(), rust_ser);

    for _ in 0..10 {
        rust_ser = Vec::new();
        let point: EdwardsAffine = EdwardsAffine::rand(rng);
        point.serialize(&mut rust_ser)?;

        let sol_point: EdOnBN254Point = point.into();
        let sol_ser = contract.serialize(sol_point.into()).call().await?;

        assert_eq!(sol_ser.to_vec(), rust_ser);
    }
    Ok(())
}
