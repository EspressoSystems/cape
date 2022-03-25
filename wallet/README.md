# Wallet

User entry point to the CAPE system. This is an instantiation of the
[Seahorse](https://github.com/EspressoSystems/seahorse) generic library framework.

There are two ways to utilize a CAPE wallet: the CLI and the web API.

## Using the CLI

### Setting up environment

Before starting the CLI, set the following environment variables.

- Smart contract
  Set the address of the CAPE smart contract using the environment variable `CAPE_CONTRACT_ADDRESS`. E.g.,

```
export CAPE_CONTRACT_ADDRESS=0x60417B5Ad8629778A46A2cAaA924D7498618622B
```

- Ethereum Query Service (EQS)

  - The default URL for the EQS is `http://localhost:50087`. To override it, use the environment variable `CAPE_EQS_URL`.
  - After the URL is set, run the EQS.

### Starting the CLI

The wallet provides a REPL-style CLI for interacting with CAPE wallets using the command line. To
start the CLI, run

```
cargo run --release --bin wallet-cli -- [options]
```

You can use `--help` to see a list of the possible values for `[options]`. A particularly useful
option is `--storage PATH`, which sets the location the wallet will use to store keystore files.
This allows you to have multiple wallets in different directories.

### Opening a wallet

When you run the CLI, you will be prompted to open a wallet. To do so, you can either create a new wallet or recover one with a mnemonic phrase.

- Creating a wallet

  - Enter `1` to accept the given phrase or `2` to generate a new one.
  - After a mnemonic phrase is accepted, follow the prompt to create a password.

- Recover a wallet
  - Enter `3` and the mnemonic phrase associated with the wallet.
  - Follow the prompts to create a new password.

### Running commands

Once you have an open wallet, you will get the REPL prompt, `>`. Now you can type `help` to view a list of commands you can execute and the arguments you need to specify.

- Transaction operations
  - `sponsor`: sponsor an asset
  - `wrap`: wrap an asset
    - Note: The `asset_def` argument must be an already-sponsored asset. To sponsor an asset, use the `sponsor` command.
  - `burn`: burn an asset
  - `transfer`: transfer some owned assets to another user
  - `transfer_from`: transfer some assets from an owned address to another user
    - Note: Unlike the `transfer` command which allocates all addresses owned by this wallet, `transfer_from` uses only the specified address, so make sure the address has sufficient balance.
  - `create_asset`: create a new asset
  - `mint`: mint an asset
    - Note: The `asset` argument must be an already-created asset. To create an asset, use the `create` command.
  - `freeze`: freeze assets owned by another users
    - Note: The `asset` argument must be a freezable asset.
  - `unfreeze`: unfreeze previously frozen assets owned by another users
  - `wait`: wait for a transaction to complete
  - `sync`: wait until the wallet has processed up to a given event index
- Information listing
  - `address`: print all public addresses of this wallet
  - `pub_key`: print all of the public keys of this wallet
  - `assets`: list assets known to the wallet
  - `asset`: print information about an asset
  - `balance`: print owned balances of asset
    - Note: It is not the balance owned by one address, but the total balance of all addresses of this wallet.
  - `transactions`: list past transactions sent and received by this wallet
  - `transaction`: print the status of a transaction
  - `keys`: list keys tracked by this wallet
  - `info`: print general information about this wallet
  - `view`: list unspent records of viewable asset types
  - `now`: print the index of the latest event processed by the wallet
- Key and record operations
  - `gen_key`: generate new keys
  - `load_key`: load a key from a file
  - `import_memo`: import an owner memo belonging to this wallet
  - `import_asset`: import an asset type

## Using the web server

```
cargo run --release --bin wallet-api -- [options]
```

You can use `--help` to see a list of the possible values for `[options]`.

Once started, the web server will serve an HTTP API at `localhost:60000` (you can override the
default port by setting the `PORT` environment variable). The endpoints are documented in
`api/api.toml`.

## Using the web server via Docker

We provide Docker containers which are built with each update of the `main` branch. These allow you
to run the web server without installing Rust or any other dependencies. To run the web server in a
Docker container, use

```
docker run -it -p 60000:60000  ghcr.io/espressosystems/cape/wallet:main
```

The `-p 60000:60000` option binds the port 60000 in the Docker container (where the web server is
hosted) to the port 60000 on the host. You can change which port on `localhost` hosts the server by
changing the first number, e.g. `-p 42000:60000`.
