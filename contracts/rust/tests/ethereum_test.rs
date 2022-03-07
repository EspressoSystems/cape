// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use anyhow::Result;
use cap_rust_sandbox::deploy::deploy_greeter_contract;

#[tokio::test]
async fn test_basic_contract_deployment() {
    let contract = deploy_greeter_contract().await.unwrap();
    let res: String = contract.greet().call().await.unwrap().into();
    assert_eq!(res, "Initial Greeting")
}

#[tokio::test]
async fn test_basic_contract_transaction() -> Result<()> {
    let contract = deploy_greeter_contract().await.unwrap();
    let _receipt = contract
        .set_greeting("Hi!".to_string())
        .send()
        .await?
        .await?;

    let res: String = contract.greet().call().await.unwrap();
    assert_eq!(res, "Hi!");
    Ok(())
}
