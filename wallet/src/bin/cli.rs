////////////////////////////////////////////////////////////////////////////////
// The CAPE Wallet Frontend
//
// For now, this "frontend" is simply a comand-line read-eval-print loop which
// allows the user to enter commands for a wallet interactively.
//

extern crate cape_wallet;
use async_std::sync::Mutex;
use cap_rust_sandbox::{
    ledger::CapeLedger,
    state::{Erc20Code, EthereumAddr},
};
use cape_wallet::{
    mocks::{CapeTest, MockCapeBackend, MockCapeLedger},
    wallet::CapeWalletExt,
};
use jf_cap::{
    keys::{AuditorPubKey, FreezerPubKey},
    proof::UniversalParam,
    structs::{AssetCode, AssetDefinition, AssetPolicy},
};
use net::UserAddress;
use seahorse::{
    cli::*,
    hd,
    io::SharedIO,
    loader::{LoadMethod, LoaderMetadata, WalletLoader},
    persistence::AtomicWalletStorage,
    testing::SystemUnderTest,
    WalletBackend, WalletError,
};
use std::any::type_name;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

pub struct CapeCli;

impl<'a> CLI<'a> for CapeCli {
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
                match wallet.wrap(from, asset_def.clone(), to.0, amount).await {
                    Ok(()) => {
                        cli_writeln!(io, "\nAsset wrapped: {}", asset_def.code);
                    }
                    Err(err) => {
                        cli_writeln!(io, "{}\nAsset was not wrapped.", err);
                    }
                }
            }
        ),
        command!(
            burn,
            "burn an asset",
            CapeCli,
            |io,
             wallet,
             asset_code: AssetCode,
             from: UserAddress,
             to: EthereumAddr,
             amount: u64,
             fee: u64;
             wait: Option<bool>| {
                let res = wallet
                    .burn(&from.0, to, &asset_code, amount, fee)
                    .await;
                    cli_writeln!(io, "{}", asset_code);

                finish_transaction::<CapeCli>(io, wallet, res, wait, "burned").await;
            }
        ),
    ]
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

    fn encrypted(&self) -> bool {
        true
    }

    fn load_method(&self) -> LoadMethod {
        LoadMethod::Mnemonic
    }

    fn use_tmp_storage(&self) -> bool {
        true
    }
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt().pretty().init();

    let (io, _, _) = SharedIO::pipe();

    let mut t = CapeTest::default();
    let (ledger, wallets) = t
        .create_test_network(&[(2, 2)], vec![1000], &mut Instant::now())
        .await;

    // Set `block_size` to `1` so we don't have to explicitly flush the ledger after each
    // transaction submission.
    ledger.lock().await.set_block_size(1).unwrap();

    // We don't actually care about the open wallet returned by `create_test_network`, because
    // the CLI does its own wallet loading. But we do want to get its key stream, so that the
    // wallet we create through the CLI can deterministically generate the key that own the
    // initial record.
    let key_stream = wallets[0].0.lock().await.backend().key_stream();

    // Initialize the wallet CLI.
    let args = MockCapeArgs {
        io,
        key_stream,
        ledger,
    };
    if let Err(err) = cli_main::<CapeLedger, CapeCli>(args).await {
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
    use cape_wallet::{cli_client::CliClient, mocks::MockCapeLedger};
    use futures::stream::{iter, StreamExt};
    use pipe::{PipeReader, PipeWriter};
    use seahorse::{hd, io::Tee, testing::cli_match::*};

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
            cli_main::<CapeLedger, CapeCli>(args).await.unwrap();
        });

        // Wait for the CLI to start up and then return the input and output pipes.
        let input = Tee::new(input);
        let mut output = Tee::new(output);
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
    // Seahorse, simiar to `test_cli_burn`.
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
    // Seahorse, simiar to `test_cli_burn`.
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
        writeln!(sponsor_input, "1").unwrap();
        wait_for_prompt(&mut sponsor_output);
        let (mut wrapper_input, mut wrapper_output) =
            create_cape_wallet(ledger.clone(), key_streams[1].clone());
        writeln!(wrapper_input, "1").unwrap();
        wait_for_prompt(&mut wrapper_output);
        let (mut receiver_input, mut receiver_output) =
            create_cape_wallet(ledger.clone(), key_streams[2].clone());
        writeln!(receiver_input, "1").unwrap();
        wait_for_prompt(&mut receiver_output);

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
        let amount = 10;
        writeln!(
            wrapper_input,
            "wrap {} {} {} {}",
            asset_def, wrapper_eth_addr, receiver_addr, amount
        )
        .unwrap();
        let asset_code = match_output(
            &mut wrapper_output,
            &["Asset wrapped: (?P<asset_code>ASSET_CODE~.*)"],
        )
        .get("asset_code");

        // Submit a dummy transaction to finalize the wrap.
        writeln!(receiver_input, "issue my_asset").unwrap();
        wait_for_prompt(&mut receiver_output);
        writeln!(
            receiver_input,
            "mint 1 {} {} 20 1",
            receiver_addr, receiver_addr
        )
        .unwrap();
        wait_for_prompt(&mut receiver_output);

        // Burn the sponsored asset.
        writeln!(
            receiver_input,
            "burn {} {} {} {} 1",
            asset_code, receiver_addr, wrapper_eth_addr, amount
        )
        .unwrap();
        match_output(&mut receiver_output, &["(?P<txn>TXN~.*)"]);
    }
}
