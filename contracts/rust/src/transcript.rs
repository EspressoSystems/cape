// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]
use crate::deploy::deploy_test_transcript_contract;
use crate::types as sol;
use crate::types::{field_to_u256, G1Point, TranscriptData};
use ark_bn254::{g1::Parameters as G1, Bn254 as E, Fq, Fr, G1Affine, G1Projective};
use ark_ff::Zero;
use ark_poly_commit::kzg10::Commitment;
use ark_std::UniformRand;
use jf_plonk::proof_system::structs::VerifyingKey;
use jf_plonk::proof_system::structs::{Proof, ProofEvaluations};
use jf_plonk::transcript::PlonkTranscript;
use jf_plonk::transcript::SolidityTranscript;
use jf_utils::field_switching;
use rand::Rng;
use std::convert::TryInto;

fn mk_empty_transcript() -> impl PlonkTranscript<Fq> {
    <SolidityTranscript as PlonkTranscript<Fq>>::new(b"ignored")
}

#[tokio::test]
async fn test_append_empty() {
    let contract = deploy_test_transcript_contract().await;
    let mut transcript = mk_empty_transcript();
    let challenge = transcript
        .get_and_append_challenge::<E>(b"ignored")
        .unwrap();

    let ethers_transcript = TranscriptData::default();
    let ret = contract
        .get_and_append_challenge(ethers_transcript)
        .call()
        .await
        .unwrap();
    assert_eq!(ret, field_to_u256(challenge));
}

#[tokio::test]
async fn test_append_message() {
    let contract = deploy_test_transcript_contract().await;
    let mut rng = ark_std::test_rng();
    for _test in 0..10 {
        let mut transcript = mk_empty_transcript();
        let message = rng.gen::<[u8; 32]>();
        transcript.append_message(b"ignored", &message).unwrap();
        let challenge = transcript
            .get_and_append_challenge::<E>(b"ignored")
            .unwrap();
        let ret = contract
            .test_append_message_and_get(TranscriptData::default(), message.into())
            .call()
            .await
            .unwrap();
        assert_eq!(ret, field_to_u256(challenge));
    }
}

#[tokio::test]
async fn test_append_challenge() {
    let contract = deploy_test_transcript_contract().await;
    let mut rng = ark_std::test_rng();
    for _test in 0..10 {
        let mut transcript = mk_empty_transcript();
        let first_challenge = Fr::rand(&mut rng);

        transcript
            .append_challenge::<E>(b"ignored", &first_challenge)
            .unwrap();

        let final_challenge = transcript
            .get_and_append_challenge::<E>(b"ignored")
            .unwrap();

        let ret = contract
            .test_append_challenge_and_get(
                TranscriptData::default(),
                field_to_u256(first_challenge),
            )
            .call()
            .await
            .unwrap();

        assert_eq!(ret, field_to_u256(final_challenge));
    }
}

#[tokio::test]
async fn test_get_and_append_challenge_multiple_times() {
    let contract = deploy_test_transcript_contract().await;
    let mut rng = ark_std::test_rng();
    for _test in 0..10 {
        let mut transcript = mk_empty_transcript();
        let times: u64 = rng.gen_range(0..10);
        let mut challenge = Fr::zero();
        for _round in 0..times {
            challenge = transcript
                .get_and_append_challenge::<E>(b"ignored")
                .unwrap()
        }

        let ret = contract
            .test_get_and_append_challenge_multiple_times(TranscriptData::default(), times.into())
            .call()
            .await
            .unwrap();

        assert_eq!(ret, field_to_u256(challenge));
    }
}

#[tokio::test]
async fn test_append_commitment() {
    let contract = deploy_test_transcript_contract().await;
    let mut rng = ark_std::test_rng();
    for _test in 0..10 {
        let mut transcript = mk_empty_transcript();
        let g1_point: G1Affine = G1Projective::rand(&mut rng).into();
        let commitment = Commitment(g1_point);
        let ethers_commitment: G1Point = g1_point.into();
        transcript
            .append_commitments::<E, G1>(b"ignored", &[commitment])
            .unwrap();

        let challenge = transcript
            .get_and_append_challenge::<E>(b"ignored")
            .unwrap();

        let ret = contract
            .test_append_commitment_and_get(TranscriptData::default(), ethers_commitment)
            .call()
            .await
            .unwrap();

        assert_eq!(ret, field_to_u256(challenge));
    }
}

#[tokio::test]
async fn test_append_commitments() {
    let contract = deploy_test_transcript_contract().await;
    let mut rng = ark_std::test_rng();
    for _test in 0..10 {
        let mut transcript = mk_empty_transcript();

        let num_comm = rng.gen_range(0..10);
        let points: Vec<G1Affine> = (0..num_comm)
            .map(|_| G1Projective::rand(&mut rng).into())
            .collect();
        let comms: Vec<Commitment<E>> = points.iter().map(|&p| Commitment(p)).collect();
        let ethers_comms: Vec<G1Point> = points.iter().map(|&p| p.into()).collect();

        transcript
            .append_commitments::<E, G1>(b"ignored", &comms)
            .unwrap();

        let challenge = transcript
            .get_and_append_challenge::<E>(b"ignored")
            .unwrap();

        let ret = contract
            .test_append_commitments_and_get(TranscriptData::default(), ethers_comms)
            .call()
            .await
            .unwrap();

        assert_eq!(ret, field_to_u256(challenge));
    }
}

#[tokio::test]
async fn test_infinity_commitment() {
    let contract = deploy_test_transcript_contract().await;
    let mut transcript = mk_empty_transcript();
    let g1_zero = G1Affine::zero();
    let commitment = Commitment(g1_zero);
    let ethers_commitment: G1Point = g1_zero.into();
    transcript
        .append_commitments::<E, G1>(b"ignored", &[commitment])
        .unwrap();

    let challenge = transcript
        .get_and_append_challenge::<E>(b"ignored")
        .unwrap();

    let ret = contract
        .test_append_commitment_and_get(TranscriptData::default(), ethers_commitment)
        .call()
        .await
        .unwrap();

    assert_eq!(ret, field_to_u256(challenge));
}

#[tokio::test]
async fn test_append_vk_and_public_inputs() {
    let contract = deploy_test_transcript_contract().await;
    let mut rng = ark_std::test_rng();
    for _test in 0..10 {
        let rust_verifying_key = VerifyingKey::<E>::dummy(10, 1024);
        let num_comm = rng.gen_range(0..10);
        let rust_public_inputs: Vec<Fr> = (0..num_comm).map(|_| Fr::rand(&mut rng)).collect();

        // rust side
        let mut rust_transcript = mk_empty_transcript();
        rust_transcript
            .append_vk_and_pub_input(&rust_verifying_key, &rust_public_inputs)
            .unwrap();

        let challenge = rust_transcript
            .get_and_append_challenge::<E>(b"ignored")
            .unwrap();

        // solidity side
        let ethers_transcript = TranscriptData::default();
        let ether_verifying_key: sol::VerifyingKey = rust_verifying_key.into();
        let ether_public_inputs = rust_public_inputs
            .iter()
            .map(|&x| field_to_u256(x))
            .collect();
        let ethers_transcript = contract
            .test_append_vk_and_pub_input(
                ethers_transcript,
                ether_verifying_key,
                ether_public_inputs,
            )
            .call()
            .await
            .unwrap();
        let ret = contract
            .get_and_append_challenge(ethers_transcript)
            .call()
            .await
            .unwrap();

        assert_eq!(ret, field_to_u256(challenge));
    }
}

#[tokio::test]
async fn test_append_proof_evaluation() {
    let contract = deploy_test_transcript_contract().await;
    let mut rng = ark_std::test_rng();
    for _test in 0..10 {
        let proof_fr_elements: Vec<Fr> = (0..36).map(|_| Fr::rand(&mut rng)).collect();
        let proof_fq_elements: Vec<Fq> = proof_fr_elements
            .iter()
            .map(|x| field_switching(x))
            .collect();
        let rust_proof: Proof<E> = proof_fq_elements.try_into().unwrap();
        let rust_proof_eval: ProofEvaluations<Fr> =
            proof_fr_elements[26..36].to_vec().try_into().unwrap();

        // rust side
        let mut rust_transcript = mk_empty_transcript();
        rust_transcript
            .append_proof_evaluations::<E>(&rust_proof_eval)
            .unwrap();

        let challenge = rust_transcript
            .get_and_append_challenge::<E>(b"ignored")
            .unwrap();

        // solidity side
        let ethers_transcript = TranscriptData::default();
        let sol_proof: sol::PlonkProof = rust_proof.into();
        let ethers_transcript = contract
            .test_append_proof_evaluations(ethers_transcript, sol_proof)
            .call()
            .await
            .unwrap();
        let ret = contract
            .get_and_append_challenge(ethers_transcript)
            .call()
            .await
            .unwrap();

        assert_eq!(ret, field_to_u256(challenge));
    }
}
