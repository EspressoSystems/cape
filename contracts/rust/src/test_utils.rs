use crate::types::{field_to_u256, SimpleToken, TestCAPE};
use ethers::prelude::{k256::ecdsa::SigningKey, Http, Provider, SignerMiddleware, Wallet, H160};
use jf_aap::keys::UserKeyPair;
use jf_aap::structs::{AssetDefinition, FreezeFlag, RecordCommitment, RecordOpening};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct ContractsInfo {
    pub cape_contract: TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    pub erc20_token_contract: SimpleToken<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    pub cape_contract_for_erc20_owner:
        TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    pub erc20_token_address: H160,
    pub owner_of_erc20_tokens_client: SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
    pub owner_of_erc20_tokens_client_address: H160,
}

// TODO try to parametrize the struct with the trait M:Middleware
impl ContractsInfo {
    pub async fn new(
        cape_contract_ref: &TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
        erc20_token_contract_ref: &SimpleToken<
            SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
        >,
    ) -> Self {
        let cape_contract = cape_contract_ref.clone();
        let erc20_token_contract = erc20_token_contract_ref.clone();

        let erc20_token_address = erc20_token_contract.address();
        let owner_of_erc20_tokens_client = erc20_token_contract.client().clone();
        let owner_of_erc20_tokens_client_address = owner_of_erc20_tokens_client.address();

        let cape_contract_for_erc20_owner = TestCAPE::new(
            cape_contract_ref.address(),
            Arc::from(owner_of_erc20_tokens_client.clone()),
        );

        Self {
            cape_contract,
            erc20_token_contract,
            cape_contract_for_erc20_owner,
            erc20_token_address,
            owner_of_erc20_tokens_client,
            owner_of_erc20_tokens_client_address,
        }
    }
}

/// Generates a user key pair that controls the faucet and call the contract for inserting a record commitment inside the merkle tree containing
/// some native fee asset records.
pub async fn create_faucet(
    contract: &TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
) -> (UserKeyPair, RecordOpening) {
    let mut rng = ChaChaRng::from_seed([42; 32]);
    let faucet_key_pair = UserKeyPair::generate(&mut rng);
    let faucet_rec = RecordOpening::new(
        &mut rng,
        u64::MAX / 2,
        AssetDefinition::native(),
        faucet_key_pair.pub_key(),
        FreezeFlag::Unfrozen,
    );
    let faucet_comm = RecordCommitment::from(&faucet_rec);
    contract
        .set_initial_record_commitments(vec![field_to_u256(faucet_comm.to_field_element())])
        .send()
        .await
        .unwrap()
        .await
        .unwrap();
    assert_eq!(contract.get_num_leaves().call().await.unwrap(), 1.into());

    (faucet_key_pair, faucet_rec)
}

pub fn contract_abi_path(contract_name: &str) -> PathBuf {
    [
        &PathBuf::from(env!("CONTRACTS_DIR")),
        Path::new("abi/contracts"),
        Path::new(&contract_name),
    ]
    .iter()
    .collect::<PathBuf>()
}
