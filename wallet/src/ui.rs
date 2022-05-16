// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Type definitions for UI-focused API responses.

use crate::wallet::{CapeWallet, CapeWalletBackend, CapeWalletExt};
use cap_rust_sandbox::ledger::{CapeLedger, CapeTransactionKind};
use cap_rust_sandbox::model::Erc20Code;
use espresso_macros::ser_test;
use ethers::prelude::Address;
use futures::stream::{iter, StreamExt};
use jf_cap::{
    keys::{AuditorKeyPair, AuditorPubKey, FreezerKeyPair, FreezerPubKey, UserKeyPair, UserPubKey},
    structs::{AssetCode, AssetDefinition as JfAssetDefinition, AssetPolicy as JfAssetPolicy},
};
use net::UserAddress;
use reef::cap;
use seahorse::{
    accounts::{AccountInfo, KeyPair},
    asset_library::Icon,
    events::EventIndex,
    txn_builder::RecordInfo,
    MintInfo,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::io::Cursor;
use std::iter::empty;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tagged_base64::TaggedBase64;

/// UI-friendly asset definition.
#[ser_test(ark(false))]
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AssetDefinition {
    pub code: AssetCode,

    /// Asset policy attributes.
    pub freezing_key: Option<FreezerPubKey>,
    pub viewing_key: Option<AuditorPubKey>,
    pub address_viewable: bool,
    pub amount_viewable: bool,
    pub blind_viewable: bool,
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
            blind_viewable: policy.is_blinding_factor_revealed(),
            viewing_threshold: policy.reveal_threshold(),
        }
    }
}

impl From<AssetDefinition> for JfAssetDefinition {
    fn from(definition: AssetDefinition) -> JfAssetDefinition {
        let code = definition.code;
        if code == AssetCode::native() {
            return JfAssetDefinition::native();
        }

        let mut policy = JfAssetPolicy::default();
        if let Some(freezing_key) = definition.freezing_key {
            policy = policy.set_freezer_pub_key(freezing_key);
        }
        if let Some(viewing_key) = definition.viewing_key {
            policy = policy.set_auditor_pub_key(viewing_key);
            if definition.address_viewable {
                policy = policy
                    .reveal_user_address()
                    .expect("Failed to set reveal user address on asset policy");
            }
            if definition.amount_viewable {
                policy = policy
                    .reveal_amount()
                    .expect("Failed to set reveal amount on asset policy");
            }
            if definition.blind_viewable {
                policy = policy
                    .reveal_blinding_factor()
                    .expect("Failed to set reveal amount on asset policy");
            }
            policy = policy.set_reveal_threshold(definition.viewing_threshold);
        }
        JfAssetDefinition::new(code, policy).expect("Failed to create Asset Definition")
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
        // "freezing key", "viewing key", "address viewable", "amount viewable", "blind viewable",
        // and "viewing threshold".
        let mut code = None;
        let mut freezing_key = None;
        let mut viewing_key = None;
        let mut address_viewable = false;
        let mut amount_viewable = false;
        let mut blind_viewable = false;
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
                "blind_viewable" => {
                    blind_viewable = value
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
            blind_viewable,
            viewing_threshold,
        })
    }
}

/// UI-friendly details about an asset type.
#[ser_test(ark(false))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetInfo {
    pub definition: AssetDefinition,
    pub mint_info: Option<MintInfo>,
    pub verified: bool,

    /// Human-readable asset name.
    pub symbol: Option<String>,
    /// Human-readable asset description.
    pub description: Option<String>,
    /// Base64-encoded PNG icon.
    pub icon: Option<String>,
    /// The ERC-20 token address that this asset wraps, if this is a wrapped asset.
    pub wrapped_erc20: Option<String>,
}

impl AssetInfo {
    pub fn new(info: seahorse::AssetInfo, wrapped_erc20: Option<Erc20Code>) -> Self {
        let icon = info.icon.map(|icon| {
            let mut bytes = Cursor::new(vec![]);
            icon.write_png(&mut bytes)
                .expect("Failed getting icon bytes");
            base64::encode(&bytes.into_inner())
        });
        Self {
            definition: info.definition.into(),
            mint_info: info.mint_info,
            verified: info.verified,
            symbol: info.name,
            description: info.description,
            icon,
            wrapped_erc20: wrapped_erc20.map(|code| format!("{:#x}", Address::from(code))),
        }
    }

    pub async fn from_code<'a, Backend: CapeWalletBackend<'a> + Sync + 'a>(
        wallet: &CapeWallet<'a, Backend>,
        code: AssetCode,
    ) -> Option<Self> {
        let asset = wallet.asset(code).await?;
        Some(Self::from_info(wallet, asset).await)
    }

    pub async fn from_info<'a, Backend: CapeWalletBackend<'a> + Sync + 'a>(
        wallet: &CapeWallet<'a, Backend>,
        info: seahorse::AssetInfo,
    ) -> Self {
        let wrapped_erc20 = wallet.wrapped_erc20(info.definition.code).await;
        Self::new(info, wrapped_erc20)
    }

    /// Details about the native asset type.
    pub fn native() -> Self {
        Self::new(seahorse::AssetInfo::native::<CapeLedger>(), None)
    }
}

impl From<AssetInfo> for seahorse::AssetInfo {
    fn from(info: AssetInfo) -> Self {
        let icon = info.icon.map(|b64| {
            let bytes = base64::decode(&b64).expect("Failed to decode asset icon");
            Icon::load_png(Cursor::new(bytes.as_slice())).expect("Failed to load asset icon")
        });

        let mut asset = seahorse::AssetInfo::from(JfAssetDefinition::from(info.definition));
        asset.mint_info = info.mint_info;
        asset.name = info.symbol;
        asset.description = info.description;
        asset.icon = icon;
        asset
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateAsset {
    pub symbol: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
/// Public keys for spending, viewing and freezing assets.
pub enum PubKey {
    Sending(UserPubKey),
    Viewing(AuditorPubKey),
    Freezing(FreezerPubKey),
}

impl Display for PubKey {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Sending(k) => k.fmt(f),
            Self::Viewing(k) => k.fmt(f),
            Self::Freezing(k) => k.fmt(f),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParsePubKeyError;

impl FromStr for PubKey {
    type Err = ParsePubKeyError;

    fn from_str(s: &str) -> Result<Self, ParsePubKeyError> {
        if let Ok(k) = UserPubKey::from_str(s) {
            return Ok(Self::Sending(k));
        }
        if let Ok(k) = AuditorPubKey::from_str(s) {
            return Ok(Self::Viewing(k));
        }
        if let Ok(k) = FreezerPubKey::from_str(s) {
            return Ok(Self::Freezing(k));
        }
        Err(ParsePubKeyError)
    }
}

#[derive(Debug, Deserialize, Serialize)]
/// Private keys for spending, viewing and freezing assets.
pub enum PrivateKey {
    Sending(UserKeyPair),
    Viewing(AuditorKeyPair),
    Freezing(FreezerKeyPair),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Balances {
    /// The balance of a single asset, in a single account.
    One(u64),
    /// All the balances of an account, by asset type with asset info.
    Account(HashMap<AssetCode, u64>),
    /// All the balances of all accounts owned by the wallet.
    All {
        by_account: HashMap<UserAddress, HashMap<AssetCode, u64>>,
        aggregate: HashMap<AssetCode, u64>,
    },
}

impl Balances {
    pub fn assets(&self) -> Box<dyn Iterator<Item = &AssetCode> + Send + '_> {
        match self {
            Self::One(_) => Box::new(empty()),
            Self::Account(by_asset) => Box::new(by_asset.keys()),
            Self::All {
                by_account,
                aggregate,
            } => Box::new(
                by_account
                    .values()
                    .flat_map(|by_asset| by_asset.keys())
                    .chain(aggregate.keys()),
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BalanceInfo {
    pub balances: Balances,
    pub assets: HashMap<AssetCode, AssetInfo>,
}

#[ser_test(ark(false))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WalletSummary {
    pub addresses: Vec<UserAddress>,
    pub sending_keys: Vec<UserPubKey>,
    pub viewing_keys: Vec<AuditorPubKey>,
    pub freezing_keys: Vec<FreezerPubKey>,
    pub assets: Vec<AssetInfo>,
    /// The time (as an event index) at which the wallet last synced with the EQS.
    pub sync_time: usize,
    /// The real-world time (as an event index) according to the EQS.
    pub real_time: usize,
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

pub trait Ui {
    type UiAdaptor: Display;
    fn ui(self) -> Self::UiAdaptor;
}

impl Ui for jf_cap::keys::UserAddress {
    type UiAdaptor = UserAddress;

    fn ui(self) -> UserAddress {
        self.into()
    }
}

impl Ui for AuditorPubKey {
    type UiAdaptor = Self;

    fn ui(self) -> Self {
        self
    }
}

impl Ui for FreezerPubKey {
    type UiAdaptor = Self;

    fn ui(self) -> Self {
        self
    }
}

#[ser_test(ark(false))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub pub_key: String,
    pub records: Vec<Record>,
    pub balances: HashMap<AssetCode, u64>,
    pub assets: HashMap<AssetCode, AssetInfo>,
    pub description: String,
    pub used: bool,
    /// The status of a ledger scan for this account's key.
    ///
    /// If a ledger scan using this account's key is in progress, `scan_index` is the index of the
    /// next event to be scanned.
    pub scan_index: Option<EventIndex>,
    /// The ending index of a ledger scan for this account's key.
    ///
    /// If a ledger scan using this account's key is in progress, `scan_last_discoverable_event` is
    /// the index of the last event in the scan's range of interest. Note that
    /// `scan_last_discoverable_event` may be less than `scan_index`, since the scan will not
    /// complete until it has caught up with the main event loop, which may have advanced past
    /// `scan_last_discoverable_event`.
    pub scan_last_discoverable_event: Option<EventIndex>,
}

impl Account {
    pub async fn from_info<'a, Key: KeyPair, Backend: CapeWalletBackend<'a> + Sync + 'a>(
        wallet: &CapeWallet<'a, Backend>,
        info: AccountInfo<Key>,
    ) -> Self
    where
        Key::PubKey: Ui,
    {
        let assets = iter(info.assets)
            .then(|asset| async {
                (
                    asset.definition.code,
                    AssetInfo::from_info(wallet, asset).await,
                )
            })
            .collect::<HashMap<_, _>>()
            .await;
        let (scan_index, scan_last_discoverable_event) = match info.scan_status {
            Some((scan_index, scan_last_discoverable_event)) => {
                (Some(scan_index), Some(scan_last_discoverable_event))
            }
            None => (None, None),
        };
        Self {
            pub_key: info.address.ui().to_string(),
            records: info.records.into_iter().map(|rec| rec.into()).collect(),
            assets,
            balances: info.balances,
            description: info.description,
            used: info.used,
            scan_index,
            scan_last_discoverable_event,
        }
    }
}

#[ser_test(ark(false))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TransactionHistoryEntry {
    pub time: String,
    pub asset: AssetCode,
    pub kind: String,
    // String representation of the TaggedBase64 encoded hash
    pub hash: Option<String>,
    /// Sending keys used to build this transaction, if available.
    ///
    /// If we sent this transaction, `senders` records the addresses of the spending keys used to
    /// submit it. If we received this transaction from someone else, we may not know who the
    /// senders are and this field may be empty.
    pub senders: Vec<UserAddress>,
    /// Receivers and corresponding amounts.
    pub receivers: Vec<(UserAddress, u64)>,
    pub status: String,
}

impl TransactionHistoryEntry {
    pub async fn from_wallet<'a, Backend: CapeWalletBackend<'a> + Sync + 'a>(
        wallet: &CapeWallet<'a, Backend>,
        entry: seahorse::txn_builder::TransactionHistoryEntry<CapeLedger>,
    ) -> Self {
        Self {
            time: entry.time.to_string(),
            asset: entry.asset,
            kind: match entry.kind {
                CapeTransactionKind::CAP(cap::TransactionKind::Send) => "send".to_string(),
                CapeTransactionKind::CAP(cap::TransactionKind::Receive) => "receive".to_string(),
                CapeTransactionKind::CAP(cap::TransactionKind::Mint) => "mint".to_string(),
                CapeTransactionKind::CAP(cap::TransactionKind::Freeze) => "freeze".to_string(),
                CapeTransactionKind::CAP(cap::TransactionKind::Unfreeze) => "unfreeze".to_string(),
                CapeTransactionKind::CAP(cap::TransactionKind::Unknown) => "unknown".to_string(),
                CapeTransactionKind::Burn => "burn".to_string(),
                CapeTransactionKind::Wrap => "wrap".to_string(),
                CapeTransactionKind::Faucet => "faucet".to_string(),
            },
            hash: {
                entry.hash.and_then(|hash| {
                    bincode::serialize(&hash).ok().and_then(|bytes| {
                        TaggedBase64::new("HASH", &bytes)
                            .ok()
                            .map(|tb| tb.to_string())
                    })
                })
            },
            senders: entry.senders.into_iter().map(UserAddress::from).collect(),
            receivers: entry
                .receivers
                .into_iter()
                .map(|(addr, amt)| (addr.into(), amt))
                .collect(),
            status: match entry.receipt {
                Some(receipt) => match wallet.transaction_status(&receipt).await {
                    Ok(status) => status.to_string(),
                    Err(_) => "unknown".to_string(),
                },
                None => "accepted".to_string(),
            },
        }
    }
}

#[ser_test(ark(false))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyStoreLocation {
    pub path: PathBuf,
    pub name: Option<String>,
}

impl From<PathBuf> for KeyStoreLocation {
    fn from(path: PathBuf) -> Self {
        // Decode the name from TaggedBase64; if it fails to decode or has the wrong tag, then this
        // is not a named keystore file.
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| TaggedBase64::parse(name).ok())
            .and_then(|tb64| {
                if tb64.tag() == "KEYSTORE" {
                    std::str::from_utf8(&tb64.value())
                        .ok()
                        .map(|s| s.to_owned())
                } else {
                    None
                }
            });
        Self { path, name }
    }
}

impl From<&Path> for KeyStoreLocation {
    fn from(path: &Path) -> Self {
        path.to_owned().into()
    }
}

/// Solidity types, serialized as JSON in a MetaMask-compatible format.
pub mod sol {
    use super::*;
    use cap_rust_sandbox::types;
    use jf_cap::structs::RecordOpening as JfRecordOpening;

    // Primitive types like big integers and addresses just get serialized as hex strings.
    #[ser_test(ark(false))]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(into = "String", try_from = "String")]
    pub struct U256(ethers::prelude::U256);

    impl From<ethers::prelude::U256> for U256 {
        fn from(x: ethers::prelude::U256) -> Self {
            Self(x)
        }
    }

    impl From<U256> for ethers::prelude::U256 {
        fn from(x: U256) -> Self {
            x.0
        }
    }

    impl From<U256> for String {
        fn from(x: U256) -> Self {
            format!("{:#x}", x.0)
        }
    }

    impl TryFrom<String> for U256 {
        type Error = <ethers::prelude::U256 as FromStr>::Err;

        fn try_from(s: String) -> Result<Self, Self::Error> {
            Ok(Self(s.parse()?))
        }
    }

    impl From<U256> for AssetCode {
        fn from(x: U256) -> Self {
            types::AssetCodeSol(x.into()).into()
        }
    }

    impl From<AssetCode> for U256 {
        fn from(x: AssetCode) -> Self {
            types::AssetCodeSol::from(x).0.into()
        }
    }

    #[ser_test(ark(false))]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(into = "String", try_from = "String")]
    pub struct Address(ethers::prelude::Address);

    impl From<ethers::prelude::Address> for Address {
        fn from(x: ethers::prelude::Address) -> Self {
            Self(x)
        }
    }

    impl From<Address> for ethers::prelude::Address {
        fn from(x: Address) -> Self {
            x.0
        }
    }

    impl From<Address> for String {
        fn from(x: Address) -> Self {
            format!("{:#x}", x.0)
        }
    }

    impl TryFrom<String> for Address {
        type Error = <ethers::prelude::Address as FromStr>::Err;

        fn try_from(s: String) -> Result<Self, Self::Error> {
            Ok(Self(s.parse()?))
        }
    }

    #[ser_test(ark(false))]
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    pub struct EdOnBN254Point {
        pub x: U256,
        pub y: U256,
    }

    impl From<types::EdOnBN254Point> for EdOnBN254Point {
        fn from(p: types::EdOnBN254Point) -> Self {
            Self {
                x: p.x.into(),
                y: p.y.into(),
            }
        }
    }

    impl From<EdOnBN254Point> for types::EdOnBN254Point {
        fn from(p: EdOnBN254Point) -> Self {
            Self {
                x: p.x.into(),
                y: p.y.into(),
            }
        }
    }

    impl From<EdOnBN254Point> for AuditorPubKey {
        fn from(p: EdOnBN254Point) -> Self {
            types::EdOnBN254Point::from(p).into()
        }
    }

    impl From<AuditorPubKey> for EdOnBN254Point {
        fn from(p: AuditorPubKey) -> Self {
            types::EdOnBN254Point::from(p).into()
        }
    }

    impl From<EdOnBN254Point> for FreezerPubKey {
        fn from(p: EdOnBN254Point) -> Self {
            types::EdOnBN254Point::from(p).into()
        }
    }

    impl From<FreezerPubKey> for EdOnBN254Point {
        fn from(p: FreezerPubKey) -> Self {
            types::EdOnBN254Point::from(p).into()
        }
    }

    #[ser_test(ark(false))]
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    pub struct AssetDefinition {
        pub code: U256,
        pub policy: AssetPolicy,
    }

    impl From<types::AssetDefinition> for AssetDefinition {
        fn from(a: types::AssetDefinition) -> Self {
            Self {
                code: a.code.into(),
                policy: a.policy.into(),
            }
        }
    }

    impl From<AssetDefinition> for types::AssetDefinition {
        fn from(a: AssetDefinition) -> Self {
            Self {
                code: a.code.into(),
                policy: a.policy.into(),
            }
        }
    }

    impl From<JfAssetDefinition> for AssetDefinition {
        fn from(a: JfAssetDefinition) -> Self {
            types::AssetDefinition::from(a).into()
        }
    }

    impl From<AssetDefinition> for JfAssetDefinition {
        fn from(a: AssetDefinition) -> Self {
            types::AssetDefinition::from(a).into()
        }
    }

    impl From<super::AssetDefinition> for AssetDefinition {
        fn from(a: super::AssetDefinition) -> Self {
            JfAssetDefinition::from(a).into()
        }
    }

    impl From<AssetDefinition> for super::AssetDefinition {
        fn from(a: AssetDefinition) -> Self {
            JfAssetDefinition::from(a).into()
        }
    }

    #[ser_test(ark(false))]
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    pub struct AssetPolicy {
        pub auditor_pk: EdOnBN254Point,
        pub cred_pk: EdOnBN254Point,
        pub freezer_pk: EdOnBN254Point,
        pub reveal_map: U256,
        pub reveal_threshold: u64,
    }

    impl From<types::AssetPolicy> for AssetPolicy {
        fn from(p: types::AssetPolicy) -> Self {
            Self {
                auditor_pk: p.auditor_pk.into(),
                cred_pk: p.cred_pk.into(),
                freezer_pk: p.freezer_pk.into(),
                reveal_map: p.reveal_map.into(),
                reveal_threshold: p.reveal_threshold,
            }
        }
    }

    impl From<AssetPolicy> for types::AssetPolicy {
        fn from(p: AssetPolicy) -> Self {
            Self {
                auditor_pk: p.auditor_pk.into(),
                cred_pk: p.cred_pk.into(),
                freezer_pk: p.freezer_pk.into(),
                reveal_map: p.reveal_map.into(),
                reveal_threshold: p.reveal_threshold,
            }
        }
    }

    impl From<JfAssetPolicy> for AssetPolicy {
        fn from(p: JfAssetPolicy) -> Self {
            types::AssetPolicy::from(p).into()
        }
    }

    impl From<AssetPolicy> for JfAssetPolicy {
        fn from(p: AssetPolicy) -> Self {
            types::AssetPolicy::from(p).into()
        }
    }

    #[ser_test(ark(false))]
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    pub struct RecordOpening {
        pub amount: u64,
        pub asset_def: AssetDefinition,
        pub user_addr: EdOnBN254Point,
        pub enc_key: [u8; 32],
        pub freeze_flag: bool,
        pub blind: U256,
    }

    impl From<types::RecordOpening> for RecordOpening {
        fn from(r: types::RecordOpening) -> Self {
            Self {
                amount: r.amount,
                asset_def: r.asset_def.into(),
                user_addr: r.user_addr.into(),
                enc_key: r.enc_key,
                freeze_flag: r.freeze_flag,
                blind: r.blind.into(),
            }
        }
    }

    impl From<RecordOpening> for types::RecordOpening {
        fn from(r: RecordOpening) -> Self {
            Self {
                amount: r.amount,
                asset_def: r.asset_def.into(),
                user_addr: r.user_addr.into(),
                enc_key: r.enc_key,
                freeze_flag: r.freeze_flag,
                blind: r.blind.into(),
            }
        }
    }

    impl From<JfRecordOpening> for RecordOpening {
        fn from(r: JfRecordOpening) -> Self {
            types::RecordOpening::from(r).into()
        }
    }

    impl From<RecordOpening> for JfRecordOpening {
        fn from(r: RecordOpening) -> Self {
            types::RecordOpening::from(r).into()
        }
    }
}
