#![cfg(test)]
mod helpers;
mod poly_eval;
mod vk;

use self::helpers::gen_plonk_proof_for_test;
use crate::assertion::Matcher;
use crate::deploy::{deploy_test_plonk_verifier_contract, EthMiddleware};
use crate::types::GenericInto;
use crate::{
    ethereum::get_funded_client,
    plonk_verifier::helpers::get_poly_evals,
    types as sol,
    types::{field_to_u256, TestPlonkVerifier},
};
use anyhow::Result;
use ark_bn254::{Bn254, Fq, Fr, G1Affine, G1Projective};
use ark_ec::ProjectiveCurve;
use ark_ff::{Field, Zero};
use ark_poly::{EvaluationDomain, Radix2EvaluationDomain};
use ark_std::rand::Rng;
use ark_std::{test_rng, One, UniformRand};
use ethers::prelude::*;
use itertools::multiunzip;
use jf_plonk::testing_apis::PcsInfo;
use jf_plonk::{
    proof_system::structs::{Proof, VerifyingKey},
    testing_apis::Verifier,
    transcript::SolidityTranscript,
};
use jf_utils::field_switching;
use rand::prelude::SliceRandom;
use std::convert::TryInto;

// contains tests for interim functions
#[tokio::test]
async fn test_prepare_pcs_info() -> Result<()> {
    let (proof, vk, public_inputs, extra_msg, domain_size) =
        gen_plonk_proof_for_test(1)?[0].clone();

    // simulate the verifier logic to drive to state for calling the tested fn.
    let domain = Radix2EvaluationDomain::new(domain_size).unwrap();
    let verifier = Verifier::<Bn254>::new(domain_size)?;
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
    // evaluate pi_poly
    let pi_eval = {
        if vanish_eval.is_zero() {
            Fr::zero()
        } else {
            let vanish_eval_div_n = Fr::from(domain.size() as u32).inverse().unwrap() * vanish_eval;
            let mut result = Fr::zero();
            for (i, val) in public_inputs.iter().enumerate() {
                let lagrange_i =
                    vanish_eval_div_n * domain.element(i) / (challenges.zeta - domain.element(i));
                result += lagrange_i * val;
            }
            result
        }
    };

    // delay the contract deployment to avoid connection reset problem caused by
    // waiting for CRS loading.
    let contract = deploy_test_plonk_verifier_contract().await;
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
        pi_eval: field_to_u256(pi_eval),
    };
    assert_eq!(
        contract
            .compute_lin_poly_constant_term(
                challenges.into(),
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

    let (comm_scalars_sol, comm_bases_sol) = contract
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
            .multi_scalar_mul(comm_bases_sol, comm_scalars_sol.clone())
            .call()
            .await?,
        comm_scalars_and_bases
            .multi_scalar_mul()
            .into_affine()
            .into(),
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
                comm_scalars_sol
            )
            .call()
            .await?,
        field_to_u256(eval)
    );

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
            extra_msg_sol.clone(),
        )
        .call()
        .await?;

    println!(
        "\ngas cost for pcs_info {}",
        contract
            .prepare_pcs_info(
                vk.clone().into(),
                public_inputs.iter().map(|f| field_to_u256(*f)).collect(),
                proof.clone().into(),
                extra_msg_sol,
            )
            .estimate_gas()
            .await?
    );

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
    assert_eq!(
        contract
            .multi_scalar_mul(sol_pcs.comm_bases, sol_pcs.comm_scalars)
            .call()
            .await?,
        rust_pcs
            .comm_scalars_and_bases
            .multi_scalar_mul()
            .into_affine()
            .into(),
    );

    Ok(())
}

#[tokio::test]
async fn test_batch_verify_opening_proofs() -> Result<()> {
    let contract = deploy_test_plonk_verifier_contract().await;

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
        let pcs_infos_sol: Vec<sol::PcsInfo> = pcs_infos
            .iter()
            .map(|info| info.clone().generic_into::<sol::PcsInfo>())
            .collect();

        // reconnect to contract to avoid connection reset problem
        let client = get_funded_client().await?;
        let contract = TestPlonkVerifier::new(contract.address(), client);
        assert!(
            contract
                .batch_verify_opening_proofs(pcs_infos_sol.clone())
                .call()
                .await?
        );
        println!(
            "\ngas cost for {} proofs {}",
            i,
            contract
                .batch_verify_opening_proofs(pcs_infos_sol)
                .estimate_gas()
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
    let contract: TestPlonkVerifier<_> = deploy_test_plonk_verifier_contract().await;
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

    // reconnect to contract to avoid connection reset problem
    let client = get_funded_client().await?;
    let contract = TestPlonkVerifier::new(contract.address(), client);

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
async fn test_batch_verify_plonk_proofs() -> Result<()> {
    let contract = deploy_test_plonk_verifier_contract().await;
    let rng = &mut test_rng();

    for num_proof in 1..5 {
        let (proofs, vks, public_inputs, extra_msgs, _): (
            Vec<Proof<Bn254>>,
            Vec<VerifyingKey<Bn254>>,
            Vec<Vec<Fr>>,
            Vec<Option<Vec<u8>>>,
            Vec<usize>,
        ) = multiunzip(gen_plonk_proof_for_test(num_proof)?);
        let vks_sol: Vec<sol::VerifyingKey> = vks
            .iter()
            .map(|vk| vk.clone().generic_into::<sol::VerifyingKey>())
            .collect();
        let bad_vks_sol: Vec<sol::VerifyingKey> = vks_sol
            .iter()
            .map(|vk| {
                let mut bad_vk = vk.clone();
                bad_vk.sigma_2 = G1Projective::rand(rng).into_affine().into();
                bad_vk.q_4 = G1Projective::rand(rng).into_affine().into();
                bad_vk.q_m34 = G1Projective::rand(rng).into_affine().into();
                bad_vk
            })
            .collect();
        let pis_sol: Vec<Vec<U256>> = public_inputs
            .iter()
            .map(|pi| pi.iter().map(|f| field_to_u256(*f)).collect())
            .collect();
        let bad_pis_sol: Vec<Vec<U256>> = pis_sol
            .iter()
            .map(|pi| pi.iter().map(|_| field_to_u256(Fr::rand(rng))).collect())
            .collect();
        let proofs_sol: Vec<sol::PlonkProof> = proofs
            .iter()
            .map(|pf| pf.clone().generic_into::<sol::PlonkProof>())
            .collect();
        let bad_proofs_sol: Vec<sol::PlonkProof> = proofs_sol
            .iter()
            .map(|pf| {
                let mut bad_pf = pf.clone();
                bad_pf.wire_4 = G1Projective::rand(rng).into_affine().into();
                bad_pf.split_0 = G1Projective::rand(rng).into_affine().into();
                bad_pf.prod_perm_zeta_omega_eval = field_to_u256(Fr::rand(rng));
                bad_pf
            })
            .collect();
        let extra_msgs_sol: Vec<Bytes> = extra_msgs
            .iter()
            .map(|msg| {
                if let Some(msg) = msg {
                    Bytes::from(msg.clone())
                } else {
                    Bytes::default()
                }
            })
            .collect();
        let bad_extra_msgs_sol: Vec<Bytes> = extra_msgs_sol
            .iter()
            .map(|m| {
                if m == &Bytes::default() {
                    Bytes::from(b"random string".to_vec())
                } else {
                    Bytes::default()
                }
            })
            .collect();

        // reconnect to contract to avoid connection reset problem
        let client = get_funded_client().await?;
        let contract = TestPlonkVerifier::new(contract.address(), client);

        assert!(
            contract
                .test_batch_verify(
                    vks_sol.clone(),
                    pis_sol.clone(),
                    proofs_sol.clone(),
                    extra_msgs_sol.clone()
                )
                .call()
                .await?
        );
        assert!(
            !contract
                .test_batch_verify(
                    bad_vks_sol,
                    pis_sol.clone(),
                    proofs_sol.clone(),
                    extra_msgs_sol.clone(),
                )
                .call()
                .await?
        );
        assert!(
            !contract
                .test_batch_verify(
                    vks_sol.clone(),
                    bad_pis_sol,
                    proofs_sol.clone(),
                    extra_msgs_sol.clone(),
                )
                .call()
                .await?
        );
        assert!(
            !contract
                .test_batch_verify(
                    vks_sol.clone(),
                    pis_sol.clone(),
                    bad_proofs_sol,
                    extra_msgs_sol.clone(),
                )
                .call()
                .await?
        );
        assert!(
            !contract
                .test_batch_verify(vks_sol, pis_sol, proofs_sol, bad_extra_msgs_sol,)
                .call()
                .await?
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_proof_and_pub_inputs_validation() -> Result<()> {
    let rng = &mut test_rng();

    let (proof, vk, public_inputs, extra_msg, _domain_size) =
        gen_plonk_proof_for_test(1)?[0].clone();
    let proof_sol: sol::PlonkProof = proof.into();
    let vk_sol: sol::VerifyingKey = vk.into();
    let pi_sol: Vec<U256> = public_inputs.iter().map(|f| field_to_u256(*f)).collect();
    let extra_msg_sol: Bytes = if let Some(msg) = extra_msg {
        Bytes::from(msg)
    } else {
        Bytes::default()
    };

    let bad_point_choices: Vec<sol::G1Point> = [
        G1Affine::default(),
        G1Affine::new(Fq::rand(rng), Fq::rand(rng), false),
        G1Affine::new(Fq::rand(rng), Fq::rand(rng), false),
        G1Affine::new(Fq::rand(rng), Fq::rand(rng), false),
    ]
    .iter()
    .filter(|p| p.is_zero() || !p.is_on_curve())
    .map(|p| (*p).into())
    .collect();
    let bad_scalar_choices = vec![U256::MAX, U256::MAX - 1, U256::MAX - 100, U256::MAX - 1000];

    let contract = deploy_test_plonk_verifier_contract().await;

    // good path
    assert!(
        contract
            .test_batch_verify(
                vec![vk_sol.clone()],
                vec![pi_sol.to_vec()],
                vec![proof_sol.clone()],
                vec![extra_msg_sol.clone()],
            )
            .call()
            .await?
    );

    // bad path
    async fn test_bad_point_in_proof(
        contract: &TestPlonkVerifier<EthMiddleware>,
        vk_sol: &sol::VerifyingKey,
        pi_sol: &[U256],
        bad_proof: sol::PlonkProof,
        extra_msg_sol: &Bytes,
    ) {
        contract
            .test_batch_verify(
                vec![vk_sol.clone()],
                vec![pi_sol.to_vec()],
                vec![bad_proof],
                vec![extra_msg_sol.clone()],
            )
            .call()
            .await
            .should_revert_with_message("Bn254: invalid G1 point");
    }

    async fn test_bad_field_in_proof(
        contract: &TestPlonkVerifier<EthMiddleware>,
        vk_sol: &sol::VerifyingKey,
        pi_sol: &[U256],
        bad_proof: sol::PlonkProof,
        extra_msg_sol: &Bytes,
    ) {
        contract
            .test_batch_verify(
                vec![vk_sol.clone()],
                vec![pi_sol.to_vec()],
                vec![bad_proof],
                vec![extra_msg_sol.clone()],
            )
            .call()
            .await
            .should_revert_with_message("Bn254: invalid scalar field");
    }

    // test potentially malformed proof
    let mut bad_proof = proof_sol.clone();
    bad_proof.wire_0 = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.wire_1 = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.wire_2 = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.wire_3 = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.wire_4 = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.prod_perm = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.split_0 = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.split_1 = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.split_2 = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.split_3 = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.split_4 = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.zeta = bad_point_choices.choose(rng).unwrap().clone();
    test_bad_point_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.wire_eval_0 = bad_scalar_choices.choose(rng).unwrap().clone();
    test_bad_field_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.wire_eval_1 = bad_scalar_choices.choose(rng).unwrap().clone();
    test_bad_field_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.wire_eval_2 = bad_scalar_choices.choose(rng).unwrap().clone();
    test_bad_field_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.wire_eval_3 = bad_scalar_choices.choose(rng).unwrap().clone();
    test_bad_field_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.wire_eval_4 = bad_scalar_choices.choose(rng).unwrap().clone();
    test_bad_field_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.sigma_eval_0 = bad_scalar_choices.choose(rng).unwrap().clone();
    test_bad_field_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.sigma_eval_1 = bad_scalar_choices.choose(rng).unwrap().clone();
    test_bad_field_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.sigma_eval_2 = bad_scalar_choices.choose(rng).unwrap().clone();
    test_bad_field_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.sigma_eval_3 = bad_scalar_choices.choose(rng).unwrap().clone();
    test_bad_field_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    let mut bad_proof = proof_sol.clone();
    bad_proof.prod_perm_zeta_omega_eval = bad_scalar_choices.choose(rng).unwrap().clone();
    test_bad_field_in_proof(&contract, &vk_sol, &pi_sol, bad_proof, &extra_msg_sol).await;

    // test potentially malformed public input
    let mut bad_pi = pi_sol.clone();
    bad_pi[0] = bad_scalar_choices.choose(rng).unwrap().clone();
    contract
        .test_batch_verify(
            vec![vk_sol.clone()],
            vec![bad_pi],
            vec![proof_sol.clone()],
            vec![extra_msg_sol.clone()],
        )
        .call()
        .await
        .should_revert_with_message("Bn254: invalid scalar field");

    Ok(())
}
