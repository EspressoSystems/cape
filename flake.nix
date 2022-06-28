# Copyright (c) 2022 Espresso Systems (espressosys.com)
# This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
#
# This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
# This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
# You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

{
  description = "A devShell example";

  inputs.nixpkgs.url = "github:nixos/nixpkgs";

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
      # MUSL pkgs
      muslPkgs = import nixpkgs {
        localSystem = "x86_64-linux";
        crossSystem = { config = "x86_64-unknown-linux-musl"; };
      };
      opensslMusl = muslPkgs.openssl.override { static = true; };
      muslPythonEnv = muslPkgs.poetry2nix.mkPoetryEnv { projectDir = ./.; };
      muslRustDeps = with muslPkgs; [
        pkgconfig
        curl
        opensslMusl.dev
        opensslMusl.out
      ];
      stableMuslRustToolchain =
        pkgs.rust-bin.stable."1.60.0".minimal.override {
          extensions = [ "rustfmt" "clippy" "llvm-tools-preview" "rust-src" ];
          targets = [ "x86_64-unknown-linux-musl" ];
        };

      # glibc pkgs
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs { inherit system overlays; };
      mySolc = pkgs.solc;
      pythonEnv = pkgs.poetry2nix.mkPoetryEnv { projectDir = ./.; };
      myPython = with pkgs; [ poetry pythonEnv ];

      stableRustToolchain = pkgs.rust-bin.stable."1.60.0".minimal.override {
        extensions = [ "rustfmt" "clippy" "llvm-tools-preview" "rust-src" ];
      };
      rustDeps = with pkgs;
        [
          pkgconfig
          openssl

          curl
          plantuml
          stableRustToolchain

          cargo-edit
          cargo-sort
        ] ++ lib.optionals stdenv.isDarwin [
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
      muslNixWithFlakes = pkgs.writeShellScriptBin "nix" ''
        exec ${muslPkgs.nixFlakes}/bin/nix --experimental-features "nix-command flakes" "$@"
      '';
    in
    {
      checks = {
        pre-commit-check = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            lint-solidity = {
              enable = true;
              files = "^contracts/contracts/";
              entry = "lint-solidity";
              types = [ "solidity" ];
            };
            check-format = {
              enable = true;
              entry = "treefmt --fail-on-change";
            };
            # The hook "clippy" that ships with nix-precommit-hooks is outdated.
            cargo-clippy = {
              enable = true;
              description = "Lint Rust code.";
              entry = "cargo-clippy --workspace -- -D warnings";
              files = "\\.rs$";
              pass_filenames = false;
            };
            cargo-sort = {
              enable = true;
              description = "Ensure Cargo.toml are sorted";
              entry = "cargo sort -w";
              pass_filenames = false;
            };
            license-header-c-style = {
              enable = true;
              description =
                "Ensure files with c-style comments have license header";
              entry = ''
                insert_license --license-filepath .license-header  --comment-style "//"'';
              types_or = [ "rust" "ts" "javascript" ];
              excludes = [
                "bindings/mod\\.rs" # generated file
              ];
              pass_filenames = true;
            };
            license-header-solidity = {
              enable = true;
              description = "Ensure solidity files have license header";
              entry = ''
                insert_license --license-filepath .license-header-solidity  --comment-style "//"'';
              types = [ "solidity" ];
              pass_filenames = true;
            };
            license-header-hash = {
              enable = true;
              description =
                "Ensure files with hash style comments have license header";
              entry = ''
                insert_license --license-filepath .license-header --comment-style "#"'';
              types_or = [ "bash" "python" "toml" "nix" ];
              excludes = [ "poetry.lock" ];
              pass_filenames = true;
            };
            license-header-html = {
              enable = true;
              description = "Ensure markdown files have license header";
              entry = ''
                insert_license --license-filepath .license-header --comment-style "<!--| ~| -->"'';
              types_or = [ "markdown" ];
              pass_filenames = true;
            };
            make-doc = {
              enable = true;
              description = "Ensure documentation builds";
              entry = "make-doc";
              pass_filenames = false;
              types = [ "solidity" ];
            };
          };
        };
      };
      # A nix base image
      packages.docker = pkgs.dockerTools.buildImage {
        name = "nix-base-docker";
        tag = "latest";
        contents = with pkgs; [ curl coreutils bashInteractive cacert ];
        config = { Env = [ "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt" ]; };
      };
      devShell = pkgs.mkShell {
        buildInputs = with pkgs;
          [
            nixWithFlakes
            (go-ethereum.overrideAttrs (old: {
              # Set deployed bytecode size limit to 32kB (which is also the tx size limit)
              # in order to deploy the TestCAPE contracts that currently exceeds the
              # 24kB size limit.
              patchPhase = old.patchPhase or "" + ''
                substituteInPlace params/protocol_params.go --replace "MaxCodeSize = 24576" "MaxCodeSize = 32768"
              '';
            }))
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
          ] ++ myPython ++ rustDeps;

        RUST_SRC_PATH = "${stableRustToolchain}/lib/rustlib/src/rust/library";
        RUST_BACKTRACE = 1;
        RUST_LOG = "info";

        SOLCX_BINARY_PATH = "${mySolc}/bin";
        SOLC_VERSION = mySolc.version;
        SOLC_PATH = "${mySolc}/bin/solc";
        SOLC_OPTIMIZER_RUNS = "1000";

        shellHook = ''
          echo "Ensuring node dependencies are installed"
          pnpm --recursive install

          if [ ! -f .env ]; then
            echo "Copying .env.sample to .env"
            cp .env.sample .env
          fi

          echo "Exporting all vars in .env file"
          set -a; source .env; set +a;

          # on mac os `bin/pwd -P` returns the canonical path on case insensitive file-systems
          my_pwd=$(/bin/pwd -P 2> /dev/null || pwd)

          export CONTRACTS_DIR=''${my_pwd}/contracts
          export HARDHAT_CONFIG=$CONTRACTS_DIR/hardhat.config.ts
          export PATH=''${my_pwd}/node_modules/.bin:$PATH
          export PATH=$CONTRACTS_DIR/node_modules/.bin:$PATH
          export PATH=''${my_pwd}/bin:$PATH
          export WALLET=''${my_pwd}/wallet

          git config --local blame.ignoreRevsFile .git-blame-ignore-revs
        ''
        # install pre-commit hooks
        + self.checks.${system}.pre-commit-check.shellHook;
      };

      devShells = {
        # shell with all needed tools to build
        # for a statically linked x86_64-unknown-linux-musl target.
        # when running cargo build
        staticShell = muslPkgs.mkShell {

          # build as much as possible with musl
          nativeBuildInputs = with muslPkgs;
            [
              muslNixWithFlakes
              go-ethereum
              mySolc
              hivemind # process runner
            ] ++ muslRustDeps;

          SOLCX_BINARY_PATH = "${mySolc}/bin";
          SOLC_VERSION = mySolc.version;
          SOLC_PATH = "${mySolc}/bin/solc";
          SOLC_OPTIMIZER_RUNS = "1000";
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = "${muslPkgs.llvmPackages_latest.lld}/bin/lld";
          RUSTFLAGS = "-C target-feature=+crt-static -L${opensslMusl.out}/lib/";
          OPENSSL_STATIC = "true";
          OPENSSL_INCLUDE_DIR = "${opensslMusl.dev}/include/";
          OPENSSL_LIB_DIR = "${opensslMusl.dev}/lib/";
          CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";

          shellHook = ''
            echo "Ensuring node dependencies are installed"
            ${pkgs.nodePackages.pnpm}/bin/pnpm --recursive install

            if [ ! -f .env ]; then
              echo "Copying .env.sample to .env"
              cp .env.sample .env
            fi

            echo "Exporting all vars in .env file"
            set -a; source .env; set +a;

            # on mac os `bin/pwd -P` returns the canonical path on case insensitive file-systems
            my_pwd=$(/bin/pwd -P 2> /dev/null || pwd)

            export CONTRACTS_DIR=''${my_pwd}/contracts
            export HARDHAT_CONFIG=$CONTRACTS_DIR/hardhat.config.ts
            export PATH=''${my_pwd}/node_modules/.bin:$PATH
            export PATH=$CONTRACTS_DIR/node_modules/.bin:$PATH
            export PATH=''${my_pwd}/bin:$PATH
            export WALLET=''${my_pwd}/wallet

            export PATH="${pkgs.nodePackages.pnpm}/bin:${pkgs.nodejs-16_x}/bin:${stableMuslRustToolchain}/bin:${pkgs.gcc}/bin/:$PATH"
          '';
        };
      };
    });

}
