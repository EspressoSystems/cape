# Wallet

User entrypoint to the CAPE system. This is an instantiation of the
[Seahorse](https://github.com/EspressoSystems/seahorse) generic library framework.

There are two ways to utilize a CAPE wallet: the CLI and the web API.

## Using the CLI

Before starting the CLI, set the address of the CAPE smart contract using environment variable `CAPE_CONTRACT_ADDRESS`. E.g.,
```
export CAPE_CONTRACT_ADDRESS=0x60417B5Ad8629778A46A2cAaA924D7498618622B
```

The wallet provides a REPL-style CLI for interacting with CAPE wallets using the command line. To
start the CLI, run

```
cargo run --release --bin wallet-cli -- [options]
```

You can use `--help` to see a list of the possible values for `[options]`. A particularly useful
option is `--storage PATH`, which sets the location the wallet will use to store keystore files.
This allows you to have multiple wallets in different directories.

When you run the CLI, you will be prompted to create or open a wallet. Once you have an open wallet, you will get the REPL prompt, `>`. Now you can type `help` to view a list of commands you can execute.

### Opening a wallet
To open a wallet, you can either create a new one, or recover one with a mnemonic phrase.
* Creating a wallet
    * Enter `1` to accept the given phrase or `2` to generate a new one.
    * After a mnemonic phrase is accepted, follow the prompt to create a password.

* Recover a wallet
    * Enter `3` and the mnemonic phrase associated with the wallet.
    * Follow the prompt to create a new password.


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
