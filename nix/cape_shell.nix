{ pkgs, checks, rustToolchain, extraPkgs ? [ ] }:
let
  mySolc = pkgs.callPackage ./solc-bin { version = "0.8.10"; };
  pythonEnv = pkgs.poetry2nix.mkPoetryEnv {
    projectDir = ../.;
  };
  myPython = with pkgs; [
    poetry
    pythonEnv
  ];

  rustDeps = with pkgs; [
    pkgconfig
    openssl

    curl
    plantuml
    rustToolchain

    cargo-edit
  ]
  ++ lib.optionals stdenv.isDarwin [
    # required to compile ethers-rs
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.CoreFoundation

    # https://github.com/NixOS/nixpkgs/issues/126182
    libiconv
  ] ++ lib.optionals (stdenv.system != "aarch64-darwin") [
    cargo-watch # broken: https://github.com/NixOS/nixpkgs/issues/146349
  ];
  # nixWithFlakes allows pre v2.4 nix installations to use flake commands (like `nix flake update`)
  nixWithFlakes = pkgs.writeShellScriptBin "nix" ''
    exec ${pkgs.nixFlakes}/bin/nix --experimental-features "nix-command flakes" "$@"
  '';
in
pkgs.mkShell
{
  buildInputs = with pkgs; [
    nixWithFlakes
    go-ethereum
    nodePackages.pnpm
    mySolc
    hivemind # process runner
    nodejs-16_x # nodejs
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
  ++ rustDeps
  ++ extraPkgs;

  RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
  RUST_BACKTRACE = 1;
  RUST_LOG = "info";

  SOLCX_BINARY_PATH = "${mySolc}/bin";
  SOLC_VERSION = mySolc.version;
  SOLC_PATH = "${mySolc}/bin/solc";
  # TODO: increase this when contract size limit is not a problem
  SOLC_OPTIMIZER_RUNS = "20";

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

    git config --local blame.ignoreRevsFile .git-blame-ignore-revs
  ''
  # install pre-commit hooks
  + checks.pre-commit-check.shellHook;
}
