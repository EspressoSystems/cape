# CAP on Ethereum

<!-- run `md-toc` inside the nix-shell to generate the table of contents -->

**Table of Contents**

- [CAP on Ethereum](#cap-on-ethereum)
  - [Obtaining the source code](#obtaining-the-source-code)
- [Environment](#environment)
  - [1. Install nix](#1-install-nix)
  - [2. Activate the nix environment](#2-activate-the-nix-environment)
  - [3. Verify installation](#3-verify-installation)
  - [4. direnv (Optional, but recommended for development)](#4-direnv-optional-but-recommended-for-development)
- [Build](#build)
- [Develop](#develop)
- [Test](#test)
  - [Testing against go-ethereum node](#testing-against-go-ethereum-node)
  - [Testing against hardhat node](#testing-against-hardhat-node)
    - [Separate hardhat node](#separate-hardhat-node)
    - [Hardhat node integrated in test command](#hardhat-node-integrated-in-test-command)
  - [Deployment](#deployment)
    - [Linking to deployed contracts](#linking-to-deployed-contracts)
  - [Precompiled solidity binaries](#precompiled-solidity-binaries)
    - [Details about solidity compiler (solc) management](#details-about-solidity-compiler-solc-management)
  - [Ethereum contracts](#ethereum-contracts)
  - [Rust](#rust)
  - [Linting & Formatting](#linting--formatting)
  - [Updating dependencies](#updating-dependencies)
  - [Alternative nix installation methods](#alternative-nix-installation-methods)
    - [Nix on debian/ubuntu](#nix-on-debianubuntu)
  - [Git hooks](#git-hooks)
  - [Ethereum key management](#ethereum-key-management)
  - [Python tools](#python-tools)
- [Rinkeby](#rinkeby)
- [Goerli](#goerli)
- [Gas Usage](#gas-usage)
  - [Gas Reporter](#gas-reporter)
  - [Gas usage of block submissions](#gas-usage-of-block-submissions)
- [CI](#ci)
  - [Nightly CI builds](#nightly-ci-builds)
- [Documentation](#documentation)
  - [CAP protocol specification](#cap-protocol-specification)
  - [CAPE Contract specification](#cape-contract-specification)

## Obtaining the source code

    git clone git@github.com:SpectrumXYZ/cape.git

# Environment

This project has a lot of dependencies. The only tested installation method is
via the [nix](https://nixos.org) package manager.

You also need access to the following currently private git repos

- https://github.com/SpectrumXYZ/arbitrary-wrappers
- https://github.com/SpectrumXYZ/atomic-store
- https://github.com/SpectrumXYZ/cap
- https://github.com/SpectrumXYZ/commit
- https://github.com/SpectrumXYZ/curves
- https://github.com/SpectrumXYZ/jellyfish-cap
- https://github.com/SpectrumXYZ/key-set
- https://github.com/SpectrumXYZ/net
- https://github.com/SpectrumXYZ/reef
- https://github.com/SpectrumXYZ/seahorse
- https://github.com/SpectrumXYZ/universal-params
- https://github.com/SpectrumXYZ/zerok-macros

Ping Mat for access.

## 1. Install nix

Installation instructions can be found [here](https://nixos.org/download.html).
If in a rush, running the following command and following the on-screen
instructions should work in most cases

    curl -L https://nixos.org/nix/install | sh

Some linux distros (ubuntu, arch, ...) have packaged `nix`. See the section
[Alternative nix installation methods](#alternative-nix-installation-methods)
for more information.

## 2. Activate the nix environment

To activate a shell with the development environment run

    nix-shell

from within the top-level directory of the repo.

Note: for the remainder of this README it is necessary that this environment is
active.

Once the `nix-shell` is activated the dependencies as well as the scripts in the
`./bin` directory will be in the `PATH`.

## 3. Verify installation

Try running some tests to verify the installation

    cape-test-geth

If this fails with errors that don't point to obvious problems please open an
issue on github. M1 Macs need to have node@16 installed to avoid memory allocation errors.

Note that these tests use `cargo test --release` which is slower for compiling but then faster for executing.

## 4. direnv (Optional, but recommended for development)

To avoid manually activating the nix shell each time the
[direnv](https://direnv.net/) shell extension can be used to activate the
environment when entering the local directory of this repo. Note that direnv
needs to be [hooked](https://direnv.net/docs/hook.html) into the shell to
function.

To enable `direnv` run

    direnv allow

from the root directory of this repo.

When developing `nix` related code it can sometimes be handy to take direnv out
of the equation: to temporarily disable `direnv` and manually enter a nix shell
run

    direnv deny
    nix-shell

# Build

To build the project run

    cargo build --release

The `--release` flag is recommended because without it many cryptographic
computations the project relies one become unbearably slow.

# Develop

To start the background services to support interactive development run the command

    hivemind

For the time being this is a `geth` node and a contract compilation watcher.

To add additional processes add lines to `Procfile` and (if desired) scripts to
run in the `./bin` directory.

# Test

An running ethereum node is needed to run the tests. We support running against
a go-ethereum (`geth`) or hardhat node running on `localhost:8545`.

The simplest way to run all the tests against a nodes is to use the scripts

    cape-test-geth
    cape-test-hardhat

These scripts will

1. Start a corresponding node if nothing is found to be listening on the
   configured port (default: `8545`).
2. Run the tests.
3. Shut down the node (if it was started in 1.)

Note that running `js` tests against the `hardhat node` may take several
minutes.

The port of the node can be changed with `RPC_PORT`. For example,

    env RPC_PORT=8877 cape-test-geth

To run all the tests against both nodes

    cape-test-all

## Testing against go-ethereum node

Start the geth chain in separate terminal

    run-geth

When running tests against geth

- Tests run faster than with the hardhat node.
- The `console.log` statements in solidity **do nothing** (except consume a tiny amount of gas (?)).
- Failing `require` statements are shown in the`geth` console log.

The genesis block is generated with the python script `bin/make-genesis-block`.

If time permits replacing the `run-geth` bash script with a python script that
uses `make-genesis-block` and `hdwallet-derive` could be useful.

Note: when making calls (not transactions) to the go-ethereum node,
`msg.sender` is the zero address.

## Testing against hardhat node

You can choose to let hardhat start a hardhat node automatically or start a node
yourself and let hardhat connect to it.

Note: when making calls (not transactions) to the hardhat node, `msg.sender` is
set by the hardhat node to be the first address in `hardhat accounts`. Since
this is differnt from the behaviour we see with go-ethereum this can lead to
confusion for example when switching from go-ethereum to hardhat to debug.

### Separate hardhat node

Start the hardhat node in separate terminal

    hardhat node --network hardhat

When running tests against this hardhat node

- Tests are slow.
- The `console.log` statements in solidity show in terminal running the node.
- Failing `require` statements are shown in human readable form in the terminal running the node.

### Hardhat node integrated in test command

It's also possible to run the hardhat node and tests in one command

    hardhat --network hardhat test

- Tests are slow.
- The `console.log` statements are shown in in the terminal.
- Failing `require` statements are shown in human readable form in the terminal.

## Deployment

The CAPE contract can be deployed with

    hardhat deploy

The deployments are saved in `contracts/deployments`. If you deploy to localhost
you have to use `hardhat deploy --reset` after you restart the geth node in
order to re-deploy.

### Linking to deployed contracts

In order to avoid re-deploying the library contracts for each test you can pass
the address obtained by running `hardhat deploy` as an env var

    env RESCUE_LIB_ADDRESS=0x5FbDB2315678afecb367f032d93F642f64180aa3 cargo test --release

## Precompiled solidity binaries

Hardhat is configured to use the solc binary installed with nix (see
[nix/solc-bin/default.nix](nix/solc-bin/default.nix)) if matches the version
number. If hardhat downloads and uses another binary a warning is printed the
console.

### Details about solidity compiler (solc) management

The binaries used by hardhat, brownie, solc-select, ... from
https://solc-bin.ethereum.org/linux-amd64/list.json underwent a change in build
process from v0.7.5 to v0.7.6 ([this
commit](https://github.com/ethereum/solidity/commit/7308abc08475869cf7bc6a0654acb9d45bafc52a)).

The new solc binaries either fail to run or depend on files that aren't provide by nix.

Hardhat always "works" because it falls back to solcjs silently (unless running with `--verbose`)

    $ hardhat compile --verbose
    ...
    hardhat:core:tasks:compile Native solc binary doesn't work, using solcjs instead +8ms
    ...

The solcjs compiler is a lot slower than native solc and brownie (using py-solc-x),
solc-select do not have such a fallback mechanisms.

## Ethereum contracts

To compile the contracts to extract the abi for the ethers abigen macro
run the following command from the root of the `cape` repo checkout:

    build-abi

Note: structs will only be included in the ABI if there is a public function
that uses them.

To have rust typings, add the contract to the `abigen!` macro call in
`./contracts/rust/src/types.rs`.

Instead of running `geth` and `build-abi` one can also just run

    hivemind

From the root directory of the repo checkout. This will also recompile the
contracts when there are changes to any of the contract files.

To recompile all contracts

    hardhat compile --force

When removing or renaming contracts it can be useful to first remove the
artifacts directory and the compilation cache with

    hardhat clean

## Rust

To run the rust tests

    cargo test

Note that this requires compiled solidity contracts and a running geth node. For
development it's convenient to keep `hivemind` running in a separate terminal
for that purpose.

To connect to various chains use `RPC_URL` and `MNEMONIC` env vars. For example

    env MNEMONIC="$RINKEBY_MNEMONIC" RPC_URL=$RINKEBY_URL cargo test

To watch the rust files and compile on changes

    cargo watch

The command (`check` by default) can be changed with `-x` (for example `cargo watch -x test`).

## Linting & Formatting

Lint the code using all formatters and linters in one shot

    lint-fix

## Updating dependencies

Run `nix flake update` if you would like to pin other version edit `flake.nix`
beforehand. Commit the lock file when happy.

To update only a single input specify it as argument, for example

    nix flake update github:oxalica/rust-overlay

To make use of newly released `solc` version run

    cd nix/solc-bin
    ./update-sources.sh

## Alternative nix installation methods

### Nix on debian/ubuntu

#### Installation

To install and setup `nix` on debian/ubuntu using [their nix
package](https://packages.debian.org/sid/nix-setup-systemd). The steps below
were tested on ubuntu 20.10.

    sudo apt install nix
    sudo usermod -a -G nix-users $USER # logout and login
    nix-channel --add https://nixos.org/channels/nixos-21.05 nixpkgs
    nix-channel --update
    source /usr/share/doc/nix-bin/examples/nix-profile.sh

The last line needs to be run once per session and is usually appended to
`.bashrc` or similar.

To test the installation, run

    $ nix-shell -p hello --run hello
    ...
    Hello, world!

#### Uninstallation

To remove `nix` (careful with the `rm` commands)

    sudo apt purge --autoremove nix-bin nix-setup-systemd
    rm -r ~/.nix-*
    sudo rm -r /nix
    # reboot machine (this step my not always be necessary)

- Remove any lines added to `.bashrc` (or other files) during installation.
- If desired remove group `nix-users` and users `nixbld*` added by nix.

## Git hooks

Pre-commit hooks are managed by nix. Edit `./nix/precommit.nix` to manage the
hooks.

## Ethereum key management

The keys are derived from the mnemonic in the `TEST_MNEMONIC` env var.

- Hardhat has builtin mnemonic support.
- For geth we start an ephemeral chain but specify a `--keystore` location and
  load the addresses into it inside the `bin/run-geth` script.
- A simple python utility to derive keys can be found in `bin/hdwallet-derive`.
  For description of the arguments run `hdwallet-derive --help`.

## Python tools

We are using `poetry` for python dependencies and `poetry2nix` to integrate them
in the nix-shell development environment.

Use any poetry command e. g. `poetry add --dev ipython` to add packages.

# Rinkeby

- Set the RINKEBY_URL in the .env file. A project can be created at
  https://infura.io/dashboard/ethereum.
- Set the RINKEBY_MNEMONIC in the .env file.
- Run the following command

To run the hardhat tests against rinkeby

    hardhat test --network rinkeby

To run an end-to-end test against rinkeby

    cape-test-rinkeby

# Goerli

- Similar to Rinkeby section (replace RINKEBY with GOERLI) and use `--network goerli`.
- Faucets: [Simple](https://goerli-faucet.slock.it),
  [Social](https://faucet.goerli.mudit.blog/).

# Gas Usage

## Gas Reporter

Set the env var `REPORT_GAS` to get extra output about the gas consumption of
contract functions called in the tests.

    env REPORT_GAS=1 hardhat test

## Gas usage of block submissions

Run

    cargo run --release --bin gas_usage

To show gas usage of sending various notes to the contract.

# CI

To locally spin up a docker container like the one used in the CI

    TODO: how to run CI locally?

## Nightly CI builds

There's a CI nightly job that runs the test suite via hardhat against the Rinkeby testnet.

2 things to note:

1. Currently the relevant contracts are deployed with each build, meaning we're not optimizing on gas costs per build.

2. The funds in the testnet wallet need to be topped off occassionally as the nightly tests consume gas over some period of time.

Based on a few successful test runs, the entire suite should consume roughly 0.079024934 gas. In case the wallet needs more funds, send more test ETH to:

```
0x2FB18F4b4519a5fc792cb6508C6505675BA659E9
```

# Documentation

## CAP protocol specification

A formal specification of the Configurable Asset Policy protocol can be found at [`./doc/cap-specification.pdf`](./doc/cap-specification.pdf)

## CAPE Contract specification

A specification of the CAPE _smart contract logic_ written in Rust can be found at `./doc/workflow/lib.rs`.

Extracting _API documentation_ from the solidity source is done using a javascript
tool called `solidity-docgen`.

To generate the documentation run

    make-doc

and observe the CLI output.
