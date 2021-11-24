let
  basePkgs = import ./nix/nixpkgs.nix { };

  rust_overlay = with basePkgs; import (fetchFromGitHub
    (lib.importJSON ./nix/oxalica_rust_overlay.json));

  pkgs = import ./nix/nixpkgs.nix { overlays = [ rust_overlay ]; };

  pre-commit-check = pkgs.callPackage ./nix/precommit.nix { };
  # Broken on aarch64-darwin (nix >= 2.4, M1 macs)
  #   https://github.com/cachix/pre-commit-hooks.nix/issues/131
  preCommitHook =
    if pkgs.stdenv.isx86_64
    then
      "${pre-commit-check.shellHook}"
    else
      "echo 'Warning: not installing pre-commit hooks'";

  mySolc = pkgs.callPackage ./nix/solc-bin { version = "0.8.4"; };
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
    extensions = [ "rustfmt" "clippy" "llvm-tools-preview" ];
  };
  darwinDeps = with pkgs; [
    # required to compile ethers-rs
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.CoreFoundation

    # https://github.com/NixOS/nixpkgs/issues/126182
    libiconv
  ];
  rustDeps = with pkgs; [
    pkgconfig
    openssl

    curl
    plantuml
    stableToolchain

    cargo-edit
  ] ++ lib.optionals stdenv.isDarwin darwinDeps
  # cargo-watch does not build on aarch darwin (M1 macs)
  #   https://github.com/NixOS/nixpkgs/issues/146349
  ++ lib.optionals stdenv.isx86_64 [ cargo-watch ];
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
    pandoc # make-doc, documentation generation
  ]
  ++ myPython
  ++ rustDeps;

  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
  RUST_BACKTRACE = 1;
  RUST_LOG = "info";

  SOLCX_BINARY_PATH = "${mySolc}/bin";
  SOLC_VERSION = mySolc.version;
  SOLC_PATH = "${mySolc}/bin/solc";
  SOLC_OPTIMIZER_RUNS = "1000000";

  shellHook = ''
    echo "Ensuring node dependencies are installed"
    pnpm i

    if [ ! -f .env ]; then
      echo "Copying .env.sample to .env"
      cp .env.sample .env
    fi

    echo "Exporting all vars in .env file"
    set -a; source .env; set +a;

    export PATH=$(pwd)/bin:$(pwd)/node_modules/.bin:$PATH

    ${preCommitHook}

    git config --local blame.ignoreRevsFile .git-blame-ignore-revs
  '';
}
