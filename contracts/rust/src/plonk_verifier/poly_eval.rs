#![cfg(test)]
use std::path::Path;

use crate::{
    assertion::Matcher,
    ethereum::{deploy, get_funded_deployer},
    types::{field_to_u256, u256_to_field, EvalDomain, TestPolynomialEval},
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
) -> Result<TestPolynomialEval<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
    let client = get_funded_deployer().await?;
    let contract = deploy(
        client.clone(),
        Path::new("../abi/contracts/mocks/TestPolynomialEval.sol/TestPolynomialEval"),
        (),
    )
    .await?;
    Ok(TestPolynomialEval::new(contract.address(), client))
}

#[tokio::test]
async fn test_vanishing_poly() -> Result<()> {
    let mut rng = test_rng();
    let contract = deploy_contract().await?;

    for log_domain_size in 15..=17 {
        // test case: 1 edge case of evaluate at zero, and 1 random case
        let test_zeta = vec![Fr::zero(), Fr::rand(&mut rng)];
        for zeta in test_zeta {
            // rust side
            let rust_domain =
                Radix2EvaluationDomain::<Fr>::new(2usize.pow(log_domain_size)).unwrap();
            let eval = rust_domain.evaluate_vanishing_polynomial(zeta);

            // solidity side
            let sol_domain: EvalDomain = rust_domain.into();
            let zeta_256 = field_to_u256(zeta);
            let ret = contract
                .evaluate_vanishing_poly(sol_domain, zeta_256)
                .call()
                .await?;

            assert_eq!(eval, u256_to_field(ret));
        }
    }

    let wrong_domain = Radix2EvaluationDomain::<Fr>::new(2usize.pow(18)).unwrap();
    contract
        .evaluate_vanishing_poly(wrong_domain.into(), field_to_u256(Fr::rand(&mut rng)))
        .call()
        .await
        .should_revert_with_message("Poly: size not in 2^{15, 16, 17}");

    Ok(())
}

#[tokio::test]
async fn test_evaluate_lagrange_one() -> Result<()> {
    let mut rng = test_rng();
    let contract = deploy_contract().await?;

    for log_domain_size in 15..=17 {
        let test_zeta = vec![Fr::zero(), Fr::rand(&mut rng)];
        for zeta in test_zeta {
            // rust side
            let rust_domain =
                Radix2EvaluationDomain::<Fr>::new(2usize.pow(log_domain_size)).unwrap();
            let rust_zeta_n_minus_one = rust_domain.evaluate_vanishing_polynomial(zeta);
            let divisor = Fr::from(rust_domain.size() as u32) * (zeta - Fr::one());
            let lagrange_1_eval = rust_zeta_n_minus_one / divisor;

            // solidity side
            let sol_domain: EvalDomain = rust_domain.into();
            let zeta_256 = field_to_u256(zeta);
            let sol_zeta_n_minus_one = contract
                .evaluate_vanishing_poly(sol_domain.clone(), zeta_256)
                .call()
                .await?;

            assert_eq!(rust_zeta_n_minus_one, u256_to_field(sol_zeta_n_minus_one));

            let sol_eval_1 = contract
                .evaluate_lagrange(sol_domain, zeta_256, sol_zeta_n_minus_one)
                .call()
                .await?;

            assert_eq!(lagrange_1_eval, u256_to_field(sol_eval_1));
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_evaluate_pi_poly() -> Result<()> {
    let mut rng = test_rng();
    let contract = deploy_contract().await?;

    for pi_length in 0..5 {
        let rust_pub_input: Vec<Fr> = (0..pi_length).map(|_| Fr::rand(&mut rng)).collect();
        let sol_pub_input: Vec<U256> = rust_pub_input.iter().map(|&x| field_to_u256(x)).collect();

        for log_domain_size in 15..=17 {
            let rust_domain =
                Radix2EvaluationDomain::<Fr>::new(2usize.pow(log_domain_size)).unwrap();

            // rust side

            let zeta = Fr::rand(&mut rng);
            let rust_zeta_n_minus_one = rust_domain.evaluate_vanishing_polynomial(zeta);
            let divisor = Fr::from(rust_domain.size() as u32) * (zeta - Fr::one());
            let lagrange_1_eval = rust_zeta_n_minus_one / divisor;

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
                .evaluate_vanishing_poly(sol_domain.clone(), zeta_256)
                .call()
                .await?;

            assert_eq!(rust_zeta_n_minus_one, u256_to_field(sol_zeta_n_minus_one));

            let sol_eval_1 = contract
                .evaluate_lagrange(sol_domain.clone(), zeta_256, sol_zeta_n_minus_one)
                .call()
                .await?;
            assert_eq!(lagrange_1_eval, u256_to_field(sol_eval_1));

            let sol_eval_pi = contract
                .evaluate_pi_poly(
                    sol_domain,
                    sol_pub_input.clone(),
                    zeta_256,
                    sol_zeta_n_minus_one,
                )
                .call()
                .await?;

            assert_eq!(rust_eval_pi, u256_to_field(sol_eval_pi));
        }
    }

    Ok(())
}
