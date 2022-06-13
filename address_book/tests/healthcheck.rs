// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::fs;

use address_book::{
    address_book_port, address_book_temp_dir, init_web_server, wait_for_server, FileStore,
};

#[async_std::test]
async fn test_healthcheck() {
    let healthcheck_url = format!("http://127.0.0.1:{}/healthcheck", address_book_port());

    let temp_dir = address_book_temp_dir();
    let store = FileStore::new(temp_dir.path().to_path_buf());
    init_web_server(store).await.expect("Failed to run server.");
    wait_for_server().await;

    // Test healtheck OK
    let response = surf::get(&healthcheck_url).await.unwrap();
    assert_eq!(response.status(), tide::StatusCode::Ok);

    // Test healthcheck fails if the directory isn't writeable.
    let mut permissions = temp_dir.path().metadata().unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(temp_dir.path(), permissions.clone()).unwrap();

    let response = surf::get(&healthcheck_url).await.unwrap();

    // Make directory writeable again now so it can be removed if the assert
    // below fails.
    permissions.set_readonly(false);
    fs::set_permissions(temp_dir.path(), permissions).unwrap();

    assert_eq!(response.status(), tide::StatusCode::InternalServerError);
}
