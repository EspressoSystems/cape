use address_book::{init_web_server, InsertPubKey, DEFAULT_PORT};
use jf_aap::keys::{UserKeyPair, UserPubKey};
use rand_chacha::rand_core::SeedableRng;
use std::time::Duration;

const ROUND_TRIP_COUNT: u64 = 100;
const NOT_FOUND_COUNT: u64 = 100;
const ADDRESS_BOOK_STARTUP_RETRIES: usize = 8;

// Shamelessly copied from relayer/src/lib.rs
async fn wait_for_server(port: u16) {
    // Wait for the server to come up and start serving.
    let mut backoff = Duration::from_millis(100);
    for _ in 0..ADDRESS_BOOK_STARTUP_RETRIES {
        if surf::connect(format!("http://localhost:{}", port))
            .send()
            .await
            .is_ok()
        {
            return;
        }
        backoff *= 2;
        std::thread::sleep(backoff);
    }
    panic!("Address Book did not start in {:?} milliseconds", backoff);
}

// Test
//    lookup(insert(x)) = x
// and
//    lookup(y) = Err, if y has not been previously inserted.
//
#[async_std::test]
async fn round_trip() {
    // TODO !corbett find an unused port rather than assuming 50078 is free.
    init_web_server().await.expect("Failed to run server.");
    wait_for_server(DEFAULT_PORT).await;

    let mut rng = rand_chacha::ChaChaRng::from_seed([0u8; 32]);
    let mut rng2 = rand_chacha::ChaChaRng::from_seed([0u8; 32]);

    // Insert and lookup a bunch of address/key pairs.
    for _ in 0..ROUND_TRIP_COUNT {
        let user_key = UserKeyPair::generate(&mut rng);
        let pub_key = user_key.pub_key();
        let pub_key_bytes = bincode::serialize(&pub_key).unwrap();
        let sig = user_key.sign(&pub_key_bytes);
        let json_request = InsertPubKey { pub_key_bytes, sig };
        let _response = surf::post("http://127.0.0.1:50078/insert_pubkey")
            .content_type(surf::http::mime::JSON)
            .body_json(&json_request)
            .unwrap()
            .await
            .unwrap();
        let address_bytes = bincode::serialize(&pub_key.address()).unwrap();
        let mut response = surf::post("http://127.0.0.1:50078/request_pubkey")
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
        let mut response = surf::post("http://127.0.0.1:50078/request_pubkey")
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
        let mut response = surf::post("http://127.0.0.1:50078/request_pubkey")
            .content_type(surf::http::mime::BYTE_STREAM)
            .body_bytes(&address_bytes)
            .await
            .unwrap();
        let bytes = response.body_bytes().await.unwrap();
        assert!(bincode::deserialize::<UserPubKey>(&bytes).is_err());
    }
}
