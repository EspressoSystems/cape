////////////////////////////////////////////////////////////////////////////////
// The CAPE Wallet Frontend
//
// For now, this "frontend" is simply a comand-line read-eval-print loop which
// allows the user to enter commands for a wallet interactively.
//

// TODO !keyao Merge duplicate CLI code among Cape, Spectrum and Seahorse.
// Issue: https://github.com/SpectrumXYZ/cape/issues/429.

extern crate cape_wallet;
use async_std::sync::Mutex;
use cap_rust_sandbox::{
    ledger::CapeLedger,
    state::{Erc20Code, EthereumAddr},
};
use cape_wallet::{
    mocks::{MockCapeBackend, MockCapeNetwork},
    wallet::CapeWalletExt,
};
use jf_aap::{
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
    loader::{LoadMethod, LoaderMetadata, WalletLoader},
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
    type Args = Args;

    fn init_backend(
        univ_param: &'a UniversalParam,
        _args: Self::Args,
        loader: &mut impl WalletLoader<CapeLedger, Meta = LoaderMetadata>,
    ) -> Result<Self::Backend, WalletError<CapeLedger>> {
        let verif_crs = VerifierKeySet {
            mint: TransactionVerifyingKey::Mint(
                jf_aap::proof::mint::preprocess(&*univ_param, CapeLedger::merkle_height())
                    .unwrap()
                    .1,
            ),
            xfr: KeySet::new(
                vec![TransactionVerifyingKey::Transfer(
                    jf_aap::proof::transfer::preprocess(
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
                    jf_aap::proof::freeze::preprocess(&*univ_param, 2, CapeLedger::merkle_height())
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
            C,
            |wallet,
             asset_def: AssetDefinition,
             from: EthereumAddr,
             to: UserAddress,
             amount: u64| {
                match wallet.wrap(from, asset_def, to.0, amount).await {
                    Ok(()) => {
                        println!("\nAsset wrapped.");
                    }
                    Err(err) => {
                        println!("{}\nAsset was not wrapped.", err);
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
             asset: ListItem<AssetCode>,
             from: UserAddress,
             to: EthereumAddr,
             amount: u64,
             fee: u64;
             wait: Option<bool>| {
                let res = wallet
                    .burn(&from.0, to, &asset.item, amount, fee)
                    .await;
                finish_transaction::<CapeCli>(io, wallet, res, wait, "burned").await;
            }
        ),
    ]
}

#[derive(StructOpt)]
pub struct Args {
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

    /// Store the contents of the wallet in plaintext.
    ///
    /// You will not require a password to access your wallet, and your wallet will not be protected
    /// from malicious software that gains access to a device where you loaded your wallet.
    ///
    /// This option is only available when creating a new wallet. When loading an existing wallet, a
    /// password will always be required if the wallet was created without the --unencrypted flag.
    #[structopt(long)]
    pub unencrypted: bool,

    /// Load the wallet using a password and salt, rather than a mnemonic phrase.
    #[structopt(long)]
    pub password: bool,

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

impl CLIArgs for Args {
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

    fn encrypted(&self) -> bool {
        !self.unencrypted
    }

    fn load_method(&self) -> LoadMethod {
        if self.password {
            LoadMethod::Password
        } else {
            LoadMethod::Mnemonic
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
    if let Err(err) = cli_main::<CapeLedger, CapeCli>(Args::from_args()).await {
        println!("{}", err);
        exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)] // Some helper functions are only used with feature "slow-tests"

    use super::*;
    use cape_wallet::cli_client::CliClient;

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

    #[test]
    fn test_cli_sponsor() {
        cape_wallet::cli_client::cli_test(|t| {
            create_wallet(t, 0)?;

            let sponsor_addr = EthereumAddr([2u8; 20]);
            cli_sponsor_all_args(t, &sponsor_addr)?;
            cli_sponsor_skipped_args(t, &sponsor_addr)?;

            Ok(())
        });
    }

    #[test]
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

    // TODO !keyao Add a positive test.
    #[cfg(feature = "slow-tests")]
    #[test]
    fn test_cli_burn_insufficient_balance() {
        cape_wallet::cli_client::cli_test(|t| {
            create_wallet(t, 0)?;
            cli_burn_insufficient_balance(t)?;

            Ok(())
        });
    }
}
