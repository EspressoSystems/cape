use cap_rust_sandbox::deploy::deploy_greeter_contract;

#[tokio::test]
async fn test_basic_contract_deployment() {
    let contract = deploy_greeter_contract().await.unwrap();
    let res: String = contract.greet().call().await.unwrap().into();
    assert_eq!(res, "Initial Greeting")
}

#[tokio::test]
async fn test_basic_contract_transaction() {
    let contract = deploy_greeter_contract().await.unwrap();
    let _receipt = contract
        .set_greeting("Hi!".to_string())
        .legacy()
        .send()
        .await
        .unwrap()
        .await
        .unwrap()
        .expect("Failed to get TX receipt");

    let res: String = contract.greet().call().await.unwrap();
    assert_eq!(res, "Hi!");
}
