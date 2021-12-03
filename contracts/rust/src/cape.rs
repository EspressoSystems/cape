use jf_txn::transfer::TransferNote;

use crate::helpers::convert_nullifier_to_u256;
use crate::types::CapeTransaction;
use itertools::Itertools;

#[allow(dead_code)]
/// Converts a TransferNote into a solidity friendly data structure that can be passed to the CAPE contract
fn to_solidity(note: &TransferNote) -> CapeTransaction {
    return CapeTransaction {
        inputs_nullifiers: note
            .inputs_nullifiers
            .clone()
            .iter()
            .map(|v| convert_nullifier_to_u256(v))
            .collect_vec(),
    };
}

#[cfg(test)]
mod tests {
    use crate::cap_jf::create_anon_xfr_2in_3out;
    use crate::cape::to_solidity;
    use crate::ethereum::{deploy, get_funded_deployer};
    use crate::helpers::convert_nullifier_to_u256;
    use crate::types::{AssetDefinition, CapeBlock, CAPE};
    use std::path::Path;

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
        for note in notes.clone() {
            let solidity_note = to_solidity(&note);
            solidity_notes.push(solidity_note.clone());
        }

        // For now the block is simply the vector of "solidity" notes
        // let block = solidity_notes;
        // let block = CapeBlock();

        // Create a dummy frontier
        // let frontier = vec![];

        // // Create dummy records openings arrary
        // let records_openings = vec![];

        // // Check that some nullifier is not yet inserted
        // let nullifier = convert_nullifier_to_u256(&notes[0].inputs_nullifiers[0]);
        // let is_nullifier_inserted: bool = contract
        //     .has_nullifier_already_been_published(nullifier)
        //     .call()
        //     .await
        //     .unwrap()
        //     .into();
        // assert!(!is_nullifier_inserted);

        // // Submit to the contract
        // let _receipt = contract
        //     .submit_cape_block(block, frontier, records_openings)
        //     .legacy()
        //     .send()
        //     .await
        //     .unwrap()
        //     .await
        //     .unwrap()
        //     .expect("Failed to get tx receipt");

        // // Check that now the nullifier has been inserted
        // let is_nullifier_inserted: bool = contract
        //     .has_nullifier_already_been_published(nullifier)
        //     .call()
        //     .await
        //     .unwrap()
        //     .into();

        // assert!(is_nullifier_inserted);
    }
}
