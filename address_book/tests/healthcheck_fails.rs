// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use address_book::{
    address_book_port, address_book_temp_dir, init_web_server, wait_for_server, FileStore,
};

// This test has its own file because the address book port is currently not
// configurable so we can't start more than one server concurrently.
#[async_std::test]
async fn test_healthcheck_fails() {
    let healthcheck_url = format!("http://127.0.0.1:{}/healthcheck", address_book_port());

    let non_existent_temp_dir = address_book_temp_dir().path().join("dummy").to_path_buf();
    let store = FileStore::new(non_existent_temp_dir);
    init_web_server(store).await.expect("Failed to run server.");
    wait_for_server().await;
    let response = surf::get(&healthcheck_url).await.unwrap();

    assert_eq!(response.status(), tide::StatusCode::InternalServerError);
}
