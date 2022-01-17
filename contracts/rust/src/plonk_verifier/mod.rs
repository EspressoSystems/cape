#![cfg(test)]
mod helpers;

use crate::{
    ethereum::{deploy, get_funded_deployer},
    plonk_verifier::helpers::get_poly_evals,
    types::{self as sol, GenericInto},
    types::{field_to_u256, u256_to_field, TestPlonkVerifier},
};
use anyhow::Result;
use ark_bn254::{Bn254, Fr};
use ark_ff::Field;
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use ark_std::{test_rng, One, UniformRand};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use jf_plonk::{
    proof_system::verifier::{PcsInfo, Verifier},
    transcript::SolidityTranscript,
};
use std::path::Path;

use self::helpers::gen_plonk_proof_for_test;

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

// contains tests for interim functions
#[tokio::test]
async fn test_prepare_pcs_info() -> Result<()> {
    let contract = deploy_contract().await?;

    for _ in 0..5 {
        let (proof, vk, public_inputs, extra_msg, domain_size) =
            gen_plonk_proof_for_test(1)?[0].clone();

        // simulate the verifier logic to drive to state for calling the tested fn.
        let domain = Radix2EvaluationDomain::new(domain_size).unwrap();
        let verifier = Verifier::new(domain_size)?;
        // compute challenges and evaluations
        let challenges = Verifier::compute_challenges::<SolidityTranscript>(
            &[&vk],
            &[&public_inputs],
            &proof.clone().into(),
            &extra_msg,
        )?;
        // pre-compute alpha related values
        let alpha_2 = challenges.alpha.square();
        let alpha_3 = alpha_2 * challenges.alpha;
        let alpha_powers = vec![alpha_2, alpha_3];
        let alpha_bases = vec![Fr::one()];

        // evaluate_vanishing_poly()
        let vanish_eval = domain.evaluate_vanishing_polynomial(challenges.zeta);
        // evaluate_lagrange_1_and_n()
        let divisor = Fr::from(domain.size() as u32) * (challenges.zeta - Fr::one());
        let lagrange_1_eval = vanish_eval / divisor;
        let divisor = Fr::from(domain.size() as u32) * (challenges.zeta - domain.group_gen_inv);
        let lagrange_n_eval = vanish_eval * domain.group_gen_inv / divisor;

        // compute the constant term of the linearization polynomial
        let lin_poly_constant = verifier.compute_lin_poly_constant_term(
            &challenges,
            &[&vk],
            &[&public_inputs],
            &proof.clone().into(),
            &vanish_eval,
            &lagrange_1_eval,
            &lagrange_n_eval,
            &alpha_powers,
            &alpha_bases,
        )?;

        // !! testing for `compute_lin_poly_constant_term()` !!
        let alpha_powers_sol: [U256; 2] = [
            field_to_u256(alpha_powers[0]),
            field_to_u256(alpha_powers[1]),
        ];
        assert_eq!(
            contract
                .compute_lin_poly_constant_term(
                    domain.into(),
                    challenges.into(),
                    vk.clone().into(),
                    public_inputs.iter().map(|f| field_to_u256(*f)).collect(),
                    proof.clone().into(),
                    field_to_u256(vanish_eval),
                    field_to_u256(lagrange_1_eval),
                    alpha_powers_sol,
                )
                .call()
                .await?,
            field_to_u256(lin_poly_constant),
        );

        // build the (aggregated) polynomial commitment instance
        let (_comm_scalars_and_bases, buffer_v_and_uv_basis) = verifier
            .aggregate_poly_commitments(
                &[&vk],
                &challenges,
                &vanish_eval,
                &lagrange_1_eval,
                &lagrange_n_eval,
                &proof.clone().into(),
                &alpha_powers,
                &alpha_bases,
            )?;
        // TODO: @zhenfei should add tests against contract output here.

        // build the (aggregated) polynomial evaluation instance
        let mut buffer_v_and_uv_basis_sol: [U256; 10] = [U256::zero(); 10];
        assert_eq!(buffer_v_and_uv_basis.len(), 10);
        for i in 0..buffer_v_and_uv_basis.len() {
            buffer_v_and_uv_basis_sol[i] = field_to_u256(buffer_v_and_uv_basis[i]);
        }
        let eval = Verifier::<Bn254>::aggregate_evaluations(
            &lin_poly_constant,
            &[get_poly_evals(proof.clone())],
            &[None],
            &buffer_v_and_uv_basis,
        )?;
        assert_eq!(
            contract
                .prepare_evaluations(
                    field_to_u256(lin_poly_constant),
                    proof.into(),
                    buffer_v_and_uv_basis_sol,
                )
                .call()
                .await?,
            field_to_u256(eval)
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_batch_verify_opening_proofs() -> Result<()> {
    let contract = deploy_contract().await?;

    for i in 1..4 {
        let pcs_infos: Vec<PcsInfo<Bn254>> = gen_plonk_proof_for_test(i)?
            .iter()
            .map(|(proof, vk, pub_input, extra_msg, domain_size)| {
                let verifier = Verifier::new(*domain_size).unwrap();
                verifier
                    .prepare_pcs_info::<SolidityTranscript>(
                        &[vk],
                        &[pub_input],
                        &(*proof).clone().into(),
                        extra_msg,
                    )
                    .unwrap()
            })
            .collect();
        let pcs_infos_sol = pcs_infos
            .iter()
            .map(|info| info.clone().generic_into::<sol::PcsInfo>())
            .collect();

        assert!(
            contract
                .batch_verify_opening_proofs(pcs_infos_sol)
                .call()
                .await?
        );
    }
    Ok(())
}
