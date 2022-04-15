# Copyright (c) 2022 Espresso Systems (espressosys.com)
# This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
#
# This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
# This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
# You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

{
  description = "A devShell example";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

  inputs.flake-utils.url = "github:numtide/flake-utils";

  inputs.flake-compat.url = "github:edolstra/flake-compat";
  inputs.flake-compat.flake = false;

  inputs.rust-overlay.url = "github:oxalica/rust-overlay";

  inputs.pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
  # See https://github.com/cachix/pre-commit-hooks.nix/pull/122
  inputs.pre-commit-hooks.inputs.flake-utils.follows = "flake-utils";
  inputs.pre-commit-hooks.inputs.nixpkgs.follows = "nixpkgs";

  outputs =
    { self
    , nixpkgs
    , flake-utils
    , flake-compat
    , rust-overlay
    , pre-commit-hooks
    , ...
    }:
    flake-utils.lib.eachDefaultSystem (system:
    let
      xpkgs = import nixpkgs {
        system = "x86_64-linux"; 
        overlays = [ (import rust-overlay) ];
      };
      pkgs = import nixpkgs {
        localSystem = "x86_64-linux"; 
        crossSystem = { config = "x86_64-unknown-linux-musl"; };
        # overlays = [ (import rust-overlay) ];
      };
      mySolc = xpkgs.callPackage ./nix/solc-bin { version = "0.8.10"; };
      pythonEnv = xpkgs.poetry2nix.mkPoetryEnv {
        projectDir = ./.;
      };
      myPython = with xpkgs; [
        poetry
        pythonEnv
      ];
    in
    {
      # A nix base image
      # packages.docker = pkgs.dockerTools.buildImage {
      #   name = "nix-base-docker";
      #   tag = "latest";
      #   contents = with pkgs; [
      #     curl
      #     coreutils
      #     bashInteractive
      #     cacert
      #   ];
      #   config = {
      #     Env = [
      #       "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
      #     ];
      #   };
      # };
      devShell =
        let
          # mySolc = pkgs.callPackage ./nix/solc-bin { version = "0.8.10"; };
          # pythonEnv = pkgs.poetry2nix.mkPoetryEnv {
          #   projectDir = ./.;
          # };
          # myPython = with pkgs; [
          #   poetry
          #   pythonEnv
          # ];

          stableToolchain = xpkgs.rust-bin.stable."1.60.0".minimal.override {
            extensions = [ "rustfmt" "clippy" "llvm-tools-preview" "rust-src" ];
	    targets = [ "x86_64-unknown-linux-musl" ];
          };
          openssl_musl = (((import nixpkgs { localSystem = "x86_64-linux"; crossSystem = { config = "x86_64-unknown-linux-musl"; }; }).openssl).override {static = true;}).dev;
          # for some odd reason only `out` exposes libssl.a
          openssl_musl_lib = (((import nixpkgs { localSystem = "x86_64-linux"; crossSystem = { config = "x86_64-unknown-linux-musl"; }; }).openssl).override {static = true;}).out;
          rustDeps = with pkgs; [
            pkgconfig
            openssl_musl
            openssl_musl_lib
            curl
            # plantuml
            stableToolchain

            # cargo-edit
            # cargo-sort
          ] ++ lib.optionals stdenv.isDarwin [
            # required to compile ethers-rs
            darwin.apple_sdk.frameworks.Security
            darwin.apple_sdk.frameworks.CoreFoundation

            # https://github.com/NixOS/nixpkgs/issues/126182
            libiconv
          ] ++ lib.optionals (stdenv.system != "aarch64-darwin") [
            # cargo-watch # broken: https://github.com/NixOS/nixpkgs/issues/146349
          ];
          # nixWithFlakes allows pre v2.4 nix installations to use flake commands (like `nix flake update`)
        in
        pkgs.mkShell
          {

            SOLCX_BINARY_PATH = "${mySolc}/bin";
            SOLC_VERSION = mySolc.version;
            SOLC_PATH = "${mySolc}/bin/solc";
            # TODO: increase this when contract size limit is not a problem
            SOLC_OPTIMIZER_RUNS = "20";

            buildInputs = rustDeps ++ [
            myPython
              xpkgs.nodePackages.pnpm
              mySolc
              xpkgs.hivemind # process runner
              xpkgs.nodejs-16_x
              xpkgs.plantuml
            ];

            # with pkgs; [
            # stableToolchain
            #   nixWithFlakes
            #   go-ethereum
            #   nodePackages.pnpm
            #   mySolc
            #   hivemind # process runner
            #   nodejs-16_x # nodejs
            #   jq
            #   entr # watch files for changes, for example: ls contracts/*.sol | entr -c hardhat compile
            #   treefmt # multi language formatter
            #   nixpkgs-fmt
            #   git # required for pre-commit hook installation
            #   netcat-gnu # only used to check for open ports
            #   cacert
            #   mdbook # make-doc, documentation generation
            #   moreutils # includes `ts`, used to add timestamps on CI
            #   # inputs.nixpkgs.legacyPackages.x86_64-musl.openssl
            # ]
            # ++ myPython
            # ++ rustDeps;

            RUST_SRC_PATH = "${stableToolchain}/lib/rustlib/src/rust/library";
            RUST_BACKTRACE = 1;
            RUST_LOG = "info";

            shellHook = ''

              my_pwd=$(/bin/pwd -P 2> /dev/null || pwd)

             set -a; source .env; set +a;
             pnpm --recursive install

              export CONTRACTS_DIR=''${my_pwd}/contracts
              export HARDHAT_CONFIG=$CONTRACTS_DIR/hardhat.config.ts
              export PATH=''${my_pwd}/node_modules/.bin:$PATH
              export PATH=$CONTRACTS_DIR/node_modules/.bin:$PATH
              export PATH=''${my_pwd}/bin:$PATH
              export WALLET=''${my_pwd}/wallet



              export RUSTFLAGS='-C target-feature=+crt-static'
              export CARGO_BUILD_TARGET='x86_64-unknown-linux-musl'
              export OPENSSL_LIB_DIR='${openssl_musl}/lib/'
              export OPENSSL_INCLUDE_DIR='${openssl_musl}/include/'
              export OPENSSL_STATIC=true
              export RUSTFLAGS='-L${openssl_musl_lib}/lib/'
            '';
          };

    }
    );
}
