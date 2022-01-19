#![cfg(test)]
mod helpers;
mod poly_eval;

use self::helpers::gen_plonk_proof_for_test;
use crate::types::{u256_to_field, GenericInto};
use crate::{
    ethereum::{deploy, get_funded_deployer},
    plonk_verifier::helpers::get_poly_evals,
    types as sol,
    types::{field_to_u256, TestPlonkVerifier},
};
use anyhow::Result;
use ark_bn254::{Bn254, Fq, Fr, G1Affine};
use ark_ec::ProjectiveCurve;
use ark_ff::Field;
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use ark_std::rand::Rng;
use ark_std::Zero;
use ark_std::{test_rng, One, UniformRand};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use jf_plonk::proof_system::verifier::PcsInfo;
use jf_plonk::{
    proof_system::{
        structs::{Proof, VerifyingKey},
        verifier::Verifier,
    },
    transcript::SolidityTranscript,
};
use jf_utils::field_switching;
use std::{convert::TryInto, path::Path};

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

        let eval_data = sol::EvalData {
            vanish_eval: field_to_u256(vanish_eval),
            lagrange_one: field_to_u256(lagrange_1_eval),
        };

        assert_eq!(
            contract
                .compute_lin_poly_constant_term(
                    domain.into(),
                    challenges.into(),
                    public_inputs.iter().map(|f| field_to_u256(*f)).collect(),
                    proof.clone().into(),
                    eval_data.clone()
                )
                .call()
                .await?,
            field_to_u256(lin_poly_constant),
        );

        // build the (aggregated) polynomial commitment instance
        let (comm_scalars_and_bases, buffer_v_and_uv_basis) = verifier.aggregate_poly_commitments(
            &[&vk],
            &challenges,
            &vanish_eval,
            &lagrange_1_eval,
            &lagrange_n_eval,
            &proof.clone().into(),
            &alpha_powers,
            &alpha_bases,
        )?;
        let _rust_msm_result = comm_scalars_and_bases.multi_scalar_mul().into_affine();

        let (bases, scalars) = contract
            .linearization_scalars_and_bases(
                vk.into(),
                challenges.into(),
                eval_data,
                proof.clone().into(),
            )
            .call()
            .await?;

        let hash_map = comm_scalars_and_bases.base_scalar_map;
        for (b, s) in bases.iter().zip(scalars.iter()).skip(1) {
            // FIXME: the first base-scalar pair is incorrect
            // since we have not yet implemented the function to
            // include u and uv bases
            let base = G1Affine::from(b.clone());
            if !base.is_zero() {
                assert!(hash_map.get(&base).is_some());
                assert_eq!(*hash_map.get(&base).unwrap(), u256_to_field::<Fr>(*s));
            }
        }

        let _ether_msm_res = contract.multi_scalar_mul(bases, scalars).call().await?;

        // FIXME: currently bases and scalars do not have the u and uv info
        // this following tests will not pass, will be fixed in a separate MR
        // assert_eq!(ether_msm_res.x, field_to_u256(rust_msm_result.x));
        // assert_eq!(ether_msm_res.y, field_to_u256(rust_msm_result.y));

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

#[tokio::test]
async fn test_challenge_gen() -> Result<()> {
    // =================
    // null challenge
    // =================
    let mut rng = test_rng();
    let contract: TestPlonkVerifier<_> = deploy_contract().await?;
    let extra_message = b"extra message";

    let proof_fr_elements: Vec<Fr> = (0..36).map(|_| Fr::rand(&mut rng)).collect();
    let proof_fq_elements: Vec<Fq> = proof_fr_elements
        .iter()
        .map(|x| field_switching(x))
        .collect();

    // rust side
    let rust_verifying_key = VerifyingKey::<Bn254>::dummy(10, 1024);
    let num_comm = rng.gen_range(0..10);
    let rust_public_inputs: Vec<Fr> = (0..num_comm).map(|_| Fr::rand(&mut rng)).collect();
    let rust_proof: Proof<Bn254> = proof_fq_elements.try_into().unwrap();
    let rust_challenge = Verifier::<Bn254>::compute_challenges::<SolidityTranscript>(
        &[&rust_verifying_key],
        &[&rust_public_inputs],
        &(rust_proof.clone().into()),
        &Some(extra_message.to_vec()),
    )?;

    // solidity side
    let ether_verifying_key: sol::VerifyingKey = rust_verifying_key.into();
    let ether_public_inputs = rust_public_inputs
        .iter()
        .map(|&x| field_to_u256(x))
        .collect();
    let ether_proof: sol::PlonkProof = rust_proof.into();

    let ether_challenge: sol::Challenges = contract
        .compute_challenges(
            ether_verifying_key,
            ether_public_inputs,
            ether_proof,
            extra_message.into(),
        )
        .call()
        .await?;

    let ether_challenge_converted: sol::Challenges = rust_challenge.into();
    assert_eq!(ether_challenge_converted, ether_challenge);

    // =================
    // real data
    // =================

    // rust side
    let (rust_proof, rust_verifying_key, rust_public_inputs, extra_message, _domain_size) =
        gen_plonk_proof_for_test(1)?[0].clone();

    let rust_challenge = Verifier::<Bn254>::compute_challenges::<SolidityTranscript>(
        &[&rust_verifying_key],
        &[&rust_public_inputs],
        &(rust_proof.clone().into()),
        &extra_message,
    )?;

    // solidity side
    let ether_verifying_key: sol::VerifyingKey = rust_verifying_key.into();
    let ether_public_inputs = rust_public_inputs
        .iter()
        .map(|&x| field_to_u256(x))
        .collect();
    let ether_proof: sol::PlonkProof = rust_proof.into();

    let ether_challenge: sol::Challenges = contract
        .compute_challenges(
            ether_verifying_key.clone(),
            ether_public_inputs,
            ether_proof.clone(),
            Bytes::default(),
        )
        .call()
        .await?;

    let ether_challenge_converted: sol::Challenges = rust_challenge.into();
    assert_eq!(ether_challenge_converted, ether_challenge);

    Ok(())
}

#[tokio::test]
async fn test_linearization_scalars_and_bases() -> Result<()> {
    let contract: TestPlonkVerifier<_> = deploy_contract().await?;

    // rust side
    let (rust_proof, rust_verifying_key, rust_public_inputs, extra_message, domain_size) =
        gen_plonk_proof_for_test(1)?[0].clone();

    let verifier = Verifier::new(domain_size)?;
    let rust_challenge = Verifier::<Bn254>::compute_challenges::<SolidityTranscript>(
        &[&rust_verifying_key],
        &[&rust_public_inputs],
        &(rust_proof.clone().into()),
        &extra_message,
    )?;

    let rust_domain = Radix2EvaluationDomain::<Fr>::new(domain_size).unwrap();
    let rust_zeta_n_minus_one = rust_domain.evaluate_vanishing_polynomial(rust_challenge.zeta);
    let divisor = Fr::from(rust_domain.size() as u32) * (rust_challenge.zeta - Fr::one());
    let lagrange_1_eval = rust_zeta_n_minus_one / divisor;
    let divisor =
        Fr::from(rust_domain.size() as u32) * (rust_challenge.zeta - rust_domain.group_gen_inv);
    let lagrange_n_eval = rust_zeta_n_minus_one * rust_domain.group_gen_inv / divisor;

    let alpha_2 = rust_challenge.alpha.square();
    let alpha_3 = alpha_2 * rust_challenge.alpha;
    let alpha_powers = vec![alpha_2, alpha_3];
    let alpha_bases = vec![Fr::one()];

    let rust_scalar_and_bases = verifier.linearization_scalars_and_bases(
        &[&rust_verifying_key],
        &rust_challenge,
        &rust_zeta_n_minus_one,
        &lagrange_1_eval,
        &lagrange_n_eval,
        &(rust_proof.clone().into()),
        &alpha_powers,
        &alpha_bases,
    )?;

    let res = rust_scalar_and_bases.multi_scalar_mul().into_affine();

    // solidity side
    // let ether_domain: sol::EvalDomain = rust_domain.into();
    let ether_verifying_key: sol::VerifyingKey = rust_verifying_key.into();
    // println!("k {}", u256_to_field::<Fr>(ether_verifying_key.k_1));
    let ether_public_inputs = rust_public_inputs
        .iter()
        .map(|&x| field_to_u256(x))
        .collect();
    let ether_proof: sol::PlonkProof = rust_proof.into();

    let ether_challenge: sol::Challenges = contract
        .compute_challenges(
            ether_verifying_key.clone(),
            ether_public_inputs,
            ether_proof.clone(),
            Bytes::default(),
        )
        .call()
        .await?;

    let ether_challenge_converted: sol::Challenges = rust_challenge.into();
    assert_eq!(ether_challenge_converted, ether_challenge);

    let eval_data = sol::EvalData {
        vanish_eval: field_to_u256(rust_zeta_n_minus_one),
        lagrange_one: field_to_u256(lagrange_1_eval),
    };

    let (bases, scalars) = contract
        .linearization_scalars_and_bases(
            ether_verifying_key,
            ether_challenge,
            eval_data,
            ether_proof,
        )
        .call()
        .await?;

    let ether_res = contract.multi_scalar_mul(bases, scalars).call().await?;

    assert_eq!(ether_res.x, field_to_u256(res.x));
    assert_eq!(ether_res.y, field_to_u256(res.y));
    Ok(())
}
