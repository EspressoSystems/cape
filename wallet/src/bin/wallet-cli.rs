// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # The CAPE Wallet CLI
//!
//! One of two main entrypoints to the wallet (the other being the web server). This executable
//! provides a command-line read-eval-print loop which allows the user to enter commands for a wallet
//! interactively.
//!
//! It instantiates the generic [seahorse::cli] in order to provide most of the functionality. It
//! then extends the generic CLI with additional CAPE-specific commands.
//!
//! ## Usage
//! ```
//! cargo run --release --bin wallet-cli -- [options]
//! ```
//!
//! You can use `--help` to see a list of the possible values for `[options]`. A particularly useful
//! option is `--storage PATH`, which sets the location the wallet will use to store keystore files.
//! This allows you to have multiple wallets in different directories.
//!
//! When you run the CLI, you will be prompted to create or open a wallet. Once you have an open
//! wallet, you will get the REPL prompt, `>`. Now you can type `help` to view a list of commands
//! you can execute.

extern crate cape_wallet;
use async_std::task::block_on;
use cap_rust_sandbox::{
    ledger::CapeLedger,
    model::{Erc20Code, EthereumAddr},
};
use cape_wallet::{
    backend::{CapeBackend, CapeBackendConfig},
    wallet::{CapeWalletBackend, CapeWalletExt},
};
use ethers::prelude::Address;
use jf_cap::{
    keys::{AuditorPubKey, FreezerPubKey},
    proof::UniversalParam,
    structs::{AssetCode, AssetDefinition, AssetPolicy},
};
use net::UserAddress;
use seahorse::{
    cli::*,
    io::SharedIO,
    loader::{LoaderMetadata, WalletLoader},
    WalletError,
};
use std::any::type_name;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::time::Duration;
use structopt::StructOpt;
use surf::Url;

/// Implementation of the [seahorse] [CLI] interface for CAPE.
pub struct CapeCli;

impl<'a> CLI<'a> for CapeCli {
    type Ledger = CapeLedger;
    type Backend = CapeBackend<'a, LoaderMetadata>;
    type Args = CapeArgs;

    fn init_backend(
        univ_param: &'a UniversalParam,
        args: Self::Args,
        loader: &mut impl WalletLoader<CapeLedger, Meta = LoaderMetadata>,
    ) -> Result<Self::Backend, WalletError<CapeLedger>> {
        let cape_contract = match (args.rpc_url, args.contract_address) {
            (Some(url), Some(address)) => Some((url, address)),
            _ => None,
        };
        block_on(CapeBackend::new(
            univ_param,
            CapeBackendConfig {
                cape_contract,
                eqs_url: args.eqs_url,
                relayer_url: args.relayer_url,
                address_book_url: args.address_book_url,
                eth_mnemonic: args.eth_mnemonic,
                min_polling_delay: Duration::from_millis(args.min_polling_delay_ms),
            },
            loader,
        ))
    }

    fn extra_commands() -> Vec<Command<'a, Self>> {
        cape_specific_cli_commands()
    }
}

impl<'a> CLIInput<'a, CapeCli> for AssetDefinition {
    fn parse_for_wallet(_wallet: &mut Wallet<'a, CapeCli>, s: &str) -> Option<Self> {
        Self::from_str(s).ok()
    }
}

impl<'a> CLIInput<'a, CapeCli> for EthereumAddr {
    fn parse_for_wallet(_wallet: &mut Wallet<'a, CapeCli>, s: &str) -> Option<Self> {
        Self::from_str(s).ok()
    }
}

impl<'a> CLIInput<'a, CapeCli> for Erc20Code {
    fn parse_for_wallet(_wallet: &mut Wallet<'a, CapeCli>, s: &str) -> Option<Self> {
        Self::from_str(s).ok()
    }
}

/// The instantiation of [seahorse::Wallet] for CAPE used by the CLI.
type CapeWallet<'a, Backend> = seahorse::Wallet<'a, Backend, CapeLedger>;

/// Implementation of the `sponsor` command for the CAPE wallet CLI.
#[allow(clippy::too_many_arguments)]
async fn cli_sponsor<'a, C: CLI<'a>>(
    io: &mut SharedIO,
    wallet: &mut CapeWallet<'a, C::Backend>,
    erc20_code: Erc20Code,
    sponsor_addr: EthereumAddr,
    symbol: Option<String>,
    viewer: Option<AuditorPubKey>,
    freezer: Option<FreezerPubKey>,
    view_amount: Option<bool>,
    view_address: Option<bool>,
    view_blind: Option<bool>,
    viewing_threshold: Option<u64>,
) where
    C::Backend: CapeWalletBackend<'a> + Sync + 'a,
{
    let mut policy = AssetPolicy::default();
    if let Some(viewer) = viewer {
        policy = policy.set_auditor_pub_key(viewer);
    }
    if let Some(freezer) = freezer {
        policy = policy.set_freezer_pub_key(freezer);
    }
    if Some(true) == view_amount {
        policy = match policy.reveal_amount() {
            Ok(policy) => policy,
            Err(err) => {
                cli_writeln!(io, "Invalid policy: {}", err);
                return;
            }
        }
    }
    if Some(true) == view_address {
        policy = match policy.reveal_user_address() {
            Ok(policy) => policy,
            Err(err) => {
                cli_writeln!(io, "Invalid policy: {}", err);
                return;
            }
        }
    }
    if Some(true) == view_blind {
        policy = match policy.reveal_blinding_factor() {
            Ok(policy) => policy,
            Err(err) => {
                cli_writeln!(io, "Invalid policy: {}", err);
                return;
            }
        }
    }
    if let Some(viewing_threshold) = viewing_threshold {
        policy = policy.set_reveal_threshold(viewing_threshold);
    }
    match wallet
        .sponsor(symbol.unwrap_or_default(), erc20_code, sponsor_addr, policy)
        .await
    {
        Ok(def) => {
            cli_writeln!(io, "{}", def);
        }
        Err(err) => {
            cli_writeln!(io, "{}\nAsset was not sponsored.", err);
        }
    }
}

/// Implementation of the `wrap` command for the CAPE wallet CLI.
async fn cli_wrap<'a, C: CLI<'a, Ledger = CapeLedger>>(
    io: &mut SharedIO,
    wallet: &mut CapeWallet<'a, C::Backend>,
    asset_def: AssetDefinition,
    from: EthereumAddr,
    to: UserAddress,
    amount: u64,
) where
    C::Backend: CapeWalletBackend<'a> + Sync + 'a,
{
    match wallet.wrap(from, asset_def.clone(), to.0, amount).await {
        Ok(()) => {
            cli_writeln!(io, "\nAsset wrapped: {}", asset_def.code);
        }
        Err(err) => {
            cli_writeln!(io, "{}\nAsset was not wrapped.", err);
        }
    }
}

/// Implementation of the `burn` command for the CAPE wallet CLI.
#[allow(clippy::too_many_arguments)]
async fn cli_burn<'a, C: CLI<'a, Ledger = CapeLedger>>(
    io: &mut SharedIO,
    wallet: &mut CapeWallet<'a, C::Backend>,
    asset: ListItem<AssetCode>,
    to: EthereumAddr,
    amount: u64,
    fee: u64,
    from: Option<UserAddress>,
    wait: Option<bool>,
) where
    C::Backend: CapeWalletBackend<'a> + Sync + 'a,
{
    let res = wallet
        .burn(
            from.map(|addr| addr.0).as_ref(),
            to,
            &asset.item,
            amount,
            fee,
        )
        .await;
    cli_writeln!(io, "{}", asset.item);

    finish_transaction::<C>(io, wallet, res, wait, "burned").await;
}

/// The collection of CLI commands which are specific to CAPE.
///
/// These commands are not part of the generic [seahorse::cli], but they are added to the CAPE CLI
/// via the [CLI::extra_commands] trait method.
fn cape_specific_cli_commands<'a>() -> Vec<Command<'a, CapeCli>> {
    vec![
        command!(
            sponsor,
            "sponsor an asset",
            CapeCli,
            |io,
             wallet,
             erc20_code: Erc20Code,
             sponsor_addr: EthereumAddr;
             symbol: Option<String>,
             viewer: Option<AuditorPubKey>,
             freezer: Option<FreezerPubKey>,
             view_amount: Option<bool>,
             view_address: Option<bool>,
             view_blind: Option<bool>,
             viewing_threshold: Option<u64>| {
                cli_sponsor::<CapeCli>(
                    io,
                    wallet,
                    erc20_code,
                    sponsor_addr,
                    symbol,
                    viewer,
                    freezer,
                    view_amount,
                    view_address,
                    view_blind,
                    viewing_threshold,
                ).await;
            }
        ),
        command!(
            wrap,
            "wrap an asset",
            CapeCli,
            |io,
             wallet,
             asset_def: AssetDefinition,
             from: EthereumAddr,
             to: UserAddress,
             amount: u64| {
                cli_wrap::<CapeCli>(io, wallet, asset_def, from, to, amount).await;
            }
        ),
        command!(
            burn,
            "burn some of a wrapped asset and withdraw the funds to an ERC-20 account",
            CapeCli,
            |io,
             wallet,
             asset: ListItem<AssetCode>,
             to: EthereumAddr,
             amount: u64,
             fee: u64;
             from: Option<UserAddress>,
             wait: Option<bool>| {
                cli_burn::<CapeCli>(io, wallet, asset, to, amount, fee, from, wait).await;
            }
        ),
    ]
}

/// Command line arguments for the CAPE wallet CLI.
#[derive(StructOpt)]
pub struct CapeArgs {
    /// Generate keys for a wallet, do not run the REPL.
    ///
    /// The keys are stored in FILE and FILE.pub.
    #[structopt(short = "g", long)]
    pub key_gen: Option<PathBuf>,

    /// Path to a saved wallet, or a new directory where this wallet will be saved.
    ///
    /// If not given, the wallet will be stored in ~/.espresso/cape/wallet. If a wallet already
    /// exists there, it will be loaded. Otherwise, a new wallet will be created.
    #[structopt(short, long)]
    pub storage: Option<PathBuf>,

    /// Create a new wallet and store it an a temporary location which will be deleted on exit.
    ///
    /// This option is mutually exclusive with --storage.
    #[structopt(long)]
    #[structopt(conflicts_with("storage"))]
    #[structopt(hidden(true))]
    pub tmp_storage: bool,

    #[structopt(long)]
    /// Run in a mode which is friendlier to automated scripting.
    ///
    /// Instead of prompting the user for input with a line editor, the prompt will be printed,
    /// followed by a newline, and the input will be read without an editor.
    pub non_interactive: bool,

    /// URL for the CAPE Ethereum Query Service.
    #[structopt(long, env = "CAPE_EQS_URL", default_value = "http://localhost:50087")]
    pub eqs_url: Url,

    /// URL for the CAPE relayer.
    #[structopt(
        long,
        env = "CAPE_RELAYER_URL",
        default_value = "http://localhost:50077"
    )]
    pub relayer_url: Url,

    /// URL for the CAPE Address Book.
    #[structopt(
        long,
        env = "CAPE_ADDRESS_BOOK_URL",
        default_value = "http://localhost:50078"
    )]
    pub address_book_url: Url,

    /// Address of the CAPE smart contract.
    #[structopt(long, env = "CAPE_CONTRACT_ADDRESS", requires = "rpc_url")]
    pub contract_address: Option<Address>,

    /// URL for Ethers HTTP Provider
    #[structopt(long, env = "CAPE_WEB3_PROVIDER_URL", requires = "contract_address")]
    pub rpc_url: Option<Url>,

    /// Mnemonic for a local Ethereum wallet for direct contract calls.
    #[structopt(long, env = "ETH_MNEMONIC")]
    pub eth_mnemonic: Option<String>,

    /// Minimum amount of time to wait between polling requests to EQS.
    #[structopt(long, env = "CAPE_WALLET_MIN_POLLING_DELAY", default_value = "500")]
    pub min_polling_delay_ms: u64,
}

impl CLIArgs for CapeArgs {
    fn key_gen_path(&self) -> Option<PathBuf> {
        self.key_gen.clone()
    }

    fn storage_path(&self) -> Option<PathBuf> {
        self.storage.clone()
    }

    fn io(&self) -> Option<SharedIO> {
        if self.non_interactive {
            Some(SharedIO::std())
        } else {
            None
        }
    }

    fn use_tmp_storage(&self) -> bool {
        self.tmp_storage
    }
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Initialize the wallet CLI.
    if let Err(err) = cli_main::<CapeLedger, CapeCli>(CapeArgs::from_args()).await {
        println!("{}", err);
        exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)] // Some helper functions are only used with feature "slow-tests"

    use super::*;
    use async_std::{
        sync::{Arc, Mutex},
        task::spawn,
    };
    use cape_wallet::{
        cli_client::CliClient,
        mocks::{CapeTest, MockCapeBackend, MockCapeLedger},
    };
    use futures::stream::{iter, StreamExt};
    use pipe::{PipeReader, PipeWriter};
    use seahorse::{
        hd, io::Tee, persistence::AtomicWalletStorage, testing::cli_match::*,
        testing::SystemUnderTest, WalletBackend,
    };
    use std::io::BufRead;
    use std::time::Instant;

    pub struct MockCapeCli;

    impl<'a> CLI<'a> for MockCapeCli {
        type Ledger = CapeLedger;
        type Backend = MockCapeBackend<'a, LoaderMetadata>;
        type Args = MockCapeArgs<'a>;

        fn init_backend(
            _univ_param: &'a UniversalParam,
            args: Self::Args,
            loader: &mut impl WalletLoader<CapeLedger, Meta = LoaderMetadata>,
        ) -> Result<Self::Backend, WalletError<CapeLedger>> {
            MockCapeBackend::new_for_test(
                args.ledger.clone(),
                Arc::new(Mutex::new(AtomicWalletStorage::new(loader, 128)?)),
                args.key_stream,
            )
        }

        fn extra_commands() -> Vec<Command<'a, Self>> {
            vec![
                command!(
                    sponsor,
                    "sponsor an asset",
                    Self,
                    |io,
                     wallet,
                     erc20_code: Erc20Code,
                     sponsor_addr: EthereumAddr;
                     symbol: Option<String>,
                     viewer: Option<AuditorPubKey>,
                     freezer: Option<FreezerPubKey>,
                     view_amount: Option<bool>,
                     view_address: Option<bool>,
                     view_blind: Option<bool>,
                     viewing_threshold: Option<u64>| {
                        cli_sponsor::<MockCapeCli>(
                            io,
                            wallet,
                            erc20_code,
                            sponsor_addr,
                            symbol,
                            viewer,
                            freezer,
                            view_amount,
                            view_address,
                            view_blind,
                            viewing_threshold,
                        ).await;
                    }
                ),
                command!(
                    wrap,
                    "wrap an asset",
                    Self,
                    |io,
                     wallet,
                     asset_def: AssetDefinition,
                     from: EthereumAddr,
                     to: UserAddress,
                     amount: u64| {
                        cli_wrap::<MockCapeCli>(io, wallet, asset_def, from, to, amount).await;
                    }
                ),
                command!(
                    burn,
                    "burn some of a wrapped asset and withdraw the funds to an ERC-20 account",
                    Self,
                    |io,
                     wallet,
                     asset: ListItem<AssetCode>,
                     to: EthereumAddr,
                     amount: u64,
                     fee: u64;
                     from: Option<UserAddress>,
                     wait: Option<bool>| {
                        cli_burn::<MockCapeCli>(io, wallet, asset, to, amount, fee, from, wait).await;
                    }
                ),
            ]
        }
    }

    impl<'a> CLIInput<'a, MockCapeCli> for AssetDefinition {
        fn parse_for_wallet(_wallet: &mut Wallet<'a, MockCapeCli>, s: &str) -> Option<Self> {
            Self::from_str(s).ok()
        }
    }
    impl<'a> CLIInput<'a, MockCapeCli> for EthereumAddr {
        fn parse_for_wallet(_wallet: &mut Wallet<'a, MockCapeCli>, s: &str) -> Option<Self> {
            Self::from_str(s).ok()
        }
    }
    impl<'a> CLIInput<'a, MockCapeCli> for Erc20Code {
        fn parse_for_wallet(_wallet: &mut Wallet<'a, MockCapeCli>, s: &str) -> Option<Self> {
            Self::from_str(s).ok()
        }
    }

    pub struct MockCapeArgs<'a> {
        io: SharedIO,
        key_stream: hd::KeyTree,
        ledger: Arc<Mutex<MockCapeLedger<'a>>>,
    }
    impl<'a> CLIArgs for MockCapeArgs<'a> {
        fn key_gen_path(&self) -> Option<PathBuf> {
            None
        }
        fn storage_path(&self) -> Option<PathBuf> {
            None
        }
        fn io(&self) -> Option<SharedIO> {
            Some(self.io.clone())
        }
        fn use_tmp_storage(&self) -> bool {
            true
        }
    }

    async fn create_cape_network<'a>(
        t: &mut CapeTest,
        initial_grants: &[u64],
    ) -> (Arc<Mutex<MockCapeLedger<'a>>>, Vec<hd::KeyTree>) {
        // Use `create_test_network` to create a ledger with some initial records.
        let (ledger, wallets) = t
            .create_test_network(&[(2, 2)], initial_grants.to_vec(), &mut Instant::now())
            .await;
        // Set `block_size` to `1` so we don't have to explicitly flush the ledger after each
        // transaction submission.
        ledger.lock().await.set_block_size(1).unwrap();
        // We don't actually care about the open wallets returned by `create_test_network`, because
        // the CLI does its own wallet loading. But we do want to get their key streams, so that
        // the wallets we create through the CLI can deterministically generate the keys that own
        // the initial records.
        let key_streams = iter(wallets)
            .then(|(wallet, _)| async move { wallet.lock().await.backend().key_stream() })
            .collect::<Vec<_>>()
            .await;
        (ledger, key_streams)
    }

    fn create_cape_wallet(
        ledger: Arc<Mutex<MockCapeLedger<'static>>>,
        key_stream: hd::KeyTree,
    ) -> (Tee<PipeWriter>, Tee<PipeReader>) {
        let (io, input, output) = SharedIO::pipe();

        // Run a CLI interface for a wallet in the background.
        spawn(async move {
            let args = MockCapeArgs {
                io,
                key_stream,
                ledger,
            };
            cli_main::<CapeLedger, MockCapeCli>(args).await.unwrap();
        });

        // Wait for the CLI to start up and then return the input and output pipes.
        let mut input = Tee::new(input);
        let mut output = Tee::new(output);
        wait_for_prompt(&mut output);

        // Accept the generated mnemonic.
        writeln!(input, "1").unwrap();
        // Instead of a ">" prompt, the wallet will ask us to create a password, so
        // `wait_for_prompt` doesn't work, we just need to consume a line of output.
        output.read_line(&mut String::new()).unwrap();
        // Create a password.
        writeln!(input, "test-password").unwrap();
        output.read_line(&mut String::new()).unwrap();
        // Confirm password.
        writeln!(input, "test-password").unwrap();
        wait_for_prompt(&mut output);

        (input, output)
    }

    fn create_wallet(t: &mut CliClient, wallet: usize) -> Result<&mut CliClient, String> {
        let key_path = t.wallet_key_path(wallet)?;
        let key_path = key_path.as_os_str().to_str().ok_or_else(|| {
            format!(
                "failed to convert key path {:?} for wallet {} to string",
                key_path, wallet
            )
        })?;
        t.open_with_args(wallet, ["--password"])?
            .output("Create password:")?
            .command(wallet, "test_password")?
            .output("Retype password:")?
            .command(wallet, "test_password")?
            .output("connecting...")?
            .command(wallet, format!("load_key send {}", key_path))?
            .output(format!("(?P<default_addr{}>ADDR~.*)", wallet))
    }

    fn cli_sponsor_all_args(t: &mut CliClient, sponsor_addr: &EthereumAddr) -> Result<(), String> {
        // Set an ERC 20 code to sponsor.
        let erc20_code = Erc20Code(EthereumAddr([1u8; 20]));

        t
            // Generate freezing and viewing keys.
            .command(0, "gen_key freezing")?
            .output("(?P<freezer>FREEZEPUBKEY~.*)")?
            .command(0, "gen_key viewing")?
            .output("(?P<viewer>AUDPUBKEY~.*)")?
            // Sponsor an asset with all policy attributes specified.
            .command(0, format!("sponsor {} {} viewer=$viewer freezer=$freezer view_amount=true view_address=true view_blind=true viewing_threshold=10", erc20_code, sponsor_addr))?
            .output(format!("(?P<asset_def>ASSET_DEF~.*)"))?;

        Ok(())
    }

    fn cli_sponsor_skipped_args(
        t: &mut CliClient,
        sponsor_addr: &EthereumAddr,
    ) -> Result<(), String> {
        // Set an ERC 20 code to sponsor.
        let erc20_code = Erc20Code(EthereumAddr([1u8; 20]));

        t
            // Sponsor an asset with the default policy.
            .command(0, format!("sponsor {} {}", erc20_code, sponsor_addr))?
            .output(format!("(?P<asset_default>ASSET_DEF~.*)"))?
            // Sponsor an unviewable asset with a freezer key.
            .command(0, "gen_key freezing")?
            .output("(?P<freezer>FREEZEPUBKEY~.*)")?
            .command(0, format!("sponsor {} {} freezer=$freezer", erc20_code, sponsor_addr))?
            .output(format!("(?P<asset_unviewable>ASSET_DEF~.*)"))?
            // Sponsor a viewable asset without a freezer key.
            .command(0, "gen_key viewing")?
            .output("(?P<viewer>AUDPUBKEY~.*)")?
            .command(0, format!("sponsor {} {} viewer=$viewer view_amount=true view_address=true view_blind=true viewing_threshold=10", erc20_code, sponsor_addr))?
            .output(format!("(?P<asset_viewable>ASSET_DEF~.*)"))?
            // Should fail to sponsor an viewable asset without a given viewing key.
            .command(0, format!("sponsor {} {} view_amount=true view_address=true view_blind=true viewing_threshold=10", erc20_code, sponsor_addr))?
            .output(format!("Invalid policy: Invalid parameters: Cannot reveal amount to dummy AuditorPublicKey"))?;

        Ok(())
    }

    fn cli_wrap_sponsored(
        t: &mut CliClient,
        eth_addr: &EthereumAddr,
        amount: u64,
    ) -> Result<(), String> {
        // Wrap an asset.
        t.command(
            0,
            format!("wrap $asset_def {} $default_addr0 {}", eth_addr, amount),
        )?
        .output(format!("Asset wrapped."))?;

        Ok(())
    }

    fn cli_wrap_unsponsored(
        t: &mut CliClient,
        eth_addr: &EthereumAddr,
        amount: u64,
    ) -> Result<(), String> {
        // Should fail to wrap an unsponsored asset.
        t.command(
            0,
            format!(
                "wrap {} {} $default_addr0 {}",
                AssetDefinition::dummy(),
                eth_addr,
                amount
            ),
        )?
        .output(format!("UndefinedAsset"))?
        .output(format!("Asset was not wrapped."))?;

        Ok(())
    }

    fn cli_burn_insufficient_balance(t: &mut CliClient) -> Result<(), String> {
        // Set a hard-coded Ethereum address for testing.
        let erc20_addr = EthereumAddr([1u8; 20]);

        // Should output an error of insufficent balance.
        t.command(0, format!("burn 0 {} 10 1", erc20_addr))?
            .output(format!("TransactionError: InsufficientBalance"))?;
        Ok(())
    }

    // Disabled until we can replace the use of `CliClient` with `CapeTest` and CLI matching helpers
    // in Seahorse, similar to `test_cli_burn`. Related issue:
    // https://github.com/EspressoSystems/cape/issues/477.
    #[test]
    #[ignore]
    fn test_cli_sponsor() {
        cape_wallet::cli_client::cli_test(|t| {
            create_wallet(t, 0)?;

            let sponsor_addr = EthereumAddr([2u8; 20]);
            cli_sponsor_all_args(t, &sponsor_addr)?;
            cli_sponsor_skipped_args(t, &sponsor_addr)?;

            Ok(())
        });
    }

    // Disabled until we can replace the use of `CliClient` with `CapeTest` and CLI matching helpers
    // in Seahorse, similar to `test_cli_burn`. Related issue:
    // https://github.com/EspressoSystems/cape/issues/477.
    #[test]
    #[ignore]
    fn test_cli_wrap() {
        cape_wallet::cli_client::cli_test(|t| {
            create_wallet(t, 0)?;
            let sponsor_addr = EthereumAddr([2u8; 20]);
            cli_sponsor_all_args(t, &sponsor_addr)?;

            let wrapper_addr = EthereumAddr([3u8; 20]);
            let amount = 10;
            cli_wrap_sponsored(t, &wrapper_addr, amount)?;
            cli_wrap_unsponsored(t, &wrapper_addr, amount)?;

            Ok(())
        });
    }

    #[cfg(feature = "slow-tests")]
    #[async_std::test]
    async fn test_cli_burn() {
        let mut t = CapeTest::default();
        let (ledger, key_streams) = create_cape_network(&mut t, &[2000, 2000, 2000]).await;

        // Create wallets for sponsor, wrapper and receiver.
        let (mut sponsor_input, mut sponsor_output) =
            create_cape_wallet(ledger.clone(), key_streams[0].clone());
        let (mut wrapper_input, mut wrapper_output) =
            create_cape_wallet(ledger.clone(), key_streams[1].clone());
        let (mut receiver_input, mut receiver_output) =
            create_cape_wallet(ledger.clone(), key_streams[2].clone());

        // Get the freezing and viewing keys for the sponsor, and the receiver's addresses.
        writeln!(sponsor_input, "gen_key freezing").unwrap();
        let freezing_key =
            match_output(&mut sponsor_output, &["(?P<freezing>FREEZEPUBKEY~.*)"]).get("freezing");
        writeln!(sponsor_input, "gen_key viewing").unwrap();
        let viewing_key =
            match_output(&mut sponsor_output, &["(?P<viewing>AUDPUBKEY~.*)"]).get("viewing");
        writeln!(receiver_input, "gen_key sending scan_from=start wait=true").unwrap();
        let receiver_addr = match_output(&mut receiver_output, &["(?P<addr>ADDR~.*)"]).get("addr");
        writeln!(receiver_input, "balance 0").unwrap();
        match_output(&mut receiver_output, &[format!("{} 1000", receiver_addr)]);

        // Sponsor and wrap an asset.
        let erc20_code = Erc20Code(EthereumAddr([1u8; 20]));
        let sponsor_eth_addr = EthereumAddr([2u8; 20]);
        writeln!(sponsor_input, "sponsor {} {} freezer={} viewer={} view_amount=true view_address=true view_blind=true viewing_threshold=10", erc20_code, sponsor_eth_addr, freezing_key, viewing_key).unwrap();
        let asset_def =
            match_output(&mut sponsor_output, &["(?P<asset_def>ASSET_DEF~.*)"]).get("asset_def");
        let wrapper_eth_addr = EthereumAddr([3u8; 20]);
        let wrap_amount = 10;
        writeln!(
            wrapper_input,
            "wrap {} {} {} {}",
            asset_def, wrapper_eth_addr, receiver_addr, wrap_amount
        )
        .unwrap();
        let wrapped_asset = match_output(
            &mut wrapper_output,
            &["Asset wrapped: (?P<asset_code>ASSET_CODE~.*)"],
        )
        .get("asset_code");

        // Submit a dummy transaction to finalize the wrap.
        writeln!(receiver_input, "create_asset my_asset").unwrap();
        wait_for_prompt(&mut receiver_output);
        let mint_amount = 20;
        writeln!(receiver_input, "mint 1 {} {} 1", receiver_addr, mint_amount).unwrap();
        let txn = match_output(&mut receiver_output, &["(?P<txn>TXN~.*)"]).get("txn");
        await_transaction(
            &txn,
            (&mut receiver_input.clone(), &mut receiver_output.clone()),
            &mut [(&mut receiver_input, &mut receiver_output)],
        );

        // Check the balance of the wrapped asset.
        writeln!(receiver_input, "balance {}", wrapped_asset).unwrap();
        match_output(
            &mut receiver_output,
            &[format!("{} {}", receiver_addr, wrap_amount)],
        );

        // Burn the wrapped asset.
        writeln!(
            receiver_input,
            "burn {} {} {} 1",
            wrapped_asset, wrapper_eth_addr, wrap_amount
        )
        .unwrap();
        let txn = match_output(&mut receiver_output, &["(?P<txn>TXN~.*)"]).get("txn");
        await_transaction(
            &txn,
            (&mut receiver_input.clone(), &mut receiver_output.clone()),
            &mut [(&mut receiver_input, &mut receiver_output)],
        );

        // Check that the wrapped asset has been burned.
        writeln!(receiver_input, "balance {}", wrapped_asset).unwrap();
        match_output(&mut receiver_output, &[format!("{} {}", receiver_addr, 0)]);
    }
}
