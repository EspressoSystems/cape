use ethers::prelude::{Bytes, U256};

use jf_txn::transfer::TransferNote;

use crate::helpers::{convert_fr254_to_u256, convert_nullifier_to_u256};
use crate::types::CapeTransaction;
use itertools::Itertools;

fn to_solidity(note: &TransferNote) -> CapeTransaction {
    return CapeTransaction {
        nullifiers: note
            .inputs_nullifiers
            .clone()
            .iter()
            .map(|v| convert_nullifier_to_u256(v))
            .collect_vec(),
    };
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::types::CAPE;
    use ethers::prelude::U256;

    use crate::cap_jf::create_anon_xfr_2in_3out;
    use crate::cape::to_solidity;
    use crate::ethereum::{deploy, get_funded_deployer};

    #[tokio::test]
    async fn test_submit_block_to_cape_contract() {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("../artifacts/contracts/CAPE.sol/CAPE"),
            (),
        )
        .await
        .unwrap();

        let contract = CAPE::new(contract.address(), client);

        // Create two transactions
        let mut prng = ark_std::test_rng();
        let notes = create_anon_xfr_2in_3out(&mut prng, 2);

        // Convert the AAP transactions into some solidity friendly representation
        let mut solidity_notes = vec![];
        for note in notes {
            let solidity_note = to_solidity(&note);
            solidity_notes.push(solidity_note.clone());
        }

        // For now the block is simply the vector of "solidity" notes
        let block = solidity_notes;

        // Create a dummy frontier
        let frontier = vec![];

        // Create dummy records openings arrary
        let records_openings = vec![];

        // Submit to the contract
        let _receipt = contract
            .submit_cape_block(block, frontier, records_openings)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap()
            .expect("Failed to get tx receipt");

        // Check that the nullifiers have been inserted into the contract hashmap

        // let is_nullifier_inserted: bool = contract
        //   .has_nullifier_already_been_published(nullifier)
        //   .call()
        //   .await
        //   .unwrap()
        //   .into();
        //
        // assert!(is_nullifier_inserted);
    }
}
