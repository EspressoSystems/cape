#![allow(dead_code)]

// use crate::Wallet;
use async_std::task::block_on;
use async_std::task::spawn_blocking;
use escargot::CargoBuild;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use tempdir::TempDir;
use toml::Value;

/// Set up and run a test of the wallet CLI.
///
/// This function initializes a [CliClient] for a new network of wallets and passes it to the test
/// function. The result is converted to an error as if by unwrapping.
///
/// It is important that CLI tests fail by returning an [Err] [Result], rather than by panicking,
/// because panicking while borrowing from a [CliClient] can prevent the [CliClient] destructor from
/// running, which can leak long-lived processes. This function will ensure the [CliClient] is
/// dropped before it panics.
pub fn cli_test(test: impl Fn(&mut CliClient) -> Result<(), String>) {
    if let Err(msg) = test(&mut CliClient::new().unwrap()) {
        panic!("{}", msg);
    }
}

pub struct CliClient {
    validators: Vec<Validator>,
    wallets: Vec<Wallet>,
    variables: HashMap<String, String>,
    prev_output: Vec<String>,
    server_port: u64,
    _tmp_dir: TempDir,
}

impl CliClient {
    pub fn new() -> Result<Self, String> {
        // Generate keys for the primary wallet.
        let tmp_dir = TempDir::new("test_wallet_cli").map_err(err)?;
        let mut key_path = PathBuf::from(tmp_dir.path());
        key_path.push("primary_key");
        Wallet::key_gen(&key_path)?;

        // Each validator gets two ports: one for its PhaseLock node and one for the web sever.
        let mut ports = [(0, 0); 6];
        for p in &mut ports {
            *p = (get_port(), get_port());
        }

        let mut state = Self {
            wallets: Default::default(),
            variables: Default::default(),
            prev_output: Default::default(),
            validators: Self::start_validators(tmp_dir.path(), &key_path, &ports)?,
            server_port: ports[0].1,
            _tmp_dir: tmp_dir,
        };
        state.load(Some(key_path))?;
        Ok(state)
    }

    pub fn open(&mut self, wallet: usize) -> Result<&mut Self, String> {
        self.open_with_args(wallet, [""; 0])
    }

    pub fn open_with_args<I, S>(&mut self, wallet: usize, args: I) -> Result<&mut Self, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        while wallet >= self.wallets.len() {
            self.load(None)?;
        }
        self.prev_output = self.wallets[wallet].open(args)?;
        Ok(self)
    }

    pub fn close(&mut self, wallet: usize) -> Result<&mut Self, String> {
        if let Some(wallet) = self.wallets.get_mut(wallet) {
            wallet.close();
        }
        Ok(self)
    }

    pub fn wallet_key_path(&mut self, wallet: usize) -> Result<PathBuf, String> {
        while wallet >= self.wallets.len() {
            self.load(None)?;
        }
        Ok(self.wallets[wallet].key_path.clone())
    }

    pub fn open_validator(&mut self, v: usize) -> Result<&mut Self, String> {
        block_on(
            self.validators
                .get_mut(v)
                .ok_or_else(|| format!("no such validator {}", v))?
                .open(),
        )?;
        Ok(self)
    }

    pub fn close_validator(&mut self, v: usize) -> Result<&mut Self, String> {
        self.validators
            .get_mut(v)
            .ok_or_else(|| format!("no such validator {}", v))?
            .close();
        Ok(self)
    }

    /// Issue a command to the wallet identified by `wallet`.
    ///
    /// The command string will be preprocessed by replacing each occurrence of `$var` in the
    /// command with the value of the variable `var`. See [output] for how variables can be bound to
    /// values using named capture groups in regexes.
    ///
    /// If `wallet` refers to a wallet that has not yet been created, a new one will be created. The
    /// [TestState] always starts off with one wallet, index 0, which gets an initial grant of 2^32
    /// native tokens. So `command(0, "command")` will not load a new wallet. But the first time
    /// `command(1, "command")` is called, it will block until wallet 1 is created.
    pub fn command(&mut self, id: usize, command: impl AsRef<str>) -> Result<&mut Self, String> {
        let command = self.substitute(command)?;
        let wallet = self
            .wallets
            .get_mut(id)
            .ok_or_else(|| format!("wallet {} is not open", id))?;
        println!("{}> {}", id, command);
        self.prev_output = wallet.command(&command)?;
        Ok(self)
    }

    /// Match the output of the previous command against a regex.
    ///
    /// `regex` always matches a whole line (and only a line) of output. The order of the output
    /// does not matter; `regex` will be matched against each line of output until finding one that
    /// matches.
    ///
    /// Strings matched by named captures groups in `regex` (syntax "(?P<name>exp)") will be
    /// assigned to variables based on the name of the capture group. The values of these variables
    /// can then be substituted into commands and regular expressions using `$name`.
    pub fn output(&mut self, regex: impl AsRef<str>) -> Result<&mut Self, String> {
        let regex = Regex::new(&self.substitute(regex)?).map_err(err)?;
        for line in &self.prev_output {
            if let Some(re_match) = regex.captures(line) {
                for var in regex.capture_names().flatten() {
                    if let Some(var_match) = re_match.name(var) {
                        self.variables
                            .insert(String::from(var), String::from(var_match.as_str()));
                    }
                }
                return Ok(self);
            }
        }

        return Err(format!(
            "regex \"{}\" did not match output:\n{}",
            regex,
            self.prev_output.join("\n")
        ));
    }

    pub fn last_output(&self) -> impl Iterator<Item = &String> {
        self.prev_output.iter()
    }

    pub fn var(&self, var: impl AsRef<str>) -> Result<String, String> {
        self.variables
            .get(var.as_ref())
            .cloned()
            .ok_or_else(|| format!("no such variable {}", var.as_ref()))
    }

    pub fn validators(&self) -> impl Iterator<Item = &Validator> {
        self.validators.iter()
    }

    pub fn validator(&self, validator: usize) -> Result<&Validator, String> {
        self.validators
            .get(validator)
            .ok_or_else(|| format!("no such validator {}", validator))
    }

    pub fn wallets(&self) -> impl Iterator<Item = &Wallet> {
        self.wallets.iter()
    }

    fn load(&mut self, key_path: Option<PathBuf>) -> Result<&mut Self, String> {
        self.wallets.push(Wallet::new(
            format!("http://localhost:{}", self.server_port),
            key_path,
        )?);
        Ok(self)
    }

    fn substitute(&self, string: impl AsRef<str>) -> Result<String, String> {
        let mut undefined = Vec::new();
        let replaced = Regex::new("\\$([a-zA-Z0-9_]+)").map_err(err)?.replace_all(
            string.as_ref(),
            |captures: &regex::Captures<'_>| {
                let var = captures.get(1).unwrap();
                match self.variables.get(var.as_str()) {
                    Some(val) => val.clone(),
                    None => {
                        undefined.push(String::from(var.as_str()));
                        String::new()
                    }
                }
            },
        );
        if !undefined.is_empty() {
            return Err(format!(
                "undefined variables in substitution: {}",
                undefined.join(", ")
            ));
        }
        Ok(String::from(replaced))
    }

    fn start_validators(
        tmp_dir: &Path,
        key_path: &Path,
        ports: &[(u64, u64)],
    ) -> Result<Vec<Validator>, String> {
        let (phaselock_ports, server_ports): (Vec<_>, Vec<_>) = ports.iter().cloned().unzip();
        let seed = vec![
            1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5,
            6, 7, 8,
        ];
        let nodes = Value::from(
            phaselock_ports
                .into_iter()
                .enumerate()
                .map(|(i, port)| {
                    (
                        i.to_string(),
                        Value::from(
                            vec![
                                ("ip", Value::from("localhost")),
                                ("port", Value::Integer(port as i64)),
                            ]
                            .into_iter()
                            .collect::<HashMap<_, _>>(),
                        ),
                    )
                })
                .collect::<HashMap<_, _>>(),
        );
        let config = Value::from(
            vec![
                ("title", Value::from("Node Configuration")),
                ("seed", Value::from(seed)),
                ("nodes", nodes),
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
        );
        let mut config_file = tmp_dir.to_path_buf();
        config_file.push("node-config.toml");
        File::create(&config_file)
            .unwrap()
            .write_all(config.to_string().as_bytes())
            .unwrap();

        let ret = block_on(futures::future::join_all(
            server_ports.into_iter().enumerate().map(|(i, port)| {
                let mut v = Validator::new(&config_file, key_path, i, port);
                async move {
                    v.open().await?;
                    Ok(v)
                }
            }),
        ))
        .into_iter()
        .collect::<Result<_, _>>();

        println!("All validators started");
        ret
    }
}

struct OpenWallet {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    process: Child,
}

pub struct Wallet {
    process: Option<OpenWallet>,
    key_path: PathBuf,
    storage: TempDir,
    server: String,
}

impl Wallet {
    pub fn pid(&self) -> Option<u32> {
        self.process.as_ref().map(|p| p.process.id())
    }

    pub fn storage(&self) -> PathBuf {
        PathBuf::from(self.storage.path())
    }

    pub fn server(&self) -> String {
        self.server.clone()
    }

    fn key_gen(key_path: &Path) -> Result<(), String> {
        cargo_run("zerok_client")?
            .args([
                "-g",
                key_path
                    .as_os_str()
                    .to_str()
                    .ok_or("failed to convert key path to string")?,
            ])
            .spawn()
            .map_err(err)?
            .wait()
            .map_err(err)?;
        Ok(())
    }

    fn new(server: String, key_path: Option<PathBuf>) -> Result<Self, String> {
        let storage = TempDir::new("test_wallet").map_err(err)?;
        let key_path = match key_path {
            Some(path) => path,
            None => {
                let mut path = PathBuf::from(storage.path());
                path.push("key");
                Self::key_gen(&path)?;
                path
            }
        };
        Ok(Self {
            process: None,
            key_path,
            storage,
            server,
        })
    }

    fn open<I, S>(&mut self, args: I) -> Result<Vec<String>, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        if self.process.is_some() {
            return Err(String::from("wallet is already open"));
        }
        let mut child = cargo_run("zerok_client")?
            .args([
                "--storage",
                self.storage.path().as_os_str().to_str().ok_or_else(|| {
                    format!(
                        "failed to convert storage path {:?} to string",
                        self.storage.path()
                    )
                })?,
            ])
            .arg("--non-interactive")
            .args(args)
            .arg(&self.server)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(err)?;
        let stdin = child
            .stdin
            .take()
            .ok_or("failed to open stdin for wallet")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("failed to open stdout for wallet")?;
        self.process = Some(OpenWallet {
            process: child,
            stdin,
            stdout: BufReader::new(stdout),
        });
        self.read_until_prompt()
    }

    fn close(&mut self) {
        if let Some(mut child) = self.process.take() {
            drop(child.stdin);
            child.process.wait().ok();
        }
    }

    fn command(&mut self, command: &str) -> Result<Vec<String>, String> {
        if let Some(child) = &mut self.process {
            writeln!(child.stdin, "{}", command).map_err(err)?;
            self.read_until_prompt()
        } else {
            Err(String::from("wallet is not open"))
        }
    }

    fn read_until_prompt(&mut self) -> Result<Vec<String>, String> {
        if let Some(child) = &mut self.process {
            let mut lines = Vec::new();
            let mut line = String::new();
            loop {
                child.stdout.read_line(&mut line).map_err(err)?;
                let line = std::mem::take(&mut line);
                let line = line.trim();
                println!("< {}", line);
                if line.starts_with("Error loading wallet") {
                    return Err(String::from(line));
                }
                if !line.is_empty() {
                    lines.push(String::from(line));
                }
                match line {
                    ">"
                    | "Enter password:"
                    | "Create password:"
                    | "Retype password:"
                    | "Enter mnemonic phrase:" => {
                        break;
                    }
                    _ => {}
                }
            }
            Ok(lines)
        } else {
            Err(String::from("wallet is not open"))
        }
    }
}

impl Drop for Wallet {
    fn drop(&mut self) {
        self.close();
    }
}

pub struct Validator {
    process: Option<Child>,
    id: usize,
    cfg_path: PathBuf,
    store_path: PathBuf,
    key_path: PathBuf,
    port: u64,
}

impl Validator {
    pub fn pid(&self) -> Option<u32> {
        self.process.as_ref().map(|p| p.id())
    }

    pub fn hostname(&self) -> String {
        String::from("localhost")
    }

    pub fn port(&self) -> u64 {
        self.port
    }

    fn new(cfg_path: &Path, key_path: &Path, id: usize, port: u64) -> Self {
        let cfg_path = PathBuf::from(cfg_path);
        let mut store_path = cfg_path.clone();
        store_path.pop(); // remove config toml file
        store_path.push(format!("store_for_{}", id));
        println!(
            "Launching validator with store path {}",
            store_path.as_os_str().to_str().unwrap()
        );
        let mut key_path = PathBuf::from(key_path);
        key_path.set_extension("pub");

        Self {
            process: None,
            id,
            cfg_path,
            store_path,
            key_path,
            port,
        }
    }

    async fn open(&mut self) -> Result<(), String> {
        if self.process.is_some() {
            return Err(format!("validator {} is already open", self.id));
        }

        let cfg_path = self.cfg_path.clone();
        let store_path = self.store_path.clone();
        let key_path = self.key_path.clone();
        let id = self.id;
        let port = self.port;
        let child = spawn_blocking(move || {
            let mut child = cargo_run("multi_machine")
                .map_err(err)?
                .args([
                    "--config",
                    cfg_path.as_os_str().to_str().unwrap(),
                    "--store_path",
                    store_path.as_os_str().to_str().unwrap(),
                    "--full",
                    "--id",
                    &id.to_string(),
                    "--wallet",
                    key_path.as_os_str().to_str().unwrap(),
                ])
                .env("PORT", port.to_string())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .map_err(err)?;
            let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
            while let Some(line) = lines.next() {
                let line = line.unwrap();
                println!("[id {}] Waiting for start: {}", id, line);
                if line.trim() == "- Starting consensus" {
                    async_std::task::spawn_blocking(
                        // A detached task to consume the validator's
                        // stdout. If we don't do this, the validator will
                        // eventually fill up its output pipe and block.
                        move || {
                            for line in lines {
                                if line.is_ok() {
                                    println!("[id {}]{}", id, line.unwrap());
                                } else {
                                    println!("[id {}]{:?}", id, line.err())
                                }
                            }
                        },
                    );
                    return Ok(child);
                }
            }
            Err(format!("validator {} exited", id))
        })
        .await?;

        self.process = Some(child);
        println!("Leaving Validator::new for {}", id);
        Ok(())
    }

    fn close(&mut self) {
        if let Some(mut child) = self.process.take() {
            child.kill().ok();
            child.wait().ok();
        }
    }
}

impl Drop for Validator {
    fn drop(&mut self) {
        self.close();
    }
}

fn err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

lazy_static! {
    static ref FREE_PORT: Arc<Mutex<u64>> = Arc::new(Mutex::new(
        std::env::var("PORT")
            .ok()
            .and_then(|port| port
                .parse()
                .map_err(|err| {
                    println!("PORT env var must be an integer. Falling back to 50000.");
                    err
                })
                .ok())
            .unwrap_or(50000)
    ));
}

fn get_port() -> u64 {
    let mut first_free_port = FREE_PORT.lock().unwrap();
    let port = *first_free_port;
    *first_free_port += 1;
    port
}

fn cargo_run(bin: impl AsRef<str>) -> Result<Command, String> {
    Ok(CargoBuild::new()
        .package(bin.as_ref())
        .bin(bin.as_ref())
        .current_release()
        .current_target()
        .run()
        .map_err(err)?
        .command())
}
