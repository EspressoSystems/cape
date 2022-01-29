#![cfg(test)]
use crate::deploy::deploy_queue_test;
use anyhow::Result;
use ethers::prelude::U256;

#[tokio::test]
async fn test_asset_pending_deposits_queue() -> Result<()> {
    let contract = deploy_queue_test().await;

    // At the beginning the queue is empty
    let is_queue_empty = contract.is_queue_empty().call().await?;
    assert!(is_queue_empty);

    // Check the queue size
    let queue_size = contract.get_queue_size().call().await?;
    assert_eq!(queue_size, U256::from(0));

    let queue_values = vec![U256::from(3), U256::from(7), U256::from(1), U256::from(23)];

    // We insert the values in the queue
    for v in queue_values.clone() {
        contract.push_to_queue(v).send().await?.await?;
    }

    // We check the queue is not empty
    let is_queue_empty = contract.is_queue_empty().call().await?;
    assert!(!is_queue_empty);

    // Check the queue size again
    let l = queue_values.len();
    let queue_size = contract.get_queue_size().call().await?;
    assert_eq!(queue_size, U256::from(l));

    // We get all the elements of the queue one by one
    let mut new_queue = vec![];
    for i in 0..l {
        let element = contract.get_queue_elem(U256::from(i)).call().await?;
        new_queue.push(element);
    }

    // We check the queue is still NOT empty
    let is_queue_empty = contract.is_queue_empty().call().await?;
    assert!(!is_queue_empty);

    // We check we got all the elements
    assert_eq!(queue_values, new_queue);

    // We empty the queue and check it is empty
    contract.empty_queue().send().await?.await?;
    assert!(contract.is_queue_empty().call().await?);

    Ok(())
}
