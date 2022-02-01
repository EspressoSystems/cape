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
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
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
            };
          };
        };
        mkCapeShell = import ./nix/cape_shell.nix;
      in
      {
        devShell = mkCapeShell {
          inherit pkgs checks;
          rustToolchain = pkgs.rust-bin.stable."1.56.1".minimal.override {
            extensions = [ "rustfmt" "clippy" "llvm-tools-preview" "rust-src" ];
          };
        };

        devShells.nightly = mkCapeShell {
          inherit pkgs checks;
          rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain:
            toolchain.default.override {
              extensions = [ "rustfmt" "clippy" "llvm-tools-preview" "rust-src" ];
            }
          );
          extraPkgs = with pkgs; [ cargo-udeps ];
        };
      }
    );
}
