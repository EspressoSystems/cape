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

### Enviornment
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

### Run
Run the command

    hivemind

to start all background services. For the time being this is just running `geth` with default configuration.

To add additional processes add lines to `Procfile` and (if desired) scripts to run in the `./bin` directory.
