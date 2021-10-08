# AAP on Ethereum
### Obtaining the source code

    git clone  git@gitlab.com:translucence/aap-on-ethereum

### Dependencies
Install the [nix](https://nixos.org) package manager to provide dependencies with
correct versions. Installation instructions can be found [here](https://nixos.org/download.html).
If in a rush, running the following command and following the on-screen instructions should
work in most cases

    curl -L https://nixos.org/nix/install | sh

To update the pinned version of `nixpkgs` run `nix/update-nix`, optionally passing a github owner and
revision as arguments. The default is: `nix/update-nix nixos master`. Make sure to commit any changed
files in the `./nix` directory afterwards.

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
### Run
Run the command

    hivemind

to start all background services. For the time being this is just running `geth` with default configuration.

To add additional processes add lines to `Procfile` and (if desired) scripts to run in the `./bin` directory.

### Testing (Javascript)
We support running against a go-ethereum (geth) or hardhat (shortcut `hh`) node running on `localhost:8545`.

#### Testing against go-ethereum node
We use a [patched version](https://gitlab.com/translucence/go-ethereum) of geth
that enables `EIP-2537` (BLS precompiles).

Start the geth chain in separate terminal

    run-geth

When running tests against geth

- Tests run faster than with the hardhat node.
- The `console.log` statements in solidity **do nothing** (except consume a tiny amount of gas (?)).
- Failing `require` statements are shown in the`geth` console log.

#### Testing against hardhat node
The hardhat node enables `EIP-2537` (BLS precompiles) via monkeypatching in `hardhat.config.js`.
This may cause undesirable effects with some hardhat features but so far we have not found any.

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

#### Running scripts
Run a script that connect to the local network (on port 8545)

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

**Note that this directory has its own `shell.nix` file.**

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
Run the following command from the root of the `aap-on-ethereum` checkout:

    build-abi

Note: structs will only be included in the ABI if there is a (public, I guess)
function that uses them.

Instead of running `geth` and `build-abi` one can also just run

    hivemind

From the root directory of the repo checkout.
This will also recompile the contracts when there are changes to any of the contract files.

### Rust
Watch directory and run tests on changes:

    cd rust
    cargo watch -x test

If some compilation error occurs, delete the files generated previously (from the root of theproject):

    rm -R artifacts rust/contracts

### Examples

Remember to "cd" into `rust` directory and launch the shell:

    cd rust
    nix-shell

Generate a `jf_txn::transfer::TransferNote` and save it to a file `my_note.bin`:

    cargo run -p aap-rust-sandbox --example create_note

Load the file:

    cargo run -p aap-rust-sandbox --example read_note

### Formatting
Format all the source files with their respective formatters:

    treefmt

Check if all files are correctly formatted:

    treefmt --fail-on-change

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

# Benchmarks

The smart contract `DummyVerifier.sol` simulates the most expensive (in gas) operations of an AAP verifier.

Our "implementation" of the Rescue permutation function is less performant than [Starkware's one](https://etherscan.io/address/0x7B6fc6b18A20823c3d3663E58AB2Af8D780D0AFe#code) .
We provide here the gas and usd cost for one AAP transaction.

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

To run the benchmarks agains Arbitrum Rinkeby follow these steps:

* Install [Metamask](https://metamask.io/) in your browser and copy the mnemonic.
* Set the ARBITRUM_MNEMONIC in the .env file.
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



## Gas Reporter
Set the env var `REPORT_GAS` to get extra output about the gas consumption of
contract functions called in the tests.

    env REPORT_GAS=1 hh test
