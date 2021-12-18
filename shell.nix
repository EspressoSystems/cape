let
  basePkgs = import ./nix/nixpkgs.nix { };

  rust_overlay = with basePkgs; import (fetchFromGitHub
    (lib.importJSON ./nix/oxalica_rust_overlay.json));

  pkgs = import ./nix/nixpkgs.nix { overlays = [ rust_overlay ]; };

  pre-commit-check = pkgs.callPackage ./nix/precommit.nix { };
  mySolc = pkgs.callPackage ./nix/solc-bin { version = "0.8.10"; };
  pythonEnv = pkgs.poetry2nix.mkPoetryEnv {
    projectDir = ./.;
    overrides = pkgs.poetry2nix.overrides.withDefaults
      (import ./nix/poetryOverrides.nix { inherit pkgs; });
  };
  myPython = with pkgs; [
    poetry
    pythonEnv
  ];

  stableToolchain = pkgs.rust-bin.stable."1.56.0".minimal.override {
    extensions = [ "rustfmt" "clippy" "llvm-tools-preview" "rust-src" ];
  };
  rustDeps = with pkgs; [
    pkgconfig
    openssl

    curl
    plantuml
    stableToolchain

    cargo-edit
    cargo-watch
  ] ++ lib.optionals stdenv.isDarwin [
    # required to compile ethers-rs
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.CoreFoundation

    # https://github.com/NixOS/nixpkgs/issues/126182
    libiconv
  ];
in
with pkgs;
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
    nixpkgs-fmt
    git # required for pre-commit hook installation
    netcat-gnu # only used to check for open ports
    cacert
    mdbook # make-doc, documentation generation
    moreutils # includes `ts`, used to add timestamps on CI
  ]
  ++ myPython
  ++ rustDeps;

  RUST_SRC_PATH = "${stableToolchain}/lib/rustlib/src/rust/library";
  RUST_BACKTRACE = 1;
  RUST_LOG = "info";

  SOLCX_BINARY_PATH = "${mySolc}/bin";
  SOLC_VERSION = mySolc.version;
  SOLC_PATH = "${mySolc}/bin/solc";
  SOLC_OPTIMIZER_RUNS = "10000"; # TODO increase this once we have split up contract deployment

  shellHook = ''
    echo "Ensuring node dependencies are installed"
    pnpm --recursive install

    if [ ! -f .env ]; then
      echo "Copying .env.sample to .env"
      cp .env.sample .env
    fi

    echo "Exporting all vars in .env file"
    set -a; source .env; set +a;

    export CONTRACTS_DIR=$(pwd)/contracts
    export HARDHAT_CONFIG=$CONTRACTS_DIR/hardhat.config.ts
    export PATH=$(pwd)/node_modules/.bin:$PATH
    export PATH=$CONTRACTS_DIR/node_modules/.bin:$PATH
    export PATH=$(pwd)/bin:$PATH

    # install pre-commit hooks
    ${pre-commit-check.shellHook}

    git config --local blame.ignoreRevsFile .git-blame-ignore-revs
  '';
}
