#![cfg(test)]
use std::path::Path;

use crate::{
    ethereum,
    types::{field_to_u256, G1Point, TestTranscript, TranscriptData},
};
use ark_ff::Zero;
use ark_poly_commit::kzg10::Commitment;
use ark_std::UniformRand;
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::{Http, Provider, SignerMiddleware, Wallet};
use jf_plonk::transcript::PlonkTranscript;
use jf_plonk::FiatShamirHash;

use ark_bn254::{g1::Parameters as G1, Bn254 as E, Fq, Fr, G1Affine, G1Projective};
use rand::Rng;

async fn deploy() -> TestTranscript<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
    let client = ethereum::get_funded_deployer().await.unwrap();
    let contract = ethereum::deploy(
        client.clone(),
        Path::new("../artifacts/contracts/mocks/TestTranscript.sol/TestTranscript"),
        (),
    )
    .await
    .unwrap();
    TestTranscript::new(contract.address(), client)
}

fn mk_empty_transcript() -> PlonkTranscript<Fq> {
    PlonkTranscript::new(b"ignored", FiatShamirHash::SolidityKeccak)
}

#[tokio::test]
async fn test_append_empty() {
    let contract = deploy().await;
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
    let contract = deploy().await;
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
    let contract = deploy().await;
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
    let contract = deploy().await;
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
    let contract = deploy().await;
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
    let contract = deploy().await;
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
    let contract = deploy().await;
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
