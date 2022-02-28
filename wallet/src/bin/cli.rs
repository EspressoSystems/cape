////////////////////////////////////////////////////////////////////////////////
// The CAPE Wallet Frontend
//
// For now, this "frontend" is simply a command-line read-eval-print loop which
// allows the user to enter commands for a wallet interactively.
//

extern crate cape_wallet;
use async_std::sync::Mutex;
use cap_rust_sandbox::{
    ledger::CapeLedger,
    model::{Erc20Code, EthereumAddr},
};
use cape_wallet::{
    mocks::{MockCapeBackend, MockCapeNetwork},
    wallet::CapeWalletExt,
};
use jf_cap::{
    keys::{AuditorPubKey, FreezerPubKey},
    proof::UniversalParam,
    structs::{AssetCode, AssetDefinition, AssetPolicy},
    MerkleTree, TransactionVerifyingKey,
};
use key_set::{KeySet, VerifierKeySet};
use net::UserAddress;
use reef::Ledger;
use seahorse::{
    cli::*,
    io::SharedIO,
    loader::{LoaderMetadata, WalletLoader},
    testing::MockLedger,
    WalletError,
};
use std::any::type_name;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::sync::Arc;
use structopt::StructOpt;

pub struct CapeCli;

impl<'a> CLI<'a> for CapeCli {
    type Ledger = CapeLedger;
    type Backend = MockCapeBackend<'a, LoaderMetadata>;
    type Args = CapeArgs;

    fn init_backend(
        univ_param: &'a UniversalParam,
        _args: Self::Args,
        loader: &mut impl WalletLoader<CapeLedger, Meta = LoaderMetadata>,
    ) -> Result<Self::Backend, WalletError<CapeLedger>> {
        let verif_crs = VerifierKeySet {
            mint: TransactionVerifyingKey::Mint(
                jf_cap::proof::mint::preprocess(&*univ_param, CapeLedger::merkle_height())
                    .unwrap()
                    .1,
            ),
            xfr: KeySet::new(
                vec![TransactionVerifyingKey::Transfer(
                    jf_cap::proof::transfer::preprocess(
                        &*univ_param,
                        3,
                        3,
                        CapeLedger::merkle_height(),
                    )
                    .unwrap()
                    .1,
                )]
                .into_iter(),
            )
            .unwrap(),
            freeze: KeySet::new(
                vec![TransactionVerifyingKey::Freeze(
                    jf_cap::proof::freeze::preprocess(&*univ_param, 2, CapeLedger::merkle_height())
                        .unwrap()
                        .1,
                )]
                .into_iter(),
            )
            .unwrap(),
        };
        let ledger = Arc::new(Mutex::new(MockLedger::new(MockCapeNetwork::new(
            verif_crs,
            MerkleTree::new(CapeLedger::merkle_height()).unwrap(),
            vec![],
        ))));
        MockCapeBackend::new(ledger, loader)
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

type CapeWallet<'a> = seahorse::Wallet<'a, MockCapeBackend<'a, LoaderMetadata>, CapeLedger>;

#[allow(clippy::too_many_arguments)]
async fn cli_sponsor<'a>(
    io: &mut SharedIO,
    wallet: &mut CapeWallet<'_>,
    erc20_code: Erc20Code,
    sponsor_addr: EthereumAddr,
    auditor: Option<AuditorPubKey>,
    freezer: Option<FreezerPubKey>,
    trace_amount: Option<bool>,
    trace_address: Option<bool>,
    trace_blind: Option<bool>,
    reveal_threshold: Option<u64>,
) {
    let mut policy = AssetPolicy::default();
    if let Some(auditor) = auditor {
        policy = policy.set_auditor_pub_key(auditor);
    }
    if let Some(freezer) = freezer {
        policy = policy.set_freezer_pub_key(freezer);
    }
    if Some(true) == trace_amount {
        policy = match policy.reveal_amount() {
            Ok(policy) => policy,
            Err(err) => {
                cli_writeln!(io, "Invalid policy: {}", err);
                return;
            }
        }
    }
    if Some(true) == trace_address {
        policy = match policy.reveal_user_address() {
            Ok(policy) => policy,
            Err(err) => {
                cli_writeln!(io, "Invalid policy: {}", err);
                return;
            }
        }
    }
    if Some(true) == trace_blind {
        policy = match policy.reveal_blinding_factor() {
            Ok(policy) => policy,
            Err(err) => {
                cli_writeln!(io, "Invalid policy: {}", err);
                return;
            }
        }
    }
    if let Some(reveal_threshold) = reveal_threshold {
        policy = policy.set_reveal_threshold(reveal_threshold);
    }
    match wallet.sponsor(erc20_code, sponsor_addr, policy).await {
        Ok(def) => {
            cli_writeln!(io, "{}", def);
        }
        Err(err) => {
            cli_writeln!(io, "{}\nAsset was not sponsored.", err);
        }
    }
}

async fn cli_wrap<'a>(
    io: &mut SharedIO,
    wallet: &mut CapeWallet<'_>,
    asset_def: AssetDefinition,
    from: EthereumAddr,
    to: UserAddress,
    amount: u64,
) {
    match wallet.wrap(from, asset_def.clone(), to.0, amount).await {
        Ok(()) => {
            cli_writeln!(io, "\nAsset wrapped: {}", asset_def.code);
        }
        Err(err) => {
            cli_writeln!(io, "{}\nAsset was not wrapped.", err);
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn cli_burn<'a>(
    io: &mut SharedIO,
    wallet: &mut CapeWallet<'_>,
    asset: ListItem<AssetCode>,
    from: UserAddress,
    to: EthereumAddr,
    amount: u64,
    fee: u64,
    wait: Option<bool>,
) {
    let res = wallet.burn(&from.0, to, &asset.item, amount, fee).await;
    cli_writeln!(io, "{}", asset.item);

    finish_transaction::<CapeCli>(io, wallet, res, wait, "burned").await;
}

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
             auditor: Option<AuditorPubKey>,
             freezer: Option<FreezerPubKey>,
             trace_amount: Option<bool>,
             trace_address: Option<bool>,
             trace_blind: Option<bool>,
             reveal_threshold: Option<u64>| {
                cli_sponsor(io, wallet, erc20_code, sponsor_addr, auditor, freezer, trace_amount, trace_address, trace_blind, reveal_threshold).await;
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
                cli_wrap(io, wallet, asset_def, from, to, amount).await;
            }
        ),
        command!(
            burn,
            "burn an asset",
            CapeCli,
            |io,
             wallet,
             asset: ListItem<AssetCode>,
             from: UserAddress,
             to: EthereumAddr,
             amount: u64,
             fee: u64;
             wait: Option<bool>| {
                cli_burn(io, wallet, asset, from, to, amount, fee, wait).await;
            }
        ),
    ]
}

#[derive(StructOpt)]
pub struct CapeArgs {
    /// Generate keys for a wallet, do not run the REPL.
    ///
    /// The keys are stored in FILE and FILE.pub.
    #[structopt(short = "g", long)]
    pub key_gen: Option<PathBuf>,

    /// Path to a saved wallet, or a new directory where this wallet will be saved.
    ///
    /// If not given, the wallet will be stored in ~/.translucence/wallet. If a wallet already
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
    tracing_subscriber::fmt().pretty().init();

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
        mocks::{CapeTest, MockCapeLedger},
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
                     auditor: Option<AuditorPubKey>,
                     freezer: Option<FreezerPubKey>,
                     trace_amount: Option<bool>,
                     trace_address: Option<bool>,
                     trace_blind: Option<bool>,
                     reveal_threshold: Option<u64>| {
                        cli_sponsor(io, wallet, erc20_code, sponsor_addr, auditor, freezer, trace_amount, trace_address, trace_blind, reveal_threshold).await;
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
                        cli_wrap(io, wallet, asset_def, from, to, amount).await;
                    }
                ),
                command!(
                    burn,
                    "burn an asset",
                    Self,
                    |io,
                     wallet,
                     asset: ListItem<AssetCode>,
                     from: UserAddress,
                     to: EthereumAddr,
                     amount: u64,
                     fee: u64;
                     wait: Option<bool>| {
                        cli_burn(io, wallet, asset, from, to, amount, fee, wait).await;
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
            .command(wallet, format!("load_key spend {}", key_path))?
            .output(format!("(?P<default_addr{}>ADDR~.*)", wallet))
    }

    fn cli_sponsor_all_args(t: &mut CliClient, sponsor_addr: &EthereumAddr) -> Result<(), String> {
        // Set an ERC 20 code to sponsor.
        let erc20_code = Erc20Code(EthereumAddr([1u8; 20]));

        t
            // Generate freezer and auditor keys.
            .command(0, "gen_key freeze")?
            .output("(?P<freezer>FREEZEPUBKEY~.*)")?
            .command(0, "gen_key audit")?
            .output("(?P<auditor>AUDPUBKEY~.*)")?
            // Sponsor an asset with all policy attributes specified.
            .command(0, format!("sponsor {} {} auditor=$auditor freezer=$freezer trace_amount=true trace_address=true trace_blind=true reveal_threshold=10", erc20_code, sponsor_addr))?
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
            // Sponsor a non-auditable asset with a freezer key.
            .command(0, "gen_key freeze")?
            .output("(?P<freezer>FREEZEPUBKEY~.*)")?
            .command(0, format!("sponsor {} {} freezer=$freezer", erc20_code, sponsor_addr))?
            .output(format!("(?P<asset_non_auditable>ASSET_DEF~.*)"))?
            // Sponsor an auditable asset without a freezer key.
            .command(0, "gen_key audit")?
            .output("(?P<auditor>AUDPUBKEY~.*)")?
            .command(0, format!("sponsor {} {} auditor=$auditor trace_amount=true trace_address=true trace_blind=true reveal_threshold=10", erc20_code, sponsor_addr))?
            .output(format!("(?P<asset_auditable>ASSET_DEF~.*)"))?
            // Should fail to sponsor an auditable asset without a given auditor key.
            .command(0, format!("sponsor {} {} trace_amount=true trace_address=true trace_blind=true reveal_threshold=10", erc20_code, sponsor_addr))?
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
        t.command(0, format!("burn 0 $default_addr0 {} 10 1", erc20_addr))?
            .output(format!("TransactionError: InsufficientBalance"))?;
        Ok(())
    }

    // TODO !keyao Replace the use of `CliClient` with `CapeTest` and CLI matching helpers in
    // Seahorse, similar to `test_cli_burn`.
    // Related issue: https://github.com/SpectrumXYZ/cape/issues/477.
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

    // TODO !keyao Replace the use of `CliClient` with `CapeTest` and CLI matching helpers in
    // Seahorse, similar to `test_cli_burn`.
    // Related issue: https://github.com/SpectrumXYZ/cape/issues/477.
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
        let (ledger, key_streams) = create_cape_network(&mut t, &[1000, 1000, 1000]).await;

        // Create wallets for sponsor, wrapper and receiver.
        let (mut sponsor_input, mut sponsor_output) =
            create_cape_wallet(ledger.clone(), key_streams[0].clone());
        let (mut wrapper_input, mut wrapper_output) =
            create_cape_wallet(ledger.clone(), key_streams[1].clone());
        let (mut receiver_input, mut receiver_output) =
            create_cape_wallet(ledger.clone(), key_streams[2].clone());

        // Get the freezer and auditor keys for the sponsor, and the receiver's addresses.
        writeln!(sponsor_input, "gen_key freeze").unwrap();
        let freezer_key =
            match_output(&mut sponsor_output, &["(?P<freezekey>FREEZEPUBKEY~.*)"]).get("freezekey");
        writeln!(sponsor_input, "gen_key audit").unwrap();
        let auditor_key =
            match_output(&mut sponsor_output, &["(?P<audkey>AUDPUBKEY~.*)"]).get("audkey");
        writeln!(receiver_input, "gen_key spend scan_from=start wait=true").unwrap();
        let receiver_addr = match_output(&mut receiver_output, &["(?P<addr>ADDR~.*)"]).get("addr");
        writeln!(receiver_input, "balance 0").unwrap();
        match_output(&mut receiver_output, &[format!("{} 1000", receiver_addr)]);

        // Sponsor and wrap an asset.
        let erc20_code = Erc20Code(EthereumAddr([1u8; 20]));
        let sponsor_eth_addr = EthereumAddr([2u8; 20]);
        writeln!(sponsor_input, "sponsor {} {} freezer={} auditor={} trace_amount=true trace_address=true trace_blind=true reveal_threshold=10", erc20_code, sponsor_eth_addr, freezer_key, auditor_key).unwrap();
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
        writeln!(receiver_input, "issue my_asset").unwrap();
        wait_for_prompt(&mut receiver_output);
        let mint_amount = 20;
        writeln!(
            receiver_input,
            "mint 1 {} {} {} 1",
            receiver_addr, receiver_addr, mint_amount
        )
        .unwrap();
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
            "burn {} {} {} {} 1",
            wrapped_asset, receiver_addr, wrapper_eth_addr, wrap_amount
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
