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
    structs::{AssetCode, AssetDefinition, AssetPolicy},
    KeyPair, VerKey,
};
use rand::distributions::{Alphanumeric, DistString};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::{
    asset_library::{Icon, VerifiedAssetLibrary},
    hd::{KeyTree, Mnemonic},
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AssetLibrarySpec {
    assets: Vec<Asset>,
}

impl AssetLibrarySpec {
    fn from_file(path: &Path) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        let toml = str::from_utf8(&bytes).map_err(|_| {
            io::Error::new(ErrorKind::Other, format!("{} is not UTF-8", path.display()))
        })?;
        toml::from_str(toml).map_err(io::Error::from)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Asset {
    symbol: String,
    description: Option<String>,
    icon: Option<PathBuf>,
    viewing_key: Option<AuditorPubKey>,
    freezing_key: Option<FreezerPubKey>,
    #[serde(flatten)]
    kind: AssetKind,
}

impl Asset {
    fn load_icon(&self, icon_dir: &Path) -> io::Result<Option<Icon>> {
        if let Some(icon) = &self.icon {
            let icon_path = [icon_dir, icon].iter().collect::<PathBuf>();
            let icon_file = File::open(&icon_path)?;
            Icon::load_png(BufReader::new(icon_file))
                .map_err(|err| {
                    io::Error::new(
                        ErrorKind::Other,
                        format!("failed to load icon {}: {}", icon_path.display(), err),
                    )
                })
                .map(Some)
        } else {
            Ok(None)
        }
    }

    async fn create<'a>(
        self,
        wallet: &mut CapeWallet<'a, CapeBackend<'a, LoaderMetadata>>,
        pub_key: &UserPubKey,
        icon_dir: &Path,
        faucet_url: &Url,
    ) -> Result<AssetInfo, CapeWalletError> {
        info!("creating asset {}", self.symbol);

        let icon = self
            .load_icon(icon_dir)
            .map_err(|source| CapeWalletError::IoError { source })?;

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
                    let mut res = surf::post(faucet_url.join("request_fee_assets").unwrap())
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
                            msg: format!(
                                "faucet request failed: {}",
                                res.body_string().await.unwrap()
                            ),
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
            AssetKind::Native => {
                info!("adding native asset to library");
                AssetDefinition::native()
            }
        };

        // Update asset metadata.
        let mut info = AssetInfo::from(asset).with_name(self.symbol);
        if let Some(description) = self.description {
            info = info.with_description(description);
        }
        if let Some(icon) = icon {
            info = info.with_icon(icon);
        }
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
    Native,
}

fn write_library(
    assets: impl IntoIterator<Item = AssetInfo>,
    key_pair: &KeyPair,
    file: Option<&PathBuf>,
) -> io::Result<()> {
    let library = VerifiedAssetLibrary::new(assets, key_pair);
    let bytes = bincode::serialize(&library)
        .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
    if let Some(file) = file {
        info!("writing asset library to {}", file.display());
        fs::write(file, &bytes)
    } else {
        stdout().write_all(&bytes)
    }
}

/// Generate or update an official asset library for CAPE.
///
/// The `generate` and `update` commands read asset library specifications from a TOML file. The
/// format of the TOML specification is an array of assets, like
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
///     type = 'wrapped|domestic|native'
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
enum Command {
    Generate(Box<GenerateCommand>),
    Update(Box<UpdateCommand>),
    Show(ShowCommand),
    KeyGen(KeyGenCommand),
}

/// Generate an official asset library for CAPE.
///
///     official-asset-library generate --key-pair KEY --cape-mnemonic CAPE_MNEMONIC --eth-mnemonic ETH_MNEMONIC --assets ASSETS -o FILE
///
/// Reads the TOML specification ASSETS, creates the specified assets and deploys them to a CAPE
/// instance, and then generates a signed binary file containing the resulting asset definitions
/// and metadata.
///
/// KEY is the signing key pair to use to sign the library
/// CAPE_MNEMONIC is the mnemonic for the CAPE wallet which will be used to create the assets.
///     This mnemonic can later be used to distributed the minted records of any domestic assets
///     in the library.
/// ETH_MNEMONIC is the mnemonic for the Ethereum wallet which will be used to sponsor the
///     wrapped assets in the library. The first address generated by this mnemonic must be
///     funded with ETH on the target chain, and it will be charged gas for all of the sponsor
///     operations required to create the asset library.     
/// ASSETS is the path to a TOML specification of the asset library to create
/// FILE is the binary asset library to create.
///
/// The user must also set the following environment variables to indicate the CAPE deployment to
/// which the new assets should be deployed:
///     CAPE_EQS_URL
///     CAPE_RELAYER_URL
///     CAPE_ADDRESS_BOOK_URL
///     CAPE_FAUCET_URL
///     CAPE_CONTRACT_ADDRESS
///     CAPE_WEB3_PROVIDER_URL
#[derive(StructOpt)]
struct GenerateCommand {
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

impl GenerateCommand {
    async fn run(self) -> io::Result<()> {
        // Read the config file specifying the library to create.
        let spec = AssetLibrarySpec::from_file(&self.assets)?;

        // Create a new wallet which we will use to create the assets. This is a one-off wallet, so
        // we use a random password and storage location.
        let dir = TempDir::new("asset-library-wallet").unwrap();
        let mut rng = ChaChaRng::from_entropy();
        let mut loader = Loader::from_literal(
            Some(self.cape_mnemonic.to_string()),
            Alphanumeric.sample_string(&mut rng, 16),
            dir.path().to_owned(),
        );
        let backend = CapeBackend::new(
            &*UNIVERSAL_PARAM,
            CapeBackendConfig {
                cape_contract: Some((self.rpc_url, self.contract_address)),
                eth_mnemonic: Some(self.eth_mnemonic.to_string()),
                eqs_url: self.eqs_url,
                relayer_url: self.relayer_url,
                address_book_url: self.address_book_url,
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
                        self.assets.parent().unwrap(),
                        &self.faucet_url,
                    )
                    .await
                    .map_err(wallet_error)?,
            );
        }

        write_library(assets, &self.key_pair, self.file.as_ref())
    }
}

/// Update metadata in an official asset library.
///
///     official-asset-library update --key-pair KEY --assets ASSETS -i IN -o OUT
///
/// Reads the TOML specification ASSETS and the binary asset library IN, updates the off-chain
/// metadata for each asset in IN according to ASSETS, and outputs the resulting signed asset
/// library to OUT.
///
/// KEY is the signing key pair to use to sign the library
/// ASSETS is the path to a TOML file specifying how to update the asset library
/// IN is the binary asset library to update
/// OUT is the binary asset library to create
///
/// This command may also be used to update the signing key that is used to sign the asset library.
/// By default, it uses the public verifying key of KEY to verify the signature on the input
/// library. However, if `--ver-key VERKEY` is passed, then VERKEY will be used to verify the input
/// library, and KEY, the key pair used to sign the output library, can be different than the key
/// used to sign in the input library.
///
/// The command will check that the set of assets defined in IN matches the specification in ASSETS,
/// and that the on-chain asset information (the asset code and policy) matches that defined in
/// ASSETS. As such, this command cannot be used to update on-chain asset information -- only
/// off-chain metadata: symbols, descriptions, and icons.
///
/// This command does not deploy any new assets on chain, nor does it check that the assets in IN
/// are properly deployed on any particular chain. It is up to the user to ensure that this command
/// is only used to update asset libraries which have already been successfully deployed, for
/// example using official-asset-library generate.
#[derive(StructOpt)]
struct UpdateCommand {
    /// The public signature verifying key used to authenticate the existing asset library.
    #[structopt(short, long, name = "VERKEY", env = "CAPE_ASSET_LIBRARY_VERIFIER_KEY")]
    ver_key: Option<VerKey>,

    /// The signing key pair to use to authenticate the generated asset library.
    #[structopt(short, long, name = "KEY", env = "CAPE_ASSET_LIBRARY_SIGNING_KEY")]
    key_pair: KeyPair,

    /// The path of the library file to update.
    #[structopt(short = "i", long = "input", name = "IN")]
    in_file: PathBuf,

    /// The path of the library file to create.
    ///
    /// If not provided, the library will be written to stdout.
    #[structopt(short = "o", long = "output", name = "OUT")]
    out_file: Option<PathBuf>,

    /// Path to a .toml file specifying the asset library to generate.
    #[structopt(short, long, name = "ASSETS")]
    assets: PathBuf,
}

impl UpdateCommand {
    fn run(self) -> io::Result<()> {
        let library_bytes = fs::read(&self.in_file)?;
        let library: VerifiedAssetLibrary = bincode::deserialize(&library_bytes)
            .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;

        // Extract the assets.
        let assets = library
            .open(&self.ver_key.unwrap_or_else(|| self.key_pair.ver_key()))
            .ok_or_else(|| io::Error::new(ErrorKind::Other, "incorrect VERKEY"))?;

        // Get the assets from the spec.
        let spec = AssetLibrarySpec::from_file(&self.assets)?.assets;

        if assets.len() != spec.len() {
            return Err(io::Error::new(
                ErrorKind::Other,
                "number of assets in ASSETS does not match IN",
            ));
        }

        let assets = assets
            .into_iter()
            .zip(spec)
            .map(|(asset, spec)| {
                // Validate that the on-chain data in the spec matches what we have.
                let policy = asset.definition.policy_ref();
                match &spec.viewing_key {
                    Some(key) if policy.is_auditor_pub_key_set() => {
                        if key != policy.auditor_pub_key() {
                            return Err(io::Error::new(
                                ErrorKind::Other,
                                format!(
                                    "viewing key for asset {} does not match ASSETS",
                                    spec.symbol
                                ),
                            ));
                        }
                    }
                    Some(_) => {
                        return Err(io::Error::new(
                            ErrorKind::Other,
                            format!(
                                "viewing key for asset {} is set in ASSETS but not in IN",
                                spec.symbol
                            ),
                        ))
                    }
                    None if policy.is_auditor_pub_key_set() => {
                        return Err(io::Error::new(
                            ErrorKind::Other,
                            format!(
                                "viewing key for asset {} is set in IN but not in ASSETS",
                                spec.symbol
                            ),
                        ));
                    }
                    None => {}
                }
                match &spec.freezing_key {
                    Some(key) if policy.is_freezer_pub_key_set() => {
                        if key != policy.freezer_pub_key() {
                            return Err(io::Error::new(
                                ErrorKind::Other,
                                format!(
                                    "freezing key for asset {} does not match ASSETS",
                                    spec.symbol
                                ),
                            ));
                        }
                    }
                    Some(_) => {
                        return Err(io::Error::new(
                            ErrorKind::Other,
                            format!(
                                "freezing key for asset {} is set in ASSETS but not in IN",
                                spec.symbol
                            ),
                        ))
                    }
                    None if policy.is_freezer_pub_key_set() => {
                        return Err(io::Error::new(
                            ErrorKind::Other,
                            format!(
                                "freezing key for asset {} is set in IN but not in ASSETS",
                                spec.symbol
                            ),
                        ));
                    }
                    None => {}
                }

                // Update the off-chain metadata according to the spec.
                let icon = spec.load_icon(self.assets.parent().unwrap())?;
                let mut asset = asset.with_name(spec.symbol);
                if let Some(description) = spec.description {
                    asset = asset.with_description(description);
                }
                if let Some(icon) = icon {
                    asset = asset.with_icon(icon);
                }
                Ok(asset)
            })
            .collect::<Result<Vec<AssetInfo>, io::Error>>()?;

        write_library(assets, &self.key_pair, self.out_file.as_ref())
    }
}

/// Print the binary asset library FILE which was signed by VERKEY.
///
/// The library is printed using the TOML specification format.
#[derive(StructOpt)]
struct ShowCommand {
    #[structopt(short = "k", long, name = "VERKEY")]
    ver_key: VerKey,

    #[structopt(name = "FILE")]
    file: PathBuf,
}

impl ShowCommand {
    fn run(self) -> io::Result<()> {
        let library_bytes = fs::read(&self.file)?;
        let library: VerifiedAssetLibrary = bincode::deserialize(&library_bytes)
            .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
        let assets = library
            .open(&self.ver_key)
            .ok_or_else(|| io::Error::new(ErrorKind::Other, "incorrect VERKEY"))?;

        for asset in assets {
            println!("[[assets]]");
            if let Some(symbol) = &asset.name {
                println!("symbol = '{}'", symbol);
            }
            if let Some(description) = &asset.description {
                println!("description = '{}'", description);
            }
            if asset.icon.is_some() {
                println!("icon = <icon for {}>", asset.name.unwrap_or_default());
            }

            let policy = asset.definition.policy_ref();
            if policy.is_auditor_pub_key_set() {
                println!("viewing_key = '{}'", policy.auditor_pub_key());
            }
            if policy.is_freezer_pub_key_set() {
                println!("freezing_key = '{}'", policy.freezer_pub_key());
            }

            println!();
        }

        Ok(())
    }
}

/// Randomly generate new keys for an official asset library.
///
///     official-asset-library key-gen
///
/// Randomly generates a mnemonic phrase for an Ethereum wallet, a mnemonic phrase for a CAPE
/// wallet, and a signing key pair. Prints out all of the secret keys and the public verifying key
/// associated with the secret signing key. The keys are printed in a format that can be sourced as
/// environment variables, using the names of the environment variables required to use them with
/// this program and with the CAPE wallet.
#[derive(StructOpt)]
struct KeyGenCommand {}

impl KeyGenCommand {
    fn run(self) {
        let mut rng = ChaChaRng::from_entropy();
        let sign_key = KeyPair::generate(&mut rng);
        let eth_mnemonic = KeyTree::random(&mut rng).1;
        let cape_mnemonic = KeyTree::random(&mut rng).1;

        println!(
            "CAPE_ASSET_LIBRARY_ETH_MNEMONIC=\"{}\"",
            eth_mnemonic.into_phrase()
        );
        println!(
            "CAPE_ASSET_LIBRARY_CAPE_MNEMONIC=\"{}\"",
            cape_mnemonic.into_phrase()
        );
        println!("CAPE_ASSET_LIBRARY_SIGNING_KEY=\"{}\"", sign_key);
        println!(
            "CAPE_WALLET_ASSET_LIBRARY_VERIFIER_KEY=\"{}\"",
            sign_key.ver_key()
        );
    }
}

#[async_std::main]
async fn main() -> io::Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    match Command::from_args() {
        Command::Generate(generate) => generate.run().await?,
        Command::Update(update) => update.run()?,
        Command::Show(show) => show.run()?,
        Command::KeyGen(keygen) => keygen.run(),
    }

    Ok(())
}

fn wallet_error(err: CapeWalletError) -> io::Error {
    io::Error::new(ErrorKind::Other, err.to_string())
}
