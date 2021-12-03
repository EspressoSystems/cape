# CAP on Ethereum

<!-- markdown-toc start - Don't edit this section. Run M-x markdown-toc-refresh-toc -->

**Table of Contents**

- [CAP on Ethereum](#cap-on-ethereum)
  - [Obtaining the source code](#obtaining-the-source-code)
  - [Dependencies](#dependencies)
  - [1. Install nix](#1-install-nix)
  - [2. Activate the nix environment](#2-activate-the-nix-environment)
  - [3. Verify installation](#3-verify-installation)
  - [(Optional, but recommended) direnv](#optional-but-recommended-direnv)
- [Development](#development)
  - [Testing (Javascript)](#testing-javascript)
  - [Testing against go-ethereum node](#testing-against-go-ethereum-node)
  - [Testing against hardhat node](#testing-against-hardhat-node)
    - [Separate hardhat node](#separate-hardhat-node)
    - [Hardhat node integrated in test command](#hardhat-node-integrated-in-test-command)
  - [Running scripts](#running-scripts)
  - [Precompiled solidity binaries](#precompiled-solidity-binaries)
    - [Details about solidity compiler (solc) management](#details-about-solidity-compiler-solc-management)
  - [Ethereum contracts](#ethereum-contracts)
  - [Rust](#rust)
  - [Examples](#examples)
  - [Linting & Formatting](#linting--formatting)
  - [Updating dependencies](#updating-dependencies)
  - [Alternative nix installation methods](#alternative-nix-installation-methods)
    - [Nix on debian/ubuntu](#nix-on-debianubuntu)
      - [Installation](#installation)
      - [Uninstallation](#uninstallation)
  - [Git hooks](#git-hooks)
  - [Ethereum key management](#ethereum-key-management)
  - [Python tools](#python-tools)
  - [Interacting with contracts from python](#interacting-with-contracts-from-python)
    - [eth-brownie usage](#eth-brownie-usage)
- [Benchmarks](#benchmarks)
- [Local network](#local-network)
- [Rinkeby](#rinkeby)
- [Goerli](#goerli)
- [Arbitrum on Rinkeby](#arbitrum-on-rinkeby)
- [CAP on Arbitrum (a.k.a CAPA)](#cap-on-arbitrum-aka-capa)
- [Running local arb-dev-node (not officially supported!)](#running-local-arb-dev-node-not-officially-supported)
- [Gas Reporter](#gas-reporter)
- [CI](#ci)
- [Documentation](#documentation)
  - [Ethereum Asset (Un)Wrapping Workflow](#ethereum-asset-unwrapping-workflow)

<!-- markdown-toc end -->

## Obtaining the source code

    git clone git@gitlab.com:translucence/cap-on-ethereum/cape

## Dependencies

This project has a lot of dependencies the only tested installation method is
via the [nix](https://nixos.org) package manager.

You also need access to the following git repos

- https://gitlab.com/translucence/crypto/jellyfish
- https://gitlab.com/translucence/crypto/curves
- https://gitlab.com/translucence/common/tagged-base64
- https://gitlab.com/translucence/systems/system (wallet crate)

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
issue on gitlab.

Note that these tests use `cargo test --release` which is slower for compiling but then faster for executing.

## (Optional, but recommended) direnv

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

# Development

To start the background services to support interactive development run the command

    hivemind

For the time being this is a `geth` node and a contract compilation watcher.

To add additional processes add lines to `Procfile` and (if desired) scripts to
run in the `./bin` directory.

## Testing (Javascript)

We support running against a go-ethereum (`geth`) or hardhat node running on
`localhost:8545`.

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

## Testing against hardhat node

You can choose to let hardhat start a hardhat node automatically or start a node
yourself and let hardhat connect to it.

### Separate hardhat node

Start the hardhat node in separate terminal

    hardhat node

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

## Running scripts

Run a script that connects to the local network (on port 8545)

    hardhat run scripts/benchmark.js --network localhost

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

Compile the contracts to extract the abi for the ethers abigen (workflow to be
improved!).
Run the following command from the root of the `cap-on-ethereum` checkout:

    build-abi

Note: structs will only be included in the ABI if there is a (public, I guess)
function that uses them.

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

To watch the rust files and compile on changes

    cargo watch

The command (`check` by default) can be changed with `-x` (for example `cargo watch -x test`).

## Examples

Generate a `jf_txn::transfer::TransferNote` and save it to a file `my_note.bin`.
Building with the `--release` flag make this a lot faster.

    cargo run -p cap-rust-sandbox --example create_note --release

Load the file:

    cargo run -p cap-rust-sandbox --example read_note

## Linting & Formatting

Lint the code using all formatters and linters in one shot

    lint-fix

## Updating dependencies

To update the pinned version of `nixpkgs` run `nix/update-nix`, optionally
passing a github owner and revision as arguments. The default is:
`nix/update-nix nixos master`. Make sure to commit any changed files in the
`./nix` directory afterwards.

The rust overlay can be updated by running `nix/update-rust-overlay`.

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

## Interacting with contracts from python

The ethereum development suite [eth-brownie](https://eth-brownie.readthedocs.io)
provides some interactive tools and makes it convenient to test the contracts
with python code.

### eth-brownie usage

Note: brownie currently only works with the hardhat node (but not with the geth
node). If the geth node is running it will try to connect to it and hang. If
brownie doesn't find something listening on port 8545 it will try starting a
node and connect to that instead.

Optionally start a hardhat node in a separate terminal

    hardhat node

Run an interactive console

    brownie console
    >>> dir()
    >>> help(Greeter.deploy)
    >>> contract = Greeter.deploy("Hi!", {'from': accounts[0]})
    >>> contract.greet()
    'Hi!'

To run the python tests in [./test](./test) run

    brownie test --network hardhat

This will start a hardhat node and run the tests against it. If there is already
a node running on `host:port` brownie will try to connect to that instead.

# Benchmarks

The smart contract `DummyVerifier.sol` simulates the most expensive (in gas) operations of an CAP verifier.

Our "implementation" of the Rescue permutation function is less performant than [Starkware's one](https://etherscan.io/address/0x7B6fc6b18A20823c3d3663E58AB2Af8D780D0AFe#code) .
We provide here the gas and usd cost for one CAP transaction.

# Local network

```
> hardhat run scripts/benchmarks.js
**** NO Merkle tree update****
verify_empty:  51892.42857142857 gas  ------ 39.205975204000005 USD
verify:  375636 gas  ------ 283.802013264 USD
batch_verify:  318932.85714285716 gas  ------ 240.96142796 USD


**** Merkle tree update (Starkware)****
verify_empty:  51895.857142857145 gas  ------ 39.208565572000005 USD
verify:  2562065.285714286 gas  ------ 1935.7018129240003 USD
batch_verify:  2504806 gas  ------ 1892.4410483440001 USD


**** Merkle tree update (NO Starkware)****
verify_empty:  51894.142857142855 gas  ------ 39.207270388 USD
verify:  2816589.5714285714 gas  ------ 2128.001019364 USD
batch_verify:  2759316.285714286 gas  ------ 2084.7296774480005 USD
```

# Rinkeby

- Set the RINKEBY_URL in the .env file. A project can be created at
  https://infura.io/dashboard/ethereum.
- Set the RINKEBY_MNEMONIC in the .env file.
- Run the following command

```
> hardhat --network rinkeby run contracts/scripts/benchmarks.js
```

# Goerli

- Similar to Rinkeby section (replace RINKEBY with GOERLI) and use `--network goerli`.
- Faucets: [Simple](https://goerli-faucet.slock.it),
  [Social](https://faucet.goerli.mudit.blog/).

# Arbitrum on Rinkeby

To run the benchmarks against Arbitrum Rinkeby follow these steps:

- Install [Metamask](https://metamask.io/) in your browser and copy the mnemonic.
- Set the RINKEBY_MNEMONIC in the .env file. Note: this variable may be looked up in the environment so restart your nix shell for the updated env var to be accurate when read.
- Switch metamask to the rinkeby network.
- Get some Rinkeby coins at the [MyCrypto faucet](https://app.mycrypto.com/faucet). You can also use the official [Rinkeby faucet](https://faucet.rinkeby.io) which is less stable but where you can get more coins at once.
- Go to the [Arbitrum bridge](https://bridge.arbitrum.io/) and deposit your
  Rinkeby coins. Leave a bit for the ethereum gas fees. Wait a few minutes until
  your account is funded.
- Run the following command

```
> hardhat --network arbitrum run contracts/scripts/benchmarks.js
```

You can check the deployment and transactions on Arbitrum for the contract
at https://testnet.arbiscan.io/address/0x2FB18F4b4519a5fc792cb6508C6505675BA659E9.

# CAP on Arbitrum (a.k.a CAPA)

Clone the arbitrum submodule (https://gitlab.com/translucence/arbitrum fork)

    git submodule update --init --recursive
    cd contracts/arbitrum
    nix-shell

# Running local arb-dev-node (not officially supported!)

Install dependencies

    pip install -r requirements-dev.txt
    yarn
    yarn install:validator

Build and run `arb-dev-node` and keep it running

    cd packages/arb-rpc-node/cmd/arb-dev-node
    go run arb-dev-node.go

Run scripts

    hardhat --network arbitrum_dev run contracts/scripts/benchmarks.js

We are investigating why some of these transactions revert.

Run tests

    hardhat --network arbitrum_dev contracts/test/test-dummy-cape-contract.js

at the moment this will fail due to gas mismatch.

# Gas Reporter

Set the env var `REPORT_GAS` to get extra output about the gas consumption of
contract functions called in the tests.

    env REPORT_GAS=1 hardhat test

# CI

To locally spin up a docker container like the one used in the CI

    docker run \
        -v $SSH_AUTH_SOCK:/ssh-agent \
        -e SSH_AUTH_SOCK=/ssh-agent \
        -v $(pwd):/code -it lnl7/nix

The code in the current directory will be at `/code`. You may have to delete the
`./node_modules` directory with root permissions afterwards.

To run the CI locally install [gitlab-runner](https://docs.gitlab.com/runner/install/) and run

    gitlab-runner exec docker test

Where the last argument is the name of the job to run.

# Documentation

Extracting documentation from the solidity source is done using a javascript
tool called `solidity-docgen`.

To generate the documentation run

    make-doc

and observe the CLI output.

## Ethereum Asset (Un)Wrapping Workflow

Documentation about wrapping and unwrapping ERC20 tokens into and out of CAPE is described in `./doc/workflow/lib.rs::test`.
