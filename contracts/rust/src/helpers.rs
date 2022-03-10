// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use ark_ed_on_bn254::Fq as Fr254;
use ark_ff::{BigInteger, PrimeField};
use ark_serialize::*;
use ethers::prelude::*;
use jf_cap::structs::Nullifier;
use jf_cap::NodeValue;

pub fn convert_u256_to_bytes_le(num: U256) -> Vec<u8> {
    let mut u8_arr = [0u8; 32];
    num.to_little_endian(&mut u8_arr);
    u8_arr.to_vec()
}

pub fn convert_fr254_to_u256(f: Fr254) -> U256 {
    U256::from(f.into_repr().to_bytes_be().as_slice())
}

pub fn compare_merkle_root_from_contract_and_jf_tree(
    contract_root_value: U256,
    jellyfish_mt_root_value: NodeValue,
) -> bool {
    convert_u256_to_bytes_le(contract_root_value).as_slice()
        == jellyfish_mt_root_value
            .to_scalar()
            .into_repr()
            .to_bytes_le()
}

pub fn convert_nullifier_to_u256(n: &Nullifier) -> U256 {
    let mut buffer: Vec<u8> = vec![];
    let _ = n.serialize(&mut buffer);
    U256::from(buffer.as_slice())
}
