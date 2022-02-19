use address_book::{
    init_web_server, override_port_from_env, wait_for_server, InsertPubKey, ServerOpt,
};
use jf_cap::keys::{UserKeyPair, UserPubKey};
use rand_chacha::rand_core::SeedableRng;
use structopt::StructOpt;
use tide::log::LevelFilter;

const ROUND_TRIP_COUNT: u64 = 100;
const NOT_FOUND_COUNT: u64 = 100;

// Test
//    lookup(insert(x)) = x
// and
//    lookup(y) = Err, if y has not been previously inserted.
//
#[async_std::test]
async fn round_trip() {
    // TODO !corbett find an unused port rather than assuming 50078 is free.
    let base_url = &ServerOpt::from_args().base_url[..];
    init_web_server(LevelFilter::Error, &base_url, None)
        .await
        .expect("Failed to run server.");
    let url = override_port_from_env(&base_url);
    wait_for_server(&url).await;

    let mut rng = rand_chacha::ChaChaRng::from_seed([0u8; 32]);
    let mut rng2 = rand_chacha::ChaChaRng::from_seed([0u8; 32]);

    let insert_pubkey = format!("{}/insert_pubkey", &url);
    let request_pubkey = format!("{}/request_pubkey", &url);

    // Insert and lookup a bunch of address/key pairs.
    for _ in 0..ROUND_TRIP_COUNT {
        let user_key = UserKeyPair::generate(&mut rng);
        let pub_key = user_key.pub_key();
        let pub_key_bytes = bincode::serialize(&pub_key).unwrap();
        let sig = user_key.sign(&pub_key_bytes);
        let json_request = InsertPubKey { pub_key_bytes, sig };
        let _response = surf::post(&insert_pubkey)
            .content_type(surf::http::mime::JSON)
            .body_json(&json_request)
            .unwrap()
            .await
            .unwrap();
        let address_bytes = bincode::serialize(&pub_key.address()).unwrap();
        let mut response = surf::post(&request_pubkey)
            .content_type(surf::http::mime::BYTE_STREAM)
            .body_bytes(&address_bytes)
            .await
            .unwrap();
        let bytes = response.body_bytes().await.unwrap();
        let gotten_pub_key: UserPubKey = bincode::deserialize(&bytes).unwrap();
        assert_eq!(gotten_pub_key, pub_key);
    }

    // Lookup the addresses just inserted to demonstrate that all the keys
    // are still present after the lookups.
    for _ in 0..ROUND_TRIP_COUNT {
        let user_key = UserKeyPair::generate(&mut rng2);
        let pub_key = user_key.pub_key();
        let address_bytes = bincode::serialize(&pub_key.address()).unwrap();
        let mut response = surf::post(&request_pubkey)
            .content_type(surf::http::mime::BYTE_STREAM)
            .body_bytes(&address_bytes)
            .await
            .unwrap();
        let bytes = response.body_bytes().await.unwrap();
        let gotten_pub_key: UserPubKey = bincode::deserialize(&bytes).unwrap();
        assert_eq!(gotten_pub_key, pub_key);
    }

    // Lookup addresses we didn't insert.
    for _ in 0..NOT_FOUND_COUNT {
        let user_key = UserKeyPair::generate(&mut rng2);
        let pub_key = user_key.pub_key();
        let address_bytes = bincode::serialize(&pub_key.address()).unwrap();
        let mut response = surf::post(&request_pubkey)
            .content_type(surf::http::mime::BYTE_STREAM)
            .body_bytes(&address_bytes)
            .await
            .unwrap();
        let bytes = response.body_bytes().await.unwrap();
        assert!(bincode::deserialize::<UserPubKey>(&bytes).is_err());
    }
}
