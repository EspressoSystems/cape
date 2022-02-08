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
    wallet::{CapeWallet, CapeWalletExt},
};
use jf_aap::{
    keys::{AuditorKeyPair, AuditorPubKey, FreezerKeyPair, FreezerPubKey, UserKeyPair},
    proof::UniversalParam,
    structs::{AssetCode, AssetPolicy, ReceiverMemo, RecordCommitment},
    MerkleTree, TransactionVerifyingKey,
};
use key_set::{KeySet, VerifierKeySet};
use net::{MerklePath, UserAddress};
use reef::Ledger;
use seahorse::{
    cli::*,
    events::EventIndex,
    loader::{LoadMethod, Loader, LoaderMetadata, WalletLoader},
    reader::Reader,
    testing::MockLedger,
    txn_builder::TransactionStatus,
    WalletError,
};
use std::any::type_name;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::sync::Arc;
use structopt::StructOpt;
use tempdir::TempDir;

pub struct CapeCli;

impl<'a> CLI<'a> for CapeCli {
    type Ledger = CapeLedger;
    type Backend = MockCapeBackend<'a, LoaderMetadata>;
    type Args = Args;

    fn init_backend(
        univ_param: &'a UniversalParam,
        _args: &'a Self::Args,
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
}

type Wallet<'a> = CapeWallet<'a, MockCapeBackend<'a, LoaderMetadata>>;

trait CapeCliInput<
    'a,
    C: CLI<'a, Ledger = CapeLedger, Backend = MockCapeBackend<'a, LoaderMetadata>>,
>: Sized
{
    fn parse_for_wallet(wallet: &mut Wallet<'a>, s: &str) -> Option<Self>;
}

macro_rules! cli_input_from_str {
    ($($t:ty),*) => {
        $(
            impl<'a, C: CLI<'a, Ledger = CapeLedger, Backend= MockCapeBackend<'a, LoaderMetadata>>> CapeCliInput<'a, C> for $t {
                fn parse_for_wallet(_wallet: &mut Wallet<'a>, s: &str) -> Option<Self> {
                    Self::from_str(s).ok()
                }
            }
        )*
    }
}

cli_input_from_str! {
    bool, u64, AssetCode, AuditorPubKey, Erc20Code, EthereumAddr, EventIndex, FreezerPubKey, MerklePath, PathBuf, ReceiverMemo, RecordCommitment, String, UserAddress
}

impl<
        'a,
        C: CLI<'a, Ledger = CapeLedger, Backend = MockCapeBackend<'a, LoaderMetadata>>,
        T: Listable<'a, C> + CapeCliInput<'a, C>,
    > CapeCliInput<'a, C> for ListItem<T>
{
    fn parse_for_wallet(wallet: &mut Wallet<'a>, s: &str) -> Option<Self> {
        if let Ok(index) = usize::from_str(s) {
            // If the input looks like a list index, build the list for type T and get an element of
            // type T by indexing.
            let mut items = T::list_sync(wallet);
            if index < items.len() {
                Some(items.remove(index))
            } else {
                None
            }
        } else {
            // Otherwise, just parse a T directly.
            T::parse_for_wallet(wallet, s).map(|item| ListItem {
                item,
                index: 0,
                annotation: None,
            })
        }
    }
}

impl<'a, C: CLI<'a, Ledger = CapeLedger, Backend = MockCapeBackend<'a, LoaderMetadata>>>
    CapeCliInput<'a, C> for KeyType
{
    fn parse_for_wallet(_wallet: &mut Wallet<'a>, s: &str) -> Option<Self> {
        match s {
            "audit" => Some(Self::Audit),
            "freeze" => Some(Self::Freeze),
            "spend" => Some(Self::Spend),
            _ => None,
        }
    }
}

macro_rules! command {
    ($name:ident,
     $help:expr,
     $cli:ident,
     |$wallet:pat, $($arg:ident : $argty:ty),*
      $(; $($kwarg:ident : Option<$kwargty:ty>),*)?| $run:expr) => {
        Command {
            name: String::from(stringify!($name)),
            params: vec![$((
                String::from(stringify!($arg)),
                String::from(type_name::<$argty>()),
            )),*],
            kwargs: vec![$($((
                String::from(stringify!($kwarg)),
                String::from(type_name::<$kwargty>()),
            )),*)?],
            help: String::from($help),
            run: Box::new(|wallet, args, kwargs| Box::pin(async move {
                if args.len() != count!($($arg)*) {
                    println!("incorrect number of arguments (expected {})", count!($($arg)*));
                    return;
                }

                // For each (arg, ty) pair in the signature of the handler function, create a local
                // variable `arg: ty` by converting from the corresponding string in the `args`
                // vector. `args` will be unused if $($arg)* is empty, hence the following allows.
                #[allow(unused_mut)]
                #[allow(unused_variables)]
                let mut args = args.into_iter();
                $(
                    let $arg = match <$argty as CapeCliInput<$cli>>::parse_for_wallet(wallet, args.next().unwrap().as_str()) {
                        Some(arg) => arg,
                        None => {
                            println!(
                                "invalid value for argument {} (expected {})",
                                stringify!($arg),
                                type_name::<$argty>());
                            return;
                        }
                    };
                )*

                // For each (kwarg, ty) pair in the signature of the handler function, create a
                // local variable `kwarg: Option<ty>` by converting the value associated with
                // `kwarg` in `kwargs` to tye type `ty`.
                $($(
                    let $kwarg = match kwargs.get(stringify!($kwarg)) {
                        Some(val) => match <$kwargty as CapeCliInput<$cli>>::parse_for_wallet(wallet, val) {
                            Some(arg) => Some(arg),
                            None => {
                                println!(
                                    "invalid value for argument {} (expected {})",
                                    stringify!($kwarg),
                                    type_name::<$kwargty>());
                                return;
                            }
                        }
                        None => None,
                    };
                )*)?
                // `kwargs` will be unused if there are no keyword params.
                let _ = kwargs;

                let $wallet = wallet;
                $run
            }))
        }
    };

    // Don't require a comma after $wallet if there are no additional args.
    ($name:ident, $help:expr, $cli:ident, |$wallet:pat| $run:expr) => {
        command!($name, $help, $cli, |$wallet,| $run)
    };

    // Don't require wallet at all.
    ($name:ident, $help:expr, $cli:ident, || $run:expr) => {
        command!($name, $help, $cli, |_| $run)
    };
}

macro_rules! count {
    () => (0);
    ($x:tt $($xs:tt)*) => (1 + count!($($xs)*));
}

fn init_commands<
    'a,
    C: CLI<'a, Ledger = CapeLedger, Backend = MockCapeBackend<'a, LoaderMetadata>>,
>() -> Vec<Command<'a, C>> {
    vec![
        command!(
            balance,
            "print owned balances of asset",
            C,
            |wallet, asset: ListItem<AssetCode>| {
                println!("Address Balance");
                for pub_key in wallet.pub_keys().await {
                    println!(
                        "{} {}",
                        UserAddress(pub_key.address()),
                        wallet.balance(&pub_key.address(), &asset.item).await
                    );
                }
            }
        ),
        command!(
            gen_key,
            "generate new keys",
            C,
            |wallet, key_type: KeyType; scan_from: Option<EventIndex>| {
                match key_type {
                    KeyType::Audit => match wallet.generate_audit_key().await {
                        Ok(pub_key) => println!("{}", pub_key),
                        Err(err) => println!("Error generating audit key: {}", err),
                    },
                    KeyType::Freeze => match wallet.generate_freeze_key().await {
                        Ok(pub_key) => println!("{}", pub_key),
                        Err(err) => println!("Error generating freeze key: {}", err),
                    },
                    KeyType::Spend => match wallet.generate_user_key(scan_from).await {
                        Ok(pub_key) => println!("{}", UserAddress(pub_key.address())),
                        Err(err) => println!("Error generating spending key: {}", err),
                    },
                }
            }
        ),
        command!(
            load_key,
            "load a key from a file",
            C,
            |wallet, key_type: KeyType, path: PathBuf; scan_from: Option<EventIndex>| {
                let mut file = match File::open(path.clone()) {
                    Ok(file) => file,
                    Err(err) => {
                        println!("Error opening file {:?}: {}", path, err);
                        return;
                    }
                };
                let mut bytes = Vec::new();
                if let Err(err) = file.read_to_end(&mut bytes) {
                    println!("Error reading file: {}", err);
                    return;
                }

                match key_type {
                    KeyType::Audit => match bincode::deserialize::<AuditorKeyPair>(&bytes) {
                        Ok(key) => match wallet.add_audit_key(key.clone()).await {
                            Ok(()) => println!("{}", key.pub_key()),
                            Err(err) => println!("Error saving audit key: {}", err),
                        },
                        Err(err) => {
                            println!("Error loading audit key: {}", err);
                        }
                    },
                    KeyType::Freeze => match bincode::deserialize::<FreezerKeyPair>(&bytes) {
                        Ok(key) => match wallet.add_freeze_key(key.clone()).await {
                            Ok(()) => println!("{}", key.pub_key()),
                            Err(err) => println!("Error saving freeze key: {}", err),
                        },
                        Err(err) => {
                            println!("Error loading freeze key: {}", err);
                        }
                    },
                    KeyType::Spend => match bincode::deserialize::<UserKeyPair>(&bytes) {
                        Ok(key) => match wallet.add_user_key(
                            key.clone(),
                            scan_from.unwrap_or_default(),
                        ).await {
                            Ok(()) => {
                                println!(
                                    "Note: assets belonging to this key will become available after\
                                     a scan of the ledger. This may take a long time. If you have\
                                     the owner memo for a record you want to use immediately, use\
                                     import_memo.");
                                println!("{}", UserAddress(key.address()));
                            }
                            Err(err) => println!("Error saving spending key: {}", err),
                        },
                        Err(err) => {
                            println!("Error loading spending key: {}", err);
                        }
                    },
                };
            }
        ),
        command!(
            sponsor,
            "sponsor an asset",
            C,
            |wallet,
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
                            println!("Invalid policy: {}", err);
                            return;
                        }
                    }
                }
                if Some(true) == trace_address {
                    policy = match policy.reveal_user_address() {
                        Ok(policy) => policy,
                        Err(err) => {
                            println!("Invalid policy: {}", err);
                            return;
                        }
                    }
                }
                if Some(true) == trace_blind {
                    policy = match policy.reveal_blinding_factor() {
                        Ok(policy) => policy,
                        Err(err) => {
                            println!("Invalid policy: {}", err);
                            return;
                        }
                    }
                }
                if let Some(reveal_threshold) = reveal_threshold {
                    policy = policy.set_reveal_threshold(reveal_threshold);
                }
                match wallet.sponsor(erc20_code, sponsor_addr, policy).await {
                    Ok(def) => {
                        println!("{}", def.code);
                    }
                    Err(err) => {
                        println!("{}\nAsset was not sponsored.", err);
                    }
                }
            }
        ),
        command!(
            burn,
            "burn an asset",
            C,
            |wallet, asset: ListItem<AssetCode>, from: UserAddress, to: EthereumAddr, amount: u64, fee: u64; wait: Option<bool>| {
                match wallet
                    .burn(&from.0, to, &asset.item, amount, fee)
                    .await
                {
                    Ok(receipt) => {
                        if wait == Some(true) {
                            match wallet.await_transaction(&receipt).await {
                                Err(err) => {
                                    println!("Error waiting for transaction to complete: {}", err);
                                }
                                Ok(TransactionStatus::Retired) => {},
                                _ => {
                                    println!("Transaction failed");
                                }
                            }
                        } else {
                            println!("Transaction {}", receipt);
                        }
                    }
                    Err(err) => {
                        println!("{}\nAssets were not burned.", err);
                    }
                }
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

    fn interactive(&self) -> bool {
        !self.non_interactive
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

async fn repl<
    'a,
    L: 'static + Ledger,
    C: CLI<'a, Ledger = CapeLedger, Backend = MockCapeBackend<'a, LoaderMetadata>>,
>(
    args: &'a C::Args,
) -> Result<(), WalletError<CapeLedger>> {
    let (storage, _tmp_dir) = match args.storage_path() {
        Some(storage) => (storage, None),
        None if !args.use_tmp_storage() => {
            let home = std::env::var("HOME").map_err(|_| WalletError::<CapeLedger>::Failed {
                msg: String::from(
                    "HOME directory is not set. Please set your HOME directory, or specify \
                        a different storage location using --storage.",
                ),
            })?;
            let mut dir = PathBuf::from(home);
            dir.push(".translucence/wallet");
            (dir, None)
        }
        None => {
            let tmp_dir = TempDir::new("wallet").unwrap();
            (PathBuf::from(tmp_dir.path()), Some(tmp_dir))
        }
    };

    println!(
        "Welcome to the {} wallet, version {}",
        C::Ledger::name(),
        env!("CARGO_PKG_VERSION")
    );
    println!("(c) 2021 Translucence Research, Inc.");

    let reader = Reader::new(args.interactive());
    let mut loader = Loader::new(args.load_method(), args.encrypted(), storage, reader);
    let universal_param = Box::leak(Box::new(universal_param::get(
        &mut loader.rng,
        L::merkle_height(),
    )));
    let backend = C::init_backend(universal_param, args, &mut loader)?;

    // Loading the wallet takes a while. Let the user know that's expected.
    //todo !jeb.bearer Make it faster
    println!("connecting...");
    let mut wallet = Wallet::new(backend).await?;
    println!("Type 'help' for a list of commands.");
    let commands = init_commands::<C>();

    let mut input = Reader::new(args.interactive());
    'repl: while let Some(line) = input.read_line() {
        let tokens = line.split_whitespace().collect::<Vec<_>>();
        if tokens.is_empty() {
            continue;
        }
        if tokens[0] == "help" {
            for command in commands.iter() {
                println!("{}", command);
            }
            continue;
        }
        for Command { name, run, .. } in commands.iter() {
            if name == tokens[0] {
                let mut args = Vec::new();
                let mut kwargs = HashMap::new();
                for tok in tokens.into_iter().skip(1) {
                    if let Some((key, value)) = tok.split_once("=") {
                        kwargs.insert(String::from(key), String::from(value));
                    } else {
                        args.push(String::from(tok));
                    }
                }
                run(&mut wallet, args, kwargs).await;
                continue 'repl;
            }
        }
        println!("Unknown command. Type 'help' for a list of valid commands.");
    }

    Ok(())
}

async fn cape_cli_main<
    'a,
    L: 'static + Ledger,
    C: CLI<'a, Ledger = CapeLedger, Backend = MockCapeBackend<'a, LoaderMetadata>>,
>(
    args: &'a C::Args,
) -> Result<(), WalletError<CapeLedger>> {
    if let Some(path) = args.key_gen_path() {
        key_gen::<C>(path)
    } else {
        repl::<L, C>(args).await
    }
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt().pretty().init();

    // Initialize the wallet CLI.
    if let Err(err) = cape_cli_main::<CapeLedger, CapeCli>(&Args::from_args()).await {
        println!("{}", err);
        exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
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

    fn cli_sponsor(t: &mut CliClient) -> Result<(), String> {
        // Set ERC 20 code and sponsor address.
        let erc20_code = Erc20Code(EthereumAddr([1u8; 20]));
        let sponsor_addr = EthereumAddr([2u8; 20]);

        t
            // Sponsor an asset with the default policy.
            .command(0, format!("sponsor {} {}", erc20_code, sponsor_addr))?
            .output(format!("(?P<asset_default>ASSET_CODE~.*)"))?
            // Sponsor a non-auditable asset with a freezer key.
            .command(0, "gen_key freeze")?
            .output("(?P<freezer>FREEZEPUBKEY~.*)")?
            .command(0, format!("sponsor {} {} freezer=$freezer", erc20_code, sponsor_addr))?
            .output(format!("(?P<asset_non_auditable>ASSET_CODE~.*)"))?
            // Sponsor an auditable asset without a freezer key.
            .command(0, "gen_key audit")?
            .output("(?P<auditor>AUDPUBKEY~.*)")?
            .command(0, format!("sponsor {} {} auditor=$auditor trace_amount=true trace_address=true trace_blind=true reveal_threshold=10", erc20_code, sponsor_addr))?
            .output(format!("(?P<asset_auditable>ASSET_CODE~.*)"))?
            // Sponsor an asset with all policy attributes specified.
            .command(0, format!("sponsor {} {} auditor=$auditor freezer=$freezer trace_amount=true trace_address=true trace_blind=true reveal_threshold=10", erc20_code, sponsor_addr))?
            .output(format!("(?P<asset_auditable>ASSET_CODE~.*)"))?
            // Should fail to sponsor an auditable asset without a given auditor key.
            .command(0, format!("sponsor {} {} trace_amount=true trace_address=true trace_blind=true reveal_threshold=10", erc20_code, sponsor_addr))?
            .output(format!("Invalid policy: Invalid parameters: Cannot reveal amount to dummy AuditorPublicKey"))?;

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
            cli_sponsor(t)?;

            Ok(())
        });
    }

    // The CAPE CLI currently doesn't support sponsor and wrap transactions, so a
    // burn transaction is expected to fail.
    #[test]
    fn test_cli_burn_insufficient_balance() {
        cape_wallet::cli_client::cli_test(|t| {
            create_wallet(t, 0)?;
            cli_burn_insufficient_balance(t)?;

            Ok(())
        });
    }
}
