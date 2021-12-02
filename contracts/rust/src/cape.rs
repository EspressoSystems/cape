#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::types::{
        Array, AuditMemo, AuxInfo, EncKey, GroupProjective, ReadCAPTx, TransferNote,
        TransferValidityProof,
    };

    use ethers::prelude::U256;

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

        // Create two transactions

        // Convert the AAP transactions into some solidity friendly representation

        // Create a block to be submitted to the contract

        // Create a dummy frontier

        // Submit to the contract

        // Check that the nullifiers have been inserted into the contract hashmap

        // let _receipt = contract
        //     .submit_transfer_note(transfer_note)
        //     .legacy()
        //     .send()
        //     .await
        //     .unwrap()
        //     .await
        //     .unwrap()
        //     .expect("Failed to get tx receipt");

        // let read_sentinel = contract.scratch().call().await.unwrap();
        // println!("Gas used {}", _receipt.gas_used.unwrap());
        // println!("Sentinel {}\n", read_sentinel);
        // assert_eq!(read_sentinel, sentinel);
    }
}
