with import ./nix/nixpkgs.nix { };

let
  pre-commit-check = callPackage ./nix/precommit.nix { };
  mySolc = callPackage ./nix/solc-bin { version = "0.8.4"; };
  myPython = [
    poetry
    (poetry2nix.mkPoetryEnv {
      projectDir = ./.;
    })
  ];
  myRustShell = import ./rust/shell.nix { inherit pkgs; };
in
mkShell
{
  buildInputs = [
    go-ethereum
    nodePackages.pnpm
    mySolc
    hivemind # process runner
    nodejs-12_x # nodejs
    jq
    entr # watch files for changes, for example: ls contracts/*.sol | entr -c hardhat compile
    treefmt # multi language formatter
    git # required for pre-commit hook installation
  ]
  ++ myPython
  ++ myRustShell.buildInputs;

  shellHook = ''

    if [ ! -f .env ]; then
      echo "Copying .env.sample to .env"
      cp .env.sample .env
    fi

    echo "Exporting all vars in .env file"
    set -a; source .env; set +a;

    export REPO_DIR=$(pwd)
    export SOLC_VERSION=${mySolc.version}
    export SOLC_PATH=${mySolc}/bin/solc
    export PATH=$(pwd)/bin:$(pwd)/node_modules/.bin:$PATH

    # install pre-commit hooks
    ${pre-commit-check.shellHook}

    git config --local blame.ignoreRevsFile .git-blame-ignore-revs
  ''
  + myRustShell.shellHook;
}
