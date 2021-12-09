#![cfg(test)]

use crate::{
    ethereum::{deploy, get_funded_deployer},
    types::{field_to_u256, G1Point, G2Point, TestBN254},
};
use anyhow::Result;
use ark_bn254::{Fq, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::AffineCurve;
use ark_ec::{group::Group, ProjectiveCurve};
use ark_ff::{field_new, to_bytes, Field};
use ark_ff::{FpParameters, PrimeField};
use ark_std::UniformRand;
use ark_std::Zero;
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use rand::RngCore;
use std::path::Path;

async fn deploy_contract() -> Result<TestBN254<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>>
{
    let client = get_funded_deployer().await.unwrap();
    let contract = deploy(
        client.clone(),
        Path::new("../artifacts/contracts/TestBN254.sol/TestBN254"),
        (),
    )
    .await
    .unwrap();
    Ok(TestBN254::new(contract.address(), client))
}

#[tokio::test]
async fn test_add() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_contract().await?;

    // test random group addition
    for _ in 0..10 {
        let p1: G1Affine = G1Projective::rand(rng).into();
        let p2: G1Affine = G1Projective::rand(rng).into();
        let res: G1Point = contract.add(p1.into(), p2.into()).call().await?.into();
        assert_eq!(res, (p1 + p2).into());
    }

    // test point of infinity, O_E + P = P
    let zero = G1Affine::zero();
    let p: G1Affine = G1Projective::rand(rng).into();
    let res: G1Point = contract.add(p.into(), zero.into()).call().await?.into();
    assert_eq!(res, p.into());

    Ok(())
}

#[tokio::test]
async fn test_group_generators() -> Result<()> {
    let contract = deploy_contract().await?;

    let g1_gen = G1Affine::prime_subgroup_generator();
    let g2_gen = G2Affine::prime_subgroup_generator();

    let g1_gen_sol: G1Point = contract.p1().call().await?.into();
    let g2_gen_sol: G2Point = contract.p2().call().await?.into();
    assert_eq!(g1_gen_sol, g1_gen.into());
    assert_eq!(g2_gen_sol, g2_gen.into());

    Ok(())
}

#[tokio::test]
async fn test_is_infinity() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_contract().await?;

    let zero = G1Affine::zero();
    assert!(contract.is_infinity(zero.into()).call().await?);
    for _ in 0..10 {
        let non_zero: G1Affine = G1Projective::rand(rng).into();
        assert!(!contract.is_infinity(non_zero.into()).call().await?);
    }

    Ok(())
}

#[tokio::test]
async fn test_negate() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_contract().await?;

    for _ in 0..10 {
        let p: G1Affine = G1Projective::rand(rng).into();
        let minus_p_sol: G1Point = contract.negate(p.into()).call().await?.into();
        assert_eq!(minus_p_sol, (-p).into());
    }

    Ok(())
}

#[tokio::test]
async fn test_scalar_mul() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_contract().await?;

    for _ in 0..10 {
        let p = G1Projective::rand(rng);
        let s = Fr::rand(rng);

        let res: G1Point = contract
            .scalar_mul(p.into_affine().into(), field_to_u256(s))
            .call()
            .await?
            .into();
        assert_eq!(res, Group::mul(&p, &s).into_affine().into());
    }

    Ok(())
}

#[tokio::test]
async fn test_is_y_negative() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_contract().await?;

    for _ in 0..10 {
        let p: G1Affine = G1Projective::rand(rng).into();
        // https://github.com/arkworks-rs/algebra/blob/98f43af6cb0a4620b78dbb3f46d3c2794bbfc66f/ec/src/models/short_weierstrass_jacobian.rs#L776
        let is_negative = p.y < -p.y;
        assert_eq!(contract.is_y_negative(p.into()).call().await?, is_negative);
        assert_eq!(
            contract.is_y_negative((-p).into()).call().await?,
            !is_negative
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_invert() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_contract().await?;

    for _ in 0..10 {
        let f = Fr::rand(rng);
        assert_eq!(
            contract.invert(field_to_u256(f)).call().await?,
            field_to_u256(f.inverse().unwrap())
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_validate_g1_point() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_contract().await?;
    let p: G1Affine = G1Projective::rand(rng).into();
    contract.validate_g1_point(p.into()).call().await?;

    let mut bad_p = p.clone();
    bad_p.x = field_new!(Fq, "0");
    // FIXME: add expect().to.revertWith("assert message") helper function
    eprintln!(
        "await result: {:?}",
        contract.validate_g1_point(bad_p.into()).call().await
    );
    // contract.validate_g1_point(bad_p.into()).call().await?;
    Ok(())
}

#[tokio::test]
async fn test_pairing_prod() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_contract().await?;
    let g1_base = G1Projective::prime_subgroup_generator();
    let g2_base = G2Projective::prime_subgroup_generator();

    for _ in 0..10 {
        // multiplicative notation: e(g1^x1, g2^x2) * e(g1^-x2, g2^x1) == 1
        // let a1 = g1^x1, a2 = g2^x2; b1 = g1^x2, b2 = g2^x1.
        // additive notation: e(a1, a2)*(-b1, b2) == 1
        let x1 = Fr::rand(rng);
        let x2 = Fr::rand(rng);
        let a1 = Group::mul(&g1_base, &x1).into_affine();
        let a2 = Group::mul(&g2_base, &x2).into_affine();
        let b1 = Group::mul(&g1_base, &x2).into_affine();
        let b2 = Group::mul(&g2_base, &x1).into_affine();
        assert!(
            contract
                .pairing_prod_2(a1.into(), a2.into(), (-b1).into(), b2.into())
                .call()
                .await?
        )
    }
    Ok(())
}

#[tokio::test]
async fn test_from_le_bytes_mod_order() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_contract().await?;

    for _ in 0..10 {
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        assert_eq!(
            contract
                .from_le_bytes_mod_order(bytes.to_vec().into())
                .call()
                .await?,
            field_to_u256(Fr::from_le_bytes_mod_order(&bytes))
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_pow_small() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_contract().await?;
    let modulus = <<Fr as PrimeField>::Params as FpParameters>::MODULUS;

    for _ in 0..10 {
        // pow_small userful when evaluating of Z_H(X) = X^n - 1 at random points
        let base = Fr::rand(rng);
        let exponent = u64::rand(rng); // small exponent (<= 64 bit)
        assert_eq!(
            contract
                .pow_small(
                    field_to_u256(base),
                    field_to_u256(Fr::from(exponent)),
                    U256::from_little_endian(&to_bytes!(modulus).unwrap()),
                )
                .call()
                .await?,
            field_to_u256(base.pow([exponent])),
        );
    }
    Ok(())
}
