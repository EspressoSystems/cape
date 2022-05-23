// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use async_std::task::sleep;
use cap_rust_sandbox::universal_param::UNIVERSAL_PARAM;
use cape_wallet::{
    backend::{CapeBackend, CapeBackendConfig},
    CapeWallet, CapeWalletError, CapeWalletExt,
};
use ethers::{prelude::Address, providers::Middleware};
use jf_cap::{
    keys::{AuditorPubKey, FreezerPubKey, UserPubKey},
    structs::{AssetCode, AssetPolicy},
    KeyPair,
};
use rand::distributions::{Alphanumeric, DistString};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::{
    asset_library::{Icon, VerifiedAssetLibrary},
    hd::Mnemonic,
    loader::{Loader, LoaderMetadata},
    txn_builder::TransactionStatus,
    AssetInfo,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, stdout, BufReader, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::str;
use std::time::Duration;
use structopt::StructOpt;
use surf::{http::mime, StatusCode, Url};
use tempdir::TempDir;
use tracing::info;

/// Generate an official asset library for CAPE.
///
/// This program reads a TOML specification of a list of CAPE assets, creates those assets and
/// deploys them to a CAPE instance, and then generates a signed binary file containing the
/// resulting asset definitions and metadata.
///
/// The basic usage looks like
///     gen-official-asset-library --key-pair KEY --cape-mnemonic CAPE_MNEMONIC --eth-mnemonic ETH_MNEMONIC --assets ASSETS -o FILE
/// where
///     KEY is the signing key pair to use to sign the library
///     CAPE_MNEMONIC is the mnemonic for the CAPE wallet which will be used to create the assets.
///         This mnemonic can later be used to distributed the minted records of any domestic assets
///         in the library.
///     ETH_MNEMONIC is the mnemonic for the Ethereum wallet which will be used to sponsor the
///         wrapped assets in the library. The first address generated by this mnemonic must be
///         funded with ETH on the target chain, and it will be charged gas for all of the sponsor
///         operations required to create the asset library.     
///     ASSETS is the path to a TOML specification of the asset library to create
///     FILE is the binary asset library to create.
/// The user must also set the following environment variables to indicate the CAPE deployment to
/// which the new assets should be deployed:
///     CAPE_EQS_URL
///     CAPE_RELAYER_URL
///     CAPE_ADDRESS_BOOK_URL
///     CAPE_FAUCET_URL
///     CAPE_CONTRACT_ADDRESS
///     CAPE_WEB3_PROVIDER_URL
///
/// The format of the TOML specification is an array of assets, like
///     [[assets]]
///     # asset 1
///
///     [[assets]]
///     # asset 2
///
///     # etc.
/// Each asset has several required keys:
///     symbol = 'string'
///     description = 'string'
///     icon = 'path'
///     type = 'wrapped|domestic'
/// `icon` is a path to an image to use as the icon for this asset. It is interpreted relative to
/// the directory containing the TOML file. If `type` is `wrapped`, the specification must also
/// include the key:
///     contract = '0xaddress'
/// Otherwise, if `type` is `domestic`, the specification must also include the key:
///     amount = number
/// specifying the amount of the domestic asset to mint.
///
/// In addition to these required fields, each asset may have the following optional fields:
///     viewing_key = 'AUDPUBKEY~...'
///     freezing_key = 'FREEZEPUBKEY~...'
/// If the `viewing_key` is given, the created asset type will be fully viewable using that key. If
/// the `freezing_key` is given, the created asset will be freezable using that key. `viewing_key`
/// is required if `freezing_key` is to be used.
#[derive(StructOpt)]
struct Options {
    /// The signing key pair to use to authenticate the generated asset library.
    #[structopt(short, long, name = "KEY", env = "CAPE_ASSET_LIBRARY_SIGNING_KEY")]
    key_pair: KeyPair,

    /// The mnemonic phrase to generate the CAPE wallet used to generate the assets.
    ///
    /// This is the wallet which will receive the minted domestic assets, so this mnemonic can be
    /// used to distribute minted domestic assets.
    #[structopt(long, env = "CAPE_ASSET_LIBRARY_CAPE_MNEMONIC")]
    cape_mnemonic: Mnemonic,

    /// The mnemonic phrase to generate the Ethereum wallet used to deploy the assets.
    #[structopt(long, env = "CAPE_ASSET_LIBRARY_ETH_MNEMONIC")]
    eth_mnemonic: Mnemonic,

    /// The path of the library file to generate.
    ///
    /// If not provided, the library will be written to stdout.
    #[structopt(short = "o", long = "output", name = "FILE")]
    file: Option<PathBuf>,

    /// Path to a .toml file specifying the asset library to generate.
    #[structopt(short, long, name = "ASSETS")]
    assets: PathBuf,

    /// URL for the Ethereum Query Service.
    #[structopt(long, env = "CAPE_EQS_URL", default_value = "http://localhost:50087")]
    eqs_url: Url,

    /// URL for the CAPE relayer.
    #[structopt(
        long,
        env = "CAPE_RELAYER_URL",
        default_value = "http://localhost:50077"
    )]
    relayer_url: Url,

    /// URL for the Ethereum Query Service.
    #[structopt(
        long,
        env = "CAPE_ADDRESS_BOOK_URL",
        default_value = "http://localhost:50078"
    )]
    address_book_url: Url,

    /// URL for the CAPE faucet.
    #[structopt(
        long,
        env = "CAPE_FAUCET_URL",
        default_value = "http://localhost:50079"
    )]
    faucet_url: Url,

    /// Address of the CAPE smart contract.
    #[structopt(long, env = "CAPE_CONTRACT_ADDRESS")]
    contract_address: Address,

    /// URL for Ethers HTTP Provider
    #[structopt(long, env = "CAPE_WEB3_PROVIDER_URL")]
    rpc_url: Url,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AssetLibrarySpec {
    assets: Vec<Asset>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Asset {
    symbol: String,
    description: String,
    icon: PathBuf,
    viewing_key: Option<AuditorPubKey>,
    freezing_key: Option<FreezerPubKey>,
    #[serde(flatten)]
    kind: AssetKind,
}

impl Asset {
    async fn create<'a>(
        self,
        wallet: &mut CapeWallet<'a, CapeBackend<'a, LoaderMetadata>>,
        pub_key: &UserPubKey,
        icon_dir: &Path,
        faucet_url: &Url,
    ) -> Result<AssetInfo, CapeWalletError> {
        info!("creating asset {}", self.symbol);

        let mut policy = AssetPolicy::default();
        if let Some(viewing_key) = self.viewing_key {
            info!(
                "{} will be fully viewable with the key {}",
                self.symbol, viewing_key
            );
            policy = policy
                .set_auditor_pub_key(viewing_key)
                .reveal_record_opening()
                .unwrap();

            if let Some(freezing_key) = self.freezing_key {
                info!(
                    "{} will be freezable with the key {}",
                    self.symbol, freezing_key
                );
                policy = policy.set_freezer_pub_key(freezing_key);
            }
        }

        let asset = match self.kind {
            AssetKind::Domestic { amount } => {
                // If we don't have an CAPE assets to pay the fee, request some from the faucets.
                if wallet
                    .balance_breakdown(&pub_key.address(), &AssetCode::native())
                    .await
                    == 0u64.into()
                {
                    info!("requesting CAPE tokens from faucet {}", faucet_url);
                    let res = surf::post(faucet_url.join("request_fee_assets").unwrap())
                        .content_type(mime::JSON)
                        .body_json(&pub_key)
                        .unwrap()
                        .send()
                        .await
                        .map_err(|source| CapeWalletError::Failed {
                            msg: format!("faucet request failed: {}", source),
                        })?;
                    if res.status() != StatusCode::Ok {
                        return Err(CapeWalletError::Failed {
                            msg: "faucet request failed".into(),
                        });
                    }

                    // Wait for the assets to show up.
                    while wallet
                        .balance_breakdown(&pub_key.address(), &AssetCode::native())
                        .await
                        == 0u64.into()
                    {
                        sleep(Duration::from_secs(5)).await;
                    }
                }

                info!("minting {} of domestic asset {}", amount, self.symbol);
                let asset = wallet
                    .define_asset(self.symbol.clone(), self.symbol.as_bytes(), policy)
                    .await?;
                let txn = wallet
                    .mint(None, 0, &asset.code, amount, pub_key.address())
                    .await?;
                if wallet.await_transaction(&txn).await? != TransactionStatus::Retired {
                    return Err(CapeWalletError::Failed {
                        msg: format!("failed to mint {}", self.symbol),
                    });
                }
                asset
            }
            AssetKind::Wrapped { contract } => {
                info!("sponsoring {} which wraps {:#x}", self.symbol, contract);
                let client = wallet.eth_client().await?;
                let eth_addr = client.address();
                info!(
                    "ETH balance of {:#x} is {}",
                    eth_addr,
                    client.get_balance(eth_addr, None).await.unwrap()
                );
                wallet
                    .sponsor(
                        self.symbol.clone(),
                        contract.into(),
                        eth_addr.into(),
                        policy,
                    )
                    .await?
            }
        };

        // Update asset metadata.
        let icon_path = [icon_dir, &self.icon].iter().collect::<PathBuf>();
        let icon_file =
            File::open(&icon_path).map_err(|source| CapeWalletError::IoError { source })?;
        let icon =
            Icon::load_png(BufReader::new(icon_file)).map_err(|err| CapeWalletError::Failed {
                msg: format!("failed to load icon {}: {}", icon_path.display(), err),
            })?;
        let info = AssetInfo::from(asset)
            .with_name(self.symbol)
            .with_description(self.description)
            .with_icon(icon);
        wallet.import_asset(info.clone()).await?;

        Ok(info)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
enum AssetKind {
    Domestic { amount: u64 },
    Wrapped { contract: Address },
}

#[async_std::main]
async fn main() -> io::Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let opt = Options::from_args();

    // Read the config file specifying the library to create.
    let bytes = fs::read(&opt.assets)?;
    let toml =
        str::from_utf8(&bytes).unwrap_or_else(|_| panic!("{} is not UTF-8", opt.assets.display()));
    let spec: AssetLibrarySpec = toml::from_str(toml)?;

    // Create a new wallet which we will use to create the assets. This is a one-off wallet, so we
    // use a random password and storage location.
    let dir = TempDir::new("asset-library-wallet").unwrap();
    let mut rng = ChaChaRng::from_entropy();
    let mut loader = Loader::from_literal(
        Some(opt.cape_mnemonic.to_string()),
        Alphanumeric.sample_string(&mut rng, 16),
        dir.path().to_owned(),
    );
    let backend = CapeBackend::new(
        &*UNIVERSAL_PARAM,
        CapeBackendConfig {
            cape_contract: Some((opt.rpc_url, opt.contract_address)),
            eth_mnemonic: Some(opt.eth_mnemonic.to_string()),
            eqs_url: opt.eqs_url,
            relayer_url: opt.relayer_url,
            address_book_url: opt.address_book_url,
            min_polling_delay: Duration::from_millis(500),
        },
        &mut loader,
    )
    .await
    .map_err(wallet_error)?;
    let mut wallet = CapeWallet::new(backend).await.map_err(wallet_error)?;

    // Generate an address.
    let pub_key = wallet
        .generate_user_key("asset library creator".into(), None)
        .await
        .map_err(wallet_error)?;
    info!("issuer public key is {}", pub_key);

    // Create assets.
    let mut assets = Vec::new();
    for asset in spec.assets {
        assets.push(
            asset
                .create(
                    &mut wallet,
                    &pub_key,
                    opt.assets.parent().unwrap(),
                    &opt.faucet_url,
                )
                .await
                .map_err(wallet_error)?,
        );
    }

    let library = VerifiedAssetLibrary::new(assets, &opt.key_pair);
    let bytes = bincode::serialize(&library)
        .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
    if let Some(file) = opt.file {
        info!("writing asset library to {}", file.display());
        fs::write(&file, &bytes)?;
    } else {
        stdout().write_all(&bytes)?;
    }

    Ok(())
}

fn wallet_error(err: CapeWalletError) -> io::Error {
    io::Error::new(ErrorKind::Other, err.to_string())
}
