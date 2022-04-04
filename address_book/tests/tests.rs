// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use address_book::{
    address_book_port, address_book_temp_dir, init_web_server, wait_for_server, FileStore,
    InsertPubKey, Store, TransientFileStore,
};
use jf_cap::keys::{UserKeyPair, UserPubKey};
use rand_chacha::rand_core::SeedableRng;

const ROUND_TRIP_COUNT: u64 = 100;
const NOT_FOUND_COUNT: u64 = 100;

// Test
//    lookup(insert(x)) = x
// and
//    lookup(y) = Err, if y has not been previously inserted.
//
async fn round_trip<T: Store + 'static>(store: T) {
    // TODO !corbett find an unused port rather than assuming 50078 is free.
    init_web_server(store).await.expect("Failed to run server.");
    wait_for_server().await;

    let mut rng = rand_chacha::ChaChaRng::from_seed([0u8; 32]);
    let mut rng2 = rand_chacha::ChaChaRng::from_seed([0u8; 32]);

    // Insert and lookup a bunch of address/key pairs.
    for _ in 0..ROUND_TRIP_COUNT {
        let user_key = UserKeyPair::generate(&mut rng);
        let pub_key = user_key.pub_key();
        let pub_key_bytes = bincode::serialize(&pub_key).unwrap();
        let sig = user_key.sign(&pub_key_bytes);
        let json_request = InsertPubKey { pub_key_bytes, sig };
        let _response = surf::post(format!(
            "http://127.0.0.1:{}/insert_pubkey",
            address_book_port()
        ))
        .content_type(surf::http::mime::JSON)
        .body_json(&json_request)
        .unwrap()
        .await
        .unwrap();
        let address_bytes = bincode::serialize(&pub_key.address()).unwrap();
        let mut response = surf::post(format!(
            "http://127.0.0.1:{}/request_pubkey",
            address_book_port()
        ))
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
        let mut response = surf::post(format!(
            "http://127.0.0.1:{}/request_pubkey",
            address_book_port()
        ))
        .content_type(surf::http::mime::BYTE_STREAM)
        .body_bytes(&address_bytes)
        .await
        .unwrap();
        let bytes = response.body_bytes().await.unwrap();
        let gotten_pub_key: UserPubKey = bincode::deserialize(&bytes).unwrap();
        assert_eq!(gotten_pub_key, pub_key);
    }

    // Lookup addresses we didn't insert.
    tracing::error!(
        "The following {} 'No such' errors are expected.",
        NOT_FOUND_COUNT
    );
    for _ in 0..NOT_FOUND_COUNT {
        let user_key = UserKeyPair::generate(&mut rng2);
        let pub_key = user_key.pub_key();
        let address_bytes = bincode::serialize(&pub_key.address()).unwrap();
        let mut response = surf::post(format!(
            "http://127.0.0.1:{}/request_pubkey",
            address_book_port()
        ))
        .content_type(surf::http::mime::BYTE_STREAM)
        .body_bytes(&address_bytes)
        .await
        .unwrap();
        let bytes = response.body_bytes().await.unwrap();
        assert!(bincode::deserialize::<UserPubKey>(&bytes).is_err());
    }
}

#[async_std::test]
async fn test_address_book() {
    // Can change to using two separate tests once the webserver port is
    // configurable.
    let temp_dir = address_book_temp_dir();
    let store = FileStore::new(temp_dir.path().to_path_buf());
    round_trip(store).await;

    let store = TransientFileStore::default();
    round_trip(store).await
}
