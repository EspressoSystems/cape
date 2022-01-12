#![cfg(test)]
use std::path::Path;

use crate::{
    ethereum::{deploy, get_funded_deployer},
    types::{field_to_u256, u256_to_field, EvalDomain, TestPlonkVerifier},
};
use anyhow::Result;
use ark_bn254::Fr;
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use ark_std::{test_rng, UniformRand};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::{Http, Provider, SignerMiddleware, Wallet};

async fn deploy_contract(
) -> Result<TestPlonkVerifier<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
    let client = get_funded_deployer().await.unwrap();
    let contract = deploy(
        client.clone(),
        Path::new("../abi/contracts/mocks/TestPlonkVerifier.sol/TestPlonkVerifier"),
        (),
    )
    .await
    .unwrap();
    Ok(TestPlonkVerifier::new(contract.address(), client))
}

#[tokio::test]
async fn test_vanishing_poly() -> Result<()> {
    let mut rng = test_rng();
    let contract: TestPlonkVerifier<_> = deploy_contract().await?;

    for log_domain_size in 15..=17 {
        // rust side
        let rust_domain = Radix2EvaluationDomain::<Fr>::new(1 << log_domain_size).unwrap();
        let zeta = Fr::rand(&mut rng);
        let eval = rust_domain.evaluate_vanishing_polynomial(zeta);

        // solidity side
        let sol_domain: EvalDomain = rust_domain.into();
        let zeta_256 = field_to_u256(zeta);
        let ret = contract
            .test_evaluate_vanishing_poly(sol_domain, zeta_256)
            .call()
            .await
            .unwrap();

        assert_eq!(eval, u256_to_field(ret));
    }

    Ok(())
}
