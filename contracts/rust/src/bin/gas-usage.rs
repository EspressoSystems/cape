// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! This executable deploys a CAPE contract, submits a block of transactions and prints the corresponding ethereum gas used.

use anyhow::Result;
use cap_rust_sandbox::{
    cape::CapeBlock,
    deploy::deploy_test_cape,
    ledger::CapeLedger,
    test_utils::PrintGas,
    types::{GenericInto, MerkleRootSol},
};
use jf_cap::{keys::UserPubKey, utils::TxnsParams};
use reef::Ledger;

#[tokio::main]
async fn main() -> Result<()> {
    let rng = &mut ark_std::test_rng();

    // Define how many transaction of each type are generated
    for (n_transfer, n_mint, n_freeze) in [
        (0, 0, 0),
        (1, 0, 0),
        (2, 0, 0),
        (0, 1, 0),
        (0, 2, 0),
        (0, 0, 1),
        (0, 0, 2),
    ] {
        let contract = deploy_test_cape().await;

        // Slow to run this each time
        let params = TxnsParams::generate_txns(
            rng,
            n_transfer,
            n_mint,
            n_freeze,
            CapeLedger::merkle_height(),
        );
        let miner = UserPubKey::default();

        if !params.txns.is_empty() {
            let root = params.txns[0].merkle_root();
            contract
                .add_root(root.generic_into::<MerkleRootSol>().0)
                .send()
                .await?
                .await?;
        }

        // Build the block from the list of transactions
        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

        // Submit the block to the CAPE contract
        contract
            .submit_cape_block(cape_block.into())
            .send()
            .await?
            .await?
            .print_gas(&format!(
                "transfer={} mint={} freeze={}",
                n_transfer, n_mint, n_freeze
            ));
    }

    Ok(())
}
