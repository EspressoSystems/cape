use cap_rust_sandbox::types as sol;
use jf_aap::proof::{freeze, mint, transfer};
use jf_aap::structs::NoteType;
use jf_aap::testing_apis::universal_setup_for_test;
use std::{fs::OpenOptions, io::prelude::*, path::PathBuf};

const TREE_DEPTH: u8 = 24;
const SUPPORTED_VKS: [(NoteType, u8, u8, u8); 3] = [
    (NoteType::Transfer, 2, 2, TREE_DEPTH),
    (NoteType::Mint, 1, 2, TREE_DEPTH),
    (NoteType::Freeze, 3, 3, TREE_DEPTH),
];

fn main() {
    let max_degree = 2usize.pow(17);
    let rng = &mut ark_std::test_rng();
    let srs = universal_setup_for_test(max_degree, rng).unwrap();

    for (note_type, num_input, num_output, tree_depth) in SUPPORTED_VKS {
        // calculate the path to solidity file
        let contract_name = get_solidity_file_name(note_type, num_input, num_output, tree_depth);
        let mut path = PathBuf::new();
        path.push(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.push("contracts/libraries");
        path.push(contract_name.clone());
        path.set_extension("sol");

        // overwrite the file
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();

        // the contract
        let vk = match note_type {
            NoteType::Transfer => {
                let (_, vk, _) =
                    transfer::preprocess(&srs, num_input as usize, num_output as usize, tree_depth)
                        .unwrap();
                vk.get_verifying_key()
            }
            NoteType::Mint => {
                let (_, vk, _) = mint::preprocess(&srs, tree_depth).unwrap();
                vk.get_verifying_key()
            }
            NoteType::Freeze => {
                let (_, vk, _) = freeze::preprocess(&srs, num_input as usize, tree_depth).unwrap();
                vk.get_verifying_key()
            }
        };
        let vk: sol::VerifyingKey = vk.into();

        let code = format!(
            "// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import \"../interfaces/IPlonkVerifier.sol\";
import \"./BN254.sol\";

library {} {{
    function getVk() internal pure returns (IPlonkVerifier.VerifyingKey memory vk) {{
        assembly {{
            // domain size
            mstore(vk, {})
            // num of public inputs
            mstore(add(vk, 0x20), {})

            // sigma0
            mstore(mload(add(vk, 0x40)), {})
            mstore(add(mload(add(vk, 0x40)), 0x20), {})
            // sigma1
            mstore(mload(add(vk, 0x60)), {})
            mstore(add(mload(add(vk, 0x60)), 0x20), {})
            // sigma2
            mstore(mload(add(vk, 0x80)), {})
            mstore(add(mload(add(vk, 0x80)), 0x20), {})
            // sigma3
            mstore(mload(add(vk, 0xa0)), {})
            mstore(add(mload(add(vk, 0xa0)), 0x20), {})
            // sigma4
            mstore(mload(add(vk, 0xc0)), {})
            mstore(add(mload(add(vk, 0xc0)), 0x20), {})

            // q1
            mstore(mload(add(vk, 0xe0)), {})
            mstore(add(mload(add(vk, 0xe0)), 0x20), {})
            // q2
            mstore(mload(add(vk, 0x100)), {})
            mstore(add(mload(add(vk, 0x100)), 0x20), {})
            // q3
            mstore(mload(add(vk, 0x120)), {})
            mstore(add(mload(add(vk, 0x120)), 0x20), {})
            // q4
            mstore(mload(add(vk, 0x140)), {})
            mstore(add(mload(add(vk, 0x140)), 0x20), {})

            // qM12
            mstore(mload(add(vk, 0x160)), {})
            mstore(add(mload(add(vk, 0x160)), 0x20), {})
            // qM34
            mstore(mload(add(vk, 0x180)), {})
            mstore(add(mload(add(vk, 0x180)), 0x20), {})

             // qO
            mstore(mload(add(vk, 0x1a0)), {})
            mstore(add(mload(add(vk, 0x1a0)), 0x20), {})
             // qC
            mstore(mload(add(vk, 0x1c0)), {})
            mstore(add(mload(add(vk, 0x1c0)), 0x20), {})
             // qH1
            mstore(mload(add(vk, 0x1e0)), {})
            mstore(add(mload(add(vk, 0x1e0)), 0x20), {})
             // qH2
            mstore(mload(add(vk, 0x200)), {})
            mstore(add(mload(add(vk, 0x200)), 0x20), {})
             // qH3
            mstore(mload(add(vk, 0x220)), {})
            mstore(add(mload(add(vk, 0x220)), 0x20), {})
             // qH4
            mstore(mload(add(vk, 0x240)), {})
            mstore(add(mload(add(vk, 0x240)), 0x20), {})
             // qEcc
            mstore(mload(add(vk, 0x260)), {})
            mstore(add(mload(add(vk, 0x260)), 0x20), {})
        }}
    }}
}}
",
            contract_name,
            vk.domain_size,
            vk.num_inputs,
            vk.sigma_0.x,
            vk.sigma_0.y,
            vk.sigma_1.x,
            vk.sigma_1.y,
            vk.sigma_2.x,
            vk.sigma_2.y,
            vk.sigma_3.x,
            vk.sigma_3.y,
            vk.sigma_4.x,
            vk.sigma_4.y,
            vk.q_1.x,
            vk.q_1.y,
            vk.q_2.x,
            vk.q_2.y,
            vk.q_3.x,
            vk.q_3.y,
            vk.q_4.x,
            vk.q_4.y,
            vk.q_m12.x,
            vk.q_m12.y,
            vk.q_m34.x,
            vk.q_m34.y,
            vk.q_o.x,
            vk.q_o.y,
            vk.q_c.x,
            vk.q_c.y,
            vk.q_h1.x,
            vk.q_h1.y,
            vk.q_h2.x,
            vk.q_h2.y,
            vk.q_h3.x,
            vk.q_h3.y,
            vk.q_h4.x,
            vk.q_h4.y,
            vk.q_ecc.x,
            vk.q_ecc.y,
        )
        .into_bytes();

        file.write_all(&code).unwrap();
    }
}

// example: "Transfer2In2Out24DepthVk" (no extension)
fn get_solidity_file_name(
    note_type: NoteType,
    num_input: u8,
    num_output: u8,
    tree_depth: u8,
) -> String {
    format!(
        "{}{}In{}Out{}DepthVk",
        match note_type {
            NoteType::Transfer => "Transfer",
            NoteType::Mint => "Mint",
            NoteType::Freeze => "Freeze",
        },
        num_input,
        num_output,
        tree_depth
    )
}
