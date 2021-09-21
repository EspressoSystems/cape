use ethers::prelude::{abigen, U256};

use crate::{G1Ark, G1Ethers};

abigen!(
    TestBN254,
    "./contracts/testBN254/abi.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

impl From<G1Ark> for G1Point {
    fn from(point: G1Ark) -> Self {
        let ethers_point = G1Ethers::from(point);
        Self {
            x: ethers_point.x,
            y: ethers_point.y,
        }
    }
}

// (U256, U256) is what contract calls return for G1Point
impl From<(U256, U256)> for G1Point {
    fn from(tuple: (U256, U256)) -> Self {
        return Self {
            x: tuple.0,
            y: tuple.1,
        };
    }
}

impl From<G1Point> for G1Ark {
    fn from(point: G1Point) -> Self {
        G1Ethers {
            x: point.x,
            y: point.y,
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        ethereum::{deploy, get_funded_deployer},
        G1Affine, G1Ark, G1Projective, Zero,
    };
    use ark_ec::{models::SWModelParameters, AffineCurve, PairingEngine, ProjectiveCurve};

    use ark_std::UniformRand;
    use ethers::prelude::Middleware;
    use num_traits::real::Real;
    use std::path::Path;

    #[tokio::test]
    async fn test_add_g1_group_elements_in_contract() {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(client.clone(), Path::new("./contracts/testBN254"))
            .await
            .unwrap();
        let contract = TestBN254::new(contract.address(), client);

        async fn add<M: Middleware>(contract: &TestBN254<M>, a: G1Affine, b: G1Affine) -> G1Affine {
            let res: G1Point = contract
                .g_1add(G1Ark(a).into(), G1Ark(b).into())
                .call()
                .await
                .unwrap()
                .into();
            *G1Ark::from(res)
        }

        let mut rng = ark_std::test_rng();
        let p1: G1Affine = G1Projective::rand(&mut rng).into();
        let p2: G1Affine = G1Projective::rand(&mut rng).into();
        let zero = G1Affine::zero();

        assert_ne!(add(&contract, p1, p2).await, p1);

        // p + q
        assert_eq!(add(&contract, p1, p2).await, p1 + p2);

        // 0 + 0 = 0
        assert_eq!(add(&contract, zero, zero).await, zero);

        // p + 0 = 0
        assert_eq!(add(&contract, p1, zero).await, p1);

        // 0 + p = 0
        assert_eq!(add(&contract, zero, p2).await, p2);

        async fn mul<M: Middleware>(contract: &TestBN254<M>, a: G1Affine, s: U256) -> G1Affine {
            let res: G1Point = contract
                .g_1mul(G1Ark(a).into(), s)
                .call()
                .await
                .unwrap()
                .into();
            *G1Ark::from(res)
        }

        // g1mul by 1
        assert_eq!(mul(&contract, p1, U256::from(1)).await, p1);

        // g1mul by 2
        let x2 = mul(&contract, p1, U256::from(2)).await;
        assert_eq!(x2, p1 + p1);

        // g1add(g1add(A, A), A) = g1mul(A, 3)
        let x2 = add(&contract, p1, p1).await;
        let x3_via_add = add(&contract, x2, p1).await;
        let x3_via_mul = mul(&contract, p1, U256::from(3)).await;
        assert_eq!(x3_via_add, x3_via_mul);
        assert_eq!(x3_via_add, p1 + p1 + p1);
        let x3_ark = p1.mul(ark_bn254::Fr::from(3u128));
        assert_eq!(x3_via_mul, x3_ark);
    }
}
