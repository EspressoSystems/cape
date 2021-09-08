# AAP on Ethereum
### Obtaining the source code
Check out the source with submodules by running

    git clone --recursive git@gitlab.com:translucence/aap-on-ethereum

Or, to initialize the submodules in an existing checkout

    git submodule update --init --recursive

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

Also run this command after pulling in changes that modify `package-lock.json`.
### Run
Run the command

    hivemind

to start all background services. For the time being this is just running `geth` with default configuration.

To add additional processes add lines to `Procfile` and (if desired) scripts to run in the `./bin` directory.

### Testing
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
