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
use ark_ec::AffineCurve;
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

    // delay the contract deployment to avoid connection reset problem caused by
    // waiting for CRS loading.
    let contract = deploy_contract().await?;
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
    let (comm_scalars_sol, comm_bases_sol, buffer_v_and_uv_basis_sol) = contract
        .prepare_poly_commitments(
            vk.clone().into(),
            challenges.into(),
            eval_data,
            proof.clone().into(),
        )
        .call()
        .await?;
    assert_eq!(
        contract
            .multi_scalar_mul(comm_bases_sol, comm_scalars_sol)
            .call()
            .await?,
        comm_scalars_and_bases
            .multi_scalar_mul()
            .into_affine()
            .into(),
    );
    assert_eq!(
        buffer_v_and_uv_basis_sol.to_vec(),
        buffer_v_and_uv_basis
            .iter()
            .map(|f| field_to_u256(*f))
            .collect::<Vec<_>>()
    );

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
                proof.clone().into(),
                buffer_v_and_uv_basis_sol,
            )
            .call()
            .await?,
        field_to_u256(eval)
    );

    // TODO: remove all intermediate steps test above?
    // end-to-end test prepare_pcs_info
    let extra_msg_sol = if let Some(msg) = extra_msg.clone() {
        Bytes::from(msg)
    } else {
        Bytes::default()
    };

    let sol_pcs = contract
        .prepare_pcs_info(
            vk.clone().into(),
            public_inputs.iter().map(|f| field_to_u256(*f)).collect(),
            proof.clone().into(),
            extra_msg_sol,
        )
        .call()
        .await?;

    let rust_pcs = verifier.prepare_pcs_info::<SolidityTranscript>(
        &[&vk],
        &[&public_inputs],
        &proof.into(),
        &extra_msg,
    )?;

    assert_eq!(sol_pcs.u, field_to_u256(rust_pcs.u));
    assert_eq!(sol_pcs.eval_point, field_to_u256(rust_pcs.eval_point));
    assert_eq!(
        sol_pcs.next_eval_point,
        field_to_u256(rust_pcs.next_eval_point)
    );
    assert_eq!(sol_pcs.eval, field_to_u256(rust_pcs.eval));
    assert_eq!(
        sol_pcs.opening_proof.x,
        field_to_u256(rust_pcs.opening_proof.0.x)
    );
    assert_eq!(
        sol_pcs.opening_proof.y,
        field_to_u256(rust_pcs.opening_proof.0.y)
    );
    assert_eq!(
        sol_pcs.shifted_opening_proof.x,
        field_to_u256(rust_pcs.shifted_opening_proof.0.x)
    );
    assert_eq!(
        sol_pcs.shifted_opening_proof.y,
        field_to_u256(rust_pcs.shifted_opening_proof.0.y)
    );

    let rust_msm = rust_pcs
        .comm_scalars_and_bases
        .multi_scalar_mul()
        .into_affine();
    let mut res = G1Affine::zero();
    for (b, s) in sol_pcs.comm_bases.iter().zip(sol_pcs.comm_scalars.iter()) {
        let base: G1Affine = b.clone().into();
        let scalar: Fr = u256_to_field(s.clone());
        res += &base.mul(scalar).into_affine();
    }
    assert_eq!(res, rust_msm);

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
