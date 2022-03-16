// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Type definitions for UI-focused API responses.

use jf_cap::{
    keys::{AuditorPubKey, FreezerPubKey, UserPubKey},
    structs::{AssetCode, AssetDefinition as JfAssetDefinition, AssetPolicy},
};
use net::UserAddress;
use seahorse::{
    accounts::{AccountInfo, KeyPair},
    txn_builder::RecordInfo,
    MintInfo,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

/// UI-friendly asset definition.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AssetDefinition {
    pub code: AssetCode,

    /// Asset policy attributes.
    pub freezing_key: Option<FreezerPubKey>,
    pub viewing_key: Option<AuditorPubKey>,
    pub address_viewable: bool,
    pub amount_viewable: bool,
    pub viewing_threshold: u64,
}

impl AssetDefinition {
    /// Return native asset definition.
    pub fn native() -> Self {
        AssetDefinition::from(JfAssetDefinition::native())
    }

    /// Return the dummy record asset definition.
    pub fn dummy() -> Self {
        AssetDefinition::from(JfAssetDefinition::dummy())
    }
}

impl From<JfAssetDefinition> for AssetDefinition {
    fn from(definition: JfAssetDefinition) -> Self {
        let policy = definition.policy_ref();
        Self {
            code: definition.code,
            // If the freezer public key is set, i.e., non-default,
            // include it in the asset definition.
            freezing_key: if policy.is_freezer_pub_key_set() {
                Some(policy.freezer_pub_key().clone())
            } else {
                None
            },
            // If the auditor public key is set, i.e., non-default,
            // include it in the asset definition.
            viewing_key: if policy.is_auditor_pub_key_set() {
                Some(policy.auditor_pub_key().clone())
            } else {
                None
            },
            address_viewable: policy.is_user_address_revealed(),
            amount_viewable: policy.is_amount_revealed(),
            viewing_threshold: policy.reveal_threshold(),
        }
    }
}

impl From<AssetDefinition> for JfAssetDefinition {
    fn from(definition: AssetDefinition) -> JfAssetDefinition {
        let code = definition.code;
        let mut policy = AssetPolicy::default();
        if let Some(freezing_key) = definition.freezing_key {
            policy = policy.set_freezer_pub_key(freezing_key);
        }
        if let Some(viewing_key) = definition.viewing_key {
            policy = policy.set_auditor_pub_key(viewing_key);
            if definition.address_viewable {
                policy = policy.reveal_user_address().unwrap();
            }
            if definition.amount_viewable {
                policy = policy.reveal_amount().unwrap();
            }
            policy = policy.set_reveal_threshold(definition.viewing_threshold);
        }
        JfAssetDefinition::new(code, policy).unwrap()
    }
}

impl Display for AssetDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "code:{}", self.code)?;
        if let Some(freezing_key) = &self.viewing_key {
            write!(f, ",freezing key:{}", freezing_key,)?;
        }
        if let Some(viewing_key) = &self.viewing_key {
            write!(f, ",viewing key:{}", viewing_key,)?;
            write!(f, ",address viewable:{}", self.address_viewable)?;
            write!(f, ",amount viewable:{}", self.amount_viewable)?;
            write!(f, ",viewing threshold:{}", self.viewing_threshold)?;
        }
        Ok(())
    }
}

impl FromStr for AssetDefinition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // This parse method is meant for a friendly, discoverable CLI interface. It parses a
        // comma-separated list of key-value pairs, like `address_viewable:true`. This allows the
        // fields to be specified in any order, or not at all. Recognized fields are "code",
        // "freezing key", "viewing key", "address viewable", "amount viewable", and "viewing
        // threshold".
        let mut code = None;
        let mut freezing_key = None;
        let mut viewing_key = None;
        let mut address_viewable = false;
        let mut amount_viewable = false;
        let mut viewing_threshold = 0;
        for kv in s.split(',') {
            let (key, value) = match kv.split_once(':') {
                Some(split) => split,
                None => return Err(format!("expected key:value pair, got {}", kv)),
            };
            match key {
                "code" => {
                    code = Some(
                        value
                            .parse()
                            .map_err(|_| format!("expected AssetCode, got {}", value))?,
                    )
                }
                "freezing_key" => {
                    freezing_key = Some(
                        value
                            .parse()
                            .map_err(|_| format!("expected FreezerPubKey, got {}", value))?,
                    )
                }
                "viewing_key" => {
                    viewing_key = Some(
                        value
                            .parse()
                            .map_err(|_| format!("expected AuditorPubKey, got {}", value))?,
                    )
                }
                "address_viewable" => {
                    address_viewable = value
                        .parse()
                        .map_err(|_| format!("expected bool, got {}", value))?;
                }
                "amount_viewable" => {
                    amount_viewable = value
                        .parse()
                        .map_err(|_| format!("expected bool, got {}", value))?;
                }
                "viewing_threshold" => {
                    viewing_threshold = value
                        .parse()
                        .map_err(|_| format!("expected u64, got {}", value))?;
                }
                _ => return Err(format!("unrecognized key {}", key)),
            }
        }

        let code = match code {
            Some(code) => code,
            None => return Err(String::from("must specify code")),
        };

        Ok(AssetDefinition {
            code,
            freezing_key,
            viewing_key,
            address_viewable,
            amount_viewable,
            viewing_threshold,
        })
    }
}

/// UI-friendly details about an asset type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssetInfo {
    pub definition: AssetDefinition,
    pub mint_info: Option<MintInfo>,
    pub verified: bool,
}

impl AssetInfo {
    pub fn new(definition: AssetDefinition, mint_info: MintInfo, verified: bool) -> Self {
        Self {
            definition,
            mint_info: Some(mint_info),
            verified,
        }
    }

    /// Details about the native asset type.
    pub fn native() -> Self {
        Self {
            definition: AssetDefinition::native(),
            mint_info: None,
            verified: false,
        }
    }
}

impl From<AssetDefinition> for AssetInfo {
    fn from(definition: AssetDefinition) -> Self {
        Self {
            definition,
            mint_info: None,
            verified: false,
        }
    }
}

impl From<seahorse::AssetInfo> for AssetInfo {
    fn from(asset_info: seahorse::AssetInfo) -> Self {
        Self {
            definition: AssetDefinition::from(asset_info.definition),
            mint_info: asset_info.mint_info,
            verified: asset_info.verified,
        }
    }
}

impl Display for AssetInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "definition:{}", self.definition)?;
        if let Some(mint_info) = &self.mint_info {
            write!(
                f,
                ",seed:{},description:{}",
                mint_info.seed,
                mint_info.fmt_description()
            )?;
        }
        write!(f, ",verified:{}", self.verified)?;
        Ok(())
    }
}

impl FromStr for AssetInfo {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // This parse method is meant for a friendly, discoverable CLI interface. It parses a
        // comma-separated list of key-value pairs, like `description:my_asset`. This allows the
        // fields to be specified in any order, or not at all. Recognized fields are "definition",
        // "seed", and "description".
        let mut definition = None;
        let mut seed = None;
        let mut description = None;
        for kv in s.split(',') {
            let (key, value) = match kv.split_once(':') {
                Some(split) => split,
                None => return Err(format!("expected key:value pair, got {}", kv)),
            };
            match key {
                "definition" => {
                    definition = Some(
                        value
                            .parse()
                            .map_err(|_| format!("expected AssetDefinition, got {}", value))?,
                    )
                }
                "seed" => {
                    seed = Some(
                        value
                            .parse()
                            .map_err(|_| format!("expected AssetCodeSeed, got {}", value))?,
                    )
                }
                "description" => description = Some(MintInfo::parse_description(value)),
                _ => return Err(format!("unrecognized key {}", key)),
            }
        }

        let definition = match definition {
            Some(definition) => definition,
            None => return Err(String::from("must specify definition")),
        };
        let mint_info = match (seed, description) {
            (Some(seed), Some(description)) => Some(MintInfo { seed, description }),
            (None, None) => None,
            _ => {
                return Err(String::from(
                    "seed and description must be specified together or not at all",
                ))
            }
        };

        Ok(AssetInfo {
            definition,
            mint_info,
            verified: false,
        })
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
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
    pub balance: u64,
    pub assets: HashMap<AssetCode, AssetInfo>,
    pub description: String,
    pub used: bool,
}

impl<Key: KeyPair> From<AccountInfo<Key>> for Account {
    fn from(info: AccountInfo<Key>) -> Self {
        Self {
            records: info.records.into_iter().map(|rec| rec.into()).collect(),
            assets: info
                .assets
                .into_iter()
                .map(|asset| (asset.definition.code, AssetInfo::from(asset)))
                .collect(),
            balance: info.balance,
            description: info.description,
            used: info.used,
        }
    }
}
