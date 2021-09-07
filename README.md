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

### Run
Run the command

    hivemind

to start all background services. For the time being this is just running `geth` with default configuration.

To add additional processes add lines to `Procfile` and (if desired) scripts to run in the `./bin` directory.

### Hardhat

Run a script that connect to the local network (on port 8545)

```
> hardhat run scripts/sample-script.js --network localhost
```

### Tests

* Launch a hardhat node based on ethereumjs (easier debugging)

```
hardhat node
```

* Launch the private `geth` blockchain (more like the real thing)
```
> cd local_network_conf
> ./run_private_geth.sh
```

* Run the tests against geth or hardhat node running on `localhost:8545`

```
> hardhat --network local test
```

* Run the tests in standalone fashion

```
> hardhat test
```
