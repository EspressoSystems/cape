// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Type definitions for UI-focused API responses.

use jf_cap::{
    keys::{AuditorPubKey, FreezerPubKey, UserPubKey},
    structs::AssetCode,
};
use net::UserAddress;
use seahorse::{txn_builder::RecordInfo, AssetInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
/// Public keys for spending, viewing and freezing assets.
pub enum PubKey {
    Sending(UserPubKey),
    Viewing(AuditorPubKey),
    Freezing(FreezerPubKey),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceInfo {
    /// The balance of a single asset, in a single account.
    Balance(u64),
    /// All the balances of an account, by asset type.
    AccountBalances(HashMap<AssetCode, u64>),
    /// All the balances of all accounts owned by the wallet.
    AllBalances(HashMap<UserAddress, HashMap<AssetCode, u64>>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WalletSummary {
    pub addresses: Vec<UserAddress>,
    pub sending_keys: Vec<UserPubKey>,
    pub viewing_keys: Vec<AuditorPubKey>,
    pub freezing_keys: Vec<FreezerPubKey>,
    pub assets: Vec<AssetInfo>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub address: UserAddress,
    pub asset: AssetCode,
    pub amount: u64,
    pub uid: u64,
}

impl From<RecordInfo> for Record {
    fn from(record: RecordInfo) -> Self {
        Self {
            address: record.ro.pub_key.address().into(),
            asset: record.ro.asset_def.code,
            amount: record.ro.amount,
            uid: record.uid,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub records: Vec<Record>,
    pub assets: HashMap<AssetCode, AssetInfo>,
}
