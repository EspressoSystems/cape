// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use jf_cap::keys::UserKeyPair;

fn main() {
    let rng = &mut ark_std::test_rng();
    let faucet_manager = UserKeyPair::generate(rng);
    let public_key_json = serde_json::to_string(&faucet_manager.pub_key());
    println!("**************** CAUTION: this key generation executable is only for testing purposes. ****************");
    println!("Public key {:?}", public_key_json);

    let private_key_json = serde_json::to_string(&faucet_manager);
    println!("Private key {:?}", private_key_json);

    // Show how to read back the user private key from its serialization
    let private_key_from_json: UserKeyPair =
        serde_json::from_str(&private_key_json.unwrap()).unwrap();
    assert!(private_key_from_json.pub_key() == faucet_manager.pub_key());
}
