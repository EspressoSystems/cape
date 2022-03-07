// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use address_book::{address_book_port, init_web_server, wait_for_server, InsertPubKey};
use jf_cap::keys::{UserKeyPair, UserPubKey};
use rand_chacha::rand_core::SeedableRng;
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
    init_web_server(LevelFilter::Error)
        .await
        .expect("Failed to run server.");
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
