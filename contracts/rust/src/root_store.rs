#![cfg(test)]
use crate::assertion::Matcher;
use crate::deploy::deploy_test_root_store_contract;
use anyhow::Result;
use ethers::prelude::U256;

#[tokio::test]
async fn test_root_store() -> Result<()> {
    let contract = deploy_test_root_store_contract().await;

    let roots: Vec<U256> = (5..10).map(U256::from).collect();

    // the store is empty
    assert!(!contract.contains_root(roots[0]).call().await?);

    // check reverts if root not found
    contract
        .check_contains_root(roots[0])
        .call()
        .await
        .should_revert_with_message("Root not found");

    contract.add_root(roots[0]).send().await?.await?;

    assert!(contract.contains_root(roots[0]).call().await?);
    // check does not revert if root found
    assert!(contract.check_contains_root(roots[0]).call().await.is_ok());

    contract.add_root(roots[1]).send().await?.await?;

    assert!(contract.contains_root(roots[0]).call().await?);
    assert!(contract.contains_root(roots[1]).call().await?);

    contract.add_root(roots[2]).send().await?.await?;

    assert!(contract.contains_root(roots[0]).call().await?);
    assert!(contract.contains_root(roots[1]).call().await?);
    assert!(contract.contains_root(roots[2]).call().await?);

    contract.add_root(roots[3]).send().await?.await?;

    // first root should be removed
    assert!(!contract.contains_root(roots[0]).call().await?);

    // last three roots remain
    assert!(contract.contains_root(roots[1]).call().await?);
    assert!(contract.contains_root(roots[2]).call().await?);
    assert!(contract.contains_root(roots[3]).call().await?);

    Ok(())
}
