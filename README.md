# CAP on Ethereum
### Obtaining the source code

    git clone  git@gitlab.com:translucence/cap-on-ethereum

### Dependencies
Install the [nix](https://nixos.org) package manager to provide dependencies with
correct versions. Installation instructions can be found [here](https://nixos.org/download.html).
If in a rush, running the following command and following the on-screen instructions should
work in most cases

    curl -L https://nixos.org/nix/install | sh

To update the pinned version of `nixpkgs` run `nix/update-nix`, optionally passing a github owner and
revision as arguments. The default is: `nix/update-nix nixos master`. Make sure to commit any changed
files in the `./nix` directory afterwards.

The rust overlay can be updated by running `nix/update-rust-overlay`.

### Environment
#### 1. Activate the nix environment
The [direnv](https://direnv.net/) shell extension can be used to activate the environment.
Note that it direnv needs to be [hooked](https://direnv.net/docs/hook.html) into the shell to function.

To enable `direnv` run

    direnv allow

from the root directory of this repo. The first time this may take a few minutes to download all dependencies.
Once the `nix-shell` is activated all dependencies as well as scripts in the `./bin` directory will be in the
`PATH`.

When developing `nix` related code it can sometimes be handy to take direnv out of the equation: to
temporarily disable `direnv` and manually enter a nix shell run

    direnv deny
    nix-shell

#### 2. Install nodejs dependencies
Install the node dependencies with pnpm

    pnpm i

Also run this command after pulling in changes that modify `pnpm-lock.yaml`.

### Development
To start the background services to support interactive development run the command

    hivemind

For the time being this is a `geth` node and a contract compilation watcher.

To add additional processes add lines to `Procfile` and (if desired) scripts to
run in the `./bin` directory.

### Testing (Javascript)
We support running against a go-ethereum (`geth`) or hardhat (shortcut `hh`) node
running on `localhost:8545`.

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

#### Testing against go-ethereum node

Start the geth chain in separate terminal

    run-geth

When running tests against geth

- Tests run faster than with the hardhat node.
- The `console.log` statements in solidity **do nothing** (except consume a tiny amount of gas (?)).
- Failing `require` statements are shown in the`geth` console log.

#### Testing against hardhat node
You can choose to let hardhat start a hardhat node automatically or start a node
yourself and let hardhat connect to it.

##### Separate hardhat node
Start the hardhat node in separate terminal

    hh node

When running tests against this hardhat node

- Tests are slow.
- The `console.log` statements in solidity show in terminal running the node.
- Failing `require` statements are shown in human readable form in the terminal running the node.

##### Hardhat node integrated in test command
It's also possible to run the hardhat node and tests in one command

    hh --network hardhat test

- Tests are slow.
- The `console.log` statements are shown in in the terminal.
- Failing `require` statements are shown in human readable form in the terminal.

### Running scripts
Run a script that connects to the local network (on port 8545)

    hh run scripts/sample-script.js --network localhost

#### Precompiled solidity binaries
Hardhat is configured to use the solc binary installed with nix (see
[nix/solc-bin/default.nix](nix/solc-bin/default.nix)) if matches the version
number. If hardhat downloads and uses another binary a warning is printed the
console.

##### Details
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

# Rust client

All the rust code can be found in the `rust` directory.

## Development
### go-ethereum / geth
Run a geth node (in a separate terminal, from anywhere):

    run-geth

The genesis block is generated with the python script `bin/make-genesis-block`.

If time permits replacing the `run-geth` bash script with a python script that
uses `make-genesis-block` and `hdwallet-derive` could be useful.

### Ethereum contracts
Compile the contracts to extract the abi for the ethers abigen (workflow to be
improved!).
Run the following command from the root of the `cap-on-ethereum` checkout:

    build-abi

Note: structs will only be included in the ABI if there is a (public, I guess)
function that uses them.

Instead of running `geth` and `build-abi` one can also just run

    hivemind

From the root directory of the repo checkout. This will also recompile the
contracts when there are changes to any of the contract files.

To recompile all contracts

    hardhat compile --force

When removing or renaming contracts it can be useful to first remove the
artifacts directory and the compilation cache with

    hardhat clean

### Rust
To run the rust tests

    cargo test

Note that this requires compiled solidity contracts and a running geth node. For
development it's convenient to keep `hivemind` running in a separate terminal
for that purpose.

To watch the rust files and compile on changes

    cargo watch

The command (`check` by default) can be changed with `-x` (for example `cargo
watch -x test`).

#### Examples

Generate a `jf_txn::transfer::TransferNote` and save it to a file `my_note.bin`.
Building with the `--release` flag make this a lot faster.

    cargo run -p cap-rust-sandbox --example create_note --release

Load the file:

    cargo run -p cap-rust-sandbox --example read_note

### Linting
Lint the solidity code using `solhint` by running

    lint-solidity

This runs also as part of the pre-commit hook.
### Formatting
Format all the source files with their respective formatters:

    treefmt

Check if all files are correctly formatted:

    treefmt --fail-on-change

For big reformatting commits, add the revision to the `.git-blame-ignore-revs`
file.

### Git hooks
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

### Interacting with contracts from python
The ethereum development suite [eth-brownie](https://eth-brownie.readthedocs.io)
provides some interactive tools and makes it convenient to test the contracts
with python code.

#### Usage
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

## Local network

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

## Rinkeby

* Set the RINKEBY_URL in the .env file. A project can be created at
  https://infura.io/dashboard/ethereum.
* Set the RINKEBY_MNEMONIC in the .env file.
* Run the following command
```
> hardhat --network rinkeby run scripts/benchmarks.js
```

## Goerli
- Similar to Rinkeby section (replace RINKEBY with GOERLI) and use `--network goerli`.
- Faucets: [Simple](https://goerli-faucet.slock.it),
  [Social](https://faucet.goerli.mudit.blog/).

## Arbitrum on Rinkeby

To run the benchmarks against Arbitrum Rinkeby follow these steps:

* Install [Metamask](https://metamask.io/) in your browser and copy the mnemonic.
* Set the RINKEBY_MNEMONIC in the .env file.
* Switch metamask to the rinkeby network.
* Get some Rinkeby coins at the [MyCrypto faucet](https://app.mycrypto.com/faucet). You can also use the official [Rinkeby faucet](https://faucet.rinkeby.io) which is less stable but where you can get more coins at once.
* Go to the [Arbitrum bridge](https://bridge.arbitrum.io/) and deposit your
 Rinkeby coins. Leave a bit for the ethereum gas fees. Wait a few minutes until
 your account is funded.
* Run the following command
```
> hardhat --network arbitrum run scripts/benchmarks.js
```

You can check the deployment and transactions on Arbitrum for the contract
at https://testnet.arbiscan.io/address/0x2FB18F4b4519a5fc792cb6508C6505675BA659E9.

# CAP on Arbitrum (a.k.a CAPA)

Clone the arbitrum submodule (https://gitlab.com/translucence/arbitrum fork)

    git submodule update --init --recursive
    cd arbitrum
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

    hardhat --network arbitrum_dev run scripts/benchmarks.js

We are investigating why some of these transactions revert.

Run tests

    hardhat --network arbitrum_dev test/test-dummy-verifier.js

at the moment this will fail due to gas mismatch.

## Gas Reporter
Set the env var `REPORT_GAS` to get extra output about the gas consumption of
contract functions called in the tests.

    env REPORT_GAS=1 hh test

## CI
To locally spin up a docker container like the one used in the CI

    docker run \
        -v $SSH_AUTH_SOCK:/ssh-agent \
        -e SSH_AUTH_SOCK=/ssh-agent \
        -v $(pwd):/code -it lnl7/nix

The code in the current directory will be at `/code`. You may have to delete the
`./node_modules` directory with root permissions afterwards.

## Documentation
Extracting documentation from the solidity source is done using a javascript tool called `solidity-docgen`
If it run it, it should generate documentation into the `/doc` directory

Example:
```bash
pnpm install
solidity-docgen --solc-module solc-0.8 -o ./doc
```

