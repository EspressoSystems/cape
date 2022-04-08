// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]

use crate::deploy::deploy_test_ed_on_bn_254_contract;
use crate::types::EdOnBN254Point;
use anyhow::Result;
use ark_ec::AffineCurve;
use ark_ed_on_bn254::EdwardsAffine;
use ark_serialize::CanonicalSerialize;
use ark_std::UniformRand;
use ark_std::Zero;

#[tokio::test]
async fn test_serialization() -> Result<()> {
    let rng = &mut ark_std::test_rng();

    // somehow deploying this contract returns an error
    let contract = deploy_test_ed_on_bn_254_contract().await;
    let mut rust_ser = Vec::new();

    // infinity
    let point = EdwardsAffine::zero();
    point.serialize(&mut rust_ser)?;
    let sol_point: EdOnBN254Point = point.into();
    let sol_ser = contract.serialize(sol_point.into()).call().await?;

    assert_eq!(sol_ser.to_vec(), rust_ser);

    // generator
    rust_ser = Vec::new();
    let point = EdwardsAffine::prime_subgroup_generator();
    point.serialize(&mut rust_ser)?;
    let sol_point: EdOnBN254Point = point.into();
    let sol_ser = contract.serialize(sol_point.into()).call().await?;

    assert_eq!(sol_ser.to_vec(), rust_ser);

    for _ in 0..10 {
        rust_ser = Vec::new();
        let point: EdwardsAffine = EdwardsAffine::rand(rng);
        point.serialize(&mut rust_ser)?;

        let sol_point: EdOnBN254Point = point.into();
        let sol_ser = contract.serialize(sol_point.into()).call().await?;

        assert_eq!(sol_ser.to_vec(), rust_ser);
    }
    Ok(())
}
