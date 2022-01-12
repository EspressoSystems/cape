#![cfg(test)]
use std::path::Path;

use crate::{
    ethereum::{deploy, get_funded_deployer},
    types::{field_to_u256, u256_to_field, EvalDomain, TestPlonkVerifier},
};
use anyhow::Result;
use ark_bn254::Fr;
use ark_ff::Field;
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use ark_std::One;
use ark_std::Zero;
use ark_std::{test_rng, UniformRand};
use ethers::prelude::{Http, Provider, SignerMiddleware, Wallet};
use ethers::{core::k256::ecdsa::SigningKey, prelude::U256};

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

#[tokio::test]
async fn test_evaluate_lagrange_one_and_n() -> Result<()> {
    let mut rng = test_rng();
    let contract: TestPlonkVerifier<_> = deploy_contract().await?;

    for log_domain_size in 15..=17 {
        // rust side
        let rust_domain = Radix2EvaluationDomain::<Fr>::new(1 << log_domain_size).unwrap();
        let zeta = Fr::rand(&mut rng);
        let rust_zeta_n_minus_one = rust_domain.evaluate_vanishing_polynomial(zeta);
        let divisor = Fr::from(rust_domain.size() as u32) * (zeta - Fr::one());
        let lagrange_1_eval = rust_zeta_n_minus_one / divisor;
        let divisor = Fr::from(rust_domain.size() as u32) * (zeta - rust_domain.group_gen_inv);
        let lagrange_n_eval = rust_zeta_n_minus_one * rust_domain.group_gen_inv / divisor;

        // solidity side
        let sol_domain: EvalDomain = rust_domain.into();
        let zeta_256 = field_to_u256(zeta);
        let sol_zeta_n_minus_one = contract
            .test_evaluate_vanishing_poly(sol_domain.clone(), zeta_256)
            .call()
            .await
            .unwrap();

        assert_eq!(rust_zeta_n_minus_one, u256_to_field(sol_zeta_n_minus_one));

        let (sol_eval_1, sol_eval_n) = contract
            .test_evaluate_lagrange_one_and_n(sol_domain, zeta_256, sol_zeta_n_minus_one)
            .call()
            .await
            .unwrap();

        assert_eq!(lagrange_1_eval, u256_to_field(sol_eval_1));
        assert_eq!(lagrange_n_eval, u256_to_field(sol_eval_n));
    }

    Ok(())
}

#[tokio::test]
async fn test_evaluate_pi() -> Result<()> {
    let mut rng = test_rng();
    let contract: TestPlonkVerifier<_> = deploy_contract().await?;

    for pi_length in 0..5 {
        let rust_pub_input: Vec<Fr> = (0..pi_length).map(|_| Fr::rand(&mut rng)).collect();
        let sol_pub_input: Vec<U256> = rust_pub_input.iter().map(|&x| field_to_u256(x)).collect();

        for log_domain_size in 15..=17 {
            let rust_domain = Radix2EvaluationDomain::<Fr>::new(1 << log_domain_size).unwrap();

            // rust side

            let zeta = Fr::rand(&mut rng);
            let rust_zeta_n_minus_one = rust_domain.evaluate_vanishing_polynomial(zeta);
            let divisor = Fr::from(rust_domain.size() as u32) * (zeta - Fr::one());
            let lagrange_1_eval = rust_zeta_n_minus_one / divisor;
            let divisor = Fr::from(rust_domain.size() as u32) * (zeta - rust_domain.group_gen_inv);
            let lagrange_n_eval = rust_zeta_n_minus_one * rust_domain.group_gen_inv / divisor;

            let vanish_eval_div_n =
                Fr::from(rust_domain.size() as u32).inverse().unwrap() * (rust_zeta_n_minus_one);
            let mut rust_eval_pi = Fr::zero();
            for (i, val) in rust_pub_input.iter().enumerate() {
                // minor optimization here: we may pre-compute all the elements for 0..len
                let lagrange_i =
                    vanish_eval_div_n * rust_domain.element(i) / (zeta - rust_domain.element(i));
                rust_eval_pi += lagrange_i * val;
            }

            // solidity side
            let sol_domain: EvalDomain = rust_domain.into();
            let zeta_256 = field_to_u256(zeta);
            let sol_zeta_n_minus_one = contract
                .test_evaluate_vanishing_poly(sol_domain.clone(), zeta_256)
                .call()
                .await
                .unwrap();

            assert_eq!(rust_zeta_n_minus_one, u256_to_field(sol_zeta_n_minus_one));

            let (sol_eval_1, sol_eval_n) = contract
                .test_evaluate_lagrange_one_and_n(
                    sol_domain.clone(),
                    zeta_256,
                    sol_zeta_n_minus_one,
                )
                .call()
                .await
                .unwrap();

            assert_eq!(lagrange_1_eval, u256_to_field(sol_eval_1));
            assert_eq!(lagrange_n_eval, u256_to_field(sol_eval_n));

            let sol_eval_pi = contract
                .test_evaluate_pi_poly(
                    sol_domain,
                    sol_pub_input.clone(),
                    zeta_256,
                    sol_zeta_n_minus_one,
                )
                .call()
                .await
                .unwrap();

            assert_eq!(rust_eval_pi, u256_to_field(sol_eval_pi));
        }
    }

    Ok(())
}
