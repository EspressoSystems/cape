#![cfg(test)]

use crate::deploy::{deploy_test_bn254_contract, EthMiddleware};
use crate::{
    assertion::Matcher,
    types::{field_to_u256, u256_to_field, G1Point, G2Point, TestBN254},
};
use anyhow::Result;
use ark_bn254::{Fq, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::msm::VariableBaseMSM;
use ark_ec::AffineCurve;
use ark_ec::{group::Group, ProjectiveCurve};
use ark_ff::SquareRootField;
use ark_ff::{field_new, to_bytes, Field, LegendreSymbol};
use ark_ff::{FpParameters, PrimeField};
use ark_serialize::CanonicalSerialize;
use ark_std::UniformRand;
use ark_std::Zero;
use ethers::prelude::*;
use rand::RngCore;

#[tokio::test]
async fn test_quadratic_residue() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_bn254_contract().await;

    // test random group addition
    for _ in 0..100 {
        let x = Fq::rand(rng);
        let (is_residue, y) = contract.quadratic_residue(field_to_u256(x)).call().await?;
        assert_eq!(x.legendre() == LegendreSymbol::QuadraticResidue, is_residue);
        if is_residue {
            let y = u256_to_field::<Fq>(y);
            assert_eq!(x, y * y);
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_fr_negate() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_bn254_contract().await;

    for _ in 0..10 {
        let p: Fr = Fr::rand(rng).into();
        let minus_p_sol: Fr = u256_to_field(contract.negate_fr(field_to_u256(p)).call().await?);
        assert_eq!(minus_p_sol, (-p).into());
    }

    Ok(())
}

#[tokio::test]
async fn test_add() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_bn254_contract().await;

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
    let contract = deploy_test_bn254_contract().await;

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
    let contract = deploy_test_bn254_contract().await;

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
    let contract = deploy_test_bn254_contract().await;

    for _ in 0..10 {
        let p: G1Affine = G1Projective::rand(rng).into();
        let minus_p_sol: G1Point = contract.negate_g1(p.into()).call().await?.into();
        assert_eq!(minus_p_sol, (-p).into());
    }

    Ok(())
}

#[tokio::test]
async fn test_scalar_mul() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_bn254_contract().await;

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
async fn test_multi_scalar_mul() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_bn254_contract().await;

    for length in 1..10 {
        let p_rust: Vec<G1Affine> = (0..length)
            .map(|_| G1Projective::rand(rng).into_affine())
            .collect();
        let p_solidity: Vec<G1Point> = p_rust.iter().map(|&x| x.into()).collect();

        let s_rust: Vec<Fr> = (0..length).map(|_| Fr::rand(rng)).collect();
        let s_solidity: Vec<U256> = s_rust.iter().map(|&x| field_to_u256(x)).collect();
        let s_rust: Vec<_> = s_rust.iter().map(|&x| x.into_repr()).collect();

        let res: G1Point = contract
            .test_multi_scalar_mul(p_solidity, s_solidity)
            .call()
            .await?
            .into();

        assert_eq!(
            res,
            VariableBaseMSM::multi_scalar_mul(&p_rust, &s_rust)
                .into_affine()
                .into()
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_is_y_negative() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_bn254_contract().await;

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
    let contract = deploy_test_bn254_contract().await;

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
    let contract = deploy_test_bn254_contract().await;
    let p: G1Affine = G1Projective::rand(rng).into();
    contract.validate_g1_point(p.into()).call().await?;

    async fn should_fail_validation(contract: &TestBN254<EthMiddleware>, bad_p: G1Point) {
        contract
            .validate_g1_point(bad_p)
            .call()
            .await
            .should_revert_with_message("Bn254: invalid G1 point");
    }

    // x = 0 should fail
    let mut bad_p = p.clone();
    bad_p.x = field_new!(Fq, "0");
    should_fail_validation(&contract, bad_p.into()).await;

    // y = 0 should fail
    let mut bad_p = p.clone();
    bad_p.y = field_new!(Fq, "0");
    should_fail_validation(&contract, bad_p.into()).await;

    // x > p should fail
    let mut bad_p_g1: G1Point = p.clone().into();
    bad_p_g1.x = U256::MAX;
    should_fail_validation(&contract, bad_p_g1).await;

    // y > p should fail
    let mut bad_p_g1: G1Point = p.clone().into();
    bad_p_g1.y = U256::MAX;
    should_fail_validation(&contract, bad_p_g1).await;

    // not on curve point (y^2 = x^3 + 3 mod p) should fail
    let bad_p = G1Affine::new(field_new!(Fq, "1"), field_new!(Fq, "3"), false);
    should_fail_validation(&contract, bad_p.into()).await;
    Ok(())
}

#[tokio::test]
async fn test_validate_scalar_field() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_bn254_contract().await;
    let f = Fr::rand(rng);
    contract
        .validate_scalar_field(field_to_u256(f))
        .call()
        .await?;

    contract
        .validate_scalar_field(
            U256::from_str_radix(
                "21888242871839275222246405745257275088548364400416034343698204186575808495618",
                10,
            )
            .unwrap(),
        )
        .call()
        .await
        .should_revert_with_message("Bn254: invalid scalar field");
    Ok(())
}

#[tokio::test]
async fn test_pairing_prod() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_bn254_contract().await;
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
    let contract = deploy_test_bn254_contract().await;

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

        let mut bytes = [0u8; 48];
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
    let contract = deploy_test_bn254_contract().await;
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

#[tokio::test]
async fn test_serialization() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_bn254_contract().await;
    let mut rust_ser = Vec::new();

    // infinity
    let point: G1Affine = G1Affine::zero();
    point.serialize(&mut rust_ser)?;
    let sol_point: G1Point = point.into();
    let sol_ser = contract.g_1_serialize(sol_point.into()).call().await?;

    assert_eq!(sol_ser.to_vec(), rust_ser);

    // generator
    rust_ser = Vec::new();
    let point = G1Affine::prime_subgroup_generator();
    point.serialize(&mut rust_ser)?;
    let sol_point: G1Affine = point.into();
    let sol_ser = contract.g_1_serialize(sol_point.into()).call().await?;

    assert_eq!(sol_ser.to_vec(), rust_ser);

    for _ in 0..10 {
        rust_ser = Vec::new();
        let point: G1Affine = G1Projective::rand(rng).into();
        point.serialize(&mut rust_ser)?;

        let sol_point: G1Point = point.into();
        let sol_ser = contract.g_1_serialize(sol_point.into()).call().await?;

        assert_eq!(sol_ser.to_vec(), rust_ser);
    }
    Ok(())
}

#[tokio::test]
async fn test_deserialization() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_bn254_contract().await;
    let mut rust_buf = Vec::new();
    let mut sol_buf = [0u8; 32];

    // infinity
    let point: G1Affine = G1Affine::zero();
    point.serialize(&mut rust_buf)?;
    sol_buf.copy_from_slice(&rust_buf);
    let sol_point: G1Point = contract.g_1_deserialize(sol_buf).call().await?;

    assert_eq!(sol_point, point.into());

    // generator
    rust_buf = Vec::new();
    let point = G1Affine::prime_subgroup_generator();
    point.serialize(&mut rust_buf)?;
    sol_buf.copy_from_slice(&rust_buf);
    let sol_point: G1Point = contract.g_1_deserialize(sol_buf).call().await?;
    assert_eq!(sol_point, point.into());

    for _ in 0..10 {
        rust_buf = Vec::new();
        let point: G1Affine = G1Projective::rand(rng).into();
        point.serialize(&mut rust_buf)?;
        sol_buf.copy_from_slice(&rust_buf);
        let sol_point: G1Point = contract.g_1_deserialize(sol_buf).call().await?;
        assert_eq!(sol_point, point.into());
    }
    Ok(())
}
