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
    async fn test_read_transfer_note_struct_in_contract() {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("../artifacts/contracts/ReadCAPTx.sol/ReadCAPTx"),
            (),
        )
        .await
        .unwrap();
        let contract = ReadCAPTx::new(contract.address(), client);

        let one = U256::one();
        let zero = U256::zero();

        let group_projective = GroupProjective {
            x: one,
            y: one,
            t: one,
            z: one,
        };

        let sentinel = U256::from(1337);
        let transfer_note = TransferNote {
            input_nullifiers: Array {
                items: vec![sentinel, zero],
            },
            output_commitments: Array { items: vec![one] },
            proof: TransferValidityProof { dummy: one },
            audit_memo: AuditMemo {
                ephemeral: EncKey {
                    key: group_projective.clone(),
                },
                data: Array {
                    items: vec![zero, one],
                },
            },
            aux_info: AuxInfo {
                merkle_root: one,
                fee: one,
                valid_until: one,
                txn_memo_ver_key: group_projective.clone(),
            },
        };

        let _receipt = contract
            .submit_transfer_note(transfer_note)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap()
            .expect("Failed to get tx receipt");

        let read_sentinel = contract.scratch().call().await.unwrap();
        println!("Gas used {}", _receipt.gas_used.unwrap());
        println!("Sentinel {}\n", read_sentinel);
        assert_eq!(read_sentinel, sentinel);
    }
}
